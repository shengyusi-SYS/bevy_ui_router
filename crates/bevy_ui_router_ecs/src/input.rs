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
use bevy_picking::{
    Pickable,
    events::PointerState,
    hover::{HoverMap, PickingInteraction, PreviousHoverMap},
};

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
    /// Targets that were previously authorized and must have all retained
    /// native pointer state removed before the next dispatch.
    revoked: Vec<Entity>,
}

impl RouterInputGate {
    pub(crate) fn new() -> Self {
        Self {
            allowed: Vec::new(),
            revoked: Vec::new(),
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

    fn replace_allowed(&mut self, next: Vec<Entity>, revoked: Vec<Entity>) {
        self.allowed = next;
        self.revoked = revoked;
    }

    fn retain_allowed(&mut self, allowed: &HashSet<Entity>) {
        for entity in self.allowed.iter().copied() {
            if !allowed.contains(&entity) && !self.revoked.contains(&entity) {
                self.revoked.push(entity);
            }
        }
        self.allowed.retain(|entity| allowed.contains(entity));
        self.revoked.retain(|entity| !allowed.contains(entity));
    }
}

#[derive(Clone)]
pub(crate) struct PreparedInputGateIsolation {
    next: Vec<Entity>,
    revoked: Vec<Entity>,
}

impl PreparedInputGateIsolation {
    pub(crate) fn apply(self, gate: &mut RouterInputGate) {
        gate.replace_allowed(self.next, self.revoked);
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
    let revoked = try_prepare_pointer_revocations(gate, &next)?;
    Ok(PreparedInputGateIsolation { next, revoked })
}

pub(crate) struct PreparedInputGateMutation<'w> {
    gate: Mut<'w, RouterInputGate>,
    next: Vec<Entity>,
    revoked: Vec<Entity>,
}

impl PreparedInputGateMutation<'_> {
    pub(crate) fn apply(mut self) {
        self.gate.replace_allowed(self.next, self.revoked);
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
    let revoked = try_prepare_pointer_revocations(&gate, &next).ok()?;
    Some(PreparedInputGateMutation {
        gate,
        next,
        revoked,
    })
}

fn try_prepare_pointer_revocations(
    gate: &RouterInputGate,
    next: &[Entity],
) -> Result<Vec<Entity>, ()> {
    let capacity = gate
        .revoked
        .len()
        .checked_add(gate.allowed.len())
        .ok_or(())?;
    let mut revoked = Vec::new();
    revoked.try_reserve_exact(capacity).map_err(|_| ())?;
    revoked.extend(gate.revoked.iter().copied());
    for entity in gate.allowed.iter().copied() {
        if !next.contains(&entity) && !revoked.contains(&entity) {
            revoked.push(entity);
        }
    }
    revoked.retain(|entity| !next.contains(entity));
    Ok(revoked)
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
    // Lifecycle publication may have revoked a target after the preceding
    // picking pass. Clear its retained gesture state before the next backend
    // sample; the second scheduled pass also filters newly generated hits.
    revoke_pending_router_pointer_targets(world);
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

/// Removes exact targets from Bevy's persistent pointer state without touching
/// application-owned or other Router targets. Callers must supply a target set
/// already authorized by their lifecycle/ownership preparation; this function
/// deliberately does not infer Router authority.
pub(crate) fn revoke_router_pointer_targets(world: &mut World, targets: &HashSet<Entity>) {
    if targets.is_empty() {
        return;
    }
    if let Some(mut hover_map) = world.get_resource_mut::<HoverMap>() {
        for hits in hover_map.values_mut() {
            hits.retain(|entity, _| !targets.contains(entity));
        }
    }
    if let Some(mut previous_hover_map) = world.get_resource_mut::<PreviousHoverMap>() {
        for hits in previous_hover_map.values_mut() {
            hits.retain(|entity, _| !targets.contains(entity));
        }
    }
    if let Some(mut pointer_state) = world.get_resource_mut::<PointerState>() {
        for state in pointer_state.pointer_buttons.values_mut() {
            state.pressing.retain(|entity, _| !targets.contains(entity));
            state.clicking.retain(|entity, _| !targets.contains(entity));
            state.dragging.retain(|entity, _| !targets.contains(entity));
            state
                .dragging_over
                .retain(|entity, _| !targets.contains(entity));
        }
        pointer_state
            .hovered_entity_ancestors
            .retain(|hovered, ancestors| {
                ancestors.retain(|ancestor| !targets.contains(ancestor));
                !targets.contains(hovered)
            });
    }
    for target in targets.iter().copied() {
        if let Some(mut interaction) = world.get_mut::<PickingInteraction>(target)
            && *interaction != PickingInteraction::None
        {
            *interaction = PickingInteraction::None;
        }
    }
}

fn revoke_pending_router_pointer_targets(world: &mut World) {
    let targets = world
        .get_resource::<RouterInputGate>()
        .map(|gate| gate.revoked.iter().copied().collect::<HashSet<_>>())
        .unwrap_or_default();
    revoke_router_pointer_targets(world, &targets);
}

/// Filters Bevy picking state immediately before its interaction/event chain.
///
/// The gate remains the authority snapshot. Registry/quarantine records are
/// consulted only to distinguish a denied Router scope from unrelated
/// application state; an exact ownership seal is required before filtering.
pub(crate) fn filter_inactive_router_pointer_state_system(world: &mut World) {
    if world.get_resource::<RouterInputGate>().is_none()
        || world.get_resource::<RouteEntityRegistry>().is_none()
        || world.get_resource::<QuarantineLeaseTable>().is_none()
    {
        return;
    }

    let mut candidates = HashSet::new();
    if let Some(hover_map) = world.get_resource::<HoverMap>() {
        candidates.extend(hover_map.values().flat_map(|hits| hits.keys().copied()));
    }
    if let Some(previous_hover_map) = world.get_resource::<PreviousHoverMap>() {
        candidates.extend(
            previous_hover_map
                .values()
                .flat_map(|hits| hits.keys().copied()),
        );
    }
    if let Some(pointer_state) = world.get_resource::<PointerState>() {
        for state in pointer_state.pointer_buttons.values() {
            candidates.extend(state.pressing.keys().copied());
            candidates.extend(state.clicking.keys().copied());
            candidates.extend(state.dragging.keys().copied());
            candidates.extend(state.dragging_over.keys().copied());
        }
        candidates.extend(pointer_state.hovered_entity_ancestors.keys().copied());
    }

    let gate = world.resource::<RouterInputGate>();
    let mut denied = gate.revoked.iter().copied().collect::<HashSet<_>>();
    denied.extend(candidates.into_iter().filter(|target| {
        !gate.allows_pointer(*target) && target_has_exact_router_boundary(world, *target)
    }));
    revoke_router_pointer_targets(world, &denied);
    world.resource_mut::<RouterInputGate>().revoked.clear();
}

fn target_has_exact_router_boundary(world: &World, target: Entity) -> bool {
    let registry = world.resource::<RouteEntityRegistry>();
    let quarantine = world.resource::<QuarantineLeaseTable>();
    let mut current = target;
    let mut visited = HashSet::new();
    while visited.insert(current) {
        let claim = registry
            .record(current)
            .map(|record| record.claim())
            .or_else(|| quarantine.record(current).map(|record| record.claim()))
            .or_else(|| {
                quarantine
                    .unresolved_record(current)
                    .map(|record| record.claim())
            });
        if claim.is_some_and(|claim| {
            world.get::<RouteOwnershipSeal>(current) == Some(&RouteOwnershipSeal::new(claim.seal()))
        }) {
            return true;
        }
        let Some(parent) = world.get::<ChildOf>(current).map(ChildOf::parent) else {
            return false;
        };
        current = parent;
    }
    false
}

pub(crate) fn input_ready_system(_world: &mut World) {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        app_ext::{AppRouterExt, RouterConfig, RouterHostRegistration},
        content::{ContentColor, FrameContentWriter, SolidRect},
        driver::{EcsViewDriver, EcsViewReportSession, ViewBuildReport},
        navigation::{
            RouterNavigation, collect_navigation_system, commit_presentation_system,
            drive_navigation_system,
        },
        plugin::BevyUiRouterPlugin,
    };
    use bevy_app::{App, PreUpdate};
    use bevy_ecs::{
        event::EntityEvent,
        observer::On,
        prelude::{ResMut, Resource},
        query::With,
    };
    use bevy_input_focus::InputFocus;
    use bevy_internal::{
        DefaultPlugins,
        camera::{Camera2d, NormalizedRenderTarget},
        math::Vec2,
        picking::{
            events::{Click, Drag, DragEnd, Pointer, Press, Release},
            hover::{HoverMap, PreviousHoverMap},
            pointer::{
                Location, PointerAction, PointerButton, PointerId, PointerInput,
                PointerInteraction, PointerLocation, PointerPress,
            },
        },
        window::{PrimaryWindow, Window, WindowRef},
    };
    use bevy_picking::events::PointerState;
    use bevy_ui::{ComputedNode, Node, PositionType, UiGlobalTransform, Val};
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

    #[derive(Clone, Copy, Default)]
    struct PointerDeliveryCounts {
        presses: usize,
        drags: usize,
        releases: usize,
        clicks: usize,
        drag_ends: usize,
    }

    #[derive(Resource, Default)]
    struct PointerDeliveries(std::collections::BTreeMap<Entity, PointerDeliveryCounts>);

    fn install_pointer_history_observers(app: &mut App) {
        app.world_mut().add_observer(
            |event: On<Pointer<Press>>, mut deliveries: ResMut<PointerDeliveries>| {
                deliveries
                    .0
                    .entry(event.event_target())
                    .or_default()
                    .presses += 1;
            },
        );
        app.world_mut().add_observer(
            |event: On<Pointer<Drag>>, mut deliveries: ResMut<PointerDeliveries>| {
                deliveries.0.entry(event.event_target()).or_default().drags += 1;
            },
        );
        app.world_mut().add_observer(
            |event: On<Pointer<Release>>, mut deliveries: ResMut<PointerDeliveries>| {
                deliveries
                    .0
                    .entry(event.event_target())
                    .or_default()
                    .releases += 1;
            },
        );
        app.world_mut().add_observer(
            |event: On<Pointer<Click>>, mut deliveries: ResMut<PointerDeliveries>| {
                deliveries.0.entry(event.event_target()).or_default().clicks += 1;
            },
        );
        app.world_mut().add_observer(
            |event: On<Pointer<DragEnd>>, mut deliveries: ResMut<PointerDeliveries>| {
                deliveries
                    .0
                    .entry(event.event_target())
                    .or_default()
                    .drag_ends += 1;
            },
        );
    }

    fn pointer_location(world: &World, window: Entity, target: Entity) -> Location {
        let position = *world.get::<UiGlobalTransform>(target).unwrap() * Vec2::ZERO;
        Location {
            target: NormalizedRenderTarget::Window(
                WindowRef::Primary.normalize(Some(window)).unwrap(),
            ),
            position,
        }
    }

    fn write_pointer_inputs(app: &mut App, inputs: impl IntoIterator<Item = PointerInput>) {
        for input in inputs {
            app.world_mut().write_message(input);
        }
        app.world_mut().run_schedule(PreUpdate);
    }

    fn stage_atomic_replacement(app: &mut App) {
        let router = app.world().resource::<RouterNavigation>().entries[0].handle;
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(
                router,
                RouteLocation::new("/").with_fragment("pointer-replacement"),
            )
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
        panic!("atomic replacement did not reach its sealed pending projection");
    }

    #[test]
    fn native_pointer_history_is_revoked_before_dispatch_without_touching_app_state() {
        let mut app = App::new();
        app.add_plugins(DefaultPlugins);
        app.add_plugins(BevyUiRouterPlugin::default());
        app.init_resource::<PointerDeliveries>();
        install_pointer_history_observers(&mut app);

        let primary_window = app
            .world_mut()
            .query_filtered::<Entity, With<PrimaryWindow>>()
            .single(app.world())
            .unwrap();
        app.world_mut()
            .get_mut::<Window>(primary_window)
            .unwrap()
            .resolution
            .set(320.0, 180.0);
        app.world_mut().spawn(Camera2d);

        let driver = ViewDriverKey::new("pointer-history-driver").unwrap();
        app.register_ui_view_driver(InputTestDriver {
            key: driver.clone(),
            nested: false,
        })
        .unwrap();
        let view = RouteViewKey::new("pointer-history-view").unwrap();
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
        app.world_mut().spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..Node::default()
            },
            RouterHostRegistration::new(router),
        ));
        let application_target = app
            .world_mut()
            .spawn(Node {
                position_type: PositionType::Absolute,
                left: Val::Px(100.0),
                top: Val::Px(60.0),
                width: Val::Px(24.0),
                height: Val::Px(16.0),
                ..Node::default()
            })
            .id();
        app.update();
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        app.update();
        app.update();

