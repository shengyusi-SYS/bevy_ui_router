//! Application-owned animation around an authoritative route publication.
//!
//! The router supplies presence and bounded ownership; the application supplies
//! its own animation systems. No clock, curve, or effect is part of this API.

use std::{
    fmt,
    hash::{Hash, Hasher},
    sync::Arc,
};

use bevy_ecs::{entity::Entity, system::Commands, world::World};
use bevy_ui_router_core::{
    NavigationKind, frame::RouteFrameVersion, ids::NavigationRequestId, location::RouteLocation,
};

use crate::app_ext::RouterHandle;

mod presence;
mod session;
mod surface;
#[cfg(feature = "tween")]
pub(crate) mod tween;
pub(crate) use presence::{
    holds_visible_claim, reap_absent_presence, release_transition_presence,
    seal_transition_presence,
};
pub(crate) use session::*;
pub(crate) use surface::{
    TransitionSurfaceBinding, TransitionTargetRole, transition_surface_display,
};

struct TransitionIdentity {
    router: RouterHandle,
}

/// Opaque identity of one application presentation session.
///
/// Copies identify the same session, but never carry lifecycle or cleanup
/// ownership. Commands for a stale or finished identity have no effect.
#[derive(Clone)]
pub struct RouteTransitionId(Arc<TransitionIdentity>);

impl RouteTransitionId {
    fn new(router: RouterHandle) -> Self {
        Self(Arc::new(TransitionIdentity { router }))
    }

    pub(crate) fn router(&self) -> RouterHandle {
        self.0.router
    }
}

impl PartialEq for RouteTransitionId {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for RouteTransitionId {}
impl Hash for RouteTransitionId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.0).hash(state);
    }
}
impl fmt::Debug for RouteTransitionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RouteTransitionId(..)")
    }
}

/// A changed frame and its temporarily authorized presentation surface.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RouteTransitionTarget {
    frame: RouteFrameVersion,
    surface: Entity,
    content: Entity,
    outermost: bool,
}

impl RouteTransitionTarget {
    pub(crate) fn new(
        frame: RouteFrameVersion,
        surface: Entity,
        content: Entity,
        outermost: bool,
    ) -> Self {
        Self {
            frame,
            surface,
            content,
            outermost,
        }
    }

    /// Exact frame identity for this target, not a reusable route key.
    pub fn frame(&self) -> &RouteFrameVersion {
        &self.frame
    }

    /// Existing router shell whose presentation values the application may edit.
    ///
    /// While this session authorizes this target, applications may change layout
    /// fields of its existing `Node` (except `display`), `UiTransform`, and
    /// `ZIndex`. Relationships, components, visibility, input, and identity remain
    /// router-owned. Put animation controllers on separate application entities
    /// and explicitly target this Entity; do not attach them to the shell.
    ///
    /// Stop all writers when `cancelled` runs, before releasing this target, or
    /// before finishing the session. Retaining this Entity does not extend that
    /// permission beyond the session.
    pub fn surface(&self) -> Entity {
        self.surface
    }

    /// Authored content root, available for application-specific animation.
    pub fn content(&self) -> Entity {
        self.content
    }

    /// Whether no other changed target of the same role encloses this target.
    ///
    /// Whole-page effects normally animate only these targets, avoiding applying
    /// the same transform to both a parent frame and its nested child. Applications
    /// can deliberately compose effects on the other changed targets as well.
    pub fn is_outermost(&self) -> bool {
        self.outermost
    }
}

/// Frozen routing facts and visual targets for a single preparation attempt.
#[derive(Clone, Debug)]
pub struct RouteTransitionContext {
    id: RouteTransitionId,
    router: RouterHandle,
    request: NavigationRequestId,
    navigation: NavigationKind,
    from: Option<RouteLocation>,
    to: RouteLocation,
    entering: Vec<RouteTransitionTarget>,
    leaving: Vec<RouteTransitionTarget>,
    preserved: Vec<RouteFrameVersion>,
}

