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
