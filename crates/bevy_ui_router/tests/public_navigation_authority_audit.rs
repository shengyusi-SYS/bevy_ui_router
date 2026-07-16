use bevy_app::App;
use bevy_ui::Node;
use bevy_ui_router::prelude::*;

struct AuditDriver {
    key: ViewDriverKey,
}

impl EcsViewDriver for AuditDriver {
    fn key(&self) -> ViewDriverKey {
        self.key.clone()
    }

    fn apply(
        &mut self,
        _effect: &ViewBuildEffect,
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
