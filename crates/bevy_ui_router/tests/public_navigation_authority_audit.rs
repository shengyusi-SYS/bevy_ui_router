use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

use bevy_app::{App, Update};
use bevy_camera::visibility::Visibility;
use bevy_ecs::{
    component::{Component, ComponentId},
    hierarchy::{ChildOf, Children},
    schedule::IntoScheduleConfigs,
    world::World,
};
use bevy_picking::Pickable;
use bevy_ui::{Node, Val};
use bevy_ui_router::ecs::RouterScheduleSet;
use bevy_ui_router::prelude::*;
use bevy_ui_router_ecs::test_support::{
    BackCapacityOrderEvent, BackCapacityOrderInstallError, testing_back_request_inventory,
    testing_install_back_capacity_order_observer,
};

const ROOT_RECT_WIDTH: u16 = 37;
const LAYOUT_RECT_WIDTH: u16 = 39;
const LEAF_RECT_WIDTH: u16 = 41;

struct AuditDriver {
    key: ViewDriverKey,
    outlet_view: Option<RouteViewKey>,
}

#[derive(Component, Debug, Eq, PartialEq)]
struct RecoveryAuditHostSentinel(u64);

#[derive(Component, Debug, Eq, PartialEq)]
struct RecoveryAuditChildSentinel(u64);

struct RecoveryAuditDriver {
    key: ViewDriverKey,
    failed_view: Option<RouteViewKey>,
    error_view: RouteViewKey,
    fallback_view: RouteViewKey,
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
