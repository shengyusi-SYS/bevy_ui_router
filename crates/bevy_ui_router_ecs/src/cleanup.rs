use std::collections::HashSet;

use bevy_ecs::hierarchy::Children;
use bevy_ecs::prelude::{Entity, World};
use bevy_ui_router_core::frame::FrameCleanupPlan;
use bevy_ui_router_core::transaction::FrameApplyRequest;

use crate::components::RouteFrameRoot;

#[allow(dead_code)]
pub(crate) struct CleanupCapability;

#[allow(dead_code)]
#[derive(Debug, Eq, PartialEq)]
pub struct CleanupReport {
    removed: Vec<Entity>,
    skipped_mismatches: Vec<Entity>,
}

#[allow(dead_code)]
impl CleanupReport {
    pub fn removed(&self) -> &[Entity] {
        &self.removed
    }

    pub fn skipped_mismatches(&self) -> &[Entity] {
        &self.skipped_mismatches
    }
}

#[allow(dead_code)]
pub(crate) fn cleanup_router_owned_subtree(
    world: &mut World,
    plan: &FrameCleanupPlan,
    _capability: CleanupCapability,
) -> CleanupReport {
    let mut frame_roots = world
        .query::<(Entity, &RouteFrameRoot)>()
        .iter(world)
        .map(|(entity, root)| (entity, *root))
        .collect::<Vec<_>>();

    frame_roots.sort_by_key(|(entity, root)| cleanup_sort_key(root, *entity));

    let mut removed_targets = Vec::new();
    let mut skipped_mismatches = Vec::new();
    let mut direct_despawn_roots = Vec::new();
    let mut seen_removed = HashSet::new();

    for (entity, root) in frame_roots {
        if matches_cleanup_plan(&root, plan) {
            direct_despawn_roots.push((entity, root));
            for descendant in collect_router_owned_subtree(world, entity) {
                if seen_removed.insert(descendant) {
                    removed_targets.push((descendant, root));
                }
            }
        } else {
            skipped_mismatches.push((entity, root));
        }
    }

    direct_despawn_roots.sort_by_key(|(entity, root)| cleanup_sort_key(root, *entity));
    removed_targets.sort_by_key(|(entity, root)| cleanup_sort_key(root, *entity));
    skipped_mismatches.sort_by_key(|(entity, root)| cleanup_sort_key(root, *entity));

    for (entity, _root) in direct_despawn_roots {
        if let Ok(entity_mut) = world.get_entity_mut(entity) {
            entity_mut.despawn();
        }
    }

    CleanupReport {
        removed: removed_targets
            .into_iter()
            .map(|(entity, _root)| entity)
            .collect(),
        skipped_mismatches: skipped_mismatches
            .into_iter()
            .map(|(entity, _root)| entity)
            .collect(),
    }
}

#[allow(dead_code)]
pub(crate) fn cleanup_frame_apply_request_subtree(
    world: &mut World,
    request: &FrameApplyRequest,
    _capability: CleanupCapability,
) -> CleanupReport {
    let mut frame_roots = world
        .query::<(Entity, &RouteFrameRoot)>()
        .iter(world)
        .map(|(entity, root)| (entity, *root))
        .collect::<Vec<_>>();

    frame_roots.sort_by_key(|(entity, root)| cleanup_sort_key(root, *entity));

    let mut removed_targets = Vec::new();
    let mut skipped_mismatches = Vec::new();
    let mut direct_despawn_roots = Vec::new();
    let mut seen_removed = HashSet::new();

    for (entity, root) in frame_roots {
        if matches_frame_apply_request(&root, request) {
            direct_despawn_roots.push((entity, root));
            for descendant in collect_router_owned_subtree(world, entity) {
                if seen_removed.insert(descendant) {
                    removed_targets.push((descendant, root));
                }
            }
        } else {
            skipped_mismatches.push((entity, root));
        }
    }

    direct_despawn_roots.sort_by_key(|(entity, root)| cleanup_sort_key(root, *entity));
    removed_targets.sort_by_key(|(entity, root)| cleanup_sort_key(root, *entity));
    skipped_mismatches.sort_by_key(|(entity, root)| cleanup_sort_key(root, *entity));

    for (entity, _root) in direct_despawn_roots {
        if let Ok(entity_mut) = world.get_entity_mut(entity) {
            entity_mut.despawn();
        }
    }

    CleanupReport {
        removed: removed_targets
            .into_iter()
            .map(|(entity, _root)| entity)
            .collect(),
        skipped_mismatches: skipped_mismatches
            .into_iter()
            .map(|(entity, _root)| entity)
            .collect(),
    }
}

