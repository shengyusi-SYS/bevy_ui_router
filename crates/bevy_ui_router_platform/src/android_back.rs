//! Android's traditional Back key input, delivered through Bevy's keyboard messages.

use bevy_app::{App, Plugin, PreUpdate};
use bevy_ecs::{
    message::{MessageReader, MessageWriter},
    schedule::IntoScheduleConfigs,
};
use bevy_input::{
    ButtonState,
    keyboard::{Key, KeyboardInput},
};

use crate::back_input::{PlatformBackInputSet, PlatformBackRequested};

/// Translates non-repeated `BrowserBack` / `GoBack` presses into return requests.
///
/// The application supplies the Android window/input backend and manifest opt-out
/// when required to receive legacy key events. This plugin installs no native
/// callbacks, predictive gestures, or automatic navigation/exit handler.
///
/// Bevy's keyboard message has no Android cancellation flag. Requests describe
/// key presses, not committed or cancelable Android Back callbacks. Ignoring a
/// request does not hand the event back to Android's default return behavior.
#[derive(Default)]
pub struct AndroidBackInputPlugin;

impl Plugin for AndroidBackInputPlugin {
    fn build(&self, app: &mut App) {
        crate::back_input::install(app);
        app.add_message::<KeyboardInput>().add_systems(
            PreUpdate,
            keyboard_back_requests.in_set(PlatformBackInputSet),
        );
    }
}

fn keyboard_back_requests(
    mut keyboard: MessageReader<KeyboardInput>,
    mut requests: MessageWriter<PlatformBackRequested>,
) {
    for input in keyboard.read() {
        if input.state == ButtonState::Pressed
            && !input.repeat
            && matches!(input.logical_key, Key::BrowserBack | Key::GoBack)
        {
            requests.write(PlatformBackRequested {
                window: input.window,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::{entity::Entity, resource::Resource, system::ResMut};
    use bevy_input::{
        ButtonInput, InputPlugin,
        keyboard::{KeyCode, NativeKeyCode},
    };

    #[derive(Resource, Default)]
    struct Observed(Vec<Entity>);

    fn observe(mut requests: MessageReader<PlatformBackRequested>, mut seen: ResMut<Observed>) {
        seen.0.extend(requests.read().map(|request| request.window));
    }

    #[test]
    fn logical_back_presses_filter_repeats_and_preserve_windows_and_order() {
        for input_first in [None, Some(true), Some(false)] {
            let mut app = App::new();
            if input_first == Some(true) {
                app.add_plugins(InputPlugin);
            }
            app.add_plugins(AndroidBackInputPlugin);
            if input_first == Some(false) {
                app.add_plugins(InputPlugin);
            }
            app.init_resource::<Observed>()
                .add_systems(PreUpdate, observe.after(PlatformBackInputSet));
            let a = app.world_mut().spawn_empty().id();
            let b = app.world_mut().spawn_empty().id();
            for (window, logical_key, state, repeat) in [
                (a, Key::BrowserBack, ButtonState::Pressed, false),
                (a, Key::BrowserBack, ButtonState::Pressed, true),
                (a, Key::BrowserBack, ButtonState::Released, false),
                (b, Key::Escape, ButtonState::Pressed, false),
                (b, Key::GoBack, ButtonState::Pressed, false),
                (a, Key::BrowserBack, ButtonState::Pressed, false),
            ] {
                app.world_mut().write_message(KeyboardInput {
                    window,
                    key_code: KeyCode::Unidentified(NativeKeyCode::Unidentified),
                    logical_key,
                    state,
                    repeat,
                    text: None,
                });
            }
            app.update();
            assert_eq!(app.world().resource::<Observed>().0, [a, b, a]);
            if input_first.is_some() {
                assert!(
                    app.world()
                        .resource::<ButtonInput<Key>>()
                        .pressed(Key::Escape)
                );
            }
            app.update();
            app.update();
            assert_eq!(app.world().resource::<Observed>().0, [a, b, a]);
        }
    }

    #[cfg(feature = "desktop-back")]
    #[test]
    fn desktop_and_android_share_one_application_channel() {
        use crate::desktop_back::{DesktopBackInputPlugin, DesktopBackInputSet};
        use bevy_input::mouse::{MouseButton, MouseButtonInput};

        let mut app = App::new();
        app.add_plugins((DesktopBackInputPlugin, AndroidBackInputPlugin))
            .init_resource::<Observed>()
            // The existing desktop set also orders the Android producer.
            .add_systems(PreUpdate, observe.after(DesktopBackInputSet));
        let window = app.world_mut().spawn_empty().id();
        app.world_mut().write_message(MouseButtonInput {
            window,
            button: MouseButton::Back,
            state: ButtonState::Pressed,
        });
        app.world_mut().write_message(KeyboardInput {
            window,
            key_code: KeyCode::Unidentified(NativeKeyCode::Unidentified),
            logical_key: Key::BrowserBack,
            state: ButtonState::Pressed,
            repeat: false,
            text: None,
        });
        app.update();
        assert_eq!(app.world().resource::<Observed>().0, [window, window]);
    }
}
