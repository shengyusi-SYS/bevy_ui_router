//! Shared window-level input, without a routing or operating-system decision.

use bevy_app::{App, PreUpdate};
use bevy_ecs::{
    entity::Entity,
    message::Message,
    schedule::{IntoScheduleConfigs, SystemSet},
};
use bevy_input::InputSystems;

/// A supported Back input delivered to a window.
///
/// Use one application handler to choose the window's Router and call its public
/// `back` command, or handle local UI first. Reading this message does not consume
/// it for other readers. No Router is selected, window closed, or event delegated
/// to the operating system by the input plugins.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlatformBackRequested {
    pub window: Entity,
}

/// Produces [`PlatformBackRequested`] in `PreUpdate`, after Bevy's input systems.
///
/// Run the application's return handler after this set in the same schedule to
/// enqueue navigation before the Router's ordinary `Update` processing. Each
/// source preserves its input order; there is no ordering between sources.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, SystemSet)]
pub struct PlatformBackInputSet;

pub(crate) fn install(app: &mut App) {
    app.add_message::<PlatformBackRequested>()
        .configure_sets(PreUpdate, PlatformBackInputSet.after(InputSystems));
}
