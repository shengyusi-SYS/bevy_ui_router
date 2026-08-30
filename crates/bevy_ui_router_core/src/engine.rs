#![allow(dead_code)]

use std::collections::VecDeque;

use crate::admission::{
    AdmissionError, HostAdmissionAllocatorCounter, HostAdmissionAllocatorExhaustionEvidence,
    HostAdmissionConfirmation, HostAdmissionIssuer, HostAdmissionIssuerOutstandingProjection,
    HostAdmissionIssuerProjection, HostAdmissionToken, HostGrant, HostLease, RetiredHostAuthority,
    RetiredHostAuthorityProjection,
};
use crate::branch::{FrozenErrorViewBinding, MatchedBranch, MatchedRoute, RouteParams};
use crate::cleanup::{
    CleanupBatchSourceProjection, CleanupBatchToken, CleanupCompletion, CleanupCompletionDigest,
    CleanupManifestDigest, CleanupManifestSeal,
};
use crate::diagnostics::{DiagnosticEvent, PostCommitCleanupAnomalyCause, ReportDisposition};
use crate::fingerprint::{RouteBranchFingerprint, RouteBranchProvenance, RouteSearchFingerprint};
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
    AttemptIdentityExhaustionEvidence, CleanupBatchId, CoreIdAllocator, FrameTaskId,
    NavigationRequestId, ReservedBranchAttempt, ReservedFrameAttempt, RouteFrameId,
    RouteGeneration, RouteHostId, RouteId, RouteSlotId, RouterIdentity, TransactionId,
};
use crate::location::RouteLocation;
use crate::matcher::RouteMatcher;
use crate::meta::ErrorBoundaryKey;
use crate::mount::{
    MountConfirmation, MountError, MountGrant, MountIssuer, MountIssuerContextDisposition,
    MountIssuerRequestProjection, MountReceiptAttestation, MountReceiptProof,
    MountRegistrationCompletion, MountRegistrationDigest, MountRegistrationPermit,
    MountRequestReceiverDisposition, MountRequestToken, MountRequestTokenProjection,
};
use crate::presentation::{
    ActivePresentation, ActivePresentationKind, RouterAvailability, RouterNavigationState,
    RouterUnavailableReason,
};
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
    Consumed,
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

#[derive(Clone, Debug, Eq, PartialEq)]
struct FrozenBoundErrorRecovery {
    failure_branch_index: usize,
    binding: FrozenErrorViewBinding,
    mount: FrozenErrorMountSource,
    prepared_prefix: Vec<RouteFrameVersion>,
    reserved_attempt: Option<crate::ids::ReservedAttemptIds>,
}

