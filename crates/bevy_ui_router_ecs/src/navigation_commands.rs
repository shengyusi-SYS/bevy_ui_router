//! Ordinary navigation through Bevy's deferred command queue.

use std::fmt;

use bevy_ecs::{
    error::{BevyError, ErrorContext, ErrorHandler, warn},
    system::{Command, Commands},
    world::World,
};
use bevy_ui_router_core::{NavigationKind, location::RouteLocation};

use crate::{NavigationEnqueueError, NavigationEnqueueErrorKind, RouterHandle, RouterNavigation};

/// Navigation commands that can coexist with [`crate::RouteInputs`] and queries
/// of application state in the same system.
pub trait RouteNavigationCommandsExt {
    /// Selects the router for subsequent deferred navigation commands.
    ///
    /// Commands are submitted to the existing navigation queue when Bevy applies
    /// deferred commands. Calling this does not navigate or allocate a request ID.
    fn router(&mut self, router: RouterHandle) -> RouterCommands<'_>;
}

impl RouteNavigationCommandsExt for Commands<'_, '_> {
    fn router(&mut self, router: RouterHandle) -> RouterCommands<'_> {
        RouterCommands {
            commands: self.reborrow(),
            router,
            error_handler: warn,
        }
    }
}

/// Deferred navigation for one router.
///
/// String destinations use [`RouteLocation::from_address`]. A structured
/// [`RouteLocation`] retains its exact search and fragment values.
///
/// Enqueue failures are logged at warning level by default. Use
/// [`Self::with_error_handler`] to select a Bevy error handler. Successful
/// requests use the normal router snapshots; these commands do not promise a
/// synchronous request ID or publication in the same update. Use
/// [`RouterNavigation`] directly when the immediate enqueue result is needed.
pub struct RouterCommands<'a> {
    commands: Commands<'a, 'a>,
    router: RouterHandle,
    error_handler: ErrorHandler,
}

impl RouterCommands<'_> {
    /// Selects an error handler for commands issued through this value.
    ///
    /// The handler receives a [`NavigationCommandError`] inside `BevyError`;
    /// `downcast_ref` exposes the router, operation, and original enqueue error.
    pub fn with_error_handler(mut self, handler: ErrorHandler) -> Self {
        self.error_handler = handler;
        self
    }

    /// Queues a Push, which appends a history entry on successful publication.
    pub fn push(&mut self, destination: impl Into<RouteLocation>) -> &mut Self {
        self.queue(NavigationOperation::Push(destination.into()))
    }

    /// Queues a Replace, which replaces the current entry on successful publication.
    pub fn replace(&mut self, destination: impl Into<RouteLocation>) -> &mut Self {
        self.queue(NavigationOperation::Replace(destination.into()))
    }

    /// Queues a Back through the existing history admission and execution path.
    pub fn back(&mut self) -> &mut Self {
        self.queue(NavigationOperation::Back)
    }

    fn queue(&mut self, operation: NavigationOperation) -> &mut Self {
        self.commands.queue_handled(
            NavigationCommand {
                router: self.router,
                operation,
            },
            self.error_handler,
        );
        self
    }
}

/// A deferred navigation failed to enter the router's request queue.
///
/// No request ID exists for this failure. Once admitted, a request's execution
/// and publication failures are instead reported by the normal snapshots.
#[derive(Debug)]
pub struct NavigationCommandError {
    router: RouterHandle,
    operation: NavigationKind,
    source: NavigationEnqueueError,
}

impl NavigationCommandError {
    pub(crate) fn new(
        router: RouterHandle,
        operation: NavigationKind,
        source: NavigationEnqueueError,
    ) -> Self {
        Self {
            router,
            operation,
            source,
        }
    }

    pub fn router(&self) -> RouterHandle {
        self.router
    }

    pub fn operation(&self) -> NavigationKind {
        self.operation
    }

    pub fn enqueue_error(&self) -> &NavigationEnqueueError {
        &self.source
    }
}

impl fmt::Display for NavigationCommandError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{:?} for {:?} was not enqueued: {}",
            self.operation, self.router, self.source,
        )
    }
}

impl std::error::Error for NavigationCommandError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

