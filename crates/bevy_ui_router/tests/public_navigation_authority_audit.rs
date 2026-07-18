use bevy_app::App;
use bevy_ui::Node;
use bevy_ui_router::prelude::*;

struct AuditDriver {
    key: ViewDriverKey,
    outlet_view: Option<RouteViewKey>,
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
        let root = content.spawn_empty().expect("root content should stage");
        let rect = content
            .spawn_solid_rect(
                SolidRect::new(37, 19, ContentColor::srgba8(11, 23, 47, 255)).unwrap(),
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
