#![forbid(unsafe_code)]

mod bridge;
pub(crate) mod cleanup;
mod components;
mod plugin;
mod state;

pub use bridge::{BeginFrameApplySessionError, FinishFrameApplySessionError};
pub use bridge::{EcsFrameApplyReport, EcsFrameApplySession, EcsFrameApplyStatus};
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
