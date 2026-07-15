use std::collections::HashSet;

use bevy_camera::visibility::Visibility;
use bevy_ecs::{change_detection::Mut, entity::Entity, prelude::World, resource::Resource};
use bevy_input_focus::InputFocus;
use bevy_picking::Pickable;

use crate::{
    components::RouterInputBarrier,
    presentation::validate_active_frame,
    registry::{EntityLifecycle, ExactRegistryClaim, QuarantineLeaseTable, RouteEntityRegistry},
};

#[derive(Default, Resource, Debug)]
pub(crate) struct RouteFocusState {
    focused: Option<Entity>,
}

impl RouteFocusState {
    pub(crate) const fn focused(&self) -> Option<Entity> {
        self.focused
    }

    pub(crate) fn set_focused(&mut self, focused: Option<Entity>) {
        self.focused = focused;
    }
}

#[derive(Resource)]
pub struct RouterInputGate {
    allowed: Vec<Entity>,
}

impl RouterInputGate {
    pub(crate) const fn new() -> Self {
        Self {
            allowed: Vec::new(),
        }
    }

    pub fn allows_pointer(&self, target: Entity) -> bool {
        self.allowed.contains(&target)
    }

    pub fn allows_keyboard(&self, target: Entity) -> bool {
        self.allowed.contains(&target)
    }

    pub fn allows_gamepad(&self, target: Entity) -> bool {
        self.allowed.contains(&target)
    }
}

pub(crate) struct PreparedInputGateMutation<'w> {
    gate: Mut<'w, RouterInputGate>,
    next: Vec<Entity>,
}

impl PreparedInputGateMutation<'_> {
    pub(crate) fn apply(mut self) {
        self.gate.allowed = self.next;
    }
}

pub(crate) struct PreparedFocusMutation<'w> {
    input: Mut<'w, InputFocus>,
    route: Mut<'w, RouteFocusState>,
    next_input: Option<InputFocus>,
    clear_route: bool,
}

impl PreparedFocusMutation<'_> {
    pub(crate) fn apply(mut self) {
        if let Some(next_input) = self.next_input {
            *self.input = next_input;
        }
        if self.clear_route {
            self.route.set_focused(None);
        }
    }
}

pub(crate) fn prepare_focus_mutation<'w>(
    input: Mut<'w, InputFocus>,
    route: Mut<'w, RouteFocusState>,
    leaving: &[Entity],
) -> PreparedFocusMutation<'w> {
    let next_input = input
        .get()
        .is_some_and(|entity| leaving.contains(&entity))
        .then(|| {
            let mut next = input.clone();
            next.clear();
            next
        });
    let clear_route = route
        .focused()
        .is_some_and(|entity| leaving.contains(&entity));
    PreparedFocusMutation {
        input,
        route,
        next_input,
        clear_route,
    }
}

pub(crate) fn prepare_input_gate_mutation<'w>(
    gate: Mut<'w, RouterInputGate>,
    remove: &[Entity],
    add: Vec<Entity>,
) -> Option<PreparedInputGateMutation<'w>> {
    if remove.iter().any(|entity| !gate.allowed.contains(entity))
        || remove
            .iter()
            .enumerate()
            .any(|(index, entity)| remove[index + 1..].contains(entity))
        || add
            .iter()
            .enumerate()
            .any(|(index, entity)| add[index + 1..].contains(entity))
        || add
            .iter()
            .any(|entity| gate.allowed.contains(entity) && !remove.contains(entity))
    {
        return None;
    }
    let mut next = gate.allowed.clone();
    next.retain(|entity| !remove.contains(entity));
    next.extend(add);
    Some(PreparedInputGateMutation { gate, next })
}

pub(crate) fn refresh_input_gate_system(world: &mut World) {
    let invalid_active = fail_closed_invalid_active_frames(world);
    clear_inert_subtree_focus(world, &invalid_active);
    let allowed = {
        let navigation = world.resource::<crate::navigation::RouterNavigation>();
        navigation
            .entries
            .iter()
            .filter_map(|entry| entry.active.as_ref())
            .filter(|frame| validate_active_frame(world, frame))
            .flat_map(|frame| frame.interactive_entities())
            .collect()
    };
    world.resource_mut::<RouterInputGate>().allowed = allowed;
}

struct PreparedPhysicalIsolation<'w> {
    entities: Vec<(
        Option<Mut<'w, Visibility>>,
        Option<Mut<'w, Pickable>>,
        Option<Mut<'w, RouterInputBarrier>>,
    )>,
}

impl PreparedPhysicalIsolation<'_> {
    fn apply(self) {
        for (visibility, pickable, barrier) in self.entities {
            if let Some(mut visibility) = visibility {
                *visibility = Visibility::Hidden;
            }
            if let Some(mut pickable) = pickable {
                *pickable = Pickable::IGNORE;
            }
            if let Some(mut barrier) = barrier {
                *barrier = RouterInputBarrier::deny_all();
            }
        }
    }
}

