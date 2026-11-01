//! Page navigation, shared layouts, history, and state retention for Bevy UI.
//!
//! # Start here
//!
//! Enable **`bsn`** for the recommended Bevy Scene integration, then import
//! [`prelude`]. Declare pages with `page`, `layout`, and `index`; install a
//! `BsnRouter` with `AppBsnExt::add_bsn_router`, and attach its handle to an
//! application-owned UI container using `RouterHostRegistration`.
//! Use `initial` for the first page and `commands.router(handle)` for subsequent
//! Push, Replace, and Back operations. `RouteInputs` reads the current context of
//! active content, including reused layouts and reactivated pages.
//!
//! The application supplies Bevy's asset/Scene runtime, UI layout, window, camera,
//! and page content. See the [integration guide] for a complete manifest and setup,
//! or run the [quick start] without a window. The [debugging guide] covers missing
//! pages, failed requests, retention, and scroll restoration.
//!
//! # Feature selection
//!
//! - `bsn`: Scene declarations and page navigation; includes `ecs` and `view`.
//! - `tween`: optional transition coordination with application-defined effects.
//! - `desktop-back` / `android-back`: optional platform Back input adapters.
//! - `view` (default): renderer-neutral view descriptions and registration.
//! - `ecs`: Bevy execution for applications assembling their own view drivers.
//! - `platform`: platform navigation contracts. [`core`] is always available.
//!
//! Published documentation is configured to include all features. Enable the
//! relevant features in your application manifest. For local documentation, use
//! `cargo doc -p bevy_ui_router --all-features --no-deps`.
//!
//! [integration guide]: https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/docs/guides/integration.en.md
//! [quick start]: https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/crates/bevy_ui_router/examples/bsn_quick_start.rs
//! [debugging guide]: https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/docs/guides/debugging.en.md

#![forbid(unsafe_code)]

/// Renderer-independent route matching, history, lifecycle, and diagnostics.
pub mod core {
    pub use bevy_ui_router_core::back::{BackCapabilitySnapshot, BackIntent, BackPolicy};
    pub use bevy_ui_router_core::branch::{MatchedBranch, MatchedRoute, RouteParams};
    pub use bevy_ui_router_core::branch_diff::{BranchDiff, BranchDiffRoute};
    pub use bevy_ui_router_core::diagnostics::{
        DiagnosticEvent, DiagnosticEventKind, DiagnosticSeverity, DiagnosticSnapshot,
        ReportDisposition,
    };
    pub use bevy_ui_router_core::error::{RouteError, RouteErrorKind, RouteErrorSourceCategory};
    pub use bevy_ui_router_core::fingerprint::{
        RouteBranchFingerprint, RouteParamsFingerprint, RouteSearchFingerprint,
        RouteSegmentReuseKey,
    };
    pub use bevy_ui_router_core::frame::{FrameOrigin, RouteFrameStatus, RouteFrameVersion};
    pub use bevy_ui_router_core::history::{HistoryError, HistorySnapshot, MemoryHistory};
    pub use bevy_ui_router_core::ids::{
        NavigationRequestId, RouteFrameId, RouteGeneration, RouteHostId, RouteId, RouteSlotId,
        RouterInstanceId,
    };
    pub use bevy_ui_router_core::location::RouteLocation;
    pub use bevy_ui_router_core::matcher::RouteMatcher;
    pub use bevy_ui_router_core::meta::{ErrorBoundaryKey, RouteExplicitKey, RouteMeta};
    pub use bevy_ui_router_core::pattern::{RoutePathSegment, RoutePattern, RoutePatternKind};
    pub use bevy_ui_router_core::presentation::{
        ActivePresentation, ActivePresentationKind, RouterAvailability, RouterUnavailableReason,
    };
    pub use bevy_ui_router_core::request::{
        NavigationFailure, NavigationFailureKind, NavigationKind, NavigationPhase, RequestStatus,
    };
    pub use bevy_ui_router_core::route_tree::{
        RouteNode, RouteNodeOptions, RouteTree, RouteTreeBuilder,
    };
    pub use bevy_ui_router_core::slot::RouteSlotKey;
    pub use bevy_ui_router_core::trace::{
        CleanupDisposition, MountRejectionKind, NavigationTraceEvent, NavigationTraceEventKind,
    };
    pub use bevy_ui_router_core::view_input::{RouteInputRef, RouteViewInput};
    pub use bevy_ui_router_core::view_key::{RouteViewKey, RouteViewKeyError};
    pub use bevy_ui_router_core::{
        CleanupSummary, HostSnapshot, HostTraceEvent, HostTraceEventKind, HostVersion,
    };
}

#[cfg(feature = "view")]
/// Renderer-neutral view descriptions and registration (`view` feature).
pub mod view {
    pub use bevy_ui_router_view::{
        RouteViewDescriptor, RouteViewRegistry, RouteViewRegistryError, ViewBuildEffect,
        ViewDriverKey, ViewDriverKeyError, ViewRequestId,
    };
}

