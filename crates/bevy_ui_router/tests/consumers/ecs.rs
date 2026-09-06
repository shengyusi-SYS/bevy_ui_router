// Migration consumer: this independent renderer is replaced by native BSN in phase C.
// It calls the production Router through the facade, without test-support or owner crates.
use bevy_app::App;
use bevy_ecs::prelude::Component;
use bevy_ui::{Node, Val};
use bevy_ui_router::prelude::*;

#[derive(Component)]
struct HostSentinel(u32);

struct Renderer(ViewDriverKey);

impl EcsViewDriver for Renderer {
    fn key(&self) -> ViewDriverKey {
        self.0.clone()
    }

    fn apply(
        &mut self,
        _effect: &ViewBuildEffect,
        content: &mut FrameContentWriter<'_>,
        report: EcsViewReportSession<'_>,
    ) -> ViewBuildReport {
        content
            .spawn_solid_rect(SolidRect::new(37, 19, ContentColor::srgba8(1, 2, 3, 255)).unwrap())
            .unwrap();
        report.applied()
    }
}

fn settle(app: &mut App, router: RouterHandle, request: NavigationRequestId) {
    for _ in 0..32 {
        app.update();
        let snapshots = app.world().resource::<RouterSnapshots>();
        if let RequestLookup::Found(snapshot) = snapshots.request(router, request) {
            if matches!(
                snapshot.status(),
                RequestStatus::Succeeded | RequestStatus::Failed(_)
            ) {
                return;
            }
        }
    }
    panic!("public navigation did not settle through ordinary App::update");
}

fn main() {
    let mut app = App::new();
    app.add_plugins(BevyUiRouterPlugin::default());
    let driver = app
        .register_ui_view_driver(Renderer(
            ViewDriverKey::new("independent-renderer").unwrap(),
        ))
        .unwrap();
    let normal = RouteViewKey::new("normal").unwrap();
    let fallback = RouteViewKey::new("fallback").unwrap();
    let mut views = RouteViewRegistry::new();
    for view in [normal.clone(), fallback.clone()] {
        views
            .insert(view, RouteViewDescriptor::new(driver.clone()))
            .unwrap();
    }
    let routes = RouteTree::builder()
        .root_with_options(
            RoutePattern::path("/").unwrap(),
            RouteNodeOptions::new().with_view(normal),
        )
        .unwrap()
        .build()
        .unwrap();
    let router = app
        .add_ui_router(RouterConfig::new(routes, views, fallback))
        .unwrap();
    let host = app
        .world_mut()
        .spawn((
            Node::default(),
            HostSentinel(7),
            RouterHostRegistration::new(router),
        ))
        .id();
    app.update();
    let request = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, RouteLocation::new("/"))
        .unwrap();
    settle(&mut app, router, request);
    let snapshots = app.world().resource::<RouterSnapshots>();
    let RequestLookup::Found(snapshot) = snapshots.request(router, request) else {
        panic!("missing request")
    };
    assert_eq!(snapshot.status(), &RequestStatus::Succeeded);
    assert_eq!(
        snapshots.get(router).unwrap().history().current(),
        Some(&RouteLocation::new("/"))
    );
    let mut nodes = app.world_mut().query::<&Node>();
    assert!(
        nodes
            .iter(app.world())
            .any(|node| node.width == Val::Px(37.0))
    );

    let missing = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, RouteLocation::new("/missing"))
        .unwrap();
    settle(&mut app, router, missing);
    let snapshots = app.world().resource::<RouterSnapshots>();
    let RequestLookup::Found(snapshot) = snapshots.request(router, missing) else {
        panic!("missing failure")
    };
    assert!(matches!(snapshot.status(), RequestStatus::Failed(_)));
    assert_eq!(
        snapshots.get(router).unwrap().history().current(),
        Some(&RouteLocation::new("/"))
    );
    assert_eq!(app.world().get::<HostSentinel>(host).unwrap().0, 7);
}
