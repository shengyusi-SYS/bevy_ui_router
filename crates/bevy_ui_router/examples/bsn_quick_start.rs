//! A finite, headless BSN navigation. See the repository README for dependencies.
use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::AssetPlugin;
use bevy_scene::{ScenePlugin, bsn};
use bevy_ui::{Node, Val};
use bevy_ui_router::prelude::*;

fn main() {
    let mut app = App::new();
    // Code-authored Scenes still require Bevy's asset and Scene runtime.
    app.add_plugins((
        TaskPoolPlugin::default(),
        AssetPlugin::default(),
        ScenePlugin,
    ));

    let home_driver = app
        .register_bsn_view(ViewDriverKey::new("home").unwrap(), |_| {
            Ok(bsn! { Node { width: Val::Px(240.0), height: Val::Px(120.0) } })
        })
        .unwrap();
    let error_driver = app
        .register_bsn_view(ViewDriverKey::new("error").unwrap(), |input| {
            eprintln!("Navigation failed: {:?}", input.failure());
            Ok(bsn! { Node })
        })
        .unwrap();
    let home = RouteViewKey::new("home").unwrap();
    let error = RouteViewKey::new("error").unwrap();
    let routes = RouteTree::builder()
        .root_with_options(
            RoutePattern::path("/").unwrap(),
            RouteNodeOptions::new().with_view(home.clone()),
        )
        .unwrap()
        .build()
        .unwrap();
    let mut views = RouteViewRegistry::new();
    views
        .insert(home, RouteViewDescriptor::new(home_driver))
        .unwrap();
    views
        .insert(error.clone(), RouteViewDescriptor::new(error_driver))
        .unwrap();
    let router = app
        .add_ui_router(RouterConfig::new(routes, views, error))
        .unwrap();
    // The application owns the host; Router manages its routed descendants.
    app.world_mut()
        .spawn((Node::default(), RouterHostRegistration::new(router)));
    app.update();

    let request = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, RouteLocation::new("/"))
        .unwrap();
    // A windowed application normally advances these updates via App::run.
    for _ in 0..32 {
        app.update();
        let snapshots = app.world().resource::<RouterSnapshots>();
        if let RequestLookup::Found(result) = snapshots.request(router, request) {
            match result.status() {
                RequestStatus::Succeeded => {
                    let snapshot = snapshots.get(router).unwrap();
                    assert_eq!(snapshot.active().location(), Some(&RouteLocation::new("/")));
                    println!("Succeeded: {:?}", snapshot.active().location());
                    return;
                }
                RequestStatus::Failed(failure) => panic!("navigation failed: {failure:?}"),
                _ => {}
            }
        }
    }
    panic!("navigation did not settle within 32 updates");
}
