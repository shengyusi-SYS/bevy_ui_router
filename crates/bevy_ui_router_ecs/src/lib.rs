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
mod instance;
mod lifecycle_preparation;
mod mount;
#[cfg(feature = "bsn")]
mod native_focus;
mod navigation;
mod navigation_commands;
mod plugin;
mod presentation;
mod read_observation;
mod registry;
mod retained;
mod route_inputs;
mod route_queries;
mod scroll;
mod snapshots;
#[cfg(test)]
mod task10_regression_tests;
#[cfg(test)]
mod test_support;
mod transition;

pub use activity::RouteActivity;
pub use app_ext::{
    AppRouterExt, RouterConfig, RouterHandle, RouterHostRegistration, RouterInstallError,
    RouterInstallErrorKind, ViewDriverRegistrationError, ViewDriverRegistrationErrorKind,
};
#[cfg(feature = "bsn")]
pub use bsn::{AppBsnExt, BsnRoute, BsnRouter, BsnRouterError, index, layout, page};
pub use input::RouterInputGate;
pub use navigation::{NavigationEnqueueError, NavigationEnqueueErrorKind, RouterNavigation};
pub use navigation_commands::{NavigationCommandError, RouteNavigationCommandsExt, RouterCommands};
pub use plugin::{BevyUiRouterPlugin, RouterInputSet, RouterScheduleSet, RouterScrollSet};
pub use route_inputs::RouteInputs;
pub use route_queries::{RouteQueries, RouteQueryReader};
pub use scroll::{
    RouteScroll, RouteScrollCommandsExt, RouteScrollRestoration, RouteScrollRestorations,
    RouteScrollRestoreId, RouteScrollRestorePolicy,
};
pub use snapshots::{NavigationRequestSnapshot, RequestLookup, RouterSnapshot, RouterSnapshots};
#[cfg(feature = "tween")]
pub use transition::tween::{RouteTweenDriver, RouteTweenPlugin, RouteTweenTracks};
pub use transition::{
    RouteTransitionCommandsExt, RouteTransitionContext, RouteTransitionDriver, RouteTransitionId,
    RouteTransitionTarget, TransitionCancelReason, TransitionPreparation,
};

pub mod prelude {
    #[cfg(feature = "bsn")]
    pub use crate::{AppBsnExt, BsnRoute, BsnRouter, BsnRouterError, index, layout, page};
    pub use crate::{
        AppRouterExt, BevyUiRouterPlugin, NavigationCommandError, NavigationEnqueueError,
        NavigationEnqueueErrorKind, NavigationRequestSnapshot, RequestLookup, RouteActivity,
        RouteInputs, RouteNavigationCommandsExt, RouteQueries, RouteQueryReader, RouteScroll,
        RouteScrollCommandsExt, RouteScrollRestoration, RouteScrollRestorations,
        RouteScrollRestoreId, RouteScrollRestorePolicy, RouteTransitionCommandsExt,
        RouteTransitionContext, RouteTransitionDriver, RouteTransitionId, RouteTransitionTarget,
        RouterCommands, RouterConfig, RouterHandle, RouterHostRegistration, RouterInputGate,
        RouterInputSet, RouterInstallError, RouterInstallErrorKind, RouterNavigation,
        RouterScheduleSet, RouterScrollSet, RouterSnapshot, RouterSnapshots,
        TransitionCancelReason, TransitionPreparation, ViewDriverRegistrationError,
        ViewDriverRegistrationErrorKind,
    };
    #[cfg(feature = "tween")]
    pub use crate::{RouteTweenDriver, RouteTweenPlugin, RouteTweenTracks};
}
