#![forbid(unsafe_code)]

mod app_ext;
mod bridge;
pub(crate) mod cleanup;
mod components;
mod content;
mod driver;
pub(crate) mod finalize;
mod navigation;
mod plugin;
mod snapshots;
mod state;

pub use app_ext::{
    RouterConfig, RouterHandle, RouterHostRegistration, RouterInstallError, RouterInstallErrorKind,
    ViewDriverRegistrationError, ViewDriverRegistrationErrorKind,
};
pub use bridge::{BeginFrameApplySessionError, FinishFrameApplySessionError};
pub use bridge::{EcsFrameApplyReport, EcsFrameApplySession, EcsFrameApplyStatus};
pub use components::{
    RouteFocusSnapshot, RouteFrameInteractionSnapshot, RouteFrameRoot,
    RouteFrameRootLifecycleSnapshot, RouteHost, RouteSlot,
};
pub use navigation::{NavigationEnqueueError, NavigationEnqueueErrorKind, RouterNavigation};
pub use plugin::{BevyUiRouterPlugin, RouterInputSet, RouterScheduleSet};
pub use snapshots::{NavigationRequestSnapshot, RequestLookup, RouterSnapshot, RouterSnapshots};
pub use state::{CompleteFrameApplySessionError, RouterEcsState};

pub mod prelude {
    pub use crate::{
        BevyUiRouterPlugin, NavigationEnqueueError, NavigationEnqueueErrorKind,
        NavigationRequestSnapshot, RequestLookup, RouteFocusSnapshot,
        RouteFrameInteractionSnapshot, RouteFrameRoot, RouteFrameRootLifecycleSnapshot, RouteHost,
        RouteSlot, RouterConfig, RouterEcsState, RouterHandle, RouterHostRegistration,
        RouterInputSet, RouterInstallError, RouterInstallErrorKind, RouterNavigation,
        RouterScheduleSet, RouterSnapshot, RouterSnapshots, ViewDriverRegistrationError,
        ViewDriverRegistrationErrorKind,
    };
}
