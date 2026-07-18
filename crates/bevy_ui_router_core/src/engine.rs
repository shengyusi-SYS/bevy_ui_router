#![allow(dead_code)]

use std::collections::VecDeque;

use crate::admission::{
    AdmissionError, HostAdmissionConfirmation, HostAdmissionIssuer, HostAdmissionToken, HostGrant,
};
use crate::branch::{MatchedBranch, MatchedRoute, RouteParams};
use crate::cleanup::{CleanupBatchToken, CleanupCompletion, CleanupManifestSeal};
use crate::diagnostics::{DiagnosticEvent, ReportDisposition};
use crate::frame::{
    FrameOrigin, LogicalChildSlotBinding, ReconciliationProjectionError, RouteFrameVersion,
    RouteReconciliationProjection,
};
use crate::history::{HistoryIntent, MemoryHistory};
use crate::ids::{
    CoreIdAllocator, FrameTaskId, NavigationRequestId, ReservedBranchAttempt, ReservedFrameAttempt,
    RouteFrameId, RouteGeneration, RouteHostId, RouteSlotId, RouterIdentity, TransactionId,
};
use crate::location::RouteLocation;
use crate::matcher::RouteMatcher;
use crate::mount::{
    MountConfirmation, MountError, MountGrant, MountIssuer, MountReceiptAttestation,
    MountReceiptProof, MountRegistrationCompletion, MountRegistrationDigest,
    MountRegistrationPermit, MountRequestToken,
};
use crate::presentation::{ActivePresentation, ActivePresentationKind, RouterNavigationState};
use crate::request::{
    NavigationFailure, NavigationFailureKind, NavigationPhase, NavigationRequestRecord,
    RequestStatus,
};
use crate::route_tree::RouteTree;
use crate::trace::{MountRejectionKind, NavigationTraceEvent, NavigationTraceEventKind};
use crate::view_input::RouteViewInput;
use crate::view_key::RouteViewKey;
use crate::{HostVersion, slot::RouteSlotKey};

const TERMINAL_RETENTION: usize = 64;
const ROUTER_DIAGNOSTIC_RETENTION: usize = 64;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FrameTaskIdentity {
    router: RouterIdentity,
    host: RouteHostId,
    host_epoch: u64,
    slot: RouteSlotId,
    request: NavigationRequestId,
    transaction: TransactionId,
    frame_task: FrameTaskId,
    frame: RouteFrameId,
    generation: RouteGeneration,
    origin: FrameOrigin,
    branch_index: usize,
}

impl FrameTaskIdentity {
    pub(crate) const fn router_identity(&self) -> RouterIdentity {
        self.router
    }

    pub(crate) const fn host(&self) -> RouteHostId {
        self.host
    }

    pub(crate) const fn host_epoch(&self) -> u64 {
        self.host_epoch
    }

    pub(crate) const fn slot(&self) -> RouteSlotId {
        self.slot
    }

    pub(crate) const fn request(&self) -> NavigationRequestId {
        self.request
    }

    pub(crate) const fn transaction(&self) -> TransactionId {
        self.transaction
    }

    pub(crate) const fn frame_task(&self) -> FrameTaskId {
        self.frame_task
    }

    pub(crate) const fn frame(&self) -> RouteFrameId {
        self.frame
    }

    pub(crate) const fn generation(&self) -> RouteGeneration {
        self.generation
    }

    pub(crate) const fn origin(&self) -> &FrameOrigin {
        &self.origin
    }

    pub(crate) const fn branch_index(&self) -> usize {
        self.branch_index
    }

    #[cfg(test)]
    fn testing_set_router(&mut self, router: RouterIdentity) {
        self.router = router;
    }

    #[cfg(test)]
    fn testing_set_host(&mut self, host: RouteHostId) {
        self.host = host;
    }

    #[cfg(test)]
    fn testing_set_host_epoch(&mut self, epoch: u64) {
        self.host_epoch = epoch;
    }

    #[cfg(test)]
    fn testing_set_top_slot(&mut self, slot: RouteSlotId) {
        self.slot = slot;
    }

    #[cfg(test)]
    fn testing_set_request(&mut self, request: NavigationRequestId) {
        self.request = request;
    }

    #[cfg(test)]
    fn testing_set_transaction(&mut self, transaction: TransactionId) {
        self.transaction = transaction;
    }

    #[cfg(test)]
    fn testing_set_frame_task(&mut self, task: FrameTaskId) {
        self.frame_task = task;
    }

    #[cfg(test)]
    fn testing_set_frame(&mut self, frame: RouteFrameId) {
        self.frame = frame;
    }

    #[cfg(test)]
    fn testing_set_generation(&mut self, generation: RouteGeneration) {
        self.generation = generation;
    }

