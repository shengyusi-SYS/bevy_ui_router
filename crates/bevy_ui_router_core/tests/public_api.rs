#[test]
fn non_prelude_branch_planning_support_types_are_public_module_api() {
    use bevy_ui_router_core::branch_diff::BranchDiffRoute;
    use bevy_ui_router_core::diagnostics::BranchPlanningDiagnostic;
    use bevy_ui_router_core::fingerprint::{
        RouteBranchFingerprint, RouteParamsFingerprint, RouteSearchFingerprint,
        RouteSegmentReuseKey,
    };

    let diagnostic = BranchPlanningDiagnostic::new(0, 0, 0, false, false, false);
    assert_eq!(diagnostic.common_prefix_len(), 0);

    fn assert_public<T>() {}
    assert_public::<BranchDiffRoute>();
    assert_public::<RouteBranchFingerprint>();
    assert_public::<RouteParamsFingerprint>();
    assert_public::<RouteSearchFingerprint>();
    assert_public::<RouteSegmentReuseKey>();
}

#[test]
fn route_view_key_exposes_value_traits_and_rejects_empty_key() {
    use bevy_ui_router_core::view_key::RouteViewKey;

    fn assert_key_traits<T: Clone + std::fmt::Debug + Eq + PartialEq + std::hash::Hash>() {}

    assert_key_traits::<RouteViewKey>();
    assert!(RouteViewKey::new("").is_err());
}

#[test]
fn recovery_profile_exposes_error_recovery_builders_with_exact_signatures() {
    use bevy_ui_router_core::{
        error::RouteError,
        ids::RouteId,
        meta::ErrorBoundaryKey,
        pattern::RoutePattern,
        route_tree::{RouteNodeOptions, RouteTreeBuilder},
        view_key::RouteViewKey,
    };

    let _: fn(&mut RouteTreeBuilder, RouteId, RoutePattern) -> Result<RouteId, RouteError> =
        RouteTreeBuilder::route;
    let _: fn(
        &mut RouteTreeBuilder,
        RouteId,
        RoutePattern,
        RouteNodeOptions,
    ) -> Result<RouteId, RouteError> = RouteTreeBuilder::route_with_options;
    let _: fn(RouteNodeOptions, ErrorBoundaryKey) -> RouteNodeOptions =
        RouteNodeOptions::with_error_boundary;
    let _: fn(RouteNodeOptions, ErrorBoundaryKey, RouteViewKey) -> RouteNodeOptions =
        RouteNodeOptions::with_error_view;

    let _: fn(
        RouteTreeBuilder,
        RoutePattern,
        RouteNodeOptions,
    ) -> Result<RouteTreeBuilder, RouteError> = RouteTreeBuilder::root_with_options;
}

