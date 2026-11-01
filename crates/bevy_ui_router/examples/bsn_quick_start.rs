//! A finite, headless BSN navigation. A windowed application uses App::run.
use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::AssetPlugin;
use bevy_scene::{ScenePlugin, bsn};
use bevy_ui::{Node, Val};
use bevy_ui_router::prelude::*;

fn main() {
    let mut app = App::new();
    // Code-authored Scenes use Bevy's ordinary asset and Scene runtime.
    app.add_plugins((
        TaskPoolPlugin::default(),
        AssetPlugin::default(),
        ScenePlugin,
    ));
    let router = app
        .add_bsn_router(
            BsnRouter::new(page("/", |_| {
                Ok(bsn! { Node { width: Val::Px(240.0), height: Val::Px(120.0) } })
            }))
            .fallback(|input| {
                eprintln!("Navigation failed: {:?}", input.failure());
                Ok(bsn! { Node })
            })
            .initial("/"),
        )
        .unwrap();
    // The application owns the host; Router manages its routed descendants.
    app.world_mut()
        .spawn((Node::default(), RouterHostRegistration::new(router)));

    // Only this finite example polls. Interactive applications call app.run().
    for _ in 0..32 {
        app.update();
        let snapshot = app
            .world()
            .resource::<RouterSnapshots>()
            .get(router)
            .unwrap();
        if snapshot.active().kind() == ActivePresentationKind::Error {
            panic!("navigation failed: {:?}", snapshot.active().failure());
        }
        if snapshot.active().location() == Some(&RouteLocation::new("/"))
            && snapshot.running_request().is_none()
        {
            assert_eq!(snapshot.history().len(), 1);
            assert!(!snapshot.cleanup().foreground_pending());
            println!("Succeeded: {:?}", snapshot.active().location());
            return;
        }
    }
    panic!("initial navigation did not settle within 32 updates");
}
