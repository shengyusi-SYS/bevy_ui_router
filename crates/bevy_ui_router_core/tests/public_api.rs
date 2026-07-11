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
fn phase_2_frame_transaction_types_are_public_observation_api() {
    use bevy_ui_router_core::diagnostics::FrameLifecycleDiagnostic;
    use bevy_ui_router_core::frame::{
        FrameCleanupPlan, FrameReconciliationPlan, RouteFrameBranch, RouteFrameIdentity,
        RouteFrameRecord, RouteFrameScope, RouteFrameStatus,
    };
    use bevy_ui_router_core::transaction::{
        FrameApplyOutcome, FrameApplyRequest, FrameApplyToken, NavigationTransactionSnapshot,
        NavigationTransactionState, StaleApplyReason, StaleApplyResult, TransactionCommitError,
        TransactionCommitErrorKind, TransactionFailure,
    };

    fn assert_public<T>() {}
    assert_public::<FrameCleanupPlan>();
    assert_public::<FrameReconciliationPlan>();
    assert_public::<RouteFrameBranch>();
    assert_public::<RouteFrameIdentity>();
    assert_public::<RouteFrameRecord>();
    assert_public::<RouteFrameScope>();
    assert_public::<FrameLifecycleDiagnostic>();
    assert_public::<FrameApplyOutcome>();
    assert_public::<FrameApplyRequest>();
    assert_public::<FrameApplyToken>();
    assert_public::<NavigationTransactionSnapshot>();
    assert_public::<NavigationTransactionState>();
    assert_public::<StaleApplyReason>();
    assert_public::<StaleApplyResult>();
    assert_public::<TransactionCommitError>();
    assert_public::<TransactionCommitErrorKind>();
    assert_public::<TransactionFailure>();
    assert_eq!(
        RouteFrameStatus::ApplyingView,
        RouteFrameStatus::ApplyingView
    );
}

#[test]
fn phase_3b_runtime_bridge_is_explicit_low_level_core_api() {
    use bevy_ui_router_core::runtime::{
        BeginNavigationError, FailureCleanupAckError, FrameApplyReportError, RouterRuntime,
    };

    fn assert_public<T>() {}
    assert_public::<RouterRuntime>();
    assert_public::<BeginNavigationError>();
    assert_public::<FrameApplyReportError>();
    assert_public::<FailureCleanupAckError>();

    let runtime = RouterRuntime::new();
    assert!(runtime.pending_snapshot().is_none());
    assert!(runtime.active_branch().is_none());
    assert_eq!(runtime.scope().router(), runtime.router());
}

#[test]
fn route_view_key_is_public_without_raw_value_access() {
    use bevy_ui_router_core::view_key::RouteViewKey;

    fn assert_key_traits<T: Clone + std::fmt::Debug + Eq + PartialEq + std::hash::Hash>() {}

    assert_key_traits::<RouteViewKey>();
    assert!(RouteViewKey::new("").is_err());

    let source = include_str!("../src/view_key.rs");
    assert!(source.contains("pub struct RouteViewKeyError {\n    _private: (),\n}"));
    for forbidden in [
        "pub struct RouteViewKey(pub ",
        "pub struct RouteViewKeyError(pub ",
        "pub fn as_str(",
        "pub fn into_inner(",
    ] {
        assert!(
            !source.contains(forbidden),
            "route view keys must not expose raw values through `{forbidden}`"
        );
    }
}

#[test]
fn root_profile_hides_deferred_route_declaration_methods() {
    let source = include_str!("../src/route_tree.rs");

    assert!(source.contains("pub fn root_with_options("));
    for forbidden in [
        "pub fn with_error_boundary(",
        "pub fn with_error_view(",
        "pub fn route(",
        "pub fn route_with_options(",
        "#[cfg(feature = \"test-support\")]",
    ] {
        assert!(
            !source.contains(forbidden),
            "root profile must not publicly expose `{forbidden}`"
        );
    }
    for required in ["pub(crate) fn route(", "pub(crate) fn route_with_options("] {
        assert!(
            source.contains(required),
            "root profile must retain crate-private `{required}`"
        );
    }
}

#[test]
fn task_3_public_methods_match_the_corrected_golden_shape() {
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

    let frame = include_str!("../src/frame.rs");
    let presentation = include_str!("../src/presentation.rs");
    let request = include_str!("../src/request.rs");
    let trace = include_str!("../src/trace.rs");
    let view_input = include_str!("../src/view_input.rs");
    fn impl_before<'a>(source: &'a str, type_name: &str, next_item: &str) -> &'a str {
        let header = format!("impl {type_name} {{");
        source
            .split_once(&header)
            .unwrap_or_else(|| panic!("{type_name} impl must exist"))
            .1
            .split_once(next_item)
            .unwrap_or_else(|| panic!("{type_name} impl must end before `{next_item}`"))
            .0
    }

    for implementation in [
        impl_before(frame, "RouteFrameVersion", "\n}\n\n#[derive"),
        impl_before(presentation, "RouterAvailability", "\n}\n\n#[derive"),
        impl_before(presentation, "ActivePresentation", "\n}\n\n#[derive"),
        impl_before(request, "NavigationFailure", "\n}\n\n#[derive"),
        impl_before(trace, "NavigationTraceEvent", "\n}\n\n#[derive"),
        impl_before(view_input, "RouteViewInput", "\n}\n\n#[cfg(test)]"),
    ] {
        assert!(!implementation.contains("pub const fn "));
    }
    assert!(!view_input.contains("#[derive("));
}

#[test]
fn task_3_private_foundations_use_item_scoped_test_aware_dead_code_allows() {
    let sources = [
        include_str!("../src/admission.rs"),
        include_str!("../src/presentation.rs"),
        include_str!("../src/request.rs"),
        include_str!("../src/trace.rs"),
        include_str!("../src/view_input.rs"),
    ];

    for source in sources {
        assert!(!source.contains("#[allow(dead_code)]"));
        assert!(!source.contains("#[cfg_attr(not(test), allow(dead_code))]\nimpl "));
    }

    for (source, state_type) in [
        (sources[0], "pub(crate) struct HostAdmissionIssuer"),
        (sources[1], "pub(crate) struct RouterNavigationState"),
        (sources[2], "pub(crate) struct NavigationRequestRecord"),
        (sources[3], "pub(crate) struct NavigationTraceLog"),
    ] {
        let marker = source.find(state_type).expect("state type must exist");
        let prefix = &source[..marker];
        let nearby = &prefix[prefix.len().saturating_sub(160)..];
        assert!(!nearby.contains("allow(dead_code)"));
    }
}
