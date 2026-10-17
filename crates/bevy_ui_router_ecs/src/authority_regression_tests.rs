use bevy_app::App;
use bevy_camera::visibility::Visibility;
use bevy_ecs::{
    component::Component,
    hierarchy::{ChildOf, Children},
    lifecycle::HookContext,
    prelude::{Entity, Resource},
};
use bevy_picking::Pickable;
use bevy_ui::{Node, Val};
use bevy_ui_router_core::prelude::{
    FrameOrigin, NavigationFailureKind, NavigationPhase, NavigationRequestId,
    NavigationTraceEventKind, RequestStatus, RouteLocation, RouteNodeOptions, RoutePattern,
    RouteSlotKey, RouteTree, RouteViewKey,
};
use bevy_ui_router_view::{RouteViewDescriptor, RouteViewRegistry, ViewBuildEffect, ViewDriverKey};

use crate::{
    AppRouterExt, BevyUiRouterPlugin, RequestLookup, RouterConfig, RouterHandle,
    RouterHostRegistration, RouterSnapshots,
    app_ext::TestAppRouterExt,
    cleanup::CleanupAttempt,
    components::{ChildSlotLifecycle, RouterInputBarrier},
    content::{ContentColor, ContentWriteErrorKind, FrameContentWriter, SolidRect},
    driver::{EcsViewDriver, EcsViewReportSession, ViewBuildReport},
    input::RouterInputGate,
    navigation::{
        RouterNavigation, collect_navigation_for_test as collect_navigation_system,
        commit_presentation_system, drive_navigation_system, observe_authority_system,
        settle_cleanup_system,
    },
    presentation::FrameOutput,
    registry::{
        ComponentManifest, EntityLifecycle, EntityOwner, QuarantineLeaseTable, RegistryOwner,
        RouteEntityRegistry,
    },
};

#[derive(Component)]
struct AppHostSentinel;

#[derive(Component)]
struct ForeignComponent;

#[derive(Resource, Default)]
struct ForeignRemoveHooks(usize);

#[derive(Resource)]
struct CleanupReentryTarget(Entity);

#[derive(Resource, Default)]
struct RepairReparent {
    target: Option<Entity>,
    parent: Option<Entity>,
}

#[derive(Resource, Default)]
struct MountDriftArmed(bool);

struct AuthorityDriver {
    key: ViewDriverKey,
}

struct PendingCycleDriver {
    key: ViewDriverKey,
    calls: usize,
}

struct NestedAuthorityDriver {
    key: ViewDriverKey,
}

impl EcsViewDriver for NestedAuthorityDriver {
    fn key(&self) -> ViewDriverKey {
        self.key.clone()
    }

    fn apply(
        &mut self,
        effect: &ViewBuildEffect,
        content: &mut FrameContentWriter<'_>,
        report: EcsViewReportSession<'_>,
    ) -> ViewBuildReport {
        let container = content.spawn_empty().unwrap();
        let rect = content
            .spawn_solid_rect(SolidRect::new(21, 13, ContentColor::srgba8(9, 17, 33, 255)).unwrap())
            .unwrap();
        content.add_child(container, rect).unwrap();
        if effect.view() == &RouteViewKey::new("layout").unwrap() {
            content
                .declare_outlet(container, RouteSlotKey::default())
                .unwrap();
        }
        report.applied()
    }
}

impl EcsViewDriver for PendingCycleDriver {
    fn key(&self) -> ViewDriverKey {
        self.key.clone()
    }

    fn apply(
        &mut self,
        _effect: &ViewBuildEffect,
        content: &mut FrameContentWriter<'_>,
        report: EcsViewReportSession<'_>,
    ) -> ViewBuildReport {
        self.calls += 1;
        if self.calls == 1 {
            content
                .spawn_solid_rect(
                    SolidRect::new(20, 12, ContentColor::srgba8(7, 8, 9, 255)).unwrap(),
                )
                .unwrap();
            return report.applied();
        }

        let parent = content.spawn_empty().unwrap();
        let child = content.spawn_empty().unwrap();
        content.add_child(parent, child).unwrap();
        assert_eq!(
            content.add_child(child, parent).unwrap_err().kind(),
            ContentWriteErrorKind::RelationshipCycle
        );
        report.apply_failed("pending relationship cycle rejected")
    }
}

impl EcsViewDriver for AuthorityDriver {
    fn key(&self) -> ViewDriverKey {
        self.key.clone()
    }

    fn apply(
        &mut self,
        _effect: &ViewBuildEffect,
        content: &mut FrameContentWriter<'_>,
        report: EcsViewReportSession<'_>,
    ) -> ViewBuildReport {
        content
            .spawn_solid_rect(SolidRect::new(20, 12, ContentColor::srgba8(7, 8, 9, 255)).unwrap())
            .unwrap();
        report.applied()
    }
}

struct Harness {
    app: App,
    router: RouterHandle,
    host: Entity,
    sentinel_child: Entity,
}

struct Leaving {
    shell: Entity,
    output: Entity,
    entities: Vec<Entity>,
}

fn harness(label: &str) -> Harness {
    harness_with_setup(label, |_| {})
}

