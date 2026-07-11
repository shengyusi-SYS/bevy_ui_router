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
    for forbidden in ["pub fn with_error_boundary(", "pub fn with_error_view("] {
        assert!(
            !source.contains(forbidden),
            "root profile must not publicly expose `{forbidden}`"
        );
    }
    for required in [
        "#[cfg(not(feature = \"test-support\"))]\n    #[allow(dead_code)]\n    pub(crate) fn route(",
        "#[cfg(not(feature = \"test-support\"))]\n    #[allow(dead_code)]\n    pub(crate) fn route_with_options(",
    ] {
        assert!(
            source.contains(required),
            "root profile must retain crate-private `{required}`"
        );
    }
}
