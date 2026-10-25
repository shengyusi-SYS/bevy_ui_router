//! Callback and session bookkeeping; lifecycle owners remain in navigation.

use bevy_ecs::{entity::Entity, resource::Resource, world::World};

use crate::{app_ext::RouterHandle, registry::ExactRegistryClaim};

use super::surface::{
    SessionSurface, TransitionSurfaceBinding, capture_surfaces, restore_surfaces, seal_surfaces,
    session_drift,
};
use super::{
    RouteTransitionContext, RouteTransitionDriver, RouteTransitionId, TransitionCancelReason,
    TransitionPreparation,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum SessionPhase {
    Prepared,
    Sealed,
    Presented,
}

pub(super) struct TransitionSession {
    pub(super) context: RouteTransitionContext,
    pub(super) surfaces: Vec<SessionSurface>,
    pub(super) phase: SessionPhase,
    pub(super) hold_exit: bool,
    pub(super) exit_released: bool,
    release_requested: bool,
    finish_requested: bool,
}

struct RouterTransitions {
    router: RouterHandle,
    driver: Option<Box<dyn RouteTransitionDriver>>,
    session: Option<TransitionSession>,
}

#[derive(Resource, Default)]
pub(crate) struct RouteTransitionRuntime {
    routers: Vec<RouterTransitions>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum TransitionSessionError {
    AlreadyRegistered,
    SessionBusy,
    MissingSession,
    CallbackInProgress,
    WrongPhase,
    InvalidProjection,
    InvalidTarget(Entity),
    InvalidPublication(Entity),
    SurfaceDrift(Vec<Entity>),
}

#[derive(Clone, Debug)]
pub(crate) struct TransitionRequests {
    pub(crate) id: RouteTransitionId,
    pub(crate) release_exit: bool,
    pub(crate) finish: bool,
}

impl RouteTransitionRuntime {
    pub(super) fn sessions(&self) -> impl Iterator<Item = &TransitionSession> {
        self.routers
            .iter()
            .filter_map(|entry| entry.session.as_ref())
    }

    fn entry(&self, router: RouterHandle) -> Option<&RouterTransitions> {
        self.routers.iter().find(|entry| entry.router == router)
    }

    fn entry_mut(&mut self, router: RouterHandle) -> Option<&mut RouterTransitions> {
        self.routers.iter_mut().find(|entry| entry.router == router)
    }

    pub(super) fn session(&self, id: &RouteTransitionId) -> Option<&TransitionSession> {
        self.entry(id.router())?
            .session
            .as_ref()
            .filter(|session| session.context.id == *id)
    }

    fn session_mut(&mut self, id: &RouteTransitionId) -> Option<&mut TransitionSession> {
        self.entry_mut(id.router())?
            .session
            .as_mut()
            .filter(|session| session.context.id == *id)
    }
}

pub(crate) fn register_transition_driver(
    world: &mut World,
    router: RouterHandle,
    driver: Box<dyn RouteTransitionDriver>,
) -> Result<(), TransitionSessionError> {
    world.init_resource::<RouteTransitionRuntime>();
    let mut runtime = world.resource_mut::<RouteTransitionRuntime>();
    if runtime.entry(router).is_some() {
        return Err(TransitionSessionError::AlreadyRegistered);
    }
    runtime.routers.push(RouterTransitions {
        router,
        driver: Some(driver),
        session: None,
    });
    super::presence::register_router_presence(world, router);
    Ok(())
}

pub(crate) fn has_transition_driver(world: &World, router: RouterHandle) -> bool {
    world
        .get_resource::<RouteTransitionRuntime>()
        .is_some_and(|runtime| runtime.entry(router).is_some())
}

pub(crate) fn current_transition(world: &World, router: RouterHandle) -> Option<RouteTransitionId> {
    current_context(world, router).map(RouteTransitionContext::id)
}

pub(crate) fn current_context(
    world: &World,
    router: RouterHandle,
) -> Option<&RouteTransitionContext> {
    world
        .get_resource::<RouteTransitionRuntime>()?
        .entry(router)?
        .session
        .as_ref()
        .map(|s| &s.context)
}

pub(crate) fn transition_is_prepared(world: &World, id: &RouteTransitionId) -> bool {
    world
        .get_resource::<RouteTransitionRuntime>()
        .and_then(|runtime| runtime.session(id))
        .is_some_and(|session| session.phase != SessionPhase::Presented)
}

/// Install all validation facts before invoking application preparation.
pub(crate) fn prepare_transition(
    world: &mut World,
    context: RouteTransitionContext,
    bindings: Vec<TransitionSurfaceBinding>,
) -> Result<TransitionPreparation, TransitionSessionError> {
    if context.entering.is_empty() && context.leaving.is_empty() {
        return Ok(TransitionPreparation::Skip);
    }
    let Some(entry) = world
        .get_resource::<RouteTransitionRuntime>()
        .and_then(|r| r.entry(context.router))
    else {
        return Ok(TransitionPreparation::Skip);
    };
    if entry.session.is_some() {
        return Err(TransitionSessionError::SessionBusy);
    }
    if entry.driver.is_none() {
        return Err(TransitionSessionError::CallbackInProgress);
    }
    let surfaces = capture_surfaces(world, &context, bindings)?;
    let id = context.id();
    let router = context.router;
    let callback_context = context.clone();
    world
        .resource_mut::<RouteTransitionRuntime>()
        .entry_mut(router)
        .unwrap()
        .session = Some(TransitionSession {
        context,
        surfaces,
        phase: SessionPhase::Prepared,
        hold_exit: false,
        exit_released: false,
        release_requested: false,
        finish_requested: false,
    });
    let mut driver = take_driver(world, &id)?;
    let mut choice = driver.prepare(world, &callback_context);
    world.flush();
    return_driver(world, id.router(), driver);
    if let TransitionPreparation::Run { hold_exit } = choice {
        let mut runtime = world.resource_mut::<RouteTransitionRuntime>();
        let session = runtime
            .session_mut(&id)
            .ok_or(TransitionSessionError::MissingSession)?;
        session.hold_exit = hold_exit && !session.context.leaving.is_empty();
        choice = TransitionPreparation::Run {
            hold_exit: session.hold_exit,
        };
    }
    let drift = session_drift(world, &id)?;
    if !drift.is_empty() {
        // The callback still receives cancellation even if its writes invalidated
        // the surface. Restoration never writes through an invalid identity.
        let _ = cancel_transition(
            world,
            id.router(),
            TransitionCancelReason::PreparationAborted,
        );
        return Err(TransitionSessionError::SurfaceDrift(drift));
    }
    if choice == TransitionPreparation::Skip {
        cancel_transition(world, id.router(), TransitionCancelReason::Skipped)?;
    } else if !transition_holds_exit(world, &id) {
        // A driver may have staged leaving styles before deciding it only needs
        // an entering animation. The immediate cleanup path stays canonical.
        if let Err(error) = restore_surfaces(world, &id, true) {
            let _ = cancel_transition(
                world,
                id.router(),
                TransitionCancelReason::PreparationAborted,
            );
            return Err(error);
        }
        world
            .resource_mut::<RouteTransitionRuntime>()
            .session_mut(&id)
            .unwrap()
            .exit_released = true;
    }
    Ok(choice)
}

/// Freeze publication expectations before the atomic publication tail. The
/// caller supplies the complete resulting claims from its registry plan.
pub(crate) fn seal_transition_publication(
    world: &mut World,
    id: &RouteTransitionId,
    resulting_claims: &[ExactRegistryClaim],
) -> Result<(), TransitionSessionError> {
    let session = world
        .get_resource::<RouteTransitionRuntime>()
        .and_then(|runtime| runtime.session(id))
        .ok_or(TransitionSessionError::MissingSession)?;
    if session.phase != SessionPhase::Prepared {
        return Err(TransitionSessionError::WrongPhase);
    }
    let drift = session_drift(world, id)?;
    if !drift.is_empty() {
        return Err(TransitionSessionError::SurfaceDrift(drift));
    }
    let mut runtime = world.resource_mut::<RouteTransitionRuntime>();
    let session = runtime.session_mut(id).unwrap();
    seal_surfaces(session, resulting_claims)?;
    session.phase = SessionPhase::Sealed;
    Ok(())
}

/// Invoke only after the production publication tail has installed its owners.
/// The frozen published expectations, not a fresh observation, become current.
pub(crate) fn present_transition(
    world: &mut World,
    id: &RouteTransitionId,
) -> Result<(), TransitionSessionError> {
    {
        let mut runtime = world
            .get_resource_mut::<RouteTransitionRuntime>()
            .ok_or(TransitionSessionError::MissingSession)?;
        let session = runtime
            .session_mut(id)
            .ok_or(TransitionSessionError::MissingSession)?;
        if session.phase != SessionPhase::Sealed {
            return Err(TransitionSessionError::WrongPhase);
        }
        session.phase = SessionPhase::Presented;
    }
    let drift = session_drift(world, id)?;
    if !drift.is_empty() {
        let _ = cancel_transition(world, id.router(), TransitionCancelReason::AuthorityLost);
        return Err(TransitionSessionError::SurfaceDrift(drift));
    }
    let mut driver = take_driver(world, id)?;
    driver.presented(world, id);
    world.flush();
    return_driver(world, id.router(), driver);
    let drift = session_drift(world, id)?;
    if !drift.is_empty() {
        let _ = cancel_transition(world, id.router(), TransitionCancelReason::AuthorityLost);
        return Err(TransitionSessionError::SurfaceDrift(drift));
    }
    Ok(())
}

pub(crate) fn cancel_transition(
    world: &mut World,
    router: RouterHandle,
    reason: TransitionCancelReason,
) -> Result<(), TransitionSessionError> {
    let Some(id) = current_transition(world, router) else {
        return Ok(());
    };
    let mut driver = take_driver(world, &id)?;
    driver.cancelled(world, &id, reason);
    world.flush();
    return_driver(world, router, driver);
    let restored = restore_surfaces(world, &id, false);
    // A stopped session must not remain authorized or receive cancellation twice
    // merely because authority loss prevents restoring one of its old targets.
    remove_session(world, &id);
    restored
}

pub(crate) fn request_release(world: &mut World, id: &RouteTransitionId, finish: bool) {
    let Some(mut runtime) = world.get_resource_mut::<RouteTransitionRuntime>() else {
        return;
    };
    let Some(session) = runtime.session_mut(id) else {
        return;
    };
    session.release_requested = true;
    session.finish_requested |= finish;
}

/// Read without consuming: lifecycle settlement owns acknowledgement/retry.
pub(crate) fn pending_transition_requests(
    world: &World,
    router: RouterHandle,
) -> Option<TransitionRequests> {
    let session = world
        .get_resource::<RouteTransitionRuntime>()?
        .entry(router)?
        .session
        .as_ref()?;
    if !session.release_requested && !session.finish_requested {
        return None;
    }
    Some(TransitionRequests {
        id: session.context.id(),
        release_exit: session.release_requested,
        finish: session.finish_requested,
    })
}

pub(crate) fn transition_holds_exit(world: &World, id: &RouteTransitionId) -> bool {
    world
        .get_resource::<RouteTransitionRuntime>()
        .and_then(|r| r.session(id))
        .is_some_and(|s| s.hold_exit && !s.exit_released)
}

/// Restore leaving surfaces before navigation revokes their visible presence.
/// This never consumes or manufactures a lifecycle owner or cleanup token.
pub(crate) fn release_transition_exit(
    world: &mut World,
    id: &RouteTransitionId,
) -> Result<(), TransitionSessionError> {
    require_presented(world, id)?;
    restore_surfaces(world, id, true)?;
    let mut runtime = world.resource_mut::<RouteTransitionRuntime>();
    let session = runtime
        .session_mut(id)
        .ok_or(TransitionSessionError::MissingSession)?;
    session.exit_released = true;
    session.release_requested = false;
    Ok(())
}

/// Application has stopped every writer. Restore surfaces before next reuse.
pub(crate) fn finish_transition(
    world: &mut World,
    id: &RouteTransitionId,
) -> Result<(), TransitionSessionError> {
    require_presented(world, id)?;
    restore_surfaces(world, id, false)?;
    remove_session(world, id);
    Ok(())
}

fn require_presented(world: &World, id: &RouteTransitionId) -> Result<(), TransitionSessionError> {
    let session = world
        .get_resource::<RouteTransitionRuntime>()
        .and_then(|runtime| runtime.session(id))
        .ok_or(TransitionSessionError::MissingSession)?;
    if session.phase != SessionPhase::Presented {
        return Err(TransitionSessionError::WrongPhase);
    }
    Ok(())
}

fn take_driver(
    world: &mut World,
    id: &RouteTransitionId,
) -> Result<Box<dyn RouteTransitionDriver>, TransitionSessionError> {
    let mut runtime = world
        .get_resource_mut::<RouteTransitionRuntime>()
        .ok_or(TransitionSessionError::MissingSession)?;
    if runtime.session(id).is_none() {
        return Err(TransitionSessionError::MissingSession);
    }
    runtime
        .entry_mut(id.router())
        .unwrap()
        .driver
        .take()
        .ok_or(TransitionSessionError::CallbackInProgress)
}

fn return_driver(world: &mut World, router: RouterHandle, driver: Box<dyn RouteTransitionDriver>) {
    // The resource stays installed throughout callbacks, including its session
    // facts. Router-private bookkeeping is not an application mutation surface.
    world
        .resource_mut::<RouteTransitionRuntime>()
        .entry_mut(router)
        .expect("transition router remains registered during callbacks")
        .driver = Some(driver);
}

fn remove_session(world: &mut World, id: &RouteTransitionId) {
    let mut runtime = world.resource_mut::<RouteTransitionRuntime>();
    if runtime.session(id).is_some() {
        runtime.entry_mut(id.router()).unwrap().session = None;
    }
}