    #[cfg(test)]
    fn testing_set_origin(&mut self, origin: FrameOrigin) {
        self.origin = origin;
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum FrameMountTarget {
    TopSlot {
        host: HostVersion,
        slot: RouteSlotId,
    },
    ChildOutlet {
        parent: RouteFrameVersion,
        outlet: RouteSlotKey,
    },
}

pub(crate) struct FrameTask {
    identity: FrameTaskIdentity,
    frame: RouteFrameVersion,
    target: FrameMountTarget,
    location: RouteLocation,
    matched: Option<MatchedRoute>,
    view: RouteViewKey,
    input: Option<RouteViewInput>,
}

impl FrameTask {
    pub(crate) const fn identity(&self) -> &FrameTaskIdentity {
        &self.identity
    }

    pub(crate) const fn frame_version(&self) -> &RouteFrameVersion {
        &self.frame
    }

    pub(crate) const fn location(&self) -> &RouteLocation {
        &self.location
    }

    #[allow(dead_code)]
    pub(crate) const fn matched(&self) -> Option<&MatchedRoute> {
        self.matched.as_ref()
    }

    pub(crate) const fn view(&self) -> &RouteViewKey {
        &self.view
    }

    pub(crate) const fn input(&self) -> &RouteViewInput {
        self.input
            .as_ref()
            .expect("frame task input is available before driver dispatch")
    }

    pub(crate) fn take_input(&mut self) -> RouteViewInput {
        self.input
            .take()
            .expect("frame task input is consumed exactly once")
    }

    pub(crate) const fn mount_target(&self) -> &FrameMountTarget {
        &self.target
    }

    #[cfg(test)]
    pub(crate) fn testing_duplicate(&self) -> Self {
        Self {
            identity: self.identity.clone(),
            frame: self.frame.clone(),
            target: self.target.clone(),
            location: self.location.clone(),
            matched: self.matched.clone(),
            view: self.view.clone(),
            input: None,
        }
    }

    pub(crate) fn into_applied_report(self, outlets: Vec<RouteSlotKey>) -> AppliedFrameReport {
        AppliedFrameReport {
            identity: self.identity,
            outlets,
            mode: AppliedReportMode::OutletAware,
        }
    }

    pub(crate) fn into_legacy_applied_report(self) -> AppliedFrameReport {
        AppliedFrameReport {
            identity: self.identity,
            outlets: Vec::new(),
            mode: AppliedReportMode::LegacyLeaf,
        }
    }
}

pub(crate) type RootFrameTask = FrameTask;
pub(crate) type RootTaskIdentity = FrameTaskIdentity;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AppliedFrameReport {
    identity: FrameTaskIdentity,
    outlets: Vec<RouteSlotKey>,
    mode: AppliedReportMode,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AppliedReportMode {
    OutletAware,
    LegacyLeaf,
}

impl AppliedFrameReport {
    #[cfg(test)]
    pub(crate) fn testing_duplicate(&self) -> Self {
        self.clone()
    }
}

pub(crate) struct FrameMountRequest {
    token: MountRequestToken,
    request: NavigationRequestId,
    parent: RouteFrameVersion,
    outlet: RouteSlotKey,
}

pub(crate) enum MountMutationArtifact {
    Grant(MountGrant),
    Receipt(MountReceiptAttestation),
    RegistrationPermit(MountRegistrationPermit),
}

pub(crate) enum ResolveMountOwner {
    Returned(FrameMountRequest),
    Reissued,
}

pub(crate) struct MountCapabilityError<Owner> {
    kind: MountError,
    owner: Owner,
}

impl<Owner> MountCapabilityError<Owner> {
    const fn new(kind: MountError, owner: Owner) -> Self {
        Self { kind, owner }
    }

    pub(crate) const fn kind(&self) -> MountError {
        self.kind
    }

    pub(crate) fn into_owner(self) -> Owner {
        self.owner
    }
}

impl<Owner> std::fmt::Debug for MountCapabilityError<Owner> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MountCapabilityError")
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum PendingFailureKind {
    Apply,
    Build,
    Mount(MountRejectionKind),
    MountIssue,
    AuthorityLost,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FailureCleanupContinuation {
    Terminal,
    BeginFallback,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FailureCleanupRequest {
    identity: FrameTaskIdentity,
    expected: PendingFailureKind,
    logical_roots: Vec<RouteFrameVersion>,
    failure: NavigationFailure,
    continuation: FailureCleanupContinuation,
}

pub(crate) struct FailureCleanupPrepareError {
    kind: FailureCleanupError,
    owner: FailureCleanupRequest,
}

impl FailureCleanupPrepareError {
    const fn new(kind: FailureCleanupError, owner: FailureCleanupRequest) -> Self {
        Self { kind, owner }
    }

    pub(crate) const fn kind(&self) -> FailureCleanupError {
        self.kind
    }

    pub(crate) fn into_owner(self) -> FailureCleanupRequest {
        self.owner
    }
}

impl std::fmt::Debug for FailureCleanupPrepareError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FailureCleanupPrepareError")
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}

impl FailureCleanupRequest {
    pub(crate) const fn request(&self) -> NavigationRequestId {
        self.identity.request()
    }

    pub(crate) fn logical_roots(&self) -> &[RouteFrameVersion] {
        &self.logical_roots
    }

    pub(crate) const fn failure(&self) -> &NavigationFailure {
        &self.failure
    }

    #[cfg(test)]
    pub(crate) fn testing_duplicate(&self) -> Self {
        self.clone()
    }
}

impl FrameMountRequest {
    pub(crate) const fn request(&self) -> NavigationRequestId {
        self.request
    }

    pub(crate) const fn parent(&self) -> &RouteFrameVersion {
        &self.parent
    }

    pub(crate) const fn outlet(&self) -> &RouteSlotKey {
        &self.outlet
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn testing_duplicate(&self) -> Self {
        Self {
            token: self.token.clone(),
            request: self.request,
            parent: self.parent.clone(),
            outlet: self.outlet.clone(),
        }
    }
}

pub(crate) enum EngineAction {
    FrameTask(FrameTask),
    MountRequest(FrameMountRequest),
    FailureCleanup(FailureCleanupRequest),
    Commit(CommitTicket),
}

#[derive(Debug)]
pub(crate) struct CommitTicket {
    identity: FrameTaskIdentity,
}

impl CommitTicket {
    #[cfg(test)]
    fn testing_duplicate(&self) -> Self {
        Self {
            identity: self.identity.clone(),
        }
    }

    #[cfg(test)]
    fn testing_set_host_epoch(&mut self, epoch: u64) {
        self.identity.host_epoch = epoch;
    }

    pub(crate) const fn identity(&self) -> &RootTaskIdentity {
        &self.identity
    }

    #[cfg(test)]
    fn testing_with_identity(&self, identity: RootTaskIdentity) -> Self {
        Self { identity }
    }
}

#[derive(Debug)]
pub(crate) struct SealedCommit {
    identity: FrameTaskIdentity,
    cleanup: CleanupBatchToken,
    publication: PublicationPlan,
}

impl SealedCommit {
    pub(crate) const fn cleanup(&self) -> &CleanupBatchToken {
        &self.cleanup
    }

    #[cfg(test)]
    fn testing_duplicate(&self) -> Self {
        Self {
            identity: self.identity.clone(),
            cleanup: self.cleanup.clone(),
            publication: self.publication.clone(),
        }
    }

    #[cfg(test)]
    fn testing_with_identity(&self, identity: RootTaskIdentity) -> Self {
        Self {
            identity,
            cleanup: self.cleanup.clone(),
            publication: self.publication.clone(),
        }
    }
}

#[derive(Clone, Debug)]
struct PublicationPlan {
    state: RouterNavigationState,
    history: MemoryHistory,
    active_branch: Option<MatchedBranch>,
    active_child_slots: Vec<LogicalChildSlotBinding>,
    record_index: usize,
    published_record: NavigationRequestRecord,
    current: CurrentWork,
}

pub(crate) struct PreparedPublication {
    sealed: SealedCommit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EngineInstallError {
    IdExhausted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PushError {
    AwaitingHost,
    IdExhausted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReportError {
    Duplicate,
    Stale,
    Invalid,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CommitError {
    Stale,
    InvalidManifest,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CleanupError {
    Duplicate,
    Stale,
    Invalid,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FailureCleanupError {
    Foreign,
    Stale,
    InvalidManifest,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PrivateRequestLookup<'a> {
    Retained(&'a NavigationRequestRecord),
    Evicted,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum WorkStage {
    Applying,
    AwaitingMount,
    AwaitingReceipt,
    ConfirmingMount,
    ReadyToCommit,
    CommitSealed {
        token: CleanupBatchToken,
    },
    PostCommitCleanup {
        outstanding: CleanupBatchToken,
    },
    FailureCleanupManifestPending {
        expected: FailureCleanupRequest,
    },
    FailureCleanupPrepared {
        outstanding: CleanupBatchToken,
        failure: NavigationFailure,
        continuation: FailureCleanupContinuation,
    },
}

#[derive(Clone, Debug)]
enum AttemptKind {
    Normal,
    Fallback { original_failure: NavigationFailure },
}

#[derive(Clone, Debug)]
struct CurrentWork {
    identity: FrameTaskIdentity,
    branch: Option<MatchedBranch>,
    location: RouteLocation,
    reservation: Vec<ReservedFrameAttempt>,
    target_frames: Vec<RouteFrameVersion>,
    applied_frames: Vec<RouteFrameVersion>,
    current_target: FrameMountTarget,
    pending_mount: Option<PendingMount>,
    history_intent: Option<HistoryIntent>,
    stage: WorkStage,
    cleanup_batch: crate::ids::CleanupBatchId,
    cleanup_roots: Vec<RouteFrameVersion>,
    attempt: AttemptKind,
    reconciliation: Option<RouteReconciliationProjection>,
}

#[derive(Clone, Debug)]
struct PendingMount {
    child_index: usize,
    parent: RouteFrameVersion,
    outlet: RouteSlotKey,
}

struct ConfirmedChildPublication {
    current: CurrentWork,
    record_index: usize,
    record: NavigationRequestRecord,
    phase: NavigationPhase,
    identity: FrameTaskIdentity,
    frame: RouteFrameVersion,
    target: FrameMountTarget,
    location: RouteLocation,
    matched: MatchedRoute,
    view: RouteViewKey,
    input: RouteViewInput,
}

#[derive(Clone, Debug)]
enum QueuedMatchOutcome {
    Matched(MatchedBranch),
    MatchFailed(NavigationFailure),
}

#[derive(Clone, Debug)]
struct QueuedPush {
    request: NavigationRequestId,
    outcome: QueuedMatchOutcome,
    reservation: Option<ReservedBranchAttempt>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ReportDispositionRecord {
    identity: FrameTaskIdentity,
    disposition: ReportDisposition,
    target: DiagnosticTarget,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DiagnosticTarget {
    Request(NavigationRequestId),
    Router,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CompletedCleanupTombstone {
    request: NavigationRequestId,
    token: CleanupBatchToken,
}

pub(crate) struct RouterEngine {
    matcher: RouteMatcher,
    ids: CoreIdAllocator,
    router: RouterIdentity,
    fallback: RouteViewKey,
    state: RouterNavigationState,
    history: MemoryHistory,
    active_branch: Option<MatchedBranch>,
    active_child_slots: Vec<LogicalChildSlotBinding>,
    records: VecDeque<NavigationRequestRecord>,
    evicted_through: Option<u64>,
    highest_issued: Option<u64>,
    queue: VecDeque<QueuedPush>,
    current: Option<CurrentWork>,
    action: Option<EngineAction>,
    issued_view_tasks: Vec<FrameTaskIdentity>,
    mount: MountIssuer,
    report_dispositions: Vec<ReportDispositionRecord>,
    router_diagnostics: Vec<DiagnosticEvent>,
    completed_cleanup: Vec<CompletedCleanupTombstone>,
}

impl RouterEngine {
    pub(crate) fn install(
        tree: RouteTree,
        fallback: RouteViewKey,
    ) -> Result<Self, EngineInstallError> {
        let mut ids = CoreIdAllocator::new().map_err(|_| EngineInstallError::IdExhausted)?;
        let router = ids
            .allocate_router_identity()
            .map_err(|_| EngineInstallError::IdExhausted)?;
        Ok(Self {
            matcher: RouteMatcher::new(tree),
            ids,
            router,
            fallback,
            state: RouterNavigationState::new(router),
            history: MemoryHistory::new(),
            active_branch: None,
            active_child_slots: Vec::new(),
            records: VecDeque::new(),
            evicted_through: None,
            highest_issued: None,
            queue: VecDeque::new(),
            current: None,
            action: None,
            issued_view_tasks: Vec::new(),
            mount: MountIssuer::new(router),
            report_dispositions: Vec::new(),
            router_diagnostics: Vec::new(),
            completed_cleanup: Vec::new(),
        })
    }

    pub(crate) fn request_host_admission(
        &mut self,
        issuer: &mut HostAdmissionIssuer,
    ) -> Result<HostAdmissionToken, AdmissionError> {
        if self.state.host().is_some() {
            return Err(AdmissionError::AuthorityExists);
        }
        issuer.issue(self.router)
    }

    pub(crate) fn grant_host_admission(
        &mut self,
        issuer: &mut HostAdmissionIssuer,
        token: &HostAdmissionToken,
    ) -> Result<HostGrant, AdmissionError> {
        issuer.grant(token, &mut self.ids)
    }

    pub(crate) fn confirm_host(
        &mut self,
        issuer: &HostAdmissionIssuer,
        confirmation: &HostAdmissionConfirmation,
    ) -> Result<(), crate::presentation::AuthorityConfirmationError> {
        self.state.confirm_host(issuer, confirmation)
    }

    pub(crate) fn push(
        &mut self,
        location: RouteLocation,
    ) -> Result<NavigationRequestId, PushError> {
        let request = self.enqueue_push(location)?;
        self.collect();
        Ok(request)
    }

    pub(crate) fn enqueue_push(
        &mut self,
        location: RouteLocation,
    ) -> Result<NavigationRequestId, PushError> {
        if self.state.host().is_none() {
            return Err(PushError::AwaitingHost);
        }
        let matched = self.matcher.match_location(&location);
        let (request, outcome, reservation) = match matched {
            Ok(branch) => {
                let reserved = self
                    .ids
                    .reserve_navigation_branch(self.router, branch.routes().len())
                    .map_err(|_| PushError::IdExhausted)?;
                (
                    reserved.request(),
                    QueuedMatchOutcome::Matched(branch),
                    Some(reserved),
                )
            }
            Err(error) => {
                let request = self
                    .ids
                    .allocate_request(self.router)
                    .map_err(|_| PushError::IdExhausted)?;
                (
                    request,
                    QueuedMatchOutcome::MatchFailed(NavigationFailure::new(
                        NavigationFailureKind::Match,
                        error.redacted_message(),
                    )),
                    None,
                )
            }
        };
        self.records
            .push_back(NavigationRequestRecord::new(request, location));
        self.highest_issued = Some(request.sequence());
        self.queue.push_back(QueuedPush {
            request,
            outcome,
            reservation,
        });
        Ok(request)
    }

    pub(crate) fn collect(&mut self) {
        if self.current.is_none() {
            self.start_next();
        }
    }

    pub(crate) fn action(&mut self) -> Option<EngineAction> {
        self.action.take()
    }

    pub(crate) fn restore_action(&mut self, action: EngineAction) {
        debug_assert!(self.action.is_none());
        self.action = Some(action);
    }

    pub(crate) fn accept_applied_report(
        &mut self,
        report: AppliedFrameReport,
    ) -> Result<(), ReportError> {
        if let Err(error) = self.validate_view_report_identity(&report.identity) {
            if self.action.is_none()
                && self
                    .current
                    .as_ref()
                    .is_some_and(|current| current.stage == WorkStage::Applying)
            {
                self.reissue_current_frame_task();
            }
            return Err(error);
        }
        if self.reject_current_reconciliation_drift() {
            return Err(ReportError::Stale);
        }
        let sorted_unique = report.outlets.windows(2).all(|pair| pair[0] < pair[1]);
        let next_outlet = self.current.as_ref().and_then(|current| {
            current
                .branch
                .as_ref()
                .and_then(|branch| branch.routes().get(report.identity.branch_index + 1))
                .map(|matched| matched.slot().clone())
        });
        if !sorted_unique || (report.mode == AppliedReportMode::LegacyLeaf && next_outlet.is_some())
        {
            self.reissue_current_frame_task();
            return Err(ReportError::Invalid);
        }

        let request = report.identity.request;
        let frame = self
            .current
            .as_ref()
            .and_then(|current| current.target_frames.last())
            .expect("an issued task has one target frame")
            .clone();
        self.record_report_disposition(
            &report.identity,
            ReportDisposition::Accepted,
            "accepted applied frame report",
        );
        let record = self
            .record_mut(request)
            .expect("running record is retained");
        record
            .append_trace(NavigationTraceEvent::shell_prepared(frame.clone()))
            .expect("running trace is open");
        record
            .append_trace(NavigationTraceEvent::view_applied(frame.clone()))
            .expect("running trace is open");
        for outlet in &report.outlets {
            record
                .append_trace(NavigationTraceEvent::outlet_registered(
                    frame.clone(),
                    outlet.clone(),
                ))
                .expect("running trace is open");
        }
        self.current
            .as_mut()
            .expect("report was preflighted")
            .applied_frames
            .push(frame.clone());

        if let Some(outlet) = next_outlet {
            let host = self.state.host().expect("confirmed host").clone();
            let transaction = report.identity.transaction;
            let token = match self.mount.issue(
                &mut self.ids,
                host,
                request,
                transaction,
                frame.clone(),
                outlet.clone(),
            ) {
                Ok(token) => token,
                Err(error) => {
                    let message = match error {
                        MountError::Exhausted => "child mount request identity exhausted",
                        MountError::Foreign | MountError::Stale | MountError::WrongPhase => {
                            "child mount request issuer rejected internal identity"
                        }
                    };
                    let failure = self.begin_failure_cleanup(
                        PendingFailureKind::MountIssue,
                        NavigationFailure::new(NavigationFailureKind::Mount, message),
                    );
                    self.action = Some(EngineAction::FailureCleanup(failure));
                    return Ok(());
                }
            };
            let child_index = report.identity.branch_index + 1;
            let current = self.current.as_mut().expect("report was preflighted");
            current.identity = report.identity;
            current.pending_mount = Some(PendingMount {
                child_index,
                parent: frame.clone(),
                outlet: outlet.clone(),
            });
            current.stage = WorkStage::AwaitingMount;
            self.record_mut(request)
                .expect("running record is retained")
                .append_trace(NavigationTraceEvent::mount_requested(
                    frame.clone(),
                    outlet.clone(),
                ))
                .expect("running trace is open");
            self.action = Some(EngineAction::MountRequest(FrameMountRequest {
                token,
                request,
                parent: frame,
                outlet,
            }));
        } else {
            let current = self.current.as_mut().expect("report was preflighted");
            current.identity = report.identity.clone();
            current.stage = WorkStage::ReadyToCommit;
            self.transition_with_trace(request, NavigationPhase::ReadyToSwitch);
            self.action = Some(EngineAction::Commit(CommitTicket {
                identity: report.identity,
            }));
        }
        Ok(())
    }

    pub(crate) fn resolve_mount(
        &mut self,
        request: FrameMountRequest,
    ) -> Result<MountGrant, MountCapabilityError<ResolveMountOwner>> {
        if let Err(kind) = self.mount.validate_request_token(&request.token) {
            return Err(MountCapabilityError::new(
                kind,
                ResolveMountOwner::Returned(request),
            ));
        }
        if !self.has_live_mount_context(
            &WorkStage::AwaitingMount,
            Some((request.request, &request.parent, &request.outlet)),
        ) {
            return Err(MountCapabilityError::new(
                MountError::WrongPhase,
                ResolveMountOwner::Returned(request),
            ));
        }
        let grant = match self.mount.grant(&request.token, &mut self.ids) {
            Ok(grant) => grant,
            Err(MountError::Exhausted) => {
                self.action = Some(EngineAction::MountRequest(request));
                return Err(MountCapabilityError::new(
                    MountError::Exhausted,
                    ResolveMountOwner::Reissued,
                ));
            }
            Err(kind) => {
                return Err(MountCapabilityError::new(
                    kind,
                    ResolveMountOwner::Returned(request),
                ));
            }
        };
        let slot = grant.slot();
        let current = self.current.as_mut().expect("mount was preflighted");
        current.stage = WorkStage::AwaitingReceipt;
        let record = self
            .record_mut(request.request)
            .expect("mount request is retained");
        record
            .append_trace(NavigationTraceEvent::mount_resolved(
                request.parent.clone(),
                request.outlet.clone(),
            ))
            .expect("running trace is open");
        record
            .append_trace(NavigationTraceEvent::mount_granted(
                request.parent,
                request.outlet,
                slot,
            ))
            .expect("running trace is open");
        Ok(grant)
    }

    pub(crate) fn accept_mount_receipt(
        &mut self,
        receipt: MountReceiptAttestation,
    ) -> Result<MountRegistrationPermit, MountCapabilityError<MountReceiptAttestation>> {
        if let Err(kind) = self.mount.validate_receipt(&receipt) {
            return Err(MountCapabilityError::new(kind, receipt));
        }
        if !self.has_live_mount_context(&WorkStage::AwaitingReceipt, None) {
            return Err(MountCapabilityError::new(MountError::WrongPhase, receipt));
        }
        let permit = match self.mount.accept_receipt(&receipt) {
            Ok(permit) => permit,
            Err(kind) => return Err(MountCapabilityError::new(kind, receipt)),
        };
        self.current
            .as_mut()
            .expect("receipt was preflighted")
            .stage = WorkStage::ConfirmingMount;
        Ok(permit)
    }

    pub(crate) fn complete_mount_registration(
        &mut self,
        completion: MountRegistrationCompletion,
    ) -> Result<(), MountError> {
        self.complete_mount_registration_with_replay(completion)
            .map(drop)
    }

    fn complete_mount_registration_with_replay(
        &mut self,
        completion: MountRegistrationCompletion,
    ) -> Result<MountConfirmation, MountError> {
        self.mount.validate_completion(&completion)?;
        if !self.current.as_ref().is_some_and(|current| {
            current.stage == WorkStage::ConfirmingMount && current.pending_mount.is_some()
        }) {
            return Err(MountError::WrongPhase);
        }
        if !self.current_reconciliation_matches_active() {
            let confirmation = self.mount.complete_registration(&completion)?;
            self.mount.consume_confirmation(&confirmation)?;
            let rejected = self.reject_current_reconciliation_drift();
            debug_assert!(rejected);
            return Err(MountError::WrongPhase);
        }
        let publication = self.prepare_confirmed_child(completion.slot())?;
        let confirmation = self.mount.complete_registration(&completion)?;
        let replay = confirmation.clone();
        self.consume_prepared_mount_confirmation(confirmation, publication)?;
        Ok(replay)
    }

    pub(crate) fn consume_mount_confirmation(
        &mut self,
        confirmation: MountConfirmation,
    ) -> Result<(), MountError> {
        self.mount.validate_confirmation(&confirmation)?;
        if !self.current.as_ref().is_some_and(|current| {
            current.stage == WorkStage::ConfirmingMount && current.pending_mount.is_some()
        }) {
            return Err(MountError::WrongPhase);
        }
        if !self.current_reconciliation_matches_active() {
            self.mount.consume_confirmation(&confirmation)?;
            let rejected = self.reject_current_reconciliation_drift();
            debug_assert!(rejected);
            return Err(MountError::WrongPhase);
        }
        let publication = self.prepare_confirmed_child(confirmation.slot())?;
        self.consume_prepared_mount_confirmation(confirmation, publication)
    }

    fn consume_prepared_mount_confirmation(
        &mut self,
        confirmation: MountConfirmation,
        publication: ConfirmedChildPublication,
    ) -> Result<(), MountError> {
        self.mount.consume_confirmation(&confirmation)?;
        self.apply_confirmed_child(publication);
        Ok(())
    }

    pub(crate) fn validate_mount_registration(
        &self,
        permit: &MountRegistrationPermit,
        proof: &MountReceiptProof,
    ) -> Result<RouteSlotId, MountError> {
        self.mount.validate_registration_permit(permit, proof)?;
        if !self.has_live_mount_context(&WorkStage::ConfirmingMount, None) {
            return Err(MountError::WrongPhase);
        }
        Ok(permit.slot())
    }

    pub(crate) fn validate_mount_registration_rejection(
        &self,
        permit: &MountRegistrationPermit,
    ) -> Result<(), MountError> {
        self.mount.validate_mutation_artifact(permit)?;
        if !self.has_live_mount_context(&WorkStage::ConfirmingMount, None) {
            return Err(MountError::WrongPhase);
        }
        Ok(())
    }

    pub(crate) fn complete_preflighted_mount_registration(
        &mut self,
        permit: MountRegistrationPermit,
        registration_digest: MountRegistrationDigest,
    ) {
        let completion = permit.complete(registration_digest);
        self.complete_mount_registration(completion)
            .expect("an exclusively borrowed preflighted registration remains valid")
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn testing_complete_preflighted_mount_registration_with_replays(
        &mut self,
        permit: MountRegistrationPermit,
        registration_digest: MountRegistrationDigest,
    ) -> Result<(MountRegistrationCompletion, MountConfirmation), MountError> {
        let completion = permit.complete(registration_digest);
        let replay = completion.clone();
        let confirmation = self.complete_mount_registration_with_replay(completion)?;
        Ok((replay, confirmation))
    }

    pub(crate) fn reject_preflighted_mount_registration(
        &mut self,
        permit: MountRegistrationPermit,
    ) -> FailureCleanupRequest {
        self.reject_mount_after_mutation(MountMutationArtifact::RegistrationPermit(permit))
            .expect("an exclusively borrowed preflighted registration remains valid")
    }

    pub(crate) fn reject_mount(
        &mut self,
        request: FrameMountRequest,
        rejection: MountRejectionKind,
    ) -> Result<FailureCleanupRequest, MountCapabilityError<FrameMountRequest>> {
        if let Err(kind) = self.mount.validate_request_token(&request.token) {
            return Err(MountCapabilityError::new(kind, request));
        }
        if !self.has_live_mount_context(
            &WorkStage::AwaitingMount,
            Some((request.request, &request.parent, &request.outlet)),
        ) {
            return Err(MountCapabilityError::new(MountError::WrongPhase, request));
        }
        if !matches!(
            rejection,
            MountRejectionKind::MissingOutlet
                | MountRejectionKind::InvalidOutlet
                | MountRejectionKind::ParentAuthorityLost
        ) {
            return Err(MountCapabilityError::new(MountError::WrongPhase, request));
        }
        if let Err(kind) = self.mount.reject(&request.token) {
            return Err(MountCapabilityError::new(kind, request));
        }
        self.record_mut(request.request)
            .expect("mount rejection request is retained")
            .append_trace(NavigationTraceEvent::mount_rejected(
                request.parent,
                request.outlet,
                rejection,
            ))
            .expect("running trace is open");
        let (pending, failure) = mount_failure(rejection);
        Ok(self.begin_failure_cleanup(pending, failure))
    }

    pub(crate) fn reject_mount_after_mutation(
        &mut self,
        artifact: MountMutationArtifact,
    ) -> Result<FailureCleanupRequest, MountCapabilityError<MountMutationArtifact>> {
        let expected_stage = match &artifact {
            MountMutationArtifact::Grant(_) | MountMutationArtifact::Receipt(_) => {
                WorkStage::AwaitingReceipt
            }
            MountMutationArtifact::RegistrationPermit(_) => WorkStage::ConfirmingMount,
        };
        let validation = match &artifact {
            MountMutationArtifact::Grant(grant) => self.mount.validate_mutation_artifact(grant),
            MountMutationArtifact::Receipt(receipt) => {
                self.mount.validate_mutation_artifact(receipt)
            }
            MountMutationArtifact::RegistrationPermit(permit) => {
                self.mount.validate_mutation_artifact(permit)
            }
        };
        if let Err(kind) = validation {
            return Err(MountCapabilityError::new(kind, artifact));
        }
        if !self.has_live_mount_context(&expected_stage, None) {
            return Err(MountCapabilityError::new(MountError::WrongPhase, artifact));
        }
        let (request, parent, outlet) = self
            .current
            .as_ref()
            .and_then(|current| {
                current.pending_mount.as_ref().map(|pending| {
                    (
                        current.identity.request,
                        pending.parent.clone(),
                        pending.outlet.clone(),
                    )
                })
            })
            .expect("live mount context retains pending facts");
        let slot = match &artifact {
            MountMutationArtifact::Grant(grant) => grant.slot(),
            MountMutationArtifact::Receipt(receipt) => receipt.slot(),
            MountMutationArtifact::RegistrationPermit(permit) => permit.slot(),
        };
        let rejection = match &artifact {
            MountMutationArtifact::Grant(grant) => self.mount.reject_after_mutation(grant),
            MountMutationArtifact::Receipt(receipt) => self.mount.reject_after_mutation(receipt),
            MountMutationArtifact::RegistrationPermit(permit) => {
                self.mount.reject_after_mutation(permit)
            }
        };
        if let Err(kind) = rejection {
            return Err(MountCapabilityError::new(kind, artifact));
        }
        self.record_mut(request)
            .expect("mount rejection request is retained")
            .append_trace(NavigationTraceEvent::mount_rejected_after_mutation(
                parent, outlet, slot,
            ))
            .expect("running trace is open");
        Ok(self.begin_failure_cleanup(
            PendingFailureKind::Mount(MountRejectionKind::RejectedAfterMutation),
            NavigationFailure::new(
                NavigationFailureKind::Mount,
                "child mount rejected after mutation",
            ),
        ))
    }

    fn has_live_mount_context(
        &self,
        expected_stage: &WorkStage,
        expected_request: Option<(NavigationRequestId, &RouteFrameVersion, &RouteSlotKey)>,
    ) -> bool {
        let Some(current) = self.current.as_ref() else {
            return false;
        };
        let Some(pending) = current.pending_mount.as_ref() else {
            return false;
        };
        let Some(host) = self.state.host() else {
            return false;
        };
        if &current.stage != expected_stage
            || current.identity.router != self.router
            || current.identity.host != host.host()
            || current.identity.host_epoch != host.epoch()
        {
            return false;
        }
        let Some(child_index) = current.identity.branch_index.checked_add(1) else {
            return false;
        };
        if pending.child_index != child_index {
            return false;
        }
        let Some(current_reservation) = current.reservation.get(current.identity.branch_index)
        else {
            return false;
        };
        if current_reservation.frame_task() != current.identity.frame_task
            || current_reservation.frame() != current.identity.frame
        {
            return false;
        }
        let current_parent = RouteFrameVersion::new(
            current.identity.router,
            current.identity.host,
            current.identity.slot,
            current.identity.origin.clone(),
            current.identity.frame,
            current.identity.generation,
        );
        if current_parent != pending.parent
            || current.target_frames.len() != child_index
            || current.target_frames.last() != Some(&pending.parent)
            || current.applied_frames != current.target_frames
        {
            return false;
        }
        let Some(branch) = current.branch.as_ref() else {
            return false;
        };
        let Some(current_route) = branch.routes().get(current.identity.branch_index) else {
            return false;
        };
        let Some(child_route) = branch.routes().get(child_index) else {
            return false;
        };
        if current.identity.origin != FrameOrigin::MatchedRoute(current_route.route())
            || child_route.slot() != &pending.outlet
            || current.reservation.get(child_index).is_none()
        {
            return false;
        }
        let current_target_is_exact = match &current.current_target {
            FrameMountTarget::TopSlot {
                host: target_host,
                slot,
            } => {
                current.identity.branch_index == 0
                    && target_host == host
                    && *slot == current.identity.slot
                    && self.state.top_slot() == Some(*slot)
            }
            FrameMountTarget::ChildOutlet { parent, outlet } => {
                current.identity.branch_index > 0
                    && current
                        .target_frames
                        .get(current.identity.branch_index - 1)
                        .is_some_and(|expected_parent| expected_parent == parent)
                    && current_route.slot() == outlet
            }
        };
        if !current_target_is_exact {
            return false;
        }
        if let Some((request, parent, outlet)) = expected_request {
            if current.identity.request != request
                || pending.parent != *parent
                || pending.outlet != *outlet
            {
                return false;
            }
        }
        self.mount.matches_outstanding_context(
            host,
            current.identity.request,
            current.identity.transaction,
            &pending.parent,
            &pending.outlet,
        )
    }

    #[cfg(test)]
    pub(crate) fn prepare_failure_cleanup(
        &mut self,
        request: FailureCleanupRequest,
        seal: CleanupManifestSeal,
    ) -> Result<CleanupBatchToken, FailureCleanupError> {
        self.prepare_failure_cleanup_retaining(request, seal)
            .map_err(|error| error.kind())
    }

    pub(crate) fn prepare_failure_cleanup_retaining(
        &mut self,
        request: FailureCleanupRequest,
        seal: CleanupManifestSeal,
    ) -> Result<CleanupBatchToken, FailureCleanupPrepareError> {
        if request.identity.router_identity() != self.router {
            return Err(FailureCleanupPrepareError::new(
                FailureCleanupError::Foreign,
                request,
            ));
        }
        let Some(current) = self.current.as_ref() else {
            return Err(FailureCleanupPrepareError::new(
                FailureCleanupError::Stale,
                request,
            ));
        };
        let valid_request = matches!(
            &current.stage,
            WorkStage::FailureCleanupManifestPending { expected } if expected == &request
        );
        if !valid_request {
            return Err(FailureCleanupPrepareError::new(
                FailureCleanupError::Stale,
                request,
            ));
        }
        if request.logical_roots != seal.logical_roots()
            || (!request.logical_roots.is_empty() && seal.obligation_count() == 0)
        {
            return Err(FailureCleanupPrepareError::new(
                FailureCleanupError::InvalidManifest,
                request,
            ));
        }

        let cleanup = CleanupBatchToken::sealed(
            request.identity.clone(),
            current.cleanup_batch,
            &request.logical_roots,
            seal.manifest_digest(),
            seal.obligation_count(),
        );
        self.current
            .as_mut()
            .expect("failure cleanup request was preflighted")
            .stage = WorkStage::FailureCleanupPrepared {
            outstanding: cleanup.clone(),
            failure: request.failure,
            continuation: request.continuation,
        };
        Ok(cleanup)
    }

    pub(crate) fn begin_view_report<'a>(
        &'a mut self,
        identity: &RootTaskIdentity,
    ) -> Result<ViewReportSession<'a>, ReportError> {
        self.validate_view_report_identity(identity)?;
        Ok(ViewReportSession {
            engine: self,
            identity: identity.clone(),
        })
    }

    pub(crate) fn validate_view_report_identity(
        &mut self,
        identity: &RootTaskIdentity,
    ) -> Result<(), ReportError> {
        if self.inspect_view_report_identity(identity).is_ok() {
            return Ok(());
        }

        let known = self
            .issued_view_tasks
            .iter()
            .any(|issued| issued == identity);
        let latest_for_request = self
            .issued_view_tasks
            .iter()
            .rev()
            .find(|issued| issued.request == identity.request);
        let (disposition, error, message) =
            if known && latest_for_request.is_some_and(|latest| latest == identity) {
                (
                    ReportDisposition::DuplicateRejected,
                    ReportError::Duplicate,
                    "duplicate view report rejected",
                )
            } else if known {
                (
                    ReportDisposition::StaleDiscarded,
                    ReportError::Stale,
                    "stale view report discarded",
                )
            } else {
                (
                    ReportDisposition::InvalidRejected,
                    ReportError::Invalid,
                    "invalid view report rejected",
                )
            };
        self.record_report_disposition(identity, disposition, message);
        Err(error)
    }

    pub(crate) fn inspect_view_report_identity(
        &self,
        identity: &RootTaskIdentity,
    ) -> Result<(), ReportError> {
        if self.current.as_ref().is_some_and(|current| {
            current.stage == WorkStage::Applying && current.identity == *identity
        }) {
            return Ok(());
        }
        let known = self
            .issued_view_tasks
            .iter()
            .any(|issued| issued == identity);
        let latest_for_request = self
            .issued_view_tasks
            .iter()
            .rev()
            .find(|issued| issued.request == identity.request);
        if known && latest_for_request.is_some_and(|latest| latest == identity) {
            Err(ReportError::Duplicate)
        } else if known {
            Err(ReportError::Stale)
        } else {
            Err(ReportError::Invalid)
        }
    }

    pub(crate) fn expected_child_outlet(
        &self,
        identity: &RootTaskIdentity,
    ) -> Result<Option<RouteSlotKey>, ReportError> {
        let Some(current) = self.current.as_ref() else {
            return Err(ReportError::Stale);
        };
        if current.stage == WorkStage::Applying && current.identity == *identity {
            return Ok(current
                .branch
                .as_ref()
                .and_then(|branch| branch.routes().get(identity.branch_index + 1))
                .map(|matched| matched.slot().clone()));
        }
        let known = self
            .issued_view_tasks
            .iter()
            .any(|issued| issued == identity);
        let latest_for_request = self
            .issued_view_tasks
            .iter()
            .rev()
            .find(|issued| issued.request == identity.request);
        if known && latest_for_request.is_some_and(|latest| latest == identity) {
            Err(ReportError::Duplicate)
        } else if known {
            Err(ReportError::Stale)
        } else {
            Err(ReportError::Invalid)
        }
    }

    pub(crate) fn validate_mount_request(
        &self,
        request: &FrameMountRequest,
    ) -> Result<(), MountError> {
        self.mount.validate_request_token(&request.token)?;
        if self.has_live_mount_context(
            &WorkStage::AwaitingMount,
            Some((request.request, &request.parent, &request.outlet)),
        ) {
            Ok(())
        } else {
            Err(MountError::WrongPhase)
        }
    }

    pub(crate) fn seal_commit(
        &mut self,
        ticket: &CommitTicket,
        seal: CleanupManifestSeal,
    ) -> Result<SealedCommit, CommitError> {
        let Some(current) = self.current.as_ref() else {
            return Err(CommitError::Stale);
        };
        if current.stage != WorkStage::ReadyToCommit || current.identity != ticket.identity {
            return Err(CommitError::Stale);
        }
        if self.reject_current_reconciliation_drift() {
            return Err(CommitError::Stale);
        }

        let current = self
            .current
            .as_ref()
            .expect("commit identity was validated");
        if current.cleanup_roots != seal.logical_roots()
            || current.cleanup_roots.is_empty() != (seal.obligation_count() == 0)
        {
            return Err(CommitError::InvalidManifest);
        }

        let cleanup = CleanupBatchToken::sealed(
            current.identity.clone(),
            current.cleanup_batch,
            &current.cleanup_roots,
            seal.manifest_digest(),
            seal.obligation_count(),
        );
        let publication = self.precompute_publication(cleanup.clone());
        self.current
            .as_mut()
            .expect("current work was validated")
            .stage = WorkStage::CommitSealed {
            token: cleanup.clone(),
        };
        Ok(SealedCommit {
            identity: ticket.identity.clone(),
            cleanup,
            publication,
        })
    }

    pub(crate) fn prepare_publication(
        &mut self,
        sealed: SealedCommit,
    ) -> Result<PreparedPublication, CommitError> {
        let valid = self.current.as_ref().is_some_and(|current| {
            current.identity == sealed.identity
                && matches!(
                    &current.stage,
                    WorkStage::CommitSealed { token } if token == &sealed.cleanup
                )
        });
        if !valid {
            return Err(CommitError::Stale);
        }
        let Some(record_slot) = self.records.get(sealed.publication.record_index) else {
            return Err(CommitError::Stale);
        };
        if record_slot.id() != sealed.identity.request {
            return Err(CommitError::Stale);
        }
        Ok(PreparedPublication { sealed })
    }

    pub(crate) fn publish_prepared<F>(&mut self, prepared: PreparedPublication, mutation: F)
    where
        F: FnOnce(),
    {
        let SealedCommit {
            identity: _,
            cleanup: _,
            publication,
        } = prepared.sealed;
        let PublicationPlan {
            state,
            history,
            active_branch,
            active_child_slots,
            record_index,
            published_record,
            current,
        } = publication;
        mutation();
        self.records[record_index] = published_record;
        self.state = state;
        self.history = history;
        self.active_branch = active_branch;
        self.active_child_slots = active_child_slots;
        self.current = Some(current);
    }

    pub(crate) fn complete_cleanup(
        &mut self,
        completion: CleanupCompletion,
    ) -> Result<(), CleanupError> {
        let outstanding = self
            .current
            .as_ref()
            .and_then(|current| match &current.stage {
                WorkStage::PostCommitCleanup { outstanding } => Some((outstanding.clone(), None)),
                WorkStage::FailureCleanupPrepared {
                    outstanding,
                    failure,
                    continuation,
                } => Some((outstanding.clone(), Some((failure.clone(), *continuation)))),
                WorkStage::Applying
                | WorkStage::AwaitingMount
                | WorkStage::AwaitingReceipt
                | WorkStage::ConfirmingMount
                | WorkStage::ReadyToCommit
                | WorkStage::CommitSealed { .. }
                | WorkStage::FailureCleanupManifestPending { .. } => None,
            });
        if !outstanding
            .as_ref()
            .is_some_and(|(expected, _)| completion.matches(expected))
        {
            if let Some(tombstone) = self
                .completed_cleanup
                .iter()
                .find(|tombstone| completion.token() == &tombstone.token)
            {
                return if completion.matches(&tombstone.token) {
                    Err(CleanupError::Duplicate)
                } else {
                    Err(CleanupError::Invalid)
                };
            }
            return if completion.is_well_formed() {
                Err(CleanupError::Stale)
            } else {
                Err(CleanupError::Invalid)
            };
        }

        let current = self.current.as_ref().expect("validated current work");
        let request = current.identity.request;
        let (completed_token, failure) =
            outstanding.expect("validated cleanup has an outstanding token");
        if let Some((failure, FailureCleanupContinuation::BeginFallback)) = &failure {
            let failure = failure.clone();
            self.record_mut(request)
                .expect("running record is retained")
                .append_trace(NavigationTraceEvent::cleanup_settled(None))
                .expect("cleanup trace is open");
            self.completed_cleanup.push(CompletedCleanupTombstone {
                request,
                token: completed_token,
            });
            self.begin_fallback(request, failure);
            return Ok(());
        }
        let terminal_status = match failure {
            Some((failure, FailureCleanupContinuation::Terminal)) => RequestStatus::Failed(failure),
            Some((_, FailureCleanupContinuation::BeginFallback)) => unreachable!(),
            None => match &current.attempt {
                AttemptKind::Normal => RequestStatus::Succeeded,
                AttemptKind::Fallback { original_failure } => {
                    RequestStatus::Failed(original_failure.clone())
                }
            },
        };
        let record = self
            .record_mut(request)
            .expect("running record is retained");
        record
            .append_trace(NavigationTraceEvent::cleanup_settled(None))
            .expect("terminal trace is open");
        record
            .transition(terminal_status)
            .expect("request is running");
        record
            .append_trace(NavigationTraceEvent::request_terminal())
            .expect("terminal status precedes terminal event");
        self.completed_cleanup.push(CompletedCleanupTombstone {
            request,
            token: completed_token,
        });
        self.current = None;
        self.state.set_request(None);
        self.retain_terminal_records();
        self.start_next();
        Ok(())
    }

    pub(crate) fn observe_cleanup_receipt(
        &mut self,
        batch: &CleanupBatchToken,
        disposition: crate::trace::CleanupDisposition,
    ) -> Result<(), CleanupError> {
        let request = self
            .current
            .as_ref()
            .and_then(|current| match &current.stage {
                WorkStage::PostCommitCleanup { outstanding } if outstanding == batch => {
                    Some(current.identity.request)
                }
                WorkStage::FailureCleanupPrepared { outstanding, .. } if outstanding == batch => {
                    Some(current.identity.request)
                }
                WorkStage::Applying
                | WorkStage::AwaitingMount
                | WorkStage::AwaitingReceipt
                | WorkStage::ConfirmingMount
                | WorkStage::ReadyToCommit
                | WorkStage::CommitSealed { .. }
                | WorkStage::PostCommitCleanup { .. }
                | WorkStage::FailureCleanupManifestPending { .. }
                | WorkStage::FailureCleanupPrepared { .. } => None,
            })
            .ok_or(CleanupError::Stale)?;
        self.record_mut(request)
            .expect("current cleanup request is retained")
            .append_trace(NavigationTraceEvent::cleanup_settled(Some(disposition)))
            .expect("current cleanup trace is open");
        Ok(())
    }

    pub(crate) const fn active(&self) -> &ActivePresentation {
        self.state.active()
    }

    pub(crate) const fn availability(&self) -> &crate::presentation::RouterAvailability {
        self.state.availability()
    }

    pub(crate) fn running_request(&self) -> Option<NavigationRequestId> {
        self.current
            .as_ref()
            .map(|current| current.identity.request)
    }

    pub(crate) fn requested(&self) -> Option<&RouteLocation> {
        self.running_request()
            .and_then(|request| self.request(request))
            .map(NavigationRequestRecord::requested)
    }

    pub(crate) fn reconciliation_projection_for(
        &self,
        identity: &RootTaskIdentity,
    ) -> Option<&RouteReconciliationProjection> {
        self.current.as_ref().and_then(|current| {
            let projection = current.reconciliation.as_ref()?;
            (current.identity == *identity
                && current.branch.as_ref().is_some_and(|branch| {
                    branch.branch_fingerprint() == projection.target_fingerprint()
                })
                && self.current_reconciliation_matches_active())
            .then_some(projection)
        })
    }

    pub(crate) fn matched(&self) -> Option<&crate::fingerprint::RouteBranchFingerprint> {
        self.running_request()
            .and_then(|request| self.request(request))
            .and_then(NavigationRequestRecord::matched)
    }

    pub(crate) fn pending_phase(&self) -> Option<&NavigationPhase> {
        match self.state.current_request() {
            Some(RequestStatus::Running(phase)) => Some(phase),
            Some(RequestStatus::Queued | RequestStatus::Succeeded | RequestStatus::Failed(_))
            | None => None,
        }
    }

    pub(crate) const fn history(&self) -> &MemoryHistory {
        &self.history
    }

    pub(crate) fn history_snapshot(&self) -> crate::history::HistorySnapshot {
        crate::history::HistorySnapshot::project(&self.history)
    }

    pub(crate) fn earliest_retained_sequence(&self) -> Option<u64> {
        self.records
            .iter()
            .filter(|record| is_terminal(record.status()))
            .map(|record| record.id().sequence())
            .min()
    }

    pub(crate) const fn request_retention_facts(
        &self,
    ) -> (RouterIdentity, Option<u64>, Option<u64>) {
        (self.router, self.evicted_through, self.highest_issued)
    }

    pub(crate) fn router_diagnostics(&self) -> &[DiagnosticEvent] {
        &self.router_diagnostics
    }

    pub(crate) fn diagnostic_snapshot(&self) -> crate::diagnostics::DiagnosticSnapshot {
        crate::diagnostics::DiagnosticSnapshot::project(self.router_diagnostics.clone())
    }

    pub(crate) fn host(&self) -> Option<&crate::HostVersion> {
        self.state.host()
    }

    pub(crate) fn top_slot(&self) -> Option<RouteSlotId> {
        self.state.top_slot()
    }

    pub(crate) fn request(&self, id: NavigationRequestId) -> Option<&NavigationRequestRecord> {
        self.records.iter().find(|record| record.id() == id)
    }

    pub(crate) fn private_lookup(&self, id: NavigationRequestId) -> PrivateRequestLookup<'_> {
        if let Some(record) = self.request(id) {
            PrivateRequestLookup::Retained(record)
        } else if id.router_identity() == self.router
            && self
                .evicted_through
                .is_some_and(|sequence| id.sequence() <= sequence)
        {
            PrivateRequestLookup::Evicted
        } else {
            PrivateRequestLookup::Unknown
        }
    }

    #[cfg(test)]
    fn current_status(&self) -> Option<&RequestStatus> {
        self.state.current_request()
    }

    fn start_next(&mut self) {
        let Some(queued) = self.queue.pop_front() else {
            self.state.set_request(None);
            return;
        };
        let request = queued.request;
        let location = self
            .request(request)
            .expect("queued record is retained")
            .requested()
            .clone();
        let branch = match queued.outcome {
            QueuedMatchOutcome::Matched(branch) => branch,
            QueuedMatchOutcome::MatchFailed(failure) => {
                self.record_mut(request)
                    .expect("running record is retained")
                    .push_diagnostic(DiagnosticEvent::match_failed(failure.message()))
                    .expect("running diagnostics are open");
                self.begin_fallback(request, failure);
                return;
            }
        };
        let reservation = queued
            .reservation
            .expect("a stored matched branch has one atomic reservation");
        self.record_mut(request)
            .expect("running record is retained")
            .set_matched(branch.branch_fingerprint().clone())
            .expect("match is written once");
        let active_location = self.state.active().location();
        let active_frames = self.state.active().frames();
        let equivalent = self.state.active().kind() == ActivePresentationKind::Route
            && active_location == Some(&location)
            && active_frames.len() == branch.routes().len();
        let fragment_only = !equivalent
            && branch.routes().len() == 1
            && active_location.is_some_and(|active| {
                active.path() == location.path()
                    && active.search() == location.search()
                    && active.fragment() != location.fragment()
            })
            && self
                .current_active_branch()
                .is_some_and(|active| active.branch_fingerprint() == branch.branch_fingerprint());
        let reconciliation_fragment = !equivalent
            && active_location.is_some_and(|active| {
                active.path() == location.path()
                    && active.search() == location.search()
                    && active.fragment() != location.fragment()
            })
            && self
                .current_active_branch()
                .is_some_and(|active| active.branch_fingerprint() == branch.branch_fingerprint());

        let host = self
            .state
            .host()
            .expect("host was checked before push")
            .clone();
        let top_slot = self.state.top_slot().expect("confirmed host has top slot");
        let reserved_frames = reservation.frames().to_vec();
        let reserved_frame_ids = reserved_frames
            .iter()
            .map(|reserved| reserved.frame())
            .collect::<Vec<_>>();
        let reconciliation = match RouteReconciliationProjection::from_running(
            self.router,
            host.host(),
            host.epoch(),
            top_slot,
            active_location,
            self.current_active_branch(),
            active_frames,
            &self.active_child_slots,
            &branch,
            &reserved_frame_ids,
            reconciliation_fragment,
        ) {
            Ok(projection) => projection,
            Err(ReconciliationProjectionError::InvalidActiveBaseline) => {
                self.fail_reserved_request(
                    request,
                    NavigationFailure::new(
                        NavigationFailureKind::AuthorityLost,
                        "active branch failed reconciliation projection",
                    ),
                );
                self.start_next();
                return;
            }
            Err(ReconciliationProjectionError::GenerationExhausted) => {
                self.fail_reserved_request(
                    request,
                    NavigationFailure::new(
                        NavigationFailureKind::AuthorityLost,
                        "route generation exhausted",
                    ),
                );
                self.start_next();
                return;
            }
        };

        if equivalent {
            let leaf_index = branch.routes().len() - 1;
            let active = active_frames
                .get(leaf_index)
                .expect("equivalent route has one active frame per match");
            let leaf = &branch.routes()[leaf_index];
            let identity = FrameTaskIdentity {
                router: self.router,
                host: host.host(),
                host_epoch: host.epoch(),
                slot: active.slot(),
                request,
                transaction: reservation.transaction(),
                frame_task: reserved_frames[leaf_index].frame_task(),
                frame: active.frame(),
                generation: active.generation(),
                origin: FrameOrigin::MatchedRoute(leaf.route()),
                branch_index: leaf_index,
            };
            self.current = Some(CurrentWork {
                identity: identity.clone(),
                branch: Some(branch),
                location: location.clone(),
                reservation: reserved_frames,
                target_frames: active_frames.to_vec(),
                applied_frames: Vec::new(),
                current_target: FrameMountTarget::TopSlot {
                    host,
                    slot: top_slot,
                },
                pending_mount: None,
                history_intent: Some(HistoryIntent::Push(location)),
                stage: WorkStage::ReadyToCommit,
                cleanup_batch: reservation.cleanup_batch(),
                cleanup_roots: Vec::new(),
                attempt: AttemptKind::Normal,
                reconciliation: Some(reconciliation),
            });
            self.transition_with_trace(request, NavigationPhase::ReadyToSwitch);
            self.action = Some(EngineAction::Commit(CommitTicket { identity }));
            return;
        }

        let (frame, generation) = if fragment_only {
            let active = active_frames
                .first()
                .expect("fragment remount has an active root");
            (
                active.frame(),
                match active.generation().checked_next() {
                    Ok(generation) => generation,
                    Err(_) => {
                        self.fail_reserved_request(
                            request,
                            NavigationFailure::new(
                                NavigationFailureKind::AuthorityLost,
                                "route generation exhausted",
                            ),
                        );
                        self.start_next();
                        return;
                    }
                },
            )
        } else {
            (reserved_frames[0].frame(), RouteGeneration::initial())
        };

        let matched = branch.routes()[0].clone();
        let origin = FrameOrigin::MatchedRoute(matched.route());
        let view = self
            .matcher
            .view_binding(matched.route())
            .expect("each scheduled route has a view binding")
            .clone();
        let input = RouteViewInput::new(
            location.clone(),
            Some(matched.route()),
            matched.params().clone(),
            origin.clone(),
            None,
        );
        let identity = FrameTaskIdentity {
            router: self.router,
            host: host.host(),
            host_epoch: host.epoch(),
            slot: top_slot,
            request,
            transaction: reservation.transaction(),
            frame_task: reserved_frames[0].frame_task(),
            frame,
            generation,
            origin: origin.clone(),
            branch_index: 0,
        };
        let target_frame = RouteFrameVersion::new(
            self.router,
            host.host(),
            top_slot,
            origin,
            frame,
            generation,
        );
        let cleanup_roots = active_frames.iter().rev().cloned().collect();
        let target = FrameMountTarget::TopSlot {
            host: host.clone(),
            slot: top_slot,
        };
        self.current = Some(CurrentWork {
            identity: identity.clone(),
            branch: Some(branch),
            location: location.clone(),
            reservation: reserved_frames,
            target_frames: vec![target_frame.clone()],
            applied_frames: Vec::new(),
            current_target: target.clone(),
            pending_mount: None,
            history_intent: Some(HistoryIntent::Push(location.clone())),
            stage: WorkStage::Applying,
            cleanup_batch: reservation.cleanup_batch(),
            cleanup_roots,
            attempt: AttemptKind::Normal,
            reconciliation: Some(reconciliation),
        });
        let record = self
            .record_mut(request)
            .expect("running record is retained");
        record
            .append_trace(NavigationTraceEvent::frame_task_issued(
                target_frame.clone(),
            ))
            .expect("running trace is open");
        record
            .transition(RequestStatus::Running(NavigationPhase::ApplyingFrame(0)))
            .expect("request is running");
        record
            .append_trace(NavigationTraceEvent::request_phase_changed(
                NavigationPhase::ApplyingFrame(0),
            ))
            .expect("running trace is open");
        self.state.set_request(Some(RequestStatus::Running(
            NavigationPhase::ApplyingFrame(0),
        )));
        self.issued_view_tasks.push(identity.clone());
        self.action = Some(EngineAction::FrameTask(FrameTask {
            identity,
            frame: target_frame,
            target,
            location,
            matched: Some(matched),
            view,
            input: Some(input),
        }));
    }

    fn begin_failure_cleanup(
        &mut self,
        expected: PendingFailureKind,
        failure: NavigationFailure,
    ) -> FailureCleanupRequest {
        self.begin_failure_cleanup_with_continuation(
            expected,
            failure,
            FailureCleanupContinuation::Terminal,
        )
    }

    fn begin_failure_cleanup_with_continuation(
        &mut self,
        expected: PendingFailureKind,
        failure: NavigationFailure,
        continuation: FailureCleanupContinuation,
    ) -> FailureCleanupRequest {
        let current = self
            .current
            .as_ref()
            .expect("a nested failure belongs to current work");
        let request = FailureCleanupRequest {
            identity: current.identity.clone(),
            expected,
            logical_roots: current.applied_frames.iter().rev().cloned().collect(),
            failure,
            continuation,
        };
        let phase = if request.expected == PendingFailureKind::AuthorityLost {
            NavigationPhase::Recovering
        } else {
            NavigationPhase::CleanupPending
        };
        let current = self
            .current
            .as_mut()
            .expect("a nested failure belongs to current work");
        current.pending_mount = None;
        current.stage = WorkStage::FailureCleanupManifestPending {
            expected: request.clone(),
        };
        self.action = None;
        self.transition_with_trace(request.identity.request, phase);
        request
    }

    fn begin_fallback(
        &mut self,
        request: NavigationRequestId,
        original_failure: NavigationFailure,
    ) {
        let matched = self
            .current
            .as_ref()
            .and_then(|current| current.branch.as_ref())
            .and_then(MatchedBranch::leaf)
            .cloned();
        self.record_mut(request)
            .expect("recovering request is retained")
            .append_trace(NavigationTraceEvent::new(
                NavigationTraceEventKind::HistoryIntentRejected,
            ))
            .expect("running trace is open");
        let reserved = match self.ids.reserve_attempt() {
            Ok(reserved) => reserved,
            Err(_) => {
                self.current = None;
                self.action = None;
                self.fail_reserved_request(
                    request,
                    NavigationFailure::new(
                        NavigationFailureKind::RecoveryFailed,
                        "router fallback identity reservation exhausted",
                    ),
                );
                self.start_next();
                return;
            }
        };
        let location = self
            .request(request)
            .expect("recovering request is retained")
            .requested()
            .clone();
        let host = self
            .state
            .host()
            .expect("accepted push has a confirmed host");
        let top_slot = self
            .state
            .top_slot()
            .expect("confirmed host has a top slot");
        let origin = FrameOrigin::RouterFallback;
        let generation = RouteGeneration::initial();
        let identity = FrameTaskIdentity {
            router: self.router,
            host: host.host(),
            host_epoch: host.epoch(),
            slot: top_slot,
            request,
            transaction: reserved.transaction(),
            frame_task: reserved.frame_task(),
            frame: reserved.candidate_frame(),
            generation,
            origin: origin.clone(),
            branch_index: 0,
        };
        let target_frame = RouteFrameVersion::new(
            self.router,
            host.host(),
            top_slot,
            origin.clone(),
            reserved.candidate_frame(),
            generation,
        );
        let cleanup_roots = self.state.active().frames().iter().rev().cloned().collect();
        let input = RouteViewInput::new(
            location.clone(),
            matched.as_ref().map(MatchedRoute::route),
            matched
                .as_ref()
                .map_or_else(RouteParams::new, |leaf| leaf.params().clone()),
            origin,
            Some(original_failure.clone()),
        );

        let target = FrameMountTarget::TopSlot {
            host: host.clone(),
            slot: top_slot,
        };
        self.current = Some(CurrentWork {
            identity: identity.clone(),
            branch: None,
            location: location.clone(),
            reservation: vec![ReservedFrameAttempt::new(
                reserved.frame_task(),
                reserved.candidate_frame(),
            )],
            target_frames: vec![target_frame.clone()],
            applied_frames: Vec::new(),
            current_target: target.clone(),
            pending_mount: None,
            history_intent: None,
            stage: WorkStage::Applying,
            cleanup_batch: reserved.cleanup_batch(),
            cleanup_roots,
            attempt: AttemptKind::Fallback { original_failure },
            reconciliation: None,
        });
        self.action = None;
        self.record_mut(request)
            .expect("recovering request is retained")
            .append_trace(NavigationTraceEvent::frame_task_issued(
                target_frame.clone(),
            ))
            .expect("running trace is open");
        self.transition_with_trace(request, NavigationPhase::Recovering);
        self.issued_view_tasks.push(identity.clone());
        self.action = Some(EngineAction::FrameTask(FrameTask {
            identity,
            frame: target_frame,
            target,
            location,
            matched,
            view: self.fallback.clone(),
            input: Some(input),
        }));
    }

    fn prepare_confirmed_child(
        &self,
        slot: RouteSlotId,
    ) -> Result<ConfirmedChildPublication, MountError> {
        if !self.has_live_mount_context(&WorkStage::ConfirmingMount, None) {
            return Err(MountError::WrongPhase);
        }
        let mut current = self.current.clone().ok_or(MountError::WrongPhase)?;
        let pending = current.pending_mount.take().ok_or(MountError::WrongPhase)?;
        let index = pending.child_index;
        let matched = current
            .branch
            .as_ref()
            .and_then(|branch| branch.routes().get(index))
            .cloned()
            .ok_or(MountError::WrongPhase)?;
        let reserved = current
            .reservation
            .get(index)
            .copied()
            .ok_or(MountError::WrongPhase)?;
        let request = current.identity.request;
        let transaction = current.identity.transaction;
        let host = current.identity.host;
        let host_epoch = current.identity.host_epoch;
        let location = current.location.clone();
        let origin = FrameOrigin::MatchedRoute(matched.route());
        let frame = RouteFrameVersion::new(
            self.router,
            host,
            slot,
            origin.clone(),
            reserved.frame(),
            RouteGeneration::initial(),
        );
        let identity = FrameTaskIdentity {
            router: self.router,
            host,
            host_epoch,
            slot,
            request,
            transaction,
            frame_task: reserved.frame_task(),
            frame: reserved.frame(),
            generation: RouteGeneration::initial(),
            origin: origin.clone(),
            branch_index: index,
        };
        let target = FrameMountTarget::ChildOutlet {
            parent: pending.parent,
            outlet: pending.outlet,
        };
        let view = self
            .matcher
            .view_binding(matched.route())
            .cloned()
            .ok_or(MountError::WrongPhase)?;
        let input = RouteViewInput::new(
            location.clone(),
            Some(matched.route()),
            matched.params().clone(),
            origin,
            None,
        );
        current.identity = identity.clone();
        current.target_frames.push(frame.clone());
        current.current_target = target.clone();
        current.stage = WorkStage::Applying;
        let record_index = self
            .records
            .iter()
            .position(|record| record.id() == request)
            .ok_or(MountError::WrongPhase)?;
        let mut record = self.records[record_index].clone();
        record
            .append_trace(NavigationTraceEvent::frame_task_issued(frame.clone()))
            .map_err(|_| MountError::WrongPhase)?;
        let phase = NavigationPhase::ApplyingFrame(index);
        record
            .transition(RequestStatus::Running(phase.clone()))
            .map_err(|_| MountError::WrongPhase)?;
        record
            .append_trace(NavigationTraceEvent::request_phase_changed(phase.clone()))
            .map_err(|_| MountError::WrongPhase)?;
        Ok(ConfirmedChildPublication {
            current,
            record_index,
            record,
            phase,
            identity,
            frame,
            target,
            location,
            matched,
            view,
            input,
        })
    }

    fn apply_confirmed_child(&mut self, publication: ConfirmedChildPublication) {
        let ConfirmedChildPublication {
            current,
            record_index,
            record,
            phase,
            identity,
            frame,
            target,
            location,
            matched,
            view,
            input,
        } = publication;
        self.current = Some(current);
        self.records[record_index] = record;
        self.state.set_request(Some(RequestStatus::Running(phase)));
        self.issued_view_tasks.push(identity.clone());
        self.action = Some(EngineAction::FrameTask(FrameTask {
            identity,
            frame,
            target,
            location,
            matched: Some(matched),
            view,
            input: Some(input),
        }));
    }

    fn reissue_current_frame_task(&mut self) {
        let Some(current) = self.current.as_ref() else {
            return;
        };
        let identity = current.identity.clone();
        let frame = current
            .target_frames
            .last()
            .expect("applying work has an issued frame")
            .clone();
        let target = current.current_target.clone();
        let location = current.location.clone();
        let matched = current
            .branch
            .as_ref()
            .and_then(|branch| branch.routes().get(identity.branch_index))
            .cloned();
        let (view, failure) = match &current.attempt {
            AttemptKind::Normal => (
                self.matcher
                    .view_binding(
                        matched
                            .as_ref()
                            .expect("normal frame belongs to a match")
                            .route(),
                    )
                    .expect("matched route has a view")
                    .clone(),
                None,
            ),
            AttemptKind::Fallback { original_failure } => {
                (self.fallback.clone(), Some(original_failure.clone()))
            }
        };
        let input = RouteViewInput::new(
            location.clone(),
            matched.as_ref().map(MatchedRoute::route),
            matched
                .as_ref()
                .map_or_else(RouteParams::new, |route| route.params().clone()),
            identity.origin.clone(),
            failure,
        );
        self.action = Some(EngineAction::FrameTask(FrameTask {
            identity,
            frame,
            target,
            location,
            matched,
            view,
            input: Some(input),
        }));
    }

    fn record_report_disposition(
        &mut self,
        identity: &RootTaskIdentity,
        disposition: ReportDisposition,
        message: impl Into<String>,
    ) {
        let target = if self.request(identity.request).is_some() {
            DiagnosticTarget::Request(identity.request)
        } else {
            DiagnosticTarget::Router
        };
        if self.report_dispositions.iter().any(|record| {
            record.identity == *identity
                && record.disposition == disposition
                && record.target == target
        }) {
            return;
        }
        if target == DiagnosticTarget::Router
            && self.router_diagnostics.len() == ROUTER_DIAGNOSTIC_RETENTION
        {
            self.router_diagnostics.remove(0);
            let oldest = self
                .report_dispositions
                .iter()
                .position(|record| record.target == DiagnosticTarget::Router)
                .expect("router diagnostic and dedup records stay aligned");
            self.report_dispositions.remove(oldest);
        }
        self.report_dispositions.push(ReportDispositionRecord {
            identity: identity.clone(),
            disposition,
            target,
        });
        let frame = RouteFrameVersion::new(
            identity.router_identity(),
            identity.host(),
            identity.slot(),
            identity.origin().clone(),
            identity.frame(),
            identity.generation(),
        );
        let event = DiagnosticEvent::reported(identity.request(), frame, disposition, message);
        match target {
            DiagnosticTarget::Request(request) => self
                .record_mut(request)
                .expect("diagnostic target request is retained")
                .push_diagnostic(event)
                .expect("report disposition diagnostics are open"),
            DiagnosticTarget::Router => self.router_diagnostics.push(event),
        }
    }

    fn current_active_branch(&self) -> Option<&MatchedBranch> {
        self.active_branch.as_ref()
    }

    fn current_reconciliation_matches_active(&self) -> bool {
        let Some(projection) = self
            .current
            .as_ref()
            .and_then(|current| current.reconciliation.as_ref())
        else {
            return true;
        };
        let Some(host) = self.state.host() else {
            return false;
        };
        let Some(top_slot) = self.state.top_slot() else {
            return false;
        };
        projection.matches_active(
            self.router,
            host.host(),
            host.epoch(),
            top_slot,
            self.state.active().location(),
            self.current_active_branch(),
            self.state.active().frames(),
            &self.active_child_slots,
        )
    }

    fn reject_current_reconciliation_drift(&mut self) -> bool {
        if self.current_reconciliation_matches_active() {
            return false;
        }
        let failure = self.begin_failure_cleanup(
            PendingFailureKind::AuthorityLost,
            NavigationFailure::new(
                NavigationFailureKind::AuthorityLost,
                "active reconciliation baseline drifted",
            ),
        );
        self.action = Some(EngineAction::FailureCleanup(failure));
        true
    }

    fn transition_with_trace(&mut self, request: NavigationRequestId, phase: NavigationPhase) {
        let record = self.record_mut(request).expect("request is retained");
        record
            .transition(RequestStatus::Running(phase.clone()))
            .expect("request is not terminal");
        record
            .append_trace(NavigationTraceEvent::request_phase_changed(phase.clone()))
            .expect("running trace is open");
        self.state.set_request(Some(RequestStatus::Running(phase)));
    }

    fn record_mut(&mut self, id: NavigationRequestId) -> Option<&mut NavigationRequestRecord> {
        self.records.iter_mut().find(|record| record.id() == id)
    }

    fn fail_reserved_request(&mut self, request: NavigationRequestId, failure: NavigationFailure) {
        let record = self
            .record_mut(request)
            .expect("accepted request is retained");
        record
            .transition(RequestStatus::Failed(failure))
            .expect("request is not terminal");
        record
            .append_trace(NavigationTraceEvent::request_terminal())
            .expect("terminal status precedes terminal event");
        self.state.set_request(None);
        self.retain_terminal_records();
    }

    fn precompute_publication(&self, cleanup: CleanupBatchToken) -> PublicationPlan {
        let current = self
            .current
            .as_ref()
            .expect("sealing validated current work");
        let request = current.identity.request;
        let location = self
            .request(request)
            .expect("running record is retained")
            .requested()
            .clone();

        let mut history = self.history.clone();
        if let Some(intent) = current.history_intent.clone() {
            intent.publish(&mut history);
        }

        let mut state = self.state.clone();
        let (active, active_branch, active_child_slots) = match &current.attempt {
            AttemptKind::Normal => {
                let active_child_slots =
                    occupied_child_slots(current.branch.as_ref(), &current.target_frames);
                (
                    ActivePresentation::route(location, current.target_frames.clone()),
                    current.branch.clone(),
                    active_child_slots,
                )
            }
            AttemptKind::Fallback { original_failure } => (
                ActivePresentation::error(
                    location,
                    self.request(request)
                        .expect("running record is retained")
                        .matched()
                        .cloned(),
                    original_failure.clone(),
                    current.target_frames.clone(),
                ),
                None,
                Vec::new(),
            ),
        };
        state.set_active(active);
        state.set_request(Some(RequestStatus::Running(
            NavigationPhase::PostCommitCleanup,
        )));

        let record_index = self
            .records
            .iter()
            .position(|record| record.id() == request)
            .expect("running record is retained");
        let mut published_record = self.records[record_index].clone();
        for frame in &current.target_frames {
            published_record
                .append_trace(NavigationTraceEvent::frame_activated(frame.clone()))
                .expect("running trace is open");
        }
        published_record
            .append_trace(NavigationTraceEvent::commit_published())
            .expect("running trace is open");
        published_record
            .transition(RequestStatus::Running(NavigationPhase::PostCommitCleanup))
            .expect("request is running");
        published_record
            .append_trace(NavigationTraceEvent::request_phase_changed(
                NavigationPhase::PostCommitCleanup,
            ))
            .expect("running trace is open");

        let mut current = current.clone();
        current.history_intent = None;
        current.stage = WorkStage::PostCommitCleanup {
            outstanding: cleanup,
        };

        PublicationPlan {
            state,
            history,
            active_branch,
            active_child_slots,
            record_index,
            published_record,
            current,
        }
    }

    fn retain_terminal_records(&mut self) {
        while self
            .records
            .iter()
            .filter(|record| is_terminal(record.status()))
            .count()
            > TERMINAL_RETENTION
        {
            let index = self
                .records
                .iter()
                .position(|record| is_terminal(record.status()))
                .expect("terminal count was nonzero");
            let evicted = self.records.remove(index).expect("index came from records");
            let evicted_request = evicted.id();
            self.issued_view_tasks
                .retain(|identity| identity.request != evicted_request);
            self.report_dispositions
                .retain(|record| record.target != DiagnosticTarget::Request(evicted_request));
            self.completed_cleanup
                .retain(|tombstone| tombstone.request != evicted_request);
            let sequence = evicted.id().sequence();
            self.evicted_through = Some(
                self.evicted_through
                    .map_or(sequence, |current| current.max(sequence)),
            );
        }
    }

    #[cfg(test)]
    fn snapshot(&self) -> EngineSnapshot {
        EngineSnapshot {
            state: self.state.snapshot(),
            ids: self.ids.snapshot(),
            history: self.history.clone(),
            active_branch: self
                .active_branch
                .as_ref()
                .map(|branch| format!("{branch:?}")),
            active_child_slots: self.active_child_slots.clone(),
            records: self.records.clone(),
            evicted_through: self.evicted_through,
            highest_issued: self.highest_issued,
            queue: self
                .queue
                .iter()
                .map(|queued| format!("{queued:?}"))
                .collect(),
            current: self.current.as_ref().map(|current| format!("{current:?}")),
            action: self.action.as_ref().map(ActionSnapshot::from),
            mount: self.mount.snapshot(),
        }
    }

    #[cfg(test)]
    pub(crate) fn testing_snapshot(&self) -> EngineSnapshot {
        self.snapshot()
    }

    #[cfg(test)]
    pub(crate) fn testing_set_mount_registration_context_live(&mut self, live: bool) {
        self.current
            .as_mut()
            .expect("mount registration fixture retains current work")
            .stage = if live {
            WorkStage::ConfirmingMount
        } else {
            WorkStage::AwaitingReceipt
        };
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn testing_set_mount_receipt_context_live(&mut self, live: bool) {
        self.current
            .as_mut()
            .expect("mount receipt fixture retains current work")
            .stage = if live {
            WorkStage::AwaitingReceipt
        } else {
            WorkStage::ConfirmingMount
        };
    }

    #[cfg(test)]
    fn testing_exhaust_reservation_counter(&mut self, counter: crate::ids::ReservationCounter) {
        self.ids
            .testing_exhaust_reservation_counter(self.router, counter);
    }

    #[cfg(test)]
    fn testing_exhaust_mount_sequence_counter(&mut self) {
        self.ids.testing_exhaust_mount_sequence_counter();
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn testing_exhaust_slot_counter(&mut self) {
        self.ids.testing_exhaust_slot_counter();
    }

    #[cfg(test)]
    const fn testing_evicted_watermark(&self) -> Option<u64> {
        self.evicted_through
    }

    #[cfg(test)]
    fn testing_set_active_generation(&mut self, generation: RouteGeneration) {
        let active = self
            .state
            .active()
            .frames()
            .first()
            .expect("test requires active route")
            .clone();
        self.state.set_active(ActivePresentation::route(
            self.state
                .active()
                .location()
                .expect("test requires active route")
                .clone(),
            vec![RouteFrameVersion::new(
                active.router_identity(),
                active.host(),
                active.slot(),
                active.origin().clone(),
                active.frame(),
                generation,
            )],
        ));
    }
}

fn mount_failure(rejection: MountRejectionKind) -> (PendingFailureKind, NavigationFailure) {
    match rejection {
        MountRejectionKind::MissingOutlet => (
            PendingFailureKind::Mount(rejection),
            NavigationFailure::new(
                NavigationFailureKind::Mount,
                "required child outlet missing",
            ),
        ),
        MountRejectionKind::InvalidOutlet => (
            PendingFailureKind::Mount(rejection),
            NavigationFailure::new(
                NavigationFailureKind::Mount,
                "child outlet authority invalid",
            ),
        ),
        MountRejectionKind::ParentAuthorityLost => (
            PendingFailureKind::AuthorityLost,
            NavigationFailure::new(
                NavigationFailureKind::AuthorityLost,
                "parent outlet authority lost",
            ),
        ),
        MountRejectionKind::RejectedAfterMutation => (
            PendingFailureKind::Mount(rejection),
            NavigationFailure::new(
                NavigationFailureKind::Mount,
                "child mount rejected after mutation",
            ),
        ),
    }
}

fn is_terminal(status: &RequestStatus) -> bool {
    matches!(status, RequestStatus::Succeeded | RequestStatus::Failed(_))
}

fn occupied_child_slots(
    branch: Option<&MatchedBranch>,
    frames: &[RouteFrameVersion],
) -> Vec<LogicalChildSlotBinding> {
    let Some(branch) = branch else {
        return Vec::new();
    };
    if branch.routes().len() != frames.len() {
        return Vec::new();
    }
    (1..frames.len())
        .map(|child_index| {
            LogicalChildSlotBinding::occupied(
                child_index - 1,
                frames[child_index - 1].clone(),
                branch.routes()[child_index].slot().clone(),
                frames[child_index].clone(),
            )
        })
        .collect()
}

pub(crate) struct ViewReportSession<'a> {
    engine: &'a mut RouterEngine,
    identity: RootTaskIdentity,
}

impl std::fmt::Debug for ViewReportSession<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ViewReportSession(..)")
    }
}

impl ViewReportSession<'_> {
    pub(crate) fn applied(self) -> CommitTicket {
        self.engine
            .accept_applied_report(AppliedFrameReport {
                identity: self.identity,
                outlets: Vec::new(),
                mode: AppliedReportMode::LegacyLeaf,
            })
            .expect("legacy internal report helper is only valid for a leaf frame");
        let Some(EngineAction::Commit(ticket)) = self.engine.action.take() else {
            panic!("a leaf report produces one commit ticket")
        };
        ticket
    }

    pub(crate) fn failed(self, failure: NavigationFailure) -> Option<FailureCleanupRequest> {
        let request = self.identity.request;
        let branch_index = self.identity.branch_index;
        let attempt = self
            .engine
            .current
            .as_ref()
            .expect("report was preflighted")
            .attempt
            .clone();
        let diagnostic_message = match &attempt {
            AttemptKind::Normal => format!(
                "accepted normal {:?} failure: {}",
                failure.kind(),
                failure.message()
            ),
            AttemptKind::Fallback { .. } => format!(
                "accepted fallback {:?} failure: {}",
                failure.kind(),
                failure.message()
            ),
        };
        self.engine.record_report_disposition(
            &self.identity,
            ReportDisposition::Accepted,
            diagnostic_message,
        );

        match attempt {
            AttemptKind::Normal if branch_index > 0 => {
                let expected = match failure.kind() {
                    NavigationFailureKind::Build => PendingFailureKind::Build,
                    NavigationFailureKind::Apply => PendingFailureKind::Apply,
                    _ => PendingFailureKind::Apply,
                };
                Some(self.engine.begin_failure_cleanup(expected, failure))
            }
            AttemptKind::Normal => {
                self.engine.begin_fallback(request, failure);
                None
            }
            AttemptKind::Fallback { .. } => {
                self.engine.current = None;
                self.engine.action = None;
                self.engine.fail_reserved_request(
                    request,
                    NavigationFailure::new(
                        NavigationFailureKind::RecoveryFailed,
                        "router fallback failed",
                    ),
                );
                self.engine.start_next();
                None
            }
        }
    }

    pub(crate) fn failed_with_physical_residual(
        self,
        failure: NavigationFailure,
    ) -> FailureCleanupRequest {
        let branch_index = self.identity.branch_index;
        let attempt = self
            .engine
            .current
            .as_ref()
            .expect("report was preflighted")
            .attempt
            .clone();
        let diagnostic_message = match &attempt {
            AttemptKind::Normal => format!(
                "accepted normal {:?} failure with physical residual: {}",
                failure.kind(),
                failure.message()
            ),
            AttemptKind::Fallback { .. } => format!(
                "accepted fallback {:?} failure with physical residual: {}",
                failure.kind(),
                failure.message()
            ),
        };
        self.engine.record_report_disposition(
            &self.identity,
            ReportDisposition::Accepted,
            diagnostic_message,
        );
        let expected = match failure.kind() {
            NavigationFailureKind::Build => PendingFailureKind::Build,
            NavigationFailureKind::Apply => PendingFailureKind::Apply,
            _ => PendingFailureKind::Apply,
        };
        let (failure, continuation) = match attempt {
            AttemptKind::Normal if branch_index == 0 => {
                (failure, FailureCleanupContinuation::BeginFallback)
            }
            AttemptKind::Normal => (failure, FailureCleanupContinuation::Terminal),
            AttemptKind::Fallback { .. } => (
                NavigationFailure::new(
                    NavigationFailureKind::RecoveryFailed,
                    "router fallback failed",
                ),
                FailureCleanupContinuation::Terminal,
            ),
        };
        self.engine
            .begin_failure_cleanup_with_continuation(expected, failure, continuation)
    }
}

#[cfg(test)]
#[derive(Clone, Debug, Eq, PartialEq)]
enum ActionSnapshot {
    FrameTask(RootTaskIdentity),
    MountRequest(NavigationRequestId, RouteFrameVersion, RouteSlotKey),
    FailureCleanup(NavigationRequestId),
    Commit(RootTaskIdentity),
}

#[cfg(test)]
impl From<&EngineAction> for ActionSnapshot {
    fn from(action: &EngineAction) -> Self {
        match action {
            EngineAction::FrameTask(task) => Self::FrameTask(task.identity.clone()),
            EngineAction::MountRequest(request) => Self::MountRequest(
                request.request,
                request.parent.clone(),
                request.outlet.clone(),
            ),
            EngineAction::FailureCleanup(request) => Self::FailureCleanup(request.request()),
            EngineAction::Commit(ticket) => Self::Commit(ticket.identity.clone()),
        }
    }
}

#[cfg(test)]
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct EngineSnapshot {
    state: RouterNavigationState,
    ids: crate::ids::CoreIdAllocatorSnapshot,
    history: MemoryHistory,
    active_branch: Option<String>,
    active_child_slots: Vec<LogicalChildSlotBinding>,
    records: VecDeque<NavigationRequestRecord>,
    evicted_through: Option<u64>,
    highest_issued: Option<u64>,
    queue: Vec<String>,
    current: Option<String>,
    action: Option<ActionSnapshot>,
    mount: crate::mount::MountIssuerSnapshot,
}

#[cfg(test)]
mod tests {
    use super::{
        ActionSnapshot, CleanupError, CommitError, EngineAction, MountError, PrivateRequestLookup,
        ROUTER_DIAGNOSTIC_RETENTION, ReportError, RootTaskIdentity, RouterEngine,
        TERMINAL_RETENTION, WorkStage, occupied_child_slots,
    };
    use crate::admission::HostAdmissionIssuer;
    use crate::cleanup::{
        CleanupBatchToken, CleanupCompletion, CleanupCompletionDigest, CleanupManifestDigest,
        CleanupManifestSeal,
    };
    use crate::diagnostics::ReportDisposition;
    use crate::frame::{
        FrameOrigin, LogicalChildSlotBinding, ProjectedChildSlotRoleKind, ProjectedEnteringSlot,
        RouteFrameVersion,
    };
    use crate::ids::{
        CleanupBatchId, ReservationCounter, RouteFrameId, RouteGeneration, RouteHostId, RouteSlotId,
    };
    use crate::location::RouteLocation;
    use crate::pattern::RoutePattern;
    use crate::presentation::{ActivePresentation, ActivePresentationKind};
    use crate::request::{
        NavigationFailure, NavigationFailureKind, NavigationPhase, RequestStatus,
    };
    use crate::route_tree::{RouteNodeOptions, RouteTree};
    use crate::slot::RouteSlotKey;
    use crate::trace::{NavigationTraceEvent, NavigationTraceEventKind};
    use crate::view_key::RouteViewKey;

    const NONZERO_OBLIGATION_COUNT: usize = 3;

    fn tree() -> RouteTree {
        let mut builder = RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/").unwrap(),
                RouteNodeOptions::new().with_view(RouteViewKey::new("root").unwrap()),
            )
            .unwrap();
        let root = builder.root_id().unwrap();
        builder
            .route_with_options(
                root,
                RoutePattern::path("items/:id").unwrap(),
                RouteNodeOptions::new().with_view(RouteViewKey::new("items").unwrap()),
            )
            .unwrap();
        builder
            .route_with_options(
                root,
                RoutePattern::path("about").unwrap(),
                RouteNodeOptions::new().with_view(RouteViewKey::new("about").unwrap()),
            )
            .unwrap();
        builder.build().unwrap()
    }

    fn reconciliation_tree() -> RouteTree {
        let mut builder = RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/").unwrap(),
                RouteNodeOptions::new().with_view(RouteViewKey::new("layout").unwrap()),
            )
            .unwrap();
        let root = builder.root_id().unwrap();
        let items = builder
            .route_with_options(
                root,
                RoutePattern::path("items").unwrap(),
                RouteNodeOptions::new().with_view(RouteViewKey::new("items").unwrap()),
            )
            .unwrap();
        builder
            .route_with_options(
                items,
                RoutePattern::path(":id").unwrap(),
                RouteNodeOptions::new().with_view(RouteViewKey::new("item").unwrap()),
            )
            .unwrap();
        builder.build().unwrap()
    }

    fn ready_engine() -> RouterEngine {
        ready_engine_with_tree(tree())
    }

    fn ready_engine_with_tree(tree: RouteTree) -> RouterEngine {
        let mut engine =
            RouterEngine::install(tree, RouteViewKey::new("fallback").unwrap()).unwrap();
        let mut issuer = HostAdmissionIssuer::new().unwrap();
        let token = engine.request_host_admission(&mut issuer).unwrap();
        let grant = engine.grant_host_admission(&mut issuer, &token).unwrap();
        let confirmation = issuer.confirm(&grant).unwrap();
        engine.confirm_host(&issuer, &confirmation).unwrap();
        engine
    }

    fn publish_current(engine: &mut RouterEngine) -> crate::cleanup::CleanupBatchToken {
        let action = engine.action().unwrap();
        publish_action(engine, action)
    }

    fn publish_action(
        engine: &mut RouterEngine,
        action: EngineAction,
    ) -> crate::cleanup::CleanupBatchToken {
        let ticket = drive_to_commit(engine, action);
        let sealed = seal_current(engine, ticket);
        let cleanup = sealed.cleanup().clone();
        publish_sealed(engine, sealed, || {}).unwrap();
        cleanup
    }

    fn drive_to_commit(engine: &mut RouterEngine, mut action: EngineAction) -> super::CommitTicket {
        loop {
            match action {
                EngineAction::FrameTask(task) => {
                    let next_outlet = engine.current.as_ref().and_then(|current| {
                        current
                            .branch
                            .as_ref()
                            .and_then(|branch| {
                                branch.routes().get(task.identity().branch_index() + 1)
                            })
                            .map(|matched| matched.slot().clone())
                    });
                    engine
                        .accept_applied_report(
                            task.into_applied_report(next_outlet.into_iter().collect()),
                        )
                        .unwrap();
                    action = engine.action().expect("accepted report produces an action");
                }
                EngineAction::MountRequest(request) => {
                    let grant = engine.resolve_mount(request).unwrap();
                    let receipt =
                        grant.attest_receipt(crate::mount::MountReceiptDigest::from_opaque(1));
                    let permit = engine.accept_mount_receipt(receipt).unwrap();
                    let completion =
                        permit.complete(crate::mount::MountRegistrationDigest::from_opaque(2));
                    engine.complete_mount_registration(completion).unwrap();
                    action = engine
                        .action()
                        .expect("mount confirmation produces child task");
                }
                EngineAction::FailureCleanup(_) => {
                    panic!("successful drive cannot produce failure cleanup")
                }
                EngineAction::Commit(ticket) => break ticket,
            }
        }
    }

    fn publish_sealed<F>(
        engine: &mut RouterEngine,
        sealed: super::SealedCommit,
        mutation: F,
    ) -> Result<(), CommitError>
    where
        F: FnOnce(),
    {
        let prepared = engine.prepare_publication(sealed)?;
        engine.publish_prepared(prepared, mutation);
        Ok(())
    }

    #[test]
    fn failed_normal_view_report_issues_fresh_router_fallback_without_publishing() {
        let mut engine = ready_engine();
        let request = engine.enqueue_push(RouteLocation::new("/")).unwrap();
        engine.collect();
        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("root frame task expected")
        };
        let normal_identity = task.identity().clone();
        let normal_cleanup_batch = engine.current.as_ref().unwrap().cleanup_batch;

        engine
            .begin_view_report(task.identity())
            .unwrap()
            .failed(NavigationFailure::new(
                NavigationFailureKind::Build,
                "view build rejected",
            ));

        assert_eq!(
            engine.request(request).unwrap().status(),
            &RequestStatus::Running(NavigationPhase::Recovering)
        );
        assert_eq!(engine.history().len(), 0);
        assert_eq!(engine.state.active().kind(), ActivePresentationKind::None);
        let EngineAction::FrameTask(fallback) = engine.action().unwrap() else {
            panic!("normal failure must issue a router fallback task")
        };
        assert_eq!(fallback.identity().request(), request);
        assert_eq!(fallback.identity().host(), normal_identity.host());
        assert_eq!(fallback.identity().slot(), normal_identity.slot());
        assert_ne!(
            fallback.identity().transaction(),
            normal_identity.transaction()
        );
        assert_ne!(
            fallback.identity().frame_task(),
            normal_identity.frame_task()
        );
        assert_ne!(fallback.identity().frame(), normal_identity.frame());
        assert_ne!(
            engine.current.as_ref().unwrap().cleanup_batch,
            normal_cleanup_batch
        );
        assert_eq!(fallback.identity().origin(), &FrameOrigin::RouterFallback);
        assert_eq!(fallback.view(), &RouteViewKey::new("fallback").unwrap());
        assert_eq!(
            fallback.input().failure().map(NavigationFailure::kind),
            Some(NavigationFailureKind::Build)
        );

        let before_stale = engine.snapshot();
        assert_eq!(
            engine.begin_view_report(&normal_identity).unwrap_err(),
            ReportError::Stale
        );
        assert_eq!(engine.state.snapshot(), before_stale.state);
        assert_eq!(engine.ids.snapshot(), before_stale.ids);
        assert_eq!(engine.history(), &before_stale.history);
        assert_eq!(
            engine
                .current
                .as_ref()
                .map(|current| format!("{current:?}")),
            before_stale.current
        );
        assert_eq!(
            engine.action.as_ref().map(ActionSnapshot::from),
            before_stale.action
        );
        assert_eq!(
            report_disposition_count(&engine, request, ReportDisposition::StaleDiscarded),
            1
        );
    }

    #[test]
    fn match_failure_recovery_keeps_matched_none_and_commits_error_without_history() {
        let mut engine = ready_engine();
        let request = engine.push(RouteLocation::new("/missing")).unwrap();
        let EngineAction::FrameTask(fallback) = engine.action().unwrap() else {
            panic!("match failure must issue a router fallback task")
        };

        assert_eq!(fallback.identity().origin(), &FrameOrigin::RouterFallback);
        assert!(fallback.input().route().is_none());
        assert!(engine.request(request).unwrap().matched().is_none());
        assert_eq!(
            engine.request(request).unwrap().trace()[0].kind(),
            NavigationTraceEventKind::HistoryIntentRejected
        );
        assert_eq!(
            engine.request(request).unwrap().trace()[1].kind(),
            NavigationTraceEventKind::FrameTaskIssued
        );
        assert_eq!(
            engine
                .request(request)
                .unwrap()
                .trace()
                .iter()
                .filter(|event| event.kind() == NavigationTraceEventKind::HistoryIntentRejected)
                .count(),
            1
        );
        assert_eq!(engine.active().kind(), ActivePresentationKind::None);

        let cleanup = publish_action(&mut engine, EngineAction::FrameTask(fallback));
        assert_eq!(engine.active().kind(), ActivePresentationKind::Error);
        assert!(engine.active().matched().is_none());
        assert_eq!(
            engine.active().failure().map(NavigationFailure::kind),
            Some(NavigationFailureKind::Match)
        );
        assert!(engine.history().is_empty());
        assert_eq!(
            engine.request(request).unwrap().status(),
            &RequestStatus::Running(NavigationPhase::PostCommitCleanup)
        );

        engine.complete_cleanup(valid_completion(&cleanup)).unwrap();
        assert!(matches!(
            engine.request(request).unwrap().status(),
            RequestStatus::Failed(failure) if failure.kind() == NavigationFailureKind::Match
        ));
    }

    #[test]
    fn fallback_failure_preserves_old_presentation_and_history_then_reports_recovery_failed() {
        let mut engine = ready_engine();
        engine.push(RouteLocation::new("/items/1")).unwrap();
        settle_current(&mut engine);
        let old_active = engine.active().clone();
        let old_history = engine.history().clone();

        let request = engine.push(RouteLocation::new("/about")).unwrap();
        let EngineAction::FrameTask(normal) = engine.action().unwrap() else {
            panic!("normal task expected")
        };
        let original_matched = engine.request(request).unwrap().matched().cloned();
        engine
            .begin_view_report(normal.identity())
            .unwrap()
            .failed(NavigationFailure::new(
                NavigationFailureKind::Apply,
                "normal apply rejected",
            ));
        let EngineAction::FrameTask(fallback) = engine.action().unwrap() else {
            panic!("fallback task expected")
        };
        assert_eq!(
            engine.request(request).unwrap().matched(),
            original_matched.as_ref()
        );

        engine
            .begin_view_report(fallback.identity())
            .unwrap()
            .failed(NavigationFailure::new(
                NavigationFailureKind::Build,
                "fallback build rejected",
            ));

        assert_eq!(engine.active(), &old_active);
        assert_eq!(engine.history(), &old_history);
        assert!(matches!(
            engine.request(request).unwrap().status(),
            RequestStatus::Failed(failure)
                if failure.kind() == NavigationFailureKind::RecoveryFailed
        ));
        assert!(engine.current.is_none());
        assert!(engine.action.is_none());
        let accepted_reports = engine
            .request(request)
            .unwrap()
            .diagnostics()
            .iter()
            .filter(|event| event.report_disposition() == Some(ReportDisposition::Accepted))
            .map(|event| event.redacted_message())
            .collect::<Vec<_>>();
        assert_eq!(accepted_reports.len(), 2);
        assert!(accepted_reports[0].contains("normal Apply"));
        assert!(accepted_reports[1].contains("fallback Build"));
    }

    #[test]
    fn post_match_fallback_commit_keeps_original_fingerprint_and_failure() {
        let mut engine = ready_engine();
        let request = engine.push(RouteLocation::new("/items/7")).unwrap();
        let original_matched = engine.request(request).unwrap().matched().cloned().unwrap();
        let EngineAction::FrameTask(normal) = engine.action().unwrap() else {
            panic!("normal task expected")
        };
        engine
            .begin_view_report(normal.identity())
            .unwrap()
            .failed(NavigationFailure::new(
                NavigationFailureKind::Apply,
                "normal apply failed",
            ));
        let EngineAction::FrameTask(fallback) = engine.action().unwrap() else {
            panic!("post-match failure must issue fallback")
        };
        let original_leaf = engine
            .matcher
            .match_location(&RouteLocation::new("/items/7"))
            .unwrap()
            .leaf()
            .unwrap()
            .clone();
        assert_eq!(fallback.input().route(), Some(original_leaf.route()));
        assert_eq!(fallback.input().params(), original_leaf.params());
        let trace = engine.request(request).unwrap().trace();
        let history_rejected = trace
            .iter()
            .position(|event| event.kind() == NavigationTraceEventKind::HistoryIntentRejected)
            .expect("normal failure must reject history intent");
        let fallback_issued = trace
            .iter()
            .rposition(|event| event.kind() == NavigationTraceEventKind::FrameTaskIssued)
            .expect("fallback task must be traced");
        assert!(history_rejected < fallback_issued);
        assert_eq!(
            trace
                .iter()
                .filter(|event| event.kind() == NavigationTraceEventKind::HistoryIntentRejected)
                .count(),
            1
        );

        let cleanup = publish_action(&mut engine, EngineAction::FrameTask(fallback));

        assert_eq!(engine.active().matched(), Some(&original_matched));
        assert_eq!(
            engine.active().failure().map(NavigationFailure::kind),
            Some(NavigationFailureKind::Apply)
        );
        assert!(engine.history().is_empty());

        engine.complete_cleanup(valid_completion(&cleanup)).unwrap();
        assert!(matches!(
            engine.request(request).unwrap().status(),
            RequestStatus::Failed(failure) if failure.kind() == NavigationFailureKind::Apply
        ));
    }

    #[test]
    fn an_error_presentation_never_takes_the_equivalent_zero_effect_path() {
        let mut engine = ready_engine();
        let failed = engine.push(RouteLocation::new("/items/7")).unwrap();
        let EngineAction::FrameTask(normal) = engine.action().unwrap() else {
            panic!("normal task expected")
        };
        engine
            .begin_view_report(normal.identity())
            .unwrap()
            .failed(NavigationFailure::new(
                NavigationFailureKind::Build,
                "normal build failed",
            ));
        let fallback_cleanup = publish_current(&mut engine);
        engine
            .complete_cleanup(valid_completion(&fallback_cleanup))
            .unwrap();
        assert!(matches!(
            engine.request(failed).unwrap().status(),
            RequestStatus::Failed(_)
        ));
        let error_frame = engine.active().frames()[0].clone();

        engine.push(RouteLocation::new("/items/7")).unwrap();
        let EngineAction::FrameTask(normal) = engine.action().unwrap() else {
            panic!("same-location push after error must build a fresh normal frame")
        };
        assert!(matches!(
            normal.identity().origin(),
            FrameOrigin::MatchedRoute(_)
        ));
        assert_ne!(normal.identity().frame(), error_frame.frame());
        assert_eq!(
            engine.current.as_ref().unwrap().cleanup_roots,
            vec![error_frame]
        );
    }

    fn report_disposition_count(
        engine: &RouterEngine,
        request: crate::ids::NavigationRequestId,
        disposition: ReportDisposition,
    ) -> usize {
        engine
            .request(request)
            .unwrap()
            .diagnostics()
            .iter()
            .filter(|event| event.report_disposition() == Some(disposition))
            .count()
    }

    #[test]
    fn rejected_view_reports_are_classified_deduplicated_and_lifecycle_zero_write() {
        let mut duplicate_engine = ready_engine();
        let duplicate_request = duplicate_engine
            .push(RouteLocation::new("/items/7"))
            .unwrap();
        let EngineAction::FrameTask(duplicate_task) = duplicate_engine.action().unwrap() else {
            panic!("normal task expected")
        };
        let ticket = drive_to_commit(
            &mut duplicate_engine,
            EngineAction::FrameTask(duplicate_task),
        );
        let duplicate_identity = ticket.identity().clone();
        let duplicate_frame = duplicate_engine
            .current
            .as_ref()
            .unwrap()
            .target_frames
            .last()
            .unwrap()
            .clone();
        let duplicate_lifecycle = (
            duplicate_engine.state.snapshot(),
            duplicate_engine.ids.snapshot(),
            duplicate_engine.history.clone(),
            format!("{:?}", duplicate_engine.current),
            duplicate_engine.action.as_ref().map(ActionSnapshot::from),
        );
        assert!(
            duplicate_engine
                .begin_view_report(&duplicate_identity)
                .is_err()
        );
        assert!(
            duplicate_engine
                .begin_view_report(&duplicate_identity)
                .is_err()
        );
        assert_eq!(
            report_disposition_count(
                &duplicate_engine,
                duplicate_request,
                ReportDisposition::DuplicateRejected,
            ),
            1
        );
        let duplicate_diagnostic = duplicate_engine
            .request(duplicate_request)
            .unwrap()
            .diagnostics()
            .iter()
            .find(|event| event.report_disposition() == Some(ReportDisposition::DuplicateRejected))
            .unwrap();
        assert_eq!(duplicate_diagnostic.request(), Some(duplicate_request));
        assert_eq!(duplicate_diagnostic.frame(), Some(&duplicate_frame));
        assert_eq!(
            (
                duplicate_engine.state.snapshot(),
                duplicate_engine.ids.snapshot(),
                duplicate_engine.history.clone(),
                format!("{:?}", duplicate_engine.current),
                duplicate_engine.action.as_ref().map(ActionSnapshot::from),
            ),
            duplicate_lifecycle
        );

        let mut stale_engine = ready_engine();
        let stale_request = stale_engine.push(RouteLocation::new("/items/7")).unwrap();
        let EngineAction::FrameTask(stale_task) = stale_engine.action().unwrap() else {
            panic!("normal task expected")
        };
        let stale_identity = stale_task.identity().clone();
        stale_engine
            .begin_view_report(&stale_identity)
            .unwrap()
            .failed(NavigationFailure::new(
                NavigationFailureKind::Build,
                "normal build failed",
            ));
        let stale_lifecycle = (
            stale_engine.state.snapshot(),
            stale_engine.ids.snapshot(),
            stale_engine.history.clone(),
            format!("{:?}", stale_engine.current),
            stale_engine.action.as_ref().map(ActionSnapshot::from),
        );
        assert!(stale_engine.begin_view_report(&stale_identity).is_err());
        assert!(stale_engine.begin_view_report(&stale_identity).is_err());
        assert_eq!(
            report_disposition_count(
                &stale_engine,
                stale_request,
                ReportDisposition::StaleDiscarded,
            ),
            1
        );
        assert_eq!(
            (
                stale_engine.state.snapshot(),
                stale_engine.ids.snapshot(),
                stale_engine.history.clone(),
                format!("{:?}", stale_engine.current),
                stale_engine.action.as_ref().map(ActionSnapshot::from),
            ),
            stale_lifecycle
        );

        let mut invalid_engine = ready_engine();
        let invalid_request = invalid_engine.push(RouteLocation::new("/items/7")).unwrap();
        let EngineAction::FrameTask(invalid_task) = invalid_engine.action().unwrap() else {
            panic!("normal task expected")
        };
        let mut invalid_identity = invalid_task.identity().clone();
        invalid_identity.testing_set_host_epoch(invalid_identity.host_epoch() + 1);
        let invalid_lifecycle = (
            invalid_engine.state.snapshot(),
            invalid_engine.ids.snapshot(),
            invalid_engine.history.clone(),
            format!("{:?}", invalid_engine.current),
            invalid_engine.action.as_ref().map(ActionSnapshot::from),
        );
        assert!(invalid_engine.begin_view_report(&invalid_identity).is_err());
        assert!(invalid_engine.begin_view_report(&invalid_identity).is_err());
        let foreign_identity = distinct_identity();
        assert!(invalid_engine.begin_view_report(&foreign_identity).is_err());
        assert!(invalid_engine.begin_view_report(&foreign_identity).is_err());
        assert_eq!(
            report_disposition_count(
                &invalid_engine,
                invalid_request,
                ReportDisposition::InvalidRejected,
            ),
            1
        );
        assert_eq!(
            invalid_engine
                .router_diagnostics()
                .iter()
                .filter(|event| {
                    event.report_disposition() == Some(ReportDisposition::InvalidRejected)
                })
                .count(),
            1
        );
        assert_eq!(
            (
                invalid_engine.state.snapshot(),
                invalid_engine.ids.snapshot(),
                invalid_engine.history.clone(),
                format!("{:?}", invalid_engine.current),
                invalid_engine.action.as_ref().map(ActionSnapshot::from),
            ),
            invalid_lifecycle
        );

        let mut terminal_engine = ready_engine();
        let terminal_request = terminal_engine
            .push(RouteLocation::new("/items/7"))
            .unwrap();
        let EngineAction::FrameTask(terminal_task) = terminal_engine.action().unwrap() else {
            panic!("normal task expected")
        };
        let terminal_cleanup =
            publish_action(&mut terminal_engine, EngineAction::FrameTask(terminal_task));
        let terminal_identity = terminal_cleanup.testing_identity().clone();
        terminal_engine
            .complete_cleanup(valid_completion(&terminal_cleanup))
            .unwrap();
        assert!(
            terminal_engine
                .begin_view_report(&terminal_identity)
                .is_err()
        );
        assert_eq!(
            report_disposition_count(
                &terminal_engine,
                terminal_request,
                ReportDisposition::DuplicateRejected,
            ),
            1
        );
    }

    #[test]
    fn evicted_report_replays_are_router_scoped_and_private_indexes_stay_bounded() {
        let mut engine = ready_engine();
        let total = TERMINAL_RETENTION + ROUTER_DIAGNOSTIC_RETENTION + 2;
        let mut requests = Vec::new();
        let mut identities = Vec::new();
        for index in 0..total {
            let request = engine
                .push(RouteLocation::new(format!("/items/{index}")))
                .unwrap();
            requests.push(request);
            let EngineAction::FrameTask(task) = engine.action().unwrap() else {
                panic!("changed root must issue one task")
            };
            identities.push(task.identity().clone());
            let cleanup = publish_action(&mut engine, EngineAction::FrameTask(task));
            engine.complete_cleanup(valid_completion(&cleanup)).unwrap();
        }
        let newest = *requests.last().unwrap();
        let newest_diagnostics = engine.request(newest).unwrap().diagnostics().to_vec();

        for identity in identities.iter().take(total - TERMINAL_RETENTION) {
            assert_eq!(
                engine.begin_view_report(identity).unwrap_err(),
                ReportError::Invalid
            );
            assert_eq!(
                engine.begin_view_report(identity).unwrap_err(),
                ReportError::Invalid
            );
        }

        assert_eq!(
            engine.request(newest).unwrap().diagnostics(),
            newest_diagnostics
        );
        assert_eq!(
            engine.router_diagnostics().len(),
            ROUTER_DIAGNOSTIC_RETENTION
        );
        assert_eq!(engine.issued_view_tasks.len(), TERMINAL_RETENTION * 2);
        assert_eq!(engine.completed_cleanup.len(), TERMINAL_RETENTION);
        assert_eq!(
            engine.report_dispositions.len(),
            TERMINAL_RETENTION * 2 + ROUTER_DIAGNOSTIC_RETENTION
        );
    }

    #[test]
    fn fallback_reservation_exhaustion_is_atomic_and_settles_recovery_failed() {
        for counter in [
            ReservationCounter::Transaction,
            ReservationCounter::FrameTask,
            ReservationCounter::CandidateFrame,
            ReservationCounter::CleanupBatch,
        ] {
            let mut engine = ready_engine();
            let request = engine.push(RouteLocation::new("/items/7")).unwrap();
            let EngineAction::FrameTask(normal) = engine.action().unwrap() else {
                panic!("normal task expected")
            };
            engine.testing_exhaust_reservation_counter(counter);
            let ids_before = engine.ids.snapshot();

            engine
                .begin_view_report(normal.identity())
                .unwrap()
                .failed(NavigationFailure::new(
                    NavigationFailureKind::Build,
                    "normal build failed",
                ));

            assert_eq!(engine.ids.snapshot(), ids_before, "counter: {counter:?}");
            assert!(matches!(
                engine.request(request).unwrap().status(),
                RequestStatus::Failed(failure)
                    if failure.kind() == NavigationFailureKind::RecoveryFailed
            ));
            assert!(engine.action().is_none());
        }
    }

    fn seal_current(engine: &mut RouterEngine, ticket: super::CommitTicket) -> super::SealedCommit {
        let roots = engine
            .current
            .as_ref()
            .expect("test requires current work")
            .cleanup_roots
            .clone();
        let count = if roots.is_empty() {
            0
        } else {
            NONZERO_OBLIGATION_COUNT
        };
        engine
            .seal_commit(
                &ticket,
                CleanupManifestSeal::new(roots, manifest_digest(41), count),
            )
            .unwrap()
    }

    fn manifest_digest(tag: u64) -> CleanupManifestDigest {
        CleanupManifestDigest::from_opaque(0x4d41_4e49_4645_5354 ^ tag)
    }

    fn settle_current(engine: &mut RouterEngine) {
        let cleanup = publish_current(engine);
        engine.complete_cleanup(valid_completion(&cleanup)).unwrap();
    }

    fn valid_completion(cleanup: &CleanupBatchToken) -> CleanupCompletion {
        let digest = if cleanup.obligation_count() == 0 {
            CleanupCompletionDigest::canonical_empty()
        } else {
            CleanupCompletionDigest::from_opaque(0x5445_524d_494e_414c)
        };
        CleanupCompletion::attest(cleanup.clone(), digest, cleanup.obligation_count())
    }

    fn pending_reconciliation_mount_completion(
        engine: &mut RouterEngine,
    ) -> crate::mount::MountRegistrationCompletion {
        engine.push(RouteLocation::new("/items/2")).unwrap();
        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("reconciliation root task expected")
        };
        let outlet = engine
            .current
            .as_ref()
            .and_then(|current| current.branch.as_ref())
            .and_then(|branch| branch.routes().get(1))
            .map(|route| route.slot().clone())
            .unwrap();
        engine
            .accept_applied_report(task.into_applied_report(vec![outlet]))
            .unwrap();
        let EngineAction::MountRequest(request) = engine.action().unwrap() else {
            panic!("reconciliation mount request expected")
        };
        let grant = engine.resolve_mount(request).unwrap();
        let receipt = grant.attest_receipt(crate::mount::MountReceiptDigest::from_opaque(81));
        let permit = engine.accept_mount_receipt(receipt).unwrap();
        permit.complete(crate::mount::MountRegistrationDigest::from_opaque(82))
    }

    fn settle_reconciliation_authority_loss_and_restart_mount(
        engine: &mut RouterEngine,
        active_child_slots: Vec<LogicalChildSlotBinding>,
    ) {
        let Some(EngineAction::FailureCleanup(failure)) = engine.action() else {
            panic!("reconciliation drift must yield failure cleanup")
        };
        let request = failure.request();
        let count = usize::from(!failure.logical_roots().is_empty());
        let cleanup = prepare_failure_cleanup(engine, failure, 83, count);
        assert_failure_waits_then_settles(
            engine,
            request,
            &cleanup,
            NavigationFailureKind::AuthorityLost,
        );

        engine.active_child_slots = active_child_slots;
        engine.push(RouteLocation::new("/items/2")).unwrap();
        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("replacement reconciliation root task expected")
        };
        let outlet = engine
            .current
            .as_ref()
            .and_then(|current| current.branch.as_ref())
            .and_then(|branch| branch.routes().get(1))
            .map(|route| route.slot().clone())
            .unwrap();
        engine
            .accept_applied_report(task.into_applied_report(vec![outlet]))
            .unwrap();
        let EngineAction::MountRequest(request) = engine.action().unwrap() else {
            panic!("replacement reconciliation mount request expected")
        };
        assert!(engine.resolve_mount(request).is_ok());
    }

    fn distinct_identity() -> RootTaskIdentity {
        let mut engine = ready_engine();
        engine.push(RouteLocation::new("/items/1")).unwrap();
        settle_current(&mut engine);
        engine.push(RouteLocation::new("/about")).unwrap();
        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("expected second frame task");
        };
        task.identity().clone()
    }

    fn identity_mismatches(
        exact: &RootTaskIdentity,
        distinct: &RootTaskIdentity,
    ) -> Vec<RootTaskIdentity> {
        let mut mismatches = Vec::new();
        let mut value = exact.clone();
        value.testing_set_router(distinct.router_identity());
        mismatches.push(value);
        let mut value = exact.clone();
        value.testing_set_host(RouteHostId::new(exact.host().raw() + 1));
        mismatches.push(value);
        let mut value = exact.clone();
        value.testing_set_host_epoch(exact.host_epoch() + 1);
        mismatches.push(value);
        let mut value = exact.clone();
        value.testing_set_top_slot(RouteSlotId::new(exact.slot().raw() + 1));
        mismatches.push(value);
        let mut value = exact.clone();
        value.testing_set_request(distinct.request());
        mismatches.push(value);
        let mut value = exact.clone();
        value.testing_set_transaction(distinct.transaction());
        mismatches.push(value);
        let mut value = exact.clone();
        value.testing_set_frame_task(distinct.frame_task());
        mismatches.push(value);
        let mut value = exact.clone();
        value.testing_set_frame(distinct.frame());
        mismatches.push(value);
        let mut value = exact.clone();
        value.testing_set_generation(RouteGeneration::initial().next());
        mismatches.push(value);
        let mut value = exact.clone();
        value.testing_set_origin(FrameOrigin::RouterFallback);
        mismatches.push(value);
        mismatches
    }

    #[test]
    fn no_host_push_rejects_without_allocating_or_changing_state() {
        let mut engine =
            RouterEngine::install(tree(), RouteViewKey::new("fallback").unwrap()).unwrap();
        let before = engine.snapshot();

        assert!(engine.push(RouteLocation::new("/items/1")).is_err());
        assert_eq!(engine.snapshot(), before);
        assert_eq!(engine.active().kind(), ActivePresentationKind::None);
    }

    #[test]
    fn confirmed_host_push_matches_internally_and_queues_one_running_request() {
        let mut engine = ready_engine();
        let request = engine.push(RouteLocation::new("/items//1/")).unwrap();
        let queued = engine.push(RouteLocation::new("/about")).unwrap();

        assert_ne!(request, queued);
        assert_eq!(
            engine.request(request).unwrap().requested().path(),
            "/items/1"
        );
        assert_eq!(
            engine.request(request).unwrap().status(),
            &RequestStatus::Running(NavigationPhase::ApplyingFrame(0))
        );
        assert_eq!(
            engine.request(queued).unwrap().status(),
            &RequestStatus::Queued
        );
        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("first root push must issue a frame task");
        };
        assert_eq!(task.location().path(), "/items/1");
        assert!(matches!(
            task.identity().origin(),
            FrameOrigin::MatchedRoute(_)
        ));
    }

    #[test]
    fn every_root_task_identity_field_is_checked_before_report_mutation() {
        let mut engine = ready_engine();
        engine.push(RouteLocation::new("/items/1")).unwrap();
        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("expected frame task");
        };
        let exact = task.identity().clone();
        let foreign = distinct_identity();

        for mismatch in identity_mismatches(&exact, &foreign) {
            let before = engine.snapshot();
            assert_eq!(
                engine.begin_view_report(&mismatch).unwrap_err(),
                ReportError::Invalid
            );
            assert_eq!(engine.state.snapshot(), before.state);
            assert_eq!(engine.ids.snapshot(), before.ids);
            assert_eq!(engine.history(), &before.history);
            assert_eq!(
                engine
                    .current
                    .as_ref()
                    .map(|current| format!("{current:?}")),
                before.current
            );
            assert_eq!(
                engine.action.as_ref().map(ActionSnapshot::from),
                before.action
            );
        }
    }

    #[test]
    fn successful_report_does_not_publish_until_preflighted_commit_returns_normally() {
        let mut engine = ready_engine();
        engine.push(RouteLocation::new("/items/1")).unwrap();
        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("expected frame task");
        };
        let ticket = drive_to_commit(&mut engine, EngineAction::FrameTask(task));

        assert_eq!(engine.active().kind(), ActivePresentationKind::None);
        assert!(engine.history().is_empty());

        let mut stale = ticket.testing_duplicate();
        stale.testing_set_host_epoch(stale.identity().host_epoch() + 1);
        let before = engine.snapshot();
        assert_eq!(
            engine
                .seal_commit(
                    &stale,
                    CleanupManifestSeal::new(vec![], manifest_digest(1), 0),
                )
                .unwrap_err(),
            CommitError::Stale
        );
        assert_eq!(engine.snapshot(), before);

        let sealed = seal_current(&mut engine, ticket);
        assert_eq!(engine.active().kind(), ActivePresentationKind::None);
        assert!(engine.history().is_empty());
        let cleanup = sealed.cleanup().clone();
        publish_sealed(&mut engine, sealed, || {}).unwrap();
        assert_eq!(engine.active().kind(), ActivePresentationKind::Route);
        assert_eq!(engine.history().len(), 1);
        assert_eq!(
            engine.current_status(),
            Some(&RequestStatus::Running(NavigationPhase::PostCommitCleanup))
        );
        assert_eq!(cleanup.obligation_count(), 0);
    }

    #[test]
    fn every_root_task_identity_field_is_checked_before_commit_mutation() {
        let mut engine = ready_engine();
        engine.push(RouteLocation::new("/items/1")).unwrap();
        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("expected frame task");
        };
        let exact = drive_to_commit(&mut engine, EngineAction::FrameTask(task));
        let foreign = distinct_identity();

        for mismatch in identity_mismatches(exact.identity(), &foreign) {
            let before = engine.snapshot();
            let stale = exact.testing_with_identity(mismatch);
            assert_eq!(
                engine
                    .seal_commit(
                        &stale,
                        CleanupManifestSeal::new(vec![], manifest_digest(2), 0),
                    )
                    .unwrap_err(),
                CommitError::Stale
            );
            assert_eq!(engine.snapshot(), before);
        }

        let sealed = seal_current(&mut engine, exact);
        let cleanup = sealed.cleanup().clone();
        publish_sealed(&mut engine, sealed, || {}).unwrap();
        engine.complete_cleanup(valid_completion(&cleanup)).unwrap();
    }

    #[test]
    fn equivalent_fragment_and_changed_root_pushes_have_exact_final_semantics() {
        let mut engine = ready_engine();
        engine.push(RouteLocation::new("/items/1")).unwrap();
        settle_current(&mut engine);
        let first = engine.active().frames()[0].clone();

        engine.push(RouteLocation::new("items//1/")).unwrap();
        let equivalent = engine.action().unwrap();
        assert!(matches!(equivalent, EngineAction::Commit(_)));
        let equivalent_cleanup = publish_action(&mut engine, equivalent);
        assert_eq!(equivalent_cleanup.obligation_count(), 0);
        engine
            .complete_cleanup(valid_completion(&equivalent_cleanup))
            .unwrap();
        assert_eq!(engine.active().frames()[0], first);

        engine
            .push(RouteLocation::new("/items/1").with_fragment("details"))
            .unwrap();
        let EngineAction::FrameTask(fragment) = engine.action().unwrap() else {
            panic!("fragment change must issue one task");
        };
        assert_ne!(fragment.identity().frame(), first.frame());
        assert_eq!(fragment.identity().generation(), RouteGeneration::initial());
        let fragment_cleanup = publish_action(&mut engine, EngineAction::FrameTask(fragment));
        assert_eq!(
            fragment_cleanup.obligation_count(),
            NONZERO_OBLIGATION_COUNT
        );
        engine
            .complete_cleanup(valid_completion(&fragment_cleanup))
            .unwrap();

        engine.push(RouteLocation::new("/about")).unwrap();
        let EngineAction::FrameTask(changed) = engine.action().unwrap() else {
            panic!("changed root must issue one entering task");
        };
        assert_ne!(changed.identity().frame(), first.frame());
        assert_eq!(changed.identity().generation(), RouteGeneration::initial());
        let changed_cleanup = publish_action(&mut engine, EngineAction::FrameTask(changed));
        assert_eq!(changed_cleanup.obligation_count(), NONZERO_OBLIGATION_COUNT);
        engine
            .complete_cleanup(valid_completion(&changed_cleanup))
            .unwrap();
        assert_eq!(engine.history().len(), 4);
    }

    #[test]
    fn reconciliation_projection_is_sealed_when_the_request_becomes_running() {
        let mut engine = ready_engine();
        engine.push(RouteLocation::new("/items/1")).unwrap();
        settle_current(&mut engine);
        let active_frames = engine.active().frames().to_vec();

        engine.enqueue_push(RouteLocation::new("/items/2")).unwrap();
        engine.collect();

        let current = engine.current.as_ref().expect("request must be running");
        let projection = current
            .reconciliation
            .as_ref()
            .unwrap_or_else(|| panic!("running reconciliation missing: {current:?}"));
        assert_eq!(projection.preserved().len(), active_frames.len() - 1);
        assert_eq!(projection.leaving().len(), 1);
        assert_eq!(projection.entering().len(), 1);
        assert_eq!(
            projection.preserved()[0].version().frame(),
            active_frames[0].frame()
        );
    }

    #[test]
    fn reconciliation_projection_classifies_equivalent_and_nested_fragment() {
        let mut engine = ready_engine_with_tree(reconciliation_tree());
        engine.push(RouteLocation::new("/items/1")).unwrap();
        settle_current(&mut engine);
        let active = engine.active().frames().to_vec();

        engine.push(RouteLocation::new("/items/1")).unwrap();
        let equivalent = engine
            .current
            .as_ref()
            .and_then(|current| current.reconciliation.as_ref())
            .unwrap();
        assert_eq!(equivalent.preserved().len(), active.len());
        assert!(equivalent.leaving().is_empty());
        assert!(equivalent.entering().is_empty());
        assert!(
            equivalent
                .child_slots()
                .iter()
                .all(|slot| slot.kind() == ProjectedChildSlotRoleKind::PreservedActive)
        );
        settle_current(&mut engine);

        engine
            .push(RouteLocation::new("/items/1").with_fragment("details"))
            .unwrap();
        let fragment = engine
            .current
            .as_ref()
            .and_then(|current| current.reconciliation.as_ref())
            .unwrap();
        assert_eq!(fragment.preserved().len(), active.len() - 1);
        assert_eq!(fragment.leaving().len(), 1);
        assert_eq!(fragment.entering().len(), 1);
        assert_eq!(fragment.leaving()[0].version(), active.last().unwrap());
        assert_eq!(
            fragment.entering()[0].frame(),
            active.last().unwrap().frame()
        );
        assert_eq!(
            fragment.entering()[0].generation(),
            active.last().unwrap().generation().next()
        );
        assert_eq!(
            fragment.entering()[0].slot(),
            ProjectedEnteringSlot::Existing(active.last().unwrap().slot())
        );
    }

    #[test]
    fn reconciliation_projection_classifies_append_truncate_and_empty_slot_reuse() {
        let mut append = ready_engine_with_tree(reconciliation_tree());
        append.push(RouteLocation::new("/items")).unwrap();
        settle_current(&mut append);
        append.push(RouteLocation::new("/items/1")).unwrap();
        let projection = append
            .current
            .as_ref()
            .and_then(|current| current.reconciliation.as_ref())
            .unwrap();
        assert_eq!(projection.preserved().len(), 2);
        assert!(projection.leaving().is_empty());
        assert_eq!(projection.entering().len(), 1);
        assert_eq!(
            projection.entering()[0].slot(),
            ProjectedEnteringSlot::MountNew
        );
        assert!(projection.child_slots().iter().any(|slot| {
            slot.parent_branch_index() == 1 && slot.kind() == ProjectedChildSlotRoleKind::MountNew
        }));

        let mut reuse = ready_engine_with_tree(reconciliation_tree());
        reuse.push(RouteLocation::new("/items")).unwrap();
        settle_current(&mut reuse);
        let active = reuse.active().frames().to_vec();
        let target = reuse
            .matcher
            .match_location(&RouteLocation::new("/items/1"))
            .unwrap();
        let retained_slot = RouteSlotId::new(900);
        reuse
            .active_child_slots
            .push(LogicalChildSlotBinding::retained_empty(
                1,
                active[1].clone(),
                target.routes()[2].slot().clone(),
                retained_slot,
            ));
        reuse.push(RouteLocation::new("/items/1")).unwrap();
        let projection = reuse
            .current
            .as_ref()
            .and_then(|current| current.reconciliation.as_ref())
            .unwrap();
        assert_eq!(
            projection.entering()[0].slot(),
            ProjectedEnteringSlot::Existing(retained_slot)
        );
        assert!(projection.child_slots().iter().any(|slot| {
            slot.parent_branch_index() == 1
                && slot.kind() == ProjectedChildSlotRoleKind::ActivateExisting
        }));

        let mut truncate = ready_engine_with_tree(reconciliation_tree());
        truncate.push(RouteLocation::new("/items/1")).unwrap();
        settle_current(&mut truncate);
        truncate.push(RouteLocation::new("/items")).unwrap();
        let projection = truncate
            .current
            .as_ref()
            .and_then(|current| current.reconciliation.as_ref())
            .unwrap();
        assert_eq!(projection.preserved().len(), 2);
        assert_eq!(projection.leaving().len(), 1);
        assert!(projection.entering().is_empty());
        assert!(projection.child_slots().iter().any(|slot| {
            slot.parent_branch_index() == 1
                && slot.kind() == ProjectedChildSlotRoleKind::RetainEmpty
        }));
    }

    #[test]
    fn reconciliation_projection_search_change_fails_closed() {
        let mut engine = ready_engine_with_tree(reconciliation_tree());
        engine
            .push(RouteLocation::new("/items/1").with_search("tab=a"))
            .unwrap();
        settle_current(&mut engine);

        engine
            .push(RouteLocation::new("/items/1").with_search("tab=b"))
            .unwrap();
        let projection = engine
            .current
            .as_ref()
            .and_then(|current| current.reconciliation.as_ref())
            .unwrap();
        assert!(projection.preserved().is_empty());
        assert_eq!(projection.leaving().len(), 3);
        assert_eq!(projection.entering().len(), 3);
        assert!(
            projection
                .child_slots()
                .iter()
                .filter(|slot| slot.kind() == ProjectedChildSlotRoleKind::Leaving)
                .count()
                >= 2
        );
    }

    #[test]
    fn reconciliation_projection_queued_request_uses_the_newly_published_active() {
        let mut engine = ready_engine_with_tree(reconciliation_tree());
        engine.push(RouteLocation::new("/items/1")).unwrap();
        settle_current(&mut engine);
        let previous = engine.active().frames().to_vec();

        engine.push(RouteLocation::new("/items/2")).unwrap();
        engine.enqueue_push(RouteLocation::new("/items/3")).unwrap();
        let cleanup = publish_current(&mut engine);
        let newly_published = engine.active().frames().to_vec();
        engine.complete_cleanup(valid_completion(&cleanup)).unwrap();

        let projection = engine
            .current
            .as_ref()
            .and_then(|current| current.reconciliation.as_ref())
            .unwrap();
        assert_eq!(projection.preserved()[0].version(), &newly_published[0]);
        assert_eq!(projection.preserved()[1].version(), &newly_published[1]);
        assert_ne!(projection.preserved()[0].version(), &previous[0]);
    }

    #[test]
    fn reconciliation_projection_drift_blocks_later_mount_or_commit_action() {
        let mut engine = ready_engine_with_tree(reconciliation_tree());
        engine.push(RouteLocation::new("/items/1")).unwrap();
        settle_current(&mut engine);

        engine.push(RouteLocation::new("/items/2")).unwrap();
        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("whole-branch scheduling still starts with one frame task in Task 1")
        };
        engine.active_child_slots.clear();
        let next_outlet = engine
            .current
            .as_ref()
            .and_then(|current| current.branch.as_ref())
            .and_then(|branch| branch.routes().get(1))
            .map(|route| route.slot().clone())
            .unwrap();

        assert_eq!(
            engine.accept_applied_report(task.into_applied_report(vec![next_outlet])),
            Err(ReportError::Stale)
        );
        assert!(matches!(
            engine.action(),
            Some(EngineAction::FailureCleanup(_))
        ));
    }

    #[test]
    fn reconciliation_projection_unused_equivalent_reservations_remain_burned() {
        let mut engine = ready_engine_with_tree(reconciliation_tree());
        engine.push(RouteLocation::new("/items/1")).unwrap();
        settle_current(&mut engine);
        let before = engine.ids.snapshot();

        engine.push(RouteLocation::new("/items/1")).unwrap();
        let after = engine.ids.snapshot();
        assert_eq!(after.next_frame(), before.next_frame() + 3);
        assert_eq!(after.next_frame_task(), before.next_frame_task() + 3);
        let projection = engine
            .current
            .as_ref()
            .and_then(|current| current.reconciliation.as_ref())
            .unwrap();
        assert!(projection.entering().is_empty());
    }

    #[test]
    fn reconciliation_projection_rejects_an_empty_binding_for_an_active_child() {
        let mut engine = ready_engine_with_tree(reconciliation_tree());
        engine.push(RouteLocation::new("/items/1")).unwrap();
        settle_current(&mut engine);
        let occupied = engine.active_child_slots[1].clone();
        engine.active_child_slots[1] = LogicalChildSlotBinding::retained_empty(
            occupied.parent_branch_index(),
            occupied.parent().clone(),
            occupied.outlet().clone(),
            occupied.slot(),
        );

        let request = engine.push(RouteLocation::new("/items/2")).unwrap();
        assert!(matches!(
            engine.request(request).unwrap().status(),
            RequestStatus::Failed(failure)
                if failure.kind() == NavigationFailureKind::AuthorityLost
        ));
        assert!(engine.current.is_none());
        assert!(engine.action().is_none());
    }

    #[test]
    fn reconciliation_projection_rejects_duplicate_slot_and_parent_outlet_mappings() {
        for duplicate_slot in [true, false] {
            let mut engine = ready_engine_with_tree(reconciliation_tree());
            engine.push(RouteLocation::new("/items/1")).unwrap();
            settle_current(&mut engine);
            let occupied = engine.active_child_slots[1].clone();
            let slot = if duplicate_slot {
                engine.active_child_slots[0].slot()
            } else {
                RouteSlotId::new(901)
            };
            let outlet = if duplicate_slot {
                RouteSlotKey::named("alternate").unwrap()
            } else {
                occupied.outlet().clone()
            };
            engine
                .active_child_slots
                .push(LogicalChildSlotBinding::retained_empty(
                    occupied.parent_branch_index(),
                    occupied.parent().clone(),
                    outlet,
                    slot,
                ));

            let request = engine.push(RouteLocation::new("/items/2")).unwrap();
            assert!(matches!(
                engine.request(request).unwrap().status(),
                RequestStatus::Failed(failure)
                    if failure.kind() == NavigationFailureKind::AuthorityLost
            ));
            assert!(engine.current.is_none());
            assert!(engine.action().is_none());
        }
    }

    #[test]
    fn reconciliation_projection_nested_fragment_overflow_is_pre_mutation() {
        let mut engine = ready_engine_with_tree(reconciliation_tree());
        engine.push(RouteLocation::new("/items/1")).unwrap();
        settle_current(&mut engine);
        let history_before = engine.history().clone();
        let ids_before = engine.ids.snapshot();
        let location = engine.active().location().unwrap().clone();
        let mut frames = engine.active().frames().to_vec();
        let leaf = frames.last().unwrap().clone();
        *frames.last_mut().unwrap() = RouteFrameVersion::new(
            leaf.router_identity(),
            leaf.host(),
            leaf.slot(),
            leaf.origin().clone(),
            leaf.frame(),
            RouteGeneration::testing_max(),
        );
        engine
            .state
            .set_active(ActivePresentation::route(location.clone(), frames.clone()));
        engine.active_child_slots = occupied_child_slots(engine.active_branch.as_ref(), &frames);

        let request = engine.push(location.with_fragment("overflow")).unwrap();
        assert!(matches!(
            engine.request(request).unwrap().status(),
            RequestStatus::Failed(failure)
                if failure.kind() == NavigationFailureKind::AuthorityLost
        ));
        assert_eq!(engine.active().frames(), frames);
        assert_eq!(engine.history(), &history_before);
        assert_eq!(
            engine.ids.snapshot().next_frame(),
            ids_before.next_frame() + 3
        );
        assert!(engine.action().is_none());
        assert!(
            engine
                .request(request)
                .unwrap()
                .trace()
                .iter()
                .all(|event| {
                    !matches!(
                        event.kind(),
                        NavigationTraceEventKind::CleanupSettled
                            | NavigationTraceEventKind::CommitPublished
                    )
                })
        );
    }

    #[test]
    fn reconciliation_projection_stale_commit_cannot_trigger_drift_settlement() {
        let mut engine = ready_engine_with_tree(reconciliation_tree());
        engine.push(RouteLocation::new("/items/1")).unwrap();
        settle_current(&mut engine);
        engine.push(RouteLocation::new("/items/2")).unwrap();
        let action = engine.action().unwrap();
        let ticket = drive_to_commit(&mut engine, action);
        engine.active_child_slots.clear();
        let before = engine.snapshot();
        let stale = ticket.testing_with_identity(distinct_identity());
        let roots = engine.current.as_ref().unwrap().cleanup_roots.clone();
        let count = if roots.is_empty() {
            0
        } else {
            NONZERO_OBLIGATION_COUNT
        };

        assert!(matches!(
            engine.seal_commit(
                &stale,
                CleanupManifestSeal::new(roots.clone(), manifest_digest(70), count),
            ),
            Err(CommitError::Stale)
        ));
        assert_eq!(engine.snapshot(), before);
        assert!(matches!(
            engine.seal_commit(
                &ticket,
                CleanupManifestSeal::new(roots, manifest_digest(71), count),
            ),
            Err(CommitError::Stale)
        ));
        assert!(matches!(
            engine.action(),
            Some(EngineAction::FailureCleanup(_))
        ));
    }

    #[test]
    fn reconciliation_projection_foreign_report_cannot_trigger_drift_settlement() {
        let mut engine = ready_engine_with_tree(reconciliation_tree());
        engine.push(RouteLocation::new("/items/1")).unwrap();
        settle_current(&mut engine);
        engine.push(RouteLocation::new("/items/2")).unwrap();
        let EngineAction::FrameTask(exact) = engine.action().unwrap() else {
            panic!("exact task expected")
        };
        engine.active_child_slots.clear();

        let mut foreign = ready_engine_with_tree(reconciliation_tree());
        foreign.push(RouteLocation::new("/items/2")).unwrap();
        let EngineAction::FrameTask(foreign_task) = foreign.action().unwrap() else {
            panic!("foreign task expected")
        };
        let next_outlet = engine
            .current
            .as_ref()
            .and_then(|current| current.branch.as_ref())
            .and_then(|branch| branch.routes().get(1))
            .map(|route| route.slot().clone())
            .unwrap();

        assert_eq!(
            engine
                .accept_applied_report(foreign_task.into_applied_report(vec![next_outlet.clone()])),
            Err(ReportError::Invalid)
        );
        assert!(matches!(engine.action(), Some(EngineAction::FrameTask(_))));
        assert_eq!(
            engine.accept_applied_report(exact.into_applied_report(vec![next_outlet])),
            Err(ReportError::Stale)
        );
        assert!(matches!(
            engine.action(),
            Some(EngineAction::FailureCleanup(_))
        ));
    }

    #[test]
    fn reconciliation_projection_mount_completion_is_capability_first_then_drift_checked() {
        let mut engine = ready_engine_with_tree(reconciliation_tree());
        engine.push(RouteLocation::new("/items/1")).unwrap();
        settle_current(&mut engine);
        let exact = pending_reconciliation_mount_completion(&mut engine);
        let active_child_slots = engine.active_child_slots.clone();
        engine.active_child_slots.clear();

        let mut foreign = ready_engine_with_tree(reconciliation_tree());
        foreign.push(RouteLocation::new("/items/1")).unwrap();
        settle_current(&mut foreign);
        let foreign_completion = pending_reconciliation_mount_completion(&mut foreign);
        let before = engine.snapshot();
        assert_eq!(
            engine.complete_mount_registration(foreign_completion),
            Err(MountError::Foreign)
        );
        assert_eq!(engine.snapshot(), before);

        assert_eq!(
            engine.complete_mount_registration(exact),
            Err(MountError::WrongPhase)
        );
        settle_reconciliation_authority_loss_and_restart_mount(&mut engine, active_child_slots);
    }

    #[test]
    fn reconciliation_projection_mount_confirmation_drift_releases_mount_issuer() {
        let mut engine = ready_engine_with_tree(reconciliation_tree());
        engine.push(RouteLocation::new("/items/1")).unwrap();
        settle_current(&mut engine);
        let completion = pending_reconciliation_mount_completion(&mut engine);
        let confirmation = engine.mount.complete_registration(&completion).unwrap();
        let active_child_slots = engine.active_child_slots.clone();
        engine.active_child_slots.clear();

        assert_eq!(
            engine.consume_mount_confirmation(confirmation),
            Err(MountError::WrongPhase)
        );
        settle_reconciliation_authority_loss_and_restart_mount(&mut engine, active_child_slots);
    }

    #[test]
    fn cleanup_completion_is_exact_single_use_and_terminal_event_is_last() {
        let mut engine = ready_engine();
        let request = engine.push(RouteLocation::new("/items/1")).unwrap();
        let cleanup = publish_current(&mut engine);
        let replay = valid_completion(&cleanup);

        assert_eq!(
            engine.request(request).unwrap().status(),
            &RequestStatus::Running(NavigationPhase::PostCommitCleanup)
        );
        assert_eq!(
            engine
                .request(request)
                .unwrap()
                .trace()
                .iter()
                .filter(|event| event.kind() == NavigationTraceEventKind::CleanupSettled)
                .count(),
            0
        );
        engine.complete_cleanup(valid_completion(&cleanup)).unwrap();
        let terminal = engine.snapshot();
        assert_eq!(
            engine.request(request).unwrap().status(),
            &RequestStatus::Succeeded
        );
        assert_eq!(
            engine
                .request(request)
                .unwrap()
                .trace()
                .iter()
                .map(|event| event.kind())
                .rev()
                .take(3)
                .collect::<Vec<_>>(),
            vec![
                NavigationTraceEventKind::RequestTerminal,
                NavigationTraceEventKind::CleanupSettled,
                NavigationTraceEventKind::RequestPhaseChanged,
            ]
        );
        let settled = engine
            .request(request)
            .unwrap()
            .trace()
            .iter()
            .filter(|event| event.kind() == NavigationTraceEventKind::CleanupSettled)
            .collect::<Vec<_>>();
        assert_eq!(settled.len(), 1);
        assert_eq!(settled[0].cleanup_disposition(), None);
        assert_eq!(
            engine.complete_cleanup(replay),
            Err(CleanupError::Duplicate)
        );
        assert_eq!(engine.snapshot(), terminal);
    }

    #[test]
    fn cleanup_rejects_each_foreign_identity_field_and_digest_without_change() {
        let mut engine = ready_engine();
        engine.push(RouteLocation::new("/items/1")).unwrap();
        let cleanup = publish_current(&mut engine);
        let foreign = distinct_identity();

        for mismatch in identity_mismatches(cleanup.testing_identity(), &foreign) {
            let before = engine.snapshot();
            let stale = cleanup.testing_with_identity(mismatch);
            assert!(engine.complete_cleanup(valid_completion(&stale)).is_err());
            assert_eq!(engine.snapshot(), before);
        }

        let mut wrong_digest = valid_completion(&cleanup);
        wrong_digest.testing_set_binding(u64::MAX);
        let before = engine.snapshot();
        assert!(engine.complete_cleanup(wrong_digest).is_err());
        assert_eq!(engine.snapshot(), before);
        engine.complete_cleanup(valid_completion(&cleanup)).unwrap();
    }

    #[test]
    fn private_retention_keeps_64_terminal_records_and_distinguishes_lookup_facts() {
        let mut engine = ready_engine();
        let mut requests = Vec::new();
        for index in 0..66 {
            let request = engine
                .push(RouteLocation::new(format!("/items/{index}")))
                .unwrap();
            requests.push(request);
            settle_current(&mut engine);
        }

        assert!(matches!(
            engine.private_lookup(requests[0]),
            PrivateRequestLookup::Evicted
        ));
        assert!(matches!(
            engine.private_lookup(requests[1]),
            PrivateRequestLookup::Evicted
        ));
        assert!(matches!(
            engine.private_lookup(requests[2]),
            PrivateRequestLookup::Retained(_)
        ));
        assert!(matches!(
            engine.private_lookup(*requests.last().unwrap()),
            PrivateRequestLookup::Retained(_)
        ));

        let mut foreign = ready_engine();
        let unknown = foreign.push(RouteLocation::new("/items/0")).unwrap();
        assert!(matches!(
            engine.private_lookup(unknown),
            PrivateRequestLookup::Unknown
        ));
        assert_eq!(
            engine.testing_evicted_watermark(),
            Some(requests[1].sequence())
        );
    }

    #[test]
    fn confirmed_engine_rejects_new_host_admission_before_touching_fresh_issuer() {
        let mut engine = ready_engine();
        let mut issuer = HostAdmissionIssuer::new().unwrap();
        let before = engine.snapshot();

        assert_eq!(
            engine.request_host_admission(&mut issuer),
            Err(crate::admission::AdmissionError::AuthorityExists)
        );
        assert!(!issuer.has_outstanding());
        assert_eq!(engine.snapshot(), before);
    }

    #[test]
    fn push_reservation_exhaustion_is_zero_change_for_every_counter() {
        for counter in [
            ReservationCounter::Request,
            ReservationCounter::Transaction,
            ReservationCounter::FrameTask,
            ReservationCounter::CandidateFrame,
            ReservationCounter::CleanupBatch,
        ] {
            let mut engine = ready_engine();
            engine.testing_exhaust_reservation_counter(counter);
            let before = engine.snapshot();

            assert_eq!(
                engine.push(RouteLocation::new("/items/1")),
                Err(super::PushError::IdExhausted),
                "counter: {counter:?}"
            );
            assert_eq!(engine.snapshot(), before, "counter: {counter:?}");
        }
    }

    #[test]
    fn commit_ticket_is_logical_only_and_sealed_token_is_tracked_before_publication() {
        let mut engine = ready_engine();
        engine.push(RouteLocation::new("/items/1")).unwrap();
        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("expected frame task");
        };
        let task_identity = task.identity().clone();
        let ticket = drive_to_commit(&mut engine, EngineAction::FrameTask(task));
        let super::CommitTicket { identity } = &ticket;
        assert_eq!(identity.request(), task_identity.request());
        let ids_after_action = engine.ids.snapshot();
        let sealed = engine
            .seal_commit(
                &ticket,
                CleanupManifestSeal::new(vec![], manifest_digest(3), 0),
            )
            .unwrap();
        let expected_cleanup = sealed.cleanup().clone();
        assert!(matches!(
            &engine.current.as_ref().unwrap().stage,
            WorkStage::CommitSealed { token } if token == &expected_cleanup
        ));
        assert_eq!(engine.active().kind(), ActivePresentationKind::None);
        assert!(engine.history().is_empty());

        let mut mutated = false;
        publish_sealed(&mut engine, sealed, || {
            mutated = true;
        })
        .unwrap();

        assert!(mutated);
        assert_eq!(engine.ids.snapshot(), ids_after_action);
        assert_eq!(engine.active().kind(), ActivePresentationKind::Route);
    }

