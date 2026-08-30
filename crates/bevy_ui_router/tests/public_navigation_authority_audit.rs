use std::{
    collections::HashSet,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

use bevy_app::{App, Update};
use bevy_camera::visibility::Visibility;
use bevy_ecs::{
    component::{Component, ComponentId},
    entity::Entity,
    hierarchy::{ChildOf, Children},
    prelude::Resource,
    schedule::IntoScheduleConfigs,
    world::World,
};
use bevy_picking::Pickable;
use bevy_ui::{Node, Val};
use bevy_ui_router::core::DiagnosticEventKind;
use bevy_ui_router::ecs::RouterScheduleSet;
use bevy_ui_router::prelude::*;
use bevy_ui_router_ecs::test_support::{
    BackCapacityOrderEvent, BackCapacityOrderInstallError, TestingDelayedDriverReportReplay,
    testing_arm_next_applied_driver_report_replay, testing_back_request_inventory,
    testing_install_back_capacity_order_observer, testing_release_applied_driver_report_replay,
    testing_router_request_inventory,
};

const ROOT_RECT_WIDTH: u16 = 37;
const LAYOUT_RECT_WIDTH: u16 = 39;
const LEAF_RECT_WIDTH: u16 = 41;

struct AuditDriver {
    key: ViewDriverKey,
    outlet_view: Option<RouteViewKey>,
}

struct RecoveryR3ReplayAuditDriver {
    key: ViewDriverKey,
    outlet_view: RouteViewKey,
    effects: Arc<AtomicUsize>,
}

#[derive(Component, Debug, Eq, PartialEq)]
struct RecoveryAuditHostSentinel(u64);

#[derive(Component, Debug, Eq, PartialEq)]
struct RecoveryAuditChildSentinel(u64);

#[derive(Resource)]
struct RecoveryR3AuditLossInjection {
    host: Entity,
    fired: bool,
}

fn inject_recovery_r3_audit_registration_loss(world: &mut World) {
    let (host, fired) = {
        let injection = world.resource::<RecoveryR3AuditLossInjection>();
        (injection.host, injection.fired)
    };
    if fired {
        return;
    }
    assert!(
        world
            .entity_mut(host)
            .take::<RouterHostRegistration>()
            .is_some(),
        "the audit seam removes only the real registration input fact"
    );
    world.resource_mut::<RecoveryR3AuditLossInjection>().fired = true;
}

struct RecoveryAuditDriver {
    key: ViewDriverKey,
    failed_view: Option<RouteViewKey>,
    error_view: RouteViewKey,
    fallback_view: RouteViewKey,
}

#[derive(Resource)]
struct RecoveryR2AuditLoss {
    router: RouterHandle,
    host: bevy_ecs::entity::Entity,
    armed: bool,
    fired: bool,
}

#[derive(Resource, Default)]
struct RecoveryR2AuditCommitBoundary {
    leaving: Vec<AuditEntityId>,
    authority_non_top: Vec<AuditEntityId>,
    top_slot: Option<AuditEntityId>,
}

fn capture_recovery_r2_audit_commit_boundary(world: &mut World) {
    let (router, armed, fired) = {
        let loss = world.resource::<RecoveryR2AuditLoss>();
        (loss.router, loss.armed, loss.fired)
    };
    if !armed || fired {
        return;
    }
    let audit = world
        .resource::<RouterAuthorityAudits>()
        .get(router)
        .expect("the commit boundary publishes one exact authority audit");
    for frame in audit.frames() {
        match frame.lifecycle() {
            AuthorityFrameLifecycle::LeavingInert | AuthorityFrameLifecycle::Active => {}
            AuthorityFrameLifecycle::PreparedInert | AuthorityFrameLifecycle::Quarantined => {
                panic!("the post-publication boundary contains only L and active A frames")
            }
        }
    }
    let leaving =
        recovery_r3_frame_observable_entities(audit, AuthorityFrameLifecycle::LeavingInert);
    let authority_non_top =
        recovery_r3_frame_observable_entities(audit, AuthorityFrameLifecycle::Active);
    let top_slot = audit.host().map(|host| host.top_slot());
    *world.resource_mut::<RecoveryR2AuditCommitBoundary>() = RecoveryR2AuditCommitBoundary {
        leaving,
        authority_non_top,
        top_slot,
    };
}

fn inject_recovery_r2_audit_registration_loss(world: &mut World) {
    let (host, armed, fired) = {
        let loss = world.resource::<RecoveryR2AuditLoss>();
        (loss.host, loss.armed, loss.fired)
    };
    if !armed || fired {
        return;
    }
    assert!(
        world
            .entity_mut(host)
            .take::<RouterHostRegistration>()
            .is_some(),
        "the authority audit seam removes only the real registration fact"
    );
    world.resource_mut::<RecoveryR2AuditLoss>().fired = true;
}

impl EcsViewDriver for RecoveryAuditDriver {
    fn key(&self) -> ViewDriverKey {
        self.key.clone()
    }

    fn apply(
        &mut self,
        effect: &ViewBuildEffect,
        content: &mut FrameContentWriter<'_>,
        report: EcsViewReportSession<'_>,
    ) -> ViewBuildReport {
        let width = if effect.view() == &self.error_view {
            67
        } else if effect.view() == &self.fallback_view {
            71
        } else {
            61
        };
        content
            .spawn_solid_rect(
                SolidRect::new(width, 21, ContentColor::srgba8(31, 47, 83, 255)).unwrap(),
            )
            .unwrap();
        if self.failed_view.as_ref() == Some(effect.view()) {
            report.apply_failed("recovery audit normal attempt fails after partial output")
        } else {
            report.applied()
        }
    }
}

fn audit_component_manifest(app: &App, entity: bevy_ecs::entity::Entity) -> Vec<ComponentId> {
    app.world()
        .get_entity(entity)
        .expect("audited app-owned Entity remains present")
        .archetype()
        .components()
        .to_vec()
}

impl EcsViewDriver for AuditDriver {
    fn key(&self) -> ViewDriverKey {
        self.key.clone()
    }

    fn apply(
        &mut self,
        effect: &ViewBuildEffect,
        content: &mut FrameContentWriter<'_>,
        report: EcsViewReportSession<'_>,
    ) -> ViewBuildReport {
        let width = match self.outlet_view.as_ref() {
            Some(layout) if layout == effect.view() => LAYOUT_RECT_WIDTH,
            Some(_) => LEAF_RECT_WIDTH,
            None => ROOT_RECT_WIDTH,
        };
        let root = content.spawn_empty().expect("root content should stage");
        let rect = content
            .spawn_solid_rect(
                SolidRect::new(width, 19, ContentColor::srgba8(11, 23, 47, 255)).unwrap(),
            )
            .expect("solid rectangle should stage");
        content
            .add_child(root, rect)
            .expect("content edge should stage");
        if self.outlet_view.as_ref() == Some(effect.view()) {
            content
                .declare_outlet(root, RouteSlotKey::default())
                .expect("layout outlet should stage");
        }
        report.applied()
    }
}

impl EcsViewDriver for RecoveryR3ReplayAuditDriver {
    fn key(&self) -> ViewDriverKey {
        self.key.clone()
    }

    fn apply(
        &mut self,
        effect: &ViewBuildEffect,
        content: &mut FrameContentWriter<'_>,
        report: EcsViewReportSession<'_>,
    ) -> ViewBuildReport {
        self.effects.fetch_add(1, Ordering::SeqCst);
        let root = content.spawn_empty().expect("R3 audit root stages");
        let rect = content
            .spawn_solid_rect(
                SolidRect::new(
                    if effect.view() == &self.outlet_view {
                        LAYOUT_RECT_WIDTH
                    } else {
                        LEAF_RECT_WIDTH
                    },
                    19,
                    ContentColor::srgba8(11, 23, 47, 255),
                )
                .unwrap(),
            )
            .expect("R3 audit rectangle stages");
        content
            .add_child(root, rect)
            .expect("R3 audit content edge stages");
        if effect.view() == &self.outlet_view {
            content
                .declare_outlet(root, RouteSlotKey::default())
                .expect("R3 audit layout outlet stages");
        }
        report.applied()
    }
}

fn push_and_publish(
    app: &mut App,
    router: RouterHandle,
    location: RouteLocation,
) -> NavigationRequestId {
    let request = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, location)
        .expect("public audit navigation should enqueue");
    for _ in 0..16 {
        app.update();
        match app
            .world()
            .resource::<RouterSnapshots>()
            .request(router, request)
        {
            RequestLookup::Found(snapshot) if snapshot.status() == &RequestStatus::Succeeded => {
                return request;
            }
            RequestLookup::Found(snapshot)
                if matches!(snapshot.status(), RequestStatus::Failed(_)) =>
            {
                panic!("public audit navigation failed: {:?}", snapshot.status());
            }
            RequestLookup::Found(_) | RequestLookup::NotRetained | RequestLookup::Unknown => {}
        }
    }
    panic!("public audit navigation did not settle within 16 App::update calls")
}

fn replace_and_publish(
    app: &mut App,
    router: RouterHandle,
    location: RouteLocation,
) -> NavigationRequestId {
    let request = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .replace(router, location)
        .expect("public audit replace should enqueue");
    for _ in 0..16 {
        app.update();
        match app
            .world()
            .resource::<RouterSnapshots>()
            .request(router, request)
        {
            RequestLookup::Found(snapshot) if snapshot.status() == &RequestStatus::Succeeded => {
                return request;
            }
            RequestLookup::Found(snapshot)
                if matches!(snapshot.status(), RequestStatus::Failed(_)) =>
            {
                panic!("public audit replace failed: {:?}", snapshot.status());
            }
            RequestLookup::Found(_) | RequestLookup::NotRetained | RequestLookup::Unknown => {}
        }
    }
    panic!("public audit replace did not settle within 16 App::update calls")
}

fn drive_recovery_r3_audit_availability(
    app: &mut App,
    router: RouterHandle,
    expected: &RouterAvailability,
) -> RouterSnapshot {
    for _ in 0..24 {
        app.update();
        let snapshot = app
            .world()
            .resource::<RouterSnapshots>()
            .get(router)
            .expect("R3 audit router retains a public snapshot")
            .clone();
        if snapshot.availability() == expected
            && snapshot.running_request().is_none()
            && snapshot.pending_phase().is_none()
            && !snapshot.cleanup().foreground_pending()
        {
            return snapshot;
        }
    }
    panic!(
        "R3 audit router did not reach stable {expected:?}: {:?}",
        app.world().resource::<RouterSnapshots>().get(router)
    )
}

