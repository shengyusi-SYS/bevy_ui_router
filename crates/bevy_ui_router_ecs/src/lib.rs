#![forbid(unsafe_code)]

mod active_scope;
mod activity;
mod app_ext;
#[cfg(test)]
mod authority_regression_tests;
mod branch_transaction;
#[cfg(feature = "bsn")]
mod bsn;
mod cleanup;
mod components;
mod content;
mod driver;
mod frame_local;
mod host;
mod input;
mod lifecycle_preparation;
mod mount;
#[cfg(feature = "bsn")]
mod native_focus;
mod navigation;
mod plugin;
mod presentation;
mod registry;
mod retained;
mod route_inputs;
mod scroll;
mod snapshots;
#[cfg(test)]
mod task10_regression_tests;
#[cfg(test)]
mod test_support;

pub use activity::RouteActivity;
pub use app_ext::{
    AppRouterExt, RouterConfig, RouterHandle, RouterHostRegistration, RouterInstallError,
    RouterInstallErrorKind, ViewDriverRegistrationError, ViewDriverRegistrationErrorKind,
};
#[cfg(feature = "bsn")]
pub use bsn::AppBsnExt;
pub use input::RouterInputGate;
pub use navigation::{NavigationEnqueueError, NavigationEnqueueErrorKind, RouterNavigation};
pub use plugin::{BevyUiRouterPlugin, RouterInputSet, RouterScheduleSet};
pub use route_inputs::RouteInputs;
pub use scroll::RouteScroll;
pub use snapshots::{NavigationRequestSnapshot, RequestLookup, RouterSnapshot, RouterSnapshots};

pub mod prelude {
    #[cfg(feature = "bsn")]
    pub use crate::AppBsnExt;
    pub use crate::{
        AppRouterExt, BevyUiRouterPlugin, NavigationEnqueueError, NavigationEnqueueErrorKind,
        NavigationRequestSnapshot, RequestLookup, RouteActivity, RouteInputs, RouteScroll,
        RouterConfig, RouterHandle, RouterHostRegistration, RouterInputGate, RouterInputSet,
        RouterInstallError, RouterInstallErrorKind, RouterNavigation, RouterScheduleSet,
        RouterSnapshot, RouterSnapshots, ViewDriverRegistrationError,
        ViewDriverRegistrationErrorKind,
    };
}
