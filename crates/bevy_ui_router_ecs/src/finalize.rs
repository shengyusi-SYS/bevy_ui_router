use bevy_camera::visibility::Visibility;
use bevy_ecs::hierarchy::ChildOf;
use bevy_ecs::prelude::{Entity, World};
use bevy_input_focus::InputFocus;
use bevy_picking::Pickable;
use bevy_ui::InteractionDisabled;

use crate::components::{
    RouteFocusSnapshot, RouteFrameInteractionBarrier, RouteFrameInteractionSnapshot,
    RouteFrameRoot, RouteFrameRootLifecycleSnapshot, RouteFrameRootLifecycleState, RouteHost,
    RouteSlot,
};
use crate::state::RouteFocusState;

#[allow(dead_code)]
pub(crate) struct PrepareInertCapability;
#[allow(dead_code)]
pub(crate) struct PostCommitActivation;
#[allow(dead_code)]
pub(crate) struct InactiveFinalizationCapability;

#[allow(dead_code)]
#[derive(Debug, Eq, PartialEq)]
pub struct FinalizationReport {
    lifecycle: Option<RouteFrameRootLifecycleSnapshot>,
    interaction: RouteFrameInteractionSnapshot,
    focus: RouteFocusSnapshot,
    accepts_focus: bool,
}

#[allow(dead_code)]
impl FinalizationReport {
    pub fn lifecycle(&self) -> Option<RouteFrameRootLifecycleSnapshot> {
        self.lifecycle
    }

    pub fn interaction(&self) -> RouteFrameInteractionSnapshot {
        self.interaction
    }

    pub fn focus(&self) -> RouteFocusSnapshot {
        self.focus
    }

    pub fn accepts_focus(&self) -> bool {
        self.accepts_focus
    }

    fn from_world(world: &World, root: Entity) -> Self {
        let valid_owned_root = is_valid_router_owned_root(world, root);
        let lifecycle = valid_owned_root
            .then(|| world.get::<RouteFrameRootLifecycleState>(root))
            .flatten()
            .map(RouteFrameRootLifecycleState::snapshot);
        let barrier = world
            .get::<RouteFrameInteractionBarrier>(root)
            .map(RouteFrameInteractionBarrier::snapshot);
        let accepts_input_and_focus = valid_owned_root
            && matches!(lifecycle, Some(RouteFrameRootLifecycleSnapshot::Active))
            && barrier_allows_all_input(barrier);
        let focused = current_focused_entity(world);
        let focused_inside_root =
            focused.is_some_and(|entity| entity_is_in_subtree(world, root, entity));
        let effective_focus = if valid_owned_root && focused_inside_root && !accepts_input_and_focus
        {
            None
        } else {
            focused
        };

        Self {
            lifecycle,
            interaction: if accepts_input_and_focus {
                RouteFrameInteractionSnapshot::new(true, true, true)
            } else {
                RouteFrameInteractionSnapshot::new(false, false, false)
            },
            focus: RouteFocusSnapshot::new(effective_focus),
            accepts_focus: accepts_input_and_focus,
        }
    }
}

#[allow(dead_code)]
pub(crate) fn prepare_inert_root(
    world: &mut World,
    root: Entity,
    _capability: PrepareInertCapability,
) -> FinalizationReport {
    if is_valid_router_owned_root(world, root) {
        world
            .entity_mut(root)
            .insert(RouteFrameRootLifecycleState::new(
                RouteFrameRootLifecycleSnapshot::PreparedInert,
            ));
        apply_root_interaction_state(world, root, false);
        clear_focus_if_in_subtree(world, root);
    }

    FinalizationReport::from_world(world, root)
}

#[allow(dead_code)]
pub(crate) fn activate_committed_root(
    world: &mut World,
    root: Entity,
    _capability: PostCommitActivation,
) -> FinalizationReport {
    let can_activate = is_valid_router_owned_root(world, root)
        && matches!(
            world
                .get::<RouteFrameRootLifecycleState>(root)
                .map(RouteFrameRootLifecycleState::snapshot),
            Some(RouteFrameRootLifecycleSnapshot::PreparedInert)
        )
        && barrier_blocks_all_input(
            world
                .get::<RouteFrameInteractionBarrier>(root)
                .map(RouteFrameInteractionBarrier::snapshot),
        );

    let valid_owned_root = is_valid_router_owned_root(world, root);

    if can_activate {
        world
            .entity_mut(root)
            .insert(RouteFrameRootLifecycleState::new(
                RouteFrameRootLifecycleSnapshot::Active,
            ));
        apply_root_interaction_state(world, root, true);
    } else if valid_owned_root {
        apply_root_interaction_state(world, root, false);
        clear_focus_if_in_subtree(world, root);
    }

    FinalizationReport::from_world(world, root)
}