fn arm_recovery_r3_audit_replay(
    app: &mut App,
    router: RouterHandle,
) -> TestingDelayedDriverReportReplay {
    testing_arm_next_applied_driver_report_replay(
        &mut app.world_mut().resource_mut::<RouterNavigation>(),
        router,
    )
    .expect("the R3 audit arms one real applied-report delivery")
}

fn release_recovery_r3_audit_replay(app: &mut App, control: TestingDelayedDriverReportReplay) {
    testing_release_applied_driver_report_replay(
        &mut app.world_mut().resource_mut::<RouterNavigation>(),
        control,
    )
    .expect("the R3 audit releases its real delayed report artifact")
}

fn recovery_r3_prepared_observable_entities(
    audit: &RouterAuthorityAuditSnapshot,
) -> Vec<AuditEntityId> {
    recovery_r3_frame_observable_entities(audit, AuthorityFrameLifecycle::PreparedInert)
}

fn recovery_r3_frame_observable_entities(
    audit: &RouterAuthorityAuditSnapshot,
    lifecycle: AuthorityFrameLifecycle,
) -> Vec<AuditEntityId> {
    let selected_frames = audit
        .frames()
        .iter()
        .filter(|mapping| mapping.lifecycle() == lifecycle)
        .map(|mapping| mapping.frame().clone())
        .collect::<Vec<_>>();
    let mut entities = audit
        .frames()
        .iter()
        .filter(|mapping| mapping.lifecycle() == lifecycle)
        .map(|mapping| mapping.root())
        .collect::<Vec<_>>();
    entities.extend(audit.slots().iter().filter_map(|slot| {
        slot.parent_frame()
            .is_some_and(|parent| selected_frames.contains(parent))
            .then_some(slot.entity())
    }));
    entities.extend(
        audit
            .outlets()
            .iter()
            .filter(|outlet| selected_frames.contains(outlet.parent()))
            .flat_map(|outlet| [Some(outlet.anchor()), outlet.child_slot()])
            .flatten(),
    );
    entities.extend(
        audit
            .content()
            .iter()
            .filter(|content| selected_frames.contains(content.frame()))
            .map(|content| content.entity()),
    );
    let mut seen = HashSet::new();
    entities.retain(|entity| seen.insert(*entity));
    entities
}

fn recovery_r3_current_observable_entities(
    audit: &RouterAuthorityAuditSnapshot,
) -> Vec<AuditEntityId> {
    let current_host = audit.host().map(|host| host.version().clone());
    let active_frames = audit
        .frames()
        .iter()
        .filter(|mapping| mapping.lifecycle() == AuthorityFrameLifecycle::Active)
        .map(|mapping| mapping.frame().clone())
        .collect::<Vec<_>>();
    let mut entities = audit
        .host()
        .map(|host| vec![host.top_slot()])
        .unwrap_or_default();
    entities.extend(
        audit
            .slots()
            .iter()
            .filter(|slot| {
                current_host
                    .as_ref()
                    .is_some_and(|host| slot.host() == host)
            })
            .map(|slot| slot.entity()),
    );
    entities.extend(
        audit
            .frames()
            .iter()
            .filter(|mapping| mapping.lifecycle() == AuthorityFrameLifecycle::Active)
            .map(|mapping| mapping.root()),
    );
    entities.extend(
        audit
            .outlets()
            .iter()
            .filter(|outlet| active_frames.contains(outlet.parent()))
            .map(|outlet| outlet.anchor()),
    );
    entities.extend(
        audit
            .content()
            .iter()
            .filter(|content| active_frames.contains(content.frame()))
            .map(|content| content.entity()),
    );
    let mut seen = HashSet::new();
    entities.retain(|entity| seen.insert(*entity));
    entities
}

fn assert_recovery_r3_observable_mapping_unchanged(
    before: &RouterAuthorityAuditSnapshot,
    after: &RouterAuthorityAuditSnapshot,
) {
    assert_eq!(after.host(), before.host());
    let current_host = before.host().map(|host| host.version());
    let current_frames = before
        .frames()
        .iter()
        .filter(|mapping| mapping.lifecycle() == AuthorityFrameLifecycle::Active)
        .map(|mapping| mapping.frame())
        .collect::<Vec<_>>();
    let before_slots = before
        .slots()
        .iter()
        .filter(|slot| current_host.is_some_and(|host| slot.host() == host))
        .collect::<Vec<_>>();
    let after_slots = after
        .slots()
        .iter()
        .filter(|slot| current_host.is_some_and(|host| slot.host() == host))
        .collect::<Vec<_>>();
    assert_eq!(after_slots, before_slots);
    assert_eq!(
        after
            .frames()
            .iter()
            .filter(|mapping| mapping.lifecycle() == AuthorityFrameLifecycle::Active)
            .collect::<Vec<_>>(),
        before
            .frames()
            .iter()
            .filter(|mapping| mapping.lifecycle() == AuthorityFrameLifecycle::Active)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        after
            .outlets()
            .iter()
            .filter(|outlet| current_frames.contains(&outlet.parent()))
            .collect::<Vec<_>>(),
        before
            .outlets()
            .iter()
            .filter(|outlet| current_frames.contains(&outlet.parent()))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        after
            .content()
            .iter()
            .filter(|content| current_frames.contains(&content.frame()))
            .collect::<Vec<_>>(),
        before
            .content()
            .iter()
            .filter(|content| current_frames.contains(&content.frame()))
            .collect::<Vec<_>>()
    );
    assert!(
        after
            .orphan_slots()
            .iter()
            .all(|slot| !after_slots.iter().any(|mapping| mapping.id() == *slot))
    );
}

fn assert_recovery_r3_audit_facade_exact(before: &RouterSnapshot, after: &RouterSnapshot) {
    assert_eq!(after.requested(), before.requested());
    assert_eq!(after.matched(), before.matched());
    assert_eq!(after.active(), before.active());
    assert_eq!(after.availability(), before.availability());
    assert_eq!(after.pending_phase(), before.pending_phase());
    assert_eq!(after.history(), before.history());
    assert_eq!(after.running_request(), before.running_request());
    assert_eq!(
        after.earliest_retained_sequence(),
        before.earliest_retained_sequence()
    );
    assert_eq!(after.cleanup(), before.cleanup());
    assert_eq!(after.host(), before.host());
}

fn back_and_publish(app: &mut App, router: RouterHandle) -> NavigationRequestId {
    let request = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .back(router)
        .expect("public audit Back should enqueue");
    for _ in 0..16 {
        app.update();
        match app
            .world()
            .resource::<RouterSnapshots>()
            .request(router, request)
        {
            RequestLookup::Found(snapshot) if snapshot.status() == &RequestStatus::Succeeded => {
                return request;
            }
            RequestLookup::Found(snapshot)
                if matches!(snapshot.status(), RequestStatus::Failed(_)) =>
            {
                panic!("public audit Back failed: {:?}", snapshot.status());
            }
            RequestLookup::Found(_) | RequestLookup::NotRetained | RequestLookup::Unknown => {}
        }
    }
    panic!("public audit Back did not settle within 16 App::update calls")
}

