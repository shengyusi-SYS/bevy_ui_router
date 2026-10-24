//! Window-level mouse Back input. The application owns the return policy.

use bevy_app::{App, Plugin, PreUpdate};
use bevy_ecs::{
    entity::Entity,
    message::{Message, MessageReader, MessageWriter},
    schedule::{IntoScheduleConfigs, SystemSet},
};
use bevy_input::{
    ButtonState, InputSystems,
    mouse::{MouseButton, MouseButtonInput},
};

/// A mouse Back press delivered to a window, without a routing decision.
///
/// Use one application handler to choose the window's Router and call its public
/// `back` command, or handle local UI first. Reading this message does not consume
/// it for other readers. No Router is selected and no window is closed by this plugin.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct DesktopBackRequested {
    pub window: Entity,
}

/// Produces [`DesktopBackRequested`] in `PreUpdate`, after Bevy's input systems.
///
/// Run the application's return handler after this set in the same schedule to
/// enqueue navigation before the Router's ordinary `Update` processing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, SystemSet)]
pub struct DesktopBackInputSet;

/// Translates mouse Back presses into window-level return requests.
///
/// The application supplies its normal input/window backend. This plugin also
/// works with injected [`MouseButtonInput`] messages; it installs no window runner,
/// picking, keyboard shortcuts, or automatic navigation handler.
#[derive(Default)]
pub struct DesktopBackInputPlugin;

impl Plugin for DesktopBackInputPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<MouseButtonInput>()
            .add_message::<DesktopBackRequested>()
            .add_systems(
                PreUpdate,
                mouse_back_requests
                    .in_set(DesktopBackInputSet)
                    .after(InputSystems),
            );
    }
}

fn mouse_back_requests(
    mut mouse: MessageReader<MouseButtonInput>,
    mut requests: MessageWriter<DesktopBackRequested>,
) {
    for input in mouse.read() {
        if input.button == MouseButton::Back && input.state == ButtonState::Pressed {
            requests.write(DesktopBackRequested {
                window: input.window,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::{resource::Resource, system::ResMut};
    use bevy_input::{ButtonInput, InputPlugin};

    #[derive(Resource, Default)]
    struct Observed(Vec<Entity>);

    fn observe(mut requests: MessageReader<DesktopBackRequested>, mut seen: ResMut<Observed>) {
        seen.0.extend(requests.read().map(|request| request.window));
    }

    #[test]
    fn back_presses_preserve_windows_and_order_without_consuming_other_input() {
        // Registering before/after the existing input plugin and headless injection
        // all use the same public messages and ordinary schedule.
        for input_first in [None, Some(true), Some(false)] {
            let mut app = App::new();
            if input_first == Some(true) {
                app.add_plugins(InputPlugin);
            }
            app.add_plugins(DesktopBackInputPlugin);
            if input_first == Some(false) {
                app.add_plugins(InputPlugin);
            }
            app.init_resource::<Observed>()
                .add_systems(PreUpdate, observe.after(DesktopBackInputSet));
            let a = app.world_mut().spawn_empty().id();
            let b = app.world_mut().spawn_empty().id();
            for (window, button, state) in [
                (a, MouseButton::Back, ButtonState::Pressed),
                (a, MouseButton::Back, ButtonState::Released),
                (a, MouseButton::Forward, ButtonState::Pressed),
                (b, MouseButton::Left, ButtonState::Pressed),
                (b, MouseButton::Back, ButtonState::Pressed),
                (a, MouseButton::Back, ButtonState::Pressed),
            ] {
                app.world_mut().write_message(MouseButtonInput {
                    window,
                    button,
                    state,
                });
            }
            app.update();
            assert_eq!(app.world().resource::<Observed>().0, [a, b, a]);
            if input_first.is_some() {
                let buttons = app.world().resource::<ButtonInput<MouseButton>>();
                assert!(buttons.pressed(MouseButton::Forward));
                assert!(buttons.pressed(MouseButton::Left));
                assert!(buttons.pressed(MouseButton::Back));
            }
            app.world_mut().write_message(MouseButtonInput {
                window: b,
                button: MouseButton::Back,
                state: ButtonState::Released,
            });
            app.update();
            app.update();
            assert_eq!(app.world().resource::<Observed>().0, [a, b, a]);
        }
    }
}
