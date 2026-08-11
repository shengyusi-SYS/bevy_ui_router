#![allow(dead_code)]

use std::collections::VecDeque;

use crate::admission::{
    AdmissionError, HostAdmissionConfirmation, HostAdmissionIssuer, HostAdmissionToken, HostGrant,
    HostLease, RetiredHostAuthority, RetiredHostAuthorityProjection,
};
use crate::branch::{MatchedBranch, MatchedRoute, RouteParams};
use crate::cleanup::{CleanupBatchToken, CleanupCompletion, CleanupManifestSeal};
use crate::diagnostics::{DiagnosticEvent, ReportDisposition};
use crate::frame::{
    FrameOrigin, LogicalChildSlotBinding, ProjectedChildSlotRoleKind, ProjectedEnteringSlot,
    ReconciliationProjectionError, RouteFrameVersion, RouteReconciliationProjection,
};
use crate::history::MemoryHistory;
use crate::history_authority::{
    AcceptedBackReservation, BackAcceptedReservationView, BackAdmissionToken, BackDestination,
    BackNoTargetSourceDriftEvidence, BackNoTargetSourceDriftOwner,
    BackNoTargetSourceDriftTerminalEvidence, BackPreview, BackResolutionEvidence,
    BackResolutionEvidenceClassification, BackResolutionOwnerHandoff,
    BackResolutionScopeProjection, BackResolutionVerifyOutcome, BackTargetAuthorityLossCause,
    BackTargetPermanentOwner, BackTargetPermanentTerminalEvidence, BackTargetPreparationBaseline,
    CanonicalHistory, ClaimHistoryAdmissionWorldError, CoreFrozenHistorySourceProjection,
    CoreIssuedWorldSeal, CoreIssuedWorldSealProjection, EnqueueBackErrorKind,
    HistoryAdmissionStamp, HistoryAdmissionWorldSeal, HistoryCommitCapability, HistoryEffect,
    HistoryExecutionBinding, HistoryExecutionBindingProjection, HistoryIncarnation,
    HistoryPreparationError, HistoryRevision, HistorySourceDrift, NormalAttemptIdentity,
    PrepareBackAdmissionErrorKind, PreparedHistoryPublication, PreparedPresentationBinding,
    VerifiedBackNoTargetCandidate, VerifiedBackTargetExecutionOwner,
};
#[cfg(test)]
use crate::history_authority::{
    HistoryAuthoritySnapshot, HistoryCapabilitySnapshot, HistoryDriftEvidenceSnapshot,
};
use crate::ids::{
    CleanupBatchId, CoreIdAllocator, FrameTaskId, NavigationRequestId, ReservedBranchAttempt,
    ReservedFrameAttempt, RouteFrameId, RouteGeneration, RouteHostId, RouteSlotId, RouterIdentity,
    TransactionId,
};
use crate::location::RouteLocation;
use crate::matcher::RouteMatcher;
use crate::mount::{
    MountConfirmation, MountError, MountGrant, MountIssuer, MountIssuerContextDisposition,
    MountIssuerRequestProjection, MountReceiptAttestation, MountReceiptProof,
    MountRegistrationCompletion, MountRegistrationDigest, MountRegistrationPermit,
    MountRequestReceiverDisposition, MountRequestToken, MountRequestTokenProjection,
};
use crate::presentation::{ActivePresentation, ActivePresentationKind, RouterNavigationState};
use crate::request::{
    NavigationFailure, NavigationFailureKind, NavigationPhase, NavigationRequestRecord,
    RequestRecordError, RequestStatus,
};
use crate::route_tree::RouteTree;
use crate::trace::{MountRejectionKind, NavigationTraceEvent, NavigationTraceEventKind};
use crate::view_input::RouteViewInput;
use crate::view_key::RouteViewKey;
use crate::{HostVersion, slot::RouteSlotKey};

const TERMINAL_RETENTION: usize = 64;
const ROUTER_DIAGNOSTIC_RETENTION: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BackAllocatorBoundary {
    Attempt,
    Failure,
}

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
    RegisteredChildSlot {
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

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn testing_replace_top_slot_mount_target(
        &mut self,
        host: HostVersion,
        slot: RouteSlotId,
    ) -> bool {
        if !matches!(&self.target, FrameMountTarget::TopSlot { .. }) {
            return false;
        }
        self.target = FrameMountTarget::TopSlot { host, slot };
        true
    }

    #[cfg(any(test, feature = "test-support"))]
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

struct ValidatedAppliedReport {
    report: AppliedFrameReport,
    frame: RouteFrameVersion,
    next_outlet: Option<RouteSlotKey>,
}

enum AppliedReportTransition {
    MountRequest(FrameMountRequest),
    Commit(CommitTicket),
    FailureCleanup(FailureCleanupRequest),
}

struct AppliedReportPredicateProjection {
    next_outlet: Option<RouteSlotKey>,
    reconciliation: Option<RouteReconciliationProjection>,
    active_baseline: BackTargetPreparationBaseline,
}

enum AppliedReportRejection {
    Identity(ReportError),
    InvalidInput,
}

enum AppliedReportInspection {
    Rejected {
        report: AppliedFrameReport,
        rejection: AppliedReportRejection,
        predicates: AppliedReportPredicateProjection,
    },
    ReconciliationDrift,
    Validated(ValidatedAppliedReport),
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
    AuthoritySettlement,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BeginAuthoritySettlementErrorKind {
    Foreign,
    Stale,
    WrongPhase,
    InvalidManifest,
    RetryablePreparation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FailedAuthorityTriggerError {
    Foreign,
    Stale,
    WrongPhase,
    Exhausted,
}

#[derive(Debug)]
pub(crate) struct BeginAuthoritySettlementError {
    kind: BeginAuthoritySettlementErrorKind,
    owner: HostLease,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum FailedAuthoritySettlementTerminal {
    Task {
        identity: FrameTaskIdentity,
        current_roots: Vec<RouteFrameVersion>,
        target_and_current_roots: Option<Vec<RouteFrameVersion>>,
    },
    FailureCleanup(FailureCleanupRequest),
    RegistrationBegin {
        identity: FrameTaskIdentity,
        permit: MountRegistrationPermit,
        logical_roots: Vec<RouteFrameVersion>,
    },
}

#[derive(Debug)]
pub(crate) struct FailedAuthoritySettlementTrigger {
    router: RouterIdentity,
    revision: u64,
    terminal: FailedAuthoritySettlementTerminal,
}

#[cfg(test)]
impl FailedAuthoritySettlementTrigger {
    pub(crate) fn testing_duplicate(&self) -> Self {
        Self {
            router: self.router,
            revision: self.revision,
            terminal: self.terminal.clone(),
        }
    }
}

enum FailedAuthoritySettlementTerminalRef<'a> {
    Task(&'a FrameTaskIdentity),
    FailureCleanup(&'a FailureCleanupRequest),
    RegistrationBegin(&'a MountRegistrationPermit),
}

struct FailedAuthoritySettlementSource {
    identity: FrameTaskIdentity,
    logical_roots: FailedAuthoritySettlementLogicalRoots,
}

enum FailedAuthoritySettlementLogicalRoots {
    Exact(Vec<RouteFrameVersion>),
    Task {
        current: Vec<RouteFrameVersion>,
        target_and_current: Option<Vec<RouteFrameVersion>>,
    },
}

impl FailedAuthoritySettlementSource {
    fn select_logical_roots(
        self,
        seal: &CleanupManifestSeal,
    ) -> Option<(FrameTaskIdentity, Vec<RouteFrameVersion>)> {
        let selected = match self.logical_roots {
            FailedAuthoritySettlementLogicalRoots::Exact(roots) => {
                (roots.as_slice() == seal.logical_roots()).then_some(roots)
            }
            FailedAuthoritySettlementLogicalRoots::Task {
                current,
                target_and_current,
            } => {
                if current.as_slice() == seal.logical_roots() {
                    Some(current)
                } else {
                    target_and_current.filter(|roots| roots.as_slice() == seal.logical_roots())
                }
            }
        }?;
        Some((self.identity, selected))
    }
}

impl BeginAuthoritySettlementError {
    const fn new(kind: BeginAuthoritySettlementErrorKind, owner: HostLease) -> Self {
        Self { kind, owner }
    }

    pub(crate) const fn kind(&self) -> BeginAuthoritySettlementErrorKind {
        self.kind
    }

    pub(crate) fn into_owner(self) -> HostLease {
        self.owner
    }
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

    #[cfg(test)]
    pub(crate) fn testing_advance_token_binding(&mut self) {
        self.token.testing_advance_binding();
    }
}

pub(crate) enum EngineAction {
    FrameTask(FrameTask),
    MountRequest(FrameMountRequest),
    FailureCleanup(FailureCleanupRequest),
    Commit(CommitTicket),
    BackResolution(BackResolutionOwnerHandoff),
}

pub(crate) enum BackTargetResumeAction {
    FrameTask(FrameTask),
    MountRequest(FrameMountRequest),
    Commit(CommitTicket),
}

pub(crate) struct BackTargetResume {
    pub(crate) action: BackTargetResumeAction,
    pub(crate) execution: HistoryExecutionBindingProjection,
    pub(crate) continuation: BackResolutionScopeProjection,
}

pub(crate) enum BackTargetStart {
    Ready(BackTargetResume),
    FailureCleanup(FailureCleanupRequest),
}

pub(crate) enum BackTargetResumeOutcome {
    Resumed(BackTargetStart),
    Permanent(BackTargetPermanentOwner),
    Mismatch(VerifiedBackTargetExecutionOwner),
}

#[cfg(test)]
#[derive(Clone, Copy, Debug)]
pub(crate) enum BackTargetBusyReceiverSlot {
    Action,
    Current,
    PendingNoTargetBack,
    PendingBackNoTargetAuthority,
    CurrentHistoryCapability,
    PendingHistoryPreparationError,
    PreparedPublicationAuthority,
    CurrentRequest,
}

pub(crate) enum BackNoTargetSourceRefreezeOutcome {
    Unchanged(VerifiedBackNoTargetCandidate),
    Drift(BackNoTargetSourceDriftOwner),
    Mismatch {
        candidate: VerifiedBackNoTargetCandidate,
        evidence: BackResolutionEvidence,
    },
}

pub(crate) struct CoreProvenEmptyApprovalProjection {
    expected: CoreBackNoTargetLogicalZeroExpectedProjection,
}

pub(crate) struct ApprovedBackNoTargetCandidate {
    candidate: VerifiedBackNoTargetCandidate,
    approval: CoreProvenEmptyApprovalProjection,
}

impl ApprovedBackNoTargetCandidate {
    pub(crate) fn from_parts(
        candidate: VerifiedBackNoTargetCandidate,
        approval: CoreProvenEmptyApprovalProjection,
    ) -> Self {
        Self {
            candidate,
            approval,
        }
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        VerifiedBackNoTargetCandidate,
        CoreProvenEmptyApprovalProjection,
    ) {
        (self.candidate, self.approval)
    }
}

pub(crate) enum BackNoTargetApprovalOutcome {
    Approved(ApprovedBackNoTargetCandidate),
    Mismatch {
        candidate: VerifiedBackNoTargetCandidate,
        mismatch: CorePreparationReceiverMismatchProjection,
    },
}

pub(crate) struct CoreBackNoTargetCleanupBatchOwner {
    evidence: BackResolutionEvidence,
    host: HostVersion,
    manifest_digest: u64,
    obligation_count: usize,
    expected_completion_digest: u64,
}

impl CoreBackNoTargetCleanupBatchOwner {
    pub(crate) fn projection(&self) -> CoreBackNoTargetCleanupBatchProjection {
        CoreBackNoTargetCleanupBatchProjection {
            evidence: self.evidence.clone(),
            host: self.host.clone(),
            manifest_digest: self.manifest_digest,
            obligation_count: self.obligation_count,
            expected_completion_digest: self.expected_completion_digest,
        }
    }
}

pub(crate) struct CoreRejectedBackContinuationOwner {
    approved: ApprovedBackNoTargetCandidate,
}

pub(crate) struct CoreBackNoTargetCleanupAction {
    request: NavigationRequestId,
}

#[derive(Clone)]
pub(crate) struct CoreBackNoTargetCleanupBatchProjection {
    evidence: BackResolutionEvidence,
    host: HostVersion,
    manifest_digest: u64,
    obligation_count: usize,
    expected_completion_digest: u64,
}

impl CoreBackNoTargetCleanupBatchProjection {
    pub(crate) fn matches_resolution_evidence(&self, evidence: &BackResolutionEvidence) -> bool {
        &self.evidence == evidence
    }

    pub(crate) const fn manifest_digest(&self) -> u64 {
        self.manifest_digest
    }

    pub(crate) const fn obligation_count(&self) -> usize {
        self.obligation_count
    }

    pub(crate) const fn expected_completion_digest(&self) -> u64 {
        self.expected_completion_digest
    }
}

pub(crate) struct CoreBackNoTargetCleanupInstallation {
    batch: CoreBackNoTargetCleanupBatchOwner,
    continuation: CoreRejectedBackContinuationOwner,
    action: CoreBackNoTargetCleanupAction,
}

impl CoreBackNoTargetCleanupInstallation {
    pub(crate) fn from_parts(
        batch: CoreBackNoTargetCleanupBatchOwner,
        continuation: CoreRejectedBackContinuationOwner,
        action: CoreBackNoTargetCleanupAction,
    ) -> Self {
        Self {
            batch,
            continuation,
            action,
        }
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        CoreBackNoTargetCleanupBatchOwner,
        CoreRejectedBackContinuationOwner,
        CoreBackNoTargetCleanupAction,
    ) {
        (self.batch, self.continuation, self.action)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CoreBackNoTargetCleanupCompletionFacts {
    manifest_digest: u64,
    obligation_count: usize,
    completion_digest: u64,
    settled_count: usize,
}

pub(crate) struct CoreBackNoTargetCleanupCompletionOwner {
    installation: CoreBackNoTargetCleanupInstallation,
    completion: CoreBackNoTargetCleanupCompletionFacts,
}

impl CoreBackNoTargetCleanupCompletionOwner {
    pub(crate) fn new(
        installation: CoreBackNoTargetCleanupInstallation,
        manifest_digest: u64,
        obligation_count: usize,
        completion_digest: u64,
        settled_count: usize,
    ) -> Self {
        Self {
            installation,
            completion: CoreBackNoTargetCleanupCompletionFacts {
                manifest_digest,
                obligation_count,
                completion_digest,
                settled_count,
            },
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CoreBackNoTargetTerminalProjection {
    request: NavigationRequestId,
    completion: CoreBackNoTargetCleanupCompletionFacts,
}

pub(crate) enum BackNoTargetCleanupCompletionCoreOutcome {
    Terminal(CoreBackNoTargetTerminalProjection),
    Mismatch {
        owner: CoreBackNoTargetCleanupCompletionOwner,
        mismatch: CorePreparationReceiverMismatchProjection,
    },
}

pub(crate) struct CoreBackNoTargetPublicationCapability {
    records: VecDeque<NavigationRequestRecord>,
    state: RouterNavigationState,
}

pub(crate) struct CorePreparedBackNoTargetPublication {
    publication: CoreBackNoTargetPublicationCapability,
    installation: CoreBackNoTargetCleanupInstallation,
}

impl CorePreparedBackNoTargetPublication {
    pub(crate) fn batch_projection(&self) -> CoreBackNoTargetCleanupBatchProjection {
        self.installation.batch.projection()
    }
}

pub(crate) enum BackNoTargetReadyCoreOutcome {
    Prepared(CorePreparedBackNoTargetPublication),
    Mismatch {
        approved: ApprovedBackNoTargetCandidate,
        mismatch: CorePreparationReceiverMismatchProjection,
    },
}

pub(crate) struct BackNoTargetAuthoritySettlementOwner {
    candidate: VerifiedBackNoTargetCandidate,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CoreBackNoTargetAuthorityAccepted {
    receiver: BackResolutionEvidence,
    source: CoreFrozenHistorySourceProjection,
    host: HostVersion,
    top_slot: RouteSlotId,
    logical_roots: Vec<RouteFrameVersion>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CoreBackNoTargetAuthorityCleanupProjection {
    receiver: BackResolutionEvidence,
    source: CoreFrozenHistorySourceProjection,
    host: HostVersion,
    top_slot: RouteSlotId,
    request: NavigationRequestId,
    manifest_digest: u64,
    obligation_count: usize,
}

impl CoreBackNoTargetAuthorityCleanupProjection {
    pub(crate) const fn manifest_digest(&self) -> u64 {
        self.manifest_digest
    }

    pub(crate) const fn obligation_count(&self) -> usize {
        self.obligation_count
    }
}

pub(crate) struct CoreBackNoTargetAuthorityCleanupBatch {
    receiver: CoreBackNoTargetAuthorityCleanupProjection,
}

impl CoreBackNoTargetAuthorityCleanupBatch {
    pub(crate) fn projection(&self) -> CoreBackNoTargetAuthorityCleanupProjection {
        self.receiver.clone()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CoreBackNoTargetAuthorityCleanupCompletionFacts {
    manifest_digest: u64,
    obligation_count: usize,
    completion_digest: u64,
    settled_count: usize,
}

pub(crate) struct CoreBackNoTargetAuthorityCleanupCompletionOwner {
    batch: CoreBackNoTargetAuthorityCleanupBatch,
    completion: CoreBackNoTargetAuthorityCleanupCompletionFacts,
}

impl CoreBackNoTargetAuthorityCleanupCompletionOwner {
    pub(crate) fn new(
        batch: CoreBackNoTargetAuthorityCleanupBatch,
        manifest_digest: u64,
        obligation_count: usize,
        completion_digest: u64,
        settled_count: usize,
    ) -> Self {
        Self {
            batch,
            completion: CoreBackNoTargetAuthorityCleanupCompletionFacts {
                manifest_digest,
                obligation_count,
                completion_digest,
                settled_count,
            },
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CoreBackNoTargetAuthorityTerminalProjection {
    request: NavigationRequestId,
    completion: CoreBackNoTargetAuthorityCleanupCompletionFacts,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CoreBackNoTargetAuthorityCompletionFactsMismatchProjection {
    expected: CoreBackNoTargetAuthorityCleanupProjection,
    observed: CoreBackNoTargetAuthorityCleanupCompletionFacts,
}

pub(crate) enum CoreBackNoTargetAuthorityCleanupMismatchEvidence {
    Receiver(CorePreparationReceiverMismatchProjection),
    Facts(CoreBackNoTargetAuthorityCompletionFactsMismatchProjection),
}

pub(crate) enum BackNoTargetAuthorityCleanupCompletionCoreOutcome {
    Terminal(CoreBackNoTargetAuthorityTerminalProjection),
    Mismatch {
        owner: CoreBackNoTargetAuthorityCleanupCompletionOwner,
        evidence: CoreBackNoTargetAuthorityCleanupMismatchEvidence,
    },
}

pub(crate) struct CoreBackTargetPermanentCleanupBatch {
    expectation: CoreBackTargetPermanentCleanupCompletionExpectedProjection,
    token: CleanupBatchToken,
}

impl CoreBackTargetPermanentCleanupBatch {
    pub(crate) fn token(&self) -> &CleanupBatchToken {
        &self.token
    }

    pub(crate) const fn manifest_digest(&self) -> u64 {
        self.expectation.cleanup.manifest_digest
    }

    pub(crate) const fn obligation_count(&self) -> usize {
        self.expectation.cleanup.obligation_count
    }

    pub(crate) fn completion_owner(
        self,
        manifest_digest: u64,
        obligation_count: usize,
        completion_digest: u64,
        settled_count: usize,
    ) -> CoreBackTargetPermanentCleanupCompletionOwner {
        CoreBackTargetPermanentCleanupCompletionOwner {
            batch: self,
            completion: CoreBackTargetPermanentCleanupCompletionFacts {
                manifest_digest,
                obligation_count,
                completion_digest,
                settled_count,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CoreBackTargetPermanentCleanupCompletionFacts {
    manifest_digest: u64,
    obligation_count: usize,
    completion_digest: u64,
    settled_count: usize,
}

pub(crate) struct CoreBackTargetPermanentCleanupCompletionOwner {
    batch: CoreBackTargetPermanentCleanupBatch,
    completion: CoreBackTargetPermanentCleanupCompletionFacts,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CoreBackTargetPermanentCleanupCompletionExpectedProjection {
    cleanup: CoreBackTargetPermanentCleanupProjection,
    current: CoreBackTargetPermanentCurrentStateProjection,
    request: CoreBackTargetPermanentRequestProjection,
    resolution: BackResolutionEvidenceClassification,
}

impl CoreBackTargetPermanentCleanupCompletionExpectedProjection {
    fn matches_observed(
        &self,
        observed: &CoreBackTargetPermanentCleanupCompletionObservedProjection,
    ) -> bool {
        observed.cleanup.as_ref() == Some(&self.cleanup)
            && observed.expectation.as_ref() == Some(self)
            && self.cleanup.scope.matches_engine_scope(
                observed.scope.world,
                observed.scope.router,
                observed.scope.history,
            )
            && self.current == observed.current
            && observed.request.as_ref() == Some(&self.request)
            && observed.resolution == Some(self.resolution)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CoreBackTargetPermanentCleanupCompletionObservedProjection {
    cleanup: Option<CoreBackTargetPermanentCleanupProjection>,
    expectation: Option<CoreBackTargetPermanentCleanupCompletionExpectedProjection>,
    scope: CoreBackTargetEngineScopeProjection,
    current: CoreBackTargetPermanentCurrentStateProjection,
    request: Option<CoreBackTargetPermanentRequestProjection>,
    resolution: Option<BackResolutionEvidenceClassification>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CoreBackTargetPermanentCleanupReceiverMismatchProjection {
    expected: CoreBackTargetPermanentCleanupCompletionExpectedProjection,
    observed: CoreBackTargetPermanentCleanupCompletionObservedProjection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CoreBackTargetPermanentCompletionFactsMismatchProjection {
    expected: CoreBackTargetPermanentCleanupProjection,
    observed: CoreBackTargetPermanentCleanupCompletionFacts,
}

pub(crate) enum CoreBackTargetPermanentCleanupMismatchEvidence {
    Receiver(CoreBackTargetPermanentCleanupReceiverMismatchProjection),
    Facts(CoreBackTargetPermanentCompletionFactsMismatchProjection),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CoreBackTargetPermanentCleanupTerminalProjection {
    request: NavigationRequestId,
    completion: CoreBackTargetPermanentCleanupCompletionFacts,
}

pub(crate) enum BackTargetPermanentCleanupCompletionCoreOutcome {
    Terminal(CoreBackTargetPermanentCleanupTerminalProjection),
    Mismatch {
        owner: CoreBackTargetPermanentCleanupCompletionOwner,
        mismatch: CoreBackTargetPermanentCleanupMismatchEvidence,
    },
}

pub(crate) enum BackNoTargetAuthorityCoreOutcome {
    Accepted(BackNoTargetAuthoritySettlementOwner),
    Mismatch {
        candidate: VerifiedBackNoTargetCandidate,
        mismatch: CorePreparationReceiverMismatchProjection,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CoreBackNoTargetAuthorityBeginRetryEvidence {
    Capacity,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CoreBackNoTargetAuthorityBeginPermanentEvidence {
    HostManifestContradiction,
    PendingStateContradiction,
    HostRetirementRejected,
}

pub(crate) struct CoreBackNoTargetAuthorityBeginOwner {
    settlement: BackNoTargetAuthoritySettlementOwner,
    host: HostLease,
    manifest: CleanupManifestSeal,
    manifest_digest: u64,
}

impl CoreBackNoTargetAuthorityBeginOwner {
    pub(crate) fn new(
        settlement: BackNoTargetAuthoritySettlementOwner,
        host: HostLease,
        manifest: CleanupManifestSeal,
        manifest_digest: u64,
    ) -> Self {
        Self {
            settlement,
            host,
            manifest,
            manifest_digest,
        }
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        BackNoTargetAuthoritySettlementOwner,
        HostLease,
        CleanupManifestSeal,
        u64,
    ) {
        (
            self.settlement,
            self.host,
            self.manifest,
            self.manifest_digest,
        )
    }
}

pub(crate) enum BackNoTargetAuthorityBeginCoreOutcome {
    Begun,
    ReceiverMismatch {
        owner: CoreBackNoTargetAuthorityBeginOwner,
        mismatch: CorePreparationReceiverMismatchProjection,
    },
    Retryable {
        owner: CoreBackNoTargetAuthorityBeginOwner,
        evidence: CoreBackNoTargetAuthorityBeginRetryEvidence,
    },
    Permanent {
        owner: CoreBackNoTargetAuthorityBeginOwner,
        evidence: CoreBackNoTargetAuthorityBeginPermanentEvidence,
    },
}

pub(crate) enum BackTargetPermanentBeginCoreOutcome<Installation> {
    Begun,
    ReceiverMismatch {
        owner: CoreBackTargetPermanentBeginOwner,
        installation: Installation,
        mismatch: CorePreparationReceiverMismatchProjection,
    },
    LogicalRootsMismatch {
        owner: CoreBackTargetPermanentBeginOwner,
        installation: Installation,
    },
    Permanent {
        owner: CoreBackTargetPermanentBeginOwner,
        installation: Installation,
        evidence: BackTargetPermanentBeginCorePermanentEvidence,
    },
}

pub(crate) struct CoreBackTargetPermanentBeginOwner {
    settlement: CoreBackTargetPermanentSettlementOwner,
    host: HostLease,
}

impl CoreBackTargetPermanentBeginOwner {
    pub(crate) fn new(settlement: CoreBackTargetPermanentSettlementOwner, host: HostLease) -> Self {
        Self { settlement, host }
    }

    pub(crate) fn into_parts(self) -> (CoreBackTargetPermanentSettlementOwner, HostLease) {
        (self.settlement, self.host)
    }
}

pub(crate) struct CoreBackTargetPermanentSettlementOwner {
    scope: BackResolutionScopeProjection,
    expected_current: CoreBackTargetPermanentCurrentStateProjection,
    expected_request: Option<CoreBackTargetPermanentRequestProjection>,
    logical_roots: Vec<RouteFrameVersion>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum BackTargetPermanentBeginCorePermanentEvidence {
    CurrentStateContradiction {
        expected: CoreBackTargetPermanentCurrentStateProjection,
        observed: CoreBackTargetPermanentCurrentStateProjection,
    },
    CanonicalRelationContradiction {
        expected: CoreBackTargetPermanentCanonicalRelationProjection,
        observed: CoreBackTargetPermanentCanonicalRelationProjection,
    },
    RequestRecordContradiction {
        expected: Option<CoreBackTargetPermanentRequestProjection>,
        observed: Option<CoreBackTargetPermanentRequestProjection>,
    },
    TerminalPlanContradiction {
        source: CoreBackTargetPermanentTerminalSourceProjection,
        expected: CoreBackTargetPermanentTerminalExpectedProjection,
        observed: CoreBackTargetPermanentTerminalObservedProjection,
    },
    HostRetirementRejected,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CoreBackTargetPermanentCanonicalRelationProjection {
    request: Option<CoreBackTargetPermanentRequestProjection>,
    request_status: Option<RequestStatus>,
    location: Option<RouteLocation>,
    logical_roots: Vec<RouteFrameVersion>,
    cleanup_roots: Option<Vec<RouteFrameVersion>>,
    reconciliation: Option<RouteReconciliationProjection>,
    active: CoreBackTargetPermanentActiveRelationProjection,
    stage: CoreBackTargetReceiverStageProjection,
    availability: crate::presentation::RouterAvailability,
    retired_host: Option<RetiredHostAuthorityProjection>,
    pending_mount: Option<CoreBackTargetPendingMountProjection>,
    forbidden_pending: CoreBackTargetPermanentForbiddenPendingProjection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum CoreBackTargetPermanentActiveRelationProjection {
    Reconciliation(RouteReconciliationProjection),
    ReconciliationRequired,
    Active(BackTargetPreparationBaseline),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CoreBackTargetPermanentForbiddenPendingProjection {
    pending_no_target: Option<NavigationRequestId>,
    pending_no_target_authority: Option<CoreBackTargetPermanentNoTargetAuthorityProjection>,
    pending_target_cleanup: Option<CoreBackTargetPermanentCleanupProjection>,
    history_preparation_error: CoreBackTargetPermanentHistoryPreparationErrorProjection,
    prepared_publication: Option<SealedCommitEvidence>,
    action: CoreBackTargetReceiverActionProjection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CoreBackTargetPermanentCurrentStateProjection {
    pub(crate) host: Option<HostVersion>,
    pub(crate) top_slot: Option<RouteSlotId>,
    pub(crate) current_identity: Option<FrameTaskIdentity>,
    pub(crate) current_location: Option<RouteLocation>,
    pub(crate) current_stage: CoreBackTargetReceiverStageProjection,
    pub(crate) current_logical_roots: Option<Vec<RouteFrameVersion>>,
    pub(crate) current_reconciliation: Option<RouteReconciliationProjection>,
    pub(crate) current_retired_host: Option<RetiredHostAuthorityProjection>,
    pub(crate) active_baseline: BackTargetPreparationBaseline,
    pub(crate) availability: crate::presentation::RouterAvailability,
    pub(crate) request_status: Option<RequestStatus>,
    pub(crate) history_binding: Option<HistoryExecutionBindingProjection>,
    pub(crate) pending_no_target: Option<NavigationRequestId>,
    pub(crate) pending_no_target_authority:
        Option<CoreBackTargetPermanentNoTargetAuthorityProjection>,
    pub(crate) pending_target_cleanup: Option<CoreBackTargetPermanentCleanupProjection>,
    pub(crate) history_preparation_error: CoreBackTargetPermanentHistoryPreparationErrorProjection,
    pub(crate) prepared_publication: Option<SealedCommitEvidence>,
    pub(crate) action: CoreBackTargetReceiverActionProjection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CoreBackTargetPermanentNoTargetAuthorityProjection {
    Accepted {
        receiver: BackResolutionEvidence,
        source: CoreFrozenHistorySourceProjection,
        host: HostVersion,
        top_slot: RouteSlotId,
        logical_roots: Vec<RouteFrameVersion>,
    },
    Cleanup(CoreBackNoTargetAuthorityCleanupProjection),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CoreBackTargetPermanentHistoryPreparationErrorProjection {
    Vacant,
    SourceDrift,
    BindingDrift,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CoreBackTargetPermanentTerminalOperation {
    RecoveringTransition,
    RecoveringTrace,
    QueuedTerminalTransition,
    QueuedTerminalTrace,
    AuthorityRecovery,
    RecoveringRetention,
    CleanupSettledTrace,
    TerminalTransition,
    RequestTerminalTrace,
    AuthoritySettlement,
    TerminalRetention,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CoreBackTargetPermanentRequestProjection {
    pub(crate) request: NavigationRequestId,
    pub(crate) status: RequestStatus,
    pub(crate) requested: Option<RouteLocation>,
    pub(crate) trace_closed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CoreBackTargetPermanentAuthorityProjection {
    pub(crate) host: Option<HostVersion>,
    pub(crate) top_slot: Option<RouteSlotId>,
    pub(crate) availability: crate::presentation::RouterAvailability,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CoreBackTargetPermanentRetentionProjection {
    pub(crate) limit: usize,
    pub(crate) terminal_count: usize,
    pub(crate) records: Vec<CoreBackTargetPermanentRequestProjection>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CoreBackTargetPermanentTerminalSourceProjection {
    pub(crate) scope: BackResolutionScopeProjection,
    pub(crate) current: CoreBackTargetPermanentCurrentStateProjection,
    pub(crate) current_request: CoreBackTargetPermanentRequestProjection,
    pub(crate) queue: Vec<CoreBackTargetPermanentQueuedRequestProjection>,
    pub(crate) authority: CoreBackTargetPermanentAuthorityProjection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CoreBackTargetPermanentQueuedRequestProjection {
    pub(crate) expected: CoreBackTargetPermanentRequestProjection,
    pub(crate) observed: Option<CoreBackTargetPermanentRequestProjection>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CoreBackTargetPermanentTerminalExpectedProjection {
    Request {
        operation: CoreBackTargetPermanentTerminalOperation,
        request: CoreBackTargetPermanentRequestProjection,
    },
    Authority {
        operation: CoreBackTargetPermanentTerminalOperation,
        authority: CoreBackTargetPermanentAuthorityProjection,
    },
    Retention {
        operation: CoreBackTargetPermanentTerminalOperation,
        retention: CoreBackTargetPermanentRetentionProjection,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CoreBackTargetPermanentTerminalFailureProjection {
    MissingRequest,
    RequestRejected(RequestRecordError),
    AuthorityRecoveryRejected,
    AuthoritySettlementRejected,
    RetentionInvariant,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CoreBackTargetPermanentTerminalObservedProjection {
    Request {
        operation: CoreBackTargetPermanentTerminalOperation,
        request: Option<CoreBackTargetPermanentRequestProjection>,
        failure: CoreBackTargetPermanentTerminalFailureProjection,
    },
    Authority {
        operation: CoreBackTargetPermanentTerminalOperation,
        authority: CoreBackTargetPermanentAuthorityProjection,
        failure: CoreBackTargetPermanentTerminalFailureProjection,
    },
    Retention {
        operation: CoreBackTargetPermanentTerminalOperation,
        retention: CoreBackTargetPermanentRetentionProjection,
        failure: CoreBackTargetPermanentTerminalFailureProjection,
    },
}

#[cfg(any(test, feature = "test-support"))]
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct BackTargetPermanentBeginTestingProjection {
    pub(crate) cleanup_owner_count: usize,
    pub(crate) current_owner_count: usize,
    pub(crate) action_owner_count: usize,
    pub(crate) current_history_slot_count: usize,
    pub(crate) cleanup_history_owner_count: usize,
    pub(crate) cleanup_settlement_owner_count: usize,
    pub(crate) queue_len: usize,
    pub(crate) conflicting_owner_count: usize,
}

pub(crate) enum BackNoTargetSourceDriftSettlementOutcome {
    Settled(BackNoTargetSourceDriftTerminalEvidence),
    Mismatch {
        owner: BackNoTargetSourceDriftOwner,
        evidence: BackNoTargetSourceDriftEvidence,
    },
}

pub(crate) enum BackTargetPermanentSettlementOutcome {
    Settled(BackTargetPermanentTerminalEvidence),
    Mismatch {
        owner: BackTargetPermanentOwner,
        evidence: BackResolutionEvidence,
    },
}

enum StartReconciledRequestOutcome {
    Started(HistoryExecutionBindingProjection),
    BackSourceDrift {
        reservation: ReservedBranchAttempt,
        drift: HistorySourceDrift,
    },
}

#[derive(Debug)]
pub(crate) struct CommitTicket {
    identity: FrameTaskIdentity,
}

impl CommitTicket {
    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn testing_duplicate(&self) -> Self {
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

pub(crate) struct SealedCommit {
    identity: FrameTaskIdentity,
    cleanup: CleanupBatchToken,
    publication: PublicationPlan,
    evidence: SealedCommitEvidence,
    source: PreparedPublicationObservation,
}

impl SealedCommit {
    pub(crate) const fn cleanup(&self) -> &CleanupBatchToken {
        &self.cleanup
    }

    #[cfg(test)]
    pub(crate) fn evidence(&self) -> &SealedCommitEvidence {
        &self.evidence
    }
}

impl std::fmt::Debug for SealedCommit {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SealedCommit")
            .field("evidence", &self.evidence)
            .finish_non_exhaustive()
    }
}

struct PublicationPlan {
    state: RouterNavigationState,
    history: Option<PreparedHistoryPublication>,
    active_branch: Option<MatchedBranch>,
    active_child_slots: Vec<LogicalChildSlotBinding>,
    record_index: usize,
    published_record: NavigationRequestRecord,
    current: CurrentWork,
}

pub(crate) struct PreparedPublication {
    evidence: SealedCommitEvidence,
}

pub(crate) struct ValidatedPublication<'a> {
    engine: &'a mut RouterEngine,
    prepared: PreparedPublication,
}

impl ValidatedPublication<'_> {
    pub(crate) fn publish<F>(self, mutation: F)
    where
        F: FnOnce(),
    {
        let Self { engine, prepared } = self;
        drop(prepared);
        engine.publish_validated(mutation);
    }
}

pub(crate) struct CoreBackTargetCommitRetainedInputProjection {
    scope: BackResolutionScopeProjection,
    error: CommitError,
}

pub(crate) struct CoreBackTargetCommitFailureCleanup {
    continuation: BackResolutionScopeProjection,
    request: FailureCleanupRequest,
}

pub(crate) struct CoreBackTargetCommitRejectedPublication {
    continuation: BackResolutionScopeProjection,
    rejection: PreparedPublicationRejection,
}

pub(crate) struct CoreBackTargetDriverFailureCleanup {
    pub(crate) scope: BackResolutionScopeProjection,
    pub(crate) request: FailureCleanupRequest,
}

pub(crate) struct CoreBackTargetDriverFallback {
    pub(crate) scope: BackResolutionScopeProjection,
    pub(crate) action: FrameTaskIdentity,
    expected_attempt: CoreBackTargetReceiverAttemptProjection,
}

pub(crate) struct CoreBackTargetDriverTerminal {
    pub(crate) scope: BackResolutionScopeProjection,
    pub(crate) request: NavigationRequestId,
}

pub(crate) struct CoreBackTargetDriverTerminalProjection {
    pub(crate) scope: BackResolutionScopeProjection,
    pub(crate) request: NavigationRequestId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CoreBackTargetFailureCleanupRequestProjection {
    identity: FrameTaskIdentity,
    expected: PendingFailureKind,
    logical_roots: Vec<RouteFrameVersion>,
    failure: NavigationFailure,
    continuation: FailureCleanupContinuation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CoreBackTargetReceiverActionProjection {
    Missing,
    FrameTask(FrameTaskIdentity),
    MountRequest {
        token: MountRequestTokenProjection,
        request: NavigationRequestId,
        parent: RouteFrameVersion,
        outlet: RouteSlotKey,
    },
    FailureCleanup(CoreBackTargetFailureCleanupRequestProjection),
    Commit(FrameTaskIdentity),
    BackResolution,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CoreBackTargetReceiverAttemptProjection {
    Missing,
    NormalPush,
    NormalReplace,
    NormalBack,
    Fallback { original_failure: NavigationFailure },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CoreBackTargetReceiverStageProjection {
    Missing,
    Applying,
    AwaitingMount,
    AwaitingReceipt,
    ConfirmingMount,
    ReadyToCommit,
    CommitSealed,
    PostCommitCleanup,
    FailureCleanupManifestPending(CoreBackTargetFailureCleanupRequestProjection),
    FailureCleanupPrepared,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CoreBackTargetEngineScopeProjection {
    pub(crate) world: Option<CoreIssuedWorldSealProjection>,
    pub(crate) router: RouterIdentity,
    pub(crate) history: HistoryIncarnation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CoreBackTargetReservedFrameProjection {
    frame_task: FrameTaskId,
    frame: RouteFrameId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CoreBackTargetPendingMountProjection {
    child_index: usize,
    parent: RouteFrameVersion,
    outlet: RouteSlotKey,
    from_preserved_parent: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CoreBackTargetMountCurrentProjection {
    identity: FrameTaskIdentity,
    branch_routes: Option<Vec<MatchedRoute>>,
    reservation: Vec<CoreBackTargetReservedFrameProjection>,
    target_frames: Vec<RouteFrameVersion>,
    applied_frames: Vec<RouteFrameVersion>,
    current_target: FrameMountTarget,
    pending_mount: Option<CoreBackTargetPendingMountProjection>,
    stage: CoreBackTargetReceiverStageProjection,
    reconciliation: Option<RouteReconciliationProjection>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CoreBackTargetMountContextProjection {
    router: RouterIdentity,
    host: Option<HostVersion>,
    top_slot: Option<RouteSlotId>,
    current: Option<CoreBackTargetMountCurrentProjection>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CoreBackTargetMountContextDisposition {
    Matched,
    Mismatch,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CoreBackTargetMountObservedProjection {
    issuer: MountIssuerRequestProjection,
    context: CoreBackTargetMountContextProjection,
    receiver: MountRequestReceiverDisposition,
    engine_context: CoreBackTargetMountContextDisposition,
    issuer_context: MountIssuerContextDisposition,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CoreBackTargetExecutionExpectedProjection {
    pub(crate) scope: BackResolutionScopeProjection,
    pub(crate) execution: HistoryExecutionBindingProjection,
    pub(crate) physical: CoreBackTargetReceiverActionProjection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CoreBackTargetExecutionObservedProjection {
    scope: CoreBackTargetEngineScopeProjection,
    history: Option<HistoryExecutionBindingProjection>,
    pub(crate) current: Option<FrameTaskIdentity>,
    action: CoreBackTargetReceiverActionProjection,
    stage: CoreBackTargetReceiverStageProjection,
    mount: Option<CoreBackTargetMountObservedProjection>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CoreBackTargetContinuationExpectedProjection {
    pub(crate) scope: BackResolutionScopeProjection,
    pub(crate) commit: Option<FrameTaskIdentity>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CoreBackTargetContinuationObservedProjection {
    pub(crate) scope: CoreBackTargetEngineScopeProjection,
    pub(crate) history: Option<HistoryExecutionBindingProjection>,
    pub(crate) current: Option<FrameTaskIdentity>,
    pub(crate) action: CoreBackTargetReceiverActionProjection,
    pub(crate) stage: CoreBackTargetReceiverStageProjection,
}

struct CoreBackTargetExecutionInspection {
    expected: CoreBackTargetExecutionExpectedProjection,
    observed: CoreBackTargetExecutionObservedProjection,
    exact: bool,
}

impl CoreBackTargetExecutionInspection {
    fn into_exact(
        self,
    ) -> Result<
        (
            CoreBackTargetExecutionExpectedProjection,
            CoreBackTargetExecutionObservedProjection,
        ),
        CorePreparationReceiverMismatchProjection,
    > {
        if self.exact {
            Ok((self.expected, self.observed))
        } else {
            Err(CorePreparationReceiverMismatchProjection::Execution {
                expected: self.expected,
                observed: self.observed,
            })
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CoreBackTargetAppliedReportProjection {
    LegacyLeaf,
    WithOutlets(Vec<RouteSlotKey>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CoreBackTargetAppliedExpectedProjection {
    receiver: CoreBackTargetExecutionExpectedProjection,
    pub(crate) report: CoreBackTargetAppliedReportProjection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CoreBackTargetAppliedObservedProjection {
    receiver: CoreBackTargetExecutionObservedProjection,
    pub(crate) next_outlet: Option<RouteSlotKey>,
    reconciliation: Option<RouteReconciliationProjection>,
    active_baseline: BackTargetPreparationBaseline,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CoreBackTargetDriverFailureCleanupExpectedProjection {
    pub(crate) scope: BackResolutionScopeProjection,
    pub(crate) request: CoreBackTargetFailureCleanupRequestProjection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CoreBackTargetDriverFailureCleanupObservedProjection {
    scope: CoreBackTargetEngineScopeProjection,
    pub(crate) current: Option<FrameTaskIdentity>,
    pub(crate) stage: CoreBackTargetReceiverStageProjection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CoreBackTargetDriverFallbackExpectedProjection {
    pub(crate) scope: BackResolutionScopeProjection,
    pub(crate) action: CoreBackTargetReceiverActionProjection,
    pub(crate) attempt: CoreBackTargetReceiverAttemptProjection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CoreBackTargetDriverFallbackObservedProjection {
    scope: CoreBackTargetEngineScopeProjection,
    pub(crate) action: CoreBackTargetReceiverActionProjection,
    pub(crate) current: Option<FrameTaskIdentity>,
    pub(crate) attempt: CoreBackTargetReceiverAttemptProjection,
    pub(crate) stage: CoreBackTargetReceiverStageProjection,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CoreBackTargetDriverTerminalExpectedStatus {
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CoreBackTargetDriverTerminalExpectedProjection {
    pub(crate) scope: BackResolutionScopeProjection,
    pub(crate) router: RouterIdentity,
    pub(crate) request: NavigationRequestId,
    pub(crate) status: CoreBackTargetDriverTerminalExpectedStatus,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CoreBackTargetDriverTerminalObservedRequestProjection {
    Retained {
        request: NavigationRequestId,
        status: RequestStatus,
    },
    Evicted {
        request: NavigationRequestId,
        evicted_through: Option<u64>,
        highest_issued: Option<u64>,
    },
    Unknown {
        request: NavigationRequestId,
        evicted_through: Option<u64>,
        highest_issued: Option<u64>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CoreBackTargetDriverTerminalObservedProjection {
    scope: CoreBackTargetEngineScopeProjection,
    pub(crate) request: CoreBackTargetDriverTerminalObservedRequestProjection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CoreBackNoTargetLogicalZeroExpectedProjection {
    receiver: BackResolutionEvidence,
    source: CoreFrozenHistorySourceProjection,
    host: Option<HostVersion>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum CoreBackNoTargetLogicalZeroRequestProjection {
    Retained {
        request: NavigationRequestId,
        status: RequestStatus,
        requested: Option<RouteLocation>,
        resolution: BackResolutionEvidenceClassification,
    },
    Missing {
        request: NavigationRequestId,
    },
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum CoreBackNoTargetLogicalZeroPreparationErrorProjection {
    Vacant,
    Source,
    Binding,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct CoreBackNoTargetLogicalZeroObservedProjection {
    scope: CoreBackTargetEngineScopeProjection,
    source: CoreFrozenHistorySourceProjection,
    host: Option<HostVersion>,
    request: CoreBackNoTargetLogicalZeroRequestProjection,
    queued_requests: Vec<NavigationRequestId>,
    current: Option<FrameTaskIdentity>,
    action: CoreBackTargetReceiverActionProjection,
    pending_no_target_back: Option<NavigationRequestId>,
    current_history_capability: Option<HistoryExecutionBindingProjection>,
    pending_history_preparation_error: CoreBackNoTargetLogicalZeroPreparationErrorProjection,
    prepared_publication: Option<SealedCommitEvidence>,
    current_request: Option<RequestStatus>,
    preview: BackPreview,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CoreBackNoTargetManifestCommitmentProjection {
    pub(crate) manifest_digest: u64,
    pub(crate) obligation_count: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CoreBackNoTargetPublicationPreparationFailureProjection {
    MissingRecord,
    HistoryIntentRejectedTrace(RequestRecordError),
    CleanupPendingTransition(RequestRecordError),
    CleanupPendingTrace(RequestRecordError),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CoreBackNoTargetReadyMismatchProjection {
    Manifest {
        request: NavigationRequestId,
        expected: CoreBackNoTargetManifestCommitmentProjection,
        observed: CoreBackNoTargetManifestCommitmentProjection,
    },
    PublicationPreparation {
        request: NavigationRequestId,
        failure: CoreBackNoTargetPublicationPreparationFailureProjection,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CoreBackNoTargetCompletionExpectedProjection {
    receiver: BackResolutionEvidence,
    host: HostVersion,
    source: CoreFrozenHistorySourceProjection,
    request: NavigationRequestId,
    manifest: CoreBackNoTargetManifestCommitmentProjection,
    settled_count: usize,
    expected_completion_digest: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CoreBackNoTargetCompletionPreparationFailureProjection {
    MissingRecord,
    CleanupSettledTrace(RequestRecordError),
    TerminalTransition(RequestRecordError),
    RequestTerminalTrace(RequestRecordError),
    RetentionInvariant,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct CoreBackNoTargetCompletionObservedProjection {
    scope: CoreBackTargetEngineScopeProjection,
    current_host: Option<HostVersion>,
    current_source: CoreFrozenHistorySourceProjection,
    candidate_receiver: BackResolutionEvidence,
    approval_receiver: BackResolutionEvidence,
    approval_host: Option<HostVersion>,
    approval_source: CoreFrozenHistorySourceProjection,
    action_request: NavigationRequestId,
    request: CoreBackNoTargetLogicalZeroRequestProjection,
    current_request: Option<RequestStatus>,
    queued_requests: Vec<NavigationRequestId>,
    current: Option<FrameTaskIdentity>,
    action: CoreBackTargetReceiverActionProjection,
    pending_no_target_back: Option<NavigationRequestId>,
    current_history_capability: Option<HistoryExecutionBindingProjection>,
    pending_history_preparation_error: CoreBackNoTargetLogicalZeroPreparationErrorProjection,
    prepared_publication: Option<SealedCommitEvidence>,
    completion: CoreBackNoTargetCleanupCompletionFacts,
    preparation_failure: Option<CoreBackNoTargetCompletionPreparationFailureProjection>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum CorePreparationReceiverMismatchProjection {
    Scope {
        expected: BackResolutionScopeProjection,
        observed: BackResolutionScopeProjection,
    },
    LogicalZero {
        expected: CoreBackNoTargetLogicalZeroExpectedProjection,
        observed: CoreBackNoTargetLogicalZeroObservedProjection,
    },
    BackNoTargetReady(CoreBackNoTargetReadyMismatchProjection),
    BackNoTargetCompletion {
        expected: CoreBackNoTargetCompletionExpectedProjection,
        observed: CoreBackNoTargetCompletionObservedProjection,
    },
    BackNoTargetAuthorityBegin {
        expected: BackResolutionEvidence,
        observed: Option<BackResolutionEvidence>,
    },
    BackNoTargetAuthorityCompletion {
        expected: CoreBackNoTargetAuthorityCleanupProjection,
        observed: Option<CoreBackNoTargetAuthorityCleanupProjection>,
    },
    Continuation {
        expected: CoreBackTargetContinuationExpectedProjection,
        observed: CoreBackTargetContinuationObservedProjection,
    },
    Execution {
        expected: CoreBackTargetExecutionExpectedProjection,
        observed: CoreBackTargetExecutionObservedProjection,
    },
    Applied {
        expected: CoreBackTargetAppliedExpectedProjection,
        observed: CoreBackTargetAppliedObservedProjection,
    },
    DriverFailureCleanup {
        expected: CoreBackTargetDriverFailureCleanupExpectedProjection,
        observed: CoreBackTargetDriverFailureCleanupObservedProjection,
    },
    DriverFallback {
        expected: CoreBackTargetDriverFallbackExpectedProjection,
        observed: CoreBackTargetDriverFallbackObservedProjection,
    },
    DriverTerminal {
        expected: CoreBackTargetDriverTerminalExpectedProjection,
        observed: CoreBackTargetDriverTerminalObservedProjection,
    },
}

#[derive(Clone, Copy)]
pub(crate) enum CoreBackTargetPhysicalReceiverRef<'a> {
    FrameTask(&'a FrameTask),
    MountRequest(&'a FrameMountRequest),
    Commit(&'a CommitTicket),
}

pub(crate) enum CoreBackTargetDriverFailureReportMode {
    Normal,
    PhysicalResidual,
}

pub(crate) enum CoreBackTargetDriverFailureReport {
    FailureCleanup(CoreBackTargetDriverFailureCleanup),
    Fallback(CoreBackTargetDriverFallback),
    Terminal(CoreBackTargetDriverTerminal),
}

pub(crate) enum CoreBackTargetAppliedOutcome {
    MountRequest(FrameMountRequest),
    Commit(CommitTicket),
    FailureCleanup(CoreBackTargetDriverFailureCleanup),
}

pub(crate) enum BackTargetCommitOperationFailure {
    RetainedInput {
        continuation: BackResolutionScopeProjection,
        evidence: CoreBackTargetCommitRetainedInputProjection,
    },
    FailureCleanup(CoreBackTargetCommitFailureCleanup),
    RejectedPublication(CoreBackTargetCommitRejectedPublication),
}

pub(crate) enum BackTargetCommitOperationOutcome<'a> {
    Validated {
        publication: ValidatedPublication<'a>,
        cleanup: CleanupBatchToken,
        continuation: BackResolutionScopeProjection,
    },
    Failed(BackTargetCommitOperationFailure),
}

enum PreparedPublicationAuthority {
    Ready(SealedCommit),
    Rejected(PreparedPublicationRejection),
}

impl PreparedPublicationAuthority {
    const fn owner(&self) -> &SealedCommit {
        match self {
            Self::Ready(owner) => owner,
            Self::Rejected(rejection) => &rejection.owner,
        }
    }
}

struct PreparedPublicationRejection {
    owner: SealedCommit,
    error: CommitError,
    observed: PreparedPublicationObservation,
}

#[derive(Clone, Debug)]
struct PreparedPublicationObservation {
    identity: Option<FrameTaskIdentity>,
    stage: Option<WorkStage>,
    record: Option<NavigationRequestRecord>,
    history_revision: HistoryRevision,
    history_execution: Option<HistoryExecutionBindingProjection>,
    current_location: Option<RouteLocation>,
    current_branch: Option<MatchedBranch>,
    current_target_frames: Vec<RouteFrameVersion>,
    current_reconciliation: Option<RouteReconciliationProjection>,
    state: RouterNavigationState,
    host: Option<HostVersion>,
    top_slot: Option<RouteSlotId>,
    active: ActivePresentation,
    active_branch: Option<MatchedBranch>,
    active_child_slots: Vec<LogicalChildSlotBinding>,
}

impl PreparedPublicationObservation {
    fn branch_fingerprint(
        branch: Option<&MatchedBranch>,
    ) -> Option<&crate::fingerprint::RouteBranchFingerprint> {
        branch.map(MatchedBranch::branch_fingerprint)
    }

    fn same_source(&self, expected: &Self) -> bool {
        self.identity == expected.identity
            && self.stage == expected.stage
            && self.record == expected.record
            && self.history_revision == expected.history_revision
            && self.history_execution == expected.history_execution
            && self.current_location == expected.current_location
            && Self::branch_fingerprint(self.current_branch.as_ref())
                == Self::branch_fingerprint(expected.current_branch.as_ref())
            && self.current_target_frames == expected.current_target_frames
            && self.current_reconciliation == expected.current_reconciliation
            && self.state == expected.state
            && self.host == expected.host
            && self.top_slot == expected.top_slot
            && self.active == expected.active
            && Self::branch_fingerprint(self.active_branch.as_ref())
                == Self::branch_fingerprint(expected.active_branch.as_ref())
            && self.active_child_slots == expected.active_child_slots
    }

    fn accepts(&self, owner: &SealedCommit) -> bool {
        let history_matches = match (
            owner.publication.history.as_ref(),
            owner.evidence.history_execution.as_ref(),
            owner.evidence.source_revision,
        ) {
            (Some(history), Some(execution), Some(source_revision)) => {
                let Some(location) = self.current_location.as_ref() else {
                    return false;
                };
                let Some(branch) = self.current_branch.as_ref() else {
                    return false;
                };
                history.matches_sealed_projection(
                    execution,
                    source_revision,
                    location,
                    owner.publication.state.active(),
                    branch.branch_fingerprint(),
                    &self.current_target_frames,
                    &owner.cleanup,
                )
            }
            (None, None, None) => true,
            _ => false,
        };
        let reconciliation_matches = match owner.publication.current.reconciliation.as_ref() {
            Some(projection) => {
                let Some(host) = self.host.as_ref() else {
                    return false;
                };
                let Some(top_slot) = self.top_slot else {
                    return false;
                };
                projection.matches_active(
                    owner.identity.router,
                    host.host(),
                    host.epoch(),
                    top_slot,
                    self.active.location(),
                    self.active_branch.as_ref(),
                    self.active.frames(),
                    &self.active_child_slots,
                )
            }
            None => true,
        };
        self.identity.as_ref() == Some(&owner.identity)
            && matches!(
                self.stage.as_ref(),
                Some(WorkStage::CommitSealed { token }) if token == &owner.cleanup
            )
            && self.record.as_ref().map(NavigationRequestRecord::id) == Some(owner.identity.request)
            && owner
                .evidence
                .source_revision
                .is_none_or(|revision| self.history_revision == revision)
            && self.history_execution == owner.evidence.history_execution
            && history_matches
            && reconciliation_matches
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SealedCommitEvidence {
    identity: FrameTaskIdentity,
    cleanup_batch: crate::ids::CleanupBatchId,
    history_execution: Option<crate::history_authority::HistoryExecutionBindingProjection>,
    source_revision: Option<HistoryRevision>,
}

impl SealedCommitEvidence {
    #[cfg(test)]
    pub(crate) fn testing_with_observed_router_and_request(
        &self,
        router: RouterIdentity,
        request: NavigationRequestId,
    ) -> Self {
        let mut observed = self.clone();
        observed.identity.router = router;
        observed.identity.request = request;
        if let Some(execution) = observed.history_execution.as_mut() {
            *execution = execution.testing_with_router_and_request(router, request);
        }
        observed
    }
}

#[cfg(test)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ConsumedPublicationEvidence {
    evidence: SealedCommitEvidence,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EngineInstallError {
    IdExhausted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PushError {
    AwaitingHost,
    AuthorityRecovering,
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
    Normal(NavigationCommandKind),
    Fallback { original_failure: NavigationFailure },
}

#[derive(Debug)]
struct CurrentWork {
    identity: FrameTaskIdentity,
    branch: Option<MatchedBranch>,
    location: RouteLocation,
    reservation: Vec<ReservedFrameAttempt>,
    target_frames: Vec<RouteFrameVersion>,
    applied_frames: Vec<RouteFrameVersion>,
    current_target: FrameMountTarget,
    pending_mount: Option<PendingMount>,
    stage: WorkStage,
    cleanup_batch: crate::ids::CleanupBatchId,
    cleanup_roots: Vec<RouteFrameVersion>,
    attempt: AttemptKind,
    reconciliation: Option<RouteReconciliationProjection>,
    retired_host: Option<RetiredHostAuthority>,
}

impl CurrentWork {
    fn clone_pre_retirement(&self) -> Option<Self> {
        if self.retired_host.is_some() {
            return None;
        }
        Some(Self {
            identity: self.identity.clone(),
            branch: self.branch.clone(),
            location: self.location.clone(),
            reservation: self.reservation.clone(),
            target_frames: self.target_frames.clone(),
            applied_frames: self.applied_frames.clone(),
            current_target: self.current_target.clone(),
            pending_mount: self.pending_mount.clone(),
            stage: self.stage.clone(),
            cleanup_batch: self.cleanup_batch,
            cleanup_roots: self.cleanup_roots.clone(),
            attempt: self.attempt.clone(),
            reconciliation: self.reconciliation.clone(),
            retired_host: None,
        })
    }
}

#[derive(Clone, Debug)]
struct PendingMount {
    child_index: usize,
    parent: RouteFrameVersion,
    outlet: RouteSlotKey,
    from_preserved_parent: bool,
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

struct PreparedBackTargetInstallation {
    current: CurrentWork,
    records: VecDeque<NavigationRequestRecord>,
    state: RouterNavigationState,
    issued_view_tasks: Vec<FrameTaskIdentity>,
}

struct PreparedBackTargetReady {
    installation: PreparedBackTargetInstallation,
    history_capability: HistoryCommitCapability,
    action: BackTargetResumeAction,
    execution: HistoryExecutionBindingProjection,
    continuation: BackResolutionScopeProjection,
}

struct PreparedBackTargetFailureCleanup {
    installation: PreparedBackTargetInstallation,
    request: FailureCleanupRequest,
}

enum PreparedBackTargetStart {
    Ready(PreparedBackTargetReady),
    FailureCleanup(PreparedBackTargetFailureCleanup),
    Permanent(BackTargetPermanentOwner),
}

enum PreparedBackNoTargetSourceRefreeze {
    Unchanged(VerifiedBackNoTargetCandidate),
    Drift(BackNoTargetSourceDriftOwner),
}

struct PreparedBackNoTargetSourceDriftTerminal {
    state: RouterNavigationState,
    records: VecDeque<NavigationRequestRecord>,
    evicted_through: Option<u64>,
    issued_view_tasks: Vec<FrameTaskIdentity>,
    report_dispositions: Vec<ReportDispositionRecord>,
    completed_cleanup: Vec<CompletedCleanupTombstone>,
    terminal: BackNoTargetSourceDriftTerminalEvidence,
}

struct PreparedBackTargetPermanentTerminal {
    records: VecDeque<NavigationRequestRecord>,
    evicted_through: Option<u64>,
    issued_view_tasks: Vec<FrameTaskIdentity>,
    report_dispositions: Vec<ReportDispositionRecord>,
    completed_cleanup: Vec<CompletedCleanupTombstone>,
    terminal: BackTargetPermanentTerminalEvidence,
}

struct PreparedBackNoTargetCleanupTerminal {
    state: RouterNavigationState,
    records: VecDeque<NavigationRequestRecord>,
    evicted_through: Option<u64>,
    issued_view_tasks: Vec<FrameTaskIdentity>,
    report_dispositions: Vec<ReportDispositionRecord>,
    completed_cleanup: Vec<CompletedCleanupTombstone>,
    terminal: CoreBackNoTargetTerminalProjection,
}

struct CoreBackNoTargetAuthorityTerminalPlan {
    state: RouterNavigationState,
    records: VecDeque<NavigationRequestRecord>,
    evicted_through: Option<u64>,
    issued_view_tasks: Vec<FrameTaskIdentity>,
    report_dispositions: Vec<ReportDispositionRecord>,
    completed_cleanup: Vec<CompletedCleanupTombstone>,
}

struct CoreBackNoTargetAuthorityCleanupState {
    receiver: CoreBackNoTargetAuthorityCleanupProjection,
    retired_host: RetiredHostAuthority,
    terminal: CoreBackNoTargetAuthorityTerminalPlan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CoreBackTargetPermanentCleanupProjection {
    scope: BackResolutionScopeProjection,
    commit: FrameTaskIdentity,
    batch: CleanupBatchId,
    logical_roots: Vec<RouteFrameVersion>,
    manifest_digest: u64,
    obligation_count: usize,
}

struct CoreBackTargetPermanentTerminalPlan {
    state: RouterNavigationState,
    records: VecDeque<NavigationRequestRecord>,
    evicted_through: Option<u64>,
    issued_view_tasks: Vec<FrameTaskIdentity>,
    report_dispositions: Vec<ReportDispositionRecord>,
    completed_cleanup: Vec<CompletedCleanupTombstone>,
    active_child_slots: Vec<LogicalChildSlotBinding>,
}

struct CoreBackTargetPermanentCleanupState {
    receiver: CoreBackTargetPermanentCleanupProjection,
    expectation: CoreBackTargetPermanentCleanupCompletionExpectedProjection,
    history: Option<HistoryCommitCapability>,
    settlement: CoreBackTargetPermanentSettlementOwner,
    retired_host: RetiredHostAuthority,
    terminal: CoreBackTargetPermanentTerminalPlan,
}

enum CorePendingBackNoTargetAuthority {
    Accepted(CoreBackNoTargetAuthorityAccepted),
    Cleanup(CoreBackNoTargetAuthorityCleanupState),
}

#[derive(Clone, Debug)]
enum QueuedMatchOutcome {
    Matched(MatchedBranch),
    MatchFailed(NavigationFailure),
}

#[derive(Clone, Debug)]
enum NavigationCommand {
    Push(RouteLocation),
    Replace(RouteLocation),
    Back,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NavigationCommandKind {
    Push,
    Replace,
    Back,
}

impl NavigationCommand {
    const fn kind(&self) -> NavigationCommandKind {
        match self {
            Self::Push(_) => NavigationCommandKind::Push,
            Self::Replace(_) => NavigationCommandKind::Replace,
            Self::Back => NavigationCommandKind::Back,
        }
    }

    fn location(&self) -> Option<&RouteLocation> {
        match self {
            Self::Push(location) | Self::Replace(location) => Some(location),
            Self::Back => None,
        }
    }

    fn history_effect(&self) -> Option<HistoryEffect> {
        match self {
            Self::Push(location) => Some(HistoryEffect::Push(location.clone())),
            Self::Replace(location) => Some(HistoryEffect::Replace(location.clone())),
            Self::Back => None,
        }
    }
}

#[derive(Debug)]
enum QueuedReservation {
    Branch(ReservedBranchAttempt),
    RequestOnly(NavigationRequestId),
}

#[derive(Debug)]
struct QueuedNavigation {
    request: NavigationRequestId,
    command: NavigationCommand,
    outcome: Option<QueuedMatchOutcome>,
    reservation: QueuedReservation,
}

#[derive(Debug)]
struct PendingNoTargetBack {
    request: NavigationRequestId,
    #[allow(dead_code)]
    source: crate::history_authority::FrozenHistorySource,
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
    history: CanonicalHistory,
    history_admission_world: Option<CoreIssuedWorldSealProjection>,
    active_branch: Option<MatchedBranch>,
    active_child_slots: Vec<LogicalChildSlotBinding>,
    records: VecDeque<NavigationRequestRecord>,
    evicted_through: Option<u64>,
    highest_issued: Option<u64>,
    queue: VecDeque<QueuedNavigation>,
    current: Option<CurrentWork>,
    pending_no_target_back: Option<PendingNoTargetBack>,
    current_history_capability: Option<HistoryCommitCapability>,
    pending_history_preparation_error: Option<HistoryPreparationError>,
    prepared_publication_authority: Option<PreparedPublicationAuthority>,
    action: Option<EngineAction>,
    issued_view_tasks: Vec<FrameTaskIdentity>,
    mount: MountIssuer,
    report_dispositions: Vec<ReportDispositionRecord>,
    router_diagnostics: Vec<DiagnosticEvent>,
    completed_cleanup: Vec<CompletedCleanupTombstone>,
    next_failed_authority_trigger_revision: u64,
    live_failed_authority_trigger_revision: Option<u64>,
    pending_back_no_target_authority: Option<CorePendingBackNoTargetAuthority>,
    pending_back_target_permanent_cleanup: Option<CoreBackTargetPermanentCleanupState>,
    #[cfg(test)]
    testing_back_target_source_drift_before_freeze_once: bool,
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
            history: CanonicalHistory::new(router),
            history_admission_world: None,
            active_branch: None,
            active_child_slots: Vec::new(),
            records: VecDeque::new(),
            evicted_through: None,
            highest_issued: None,
            queue: VecDeque::new(),
            current: None,
            pending_no_target_back: None,
            current_history_capability: None,
            pending_history_preparation_error: None,
            prepared_publication_authority: None,
            action: None,
            issued_view_tasks: Vec::new(),
            mount: MountIssuer::new(router),
            report_dispositions: Vec::new(),
            router_diagnostics: Vec::new(),
            completed_cleanup: Vec::new(),
            next_failed_authority_trigger_revision: 1,
            live_failed_authority_trigger_revision: None,
            pending_back_no_target_authority: None,
            pending_back_target_permanent_cleanup: None,
            #[cfg(test)]
            testing_back_target_source_drift_before_freeze_once: false,
        })
    }

    pub(crate) fn request_host_admission(
        &mut self,
        issuer: &mut HostAdmissionIssuer,
    ) -> Result<HostAdmissionToken, AdmissionError> {
        if self.state.availability() != &crate::presentation::RouterAvailability::AwaitingHost
            || self.state.host().is_some()
        {
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

    pub(crate) fn begin_authority_settlement_retaining<F>(
        &mut self,
        issuer: &mut HostAdmissionIssuer,
        lease: HostLease,
        seal: CleanupManifestSeal,
        authority_mutation: F,
    ) -> Result<CleanupBatchToken, BeginAuthoritySettlementError>
    where
        F: FnOnce(&Self, CleanupBatchToken),
    {
        self.begin_authority_settlement_publication(issuer, lease, seal, None, authority_mutation)
    }

    pub(crate) fn failed_authority_trigger_for_task(
        &mut self,
        identity: &FrameTaskIdentity,
    ) -> Result<FailedAuthoritySettlementTrigger, FailedAuthorityTriggerError> {
        self.mint_failed_authority_trigger(FailedAuthoritySettlementTerminalRef::Task(identity))
    }

    pub(crate) fn failed_authority_trigger_for_cleanup(
        &mut self,
        request: &FailureCleanupRequest,
    ) -> Result<FailedAuthoritySettlementTrigger, FailedAuthorityTriggerError> {
        self.mint_failed_authority_trigger(FailedAuthoritySettlementTerminalRef::FailureCleanup(
            request,
        ))
    }

    pub(crate) fn failed_authority_trigger_for_registration_begin(
        &mut self,
        permit: &MountRegistrationPermit,
    ) -> Result<FailedAuthoritySettlementTrigger, FailedAuthorityTriggerError> {
        self.mint_failed_authority_trigger(FailedAuthoritySettlementTerminalRef::RegistrationBegin(
            permit,
        ))
    }

    fn mint_failed_authority_trigger(
        &mut self,
        terminal: FailedAuthoritySettlementTerminalRef<'_>,
    ) -> Result<FailedAuthoritySettlementTrigger, FailedAuthorityTriggerError> {
        let terminal = self.freeze_failed_authority_terminal(terminal)?;
        let revision = self.next_failed_authority_trigger_revision;
        self.next_failed_authority_trigger_revision = revision
            .checked_add(1)
            .ok_or(FailedAuthorityTriggerError::Exhausted)?;
        self.live_failed_authority_trigger_revision = Some(revision);
        Ok(FailedAuthoritySettlementTrigger {
            router: self.router,
            revision,
            terminal,
        })
    }

    fn freeze_failed_authority_terminal(
        &self,
        terminal: FailedAuthoritySettlementTerminalRef<'_>,
    ) -> Result<FailedAuthoritySettlementTerminal, FailedAuthorityTriggerError> {
        if self.state.availability() != &crate::presentation::RouterAvailability::Ready
            || self.state.host().is_none()
            || self.state.top_slot().is_none()
        {
            return Err(FailedAuthorityTriggerError::WrongPhase);
        }
        let Some(current) = self.current.as_ref() else {
            return Err(FailedAuthorityTriggerError::Stale);
        };
        if current.retired_host.is_some() {
            return Err(FailedAuthorityTriggerError::WrongPhase);
        }
        match terminal {
            FailedAuthoritySettlementTerminalRef::Task(identity) => {
                if identity.router_identity() != self.router {
                    return Err(FailedAuthorityTriggerError::Foreign);
                }
                if current.stage != WorkStage::Applying || current.identity != *identity {
                    return Err(FailedAuthorityTriggerError::Stale);
                }
                let current_roots = current
                    .applied_frames
                    .iter()
                    .rev()
                    .cloned()
                    .collect::<Vec<_>>();
                let task_target = RouteFrameVersion::new(
                    identity.router,
                    identity.host,
                    identity.slot,
                    identity.origin.clone(),
                    identity.frame,
                    identity.generation,
                );
                let target_and_current_roots = if current.applied_frames.contains(&task_target) {
                    None
                } else {
                    let mut roots = Vec::with_capacity(current_roots.len() + 1);
                    roots.push(task_target);
                    roots.extend(current_roots.iter().cloned());
                    Some(roots)
                };
                Ok(FailedAuthoritySettlementTerminal::Task {
                    identity: identity.clone(),
                    current_roots,
                    target_and_current_roots,
                })
            }
            FailedAuthoritySettlementTerminalRef::FailureCleanup(request) => {
                if request.identity.router_identity() != self.router {
                    return Err(FailedAuthorityTriggerError::Foreign);
                }
                if !matches!(
                    &current.stage,
                    WorkStage::FailureCleanupManifestPending { expected }
                        if expected == request
                ) {
                    return Err(FailedAuthorityTriggerError::Stale);
                }
                Ok(FailedAuthoritySettlementTerminal::FailureCleanup(
                    request.clone(),
                ))
            }
            FailedAuthoritySettlementTerminalRef::RegistrationBegin(permit) => {
                self.validate_mount_registration_rejection(permit).map_err(
                    |error| match error {
                        MountError::Foreign => FailedAuthorityTriggerError::Foreign,
                        MountError::Stale => FailedAuthorityTriggerError::Stale,
                        MountError::WrongPhase => FailedAuthorityTriggerError::WrongPhase,
                        MountError::Exhausted => FailedAuthorityTriggerError::Exhausted,
                    },
                )?;
                Ok(FailedAuthoritySettlementTerminal::RegistrationBegin {
                    identity: current.identity.clone(),
                    permit: permit.clone(),
                    logical_roots: current.applied_frames.iter().rev().cloned().collect(),
                })
            }
        }
    }

    pub(crate) fn begin_failed_authority_settlement_retaining<F>(
        &mut self,
        issuer: &mut HostAdmissionIssuer,
        lease: HostLease,
        trigger: FailedAuthoritySettlementTrigger,
        seal: CleanupManifestSeal,
        authority_mutation: F,
    ) -> Result<CleanupBatchToken, BeginAuthoritySettlementError>
    where
        F: FnOnce(&Self, CleanupBatchToken),
    {
        let source = match self.consume_failed_authority_trigger(trigger) {
            Ok(source) => source,
            Err(kind) => return Err(BeginAuthoritySettlementError::new(kind, lease)),
        };
        self.begin_authority_settlement_publication(
            issuer,
            lease,
            seal,
            Some(source),
            authority_mutation,
        )
    }

    fn consume_failed_authority_trigger(
        &mut self,
        trigger: FailedAuthoritySettlementTrigger,
    ) -> Result<FailedAuthoritySettlementSource, BeginAuthoritySettlementErrorKind> {
        if trigger.router != self.router {
            return Err(BeginAuthoritySettlementErrorKind::Foreign);
        }
        if self.live_failed_authority_trigger_revision != Some(trigger.revision) {
            return Err(BeginAuthoritySettlementErrorKind::Stale);
        }
        self.live_failed_authority_trigger_revision = None;
        let terminal_ref = match &trigger.terminal {
            FailedAuthoritySettlementTerminal::Task { identity, .. } => {
                FailedAuthoritySettlementTerminalRef::Task(identity)
            }
            FailedAuthoritySettlementTerminal::FailureCleanup(request) => {
                FailedAuthoritySettlementTerminalRef::FailureCleanup(request)
            }
            FailedAuthoritySettlementTerminal::RegistrationBegin { permit, .. } => {
                FailedAuthoritySettlementTerminalRef::RegistrationBegin(permit)
            }
        };
        let refrozen = self
            .freeze_failed_authority_terminal(terminal_ref)
            .map_err(|error| match error {
                FailedAuthorityTriggerError::Foreign => BeginAuthoritySettlementErrorKind::Foreign,
                FailedAuthorityTriggerError::Stale => BeginAuthoritySettlementErrorKind::Stale,
                FailedAuthorityTriggerError::WrongPhase => {
                    BeginAuthoritySettlementErrorKind::WrongPhase
                }
                FailedAuthorityTriggerError::Exhausted => {
                    BeginAuthoritySettlementErrorKind::RetryablePreparation
                }
            })?;
        if refrozen != trigger.terminal {
            return Err(BeginAuthoritySettlementErrorKind::Stale);
        }
        let (identity, logical_roots) = match trigger.terminal {
            FailedAuthoritySettlementTerminal::Task {
                identity,
                current_roots,
                target_and_current_roots,
            } => (
                identity,
                FailedAuthoritySettlementLogicalRoots::Task {
                    current: current_roots,
                    target_and_current: target_and_current_roots,
                },
            ),
            FailedAuthoritySettlementTerminal::RegistrationBegin {
                identity,
                logical_roots,
                ..
            } => (
                identity,
                FailedAuthoritySettlementLogicalRoots::Exact(logical_roots),
            ),
            FailedAuthoritySettlementTerminal::FailureCleanup(request) => (
                request.identity,
                FailedAuthoritySettlementLogicalRoots::Exact(request.logical_roots),
            ),
        };
        Ok(FailedAuthoritySettlementSource {
            identity,
            logical_roots,
        })
    }

    fn begin_authority_settlement_publication<F>(
        &mut self,
        issuer: &mut HostAdmissionIssuer,
        lease: HostLease,
        seal: CleanupManifestSeal,
        failed: Option<FailedAuthoritySettlementSource>,
        authority_mutation: F,
    ) -> Result<CleanupBatchToken, BeginAuthoritySettlementError>
    where
        F: FnOnce(&Self, CleanupBatchToken),
    {
        let Some(version) = self.state.host().cloned() else {
            return Err(BeginAuthoritySettlementError::new(
                BeginAuthoritySettlementErrorKind::Stale,
                lease,
            ));
        };
        let Some(top_slot) = self.state.top_slot() else {
            return Err(BeginAuthoritySettlementError::new(
                BeginAuthoritySettlementErrorKind::Stale,
                lease,
            ));
        };
        if !lease.matches(self.router, &version, top_slot) {
            return Err(BeginAuthoritySettlementError::new(
                BeginAuthoritySettlementErrorKind::Foreign,
                lease,
            ));
        }
        let valid_common_phase = self.current.as_ref().is_some_and(|current| {
            current.reconciliation.is_some() && current.retired_host.is_none()
        }) && self.action.is_none()
            && self.current_reconciliation_matches_active();
        let valid_source_phase = match &failed {
            Some(source) => self
                .current
                .as_ref()
                .is_some_and(|current| current.identity == source.identity),
            None => self.current.as_ref().is_some_and(|current| {
                matches!(
                    current.stage,
                    WorkStage::Applying | WorkStage::ReadyToCommit | WorkStage::CommitSealed { .. }
                )
            }),
        };
        if !valid_common_phase || !valid_source_phase {
            return Err(BeginAuthoritySettlementError::new(
                BeginAuthoritySettlementErrorKind::WrongPhase,
                lease,
            ));
        }

        let (current_identity, logical_roots) = match failed {
            Some(source) => {
                let Some((identity, logical_roots)) = source.select_logical_roots(&seal) else {
                    return Err(BeginAuthoritySettlementError::new(
                        BeginAuthoritySettlementErrorKind::InvalidManifest,
                        lease,
                    ));
                };
                (identity, logical_roots)
            }
            None => {
                let identity = self
                    .current
                    .as_ref()
                    .expect("authority settlement phase was preflighted")
                    .identity
                    .clone();
                (identity, self.authority_settlement_logical_roots())
            }
        };
        let failure = NavigationFailure::new(
            NavigationFailureKind::AuthorityLost,
            "physical reconciliation authority was lost",
        );
        let request = FailureCleanupRequest {
            identity: current_identity,
            expected: PendingFailureKind::AuthorityLost,
            logical_roots,
            failure,
            continuation: FailureCleanupContinuation::AuthoritySettlement,
        };
        if request.logical_roots != seal.logical_roots() {
            return Err(BeginAuthoritySettlementError::new(
                BeginAuthoritySettlementErrorKind::InvalidManifest,
                lease,
            ));
        }
        let cleanup = CleanupBatchToken::sealed(
            request.identity.clone(),
            self.current
                .as_ref()
                .expect("authority settlement phase was preflighted")
                .cleanup_batch,
            &request.logical_roots,
            seal.manifest_digest(),
            seal.obligation_count(),
        );
        let mut recovering_record = self
            .request(request.request())
            .expect("authority settlement request remains retained")
            .clone();
        recovering_record
            .transition(RequestStatus::Running(NavigationPhase::Recovering))
            .expect("authority settlement request is running");
        recovering_record
            .append_trace(NavigationTraceEvent::request_phase_changed(
                NavigationPhase::Recovering,
            ))
            .expect("authority settlement trace is open");
        let mut queued_records = Vec::new();
        if queued_records.try_reserve_exact(self.queue.len()).is_err() {
            return Err(BeginAuthoritySettlementError::new(
                BeginAuthoritySettlementErrorKind::RetryablePreparation,
                lease,
            ));
        }
        for queued in &self.queue {
            let mut record = self
                .request(queued.request)
                .expect("queued authority-loss request remains retained")
                .clone();
            record
                .transition(RequestStatus::Failed(NavigationFailure::new(
                    NavigationFailureKind::AuthorityLost,
                    "router authority was lost before request start",
                )))
                .expect("queued request is not terminal");
            record
                .append_trace(NavigationTraceEvent::request_terminal())
                .expect("queued failure terminal event is last");
            queued_records.push((queued.request, record));
        }
        let retired = match issuer.retire_current(lease) {
            Ok(retired) => retired,
            Err(error) => {
                return Err(BeginAuthoritySettlementError::new(
                    BeginAuthoritySettlementErrorKind::Stale,
                    error.into_owner(),
                ));
            }
        };
        assert!(retired.matches(self.router, &version, top_slot));
        authority_mutation(self, cleanup.clone());
        assert!(self.state.begin_authority_recovery(&version, top_slot));
        let current = self
            .current
            .as_mut()
            .expect("authority settlement phase was preflighted");
        current.pending_mount = None;
        current.retired_host = Some(retired);
        current.stage = WorkStage::FailureCleanupPrepared {
            outstanding: cleanup.clone(),
            failure: request.failure.clone(),
            continuation: FailureCleanupContinuation::AuthoritySettlement,
        };
        self.action = None;
        *self
            .record_mut(request.request())
            .expect("authority settlement request remains retained") = recovering_record;
        for (queued, record) in queued_records {
            *self
                .record_mut(queued)
                .expect("queued authority-loss request remains retained") = record;
        }
        self.queue.clear();
        self.state
            .set_request(Some(RequestStatus::Running(NavigationPhase::Recovering)));
        self.retain_terminal_records();
        self.retire_history_authority();
        Ok(cleanup)
    }

    pub(crate) fn fail_closed_authority_protocol_retaining<F>(
        &mut self,
        issuer: &mut HostAdmissionIssuer,
        lease: Option<HostLease>,
        authority_mutation: F,
    ) -> Result<(), Option<HostLease>>
    where
        F: FnOnce(),
    {
        let mut owner = lease;
        if self.fail_closed_authority_protocol(|| match owner.take() {
            Some(lease) => match issuer.retire_current(lease) {
                Ok(retired) => {
                    authority_mutation();
                    drop(retired);
                    true
                }
                Err(error) => {
                    owner = Some(error.into_owner());
                    false
                }
            },
            None => {
                authority_mutation();
                true
            }
        }) {
            Ok(())
        } else {
            Err(owner)
        }
    }

    fn fail_closed_authority_protocol<F>(&mut self, authority_mutation: F) -> bool
    where
        F: FnOnce() -> bool,
    {
        let Some(request) = self
            .current
            .as_ref()
            .map(|current| current.identity.request)
        else {
            return false;
        };
        let failure = NavigationFailure::new(
            NavigationFailureKind::AuthorityLost,
            "authority settlement protocol invariant was violated",
        );
        let mut terminal = self
            .request(request)
            .expect("current protocol-failure request remains retained")
            .clone();
        if terminal
            .transition(RequestStatus::Failed(failure.clone()))
            .is_err()
            || terminal
                .append_trace(NavigationTraceEvent::request_terminal())
                .is_err()
        {
            return false;
        }
        let mut queued_records = Vec::new();
        if queued_records.try_reserve_exact(self.queue.len()).is_err() {
            return false;
        }
        for queued in &self.queue {
            let mut record = self
                .request(queued.request)
                .expect("queued protocol-failure request remains retained")
                .clone();
            record
                .transition(RequestStatus::Failed(failure.clone()))
                .expect("queued protocol-failure request is not terminal");
            record
                .append_trace(NavigationTraceEvent::request_terminal())
                .expect("queued protocol-failure terminal event is last");
            queued_records.push((queued.request, record));
        }

        if !authority_mutation() {
            return false;
        }
        self.retire_history_authority();
        *self
            .record_mut(request)
            .expect("current protocol-failure request remains retained") = terminal;
        for (queued, record) in queued_records {
            *self
                .record_mut(queued)
                .expect("queued protocol-failure request remains retained") = record;
        }
        self.queue.clear();
        self.current = None;
        self.action = None;
        self.active_branch = None;
        self.active_child_slots.clear();
        self.state.fail_closed_unavailable(
            crate::presentation::RouterUnavailableReason::RegistryInvariantViolation,
        );
        self.retain_terminal_records();
        true
    }

    pub(crate) fn push(
        &mut self,
        location: RouteLocation,
    ) -> Result<NavigationRequestId, PushError> {
        let request = self.enqueue_push(location)?;
        self.collect();
        Ok(request)
    }

    pub(crate) fn replace(
        &mut self,
        location: RouteLocation,
    ) -> Result<NavigationRequestId, PushError> {
        let request = self.enqueue_replace(location)?;
        self.collect();
        Ok(request)
    }

    pub(crate) fn enqueue_push(
        &mut self,
        location: RouteLocation,
    ) -> Result<NavigationRequestId, PushError> {
        self.enqueue_navigation(NavigationCommand::Push(location))
    }

    pub(crate) fn enqueue_replace(
        &mut self,
        location: RouteLocation,
    ) -> Result<NavigationRequestId, PushError> {
        self.enqueue_navigation(NavigationCommand::Replace(location))
    }

    fn enqueue_navigation(
        &mut self,
        command: NavigationCommand,
    ) -> Result<NavigationRequestId, PushError> {
        if self.state.availability()
            == &crate::presentation::RouterAvailability::RecoveringAuthority
            || self.pending_back_no_target_authority.is_some()
        {
            return Err(PushError::AuthorityRecovering);
        }
        if self.state.host().is_none() {
            return Err(PushError::AwaitingHost);
        }
        let location = command
            .location()
            .expect("push and replace carry an enqueue-time target")
            .clone();
        let matched = self.matcher.match_location(&location);
        let (request, outcome, reservation) = match matched {
            Ok(branch) => {
                let reserved = self
                    .ids
                    .reserve_navigation_branch(self.router, branch.routes().len())
                    .map_err(|_| PushError::IdExhausted)?;
                (
                    reserved.request(),
                    Some(QueuedMatchOutcome::Matched(branch)),
                    QueuedReservation::Branch(reserved),
                )
            }
            Err(error) => {
                let request = self
                    .ids
                    .allocate_request(self.router)
                    .map_err(|_| PushError::IdExhausted)?;
                (
                    request,
                    Some(QueuedMatchOutcome::MatchFailed(NavigationFailure::new(
                        NavigationFailureKind::Match,
                        error.redacted_message(),
                    ))),
                    QueuedReservation::RequestOnly(request),
                )
            }
        };
        self.records
            .push_back(NavigationRequestRecord::with_resolved_target(
                request, location,
            ));
        self.highest_issued = Some(request.sequence());
        self.queue.push_back(QueuedNavigation {
            request,
            command,
            outcome,
            reservation,
        });
        Ok(request)
    }

    pub(crate) fn claim_history_admission_world(
        &mut self,
    ) -> Result<HistoryAdmissionWorldSeal, ClaimHistoryAdmissionWorldError> {
        if self.history_admission_world.is_some() {
            return Err(ClaimHistoryAdmissionWorldError::AlreadyClaimed);
        }
        let seal = CoreIssuedWorldSeal::issue(self.router, self.history.incarnation())?;
        self.history_admission_world = Some(seal.projection());
        Ok(HistoryAdmissionWorldSeal::issued(seal))
    }

    pub(crate) fn history_admission_stamp(
        &self,
        world: &HistoryAdmissionWorldSeal,
    ) -> Result<HistoryAdmissionStamp, PrepareBackAdmissionErrorKind> {
        let Some(issued) = self.history_admission_world else {
            return Err(PrepareBackAdmissionErrorKind::ForeignStamp);
        };
        if world.projection() != issued {
            return Err(PrepareBackAdmissionErrorKind::ForeignStamp);
        }
        Ok(HistoryAdmissionStamp::issued(
            issued,
            self.router,
            self.history.incarnation(),
            self.history.revision(),
            self.history.snapshot().can_back(),
        ))
    }

    pub(crate) fn prepare_back_admission(
        &self,
        current_world: &HistoryAdmissionWorldSeal,
        stamp: &HistoryAdmissionStamp,
    ) -> Result<BackAdmissionToken, PrepareBackAdmissionErrorKind> {
        let Some(issued) = self.history_admission_world else {
            return Err(PrepareBackAdmissionErrorKind::ForeignStamp);
        };
        if current_world.projection() != issued
            || !stamp.matches(issued, self.router, self.history.incarnation())
        {
            return Err(PrepareBackAdmissionErrorKind::ForeignStamp);
        }
        let _published_revision = stamp.published_revision();
        if self.state.availability() != &crate::presentation::RouterAvailability::Ready
            || self.pending_back_no_target_authority.is_some()
        {
            return Err(PrepareBackAdmissionErrorKind::AuthorityRecovering);
        }
        if !stamp.can_back() {
            return Err(PrepareBackAdmissionErrorKind::NoBackEntry);
        }
        Ok(BackAdmissionToken::issued(stamp, self.history.revision()))
    }

    pub(crate) fn enqueue_back(
        &mut self,
        admission: BackAdmissionToken,
    ) -> Result<AcceptedBackReservation, EnqueueBackErrorKind> {
        self.enqueue_back_observed(admission, |_| {})
    }

    pub(crate) fn enqueue_back_observed<F>(
        &mut self,
        admission: BackAdmissionToken,
        mut observe: F,
    ) -> Result<AcceptedBackReservation, EnqueueBackErrorKind>
    where
        F: FnMut(BackAllocatorBoundary),
    {
        let Some(issued) = self.history_admission_world else {
            return Err(EnqueueBackErrorKind::AuthorityRecovering);
        };
        if self.state.availability() != &crate::presentation::RouterAvailability::Ready
            || self.pending_back_no_target_authority.is_some()
            || !admission.matches(
                issued,
                self.router,
                self.history.incarnation(),
                self.history.revision(),
            )
        {
            return Err(EnqueueBackErrorKind::AuthorityRecovering);
        }
        let maximum_depth = self.matcher.max_branch_depth();
        debug_assert!(maximum_depth > 0, "an installed route tree has one root");
        observe(BackAllocatorBoundary::Attempt);
        let reservation = match self
            .ids
            .reserve_navigation_branch(self.router, maximum_depth)
        {
            Ok(reservation) => reservation,
            Err(_) => {
                observe(BackAllocatorBoundary::Failure);
                return Err(EnqueueBackErrorKind::Exhausted);
            }
        };
        let request = reservation.request();
        let accepted = AcceptedBackReservation::accepted(
            issued,
            self.router,
            self.history.incarnation(),
            &reservation,
        );
        self.records
            .push_back(NavigationRequestRecord::unresolved_back(request));
        self.highest_issued = Some(request.sequence());
        self.queue.push_back(QueuedNavigation {
            request,
            command: NavigationCommand::Back,
            outcome: None,
            reservation: QueuedReservation::Branch(reservation),
        });
        Ok(accepted)
    }

    pub(crate) fn verify_back_resolution_handoff(
        &mut self,
        accepted: BackAcceptedReservationView,
        handoff: BackResolutionOwnerHandoff,
    ) -> BackResolutionVerifyOutcome {
        let observed = handoff.evidence();
        let request = observed.request();
        let exact_lineage = accepted.matches(
            self.history_admission_world,
            self.router,
            self.history.incarnation(),
            handoff.scope_projection(),
        );
        let exact_record = exact_lineage
            && self.request(request).is_some_and(|record| {
                record.classify_back_resolution(&observed)
                    == BackResolutionEvidenceClassification::Current
            });
        if exact_record {
            self.record_mut(request)
                .expect("exact retained Back evidence was checked before consumption")
                .consume_back_resolution(&observed)
                .expect("exact current Back evidence is consumed once");
            return BackResolutionVerifyOutcome::Matched(handoff.into_verified(accepted));
        }
        let evidence = BackResolutionEvidence::mismatch(&accepted, &handoff);
        BackResolutionVerifyOutcome::Mismatch {
            accepted,
            handoff,
            evidence,
        }
    }

    pub(crate) fn classify_back_resolution_evidence(
        &self,
        evidence: &BackResolutionEvidence,
    ) -> BackResolutionEvidenceClassification {
        if !evidence.matches_engine_scope(
            self.history_admission_world,
            self.router,
            self.history.incarnation(),
        ) {
            return BackResolutionEvidenceClassification::Foreign;
        }
        let request = evidence.request();
        if let Some(record) = self.request(request) {
            return record.classify_back_resolution(evidence);
        }
        let sequence = request.sequence();
        if request.router_identity() == self.router
            && self
                .evicted_through
                .is_some_and(|evicted| sequence <= evicted)
            && self
                .highest_issued
                .is_some_and(|highest| sequence <= highest)
        {
            BackResolutionEvidenceClassification::ConsumedReplay
        } else {
            BackResolutionEvidenceClassification::Foreign
        }
    }

    pub(crate) fn resume_back_target_with_retirement<F>(
        &mut self,
        owner: VerifiedBackTargetExecutionOwner,
        commit_retirement: F,
    ) -> BackTargetResumeOutcome
    where
        F: FnOnce(),
    {
        let evidence = owner.evidence();
        let request = evidence.request();
        let target_location = owner.target_location();
        let record_index = if owner.matches_receiver(
            self.history_admission_world,
            self.router,
            self.history.incarnation(),
        ) && self.action.is_none()
            && self.current.is_none()
            && self.pending_no_target_back.is_none()
            && self.pending_back_no_target_authority.is_none()
            && self.current_history_capability.is_none()
            && self.pending_history_preparation_error.is_none()
            && self.prepared_publication_authority.is_none()
            && self.state.current_request().is_none()
            && self.queue.iter().all(|queued| queued.request != request)
        {
            self.records.iter().position(|record| {
                record.id() == request
                    && record.classify_back_resolution(&evidence)
                        == BackResolutionEvidenceClassification::ConsumedReplay
                    && matches!(record.status(), RequestStatus::Queued)
                    && record.requested() == Some(target_location)
            })
        } else {
            None
        };
        let Some(record_index) = record_index else {
            return BackTargetResumeOutcome::Mismatch(owner);
        };

        let prepared = self.prepare_back_target_start(owner, record_index);
        commit_retirement();
        self.install_prepared_back_target_start(prepared)
    }

    pub(crate) fn refreeze_back_no_target_source_with_transfer<F>(
        &mut self,
        candidate: VerifiedBackNoTargetCandidate,
        commit_transfer: F,
    ) -> BackNoTargetSourceRefreezeOutcome
    where
        F: FnOnce(),
    {
        let evidence = candidate.evidence();
        let request = evidence.request();
        let exact_receiver = candidate.matches_receiver(
            self.history_admission_world,
            self.router,
            self.history.incarnation(),
        ) && self.action.is_none()
            && self.current.is_none()
            && self.pending_no_target_back.is_none()
            && self.pending_back_no_target_authority.is_none()
            && self.current_history_capability.is_none()
            && self.state.current_request().is_none()
            && self.request(request).is_some_and(|record| {
                record.classify_back_resolution(&evidence)
                    == BackResolutionEvidenceClassification::ConsumedReplay
                    && matches!(record.status(), RequestStatus::Queued)
                    && record.requested().is_none()
            });
        if !exact_receiver {
            return BackNoTargetSourceRefreezeOutcome::Mismatch {
                candidate,
                evidence,
            };
        }

        let prepared = match self
            .history
            .refreeze_back_no_target_source(candidate.source())
        {
            Ok(()) => PreparedBackNoTargetSourceRefreeze::Unchanged(candidate),
            Err(observed) => PreparedBackNoTargetSourceRefreeze::Drift(
                candidate.into_source_drift_owner(observed),
            ),
        };
        commit_transfer();
        match prepared {
            PreparedBackNoTargetSourceRefreeze::Unchanged(candidate) => {
                BackNoTargetSourceRefreezeOutcome::Unchanged(candidate)
            }
            PreparedBackNoTargetSourceRefreeze::Drift(owner) => {
                BackNoTargetSourceRefreezeOutcome::Drift(owner)
            }
        }
    }

    fn observe_back_no_target_logical_zero(
        &self,
        candidate: &VerifiedBackNoTargetCandidate,
    ) -> (
        CoreBackNoTargetLogicalZeroExpectedProjection,
        CoreBackNoTargetLogicalZeroObservedProjection,
        bool,
    ) {
        let receiver = candidate.evidence();
        let request = receiver.request();
        let expected = CoreBackNoTargetLogicalZeroExpectedProjection {
            receiver: receiver.clone(),
            source: CoreFrozenHistorySourceProjection::project(candidate.source()),
            host: self.state.host().cloned(),
        };
        let preview = self.history.preview_back();
        let request_projection = match self.request(request) {
            Some(record) => CoreBackNoTargetLogicalZeroRequestProjection::Retained {
                request,
                status: record.status().clone(),
                requested: record.requested().cloned(),
                resolution: record.classify_back_resolution(&receiver),
            },
            None => CoreBackNoTargetLogicalZeroRequestProjection::Missing { request },
        };
        let pending_history_preparation_error = match &self.pending_history_preparation_error {
            None => CoreBackNoTargetLogicalZeroPreparationErrorProjection::Vacant,
            Some(HistoryPreparationError::Source(_)) => {
                CoreBackNoTargetLogicalZeroPreparationErrorProjection::Source
            }
            Some(HistoryPreparationError::Binding(_)) => {
                CoreBackNoTargetLogicalZeroPreparationErrorProjection::Binding
            }
        };
        let observed = CoreBackNoTargetLogicalZeroObservedProjection {
            scope: self.back_target_engine_scope_projection(),
            source: CoreFrozenHistorySourceProjection::project(preview.observed_source()),
            host: self.state.host().cloned(),
            request: request_projection,
            queued_requests: self.queue.iter().map(|queued| queued.request).collect(),
            current: self
                .current
                .as_ref()
                .map(|current| current.identity.clone()),
            action: Self::back_target_action_projection(self.action.as_ref()),
            pending_no_target_back: self
                .pending_no_target_back
                .as_ref()
                .map(|pending| pending.request),
            current_history_capability: self
                .current_history_capability
                .as_ref()
                .map(HistoryCommitCapability::execution_projection),
            pending_history_preparation_error,
            prepared_publication: self
                .prepared_publication_authority
                .as_ref()
                .map(|authority| authority.owner().evidence.clone()),
            current_request: self.state.current_request().cloned(),
            preview,
        };
        let exact_receiver = candidate.matches_receiver(
            self.history_admission_world,
            self.router,
            self.history.incarnation(),
        );
        let exact_request = matches!(
            &observed.request,
            CoreBackNoTargetLogicalZeroRequestProjection::Retained {
                status: RequestStatus::Queued,
                requested: None,
                resolution: BackResolutionEvidenceClassification::ConsumedReplay,
                ..
            }
        ) && observed
            .queued_requests
            .iter()
            .all(|queued| *queued != request);
        let exact_owner_vacancies = observed.current.is_none()
            && matches!(
                &observed.action,
                CoreBackTargetReceiverActionProjection::Missing
            )
            && observed.pending_no_target_back.is_none()
            && self.pending_back_no_target_authority.is_none()
            && observed.current_history_capability.is_none()
            && matches!(
                &observed.pending_history_preparation_error,
                CoreBackNoTargetLogicalZeroPreparationErrorProjection::Vacant
            )
            && observed.prepared_publication.is_none()
            && observed.current_request.is_none();
        let exact_source = self
            .history
            .refreeze_back_no_target_source(candidate.source())
            .is_ok();
        let exact_host = expected.host.is_some() && observed.host == expected.host;
        let logical_zero = matches!(
            &observed.preview,
            BackPreview::NoTarget(source) if source == candidate.source()
        );
        (
            expected,
            observed,
            exact_receiver
                && exact_request
                && exact_owner_vacancies
                && exact_source
                && exact_host
                && logical_zero,
        )
    }

    pub(crate) fn approve_back_no_target_logical_zero(
        &mut self,
        candidate: VerifiedBackNoTargetCandidate,
    ) -> BackNoTargetApprovalOutcome {
        let (expected, observed, exact) = self.observe_back_no_target_logical_zero(&candidate);
        if exact {
            BackNoTargetApprovalOutcome::Approved(ApprovedBackNoTargetCandidate {
                candidate,
                approval: CoreProvenEmptyApprovalProjection { expected },
            })
        } else {
            BackNoTargetApprovalOutcome::Mismatch {
                candidate,
                mismatch: CorePreparationReceiverMismatchProjection::LogicalZero {
                    expected,
                    observed,
                },
            }
        }
    }

    pub(crate) fn accept_back_no_target_authority_lost_with_retirement<F>(
        &mut self,
        candidate: VerifiedBackNoTargetCandidate,
        commit_retirement: F,
    ) -> BackNoTargetAuthorityCoreOutcome
    where
        F: FnOnce(),
    {
        let (expected, observed, exact) = self.observe_back_no_target_logical_zero(&candidate);
        if !exact || self.pending_back_no_target_authority.is_some() {
            return BackNoTargetAuthorityCoreOutcome::Mismatch {
                candidate,
                mismatch: CorePreparationReceiverMismatchProjection::LogicalZero {
                    expected,
                    observed,
                },
            };
        }
        let Some(host) = expected.host.clone() else {
            return BackNoTargetAuthorityCoreOutcome::Mismatch {
                candidate,
                mismatch: CorePreparationReceiverMismatchProjection::LogicalZero {
                    expected,
                    observed,
                },
            };
        };
        let Some(top_slot) = self.state.top_slot() else {
            return BackNoTargetAuthorityCoreOutcome::Mismatch {
                candidate,
                mismatch: CorePreparationReceiverMismatchProjection::LogicalZero {
                    expected,
                    observed,
                },
            };
        };
        let accepted = CoreBackNoTargetAuthorityAccepted {
            receiver: expected.receiver.clone(),
            source: expected.source.clone(),
            host,
            top_slot,
            logical_roots: self.state.active().frames().iter().rev().cloned().collect(),
        };
        commit_retirement();
        self.pending_back_no_target_authority =
            Some(CorePendingBackNoTargetAuthority::Accepted(accepted));
        BackNoTargetAuthorityCoreOutcome::Accepted(BackNoTargetAuthoritySettlementOwner {
            candidate,
        })
    }

    pub(crate) fn begin_back_no_target_authority_settlement<F>(
        &mut self,
        issuer: &mut HostAdmissionIssuer,
        owner: CoreBackNoTargetAuthorityBeginOwner,
        install: F,
    ) -> BackNoTargetAuthorityBeginCoreOutcome
    where
        F: FnOnce(CoreBackNoTargetAuthorityCleanupBatch),
    {
        let CoreBackNoTargetAuthorityBeginOwner {
            settlement,
            host,
            manifest,
            manifest_digest,
        } = owner;
        let candidate = &settlement.candidate;
        let candidate_receiver = candidate.evidence();
        let observed_receiver =
            self.pending_back_no_target_authority
                .as_ref()
                .and_then(|pending| match pending {
                    CorePendingBackNoTargetAuthority::Accepted(accepted) => {
                        Some(accepted.receiver.clone())
                    }
                    CorePendingBackNoTargetAuthority::Cleanup(cleanup) => {
                        Some(cleanup.receiver.receiver.clone())
                    }
                });
        if !candidate.matches_receiver(
            self.history_admission_world,
            self.router,
            self.history.incarnation(),
        ) {
            return BackNoTargetAuthorityBeginCoreOutcome::ReceiverMismatch {
                owner: CoreBackNoTargetAuthorityBeginOwner {
                    settlement,
                    host,
                    manifest,
                    manifest_digest,
                },
                mismatch: CorePreparationReceiverMismatchProjection::BackNoTargetAuthorityBegin {
                    expected: candidate_receiver,
                    observed: observed_receiver,
                },
            };
        }
        let Some(CorePendingBackNoTargetAuthority::Accepted(accepted)) =
            self.pending_back_no_target_authority.as_ref()
        else {
            return BackNoTargetAuthorityBeginCoreOutcome::Permanent {
                owner: CoreBackNoTargetAuthorityBeginOwner {
                    settlement,
                    host,
                    manifest,
                    manifest_digest,
                },
                evidence:
                    CoreBackNoTargetAuthorityBeginPermanentEvidence::PendingStateContradiction,
            };
        };
        let candidate_source = CoreFrozenHistorySourceProjection::project(candidate.source());
        if accepted.receiver != candidate_receiver || accepted.source != candidate_source {
            return BackNoTargetAuthorityBeginCoreOutcome::ReceiverMismatch {
                owner: CoreBackNoTargetAuthorityBeginOwner {
                    settlement,
                    host,
                    manifest,
                    manifest_digest,
                },
                mismatch: CorePreparationReceiverMismatchProjection::BackNoTargetAuthorityBegin {
                    expected: candidate_receiver,
                    observed: Some(accepted.receiver.clone()),
                },
            };
        }
        let exact_host_manifest = self.state.availability()
            == &crate::presentation::RouterAvailability::Ready
            && self.state.host() == Some(&accepted.host)
            && self.state.top_slot() == Some(accepted.top_slot)
            && host.matches(self.router, &accepted.host, accepted.top_slot)
            && manifest.logical_roots() == accepted.logical_roots
            && self.current.is_none()
            && self.action.is_none()
            && self.pending_no_target_back.is_none()
            && self.current_history_capability.is_none()
            && self.pending_history_preparation_error.is_none()
            && self.prepared_publication_authority.is_none()
            && self.state.current_request().is_none()
            && self
                .history
                .refreeze_back_no_target_source(candidate.source())
                .is_ok();
        if !exact_host_manifest {
            return BackNoTargetAuthorityBeginCoreOutcome::Permanent {
                owner: CoreBackNoTargetAuthorityBeginOwner {
                    settlement,
                    host,
                    manifest,
                    manifest_digest,
                },
                evidence:
                    CoreBackNoTargetAuthorityBeginPermanentEvidence::HostManifestContradiction,
            };
        }
        let request = candidate_receiver.request();
        let Some(record_index) = self.records.iter().position(|record| {
            record.id() == request
                && record.classify_back_resolution(&candidate_receiver)
                    == BackResolutionEvidenceClassification::ConsumedReplay
                && matches!(record.status(), RequestStatus::Queued)
                && record.requested().is_none()
        }) else {
            return BackNoTargetAuthorityBeginCoreOutcome::Permanent {
                owner: CoreBackNoTargetAuthorityBeginOwner {
                    settlement,
                    host,
                    manifest,
                    manifest_digest,
                },
                evidence:
                    CoreBackNoTargetAuthorityBeginPermanentEvidence::PendingStateContradiction,
            };
        };

        let failure = NavigationFailure::new(
            NavigationFailureKind::AuthorityLost,
            "physical no-target authority was lost before history execution",
        );
        let mut recovering_records = self.records.clone();
        let Some(recovering_record) = recovering_records.get_mut(record_index) else {
            return BackNoTargetAuthorityBeginCoreOutcome::Permanent {
                owner: CoreBackNoTargetAuthorityBeginOwner {
                    settlement,
                    host,
                    manifest,
                    manifest_digest,
                },
                evidence:
                    CoreBackNoTargetAuthorityBeginPermanentEvidence::PendingStateContradiction,
            };
        };
        if recovering_record
            .transition(RequestStatus::Running(NavigationPhase::Recovering))
            .is_err()
            || recovering_record
                .append_trace(NavigationTraceEvent::request_phase_changed(
                    NavigationPhase::Recovering,
                ))
                .is_err()
        {
            return BackNoTargetAuthorityBeginCoreOutcome::Permanent {
                owner: CoreBackNoTargetAuthorityBeginOwner {
                    settlement,
                    host,
                    manifest,
                    manifest_digest,
                },
                evidence:
                    CoreBackNoTargetAuthorityBeginPermanentEvidence::PendingStateContradiction,
            };
        }
        for queued in &self.queue {
            let Some(record) = recovering_records
                .iter_mut()
                .find(|record| record.id() == queued.request)
            else {
                return BackNoTargetAuthorityBeginCoreOutcome::Permanent {
                    owner: CoreBackNoTargetAuthorityBeginOwner {
                        settlement,
                        host,
                        manifest,
                        manifest_digest,
                    },
                    evidence:
                        CoreBackNoTargetAuthorityBeginPermanentEvidence::PendingStateContradiction,
                };
            };
            if record
                .transition(RequestStatus::Failed(NavigationFailure::new(
                    NavigationFailureKind::AuthorityLost,
                    "router authority was lost before request start",
                )))
                .is_err()
                || record
                    .append_trace(NavigationTraceEvent::request_terminal())
                    .is_err()
            {
                return BackNoTargetAuthorityBeginCoreOutcome::Permanent {
                    owner: CoreBackNoTargetAuthorityBeginOwner {
                        settlement,
                        host,
                        manifest,
                        manifest_digest,
                    },
                    evidence:
                        CoreBackNoTargetAuthorityBeginPermanentEvidence::PendingStateContradiction,
                };
            }
        }
        let mut recovering_state = self.state.clone();
        if !recovering_state.begin_authority_recovery(&accepted.host, accepted.top_slot) {
            return BackNoTargetAuthorityBeginCoreOutcome::Permanent {
                owner: CoreBackNoTargetAuthorityBeginOwner {
                    settlement,
                    host,
                    manifest,
                    manifest_digest,
                },
                evidence:
                    CoreBackNoTargetAuthorityBeginPermanentEvidence::HostManifestContradiction,
            };
        }
        recovering_state.set_request(Some(RequestStatus::Running(NavigationPhase::Recovering)));

        let mut recovering_evicted_through = self.evicted_through;
        let mut recovering_issued_view_tasks = self.issued_view_tasks.clone();
        let mut recovering_report_dispositions = self.report_dispositions.clone();
        let mut recovering_completed_cleanup = self.completed_cleanup.clone();
        while recovering_records
            .iter()
            .filter(|record| is_terminal(record.status()))
            .count()
            > TERMINAL_RETENTION
        {
            let Some(index) = recovering_records
                .iter()
                .position(|record| is_terminal(record.status()))
            else {
                return BackNoTargetAuthorityBeginCoreOutcome::Permanent {
                    owner: CoreBackNoTargetAuthorityBeginOwner {
                        settlement,
                        host,
                        manifest,
                        manifest_digest,
                    },
                    evidence:
                        CoreBackNoTargetAuthorityBeginPermanentEvidence::PendingStateContradiction,
                };
            };
            let Some(evicted) = recovering_records.remove(index) else {
                return BackNoTargetAuthorityBeginCoreOutcome::Permanent {
                    owner: CoreBackNoTargetAuthorityBeginOwner {
                        settlement,
                        host,
                        manifest,
                        manifest_digest,
                    },
                    evidence:
                        CoreBackNoTargetAuthorityBeginPermanentEvidence::PendingStateContradiction,
                };
            };
            let evicted_request = evicted.id();
            recovering_issued_view_tasks.retain(|identity| identity.request != evicted_request);
            recovering_report_dispositions
                .retain(|record| record.target != DiagnosticTarget::Request(evicted_request));
            recovering_completed_cleanup.retain(|tombstone| tombstone.request != evicted_request);
            let sequence = evicted_request.sequence();
            recovering_evicted_through =
                Some(recovering_evicted_through.map_or(sequence, |current| current.max(sequence)));
        }

        let mut terminal_records = recovering_records.clone();
        let Some(terminal_record) = terminal_records
            .iter_mut()
            .find(|record| record.id() == request)
        else {
            return BackNoTargetAuthorityBeginCoreOutcome::Permanent {
                owner: CoreBackNoTargetAuthorityBeginOwner {
                    settlement,
                    host,
                    manifest,
                    manifest_digest,
                },
                evidence:
                    CoreBackNoTargetAuthorityBeginPermanentEvidence::PendingStateContradiction,
            };
        };
        if terminal_record
            .append_trace(NavigationTraceEvent::cleanup_settled(None))
            .is_err()
            || terminal_record
                .transition(RequestStatus::Failed(failure))
                .is_err()
            || terminal_record
                .append_trace(NavigationTraceEvent::request_terminal())
                .is_err()
        {
            return BackNoTargetAuthorityBeginCoreOutcome::Permanent {
                owner: CoreBackNoTargetAuthorityBeginOwner {
                    settlement,
                    host,
                    manifest,
                    manifest_digest,
                },
                evidence:
                    CoreBackNoTargetAuthorityBeginPermanentEvidence::PendingStateContradiction,
            };
        }
        let mut terminal_state = recovering_state.clone();
        if !terminal_state.complete_authority_settlement(&accepted.host, accepted.top_slot) {
            return BackNoTargetAuthorityBeginCoreOutcome::Permanent {
                owner: CoreBackNoTargetAuthorityBeginOwner {
                    settlement,
                    host,
                    manifest,
                    manifest_digest,
                },
                evidence:
                    CoreBackNoTargetAuthorityBeginPermanentEvidence::HostManifestContradiction,
            };
        }
        terminal_state.set_request(None);
        let mut terminal_evicted_through = recovering_evicted_through;
        let mut terminal_issued_view_tasks = recovering_issued_view_tasks.clone();
        let mut terminal_report_dispositions = recovering_report_dispositions.clone();
        let mut terminal_completed_cleanup = recovering_completed_cleanup.clone();
        while terminal_records
            .iter()
            .filter(|record| is_terminal(record.status()))
            .count()
            > TERMINAL_RETENTION
        {
            let Some(index) = terminal_records
                .iter()
                .position(|record| is_terminal(record.status()))
            else {
                return BackNoTargetAuthorityBeginCoreOutcome::Permanent {
                    owner: CoreBackNoTargetAuthorityBeginOwner {
                        settlement,
                        host,
                        manifest,
                        manifest_digest,
                    },
                    evidence:
                        CoreBackNoTargetAuthorityBeginPermanentEvidence::PendingStateContradiction,
                };
            };
            let Some(evicted) = terminal_records.remove(index) else {
                return BackNoTargetAuthorityBeginCoreOutcome::Permanent {
                    owner: CoreBackNoTargetAuthorityBeginOwner {
                        settlement,
                        host,
                        manifest,
                        manifest_digest,
                    },
                    evidence:
                        CoreBackNoTargetAuthorityBeginPermanentEvidence::PendingStateContradiction,
                };
            };
            let evicted_request = evicted.id();
            terminal_issued_view_tasks.retain(|identity| identity.request != evicted_request);
            terminal_report_dispositions
                .retain(|record| record.target != DiagnosticTarget::Request(evicted_request));
            terminal_completed_cleanup.retain(|tombstone| tombstone.request != evicted_request);
            let sequence = evicted_request.sequence();
            terminal_evicted_through =
                Some(terminal_evicted_through.map_or(sequence, |current| current.max(sequence)));
        }
        let receiver = CoreBackNoTargetAuthorityCleanupProjection {
            receiver: candidate_receiver,
            source: candidate_source,
            host: accepted.host.clone(),
            top_slot: accepted.top_slot,
            request,
            manifest_digest,
            obligation_count: manifest.obligation_count(),
        };
        let batch = CoreBackNoTargetAuthorityCleanupBatch {
            receiver: receiver.clone(),
        };
        let terminal = CoreBackNoTargetAuthorityTerminalPlan {
            state: terminal_state,
            records: terminal_records,
            evicted_through: terminal_evicted_through,
            issued_view_tasks: terminal_issued_view_tasks,
            report_dispositions: terminal_report_dispositions,
            completed_cleanup: terminal_completed_cleanup,
        };
        let retired_host = match issuer.retire_current(host) {
            Ok(retired) => retired,
            Err(error) => {
                return BackNoTargetAuthorityBeginCoreOutcome::Permanent {
                    owner: CoreBackNoTargetAuthorityBeginOwner {
                        settlement,
                        host: error.into_owner(),
                        manifest,
                        manifest_digest,
                    },
                    evidence:
                        CoreBackNoTargetAuthorityBeginPermanentEvidence::HostRetirementRejected,
                };
            }
        };

        install(batch);
        self.pending_back_no_target_authority = Some(CorePendingBackNoTargetAuthority::Cleanup(
            CoreBackNoTargetAuthorityCleanupState {
                receiver,
                retired_host,
                terminal,
            },
        ));
        self.records = recovering_records;
        self.state = recovering_state;
        self.evicted_through = recovering_evicted_through;
        self.issued_view_tasks = recovering_issued_view_tasks;
        self.report_dispositions = recovering_report_dispositions;
        self.completed_cleanup = recovering_completed_cleanup;
        self.queue.clear();
        BackNoTargetAuthorityBeginCoreOutcome::Begun
    }

    pub(crate) fn begin_back_target_permanent_cleanup<Installation, F>(
        &mut self,
        issuer: &mut HostAdmissionIssuer,
        owner: CoreBackTargetPermanentBeginOwner,
        ticket: &CommitTicket,
        logical_roots: Vec<RouteFrameVersion>,
        manifest_digest: u64,
        obligation_count: usize,
        installation: Installation,
        install: F,
    ) -> BackTargetPermanentBeginCoreOutcome<Installation>
    where
        F: FnOnce(Installation, CoreBackTargetPermanentCleanupBatch),
    {
        let CoreBackTargetPermanentBeginOwner {
            settlement,
            host: host_lease,
        } = owner;
        if let Err(mismatch) =
            self.inspect_back_target_continuation(&settlement.scope, Some(ticket))
        {
            return BackTargetPermanentBeginCoreOutcome::ReceiverMismatch {
                owner: CoreBackTargetPermanentBeginOwner {
                    settlement,
                    host: host_lease,
                },
                installation,
                mismatch,
            };
        }
        if settlement.logical_roots != logical_roots {
            return BackTargetPermanentBeginCoreOutcome::LogicalRootsMismatch {
                owner: CoreBackTargetPermanentBeginOwner {
                    settlement,
                    host: host_lease,
                },
                installation,
            };
        }

        let observed_current = self.back_target_permanent_current_state_projection();
        if observed_current != settlement.expected_current {
            return BackTargetPermanentBeginCoreOutcome::Permanent {
                evidence:
                    BackTargetPermanentBeginCorePermanentEvidence::CurrentStateContradiction {
                        expected: settlement.expected_current.clone(),
                        observed: observed_current,
                    },
                owner: CoreBackTargetPermanentBeginOwner {
                    settlement,
                    host: host_lease,
                },
                installation,
            };
        }
        let (expected_canonical, observed_canonical) =
            self.back_target_permanent_canonical_relation_projections(&settlement);
        if expected_canonical != observed_canonical {
            return BackTargetPermanentBeginCoreOutcome::Permanent {
                evidence:
                    BackTargetPermanentBeginCorePermanentEvidence::CanonicalRelationContradiction {
                        expected: expected_canonical,
                        observed: observed_canonical,
                    },
                owner: CoreBackTargetPermanentBeginOwner {
                    settlement,
                    host: host_lease,
                },
                installation,
            };
        }

        let request = settlement.scope.request();
        let observed_request = self
            .records
            .iter()
            .find(|record| record.id() == request)
            .map(Self::back_target_permanent_request_projection);
        let exact_request_record = observed_request.as_ref().is_some_and(|observed| {
            settlement
                .expected_request
                .as_ref()
                .is_some_and(|expected| {
                    observed.request == expected.request
                        && observed.status == expected.status
                        && observed.requested == expected.requested
                })
        });
        if !exact_request_record {
            return BackTargetPermanentBeginCoreOutcome::Permanent {
                evidence:
                    BackTargetPermanentBeginCorePermanentEvidence::RequestRecordContradiction {
                        expected: settlement.expected_request.clone(),
                        observed: observed_request,
                    },
                owner: CoreBackTargetPermanentBeginOwner {
                    settlement,
                    host: host_lease,
                },
                installation,
            };
        }

        let current = self
            .current
            .as_ref()
            .expect("the exact frozen current projection retains current work");
        let host = self
            .state
            .host()
            .cloned()
            .expect("the exact frozen current projection retains host authority");
        let top_slot = self
            .state
            .top_slot()
            .expect("the exact frozen current projection retains the top slot");
        let record_index = self
            .records
            .iter()
            .position(|record| record.id() == request)
            .expect("the exact frozen request projection retains its record");
        let cleanup_batch = current.cleanup_batch;

        let source = CoreBackTargetPermanentTerminalSourceProjection {
            scope: settlement.scope.clone(),
            current: observed_current,
            current_request: observed_request
                .expect("the exact frozen request projection retains its record"),
            queue: self
                .queue
                .iter()
                .map(|queued| CoreBackTargetPermanentQueuedRequestProjection {
                    expected: CoreBackTargetPermanentRequestProjection {
                        request: queued.request,
                        status: RequestStatus::Queued,
                        requested: queued.command.location().cloned(),
                        trace_closed: false,
                    },
                    observed: self
                        .records
                        .iter()
                        .find(|record| record.id() == queued.request)
                        .map(Self::back_target_permanent_request_projection),
                })
                .collect(),
            authority: Self::back_target_permanent_authority_projection(&self.state),
        };

        let failure = NavigationFailure::new(
            NavigationFailureKind::AuthorityLost,
            "physical back-target cleanup authority was lost after splice preparation",
        );
        let mut recovering_records = self.records.clone();
        let recovering_record = recovering_records
            .get_mut(record_index)
            .expect("the clone retains the exact frozen record index");
        let recovering_expected = CoreBackTargetPermanentRequestProjection {
            request,
            status: RequestStatus::Running(NavigationPhase::Recovering),
            requested: recovering_record.requested().cloned(),
            trace_closed: recovering_record.trace_closed(),
        };
        if let Err(error) =
            recovering_record.transition(RequestStatus::Running(NavigationPhase::Recovering))
        {
            return BackTargetPermanentBeginCoreOutcome::Permanent {
                evidence:
                    BackTargetPermanentBeginCorePermanentEvidence::TerminalPlanContradiction {
                        source,
                        expected: CoreBackTargetPermanentTerminalExpectedProjection::Request {
                            operation:
                                CoreBackTargetPermanentTerminalOperation::RecoveringTransition,
                            request: recovering_expected,
                        },
                        observed: CoreBackTargetPermanentTerminalObservedProjection::Request {
                            operation:
                                CoreBackTargetPermanentTerminalOperation::RecoveringTransition,
                            request: Some(Self::back_target_permanent_request_projection(
                                recovering_record,
                            )),
                            failure:
                                CoreBackTargetPermanentTerminalFailureProjection::RequestRejected(
                                    error,
                                ),
                        },
                    },
                owner: CoreBackTargetPermanentBeginOwner {
                    settlement,
                    host: host_lease,
                },
                installation,
            };
        }
        let completion_expected_request = recovering_expected.clone();
        let recovering_trace_expected = CoreBackTargetPermanentRequestProjection {
            trace_closed: false,
            ..recovering_expected
        };
        if let Err(error) = recovering_record.append_trace(
            NavigationTraceEvent::request_phase_changed(NavigationPhase::Recovering),
        ) {
            return BackTargetPermanentBeginCoreOutcome::Permanent {
                evidence:
                    BackTargetPermanentBeginCorePermanentEvidence::TerminalPlanContradiction {
                        source,
                        expected: CoreBackTargetPermanentTerminalExpectedProjection::Request {
                            operation: CoreBackTargetPermanentTerminalOperation::RecoveringTrace,
                            request: recovering_trace_expected,
                        },
                        observed: CoreBackTargetPermanentTerminalObservedProjection::Request {
                            operation: CoreBackTargetPermanentTerminalOperation::RecoveringTrace,
                            request: Some(Self::back_target_permanent_request_projection(
                                recovering_record,
                            )),
                            failure:
                                CoreBackTargetPermanentTerminalFailureProjection::RequestRejected(
                                    error,
                                ),
                        },
                    },
                owner: CoreBackTargetPermanentBeginOwner {
                    settlement,
                    host: host_lease,
                },
                installation,
            };
        }

        for queued in &self.queue {
            let Some(record) = recovering_records
                .iter_mut()
                .find(|record| record.id() == queued.request)
            else {
                let expected = CoreBackTargetPermanentRequestProjection {
                    request: queued.request,
                    status: RequestStatus::Queued,
                    requested: queued.command.location().cloned(),
                    trace_closed: false,
                };
                return BackTargetPermanentBeginCoreOutcome::Permanent {
                    evidence: BackTargetPermanentBeginCorePermanentEvidence::TerminalPlanContradiction {
                        source,
                        expected: CoreBackTargetPermanentTerminalExpectedProjection::Request {
                            operation: CoreBackTargetPermanentTerminalOperation::QueuedTerminalTransition,
                            request: expected,
                        },
                        observed: CoreBackTargetPermanentTerminalObservedProjection::Request {
                            operation: CoreBackTargetPermanentTerminalOperation::QueuedTerminalTransition,
                            request: None,
                            failure: CoreBackTargetPermanentTerminalFailureProjection::MissingRequest,
                        },
                    },
                owner: CoreBackTargetPermanentBeginOwner {
                    settlement,
                    host: host_lease,
                },
                    installation,
                };
            };
            let queued_failure = NavigationFailure::new(
                NavigationFailureKind::AuthorityLost,
                "router authority was lost before request start",
            );
            let transition_expected = CoreBackTargetPermanentRequestProjection {
                request: record.id(),
                status: RequestStatus::Failed(queued_failure.clone()),
                requested: record.requested().cloned(),
                trace_closed: record.trace_closed(),
            };
            if let Err(error) = record.transition(RequestStatus::Failed(queued_failure)) {
                return BackTargetPermanentBeginCoreOutcome::Permanent {
                    evidence: BackTargetPermanentBeginCorePermanentEvidence::TerminalPlanContradiction {
                        source,
                        expected: CoreBackTargetPermanentTerminalExpectedProjection::Request {
                            operation: CoreBackTargetPermanentTerminalOperation::QueuedTerminalTransition,
                            request: transition_expected,
                        },
                        observed: CoreBackTargetPermanentTerminalObservedProjection::Request {
                            operation: CoreBackTargetPermanentTerminalOperation::QueuedTerminalTransition,
                            request: Some(Self::back_target_permanent_request_projection(record)),
                            failure: CoreBackTargetPermanentTerminalFailureProjection::RequestRejected(error),
                        },
                    },
                owner: CoreBackTargetPermanentBeginOwner {
                    settlement,
                    host: host_lease,
                },
                    installation,
                };
            }
            let trace_expected = CoreBackTargetPermanentRequestProjection {
                trace_closed: true,
                ..transition_expected
            };
            if let Err(error) = record.append_trace(NavigationTraceEvent::request_terminal()) {
                return BackTargetPermanentBeginCoreOutcome::Permanent {
                    evidence: BackTargetPermanentBeginCorePermanentEvidence::TerminalPlanContradiction {
                        source,
                        expected: CoreBackTargetPermanentTerminalExpectedProjection::Request {
                            operation: CoreBackTargetPermanentTerminalOperation::QueuedTerminalTrace,
                            request: trace_expected,
                        },
                        observed: CoreBackTargetPermanentTerminalObservedProjection::Request {
                            operation: CoreBackTargetPermanentTerminalOperation::QueuedTerminalTrace,
                            request: Some(Self::back_target_permanent_request_projection(record)),
                            failure: CoreBackTargetPermanentTerminalFailureProjection::RequestRejected(error),
                        },
                    },
                owner: CoreBackTargetPermanentBeginOwner {
                    settlement,
                    host: host_lease,
                },
                    installation,
                };
            }
        }

        let mut recovering_state = self.state.clone();
        if !recovering_state.begin_authority_recovery(&host, top_slot) {
            return BackTargetPermanentBeginCoreOutcome::Permanent {
                evidence: BackTargetPermanentBeginCorePermanentEvidence::TerminalPlanContradiction {
                    source,
                    expected: CoreBackTargetPermanentTerminalExpectedProjection::Authority {
                        operation: CoreBackTargetPermanentTerminalOperation::AuthorityRecovery,
                        authority: CoreBackTargetPermanentAuthorityProjection {
                            host: Some(host),
                            top_slot: Some(top_slot),
                            availability: crate::presentation::RouterAvailability::RecoveringAuthority,
                        },
                    },
                    observed: CoreBackTargetPermanentTerminalObservedProjection::Authority {
                        operation: CoreBackTargetPermanentTerminalOperation::AuthorityRecovery,
                        authority: Self::back_target_permanent_authority_projection(&recovering_state),
                        failure: CoreBackTargetPermanentTerminalFailureProjection::AuthorityRecoveryRejected,
                    },
                },
                owner: CoreBackTargetPermanentBeginOwner {
                    settlement,
                    host: host_lease,
                },
                installation,
            };
        }
        recovering_state.set_request(Some(RequestStatus::Running(NavigationPhase::Recovering)));

        let mut recovering_evicted_through = self.evicted_through;
        let mut recovering_issued_view_tasks = self.issued_view_tasks.clone();
        let mut recovering_report_dispositions = self.report_dispositions.clone();
        let mut recovering_completed_cleanup = self.completed_cleanup.clone();
        while recovering_records
            .iter()
            .filter(|record| is_terminal(record.status()))
            .count()
            > TERMINAL_RETENTION
        {
            let Some(index) = recovering_records
                .iter()
                .position(|record| is_terminal(record.status()))
            else {
                let retention =
                    Self::back_target_permanent_retention_projection(&recovering_records);
                return BackTargetPermanentBeginCoreOutcome::Permanent {
                    evidence: BackTargetPermanentBeginCorePermanentEvidence::TerminalPlanContradiction {
                        source,
                        expected: CoreBackTargetPermanentTerminalExpectedProjection::Retention {
                            operation: CoreBackTargetPermanentTerminalOperation::RecoveringRetention,
                            retention: retention.clone(),
                        },
                        observed: CoreBackTargetPermanentTerminalObservedProjection::Retention {
                            operation: CoreBackTargetPermanentTerminalOperation::RecoveringRetention,
                            retention,
                            failure: CoreBackTargetPermanentTerminalFailureProjection::RetentionInvariant,
                        },
                    },
                owner: CoreBackTargetPermanentBeginOwner {
                    settlement,
                    host: host_lease,
                },
                    installation,
                };
            };
            let evicted = recovering_records
                .remove(index)
                .expect("the located terminal clone entry remains removable");
            let evicted_request = evicted.id();
            recovering_issued_view_tasks.retain(|identity| identity.request != evicted_request);
            recovering_report_dispositions
                .retain(|record| record.target != DiagnosticTarget::Request(evicted_request));
            recovering_completed_cleanup.retain(|tombstone| tombstone.request != evicted_request);
            let sequence = evicted_request.sequence();
            recovering_evicted_through =
                Some(recovering_evicted_through.map_or(sequence, |current| current.max(sequence)));
        }

        let mut terminal_records = recovering_records.clone();
        let terminal_record = terminal_records
            .iter_mut()
            .find(|record| record.id() == request)
            .expect("the recovering clone retains the current request");
        let cleanup_trace_expected = CoreBackTargetPermanentRequestProjection {
            request,
            status: terminal_record.status().clone(),
            requested: terminal_record.requested().cloned(),
            trace_closed: false,
        };
        if let Err(error) =
            terminal_record.append_trace(NavigationTraceEvent::cleanup_settled(None))
        {
            return BackTargetPermanentBeginCoreOutcome::Permanent {
                evidence:
                    BackTargetPermanentBeginCorePermanentEvidence::TerminalPlanContradiction {
                        source,
                        expected: CoreBackTargetPermanentTerminalExpectedProjection::Request {
                            operation:
                                CoreBackTargetPermanentTerminalOperation::CleanupSettledTrace,
                            request: cleanup_trace_expected,
                        },
                        observed: CoreBackTargetPermanentTerminalObservedProjection::Request {
                            operation:
                                CoreBackTargetPermanentTerminalOperation::CleanupSettledTrace,
                            request: Some(Self::back_target_permanent_request_projection(
                                terminal_record,
                            )),
                            failure:
                                CoreBackTargetPermanentTerminalFailureProjection::RequestRejected(
                                    error,
                                ),
                        },
                    },
                owner: CoreBackTargetPermanentBeginOwner {
                    settlement,
                    host: host_lease,
                },
                installation,
            };
        }
        let terminal_expected = CoreBackTargetPermanentRequestProjection {
            request,
            status: RequestStatus::Failed(failure.clone()),
            requested: terminal_record.requested().cloned(),
            trace_closed: terminal_record.trace_closed(),
        };
        if let Err(error) = terminal_record.transition(RequestStatus::Failed(failure)) {
            return BackTargetPermanentBeginCoreOutcome::Permanent {
                evidence:
                    BackTargetPermanentBeginCorePermanentEvidence::TerminalPlanContradiction {
                        source,
                        expected: CoreBackTargetPermanentTerminalExpectedProjection::Request {
                            operation: CoreBackTargetPermanentTerminalOperation::TerminalTransition,
                            request: terminal_expected,
                        },
                        observed: CoreBackTargetPermanentTerminalObservedProjection::Request {
                            operation: CoreBackTargetPermanentTerminalOperation::TerminalTransition,
                            request: Some(Self::back_target_permanent_request_projection(
                                terminal_record,
                            )),
                            failure:
                                CoreBackTargetPermanentTerminalFailureProjection::RequestRejected(
                                    error,
                                ),
                        },
                    },
                owner: CoreBackTargetPermanentBeginOwner {
                    settlement,
                    host: host_lease,
                },
                installation,
            };
        }
        let terminal_trace_expected = CoreBackTargetPermanentRequestProjection {
            trace_closed: true,
            ..terminal_expected
        };
        if let Err(error) = terminal_record.append_trace(NavigationTraceEvent::request_terminal()) {
            return BackTargetPermanentBeginCoreOutcome::Permanent {
                evidence:
                    BackTargetPermanentBeginCorePermanentEvidence::TerminalPlanContradiction {
                        source,
                        expected: CoreBackTargetPermanentTerminalExpectedProjection::Request {
                            operation:
                                CoreBackTargetPermanentTerminalOperation::RequestTerminalTrace,
                            request: terminal_trace_expected,
                        },
                        observed: CoreBackTargetPermanentTerminalObservedProjection::Request {
                            operation:
                                CoreBackTargetPermanentTerminalOperation::RequestTerminalTrace,
                            request: Some(Self::back_target_permanent_request_projection(
                                terminal_record,
                            )),
                            failure:
                                CoreBackTargetPermanentTerminalFailureProjection::RequestRejected(
                                    error,
                                ),
                        },
                    },
                owner: CoreBackTargetPermanentBeginOwner {
                    settlement,
                    host: host_lease,
                },
                installation,
            };
        }

        let mut terminal_state = recovering_state.clone();
        if !terminal_state.complete_authority_settlement(&host, top_slot) {
            return BackTargetPermanentBeginCoreOutcome::Permanent {
                evidence: BackTargetPermanentBeginCorePermanentEvidence::TerminalPlanContradiction {
                    source,
                    expected: CoreBackTargetPermanentTerminalExpectedProjection::Authority {
                        operation: CoreBackTargetPermanentTerminalOperation::AuthoritySettlement,
                        authority: CoreBackTargetPermanentAuthorityProjection {
                            host: None,
                            top_slot: None,
                            availability: crate::presentation::RouterAvailability::AwaitingHost,
                        },
                    },
                    observed: CoreBackTargetPermanentTerminalObservedProjection::Authority {
                        operation: CoreBackTargetPermanentTerminalOperation::AuthoritySettlement,
                        authority: Self::back_target_permanent_authority_projection(&terminal_state),
                        failure: CoreBackTargetPermanentTerminalFailureProjection::AuthoritySettlementRejected,
                    },
                },
                owner: CoreBackTargetPermanentBeginOwner {
                    settlement,
                    host: host_lease,
                },
                installation,
            };
        }
        terminal_state.set_request(None);
        let mut terminal_evicted_through = recovering_evicted_through;
        let mut terminal_issued_view_tasks = recovering_issued_view_tasks.clone();
        let mut terminal_report_dispositions = recovering_report_dispositions.clone();
        let mut terminal_completed_cleanup = recovering_completed_cleanup.clone();
        while terminal_records
            .iter()
            .filter(|record| is_terminal(record.status()))
            .count()
            > TERMINAL_RETENTION
        {
            let Some(index) = terminal_records
                .iter()
                .position(|record| is_terminal(record.status()))
            else {
                let retention = Self::back_target_permanent_retention_projection(&terminal_records);
                return BackTargetPermanentBeginCoreOutcome::Permanent {
                    evidence: BackTargetPermanentBeginCorePermanentEvidence::TerminalPlanContradiction {
                        source,
                        expected: CoreBackTargetPermanentTerminalExpectedProjection::Retention {
                            operation: CoreBackTargetPermanentTerminalOperation::TerminalRetention,
                            retention: retention.clone(),
                        },
                        observed: CoreBackTargetPermanentTerminalObservedProjection::Retention {
                            operation: CoreBackTargetPermanentTerminalOperation::TerminalRetention,
                            retention,
                            failure: CoreBackTargetPermanentTerminalFailureProjection::RetentionInvariant,
                        },
                    },
                owner: CoreBackTargetPermanentBeginOwner {
                    settlement,
                    host: host_lease,
                },
                    installation,
                };
            };
            let evicted = terminal_records
                .remove(index)
                .expect("the located terminal clone entry remains removable");
            let evicted_request = evicted.id();
            terminal_issued_view_tasks.retain(|identity| identity.request != evicted_request);
            terminal_report_dispositions
                .retain(|record| record.target != DiagnosticTarget::Request(evicted_request));
            terminal_completed_cleanup.retain(|tombstone| tombstone.request != evicted_request);
            let sequence = evicted_request.sequence();
            terminal_evicted_through =
                Some(terminal_evicted_through.map_or(sequence, |current| current.max(sequence)));
        }

        let receiver = CoreBackTargetPermanentCleanupProjection {
            scope: settlement.scope.clone(),
            commit: ticket.identity.clone(),
            batch: cleanup_batch,
            logical_roots: logical_roots.clone(),
            manifest_digest,
            obligation_count,
        };
        let expectation = CoreBackTargetPermanentCleanupCompletionExpectedProjection {
            cleanup: receiver.clone(),
            current: CoreBackTargetPermanentCurrentStateProjection {
                host: Some(host.clone()),
                top_slot: Some(top_slot),
                current_identity: None,
                current_location: None,
                current_stage: CoreBackTargetReceiverStageProjection::Missing,
                current_logical_roots: None,
                current_reconciliation: None,
                current_retired_host: Some(host_lease.retirement_projection()),
                active_baseline: BackTargetPreparationBaseline::new(
                    false,
                    Some(host.clone()),
                    Some(top_slot),
                    recovering_state.active().clone(),
                    self.current_active_branch()
                        .map(|branch| branch.branch_fingerprint().clone()),
                    self.active_child_slots.clone(),
                ),
                availability: crate::presentation::RouterAvailability::RecoveringAuthority,
                request_status: Some(RequestStatus::Running(NavigationPhase::Recovering)),
                history_binding: None,
                pending_no_target: None,
                pending_no_target_authority: None,
                pending_target_cleanup: Some(receiver.clone()),
                history_preparation_error:
                    CoreBackTargetPermanentHistoryPreparationErrorProjection::Vacant,
                prepared_publication: None,
                action: CoreBackTargetReceiverActionProjection::Missing,
            },
            request: completion_expected_request,
            resolution: BackResolutionEvidenceClassification::ConsumedReplay,
        };
        let batch = CoreBackTargetPermanentCleanupBatch {
            expectation: expectation.clone(),
            token: CleanupBatchToken::sealed(
                ticket.identity.clone(),
                cleanup_batch,
                &logical_roots,
                crate::cleanup::CleanupManifestDigest::from_opaque(manifest_digest),
                obligation_count,
            ),
        };
        let terminal = CoreBackTargetPermanentTerminalPlan {
            state: terminal_state,
            records: terminal_records,
            evicted_through: terminal_evicted_through,
            issued_view_tasks: terminal_issued_view_tasks,
            report_dispositions: terminal_report_dispositions,
            completed_cleanup: terminal_completed_cleanup,
            active_child_slots: Vec::new(),
        };
        let recovering_queue = VecDeque::new();

        let retired_host = match issuer.retire_current(host_lease) {
            Ok(retired_host) => retired_host,
            Err(error) => {
                return BackTargetPermanentBeginCoreOutcome::Permanent {
                    owner: CoreBackTargetPermanentBeginOwner {
                        settlement,
                        host: error.into_owner(),
                    },
                    installation,
                    evidence: BackTargetPermanentBeginCorePermanentEvidence::HostRetirementRejected,
                };
            }
        };

        install(installation, batch);
        let history = self.current_history_capability.take();
        self.pending_back_target_permanent_cleanup = Some(CoreBackTargetPermanentCleanupState {
            receiver,
            expectation,
            history,
            settlement,
            retired_host,
            terminal,
        });
        self.records = recovering_records;
        self.state = recovering_state;
        self.evicted_through = recovering_evicted_through;
        self.issued_view_tasks = recovering_issued_view_tasks;
        self.report_dispositions = recovering_report_dispositions;
        self.completed_cleanup = recovering_completed_cleanup;
        self.queue = recovering_queue;
        self.current = None;
        self.pending_history_preparation_error = None;
        self.prepared_publication_authority = None;
        self.action = None;
        BackTargetPermanentBeginCoreOutcome::Begun
    }

    pub(crate) fn complete_back_no_target_authority_cleanup<F>(
        &mut self,
        owner: CoreBackNoTargetAuthorityCleanupCompletionOwner,
        commit_physical_and_clear_holder: F,
    ) -> BackNoTargetAuthorityCleanupCompletionCoreOutcome
    where
        F: FnOnce(),
    {
        let expected = owner.batch.receiver.clone();
        let observed = self
            .pending_back_no_target_authority
            .as_ref()
            .and_then(|pending| match pending {
                CorePendingBackNoTargetAuthority::Accepted(_) => None,
                CorePendingBackNoTargetAuthority::Cleanup(cleanup) => {
                    Some(cleanup.receiver.clone())
                }
            });
        let exact_receiver = observed.as_ref() == Some(&expected)
            && self.state.availability()
                == &crate::presentation::RouterAvailability::RecoveringAuthority
            && self.state.host() == Some(&expected.host)
            && self.state.top_slot() == Some(expected.top_slot)
            && matches!(
                self.state.current_request(),
                Some(RequestStatus::Running(NavigationPhase::Recovering))
            )
            && self.current.is_none()
            && self.action.is_none()
            && self.pending_no_target_back.is_none()
            && self.current_history_capability.is_none()
            && self.pending_history_preparation_error.is_none()
            && self.prepared_publication_authority.is_none()
            && self.request(expected.request).is_some_and(|record| {
                record.classify_back_resolution(&expected.receiver)
                    == BackResolutionEvidenceClassification::ConsumedReplay
                    && matches!(
                        record.status(),
                        RequestStatus::Running(NavigationPhase::Recovering)
                    )
                    && record.requested().is_none()
            })
            && self.pending_back_no_target_authority.as_ref().is_some_and(
                |pending| match pending {
                    CorePendingBackNoTargetAuthority::Accepted(_) => false,
                    CorePendingBackNoTargetAuthority::Cleanup(cleanup) => cleanup
                        .retired_host
                        .matches(self.router, &expected.host, expected.top_slot),
                },
            );
        if !exact_receiver {
            return BackNoTargetAuthorityCleanupCompletionCoreOutcome::Mismatch {
                owner,
                evidence: CoreBackNoTargetAuthorityCleanupMismatchEvidence::Receiver(
                    CorePreparationReceiverMismatchProjection::BackNoTargetAuthorityCompletion {
                        expected,
                        observed,
                    },
                ),
            };
        }
        let exact_facts = owner.completion.manifest_digest == expected.manifest_digest
            && owner.completion.obligation_count == expected.obligation_count
            && owner.completion.settled_count == owner.completion.obligation_count;
        if !exact_facts {
            let observed_facts = owner.completion;
            return BackNoTargetAuthorityCleanupCompletionCoreOutcome::Mismatch {
                owner,
                evidence: CoreBackNoTargetAuthorityCleanupMismatchEvidence::Facts(
                    CoreBackNoTargetAuthorityCompletionFactsMismatchProjection {
                        expected,
                        observed: observed_facts,
                    },
                ),
            };
        }
        let terminal = CoreBackNoTargetAuthorityTerminalProjection {
            request: expected.request,
            completion: owner.completion,
        };
        commit_physical_and_clear_holder();
        if let Some(CorePendingBackNoTargetAuthority::Cleanup(cleanup)) =
            self.pending_back_no_target_authority.take()
        {
            let CoreBackNoTargetAuthorityTerminalPlan {
                state,
                records,
                evicted_through,
                issued_view_tasks,
                report_dispositions,
                completed_cleanup,
            } = cleanup.terminal;
            drop(cleanup.retired_host);
            self.state = state;
            self.records = records;
            self.evicted_through = evicted_through;
            self.issued_view_tasks = issued_view_tasks;
            self.report_dispositions = report_dispositions;
            self.completed_cleanup = completed_cleanup;
            self.active_branch = None;
            self.active_child_slots.clear();
        }
        BackNoTargetAuthorityCleanupCompletionCoreOutcome::Terminal(terminal)
    }

    pub(crate) fn complete_back_target_permanent_cleanup<F>(
        &mut self,
        owner: CoreBackTargetPermanentCleanupCompletionOwner,
        commit_completion: F,
    ) -> BackTargetPermanentCleanupCompletionCoreOutcome
    where
        F: FnOnce(),
    {
        let expected = owner.batch.expectation.clone();
        let observed =
            self.back_target_permanent_cleanup_completion_observed(&expected.cleanup.scope);
        let exact_receiver = expected.matches_observed(&observed);
        if !exact_receiver {
            return BackTargetPermanentCleanupCompletionCoreOutcome::Mismatch {
                owner,
                mismatch: CoreBackTargetPermanentCleanupMismatchEvidence::Receiver(
                    CoreBackTargetPermanentCleanupReceiverMismatchProjection { expected, observed },
                ),
            };
        }
        let exact_facts = owner.completion.manifest_digest == expected.cleanup.manifest_digest
            && owner.completion.obligation_count == expected.cleanup.obligation_count
            && owner.completion.settled_count == owner.completion.obligation_count;
        if !exact_facts {
            let observed_facts = owner.completion;
            return BackTargetPermanentCleanupCompletionCoreOutcome::Mismatch {
                owner,
                mismatch: CoreBackTargetPermanentCleanupMismatchEvidence::Facts(
                    CoreBackTargetPermanentCompletionFactsMismatchProjection {
                        expected: expected.cleanup,
                        observed: observed_facts,
                    },
                ),
            };
        }
        let terminal = CoreBackTargetPermanentCleanupTerminalProjection {
            request: expected.cleanup.scope.request(),
            completion: owner.completion,
        };
        let cleanup = match self.pending_back_target_permanent_cleanup.take() {
            Some(cleanup) => cleanup,
            None => {
                let observed =
                    self.back_target_permanent_cleanup_completion_observed(&expected.cleanup.scope);
                return BackTargetPermanentCleanupCompletionCoreOutcome::Mismatch {
                    owner,
                    mismatch: CoreBackTargetPermanentCleanupMismatchEvidence::Receiver(
                        CoreBackTargetPermanentCleanupReceiverMismatchProjection {
                            expected,
                            observed,
                        },
                    ),
                };
            }
        };
        let local_exact = cleanup.expectation == expected
            && cleanup.receiver == expected.cleanup
            && expected.current.current_retired_host.as_ref()
                == Some(&cleanup.retired_host.projection());
        if !local_exact {
            self.pending_back_target_permanent_cleanup = Some(cleanup);
            let observed =
                self.back_target_permanent_cleanup_completion_observed(&expected.cleanup.scope);
            return BackTargetPermanentCleanupCompletionCoreOutcome::Mismatch {
                owner,
                mismatch: CoreBackTargetPermanentCleanupMismatchEvidence::Receiver(
                    CoreBackTargetPermanentCleanupReceiverMismatchProjection { expected, observed },
                ),
            };
        }
        let CoreBackTargetPermanentCleanupCompletionOwner { batch, .. } = owner;
        let CoreBackTargetPermanentCleanupState {
            history,
            settlement,
            retired_host,
            terminal:
                CoreBackTargetPermanentTerminalPlan {
                    state,
                    records,
                    evicted_through,
                    issued_view_tasks,
                    report_dispositions,
                    completed_cleanup,
                    active_child_slots,
                },
            ..
        } = cleanup;
        commit_completion();
        drop(batch);
        drop(history);
        drop(settlement);
        drop(retired_host);
        self.state = state;
        self.records = records;
        self.evicted_through = evicted_through;
        self.issued_view_tasks = issued_view_tasks;
        self.report_dispositions = report_dispositions;
        self.completed_cleanup = completed_cleanup;
        self.active_branch = None;
        self.active_child_slots = active_child_slots;
        BackTargetPermanentCleanupCompletionCoreOutcome::Terminal(terminal)
    }

    fn prepare_back_no_target_publication(
        mut records: VecDeque<NavigationRequestRecord>,
        mut state: RouterNavigationState,
        request: NavigationRequestId,
    ) -> Result<
        CoreBackNoTargetPublicationCapability,
        CoreBackNoTargetPublicationPreparationFailureProjection,
    > {
        let Some(record) = records.iter_mut().find(|record| record.id() == request) else {
            return Err(CoreBackNoTargetPublicationPreparationFailureProjection::MissingRecord);
        };
        record
            .transition(RequestStatus::Running(NavigationPhase::CleanupPending))
            .map_err(
                CoreBackNoTargetPublicationPreparationFailureProjection::CleanupPendingTransition,
            )?;
        record
            .append_trace(NavigationTraceEvent::new(
                NavigationTraceEventKind::HistoryIntentRejected,
            ))
            .map_err(
                CoreBackNoTargetPublicationPreparationFailureProjection::HistoryIntentRejectedTrace,
            )?;
        record
            .append_trace(NavigationTraceEvent::request_phase_changed(
                NavigationPhase::CleanupPending,
            ))
            .map_err(
                CoreBackNoTargetPublicationPreparationFailureProjection::CleanupPendingTrace,
            )?;
        state.set_request(Some(RequestStatus::Running(
            NavigationPhase::CleanupPending,
        )));
        Ok(CoreBackNoTargetPublicationCapability { records, state })
    }

    fn back_no_target_publication_preparation_mismatch(
        request: NavigationRequestId,
        failure: CoreBackNoTargetPublicationPreparationFailureProjection,
    ) -> CorePreparationReceiverMismatchProjection {
        CorePreparationReceiverMismatchProjection::BackNoTargetReady(
            CoreBackNoTargetReadyMismatchProjection::PublicationPreparation { request, failure },
        )
    }

    pub(crate) fn accept_back_no_target_ready_with_canonical_zero_bind<F>(
        &mut self,
        approved: ApprovedBackNoTargetCandidate,
        manifest_digest: u64,
        obligation_count: usize,
        expected_completion_digest: u64,
        commit_canonical_zero: F,
    ) -> BackNoTargetReadyCoreOutcome
    where
        F: FnOnce(),
    {
        let (expected, observed, exact) =
            self.observe_back_no_target_logical_zero(&approved.candidate);
        let exact_approval_lineage = approved.approval.expected == expected;
        if !exact || !exact_approval_lineage {
            let expected = if exact_approval_lineage {
                expected
            } else {
                approved.approval.expected.clone()
            };
            return BackNoTargetReadyCoreOutcome::Mismatch {
                approved,
                mismatch: CorePreparationReceiverMismatchProjection::LogicalZero {
                    expected,
                    observed,
                },
            };
        }

        if obligation_count != 0 {
            return BackNoTargetReadyCoreOutcome::Mismatch {
                approved,
                mismatch: CorePreparationReceiverMismatchProjection::BackNoTargetReady(
                    CoreBackNoTargetReadyMismatchProjection::Manifest {
                        request: expected.receiver.request(),
                        expected: CoreBackNoTargetManifestCommitmentProjection {
                            manifest_digest,
                            obligation_count: 0,
                        },
                        observed: CoreBackNoTargetManifestCommitmentProjection {
                            manifest_digest,
                            obligation_count,
                        },
                    },
                ),
            };
        }

        let Some(host) = approved.approval.expected.host.clone() else {
            return BackNoTargetReadyCoreOutcome::Mismatch {
                approved,
                mismatch: CorePreparationReceiverMismatchProjection::LogicalZero {
                    expected,
                    observed,
                },
            };
        };
        let evidence = approved.approval.expected.receiver.clone();
        let request = evidence.request();
        let publication = match Self::prepare_back_no_target_publication(
            self.records.clone(),
            self.state.clone(),
            request,
        ) {
            Ok(publication) => publication,
            Err(failure) => {
                return BackNoTargetReadyCoreOutcome::Mismatch {
                    approved,
                    mismatch: Self::back_no_target_publication_preparation_mismatch(
                        request, failure,
                    ),
                };
            }
        };
        let prepared = CorePreparedBackNoTargetPublication {
            publication,
            installation: CoreBackNoTargetCleanupInstallation {
                batch: CoreBackNoTargetCleanupBatchOwner {
                    evidence,
                    host,
                    manifest_digest,
                    obligation_count,
                    expected_completion_digest,
                },
                continuation: CoreRejectedBackContinuationOwner { approved },
                action: CoreBackNoTargetCleanupAction { request },
            },
        };
        commit_canonical_zero();
        BackNoTargetReadyCoreOutcome::Prepared(prepared)
    }

    pub(crate) fn publish_prepared_back_no_target<F>(
        &mut self,
        prepared: CorePreparedBackNoTargetPublication,
        install_cleanup: F,
    ) where
        F: FnOnce(CoreBackNoTargetCleanupInstallation),
    {
        let CorePreparedBackNoTargetPublication {
            publication,
            installation,
        } = prepared;
        let CoreBackNoTargetPublicationCapability { records, state } = publication;
        install_cleanup(installation);
        self.records = records;
        self.state = state;
    }

    fn observe_back_no_target_cleanup_completion(
        &self,
        owner: &CoreBackNoTargetCleanupCompletionOwner,
    ) -> (
        CoreBackNoTargetCompletionExpectedProjection,
        CoreBackNoTargetCompletionObservedProjection,
        bool,
    ) {
        let installation = &owner.installation;
        let batch = &installation.batch;
        let approved = &installation.continuation.approved;
        let candidate = &approved.candidate;
        let request = batch.evidence.request();
        let expected = CoreBackNoTargetCompletionExpectedProjection {
            receiver: batch.evidence.clone(),
            host: batch.host.clone(),
            source: approved.approval.expected.source.clone(),
            request,
            manifest: CoreBackNoTargetManifestCommitmentProjection {
                manifest_digest: batch.manifest_digest,
                obligation_count: batch.obligation_count,
            },
            settled_count: batch.obligation_count,
            expected_completion_digest: batch.expected_completion_digest,
        };
        let preview = self.history.preview_back();
        let request_projection = match self.request(request) {
            Some(record) => CoreBackNoTargetLogicalZeroRequestProjection::Retained {
                request,
                status: record.status().clone(),
                requested: record.requested().cloned(),
                resolution: record.classify_back_resolution(&batch.evidence),
            },
            None => CoreBackNoTargetLogicalZeroRequestProjection::Missing { request },
        };
        let pending_history_preparation_error = match &self.pending_history_preparation_error {
            None => CoreBackNoTargetLogicalZeroPreparationErrorProjection::Vacant,
            Some(HistoryPreparationError::Source(_)) => {
                CoreBackNoTargetLogicalZeroPreparationErrorProjection::Source
            }
            Some(HistoryPreparationError::Binding(_)) => {
                CoreBackNoTargetLogicalZeroPreparationErrorProjection::Binding
            }
        };
        let candidate_receiver = candidate.evidence();
        let candidate_source = CoreFrozenHistorySourceProjection::project(candidate.source());
        let observed = CoreBackNoTargetCompletionObservedProjection {
            scope: self.back_target_engine_scope_projection(),
            current_host: self.state.host().cloned(),
            current_source: CoreFrozenHistorySourceProjection::project(preview.observed_source()),
            candidate_receiver: candidate_receiver.clone(),
            approval_receiver: approved.approval.expected.receiver.clone(),
            approval_host: approved.approval.expected.host.clone(),
            approval_source: approved.approval.expected.source.clone(),
            action_request: installation.action.request,
            request: request_projection,
            current_request: self.state.current_request().cloned(),
            queued_requests: self.queue.iter().map(|queued| queued.request).collect(),
            current: self
                .current
                .as_ref()
                .map(|current| current.identity.clone()),
            action: Self::back_target_action_projection(self.action.as_ref()),
            pending_no_target_back: self
                .pending_no_target_back
                .as_ref()
                .map(|pending| pending.request),
            current_history_capability: self
                .current_history_capability
                .as_ref()
                .map(HistoryCommitCapability::execution_projection),
            pending_history_preparation_error,
            prepared_publication: self
                .prepared_publication_authority
                .as_ref()
                .map(|authority| authority.owner().evidence.clone()),
            completion: owner.completion,
            preparation_failure: None,
        };
        let exact_lineage = candidate_receiver == batch.evidence
            && approved.approval.expected.receiver == batch.evidence
            && approved.approval.expected.host.as_ref() == Some(&batch.host)
            && approved.approval.expected.source == candidate_source
            && installation.action.request == request;
        let exact_receiver = candidate.matches_receiver(
            self.history_admission_world,
            self.router,
            self.history.incarnation(),
        );
        let exact_source = self
            .history
            .refreeze_back_no_target_source(candidate.source())
            .is_ok()
            && matches!(
                &preview,
                BackPreview::NoTarget(source) if source == candidate.source()
            );
        let exact_request = matches!(
            &observed.request,
            CoreBackNoTargetLogicalZeroRequestProjection::Retained {
                status: RequestStatus::Running(NavigationPhase::CleanupPending),
                requested: None,
                resolution: BackResolutionEvidenceClassification::ConsumedReplay,
                ..
            }
        ) && observed
            .queued_requests
            .iter()
            .all(|queued| *queued != request);
        let exact_owner_vacancies = observed.current.is_none()
            && matches!(
                &observed.action,
                CoreBackTargetReceiverActionProjection::Missing
            )
            && observed.pending_no_target_back.is_none()
            && self.pending_back_no_target_authority.is_none()
            && observed.current_history_capability.is_none()
            && matches!(
                &observed.pending_history_preparation_error,
                CoreBackNoTargetLogicalZeroPreparationErrorProjection::Vacant
            )
            && observed.prepared_publication.is_none();
        let exact_state = observed.current_host.as_ref() == Some(&batch.host)
            && matches!(
                &observed.current_request,
                Some(RequestStatus::Running(NavigationPhase::CleanupPending))
            );
        let exact_completion = batch.obligation_count == 0
            && owner.completion.manifest_digest == batch.manifest_digest
            && owner.completion.obligation_count == batch.obligation_count
            && owner.completion.settled_count == owner.completion.obligation_count
            && owner.completion.completion_digest == batch.expected_completion_digest;
        (
            expected,
            observed,
            exact_lineage
                && exact_receiver
                && exact_source
                && exact_request
                && exact_owner_vacancies
                && exact_state
                && exact_completion,
        )
    }

    fn prepare_back_no_target_cleanup_terminal(
        &self,
        request: NavigationRequestId,
        completion: CoreBackNoTargetCleanupCompletionFacts,
    ) -> Result<
        PreparedBackNoTargetCleanupTerminal,
        CoreBackNoTargetCompletionPreparationFailureProjection,
    > {
        let mut state = self.state.clone();
        let mut records = self.records.clone();
        let Some(record) = records.iter_mut().find(|record| record.id() == request) else {
            return Err(CoreBackNoTargetCompletionPreparationFailureProjection::MissingRecord);
        };
        record
            .append_trace(NavigationTraceEvent::cleanup_settled(None))
            .map_err(CoreBackNoTargetCompletionPreparationFailureProjection::CleanupSettledTrace)?;
        record
            .transition(RequestStatus::Failed(NavigationFailure::new(
                NavigationFailureKind::BackTargetUnavailable,
                "back target became unavailable before execution",
            )))
            .map_err(CoreBackNoTargetCompletionPreparationFailureProjection::TerminalTransition)?;
        record
            .append_trace(NavigationTraceEvent::request_terminal())
            .map_err(
                CoreBackNoTargetCompletionPreparationFailureProjection::RequestTerminalTrace,
            )?;
        state.set_request(None);

        let mut issued_view_tasks = self.issued_view_tasks.clone();
        let mut report_dispositions = self.report_dispositions.clone();
        let mut completed_cleanup = self.completed_cleanup.clone();
        let mut evicted_through = self.evicted_through;
        while records
            .iter()
            .filter(|record| is_terminal(record.status()))
            .count()
            > TERMINAL_RETENTION
        {
            let Some(index) = records
                .iter()
                .position(|record| is_terminal(record.status()))
            else {
                return Err(
                    CoreBackNoTargetCompletionPreparationFailureProjection::RetentionInvariant,
                );
            };
            let Some(evicted) = records.remove(index) else {
                return Err(
                    CoreBackNoTargetCompletionPreparationFailureProjection::RetentionInvariant,
                );
            };
            let evicted_request = evicted.id();
            issued_view_tasks.retain(|identity| identity.request != evicted_request);
            report_dispositions
                .retain(|record| record.target != DiagnosticTarget::Request(evicted_request));
            completed_cleanup.retain(|tombstone| tombstone.request != evicted_request);
            let sequence = evicted_request.sequence();
            evicted_through =
                Some(evicted_through.map_or(sequence, |current| current.max(sequence)));
        }

        Ok(PreparedBackNoTargetCleanupTerminal {
            state,
            records,
            evicted_through,
            issued_view_tasks,
            report_dispositions,
            completed_cleanup,
            terminal: CoreBackNoTargetTerminalProjection {
                request,
                completion,
            },
        })
    }

    pub(crate) fn complete_back_no_target_cleanup<F>(
        &mut self,
        owner: CoreBackNoTargetCleanupCompletionOwner,
        commit_reclaim: F,
    ) -> BackNoTargetCleanupCompletionCoreOutcome
    where
        F: FnOnce(),
    {
        let (expected, mut observed, exact) =
            self.observe_back_no_target_cleanup_completion(&owner);
        if !exact {
            return BackNoTargetCleanupCompletionCoreOutcome::Mismatch {
                owner,
                mismatch: CorePreparationReceiverMismatchProjection::BackNoTargetCompletion {
                    expected,
                    observed,
                },
            };
        }
        let request = expected.request;
        let prepared = match self.prepare_back_no_target_cleanup_terminal(request, owner.completion)
        {
            Ok(prepared) => prepared,
            Err(failure) => {
                observed.preparation_failure = Some(failure);
                return BackNoTargetCleanupCompletionCoreOutcome::Mismatch {
                    owner,
                    mismatch: CorePreparationReceiverMismatchProjection::BackNoTargetCompletion {
                        expected,
                        observed,
                    },
                };
            }
        };
        let PreparedBackNoTargetCleanupTerminal {
            state,
            records,
            evicted_through,
            issued_view_tasks,
            report_dispositions,
            completed_cleanup,
            terminal,
        } = prepared;
        commit_reclaim();
        self.state = state;
        self.records = records;
        self.evicted_through = evicted_through;
        self.issued_view_tasks = issued_view_tasks;
        self.report_dispositions = report_dispositions;
        self.completed_cleanup = completed_cleanup;
        BackNoTargetCleanupCompletionCoreOutcome::Terminal(terminal)
    }

    pub(crate) fn settle_back_no_target_source_drift_with_retirement<F>(
        &mut self,
        owner: BackNoTargetSourceDriftOwner,
        commit_retirement: F,
    ) -> BackNoTargetSourceDriftSettlementOutcome
    where
        F: FnOnce(),
    {
        let prepared = match self.prepare_back_no_target_source_drift_terminal(owner) {
            Ok(prepared) => prepared,
            Err(owner) => {
                let evidence = owner.evidence();
                return BackNoTargetSourceDriftSettlementOutcome::Mismatch { owner, evidence };
            }
        };
        commit_retirement();
        BackNoTargetSourceDriftSettlementOutcome::Settled(
            self.install_prepared_back_no_target_source_drift_terminal(prepared),
        )
    }

    pub(crate) fn settle_back_target_permanent_exact(
        &mut self,
        owner: BackTargetPermanentOwner,
    ) -> BackTargetPermanentSettlementOutcome {
        let prepared = match self.prepare_back_target_permanent_terminal(owner) {
            Ok(prepared) => prepared,
            Err(owner) => {
                let evidence = owner.evidence();
                return BackTargetPermanentSettlementOutcome::Mismatch { owner, evidence };
            }
        };
        BackTargetPermanentSettlementOutcome::Settled(
            self.install_prepared_back_target_permanent_terminal(prepared),
        )
    }

    fn prepare_back_target_permanent_terminal(
        &self,
        owner: BackTargetPermanentOwner,
    ) -> Result<PreparedBackTargetPermanentTerminal, BackTargetPermanentOwner> {
        let request = owner.request();
        let resolution = owner.resolution_evidence();
        let Some(target_location) = owner.target_location() else {
            return Err(owner);
        };
        let exact_receiver = owner.matches_receiver(
            self.history_admission_world,
            self.router,
            self.history.incarnation(),
        ) && owner.observed_source_is_current(&self.history)
            && owner.observed_baseline_is_current(&self.back_target_preparation_baseline())
            && self.action.is_none()
            && self.current.is_none()
            && self.pending_no_target_back.is_none()
            && self.pending_back_no_target_authority.is_none()
            && self.current_history_capability.is_none()
            && self.pending_history_preparation_error.is_none()
            && self.prepared_publication_authority.is_none()
            && self.state.current_request().is_none()
            && self.queue.iter().all(|queued| queued.request != request);
        if !exact_receiver {
            return Err(owner);
        }

        let Some(record_index) = self.records.iter().position(|record| {
            record.id() == request
                && record.classify_back_resolution(&resolution)
                    == BackResolutionEvidenceClassification::ConsumedReplay
                && matches!(record.status(), RequestStatus::Queued)
                && record.requested() == Some(target_location)
        }) else {
            return Err(owner);
        };

        let mut records = self.records.clone();
        let Some(record) = records.get_mut(record_index) else {
            return Err(owner);
        };
        if record
            .transition(RequestStatus::Failed(NavigationFailure::new(
                NavigationFailureKind::AuthorityLost,
                "back target authority was lost before physical splice",
            )))
            .is_err()
            || record
                .append_trace(NavigationTraceEvent::request_terminal())
                .is_err()
        {
            return Err(owner);
        }

        let mut issued_view_tasks = self.issued_view_tasks.clone();
        let mut report_dispositions = self.report_dispositions.clone();
        let mut completed_cleanup = self.completed_cleanup.clone();
        let mut evicted_through = self.evicted_through;
        while records
            .iter()
            .filter(|record| is_terminal(record.status()))
            .count()
            > TERMINAL_RETENTION
        {
            let Some(index) = records
                .iter()
                .position(|record| is_terminal(record.status()))
            else {
                return Err(owner);
            };
            let Some(evicted) = records.remove(index) else {
                return Err(owner);
            };
            let evicted_request = evicted.id();
            issued_view_tasks.retain(|identity| identity.request != evicted_request);
            report_dispositions
                .retain(|record| record.target != DiagnosticTarget::Request(evicted_request));
            completed_cleanup.retain(|tombstone| tombstone.request != evicted_request);
            let sequence = evicted_request.sequence();
            evicted_through =
                Some(evicted_through.map_or(sequence, |current| current.max(sequence)));
        }

        let terminal = match owner.into_terminal_evidence() {
            Ok(terminal) => terminal,
            Err(owner) => return Err(owner),
        };
        Ok(PreparedBackTargetPermanentTerminal {
            records,
            evicted_through,
            issued_view_tasks,
            report_dispositions,
            completed_cleanup,
            terminal,
        })
    }

    fn install_prepared_back_target_permanent_terminal(
        &mut self,
        prepared: PreparedBackTargetPermanentTerminal,
    ) -> BackTargetPermanentTerminalEvidence {
        let PreparedBackTargetPermanentTerminal {
            records,
            evicted_through,
            issued_view_tasks,
            report_dispositions,
            completed_cleanup,
            terminal,
        } = prepared;
        self.records = records;
        self.evicted_through = evicted_through;
        self.issued_view_tasks = issued_view_tasks;
        self.report_dispositions = report_dispositions;
        self.completed_cleanup = completed_cleanup;
        terminal
    }

    fn prepare_back_no_target_source_drift_terminal(
        &self,
        owner: BackNoTargetSourceDriftOwner,
    ) -> Result<PreparedBackNoTargetSourceDriftTerminal, BackNoTargetSourceDriftOwner> {
        let request = owner.request();
        let resolution = owner.resolution_evidence();
        let exact_receiver = owner.matches_receiver(
            self.history_admission_world,
            self.router,
            self.history.incarnation(),
        ) && owner.observed_source_is_current(&self.history)
            && self.action.is_none()
            && self.current.is_none()
            && self.pending_no_target_back.is_none()
            && self.pending_back_no_target_authority.is_none()
            && self.current_history_capability.is_none()
            && self.pending_history_preparation_error.is_none()
            && self.prepared_publication_authority.is_none()
            && self.state.current_request().is_none()
            && self.queue.iter().all(|queued| queued.request != request);
        if !exact_receiver {
            return Err(owner);
        }

        let Some(record_index) = self.records.iter().position(|record| {
            record.id() == request
                && record.classify_back_resolution(&resolution)
                    == BackResolutionEvidenceClassification::ConsumedReplay
                && matches!(record.status(), RequestStatus::Queued)
                && record.requested().is_none()
        }) else {
            return Err(owner);
        };

        let state = self.state.clone();
        let mut records = self.records.clone();
        let Some(record) = records.get_mut(record_index) else {
            return Err(owner);
        };
        if record
            .transition(RequestStatus::Failed(NavigationFailure::new(
                NavigationFailureKind::AuthorityLost,
                "back no-target source drifted before physical proof",
            )))
            .is_err()
            || record
                .append_trace(NavigationTraceEvent::request_terminal())
                .is_err()
        {
            return Err(owner);
        }

        let mut issued_view_tasks = self.issued_view_tasks.clone();
        let mut report_dispositions = self.report_dispositions.clone();
        let mut completed_cleanup = self.completed_cleanup.clone();
        let mut evicted_through = self.evicted_through;
        while records
            .iter()
            .filter(|record| is_terminal(record.status()))
            .count()
            > TERMINAL_RETENTION
        {
            let Some(index) = records
                .iter()
                .position(|record| is_terminal(record.status()))
            else {
                return Err(owner);
            };
            let Some(evicted) = records.remove(index) else {
                return Err(owner);
            };
            let evicted_request = evicted.id();
            issued_view_tasks.retain(|identity| identity.request != evicted_request);
            report_dispositions
                .retain(|record| record.target != DiagnosticTarget::Request(evicted_request));
            completed_cleanup.retain(|tombstone| tombstone.request != evicted_request);
            let sequence = evicted_request.sequence();
            evicted_through =
                Some(evicted_through.map_or(sequence, |current| current.max(sequence)));
        }

        Ok(PreparedBackNoTargetSourceDriftTerminal {
            state,
            records,
            evicted_through,
            issued_view_tasks,
            report_dispositions,
            completed_cleanup,
            terminal: owner.into_terminal_evidence(),
        })
    }

    fn install_prepared_back_no_target_source_drift_terminal(
        &mut self,
        prepared: PreparedBackNoTargetSourceDriftTerminal,
    ) -> BackNoTargetSourceDriftTerminalEvidence {
        let PreparedBackNoTargetSourceDriftTerminal {
            state,
            records,
            evicted_through,
            issued_view_tasks,
            report_dispositions,
            completed_cleanup,
            terminal,
        } = prepared;
        self.state = state;
        self.records = records;
        self.evicted_through = evicted_through;
        self.issued_view_tasks = issued_view_tasks;
        self.report_dispositions = report_dispositions;
        self.completed_cleanup = completed_cleanup;
        terminal
    }

    fn prepare_back_target_start(
        &mut self,
        owner: VerifiedBackTargetExecutionOwner,
        record_index: usize,
    ) -> PreparedBackTargetStart {
        let request = owner.evidence().request();
        let (continuation, reservation, destination, expected_baseline) =
            owner.into_execution_parts();
        if let Err(observed_source) = self.history.refreeze_back_source(&destination) {
            let observed_baseline = self.back_target_preparation_baseline();
            return PreparedBackTargetStart::Permanent(
                BackTargetPermanentOwner::source_refreeze_drift(
                    continuation,
                    reservation,
                    destination,
                    observed_source,
                    expected_baseline,
                    observed_baseline,
                ),
            );
        }
        #[cfg(test)]
        if std::mem::take(&mut self.testing_back_target_source_drift_before_freeze_once) {
            self.history.testing_move_cursor_back();
        }
        let observed_baseline = self.back_target_preparation_baseline();

        let authority_lost = |continuation, reservation, destination, cause| {
            PreparedBackTargetStart::Permanent(BackTargetPermanentOwner::authority_lost(
                continuation,
                reservation,
                destination,
                cause,
                expected_baseline.clone(),
                observed_baseline.clone(),
            ))
        };

        if let Err(cause) = expected_baseline.classify_refreeze(&observed_baseline) {
            return authority_lost(continuation, reservation, destination, cause);
        }
        let records = self.records.clone();
        let mut record = records[record_index].clone();

        let Some(host) = self.state.host().cloned() else {
            return authority_lost(
                continuation,
                reservation,
                destination,
                BackTargetAuthorityLossCause::Host,
            );
        };
        let Some(top_slot) = self.state.top_slot() else {
            return authority_lost(
                continuation,
                reservation,
                destination,
                BackTargetAuthorityLossCause::Host,
            );
        };
        if host.router_identity() != self.router {
            return authority_lost(
                continuation,
                reservation,
                destination,
                BackTargetAuthorityLossCause::Host,
            );
        }

        let location = destination.target_location().clone();
        let Ok(branch) = self.matcher.match_location(&location) else {
            return authority_lost(
                continuation,
                reservation,
                destination,
                BackTargetAuthorityLossCause::TargetMatch,
            );
        };
        if record
            .set_matched(branch.branch_fingerprint().clone())
            .is_err()
        {
            return authority_lost(
                continuation,
                reservation,
                destination,
                BackTargetAuthorityLossCause::RequestRecord,
            );
        }
        if branch.routes().len() > reservation.frames().len() {
            return authority_lost(
                continuation,
                reservation,
                destination,
                BackTargetAuthorityLossCause::MaximumDepth,
            );
        }

        let reserved_frames = reservation.frames()[..branch.routes().len()].to_vec();
        let reserved_frame_ids = reserved_frames
            .iter()
            .map(|reserved| reserved.frame())
            .collect::<Vec<_>>();
        let active_location = self.state.active().location();
        let reconciliation_fragment = active_location.is_some_and(|active| {
            active.path() == location.path()
                && active.search() == location.search()
                && active.fragment() != location.fragment()
        }) && self
            .current_active_branch()
            .is_some_and(|active| active.branch_fingerprint() == branch.branch_fingerprint());
        let reconciliation = match RouteReconciliationProjection::from_running(
            self.router,
            host.host(),
            host.epoch(),
            top_slot,
            active_location,
            self.current_active_branch(),
            self.state.active().frames(),
            &self.active_child_slots,
            &branch,
            &reserved_frame_ids,
            reconciliation_fragment,
        ) {
            Ok(reconciliation) => reconciliation,
            Err(ReconciliationProjectionError::InvalidActiveBaseline) => {
                return authority_lost(
                    continuation,
                    reservation,
                    destination,
                    BackTargetAuthorityLossCause::ActiveBaseline,
                );
            }
            Err(ReconciliationProjectionError::GenerationExhausted) => {
                return authority_lost(
                    continuation,
                    reservation,
                    destination,
                    BackTargetAuthorityLossCause::Generation,
                );
            }
        };

        let mut target_frames = reconciliation
            .preserved()
            .iter()
            .map(|frame| frame.version().clone())
            .collect::<Vec<_>>();
        let cleanup_roots = reconciliation
            .leaving()
            .iter()
            .map(|frame| frame.version().clone())
            .collect::<Vec<_>>();
        let root_frame = reconciliation
            .preserved()
            .iter()
            .find(|frame| frame.branch_index() == 0)
            .map(|frame| frame.version().clone())
            .or_else(|| {
                reconciliation
                    .entering()
                    .iter()
                    .find(|frame| frame.branch_index() == 0)
                    .map(|entering| {
                        RouteFrameVersion::new(
                            self.router,
                            host.host(),
                            top_slot,
                            FrameOrigin::MatchedRoute(entering.route()),
                            entering.frame(),
                            entering.generation(),
                        )
                    })
            });
        let Some(root_frame) = root_frame else {
            return authority_lost(
                continuation,
                reservation,
                destination,
                BackTargetAuthorityLossCause::ActiveBaseline,
            );
        };
        let Some(root_reservation) = reserved_frames.first() else {
            return authority_lost(
                continuation,
                reservation,
                destination,
                BackTargetAuthorityLossCause::MaximumDepth,
            );
        };
        let root_attempt = NormalAttemptIdentity::new(
            host.host(),
            host.epoch(),
            top_slot,
            root_reservation.frame_task(),
            root_frame.frame(),
            root_frame.generation(),
            root_frame.origin().clone(),
            0,
        );
        let execution = HistoryExecutionBinding::new(
            self.router,
            request,
            root_attempt,
            reservation.transaction(),
            reservation.cleanup_batch(),
        );
        let execution_projection = execution.projection();

        let Some(entering) = reconciliation.entering().first().cloned() else {
            let Some(leaf_index) = target_frames.len().checked_sub(1) else {
                return authority_lost(
                    continuation,
                    reservation,
                    destination,
                    BackTargetAuthorityLossCause::ActiveBaseline,
                );
            };
            let Some(leaf) = branch.routes().get(leaf_index) else {
                return authority_lost(
                    continuation,
                    reservation,
                    destination,
                    BackTargetAuthorityLossCause::ActiveBaseline,
                );
            };
            let Some(reserved) = reserved_frames.get(leaf_index) else {
                return authority_lost(
                    continuation,
                    reservation,
                    destination,
                    BackTargetAuthorityLossCause::MaximumDepth,
                );
            };
            let frame = target_frames[leaf_index].clone();
            let identity = FrameTaskIdentity {
                router: self.router,
                host: host.host(),
                host_epoch: host.epoch(),
                slot: frame.slot(),
                request,
                transaction: reservation.transaction(),
                frame_task: reserved.frame_task(),
                frame: frame.frame(),
                generation: frame.generation(),
                origin: FrameOrigin::MatchedRoute(leaf.route()),
                branch_index: leaf_index,
            };
            let target = if leaf_index == 0 {
                FrameMountTarget::TopSlot {
                    host: host.clone(),
                    slot: top_slot,
                }
            } else {
                let Some(parent) = target_frames.get(leaf_index - 1).cloned() else {
                    return authority_lost(
                        continuation,
                        reservation,
                        destination,
                        BackTargetAuthorityLossCause::ActiveBaseline,
                    );
                };
                FrameMountTarget::ChildOutlet {
                    parent,
                    outlet: leaf.slot().clone(),
                }
            };
            let phase = NavigationPhase::ReadyToSwitch;
            if record
                .transition(RequestStatus::Running(phase.clone()))
                .is_err()
                || record
                    .append_trace(NavigationTraceEvent::request_phase_changed(phase.clone()))
                    .is_err()
            {
                return authority_lost(
                    continuation,
                    reservation,
                    destination,
                    BackTargetAuthorityLossCause::RequestRecord,
                );
            }
            let mut state = self.state.clone();
            state.set_request(Some(RequestStatus::Running(phase)));
            let history_capability = match self.history.freeze_back(execution, destination) {
                Ok(capability) => capability,
                Err(drift) => {
                    return PreparedBackTargetStart::Permanent(
                        BackTargetPermanentOwner::source_drift(
                            continuation,
                            reservation,
                            drift,
                            expected_baseline,
                            observed_baseline,
                        ),
                    );
                }
            };
            let current = CurrentWork {
                identity: identity.clone(),
                branch: Some(branch),
                location,
                reservation: reserved_frames,
                target_frames,
                applied_frames: Vec::new(),
                current_target: target,
                pending_mount: None,
                stage: WorkStage::ReadyToCommit,
                cleanup_batch: reservation.cleanup_batch(),
                cleanup_roots,
                attempt: AttemptKind::Normal(NavigationCommandKind::Back),
                reconciliation: Some(reconciliation),
                retired_host: None,
            };
            let mut records = records;
            records[record_index] = record;
            return PreparedBackTargetStart::Ready(PreparedBackTargetReady {
                installation: PreparedBackTargetInstallation {
                    current,
                    records,
                    state,
                    issued_view_tasks: self.issued_view_tasks.clone(),
                },
                history_capability,
                action: BackTargetResumeAction::Commit(CommitTicket { identity }),
                execution: execution_projection,
                continuation,
            });
        };

        let branch_index = entering.branch_index();
        let Some(matched) = branch.routes().get(branch_index).cloned() else {
            return authority_lost(
                continuation,
                reservation,
                destination,
                BackTargetAuthorityLossCause::ActiveBaseline,
            );
        };
        let Some(reserved) = reserved_frames.get(branch_index) else {
            return authority_lost(
                continuation,
                reservation,
                destination,
                BackTargetAuthorityLossCause::MaximumDepth,
            );
        };
        let origin = FrameOrigin::MatchedRoute(entering.route());
        if entering.slot() == ProjectedEnteringSlot::MountNew {
            let Some(parent_index) = branch_index.checked_sub(1) else {
                return authority_lost(
                    continuation,
                    reservation,
                    destination,
                    BackTargetAuthorityLossCause::ActiveBaseline,
                );
            };
            let Some(parent) = target_frames.get(parent_index).cloned() else {
                return authority_lost(
                    continuation,
                    reservation,
                    destination,
                    BackTargetAuthorityLossCause::ActiveBaseline,
                );
            };
            let outlet = matched.slot().clone();
            let identity = FrameTaskIdentity {
                router: self.router,
                host: host.host(),
                host_epoch: host.epoch(),
                slot: parent.slot(),
                request,
                transaction: reservation.transaction(),
                frame_task: reserved.frame_task(),
                frame: parent.frame(),
                generation: parent.generation(),
                origin: parent.origin().clone(),
                branch_index: parent_index,
            };
            let target = if parent_index == 0 {
                FrameMountTarget::TopSlot {
                    host: host.clone(),
                    slot: top_slot,
                }
            } else {
                let Some(grandparent) = target_frames.get(parent_index - 1).cloned() else {
                    return authority_lost(
                        continuation,
                        reservation,
                        destination,
                        BackTargetAuthorityLossCause::ActiveBaseline,
                    );
                };
                let Some(parent_route) = branch.routes().get(parent_index) else {
                    return authority_lost(
                        continuation,
                        reservation,
                        destination,
                        BackTargetAuthorityLossCause::ActiveBaseline,
                    );
                };
                FrameMountTarget::ChildOutlet {
                    parent: grandparent,
                    outlet: parent_route.slot().clone(),
                }
            };
            let phase = NavigationPhase::ResolvingMount;
            if record
                .transition(RequestStatus::Running(phase.clone()))
                .is_err()
                || record
                    .append_trace(NavigationTraceEvent::request_phase_changed(phase.clone()))
                    .is_err()
            {
                return authority_lost(
                    continuation,
                    reservation,
                    destination,
                    BackTargetAuthorityLossCause::RequestRecord,
                );
            }
            let mut success_record = record.clone();
            if success_record
                .append_trace(NavigationTraceEvent::mount_requested(
                    parent.clone(),
                    outlet.clone(),
                ))
                .is_err()
            {
                return authority_lost(
                    continuation,
                    reservation,
                    destination,
                    BackTargetAuthorityLossCause::RequestRecord,
                );
            }
            let mut failure_record = record;
            let failure_phase = NavigationPhase::CleanupPending;
            if failure_record
                .transition(RequestStatus::Running(failure_phase.clone()))
                .is_err()
                || failure_record
                    .append_trace(NavigationTraceEvent::request_phase_changed(
                        failure_phase.clone(),
                    ))
                    .is_err()
            {
                return authority_lost(
                    continuation,
                    reservation,
                    destination,
                    BackTargetAuthorityLossCause::RequestRecord,
                );
            }
            let mut state = self.state.clone();
            state.set_request(Some(RequestStatus::Running(phase)));
            let history_capability = match self.history.freeze_back(execution, destination) {
                Ok(capability) => capability,
                Err(drift) => {
                    return PreparedBackTargetStart::Permanent(
                        BackTargetPermanentOwner::source_drift(
                            continuation,
                            reservation,
                            drift,
                            expected_baseline,
                            observed_baseline,
                        ),
                    );
                }
            };
            let mut current = CurrentWork {
                identity: identity.clone(),
                branch: Some(branch),
                location,
                reservation: reserved_frames,
                target_frames,
                applied_frames: Vec::new(),
                current_target: target,
                pending_mount: Some(PendingMount {
                    child_index: branch_index,
                    parent: parent.clone(),
                    outlet: outlet.clone(),
                    from_preserved_parent: true,
                }),
                stage: WorkStage::AwaitingMount,
                cleanup_batch: reservation.cleanup_batch(),
                cleanup_roots,
                attempt: AttemptKind::Normal(NavigationCommandKind::Back),
                reconciliation: Some(reconciliation),
                retired_host: None,
            };
            let action_parent = parent.clone();
            let action_outlet = outlet.clone();
            let issued_view_tasks = self.issued_view_tasks.clone();
            let mut success_records = records.clone();
            success_records[record_index] = success_record;
            let mut failure_records = records;
            failure_records[record_index] = failure_record;
            return match self.mount.issue(
                &mut self.ids,
                host,
                request,
                reservation.transaction(),
                parent,
                outlet,
            ) {
                Ok(token) => PreparedBackTargetStart::Ready(PreparedBackTargetReady {
                    installation: PreparedBackTargetInstallation {
                        current,
                        records: success_records,
                        state,
                        issued_view_tasks,
                    },
                    history_capability,
                    action: BackTargetResumeAction::MountRequest(FrameMountRequest {
                        token,
                        request,
                        parent: action_parent,
                        outlet: action_outlet,
                    }),
                    execution: execution_projection,
                    continuation,
                }),
                Err(error) => {
                    drop(history_capability);
                    let message = match error {
                        MountError::Exhausted => "child mount request identity exhausted",
                        MountError::Foreign | MountError::Stale | MountError::WrongPhase => {
                            "child mount request issuer rejected internal identity"
                        }
                    };
                    let failure = FailureCleanupRequest {
                        identity: current.identity.clone(),
                        expected: PendingFailureKind::MountIssue,
                        logical_roots: current.applied_frames.iter().rev().cloned().collect(),
                        failure: NavigationFailure::new(NavigationFailureKind::Mount, message),
                        continuation: FailureCleanupContinuation::Terminal,
                    };
                    current.pending_mount = None;
                    current.stage = WorkStage::FailureCleanupManifestPending {
                        expected: failure.clone(),
                    };
                    state.set_request(Some(RequestStatus::Running(failure_phase)));
                    PreparedBackTargetStart::FailureCleanup(PreparedBackTargetFailureCleanup {
                        installation: PreparedBackTargetInstallation {
                            current,
                            records: failure_records,
                            state,
                            issued_view_tasks,
                        },
                        request: failure,
                    })
                }
            };
        }

        let ProjectedEnteringSlot::Existing(slot) = entering.slot() else {
            return authority_lost(
                continuation,
                reservation,
                destination,
                BackTargetAuthorityLossCause::ActiveBaseline,
            );
        };
        let target_frame = RouteFrameVersion::new(
            self.router,
            host.host(),
            slot,
            origin.clone(),
            entering.frame(),
            entering.generation(),
        );
        let target = if branch_index == 0 {
            FrameMountTarget::TopSlot {
                host: host.clone(),
                slot: top_slot,
            }
        } else {
            let Some(parent) = target_frames.get(branch_index - 1).cloned() else {
                return authority_lost(
                    continuation,
                    reservation,
                    destination,
                    BackTargetAuthorityLossCause::ActiveBaseline,
                );
            };
            FrameMountTarget::RegisteredChildSlot {
                parent,
                outlet: matched.slot().clone(),
            }
        };
        let Some(view) = self.matcher.view_binding(matched.route()).cloned() else {
            return authority_lost(
                continuation,
                reservation,
                destination,
                BackTargetAuthorityLossCause::ViewBinding,
            );
        };
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
            slot,
            request,
            transaction: reservation.transaction(),
            frame_task: reserved.frame_task(),
            frame: entering.frame(),
            generation: entering.generation(),
            origin,
            branch_index,
        };
        target_frames.push(target_frame.clone());
        if record
            .append_trace(NavigationTraceEvent::frame_task_issued(
                target_frame.clone(),
            ))
            .is_err()
        {
            return authority_lost(
                continuation,
                reservation,
                destination,
                BackTargetAuthorityLossCause::RequestRecord,
            );
        }
        let phase = NavigationPhase::ApplyingFrame(branch_index);
        if record
            .transition(RequestStatus::Running(phase.clone()))
            .is_err()
            || record
                .append_trace(NavigationTraceEvent::request_phase_changed(phase.clone()))
                .is_err()
        {
            return authority_lost(
                continuation,
                reservation,
                destination,
                BackTargetAuthorityLossCause::RequestRecord,
            );
        }
        let mut state = self.state.clone();
        state.set_request(Some(RequestStatus::Running(phase)));
        let mut issued_view_tasks = self.issued_view_tasks.clone();
        issued_view_tasks.push(identity.clone());
        let history_capability = match self.history.freeze_back(execution, destination) {
            Ok(capability) => capability,
            Err(drift) => {
                return PreparedBackTargetStart::Permanent(BackTargetPermanentOwner::source_drift(
                    continuation,
                    reservation,
                    drift,
                    expected_baseline,
                    observed_baseline,
                ));
            }
        };
        let current = CurrentWork {
            identity: identity.clone(),
            branch: Some(branch),
            location: location.clone(),
            reservation: reserved_frames,
            target_frames,
            applied_frames: Vec::new(),
            current_target: target.clone(),
            pending_mount: None,
            stage: WorkStage::Applying,
            cleanup_batch: reservation.cleanup_batch(),
            cleanup_roots,
            attempt: AttemptKind::Normal(NavigationCommandKind::Back),
            reconciliation: Some(reconciliation),
            retired_host: None,
        };
        let mut records = records;
        records[record_index] = record;
        PreparedBackTargetStart::Ready(PreparedBackTargetReady {
            installation: PreparedBackTargetInstallation {
                current,
                records,
                state,
                issued_view_tasks,
            },
            history_capability,
            action: BackTargetResumeAction::FrameTask(FrameTask {
                identity,
                frame: target_frame,
                target,
                location,
                matched: Some(matched),
                view,
                input: Some(input),
            }),
            execution: execution_projection,
            continuation,
        })
    }

    fn install_prepared_back_target_start(
        &mut self,
        prepared: PreparedBackTargetStart,
    ) -> BackTargetResumeOutcome {
        match prepared {
            PreparedBackTargetStart::Ready(prepared) => {
                let PreparedBackTargetReady {
                    installation,
                    history_capability,
                    action,
                    execution,
                    continuation,
                } = prepared;
                let PreparedBackTargetInstallation {
                    current,
                    records,
                    state,
                    issued_view_tasks,
                } = installation;
                self.current = Some(current);
                self.records = records;
                self.state = state;
                self.issued_view_tasks = issued_view_tasks;
                self.current_history_capability = Some(history_capability);
                BackTargetResumeOutcome::Resumed(BackTargetStart::Ready(BackTargetResume {
                    action,
                    execution,
                    continuation,
                }))
            }
            PreparedBackTargetStart::FailureCleanup(prepared) => {
                let PreparedBackTargetFailureCleanup {
                    installation,
                    request,
                } = prepared;
                let PreparedBackTargetInstallation {
                    current,
                    records,
                    state,
                    issued_view_tasks,
                } = installation;
                self.current = Some(current);
                self.records = records;
                self.state = state;
                self.issued_view_tasks = issued_view_tasks;
                self.current_history_capability = None;
                BackTargetResumeOutcome::Resumed(BackTargetStart::FailureCleanup(request))
            }
            PreparedBackTargetStart::Permanent(owner) => BackTargetResumeOutcome::Permanent(owner),
        }
    }

    pub(crate) fn collect(&mut self) {
        if self.action.is_none()
            && self.current.is_none()
            && self.pending_no_target_back.is_none()
            && self.pending_back_no_target_authority.is_none()
        {
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

    fn inspect_applied_report(&self, report: AppliedFrameReport) -> AppliedReportInspection {
        let next_outlet = self.current.as_ref().and_then(|current| {
            current
                .branch
                .as_ref()
                .and_then(|branch| branch.routes().get(report.identity.branch_index + 1))
                .map(|matched| matched.slot().clone())
        });
        let predicates = AppliedReportPredicateProjection {
            next_outlet: next_outlet.clone(),
            reconciliation: self
                .current
                .as_ref()
                .and_then(|current| current.reconciliation.clone()),
            active_baseline: self.back_target_preparation_baseline(),
        };
        if let Err(error) = self.inspect_view_report_identity(&report.identity) {
            return AppliedReportInspection::Rejected {
                report,
                rejection: AppliedReportRejection::Identity(error),
                predicates,
            };
        }
        if !self.current_reconciliation_matches_active() {
            return AppliedReportInspection::ReconciliationDrift;
        }
        let sorted_unique = report.outlets.windows(2).all(|pair| pair[0] < pair[1]);
        if !sorted_unique || (report.mode == AppliedReportMode::LegacyLeaf && next_outlet.is_some())
        {
            return AppliedReportInspection::Rejected {
                report,
                rejection: AppliedReportRejection::InvalidInput,
                predicates,
            };
        }
        let frame = self
            .current
            .as_ref()
            .and_then(|current| current.target_frames.last())
            .expect("an inspected task has one target frame")
            .clone();
        AppliedReportInspection::Validated(ValidatedAppliedReport {
            report,
            frame,
            next_outlet,
        })
    }

    pub(crate) fn accept_applied_report(
        &mut self,
        report: AppliedFrameReport,
    ) -> Result<(), ReportError> {
        match self.inspect_applied_report(report) {
            AppliedReportInspection::Rejected {
                report,
                rejection: AppliedReportRejection::Identity(error),
                predicates: _,
            } => {
                let validation = self.validate_view_report_identity(&report.identity);
                debug_assert_eq!(validation, Err(error));
                if self.action.is_none()
                    && self
                        .current
                        .as_ref()
                        .is_some_and(|current| current.stage == WorkStage::Applying)
                {
                    self.reissue_current_frame_task();
                }
                Err(error)
            }
            AppliedReportInspection::Rejected {
                rejection: AppliedReportRejection::InvalidInput,
                ..
            } => {
                self.reissue_current_frame_task();
                Err(ReportError::Invalid)
            }
            AppliedReportInspection::ReconciliationDrift => {
                self.install_current_reconciliation_drift_failure();
                Err(ReportError::Stale)
            }
            AppliedReportInspection::Validated(plan) => {
                let transition = self.apply_validated_applied_report(plan);
                self.install_applied_report_transition(transition);
                Ok(())
            }
        }
    }

    fn install_applied_report_transition(&mut self, transition: AppliedReportTransition) {
        self.action = Some(match transition {
            AppliedReportTransition::MountRequest(request) => EngineAction::MountRequest(request),
            AppliedReportTransition::Commit(ticket) => EngineAction::Commit(ticket),
            AppliedReportTransition::FailureCleanup(request) => {
                EngineAction::FailureCleanup(request)
            }
        });
    }

    fn apply_validated_applied_report(
        &mut self,
        plan: ValidatedAppliedReport,
    ) -> AppliedReportTransition {
        let ValidatedAppliedReport {
            report,
            frame,
            next_outlet,
        } = plan;
        let request = report.identity.request;
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
                    return AppliedReportTransition::FailureCleanup(failure);
                }
            };
            let child_index = report.identity.branch_index + 1;
            let current = self.current.as_mut().expect("report was preflighted");
            current.identity = report.identity;
            current.pending_mount = Some(PendingMount {
                child_index,
                parent: frame.clone(),
                outlet: outlet.clone(),
                from_preserved_parent: false,
            });
            current.stage = WorkStage::AwaitingMount;
            self.record_mut(request)
                .expect("running record is retained")
                .append_trace(NavigationTraceEvent::mount_requested(
                    frame.clone(),
                    outlet.clone(),
                ))
                .expect("running trace is open");
            AppliedReportTransition::MountRequest(FrameMountRequest {
                token,
                request,
                parent: frame,
                outlet,
            })
        } else {
            let current = self.current.as_mut().expect("report was preflighted");
            current.identity = report.identity.clone();
            current.stage = WorkStage::ReadyToCommit;
            self.transition_with_trace(request, NavigationPhase::ReadyToSwitch);
            AppliedReportTransition::Commit(CommitTicket {
                identity: report.identity,
            })
        }
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
        let reservation_matches = if pending.from_preserved_parent {
            current
                .reservation
                .get(pending.child_index)
                .is_some_and(|reserved| reserved.frame_task() == current.identity.frame_task)
        } else {
            current
                .reservation
                .get(current.identity.branch_index)
                .is_some_and(|reserved| {
                    reserved.frame_task() == current.identity.frame_task
                        && reserved.frame() == current.identity.frame
                })
        };
        if !reservation_matches {
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
        let entering_start = current
            .reconciliation
            .as_ref()
            .map_or(0, |projection| projection.preserved().len());
        if current_parent != pending.parent
            || current.target_frames.len() != child_index
            || current.target_frames.last() != Some(&pending.parent)
            || entering_start > current.target_frames.len()
            || current.applied_frames != current.target_frames[entering_start..]
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
        let current_target_is_exact = if pending.from_preserved_parent {
            current.reconciliation.as_ref().is_some_and(|projection| {
                projection.preserved().iter().any(|preserved| {
                    preserved.branch_index() == current.identity.branch_index
                        && preserved.version() == &pending.parent
                }) && projection.entering().first().is_some_and(|entering| {
                    entering.branch_index() == pending.child_index
                        && entering.slot() == ProjectedEnteringSlot::MountNew
                })
            })
        } else {
            match &current.current_target {
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
                FrameMountTarget::RegisteredChildSlot { parent, outlet } => {
                    current.identity.branch_index > 0
                        && current
                            .target_frames
                            .get(current.identity.branch_index - 1)
                            .is_some_and(|expected_parent| expected_parent == parent)
                        && current_route.slot() == outlet
                }
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
        if request.logical_roots != seal.logical_roots() {
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

    fn back_target_engine_scope_projection(&self) -> CoreBackTargetEngineScopeProjection {
        CoreBackTargetEngineScopeProjection {
            world: self.history_admission_world,
            router: self.router,
            history: self.history.incarnation(),
        }
    }

    fn back_target_permanent_cleanup_completion_observed(
        &self,
        scope: &BackResolutionScopeProjection,
    ) -> CoreBackTargetPermanentCleanupCompletionObservedProjection {
        let resolution = BackResolutionEvidence::observed(scope);
        let (request, resolution) = match self.request(scope.request()) {
            Some(record) => (
                Some(Self::back_target_permanent_request_projection(record)),
                Some(record.classify_back_resolution(&resolution)),
            ),
            None => (None, None),
        };
        CoreBackTargetPermanentCleanupCompletionObservedProjection {
            cleanup: self
                .pending_back_target_permanent_cleanup
                .as_ref()
                .map(|cleanup| cleanup.receiver.clone()),
            expectation: self
                .pending_back_target_permanent_cleanup
                .as_ref()
                .map(|cleanup| cleanup.expectation.clone()),
            scope: self.back_target_engine_scope_projection(),
            current: self.back_target_permanent_current_state_projection(),
            request,
            resolution,
        }
    }

    fn back_target_failure_cleanup_request_projection(
        request: &FailureCleanupRequest,
    ) -> CoreBackTargetFailureCleanupRequestProjection {
        CoreBackTargetFailureCleanupRequestProjection {
            identity: request.identity.clone(),
            expected: request.expected.clone(),
            logical_roots: request.logical_roots.clone(),
            failure: request.failure.clone(),
            continuation: request.continuation,
        }
    }

    fn back_target_action_projection(
        action: Option<&EngineAction>,
    ) -> CoreBackTargetReceiverActionProjection {
        match action {
            Some(EngineAction::FrameTask(task)) => {
                CoreBackTargetReceiverActionProjection::FrameTask(task.identity.clone())
            }
            Some(EngineAction::MountRequest(request)) => {
                CoreBackTargetReceiverActionProjection::MountRequest {
                    token: request.token.receiver_projection(),
                    request: request.request,
                    parent: request.parent.clone(),
                    outlet: request.outlet.clone(),
                }
            }
            Some(EngineAction::FailureCleanup(request)) => {
                CoreBackTargetReceiverActionProjection::FailureCleanup(
                    Self::back_target_failure_cleanup_request_projection(request),
                )
            }
            Some(EngineAction::Commit(ticket)) => {
                CoreBackTargetReceiverActionProjection::Commit(ticket.identity.clone())
            }
            Some(EngineAction::BackResolution(_)) => {
                CoreBackTargetReceiverActionProjection::BackResolution
            }
            None => CoreBackTargetReceiverActionProjection::Missing,
        }
    }

    fn back_target_physical_projection(
        physical: CoreBackTargetPhysicalReceiverRef<'_>,
    ) -> CoreBackTargetReceiverActionProjection {
        match physical {
            CoreBackTargetPhysicalReceiverRef::FrameTask(task) => {
                CoreBackTargetReceiverActionProjection::FrameTask(task.identity.clone())
            }
            CoreBackTargetPhysicalReceiverRef::MountRequest(request) => {
                CoreBackTargetReceiverActionProjection::MountRequest {
                    token: request.token.receiver_projection(),
                    request: request.request,
                    parent: request.parent.clone(),
                    outlet: request.outlet.clone(),
                }
            }
            CoreBackTargetPhysicalReceiverRef::Commit(ticket) => {
                CoreBackTargetReceiverActionProjection::Commit(ticket.identity.clone())
            }
        }
    }

    fn back_target_stage_projection(
        current: Option<&CurrentWork>,
    ) -> CoreBackTargetReceiverStageProjection {
        match current.map(|current| &current.stage) {
            Some(WorkStage::Applying) => CoreBackTargetReceiverStageProjection::Applying,
            Some(WorkStage::AwaitingMount) => CoreBackTargetReceiverStageProjection::AwaitingMount,
            Some(WorkStage::AwaitingReceipt) => {
                CoreBackTargetReceiverStageProjection::AwaitingReceipt
            }
            Some(WorkStage::ConfirmingMount) => {
                CoreBackTargetReceiverStageProjection::ConfirmingMount
            }
            Some(WorkStage::ReadyToCommit) => CoreBackTargetReceiverStageProjection::ReadyToCommit,
            Some(WorkStage::CommitSealed { .. }) => {
                CoreBackTargetReceiverStageProjection::CommitSealed
            }
            Some(WorkStage::PostCommitCleanup { .. }) => {
                CoreBackTargetReceiverStageProjection::PostCommitCleanup
            }
            Some(WorkStage::FailureCleanupManifestPending { expected }) => {
                CoreBackTargetReceiverStageProjection::FailureCleanupManifestPending(
                    Self::back_target_failure_cleanup_request_projection(expected),
                )
            }
            Some(WorkStage::FailureCleanupPrepared { .. }) => {
                CoreBackTargetReceiverStageProjection::FailureCleanupPrepared
            }
            None => CoreBackTargetReceiverStageProjection::Missing,
        }
    }

    fn back_target_attempt_projection(
        current: Option<&CurrentWork>,
    ) -> CoreBackTargetReceiverAttemptProjection {
        match current.map(|current| &current.attempt) {
            Some(AttemptKind::Normal(NavigationCommandKind::Push)) => {
                CoreBackTargetReceiverAttemptProjection::NormalPush
            }
            Some(AttemptKind::Normal(NavigationCommandKind::Replace)) => {
                CoreBackTargetReceiverAttemptProjection::NormalReplace
            }
            Some(AttemptKind::Normal(NavigationCommandKind::Back)) => {
                CoreBackTargetReceiverAttemptProjection::NormalBack
            }
            Some(AttemptKind::Fallback { original_failure }) => {
                CoreBackTargetReceiverAttemptProjection::Fallback {
                    original_failure: original_failure.clone(),
                }
            }
            None => CoreBackTargetReceiverAttemptProjection::Missing,
        }
    }

    fn back_target_mount_context_projection(&self) -> CoreBackTargetMountContextProjection {
        CoreBackTargetMountContextProjection {
            router: self.router,
            host: self.state.host().cloned(),
            top_slot: self.state.top_slot(),
            current: self
                .current
                .as_ref()
                .map(|current| CoreBackTargetMountCurrentProjection {
                    identity: current.identity.clone(),
                    branch_routes: current
                        .branch
                        .as_ref()
                        .map(|branch| branch.routes().to_vec()),
                    reservation: current
                        .reservation
                        .iter()
                        .map(|reserved| CoreBackTargetReservedFrameProjection {
                            frame_task: reserved.frame_task(),
                            frame: reserved.frame(),
                        })
                        .collect(),
                    target_frames: current.target_frames.clone(),
                    applied_frames: current.applied_frames.clone(),
                    current_target: current.current_target.clone(),
                    pending_mount: current.pending_mount.as_ref().map(|pending| {
                        CoreBackTargetPendingMountProjection {
                            child_index: pending.child_index,
                            parent: pending.parent.clone(),
                            outlet: pending.outlet.clone(),
                            from_preserved_parent: pending.from_preserved_parent,
                        }
                    }),
                    stage: Self::back_target_stage_projection(Some(current)),
                    reconciliation: current.reconciliation.clone(),
                }),
        }
    }

    fn classify_back_target_mount_context(
        context: &CoreBackTargetMountContextProjection,
        issuer: &MountIssuerRequestProjection,
        request: NavigationRequestId,
        parent: &RouteFrameVersion,
        outlet: &RouteSlotKey,
    ) -> (
        CoreBackTargetMountContextDisposition,
        MountIssuerContextDisposition,
    ) {
        let Some(current) = context.current.as_ref() else {
            return (
                CoreBackTargetMountContextDisposition::Mismatch,
                MountIssuerContextDisposition::Mismatch,
            );
        };
        let Some(pending) = current.pending_mount.as_ref() else {
            return (
                CoreBackTargetMountContextDisposition::Mismatch,
                MountIssuerContextDisposition::Mismatch,
            );
        };
        let Some(host) = context.host.as_ref() else {
            return (
                CoreBackTargetMountContextDisposition::Mismatch,
                MountIssuerContextDisposition::Mismatch,
            );
        };
        let issuer_context = issuer.classify_context(
            host,
            current.identity.request,
            current.identity.transaction,
            &pending.parent,
            &pending.outlet,
        );
        let identity_is_current = current.stage
            == CoreBackTargetReceiverStageProjection::AwaitingMount
            && current.identity.router == context.router
            && current.identity.host == host.host()
            && current.identity.host_epoch == host.epoch();
        let Some(child_index) = current.identity.branch_index.checked_add(1) else {
            return (
                CoreBackTargetMountContextDisposition::Mismatch,
                issuer_context,
            );
        };
        let reservation_matches = if pending.from_preserved_parent {
            current
                .reservation
                .get(pending.child_index)
                .is_some_and(|reserved| reserved.frame_task == current.identity.frame_task)
        } else {
            current
                .reservation
                .get(current.identity.branch_index)
                .is_some_and(|reserved| {
                    reserved.frame_task == current.identity.frame_task
                        && reserved.frame == current.identity.frame
                })
        };
        let current_parent = RouteFrameVersion::new(
            current.identity.router,
            current.identity.host,
            current.identity.slot,
            current.identity.origin.clone(),
            current.identity.frame,
            current.identity.generation,
        );
        let entering_start = current
            .reconciliation
            .as_ref()
            .map_or(0, |projection| projection.preserved().len());
        let topology_is_current = pending.child_index == child_index
            && reservation_matches
            && current_parent == pending.parent
            && current.target_frames.len() == child_index
            && current.target_frames.last() == Some(&pending.parent)
            && entering_start <= current.target_frames.len()
            && current.applied_frames == current.target_frames[entering_start..];
        let Some(branch_routes) = current.branch_routes.as_ref() else {
            return (
                CoreBackTargetMountContextDisposition::Mismatch,
                issuer_context,
            );
        };
        let Some(current_route) = branch_routes.get(current.identity.branch_index) else {
            return (
                CoreBackTargetMountContextDisposition::Mismatch,
                issuer_context,
            );
        };
        let Some(child_route) = branch_routes.get(child_index) else {
            return (
                CoreBackTargetMountContextDisposition::Mismatch,
                issuer_context,
            );
        };
        let branch_is_current = current.identity.origin
            == FrameOrigin::MatchedRoute(current_route.route())
            && child_route.slot() == &pending.outlet
            && current.reservation.get(child_index).is_some();
        let current_target_is_exact = if pending.from_preserved_parent {
            current.reconciliation.as_ref().is_some_and(|projection| {
                projection.preserved().iter().any(|preserved| {
                    preserved.branch_index() == current.identity.branch_index
                        && preserved.version() == &pending.parent
                }) && projection.entering().first().is_some_and(|entering| {
                    entering.branch_index() == pending.child_index
                        && entering.slot() == ProjectedEnteringSlot::MountNew
                })
            })
        } else {
            match &current.current_target {
                FrameMountTarget::TopSlot {
                    host: target_host,
                    slot,
                } => {
                    current.identity.branch_index == 0
                        && target_host == host
                        && *slot == current.identity.slot
                        && context.top_slot == Some(*slot)
                }
                FrameMountTarget::ChildOutlet { parent, outlet }
                | FrameMountTarget::RegisteredChildSlot { parent, outlet } => {
                    current.identity.branch_index > 0
                        && current
                            .target_frames
                            .get(current.identity.branch_index - 1)
                            .is_some_and(|expected_parent| expected_parent == parent)
                        && current_route.slot() == outlet
                }
            }
        };
        let expected_request_is_exact = current.identity.request == request
            && pending.parent == *parent
            && pending.outlet == *outlet;
        let disposition = if identity_is_current
            && topology_is_current
            && branch_is_current
            && current_target_is_exact
            && expected_request_is_exact
        {
            CoreBackTargetMountContextDisposition::Matched
        } else {
            CoreBackTargetMountContextDisposition::Mismatch
        };
        (disposition, issuer_context)
    }

    fn back_target_execution_pairing_inspection(
        &self,
        scope: &BackResolutionScopeProjection,
        execution: &HistoryExecutionBindingProjection,
        physical: CoreBackTargetPhysicalReceiverRef<'_>,
    ) -> CoreBackTargetExecutionInspection {
        let (physical_projection, mount) = match physical {
            CoreBackTargetPhysicalReceiverRef::MountRequest(request) => {
                let (token, issuer, receiver) = self
                    .mount
                    .inspect_request_receiver(&request.token)
                    .into_parts();
                let context = self.back_target_mount_context_projection();
                let (engine_context, issuer_context) = Self::classify_back_target_mount_context(
                    &context,
                    &issuer,
                    request.request,
                    &request.parent,
                    &request.outlet,
                );
                (
                    CoreBackTargetReceiverActionProjection::MountRequest {
                        token,
                        request: request.request,
                        parent: request.parent.clone(),
                        outlet: request.outlet.clone(),
                    },
                    Some(CoreBackTargetMountObservedProjection {
                        issuer,
                        context,
                        receiver,
                        engine_context,
                        issuer_context,
                    }),
                )
            }
            CoreBackTargetPhysicalReceiverRef::FrameTask(_)
            | CoreBackTargetPhysicalReceiverRef::Commit(_) => {
                (Self::back_target_physical_projection(physical), None)
            }
        };
        let expected = CoreBackTargetExecutionExpectedProjection {
            scope: scope.clone(),
            execution: execution.clone(),
            physical: physical_projection,
        };
        let current = self.current.as_ref();
        let observed = CoreBackTargetExecutionObservedProjection {
            scope: self.back_target_engine_scope_projection(),
            history: self
                .current_history_capability
                .as_ref()
                .map(HistoryCommitCapability::execution_projection),
            current: current.map(|current| current.identity.clone()),
            action: Self::back_target_action_projection(self.action.as_ref()),
            stage: Self::back_target_stage_projection(current),
            mount,
        };
        let physical_is_exact = match (&expected.physical, physical) {
            (
                CoreBackTargetReceiverActionProjection::FrameTask(identity),
                CoreBackTargetPhysicalReceiverRef::FrameTask(_),
            ) => {
                observed.current.as_ref() == Some(identity)
                    && observed.stage == CoreBackTargetReceiverStageProjection::Applying
            }
            (
                CoreBackTargetReceiverActionProjection::MountRequest { .. },
                CoreBackTargetPhysicalReceiverRef::MountRequest(_),
            ) => observed.mount.as_ref().is_some_and(|mount| {
                mount.receiver == MountRequestReceiverDisposition::Matched
                    && mount.engine_context == CoreBackTargetMountContextDisposition::Matched
                    && mount.issuer_context == MountIssuerContextDisposition::Matched
            }),
            (
                CoreBackTargetReceiverActionProjection::Commit(identity),
                CoreBackTargetPhysicalReceiverRef::Commit(_),
            ) => {
                observed.current.as_ref() == Some(identity)
                    && observed.stage == CoreBackTargetReceiverStageProjection::ReadyToCommit
            }
            _ => false,
        };
        let exact = scope.matches_engine_scope(
            observed.scope.world,
            observed.scope.router,
            observed.scope.history,
        ) && scope.matches_execution(execution)
            && observed.history.as_ref() == Some(execution)
            && physical_is_exact
            && observed.action == CoreBackTargetReceiverActionProjection::Missing;
        CoreBackTargetExecutionInspection {
            expected,
            observed,
            exact,
        }
    }

    pub(crate) fn inspect_back_target_execution_pairing(
        &self,
        scope: &BackResolutionScopeProjection,
        execution: &HistoryExecutionBindingProjection,
        physical: CoreBackTargetPhysicalReceiverRef<'_>,
    ) -> Result<(), CorePreparationReceiverMismatchProjection> {
        self.back_target_execution_pairing_inspection(scope, execution, physical)
            .into_exact()
            .map(|_| ())
    }

    pub(crate) fn is_current_root_back_frame_task(&self, identity: &RootTaskIdentity) -> bool {
        self.inspect_view_report_identity(identity).is_ok()
            && self.current.as_ref().is_some_and(|current| {
                current.identity == *identity
                    && matches!(
                        current.attempt,
                        AttemptKind::Normal(NavigationCommandKind::Back)
                    )
            })
    }

    pub(crate) fn report_back_target_failed(
        &mut self,
        scope: BackResolutionScopeProjection,
        execution: &HistoryExecutionBindingProjection,
        task: &FrameTask,
        failure: NavigationFailure,
        mode: CoreBackTargetDriverFailureReportMode,
    ) -> Result<CoreBackTargetDriverFailureReport, CorePreparationReceiverMismatchProjection> {
        self.inspect_back_target_execution_pairing(
            &scope,
            execution,
            CoreBackTargetPhysicalReceiverRef::FrameTask(task),
        )?;
        let session = ViewReportSession {
            engine: self,
            identity: task.identity().clone(),
        };
        Ok(session.failed_back_target(scope, failure, mode))
    }

    pub(crate) fn inspect_back_target_driver_failure_fallback(
        &self,
        owner: &CoreBackTargetDriverFallback,
    ) -> Result<(), CorePreparationReceiverMismatchProjection> {
        let expected = CoreBackTargetDriverFallbackExpectedProjection {
            scope: owner.scope.clone(),
            action: CoreBackTargetReceiverActionProjection::FrameTask(owner.action.clone()),
            attempt: owner.expected_attempt.clone(),
        };
        let current = self.current.as_ref();
        let observed = CoreBackTargetDriverFallbackObservedProjection {
            scope: self.back_target_engine_scope_projection(),
            action: Self::back_target_action_projection(self.action.as_ref()),
            current: current.map(|current| current.identity.clone()),
            attempt: Self::back_target_attempt_projection(current),
            stage: Self::back_target_stage_projection(current),
        };
        let exact = owner.scope.matches_engine_scope(
            observed.scope.world,
            observed.scope.router,
            observed.scope.history,
        ) && observed.action == expected.action
            && observed.current.as_ref() == Some(&owner.action)
            && observed.attempt == expected.attempt
            && observed.stage == CoreBackTargetReceiverStageProjection::Applying
            && owner.action.router_identity() == self.router;
        if exact {
            Ok(())
        } else {
            Err(CorePreparationReceiverMismatchProjection::DriverFallback { expected, observed })
        }
    }

    pub(crate) fn inspect_back_target_driver_failure_cleanup(
        &self,
        owner: &CoreBackTargetDriverFailureCleanup,
    ) -> Result<(), CorePreparationReceiverMismatchProjection> {
        let expected = CoreBackTargetDriverFailureCleanupExpectedProjection {
            scope: owner.scope.clone(),
            request: Self::back_target_failure_cleanup_request_projection(&owner.request),
        };
        let current = self.current.as_ref();
        let observed = CoreBackTargetDriverFailureCleanupObservedProjection {
            scope: self.back_target_engine_scope_projection(),
            current: current.map(|current| current.identity.clone()),
            stage: Self::back_target_stage_projection(current),
        };
        let exact = owner.scope.matches_engine_scope(
            observed.scope.world,
            observed.scope.router,
            observed.scope.history,
        ) && observed.current.as_ref() == Some(&expected.request.identity)
            && observed.stage
                == CoreBackTargetReceiverStageProjection::FailureCleanupManifestPending(
                    expected.request.clone(),
                );
        if exact {
            Ok(())
        } else {
            Err(
                CorePreparationReceiverMismatchProjection::DriverFailureCleanup {
                    expected,
                    observed,
                },
            )
        }
    }

    pub(crate) fn inspect_back_target_driver_failure_terminal(
        &self,
        owner: &CoreBackTargetDriverTerminal,
    ) -> Result<(), CorePreparationReceiverMismatchProjection> {
        let expected = CoreBackTargetDriverTerminalExpectedProjection {
            scope: owner.scope.clone(),
            router: owner.request.router_identity(),
            request: owner.request,
            status: CoreBackTargetDriverTerminalExpectedStatus::Failed,
        };
        let request = match self.private_lookup(owner.request) {
            PrivateRequestLookup::Retained(record) => {
                CoreBackTargetDriverTerminalObservedRequestProjection::Retained {
                    request: record.id(),
                    status: record.status().clone(),
                }
            }
            PrivateRequestLookup::Evicted => {
                CoreBackTargetDriverTerminalObservedRequestProjection::Evicted {
                    request: owner.request,
                    evicted_through: self.evicted_through,
                    highest_issued: self.highest_issued,
                }
            }
            PrivateRequestLookup::Unknown => {
                CoreBackTargetDriverTerminalObservedRequestProjection::Unknown {
                    request: owner.request,
                    evicted_through: self.evicted_through,
                    highest_issued: self.highest_issued,
                }
            }
        };
        let observed = CoreBackTargetDriverTerminalObservedProjection {
            scope: self.back_target_engine_scope_projection(),
            request,
        };
        let exact = owner.scope.matches_engine_scope(
            observed.scope.world,
            observed.scope.router,
            observed.scope.history,
        ) && expected.router == self.router
            && matches!(
                (expected.status, &observed.request),
                (
                    CoreBackTargetDriverTerminalExpectedStatus::Failed,
                    CoreBackTargetDriverTerminalObservedRequestProjection::Retained {
                        request,
                        status: RequestStatus::Failed(_),
                    }
                ) if *request == expected.request
            );
        if exact {
            Ok(())
        } else {
            Err(CorePreparationReceiverMismatchProjection::DriverTerminal { expected, observed })
        }
    }

    pub(crate) fn report_back_target_applied(
        &mut self,
        scope: BackResolutionScopeProjection,
        execution: &HistoryExecutionBindingProjection,
        task: &FrameTask,
        outlets: Option<Vec<RouteSlotKey>>,
    ) -> Result<CoreBackTargetAppliedOutcome, CorePreparationReceiverMismatchProjection> {
        let (receiver_expected, receiver_observed) = self
            .back_target_execution_pairing_inspection(
                &scope,
                execution,
                CoreBackTargetPhysicalReceiverRef::FrameTask(task),
            )
            .into_exact()?;
        let report = match outlets {
            Some(outlets) => AppliedFrameReport {
                identity: task.identity().clone(),
                outlets,
                mode: AppliedReportMode::OutletAware,
            },
            None => AppliedFrameReport {
                identity: task.identity().clone(),
                outlets: Vec::new(),
                mode: AppliedReportMode::LegacyLeaf,
            },
        };
        let report_projection = match report.mode {
            AppliedReportMode::LegacyLeaf => CoreBackTargetAppliedReportProjection::LegacyLeaf,
            AppliedReportMode::OutletAware => {
                CoreBackTargetAppliedReportProjection::WithOutlets(report.outlets.clone())
            }
        };
        let transition = match self.inspect_applied_report(report) {
            AppliedReportInspection::Rejected {
                report: _,
                rejection: _,
                predicates,
            } => {
                return Err(CorePreparationReceiverMismatchProjection::Applied {
                    expected: CoreBackTargetAppliedExpectedProjection {
                        receiver: receiver_expected,
                        report: report_projection,
                    },
                    observed: CoreBackTargetAppliedObservedProjection {
                        receiver: receiver_observed,
                        next_outlet: predicates.next_outlet,
                        reconciliation: predicates.reconciliation,
                        active_baseline: predicates.active_baseline,
                    },
                });
            }
            AppliedReportInspection::ReconciliationDrift => {
                AppliedReportTransition::FailureCleanup(
                    self.begin_current_reconciliation_drift_failure(),
                )
            }
            AppliedReportInspection::Validated(plan) => self.apply_validated_applied_report(plan),
        };
        Ok(match transition {
            AppliedReportTransition::MountRequest(request) => {
                CoreBackTargetAppliedOutcome::MountRequest(request)
            }
            AppliedReportTransition::Commit(ticket) => CoreBackTargetAppliedOutcome::Commit(ticket),
            AppliedReportTransition::FailureCleanup(request) => {
                CoreBackTargetAppliedOutcome::FailureCleanup(CoreBackTargetDriverFailureCleanup {
                    scope,
                    request,
                })
            }
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
        if current.cleanup_roots != seal.logical_roots() {
            return Err(CommitError::InvalidManifest);
        }

        let current = current.clone_pre_retirement().ok_or(CommitError::Stale)?;
        let cleanup = CleanupBatchToken::sealed(
            current.identity.clone(),
            current.cleanup_batch,
            &current.cleanup_roots,
            seal.manifest_digest(),
            seal.obligation_count(),
        );
        let is_normal = matches!(&current.attempt, AttemptKind::Normal(_));
        let history_capability = if is_normal {
            match self.current_history_capability.take() {
                Some(capability) => Some(capability),
                None => return Err(CommitError::Stale),
            }
        } else {
            debug_assert!(self.current_history_capability.is_none());
            None
        };
        let publication =
            match self.precompute_publication(current, cleanup.clone(), history_capability) {
                Ok(publication) => publication,
                Err(error) => {
                    debug_assert!(error.evidence_is_consistent_for(ticket.identity.request));
                    self.pending_history_preparation_error = Some(error);
                    let failure = self.begin_failure_cleanup(
                        PendingFailureKind::AuthorityLost,
                        NavigationFailure::new(
                            NavigationFailureKind::AuthorityLost,
                            "canonical history source or execution binding drifted",
                        ),
                    );
                    self.action = Some(EngineAction::FailureCleanup(failure));
                    return Err(CommitError::Stale);
                }
            };
        let evidence = SealedCommitEvidence {
            identity: ticket.identity.clone(),
            cleanup_batch: cleanup.batch(),
            history_execution: publication
                .history
                .as_ref()
                .map(PreparedHistoryPublication::execution_projection),
            source_revision: publication
                .history
                .as_ref()
                .map(PreparedHistoryPublication::source_revision),
        };
        self.current
            .as_mut()
            .expect("current work was validated")
            .stage = WorkStage::CommitSealed {
            token: cleanup.clone(),
        };
        let source = self.observe_prepared_publication_source(publication.record_index);
        Ok(SealedCommit {
            identity: ticket.identity.clone(),
            cleanup,
            publication,
            evidence,
            source,
        })
    }

    pub(crate) fn inspect_commit_identity(&self, ticket: &CommitTicket) -> Result<(), CommitError> {
        self.current
            .as_ref()
            .filter(|current| {
                current.stage == WorkStage::ReadyToCommit && current.identity == ticket.identity
            })
            .map(|_| ())
            .ok_or(CommitError::Stale)
    }

    pub(crate) fn inspect_back_target_continuation(
        &self,
        scope: &BackResolutionScopeProjection,
        ticket: Option<&CommitTicket>,
    ) -> Result<(), CorePreparationReceiverMismatchProjection> {
        let current = self.current.as_ref();
        let observed = CoreBackTargetContinuationObservedProjection {
            scope: self.back_target_engine_scope_projection(),
            history: self
                .current_history_capability
                .as_ref()
                .map(HistoryCommitCapability::execution_projection),
            current: current.map(|current| current.identity.clone()),
            action: Self::back_target_action_projection(self.action.as_ref()),
            stage: Self::back_target_stage_projection(current),
        };
        let continuation_is_exact = scope.matches_engine_scope(
            observed.scope.world,
            observed.scope.router,
            observed.scope.history,
        ) && observed
            .history
            .as_ref()
            .is_some_and(|execution| scope.matches_execution(execution));
        let commit_is_exact = ticket.is_none_or(|ticket| {
            observed.current.as_ref() == Some(&ticket.identity)
                && observed.stage == CoreBackTargetReceiverStageProjection::ReadyToCommit
        });
        let action_is_exact = observed.action == CoreBackTargetReceiverActionProjection::Missing;
        if continuation_is_exact && commit_is_exact && action_is_exact {
            Ok(())
        } else {
            Err(CorePreparationReceiverMismatchProjection::Continuation {
                expected: CoreBackTargetContinuationExpectedProjection {
                    scope: scope.clone(),
                    commit: ticket.map(|ticket| ticket.identity.clone()),
                },
                observed,
            })
        }
    }

    pub(crate) fn consume_back_target_pending_rejected(
        &mut self,
        scope: BackResolutionScopeProjection,
        ticket: CommitTicket,
    ) -> Result<
        FailureCleanupRequest,
        (
            BackResolutionScopeProjection,
            CommitTicket,
            CorePreparationReceiverMismatchProjection,
        ),
    > {
        if let Err(mismatch) = self.inspect_back_target_continuation(&scope, Some(&ticket)) {
            return Err((scope, ticket, mismatch));
        }
        drop(scope);
        drop(ticket);
        Ok(self.begin_failure_cleanup_with_continuation(
            PendingFailureKind::Apply,
            NavigationFailure::new(
                NavigationFailureKind::Apply,
                "frame final preflight rejected",
            ),
            FailureCleanupContinuation::BeginFallback,
        ))
    }

    pub(crate) fn accepts_back_target_continuation(
        &self,
        scope: &BackResolutionScopeProjection,
    ) -> bool {
        let exact_scope = self.accepts_back_target_scope(scope);
        let exact_execution = self
            .current_history_capability
            .as_ref()
            .is_some_and(|capability| scope.matches_execution(&capability.execution_projection()));
        exact_scope && exact_execution
    }

    pub(crate) fn accepts_back_target_scope(&self, scope: &BackResolutionScopeProjection) -> bool {
        scope.matches_engine_scope(
            self.history_admission_world,
            self.router,
            self.history.incarnation(),
        )
    }

    pub(crate) fn accepts_back_target_execution(
        &self,
        scope: &BackResolutionScopeProjection,
        execution: &HistoryExecutionBindingProjection,
    ) -> bool {
        self.accepts_back_target_continuation(scope)
            && scope.matches_execution(execution)
            && self
                .current_history_capability
                .as_ref()
                .is_some_and(|capability| capability.execution_projection() == *execution)
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn testing_invalidate_commit_action(&mut self) {
        if let Some(current) = self.current.as_mut()
            && current.stage == WorkStage::ReadyToCommit
        {
            current.stage = WorkStage::Applying;
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn testing_drift_current_frame_task_identity(&mut self) -> bool {
        let Some(current) = self
            .current
            .as_mut()
            .filter(|current| current.stage == WorkStage::Applying)
        else {
            return false;
        };
        let Some(host_epoch) = current.identity.host_epoch.checked_add(1) else {
            return false;
        };
        current.identity.host_epoch = host_epoch;
        true
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn testing_remove_back_target_permanent_current_reconciliation(&mut self) -> bool {
        self.current
            .as_mut()
            .and_then(|current| current.reconciliation.take())
            .is_some()
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn testing_remove_back_target_permanent_active_branch(&mut self) -> bool {
        self.active_branch.take().is_some()
    }

    #[cfg(test)]
    pub(crate) fn testing_drift_current_fallback_failure(&mut self) -> bool {
        let Some(AttemptKind::Fallback { original_failure }) = self
            .current
            .as_mut()
            .filter(|current| current.stage == WorkStage::Applying)
            .map(|current| &mut current.attempt)
        else {
            return false;
        };
        *original_failure = NavigationFailure::new(
            NavigationFailureKind::AuthorityLost,
            "test-only fallback attempt fact drift",
        );
        true
    }

    #[cfg(test)]
    pub(crate) fn testing_drift_failure_cleanup_manifest_request(&mut self) -> bool {
        let Some(WorkStage::FailureCleanupManifestPending { expected }) =
            self.current.as_mut().map(|current| &mut current.stage)
        else {
            return false;
        };
        let Some(host_epoch) = expected.identity.host_epoch.checked_add(1) else {
            return false;
        };
        expected.identity.host_epoch = host_epoch;
        true
    }

    #[cfg(test)]
    pub(crate) fn testing_drift_retained_terminal_request_status(
        &mut self,
        request: NavigationRequestId,
    ) -> bool {
        let Some(record) = self
            .records
            .iter_mut()
            .find(|record| record.id() == request)
            .filter(|record| matches!(record.status(), RequestStatus::Failed(_)))
        else {
            return false;
        };
        *record = NavigationRequestRecord::unresolved_back(request);
        true
    }

    #[cfg(test)]
    pub(crate) fn testing_back_no_target_authority_is_accepted(&self) -> bool {
        matches!(
            self.pending_back_no_target_authority.as_ref(),
            Some(CorePendingBackNoTargetAuthority::Accepted(_))
        )
    }

    #[cfg(test)]
    pub(crate) fn testing_back_no_target_authority_is_cleanup(&self) -> bool {
        matches!(
            self.pending_back_no_target_authority.as_ref(),
            Some(CorePendingBackNoTargetAuthority::Cleanup(_))
        )
    }

    #[cfg(test)]
    pub(crate) fn testing_back_no_target_authority_is_outstanding(&self) -> bool {
        self.pending_back_no_target_authority.is_some()
    }

    pub(crate) fn prepare_publication(
        &mut self,
        sealed: SealedCommit,
    ) -> Result<PreparedPublication, CommitError> {
        assert!(
            self.prepared_publication_authority.is_none(),
            "a prepared publication authority must be settled before another is installed"
        );
        if let Err(error) = self.validate_prepared_publication_owner(&sealed) {
            return Err(self.retain_rejected_publication(sealed, error));
        }
        let evidence = sealed.evidence.clone();
        self.prepared_publication_authority = Some(PreparedPublicationAuthority::Ready(sealed));
        Ok(PreparedPublication { evidence })
    }

    fn validate_prepared_publication_owner(
        &self,
        sealed: &SealedCommit,
    ) -> Result<(), CommitError> {
        self.validate_sealed_commit_evidence(&sealed.evidence)?;
        let observed = self.observe_prepared_publication_source(sealed.publication.record_index);
        if !observed.same_source(&sealed.source) {
            return Err(CommitError::Stale);
        }
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
        let history_owner_matches = match (
            sealed.publication.history.as_ref(),
            sealed.evidence.history_execution.as_ref(),
            sealed.evidence.source_revision,
        ) {
            (Some(history), Some(execution), Some(source_revision)) => {
                let Some(current) = self.current.as_ref() else {
                    return Err(CommitError::Stale);
                };
                let Some(matched) = current.branch.as_ref() else {
                    return Err(CommitError::Stale);
                };
                history.matches_sealed_projection(
                    execution,
                    source_revision,
                    &current.location,
                    sealed.publication.state.active(),
                    matched.branch_fingerprint(),
                    &current.target_frames,
                    &sealed.cleanup,
                )
            }
            (None, None, None) => true,
            _ => false,
        };
        if !history_owner_matches {
            return Err(CommitError::Stale);
        }
        if sealed
            .publication
            .history
            .as_ref()
            .is_some_and(|publication| !publication.matches_canonical_source(&self.history))
        {
            return Err(CommitError::Stale);
        }
        if !self.current_reconciliation_matches_active() {
            return Err(CommitError::Stale);
        }
        Ok(())
    }

    fn observe_prepared_publication_source(
        &self,
        record_index: usize,
    ) -> PreparedPublicationObservation {
        let current = self.current.as_ref();
        PreparedPublicationObservation {
            identity: current.map(|current| current.identity.clone()),
            stage: current.map(|current| current.stage.clone()),
            record: self.records.get(record_index).cloned(),
            history_revision: self.history.revision(),
            history_execution: current.and_then(|current| match &current.attempt {
                AttemptKind::Normal(_) => Some(self.current_history_execution_projection(current)),
                AttemptKind::Fallback { .. } => None,
            }),
            current_location: current.map(|current| current.location.clone()),
            current_branch: current.and_then(|current| current.branch.clone()),
            current_target_frames: current
                .map(|current| current.target_frames.clone())
                .unwrap_or_default(),
            current_reconciliation: current.and_then(|current| current.reconciliation.clone()),
            state: self.state.clone(),
            host: self.state.host().cloned(),
            top_slot: self.state.top_slot(),
            active: self.state.active().clone(),
            active_branch: self.active_branch.clone(),
            active_child_slots: self.active_child_slots.clone(),
        }
    }

    fn retain_rejected_publication(
        &mut self,
        sealed: SealedCommit,
        error: CommitError,
    ) -> CommitError {
        let observed = self.observe_prepared_publication_source(sealed.publication.record_index);
        debug_assert!(
            !observed.same_source(&sealed.source) || !observed.accepts(&sealed),
            "rejected publication evidence must describe a real authority mismatch"
        );
        self.prepared_publication_authority = Some(PreparedPublicationAuthority::Rejected(
            PreparedPublicationRejection {
                owner: sealed,
                error,
                observed,
            },
        ));
        error
    }

    fn validate_sealed_commit_evidence(
        &self,
        evidence: &SealedCommitEvidence,
    ) -> Result<(), CommitError> {
        let valid = self.current.as_ref().is_some_and(|current| {
            let execution_matches = match (
                &evidence.history_execution,
                evidence.source_revision,
                &current.attempt,
            ) {
                (Some(execution), Some(_), AttemptKind::Normal(_)) => {
                    self.current_history_execution_projection(current) == *execution
                }
                (None, None, AttemptKind::Fallback { .. }) => true,
                _ => false,
            };
            current.identity == evidence.identity
                && matches!(
                    &current.stage,
                    WorkStage::CommitSealed { token }
                        if token.batch() == evidence.cleanup_batch
                )
                && evidence
                    .source_revision
                    .is_none_or(|revision| revision == self.history.revision())
                && execution_matches
        });
        valid.then_some(()).ok_or(CommitError::Stale)
    }

    #[cfg(test)]
    pub(crate) fn probe_sealed_commit_evidence(
        &self,
        evidence: &SealedCommitEvidence,
    ) -> Result<(), CommitError> {
        self.validate_sealed_commit_evidence(evidence)
    }

    #[cfg(test)]
    pub(crate) fn probe_consumed_publication_evidence(
        &self,
        consumed: &ConsumedPublicationEvidence,
    ) -> Result<(), CommitError> {
        self.validate_sealed_commit_evidence(&consumed.evidence)
    }

    #[cfg(test)]
    pub(crate) fn publish_prepared_with_evidence<F>(
        &mut self,
        prepared: PreparedPublication,
        mutation: F,
    ) -> Result<ConsumedPublicationEvidence, CommitError>
    where
        F: FnOnce(),
    {
        let evidence = prepared.evidence.clone();
        self.publish_prepared(prepared, mutation)?;
        Ok(ConsumedPublicationEvidence { evidence })
    }

    pub(crate) fn validate_publication(
        &mut self,
        prepared: PreparedPublication,
    ) -> Result<ValidatedPublication<'_>, CommitError> {
        let prepared = self.validate_publication_owner(prepared)?;
        Ok(ValidatedPublication {
            engine: self,
            prepared,
        })
    }

    fn validate_publication_owner(
        &mut self,
        prepared: PreparedPublication,
    ) -> Result<PreparedPublication, CommitError> {
        let validation = match self.prepared_publication_authority.as_ref() {
            Some(PreparedPublicationAuthority::Ready(owner))
                if owner.evidence == prepared.evidence =>
            {
                self.validate_prepared_publication_owner(owner)
            }
            Some(PreparedPublicationAuthority::Rejected(rejection))
                if rejection.owner.evidence == prepared.evidence =>
            {
                return Err(rejection.error);
            }
            Some(
                PreparedPublicationAuthority::Ready(_) | PreparedPublicationAuthority::Rejected(_),
            )
            | None => return Err(CommitError::Stale),
        };
        if let Err(error) = validation {
            let Some(PreparedPublicationAuthority::Ready(owner)) =
                self.prepared_publication_authority.take()
            else {
                unreachable!("publication validation only runs for a ready owner")
            };
            return Err(self.retain_rejected_publication(owner, error));
        }
        Ok(prepared)
    }

    pub(crate) fn prepare_back_target_commit(
        &mut self,
        scope: BackResolutionScopeProjection,
        ticket: &CommitTicket,
        seal: CleanupManifestSeal,
    ) -> BackTargetCommitOperationOutcome<'_> {
        let sealed = match self.seal_commit(ticket, seal) {
            Ok(sealed) => sealed,
            Err(error) => {
                return BackTargetCommitOperationOutcome::Failed(
                    self.take_back_target_commit_failure(scope, error),
                );
            }
        };
        let cleanup = sealed.cleanup().clone();
        let prepared = match self.prepare_publication(sealed) {
            Ok(prepared) => prepared,
            Err(error) => {
                return BackTargetCommitOperationOutcome::Failed(
                    self.take_back_target_commit_failure(scope, error),
                );
            }
        };
        match self.validate_publication_owner(prepared) {
            Ok(prepared) => BackTargetCommitOperationOutcome::Validated {
                publication: ValidatedPublication {
                    engine: self,
                    prepared,
                },
                cleanup,
                continuation: scope,
            },
            Err(error) => BackTargetCommitOperationOutcome::Failed(
                self.take_back_target_commit_failure(scope, error),
            ),
        }
    }

    fn take_back_target_commit_failure(
        &mut self,
        scope: BackResolutionScopeProjection,
        error: CommitError,
    ) -> BackTargetCommitOperationFailure {
        if matches!(
            self.action.as_ref(),
            Some(EngineAction::FailureCleanup(request))
                if request.request() == scope.request()
        ) {
            if let Some(EngineAction::FailureCleanup(request)) = self.action.take() {
                return BackTargetCommitOperationFailure::FailureCleanup(
                    CoreBackTargetCommitFailureCleanup {
                        continuation: scope,
                        request,
                    },
                );
            }
        }
        if matches!(
            self.prepared_publication_authority.as_ref(),
            Some(PreparedPublicationAuthority::Rejected(rejection))
                if rejection
                    .owner
                    .evidence
                    .history_execution
                    .as_ref()
                    .is_some_and(|execution| scope.matches_execution(execution))
        ) {
            if let Some(PreparedPublicationAuthority::Rejected(rejection)) =
                self.prepared_publication_authority.take()
            {
                return BackTargetCommitOperationFailure::RejectedPublication(
                    CoreBackTargetCommitRejectedPublication {
                        continuation: scope,
                        rejection,
                    },
                );
            }
        }
        debug_assert!(
            self.accepts_back_target_continuation(&scope),
            "retained-input failure must leave the exact history capability in the engine"
        );
        BackTargetCommitOperationFailure::RetainedInput {
            continuation: scope.clone(),
            evidence: CoreBackTargetCommitRetainedInputProjection { scope, error },
        }
    }

    #[cfg(test)]
    pub(crate) fn publish_prepared<F>(
        &mut self,
        prepared: PreparedPublication,
        mutation: F,
    ) -> Result<(), CommitError>
    where
        F: FnOnce(),
    {
        let validated = self.validate_publication(prepared)?;
        validated.publish(mutation);
        Ok(())
    }

    fn publish_validated<F>(&mut self, mutation: F)
    where
        F: FnOnce(),
    {
        let Some(PreparedPublicationAuthority::Ready(owner)) =
            self.prepared_publication_authority.take()
        else {
            unreachable!("validated publication retains its unique ready owner")
        };
        let SealedCommit {
            identity: _,
            cleanup: _,
            publication,
            evidence: _,
            source: _,
        } = owner;
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
        self.active_branch = active_branch;
        self.active_child_slots = active_child_slots;
        self.current = Some(current);
        if let Some(history) = history {
            self.history.publish(history);
        }
    }

    pub(crate) fn complete_cleanup(
        &mut self,
        completion: CleanupCompletion,
    ) -> Result<(), CleanupError> {
        if self.authority_settlement_cleanup_pending() {
            return Err(CleanupError::Invalid);
        }
        self.complete_cleanup_impl(completion, || {})
    }

    pub(crate) fn complete_cleanup_with_authority_mutation<F>(
        &mut self,
        completion: CleanupCompletion,
        authority_mutation: F,
    ) -> Result<(), CleanupError>
    where
        F: FnOnce(),
    {
        if !self.authority_settlement_cleanup_pending() {
            return Err(CleanupError::Invalid);
        }
        self.complete_cleanup_impl(completion, authority_mutation)
    }

    fn authority_settlement_cleanup_pending(&self) -> bool {
        self.current.as_ref().is_some_and(|current| {
            matches!(
                current.stage,
                WorkStage::FailureCleanupPrepared {
                    continuation: FailureCleanupContinuation::AuthoritySettlement,
                    ..
                }
            )
        })
    }

    fn complete_cleanup_impl<F>(
        &mut self,
        completion: CleanupCompletion,
        authority_mutation: F,
    ) -> Result<(), CleanupError>
    where
        F: FnOnce(),
    {
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
            let _ = self.begin_fallback(request, failure);
            return Ok(());
        }
        if let Some((failure, FailureCleanupContinuation::AuthoritySettlement)) = &failure {
            let retired = current.retired_host.as_ref().ok_or(CleanupError::Stale)?;
            let version = self.state.host().cloned().ok_or(CleanupError::Stale)?;
            let top_slot = self.state.top_slot().ok_or(CleanupError::Stale)?;
            if !retired.matches(self.router, &version, top_slot)
                || self.state.availability()
                    != &crate::presentation::RouterAvailability::RecoveringAuthority
            {
                return Err(CleanupError::Stale);
            }
            let mut terminal_record = self
                .request(request)
                .expect("authority settlement request remains retained")
                .clone();
            terminal_record
                .append_trace(NavigationTraceEvent::cleanup_settled(None))
                .expect("authority settlement trace remains open");
            terminal_record
                .transition(RequestStatus::Failed(failure.clone()))
                .expect("authority settlement request is running");
            terminal_record
                .append_trace(NavigationTraceEvent::request_terminal())
                .expect("authority settlement terminal event is last");
            if self.completed_cleanup.try_reserve(1).is_err() {
                return Err(CleanupError::Stale);
            }
            authority_mutation();
            *self
                .record_mut(request)
                .expect("authority settlement request remains retained") = terminal_record;
            self.completed_cleanup.push(CompletedCleanupTombstone {
                request,
                token: completed_token,
            });
            assert!(self.state.complete_authority_settlement(&version, top_slot));
            self.active_branch = None;
            self.active_child_slots.clear();
            self.retire_history_authority();
            self.current = None;
            self.state.set_request(None);
            self.retain_terminal_records();
            return Ok(());
        }
        let terminal_status = match failure {
            Some((failure, FailureCleanupContinuation::Terminal)) => RequestStatus::Failed(failure),
            Some((_, FailureCleanupContinuation::BeginFallback)) => unreachable!(),
            Some((_, FailureCleanupContinuation::AuthoritySettlement)) => unreachable!(),
            None => match &current.attempt {
                AttemptKind::Normal(_) => RequestStatus::Succeeded,
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
        self.retire_history_authority();
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
            .or_else(|| {
                self.pending_no_target_back
                    .as_ref()
                    .map(|pending| pending.request)
            })
    }

    pub(crate) fn requested(&self) -> Option<&RouteLocation> {
        self.running_request()
            .and_then(|request| self.request(request))
            .and_then(NavigationRequestRecord::requested)
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
        self.history.projection()
    }

    pub(crate) fn history_snapshot(&self) -> crate::history::HistorySnapshot {
        self.history.snapshot()
    }

    #[cfg(test)]
    pub(crate) fn testing_history_authority_snapshot(&self) -> HistoryAuthoritySnapshot {
        self.history.authority_snapshot()
    }

    #[cfg(test)]
    pub(crate) fn testing_history_capability_snapshot(&self) -> HistoryCapabilitySnapshot {
        let prepared = self
            .prepared_publication_authority
            .as_ref()
            .and_then(|authority| authority.owner().publication.history.as_ref());
        match (
            &self.current_history_capability,
            &self.pending_history_preparation_error,
            prepared,
        ) {
            (Some(capability), None, None) => capability.snapshot(),
            (None, Some(error), None) => error.capability_snapshot(),
            (None, None, Some(publication)) => publication.capability_snapshot(),
            (None, None, None) => HistoryCapabilitySnapshot::none(),
            _ => panic!("history authority has more than one live owner"),
        }
    }

    #[cfg(test)]
    pub(crate) fn testing_history_drift_evidence_snapshot(
        &self,
    ) -> Option<HistoryDriftEvidenceSnapshot> {
        self.pending_history_preparation_error
            .as_ref()
            .map(HistoryPreparationError::snapshot)
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
        let command = queued.command;
        let (location, branch, reservation, back_destination) = match &command {
            NavigationCommand::Back => {
                let QueuedReservation::Branch(reservation) = queued.reservation else {
                    unreachable!("accepted back retains one maximum-depth branch reservation")
                };
                let world = self
                    .history_admission_world
                    .expect("accepted back retains its issued world projection");
                let preparation_baseline = self.back_target_preparation_baseline();
                let handoff = BackResolutionOwnerHandoff::resolved(
                    world,
                    self.router,
                    self.history.incarnation(),
                    reservation,
                    preparation_baseline,
                    self.history.preview_back(),
                );
                let evidence = handoff.evidence();
                let record = self
                    .record_mut(request)
                    .expect("accepted Back request remains retained until resolution");
                if let Some(target) = handoff.target_location().cloned() {
                    record
                        .resolve_requested_once(target)
                        .expect("Back target resolution is single-assignment");
                }
                record
                    .install_back_resolution(evidence)
                    .expect("raw Back resolution evidence is installed exactly once");
                self.action = Some(EngineAction::BackResolution(handoff));
                return;
            }
            NavigationCommand::Push(location) | NavigationCommand::Replace(location) => {
                let outcome = queued
                    .outcome
                    .expect("push and replace retain an enqueue-time match outcome");
                let branch = match outcome {
                    QueuedMatchOutcome::Matched(branch) => branch,
                    QueuedMatchOutcome::MatchFailed(failure) => {
                        let QueuedReservation::RequestOnly(reserved_request) = queued.reservation
                        else {
                            unreachable!("match failure retains request-only identity")
                        };
                        debug_assert_eq!(reserved_request, request);
                        self.record_mut(request)
                            .expect("running record is retained")
                            .push_diagnostic(DiagnosticEvent::match_failed(failure.message()))
                            .expect("running diagnostics are open");
                        let _ = self.begin_fallback(request, failure);
                        return;
                    }
                };
                let QueuedReservation::Branch(reservation) = queued.reservation else {
                    unreachable!("a stored matched branch has one atomic reservation")
                };
                (location.clone(), branch, reservation, None)
            }
        };
        self.record_mut(request)
            .expect("running record is retained")
            .set_matched(branch.branch_fingerprint().clone())
            .expect("match is written once");
        let active_location = self.state.active().location();
        let active_frames = self.state.active().frames();
        let reconciliation_fragment = active_location.is_some_and(|active| {
            active.path() == location.path()
                && active.search() == location.search()
                && active.fragment() != location.fragment()
        }) && self
            .current_active_branch()
            .is_some_and(|active| active.branch_fingerprint() == branch.branch_fingerprint());

        let host = self
            .state
            .host()
            .expect("host was checked before push")
            .clone();
        let top_slot = self.state.top_slot().expect("confirmed host has top slot");
        if branch.routes().len() > reservation.frames().len() {
            self.fail_reserved_request(
                request,
                NavigationFailure::new(
                    NavigationFailureKind::AuthorityLost,
                    "resolved back target exceeded installed maximum branch depth",
                ),
            );
            self.start_next();
            return;
        }
        let reserved_frames = reservation.frames()[..branch.routes().len()].to_vec();
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

        debug_assert_eq!(
            reconciliation.target_fingerprint(),
            branch.branch_fingerprint()
        );
        let _ = self.start_reconciled_request(
            request,
            command,
            location,
            branch,
            reservation,
            reserved_frames,
            host,
            top_slot,
            reconciliation,
            back_destination,
        );
    }
    #[allow(clippy::too_many_arguments)]
    fn start_reconciled_request(
        &mut self,
        request: NavigationRequestId,
        command: NavigationCommand,
        location: RouteLocation,
        branch: MatchedBranch,
        reservation: ReservedBranchAttempt,
        reserved_frames: Vec<ReservedFrameAttempt>,
        host: HostVersion,
        top_slot: RouteSlotId,
        reconciliation: RouteReconciliationProjection,
        back_destination: Option<BackDestination>,
    ) -> StartReconciledRequestOutcome {
        let mut target_frames = reconciliation
            .preserved()
            .iter()
            .map(|frame| frame.version().clone())
            .collect::<Vec<_>>();
        let cleanup_roots = reconciliation
            .leaving()
            .iter()
            .map(|frame| frame.version().clone())
            .collect::<Vec<_>>();
        let command_kind = command.kind();
        let root_frame = reconciliation
            .preserved()
            .iter()
            .find(|frame| frame.branch_index() == 0)
            .map(|frame| frame.version().clone())
            .unwrap_or_else(|| {
                let entering = reconciliation
                    .entering()
                    .iter()
                    .find(|frame| frame.branch_index() == 0)
                    .expect("a reconciled branch has an exact root frame");
                RouteFrameVersion::new(
                    self.router,
                    host.host(),
                    top_slot,
                    FrameOrigin::MatchedRoute(entering.route()),
                    entering.frame(),
                    entering.generation(),
                )
            });
        let root_attempt = NormalAttemptIdentity::new(
            host.host(),
            host.epoch(),
            top_slot,
            reserved_frames[0].frame_task(),
            root_frame.frame(),
            root_frame.generation(),
            root_frame.origin().clone(),
            0,
        );
        let execution = HistoryExecutionBinding::new(
            self.router,
            request,
            root_attempt,
            reservation.transaction(),
            reservation.cleanup_batch(),
        );
        let execution_projection = execution.projection();
        let history_capability = match back_destination {
            Some(destination) => match self.history.freeze_back(execution, destination) {
                Ok(capability) => capability,
                Err(drift) => {
                    return StartReconciledRequestOutcome::BackSourceDrift { reservation, drift };
                }
            },
            None => self.history.freeze(
                execution,
                command
                    .history_effect()
                    .expect("push and replace freeze their enqueue-time effect"),
            ),
        };
        self.current_history_capability = Some(history_capability);
        let Some(entering) = reconciliation.entering().first().cloned() else {
            let leaf_index = target_frames
                .len()
                .checked_sub(1)
                .expect("a matched branch retains at least its root");
            let leaf = branch
                .routes()
                .get(leaf_index)
                .expect("retained branch and target match have equal depth");
            let frame = target_frames[leaf_index].clone();
            let identity = FrameTaskIdentity {
                router: self.router,
                host: host.host(),
                host_epoch: host.epoch(),
                slot: frame.slot(),
                request,
                transaction: reservation.transaction(),
                frame_task: reserved_frames[leaf_index].frame_task(),
                frame: frame.frame(),
                generation: frame.generation(),
                origin: FrameOrigin::MatchedRoute(leaf.route()),
                branch_index: leaf_index,
            };
            let target = if leaf_index == 0 {
                FrameMountTarget::TopSlot {
                    host: host.clone(),
                    slot: top_slot,
                }
            } else {
                FrameMountTarget::ChildOutlet {
                    parent: target_frames[leaf_index - 1].clone(),
                    outlet: leaf.slot().clone(),
                }
            };
            self.current = Some(CurrentWork {
                identity: identity.clone(),
                branch: Some(branch),
                location: location.clone(),
                reservation: reserved_frames,
                target_frames,
                applied_frames: Vec::new(),
                current_target: target,
                pending_mount: None,
                stage: WorkStage::ReadyToCommit,
                cleanup_batch: reservation.cleanup_batch(),
                cleanup_roots,
                attempt: AttemptKind::Normal(command_kind),
                reconciliation: Some(reconciliation),
                retired_host: None,
            });
            self.transition_with_trace(request, NavigationPhase::ReadyToSwitch);
            self.action = Some(EngineAction::Commit(CommitTicket { identity }));
            return StartReconciledRequestOutcome::Started(execution_projection);
        };

        let branch_index = entering.branch_index();
        let matched = branch.routes()[branch_index].clone();
        let origin = FrameOrigin::MatchedRoute(entering.route());
        if entering.slot() == ProjectedEnteringSlot::MountNew {
            let parent_index = branch_index
                .checked_sub(1)
                .expect("the top slot always exists before reconciliation");
            let parent = target_frames[parent_index].clone();
            let outlet = matched.slot().clone();
            let identity = FrameTaskIdentity {
                router: self.router,
                host: host.host(),
                host_epoch: host.epoch(),
                slot: parent.slot(),
                request,
                transaction: reservation.transaction(),
                frame_task: reserved_frames[branch_index].frame_task(),
                frame: parent.frame(),
                generation: parent.generation(),
                origin: parent.origin().clone(),
                branch_index: parent_index,
            };
            let target = if parent_index == 0 {
                FrameMountTarget::TopSlot {
                    host: host.clone(),
                    slot: top_slot,
                }
            } else {
                FrameMountTarget::ChildOutlet {
                    parent: target_frames[parent_index - 1].clone(),
                    outlet: branch.routes()[parent_index].slot().clone(),
                }
            };
            self.current = Some(CurrentWork {
                identity,
                branch: Some(branch),
                location: location.clone(),
                reservation: reserved_frames,
                target_frames,
                applied_frames: Vec::new(),
                current_target: target,
                pending_mount: Some(PendingMount {
                    child_index: branch_index,
                    parent: parent.clone(),
                    outlet: outlet.clone(),
                    from_preserved_parent: true,
                }),
                stage: WorkStage::AwaitingMount,
                cleanup_batch: reservation.cleanup_batch(),
                cleanup_roots,
                attempt: AttemptKind::Normal(command_kind),
                reconciliation: Some(reconciliation),
                retired_host: None,
            });
            self.transition_with_trace(request, NavigationPhase::ResolvingMount);
            let token = match self.mount.issue(
                &mut self.ids,
                host,
                request,
                reservation.transaction(),
                parent.clone(),
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
                    return StartReconciledRequestOutcome::Started(execution_projection);
                }
            };
            self.record_mut(request)
                .expect("running record is retained")
                .append_trace(NavigationTraceEvent::mount_requested(
                    parent.clone(),
                    outlet.clone(),
                ))
                .expect("running trace is open");
            self.action = Some(EngineAction::MountRequest(FrameMountRequest {
                token,
                request,
                parent,
                outlet,
            }));
            return StartReconciledRequestOutcome::Started(execution_projection);
        }

        let ProjectedEnteringSlot::Existing(slot) = entering.slot() else {
            unreachable!("mount-new entering was handled above")
        };
        let target_frame = RouteFrameVersion::new(
            self.router,
            host.host(),
            slot,
            origin.clone(),
            entering.frame(),
            entering.generation(),
        );
        let target = if branch_index == 0 {
            FrameMountTarget::TopSlot {
                host: host.clone(),
                slot: top_slot,
            }
        } else {
            FrameMountTarget::RegisteredChildSlot {
                parent: target_frames[branch_index - 1].clone(),
                outlet: matched.slot().clone(),
            }
        };
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
            slot,
            request,
            transaction: reservation.transaction(),
            frame_task: reserved_frames[branch_index].frame_task(),
            frame: entering.frame(),
            generation: entering.generation(),
            origin,
            branch_index,
        };
        target_frames.push(target_frame.clone());
        self.current = Some(CurrentWork {
            identity: identity.clone(),
            branch: Some(branch),
            location: location.clone(),
            reservation: reserved_frames,
            target_frames,
            applied_frames: Vec::new(),
            current_target: target.clone(),
            pending_mount: None,
            stage: WorkStage::Applying,
            cleanup_batch: reservation.cleanup_batch(),
            cleanup_roots,
            attempt: AttemptKind::Normal(command_kind),
            reconciliation: Some(reconciliation),
            retired_host: None,
        });
        let record = self
            .record_mut(request)
            .expect("running record is retained");
        record
            .append_trace(NavigationTraceEvent::frame_task_issued(
                target_frame.clone(),
            ))
            .expect("running trace is open");
        let phase = NavigationPhase::ApplyingFrame(branch_index);
        record
            .transition(RequestStatus::Running(phase.clone()))
            .expect("request is running");
        record
            .append_trace(NavigationTraceEvent::request_phase_changed(phase.clone()))
            .expect("running trace is open");
        self.state.set_request(Some(RequestStatus::Running(phase)));
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
        StartReconciledRequestOutcome::Started(execution_projection)
    }

    fn begin_failure_cleanup(
        &mut self,
        expected: PendingFailureKind,
        failure: NavigationFailure,
    ) -> FailureCleanupRequest {
        let continuation = if expected != PendingFailureKind::AuthorityLost
            && self.current.as_ref().is_some_and(|current| {
                matches!(
                    current.attempt,
                    AttemptKind::Normal(NavigationCommandKind::Replace)
                )
            }) {
            FailureCleanupContinuation::BeginFallback
        } else {
            FailureCleanupContinuation::Terminal
        };
        self.begin_failure_cleanup_with_continuation(expected, failure, continuation)
    }

    fn begin_failure_cleanup_with_continuation(
        &mut self,
        expected: PendingFailureKind,
        failure: NavigationFailure,
        continuation: FailureCleanupContinuation,
    ) -> FailureCleanupRequest {
        let logical_roots = self
            .current
            .as_ref()
            .expect("a nested failure belongs to current work")
            .applied_frames
            .iter()
            .rev()
            .cloned()
            .collect();
        self.begin_failure_cleanup_with_roots(expected, failure, continuation, logical_roots)
    }

    fn begin_failure_cleanup_with_roots(
        &mut self,
        expected: PendingFailureKind,
        failure: NavigationFailure,
        continuation: FailureCleanupContinuation,
        logical_roots: Vec<RouteFrameVersion>,
    ) -> FailureCleanupRequest {
        self.current_history_capability = None;
        let current = self
            .current
            .as_ref()
            .expect("a nested failure belongs to current work");
        let request = FailureCleanupRequest {
            identity: current.identity.clone(),
            expected,
            logical_roots,
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

    fn retire_history_authority(&mut self) {
        let prepared_history_owner = self
            .prepared_publication_authority
            .as_ref()
            .is_some_and(|authority| authority.owner().publication.history.is_some());
        debug_assert!(
            usize::from(self.current_history_capability.is_some())
                + usize::from(self.pending_history_preparation_error.is_some())
                + usize::from(prepared_history_owner)
                <= 1,
            "history authority must retain a single linear owner"
        );
        if let Some(PreparedPublicationAuthority::Rejected(rejection)) =
            self.prepared_publication_authority.as_ref()
        {
            debug_assert!(
                !rejection.observed.same_source(&rejection.owner.source)
                    || !rejection.observed.accepts(&rejection.owner),
                "rejected publication evidence must retain the authority mismatch"
            );
            debug_assert!(matches!(
                rejection.error,
                CommitError::Stale | CommitError::InvalidManifest
            ));
        }
        self.current_history_capability = None;
        self.pending_history_preparation_error = None;
        self.prepared_publication_authority = None;
    }

    fn authority_settlement_logical_roots(&self) -> Vec<RouteFrameVersion> {
        let current = self
            .current
            .as_ref()
            .expect("authority settlement phase was preflighted");
        let projection = current
            .reconciliation
            .as_ref()
            .expect("authority settlement requires reconciliation");
        let mut roots = projection
            .entering()
            .iter()
            .rev()
            .filter_map(|entering| current.target_frames.get(entering.branch_index()))
            .cloned()
            .collect::<Vec<_>>();
        for active in self.state.active().frames().iter().rev() {
            if !roots.contains(active) {
                roots.push(active.clone());
            }
        }
        roots
    }

    fn begin_fallback(
        &mut self,
        request: NavigationRequestId,
        original_failure: NavigationFailure,
    ) -> Option<FrameTaskIdentity> {
        self.current_history_capability = None;
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
                self.retire_history_authority();
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
                return None;
            }
        };
        let location = self
            .request(request)
            .expect("recovering request is retained")
            .requested()
            .cloned()
            .expect("a recovering push or replace retains its requested target");
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
            stage: WorkStage::Applying,
            cleanup_batch: reserved.cleanup_batch(),
            cleanup_roots,
            attempt: AttemptKind::Fallback { original_failure },
            reconciliation: None,
            retired_host: None,
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
        let fallback_identity = identity.clone();
        self.action = Some(EngineAction::FrameTask(FrameTask {
            identity,
            frame: target_frame,
            target,
            location,
            matched,
            view: self.fallback.clone(),
            input: Some(input),
        }));
        Some(fallback_identity)
    }

    fn prepare_confirmed_child(
        &self,
        slot: RouteSlotId,
    ) -> Result<ConfirmedChildPublication, MountError> {
        if !self.has_live_mount_context(&WorkStage::ConfirmingMount, None) {
            return Err(MountError::WrongPhase);
        }
        let mut current = self
            .current
            .as_ref()
            .and_then(CurrentWork::clone_pre_retirement)
            .ok_or(MountError::WrongPhase)?;
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
            AttemptKind::Normal(_) => (
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

    fn back_target_preparation_baseline(&self) -> BackTargetPreparationBaseline {
        BackTargetPreparationBaseline::new(
            self.state.availability() == &crate::presentation::RouterAvailability::Ready,
            self.state.host().cloned(),
            self.state.top_slot(),
            self.state.active().clone(),
            self.current_active_branch()
                .map(|branch| branch.branch_fingerprint().clone()),
            self.active_child_slots.clone(),
        )
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
        self.install_current_reconciliation_drift_failure();
        true
    }

    fn install_current_reconciliation_drift_failure(&mut self) {
        let failure = self.begin_current_reconciliation_drift_failure();
        self.action = Some(EngineAction::FailureCleanup(failure));
    }

    fn begin_current_reconciliation_drift_failure(&mut self) -> FailureCleanupRequest {
        self.action = None;
        self.begin_failure_cleanup(
            PendingFailureKind::AuthorityLost,
            NavigationFailure::new(
                NavigationFailureKind::AuthorityLost,
                "active reconciliation baseline drifted",
            ),
        )
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

    fn precompute_publication(
        &self,
        mut current: CurrentWork,
        cleanup: CleanupBatchToken,
        history_capability: Option<HistoryCommitCapability>,
    ) -> Result<PublicationPlan, HistoryPreparationError> {
        let request = current.identity.request;
        let location = self
            .request(request)
            .expect("running record is retained")
            .requested()
            .cloned()
            .expect("a publishable request has resolved its requested target");

        let mut state = self.state.clone();
        let (active, active_branch, active_child_slots) = match &current.attempt {
            AttemptKind::Normal(_) => {
                let active_child_slots = reconciled_child_slots(&current);
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
        state.set_active(active.clone());
        state.set_request(Some(RequestStatus::Running(
            NavigationPhase::PostCommitCleanup,
        )));

        let record_index = self
            .records
            .iter()
            .position(|record| record.id() == request)
            .expect("running record is retained");
        let mut published_record = self.records[record_index].clone();
        for frame in &current.applied_frames {
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

        let history = match history_capability {
            Some(capability) => {
                let execution = self.current_history_execution_projection(&current);
                let matched = current
                    .branch
                    .as_ref()
                    .expect("normal publication retains its matched branch")
                    .branch_fingerprint()
                    .clone();
                let presentation = PreparedPresentationBinding::new(
                    execution,
                    active,
                    matched,
                    current.target_frames.clone(),
                    cleanup.clone(),
                );
                Some(self.history.prepare(capability, presentation)?)
            }
            None => None,
        };

        current.stage = WorkStage::PostCommitCleanup {
            outstanding: cleanup,
        };

        Ok(PublicationPlan {
            state,
            history,
            active_branch,
            active_child_slots,
            record_index,
            published_record,
            current,
        })
    }

    fn current_history_execution_projection(
        &self,
        current: &CurrentWork,
    ) -> HistoryExecutionBindingProjection {
        let root_reservation = current
            .reservation
            .first()
            .expect("normal work retains its root reservation");
        let root_frame = current
            .target_frames
            .first()
            .expect("normal work retains its root target frame");
        HistoryExecutionBindingProjection::observed(
            self.router,
            current.identity.request,
            NormalAttemptIdentity::new(
                root_frame.host(),
                current.identity.host_epoch,
                root_frame.slot(),
                root_reservation.frame_task(),
                root_frame.frame(),
                root_frame.generation(),
                root_frame.origin().clone(),
                0,
            ),
            current.identity.transaction,
            current.cleanup_batch,
        )
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
            history: self.history.authority_snapshot(),
            history_admission_world: self.history_admission_world,
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
            pending_no_target_back: self
                .pending_no_target_back
                .as_ref()
                .map(|pending| format!("{pending:?}")),
            current_history_capability: self
                .current_history_capability
                .as_ref()
                .map(HistoryCommitCapability::snapshot),
            pending_history_preparation_error: self
                .pending_history_preparation_error
                .as_ref()
                .map(HistoryPreparationError::snapshot),
            prepared_publication_authority: self
                .prepared_publication_authority
                .as_ref()
                .map(PreparedPublicationAuthoritySnapshot::from),
            pending_back_no_target_authority: self.pending_back_no_target_authority.as_ref().map(
                |pending| match pending {
                    CorePendingBackNoTargetAuthority::Accepted(_) => "accepted",
                    CorePendingBackNoTargetAuthority::Cleanup(_) => "cleanup",
                },
            ),
            pending_back_target_permanent_cleanup: self
                .pending_back_target_permanent_cleanup
                .as_ref()
                .map(|_| "cleanup"),
            action: self.action.as_ref().map(ActionSnapshot::from),
            issued_view_tasks: self.issued_view_tasks.clone(),
            mount: self.mount.snapshot(),
            report_dispositions: self.report_dispositions.clone(),
            router_diagnostics: self.router_diagnostics.clone(),
            completed_cleanup: self.completed_cleanup.clone(),
            next_failed_authority_trigger_revision: self.next_failed_authority_trigger_revision,
            live_failed_authority_trigger_revision: self.live_failed_authority_trigger_revision,
        }
    }

    #[cfg(test)]
    pub(crate) fn testing_snapshot(&self) -> EngineSnapshot {
        self.snapshot()
    }

    fn back_target_permanent_current_state_projection(
        &self,
    ) -> CoreBackTargetPermanentCurrentStateProjection {
        let current = self.current.as_ref();
        let pending_no_target_authority =
            self.pending_back_no_target_authority
                .as_ref()
                .map(|pending| match pending {
                    CorePendingBackNoTargetAuthority::Accepted(accepted) => {
                        CoreBackTargetPermanentNoTargetAuthorityProjection::Accepted {
                            receiver: accepted.receiver.clone(),
                            source: accepted.source.clone(),
                            host: accepted.host.clone(),
                            top_slot: accepted.top_slot,
                            logical_roots: accepted.logical_roots.clone(),
                        }
                    }
                    CorePendingBackNoTargetAuthority::Cleanup(cleanup) => {
                        CoreBackTargetPermanentNoTargetAuthorityProjection::Cleanup(
                            cleanup.receiver.clone(),
                        )
                    }
                });
        let history_preparation_error = match self.pending_history_preparation_error.as_ref() {
            None => CoreBackTargetPermanentHistoryPreparationErrorProjection::Vacant,
            Some(HistoryPreparationError::Source(_)) => {
                CoreBackTargetPermanentHistoryPreparationErrorProjection::SourceDrift
            }
            Some(HistoryPreparationError::Binding(_)) => {
                CoreBackTargetPermanentHistoryPreparationErrorProjection::BindingDrift
            }
        };
        CoreBackTargetPermanentCurrentStateProjection {
            host: self.state.host().cloned(),
            top_slot: self.state.top_slot(),
            current_identity: current.map(|current| current.identity.clone()),
            current_location: current.map(|current| current.location.clone()),
            current_stage: Self::back_target_stage_projection(current),
            current_logical_roots: current.map(|current| current.cleanup_roots.clone()),
            current_reconciliation: current
                .and_then(|current| current.reconciliation.as_ref().cloned()),
            current_retired_host: current
                .and_then(|current| {
                    current
                        .retired_host
                        .as_ref()
                        .map(RetiredHostAuthority::projection)
                })
                .or_else(|| {
                    self.pending_back_target_permanent_cleanup
                        .as_ref()
                        .map(|cleanup| cleanup.retired_host.projection())
                }),
            active_baseline: self.back_target_preparation_baseline(),
            availability: self.state.availability().clone(),
            request_status: self.state.current_request().cloned(),
            history_binding: self
                .current_history_capability
                .as_ref()
                .map(HistoryCommitCapability::execution_projection),
            pending_no_target: self
                .pending_no_target_back
                .as_ref()
                .map(|pending| pending.request),
            pending_no_target_authority,
            pending_target_cleanup: self
                .pending_back_target_permanent_cleanup
                .as_ref()
                .map(|cleanup| cleanup.receiver.clone()),
            history_preparation_error,
            prepared_publication: self
                .prepared_publication_authority
                .as_ref()
                .map(|authority| authority.owner().evidence.clone()),
            action: Self::back_target_action_projection(self.action.as_ref()),
        }
    }

    fn back_target_permanent_canonical_relation_projections(
        &self,
        settlement: &CoreBackTargetPermanentSettlementOwner,
    ) -> (
        CoreBackTargetPermanentCanonicalRelationProjection,
        CoreBackTargetPermanentCanonicalRelationProjection,
    ) {
        let current = self.current.as_ref();
        let reconciliation = current.and_then(|current| current.reconciliation.clone());
        let expected_active = reconciliation.clone().map_or(
            CoreBackTargetPermanentActiveRelationProjection::ReconciliationRequired,
            CoreBackTargetPermanentActiveRelationProjection::Reconciliation,
        );
        let active_baseline = self.back_target_preparation_baseline();
        let observed_active = if self.current_reconciliation_matches_active() {
            reconciliation.clone().map_or(
                CoreBackTargetPermanentActiveRelationProjection::Active(active_baseline.clone()),
                CoreBackTargetPermanentActiveRelationProjection::Reconciliation,
            )
        } else {
            CoreBackTargetPermanentActiveRelationProjection::Active(active_baseline.clone())
        };
        let expected_request = Some(CoreBackTargetPermanentRequestProjection {
            request: settlement.scope.request(),
            status: RequestStatus::Running(NavigationPhase::ReadyToSwitch),
            requested: settlement
                .expected_request
                .as_ref()
                .and_then(|request| request.requested.clone()),
            trace_closed: settlement
                .expected_request
                .as_ref()
                .is_some_and(|request| request.trace_closed),
        });
        let expected_location = expected_request
            .as_ref()
            .and_then(|request| request.requested.clone());
        let observed_pending_mount = current.and_then(|current| {
            current
                .pending_mount
                .as_ref()
                .map(|pending| CoreBackTargetPendingMountProjection {
                    child_index: pending.child_index,
                    parent: pending.parent.clone(),
                    outlet: pending.outlet.clone(),
                    from_preserved_parent: pending.from_preserved_parent,
                })
        });
        let pending_no_target_authority =
            self.pending_back_no_target_authority
                .as_ref()
                .map(|pending| match pending {
                    CorePendingBackNoTargetAuthority::Accepted(accepted) => {
                        CoreBackTargetPermanentNoTargetAuthorityProjection::Accepted {
                            receiver: accepted.receiver.clone(),
                            source: accepted.source.clone(),
                            host: accepted.host.clone(),
                            top_slot: accepted.top_slot,
                            logical_roots: accepted.logical_roots.clone(),
                        }
                    }
                    CorePendingBackNoTargetAuthority::Cleanup(cleanup) => {
                        CoreBackTargetPermanentNoTargetAuthorityProjection::Cleanup(
                            cleanup.receiver.clone(),
                        )
                    }
                });
        let history_preparation_error = match self.pending_history_preparation_error.as_ref() {
            None => CoreBackTargetPermanentHistoryPreparationErrorProjection::Vacant,
            Some(HistoryPreparationError::Source(_)) => {
                CoreBackTargetPermanentHistoryPreparationErrorProjection::SourceDrift
            }
            Some(HistoryPreparationError::Binding(_)) => {
                CoreBackTargetPermanentHistoryPreparationErrorProjection::BindingDrift
            }
        };
        let vacant_pending = CoreBackTargetPermanentForbiddenPendingProjection {
            pending_no_target: None,
            pending_no_target_authority: None,
            pending_target_cleanup: None,
            history_preparation_error:
                CoreBackTargetPermanentHistoryPreparationErrorProjection::Vacant,
            prepared_publication: None,
            action: CoreBackTargetReceiverActionProjection::Missing,
        };
        let observed_pending = CoreBackTargetPermanentForbiddenPendingProjection {
            pending_no_target: self
                .pending_no_target_back
                .as_ref()
                .map(|pending| pending.request),
            pending_no_target_authority,
            pending_target_cleanup: self
                .pending_back_target_permanent_cleanup
                .as_ref()
                .map(|cleanup| cleanup.receiver.clone()),
            history_preparation_error,
            prepared_publication: self
                .prepared_publication_authority
                .as_ref()
                .map(|authority| authority.owner().evidence.clone()),
            action: Self::back_target_action_projection(self.action.as_ref()),
        };
        let expected = CoreBackTargetPermanentCanonicalRelationProjection {
            request: expected_request,
            request_status: Some(RequestStatus::Running(NavigationPhase::ReadyToSwitch)),
            location: expected_location,
            logical_roots: settlement.logical_roots.clone(),
            cleanup_roots: Some(settlement.logical_roots.clone()),
            reconciliation: reconciliation.clone(),
            active: expected_active,
            stage: CoreBackTargetReceiverStageProjection::ReadyToCommit,
            availability: crate::presentation::RouterAvailability::Ready,
            retired_host: None,
            pending_mount: None,
            forbidden_pending: vacant_pending,
        };
        let observed = CoreBackTargetPermanentCanonicalRelationProjection {
            request: settlement.expected_request.clone(),
            request_status: self.state.current_request().cloned(),
            location: current.map(|current| current.location.clone()),
            logical_roots: settlement.logical_roots.clone(),
            cleanup_roots: current.map(|current| current.cleanup_roots.clone()),
            reconciliation,
            active: observed_active,
            stage: Self::back_target_stage_projection(current),
            availability: self.state.availability().clone(),
            retired_host: current.and_then(|current| {
                current
                    .retired_host
                    .as_ref()
                    .map(RetiredHostAuthority::projection)
            }),
            pending_mount: observed_pending_mount,
            forbidden_pending: observed_pending,
        };
        (expected, observed)
    }

    fn back_target_permanent_request_projection(
        record: &NavigationRequestRecord,
    ) -> CoreBackTargetPermanentRequestProjection {
        CoreBackTargetPermanentRequestProjection {
            request: record.id(),
            status: record.status().clone(),
            requested: record.requested().cloned(),
            trace_closed: record.trace_closed(),
        }
    }

    fn back_target_permanent_authority_projection(
        state: &RouterNavigationState,
    ) -> CoreBackTargetPermanentAuthorityProjection {
        CoreBackTargetPermanentAuthorityProjection {
            host: state.host().cloned(),
            top_slot: state.top_slot(),
            availability: state.availability().clone(),
        }
    }

    fn back_target_permanent_retention_projection(
        records: &VecDeque<NavigationRequestRecord>,
    ) -> CoreBackTargetPermanentRetentionProjection {
        CoreBackTargetPermanentRetentionProjection {
            limit: TERMINAL_RETENTION,
            terminal_count: records
                .iter()
                .filter(|record| is_terminal(record.status()))
                .count(),
            records: records
                .iter()
                .map(Self::back_target_permanent_request_projection)
                .collect(),
        }
    }

    pub(crate) fn prepare_back_target_permanent_settlement_owner(
        &self,
        scope: BackResolutionScopeProjection,
        logical_roots: Vec<RouteFrameVersion>,
    ) -> CoreBackTargetPermanentSettlementOwner {
        CoreBackTargetPermanentSettlementOwner {
            expected_current: self.back_target_permanent_current_state_projection(),
            expected_request: self
                .records
                .iter()
                .find(|record| record.id() == scope.request())
                .map(Self::back_target_permanent_request_projection),
            scope,
            logical_roots,
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn testing_back_target_permanent_current_state_projection(
        &self,
    ) -> CoreBackTargetPermanentCurrentStateProjection {
        self.back_target_permanent_current_state_projection()
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn testing_back_target_permanent_retired_host_matches(
        &self,
        version: &crate::admission::HostVersion,
        top_slot: crate::ids::RouteSlotId,
    ) -> bool {
        self.pending_back_target_permanent_cleanup
            .as_ref()
            .is_some_and(|cleanup| {
                cleanup
                    .retired_host
                    .projection()
                    .matches(self.router, version, top_slot)
            })
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn testing_back_target_permanent_begin_projection(
        &self,
    ) -> BackTargetPermanentBeginTestingProjection {
        BackTargetPermanentBeginTestingProjection {
            cleanup_owner_count: usize::from(self.pending_back_target_permanent_cleanup.is_some()),
            current_owner_count: usize::from(self.current.is_some()),
            action_owner_count: usize::from(self.action.is_some()),
            current_history_slot_count: usize::from(self.current_history_capability.is_some()),
            cleanup_history_owner_count: usize::from(
                self.pending_back_target_permanent_cleanup
                    .as_ref()
                    .is_some_and(|cleanup| cleanup.history.is_some()),
            ),
            cleanup_settlement_owner_count: usize::from(
                self.pending_back_target_permanent_cleanup.is_some(),
            ),
            queue_len: self.queue.len(),
            conflicting_owner_count: usize::from(self.pending_no_target_back.is_some())
                + usize::from(self.pending_back_no_target_authority.is_some())
                + usize::from(self.pending_history_preparation_error.is_some())
                + usize::from(self.prepared_publication_authority.is_some()),
        }
    }

    #[cfg(test)]
    pub(crate) fn testing_request_back_target_source_drift_before_freeze_once(&mut self) {
        self.testing_back_target_source_drift_before_freeze_once = true;
    }

    #[cfg(test)]
    pub(crate) fn testing_move_back_target_current_source_cursor_back(&mut self) {
        self.history.testing_move_cursor_back();
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn testing_transition_back_target_record_to_matching(
        &mut self,
        request: NavigationRequestId,
    ) -> bool {
        self.records
            .iter_mut()
            .find(|record| record.id() == request)
            .is_some_and(|record| {
                record
                    .transition(RequestStatus::Running(NavigationPhase::Matching))
                    .is_ok()
            })
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn testing_close_back_target_request_trace(
        &mut self,
        request: NavigationRequestId,
    ) -> bool {
        self.records
            .iter_mut()
            .find(|record| record.id() == request)
            .is_some_and(|record| {
                record.testing_close_trace();
                true
            })
    }

    #[cfg(test)]
    pub(crate) fn testing_move_back_target_busy_receiver_slot_from(
        &mut self,
        donor: &mut Self,
        slot: BackTargetBusyReceiverSlot,
    ) {
        match slot {
            BackTargetBusyReceiverSlot::Action => {
                debug_assert!(self.action.is_none());
                self.action = Some(donor.action.take().expect("the donor has a real action"));
            }
            BackTargetBusyReceiverSlot::Current => {
                debug_assert!(self.current.is_none());
                self.current = Some(
                    donor
                        .current
                        .take()
                        .expect("the donor has real current work"),
                );
            }
            BackTargetBusyReceiverSlot::PendingNoTargetBack => {
                debug_assert!(self.pending_no_target_back.is_none());
                let source = match donor.history.preview_back() {
                    BackPreview::NoTarget(source) => source,
                    BackPreview::Target(_) | BackPreview::Contradiction(_) => {
                        panic!("the donor must produce a real no-target source")
                    }
                };
                let request = donor
                    .records
                    .back()
                    .expect("the no-target donor retains a real request")
                    .id();
                self.pending_no_target_back = Some(PendingNoTargetBack { request, source });
            }
            BackTargetBusyReceiverSlot::PendingBackNoTargetAuthority => {
                debug_assert!(self.pending_back_no_target_authority.is_none());
                self.pending_back_no_target_authority = Some(
                    donor
                        .pending_back_no_target_authority
                        .take()
                        .expect("the donor has real pending no-target authority"),
                );
            }
            BackTargetBusyReceiverSlot::CurrentHistoryCapability => {
                debug_assert!(self.current_history_capability.is_none());
                self.current_history_capability = Some(
                    donor
                        .current_history_capability
                        .take()
                        .expect("the donor has a real history capability"),
                );
            }
            BackTargetBusyReceiverSlot::PendingHistoryPreparationError => {
                debug_assert!(self.pending_history_preparation_error.is_none());
                self.pending_history_preparation_error = Some(
                    donor
                        .pending_history_preparation_error
                        .take()
                        .expect("the donor has a real history preparation error"),
                );
            }
            BackTargetBusyReceiverSlot::PreparedPublicationAuthority => {
                debug_assert!(self.prepared_publication_authority.is_none());
                self.prepared_publication_authority = Some(
                    donor
                        .prepared_publication_authority
                        .take()
                        .expect("the donor has real prepared publication authority"),
                );
            }
            BackTargetBusyReceiverSlot::CurrentRequest => {
                debug_assert!(self.state.current_request().is_none());
                self.state
                    .set_request(Some(RequestStatus::Running(NavigationPhase::Matching)));
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn testing_drift_current_history_binding(&mut self) -> bool {
        let Some(current) = self.current.as_mut() else {
            return false;
        };
        let Some(alternate) = current.reservation.get(1).cloned() else {
            return false;
        };
        if current.reservation[0].frame_task() == alternate.frame_task() {
            return false;
        }
        current.reservation[0] = alternate;
        true
    }

    #[cfg(test)]
    pub(crate) fn testing_next_frame_identities(&self) -> (u64, u64) {
        let ids = self.ids.snapshot();
        (ids.next_frame(), ids.next_frame_task())
    }

    #[cfg(test)]
    pub(crate) fn testing_id_allocator_snapshot(&self) -> crate::ids::CoreIdAllocatorSnapshot {
        self.ids.snapshot()
    }

    #[cfg(test)]
    pub(crate) fn testing_mount_issuer_snapshot(&self) -> crate::mount::MountIssuerSnapshot {
        self.mount.snapshot()
    }

    #[cfg(test)]
    pub(crate) fn testing_begin_back_target_host_drift(&mut self) -> bool {
        let Some(host) = self.state.host().cloned() else {
            return false;
        };
        let Some(top_slot) = self.state.top_slot() else {
            return false;
        };
        self.state.begin_authority_recovery(&host, top_slot)
    }

    #[cfg(test)]
    pub(crate) fn testing_remove_back_no_target_host(&mut self) -> bool {
        let Some(host) = self.state.host().cloned() else {
            return false;
        };
        let Some(top_slot) = self.state.top_slot() else {
            return false;
        };
        self.state.begin_authority_recovery(&host, top_slot)
            && self.state.complete_authority_settlement(&host, top_slot)
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

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn testing_exhaust_reservation_counter(
        &mut self,
        counter: crate::ids::ReservationCounter,
    ) {
        self.ids
            .testing_exhaust_reservation_counter(self.router, counter);
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn testing_exhaust_mount_sequence_counter(&mut self) {
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
    pub(crate) fn testing_set_active_generation(&mut self, generation: RouteGeneration) {
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

fn reconciled_child_slots(current: &CurrentWork) -> Vec<LogicalChildSlotBinding> {
    let mut bindings = occupied_child_slots(current.branch.as_ref(), &current.target_frames);
    let Some(projection) = current.reconciliation.as_ref() else {
        return bindings;
    };
    for role in projection
        .child_slots()
        .iter()
        .filter(|role| role.kind() == ProjectedChildSlotRoleKind::RetainEmpty)
    {
        let Some(binding) = role.binding() else {
            continue;
        };
        let mapping_exists = bindings.iter().any(|candidate| {
            candidate.parent_branch_index() == binding.parent_branch_index()
                && candidate.outlet() == binding.outlet()
        });
        if !mapping_exists {
            bindings.push(LogicalChildSlotBinding::retained_empty(
                binding.parent_branch_index(),
                binding.parent().clone(),
                binding.outlet().clone(),
                binding.slot(),
            ));
        }
    }
    bindings
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
    fn failed_back_target(
        self,
        scope: BackResolutionScopeProjection,
        failure: NavigationFailure,
        mode: CoreBackTargetDriverFailureReportMode,
    ) -> CoreBackTargetDriverFailureReport {
        if matches!(
            mode,
            CoreBackTargetDriverFailureReportMode::PhysicalResidual
        ) {
            let request = self.failed_with_physical_residual(failure);
            return CoreBackTargetDriverFailureReport::FailureCleanup(
                CoreBackTargetDriverFailureCleanup { scope, request },
            );
        }

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
            AttemptKind::Normal(_) => format!(
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
            AttemptKind::Normal(_) if branch_index > 0 => {
                let expected = match failure.kind() {
                    NavigationFailureKind::Build => PendingFailureKind::Build,
                    NavigationFailureKind::Apply => PendingFailureKind::Apply,
                    _ => PendingFailureKind::Apply,
                };
                CoreBackTargetDriverFailureReport::FailureCleanup(
                    CoreBackTargetDriverFailureCleanup {
                        scope,
                        request: self.engine.begin_failure_cleanup(expected, failure),
                    },
                )
            }
            AttemptKind::Normal(_) => {
                let expected_attempt = CoreBackTargetReceiverAttemptProjection::Fallback {
                    original_failure: failure.clone(),
                };
                match self.engine.begin_fallback(request, failure) {
                    Some(action) => {
                        CoreBackTargetDriverFailureReport::Fallback(CoreBackTargetDriverFallback {
                            scope,
                            action,
                            expected_attempt,
                        })
                    }
                    None => {
                        CoreBackTargetDriverFailureReport::Terminal(CoreBackTargetDriverTerminal {
                            scope,
                            request,
                        })
                    }
                }
            }
            AttemptKind::Fallback { .. } => {
                self.engine.retire_history_authority();
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
                CoreBackTargetDriverFailureReport::Terminal(CoreBackTargetDriverTerminal {
                    scope,
                    request,
                })
            }
        }
    }

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
            AttemptKind::Normal(_) => format!(
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
            AttemptKind::Normal(_) if branch_index > 0 => {
                let expected = match failure.kind() {
                    NavigationFailureKind::Build => PendingFailureKind::Build,
                    NavigationFailureKind::Apply => PendingFailureKind::Apply,
                    _ => PendingFailureKind::Apply,
                };
                Some(self.engine.begin_failure_cleanup(expected, failure))
            }
            AttemptKind::Normal(_) => {
                let _ = self.engine.begin_fallback(request, failure);
                None
            }
            AttemptKind::Fallback { .. } => {
                self.engine.retire_history_authority();
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
        let has_existing_active = self.engine.state.active().kind() != ActivePresentationKind::None;
        let attempt = self
            .engine
            .current
            .as_ref()
            .expect("report was preflighted")
            .attempt
            .clone();
        let diagnostic_message = match &attempt {
            AttemptKind::Normal(_) => format!(
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
        if has_existing_active {
            let current = self
                .engine
                .current
                .as_mut()
                .expect("report was preflighted");
            if current.reconciliation.is_some() {
                let residual = current
                    .target_frames
                    .last()
                    .expect("an applying task has one target frame")
                    .clone();
                if !current.applied_frames.contains(&residual) {
                    current.applied_frames.push(residual);
                }
            }
        }
        let expected = match failure.kind() {
            NavigationFailureKind::Build => PendingFailureKind::Build,
            NavigationFailureKind::Apply => PendingFailureKind::Apply,
            _ => PendingFailureKind::Apply,
        };
        let (failure, continuation) = match attempt {
            AttemptKind::Normal(_) if branch_index == 0 => {
                (failure, FailureCleanupContinuation::BeginFallback)
            }
            AttemptKind::Normal(NavigationCommandKind::Replace) => {
                (failure, FailureCleanupContinuation::BeginFallback)
            }
            AttemptKind::Normal(NavigationCommandKind::Push) => {
                (failure, FailureCleanupContinuation::Terminal)
            }
            AttemptKind::Normal(NavigationCommandKind::Back) => {
                (failure, FailureCleanupContinuation::Terminal)
            }
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
pub(crate) struct HistoryAuthorityFixture {
    engine: RouterEngine,
}

#[cfg(test)]
impl HistoryAuthorityFixture {
    pub(crate) fn new() -> Self {
        Self {
            engine: history_fixture_ready_engine(),
        }
    }

    pub(crate) fn running_replace(target: &str) -> Self {
        let mut fixture = Self::new();
        fixture
            .engine
            .push(RouteLocation::new("/items/1"))
            .expect("fixture initial push reserves");
        history_fixture_settle_current(&mut fixture.engine);
        fixture
            .engine
            .replace(RouteLocation::new(target))
            .expect("fixture replace reserves");
        fixture
    }

    pub(crate) const fn router_identity(&self) -> RouterIdentity {
        self.engine.router
    }

    pub(crate) fn running_request(&self) -> NavigationRequestId {
        self.engine
            .running_request()
            .expect("fixture has one running request")
    }

    pub(crate) fn next_history_binding(
        &mut self,
    ) -> (HistoryExecutionBinding, PreparedPresentationBinding) {
        let reservation = self
            .engine
            .ids
            .reserve_navigation_branch(self.engine.router, 1)
            .expect("fixture history binding reserves one identity set");
        let host = self.engine.state.host().expect("fixture host is ready");
        let slot = self
            .engine
            .state
            .top_slot()
            .expect("fixture top slot is ready");
        let matched = self
            .engine
            .matcher
            .match_location(&RouteLocation::new("/"))
            .expect("fixture root matches");
        let origin = FrameOrigin::MatchedRoute(matched.routes()[0].route());
        let frame = RouteFrameVersion::new(
            self.engine.router,
            host.host(),
            slot,
            origin.clone(),
            reservation.frames()[0].frame(),
            RouteGeneration::initial(),
        );
        let identity = FrameTaskIdentity {
            router: self.engine.router,
            host: host.host(),
            host_epoch: host.epoch(),
            slot,
            request: reservation.request(),
            transaction: reservation.transaction(),
            frame_task: reservation.frames()[0].frame_task(),
            frame: frame.frame(),
            generation: frame.generation(),
            origin: origin.clone(),
            branch_index: 0,
        };
        let execution = HistoryExecutionBinding::new(
            self.engine.router,
            reservation.request(),
            NormalAttemptIdentity::new(
                host.host(),
                host.epoch(),
                slot,
                reservation.frames()[0].frame_task(),
                frame.frame(),
                frame.generation(),
                origin,
                0,
            ),
            reservation.transaction(),
            reservation.cleanup_batch(),
        );
        let cleanup = CleanupBatchToken::sealed(
            identity,
            reservation.cleanup_batch(),
            &[],
            crate::cleanup::CleanupManifestDigest::from_opaque(0x4849_5354),
            0,
        );
        let presentation = PreparedPresentationBinding::new(
            execution.projection(),
            ActivePresentation::route(RouteLocation::new("/"), vec![frame.clone()]),
            matched.branch_fingerprint().clone(),
            vec![frame],
            cleanup,
        );
        (execution, presentation)
    }

    pub(crate) fn move_canonical_cursor_back(&mut self, history: &mut CanonicalHistory) {
        history.testing_move_cursor_back();
    }

    pub(crate) fn seal_current_replace(&mut self) -> SealedCommit {
        let action = self.engine.action().expect("fixture replace has work");
        let ticket = history_fixture_drive_to_commit(&mut self.engine, action);
        history_fixture_seal_current(&mut self.engine, ticket)
    }

    pub(crate) fn clone_current_work_pre_retirement(&self) {
        let _cloned = self
            .engine
            .current
            .as_ref()
            .expect("fixture has current work")
            .clone_pre_retirement()
            .expect("fixture current work has not retired its host");
    }

    pub(crate) fn history_capability_snapshot(&self) -> HistoryCapabilitySnapshot {
        self.engine.testing_history_capability_snapshot()
    }

    pub(crate) fn engine_snapshot(&self) -> EngineSnapshot {
        self.engine.testing_snapshot()
    }

    pub(crate) fn probe_sealed_commit_evidence(
        &self,
        evidence: &SealedCommitEvidence,
    ) -> Result<(), CommitError> {
        self.engine.probe_sealed_commit_evidence(evidence)
    }

    pub(crate) fn prepare_publication(
        &mut self,
        sealed: SealedCommit,
    ) -> Result<PreparedPublication, CommitError> {
        self.engine.prepare_publication(sealed)
    }

    pub(crate) fn publish_and_return_consumed_evidence(
        &mut self,
        prepared: PreparedPublication,
    ) -> ConsumedPublicationEvidence {
        self.engine
            .publish_prepared_with_evidence(prepared, || {})
            .expect("fresh fixture publication remains exactly authorized")
    }

    pub(crate) fn probe_consumed_publication_evidence(
        &self,
        consumed: &ConsumedPublicationEvidence,
    ) -> Result<(), CommitError> {
        self.engine.probe_consumed_publication_evidence(consumed)
    }

    pub(crate) fn request_trace(&self) -> &[NavigationTraceEvent] {
        self.engine
            .request(self.running_request())
            .expect("fixture request remains retained")
            .trace()
    }
}

#[cfg(test)]
fn history_fixture_ready_engine() -> RouterEngine {
    let mut builder = RouteTree::builder()
        .root_with_options(
            crate::pattern::RoutePattern::path("/").unwrap(),
            crate::route_tree::RouteNodeOptions::new()
                .with_view(RouteViewKey::new("history-root").unwrap()),
        )
        .unwrap();
    let root = builder.root_id().unwrap();
    builder
        .route_with_options(
            root,
            crate::pattern::RoutePattern::path("items/:id").unwrap(),
            crate::route_tree::RouteNodeOptions::new()
                .with_view(RouteViewKey::new("history-items").unwrap()),
        )
        .unwrap();
    let mut engine = RouterEngine::install(
        builder.build().unwrap(),
        RouteViewKey::new("history-fallback").unwrap(),
    )
    .unwrap();
    let mut issuer = HostAdmissionIssuer::new().unwrap();
    let token = engine.request_host_admission(&mut issuer).unwrap();
    let grant = engine.grant_host_admission(&mut issuer, &token).unwrap();
    let confirmation = issuer.confirm(&grant).unwrap();
    engine.confirm_host(&issuer, &confirmation).unwrap();
    engine
}

#[cfg(test)]
fn history_fixture_drive_to_commit(
    engine: &mut RouterEngine,
    mut action: EngineAction,
) -> CommitTicket {
    loop {
        action = match action {
            EngineAction::FrameTask(task) => {
                let next_outlet = engine.current.as_ref().and_then(|current| {
                    current
                        .branch
                        .as_ref()
                        .and_then(|branch| branch.routes().get(task.identity().branch_index() + 1))
                        .map(|matched| matched.slot().clone())
                });
                engine
                    .accept_applied_report(
                        task.into_applied_report(next_outlet.into_iter().collect()),
                    )
                    .unwrap();
                engine.action().expect("fixture report advances")
            }
            EngineAction::MountRequest(request) => {
                let grant = engine.resolve_mount(request).unwrap();
                let receipt =
                    grant.attest_receipt(crate::mount::MountReceiptDigest::from_opaque(1));
                let permit = engine.accept_mount_receipt(receipt).unwrap();
                let completion =
                    permit.complete(crate::mount::MountRegistrationDigest::from_opaque(2));
                engine.complete_mount_registration(completion).unwrap();
                engine.action().expect("fixture mount advances")
            }
            EngineAction::FailureCleanup(_) => panic!("fixture success reached failure cleanup"),
            EngineAction::Commit(ticket) => return ticket,
            EngineAction::BackResolution(_) => {
                panic!("history commit fixture does not drive unresolved back")
            }
        };
    }
}

#[cfg(test)]
fn history_fixture_seal_current(engine: &mut RouterEngine, ticket: CommitTicket) -> SealedCommit {
    let roots = engine.current.as_ref().unwrap().cleanup_roots.clone();
    let count = usize::from(!roots.is_empty());
    engine
        .seal_commit(
            &ticket,
            CleanupManifestSeal::new(
                roots,
                crate::cleanup::CleanupManifestDigest::from_opaque(0x4849_5354_5345_414c),
                count,
            ),
        )
        .unwrap()
}

#[cfg(test)]
fn history_fixture_settle_current(engine: &mut RouterEngine) {
    let action = engine.action().expect("fixture request has work");
    let ticket = history_fixture_drive_to_commit(engine, action);
    let sealed = history_fixture_seal_current(engine, ticket);
    let cleanup = sealed.cleanup().clone();
    let prepared = engine.prepare_publication(sealed).unwrap();
    engine.publish_prepared(prepared, || {}).unwrap();
    let digest = if cleanup.obligation_count() == 0 {
        crate::cleanup::CleanupCompletionDigest::canonical_empty()
    } else {
        crate::cleanup::CleanupCompletionDigest::from_opaque(0x4849_5354_444f_4e45)
    };
    engine
        .complete_cleanup(CleanupCompletion::attest(
            cleanup.clone(),
            digest,
            cleanup.obligation_count(),
        ))
        .unwrap();
}

#[cfg(test)]
#[derive(Clone, Debug, Eq, PartialEq)]
enum ActionSnapshot {
    FrameTask(RootTaskIdentity),
    MountRequest(NavigationRequestId, RouteFrameVersion, RouteSlotKey),
    FailureCleanup(NavigationRequestId),
    Commit(RootTaskIdentity),
    BackResolution,
}

#[cfg(test)]
#[derive(Clone, Debug, PartialEq)]
struct PreparedPublicationOwnerSnapshot {
    identity: FrameTaskIdentity,
    evidence: SealedCommitEvidence,
    source: String,
    state: RouterNavigationState,
    history: Option<HistoryCapabilitySnapshot>,
    active_branch: Option<String>,
    active_child_slots: Vec<LogicalChildSlotBinding>,
    record_index: usize,
    published_record: NavigationRequestRecord,
    current: String,
}

#[cfg(test)]
impl From<&SealedCommit> for PreparedPublicationOwnerSnapshot {
    fn from(owner: &SealedCommit) -> Self {
        Self {
            identity: owner.identity.clone(),
            evidence: owner.evidence.clone(),
            source: format!("{:?}", owner.source),
            state: owner.publication.state.clone(),
            history: owner
                .publication
                .history
                .as_ref()
                .map(PreparedHistoryPublication::capability_snapshot),
            active_branch: owner
                .publication
                .active_branch
                .as_ref()
                .map(|branch| format!("{branch:?}")),
            active_child_slots: owner.publication.active_child_slots.clone(),
            record_index: owner.publication.record_index,
            published_record: owner.publication.published_record.clone(),
            current: format!("{:?}", owner.publication.current),
        }
    }
}

#[cfg(test)]
#[derive(Clone, Debug, PartialEq)]
enum PreparedPublicationAuthoritySnapshot {
    Ready(PreparedPublicationOwnerSnapshot),
    Rejected {
        owner: PreparedPublicationOwnerSnapshot,
        error: CommitError,
        observed: String,
    },
}

#[cfg(test)]
impl From<&PreparedPublicationAuthority> for PreparedPublicationAuthoritySnapshot {
    fn from(authority: &PreparedPublicationAuthority) -> Self {
        match authority {
            PreparedPublicationAuthority::Ready(owner) => Self::Ready(owner.into()),
            PreparedPublicationAuthority::Rejected(rejection) => Self::Rejected {
                owner: (&rejection.owner).into(),
                error: rejection.error,
                observed: format!("{:?}", rejection.observed),
            },
        }
    }
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
            EngineAction::BackResolution(_) => Self::BackResolution,
        }
    }
}

#[cfg(test)]
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct EngineSnapshot {
    state: RouterNavigationState,
    ids: crate::ids::CoreIdAllocatorSnapshot,
    history: HistoryAuthoritySnapshot,
    history_admission_world: Option<CoreIssuedWorldSealProjection>,
    active_branch: Option<String>,
    active_child_slots: Vec<LogicalChildSlotBinding>,
    records: VecDeque<NavigationRequestRecord>,
    evicted_through: Option<u64>,
    highest_issued: Option<u64>,
    queue: Vec<String>,
    current: Option<String>,
    pending_no_target_back: Option<String>,
    current_history_capability: Option<HistoryCapabilitySnapshot>,
    pending_history_preparation_error: Option<HistoryDriftEvidenceSnapshot>,
    prepared_publication_authority: Option<PreparedPublicationAuthoritySnapshot>,
    pending_back_no_target_authority: Option<&'static str>,
    pending_back_target_permanent_cleanup: Option<&'static str>,
    action: Option<ActionSnapshot>,
    issued_view_tasks: Vec<FrameTaskIdentity>,
    mount: crate::mount::MountIssuerSnapshot,
    report_dispositions: Vec<ReportDispositionRecord>,
    router_diagnostics: Vec<DiagnosticEvent>,
    completed_cleanup: Vec<CompletedCleanupTombstone>,
    next_failed_authority_trigger_revision: u64,
    live_failed_authority_trigger_revision: Option<u64>,
}

#[cfg(test)]
impl EngineSnapshot {
    pub(crate) fn without_back_resolution_state_for(
        mut self,
        request: NavigationRequestId,
    ) -> Self {
        self.records
            .iter_mut()
            .find(|record| record.id() == request)
            .expect("the compared Back request remains retained")
            .testing_clear_back_resolution();
        self
    }
}

#[cfg(test)]
mod tests {
    use super::CoreBackTargetPermanentBeginOwner;
    use crate::admission::HostLease;
    use std::collections::VecDeque;

    use super::{
        ActionSnapshot, BackNoTargetSourceDriftSettlementOutcome,
        BackNoTargetSourceRefreezeOutcome, BackTargetPermanentSettlementOutcome,
        BackTargetResumeAction, BackTargetResumeOutcome, BackTargetStart, CleanupError,
        CommitError, CoreBackNoTargetPublicationPreparationFailureProjection,
        CoreBackNoTargetReadyMismatchProjection, CorePreparationReceiverMismatchProjection,
        EngineAction, MountError, NavigationCommand, PrivateRequestLookup, PushError,
        QueuedNavigation, QueuedReservation, ROUTER_DIAGNOSTIC_RETENTION, ReportError,
        RootTaskIdentity, RouterEngine, TERMINAL_RETENTION, WorkStage, occupied_child_slots,
    };
    use crate::admission::{AdmissionError, HostAdmissionIssuer};
    use crate::cleanup::{
        CleanupBatchToken, CleanupCompletion, CleanupCompletionDigest, CleanupManifestDigest,
        CleanupManifestSeal,
    };
    use crate::diagnostics::ReportDisposition;
    use crate::frame::{
        FrameOrigin, LogicalChildSlotBinding, ProjectedChildSlotRoleKind, ProjectedEnteringSlot,
        RouteFrameVersion,
    };
    use crate::history_authority::{
        BackResolutionEvidenceClassification, BackResolutionVerifyOutcome,
        BackTargetPermanentCauseSnapshot, VerifiedBackResolutionOwnerHandoff,
    };
    use crate::ids::{
        CleanupBatchId, ReservationCounter, RouteFrameId, RouteGeneration, RouteHostId, RouteSlotId,
    };
    use crate::location::RouteLocation;
    use crate::pattern::RoutePattern;
    use crate::presentation::{ActivePresentation, ActivePresentationKind};
    use crate::request::{
        NavigationFailure, NavigationFailureKind, NavigationPhase, RequestRecordError,
        RequestStatus,
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
        ready_engine_with_tree_and_authority(tree).0
    }

    fn ready_engine_with_tree_and_authority(
        tree: RouteTree,
    ) -> (RouterEngine, HostAdmissionIssuer, HostLease) {
        let mut engine =
            RouterEngine::install(tree, RouteViewKey::new("fallback").unwrap()).unwrap();
        let mut issuer = HostAdmissionIssuer::new().unwrap();
        let token = engine.request_host_admission(&mut issuer).unwrap();
        let grant = engine.grant_host_admission(&mut issuer, &token).unwrap();
        let confirmation = issuer.confirm(&grant).unwrap();
        engine.confirm_host(&issuer, &confirmation).unwrap();
        let lease = confirmation.into_lease();
        (engine, issuer, lease)
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

    #[test]
    fn back_no_target_publication_preparation_retains_typed_failure_evidence() {
        let mut engine = ready_engine();
        let request = engine.push(RouteLocation::new("/items/1")).unwrap();

        let missing = match RouterEngine::prepare_back_no_target_publication(
            VecDeque::new(),
            engine.state.clone(),
            request,
        ) {
            Err(failure) => failure,
            Ok(_) => panic!("a missing request record must not prepare publication"),
        };
        assert_eq!(
            RouterEngine::back_no_target_publication_preparation_mismatch(request, missing),
            CorePreparationReceiverMismatchProjection::BackNoTargetReady(
                CoreBackNoTargetReadyMismatchProjection::PublicationPreparation {
                    request,
                    failure: CoreBackNoTargetPublicationPreparationFailureProjection::MissingRecord,
                }
            )
        );

        let trace = RouterEngine::back_no_target_publication_preparation_mismatch(
            request,
            CoreBackNoTargetPublicationPreparationFailureProjection::HistoryIntentRejectedTrace(
                RequestRecordError::TraceClosed,
            ),
        );
        assert_eq!(
            trace,
            CorePreparationReceiverMismatchProjection::BackNoTargetReady(
                CoreBackNoTargetReadyMismatchProjection::PublicationPreparation {
                    request,
                    failure:
                        CoreBackNoTargetPublicationPreparationFailureProjection::HistoryIntentRejectedTrace(
                            RequestRecordError::TraceClosed
                        ),
                }
            )
        );

        let cleanup_pending_trace = RouterEngine::back_no_target_publication_preparation_mismatch(
            request,
            CoreBackNoTargetPublicationPreparationFailureProjection::CleanupPendingTrace(
                RequestRecordError::TraceClosed,
            ),
        );
        assert_eq!(
            cleanup_pending_trace,
            CorePreparationReceiverMismatchProjection::BackNoTargetReady(
                CoreBackNoTargetReadyMismatchProjection::PublicationPreparation {
                    request,
                    failure:
                        CoreBackNoTargetPublicationPreparationFailureProjection::CleanupPendingTrace(
                            RequestRecordError::TraceClosed
                        ),
                }
            )
        );

        let mut terminal_records = engine.records.clone();
        let record = terminal_records
            .iter_mut()
            .find(|record| record.id() == request)
            .expect("the real request record remains retained");
        record
            .transition(RequestStatus::Failed(NavigationFailure::new(
                NavigationFailureKind::AuthorityLost,
                "typed preparation failure fixture",
            )))
            .unwrap();
        let transition = match RouterEngine::prepare_back_no_target_publication(
            terminal_records,
            engine.state.clone(),
            request,
        ) {
            Err(failure) => failure,
            Ok(_) => panic!("a terminal request record must not re-enter CleanupPending"),
        };
        assert_eq!(
            RouterEngine::back_no_target_publication_preparation_mismatch(request, transition),
            CorePreparationReceiverMismatchProjection::BackNoTargetReady(
                CoreBackNoTargetReadyMismatchProjection::PublicationPreparation {
                    request,
                    failure:
                        CoreBackNoTargetPublicationPreparationFailureProjection::CleanupPendingTransition(
                            RequestRecordError::AlreadyTerminal
                        ),
                }
            )
        );
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
                EngineAction::BackResolution(_) => {
                    panic!("commit helper does not drive unresolved back")
                }
            }
        }
    }

    fn back_target_permanent_begin_fixture() -> (
        RouterEngine,
        crate::history_authority::BackResolutionScopeProjection,
        super::CommitTicket,
        Vec<RouteFrameVersion>,
        HostAdmissionIssuer,
        HostLease,
    ) {
        let (mut engine, issuer, lease) = ready_engine_with_tree_and_authority(tree());
        engine.push(RouteLocation::new("/")).unwrap();
        settle_current(&mut engine);
        engine.push(RouteLocation::new("/about")).unwrap();
        settle_current(&mut engine);

        let world = engine
            .claim_history_admission_world()
            .expect("the confirmed fixture issues one history world seal");
        let stamp = engine
            .history_admission_stamp(&world)
            .expect("the two-entry history admits Back");
        let admission = engine
            .prepare_back_admission(&world, &stamp)
            .expect("the target Back exact-refreezes the live history");
        let accepted = engine.enqueue_back(admission).unwrap();
        engine.collect();
        let handoff = match engine.action().unwrap() {
            EngineAction::BackResolution(handoff) => handoff,
            _ => panic!("the accepted Back must produce a real resolution handoff"),
        };
        let (_request, view) = accepted.into_parts();
        let verified = match engine.verify_back_resolution_handoff(view, handoff) {
            BackResolutionVerifyOutcome::Matched(verified) => verified,
            BackResolutionVerifyOutcome::Mismatch { .. } => {
                panic!("the exact Back lineage must verify")
            }
        };
        let (_view, owner) = verified.into_parts();
        let target = match owner {
            VerifiedBackResolutionOwnerHandoff::Target(target) => target,
            VerifiedBackResolutionOwnerHandoff::NoTarget(_)
            | VerifiedBackResolutionOwnerHandoff::Contradiction(_) => {
                panic!("the two-entry history must resolve its predecessor")
            }
        };
        let resume = match engine.resume_back_target_with_retirement(target, || {}) {
            BackTargetResumeOutcome::Resumed(BackTargetStart::Ready(resume)) => resume,
            BackTargetResumeOutcome::Resumed(BackTargetStart::FailureCleanup(_))
            | BackTargetResumeOutcome::Permanent(_)
            | BackTargetResumeOutcome::Mismatch(_) => {
                panic!("the exact target Back must reach a normal action")
            }
        };
        let super::BackTargetResume {
            action,
            execution: _,
            continuation,
        } = resume;
        let action = match action {
            BackTargetResumeAction::FrameTask(task) => EngineAction::FrameTask(task),
            BackTargetResumeAction::MountRequest(request) => EngineAction::MountRequest(request),
            BackTargetResumeAction::Commit(ticket) => EngineAction::Commit(ticket),
        };
        let ticket = drive_to_commit(&mut engine, action);
        let logical_roots = engine
            .current
            .as_ref()
            .expect("the ReadyToCommit fixture retains current work")
            .cleanup_roots
            .clone();
        (engine, continuation, ticket, logical_roots, issuer, lease)
    }

    fn real_retired_host_authority() -> (
        crate::ids::RouterIdentity,
        crate::admission::HostVersion,
        RouteSlotId,
        crate::admission::RetiredHostAuthority,
    ) {
        let mut donor =
            RouterEngine::install(tree(), RouteViewKey::new("fallback").unwrap()).unwrap();
        let mut issuer = HostAdmissionIssuer::new().unwrap();
        let token = donor.request_host_admission(&mut issuer).unwrap();
        let grant = donor.grant_host_admission(&mut issuer, &token).unwrap();
        let confirmation = issuer.confirm(&grant).unwrap();
        donor.confirm_host(&issuer, &confirmation).unwrap();
        let router = donor.router;
        let version = confirmation.version().clone();
        let top_slot = confirmation.top_slot();
        let authority = issuer.retire_current(confirmation.into_lease()).unwrap();
        assert!(authority.matches(router, &version, top_slot));
        (router, version, top_slot, authority)
    }

    fn real_pending_mount() -> super::PendingMount {
        let mut donor = ready_engine();
        donor.push(RouteLocation::new("/items/1")).unwrap();
        let task = match donor.action().unwrap() {
            EngineAction::FrameTask(task) => task,
            _ => panic!("the nested route must begin with its real root frame task"),
        };
        let next_outlet = donor
            .current
            .as_ref()
            .and_then(|current| current.branch.as_ref())
            .and_then(|branch| branch.routes().get(task.identity().branch_index() + 1))
            .map(|matched| matched.slot().clone())
            .expect("the nested route retains its real child outlet");
        donor
            .accept_applied_report(task.into_applied_report(vec![next_outlet]))
            .unwrap();
        assert!(matches!(
            donor.action.as_ref(),
            Some(EngineAction::MountRequest(_))
        ));
        donor
            .current
            .as_mut()
            .and_then(|current| current.pending_mount.take())
            .expect("the real mount request installs its pending mount fact")
    }

    #[test]
    fn back_target_permanent_begin_rejects_noncanonical_state_frozen_before_acceptance() {
        #[derive(Clone, Copy, Debug)]
        enum NoncanonicalFact {
            CurrentLocation,
            LogicalRoots,
            Reconciliation,
            Availability,
            PendingRelation,
        }

        for fact in [
            NoncanonicalFact::CurrentLocation,
            NoncanonicalFact::LogicalRoots,
            NoncanonicalFact::Reconciliation,
            NoncanonicalFact::Availability,
            NoncanonicalFact::PendingRelation,
        ] {
            let (mut engine, scope, ticket, actual_logical_roots, mut issuer, lease) =
                back_target_permanent_begin_fixture();
            let mut submitted_logical_roots = actual_logical_roots.clone();
            match fact {
                NoncanonicalFact::CurrentLocation => {
                    let requested = engine
                        .records
                        .iter()
                        .find(|record| record.id() == scope.request())
                        .and_then(|record| record.requested())
                        .cloned()
                        .expect("the Back request retains its requested target");
                    let current = engine
                        .current
                        .as_mut()
                        .expect("the fixture retains ReadyToCommit work");
                    assert_eq!(current.location, requested);
                    current.location = RouteLocation::new("/items/noncanonical-current");
                }
                NoncanonicalFact::LogicalRoots => {
                    assert!(
                        !actual_logical_roots.is_empty(),
                        "the real Back splice fixture must retain cleanup roots"
                    );
                    submitted_logical_roots.clear();
                }
                NoncanonicalFact::Reconciliation => {
                    assert!(
                        engine
                            .current
                            .as_ref()
                            .and_then(|current| current.reconciliation.as_ref())
                            .is_some(),
                        "the real Back fixture retains its reconciliation proof"
                    );
                    assert!(
                        engine.active_branch.take().is_some(),
                        "the seam changes the real active-side reconciliation input"
                    );
                }
                NoncanonicalFact::Availability => {
                    let host = engine
                        .state
                        .host()
                        .cloned()
                        .expect("the host remains exact");
                    let top_slot = engine.state.top_slot().expect("the top slot remains exact");
                    assert!(engine.state.begin_authority_recovery(&host, top_slot));
                }
                NoncanonicalFact::PendingRelation => {
                    let mut donor = ready_engine();
                    donor.push(RouteLocation::new("/")).unwrap();
                    settle_current(&mut donor);
                    engine.testing_move_back_target_busy_receiver_slot_from(
                        &mut donor,
                        super::BackTargetBusyReceiverSlot::PendingNoTargetBack,
                    );
                    assert!(engine.pending_no_target_back.is_some());
                }
            }

            let settlement = engine.prepare_back_target_permanent_settlement_owner(
                scope,
                submitted_logical_roots.clone(),
            );
            let expected_scope = settlement.scope.clone();
            let expected_current = settlement.expected_current.clone();
            let expected_request = settlement.expected_request.clone();
            let expected_roots = settlement.logical_roots.clone();
            let before = engine.snapshot();
            let callback_calls = std::cell::Cell::new(0usize);
            let installation = format!("{fact:?}");

            let evidence = match engine.begin_back_target_permanent_cleanup(
                &mut issuer,
                CoreBackTargetPermanentBeginOwner::new(settlement, lease),
                &ticket,
                submitted_logical_roots,
                0x4341_4e4f_4e49_4341,
                NONZERO_OBLIGATION_COUNT,
                installation.clone(),
                |_, _| callback_calls.set(callback_calls.get() + 1),
            ) {
                super::BackTargetPermanentBeginCoreOutcome::Permanent {
                    owner,
                    installation: returned_installation,
                    evidence,
                } => {
                    let (owner, _lease) = owner.into_parts();
                    assert_eq!(owner.scope, expected_scope);
                    assert_eq!(owner.expected_current, expected_current);
                    assert_eq!(owner.expected_request, expected_request);
                    assert_eq!(owner.logical_roots, expected_roots);
                    assert_eq!(returned_installation, installation);
                    evidence
                }
                super::BackTargetPermanentBeginCoreOutcome::Begun
                | super::BackTargetPermanentBeginCoreOutcome::ReceiverMismatch { .. }
                | super::BackTargetPermanentBeginCoreOutcome::LogicalRootsMismatch { .. } => {
                    panic!("{fact:?} must not pass or be downgraded to receiver/argument mismatch")
                }
            };
            assert_eq!(callback_calls.get(), 0, "fact: {fact:?}");
            assert_eq!(engine.snapshot(), before, "fact: {fact:?}");

            let evidence = format!("{evidence:?}");
            assert!(
                evidence.contains("CanonicalRelationContradiction"),
                "{fact:?} must report purpose-specific typed canonical relation evidence, got {evidence}"
            );
            assert!(
                evidence.contains("expected:") && evidence.contains("observed:"),
                "{fact:?} canonical evidence must retain named expected/observed relations"
            );
            assert!(
                !evidence.contains("CurrentStateContradiction"),
                "{fact:?} was already noncanonical when frozen and is not ordinary snapshot drift"
            );
        }
    }

    #[test]
    fn back_target_permanent_begin_rejects_owned_noncanonical_relations_losslessly() {
        {
            let (mut engine, scope, ticket, logical_roots, mut issuer, lease) =
                back_target_permanent_begin_fixture();
            let (router, version, top_slot, retired_host) = real_retired_host_authority();
            engine
                .current
                .as_mut()
                .expect("the fixture retains ReadyToCommit work")
                .retired_host = Some(retired_host);

            let settlement =
                engine.prepare_back_target_permanent_settlement_owner(scope, logical_roots.clone());
            let expected_scope = settlement.scope.clone();
            let expected_current = settlement.expected_current.clone();
            let expected_request = settlement.expected_request.clone();
            let expected_roots = settlement.logical_roots.clone();
            let before = engine.snapshot();
            let callback_calls = std::cell::Cell::new(0usize);
            let installation = "owned-retired-host";

            let evidence = match engine.begin_back_target_permanent_cleanup(
                &mut issuer,
                CoreBackTargetPermanentBeginOwner::new(settlement, lease),
                &ticket,
                logical_roots,
                0x4f57_4e45_445f_524c,
                NONZERO_OBLIGATION_COUNT,
                installation,
                |_, _| callback_calls.set(callback_calls.get() + 1),
            ) {
                super::BackTargetPermanentBeginCoreOutcome::Permanent {
                    owner,
                    installation: returned_installation,
                    evidence,
                } => {
                    let (owner, _lease) = owner.into_parts();
                    assert_eq!(owner.scope, expected_scope);
                    assert_eq!(owner.expected_current, expected_current);
                    assert_eq!(owner.expected_request, expected_request);
                    assert_eq!(owner.logical_roots, expected_roots);
                    assert_eq!(returned_installation, installation);
                    evidence
                }
                super::BackTargetPermanentBeginCoreOutcome::Begun
                | super::BackTargetPermanentBeginCoreOutcome::ReceiverMismatch { .. }
                | super::BackTargetPermanentBeginCoreOutcome::LogicalRootsMismatch { .. } => {
                    panic!("owned retired-host contradiction must fail closed")
                }
            };
            match evidence {
                super::BackTargetPermanentBeginCorePermanentEvidence::CanonicalRelationContradiction {
                    expected,
                    observed,
                } => {
                    assert!(expected.retired_host.is_none());
                    assert!(
                        observed
                            .retired_host
                            .as_ref()
                            .is_some_and(|projection| projection.matches(
                                router,
                                &version,
                                top_slot
                            )),
                        "canonical evidence must retain the exact retired-host identity"
                    );
                }
                other => panic!(
                    "owned retired-host contradiction needs canonical relation evidence, got {other:?}"
                ),
            }
            assert_eq!(callback_calls.get(), 0);
            assert_eq!(engine.snapshot(), before);
            assert!(
                engine
                    .current
                    .as_ref()
                    .and_then(|current| current.retired_host.as_ref())
                    .is_some_and(|authority| authority.matches(router, &version, top_slot)),
                "rejection must leave the sole authority in current"
            );
        }

        {
            let (mut engine, scope, ticket, logical_roots, mut issuer, lease) =
                back_target_permanent_begin_fixture();
            let pending_mount = real_pending_mount();
            let expected_child_index = pending_mount.child_index;
            let expected_parent = pending_mount.parent.clone();
            let expected_outlet = pending_mount.outlet.clone();
            let expected_from_preserved_parent = pending_mount.from_preserved_parent;
            engine
                .current
                .as_mut()
                .expect("the fixture retains ReadyToCommit work")
                .pending_mount = Some(pending_mount);

            let settlement =
                engine.prepare_back_target_permanent_settlement_owner(scope, logical_roots.clone());
            let expected_scope = settlement.scope.clone();
            let expected_current = settlement.expected_current.clone();
            let expected_request = settlement.expected_request.clone();
            let expected_roots = settlement.logical_roots.clone();
            let before = engine.snapshot();
            let callback_calls = std::cell::Cell::new(0usize);
            let installation = "owned-pending-mount";

            let evidence = match engine.begin_back_target_permanent_cleanup(
                &mut issuer,
                CoreBackTargetPermanentBeginOwner::new(settlement, lease),
                &ticket,
                logical_roots,
                0x4f57_4e45_445f_524c,
                NONZERO_OBLIGATION_COUNT,
                installation,
                |_, _| callback_calls.set(callback_calls.get() + 1),
            ) {
                super::BackTargetPermanentBeginCoreOutcome::Permanent {
                    owner,
                    installation: returned_installation,
                    evidence,
                } => {
                    let (owner, _lease) = owner.into_parts();
                    assert_eq!(owner.scope, expected_scope);
                    assert_eq!(owner.expected_current, expected_current);
                    assert_eq!(owner.expected_request, expected_request);
                    assert_eq!(owner.logical_roots, expected_roots);
                    assert_eq!(returned_installation, installation);
                    evidence
                }
                super::BackTargetPermanentBeginCoreOutcome::Begun
                | super::BackTargetPermanentBeginCoreOutcome::ReceiverMismatch { .. }
                | super::BackTargetPermanentBeginCoreOutcome::LogicalRootsMismatch { .. } => {
                    panic!("owned pending-mount contradiction must fail closed")
                }
            };
            let observed = match evidence {
                super::BackTargetPermanentBeginCorePermanentEvidence::CanonicalRelationContradiction {
                    expected,
                    observed,
                } => {
                    assert!(expected.pending_mount.is_none());
                    observed
                        .pending_mount
                        .expect("canonical evidence must retain the pending-mount projection")
                }
                other => panic!(
                    "owned pending-mount contradiction needs canonical relation evidence, got {other:?}"
                ),
            };
            assert_eq!(observed.child_index, expected_child_index);
            assert_eq!(observed.parent, expected_parent);
            assert_eq!(observed.outlet, expected_outlet);
            assert_eq!(
                observed.from_preserved_parent,
                expected_from_preserved_parent
            );
            assert_eq!(callback_calls.get(), 0);
            assert_eq!(engine.snapshot(), before);
            let current = engine
                .current
                .as_ref()
                .and_then(|current| current.pending_mount.as_ref())
                .expect("rejection must leave the pending mount in current");
            assert_eq!(current.child_index, expected_child_index);
            assert_eq!(current.parent, expected_parent);
            assert_eq!(current.outlet, expected_outlet);
            assert_eq!(
                current.from_preserved_parent,
                expected_from_preserved_parent
            );
        }

        fn braced_item<'a>(source: &'a str, marker: &str) -> &'a str {
            let tail = &source[source
                .find(marker)
                .expect("typed contract item must remain named")..];
            let open = tail
                .find('{')
                .expect("typed contract item must have a body");
            let mut depth = 0usize;
            for (offset, character) in tail[open..].char_indices() {
                match character {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            return &tail[..open + offset + character.len_utf8()];
                        }
                    }
                    _ => {}
                }
            }
            panic!("typed contract item body must close")
        }

        fn compact(source: &str) -> String {
            source
                .chars()
                .filter(|character| !character.is_whitespace())
                .collect::<String>()
        }

        let admission_and_tests = include_str!("../../bevy_ui_router_core/src/admission.rs");
        let admission = &admission_and_tests[..admission_and_tests
            .find("mod tests {")
            .expect("the admission production/test boundary must remain explicit")];
        let retired_projection = braced_item(admission, "struct RetiredHostAuthorityProjection");
        let compact_retired_projection = compact(retired_projection);
        let exact_projection = compact_retired_projection.contains("key:AdmissionKey")
            && compact_retired_projection.contains("version:HostVersion")
            && compact_retired_projection.contains("top_slot:RouteSlotId");
        assert!(
            exact_projection
                && !compact_retired_projection.contains(":RetiredHostAuthority,")
                && !compact_retired_projection.contains(":Option<RetiredHostAuthority>")
                && !compact_retired_projection.contains("bool")
                && !compact_retired_projection.contains(":()"),
            "admission must expose an exact, typed, non-authorizing retired-host identity projection"
        );
        let admission_key = compact(braced_item(admission, "struct AdmissionKey"));
        for exact_field in [
            "issuer:AdmissionIssuerId",
            "router:RouterIdentity",
            "sequence:u64",
        ] {
            assert!(
                admission_key.contains(exact_field),
                "AdmissionKey must retain exact {exact_field} identity"
            );
        }
        let retired_authority_marker = admission
            .find("struct RetiredHostAuthority")
            .expect("retired-host authority must remain explicit");
        let retired_authority_derive = &admission[..retired_authority_marker][admission
            [..retired_authority_marker]
            .rfind("#[derive(")
            .expect("retired-host authority must retain an explicit derive list")..];
        assert!(
            !retired_authority_derive.contains("Clone"),
            "linear retired-host authority must not implement Clone"
        );
        let compact_admission = compact(admission);
        assert_eq!(
            compact_admission.matches("RetiredHostAuthority{").count(),
            3,
            "retired-host authority may appear only in its declaration, sole retirement construction, and impl"
        );
        assert_eq!(
            compact_admission
                .matches("Ok(RetiredHostAuthority{")
                .count(),
            1,
            "retire_current must remain the sole retired-host authority constructor"
        );
        let retired_authority_impl = compact(braced_item(admission, "impl RetiredHostAuthority"));
        let projection_method = compact(braced_item(&retired_authority_impl, "fnprojection"));
        for exact_mapping in [
            "key:self.identity.key",
            "version:self.identity.version.clone()",
            "top_slot:self.identity.top_slot",
        ] {
            assert!(
                projection_method.contains(exact_mapping),
                "borrowed projection must map exact authority identity: {exact_mapping}"
            );
        }

        let engine_and_tests = include_str!("../../bevy_ui_router_core/src/engine.rs");
        let engine = &engine_and_tests[..engine_and_tests
            .find("mod tests {")
            .expect("the engine production/test boundary must remain explicit")];
        let current_projection = braced_item(
            engine,
            "struct CoreBackTargetPermanentCurrentStateProjection",
        );
        let canonical_projection = braced_item(
            engine,
            "struct CoreBackTargetPermanentCanonicalRelationProjection",
        );
        let evidence = braced_item(engine, "enum BackTargetPermanentBeginCorePermanentEvidence");
        for projection_or_evidence in [current_projection, canonical_projection, evidence] {
            let projection_or_evidence = compact(projection_or_evidence);
            assert!(
                !projection_or_evidence.contains("Option<RetiredHostAuthority>")
                    && !projection_or_evidence.contains(":RetiredHostAuthority,"),
                "permanent validation evidence must not hold retired-host authority"
            );
        }
        assert!(
            compact(current_projection)
                .contains("current_retired_host:Option<RetiredHostAuthorityProjection>")
                && compact(canonical_projection)
                    .contains("retired_host:Option<RetiredHostAuthorityProjection>"),
            "permanent begin must retain the admission projection, not a parallel Core identity type"
        );
        let compact_engine = compact(engine);
        let named_borrow_count = compact_engine
            .matches("current.retired_host.as_ref().map(RetiredHostAuthority::projection)")
            .count();
        let closure_borrow_count = compact_engine
            .matches("current.retired_host.as_ref().map(|authority|authority.projection())")
            .count();
        assert!(
            named_borrow_count + closure_borrow_count >= 2
                && !compact_engine.contains("retired_host.clone()"),
            "both permanent projection builders must borrow retired-host authority without cloning"
        );

        let integration_and_tests = include_str!("../../bevy_ui_router_core/src/integration.rs");
        let integration = &integration_and_tests[..integration_and_tests
            .find("mod tests {")
            .unwrap_or(integration_and_tests.len())];
        assert!(
            !compact(integration).contains("RetiredHostAuthority"),
            "integration evidence must not acquire or clone retired-host authority"
        );
    }

    #[test]
    fn back_target_permanent_canonical_evidence_is_typed_and_snapshot_drift_stays_distinct() {
        let (mut engine, scope, ticket, logical_roots, mut issuer, lease) =
            back_target_permanent_begin_fixture();
        let settlement =
            engine.prepare_back_target_permanent_settlement_owner(scope, logical_roots.clone());
        let expected_scope = settlement.scope.clone();
        let expected_current = settlement.expected_current.clone();
        let expected_request = settlement.expected_request.clone();
        let expected_roots = settlement.logical_roots.clone();
        engine
            .current
            .as_mut()
            .expect("the fixture retains ReadyToCommit work")
            .reconciliation = None;
        let before = engine.snapshot();
        let callback_calls = std::cell::Cell::new(0usize);

        match engine.begin_back_target_permanent_cleanup(
            &mut issuer,
            CoreBackTargetPermanentBeginOwner::new(settlement, lease),
            &ticket,
            logical_roots,
            0x534e_4150_5f44_5246,
            NONZERO_OBLIGATION_COUNT,
            "snapshot-drift-installation",
            |_, _| callback_calls.set(callback_calls.get() + 1),
        ) {
            super::BackTargetPermanentBeginCoreOutcome::Permanent {
                owner,
                installation,
                evidence:
                    super::BackTargetPermanentBeginCorePermanentEvidence::CurrentStateContradiction {
                        expected,
                        observed,
                    },
            } => {
                let (owner, _lease) = owner.into_parts();
                assert_eq!(owner.scope, expected_scope);
                assert_eq!(owner.expected_current, expected_current);
                assert_eq!(owner.expected_request, expected_request);
                assert_eq!(owner.logical_roots, expected_roots);
                assert_eq!(installation, "snapshot-drift-installation");
                assert_ne!(expected, observed);
            }
            _ => panic!("post-freeze projection drift must remain CurrentStateContradiction"),
        }
        assert_eq!(callback_calls.get(), 0);
        assert_eq!(engine.snapshot(), before);

        fn item_between<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
            let tail = &source[source.find(start).expect("contract item must remain named")..];
            &tail[..tail.find(end).expect("contract boundary must remain named")]
        }

        fn braced_item<'a>(source: &'a str, marker: &str) -> &'a str {
            let tail = &source[source
                .find(marker)
                .expect("typed contract item must remain named")..];
            let open = tail
                .find('{')
                .expect("typed contract item must have a body");
            let mut depth = 0usize;
            for (offset, character) in tail[open..].char_indices() {
                match character {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            return &tail[..open + offset + character.len_utf8()];
                        }
                    }
                    _ => {}
                }
            }
            panic!("typed contract item body must close")
        }

        let engine_source = include_str!("../../bevy_ui_router_core/src/engine.rs");
        let production_source = &engine_source[..engine_source
            .find("mod tests {")
            .expect("the production/test boundary must remain explicit")];
        let evidence = item_between(
            production_source,
            "pub(crate) enum BackTargetPermanentBeginCorePermanentEvidence",
            "pub(crate) struct CoreBackTargetPermanentCurrentStateProjection",
        );
        assert!(
            evidence.contains("CanonicalRelationContradiction"),
            "pre-freeze canonical rejection needs a purpose-specific evidence variant"
        );
        let canonical = braced_item(evidence, "CanonicalRelationContradiction");
        let compact_canonical = canonical
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect::<String>();
        assert!(
            compact_canonical
                .contains("expected:CoreBackTargetPermanentCanonicalRelationProjection")
                && compact_canonical
                    .contains("observed:CoreBackTargetPermanentCanonicalRelationProjection"),
            "canonical rejection must carry the complete typed expected/observed relation pair"
        );
        assert!(
            !compact_canonical.contains("canonical:bool"),
            "canonical relation evidence must not collapse to a hidden boolean"
        );

        let projection = braced_item(
            production_source,
            "struct CoreBackTargetPermanentCanonicalRelationProjection",
        );
        for relation in [
            "request",
            "location",
            "logical_roots",
            "cleanup_roots",
            "reconciliation",
            "active",
            "stage",
            "availability",
            "retired_host",
            "pending_mount",
            "forbidden_pending",
        ] {
            assert!(
                projection.contains(relation),
                "the compact canonical relation projection must name {relation}"
            );
        }
    }

    #[test]
    fn back_target_permanent_request_contradiction_preserves_presence_and_trace_contract() {
        let (mut engine, scope, ticket, logical_roots, mut issuer, lease) =
            back_target_permanent_begin_fixture();
        let settlement = engine
            .prepare_back_target_permanent_settlement_owner(scope.clone(), logical_roots.clone());
        let expected_scope = settlement.scope.clone();
        let expected_current = settlement.expected_current.clone();
        let expected_request = settlement.expected_request.clone();
        let expected_roots = settlement.logical_roots.clone();
        engine
            .records
            .iter_mut()
            .find(|record| record.id() == scope.request())
            .expect("the trace fixture retains its current request")
            .testing_close_trace();
        let before = engine.snapshot();
        let callback_calls = std::cell::Cell::new(0usize);

        let source = match engine.begin_back_target_permanent_cleanup(
            &mut issuer,
            CoreBackTargetPermanentBeginOwner::new(settlement, lease),
            &ticket,
            logical_roots,
            0x5452_4143_455f_4f4e,
            NONZERO_OBLIGATION_COUNT,
            "trace-only-installation",
            |_, _| callback_calls.set(callback_calls.get() + 1),
        ) {
            super::BackTargetPermanentBeginCoreOutcome::Permanent {
                owner,
                installation,
                evidence:
                    super::BackTargetPermanentBeginCorePermanentEvidence::TerminalPlanContradiction {
                        source,
                        expected,
                        observed,
                    },
            } => {
                let (owner, _lease) = owner.into_parts();
                assert_eq!(owner.scope, expected_scope);
                assert_eq!(owner.expected_current, expected_current);
                assert_eq!(owner.expected_request, expected_request);
                assert_eq!(owner.logical_roots, expected_roots);
                assert_eq!(installation, "trace-only-installation");
                assert!(source.current_request.trace_closed);
                assert!(matches!(
                    expected,
                    super::CoreBackTargetPermanentTerminalExpectedProjection::Request {
                        operation: super::CoreBackTargetPermanentTerminalOperation::RecoveringTrace,
                        request,
                    } if request.request == scope.request() && !request.trace_closed
                ));
                assert!(matches!(
                    observed,
                    super::CoreBackTargetPermanentTerminalObservedProjection::Request {
                        operation: super::CoreBackTargetPermanentTerminalOperation::RecoveringTrace,
                        request: Some(request),
                        failure: super::CoreBackTargetPermanentTerminalFailureProjection::RequestRejected(
                            RequestRecordError::TraceClosed,
                        ),
                    } if request.request == scope.request() && request.trace_closed
                ));
                source
            }
            super::BackTargetPermanentBeginCoreOutcome::Permanent { .. } => {
                panic!("trace-only drift must retain purpose-specific terminal evidence")
            }
            _ => panic!("the closed real trace must fail before physical installation"),
        };
        assert_eq!(callback_calls.get(), 0);
        assert_eq!(engine.snapshot(), before);
        assert!(source.current_request.trace_closed);

        fn item_between<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
            let tail = &source[source.find(start).expect("contract item must remain named")..];
            &tail[..tail.find(end).expect("contract boundary must remain named")]
        }
        let engine_source = include_str!("../../bevy_ui_router_core/src/engine.rs");
        let evidence = item_between(
            engine_source,
            "pub(crate) enum BackTargetPermanentBeginCorePermanentEvidence",
            "pub(crate) struct CoreBackTargetPermanentCurrentStateProjection",
        );
        let request_contradiction = item_between(
            evidence,
            "RequestRecordContradiction",
            "TerminalPlanContradiction",
        );
        let compact_request_contradiction = request_contradiction
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect::<String>();
        let request_projection = item_between(
            engine_source,
            "pub(crate) struct CoreBackTargetPermanentRequestProjection",
            "pub(crate) struct CoreBackTargetPermanentAuthorityProjection",
        );
        let begin = item_between(
            engine_source,
            "pub(crate) fn begin_back_target_permanent_cleanup",
            "pub(crate) fn complete_back_no_target_authority_cleanup",
        );
        let request_gate = item_between(
            begin,
            "let exact_request_record",
            "if !exact_request_record",
        );
        assert!(
            compact_request_contradiction
                .contains("expected:Option<CoreBackTargetPermanentRequestProjection>")
                && compact_request_contradiction
                    .contains("observed:Option<CoreBackTargetPermanentRequestProjection>")
                && !compact_request_contradiction.contains("expected_status:")
                && !compact_request_contradiction.contains("observed_status:"),
            "generic request contradiction must retain presence-aware full expected/observed records"
        );
        assert!(
            request_projection.contains("trace_closed: bool"),
            "the full record projection must retain trace closure"
        );
        assert!(
            !request_gate.contains("trace_closed"),
            "trace-only drift must remain a purpose-specific terminal planning failure"
        );
    }

    #[test]
    fn back_target_permanent_missing_current_record_returns_lossless_typed_evidence() {
        let (mut engine, scope, ticket, logical_roots, mut issuer, lease) =
            back_target_permanent_begin_fixture();
        let settlement = engine
            .prepare_back_target_permanent_settlement_owner(scope.clone(), logical_roots.clone());
        let expected_scope = settlement.scope.clone();
        let expected_current = settlement.expected_current.clone();
        let expected_request = settlement.expected_request.clone();
        let expected_roots = settlement.logical_roots.clone();
        engine
            .records
            .retain(|record| record.id() != scope.request());
        let before = engine.snapshot();
        let callback_calls = std::cell::Cell::new(0usize);

        let evidence = match engine.begin_back_target_permanent_cleanup(
            &mut issuer,
            CoreBackTargetPermanentBeginOwner::new(settlement, lease),
            &ticket,
            logical_roots,
            0x4d49_5353_494e_475f,
            NONZERO_OBLIGATION_COUNT,
            "missing-current-installation",
            |_, _| callback_calls.set(callback_calls.get() + 1),
        ) {
            super::BackTargetPermanentBeginCoreOutcome::Permanent {
                owner,
                installation,
                evidence,
            } => {
                let (owner, _lease) = owner.into_parts();
                assert_eq!(owner.scope, expected_scope);
                assert_eq!(owner.expected_current, expected_current);
                assert_eq!(owner.expected_request, expected_request);
                assert_eq!(owner.logical_roots, expected_roots);
                assert_eq!(installation, "missing-current-installation");
                assert!(matches!(
                    evidence,
                    super::BackTargetPermanentBeginCorePermanentEvidence::RequestRecordContradiction {
                        ..
                    }
                ));
                evidence
            }
            _ => panic!("a missing frozen current record must return typed contradiction evidence"),
        };
        assert_eq!(callback_calls.get(), 0);
        assert_eq!(engine.snapshot(), before);
        let evidence = format!("{evidence:?}");
        assert!(
            evidence.contains("expected: Some(")
                && evidence.contains("trace_closed: false")
                && evidence.contains("observed: None"),
            "missing record evidence must not synthesize an observed status: {evidence}"
        );
    }

    #[test]
    fn back_target_permanent_terminal_source_retains_missing_queued_record_expectation() {
        let (mut engine, scope, ticket, logical_roots, mut issuer, lease) =
            back_target_permanent_begin_fixture();
        let queued_location = RouteLocation::new("/items/queued-after-back");
        let queued_request = engine
            .push(queued_location.clone())
            .expect("the ReadyToCommit fixture queues a later request");
        assert_eq!(engine.queue.len(), 1);
        let settlement =
            engine.prepare_back_target_permanent_settlement_owner(scope, logical_roots.clone());
        let expected_scope = settlement.scope.clone();
        let expected_current = settlement.expected_current.clone();
        let expected_request = settlement.expected_request.clone();
        let expected_roots = settlement.logical_roots.clone();
        engine
            .records
            .retain(|record| record.id() != queued_request);
        let before = engine.snapshot();
        let callback_calls = std::cell::Cell::new(0usize);

        let source = match engine.begin_back_target_permanent_cleanup(
            &mut issuer,
            CoreBackTargetPermanentBeginOwner::new(settlement, lease),
            &ticket,
            logical_roots,
            0x5155_4555_455f_4f4e,
            NONZERO_OBLIGATION_COUNT,
            "missing-queued-installation",
            |_, _| callback_calls.set(callback_calls.get() + 1),
        ) {
            super::BackTargetPermanentBeginCoreOutcome::Permanent {
                owner,
                installation,
                evidence:
                    super::BackTargetPermanentBeginCorePermanentEvidence::TerminalPlanContradiction {
                        source,
                        expected,
                        observed,
                    },
            } => {
                let (owner, _lease) = owner.into_parts();
                assert_eq!(owner.scope, expected_scope);
                assert_eq!(owner.expected_current, expected_current);
                assert_eq!(owner.expected_request, expected_request);
                assert_eq!(owner.logical_roots, expected_roots);
                assert_eq!(installation, "missing-queued-installation");
                assert!(matches!(
                    expected,
                    super::CoreBackTargetPermanentTerminalExpectedProjection::Request {
                        operation: super::CoreBackTargetPermanentTerminalOperation::QueuedTerminalTransition,
                        request,
                    } if request.request == queued_request
                        && request.status == RequestStatus::Queued
                        && request.requested.as_ref() == Some(&queued_location)
                        && !request.trace_closed
                ));
                assert!(matches!(
                    observed,
                    super::CoreBackTargetPermanentTerminalObservedProjection::Request {
                        operation: super::CoreBackTargetPermanentTerminalOperation::QueuedTerminalTransition,
                        request: None,
                        failure: super::CoreBackTargetPermanentTerminalFailureProjection::MissingRequest,
                    }
                ));
                source
            }
            _ => panic!("the missing queued record must return its typed terminal contradiction"),
        };
        assert_eq!(callback_calls.get(), 0);
        assert_eq!(engine.snapshot(), before);
        assert_eq!(
            source.queue.len(),
            1,
            "terminal source must retain every queued command even when its record is absent"
        );
        let source = format!("{source:?}");
        assert!(
            source.contains("expected:")
                && source.contains(&format!("request: {queued_request:?}"))
                && source.contains("observed: None"),
            "the queued source item must pair command expectation with observed record presence: {source}"
        );

        let engine_source = include_str!("../../bevy_ui_router_core/src/engine.rs");
        let source_start = engine_source
            .find("let source = CoreBackTargetPermanentTerminalSourceProjection")
            .expect("the terminal source freeze must remain named");
        let source_tail = &engine_source[source_start..];
        let source_freeze = &source_tail[..source_tail
            .find("let failure = NavigationFailure::new")
            .expect("the terminal source freeze boundary must remain named")];
        assert!(
            !source_freeze.contains(".filter_map("),
            "terminal source freeze must not discard queued commands with missing records"
        );
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
        engine.publish_prepared(prepared, mutation)
    }

    #[test]
    fn back_resolution_action_blocks_next_collect() {
        let mut engine = ready_engine();
        engine.enqueue_push(RouteLocation::new("/")).unwrap();
        engine.collect();
        settle_current(&mut engine);
        engine.enqueue_push(RouteLocation::new("/about")).unwrap();
        engine.collect();
        settle_current(&mut engine);

        let world = engine
            .claim_history_admission_world()
            .expect("the confirmed engine issues one real history world seal");
        let stamp = engine
            .history_admission_stamp(&world)
            .expect("the published history has a real back entry");
        let first_admission = engine
            .prepare_back_admission(&world, &stamp)
            .expect("the first back admission exact-refreezes the live history");
        let second_admission = engine
            .prepare_back_admission(&world, &stamp)
            .expect("the second back admission exact-refreezes the same live history");
        let history_authority_before = engine.testing_history_authority_snapshot();
        let history_world_before = engine.history_admission_world;
        let active_before = engine.active().clone();
        let state_before = engine.state.snapshot();
        let first_accepted = engine
            .enqueue_back(first_admission)
            .expect("the first back reserves a real branch owner");
        let second_accepted = engine
            .enqueue_back(second_admission)
            .expect("the second back reserves a distinct real branch owner");
        let first_request = first_accepted.request();
        let second_request = second_accepted.request();
        assert_ne!(first_request, second_request);

        let queued_back_projection = |queued: &QueuedNavigation| {
            let QueuedReservation::Branch(reservation) = &queued.reservation else {
                panic!("accepted Back must retain its complete branch reservation")
            };
            (
                queued.request,
                matches!(&queued.command, NavigationCommand::Back),
                queued.outcome.is_none(),
                reservation.request(),
                reservation.transaction(),
                reservation.cleanup_batch(),
                reservation
                    .frames()
                    .iter()
                    .map(|frame| (frame.frame_task(), frame.frame()))
                    .collect::<Vec<_>>(),
            )
        };
        let second_queued_before_collect = queued_back_projection(
            engine
                .queue
                .get(1)
                .expect("the second accepted Back remains queued behind the first"),
        );
        assert_eq!(second_queued_before_collect.0, second_request);
        assert!(second_queued_before_collect.1);
        assert!(second_queued_before_collect.2);
        assert_eq!(engine.queue.len(), 2);
        for (queued, request) in engine.queue.iter().zip([first_request, second_request]) {
            assert_eq!(queued.request, request);
            assert!(matches!(&queued.command, NavigationCommand::Back));
            assert!(queued.outcome.is_none());
            let QueuedReservation::Branch(reservation) = &queued.reservation else {
                panic!("each accepted Back must retain its complete branch reservation")
            };
            assert_eq!(reservation.request(), request);
            assert_eq!(
                reservation.frames().len(),
                engine.matcher.max_branch_depth(),
                "each accepted Back reserves the installed maximum depth before resolution"
            );
        }
        for request in [first_request, second_request] {
            let record = engine
                .request(request)
                .expect("each accepted Back retains its queued request record");
            assert_eq!(record.status(), &RequestStatus::Queued);
            assert_eq!(record.requested(), None);
            assert_eq!(record.matched(), None);
            assert!(record.trace().is_empty());
            assert!(record.diagnostics().is_empty());
        }
        assert!(engine.current.is_none());
        assert!(engine.pending_no_target_back.is_none());
        assert!(engine.action.is_none());
        assert!(engine.current_history_capability.is_none());
        assert!(engine.pending_history_preparation_error.is_none());
        assert!(engine.prepared_publication_authority.is_none());
        assert_eq!(
            engine.testing_history_authority_snapshot(),
            history_authority_before
        );
        assert_eq!(engine.history_admission_world, history_world_before);
        assert_eq!(engine.active(), &active_before);
        assert_eq!(engine.state.snapshot(), state_before);

        engine.collect();
        let queue_len_with_outstanding_action = engine.queue.len();
        let first_action_evidence = match engine.action.as_ref() {
            Some(EngineAction::BackResolution(handoff)) => handoff.evidence(),
            _ => panic!("the first accepted back must leave one raw resolution action outstanding"),
        };
        assert_eq!(queue_len_with_outstanding_action, 1);

        engine.collect();

        assert_eq!(first_accepted.request(), first_request);
        assert_eq!(second_accepted.request(), second_request);
        let observed_action_evidence = match engine.action.as_ref() {
            Some(EngineAction::BackResolution(handoff)) => handoff.evidence(),
            _ => panic!("the original raw resolution action must remain outstanding"),
        };
        assert_eq!(
            observed_action_evidence, first_action_evidence,
            "an outstanding BackResolution action must not be overwritten by collect"
        );
        assert_eq!(
            engine.queue.len(),
            queue_len_with_outstanding_action,
            "an outstanding BackResolution action must block collect from consuming the queued back"
        );
        assert_eq!(
            queued_back_projection(
                engine
                    .queue
                    .front()
                    .expect("the second accepted Back must remain at the front of the queue"),
            ),
            second_queued_before_collect,
            "the blocked second Back must retain its exact non-authorizing queue projection"
        );
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
        assert_eq!(
            engine.testing_history_authority_snapshot(),
            before_stale.history
        );
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
    fn replace_match_failure_and_fallback_failure_preserve_published_history() {
        let mut engine = ready_engine();
        engine.push(RouteLocation::new("/items/1")).unwrap();
        settle_current(&mut engine);
        let active_before = engine.active().clone();
        let history_before = engine.testing_history_authority_snapshot();

        let request = engine.replace(RouteLocation::new("/missing")).unwrap();
        assert!(!engine.testing_history_capability_snapshot().has_owner());
        let EngineAction::FrameTask(fallback) = engine.action().unwrap() else {
            panic!("replace match failure starts a fresh fallback attempt")
        };
        assert_eq!(fallback.identity().origin(), &FrameOrigin::RouterFallback);
        engine
            .begin_view_report(fallback.identity())
            .unwrap()
            .failed(NavigationFailure::new(
                NavigationFailureKind::Build,
                "replace match fallback failed",
            ));

        assert!(matches!(
            engine.request(request).unwrap().status(),
            RequestStatus::Failed(failure)
                if failure.kind() == NavigationFailureKind::RecoveryFailed
        ));
        assert_eq!(engine.active(), &active_before);
        assert_eq!(engine.testing_history_authority_snapshot(), history_before);
        assert!(!engine.testing_history_capability_snapshot().has_owner());
    }

    #[test]
    fn replace_mount_failure_after_capability_issue_preserves_history_through_fallback_failure() {
        let mut engine = ready_engine();
        engine.push(RouteLocation::new("/")).unwrap();
        settle_current(&mut engine);
        let active_before = engine.active().clone();
        let history_before = engine.testing_history_authority_snapshot();

        let request = engine.replace(RouteLocation::new("/items/1")).unwrap();
        assert!(
            engine
                .testing_history_capability_snapshot()
                .has_exactly_one_owner()
        );
        let EngineAction::MountRequest(mount) = engine.action().unwrap() else {
            panic!("the preserved root must request the entering child mount")
        };
        assert!(
            engine
                .testing_history_capability_snapshot()
                .has_exactly_one_owner()
        );
        let failure = engine
            .reject_mount(mount, crate::trace::MountRejectionKind::MissingOutlet)
            .unwrap();
        assert!(!engine.testing_history_capability_snapshot().has_owner());
        let roots = failure.logical_roots().to_vec();
        let count = usize::from(!roots.is_empty());
        let cleanup = engine
            .prepare_failure_cleanup(
                failure,
                CleanupManifestSeal::new(roots, manifest_digest(0xA0F1), count),
            )
            .unwrap();
        engine.complete_cleanup(valid_completion(&cleanup)).unwrap();

        let EngineAction::FrameTask(fallback) = engine.action().unwrap() else {
            panic!("replace mount failure continues through a fresh fallback")
        };
        assert_eq!(fallback.identity().origin(), &FrameOrigin::RouterFallback);
        engine
            .begin_view_report(fallback.identity())
            .unwrap()
            .failed(NavigationFailure::new(
                NavigationFailureKind::Build,
                "replace mount fallback failed",
            ));

        assert!(matches!(
            engine.request(request).unwrap().status(),
            RequestStatus::Failed(failure)
                if failure.kind() == NavigationFailureKind::RecoveryFailed
        ));
        assert_eq!(engine.active(), &active_before);
        assert_eq!(engine.testing_history_authority_snapshot(), history_before);
        assert!(!engine.testing_history_capability_snapshot().has_owner());
    }

    #[test]
    fn fallback_failure_preserves_old_presentation_and_history_then_reports_recovery_failed() {
        let mut engine = ready_engine();
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
            duplicate_engine.testing_history_authority_snapshot(),
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
                duplicate_engine.testing_history_authority_snapshot(),
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
            stale_engine.testing_history_authority_snapshot(),
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
                stale_engine.testing_history_authority_snapshot(),
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
            invalid_engine.testing_history_authority_snapshot(),
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
                invalid_engine.testing_history_authority_snapshot(),
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
        assert_eq!(engine.issued_view_tasks.len(), TERMINAL_RETENTION);
        assert_eq!(engine.completed_cleanup.len(), TERMINAL_RETENTION);
        assert_eq!(
            engine.report_dispositions.len(),
            TERMINAL_RETENTION + ROUTER_DIAGNOSTIC_RETENTION
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
            engine.request(request).unwrap().requested().unwrap().path(),
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
            assert_eq!(engine.testing_history_authority_snapshot(), before.history);
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
        let first_root = engine.active().frames()[0].clone();
        let first_leaf = engine.active().frames()[1].clone();

        engine.push(RouteLocation::new("items//1/")).unwrap();
        let equivalent = engine.action().unwrap();
        assert!(matches!(equivalent, EngineAction::Commit(_)));
        let equivalent_cleanup = publish_action(&mut engine, equivalent);
        assert_eq!(equivalent_cleanup.obligation_count(), 0);
        engine
            .complete_cleanup(valid_completion(&equivalent_cleanup))
            .unwrap();
        assert_eq!(engine.active().frames()[0], first_root);

        engine
            .push(RouteLocation::new("/items/1").with_fragment("details"))
            .unwrap();
        let EngineAction::FrameTask(fragment) = engine.action().unwrap() else {
            panic!("fragment change must issue one task");
        };
        assert_eq!(fragment.identity().frame(), first_leaf.frame());
        assert_eq!(
            fragment.identity().generation(),
            first_leaf.generation().next()
        );
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
        assert_ne!(changed.identity().frame(), first_leaf.frame());
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
        assert_eq!(projection.preserved()[0].version(), &previous[0]);
        assert_eq!(projection.preserved()[1].version(), &previous[1]);
        assert_ne!(newly_published[2], previous[2]);
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
        engine.push(RouteLocation::new("/items")).unwrap();
        settle_current(&mut engine);
        let exact = pending_reconciliation_mount_completion(&mut engine);
        let active_child_slots = engine.active_child_slots.clone();
        engine.active_child_slots.clear();

        let mut foreign = ready_engine_with_tree(reconciliation_tree());
        foreign.push(RouteLocation::new("/items")).unwrap();
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
        engine.push(RouteLocation::new("/items")).unwrap();
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
    fn back_no_target_source_drift_prepared_terminal_evicts_exact_oldest_retention_state() {
        let mut engine = ready_engine();
        engine.push(RouteLocation::new("/")).unwrap();
        settle_current(&mut engine);
        engine.push(RouteLocation::new("/about")).unwrap();
        settle_current(&mut engine);
        for index in 0..(TERMINAL_RETENTION - 2) {
            engine
                .replace(RouteLocation::new(format!("/items/{index}")))
                .unwrap();
            settle_current(&mut engine);
        }
        assert_eq!(
            engine
                .records
                .iter()
                .filter(|record| {
                    matches!(
                        record.status(),
                        RequestStatus::Succeeded | RequestStatus::Failed(_)
                    )
                })
                .count(),
            TERMINAL_RETENTION,
            "real push/replace cleanup must fill retention before Back resolution"
        );
        assert_eq!(engine.history().len(), 2);

        let world = engine
            .claim_history_admission_world()
            .expect("the confirmed engine issues one real history world seal");
        let stamp = engine
            .history_admission_stamp(&world)
            .expect("the two-entry history admits Back");
        let first_admission = engine
            .prepare_back_admission(&world, &stamp)
            .expect("the first Back exact-refreezes the live history");
        let second_admission = engine
            .prepare_back_admission(&world, &stamp)
            .expect("the second Back shares the same pre-publication source");
        let first_accepted = engine.enqueue_back(first_admission).unwrap();
        let second_accepted = engine.enqueue_back(second_admission).unwrap();
        let source_drift_request = second_accepted.request();

        engine.collect();
        let first_handoff = match engine.action().unwrap() {
            EngineAction::BackResolution(handoff) => handoff,
            _ => panic!("the first accepted Back must produce a real resolution handoff"),
        };
        let (_first_request, first_view) = first_accepted.into_parts();
        let first_verified = match engine.verify_back_resolution_handoff(first_view, first_handoff)
        {
            BackResolutionVerifyOutcome::Matched(verified) => verified,
            BackResolutionVerifyOutcome::Mismatch { .. } => {
                panic!("the exact first Back lineage must verify")
            }
        };
        let (_first_view, first_owner) = first_verified.into_parts();
        let target = match first_owner {
            VerifiedBackResolutionOwnerHandoff::Target(owner) => owner,
            VerifiedBackResolutionOwnerHandoff::NoTarget(_)
            | VerifiedBackResolutionOwnerHandoff::Contradiction(_) => {
                panic!("the first Back from a two-entry history must resolve its predecessor")
            }
        };
        let start = match engine.resume_back_target_with_retirement(target, || {}) {
            BackTargetResumeOutcome::Resumed(start) => start,
            BackTargetResumeOutcome::Permanent(_) | BackTargetResumeOutcome::Mismatch(_) => {
                panic!("the exact first Back target must resume")
            }
        };
        let resume = match start {
            BackTargetStart::Ready(resume) => resume,
            BackTargetStart::FailureCleanup(_) => {
                panic!("the real predecessor must reach a normal target action")
            }
        };
        let target_action = match resume.action {
            BackTargetResumeAction::FrameTask(task) => EngineAction::FrameTask(task),
            BackTargetResumeAction::MountRequest(request) => EngineAction::MountRequest(request),
            BackTargetResumeAction::Commit(ticket) => EngineAction::Commit(ticket),
        };
        let first_cleanup = publish_action(&mut engine, target_action);
        engine
            .complete_cleanup(valid_completion(&first_cleanup))
            .unwrap();
        assert_eq!(engine.history_snapshot().cursor(), Some(0));

        let second_handoff = match engine.action().unwrap() {
            EngineAction::BackResolution(handoff) => handoff,
            _ => panic!("the queued second Back must resolve after the first terminal"),
        };
        let (_second_request, second_view) = second_accepted.into_parts();
        let second_verified =
            match engine.verify_back_resolution_handoff(second_view, second_handoff) {
                BackResolutionVerifyOutcome::Matched(verified) => verified,
                BackResolutionVerifyOutcome::Mismatch { .. } => {
                    panic!("the exact second Back lineage must verify")
                }
            };
        let (_second_view, second_owner) = second_verified.into_parts();
        let candidate = match second_owner {
            VerifiedBackResolutionOwnerHandoff::NoTarget(candidate) => candidate,
            VerifiedBackResolutionOwnerHandoff::Target(_)
            | VerifiedBackResolutionOwnerHandoff::Contradiction(_) => {
                panic!("the second live Back at cursor zero must resolve NoTarget")
            }
        };
        let source_resolution = candidate.evidence();

        engine
            .replace(RouteLocation::new("/items/drifted"))
            .unwrap();
        settle_current(&mut engine);
        let transfer_calls = std::cell::Cell::new(0usize);
        let drift = match engine.refreeze_back_no_target_source_with_transfer(candidate, || {
            transfer_calls.set(transfer_calls.get() + 1);
        }) {
            BackNoTargetSourceRefreezeOutcome::Drift(owner) => owner,
            BackNoTargetSourceRefreezeOutcome::Unchanged(_)
            | BackNoTargetSourceRefreezeOutcome::Mismatch { .. } => {
                panic!("the real replacement must produce an exact source-drift owner")
            }
        };
        assert_eq!(transfer_calls.get(), 1);

        let mut retained_requests = engine
            .records
            .iter()
            .filter(|record| {
                matches!(
                    record.status(),
                    RequestStatus::Succeeded | RequestStatus::Failed(_)
                )
            })
            .map(|record| record.id())
            .collect::<Vec<_>>();
        retained_requests.sort_by_key(|request| request.sequence());
        assert_eq!(
            retained_requests.len(),
            TERMINAL_RETENTION,
            "the drift settlement must begin with a full terminal retention window"
        );
        let evicted_request = retained_requests[0];
        let expected_earliest_after = retained_requests[1].sequence();
        let watermark_before = engine.testing_evicted_watermark();
        assert!(
            engine
                .issued_view_tasks
                .iter()
                .any(|identity| identity.request == evicted_request),
            "the oldest real terminal must retain issued task inventory before eviction"
        );
        assert!(
            engine
                .completed_cleanup
                .iter()
                .any(|tombstone| tombstone.request == evicted_request),
            "the oldest real successful terminal must retain cleanup inventory before eviction"
        );

        let retirement_calls = std::cell::Cell::new(0usize);
        let _terminal =
            match engine.settle_back_no_target_source_drift_with_retirement(drift, || {
                retirement_calls.set(retirement_calls.get() + 1);
            }) {
                BackNoTargetSourceDriftSettlementOutcome::Settled(terminal) => terminal,
                BackNoTargetSourceDriftSettlementOutcome::Mismatch { .. } => {
                    panic!("the exact observed drift must terminal under retention pressure")
                }
            };
        assert_eq!(retirement_calls.get(), 1);
        assert!(matches!(
            engine.private_lookup(evicted_request),
            PrivateRequestLookup::Evicted
        ));
        assert_eq!(
            engine.testing_evicted_watermark(),
            Some(evicted_request.sequence()),
            "prepared retention must advance through the exact oldest retained request"
        );
        assert!(
            watermark_before.is_none_or(|watermark| watermark < evicted_request.sequence()),
            "the prepared eviction must strictly advance the existing watermark"
        );
        assert_eq!(
            engine.earliest_retained_sequence(),
            Some(expected_earliest_after)
        );
        assert!(
            engine
                .issued_view_tasks
                .iter()
                .all(|identity| identity.request != evicted_request)
        );
        assert!(
            engine
                .report_dispositions
                .iter()
                .all(|record| record.target != super::DiagnosticTarget::Request(evicted_request))
        );
        assert!(
            engine
                .completed_cleanup
                .iter()
                .all(|tombstone| tombstone.request != evicted_request)
        );
        assert!(engine.completed_cleanup.len() <= TERMINAL_RETENTION);
        assert!(
            engine.report_dispositions.len() <= TERMINAL_RETENTION + ROUTER_DIAGNOSTIC_RETENTION
        );
        let drift_record = engine
            .request(source_drift_request)
            .expect("the newest source-drift request remains retained");
        assert!(matches!(
            drift_record.status(),
            RequestStatus::Failed(failure)
                if failure.kind() == NavigationFailureKind::AuthorityLost
        ));
        assert_eq!(
            drift_record
                .trace()
                .iter()
                .filter(|event| event.kind() == NavigationTraceEventKind::RequestTerminal)
                .count(),
            1
        );
        assert_eq!(
            engine.classify_back_resolution_evidence(&source_resolution),
            BackResolutionEvidenceClassification::ConsumedReplay
        );
    }

    #[test]
    fn back_target_permanent_prepared_terminal_evicts_exact_oldest_retention_state() {
        let mut engine = ready_engine();
        engine.push(RouteLocation::new("/")).unwrap();
        settle_current(&mut engine);
        engine.push(RouteLocation::new("/about")).unwrap();
        settle_current(&mut engine);
        for index in 0..(TERMINAL_RETENTION - 2) {
            engine
                .replace(RouteLocation::new(format!("/items/{index}")))
                .unwrap();
            settle_current(&mut engine);
        }

        let mut retained_terminals = engine
            .records
            .iter()
            .filter(|record| {
                matches!(
                    record.status(),
                    RequestStatus::Succeeded | RequestStatus::Failed(_)
                )
            })
            .map(|record| record.id())
            .collect::<Vec<_>>();
        retained_terminals.sort_by_key(|request| request.sequence());
        assert_eq!(retained_terminals.len(), TERMINAL_RETENTION);
        assert_eq!(engine.history().len(), 2);
        let evicted_request = retained_terminals[0];
        let next_earliest_request = retained_terminals[1];
        assert!(
            engine
                .issued_view_tasks
                .iter()
                .any(|identity| identity.request == evicted_request),
            "the oldest real terminal must own issued task inventory before eviction"
        );
        assert!(
            engine.report_dispositions.iter().any(|record| {
                record.target == super::DiagnosticTarget::Request(evicted_request)
            }),
            "the oldest real terminal must own report inventory before eviction"
        );
        assert!(
            engine
                .completed_cleanup
                .iter()
                .any(|tombstone| tombstone.request == evicted_request),
            "the oldest real terminal must own cleanup inventory before eviction"
        );

        let world = engine
            .claim_history_admission_world()
            .expect("the confirmed engine issues one real history world seal");
        let stamp = engine
            .history_admission_stamp(&world)
            .expect("the two-entry history admits a target Back");
        let admission = engine
            .prepare_back_admission(&world, &stamp)
            .expect("the target Back exact-refreezes the live history");
        let accepted = engine.enqueue_back(admission).unwrap();
        let permanent_request = accepted.request();
        engine.collect();
        let handoff = match engine.action().unwrap() {
            EngineAction::BackResolution(handoff) => handoff,
            _ => panic!("the accepted Back must produce a real resolution handoff"),
        };
        let (_request, view) = accepted.into_parts();
        let verified = match engine.verify_back_resolution_handoff(view, handoff) {
            BackResolutionVerifyOutcome::Matched(verified) => verified,
            BackResolutionVerifyOutcome::Mismatch { .. } => {
                panic!("the exact Back lineage must verify")
            }
        };
        let (_view, owner) = verified.into_parts();
        let target = match owner {
            VerifiedBackResolutionOwnerHandoff::Target(owner) => owner,
            VerifiedBackResolutionOwnerHandoff::NoTarget(_)
            | VerifiedBackResolutionOwnerHandoff::Contradiction(_) => {
                panic!("the two-entry history must resolve a real target")
            }
        };
        engine.testing_request_back_target_source_drift_before_freeze_once();
        let retirement_calls = std::cell::Cell::new(0usize);
        let permanent = match engine.resume_back_target_with_retirement(target, || {
            retirement_calls.set(retirement_calls.get() + 1);
        }) {
            BackTargetResumeOutcome::Permanent(owner) => owner,
            BackTargetResumeOutcome::Resumed(_) => {
                panic!("the real post-refreeze source drift must form a permanent owner")
            }
            BackTargetResumeOutcome::Mismatch(_) => {
                panic!("the exact target receiver must not reject its real owner")
            }
        };
        assert_eq!(retirement_calls.get(), 1);
        let owner_snapshot = permanent.testing_evidence_snapshot();
        assert_eq!(
            owner_snapshot.cause(),
            BackTargetPermanentCauseSnapshot::SourceCapability
        );
        assert!(matches!(
            engine.request(permanent_request).unwrap().status(),
            RequestStatus::Queued
        ));
        assert_eq!(
            engine
                .records
                .iter()
                .filter(|record| {
                    matches!(
                        record.status(),
                        RequestStatus::Succeeded | RequestStatus::Failed(_)
                    )
                })
                .count(),
            TERMINAL_RETENTION,
            "the permanent target must remain queued beside a full terminal window"
        );
        let watermark_before = engine.testing_evicted_watermark();

        let terminal = match engine.settle_back_target_permanent_exact(permanent) {
            BackTargetPermanentSettlementOutcome::Settled(terminal) => terminal,
            BackTargetPermanentSettlementOutcome::Mismatch { .. } => {
                panic!("the exact permanent target owner must settle under retention pressure")
            }
        };

        assert_eq!(terminal.testing_evidence_snapshot(), owner_snapshot);
        assert!(matches!(
            engine.private_lookup(evicted_request),
            PrivateRequestLookup::Evicted
        ));
        assert!(matches!(
            engine.private_lookup(next_earliest_request),
            PrivateRequestLookup::Retained(_)
        ));
        assert_eq!(
            engine.testing_evicted_watermark(),
            Some(evicted_request.sequence())
        );
        assert!(watermark_before.is_none_or(|watermark| watermark < evicted_request.sequence()));
        assert_eq!(
            engine.earliest_retained_sequence(),
            Some(next_earliest_request.sequence())
        );
        assert!(
            engine
                .issued_view_tasks
                .iter()
                .all(|identity| identity.request != evicted_request)
        );
        assert!(
            engine.report_dispositions.iter().all(|record| {
                record.target != super::DiagnosticTarget::Request(evicted_request)
            })
        );
        assert!(
            engine
                .completed_cleanup
                .iter()
                .all(|tombstone| tombstone.request != evicted_request)
        );
        let permanent_record = engine
            .request(permanent_request)
            .expect("the new permanent target request remains retained");
        assert!(matches!(
            permanent_record.status(),
            RequestStatus::Failed(failure)
                if failure.kind() == NavigationFailureKind::AuthorityLost
        ));
        assert_eq!(
            permanent_record
                .trace()
                .iter()
                .filter(|event| event.kind() == NavigationTraceEventKind::RequestTerminal)
                .count(),
            1
        );
        assert_eq!(
            engine
                .records
                .iter()
                .filter(|record| {
                    matches!(
                        record.status(),
                        RequestStatus::Succeeded | RequestStatus::Failed(_)
                    )
                })
                .count(),
            TERMINAL_RETENTION
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
            &RequestStatus::Running(NavigationPhase::ApplyingFrame(1))
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
        assert_eq!(engine.current.as_ref().unwrap().cleanup_roots, vec![child]);

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
    fn logical_roots_and_ecs_obligation_count_seal_independently() {
        fn ready_to_seal() -> (RouterEngine, super::CommitTicket) {
            let mut engine = ready_engine();
            engine.push(RouteLocation::new("/items/1")).unwrap();
            let EngineAction::FrameTask(task) = engine.action().unwrap() else {
                panic!("expected frame task");
            };
            let ticket = drive_to_commit(&mut engine, EngineAction::FrameTask(task));
            (engine, ticket)
        }

        let (mut empty_roots, empty_ticket) = ready_to_seal();
        let empty_digest = manifest_digest(5);
        let empty_expected = CleanupBatchToken::sealed(
            empty_ticket.identity().clone(),
            empty_roots.current.as_ref().unwrap().cleanup_batch,
            &[],
            empty_digest,
            1,
        );
        let empty_sealed = empty_roots
            .seal_commit(
                &empty_ticket,
                CleanupManifestSeal::new(Vec::new(), empty_digest, 1),
            )
            .expect("logical roots do not infer the ECS entity obligation count");
        assert_eq!(empty_sealed.cleanup(), &empty_expected);

        let (mut nonempty_roots, nonempty_ticket) = ready_to_seal();
        let root = nonempty_roots
            .current
            .as_ref()
            .unwrap()
            .target_frames
            .last()
            .unwrap()
            .clone();
        nonempty_roots.current.as_mut().unwrap().cleanup_roots = vec![root.clone()];
        let nonempty_digest = manifest_digest(6);
        let nonempty_expected = CleanupBatchToken::sealed(
            nonempty_ticket.identity().clone(),
            nonempty_roots.current.as_ref().unwrap().cleanup_batch,
            std::slice::from_ref(&root),
            nonempty_digest,
            0,
        );
        let nonempty_sealed = nonempty_roots
            .seal_commit(
                &nonempty_ticket,
                CleanupManifestSeal::new(vec![root], nonempty_digest, 0),
            )
            .expect("ECS can prove zero entity obligations for nonempty logical roots");
        assert_eq!(nonempty_sealed.cleanup(), &nonempty_expected);
    }

    #[test]
    fn coherent_foreign_evidence_is_rejected_by_real_prepare_and_consumed_replay_is_zero_write() {
        let mut foreign_probe = ready_engine();
        foreign_probe.push(RouteLocation::new("/items/1")).unwrap();
        let EngineAction::FrameTask(task) = foreign_probe.action().unwrap() else {
            panic!("expected frame task");
        };
        let ticket = drive_to_commit(&mut foreign_probe, EngineAction::FrameTask(task));
        let sealed = seal_current(&mut foreign_probe, ticket);
        let foreign_identity = distinct_identity();
        let foreign = sealed.evidence().testing_with_observed_router_and_request(
            foreign_identity.router_identity(),
            foreign_identity.request(),
        );
        let before = foreign_probe.snapshot();
        let capability_before = foreign_probe.testing_history_capability_snapshot();
        assert_eq!(
            foreign_probe.probe_sealed_commit_evidence(&foreign),
            Err(CommitError::Stale)
        );
        assert_eq!(foreign_probe.snapshot(), before);
        assert_eq!(
            foreign_probe.testing_history_capability_snapshot(),
            capability_before
        );
        let prepared = foreign_probe
            .prepare_publication(sealed)
            .expect("foreign projection probing cannot consume the real sealed owner");
        foreign_probe.publish_prepared(prepared, || {}).unwrap();

        let mut engine = ready_engine();
        engine.push(RouteLocation::new("/items/1")).unwrap();
        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("expected frame task");
        };
        let ticket = drive_to_commit(&mut engine, EngineAction::FrameTask(task));
        let sealed = seal_current(&mut engine, ticket);
        let prepared = engine.prepare_publication(sealed).unwrap();
        let consumed = engine
            .publish_prepared_with_evidence(prepared, || {})
            .unwrap();
        let before = engine.snapshot();
        assert_eq!(
            engine.probe_consumed_publication_evidence(&consumed),
            Err(CommitError::Stale),
        );
        assert_eq!(engine.snapshot(), before);
    }

    #[test]
    fn cross_incarnation_history_replay_is_zero_write() {
        fn stage_equal_history_owner(
            engine: &mut RouterEngine,
        ) -> (super::NavigationRequestId, super::SealedCommit) {
            engine.push(RouteLocation::new("/items/1")).unwrap();
            settle_current(engine);
            let request = engine
                .replace(RouteLocation::new("/items/2"))
                .expect("the equal-shape fixture reserves its replacement");
            let action = engine.action().expect("the replacement has real work");
            let ticket = drive_to_commit(engine, action);
            (request, seal_current(engine, ticket))
        }

        let mut source = ready_engine();
        let mut receiver = ready_engine();
        let (source_request, source_owner) = stage_equal_history_owner(&mut source);
        let (receiver_request, receiver_owner) = stage_equal_history_owner(&mut receiver);

        assert_ne!(source.router, receiver.router);
        assert_eq!(source.history_snapshot(), receiver.history_snapshot());
        let source_engine_before = source.snapshot();
        let receiver_engine_before = receiver.snapshot();
        let source_history_before = source.testing_history_authority_snapshot();
        let receiver_history_before = receiver.testing_history_authority_snapshot();
        let source_request_before = source.request(source_request).unwrap().clone();
        let receiver_request_before = receiver.request(receiver_request).unwrap().clone();
        let source_owner_before = super::PreparedPublicationOwnerSnapshot::from(&source_owner);
        let receiver_owner_before = super::PreparedPublicationOwnerSnapshot::from(&receiver_owner);

        assert_eq!(
            receiver.probe_sealed_commit_evidence(source_owner.evidence()),
            Err(CommitError::Stale),
            "equal raw history shape cannot cross the router/history incarnation boundary"
        );

        assert_eq!(source.snapshot(), source_engine_before);
        assert_eq!(receiver.snapshot(), receiver_engine_before);
        assert_eq!(
            source.testing_history_authority_snapshot(),
            source_history_before
        );
        assert_eq!(
            receiver.testing_history_authority_snapshot(),
            receiver_history_before
        );
        assert_eq!(source.request(source_request), Some(&source_request_before));
        assert_eq!(
            receiver.request(receiver_request),
            Some(&receiver_request_before)
        );
        assert_eq!(
            super::PreparedPublicationOwnerSnapshot::from(&source_owner),
            source_owner_before
        );
        assert_eq!(
            super::PreparedPublicationOwnerSnapshot::from(&receiver_owner),
            receiver_owner_before
        );
    }

    #[test]
    fn prepared_replace_keeps_exact_owner_in_core() {
        let mut engine = ready_engine();
        engine.push(RouteLocation::new("/items/1")).unwrap();
        settle_current(&mut engine);
        engine.replace(RouteLocation::new("/items/2")).unwrap();
        let action = engine
            .action()
            .expect("replace has one real execution action");
        let ticket = drive_to_commit(&mut engine, action);
        let exact_owner = engine.testing_history_capability_snapshot();
        assert!(exact_owner.has_exactly_one_owner());
        assert!(exact_owner.execution().is_some());
        assert!(exact_owner.source().is_some());

        let sealed = seal_current(&mut engine, ticket);
        let _ticket = engine
            .prepare_publication(sealed)
            .expect("the exact replace publication prepares");

        assert_eq!(
            engine.testing_history_capability_snapshot(),
            exact_owner,
            "preparation must leave the one exact authorizing owner in Core; the returned value is only a ticket"
        );
    }

    #[test]
    fn prepare_rejection_retains_exact_owner_without_stage_reconstruction() {
        let mut engine = ready_engine();
        engine.push(RouteLocation::new("/items/1")).unwrap();
        settle_current(&mut engine);
        engine.replace(RouteLocation::new("/items/2")).unwrap();
        let action = engine
            .action()
            .expect("replace has one real execution action");
        let ticket = drive_to_commit(&mut engine, action);
        let exact_owner = engine.testing_history_capability_snapshot();
        let sealed = seal_current(&mut engine, ticket);
        engine.current.as_mut().unwrap().stage = WorkStage::Applying;
        let history_before = engine.testing_history_authority_snapshot();
        let state_before = engine.state.snapshot();
        let ids_before = engine.ids.snapshot();

        assert!(matches!(
            engine.prepare_publication(sealed),
            Err(CommitError::Stale)
        ));
        assert_eq!(engine.state.snapshot(), state_before);
        assert_eq!(engine.ids.snapshot(), ids_before);
        assert_eq!(engine.testing_history_authority_snapshot(), history_before);
        assert_eq!(engine.current.as_ref().unwrap().stage, WorkStage::Applying);
        assert_eq!(
            engine.testing_history_capability_snapshot(),
            exact_owner,
            "a real preparation rejection must retain the exact owner independently of WorkStage"
        );
    }

    #[test]
    fn publication_refreeze_rejection_retains_owner_until_bounded_fail_closed() {
        let mut engine = ready_engine();
        engine.push(RouteLocation::new("/items/1")).unwrap();
        settle_current(&mut engine);
        engine.replace(RouteLocation::new("/items/2")).unwrap();
        let action = engine
            .action()
            .expect("replace has one real execution action");
        let ticket = drive_to_commit(&mut engine, action);
        let sealed = seal_current(&mut engine, ticket);
        let prepared = engine
            .prepare_publication(sealed)
            .expect("replace publication prepares before refreeze drift");
        let exact_owner = engine.testing_history_capability_snapshot();
        engine.current.as_mut().unwrap().stage = WorkStage::Applying;
        let state_before = engine.state.snapshot();
        let history_before = engine.testing_history_authority_snapshot();
        let mutation_ran = std::cell::Cell::new(false);

        assert_eq!(
            engine.publish_prepared(prepared, || mutation_ran.set(true)),
            Err(CommitError::Stale)
        );
        assert!(!mutation_ran.get());
        assert_eq!(engine.state.snapshot(), state_before);
        assert_eq!(engine.testing_history_authority_snapshot(), history_before);
        assert_eq!(engine.testing_history_capability_snapshot(), exact_owner);

        assert!(engine.fail_closed_authority_protocol(|| true));
        assert!(!engine.testing_history_capability_snapshot().has_owner());
    }

    #[test]
    fn prepared_replace_ticket_after_fail_closed_is_exact_zero_write_and_ownerless() {
        let mut engine = ready_engine();
        engine.push(RouteLocation::new("/items/1")).unwrap();
        settle_current(&mut engine);
        let request = engine.replace(RouteLocation::new("/items/2")).unwrap();
        let action = engine
            .action()
            .expect("replace has one real execution action");
        let ticket = drive_to_commit(&mut engine, action);
        let sealed = seal_current(&mut engine, ticket);
        let prepared = engine
            .prepare_publication(sealed)
            .expect("replace publication prepares before authority loss");

        assert!(engine.fail_closed_authority_protocol(|| true));
        let terminal_state = engine.state.snapshot();
        let terminal_history = engine.testing_history_authority_snapshot();
        let terminal_status = engine.request(request).unwrap().status().clone();
        let terminal_trace = engine.request(request).unwrap().trace().to_vec();
        let terminal_owner = engine.testing_history_capability_snapshot();
        assert!(!terminal_owner.has_owner());
        assert!(engine.current.is_none());
        assert!(engine.action.is_none());

        let mutation_ran = std::cell::Cell::new(false);
        assert_eq!(
            engine.publish_prepared(prepared, || mutation_ran.set(true)),
            Err(CommitError::Stale)
        );

        assert_eq!(
            (
                mutation_ran.get(),
                engine.state.snapshot(),
                engine.testing_history_authority_snapshot(),
                engine.request(request).unwrap().status().clone(),
                engine.request(request).unwrap().trace().to_vec(),
                format!("{:?}", engine.current),
                engine.action.is_none(),
                engine.testing_history_capability_snapshot(),
            ),
            (
                false,
                terminal_state,
                terminal_history,
                terminal_status,
                terminal_trace,
                "None".to_owned(),
                true,
                terminal_owner,
            ),
            "a stale prepared ticket cannot run mutation, revive fail-closed state/current, or publish history"
        );
    }

    #[test]
    fn running_replace_stale_report_is_exact_zero_write_for_history_and_owner() {
        let mut engine = ready_engine_with_tree(reconciliation_tree());
        let request = engine.replace(RouteLocation::new("/items/1")).unwrap();
        let EngineAction::FrameTask(parent) = engine.action().unwrap() else {
            panic!("empty-history replace starts with the real parent frame task")
        };
        let stale_parent = parent.identity().clone();
        let next_outlet = engine
            .current
            .as_ref()
            .unwrap()
            .branch
            .as_ref()
            .unwrap()
            .routes()[1]
            .slot()
            .clone();
        engine
            .accept_applied_report(parent.into_applied_report(vec![next_outlet]))
            .unwrap();
        let EngineAction::MountRequest(mount) = engine.action().unwrap() else {
            panic!("the accepted parent report yields the real child mount")
        };
        let grant = engine.resolve_mount(mount).unwrap();
        let receipt = grant.attest_receipt(crate::mount::MountReceiptDigest::from_opaque(0x51));
        let permit = engine.accept_mount_receipt(receipt).unwrap();
        let completion = permit.complete(crate::mount::MountRegistrationDigest::from_opaque(0x52));
        engine.complete_mount_registration(completion).unwrap();
        assert!(matches!(&engine.action, Some(EngineAction::FrameTask(_))));

        let status_before = engine.request(request).unwrap().status().clone();
        let trace_before = engine.request(request).unwrap().trace().to_vec();
        let state_before = engine.state.snapshot();
        let ids_before = engine.ids.snapshot();
        let history_before = engine.testing_history_authority_snapshot();
        let owner_before = engine.testing_history_capability_snapshot();
        let current_before = format!("{:?}", engine.current);
        let action_before = engine.action.as_ref().map(ActionSnapshot::from);

        assert_eq!(
            engine.begin_view_report(&stale_parent).map(|_| ()),
            Err(ReportError::Stale)
        );
        assert_eq!(engine.request(request).unwrap().status(), &status_before);
        assert_eq!(engine.request(request).unwrap().trace(), trace_before);
        assert_eq!(engine.state.snapshot(), state_before);
        assert_eq!(engine.ids.snapshot(), ids_before);
        assert_eq!(engine.testing_history_authority_snapshot(), history_before);
        assert_eq!(engine.testing_history_capability_snapshot(), owner_before);
        assert_eq!(format!("{:?}", engine.current), current_before);
        assert_eq!(
            engine.action.as_ref().map(ActionSnapshot::from),
            action_before
        );
    }

    #[test]
    fn running_replace_old_cleanup_replay_is_exact_zero_write_for_history_and_owner() {
        let mut engine = ready_engine();
        engine.push(RouteLocation::new("/items/1")).unwrap();
        let old_cleanup = publish_current(&mut engine);
        let old_replay = valid_completion(&old_cleanup);
        engine
            .complete_cleanup(valid_completion(&old_cleanup))
            .unwrap();

        let request = engine.replace(RouteLocation::new("/items/2")).unwrap();
        let status_before = engine.request(request).unwrap().status().clone();
        let trace_before = engine.request(request).unwrap().trace().to_vec();
        let state_before = engine.state.snapshot();
        let ids_before = engine.ids.snapshot();
        let history_before = engine.testing_history_authority_snapshot();
        let owner_before = engine.testing_history_capability_snapshot();
        let current_before = format!("{:?}", engine.current);
        let action_before = engine.action.as_ref().map(ActionSnapshot::from);

        assert_eq!(
            engine.complete_cleanup(old_replay),
            Err(CleanupError::Duplicate)
        );
        assert_eq!(engine.request(request).unwrap().status(), &status_before);
        assert_eq!(engine.request(request).unwrap().trace(), trace_before);
        assert_eq!(engine.state.snapshot(), state_before);
        assert_eq!(engine.ids.snapshot(), ids_before);
        assert_eq!(engine.testing_history_authority_snapshot(), history_before);
        assert_eq!(engine.testing_history_capability_snapshot(), owner_before);
        assert_eq!(format!("{:?}", engine.current), current_before);
        assert_eq!(
            engine.action.as_ref().map(ActionSnapshot::from),
            action_before
        );
    }

    #[test]
    fn normal_root_binding_is_independently_refrozen_before_history_prepare() {
        let mut engine = ready_engine();
        engine.push(RouteLocation::new("/items/1")).unwrap();
        settle_current(&mut engine);
        engine.replace(RouteLocation::new("/items/2")).unwrap();
        let action = engine
            .action()
            .expect("replace has one real execution action");
        let ticket = drive_to_commit(&mut engine, action);
        let capability = engine.testing_history_capability_snapshot();
        let active_before = engine.active().clone();
        let history_before = engine.testing_history_authority_snapshot();
        let alternate = engine.current.as_ref().unwrap().reservation[1].clone();
        assert_ne!(
            engine.current.as_ref().unwrap().reservation[0].frame_task(),
            alternate.frame_task()
        );
        engine.current.as_mut().unwrap().reservation[0] = alternate;
        let roots = engine.current.as_ref().unwrap().cleanup_roots.clone();
        let count = usize::from(!roots.is_empty());

        assert_eq!(
            engine
                .seal_commit(
                    &ticket,
                    CleanupManifestSeal::new(roots, manifest_digest(0xB1AD), count),
                )
                .map(|_| ()),
            Err(CommitError::Stale),
            "the ticket identity cannot substitute for an exact normal-root refreeze"
        );
        let evidence = engine
            .testing_history_drift_evidence_snapshot()
            .expect("binding drift keeps typed evidence through cleanup");
        assert_eq!(
            evidence.kind(),
            crate::history_authority::HistoryDriftEvidenceKind::Binding
        );
        assert_eq!(evidence.capability(), &capability);
        assert_eq!(evidence.expected_binding(), capability.execution());
        assert_ne!(evidence.observed_binding(), capability.execution());
        assert!(evidence.capability().has_exactly_one_owner());
        assert_eq!(engine.active(), &active_before);
        assert_eq!(engine.testing_history_authority_snapshot(), history_before);

        let Some(EngineAction::FailureCleanup(failure)) = engine.action() else {
            panic!("binding drift enters bounded authority-loss cleanup")
        };
        let request = failure.request();
        let roots = failure.logical_roots().to_vec();
        let count = usize::from(!roots.is_empty());
        let cleanup = engine
            .prepare_failure_cleanup(
                failure,
                CleanupManifestSeal::new(roots, manifest_digest(0xB1AE), count),
            )
            .unwrap();
        assert!(engine.testing_history_drift_evidence_snapshot().is_some());
        engine.complete_cleanup(valid_completion(&cleanup)).unwrap();
        assert!(matches!(
            engine.request(request).unwrap().status(),
            RequestStatus::Failed(failure)
                if failure.kind() == NavigationFailureKind::AuthorityLost
        ));
        assert!(!engine.testing_history_capability_snapshot().has_owner());
        assert!(engine.testing_history_drift_evidence_snapshot().is_none());
        assert_eq!(engine.active(), &active_before);
        assert_eq!(engine.testing_history_authority_snapshot(), history_before);
    }

    #[test]
    fn source_drift_keeps_exact_owner_and_observed_evidence_until_cleanup_terminal() {
        let mut engine = ready_engine();
        engine.push(RouteLocation::new("/items/1")).unwrap();
        settle_current(&mut engine);
        engine.push(RouteLocation::new("/about")).unwrap();
        settle_current(&mut engine);
        engine.replace(RouteLocation::new("/items/2")).unwrap();
        let action = engine
            .action()
            .expect("replace has one real execution action");
        let ticket = drive_to_commit(&mut engine, action);
        let capability = engine.testing_history_capability_snapshot();
        engine.history.testing_move_cursor_back();
        let active_before = engine.active().clone();
        let history_after_drift = engine.testing_history_authority_snapshot();
        let roots = engine.current.as_ref().unwrap().cleanup_roots.clone();
        let count = usize::from(!roots.is_empty());

        assert_eq!(
            engine
                .seal_commit(
                    &ticket,
                    CleanupManifestSeal::new(roots, manifest_digest(0x50A2), count),
                )
                .map(|_| ()),
            Err(CommitError::Stale)
        );
        let evidence = engine
            .testing_history_drift_evidence_snapshot()
            .expect("source drift keeps typed evidence through cleanup");
        assert_eq!(
            evidence.kind(),
            crate::history_authority::HistoryDriftEvidenceKind::Source
        );
        assert_eq!(evidence.capability(), &capability);
        assert_eq!(evidence.expected_source(), capability.source());
        assert_ne!(evidence.observed_source(), capability.source());
        assert!(evidence.capability().has_exactly_one_owner());
        assert_eq!(engine.active(), &active_before);
        assert_eq!(
            engine.testing_history_authority_snapshot(),
            history_after_drift
        );

        let Some(EngineAction::FailureCleanup(failure)) = engine.action() else {
            panic!("source drift enters bounded authority-loss cleanup")
        };
        let request = failure.request();
        let roots = failure.logical_roots().to_vec();
        let count = usize::from(!roots.is_empty());
        let cleanup = engine
            .prepare_failure_cleanup(
                failure,
                CleanupManifestSeal::new(roots, manifest_digest(0x50A3), count),
            )
            .unwrap();
        assert!(engine.testing_history_drift_evidence_snapshot().is_some());
        engine.complete_cleanup(valid_completion(&cleanup)).unwrap();
        assert!(matches!(
            engine.request(request).unwrap().status(),
            RequestStatus::Failed(failure)
                if failure.kind() == NavigationFailureKind::AuthorityLost
        ));
        assert!(!engine.testing_history_capability_snapshot().has_owner());
        assert!(engine.testing_history_drift_evidence_snapshot().is_none());
        assert_eq!(engine.active(), &active_before);
        assert_eq!(
            engine.testing_history_authority_snapshot(),
            history_after_drift
        );
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
            &RequestStatus::Running(NavigationPhase::ApplyingFrame(1))
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
    fn reconciliation_schedule_first_entering_leaf_targets_the_preserved_boundary_slot() {
        let mut engine = ready_engine_with_tree(reconciliation_tree());
        engine.push(RouteLocation::new("/items/1")).unwrap();
        settle_current(&mut engine);
        let active = engine.active().frames().to_vec();
        let boundary_slot = active[2].slot();

        engine.push(RouteLocation::new("/items/2")).unwrap();
        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("preserved boundary slot must issue the entering leaf directly")
        };

        assert_eq!(task.identity().branch_index(), 2);
        assert_eq!(task.frame_version().slot(), boundary_slot);
        assert_eq!(task.view(), &RouteViewKey::new("item").unwrap());
        assert_eq!(
            &engine.current.as_ref().unwrap().target_frames[..2],
            &active[..2]
        );
        assert!(engine.current.as_ref().unwrap().applied_frames.is_empty());
        assert_eq!(
            engine.current.as_ref().unwrap().cleanup_roots,
            vec![active[2].clone()]
        );
    }

    #[test]
    fn reconciliation_schedule_append_without_retained_slot_uses_sealed_mount_protocol() {
        let mut engine = ready_engine_with_tree(reconciliation_tree());
        engine.push(RouteLocation::new("/items")).unwrap();
        settle_current(&mut engine);
        let active = engine.active().frames().to_vec();

        engine.push(RouteLocation::new("/items/1")).unwrap();
        let EngineAction::MountRequest(request) = engine.action().unwrap() else {
            panic!("append without a registered boundary slot must request one mount")
        };
        assert_eq!(request.parent(), &active[1]);
        let grant = engine.resolve_mount(request).unwrap();
        let slot = grant.slot();
        let receipt = grant.attest_receipt(crate::mount::MountReceiptDigest::from_opaque(0x901));
        let permit = engine.accept_mount_receipt(receipt).unwrap();
        let completion = permit.complete(crate::mount::MountRegistrationDigest::from_opaque(0x903));
        engine.complete_mount_registration(completion).unwrap();

        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("sealed mount completion must issue the entering leaf")
        };
        assert_eq!(task.identity().branch_index(), 2);
        assert_eq!(task.frame_version().slot(), slot);
        assert_eq!(
            &engine.current.as_ref().unwrap().target_frames[..2],
            &active
        );
    }

    #[test]
    fn reconciliation_schedule_preserved_mount_rejects_foreign_projection_context() {
        let mut engine = ready_engine_with_tree(reconciliation_tree());
        engine.push(RouteLocation::new("/items")).unwrap();
        settle_current(&mut engine);
        engine.push(RouteLocation::new("/items/1")).unwrap();
        let EngineAction::MountRequest(request) = engine.action().unwrap() else {
            panic!("preserved parent must request the missing child slot")
        };

        let mut foreign = ready_engine_with_tree(reconciliation_tree());
        foreign.push(RouteLocation::new("/items/1")).unwrap();
        settle_current(&mut foreign);
        foreign.push(RouteLocation::new("/items/2")).unwrap();
        let foreign_projection = foreign
            .current
            .as_ref()
            .and_then(|current| current.reconciliation.clone())
            .unwrap();
        engine.current.as_mut().unwrap().reconciliation = Some(foreign_projection);
        let before = engine.snapshot();

        assert_eq!(
            engine.resolve_mount(request).map_err(|error| error.kind()),
            Err(MountError::WrongPhase)
        );
        assert_eq!(engine.snapshot(), before);
    }

    #[test]
    fn reconciliation_schedule_truncate_reaches_ready_to_switch_without_entering_task() {
        let mut engine = ready_engine_with_tree(reconciliation_tree());
        engine.push(RouteLocation::new("/items/1")).unwrap();
        settle_current(&mut engine);
        let active = engine.active().frames().to_vec();
        let history_before = engine.history().clone();

        let request = engine.push(RouteLocation::new("/items")).unwrap();
        let EngineAction::Commit(ticket) = engine.action().unwrap() else {
            panic!("truncate must consume a sealed commit ticket without entering work")
        };
        assert_eq!(
            engine.request(request).unwrap().status(),
            &RequestStatus::Running(NavigationPhase::ReadyToSwitch)
        );
        assert_eq!(engine.history(), &history_before);
        let current = engine.current.as_ref().unwrap();
        assert_eq!(current.target_frames, active[..2]);
        assert!(current.applied_frames.is_empty());
        assert_eq!(current.cleanup_roots, vec![active[2].clone()]);

        let sealed = seal_current(&mut engine, ticket);
        assert_eq!(engine.history(), &history_before);
        let cleanup = sealed.cleanup().clone();
        publish_sealed(&mut engine, sealed, || {}).unwrap();
        assert_eq!(engine.history().len(), history_before.len() + 1);
        assert_eq!(engine.active().frames(), &active[..2]);
        engine.complete_cleanup(valid_completion(&cleanup)).unwrap();
    }

    #[test]
    fn reconciliation_schedule_nested_fragment_issues_one_new_generation_effect() {
        let mut engine = ready_engine_with_tree(reconciliation_tree());
        engine.push(RouteLocation::new("/items/1")).unwrap();
        settle_current(&mut engine);
        let active = engine.active().frames().to_vec();

        let request = engine
            .push(RouteLocation::new("/items/1").with_fragment("details"))
            .unwrap();
        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("nested fragment must issue the leaf effect directly")
        };
        assert_eq!(task.identity().branch_index(), 2);
        assert_eq!(task.identity().frame(), active[2].frame());
        assert_eq!(task.identity().generation(), active[2].generation().next());
        assert_eq!(task.input().fragment(), Some("details"));
        assert_eq!(
            engine
                .request(request)
                .unwrap()
                .trace()
                .iter()
                .filter(|event| event.kind() == NavigationTraceEventKind::FrameTaskIssued)
                .count(),
            1
        );
    }

    #[test]
    fn reconciliation_schedule_failure_cleanup_contains_only_applied_entering_suffix() {
        let mut engine = ready_engine_with_tree(reconciliation_tree());
        engine.push(RouteLocation::new("/items/1")).unwrap();
        settle_current(&mut engine);
        let preserved = engine.active().frames().to_vec();

        engine.push(RouteLocation::new("/items/2")).unwrap();
        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("entering leaf task expected")
        };
        let entering = task.frame_version().clone();
        engine
            .accept_applied_report(task.into_applied_report(Vec::new()))
            .unwrap();
        let EngineAction::Commit(_) = engine.action().unwrap() else {
            panic!("applied entering leaf must reach commit")
        };
        let failure = engine.begin_failure_cleanup(
            super::PendingFailureKind::Apply,
            NavigationFailure::new(NavigationFailureKind::Apply, "pre-commit apply rejected"),
        );

        assert_eq!(failure.logical_roots(), &[entering]);
        assert!(
            failure
                .logical_roots()
                .iter()
                .all(|root| !preserved.contains(root))
        );
    }

    #[test]
    fn reconciliation_schedule_cleanup_settles_before_request_terminal() {
        let mut engine = ready_engine_with_tree(reconciliation_tree());
        engine.push(RouteLocation::new("/items/1")).unwrap();
        settle_current(&mut engine);
        let request = engine.push(RouteLocation::new("/items/2")).unwrap();
        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("entering leaf task expected")
        };
        let failure = engine
            .begin_view_report(task.identity())
            .unwrap()
            .failed(NavigationFailure::new(
                NavigationFailureKind::Apply,
                "entering effect rejected",
            ))
            .unwrap();
        let cleanup = engine
            .prepare_failure_cleanup(
                failure,
                CleanupManifestSeal::new(Vec::new(), manifest_digest(0x905), 0),
            )
            .unwrap();
        engine.complete_cleanup(valid_completion(&cleanup)).unwrap();

        let trace = engine.request(request).unwrap().trace();
        let settled = trace
            .iter()
            .position(|event| event.kind() == NavigationTraceEventKind::CleanupSettled)
            .unwrap();
        let terminal = trace
            .iter()
            .position(|event| event.kind() == NavigationTraceEventKind::RequestTerminal)
            .unwrap();
        assert!(settled < terminal);
    }

    #[test]
    fn reconciliation_schedule_fragment_residual_cleans_only_new_generation_before_terminal() {
        let mut engine = ready_engine_with_tree(reconciliation_tree());
        engine.push(RouteLocation::new("/items/1")).unwrap();
        settle_current(&mut engine);
        let active_before = engine.active().clone();
        let history_before = engine.history().clone();

        let request = engine
            .push(RouteLocation::new("/items/1").with_fragment("details"))
            .unwrap();
        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("fragment leaf effect expected")
        };
        let new_generation = task.frame_version().clone();
        let failure = engine
            .begin_view_report(task.identity())
            .unwrap()
            .failed_with_physical_residual(NavigationFailure::new(
                NavigationFailureKind::Apply,
                "fragment effect left a physical residual",
            ));

        assert_eq!(failure.logical_roots(), &[new_generation.clone()]);
        assert_eq!(engine.active(), &active_before);
        assert_eq!(engine.history(), &history_before);
        let cleanup = engine
            .prepare_failure_cleanup(
                failure,
                CleanupManifestSeal::new(
                    vec![new_generation],
                    manifest_digest(0x907),
                    NONZERO_OBLIGATION_COUNT,
                ),
            )
            .unwrap();
        assert_eq!(engine.active(), &active_before);
        assert_eq!(engine.history(), &history_before);
        engine.complete_cleanup(valid_completion(&cleanup)).unwrap();
        assert_eq!(engine.active(), &active_before);
        assert_eq!(engine.history(), &history_before);
        assert!(matches!(
            engine.request(request).unwrap().status(),
            RequestStatus::Failed(failure)
                if failure.kind() == NavigationFailureKind::Apply
        ));
    }

    #[test]
    fn reconciliation_schedule_equivalent_consumes_commit_and_publishes_push_once() {
        let mut engine = ready_engine_with_tree(reconciliation_tree());
        engine.push(RouteLocation::new("/items/1")).unwrap();
        settle_current(&mut engine);
        let history_before = engine.history().clone();

        let request = engine.push(RouteLocation::new("items//1/")).unwrap();
        let EngineAction::Commit(ticket) = engine.action().unwrap() else {
            panic!("equivalent push must still consume one sealed commit ticket")
        };
        assert_eq!(engine.history(), &history_before);
        let sealed = seal_current(&mut engine, ticket);
        assert_eq!(engine.history(), &history_before);
        let cleanup = sealed.cleanup().clone();
        publish_sealed(&mut engine, sealed, || {}).unwrap();
        assert_eq!(engine.history().len(), history_before.len() + 1);
        engine.complete_cleanup(valid_completion(&cleanup)).unwrap();
        assert_eq!(engine.history().len(), history_before.len() + 1);
        assert_eq!(
            engine
                .request(request)
                .unwrap()
                .trace()
                .iter()
                .filter(|event| event.kind() == NavigationTraceEventKind::CommitPublished)
                .count(),
            1
        );
    }

    #[test]
    fn reconciliation_schedule_truncate_retains_slot_for_direct_append_reuse() {
        let mut engine = ready_engine_with_tree(reconciliation_tree());
        engine.push(RouteLocation::new("/items/1")).unwrap();
        settle_current(&mut engine);
        let retained_slot = engine.active().frames()[2].slot();

        engine.push(RouteLocation::new("/items")).unwrap();
        settle_current(&mut engine);
        assert!(engine.active_child_slots.iter().any(|binding| {
            binding.parent_branch_index() == 1
                && binding.slot() == retained_slot
                && binding.child().is_none()
        }));
        let next_slot_before = engine.ids.snapshot().next_slot();

        engine.push(RouteLocation::new("/items/2")).unwrap();
        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("retained empty slot must issue the appended leaf directly")
        };
        assert_eq!(task.identity().branch_index(), 2);
        assert_eq!(task.frame_version().slot(), retained_slot);
        assert_eq!(engine.ids.snapshot().next_slot(), next_slot_before);
    }

    #[test]
    fn reconciliation_schedule_prefix_zero_failure_starts_fallback_with_existing_active() {
        let mut engine = ready_engine_with_tree(reconciliation_tree());
        engine
            .push(RouteLocation::new("/items/1").with_search("?tab=one"))
            .unwrap();
        settle_current(&mut engine);
        let active_before = engine.active().clone();
        let history_before = engine.history().clone();

        engine
            .push(RouteLocation::new("/items/2").with_search("?tab=two"))
            .unwrap();
        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("prefix-zero replacement root effect expected")
        };
        assert_eq!(task.identity().branch_index(), 0);
        let failure_cleanup =
            engine
                .begin_view_report(task.identity())
                .unwrap()
                .failed(NavigationFailure::new(
                    NavigationFailureKind::Apply,
                    "replacement root effect rejected",
                ));

        assert!(failure_cleanup.is_none());
        assert_eq!(engine.active(), &active_before);
        assert_eq!(engine.history(), &history_before);
        let Some(EngineAction::FrameTask(fallback)) = engine.action() else {
            panic!("prefix-zero root failure must retain mandatory fallback")
        };
        assert_eq!(fallback.identity().origin(), &FrameOrigin::RouterFallback);
    }

    #[test]
    fn reconciliation_schedule_prefix_zero_residual_cleans_new_root_before_fallback() {
        let mut engine = ready_engine_with_tree(reconciliation_tree());
        engine
            .push(RouteLocation::new("/items/1").with_search("?tab=one"))
            .unwrap();
        settle_current(&mut engine);
        let old = engine.active().frames().to_vec();
        let active_before = engine.active().clone();
        let history_before = engine.history().clone();

        engine
            .push(RouteLocation::new("/items/2").with_search("?tab=two"))
            .unwrap();
        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("prefix-zero replacement root effect expected")
        };
        let new_root = task.frame_version().clone();
        let failure = engine
            .begin_view_report(task.identity())
            .unwrap()
            .failed_with_physical_residual(NavigationFailure::new(
                NavigationFailureKind::Apply,
                "replacement root left a physical residual",
            ));

        assert_eq!(failure.logical_roots(), &[new_root.clone()]);
        assert!(
            failure
                .logical_roots()
                .iter()
                .all(|root| !old.contains(root))
        );
        assert!(matches!(
            failure.continuation,
            super::FailureCleanupContinuation::BeginFallback
        ));
        assert_eq!(engine.active(), &active_before);
        assert_eq!(engine.history(), &history_before);

        let cleanup = engine
            .prepare_failure_cleanup(
                failure,
                CleanupManifestSeal::new(
                    vec![new_root],
                    manifest_digest(0x915),
                    NONZERO_OBLIGATION_COUNT,
                ),
            )
            .unwrap();
        engine.complete_cleanup(valid_completion(&cleanup)).unwrap();

        assert_eq!(engine.active(), &active_before);
        assert_eq!(engine.history(), &history_before);
        let Some(EngineAction::FrameTask(fallback)) = engine.action() else {
            panic!("prefix-zero residual cleanup must continue into fallback")
        };
        assert_eq!(fallback.identity().origin(), &FrameOrigin::RouterFallback);
    }

    #[test]
    fn nested_branch_changing_nested_push_replaces_only_the_suffix() {
        let mut engine = ready_engine();
        engine.push(RouteLocation::new("/items/1")).unwrap();
        settle_current(&mut engine);
        let old = engine.active().frames().to_vec();
        engine.push(RouteLocation::new("/about")).unwrap();
        let EngineAction::FrameTask(leaf) = engine.action().unwrap() else {
            panic!("changed nested branch starts with its entering suffix")
        };
        assert_eq!(leaf.view(), &RouteViewKey::new("about").unwrap());
        assert_eq!(leaf.identity().branch_index(), 1);
        assert_ne!(leaf.frame_version().frame(), old[1].frame());
        let ticket = drive_to_commit(&mut engine, EngineAction::FrameTask(leaf));
        let current = engine.current.as_ref().unwrap();
        assert_eq!(current.cleanup_roots, vec![old[1].clone()]);
        assert_eq!(current.target_frames[0], old[0]);
        assert_ne!(current.target_frames[1].frame(), old[1].frame());
        let sealed = seal_current(&mut engine, ticket);
        let cleanup = sealed.cleanup().clone();
        publish_sealed(&mut engine, sealed, || {}).unwrap();
        assert_eq!(engine.active().frames().len(), 2);
        engine.complete_cleanup(valid_completion(&cleanup)).unwrap();
    }

    #[test]
    fn nested_fragment_push_reconciles_only_the_leaf_generation() {
        let mut engine = ready_engine();
        engine.push(RouteLocation::new("/items/1")).unwrap();
        settle_current(&mut engine);
        let old = engine.active().frames().to_vec();
        engine
            .push(RouteLocation::new("/items/1").with_fragment("details"))
            .unwrap();
        let EngineAction::FrameTask(leaf) = engine.action().unwrap() else {
            panic!("nested fragment starts with the leaf effect")
        };
        assert_eq!(leaf.view(), &RouteViewKey::new("items").unwrap());
        assert_eq!(leaf.identity().branch_index(), 1);
        assert_eq!(leaf.frame_version().frame(), old[1].frame());
        assert_eq!(
            leaf.frame_version().generation(),
            old[1].generation().next()
        );
        let ticket = drive_to_commit(&mut engine, EngineAction::FrameTask(leaf));
        let current = engine.current.as_ref().unwrap();
        assert_eq!(current.cleanup_roots, vec![old[1].clone()]);
        assert_eq!(current.target_frames[0], old[0]);
        assert_eq!(current.target_frames[1].frame(), old[1].frame());
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
            unsorted_engine.testing_history_authority_snapshot(),
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
                unsorted_engine.testing_history_authority_snapshot(),
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
            foreign_engine.testing_history_authority_snapshot(),
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
                foreign_engine.testing_history_authority_snapshot(),
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
            duplicate_engine.testing_history_authority_snapshot(),
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
                duplicate_engine.testing_history_authority_snapshot(),
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
        let mut engine = ready_engine_with_tree(reconciliation_tree());
        engine.push(RouteLocation::new("/items")).unwrap();
        settle_current(&mut engine);
        let old_active = engine.active().clone();
        let old_history = engine.history().clone();
        let old_frames = old_active.frames().to_vec();
        let request = engine.push(RouteLocation::new("/items/1")).unwrap();
        let EngineAction::MountRequest(mount) = engine.action().unwrap() else {
            panic!("append without a retained slot must issue one mount request")
        };

        let failure = engine
            .reject_mount(mount, crate::trace::MountRejectionKind::MissingOutlet)
            .unwrap();

        assert!(failure.logical_roots().is_empty());
        assert!(
            failure
                .logical_roots()
                .iter()
                .all(|root| !old_frames.contains(root))
        );
        assert_eq!(engine.active(), &old_active);
        assert_eq!(engine.history(), &old_history);
        let cleanup = prepare_failure_cleanup(&mut engine, failure, 102, 0);
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
    fn reconciliation_authority_loss_rejects_queue_settles_then_readmits_without_replay() {
        let mut engine = RouterEngine::install(
            reconciliation_tree(),
            RouteViewKey::new("fallback").unwrap(),
        )
        .unwrap();
        let mut issuer = HostAdmissionIssuer::new().unwrap();
        let token = engine.request_host_admission(&mut issuer).unwrap();
        let grant = engine.grant_host_admission(&mut issuer, &token).unwrap();
        let confirmation = issuer.confirm(&grant).unwrap();
        engine.confirm_host(&issuer, &confirmation).unwrap();
        let lease = confirmation.into_lease();
        engine.push(RouteLocation::new("/items")).unwrap();
        settle_current(&mut engine);
        let history_before = engine.history().clone();

        let current = engine.push(RouteLocation::new("/items/1")).unwrap();
        let action = engine.action().unwrap();
        let _ticket = drive_to_commit(&mut engine, action);
        assert!(
            engine
                .testing_history_capability_snapshot()
                .has_exactly_one_owner()
        );
        let queued_first = engine.enqueue_push(RouteLocation::new("/items/2")).unwrap();
        let queued_second = engine.enqueue_push(RouteLocation::new("/items/3")).unwrap();

        let roots = engine.authority_settlement_logical_roots();
        let obligation_count = usize::from(!roots.is_empty());
        let cleanup = engine
            .begin_authority_settlement_retaining(
                &mut issuer,
                lease,
                CleanupManifestSeal::new(roots, manifest_digest(0x515), obligation_count),
                |engine, _| {
                    assert_eq!(
                        engine.availability(),
                        &crate::presentation::RouterAvailability::Ready
                    );
                    assert!(engine.host().is_some());
                    assert!(matches!(
                        engine.request(current).unwrap().status(),
                        RequestStatus::Running(_)
                    ));
                },
            )
            .unwrap();

        assert!(!engine.testing_history_capability_snapshot().has_owner());
        assert!(engine.testing_history_drift_evidence_snapshot().is_none());

        assert_eq!(
            engine.availability(),
            &crate::presentation::RouterAvailability::RecoveringAuthority
        );
        assert_eq!(
            engine.request(current).unwrap().status(),
            &RequestStatus::Running(NavigationPhase::Recovering)
        );
        for queued in [queued_first, queued_second] {
            assert!(matches!(
                engine.request(queued).unwrap().status(),
                RequestStatus::Failed(failure)
                    if failure.kind() == NavigationFailureKind::AuthorityLost
            ));
            assert_eq!(
                engine
                    .request(queued)
                    .unwrap()
                    .trace()
                    .last()
                    .map(NavigationTraceEvent::kind),
                Some(NavigationTraceEventKind::RequestTerminal)
            );
        }

        let ids_before_rejected_push = engine.ids.snapshot();
        assert_eq!(
            engine.enqueue_push(RouteLocation::new("/items/4")),
            Err(PushError::AuthorityRecovering)
        );
        assert_eq!(engine.ids.snapshot(), ids_before_rejected_push);
        if obligation_count != 0 {
            engine
                .observe_cleanup_receipt(&cleanup, crate::trace::CleanupDisposition::Removed)
                .unwrap();
        }
        let receipt_events = engine
            .request(current)
            .unwrap()
            .trace()
            .iter()
            .filter(|event| event.cleanup_disposition().is_some())
            .count();
        let mut invalid_completion = valid_completion(&cleanup);
        invalid_completion.testing_invalidate_binding();
        let mutation_ran = std::cell::Cell::new(false);
        assert_eq!(
            engine.complete_cleanup_with_authority_mutation(invalid_completion, || {
                mutation_ran.set(true)
            }),
            Err(CleanupError::Invalid)
        );
        assert!(!mutation_ran.get());
        assert_eq!(
            engine.availability(),
            &crate::presentation::RouterAvailability::RecoveringAuthority
        );
        assert_eq!(
            engine
                .request(current)
                .unwrap()
                .trace()
                .iter()
                .filter(|event| event.cleanup_disposition().is_some())
                .count(),
            receipt_events
        );
        engine
            .complete_cleanup_with_authority_mutation(valid_completion(&cleanup), || {})
            .unwrap();

        let trace = engine.request(current).unwrap().trace();
        let receipt = trace.iter().position(|event| {
            event.kind() == NavigationTraceEventKind::CleanupSettled
                && event.cleanup_disposition().is_some()
        });
        let settled = trace
            .iter()
            .rposition(|event| event.kind() == NavigationTraceEventKind::CleanupSettled)
            .unwrap();
        let terminal = trace
            .iter()
            .position(|event| event.kind() == NavigationTraceEventKind::RequestTerminal)
            .unwrap();
        if let Some(receipt) = receipt {
            assert!(receipt < settled);
        }
        assert!(settled < terminal);
        assert!(matches!(
            engine.request(current).unwrap().status(),
            RequestStatus::Failed(failure)
                if failure.kind() == NavigationFailureKind::AuthorityLost
        ));
        assert_eq!(engine.active().kind(), ActivePresentationKind::None);
        assert!(engine.host().is_none());
        assert_eq!(engine.history(), &history_before);
        assert!(engine.action().is_none());
        assert!(!engine.testing_history_capability_snapshot().has_owner());
        assert!(engine.testing_history_drift_evidence_snapshot().is_none());

        let token = engine.request_host_admission(&mut issuer).unwrap();
        let grant = engine.grant_host_admission(&mut issuer, &token).unwrap();
        let confirmation = issuer.confirm(&grant).unwrap();
        engine.confirm_host(&issuer, &confirmation).unwrap();

        assert_eq!(
            engine.availability(),
            &crate::presentation::RouterAvailability::Ready
        );
        assert_eq!(engine.active().kind(), ActivePresentationKind::None);
        assert_eq!(engine.history(), &history_before);
        assert!(engine.running_request().is_none());
        assert!(engine.action().is_none());
    }

    #[test]
    fn authority_protocol_failure_retires_lease_and_rejects_readmission_before_issue() {
        let mut engine = RouterEngine::install(
            reconciliation_tree(),
            RouteViewKey::new("fallback").unwrap(),
        )
        .unwrap();
        let mut issuer = HostAdmissionIssuer::new().unwrap();
        let token = engine.request_host_admission(&mut issuer).unwrap();
        let grant = engine.grant_host_admission(&mut issuer, &token).unwrap();
        let confirmation = issuer.confirm(&grant).unwrap();
        engine.confirm_host(&issuer, &confirmation).unwrap();
        let lease = confirmation.into_lease();
        engine.push(RouteLocation::new("/items")).unwrap();
        settle_current(&mut engine);
        let request = engine.push(RouteLocation::new("/items/1")).unwrap();
        let action = engine.action().unwrap();
        let _ticket = drive_to_commit(&mut engine, action);
        assert!(
            engine
                .testing_history_capability_snapshot()
                .has_exactly_one_owner()
        );
        let isolated = std::cell::Cell::new(false);

        engine
            .fail_closed_authority_protocol_retaining(&mut issuer, Some(lease), || {
                isolated.set(true)
            })
            .unwrap();

        assert!(isolated.get());
        assert!(!engine.testing_history_capability_snapshot().has_owner());
        assert!(engine.testing_history_drift_evidence_snapshot().is_none());
        assert!(!issuer.has_current());
        assert_eq!(
            engine.availability(),
            &crate::presentation::RouterAvailability::Unavailable(
                crate::presentation::RouterUnavailableReason::RegistryInvariantViolation,
            )
        );
        assert!(matches!(
            engine.request(request).unwrap().status(),
            RequestStatus::Failed(failure)
                if failure.kind() == NavigationFailureKind::AuthorityLost
        ));
        let ids_before = engine.ids.snapshot();
        assert_eq!(
            engine.request_host_admission(&mut issuer),
            Err(AdmissionError::AuthorityExists)
        );
        assert_eq!(engine.ids.snapshot(), ids_before);
    }

    #[test]
    fn failed_authority_trigger_requires_an_exact_refrozen_terminal_snapshot() {
        let mut engine = RouterEngine::install(
            reconciliation_tree(),
            RouteViewKey::new("fallback").unwrap(),
        )
        .unwrap();
        let mut issuer = HostAdmissionIssuer::new().unwrap();
        let token = engine.request_host_admission(&mut issuer).unwrap();
        let grant = engine.grant_host_admission(&mut issuer, &token).unwrap();
        let confirmation = issuer.confirm(&grant).unwrap();
        engine.confirm_host(&issuer, &confirmation).unwrap();
        let lease = confirmation.into_lease();
        engine.push(RouteLocation::new("/items/1")).unwrap();
        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("failed terminal fixture requires one exact frame task")
        };
        let trigger = engine
            .failed_authority_trigger_for_task(task.identity())
            .unwrap();

        engine
            .current
            .as_mut()
            .expect("task retains current work")
            .applied_frames
            .push(task.frame_version().clone());
        let mut before = engine.snapshot();
        before.live_failed_authority_trigger_revision = None;
        let mutation_ran = std::cell::Cell::new(false);
        let error = engine
            .begin_failed_authority_settlement_retaining(
                &mut issuer,
                lease,
                trigger,
                CleanupManifestSeal::new(
                    vec![task.frame_version().clone()],
                    manifest_digest(0xFA16),
                    1,
                ),
                |_, _| mutation_ran.set(true),
            )
            .expect_err("a merely valid but different refrozen snapshot is stale");

        assert_eq!(
            error.kind(),
            super::BeginAuthoritySettlementErrorKind::Stale
        );
        let _lease = error.into_owner();
        assert!(!mutation_ran.get());
        assert!(issuer.has_current());
        assert_eq!(
            engine.live_failed_authority_trigger_revision, None,
            "an exact-receiver rejection burns the one-shot trigger while returning the lease"
        );
        assert_eq!(engine.snapshot(), before);
    }

    #[test]
    fn failed_authority_source_selection_uses_exact_roots_not_entity_count() {
        let mut engine = RouterEngine::install(
            reconciliation_tree(),
            RouteViewKey::new("fallback").unwrap(),
        )
        .unwrap();
        let mut issuer = HostAdmissionIssuer::new().unwrap();
        let token = engine.request_host_admission(&mut issuer).unwrap();
        let grant = engine.grant_host_admission(&mut issuer, &token).unwrap();
        let confirmation = issuer.confirm(&grant).unwrap();
        engine.confirm_host(&issuer, &confirmation).unwrap();
        let lease = confirmation.into_lease();
        engine.push(RouteLocation::new("/items/1")).unwrap();
        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("failed source fixture requires one exact frame task")
        };
        let trigger = engine
            .failed_authority_trigger_for_task(task.identity())
            .unwrap();
        let roots = vec![task.frame_version().clone()];
        let digest = manifest_digest(0xA570);
        let expected = CleanupBatchToken::sealed(
            task.identity().clone(),
            engine.current.as_ref().unwrap().cleanup_batch,
            &roots,
            digest,
            0,
        );

        let cleanup = engine
            .begin_failed_authority_settlement_retaining(
                &mut issuer,
                lease,
                trigger,
                CleanupManifestSeal::new(roots, digest, 0),
                |_, _| {},
            )
            .expect("exact logical target selection is independent of ECS entity count");

        assert_eq!(cleanup, expected);
        assert!(matches!(
            engine.current.as_ref().map(|current| &current.stage),
            Some(WorkStage::FailureCleanupPrepared {
                continuation: super::FailureCleanupContinuation::AuthoritySettlement,
                ..
            })
        ));
    }

    #[test]
    fn failure_cleanup_roots_count_and_continuation_are_independent() {
        fn pending_failure(
            roots: Vec<RouteFrameVersion>,
            continuation: super::FailureCleanupContinuation,
        ) -> (RouterEngine, super::FailureCleanupRequest) {
            let mut engine = ready_engine();
            engine.push(RouteLocation::new("/items/1")).unwrap();
            let request = engine.begin_failure_cleanup_with_roots(
                super::PendingFailureKind::Apply,
                NavigationFailure::new(
                    NavigationFailureKind::Apply,
                    "independent cleanup manifest fixture",
                ),
                continuation,
                roots,
            );
            (engine, request)
        }

        let (mut empty, empty_request) =
            pending_failure(Vec::new(), super::FailureCleanupContinuation::Terminal);
        let empty_digest = manifest_digest(0xC1EA);
        let empty_expected = CleanupBatchToken::sealed(
            empty_request.identity.clone(),
            empty.current.as_ref().unwrap().cleanup_batch,
            &[],
            empty_digest,
            2,
        );
        let empty_cleanup = empty
            .prepare_failure_cleanup(
                empty_request,
                CleanupManifestSeal::new(Vec::new(), empty_digest, 2),
            )
            .unwrap();
        assert_eq!(empty_cleanup, empty_expected);
        assert!(matches!(
            empty.current.as_ref().map(|current| &current.stage),
            Some(WorkStage::FailureCleanupPrepared {
                continuation: super::FailureCleanupContinuation::Terminal,
                ..
            })
        ));

        let mut roots_engine = ready_engine();
        roots_engine.push(RouteLocation::new("/items/1")).unwrap();
        let root = roots_engine.current.as_ref().unwrap().target_frames[0].clone();
        let roots_request = roots_engine.begin_failure_cleanup_with_roots(
            super::PendingFailureKind::Apply,
            NavigationFailure::new(
                NavigationFailureKind::Apply,
                "zero entity obligation fixture",
            ),
            super::FailureCleanupContinuation::BeginFallback,
            vec![root.clone()],
        );
        let roots_digest = manifest_digest(0xC1EB);
        let roots_expected = CleanupBatchToken::sealed(
            roots_request.identity.clone(),
            roots_engine.current.as_ref().unwrap().cleanup_batch,
            std::slice::from_ref(&root),
            roots_digest,
            0,
        );
        let roots_cleanup = roots_engine
            .prepare_failure_cleanup(
                roots_request,
                CleanupManifestSeal::new(vec![root], roots_digest, 0),
            )
            .expect("logical cleanup roots do not imply an ECS entity obligation");
        assert_eq!(roots_cleanup, roots_expected);
        assert!(matches!(
            roots_engine.current.as_ref().map(|current| &current.stage),
            Some(WorkStage::FailureCleanupPrepared {
                continuation: super::FailureCleanupContinuation::BeginFallback,
                ..
            })
        ));
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

    #[test]
    fn replace_failure_and_fallback_never_publish_history() {
        let mut engine = ready_engine();
        engine.push(RouteLocation::new("/items/1")).unwrap();
        settle_current(&mut engine);
        let history_before = engine.testing_history_authority_snapshot();

        let request = engine
            .replace(RouteLocation::new("/items/2"))
            .expect("replace must reserve one exact normal attempt");
        let EngineAction::FrameTask(normal) = engine.action().unwrap() else {
            panic!("replace must issue its changed frame task")
        };
        let failure = engine
            .begin_view_report(normal.identity())
            .unwrap()
            .failed(NavigationFailure::new(
                NavigationFailureKind::Build,
                "replace build failed",
            ))
            .expect("replace failure must retain exact cleanup authority");
        let cleanup = prepare_failure_cleanup(&mut engine, failure, 227, 0);
        engine.complete_cleanup(valid_completion(&cleanup)).unwrap();

        let EngineAction::FrameTask(fallback) = engine.action().unwrap() else {
            panic!("settled replace failure must issue the fallback branch")
        };
        engine
            .begin_view_report(fallback.identity())
            .unwrap()
            .failed(NavigationFailure::new(
                NavigationFailureKind::Build,
                "replace fallback failed",
            ));

        assert!(matches!(
            engine.request(request).unwrap().status(),
            RequestStatus::Failed(failure)
                if failure.kind() == NavigationFailureKind::RecoveryFailed
        ));
        assert_eq!(engine.testing_history_authority_snapshot(), history_before);
        assert!(
            engine
                .request(request)
                .unwrap()
                .trace()
                .iter()
                .all(|event| event.kind() != NavigationTraceEventKind::CommitPublished)
        );
    }

    #[test]
    fn replace_publication_updates_active_and_history_once() {
        let mut engine = ready_engine();
        engine.push(RouteLocation::new("/items/1")).unwrap();
        settle_current(&mut engine);
        let active_before = engine.active().clone();
        let history_before = engine.testing_history_authority_snapshot();

        let request = engine
            .replace(RouteLocation::new("/items/2"))
            .expect("replace must enqueue");
        let action = engine.action().expect("replace must produce work");
        let ticket = drive_to_commit(&mut engine, action);
        let sealed = seal_current(&mut engine, ticket);
        let prepared = engine.prepare_publication(sealed).unwrap();

        assert_eq!(engine.active(), &active_before);
        assert_eq!(engine.testing_history_authority_snapshot(), history_before);

        engine.publish_prepared(prepared, || {}).unwrap();

        assert_eq!(
            engine.active().location(),
            Some(&RouteLocation::new("/items/2"))
        );
        assert_eq!(
            engine.history().current(),
            Some(&RouteLocation::new("/items/2"))
        );
        let history_after = engine.testing_history_authority_snapshot();
        assert_ne!(history_after.revision(), history_before.revision());
        assert_eq!(history_after.committed_by(), Some(request));
        assert_eq!(
            engine
                .request(request)
                .unwrap()
                .trace()
                .iter()
                .filter(|event| event.kind() == NavigationTraceEventKind::CommitPublished)
                .count(),
            1
        );
    }

    #[test]
    fn replace_identity_exhaustion_is_complete_zero_change() {
        for counter in [
            ReservationCounter::Request,
            ReservationCounter::Transaction,
            ReservationCounter::FrameTask,
            ReservationCounter::CandidateFrame,
            ReservationCounter::CleanupBatch,
        ] {
            let mut engine = ready_engine();
            engine.push(RouteLocation::new("/items/1")).unwrap();
            settle_current(&mut engine);
            engine.testing_exhaust_reservation_counter(counter);
            let before = engine.snapshot();

            assert_eq!(
                engine.replace(RouteLocation::new("/items/2")),
                Err(PushError::IdExhausted),
                "counter: {counter:?}"
            );
            assert_eq!(engine.snapshot(), before, "counter: {counter:?}");
        }
    }

    #[test]
    fn back_target_atomic_applied_receiver_never_derives_outcome_from_ambient_action() {
        let source = include_str!("engine.rs");
        let signature = "pub(crate) fn report_back_target_applied(";
        let function_start = source
            .find(signature)
            .expect("the target atomic applied receiver must remain present");
        let body_start = function_start
            + source[function_start..]
                .find('{')
                .expect("the target atomic applied receiver must have one body");
        let mut depth = 0usize;
        let mut body_end = None;
        for (offset, character) in source[body_start..].char_indices() {
            match character {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        body_end = Some(body_start + offset + character.len_utf8());
                        break;
                    }
                }
                _ => {}
            }
        }
        let receiver = &source
            [function_start..body_end.expect("the target atomic applied receiver body must close")];

        assert!(
            !receiver.contains("self.action"),
            "the target atomic applied receiver must consume a direct typed producer result, not read ambient action"
        );
        assert!(
            !receiver.contains("EngineAction::"),
            "the target atomic applied receiver must not infer its typed outcome by matching EngineAction"
        );
    }
}
