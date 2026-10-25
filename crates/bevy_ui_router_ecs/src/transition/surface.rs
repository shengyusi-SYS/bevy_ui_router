//! Frozen presentation surfaces and their exact restoration write set.

use bevy_ecs::{
    entity::Entity,
    hierarchy::{ChildOf, Children},
    world::World,
};
use bevy_ui::{Display, Node, UiTransform, ZIndex};
use bevy_ui_router_core::frame::RouteFrameVersion;

use crate::{
    components::RouteOwnershipSeal,
    presentation::{FrameLifecycle, FrameShell},
    registry::{
        EntityLifecycle, EntityOwner, ExactRegistryClaim, RegistryOwner, RouteEntityRegistry,
    },
};

use super::session::{
    RouteTransitionRuntime, SessionPhase, TransitionSession, TransitionSessionError,
};
use super::{RouteTransitionContext, RouteTransitionId, RouteTransitionTarget};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TransitionTargetRole {
    Entering,
    Leaving,
}

/// The current expectation is frozen before application preparation; the
/// publication plan supplies the independent published expectation later.
pub(crate) struct TransitionSurfaceBinding {
    target: RouteTransitionTarget,
    role: TransitionTargetRole,
    current: ExactRegistryClaim,
    published: Option<ExactRegistryClaim>,
}

impl TransitionSurfaceBinding {
    pub(crate) fn new(
        target: RouteTransitionTarget,
        role: TransitionTargetRole,
        current: ExactRegistryClaim,
    ) -> Self {
        Self {
            target,
            role,
            current,
            published: None,
        }
    }
}

#[derive(Clone)]
struct SurfaceBaseline {
    node: Node,
    transform: UiTransform,
    z_index: ZIndex,
}

pub(super) struct SessionSurface {
    binding: TransitionSurfaceBinding,
    baseline: SurfaceBaseline,
    topology: SurfaceTopology,
}

/// Only the edges bounding this module's write set are frozen. Content below
/// the root and siblings in the mount remain application/presentation concerns.
struct SurfaceTopology {
    parent: Entity,
    content: Entity,
}

impl SurfaceTopology {
    fn capture(world: &World, target: &RouteTransitionTarget) -> Option<Self> {
        let topology = Self {
            parent: world.get::<ChildOf>(target.surface)?.parent(),
            content: target.content,
        };
        topology.matches(world, target.surface).then_some(topology)
    }

    fn matches(&self, world: &World, shell: Entity) -> bool {
        world.get::<ChildOf>(shell).map(ChildOf::parent) == Some(self.parent)
            && world.get::<Children>(self.parent).is_some_and(|children| {
                children.iter().filter(|child| **child == shell).count() == 1
            })
            && world
                .get::<Children>(shell)
                .is_some_and(|children| children.len() == 1 && children[0] == self.content)
            && world.get::<ChildOf>(self.content).map(ChildOf::parent) == Some(shell)
    }
}

/// Value relaxation only. The caller must retain its complete structural,
/// visibility, input, and target/protected validation.
pub(crate) fn transition_surface_display(
    world: &World,
    frame: &RouteFrameVersion,
    shell: Entity,
    lifecycle: EntityLifecycle,
) -> Option<Display> {
    let runtime = world.get_resource::<RouteTransitionRuntime>()?;
    for session in runtime.sessions() {
        for surface in &session.surfaces {
            if surface.binding.target.surface != shell || !surface_is_authorized(session, surface) {
                continue;
            }
            let Some(claim) = expected_claim(session, surface) else {
                continue;
            };
            if claim.lifecycle() == lifecycle
                && claim_frame(claim) == Some(frame)
                && claim_is_current(world, claim, &surface.topology)
            {
                return Some(expected_display(session, surface));
            }
        }
    }
    None
}

fn claim_frame(claim: &ExactRegistryClaim) -> Option<&RouteFrameVersion> {
    match claim.owner() {
        RegistryOwner::Committed(EntityOwner::FrameShell(frame)) => Some(frame),
        _ => None,
    }
}