fn install_history_audit_router(app: &mut App, label: &str) -> RouterHandle {
    let driver = app
        .register_ui_view_driver(AuditDriver {
            key: ViewDriverKey::new(format!("{label}-driver")).unwrap(),
            outlet_view: None,
        })
        .unwrap();
    let view = RouteViewKey::new(format!("{label}-view")).unwrap();
    let routes = RouteTree::builder()
        .root_with_options(
            RoutePattern::path("/items/:id").unwrap(),
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
    router
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Boundary {
    BeforeCommit,
    AfterCommit,
}

#[derive(Default)]
struct BoundaryProbe {
    armed: AtomicBool,
    samples: Mutex<Vec<(Boundary, usize)>>,
}

fn interactive_leaf_count(world: &World) -> usize {
    let gate = world.resource::<RouterInputGate>();
    world
        .iter_entities()
        .filter(|entity| {
            entity
                .get::<Node>()
                .is_some_and(|node| node.width == Val::Px(f32::from(LEAF_RECT_WIDTH)))
                && entity.get::<Visibility>() != Some(&Visibility::Hidden)
                && entity
                    .get::<Pickable>()
                    .is_some_and(|pickable| pickable.should_block_lower && pickable.is_hoverable)
                && gate.allows_pointer(entity.id())
                && gate.allows_keyboard(entity.id())
                && gate.allows_gamepad(entity.id())
        })
        .count()
}

fn install_boundary_probe(app: &mut App, probe: Arc<BoundaryProbe>) {
    let before = probe.clone();
    app.add_systems(
        Update,
        (move |world: &mut World| {
            if before.armed.load(Ordering::Acquire) {
                before
                    .samples
                    .lock()
                    .expect("boundary sample lock must not be poisoned")
                    .push((Boundary::BeforeCommit, interactive_leaf_count(world)));
            }
        })
        .after(RouterScheduleSet::DriveNavigation)
        .before(RouterScheduleSet::CommitPresentation),
    );
    app.add_systems(
        Update,
        (move |world: &mut World| {
            if probe.armed.load(Ordering::Acquire) {
                probe
                    .samples
                    .lock()
                    .expect("boundary sample lock must not be poisoned")
                    .push((Boundary::AfterCommit, interactive_leaf_count(world)));
            }
        })
        .after(RouterScheduleSet::CommitPresentation)
        .before(RouterScheduleSet::SettleCleanup),
    );
}

#[test]
fn back_capacity_order_observer_installs_across_crates_read_only() {
    fn install_router(app: &mut App, label: &str) -> RouterHandle {
        let driver = app
            .register_ui_view_driver(AuditDriver {
                key: ViewDriverKey::new(format!("{label}-driver")).unwrap(),
                outlet_view: None,
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
        app.add_ui_router(RouterConfig::new(routes, views, view))
            .unwrap()
    }

    let mut app = App::new();
    app.add_plugins(BevyUiRouterPlugin::default());
    let router_a = install_router(&mut app, "back-capacity-order-a");
    let router_b = install_router(&mut app, "back-capacity-order-b");

    let observer_a = testing_install_back_capacity_order_observer(
        &mut app.world_mut().resource_mut::<RouterNavigation>(),
        router_a,
    )
    .unwrap();
    let observer_a_clone = observer_a.clone();
    let observer_b = testing_install_back_capacity_order_observer(
        &mut app.world_mut().resource_mut::<RouterNavigation>(),
        router_b,
    )
    .unwrap();

    assert!(observer_a.observes_same_journal(&observer_a_clone));
    assert!(!observer_a.observes_same_journal(&observer_b));
    assert!(observer_a.snapshot().events().is_empty());
    assert!(observer_b.snapshot().events().is_empty());

    assert!(matches!(
        testing_install_back_capacity_order_observer(
            &mut app.world_mut().resource_mut::<RouterNavigation>(),
            router_a,
        ),
        Err(BackCapacityOrderInstallError::AlreadyInstalled)
    ));
    assert!(observer_a.observes_same_journal(&observer_a_clone));
    assert!(!observer_a.observes_same_journal(&observer_b));
    assert!(observer_a.snapshot().events().is_empty());
    assert!(observer_b.snapshot().events().is_empty());

    let mut foreign_app = App::new();
    foreign_app.add_plugins(BevyUiRouterPlugin::default());
    let foreign_router = install_router(&mut foreign_app, "back-capacity-order-foreign");
    assert!(matches!(
        testing_install_back_capacity_order_observer(
            &mut app.world_mut().resource_mut::<RouterNavigation>(),
            foreign_router,
        ),
        Err(BackCapacityOrderInstallError::ForeignRouter)
    ));
    assert!(observer_a.observes_same_journal(&observer_a_clone));
    assert!(!observer_a.observes_same_journal(&observer_b));
    assert!(observer_a.snapshot().events().is_empty());
    assert!(observer_b.snapshot().events().is_empty());
}

#[test]
fn history_back_no_entry_is_synchronous_and_creates_no_request() {
    let mut app = App::new();
    app.add_plugins(BevyUiRouterPlugin::default());
    let router = install_history_audit_router(&mut app, "history-back-no-entry");
    let first = push_and_publish(&mut app, router, RouteLocation::new("/items/1"));
    let before_router = app
        .world()
        .resource::<RouterSnapshots>()
        .get(router)
        .expect("the installed router has a snapshot")
        .clone();
    let before_request = match app
        .world()
        .resource::<RouterSnapshots>()
        .request(router, first)
    {
        RequestLookup::Found(snapshot) => snapshot.clone(),
        RequestLookup::NotRetained | RequestLookup::Unknown => {
            panic!("the setup request must remain retained")
        }
    };
    let observer = testing_install_back_capacity_order_observer(
        &mut app.world_mut().resource_mut::<RouterNavigation>(),
        router,
    )
    .unwrap();
    let inventory_before =
        testing_back_request_inventory(&app.world().resource::<RouterNavigation>(), router)
            .expect("the installed router has a Back request inventory");

    let error = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .back(router)
        .expect_err("cursor zero must reject Back synchronously");
    assert_eq!(error.kind(), NavigationEnqueueErrorKind::NoBackEntry);
    let inventory_after_error =
        testing_back_request_inventory(&app.world().resource::<RouterNavigation>(), router)
            .expect("the synchronous rejection retains its Back request inventory");
    assert_eq!(inventory_after_error, inventory_before);
    assert_eq!(
        observer.snapshot().events(),
        &[(0, BackCapacityOrderEvent::FacadeTypedReturn)]
    );

    app.update();

    let snapshots = app.world().resource::<RouterSnapshots>();
    assert_eq!(snapshots.get(router), Some(&before_router));
    match snapshots.request(router, first) {
        RequestLookup::Found(snapshot) => assert_eq!(snapshot, &before_request),
        RequestLookup::NotRetained | RequestLookup::Unknown => {
            panic!("synchronous rejection must preserve the retained request collection")
        }
    }
    assert_eq!(
        testing_back_request_inventory(&app.world().resource::<RouterNavigation>(), router),
        Some(inventory_before)
    );
}

#[test]
fn history_back_then_push_truncates_forward_tail() {
    let mut app = App::new();
    app.add_plugins(BevyUiRouterPlugin::default());
    let router = install_history_audit_router(&mut app, "history-back-truncate");
    let route_a = RouteLocation::new("/items/1");
    let route_b = RouteLocation::new("/items/2");
    let route_c = RouteLocation::new("/items/3");
    let route_d = RouteLocation::new("/items/4");

    push_and_publish(&mut app, router, route_a.clone());
    push_and_publish(&mut app, router, route_b.clone());
    push_and_publish(&mut app, router, route_c.clone());
    let back = back_and_publish(&mut app, router);
    let back_snapshot = match app
        .world()
        .resource::<RouterSnapshots>()
        .request(router, back)
    {
        RequestLookup::Found(snapshot) => snapshot,
        RequestLookup::NotRetained | RequestLookup::Unknown => {
            panic!("the completed Back request must remain retained")
        }
    };
    assert_eq!(back_snapshot.status(), &RequestStatus::Succeeded);
    assert_eq!(back_snapshot.requested(), Some(&route_b));

    push_and_publish(&mut app, router, route_d.clone());
    let snapshots = app.world().resource::<RouterSnapshots>();
    let router_snapshot = snapshots.get(router).unwrap();
    assert_eq!(
        router_snapshot.history().entries(),
        [route_a, route_b, route_d.clone()]
    );
    assert_eq!(router_snapshot.history().cursor(), Some(2));
    assert_eq!(router_snapshot.history().current(), Some(&route_d));
    assert_eq!(router_snapshot.active().location(), Some(&route_d));
    assert!(!router_snapshot.history().entries().contains(&route_c));
}

#[test]
fn root_push_publishes_read_only_authority_mappings_from_the_real_registry() {
    let mut app = App::new();
    app.add_plugins(BevyUiRouterPlugin::default());
    let driver = app
        .register_ui_view_driver(AuditDriver {
            key: ViewDriverKey::new("authority-audit-driver").unwrap(),
            outlet_view: None,
        })
        .unwrap();
    let view = RouteViewKey::new("authority-audit-root").unwrap();
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
    let host = app
        .world_mut()
        .spawn((Node::default(), RouterHostRegistration::new(router)))
        .id();
    app.update();

    let request = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, RouteLocation::new("/"))
        .unwrap();
    for _ in 0..8 {
        app.update();
        let terminal = {
            let snapshots = app.world().resource::<RouterSnapshots>();
            matches!(
                snapshots.request(router, request),
                RequestLookup::Found(snapshot)
                    if snapshot.status() == &RequestStatus::Succeeded
            )
        };
        if terminal {
            break;
        }
    }

    let snapshots = app.world().resource::<RouterSnapshots>();
    assert!(matches!(
        snapshots.request(router, request),
        RequestLookup::Found(snapshot) if snapshot.status() == &RequestStatus::Succeeded
    ));
    let audits = app.world().resource::<RouterAuthorityAudits>();
    let audit = audits.get(router).expect("installed router has an audit");
    let host_mapping = audit.host().expect("ready router has a host mapping");
    assert!(host_mapping.matches_app_entity(host));
    assert_eq!(audit.slots().len(), 1);
    assert_eq!(audit.slots()[0].kind(), AuthoritySlotKind::HostTop);
    assert_eq!(audit.frames().len(), 1);
    assert_eq!(
        audit.frames()[0].lifecycle(),
        AuthorityFrameLifecycle::Active
    );
    assert_eq!(audit.content().len(), 1);
    assert_eq!(
        audit.content()[0].primitive(),
        ContentPrimitiveKind::SolidRect
    );
    assert_eq!(audit.content()[0].width_px(), 37);
    assert_eq!(audit.content()[0].height_px(), 19);
    assert!(audit.content()[0].pickable_active());
    assert!(!audit.content()[0].visibility_hidden());
    assert!(audit.orphan_slots().is_empty());

    let kinds = audit
        .events()
        .iter()
        .map(RouterLifecycleAuditEvent::kind)
        .collect::<Vec<_>>();
    let prepared = kinds
        .iter()
        .position(|kind| *kind == RouterLifecycleAuditEventKind::ShellPrepared)
        .expect("shell preparation must be observed");
    let active = kinds
        .iter()
        .position(|kind| *kind == RouterLifecycleAuditEventKind::FrameActivated)
        .expect("frame activation must be observed");
    let active_frame = audit.frames()[0].frame();
    assert!(audit.events().iter().any(|event| {
        event.kind() == RouterLifecycleAuditEventKind::RegistryPublished
            && event.frame() == Some(active_frame)
            && event.entity().is_some_and(|entity| {
                audit.owned_entity_state(entity) == AuditEntityState::PresentOwned
            })
    }));
    assert!(
        prepared < active,
        "unexpected lifecycle audit order: {kinds:?}"
    );
}

#[test]
fn nested_outlet_authority_is_opaque_and_exact() {
    let mut app = App::new();
    app.add_plugins(BevyUiRouterPlugin::default());
    let layout = RouteViewKey::new("authority-audit-layout").unwrap();
    let leaf = RouteViewKey::new("authority-audit-leaf").unwrap();
    let fallback = RouteViewKey::new("authority-audit-fallback").unwrap();
    let driver = app
        .register_ui_view_driver(AuditDriver {
            key: ViewDriverKey::new("nested-authority-audit-driver").unwrap(),
            outlet_view: Some(layout.clone()),
        })
        .unwrap();
    let mut routes = RouteTree::builder()
        .root_with_options(
            RoutePattern::pathless(),
            RouteNodeOptions::new().with_view(layout.clone()),
        )
        .unwrap();
    let layout_route = routes.root_id().unwrap();
    routes
        .route_with_options(
            layout_route,
            RoutePattern::path("items/:id").unwrap(),
            RouteNodeOptions::new()
                .with_view(leaf.clone())
                .with_slot(RouteSlotKey::default()),
        )
        .unwrap();
    let routes = routes.build().unwrap();
    let mut views = RouteViewRegistry::new();
    for view in [layout, leaf, fallback.clone()] {
        views
            .insert(view, RouteViewDescriptor::new(driver.clone()))
            .unwrap();
    }
    let router = app
        .add_ui_router(RouterConfig::new(routes, views, fallback))
        .unwrap();
    app.world_mut()
        .spawn((Node::default(), RouterHostRegistration::new(router)));
    app.update();

    let request = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, RouteLocation::new("/items/1"))
        .unwrap();
    for _ in 0..16 {
        app.update();
        if matches!(
            app.world()
                .resource::<RouterSnapshots>()
                .request(router, request),
            RequestLookup::Found(snapshot) if snapshot.status() == &RequestStatus::Succeeded
        ) {
            break;
        }
    }

    let audits = app.world().resource::<RouterAuthorityAudits>();
    let audit = audits.get(router).expect("installed router has an audit");
    assert_eq!(audit.frames().len(), 2);
    assert_eq!(audit.slots().len(), 2);
    assert_eq!(audit.outlets().len(), 1);
    assert_eq!(audit.slots()[0].kind(), AuthoritySlotKind::HostTop);
    assert_eq!(audit.slots()[1].kind(), AuthoritySlotKind::ChildOutlet);
    assert_ne!(audit.frames()[0].root(), audit.frames()[1].root());
    assert_eq!(
        audit.frames()[0].lifecycle(),
        AuthorityFrameLifecycle::Active
    );
    assert_eq!(
        audit.frames()[1].lifecycle(),
        AuthorityFrameLifecycle::Active
    );
    let active_frames = audit
        .frames()
        .iter()
        .filter(|frame| frame.lifecycle() == AuthorityFrameLifecycle::Active)
        .map(|frame| frame.frame())
        .collect::<Vec<_>>();
    assert_eq!(active_frames.len(), 2);
    assert_eq!(audit.content().len(), 2);
    for active_frame in active_frames {
        let mappings = audit
            .content()
            .iter()
            .filter(|mapping| mapping.frame() == active_frame)
            .collect::<Vec<_>>();
        assert_eq!(mappings.len(), 1);
        let mapping = mappings[0];
        assert_eq!(mapping.primitive(), ContentPrimitiveKind::SolidRect);
        assert!(mapping.width_px() > 0);
        assert!(mapping.height_px() > 0);
        assert_eq!(mapping.color(), ContentColor::srgba8(11, 23, 47, 255));
        assert!(!mapping.visibility_hidden());
        assert!(mapping.pickable_active());
        assert_eq!(
            audit.owned_entity_state(mapping.entity()),
            AuditEntityState::PresentOwned
        );
    }
    assert_ne!(audit.content()[0].entity(), audit.content()[1].entity());
    assert_eq!(audit.frames()[0].slot_entity(), audit.slots()[0].entity());
    assert_eq!(audit.frames()[1].slot_entity(), audit.slots()[1].entity());
    let outlet = &audit.outlets()[0];
    assert_eq!(outlet.parent(), audit.frames()[0].frame());
    assert_eq!(outlet.key(), &RouteSlotKey::default());
    assert_eq!(outlet.child_slot(), Some(audit.slots()[1].entity()));
    assert_eq!(
        audit.owned_entity_state(outlet.anchor()),
        AuditEntityState::PresentOwned
    );
    assert!(audit.orphan_slots().is_empty());
}

#[test]
fn history_replace_f2_fragment_authority_binds_exact_old_generation_and_receipts() {
    let mut app = App::new();
    app.add_plugins(BevyUiRouterPlugin::default());
    let layout = RouteViewKey::new("history-replace-audit-layout").unwrap();
    let leaf = RouteViewKey::new("history-replace-audit-leaf").unwrap();
    let fallback = RouteViewKey::new("history-replace-audit-fallback").unwrap();
    let driver = app
        .register_ui_view_driver(AuditDriver {
            key: ViewDriverKey::new("history-replace-audit-driver").unwrap(),
            outlet_view: Some(layout.clone()),
        })
        .unwrap();
    let mut routes = RouteTree::builder()
        .root_with_options(
            RoutePattern::pathless(),
            RouteNodeOptions::new().with_view(layout.clone()),
        )
        .unwrap();
    let layout_route = routes.root_id().unwrap();
    routes
        .route_with_options(
            layout_route,
            RoutePattern::path("items/:id").unwrap(),
            RouteNodeOptions::new()
                .with_view(leaf.clone())
                .with_slot(RouteSlotKey::default()),
        )
        .unwrap();
    let mut views = RouteViewRegistry::new();
    for view in [layout, leaf, fallback.clone()] {
        views
            .insert(view, RouteViewDescriptor::new(driver.clone()))
            .unwrap();
    }
    let router = app
        .add_ui_router(RouterConfig::new(routes.build().unwrap(), views, fallback))
        .unwrap();
    app.world_mut()
        .spawn((Node::default(), RouterHostRegistration::new(router)));
    app.update();

    push_and_publish(&mut app, router, RouteLocation::new("/items/1"));
    let before_frames = app
        .world()
        .resource::<RouterSnapshots>()
        .get(router)
        .unwrap()
        .active()
        .frames()
        .to_vec();
    let before_authority = {
        let audit = app.world().resource::<RouterAuthorityAudits>();
        let audit = audit.get(router).unwrap();
        let outlet = audit
            .outlets()
            .iter()
            .find(|outlet| outlet.parent() == &before_frames[0])
            .unwrap();
        let child_slot = outlet.child_slot().unwrap();
        let slot = audit
            .slots()
            .iter()
            .find(|slot| slot.entity() == child_slot)
            .unwrap();
        (
            audit
                .frames()
                .iter()
                .find(|frame| frame.frame() == &before_frames[0])
                .unwrap()
                .root(),
            outlet.anchor(),
            child_slot,
            slot.id(),
            audit
                .frames()
                .iter()
                .find(|frame| frame.frame() == &before_frames[1])
                .unwrap()
                .root(),
        )
    };

    let target = RouteLocation::new("/items/1").with_fragment("details");
    let request = replace_and_publish(&mut app, router, target.clone());
    let snapshots = app.world().resource::<RouterSnapshots>();
    let router_snapshot = snapshots.get(router).unwrap();
    let RequestLookup::Found(request_snapshot) = snapshots.request(router, request) else {
        panic!("replace request remains retained")
    };
    assert_eq!(router_snapshot.active().frames()[0], before_frames[0]);
    assert_eq!(router_snapshot.history().entries(), [target]);
    let entity_receipts = request_snapshot
        .trace()
        .iter()
        .filter(|event| {
            event.kind() == NavigationTraceEventKind::CleanupSettled
                && event.cleanup_disposition().is_some()
        })
        .collect::<Vec<_>>();
    assert!(
        !entity_receipts.is_empty(),
        "the old leaf exact manifest produces entity receipts"
    );
    assert!(
        entity_receipts
            .iter()
            .all(|event| event.cleanup_disposition() == Some(CleanupDisposition::Removed)),
        "every exact Entity obligation is removed"
    );
    let batch_completions = request_snapshot
        .trace()
        .iter()
        .filter(|event| {
            event.kind() == NavigationTraceEventKind::CleanupSettled
                && event.cleanup_disposition().is_none()
        })
        .count();
    assert_eq!(batch_completions, 1, "cleanup batch completes exactly once");
    let new_leaf = &router_snapshot.active().frames()[1];
    assert_eq!(new_leaf.slot(), before_frames[1].slot());
    assert_eq!(new_leaf.frame(), before_frames[1].frame());
    assert_eq!(
        new_leaf.generation().raw(),
        before_frames[1].generation().raw() + 1
    );

    let audits = app.world().resource::<RouterAuthorityAudits>();
    let audit = audits.get(router).unwrap();
    let layout_mapping = audit
        .frames()
        .iter()
        .find(|frame| frame.frame() == &before_frames[0])
        .unwrap();
    assert_eq!(layout_mapping.root(), before_authority.0);
    let outlet = audit
        .outlets()
        .iter()
        .find(|outlet| outlet.parent() == &before_frames[0])
        .unwrap();
    assert_eq!(outlet.anchor(), before_authority.1);
    assert_eq!(outlet.child_slot(), Some(before_authority.2));
    let child_slot = audit
        .slots()
        .iter()
        .find(|slot| slot.entity() == before_authority.2)
        .unwrap();
    assert_eq!(child_slot.id(), before_authority.3);
    assert_eq!(
        audit.owned_entity_state(before_authority.4),
        AuditEntityState::Removed
    );
    assert!(
        audit
            .frames()
            .iter()
            .all(|mapping| mapping.frame() != &before_frames[1])
    );
    let isolated = audit
        .events()
        .iter()
        .filter(|event| {
            event.kind() == RouterLifecycleAuditEventKind::FrameIsolated
                && event.frame() == Some(&before_frames[1])
                && event.entity() == Some(before_authority.4)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        isolated.len(),
        1,
        "the exact old leaf root is isolated once"
    );
    assert!(isolated[0].visibility_hidden());
    assert!(!isolated[0].accepts_pointer());
    assert!(!isolated[0].accepts_keyboard());
    assert!(!isolated[0].accepts_gamepad());
    assert!(!isolated[0].focused());
}

#[test]
fn reconciliation_authority_is_preserved_and_old_leaf_is_removed() {
    let mut app = App::new();
    app.add_plugins(BevyUiRouterPlugin::default());
    let probe = Arc::new(BoundaryProbe::default());
    install_boundary_probe(&mut app, probe.clone());

    let layout = RouteViewKey::new("reconciliation-audit-layout").unwrap();
    let leaf = RouteViewKey::new("reconciliation-audit-leaf").unwrap();
    let fallback = RouteViewKey::new("reconciliation-audit-fallback").unwrap();
    let driver = app
        .register_ui_view_driver(AuditDriver {
            key: ViewDriverKey::new("reconciliation-audit-driver").unwrap(),
            outlet_view: Some(layout.clone()),
        })
        .unwrap();
    let mut routes = RouteTree::builder()
        .root_with_options(
            RoutePattern::pathless(),
            RouteNodeOptions::new().with_view(layout.clone()),
        )
        .unwrap();
    let layout_route = routes.root_id().unwrap();
    routes
        .route_with_options(
            layout_route,
            RoutePattern::path("items/:id").unwrap(),
            RouteNodeOptions::new()
                .with_view(leaf.clone())
                .with_slot(RouteSlotKey::default()),
        )
        .unwrap();
    let routes = routes.build().unwrap();
    let mut views = RouteViewRegistry::new();
    for view in [layout, leaf, fallback.clone()] {
        views
            .insert(view, RouteViewDescriptor::new(driver.clone()))
            .unwrap();
    }
    let router = app
        .add_ui_router(RouterConfig::new(routes, views, fallback))
        .unwrap();
    app.world_mut()
        .spawn((Node::default(), RouterHostRegistration::new(router)));
    app.update();

    push_and_publish(&mut app, router, RouteLocation::new("/items/1"));
    let initial_frames = app
        .world()
        .resource::<RouterSnapshots>()
        .get(router)
        .expect("installed router has a snapshot")
        .active()
        .frames()
        .to_vec();
    assert_eq!(initial_frames.len(), 2);
    let preserved_layout = initial_frames[0].clone();
    let old_leaf = initial_frames[1].clone();
    let (
        preserved_layout_root,
        preserved_layout_slot,
        preserved_outlet_anchor,
        preserved_child_slot,
        preserved_child_slot_id,
        preserved_child_slot_host,
        old_leaf_root,
    ) = {
        let audits = app.world().resource::<RouterAuthorityAudits>();
        let audit = audits.get(router).expect("installed router has an audit");
        let layout_mapping = audit
            .frames()
            .iter()
            .find(|mapping| mapping.frame() == &preserved_layout)
            .expect("layout mapping is published");
        let old_leaf_mapping = audit
            .frames()
            .iter()
            .find(|mapping| mapping.frame() == &old_leaf)
            .expect("old leaf mapping is published");
        let outlet = audit
            .outlets()
            .iter()
            .find(|outlet| {
                outlet.parent() == &preserved_layout && outlet.key() == &RouteSlotKey::default()
            })
            .expect("layout outlet mapping is published");
        let child_slot = outlet.child_slot().expect("layout outlet has a child slot");
        let slot_mapping = audit
            .slots()
            .iter()
            .find(|slot| slot.entity() == child_slot)
            .expect("child slot mapping is published");
        assert_eq!(slot_mapping.kind(), AuthoritySlotKind::ChildOutlet);
        assert_eq!(slot_mapping.parent_frame(), Some(&preserved_layout));
        assert_eq!(slot_mapping.outlet(), Some(&RouteSlotKey::default()));
        (
            layout_mapping.root(),
            layout_mapping.slot_entity(),
            outlet.anchor(),
            child_slot,
            slot_mapping.id(),
            slot_mapping.host().clone(),
            old_leaf_mapping.root(),
        )
    };

    probe.armed.store(true, Ordering::Release);
    push_and_publish(&mut app, router, RouteLocation::new("/items/2"));
    probe.armed.store(false, Ordering::Release);

    let journey_frames = app
        .world()
        .resource::<RouterSnapshots>()
        .get(router)
        .expect("installed router has a snapshot")
        .active()
        .frames()
        .to_vec();
    assert_eq!(journey_frames.len(), 2);
    assert_eq!(journey_frames[0], preserved_layout);
    let new_leaf = journey_frames[1].clone();
    let audits = app.world().resource::<RouterAuthorityAudits>();
    let audit = audits.get(router).expect("installed router has an audit");
    let layout_mapping = audit
        .frames()
        .iter()
        .find(|mapping| mapping.frame() == &preserved_layout)
        .expect("preserved layout mapping remains published");
    assert_eq!(layout_mapping.root(), preserved_layout_root);
    assert_eq!(layout_mapping.slot_entity(), preserved_layout_slot);
    let outlet = audit
        .outlets()
        .iter()
        .find(|outlet| {
            outlet.parent() == &preserved_layout && outlet.key() == &RouteSlotKey::default()
        })
        .expect("preserved layout outlet remains published");
    assert_eq!(outlet.anchor(), preserved_outlet_anchor);
    assert_eq!(outlet.child_slot(), Some(preserved_child_slot));
    let child_slot = audit
        .slots()
        .iter()
        .find(|slot| slot.entity() == preserved_child_slot)
        .expect("preserved child slot remains published");
    assert_eq!(child_slot.id(), preserved_child_slot_id);
    assert_eq!(child_slot.host(), &preserved_child_slot_host);
    assert_eq!(child_slot.kind(), AuthoritySlotKind::ChildOutlet);
    assert_eq!(child_slot.parent_frame(), Some(&preserved_layout));
    assert_eq!(child_slot.outlet(), Some(&RouteSlotKey::default()));
    assert_eq!(
        audit.owned_entity_state(old_leaf_root),
        AuditEntityState::Removed
    );

    let isolated = audit
        .events()
        .iter()
        .position(|event| {
            event.kind() == RouterLifecycleAuditEventKind::FrameIsolated
                && event.frame() == Some(&old_leaf)
                && event.entity() == Some(old_leaf_root)
        })
        .expect("old leaf isolation is published");
    let activated = audit
        .events()
        .iter()
        .position(|event| {
            event.kind() == RouterLifecycleAuditEventKind::FrameActivated
                && event.frame() == Some(&new_leaf)
        })
        .expect("new leaf activation is published");
    assert!(isolated < activated);
    let isolation = &audit.events()[isolated];
    assert!(isolation.visibility_hidden());
    assert!(!isolation.accepts_pointer());
    assert!(!isolation.accepts_keyboard());
    assert!(!isolation.accepts_gamepad());
    assert!(!isolation.focused());
    let activation = &audit.events()[activated];
    assert!(!activation.visibility_hidden());
    assert!(activation.accepts_pointer());
    assert!(activation.accepts_keyboard());
    assert!(activation.accepts_gamepad());

    let samples = probe
        .samples
        .lock()
        .expect("boundary sample lock must not be poisoned")
        .clone();
    assert!(samples.iter().any(|(boundary, interactive)| {
        *boundary == Boundary::BeforeCommit && *interactive == 1
    }));
    assert!(samples.iter().any(|(boundary, interactive)| {
        *boundary == Boundary::AfterCommit && *interactive == 1
    }));
    assert!(samples.iter().all(|(_, interactive)| *interactive <= 1));

    push_and_publish(
        &mut app,
        router,
        RouteLocation::new("/items/2").with_fragment("details"),
    );
    let audits = app.world().resource::<RouterAuthorityAudits>();
    let audit = audits.get(router).expect("installed router has an audit");
    let layout_mapping = audit
        .frames()
        .iter()
        .find(|mapping| mapping.frame() == &preserved_layout)
        .expect("fragment preserves the layout mapping");
    assert_eq!(layout_mapping.root(), preserved_layout_root);
    assert_eq!(layout_mapping.slot_entity(), preserved_layout_slot);
    let outlet = audit
        .outlets()
        .iter()
        .find(|outlet| {
            outlet.parent() == &preserved_layout && outlet.key() == &RouteSlotKey::default()
        })
        .expect("fragment preserves the layout outlet");
    assert_eq!(outlet.anchor(), preserved_outlet_anchor);
    assert_eq!(outlet.child_slot(), Some(preserved_child_slot));
    let child_slot = audit
        .slots()
        .iter()
        .find(|slot| slot.entity() == preserved_child_slot)
        .expect("fragment preserves the child slot mapping");
    assert_eq!(child_slot.id(), preserved_child_slot_id);
    assert_eq!(child_slot.host(), &preserved_child_slot_host);
    assert_eq!(child_slot.parent_frame(), Some(&preserved_layout));
    assert_eq!(child_slot.outlet(), Some(&RouteSlotKey::default()));
    assert_eq!(
        audit.owned_entity_state(old_leaf_root),
        AuditEntityState::Removed
    );
}

#[test]
fn recovery_r1_error_presentation_authority_maps_exact_frame() {
    #[derive(Clone, Copy)]
    enum Case {
        Bound,
        RouterFallback,
    }

    for (label, case) in [
        ("r1-authority-bound", Case::Bound),
        ("r1-authority-fallback", Case::RouterFallback),
    ] {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let normal = RouteViewKey::new(format!("{label}-normal")).unwrap();
        let error = RouteViewKey::new(format!("{label}-error")).unwrap();
        let fallback = RouteViewKey::new(format!("{label}-fallback")).unwrap();
        let driver = app
            .register_ui_view_driver(RecoveryAuditDriver {
                key: ViewDriverKey::new(format!("{label}-driver")).unwrap(),
                failed_view: matches!(case, Case::Bound).then_some(normal.clone()),
                error_view: error.clone(),
                fallback_view: fallback.clone(),
            })
            .unwrap();
        let options = match case {
            Case::Bound => RouteNodeOptions::new()
                .with_view(normal.clone())
                .with_error_view(
                    ErrorBoundaryKey::named(format!("{label}-boundary")).unwrap(),
                    error.clone(),
                ),
            Case::RouterFallback => RouteNodeOptions::new().with_view(normal.clone()),
        };
        let routes = RouteTree::builder()
            .root_with_options(RoutePattern::path("known").unwrap(), options)
            .unwrap();
        let boundary_route = routes.root_id().unwrap();
        let routes = routes.build().unwrap();
        let mut views = RouteViewRegistry::new();
        for view in [normal, error.clone(), fallback.clone()] {
            views
                .insert(view, RouteViewDescriptor::new(driver.clone()))
                .unwrap();
        }
        let router = app
            .add_ui_router(RouterConfig::new(routes, views, fallback))
            .unwrap();
        let host = app
            .world_mut()
            .spawn((
                Node {
                    width: Val::Px(503.0),
                    height: Val::Px(307.0),
                    ..Node::default()
                },
                RecoveryAuditHostSentinel(0x51),
                RouterHostRegistration::new(router),
            ))
            .id();
        let child = app
            .world_mut()
            .spawn((
                Node {
                    width: Val::Px(13.0),
                    ..Node::default()
                },
                RecoveryAuditChildSentinel(0x52),
                ChildOf(host),
            ))
            .id();
        let host_manifest_before = audit_component_manifest(&app, host);
        let child_manifest_before = audit_component_manifest(&app, child);
        app.update();
        let top_slot = app
            .world()
            .get::<Children>(host)
            .unwrap()
            .iter()
            .copied()
            .find(|candidate| *candidate != child)
            .expect("admission adds one exact top slot");

        let requested = match case {
            Case::Bound => RouteLocation::new("/known"),
            Case::RouterFallback => RouteLocation::new("/missing"),
        };
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, requested.clone())
            .unwrap();
        for _ in 0..16 {
            app.update();
            if matches!(
                app.world().resource::<RouterSnapshots>().request(router, request),
                RequestLookup::Found(snapshot) if matches!(snapshot.status(), RequestStatus::Failed(_))
            ) {
                break;
            }
        }

        let snapshots = app.world().resource::<RouterSnapshots>();
        let router_snapshot = snapshots.get(router).unwrap();
        let RequestLookup::Found(request_snapshot) = snapshots.request(router, request) else {
            panic!("recovery audit request remains retained")
        };
        let expected_failure = match case {
            Case::Bound => NavigationFailureKind::Apply,
            Case::RouterFallback => NavigationFailureKind::Match,
        };
        assert!(matches!(
            request_snapshot.status(),
            RequestStatus::Failed(failure) if failure.kind() == expected_failure
        ));
        assert_eq!(request_snapshot.requested(), Some(&requested));
        assert_eq!(
            request_snapshot
                .trace()
                .last()
                .map(NavigationTraceEvent::kind),
            Some(NavigationTraceEventKind::RequestTerminal)
        );
        assert_eq!(router_snapshot.active().frames().len(), 1);
        let active = &router_snapshot.active().frames()[0];
        let expected_origin = match case {
            Case::Bound => FrameOrigin::RouteError(boundary_route),
            Case::RouterFallback => FrameOrigin::RouterFallback,
        };
        assert_eq!(active.origin(), &expected_origin);
        assert_eq!(router_snapshot.active().requested(), Some(&requested));
        assert_eq!(
            router_snapshot
                .active()
                .failure()
                .map(NavigationFailure::kind),
            Some(expected_failure)
        );
        assert!(router_snapshot.running_request().is_none());
        assert!(router_snapshot.pending_phase().is_none());
        assert!(!router_snapshot.cleanup().foreground_pending());
        assert_eq!(router_snapshot.cleanup().quarantine_count(), 0);

        let audits = app.world().resource::<RouterAuthorityAudits>();
        let audit = audits
            .get(router)
            .expect("terminal router retains exact audit");
        let host_mapping = audit.host().unwrap();
        assert!(host_mapping.matches_app_entity(host));
        assert_eq!(audit.frames().len(), 1);
        let mapping = &audit.frames()[0];
        assert_eq!(mapping.frame(), active);
        assert_eq!(mapping.lifecycle(), AuthorityFrameLifecycle::Active);
        assert_eq!(mapping.slot_entity(), host_mapping.top_slot());
        assert_eq!(audit.slots().len(), 1);
        assert_eq!(audit.slots()[0].entity(), host_mapping.top_slot());
        assert_eq!(audit.slots()[0].kind(), AuthoritySlotKind::HostTop);
        assert_eq!(audit.content().len(), 1);
        assert_eq!(audit.content()[0].frame(), active);
        assert_eq!(
            audit.content()[0].primitive(),
            ContentPrimitiveKind::SolidRect
        );
        assert_eq!(
            audit.content()[0].width_px(),
            match case {
                Case::Bound => 67,
                Case::RouterFallback => 71,
            }
        );
        assert!(!audit.content()[0].visibility_hidden());
        assert!(audit.content()[0].pickable_active());
        assert!(audit.outlets().is_empty());
        assert!(audit.orphan_slots().is_empty());

        assert_eq!(audit_component_manifest(&app, host), host_manifest_before);
        assert_eq!(audit_component_manifest(&app, child), child_manifest_before);
        assert_eq!(
            app.world().get::<RecoveryAuditHostSentinel>(host),
            Some(&RecoveryAuditHostSentinel(0x51))
        );
        assert_eq!(
            app.world().get::<RecoveryAuditChildSentinel>(child),
            Some(&RecoveryAuditChildSentinel(0x52))
        );
        assert_eq!(
            app.world().get::<ChildOf>(child).map(ChildOf::parent),
            Some(host)
        );
        assert_eq!(
            app.world()
                .get::<Children>(host)
                .unwrap()
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            vec![child, top_slot]
        );
        assert_eq!(app.world().get::<Node>(host).unwrap().width, Val::Px(503.0));
        assert_eq!(app.world().get::<Node>(child).unwrap().width, Val::Px(13.0));
    }
}

#[test]
fn recovery_r2_authority_audit_closes_l_a_h_and_preserves_host_order() {
    let mut app = App::new();
    app.add_plugins(BevyUiRouterPlugin::default());
    let driver = app
        .register_ui_view_driver(AuditDriver {
            key: ViewDriverKey::new("r2-authority-audit-driver").unwrap(),
            outlet_view: None,
        })
        .unwrap();
    let view = RouteViewKey::new("r2-authority-audit-view").unwrap();
    let routes = RouteTree::builder()
        .root_with_options(
            RoutePattern::path("/items/:id").unwrap(),
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
    let host = app
        .world_mut()
        .spawn((
            Node {
                width: Val::Px(607.0),
                height: Val::Px(347.0),
                ..Node::default()
            },
            RecoveryAuditHostSentinel(0x621),
            RouterHostRegistration::new(router),
        ))
        .id();
    let first_child = app
        .world_mut()
        .spawn((
            Node {
                width: Val::Px(17.0),
                ..Node::default()
            },
            RecoveryAuditChildSentinel(0x622),
            ChildOf(host),
        ))
        .id();
    let second_child = app
        .world_mut()
        .spawn((
            Node {
                width: Val::Px(19.0),
                ..Node::default()
            },
            RecoveryAuditChildSentinel(0x623),
            ChildOf(host),
        ))
        .id();
    let host_manifest_before = audit_component_manifest(&app, host);
    let first_manifest_before = audit_component_manifest(&app, first_child);
    let second_manifest_before = audit_component_manifest(&app, second_child);
    app.world_mut().insert_resource(RecoveryR2AuditLoss {
        router,
        host,
        armed: false,
        fired: false,
    });
    app.world_mut()
        .init_resource::<RecoveryR2AuditCommitBoundary>();
    app.add_systems(
        Update,
        capture_recovery_r2_audit_commit_boundary
            .after(RouterScheduleSet::CommitPresentation)
            .before(inject_recovery_r2_audit_registration_loss),
    );
    app.add_systems(
        Update,
        inject_recovery_r2_audit_registration_loss
            .after(RouterScheduleSet::CommitPresentation)
            .before(RouterScheduleSet::SettleCleanup),
    );
    let app_entities_before_admission = app
        .world()
        .iter_entities()
        .map(|entity| entity.id())
        .collect::<HashSet<_>>();
    app.update();
    let first = push_and_publish(&mut app, router, RouteLocation::new("/items/1"));
    assert!(matches!(
        app.world().resource::<RouterSnapshots>().request(router, first),
        RequestLookup::Found(snapshot) if snapshot.status() == &RequestStatus::Succeeded
    ));
    let old_observable_mapping = {
        let audit = app
            .world()
            .resource::<RouterAuthorityAudits>()
            .get(router)
            .unwrap();
        recovery_r3_frame_observable_entities(audit, AuthorityFrameLifecycle::Active)
    };
    assert_eq!(
        old_observable_mapping
            .iter()
            .copied()
            .collect::<HashSet<_>>()
            .len(),
        old_observable_mapping.len(),
        "the baseline observable public mapping must not alias its exposed ids"
    );
    let request = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, RouteLocation::new("/items/2"))
        .unwrap();
    {
        let mut loss = app.world_mut().resource_mut::<RecoveryR2AuditLoss>();
        loss.armed = true;
        loss.fired = false;
    }
    for _ in 0..16 {
        app.update();
        if matches!(
            app.world().resource::<RouterSnapshots>().request(router, request),
            RequestLookup::Found(snapshot) if matches!(snapshot.status(), RequestStatus::Succeeded | RequestStatus::Failed(_))
        ) {
            break;
        }
    }

    let snapshots = app.world().resource::<RouterSnapshots>();
    let RequestLookup::Found(request_snapshot) = snapshots.request(router, request) else {
        panic!("the R2 audit request remains retained")
    };
    assert_eq!(request_snapshot.status(), &RequestStatus::Succeeded);
    let router_snapshot = snapshots.get(router).unwrap();
    assert_eq!(
        router_snapshot.availability(),
        &RouterAvailability::Unavailable(RouterUnavailableReason::HostLost)
    );
    assert_eq!(
        router_snapshot.active().kind(),
        ActivePresentationKind::None
    );
    assert_eq!(
        router_snapshot.history().entries(),
        [
            RouteLocation::new("/items/1"),
            RouteLocation::new("/items/2")
        ]
    );
    assert!(router_snapshot.running_request().is_none());
    assert!(router_snapshot.pending_phase().is_none());
    assert!(!router_snapshot.cleanup().foreground_pending());

    let boundary = app.world().resource::<RecoveryR2AuditCommitBoundary>();
    let observable_leaving = boundary.leaving.clone();
    let observable_authority_non_top = boundary.authority_non_top.clone();
    let top_slot = boundary
        .top_slot
        .expect("the commit boundary freezes the exact A-top slot");
    assert!(!observable_leaving.is_empty());
    assert!(!observable_authority_non_top.is_empty());
    assert_eq!(
        observable_leaving, old_observable_mapping,
        "the public audit preserves the observable leaving mapping"
    );
    assert_eq!(
        observable_leaving
            .iter()
            .copied()
            .collect::<HashSet<_>>()
            .len(),
        observable_leaving.len()
    );
    assert_eq!(
        observable_authority_non_top
            .iter()
            .copied()
            .collect::<HashSet<_>>()
            .len(),
        observable_authority_non_top.len()
    );
    let leaving_set = observable_leaving.iter().copied().collect::<HashSet<_>>();
    let authority_set = observable_authority_non_top
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    assert!(leaving_set.is_disjoint(&authority_set));
    assert!(!leaving_set.contains(&top_slot));
    assert!(!authority_set.contains(&top_slot));

    let trace = request_snapshot.trace();
    let commit = trace
        .iter()
        .position(|event| event.kind() == NavigationTraceEventKind::CommitPublished)
        .unwrap();
    let tail = &trace[commit..];
    assert_eq!(tail[0].kind(), NavigationTraceEventKind::CommitPublished);
    assert_eq!(
        tail[1].kind(),
        NavigationTraceEventKind::RequestPhaseChanged
    );
    assert_eq!(tail[1].phase(), Some(&NavigationPhase::PostCommitCleanup));
    assert_eq!(
        tail[2].kind(),
        NavigationTraceEventKind::RequestPhaseChanged
    );
    assert_eq!(tail[2].phase(), Some(&NavigationPhase::Recovering));
    let authority_end = tail.len() - 4;
    assert!(tail[3..authority_end].iter().all(|event| {
        event.kind() == NavigationTraceEventKind::CleanupSettled
            && event.cleanup_disposition().is_some()
    }));
    assert_eq!(
        tail[authority_end].kind(),
        NavigationTraceEventKind::CleanupSettled
    );
    assert_eq!(tail[authority_end].cleanup_disposition(), None);
    assert_eq!(
        tail[authority_end + 1].kind(),
        NavigationTraceEventKind::CleanupSettled
    );
    assert!(tail[authority_end + 1].cleanup_disposition().is_some());
    assert_eq!(
        tail[authority_end + 2].kind(),
        NavigationTraceEventKind::CleanupSettled
    );
    assert_eq!(tail[authority_end + 2].cleanup_disposition(), None);
    assert_eq!(
        tail[authority_end + 3].kind(),
        NavigationTraceEventKind::RequestTerminal
    );

    let audit = app
        .world()
        .resource::<RouterAuthorityAudits>()
        .get(router)
        .expect("stable unavailable retains the terminal authority audit");
    assert!(audit.frames().is_empty());
    assert!(audit.slots().is_empty());
    assert!(audit.content().is_empty());
    assert!(audit.outlets().is_empty());
    assert!(audit.host().is_none());
    assert!(audit.orphan_slots().is_empty());
    assert!(
        observable_leaving
            .iter()
            .chain(&observable_authority_non_top)
            .chain(std::iter::once(&top_slot))
            .all(|entity| audit.owned_entity_state(*entity) == AuditEntityState::Removed)
    );

    assert_eq!(
        app.world().get::<RecoveryAuditHostSentinel>(host),
        Some(&RecoveryAuditHostSentinel(0x621))
    );
    let host_node = app.world().get::<Node>(host).unwrap();
    assert_eq!(host_node.width, Val::Px(607.0));
    assert_eq!(host_node.height, Val::Px(347.0));
    assert_eq!(
        app.world().get::<RecoveryAuditChildSentinel>(first_child),
        Some(&RecoveryAuditChildSentinel(0x622))
    );
    assert_eq!(
        app.world().get::<RecoveryAuditChildSentinel>(second_child),
        Some(&RecoveryAuditChildSentinel(0x623))
    );
    assert_eq!(
        app.world().get::<ChildOf>(first_child).map(ChildOf::parent),
        Some(host)
    );
    assert_eq!(
        app.world()
            .get::<ChildOf>(second_child)
            .map(ChildOf::parent),
        Some(host)
    );
    assert_eq!(
        app.world().get::<Node>(first_child).unwrap().width,
        Val::Px(17.0)
    );
    assert_eq!(
        app.world().get::<Node>(second_child).unwrap().width,
        Val::Px(19.0)
    );
    assert_eq!(
        audit_component_manifest(&app, first_child),
        first_manifest_before
    );
    assert_eq!(
        audit_component_manifest(&app, second_child),
        second_manifest_before
    );
    assert_eq!(
        app.world()
            .get::<Children>(host)
            .unwrap()
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![first_child, second_child]
    );
    let registration_component = app
        .world()
        .component_id::<RouterHostRegistration>()
        .unwrap();
    let host_manifest_after = audit_component_manifest(&app, host);
    assert_eq!(
        host_manifest_after
            .iter()
            .filter(|component| **component != registration_component)
            .copied()
            .collect::<Vec<_>>(),
        host_manifest_before
            .iter()
            .filter(|component| **component != registration_component)
            .copied()
            .collect::<Vec<_>>()
    );
    assert_eq!(
        app.world()
            .iter_entities()
            .map(|entity| entity.id())
            .collect::<HashSet<_>>(),
        app_entities_before_admission,
        "terminal audit completeness requires every router-created hidden Entity to be gone"
    );
}

#[test]
fn recovery_r3_same_host_and_stale_epoch_authority_audit_is_exact() {
    let mut app = App::new();
    app.add_plugins(BevyUiRouterPlugin::default());
    let layout = RouteViewKey::new("r3-replay-audit-layout").unwrap();
    let leaf = RouteViewKey::new("r3-replay-audit-leaf").unwrap();
    let fallback = RouteViewKey::new("r3-replay-audit-fallback").unwrap();
    let effects = Arc::new(AtomicUsize::new(0));
    let driver = app
        .register_ui_view_driver(RecoveryR3ReplayAuditDriver {
            key: ViewDriverKey::new("r3-replay-audit-driver").unwrap(),
            outlet_view: layout.clone(),
            effects: effects.clone(),
        })
        .unwrap();
    let mut routes = RouteTree::builder()
        .root_with_options(
            RoutePattern::pathless(),
            RouteNodeOptions::new().with_view(layout.clone()),
        )
        .unwrap();
    let root_route = routes.root_id().unwrap();
    let section = routes
        .route_with_options(
            root_route,
            RoutePattern::path("section").unwrap(),
            RouteNodeOptions::new()
                .with_view(layout.clone())
                .with_slot(RouteSlotKey::default()),
        )
        .unwrap();
    routes
        .route_with_options(
            section,
            RoutePattern::path("items/:id").unwrap(),
            RouteNodeOptions::new()
                .with_view(leaf.clone())
                .with_slot(RouteSlotKey::default()),
        )
        .unwrap();
    let alternate = routes
        .route_with_options(
            root_route,
            RoutePattern::path("alternate").unwrap(),
            RouteNodeOptions::new()
                .with_view(layout.clone())
                .with_slot(RouteSlotKey::default()),
        )
        .unwrap();
    routes
        .route_with_options(
            alternate,
            RoutePattern::path("items/:id").unwrap(),
            RouteNodeOptions::new()
                .with_view(leaf.clone())
                .with_slot(RouteSlotKey::default()),
        )
        .unwrap();
    let mut views = RouteViewRegistry::new();
    for view in [layout, leaf, fallback.clone()] {
        views
            .insert(view, RouteViewDescriptor::new(driver.clone()))
            .unwrap();
    }
    let router = app
        .add_ui_router(RouterConfig::new(routes.build().unwrap(), views, fallback))
        .unwrap();
    let host = app
        .world_mut()
        .spawn((
            Node {
                width: Val::Px(613.0),
                height: Val::Px(353.0),
                ..Node::default()
            },
            RecoveryAuditHostSentinel(0x731),
            RouterHostRegistration::new(router),
        ))
        .id();
    let first_child = app
        .world_mut()
        .spawn((
            Node {
                width: Val::Px(23.0),
                ..Node::default()
            },
            RecoveryAuditChildSentinel(0x732),
            ChildOf(host),
        ))
        .id();
    let second_child = app
        .world_mut()
        .spawn((
            Node {
                width: Val::Px(29.0),
                ..Node::default()
            },
            RecoveryAuditChildSentinel(0x733),
            ChildOf(host),
        ))
        .id();
    let host_manifest = audit_component_manifest(&app, host);
    let first_manifest = audit_component_manifest(&app, first_child);
    let second_manifest = audit_component_manifest(&app, second_child);
    app.update();

    let before_loss = app
        .world()
        .resource::<RouterSnapshots>()
        .get(router)
        .unwrap()
        .clone();
    let history_before_loss = before_loss.history().clone();
    assert!(history_before_loss.current().is_none());
    let before_loss_audit = app
        .world()
        .resource::<RouterAuthorityAudits>()
        .get(router)
        .unwrap()
        .clone();
    let old_host_mapping = before_loss_audit.host().unwrap().clone();
    assert!(old_host_mapping.matches_app_entity(host));
    let old_top_slot = app
        .world()
        .get::<Children>(host)
        .unwrap()
        .iter()
        .copied()
        .find(|child| ![first_child, second_child].contains(child))
        .expect("initial admission exposes one real top-slot Entity");
    let effects_before_stale = effects.load(Ordering::SeqCst);
    let stale_request = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, RouteLocation::new("/section/items/1"))
        .expect("the sealed nested audit branch enqueues");
    app.update();
    assert!(effects.load(Ordering::SeqCst) > effects_before_stale);
    let stale_audit_after_first_effect = app
        .world()
        .resource::<RouterAuthorityAudits>()
        .get(router)
        .unwrap();
    let prepared_before_capture =
        recovery_r3_prepared_observable_entities(stale_audit_after_first_effect);
    assert!(
        !prepared_before_capture.is_empty(),
        "one entering output is physically PreparedInert before capture: snapshot={:?}, audit={stale_audit_after_first_effect:?}, effects_before={effects_before_stale}, effects_after={}",
        app.world().resource::<RouterSnapshots>().get(router),
        effects.load(Ordering::SeqCst),
    );
    let replay = arm_recovery_r3_audit_replay(&mut app, router);
    let capture_effect = effects.load(Ordering::SeqCst) + 1;
    for _ in 0..16 {
        app.update();
        if effects.load(Ordering::SeqCst) >= capture_effect {
            break;
        }
    }
    assert_eq!(effects.load(Ordering::SeqCst), capture_effect);
    let stale_entities = recovery_r3_prepared_observable_entities(
        app.world()
            .resource::<RouterAuthorityAudits>()
            .get(router)
            .expect("the captured stale artifact retains an authority audit"),
    );
    assert!(
        !stale_entities.is_empty(),
        "the captured stale artifact must retain a non-empty observable public mapping"
    );
    let foreign = app
        .world_mut()
        .spawn((RecoveryAuditChildSentinel(0x734), ChildOf(old_top_slot)))
        .id();
    let foreign_manifest = audit_component_manifest(&app, foreign);

    app.world_mut()
        .insert_resource(RecoveryR3AuditLossInjection { host, fired: false });
    app.add_systems(
        Update,
        inject_recovery_r3_audit_registration_loss
            .after(RouterScheduleSet::CollectNavigation)
            .before(RouterScheduleSet::DriveNavigation),
    );
    let unavailable = drive_recovery_r3_audit_availability(
        &mut app,
        router,
        &RouterAvailability::Unavailable(RouterUnavailableReason::HostLost),
    );
    assert!(app.world().resource::<RecoveryR3AuditLossInjection>().fired);
    assert_eq!(unavailable.active().kind(), ActivePresentationKind::None);
    assert_eq!(unavailable.history(), &history_before_loss);
    let stale_terminal = app
        .world()
        .resource::<RouterSnapshots>()
        .request(router, stale_request);
    assert!(matches!(
        stale_terminal,
        RequestLookup::Found(snapshot)
            if matches!(snapshot.status(), RequestStatus::Failed(failure) if failure.kind() == NavigationFailureKind::HostLost)
    ));
    let unavailable_audit = app
        .world()
        .resource::<RouterAuthorityAudits>()
        .get(router)
        .unwrap();
    assert!(unavailable_audit.host().is_none());
    assert!(unavailable_audit.frames().is_empty());
    assert!(unavailable_audit.slots().is_empty());
    assert!(unavailable_audit.outlets().is_empty());
    assert!(unavailable_audit.content().is_empty());
    assert_eq!(
        unavailable_audit.owned_entity_state(old_host_mapping.top_slot()),
        AuditEntityState::Quarantined
    );
    assert!(stale_entities.iter().all(|entity| matches!(
        unavailable_audit.owned_entity_state(*entity),
        AuditEntityState::PresentOwned | AuditEntityState::Quarantined | AuditEntityState::Removed
    )));
    let stale_present_after_loss = stale_entities
        .iter()
        .copied()
        .filter(|entity| unavailable_audit.owned_entity_state(*entity) != AuditEntityState::Removed)
        .collect::<Vec<_>>();
    assert!(
        !stale_present_after_loss.is_empty(),
        "the stale replay retains a non-empty observable public mapping after HostLost"
    );
    assert_eq!(
        app.world()
            .get::<Children>(host)
            .unwrap()
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![first_child, second_child]
    );
    assert!(app.world().get::<ChildOf>(old_top_slot).is_none());
    assert_eq!(
        app.world().get::<ChildOf>(foreign).map(ChildOf::parent),
        Some(old_top_slot)
    );

    app.world_mut()
        .entity_mut(host)
        .insert(RouterHostRegistration::new(router));
    let ready = drive_recovery_r3_audit_availability(&mut app, router, &RouterAvailability::Ready);
    assert_eq!(ready.active().kind(), ActivePresentationKind::None);
    assert_eq!(ready.history(), &history_before_loss);
    let ready_audit = app
        .world()
        .resource::<RouterAuthorityAudits>()
        .get(router)
        .unwrap();
    let new_host_mapping = ready_audit.host().unwrap();
    assert!(new_host_mapping.matches_app_entity(host));
    assert_ne!(new_host_mapping.version(), old_host_mapping.version());
    assert_ne!(new_host_mapping.top_slot(), old_host_mapping.top_slot());
    let new_top_slot = app
        .world()
        .get::<Children>(host)
        .unwrap()
        .iter()
        .copied()
        .find(|child| ![first_child, second_child].contains(child))
        .expect("same-host re-admission exposes one fresh top slot");
    assert_ne!(new_top_slot, old_top_slot);
    assert_eq!(
        app.world()
            .get::<Children>(host)
            .unwrap()
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![first_child, second_child, new_top_slot]
    );
    assert!(app.world().get::<ChildOf>(old_top_slot).is_none());
    assert_eq!(
        app.world().get::<ChildOf>(foreign).map(ChildOf::parent),
        Some(old_top_slot)
    );

    let restored_location = RouteLocation::new("/section/items/1");
    let restored = replace_and_publish(&mut app, router, restored_location.clone());
    let snapshot_before_release = app
        .world()
        .resource::<RouterSnapshots>()
        .get(router)
        .unwrap()
        .clone();
    assert_eq!(
        snapshot_before_release.active().location(),
        Some(&restored_location)
    );
    assert_eq!(
        snapshot_before_release.history().current(),
        Some(&restored_location)
    );
    assert_eq!(
        snapshot_before_release.history().entries(),
        [restored_location.clone()]
    );
    let restored_before_release = match app
        .world()
        .resource::<RouterSnapshots>()
        .request(router, restored)
    {
        RequestLookup::Found(snapshot) => snapshot.clone(),
        _ => panic!("the explicit restore request remains retained"),
    };
    let stale_before_release = match app
        .world()
        .resource::<RouterSnapshots>()
        .request(router, stale_request)
    {
        RequestLookup::Found(snapshot) => snapshot.clone(),
        _ => panic!("the stale request remains retained"),
    };
    let audit_before_release = app
        .world()
        .resource::<RouterAuthorityAudits>()
        .get(router)
        .unwrap()
        .clone();
    let current_entities = recovery_r3_current_observable_entities(&audit_before_release);
    assert!(current_entities.iter().all(|entity| {
        audit_before_release.owned_entity_state(*entity) == AuditEntityState::PresentOwned
    }));
    let entities_before_release = app
        .world()
        .iter_entities()
        .map(|entity| entity.id())
        .collect::<HashSet<_>>();
    let diagnostics_before = snapshot_before_release.diagnostics().events().to_vec();
    let effects_before_release = effects.load(Ordering::SeqCst);
    let inventory_before_release = {
        let world = app.world();
        testing_router_request_inventory(
            world.resource::<RouterNavigation>(),
            world.resource::<RouterSnapshots>(),
        )
    };

    release_recovery_r3_audit_replay(&mut app, replay);
    for _ in 0..24 {
        app.update();
        let audit = app
            .world()
            .resource::<RouterAuthorityAudits>()
            .get(router)
            .unwrap();
        if stale_entities
            .iter()
            .all(|entity| audit.owned_entity_state(*entity) == AuditEntityState::Removed)
        {
            break;
        }
    }

    let snapshot_after_release = app
        .world()
        .resource::<RouterSnapshots>()
        .get(router)
        .unwrap()
        .clone();
    assert_recovery_r3_audit_facade_exact(&snapshot_before_release, &snapshot_after_release);
    assert!(
        snapshot_after_release
            .diagnostics()
            .events()
            .starts_with(&diagnostics_before)
    );
    let diagnostic_delta =
        &snapshot_after_release.diagnostics().events()[diagnostics_before.len()..];
    assert!(diagnostic_delta.len() <= 1);
    assert!(diagnostic_delta.iter().all(|event| {
        event.kind() == DiagnosticEventKind::ReportDisposition
            && event.request() == Some(stale_request)
            && event.report_disposition() == Some(ReportDisposition::StaleDiscarded)
    }));
    let audit_after_release = app
        .world()
        .resource::<RouterAuthorityAudits>()
        .get(router)
        .unwrap();
    assert_recovery_r3_observable_mapping_unchanged(&audit_before_release, audit_after_release);
    assert!(stale_entities.iter().all(|entity| {
        audit_after_release.owned_entity_state(*entity) == AuditEntityState::Removed
    }));
    assert!(current_entities.iter().all(|entity| {
        audit_after_release.owned_entity_state(*entity) == AuditEntityState::PresentOwned
    }));
    assert_eq!(
        audit_after_release.owned_entity_state(old_host_mapping.top_slot()),
        AuditEntityState::Quarantined
    );
    assert_eq!(
        match app
            .world()
            .resource::<RouterSnapshots>()
            .request(router, restored)
        {
            RequestLookup::Found(snapshot) => snapshot,
            _ => panic!("the restore request remains retained after replay"),
        },
        &restored_before_release
    );
    let stale_after_release = match app
        .world()
        .resource::<RouterSnapshots>()
        .request(router, stale_request)
    {
        RequestLookup::Found(snapshot) => snapshot,
        _ => panic!("the stale request remains retained after replay"),
    };
    assert_eq!(
        stale_after_release.requested(),
        stale_before_release.requested()
    );
    assert_eq!(
        stale_after_release.matched(),
        stale_before_release.matched()
    );
    assert_eq!(stale_after_release.status(), stale_before_release.status());
    assert_eq!(stale_after_release.trace(), stale_before_release.trace());
    assert!(
        stale_after_release
            .diagnostics()
            .starts_with(stale_before_release.diagnostics())
    );
    let stale_diagnostic_delta =
        &stale_after_release.diagnostics()[stale_before_release.diagnostics().len()..];
    assert!(stale_diagnostic_delta.len() <= 1);
    assert!(stale_diagnostic_delta.iter().all(|event| {
        event.kind() == DiagnosticEventKind::ReportDisposition
            && event.request() == Some(stale_request)
            && event.report_disposition() == Some(ReportDisposition::StaleDiscarded)
    }));
    let entities_after_release = app
        .world()
        .iter_entities()
        .map(|entity| entity.id())
        .collect::<HashSet<_>>();
    assert!(entities_after_release.is_subset(&entities_before_release));
    assert_eq!(effects.load(Ordering::SeqCst), effects_before_release);
    {
        let world = app.world();
        assert_eq!(
            testing_router_request_inventory(
                world.resource::<RouterNavigation>(),
                world.resource::<RouterSnapshots>(),
            ),
            inventory_before_release
        );
    }
    assert_eq!(audit_component_manifest(&app, host), host_manifest);
    assert_eq!(audit_component_manifest(&app, first_child), first_manifest);
    assert_eq!(
        audit_component_manifest(&app, second_child),
        second_manifest
    );
    assert_eq!(audit_component_manifest(&app, foreign), foreign_manifest);
    assert_eq!(
        app.world().get::<RecoveryAuditHostSentinel>(host),
        Some(&RecoveryAuditHostSentinel(0x731))
    );
    assert_eq!(
        app.world().get::<RecoveryAuditChildSentinel>(first_child),
        Some(&RecoveryAuditChildSentinel(0x732))
    );
    assert_eq!(
        app.world().get::<RecoveryAuditChildSentinel>(second_child),
        Some(&RecoveryAuditChildSentinel(0x733))
    );
    assert_eq!(
        app.world().get::<RecoveryAuditChildSentinel>(foreign),
        Some(&RecoveryAuditChildSentinel(0x734))
    );
    assert_eq!(
        app.world()
            .get::<Children>(host)
            .unwrap()
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![first_child, second_child, new_top_slot]
    );
    assert!(app.world().get::<ChildOf>(old_top_slot).is_none());
    assert_eq!(
        app.world().get::<ChildOf>(foreign).map(ChildOf::parent),
        Some(old_top_slot)
    );
    assert!(snapshot_after_release.running_request().is_none());
    assert!(snapshot_after_release.pending_phase().is_none());
    assert!(!snapshot_after_release.cleanup().foreground_pending());
    assert_eq!(snapshot_after_release.cleanup().quarantine_count(), 1);
}
