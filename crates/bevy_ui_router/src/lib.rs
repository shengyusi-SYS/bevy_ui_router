#![forbid(unsafe_code)]

pub use bevy_ui_router_core as core;

#[cfg(feature = "view")]
pub use bevy_ui_router_view as view;

#[cfg(feature = "ecs")]
pub use bevy_ui_router_ecs as ecs;

#[cfg(feature = "bsn")]
pub use bevy_ui_router_bsn as bsn;

#[cfg(feature = "platform")]
pub use bevy_ui_router_platform as platform;

pub mod prelude {
    #[allow(unused_imports)]
    pub use bevy_ui_router_core::prelude::*;

    #[cfg(feature = "view")]
    #[allow(unused_imports)]
    pub use bevy_ui_router_view::prelude::*;
}
