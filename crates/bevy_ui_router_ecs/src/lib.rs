#![forbid(unsafe_code)]

mod components;
mod plugin;
mod state;

pub use components::{
    RouteFocusSnapshot, RouteFrameInteractionSnapshot, RouteFrameRoot,
    RouteFrameRootLifecycleSnapshot, RouteHost, RouteSlot,
};
pub use plugin::{BevyUiRouterPlugin, RouterScheduleSet};
pub use state::RouterEcsState;

pub mod prelude {
    pub use crate::{
        BevyUiRouterPlugin, RouteFocusSnapshot, RouteFrameInteractionSnapshot, RouteFrameRoot,
        RouteFrameRootLifecycleSnapshot, RouteHost, RouteSlot, RouterEcsState, RouterScheduleSet,
    };
}
