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