impl RouteTransitionContext {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        router: RouterHandle,
        request: NavigationRequestId,
        navigation: NavigationKind,
        from: Option<RouteLocation>,
        to: RouteLocation,
        entering: Vec<RouteTransitionTarget>,
        leaving: Vec<RouteTransitionTarget>,
        preserved: Vec<RouteFrameVersion>,
    ) -> Self {
        Self {
            id: RouteTransitionId::new(router),
            router,
            request,
            navigation,
            from,
            to,
            entering,
            leaving,
            preserved,
        }
    }

    pub fn id(&self) -> RouteTransitionId {
        self.id.clone()
    }
    pub fn router(&self) -> RouterHandle {
        self.router
    }
    pub fn request(&self) -> NavigationRequestId {
        self.request
    }
    pub fn navigation(&self) -> NavigationKind {
        self.navigation
    }
    pub fn from(&self) -> Option<&RouteLocation> {
        self.from.as_ref()
    }
    pub fn to(&self) -> &RouteLocation {
        &self.to
    }
    pub fn entering(&self) -> &[RouteTransitionTarget] {
        &self.entering
    }
    pub fn leaving(&self) -> &[RouteTransitionTarget] {
        &self.leaving
    }

    /// Unchanged frames are context only; their surfaces are not authorized.
    pub fn preserved(&self) -> &[RouteFrameVersion] {
        &self.preserved
    }
}

/// Application choice after synchronously preparing the initial presentation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransitionPreparation {
    /// Continue ordinary navigation, without keeping an application session.
    Skip,
    /// Start an application session, optionally delaying the leaving disposition.
    ///
    /// `hold_exit: false` supports enter-only effects, including initial entry.
    /// It does not delay the navigation request's ordinary completion.
    Run { hold_exit: bool },
}

/// Why the router is synchronously stopping an application session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransitionCancelReason {
    Skipped,
    PreparationAborted,
    Superseded,
    AuthorityLost,
}

/// Application callbacks for one router's presentation changes.
///
/// Callbacks run synchronously outside the atomic publication tail. They may use
/// ordinary application resources and authored content. Router targets are only
/// authorized as documented by [`RouteTransitionTarget::surface`].
///
/// Each started `prepare` is followed by `presented` or `cancelled`; a presented
/// session may subsequently be cancelled too. The application must not infer
/// activation from `prepare` or from request completion.
pub trait RouteTransitionDriver: Send + Sync + 'static {
    /// Set initial styles before the first visible presentation and register any
    /// application animation state. Do not start an elapsed-time animation yet.
    fn prepare(
        &mut self,
        world: &mut World,
        context: &RouteTransitionContext,
    ) -> TransitionPreparation;

    /// The authoritative navigation was published; application animation may run.
    fn presented(&mut self, world: &mut World, id: &RouteTransitionId);

    /// Stop this session's writers and remove its application animation state.
    ///
    /// This must be complete when the callback returns (queued Commands are
    /// flushed before the router continues). The router restores authorized shell
    /// values when their exact ownership is still valid. The application handles
    /// its own authored-content changes; arbitrary resources are not rolled back.
    fn cancelled(
        &mut self,
        world: &mut World,
        id: &RouteTransitionId,
        reason: TransitionCancelReason,
    );
}

/// Deferred application completion of one exact presentation session.
pub trait RouteTransitionCommandsExt {
    /// Release the leaving views after stopping their animation writers.
    /// Entering animation may continue, independently of request completion.
    fn release_route_transition_exit(&mut self, id: RouteTransitionId) -> &mut Self;

    /// Finish the entire session after stopping all of its writers.
    /// Also releases any outstanding exit. Duplicate or stale commands are inert.
    fn finish_route_transition(&mut self, id: RouteTransitionId) -> &mut Self;
}

impl RouteTransitionCommandsExt for Commands<'_, '_> {
    fn release_route_transition_exit(&mut self, id: RouteTransitionId) -> &mut Self {
        self.queue(move |world: &mut World| request_release(world, &id, false));
        self
    }

    fn finish_route_transition(&mut self, id: RouteTransitionId) -> &mut Self {
        self.queue(move |world: &mut World| request_release(world, &id, true));
        self
    }
}