pub(crate) fn warn_navigation_enqueue_error(
    router: RouterHandle,
    operation: NavigationKind,
    source: NavigationEnqueueError,
) {
    warn(
        BevyError::from(NavigationCommandError::new(router, operation, source)),
        ErrorContext::Command {
            name: "Router initial navigation".into(),
        },
    );
}

enum NavigationOperation {
    Push(RouteLocation),
    Replace(RouteLocation),
    Back,
}

impl NavigationOperation {
    fn kind(&self) -> NavigationKind {
        match self {
            Self::Push(_) => NavigationKind::Push,
            Self::Replace(_) => NavigationKind::Replace,
            Self::Back => NavigationKind::Back,
        }
    }
}

struct NavigationCommand {
    router: RouterHandle,
    operation: NavigationOperation,
}

impl Command for NavigationCommand {
    type Out = Result<(), NavigationCommandError>;

    fn apply(self, world: &mut World) -> Self::Out {
        let kind = self.operation.kind();
        let result = if let Some(mut navigation) = world.get_resource_mut::<RouterNavigation>() {
            match self.operation {
                NavigationOperation::Push(location) => navigation.push(self.router, location),
                NavigationOperation::Replace(location) => navigation.replace(self.router, location),
                NavigationOperation::Back => navigation.back(self.router),
            }
        } else {
            Err(NavigationEnqueueError::new(
                NavigationEnqueueErrorKind::UnknownRouter,
            ))
        };
        result
            .map(|_| ())
            .map_err(|source| NavigationCommandError::new(self.router, kind, source))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use bevy_ecs::world::CommandQueue;

    use super::*;

    static FAILURES: Mutex<Vec<(RouterHandle, NavigationKind, NavigationEnqueueErrorKind)>> =
        Mutex::new(Vec::new());

    fn observe_error(error: BevyError, _context: ErrorContext) {
        let error = error.downcast_ref::<NavigationCommandError>().unwrap();
        FAILURES.lock().unwrap().push((
            error.router(),
            error.operation(),
            error.enqueue_error().kind(),
        ));
    }

    #[test]
    fn failed_commands_report_original_router_and_operation_only_when_applied() {
        let mut world = World::new();
        let mut queue = CommandQueue::default();
        let router = RouterHandle::fixture(42, 7);
        Commands::new(&mut queue, &world)
            .router(router)
            .with_error_handler(observe_error)
            .push("/items?tab=info#top")
            .replace(RouteLocation::new("/other"))
            .back();
        assert!(FAILURES.lock().unwrap().is_empty());

        queue.apply(&mut world);

        let actual = std::mem::take(&mut *FAILURES.lock().unwrap());
        assert_eq!(
            actual,
            vec![
                (
                    router,
                    NavigationKind::Push,
                    NavigationEnqueueErrorKind::UnknownRouter
                ),
                (
                    router,
                    NavigationKind::Replace,
                    NavigationEnqueueErrorKind::UnknownRouter
                ),
                (
                    router,
                    NavigationKind::Back,
                    NavigationEnqueueErrorKind::UnknownRouter
                ),
            ]
        );
    }

    #[test]
    fn ordinary_enqueue_rejection_uses_non_panicking_default_handler() {
        use bevy_ui_router_core::prelude::{
            RouteNodeOptions, RoutePattern, RouteTree, RouteViewKey,
        };
        use bevy_ui_router_view::RouteViewRegistry;

        let mut world = World::new();
        let view = RouteViewKey::new("home").unwrap();
        let routes = RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/").unwrap(),
                RouteNodeOptions::new().with_view(view.clone()),
            )
            .unwrap()
            .build()
            .unwrap();
        let mut navigation = RouterNavigation::new();
        let router = navigation
            .install(routes, RouteViewRegistry::new(), view, 0)
            .unwrap();
        // The ordinary startup condition: installed router, no admitted host yet.
        assert_eq!(
            navigation
                .push(router, RouteLocation::new("/"))
                .unwrap_err()
                .kind(),
            NavigationEnqueueErrorKind::HostUnavailable,
        );
        world.insert_resource(navigation);
        let mut queue = CommandQueue::default();
        Commands::new(&mut queue, &world)
            .router(router)
            .push("/")
            .back();
        queue.apply(&mut world);
    }
}