    #[test]
    fn sealed_publication_preserves_later_queue_and_starts_next_request() {
        let mut engine = ready_engine();
        let first = engine.push(RouteLocation::new("/items/1")).unwrap();
        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("expected first frame task");
        };
        let ticket = drive_to_commit(&mut engine, EngineAction::FrameTask(task));
        let sealed = seal_current(&mut engine, ticket);
        let cleanup = sealed.cleanup().clone();

        let second = engine.push(RouteLocation::new("/about")).unwrap();
        let ids_after_second = engine.ids.snapshot();
        assert_eq!(
            engine.request(second).unwrap().status(),
            &RequestStatus::Queued
        );
        assert_eq!(engine.queue.front().unwrap().request, second);

        publish_sealed(&mut engine, sealed, || {}).unwrap();

        assert_eq!(engine.ids.snapshot(), ids_after_second);
        assert_eq!(
            engine.request(second).unwrap().status(),
            &RequestStatus::Queued
        );
        assert_eq!(engine.queue.front().unwrap().request, second);

        engine.complete_cleanup(valid_completion(&cleanup)).unwrap();

        assert_eq!(engine.ids.snapshot(), ids_after_second);
        assert_eq!(
            engine.request(first).unwrap().status(),
            &RequestStatus::Succeeded
        );
        assert_eq!(
            engine.request(second).unwrap().status(),
            &RequestStatus::Running(NavigationPhase::ApplyingFrame(0))
        );
        let EngineAction::FrameTask(second_task) = engine.action().unwrap() else {
            panic!("second request must start after first cleanup");
        };
        assert_eq!(second_task.identity().request(), second);
    }

    #[test]
    fn logical_cleanup_roots_are_retained_and_verified_in_exact_order() {
        let mut engine = ready_engine();
        engine.push(RouteLocation::new("/items/1")).unwrap();
        settle_current(&mut engine);
        let active = engine.active().frames().to_vec();
        let first = active[0].clone();
        let child = active[1].clone();

        engine
            .push(RouteLocation::new("/items/1").with_fragment("details"))
            .unwrap();
        let EngineAction::FrameTask(fragment) = engine.action().unwrap() else {
            panic!("expected fragment task");
        };
        let fragment_ticket = drive_to_commit(&mut engine, EngineAction::FrameTask(fragment));
        assert_eq!(
            engine.current.as_ref().unwrap().cleanup_roots,
            vec![child, first.clone()]
        );

        let second = RouteFrameVersion::new(
            first.router_identity(),
            first.host(),
            first.slot(),
            first.origin().clone(),
            RouteFrameId::new(first.frame().raw() + 10_000),
            first.generation(),
        );
        let foreign = RouteFrameVersion::new(
            first.router_identity(),
            first.host(),
            first.slot(),
            first.origin().clone(),
            RouteFrameId::new(first.frame().raw() + 20_000),
            first.generation(),
        );
        engine.current.as_mut().unwrap().cleanup_roots = vec![first.clone(), second.clone()];

        for invalid in [
            vec![],
            vec![first.clone()],
            vec![first.clone(), second.clone(), first.clone()],
            vec![first.clone(), first.clone()],
            vec![first.clone(), foreign],
            vec![second.clone(), first.clone()],
        ] {
            let before = engine.snapshot();
            assert_eq!(
                engine
                    .seal_commit(
                        &fragment_ticket.testing_duplicate(),
                        CleanupManifestSeal::new(invalid, manifest_digest(4), 7),
                    )
                    .unwrap_err(),
                CommitError::InvalidManifest
            );
            assert_eq!(engine.snapshot(), before);
        }

        let sealed = engine
            .seal_commit(
                &fragment_ticket,
                CleanupManifestSeal::new(vec![first, second], manifest_digest(4), 7),
            )
            .unwrap();
        assert_eq!(sealed.cleanup().obligation_count(), 7);
    }

    #[test]
    fn root_count_coherence_is_rejected_before_any_mutation() {
        let mut engine = ready_engine();
        engine.push(RouteLocation::new("/items/1")).unwrap();
        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("expected frame task");
        };
        let ticket = drive_to_commit(&mut engine, EngineAction::FrameTask(task));

        let before = engine.snapshot();
        assert_eq!(
            engine
                .seal_commit(
                    &ticket.testing_duplicate(),
                    CleanupManifestSeal::new(vec![], manifest_digest(5), 1),
                )
                .unwrap_err(),
            CommitError::InvalidManifest
        );
        assert_eq!(engine.snapshot(), before);

        let root = engine
            .current
            .as_ref()
            .unwrap()
            .target_frames
            .last()
            .unwrap()
            .clone();
        engine.current.as_mut().unwrap().cleanup_roots = vec![root.clone()];
        let before = engine.snapshot();
        assert_eq!(
            engine
                .seal_commit(
                    &ticket,
                    CleanupManifestSeal::new(vec![root], manifest_digest(5), 0),
                )
                .unwrap_err(),
            CommitError::InvalidManifest
        );
        assert_eq!(engine.snapshot(), before);
    }

    #[test]
    fn duplicate_and_foreign_sealed_commit_capabilities_are_zero_write() {
        let mut engine = ready_engine();
        engine.push(RouteLocation::new("/items/1")).unwrap();
        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("expected frame task");
        };
        let ticket = drive_to_commit(&mut engine, EngineAction::FrameTask(task));
        let sealed = seal_current(&mut engine, ticket);
        let duplicate = sealed.testing_duplicate();
        let foreign_identity = distinct_identity();
        let foreign = sealed.testing_with_identity(foreign_identity);

        for invalid in [foreign] {
            let before = engine.snapshot();
            let mut mutated = false;
            assert_eq!(
                publish_sealed(&mut engine, invalid, || {
                    mutated = true;
                })
                .unwrap_err(),
                CommitError::Stale
            );
            assert!(!mutated);
            assert_eq!(engine.snapshot(), before);
        }

        publish_sealed(&mut engine, sealed, || {}).unwrap();
        let before = engine.snapshot();
        let mut mutated = false;
        assert_eq!(
            publish_sealed(&mut engine, duplicate, || {
                mutated = true;
            })
            .unwrap_err(),
            CommitError::Stale
        );
        assert!(!mutated);
        assert_eq!(engine.snapshot(), before);
    }

    #[test]
    fn manifest_digest_count_and_batch_are_bound_into_the_token() {
        let mut engine = ready_engine();
        engine.push(RouteLocation::new("/items/1")).unwrap();
        settle_current(&mut engine);
        engine.push(RouteLocation::new("/about")).unwrap();
        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("expected replacement task");
        };
        let ticket = drive_to_commit(&mut engine, EngineAction::FrameTask(task));
        let current = engine.current.as_ref().unwrap();
        let exact = CleanupBatchToken::sealed(
            ticket.identity().clone(),
            current.cleanup_batch,
            &current.cleanup_roots,
            manifest_digest(10),
            NONZERO_OBLIGATION_COUNT,
        );
        let different_manifest = CleanupBatchToken::sealed(
            ticket.identity().clone(),
            current.cleanup_batch,
            &current.cleanup_roots,
            manifest_digest(11),
            NONZERO_OBLIGATION_COUNT,
        );
        let different_count = CleanupBatchToken::sealed(
            ticket.identity().clone(),
            current.cleanup_batch,
            &current.cleanup_roots,
            manifest_digest(10),
            NONZERO_OBLIGATION_COUNT + 1,
        );
        let different_batch = CleanupBatchToken::sealed(
            ticket.identity().clone(),
            CleanupBatchId::new(current.cleanup_batch.raw() + 1),
            &current.cleanup_roots,
            manifest_digest(10),
            NONZERO_OBLIGATION_COUNT,
        );

        for different in [different_manifest, different_count, different_batch] {
            assert_ne!(exact, different);
            assert_ne!(exact.testing_binding(), different.testing_binding());
        }

        let sealed = engine
            .seal_commit(
                &ticket,
                CleanupManifestSeal::new(
                    current.cleanup_roots.clone(),
                    manifest_digest(10),
                    NONZERO_OBLIGATION_COUNT,
                ),
            )
            .unwrap();
        assert_eq!(sealed.cleanup(), &exact);
    }

    #[test]
    fn nonzero_completion_is_aggregate_attestation_without_core_receipts() {
        let mut engine = ready_engine();
        engine.push(RouteLocation::new("/items/1")).unwrap();
        settle_current(&mut engine);
        let request = engine.push(RouteLocation::new("/about")).unwrap();
        let cleanup = publish_current(&mut engine);
        assert_eq!(cleanup.obligation_count(), NONZERO_OBLIGATION_COUNT);

        engine.complete_cleanup(valid_completion(&cleanup)).unwrap();
        let settled = engine
            .request(request)
            .unwrap()
            .trace()
            .iter()
            .filter(|event| event.kind() == NavigationTraceEventKind::CleanupSettled)
            .collect::<Vec<_>>();
        assert_eq!(settled.len(), 1);
        assert_eq!(settled[0].cleanup_disposition(), None);
    }

    #[test]
    fn completion_rejects_wrong_token_count_zero_representation_and_binding() {
        let mut engine = ready_engine();
        engine.push(RouteLocation::new("/items/1")).unwrap();
        let cleanup = publish_current(&mut engine);

        let mut foreign_engine = ready_engine();
        foreign_engine.push(RouteLocation::new("/items/1")).unwrap();
        let foreign = publish_current(&mut foreign_engine);

        let mut wrong_binding = valid_completion(&cleanup);
        wrong_binding.testing_set_binding(u64::MAX);
        let wrong_batch =
            cleanup.testing_with_batch(CleanupBatchId::new(cleanup.testing_batch().raw() + 1));
        for (invalid, expected) in [
            (
                CleanupCompletion::attest(foreign, CleanupCompletionDigest::canonical_empty(), 0),
                CleanupError::Stale,
            ),
            (
                CleanupCompletion::attest(
                    cleanup.clone(),
                    CleanupCompletionDigest::from_opaque(76),
                    1,
                ),
                CleanupError::Invalid,
            ),
            (
                CleanupCompletion::attest(
                    cleanup.clone(),
                    CleanupCompletionDigest::from_opaque(77),
                    0,
                ),
                CleanupError::Invalid,
            ),
            (
                CleanupCompletion::attest(
                    wrong_batch,
                    CleanupCompletionDigest::canonical_empty(),
                    0,
                ),
                CleanupError::Stale,
            ),
            (wrong_binding, CleanupError::Invalid),
        ] {
            let before = engine.snapshot();
            assert_eq!(engine.complete_cleanup(invalid), Err(expected));
            assert_eq!(engine.snapshot(), before);
        }

        engine.complete_cleanup(valid_completion(&cleanup)).unwrap();
    }

    #[test]
    fn generation_overflow_settles_accepted_request_and_does_not_poison_prior_cleanup() {
        let mut engine = ready_engine();
        let first = engine.push(RouteLocation::new("/")).unwrap();
        let cleanup = publish_current(&mut engine);
        engine.testing_set_active_generation(RouteGeneration::testing_max());
        let overflow = engine
            .push(RouteLocation::new("/").with_fragment("details"))
            .unwrap();

        engine.complete_cleanup(valid_completion(&cleanup)).unwrap();

        assert_eq!(
            engine.request(first).unwrap().status(),
            &RequestStatus::Succeeded
        );
        assert!(matches!(
            engine.request(overflow).unwrap().status(),
            RequestStatus::Failed(_)
        ));
        assert_eq!(engine.current_status(), None);
        assert!(engine.action().is_none());
    }

    #[test]
    fn match_failure_stays_recovering_until_fallback_settles() {
        let mut engine = ready_engine();
        let failed = engine.push(RouteLocation::new("/missing")).unwrap();

        assert_eq!(
            engine.request(failed).unwrap().status(),
            &RequestStatus::Running(NavigationPhase::Recovering)
        );
        let fallback = engine.action().unwrap();
        let cleanup = publish_action(&mut engine, fallback);
        assert_eq!(
            engine.current_status(),
            Some(&RequestStatus::Running(NavigationPhase::PostCommitCleanup))
        );
        engine.complete_cleanup(valid_completion(&cleanup)).unwrap();
        assert!(matches!(
            engine.request(failed).unwrap().status(),
            RequestStatus::Failed(failure) if failure.kind() == NavigationFailureKind::Match
        ));
        assert_eq!(engine.current_status(), None);

        let next = engine.push(RouteLocation::new("/items/1")).unwrap();
        assert_eq!(
            engine.request(next).unwrap().status(),
            &RequestStatus::Running(NavigationPhase::ApplyingFrame(0))
        );
    }

    #[test]
    fn queued_match_failure_fallback_settles_before_next_reserved_request_starts() {
        let mut engine = ready_engine();
        engine.push(RouteLocation::new("/items/1")).unwrap();
        let cleanup = publish_current(&mut engine);
        let failed = engine.push(RouteLocation::new("/missing")).unwrap();
        let next = engine.push(RouteLocation::new("/about")).unwrap();

        engine.complete_cleanup(valid_completion(&cleanup)).unwrap();

        assert_eq!(
            engine.request(failed).unwrap().status(),
            &RequestStatus::Running(NavigationPhase::Recovering)
        );
        let EngineAction::FrameTask(fallback) = engine.action().unwrap() else {
            panic!("failed request must issue fallback before advancing the queue")
        };
        engine
            .begin_view_report(fallback.identity())
            .unwrap()
            .failed(NavigationFailure::new(
                NavigationFailureKind::Build,
                "fallback build failed",
            ));
        assert!(matches!(
            engine.request(failed).unwrap().status(),
            RequestStatus::Failed(failure)
                if failure.kind() == NavigationFailureKind::RecoveryFailed
        ));
        assert_eq!(
            engine.request(next).unwrap().status(),
            &RequestStatus::Running(NavigationPhase::ApplyingFrame(0))
        );
        assert!(matches!(engine.action(), Some(EngineAction::FrameTask(_))));
    }

    #[test]
    fn nested_happy_path_schedules_root_to_leaf_branch() {
        let mut engine = ready_engine();
        let requested = RouteLocation::new("/items/1");
        let request = engine.push(requested.clone()).unwrap();

        let EngineAction::FrameTask(parent) = engine.action().unwrap() else {
            panic!("nested navigation must start with its parent frame task")
        };
        assert_eq!(parent.view(), &RouteViewKey::new("root").unwrap());
        assert!(matches!(
            parent.mount_target(),
            super::FrameMountTarget::TopSlot { .. }
        ));
        let parent_version = parent.frame_version().clone();
        engine
            .accept_applied_report(
                parent.into_applied_report(vec![crate::slot::RouteSlotKey::default()]),
            )
            .unwrap();

        let EngineAction::MountRequest(mount) = engine.action().unwrap() else {
            panic!("parent report must request its default child outlet")
        };
        assert_eq!(mount.request(), request);
        assert_eq!(mount.parent(), &parent_version);
        assert!(mount.outlet().is_default());
        let grant = engine.resolve_mount(mount).unwrap();
        let child_slot = grant.slot();
        let receipt = grant.attest_receipt(crate::mount::MountReceiptDigest::from_opaque(11));
        let permit = engine.accept_mount_receipt(receipt).unwrap();
        assert_eq!(permit.slot(), child_slot);
        assert!(engine.action().is_none());
        let completion = permit.complete(crate::mount::MountRegistrationDigest::from_opaque(13));
        assert!(engine.action().is_none());
        engine.complete_mount_registration(completion).unwrap();

        let EngineAction::FrameTask(child) = engine.action().unwrap() else {
            panic!("exact final mount confirmation must issue the child task")
        };
        assert_eq!(child.view(), &RouteViewKey::new("items").unwrap());
        assert_eq!(child.frame_version().slot(), child_slot);
        assert!(matches!(
            child.mount_target(),
            super::FrameMountTarget::ChildOutlet { parent, outlet }
                if parent == &parent_version && outlet.is_default()
        ));
        let child_version = child.frame_version().clone();
        engine
            .accept_applied_report(child.into_applied_report(Vec::new()))
            .unwrap();
        let EngineAction::Commit(ticket) = engine.action().unwrap() else {
            panic!("leaf report must issue one commit")
        };
        let sealed = seal_current(&mut engine, ticket);
        let cleanup = sealed.cleanup().clone();
        publish_sealed(&mut engine, sealed, || {}).unwrap();

        assert_eq!(
            engine.active().frames(),
            &[parent_version.clone(), child_version.clone()]
        );
        assert_eq!(engine.history().len(), 1);
        assert_eq!(engine.history().current(), Some(&requested));
        let exact = engine
            .request(request)
            .unwrap()
            .trace()
            .iter()
            .filter_map(|event| match event.kind() {
                NavigationTraceEventKind::FrameTaskIssued
                | NavigationTraceEventKind::ShellPrepared
                | NavigationTraceEventKind::ViewApplied
                | NavigationTraceEventKind::OutletRegistered
                | NavigationTraceEventKind::MountRequested
                | NavigationTraceEventKind::MountResolved
                | NavigationTraceEventKind::MountGranted
                | NavigationTraceEventKind::FrameActivated
                | NavigationTraceEventKind::CommitPublished => Some(event.kind()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            exact,
            vec![
                NavigationTraceEventKind::FrameTaskIssued,
                NavigationTraceEventKind::ShellPrepared,
                NavigationTraceEventKind::ViewApplied,
                NavigationTraceEventKind::OutletRegistered,
                NavigationTraceEventKind::MountRequested,
                NavigationTraceEventKind::MountResolved,
                NavigationTraceEventKind::MountGranted,
                NavigationTraceEventKind::FrameTaskIssued,
                NavigationTraceEventKind::ShellPrepared,
                NavigationTraceEventKind::ViewApplied,
                NavigationTraceEventKind::FrameActivated,
                NavigationTraceEventKind::FrameActivated,
                NavigationTraceEventKind::CommitPublished,
            ]
        );
        engine.complete_cleanup(valid_completion(&cleanup)).unwrap();
    }

    #[test]
    fn mount_confirmation_consumer_rejects_foreign_and_consumed_replays_without_write() {
        fn completed_confirmation(
            engine: &mut RouterEngine,
            digest: u64,
        ) -> crate::mount::MountConfirmation {
            let (_, _, mount) =
                begin_nested_mount(engine, vec![crate::slot::RouteSlotKey::default()]);
            let grant = engine.resolve_mount(mount).unwrap();
            let receipt =
                grant.attest_receipt(crate::mount::MountReceiptDigest::from_opaque(digest));
            let permit = engine.accept_mount_receipt(receipt).unwrap();
            let completion = permit.complete(crate::mount::MountRegistrationDigest::from_opaque(
                digest + 1,
            ));
            engine
                .complete_mount_registration_with_replay(completion)
                .unwrap()
        }

        let mut source = ready_engine();
        let replay = completed_confirmation(&mut source, 0x431);
        let completed = source.snapshot();
        assert_eq!(
            source.consume_mount_confirmation(replay),
            Err(crate::mount::MountError::Stale)
        );
        assert_eq!(source.snapshot(), completed);

        let mut foreign_source = ready_engine();
        let foreign = completed_confirmation(&mut foreign_source, 0x439);
        let mut receiver = ready_engine();
        let (_, _, mount) =
            begin_nested_mount(&mut receiver, vec![crate::slot::RouteSlotKey::default()]);
        let grant = receiver.resolve_mount(mount).unwrap();
        let receipt = grant.attest_receipt(crate::mount::MountReceiptDigest::from_opaque(0x443));
        receiver.accept_mount_receipt(receipt).unwrap();
        let before = receiver.snapshot();
        assert_eq!(
            receiver.consume_mount_confirmation(foreign),
            Err(crate::mount::MountError::Foreign)
        );
        assert_eq!(receiver.snapshot(), before);
        assert!(receiver.action().is_none());
    }

    #[test]
    fn nested_equivalent_nested_push_is_zero_entering() {
        let mut engine = ready_engine();
        engine.push(RouteLocation::new("/items/1")).unwrap();
        settle_current(&mut engine);
        let active = engine.active().frames().to_vec();

        let request = engine.push(RouteLocation::new("items//1/")).unwrap();
        let action = engine.action().unwrap();
        assert!(matches!(action, EngineAction::Commit(_)));
        let cleanup = publish_action(&mut engine, action);
        assert_eq!(cleanup.obligation_count(), 0);
        assert_eq!(engine.active().frames(), active);
        assert_eq!(
            engine
                .request(request)
                .unwrap()
                .trace()
                .iter()
                .filter(|event| event.kind() == NavigationTraceEventKind::FrameTaskIssued)
                .count(),
            0
        );
        engine.complete_cleanup(valid_completion(&cleanup)).unwrap();
    }

    #[test]
    fn nested_branch_changing_nested_push_replaces_the_whole_branch() {
        let mut engine = ready_engine();
        engine.push(RouteLocation::new("/items/1")).unwrap();
        settle_current(&mut engine);
        let old = engine.active().frames().to_vec();
        engine.push(RouteLocation::new("/about")).unwrap();
        let EngineAction::FrameTask(parent) = engine.action().unwrap() else {
            panic!("changed nested branch starts with a fresh parent")
        };
        assert_eq!(parent.view(), &RouteViewKey::new("root").unwrap());
        assert_ne!(parent.frame_version().frame(), old[0].frame());
        let ticket = drive_to_commit(&mut engine, EngineAction::FrameTask(parent));
        let current = engine.current.as_ref().unwrap();
        assert_eq!(current.cleanup_roots, vec![old[1].clone(), old[0].clone()]);
        assert!(
            current
                .target_frames
                .iter()
                .zip(&old)
                .all(|(new, old)| new.frame() != old.frame())
        );
        let sealed = seal_current(&mut engine, ticket);
        let cleanup = sealed.cleanup().clone();
        publish_sealed(&mut engine, sealed, || {}).unwrap();
        assert_eq!(engine.active().frames().len(), 2);
        engine.complete_cleanup(valid_completion(&cleanup)).unwrap();
    }

    #[test]
    fn nested_fragment_push_replaces_the_whole_branch_until_reconciliation() {
        let mut engine = ready_engine();
        engine.push(RouteLocation::new("/items/1")).unwrap();
        settle_current(&mut engine);
        let old = engine.active().frames().to_vec();
        engine
            .push(RouteLocation::new("/items/1").with_fragment("details"))
            .unwrap();
        let EngineAction::FrameTask(parent) = engine.action().unwrap() else {
            panic!("nested fragment starts with a fresh parent")
        };
        assert_eq!(parent.view(), &RouteViewKey::new("root").unwrap());
        assert_ne!(parent.frame_version().frame(), old[0].frame());
        assert_eq!(
            parent.frame_version().generation(),
            RouteGeneration::initial()
        );
        let ticket = drive_to_commit(&mut engine, EngineAction::FrameTask(parent));
        let current = engine.current.as_ref().unwrap();
        assert_eq!(current.cleanup_roots, vec![old[1].clone(), old[0].clone()]);
        assert!(
            current
                .target_frames
                .iter()
                .zip(&old)
                .all(|(new, old)| new.frame() != old.frame())
        );
        let sealed = seal_current(&mut engine, ticket);
        let cleanup = sealed.cleanup().clone();
        publish_sealed(&mut engine, sealed, || {}).unwrap();
        engine.complete_cleanup(valid_completion(&cleanup)).unwrap();
    }

    #[test]
    fn nested_depth_one_fragment_push_preserves_frame_and_advances_generation() {
        let mut engine = ready_engine();
        engine.push(RouteLocation::new("/")).unwrap();
        settle_current(&mut engine);
        let first = engine.active().frames()[0].clone();

        engine
            .push(RouteLocation::new("/").with_fragment("details"))
            .unwrap();
        let EngineAction::FrameTask(fragment) = engine.action().unwrap() else {
            panic!("depth-one fragment push must issue a frame task")
        };

        assert_eq!(fragment.identity().frame(), first.frame());
        assert_eq!(fragment.identity().generation(), first.generation().next());
    }

    #[test]
    fn nested_queued_push_matches_once_and_reuses_the_stored_branch() {
        let mut engine = ready_engine();
        let request = engine.enqueue_push(RouteLocation::new("/items/1")).unwrap();
        engine.matcher = crate::matcher::RouteMatcher::new(
            RouteTree::builder()
                .root_with_options(
                    RoutePattern::path("/").unwrap(),
                    RouteNodeOptions::new().with_view(RouteViewKey::new("root").unwrap()),
                )
                .unwrap()
                .build()
                .unwrap(),
        );

        engine.collect();

        let EngineAction::FrameTask(parent) = engine.action().unwrap() else {
            panic!("stored match must still issue the original branch")
        };
        assert_eq!(parent.identity().request(), request);
        assert_eq!(parent.view(), &RouteViewKey::new("root").unwrap());
    }

    #[test]
    fn nested_match_failure_queues_without_branch_reservation_then_reserves_fallback_attempt() {
        let mut engine = ready_engine();
        let before = engine.ids.snapshot();
        let request = engine.enqueue_push(RouteLocation::new("/missing")).unwrap();
        let queued = engine.ids.snapshot();

        assert_eq!(
            engine.request(request).unwrap().status(),
            &RequestStatus::Queued
        );
        assert!(engine.action().is_none());
        assert_eq!(
            queued.next_request(engine.router),
            Some(request.sequence() + 1)
        );
        assert_eq!(queued.next_frame(), before.next_frame());
        assert_eq!(queued.next_frame_task(), before.next_frame_task());
        assert_eq!(queued.next_transaction(), before.next_transaction());
        assert_eq!(queued.next_cleanup_batch(), before.next_cleanup_batch());
        engine.collect();
        let fallback_reserved = engine.ids.snapshot();
        let EngineAction::FrameTask(fallback) = engine.action().unwrap() else {
            panic!("match failure must reserve and issue one fallback attempt")
        };
        assert_eq!(fallback.identity().request(), request);
        assert_eq!(fallback.identity().origin(), &FrameOrigin::RouterFallback);
        assert_eq!(fallback_reserved.next_frame(), queued.next_frame() + 1);
        assert_eq!(
            fallback_reserved.next_frame_task(),
            queued.next_frame_task() + 1
        );
        assert_eq!(
            fallback_reserved.next_transaction(),
            queued.next_transaction() + 1
        );
        assert_eq!(
            fallback_reserved.next_cleanup_batch(),
            queued.next_cleanup_batch() + 1
        );
    }

    #[test]
    fn nested_applied_reports_reject_unsorted_foreign_and_duplicate_capabilities() {
        let mut unsorted_engine = ready_engine();
        unsorted_engine
            .push(RouteLocation::new("/items/1"))
            .unwrap();
        let EngineAction::FrameTask(task) = unsorted_engine.action().unwrap() else {
            panic!("parent task expected")
        };
        let exact_identity = task.identity().clone();
        let lifecycle = (
            unsorted_engine.state.snapshot(),
            unsorted_engine.ids.snapshot(),
            unsorted_engine.history.clone(),
            unsorted_engine.records.clone(),
            format!("{:?}", unsorted_engine.current),
        );
        let report = task.into_applied_report(vec![
            crate::slot::RouteSlotKey::named("z").unwrap(),
            crate::slot::RouteSlotKey::default(),
        ]);
        assert_eq!(
            unsorted_engine.accept_applied_report(report),
            Err(ReportError::Invalid)
        );
        assert_eq!(
            (
                unsorted_engine.state.snapshot(),
                unsorted_engine.ids.snapshot(),
                unsorted_engine.history.clone(),
                unsorted_engine.records.clone(),
                format!("{:?}", unsorted_engine.current),
            ),
            lifecycle
        );
        let EngineAction::FrameTask(reissued) = unsorted_engine.action().unwrap() else {
            panic!("invalid consumed report must restore its exact task")
        };
        assert_eq!(reissued.identity(), &exact_identity);

        let mut foreign_engine = ready_engine();
        foreign_engine.push(RouteLocation::new("/items/1")).unwrap();
        let EngineAction::FrameTask(task) = foreign_engine.action().unwrap() else {
            panic!("parent task expected")
        };
        let exact_identity = task.identity().clone();
        let mut foreign = task.into_applied_report(vec![crate::slot::RouteSlotKey::default()]);
        foreign
            .identity
            .testing_set_router(distinct_identity().router_identity());
        let before = (
            foreign_engine.state.snapshot(),
            foreign_engine.ids.snapshot(),
            foreign_engine.history.clone(),
            format!("{:?}", foreign_engine.current),
        );
        assert_eq!(
            foreign_engine.accept_applied_report(foreign),
            Err(ReportError::Invalid)
        );
        assert_eq!(
            (
                foreign_engine.state.snapshot(),
                foreign_engine.ids.snapshot(),
                foreign_engine.history.clone(),
                format!("{:?}", foreign_engine.current),
            ),
            before
        );
        let EngineAction::FrameTask(reissued) = foreign_engine.action().unwrap() else {
            panic!("foreign consumed report must restore the current exact task")
        };
        assert_eq!(reissued.identity(), &exact_identity);

        let mut duplicate_engine = ready_engine();
        duplicate_engine
            .push(RouteLocation::new("/items/1"))
            .unwrap();
        let EngineAction::FrameTask(task) = duplicate_engine.action().unwrap() else {
            panic!("parent task expected")
        };
        let report = task.into_applied_report(vec![crate::slot::RouteSlotKey::default()]);
        let duplicate = report.testing_duplicate();
        duplicate_engine.accept_applied_report(report).unwrap();
        let before = (
            duplicate_engine.state.snapshot(),
            duplicate_engine.ids.snapshot(),
            duplicate_engine.history.clone(),
            format!("{:?}", duplicate_engine.current),
            duplicate_engine.action.as_ref().map(ActionSnapshot::from),
        );
        assert_eq!(
            duplicate_engine.accept_applied_report(duplicate),
            Err(ReportError::Duplicate)
        );
        assert_eq!(
            (
                duplicate_engine.state.snapshot(),
                duplicate_engine.ids.snapshot(),
                duplicate_engine.history.clone(),
                format!("{:?}", duplicate_engine.current),
                duplicate_engine.action.as_ref().map(ActionSnapshot::from),
            ),
            before
        );
    }

    #[test]
    fn nested_branch_reservation_exhaustion_is_complete_zero_change() {
        let mut engine = ready_engine();
        engine.testing_exhaust_reservation_counter(ReservationCounter::FrameTask);
        let before = engine.snapshot();

        assert_eq!(
            engine.push(RouteLocation::new("/items/1")),
            Err(super::PushError::IdExhausted)
        );
        assert_eq!(engine.snapshot(), before);
    }

    #[test]
    fn nested_mount_resolution_exhaustion_reissues_request_without_fallback() {
        let mut engine = ready_engine();
        let before = engine.ids.snapshot();
        engine.push(RouteLocation::new("/items/1")).unwrap();
        let reserved = engine.ids.snapshot();
        assert_eq!(reserved.next_frame(), before.next_frame() + 2);
        assert_eq!(reserved.next_frame_task(), before.next_frame_task() + 2);
        let EngineAction::FrameTask(parent) = engine.action().unwrap() else {
            panic!("parent task expected")
        };
        engine
            .accept_applied_report(
                parent.into_applied_report(vec![crate::slot::RouteSlotKey::default()]),
            )
            .unwrap();
        let EngineAction::MountRequest(mount) = engine.action().unwrap() else {
            panic!("mount request expected")
        };
        engine.ids.testing_exhaust_slot_counter();
        let exhausted = engine.ids.snapshot();

        assert!(matches!(
            engine.resolve_mount(mount).map_err(|error| error.kind()),
            Err(crate::mount::MountError::Exhausted)
        ));
        assert_eq!(engine.ids.snapshot(), exhausted);
        let Some(EngineAction::MountRequest(reissued)) = engine.action() else {
            panic!("resolution exhaustion must reissue the same mount capability")
        };
        let failure = engine
            .reject_mount(
                reissued,
                crate::trace::MountRejectionKind::ParentAuthorityLost,
            )
            .unwrap();
        assert_eq!(
            failure.failure().kind(),
            NavigationFailureKind::AuthorityLost
        );
        assert_eq!(engine.ids.snapshot(), exhausted);
    }

    #[test]
    fn frame_task_issued_is_the_first_trace_event_for_normal_push() {
        let mut engine = ready_engine();
        let request = engine.push(RouteLocation::new("/items/1")).unwrap();

        assert_eq!(
            engine.request(request).unwrap().trace()[0].kind(),
            NavigationTraceEventKind::FrameTaskIssued
        );
    }

    fn begin_nested_mount(
        engine: &mut RouterEngine,
        outlets: Vec<crate::slot::RouteSlotKey>,
    ) -> (
        crate::ids::NavigationRequestId,
        RouteFrameVersion,
        super::FrameMountRequest,
    ) {
        let request = engine.push(RouteLocation::new("/items/1")).unwrap();
        let EngineAction::FrameTask(parent) = engine.action().unwrap() else {
            panic!("nested request starts with a parent task")
        };
        let parent_frame = parent.frame_version().clone();
        engine
            .accept_applied_report(parent.into_applied_report(outlets))
            .unwrap();
        let EngineAction::MountRequest(mount) = engine.action().unwrap() else {
            panic!("accepted parent report yields the required mount request")
        };
        (request, parent_frame, mount)
    }

    #[test]
    fn exact_mount_request_stage_drift_does_not_grant_or_allocate() {
        let mut engine = ready_engine();
        let (_, _, mount) =
            begin_nested_mount(&mut engine, vec![crate::slot::RouteSlotKey::default()]);
        engine.current.as_mut().unwrap().stage = WorkStage::AwaitingReceipt;
        let before = engine.snapshot();

        assert_eq!(
            engine.resolve_mount(mount).map_err(|error| error.kind()),
            Err(crate::mount::MountError::WrongPhase)
        );
        assert_eq!(engine.snapshot(), before);
    }

    #[test]
    fn exact_mount_request_stage_drift_does_not_reject_or_clear_issuer() {
        let mut engine = ready_engine();
        let (_, _, mount) = begin_nested_mount(&mut engine, Vec::new());
        engine.current.as_mut().unwrap().stage = WorkStage::AwaitingReceipt;
        let before = engine.snapshot();

        assert_eq!(
            engine
                .reject_mount(mount, crate::trace::MountRejectionKind::MissingOutlet)
                .map_err(|error| error.kind()),
            Err(crate::mount::MountError::WrongPhase)
        );
        assert_eq!(engine.snapshot(), before);
    }

    #[test]
    fn exact_mount_request_pending_identity_drift_is_zero_write() {
        let mut engine = ready_engine();
        let (_, _, mount) = begin_nested_mount(&mut engine, Vec::new());
        engine
            .current
            .as_mut()
            .unwrap()
            .pending_mount
            .as_mut()
            .unwrap()
            .outlet = crate::slot::RouteSlotKey::named("drift").unwrap();
        let before = engine.snapshot();

        assert_eq!(
            engine.resolve_mount(mount).map_err(|error| error.kind()),
            Err(crate::mount::MountError::WrongPhase)
        );
        assert_eq!(engine.snapshot(), before);
    }

    #[test]
    fn exact_mount_grant_stage_drift_does_not_reject_or_clear_issuer() {
        let mut engine = ready_engine();
        let (_, _, mount) =
            begin_nested_mount(&mut engine, vec![crate::slot::RouteSlotKey::default()]);
        let grant = engine.resolve_mount(mount).unwrap();
        engine.current.as_mut().unwrap().stage = WorkStage::ConfirmingMount;
        let before = engine.snapshot();

        assert_eq!(
            engine
                .reject_mount_after_mutation(super::MountMutationArtifact::Grant(grant))
                .map_err(|error| error.kind()),
            Err(crate::mount::MountError::WrongPhase)
        );
        assert_eq!(engine.snapshot(), before);
    }

    #[test]
    fn exact_mount_permit_stage_drift_does_not_reject_or_clear_issuer() {
        let mut engine = ready_engine();
        let (_, _, mount) =
            begin_nested_mount(&mut engine, vec![crate::slot::RouteSlotKey::default()]);
        let grant = engine.resolve_mount(mount).unwrap();
        let receipt = grant.attest_receipt(crate::mount::MountReceiptDigest::from_opaque(231));
        let permit = engine.accept_mount_receipt(receipt).unwrap();
        engine.current.as_mut().unwrap().stage = WorkStage::AwaitingReceipt;
        let before = engine.snapshot();

        assert_eq!(
            engine
                .reject_mount_after_mutation(super::MountMutationArtifact::RegistrationPermit(
                    permit
                ),)
                .map_err(|error| error.kind()),
            Err(crate::mount::MountError::WrongPhase)
        );
        assert_eq!(engine.snapshot(), before);
    }

    #[test]
    fn exact_mount_receipt_stage_drift_is_zero_write() {
        let mut engine = ready_engine();
        let (_, _, mount) =
            begin_nested_mount(&mut engine, vec![crate::slot::RouteSlotKey::default()]);
        let grant = engine.resolve_mount(mount).unwrap();
        let receipt = grant.attest_receipt(crate::mount::MountReceiptDigest::from_opaque(233));
        engine.current.as_mut().unwrap().stage = WorkStage::ConfirmingMount;
        let before = engine.snapshot();

        assert_eq!(
            engine
                .accept_mount_receipt(receipt)
                .map_err(|error| error.kind()),
            Err(crate::mount::MountError::WrongPhase)
        );
        assert_eq!(engine.snapshot(), before);
    }

    #[test]
    fn exact_mount_completion_stage_drift_is_zero_write() {
        let mut engine = ready_engine();
        let (_, _, mount) =
            begin_nested_mount(&mut engine, vec![crate::slot::RouteSlotKey::default()]);
        let grant = engine.resolve_mount(mount).unwrap();
        let receipt = grant.attest_receipt(crate::mount::MountReceiptDigest::from_opaque(239));
        let permit = engine.accept_mount_receipt(receipt).unwrap();
        let completion = permit.complete(crate::mount::MountRegistrationDigest::from_opaque(241));
        engine.current.as_mut().unwrap().stage = WorkStage::AwaitingReceipt;
        let before = engine.snapshot();

        assert_eq!(
            engine.complete_mount_registration(completion),
            Err(crate::mount::MountError::WrongPhase)
        );
        assert_eq!(engine.snapshot(), before);
    }

    #[test]
    fn mount_registration_begin_context_drift_is_zero_write() {
        let mut engine = ready_engine();
        let (_, _, mount) =
            begin_nested_mount(&mut engine, vec![crate::slot::RouteSlotKey::default()]);
        let grant = engine.resolve_mount(mount).unwrap();
        let (receipt, proof) =
            grant.attest_receipt_with_proof(crate::mount::MountReceiptDigest::from_opaque(257));
        let permit = engine.accept_mount_receipt(receipt).unwrap();
        engine.current.as_mut().unwrap().stage = WorkStage::AwaitingReceipt;
        let before = engine.snapshot();

        assert_eq!(
            engine.validate_mount_registration(&permit, &proof),
            Err(crate::mount::MountError::WrongPhase)
        );
        assert_eq!(engine.snapshot(), before);
    }

    #[test]
    fn current_parent_slot_drift_blocks_mount_resolution_without_writes() {
        let mut engine = ready_engine();
        let (_, _, mount) =
            begin_nested_mount(&mut engine, vec![crate::slot::RouteSlotKey::default()]);
        let drifted_slot = engine.ids.allocate_slot().unwrap();
        engine
            .current
            .as_mut()
            .unwrap()
            .identity
            .testing_set_top_slot(drifted_slot);
        let before = engine.snapshot();

        assert_eq!(
            engine.resolve_mount(mount).map_err(|error| error.kind()),
            Err(crate::mount::MountError::WrongPhase)
        );
        assert_eq!(engine.snapshot(), before);
    }

    #[test]
    fn current_parent_origin_drift_blocks_mount_rejection_without_writes() {
        let mut engine = ready_engine();
        let (_, _, mount) = begin_nested_mount(&mut engine, Vec::new());
        engine
            .current
            .as_mut()
            .unwrap()
            .identity
            .testing_set_origin(FrameOrigin::RouterFallback);
        let before = engine.snapshot();

        assert_eq!(
            engine
                .reject_mount(mount, crate::trace::MountRejectionKind::MissingOutlet)
                .map_err(|error| error.kind()),
            Err(crate::mount::MountError::WrongPhase)
        );
        assert_eq!(engine.snapshot(), before);
    }

    #[test]
    fn current_frame_task_drift_blocks_mount_rejection_without_writes() {
        let mut engine = ready_engine();
        let (_, _, mount) = begin_nested_mount(&mut engine, Vec::new());
        let drifted_frame_task = engine.ids.allocate_frame_task().unwrap();
        engine
            .current
            .as_mut()
            .unwrap()
            .identity
            .testing_set_frame_task(drifted_frame_task);
        let before = engine.snapshot();

        assert_eq!(
            engine
                .reject_mount(mount, crate::trace::MountRejectionKind::MissingOutlet)
                .map_err(|error| error.kind()),
            Err(crate::mount::MountError::WrongPhase)
        );
        assert_eq!(engine.snapshot(), before);
    }

    #[test]
    fn current_parent_frame_drift_blocks_mount_receipt_without_writes() {
        let mut engine = ready_engine();
        let (_, _, mount) =
            begin_nested_mount(&mut engine, vec![crate::slot::RouteSlotKey::default()]);
        let grant = engine.resolve_mount(mount).unwrap();
        let receipt = grant.attest_receipt(crate::mount::MountReceiptDigest::from_opaque(243));
        let drifted_frame = engine.ids.allocate_frame().unwrap();
        engine
            .current
            .as_mut()
            .unwrap()
            .identity
            .testing_set_frame(drifted_frame);
        let before = engine.snapshot();

        assert_eq!(
            engine
                .accept_mount_receipt(receipt)
                .map_err(|error| error.kind()),
            Err(crate::mount::MountError::WrongPhase)
        );
        assert_eq!(engine.snapshot(), before);
    }

    #[test]
    fn current_parent_generation_drift_blocks_mount_completion_without_writes() {
        let mut engine = ready_engine();
        let (_, _, mount) =
            begin_nested_mount(&mut engine, vec![crate::slot::RouteSlotKey::default()]);
        let grant = engine.resolve_mount(mount).unwrap();
        let receipt = grant.attest_receipt(crate::mount::MountReceiptDigest::from_opaque(247));
        let permit = engine.accept_mount_receipt(receipt).unwrap();
        let completion = permit.complete(crate::mount::MountRegistrationDigest::from_opaque(249));
        let generation = engine.current.as_ref().unwrap().identity.generation;
        engine
            .current
            .as_mut()
            .unwrap()
            .identity
            .testing_set_generation(generation.checked_next().unwrap());
        let before = engine.snapshot();

        assert_eq!(
            engine.complete_mount_registration(completion),
            Err(crate::mount::MountError::WrongPhase)
        );
        assert_eq!(engine.snapshot(), before);
    }

    #[test]
    fn pending_child_index_drift_blocks_grant_rejection_without_writes() {
        let mut engine = ready_engine();
        let (_, _, mount) =
            begin_nested_mount(&mut engine, vec![crate::slot::RouteSlotKey::default()]);
        let grant = engine.resolve_mount(mount).unwrap();
        engine
            .current
            .as_mut()
            .unwrap()
            .pending_mount
            .as_mut()
            .unwrap()
            .child_index = 0;
        let before = engine.snapshot();

        assert_eq!(
            engine
                .reject_mount_after_mutation(super::MountMutationArtifact::Grant(grant))
                .map_err(|error| error.kind()),
            Err(crate::mount::MountError::WrongPhase)
        );
        assert_eq!(engine.snapshot(), before);
    }

    #[test]
    fn pending_child_index_drift_cannot_issue_the_wrong_child() {
        let mut engine = ready_engine();
        let (_, _, mount) =
            begin_nested_mount(&mut engine, vec![crate::slot::RouteSlotKey::default()]);
        let grant = engine.resolve_mount(mount).unwrap();
        let receipt = grant.attest_receipt(crate::mount::MountReceiptDigest::from_opaque(253));
        let permit = engine.accept_mount_receipt(receipt).unwrap();
        let completion = permit.complete(crate::mount::MountRegistrationDigest::from_opaque(255));
        engine
            .current
            .as_mut()
            .unwrap()
            .pending_mount
            .as_mut()
            .unwrap()
            .child_index = 0;
        let before = engine.snapshot();

        assert_eq!(
            engine.complete_mount_registration(completion),
            Err(crate::mount::MountError::WrongPhase)
        );
        assert_eq!(engine.snapshot(), before);
    }

    fn prepare_failure_cleanup(
        engine: &mut RouterEngine,
        request: super::FailureCleanupRequest,
        digest: u64,
        count: usize,
    ) -> CleanupBatchToken {
        let roots = request.logical_roots().to_vec();
        engine
            .prepare_failure_cleanup(
                request,
                CleanupManifestSeal::new(roots, manifest_digest(digest), count),
            )
            .unwrap()
    }

    fn assert_failure_waits_then_settles(
        engine: &mut RouterEngine,
        request: crate::ids::NavigationRequestId,
        cleanup: &CleanupBatchToken,
        expected: NavigationFailureKind,
    ) {
        assert!(matches!(
            engine.request(request).unwrap().status(),
            RequestStatus::Running(_)
        ));
        assert!(
            engine
                .request(request)
                .unwrap()
                .trace()
                .iter()
                .all(|event| event.kind() != NavigationTraceEventKind::RequestTerminal)
        );

        engine.complete_cleanup(valid_completion(cleanup)).unwrap();

        assert!(matches!(
            engine.request(request).unwrap().status(),
            RequestStatus::Failed(failure) if failure.kind() == expected
        ));
        assert_eq!(
            engine
                .request(request)
                .unwrap()
                .trace()
                .iter()
                .map(NavigationTraceEvent::kind)
                .rev()
                .take(2)
                .collect::<Vec<_>>(),
            vec![
                NavigationTraceEventKind::RequestTerminal,
                NavigationTraceEventKind::CleanupSettled,
            ]
        );
    }

    #[test]
    fn missing_outlet_waits_for_cleanup_before_mount_failure_terminal() {
        let mut engine = ready_engine();
        let (request, parent, mount) = begin_nested_mount(&mut engine, Vec::new());

        let failure = engine
            .reject_mount(mount, crate::trace::MountRejectionKind::MissingOutlet)
            .unwrap();

        assert_eq!(failure.logical_roots(), &[parent]);
        assert_eq!(
            engine.request(request).unwrap().status(),
            &RequestStatus::Running(NavigationPhase::CleanupPending)
        );
        assert!(engine.action().is_none());
        let cleanup = prepare_failure_cleanup(&mut engine, failure, 101, 1);
        let rejected = engine
            .request(request)
            .unwrap()
            .trace()
            .iter()
            .find(|event| event.kind() == NavigationTraceEventKind::MountRejected)
            .unwrap();
        assert_eq!(
            rejected.mount_rejection(),
            Some(crate::trace::MountRejectionKind::MissingOutlet)
        );
        assert_failure_waits_then_settles(
            &mut engine,
            request,
            &cleanup,
            NavigationFailureKind::Mount,
        );
    }

    #[test]
    fn nested_failure_cleanup_preserves_old_active_branch_and_history() {
        let mut engine = ready_engine();
        engine.push(RouteLocation::new("/about")).unwrap();
        settle_current(&mut engine);
        let old_active = engine.active().clone();
        let old_history = engine.history().clone();
        let old_frames = old_active.frames().to_vec();
        let (request, entering_parent, mount) = begin_nested_mount(&mut engine, Vec::new());

        let failure = engine
            .reject_mount(mount, crate::trace::MountRejectionKind::MissingOutlet)
            .unwrap();

        assert_eq!(failure.logical_roots(), &[entering_parent.clone()]);
        assert!(
            failure
                .logical_roots()
                .iter()
                .all(|root| !old_frames.contains(root))
        );
        assert_eq!(engine.active(), &old_active);
        assert_eq!(engine.history(), &old_history);
        let cleanup = prepare_failure_cleanup(&mut engine, failure, 102, 1);
        engine.complete_cleanup(valid_completion(&cleanup)).unwrap();
        assert_eq!(engine.active(), &old_active);
        assert_eq!(engine.history(), &old_history);
        assert!(matches!(
            engine.request(request).unwrap().status(),
            RequestStatus::Failed(failure)
                if failure.kind() == NavigationFailureKind::Mount
        ));
    }

    #[test]
    fn failure_cleanup_orders_only_accepted_applied_prefix_child_to_parent() {
        let mut builder = RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/").unwrap(),
                RouteNodeOptions::new().with_view(RouteViewKey::new("root").unwrap()),
            )
            .unwrap();
        let root = builder.root_id().unwrap();
        let layout = builder
            .route_with_options(
                root,
                RoutePattern::path("items").unwrap(),
                RouteNodeOptions::new().with_view(RouteViewKey::new("layout").unwrap()),
            )
            .unwrap();
        builder
            .route_with_options(
                layout,
                RoutePattern::path(":id").unwrap(),
                RouteNodeOptions::new().with_view(RouteViewKey::new("item").unwrap()),
            )
            .unwrap();
        let mut engine = ready_engine_with_tree(builder.build().unwrap());
        let request = engine.push(RouteLocation::new("/items/1")).unwrap();
        let EngineAction::FrameTask(root_task) = engine.action().unwrap() else {
            panic!("root task expected")
        };
        let root_frame = root_task.frame_version().clone();
        engine
            .accept_applied_report(
                root_task.into_applied_report(vec![crate::slot::RouteSlotKey::default()]),
            )
            .unwrap();
        let EngineAction::MountRequest(layout_mount) = engine.action().unwrap() else {
            panic!("layout mount expected")
        };
        let grant = engine.resolve_mount(layout_mount).unwrap();
        let receipt = grant.attest_receipt(crate::mount::MountReceiptDigest::from_opaque(211));
        let permit = engine.accept_mount_receipt(receipt).unwrap();
        let completion = permit.complete(crate::mount::MountRegistrationDigest::from_opaque(223));
        engine.complete_mount_registration(completion).unwrap();
        let EngineAction::FrameTask(layout_task) = engine.action().unwrap() else {
            panic!("layout task expected")
        };
        let layout_frame = layout_task.frame_version().clone();
        engine
            .accept_applied_report(layout_task.into_applied_report(Vec::new()))
            .unwrap();
        let EngineAction::MountRequest(leaf_mount) = engine.action().unwrap() else {
            panic!("missing leaf outlet must still yield a mount request")
        };

        let failure = engine
            .reject_mount(leaf_mount, crate::trace::MountRejectionKind::MissingOutlet)
            .unwrap();

        assert_eq!(
            failure.logical_roots(),
            &[layout_frame.clone(), root_frame.clone()]
        );
        let cleanup = engine
            .prepare_failure_cleanup(
                failure,
                CleanupManifestSeal::new(vec![layout_frame, root_frame], manifest_digest(227), 2),
            )
            .unwrap();
        assert_failure_waits_then_settles(
            &mut engine,
            request,
            &cleanup,
            NavigationFailureKind::Mount,
        );
    }

    #[test]
    fn invalid_outlet_waits_for_cleanup_before_mount_failure_terminal() {
        let mut engine = ready_engine();
        let (request, parent, mount) =
            begin_nested_mount(&mut engine, vec![crate::slot::RouteSlotKey::default()]);

        let failure = engine
            .reject_mount(mount, crate::trace::MountRejectionKind::InvalidOutlet)
            .unwrap();

        assert_eq!(failure.logical_roots(), &[parent]);
        let cleanup = prepare_failure_cleanup(&mut engine, failure, 103, 1);
        let rejected = engine
            .request(request)
            .unwrap()
            .trace()
            .iter()
            .find(|event| event.kind() == NavigationTraceEventKind::MountRejected)
            .unwrap();
        assert_eq!(
            rejected.mount_rejection(),
            Some(crate::trace::MountRejectionKind::InvalidOutlet)
        );
        assert_failure_waits_then_settles(
            &mut engine,
            request,
            &cleanup,
            NavigationFailureKind::Mount,
        );
    }

    #[test]
    fn rejected_after_mutation_burns_slot_and_waits_for_cleanup() {
        let mut engine = ready_engine();
        let (request, parent, mount) =
            begin_nested_mount(&mut engine, vec![crate::slot::RouteSlotKey::default()]);
        let grant = engine.resolve_mount(mount).unwrap();
        let burned_slot = grant.slot();
        let next_slot = engine.ids.snapshot().next_slot();

        let failure = engine
            .reject_mount_after_mutation(super::MountMutationArtifact::Grant(grant))
            .unwrap();

        assert_eq!(failure.logical_roots(), &[parent]);
        assert_eq!(engine.ids.snapshot().next_slot(), next_slot);
        let cleanup = prepare_failure_cleanup(&mut engine, failure, 107, 2);
        assert_eq!(cleanup.obligation_count(), 2);
        let rejected = engine
            .request(request)
            .unwrap()
            .trace()
            .iter()
            .find(|event| event.kind() == NavigationTraceEventKind::MountRejectedAfterMutation)
            .unwrap();
        assert_eq!(rejected.slot(), Some(burned_slot));
        assert_eq!(
            rejected.mount_rejection(),
            Some(crate::trace::MountRejectionKind::RejectedAfterMutation)
        );
        assert_failure_waits_then_settles(
            &mut engine,
            request,
            &cleanup,
            NavigationFailureKind::Mount,
        );
    }

    #[test]
    fn child_build_and_apply_failure_wait_for_cleanup() {
        for kind in [NavigationFailureKind::Build, NavigationFailureKind::Apply] {
            let mut engine = ready_engine();
            let (request, parent, mount) =
                begin_nested_mount(&mut engine, vec![crate::slot::RouteSlotKey::default()]);
            let grant = engine.resolve_mount(mount).unwrap();
            let receipt = grant.attest_receipt(crate::mount::MountReceiptDigest::from_opaque(109));
            let permit = engine.accept_mount_receipt(receipt).unwrap();
            let completion =
                permit.complete(crate::mount::MountRegistrationDigest::from_opaque(113));
            engine.complete_mount_registration(completion).unwrap();
            let EngineAction::FrameTask(child) = engine.action().unwrap() else {
                panic!("confirmed child task expected")
            };
            let child_frame = child.frame_version().clone();

            let failure = engine
                .begin_view_report(child.identity())
                .unwrap()
                .failed(NavigationFailure::new(kind, "child frame failed"))
                .expect("child failure requires cleanup");

            assert_eq!(failure.logical_roots(), &[parent.clone()]);
            assert_ne!(failure.logical_roots(), &[child_frame]);
            let cleanup = prepare_failure_cleanup(&mut engine, failure, 127, 2);
            assert_failure_waits_then_settles(&mut engine, request, &cleanup, kind);
        }
    }

    #[test]
    fn parent_authority_loss_recovers_before_authority_lost_terminal() {
        let mut engine = ready_engine();
        let (request, parent, mount) =
            begin_nested_mount(&mut engine, vec![crate::slot::RouteSlotKey::default()]);

        let failure = engine
            .reject_mount(mount, crate::trace::MountRejectionKind::ParentAuthorityLost)
            .unwrap();

        assert_eq!(failure.logical_roots(), &[parent]);
        assert_eq!(
            engine.request(request).unwrap().status(),
            &RequestStatus::Running(NavigationPhase::Recovering)
        );
        assert!(engine.action().is_none());
        assert!(
            engine
                .request(request)
                .unwrap()
                .trace()
                .iter()
                .all(|event| {
                    !(event.kind() == NavigationTraceEventKind::FrameTaskIssued
                        && event
                            .frame()
                            .is_some_and(|frame| frame.origin() == &FrameOrigin::RouterFallback))
                })
        );
        let cleanup = prepare_failure_cleanup(&mut engine, failure, 131, 1);
        assert_failure_waits_then_settles(
            &mut engine,
            request,
            &cleanup,
            NavigationFailureKind::AuthorityLost,
        );
    }

    #[test]
    fn failure_cleanup_request_stage_manifest_and_replay_are_exact_zero_write() {
        let mut engine = ready_engine();
        let (_request, parent, mount) = begin_nested_mount(&mut engine, Vec::new());
        let failure = engine
            .reject_mount(mount, crate::trace::MountRejectionKind::MissingOutlet)
            .unwrap();
        let exact = failure.testing_duplicate();
        let replay = failure.testing_duplicate();

        let incoherent = failure.testing_duplicate();
        let before = engine.snapshot();
        assert_eq!(
            engine.prepare_failure_cleanup(
                incoherent,
                CleanupManifestSeal::new(vec![parent.clone()], manifest_digest(133), 0),
            ),
            Err(super::FailureCleanupError::InvalidManifest)
        );
        assert_eq!(engine.snapshot(), before);

        let mut cross_router = failure.testing_duplicate();
        cross_router
            .identity
            .testing_set_router(distinct_identity().router_identity());
        let cross_router_roots = cross_router.logical_roots().to_vec();
        let before = engine.snapshot();
        assert_eq!(
            engine.prepare_failure_cleanup(
                cross_router,
                CleanupManifestSeal::new(cross_router_roots, manifest_digest(135), 1),
            ),
            Err(super::FailureCleanupError::Foreign)
        );
        assert_eq!(engine.snapshot(), before);

        let mut stale_identity = failure.testing_duplicate();
        stale_identity
            .identity
            .testing_set_host_epoch(stale_identity.identity.host_epoch() + 1);
        let stale_roots = stale_identity.logical_roots().to_vec();
        let before = engine.snapshot();
        assert_eq!(
            engine.prepare_failure_cleanup(
                stale_identity,
                CleanupManifestSeal::new(stale_roots, manifest_digest(137), 1),
            ),
            Err(super::FailureCleanupError::Stale)
        );
        assert_eq!(engine.snapshot(), before);

        let mut wrong_stage = failure.testing_duplicate();
        wrong_stage.expected = super::PendingFailureKind::Build;
        let wrong_stage_roots = wrong_stage.logical_roots().to_vec();
        let before = engine.snapshot();
        assert_eq!(
            engine.prepare_failure_cleanup(
                wrong_stage,
                CleanupManifestSeal::new(wrong_stage_roots, manifest_digest(139), 1),
            ),
            Err(super::FailureCleanupError::Stale)
        );
        assert_eq!(engine.snapshot(), before);

        let before = engine.snapshot();
        assert_eq!(
            engine.prepare_failure_cleanup(
                failure,
                CleanupManifestSeal::new(Vec::new(), manifest_digest(149), 1),
            ),
            Err(super::FailureCleanupError::InvalidManifest)
        );
        assert_eq!(engine.snapshot(), before);

        let cleanup = engine
            .prepare_failure_cleanup(
                exact,
                CleanupManifestSeal::new(vec![parent.clone()], manifest_digest(151), 3),
            )
            .unwrap();
        let prepared = engine.snapshot();
        assert_eq!(
            engine.prepare_failure_cleanup(
                replay,
                CleanupManifestSeal::new(vec![parent], manifest_digest(151), 3),
            ),
            Err(super::FailureCleanupError::Stale)
        );
        assert_eq!(engine.snapshot(), prepared);

        let malformed = CleanupCompletion::attest(
            cleanup.clone(),
            CleanupCompletionDigest::from_opaque(157),
            cleanup.obligation_count() - 1,
        );
        assert_eq!(
            engine.complete_cleanup(malformed),
            Err(CleanupError::Invalid)
        );
        assert_eq!(engine.snapshot(), prepared);
        engine.complete_cleanup(valid_completion(&cleanup)).unwrap();
    }

    #[test]
    fn foreign_and_replayed_mount_rejections_are_classified_without_lifecycle_write() {
        let mut engine = ready_engine();
        let (_request, _parent, exact) = begin_nested_mount(&mut engine, Vec::new());
        let replay = exact.testing_duplicate();
        let mut foreign_engine = ready_engine();
        let (_, _, foreign) = begin_nested_mount(&mut foreign_engine, Vec::new());

        let before = engine.snapshot();
        assert_eq!(
            engine
                .reject_mount(foreign, crate::trace::MountRejectionKind::MissingOutlet)
                .map_err(|error| error.kind()),
            Err(crate::mount::MountError::Foreign)
        );
        assert_eq!(engine.snapshot(), before);

        let before = engine.snapshot();
        assert_eq!(
            engine
                .reject_mount(
                    exact.testing_duplicate(),
                    crate::trace::MountRejectionKind::RejectedAfterMutation,
                )
                .map_err(|error| error.kind()),
            Err(crate::mount::MountError::WrongPhase)
        );
        assert_eq!(engine.snapshot(), before);

        engine
            .reject_mount(exact, crate::trace::MountRejectionKind::MissingOutlet)
            .unwrap();
        let failed = engine.snapshot();
        assert_eq!(
            engine
                .reject_mount(replay, crate::trace::MountRejectionKind::MissingOutlet)
                .map_err(|error| error.kind()),
            Err(crate::mount::MountError::Stale)
        );
        assert_eq!(engine.snapshot(), failed);
    }

    #[test]
    fn registration_stage_rejection_uses_only_the_live_permit_and_burned_slot() {
        let mut engine = ready_engine();
        let (request, parent, mount) =
            begin_nested_mount(&mut engine, vec![crate::slot::RouteSlotKey::default()]);
        let grant = engine.resolve_mount(mount).unwrap();
        let slot = grant.slot();
        let receipt = grant.attest_receipt(crate::mount::MountReceiptDigest::from_opaque(163));
        let permit = engine.accept_mount_receipt(receipt).unwrap();

        let failure = engine
            .reject_mount_after_mutation(super::MountMutationArtifact::RegistrationPermit(permit))
            .unwrap();

        assert_eq!(failure.logical_roots(), &[parent]);
        let cleanup = prepare_failure_cleanup(&mut engine, failure, 167, 2);
        let rejected = engine
            .request(request)
            .unwrap()
            .trace()
            .iter()
            .find(|event| event.kind() == NavigationTraceEventKind::MountRejectedAfterMutation)
            .unwrap();
        assert_eq!(rejected.slot(), Some(slot));
        assert_failure_waits_then_settles(
            &mut engine,
            request,
            &cleanup,
            NavigationFailureKind::Mount,
        );
    }

    #[test]
    fn failure_cleanup_completion_starts_next_request_and_replay_is_duplicate() {
        let mut engine = ready_engine();
        let (failed, parent, mount) = begin_nested_mount(&mut engine, Vec::new());
        let next = engine.enqueue_push(RouteLocation::new("/about")).unwrap();
        let failure = engine
            .reject_mount(mount, crate::trace::MountRejectionKind::MissingOutlet)
            .unwrap();
        let cleanup = engine
            .prepare_failure_cleanup(
                failure,
                CleanupManifestSeal::new(vec![parent], manifest_digest(173), 1),
            )
            .unwrap();
        let completion = valid_completion(&cleanup);
        let replay = completion.clone();

        engine.complete_cleanup(completion).unwrap();

        assert!(matches!(
            engine.request(failed).unwrap().status(),
            RequestStatus::Failed(failure)
                if failure.kind() == NavigationFailureKind::Mount
        ));
        assert_eq!(
            engine.request(next).unwrap().status(),
            &RequestStatus::Running(NavigationPhase::ApplyingFrame(0))
        );
        let Some(EngineAction::FrameTask(task)) = engine.action() else {
            panic!("next queued request starts only after failure cleanup completion")
        };
        assert_eq!(task.identity().request(), next);
        engine.restore_action(EngineAction::FrameTask(task));
        let started = engine.snapshot();
        assert_eq!(
            engine.complete_cleanup(replay),
            Err(CleanupError::Duplicate)
        );
        assert_eq!(engine.snapshot(), started);
    }

    #[test]
    fn mount_sequence_exhaustion_emits_failure_cleanup_action_without_fallback() {
        let mut engine = ready_engine();
        let request = engine.push(RouteLocation::new("/items/1")).unwrap();
        let EngineAction::FrameTask(parent) = engine.action().unwrap() else {
            panic!("nested parent task expected")
        };
        let parent_frame = parent.frame_version().clone();
        engine.testing_exhaust_mount_sequence_counter();
        let ids_before = engine.ids.snapshot();

        engine
            .accept_applied_report(
                parent.into_applied_report(vec![crate::slot::RouteSlotKey::default()]),
            )
            .unwrap();

        assert_eq!(engine.ids.snapshot(), ids_before);
        let Some(EngineAction::FailureCleanup(failure)) = engine.action() else {
            panic!("accepted parent mount-sequence exhaustion must yield failure cleanup")
        };
        assert_eq!(failure.request(), request);
        assert_eq!(failure.logical_roots(), &[parent_frame.clone()]);
        assert_eq!(failure.failure().kind(), NavigationFailureKind::Mount);
        assert_eq!(
            engine.request(request).unwrap().status(),
            &RequestStatus::Running(NavigationPhase::CleanupPending)
        );
        let trace = engine.request(request).unwrap().trace();
        assert!(trace.iter().all(|event| !matches!(
            event.kind(),
            NavigationTraceEventKind::MountRejected
                | NavigationTraceEventKind::MountRejectedAfterMutation
        )));
        assert!(trace.iter().all(|event| {
            event
                .frame()
                .is_none_or(|frame| frame.origin() != &FrameOrigin::RouterFallback)
        }));

        let cleanup = engine
            .prepare_failure_cleanup(
                failure,
                CleanupManifestSeal::new(vec![parent_frame], manifest_digest(179), 1),
            )
            .unwrap();
        assert_failure_waits_then_settles(
            &mut engine,
            request,
            &cleanup,
            NavigationFailureKind::Mount,
        );
    }

    #[test]
    fn resolve_and_mutation_artifacts_preserve_foreign_and_stale_taxonomy() {
        let mut resolve_engine = ready_engine();
        let (_, _, exact_request) = begin_nested_mount(
            &mut resolve_engine,
            vec![crate::slot::RouteSlotKey::default()],
        );
        let replay_request = exact_request.testing_duplicate();
        let mut foreign_engine = ready_engine();
        let (_, _, foreign_request) = begin_nested_mount(
            &mut foreign_engine,
            vec![crate::slot::RouteSlotKey::default()],
        );

        let before = resolve_engine.snapshot();
        assert!(matches!(
            resolve_engine
                .resolve_mount(foreign_request)
                .map_err(|error| error.kind()),
            Err(crate::mount::MountError::Foreign)
        ));
        assert_eq!(resolve_engine.snapshot(), before);
        let exact_grant = resolve_engine.resolve_mount(exact_request).unwrap();
        let grant_replay = exact_grant.clone();
        let resolved = resolve_engine.snapshot();
        assert!(matches!(
            resolve_engine
                .resolve_mount(replay_request)
                .map_err(|error| error.kind()),
            Err(crate::mount::MountError::Stale)
        ));
        assert_eq!(resolve_engine.snapshot(), resolved);

        let mut foreign_grant_engine = ready_engine();
        let (_, _, foreign_mount) = begin_nested_mount(
            &mut foreign_grant_engine,
            vec![crate::slot::RouteSlotKey::default()],
        );
        let foreign_grant = foreign_grant_engine.resolve_mount(foreign_mount).unwrap();
        let before = resolve_engine.snapshot();
        assert!(matches!(
            resolve_engine
                .reject_mount_after_mutation(super::MountMutationArtifact::Grant(foreign_grant))
                .map_err(|error| error.kind()),
            Err(crate::mount::MountError::Foreign)
        ));
        assert_eq!(resolve_engine.snapshot(), before);

        resolve_engine
            .reject_mount_after_mutation(super::MountMutationArtifact::Grant(exact_grant))
            .unwrap();
        let failed = resolve_engine.snapshot();
        assert!(matches!(
            resolve_engine
                .reject_mount_after_mutation(super::MountMutationArtifact::Grant(grant_replay))
                .map_err(|error| error.kind()),
            Err(crate::mount::MountError::Stale)
        ));
        assert_eq!(resolve_engine.snapshot(), failed);
    }

    #[test]
    fn root_apply_failure_still_issues_router_fallback() {
        let mut engine = ready_engine();
        let request = engine.push(RouteLocation::new("/")).unwrap();
        let EngineAction::FrameTask(root) = engine.action().unwrap() else {
            panic!("root task expected")
        };

        let failure_cleanup =
            engine
                .begin_view_report(root.identity())
                .unwrap()
                .failed(NavigationFailure::new(
                    NavigationFailureKind::Apply,
                    "root apply failed",
                ));

        assert!(failure_cleanup.is_none());
        let EngineAction::FrameTask(fallback) = engine.action().unwrap() else {
            panic!("Root apply failure must retain mandatory fallback")
        };
        assert_eq!(fallback.identity().request(), request);
        assert_eq!(fallback.identity().origin(), &FrameOrigin::RouterFallback);
    }

    #[test]
    fn root_partial_failure_waits_for_cleanup_completion_before_fallback() {
        let mut engine = ready_engine();
        let request = engine.push(RouteLocation::new("/")).unwrap();
        let EngineAction::FrameTask(root) = engine.action().unwrap() else {
            panic!("root task expected")
        };
        let failure = engine
            .begin_view_report(root.identity())
            .unwrap()
            .failed_with_physical_residual(NavigationFailure::new(
                NavigationFailureKind::Apply,
                "partial root apply failed",
            ));

        assert!(failure.logical_roots().is_empty());
        assert!(engine.action().is_none());
        assert_eq!(
            engine.request(request).unwrap().status(),
            &RequestStatus::Running(NavigationPhase::CleanupPending)
        );
        let cleanup = engine
            .prepare_failure_cleanup(
                failure,
                CleanupManifestSeal::new(Vec::new(), manifest_digest(211), 3),
            )
            .unwrap();
        engine.complete_cleanup(valid_completion(&cleanup)).unwrap();

        assert_eq!(
            engine.request(request).unwrap().status(),
            &RequestStatus::Running(NavigationPhase::Recovering)
        );
        let Some(EngineAction::FrameTask(fallback)) = engine.action() else {
            panic!("cleanup completion must issue the fallback task")
        };
        assert_eq!(fallback.identity().origin(), &FrameOrigin::RouterFallback);
        let trace = engine.request(request).unwrap().trace();
        let settled = trace
            .iter()
            .position(|event| event.kind() == NavigationTraceEventKind::CleanupSettled)
            .unwrap();
        let fallback_issued = trace
            .iter()
            .rposition(|event| {
                event.kind() == NavigationTraceEventKind::FrameTaskIssued
                    && event
                        .frame()
                        .is_some_and(|frame| frame.origin() == &FrameOrigin::RouterFallback)
            })
            .unwrap();
        assert!(settled < fallback_issued);
    }

    #[test]
    fn fallback_partial_failure_waits_for_cleanup_before_recovery_terminal() {
        let mut engine = ready_engine();
        let request = engine.push(RouteLocation::new("/")).unwrap();
        let EngineAction::FrameTask(root) = engine.action().unwrap() else {
            panic!("root task expected")
        };
        engine
            .begin_view_report(root.identity())
            .unwrap()
            .failed(NavigationFailure::new(
                NavigationFailureKind::Apply,
                "normal root failed",
            ));
        let Some(EngineAction::FrameTask(fallback)) = engine.action() else {
            panic!("normal root failure must issue fallback")
        };
        let failure = engine
            .begin_view_report(fallback.identity())
            .unwrap()
            .failed_with_physical_residual(NavigationFailure::new(
                NavigationFailureKind::Apply,
                "partial fallback apply failed",
            ));

        assert!(failure.logical_roots().is_empty());
        assert!(matches!(
            engine.request(request).unwrap().status(),
            RequestStatus::Running(_)
        ));
        let cleanup = engine
            .prepare_failure_cleanup(
                failure,
                CleanupManifestSeal::new(Vec::new(), manifest_digest(223), 2),
            )
            .unwrap();
        assert!(matches!(
            engine.request(request).unwrap().status(),
            RequestStatus::Running(_)
        ));
        engine.complete_cleanup(valid_completion(&cleanup)).unwrap();

        assert!(matches!(
            engine.request(request).unwrap().status(),
            RequestStatus::Failed(failure)
                if failure.kind() == NavigationFailureKind::RecoveryFailed
        ));
        assert!(engine.action().is_none());
        assert_eq!(
            engine
                .request(request)
                .unwrap()
                .trace()
                .iter()
                .map(|event| event.kind())
                .rev()
                .take(2)
                .collect::<Vec<_>>(),
            vec![
                NavigationTraceEventKind::RequestTerminal,
                NavigationTraceEventKind::CleanupSettled,
            ]
        );
    }
}