fn harness_with_setup(label: &str, setup: impl FnOnce(&mut App)) -> Harness {
    let mut app = App::new();
    setup(&mut app);
    app.add_plugins(BevyUiRouterPlugin::default());
    let driver = ViewDriverKey::new(format!("{label}-driver")).unwrap();
    app.register_ui_view_driver(AuthorityDriver {
        key: driver.clone(),
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
    let sentinel_child = app.world_mut().spawn(Node::default()).id();
    let host = app
        .world_mut()
        .spawn((
            Node::default(),
            RouterHostRegistration::new(router),
            AppHostSentinel,
        ))
        .id();
    app.world_mut().entity_mut(host).add_child(sentinel_child);
    observe_authority_system(app.world_mut());
    Harness {
        app,
        router,
        host,
        sentinel_child,
    }
}

fn first_push(harness: &mut Harness) {
    harness
        .app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(harness.router, RouteLocation::new("/"))
        .unwrap();
    collect_navigation_system(harness.app.world_mut());
    drive_navigation_system(harness.app.world_mut());
    commit_presentation_system(harness.app.world_mut());
    settle_cleanup_system(harness.app.world_mut());
}

fn begin_replacement(harness: &mut Harness) -> (Leaving, NavigationRequestId) {
    let leaving = {
        let navigation = harness.app.world().resource::<RouterNavigation>();
        let frame = navigation.entries[0].active.as_ref().unwrap();
        Leaving {
            shell: frame.first_frame().unwrap().shell(),
            output: frame.interactive_entities()[0],
            entities: frame.claims().map(|claim| claim.entity()).collect(),
        }
    };
    let request = harness
        .app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(
            harness.router,
            RouteLocation::new("/").with_fragment("replacement"),
        )
        .unwrap();
    collect_navigation_system(harness.app.world_mut());
    drive_navigation_system(harness.app.world_mut());
    commit_presentation_system(harness.app.world_mut());
    (leaving, request)
}

fn issue_attempt(harness: &mut Harness) -> CleanupAttempt {
    harness.app.world_mut().resource_scope(
        |world, mut navigation: bevy_ecs::change_detection::Mut<RouterNavigation>| {
            navigation.entries[0]
                .cleanup_ledger
                .as_mut()
                .unwrap()
                .attempt(world)
                .unwrap()
        },
    )
}

fn settle_attempt(harness: &mut Harness, attempt: CleanupAttempt) -> bool {
    harness.app.world_mut().resource_scope(
        |world, mut navigation: bevy_ecs::change_detection::Mut<RouterNavigation>| {
            navigation.entries[0]
                .cleanup_ledger
                .as_mut()
                .unwrap()
                .settle(world, attempt)
        },
    )
}

fn receipt_count(harness: &Harness) -> usize {
    harness.app.world().resource::<RouterNavigation>().entries[0]
        .cleanup_ledger
        .as_ref()
        .unwrap()
        .receipt_count()
}

fn install_driver_router(app: &mut App, label: &str, driver: impl EcsViewDriver) -> RouterHandle {
    let driver_key = driver.key();
    app.register_ui_view_driver(driver).unwrap();
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
        .insert(view.clone(), RouteViewDescriptor::new(driver_key))
        .unwrap();
    let router = app
        .add_ui_router(RouterConfig::new(routes, views, view))
        .unwrap();
    app.world_mut()
        .spawn((Node::default(), RouterHostRegistration::new(router)));
    router
}

fn world_entity_snapshot(app: &App) -> Vec<(Entity, Vec<bevy_ecs::component::ComponentId>)> {
    let mut snapshot = app
        .world()
        .iter_entities()
        .map(|entity| (entity.id(), entity.archetype().components().to_vec()))
        .collect::<Vec<_>>();
    snapshot.sort_by_key(|(entity, _)| entity.to_bits());
    snapshot
}

fn install_nested_authority_router(app: &mut App, label: &str) -> (RouterHandle, Entity) {
    let key = ViewDriverKey::new(format!("{label}-nested-driver")).unwrap();
    app.register_ui_view_driver(NestedAuthorityDriver { key: key.clone() })
        .unwrap();
    let mut views = RouteViewRegistry::new();
    for view in ["layout", "item", "fallback"] {
        views
            .insert(
                RouteViewKey::new(view).unwrap(),
                RouteViewDescriptor::new(key.clone()),
            )
            .unwrap();
    }
    let router = app
        .add_ui_router(RouterConfig::new(
            bevy_ui_router_core::test_support::nested_items_view_route_tree(),
            views,
            RouteViewKey::new("fallback").unwrap(),
        ))
        .unwrap();
    let host = app
        .world_mut()
        .spawn((
            Node::default(),
            RouterHostRegistration::new(router),
            AppHostSentinel,
        ))
        .id();
    (router, host)
}

#[derive(Debug, PartialEq)]
struct AppHostEvidence {
    entity: Entity,
    node: Node,
    registration: (RouterHandle, u64),
    components: Vec<bevy_ecs::component::ComponentId>,
    children: Vec<Entity>,
}

fn app_host_evidence(app: &App, host: Entity) -> AppHostEvidence {
    let host_ref = app
        .world()
        .get_entity(host)
        .expect("the exact app-owned host must remain alive");
    AppHostEvidence {
        entity: host,
        node: host_ref
            .get::<Node>()
            .expect("the exact app-owned host must retain Node")
            .clone(),
        registration: host_ref
            .get::<RouterHostRegistration>()
            .expect("the exact app-owned host must retain its registration")
            .testing_identity(),
        components: host_ref.archetype().components().to_vec(),
        children: host_ref
            .get::<Children>()
            .map_or_else(Vec::new, |children| children.iter().copied().collect()),
    }
}

fn settle_public_request(
    app: &mut App,
    router: RouterHandle,
    location: RouteLocation,
) -> NavigationRequestId {
    let request = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, location)
        .unwrap();
    for _ in 0..16 {
        app.update();
        if matches!(
            app.world().resource::<RouterSnapshots>().request(router, request),
            RequestLookup::Found(snapshot)
                if matches!(snapshot.status(), RequestStatus::Succeeded | RequestStatus::Failed(_))
        ) {
            return request;
        }
    }
    panic!("nested authority request did not settle")
}

fn assert_stable_child_slot_closure(app: &App) {
    let navigation = app.world().resource::<RouterNavigation>();
    let entry = &navigation.entries[0];
    assert!(entry.engine.snapshot().running_request().is_none());
    assert!(entry.cleanup_ledger.is_none());
    let active_slots = entry
        .active
        .as_ref()
        .into_iter()
        .flat_map(|branch| branch.child_slots())
        .map(|slot| slot.slot())
        .collect::<Vec<_>>();
    let registry = app.world().resource::<RouteEntityRegistry>();
    let gate = app.world().resource::<RouterInputGate>();
    assert!(
        registry
            .records()
            .iter()
            .all(|record| !matches!(record.owner(), RegistryOwner::Provisional(_)))
    );
    let registered_slots = registry
        .records()
        .iter()
        .filter_map(|record| match record.owner() {
            RegistryOwner::Committed(EntityOwner::ChildSlot { .. }) => {
                assert_eq!(record.lifecycle(), EntityLifecycle::Active);
                assert_eq!(
                    app.world().get::<Visibility>(record.entity()),
                    Some(&Visibility::Visible),
                );
                assert_eq!(
                    app.world().get::<Pickable>(record.entity()),
                    Some(&Pickable::IGNORE),
                );
                assert!(
                    app.world()
                        .get::<RouterInputBarrier>(record.entity())
                        .is_some_and(RouterInputBarrier::denies_all)
                );
                assert!(!gate.allows_pointer(record.entity()));
                assert!(!gate.allows_keyboard(record.entity()));
                assert!(!gate.allows_gamepad(record.entity()));
                Some(record.entity())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(registered_slots, active_slots);
}

#[test]
fn commit_gate_switch_is_same_frame_atomic() {
    crate::navigation::tests::commit_switches_registry_pickable_gate_and_core_history_in_one_system(
    );
}

#[test]
fn host_children_change_only_top_slot_membership() {
    crate::host::tests::staging_registers_and_blocks_input_before_probe_then_attaches_only_the_exact_host_edge();
}

#[test]
fn host_grant_observer_drift_rejects_after_mutation() {
    crate::navigation::tests::host_grant_observer_drift_is_rejected_by_the_real_authority_system();
}

#[test]
fn host_ui_topology_requires_all_node_ancestry() {
    crate::host::tests::rejects_non_node_host_and_cycles_and_revisions_are_monotonic();
}

#[test]
fn mount_grant_observer_drift_rejects_after_mutation() {
    crate::mount::tests::verify_mount_grant_observer_drift_contract();

    let mut app = App::new();
    app.world_mut().insert_resource(MountDriftArmed::default());
    app.world_mut()
        .register_component_hooks::<ChildSlotLifecycle>()
        .on_insert(|mut world, HookContext { entity, .. }| {
            if world
                .get_resource::<MountDriftArmed>()
                .is_some_and(|armed| armed.0)
            {
                world.commands().entity(entity).insert(Node {
                    width: Val::Percent(99.0),
                    height: Val::Percent(100.0),
                    ..Node::default()
                });
            }
        });
    app.add_plugins(BevyUiRouterPlugin::default());
    let (router, host) = install_nested_authority_router(&mut app, "grant-drift");
    app.update();
    // Publish only the parent first so the next append must consume a real MountNew boundary.
    let first = settle_public_request(&mut app, router, RouteLocation::new("/"));
    assert!(matches!(
        app.world().resource::<RouterSnapshots>().request(router, first),
        RequestLookup::Found(snapshot) if snapshot.status() == &RequestStatus::Succeeded
    ));
    let before = app
        .world()
        .resource::<RouterSnapshots>()
        .get(router)
        .unwrap()
        .clone();
    let seal_before = app
        .world()
        .resource::<RouteEntityRegistry>()
        .latest_issued_seal()
        .unwrap();
    let host_before = app_host_evidence(&app, host);

    app.world_mut().resource_mut::<MountDriftArmed>().0 = true;
    let rejected = settle_public_request(&mut app, router, RouteLocation::new("/items/2"));
    let rejected_slot = match app
        .world()
        .resource::<RouterSnapshots>()
        .request(router, rejected)
    {
        RequestLookup::Found(snapshot) => {
            assert!(matches!(
                snapshot.status(),
                RequestStatus::Failed(failure)
                    if failure.kind() == bevy_ui_router_core::prelude::NavigationFailureKind::Mount
            ));
            snapshot
                .trace()
                .iter()
                .find(|event| {
                    event.kind()
                        == bevy_ui_router_core::prelude::NavigationTraceEventKind::MountRejectedAfterMutation
                })
                .and_then(|event| event.slot())
                .expect("post-mutation rejection must retain the burned slot id")
        }
        _ => panic!("rejected request must remain retained"),
    };
    let after = app
        .world()
        .resource::<RouterSnapshots>()
        .get(router)
        .unwrap();
    assert_eq!(
        after.active().failure().map(|failure| failure.kind()),
        Some(NavigationFailureKind::Mount)
    );
    assert!(
        after
            .active()
            .frames()
            .iter()
            .all(|frame| frame.origin() == &FrameOrigin::RouterFallback)
    );
    assert_eq!(after.history(), before.history());
    assert_eq!(app_host_evidence(&app, host), host_before);
    assert!(app.world().get::<AppHostSentinel>(host).is_some());
    assert!(
        app.world()
            .resource::<RouteEntityRegistry>()
            .latest_issued_seal()
            .is_some_and(|seal| seal.stable_identity() > seal_before.stable_identity())
    );
    assert_stable_child_slot_closure(&app);

    app.world_mut().resource_mut::<MountDriftArmed>().0 = false;
    let next = settle_public_request(&mut app, router, RouteLocation::new("/items/3"));
    assert!(matches!(
        app.world().resource::<RouterSnapshots>().request(router, next),
        RequestLookup::Found(snapshot) if snapshot.status() == &RequestStatus::Succeeded
    ));
    let next_slot = app.world().resource::<RouterNavigation>().entries[0]
        .active
        .as_ref()
        .unwrap()
        .child_slots()[0]
        .slot_id();
    assert!(next_slot.raw() > rejected_slot.raw());
    assert_stable_child_slot_closure(&app);
}

#[test]
fn mount_token_replay_is_zero_write() {
    crate::mount::tests::verify_mount_artifact_replay_contract();
}

#[test]
fn slot_lifecycle_has_no_orphans() {
    crate::mount::tests::verify_child_slot_precommit_lifecycle_contract();

    let mut app = App::new();
    app.add_plugins(BevyUiRouterPlugin::default());
    let (router, _host) = install_nested_authority_router(&mut app, "slot-lifecycle");
    app.update();
    let first = settle_public_request(&mut app, router, RouteLocation::new("/items/1"));
    assert!(matches!(
        app.world().resource::<RouterSnapshots>().request(router, first),
        RequestLookup::Found(snapshot) if snapshot.status() == &RequestStatus::Succeeded
    ));
    assert_stable_child_slot_closure(&app);
    let old_slot_claim = app.world().resource::<RouterNavigation>().entries[0]
        .active
        .as_ref()
        .unwrap()
        .child_slots()[0]
        .slot_claim()
        .clone();

    let replacement = settle_public_request(&mut app, router, RouteLocation::new("/items/2"));
    assert!(matches!(
        app.world()
            .resource::<RouterSnapshots>()
            .request(router, replacement),
        RequestLookup::Found(snapshot) if snapshot.status() == &RequestStatus::Succeeded
    ));
    assert_stable_child_slot_closure(&app);
    let active_slot = app.world().resource::<RouterNavigation>().entries[0]
        .active
        .as_ref()
        .unwrap()
        .child_slots()[0]
        .slot();
    assert_eq!(active_slot, old_slot_claim.entity());
    assert!(app.world().get_entity(old_slot_claim.entity()).is_ok());
    assert!(
        app.world()
            .resource::<RouteEntityRegistry>()
            .record(old_slot_claim.entity())
            .is_some_and(|record| {
                record.owner() == old_slot_claim.owner()
                    && record.seal() == old_slot_claim.seal()
                    && record.lifecycle() == EntityLifecycle::Active
            })
    );
}

#[test]
fn pending_cycle_rejects_without_touching_active() {
    let mut app = App::new();
    app.add_plugins(BevyUiRouterPlugin::default());
    let key = ViewDriverKey::new("pending-cycle-driver").unwrap();
    let router = install_driver_router(
        &mut app,
        "pending-cycle",
        PendingCycleDriver { key, calls: 0 },
    );
    observe_authority_system(app.world_mut());
    app.world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, RouteLocation::new("/"))
        .unwrap();
    collect_navigation_system(app.world_mut());
    drive_navigation_system(app.world_mut());
    commit_presentation_system(app.world_mut());
    settle_cleanup_system(app.world_mut());

    let active_before = {
        let navigation = app.world().resource::<RouterNavigation>();
        let active = navigation.entries[0].active.as_ref().unwrap();
        (
            active.first_frame().unwrap().frame().clone(),
            active.first_frame().unwrap().shell(),
            active.interactive_entities(),
            active.claims().cloned().collect::<Vec<_>>(),
        )
    };
    let history_before = app.world().resource::<RouterNavigation>().entries[0]
        .engine
        .history()
        .clone();
    let registry_before = app
        .world()
        .resource::<RouteEntityRegistry>()
        .records()
        .iter()
        .map(|record| record.claim())
        .collect::<Vec<_>>();
    let world_before = world_entity_snapshot(&app);

    let request = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(
            router,
            RouteLocation::new("/").with_fragment("cycle-attempt"),
        )
        .unwrap();
    collect_navigation_system(app.world_mut());
    drive_navigation_system(app.world_mut());

    {
        let navigation = app.world().resource::<RouterNavigation>();
        let entry = &navigation.entries[0];
        let active = entry.active.as_ref().unwrap();
        assert_eq!(
            (
                active.first_frame().unwrap().frame().clone(),
                active.first_frame().unwrap().shell(),
                active.interactive_entities(),
                active.claims().cloned().collect::<Vec<_>>(),
            ),
            active_before
        );
        assert_eq!(entry.engine.history(), &history_before);
        assert_eq!(
            entry.engine.request(request).unwrap().status(),
            &RequestStatus::Running(bevy_ui_router_core::prelude::NavigationPhase::Recovering)
        );
        assert!(
            entry
                .engine
                .request(request)
                .unwrap()
                .trace()
                .iter()
                .any(|event| {
                    event.kind() == NavigationTraceEventKind::FrameTaskIssued
                        && event
                            .frame()
                            .is_some_and(|frame| frame.origin() == &FrameOrigin::RouterFallback)
                })
        );
        assert_eq!(
            app.world()
                .resource::<RouteEntityRegistry>()
                .records()
                .iter()
                .map(|record| record.claim())
                .collect::<Vec<_>>(),
            registry_before
        );
        assert_eq!(world_entity_snapshot(&app), world_before);
    }

    drive_navigation_system(app.world_mut());
    commit_presentation_system(app.world_mut());
    settle_cleanup_system(app.world_mut());

    let navigation = app.world().resource::<RouterNavigation>();
    let entry = &navigation.entries[0];
    let active = entry.active.as_ref().unwrap();
    assert_eq!(
        (
            active.first_frame().unwrap().frame().clone(),
            active.first_frame().unwrap().shell(),
            active.interactive_entities(),
            active.claims().cloned().collect::<Vec<_>>(),
        ),
        active_before
    );
    assert_eq!(entry.engine.history(), &history_before);
    assert!(matches!(
        entry.engine.request(request).unwrap().status(),
        RequestStatus::Failed(failure)
            if failure.kind()
                == bevy_ui_router_core::prelude::NavigationFailureKind::RecoveryFailed
    ));
    assert_eq!(
        app.world()
            .resource::<RouteEntityRegistry>()
            .records()
            .iter()
            .map(|record| record.claim())
            .collect::<Vec<_>>(),
        registry_before
    );
    assert_eq!(world_entity_snapshot(&app), world_before);
}

#[test]
fn persistent_admission_observer_latches_retry() {
    crate::navigation::tests::persistent_host_observer_latches_each_revision_to_one_attempt();
}

#[test]
fn plan_relationship_errors_are_world_zero() {
    crate::content::tests::relationship_rejections_are_zero_write_with_fixed_priority();
}

#[test]
fn solid_rect_required_components_match_locked_bevy() {
    crate::presentation::tests::post_write_required_component_removal_is_rejected_by_the_frozen_manifest();
    crate::navigation::tests::locked_bevy_plugins_produce_visible_layout_and_native_picking();
}

#[test]
fn solid_rect_runs_real_layout_and_picking_pipeline() {
    crate::navigation::tests::locked_bevy_plugins_produce_visible_layout_and_native_picking();
}

#[test]
fn stale_report_identity_is_zero_write() {
    crate::navigation::tests::production_report_application_rejects_opaque_or_foreign_identity_before_world_write();
}

#[test]
fn structural_outputs_have_locked_manifest_and_topology() {
    crate::presentation::tests::realization_is_stable_registered_and_inert();
    crate::cleanup::tests::external_frame_output_value_and_manifest_drift_is_zero_write_and_retains_focus();
}

#[test]
fn post_commit_cleanup_waits_for_receipt() {
    let mut harness = harness("wait");
    first_push(&mut harness);
    let (leaving, request) = begin_replacement(&mut harness);

    assert!(matches!(
        harness.app.world().resource::<RouterNavigation>().entries[0]
            .engine
            .request(request)
            .unwrap()
            .status(),
        RequestStatus::Running(NavigationPhase::PostCommitCleanup)
    ));
    assert!(harness.app.world().get_entity(leaving.output).is_ok());

    settle_cleanup_system(harness.app.world_mut());
    assert!(matches!(
        harness.app.world().resource::<RouterNavigation>().entries[0]
            .engine
            .request(request)
            .unwrap()
            .status(),
        RequestStatus::Succeeded
    ));
    assert!(harness.app.world().get_entity(leaving.output).is_err());
}

#[test]
fn cleanup_receipt_replay_is_rejected() {
    crate::cleanup::tests::foreign_and_replayed_receipts_are_ledger_zero();
}

#[test]
fn post_commit_cleanup_drift_supersedes_attempt() {
    let mut harness = harness("drift");
    first_push(&mut harness);
    let (leaving, _request) = begin_replacement(&mut harness);
    let attempt = issue_attempt(&mut harness);
    let foreign = harness.app.world_mut().spawn_empty().id();
    harness
        .app
        .world_mut()
        .entity_mut(leaving.shell)
        .add_child(foreign);

    assert!(!settle_attempt(&mut harness, attempt));
    assert_eq!(receipt_count(&harness), 0);
    let retry = issue_attempt(&mut harness);
    assert!(settle_attempt(&mut harness, retry));
    assert!(harness.app.world().get_entity(foreign).is_ok());
    assert!(
        harness
            .app
            .world()
            .resource::<QuarantineLeaseTable>()
            .record(leaving.shell)
            .is_some()
    );
}

#[test]
fn post_commit_host_loss_preserves_committed_history() {
    crate::navigation::tests::post_commit_host_loss_enters_single_compound_without_rolling_back_history();
}

#[test]
fn exact_owned_cleanup_cycle_quarantines_exact_set_and_releases_focus() {
    crate::navigation::tests::failed_branch_relationship_cycle_quarantines_exact_set_and_releases_focus();
}
#[test]
fn foreign_component_quarantine_avoids_hooks() {
    let mut harness = harness("foreign-component");
    harness
        .app
        .world_mut()
        .init_resource::<ForeignRemoveHooks>();
    harness
        .app
        .world_mut()
        .register_component_hooks::<ForeignComponent>()
        .on_remove(|mut world, HookContext { .. }| {
            world.resource_mut::<ForeignRemoveHooks>().0 += 1;
        });
    first_push(&mut harness);
    let (leaving, _request) = begin_replacement(&mut harness);
    harness
        .app
        .world_mut()
        .entity_mut(leaving.output)
        .insert(ForeignComponent);

    settle_cleanup_system(harness.app.world_mut());

    assert!(
        harness
            .app
            .world()
            .get::<ForeignComponent>(leaving.output)
            .is_some()
    );
    assert_eq!(harness.app.world().resource::<ForeignRemoveHooks>().0, 0);
    assert!(
        harness
            .app
            .world()
            .resource::<QuarantineLeaseTable>()
            .record(leaving.output)
            .is_some()
    );
}

#[test]
fn foreign_descendant_quarantines_owned_ancestor() {
    let mut harness = harness("foreign-child");
    first_push(&mut harness);
    let (leaving, _request) = begin_replacement(&mut harness);
    let foreign = harness.app.world_mut().spawn_empty().id();
    harness
        .app
        .world_mut()
        .entity_mut(leaving.output)
        .add_child(foreign);

    settle_cleanup_system(harness.app.world_mut());

    assert!(harness.app.world().get_entity(foreign).is_ok());
    assert!(harness.app.world().get_entity(leaving.output).is_ok());
    assert!(
        harness
            .app
            .world()
            .resource::<QuarantineLeaseTable>()
            .record(leaving.output)
            .is_some()
    );
}

#[test]
fn protected_parent_quarantine_preserves_relationship() {
    let mut harness = harness("protected-parent");
    first_push(&mut harness);
    let (leaving, _request) = begin_replacement(&mut harness);
    let protected = harness.app.world_mut().spawn(Node::default()).id();
    harness
        .app
        .world_mut()
        .entity_mut(leaving.output)
        .insert(ChildOf(protected));
    let leaving_claim = harness
        .app
        .world()
        .resource::<RouteEntityRegistry>()
        .record(leaving.output)
        .unwrap()
        .claim();
    assert_eq!(
        leaving_claim.lifecycle(),
        crate::registry::EntityLifecycle::Leaving
    );
    let leases_before = harness
        .app
        .world()
        .resource::<QuarantineLeaseTable>()
        .leases()
        .len();

    settle_cleanup_system(harness.app.world_mut());

    assert_eq!(
        harness
            .app
            .world()
            .get::<ChildOf>(leaving.output)
            .map(ChildOf::parent),
        Some(protected)
    );
    assert!(harness.app.world().get_entity(protected).is_ok());
    assert!(harness.app.world().get_entity(leaving.output).is_ok());
    assert!(
        harness
            .app
            .world()
            .resource::<RouteEntityRegistry>()
            .record(leaving.output)
            .is_none()
    );
    let quarantine = harness.app.world().resource::<QuarantineLeaseTable>();
    assert_eq!(quarantine.leases().len(), leases_before + 1);
    assert!(
        quarantine
            .leases()
            .last()
            .unwrap()
            .owned_entities()
            .contains(&leaving.output)
    );
    let quarantined = quarantine.record(leaving.output).unwrap();
    assert_eq!(quarantined.owner(), leaving_claim.owner());
    assert_eq!(quarantined.seal(), leaving_claim.seal());
    assert_eq!(
        quarantined.lifecycle(),
        crate::registry::EntityLifecycle::Quarantined
    );
    assert_eq!(
        harness
            .app
            .world()
            .get::<bevy_camera::visibility::Visibility>(leaving.output),
        Some(&bevy_camera::visibility::Visibility::Hidden)
    );
    assert_eq!(
        harness.app.world().get::<Pickable>(leaving.output),
        Some(&Pickable::IGNORE)
    );
    assert!(
        harness
            .app
            .world()
            .get::<RouterInputBarrier>(leaving.output)
            .is_some_and(RouterInputBarrier::denies_all)
    );
}

#[test]
fn protected_host_is_never_despawned() {
    let mut harness = harness("protected-host");
    first_push(&mut harness);
    let (leaving, _request) = begin_replacement(&mut harness);
    settle_cleanup_system(harness.app.world_mut());

    assert!(
        leaving
            .entities
            .iter()
            .all(|entity| harness.app.world().get_entity(*entity).is_err())
    );
    assert!(
        harness
            .app
            .world()
            .get::<AppHostSentinel>(harness.host)
            .is_some()
    );
    assert!(
        harness
            .app
            .world()
            .get_entity(harness.sentinel_child)
            .is_ok()
    );
    assert!(
        harness
            .app
            .world()
            .get::<Children>(harness.host)
            .is_some_and(|children| children.contains(&harness.sentinel_child))
    );
}

#[test]
fn cleanup_observer_reentry_supersedes_attempt() {
    let mut harness = harness("observer-reentry");
    harness
        .app
        .world_mut()
        .register_component_hooks::<FrameOutput>()
        .on_despawn(|mut world, HookContext { .. }| {
            let target = world.resource::<CleanupReentryTarget>().0;
            world.commands().entity(target).insert(ForeignComponent);
        });
    first_push(&mut harness);
    let leaving = {
        let navigation = harness.app.world().resource::<RouterNavigation>();
        let frame = navigation.entries[0].active.as_ref().unwrap();
        Leaving {
            shell: frame.first_frame().unwrap().shell(),
            output: frame.interactive_entities()[0],
            entities: frame.claims().map(|claim| claim.entity()).collect(),
        }
    };
    harness
        .app
        .world_mut()
        .insert_resource(CleanupReentryTarget(leaving.shell));
    let (_captured, _request) = begin_replacement(&mut harness);
    let attempt = issue_attempt(&mut harness);

    assert!(!settle_attempt(&mut harness, attempt));
    assert_eq!(receipt_count(&harness), 1);
    assert!(
        harness
            .app
            .world()
            .get::<ForeignComponent>(leaving.shell)
            .is_some()
    );
    let retry = issue_attempt(&mut harness);
    assert!(settle_attempt(&mut harness, retry));
    assert!(
        harness
            .app
            .world()
            .resource::<QuarantineLeaseTable>()
            .record(leaving.shell)
            .is_some()
    );
}

#[test]
fn active_cycle_enters_authority_settlement() {
    crate::navigation::tests::active_cycle_enters_authority_settlement();
}

#[test]
fn error_outlet_role_classification_is_preserved() {
    crate::navigation::tests::error_outlet_role_classification_is_preserved_fixture();
}

pub(crate) fn partial_receipt_then_missing_pickable_still_quarantines_and_terminals() {
    let mut harness = harness("partial-persistent-drift");
    first_push(&mut harness);
    let (leaving, request) = begin_replacement(&mut harness);
    let drifted = leaving
        .entities
        .iter()
        .copied()
        .find(|entity| *entity != leaving.shell && *entity != leaving.output)
        .expect("the realized frame has a content target");
    assert!(harness.app.world_mut().despawn(leaving.output));
    harness
        .app
        .world_mut()
        .entity_mut(drifted)
        .remove::<Pickable>();

    let attempt = issue_attempt(&mut harness);
    assert!(settle_attempt(&mut harness, attempt));
    assert_eq!(receipt_count(&harness), leaving.entities.len());

    settle_cleanup_system(harness.app.world_mut());

    assert!(matches!(
        harness.app.world().resource::<RouterNavigation>().entries[0]
            .engine
            .request(request)
            .unwrap()
            .status(),
        RequestStatus::Succeeded
    ));
    assert!(
        harness
            .app
            .world()
            .resource::<QuarantineLeaseTable>()
            .record(drifted)
            .is_some()
    );
    assert!(harness.app.world().get_entity(harness.host).is_ok());
    assert!(
        harness
            .app
            .world()
            .get::<AppHostSentinel>(harness.host)
            .is_some()
    );
    assert!(
        harness
            .app
            .world()
            .get_entity(harness.sentinel_child)
            .is_ok()
    );
}

pub(crate) fn pickable_repair_observer_drift_supersedes_attempt_before_quarantine_transfer() {
    let mut harness = harness_with_setup("repair-observer-drift", |app| {
        app.world_mut().insert_resource(RepairReparent::default());
        app.world_mut()
            .register_component_hooks::<Pickable>()
            .on_insert(|mut world, HookContext { entity, .. }| {
                let Some(repair) = world.get_resource::<RepairReparent>() else {
                    return;
                };
                if repair.target != Some(entity) {
                    return;
                }
                let parent = repair.parent.expect("armed repair drift has a parent");
                world.commands().entity(entity).insert(ChildOf(parent));
            });
    });
    first_push(&mut harness);
    let (leaving, request) = begin_replacement(&mut harness);
    let drifted = leaving
        .entities
        .iter()
        .copied()
        .find(|entity| *entity != leaving.shell && *entity != leaving.output)
        .expect("the realized frame has a content target");
    assert!(harness.app.world_mut().despawn(leaving.output));
    {
        let mut repair = harness.app.world_mut().resource_mut::<RepairReparent>();
        repair.target = Some(drifted);
        repair.parent = Some(harness.host);
    }
    harness
        .app
        .world_mut()
        .entity_mut(drifted)
        .remove::<Pickable>();

    let stale = issue_attempt(&mut harness);
    assert!(!settle_attempt(&mut harness, stale));
    assert_eq!(receipt_count(&harness), 1);
    assert_eq!(
        harness
            .app
            .world()
            .get::<ChildOf>(drifted)
            .map(ChildOf::parent),
        Some(harness.host)
    );
    assert!(
        harness
            .app
            .world()
            .resource::<QuarantineLeaseTable>()
            .record(drifted)
            .is_none()
    );

    let fresh = issue_attempt(&mut harness);
    assert!(settle_attempt(&mut harness, fresh));
    settle_cleanup_system(harness.app.world_mut());

    assert!(matches!(
        harness.app.world().resource::<RouterNavigation>().entries[0]
            .engine
            .request(request)
            .unwrap()
            .status(),
        RequestStatus::Succeeded
    ));
    assert!(
        harness
            .app
            .world()
            .resource::<QuarantineLeaseTable>()
            .record(drifted)
            .is_some()
    );
    assert_eq!(
        harness
            .app
            .world()
            .get::<ChildOf>(drifted)
            .map(ChildOf::parent),
        Some(harness.host)
    );
    assert!(
        harness
            .app
            .world()
            .get::<AppHostSentinel>(harness.host)
            .is_some()
    );
}

#[test]
pub(crate) fn host_reacceptance_detaches_old_top_slot() {
    crate::navigation::tests::same_host_readmission_waits_for_retired_edge_and_old_slot_disposition(
    );
}

#[test]
fn multi_router_registries_are_partitioned() {
    let mut app = App::new();
    app.add_plugins(BevyUiRouterPlugin::default());
    let (first_router, _first_host) = install_nested_authority_router(&mut app, "partition-first");
    let (second_router, second_host) =
        install_nested_authority_router(&mut app, "partition-second");
    observe_authority_system(app.world_mut());

    let first_initial =
        settle_public_request(&mut app, first_router, RouteLocation::new("/items/1"));
    let second_initial =
        settle_public_request(&mut app, second_router, RouteLocation::new("/items/1"));
    for (router, request) in [
        (first_router, first_initial),
        (second_router, second_initial),
    ] {
        assert!(matches!(
            app.world().resource::<RouterSnapshots>().request(router, request),
            RequestLookup::Found(snapshot) if snapshot.status() == &RequestStatus::Succeeded
        ));
    }

    let (first_engine_before, second_engine_before, second_claims, second_interactive) = {
        let navigation = app.world().resource::<RouterNavigation>();
        let first = navigation
            .entries
            .iter()
            .find(|entry| entry.handle == first_router)
            .expect("the first production router retains its entry");
        let second = navigation
            .entries
            .iter()
            .find(|entry| entry.handle == second_router)
            .expect("the second production router retains its entry");
        let active = second
            .active
            .as_ref()
            .expect("the second router has a committed branch");
        let mut claims = active.claims().cloned().collect::<Vec<_>>();
        claims.push(
            second
                .host
                .as_ref()
                .expect("the second router retains its exact host binding")
                .top_slot_claim
                .clone(),
        );
        claims.sort_by_key(|claim| claim.entity().to_bits());
        assert!(
            claims
                .windows(2)
                .all(|pair| pair[0].entity() != pair[1].entity()),
            "the second router must retain one exact registry claim per entity"
        );
        (
            first.engine.snapshot(),
            second.engine.snapshot(),
            claims,
            active.interactive_entities(),
        )
    };
    let second_public_snapshot = app
        .world()
        .resource::<RouterSnapshots>()
        .get(second_router)
        .expect("the second router publishes a public snapshot")
        .clone();
    let second_host_before = app_host_evidence(&app, second_host);
    let second_projection = second_claims
        .iter()
        .map(|claim| {
            let entity = claim.entity();
            (
                claim.clone(),
                ComponentManifest::from_world(app.world(), entity)
                    .expect("the second router exact claim entity remains present"),
                app.world().get::<ChildOf>(entity).map(ChildOf::parent),
                app.world().get::<Visibility>(entity).copied(),
                app.world().get::<Pickable>(entity).copied(),
                app.world().get::<RouterInputBarrier>(entity).copied(),
                app.world()
                    .get::<crate::presentation::FrameLifecycle>(entity)
                    .copied(),
            )
        })
        .collect::<Vec<_>>();
    let second_gate = {
        let gate = app.world().resource::<RouterInputGate>();
        second_interactive
            .iter()
            .map(|entity| {
                (
                    *entity,
                    gate.allows_pointer(*entity),
                    gate.allows_keyboard(*entity),
                    gate.allows_gamepad(*entity),
                )
            })
            .collect::<Vec<_>>()
    };

    let replacement = settle_public_request(&mut app, first_router, RouteLocation::new("/items/2"));
    assert!(matches!(
        app.world()
            .resource::<RouterSnapshots>()
            .request(first_router, replacement),
        RequestLookup::Found(snapshot) if snapshot.status() == &RequestStatus::Succeeded
    ));
    let navigation = app.world().resource::<RouterNavigation>();
    let first_after = navigation
        .entries
        .iter()
        .find(|entry| entry.handle == first_router)
        .expect("the first production router retains its entry after replacement");
    let second_after = navigation
        .entries
        .iter()
        .find(|entry| entry.handle == second_router)
        .expect("the second production router retains its entry after first-router settlement");
    assert_ne!(first_after.engine.snapshot(), first_engine_before);
    assert_eq!(second_after.engine.snapshot(), second_engine_before);
    assert_eq!(
        app.world().resource::<RouterSnapshots>().get(second_router),
        Some(&second_public_snapshot)
    );
    assert_eq!(app_host_evidence(&app, second_host), second_host_before);
    let registry = app.world().resource::<RouteEntityRegistry>();
    for (claim, manifest, parent, visibility, pickable, barrier, lifecycle) in second_projection {
        let entity = claim.entity();
        assert!(
            registry
                .record(entity)
                .is_some_and(|record| record.matches_claim(&claim))
        );
        assert_eq!(
            ComponentManifest::from_world(app.world(), entity),
            Some(manifest)
        );
        assert_eq!(
            app.world().get::<ChildOf>(entity).map(ChildOf::parent),
            parent
        );
        assert_eq!(app.world().get::<Visibility>(entity).copied(), visibility);
        assert_eq!(app.world().get::<Pickable>(entity).copied(), pickable);
        assert_eq!(
            app.world().get::<RouterInputBarrier>(entity).copied(),
            barrier
        );
        assert_eq!(
            app.world()
                .get::<crate::presentation::FrameLifecycle>(entity)
                .copied(),
            lifecycle
        );
    }
    let gate = app.world().resource::<RouterInputGate>();
    for (entity, pointer, keyboard, gamepad) in second_gate {
        assert_eq!(gate.allows_pointer(entity), pointer);
        assert_eq!(gate.allows_keyboard(entity), keyboard);
        assert_eq!(gate.allows_gamepad(entity), gamepad);
    }
}

#[test]
fn old_quarantine_lease_cannot_mutate_new_epoch() {
    crate::navigation::tests::old_quarantine_lease_cannot_mutate_new_epoch_fixture();
}

#[test]
fn registry_invariant_violation_is_fail_closed() {
    crate::navigation::tests::registry_invariant_violation_fail_closed_fixture();
}