#[cfg(feature = "ecs")]
/// Bevy execution, navigation commands, current inputs, scrolling, and transitions.
pub mod ecs {
    pub use bevy_ui_router_ecs::{
        AppRouterExt, BevyUiRouterPlugin, NavigationCommandError, NavigationEnqueueError,
        NavigationEnqueueErrorKind, NavigationRequestSnapshot, RequestLookup, RouteActivity,
        RouteInputs, RouteNavigationCommandsExt, RouteScroll, RouteScrollCommandsExt,
        RouteScrollRestoration, RouteScrollRestorations, RouteScrollRestoreId,
        RouteScrollRestorePolicy, RouteTransitionCommandsExt, RouteTransitionContext,
        RouteTransitionDriver, RouteTransitionId, RouteTransitionTarget, RouterCommands,
        RouterConfig, RouterHandle, RouterHostRegistration, RouterInputGate, RouterInputSet,
        RouterInstallError, RouterInstallErrorKind, RouterNavigation, RouterScheduleSet,
        RouterScrollSet, RouterSnapshot, RouterSnapshots, TransitionCancelReason,
        TransitionPreparation, ViewDriverRegistrationError, ViewDriverRegistrationErrorKind,
    };
    #[cfg(feature = "tween")]
    pub use bevy_ui_router_ecs::{RouteTweenDriver, RouteTweenPlugin, RouteTweenTracks};
}

#[cfg(feature = "bsn")]
/// Recommended Scene declarations: start with [`BsnRouter`](bsn::BsnRouter), [`layout`](bsn::layout), and [`page`](bsn::page).
pub mod bsn {
    pub use bevy_ui_router_bsn::{BsnBuildError, BsnSceneFactory, DefaultOutlet};
    pub use bevy_ui_router_ecs::{
        AppBsnExt, BsnRoute, BsnRouter, BsnRouterError, index, layout, page,
    };
}

#[cfg(feature = "platform")]
/// Platform navigation intents and optional desktop/Android input adapters.
pub mod platform {
    pub use bevy_ui_router_platform::*;
}

/// Common imports, enabled according to the crate's selected features.
pub mod prelude {
    #[cfg(feature = "tween")]
    pub use bevy_ui_router_ecs::{RouteTweenDriver, RouteTweenPlugin, RouteTweenTracks};
    #[cfg(feature = "android-back")]
    pub use bevy_ui_router_platform::android_back::AndroidBackInputPlugin;
    #[cfg(any(feature = "desktop-back", feature = "android-back"))]
    pub use bevy_ui_router_platform::back_input::{PlatformBackInputSet, PlatformBackRequested};
    #[cfg(feature = "desktop-back")]
    pub use bevy_ui_router_platform::desktop_back::{
        DesktopBackInputPlugin, DesktopBackInputSet, DesktopBackRequested,
    };

    #[cfg(feature = "bsn")]
    pub use crate::bsn::{
        AppBsnExt, BsnBuildError, BsnRoute, BsnRouter, BsnRouterError, BsnSceneFactory,
        DefaultOutlet, index, layout, page,
    };

    pub use crate::core::{
        ActivePresentation, ActivePresentationKind, BackCapabilitySnapshot, BackIntent, BackPolicy,
        BranchDiff, BranchDiffRoute, CleanupDisposition, CleanupSummary, DiagnosticSeverity,
        DiagnosticSnapshot, ErrorBoundaryKey, FrameOrigin, HistoryError, HistorySnapshot,
        HostSnapshot, HostTraceEvent, HostTraceEventKind, HostVersion, MatchedBranch, MatchedRoute,
        MemoryHistory, MountRejectionKind, NavigationFailure, NavigationFailureKind,
        NavigationKind, NavigationPhase, NavigationRequestId, NavigationTraceEvent,
        NavigationTraceEventKind, ReportDisposition, RequestStatus, RouteBranchFingerprint,
        RouteError, RouteErrorKind, RouteErrorSourceCategory, RouteExplicitKey, RouteFrameId,
        RouteFrameStatus, RouteFrameVersion, RouteGeneration, RouteHostId, RouteId, RouteInputRef,
        RouteLocation, RouteMatcher, RouteMeta, RouteNodeOptions, RouteParams, RoutePathSegment,
        RoutePattern, RoutePatternKind, RouteSlotId, RouteSlotKey, RouteTree, RouteTreeBuilder,
        RouteViewInput, RouteViewKey, RouteViewKeyError, RouterAvailability, RouterInstanceId,
        RouterUnavailableReason,
    };

    #[cfg(feature = "view")]
    pub use crate::view::{
        RouteViewDescriptor, RouteViewRegistry, RouteViewRegistryError, ViewBuildEffect,
        ViewDriverKey, ViewDriverKeyError, ViewRequestId,
    };

    #[cfg(feature = "ecs")]
    pub use crate::ecs::{
        AppRouterExt, BevyUiRouterPlugin, NavigationCommandError, NavigationEnqueueError,
        NavigationEnqueueErrorKind, NavigationRequestSnapshot, RequestLookup, RouteActivity,
        RouteInputs, RouteNavigationCommandsExt, RouteScroll, RouteScrollCommandsExt,
        RouteScrollRestoration, RouteScrollRestorations, RouteScrollRestoreId,
        RouteScrollRestorePolicy, RouteTransitionCommandsExt, RouteTransitionContext,
        RouteTransitionDriver, RouteTransitionId, RouteTransitionTarget, RouterCommands,
        RouterConfig, RouterHandle, RouterHostRegistration, RouterInputGate, RouterInputSet,
        RouterInstallError, RouterInstallErrorKind, RouterNavigation, RouterScheduleSet,
        RouterScrollSet, RouterSnapshot, RouterSnapshots, TransitionCancelReason,
        TransitionPreparation, ViewDriverRegistrationError, ViewDriverRegistrationErrorKind,
    };
}
