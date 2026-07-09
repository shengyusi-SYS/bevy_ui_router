use bevy_ecs::hierarchy::ChildOf;
use bevy_ecs::prelude::{Entity, World};

use crate::components::{
    RouteFocusSnapshot, RouteFrameInteractionBarrier, RouteFrameInteractionSnapshot,
    RouteFrameRoot, RouteFrameRootLifecycleSnapshot, RouteFrameRootLifecycleState, RouteHost,
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
        let effective_focus = if focused_inside_root && !accepts_input_and_focus {
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
        let mut entity = world.entity_mut(root);
        entity.insert(RouteFrameRootLifecycleState::new(
            RouteFrameRootLifecycleSnapshot::PreparedInert,
        ));
        entity.insert(RouteFrameInteractionBarrier::new(false, false, false));
        drop(entity);
    }

    clear_focus_if_in_subtree(world, root);
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

    if can_activate {
        let mut entity = world.entity_mut(root);
        entity.insert(RouteFrameRootLifecycleState::new(
            RouteFrameRootLifecycleSnapshot::Active,
        ));
        entity.insert(RouteFrameInteractionBarrier::new(true, true, true));
        drop(entity);
    } else {
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
        world
            .entity_mut(root)
            .insert(RouteFrameInteractionBarrier::new(false, false, false));
    }

    if should_finalize || !FinalizationReport::from_world(world, root).accepts_focus() {
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

    let Some(route_host) = nearest_route_host_ancestor(world, root) else {
        return false;
    };

    route_host.router() == frame_root.identity().router()
        && route_host.host() == frame_root.identity().host()
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
fn clear_focus_if_in_subtree(world: &mut World, root: Entity) {
    let focused = current_focused_entity(world);
    if focused.is_some_and(|entity| entity_is_in_subtree(world, root, entity)) {
        if let Some(mut focus) = world.get_resource_mut::<RouteFocusState>() {
            focus.set_focused(None);
        }
    }
}

#[allow(dead_code)]
fn current_focused_entity(world: &World) -> Option<Entity> {
    world
        .get_resource::<RouteFocusState>()
        .and_then(RouteFocusState::focused)
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
    use bevy_ecs::prelude::{Component, World};
    use bevy_ui_router_core::location::RouteLocation;
    use bevy_ui_router_core::matcher::RouteMatcher;
    use bevy_ui_router_core::pattern::RoutePattern;
    use bevy_ui_router_core::route_tree::RouteTree;
    use bevy_ui_router_core::runtime::RouterRuntime;
    use bevy_ui_router_core::transaction::FrameApplyRequest;

    use crate::components::{
        RouteFrameInteractionBarrier, RouteFrameInteractionSnapshot, RouteFrameRoot,
        RouteFrameRootLifecycleSnapshot, RouteFrameRootLifecycleState, RouteHost,
    };
    use crate::state::RouteFocusState;

    use super::{
        InactiveFinalizationCapability, PostCommitActivation, PrepareInertCapability,
        activate_committed_root, finalize_inactive_root, prepare_inert_root,
    };

    #[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
    struct AppOwnedSibling;

    #[test]
    fn post_commit_activation_is_the_only_path_that_enables_input_and_focus() {
        let (runtime, request) = frame_request("/");
        let mut world = World::new();
        world.insert_resource(RouteFocusState::default());

        let host = world.spawn((RouteHost::from_runtime(&runtime),)).id();
        let root = world
            .spawn((RouteFrameRoot::from_frame_request(&request),))
            .id();
        let focused_child = world.spawn_empty().id();

        world.entity_mut(host).add_child(root);
        world.entity_mut(root).add_child(focused_child);
        world
            .entity_mut(root)
            .insert(RouteFrameRootLifecycleState::new(
                RouteFrameRootLifecycleSnapshot::Preparing,
            ));
        world
            .resource_mut::<RouteFocusState>()
            .set_focused(Some(focused_child));

        let prepared = prepare_inert_root(&mut world, root, PrepareInertCapability);
        assert_eq!(
            prepared.lifecycle(),
            Some(RouteFrameRootLifecycleSnapshot::PreparedInert)
        );
        assert_rejects_all_input(prepared.interaction());
        assert!(!prepared.accepts_focus());
        assert_eq!(prepared.focus().focused(), None);
        assert_eq!(world.resource::<RouteFocusState>().focused(), None);

        let active = activate_committed_root(&mut world, root, PostCommitActivation);
        assert_eq!(
            active.lifecycle(),
            Some(RouteFrameRootLifecycleSnapshot::Active)
        );
        assert_accepts_all_input(active.interaction());
        assert!(active.accepts_focus());
        assert_eq!(active.focus().focused(), None);
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
            world.insert_resource(RouteFocusState::default());

            let host = world.spawn((RouteHost::from_runtime(&runtime),)).id();
            let root = world
                .spawn((RouteFrameRoot::from_frame_request(&request),))
                .id();
            let focused_child = world.spawn_empty().id();

            world.entity_mut(host).add_child(root);
            world.entity_mut(root).add_child(focused_child);
            world
                .entity_mut(root)
                .insert(RouteFrameRootLifecycleState::new(lifecycle))
                .insert(RouteFrameInteractionBarrier::new(true, true, true));
            world
                .resource_mut::<RouteFocusState>()
                .set_focused(Some(focused_child));

            let report = finalize_inactive_root(&mut world, root, InactiveFinalizationCapability);
            assert_eq!(report.lifecycle(), Some(lifecycle));
            assert_rejects_all_input(report.interaction());
            assert!(!report.accepts_focus());
            assert_eq!(report.focus().focused(), None);
            assert_eq!(world.resource::<RouteFocusState>().focused(), None);
        }
    }

    #[test]
    fn missing_lifecycle_markers_are_inert_and_cannot_become_active_or_keep_focus() {
        let (runtime, request) = frame_request("/");
        let mut world = World::new();
        world.insert_resource(RouteFocusState::default());

        let host = world.spawn((RouteHost::from_runtime(&runtime),)).id();
        let root = world
            .spawn((RouteFrameRoot::from_frame_request(&request),))
            .id();
        let focused_child = world.spawn_empty().id();

        world.entity_mut(host).add_child(root);
        world.entity_mut(root).add_child(focused_child);
        world
            .resource_mut::<RouteFocusState>()
            .set_focused(Some(focused_child));

        let report = activate_committed_root(&mut world, root, PostCommitActivation);
        assert_eq!(report.lifecycle(), None);
        assert_rejects_all_input(report.interaction());
        assert!(!report.accepts_focus());
        assert_eq!(report.focus().focused(), None);
        assert_eq!(world.resource::<RouteFocusState>().focused(), None);
    }

    #[test]
    fn foreign_roots_are_inert_and_cannot_become_active_or_keep_focus() {
        let (runtime, request) = frame_request("/");
        let (foreign_runtime, foreign_request) = frame_request("/");
        let mut world = World::new();
        world.insert_resource(RouteFocusState::default());

        let host = world.spawn((RouteHost::from_runtime(&runtime),)).id();
        let root = world
            .spawn((
                RouteFrameRoot::from_frame_request(&foreign_request),
                RouteFrameRootLifecycleState::new(RouteFrameRootLifecycleSnapshot::PreparedInert),
                RouteFrameInteractionBarrier::new(false, false, false),
            ))
            .id();
        let focused_child = world.spawn_empty().id();

        let _ = request;
        let _ = foreign_runtime;

        world.entity_mut(host).add_child(root);
        world.entity_mut(root).add_child(focused_child);
        world
            .resource_mut::<RouteFocusState>()
            .set_focused(Some(focused_child));

        let report = activate_committed_root(&mut world, root, PostCommitActivation);
        assert_eq!(report.lifecycle(), None);
        assert_rejects_all_input(report.interaction());
        assert!(!report.accepts_focus());
        assert_eq!(report.focus().focused(), None);
        assert_eq!(world.resource::<RouteFocusState>().focused(), None);
    }

    #[test]
    fn invalid_lifecycle_and_interaction_combinations_are_inert_and_cannot_keep_focus() {
        let (runtime, request) = frame_request("/");
        let mut world = World::new();
        world.insert_resource(RouteFocusState::default());

        let host = world.spawn((RouteHost::from_runtime(&runtime),)).id();
        let root = world
            .spawn((
                RouteFrameRoot::from_frame_request(&request),
                RouteFrameRootLifecycleState::new(RouteFrameRootLifecycleSnapshot::Hidden),
                RouteFrameInteractionBarrier::new(true, true, true),
            ))
            .id();
        let focused_child = world.spawn_empty().id();

        world.entity_mut(host).add_child(root);
        world.entity_mut(root).add_child(focused_child);
        world
            .resource_mut::<RouteFocusState>()
            .set_focused(Some(focused_child));

        let report = activate_committed_root(&mut world, root, PostCommitActivation);
        assert_eq!(
            report.lifecycle(),
            Some(RouteFrameRootLifecycleSnapshot::Hidden)
        );
        assert_rejects_all_input(report.interaction());
        assert!(!report.accepts_focus());
        assert_eq!(report.focus().focused(), None);
        assert_eq!(world.resource::<RouteFocusState>().focused(), None);
    }

    #[test]
    fn focus_outside_targeted_router_owned_subtree_and_on_app_owned_host_is_preserved() {
        let (runtime, request) = frame_request("/");
        let mut world = World::new();
        world.insert_resource(RouteFocusState::default());

        let host = world.spawn((RouteHost::from_runtime(&runtime),)).id();
        let root = world
            .spawn((
                RouteFrameRoot::from_frame_request(&request),
                RouteFrameRootLifecycleState::new(RouteFrameRootLifecycleSnapshot::Hidden),
                RouteFrameInteractionBarrier::new(true, true, true),
            ))
            .id();
        let app_owned_sibling = world.spawn((AppOwnedSibling,)).id();
        let focused_outside = world.spawn_empty().id();

        world
            .entity_mut(host)
            .add_children(&[root, app_owned_sibling]);
        world
            .entity_mut(app_owned_sibling)
            .add_child(focused_outside);
        world
            .resource_mut::<RouteFocusState>()
            .set_focused(Some(host));

        let host_report = finalize_inactive_root(&mut world, root, InactiveFinalizationCapability);
        assert_eq!(host_report.focus().focused(), Some(host));
        assert_eq!(world.resource::<RouteFocusState>().focused(), Some(host));

        world
            .resource_mut::<RouteFocusState>()
            .set_focused(Some(focused_outside));

        let outside_report =
            finalize_inactive_root(&mut world, root, InactiveFinalizationCapability);
        assert_eq!(outside_report.focus().focused(), Some(focused_outside));
        assert_eq!(
            world.resource::<RouteFocusState>().focused(),
            Some(focused_outside)
        );
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
