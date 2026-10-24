// Independent android-back-only consumer: no Router ECS, UI or Android runner.
use bevy_app::{App, PreUpdate};
use bevy_ecs::prelude::*;
use bevy_input::{ButtonState, keyboard::{Key, KeyCode, KeyboardInput, NativeKeyCode}};
use bevy_ui_router::platform::prelude::*;

#[derive(Resource, Default)]
struct Requests(Vec<Entity>);

fn handle(mut input: MessageReader<PlatformBackRequested>, mut seen: ResMut<Requests>) {
    seen.0.extend(input.read().map(|request| request.window));
}

fn main() {
    let mut app = App::new();
    app.add_plugins(AndroidBackInputPlugin)
        .init_resource::<Requests>()
        .add_systems(PreUpdate, handle.after(PlatformBackInputSet));
    let window = app.world_mut().spawn_empty().id();
    for state in [ButtonState::Pressed, ButtonState::Released] {
        app.world_mut().write_message(KeyboardInput {
            window, state, logical_key: Key::BrowserBack,
            key_code: KeyCode::Unidentified(NativeKeyCode::Unidentified),
            text: None, repeat: false,
        });
    }
    app.update();
    app.update();
    assert_eq!(app.world().resource::<Requests>().0, [window]);
}
