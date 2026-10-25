//! Read-only presence projection, separate from both animation and exit owners.
//!
//! A cancelled writer session may end before its original physical exit is
//! hidden. Keep this exact projection until hiding succeeds. It never grants
//! input, cleanup, or lifecycle authority.

use bevy_ecs::{resource::Resource, world::World};

use crate::{
    app_ext::RouterHandle,
    input::native_ui_candidates::NativeUiInputProjection,
    registry::{ExactRegistryClaim, RouteEntityRegistry},
};

use super::RouteTransitionId;

#[derive(Resource, Default)]
struct ExitPresence {
    routers: Vec<(
        RouterHandle,
        Option<(RouteTransitionId, Vec<ExactRegistryClaim>)>,
    )>,
}

pub(super) fn register_router_presence(world: &mut World, router: RouterHandle) {
    world.init_resource::<ExitPresence>();
    let mut presence = world.resource_mut::<ExitPresence>();
    if !presence.routers.iter().any(|(handle, _)| *handle == router) {
        presence.routers.push((router, None));
    }
}

/// Claims come from the frozen publication plan, not a post-publication scan.
/// Until that plan is applied they do not match the registry's current claims.
pub(crate) fn seal_transition_presence(
    world: &mut World,
    id: &RouteTransitionId,
    claims: Vec<ExactRegistryClaim>,
) {
    let mut presence = world.resource_mut::<ExitPresence>();
    let (_, slot) = presence
        .routers
        .iter_mut()
        .find(|(router, _)| *router == id.router())
        .expect("transition registration preallocates its presence slot");
    *slot = Some((id.clone(), claims));
    update_input_hint(world);
}

pub(crate) fn release_transition_presence(world: &mut World, id: &RouteTransitionId) {
    if let Some(mut presence) = world.get_resource_mut::<ExitPresence>()
        && let Some((_, slot)) = presence
            .routers
            .iter_mut()
            .find(|(router, _)| *router == id.router())
        && slot.as_ref().is_some_and(|(current, _)| current == id)
    {
        *slot = None;
    }
    update_input_hint(world);
}

/// Retires only visual projections whose exact original Entities no longer
/// exist. Session termination alone is insufficient, and this creates no
/// cleanup receipt or ownership disposition.
pub(crate) fn reap_absent_presence(world: &mut World) {
    if !world.contains_resource::<ExitPresence>() {
        return;
    }
    let changed = world.resource_scope(
        |world, mut presence: bevy_ecs::change_detection::Mut<ExitPresence>| {
            let mut changed = false;
            for (_, slot) in &mut presence.routers {
                if slot.as_ref().is_some_and(|(_, claims)| {
                    claims
                        .iter()
                        .all(|claim| world.get_entity(claim.entity()).is_err())
                }) {
                    *slot = None;
                    changed = true;
                }
            }
            changed
        },
    );
    if changed {
        update_input_hint(world);
    }
}

pub(crate) fn holds_visible_claim(world: &World, claim: &ExactRegistryClaim) -> bool {
    world
        .get_resource::<ExitPresence>()
        .is_some_and(|presence| {
            presence.routers.iter().any(|(_, slot)| {
                slot.as_ref()
                    .is_some_and(|(_, claims)| claims.contains(claim))
            })
        })
        && world
            .get_resource::<RouteEntityRegistry>()
            .and_then(|registry| registry.record(claim.entity()))
            .is_some_and(|record| record.matches_claim(claim))
}

fn update_input_hint(world: &mut World) {
    let held = world
        .get_resource::<ExitPresence>()
        .is_some_and(|presence| {
            presence
                .routers
                .iter()
                .any(|(_, slot)| slot.as_ref().is_some_and(|(_, claims)| !claims.is_empty()))
        });
    if let Some(mut projection) = world.get_resource_mut::<NativeUiInputProjection>() {
        projection.set_presence(held);
    }
}
