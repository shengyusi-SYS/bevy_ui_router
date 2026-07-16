use bevy_app::App;
use bevy_ecs::{
    component::Component,
    hierarchy::{ChildOf, Children},
    lifecycle::HookContext,
    prelude::{Entity, Resource},
};
use bevy_picking::Pickable;
use bevy_ui::Node;
use bevy_ui_router_core::prelude::{
    NavigationPhase, NavigationRequestId, RequestStatus, RouteLocation, RouteNodeOptions,
    RoutePattern, RouteTree, RouteViewKey,
};
use bevy_ui_router_view::{RouteViewDescriptor, RouteViewRegistry, ViewBuildEffect, ViewDriverKey};

use crate::{
    AppRouterExt, BevyUiRouterPlugin, ContentColor, EcsViewDriver, EcsViewReportSession,
    FrameContentWriter, RouterConfig, RouterHandle, RouterHostRegistration, SolidRect,
    ViewBuildReport,
    cleanup::CleanupAttempt,
    navigation::{
        RouterNavigation, collect_navigation_system, commit_presentation_system,
        drive_navigation_system, observe_authority_system, settle_cleanup_system,
    },
    presentation::FrameOutput,
    registry::{QuarantineLeaseTable, RouteEntityRegistry},
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

struct AuthorityDriver {
    key: ViewDriverKey,
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
            shell: frame.shell(),
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
    let mut harness = harness("receipt-replay");
    first_push(&mut harness);
    let (_leaving, _request) = begin_replacement(&mut harness);
    let attempt = issue_attempt(&mut harness);
    let replay = attempt.clone();

    assert!(settle_attempt(&mut harness, attempt));
    let records = harness
        .app
        .world()
        .resource::<RouteEntityRegistry>()
        .records()
        .len();
    assert!(!settle_attempt(&mut harness, replay));
    assert_eq!(
        harness
            .app
            .world()
            .resource::<RouteEntityRegistry>()
            .records()
            .len(),
        records
    );
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
            shell: frame.shell(),
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
fn partial_receipt_then_missing_pickable_still_quarantines_and_terminals() {
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

#[test]
fn pickable_repair_observer_drift_supersedes_attempt_before_quarantine_transfer() {
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