fn claim_is_current(world: &World, claim: &ExactRegistryClaim, topology: &SurfaceTopology) -> bool {
    let Some(frame) = claim_frame(claim) else {
        return false;
    };
    world
        .get_resource::<RouteEntityRegistry>()
        .and_then(|r| r.record(claim.entity()))
        .is_some_and(|record| record.matches_claim(claim))
        && world.get::<RouteOwnershipSeal>(claim.entity())
            == Some(&RouteOwnershipSeal::new(claim.seal()))
        && world
            .get::<FrameShell>(claim.entity())
            .is_some_and(|shell| shell.matches_frame(frame))
        && world.get::<FrameLifecycle>(claim.entity()) == Some(&FrameLifecycle(claim.lifecycle()))
        && topology.matches(world, claim.entity())
}

pub(super) fn capture_surfaces(
    world: &World,
    context: &RouteTransitionContext,
    bindings: Vec<TransitionSurfaceBinding>,
) -> Result<Vec<SessionSurface>, TransitionSessionError> {
    if bindings.len() != context.entering.len() + context.leaving.len() {
        return Err(TransitionSessionError::InvalidProjection);
    }
    let mut surfaces: Vec<SessionSurface> = Vec::with_capacity(bindings.len());
    for binding in bindings {
        let entity = binding.target.surface;
        let targets = match binding.role {
            TransitionTargetRole::Entering => &context.entering,
            TransitionTargetRole::Leaving => &context.leaving,
        };
        let topology = SurfaceTopology::capture(world, &binding.target)
            .ok_or(TransitionSessionError::InvalidTarget(entity))?;
        if binding.current.entity() != entity
            || claim_frame(&binding.current) != Some(&binding.target.frame)
            || context.preserved.contains(&binding.target.frame)
            || targets
                .iter()
                .filter(|target| **target == binding.target)
                .count()
                != 1
            || !claim_is_current(world, &binding.current, &topology)
            || surfaces
                .iter()
                .any(|surface| surface.binding.target.surface == entity)
        {
            return Err(TransitionSessionError::InvalidTarget(entity));
        }
        let baseline = SurfaceBaseline {
            node: world
                .get::<Node>(entity)
                .cloned()
                .ok_or(TransitionSessionError::InvalidTarget(entity))?,
            transform: *world
                .get::<UiTransform>(entity)
                .ok_or(TransitionSessionError::InvalidTarget(entity))?,
            z_index: *world
                .get::<ZIndex>(entity)
                .ok_or(TransitionSessionError::InvalidTarget(entity))?,
        };
        surfaces.push(SessionSurface {
            binding,
            baseline,
            topology,
        });
    }
    for target in context.entering.iter().chain(&context.leaving) {
        if !surfaces
            .iter()
            .any(|surface| surface.binding.target == *target)
        {
            return Err(TransitionSessionError::InvalidTarget(target.surface));
        }
    }
    Ok(surfaces)
}

/// Freeze only claims projected by the caller's still-unapplied publication
/// plan. A released leaving target no longer needs a future presentation claim.
pub(super) fn seal_surfaces(
    session: &mut TransitionSession,
    resulting_claims: &[ExactRegistryClaim],
) -> Result<(), TransitionSessionError> {
    let mut published = Vec::with_capacity(session.surfaces.len());
    for surface in &session.surfaces {
        if !surface_is_authorized(session, surface) {
            published.push(None);
            continue;
        }
        let binding = &surface.binding;
        let entity = binding.target.surface;
        let mut matches = resulting_claims
            .iter()
            .filter(|claim| claim.entity() == entity);
        let claim = matches
            .next()
            .ok_or(TransitionSessionError::InvalidPublication(entity))?;
        let lifecycle_is_valid = match binding.role {
            TransitionTargetRole::Entering => claim.lifecycle() == EntityLifecycle::Active,
            TransitionTargetRole::Leaving => matches!(
                claim.lifecycle(),
                EntityLifecycle::Leaving | EntityLifecycle::Retained
            ),
        };
        if matches.next().is_some()
            || claim.seal() != binding.current.seal()
            || claim_frame(claim) != Some(&binding.target.frame)
            || !lifecycle_is_valid
        {
            return Err(TransitionSessionError::InvalidPublication(entity));
        }
        published.push(Some(claim.clone()));
    }
    // No partial seal is installed if any projected target was invalid.
    for (surface, claim) in session.surfaces.iter_mut().zip(published) {
        surface.binding.published = claim;
    }
    Ok(())
}