        let router_target = app.world().resource::<RouterNavigation>().entries[0]
            .active
            .as_ref()
            .unwrap()
            .interactive_entities()[0];
        assert!(
            app.world()
                .get::<ComputedNode>(router_target)
                .is_some_and(|computed| computed.size().cmpgt(Vec2::ZERO).all())
        );
        assert!(
            app.world()
                .get::<ComputedNode>(application_target)
                .is_some_and(|computed| computed.size().cmpgt(Vec2::ZERO).all())
        );

        let mouse_location = pointer_location(app.world(), primary_window, router_target);
        let application_location =
            pointer_location(app.world(), primary_window, application_target);
        let mouse_pointer = app
            .world_mut()
            .query::<(Entity, &PointerId)>()
            .iter(app.world())
            .find_map(|(entity, id)| (*id == PointerId::Mouse).then_some(entity))
            .unwrap();
        *app.world_mut()
            .get_mut::<PointerLocation>(mouse_pointer)
            .unwrap() = PointerLocation::new(mouse_location.clone());
        let application_pointer = PointerId::Touch(0x5255_4954);
        app.world_mut().spawn((
            application_pointer,
            PointerLocation::new(application_location.clone()),
            PointerPress::default(),
            PointerInteraction::default(),
        ));
        app.world_mut().run_schedule(PreUpdate);
        assert!(
            app.world()
                .resource::<HoverMap>()
                .get(&PointerId::Mouse)
                .is_some_and(|hits| hits.contains_key(&router_target)),
            "the real UI backend must positively hit active router content"
        );
        assert!(
            app.world()
                .resource::<HoverMap>()
                .get(&application_pointer)
                .is_some_and(|hits| hits.contains_key(&application_target)),
            "the real UI backend must positively hit independent application content"
        );
        let active_router_ancestor = {
            let world = app.world();
            let gate = world.resource::<RouterInputGate>();
            let mut current = router_target;
            let mut visited = HashSet::new();
            loop {
                assert!(
                    visited.insert(current),
                    "the active Router hierarchy must remain acyclic"
                );
                let parent = world
                    .get::<ChildOf>(current)
                    .map(ChildOf::parent)
                    .expect("active Router content must remain below its sealed allocation");
                if world.get::<RouteOwnershipSeal>(parent).is_some() && !gate.allows_pointer(parent)
                {
                    break parent;
                }
                current = parent;
            }
        };

