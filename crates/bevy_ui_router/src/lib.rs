#![forbid(unsafe_code)]

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
pub mod view {
    pub use bevy_ui_router_view::{
        RouteViewDescriptor, RouteViewRegistry, RouteViewRegistryError, ViewBuildEffect,
        ViewDriverKey, ViewDriverKeyError, ViewRequestId,
    };
}

#[cfg(feature = "ecs")]
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
pub mod bsn {
    pub use bevy_ui_router_bsn::{BsnBuildError, BsnSceneFactory, DefaultOutlet};
    pub use bevy_ui_router_ecs::{
        AppBsnExt, BsnRoute, BsnRouter, BsnRouterError, index, layout, page,
    };
}

#[cfg(feature = "platform")]
pub mod platform {
    pub use bevy_ui_router_platform::*;
}

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
