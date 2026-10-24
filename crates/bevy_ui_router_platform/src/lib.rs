#![forbid(unsafe_code)]

#[cfg(feature = "desktop-back")]
pub mod desktop_back;

pub mod prelude {
    pub use bevy_ui_router_core::prelude::{BackCapabilitySnapshot, BackIntent, BackPolicy};

    #[cfg(feature = "desktop-back")]
    pub use crate::desktop_back::{
        DesktopBackInputPlugin, DesktopBackInputSet, DesktopBackRequested,
    };
}