#[allow(dead_code)]
pub(crate) fn cleanup_unclaimed_frame_root_subtree(
    world: &mut World,
    root: Entity,
    _capability: CleanupCapability,
) -> CleanupReport {
    let Some(frame_root) = world.get::<RouteFrameRoot>(root).copied() else {
        return CleanupReport {
            removed: Vec::new(),
            skipped_mismatches: Vec::new(),
        };
    };

    let mut removed_targets = collect_router_owned_subtree(world, root)
        .into_iter()
        .map(|entity| (entity, frame_root))
        .collect::<Vec<_>>();
    removed_targets.sort_by_key(|(entity, frame_root)| cleanup_sort_key(frame_root, *entity));

    if let Ok(entity_mut) = world.get_entity_mut(root) {
        entity_mut.despawn();
    }

    CleanupReport {
        removed: removed_targets
            .into_iter()
            .map(|(entity, _root)| entity)
            .collect(),
        skipped_mismatches: Vec::new(),
    }
}

#[allow(dead_code)]
fn matches_cleanup_plan(root: &RouteFrameRoot, plan: &FrameCleanupPlan) -> bool {
    let identity = root.identity();

    identity.router() == plan.identity().router()
        && identity.host() == plan.identity().host()
        && identity.slot() == plan.identity().slot()
        && identity.route() == plan.identity().route()
        && identity.frame() == plan.identity().frame()
        && identity.generation() == plan.identity().generation()
}

#[allow(dead_code)]
fn matches_frame_apply_request(root: &RouteFrameRoot, request: &FrameApplyRequest) -> bool {
    let identity = root.identity();
    let request_identity = request.frame().identity();

    identity.router() == request_identity.router()
        && identity.host() == request_identity.host()
        && identity.slot() == request_identity.slot()
        && identity.route() == request_identity.route()
        && identity.frame() == request_identity.frame()
        && identity.generation() == request_identity.generation()
}

#[allow(dead_code)]
fn cleanup_sort_key(
    root: &RouteFrameRoot,
    entity: Entity,
) -> (
    u64,
    u64,
    u64,
    u64,
    u64,
    u64,
    usize,
    bevy_ecs::entity::EntityIndex,
) {
    (
        root.identity().router().raw(),
        root.identity().host().raw(),
        root.identity().slot().raw(),
        root.identity().route().raw(),
        root.identity().frame().raw(),
        root.identity().generation().raw(),
        root.branch_index(),
        entity.index(),
    )
}

