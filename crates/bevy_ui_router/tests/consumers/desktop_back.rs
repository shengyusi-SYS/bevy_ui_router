// Independent desktop-back-only consumer: no Router ECS, UI or window runner.
use bevy_app::{App, PreUpdate};
use bevy_ecs::prelude::*;
use bevy_input::{ButtonState, mouse::{MouseButton, MouseButtonInput}};
use bevy_ui_router::platform::prelude::*;

#[derive(Resource, Default)]
struct Requests(Vec<Entity>);

fn handle(mut input: MessageReader<DesktopBackRequested>, mut seen: ResMut<Requests>) {
    seen.0.extend(input.read().map(|request| request.window));
}

fn main() {
    let mut app = App::new();
    app.add_plugins(DesktopBackInputPlugin)
        .init_resource::<Requests>()
        .add_systems(PreUpdate, handle.after(DesktopBackInputSet));
    let window = app.world_mut().spawn_empty().id();
    for state in [ButtonState::Pressed, ButtonState::Released] {
        app.world_mut().write_message(MouseButtonInput {
            window, state, button: MouseButton::Back,
        });
    }
    app.update();
    app.update();
    assert_eq!(app.world().resource::<Requests>().0, [window]);
}
