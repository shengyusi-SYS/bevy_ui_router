use std::collections::HashSet;

use bevy_camera::visibility::Visibility;
use bevy_ecs::{
    change_detection::Mut,
    entity::Entity,
    hierarchy::{ChildOf, Children},
    prelude::World,
    resource::Resource,
};
use bevy_input_focus::InputFocus;
use bevy_picking::Pickable;

use crate::{
    components::{ChildSlotAuthority, ChildSlotLifecycle, RouteOwnershipSeal, RouterInputBarrier},
    frame_local::validate_active_branch_with_entering,
    presentation::FrameLifecycle,
    registry::{
        EntityLifecycle, EntityOwner, ExactRegistryClaim, ProvisionalOwner, QuarantineLeaseTable,
        RegistryOwner, RouteEntityRegistry,
    },
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
    pub(crate) fn new() -> Self {
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

    pub(crate) fn allowed_projection(&self) -> &[Entity] {
        &self.allowed
    }

    fn replace_allowed(&mut self, next: Vec<Entity>) {
        self.allowed = next;
    }

    fn retain_allowed(&mut self, allowed: &HashSet<Entity>) {
        self.allowed.retain(|entity| allowed.contains(entity));
    }
}

#[derive(Clone)]
pub(crate) struct PreparedInputGateIsolation {
    next: Vec<Entity>,
}

impl PreparedInputGateIsolation {
    pub(crate) fn apply(self, gate: &mut RouterInputGate) {
        gate.replace_allowed(self.next);
    }
}

pub(crate) fn try_prepare_input_gate_isolation(
    gate: &RouterInputGate,
    targets: &[Entity],
) -> Result<PreparedInputGateIsolation, ()> {
    let mut next = Vec::new();
    next.try_reserve_exact(gate.allowed.len()).map_err(|_| ())?;
    next.extend(gate.allowed.iter().copied());
    next.retain(|entity| !targets.contains(entity));
    Ok(PreparedInputGateIsolation { next })
}

pub(crate) struct PreparedInputGateMutation<'w> {
    gate: Mut<'w, RouterInputGate>,
    next: Vec<Entity>,
}