#[allow(dead_code)]
pub(crate) fn finalize_inactive_root(
    world: &mut World,
    root: Entity,
    _capability: InactiveFinalizationCapability,
) -> FinalizationReport {
    let should_finalize = is_valid_router_owned_root(world, root)
        && matches!(
            world
                .get::<RouteFrameRootLifecycleState>(root)
                .map(RouteFrameRootLifecycleState::snapshot),
            Some(
                RouteFrameRootLifecycleSnapshot::Hidden
                    | RouteFrameRootLifecycleSnapshot::Leaving
                    | RouteFrameRootLifecycleSnapshot::Failed
                    | RouteFrameRootLifecycleSnapshot::CleanupPending
                    | RouteFrameRootLifecycleSnapshot::Cached
            )
        );

    if should_finalize {
        apply_root_interaction_state(world, root, false);
        clear_focus_if_in_subtree(world, root);
    }

    FinalizationReport::from_world(world, root)
}

#[allow(dead_code)]
fn is_valid_router_owned_root(world: &World, root: Entity) -> bool {
    let Some(frame_root) = world.get::<RouteFrameRoot>(root) else {
        return false;
    };

    if world.get::<RouteHost>(root).is_some() {
        return false;
    }

    if world.get::<RouteSlot>(root).is_some() {
        return false;
    }

    let Some(route_host) = nearest_route_host_ancestor(world, root) else {
        return false;
    };

    let Some(route_slot) = nearest_route_slot_ancestor(world, root) else {
        return false;
    };

    let identity = frame_root.identity();
    route_host.router() == identity.router()
        && route_host.host() == identity.host()
        && route_slot.router() == identity.router()
        && route_slot.host() == identity.host()
        && route_slot.slot() == identity.slot()
}

#[allow(dead_code)]
fn nearest_route_host_ancestor(world: &World, start: Entity) -> Option<RouteHost> {
    let mut current = start;

    while let Some(parent) = world.get::<ChildOf>(current).map(ChildOf::parent) {
        if let Some(route_host) = world.get::<RouteHost>(parent) {
            return Some(*route_host);
        }
        current = parent;
    }

    None
}

#[allow(dead_code)]
fn nearest_route_slot_ancestor(world: &World, start: Entity) -> Option<RouteSlot> {
    let mut current = start;

    while let Some(parent) = world.get::<ChildOf>(current).map(ChildOf::parent) {
        if let Some(route_slot) = world.get::<RouteSlot>(parent) {
            return Some(*route_slot);
        }
        current = parent;
    }

    None
}

#[allow(dead_code)]
fn barrier_allows_all_input(barrier: Option<RouteFrameInteractionSnapshot>) -> bool {
    barrier.is_some_and(|snapshot| {
        snapshot.accepts_pointer() && snapshot.accepts_keyboard() && snapshot.accepts_gamepad()
    })
}

#[allow(dead_code)]
fn barrier_blocks_all_input(barrier: Option<RouteFrameInteractionSnapshot>) -> bool {
    barrier.is_some_and(|snapshot| {
        !snapshot.accepts_pointer() && !snapshot.accepts_keyboard() && !snapshot.accepts_gamepad()
    })
}

#[allow(dead_code)]
fn apply_root_interaction_state(world: &mut World, root: Entity, accepts_input: bool) {
    let mut entity = world.entity_mut(root);
    entity.insert(RouteFrameInteractionBarrier::new(
        accepts_input,
        accepts_input,
        accepts_input,
    ));

    if accepts_input {
        entity.insert(Visibility::Inherited);
        entity.insert(Pickable::default());
        entity.remove::<InteractionDisabled>();
    } else {
        entity.insert(Visibility::Hidden);
        entity.insert(Pickable::IGNORE);
        entity.insert(InteractionDisabled);
    }
}