        write_pointer_inputs(
            &mut app,
            [
                PointerInput::new(
                    PointerId::Mouse,
                    mouse_location.clone(),
                    PointerAction::Press(PointerButton::Primary),
                ),
                PointerInput::new(
                    application_pointer,
                    application_location.clone(),
                    PointerAction::Press(PointerButton::Primary),
                ),
            ],
        );
        let moved_mouse = Location {
            position: mouse_location.position + Vec2::X,
            ..mouse_location.clone()
        };
        let moved_application = Location {
            position: application_location.position + Vec2::X,
            ..application_location.clone()
        };
        write_pointer_inputs(
            &mut app,
            [
                PointerInput::new(
                    PointerId::Mouse,
                    moved_mouse.clone(),
                    PointerAction::Move { delta: Vec2::X },
                ),
                PointerInput::new(
                    application_pointer,
                    moved_application.clone(),
                    PointerAction::Move { delta: Vec2::X },
                ),
            ],
        );
        let before_leave = app.world().resource::<PointerDeliveries>().0.clone();
        assert_eq!(before_leave[&router_target].presses, 1);
        assert_eq!(before_leave[&router_target].drags, 1);
        assert_eq!(before_leave[&application_target].presses, 1);
        assert_eq!(before_leave[&application_target].drags, 1);
        assert!(
            app.world()
                .resource::<PointerState>()
                .get_ancestors(&router_target)
                .is_some_and(|ancestors| ancestors.contains(&active_router_ancestor)),
            "an active leaf must retain its non-interactive Router ancestor cache"
        );
        assert_eq!(
            app.world().get::<PickingInteraction>(router_target),
            Some(&PickingInteraction::Pressed)
        );
        assert_eq!(
            app.world().get::<PickingInteraction>(application_target),
            Some(&PickingInteraction::Pressed)
        );

