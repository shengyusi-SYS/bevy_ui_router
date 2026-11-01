//! Application declarations compiled to the existing Core/View routing model.

use bevy_scene::Scene;
use bevy_ui_router_bsn::{BsnBuildError, BsnSceneFactory};
use bevy_ui_router_core::prelude::{
    RouteError, RouteExplicitKey, RouteLocation, RouteMeta, RouteNodeOptions, RoutePattern,
    RouteViewInput,
};

use crate::transition::RouteTransitionDriver;

/// A page or layout and the factory that creates its content.
///
/// Declarations are inert: factories run only when the normal navigation
/// pipeline needs fresh content. Children use the parent's `DefaultOutlet`.
pub struct BsnRoute {
    pub(super) pattern: Result<RoutePattern, RouteError>,
    pub(super) label: String,
    pub(super) options: RouteNodeOptions,
    pub(super) factory: BsnSceneFactory,
    pub(super) error_factory: Option<BsnSceneFactory>,
    pub(super) children: Vec<BsnRoute>,
}

/// Declares a path relative to its parent, with a directly associated factory.
///
/// As with `RoutePattern::path`, a leading slash does not escape the parent.
/// Invalid patterns are reported by `AppBsnExt::add_bsn_router` with their
/// declaration location, so callers need not unwrap every route separately.
pub fn page<F, S>(path: impl Into<String>, factory: F) -> BsnRoute
where
    F: FnMut(&RouteViewInput) -> Result<S, BsnBuildError> + Send + Sync + 'static,
    S: Scene,
{
    let path = path.into();
    BsnRoute::new(RoutePattern::path(path.clone()), path, factory)
}

/// Declares an explicit pathless layout; its Scene supplies `DefaultOutlet`.
pub fn layout<F, S>(factory: F) -> BsnRoute
where
    F: FnMut(&RouteViewInput) -> Result<S, BsnBuildError> + Send + Sync + 'static,
    S: Scene,
{
    BsnRoute::new(Ok(RoutePattern::pathless()), "<layout>".into(), factory)
}

/// Declares the default child selected when its parent has no remaining path.
pub fn index<F, S>(factory: F) -> BsnRoute
where
    F: FnMut(&RouteViewInput) -> Result<S, BsnBuildError> + Send + Sync + 'static,
    S: Scene,
{
    BsnRoute::new(Ok(RoutePattern::index()), "<index>".into(), factory)
}

pub(super) fn boxed_factory<F, S>(mut factory: F) -> BsnSceneFactory
where
    F: FnMut(&RouteViewInput) -> Result<S, BsnBuildError> + Send + Sync + 'static,
    S: Scene,
{
    Box::new(move |input| factory(input).map(|scene| Box::new(scene) as Box<dyn Scene>))
}

impl BsnRoute {
    fn new<F, S>(pattern: Result<RoutePattern, RouteError>, label: String, factory: F) -> Self
    where
        F: FnMut(&RouteViewInput) -> Result<S, BsnBuildError> + Send + Sync + 'static,
        S: Scene,
    {
        Self {
            pattern,
            label,
            options: RouteNodeOptions::new(),
            factory: boxed_factory(factory),
            error_factory: None,
            children: Vec::new(),
        }
    }

    pub fn children(mut self, children: impl IntoIterator<Item = BsnRoute>) -> Self {
        self.children.extend(children);
        self
    }

    /// Allows this normal instance to remain after leaving the active branch.
    ///
    /// Requires nonzero router capacity and a surviving normal parent scope.
    /// Parent and child opt in independently; the host-root route is ineligible.
    pub fn keep_alive(mut self) -> Self {
        self.options = self.options.with_keep_alive(true);
        self
    }

    /// Allows this node to reuse content across raw search changes.
    ///
    /// Each ancestor and child chooses independently. Reused content reads the
    /// published location through `RouteInputs`, rather than its factory input.
    pub fn preserve_on_search(mut self) -> Self {
        self.options = self.options.with_preserve_on_search(true);
        self
    }

    /// Handles route failures at this node using a fresh error Scene.
    pub fn error_view<F, S>(mut self, factory: F) -> Self
    where
        F: FnMut(&RouteViewInput) -> Result<S, BsnBuildError> + Send + Sync + 'static,
        S: Scene,
    {
        self.error_factory = Some(boxed_factory(factory));
        self
    }

    pub fn explicit_key(mut self, key: RouteExplicitKey) -> Self {
        self.options = self.options.with_explicit_key(key);
        self
    }

    pub fn meta(mut self, meta: RouteMeta) -> Self {
        self.options = self.options.with_meta(meta);
        self
    }
}

impl std::fmt::Debug for BsnRoute {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BsnRoute")
            .field("pattern", &self.pattern)
            .field("options", &self.options)
            .field("error_view", &self.error_factory.is_some())
            .field("children", &self.children)
            .finish_non_exhaustive()
    }
}

/// A complete BSN routing declaration with one explicit root.
///
/// Use `page` for a single root page, or `layout(...).children(...)` for nested
/// pages. The application still creates and styles its own router host.
pub struct BsnRouter {
    pub(super) root: BsnRoute,
    pub(super) fallback: Option<BsnSceneFactory>,
    pub(super) keep_alive_capacity: usize,
    pub(super) initial: Option<RouteLocation>,
    pub(super) transition_driver: Option<Box<dyn RouteTransitionDriver>>,
}

impl BsnRouter {
    pub fn new(root: BsnRoute) -> Self {
        Self {
            root,
            fallback: None,
            keep_alive_capacity: 0,
            initial: None,
            transition_driver: None,
        }
    }

    /// Supplies the router-wide error page required by the existing pipeline.
    pub fn fallback<F, S>(mut self, factory: F) -> Self
    where
        F: FnMut(&RouteViewInput) -> Result<S, BsnBuildError> + Send + Sync + 'static,
        S: Scene,
    {
        self.fallback = Some(boxed_factory(factory));
        self
    }

    /// Limits inactive opted-in instances; the default and zero disable retention.
    /// Parent layouts and their children count separately.
    pub fn keep_alive_capacity(mut self, capacity: usize) -> Self {
        self.keep_alive_capacity = capacity;
        self
    }

    /// Submits this default location once after the first ready host.
    ///
    /// Strings use `RouteLocation::from_address`. A successfully enqueued explicit
    /// navigation supersedes this default before it is submitted. A consumed
    /// default is never replayed after host re-registration.
    pub fn initial(mut self, location: impl Into<RouteLocation>) -> Self {
        self.initial = Some(location.into());
        self
    }

    /// Adds an application-owned transition driver without choosing an effect.
    pub fn transition_driver(mut self, driver: impl RouteTransitionDriver) -> Self {
        self.transition_driver = Some(Box::new(driver));
        self
    }
}

impl std::fmt::Debug for BsnRouter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BsnRouter")
            .field("root", &self.root)
            .field("fallback", &self.fallback.is_some())
            .field("keep_alive_capacity", &self.keep_alive_capacity)
            .field("initial", &self.initial)
            .field("transition_driver", &self.transition_driver.is_some())
            .finish()
    }
}