#[test]
fn navigation_projections_expose_read_only_methods_and_value_traits() {
    use bevy_ui_router_core::HostVersion;
    use bevy_ui_router_core::branch::RouteParams;
    use bevy_ui_router_core::fingerprint::RouteBranchFingerprint;
    use bevy_ui_router_core::frame::{FrameOrigin, RouteFrameVersion};
    use bevy_ui_router_core::ids::{
        RouteFrameId, RouteGeneration, RouteHostId, RouteId, RouteSlotId, RouterInstanceId,
    };
    use bevy_ui_router_core::location::RouteLocation;
    use bevy_ui_router_core::prelude::HostVersion as PreludeHostVersion;
    use bevy_ui_router_core::presentation::{
        ActivePresentation, ActivePresentationKind, RouterAvailability, RouterUnavailableReason,
    };
    use bevy_ui_router_core::request::{
        NavigationFailure, NavigationFailureKind, NavigationPhase, RequestStatus,
    };
    use bevy_ui_router_core::slot::RouteSlotKey;
    use bevy_ui_router_core::trace::{
        CleanupDisposition, MountRejectionKind, NavigationTraceEvent, NavigationTraceEventKind,
    };
    use bevy_ui_router_core::view_input::RouteViewInput;

    fn assert_clone_debug_eq<T: Clone + std::fmt::Debug + Eq + PartialEq>() {}
    fn assert_copy_debug_eq<T: Clone + Copy + std::fmt::Debug + Eq + PartialEq>() {}

    assert_clone_debug_eq::<HostVersion>();
    assert_clone_debug_eq::<PreludeHostVersion>();
    assert_clone_debug_eq::<FrameOrigin>();
    assert_clone_debug_eq::<RouteFrameVersion>();
    assert_copy_debug_eq::<RouterUnavailableReason>();
    assert_clone_debug_eq::<RouterAvailability>();
    assert_copy_debug_eq::<ActivePresentationKind>();
    assert_clone_debug_eq::<ActivePresentation>();
    assert_copy_debug_eq::<NavigationFailureKind>();
    assert_clone_debug_eq::<NavigationFailure>();
    assert_clone_debug_eq::<NavigationPhase>();
    assert_clone_debug_eq::<RequestStatus>();
    assert_copy_debug_eq::<MountRejectionKind>();
    assert_copy_debug_eq::<CleanupDisposition>();
    assert_copy_debug_eq::<NavigationTraceEventKind>();
    assert_clone_debug_eq::<NavigationTraceEvent>();

    let _: fn(&RouteFrameVersion) -> RouterInstanceId = RouteFrameVersion::router;
    let _: fn(&RouteFrameVersion) -> RouteHostId = RouteFrameVersion::host;
    let _: fn(&RouteFrameVersion) -> RouteSlotId = RouteFrameVersion::slot;
    let _: for<'a> fn(&'a RouteFrameVersion) -> &'a FrameOrigin = RouteFrameVersion::origin;
    let _: fn(&RouteFrameVersion) -> RouteFrameId = RouteFrameVersion::frame;
    let _: fn(&RouteFrameVersion) -> RouteGeneration = RouteFrameVersion::generation;

    let _: fn(&RouterAvailability) -> Option<RouterUnavailableReason> =
        RouterAvailability::unavailable_reason;
    let _: fn(&ActivePresentation) -> ActivePresentationKind = ActivePresentation::kind;
    let _: for<'a> fn(&'a ActivePresentation) -> Option<&'a RouteLocation> =
        ActivePresentation::location;
    let _: for<'a> fn(&'a ActivePresentation) -> Option<&'a RouteLocation> =
        ActivePresentation::requested;
    let _: for<'a> fn(&'a ActivePresentation) -> Option<&'a RouteBranchFingerprint> =
        ActivePresentation::matched;
    let _: for<'a> fn(&'a ActivePresentation) -> &'a [RouteFrameVersion] =
        ActivePresentation::frames;
    let _: for<'a> fn(&'a ActivePresentation) -> Option<&'a NavigationFailure> =
        ActivePresentation::failure;

    let _: fn(&NavigationFailure) -> NavigationFailureKind = NavigationFailure::kind;
    let _: for<'a> fn(&'a NavigationFailure) -> &'a str = NavigationFailure::message;

    let _: fn(&NavigationTraceEvent) -> NavigationTraceEventKind = NavigationTraceEvent::kind;
    let _: for<'a> fn(&'a NavigationTraceEvent) -> Option<&'a RouteFrameVersion> =
        NavigationTraceEvent::frame;
    let _: for<'a> fn(&'a NavigationTraceEvent) -> Option<&'a RouteSlotKey> =
        NavigationTraceEvent::outlet;
    let _: fn(&NavigationTraceEvent) -> Option<RouteSlotId> = NavigationTraceEvent::slot;
    let _: for<'a> fn(&'a NavigationTraceEvent) -> Option<&'a NavigationPhase> =
        NavigationTraceEvent::phase;
    let _: fn(&NavigationTraceEvent) -> Option<MountRejectionKind> =
        NavigationTraceEvent::mount_rejection;
    let _: fn(&NavigationTraceEvent) -> Option<CleanupDisposition> =
        NavigationTraceEvent::cleanup_disposition;

    let _: for<'a> fn(&'a RouteViewInput) -> &'a RouteLocation = RouteViewInput::location;
    let _: fn(&RouteViewInput) -> Option<RouteId> = RouteViewInput::route;
    let _: for<'a> fn(&'a RouteViewInput) -> &'a RouteParams = RouteViewInput::params;
    let _: for<'a> fn(&'a RouteViewInput) -> Option<&'a str> = RouteViewInput::raw_search;
    let _: for<'a> fn(&'a RouteViewInput) -> Option<&'a str> = RouteViewInput::fragment;
    let _: for<'a> fn(&'a RouteViewInput) -> &'a FrameOrigin = RouteViewInput::origin;
    let _: for<'a> fn(&'a RouteViewInput) -> Option<&'a NavigationFailure> =
        RouteViewInput::failure;
}

#[test]
fn sealed_report_identity_exposes_opaque_value_traits() {
    use bevy_ui_router_core::integration::ViewReportIdentity;

    fn assert_opaque_traits<T: Clone + Eq + PartialEq>() {}
    assert_opaque_traits::<ViewReportIdentity>();
}