fn expected_claim<'a>(
    session: &TransitionSession,
    surface: &'a SessionSurface,
) -> Option<&'a ExactRegistryClaim> {
    match session.phase {
        SessionPhase::Prepared | SessionPhase::Sealed => Some(&surface.binding.current),
        SessionPhase::Presented => surface.binding.published.as_ref(),
    }
}

fn surface_is_authorized(session: &TransitionSession, surface: &SessionSurface) -> bool {
    surface.binding.role == TransitionTargetRole::Entering
        || (!session.exit_released
            && (session.phase != SessionPhase::Presented || session.hold_exit))
}

fn expected_display(session: &TransitionSession, surface: &SessionSurface) -> Display {
    match session.phase {
        SessionPhase::Prepared | SessionPhase::Sealed => surface.baseline.node.display,
        SessionPhase::Presented => Display::Flex,
    }
}

fn surface_is_current(
    world: &World,
    session: &TransitionSession,
    surface: &SessionSurface,
) -> bool {
    expected_claim(session, surface)
        .is_some_and(|claim| claim_is_current(world, claim, &surface.topology))
        && world
            .get::<Node>(surface.binding.target.surface)
            .is_some_and(|node| node.display == expected_display(session, surface))
        && world
            .get::<UiTransform>(surface.binding.target.surface)
            .is_some()
        && world
            .get::<ZIndex>(surface.binding.target.surface)
            .is_some()
}

pub(super) fn session_drift(
    world: &World,
    id: &RouteTransitionId,
) -> Result<Vec<Entity>, TransitionSessionError> {
    let session = world
        .get_resource::<RouteTransitionRuntime>()
        .and_then(|r| r.session(id))
        .ok_or(TransitionSessionError::MissingSession)?;
    Ok(session
        .surfaces
        .iter()
        .filter(|surface| {
            surface_is_authorized(session, surface) && !surface_is_current(world, session, surface)
        })
        .map(|surface| surface.binding.target.surface)
        .collect())
}

pub(super) fn restore_surfaces(
    world: &mut World,
    id: &RouteTransitionId,
    leaving_only: bool,
) -> Result<(), TransitionSessionError> {
    let session = world
        .get_resource::<RouteTransitionRuntime>()
        .and_then(|r| r.session(id))
        .ok_or(TransitionSessionError::MissingSession)?;
    let selected: Vec<_> = session
        .surfaces
        .iter()
        .filter(|surface| {
            surface_is_authorized(session, surface)
                && (!leaving_only || surface.binding.role == TransitionTargetRole::Leaving)
        })
        .collect();
    let drift: Vec<_> = selected
        .iter()
        .filter(|surface| !surface_is_current(world, session, surface))
        .map(|surface| surface.binding.target.surface)
        .collect();
    let values: Vec<_> = selected
        .iter()
        .filter(|surface| surface_is_current(world, session, surface))
        .map(|surface| (surface.binding.target.surface, surface.baseline.clone()))
        .collect();
    for (entity, baseline) in values {
        // Existing-component value writes do not run structural hooks. In
        // particular, the PreparedInert baseline must not hide a published page.
        {
            let mut node = world.get_mut::<Node>(entity).unwrap();
            let display = node.display;
            *node = baseline.node;
            node.display = display;
        }
        *world.get_mut::<UiTransform>(entity).unwrap() = baseline.transform;
        *world.get_mut::<ZIndex>(entity).unwrap() = baseline.z_index;
    }
    if drift.is_empty() {
        Ok(())
    } else {
        Err(TransitionSessionError::SurfaceDrift(drift))
    }
}