#[allow(dead_code)]
fn clear_focus_if_in_subtree(world: &mut World, root: Entity) {
    let route_focus = current_route_focus_entity(world);
    if route_focus.is_some_and(|entity| entity_is_in_subtree(world, root, entity)) {
        if let Some(mut focus) = world.get_resource_mut::<RouteFocusState>() {
            focus.set_focused(None);
        }
    }

    let input_focus = current_input_focus_entity(world);
    if input_focus.is_some_and(|entity| entity_is_in_subtree(world, root, entity)) {
        if let Some(mut focus) = world.get_resource_mut::<InputFocus>() {
            focus.clear();
        }
    }
}

#[allow(dead_code)]
fn current_focused_entity(world: &World) -> Option<Entity> {
    current_input_focus_entity(world).or_else(|| current_route_focus_entity(world))
}

#[allow(dead_code)]
fn current_route_focus_entity(world: &World) -> Option<Entity> {
    world
        .get_resource::<RouteFocusState>()
        .and_then(RouteFocusState::focused)
}

#[allow(dead_code)]
fn current_input_focus_entity(world: &World) -> Option<Entity> {
    world.get_resource::<InputFocus>().and_then(InputFocus::get)
}

#[allow(dead_code)]
fn entity_is_in_subtree(world: &World, root: Entity, entity: Entity) -> bool {
    let mut current = entity;

    loop {
        if current == root {
            return true;
        }

        let Some(parent) = world.get::<ChildOf>(current).map(ChildOf::parent) else {
            return false;
        };
        current = parent;
    }
}

#[cfg(test)]
mod tests {
    use bevy_camera::visibility::Visibility;
    use bevy_ecs::prelude::{Component, Entity, World};
    use bevy_input_focus::InputFocus;
    use bevy_picking::Pickable;
    use bevy_ui::InteractionDisabled;
    use bevy_ui_router_core::location::RouteLocation;
    use bevy_ui_router_core::matcher::RouteMatcher;
    use bevy_ui_router_core::pattern::RoutePattern;
    use bevy_ui_router_core::route_tree::RouteTree;
    use bevy_ui_router_core::runtime::RouterRuntime;
    use bevy_ui_router_core::transaction::FrameApplyRequest;

    use crate::components::{
        RouteFrameInteractionBarrier, RouteFrameInteractionSnapshot, RouteFrameRoot,
        RouteFrameRootLifecycleSnapshot, RouteFrameRootLifecycleState, RouteHost, RouteSlot,
    };
    use crate::state::RouteFocusState;

    use super::{
        FinalizationReport, InactiveFinalizationCapability, PostCommitActivation,
        PrepareInertCapability, activate_committed_root, finalize_inactive_root,
        prepare_inert_root,
    };

