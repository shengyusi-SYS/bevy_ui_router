//! ```compile_fail
//! use bevy_ui_router_core::transaction::NavigationTransaction;
//!
//! let _ = core::mem::size_of::<NavigationTransaction>();
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_core::transaction::NavigationTransactionId;
//!
//! let _ = core::mem::size_of::<NavigationTransactionId>();
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_core::prelude::FrameApplyToken;
//!
//! let _ = FrameApplyToken {
//!     transaction: unreachable!(),
//!     sequence: 0,
//!     router: unreachable!(),
//!     host: unreachable!(),
//!     slot: unreachable!(),
//!     route: unreachable!(),
//!     frame: unreachable!(),
//!     generation: unreachable!(),
//! };
//! ```

use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::{RouteError, RouteErrorKind};
use crate::frame::{
    FrameCleanupPlan, FrameReconciliationPlan, RouteFrameBranch, RouteFrameRecord, RouteFrameStatus,
};
use crate::ids::{
    RouteFrameId, RouteGeneration, RouteHostId, RouteId, RouteSlotId, RouterInstanceId,
};
use crate::meta::ErrorBoundaryKey;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NavigationTransactionState {
    PendingApply,
    ReadyToCommit,
    Committed,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NavigationTransactionId(u64);

impl NavigationTransactionId {
    #[cfg_attr(not(test), allow(dead_code))]
    fn next() -> Self {
        static NEXT_TRANSACTION: AtomicU64 = AtomicU64::new(1);
        Self(NEXT_TRANSACTION.fetch_add(1, Ordering::Relaxed))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrameApplyToken {
    transaction: NavigationTransactionId,
    sequence: u64,
    router: RouterInstanceId,
    host: RouteHostId,
    slot: RouteSlotId,
    route: RouteId,
    frame: RouteFrameId,
    generation: RouteGeneration,
}

impl FrameApplyToken {
    #[cfg_attr(not(test), allow(dead_code))]
    fn from_record(
        transaction: NavigationTransactionId,
        sequence: u64,
        record: &RouteFrameRecord,
    ) -> Self {
        let identity = record.identity();
        Self {
            transaction,
            sequence,
            router: identity.router(),
            host: identity.host(),
            slot: identity.slot(),
            route: identity.route(),
            frame: identity.frame(),
            generation: identity.generation(),
        }
    }

    pub const fn sequence(&self) -> u64 {
        self.sequence
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

    pub const fn route(&self) -> RouteId {
        self.route
    }

    pub const fn frame(&self) -> RouteFrameId {
        self.frame
    }

    pub const fn generation(&self) -> RouteGeneration {
        self.generation
    }

    #[cfg(test)]
    pub(crate) fn testing_set_generation(&mut self, generation: RouteGeneration) {
        self.generation = generation;
    }

    #[cfg(test)]
    pub(crate) fn testing_set_transaction_for_stale_check(&mut self) {
        self.transaction = NavigationTransactionId(self.transaction.0 + 1_024);
    }

    #[cfg(test)]
    pub(crate) fn testing_set_sequence(&mut self, sequence: u64) {
        self.sequence = sequence;
    }

    #[cfg(test)]
    pub(crate) fn testing_set_router(&mut self, router: RouterInstanceId) {
        self.router = router;
    }

    #[cfg(test)]
    pub(crate) fn testing_set_host(&mut self, host: RouteHostId) {
        self.host = host;
    }

    #[cfg(test)]
    pub(crate) fn testing_set_slot(&mut self, slot: RouteSlotId) {
        self.slot = slot;
    }

    #[cfg(test)]
    pub(crate) fn testing_set_route(&mut self, route: RouteId) {
        self.route = route;
    }

    #[cfg(test)]
    pub(crate) fn testing_set_frame(&mut self, frame: RouteFrameId) {
        self.frame = frame;
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrameApplyRequest {
    token: FrameApplyToken,
    frame: RouteFrameRecord,
}

impl FrameApplyRequest {
    pub fn token(&self) -> &FrameApplyToken {
        &self.token
    }

    pub fn frame(&self) -> &RouteFrameRecord {
        &self.frame
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FrameApplyOutcome {
    Applied,
    Failed(RouteError),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StaleApplyReason {
    TransactionMismatch,
    SequenceMismatch,
    RouterMismatch,
    HostMismatch,
    SlotMismatch,
    RouteMismatch,
    FrameMismatch,
    GenerationMismatch,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaleApplyResult {
    reason: StaleApplyReason,
}

impl StaleApplyResult {
    pub const fn reason(&self) -> StaleApplyReason {
        self.reason
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransactionCommitErrorKind {
    ChangedCurrentBranch,
    PendingApply,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransactionCommitError {
    kind: TransactionCommitErrorKind,
}

impl TransactionCommitError {
    pub const fn kind(&self) -> TransactionCommitErrorKind {
        self.kind
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransactionFailure {
    failing_route: RouteId,
    error: RouteError,
    nearest_error_boundary: Option<ErrorBoundaryKey>,
    cleanup: Vec<FrameCleanupPlan>,
}

impl TransactionFailure {
    pub const fn failing_route(&self) -> RouteId {
        self.failing_route
    }

    pub fn error(&self) -> &RouteError {
        &self.error
    }

    pub fn nearest_error_boundary(&self) -> Option<&ErrorBoundaryKey> {
        self.nearest_error_boundary.as_ref()
    }

    pub fn cleanup(&self) -> &[FrameCleanupPlan] {
        &self.cleanup
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NavigationTransactionSnapshot {
    state: NavigationTransactionState,
    requests: Vec<FrameApplyRequest>,
    failure: Option<TransactionFailure>,
}

impl NavigationTransactionSnapshot {
    pub const fn state(&self) -> NavigationTransactionState {
        self.state
    }

    pub fn requests(&self) -> &[FrameApplyRequest] {
        &self.requests
    }

    pub fn failure(&self) -> Option<&TransactionFailure> {
        self.failure.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NavigationTransaction {
    plan: FrameReconciliationPlan,
    state: NavigationTransactionState,
    requests: Vec<FrameApplyRequest>,
    accepted: Vec<RouteFrameId>,
    failure: Option<TransactionFailure>,
}

impl NavigationTransaction {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn new(plan: FrameReconciliationPlan) -> Self {
        let id = NavigationTransactionId::next();
        let requests = plan
            .entering()
            .iter()
            .enumerate()
            .map(|(index, record)| FrameApplyRequest {
                token: FrameApplyToken::from_record(id, index as u64, record),
                frame: record.clone(),
            })
            .collect();
        let state = if plan.entering().is_empty() {
            NavigationTransactionState::ReadyToCommit
        } else {
            NavigationTransactionState::PendingApply
        };

        Self {
            plan,
            state,
            requests,
            accepted: Vec::new(),
            failure: None,
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn snapshot(&self) -> NavigationTransactionSnapshot {
        NavigationTransactionSnapshot {
            state: self.state,
            requests: self.requests.clone(),
            failure: self.failure.clone(),
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn accepted_count(&self) -> usize {
        self.accepted.len()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn accept_apply_result(
        &mut self,
        token: &FrameApplyToken,
        outcome: FrameApplyOutcome,
    ) -> Result<(), StaleApplyResult> {
        let expected = self
            .requests
            .iter()
            .find(|candidate| candidate.token().sequence() == token.sequence())
            .map(FrameApplyRequest::token)
            .ok_or(StaleApplyResult {
                reason: StaleApplyReason::SequenceMismatch,
            })?;

        check_token(expected, token)?;

        if self.state == NavigationTransactionState::Failed {
            return Ok(());
        }

        match outcome {
            FrameApplyOutcome::Applied => {
                if !self.accepted.contains(&token.frame()) {
                    self.accepted.push(token.frame());
                }
                if self.accepted.len() == self.requests.len() {
                    self.state = NavigationTransactionState::ReadyToCommit;
                }
            }
            FrameApplyOutcome::Failed(error) => {
                let error_kind = error.kind();
                let failed_record = self
                    .plan
                    .entering()
                    .iter()
                    .find(|record| record.identity().frame() == token.frame())
                    .expect("token was checked against entering records");
                self.state = NavigationTransactionState::Failed;
                self.failure = Some(TransactionFailure {
                    failing_route: token.route(),
                    error,
                    nearest_error_boundary: self
                        .plan
                        .nearest_error_boundary_for(token.route())
                        .cloned(),
                    cleanup: self.cleanup_for_failure(
                        *failed_record.identity(),
                        token.frame(),
                        error_kind,
                    ),
                });
            }
        }

        Ok(())
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn commit(
        self,
        current_active: Option<&RouteFrameBranch>,
    ) -> Result<RouteFrameBranch, TransactionCommitError> {
        if self.state == NavigationTransactionState::Failed {
            return Err(TransactionCommitError {
                kind: TransactionCommitErrorKind::Failed,
            });
        }
        if self.state != NavigationTransactionState::ReadyToCommit {
            return Err(TransactionCommitError {
                kind: TransactionCommitErrorKind::PendingApply,
            });
        }

        if !self.plan.matches_current_active(current_active) {
            return Err(TransactionCommitError {
                kind: TransactionCommitErrorKind::ChangedCurrentBranch,
            });
        }

        let frames = self
            .plan
            .preserved()
            .iter()
            .cloned()
            .chain(
                self.plan
                    .entering()
                    .iter()
                    .map(|record| record.with_status(RouteFrameStatus::Active)),
            )
            .collect();

        Ok(RouteFrameBranch::new(
            frames,
            self.plan.target_fingerprint().clone(),
        ))
    }

    fn cleanup_for_failure(
        &self,
        failed_identity: crate::frame::RouteFrameIdentity,
        failed_frame: RouteFrameId,
        error_kind: RouteErrorKind,
    ) -> Vec<FrameCleanupPlan> {
        let mut cleanup = self
            .accepted
            .iter()
            .filter_map(|frame| {
                self.plan
                    .entering()
                    .iter()
                    .find(|record| record.identity().frame() == *frame)
            })
            .map(|record| FrameCleanupPlan::router_owned_partial_output(*record.identity()))
            .collect::<Vec<_>>();
        if error_kind == RouteErrorKind::ViewApplyFailed
            && !cleanup
                .iter()
                .any(|plan| plan.identity().frame() == failed_frame)
        {
            cleanup.push(FrameCleanupPlan::router_owned_partial_output(
                failed_identity,
            ));
        }
        cleanup
    }
}

#[cfg_attr(not(test), allow(dead_code))]
fn check_token(
    expected: &FrameApplyToken,
    actual: &FrameApplyToken,
) -> Result<(), StaleApplyResult> {
    let reason = if expected.sequence != actual.sequence {
        Some(StaleApplyReason::SequenceMismatch)
    } else if expected.router != actual.router {
        Some(StaleApplyReason::RouterMismatch)
    } else if expected.host != actual.host {
        Some(StaleApplyReason::HostMismatch)
    } else if expected.slot != actual.slot {
        Some(StaleApplyReason::SlotMismatch)
    } else if expected.route != actual.route {
        Some(StaleApplyReason::RouteMismatch)
    } else if expected.frame != actual.frame {
        Some(StaleApplyReason::FrameMismatch)
    } else if expected.generation != actual.generation {
        Some(StaleApplyReason::GenerationMismatch)
    } else if expected.transaction != actual.transaction {
        Some(StaleApplyReason::TransactionMismatch)
    } else {
        None
    };

    match reason {
        Some(reason) => Err(StaleApplyResult { reason }),
        None => Ok(()),
    }
}

#[cfg(test)]
pub(crate) struct FakeApplyHarness;

#[cfg(test)]
pub(crate) struct FakePartialOutput {
    token: FrameApplyToken,
    frame: RouteFrameId,
}

#[cfg(test)]
impl FakePartialOutput {
    pub(crate) const fn frame(&self) -> RouteFrameId {
        self.frame
    }
}

#[cfg(test)]
impl FakeApplyHarness {
    pub(crate) fn build_success(token: &FrameApplyToken) -> FakePartialOutput {
        FakePartialOutput {
            token: token.clone(),
            frame: token.frame(),
        }
    }

    pub(crate) fn build_failure(
        token: &FrameApplyToken,
        error: RouteError,
    ) -> (FrameApplyToken, FrameApplyOutcome) {
        (token.clone(), FrameApplyOutcome::Failed(error))
    }

    pub(crate) fn apply_success(output: FakePartialOutput) -> (FrameApplyToken, FrameApplyOutcome) {
        (output.token, FrameApplyOutcome::Applied)
    }

    pub(crate) fn apply_failure(
        output: FakePartialOutput,
        error: RouteError,
    ) -> (FrameApplyToken, FrameApplyOutcome) {
        (output.token, FrameApplyOutcome::Failed(error))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::branch_diff::BranchDiff;
    use crate::error::{RouteError, RouteErrorKind, RouteErrorSourceCategory};
    use crate::frame::{
        FrameAllocator, RouteFrameIdentity, RouteFrameRecord, RouteFrameScope, RouteFrameStatus,
    };
    use crate::ids::{
        RouteFrameId, RouteGeneration, RouteHostId, RouteId, RouteSlotId, RouterInstanceId,
    };
    use crate::location::RouteLocation;
    use crate::matcher::RouteMatcher;
    use crate::meta::ErrorBoundaryKey;
    use crate::pattern::RoutePattern;
    use crate::route_tree::{RouteNodeOptions, RouteTree};

    fn branches() -> (RouteFrameBranch, FrameReconciliationPlan) {
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
        let matcher = RouteMatcher::new(builder.build().unwrap());
        let old_branch = matcher
            .match_location(&RouteLocation::new("/items/1"))
            .unwrap();
        let new_branch = matcher
            .match_location(&RouteLocation::new("/items/2"))
            .unwrap();
        let diff = BranchDiff::compare(&old_branch, &new_branch);
        let mut allocator = FrameAllocator::new();
        let scope = RouteFrameScope::new(
            RouterInstanceId::new(1),
            RouteHostId::new(2),
            RouteSlotId::new(3),
        );
        let active = RouteFrameBranch::from_matched_branch(scope, &old_branch, &mut allocator);
        let plan = FrameReconciliationPlan::from_diff(
            scope,
            Some(&active),
            &new_branch,
            &diff,
            &mut allocator,
        );
        (active, plan)
    }

    fn branches_with_error_boundary() -> (RouteFrameBranch, FrameReconciliationPlan) {
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
        let matcher = RouteMatcher::new(builder.build().unwrap());
        let old_branch = matcher
            .match_location(&RouteLocation::new("/section/items/1"))
            .unwrap();
        let new_branch = matcher
            .match_location(&RouteLocation::new("/section/items/2"))
            .unwrap();
        let diff = BranchDiff::compare(&old_branch, &new_branch);
        let mut allocator = FrameAllocator::new();
        let scope = RouteFrameScope::new(
            RouterInstanceId::new(1),
            RouteHostId::new(2),
            RouteSlotId::new(3),
        );
        let active = RouteFrameBranch::from_matched_branch(scope, &old_branch, &mut allocator);
        let plan = FrameReconciliationPlan::from_diff(
            scope,
            Some(&active),
            &new_branch,
            &diff,
            &mut allocator,
        );
        (active, plan)
    }

    fn branches_without_error_boundary() -> (RouteFrameBranch, FrameReconciliationPlan) {
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
        let matcher = RouteMatcher::new(builder.build().unwrap());
        let old_branch = matcher
            .match_location(&RouteLocation::new("/items/1"))
            .unwrap();
        let new_branch = matcher
            .match_location(&RouteLocation::new("/items/2"))
            .unwrap();
        let diff = BranchDiff::compare(&old_branch, &new_branch);
        let mut allocator = FrameAllocator::new();
        let scope = RouteFrameScope::new(
            RouterInstanceId::new(1),
            RouteHostId::new(2),
            RouteSlotId::new(3),
        );
        let active = RouteFrameBranch::from_matched_branch(scope, &old_branch, &mut allocator);
        let plan = FrameReconciliationPlan::from_diff(
            scope,
            Some(&active),
            &new_branch,
            &diff,
            &mut allocator,
        );
        (active, plan)
    }

    fn branches_with_three_entering_frames() -> (RouteFrameBranch, FrameReconciliationPlan) {
        let mut builder = RouteTree::builder()
            .root(RoutePattern::path("/").unwrap())
            .unwrap();
        let root = builder.root_id().unwrap();
        let section = builder
            .route(root, RoutePattern::path("section").unwrap())
            .unwrap();
        let items = builder
            .route(section, RoutePattern::path("items").unwrap())
            .unwrap();
        builder
            .route(items, RoutePattern::path(":id").unwrap())
            .unwrap();
        let matcher = RouteMatcher::new(builder.build().unwrap());
        let old_branch = matcher.match_location(&RouteLocation::new("/")).unwrap();
        let new_branch = matcher
            .match_location(&RouteLocation::new("/section/items/1"))
            .unwrap();
        let diff = BranchDiff::compare(&old_branch, &new_branch);
        let mut allocator = FrameAllocator::new();
        let scope = RouteFrameScope::new(
            RouterInstanceId::new(1),
            RouteHostId::new(2),
            RouteSlotId::new(3),
        );
        let active = RouteFrameBranch::from_matched_branch(scope, &old_branch, &mut allocator);
        let plan = FrameReconciliationPlan::from_diff(
            scope,
            Some(&active),
            &new_branch,
            &diff,
            &mut allocator,
        );
        assert_eq!(plan.entering().len(), 3);
        (active, plan)
    }

    fn plan_with_three_entering_requests() -> FrameReconciliationPlan {
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
        let matcher = RouteMatcher::new(builder.build().unwrap());
        let target = matcher
            .match_location(&RouteLocation::new("/items/2"))
            .unwrap();
        let mut allocator = FrameAllocator::new();
        let scope = RouteFrameScope::new(
            RouterInstanceId::new(1),
            RouteHostId::new(2),
            RouteSlotId::new(3),
        );
        FrameReconciliationPlan::from_first_navigation(scope, &target, &mut allocator)
    }

    fn cleanup_frame_ids(failure: &TransactionFailure) -> Vec<RouteFrameId> {
        failure
            .cleanup()
            .iter()
            .map(|plan| plan.identity().frame())
            .collect()
    }

    #[test]
    fn all_apply_token_identity_mismatches_are_stale_without_side_effects() {
        let (_active, plan) = branches();
        let transaction = NavigationTransaction::new(plan.clone());
        for (stale, expected) in stale_token_cases(&transaction) {
            let mut transaction = NavigationTransaction::new(plan.clone());

            let result = transaction.accept_apply_result(&stale, FrameApplyOutcome::Applied);

            assert_eq!(result.unwrap_err().reason(), expected);
            assert_eq!(
                transaction.snapshot().state(),
                NavigationTransactionState::PendingApply
            );
            assert_eq!(transaction.accepted_count(), 0);
            assert!(transaction.snapshot().failure().is_none());
        }
    }

    #[test]
    fn commit_rejects_changed_current_active_branch() {
        let (_active, plan) = branches();
        let mut transaction = NavigationTransaction::new(plan.clone());
        let token = transaction.snapshot().requests()[0].token().clone();
        transaction
            .accept_apply_result(&token, FrameApplyOutcome::Applied)
            .unwrap();
        let changed_active = RouteFrameBranch::new(Vec::new(), plan.target_fingerprint().clone());

        let result = transaction.commit(Some(&changed_active));

        assert_eq!(
            result.unwrap_err().kind(),
            TransactionCommitErrorKind::ChangedCurrentBranch
        );
    }

    #[test]
    fn commit_rejects_remounted_current_active_branch_with_same_fingerprint() {
        let (active, plan) = branches();
        let mut transaction = NavigationTransaction::new(plan);
        let requests = transaction.snapshot().requests().to_vec();
        for request in requests {
            transaction
                .accept_apply_result(request.token(), FrameApplyOutcome::Applied)
                .unwrap();
        }
        let remounted_active = RouteFrameBranch::new(
            active
                .frames()
                .iter()
                .enumerate()
                .map(|(index, record)| {
                    let route = record.identity().route();
                    RouteFrameRecord::new(
                        RouteFrameIdentity::new(
                            record.identity().router(),
                            record.identity().host(),
                            record.identity().slot(),
                            route,
                            RouteFrameId::new(100 + index as u64),
                            RouteGeneration::initial().next(),
                        ),
                        record.branch_index(),
                        record.slot_key().clone(),
                        record.segment_reuse_key().clone(),
                        RouteFrameStatus::Active,
                    )
                })
                .collect(),
            active.branch_fingerprint().clone(),
        );

        let result = transaction.commit(Some(&remounted_active));

        assert_eq!(
            result.unwrap_err().kind(),
            TransactionCommitErrorKind::ChangedCurrentBranch
        );
    }

    #[test]
    fn fake_apply_harness_models_build_apply_and_partial_output_paths() {
        let (_active, plan) = branches();

        let mut success = NavigationTransaction::new(plan.clone());
        let built = FakeApplyHarness::build_success(success.snapshot().requests()[0].token());
        assert_eq!(
            built.frame(),
            success.snapshot().requests()[0].token().frame()
        );
        let (token, outcome) = FakeApplyHarness::apply_success(built);
        success.accept_apply_result(&token, outcome).unwrap();
        assert_eq!(
            success.snapshot().state(),
            NavigationTransactionState::ReadyToCommit
        );

        let mut build_failure = NavigationTransaction::new(plan.clone());
        let (token, outcome) = FakeApplyHarness::build_failure(
            build_failure.snapshot().requests()[0].token(),
            RouteError::new(
                RouteErrorKind::ViewBuildFailed,
                RouteErrorSourceCategory::RendererContract,
                "view build failed",
            ),
        );
        build_failure.accept_apply_result(&token, outcome).unwrap();
        assert_eq!(
            build_failure.snapshot().state(),
            NavigationTransactionState::Failed
        );

        let mut apply_failure = NavigationTransaction::new(plan);
        let built = FakeApplyHarness::build_success(apply_failure.snapshot().requests()[0].token());
        let (token, outcome) = FakeApplyHarness::apply_failure(
            built,
            RouteError::new(
                RouteErrorKind::ViewApplyFailed,
                RouteErrorSourceCategory::RendererContract,
                "view apply failed",
            ),
        );
        apply_failure.accept_apply_result(&token, outcome).unwrap();
        assert_eq!(
            apply_failure.snapshot().state(),
            NavigationTransactionState::Failed
        );
    }

    #[test]
    fn failed_transactions_ignore_later_valid_apply_results_but_still_reject_stale_tokens() {
        let (_active, plan) = branches();
        let mut transaction = NavigationTransaction::new(plan);
        let token = transaction.snapshot().requests()[0].token().clone();
        let stale = {
            let mut token = token.clone();
            token.testing_set_router(RouterInstanceId::new(9));
            token
        };

        transaction
            .accept_apply_result(
                &token,
                FrameApplyOutcome::Failed(RouteError::new(
                    RouteErrorKind::ViewApplyFailed,
                    RouteErrorSourceCategory::RendererContract,
                    "apply failed",
                )),
            )
            .unwrap();
        let failed_snapshot = transaction.snapshot();

        let stale_result = transaction.accept_apply_result(&stale, FrameApplyOutcome::Applied);
        assert_eq!(
            stale_result.unwrap_err().reason(),
            StaleApplyReason::RouterMismatch
        );

        transaction
            .accept_apply_result(&token, FrameApplyOutcome::Applied)
            .unwrap();

        assert_eq!(transaction.snapshot(), failed_snapshot);
        assert_eq!(transaction.accepted_count(), 0);
        assert_eq!(
            transaction.snapshot().state(),
            NavigationTransactionState::Failed
        );
    }

    #[test]
    fn cleanup_tracks_only_actual_partial_output_for_apply_failures() {
        let plan = plan_with_three_entering_requests();
        let mut transaction = NavigationTransaction::new(plan);

        let first_built =
            FakeApplyHarness::build_success(transaction.snapshot().requests()[0].token());
        let (first_token, first_outcome) = FakeApplyHarness::apply_success(first_built);
        transaction
            .accept_apply_result(&first_token, first_outcome)
            .unwrap();

        let second_built =
            FakeApplyHarness::build_success(transaction.snapshot().requests()[1].token());
        let failed_frame = second_built.frame();
        let (second_token, second_outcome) = FakeApplyHarness::apply_failure(
            second_built,
            RouteError::new(
                RouteErrorKind::ViewApplyFailed,
                RouteErrorSourceCategory::RendererContract,
                "apply failed",
            ),
        );
        transaction
            .accept_apply_result(&second_token, second_outcome)
            .unwrap();

        let failure = transaction.snapshot().failure().cloned().unwrap();
        let frame_ids = cleanup_frame_ids(&failure);

        assert_eq!(frame_ids.len(), 2);
        assert_eq!(frame_ids[0], first_token.frame());
        assert_eq!(frame_ids[1], failed_frame);
        assert!(!frame_ids.contains(&transaction.snapshot().requests()[2].token().frame()));
    }

    #[test]
    fn apply_failure_preserves_old_active_and_limits_cleanup_scope() {
        let (active, plan) = branches_with_error_boundary();
        let mut transaction = NavigationTransaction::new(plan);
        let token = transaction.snapshot().requests()[0].token().clone();

        transaction
            .accept_apply_result(
                &token,
                FrameApplyOutcome::Failed(RouteError::new(
                    RouteErrorKind::ViewApplyFailed,
                    RouteErrorSourceCategory::RendererContract,
                    "view apply failed",
                )),
            )
            .unwrap();

        assert_eq!(
            transaction.snapshot().state(),
            NavigationTransactionState::Failed
        );
        let snapshot = transaction.snapshot();
        let failure = snapshot.failure().unwrap();
        assert_eq!(failure.failing_route(), token.route());
        assert_eq!(failure.cleanup().len(), 1);
        assert!(failure.cleanup()[0].scope().excludes_app_owned_host());
        assert_eq!(active.frames()[0].status(), RouteFrameStatus::Active);
    }

    #[test]
    fn failure_prefers_nearest_nested_error_boundary() {
        let (_active, plan) = branches_with_error_boundary();
        let mut transaction = NavigationTransaction::new(plan);
        let token = transaction.snapshot().requests()[0].token().clone();

        transaction
            .accept_apply_result(
                &token,
                FrameApplyOutcome::Failed(RouteError::new(
                    RouteErrorKind::ViewApplyFailed,
                    RouteErrorSourceCategory::RendererContract,
                    "view apply failed",
                )),
            )
            .unwrap();

        assert_eq!(
            transaction
                .snapshot()
                .failure()
                .unwrap()
                .nearest_error_boundary()
                .map(ErrorBoundaryKey::as_str),
            Some("items")
        );
    }

    #[test]
    fn failure_without_boundary_records_router_fallback() {
        let (_active, plan) = branches_without_error_boundary();
        let mut transaction = NavigationTransaction::new(plan);
        let token = transaction.snapshot().requests()[0].token().clone();

        transaction
            .accept_apply_result(
                &token,
                FrameApplyOutcome::Failed(RouteError::new(
                    RouteErrorKind::ViewApplyFailed,
                    RouteErrorSourceCategory::RendererContract,
                    "view apply failed",
                )),
            )
            .unwrap();

        assert!(
            transaction
                .snapshot()
                .failure()
                .unwrap()
                .nearest_error_boundary()
                .is_none()
        );
    }

    #[test]
    fn multi_frame_failure_cleans_only_attempted_transaction_output() {
        let (_active, plan) = branches_with_three_entering_frames();
        let mut transaction = NavigationTransaction::new(plan);
        let first = transaction.snapshot().requests()[0].token().clone();
        let second = transaction.snapshot().requests()[1].token().clone();

        transaction
            .accept_apply_result(&first, FrameApplyOutcome::Applied)
            .unwrap();
        transaction
            .accept_apply_result(
                &second,
                FrameApplyOutcome::Failed(RouteError::new(
                    RouteErrorKind::ViewApplyFailed,
                    RouteErrorSourceCategory::RendererContract,
                    "view apply failed",
                )),
            )
            .unwrap();

        let snapshot = transaction.snapshot();
        let cleanup = snapshot.failure().unwrap().cleanup();
        assert_eq!(cleanup.len(), 2);
        assert_eq!(cleanup[0].identity().frame(), first.frame());
        assert_eq!(cleanup[1].identity().frame(), second.frame());
        assert!(
            cleanup
                .iter()
                .all(|plan| plan.scope().excludes_app_owned_host())
        );
    }

    #[test]
    fn build_failure_before_partial_output_does_not_cleanup_failed_frame() {
        let plan = plan_with_three_entering_requests();
        let mut transaction = NavigationTransaction::new(plan);

        let first_built =
            FakeApplyHarness::build_success(transaction.snapshot().requests()[0].token());
        let (first_token, first_outcome) = FakeApplyHarness::apply_success(first_built);
        transaction
            .accept_apply_result(&first_token, first_outcome)
            .unwrap();

        let failed_token = transaction.snapshot().requests()[1].token().clone();
        let (second_token, second_outcome) = FakeApplyHarness::build_failure(
            &failed_token,
            RouteError::new(
                RouteErrorKind::ViewBuildFailed,
                RouteErrorSourceCategory::RendererContract,
                "build failed",
            ),
        );
        transaction
            .accept_apply_result(&second_token, second_outcome)
            .unwrap();

        let failure = transaction.snapshot().failure().cloned().unwrap();
        let frame_ids = cleanup_frame_ids(&failure);

        assert_eq!(frame_ids, vec![first_token.frame()]);
        assert!(!frame_ids.contains(&failed_token.frame()));
    }

    fn stale_token_cases(
        transaction: &NavigationTransaction,
    ) -> Vec<(FrameApplyToken, StaleApplyReason)> {
        let base = transaction.snapshot().requests()[0].token().clone();
        let mut cases = Vec::new();

        let mut token = base.clone();
        token.testing_set_transaction_for_stale_check();
        cases.push((token, StaleApplyReason::TransactionMismatch));

        let mut token = base.clone();
        token.testing_set_sequence(999);
        cases.push((token, StaleApplyReason::SequenceMismatch));

        let mut token = base.clone();
        token.testing_set_router(RouterInstanceId::new(9));
        cases.push((token, StaleApplyReason::RouterMismatch));

        let mut token = base.clone();
        token.testing_set_host(RouteHostId::new(9));
        cases.push((token, StaleApplyReason::HostMismatch));

        let mut token = base.clone();
        token.testing_set_slot(RouteSlotId::new(9));
        cases.push((token, StaleApplyReason::SlotMismatch));

        let mut token = base.clone();
        token.testing_set_route(RouteId::new(9));
        cases.push((token, StaleApplyReason::RouteMismatch));

        let mut token = base.clone();
        token.testing_set_frame(RouteFrameId::new(9));
        cases.push((token, StaleApplyReason::FrameMismatch));

        let mut token = base;
        token.testing_set_generation(RouteGeneration::initial().next());
        cases.push((token, StaleApplyReason::GenerationMismatch));

        cases
    }
}