#[allow(dead_code)]
fn collect_router_owned_subtree(world: &World, root_entity: Entity) -> Vec<Entity> {
    let mut subtree = Vec::new();
    let mut stack = vec![root_entity];

    while let Some(entity) = stack.pop() {
        subtree.push(entity);

        if let Some(children) = world.get::<Children>(entity) {
            let mut ordered_children = children.iter().copied().collect::<Vec<_>>();
            ordered_children.sort_by_key(|entity| entity.index());
            stack.extend(ordered_children.into_iter().rev());
        }
    }

    subtree
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use bevy_ecs::hierarchy::Children;
    use bevy_ecs::prelude::World;
    use bevy_ui_router_core::error::{RouteError, RouteErrorKind, RouteErrorSourceCategory};
    use bevy_ui_router_core::frame::FrameCleanupPlan;
    use bevy_ui_router_core::location::RouteLocation;
    use bevy_ui_router_core::matcher::RouteMatcher;
    use bevy_ui_router_core::runtime::RouterRuntime;
    use bevy_ui_router_core::test_support::{
        frame_apply_request_with_next_generation, nested_items_route_tree,
    };
    use bevy_ui_router_core::transaction::FrameApplyOutcome;

    use crate::components::{RouteFrameRoot, RouteHost};

    use super::{CleanupCapability, cleanup_router_owned_subtree, cleanup_sort_key};

    #[derive(bevy_ecs::prelude::Component, Clone, Copy, Debug, Eq, PartialEq)]
    struct AppOwnedSibling;

    #[test]
    fn cleanup_removes_only_matching_subtree_and_preserves_host_and_siblings() {
        let (runtime, request_roots, cleanup_plan) = failure_fixture("/items/42", 1);
        let (_foreign_runtime, foreign_requests, _foreign_plan) = failure_fixture("/", 0);
        let mut world = World::new();

        let host = world.spawn((RouteHost::from_runtime(&runtime),)).id();
        let app_owned_sibling = world.spawn((AppOwnedSibling,)).id();
        let same_router_other_frame = world
            .spawn((RouteFrameRoot::from_frame_request(&request_roots[0]),))
            .id();
        let matching_root = world
            .spawn((RouteFrameRoot::from_frame_request(&request_roots[1]),))
            .id();
        let matching_child_a = world.spawn_empty().id();
        let matching_child_b = world.spawn_empty().id();
        let matching_grandchild = world.spawn_empty().id();
        let foreign_router_root = world
            .spawn((RouteFrameRoot::from_frame_request(&foreign_requests[0]),))
            .id();

        world.entity_mut(host).add_children(&[
            app_owned_sibling,
            same_router_other_frame,
            matching_root,
            foreign_router_root,
        ]);
        world
            .entity_mut(matching_root)
            .add_children(&[matching_child_a, matching_child_b]);
        world
            .entity_mut(matching_child_b)
            .add_child(matching_grandchild);

        let report = cleanup_router_owned_subtree(&mut world, &cleanup_plan, CleanupCapability);

        assert!(world.get_entity(host).is_ok(), "app-owned host must remain");
        assert!(
            world.get_entity(app_owned_sibling).is_ok(),
            "app-owned sibling child must remain"
        );
        assert!(
            world.get_entity(same_router_other_frame).is_ok(),
            "different frame root under the same host must remain"
        );
        assert!(
            world.get_entity(foreign_router_root).is_ok(),
            "foreign router frame root must remain"
        );
        assert!(
            world.get_entity(matching_root).is_err()
                && world.get_entity(matching_child_a).is_err()
                && world.get_entity(matching_child_b).is_err()
                && world.get_entity(matching_grandchild).is_err(),
            "matching router-owned subtree must be despawned"
        );

        let host_children = world
            .get::<Children>(host)
            .expect("host should still track surviving children");
        assert_eq!(
            host_children.iter().copied().collect::<Vec<_>>(),
            vec![
                app_owned_sibling,
                same_router_other_frame,
                foreign_router_root
            ]
        );

        let mut expected_removed = vec![
            matching_root,
            matching_child_a,
            matching_child_b,
            matching_grandchild,
        ];
        let matching_root_component = RouteFrameRoot::from_frame_request(&request_roots[1]);
        expected_removed.sort_by_key(|entity| cleanup_sort_key(&matching_root_component, *entity));
        assert_eq!(report.removed(), expected_removed.as_slice());

        let mut expected_skipped = vec![
            (
                same_router_other_frame,
                RouteFrameRoot::from_frame_request(&request_roots[0]),
            ),
            (
                foreign_router_root,
                RouteFrameRoot::from_frame_request(&foreign_requests[0]),
            ),
        ];
        expected_skipped.sort_by_key(|(entity, root)| cleanup_sort_key(root, *entity));
        assert_eq!(
            report.skipped_mismatches(),
            expected_skipped
                .into_iter()
                .map(|(entity, _root)| entity)
                .collect::<Vec<_>>()
                .as_slice()
        );
    }

    #[test]
    fn cleanup_skips_same_frame_with_different_generation_from_core_test_support() {
        let (runtime, request_roots, cleanup_plan) = failure_fixture("/items/42", 1);
        let mut world = World::new();

        let host = world.spawn((RouteHost::from_runtime(&runtime),)).id();
        let matching_root = world
            .spawn((RouteFrameRoot::from_frame_request(&request_roots[1]),))
            .id();
        let next_generation_request = frame_apply_request_with_next_generation(&request_roots[1]);
        let next_generation_root = world
            .spawn((RouteFrameRoot::from_frame_request(&next_generation_request),))
            .id();

        world
            .entity_mut(host)
            .add_children(&[next_generation_root, matching_root]);

        let report = cleanup_router_owned_subtree(&mut world, &cleanup_plan, CleanupCapability);

        assert!(world.get_entity(host).is_ok(), "app-owned host must remain");
        assert!(
            world.get_entity(matching_root).is_err(),
            "matching generation should be removed"
        );
        assert!(
            world.get_entity(next_generation_root).is_ok(),
            "same frame id with different generation must survive cleanup"
        );
        assert_eq!(report.removed(), &[matching_root]);
        assert_eq!(report.skipped_mismatches(), &[next_generation_root]);
    }

    #[test]
    fn cleanup_orders_removed_and_skipped_reports_deterministically_at_runtime() {
        let (runtime, request_roots, cleanup_plan) = failure_fixture("/items/42", 1);
        let (_foreign_runtime, foreign_requests, _foreign_plan) = failure_fixture("/", 0);
        let mut world = World::new();

        let host = world.spawn((RouteHost::from_runtime(&runtime),)).id();
        let matching_root_low = world
            .spawn((RouteFrameRoot::from_frame_request(&request_roots[1]),))
            .id();
        let matching_root_high = world
            .spawn((RouteFrameRoot::from_frame_request(&request_roots[1]),))
            .id();
        let mismatch_other_frame = world
            .spawn((RouteFrameRoot::from_frame_request(&request_roots[0]),))
            .id();
        let mismatch_foreign_router = world
            .spawn((RouteFrameRoot::from_frame_request(&foreign_requests[0]),))
            .id();
        let mismatch_next_generation_request =
            frame_apply_request_with_next_generation(&request_roots[1]);
        let mismatch_next_generation = world
            .spawn((RouteFrameRoot::from_frame_request(
                &mismatch_next_generation_request,
            ),))
            .id();

        let matching_low_child_low = world.spawn_empty().id();
        let matching_low_child_high = world.spawn_empty().id();
        let matching_low_grandchild = world.spawn_empty().id();
        let matching_high_child_low = world.spawn_empty().id();
        let matching_high_child_high = world.spawn_empty().id();

        world.entity_mut(host).add_children(&[
            mismatch_next_generation,
            mismatch_foreign_router,
            matching_root_high,
            mismatch_other_frame,
            matching_root_low,
        ]);
        world
            .entity_mut(matching_root_low)
            .add_children(&[matching_low_child_high, matching_low_child_low]);
        world
            .entity_mut(matching_low_child_high)
            .add_child(matching_low_grandchild);
        world
            .entity_mut(matching_root_high)
            .add_children(&[matching_high_child_high, matching_high_child_low]);

        let report = cleanup_router_owned_subtree(&mut world, &cleanup_plan, CleanupCapability);

        let matching_identity = RouteFrameRoot::from_frame_request(&request_roots[1]);
        let mut expected_removed = vec![
            matching_root_low,
            matching_low_child_low,
            matching_low_child_high,
            matching_low_grandchild,
            matching_root_high,
            matching_high_child_low,
            matching_high_child_high,
        ];
        expected_removed.sort_by_key(|entity| cleanup_sort_key(&matching_identity, *entity));
        assert_eq!(report.removed(), expected_removed.as_slice());

        let mut expected_skipped = vec![
            (
                mismatch_next_generation,
                RouteFrameRoot::from_frame_request(&mismatch_next_generation_request),
            ),
            (
                mismatch_foreign_router,
                RouteFrameRoot::from_frame_request(&foreign_requests[0]),
            ),
            (
                mismatch_other_frame,
                RouteFrameRoot::from_frame_request(&request_roots[0]),
            ),
        ];
        expected_skipped.sort_by_key(|(entity, root)| cleanup_sort_key(root, *entity));
        assert_eq!(
            report.skipped_mismatches(),
            expected_skipped
                .iter()
                .map(|(entity, _root)| *entity)
                .collect::<Vec<_>>()
                .as_slice()
        );
    }

    #[test]
    fn cleanup_source_commits_to_full_identity_checks_and_stable_ordering_key() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let source = fs::read_to_string(manifest_dir.join("src").join("cleanup.rs"))
            .expect("cleanup source should be readable");

        for fragment in [
            "pub fn removed(&self) -> &[Entity]",
            "pub fn skipped_mismatches(&self) -> &[Entity]",
            "identity.router() == plan.identity().router()",
            "identity.host() == plan.identity().host()",
            "identity.slot() == plan.identity().slot()",
            "identity.route() == plan.identity().route()",
            "identity.frame() == plan.identity().frame()",
            "identity.generation() == plan.identity().generation()",
            "root.identity().router().raw()",
            "root.identity().host().raw()",
            "root.identity().slot().raw()",
            "root.identity().route().raw()",
            "root.identity().frame().raw()",
            "root.identity().generation().raw()",
            "root.branch_index()",
            "entity.index()",
        ] {
            assert!(
                source.contains(fragment),
                "cleanup implementation should contain `{fragment}`"
            );
        }
    }

    fn failure_fixture(
        path: &str,
        failing_request_index: usize,
    ) -> (
        RouterRuntime,
        Vec<bevy_ui_router_core::transaction::FrameApplyRequest>,
        FrameCleanupPlan,
    ) {
        let matcher = route_matcher();
        let target = matcher.match_location(&RouteLocation::new(path)).unwrap();
        let mut runtime = RouterRuntime::new();
        let snapshot = runtime.begin_first_navigation(&target).unwrap();
        let requests = snapshot.requests().to_vec();

        for request in &requests[..failing_request_index] {
            runtime
                .report_frame_apply(request.token(), FrameApplyOutcome::Applied)
                .unwrap();
        }

        let failure_snapshot = runtime
            .report_frame_apply(
                requests[failing_request_index].token(),
                FrameApplyOutcome::Failed(RouteError::new(
                    RouteErrorKind::ViewApplyFailed,
                    RouteErrorSourceCategory::RendererContract,
                    "cleanup fixture apply failed",
                )),
            )
            .unwrap();
        let cleanup = failure_snapshot
            .failure()
            .expect("failure snapshot should record cleanup")
            .cleanup()
            .to_vec();

        (
            runtime,
            requests,
            cleanup
                .last()
                .copied()
                .expect("failure fixture should produce at least one cleanup plan"),
        )
    }

    fn route_matcher() -> RouteMatcher {
        RouteMatcher::new(nested_items_route_tree())
    }
}