    #[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
    struct AppOwnedSibling;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct MountedRoot {
        host: Entity,
        root: Entity,
        focused_child: Entity,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum InvalidTargetKind {
        Host,
        AppOwnedSibling,
        PlainEntity,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum InvalidRouterRootKind {
        ForeignRootIdentity,
        WrongSlot,
        MissingSlot,
    }

    #[test]
    fn post_commit_activation_is_the_only_path_that_enables_input_and_focus() {
        let (runtime, request) = frame_request("/");
        let mut world = World::new();
        let mounted = mount_router_root(&mut world, &runtime, &request);

        world
            .entity_mut(mounted.root)
            .insert(RouteFrameRootLifecycleState::new(
                RouteFrameRootLifecycleSnapshot::Preparing,
            ));
        set_focus(&mut world, mounted.focused_child);

        let prepared = prepare_inert_root(&mut world, mounted.root, PrepareInertCapability);
        assert_eq!(
            prepared.lifecycle(),
            Some(RouteFrameRootLifecycleSnapshot::PreparedInert)
        );
        assert_rejects_all_input(prepared.interaction());
        assert!(!prepared.accepts_focus());
        assert_eq!(prepared.focus().focused(), None);
        assert_authoritative_inert(&world, mounted.root);
        assert_bevy_root_inert(&world, mounted.root);
        assert_focus_cleared(&world);

        let active = activate_committed_root(&mut world, mounted.root, PostCommitActivation);
        assert_eq!(
            active.lifecycle(),
            Some(RouteFrameRootLifecycleSnapshot::Active)
        );
        assert_accepts_all_input(active.interaction());
        assert!(active.accepts_focus());
        assert_eq!(active.focus().focused(), None);
        assert_authoritative_accepts_all_input(&world, mounted.root);
        assert_bevy_root_active(&world, mounted.root);
    }

    #[test]
    fn hidden_leaving_failed_cleanup_pending_and_cached_roots_are_inert_and_release_focus() {
        for lifecycle in [
            RouteFrameRootLifecycleSnapshot::Hidden,
            RouteFrameRootLifecycleSnapshot::Leaving,
            RouteFrameRootLifecycleSnapshot::Failed,
            RouteFrameRootLifecycleSnapshot::CleanupPending,
            RouteFrameRootLifecycleSnapshot::Cached,
        ] {
            let (runtime, request) = frame_request("/");
            let mut world = World::new();
            let mounted = mount_router_root(&mut world, &runtime, &request);

            world
                .entity_mut(mounted.root)
                .insert(RouteFrameRootLifecycleState::new(lifecycle))
                .insert(RouteFrameInteractionBarrier::new(true, true, true));
            set_focus(&mut world, mounted.focused_child);

            let report =
                finalize_inactive_root(&mut world, mounted.root, InactiveFinalizationCapability);
            assert_eq!(report.lifecycle(), Some(lifecycle));
            assert_rejects_all_input(report.interaction());
            assert!(!report.accepts_focus());
            assert_eq!(report.focus().focused(), None);
            assert_authoritative_inert(&world, mounted.root);
            assert_bevy_root_inert(&world, mounted.root);
            assert_focus_cleared(&world);
        }
    }

    #[test]
    fn missing_lifecycle_markers_are_inert_and_cannot_become_active_or_keep_focus() {
        let (runtime, request) = frame_request("/");
        let mut world = World::new();
        let mounted = mount_router_root(&mut world, &runtime, &request);
        set_focus(&mut world, mounted.focused_child);

        let report = activate_committed_root(&mut world, mounted.root, PostCommitActivation);
        assert_eq!(report.lifecycle(), None);
        assert_rejects_all_input(report.interaction());
        assert!(!report.accepts_focus());
        assert_eq!(report.focus().focused(), None);
        assert_authoritative_inert(&world, mounted.root);
        assert_bevy_root_inert(&world, mounted.root);
        assert_focus_cleared(&world);
    }

    #[test]
    fn foreign_wrong_slot_and_missing_slot_roots_do_not_mutate_or_clear_focus() {
        for kind in [
            InvalidRouterRootKind::ForeignRootIdentity,
            InvalidRouterRootKind::WrongSlot,
            InvalidRouterRootKind::MissingSlot,
        ] {
            let (mut world, root, focused_child) = invalid_router_root_fixture(kind);

            let report = activate_committed_root(&mut world, root, PostCommitActivation);
            assert_eq!(report.lifecycle(), None);
            assert_rejects_all_input(report.interaction());
            assert!(!report.accepts_focus());
            assert_eq!(report.focus().focused(), Some(focused_child));
            assert_focus_preserved(&world, focused_child);
            assert_no_bevy_mapping(&world, root);
        }
    }

    #[test]
    fn invalid_lifecycle_and_interaction_combinations_are_inert_and_cannot_keep_focus() {
        let (runtime, request) = frame_request("/");
        let mut world = World::new();
        let mounted = mount_router_root(&mut world, &runtime, &request);

        world
            .entity_mut(mounted.root)
            .insert(RouteFrameRootLifecycleState::new(
                RouteFrameRootLifecycleSnapshot::Hidden,
            ))
            .insert(RouteFrameInteractionBarrier::new(true, true, true));
        set_focus(&mut world, mounted.focused_child);

        let report = activate_committed_root(&mut world, mounted.root, PostCommitActivation);
        assert_eq!(
            report.lifecycle(),
            Some(RouteFrameRootLifecycleSnapshot::Hidden)
        );
        assert_rejects_all_input(report.interaction());
        assert!(!report.accepts_focus());
        assert_eq!(report.focus().focused(), None);
        assert_authoritative_inert(&world, mounted.root);
        assert_bevy_root_inert(&world, mounted.root);
        assert_focus_cleared(&world);
    }

    #[test]
    fn app_owned_host_sibling_and_plain_entities_do_not_receive_focus_side_effects() {
        assert_invalid_targets_preserve_focus(|world, target| {
            prepare_inert_root(world, target, PrepareInertCapability)
        });
        assert_invalid_targets_preserve_focus(|world, target| {
            activate_committed_root(world, target, PostCommitActivation)
        });
        assert_invalid_targets_preserve_focus(|world, target| {
            finalize_inactive_root(world, target, InactiveFinalizationCapability)
        });
    }

    #[test]
    fn focus_outside_targeted_router_owned_subtree_and_on_app_owned_host_is_preserved() {
        let (runtime, request) = frame_request("/");
        let mut world = World::new();
        let mounted = mount_router_root(&mut world, &runtime, &request);
        let app_owned_sibling = world.spawn((AppOwnedSibling,)).id();
        let focused_outside = world.spawn_empty().id();

        world.entity_mut(mounted.host).add_child(app_owned_sibling);
        world
            .entity_mut(app_owned_sibling)
            .add_child(focused_outside);
        world
            .entity_mut(mounted.root)
            .insert(RouteFrameRootLifecycleState::new(
                RouteFrameRootLifecycleSnapshot::Hidden,
            ))
            .insert(RouteFrameInteractionBarrier::new(true, true, true));

        set_focus(&mut world, mounted.host);

        let host_report =
            finalize_inactive_root(&mut world, mounted.root, InactiveFinalizationCapability);
        assert_eq!(host_report.focus().focused(), Some(mounted.host));
        assert_focus_preserved(&world, mounted.host);

        set_focus(&mut world, focused_outside);

        let outside_report =
            finalize_inactive_root(&mut world, mounted.root, InactiveFinalizationCapability);
        assert_eq!(outside_report.focus().focused(), Some(focused_outside));
        assert_focus_preserved(&world, focused_outside);
    }

    fn assert_rejects_all_input(interaction: RouteFrameInteractionSnapshot) {
        assert!(!interaction.accepts_pointer());
        assert!(!interaction.accepts_keyboard());
        assert!(!interaction.accepts_gamepad());
    }

    fn assert_accepts_all_input(interaction: RouteFrameInteractionSnapshot) {
        assert!(interaction.accepts_pointer());
        assert!(interaction.accepts_keyboard());
        assert!(interaction.accepts_gamepad());
    }

    fn assert_authoritative_inert(world: &World, root: Entity) {
        assert_eq!(
            world
                .get::<RouteFrameInteractionBarrier>(root)
                .map(RouteFrameInteractionBarrier::snapshot),
            Some(RouteFrameInteractionSnapshot::new(false, false, false))
        );
    }

    fn assert_authoritative_accepts_all_input(world: &World, root: Entity) {
        assert_eq!(
            world
                .get::<RouteFrameInteractionBarrier>(root)
                .map(RouteFrameInteractionBarrier::snapshot),
            Some(RouteFrameInteractionSnapshot::new(true, true, true))
        );
    }

    fn assert_bevy_root_inert(world: &World, root: Entity) {
        assert_eq!(world.get::<Visibility>(root), Some(&Visibility::Hidden));
        assert_eq!(world.get::<Pickable>(root), Some(&Pickable::IGNORE));
        assert!(
            world.get::<InteractionDisabled>(root).is_some(),
            "inert root should carry InteractionDisabled"
        );
    }

    fn assert_bevy_root_active(world: &World, root: Entity) {
        assert_eq!(world.get::<Visibility>(root), Some(&Visibility::Inherited));
        assert_eq!(world.get::<Pickable>(root), Some(&Pickable::default()));
        assert!(
            world.get::<InteractionDisabled>(root).is_none(),
            "active root should not carry InteractionDisabled"
        );
    }

    fn assert_no_bevy_mapping(world: &World, entity: Entity) {
        assert_eq!(world.get::<Visibility>(entity), None);
        assert_eq!(world.get::<Pickable>(entity), None);
        assert!(world.get::<InteractionDisabled>(entity).is_none());
    }

    fn assert_focus_cleared(world: &World) {
        assert_eq!(world.resource::<RouteFocusState>().focused(), None);
        assert_eq!(world.resource::<InputFocus>().get(), None);
    }

    fn assert_focus_preserved(world: &World, focused: Entity) {
        assert_eq!(world.resource::<RouteFocusState>().focused(), Some(focused));
        assert_eq!(world.resource::<InputFocus>().get(), Some(focused));
    }

    fn set_focus(world: &mut World, focused: Entity) {
        if world.get_resource::<RouteFocusState>().is_none() {
            world.insert_resource(RouteFocusState::default());
        }
        world
            .resource_mut::<RouteFocusState>()
            .set_focused(Some(focused));
        world.insert_resource(InputFocus::from_entity(focused));
    }

    fn assert_invalid_targets_preserve_focus(
        operation: impl Fn(&mut World, Entity) -> FinalizationReport + Copy,
    ) {
        for kind in [
            InvalidTargetKind::Host,
            InvalidTargetKind::AppOwnedSibling,
            InvalidTargetKind::PlainEntity,
        ] {
            let (mut world, target, focused_child) = invalid_target_fixture(kind);

            let report = operation(&mut world, target);
            assert_eq!(report.lifecycle(), None);
            assert_rejects_all_input(report.interaction());
            assert!(!report.accepts_focus());
            assert_eq!(report.focus().focused(), Some(focused_child));
            assert_focus_preserved(&world, focused_child);
            assert_no_bevy_mapping(&world, target);
        }
    }

    fn mount_router_root(
        world: &mut World,
        runtime: &RouterRuntime,
        request: &FrameApplyRequest,
    ) -> MountedRoot {
        let host = world.spawn((RouteHost::from_runtime(runtime),)).id();
        let slot = world.spawn((RouteSlot::from_runtime(runtime),)).id();
        let root = world
            .spawn((RouteFrameRoot::from_frame_request(request),))
            .id();
        let focused_child = world.spawn_empty().id();

        world.entity_mut(host).add_child(slot);
        world.entity_mut(slot).add_child(root);
        world.entity_mut(root).add_child(focused_child);

        MountedRoot {
            host,
            root,
            focused_child,
        }
    }

    fn invalid_target_fixture(kind: InvalidTargetKind) -> (World, Entity, Entity) {
        let (runtime, request) = frame_request("/");
        let mut world = World::new();
        let mounted = mount_router_root(&mut world, &runtime, &request);
        let app_owned_sibling = world.spawn((AppOwnedSibling,)).id();
        let plain_entity = world.spawn_empty().id();
        let focused_child = world.spawn_empty().id();

        world.entity_mut(mounted.host).add_child(app_owned_sibling);

        let target = match kind {
            InvalidTargetKind::Host => {
                world.entity_mut(mounted.host).add_child(focused_child);
                mounted.host
            }
            InvalidTargetKind::AppOwnedSibling => {
                world.entity_mut(app_owned_sibling).add_child(focused_child);
                app_owned_sibling
            }
            InvalidTargetKind::PlainEntity => {
                world.entity_mut(plain_entity).add_child(focused_child);
                plain_entity
            }
        };

        set_focus(&mut world, focused_child);
        (world, target, focused_child)
    }

    fn invalid_router_root_fixture(kind: InvalidRouterRootKind) -> (World, Entity, Entity) {
        let (runtime, request) = frame_request("/");
        let (foreign_runtime, foreign_request) = frame_request("/");
        let mut world = World::new();

        let host = world.spawn((RouteHost::from_runtime(&runtime),)).id();
        let valid_slot = world.spawn((RouteSlot::from_runtime(&runtime),)).id();
        let wrong_slot = world
            .spawn((RouteSlot::from_runtime(&foreign_runtime),))
            .id();
        let root_request = match kind {
            InvalidRouterRootKind::ForeignRootIdentity => &foreign_request,
            InvalidRouterRootKind::WrongSlot | InvalidRouterRootKind::MissingSlot => &request,
        };
        let root = world
            .spawn((
                RouteFrameRoot::from_frame_request(root_request),
                RouteFrameRootLifecycleState::new(RouteFrameRootLifecycleSnapshot::PreparedInert),
                RouteFrameInteractionBarrier::new(false, false, false),
            ))
            .id();
        let focused_child = world.spawn_empty().id();

        world
            .entity_mut(host)
            .add_children(&[valid_slot, wrong_slot]);
        match kind {
            InvalidRouterRootKind::ForeignRootIdentity => {
                world.entity_mut(valid_slot).add_child(root);
            }
            InvalidRouterRootKind::WrongSlot => {
                world.entity_mut(wrong_slot).add_child(root);
            }
            InvalidRouterRootKind::MissingSlot => {
                world.entity_mut(host).add_child(root);
            }
        }
        world.entity_mut(root).add_child(focused_child);

        set_focus(&mut world, focused_child);
        (world, root, focused_child)
    }

    fn frame_request(path: &str) -> (RouterRuntime, FrameApplyRequest) {
        let matcher = RouteMatcher::new(
            RouteTree::builder()
                .root(RoutePattern::path("/").unwrap())
                .unwrap()
                .build()
                .unwrap(),
        );
        let target = matcher.match_location(&RouteLocation::new(path)).unwrap();
        let mut runtime = RouterRuntime::new();
        let snapshot = runtime.begin_first_navigation(&target).unwrap();
        (runtime, snapshot.requests()[0].clone())
    }
}
