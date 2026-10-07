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
        NavigationFailure, NavigationFailureKind, NavigationPhase, RequestStatus,
    };
    pub use bevy_ui_router_core::route_tree::{
        RouteNode, RouteNodeOptions, RouteTree, RouteTreeBuilder,
    };
    pub use bevy_ui_router_core::slot::RouteSlotKey;
    pub use bevy_ui_router_core::trace::{
        CleanupDisposition, MountRejectionKind, NavigationTraceEvent, NavigationTraceEventKind,
    };
    pub use bevy_ui_router_core::view_input::RouteViewInput;
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
        AppRouterExt, BevyUiRouterPlugin, NavigationEnqueueError, NavigationEnqueueErrorKind,
        NavigationRequestSnapshot, RequestLookup, RouteScroll, RouterConfig, RouterHandle,
        RouterHostRegistration, RouterInputGate, RouterInputSet, RouterInstallError,
        RouterInstallErrorKind, RouterNavigation, RouterScheduleSet, RouterSnapshot,
        RouterSnapshots, ViewDriverRegistrationError, ViewDriverRegistrationErrorKind,
    };
}

#[cfg(feature = "bsn")]
pub mod bsn {
    pub use bevy_ui_router_bsn::{BsnBuildError, BsnSceneFactory, DefaultOutlet};
    pub use bevy_ui_router_ecs::AppBsnExt;
}

#[cfg(feature = "platform")]
pub mod platform {
    pub mod prelude {
        pub use crate::core::{BackCapabilitySnapshot, BackIntent, BackPolicy};
    }
}

pub mod prelude {
    #[cfg(feature = "bsn")]
    pub use crate::bsn::{AppBsnExt, BsnBuildError, BsnSceneFactory, DefaultOutlet};

    pub use crate::core::{
        ActivePresentation, ActivePresentationKind, BackCapabilitySnapshot, BackIntent, BackPolicy,
        BranchDiff, BranchDiffRoute, CleanupDisposition, CleanupSummary, DiagnosticSeverity,
        DiagnosticSnapshot, ErrorBoundaryKey, FrameOrigin, HistoryError, HistorySnapshot,
        HostSnapshot, HostTraceEvent, HostTraceEventKind, HostVersion, MatchedBranch, MatchedRoute,
        MemoryHistory, MountRejectionKind, NavigationFailure, NavigationFailureKind,
        NavigationPhase, NavigationRequestId, NavigationTraceEvent, NavigationTraceEventKind,
        ReportDisposition, RequestStatus, RouteBranchFingerprint, RouteError, RouteErrorKind,
        RouteErrorSourceCategory, RouteExplicitKey, RouteFrameId, RouteFrameStatus,
        RouteFrameVersion, RouteGeneration, RouteHostId, RouteId, RouteLocation, RouteMatcher,
        RouteMeta, RouteNodeOptions, RouteParams, RoutePathSegment, RoutePattern, RoutePatternKind,
        RouteSlotId, RouteSlotKey, RouteTree, RouteTreeBuilder, RouteViewInput, RouteViewKey,
        RouteViewKeyError, RouterAvailability, RouterInstanceId, RouterUnavailableReason,
    };

    #[cfg(feature = "view")]
    pub use crate::view::{
        RouteViewDescriptor, RouteViewRegistry, RouteViewRegistryError, ViewBuildEffect,
        ViewDriverKey, ViewDriverKeyError, ViewRequestId,
    };

    #[cfg(feature = "ecs")]
    pub use crate::ecs::{
        AppRouterExt, BevyUiRouterPlugin, NavigationEnqueueError, NavigationEnqueueErrorKind,
        NavigationRequestSnapshot, RequestLookup, RouteScroll, RouterConfig, RouterHandle,
        RouterHostRegistration, RouterInputGate, RouterInstallError, RouterInstallErrorKind,
        RouterNavigation, RouterSnapshot, RouterSnapshots, ViewDriverRegistrationError,
        ViewDriverRegistrationErrorKind,
    };
}
