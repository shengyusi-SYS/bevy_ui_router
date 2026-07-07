use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::RouteError;
use crate::frame::{
    FrameCleanupPlan, FrameReconciliationPlan, RouteFrameBranch, RouteFrameRecord, RouteFrameStatus,
};
use crate::ids::{
    RouteFrameId, RouteGeneration, RouteHostId, RouteId, RouteSlotId, RouterInstanceId,
};

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
        self.transaction = NavigationTransactionId(self.transaction.0 + 1);
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
    cleanup: Vec<FrameCleanupPlan>,
}

impl TransactionFailure {
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

        match outcome {
            FrameApplyOutcome::Applied => {
                if !self.accepted.contains(&token.frame()) {
                    self.accepted.push(token.frame());
                }
                if self.accepted.len() == self.requests.len() {
                    self.state = NavigationTransactionState::ReadyToCommit;
                }
            }
            FrameApplyOutcome::Failed(_) => {
                self.state = NavigationTransactionState::Failed;
                self.failure = Some(TransactionFailure {
                    cleanup: self
                        .requests
                        .iter()
                        .map(FrameApplyRequest::frame)
                        .map(RouteFrameRecord::identity)
                        .copied()
                        .map(FrameCleanupPlan::router_owned_partial_output)
                        .collect(),
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

        let current_fingerprint = current_active.map(RouteFrameBranch::branch_fingerprint);
        if current_fingerprint != self.plan.baseline_fingerprint() {
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
    use crate::frame::{FrameAllocator, RouteFrameScope};
    use crate::ids::{
        RouteFrameId, RouteGeneration, RouteHostId, RouteId, RouteSlotId, RouterInstanceId,
    };
    use crate::location::RouteLocation;
    use crate::matcher::RouteMatcher;
    use crate::pattern::RoutePattern;
    use crate::route_tree::RouteTree;

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