fn fail_closed_invalid_active_frames(world: &mut World) -> HashSet<Entity> {
    let frames = {
        let navigation = world.resource::<crate::navigation::RouterNavigation>();
        navigation
            .entries
            .iter()
            .filter_map(|entry| entry.active.as_ref())
            .filter(|frame| !validate_active_frame(world, frame))
            .map(|frame| frame.claims().cloned().collect::<Vec<_>>())
            .collect::<Vec<_>>()
    };
    let exact_frames = {
        let registry = world.resource::<RouteEntityRegistry>();
        frames
            .into_iter()
            .filter_map(|claims| {
                let current = claims
                    .into_iter()
                    .filter(|claim| {
                        registry
                            .record(claim.entity())
                            .is_some_and(|record| record.claim() == *claim)
                    })
                    .collect::<Vec<_>>();
                (!current.is_empty()).then_some(current)
            })
            .collect::<Vec<_>>()
    };
    let entities = exact_frames
        .iter()
        .flatten()
        .map(ExactRegistryClaim::entity)
        .collect::<HashSet<_>>();
    prepare_physical_isolation(world, &entities).apply();
    entities
}

fn prepare_physical_isolation<'w>(
    world: &'w mut World,
    targets: &HashSet<Entity>,
) -> PreparedPhysicalIsolation<'w> {
    let mut query = world.query::<(
        Entity,
        Option<&mut Visibility>,
        Option<&mut Pickable>,
        Option<&mut RouterInputBarrier>,
    )>();
    let mut entities = Vec::with_capacity(targets.len());
    for (entity, visibility, pickable, barrier) in query.iter_mut(world) {
        if !targets.contains(&entity) {
            continue;
        }
        entities.push((visibility, pickable, barrier));
    }
    PreparedPhysicalIsolation { entities }
}

fn clear_inert_subtree_focus(world: &mut World, invalid_active: &HashSet<Entity>) {
    let mut inert = world
        .resource::<RouteEntityRegistry>()
        .records()
        .iter()
        .filter(|record| record.lifecycle() != EntityLifecycle::Active)
        .map(|record| record.entity())
        .collect::<HashSet<_>>();
    inert.extend(
        world
            .resource::<QuarantineLeaseTable>()
            .inert_owned_entities(),
    );
    inert.extend(invalid_active.iter().copied());
    let input = world.get_resource::<InputFocus>().and_then(InputFocus::get);
    if input.is_some_and(|focused| inert.contains(&focused)) {
        world.resource_mut::<InputFocus>().clear();
    }
    let route = world
        .get_resource::<RouteFocusState>()
        .and_then(RouteFocusState::focused);
    if route.is_some_and(|focused| inert.contains(&focused)) {
        world.resource_mut::<RouteFocusState>().set_focused(None);
    }
}

pub(crate) fn input_ready_system(_world: &mut World) {}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_input_focus::InputFocus;

    #[test]
    fn gate_delta_preserves_other_router_permissions() {
        let mut world = World::new();
        let first = world.spawn_empty().id();
        let second = world.spawn_empty().id();
        let replacement = world.spawn_empty().id();
        world.insert_resource(RouterInputGate {
            allowed: vec![first],
        });

        world.resource_scope(|_world, gate: Mut<RouterInputGate>| {
            prepare_input_gate_mutation(gate, &[], vec![second])
                .expect("another router's permission is not the current router baseline")
                .apply();
        });
        assert_eq!(
            world.resource::<RouterInputGate>().allowed,
            vec![first, second]
        );

        world.resource_scope(|_world, gate: Mut<RouterInputGate>| {
            prepare_input_gate_mutation(gate, &[first], vec![replacement])
                .expect("the current router may replace only its own permission")
                .apply();
        });
        assert_eq!(
            world.resource::<RouterInputGate>().allowed,
            vec![second, replacement]
        );
    }

    #[test]
    fn focus_mutation_clears_both_focus_sources_for_a_leaving_target() {
        let mut world = World::new();
        let leaving = world.spawn_empty().id();
        world.insert_resource(InputFocus::from_entity(leaving));
        let mut route_focus = RouteFocusState::default();
        route_focus.set_focused(Some(leaving));
        world.insert_resource(route_focus);

        world.resource_scope(|world, input: Mut<InputFocus>| {
            world.resource_scope(|_world, route: Mut<RouteFocusState>| {
                prepare_focus_mutation(input, route, &[leaving]).apply();
            });
        });

        assert_eq!(world.resource::<InputFocus>().get(), None);
        assert_eq!(world.resource::<RouteFocusState>().focused(), None);
    }
}
