use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::AssetPlugin;
use bevy_scene::{ScenePlugin, WorldSceneExt, bsn};
use bevy_ui_router::prelude::DefaultOutlet;

// Declaration compatibility only. Native Router registration/publication is
// validated separately when the phase C realization path is enabled.
#[test]
fn facade_outlet_declaration_is_usable_in_an_official_scene() {
    let mut app = App::new();
    app.add_plugins((
        TaskPoolPlugin::default(),
        AssetPlugin::default(),
        ScenePlugin,
    ));
    let root = app
        .world_mut()
        .spawn_scene(bsn! { DefaultOutlet })
        .unwrap()
        .id();
    assert!(app.world().get::<DefaultOutlet>(root).is_some());
}
