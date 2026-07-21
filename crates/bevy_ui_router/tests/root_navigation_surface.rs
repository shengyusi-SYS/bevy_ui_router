#![cfg(feature = "ecs")]

use bevy_app::App;
use bevy_ui::Node;
use bevy_ui_router::prelude::*;

struct RootDriver {
    key: ViewDriverKey,
}

impl EcsViewDriver for RootDriver {
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
                SolidRect::new(32, 24, ContentColor::srgba8(32, 64, 96, 255)).unwrap(),
            )
            .expect("solid rect should stage");
        content
            .add_child(root, rect)
            .expect("content edge should stage");
        report.applied()
    }
}

#[test]
fn root_push_is_reachable_only_through_the_public_facade() {
    let mut app = App::new();
    app.add_plugins(BevyUiRouterPlugin::default());

    let driver = ViewDriverKey::new("root-driver").unwrap();
    let registered = app
        .register_ui_view_driver(RootDriver {
            key: driver.clone(),
        })
        .unwrap();
    let view = RouteViewKey::new("root").unwrap();
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
        .insert(view.clone(), RouteViewDescriptor::new(registered))
        .unwrap();
    let router = app
        .add_ui_router(RouterConfig::new(routes, views, view))
        .unwrap();

    app.world_mut()
        .spawn((Node::default(), RouterHostRegistration::new(router)));
    app.update();

    let request = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, RouteLocation::new("/"))
        .unwrap();
    for _ in 0..8 {
        app.update();
    }

    let snapshots = app.world().resource::<RouterSnapshots>();
    let RequestLookup::Found(snapshot) = snapshots.request(router, request) else {
        panic!("root request should be retained")
    };
    assert_eq!(snapshot.requested(), Some(&RouteLocation::new("/")));
}