impl PreparedInputGateMutation<'_> {
    pub(crate) fn apply(mut self) {
        self.gate.replace_allowed(self.next);
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
    let (allowed, invalid_claims) = {
        let navigation = world.resource::<crate::navigation::RouterNavigation>();
        let mut allowed = HashSet::new();
        let mut invalid_claims = Vec::new();
        for branch in navigation
            .entries
            .iter()
            .filter_map(|entry| entry.active.as_ref().map(|active| (entry, active)))
        {
            let (entry, active) = branch;
            if validate_active_branch_with_entering(
                world,
                active,
                entry.current_reconciliation_entering_branch(),
                entry.current_mount_new_staging_allowance(),
            )
            .is_ok()
            {
                allowed.extend(active.interactive_entities());
            } else {
                invalid_claims.push(active.claims().cloned().collect::<Vec<_>>());
            }
        }
        (allowed, invalid_claims)
    };
    let invalid_active = fail_closed_invalid_active_frames(world, invalid_claims);
    clear_inert_subtree_focus(world, &invalid_active);
    world
        .resource_mut::<RouterInputGate>()
        .retain_allowed(&allowed);
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

fn fail_closed_invalid_active_frames(
    world: &mut World,
    invalid_claims: Vec<Vec<ExactRegistryClaim>>,
) -> HashSet<Entity> {
    let exact_branches = {
        let registry = world.resource::<RouteEntityRegistry>();
        invalid_claims
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
    let entities = exact_branches
        .iter()
        .flatten()
        .map(ExactRegistryClaim::entity)
        .collect::<HashSet<_>>();
    let focus_closure = observed_descendant_closure(world, entities.iter().copied());
    prepare_physical_isolation(world, &entities).apply();
    focus_closure
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
    let inert_roots = world
        .resource::<QuarantineLeaseTable>()
        .inert_owned_entities()
        .collect::<Vec<_>>();
    let stale_roots = inert_roots
        .iter()
        .copied()
        .filter(|entity| !inert_table_root_is_exact(world, *entity))
        .collect::<HashSet<_>>();
    let mut stack = inert_roots
        .into_iter()
        .filter(|entity| !stale_roots.contains(entity))
        .collect::<Vec<_>>();
    let mut blocked = HashSet::new();
    while let Some(entity) = stack.pop() {
        if stale_roots.contains(&entity) || !blocked.insert(entity) {
            continue;
        }
        if let Some(children) = world.get::<Children>(entity) {
            stack.extend(children.iter().copied());
        }
    }
    blocked.extend(invalid_active.iter().copied());
    let input = world.get_resource::<InputFocus>().and_then(InputFocus::get);
    let clear_input = input.is_some_and(|focused| {
        blocked.contains(&focused) || nearest_registry_boundary_is_inert(world, focused)
    });
    if clear_input {
        world.resource_mut::<InputFocus>().clear();
    }
    let route = world
        .get_resource::<RouteFocusState>()
        .and_then(RouteFocusState::focused);
    let clear_route = route.is_some_and(|focused| {
        blocked.contains(&focused) || nearest_registry_boundary_is_inert(world, focused)
    });
    if clear_route {
        world.resource_mut::<RouteFocusState>().set_focused(None);
    }
}

fn nearest_registry_boundary_is_inert(world: &World, focused: Entity) -> bool {
    let registry = world.resource::<RouteEntityRegistry>();
    let table = world.resource::<QuarantineLeaseTable>();
    let mut current = focused;
    let mut visited = HashSet::new();
    loop {
        if !visited.insert(current) {
            return true;
        }
        if table.record(current).is_some() || table.unresolved_record(current).is_some() {
            return inert_table_root_is_exact(world, current);
        }
        if let Some(record) = registry.record(current) {
            return record.lifecycle() != EntityLifecycle::Active;
        }
        let Some(parent) = world.get::<ChildOf>(current).map(ChildOf::parent) else {
            return false;
        };
        current = parent;
    }
}

fn inert_table_root_is_exact(world: &World, entity: Entity) -> bool {
    let table = world.resource::<QuarantineLeaseTable>();
    let Some(record) = table
        .record(entity)
        .or_else(|| table.unresolved_record(entity))
    else {
        return false;
    };
    let claim = record.claim();
    let lifecycle_is_exact = match claim.owner() {
        RegistryOwner::Committed(
            EntityOwner::FrameShell(_)
            | EntityOwner::FrameContentTarget(_)
            | EntityOwner::FrameOutput { .. },
        ) => world.get::<FrameLifecycle>(entity) == Some(&FrameLifecycle(claim.lifecycle())),
        RegistryOwner::Committed(EntityOwner::ChildSlot {
            parent,
            outlet,
            slot,
        })
        | RegistryOwner::Provisional(ProvisionalOwner::ChildSlot {
            parent,
            outlet,
            slot,
        }) => {
            let expected_lifecycle = match (claim.owner(), claim.lifecycle()) {
                (
                    RegistryOwner::Provisional(ProvisionalOwner::ChildSlot { .. }),
                    EntityLifecycle::PreparedInert,
                ) => ChildSlotLifecycle::ProvisionalInert,
                (
                    RegistryOwner::Committed(EntityOwner::ChildSlot { .. }),
                    EntityLifecycle::PreparedInert,
                ) => ChildSlotLifecycle::RegisteredInert,
                (
                    RegistryOwner::Committed(EntityOwner::ChildSlot { .. })
                    | RegistryOwner::Provisional(ProvisionalOwner::ChildSlot { .. }),
                    EntityLifecycle::Quarantined,
                ) => ChildSlotLifecycle::Quarantined,
                _ => return false,
            };
            world.get::<ChildSlotLifecycle>(entity) == Some(&expected_lifecycle)
                && world
                    .get::<ChildSlotAuthority>(entity)
                    .is_some_and(|authority| {
                        authority.parent() == parent
                            && authority.outlet() == outlet
                            && authority.slot() == *slot
                    })
        }
        _ => false,
    };
    world.get::<RouteOwnershipSeal>(entity) == Some(&RouteOwnershipSeal::new(claim.seal()))
        && lifecycle_is_exact
        && world.get::<Visibility>(entity) == Some(&Visibility::Hidden)
        && world.get::<Pickable>(entity) == Some(&Pickable::IGNORE)
        && world
            .get::<RouterInputBarrier>(entity)
            .is_some_and(RouterInputBarrier::denies_all)
}

pub(crate) fn observed_descendant_closure(
    world: &World,
    roots: impl IntoIterator<Item = Entity>,
) -> HashSet<Entity> {
    let mut stack = roots.into_iter().collect::<Vec<_>>();
    stack.sort_by_key(|entity| entity.to_bits());
    stack.reverse();
    let mut visited = HashSet::new();
    while let Some(entity) = stack.pop() {
        if !visited.insert(entity) {
            continue;
        }
        let mut children = world
            .get::<Children>(entity)
            .map(|children| children.iter().copied().collect::<Vec<_>>())
            .unwrap_or_default();
        children.sort_by_key(|child| child.to_bits());
        stack.extend(children.into_iter().rev());
    }
    visited
}

pub(crate) fn input_ready_system(_world: &mut World) {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        app_ext::{AppRouterExt, RouterConfig, RouterHostRegistration},
        content::{ContentColor, FrameContentWriter, SolidRect},
        driver::{EcsViewDriver, EcsViewReportSession, ViewBuildReport},
        navigation::{RouterNavigation, collect_navigation_system, drive_navigation_system},
        plugin::BevyUiRouterPlugin,
    };
    use bevy_app::App;
    use bevy_input_focus::InputFocus;
    use bevy_ui::Node;
    use bevy_ui_router_core::prelude::{
        RouteLocation, RouteNodeOptions, RoutePattern, RouteTree, RouteViewKey,
    };
    use bevy_ui_router_view::{
        RouteViewDescriptor, RouteViewRegistry, ViewBuildEffect, ViewDriverKey,
    };

    struct InputTestDriver {
        key: ViewDriverKey,
        nested: bool,
    }

    impl EcsViewDriver for InputTestDriver {
        fn key(&self) -> ViewDriverKey {
            self.key.clone()
        }

        fn apply(
            &mut self,
            effect: &ViewBuildEffect,
            content: &mut FrameContentWriter<'_>,
            report: EcsViewReportSession<'_>,
        ) -> ViewBuildReport {
            if self.nested && effect.view() == &RouteViewKey::new("layout").unwrap() {
                let anchor = content.spawn_empty().unwrap();
                content
                    .declare_outlet(anchor, bevy_ui_router_core::slot::RouteSlotKey::default())
                    .unwrap();
            } else {
                content
                    .spawn_solid_rect(
                        SolidRect::new(24, 16, ContentColor::srgba8(1, 2, 3, 255)).unwrap(),
                    )
                    .unwrap();
            }
            report.applied()
        }
    }

    fn active_atomic_app(label: &str) -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let driver = ViewDriverKey::new(format!("{label}-driver")).unwrap();
        app.register_ui_view_driver(InputTestDriver {
            key: driver.clone(),
            nested: false,
        })
        .unwrap();
        let view = RouteViewKey::new(format!("{label}-view")).unwrap();
        let routes = RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/").unwrap(),
                RouteNodeOptions::new().with_view(view.clone()),
            )
            .unwrap()
            .build()
            .unwrap();
        let mut views = RouteViewRegistry::new();
        views
            .insert(view.clone(), RouteViewDescriptor::new(driver))
            .unwrap();
        let router = app
            .add_ui_router(RouterConfig::new(routes, views, view))
            .unwrap();
        app.world_mut()
            .spawn((Node::default(), RouterHostRegistration::new(router)));
        app.update();
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        app.update();
        let output = app.world().resource::<RouterNavigation>().entries[0]
            .active
            .as_ref()
            .unwrap()
            .interactive_entities()[0];
        (app, output)
    }

    fn active_nested_app() -> (App, Entity, Entity) {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let driver = ViewDriverKey::new("input-nested-driver").unwrap();
        app.register_ui_view_driver(InputTestDriver {
            key: driver.clone(),
            nested: true,
        })
        .unwrap();
        let layout = RouteViewKey::new("layout").unwrap();
        let item = RouteViewKey::new("item").unwrap();
        let fallback = RouteViewKey::new("fallback").unwrap();
        let mut views = RouteViewRegistry::new();
        for view in [&layout, &item, &fallback] {
            views
                .insert(view.clone(), RouteViewDescriptor::new(driver.clone()))
                .unwrap();
        }
        let router = app
            .add_ui_router(RouterConfig::new(
                bevy_ui_router_core::test_support::nested_items_view_route_tree(),
                views,
                fallback,
            ))
            .unwrap();
        app.world_mut()
            .spawn((Node::default(), RouterHostRegistration::new(router)));
        app.update();
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/items/1"))
            .unwrap();
        for _ in 0..8 {
            app.update();
            if app.world().resource::<RouterNavigation>().entries[0]
                .active
                .is_some()
            {
                break;
            }
        }
        let active = app.world().resource::<RouterNavigation>().entries[0]
            .active
            .as_ref()
            .unwrap();
        let child_slot = active.child_slots()[0].slot();
        let child_output = active.frames()[1].interactive_entities()[0];
        (app, child_slot, child_output)
    }

    fn stage_nested_replacement(app: &mut App) {
        let router = app.world().resource::<RouterNavigation>().entries[0].handle;
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/items/2"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        for _ in 0..8 {
            drive_navigation_system(app.world_mut());
            if app
                .world()
                .resource::<RouterNavigation>()
                .pending_shape_for_test()
                .is_some_and(|(_, _, final_task)| final_task)
            {
                return;
            }
        }
        panic!("nested replacement did not reach its sealed pending projection");
    }

    #[test]
    fn gate_delta_preserves_other_router_permissions() {
        let mut world = World::new();
        let first = world.spawn_empty().id();
        let second = world.spawn_empty().id();
        let replacement = world.spawn_empty().id();
        world.insert_resource(RouterInputGate::new());
        world.resource_scope(|_world, gate: Mut<RouterInputGate>| {
            prepare_input_gate_mutation(gate, &[], vec![first])
                .expect("the first router permission can be installed")
                .apply();
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

    #[test]
    fn refresh_does_not_grant_a_missing_active_capability() {
        let (mut app, active) = active_atomic_app("refresh-no-grant");
        assert!(
            app.world()
                .resource::<RouterInputGate>()
                .allows_pointer(active)
        );
        app.world_mut()
            .resource_mut::<RouterInputGate>()
            .allowed
            .clear();

        refresh_input_gate_system(app.world_mut());

        assert!(
            !app.world()
                .resource::<RouterInputGate>()
                .allows_pointer(active)
        );
    }

    #[test]
    fn refresh_revokes_an_invalid_active_capability() {
        let (mut app, active) = active_atomic_app("refresh-revoke");
        assert!(
            app.world()
                .resource::<RouterInputGate>()
                .allows_pointer(active)
        );
        *app.world_mut().get_mut::<Visibility>(active).unwrap() = Visibility::Hidden;

        refresh_input_gate_system(app.world_mut());

        assert!(
            !app.world()
                .resource::<RouterInputGate>()
                .allows_pointer(active)
        );
    }

    #[test]
    fn refresh_rejects_child_boundary_drift_and_clears_inert_focus() {
        let (mut app, child_slot, child_output) = active_nested_app();
        assert!(
            app.world()
                .resource::<RouterInputGate>()
                .allows_pointer(child_output)
        );
        *app.world_mut().resource_mut::<InputFocus>() = InputFocus::from_entity(child_output);
        app.world_mut()
            .resource_mut::<RouteFocusState>()
            .set_focused(Some(child_output));
        *app.world_mut()
            .get_mut::<ChildSlotLifecycle>(child_slot)
            .unwrap() = ChildSlotLifecycle::RegisteredInert;

        refresh_input_gate_system(app.world_mut());

        assert!(
            !app.world()
                .resource::<RouterInputGate>()
                .allows_pointer(child_output)
        );
        assert_eq!(app.world().resource::<InputFocus>().get(), None);
        assert_eq!(app.world().resource::<RouteFocusState>().focused(), None);
    }

    #[test]
    fn refresh_rejects_an_unknown_sibling_beside_an_active_child_shell() {
        let (mut app, child_slot, child_output) = active_nested_app();
        app.world_mut().spawn(ChildOf(child_slot));

        refresh_input_gate_system(app.world_mut());

        assert!(
            !app.world()
                .resource::<RouterInputGate>()
                .allows_pointer(child_output)
        );
        assert_eq!(
            app.world().get::<Visibility>(child_slot),
            Some(&Visibility::Hidden)
        );
    }

    #[test]
    fn refresh_rejects_a_pending_sibling_after_its_projection_authority_is_stale() {
        let (mut app, child_slot, child_output) = active_nested_app();
        stage_nested_replacement(&mut app);
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .stale_pending_projection_for_test();

        refresh_input_gate_system(app.world_mut());

        assert!(
            !app.world()
                .resource::<RouterInputGate>()
                .allows_pointer(child_output)
        );
        assert_eq!(
            app.world().get::<Visibility>(child_slot),
            Some(&Visibility::Hidden)
        );
    }
}
