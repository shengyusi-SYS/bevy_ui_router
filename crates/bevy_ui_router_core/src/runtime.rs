//! ```
//! let runtime = bevy_ui_router_core::runtime::RouterRuntime::new();
//! let _ = runtime.scope();
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_core::prelude::RouterRuntime;
//!
//! let _ = RouterRuntime::new();
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_core::prelude::{
//!     BeginNavigationError, FailureCleanupAckError, FrameApplyReportError,
//! };
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_core::frame::RouteFrameScope;
//!
//! let _ = RouteFrameScope::new(unreachable!(), unreachable!(), unreachable!());
//! ```

use std::sync::atomic::{AtomicU64, Ordering};

use crate::branch::MatchedBranch;
use crate::branch_diff::BranchDiff;
use crate::frame::{
    FrameAllocator, FrameCleanupPlan, FrameReconciliationPlan, RouteFrameBranch, RouteFrameScope,
};
use crate::ids::{RouteHostId, RouteSlotId, RouterInstanceId};
use crate::transaction::{
    FrameApplyOutcome, FrameApplyToken, NavigationTransaction, NavigationTransactionSnapshot,
    NavigationTransactionState, StaleApplyResult, TransactionCommitError,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BeginNavigationError {
    PendingNavigation,
    FailedNavigationNeedsCleanup,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FrameApplyReportError {
    Stale(StaleApplyResult),
    NoPendingNavigation,
    TerminalFailure,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailureCleanupAckError {
    NoFailedNavigation,
    CleanupPlanMismatch,
}

#[derive(Debug)]
pub struct RouterRuntime {
    router: RouterInstanceId,
    host: RouteHostId,
    slot: RouteSlotId,
    allocator: FrameAllocator,
    active: Option<RouteFrameBranch>,
    pending: Option<NavigationTransaction>,
}

impl RouterRuntime {
    pub fn new() -> Self {
        Self {
            router: next_router_id(),
            host: next_host_id(),
            slot: next_slot_id(),
            allocator: FrameAllocator::new(),
            active: None,
            pending: None,
        }
    }

    pub const fn router(&self) -> RouterInstanceId {
        self.router
    }

    pub const fn host(&self) -> RouteHostId {
        self.host
    }

    pub const fn slot(&self) -> RouteSlotId {
        self.slot
    }

    pub const fn scope(&self) -> RouteFrameScope {
        RouteFrameScope::new(self.router, self.host, self.slot)
    }

    pub fn active_branch(&self) -> Option<&RouteFrameBranch> {
        self.active.as_ref()
    }

    pub fn pending_snapshot(&self) -> Option<NavigationTransactionSnapshot> {
        self.pending.as_ref().map(NavigationTransaction::snapshot)
    }

    pub fn begin_first_navigation(
        &mut self,
        target: &MatchedBranch,
    ) -> Result<NavigationTransactionSnapshot, BeginNavigationError> {
        self.ensure_can_begin_navigation()?;
        let plan = FrameReconciliationPlan::from_first_navigation(
            self.scope(),
            target,
            &mut self.allocator,
        );
        self.begin_navigation(plan)
    }

    pub fn begin_reconciled_navigation(
        &mut self,
        target: &MatchedBranch,
        diff: &BranchDiff,
    ) -> Result<NavigationTransactionSnapshot, BeginNavigationError> {
        self.ensure_can_begin_navigation()?;
        let plan = FrameReconciliationPlan::from_diff(
            self.scope(),
            self.active.as_ref(),
            target,
            diff,
            &mut self.allocator,
        );
        self.begin_navigation(plan)
    }

    pub fn report_frame_apply(
        &mut self,
        token: &FrameApplyToken,
        outcome: FrameApplyOutcome,
    ) -> Result<NavigationTransactionSnapshot, FrameApplyReportError> {
        let pending = self
            .pending
            .as_mut()
            .ok_or(FrameApplyReportError::NoPendingNavigation)?;
        let was_failed = pending.snapshot().state() == NavigationTransactionState::Failed;

        pending
            .accept_apply_result(token, outcome)
            .map_err(FrameApplyReportError::Stale)?;
        if was_failed {
            return Err(FrameApplyReportError::TerminalFailure);
        }
        Ok(pending.snapshot())
    }

    pub fn commit_ready_navigation(&mut self) -> Result<&RouteFrameBranch, TransactionCommitError> {
        let branch = self
            .pending
            .clone()
            .ok_or_else(TransactionCommitError::pending_apply)?
            .commit(self.active.as_ref())?;
        self.active = Some(branch);
        self.pending = None;
        Ok(self
            .active
            .as_ref()
            .expect("active branch was just committed"))
    }

    pub fn acknowledge_failed_navigation_cleanup(
        &mut self,
        completed: &[FrameCleanupPlan],
    ) -> Result<(), FailureCleanupAckError> {
        let failure = self
            .pending
            .as_ref()
            .filter(|pending| pending.snapshot().state() == NavigationTransactionState::Failed)
            .and_then(|pending| pending.snapshot().failure().cloned())
            .ok_or(FailureCleanupAckError::NoFailedNavigation)?;

        if failure.cleanup() != completed {
            return Err(FailureCleanupAckError::CleanupPlanMismatch);
        }

        self.pending = None;
        Ok(())
    }

    fn begin_navigation(
        &mut self,
        plan: FrameReconciliationPlan,
    ) -> Result<NavigationTransactionSnapshot, BeginNavigationError> {
        if let Some(pending) = self.pending.as_ref() {
            return Err(match pending.snapshot().state() {
                NavigationTransactionState::Failed => {
                    BeginNavigationError::FailedNavigationNeedsCleanup
                }
                _ => BeginNavigationError::PendingNavigation,
            });
        }

        let pending = NavigationTransaction::new(plan);
        let snapshot = pending.snapshot();
        self.pending = Some(pending);
        Ok(snapshot)
    }

    fn ensure_can_begin_navigation(&self) -> Result<(), BeginNavigationError> {
        if let Some(pending) = self.pending.as_ref() {
            return Err(match pending.snapshot().state() {
                NavigationTransactionState::Failed => {
                    BeginNavigationError::FailedNavigationNeedsCleanup
                }
                _ => BeginNavigationError::PendingNavigation,
            });
        }

        Ok(())
    }
}

impl Default for RouterRuntime {
    fn default() -> Self {
        Self::new()
    }
}

fn next_router_id() -> RouterInstanceId {
    static NEXT_ROUTER_ID: AtomicU64 = AtomicU64::new(1);
    RouterInstanceId::new(NEXT_ROUTER_ID.fetch_add(1, Ordering::Relaxed))
}

fn next_host_id() -> RouteHostId {
    static NEXT_HOST_ID: AtomicU64 = AtomicU64::new(1);
    RouteHostId::new(NEXT_HOST_ID.fetch_add(1, Ordering::Relaxed))
}

fn next_slot_id() -> RouteSlotId {
    static NEXT_SLOT_ID: AtomicU64 = AtomicU64::new(1);
    RouteSlotId::new(NEXT_SLOT_ID.fetch_add(1, Ordering::Relaxed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::branch::MatchedBranch;
    use crate::branch_diff::BranchDiff;
    use crate::error::{RouteError, RouteErrorKind, RouteErrorSourceCategory};
    use crate::frame::RouteFrameBranch;
    use crate::ids::{
        RouteFrameId, RouteGeneration, RouteHostId, RouteId, RouteSlotId, RouterInstanceId,
    };
    use crate::location::RouteLocation;
    use crate::matcher::RouteMatcher;
    use crate::meta::ErrorBoundaryKey;
    use crate::pattern::RoutePattern;
    use crate::route_tree::{RouteNodeOptions, RouteTree};
    use crate::transaction::{
        FrameApplyOutcome, NavigationTransactionSnapshot, NavigationTransactionState,
        StaleApplyReason, TransactionCommitErrorKind,
    };

    #[test]
    fn runtime_issues_router_host_slot_scope_without_public_id_constructors() {
        let runtime = RouterRuntime::new();
        let scope = runtime.scope();

        assert_eq!(runtime.router(), scope.router());
        assert_eq!(runtime.host(), scope.host());
        assert_eq!(runtime.slot(), scope.slot());
        assert!(runtime.active_branch().is_none());
        assert!(runtime.pending_snapshot().is_none());
    }

    #[test]
    fn runtime_begin_first_navigation_issues_frame_apply_requests() {
        let matcher = items_matcher();
        let target = matched_branch(&matcher, "/items/2");
        let mut runtime = RouterRuntime::new();

        let snapshot = runtime.begin_first_navigation(&target).unwrap();

        assert_eq!(snapshot.state(), NavigationTransactionState::PendingApply);
        assert_eq!(snapshot.requests().len(), target.routes().len());
        assert!(snapshot.failure().is_none());
        for request in snapshot.requests() {
            let token = request.token();
            let identity = request.frame().identity();
            assert_eq!(token.router(), runtime.router());
            assert_eq!(token.host(), runtime.host());
            assert_eq!(token.slot(), runtime.slot());
            assert_eq!(token.router(), identity.router());
            assert_eq!(token.host(), identity.host());
            assert_eq!(token.slot(), identity.slot());
            assert_eq!(token.route(), identity.route());
            assert_eq!(token.frame(), identity.frame());
            assert_eq!(token.generation(), identity.generation());
        }
    }

    #[test]
    fn runtime_instances_issue_distinct_router_host_slot_identities() {
        let first = RouterRuntime::new();
        let second = RouterRuntime::new();

        assert_ne!(first.router(), second.router());
        assert_ne!(first.host(), second.host());
        assert_ne!(first.slot(), second.slot());
        assert_ne!(first.scope(), second.scope());
    }

    #[test]
    fn runtime_pending_navigation_conflict_returns_explicit_error() {
        let matcher = items_matcher();
        let target = matched_branch(&matcher, "/items/1");
        let mut runtime = RouterRuntime::new();

        runtime.begin_first_navigation(&target).unwrap();
        let error = runtime.begin_first_navigation(&target).unwrap_err();

        assert_eq!(error, BeginNavigationError::PendingNavigation);
    }

    #[test]
    fn runtime_rejected_begin_first_navigation_is_allocator_side_effect_free() {
        let matcher = items_matcher();
        let initial = matched_branch(&matcher, "/items/1");
        let target = matched_branch(&matcher, "/items/2");

        let expected = {
            let mut control = RouterRuntime::new();
            let initial_snapshot = control.begin_first_navigation(&initial).unwrap();
            fail_pending_with_build_error(&mut control, initial_snapshot.requests()[0].token());
            control.acknowledge_failed_navigation_cleanup(&[]).unwrap();
            frame_id_raws(&control.begin_first_navigation(&target).unwrap())
        };

        let actual = {
            let mut experiment = RouterRuntime::new();
            let initial_snapshot = experiment.begin_first_navigation(&initial).unwrap();
            assert_eq!(
                experiment.begin_first_navigation(&target).unwrap_err(),
                BeginNavigationError::PendingNavigation
            );
            fail_pending_with_build_error(&mut experiment, initial_snapshot.requests()[0].token());
            experiment
                .acknowledge_failed_navigation_cleanup(&[])
                .unwrap();
            frame_id_raws(&experiment.begin_first_navigation(&target).unwrap())
        };

        assert_eq!(actual, expected);
    }

    #[test]
    fn runtime_accepts_apply_only_through_core_stale_check() {
        let matcher = root_only_matcher();
        let target = matched_branch(&matcher, "/");
        let mut runtime = RouterRuntime::new();
        let snapshot = runtime.begin_first_navigation(&target).unwrap();
        let token = snapshot.requests()[0].token().clone();
        let mut stale = token.clone();
        stale.testing_set_router(RouterInstanceId::new(runtime.router().raw() + 10_000));

        let error = runtime
            .report_frame_apply(&stale, FrameApplyOutcome::Applied)
            .unwrap_err();

        assert_stale_reason(error, StaleApplyReason::RouterMismatch);

        let next = runtime
            .report_frame_apply(&token, FrameApplyOutcome::Applied)
            .unwrap();
        assert_eq!(next.state(), NavigationTransactionState::ReadyToCommit);
    }

    #[test]
    fn runtime_does_not_update_active_branch_before_explicit_commit() {
        let matcher = items_matcher();
        let target = matched_branch(&matcher, "/items/2");
        let mut runtime = RouterRuntime::new();
        let snapshot = runtime.begin_first_navigation(&target).unwrap();

        apply_all_success(&mut runtime, &snapshot);

        assert!(runtime.active_branch().is_none());
        assert_eq!(
            runtime.pending_snapshot().unwrap().state(),
            NavigationTransactionState::ReadyToCommit
        );

        let active = runtime.commit_ready_navigation().unwrap();
        assert_eq!(active.frames().len(), target.routes().len());
        assert!(runtime.pending_snapshot().is_none());
    }

    #[test]
    fn runtime_commit_rechecks_active_baseline() {
        let matcher = items_matcher();
        let initial = matched_branch(&matcher, "/items/1");
        let target = matched_branch(&matcher, "/items/2");
        let mut runtime = RouterRuntime::new();

        let first = runtime.begin_first_navigation(&initial).unwrap();
        apply_all_success(&mut runtime, &first);
        runtime.commit_ready_navigation().unwrap();

        let diff = BranchDiff::compare(&initial, &target);
        let second = runtime.begin_reconciled_navigation(&target, &diff).unwrap();
        apply_all_success(&mut runtime, &second);

        let original = runtime
            .active_branch()
            .unwrap()
            .branch_fingerprint()
            .clone();
        runtime.active = Some(RouteFrameBranch::new(Vec::new(), original));

        let error = runtime.commit_ready_navigation().unwrap_err();

        assert_eq!(
            error.kind(),
            TransactionCommitErrorKind::ChangedCurrentBranch
        );
        assert_eq!(
            runtime.pending_snapshot().unwrap().state(),
            NavigationTransactionState::ReadyToCommit
        );
    }

    #[test]
    fn runtime_failure_exposes_router_owned_cleanup_without_touching_host() {
        let matcher = boundary_matcher();
        let target = matched_branch(&matcher, "/section/items/2");
        let mut runtime = RouterRuntime::new();
        let snapshot = runtime.begin_first_navigation(&target).unwrap();
        let failed = snapshot.requests().last().unwrap().token().clone();

        let failure = runtime
            .report_frame_apply(
                &failed,
                FrameApplyOutcome::Failed(RouteError::new(
                    RouteErrorKind::ViewApplyFailed,
                    RouteErrorSourceCategory::RendererContract,
                    "view apply failed",
                )),
            )
            .unwrap()
            .failure()
            .cloned()
            .unwrap();

        assert_eq!(failure.failing_route(), failed.route());
        assert_eq!(
            failure
                .nearest_error_boundary()
                .map(ErrorBoundaryKey::as_str),
            Some("items")
        );
        assert_eq!(failure.cleanup().len(), 1);
        assert_eq!(failure.cleanup()[0].identity().host(), runtime.host());
        assert!(failure.cleanup()[0].scope().excludes_app_owned_host());
        assert!(runtime.active_branch().is_none());
    }

    #[test]
    fn runtime_stale_apply_result_has_no_visible_side_effects_for_transaction_router_host_slot_route_frame_generation_and_sequence()
     {
        let matcher = root_only_matcher();
        let target = matched_branch(&matcher, "/");

        for (mutated, expected) in stale_cases(&target) {
            let mut runtime = RouterRuntime::new();
            let snapshot = runtime.begin_first_navigation(&target).unwrap();
            let baseline = runtime.pending_snapshot().unwrap();
            let router = runtime.router();
            let host = runtime.host();
            let slot = runtime.slot();

            let error = runtime
                .report_frame_apply(
                    &mutated(snapshot.requests()[0].token().clone()),
                    FrameApplyOutcome::Applied,
                )
                .unwrap_err();

            assert_stale_reason(error, expected);
            assert_eq!(runtime.router(), router);
            assert_eq!(runtime.host(), host);
            assert_eq!(runtime.slot(), slot);
            assert_eq!(runtime.pending_snapshot(), Some(baseline));
            assert!(runtime.active_branch().is_none());
        }
    }

    #[test]
    fn runtime_failed_navigation_requires_cleanup_ack_before_next_navigation() {
        let matcher = items_matcher();
        let target = matched_branch(&matcher, "/items/2");
        let mut runtime = RouterRuntime::new();
        let snapshot = runtime.begin_first_navigation(&target).unwrap();
        let failed = snapshot.requests()[0].token().clone();

        let failed_snapshot = runtime
            .report_frame_apply(
                &failed,
                FrameApplyOutcome::Failed(RouteError::new(
                    RouteErrorKind::ViewApplyFailed,
                    RouteErrorSourceCategory::RendererContract,
                    "view apply failed",
                )),
            )
            .unwrap();

        assert_eq!(
            runtime.begin_first_navigation(&target).unwrap_err(),
            BeginNavigationError::FailedNavigationNeedsCleanup
        );

        let cleanup = failed_snapshot.failure().unwrap().cleanup().to_vec();
        runtime
            .acknowledge_failed_navigation_cleanup(&cleanup)
            .unwrap();

        let restarted = runtime.begin_first_navigation(&target).unwrap();
        assert_eq!(restarted.state(), NavigationTransactionState::PendingApply);
    }

    #[test]
    fn runtime_rejected_begin_reconciled_navigation_is_allocator_side_effect_free() {
        let matcher = items_matcher();
        let initial = matched_branch(&matcher, "/items/1");
        let failed_target = matched_branch(&matcher, "/items/2");
        let target = matched_branch(&matcher, "/items/3");

        let expected = {
            let mut control = RouterRuntime::new();
            commit_branch(&mut control, &initial);
            let failed_diff = BranchDiff::compare(&initial, &failed_target);
            let failed_snapshot = control
                .begin_reconciled_navigation(&failed_target, &failed_diff)
                .unwrap();
            fail_pending_with_build_error(&mut control, failed_snapshot.requests()[0].token());
            control.acknowledge_failed_navigation_cleanup(&[]).unwrap();
            let target_diff = BranchDiff::compare(&initial, &target);
            frame_id_raws(
                &control
                    .begin_reconciled_navigation(&target, &target_diff)
                    .unwrap(),
            )
        };

        let actual = {
            let mut experiment = RouterRuntime::new();
            commit_branch(&mut experiment, &initial);
            let failed_diff = BranchDiff::compare(&initial, &failed_target);
            let failed_snapshot = experiment
                .begin_reconciled_navigation(&failed_target, &failed_diff)
                .unwrap();
            fail_pending_with_build_error(&mut experiment, failed_snapshot.requests()[0].token());
            let target_diff = BranchDiff::compare(&initial, &target);
            assert_eq!(
                experiment
                    .begin_reconciled_navigation(&target, &target_diff)
                    .unwrap_err(),
                BeginNavigationError::FailedNavigationNeedsCleanup
            );
            experiment
                .acknowledge_failed_navigation_cleanup(&[])
                .unwrap();
            frame_id_raws(
                &experiment
                    .begin_reconciled_navigation(&target, &target_diff)
                    .unwrap(),
            )
        };

        assert_eq!(actual, expected);
    }

    #[test]
    fn runtime_cleanup_ack_rejects_mismatched_cleanup_plans() {
        let matcher = items_matcher();
        let target = matched_branch(&matcher, "/items/2");
        let mut runtime = RouterRuntime::new();
        let snapshot = runtime.begin_first_navigation(&target).unwrap();
        let failed = snapshot.requests()[0].token().clone();

        runtime
            .report_frame_apply(
                &failed,
                FrameApplyOutcome::Failed(RouteError::new(
                    RouteErrorKind::ViewApplyFailed,
                    RouteErrorSourceCategory::RendererContract,
                    "view apply failed",
                )),
            )
            .unwrap();

        let error = runtime
            .acknowledge_failed_navigation_cleanup(&[])
            .unwrap_err();

        assert_eq!(error, FailureCleanupAckError::CleanupPlanMismatch);
        assert_eq!(
            runtime.begin_first_navigation(&target).unwrap_err(),
            BeginNavigationError::FailedNavigationNeedsCleanup
        );
    }

    fn root_only_matcher() -> RouteMatcher {
        RouteMatcher::new(
            RouteTree::builder()
                .root(RoutePattern::path("/").unwrap())
                .unwrap()
                .build()
                .unwrap(),
        )
    }

    fn items_matcher() -> RouteMatcher {
        let mut builder = RouteTree::builder()
            .root(RoutePattern::path("/").unwrap())
            .unwrap();
        let root = builder.root_id().unwrap();
        let items = builder
            .route(root, RoutePattern::path("items").unwrap())
            .unwrap();
        builder
            .route(items, RoutePattern::path(":id").unwrap())
            .unwrap();
        RouteMatcher::new(builder.build().unwrap())
    }

    fn boundary_matcher() -> RouteMatcher {
        let mut builder = RouteTree::builder()
            .root(RoutePattern::path("/").unwrap())
            .unwrap();
        let root = builder.root_id().unwrap();
        let section = builder
            .route_with_options(
                root,
                RoutePattern::path("section").unwrap(),
                RouteNodeOptions::new()
                    .with_error_boundary(ErrorBoundaryKey::named("section").unwrap()),
            )
            .unwrap();
        let items = builder
            .route_with_options(
                section,
                RoutePattern::path("items").unwrap(),
                RouteNodeOptions::new()
                    .with_error_boundary(ErrorBoundaryKey::named("items").unwrap()),
            )
            .unwrap();
        builder
            .route(items, RoutePattern::path(":id").unwrap())
            .unwrap();
        RouteMatcher::new(builder.build().unwrap())
    }

    fn matched_branch(matcher: &RouteMatcher, path: &str) -> MatchedBranch {
        matcher.match_location(&RouteLocation::new(path)).unwrap()
    }

    fn apply_all_success(runtime: &mut RouterRuntime, snapshot: &NavigationTransactionSnapshot) {
        for request in snapshot.requests() {
            runtime
                .report_frame_apply(request.token(), FrameApplyOutcome::Applied)
                .unwrap();
        }
    }

    fn commit_branch(runtime: &mut RouterRuntime, target: &MatchedBranch) {
        let snapshot = runtime.begin_first_navigation(target).unwrap();
        apply_all_success(runtime, &snapshot);
        runtime.commit_ready_navigation().unwrap();
    }

    fn fail_pending_with_build_error(
        runtime: &mut RouterRuntime,
        token: &crate::transaction::FrameApplyToken,
    ) {
        runtime
            .report_frame_apply(
                token,
                FrameApplyOutcome::Failed(RouteError::new(
                    RouteErrorKind::ViewBuildFailed,
                    RouteErrorSourceCategory::RendererContract,
                    "view build failed",
                )),
            )
            .unwrap();
    }

    fn frame_id_raws(snapshot: &NavigationTransactionSnapshot) -> Vec<u64> {
        snapshot
            .requests()
            .iter()
            .map(|request| request.token().frame().raw())
            .collect()
    }

    fn assert_stale_reason(error: FrameApplyReportError, expected: StaleApplyReason) {
        match error {
            FrameApplyReportError::Stale(result) => assert_eq!(result.reason(), expected),
            other => panic!("expected stale result, got {other:?}"),
        }
    }

    type TokenMutator =
        fn(crate::transaction::FrameApplyToken) -> crate::transaction::FrameApplyToken;

    fn stale_cases(target: &MatchedBranch) -> Vec<(TokenMutator, StaleApplyReason)> {
        let _ = target;
        vec![
            (
                |mut token| {
                    token.testing_set_transaction_for_stale_check();
                    token
                },
                StaleApplyReason::TransactionMismatch,
            ),
            (
                |mut token| {
                    token.testing_set_sequence(token.sequence() + 1);
                    token
                },
                StaleApplyReason::SequenceMismatch,
            ),
            (
                |mut token| {
                    token.testing_set_router(RouterInstanceId::new(token.router().raw() + 1));
                    token
                },
                StaleApplyReason::RouterMismatch,
            ),
            (
                |mut token| {
                    token.testing_set_host(RouteHostId::new(token.host().raw() + 1));
                    token
                },
                StaleApplyReason::HostMismatch,
            ),
            (
                |mut token| {
                    token.testing_set_slot(RouteSlotId::new(token.slot().raw() + 1));
                    token
                },
                StaleApplyReason::SlotMismatch,
            ),
            (
                |mut token| {
                    token.testing_set_route(RouteId::new(token.route().raw() + 1));
                    token
                },
                StaleApplyReason::RouteMismatch,
            ),
            (
                |mut token| {
                    token.testing_set_frame(RouteFrameId::new(token.frame().raw() + 1));
                    token
                },
                StaleApplyReason::FrameMismatch,
            ),
            (
                |mut token| {
                    token.testing_set_generation(RouteGeneration::initial().next());
                    token
                },
                StaleApplyReason::GenerationMismatch,
            ),
        ]
    }
}