        stage_atomic_replacement(&mut app);
        commit_presentation_system(app.world_mut());
        assert!(
            !app.world()
                .resource::<RouterInputGate>()
                .allows_pointer(router_target)
        );
        assert!(app.world().get_entity(router_target).is_ok());
        // Historical authorization, rather than an ancestor boundary or the
        // now-drifted physical claim, must still revoke the gesture begun active.
        app.world_mut()
            .entity_mut(router_target)
            .remove::<(ChildOf, RouteOwnershipSeal)>();

        let moved_mouse_again = Location {
            position: moved_mouse.position + Vec2::X,
            ..moved_mouse.clone()
        };
        let moved_application_again = Location {
            position: moved_application.position + Vec2::X,
            ..moved_application.clone()
        };
        write_pointer_inputs(
            &mut app,
            [
                PointerInput::new(
                    PointerId::Mouse,
                    moved_mouse_again.clone(),
                    PointerAction::Move { delta: Vec2::X },
                ),
                PointerInput::new(
                    application_pointer,
                    moved_application_again.clone(),
                    PointerAction::Move { delta: Vec2::X },
                ),
            ],
        );

        let after_filtered_move = app.world().resource::<PointerDeliveries>().0.clone();
        assert_eq!(
            after_filtered_move[&router_target].drags, before_leave[&router_target].drags,
            "a surviving leaving target must not receive a continued drag"
        );
        assert_eq!(
            after_filtered_move[&application_target].drags,
            before_leave[&application_target].drags + 1,
            "another pointer's application-owned drag must remain live"
        );
        let state = app.world().resource::<PointerState>();
        let router_button = state.get(PointerId::Mouse, PointerButton::Primary).unwrap();
        assert!(!router_button.pressing.contains_key(&router_target));
        assert!(!router_button.clicking.contains_key(&router_target));
        assert!(!router_button.dragging.contains_key(&router_target));
        assert!(!router_button.dragging_over.contains_key(&router_target));
        let application_button = state
            .get(application_pointer, PointerButton::Primary)
            .unwrap();
        assert!(
            application_button
                .pressing
                .contains_key(&application_target)
        );
        assert!(
            application_button
                .clicking
                .contains_key(&application_target)
        );
        assert!(
            application_button
                .dragging
                .contains_key(&application_target)
        );
        assert_eq!(
            app.world().get::<PickingInteraction>(router_target),
            Some(&PickingInteraction::None),
            "revoking both hover maps must also clear retained interaction state"
        );
        assert_eq!(
            app.world().get::<PickingInteraction>(application_target),
            Some(&PickingInteraction::Pressed),
            "application interaction state must remain live"
        );
        assert!(
            app.world()
                .resource::<HoverMap>()
                .get(&PointerId::Mouse)
                .is_none_or(|hits| !hits.contains_key(&router_target))
        );
        assert!(
            app.world()
                .resource::<PreviousHoverMap>()
                .get(&PointerId::Mouse)
                .is_none_or(|hits| !hits.contains_key(&router_target))
        );
        assert!(state.get_ancestors(&router_target).is_none());
        assert!(
            state
                .hovered_entity_ancestors
                .values()
                .all(|ancestors| !ancestors.contains(&router_target))
        );

        write_pointer_inputs(
            &mut app,
            [
                PointerInput::new(
                    PointerId::Mouse,
                    moved_mouse_again,
                    PointerAction::Release(PointerButton::Primary),
                ),
                PointerInput::new(
                    application_pointer,
                    moved_application_again,
                    PointerAction::Release(PointerButton::Primary),
                ),
            ],
        );
        let after_release = &app.world().resource::<PointerDeliveries>().0;
        assert_eq!(
            after_release[&router_target].releases,
            before_leave[&router_target].releases
        );
        assert_eq!(
            after_release[&router_target].clicks,
            before_leave[&router_target].clicks
        );
        assert_eq!(
            after_release[&router_target].drag_ends,
            before_leave[&router_target].drag_ends
        );
        assert_eq!(after_release[&application_target].releases, 1);
        assert_eq!(after_release[&application_target].clicks, 1);
        assert_eq!(after_release[&application_target].drag_ends, 1);
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
