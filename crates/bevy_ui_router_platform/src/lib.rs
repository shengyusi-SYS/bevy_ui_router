#![forbid(unsafe_code)]

#[cfg(feature = "android-back")]
pub mod android_back;
#[cfg(any(feature = "desktop-back", feature = "android-back"))]
pub mod back_input;
#[cfg(feature = "desktop-back")]
pub mod desktop_back;

pub mod prelude {
    pub use bevy_ui_router_core::prelude::{BackCapabilitySnapshot, BackIntent, BackPolicy};

    #[cfg(feature = "android-back")]
    pub use crate::android_back::AndroidBackInputPlugin;
    #[cfg(any(feature = "desktop-back", feature = "android-back"))]
    pub use crate::back_input::{PlatformBackInputSet, PlatformBackRequested};
    #[cfg(feature = "desktop-back")]
    pub use crate::desktop_back::{
        DesktopBackInputPlugin, DesktopBackInputSet, DesktopBackRequested,
    };
}