#[derive(Debug, Eq, PartialEq)]
struct BoundErrorRecoveryIdentityExhaustedOwner {
    recovery: Box<FrozenBoundErrorRecovery>,
    original_failure: NavigationFailure,
    terminal_failure: NavigationFailure,
    exhaustion: AttemptIdentityExhaustionEvidence,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum FrozenErrorMountSource {
    TopSlot {
        host: HostVersion,
        slot: RouteSlotId,
    },
    ExistingChild {
        parent: RouteFrameVersion,
        outlet: RouteSlotKey,
        slot: RouteSlotId,
    },
    MountRequired {
        parent: RouteFrameVersion,
        outlet: RouteSlotKey,
        parent_source: FrozenParentMountSource,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum FrozenParentMountSource {
    TopSlot {
        host: HostVersion,
        slot: RouteSlotId,
    },
    RegisteredChild {
        claim: LogicalChildSlotBinding,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BoundErrorIdentityExhaustedProjection {
    failure_branch_index: usize,
    boundary_route: RouteId,
    boundary: ErrorBoundaryKey,
    view: RouteViewKey,
    mount: FrozenErrorMountSource,
    prepared_prefix: Vec<RouteFrameVersion>,
    original_failure: NavigationFailure,
    terminal_failure: NavigationFailure,
    exhaustion: AttemptIdentityExhaustionEvidence,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum FailureCleanupRecoveryTargetProjection {
    Terminal,
    RouterFallback,
    BoundError {
        failure_branch_index: usize,
        boundary_route: RouteId,
        boundary: ErrorBoundaryKey,
        view: RouteViewKey,
        prepared_prefix: Vec<RouteFrameVersion>,
        reserved_attempt: Option<crate::ids::ReservedAttemptIds>,
    },
    BoundErrorIdentityExhausted(Box<BoundErrorIdentityExhaustedProjection>),
    AuthoritySettlement,
}

#[derive(Debug, Eq, PartialEq)]
enum FailureCleanupContinuationOwner {
    Terminal,
    TerminalRecoveryFailed(Box<BoundErrorRecoveryIdentityExhaustedOwner>),
    BeginFallback,
    BeginBoundError(Box<FrozenBoundErrorRecovery>),
    AuthoritySettlement,
}

impl FailureCleanupContinuationOwner {
    fn receiver_projection(&self) -> FailureCleanupRecoveryTargetProjection {
        match self {
            Self::Terminal => FailureCleanupRecoveryTargetProjection::Terminal,
            Self::TerminalRecoveryFailed(owner) => {
                FailureCleanupRecoveryTargetProjection::BoundErrorIdentityExhausted(Box::new(
                    BoundErrorIdentityExhaustedProjection {
                        failure_branch_index: owner.recovery.failure_branch_index,
                        boundary_route: owner.recovery.binding.boundary_route(),
                        boundary: owner.recovery.binding.boundary().clone(),
                        view: owner.recovery.binding.view().clone(),
                        mount: owner.recovery.mount.clone(),
                        prepared_prefix: owner.recovery.prepared_prefix.clone(),
                        original_failure: owner.original_failure.clone(),
                        terminal_failure: owner.terminal_failure.clone(),
                        exhaustion: owner.exhaustion,
                    },
                ))
            }
            Self::BeginFallback => FailureCleanupRecoveryTargetProjection::RouterFallback,
            Self::BeginBoundError(recovery) => FailureCleanupRecoveryTargetProjection::BoundError {
                failure_branch_index: recovery.failure_branch_index,
                boundary_route: recovery.binding.boundary_route(),
                boundary: recovery.binding.boundary().clone(),
                view: recovery.binding.view().clone(),
                prepared_prefix: recovery.prepared_prefix.clone(),
                reserved_attempt: recovery.reserved_attempt,
            },
            Self::AuthoritySettlement => {
                FailureCleanupRecoveryTargetProjection::AuthoritySettlement
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FailureCleanupReceiverProjection {
    identity: FrameTaskIdentity,
    expected: PendingFailureKind,
    logical_roots: Vec<RouteFrameVersion>,
    recovery: FailureCleanupRecoveryTargetProjection,
}

impl FailureCleanupReceiverProjection {
    pub(crate) const fn request(&self) -> NavigationRequestId {
        self.identity.request()
    }
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
pub(crate) enum PreCommitAuthorityCause {
    AuthorityLost,
    HostLost,
}

impl PreCommitAuthorityCause {
    fn running_failure(self) -> NavigationFailure {
        match self {
            Self::AuthorityLost => NavigationFailure::new(
                NavigationFailureKind::AuthorityLost,
                "physical reconciliation authority was lost",
            ),
            Self::HostLost => NavigationFailure::new(
                NavigationFailureKind::HostLost,
                "router host was lost before presentation commit",
            ),
        }
    }

    fn queued_failure(self) -> NavigationFailure {
        match self {
            Self::AuthorityLost => NavigationFailure::new(
                NavigationFailureKind::AuthorityLost,
                "router authority was lost before request start",
            ),
            Self::HostLost => NavigationFailure::new(
                NavigationFailureKind::HostLost,
                "router host was lost before request start",
            ),
        }
    }
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
    FailureCleanup(FailureCleanupReceiverProjection),
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
    FailureCleanupProjection(&'a FailureCleanupReceiverProjection),
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

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct FailureCleanupRequest {
    receiver: FailureCleanupReceiverProjection,
    failure: NavigationFailure,
    continuation: FailureCleanupContinuationOwner,
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
    fn new(
        identity: FrameTaskIdentity,
        expected: PendingFailureKind,
        logical_roots: Vec<RouteFrameVersion>,
        failure: NavigationFailure,
        continuation: FailureCleanupContinuationOwner,
    ) -> Self {
        let recovery = continuation.receiver_projection();
        Self {
            receiver: FailureCleanupReceiverProjection {
                identity,
                expected,
                logical_roots,
                recovery,
            },
            failure,
            continuation,
        }
    }

    pub(crate) const fn request(&self) -> NavigationRequestId {
        self.receiver.identity.request()
    }

    pub(crate) fn logical_roots(&self) -> &[RouteFrameVersion] {
        &self.receiver.logical_roots
    }

    pub(crate) fn retained_prepared_prefix(&self) -> &[RouteFrameVersion] {
        match &self.continuation {
            FailureCleanupContinuationOwner::BeginBoundError(recovery) => &recovery.prepared_prefix,
            FailureCleanupContinuationOwner::Terminal
            | FailureCleanupContinuationOwner::TerminalRecoveryFailed(_)
            | FailureCleanupContinuationOwner::BeginFallback
            | FailureCleanupContinuationOwner::AuthoritySettlement => &[],
        }
    }

    pub(crate) const fn failure(&self) -> &NavigationFailure {
        &self.failure
    }

    pub(crate) fn freeze_zero_physical_projection(
        &self,
        manifest_digest: CleanupManifestDigest,
        obligation_count: usize,
    ) -> FailureCleanupZeroPhysicalProjection {
        FailureCleanupZeroPhysicalProjection {
            receiver: self.receiver.clone(),
            manifest_digest,
            obligation_count,
        }
    }

    pub(crate) fn physical_receiver_mismatch(
        &self,
        observed: &FailureCleanupZeroPhysicalProjection,
    ) -> Option<FailureCleanupReceiverMismatchProjection> {
        if observed.receiver == self.receiver {
            return None;
        }
        let kind = if observed.receiver.identity.router_identity()
            != self.receiver.identity.router_identity()
        {
            FailureCleanupError::Foreign
        } else {
            FailureCleanupError::Stale
        };
        Some(FailureCleanupReceiverMismatchProjection {
            kind,
            expected: self.receiver.clone(),
            observed_router: observed.receiver.identity.router_identity(),
            observed: Some(observed.receiver.clone()),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FailureCleanupReceiverMismatchProjection {
    pub(crate) kind: FailureCleanupError,
    pub(crate) expected: FailureCleanupReceiverProjection,
    pub(crate) observed_router: RouterIdentity,
    pub(crate) observed: Option<FailureCleanupReceiverProjection>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FailureCleanupManifestContradictionProjection {
    pub(crate) source: FailureCleanupReceiverProjection,
    pub(crate) expected_logical_roots: Vec<RouteFrameVersion>,
    pub(crate) observed_logical_roots: Vec<RouteFrameVersion>,
    pub(crate) submitted_manifest_digest: CleanupManifestDigest,
    pub(crate) submitted_obligation_count: usize,
}

pub(crate) struct CoreFailureCleanupBatchOwner {
    token: CleanupBatchToken,
}

impl CoreFailureCleanupBatchOwner {
    pub(crate) fn into_token(self) -> CleanupBatchToken {
        self.token
    }
}

pub(crate) enum FailureCleanupPrepareOutcome {
    Prepared(CoreFailureCleanupBatchOwner),
    Mismatch {
        request: FailureCleanupRequest,
        evidence: FailureCleanupReceiverMismatchProjection,
    },
    Permanent {
        request: FailureCleanupRequest,
        evidence: FailureCleanupManifestContradictionProjection,
    },
}

pub(crate) struct FailureCleanupPermanentHolder<Physical> {
    pub(crate) request: FailureCleanupRequest,
    pub(crate) physical: Physical,
    pub(crate) evidence: FailureCleanupManifestContradictionProjection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FailureCleanupZeroPhysicalProjection {
    pub(crate) receiver: FailureCleanupReceiverProjection,
    pub(crate) manifest_digest: CleanupManifestDigest,
    pub(crate) obligation_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FailureCleanupZeroPhysicalMismatchProjection {
    pub(crate) source: FailureCleanupReceiverProjection,
    pub(crate) expected: FailureCleanupZeroPhysicalProjection,
    pub(crate) observed: FailureCleanupZeroPhysicalProjection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FailureCleanupZeroTerminalBindingProjection {
    receiver: FailureCleanupReceiverProjection,
    contradiction: FailureCleanupManifestContradictionProjection,
    physical: FailureCleanupZeroPhysicalProjection,
}

pub(crate) struct PreparedFailureCleanupZeroPhysical<Physical> {
    pub(crate) physical: Physical,
    pub(crate) proof: FailureCleanupZeroPhysicalProjection,
    binding: FailureCleanupZeroTerminalBindingProjection,
}

impl<Physical> PreparedFailureCleanupZeroPhysical<Physical> {
    pub(crate) fn bind_zero(
        physical: Physical,
        expected: &FailureCleanupZeroTerminalBindingProjection,
        observed: FailureCleanupZeroPhysicalProjection,
    ) -> Result<Self, (Physical, FailureCleanupZeroPhysicalMismatchProjection)> {
        if expected.physical == observed && observed.obligation_count == 0 {
            Ok(Self {
                physical,
                proof: observed,
                binding: expected.clone(),
            })
        } else {
            Err((
                physical,
                FailureCleanupZeroPhysicalMismatchProjection {
                    source: expected.receiver.clone(),
                    expected: expected.physical.clone(),
                    observed,
                },
            ))
        }
    }

    pub(crate) fn into_physical(self) -> Physical {
        self.physical
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FailureCleanupTerminalProjection {
    pub(crate) identity: RootTaskIdentity,
    pub(crate) status: RequestStatus,
    pub(crate) failure: NavigationFailure,
    pub(crate) trace_digest: u64,
}

#[derive(Clone, Eq, PartialEq)]
struct FailureCleanupZeroTerminalSourceProjection {
    records: VecDeque<NavigationRequestRecord>,
    queue: Vec<NavigationRequestId>,
    state: RouterNavigationState,
    evicted_through: Option<u64>,
    issued_view_tasks: Vec<FrameTaskIdentity>,
    report_dispositions: Vec<ReportDispositionRecord>,
    completed_cleanup: Vec<CompletedCleanupTombstone>,
    current: Option<FailureCleanupZeroTerminalCurrentProjection>,
    action_present: bool,
}

#[derive(Clone, Eq, PartialEq)]
struct FailureCleanupZeroTerminalBranchProjection {
    routes: Vec<MatchedRoute>,
    provenance: RouteBranchProvenance,
    search_fingerprint: RouteSearchFingerprint,
    branch_fingerprint: RouteBranchFingerprint,
}

impl std::fmt::Debug for FailureCleanupZeroTerminalBranchProjection {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FailureCleanupZeroTerminalBranchProjection")
            .field("routes", &self.routes)
            .field("provenance", &"<opaque>")
            .field("search_fingerprint", &self.search_fingerprint)
            .field("branch_fingerprint", &self.branch_fingerprint)
            .finish()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FailureCleanupZeroTerminalPendingMountProjection {
    child_index: usize,
    parent: RouteFrameVersion,
    outlet: RouteSlotKey,
    from_preserved_parent: bool,
    transaction: TransactionId,
    reserved: ReservedFrameAttempt,
    kind: PendingMountKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FailureCleanupZeroTerminalBoundErrorProjection {
    binding: FrozenErrorViewBinding,
    matched: MatchedRoute,
    active_child_slots: Vec<LogicalChildSlotBinding>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum FailureCleanupZeroTerminalAttemptProjection {
    Normal(NavigationCommandKind),
    Fallback {
        original_failure: NavigationFailure,
        bound_error: Option<FailureCleanupZeroTerminalBoundErrorProjection>,
    },
}

#[derive(Clone, Eq, PartialEq)]
pub(crate) struct FailureCleanupZeroTerminalCurrentProjection {
    identity: FrameTaskIdentity,
    branch: Option<FailureCleanupZeroTerminalBranchProjection>,
    location: RouteLocation,
    reservation: Vec<ReservedFrameAttempt>,
    target_frames: Vec<RouteFrameVersion>,
    applied_frames: Vec<RouteFrameVersion>,
    current_target: FrameMountTarget,
    pending_mount: Option<FailureCleanupZeroTerminalPendingMountProjection>,
    stage: WorkStageProjection,
    cleanup_batch: CleanupBatchId,
    cleanup_roots: Vec<RouteFrameVersion>,
    attempt: FailureCleanupZeroTerminalAttemptProjection,
    reconciliation: Option<RouteReconciliationProjection>,
    retired_host: Option<RetiredHostAuthorityProjection>,
}

#[derive(Clone, Eq, PartialEq)]
pub(crate) struct FailureCleanupZeroTerminalMismatchProjection {
    expected: FailureCleanupZeroTerminalSourceProjection,
    observed: FailureCleanupZeroTerminalSourceProjection,
    expected_binding: FailureCleanupZeroTerminalBindingProjection,
    observed_binding: FailureCleanupZeroTerminalBindingProjection,
}

struct PreparedFailureTerminal {
    projection: FailureCleanupTerminalProjection,
    records: VecDeque<NavigationRequestRecord>,
    state: RouterNavigationState,
    evicted_through: Option<u64>,
    issued_view_tasks: Vec<FrameTaskIdentity>,
    report_dispositions: Vec<ReportDispositionRecord>,
    completed_cleanup: Vec<CompletedCleanupTombstone>,
}

pub(crate) struct FailureCleanupZeroTerminalOwner {
    pub(crate) receiver: FailureCleanupReceiverProjection,
    failure: NavigationFailure,
    continuation: FailureCleanupContinuationOwner,
    binding: FailureCleanupZeroTerminalBindingProjection,
    source: FailureCleanupZeroTerminalSourceProjection,
    terminal: PreparedFailureTerminal,
}

pub(crate) struct FailureCleanupPermanentAuthorityOwner {
    request: FailureCleanupRequest,
    evidence: FailureCleanupManifestContradictionProjection,
}

impl FailureCleanupPermanentAuthorityOwner {
    pub(crate) const fn request(&self) -> NavigationRequestId {
        self.request.request()
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        FailureCleanupRequest,
        FailureCleanupManifestContradictionProjection,
    ) {
        (self.request, self.evidence)
    }
}

pub(crate) enum FailureCleanupPermanentCoreOutcome<Physical> {
    AuthoritySettlement {
        physical: Physical,
        owner: FailureCleanupPermanentAuthorityOwner,
    },
    ProvenZero {
        zero: PreparedFailureCleanupZeroPhysical<Physical>,
        owner: FailureCleanupZeroTerminalOwner,
    },
    ZeroMismatch {
        holder: FailureCleanupPermanentHolder<Physical>,
        evidence: FailureCleanupZeroPhysicalMismatchProjection,
    },
    Mismatch(FailureCleanupPermanentHolder<Physical>),
}

pub(crate) enum FailureCleanupZeroTerminalOutcome<Physical> {
    Terminal(FailureCleanupTerminalProjection),
    Mismatch {
        zero: PreparedFailureCleanupZeroPhysical<Physical>,
        owner: FailureCleanupZeroTerminalOwner,
        evidence: FailureCleanupZeroTerminalMismatchProjection,
    },
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
    pub(crate) receiver: FailureCleanupReceiverProjection,
    failure: NavigationFailure,
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
    FailureCleanupManifestPending(FailureCleanupReceiverProjection),
    FailureCleanupPrepared,
    FailureCleanupZeroTerminalPending(FailureCleanupReceiverProjection),
    FailureCleanupCompleting,
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

#[derive(Debug)]
struct PreparedPublicationObservation {
    identity: Option<FrameTaskIdentity>,
    stage: Option<WorkStageProjection>,
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
                Some(WorkStageProjection::CommitSealed(token)) if token == &owner.cleanup
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

#[derive(Debug, Eq, PartialEq)]
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
    PostCommitAuthorityAccepted {
        normal: CleanupBatchToken,
        expected: CorePostCommitCleanupReceiverProjection,
    },
    PostCommitAuthorityCleanup {
        normal: CleanupBatchToken,
        expected: CorePostCommitCompoundProjection,
    },
    FailureCleanupManifestPending {
        expected: FailureCleanupReceiverProjection,
    },
    FailureCleanupPrepared {
        outstanding: CleanupBatchToken,
        failure: NavigationFailure,
        continuation: FailureCleanupContinuationOwner,
    },
    FailureCleanupZeroTerminalPending {
        expected: FailureCleanupZeroTerminalBindingProjection,
    },
    FailureCleanupCompleting {
        outstanding: CleanupBatchToken,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum WorkStageProjection {
    Applying,
    AwaitingMount,
    AwaitingReceipt,
    ConfirmingMount,
    ReadyToCommit,
    CommitSealed(CleanupBatchToken),
    PostCommitCleanup(CleanupBatchToken),
    PostCommitAuthorityAccepted {
        normal: CleanupBatchToken,
        expected: CorePostCommitCleanupReceiverProjection,
    },
    PostCommitAuthorityCleanup {
        normal: CleanupBatchToken,
        expected: CorePostCommitCompoundProjection,
    },
    FailureCleanupManifestPending(FailureCleanupReceiverProjection),
    FailureCleanupPrepared {
        outstanding: CleanupBatchToken,
        failure: NavigationFailure,
    },
    FailureCleanupZeroTerminalPending(FailureCleanupZeroTerminalBindingProjection),
    FailureCleanupCompleting(CleanupBatchToken),
}

impl WorkStage {
    fn projection(&self) -> WorkStageProjection {
        match self {
            Self::Applying => WorkStageProjection::Applying,
            Self::AwaitingMount => WorkStageProjection::AwaitingMount,
            Self::AwaitingReceipt => WorkStageProjection::AwaitingReceipt,
            Self::ConfirmingMount => WorkStageProjection::ConfirmingMount,
            Self::ReadyToCommit => WorkStageProjection::ReadyToCommit,
            Self::CommitSealed { token } => WorkStageProjection::CommitSealed(token.clone()),
            Self::PostCommitCleanup { outstanding } => {
                WorkStageProjection::PostCommitCleanup(outstanding.clone())
            }
            Self::PostCommitAuthorityAccepted { normal, expected } => {
                WorkStageProjection::PostCommitAuthorityAccepted {
                    normal: normal.clone(),
                    expected: expected.clone(),
                }
            }
            Self::PostCommitAuthorityCleanup { normal, expected } => {
                WorkStageProjection::PostCommitAuthorityCleanup {
                    normal: normal.clone(),
                    expected: expected.clone(),
                }
            }
            Self::FailureCleanupManifestPending { expected } => {
                WorkStageProjection::FailureCleanupManifestPending(expected.clone())
            }
            Self::FailureCleanupPrepared {
                outstanding,
                failure,
                continuation: _,
            } => WorkStageProjection::FailureCleanupPrepared {
                outstanding: outstanding.clone(),
                failure: failure.clone(),
            },
            Self::FailureCleanupZeroTerminalPending { expected } => {
                WorkStageProjection::FailureCleanupZeroTerminalPending(expected.clone())
            }
            Self::FailureCleanupCompleting { outstanding } => {
                WorkStageProjection::FailureCleanupCompleting(outstanding.clone())
            }
        }
    }

    fn clone_pre_retirement(&self) -> Option<Self> {
        match self {
            Self::Applying => Some(Self::Applying),
            Self::AwaitingMount => Some(Self::AwaitingMount),
            Self::AwaitingReceipt => Some(Self::AwaitingReceipt),
            Self::ConfirmingMount => Some(Self::ConfirmingMount),
            Self::ReadyToCommit => Some(Self::ReadyToCommit),
            Self::CommitSealed { token } => Some(Self::CommitSealed {
                token: token.clone(),
            }),
            Self::PostCommitCleanup { outstanding } => Some(Self::PostCommitCleanup {
                outstanding: outstanding.clone(),
            }),
            Self::PostCommitAuthorityAccepted { .. }
            | Self::PostCommitAuthorityCleanup { .. }
            | Self::FailureCleanupManifestPending { .. }
            | Self::FailureCleanupPrepared { .. }
            | Self::FailureCleanupZeroTerminalPending { .. }
            | Self::FailureCleanupCompleting { .. } => None,
        }
    }
}

pub(crate) struct FailureCleanupCompletionOwner {
    router: RouterIdentity,
    identity: FrameTaskIdentity,
    outstanding: CleanupBatchToken,
    failure: NavigationFailure,
    continuation: FailureCleanupContinuationOwner,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FailureCleanupCompletionDisposition {
    Settled,
    RetainedPreparedPrefixBound,
    RetainedPreparedPrefixCleanup,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BoundErrorBeginDisposition {
    Bound,
    Cleanup,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FailureCleanupCompletionReceiverProjection {
    router: RouterIdentity,
    identity: Option<FrameTaskIdentity>,
    stage: Option<WorkStageProjection>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FailureCleanupCompletionReceiverMismatchProjection {
    expected: FailureCleanupCompletionReceiverProjection,
    observed: FailureCleanupCompletionReceiverProjection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum FailureCleanupCompletionPreparationStageProjection {
    Missing,
    FailureCleanupPrepared {
        outstanding: CleanupBatchToken,
        authority_settlement: bool,
    },
    Other(WorkStageProjection),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FailureCleanupCompletionPreparationReceiverProjection {
    router: RouterIdentity,
    identity: Option<FrameTaskIdentity>,
    stage: FailureCleanupCompletionPreparationStageProjection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FailureCleanupCompletionPreparationMismatchProjection {
    expected: FailureCleanupCompletionPreparationReceiverProjection,
    observed: FailureCleanupCompletionPreparationReceiverProjection,
}

pub(crate) struct FailureCleanupCompletionPrepareError {
    kind: CleanupError,
    mismatch: FailureCleanupCompletionPreparationMismatchProjection,
}

impl FailureCleanupCompletionPrepareError {
    const fn new(
        kind: CleanupError,
        mismatch: FailureCleanupCompletionPreparationMismatchProjection,
    ) -> Self {
        Self { kind, mismatch }
    }

    pub(crate) const fn kind(&self) -> CleanupError {
        self.kind
    }

    pub(crate) fn into_mismatch(self) -> FailureCleanupCompletionPreparationMismatchProjection {
        self.mismatch
    }
}

impl std::fmt::Debug for FailureCleanupCompletionPrepareError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FailureCleanupCompletionPrepareError")
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}

pub(crate) struct FailureCleanupCompletionCommitError<Physical> {
    kind: CleanupError,
    owner: FailureCleanupCompletionOwner,
    physical: Physical,
    mismatch: FailureCleanupCompletionReceiverMismatchProjection,
}

impl<Physical> FailureCleanupCompletionCommitError<Physical> {
    const fn new(
        kind: CleanupError,
        owner: FailureCleanupCompletionOwner,
        physical: Physical,
        mismatch: FailureCleanupCompletionReceiverMismatchProjection,
    ) -> Self {
        Self {
            kind,
            owner,
            physical,
            mismatch,
        }
    }

    pub(crate) const fn kind(&self) -> CleanupError {
        self.kind
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        FailureCleanupCompletionOwner,
        Physical,
        FailureCleanupCompletionReceiverMismatchProjection,
    ) {
        (self.owner, self.physical, self.mismatch)
    }
}

impl<Physical> std::fmt::Debug for FailureCleanupCompletionCommitError<Physical> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FailureCleanupCompletionCommitError")
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CleanupBatchProjection {
    identity: RootTaskIdentity,
    batch: CleanupBatchId,
    manifest_digest: CleanupManifestDigest,
    obligation_count: usize,
}

impl CleanupBatchProjection {
    pub(crate) fn from_token(token: &CleanupBatchToken) -> Self {
        Self {
            identity: token.identity().clone(),
            batch: token.batch(),
            manifest_digest: token.manifest_digest(),
            obligation_count: token.obligation_count(),
        }
    }

    fn matches(&self, token: &CleanupBatchToken) -> bool {
        self == &Self::from_token(token)
    }

    pub(crate) fn manifest_digest_matches(&self, value: u64) -> bool {
        self.manifest_digest == CleanupManifestDigest::from_opaque(value)
    }

    pub(crate) const fn obligation_count(&self) -> usize {
        self.obligation_count
    }
}

impl CleanupBatchToken {
    pub(crate) fn projection(&self) -> CleanupBatchProjection {
        CleanupBatchProjection::from_token(self)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CorePostCommitCleanupStageProjection {
    PostCommitCleanup,
    AuthorityAccepted,
    AuthorityCleanup,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FrozenCommittedHistoryProjection {
    entries: Vec<RouteLocation>,
    cursor: Option<usize>,
    digest: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FrozenActiveBranchProjection {
    identity: RootTaskIdentity,
    presentation: ActivePresentation,
    frames: Vec<RouteFrameVersion>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CorePostCommitCleanupReceiverProjection {
    router: RouterIdentity,
    request: NavigationRequestId,
    identity: RootTaskIdentity,
    normal_batch: CleanupBatchProjection,
    current_host: HostVersion,
    current_top_slot: RouteSlotId,
    committed_history: FrozenCommittedHistoryProjection,
    active: FrozenActiveBranchProjection,
    stage: CorePostCommitCleanupStageProjection,
}

impl CorePostCommitCleanupReceiverProjection {
    pub(crate) const fn request(&self) -> NavigationRequestId {
        self.request
    }

    pub(crate) const fn normal_batch(&self) -> &CleanupBatchProjection {
        &self.normal_batch
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CoreCompetingCleanupOwnerStage {
    Prepared,
    Active,
    Completing,
    Inert,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CoreCompetingCleanupOwnerProjection {
    identity: RootTaskIdentity,
    batch: CleanupBatchProjection,
    stage: CoreCompetingCleanupOwnerStage,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CorePostCommitCleanupObservedProjection {
    receiver: Option<CorePostCommitCleanupReceiverProjection>,
    retired_host: Option<RetiredHostAuthorityProjection>,
    competing_cleanup: Option<CoreCompetingCleanupOwnerProjection>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CorePostCommitCompoundProjection {
    receiver: CorePostCommitCleanupReceiverProjection,
    authority_batch: CleanupBatchProjection,
    retired_host: RetiredHostAuthorityProjection,
    cause: PostCommitAuthorityCause,
}

impl CorePostCommitCompoundProjection {
    pub(crate) const fn receiver(&self) -> &CorePostCommitCleanupReceiverProjection {
        &self.receiver
    }

    pub(crate) const fn authority_batch(&self) -> &CleanupBatchProjection {
        &self.authority_batch
    }

    pub(crate) const fn cause(&self) -> PostCommitAuthorityCause {
        self.cause
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CorePostCommitReceiverMismatchProjection {
    expected: CorePostCommitCleanupReceiverProjection,
    observed: CorePostCommitCleanupObservedProjection,
}

impl CorePostCommitReceiverMismatchProjection {
    pub(crate) const fn expected(&self) -> &CorePostCommitCleanupReceiverProjection {
        &self.expected
    }

    pub(crate) const fn observed(&self) -> &CorePostCommitCleanupObservedProjection {
        &self.observed
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PostCommitAuthorityCause {
    HostRegistrationLost,
    HostEntityMissing,
    RequiredResourcesLost,
}

impl From<PostCommitAuthorityCause> for PostCommitCleanupAnomalyCause {
    fn from(cause: PostCommitAuthorityCause) -> Self {
        match cause {
            PostCommitAuthorityCause::HostRegistrationLost => Self::HostRegistrationLost,
            PostCommitAuthorityCause::HostEntityMissing => Self::HostEntityMissing,
            PostCommitAuthorityCause::RequiredResourcesLost => Self::RequiredResourcesLost,
        }
    }
}

impl From<PostCommitCleanupAnomalyCause> for PostCommitAuthorityCause {
    fn from(cause: PostCommitCleanupAnomalyCause) -> Self {
        match cause {
            PostCommitCleanupAnomalyCause::HostRegistrationLost => Self::HostRegistrationLost,
            PostCommitCleanupAnomalyCause::HostEntityMissing => Self::HostEntityMissing,
            PostCommitCleanupAnomalyCause::RequiredResourcesLost => Self::RequiredResourcesLost,
        }
    }
}

pub(crate) struct CorePostCommitAuthorityTransferOwner {
    receiver: CorePostCommitCleanupReceiverProjection,
    normal_source: CleanupBatchSourceProjection,
    cause: PostCommitAuthorityCause,
}

impl CorePostCommitAuthorityTransferOwner {
    pub(crate) const fn receiver(&self) -> &CorePostCommitCleanupReceiverProjection {
        &self.receiver
    }

    pub(crate) const fn cause(&self) -> PostCommitAuthorityCause {
        self.cause
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CoreRetiredHostAuthorityExpectedProjection {
    receiver: CorePostCommitCleanupReceiverProjection,
    retired_host: RetiredHostAuthorityProjection,
    cause: PostCommitAuthorityCause,
}

impl CoreRetiredHostAuthorityExpectedProjection {
    pub(crate) const fn cause(&self) -> PostCommitAuthorityCause {
        self.cause
    }
}

#[derive(Debug)]
pub(crate) struct CoreRetiredHostAuthorityTerminalEvidence {
    expected: CoreRetiredHostAuthorityExpectedProjection,
    consumed: RetiredHostAuthorityProjection,
}

impl CoreRetiredHostAuthorityTerminalEvidence {
    pub(crate) const fn cause(&self) -> PostCommitAuthorityCause {
        self.expected.cause
    }
}

pub(crate) struct CoreRetiredHostEdgeSettlementOwner {
    authority: RetiredHostAuthority,
    expected: CoreRetiredHostAuthorityExpectedProjection,
    expected_physical_digest: u64,
}

pub(crate) struct CorePostCommitAuthorityPreparationSource {
    transfer: CorePostCommitAuthorityTransferOwner,
    host: HostLease,
}

impl CorePostCommitAuthorityPreparationSource {
    pub(crate) const fn new(
        transfer: CorePostCommitAuthorityTransferOwner,
        host: HostLease,
    ) -> Self {
        Self { transfer, host }
    }

    pub(crate) fn into_parts(self) -> (CorePostCommitAuthorityTransferOwner, HostLease) {
        (self.transfer, self.host)
    }

    pub(crate) const fn cause(&self) -> PostCommitAuthorityCause {
        self.transfer.cause
    }
}

pub(crate) struct CorePreparedPostCommitAuthorityTransfer {
    transfer: CorePostCommitAuthorityTransferOwner,
    host: HostLease,
    authority_batch: CleanupBatchToken,
    authority_manifest: CleanupManifestSeal,
}

impl CorePreparedPostCommitAuthorityTransfer {
    pub(crate) fn batch_projection(&self) -> CleanupBatchProjection {
        self.authority_batch.projection()
    }

    pub(crate) fn into_source(self) -> CorePostCommitAuthorityPreparationSource {
        CorePostCommitAuthorityPreparationSource {
            transfer: self.transfer,
            host: self.host,
        }
    }

    pub(crate) const fn cause(&self) -> PostCommitAuthorityCause {
        self.transfer.cause
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CorePostCommitCapacityKind {
    PreparedTrace,
    PreparedTerminal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CorePostCommitAuthorityContradiction {
    ForeignReceiver,
    CompetingSameSourceOwner,
    RetiredHostMismatch,
    HistoryOrActiveMismatch,
    IdentityExhausted,
    RequestOrQueueMismatch,
    TraceMismatch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CorePostCommitTerminalPreparationFailure {
    Retryable(CorePostCommitCapacityKind),
    Permanent(CorePostCommitAuthorityContradiction),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CorePostCommitAuthorityBeginRetryEvidence {
    expected: CorePostCommitCleanupReceiverProjection,
    observed: CorePostCommitCleanupObservedProjection,
    cause: PostCommitAuthorityCause,
    missing_capacity: CorePostCommitCapacityKind,
}

impl CorePostCommitAuthorityBeginRetryEvidence {
    pub(crate) const fn missing_capacity(&self) -> CorePostCommitCapacityKind {
        self.missing_capacity
    }

    pub(crate) const fn cause(&self) -> PostCommitAuthorityCause {
        self.cause
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CorePostCommitAuthorityBeginPermanentEvidence {
    expected: CorePostCommitCleanupReceiverProjection,
    observed: CorePostCommitCleanupObservedProjection,
    cause: PostCommitAuthorityCause,
    contradiction: CorePostCommitAuthorityContradiction,
}

impl CorePostCommitAuthorityBeginPermanentEvidence {
    pub(crate) const fn contradiction(&self) -> CorePostCommitAuthorityContradiction {
        self.contradiction
    }

    pub(crate) const fn cause(&self) -> PostCommitAuthorityCause {
        self.cause
    }
}

pub(crate) enum PostCommitAuthorityAcceptanceOutcome {
    Accepted(CorePostCommitAuthorityTransferOwner),
    Mismatch {
        expected: CorePostCommitCleanupReceiverProjection,
        observed: CorePostCommitCleanupObservedProjection,
    },
}

pub(crate) enum PostCommitAuthorityPreparationOutcome {
    Prepared {
        owner: CorePreparedPostCommitAuthorityTransfer,
        batch: CleanupBatchProjection,
    },
    Mismatch {
        source: CorePostCommitAuthorityPreparationSource,
        evidence: CorePostCommitReceiverMismatchProjection,
    },
    Retryable {
        source: CorePostCommitAuthorityPreparationSource,
        evidence: CorePostCommitAuthorityBeginRetryEvidence,
    },
    Permanent {
        source: CorePostCommitAuthorityPreparationSource,
        evidence: CorePostCommitAuthorityBeginPermanentEvidence,
    },
}

pub(crate) enum PostCommitAuthorityBeginCoreOutcome<Unbound, Bound, BindError> {
    Begun,
    BindingMismatch {
        owner: CorePreparedPostCommitAuthorityTransfer,
        unbound: Unbound,
        evidence: BindError,
    },
    ReceiverMismatch {
        owner: CorePreparedPostCommitAuthorityTransfer,
        bound: Bound,
        evidence: CorePostCommitReceiverMismatchProjection,
    },
    Retryable {
        owner: CorePreparedPostCommitAuthorityTransfer,
        bound: Bound,
        evidence: CorePostCommitAuthorityBeginRetryEvidence,
    },
    Permanent {
        owner: CorePreparedPostCommitAuthorityTransfer,
        bound: Bound,
        evidence: CorePostCommitAuthorityBeginPermanentEvidence,
    },
}

pub(crate) struct CorePostCommitCompoundBatchOwner {
    receiver: CorePostCommitCompoundProjection,
    authority_batch: CleanupBatchToken,
    host_edge: CoreRetiredHostEdgeSettlementOwner,
    settlement: Option<CorePostCommitSettlementCapability>,
}

impl CorePostCommitCompoundBatchOwner {
    pub(crate) fn projection(&self) -> CorePostCommitCompoundProjection {
        self.receiver.clone()
    }

    pub(crate) fn authority_batch_projection(&self) -> CleanupBatchProjection {
        self.authority_batch.projection()
    }

    pub(crate) fn host_expected_projection(&self) -> CoreRetiredHostAuthorityExpectedProjection {
        self.host_edge.expected.clone()
    }

    pub(crate) const fn expected_host_physical_digest(&self) -> u64 {
        self.host_edge.expected_physical_digest
    }

    pub(crate) const fn cause(&self) -> PostCommitAuthorityCause {
        self.receiver.cause
    }

    pub(crate) fn settlement_projection(&self) -> CorePostCommitSettlementTraceProjection {
        self.settlement
            .as_ref()
            .expect("the compound owner is not externally visible while its permit is issued")
            .projection
            .clone()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CoreNormalCleanupCompletionFacts {
    batch: CleanupBatchProjection,
    manifest_digest: CleanupManifestDigest,
    completion_digest: CleanupCompletionDigest,
    settled_count: usize,
}

impl CoreNormalCleanupCompletionFacts {
    pub(crate) const fn new(
        batch: CleanupBatchProjection,
        manifest_digest: CleanupManifestDigest,
        completion_digest: CleanupCompletionDigest,
        settled_count: usize,
    ) -> Self {
        Self {
            batch,
            manifest_digest,
            completion_digest,
            settled_count,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CoreAuthorityCleanupCompletionFacts {
    batch: CleanupBatchProjection,
    completion_digest: CleanupCompletionDigest,
    settled_count: usize,
}

impl CoreAuthorityCleanupCompletionFacts {
    pub(crate) const fn new(
        batch: CleanupBatchProjection,
        completion_digest: CleanupCompletionDigest,
        settled_count: usize,
    ) -> Self {
        Self {
            batch,
            completion_digest,
            settled_count,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CoreRetiredHostEdgeCompletionFacts {
    expected: CoreRetiredHostAuthorityExpectedProjection,
    physical_digest: u64,
}

impl CoreRetiredHostEdgeCompletionFacts {
    pub(crate) const fn new(
        expected: CoreRetiredHostAuthorityExpectedProjection,
        physical_digest: u64,
    ) -> Self {
        Self {
            expected,
            physical_digest,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CorePostCommitCompoundCompletionFactsProjection {
    cause: PostCommitAuthorityCause,
    normal: CoreNormalCleanupCompletionFacts,
    authority: CoreAuthorityCleanupCompletionFacts,
    host_edge: CoreRetiredHostEdgeCompletionFacts,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CorePostCommitSettlementKeyProjection {
    namespace: u64,
    identity: Vec<u64>,
}

impl CorePostCommitSettlementKeyProjection {
    pub(crate) fn new(namespace: u64, identity: Vec<u64>) -> Self {
        Self {
            namespace,
            identity,
        }
    }

    pub(crate) const fn namespace(&self) -> u64 {
        self.namespace
    }

    pub(crate) fn identity(&self) -> &[u64] {
        &self.identity
    }

    fn is_empty(&self) -> bool {
        self.identity.is_empty()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CorePostCommitSettlementRole {
    OriginalResidual,
    AuthorityNonTop,
    RetiredHostEdge,
    AuthorityTop,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CorePostCommitPlannedCleanupProjection {
    obligation: CorePostCommitSettlementKeyProjection,
    physical_source: CorePostCommitSettlementKeyProjection,
    disposition: crate::trace::CleanupDisposition,
}

impl CorePostCommitPlannedCleanupProjection {
    pub(crate) fn new(
        obligation: CorePostCommitSettlementKeyProjection,
        physical_source: CorePostCommitSettlementKeyProjection,
        disposition: crate::trace::CleanupDisposition,
    ) -> Self {
        Self {
            obligation,
            physical_source,
            disposition,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CorePostCommitSettledOriginalProjection {
    obligation: CorePostCommitSettlementKeyProjection,
    receipt: CorePostCommitSettlementKeyProjection,
    disposition: crate::trace::CleanupDisposition,
}

impl CorePostCommitSettledOriginalProjection {
    pub(crate) fn new(
        obligation: CorePostCommitSettlementKeyProjection,
        receipt: CorePostCommitSettlementKeyProjection,
        disposition: crate::trace::CleanupDisposition,
    ) -> Self {
        Self {
            obligation,
            receipt,
            disposition,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CorePostCommitPlannedHostEdgeProjection {
    obligation: CorePostCommitSettlementKeyProjection,
    physical_source: CorePostCommitSettlementKeyProjection,
}

impl CorePostCommitPlannedHostEdgeProjection {
    pub(crate) fn new(
        obligation: CorePostCommitSettlementKeyProjection,
        physical_source: CorePostCommitSettlementKeyProjection,
    ) -> Self {
        Self {
            obligation,
            physical_source,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CorePostCommitSettlementPlanDescriptor {
    prior_trace_start: usize,
    settled_original: Vec<CorePostCommitSettledOriginalProjection>,
    residual_original: Vec<CorePostCommitPlannedCleanupProjection>,
    authority_non_top: Vec<CorePostCommitPlannedCleanupProjection>,
    authority_top: CorePostCommitPlannedCleanupProjection,
    authority_top_dependency: CorePostCommitSettlementKeyProjection,
    host_edge: CorePostCommitPlannedHostEdgeProjection,
}

impl CorePostCommitSettlementPlanDescriptor {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        prior_trace_start: usize,
        settled_original: Vec<CorePostCommitSettledOriginalProjection>,
        residual_original: Vec<CorePostCommitPlannedCleanupProjection>,
        authority_non_top: Vec<CorePostCommitPlannedCleanupProjection>,
        authority_top: CorePostCommitPlannedCleanupProjection,
        authority_top_dependency: CorePostCommitSettlementKeyProjection,
        host_edge: CorePostCommitPlannedHostEdgeProjection,
    ) -> Self {
        Self {
            prior_trace_start,
            settled_original,
            residual_original,
            authority_non_top,
            authority_top,
            authority_top_dependency,
            host_edge,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CorePostCommitSettlementPlanMismatchKind {
    OriginalCoverage,
    AuthorityCoverage,
    DuplicateKey,
    EmptyKey,
    PriorTrace,
    WrongSource,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CorePostCommitSettlementPlanMismatchEvidence {
    kind: CorePostCommitSettlementPlanMismatchKind,
    descriptor: CorePostCommitSettlementPlanDescriptor,
}

impl CorePostCommitSettlementPlanMismatchEvidence {
    pub(crate) const fn kind(&self) -> CorePostCommitSettlementPlanMismatchKind {
        self.kind
    }
}

pub(crate) struct CorePostCommitSettlementPlanBuilder {
    compound: CorePostCommitCompoundProjection,
    normal_batch: CleanupBatchSourceProjection,
    authority_batch: CleanupBatchSourceProjection,
    normal_obligation_count: usize,
    authority_obligation_count: usize,
    current_trace: Vec<NavigationTraceEvent>,
}

pub(crate) struct CoreBoundPostCommitSettlementPlan {
    compound: CorePostCommitCompoundProjection,
    normal_batch: CleanupBatchSourceProjection,
    authority_batch: CleanupBatchSourceProjection,
    descriptor: CorePostCommitSettlementPlanDescriptor,
    prior_trace_prefix_digest: u64,
    prior_trace_digest: u64,
}

impl CorePostCommitSettlementPlanBuilder {
    pub(crate) fn current_trace_len(&self) -> usize {
        self.current_trace.len()
    }

    pub(crate) fn bind(
        self,
        descriptor: CorePostCommitSettlementPlanDescriptor,
    ) -> Result<CoreBoundPostCommitSettlementPlan, CorePostCommitSettlementPlanMismatchEvidence>
    {
        let original_coverage = descriptor.settled_original.len()
            + descriptor.residual_original.len()
            == self.normal_obligation_count;
        let authority_coverage =
            descriptor.authority_non_top.len() + 1 == self.authority_obligation_count;
        let all_keys = descriptor
            .settled_original
            .iter()
            .flat_map(|entry| [&entry.obligation, &entry.receipt])
            .chain(
                descriptor
                    .residual_original
                    .iter()
                    .flat_map(|entry| [&entry.obligation, &entry.physical_source]),
            )
            .chain(
                descriptor
                    .authority_non_top
                    .iter()
                    .flat_map(|entry| [&entry.obligation, &entry.physical_source]),
            )
            .chain([
                &descriptor.authority_top.obligation,
                &descriptor.authority_top.physical_source,
                &descriptor.authority_top_dependency,
                &descriptor.host_edge.obligation,
                &descriptor.host_edge.physical_source,
            ])
            .collect::<Vec<_>>();
        let empty_key = all_keys.iter().any(|key| key.is_empty());
        let exact_role_keys = descriptor
            .settled_original
            .iter()
            .flat_map(|entry| [&entry.obligation, &entry.receipt])
            .chain(
                descriptor
                    .residual_original
                    .iter()
                    .flat_map(|entry| [&entry.obligation, &entry.physical_source]),
            )
            .chain(
                descriptor
                    .authority_non_top
                    .iter()
                    .flat_map(|entry| [&entry.obligation, &entry.physical_source]),
            )
            .chain([
                &descriptor.authority_top.obligation,
                &descriptor.authority_top.physical_source,
                &descriptor.host_edge.obligation,
                &descriptor.host_edge.physical_source,
            ])
            .collect::<Vec<_>>();
        let duplicate_role_key = exact_role_keys.iter().enumerate().any(|(index, key)| {
            exact_role_keys
                .iter()
                .skip(index + 1)
                .any(|other| other == key)
        });
        let duplicate_original_obligation = descriptor
            .settled_original
            .iter()
            .map(|entry| &entry.obligation)
            .chain(
                descriptor
                    .residual_original
                    .iter()
                    .map(|entry| &entry.obligation),
            )
            .enumerate()
            .any(|(index, key)| {
                descriptor
                    .settled_original
                    .iter()
                    .map(|entry| &entry.obligation)
                    .chain(
                        descriptor
                            .residual_original
                            .iter()
                            .map(|entry| &entry.obligation),
                    )
                    .skip(index + 1)
                    .any(|other| other == key)
            });
        let duplicate_settled_receipt = descriptor
            .settled_original
            .iter()
            .map(|entry| &entry.receipt)
            .enumerate()
            .any(|(index, key)| {
                descriptor
                    .settled_original
                    .iter()
                    .map(|entry| &entry.receipt)
                    .skip(index + 1)
                    .any(|other| other == key)
            });
        let duplicate_residual_source = descriptor
            .residual_original
            .iter()
            .map(|entry| &entry.physical_source)
            .enumerate()
            .any(|(index, key)| {
                descriptor
                    .residual_original
                    .iter()
                    .map(|entry| &entry.physical_source)
                    .skip(index + 1)
                    .any(|other| other == key)
            });
        let duplicate_authority_obligation = descriptor
            .authority_non_top
            .iter()
            .map(|entry| &entry.obligation)
            .chain([&descriptor.authority_top.obligation])
            .enumerate()
            .any(|(index, key)| {
                descriptor
                    .authority_non_top
                    .iter()
                    .map(|entry| &entry.obligation)
                    .chain([&descriptor.authority_top.obligation])
                    .skip(index + 1)
                    .any(|other| other == key)
            });
        let duplicate_authority_source = descriptor
            .authority_non_top
            .iter()
            .map(|entry| &entry.physical_source)
            .chain([&descriptor.authority_top.physical_source])
            .enumerate()
            .any(|(index, key)| {
                descriptor
                    .authority_non_top
                    .iter()
                    .map(|entry| &entry.physical_source)
                    .chain([&descriptor.authority_top.physical_source])
                    .skip(index + 1)
                    .any(|other| other == key)
            });
        let prior_trace = self
            .current_trace
            .get(descriptor.prior_trace_start..)
            .filter(|events| events.len() == descriptor.settled_original.len())
            .is_some_and(|events| {
                events
                    .iter()
                    .zip(&descriptor.settled_original)
                    .all(|(event, settled)| {
                        event == &NavigationTraceEvent::cleanup_settled(Some(settled.disposition))
                    })
            });
        let mismatch = if !original_coverage {
            Some(CorePostCommitSettlementPlanMismatchKind::OriginalCoverage)
        } else if !authority_coverage {
            Some(CorePostCommitSettlementPlanMismatchKind::AuthorityCoverage)
        } else if empty_key {
            Some(CorePostCommitSettlementPlanMismatchKind::EmptyKey)
        } else if duplicate_role_key
            || duplicate_original_obligation
            || duplicate_settled_receipt
            || duplicate_residual_source
            || duplicate_authority_obligation
            || duplicate_authority_source
        {
            Some(CorePostCommitSettlementPlanMismatchKind::DuplicateKey)
        } else if descriptor.authority_top_dependency != descriptor.host_edge.obligation {
            Some(CorePostCommitSettlementPlanMismatchKind::WrongSource)
        } else if !prior_trace {
            Some(CorePostCommitSettlementPlanMismatchKind::PriorTrace)
        } else {
            None
        };
        if let Some(kind) = mismatch {
            return Err(CorePostCommitSettlementPlanMismatchEvidence { kind, descriptor });
        }
        Ok(CoreBoundPostCommitSettlementPlan {
            compound: self.compound,
            normal_batch: self.normal_batch,
            authority_batch: self.authority_batch,
            prior_trace_prefix_digest: navigation_trace_digest(
                &self.current_trace[..descriptor.prior_trace_start],
            ),
            prior_trace_digest: navigation_trace_digest(&self.current_trace),
            descriptor,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CorePostCommitSettlementPlanBindingProjection {
    compound: CorePostCommitCompoundProjection,
    normal_batch: CleanupBatchSourceProjection,
    authority_batch: CleanupBatchSourceProjection,
    prior_trace_start: usize,
    prior_trace_prefix_digest: u64,
    prior_trace_digest: u64,
    settled_original: Vec<CorePostCommitSettledOriginalProjection>,
    authority_top_dependency: CorePostCommitSettlementKeyProjection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CorePostCommitSettlementExpectedProjection {
    ordinal: usize,
    role: CorePostCommitSettlementRole,
    obligation: CorePostCommitSettlementKeyProjection,
    physical_source: CorePostCommitSettlementKeyProjection,
    disposition: Option<crate::trace::CleanupDisposition>,
}

impl CorePostCommitSettlementExpectedProjection {
    pub(crate) const fn role(&self) -> CorePostCommitSettlementRole {
        self.role
    }

    pub(crate) const fn obligation(&self) -> &CorePostCommitSettlementKeyProjection {
        &self.obligation
    }

    pub(crate) const fn physical_source(&self) -> &CorePostCommitSettlementKeyProjection {
        &self.physical_source
    }

    pub(crate) const fn disposition(&self) -> Option<crate::trace::CleanupDisposition> {
        self.disposition
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CorePostCommitSettlementTraceProgress {
    OriginalResidual,
    AuthorityNonTop,
    RetiredHostEdge,
    AuthorityTop,
    Complete,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CorePostCommitSettlementTraceProjection {
    binding: CorePostCommitSettlementPlanBindingProjection,
    expected: Vec<CorePostCommitSettlementExpectedProjection>,
    cursor: usize,
    recorded: Vec<CorePostCommitSettlementExpectedProjection>,
    settlement_event_start: usize,
    digest: u64,
}

impl CorePostCommitSettlementTraceProjection {
    pub(crate) const fn cause(&self) -> PostCommitAuthorityCause {
        self.binding.compound.cause
    }

    pub(crate) fn progress(&self) -> CorePostCommitSettlementTraceProgress {
        self.expected.get(self.cursor).map_or(
            CorePostCommitSettlementTraceProgress::Complete,
            |expected| match expected.role {
                CorePostCommitSettlementRole::OriginalResidual => {
                    CorePostCommitSettlementTraceProgress::OriginalResidual
                }
                CorePostCommitSettlementRole::AuthorityNonTop => {
                    CorePostCommitSettlementTraceProgress::AuthorityNonTop
                }
                CorePostCommitSettlementRole::RetiredHostEdge => {
                    CorePostCommitSettlementTraceProgress::RetiredHostEdge
                }
                CorePostCommitSettlementRole::AuthorityTop => {
                    CorePostCommitSettlementTraceProgress::AuthorityTop
                }
            },
        )
    }

    pub(crate) const fn digest(&self) -> u64 {
        self.digest
    }

    fn is_complete(&self) -> bool {
        self.cursor == self.expected.len() && self.recorded == self.expected
    }
}

struct CorePostCommitSettlementCapability {
    projection: CorePostCommitSettlementTraceProjection,
}

impl CorePostCommitSettlementCapability {
    fn from_plan(plan: CoreBoundPostCommitSettlementPlan, settlement_event_start: usize) -> Self {
        let mut expected = Vec::new();
        for cleanup in &plan.descriptor.residual_original {
            expected.push(CorePostCommitSettlementExpectedProjection {
                ordinal: expected.len(),
                role: CorePostCommitSettlementRole::OriginalResidual,
                obligation: cleanup.obligation.clone(),
                physical_source: cleanup.physical_source.clone(),
                disposition: Some(cleanup.disposition),
            });
        }
        for cleanup in &plan.descriptor.authority_non_top {
            expected.push(CorePostCommitSettlementExpectedProjection {
                ordinal: expected.len(),
                role: CorePostCommitSettlementRole::AuthorityNonTop,
                obligation: cleanup.obligation.clone(),
                physical_source: cleanup.physical_source.clone(),
                disposition: Some(cleanup.disposition),
            });
        }
        expected.push(CorePostCommitSettlementExpectedProjection {
            ordinal: expected.len(),
            role: CorePostCommitSettlementRole::RetiredHostEdge,
            obligation: plan.descriptor.host_edge.obligation.clone(),
            physical_source: plan.descriptor.host_edge.physical_source.clone(),
            disposition: None,
        });
        expected.push(CorePostCommitSettlementExpectedProjection {
            ordinal: expected.len(),
            role: CorePostCommitSettlementRole::AuthorityTop,
            obligation: plan.descriptor.authority_top.obligation.clone(),
            physical_source: plan.descriptor.authority_top.physical_source.clone(),
            disposition: Some(plan.descriptor.authority_top.disposition),
        });
        let binding = CorePostCommitSettlementPlanBindingProjection {
            compound: plan.compound,
            normal_batch: plan.normal_batch,
            authority_batch: plan.authority_batch,
            prior_trace_start: plan.descriptor.prior_trace_start,
            prior_trace_prefix_digest: plan.prior_trace_prefix_digest,
            prior_trace_digest: plan.prior_trace_digest,
            settled_original: plan.descriptor.settled_original,
            authority_top_dependency: plan.descriptor.authority_top_dependency,
        };
        let mut projection = CorePostCommitSettlementTraceProjection {
            binding,
            expected,
            cursor: 0,
            recorded: Vec::new(),
            settlement_event_start,
            digest: 0,
        };
        projection.digest = post_commit_settlement_trace_digest(&projection);
        Self { projection }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CorePostCommitSettlementTraceMismatchKind {
    Receiver,
    WrongOwner,
    Replay,
    TraceUnavailable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CorePostCommitSettlementTraceMismatchEvidence {
    expected: Option<CorePostCommitSettlementTraceProjection>,
    observed_receiver: Option<CorePostCommitCompoundProjection>,
    submitted: CorePostCommitCompoundProjection,
    submitted_normal: CleanupBatchSourceProjection,
    submitted_trace: CorePostCommitSettlementTraceProjection,
    expected_next: Option<CorePostCommitSettlementExpectedProjection>,
    submitted_next: Option<CorePostCommitSettlementExpectedProjection>,
    kind: CorePostCommitSettlementTraceMismatchKind,
}

impl CorePostCommitSettlementTraceMismatchEvidence {
    pub(crate) const fn kind(&self) -> CorePostCommitSettlementTraceMismatchKind {
        self.kind
    }

    pub(crate) fn cause(&self) -> PostCommitAuthorityCause {
        self.expected.as_ref().map_or(
            self.submitted.cause,
            CorePostCommitSettlementTraceProjection::cause,
        )
    }

    pub(crate) const fn submitted_normal(&self) -> &CleanupBatchSourceProjection {
        &self.submitted_normal
    }

    pub(crate) const fn expected_next(
        &self,
    ) -> Option<&CorePostCommitSettlementExpectedProjection> {
        self.expected_next.as_ref()
    }

    pub(crate) const fn submitted_trace(&self) -> &CorePostCommitSettlementTraceProjection {
        &self.submitted_trace
    }

    pub(crate) const fn submitted_next(
        &self,
    ) -> Option<&CorePostCommitSettlementExpectedProjection> {
        self.submitted_next.as_ref()
    }
}

pub(crate) struct CorePostCommitSettlementPermit {
    capability: CorePostCommitSettlementCapability,
    expected: CorePostCommitSettlementExpectedProjection,
    binding: u64,
}

impl CorePostCommitSettlementPermit {
    pub(crate) const fn expected(&self) -> &CorePostCommitSettlementExpectedProjection {
        &self.expected
    }
}

pub(crate) struct CorePreparedPostCommitSettlement {
    owner: CorePostCommitCompoundBatchOwner,
    permit: CorePostCommitSettlementPermit,
}

pub(crate) enum CorePostCommitSettlementPermitOutcome {
    Prepared(CorePreparedPostCommitSettlement),
    Mismatch {
        owner: CorePostCommitCompoundBatchOwner,
        evidence: CorePostCommitSettlementTraceMismatchEvidence,
    },
}

pub(crate) struct CorePostCommitSettlementAttestation<Physical> {
    physical: Physical,
    permit_binding: u64,
}

impl<Physical> CorePostCommitSettlementAttestation<Physical> {
    pub(crate) fn from_trusted_physical(
        physical: Physical,
        permit: &CorePostCommitSettlementPermit,
    ) -> Self {
        Self {
            physical,
            permit_binding: permit.binding,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CorePostCommitSettlementAttestationMismatchEvidence {
    expected_permit_binding: u64,
    observed_permit_binding: u64,
}

pub(crate) enum CorePostCommitSettlementRecordOutcome<Physical, BindError> {
    Recorded {
        owner: CorePostCommitCompoundBatchOwner,
        trace: CorePostCommitSettlementTraceProjection,
    },
    Mismatch {
        attempt: CorePreparedPostCommitSettlement,
        physical: Physical,
        evidence: CorePostCommitSettlementTraceMismatchEvidence,
    },
    BindingMismatch {
        attempt: CorePreparedPostCommitSettlement,
        physical: Physical,
        bind_error: BindError,
    },
    AttestationMismatch {
        attempt: CorePreparedPostCommitSettlement,
        physical: Physical,
        evidence: CorePostCommitSettlementAttestationMismatchEvidence,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CorePostCommitSettlementTraceCompletionExpectedProjection {
    compound: CorePostCommitCompoundProjection,
    cause: PostCommitAuthorityCause,
    plan: CorePostCommitSettlementTraceProjection,
    required_progress: CorePostCommitSettlementTraceProgress,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CorePostCommitCurrentSourceProjection {
    identity: FrameTaskIdentity,
    branch: Option<FailureCleanupZeroTerminalBranchProjection>,
    location: RouteLocation,
    reservation: Vec<ReservedFrameAttempt>,
    target_frames: Vec<RouteFrameVersion>,
    applied_frames: Vec<RouteFrameVersion>,
    current_target: FrameMountTarget,
    pending_mount: Option<FailureCleanupZeroTerminalPendingMountProjection>,
    authority_cleanup: Option<CorePostCommitAuthorityCleanupSourceProjection>,
    cleanup_batch: CleanupBatchId,
    cleanup_roots: Vec<RouteFrameVersion>,
    attempt: FailureCleanupZeroTerminalAttemptProjection,
    reconciliation: Option<RouteReconciliationProjection>,
    retired_host: Option<RetiredHostAuthorityProjection>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CorePostCommitAuthorityCleanupSourceProjection {
    normal: CleanupBatchSourceProjection,
    expected: CorePostCommitCompoundProjection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CoreCompletedCleanupSourceProjection {
    request: NavigationRequestId,
    token: CleanupBatchSourceProjection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CorePreparedPostCommitTerminalSourceProjection {
    cause: PostCommitAuthorityCause,
    history_before: FrozenCommittedHistoryProjection,
    recovering_record: NavigationTraceEvent,
    aggregate_cleanup: NavigationTraceEvent,
    diagnostic: DiagnosticEvent,
    request_terminal: NavigationTraceEvent,
    queued_terminals: Vec<NavigationTraceEvent>,
    final_state: crate::presentation::RouterAvailability,
    recovering_state: RouterNavigationState,
    recovering_records: VecDeque<NavigationRequestRecord>,
    final_navigation_state: RouterNavigationState,
    final_records: VecDeque<NavigationRequestRecord>,
    final_evicted_through: Option<u64>,
    final_issued_view_tasks: Vec<FrameTaskIdentity>,
    final_report_dispositions: Vec<ReportDispositionRecord>,
    final_completed_cleanup: Vec<CoreCompletedCleanupSourceProjection>,
    final_router_diagnostics: Vec<DiagnosticEvent>,
    retain_completed_current: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CorePostCommitCompletionSourceProjection {
    receiver: Option<CorePostCommitCompoundProjection>,
    history: FrozenCommittedHistoryProjection,
    state: RouterNavigationState,
    records: VecDeque<NavigationRequestRecord>,
    evicted_through: Option<u64>,
    issued_view_tasks: Vec<FrameTaskIdentity>,
    report_dispositions: Vec<ReportDispositionRecord>,
    completed_cleanup: Vec<CoreCompletedCleanupSourceProjection>,
    router_diagnostics: Vec<DiagnosticEvent>,
    active_branch: Option<FailureCleanupZeroTerminalBranchProjection>,
    active_child_slots: Vec<LogicalChildSlotBinding>,
    current: Option<CorePostCommitCurrentSourceProjection>,
    action_present: bool,
    terminal_plan: Option<CorePreparedPostCommitTerminalSourceProjection>,
    settlement_trace: Option<CorePostCommitSettlementTraceProjection>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CorePostCommitCompoundMismatchEvidence {
    Receiver {
        expected: CorePostCommitCompoundProjection,
        observed: Option<CorePostCommitCompoundProjection>,
    },
    CompletionFacts {
        expected: CorePostCommitCompoundCompletionFactsProjection,
        observed: CorePostCommitCompoundCompletionFactsProjection,
    },
    SettlementTrace {
        expected: CorePostCommitSettlementTraceCompletionExpectedProjection,
        observed: Option<CorePostCommitSettlementTraceProjection>,
    },
    Source {
        expected: CorePostCommitCompletionSourceProjection,
        observed: CorePostCommitCompletionSourceProjection,
    },
}

impl CorePostCommitCompoundMismatchEvidence {
    pub(crate) const fn is_completion_facts(&self) -> bool {
        matches!(self, Self::CompletionFacts { .. })
    }

    pub(crate) const fn is_source(&self) -> bool {
        matches!(self, Self::Source { .. })
    }

    pub(crate) const fn is_settlement_trace(&self) -> bool {
        matches!(self, Self::SettlementTrace { .. })
    }

    pub(crate) fn cause(&self) -> Option<PostCommitAuthorityCause> {
        match self {
            Self::Receiver { expected, .. } => Some(expected.cause),
            Self::CompletionFacts { expected, .. } => Some(expected.cause),
            Self::SettlementTrace { expected, .. } => Some(expected.cause),
            Self::Source { expected, .. } => {
                expected.receiver.as_ref().map(|receiver| receiver.cause)
            }
        }
    }
}

pub(crate) struct PostCommitCompoundCompletionInputs {
    normal: CleanupCompletion,
    normal_facts: CoreNormalCleanupCompletionFacts,
    authority_expected: CoreAuthorityCleanupCompletionFacts,
    authority: CoreAuthorityCleanupCompletionFacts,
    host_edge: CoreRetiredHostEdgeCompletionFacts,
    settlement: CorePostCommitSettlementTraceProjection,
}

impl PostCommitCompoundCompletionInputs {
    pub(crate) const fn new(
        normal: CleanupCompletion,
        normal_facts: CoreNormalCleanupCompletionFacts,
        authority_expected: CoreAuthorityCleanupCompletionFacts,
        authority: CoreAuthorityCleanupCompletionFacts,
        host_edge: CoreRetiredHostEdgeCompletionFacts,
        settlement: CorePostCommitSettlementTraceProjection,
    ) -> Self {
        Self {
            normal,
            normal_facts,
            authority_expected,
            authority,
            host_edge,
            settlement,
        }
    }
}

pub(crate) struct CorePostCommitCompoundCompletionOwner {
    batch: CorePostCommitCompoundBatchOwner,
    normal: CleanupCompletion,
    authority: CoreAuthorityCleanupCompletionFacts,
    host_edge: CoreRetiredHostEdgeCompletionFacts,
    source: CorePostCommitCompletionSourceProjection,
}

pub(crate) enum PostCommitCompoundCompletionPreparationOutcome {
    Prepared(CorePostCommitCompoundCompletionOwner),
    Mismatch {
        batch: CorePostCommitCompoundBatchOwner,
        inputs: PostCommitCompoundCompletionInputs,
        evidence: CorePostCommitCompoundMismatchEvidence,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CorePostCommitTerminalProjection {
    router: RouterIdentity,
    request: NavigationRequestId,
    cause: PostCommitAuthorityCause,
    history: FrozenCommittedHistoryProjection,
    availability: crate::presentation::RouterAvailability,
    trace_digest: u64,
}

impl CorePostCommitTerminalProjection {
    pub(crate) const fn request(&self) -> NavigationRequestId {
        self.request
    }

    pub(crate) const fn cause(&self) -> PostCommitAuthorityCause {
        self.cause
    }

    pub(crate) const fn trace_digest(&self) -> u64 {
        self.trace_digest
    }
}

pub(crate) enum PostCommitCompoundCompletionCoreOutcome<PhysicalTerminal> {
    Terminal {
        terminal: CorePostCommitTerminalProjection,
        physical: PhysicalTerminal,
        retired_host: CoreRetiredHostAuthorityTerminalEvidence,
    },
    Mismatch {
        owner: CorePostCommitCompoundCompletionOwner,
        evidence: CorePostCommitCompoundMismatchEvidence,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PreparedPostCommitTerminalPlan {
    cause: PostCommitAuthorityCause,
    history_before: FrozenCommittedHistoryProjection,
    recovering_record: NavigationTraceEvent,
    aggregate_cleanup: NavigationTraceEvent,
    diagnostic: DiagnosticEvent,
    request_terminal: NavigationTraceEvent,
    queued_terminals: Vec<NavigationTraceEvent>,
    final_state: crate::presentation::RouterAvailability,
    recovering_state: RouterNavigationState,
    recovering_records: VecDeque<NavigationRequestRecord>,
    final_navigation_state: RouterNavigationState,
    final_records: VecDeque<NavigationRequestRecord>,
    final_evicted_through: Option<u64>,
    final_issued_view_tasks: Vec<FrameTaskIdentity>,
    final_report_dispositions: Vec<ReportDispositionRecord>,
    final_completed_cleanup: Vec<CompletedCleanupTombstone>,
    final_router_diagnostics: Vec<DiagnosticEvent>,
    retain_completed_current: bool,
}

impl From<&PreparedPostCommitTerminalPlan> for CorePreparedPostCommitTerminalSourceProjection {
    fn from(terminal: &PreparedPostCommitTerminalPlan) -> Self {
        Self {
            cause: terminal.cause,
            history_before: terminal.history_before.clone(),
            recovering_record: terminal.recovering_record.clone(),
            aggregate_cleanup: terminal.aggregate_cleanup.clone(),
            diagnostic: terminal.diagnostic.clone(),
            request_terminal: terminal.request_terminal.clone(),
            queued_terminals: terminal.queued_terminals.clone(),
            final_state: terminal.final_state.clone(),
            recovering_state: terminal.recovering_state.clone(),
            recovering_records: terminal.recovering_records.clone(),
            final_navigation_state: terminal.final_navigation_state.clone(),
            final_records: terminal.final_records.clone(),
            final_evicted_through: terminal.final_evicted_through,
            final_issued_view_tasks: terminal.final_issued_view_tasks.clone(),
            final_report_dispositions: terminal.final_report_dispositions.clone(),
            final_completed_cleanup: terminal
                .final_completed_cleanup
                .iter()
                .map(|tombstone| CoreCompletedCleanupSourceProjection {
                    request: tombstone.request,
                    token: tombstone.token.source_projection(),
                })
                .collect(),
            final_router_diagnostics: terminal.final_router_diagnostics.clone(),
            retain_completed_current: terminal.retain_completed_current,
        }
    }
}

#[derive(Clone, Debug)]
enum AttemptKind {
    Normal(NavigationCommandKind),
    Fallback {
        original_failure: NavigationFailure,
        bound_error: Option<BoundErrorAttempt>,
    },
}

#[derive(Clone, Debug)]
struct BoundErrorAttempt {
    binding: FrozenErrorViewBinding,
    matched: MatchedRoute,
    active_child_slots: Vec<LogicalChildSlotBinding>,
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
            stage: self.stage.clone_pre_retirement()?,
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
    transaction: TransactionId,
    reserved: ReservedFrameAttempt,
    kind: PendingMountKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PendingMountKind {
    Normal,
    BoundError,
}

fn failure_cleanup_zero_terminal_branch_projection(
    branch: &MatchedBranch,
) -> FailureCleanupZeroTerminalBranchProjection {
    FailureCleanupZeroTerminalBranchProjection {
        routes: branch.routes().to_vec(),
        provenance: branch.provenance().clone(),
        search_fingerprint: branch.search_fingerprint().clone(),
        branch_fingerprint: branch.branch_fingerprint().clone(),
    }
}

impl From<&PendingMount> for FailureCleanupZeroTerminalPendingMountProjection {
    fn from(pending: &PendingMount) -> Self {
        Self {
            child_index: pending.child_index,
            parent: pending.parent.clone(),
            outlet: pending.outlet.clone(),
            from_preserved_parent: pending.from_preserved_parent,
            transaction: pending.transaction,
            reserved: pending.reserved,
            kind: pending.kind,
        }
    }
}

impl From<&BoundErrorAttempt> for FailureCleanupZeroTerminalBoundErrorProjection {
    fn from(bound_error: &BoundErrorAttempt) -> Self {
        Self {
            binding: bound_error.binding.clone(),
            matched: bound_error.matched.clone(),
            active_child_slots: bound_error.active_child_slots.clone(),
        }
    }
}

impl From<&AttemptKind> for FailureCleanupZeroTerminalAttemptProjection {
    fn from(attempt: &AttemptKind) -> Self {
        match attempt {
            AttemptKind::Normal(kind) => Self::Normal(*kind),
            AttemptKind::Fallback {
                original_failure,
                bound_error,
            } => Self::Fallback {
                original_failure: original_failure.clone(),
                bound_error: bound_error.as_ref().map(Into::into),
            },
        }
    }
}

impl From<&CurrentWork> for FailureCleanupZeroTerminalCurrentProjection {
    fn from(current: &CurrentWork) -> Self {
        Self {
            identity: current.identity.clone(),
            branch: current
                .branch
                .as_ref()
                .map(failure_cleanup_zero_terminal_branch_projection),
            location: current.location.clone(),
            reservation: current.reservation.clone(),
            target_frames: current.target_frames.clone(),
            applied_frames: current.applied_frames.clone(),
            current_target: current.current_target.clone(),
            pending_mount: current.pending_mount.as_ref().map(Into::into),
            stage: current.stage.projection(),
            cleanup_batch: current.cleanup_batch,
            cleanup_roots: current.cleanup_roots.clone(),
            attempt: (&current.attempt).into(),
            reconciliation: current.reconciliation.clone(),
            retired_host: current
                .retired_host
                .as_ref()
                .map(RetiredHostAuthority::projection),
        }
    }
}

impl From<&CurrentWork> for CorePostCommitCurrentSourceProjection {
    fn from(current: &CurrentWork) -> Self {
        Self {
            identity: current.identity.clone(),
            branch: current
                .branch
                .as_ref()
                .map(failure_cleanup_zero_terminal_branch_projection),
            location: current.location.clone(),
            reservation: current.reservation.clone(),
            target_frames: current.target_frames.clone(),
            applied_frames: current.applied_frames.clone(),
            current_target: current.current_target.clone(),
            pending_mount: current.pending_mount.as_ref().map(Into::into),
            authority_cleanup: match &current.stage {
                WorkStage::PostCommitAuthorityCleanup { normal, expected } => {
                    Some(CorePostCommitAuthorityCleanupSourceProjection {
                        normal: normal.source_projection(),
                        expected: expected.clone(),
                    })
                }
                _ => None,
            },
            cleanup_batch: current.cleanup_batch,
            cleanup_roots: current.cleanup_roots.clone(),
            attempt: (&current.attempt).into(),
            reconciliation: current.reconciliation.clone(),
            retired_host: current
                .retired_host
                .as_ref()
                .map(RetiredHostAuthority::projection),
        }
    }
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

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct AvailabilityIncarnation(pub(crate) u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HostAdmissionSourceKind {
    InitialAwaiting,
    RetryableHostLost,
    RetryableHostConflict,
    RetryableInvalidHostUiTopology,
    RetryableAuthorityEstablishmentFailed,
    PermanentAuthorityEstablishmentFailed,
    PermanentIdentityExhausted,
    PermanentRegistryInvariantViolation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HostAdmissionSourceProjection {
    pub(crate) router: RouterIdentity,
    pub(crate) availability_incarnation: AvailabilityIncarnation,
    pub(crate) kind: HostAdmissionSourceKind,
    pub(crate) public_reason: Option<RouterUnavailableReason>,
    pub(crate) host: Option<HostVersion>,
    pub(crate) top_slot: Option<RouteSlotId>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct HostAdmissionSourcePermitKey {
    pub(crate) router: RouterIdentity,
    pub(crate) availability_incarnation: AvailabilityIncarnation,
    pub(crate) sequence: u64,
    pub(crate) binding: u64,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct HostAdmissionSourcePermit {
    pub(crate) source: HostAdmissionSourceProjection,
    pub(crate) key: HostAdmissionSourcePermitKey,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HostAdmissionSourceIneligibleProjection {
    pub(crate) expected: HostAdmissionSourceProjection,
    pub(crate) observed: Option<HostAdmissionSourceProjection>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HostGrantProjection {
    pub(crate) source: HostAdmissionSourceProjection,
    pub(crate) permit_key: HostAdmissionSourcePermitKey,
    pub(crate) host: HostVersion,
    pub(crate) top_slot: RouteSlotId,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct SourceBoundHostAdmissionToken {
    pub(crate) permit: HostAdmissionSourcePermit,
    pub(crate) token: HostAdmissionToken,
    issuer: HostAdmissionIssuerProjection,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct SourceBoundHostGrant {
    pub(crate) source: SourceBoundHostAdmissionToken,
    pub(crate) grant: HostGrant,
    issuer: HostAdmissionIssuerProjection,
}

impl SourceBoundHostGrant {
    pub(crate) fn projection(&self) -> HostGrantProjection {
        HostGrantProjection {
            source: self.source.permit.source.clone(),
            permit_key: self.source.permit.key,
            host: self.grant.version().clone(),
            top_slot: self.grant.top_slot(),
        }
    }

    pub(crate) fn expected_issuer_projection(&self) -> HostAdmissionIssuerProjection {
        self.issuer.clone()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CorePendingHostAdmissionStage {
    Permit,
    Token,
    Grant,
    Confirming,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CorePendingHostAdmissionProjection {
    pub(crate) source: HostAdmissionSourceProjection,
    pub(crate) stage: CorePendingHostAdmissionStage,
    pub(crate) permit_key: Option<HostAdmissionSourcePermitKey>,
    pub(crate) host: Option<HostVersion>,
    pub(crate) top_slot: Option<RouteSlotId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HostConfirmationReceiverProjection {
    pub(crate) source: HostAdmissionSourceProjection,
    pub(crate) grant: HostGrantProjection,
    pub(crate) pending: CorePendingHostAdmissionProjection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HostConfirmationReceiverMismatchProjection {
    pub(crate) expected: HostConfirmationReceiverProjection,
    pub(crate) observed: Option<HostConfirmationReceiverProjection>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum CorePendingHostAdmission {
    Permit(CorePendingHostAdmissionProjection),
    Token(CorePendingHostAdmissionProjection),
    Grant(CorePendingHostAdmissionProjection),
    Confirming(HostConfirmationReceiverProjection),
}

impl CorePendingHostAdmission {
    fn projection(&self) -> CorePendingHostAdmissionProjection {
        match self {
            Self::Permit(projection) | Self::Token(projection) | Self::Grant(projection) => {
                projection.clone()
            }
            Self::Confirming(receiver) => receiver.pending.clone(),
        }
    }

    fn confirmation_receiver(&self) -> Option<HostConfirmationReceiverProjection> {
        match self {
            Self::Confirming(receiver) => Some(receiver.clone()),
            Self::Permit(_) | Self::Token(_) | Self::Grant(_) => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HostAdmissionUnavailableEvidenceKind {
    NoCandidate,
    Conflict,
    InvalidTopology,
    HostLost,
    AuthorityEstablishmentFailed,
    RegistryInvariantViolation,
    IssuerOperationFailed,
    ConfirmationFailed,
    PreallocationRejected,
    PostAllocationRejected,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HostAdmissionOperation {
    Request,
    Grant,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HostAdmissionOperationFailureDisposition {
    PermanentAuthorityConflict,
    PermanentForeignOrStale,
    PermanentIdentityExhausted,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum HostAdmissionOperationExhaustionExpectation {
    Forbidden,
    RequiredOneOf(Vec<HostAdmissionAllocatorCounter>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum HostAdmissionOperationFailureContradiction {
    OutstandingWithoutReturnedOwner {
        observed: HostAdmissionIssuerOutstandingProjection,
    },
    ExhaustionPairMismatch,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HostAdmissionOperationFailureProjection {
    pub(crate) source: HostAdmissionSourceProjection,
    pub(crate) operation: HostAdmissionOperation,
    pub(crate) error: AdmissionError,
    pub(crate) disposition: HostAdmissionOperationFailureDisposition,
    pub(crate) expected_issuer: HostAdmissionIssuerProjection,
    pub(crate) observed_issuer: HostAdmissionIssuerProjection,
    pub(crate) exhaustion: Option<HostAdmissionAllocatorExhaustionEvidence>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HostAdmissionOperationFailureEvidence {
    pub(crate) projection: HostAdmissionOperationFailureProjection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HostAdmissionOperationFailureEvidenceMismatch {
    pub(crate) source: HostAdmissionSourceProjection,
    pub(crate) operation: HostAdmissionOperation,
    pub(crate) error: AdmissionError,
    pub(crate) expected_issuer: HostAdmissionIssuerProjection,
    pub(crate) observed_issuer: HostAdmissionIssuerProjection,
    pub(crate) expected_exhaustion: HostAdmissionOperationExhaustionExpectation,
    pub(crate) observed_exhaustion: Option<HostAdmissionAllocatorExhaustionEvidence>,
    pub(crate) contradiction: HostAdmissionOperationFailureContradiction,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum HostAdmissionOperationFailureFreezeOutcome {
    Frozen(HostAdmissionOperationFailureEvidence),
    Mismatch(HostAdmissionOperationFailureEvidenceMismatch),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HostAdmissionUnavailableEvidenceProjection {
    pub(crate) source: HostAdmissionSourceProjection,
    pub(crate) kind: HostAdmissionUnavailableEvidenceKind,
    pub(crate) candidate_revision: Option<u64>,
    pub(crate) expected_digest: u64,
    pub(crate) observed_digest: u64,
    pub(crate) exhaustion: Option<HostAdmissionAllocatorExhaustionEvidence>,
    pub(crate) operation_failure: Option<HostAdmissionOperationFailureProjection>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HostAdmissionPermanence {
    RetryableAfterCandidateChange,
    PermanentForRouter,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum HostAdmissionUnavailableDisposition {
    Awaiting,
    Unavailable {
        reason: RouterUnavailableReason,
        permanence: HostAdmissionPermanence,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HostAdmissionUnavailableCause {
    pub(crate) source: HostAdmissionSourceProjection,
    pub(crate) evidence: HostAdmissionUnavailableEvidenceProjection,
    pub(crate) disposition: HostAdmissionUnavailableDisposition,
    pub(crate) cause_digest: u64,
    pub(crate) expected_issuer: Box<HostAdmissionIssuerProjection>,
    expected_owner_inventory: Box<HostAdmissionUnavailableOwnerInventoryProjection>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct PreparedHostAdmissionUnavailableCause<Evidence> {
    pub(crate) core: HostAdmissionUnavailableCause,
    pub(crate) evidence: Evidence,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum HostAdmissionUnavailableCauseBindOutcome<Evidence> {
    Bound(PreparedHostAdmissionUnavailableCause<Evidence>),
    Mismatch {
        source: HostAdmissionSourceProjection,
        evidence: Evidence,
        projection: HostAdmissionUnavailableEvidenceProjection,
        mismatch: HostAdmissionSourceIneligibleProjection,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HostAdmissionUnavailableTerminalProjection {
    pub(crate) source: HostAdmissionSourceProjection,
    pub(crate) cause_digest: u64,
    pub(crate) reason: RouterUnavailableReason,
    pub(crate) permanence: HostAdmissionPermanence,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HostAdmissionUnavailableMismatchProjection {
    pub(crate) expected_source: HostAdmissionSourceProjection,
    pub(crate) observed_pending: Option<CorePendingHostAdmissionProjection>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HostAdmissionUnavailableObservedMismatchProjection {
    expected_source: HostAdmissionSourceProjection,
    observed_source: HostAdmissionSourceProjection,
    observed_source_open: bool,
    observed_pending: Option<CorePendingHostAdmissionProjection>,
    observed_availability: RouterAvailability,
    observed_host: Option<HostVersion>,
    observed_top_slot: Option<RouteSlotId>,
    observed_active: ActivePresentation,
    observed_request: Option<RequestStatus>,
    observed_current_identity: Option<FrameTaskIdentity>,
    observed_current_stage: Option<WorkStageProjection>,
    observed_action_present: bool,
    expected_owner_inventory: HostAdmissionUnavailableOwnerInventoryProjection,
    observed_owner_inventory: HostAdmissionUnavailableOwnerInventoryProjection,
    expected_issuer: HostAdmissionIssuerProjection,
    observed_issuer: HostAdmissionIssuerProjection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct HostAdmissionUnavailableCurrentOwnerProjection {
    identity: FrameTaskIdentity,
    stage: WorkStageProjection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum HostAdmissionUnavailableActionOwnerProjection {
    FrameTask(FrameTaskIdentity),
    MountRequest {
        request: NavigationRequestId,
        parent: RouteFrameVersion,
        outlet: RouteSlotKey,
    },
    FailureCleanup(NavigationRequestId),
    Commit(FrameTaskIdentity),
    BackResolution,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HostAdmissionUnavailableHistoryPreparationOwnerProjection {
    Source,
    Binding,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum HostAdmissionUnavailablePublicationOwnerProjection {
    Ready {
        identity: FrameTaskIdentity,
        evidence: SealedCommitEvidence,
    },
    Rejected {
        identity: FrameTaskIdentity,
        evidence: SealedCommitEvidence,
        error: CommitError,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum HostAdmissionUnavailableBackNoTargetAuthorityOwnerProjection {
    Accepted {
        receiver: BackResolutionEvidence,
        source: CoreFrozenHistorySourceProjection,
        host: HostVersion,
        top_slot: RouteSlotId,
        logical_roots: Vec<RouteFrameVersion>,
    },
    Cleanup(CoreBackNoTargetAuthorityCleanupProjection),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct HostAdmissionUnavailableOwnerInventoryProjection {
    pending_host_admission: Option<CorePendingHostAdmissionProjection>,
    history_admission_world: Option<CoreIssuedWorldSealProjection>,
    queued_requests: Vec<NavigationRequestId>,
    current: Option<HostAdmissionUnavailableCurrentOwnerProjection>,
    action: Option<HostAdmissionUnavailableActionOwnerProjection>,
    pending_no_target_back: Option<NavigationRequestId>,
    current_history_capability: Option<HistoryExecutionBindingProjection>,
    pending_history_preparation_error:
        Option<HostAdmissionUnavailableHistoryPreparationOwnerProjection>,
    prepared_publication_authority: Option<HostAdmissionUnavailablePublicationOwnerProjection>,
    pending_back_no_target_authority:
        Option<HostAdmissionUnavailableBackNoTargetAuthorityOwnerProjection>,
    pending_back_target_permanent_cleanup: Option<CoreBackTargetPermanentCleanupProjection>,
    pending_post_commit_terminal: Option<CorePreparedPostCommitTerminalSourceProjection>,
    pending_post_commit_settlement_trace: Option<CorePostCommitSettlementTraceProjection>,
    live_failed_authority_trigger_revision: Option<u64>,
    mount_issuer: MountIssuerRequestProjection,
}

impl HostAdmissionUnavailableOwnerInventoryProjection {
    fn is_observed_unavailable_idle(&self) -> bool {
        self.pending_host_admission.is_none()
            && self.queued_requests.is_empty()
            && self.current.is_none()
            && self.action.is_none()
            && self.pending_no_target_back.is_none()
            && self.current_history_capability.is_none()
            && self.pending_history_preparation_error.is_none()
            && self.prepared_publication_authority.is_none()
            && self.pending_back_no_target_authority.is_none()
            && self.pending_back_target_permanent_cleanup.is_none()
            && self.pending_post_commit_terminal.is_none()
            && self.pending_post_commit_settlement_trace.is_none()
            && self.live_failed_authority_trigger_revision.is_none()
            && !self.mount_issuer.has_outstanding()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HostAdmissionUnavailableIssuerOperation {
    RefreezeCause,
    CancelToken,
    RejectGrant,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HostAdmissionUnavailableIssuerMismatchProjection {
    pub(crate) receiver: HostAdmissionUnavailableMismatchProjection,
    pub(crate) operation: HostAdmissionUnavailableIssuerOperation,
    pub(crate) expected_issuer: HostAdmissionIssuerProjection,
    pub(crate) observed_issuer: HostAdmissionIssuerProjection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum HostAdmissionUnavailableFailureEvidence {
    Receiver(HostAdmissionUnavailableMismatchProjection),
    ObservedReceiver(Box<HostAdmissionUnavailableObservedMismatchProjection>),
    Issuer(HostAdmissionUnavailableIssuerMismatchProjection),
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum HostAdmissionUnavailablePreStagingOwner {
    Permit(HostAdmissionSourcePermit),
    Token(SourceBoundHostAdmissionToken),
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct HostAdmissionUnavailablePreStagingSource<Installation, Evidence> {
    pub(crate) prepared: PreparedHostAdmissionUnavailableCause<Evidence>,
    pub(crate) owner: HostAdmissionUnavailablePreStagingOwner,
    pub(crate) installation: Installation,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct HostAdmissionUnavailableGrantedSource<Installation, Evidence> {
    pub(crate) prepared: PreparedHostAdmissionUnavailableCause<Evidence>,
    pub(crate) owner: CoreHostConfirmationSource<Installation>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum HostAdmissionUnavailablePreStagingOutcome<Installation, Evidence, PhysicalTerminal>
{
    Published {
        terminal: HostAdmissionUnavailableTerminalProjection,
        physical: PhysicalTerminal,
    },
    Mismatch {
        source: HostAdmissionUnavailablePreStagingSource<Installation, Evidence>,
        evidence: HostAdmissionUnavailableFailureEvidence,
    },
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum HostAdmissionUnavailableGrantedOutcome<Installation, Evidence, PhysicalTerminal> {
    Published {
        terminal: HostAdmissionUnavailableTerminalProjection,
        physical: PhysicalTerminal,
    },
    Mismatch {
        source: HostAdmissionUnavailableGrantedSource<Installation, Evidence>,
        evidence: HostAdmissionUnavailableFailureEvidence,
    },
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum HostAdmissionUnavailableObservedOutcome<Evidence> {
    Published(HostAdmissionUnavailableTerminalProjection),
    Stable {
        source: HostAdmissionSourceProjection,
        evidence: Evidence,
    },
    Awaiting {
        source: HostAdmissionSourceProjection,
        evidence: Evidence,
    },
    Mismatch {
        prepared: PreparedHostAdmissionUnavailableCause<Evidence>,
        evidence: HostAdmissionUnavailableFailureEvidence,
    },
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum HostAdmissionSourcePermitOutcome {
    Issued(HostAdmissionSourcePermit),
    Ineligible {
        source: HostAdmissionSourceProjection,
        evidence: HostAdmissionSourceIneligibleProjection,
    },
    Outstanding {
        source: HostAdmissionSourceProjection,
    },
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct AdmissionOwnerError<Owner> {
    pub(crate) owner: Owner,
    pub(crate) evidence: HostAdmissionOperationFailureFreezeOutcome,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HostConfirmationIssuerMismatchProjection {
    pub(crate) receiver: HostConfirmationReceiverProjection,
    pub(crate) expected_issuer: HostAdmissionIssuerProjection,
    pub(crate) observed_issuer: HostAdmissionIssuerProjection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum HostAdmissionConfirmationFailureEvidence {
    Receiver(HostConfirmationReceiverMismatchProjection),
    Issuer(HostConfirmationIssuerMismatchProjection),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedHostReadyPublication {
    pub(crate) projection: HostAdmissionReadyProjection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PreparedHostUnavailablePublication {
    terminal: HostAdmissionUnavailableTerminalProjection,
    source: HostAdmissionSourceProjection,
    source_open: bool,
    state: RouterNavigationState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HostAdmissionReadyProjection {
    pub(crate) source: HostAdmissionSourceProjection,
    pub(crate) host: HostVersion,
    pub(crate) top_slot: RouteSlotId,
    pub(crate) availability: RouterAvailability,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct CoreHostConfirmationSource<Installation> {
    pub(crate) grant: SourceBoundHostGrant,
    pub(crate) installation: Installation,
}

impl<Installation> CoreHostConfirmationSource<Installation> {
    pub(crate) fn receiver_projection(&self) -> HostConfirmationReceiverProjection {
        RouterEngine::confirmation_receiver_for(&self.grant)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct CorePreparedHostConfirmation<Installation> {
    pub(crate) grant: SourceBoundHostGrant,
    pub(crate) installation: Installation,
    pub(crate) receiver: HostConfirmationReceiverProjection,
    pub(crate) ready: PreparedHostReadyPublication,
    expected_issuer: HostAdmissionIssuerProjection,
    ready_state: RouterNavigationState,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct RejectedHostConfirmationPreparation<Installation> {
    grant: SourceBoundHostGrant,
    installation: Installation,
    evidence: HostConfirmationReceiverMismatchProjection,
}

impl<Installation> RejectedHostConfirmationPreparation<Installation> {
    pub(crate) fn into_parts(
        self,
    ) -> (
        SourceBoundHostGrant,
        Installation,
        HostConfirmationReceiverMismatchProjection,
    ) {
        (self.grant, self.installation, self.evidence)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum HostAdmissionConfirmationCoreOutcome<Installation, Installed> {
    Ready {
        projection: HostAdmissionReadyProjection,
        installed: Installed,
    },
    ReceiverMismatch {
        owner: CorePreparedHostConfirmation<Installation>,
        evidence: HostConfirmationReceiverMismatchProjection,
    },
    IssuerRejected {
        owner: CorePreparedHostConfirmation<Installation>,
        evidence: HostConfirmationIssuerMismatchProjection,
    },
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum HostConfirmationUnwindOutcome<Installation> {
    Grant(CoreHostConfirmationSource<Installation>),
    Mismatch {
        owner: CorePreparedHostConfirmation<Installation>,
        evidence: HostConfirmationReceiverMismatchProjection,
    },
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum HostConfirmationPreparationOutcome<Installation> {
    Prepared(CorePreparedHostConfirmation<Installation>),
    ReceiverMismatch {
        owner: CoreHostConfirmationSource<Installation>,
        evidence: HostConfirmationReceiverMismatchProjection,
    },
}

pub(crate) fn bind_granted_host_admission_source<Installation>(
    grant: SourceBoundHostGrant,
    installation: Installation,
) -> CoreHostConfirmationSource<Installation> {
    CoreHostConfirmationSource {
        grant,
        installation,
    }
}

pub(crate) fn consume_host_confirmation_preparation_mismatch<Installation>(
    owner: CoreHostConfirmationSource<Installation>,
    evidence: HostConfirmationReceiverMismatchProjection,
) -> RejectedHostConfirmationPreparation<Installation> {
    RejectedHostConfirmationPreparation {
        grant: owner.grant,
        installation: owner.installation,
        evidence,
    }
}

pub(crate) fn freeze_host_admission_operation_failure(
    source: HostAdmissionSourceProjection,
    operation: HostAdmissionOperation,
    error: AdmissionError,
    expected_issuer: HostAdmissionIssuerProjection,
    observed_issuer: HostAdmissionIssuerProjection,
    exhaustion: Option<HostAdmissionAllocatorExhaustionEvidence>,
) -> HostAdmissionOperationFailureFreezeOutcome {
    let expected_exhaustion = match (operation, error) {
        (HostAdmissionOperation::Request, AdmissionError::Exhausted) => {
            HostAdmissionOperationExhaustionExpectation::RequiredOneOf(vec![
                HostAdmissionAllocatorCounter::RequestSequence,
            ])
        }
        (HostAdmissionOperation::Grant, AdmissionError::Exhausted) => {
            HostAdmissionOperationExhaustionExpectation::RequiredOneOf(vec![
                HostAdmissionAllocatorCounter::GrantEpoch,
                HostAdmissionAllocatorCounter::HostVersion,
                HostAdmissionAllocatorCounter::TopSlot,
            ])
        }
        _ => HostAdmissionOperationExhaustionExpectation::Forbidden,
    };
    let exhaustion_matches = match (&expected_exhaustion, exhaustion) {
        (HostAdmissionOperationExhaustionExpectation::Forbidden, None) => true,
        (HostAdmissionOperationExhaustionExpectation::RequiredOneOf(allowed), Some(observed)) => {
            allowed.contains(&observed.counter)
        }
        (HostAdmissionOperationExhaustionExpectation::Forbidden, Some(_))
        | (HostAdmissionOperationExhaustionExpectation::RequiredOneOf(_), None) => false,
    };
    let contradiction = if error == AdmissionError::OutstandingExists {
        observed_issuer.outstanding.clone().map(|observed| {
            HostAdmissionOperationFailureContradiction::OutstandingWithoutReturnedOwner { observed }
        })
    } else if !exhaustion_matches {
        Some(HostAdmissionOperationFailureContradiction::ExhaustionPairMismatch)
    } else {
        None
    };
    if let Some(contradiction) = contradiction {
        return HostAdmissionOperationFailureFreezeOutcome::Mismatch(
            HostAdmissionOperationFailureEvidenceMismatch {
                source,
                operation,
                error,
                expected_issuer,
                observed_issuer,
                expected_exhaustion,
                observed_exhaustion: exhaustion,
                contradiction,
            },
        );
    }
    let Some(disposition) = (match error {
        AdmissionError::AuthorityExists => {
            Some(HostAdmissionOperationFailureDisposition::PermanentAuthorityConflict)
        }
        AdmissionError::ForeignRouter | AdmissionError::Stale => {
            Some(HostAdmissionOperationFailureDisposition::PermanentForeignOrStale)
        }
        AdmissionError::Exhausted => {
            Some(HostAdmissionOperationFailureDisposition::PermanentIdentityExhausted)
        }
        AdmissionError::OutstandingExists => None,
    }) else {
        return HostAdmissionOperationFailureFreezeOutcome::Mismatch(
            HostAdmissionOperationFailureEvidenceMismatch {
                source,
                operation,
                error,
                expected_issuer,
                observed_issuer,
                expected_exhaustion,
                observed_exhaustion: exhaustion,
                contradiction: HostAdmissionOperationFailureContradiction::ExhaustionPairMismatch,
            },
        );
    };
    HostAdmissionOperationFailureFreezeOutcome::Frozen(HostAdmissionOperationFailureEvidence {
        projection: HostAdmissionOperationFailureProjection {
            source,
            operation,
            error,
            disposition,
            expected_issuer,
            observed_issuer,
            exhaustion,
        },
    })
}

fn bind_host_admission_unavailable_cause_with_issuer<Evidence>(
    source: HostAdmissionSourceProjection,
    projection: &HostAdmissionUnavailableEvidenceProjection,
    expected_issuer: HostAdmissionIssuerProjection,
    expected_owner_inventory: HostAdmissionUnavailableOwnerInventoryProjection,
    evidence: Evidence,
) -> HostAdmissionUnavailableCauseBindOutcome<Evidence> {
    let operation_exact = match &projection.operation_failure {
        Some(operation) => {
            projection.kind == HostAdmissionUnavailableEvidenceKind::IssuerOperationFailed
                && operation.source == source
                && projection.exhaustion == operation.exhaustion
        }
        None => projection.kind != HostAdmissionUnavailableEvidenceKind::IssuerOperationFailed,
    };
    if projection.source != source || !operation_exact {
        return HostAdmissionUnavailableCauseBindOutcome::Mismatch {
            mismatch: HostAdmissionSourceIneligibleProjection {
                expected: source.clone(),
                observed: Some(projection.source.clone()),
            },
            source,
            evidence,
            projection: projection.clone(),
        };
    }
    let permanent_identity = projection.exhaustion.is_some_and(|exhaustion| {
        matches!(
            exhaustion.counter,
            HostAdmissionAllocatorCounter::RegistrationIncarnation
                | HostAdmissionAllocatorCounter::CandidateRevision
        )
    });
    let disposition = match projection.kind {
        HostAdmissionUnavailableEvidenceKind::NoCandidate
            if source.kind == HostAdmissionSourceKind::InitialAwaiting =>
        {
            HostAdmissionUnavailableDisposition::Awaiting
        }
        HostAdmissionUnavailableEvidenceKind::NoCandidate => match source.public_reason.clone() {
            Some(reason) => HostAdmissionUnavailableDisposition::Unavailable {
                reason,
                permanence: if matches!(
                    source.kind,
                    HostAdmissionSourceKind::PermanentAuthorityEstablishmentFailed
                        | HostAdmissionSourceKind::PermanentIdentityExhausted
                        | HostAdmissionSourceKind::PermanentRegistryInvariantViolation
                ) {
                    HostAdmissionPermanence::PermanentForRouter
                } else {
                    HostAdmissionPermanence::RetryableAfterCandidateChange
                },
            },
            None => HostAdmissionUnavailableDisposition::Awaiting,
        },
        HostAdmissionUnavailableEvidenceKind::Conflict => {
            HostAdmissionUnavailableDisposition::Unavailable {
                reason: RouterUnavailableReason::HostConflict,
                permanence: HostAdmissionPermanence::RetryableAfterCandidateChange,
            }
        }
        HostAdmissionUnavailableEvidenceKind::InvalidTopology => {
            HostAdmissionUnavailableDisposition::Unavailable {
                reason: RouterUnavailableReason::InvalidHostUiTopology,
                permanence: HostAdmissionPermanence::RetryableAfterCandidateChange,
            }
        }
        HostAdmissionUnavailableEvidenceKind::HostLost => {
            HostAdmissionUnavailableDisposition::Unavailable {
                reason: RouterUnavailableReason::HostLost,
                permanence: HostAdmissionPermanence::RetryableAfterCandidateChange,
            }
        }
        HostAdmissionUnavailableEvidenceKind::RegistryInvariantViolation => {
            HostAdmissionUnavailableDisposition::Unavailable {
                reason: RouterUnavailableReason::RegistryInvariantViolation,
                permanence: HostAdmissionPermanence::PermanentForRouter,
            }
        }
        HostAdmissionUnavailableEvidenceKind::AuthorityEstablishmentFailed
        | HostAdmissionUnavailableEvidenceKind::PostAllocationRejected
            if !permanent_identity
                && !matches!(
                    source.kind,
                    HostAdmissionSourceKind::PermanentAuthorityEstablishmentFailed
                        | HostAdmissionSourceKind::PermanentIdentityExhausted
                        | HostAdmissionSourceKind::PermanentRegistryInvariantViolation
                ) =>
        {
            HostAdmissionUnavailableDisposition::Unavailable {
                reason: RouterUnavailableReason::AuthorityEstablishmentFailed,
                permanence: HostAdmissionPermanence::RetryableAfterCandidateChange,
            }
        }
        HostAdmissionUnavailableEvidenceKind::AuthorityEstablishmentFailed
        | HostAdmissionUnavailableEvidenceKind::IssuerOperationFailed
        | HostAdmissionUnavailableEvidenceKind::ConfirmationFailed
        | HostAdmissionUnavailableEvidenceKind::PreallocationRejected
        | HostAdmissionUnavailableEvidenceKind::PostAllocationRejected => {
            HostAdmissionUnavailableDisposition::Unavailable {
                reason: RouterUnavailableReason::AuthorityEstablishmentFailed,
                permanence: HostAdmissionPermanence::PermanentForRouter,
            }
        }
    };
    let cause_digest = projection
        .expected_digest
        .rotate_left(17)
        .wrapping_add(projection.observed_digest.rotate_right(9))
        .wrapping_add(source.availability_incarnation.0)
        .wrapping_add(projection.candidate_revision.unwrap_or(0))
        .wrapping_add(1);
    let cause_digest = if cause_digest == 0 { 1 } else { cause_digest };
    HostAdmissionUnavailableCauseBindOutcome::Bound(PreparedHostAdmissionUnavailableCause {
        core: HostAdmissionUnavailableCause {
            source,
            evidence: projection.clone(),
            disposition,
            cause_digest,
            expected_issuer: Box::new(expected_issuer),
            expected_owner_inventory: Box::new(expected_owner_inventory),
        },
        evidence,
    })
}

pub(crate) struct RouterEngine {
    matcher: RouteMatcher,
    ids: CoreIdAllocator,
    router: RouterIdentity,
    fallback: RouteViewKey,
    state: RouterNavigationState,
    host_admission_source: HostAdmissionSourceProjection,
    host_admission_source_open: bool,
    pending_host_admission: Option<CorePendingHostAdmission>,
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
    pending_post_commit_terminal: Option<PreparedPostCommitTerminalPlan>,
    pending_post_commit_settlement_trace: Option<CorePostCommitSettlementTraceProjection>,
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
        let host_admission_source = HostAdmissionSourceProjection {
            router,
            availability_incarnation: AvailabilityIncarnation(1),
            kind: HostAdmissionSourceKind::InitialAwaiting,
            public_reason: None,
            host: None,
            top_slot: None,
        };
        Ok(Self {
            matcher: RouteMatcher::new(tree),
            ids,
            router,
            fallback,
            state: RouterNavigationState::new(router),
            host_admission_source,
            host_admission_source_open: true,
            pending_host_admission: None,
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
            pending_post_commit_terminal: None,
            pending_post_commit_settlement_trace: None,
            #[cfg(test)]
            testing_back_target_source_drift_before_freeze_once: false,
        })
    }

    pub(crate) fn bind_host_admission_unavailable_cause<Evidence>(
        &self,
        issuer: &HostAdmissionIssuer,
        source: HostAdmissionSourceProjection,
        projection: &HostAdmissionUnavailableEvidenceProjection,
        evidence: Evidence,
    ) -> HostAdmissionUnavailableCauseBindOutcome<Evidence> {
        bind_host_admission_unavailable_cause_with_issuer(
            source,
            projection,
            issuer.projection(),
            self.observed_unavailable_owner_inventory(),
            evidence,
        )
    }

    pub(crate) fn request_legacy_host_admission(
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

    pub(crate) fn grant_legacy_host_admission(
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

    pub(crate) fn issue_host_admission_source_permit(
        &mut self,
    ) -> HostAdmissionSourcePermitOutcome {
        let source = self.host_admission_source.clone();
        if self.pending_host_admission.is_some() {
            return HostAdmissionSourcePermitOutcome::Outstanding { source };
        }
        let availability_matches = match source.public_reason.as_ref() {
            None => self.state.availability() == &RouterAvailability::AwaitingHost,
            Some(reason) => {
                self.state.availability() == &RouterAvailability::Unavailable(reason.clone())
            }
        };
        if !self.host_admission_source_open || !availability_matches || self.state.host().is_some()
        {
            return HostAdmissionSourcePermitOutcome::Ineligible {
                evidence: HostAdmissionSourceIneligibleProjection {
                    expected: source.clone(),
                    observed: Some(source.clone()),
                },
                source,
            };
        }
        let sequence = source.availability_incarnation.0;
        let key = HostAdmissionSourcePermitKey {
            router: self.router,
            availability_incarnation: source.availability_incarnation,
            sequence,
            binding: sequence
                .rotate_left(19)
                .wrapping_add(source.availability_incarnation.0.rotate_right(7))
                .wrapping_add(self.router.router().raw()),
        };
        let projection = CorePendingHostAdmissionProjection {
            source: source.clone(),
            stage: CorePendingHostAdmissionStage::Permit,
            permit_key: Some(key),
            host: None,
            top_slot: None,
        };
        self.host_admission_source_open = false;
        self.pending_host_admission = Some(CorePendingHostAdmission::Permit(projection));
        HostAdmissionSourcePermitOutcome::Issued(HostAdmissionSourcePermit { source, key })
    }

    pub(crate) fn request_host_admission(
        &mut self,
        issuer: &mut HostAdmissionIssuer,
        permit: HostAdmissionSourcePermit,
    ) -> Result<SourceBoundHostAdmissionToken, AdmissionOwnerError<HostAdmissionSourcePermit>> {
        let expected_pending = CorePendingHostAdmissionProjection {
            source: permit.source.clone(),
            stage: CorePendingHostAdmissionStage::Permit,
            permit_key: Some(permit.key),
            host: None,
            top_slot: None,
        };
        let expected_issuer = issuer.projection();
        let receiver_exact = self.host_admission_source == permit.source
            && self.pending_host_admission.as_ref()
                == Some(&CorePendingHostAdmission::Permit(expected_pending));
        if !receiver_exact {
            let observed_issuer = issuer.projection();
            return Err(AdmissionOwnerError {
                evidence: freeze_host_admission_operation_failure(
                    permit.source.clone(),
                    HostAdmissionOperation::Request,
                    AdmissionError::Stale,
                    expected_issuer,
                    observed_issuer,
                    None,
                ),
                owner: permit,
            });
        }
        match issuer.issue_with_evidence(self.router) {
            Ok(token) => {
                let issuer_projection = issuer.projection();
                let projection = CorePendingHostAdmissionProjection {
                    source: permit.source.clone(),
                    stage: CorePendingHostAdmissionStage::Token,
                    permit_key: Some(permit.key),
                    host: None,
                    top_slot: None,
                };
                self.pending_host_admission = Some(CorePendingHostAdmission::Token(projection));
                Ok(SourceBoundHostAdmissionToken {
                    permit,
                    token,
                    issuer: issuer_projection,
                })
            }
            Err(failure) => {
                let observed_issuer = issuer.projection();
                Err(AdmissionOwnerError {
                    evidence: freeze_host_admission_operation_failure(
                        permit.source.clone(),
                        HostAdmissionOperation::Request,
                        failure.error,
                        expected_issuer,
                        observed_issuer,
                        failure.exhaustion,
                    ),
                    owner: permit,
                })
            }
        }
    }

    pub(crate) fn grant_host_admission(
        &mut self,
        issuer: &mut HostAdmissionIssuer,
        token: SourceBoundHostAdmissionToken,
    ) -> Result<SourceBoundHostGrant, AdmissionOwnerError<SourceBoundHostAdmissionToken>> {
        let expected_pending = CorePendingHostAdmissionProjection {
            source: token.permit.source.clone(),
            stage: CorePendingHostAdmissionStage::Token,
            permit_key: Some(token.permit.key),
            host: None,
            top_slot: None,
        };
        let expected_issuer = issuer.projection();
        let receiver_exact = self.host_admission_source == token.permit.source
            && self.pending_host_admission.as_ref()
                == Some(&CorePendingHostAdmission::Token(expected_pending));
        if !receiver_exact {
            let observed_issuer = issuer.projection();
            return Err(AdmissionOwnerError {
                evidence: freeze_host_admission_operation_failure(
                    token.permit.source.clone(),
                    HostAdmissionOperation::Grant,
                    AdmissionError::Stale,
                    expected_issuer,
                    observed_issuer,
                    None,
                ),
                owner: token,
            });
        }
        match issuer.grant_with_evidence(&token.token, &mut self.ids) {
            Ok(grant) => {
                let issuer_projection = issuer.projection();
                let projection = CorePendingHostAdmissionProjection {
                    source: token.permit.source.clone(),
                    stage: CorePendingHostAdmissionStage::Grant,
                    permit_key: Some(token.permit.key),
                    host: Some(grant.version().clone()),
                    top_slot: Some(grant.top_slot()),
                };
                self.pending_host_admission = Some(CorePendingHostAdmission::Grant(projection));
                Ok(SourceBoundHostGrant {
                    source: token,
                    grant,
                    issuer: issuer_projection,
                })
            }
            Err(failure) => {
                let observed_issuer = issuer.projection();
                Err(AdmissionOwnerError {
                    evidence: freeze_host_admission_operation_failure(
                        token.permit.source.clone(),
                        HostAdmissionOperation::Grant,
                        failure.error,
                        expected_issuer,
                        observed_issuer,
                        failure.exhaustion,
                    ),
                    owner: token,
                })
            }
        }
    }

    pub(crate) fn confirmation_receiver_for(
        grant: &SourceBoundHostGrant,
    ) -> HostConfirmationReceiverProjection {
        let grant_projection = grant.projection();
        HostConfirmationReceiverProjection {
            source: grant_projection.source.clone(),
            pending: CorePendingHostAdmissionProjection {
                source: grant_projection.source.clone(),
                stage: CorePendingHostAdmissionStage::Confirming,
                permit_key: Some(grant_projection.permit_key),
                host: Some(grant_projection.host.clone()),
                top_slot: Some(grant_projection.top_slot),
            },
            grant: grant_projection,
        }
    }

    fn grant_pending_for(grant: &SourceBoundHostGrant) -> CorePendingHostAdmissionProjection {
        let projection = grant.projection();
        CorePendingHostAdmissionProjection {
            source: projection.source,
            stage: CorePendingHostAdmissionStage::Grant,
            permit_key: Some(projection.permit_key),
            host: Some(projection.host),
            top_slot: Some(projection.top_slot),
        }
    }

    pub(crate) fn prepare_host_confirmation<Installation>(
        &mut self,
        owner: CoreHostConfirmationSource<Installation>,
    ) -> HostConfirmationPreparationOutcome<Installation> {
        let receiver = Self::confirmation_receiver_for(&owner.grant);
        let expected_grant_pending = Self::grant_pending_for(&owner.grant);
        let exact = self.host_admission_source == receiver.source
            && self.pending_host_admission.as_ref()
                == Some(&CorePendingHostAdmission::Grant(expected_grant_pending));
        let Some(ready_state) = self
            .state
            .prepare_host_ready(&receiver.grant.host, receiver.grant.top_slot)
        else {
            return HostConfirmationPreparationOutcome::ReceiverMismatch {
                evidence: HostConfirmationReceiverMismatchProjection {
                    expected: receiver,
                    observed: self
                        .pending_host_admission
                        .as_ref()
                        .and_then(CorePendingHostAdmission::confirmation_receiver),
                },
                owner,
            };
        };
        if !exact {
            return HostConfirmationPreparationOutcome::ReceiverMismatch {
                evidence: HostConfirmationReceiverMismatchProjection {
                    expected: receiver,
                    observed: self
                        .pending_host_admission
                        .as_ref()
                        .and_then(CorePendingHostAdmission::confirmation_receiver),
                },
                owner,
            };
        }
        let ready = PreparedHostReadyPublication {
            projection: HostAdmissionReadyProjection {
                source: receiver.source.clone(),
                host: receiver.grant.host.clone(),
                top_slot: receiver.grant.top_slot,
                availability: RouterAvailability::Ready,
            },
        };
        let expected_issuer = owner.grant.issuer.clone();
        self.pending_host_admission = Some(CorePendingHostAdmission::Confirming(receiver.clone()));
        HostConfirmationPreparationOutcome::Prepared(CorePreparedHostConfirmation {
            grant: owner.grant,
            installation: owner.installation,
            receiver,
            ready,
            expected_issuer,
            ready_state,
        })
    }

    pub(crate) fn unwind_host_confirmation<Installation>(
        &mut self,
        owner: CorePreparedHostConfirmation<Installation>,
    ) -> HostConfirmationUnwindOutcome<Installation> {
        let exact = self.pending_host_admission.as_ref()
            == Some(&CorePendingHostAdmission::Confirming(
                owner.receiver.clone(),
            ));
        if !exact {
            let evidence = HostConfirmationReceiverMismatchProjection {
                expected: owner.receiver.clone(),
                observed: self
                    .pending_host_admission
                    .as_ref()
                    .and_then(CorePendingHostAdmission::confirmation_receiver),
            };
            return HostConfirmationUnwindOutcome::Mismatch { owner, evidence };
        }
        self.pending_host_admission = Some(CorePendingHostAdmission::Grant(
            Self::grant_pending_for(&owner.grant),
        ));
        HostConfirmationUnwindOutcome::Grant(CoreHostConfirmationSource {
            grant: owner.grant,
            installation: owner.installation,
        })
    }

    pub(crate) fn confirm_host_admission_with_binding<Installation, Installed, F>(
        &mut self,
        issuer: &mut HostAdmissionIssuer,
        owner: CorePreparedHostConfirmation<Installation>,
        install_binding: F,
    ) -> HostAdmissionConfirmationCoreOutcome<Installation, Installed>
    where
        F: FnOnce(Installation, HostLease) -> Installed,
    {
        let receiver_exact = self.pending_host_admission.as_ref()
            == Some(&CorePendingHostAdmission::Confirming(
                owner.receiver.clone(),
            ));
        if !receiver_exact {
            let evidence = HostConfirmationReceiverMismatchProjection {
                expected: owner.receiver.clone(),
                observed: self
                    .pending_host_admission
                    .as_ref()
                    .and_then(CorePendingHostAdmission::confirmation_receiver),
            };
            return HostAdmissionConfirmationCoreOutcome::ReceiverMismatch { owner, evidence };
        }
        let observed_issuer = issuer.projection();
        if observed_issuer != owner.expected_issuer {
            let evidence = HostConfirmationIssuerMismatchProjection {
                receiver: owner.receiver.clone(),
                expected_issuer: owner.expected_issuer.clone(),
                observed_issuer,
            };
            return HostAdmissionConfirmationCoreOutcome::IssuerRejected { owner, evidence };
        }
        let confirmation = match issuer.confirm(&owner.grant.grant) {
            Ok(confirmation) => confirmation,
            Err(_) => {
                let evidence = HostConfirmationIssuerMismatchProjection {
                    receiver: owner.receiver.clone(),
                    expected_issuer: owner.expected_issuer.clone(),
                    observed_issuer: issuer.projection(),
                };
                return HostAdmissionConfirmationCoreOutcome::IssuerRejected { owner, evidence };
            }
        };
        let projection = owner.ready.projection;
        let installed = install_binding(owner.installation, confirmation.into_lease());
        self.pending_host_admission = None;
        self.host_admission_source_open = false;
        self.state = owner.ready_state;
        HostAdmissionConfirmationCoreOutcome::Ready {
            projection,
            installed,
        }
    }

    fn unavailable_receiver_mismatch(
        &self,
        expected_source: &HostAdmissionSourceProjection,
    ) -> HostAdmissionUnavailableMismatchProjection {
        HostAdmissionUnavailableMismatchProjection {
            expected_source: expected_source.clone(),
            observed_pending: self
                .pending_host_admission
                .as_ref()
                .map(CorePendingHostAdmission::projection),
        }
    }

    fn observed_unavailable_owner_inventory(
        &self,
    ) -> HostAdmissionUnavailableOwnerInventoryProjection {
        let action = self.action.as_ref().map(|action| match action {
            EngineAction::FrameTask(task) => {
                HostAdmissionUnavailableActionOwnerProjection::FrameTask(task.identity.clone())
            }
            EngineAction::MountRequest(request) => {
                HostAdmissionUnavailableActionOwnerProjection::MountRequest {
                    request: request.request,
                    parent: request.parent.clone(),
                    outlet: request.outlet.clone(),
                }
            }
            EngineAction::FailureCleanup(request) => {
                HostAdmissionUnavailableActionOwnerProjection::FailureCleanup(request.request())
            }
            EngineAction::Commit(ticket) => {
                HostAdmissionUnavailableActionOwnerProjection::Commit(ticket.identity.clone())
            }
            EngineAction::BackResolution(_) => {
                HostAdmissionUnavailableActionOwnerProjection::BackResolution
            }
        });
        let pending_history_preparation_error = self
            .pending_history_preparation_error
            .as_ref()
            .map(|error| match error {
                HistoryPreparationError::Source(_) => {
                    HostAdmissionUnavailableHistoryPreparationOwnerProjection::Source
                }
                HistoryPreparationError::Binding(_) => {
                    HostAdmissionUnavailableHistoryPreparationOwnerProjection::Binding
                }
            });
        let prepared_publication_authority =
            self.prepared_publication_authority
                .as_ref()
                .map(|authority| match authority {
                    PreparedPublicationAuthority::Ready(owner) => {
                        HostAdmissionUnavailablePublicationOwnerProjection::Ready {
                            identity: owner.identity.clone(),
                            evidence: owner.evidence.clone(),
                        }
                    }
                    PreparedPublicationAuthority::Rejected(rejection) => {
                        HostAdmissionUnavailablePublicationOwnerProjection::Rejected {
                            identity: rejection.owner.identity.clone(),
                            evidence: rejection.owner.evidence.clone(),
                            error: rejection.error,
                        }
                    }
                });
        let pending_back_no_target_authority =
            self.pending_back_no_target_authority
                .as_ref()
                .map(|pending| match pending {
                    CorePendingBackNoTargetAuthority::Accepted(accepted) => {
                        HostAdmissionUnavailableBackNoTargetAuthorityOwnerProjection::Accepted {
                            receiver: accepted.receiver.clone(),
                            source: accepted.source.clone(),
                            host: accepted.host.clone(),
                            top_slot: accepted.top_slot,
                            logical_roots: accepted.logical_roots.clone(),
                        }
                    }
                    CorePendingBackNoTargetAuthority::Cleanup(cleanup) => {
                        HostAdmissionUnavailableBackNoTargetAuthorityOwnerProjection::Cleanup(
                            cleanup.receiver.clone(),
                        )
                    }
                });
        HostAdmissionUnavailableOwnerInventoryProjection {
            pending_host_admission: self
                .pending_host_admission
                .as_ref()
                .map(CorePendingHostAdmission::projection),
            history_admission_world: self.history_admission_world,
            queued_requests: self.queue.iter().map(|queued| queued.request).collect(),
            current: self.current.as_ref().map(|current| {
                HostAdmissionUnavailableCurrentOwnerProjection {
                    identity: current.identity.clone(),
                    stage: current.stage.projection(),
                }
            }),
            action,
            pending_no_target_back: self
                .pending_no_target_back
                .as_ref()
                .map(|pending| pending.request),
            current_history_capability: self
                .current_history_capability
                .as_ref()
                .map(HistoryCommitCapability::execution_projection),
            pending_history_preparation_error,
            prepared_publication_authority,
            pending_back_no_target_authority,
            pending_back_target_permanent_cleanup: self
                .pending_back_target_permanent_cleanup
                .as_ref()
                .map(|cleanup| cleanup.receiver.clone()),
            pending_post_commit_terminal: self
                .pending_post_commit_terminal
                .as_ref()
                .map(CorePreparedPostCommitTerminalSourceProjection::from),
            pending_post_commit_settlement_trace: self.pending_post_commit_settlement_trace.clone(),
            live_failed_authority_trigger_revision: self.live_failed_authority_trigger_revision,
            mount_issuer: self.mount.projection(),
        }
    }

    fn observed_unavailable_receiver_mismatch(
        &self,
        issuer: &HostAdmissionIssuer,
        cause: &HostAdmissionUnavailableCause,
    ) -> HostAdmissionUnavailableObservedMismatchProjection {
        let expected_source = &cause.source;
        HostAdmissionUnavailableObservedMismatchProjection {
            expected_source: expected_source.clone(),
            observed_source: self.host_admission_source.clone(),
            observed_source_open: self.host_admission_source_open,
            observed_pending: self
                .pending_host_admission
                .as_ref()
                .map(CorePendingHostAdmission::projection),
            observed_availability: self.state.availability().clone(),
            observed_host: self.state.host().cloned(),
            observed_top_slot: self.state.top_slot(),
            observed_active: self.state.active().clone(),
            observed_request: self.state.current_request().cloned(),
            observed_current_identity: self
                .current
                .as_ref()
                .map(|current| current.identity.clone()),
            observed_current_stage: self
                .current
                .as_ref()
                .map(|current| current.stage.projection()),
            observed_action_present: self.action.is_some(),
            expected_owner_inventory: cause.expected_owner_inventory.as_ref().clone(),
            observed_owner_inventory: self.observed_unavailable_owner_inventory(),
            expected_issuer: cause.expected_issuer.as_ref().clone(),
            observed_issuer: issuer.projection(),
        }
    }

    fn observed_unavailable_receiver_is_exact(
        &self,
        issuer: &HostAdmissionIssuer,
        cause: &HostAdmissionUnavailableCause,
    ) -> bool {
        let expected_source = &cause.source;
        let expected_availability = match expected_source.kind {
            HostAdmissionSourceKind::InitialAwaiting => RouterAvailability::AwaitingHost,
            _ => {
                let Some(reason) = expected_source.public_reason.clone() else {
                    return false;
                };
                RouterAvailability::Unavailable(reason)
            }
        };
        let expected_source_open = expected_source.kind == HostAdmissionSourceKind::InitialAwaiting;
        self.host_admission_source == *expected_source
            && self.host_admission_source_open == expected_source_open
            && self.pending_host_admission.is_none()
            && self.state.availability() == &expected_availability
            && self.state.host().is_none()
            && self.state.top_slot().is_none()
            && self.state.active() == &ActivePresentation::none()
            && self.state.current_request().is_none()
            && cause
                .expected_owner_inventory
                .is_observed_unavailable_idle()
            && &self.observed_unavailable_owner_inventory()
                == cause.expected_owner_inventory.as_ref()
            && &issuer.projection() == cause.expected_issuer.as_ref()
    }

    fn prepare_host_admission_unavailable_publication(
        &self,
        cause: HostAdmissionUnavailableCause,
        reopen_after_changed_observation: bool,
    ) -> PreparedHostUnavailablePublication {
        let HostAdmissionUnavailableDisposition::Unavailable {
            mut reason,
            mut permanence,
        } = cause.disposition
        else {
            unreachable!("Awaiting is returned before unavailable publication")
        };
        let (next_incarnation, incarnation_exhausted) =
            match cause.source.availability_incarnation.0.checked_add(1) {
                Some(next) => (next, false),
                None => {
                    reason = RouterUnavailableReason::AuthorityEstablishmentFailed;
                    permanence = HostAdmissionPermanence::PermanentForRouter;
                    (cause.source.availability_incarnation.0, true)
                }
            };
        let kind = match (&reason, permanence) {
            (_, HostAdmissionPermanence::PermanentForRouter)
                if cause.evidence.exhaustion.is_some()
                    || incarnation_exhausted
                    || cause
                        .evidence
                        .operation_failure
                        .as_ref()
                        .is_some_and(|failure| {
                            failure.disposition
                                == HostAdmissionOperationFailureDisposition::PermanentIdentityExhausted
                        }) => HostAdmissionSourceKind::PermanentIdentityExhausted,
            (RouterUnavailableReason::RegistryInvariantViolation, _) => {
                HostAdmissionSourceKind::PermanentRegistryInvariantViolation
            }
            (RouterUnavailableReason::HostLost, _) => HostAdmissionSourceKind::RetryableHostLost,
            (RouterUnavailableReason::HostConflict, _) => {
                HostAdmissionSourceKind::RetryableHostConflict
            }
            (RouterUnavailableReason::InvalidHostUiTopology, _) => {
                HostAdmissionSourceKind::RetryableInvalidHostUiTopology
            }
            (RouterUnavailableReason::AuthorityEstablishmentFailed, _) => {
                if permanence == HostAdmissionPermanence::PermanentForRouter {
                    HostAdmissionSourceKind::PermanentAuthorityEstablishmentFailed
                } else {
                    HostAdmissionSourceKind::RetryableAuthorityEstablishmentFailed
                }
            }
        };
        let retained_host = cause.source.host.or_else(|| self.state.host().cloned());
        let retained_top_slot = cause.source.top_slot.or_else(|| self.state.top_slot());
        let source = HostAdmissionSourceProjection {
            router: self.router,
            availability_incarnation: AvailabilityIncarnation(next_incarnation),
            kind,
            public_reason: Some(reason.clone()),
            host: retained_host,
            top_slot: retained_top_slot,
        };
        let terminal = HostAdmissionUnavailableTerminalProjection {
            source: source.clone(),
            cause_digest: cause.cause_digest,
            reason: reason.clone(),
            permanence,
        };
        let mut state = self.state.clone();
        state.fail_closed_unavailable(reason);
        PreparedHostUnavailablePublication {
            source_open: reopen_after_changed_observation
                && permanence == HostAdmissionPermanence::RetryableAfterCandidateChange,
            terminal,
            source,
            state,
        }
    }

    fn publish_prepared_host_admission_unavailable(
        &mut self,
        prepared: PreparedHostUnavailablePublication,
    ) -> HostAdmissionUnavailableTerminalProjection {
        self.pending_host_admission = None;
        self.host_admission_source = prepared.source;
        self.host_admission_source_open = prepared.source_open;
        self.state = prepared.state;
        prepared.terminal
    }

    fn retain_host_loss_admission_source(&mut self, host: HostVersion, top_slot: RouteSlotId) {
        let Some(incarnation) = self
            .host_admission_source
            .availability_incarnation
            .0
            .checked_add(1)
        else {
            self.host_admission_source = HostAdmissionSourceProjection {
                router: self.router,
                availability_incarnation: self.host_admission_source.availability_incarnation,
                kind: HostAdmissionSourceKind::PermanentIdentityExhausted,
                public_reason: Some(RouterUnavailableReason::AuthorityEstablishmentFailed),
                host: Some(host),
                top_slot: Some(top_slot),
            };
            self.host_admission_source_open = false;
            self.pending_host_admission = None;
            self.state
                .fail_closed_unavailable(RouterUnavailableReason::AuthorityEstablishmentFailed);
            return;
        };
        self.host_admission_source = HostAdmissionSourceProjection {
            router: self.router,
            availability_incarnation: AvailabilityIncarnation(incarnation),
            kind: HostAdmissionSourceKind::RetryableHostLost,
            public_reason: Some(RouterUnavailableReason::HostLost),
            host: Some(host),
            top_slot: Some(top_slot),
        };
        self.host_admission_source_open = false;
        self.pending_host_admission = None;
    }

    fn retain_awaiting_admission_source(&mut self) {
        let Some(incarnation) = self
            .host_admission_source
            .availability_incarnation
            .0
            .checked_add(1)
        else {
            self.host_admission_source = HostAdmissionSourceProjection {
                router: self.router,
                availability_incarnation: self.host_admission_source.availability_incarnation,
                kind: HostAdmissionSourceKind::PermanentIdentityExhausted,
                public_reason: Some(RouterUnavailableReason::AuthorityEstablishmentFailed),
                host: None,
                top_slot: None,
            };
            self.host_admission_source_open = false;
            self.pending_host_admission = None;
            self.state
                .fail_closed_unavailable(RouterUnavailableReason::AuthorityEstablishmentFailed);
            return;
        };
        self.host_admission_source = HostAdmissionSourceProjection {
            router: self.router,
            availability_incarnation: AvailabilityIncarnation(incarnation),
            kind: HostAdmissionSourceKind::InitialAwaiting,
            public_reason: None,
            host: None,
            top_slot: None,
        };
        self.host_admission_source_open = true;
        self.pending_host_admission = None;
    }

    pub(crate) fn complete_pre_staging_host_admission_unavailable<
        Installation,
        Evidence,
        PhysicalTerminal,
        Consume,
    >(
        &mut self,
        issuer: &mut HostAdmissionIssuer,
        source: HostAdmissionUnavailablePreStagingSource<Installation, Evidence>,
        consume_installation_and_clear_holder: Consume,
    ) -> HostAdmissionUnavailablePreStagingOutcome<Installation, Evidence, PhysicalTerminal>
    where
        Consume: FnOnce(Installation) -> PhysicalTerminal,
    {
        let expected_source = source.prepared.core.source.clone();
        let expected_pending = match &source.owner {
            HostAdmissionUnavailablePreStagingOwner::Permit(permit) => {
                CorePendingHostAdmission::Permit(CorePendingHostAdmissionProjection {
                    source: permit.source.clone(),
                    stage: CorePendingHostAdmissionStage::Permit,
                    permit_key: Some(permit.key),
                    host: None,
                    top_slot: None,
                })
            }
            HostAdmissionUnavailablePreStagingOwner::Token(token) => {
                CorePendingHostAdmission::Token(CorePendingHostAdmissionProjection {
                    source: token.permit.source.clone(),
                    stage: CorePendingHostAdmissionStage::Token,
                    permit_key: Some(token.permit.key),
                    host: None,
                    top_slot: None,
                })
            }
        };
        if self.host_admission_source != expected_source
            || self.pending_host_admission.as_ref() != Some(&expected_pending)
        {
            let evidence = HostAdmissionUnavailableFailureEvidence::Receiver(
                self.unavailable_receiver_mismatch(&expected_source),
            );
            return HostAdmissionUnavailablePreStagingOutcome::Mismatch { source, evidence };
        }
        let expected_cause_issuer = source.prepared.core.expected_issuer.as_ref().clone();
        let observed_cause_issuer = issuer.projection();
        if observed_cause_issuer != expected_cause_issuer {
            let evidence = HostAdmissionUnavailableFailureEvidence::Issuer(
                HostAdmissionUnavailableIssuerMismatchProjection {
                    receiver: self.unavailable_receiver_mismatch(&expected_source),
                    operation: HostAdmissionUnavailableIssuerOperation::RefreezeCause,
                    expected_issuer: expected_cause_issuer,
                    observed_issuer: observed_cause_issuer,
                },
            );
            return HostAdmissionUnavailablePreStagingOutcome::Mismatch { source, evidence };
        }
        if let HostAdmissionUnavailablePreStagingOwner::Token(token) = &source.owner {
            let expected_issuer = token.issuer.clone();
            let observed_issuer = issuer.projection();
            if observed_issuer != expected_issuer {
                let evidence = HostAdmissionUnavailableFailureEvidence::Issuer(
                    HostAdmissionUnavailableIssuerMismatchProjection {
                        receiver: self.unavailable_receiver_mismatch(&expected_source),
                        operation: HostAdmissionUnavailableIssuerOperation::CancelToken,
                        expected_issuer,
                        observed_issuer,
                    },
                );
                return HostAdmissionUnavailablePreStagingOutcome::Mismatch { source, evidence };
            }
        }
        let HostAdmissionUnavailablePreStagingSource {
            prepared,
            owner,
            installation,
        } = source;
        let publication =
            self.prepare_host_admission_unavailable_publication(prepared.core.clone(), false);
        if let HostAdmissionUnavailablePreStagingOwner::Token(token) = &owner
            && issuer.cancel(&token.token).is_err()
        {
            let evidence = HostAdmissionUnavailableFailureEvidence::Issuer(
                HostAdmissionUnavailableIssuerMismatchProjection {
                    receiver: self.unavailable_receiver_mismatch(&expected_source),
                    operation: HostAdmissionUnavailableIssuerOperation::CancelToken,
                    expected_issuer: token.issuer.clone(),
                    observed_issuer: issuer.projection(),
                },
            );
            return HostAdmissionUnavailablePreStagingOutcome::Mismatch {
                source: HostAdmissionUnavailablePreStagingSource {
                    prepared,
                    owner,
                    installation,
                },
                evidence,
            };
        }
        let physical = consume_installation_and_clear_holder(installation);
        let terminal = self.publish_prepared_host_admission_unavailable(publication);
        HostAdmissionUnavailablePreStagingOutcome::Published { terminal, physical }
    }

    pub(crate) fn complete_granted_host_admission_unavailable<
        Installation,
        Evidence,
        PhysicalTerminal,
        Consume,
    >(
        &mut self,
        issuer: &mut HostAdmissionIssuer,
        source: HostAdmissionUnavailableGrantedSource<Installation, Evidence>,
        consume_physical_terminal: Consume,
    ) -> HostAdmissionUnavailableGrantedOutcome<Installation, Evidence, PhysicalTerminal>
    where
        Consume: FnOnce(Installation) -> PhysicalTerminal,
    {
        let expected_source = source.prepared.core.source.clone();
        let expected_pending =
            CorePendingHostAdmission::Grant(Self::grant_pending_for(&source.owner.grant));
        if self.host_admission_source != expected_source
            || self.pending_host_admission.as_ref() != Some(&expected_pending)
        {
            let evidence = HostAdmissionUnavailableFailureEvidence::Receiver(
                self.unavailable_receiver_mismatch(&expected_source),
            );
            return HostAdmissionUnavailableGrantedOutcome::Mismatch { source, evidence };
        }
        let expected_issuer = source.owner.grant.issuer.clone();
        let observed_issuer = issuer.projection();
        if observed_issuer != expected_issuer {
            let evidence = HostAdmissionUnavailableFailureEvidence::Issuer(
                HostAdmissionUnavailableIssuerMismatchProjection {
                    receiver: self.unavailable_receiver_mismatch(&expected_source),
                    operation: HostAdmissionUnavailableIssuerOperation::RejectGrant,
                    expected_issuer,
                    observed_issuer,
                },
            );
            return HostAdmissionUnavailableGrantedOutcome::Mismatch { source, evidence };
        }
        let HostAdmissionUnavailableGrantedSource { prepared, owner } = source;
        let publication =
            self.prepare_host_admission_unavailable_publication(prepared.core.clone(), false);
        if issuer.reject_after_mutation(&owner.grant.grant).is_err() {
            let evidence = HostAdmissionUnavailableFailureEvidence::Issuer(
                HostAdmissionUnavailableIssuerMismatchProjection {
                    receiver: self.unavailable_receiver_mismatch(&expected_source),
                    operation: HostAdmissionUnavailableIssuerOperation::RejectGrant,
                    expected_issuer,
                    observed_issuer: issuer.projection(),
                },
            );
            return HostAdmissionUnavailableGrantedOutcome::Mismatch {
                source: HostAdmissionUnavailableGrantedSource { prepared, owner },
                evidence,
            };
        }
        let physical = consume_physical_terminal(owner.installation);
        let terminal = self.publish_prepared_host_admission_unavailable(publication);
        HostAdmissionUnavailableGrantedOutcome::Published { terminal, physical }
    }

    pub(crate) fn complete_observed_host_admission_unavailable<Evidence, Clear>(
        &mut self,
        issuer: &HostAdmissionIssuer,
        prepared: PreparedHostAdmissionUnavailableCause<Evidence>,
        clear_holder: Clear,
    ) -> HostAdmissionUnavailableObservedOutcome<Evidence>
    where
        Clear: FnOnce(),
    {
        let expected_source = prepared.core.source.clone();
        if !self.observed_unavailable_receiver_is_exact(issuer, &prepared.core) {
            let evidence = HostAdmissionUnavailableFailureEvidence::ObservedReceiver(Box::new(
                self.observed_unavailable_receiver_mismatch(issuer, &prepared.core),
            ));
            return HostAdmissionUnavailableObservedOutcome::Mismatch { prepared, evidence };
        }
        if prepared.core.disposition == HostAdmissionUnavailableDisposition::Awaiting {
            return HostAdmissionUnavailableObservedOutcome::Awaiting {
                source: expected_source,
                evidence: prepared.evidence,
            };
        }
        if matches!(
            expected_source.kind,
            HostAdmissionSourceKind::PermanentAuthorityEstablishmentFailed
                | HostAdmissionSourceKind::PermanentIdentityExhausted
                | HostAdmissionSourceKind::PermanentRegistryInvariantViolation
        ) {
            self.host_admission_source_open = false;
            return HostAdmissionUnavailableObservedOutcome::Stable {
                source: expected_source,
                evidence: prepared.evidence,
            };
        }
        let changed =
            prepared.core.evidence.expected_digest != prepared.core.evidence.observed_digest;
        if !changed
            && matches!(
                &prepared.core.disposition,
                HostAdmissionUnavailableDisposition::Unavailable {
                    permanence: HostAdmissionPermanence::RetryableAfterCandidateChange,
                    ..
                }
            )
        {
            return HostAdmissionUnavailableObservedOutcome::Awaiting {
                source: expected_source,
                evidence: prepared.evidence,
            };
        }
        let publication =
            self.prepare_host_admission_unavailable_publication(prepared.core, changed);
        clear_holder();
        HostAdmissionUnavailableObservedOutcome::Published(
            self.publish_prepared_host_admission_unavailable(publication),
        )
    }

    pub(crate) fn accept_post_commit_authority_transfer(
        &mut self,
        normal: &CleanupBatchToken,
        cause: PostCommitAuthorityCause,
    ) -> PostCommitAuthorityAcceptanceOutcome {
        let expected = self
            .post_commit_receiver_projection(
                normal,
                CorePostCommitCleanupStageProjection::PostCommitCleanup,
            )
            .expect("a post-commit cleanup token retains its frozen Core receiver");
        let observed = self.observe_post_commit_cleanup();
        let exact = observed.receiver.as_ref() == Some(&expected)
            && self.pending_post_commit_terminal.is_none()
            && self.pending_post_commit_settlement_trace.is_none()
            && self.action.is_none()
            && self.current_history_capability.is_none()
            && self.pending_history_preparation_error.is_none()
            && self.prepared_publication_authority.is_none();
        if !exact {
            return PostCommitAuthorityAcceptanceOutcome::Mismatch { expected, observed };
        }

        let mut accepted = expected;
        accepted.stage = CorePostCommitCleanupStageProjection::AuthorityAccepted;
        let current = self
            .current
            .as_mut()
            .expect("the exact post-commit receiver remains current");
        let stage = std::mem::replace(&mut current.stage, WorkStage::Applying);
        let WorkStage::PostCommitCleanup { outstanding } = stage else {
            unreachable!("acceptance consumes only the exact post-commit stage")
        };
        let normal_source = outstanding.source_projection();
        current.stage = WorkStage::PostCommitAuthorityAccepted {
            normal: outstanding,
            expected: accepted.clone(),
        };
        PostCommitAuthorityAcceptanceOutcome::Accepted(CorePostCommitAuthorityTransferOwner {
            receiver: accepted,
            normal_source,
            cause,
        })
    }

    pub(crate) fn prepare_post_commit_authority_transfer(
        &mut self,
        source: CorePostCommitAuthorityPreparationSource,
        manifest: CleanupManifestSeal,
    ) -> PostCommitAuthorityPreparationOutcome {
        let cause = source.transfer.cause;
        let observed = self.observe_post_commit_cleanup();
        let expected = source.transfer.receiver.clone();
        if observed.receiver.as_ref() != Some(&expected) {
            return PostCommitAuthorityPreparationOutcome::Mismatch {
                source,
                evidence: CorePostCommitReceiverMismatchProjection { expected, observed },
            };
        }
        if !source.host.matches(
            self.router,
            &source.transfer.receiver.current_host,
            source.transfer.receiver.current_top_slot,
        ) {
            return PostCommitAuthorityPreparationOutcome::Permanent {
                source,
                evidence: CorePostCommitAuthorityBeginPermanentEvidence {
                    expected,
                    observed,
                    cause,
                    contradiction: CorePostCommitAuthorityContradiction::ForeignReceiver,
                },
            };
        }
        let Some(current) = self.current.as_ref() else {
            return PostCommitAuthorityPreparationOutcome::Mismatch {
                source,
                evidence: CorePostCommitReceiverMismatchProjection { expected, observed },
            };
        };
        if manifest.logical_roots()
            != current
                .target_frames
                .iter()
                .rev()
                .cloned()
                .collect::<Vec<_>>()
        {
            return PostCommitAuthorityPreparationOutcome::Permanent {
                source,
                evidence: CorePostCommitAuthorityBeginPermanentEvidence {
                    expected,
                    observed,
                    cause,
                    contradiction: CorePostCommitAuthorityContradiction::HistoryOrActiveMismatch,
                },
            };
        }
        let batch_id = match self.ids.reserve_cleanup_batch() {
            Ok(batch) => batch,
            Err(_) => {
                return PostCommitAuthorityPreparationOutcome::Permanent {
                    source,
                    evidence: CorePostCommitAuthorityBeginPermanentEvidence {
                        expected,
                        observed,
                        cause,
                        contradiction: CorePostCommitAuthorityContradiction::IdentityExhausted,
                    },
                };
            }
        };
        let authority_batch = CleanupBatchToken::sealed(
            source.transfer.receiver.identity.clone(),
            batch_id,
            manifest.logical_roots(),
            manifest.manifest_digest(),
            manifest.obligation_count(),
        );
        let batch = authority_batch.projection();
        PostCommitAuthorityPreparationOutcome::Prepared {
            owner: CorePreparedPostCommitAuthorityTransfer {
                transfer: source.transfer,
                host: source.host,
                authority_batch,
                authority_manifest: manifest,
            },
            batch,
        }
    }

    pub(crate) fn begin_post_commit_authority_transfer<Unbound, Bound, BindError, Bind, Install>(
        &mut self,
        issuer: &mut HostAdmissionIssuer,
        owner: CorePreparedPostCommitAuthorityTransfer,
        unbound: Unbound,
        bind: Bind,
        install_compound: Install,
    ) -> PostCommitAuthorityBeginCoreOutcome<Unbound, Bound, BindError>
    where
        Bind: FnOnce(
            Unbound,
            &CleanupBatchProjection,
            CorePostCommitSettlementPlanBuilder,
        )
            -> Result<(Bound, CoreBoundPostCommitSettlementPlan), (Unbound, BindError)>,
        Install: FnOnce(Bound, CorePostCommitCompoundBatchOwner),
    {
        let cause = owner.transfer.cause;
        let batch_projection = owner.authority_batch.projection();
        let expected = owner.transfer.receiver.clone();
        let mut cleanup_receiver = owner.transfer.receiver.clone();
        cleanup_receiver.stage = CorePostCommitCleanupStageProjection::AuthorityCleanup;
        let retired_projection = owner.host.retirement_projection();
        let compound_projection = CorePostCommitCompoundProjection {
            receiver: cleanup_receiver.clone(),
            authority_batch: owner.authority_batch.projection(),
            retired_host: retired_projection.clone(),
            cause,
        };
        let current_trace = self
            .request(expected.request)
            .map_or_else(Vec::new, |record| record.trace().to_vec());
        let plan_builder = CorePostCommitSettlementPlanBuilder {
            compound: compound_projection.clone(),
            normal_batch: owner.transfer.normal_source.clone(),
            authority_batch: owner.authority_batch.source_projection(),
            normal_obligation_count: expected.normal_batch.obligation_count,
            authority_obligation_count: owner.authority_batch.obligation_count(),
            current_trace,
        };
        let (bound, settlement_plan) = match bind(unbound, &batch_projection, plan_builder) {
            Ok(bound) => bound,
            Err((unbound, evidence)) => {
                return PostCommitAuthorityBeginCoreOutcome::BindingMismatch {
                    owner,
                    unbound,
                    evidence,
                };
            }
        };

        let observed = self.observe_post_commit_cleanup();
        if observed.receiver.as_ref() != Some(&expected) {
            return PostCommitAuthorityBeginCoreOutcome::ReceiverMismatch {
                owner,
                bound,
                evidence: CorePostCommitReceiverMismatchProjection { expected, observed },
            };
        }
        if observed.competing_cleanup.is_some() {
            return PostCommitAuthorityBeginCoreOutcome::Permanent {
                owner,
                bound,
                evidence: CorePostCommitAuthorityBeginPermanentEvidence {
                    expected,
                    observed,
                    cause,
                    contradiction: CorePostCommitAuthorityContradiction::CompetingSameSourceOwner,
                },
            };
        }
        let authority_state_is_ready = self.state.availability()
            == &crate::presentation::RouterAvailability::Ready
            && self.state.host() == Some(&owner.transfer.receiver.current_host)
            && self.state.top_slot() == Some(owner.transfer.receiver.current_top_slot);
        if !authority_state_is_ready {
            return PostCommitAuthorityBeginCoreOutcome::Permanent {
                owner,
                bound,
                evidence: CorePostCommitAuthorityBeginPermanentEvidence {
                    expected,
                    observed,
                    cause,
                    contradiction: CorePostCommitAuthorityContradiction::RetiredHostMismatch,
                },
            };
        }

        let normal = match self.current.as_ref().map(|current| &current.stage) {
            Some(WorkStage::PostCommitAuthorityAccepted { normal, .. }) => normal.clone(),
            _ => {
                return PostCommitAuthorityBeginCoreOutcome::ReceiverMismatch {
                    owner,
                    bound,
                    evidence: CorePostCommitReceiverMismatchProjection { expected, observed },
                };
            }
        };
        let live_trace = self
            .request(expected.request)
            .map(|record| record.trace())
            .unwrap_or_default();
        let exact_plan_source = settlement_plan.compound == compound_projection
            && settlement_plan.normal_batch == normal.source_projection()
            && settlement_plan.authority_batch == owner.authority_batch.source_projection()
            && settlement_plan.prior_trace_digest == navigation_trace_digest(live_trace);
        if !exact_plan_source {
            return PostCommitAuthorityBeginCoreOutcome::Permanent {
                owner,
                bound,
                evidence: CorePostCommitAuthorityBeginPermanentEvidence {
                    expected,
                    observed,
                    cause,
                    contradiction: CorePostCommitAuthorityContradiction::HistoryOrActiveMismatch,
                },
            };
        }
        let terminal = match self.prepare_post_commit_terminal_plan(
            &normal,
            &owner.authority_batch,
            cause,
            &owner.transfer.receiver.current_host,
            owner.transfer.receiver.current_top_slot,
        ) {
            Ok(terminal) => terminal,
            Err(CorePostCommitTerminalPreparationFailure::Retryable(missing_capacity)) => {
                return PostCommitAuthorityBeginCoreOutcome::Retryable {
                    owner,
                    bound,
                    evidence: CorePostCommitAuthorityBeginRetryEvidence {
                        expected,
                        observed,
                        cause,
                        missing_capacity,
                    },
                };
            }
            Err(CorePostCommitTerminalPreparationFailure::Permanent(contradiction)) => {
                return PostCommitAuthorityBeginCoreOutcome::Permanent {
                    owner,
                    bound,
                    evidence: CorePostCommitAuthorityBeginPermanentEvidence {
                        expected,
                        observed,
                        cause,
                        contradiction,
                    },
                };
            }
        };

        let installed_projection = compound_projection.clone();
        let host_expected = CoreRetiredHostAuthorityExpectedProjection {
            receiver: cleanup_receiver,
            retired_host: retired_projection.clone(),
            cause,
        };
        let expected_physical_digest = post_commit_host_physical_digest(&host_expected.receiver);
        let recovering_state = terminal.recovering_state.clone();
        let recovering_records = terminal.recovering_records.clone();
        let settlement_event_start = recovering_records
            .iter()
            .find(|record| record.id() == normal.identity().request())
            .map_or(0, |record| record.trace().len());
        let settlement =
            CorePostCommitSettlementCapability::from_plan(settlement_plan, settlement_event_start);
        let settlement_trace = settlement.projection.clone();

        let retired_host = match issuer.retire_current(owner.host) {
            Ok(retired) => retired,
            Err(error) => {
                return PostCommitAuthorityBeginCoreOutcome::Permanent {
                    owner: CorePreparedPostCommitAuthorityTransfer {
                        transfer: owner.transfer,
                        host: error.into_owner(),
                        authority_batch: owner.authority_batch,
                        authority_manifest: owner.authority_manifest,
                    },
                    bound,
                    evidence: CorePostCommitAuthorityBeginPermanentEvidence {
                        expected,
                        observed,
                        cause,
                        contradiction: CorePostCommitAuthorityContradiction::RetiredHostMismatch,
                    },
                };
            }
        };
        let compound = CorePostCommitCompoundBatchOwner {
            receiver: compound_projection,
            authority_batch: owner.authority_batch,
            host_edge: CoreRetiredHostEdgeSettlementOwner {
                authority: retired_host,
                expected: host_expected,
                expected_physical_digest,
            },
            settlement: Some(settlement),
        };

        install_compound(bound, compound);

        let current = self
            .current
            .as_mut()
            .expect("the post-bind receiver was exact before host retirement");
        let stage = std::mem::replace(&mut current.stage, WorkStage::Applying);
        let WorkStage::PostCommitAuthorityAccepted { normal, .. } = stage else {
            unreachable!("no fallible work occurs after host retirement")
        };
        current.stage = WorkStage::PostCommitAuthorityCleanup {
            normal,
            expected: installed_projection,
        };
        self.state = recovering_state;
        self.records = recovering_records;
        self.queue.clear();
        self.pending_post_commit_terminal = Some(terminal);
        self.pending_post_commit_settlement_trace = Some(settlement_trace);
        PostCommitAuthorityBeginCoreOutcome::Begun
    }

    fn freeze_committed_history(&self) -> FrozenCommittedHistoryProjection {
        let snapshot = self.history.snapshot();
        FrozenCommittedHistoryProjection {
            entries: snapshot.entries().to_vec(),
            cursor: snapshot.cursor(),
            digest: committed_history_digest(snapshot.entries(), snapshot.cursor()),
        }
    }

    fn post_commit_receiver_projection(
        &self,
        normal: &CleanupBatchToken,
        stage: CorePostCommitCleanupStageProjection,
    ) -> Option<CorePostCommitCleanupReceiverProjection> {
        let current = self.current.as_ref()?;
        let current_host = self.state.host()?.clone();
        let current_top_slot = self.state.top_slot()?;
        Some(CorePostCommitCleanupReceiverProjection {
            router: self.router,
            request: current.identity.request,
            identity: current.identity.clone(),
            normal_batch: normal.projection(),
            current_host,
            current_top_slot,
            committed_history: self.freeze_committed_history(),
            active: FrozenActiveBranchProjection {
                identity: current.identity.clone(),
                presentation: self.state.active().clone(),
                frames: self.state.active().frames().to_vec(),
            },
            stage,
        })
    }

    fn observe_post_commit_cleanup(&self) -> CorePostCommitCleanupObservedProjection {
        let (receiver, retired_host) = match self.current.as_ref().map(|current| &current.stage) {
            Some(WorkStage::PostCommitCleanup { outstanding }) => (
                self.post_commit_receiver_projection(
                    outstanding,
                    CorePostCommitCleanupStageProjection::PostCommitCleanup,
                ),
                None,
            ),
            Some(WorkStage::PostCommitAuthorityAccepted { normal, .. }) => (
                self.post_commit_receiver_projection(
                    normal,
                    CorePostCommitCleanupStageProjection::AuthorityAccepted,
                ),
                None,
            ),
            Some(WorkStage::PostCommitAuthorityCleanup { normal, expected }) => (
                self.post_commit_receiver_projection(
                    normal,
                    CorePostCommitCleanupStageProjection::AuthorityCleanup,
                ),
                Some(expected.retired_host.clone()),
            ),
            _ => (None, None),
        };
        CorePostCommitCleanupObservedProjection {
            receiver,
            retired_host,
            competing_cleanup: None,
        }
    }

    fn observe_post_commit_compound(&self) -> Option<CorePostCommitCompoundProjection> {
        let current = self.current.as_ref()?;
        let WorkStage::PostCommitAuthorityCleanup { normal, expected } = &current.stage else {
            return None;
        };
        let receiver = self.post_commit_receiver_projection(
            normal,
            CorePostCommitCleanupStageProjection::AuthorityCleanup,
        )?;
        Some(CorePostCommitCompoundProjection {
            receiver,
            authority_batch: expected.authority_batch.clone(),
            retired_host: expected.retired_host.clone(),
            cause: expected.cause,
        })
    }

    fn post_commit_settlement_trace_mismatch(
        &self,
        batch: &CorePostCommitCompoundBatchOwner,
        kind: CorePostCommitSettlementTraceMismatchKind,
    ) -> CorePostCommitSettlementTraceMismatchEvidence {
        let submitted_trace = batch.settlement_projection();
        self.post_commit_settlement_trace_mismatch_from(batch, submitted_trace, kind)
    }

    fn post_commit_settlement_trace_mismatch_from(
        &self,
        batch: &CorePostCommitCompoundBatchOwner,
        submitted_trace: CorePostCommitSettlementTraceProjection,
        kind: CorePostCommitSettlementTraceMismatchKind,
    ) -> CorePostCommitSettlementTraceMismatchEvidence {
        let expected_next = self
            .pending_post_commit_settlement_trace
            .as_ref()
            .and_then(|trace| trace.expected.get(trace.cursor))
            .cloned();
        let submitted_next = submitted_trace
            .expected
            .get(submitted_trace.cursor)
            .cloned();
        CorePostCommitSettlementTraceMismatchEvidence {
            expected: self.pending_post_commit_settlement_trace.clone(),
            observed_receiver: self.observe_post_commit_compound(),
            submitted: batch.projection(),
            submitted_normal: submitted_trace.binding.normal_batch.clone(),
            submitted_trace,
            expected_next,
            submitted_next,
            kind,
        }
    }

    pub(crate) fn prepare_next_post_commit_settlement(
        &mut self,
        mut batch: CorePostCommitCompoundBatchOwner,
    ) -> CorePostCommitSettlementPermitOutcome {
        let submitted_trace = batch.settlement_projection();
        let Some(expected_trace) = self.pending_post_commit_settlement_trace.as_ref() else {
            let evidence = self.post_commit_settlement_trace_mismatch(
                &batch,
                CorePostCommitSettlementTraceMismatchKind::TraceUnavailable,
            );
            return CorePostCommitSettlementPermitOutcome::Mismatch {
                owner: batch,
                evidence,
            };
        };
        let exact_owner = expected_trace == &submitted_trace
            && submitted_trace.binding.compound == batch.projection()
            && submitted_trace.binding.authority_batch == batch.authority_batch.source_projection()
            && submitted_trace.binding.compound.retired_host
                == batch.host_edge.authority.projection();
        if !exact_owner {
            let evidence = self.post_commit_settlement_trace_mismatch(
                &batch,
                CorePostCommitSettlementTraceMismatchKind::WrongOwner,
            );
            return CorePostCommitSettlementPermitOutcome::Mismatch {
                owner: batch,
                evidence,
            };
        }
        let observed_receiver = self.observe_post_commit_compound();
        let live_normal_is_exact = matches!(
            self.current.as_ref().map(|current| &current.stage),
            Some(WorkStage::PostCommitAuthorityCleanup { normal, .. })
                if normal.source_projection() == submitted_trace.binding.normal_batch
        );
        if observed_receiver.as_ref() != Some(&submitted_trace.binding.compound)
            || !live_normal_is_exact
        {
            let evidence = self.post_commit_settlement_trace_mismatch(
                &batch,
                CorePostCommitSettlementTraceMismatchKind::Receiver,
            );
            return CorePostCommitSettlementPermitOutcome::Mismatch {
                owner: batch,
                evidence,
            };
        }
        let Some(expected_next) = submitted_trace
            .expected
            .get(submitted_trace.cursor)
            .cloned()
        else {
            let evidence = self.post_commit_settlement_trace_mismatch(
                &batch,
                CorePostCommitSettlementTraceMismatchKind::Replay,
            );
            return CorePostCommitSettlementPermitOutcome::Mismatch {
                owner: batch,
                evidence,
            };
        };
        let capability = batch
            .settlement
            .take()
            .expect("the submitted projection came from the sole linear capability");
        let binding = post_commit_settlement_permit_digest(&submitted_trace, &expected_next);
        CorePostCommitSettlementPermitOutcome::Prepared(CorePreparedPostCommitSettlement {
            owner: batch,
            permit: CorePostCommitSettlementPermit {
                capability,
                expected: expected_next,
                binding,
            },
        })
    }

    pub(crate) fn record_next_post_commit_settlement<Physical, BindError, Bind>(
        &mut self,
        mut attempt: CorePreparedPostCommitSettlement,
        physical: Physical,
        bind: Bind,
    ) -> CorePostCommitSettlementRecordOutcome<Physical, BindError>
    where
        Bind:
            FnOnce(
                Physical,
                &CorePostCommitSettlementPermit,
            )
                -> Result<CorePostCommitSettlementAttestation<Physical>, (Physical, BindError)>,
    {
        let submitted_trace = attempt.permit.capability.projection.clone();
        let expected_next = attempt.permit.expected.clone();
        let expected_trace_is_exact = self
            .pending_post_commit_settlement_trace
            .as_ref()
            .is_some_and(|expected| expected == &submitted_trace);
        let observed_receiver = self.observe_post_commit_compound();
        let live_normal_is_exact = matches!(
            self.current.as_ref().map(|current| &current.stage),
            Some(WorkStage::PostCommitAuthorityCleanup { normal, .. })
                if normal.source_projection() == submitted_trace.binding.normal_batch
        );
        let exact_owner = submitted_trace.binding.compound == attempt.owner.projection()
            && submitted_trace.binding.authority_batch
                == attempt.owner.authority_batch.source_projection()
            && submitted_trace.binding.compound.retired_host
                == attempt.owner.host_edge.authority.projection();
        if !expected_trace_is_exact
            || !exact_owner
            || observed_receiver.as_ref() != Some(&submitted_trace.binding.compound)
            || !live_normal_is_exact
        {
            let kind = if !expected_trace_is_exact || !exact_owner {
                CorePostCommitSettlementTraceMismatchKind::WrongOwner
            } else {
                CorePostCommitSettlementTraceMismatchKind::Receiver
            };
            let evidence = self.post_commit_settlement_trace_mismatch_from(
                &attempt.owner,
                submitted_trace,
                kind,
            );
            return CorePostCommitSettlementRecordOutcome::Mismatch {
                attempt,
                physical,
                evidence,
            };
        }
        let request = submitted_trace.binding.compound.receiver.request;
        let public_tail_is_exact = self
            .records
            .iter()
            .find(|record| record.id() == request)
            .and_then(|record| record.trace().get(submitted_trace.settlement_event_start..))
            .is_some_and(|events| {
                events.len() == submitted_trace.recorded.len()
                    && events
                        .iter()
                        .zip(&submitted_trace.recorded)
                        .all(|(event, recorded)| {
                            event == &NavigationTraceEvent::cleanup_settled(recorded.disposition)
                        })
            });
        if !public_tail_is_exact {
            let evidence = self.post_commit_settlement_trace_mismatch_from(
                &attempt.owner,
                submitted_trace,
                CorePostCommitSettlementTraceMismatchKind::TraceUnavailable,
            );
            return CorePostCommitSettlementRecordOutcome::Mismatch {
                attempt,
                physical,
                evidence,
            };
        }

        let attestation = match bind(physical, &attempt.permit) {
            Ok(attestation) => attestation,
            Err((physical, bind_error)) => {
                return CorePostCommitSettlementRecordOutcome::BindingMismatch {
                    attempt,
                    physical,
                    bind_error,
                };
            }
        };
        if attestation.permit_binding != attempt.permit.binding {
            let CorePostCommitSettlementAttestation {
                physical,
                permit_binding,
            } = attestation;
            return CorePostCommitSettlementRecordOutcome::AttestationMismatch {
                evidence: CorePostCommitSettlementAttestationMismatchEvidence {
                    expected_permit_binding: attempt.permit.binding,
                    observed_permit_binding: permit_binding,
                },
                attempt,
                physical,
            };
        }

        let mut next_trace = submitted_trace;
        next_trace.cursor += 1;
        next_trace.recorded.push(expected_next.clone());
        next_trace.digest = post_commit_settlement_trace_digest(&next_trace);
        let mut next_records = self.records.clone();
        if Self::append_cleanup_settlement_trace_record(
            &mut next_records,
            request,
            expected_next.disposition,
        )
        .is_err()
        {
            let CorePostCommitSettlementAttestation { physical, .. } = attestation;
            let evidence = self.post_commit_settlement_trace_mismatch_from(
                &attempt.owner,
                attempt.permit.capability.projection.clone(),
                CorePostCommitSettlementTraceMismatchKind::TraceUnavailable,
            );
            return CorePostCommitSettlementRecordOutcome::Mismatch {
                attempt,
                physical,
                evidence,
            };
        }
        attempt.permit.capability.projection = next_trace.clone();
        attempt.owner.settlement = Some(attempt.permit.capability);
        self.records = next_records;
        self.pending_post_commit_settlement_trace = Some(next_trace.clone());
        let CorePostCommitSettlementAttestation { physical, .. } = attestation;
        drop(physical);
        CorePostCommitSettlementRecordOutcome::Recorded {
            owner: attempt.owner,
            trace: next_trace,
        }
    }

    fn prepare_post_commit_terminal_plan(
        &self,
        normal: &CleanupBatchToken,
        _authority: &CleanupBatchToken,
        cause: PostCommitAuthorityCause,
        current_host: &HostVersion,
        current_top_slot: RouteSlotId,
    ) -> Result<PreparedPostCommitTerminalPlan, CorePostCommitTerminalPreparationFailure> {
        let request = normal.identity().request();
        let history_before = self.freeze_committed_history();
        let recovering_record =
            NavigationTraceEvent::request_phase_changed(NavigationPhase::Recovering);
        let aggregate_cleanup = NavigationTraceEvent::cleanup_settled(None);
        let diagnostic = DiagnosticEvent::post_commit_cleanup_anomaly(
            request,
            cause.into(),
            "post-commit host authority was lost during cleanup",
        );
        let request_terminal = NavigationTraceEvent::request_terminal();

        let mut recovering_state = self.state.clone();
        assert!(
            recovering_state.begin_authority_recovery(current_host, current_top_slot),
            "the terminal plan is built only after exact Ready host authority preflight"
        );
        recovering_state.set_request(Some(RequestStatus::Running(NavigationPhase::Recovering)));
        let mut recovering_records = self.records.clone();
        let Some(record) = recovering_records
            .iter_mut()
            .find(|record| record.id() == request)
        else {
            return Err(CorePostCommitTerminalPreparationFailure::Permanent(
                CorePostCommitAuthorityContradiction::RequestOrQueueMismatch,
            ));
        };
        if record
            .transition(RequestStatus::Running(NavigationPhase::Recovering))
            .is_err()
        {
            return Err(CorePostCommitTerminalPreparationFailure::Permanent(
                CorePostCommitAuthorityContradiction::RequestOrQueueMismatch,
            ));
        }
        if record.append_trace(recovering_record.clone()).is_err() {
            return Err(CorePostCommitTerminalPreparationFailure::Permanent(
                CorePostCommitAuthorityContradiction::TraceMismatch,
            ));
        }

        let mut queued_terminals = Vec::new();
        if queued_terminals
            .try_reserve_exact(self.queue.len())
            .is_err()
        {
            return Err(CorePostCommitTerminalPreparationFailure::Retryable(
                CorePostCommitCapacityKind::PreparedTrace,
            ));
        }
        for queued in &self.queue {
            let Some(record) = recovering_records
                .iter_mut()
                .find(|record| record.id() == queued.request)
            else {
                return Err(CorePostCommitTerminalPreparationFailure::Permanent(
                    CorePostCommitAuthorityContradiction::RequestOrQueueMismatch,
                ));
            };
            if record
                .transition(RequestStatus::Failed(NavigationFailure::new(
                    NavigationFailureKind::HostLost,
                    "host was lost after another request committed",
                )))
                .is_err()
            {
                return Err(CorePostCommitTerminalPreparationFailure::Permanent(
                    CorePostCommitAuthorityContradiction::RequestOrQueueMismatch,
                ));
            }
            let terminal = NavigationTraceEvent::request_terminal();
            if record.append_trace(terminal.clone()).is_err() {
                return Err(CorePostCommitTerminalPreparationFailure::Permanent(
                    CorePostCommitAuthorityContradiction::TraceMismatch,
                ));
            }
            queued_terminals.push(terminal);
        }

        let mut final_navigation_state = recovering_state.clone();
        final_navigation_state.terminalize_post_commit_host_loss();
        let mut final_records = recovering_records.clone();
        let Some(record) = final_records
            .iter_mut()
            .find(|record| record.id() == request)
        else {
            return Err(CorePostCommitTerminalPreparationFailure::Permanent(
                CorePostCommitAuthorityContradiction::RequestOrQueueMismatch,
            ));
        };
        if record.append_trace(aggregate_cleanup.clone()).is_err() {
            return Err(CorePostCommitTerminalPreparationFailure::Permanent(
                CorePostCommitAuthorityContradiction::TraceMismatch,
            ));
        }
        if record.transition(RequestStatus::Succeeded).is_err() {
            return Err(CorePostCommitTerminalPreparationFailure::Permanent(
                CorePostCommitAuthorityContradiction::RequestOrQueueMismatch,
            ));
        }
        if record.push_diagnostic(diagnostic.clone()).is_err()
            || record.append_trace(request_terminal.clone()).is_err()
        {
            return Err(CorePostCommitTerminalPreparationFailure::Permanent(
                CorePostCommitAuthorityContradiction::TraceMismatch,
            ));
        }

        let mut final_completed_cleanup = self.completed_cleanup.clone();
        if final_completed_cleanup.try_reserve(2).is_err() {
            return Err(CorePostCommitTerminalPreparationFailure::Retryable(
                CorePostCommitCapacityKind::PreparedTerminal,
            ));
        }

        let mut final_router_diagnostics = self.router_diagnostics.clone();
        if final_router_diagnostics.try_reserve(1).is_err() {
            return Err(CorePostCommitTerminalPreparationFailure::Retryable(
                CorePostCommitCapacityKind::PreparedTerminal,
            ));
        }
        if final_router_diagnostics.len() == ROUTER_DIAGNOSTIC_RETENTION {
            final_router_diagnostics.remove(0);
        }
        final_router_diagnostics.push(diagnostic.clone());

        let mut final_issued_view_tasks = self.issued_view_tasks.clone();
        let mut final_report_dispositions = self.report_dispositions.clone();
        let mut final_evicted_through = self.evicted_through;
        while final_records
            .iter()
            .filter(|record| is_terminal(record.status()))
            .count()
            > TERMINAL_RETENTION
        {
            let Some(index) = final_records
                .iter()
                .position(|record| is_terminal(record.status()))
            else {
                return Err(CorePostCommitTerminalPreparationFailure::Permanent(
                    CorePostCommitAuthorityContradiction::RequestOrQueueMismatch,
                ));
            };
            let Some(evicted) = final_records.remove(index) else {
                return Err(CorePostCommitTerminalPreparationFailure::Permanent(
                    CorePostCommitAuthorityContradiction::RequestOrQueueMismatch,
                ));
            };
            let evicted_request = evicted.id();
            final_issued_view_tasks.retain(|identity| identity.request != evicted_request);
            final_report_dispositions
                .retain(|record| record.target != DiagnosticTarget::Request(evicted_request));
            final_completed_cleanup.retain(|tombstone| tombstone.request != evicted_request);
            let sequence = evicted_request.sequence();
            final_evicted_through =
                Some(final_evicted_through.map_or(sequence, |current| current.max(sequence)));
        }

        let retain_completed_current = final_records.iter().any(|record| record.id() == request);
        Ok(PreparedPostCommitTerminalPlan {
            cause,
            history_before,
            recovering_record,
            aggregate_cleanup,
            diagnostic,
            request_terminal,
            queued_terminals,
            final_state: crate::presentation::RouterAvailability::Unavailable(
                crate::presentation::RouterUnavailableReason::HostLost,
            ),
            recovering_state,
            recovering_records,
            final_navigation_state,
            final_records,
            final_evicted_through,
            final_issued_view_tasks,
            final_report_dispositions,
            final_completed_cleanup,
            final_router_diagnostics,
            retain_completed_current,
        })
    }

    fn refresh_post_commit_terminal_plan(&mut self) {
        let (
            aggregate_cleanup,
            diagnostic,
            request_terminal,
            mut final_completed_cleanup,
            mut final_router_diagnostics,
        ) = {
            let terminal = self
                .pending_post_commit_terminal
                .as_mut()
                .expect("compound completion retains its terminal plan");
            (
                terminal.aggregate_cleanup.clone(),
                terminal.diagnostic.clone(),
                terminal.request_terminal.clone(),
                std::mem::take(&mut terminal.final_completed_cleanup),
                std::mem::take(&mut terminal.final_router_diagnostics),
            )
        };
        let request = self
            .current
            .as_ref()
            .expect("compound completion retains current work")
            .identity
            .request;
        let mut final_navigation_state = self.state.clone();
        final_navigation_state.terminalize_post_commit_host_loss();
        let mut final_records = self.records.clone();
        let record = final_records
            .iter_mut()
            .find(|record| record.id() == request)
            .expect("the exact recovering request remains retained");
        record
            .append_trace(aggregate_cleanup)
            .expect("the exact recovering trace remains open");
        record
            .transition(RequestStatus::Succeeded)
            .expect("the committed request remains nonterminal until compound settlement");
        record
            .push_diagnostic(diagnostic.clone())
            .expect("diagnostic projection accepts its frozen event");
        record
            .append_trace(request_terminal)
            .expect("terminal status precedes the frozen terminal event");

        final_completed_cleanup.clear();
        final_completed_cleanup.extend(self.completed_cleanup.iter().cloned());
        final_router_diagnostics.clear();
        final_router_diagnostics.extend(self.router_diagnostics.iter().cloned());
        if final_router_diagnostics.len() == ROUTER_DIAGNOSTIC_RETENTION {
            final_router_diagnostics.remove(0);
        }
        final_router_diagnostics.push(diagnostic);

        let mut final_issued_view_tasks = self.issued_view_tasks.clone();
        let mut final_report_dispositions = self.report_dispositions.clone();
        let mut final_evicted_through = self.evicted_through;
        while final_records
            .iter()
            .filter(|record| is_terminal(record.status()))
            .count()
            > TERMINAL_RETENTION
        {
            let index = final_records
                .iter()
                .position(|record| is_terminal(record.status()))
                .expect("terminal retention count has an exact terminal member");
            let evicted = final_records
                .remove(index)
                .expect("the located terminal remains removable");
            let evicted_request = evicted.id();
            final_issued_view_tasks.retain(|identity| identity.request != evicted_request);
            final_report_dispositions
                .retain(|record| record.target != DiagnosticTarget::Request(evicted_request));
            final_completed_cleanup.retain(|tombstone| tombstone.request != evicted_request);
            let sequence = evicted_request.sequence();
            final_evicted_through =
                Some(final_evicted_through.map_or(sequence, |current| current.max(sequence)));
        }
        let retain_completed_current = final_records.iter().any(|record| record.id() == request);

        let terminal = self
            .pending_post_commit_terminal
            .as_mut()
            .expect("the exact terminal plan remains installed");
        terminal.final_navigation_state = final_navigation_state;
        terminal.final_records = final_records;
        terminal.final_evicted_through = final_evicted_through;
        terminal.final_issued_view_tasks = final_issued_view_tasks;
        terminal.final_report_dispositions = final_report_dispositions;
        terminal.final_completed_cleanup = final_completed_cleanup;
        terminal.final_router_diagnostics = final_router_diagnostics;
        terminal.retain_completed_current = retain_completed_current;
    }

    fn freeze_post_commit_completion_source(&self) -> CorePostCommitCompletionSourceProjection {
        CorePostCommitCompletionSourceProjection {
            receiver: self.observe_post_commit_compound(),
            history: self.freeze_committed_history(),
            state: self.state.clone(),
            records: self.records.clone(),
            evicted_through: self.evicted_through,
            issued_view_tasks: self.issued_view_tasks.clone(),
            report_dispositions: self.report_dispositions.clone(),
            completed_cleanup: self
                .completed_cleanup
                .iter()
                .map(|tombstone| CoreCompletedCleanupSourceProjection {
                    request: tombstone.request,
                    token: tombstone.token.source_projection(),
                })
                .collect(),
            router_diagnostics: self.router_diagnostics.clone(),
            active_branch: self
                .active_branch
                .as_ref()
                .map(failure_cleanup_zero_terminal_branch_projection),
            active_child_slots: self.active_child_slots.clone(),
            current: self
                .current
                .as_ref()
                .map(CorePostCommitCurrentSourceProjection::from),
            action_present: self.action.is_some(),
            terminal_plan: self
                .pending_post_commit_terminal
                .as_ref()
                .map(CorePreparedPostCommitTerminalSourceProjection::from),
            settlement_trace: self.pending_post_commit_settlement_trace.clone(),
        }
    }

    pub(crate) fn prepare_post_commit_compound_completion(
        &mut self,
        batch: CorePostCommitCompoundBatchOwner,
        inputs: PostCommitCompoundCompletionInputs,
    ) -> PostCommitCompoundCompletionPreparationOutcome {
        let expected_receiver = batch.receiver.clone();
        let observed_receiver = self.observe_post_commit_compound();
        if observed_receiver.as_ref() != Some(&expected_receiver) {
            return PostCommitCompoundCompletionPreparationOutcome::Mismatch {
                batch,
                inputs,
                evidence: CorePostCommitCompoundMismatchEvidence::Receiver {
                    expected: expected_receiver,
                    observed: observed_receiver,
                },
            };
        }
        let normal_token = match self.current.as_ref().map(|current| &current.stage) {
            Some(WorkStage::PostCommitAuthorityCleanup { normal, .. }) => normal,
            _ => unreachable!("the exact compound receiver retains its normal cleanup owner"),
        };
        let expected = CorePostCommitCompoundCompletionFactsProjection {
            cause: batch.cause(),
            normal: CoreNormalCleanupCompletionFacts {
                batch: normal_token.projection(),
                manifest_digest: normal_token.manifest_digest(),
                completion_digest: inputs.normal.completion_digest(),
                settled_count: inputs.normal.settled_count(),
            },
            authority: CoreAuthorityCleanupCompletionFacts {
                batch: batch.authority_batch.projection(),
                completion_digest: inputs.authority_expected.completion_digest,
                settled_count: batch.authority_batch.obligation_count(),
            },
            host_edge: CoreRetiredHostEdgeCompletionFacts {
                expected: batch.host_edge.expected.clone(),
                physical_digest: batch.host_edge.expected_physical_digest,
            },
        };
        let observed = CorePostCommitCompoundCompletionFactsProjection {
            cause: batch.cause(),
            normal: inputs.normal_facts.clone(),
            authority: inputs.authority.clone(),
            host_edge: inputs.host_edge.clone(),
        };
        let authority_expected_is_bound = inputs.authority_expected.batch
            == batch.authority_batch.projection()
            && inputs.authority_expected.settled_count == batch.authority_batch.obligation_count();
        if expected != observed
            || !authority_expected_is_bound
            || !inputs.normal.matches(normal_token)
        {
            return PostCommitCompoundCompletionPreparationOutcome::Mismatch {
                batch,
                inputs,
                evidence: CorePostCommitCompoundMismatchEvidence::CompletionFacts {
                    expected,
                    observed,
                },
            };
        }

        let trace_expected = CorePostCommitSettlementTraceCompletionExpectedProjection {
            compound: batch.projection(),
            cause: batch.cause(),
            plan: batch.settlement_projection(),
            required_progress: CorePostCommitSettlementTraceProgress::Complete,
        };
        let trace_observed = self.pending_post_commit_settlement_trace.clone();
        let trace_is_exact = self
            .pending_post_commit_settlement_trace
            .as_ref()
            .is_some_and(|trace| {
                let binding = &trace.binding;
                let original_residual_count = trace
                    .expected
                    .iter()
                    .filter(|entry| entry.role == CorePostCommitSettlementRole::OriginalResidual)
                    .count();
                let authority_non_top_count = trace
                    .expected
                    .iter()
                    .filter(|entry| entry.role == CorePostCommitSettlementRole::AuthorityNonTop)
                    .count();
                let retired_host_count = trace
                    .expected
                    .iter()
                    .filter(|entry| entry.role == CorePostCommitSettlementRole::RetiredHostEdge)
                    .count();
                let authority_top_count = trace
                    .expected
                    .iter()
                    .filter(|entry| entry.role == CorePostCommitSettlementRole::AuthorityTop)
                    .count();
                let ordinals_are_exact = trace
                    .expected
                    .iter()
                    .enumerate()
                    .all(|(ordinal, entry)| entry.ordinal == ordinal);
                let recorded_are_exact = trace.recorded == trace.expected;
                let public_trace_is_exact = self
                    .records
                    .iter()
                    .find(|record| record.id() == batch.receiver.receiver.request)
                    .is_some_and(|record| {
                        record
                            .trace()
                            .get(trace.settlement_event_start..)
                            .is_some_and(|events| {
                                events.len() == trace.recorded.len()
                                    && events.iter().zip(&trace.recorded).all(|(event, entry)| {
                                        event
                                            == &NavigationTraceEvent::cleanup_settled(
                                                entry.disposition,
                                            )
                                    })
                            })
                    });

                binding.compound == batch.projection()
                    && binding.normal_batch == normal_token.source_projection()
                    && binding.authority_batch == batch.authority_batch.source_projection()
                    && binding.compound.retired_host == batch.host_edge.authority.projection()
                    && binding.compound.cause == batch.cause()
                    && binding.settled_original.len() + original_residual_count
                        == inputs.normal.settled_count()
                    && authority_non_top_count + authority_top_count
                        == inputs.authority.settled_count
                    && retired_host_count == 1
                    && authority_top_count == 1
                    && trace.is_complete()
                    && ordinals_are_exact
                    && recorded_are_exact
                    && public_trace_is_exact
                    && trace == &inputs.settlement
                    && batch
                        .settlement
                        .as_ref()
                        .is_some_and(|capability| trace == &capability.projection)
            });
        if !trace_is_exact {
            return PostCommitCompoundCompletionPreparationOutcome::Mismatch {
                batch,
                inputs,
                evidence: CorePostCommitCompoundMismatchEvidence::SettlementTrace {
                    expected: trace_expected,
                    observed: trace_observed,
                },
            };
        }

        self.refresh_post_commit_terminal_plan();
        let source = self.freeze_post_commit_completion_source();

        PostCommitCompoundCompletionPreparationOutcome::Prepared(
            CorePostCommitCompoundCompletionOwner {
                batch,
                normal: inputs.normal,
                authority: inputs.authority,
                host_edge: inputs.host_edge,
                source,
            },
        )
    }

    /// Trusted integration bridge used only after ECS has consumed its private
    /// non-Clone physical L/A/H completion proof. No caller-supplied authority
    /// or host expected/observed facts cross this boundary: they are derived
    /// from the exact bound Core plan and retired-host owner.
    pub(crate) fn prepare_post_commit_compound_completion_from_trusted_physical(
        &mut self,
        batch: CorePostCommitCompoundBatchOwner,
        normal: CleanupCompletion,
    ) -> PostCommitCompoundCompletionPreparationOutcome {
        let settlement = batch.settlement_projection();
        let authority_expected_count = settlement
            .expected
            .iter()
            .filter(|entry| {
                matches!(
                    entry.role,
                    CorePostCommitSettlementRole::AuthorityNonTop
                        | CorePostCommitSettlementRole::AuthorityTop
                )
            })
            .count();
        let authority_settled_count = settlement
            .recorded
            .iter()
            .filter(|entry| {
                matches!(
                    entry.role,
                    CorePostCommitSettlementRole::AuthorityNonTop
                        | CorePostCommitSettlementRole::AuthorityTop
                )
            })
            .count();
        let authority_expected_completion_digest =
            post_commit_authority_expected_completion_digest(&settlement);
        let authority_completion_digest = post_commit_authority_completion_digest(&settlement);
        let normal_facts = CoreNormalCleanupCompletionFacts {
            batch: normal.token().projection(),
            manifest_digest: normal.token().manifest_digest(),
            completion_digest: normal.completion_digest(),
            settled_count: normal.settled_count(),
        };
        let authority_expected = CoreAuthorityCleanupCompletionFacts {
            batch: batch.authority_batch.projection(),
            completion_digest: CleanupCompletionDigest::from_opaque(
                authority_expected_completion_digest,
            ),
            settled_count: authority_expected_count,
        };
        let authority = CoreAuthorityCleanupCompletionFacts {
            batch: batch.authority_batch.projection(),
            completion_digest: CleanupCompletionDigest::from_opaque(authority_completion_digest),
            settled_count: authority_settled_count,
        };
        let host_edge = CoreRetiredHostEdgeCompletionFacts {
            expected: batch.host_edge.expected.clone(),
            physical_digest: batch.host_edge.expected_physical_digest,
        };
        let inputs = PostCommitCompoundCompletionInputs {
            normal,
            normal_facts,
            authority_expected,
            authority,
            host_edge,
            settlement,
        };
        self.prepare_post_commit_compound_completion(batch, inputs)
    }

    pub(crate) fn complete_post_commit_authority_compound<PhysicalTerminal, F>(
        &mut self,
        owner: CorePostCommitCompoundCompletionOwner,
        clear_physical_holder: F,
    ) -> PostCommitCompoundCompletionCoreOutcome<PhysicalTerminal>
    where
        F: FnOnce(CoreRetiredHostAuthorityExpectedProjection) -> PhysicalTerminal,
    {
        let expected_receiver = owner.batch.receiver.clone();
        let observed_receiver = self.observe_post_commit_compound();
        if observed_receiver.as_ref() != Some(&expected_receiver) {
            return PostCommitCompoundCompletionCoreOutcome::Mismatch {
                owner,
                evidence: CorePostCommitCompoundMismatchEvidence::Receiver {
                    expected: expected_receiver,
                    observed: observed_receiver,
                },
            };
        }

        let observed_source = self.freeze_post_commit_completion_source();
        if observed_source != owner.source {
            let expected_source = owner.source.clone();
            return PostCommitCompoundCompletionCoreOutcome::Mismatch {
                owner,
                evidence: CorePostCommitCompoundMismatchEvidence::Source {
                    expected: expected_source,
                    observed: observed_source,
                },
            };
        }

        let normal_token = match self.current.as_ref().map(|current| &current.stage) {
            Some(WorkStage::PostCommitAuthorityCleanup { normal, .. }) => normal,
            _ => unreachable!("the exact compound receiver retains its normal cleanup owner"),
        };
        let expected_facts = CorePostCommitCompoundCompletionFactsProjection {
            cause: owner.batch.cause(),
            normal: CoreNormalCleanupCompletionFacts {
                batch: normal_token.projection(),
                manifest_digest: normal_token.manifest_digest(),
                completion_digest: owner.normal.completion_digest(),
                settled_count: owner.normal.settled_count(),
            },
            authority: CoreAuthorityCleanupCompletionFacts {
                batch: owner.batch.authority_batch.projection(),
                completion_digest: owner.authority.completion_digest,
                settled_count: owner.batch.authority_batch.obligation_count(),
            },
            host_edge: CoreRetiredHostEdgeCompletionFacts {
                expected: owner.batch.host_edge.expected.clone(),
                physical_digest: owner.batch.host_edge.expected_physical_digest,
            },
        };
        let observed_facts = CorePostCommitCompoundCompletionFactsProjection {
            cause: owner.batch.cause(),
            normal: CoreNormalCleanupCompletionFacts {
                batch: owner.normal.token().projection(),
                manifest_digest: owner.normal.token().manifest_digest(),
                completion_digest: owner.normal.completion_digest(),
                settled_count: owner.normal.settled_count(),
            },
            authority: owner.authority.clone(),
            host_edge: owner.host_edge.clone(),
        };
        let owner_facts_are_exact = expected_facts == observed_facts
            && owner.normal.matches(normal_token)
            && owner.batch.host_edge.authority.projection()
                == owner.batch.host_edge.expected.retired_host;
        if !owner_facts_are_exact {
            return PostCommitCompoundCompletionCoreOutcome::Mismatch {
                owner,
                evidence: CorePostCommitCompoundMismatchEvidence::CompletionFacts {
                    expected: expected_facts,
                    observed: observed_facts,
                },
            };
        }

        let expected_host = owner.batch.host_edge.expected.clone();
        let consumed = owner.batch.host_edge.authority.projection();
        let retired_host = CoreRetiredHostAuthorityTerminalEvidence {
            expected: expected_host.clone(),
            consumed,
        };
        let request = owner.batch.receiver.receiver.request;
        let terminal_plan = self
            .pending_post_commit_terminal
            .as_ref()
            .expect("the exact completion owner retains its prepared terminal plan");
        let trace_digest = terminal_plan
            .final_records
            .iter()
            .find(|record| record.id() == request)
            .map_or(0, |record| navigation_trace_digest(record.trace()));
        let terminal = CorePostCommitTerminalProjection {
            router: self.router,
            request,
            cause: terminal_plan.cause,
            history: terminal_plan.history_before.clone(),
            availability: terminal_plan.final_state.clone(),
            trace_digest,
        };

        let former_host = expected_host.retired_host.version().clone();
        let former_top_slot = expected_host.retired_host.top_slot();

        let physical = clear_physical_holder(expected_host);
        let mut terminal_plan = self
            .pending_post_commit_terminal
            .take()
            .expect("the prevalidated terminal plan remains installed");

        let current = self
            .current
            .take()
            .expect("the prevalidated compound receiver remains current");
        let WorkStage::PostCommitAuthorityCleanup { normal, .. } = current.stage else {
            unreachable!("the completion callback follows exact stage validation")
        };
        let CorePostCommitCompoundCompletionOwner {
            batch,
            normal: normal_completion,
            authority,
            host_edge,
            source: _,
        } = owner;
        let CorePostCommitCompoundBatchOwner {
            receiver: _,
            authority_batch,
            host_edge: host_settlement,
            settlement,
        } = batch;
        if terminal_plan.retain_completed_current {
            terminal_plan
                .final_completed_cleanup
                .push(CompletedCleanupTombstone {
                    request,
                    token: normal,
                });
            terminal_plan
                .final_completed_cleanup
                .push(CompletedCleanupTombstone {
                    request,
                    token: authority_batch,
                });
        } else {
            drop(normal);
            drop(authority_batch);
        }

        self.state = terminal_plan.final_navigation_state;
        self.records = terminal_plan.final_records;
        self.evicted_through = terminal_plan.final_evicted_through;
        self.issued_view_tasks = terminal_plan.final_issued_view_tasks;
        self.report_dispositions = terminal_plan.final_report_dispositions;
        self.completed_cleanup = terminal_plan.final_completed_cleanup;
        self.router_diagnostics = terminal_plan.final_router_diagnostics;
        self.active_branch = None;
        self.active_child_slots.clear();
        self.action = None;
        self.pending_post_commit_settlement_trace = None;
        self.retain_host_loss_admission_source(former_host, former_top_slot);

        drop(normal_completion);
        drop(authority);
        drop(host_edge);
        drop(host_settlement);
        drop(settlement);
        PostCommitCompoundCompletionCoreOutcome::Terminal {
            terminal,
            physical,
            retired_host,
        }
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
        self.begin_authority_settlement_for_cause_retaining(
            issuer,
            lease,
            PreCommitAuthorityCause::AuthorityLost,
            seal,
            authority_mutation,
        )
    }

    pub(crate) fn begin_authority_settlement_for_cause_retaining<F>(
        &mut self,
        issuer: &mut HostAdmissionIssuer,
        lease: HostLease,
        cause: PreCommitAuthorityCause,
        seal: CleanupManifestSeal,
        authority_mutation: F,
    ) -> Result<CleanupBatchToken, BeginAuthoritySettlementError>
    where
        F: FnOnce(&Self, CleanupBatchToken),
    {
        self.begin_authority_settlement_publication(
            issuer,
            lease,
            cause,
            seal,
            None,
            authority_mutation,
        )
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
                if request.receiver.identity.router_identity() != self.router {
                    return Err(FailedAuthorityTriggerError::Foreign);
                }
                if !matches!(
                    &current.stage,
                    WorkStage::FailureCleanupManifestPending { expected }
                        if expected == &request.receiver
                ) {
                    return Err(FailedAuthorityTriggerError::Stale);
                }
                Ok(FailedAuthoritySettlementTerminal::FailureCleanup(
                    request.receiver.clone(),
                ))
            }
            FailedAuthoritySettlementTerminalRef::FailureCleanupProjection(request) => {
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
        self.begin_failed_authority_settlement_for_cause_retaining(
            issuer,
            lease,
            trigger,
            PreCommitAuthorityCause::AuthorityLost,
            seal,
            authority_mutation,
        )
    }

    pub(crate) fn begin_failed_authority_settlement_for_cause_retaining<F>(
        &mut self,
        issuer: &mut HostAdmissionIssuer,
        lease: HostLease,
        trigger: FailedAuthoritySettlementTrigger,
        cause: PreCommitAuthorityCause,
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
            cause,
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
                FailedAuthoritySettlementTerminalRef::FailureCleanupProjection(request)
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
        cause: PreCommitAuthorityCause,
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
        let failure = cause.running_failure();
        let request = FailureCleanupRequest::new(
            current_identity,
            PendingFailureKind::AuthorityLost,
            logical_roots,
            failure,
            FailureCleanupContinuationOwner::AuthoritySettlement,
        );
        if request.logical_roots() != seal.logical_roots() {
            return Err(BeginAuthoritySettlementError::new(
                BeginAuthoritySettlementErrorKind::InvalidManifest,
                lease,
            ));
        }
        let cleanup = CleanupBatchToken::sealed(
            request.receiver.identity.clone(),
            self.current
                .as_ref()
                .expect("authority settlement phase was preflighted")
                .cleanup_batch,
            request.logical_roots(),
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
                .transition(RequestStatus::Failed(cause.queued_failure()))
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
            continuation: FailureCleanupContinuationOwner::AuthoritySettlement,
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
            self.retain_awaiting_admission_source();
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
        self.retain_awaiting_admission_source();
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
            let pending_reserved = reserved_frames[branch_index];
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
                    transaction: reservation.transaction(),
                    reserved: pending_reserved,
                    kind: PendingMountKind::Normal,
                }),
                stage: WorkStage::AwaitingMount,
                cleanup_batch: reservation.cleanup_batch(),
                cleanup_roots,
                attempt: AttemptKind::Normal(NavigationCommandKind::Back),
                reconciliation: Some(reconciliation),
                retired_host: None,
            };
            let failure_continuation =
                self.frozen_failure_recovery_continuation_for(&current, branch_index);
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
                    let (failure, failure_continuation) = self
                        .reserve_frozen_failure_recovery_attempt(
                            NavigationFailure::new(NavigationFailureKind::Mount, message),
                            failure_continuation,
                        );
                    let failure = FailureCleanupRequest::new(
                        current.identity.clone(),
                        PendingFailureKind::MountIssue,
                        current.applied_frames.iter().rev().cloned().collect(),
                        failure,
                        failure_continuation,
                    );
                    current.pending_mount = None;
                    current.stage = WorkStage::FailureCleanupManifestPending {
                        expected: failure.receiver.clone(),
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
            let child_index = report.identity.branch_index + 1;
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
                    let failure = self.begin_failure_cleanup_for_branch(
                        PendingFailureKind::MountIssue,
                        NavigationFailure::new(NavigationFailureKind::Mount, message),
                        child_index,
                    );
                    return AppliedReportTransition::FailureCleanup(failure);
                }
            };
            let current = self.current.as_mut().expect("report was preflighted");
            current.identity = report.identity;
            let pending_reserved = current.reservation[child_index];
            current.pending_mount = Some(PendingMount {
                child_index,
                parent: frame.clone(),
                outlet: outlet.clone(),
                from_preserved_parent: false,
                transaction,
                reserved: pending_reserved,
                kind: PendingMountKind::Normal,
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
        let pending_bound_error_branch_index = self
            .current
            .as_ref()
            .and_then(|current| current.pending_mount.as_ref())
            .filter(|pending| pending.kind == PendingMountKind::BoundError)
            .map(|pending| pending.child_index);
        if pending_bound_error_branch_index.is_some()
            && !self.has_live_mount_context(
                &WorkStage::AwaitingMount,
                Some((request.request, &request.parent, &request.outlet)),
            )
        {
            return Err(MountCapabilityError::new(
                MountError::WrongPhase,
                ResolveMountOwner::Returned(request),
            ));
        }
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
                if let Some(failure_branch_index) = pending_bound_error_branch_index {
                    if let Err(kind) = self.mount.reject(&request.token) {
                        return Err(MountCapabilityError::new(
                            kind,
                            ResolveMountOwner::Returned(request),
                        ));
                    }
                    let failure = self.begin_failure_cleanup_for_branch(
                        PendingFailureKind::MountIssue,
                        NavigationFailure::new(
                            NavigationFailureKind::Mount,
                            "bound error recovery slot allocation exhausted",
                        ),
                        failure_branch_index,
                    );
                    self.action = Some(EngineAction::FailureCleanup(failure));
                    return Err(MountCapabilityError::new(
                        MountError::Exhausted,
                        ResolveMountOwner::Consumed,
                    ));
                }
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
        let failure_branch_index = self
            .current
            .as_ref()
            .and_then(|current| current.pending_mount.as_ref())
            .map(|pending| pending.child_index)
            .expect("live mount context retains the failed child index");
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
        Ok(self.begin_failure_cleanup_for_branch(pending, failure, failure_branch_index))
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
        let (request, parent, outlet, failure_branch_index) = self
            .current
            .as_ref()
            .and_then(|current| {
                current.pending_mount.as_ref().map(|pending| {
                    (
                        current.identity.request,
                        pending.parent.clone(),
                        pending.outlet.clone(),
                        pending.child_index,
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
        Ok(self.begin_failure_cleanup_for_branch(
            PendingFailureKind::Mount(MountRejectionKind::RejectedAfterMutation),
            NavigationFailure::new(
                NavigationFailureKind::Mount,
                "child mount rejected after mutation",
            ),
            failure_branch_index,
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
        if pending.kind == PendingMountKind::BoundError {
            if !self.current_reconciliation_matches_active() {
                return false;
            }
            let bound_is_exact = matches!(
                &current.attempt,
                AttemptKind::Fallback {
                    bound_error: Some(bound),
                    ..
                } if bound.binding.branch_index() == pending.child_index
                    && bound.binding.outlet() == &pending.outlet
            ) && pending.from_preserved_parent
                && pending.transaction == current.identity.transaction
                && pending.reserved.frame_task() == current.identity.frame_task
                && current.reservation.len() == 1
                && current.reservation[0] == pending.reserved
                && current.target_frames.len() == child_index
                && current.target_frames.last() == Some(&pending.parent);
            if !bound_is_exact {
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
            return self.mount.matches_outstanding_context(
                host,
                current.identity.request,
                pending.transaction,
                &pending.parent,
                &pending.outlet,
            );
        }
        let reservation_matches = if pending.from_preserved_parent {
            current
                .reservation
                .get(pending.child_index)
                .is_some_and(|reserved| {
                    reserved == &pending.reserved
                        && reserved.frame_task() == current.identity.frame_task
                })
        } else {
            current.reservation.get(pending.child_index) == Some(&pending.reserved)
                && current
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
            pending.transaction,
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
        match self.prepare_failure_cleanup_retaining(request, seal) {
            FailureCleanupPrepareOutcome::Prepared(owner) => Ok(owner.into_token()),
            FailureCleanupPrepareOutcome::Mismatch { evidence, .. } => Err(evidence.kind),
            FailureCleanupPrepareOutcome::Permanent { .. } => {
                Err(FailureCleanupError::InvalidManifest)
            }
        }
    }

    pub(crate) fn prepare_failure_cleanup_retaining(
        &mut self,
        request: FailureCleanupRequest,
        seal: CleanupManifestSeal,
    ) -> FailureCleanupPrepareOutcome {
        let observed = self
            .current
            .as_ref()
            .and_then(|current| match &current.stage {
                WorkStage::FailureCleanupManifestPending { expected } => Some(expected.clone()),
                WorkStage::Applying
                | WorkStage::AwaitingMount
                | WorkStage::AwaitingReceipt
                | WorkStage::ConfirmingMount
                | WorkStage::ReadyToCommit
                | WorkStage::CommitSealed { .. }
                | WorkStage::PostCommitCleanup { .. }
                | WorkStage::PostCommitAuthorityAccepted { .. }
                | WorkStage::PostCommitAuthorityCleanup { .. }
                | WorkStage::FailureCleanupPrepared { .. }
                | WorkStage::FailureCleanupZeroTerminalPending { .. }
                | WorkStage::FailureCleanupCompleting { .. } => None,
            });
        let mismatch_kind = if request.receiver.identity.router_identity() != self.router {
            Some(FailureCleanupError::Foreign)
        } else if observed.as_ref() != Some(&request.receiver) {
            Some(FailureCleanupError::Stale)
        } else {
            None
        };
        if let Some(kind) = mismatch_kind {
            let expected = request.receiver.clone();
            return FailureCleanupPrepareOutcome::Mismatch {
                request,
                evidence: FailureCleanupReceiverMismatchProjection {
                    kind,
                    expected,
                    observed_router: self.router,
                    observed,
                },
            };
        }
        if request.logical_roots() != seal.logical_roots() {
            let evidence = FailureCleanupManifestContradictionProjection {
                source: request.receiver.clone(),
                expected_logical_roots: request.logical_roots().to_vec(),
                observed_logical_roots: seal.logical_roots().to_vec(),
                submitted_manifest_digest: seal.manifest_digest(),
                submitted_obligation_count: seal.obligation_count(),
            };
            return FailureCleanupPrepareOutcome::Permanent { request, evidence };
        }

        let cleanup = CleanupBatchToken::sealed(
            request.receiver.identity.clone(),
            self.current
                .as_ref()
                .expect("failure cleanup receiver was preflighted")
                .cleanup_batch,
            request.logical_roots(),
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
        FailureCleanupPrepareOutcome::Prepared(CoreFailureCleanupBatchOwner { token: cleanup })
    }

    fn prepare_failure_cleanup_zero_terminal_plan(
        &self,
        request: &FailureCleanupRequest,
    ) -> Option<PreparedFailureTerminal> {
        if !matches!(
            &request.continuation,
            FailureCleanupContinuationOwner::Terminal
                | FailureCleanupContinuationOwner::TerminalRecoveryFailed(_)
        ) || request.failure.kind() != NavigationFailureKind::RecoveryFailed
            || self.current_history_capability.is_some()
            || self.pending_history_preparation_error.is_some()
            || self.prepared_publication_authority.is_some()
        {
            return None;
        }
        let record_index = self
            .records
            .iter()
            .position(|record| record.id() == request.request())?;
        let mut records = self.records.clone();
        let record = records.get_mut(record_index)?;
        record
            .transition(RequestStatus::Failed(request.failure.clone()))
            .ok()?;
        record
            .append_trace(NavigationTraceEvent::request_terminal())
            .ok()?;

        let mut state = self.state.clone();
        state.set_request(None);
        let mut evicted_through = self.evicted_through;
        let mut issued_view_tasks = self.issued_view_tasks.clone();
        let mut report_dispositions = self.report_dispositions.clone();
        let mut completed_cleanup = self.completed_cleanup.clone();
        while records
            .iter()
            .filter(|record| is_terminal(record.status()))
            .count()
            > TERMINAL_RETENTION
        {
            let index = records
                .iter()
                .position(|record| is_terminal(record.status()))?;
            let evicted = records.remove(index)?;
            let evicted_request = evicted.id();
            issued_view_tasks.retain(|identity| identity.request != evicted_request);
            report_dispositions
                .retain(|record| record.target != DiagnosticTarget::Request(evicted_request));
            completed_cleanup.retain(|tombstone| tombstone.request != evicted_request);
            let sequence = evicted_request.sequence();
            evicted_through =
                Some(evicted_through.map_or(sequence, |current| current.max(sequence)));
        }
        let terminal_record = records
            .iter()
            .find(|record| record.id() == request.request())?;
        let projection = FailureCleanupTerminalProjection {
            identity: request.receiver.identity.clone(),
            status: terminal_record.status().clone(),
            failure: request.failure.clone(),
            trace_digest: terminal_record.trace().len() as u64,
        };
        Some(PreparedFailureTerminal {
            projection,
            records,
            state,
            evicted_through,
            issued_view_tasks,
            report_dispositions,
            completed_cleanup,
        })
    }

    fn failure_cleanup_zero_terminal_source_projection(
        &self,
    ) -> FailureCleanupZeroTerminalSourceProjection {
        FailureCleanupZeroTerminalSourceProjection {
            records: self.records.clone(),
            queue: self.queue.iter().map(|queued| queued.request).collect(),
            state: self.state.clone(),
            evicted_through: self.evicted_through,
            issued_view_tasks: self.issued_view_tasks.clone(),
            report_dispositions: self.report_dispositions.clone(),
            completed_cleanup: self.completed_cleanup.clone(),
            current: self.current.as_ref().map(Into::into),
            action_present: self.action.is_some(),
        }
    }

    pub(crate) fn classify_failure_cleanup_permanent<Physical, BindZero>(
        &mut self,
        holder: FailureCleanupPermanentHolder<Physical>,
        bind_zero: BindZero,
    ) -> FailureCleanupPermanentCoreOutcome<Physical>
    where
        BindZero: FnOnce(
            Physical,
            &FailureCleanupZeroTerminalBindingProjection,
        ) -> Result<
            PreparedFailureCleanupZeroPhysical<Physical>,
            (Physical, FailureCleanupZeroPhysicalMismatchProjection),
        >,
    {
        let FailureCleanupPermanentHolder {
            request,
            physical,
            evidence,
        } = holder;
        let receiver_is_exact = request.receiver == evidence.source
            && request.logical_roots() == evidence.expected_logical_roots.as_slice()
            && evidence.expected_logical_roots != evidence.observed_logical_roots
            && self.current.as_ref().is_some_and(|current| {
                current.identity == request.receiver.identity
                    && matches!(
                        &current.stage,
                        WorkStage::FailureCleanupManifestPending { expected }
                            if expected == &request.receiver
                    )
            });
        if !receiver_is_exact {
            return FailureCleanupPermanentCoreOutcome::Mismatch(FailureCleanupPermanentHolder {
                request,
                physical,
                evidence,
            });
        }

        let terminal = if evidence.submitted_obligation_count == 0 {
            self.prepare_failure_cleanup_zero_terminal_plan(&request)
        } else {
            None
        };
        let Some(terminal) = terminal else {
            return FailureCleanupPermanentCoreOutcome::AuthoritySettlement {
                physical,
                owner: FailureCleanupPermanentAuthorityOwner { request, evidence },
            };
        };
        let expected_physical = FailureCleanupZeroPhysicalProjection {
            receiver: request.receiver.clone(),
            manifest_digest: evidence.submitted_manifest_digest,
            obligation_count: evidence.submitted_obligation_count,
        };
        let binding = FailureCleanupZeroTerminalBindingProjection {
            receiver: request.receiver.clone(),
            contradiction: evidence.clone(),
            physical: expected_physical,
        };
        let zero = match bind_zero(physical, &binding) {
            Ok(zero) => zero,
            Err((physical, mismatch)) => {
                return FailureCleanupPermanentCoreOutcome::ZeroMismatch {
                    holder: FailureCleanupPermanentHolder {
                        request,
                        physical,
                        evidence,
                    },
                    evidence: mismatch,
                };
            }
        };
        let FailureCleanupRequest {
            receiver,
            failure,
            continuation,
        } = request;
        self.current
            .as_mut()
            .expect("permanent failure cleanup receiver was preflighted")
            .stage = WorkStage::FailureCleanupZeroTerminalPending {
            expected: binding.clone(),
        };
        let source = self.failure_cleanup_zero_terminal_source_projection();
        FailureCleanupPermanentCoreOutcome::ProvenZero {
            zero,
            owner: FailureCleanupZeroTerminalOwner {
                receiver,
                failure,
                continuation,
                binding,
                source,
                terminal,
            },
        }
    }

    pub(crate) fn complete_failure_cleanup_zero_terminal<Physical, ConsumeZero>(
        &mut self,
        owner: FailureCleanupZeroTerminalOwner,
        zero: PreparedFailureCleanupZeroPhysical<Physical>,
        consume_zero_and_clear_holder: ConsumeZero,
    ) -> FailureCleanupZeroTerminalOutcome<Physical>
    where
        ConsumeZero: FnOnce(PreparedFailureCleanupZeroPhysical<Physical>),
    {
        let observed_source = self.failure_cleanup_zero_terminal_source_projection();
        let exact = observed_source == owner.source
            && owner.binding == zero.binding
            && zero.proof == zero.binding.physical
            && owner.receiver == owner.binding.receiver
            && owner.receiver.identity.router_identity() == self.router
            && matches!(
                &owner.continuation,
                FailureCleanupContinuationOwner::Terminal
                    | FailureCleanupContinuationOwner::TerminalRecoveryFailed(_)
            )
            && owner.failure.kind() == NavigationFailureKind::RecoveryFailed
            && owner.terminal.projection.failure == owner.failure
            && owner.terminal.projection.status == RequestStatus::Failed(owner.failure.clone());
        if !exact {
            let observed_binding = zero.binding.clone();
            return FailureCleanupZeroTerminalOutcome::Mismatch {
                zero,
                evidence: FailureCleanupZeroTerminalMismatchProjection {
                    expected: owner.source.clone(),
                    observed: observed_source,
                    expected_binding: owner.binding.clone(),
                    observed_binding,
                },
                owner,
            };
        }

        let FailureCleanupZeroTerminalOwner {
            receiver: _,
            failure: _,
            continuation: _,
            binding: _,
            source: _,
            terminal,
        } = owner;
        consume_zero_and_clear_holder(zero);
        let PreparedFailureTerminal {
            projection,
            records,
            state,
            evicted_through,
            issued_view_tasks,
            report_dispositions,
            completed_cleanup,
        } = terminal;
        self.records = records;
        self.state = state;
        self.evicted_through = evicted_through;
        self.issued_view_tasks = issued_view_tasks;
        self.report_dispositions = report_dispositions;
        self.completed_cleanup = completed_cleanup;
        self.current = None;
        self.action = None;
        FailureCleanupZeroTerminalOutcome::Terminal(projection)
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
            receiver: request.receiver.clone(),
            failure: request.failure.clone(),
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
            Some(
                WorkStage::PostCommitAuthorityAccepted { .. }
                | WorkStage::PostCommitAuthorityCleanup { .. },
            ) => CoreBackTargetReceiverStageProjection::PostCommitCleanup,
            Some(WorkStage::FailureCleanupManifestPending { expected }) => {
                CoreBackTargetReceiverStageProjection::FailureCleanupManifestPending(
                    expected.clone(),
                )
            }
            Some(WorkStage::FailureCleanupPrepared { .. }) => {
                CoreBackTargetReceiverStageProjection::FailureCleanupPrepared
            }
            Some(WorkStage::FailureCleanupZeroTerminalPending { expected }) => {
                CoreBackTargetReceiverStageProjection::FailureCleanupZeroTerminalPending(
                    expected.receiver.clone(),
                )
            }
            Some(WorkStage::FailureCleanupCompleting { .. }) => {
                CoreBackTargetReceiverStageProjection::FailureCleanupCompleting
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
            Some(AttemptKind::Fallback {
                original_failure, ..
            }) => CoreBackTargetReceiverAttemptProjection::Fallback {
                original_failure: original_failure.clone(),
            },
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
        ) && observed.current.as_ref() == Some(&expected.request.receiver.identity)
            && observed.stage
                == CoreBackTargetReceiverStageProjection::FailureCleanupManifestPending(
                    expected.request.receiver.clone(),
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
        let pending_bound_error = self
            .current
            .as_ref()
            .and_then(|current| current.pending_mount.as_ref())
            .is_some_and(|pending| pending.kind == PendingMountKind::BoundError);
        if pending_bound_error
            && !self.has_live_mount_context(
                &WorkStage::AwaitingMount,
                Some((request.request, &request.parent, &request.outlet)),
            )
        {
            return Err(MountError::WrongPhase);
        }
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
            FailureCleanupContinuationOwner::BeginFallback,
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
        let Some(AttemptKind::Fallback {
            original_failure, ..
        }) = self
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
            stage: current.map(|current| current.stage.projection()),
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

    pub(crate) fn cancel_prepared_publication_for_authority_settlement(
        &mut self,
        prepared: PreparedPublication,
    ) -> Result<(), (PreparedPublication, CommitError)> {
        let validation = match self.prepared_publication_authority.as_ref() {
            Some(PreparedPublicationAuthority::Ready(owner))
                if owner.evidence == prepared.evidence =>
            {
                self.validate_prepared_publication_owner(owner)
            }
            Some(
                PreparedPublicationAuthority::Ready(_) | PreparedPublicationAuthority::Rejected(_),
            )
            | None => Err(CommitError::Stale),
        };
        if let Err(error) = validation {
            return Err((prepared, error));
        }
        let Some(PreparedPublicationAuthority::Ready(owner)) =
            self.prepared_publication_authority.take()
        else {
            unreachable!("validated cancellation retains the exact ready publication owner")
        };
        drop((prepared, owner));
        Ok(())
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
        if self.failure_cleanup_completion_pending() {
            let owner = self
                .prepare_failure_cleanup_completion(completion)
                .map_err(|error| error.kind())?;
            return self
                .complete_prepared_failure_cleanup_with_ledger(owner, (), |()| {})
                .map(|_| ())
                .map_err(|error| error.kind());
        }
        self.complete_cleanup_impl(completion, || {})
    }

    fn failure_cleanup_completion_pending(&self) -> bool {
        self.current.as_ref().is_some_and(|current| {
            matches!(
                &current.stage,
                WorkStage::FailureCleanupPrepared { continuation, .. }
                    if !matches!(
                        continuation,
                        FailureCleanupContinuationOwner::AuthoritySettlement
                    )
            )
        })
    }

    pub(crate) fn prepare_failure_cleanup_completion(
        &mut self,
        completion: CleanupCompletion,
    ) -> Result<FailureCleanupCompletionOwner, FailureCleanupCompletionPrepareError> {
        let expected = FailureCleanupCompletionPreparationReceiverProjection {
            router: completion.token().identity().router_identity(),
            identity: Some(completion.token().identity().clone()),
            stage: FailureCleanupCompletionPreparationStageProjection::FailureCleanupPrepared {
                outstanding: completion.token().clone(),
                authority_settlement: false,
            },
        };
        let observed = FailureCleanupCompletionPreparationReceiverProjection {
            router: self.router,
            identity: self
                .current
                .as_ref()
                .map(|current| current.identity.clone()),
            stage: match self.current.as_ref().map(|current| &current.stage) {
                Some(WorkStage::FailureCleanupPrepared {
                    outstanding,
                    continuation,
                    ..
                }) => FailureCleanupCompletionPreparationStageProjection::FailureCleanupPrepared {
                    outstanding: outstanding.clone(),
                    authority_settlement: matches!(
                        continuation,
                        FailureCleanupContinuationOwner::AuthoritySettlement
                    ),
                },
                Some(stage) => {
                    FailureCleanupCompletionPreparationStageProjection::Other(stage.projection())
                }
                None => FailureCleanupCompletionPreparationStageProjection::Missing,
            },
        };
        let mismatch = || {
            FailureCleanupCompletionPrepareError::new(
                self.cleanup_completion_mismatch(&completion),
                FailureCleanupCompletionPreparationMismatchProjection {
                    expected: expected.clone(),
                    observed: observed.clone(),
                },
            )
        };
        if expected != observed {
            return Err(mismatch());
        }
        let Some(current) = self.current.as_ref() else {
            return Err(mismatch());
        };
        let WorkStage::FailureCleanupPrepared {
            outstanding,
            continuation,
            ..
        } = &current.stage
        else {
            return Err(mismatch());
        };
        if matches!(
            continuation,
            FailureCleanupContinuationOwner::AuthoritySettlement
        ) || !completion.matches(outstanding)
        {
            return Err(mismatch());
        }

        let identity = current.identity.clone();
        let expected = outstanding.clone();
        let current = self
            .current
            .as_mut()
            .expect("failure cleanup completion was preflighted");
        let stage = std::mem::replace(
            &mut current.stage,
            WorkStage::FailureCleanupCompleting {
                outstanding: expected.clone(),
            },
        );
        let WorkStage::FailureCleanupPrepared {
            outstanding,
            failure,
            continuation,
        } = stage
        else {
            unreachable!("failure cleanup completion consumed its preflighted stage")
        };
        Ok(FailureCleanupCompletionOwner {
            router: self.router,
            identity,
            outstanding,
            failure,
            continuation,
        })
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn drift_failure_cleanup_completion_receiver_for_test(&mut self) {
        let current = self
            .current
            .as_mut()
            .expect("failure completion receiver drift requires current work");
        assert!(matches!(
            current.stage,
            WorkStage::FailureCleanupPrepared { .. } | WorkStage::FailureCleanupCompleting { .. }
        ));
        current.identity.branch_index = current.identity.branch_index.saturating_add(1);
    }

    pub(crate) fn complete_prepared_failure_cleanup_with_ledger<Physical, F>(
        &mut self,
        owner: FailureCleanupCompletionOwner,
        physical: Physical,
        commit_ledger_and_clear_holder: F,
    ) -> Result<FailureCleanupCompletionDisposition, FailureCleanupCompletionCommitError<Physical>>
    where
        F: FnOnce(Physical),
    {
        let expected = FailureCleanupCompletionReceiverProjection {
            router: owner.router,
            identity: Some(owner.identity.clone()),
            stage: Some(WorkStageProjection::FailureCleanupCompleting(
                owner.outstanding.clone(),
            )),
        };
        let observed = FailureCleanupCompletionReceiverProjection {
            router: self.router,
            identity: self
                .current
                .as_ref()
                .map(|current| current.identity.clone()),
            stage: self
                .current
                .as_ref()
                .map(|current| current.stage.projection()),
        };
        let exact = owner.router == self.router
            && self.current.as_ref().is_some_and(|current| {
                current.identity == owner.identity
                    && matches!(
                        &current.stage,
                        WorkStage::FailureCleanupCompleting { outstanding }
                            if outstanding == &owner.outstanding
                    )
            });
        if !exact {
            return Err(FailureCleanupCompletionCommitError::new(
                CleanupError::Stale,
                owner,
                physical,
                FailureCleanupCompletionReceiverMismatchProjection { expected, observed },
            ));
        }

        commit_ledger_and_clear_holder(physical);
        let FailureCleanupCompletionOwner {
            router: _,
            identity,
            outstanding,
            failure,
            continuation,
        } = owner;
        let request = identity.request;
        self.record_mut(request)
            .expect("running record is retained")
            .append_trace(NavigationTraceEvent::cleanup_settled(None))
            .expect("cleanup trace is open");
        self.completed_cleanup.push(CompletedCleanupTombstone {
            request,
            token: outstanding,
        });
        let disposition = match continuation {
            FailureCleanupContinuationOwner::BeginBoundError(binding) => {
                let retained_prepared_prefix = !binding.prepared_prefix.is_empty();
                match (
                    retained_prepared_prefix,
                    self.begin_bound_error(request, failure, *binding),
                ) {
                    (true, BoundErrorBeginDisposition::Bound) => {
                        FailureCleanupCompletionDisposition::RetainedPreparedPrefixBound
                    }
                    (true, BoundErrorBeginDisposition::Cleanup) => {
                        FailureCleanupCompletionDisposition::RetainedPreparedPrefixCleanup
                    }
                    (false, _) => FailureCleanupCompletionDisposition::Settled,
                }
            }
            FailureCleanupContinuationOwner::BeginFallback => {
                let _ = self.begin_fallback(request, failure);
                FailureCleanupCompletionDisposition::Settled
            }
            FailureCleanupContinuationOwner::Terminal
            | FailureCleanupContinuationOwner::TerminalRecoveryFailed(_) => {
                let record = self
                    .record_mut(request)
                    .expect("running record is retained");
                record
                    .transition(RequestStatus::Failed(failure))
                    .expect("request is running");
                record
                    .append_trace(NavigationTraceEvent::request_terminal())
                    .expect("terminal status precedes terminal event");
                self.retire_history_authority();
                self.current = None;
                self.state.set_request(None);
                self.retain_terminal_records();
                self.start_next();
                FailureCleanupCompletionDisposition::Settled
            }
            FailureCleanupContinuationOwner::AuthoritySettlement => {
                unreachable!("authority settlement uses its dedicated composed owner")
            }
        };
        Ok(disposition)
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
                    continuation: FailureCleanupContinuationOwner::AuthoritySettlement,
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
                    continuation: FailureCleanupContinuationOwner::AuthoritySettlement,
                } => Some((outstanding.clone(), Some(failure.clone()))),
                WorkStage::Applying
                | WorkStage::AwaitingMount
                | WorkStage::AwaitingReceipt
                | WorkStage::ConfirmingMount
                | WorkStage::ReadyToCommit
                | WorkStage::CommitSealed { .. }
                | WorkStage::PostCommitAuthorityAccepted { .. }
                | WorkStage::PostCommitAuthorityCleanup { .. }
                | WorkStage::FailureCleanupManifestPending { .. }
                | WorkStage::FailureCleanupPrepared { .. }
                | WorkStage::FailureCleanupZeroTerminalPending { .. }
                | WorkStage::FailureCleanupCompleting { .. } => None,
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
        let (completed_token, authority_failure) =
            outstanding.expect("validated cleanup has an outstanding token");
        if let Some(failure) = &authority_failure {
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
            let settled = if failure.kind() == NavigationFailureKind::HostLost {
                self.state.complete_host_loss_settlement(&version, top_slot)
            } else {
                self.state.complete_authority_settlement(&version, top_slot)
            };
            assert!(settled);
            if failure.kind() == NavigationFailureKind::HostLost {
                self.retain_host_loss_admission_source(version, top_slot);
            } else {
                self.retain_awaiting_admission_source();
            }
            self.active_branch = None;
            self.active_child_slots.clear();
            self.retire_history_authority();
            self.current = None;
            self.state.set_request(None);
            self.retain_terminal_records();
            return Ok(());
        }
        let terminal_status = match &current.attempt {
            AttemptKind::Normal(_) => RequestStatus::Succeeded,
            AttemptKind::Fallback {
                original_failure, ..
            } => RequestStatus::Failed(original_failure.clone()),
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

    fn cleanup_completion_mismatch(&self, completion: &CleanupCompletion) -> CleanupError {
        if let Some(tombstone) = self
            .completed_cleanup
            .iter()
            .find(|tombstone| completion.token() == &tombstone.token)
        {
            if completion.matches(&tombstone.token) {
                CleanupError::Duplicate
            } else {
                CleanupError::Invalid
            }
        } else if completion.is_well_formed() {
            CleanupError::Stale
        } else {
            CleanupError::Invalid
        }
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
                | WorkStage::PostCommitAuthorityAccepted { .. }
                | WorkStage::PostCommitAuthorityCleanup { .. }
                | WorkStage::FailureCleanupManifestPending { .. }
                | WorkStage::FailureCleanupPrepared { .. }
                | WorkStage::FailureCleanupZeroTerminalPending { .. }
                | WorkStage::FailureCleanupCompleting { .. } => None,
            })
            .ok_or(CleanupError::Stale)?;
        Self::append_cleanup_settlement_trace_record(&mut self.records, request, Some(disposition))
            .map_err(|()| CleanupError::Invalid)?;
        Ok(())
    }

    fn append_cleanup_settlement_trace_record(
        records: &mut VecDeque<NavigationRequestRecord>,
        request: NavigationRequestId,
        disposition: Option<crate::trace::CleanupDisposition>,
    ) -> Result<(), ()> {
        let record = records
            .iter_mut()
            .find(|record| record.id() == request)
            .ok_or(())?;
        record
            .append_trace(NavigationTraceEvent::cleanup_settled(disposition))
            .map_err(|_| ())?;
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
            let target_is_exact = match (&current.attempt, current.branch.as_ref()) {
                (AttemptKind::Normal(_), Some(branch)) => projection.matches_matched_target(branch),
                (
                    AttemptKind::Fallback {
                        bound_error: Some(bound_error),
                        ..
                    },
                    None,
                ) => {
                    self.request(current.identity.request)
                        .and_then(NavigationRequestRecord::matched)
                        .is_some_and(|target| {
                            projection.matches_bound_error_target(
                                target,
                                bound_error.binding.branch_index(),
                                bound_error.binding.boundary_route(),
                                current.identity.frame,
                                current.identity.generation,
                            )
                        })
                        && current.identity.origin
                            == FrameOrigin::RouteError(bound_error.binding.boundary_route())
                }
                (AttemptKind::Normal(_), None)
                | (AttemptKind::Fallback { .. }, Some(_))
                | (
                    AttemptKind::Fallback {
                        bound_error: None, ..
                    },
                    None,
                ) => false,
            };
            (current.identity == *identity
                && target_is_exact
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
            let pending_reserved = reserved_frames[branch_index];
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
                    transaction: reservation.transaction(),
                    reserved: pending_reserved,
                    kind: PendingMountKind::Normal,
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
                    let failure = self.begin_failure_cleanup_for_branch(
                        PendingFailureKind::MountIssue,
                        NavigationFailure::new(NavigationFailureKind::Mount, message),
                        branch_index,
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
        let failure_branch_index = self
            .current
            .as_ref()
            .expect("a failure belongs to current work")
            .identity
            .branch_index;
        self.begin_failure_cleanup_for_branch(expected, failure, failure_branch_index)
    }

    fn begin_failure_cleanup_for_branch(
        &mut self,
        expected: PendingFailureKind,
        failure: NavigationFailure,
        failure_branch_index: usize,
    ) -> FailureCleanupRequest {
        let failure = if self
            .current
            .as_ref()
            .is_some_and(|current| matches!(current.attempt, AttemptKind::Fallback { .. }))
        {
            NavigationFailure::new(
                NavigationFailureKind::RecoveryFailed,
                "bound error or router fallback recovery failed",
            )
        } else {
            failure
        };
        let (failure, continuation) = if expected == PendingFailureKind::AuthorityLost {
            (failure, FailureCleanupContinuationOwner::Terminal)
        } else if self
            .current
            .as_ref()
            .is_some_and(|current| matches!(current.attempt, AttemptKind::Normal(_)))
        {
            self.frozen_failure_recovery_continuation(failure, failure_branch_index)
        } else {
            (failure, FailureCleanupContinuationOwner::Terminal)
        };
        self.begin_failure_cleanup_with_continuation(expected, failure, continuation)
    }

    fn frozen_failure_recovery_continuation(
        &mut self,
        failure: NavigationFailure,
        failure_branch_index: usize,
    ) -> (NavigationFailure, FailureCleanupContinuationOwner) {
        let continuation = {
            let Some(current) = self.current.as_ref() else {
                return (failure, FailureCleanupContinuationOwner::Terminal);
            };
            self.frozen_failure_recovery_continuation_for(current, failure_branch_index)
        };
        self.reserve_frozen_failure_recovery_attempt(failure, continuation)
    }

    fn reserve_frozen_failure_recovery_attempt(
        &mut self,
        failure: NavigationFailure,
        continuation: FailureCleanupContinuationOwner,
    ) -> (NavigationFailure, FailureCleanupContinuationOwner) {
        match continuation {
            FailureCleanupContinuationOwner::BeginBoundError(mut recovery) => {
                let reserved = match self.ids.reserve_attempt_with_evidence() {
                    Ok(reserved) => reserved,
                    Err(exhaustion) => {
                        let terminal_failure = NavigationFailure::new(
                            NavigationFailureKind::RecoveryFailed,
                            "bound error recovery attempt identity exhausted",
                        );
                        let owner = BoundErrorRecoveryIdentityExhaustedOwner {
                            recovery,
                            original_failure: failure,
                            terminal_failure: terminal_failure.clone(),
                            exhaustion,
                        };
                        return (
                            terminal_failure,
                            FailureCleanupContinuationOwner::TerminalRecoveryFailed(Box::new(
                                owner,
                            )),
                        );
                    }
                };
                recovery.reserved_attempt = Some(reserved);
                (
                    failure,
                    FailureCleanupContinuationOwner::BeginBoundError(recovery),
                )
            }
            continuation => (failure, continuation),
        }
    }

    fn frozen_failure_recovery_continuation_for(
        &self,
        current: &CurrentWork,
        failure_branch_index: usize,
    ) -> FailureCleanupContinuationOwner {
        let Some(branch) = current.branch.as_ref() else {
            return FailureCleanupContinuationOwner::Terminal;
        };
        let Some(binding) = branch.nearest_error_view_for(failure_branch_index) else {
            return FailureCleanupContinuationOwner::BeginFallback;
        };
        let Some(boundary) = branch.routes().get(binding.branch_index()) else {
            return FailureCleanupContinuationOwner::Terminal;
        };
        if !self.reconciliation_matches_active_for(current)
            || binding.branch_index() > failure_branch_index
            || boundary.route() != binding.boundary_route()
            || boundary.error_boundary() != Some(binding.boundary())
            || boundary.slot() != binding.outlet()
        {
            return FailureCleanupContinuationOwner::Terminal;
        }
        let Some(prepared_prefix) =
            self.freeze_bound_error_prepared_prefix(current, binding.branch_index())
        else {
            return FailureCleanupContinuationOwner::Terminal;
        };
        let Some(mount) = self.freeze_bound_error_mount_source(current, binding, &prepared_prefix)
        else {
            return FailureCleanupContinuationOwner::Terminal;
        };
        FailureCleanupContinuationOwner::BeginBoundError(Box::new(FrozenBoundErrorRecovery {
            failure_branch_index,
            binding: binding.clone(),
            mount,
            prepared_prefix,
            reserved_attempt: None,
        }))
    }

    fn freeze_bound_error_prepared_prefix(
        &self,
        current: &CurrentWork,
        boundary_index: usize,
    ) -> Option<Vec<RouteFrameVersion>> {
        let branch = current.branch.as_ref()?;
        let projection = current.reconciliation.as_ref()?;
        let mut preserved = projection
            .preserved()
            .iter()
            .filter(|frame| frame.branch_index() < boundary_index)
            .collect::<Vec<_>>();
        preserved.sort_by_key(|frame| frame.branch_index());
        if preserved
            .iter()
            .enumerate()
            .any(|(branch_index, frame)| frame.branch_index() != branch_index)
        {
            return None;
        }
        let prepared_start = preserved.len();
        let mut prepared = Vec::with_capacity(boundary_index.saturating_sub(prepared_start));
        for branch_index in prepared_start..boundary_index {
            let route = branch.routes().get(branch_index)?;
            let projected = projection
                .entering()
                .iter()
                .find(|frame| frame.branch_index() == branch_index)?;
            let frame = current.target_frames.get(branch_index)?;
            let slot_matches = match projected.slot() {
                ProjectedEnteringSlot::Existing(expected) => frame.slot() == expected,
                ProjectedEnteringSlot::MountNew => true,
            };
            if projected.route() != route.route()
                || projected.origin() != &FrameOrigin::MatchedRoute(route.route())
                || frame.origin() != projected.origin()
                || frame.frame() != projected.frame()
                || frame.generation() != projected.generation()
                || !slot_matches
                || !current.applied_frames.contains(frame)
            {
                return None;
            }
            prepared.push(frame.clone());
        }
        Some(prepared)
    }

    fn freeze_bound_error_mount_source(
        &self,
        current: &CurrentWork,
        binding: &FrozenErrorViewBinding,
        prepared_prefix: &[RouteFrameVersion],
    ) -> Option<FrozenErrorMountSource> {
        let boundary_index = binding.branch_index();
        let projection = current.reconciliation.as_ref()?;
        let active_frames = self.state.active().frames();
        let strict_prefix_is_exact = (0..boundary_index).all(|branch_index| {
            let preserved_is_exact = projection
                .preserved()
                .iter()
                .find(|preserved| preserved.branch_index() == branch_index)
                .is_some_and(|preserved| {
                    active_frames.get(branch_index) == Some(preserved.version())
                        && current.target_frames.get(branch_index) == Some(preserved.version())
                });
            let prepared_is_exact = current
                .target_frames
                .get(branch_index)
                .is_some_and(|frame| prepared_prefix.contains(frame));
            preserved_is_exact || prepared_is_exact
        });
        if !strict_prefix_is_exact {
            return None;
        }

        let projected_slot = projection
            .preserved()
            .iter()
            .find(|preserved| preserved.branch_index() == boundary_index)
            .and_then(|preserved| {
                current
                    .target_frames
                    .get(boundary_index)
                    .filter(|frame| *frame == preserved.version())
                    .map(|frame| ProjectedEnteringSlot::Existing(frame.slot()))
            })
            .or_else(|| {
                projection
                    .entering()
                    .iter()
                    .find(|entering| {
                        entering.branch_index() == boundary_index
                            && entering.route() == binding.boundary_route()
                    })
                    .and_then(|entering| match entering.slot() {
                        ProjectedEnteringSlot::Existing(slot) => current
                            .target_frames
                            .get(boundary_index)
                            .filter(|frame| {
                                frame.slot() == slot
                                    && frame.frame() == entering.frame()
                                    && frame.generation() == entering.generation()
                                    && frame.origin()
                                        == &FrameOrigin::MatchedRoute(binding.boundary_route())
                            })
                            .map(|_| ProjectedEnteringSlot::Existing(slot)),
                        ProjectedEnteringSlot::MountNew => current
                            .target_frames
                            .get(boundary_index)
                            .filter(|frame| {
                                frame.frame() == entering.frame()
                                    && frame.generation() == entering.generation()
                                    && frame.origin()
                                        == &FrameOrigin::MatchedRoute(binding.boundary_route())
                            })
                            .map_or(Some(ProjectedEnteringSlot::MountNew), |frame| {
                                Some(ProjectedEnteringSlot::Existing(frame.slot()))
                            }),
                    })
            })?;

        if boundary_index == 0 {
            let host = self.state.host()?.clone();
            let slot = self.state.top_slot()?;
            return (projected_slot == ProjectedEnteringSlot::Existing(slot))
                .then_some(FrozenErrorMountSource::TopSlot { host, slot });
        }

        let parent_index = boundary_index - 1;
        let parent = current.target_frames.get(parent_index)?.clone();
        let parent_is_exact = projection.preserved().iter().any(|preserved| {
            preserved.branch_index() == parent_index && preserved.version() == &parent
        }) || prepared_prefix.contains(&parent);
        if !parent_is_exact {
            return None;
        }
        let role = projection.child_slots().iter().find(|role| {
            role.parent_branch_index() == parent_index && role.outlet() == binding.outlet()
        })?;
        match projected_slot {
            ProjectedEnteringSlot::Existing(slot) => {
                let exact_binding = role.binding().is_some_and(|child_slot| {
                    child_slot.parent_branch_index() == parent_index
                        && child_slot.parent() == &parent
                        && child_slot.outlet() == binding.outlet()
                        && child_slot.slot() == slot
                });
                let exact_staged_slot = role.kind() == ProjectedChildSlotRoleKind::MountNew
                    && role.binding().is_none()
                    && current
                        .target_frames
                        .get(boundary_index)
                        .is_some_and(|frame| {
                            frame.slot() == slot
                                && frame.origin()
                                    == &FrameOrigin::MatchedRoute(binding.boundary_route())
                        });
                (exact_binding || exact_staged_slot).then_some(
                    FrozenErrorMountSource::ExistingChild {
                        parent,
                        outlet: binding.outlet().clone(),
                        slot,
                    },
                )
            }
            ProjectedEnteringSlot::MountNew => {
                if role.kind() != ProjectedChildSlotRoleKind::MountNew || role.binding().is_some() {
                    return None;
                }
                let parent_source = self.freeze_bound_error_parent_mount_source(
                    current,
                    projection,
                    parent_index,
                    &parent,
                    prepared_prefix,
                )?;
                Some(FrozenErrorMountSource::MountRequired {
                    parent,
                    outlet: binding.outlet().clone(),
                    parent_source,
                })
            }
        }
    }

    fn freeze_bound_error_parent_mount_source(
        &self,
        current: &CurrentWork,
        projection: &RouteReconciliationProjection,
        parent_index: usize,
        parent: &RouteFrameVersion,
        prepared_prefix: &[RouteFrameVersion],
    ) -> Option<FrozenParentMountSource> {
        let parent_route = current.branch.as_ref()?.routes().get(parent_index)?;
        if parent.origin() != &FrameOrigin::MatchedRoute(parent_route.route()) {
            return None;
        }
        if parent_index == 0 {
            let host = self.state.host()?.clone();
            let slot = self.state.top_slot()?;
            return (parent.router_identity() == self.router
                && parent.host() == host.host()
                && parent.slot() == slot)
                .then_some(FrozenParentMountSource::TopSlot { host, slot });
        }

        let grandparent_index = parent_index - 1;
        let role = projection.child_slots().iter().find(|role| {
            role.parent_branch_index() == grandparent_index && role.outlet() == parent_route.slot()
        })?;
        let grandparent = current.target_frames.get(grandparent_index)?;
        let grandparent_is_exact = projection.preserved().iter().any(|preserved| {
            preserved.branch_index() == grandparent_index && preserved.version() == grandparent
        }) || prepared_prefix.contains(grandparent);
        if !grandparent_is_exact {
            return None;
        }
        match role.binding() {
            Some(claim)
                if claim.parent_branch_index() == grandparent_index
                    && claim.parent() == grandparent
                    && claim.outlet() == parent_route.slot()
                    && claim.slot() == parent.slot()
                    && claim.child() == Some(parent) =>
            {
                Some(FrozenParentMountSource::RegisteredChild {
                    claim: claim.clone(),
                })
            }
            Some(claim)
                if role.kind() == ProjectedChildSlotRoleKind::ActivateExisting
                    && claim.parent_branch_index() == grandparent_index
                    && claim.parent() == grandparent
                    && claim.outlet() == parent_route.slot()
                    && claim.slot() == parent.slot()
                    && claim.child().is_none() =>
            {
                Some(FrozenParentMountSource::RegisteredChild {
                    claim: LogicalChildSlotBinding::occupied(
                        grandparent_index,
                        grandparent.clone(),
                        parent_route.slot().clone(),
                        parent.clone(),
                    ),
                })
            }
            None if role.kind() == ProjectedChildSlotRoleKind::MountNew => {
                Some(FrozenParentMountSource::RegisteredChild {
                    claim: LogicalChildSlotBinding::occupied(
                        grandparent_index,
                        grandparent.clone(),
                        parent_route.slot().clone(),
                        parent.clone(),
                    ),
                })
            }
            Some(_) | None => None,
        }
    }

    fn begin_failure_cleanup_with_continuation(
        &mut self,
        expected: PendingFailureKind,
        failure: NavigationFailure,
        continuation: FailureCleanupContinuationOwner,
    ) -> FailureCleanupRequest {
        let retained_prepared_prefix = match &continuation {
            FailureCleanupContinuationOwner::BeginBoundError(recovery) => {
                recovery.prepared_prefix.as_slice()
            }
            FailureCleanupContinuationOwner::Terminal
            | FailureCleanupContinuationOwner::TerminalRecoveryFailed(_)
            | FailureCleanupContinuationOwner::BeginFallback
            | FailureCleanupContinuationOwner::AuthoritySettlement => &[],
        };
        let logical_roots = self
            .current
            .as_ref()
            .expect("a nested failure belongs to current work")
            .applied_frames
            .iter()
            .rev()
            .filter(|frame| !retained_prepared_prefix.contains(frame))
            .cloned()
            .collect();
        self.begin_failure_cleanup_with_roots(expected, failure, continuation, logical_roots)
    }

    fn begin_failure_cleanup_with_roots(
        &mut self,
        expected: PendingFailureKind,
        failure: NavigationFailure,
        continuation: FailureCleanupContinuationOwner,
        logical_roots: Vec<RouteFrameVersion>,
    ) -> FailureCleanupRequest {
        self.current_history_capability = None;
        let current = self
            .current
            .as_ref()
            .expect("a nested failure belongs to current work");
        let request = FailureCleanupRequest::new(
            current.identity.clone(),
            expected,
            logical_roots,
            failure,
            continuation,
        );
        let phase = if request.receiver.expected == PendingFailureKind::AuthorityLost {
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
            expected: request.receiver.clone(),
        };
        self.action = None;
        self.transition_with_trace(request.receiver.identity.request, phase);
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

    pub(crate) fn authority_settlement_logical_roots(&self) -> Vec<RouteFrameVersion> {
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

    fn fail_bound_error_before_issue(
        &mut self,
        request: NavigationRequestId,
        prepared_prefix: &[RouteFrameVersion],
        reserved: Option<crate::ids::ReservedAttemptIds>,
        failure: NavigationFailure,
    ) {
        self.retire_history_authority();
        if prepared_prefix.is_empty() {
            self.current = None;
            self.action = None;
            self.fail_reserved_request(request, failure);
            self.start_next();
            return;
        }

        let Some(reserved) = reserved else {
            debug_assert!(
                false,
                "a retained bound-error prefix must own a pre-reserved cleanup attempt"
            );
            self.action = None;
            return;
        };
        let current = self
            .current
            .as_mut()
            .expect("a retained bound-error prefix remains current until recovery is bound");
        let retained = prepared_prefix
            .last()
            .expect("a retained bound-error prefix is non-empty");
        current.identity = FrameTaskIdentity {
            router: self.router,
            host: retained.host(),
            host_epoch: current.identity.host_epoch,
            slot: retained.slot(),
            request,
            transaction: reserved.transaction(),
            frame_task: reserved.frame_task(),
            frame: retained.frame(),
            generation: retained.generation(),
            origin: retained.origin().clone(),
            branch_index: prepared_prefix.len() - 1,
        };
        current.reservation = vec![ReservedFrameAttempt::new(
            reserved.frame_task(),
            reserved.candidate_frame(),
        )];
        current.applied_frames = prepared_prefix.to_vec();
        current.cleanup_batch = reserved.cleanup_batch();
        let roots = prepared_prefix.iter().rev().cloned().collect();
        let cleanup = self.begin_failure_cleanup_with_roots(
            PendingFailureKind::Apply,
            failure,
            FailureCleanupContinuationOwner::Terminal,
            roots,
        );
        self.action = Some(EngineAction::FailureCleanup(cleanup));
    }

    fn begin_bound_error(
        &mut self,
        request: NavigationRequestId,
        original_failure: NavigationFailure,
        mut recovery: FrozenBoundErrorRecovery,
    ) -> BoundErrorBeginDisposition {
        self.current_history_capability = None;
        let source_is_exact = self.current_reconciliation_matches_active();
        let prepared = source_is_exact.then(|| {
            let current = self.current.as_ref()?;
            let branch = current.branch.as_ref()?;
            let observed = branch.nearest_error_view_for(recovery.failure_branch_index)?;
            let observed_prepared_prefix =
                self.freeze_bound_error_prepared_prefix(current, observed.branch_index())?;
            let observed_mount =
                self.freeze_bound_error_mount_source(current, observed, &observed_prepared_prefix)?;
            if observed != &recovery.binding
                || observed_mount != recovery.mount
                || observed_prepared_prefix != recovery.prepared_prefix
            {
                return None;
            }
            let boundary_index = recovery.binding.branch_index();
            let reconciliation = current.reconciliation.as_ref()?;
            let mut preserved_roles = reconciliation
                .preserved()
                .iter()
                .filter(|preserved| preserved.branch_index() < boundary_index)
                .collect::<Vec<_>>();
            preserved_roles.sort_by_key(|preserved| preserved.branch_index());
            if preserved_roles
                .iter()
                .enumerate()
                .any(|(branch_index, preserved)| preserved.branch_index() != branch_index)
            {
                return None;
            }
            let mut target_frames = preserved_roles
                .into_iter()
                .map(|preserved| preserved.version().clone())
                .collect::<Vec<_>>();
            target_frames.extend(recovery.prepared_prefix.iter().cloned());
            if target_frames.len() != boundary_index {
                return None;
            }
            let mut cleanup_roles = reconciliation
                .preserved()
                .iter()
                .filter(|frame| frame.branch_index() >= boundary_index)
                .map(|frame| (frame.branch_index(), frame.version().clone()))
                .collect::<Vec<_>>();
            cleanup_roles.extend(
                reconciliation
                    .leaving()
                    .iter()
                    .map(|frame| (frame.branch_index(), frame.version().clone())),
            );
            cleanup_roles.sort_by(|left, right| right.0.cmp(&left.0));
            let cleanup_roots = cleanup_roles
                .into_iter()
                .map(|(_, frame)| frame)
                .collect::<Vec<_>>();
            let mut retained_child_slots = Vec::new();
            for child_index in 1..boundary_index {
                let parent = target_frames.get(child_index - 1)?;
                let child = target_frames.get(child_index)?;
                let route = branch.routes().get(child_index)?;
                let projected = reconciliation.child_slots().iter().find(|slot| {
                    slot.parent_branch_index() == child_index - 1 && slot.outlet() == route.slot()
                })?;
                let slot_is_exact = projected.binding().is_none_or(|slot| {
                    slot.parent() == parent
                        && slot.outlet() == route.slot()
                        && slot.slot() == child.slot()
                });
                if !slot_is_exact {
                    return None;
                }
                retained_child_slots.push(LogicalChildSlotBinding::occupied(
                    child_index - 1,
                    parent.clone(),
                    route.slot().clone(),
                    child.clone(),
                ));
            }
            for projected in reconciliation
                .child_slots()
                .iter()
                .filter(|slot| slot.kind() == ProjectedChildSlotRoleKind::RetainEmpty)
            {
                let binding = projected.binding()?;
                let parent = target_frames.get(binding.parent_branch_index())?;
                if binding.parent() != parent || binding.child().is_some() {
                    return None;
                }
                let mapping_exists = retained_child_slots.iter().any(|candidate| {
                    candidate.parent_branch_index() == binding.parent_branch_index()
                        && candidate.outlet() == binding.outlet()
                });
                let slot_is_reused = retained_child_slots
                    .iter()
                    .any(|candidate| candidate.slot() == binding.slot());
                if mapping_exists || slot_is_reused {
                    return None;
                }
                retained_child_slots.push(LogicalChildSlotBinding::retained_empty(
                    binding.parent_branch_index(),
                    binding.parent().clone(),
                    binding.outlet().clone(),
                    binding.slot(),
                ));
            }
            Some((
                current.location.clone(),
                branch.clone(),
                branch.routes().get(boundary_index)?.clone(),
                target_frames,
                cleanup_roots,
                retained_child_slots,
                reconciliation.clone(),
                recovery.prepared_prefix.clone(),
            ))
        });
        let Some(Some((
            location,
            target_branch,
            matched,
            mut target_frames,
            cleanup_roots,
            mut active_child_slots,
            reconciliation,
            prepared_prefix,
        ))) = prepared
        else {
            self.fail_bound_error_before_issue(
                request,
                &recovery.prepared_prefix,
                recovery.reserved_attempt,
                NavigationFailure::new(
                    NavigationFailureKind::RecoveryFailed,
                    "bound error recovery source drifted",
                ),
            );
            return BoundErrorBeginDisposition::Cleanup;
        };
        self.record_mut(request)
            .expect("recovering request is retained")
            .append_trace(NavigationTraceEvent::new(
                NavigationTraceEventKind::HistoryIntentRejected,
            ))
            .expect("running trace is open");
        let Some(reserved) = recovery.reserved_attempt.take() else {
            debug_assert!(
                false,
                "a frozen bound-error continuation must own a pre-reserved attempt"
            );
            self.fail_bound_error_before_issue(
                request,
                &prepared_prefix,
                None,
                NavigationFailure::new(
                    NavigationFailureKind::RecoveryFailed,
                    "bound error recovery lost its reserved attempt",
                ),
            );
            return BoundErrorBeginDisposition::Cleanup;
        };
        let host = self
            .state
            .host()
            .expect("accepted recovery has a confirmed host")
            .clone();
        let binding = recovery.binding;
        let boundary_index = binding.branch_index();
        let origin = FrameOrigin::RouteError(binding.boundary_route());
        let generation = RouteGeneration::initial();
        let projected_slot = match &recovery.mount {
            FrozenErrorMountSource::TopSlot { slot, .. }
            | FrozenErrorMountSource::ExistingChild { slot, .. } => {
                ProjectedEnteringSlot::Existing(*slot)
            }
            FrozenErrorMountSource::MountRequired { .. } => ProjectedEnteringSlot::MountNew,
        };
        let Some(reconciliation) = reconciliation.retarget_bound_error(
            &target_branch,
            binding.branch_index(),
            &prepared_prefix,
            reserved.candidate_frame(),
            generation,
            projected_slot,
        ) else {
            self.fail_bound_error_before_issue(
                request,
                &prepared_prefix,
                Some(reserved),
                NavigationFailure::new(
                    NavigationFailureKind::RecoveryFailed,
                    "bound error recovery projection drifted",
                ),
            );
            return BoundErrorBeginDisposition::Cleanup;
        };
        let direct_mount = match recovery.mount {
            FrozenErrorMountSource::MountRequired {
                parent,
                outlet,
                parent_source,
            } => {
                let parent_index = binding
                    .branch_index()
                    .checked_sub(1)
                    .expect("a root boundary never requires a child mount");
                let reserved_frame =
                    ReservedFrameAttempt::new(reserved.frame_task(), reserved.candidate_frame());
                let identity = FrameTaskIdentity {
                    router: self.router,
                    host: host.host(),
                    host_epoch: host.epoch(),
                    slot: parent.slot(),
                    request,
                    transaction: reserved.transaction(),
                    frame_task: reserved.frame_task(),
                    frame: parent.frame(),
                    generation: parent.generation(),
                    origin: parent.origin().clone(),
                    branch_index: parent_index,
                };
                let parent_target = match parent_source {
                    FrozenParentMountSource::TopSlot {
                        host: parent_host,
                        slot,
                    } => FrameMountTarget::TopSlot {
                        host: parent_host,
                        slot,
                    },
                    FrozenParentMountSource::RegisteredChild { claim } => {
                        FrameMountTarget::RegisteredChildSlot {
                            parent: claim.parent().clone(),
                            outlet: claim.outlet().clone(),
                        }
                    }
                };
                self.current = Some(CurrentWork {
                    identity,
                    branch: None,
                    location: location.clone(),
                    reservation: vec![reserved_frame],
                    target_frames,
                    applied_frames: prepared_prefix,
                    current_target: parent_target,
                    pending_mount: Some(PendingMount {
                        child_index: binding.branch_index(),
                        parent: parent.clone(),
                        outlet: outlet.clone(),
                        from_preserved_parent: true,
                        transaction: reserved.transaction(),
                        reserved: reserved_frame,
                        kind: PendingMountKind::BoundError,
                    }),
                    stage: WorkStage::AwaitingMount,
                    cleanup_batch: reserved.cleanup_batch(),
                    cleanup_roots,
                    attempt: AttemptKind::Fallback {
                        original_failure: original_failure.clone(),
                        bound_error: Some(BoundErrorAttempt {
                            binding,
                            matched,
                            active_child_slots,
                        }),
                    },
                    reconciliation: Some(reconciliation),
                    retired_host: None,
                });
                let token = match self.mount.issue(
                    &mut self.ids,
                    host.clone(),
                    request,
                    reserved.transaction(),
                    parent.clone(),
                    outlet.clone(),
                ) {
                    Ok(token) => token,
                    Err(_) => {
                        let failure = self.begin_failure_cleanup_for_branch(
                            PendingFailureKind::MountIssue,
                            NavigationFailure::new(
                                NavigationFailureKind::RecoveryFailed,
                                "bound error recovery mount request failed",
                            ),
                            boundary_index,
                        );
                        self.action = Some(EngineAction::FailureCleanup(failure));
                        return BoundErrorBeginDisposition::Cleanup;
                    }
                };
                self.record_mut(request)
                    .expect("recovering request is retained")
                    .append_trace(NavigationTraceEvent::mount_requested(
                        parent.clone(),
                        outlet.clone(),
                    ))
                    .expect("running trace is open");
                self.transition_with_trace(request, NavigationPhase::ResolvingMount);
                self.action = Some(EngineAction::MountRequest(FrameMountRequest {
                    token,
                    request,
                    parent,
                    outlet,
                }));
                return BoundErrorBeginDisposition::Bound;
            }
            direct_mount => direct_mount,
        };
        let (slot, target) = match direct_mount {
            FrozenErrorMountSource::TopSlot {
                host: frozen_host,
                slot,
            } => (
                slot,
                FrameMountTarget::TopSlot {
                    host: frozen_host,
                    slot,
                },
            ),
            FrozenErrorMountSource::ExistingChild {
                parent,
                outlet,
                slot,
            } => (
                slot,
                FrameMountTarget::RegisteredChildSlot { parent, outlet },
            ),
            FrozenErrorMountSource::MountRequired { .. } => unreachable!(),
        };
        let identity = FrameTaskIdentity {
            router: self.router,
            host: host.host(),
            host_epoch: host.epoch(),
            slot,
            request,
            transaction: reserved.transaction(),
            frame_task: reserved.frame_task(),
            frame: reserved.candidate_frame(),
            generation,
            origin: origin.clone(),
            branch_index: binding.branch_index(),
        };
        let target_frame = RouteFrameVersion::new(
            self.router,
            host.host(),
            slot,
            origin.clone(),
            reserved.candidate_frame(),
            generation,
        );
        if let FrameMountTarget::RegisteredChildSlot { parent, outlet } = &target {
            active_child_slots.push(LogicalChildSlotBinding::occupied(
                binding.branch_index() - 1,
                parent.clone(),
                outlet.clone(),
                target_frame.clone(),
            ));
        }
        target_frames.push(target_frame.clone());
        let input = RouteViewInput::new(
            location.clone(),
            Some(matched.route()),
            matched.params().clone(),
            origin,
            Some(original_failure.clone()),
        );
        let view = binding.view().clone();
        self.current = Some(CurrentWork {
            identity: identity.clone(),
            branch: None,
            location: location.clone(),
            reservation: vec![ReservedFrameAttempt::new(
                reserved.frame_task(),
                reserved.candidate_frame(),
            )],
            target_frames,
            applied_frames: prepared_prefix,
            current_target: target.clone(),
            pending_mount: None,
            stage: WorkStage::Applying,
            cleanup_batch: reserved.cleanup_batch(),
            cleanup_roots,
            attempt: AttemptKind::Fallback {
                original_failure,
                bound_error: Some(BoundErrorAttempt {
                    binding,
                    matched: matched.clone(),
                    active_child_slots,
                }),
            },
            reconciliation: Some(reconciliation),
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
        self.action = Some(EngineAction::FrameTask(FrameTask {
            identity,
            frame: target_frame,
            target,
            location,
            matched: Some(matched),
            view,
            input: Some(input),
        }));
        BoundErrorBeginDisposition::Bound
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
            attempt: AttemptKind::Fallback {
                original_failure,
                bound_error: None,
            },
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
        let (matched, origin, view, failure) = match &current.attempt {
            AttemptKind::Normal(_) if pending.kind == PendingMountKind::Normal => {
                let matched = current
                    .branch
                    .as_ref()
                    .and_then(|branch| branch.routes().get(index))
                    .cloned()
                    .ok_or(MountError::WrongPhase)?;
                let view = self
                    .matcher
                    .view_binding(matched.route())
                    .cloned()
                    .ok_or(MountError::WrongPhase)?;
                (
                    matched.clone(),
                    FrameOrigin::MatchedRoute(matched.route()),
                    view,
                    None,
                )
            }
            AttemptKind::Fallback {
                original_failure,
                bound_error: Some(bound),
            } if pending.kind == PendingMountKind::BoundError
                && bound.binding.branch_index() == index
                && bound.binding.outlet() == &pending.outlet =>
            {
                (
                    bound.matched.clone(),
                    FrameOrigin::RouteError(bound.binding.boundary_route()),
                    bound.binding.view().clone(),
                    Some(original_failure.clone()),
                )
            }
            AttemptKind::Normal(_) | AttemptKind::Fallback { .. } => {
                return Err(MountError::WrongPhase);
            }
        };
        let reserved = pending.reserved;
        let request = current.identity.request;
        let transaction = pending.transaction;
        let host = current.identity.host;
        let host_epoch = current.identity.host_epoch;
        let location = current.location.clone();
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
            parent: pending.parent.clone(),
            outlet: pending.outlet.clone(),
        };
        let input = RouteViewInput::new(
            location.clone(),
            Some(matched.route()),
            matched.params().clone(),
            origin,
            failure,
        );
        current.identity = identity.clone();
        current.target_frames.push(frame.clone());
        if let AttemptKind::Fallback {
            bound_error: Some(bound),
            ..
        } = &mut current.attempt
        {
            bound
                .active_child_slots
                .push(LogicalChildSlotBinding::occupied(
                    index - 1,
                    pending.parent.clone(),
                    pending.outlet.clone(),
                    frame.clone(),
                ));
        }
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
        let matched = match &current.attempt {
            AttemptKind::Fallback {
                bound_error: Some(attempt),
                ..
            } => Some(attempt.matched.clone()),
            AttemptKind::Normal(_) | AttemptKind::Fallback { .. } => current
                .branch
                .as_ref()
                .and_then(|branch| branch.routes().get(identity.branch_index))
                .cloned(),
        };
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
            AttemptKind::Fallback {
                original_failure,
                bound_error,
            } => (
                bound_error.as_ref().map_or_else(
                    || self.fallback.clone(),
                    |attempt| attempt.binding.view().clone(),
                ),
                Some(original_failure.clone()),
            ),
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
        self.current
            .as_ref()
            .is_none_or(|current| self.reconciliation_matches_active_for(current))
    }

    fn reconciliation_matches_active_for(&self, current: &CurrentWork) -> bool {
        let Some(projection) = current.reconciliation.as_ref() else {
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
            AttemptKind::Fallback {
                original_failure,
                bound_error,
            } => (
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
                bound_error
                    .as_ref()
                    .map(|attempt| attempt.active_child_slots.clone())
                    .unwrap_or_default(),
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
            host_admission_source: self.host_admission_source.clone(),
            host_admission_source_open: self.host_admission_source_open,
            pending_host_admission: self
                .pending_host_admission
                .as_ref()
                .map(CorePendingHostAdmission::projection),
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
            pending_post_commit_terminal: self
                .pending_post_commit_terminal
                .as_ref()
                .map(|_| "prepared"),
            pending_post_commit_settlement_trace: self.pending_post_commit_settlement_trace.clone(),
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

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn testing_host_admission_source_projection(&self) -> HostAdmissionSourceProjection {
        self.host_admission_source.clone()
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn testing_pending_host_admission_projection(
        &self,
    ) -> Option<CorePendingHostAdmissionProjection> {
        self.pending_host_admission
            .as_ref()
            .map(CorePendingHostAdmission::projection)
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn testing_drift_prepared_host_confirmation_receiver_input(&mut self) -> bool {
        if !matches!(
            self.pending_host_admission.as_ref(),
            Some(CorePendingHostAdmission::Confirming(_))
        ) {
            return false;
        }
        self.pending_host_admission = None;
        true
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn testing_drift_host_confirmation_preparation_receiver_input(&mut self) -> bool {
        if !matches!(
            self.pending_host_admission.as_ref(),
            Some(CorePendingHostAdmission::Grant(_))
        ) {
            return false;
        }
        self.pending_host_admission = None;
        true
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn testing_replace_host_allocator_with_foreign_domain(&mut self) -> bool {
        let Ok(ids) = CoreIdAllocator::new() else {
            return false;
        };
        self.ids = ids;
        true
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn testing_cancel_source_bound_token(
        issuer: &mut HostAdmissionIssuer,
        token: &SourceBoundHostAdmissionToken,
    ) -> bool {
        issuer.cancel(&token.token).is_ok()
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
        let settled = self.state.begin_authority_recovery(&host, top_slot)
            && self.state.complete_authority_settlement(&host, top_slot);
        if settled {
            self.retain_awaiting_admission_source();
        }
        settled
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

fn committed_history_digest(entries: &[RouteLocation], cursor: Option<usize>) -> u64 {
    let mut digest = 0xcbf2_9ce4_8422_2325_u64;
    digest = digest_u64(digest, entries.len() as u64);
    digest = digest_u64(digest, cursor.map_or(u64::MAX, |cursor| cursor as u64));
    for entry in entries {
        digest = digest_bytes(digest, entry.path().as_bytes());
        digest = digest_bytes(digest, entry.search().unwrap_or("").as_bytes());
        digest = digest_bytes(digest, entry.fragment().unwrap_or("").as_bytes());
    }
    digest
}

fn post_commit_host_physical_digest(receiver: &CorePostCommitCleanupReceiverProjection) -> u64 {
    let (domain, router) = receiver.router.provenance();
    let mut digest = 0x484f_5354_4544_4745_u64;
    for value in [
        domain,
        router,
        receiver.request.sequence(),
        receiver.identity.host().raw(),
        receiver.identity.host_epoch(),
        receiver.current_top_slot.raw(),
        receiver.committed_history.digest,
    ] {
        digest = digest_u64(digest, value);
    }
    digest
}

fn post_commit_settlement_trace_digest(trace: &CorePostCommitSettlementTraceProjection) -> u64 {
    let mut digest = 0x5345_5454_4c45_5253_u64;
    digest = digest_u64(
        digest,
        post_commit_compound_binding_digest(&trace.binding.compound),
    );
    digest = digest_u64(digest, trace.binding.normal_batch.binding_digest());
    digest = digest_u64(digest, trace.binding.authority_batch.binding_digest());
    digest = digest_u64(
        digest,
        match trace.cause() {
            PostCommitAuthorityCause::HostRegistrationLost => 1,
            PostCommitAuthorityCause::HostEntityMissing => 2,
            PostCommitAuthorityCause::RequiredResourcesLost => 3,
        },
    );
    digest = digest_u64(digest, trace.binding.prior_trace_start as u64);
    digest = digest_u64(digest, trace.binding.prior_trace_prefix_digest);
    digest = digest_u64(digest, trace.binding.prior_trace_digest);
    digest = digest_u64(digest, trace.binding.settled_original.len() as u64);
    for settled in &trace.binding.settled_original {
        digest = post_commit_settlement_key_digest(digest, &settled.obligation);
        digest = post_commit_settlement_key_digest(digest, &settled.receipt);
        digest = digest_u64(digest, cleanup_disposition_digest_tag(settled.disposition));
    }
    digest = post_commit_settlement_key_digest(digest, &trace.binding.authority_top_dependency);
    digest = digest_u64(digest, trace.settlement_event_start as u64);
    digest = digest_u64(digest, trace.cursor as u64);
    digest = digest_u64(digest, trace.expected.len() as u64);
    for entry in &trace.expected {
        digest = digest_u64(digest, entry.ordinal as u64);
        digest = digest_u64(
            digest,
            match entry.role {
                CorePostCommitSettlementRole::OriginalResidual => 1,
                CorePostCommitSettlementRole::AuthorityNonTop => 2,
                CorePostCommitSettlementRole::RetiredHostEdge => 3,
                CorePostCommitSettlementRole::AuthorityTop => 4,
            },
        );
        digest = post_commit_settlement_key_digest(digest, &entry.obligation);
        digest = post_commit_settlement_key_digest(digest, &entry.physical_source);
        match entry.disposition {
            Some(disposition) => {
                digest = digest_u64(digest, 1);
                digest = digest_u64(digest, cleanup_disposition_digest_tag(disposition));
            }
            None => {
                digest = digest_u64(digest, 0);
            }
        }
    }
    digest = digest_u64(digest, trace.recorded.len() as u64);
    for recorded in &trace.recorded {
        digest = post_commit_settlement_key_digest(digest, &recorded.obligation);
        digest = post_commit_settlement_key_digest(digest, &recorded.physical_source);
        match recorded.disposition {
            Some(disposition) => {
                digest = digest_u64(digest, 1);
                digest = digest_u64(digest, cleanup_disposition_digest_tag(disposition));
            }
            None => digest = digest_u64(digest, 0),
        }
    }
    digest
}

fn post_commit_authority_completion_digest(trace: &CorePostCommitSettlementTraceProjection) -> u64 {
    let mut digest = 0x4155_5448_434f_4d50_u64;
    let authority = trace.recorded.iter().filter(|entry| {
        matches!(
            entry.role,
            CorePostCommitSettlementRole::AuthorityNonTop
                | CorePostCommitSettlementRole::AuthorityTop
        )
    });
    for entry in authority {
        digest = post_commit_settlement_key_digest(digest, &entry.obligation);
        digest = post_commit_settlement_key_digest(digest, &entry.physical_source);
        digest = digest_u64(
            digest,
            entry.disposition.map_or(0, |disposition| {
                1 + cleanup_disposition_digest_tag(disposition)
            }),
        );
    }
    digest
}

fn post_commit_authority_expected_completion_digest(
    trace: &CorePostCommitSettlementTraceProjection,
) -> u64 {
    let mut digest = 0x4155_5448_434f_4d50_u64;
    let authority = trace.expected.iter().filter(|entry| {
        matches!(
            entry.role,
            CorePostCommitSettlementRole::AuthorityNonTop
                | CorePostCommitSettlementRole::AuthorityTop
        )
    });
    for entry in authority {
        digest = post_commit_settlement_key_digest(digest, &entry.obligation);
        digest = post_commit_settlement_key_digest(digest, &entry.physical_source);
        digest = digest_u64(
            digest,
            entry.disposition.map_or(0, |disposition| {
                1 + cleanup_disposition_digest_tag(disposition)
            }),
        );
    }
    digest
}

fn post_commit_compound_binding_digest(compound: &CorePostCommitCompoundProjection) -> u64 {
    let mut digest = post_commit_host_physical_digest(&compound.receiver);
    digest = digest_u64(
        digest,
        match compound.receiver.stage {
            CorePostCommitCleanupStageProjection::PostCommitCleanup => 1,
            CorePostCommitCleanupStageProjection::AuthorityAccepted => 2,
            CorePostCommitCleanupStageProjection::AuthorityCleanup => 3,
        },
    );
    digest = digest_u64(
        digest,
        compound.receiver.normal_batch.obligation_count as u64,
    );
    digest = digest_u64(digest, compound.authority_batch.obligation_count as u64);
    digest = digest_u64(
        digest,
        match compound.cause {
            PostCommitAuthorityCause::HostRegistrationLost => 1,
            PostCommitAuthorityCause::HostEntityMissing => 2,
            PostCommitAuthorityCause::RequiredResourcesLost => 3,
        },
    );
    digest
}

fn post_commit_settlement_permit_digest(
    trace: &CorePostCommitSettlementTraceProjection,
    expected: &CorePostCommitSettlementExpectedProjection,
) -> u64 {
    let mut digest = digest_u64(0x5045_524d_4954_5252_u64, trace.digest);
    digest = digest_u64(digest, expected.ordinal as u64);
    digest = digest_u64(
        digest,
        match expected.role {
            CorePostCommitSettlementRole::OriginalResidual => 1,
            CorePostCommitSettlementRole::AuthorityNonTop => 2,
            CorePostCommitSettlementRole::RetiredHostEdge => 3,
            CorePostCommitSettlementRole::AuthorityTop => 4,
        },
    );
    digest = post_commit_settlement_key_digest(digest, &expected.obligation);
    digest = post_commit_settlement_key_digest(digest, &expected.physical_source);
    digest = digest_u64(
        digest,
        expected.disposition.map_or(0, |disposition| {
            1 + cleanup_disposition_digest_tag(disposition)
        }),
    );
    digest
}

fn post_commit_settlement_key_digest(
    mut digest: u64,
    key: &CorePostCommitSettlementKeyProjection,
) -> u64 {
    digest = digest_u64(digest, key.namespace);
    digest = digest_u64(digest, key.identity.len() as u64);
    for value in &key.identity {
        digest = digest_u64(digest, *value);
    }
    digest
}

fn navigation_trace_digest(events: &[NavigationTraceEvent]) -> u64 {
    let mut digest = 0x4e41_5654_5241_4345_u64;
    digest = digest_u64(digest, events.len() as u64);
    for event in events {
        digest = digest_u64(digest, navigation_trace_kind_digest_tag(event.kind()));
        match event.frame() {
            Some(frame) => {
                digest = digest_u64(digest, 1);
                let (domain, router) = frame.router_identity().provenance();
                digest = digest_u64(digest, domain);
                digest = digest_u64(digest, router);
                digest = digest_u64(digest, frame.host().raw());
                digest = digest_u64(digest, frame.slot().raw());
                match frame.origin() {
                    FrameOrigin::MatchedRoute(route) => {
                        digest = digest_u64(digest, 1);
                        digest = digest_u64(digest, route.raw());
                    }
                    FrameOrigin::RouteError(route) => {
                        digest = digest_u64(digest, 2);
                        digest = digest_u64(digest, route.raw());
                    }
                    FrameOrigin::RouterFallback => {
                        digest = digest_u64(digest, 3);
                    }
                }
                digest = digest_u64(digest, frame.frame().raw());
                digest = digest_u64(digest, frame.generation().raw());
            }
            None => {
                digest = digest_u64(digest, 0);
            }
        }
        match event.outlet() {
            Some(outlet) => {
                digest = digest_u64(digest, 1);
                digest = digest_u64(digest, outlet.as_str().len() as u64);
                digest = digest_bytes(digest, outlet.as_str().as_bytes());
            }
            None => {
                digest = digest_u64(digest, 0);
            }
        }
        match event.slot() {
            Some(slot) => {
                digest = digest_u64(digest, 1);
                digest = digest_u64(digest, slot.raw());
            }
            None => {
                digest = digest_u64(digest, 0);
            }
        }
        match event.phase() {
            Some(phase) => {
                digest = digest_u64(digest, 1);
                let (tag, index) = navigation_phase_digest_parts(phase);
                digest = digest_u64(digest, tag);
                digest = digest_u64(digest, index);
            }
            None => {
                digest = digest_u64(digest, 0);
            }
        }
        match event.mount_rejection() {
            Some(rejection) => {
                digest = digest_u64(digest, 1);
                digest = digest_u64(digest, mount_rejection_digest_tag(rejection));
            }
            None => {
                digest = digest_u64(digest, 0);
            }
        }
        match event.cleanup_disposition() {
            Some(disposition) => {
                digest = digest_u64(digest, 1);
                digest = digest_u64(digest, cleanup_disposition_digest_tag(disposition));
            }
            None => {
                digest = digest_u64(digest, 0);
            }
        }
    }
    digest
}

fn navigation_trace_kind_digest_tag(kind: NavigationTraceEventKind) -> u64 {
    match kind {
        NavigationTraceEventKind::FrameTaskIssued => 1,
        NavigationTraceEventKind::RequestPhaseChanged => 2,
        NavigationTraceEventKind::HistoryIntentRejected => 3,
        NavigationTraceEventKind::MountRequested => 4,
        NavigationTraceEventKind::MountResolved => 5,
        NavigationTraceEventKind::MountGranted => 6,
        NavigationTraceEventKind::MountRejected => 7,
        NavigationTraceEventKind::MountRejectedAfterMutation => 8,
        NavigationTraceEventKind::ShellPrepared => 9,
        NavigationTraceEventKind::ViewApplied => 10,
        NavigationTraceEventKind::OutletRegistered => 11,
        NavigationTraceEventKind::FrameIsolated => 12,
        NavigationTraceEventKind::FrameActivated => 13,
        NavigationTraceEventKind::CommitPublished => 14,
        NavigationTraceEventKind::CleanupSettled => 15,
        NavigationTraceEventKind::RequestTerminal => 16,
    }
}

fn navigation_phase_digest_parts(phase: &NavigationPhase) -> (u64, u64) {
    match phase {
        NavigationPhase::Matching => (1, 0),
        NavigationPhase::Planning => (2, 0),
        NavigationPhase::ResolvingMount => (3, 0),
        NavigationPhase::PreparingFrame(index) => (4, *index as u64),
        NavigationPhase::ApplyingFrame(index) => (5, *index as u64),
        NavigationPhase::RegisteringOutlets(index) => (6, *index as u64),
        NavigationPhase::ReadyToSwitch => (7, 0),
        NavigationPhase::Recovering => (8, 0),
        NavigationPhase::CleanupPending => (9, 0),
        NavigationPhase::PostCommitCleanup => (10, 0),
    }
}

fn mount_rejection_digest_tag(rejection: MountRejectionKind) -> u64 {
    match rejection {
        MountRejectionKind::MissingOutlet => 1,
        MountRejectionKind::InvalidOutlet => 2,
        MountRejectionKind::ParentAuthorityLost => 3,
        MountRejectionKind::RejectedAfterMutation => 4,
    }
}

fn cleanup_disposition_digest_tag(disposition: crate::trace::CleanupDisposition) -> u64 {
    match disposition {
        crate::trace::CleanupDisposition::Removed => 1,
        crate::trace::CleanupDisposition::RemovedDetached => 2,
        crate::trace::CleanupDisposition::AlreadyAbsent => 3,
        crate::trace::CleanupDisposition::RejectedProtected => 4,
        crate::trace::CleanupDisposition::QuarantinedForeignDescendant => 5,
        crate::trace::CleanupDisposition::QuarantinedProtectedParent => 6,
        crate::trace::CleanupDisposition::QuarantinedForeignComponent => 7,
    }
}

fn digest_u64(digest: u64, value: u64) -> u64 {
    digest_bytes(digest, &value.to_le_bytes())
}

fn digest_bytes(mut digest: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        digest ^= u64::from(*byte);
        digest = digest.wrapping_mul(0x0000_0100_0000_01b3);
    }
    digest
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
                        request: self.engine.begin_failure_cleanup_for_branch(
                            expected,
                            failure,
                            branch_index,
                        ),
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
            AttemptKind::Normal(_) => {
                let expected = match failure.kind() {
                    NavigationFailureKind::Build => PendingFailureKind::Build,
                    NavigationFailureKind::Apply => PendingFailureKind::Apply,
                    _ => PendingFailureKind::Apply,
                };
                Some(
                    self.engine
                        .begin_failure_cleanup_for_branch(expected, failure, branch_index),
                )
            }
            AttemptKind::Fallback { .. } => {
                let expected = match failure.kind() {
                    NavigationFailureKind::Build => PendingFailureKind::Build,
                    NavigationFailureKind::Apply => PendingFailureKind::Apply,
                    _ => PendingFailureKind::Apply,
                };
                Some(self.engine.begin_failure_cleanup_with_continuation(
                    expected,
                    NavigationFailure::new(
                        NavigationFailureKind::RecoveryFailed,
                        "router fallback failed",
                    ),
                    FailureCleanupContinuationOwner::Terminal,
                ))
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
            AttemptKind::Normal(_) => self
                .engine
                .frozen_failure_recovery_continuation(failure, branch_index),
            AttemptKind::Fallback { .. } => (
                NavigationFailure::new(
                    NavigationFailureKind::RecoveryFailed,
                    "router fallback failed",
                ),
                FailureCleanupContinuationOwner::Terminal,
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
    let token = engine.request_legacy_host_admission(&mut issuer).unwrap();
    let grant = engine
        .grant_legacy_host_admission(&mut issuer, &token)
        .unwrap();
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
    host_admission_source: HostAdmissionSourceProjection,
    host_admission_source_open: bool,
    pending_host_admission: Option<CorePendingHostAdmissionProjection>,
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
    pending_post_commit_terminal: Option<&'static str>,
    pending_post_commit_settlement_trace: Option<CorePostCommitSettlementTraceProjection>,
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
#[path = "engine_tests.rs"]
mod tests;
