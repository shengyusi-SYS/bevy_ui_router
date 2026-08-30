use crate::admission::{
    AdmissionError as CoreAdmissionError,
    HostAdmissionAllocatorCounter as CoreHostAdmissionAllocatorCounter,
    HostAdmissionAllocatorExhaustionEvidence as CoreHostAdmissionAllocatorExhaustionEvidence,
    HostAdmissionIssuer,
    HostAdmissionIssuerOutstandingProjection as CoreHostAdmissionIssuerOutstandingProjection,
    HostAdmissionIssuerProjection as CoreHostAdmissionIssuerProjection,
    HostAdmissionToken as CoreHostAdmissionToken, HostGrant as CoreHostGrant,
    HostLease as CoreHostLease,
};
use crate::cleanup::{
    CleanupBatchSourceProjection, CleanupBatchToken, CleanupCompletion as CoreCleanupCompletion,
    CleanupCompletionDigest, CleanupManifestDigest, CleanupManifestSeal, CleanupSummary,
};
#[cfg(feature = "test-support")]
use crate::engine::BackAllocatorBoundary;
use crate::engine::{
    BackNoTargetApprovalOutcome as CoreBackNoTargetApprovalOutcome,
    BackNoTargetAuthorityBeginCoreOutcome as CoreBackNoTargetAuthorityBeginOutcome,
    BackNoTargetAuthorityCleanupCompletionCoreOutcome as CoreBackNoTargetAuthorityCleanupCompletionOutcome,
    BackNoTargetAuthorityCoreOutcome as CoreBackNoTargetAuthorityCoreOutcome,
    BackNoTargetAuthoritySettlementOwner as CoreBackNoTargetAuthoritySettlementOwner,
    BackNoTargetCleanupCompletionCoreOutcome as CoreBackNoTargetCleanupCompletionCoreOutcome,
    BackNoTargetReadyCoreOutcome as CoreBackNoTargetReadyCoreOutcome,
    BackNoTargetSourceDriftSettlementOutcome as CoreBackNoTargetSourceDriftSettlementOutcome,
    BackNoTargetSourceRefreezeOutcome as CoreBackNoTargetSourceRefreezeOutcome,
    BackTargetCommitOperationFailure as CoreBackTargetCommitOperationFailure,
    BackTargetCommitOperationOutcome as CoreBackTargetCommitOperationOutcome,
    BackTargetPermanentBeginCoreOutcome as CoreBackTargetPermanentBeginOutcome,
    BackTargetPermanentBeginCorePermanentEvidence,
    BackTargetPermanentCleanupCompletionCoreOutcome as CoreBackTargetPermanentCleanupCompletionOutcome,
    BackTargetPermanentSettlementOutcome as CoreBackTargetPermanentSettlementOutcome,
    BackTargetResumeAction as CoreBackTargetResumeAction,
    BackTargetResumeOutcome as CoreBackTargetResumeOutcome,
    BeginAuthoritySettlementErrorKind as CoreBeginAuthoritySettlementErrorKind,
    CleanupBatchProjection as CoreCleanupBatchProjection, CommitError as CoreCommitError,
    CommitTicket as CoreCommitTicket, CoreAuthorityCleanupCompletionFacts,
    CoreBackNoTargetAuthorityBeginOwner, CoreBackNoTargetAuthorityBeginPermanentEvidence,
    CoreBackNoTargetAuthorityBeginRetryEvidence, CoreBackNoTargetAuthorityCleanupBatch,
    CoreBackNoTargetAuthorityCleanupCompletionOwner,
    CoreBackNoTargetAuthorityCleanupMismatchEvidence,
    CoreBackNoTargetAuthorityCompletionFactsMismatchProjection as InnerBackNoTargetAuthorityCompletionFactsMismatchProjection,
    CoreBackNoTargetAuthorityTerminalProjection, CoreBackNoTargetCleanupAction,
    CoreBackNoTargetCleanupBatchOwner, CoreBackNoTargetCleanupBatchProjection,
    CoreBackNoTargetCleanupCompletionOwner, CoreBackNoTargetTerminalProjection,
    CoreBackTargetAppliedOutcome, CoreBackTargetCommitFailureCleanup,
    CoreBackTargetCommitRejectedPublication, CoreBackTargetCommitRetainedInputProjection,
    CoreBackTargetDriverFailureCleanup, CoreBackTargetDriverFailureReport,
    CoreBackTargetDriverFailureReportMode, CoreBackTargetDriverFallback,
    CoreBackTargetDriverTerminal, CoreBackTargetDriverTerminalProjection,
    CoreBackTargetPermanentBeginOwner, CoreBackTargetPermanentCanonicalRelationProjection,
    CoreBackTargetPermanentCleanupBatch, CoreBackTargetPermanentCleanupCompletionOwner,
    CoreBackTargetPermanentCleanupMismatchEvidence,
    CoreBackTargetPermanentCleanupReceiverMismatchProjection,
    CoreBackTargetPermanentCleanupTerminalProjection,
    CoreBackTargetPermanentCompletionFactsMismatchProjection,
    CoreBackTargetPermanentCurrentStateProjection,
    CoreBackTargetPermanentHistoryPreparationErrorProjection,
    CoreBackTargetPermanentRequestProjection, CoreBackTargetPermanentSettlementOwner,
    CoreBackTargetPermanentTerminalExpectedProjection,
    CoreBackTargetPermanentTerminalFailureProjection,
    CoreBackTargetPermanentTerminalObservedProjection, CoreBackTargetPermanentTerminalOperation,
    CoreBackTargetPermanentTerminalSourceProjection, CoreBackTargetPhysicalReceiverRef,
    CoreBoundPostCommitSettlementPlan,
    CoreHostConfirmationSource as InnerCoreHostConfirmationSource,
    CoreNormalCleanupCompletionFacts,
    CorePendingHostAdmissionProjection as InnerCorePendingHostAdmissionProjection,
    CorePostCommitAuthorityBeginPermanentEvidence, CorePostCommitAuthorityBeginRetryEvidence,
    CorePostCommitAuthorityPreparationSource, CorePostCommitAuthorityTransferOwner,
    CorePostCommitCleanupObservedProjection, CorePostCommitCleanupReceiverProjection,
    CorePostCommitCompoundBatchOwner, CorePostCommitCompoundCompletionOwner,
    CorePostCommitCompoundMismatchEvidence, CorePostCommitCompoundProjection,
    CorePostCommitPlannedCleanupProjection, CorePostCommitPlannedHostEdgeProjection,
    CorePostCommitReceiverMismatchProjection, CorePostCommitSettledOriginalProjection,
    CorePostCommitSettlementExpectedProjection, CorePostCommitSettlementKeyProjection,
    CorePostCommitSettlementPermitOutcome, CorePostCommitSettlementPlanBuilder,
    CorePostCommitSettlementPlanDescriptor, CorePostCommitSettlementPlanMismatchEvidence,
    CorePostCommitSettlementPlanMismatchKind, CorePostCommitSettlementRecordOutcome,
    CorePostCommitSettlementRole, CorePostCommitSettlementTraceMismatchEvidence,
    CorePostCommitSettlementTraceMismatchKind, CorePostCommitSettlementTraceProgress,
    CorePostCommitSettlementTraceProjection, CorePostCommitTerminalProjection,
    CorePreparationReceiverMismatchProjection,
    CorePreparedHostConfirmation as InnerCorePreparedHostConfirmation,
    CorePreparedPostCommitAuthorityTransfer, CorePreparedPostCommitSettlement,
    CoreProvenEmptyApprovalProjection, CoreRejectedBackContinuationOwner,
    CoreRetiredHostAuthorityExpectedProjection, CoreRetiredHostAuthorityTerminalEvidence,
    CoreRetiredHostEdgeCompletionFacts, EngineAction as CoreEngineAction,
    EngineInstallError as CoreEngineInstallError,
    FailedAuthoritySettlementTrigger as CoreFailedAuthoritySettlementTrigger,
    FailedAuthorityTriggerError as CoreFailedAuthorityTriggerError,
    FailureCleanupCompletionCommitError as CoreFailureCleanupCompletionCommitError,
    FailureCleanupCompletionDisposition as CoreFailureCleanupCompletionDisposition,
    FailureCleanupCompletionOwner as CoreFailureCleanupCompletionOwner,
    FailureCleanupCompletionPreparationMismatchProjection as CoreFailureCleanupCompletionPreparationMismatchProjection,
    FailureCleanupCompletionPrepareError as CoreFailureCleanupCompletionPrepareError,
    FailureCleanupCompletionReceiverMismatchProjection as CoreFailureCleanupCompletionReceiverMismatchProjection,
    FailureCleanupError as CoreFailureCleanupError,
    FailureCleanupManifestContradictionProjection as CoreFailureCleanupManifestContradictionProjection,
    FailureCleanupPermanentAuthorityOwner as CoreFailureCleanupPermanentAuthorityOwner,
    FailureCleanupPermanentCoreOutcome as CoreFailureCleanupPermanentOutcome,
    FailureCleanupPermanentHolder as CoreFailureCleanupPermanentHolder,
    FailureCleanupPrepareOutcome as CoreFailureCleanupPrepareOutcome,
    FailureCleanupReceiverMismatchProjection as CoreFailureCleanupReceiverMismatchProjection,
    FailureCleanupRequest as CoreFailureCleanupRequest,
    FailureCleanupZeroPhysicalMismatchProjection as CoreFailureCleanupZeroPhysicalMismatchProjection,
    FailureCleanupZeroPhysicalProjection as CoreFailureCleanupZeroPhysicalProjection,
    FailureCleanupZeroTerminalMismatchProjection as CoreFailureCleanupZeroTerminalMismatchProjection,
    FailureCleanupZeroTerminalOutcome as CoreFailureCleanupZeroTerminalOutcome,
    FailureCleanupZeroTerminalOwner as CoreFailureCleanupZeroTerminalOwner,
    FrameMountRequest as CoreFrameMountRequest, FrameMountTarget as CoreFrameMountTarget,
    FrameTask as CoreFrameTask,
    HostAdmissionConfirmationCoreOutcome as CoreHostAdmissionConfirmationCoreOutcome,
    HostAdmissionConfirmationFailureEvidence as CoreHostAdmissionConfirmationFailureEvidence,
    HostAdmissionOperation as CoreHostAdmissionOperation,
    HostAdmissionOperationExhaustionExpectation as CoreHostAdmissionOperationExhaustionExpectation,
    HostAdmissionOperationFailureContradiction as CoreHostAdmissionOperationFailureContradiction,
    HostAdmissionOperationFailureDisposition as CoreHostAdmissionOperationFailureDisposition,
    HostAdmissionOperationFailureEvidence as CoreHostAdmissionOperationFailureEvidence,
    HostAdmissionOperationFailureFreezeOutcome as CoreHostAdmissionOperationFailureFreezeOutcome,
    HostAdmissionOperationFailureProjection as CoreHostAdmissionOperationFailureProjection,
    HostAdmissionPermanence as CoreHostAdmissionPermanence,
    HostAdmissionReadyProjection as CoreHostAdmissionReadyProjection,
    HostAdmissionSourceIneligibleProjection as CoreHostAdmissionSourceIneligibleProjection,
    HostAdmissionSourcePermit as CoreHostAdmissionSourcePermit,
    HostAdmissionSourcePermitOutcome as CoreHostAdmissionSourcePermitOutcome,
    HostAdmissionSourceProjection as CoreHostAdmissionSourceProjection,
    HostAdmissionUnavailableCause as CoreHostAdmissionUnavailableCause,
    HostAdmissionUnavailableCauseBindOutcome as CoreHostAdmissionUnavailableCauseBindOutcome,
    HostAdmissionUnavailableEvidenceKind as CoreHostAdmissionUnavailableEvidenceKind,
    HostAdmissionUnavailableEvidenceProjection as CoreHostAdmissionUnavailableEvidenceProjection,
    HostAdmissionUnavailableFailureEvidence as CoreHostAdmissionUnavailableFailureEvidence,
    HostAdmissionUnavailableGrantedOutcome as CoreHostAdmissionUnavailableGrantedOutcome,
    HostAdmissionUnavailableGrantedSource as CoreHostAdmissionUnavailableGrantedSource,
    HostAdmissionUnavailableIssuerMismatchProjection as CoreHostAdmissionUnavailableIssuerMismatchProjection,
    HostAdmissionUnavailableIssuerOperation as CoreHostAdmissionUnavailableIssuerOperation,
    HostAdmissionUnavailableMismatchProjection as CoreHostAdmissionUnavailableMismatchProjection,
    HostAdmissionUnavailableObservedMismatchProjection as CoreHostAdmissionUnavailableObservedMismatchProjection,
    HostAdmissionUnavailableObservedOutcome as CoreHostAdmissionUnavailableObservedOutcome,
    HostAdmissionUnavailablePreStagingOutcome as CoreHostAdmissionUnavailablePreStagingOutcome,
    HostAdmissionUnavailablePreStagingOwner as CoreHostAdmissionUnavailablePreStagingOwner,
    HostAdmissionUnavailablePreStagingSource as CoreHostAdmissionUnavailablePreStagingSource,
    HostAdmissionUnavailableTerminalProjection as CoreHostAdmissionUnavailableTerminalProjection,
    HostConfirmationIssuerMismatchProjection as CoreHostConfirmationIssuerMismatchProjection,
    HostConfirmationPreparationOutcome as CoreHostConfirmationPreparationOutcome,
    HostConfirmationReceiverMismatchProjection as CoreHostConfirmationReceiverMismatchProjection,
    HostConfirmationReceiverProjection as CoreHostConfirmationReceiverProjection,
    HostConfirmationUnwindOutcome as CoreHostConfirmationUnwindOutcome,
    MountMutationArtifact as CoreMountMutationArtifact,
    PostCommitAuthorityAcceptanceOutcome as CorePostCommitAuthorityAcceptanceOutcome,
    PostCommitAuthorityBeginCoreOutcome as CorePostCommitAuthorityBeginOutcome,
    PostCommitAuthorityCause as CorePostCommitAuthorityCause,
    PostCommitAuthorityPreparationOutcome as CorePostCommitAuthorityPreparationOutcome,
    PostCommitCompoundCompletionCoreOutcome as CorePostCommitCompoundCompletionOutcome,
    PostCommitCompoundCompletionInputs as CorePostCommitCompoundCompletionInputs,
    PostCommitCompoundCompletionPreparationOutcome as CorePostCommitCompoundCompletionPreparationOutcome,
    PreCommitAuthorityCause as CorePreCommitAuthorityCause,
    PreparedFailureCleanupZeroPhysical as CorePreparedFailureCleanupZeroPhysical,
    PreparedHostAdmissionUnavailableCause as CorePreparedHostAdmissionUnavailableCause,
    PreparedPublication, PrivateRequestLookup, PushError as CorePushError,
    ReportError as CoreReportError, ResolveMountOwner as CoreResolveMountOwner,
    RouterEngine as CoreRouterEngine,
    SourceBoundHostAdmissionToken as CoreSourceBoundHostAdmissionToken,
    SourceBoundHostGrant as CoreSourceBoundHostGrant,
    ValidatedPublication as CoreValidatedPublication,
};
use crate::frame::{
    LogicalChildSlotBinding, ProjectedChildSlotRole, ProjectedChildSlotRoleKind,
    ProjectedEnteringFrame, ProjectedEnteringSlot, ProjectedExistingFrame, RouteFrameVersion,
    RouteReconciliationProjection,
};
use crate::history::{HistorySnapshot, MemoryHistory};
pub use crate::history_authority::{
    AcceptedBackReservation, BackAcceptedReservationView, BackAdmissionToken,
    BackContradictionTerminalEvidence, BackNoTargetCandidate, BackNoTargetSourceDriftEvidence,
    BackNoTargetSourceDriftOwner, BackNoTargetSourceDriftTerminalEvidence,
    BackResolutionContradiction, BackResolutionEvidence, BackResolutionEvidenceClassification,
    BackResolutionOwnerHandoff, BackResolutionScopeView, BackResolutionVerifyOutcome,
    BackTargetExecutionOwner, BackTargetPermanentOwner, BackTargetPermanentTerminalEvidence,
    EnqueueBackErrorKind, HistoryAdmissionStamp, HistoryAdmissionWorldSeal,
    PrepareBackAdmissionErrorKind, VerifiedBackNoTargetCandidate,
    VerifiedBackResolutionContradiction, VerifiedBackResolutionHandoff,
    VerifiedBackResolutionOwnerHandoff, VerifiedBackTargetExecutionOwner,
};
use crate::history_authority::{
    BackResolutionScopeProjection, ClaimHistoryAdmissionWorldError,
    HistoryExecutionBindingProjection,
};
use crate::ids::{NavigationRequestId, RouteSlotId, RouterIdentity};
use crate::location::RouteLocation;
use crate::meta::ErrorBoundaryKey;
#[cfg(any(test, feature = "test-support"))]
use crate::mount::MountRegistrationCompletion as CoreMountRegistrationCompletion;
use crate::mount::{
    MountGrant as CoreMountGrant, MountReceiptAttestation as CoreMountReceiptAttestation,
    MountReceiptDigest, MountReceiptProof as CoreMountReceiptProof, MountRegistrationDigest,
    MountRegistrationPermit as CoreMountRegistrationPermit,
};
use crate::presentation::{ActivePresentation, RouterAvailability};
use crate::request::{
    NavigationFailure, NavigationFailureKind, NavigationPhase, NavigationRequestRecord,
    RequestRecordError, RequestStatus,
};
use crate::route_tree::RouteTree;
use crate::slot::RouteSlotKey;
use crate::trace::{CleanupDisposition, MountRejectionKind, NavigationTraceEvent};
use crate::view_input::RouteViewInput;
use crate::view_key::RouteViewKey;
use crate::{
    HostSnapshot, HostTraceEvent, HostTraceEventKind, HostVersion,
    diagnostics::{
        DiagnosticEvent, DiagnosticSnapshot,
        PostCommitCleanupAnomalyCause as CorePostCommitCleanupAnomalyCause,
    },
    fingerprint::RouteBranchFingerprint,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdmissionError {
    OutstandingExists,
    AuthorityExists,
    ForeignRouter,
    Stale,
    Exhausted,
}

impl From<crate::admission::AdmissionError> for AdmissionError {
    fn from(error: CoreAdmissionError) -> Self {
        match error {
            CoreAdmissionError::OutstandingExists => Self::OutstandingExists,
            CoreAdmissionError::AuthorityExists => Self::AuthorityExists,
            CoreAdmissionError::ForeignRouter => Self::ForeignRouter,
            CoreAdmissionError::Stale => Self::Stale,
            CoreAdmissionError::Exhausted => Self::Exhausted,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostAdmissionAllocatorCounter {
    RegistrationIncarnation,
    CandidateRevision,
    RequestSequence,
    GrantEpoch,
    HostVersion,
    TopSlot,
}

impl From<HostAdmissionAllocatorCounter> for crate::admission::HostAdmissionAllocatorCounter {
    fn from(counter: HostAdmissionAllocatorCounter) -> Self {
        match counter {
            HostAdmissionAllocatorCounter::RegistrationIncarnation => Self::RegistrationIncarnation,
            HostAdmissionAllocatorCounter::CandidateRevision => Self::CandidateRevision,
            HostAdmissionAllocatorCounter::RequestSequence => Self::RequestSequence,
            HostAdmissionAllocatorCounter::GrantEpoch => Self::GrantEpoch,
            HostAdmissionAllocatorCounter::HostVersion => Self::HostVersion,
            HostAdmissionAllocatorCounter::TopSlot => Self::TopSlot,
        }
    }
}

impl From<crate::admission::HostAdmissionAllocatorCounter> for HostAdmissionAllocatorCounter {
    fn from(counter: CoreHostAdmissionAllocatorCounter) -> Self {
        match counter {
            CoreHostAdmissionAllocatorCounter::RegistrationIncarnation => {
                Self::RegistrationIncarnation
            }
            CoreHostAdmissionAllocatorCounter::CandidateRevision => Self::CandidateRevision,
            CoreHostAdmissionAllocatorCounter::RequestSequence => Self::RequestSequence,
            CoreHostAdmissionAllocatorCounter::GrantEpoch => Self::GrantEpoch,
            CoreHostAdmissionAllocatorCounter::HostVersion => Self::HostVersion,
            CoreHostAdmissionAllocatorCounter::TopSlot => Self::TopSlot,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostAdmissionAllocatorExhaustionEvidence {
    pub counter: HostAdmissionAllocatorCounter,
    pub current: u64,
    pub attempted: u64,
}

impl From<crate::admission::HostAdmissionAllocatorExhaustionEvidence>
    for HostAdmissionAllocatorExhaustionEvidence
{
    fn from(evidence: CoreHostAdmissionAllocatorExhaustionEvidence) -> Self {
        Self {
            counter: evidence.counter.into(),
            current: evidence.current,
            attempted: evidence.attempted,
        }
    }
}

impl From<HostAdmissionAllocatorExhaustionEvidence>
    for crate::admission::HostAdmissionAllocatorExhaustionEvidence
{
    fn from(evidence: HostAdmissionAllocatorExhaustionEvidence) -> Self {
        Self {
            counter: evidence.counter.into(),
            current: evidence.current,
            attempted: evidence.attempted,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostAdmissionOperation {
    Request,
    Grant,
}

impl From<crate::engine::HostAdmissionOperation> for HostAdmissionOperation {
    fn from(operation: CoreHostAdmissionOperation) -> Self {
        match operation {
            CoreHostAdmissionOperation::Request => Self::Request,
            CoreHostAdmissionOperation::Grant => Self::Grant,
        }
    }
}

impl From<HostAdmissionOperation> for crate::engine::HostAdmissionOperation {
    fn from(operation: HostAdmissionOperation) -> Self {
        match operation {
            HostAdmissionOperation::Request => Self::Request,
            HostAdmissionOperation::Grant => Self::Grant,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostAdmissionOperationFailureDisposition {
    PermanentAuthorityConflict,
    PermanentForeignOrStale,
    PermanentIdentityExhausted,
}

impl From<crate::engine::HostAdmissionOperationFailureDisposition>
    for HostAdmissionOperationFailureDisposition
{
    fn from(disposition: CoreHostAdmissionOperationFailureDisposition) -> Self {
        match disposition {
            CoreHostAdmissionOperationFailureDisposition::PermanentAuthorityConflict => {
                Self::PermanentAuthorityConflict
            }
            CoreHostAdmissionOperationFailureDisposition::PermanentForeignOrStale => {
                Self::PermanentForeignOrStale
            }
            CoreHostAdmissionOperationFailureDisposition::PermanentIdentityExhausted => {
                Self::PermanentIdentityExhausted
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostAdmissionSourceProjection {
    inner: CoreHostAdmissionSourceProjection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostAdmissionIssuerProjection {
    inner: CoreHostAdmissionIssuerProjection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostAdmissionIssuerOutstandingProjection {
    inner: CoreHostAdmissionIssuerOutstandingProjection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingHostAdmissionProjection {
    inner: InnerCorePendingHostAdmissionProjection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HostAdmissionOperationExhaustionExpectation {
    Forbidden,
    RequiredOneOf(Vec<HostAdmissionAllocatorCounter>),
}

impl From<crate::engine::HostAdmissionOperationExhaustionExpectation>
    for HostAdmissionOperationExhaustionExpectation
{
    fn from(expectation: CoreHostAdmissionOperationExhaustionExpectation) -> Self {
        match expectation {
            CoreHostAdmissionOperationExhaustionExpectation::Forbidden => Self::Forbidden,
            CoreHostAdmissionOperationExhaustionExpectation::RequiredOneOf(counters) => {
                Self::RequiredOneOf(counters.into_iter().map(Into::into).collect())
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HostAdmissionOperationFailureContradiction {
    OutstandingWithoutReturnedOwner {
        observed: HostAdmissionIssuerOutstandingProjection,
    },
    ExhaustionPairMismatch,
}

impl From<crate::engine::HostAdmissionOperationFailureContradiction>
    for HostAdmissionOperationFailureContradiction
{
    fn from(contradiction: CoreHostAdmissionOperationFailureContradiction) -> Self {
        match contradiction {
            CoreHostAdmissionOperationFailureContradiction::OutstandingWithoutReturnedOwner {
                observed,
            } => Self::OutstandingWithoutReturnedOwner {
                observed: HostAdmissionIssuerOutstandingProjection { inner: observed },
            },
            CoreHostAdmissionOperationFailureContradiction::ExhaustionPairMismatch => {
                Self::ExhaustionPairMismatch
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostAdmissionOperationFailureProjection {
    source: HostAdmissionSourceProjection,
    operation: HostAdmissionOperation,
    error: AdmissionError,
    disposition: HostAdmissionOperationFailureDisposition,
    expected_issuer: HostAdmissionIssuerProjection,
    observed_issuer: HostAdmissionIssuerProjection,
    exhaustion: Option<HostAdmissionAllocatorExhaustionEvidence>,
}

impl HostAdmissionOperationFailureProjection {
    pub fn source(&self) -> &HostAdmissionSourceProjection {
        &self.source
    }

    pub fn operation(&self) -> HostAdmissionOperation {
        self.operation
    }

    pub fn error(&self) -> AdmissionError {
        self.error
    }

    pub fn disposition(&self) -> HostAdmissionOperationFailureDisposition {
        self.disposition
    }

    pub fn expected_issuer(&self) -> &HostAdmissionIssuerProjection {
        &self.expected_issuer
    }

    pub fn observed_issuer(&self) -> &HostAdmissionIssuerProjection {
        &self.observed_issuer
    }

    pub fn exhaustion(&self) -> Option<HostAdmissionAllocatorExhaustionEvidence> {
        self.exhaustion
    }
}

impl From<crate::engine::HostAdmissionOperationFailureProjection>
    for HostAdmissionOperationFailureProjection
{
    fn from(projection: CoreHostAdmissionOperationFailureProjection) -> Self {
        Self {
            source: HostAdmissionSourceProjection {
                inner: projection.source,
            },
            operation: projection.operation.into(),
            error: projection.error.into(),
            disposition: projection.disposition.into(),
            expected_issuer: HostAdmissionIssuerProjection {
                inner: projection.expected_issuer,
            },
            observed_issuer: HostAdmissionIssuerProjection {
                inner: projection.observed_issuer,
            },
            exhaustion: projection.exhaustion.map(Into::into),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostAdmissionOperationFailureEvidence {
    inner: CoreHostAdmissionOperationFailureEvidence,
}

impl HostAdmissionOperationFailureEvidence {
    pub fn projection(&self) -> HostAdmissionOperationFailureProjection {
        self.inner.projection.clone().into()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostAdmissionOperationFailureEvidenceMismatch {
    pub source: HostAdmissionSourceProjection,
    pub operation: HostAdmissionOperation,
    pub error: AdmissionError,
    pub expected_issuer: HostAdmissionIssuerProjection,
    pub observed_issuer: HostAdmissionIssuerProjection,
    pub expected_exhaustion: HostAdmissionOperationExhaustionExpectation,
    pub observed_exhaustion: Option<HostAdmissionAllocatorExhaustionEvidence>,
    pub contradiction: HostAdmissionOperationFailureContradiction,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HostAdmissionOperationFailureFreezeOutcome {
    Frozen(HostAdmissionOperationFailureEvidence),
    Mismatch(HostAdmissionOperationFailureEvidenceMismatch),
}

impl From<crate::engine::HostAdmissionOperationFailureFreezeOutcome>
    for HostAdmissionOperationFailureFreezeOutcome
{
    fn from(outcome: CoreHostAdmissionOperationFailureFreezeOutcome) -> Self {
        match outcome {
            CoreHostAdmissionOperationFailureFreezeOutcome::Frozen(evidence) => {
                Self::Frozen(HostAdmissionOperationFailureEvidence { inner: evidence })
            }
            CoreHostAdmissionOperationFailureFreezeOutcome::Mismatch(evidence) => {
                Self::Mismatch(HostAdmissionOperationFailureEvidenceMismatch {
                    source: HostAdmissionSourceProjection {
                        inner: evidence.source,
                    },
                    operation: evidence.operation.into(),
                    error: evidence.error.into(),
                    expected_issuer: HostAdmissionIssuerProjection {
                        inner: evidence.expected_issuer,
                    },
                    observed_issuer: HostAdmissionIssuerProjection {
                        inner: evidence.observed_issuer,
                    },
                    expected_exhaustion: evidence.expected_exhaustion.into(),
                    observed_exhaustion: evidence.observed_exhaustion.map(Into::into),
                    contradiction: evidence.contradiction.into(),
                })
            }
        }
    }
}

pub enum HostAdmissionUnavailableEvidence {
    Operation(HostAdmissionOperationFailureEvidence),
}

pub struct HostAdmissionSourcePermit {
    inner: CoreHostAdmissionSourcePermit,
}

pub struct SourceBoundHostAdmissionToken {
    inner: CoreSourceBoundHostAdmissionToken,
}

pub struct SourceBoundHostGrant {
    inner: CoreSourceBoundHostGrant,
}

impl SourceBoundHostGrant {
    pub fn source_projection(&self) -> HostAdmissionSourceProjection {
        HostAdmissionSourceProjection {
            inner: self.inner.projection().source,
        }
    }

    pub fn version(&self) -> &HostVersion {
        self.inner.grant.version()
    }

    pub fn top_slot(&self) -> RouteSlotId {
        self.inner.grant.top_slot()
    }
}

pub enum HostAdmissionUnavailablePreStagingOwner {
    Permit(HostAdmissionSourcePermit),
    Token(SourceBoundHostAdmissionToken),
}

impl From<crate::engine::HostAdmissionUnavailablePreStagingOwner>
    for HostAdmissionUnavailablePreStagingOwner
{
    fn from(owner: CoreHostAdmissionUnavailablePreStagingOwner) -> Self {
        match owner {
            CoreHostAdmissionUnavailablePreStagingOwner::Permit(inner) => {
                Self::Permit(HostAdmissionSourcePermit { inner })
            }
            CoreHostAdmissionUnavailablePreStagingOwner::Token(inner) => {
                Self::Token(SourceBoundHostAdmissionToken { inner })
            }
        }
    }
}

impl From<HostAdmissionUnavailablePreStagingOwner>
    for crate::engine::HostAdmissionUnavailablePreStagingOwner
{
    fn from(owner: HostAdmissionUnavailablePreStagingOwner) -> Self {
        match owner {
            HostAdmissionUnavailablePreStagingOwner::Permit(owner) => Self::Permit(owner.inner),
            HostAdmissionUnavailablePreStagingOwner::Token(owner) => Self::Token(owner.inner),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostAdmissionTraceReservationStage {
    Request,
    Grant,
    TopSlotRegistration,
    ConfirmationPreparation,
    ConfirmationPublication,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostAdmissionTraceCapacityFailureEvidence {
    source: HostAdmissionSourceProjection,
    receiver: Option<HostConfirmationReceiverProjection>,
    stage: HostAdmissionTraceReservationStage,
    requested_slots: usize,
    observed_len: usize,
    observed_capacity: usize,
}

fn projection_mix(state: u64, value: u64) -> u64 {
    state.rotate_left(17) ^ value.wrapping_mul(0x9e37_79b9_7f4a_7c15)
}

fn host_version_digest(version: &crate::admission::HostVersion) -> u64 {
    let (domain, router) = version.router_identity().provenance();
    projection_mix(
        projection_mix(projection_mix(domain, router), version.host().raw()),
        version.epoch(),
    )
}

fn optional_host_digest(version: Option<&crate::admission::HostVersion>) -> u64 {
    version.map_or(1, |version| projection_mix(2, host_version_digest(version)))
}

fn optional_slot_digest(slot: Option<crate::ids::RouteSlotId>) -> u64 {
    slot.map_or(1, |slot| projection_mix(2, slot.raw()))
}

fn host_source_digest(source: &HostAdmissionSourceProjection) -> u64 {
    let source = &source.inner;
    let (domain, router) = source.router.provenance();
    let kind = match source.kind {
        crate::engine::HostAdmissionSourceKind::InitialAwaiting => 1,
        crate::engine::HostAdmissionSourceKind::RetryableHostLost => 2,
        crate::engine::HostAdmissionSourceKind::RetryableHostConflict => 3,
        crate::engine::HostAdmissionSourceKind::RetryableInvalidHostUiTopology => 4,
        crate::engine::HostAdmissionSourceKind::RetryableAuthorityEstablishmentFailed => 5,
        crate::engine::HostAdmissionSourceKind::PermanentAuthorityEstablishmentFailed => 6,
        crate::engine::HostAdmissionSourceKind::PermanentIdentityExhausted => 7,
        crate::engine::HostAdmissionSourceKind::PermanentRegistryInvariantViolation => 8,
    };
    let reason = match source.public_reason {
        None => 0,
        Some(crate::presentation::RouterUnavailableReason::HostConflict) => 1,
        Some(crate::presentation::RouterUnavailableReason::InvalidHostUiTopology) => 2,
        Some(crate::presentation::RouterUnavailableReason::HostLost) => 3,
        Some(crate::presentation::RouterUnavailableReason::AuthorityEstablishmentFailed) => 4,
        Some(crate::presentation::RouterUnavailableReason::RegistryInvariantViolation) => 5,
    };
    [
        domain,
        router,
        source.availability_incarnation.0,
        kind,
        reason,
        optional_host_digest(source.host.as_ref()),
        optional_slot_digest(source.top_slot),
    ]
    .into_iter()
    .fold(0x4853_4f55_5243_4501, projection_mix)
}

fn permit_key_digest(key: &crate::engine::HostAdmissionSourcePermitKey) -> u64 {
    let (domain, router) = key.router.provenance();
    [
        domain,
        router,
        key.availability_incarnation.0,
        key.sequence,
        key.binding,
    ]
    .into_iter()
    .fold(0x5045_524d_4954_0001, projection_mix)
}

fn optional_permit_key_digest(key: Option<&crate::engine::HostAdmissionSourcePermitKey>) -> u64 {
    key.map_or(1, |key| projection_mix(2, permit_key_digest(key)))
}

fn confirmation_receiver_digest(receiver: &HostConfirmationReceiverProjection) -> u64 {
    let receiver = &receiver.inner;
    let stage = match receiver.pending.stage {
        crate::engine::CorePendingHostAdmissionStage::Permit => 1,
        crate::engine::CorePendingHostAdmissionStage::Token => 2,
        crate::engine::CorePendingHostAdmissionStage::Grant => 3,
        crate::engine::CorePendingHostAdmissionStage::Confirming => 4,
    };
    [
        host_source_digest(&HostAdmissionSourceProjection {
            inner: receiver.source.clone(),
        }),
        host_source_digest(&HostAdmissionSourceProjection {
            inner: receiver.grant.source.clone(),
        }),
        permit_key_digest(&receiver.grant.permit_key),
        host_version_digest(&receiver.grant.host),
        receiver.grant.top_slot.raw(),
        host_source_digest(&HostAdmissionSourceProjection {
            inner: receiver.pending.source.clone(),
        }),
        stage,
        optional_permit_key_digest(receiver.pending.permit_key.as_ref()),
        optional_host_digest(receiver.pending.host.as_ref()),
        optional_slot_digest(receiver.pending.top_slot),
    ]
    .into_iter()
    .fold(0x5245_4345_4956_0001, projection_mix)
}

fn optional_confirmation_receiver_digest(
    receiver: Option<&HostConfirmationReceiverProjection>,
) -> u64 {
    receiver.map_or(1, |receiver| {
        projection_mix(2, confirmation_receiver_digest(receiver))
    })
}

fn issuer_grant_identity_digest(
    grant: &crate::admission::HostIssuerGrantIdentityProjection,
) -> u64 {
    let (domain, router) = grant.router.provenance();
    [
        domain,
        router,
        grant.sequence,
        host_version_digest(&grant.host),
        grant.top_slot.raw(),
    ]
    .into_iter()
    .fold(0x4953_4752_414e_0001, projection_mix)
}

fn issuer_projection_digest(issuer: &HostAdmissionIssuerProjection) -> u64 {
    let issuer = &issuer.inner;
    let outstanding = match issuer.outstanding.as_ref() {
        None => 1,
        Some(CoreHostAdmissionIssuerOutstandingProjection::Requested { router, sequence }) => {
            let (domain, router) = router.provenance();
            [2, domain, router, *sequence]
                .into_iter()
                .fold(0x4953_4f55_5453_0001, projection_mix)
        }
        Some(CoreHostAdmissionIssuerOutstandingProjection::Granted(grant)) => {
            projection_mix(3, issuer_grant_identity_digest(grant))
        }
    };
    let current = issuer.current.as_ref().map_or(1, |grant| {
        projection_mix(2, issuer_grant_identity_digest(grant))
    });
    [
        issuer.issuer_fingerprint,
        issuer.next_sequence,
        issuer.next_epoch,
        outstanding,
        current,
    ]
    .into_iter()
    .fold(0x4953_5355_4552_0001, projection_mix)
}

fn optional_trace_digest(trace: Option<&HostTraceEvent>) -> u64 {
    trace.map_or(1, |trace| {
        let kind = match trace.kind() {
            crate::admission::HostTraceEventKind::AdmissionRequested => 1,
            crate::admission::HostTraceEventKind::AdmissionRejected => 2,
            crate::admission::HostTraceEventKind::AdmissionRejectedAfterMutation => 3,
            crate::admission::HostTraceEventKind::HostGranted => 4,
            crate::admission::HostTraceEventKind::TopSlotRegistered => 5,
            crate::admission::HostTraceEventKind::HostReady => 6,
            crate::admission::HostTraceEventKind::SettlementStarted => 7,
            crate::admission::HostTraceEventKind::SettlementCompleted => 8,
        };
        [
            kind,
            optional_host_digest(trace.version()),
            optional_slot_digest(trace.slot()),
        ]
        .into_iter()
        .fold(0x5452_4143_4500_0001, projection_mix)
    })
}

impl HostAdmissionTraceCapacityFailureEvidence {
    fn new(
        source: HostAdmissionSourceProjection,
        receiver: Option<HostConfirmationReceiverProjection>,
        stage: HostAdmissionTraceReservationStage,
        requested_slots: usize,
        observed_len: usize,
        observed_capacity: usize,
    ) -> Self {
        Self {
            source,
            receiver,
            stage,
            requested_slots,
            observed_len,
            observed_capacity,
        }
    }

    pub fn source(&self) -> &HostAdmissionSourceProjection {
        &self.source
    }

    pub fn receiver(&self) -> Option<&HostConfirmationReceiverProjection> {
        self.receiver.as_ref()
    }

    pub fn stage(&self) -> HostAdmissionTraceReservationStage {
        self.stage
    }

    pub fn requested_slots(&self) -> usize {
        self.requested_slots
    }

    pub fn observed_len(&self) -> usize {
        self.observed_len
    }

    pub fn observed_capacity(&self) -> usize {
        self.observed_capacity
    }

    pub fn digest(&self) -> u64 {
        let stage = match self.stage {
            HostAdmissionTraceReservationStage::Request => 1_u64,
            HostAdmissionTraceReservationStage::Grant => 2,
            HostAdmissionTraceReservationStage::TopSlotRegistration => 3,
            HostAdmissionTraceReservationStage::ConfirmationPreparation => 4,
            HostAdmissionTraceReservationStage::ConfirmationPublication => 5,
        };
        stage
            ^ (self.requested_slots as u64).rotate_left(7)
            ^ (self.observed_len as u64).rotate_left(19)
            ^ (self.observed_capacity as u64).rotate_left(37)
            ^ u64::from(self.receiver.is_some()).rotate_left(53)
    }

    fn expected_digest(&self) -> u64 {
        self.capacity_projection_digest(self.observed_len.saturating_add(self.requested_slots))
    }

    fn observed_digest(&self) -> u64 {
        self.capacity_projection_digest(self.observed_capacity)
    }

    fn capacity_projection_digest(&self, capacity: usize) -> u64 {
        let stage = match self.stage {
            HostAdmissionTraceReservationStage::Request => 1,
            HostAdmissionTraceReservationStage::Grant => 2,
            HostAdmissionTraceReservationStage::TopSlotRegistration => 3,
            HostAdmissionTraceReservationStage::ConfirmationPreparation => 4,
            HostAdmissionTraceReservationStage::ConfirmationPublication => 5,
        };
        [
            host_source_digest(&self.source),
            optional_confirmation_receiver_digest(self.receiver.as_ref()),
            stage,
            self.requested_slots as u64,
            self.observed_len as u64,
            capacity as u64,
        ]
        .into_iter()
        .fold(0x4341_5041_4349_0001, projection_mix)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostTopSlotRegistrationCapacityFailureEvidence {
    inner: HostAdmissionTraceCapacityFailureEvidence,
}

impl HostTopSlotRegistrationCapacityFailureEvidence {
    pub fn digest(&self) -> u64 {
        self.inner.digest()
    }

    fn expected_digest(&self) -> u64 {
        self.inner.expected_digest()
    }

    fn observed_digest(&self) -> u64 {
        self.inner.observed_digest()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostConfirmationPreparationCapacityFailureEvidence {
    inner: HostAdmissionTraceCapacityFailureEvidence,
}

impl HostConfirmationPreparationCapacityFailureEvidence {
    pub fn digest(&self) -> u64 {
        self.inner.digest()
    }

    fn expected_digest(&self) -> u64 {
        self.inner.expected_digest()
    }

    fn observed_digest(&self) -> u64 {
        self.inner.observed_digest()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostConfirmationPublicationCapacityFailureEvidence {
    inner: HostAdmissionTraceCapacityFailureEvidence,
}

impl HostConfirmationPublicationCapacityFailureEvidence {
    pub fn digest(&self) -> u64 {
        self.inner.digest()
    }

    fn expected_digest(&self) -> u64 {
        self.inner.expected_digest()
    }

    fn observed_digest(&self) -> u64 {
        self.inner.observed_digest()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceBoundTopSlotRegistrationFailureKind {
    StaleGrant,
    DuplicateTrace,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceBoundTopSlotRegistrationOperationFailureEvidence {
    kind: SourceBoundTopSlotRegistrationFailureKind,
    source: HostAdmissionSourceProjection,
    expected_receiver: HostConfirmationReceiverProjection,
    expected_issuer: HostAdmissionIssuerProjection,
    observed_issuer: HostAdmissionIssuerProjection,
    expected_trace: HostTraceEvent,
    observed_trace: Option<HostTraceEvent>,
}

impl SourceBoundTopSlotRegistrationOperationFailureEvidence {
    pub fn kind(&self) -> SourceBoundTopSlotRegistrationFailureKind {
        self.kind
    }

    pub fn source(&self) -> &HostAdmissionSourceProjection {
        &self.source
    }

    pub fn digest(&self) -> u64 {
        let kind = match self.kind {
            SourceBoundTopSlotRegistrationFailureKind::StaleGrant => 1_u64,
            SourceBoundTopSlotRegistrationFailureKind::DuplicateTrace => 2,
        };
        kind ^ self
            .expected_receiver
            .inner
            .grant
            .top_slot
            .raw()
            .rotate_left(13)
            ^ self.expected_issuer.inner.next_sequence.rotate_left(29)
            ^ self.observed_issuer.inner.next_sequence.rotate_left(43)
            ^ u64::from(self.observed_trace.is_some()).rotate_left(59)
    }

    fn expected_digest(&self) -> u64 {
        self.operation_projection_digest(&self.expected_issuer, Some(&self.expected_trace))
    }

    fn observed_digest(&self) -> u64 {
        self.operation_projection_digest(&self.observed_issuer, self.observed_trace.as_ref())
    }

    fn operation_projection_digest(
        &self,
        issuer: &HostAdmissionIssuerProjection,
        trace: Option<&HostTraceEvent>,
    ) -> u64 {
        let kind = match self.kind {
            SourceBoundTopSlotRegistrationFailureKind::StaleGrant => 1,
            SourceBoundTopSlotRegistrationFailureKind::DuplicateTrace => 2,
        };
        [
            kind,
            host_source_digest(&self.source),
            confirmation_receiver_digest(&self.expected_receiver),
            issuer_projection_digest(issuer),
            optional_trace_digest(trace),
        ]
        .into_iter()
        .fold(0x5453_4f50_4f50_0001, projection_mix)
    }
}

pub enum AdmissionOwnerFailureEvidence {
    Operation(HostAdmissionOperationFailureFreezeOutcome),
    TraceCapacity(HostAdmissionTraceCapacityFailureEvidence),
}

pub struct AdmissionOwnerError<Owner> {
    pub owner: Owner,
    pub evidence: AdmissionOwnerFailureEvidence,
}

pub enum HostAdmissionSourcePermitOutcome {
    Issued(HostAdmissionSourcePermit),
    Ineligible {
        source: HostAdmissionSourceProjection,
        evidence: HostAdmissionSourceIneligibleProjection,
    },
    Outstanding {
        source: HostAdmissionSourceProjection,
    },
}

impl HostAdmissionSourcePermit {
    pub fn source_projection(&self) -> HostAdmissionSourceProjection {
        HostAdmissionSourceProjection {
            inner: self.inner.source.clone(),
        }
    }
}

impl SourceBoundHostAdmissionToken {
    pub fn source_projection(&self) -> HostAdmissionSourceProjection {
        HostAdmissionSourceProjection {
            inner: self.inner.permit.source.clone(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostAdmissionSourceIneligibleProjection {
    inner: CoreHostAdmissionSourceIneligibleProjection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostAdmissionUnavailableEvidenceProjection {
    inner: CoreHostAdmissionUnavailableEvidenceProjection,
}

impl HostAdmissionUnavailableEvidenceProjection {
    fn from_observation(
        source: &HostAdmissionSourceProjection,
        kind: CoreHostAdmissionUnavailableEvidenceKind,
        candidate_revision: Option<u64>,
        expected_digest: u64,
        observed_digest: u64,
        exhaustion: Option<HostAdmissionAllocatorExhaustionEvidence>,
    ) -> Self {
        Self {
            inner: CoreHostAdmissionUnavailableEvidenceProjection {
                source: source.inner.clone(),
                kind,
                candidate_revision,
                expected_digest,
                observed_digest,
                exhaustion: exhaustion.map(Into::into),
                operation_failure: None,
            },
        }
    }

    pub fn no_candidate(
        source: &HostAdmissionSourceProjection,
        candidate_revision: Option<u64>,
        expected_digest: u64,
        observed_digest: u64,
        exhaustion: Option<HostAdmissionAllocatorExhaustionEvidence>,
    ) -> Self {
        Self::from_observation(
            source,
            CoreHostAdmissionUnavailableEvidenceKind::NoCandidate,
            candidate_revision,
            expected_digest,
            observed_digest,
            exhaustion,
        )
    }

    pub fn conflict(
        source: &HostAdmissionSourceProjection,
        candidate_revision: Option<u64>,
        expected_digest: u64,
        observed_digest: u64,
    ) -> Self {
        Self::from_observation(
            source,
            CoreHostAdmissionUnavailableEvidenceKind::Conflict,
            candidate_revision,
            expected_digest,
            observed_digest,
            None,
        )
    }

    pub fn invalid_topology(
        source: &HostAdmissionSourceProjection,
        candidate_revision: Option<u64>,
        expected_digest: u64,
        observed_digest: u64,
    ) -> Self {
        Self::from_observation(
            source,
            CoreHostAdmissionUnavailableEvidenceKind::InvalidTopology,
            candidate_revision,
            expected_digest,
            observed_digest,
            None,
        )
    }

    pub fn host_lost(
        source: &HostAdmissionSourceProjection,
        candidate_revision: Option<u64>,
        expected_digest: u64,
        observed_digest: u64,
    ) -> Self {
        Self::from_observation(
            source,
            CoreHostAdmissionUnavailableEvidenceKind::HostLost,
            candidate_revision,
            expected_digest,
            observed_digest,
            None,
        )
    }

    pub fn authority_establishment_failed(
        source: &HostAdmissionSourceProjection,
        candidate_revision: Option<u64>,
        expected_digest: u64,
        observed_digest: u64,
        exhaustion: Option<HostAdmissionAllocatorExhaustionEvidence>,
    ) -> Self {
        Self::from_observation(
            source,
            CoreHostAdmissionUnavailableEvidenceKind::AuthorityEstablishmentFailed,
            candidate_revision,
            expected_digest,
            observed_digest,
            exhaustion,
        )
    }

    pub fn registry_invariant_violation(
        source: &HostAdmissionSourceProjection,
        candidate_revision: Option<u64>,
        expected_digest: u64,
        observed_digest: u64,
    ) -> Self {
        Self::from_observation(
            source,
            CoreHostAdmissionUnavailableEvidenceKind::RegistryInvariantViolation,
            candidate_revision,
            expected_digest,
            observed_digest,
            None,
        )
    }

    pub fn operation_failure(
        evidence: &HostAdmissionOperationFailureEvidence,
        expected_digest: u64,
        observed_digest: u64,
    ) -> Self {
        let operation_failure = evidence.inner.projection.clone();
        Self {
            inner: CoreHostAdmissionUnavailableEvidenceProjection {
                source: operation_failure.source.clone(),
                kind: CoreHostAdmissionUnavailableEvidenceKind::IssuerOperationFailed,
                candidate_revision: None,
                expected_digest,
                observed_digest,
                exhaustion: operation_failure.exhaustion,
                operation_failure: Some(operation_failure),
            },
        }
    }

    pub fn confirmation_failed(
        source: &HostAdmissionSourceProjection,
        candidate_revision: Option<u64>,
        expected_digest: u64,
        observed_digest: u64,
    ) -> Self {
        Self::from_observation(
            source,
            CoreHostAdmissionUnavailableEvidenceKind::ConfirmationFailed,
            candidate_revision,
            expected_digest,
            observed_digest,
            None,
        )
    }

    pub fn preallocation_rejected(
        source: &HostAdmissionSourceProjection,
        candidate_revision: Option<u64>,
        expected_digest: u64,
        observed_digest: u64,
    ) -> Self {
        Self::from_observation(
            source,
            CoreHostAdmissionUnavailableEvidenceKind::PreallocationRejected,
            candidate_revision,
            expected_digest,
            observed_digest,
            None,
        )
    }

    pub fn post_allocation_rejected(
        source: &HostAdmissionSourceProjection,
        candidate_revision: Option<u64>,
        expected_digest: u64,
        observed_digest: u64,
    ) -> Self {
        Self::from_observation(
            source,
            CoreHostAdmissionUnavailableEvidenceKind::PostAllocationRejected,
            candidate_revision,
            expected_digest,
            observed_digest,
            None,
        )
    }
}

pub struct PreparedHostAdmissionUnavailableCause<Evidence> {
    core: CoreHostAdmissionUnavailableCause,
    pub evidence: Evidence,
}

impl<Evidence> From<crate::engine::PreparedHostAdmissionUnavailableCause<Evidence>>
    for PreparedHostAdmissionUnavailableCause<Evidence>
{
    fn from(prepared: CorePreparedHostAdmissionUnavailableCause<Evidence>) -> Self {
        Self {
            core: prepared.core,
            evidence: prepared.evidence,
        }
    }
}

impl<Evidence> From<PreparedHostAdmissionUnavailableCause<Evidence>>
    for crate::engine::PreparedHostAdmissionUnavailableCause<Evidence>
{
    fn from(prepared: PreparedHostAdmissionUnavailableCause<Evidence>) -> Self {
        Self {
            core: prepared.core,
            evidence: prepared.evidence,
        }
    }
}

pub enum HostAdmissionUnavailableCauseBindOutcome<Evidence> {
    Bound(PreparedHostAdmissionUnavailableCause<Evidence>),
    Mismatch {
        source: HostAdmissionSourceProjection,
        evidence: Evidence,
        projection: HostAdmissionUnavailableEvidenceProjection,
        mismatch: HostAdmissionSourceIneligibleProjection,
    },
}

pub struct HostAdmissionUnavailablePreStagingSource<Installation, Evidence> {
    pub prepared: PreparedHostAdmissionUnavailableCause<Evidence>,
    pub owner: HostAdmissionUnavailablePreStagingOwner,
    pub installation: Installation,
}

impl<Installation, Evidence> From<HostAdmissionUnavailablePreStagingSource<Installation, Evidence>>
    for crate::engine::HostAdmissionUnavailablePreStagingSource<Installation, Evidence>
{
    fn from(source: HostAdmissionUnavailablePreStagingSource<Installation, Evidence>) -> Self {
        Self {
            prepared: source.prepared.into(),
            owner: source.owner.into(),
            installation: source.installation,
        }
    }
}

impl<Installation, Evidence>
    From<crate::engine::HostAdmissionUnavailablePreStagingSource<Installation, Evidence>>
    for HostAdmissionUnavailablePreStagingSource<Installation, Evidence>
{
    fn from(source: CoreHostAdmissionUnavailablePreStagingSource<Installation, Evidence>) -> Self {
        Self {
            prepared: source.prepared.into(),
            owner: source.owner.into(),
            installation: source.installation,
        }
    }
}

pub struct CoreHostConfirmationSource<Installation> {
    inner: InnerCoreHostConfirmationSource<Installation>,
}

impl<Installation> CoreHostConfirmationSource<Installation> {
    pub fn source_projection(&self) -> HostAdmissionSourceProjection {
        HostAdmissionSourceProjection {
            inner: self.inner.grant.projection().source,
        }
    }

    pub fn into_parts(self) -> (SourceBoundHostGrant, Installation) {
        (
            SourceBoundHostGrant {
                inner: self.inner.grant,
            },
            self.inner.installation,
        )
    }
}

pub fn bind_granted_host_admission_source<Installation>(
    grant: SourceBoundHostGrant,
    installation: Installation,
) -> CoreHostConfirmationSource<Installation> {
    CoreHostConfirmationSource {
        inner: crate::engine::bind_granted_host_admission_source(grant.inner, installation),
    }
}

pub struct HostAdmissionUnavailableGrantedSource<Installation, Evidence> {
    pub prepared: PreparedHostAdmissionUnavailableCause<Evidence>,
    pub owner: CoreHostConfirmationSource<Installation>,
}

impl<Installation, Evidence> From<HostAdmissionUnavailableGrantedSource<Installation, Evidence>>
    for crate::engine::HostAdmissionUnavailableGrantedSource<Installation, Evidence>
{
    fn from(source: HostAdmissionUnavailableGrantedSource<Installation, Evidence>) -> Self {
        Self {
            prepared: source.prepared.into(),
            owner: source.owner.inner,
        }
    }
}

impl<Installation, Evidence>
    From<crate::engine::HostAdmissionUnavailableGrantedSource<Installation, Evidence>>
    for HostAdmissionUnavailableGrantedSource<Installation, Evidence>
{
    fn from(source: CoreHostAdmissionUnavailableGrantedSource<Installation, Evidence>) -> Self {
        Self {
            prepared: source.prepared.into(),
            owner: CoreHostConfirmationSource {
                inner: source.owner,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostAdmissionPermanence {
    RetryableAfterCandidateChange,
    PermanentForRouter,
}

impl From<crate::engine::HostAdmissionPermanence> for HostAdmissionPermanence {
    fn from(permanence: CoreHostAdmissionPermanence) -> Self {
        match permanence {
            CoreHostAdmissionPermanence::RetryableAfterCandidateChange => {
                Self::RetryableAfterCandidateChange
            }
            CoreHostAdmissionPermanence::PermanentForRouter => Self::PermanentForRouter,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostAdmissionUnavailableTerminalProjection {
    pub source: HostAdmissionSourceProjection,
    pub cause_digest: u64,
    pub reason: crate::presentation::RouterUnavailableReason,
    pub permanence: HostAdmissionPermanence,
}

impl From<crate::engine::HostAdmissionUnavailableTerminalProjection>
    for HostAdmissionUnavailableTerminalProjection
{
    fn from(terminal: CoreHostAdmissionUnavailableTerminalProjection) -> Self {
        Self {
            source: HostAdmissionSourceProjection {
                inner: terminal.source,
            },
            cause_digest: terminal.cause_digest,
            reason: terminal.reason,
            permanence: terminal.permanence.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostAdmissionUnavailableMismatchProjection {
    pub expected_source: HostAdmissionSourceProjection,
    pub observed_pending: Option<PendingHostAdmissionProjection>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostAdmissionUnavailableObservedMismatchProjection {
    inner: Box<CoreHostAdmissionUnavailableObservedMismatchProjection>,
}

impl From<crate::engine::HostAdmissionUnavailableMismatchProjection>
    for HostAdmissionUnavailableMismatchProjection
{
    fn from(mismatch: CoreHostAdmissionUnavailableMismatchProjection) -> Self {
        Self {
            expected_source: HostAdmissionSourceProjection {
                inner: mismatch.expected_source,
            },
            observed_pending: mismatch
                .observed_pending
                .map(|inner| PendingHostAdmissionProjection { inner }),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostAdmissionUnavailableIssuerOperation {
    RefreezeCause,
    CancelToken,
    RejectGrant,
}

impl From<crate::engine::HostAdmissionUnavailableIssuerOperation>
    for HostAdmissionUnavailableIssuerOperation
{
    fn from(operation: CoreHostAdmissionUnavailableIssuerOperation) -> Self {
        match operation {
            CoreHostAdmissionUnavailableIssuerOperation::RefreezeCause => Self::RefreezeCause,
            CoreHostAdmissionUnavailableIssuerOperation::CancelToken => Self::CancelToken,
            CoreHostAdmissionUnavailableIssuerOperation::RejectGrant => Self::RejectGrant,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostAdmissionUnavailableIssuerMismatchProjection {
    pub receiver: HostAdmissionUnavailableMismatchProjection,
    pub operation: HostAdmissionUnavailableIssuerOperation,
    pub expected_issuer: HostAdmissionIssuerProjection,
    pub observed_issuer: HostAdmissionIssuerProjection,
}

impl From<crate::engine::HostAdmissionUnavailableIssuerMismatchProjection>
    for HostAdmissionUnavailableIssuerMismatchProjection
{
    fn from(mismatch: CoreHostAdmissionUnavailableIssuerMismatchProjection) -> Self {
        Self {
            receiver: mismatch.receiver.into(),
            operation: mismatch.operation.into(),
            expected_issuer: HostAdmissionIssuerProjection {
                inner: mismatch.expected_issuer,
            },
            observed_issuer: HostAdmissionIssuerProjection {
                inner: mismatch.observed_issuer,
            },
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HostAdmissionUnavailableFailureEvidence {
    Receiver(HostAdmissionUnavailableMismatchProjection),
    ObservedReceiver(HostAdmissionUnavailableObservedMismatchProjection),
    Issuer(HostAdmissionUnavailableIssuerMismatchProjection),
}

impl From<crate::engine::HostAdmissionUnavailableFailureEvidence>
    for HostAdmissionUnavailableFailureEvidence
{
    fn from(evidence: CoreHostAdmissionUnavailableFailureEvidence) -> Self {
        match evidence {
            CoreHostAdmissionUnavailableFailureEvidence::Receiver(mismatch) => {
                Self::Receiver(mismatch.into())
            }
            CoreHostAdmissionUnavailableFailureEvidence::ObservedReceiver(mismatch) => {
                Self::ObservedReceiver(HostAdmissionUnavailableObservedMismatchProjection {
                    inner: mismatch,
                })
            }
            CoreHostAdmissionUnavailableFailureEvidence::Issuer(mismatch) => {
                Self::Issuer(mismatch.into())
            }
        }
    }
}

pub enum HostAdmissionUnavailablePreStagingOutcome<Installation, Evidence, PhysicalTerminal> {
    Published {
        terminal: HostAdmissionUnavailableTerminalProjection,
        physical: PhysicalTerminal,
    },
    Mismatch {
        source: HostAdmissionUnavailablePreStagingSource<Installation, Evidence>,
        evidence: HostAdmissionUnavailableFailureEvidence,
    },
}

pub enum HostAdmissionUnavailableGrantedOutcome<Installation, Evidence, PhysicalTerminal> {
    Published {
        terminal: HostAdmissionUnavailableTerminalProjection,
        physical: PhysicalTerminal,
    },
    Mismatch {
        source: HostAdmissionUnavailableGrantedSource<Installation, Evidence>,
        evidence: HostAdmissionUnavailableFailureEvidence,
    },
}

pub enum HostAdmissionUnavailableObservedOutcome<Evidence> {
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostConfirmationReceiverProjection {
    inner: CoreHostConfirmationReceiverProjection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostConfirmationReceiverMismatchProjection {
    pub expected: HostConfirmationReceiverProjection,
    pub observed: Option<HostConfirmationReceiverProjection>,
}

impl From<crate::engine::HostConfirmationReceiverMismatchProjection>
    for HostConfirmationReceiverMismatchProjection
{
    fn from(mismatch: CoreHostConfirmationReceiverMismatchProjection) -> Self {
        Self {
            expected: HostConfirmationReceiverProjection {
                inner: mismatch.expected,
            },
            observed: mismatch
                .observed
                .map(|inner| HostConfirmationReceiverProjection { inner }),
        }
    }
}

impl From<HostConfirmationReceiverProjection>
    for crate::engine::HostConfirmationReceiverProjection
{
    fn from(projection: HostConfirmationReceiverProjection) -> Self {
        projection.inner
    }
}

impl From<HostConfirmationReceiverMismatchProjection>
    for crate::engine::HostConfirmationReceiverMismatchProjection
{
    fn from(mismatch: HostConfirmationReceiverMismatchProjection) -> Self {
        Self {
            expected: mismatch.expected.into(),
            observed: mismatch.observed.map(Into::into),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostConfirmationIssuerMismatchProjection {
    pub receiver: HostConfirmationReceiverProjection,
    pub expected_issuer: HostAdmissionIssuerProjection,
    pub observed_issuer: HostAdmissionIssuerProjection,
}

impl From<crate::engine::HostConfirmationIssuerMismatchProjection>
    for HostConfirmationIssuerMismatchProjection
{
    fn from(mismatch: CoreHostConfirmationIssuerMismatchProjection) -> Self {
        Self {
            receiver: HostConfirmationReceiverProjection {
                inner: mismatch.receiver,
            },
            expected_issuer: HostAdmissionIssuerProjection {
                inner: mismatch.expected_issuer,
            },
            observed_issuer: HostAdmissionIssuerProjection {
                inner: mismatch.observed_issuer,
            },
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HostAdmissionConfirmationFailureEvidence {
    Receiver(HostConfirmationReceiverMismatchProjection),
    Issuer(HostConfirmationIssuerMismatchProjection),
    TopSlotRegistrationOperationRejected(SourceBoundTopSlotRegistrationOperationFailureEvidence),
    TopSlotRegistrationRejected(HostTopSlotRegistrationCapacityFailureEvidence),
    ConfirmationPreparationRejected(HostConfirmationPreparationCapacityFailureEvidence),
    ConfirmationPublicationRejected(HostConfirmationPublicationCapacityFailureEvidence),
}

impl HostAdmissionConfirmationFailureEvidence {
    pub fn expected_digest(&self) -> u64 {
        match self {
            Self::Receiver(evidence) => {
                optional_confirmation_receiver_digest(Some(&evidence.expected))
            }
            Self::Issuer(evidence) => {
                issuer_confirmation_failure_digest(&evidence.receiver, &evidence.expected_issuer)
            }
            Self::TopSlotRegistrationOperationRejected(evidence) => evidence.expected_digest(),
            Self::TopSlotRegistrationRejected(evidence) => evidence.expected_digest(),
            Self::ConfirmationPreparationRejected(evidence) => evidence.expected_digest(),
            Self::ConfirmationPublicationRejected(evidence) => evidence.expected_digest(),
        }
    }

    pub fn observed_digest(&self) -> u64 {
        match self {
            Self::Receiver(evidence) => {
                optional_confirmation_receiver_digest(evidence.observed.as_ref())
            }
            Self::Issuer(evidence) => {
                issuer_confirmation_failure_digest(&evidence.receiver, &evidence.observed_issuer)
            }
            Self::TopSlotRegistrationOperationRejected(evidence) => evidence.observed_digest(),
            Self::TopSlotRegistrationRejected(evidence) => evidence.observed_digest(),
            Self::ConfirmationPreparationRejected(evidence) => evidence.observed_digest(),
            Self::ConfirmationPublicationRejected(evidence) => evidence.observed_digest(),
        }
    }
}

fn issuer_confirmation_failure_digest(
    receiver: &HostConfirmationReceiverProjection,
    issuer: &HostAdmissionIssuerProjection,
) -> u64 {
    [
        confirmation_receiver_digest(receiver),
        issuer_projection_digest(issuer),
    ]
    .into_iter()
    .fold(0x434f_4e46_4953_0001, projection_mix)
}

pub enum SourceBoundTopSlotRegistrationError {
    Operation(SourceBoundTopSlotRegistrationOperationFailureEvidence),
    TraceCapacity(HostTopSlotRegistrationCapacityFailureEvidence),
}

impl From<crate::engine::HostAdmissionConfirmationFailureEvidence>
    for HostAdmissionConfirmationFailureEvidence
{
    fn from(evidence: CoreHostAdmissionConfirmationFailureEvidence) -> Self {
        match evidence {
            CoreHostAdmissionConfirmationFailureEvidence::Receiver(mismatch) => {
                Self::Receiver(mismatch.into())
            }
            CoreHostAdmissionConfirmationFailureEvidence::Issuer(mismatch) => {
                Self::Issuer(mismatch.into())
            }
        }
    }
}

pub struct CorePreparedHostConfirmation<Installation> {
    inner: InnerCorePreparedHostConfirmation<Installation>,
    ready_trace: HostTraceEvent,
}

pub struct RejectedHostConfirmationPreparation<Installation> {
    grant: SourceBoundHostGrant,
    installation: Installation,
    evidence: HostConfirmationReceiverMismatchProjection,
}

impl<Installation> RejectedHostConfirmationPreparation<Installation> {
    pub fn into_parts(
        self,
    ) -> (
        SourceBoundHostGrant,
        Installation,
        HostConfirmationReceiverMismatchProjection,
    ) {
        (self.grant, self.installation, self.evidence)
    }
}

pub fn consume_host_confirmation_preparation_mismatch<Installation>(
    owner: CoreHostConfirmationSource<Installation>,
    evidence: HostConfirmationReceiverMismatchProjection,
) -> RejectedHostConfirmationPreparation<Installation> {
    let rejected = crate::engine::consume_host_confirmation_preparation_mismatch(
        owner.inner,
        evidence.clone().into(),
    );
    let (grant, installation, _) = rejected.into_parts();
    RejectedHostConfirmationPreparation {
        grant: SourceBoundHostGrant { inner: grant },
        installation,
        evidence,
    }
}

pub enum HostConfirmationPreparationOutcome<Installation> {
    Prepared(CorePreparedHostConfirmation<Installation>),
    PublicationPreparationRejected {
        owner: CoreHostConfirmationSource<Installation>,
        evidence: HostConfirmationPreparationCapacityFailureEvidence,
    },
    ReceiverMismatch {
        owner: CoreHostConfirmationSource<Installation>,
        evidence: HostConfirmationReceiverMismatchProjection,
    },
}

pub enum HostConfirmationUnwindOutcome<Installation> {
    Grant(CoreHostConfirmationSource<Installation>),
    Mismatch {
        owner: CorePreparedHostConfirmation<Installation>,
        evidence: HostConfirmationReceiverMismatchProjection,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostAdmissionReadyProjection {
    pub source: HostAdmissionSourceProjection,
    pub host: HostVersion,
    pub top_slot: RouteSlotId,
    pub availability: RouterAvailability,
}

impl From<crate::engine::HostAdmissionReadyProjection> for HostAdmissionReadyProjection {
    fn from(projection: CoreHostAdmissionReadyProjection) -> Self {
        Self {
            source: HostAdmissionSourceProjection {
                inner: projection.source,
            },
            host: projection.host,
            top_slot: projection.top_slot,
            availability: projection.availability,
        }
    }
}

pub enum HostAdmissionConfirmationCoreOutcome<Installation, Installed> {
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
    PublicationPreparationRejected {
        owner: CorePreparedHostConfirmation<Installation>,
        evidence: HostConfirmationPublicationCapacityFailureEvidence,
    },
}

#[doc(hidden)]
pub struct TestingHostAdmissionOperationInput {
    pub source: HostAdmissionSourceProjection,
    pub expected_issuer: HostAdmissionIssuerProjection,
    pub observed_issuer: HostAdmissionIssuerProjection,
    pub outstanding: HostAdmissionIssuerOutstandingProjection,
    pub pending: Option<PendingHostAdmissionProjection>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntegrationError {
    Exhausted,
    AwaitingHost,
    OutstandingAdmission,
    AuthorityExists,
    ForeignRouter,
    Duplicate,
    Stale,
    Invalid,
    InvalidManifest,
}

#[doc(hidden)]
#[cfg(any(test, feature = "test-support"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TestingReservationCounter {
    Request,
    Transaction,
    FrameTask,
    CandidateFrame,
    CleanupBatch,
}

#[cfg(any(test, feature = "test-support"))]
impl TestingReservationCounter {
    pub const ALL: [Self; 5] = [
        Self::Request,
        Self::Transaction,
        Self::FrameTask,
        Self::CandidateFrame,
        Self::CleanupBatch,
    ];
}

#[doc(hidden)]
pub struct HostLease {
    inner: CoreHostLease,
}

#[doc(hidden)]
pub struct FailedAuthoritySettlementTrigger {
    inner: CoreFailedAuthoritySettlementTrigger,
}

#[cfg(test)]
impl FailedAuthoritySettlementTrigger {
    pub fn testing_duplicate(&self) -> Self {
        Self {
            inner: self.inner.testing_duplicate(),
        }
    }
}

#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailedAuthorityTriggerError {
    Foreign,
    Stale,
    WrongPhase,
    Exhausted,
}

#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BeginAuthoritySettlementErrorKind {
    Foreign,
    Stale,
    WrongPhase,
    InvalidManifest,
    RetryablePreparation,
}

#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreCommitAuthorityCause {
    AuthorityLost,
    HostLost,
}

impl PreCommitAuthorityCause {
    const fn inner(self) -> CorePreCommitAuthorityCause {
        match self {
            Self::AuthorityLost => CorePreCommitAuthorityCause::AuthorityLost,
            Self::HostLost => CorePreCommitAuthorityCause::HostLost,
        }
    }
}

#[must_use = "failed authority settlement begin returns the exact host lease"]
#[doc(hidden)]
pub struct BeginAuthoritySettlementError {
    kind: BeginAuthoritySettlementErrorKind,
    owner: HostLease,
}

impl BeginAuthoritySettlementError {
    pub fn kind(&self) -> BeginAuthoritySettlementErrorKind {
        self.kind
    }

    pub fn into_owner(self) -> HostLease {
        self.owner
    }
}

impl std::fmt::Debug for BeginAuthoritySettlementError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BeginAuthoritySettlementError")
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}

#[doc(hidden)]
pub enum ResolveMountOwner {
    Returned(MountRequest),
    Reissued,
    Consumed,
}

#[doc(hidden)]
#[must_use = "a failed mount resolution returns, reissues, or terminally consumes its request capability"]
pub struct ResolveMountError {
    kind: IntegrationError,
    owner: ResolveMountOwner,
}

impl ResolveMountError {
    pub const fn kind(&self) -> IntegrationError {
        self.kind
    }

    pub fn into_owner(self) -> ResolveMountOwner {
        self.owner
    }
}

#[doc(hidden)]
#[must_use = "a failed mount rejection retains its request capability"]
pub struct RejectMountError {
    kind: IntegrationError,
    owner: MountRequest,
}

impl RejectMountError {
    pub const fn kind(&self) -> IntegrationError {
        self.kind
    }

    pub fn into_owner(self) -> MountRequest {
        self.owner
    }
}

#[doc(hidden)]
#[must_use = "a failed receipt acceptance retains its attestation capability"]
pub struct AcceptMountReceiptError {
    kind: IntegrationError,
    owner: MountReceiptAttestation,
}

impl AcceptMountReceiptError {
    pub const fn kind(&self) -> IntegrationError {
        self.kind
    }

    pub fn into_owner(self) -> MountReceiptAttestation {
        self.owner
    }
}

#[doc(hidden)]
#[must_use = "a failed mutation rejection retains its mutation capability"]
pub struct RejectMountMutationError {
    kind: IntegrationError,
    owner: MountMutationArtifact,
}

impl RejectMountMutationError {
    pub const fn kind(&self) -> IntegrationError {
        self.kind
    }

    pub fn into_owner(self) -> MountMutationArtifact {
        self.owner
    }
}

#[doc(hidden)]
#[must_use = "a failed cleanup preparation retains its cleanup request"]
pub struct PrepareFailureCleanupError {
    kind: IntegrationError,
    owner: FailureCleanupRequest,
}

impl PrepareFailureCleanupError {
    pub const fn kind(&self) -> IntegrationError {
        self.kind
    }

    pub fn into_owner(self) -> FailureCleanupRequest {
        self.owner
    }
}

fn format_capability_error(
    formatter: &mut std::fmt::Formatter<'_>,
    name: &str,
    kind: IntegrationError,
) -> std::fmt::Result {
    formatter
        .debug_struct(name)
        .field("kind", &kind)
        .finish_non_exhaustive()
}

impl std::fmt::Debug for ResolveMountError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        format_capability_error(formatter, "ResolveMountError", self.kind)
    }
}

impl std::fmt::Debug for RejectMountError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        format_capability_error(formatter, "RejectMountError", self.kind)
    }
}

impl std::fmt::Debug for AcceptMountReceiptError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        format_capability_error(formatter, "AcceptMountReceiptError", self.kind)
    }
}

impl std::fmt::Debug for RejectMountMutationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        format_capability_error(formatter, "RejectMountMutationError", self.kind)
    }
}

impl std::fmt::Debug for PrepareFailureCleanupError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        format_capability_error(formatter, "PrepareFailureCleanupError", self.kind)
    }
}

pub struct HostAdmissionToken {
    inner: CoreHostAdmissionToken,
}

pub struct HostGrant {
    inner: CoreHostGrant,
}

pub struct MountGrant {
    inner: CoreMountGrant,
}

impl MountGrant {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) const fn sealed(inner: CoreMountGrant) -> Self {
        Self { inner }
    }

    pub fn slot(&self) -> crate::ids::RouteSlotId {
        self.inner.slot()
    }

    #[doc(hidden)]
    #[cfg(any(test, feature = "test-support"))]
    pub fn testing_duplicate(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }

    #[cfg(test)]
    fn attest_receipt(self, digest: u64) -> MountReceiptAttestation {
        MountReceiptAttestation {
            inner: self
                .inner
                .attest_receipt(MountReceiptDigest::from_opaque(digest)),
        }
    }

    pub fn attest_receipt_with_proof(
        self,
        digest: u64,
    ) -> (MountReceiptAttestation, MountReceiptProof) {
        let (attestation, proof) = self
            .inner
            .attest_receipt_with_proof(MountReceiptDigest::from_opaque(digest));
        (
            MountReceiptAttestation { inner: attestation },
            MountReceiptProof { inner: proof },
        )
    }
}

pub struct MountReceiptAttestation {
    #[cfg_attr(not(test), allow(dead_code))]
    inner: CoreMountReceiptAttestation,
}

impl MountReceiptAttestation {
    #[doc(hidden)]
    #[cfg(any(test, feature = "test-support"))]
    pub fn testing_duplicate(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

pub struct MountReceiptProof {
    inner: CoreMountReceiptProof,
}

pub struct MountRegistrationPermit {
    inner: CoreMountRegistrationPermit,
}

impl MountRegistrationPermit {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) const fn sealed(inner: CoreMountRegistrationPermit) -> Self {
        Self { inner }
    }

    #[cfg(test)]
    fn slot(&self) -> crate::ids::RouteSlotId {
        self.inner.slot()
    }

    #[doc(hidden)]
    #[cfg(any(test, feature = "test-support"))]
    pub fn testing_duplicate(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }

    #[cfg(test)]
    fn complete(self, registration_digest: u64) -> MountRegistrationCompletion {
        MountRegistrationCompletion {
            inner: self
                .inner
                .complete(MountRegistrationDigest::from_opaque(registration_digest)),
        }
    }
}

#[cfg(test)]
struct MountRegistrationCompletion {
    inner: CoreMountRegistrationCompletion,
}

#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
pub struct MountRegistrationReplays {
    completion: CoreMountRegistrationCompletion,
    confirmation: crate::mount::MountConfirmation,
}

#[must_use = "a mount registration session must complete or reject"]
pub struct MountRegistrationSession<'engine> {
    engine: &'engine mut CoreRouterEngine,
    permit: CoreMountRegistrationPermit,
    slot: RouteSlotId,
}

impl MountRegistrationSession<'_> {
    pub const fn slot(&self) -> RouteSlotId {
        self.slot
    }

    pub fn complete(self, registration_digest: u64) {
        self.engine.complete_preflighted_mount_registration(
            self.permit,
            MountRegistrationDigest::from_opaque(registration_digest),
        )
    }

    #[doc(hidden)]
    #[cfg(any(test, feature = "test-support"))]
    pub fn testing_complete_with_replays(
        self,
        registration_digest: u64,
    ) -> Result<MountRegistrationReplays, IntegrationError> {
        let (completion, confirmation) = self
            .engine
            .testing_complete_preflighted_mount_registration_with_replays(
                self.permit,
                MountRegistrationDigest::from_opaque(registration_digest),
            )
            .map_err(map_mount_error)?;
        Ok(MountRegistrationReplays {
            completion,
            confirmation,
        })
    }

    pub fn reject(self) -> FailureCleanupRequest {
        FailureCleanupRequest {
            inner: self
                .engine
                .reject_preflighted_mount_registration(self.permit),
        }
    }
}

#[must_use = "a failed mount registration begin retains the rejection capability"]
pub struct MountRegistrationBeginError {
    kind: IntegrationError,
    permit: CoreMountRegistrationPermit,
}

impl MountRegistrationBeginError {
    pub const fn kind(&self) -> IntegrationError {
        self.kind
    }

    #[doc(hidden)]
    pub fn validate_rejection(&self, engine: &RouterEngine) -> Result<(), IntegrationError> {
        engine
            .inner
            .validate_mount_registration_rejection(&self.permit)
            .map_err(map_mount_error)
    }

    pub fn reject(
        self,
        engine: &mut RouterEngine,
    ) -> Result<FailureCleanupRequest, MountRegistrationBeginError> {
        if let Err(kind) = self.validate_rejection(engine) {
            return Err(Self {
                kind,
                permit: self.permit,
            });
        }
        Ok(FailureCleanupRequest {
            inner: engine
                .inner
                .reject_preflighted_mount_registration(self.permit),
        })
    }
}

impl std::fmt::Debug for MountRegistrationBeginError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MountRegistrationBeginError")
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}

impl HostGrant {
    pub fn version(&self) -> &crate::HostVersion {
        self.inner.version()
    }

    pub fn top_slot(&self) -> crate::ids::RouteSlotId {
        self.inner.top_slot()
    }
}

pub fn host_version_matches_frame(version: &crate::HostVersion, frame: &RouteFrameVersion) -> bool {
    host_version_shares_router_with_frame(version, frame) && version.host() == frame.host()
}

#[doc(hidden)]
pub fn host_version_shares_router_with_frame(
    version: &crate::HostVersion,
    frame: &RouteFrameVersion,
) -> bool {
    version.router_identity() == frame.router_identity()
}

#[doc(hidden)]
pub fn frame_versions_share_router(left: &RouteFrameVersion, right: &RouteFrameVersion) -> bool {
    left.router_identity() == right.router_identity()
}

#[doc(hidden)]
pub fn mount_grant_matches_target(
    grant: &MountGrant,
    parent: &RouteFrameVersion,
    outlet: &crate::slot::RouteSlotKey,
) -> bool {
    grant.inner.matches_target(parent, outlet)
}

pub fn root_view(tree: &RouteTree) -> Option<&RouteViewKey> {
    tree.node(tree.root())?.options().view_binding()
}

pub fn for_each_route_view_binding(
    tree: &RouteTree,
    mut visit: impl FnMut(Option<&RouteViewKey>, Option<&ErrorBoundaryKey>, Option<&RouteViewKey>),
) {
    for node in tree.nodes_in_declaration_order() {
        let options = node.options();
        visit(
            options.view_binding(),
            options.error_boundary(),
            options.error_view_binding().map(|(_, view)| view),
        );
    }
}

pub struct FrameTask {
    inner: CoreFrameTask,
}

pub type RootFrameTask = FrameTask;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FrameMountTarget {
    TopSlot {
        host: crate::HostVersion,
        slot: RouteSlotId,
    },
    ChildOutlet {
        parent: RouteFrameVersion,
        outlet: RouteSlotKey,
    },
}

#[derive(Clone, Eq, PartialEq)]
pub struct ViewReportIdentity {
    inner: crate::engine::RootTaskIdentity,
}

impl FrameTask {
    #[doc(hidden)]
    #[cfg(any(test, feature = "test-support"))]
    pub fn testing_duplicate(&self) -> Self {
        Self {
            inner: self.inner.testing_duplicate(),
        }
    }

    pub fn report_identity(&self) -> ViewReportIdentity {
        ViewReportIdentity {
            inner: self.inner.identity().clone(),
        }
    }

    pub fn request(&self) -> NavigationRequestId {
        self.inner.identity().request()
    }

    pub fn view_request_sequence(&self) -> u64 {
        self.inner.identity().frame_task().raw()
    }

    pub fn frame(&self) -> &RouteFrameVersion {
        let current = &self.inner;
        // The engine creates the same logical version from this sealed identity
        // and the projection is the only cross-crate representation ECS needs
        current.frame_version()
    }

    pub fn view(&self) -> &RouteViewKey {
        self.inner.view()
    }

    pub fn input(&self) -> &RouteViewInput {
        self.inner.input()
    }

    pub fn take_input(&mut self) -> RouteViewInput {
        self.inner.take_input()
    }

    pub fn mount_target(&self) -> FrameMountTarget {
        match self.inner.mount_target() {
            CoreFrameMountTarget::TopSlot { host, slot } => FrameMountTarget::TopSlot {
                host: host.clone(),
                slot: *slot,
            },
            CoreFrameMountTarget::ChildOutlet { parent, outlet } => FrameMountTarget::ChildOutlet {
                parent: parent.clone(),
                outlet: outlet.clone(),
            },
            CoreFrameMountTarget::RegisteredChildSlot { parent, outlet } => {
                FrameMountTarget::ChildOutlet {
                    parent: parent.clone(),
                    outlet: outlet.clone(),
                }
            }
        }
    }

    #[doc(hidden)]
    #[cfg(any(test, feature = "test-support"))]
    pub fn testing_replace_top_slot_mount_target(
        &mut self,
        host: crate::HostVersion,
        slot: RouteSlotId,
    ) -> bool {
        self.inner.testing_replace_top_slot_mount_target(host, slot)
    }
}

pub struct MountRequest {
    inner: CoreFrameMountRequest,
}

impl MountRequest {
    pub fn request(&self) -> NavigationRequestId {
        self.inner.request()
    }

    pub fn parent(&self) -> &RouteFrameVersion {
        self.inner.parent()
    }

    pub fn outlet(&self) -> &RouteSlotKey {
        self.inner.outlet()
    }

    #[doc(hidden)]
    #[cfg(any(test, feature = "test-support"))]
    pub fn testing_duplicate(&self) -> Self {
        Self {
            inner: self.inner.testing_duplicate(),
        }
    }
}

#[doc(hidden)]
pub enum MountMutationArtifact {
    Grant(MountGrant),
    #[doc(hidden)]
    Receipt(MountReceiptAttestation),
    RegistrationPermit(MountRegistrationPermit),
}

#[doc(hidden)]
pub struct FailureCleanupRequest {
    inner: CoreFailureCleanupRequest,
}

impl FailureCleanupRequest {
    pub fn request(&self) -> NavigationRequestId {
        self.inner.request()
    }

    pub fn logical_roots(&self) -> &[RouteFrameVersion] {
        self.inner.logical_roots()
    }

    pub fn retained_prepared_prefix(&self) -> &[RouteFrameVersion] {
        self.inner.retained_prepared_prefix()
    }

    pub fn failure(&self) -> &NavigationFailure {
        self.inner.failure()
    }
}

#[doc(hidden)]
pub struct FailureCleanupReceiverMismatchView {
    inner: CoreFailureCleanupReceiverMismatchProjection,
}

impl FailureCleanupReceiverMismatchView {
    pub fn kind(&self) -> IntegrationError {
        map_failure_cleanup_error(self.inner.kind)
    }
}

#[doc(hidden)]
pub struct FailureCleanupManifestContradictionView {
    inner: CoreFailureCleanupManifestContradictionProjection,
}

impl FailureCleanupManifestContradictionView {
    pub const fn submitted_obligation_count(&self) -> usize {
        self.inner.submitted_obligation_count
    }

    pub fn expected_logical_roots(&self) -> &[RouteFrameVersion] {
        &self.inner.expected_logical_roots
    }

    pub fn observed_logical_roots(&self) -> &[RouteFrameVersion] {
        &self.inner.observed_logical_roots
    }
}

#[doc(hidden)]
pub struct FailureCleanupPrepareMismatch {
    request: FailureCleanupRequest,
    evidence: FailureCleanupReceiverMismatchView,
}

impl FailureCleanupPrepareMismatch {
    pub fn into_parts(self) -> (FailureCleanupRequest, FailureCleanupReceiverMismatchView) {
        (self.request, self.evidence)
    }
}

#[doc(hidden)]
pub struct PermanentFailureCleanup {
    request: FailureCleanupRequest,
    evidence: FailureCleanupManifestContradictionView,
}

impl PermanentFailureCleanup {
    pub fn into_parts(
        self,
    ) -> (
        FailureCleanupRequest,
        FailureCleanupManifestContradictionView,
    ) {
        (self.request, self.evidence)
    }
}

#[doc(hidden)]
pub enum FailureCleanupPrepareOutcome {
    Prepared(CleanupBatch),
    Mismatch(FailureCleanupPrepareMismatch),
    Permanent(PermanentFailureCleanup),
}

#[doc(hidden)]
pub struct FailureCleanupPhysicalBinding<Physical> {
    physical: Physical,
    observed: CoreFailureCleanupZeroPhysicalProjection,
}

impl<Physical> FailureCleanupPhysicalBinding<Physical> {
    pub fn request(&self) -> NavigationRequestId {
        self.observed.receiver.request()
    }

    pub fn physical(&self) -> &Physical {
        &self.physical
    }
}

#[doc(hidden)]
pub struct PreparedFailureCleanupPhysical<Physical> {
    batch: CleanupBatch,
    physical: Physical,
}

impl<Physical> PreparedFailureCleanupPhysical<Physical> {
    pub fn into_parts(self) -> (CleanupBatch, Physical) {
        (self.batch, self.physical)
    }
}

#[doc(hidden)]
pub struct FailureCleanupPhysicalPrepareMismatch<Physical> {
    request: FailureCleanupRequest,
    evidence: FailureCleanupReceiverMismatchView,
    physical: FailureCleanupPhysicalBinding<Physical>,
}

impl<Physical> FailureCleanupPhysicalPrepareMismatch<Physical> {
    pub fn physical(&self) -> &Physical {
        &self.physical.physical
    }

    pub fn bound_request(&self) -> NavigationRequestId {
        self.physical.request()
    }

    pub fn into_parts(
        self,
    ) -> (
        FailureCleanupRequest,
        FailureCleanupReceiverMismatchView,
        FailureCleanupPhysicalBinding<Physical>,
    ) {
        (self.request, self.evidence, self.physical)
    }
}

#[doc(hidden)]
pub struct FailureCleanupPermanentOwner<Physical> {
    inner: CoreFailureCleanupPermanentHolder<FailureCleanupPhysicalBinding<Physical>>,
}

impl<Physical> FailureCleanupPermanentOwner<Physical> {
    pub fn request(&self) -> NavigationRequestId {
        self.inner.request.request()
    }

    pub fn physical(&self) -> &Physical {
        &self.inner.physical.physical
    }
}

#[doc(hidden)]
pub enum FailureCleanupPhysicalPrepareOutcome<Physical> {
    Prepared(PreparedFailureCleanupPhysical<Physical>),
    Mismatch(FailureCleanupPhysicalPrepareMismatch<Physical>),
    Permanent(FailureCleanupPermanentOwner<Physical>),
}

#[doc(hidden)]
pub struct FailureCleanupZeroPhysicalMismatchView {
    inner: CoreFailureCleanupZeroPhysicalMismatchProjection,
}

impl FailureCleanupZeroPhysicalMismatchView {
    pub fn source_request(&self) -> NavigationRequestId {
        self.inner.source.request()
    }

    pub fn expected_request(&self) -> NavigationRequestId {
        self.inner.expected.receiver.request()
    }

    pub fn observed_request(&self) -> NavigationRequestId {
        self.inner.observed.receiver.request()
    }

    pub fn source_matches_expected_receiver(&self) -> bool {
        self.inner.source == self.inner.expected.receiver
    }

    pub fn source_matches_observed_receiver(&self) -> bool {
        self.inner.source == self.inner.observed.receiver
    }

    pub fn expected_manifest_digest_matches(&self, manifest_digest: u64) -> bool {
        self.inner.expected.manifest_digest == CleanupManifestDigest::from_opaque(manifest_digest)
    }

    pub fn observed_manifest_digest_matches(&self, manifest_digest: u64) -> bool {
        self.inner.observed.manifest_digest == CleanupManifestDigest::from_opaque(manifest_digest)
    }

    pub fn expected_obligation_count(&self) -> usize {
        self.inner.expected.obligation_count
    }

    pub fn observed_obligation_count(&self) -> usize {
        self.inner.observed.obligation_count
    }
}

#[doc(hidden)]
pub struct FailureCleanupPermanentAuthorityOwner {
    inner: CoreFailureCleanupPermanentAuthorityOwner,
}

impl FailureCleanupPermanentAuthorityOwner {
    pub fn request(&self) -> NavigationRequestId {
        self.inner.request()
    }
}

#[doc(hidden)]
pub struct FailureCleanupPermanentAuthorityPhysical<Physical> {
    inner: FailureCleanupPhysicalBinding<Physical>,
}

impl<Physical> FailureCleanupPermanentAuthorityPhysical<Physical> {
    pub fn physical(&self) -> &Physical {
        &self.inner.physical
    }
}

#[doc(hidden)]
pub struct PreparedFailureCleanupZeroPhysical<Physical> {
    inner: CorePreparedFailureCleanupZeroPhysical<FailureCleanupPhysicalBinding<Physical>>,
}

impl<Physical> PreparedFailureCleanupZeroPhysical<Physical> {
    pub fn physical(&self) -> &Physical {
        &self.inner.physical.physical
    }
}

#[doc(hidden)]
pub struct FailureCleanupZeroTerminalOwner {
    inner: CoreFailureCleanupZeroTerminalOwner,
}

#[doc(hidden)]
pub struct FailureCleanupZeroTerminalMismatchView {
    inner: CoreFailureCleanupZeroTerminalMismatchProjection,
}

#[doc(hidden)]
pub enum FailureCleanupPermanentOutcome<Physical> {
    AuthoritySettlement {
        physical: FailureCleanupPermanentAuthorityPhysical<Physical>,
        owner: FailureCleanupPermanentAuthorityOwner,
    },
    ProvenZero {
        zero: PreparedFailureCleanupZeroPhysical<Physical>,
        owner: FailureCleanupZeroTerminalOwner,
    },
    ZeroMismatch {
        holder: FailureCleanupPermanentOwner<Physical>,
        evidence: FailureCleanupZeroPhysicalMismatchView,
    },
    Mismatch(FailureCleanupPermanentOwner<Physical>),
}

#[doc(hidden)]
pub enum FailureCleanupZeroTerminalOutcome<Physical> {
    Terminal,
    Mismatch {
        zero: PreparedFailureCleanupZeroPhysical<Physical>,
        owner: FailureCleanupZeroTerminalOwner,
        evidence: FailureCleanupZeroTerminalMismatchView,
    },
}

#[doc(hidden)]
pub struct FailureCleanupCompletionOwner {
    inner: CoreFailureCleanupCompletionOwner,
}

#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailureCleanupCompletionDisposition {
    Settled,
    RetainedPreparedPrefixBound,
    RetainedPreparedPrefixCleanup,
}

#[doc(hidden)]
pub struct FailureCleanupCompletionReceiverMismatchView {
    inner: CoreFailureCleanupCompletionReceiverMismatchProjection,
}

#[doc(hidden)]
pub struct FailureCleanupCompletionPreparationMismatchView {
    inner: CoreFailureCleanupCompletionPreparationMismatchProjection,
}

#[doc(hidden)]
#[must_use = "a failed cleanup completion preparation retains typed receiver evidence"]
pub struct PrepareFailureCleanupCompletionError {
    kind: IntegrationError,
    mismatch: FailureCleanupCompletionPreparationMismatchView,
}

impl PrepareFailureCleanupCompletionError {
    pub const fn kind(&self) -> IntegrationError {
        self.kind
    }

    pub fn into_mismatch(self) -> FailureCleanupCompletionPreparationMismatchView {
        self.mismatch
    }
}

impl std::fmt::Debug for PrepareFailureCleanupCompletionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        format_capability_error(formatter, "PrepareFailureCleanupCompletionError", self.kind)
    }
}

#[doc(hidden)]
#[must_use = "a failed cleanup completion retains its linear Core and physical owners"]
pub struct CompleteFailureCleanupError<Physical> {
    kind: IntegrationError,
    owner: FailureCleanupCompletionOwner,
    physical: Physical,
    mismatch: FailureCleanupCompletionReceiverMismatchView,
}

impl<Physical> CompleteFailureCleanupError<Physical> {
    pub const fn kind(&self) -> IntegrationError {
        self.kind
    }

    pub fn into_parts(
        self,
    ) -> (
        FailureCleanupCompletionOwner,
        Physical,
        FailureCleanupCompletionReceiverMismatchView,
    ) {
        (self.owner, self.physical, self.mismatch)
    }
}

impl<Physical> std::fmt::Debug for CompleteFailureCleanupError<Physical> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        format_capability_error(formatter, "CompleteFailureCleanupError", self.kind)
    }
}

pub enum EngineAction {
    FrameTask(FrameTask),
    MountRequest(MountRequest),
    #[doc(hidden)]
    FailureCleanup(FailureCleanupRequest),
    Commit(CommitTicket),
    #[doc(hidden)]
    BackResolution(BackResolutionOwnerHandoff),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewFailureKind {
    Build,
    Apply,
}

pub struct CommitTicket {
    inner: CoreCommitTicket,
}

impl CommitTicket {
    #[doc(hidden)]
    #[cfg(any(test, feature = "test-support"))]
    pub fn testing_duplicate(&self) -> Self {
        Self {
            inner: self.inner.testing_duplicate(),
        }
    }

    #[doc(hidden)]
    pub fn report_identity(&self) -> ViewReportIdentity {
        ViewReportIdentity {
            inner: self.inner.identity().clone(),
        }
    }
}

pub struct CleanupManifest {
    logical_roots: Vec<RouteFrameVersion>,
    manifest_digest: u64,
    obligation_count: usize,
}

#[doc(hidden)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CleanupManifestView {
    logical_roots: Vec<RouteFrameVersion>,
    manifest_digest: u64,
    obligation_count: usize,
}

impl CleanupManifest {
    pub fn empty() -> Self {
        Self {
            logical_roots: Vec::new(),
            manifest_digest: 0,
            obligation_count: 0,
        }
    }

    pub fn new(
        logical_roots: Vec<RouteFrameVersion>,
        manifest_digest: u64,
        obligation_count: usize,
    ) -> Self {
        Self {
            logical_roots,
            manifest_digest,
            obligation_count,
        }
    }

    pub fn projection(&self) -> CleanupManifestView {
        CleanupManifestView {
            logical_roots: self.logical_roots.clone(),
            manifest_digest: self.manifest_digest,
            obligation_count: self.obligation_count,
        }
    }

    pub fn manifest_digest(&self) -> u64 {
        self.manifest_digest
    }

    pub fn obligation_count(&self) -> usize {
        self.obligation_count
    }

    #[doc(hidden)]
    #[cfg(any(test, feature = "test-support"))]
    pub fn testing_with_obligation_count(mut self, obligation_count: usize) -> Self {
        self.obligation_count = obligation_count;
        self
    }
}

impl CleanupManifestView {
    pub fn manifest_digest(&self) -> u64 {
        self.manifest_digest
    }

    pub fn obligation_count(&self) -> usize {
        self.obligation_count
    }
}

pub struct CleanupBatch {
    inner: CleanupBatchToken,
    manifest_digest: u64,
    obligation_count: usize,
}

impl Clone for CleanupBatch {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            manifest_digest: self.manifest_digest,
            obligation_count: self.obligation_count,
        }
    }
}

impl CleanupBatch {
    pub fn manifest_digest(&self) -> u64 {
        self.manifest_digest
    }

    pub fn obligation_count(&self) -> usize {
        self.obligation_count
    }

    pub fn complete_empty(self) -> CleanupCompletion {
        assert_eq!(self.obligation_count, 0, "cleanup batch is not empty");
        CleanupCompletion {
            inner: CoreCleanupCompletion::attest(
                self.inner,
                CleanupCompletionDigest::canonical_empty(),
                0,
            ),
            manifest_digest: self.manifest_digest,
            completion_digest: 0,
            settled_count: 0,
        }
    }

    pub fn complete(self, completion_digest: u64, settled_count: usize) -> CleanupCompletion {
        CleanupCompletion {
            inner: CoreCleanupCompletion::attest(
                self.inner,
                CleanupCompletionDigest::from_opaque(completion_digest),
                settled_count,
            ),
            manifest_digest: self.manifest_digest,
            completion_digest,
            settled_count,
        }
    }
}

pub struct CleanupCompletion {
    inner: CoreCleanupCompletion,
    manifest_digest: u64,
    completion_digest: u64,
    settled_count: usize,
}

#[doc(hidden)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PostCommitCleanupBatchView {
    inner: CoreCleanupBatchProjection,
    manifest_digest: u64,
    obligation_count: usize,
}

impl PostCommitCleanupBatchView {
    pub fn manifest_digest(&self) -> u64 {
        self.manifest_digest
    }

    pub fn obligation_count(&self) -> usize {
        self.obligation_count
    }

    pub fn is_exact_core_projection(&self) -> bool {
        self.inner.manifest_digest_matches(self.manifest_digest)
            && self.inner.obligation_count() == self.obligation_count
    }
}

#[doc(hidden)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PostCommitCleanupReceiverView {
    inner: CorePostCommitCleanupReceiverProjection,
}

impl PostCommitCleanupReceiverView {
    pub fn request(&self) -> NavigationRequestId {
        self.inner.request()
    }
}

#[doc(hidden)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PostCommitCleanupObservedView {
    inner: CorePostCommitCleanupObservedProjection,
}

#[doc(hidden)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PostCommitReceiverMismatchView {
    inner: CorePostCommitReceiverMismatchProjection,
}

impl PostCommitReceiverMismatchView {
    pub fn expected(&self) -> PostCommitCleanupReceiverView {
        PostCommitCleanupReceiverView {
            inner: self.inner.expected().clone(),
        }
    }

    pub fn observed(&self) -> PostCommitCleanupObservedView {
        PostCommitCleanupObservedView {
            inner: self.inner.observed().clone(),
        }
    }
}

#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PostCommitAuthorityCause {
    HostRegistrationLost,
    HostEntityMissing,
    RequiredResourcesLost,
}

impl From<PostCommitAuthorityCause> for CorePostCommitAuthorityCause {
    fn from(cause: PostCommitAuthorityCause) -> Self {
        match cause {
            PostCommitAuthorityCause::HostRegistrationLost => Self::HostRegistrationLost,
            PostCommitAuthorityCause::HostEntityMissing => Self::HostEntityMissing,
            PostCommitAuthorityCause::RequiredResourcesLost => Self::RequiredResourcesLost,
        }
    }
}

#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PostCommitAuthorityCauseKind {
    HostRegistrationLost,
    HostEntityMissing,
    RequiredResourcesLost,
}

#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PostCommitAuthorityCauseView {
    inner: CorePostCommitAuthorityCause,
}

impl PostCommitAuthorityCauseView {
    fn from_core(inner: CorePostCommitAuthorityCause) -> Self {
        Self { inner }
    }

    pub fn from_diagnostic(diagnostic: &DiagnosticEvent) -> Option<Self> {
        diagnostic
            .post_commit_cleanup_anomaly_cause()
            .map(|cause| match cause {
                CorePostCommitCleanupAnomalyCause::HostRegistrationLost => {
                    CorePostCommitAuthorityCause::HostRegistrationLost
                }
                CorePostCommitCleanupAnomalyCause::HostEntityMissing => {
                    CorePostCommitAuthorityCause::HostEntityMissing
                }
                CorePostCommitCleanupAnomalyCause::RequiredResourcesLost => {
                    CorePostCommitAuthorityCause::RequiredResourcesLost
                }
            })
            .map(Self::from_core)
    }

    pub fn kind(self) -> PostCommitAuthorityCauseKind {
        match self.inner {
            CorePostCommitAuthorityCause::HostRegistrationLost => {
                PostCommitAuthorityCauseKind::HostRegistrationLost
            }
            CorePostCommitAuthorityCause::HostEntityMissing => {
                PostCommitAuthorityCauseKind::HostEntityMissing
            }
            CorePostCommitAuthorityCause::RequiredResourcesLost => {
                PostCommitAuthorityCauseKind::RequiredResourcesLost
            }
        }
    }
}

#[doc(hidden)]
pub struct PostCommitAuthorityTransferOwner {
    inner: CorePostCommitAuthorityTransferOwner,
    normal_manifest_digest: u64,
    normal_obligation_count: usize,
}

impl PostCommitAuthorityTransferOwner {
    pub fn receiver(&self) -> PostCommitCleanupReceiverView {
        PostCommitCleanupReceiverView {
            inner: self.inner.receiver().clone(),
        }
    }

    pub fn cause(&self) -> PostCommitAuthorityCauseView {
        PostCommitAuthorityCauseView::from_core(self.inner.cause())
    }
}

#[doc(hidden)]
pub enum PostCommitAuthorityAcceptanceOutcome {
    Accepted(PostCommitAuthorityTransferOwner),
    Mismatch {
        expected: PostCommitCleanupReceiverView,
        observed: PostCommitCleanupObservedView,
    },
}

#[doc(hidden)]
pub struct PostCommitAuthorityPreparationSource {
    inner: CorePostCommitAuthorityPreparationSource,
    normal_manifest_digest: u64,
    normal_obligation_count: usize,
}

impl PostCommitAuthorityPreparationSource {
    pub fn new(transfer: PostCommitAuthorityTransferOwner, host: HostLease) -> Self {
        Self {
            inner: CorePostCommitAuthorityPreparationSource::new(transfer.inner, host.inner),
            normal_manifest_digest: transfer.normal_manifest_digest,
            normal_obligation_count: transfer.normal_obligation_count,
        }
    }

    pub fn into_parts(self) -> (PostCommitAuthorityTransferOwner, HostLease) {
        let (transfer, host) = self.inner.into_parts();
        (
            PostCommitAuthorityTransferOwner {
                inner: transfer,
                normal_manifest_digest: self.normal_manifest_digest,
                normal_obligation_count: self.normal_obligation_count,
            },
            HostLease { inner: host },
        )
    }

    pub fn cause(&self) -> PostCommitAuthorityCauseView {
        PostCommitAuthorityCauseView::from_core(self.inner.cause())
    }
}

#[doc(hidden)]
pub struct PreparedPostCommitAuthorityTransfer {
    inner: CorePreparedPostCommitAuthorityTransfer,
    normal_manifest_digest: u64,
    normal_obligation_count: usize,
    authority_manifest_digest: u64,
    authority_obligation_count: usize,
}

impl PreparedPostCommitAuthorityTransfer {
    pub fn authority_batch(&self) -> PostCommitCleanupBatchView {
        PostCommitCleanupBatchView {
            inner: self.inner.batch_projection(),
            manifest_digest: self.authority_manifest_digest,
            obligation_count: self.authority_obligation_count,
        }
    }

    pub fn into_source(self) -> PostCommitAuthorityPreparationSource {
        PostCommitAuthorityPreparationSource {
            inner: self.inner.into_source(),
            normal_manifest_digest: self.normal_manifest_digest,
            normal_obligation_count: self.normal_obligation_count,
        }
    }

    pub fn cause(&self) -> PostCommitAuthorityCauseView {
        PostCommitAuthorityCauseView::from_core(self.inner.cause())
    }
}

#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PostCommitCapacityKind {
    PreparedTrace,
    PreparedTerminal,
}

#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PostCommitAuthorityContradiction {
    ForeignReceiver,
    CompetingSameSourceOwner,
    RetiredHostMismatch,
    HistoryOrActiveMismatch,
    IdentityExhausted,
    RequestOrQueueMismatch,
    TraceMismatch,
}

#[doc(hidden)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PostCommitAuthorityBeginRetryEvidence {
    inner: CorePostCommitAuthorityBeginRetryEvidence,
}

impl PostCommitAuthorityBeginRetryEvidence {
    pub fn missing_capacity(&self) -> PostCommitCapacityKind {
        match self.inner.missing_capacity() {
            crate::engine::CorePostCommitCapacityKind::PreparedTrace => {
                PostCommitCapacityKind::PreparedTrace
            }
            crate::engine::CorePostCommitCapacityKind::PreparedTerminal => {
                PostCommitCapacityKind::PreparedTerminal
            }
        }
    }

    pub fn cause(&self) -> PostCommitAuthorityCauseView {
        PostCommitAuthorityCauseView::from_core(self.inner.cause())
    }
}

#[doc(hidden)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PostCommitAuthorityBeginPermanentEvidence {
    inner: CorePostCommitAuthorityBeginPermanentEvidence,
}

impl PostCommitAuthorityBeginPermanentEvidence {
    pub fn contradiction(&self) -> PostCommitAuthorityContradiction {
        match self.inner.contradiction() {
            crate::engine::CorePostCommitAuthorityContradiction::ForeignReceiver => {
                PostCommitAuthorityContradiction::ForeignReceiver
            }
            crate::engine::CorePostCommitAuthorityContradiction::CompetingSameSourceOwner => {
                PostCommitAuthorityContradiction::CompetingSameSourceOwner
            }
            crate::engine::CorePostCommitAuthorityContradiction::RetiredHostMismatch => {
                PostCommitAuthorityContradiction::RetiredHostMismatch
            }
            crate::engine::CorePostCommitAuthorityContradiction::HistoryOrActiveMismatch => {
                PostCommitAuthorityContradiction::HistoryOrActiveMismatch
            }
            crate::engine::CorePostCommitAuthorityContradiction::IdentityExhausted => {
                PostCommitAuthorityContradiction::IdentityExhausted
            }
            crate::engine::CorePostCommitAuthorityContradiction::RequestOrQueueMismatch => {
                PostCommitAuthorityContradiction::RequestOrQueueMismatch
            }
            crate::engine::CorePostCommitAuthorityContradiction::TraceMismatch => {
                PostCommitAuthorityContradiction::TraceMismatch
            }
        }
    }

    pub fn cause(&self) -> PostCommitAuthorityCauseView {
        PostCommitAuthorityCauseView::from_core(self.inner.cause())
    }
}

#[doc(hidden)]
pub enum PostCommitAuthorityPreparationOutcome {
    Prepared {
        owner: PreparedPostCommitAuthorityTransfer,
        batch: PostCommitCleanupBatchView,
    },
    Mismatch {
        source: PostCommitAuthorityPreparationSource,
        evidence: PostCommitReceiverMismatchView,
    },
    Retryable {
        source: PostCommitAuthorityPreparationSource,
        evidence: PostCommitAuthorityBeginRetryEvidence,
    },
    Permanent {
        source: PostCommitAuthorityPreparationSource,
        evidence: PostCommitAuthorityBeginPermanentEvidence,
    },
}

#[doc(hidden)]
pub enum PostCommitAuthorityBeginOutcome<Unbound, Bound, BindError> {
    Begun,
    BindingMismatch {
        owner: PreparedPostCommitAuthorityTransfer,
        unbound: Unbound,
        evidence: BindError,
    },
    ReceiverMismatch {
        owner: PreparedPostCommitAuthorityTransfer,
        bound: Bound,
        evidence: PostCommitReceiverMismatchView,
    },
    Retryable {
        owner: PreparedPostCommitAuthorityTransfer,
        bound: Bound,
        evidence: PostCommitAuthorityBeginRetryEvidence,
    },
    Permanent {
        owner: PreparedPostCommitAuthorityTransfer,
        bound: Bound,
        evidence: PostCommitAuthorityBeginPermanentEvidence,
    },
}

#[doc(hidden)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PostCommitCompoundView {
    inner: CorePostCommitCompoundProjection,
}

impl PostCommitCompoundView {
    pub fn receiver(&self) -> PostCommitCleanupReceiverView {
        PostCommitCleanupReceiverView {
            inner: self.inner.receiver().clone(),
        }
    }

    pub fn cause(&self) -> PostCommitAuthorityCauseView {
        PostCommitAuthorityCauseView::from_core(self.inner.cause())
    }
}

#[doc(hidden)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetiredHostAuthorityExpectedView {
    inner: CoreRetiredHostAuthorityExpectedProjection,
}

impl RetiredHostAuthorityExpectedView {
    pub fn cause(&self) -> PostCommitAuthorityCauseView {
        PostCommitAuthorityCauseView::from_core(self.inner.cause())
    }
}

#[doc(hidden)]
pub struct PostCommitCompoundBatchOwner {
    inner: CorePostCommitCompoundBatchOwner,
    normal_manifest_digest: u64,
    normal_obligation_count: usize,
    authority_manifest_digest: u64,
    authority_obligation_count: usize,
}

impl PostCommitCompoundBatchOwner {
    pub fn projection(&self) -> PostCommitCompoundView {
        PostCommitCompoundView {
            inner: self.inner.projection(),
        }
    }

    pub fn normal_batch(&self) -> PostCommitCleanupBatchView {
        PostCommitCleanupBatchView {
            inner: self.inner.projection().receiver().normal_batch().clone(),
            manifest_digest: self.normal_manifest_digest,
            obligation_count: self.normal_obligation_count,
        }
    }

    pub fn authority_batch(&self) -> PostCommitCleanupBatchView {
        PostCommitCleanupBatchView {
            inner: self.inner.authority_batch_projection(),
            manifest_digest: self.authority_manifest_digest,
            obligation_count: self.authority_obligation_count,
        }
    }

    pub fn retired_host_expected(&self) -> RetiredHostAuthorityExpectedView {
        RetiredHostAuthorityExpectedView {
            inner: self.inner.host_expected_projection(),
        }
    }

    pub fn expected_host_physical_digest(&self) -> u64 {
        self.inner.expected_host_physical_digest()
    }

    pub fn cause(&self) -> PostCommitAuthorityCauseView {
        PostCommitAuthorityCauseView::from_core(self.inner.cause())
    }

    pub fn settlement(&self) -> PostCommitSettlementTraceView {
        PostCommitSettlementTraceView {
            inner: self.inner.settlement_projection(),
        }
    }
}

#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PostCommitSettlementTraceProgress {
    OriginalResidual,
    AuthorityNonTop,
    RetiredHostEdge,
    AuthorityTop,
    Complete,
}

#[doc(hidden)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PostCommitSettlementTraceView {
    inner: CorePostCommitSettlementTraceProjection,
}

impl PostCommitSettlementTraceView {
    pub fn cause(&self) -> PostCommitAuthorityCauseView {
        PostCommitAuthorityCauseView::from_core(self.inner.cause())
    }

    pub fn progress(&self) -> PostCommitSettlementTraceProgress {
        match self.inner.progress() {
            CorePostCommitSettlementTraceProgress::OriginalResidual => {
                PostCommitSettlementTraceProgress::OriginalResidual
            }
            CorePostCommitSettlementTraceProgress::AuthorityNonTop => {
                PostCommitSettlementTraceProgress::AuthorityNonTop
            }
            CorePostCommitSettlementTraceProgress::RetiredHostEdge => {
                PostCommitSettlementTraceProgress::RetiredHostEdge
            }
            CorePostCommitSettlementTraceProgress::AuthorityTop => {
                PostCommitSettlementTraceProgress::AuthorityTop
            }
            CorePostCommitSettlementTraceProgress::Complete => {
                PostCommitSettlementTraceProgress::Complete
            }
        }
    }

    pub fn digest(&self) -> u64 {
        self.inner.digest()
    }
}

#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PostCommitSettlementTraceMismatchKind {
    Receiver,
    WrongOwner,
    Replay,
    TraceUnavailable,
}

#[doc(hidden)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PostCommitSettlementTraceMismatchEvidence {
    inner: CorePostCommitSettlementTraceMismatchEvidence,
}

#[doc(hidden)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PostCommitCleanupSourceView {
    inner: CleanupBatchSourceProjection,
}

impl PostCommitSettlementTraceMismatchEvidence {
    pub fn kind(&self) -> PostCommitSettlementTraceMismatchKind {
        match self.inner.kind() {
            CorePostCommitSettlementTraceMismatchKind::Receiver => {
                PostCommitSettlementTraceMismatchKind::Receiver
            }
            CorePostCommitSettlementTraceMismatchKind::WrongOwner => {
                PostCommitSettlementTraceMismatchKind::WrongOwner
            }
            CorePostCommitSettlementTraceMismatchKind::Replay => {
                PostCommitSettlementTraceMismatchKind::Replay
            }
            CorePostCommitSettlementTraceMismatchKind::TraceUnavailable => {
                PostCommitSettlementTraceMismatchKind::TraceUnavailable
            }
        }
    }

    pub fn cause(&self) -> PostCommitAuthorityCauseView {
        PostCommitAuthorityCauseView::from_core(self.inner.cause())
    }

    pub fn submitted_normal(&self) -> PostCommitCleanupSourceView {
        PostCommitCleanupSourceView {
            inner: self.inner.submitted_normal().clone(),
        }
    }

    pub fn expected_next(&self) -> Option<PostCommitSettlementExpectedView> {
        self.inner
            .expected_next()
            .cloned()
            .map(|inner| PostCommitSettlementExpectedView { inner })
    }

    pub fn submitted_trace(&self) -> PostCommitSettlementTraceView {
        PostCommitSettlementTraceView {
            inner: self.inner.submitted_trace().clone(),
        }
    }

    pub fn submitted_next(&self) -> Option<PostCommitSettlementExpectedView> {
        self.inner
            .submitted_next()
            .cloned()
            .map(|inner| PostCommitSettlementExpectedView { inner })
    }
}

#[doc(hidden)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PostCommitSettlementKeyProjection {
    inner: CorePostCommitSettlementKeyProjection,
}

impl PostCommitSettlementKeyProjection {
    /// Trusted ECS integration SPI. The identity must come from an independently
    /// frozen physical obligation, never from a Core expected projection.
    pub fn from_trusted_ecs_freeze(namespace: u64, identity: Vec<u64>) -> Self {
        Self {
            inner: CorePostCommitSettlementKeyProjection::new(namespace, identity),
        }
    }

    pub fn namespace(&self) -> u64 {
        self.inner.namespace()
    }

    pub fn identity(&self) -> &[u64] {
        self.inner.identity()
    }
}

#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PostCommitSettlementRole {
    OriginalResidual,
    AuthorityNonTop,
    RetiredHostEdge,
    AuthorityTop,
}

impl PostCommitSettlementRole {
    fn from_core(role: CorePostCommitSettlementRole) -> Self {
        match role {
            CorePostCommitSettlementRole::OriginalResidual => Self::OriginalResidual,
            CorePostCommitSettlementRole::AuthorityNonTop => Self::AuthorityNonTop,
            CorePostCommitSettlementRole::RetiredHostEdge => Self::RetiredHostEdge,
            CorePostCommitSettlementRole::AuthorityTop => Self::AuthorityTop,
        }
    }
}

#[doc(hidden)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PostCommitPlannedCleanupProjection {
    inner: CorePostCommitPlannedCleanupProjection,
}

impl PostCommitPlannedCleanupProjection {
    /// Trusted ECS integration SPI.
    pub fn from_trusted_ecs_freeze(
        obligation: PostCommitSettlementKeyProjection,
        physical_source: PostCommitSettlementKeyProjection,
        disposition: CleanupDisposition,
    ) -> Self {
        Self {
            inner: CorePostCommitPlannedCleanupProjection::new(
                obligation.inner,
                physical_source.inner,
                disposition,
            ),
        }
    }
}

#[doc(hidden)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PostCommitSettledOriginalProjection {
    inner: CorePostCommitSettledOriginalProjection,
}

impl PostCommitSettledOriginalProjection {
    /// Trusted ECS integration SPI.
    pub fn from_trusted_ecs_freeze(
        obligation: PostCommitSettlementKeyProjection,
        receipt: PostCommitSettlementKeyProjection,
        disposition: CleanupDisposition,
    ) -> Self {
        Self {
            inner: CorePostCommitSettledOriginalProjection::new(
                obligation.inner,
                receipt.inner,
                disposition,
            ),
        }
    }
}

#[doc(hidden)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PostCommitPlannedHostEdgeProjection {
    inner: CorePostCommitPlannedHostEdgeProjection,
}

impl PostCommitPlannedHostEdgeProjection {
    /// Trusted ECS integration SPI.
    pub fn from_trusted_ecs_freeze(
        obligation: PostCommitSettlementKeyProjection,
        physical_source: PostCommitSettlementKeyProjection,
    ) -> Self {
        Self {
            inner: CorePostCommitPlannedHostEdgeProjection::new(
                obligation.inner,
                physical_source.inner,
            ),
        }
    }
}

#[doc(hidden)]
pub struct PostCommitSettlementPlanDescriptor {
    inner: CorePostCommitSettlementPlanDescriptor,
}

impl PostCommitSettlementPlanDescriptor {
    #[allow(clippy::too_many_arguments)]
    /// Trusted ECS integration SPI. Core still validates exact coverage,
    /// role separation, the prior L receipt suffix, and the H/top dependency.
    pub fn from_trusted_ecs_freeze(
        prior_trace_start: usize,
        settled_original: Vec<PostCommitSettledOriginalProjection>,
        residual_original: Vec<PostCommitPlannedCleanupProjection>,
        authority_non_top: Vec<PostCommitPlannedCleanupProjection>,
        authority_top: PostCommitPlannedCleanupProjection,
        authority_top_dependency: PostCommitSettlementKeyProjection,
        host_edge: PostCommitPlannedHostEdgeProjection,
    ) -> Self {
        Self {
            inner: CorePostCommitSettlementPlanDescriptor::new(
                prior_trace_start,
                settled_original
                    .into_iter()
                    .map(|entry| entry.inner)
                    .collect(),
                residual_original
                    .into_iter()
                    .map(|entry| entry.inner)
                    .collect(),
                authority_non_top
                    .into_iter()
                    .map(|entry| entry.inner)
                    .collect(),
                authority_top.inner,
                authority_top_dependency.inner,
                host_edge.inner,
            ),
        }
    }
}

#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PostCommitSettlementPlanMismatchKind {
    OriginalCoverage,
    AuthorityCoverage,
    DuplicateKey,
    EmptyKey,
    PriorTrace,
    WrongSource,
}

#[doc(hidden)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PostCommitSettlementPlanMismatchEvidence {
    inner: CorePostCommitSettlementPlanMismatchEvidence,
}

impl PostCommitSettlementPlanMismatchEvidence {
    pub fn kind(&self) -> PostCommitSettlementPlanMismatchKind {
        match self.inner.kind() {
            CorePostCommitSettlementPlanMismatchKind::OriginalCoverage => {
                PostCommitSettlementPlanMismatchKind::OriginalCoverage
            }
            CorePostCommitSettlementPlanMismatchKind::AuthorityCoverage => {
                PostCommitSettlementPlanMismatchKind::AuthorityCoverage
            }
            CorePostCommitSettlementPlanMismatchKind::DuplicateKey => {
                PostCommitSettlementPlanMismatchKind::DuplicateKey
            }
            CorePostCommitSettlementPlanMismatchKind::EmptyKey => {
                PostCommitSettlementPlanMismatchKind::EmptyKey
            }
            CorePostCommitSettlementPlanMismatchKind::PriorTrace => {
                PostCommitSettlementPlanMismatchKind::PriorTrace
            }
            CorePostCommitSettlementPlanMismatchKind::WrongSource => {
                PostCommitSettlementPlanMismatchKind::WrongSource
            }
        }
    }
}

#[doc(hidden)]
pub struct PostCommitSettlementPlanBuilder {
    inner: CorePostCommitSettlementPlanBuilder,
}

#[doc(hidden)]
pub struct PostCommitSettlementPlan {
    inner: CoreBoundPostCommitSettlementPlan,
}

impl PostCommitSettlementPlanBuilder {
    /// Length of the already-published request trace frozen by Core for
    /// binding an exact settled-L suffix. No settlement expectation is
    /// exposed by this projection.
    pub fn prior_trace_len(&self) -> usize {
        self.inner.current_trace_len()
    }

    /// Trusted ECS integration SPI. The descriptor must be built from the
    /// physical freeze retained by the sole ECS compound owner.
    pub fn bind_trusted_ecs(
        self,
        descriptor: PostCommitSettlementPlanDescriptor,
    ) -> Result<PostCommitSettlementPlan, PostCommitSettlementPlanMismatchEvidence> {
        self.inner
            .bind(descriptor.inner)
            .map(|inner| PostCommitSettlementPlan { inner })
            .map_err(|inner| PostCommitSettlementPlanMismatchEvidence { inner })
    }
}

/// Opaque linear Core settlement attempt. It contains the only permit for the
/// next frozen L/A/H plan entry and deliberately exposes no expected facts.
#[doc(hidden)]
pub struct TrustedPostCommitSettlementAttempt {
    inner: CorePreparedPostCommitSettlement,
    normal_manifest_digest: u64,
    normal_obligation_count: usize,
    authority_manifest_digest: u64,
    authority_obligation_count: usize,
}

/// Result of moving the compound owner into the next exact settlement permit.
#[doc(hidden)]
pub enum TrustedPostCommitSettlementPermitOutcome {
    Prepared(TrustedPostCommitSettlementAttempt),
    Mismatch {
        owner: PostCommitCompoundBatchOwner,
        evidence: PostCommitSettlementTraceMismatchEvidence,
    },
}

/// Result of the trusted ECS physical-attestation bridge. The bridge accepts
/// no submitted role, key, disposition, or expected projection; Core records
/// only the fact already frozen into its private permit.
#[doc(hidden)]
pub enum TrustedPostCommitSettlementRecordOutcome<Physical> {
    Recorded {
        owner: PostCommitCompoundBatchOwner,
        trace: PostCommitSettlementTraceView,
    },
    Mismatch {
        attempt: TrustedPostCommitSettlementAttempt,
        physical: Physical,
        evidence: PostCommitSettlementTraceMismatchEvidence,
    },
}

#[doc(hidden)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PostCommitSettlementExpectedView {
    inner: CorePostCommitSettlementExpectedProjection,
}

impl PostCommitSettlementExpectedView {
    pub fn role(&self) -> PostCommitSettlementRole {
        PostCommitSettlementRole::from_core(self.inner.role())
    }

    pub fn obligation(&self) -> PostCommitSettlementKeyProjection {
        PostCommitSettlementKeyProjection {
            inner: self.inner.obligation().clone(),
        }
    }

    pub fn physical_source(&self) -> PostCommitSettlementKeyProjection {
        PostCommitSettlementKeyProjection {
            inner: self.inner.physical_source().clone(),
        }
    }

    pub fn disposition(&self) -> Option<CleanupDisposition> {
        self.inner.disposition()
    }
}

#[doc(hidden)]
pub struct AuthorityCleanupCompletionFacts {
    inner: CoreAuthorityCleanupCompletionFacts,
}

impl AuthorityCleanupCompletionFacts {
    pub(crate) fn new(
        batch: PostCommitCleanupBatchView,
        completion_digest: u64,
        settled_count: usize,
    ) -> Self {
        Self {
            inner: CoreAuthorityCleanupCompletionFacts::new(
                batch.inner,
                CleanupCompletionDigest::from_opaque(completion_digest),
                settled_count,
            ),
        }
    }
}

#[doc(hidden)]
pub struct RetiredHostEdgeCompletionFacts {
    inner: CoreRetiredHostEdgeCompletionFacts,
}

impl RetiredHostEdgeCompletionFacts {
    pub(crate) fn new(expected: RetiredHostAuthorityExpectedView, physical_digest: u64) -> Self {
        Self {
            inner: CoreRetiredHostEdgeCompletionFacts::new(expected.inner, physical_digest),
        }
    }
}

#[doc(hidden)]
pub struct PostCommitCompoundCompletionInputs {
    inner: CorePostCommitCompoundCompletionInputs,
}

impl PostCommitCompoundCompletionInputs {
    pub(crate) fn new(
        normal: CleanupCompletion,
        normal_batch: PostCommitCleanupBatchView,
        authority_expected: AuthorityCleanupCompletionFacts,
        authority: AuthorityCleanupCompletionFacts,
        host_edge: RetiredHostEdgeCompletionFacts,
        settlement: PostCommitSettlementTraceView,
    ) -> Self {
        let normal_facts = CoreNormalCleanupCompletionFacts::new(
            normal_batch.inner,
            CleanupManifestDigest::from_opaque(normal.manifest_digest),
            CleanupCompletionDigest::from_opaque(normal.completion_digest),
            normal.settled_count,
        );
        Self {
            inner: CorePostCommitCompoundCompletionInputs::new(
                normal.inner,
                normal_facts,
                authority_expected.inner,
                authority.inner,
                host_edge.inner,
                settlement.inner,
            ),
        }
    }
}

#[doc(hidden)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PostCommitCompoundMismatchEvidence {
    inner: CorePostCommitCompoundMismatchEvidence,
}

impl PostCommitCompoundMismatchEvidence {
    pub fn is_completion_facts(&self) -> bool {
        self.inner.is_completion_facts()
    }

    pub fn is_source(&self) -> bool {
        self.inner.is_source()
    }

    pub fn is_settlement_trace(&self) -> bool {
        self.inner.is_settlement_trace()
    }

    pub fn cause(&self) -> Option<PostCommitAuthorityCauseView> {
        self.inner
            .cause()
            .map(PostCommitAuthorityCauseView::from_core)
    }
}

#[doc(hidden)]
pub struct PostCommitCompoundCompletionOwner {
    inner: CorePostCommitCompoundCompletionOwner,
}

#[doc(hidden)]
pub enum PostCommitCompoundCompletionPreparationOutcome {
    Prepared(PostCommitCompoundCompletionOwner),
    Mismatch {
        batch: PostCommitCompoundBatchOwner,
        inputs: PostCommitCompoundCompletionInputs,
        evidence: PostCommitCompoundMismatchEvidence,
    },
}

/// Trusted ECS completion bridge result. `Physical` is the whole private ECS
/// compound owner, so a Core mismatch returns it unchanged beside the exact
/// Core batch and opaque derived inputs.
#[doc(hidden)]
pub enum TrustedPostCommitCompoundCompletionPreparationOutcome<Physical> {
    Prepared {
        owner: PostCommitCompoundCompletionOwner,
        physical: Physical,
    },
    Mismatch {
        batch: PostCommitCompoundBatchOwner,
        physical: Physical,
        inputs: PostCommitCompoundCompletionInputs,
        evidence: PostCommitCompoundMismatchEvidence,
    },
}

#[doc(hidden)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PostCommitTerminalEvidence {
    inner: CorePostCommitTerminalProjection,
}

impl PostCommitTerminalEvidence {
    pub fn request(&self) -> NavigationRequestId {
        self.inner.request()
    }

    pub fn cause(&self) -> PostCommitAuthorityCauseView {
        PostCommitAuthorityCauseView::from_core(self.inner.cause())
    }

    pub fn trace_digest(&self) -> u64 {
        self.inner.trace_digest()
    }
}

#[doc(hidden)]
pub struct RetiredHostAuthorityTerminalEvidence {
    inner: CoreRetiredHostAuthorityTerminalEvidence,
}

impl RetiredHostAuthorityTerminalEvidence {
    pub fn cause(&self) -> PostCommitAuthorityCauseView {
        PostCommitAuthorityCauseView::from_core(self.inner.cause())
    }
}

#[doc(hidden)]
pub enum PostCommitCompoundCompletionOutcome<PhysicalTerminal> {
    Terminal {
        terminal: PostCommitTerminalEvidence,
        physical: PhysicalTerminal,
        retired_host: RetiredHostAuthorityTerminalEvidence,
    },
    Mismatch {
        owner: PostCommitCompoundCompletionOwner,
        evidence: PostCommitCompoundMismatchEvidence,
    },
}

/// Completion result for the trusted ECS compound. A receiver mismatch keeps
/// the complete private physical owner beside the linear Core owner.
#[doc(hidden)]
pub enum TrustedPostCommitCompoundCompletionOutcome<Physical, PhysicalTerminal> {
    Terminal {
        terminal: PostCommitTerminalEvidence,
        physical: PhysicalTerminal,
        retired_host: RetiredHostAuthorityTerminalEvidence,
    },
    Mismatch {
        owner: PostCommitCompoundCompletionOwner,
        physical: Physical,
        evidence: PostCommitCompoundMismatchEvidence,
    },
}

pub struct PreparedCommit {
    publication: PreparedPublication,
    cleanup: CleanupBatch,
}

impl PreparedCommit {
    pub fn cleanup(&self) -> &CleanupBatch {
        &self.cleanup
    }
}

pub struct ValidatedCommit<'a> {
    publication: CoreValidatedPublication<'a>,
    cleanup: CleanupBatch,
    back_target_continuation: Option<PreparedPostCommitContinuation>,
}

impl ValidatedCommit<'_> {
    pub fn cleanup(&self) -> &CleanupBatch {
        &self.cleanup
    }

    pub fn publish<F>(self, mutation: F)
    where
        F: FnOnce(),
    {
        let Self {
            publication,
            cleanup,
            back_target_continuation,
        } = self;
        drop(cleanup);
        publication.publish(mutation);
        drop(back_target_continuation);
    }
}

pub struct NavigationRequestView<'a> {
    record: &'a NavigationRequestRecord,
}

#[doc(hidden)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoryExecutionBindingView {
    inner: HistoryExecutionBindingProjection,
}

struct LifecycleContinuationBinding {
    scope: BackResolutionScopeProjection,
}

#[doc(hidden)]
pub struct LifecycleContinuationCapacity {
    inner: LifecycleContinuationBinding,
}

#[doc(hidden)]
#[derive(Clone)]
pub struct PreparationBoundaryScopeView {
    inner: BackResolutionScopeProjection,
}

#[doc(hidden)]
pub struct PreparationReceiverMismatchView {
    inner: CorePreparationReceiverMismatchProjection,
}

#[doc(hidden)]
pub enum BackTargetPreparationAcceptance<Accepted> {
    Matched(Accepted),
    Mismatch {
        capacity: LifecycleContinuationCapacity,
        mismatch: PreparationReceiverMismatchView,
    },
}

#[doc(hidden)]
pub enum BackTargetExecutionOwnerRef<'a> {
    Driving {
        history: &'a HistoryExecutionBindingView,
        continuation: &'a LifecycleContinuationCapacity,
    },
    Classifying {
        history: &'a HistoryExecutionBindingView,
        continuation: &'a LifecycleContinuationCapacity,
    },
    Prepared {
        history: &'a HistoryExecutionBindingView,
        continuation: &'a PreparedPostCommitContinuationOwner,
    },
}

#[doc(hidden)]
pub enum BackTargetPhysicalOwnerRef<'a> {
    ResumeAction(&'a BackTargetResumeAction),
    FrameTask(&'a RootFrameTask),
    MountRequest(&'a MountRequest),
    CommitTicket(&'a CommitTicket),
}

#[doc(hidden)]
pub enum BackTargetExecutionPairingOutcome {
    Matched,
    Mismatch(PreparationReceiverMismatchView),
}

#[doc(hidden)]
pub enum BackTargetDriverFailureReportMode {
    Normal,
    PhysicalResidual,
}

#[doc(hidden)]
pub struct BackTargetDriverFailureCleanupOwner {
    inner: CoreBackTargetDriverFailureCleanup,
}

#[doc(hidden)]
pub struct BackTargetDriverFallbackOwner {
    inner: CoreBackTargetDriverFallback,
}

#[doc(hidden)]
pub struct BackTargetDriverTerminalOwner {
    inner: CoreBackTargetDriverTerminal,
}

#[doc(hidden)]
pub struct BackTargetDriverTerminalEvidence {
    inner: CoreBackTargetDriverTerminalProjection,
}

#[doc(hidden)]
pub enum BackTargetDriverFailureCoreOutcome {
    FailureCleanup(BackTargetDriverFailureCleanupOwner),
    Fallback(BackTargetDriverFallbackOwner),
    Terminal(BackTargetDriverTerminalOwner),
}

#[doc(hidden)]
pub enum BackTargetDriverFailureReportOutcome {
    Reported(BackTargetDriverFailureCoreOutcome),
    Mismatch {
        capacity: LifecycleContinuationCapacity,
        task: RootFrameTask,
        mismatch: PreparationReceiverMismatchView,
    },
}

#[doc(hidden)]
pub enum BackTargetDriverFailureConsumeOutcome {
    FailureCleanup(FailureCleanupRequest),
    FallbackReleased,
    Terminal(BackTargetDriverTerminalEvidence),
    Mismatch {
        owner: BackTargetDriverFailureCoreOutcome,
        mismatch: PreparationReceiverMismatchView,
    },
}

#[doc(hidden)]
pub enum BackTargetAppliedReport {
    LegacyLeaf,
    WithOutlets(Vec<RouteSlotKey>),
}

#[doc(hidden)]
pub enum BackTargetAppliedReportOutcome {
    MountRequest {
        capacity: LifecycleContinuationCapacity,
        request: MountRequest,
    },
    Commit {
        capacity: LifecycleContinuationCapacity,
        ticket: CommitTicket,
    },
    FailureCleanup(BackTargetDriverFailureCleanupOwner),
    Mismatch {
        capacity: LifecycleContinuationCapacity,
        task: RootFrameTask,
        report: BackTargetAppliedReport,
        mismatch: PreparationReceiverMismatchView,
    },
}

struct PreparedPostCommitContinuation {
    binding: LifecycleContinuationBinding,
}

#[doc(hidden)]
pub struct PreparedPostCommitContinuationOwner {
    inner: PreparedPostCommitContinuation,
}

#[doc(hidden)]
pub struct BackTargetCommitRetainedInputOwner {
    continuation: PreparedPostCommitContinuationOwner,
    manifest: CleanupManifest,
    evidence: CoreBackTargetCommitRetainedInputProjection,
}

#[doc(hidden)]
pub struct BackTargetCommitFailureCleanupOwner {
    inner: CoreBackTargetCommitFailureCleanup,
}

#[doc(hidden)]
pub struct BackTargetCommitRejectedPublicationOwner {
    inner: CoreBackTargetCommitRejectedPublication,
}

#[doc(hidden)]
pub enum BackTargetCommitPreparationFailure {
    RetainedInput(BackTargetCommitRetainedInputOwner),
    FailureCleanup(BackTargetCommitFailureCleanupOwner),
    RejectedPublication(BackTargetCommitRejectedPublicationOwner),
}

#[doc(hidden)]
pub struct RouteFailureContinuationOwner {
    inner: PreparedPostCommitContinuation,
}

#[doc(hidden)]
pub enum BackTargetPendingRejectedConsumeOutcome {
    FailureCleanup(FailureCleanupRequest),
    Mismatch {
        owner: RouteFailureContinuationOwner,
        ticket: CommitTicket,
        mismatch: PreparationReceiverMismatchView,
    },
}

#[doc(hidden)]
pub struct TargetAuthoritySettlementOwner {
    inner: CoreBackTargetPermanentSettlementOwner,
}

#[doc(hidden)]
pub struct BackTargetPermanentBeginOwner {
    settlement: TargetAuthoritySettlementOwner,
    host: HostLease,
    manifest: CleanupManifest,
}

impl BackTargetPermanentBeginOwner {
    pub fn new(
        settlement: TargetAuthoritySettlementOwner,
        host: HostLease,
        manifest: CleanupManifest,
    ) -> Self {
        Self {
            settlement,
            host,
            manifest,
        }
    }

    pub fn into_parts(self) -> (TargetAuthoritySettlementOwner, HostLease, CleanupManifest) {
        (self.settlement, self.host, self.manifest)
    }
}

#[doc(hidden)]
pub struct BackTargetPermanentCleanupBatchOwner {
    inner: CoreBackTargetPermanentCleanupBatch,
}

#[doc(hidden)]
#[derive(Clone, Eq, PartialEq)]
pub struct BackTargetPermanentCleanupBatchView {
    identity: CleanupBatchToken,
    manifest_digest: u64,
    obligation_count: usize,
}

impl BackTargetPermanentCleanupBatchOwner {
    pub fn batch(&self) -> BackTargetPermanentCleanupBatchView {
        BackTargetPermanentCleanupBatchView {
            identity: self.inner.token().clone(),
            manifest_digest: self.inner.manifest_digest(),
            obligation_count: self.inner.obligation_count(),
        }
    }

    pub fn complete(
        self,
        manifest_digest: u64,
        obligation_count: usize,
        completion_digest: u64,
        settled_count: usize,
    ) -> BackTargetPermanentCleanupCompletionOwner {
        BackTargetPermanentCleanupCompletionOwner {
            inner: self.inner.completion_owner(
                manifest_digest,
                obligation_count,
                completion_digest,
                settled_count,
            ),
        }
    }
}

#[doc(hidden)]
pub struct BackTargetPermanentCleanupCompletionOwner {
    inner: CoreBackTargetPermanentCleanupCompletionOwner,
}

#[doc(hidden)]
pub enum BackTargetPermanentCleanupMismatchEvidence {
    Receiver(BackTargetPermanentCleanupReceiverMismatchView),
    Facts(BackTargetPermanentCompletionFactsMismatchView),
}

#[doc(hidden)]
pub struct BackTargetPermanentCleanupReceiverMismatchView {
    inner: CoreBackTargetPermanentCleanupReceiverMismatchProjection,
}

#[doc(hidden)]
pub struct BackTargetPermanentCompletionFactsMismatchView {
    inner: CoreBackTargetPermanentCompletionFactsMismatchProjection,
}

#[doc(hidden)]
pub struct BackTargetPermanentCleanupTerminalEvidence {
    inner: CoreBackTargetPermanentCleanupTerminalProjection,
}

#[doc(hidden)]
pub enum BackTargetPermanentCleanupCompletionOutcome {
    Terminal(BackTargetPermanentCleanupTerminalEvidence),
    Mismatch {
        owner: BackTargetPermanentCleanupCompletionOwner,
        mismatch: BackTargetPermanentCleanupMismatchEvidence,
    },
}

impl BackTargetPermanentCleanupBatchView {
    pub fn report_identity(&self) -> ViewReportIdentity {
        ViewReportIdentity {
            inner: self.identity.identity().clone(),
        }
    }

    pub fn manifest_digest(&self) -> u64 {
        self.manifest_digest
    }

    pub fn obligation_count(&self) -> usize {
        self.obligation_count
    }
}

#[doc(hidden)]
pub enum BackTargetPermanentBeginOutcome<Installation> {
    Begun,
    ReceiverMismatch {
        owner: BackTargetPermanentBeginOwner,
        installation: Installation,
        mismatch: PreparationReceiverMismatchView,
    },
    LogicalRootsMismatch {
        owner: BackTargetPermanentBeginOwner,
        installation: Installation,
    },
    Permanent {
        owner: BackTargetPermanentBeginOwner,
        installation: Installation,
        evidence: BackTargetPermanentBeginPermanentEvidence,
    },
}

#[doc(hidden)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BackTargetPermanentBeginPermanentEvidence {
    CurrentStateContradiction(BackTargetPermanentCurrentStateContradictionView),
    CanonicalRelationContradiction(BackTargetPermanentCanonicalRelationContradictionView),
    RequestRecordContradiction(BackTargetPermanentRequestRecordContradictionView),
    TerminalPlanContradiction(BackTargetPermanentTerminalPlanContradictionView),
    HostRetirementRejected,
}

#[doc(hidden)]
#[allow(dead_code)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackTargetPermanentCanonicalRelationContradictionView {
    expected: CoreBackTargetPermanentCanonicalRelationProjection,
    observed: CoreBackTargetPermanentCanonicalRelationProjection,
}

#[doc(hidden)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackTargetPermanentCurrentStateContradictionView {
    expected: BackTargetPermanentCurrentStateProjectionView,
    observed: BackTargetPermanentCurrentStateProjectionView,
}

impl BackTargetPermanentCurrentStateContradictionView {
    pub fn expected(&self) -> &BackTargetPermanentCurrentStateProjectionView {
        &self.expected
    }

    pub fn observed(&self) -> &BackTargetPermanentCurrentStateProjectionView {
        &self.observed
    }
}

#[doc(hidden)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackTargetPermanentCurrentStateProjectionView {
    inner: CoreBackTargetPermanentCurrentStateProjection,
}

impl BackTargetPermanentCurrentStateProjectionView {
    pub fn host_present(&self) -> bool {
        self.inner.host.is_some()
    }

    pub fn top_slot_present(&self) -> bool {
        self.inner.top_slot.is_some()
    }

    pub fn current_identity(&self) -> Option<ViewReportIdentity> {
        self.inner
            .current_identity
            .as_ref()
            .map(|inner| ViewReportIdentity {
                inner: inner.clone(),
            })
    }

    pub fn current_logical_roots(&self) -> Option<&[RouteFrameVersion]> {
        self.inner.current_logical_roots.as_deref()
    }

    pub fn current_reconciliation_present(&self) -> bool {
        self.inner.current_reconciliation.is_some()
    }

    pub fn current_reconciliation_matches(&self, other: &Self) -> bool {
        self.inner.current_reconciliation == other.inner.current_reconciliation
    }

    pub fn active_baseline_matches(&self, other: &Self) -> bool {
        self.inner.active_baseline == other.inner.active_baseline
    }

    pub fn current_retired_host_present(&self) -> bool {
        self.inner.current_retired_host.is_some()
    }

    pub fn availability(&self) -> &RouterAvailability {
        &self.inner.availability
    }

    pub fn request_status(&self) -> Option<&RequestStatus> {
        self.inner.request_status.as_ref()
    }

    pub fn history_binding_present(&self) -> bool {
        self.inner.history_binding.is_some()
    }

    pub fn pending_no_target_present(&self) -> bool {
        self.inner.pending_no_target.is_some()
    }

    pub fn pending_no_target_authority_present(&self) -> bool {
        self.inner.pending_no_target_authority.is_some()
    }

    pub fn pending_target_cleanup_present(&self) -> bool {
        self.inner.pending_target_cleanup.is_some()
    }

    pub fn history_preparation_error_present(&self) -> bool {
        self.inner.history_preparation_error
            != CoreBackTargetPermanentHistoryPreparationErrorProjection::Vacant
    }

    pub fn prepared_publication_present(&self) -> bool {
        self.inner.prepared_publication.is_some()
    }

    pub fn action_present(&self) -> bool {
        self.inner.action != crate::engine::CoreBackTargetReceiverActionProjection::Missing
    }
}

#[doc(hidden)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackTargetPermanentRequestRecordContradictionView {
    expected: Option<CoreBackTargetPermanentRequestProjection>,
    observed: Option<CoreBackTargetPermanentRequestProjection>,
}

impl BackTargetPermanentRequestRecordContradictionView {
    pub fn expected(&self) -> Option<BackTargetPermanentRequestProjectionView<'_>> {
        self.expected
            .as_ref()
            .map(|inner| BackTargetPermanentRequestProjectionView { inner })
    }

    pub fn observed(&self) -> Option<BackTargetPermanentRequestProjectionView<'_>> {
        self.observed
            .as_ref()
            .map(|inner| BackTargetPermanentRequestProjectionView { inner })
    }
}

#[doc(hidden)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackTargetPermanentTerminalPlanContradictionView {
    source: BackTargetPermanentTerminalSourceProjectionView,
    expected: BackTargetPermanentTerminalExpectedProjectionView,
    observed: BackTargetPermanentTerminalObservedProjectionView,
}

impl BackTargetPermanentTerminalPlanContradictionView {
    pub fn source(&self) -> &BackTargetPermanentTerminalSourceProjectionView {
        &self.source
    }

    pub fn expected(&self) -> &BackTargetPermanentTerminalExpectedProjectionView {
        &self.expected
    }

    pub fn observed(&self) -> &BackTargetPermanentTerminalObservedProjectionView {
        &self.observed
    }
}

#[doc(hidden)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackTargetPermanentTerminalSourceProjectionView {
    inner: CoreBackTargetPermanentTerminalSourceProjection,
}

impl BackTargetPermanentTerminalSourceProjectionView {
    pub fn current(&self) -> BackTargetPermanentCurrentStateProjectionView {
        BackTargetPermanentCurrentStateProjectionView {
            inner: self.inner.current.clone(),
        }
    }

    pub fn current_request(&self) -> BackTargetPermanentRequestProjectionView<'_> {
        BackTargetPermanentRequestProjectionView {
            inner: &self.inner.current_request,
        }
    }

    pub fn queue_len(&self) -> usize {
        self.inner.queue.len()
    }

    pub fn queued_request(
        &self,
        index: usize,
    ) -> Option<BackTargetPermanentQueuedRequestProjectionView<'_>> {
        self.inner
            .queue
            .get(index)
            .map(|inner| BackTargetPermanentQueuedRequestProjectionView { inner })
    }

    pub fn host(&self) -> Option<&HostVersion> {
        self.inner.authority.host.as_ref()
    }

    pub fn top_slot(&self) -> Option<RouteSlotId> {
        self.inner.authority.top_slot
    }

    pub fn host_present(&self) -> bool {
        self.inner.authority.host.is_some()
    }

    pub fn top_slot_present(&self) -> bool {
        self.inner.authority.top_slot.is_some()
    }

    pub fn availability(&self) -> &RouterAvailability {
        &self.inner.authority.availability
    }
}

#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BackTargetPermanentQueuedRequestProjectionView<'a> {
    inner: &'a crate::engine::CoreBackTargetPermanentQueuedRequestProjection,
}

impl<'a> BackTargetPermanentQueuedRequestProjectionView<'a> {
    pub fn expected(self) -> BackTargetPermanentRequestProjectionView<'a> {
        BackTargetPermanentRequestProjectionView {
            inner: &self.inner.expected,
        }
    }

    pub fn observed(self) -> Option<BackTargetPermanentRequestProjectionView<'a>> {
        self.inner
            .observed
            .as_ref()
            .map(|inner| BackTargetPermanentRequestProjectionView { inner })
    }
}

#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BackTargetPermanentTerminalOperation {
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

#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BackTargetPermanentTerminalFailureKind {
    MissingRequest,
    RequestRejected(BackTargetPermanentRequestFailureKind),
    AuthorityRecoveryRejected,
    AuthoritySettlementRejected,
    RetentionInvariant,
}

#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BackTargetPermanentRequestFailureKind {
    AlreadyTerminal,
    BackResolutionAlreadyInstalled,
    BackResolutionMismatch,
    ImmutableMatch,
    OutstandingBackResolution,
    TerminalStatusRequired,
    TerminalEventRequired,
    TraceClosed,
}

#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BackTargetPermanentRequestProjectionView<'a> {
    inner: &'a CoreBackTargetPermanentRequestProjection,
}

#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BackTargetPermanentAuthorityProjectionView<'a> {
    inner: &'a crate::engine::CoreBackTargetPermanentAuthorityProjection,
}

impl<'a> BackTargetPermanentAuthorityProjectionView<'a> {
    pub fn host(self) -> Option<&'a HostVersion> {
        self.inner.host.as_ref()
    }

    pub fn top_slot(self) -> Option<RouteSlotId> {
        self.inner.top_slot
    }

    pub fn availability(self) -> &'a RouterAvailability {
        &self.inner.availability
    }
}

#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BackTargetPermanentRetentionProjectionView<'a> {
    inner: &'a crate::engine::CoreBackTargetPermanentRetentionProjection,
}

impl<'a> BackTargetPermanentRetentionProjectionView<'a> {
    pub fn limit(self) -> usize {
        self.inner.limit
    }

    pub fn terminal_count(self) -> usize {
        self.inner.terminal_count
    }

    pub fn record_count(self) -> usize {
        self.inner.records.len()
    }

    pub fn record(&self, index: usize) -> Option<BackTargetPermanentRequestProjectionView<'_>> {
        self.inner
            .records
            .get(index)
            .map(|inner| BackTargetPermanentRequestProjectionView { inner })
    }
}

impl<'a> BackTargetPermanentRequestProjectionView<'a> {
    pub fn request(self) -> NavigationRequestId {
        self.inner.request
    }

    pub fn status(self) -> &'a RequestStatus {
        &self.inner.status
    }

    pub fn requested(self) -> Option<&'a RouteLocation> {
        self.inner.requested.as_ref()
    }

    pub fn trace_closed(self) -> bool {
        self.inner.trace_closed
    }
}

#[doc(hidden)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackTargetPermanentTerminalExpectedProjectionView {
    inner: CoreBackTargetPermanentTerminalExpectedProjection,
}

impl BackTargetPermanentTerminalExpectedProjectionView {
    pub fn operation(&self) -> BackTargetPermanentTerminalOperation {
        expected_terminal_operation(&self.inner)
    }

    pub fn request(&self) -> Option<BackTargetPermanentRequestProjectionView<'_>> {
        match &self.inner {
            CoreBackTargetPermanentTerminalExpectedProjection::Request { request, .. } => {
                Some(BackTargetPermanentRequestProjectionView { inner: request })
            }
            _ => None,
        }
    }

    pub fn authority(&self) -> Option<BackTargetPermanentAuthorityProjectionView<'_>> {
        match &self.inner {
            CoreBackTargetPermanentTerminalExpectedProjection::Authority { authority, .. } => {
                Some(BackTargetPermanentAuthorityProjectionView { inner: authority })
            }
            _ => None,
        }
    }

    pub fn retention(&self) -> Option<BackTargetPermanentRetentionProjectionView<'_>> {
        match &self.inner {
            CoreBackTargetPermanentTerminalExpectedProjection::Retention { retention, .. } => {
                Some(BackTargetPermanentRetentionProjectionView { inner: retention })
            }
            _ => None,
        }
    }
}

#[doc(hidden)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackTargetPermanentTerminalObservedProjectionView {
    inner: CoreBackTargetPermanentTerminalObservedProjection,
}

impl BackTargetPermanentTerminalObservedProjectionView {
    pub fn operation(&self) -> BackTargetPermanentTerminalOperation {
        observed_terminal_operation(&self.inner)
    }

    pub fn request(&self) -> Option<BackTargetPermanentRequestProjectionView<'_>> {
        match &self.inner {
            CoreBackTargetPermanentTerminalObservedProjection::Request { request, .. } => request
                .as_ref()
                .map(|inner| BackTargetPermanentRequestProjectionView { inner }),
            _ => None,
        }
    }

    pub fn authority(&self) -> Option<BackTargetPermanentAuthorityProjectionView<'_>> {
        match &self.inner {
            CoreBackTargetPermanentTerminalObservedProjection::Authority { authority, .. } => {
                Some(BackTargetPermanentAuthorityProjectionView { inner: authority })
            }
            _ => None,
        }
    }

    pub fn retention(&self) -> Option<BackTargetPermanentRetentionProjectionView<'_>> {
        match &self.inner {
            CoreBackTargetPermanentTerminalObservedProjection::Retention { retention, .. } => {
                Some(BackTargetPermanentRetentionProjectionView { inner: retention })
            }
            _ => None,
        }
    }

    pub fn failure(&self) -> BackTargetPermanentTerminalFailureKind {
        let failure = match &self.inner {
            CoreBackTargetPermanentTerminalObservedProjection::Request { failure, .. }
            | CoreBackTargetPermanentTerminalObservedProjection::Authority { failure, .. }
            | CoreBackTargetPermanentTerminalObservedProjection::Retention { failure, .. } => {
                *failure
            }
        };
        match failure {
            CoreBackTargetPermanentTerminalFailureProjection::MissingRequest => {
                BackTargetPermanentTerminalFailureKind::MissingRequest
            }
            CoreBackTargetPermanentTerminalFailureProjection::RequestRejected(error) => {
                let error = match error {
                    RequestRecordError::AlreadyTerminal => {
                        BackTargetPermanentRequestFailureKind::AlreadyTerminal
                    }
                    RequestRecordError::BackResolutionAlreadyInstalled => {
                        BackTargetPermanentRequestFailureKind::BackResolutionAlreadyInstalled
                    }
                    RequestRecordError::BackResolutionMismatch => {
                        BackTargetPermanentRequestFailureKind::BackResolutionMismatch
                    }
                    RequestRecordError::ImmutableMatch => {
                        BackTargetPermanentRequestFailureKind::ImmutableMatch
                    }
                    RequestRecordError::OutstandingBackResolution => {
                        BackTargetPermanentRequestFailureKind::OutstandingBackResolution
                    }
                    RequestRecordError::TerminalStatusRequired => {
                        BackTargetPermanentRequestFailureKind::TerminalStatusRequired
                    }
                    RequestRecordError::TerminalEventRequired => {
                        BackTargetPermanentRequestFailureKind::TerminalEventRequired
                    }
                    RequestRecordError::TraceClosed => {
                        BackTargetPermanentRequestFailureKind::TraceClosed
                    }
                };
                BackTargetPermanentTerminalFailureKind::RequestRejected(error)
            }
            CoreBackTargetPermanentTerminalFailureProjection::AuthorityRecoveryRejected => {
                BackTargetPermanentTerminalFailureKind::AuthorityRecoveryRejected
            }
            CoreBackTargetPermanentTerminalFailureProjection::AuthoritySettlementRejected => {
                BackTargetPermanentTerminalFailureKind::AuthoritySettlementRejected
            }
            CoreBackTargetPermanentTerminalFailureProjection::RetentionInvariant => {
                BackTargetPermanentTerminalFailureKind::RetentionInvariant
            }
        }
    }
}

fn map_terminal_operation(
    operation: CoreBackTargetPermanentTerminalOperation,
) -> BackTargetPermanentTerminalOperation {
    use BackTargetPermanentTerminalOperation as Public;
    use CoreBackTargetPermanentTerminalOperation as Core;
    match operation {
        Core::RecoveringTransition => Public::RecoveringTransition,
        Core::RecoveringTrace => Public::RecoveringTrace,
        Core::QueuedTerminalTransition => Public::QueuedTerminalTransition,
        Core::QueuedTerminalTrace => Public::QueuedTerminalTrace,
        Core::AuthorityRecovery => Public::AuthorityRecovery,
        Core::RecoveringRetention => Public::RecoveringRetention,
        Core::CleanupSettledTrace => Public::CleanupSettledTrace,
        Core::TerminalTransition => Public::TerminalTransition,
        Core::RequestTerminalTrace => Public::RequestTerminalTrace,
        Core::AuthoritySettlement => Public::AuthoritySettlement,
        Core::TerminalRetention => Public::TerminalRetention,
    }
}

fn expected_terminal_operation(
    projection: &CoreBackTargetPermanentTerminalExpectedProjection,
) -> BackTargetPermanentTerminalOperation {
    let operation = match projection {
        CoreBackTargetPermanentTerminalExpectedProjection::Request { operation, .. }
        | CoreBackTargetPermanentTerminalExpectedProjection::Authority { operation, .. }
        | CoreBackTargetPermanentTerminalExpectedProjection::Retention { operation, .. } => {
            *operation
        }
    };
    map_terminal_operation(operation)
}

fn observed_terminal_operation(
    projection: &CoreBackTargetPermanentTerminalObservedProjection,
) -> BackTargetPermanentTerminalOperation {
    let operation = match projection {
        CoreBackTargetPermanentTerminalObservedProjection::Request { operation, .. }
        | CoreBackTargetPermanentTerminalObservedProjection::Authority { operation, .. }
        | CoreBackTargetPermanentTerminalObservedProjection::Retention { operation, .. } => {
            *operation
        }
    };
    map_terminal_operation(operation)
}

#[doc(hidden)]
#[cfg(any(test, feature = "test-support"))]
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct BackTargetPermanentBeginStateView {
    inner: crate::engine::BackTargetPermanentBeginTestingProjection,
}

#[cfg(any(test, feature = "test-support"))]
impl BackTargetPermanentBeginStateView {
    pub fn cleanup_owner_count(self) -> usize {
        self.inner.cleanup_owner_count
    }

    pub fn current_owner_count(self) -> usize {
        self.inner.current_owner_count
    }

    pub fn action_owner_count(self) -> usize {
        self.inner.action_owner_count
    }

    pub fn current_history_slot_count(self) -> usize {
        self.inner.current_history_slot_count
    }

    pub fn cleanup_history_owner_count(self) -> usize {
        self.inner.cleanup_history_owner_count
    }

    pub fn cleanup_settlement_owner_count(self) -> usize {
        self.inner.cleanup_settlement_owner_count
    }

    pub fn queue_len(self) -> usize {
        self.inner.queue_len
    }

    pub fn conflicting_owner_count(self) -> usize {
        self.inner.conflicting_owner_count
    }
}

#[doc(hidden)]
pub enum BackTargetCommitPreparationOutcome<'a> {
    Validated(ValidatedCommit<'a>),
    Mismatch {
        owner: PreparedPostCommitContinuationOwner,
        manifest: CleanupManifest,
        mismatch: PreparationReceiverMismatchView,
    },
    PreparationFailed(BackTargetCommitPreparationFailure),
}

impl LifecycleContinuationCapacity {
    pub fn preparation_boundary_scope(&self) -> PreparationBoundaryScopeView {
        PreparationBoundaryScopeView {
            inner: self.inner.scope.clone(),
        }
    }
}

#[doc(hidden)]
pub enum BackTargetResumeAction {
    FrameTask(RootFrameTask),
    MountRequest(MountRequest),
    Commit(CommitTicket),
}

#[doc(hidden)]
pub struct BackTargetResume {
    action: BackTargetResumeAction,
    execution: HistoryExecutionBindingView,
    continuation: LifecycleContinuationCapacity,
}

impl BackTargetResume {
    pub fn parts(
        &self,
    ) -> (
        &BackTargetResumeAction,
        &HistoryExecutionBindingView,
        &LifecycleContinuationCapacity,
    ) {
        (&self.action, &self.execution, &self.continuation)
    }

    pub fn into_parts(
        self,
    ) -> (
        BackTargetResumeAction,
        HistoryExecutionBindingView,
        LifecycleContinuationCapacity,
    ) {
        (self.action, self.execution, self.continuation)
    }

    pub fn into_frame_task_parts(
        self,
    ) -> Result<
        (
            RootFrameTask,
            HistoryExecutionBindingView,
            LifecycleContinuationCapacity,
        ),
        Self,
    > {
        let Self {
            action,
            execution,
            continuation,
        } = self;
        match action {
            BackTargetResumeAction::FrameTask(task) => Ok((task, execution, continuation)),
            action => Err(Self {
                action,
                execution,
                continuation,
            }),
        }
    }

    pub fn into_commit_parts(
        self,
    ) -> Result<
        (
            CommitTicket,
            HistoryExecutionBindingView,
            LifecycleContinuationCapacity,
        ),
        Self,
    > {
        let Self {
            action,
            execution,
            continuation,
        } = self;
        match action {
            BackTargetResumeAction::Commit(ticket) => Ok((ticket, execution, continuation)),
            action => Err(Self {
                action,
                execution,
                continuation,
            }),
        }
    }
}

#[doc(hidden)]
pub enum BackTargetStart {
    Ready(BackTargetResume),
    FailureCleanup(FailureCleanupRequest),
}

#[doc(hidden)]
pub enum BackTargetCoreResumeOutcome {
    Resumed(BackTargetStart),
    Permanent(BackTargetPermanentOwner),
    Mismatch {
        owner: VerifiedBackTargetExecutionOwner,
        evidence: BackResolutionEvidence,
    },
}

#[doc(hidden)]
pub enum BackNoTargetSourceRefreezeOutcome {
    Unchanged(VerifiedBackNoTargetCandidate),
    Drift(BackNoTargetSourceDriftOwner),
    Mismatch {
        candidate: VerifiedBackNoTargetCandidate,
        evidence: BackResolutionEvidence,
    },
}

#[doc(hidden)]
pub struct ProvenEmptyApprovalView {
    inner: CoreProvenEmptyApprovalProjection,
}

#[doc(hidden)]
pub struct ApprovedBackNoTargetCandidate {
    candidate: VerifiedBackNoTargetCandidate,
    approval: ProvenEmptyApprovalView,
}

#[doc(hidden)]
pub enum BackNoTargetApprovalOutcome {
    Approved(ApprovedBackNoTargetCandidate),
    Mismatch {
        candidate: VerifiedBackNoTargetCandidate,
        mismatch: PreparationReceiverMismatchView,
    },
}

#[doc(hidden)]
pub struct BackNoTargetAuthoritySettlementOwner {
    inner: CoreBackNoTargetAuthoritySettlementOwner,
}

#[doc(hidden)]
pub enum BackNoTargetAuthorityCoreOutcome {
    Accepted(BackNoTargetAuthoritySettlementOwner),
    Mismatch {
        candidate: VerifiedBackNoTargetCandidate,
        mismatch: PreparationReceiverMismatchView,
    },
}

#[doc(hidden)]
pub struct BackNoTargetAuthorityManifest {
    cleanup: CleanupManifest,
}

impl BackNoTargetAuthorityManifest {
    pub fn new(cleanup: CleanupManifest) -> Self {
        Self { cleanup }
    }
}

#[doc(hidden)]
pub struct BackNoTargetAuthorityBeginOwner {
    settlement: BackNoTargetAuthoritySettlementOwner,
    host: HostLease,
    manifest: BackNoTargetAuthorityManifest,
}

impl BackNoTargetAuthorityBeginOwner {
    pub fn new(
        settlement: BackNoTargetAuthoritySettlementOwner,
        host: HostLease,
        manifest: BackNoTargetAuthorityManifest,
    ) -> Self {
        Self {
            settlement,
            host,
            manifest,
        }
    }

    pub fn into_parts(
        self,
    ) -> (
        BackNoTargetAuthoritySettlementOwner,
        HostLease,
        BackNoTargetAuthorityManifest,
    ) {
        (self.settlement, self.host, self.manifest)
    }
}

#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BackNoTargetAuthorityBeginRetryEvidence {
    Capacity,
}

#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BackNoTargetAuthorityBeginPermanentEvidence {
    HostManifestContradiction,
    PendingStateContradiction,
    HostRetirementRejected,
}

#[doc(hidden)]
pub struct BackNoTargetAuthorityCleanupBatchOwner {
    inner: CoreBackNoTargetAuthorityCleanupBatch,
}

#[doc(hidden)]
pub struct BackNoTargetAuthorityCleanupBatchView {
    inner: crate::engine::CoreBackNoTargetAuthorityCleanupProjection,
}

impl BackNoTargetAuthorityCleanupBatchView {
    pub fn manifest_digest(&self) -> u64 {
        self.inner.manifest_digest()
    }

    pub fn obligation_count(&self) -> usize {
        self.inner.obligation_count()
    }
}

impl BackNoTargetAuthorityCleanupBatchOwner {
    pub fn batch(&self) -> BackNoTargetAuthorityCleanupBatchView {
        BackNoTargetAuthorityCleanupBatchView {
            inner: self.inner.projection(),
        }
    }

    pub fn complete(
        self,
        manifest_digest: u64,
        obligation_count: usize,
        completion_digest: u64,
        settled_count: usize,
    ) -> BackNoTargetAuthorityCleanupCompletionOwner {
        BackNoTargetAuthorityCleanupCompletionOwner {
            inner: CoreBackNoTargetAuthorityCleanupCompletionOwner::new(
                self.inner,
                manifest_digest,
                obligation_count,
                completion_digest,
                settled_count,
            ),
        }
    }
}

#[doc(hidden)]
pub struct BackNoTargetAuthorityCleanupCompletionOwner {
    inner: CoreBackNoTargetAuthorityCleanupCompletionOwner,
}

#[doc(hidden)]
pub enum BackNoTargetAuthorityCleanupMismatchEvidence {
    Receiver(PreparationReceiverMismatchView),
    Facts(CoreBackNoTargetAuthorityCompletionFactsMismatchProjection),
}

#[doc(hidden)]
pub struct CoreBackNoTargetAuthorityCompletionFactsMismatchProjection {
    inner: InnerBackNoTargetAuthorityCompletionFactsMismatchProjection,
}

#[doc(hidden)]
pub struct BackNoTargetAuthorityTerminalEvidence {
    inner: CoreBackNoTargetAuthorityTerminalProjection,
}

#[doc(hidden)]
pub enum BackNoTargetAuthorityBeginOutcome {
    Begun,
    ReceiverMismatch {
        owner: BackNoTargetAuthorityBeginOwner,
        mismatch: PreparationReceiverMismatchView,
    },
    Retryable {
        owner: BackNoTargetAuthorityBeginOwner,
        evidence: BackNoTargetAuthorityBeginRetryEvidence,
    },
    Permanent {
        owner: BackNoTargetAuthorityBeginOwner,
        evidence: BackNoTargetAuthorityBeginPermanentEvidence,
    },
}

#[doc(hidden)]
pub enum BackNoTargetAuthorityCleanupCompletionOutcome {
    Terminal(BackNoTargetAuthorityTerminalEvidence),
    Mismatch {
        owner: BackNoTargetAuthorityCleanupCompletionOwner,
        evidence: BackNoTargetAuthorityCleanupMismatchEvidence,
    },
}

#[doc(hidden)]
pub struct BackNoTargetCleanupBatchOwner {
    inner: CoreBackNoTargetCleanupBatchOwner,
}

#[doc(hidden)]
pub struct RejectedBackContinuationOwner {
    inner: CoreRejectedBackContinuationOwner,
}

#[doc(hidden)]
pub struct BackNoTargetCleanupAction {
    inner: CoreBackNoTargetCleanupAction,
}

#[doc(hidden)]
pub struct BackNoTargetCleanupBatchView {
    inner: CoreBackNoTargetCleanupBatchProjection,
}

impl BackNoTargetCleanupBatchView {
    pub(crate) fn matches_resolution_evidence(&self, evidence: &BackResolutionEvidence) -> bool {
        self.inner.matches_resolution_evidence(evidence)
    }

    pub fn manifest_digest(&self) -> u64 {
        self.inner.manifest_digest()
    }

    pub fn obligation_count(&self) -> usize {
        self.inner.obligation_count()
    }

    pub fn expected_completion_digest(&self) -> u64 {
        self.inner.expected_completion_digest()
    }
}

#[doc(hidden)]
pub struct BackNoTargetCleanupInstallation {
    batch: BackNoTargetCleanupBatchOwner,
    continuation: RejectedBackContinuationOwner,
    action: BackNoTargetCleanupAction,
}

impl BackNoTargetCleanupInstallation {
    pub fn batch(&self) -> BackNoTargetCleanupBatchView {
        BackNoTargetCleanupBatchView {
            inner: self.batch.inner.projection(),
        }
    }

    pub fn complete(
        self,
        manifest_digest: u64,
        obligation_count: usize,
        completion_digest: u64,
        settled_count: usize,
    ) -> BackNoTargetCleanupCompletionOwner {
        let installation = crate::engine::CoreBackNoTargetCleanupInstallation::from_parts(
            self.batch.inner,
            self.continuation.inner,
            self.action.inner,
        );
        BackNoTargetCleanupCompletionOwner {
            inner: CoreBackNoTargetCleanupCompletionOwner::new(
                installation,
                manifest_digest,
                obligation_count,
                completion_digest,
                settled_count,
            ),
        }
    }
}

#[doc(hidden)]
pub struct BackNoTargetCleanupCompletionOwner {
    inner: CoreBackNoTargetCleanupCompletionOwner,
}

#[doc(hidden)]
pub struct BackNoTargetTerminalEvidence {
    inner: CoreBackNoTargetTerminalProjection,
}

#[doc(hidden)]
pub enum BackNoTargetCleanupCompletionOutcome {
    Terminal(BackNoTargetTerminalEvidence),
    Mismatch {
        owner: BackNoTargetCleanupCompletionOwner,
        mismatch: PreparationReceiverMismatchView,
    },
}

#[doc(hidden)]
pub struct PreparedBackNoTargetPublication {
    inner: crate::engine::CorePreparedBackNoTargetPublication,
}

impl PreparedBackNoTargetPublication {
    pub(crate) fn batch(&self) -> BackNoTargetCleanupBatchView {
        BackNoTargetCleanupBatchView {
            inner: self.inner.batch_projection(),
        }
    }
}

#[doc(hidden)]
pub enum BackNoTargetReadyCoreOutcome {
    Prepared(PreparedBackNoTargetPublication),
    Mismatch {
        approved: ApprovedBackNoTargetCandidate,
        mismatch: PreparationReceiverMismatchView,
    },
}

#[doc(hidden)]
pub enum BackNoTargetSourceDriftSettlementOutcome {
    Settled(BackNoTargetSourceDriftTerminalEvidence),
    Mismatch {
        owner: BackNoTargetSourceDriftOwner,
        evidence: BackNoTargetSourceDriftEvidence,
    },
}

#[doc(hidden)]
pub enum BackContradictionCoreOutcome {
    Failed(BackContradictionTerminalEvidence),
    Mismatch {
        owner: VerifiedBackResolutionContradiction,
        evidence: BackResolutionEvidence,
    },
}

#[doc(hidden)]
pub enum BackTargetPermanentSettlementOutcome {
    Settled(BackTargetPermanentTerminalEvidence),
    Mismatch {
        owner: BackTargetPermanentOwner,
        evidence: BackResolutionEvidence,
    },
}

#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReconciliationChildSlotRoleKind {
    PreservedActive,
    RetainEmpty,
    ActivateExisting,
    MountNew,
    Leaving,
}

#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReconciliationEnteringSlotKind {
    Existing(RouteSlotId),
    MountNew,
}

#[doc(hidden)]
#[derive(Clone, Copy, Debug)]
pub struct ReconciliationExistingFrameView<'a> {
    inner: &'a ProjectedExistingFrame,
}

impl ReconciliationExistingFrameView<'_> {
    pub fn branch_index(&self) -> usize {
        self.inner.branch_index()
    }

    pub fn version(&self) -> &RouteFrameVersion {
        self.inner.version()
    }
}

#[doc(hidden)]
#[derive(Clone, Copy, Debug)]
pub struct ReconciliationEnteringFrameView<'a> {
    inner: &'a ProjectedEnteringFrame,
}

impl ReconciliationEnteringFrameView<'_> {
    pub fn branch_index(&self) -> usize {
        self.inner.branch_index()
    }

    pub fn route(&self) -> crate::ids::RouteId {
        self.inner.route()
    }

    pub fn origin(&self) -> &crate::frame::FrameOrigin {
        self.inner.origin()
    }

    pub fn frame(&self) -> crate::ids::RouteFrameId {
        self.inner.frame()
    }

    pub fn generation(&self) -> crate::ids::RouteGeneration {
        self.inner.generation()
    }

    pub fn slot(&self) -> ReconciliationEnteringSlotKind {
        match self.inner.slot() {
            ProjectedEnteringSlot::Existing(slot) => ReconciliationEnteringSlotKind::Existing(slot),
            ProjectedEnteringSlot::MountNew => ReconciliationEnteringSlotKind::MountNew,
        }
    }
}

#[doc(hidden)]
#[derive(Clone, Copy, Debug)]
pub struct ReconciliationChildSlotBindingView<'a> {
    inner: &'a LogicalChildSlotBinding,
}

impl ReconciliationChildSlotBindingView<'_> {
    pub fn parent_branch_index(&self) -> usize {
        self.inner.parent_branch_index()
    }

    pub fn parent(&self) -> &RouteFrameVersion {
        self.inner.parent()
    }

    pub fn outlet(&self) -> &RouteSlotKey {
        self.inner.outlet()
    }

    pub fn slot(&self) -> RouteSlotId {
        self.inner.slot()
    }

    pub fn child(&self) -> Option<&RouteFrameVersion> {
        self.inner.child()
    }
}

#[doc(hidden)]
#[derive(Clone, Copy, Debug)]
pub struct ReconciliationChildSlotRoleView<'a> {
    inner: &'a ProjectedChildSlotRole,
}

impl ReconciliationChildSlotRoleView<'_> {
    pub fn parent_branch_index(&self) -> usize {
        self.inner.parent_branch_index()
    }

    pub fn outlet(&self) -> &RouteSlotKey {
        self.inner.outlet()
    }

    pub fn kind(&self) -> ReconciliationChildSlotRoleKind {
        match self.inner.kind() {
            ProjectedChildSlotRoleKind::PreservedActive => {
                ReconciliationChildSlotRoleKind::PreservedActive
            }
            ProjectedChildSlotRoleKind::RetainEmpty => ReconciliationChildSlotRoleKind::RetainEmpty,
            ProjectedChildSlotRoleKind::ActivateExisting => {
                ReconciliationChildSlotRoleKind::ActivateExisting
            }
            ProjectedChildSlotRoleKind::MountNew => ReconciliationChildSlotRoleKind::MountNew,
            ProjectedChildSlotRoleKind::Leaving => ReconciliationChildSlotRoleKind::Leaving,
        }
    }

    pub fn binding(&self) -> Option<ReconciliationChildSlotBindingView<'_>> {
        self.inner
            .binding()
            .map(|inner| ReconciliationChildSlotBindingView { inner })
    }
}

#[doc(hidden)]
#[derive(Clone, Copy, Debug)]
pub struct ReconciliationProjectionView<'a> {
    inner: &'a RouteReconciliationProjection,
}

impl<'a> ReconciliationProjectionView<'a> {
    pub fn preserved(
        &self,
    ) -> impl ExactSizeIterator<Item = ReconciliationExistingFrameView<'a>> + 'a {
        self.inner
            .preserved()
            .iter()
            .map(|inner| ReconciliationExistingFrameView { inner })
    }

    pub fn leaving(
        &self,
    ) -> impl ExactSizeIterator<Item = ReconciliationExistingFrameView<'a>> + 'a {
        self.inner
            .leaving()
            .iter()
            .map(|inner| ReconciliationExistingFrameView { inner })
    }

    pub fn entering(
        &self,
    ) -> impl ExactSizeIterator<Item = ReconciliationEnteringFrameView<'a>> + 'a {
        self.inner
            .entering()
            .iter()
            .map(|inner| ReconciliationEnteringFrameView { inner })
    }

    pub fn child_slots(
        &self,
    ) -> impl ExactSizeIterator<Item = ReconciliationChildSlotRoleView<'a>> + 'a {
        self.inner
            .child_slots()
            .iter()
            .map(|inner| ReconciliationChildSlotRoleView { inner })
    }

    pub fn target_fingerprint(&self) -> &crate::fingerprint::RouteBranchFingerprint {
        self.inner.target_fingerprint()
    }
}

pub enum RequestLookup<'a> {
    Found(NavigationRequestView<'a>),
    NotRetained,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MissingRequestClassification {
    NotRetained,
    Unknown,
}

#[derive(Clone, Eq, PartialEq)]
pub struct RequestRetentionClassifier {
    router: RouterIdentity,
    evicted_through: Option<u64>,
    highest_issued: Option<u64>,
}

impl std::fmt::Debug for RequestRetentionClassifier {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("RequestRetentionClassifier { .. }")
    }
}

impl RequestRetentionClassifier {
    pub(crate) const fn project(
        router: RouterIdentity,
        evicted_through: Option<u64>,
        highest_issued: Option<u64>,
    ) -> Self {
        Self {
            router,
            evicted_through,
            highest_issued,
        }
    }

    pub fn classify(&self, id: NavigationRequestId) -> MissingRequestClassification {
        if id.router_identity() != self.router
            || !self
                .highest_issued
                .is_some_and(|highest| id.sequence() <= highest)
        {
            return MissingRequestClassification::Unknown;
        }
        if self
            .evicted_through
            .is_some_and(|sequence| id.sequence() <= sequence)
        {
            MissingRequestClassification::NotRetained
        } else {
            MissingRequestClassification::Unknown
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouterSnapshotProjection {
    requested: Option<RouteLocation>,
    matched: Option<RouteBranchFingerprint>,
    active: ActivePresentation,
    availability: RouterAvailability,
    pending_phase: Option<NavigationPhase>,
    history: HistorySnapshot,
    running_request: Option<NavigationRequestId>,
    earliest_retained_sequence: Option<u64>,
    request_retention: RequestRetentionClassifier,
    diagnostics: DiagnosticSnapshot,
    host: HostSnapshot,
}

impl RouterSnapshotProjection {
    pub fn requested(&self) -> Option<&RouteLocation> {
        self.requested.as_ref()
    }

    pub fn matched(&self) -> Option<&RouteBranchFingerprint> {
        self.matched.as_ref()
    }

    pub fn active(&self) -> &ActivePresentation {
        &self.active
    }

    pub fn availability(&self) -> &RouterAvailability {
        &self.availability
    }

    pub fn pending_phase(&self) -> Option<&NavigationPhase> {
        self.pending_phase.as_ref()
    }

    pub fn history(&self) -> &HistorySnapshot {
        &self.history
    }

    pub fn running_request(&self) -> Option<NavigationRequestId> {
        self.running_request
    }

    pub fn earliest_retained_sequence(&self) -> Option<u64> {
        self.earliest_retained_sequence
    }

    pub fn request_retention(&self) -> &RequestRetentionClassifier {
        &self.request_retention
    }

    pub fn diagnostics(&self) -> &DiagnosticSnapshot {
        &self.diagnostics
    }

    pub fn host(&self) -> &HostSnapshot {
        &self.host
    }
}

pub fn project_cleanup_summary(
    foreground_pending: bool,
    quarantine_count: usize,
    last_disposition: Option<CleanupDisposition>,
) -> CleanupSummary {
    CleanupSummary::project(foreground_pending, quarantine_count, last_disposition)
}

impl NavigationRequestView<'_> {
    pub fn id(&self) -> NavigationRequestId {
        self.record.id()
    }

    pub fn requested(&self) -> Option<&RouteLocation> {
        self.record.requested()
    }

    pub fn matched(&self) -> Option<&RouteBranchFingerprint> {
        self.record.matched()
    }

    pub fn status(&self) -> &RequestStatus {
        self.record.status()
    }

    pub fn trace(&self) -> &[NavigationTraceEvent] {
        self.record.trace()
    }

    pub fn diagnostics(&self) -> &[DiagnosticEvent] {
        self.record.diagnostics()
    }
}

#[cfg(feature = "test-support")]
#[doc(hidden)]
pub struct BackCapacityCoreProducerSlot<'engine> {
    slot: &'engine mut Option<crate::test_support::BackCapacityCoreProducer>,
}

#[cfg(feature = "test-support")]
impl BackCapacityCoreProducerSlot<'_> {
    #[doc(hidden)]
    pub fn install(self, producer: crate::test_support::BackCapacityCoreProducer) {
        *self.slot = Some(producer);
    }
}

pub struct RouterEngine {
    inner: CoreRouterEngine,
    admission: HostAdmissionIssuer,
    host_trace: Vec<HostTraceEvent>,
    #[cfg(any(test, feature = "test-support"))]
    testing_host_admission_owner: Option<HostAdmissionUnavailablePreStagingOwner>,
    #[cfg(any(test, feature = "test-support"))]
    testing_host_admission_exhaustion_override:
        Option<Option<HostAdmissionAllocatorExhaustionEvidence>>,
    #[cfg(any(test, feature = "test-support"))]
    testing_host_admission_authority: Option<CoreHostLease>,
    #[cfg(feature = "test-support")]
    back_capacity_core_producer: Option<crate::test_support::BackCapacityCoreProducer>,
}

impl RouterEngine {
    pub fn install(tree: RouteTree, fallback: RouteViewKey) -> Result<Self, IntegrationError> {
        let inner = CoreRouterEngine::install(tree, fallback).map_err(map_install_error)?;
        let admission = HostAdmissionIssuer::new().map_err(|_| IntegrationError::Exhausted)?;
        Ok(Self {
            inner,
            admission,
            host_trace: Vec::new(),
            #[cfg(any(test, feature = "test-support"))]
            testing_host_admission_owner: None,
            #[cfg(any(test, feature = "test-support"))]
            testing_host_admission_exhaustion_override: None,
            #[cfg(any(test, feature = "test-support"))]
            testing_host_admission_authority: None,
            #[cfg(feature = "test-support")]
            back_capacity_core_producer: None,
        })
    }

    #[doc(hidden)]
    pub fn bind_host_admission_unavailable_cause<Evidence>(
        &self,
        source: HostAdmissionSourceProjection,
        projection: &HostAdmissionUnavailableEvidenceProjection,
        evidence: Evidence,
    ) -> HostAdmissionUnavailableCauseBindOutcome<Evidence> {
        match self.inner.bind_host_admission_unavailable_cause(
            &self.admission,
            source.inner,
            &projection.inner,
            evidence,
        ) {
            CoreHostAdmissionUnavailableCauseBindOutcome::Bound(prepared) => {
                HostAdmissionUnavailableCauseBindOutcome::Bound(prepared.into())
            }
            CoreHostAdmissionUnavailableCauseBindOutcome::Mismatch {
                source,
                evidence,
                projection,
                mismatch,
            } => HostAdmissionUnavailableCauseBindOutcome::Mismatch {
                source: HostAdmissionSourceProjection { inner: source },
                evidence,
                projection: HostAdmissionUnavailableEvidenceProjection { inner: projection },
                mismatch: HostAdmissionSourceIneligibleProjection { inner: mismatch },
            },
        }
    }

    #[doc(hidden)]
    pub fn accept_post_commit_authority_transfer(
        &mut self,
        normal: &CleanupBatch,
        cause: PostCommitAuthorityCause,
    ) -> PostCommitAuthorityAcceptanceOutcome {
        match self
            .inner
            .accept_post_commit_authority_transfer(&normal.inner, cause.into())
        {
            CorePostCommitAuthorityAcceptanceOutcome::Accepted(inner) => {
                PostCommitAuthorityAcceptanceOutcome::Accepted(PostCommitAuthorityTransferOwner {
                    inner,
                    normal_manifest_digest: normal.manifest_digest,
                    normal_obligation_count: normal.obligation_count,
                })
            }
            CorePostCommitAuthorityAcceptanceOutcome::Mismatch { expected, observed } => {
                PostCommitAuthorityAcceptanceOutcome::Mismatch {
                    expected: PostCommitCleanupReceiverView { inner: expected },
                    observed: PostCommitCleanupObservedView { inner: observed },
                }
            }
        }
    }

    #[doc(hidden)]
    pub fn prepare_post_commit_authority_transfer(
        &mut self,
        source: PostCommitAuthorityPreparationSource,
        manifest: CleanupManifest,
    ) -> PostCommitAuthorityPreparationOutcome {
        let PostCommitAuthorityPreparationSource {
            inner: source,
            normal_manifest_digest,
            normal_obligation_count,
        } = source;
        let authority_manifest_digest = manifest.manifest_digest;
        let authority_obligation_count = manifest.obligation_count;
        let seal = CleanupManifestSeal::new(
            manifest.logical_roots,
            CleanupManifestDigest::from_opaque(authority_manifest_digest),
            authority_obligation_count,
        );
        match self
            .inner
            .prepare_post_commit_authority_transfer(source, seal)
        {
            CorePostCommitAuthorityPreparationOutcome::Prepared { owner, batch } => {
                PostCommitAuthorityPreparationOutcome::Prepared {
                    owner: PreparedPostCommitAuthorityTransfer {
                        inner: owner,
                        normal_manifest_digest,
                        normal_obligation_count,
                        authority_manifest_digest,
                        authority_obligation_count,
                    },
                    batch: PostCommitCleanupBatchView {
                        inner: batch,
                        manifest_digest: authority_manifest_digest,
                        obligation_count: authority_obligation_count,
                    },
                }
            }
            CorePostCommitAuthorityPreparationOutcome::Mismatch { source, evidence } => {
                PostCommitAuthorityPreparationOutcome::Mismatch {
                    source: PostCommitAuthorityPreparationSource {
                        inner: source,
                        normal_manifest_digest,
                        normal_obligation_count,
                    },
                    evidence: PostCommitReceiverMismatchView { inner: evidence },
                }
            }
            CorePostCommitAuthorityPreparationOutcome::Retryable { source, evidence } => {
                PostCommitAuthorityPreparationOutcome::Retryable {
                    source: PostCommitAuthorityPreparationSource {
                        inner: source,
                        normal_manifest_digest,
                        normal_obligation_count,
                    },
                    evidence: PostCommitAuthorityBeginRetryEvidence { inner: evidence },
                }
            }
            CorePostCommitAuthorityPreparationOutcome::Permanent { source, evidence } => {
                PostCommitAuthorityPreparationOutcome::Permanent {
                    source: PostCommitAuthorityPreparationSource {
                        inner: source,
                        normal_manifest_digest,
                        normal_obligation_count,
                    },
                    evidence: PostCommitAuthorityBeginPermanentEvidence { inner: evidence },
                }
            }
        }
    }

    #[doc(hidden)]
    pub fn begin_post_commit_authority_transfer<Unbound, Bound, BindError, Bind, Install>(
        &mut self,
        owner: PreparedPostCommitAuthorityTransfer,
        unbound: Unbound,
        bind: Bind,
        install: Install,
    ) -> PostCommitAuthorityBeginOutcome<Unbound, Bound, BindError>
    where
        Bind: FnOnce(
            Unbound,
            &PostCommitCleanupBatchView,
            PostCommitSettlementPlanBuilder,
        ) -> Result<(Bound, PostCommitSettlementPlan), (Unbound, BindError)>,
        Install: FnOnce(Bound, PostCommitCompoundBatchOwner),
    {
        let PreparedPostCommitAuthorityTransfer {
            inner: owner,
            normal_manifest_digest,
            normal_obligation_count,
            authority_manifest_digest,
            authority_obligation_count,
        } = owner;
        let outcome = self.inner.begin_post_commit_authority_transfer(
            &mut self.admission,
            owner,
            unbound,
            |unbound, batch, plan| {
                bind(
                    unbound,
                    &PostCommitCleanupBatchView {
                        inner: batch.clone(),
                        manifest_digest: authority_manifest_digest,
                        obligation_count: authority_obligation_count,
                    },
                    PostCommitSettlementPlanBuilder { inner: plan },
                )
                .map(|(bound, plan)| (bound, plan.inner))
            },
            |bound, batch| {
                install(
                    bound,
                    PostCommitCompoundBatchOwner {
                        inner: batch,
                        normal_manifest_digest,
                        normal_obligation_count,
                        authority_manifest_digest,
                        authority_obligation_count,
                    },
                );
            },
        );
        let wrap_owner = |inner| PreparedPostCommitAuthorityTransfer {
            inner,
            normal_manifest_digest,
            normal_obligation_count,
            authority_manifest_digest,
            authority_obligation_count,
        };
        match outcome {
            CorePostCommitAuthorityBeginOutcome::Begun => PostCommitAuthorityBeginOutcome::Begun,
            CorePostCommitAuthorityBeginOutcome::BindingMismatch {
                owner,
                unbound,
                evidence,
            } => PostCommitAuthorityBeginOutcome::BindingMismatch {
                owner: wrap_owner(owner),
                unbound,
                evidence,
            },
            CorePostCommitAuthorityBeginOutcome::ReceiverMismatch {
                owner,
                bound,
                evidence,
            } => PostCommitAuthorityBeginOutcome::ReceiverMismatch {
                owner: wrap_owner(owner),
                bound,
                evidence: PostCommitReceiverMismatchView { inner: evidence },
            },
            CorePostCommitAuthorityBeginOutcome::Retryable {
                owner,
                bound,
                evidence,
            } => PostCommitAuthorityBeginOutcome::Retryable {
                owner: wrap_owner(owner),
                bound,
                evidence: PostCommitAuthorityBeginRetryEvidence { inner: evidence },
            },
            CorePostCommitAuthorityBeginOutcome::Permanent {
                owner,
                bound,
                evidence,
            } => PostCommitAuthorityBeginOutcome::Permanent {
                owner: wrap_owner(owner),
                bound,
                evidence: PostCommitAuthorityBeginPermanentEvidence { inner: evidence },
            },
        }
    }

    /// Trusted ECS integration SPI. This moves the sole compound owner into an
    /// opaque permit whose expected role/key/disposition cannot be inspected.
    #[doc(hidden)]
    pub fn prepare_next_post_commit_settlement_trusted(
        &mut self,
        owner: PostCommitCompoundBatchOwner,
    ) -> TrustedPostCommitSettlementPermitOutcome {
        let PostCommitCompoundBatchOwner {
            inner,
            normal_manifest_digest,
            normal_obligation_count,
            authority_manifest_digest,
            authority_obligation_count,
        } = owner;
        match self.inner.prepare_next_post_commit_settlement(inner) {
            CorePostCommitSettlementPermitOutcome::Prepared(inner) => {
                TrustedPostCommitSettlementPermitOutcome::Prepared(
                    TrustedPostCommitSettlementAttempt {
                        inner,
                        normal_manifest_digest,
                        normal_obligation_count,
                        authority_manifest_digest,
                        authority_obligation_count,
                    },
                )
            }
            CorePostCommitSettlementPermitOutcome::Mismatch { owner, evidence } => {
                TrustedPostCommitSettlementPermitOutcome::Mismatch {
                    owner: PostCommitCompoundBatchOwner {
                        inner: owner,
                        normal_manifest_digest,
                        normal_obligation_count,
                        authority_manifest_digest,
                        authority_obligation_count,
                    },
                    evidence: PostCommitSettlementTraceMismatchEvidence { inner: evidence },
                }
            }
        }
    }

    /// Trusted ECS integration SPI. `physical` must be the private non-Clone
    /// proof emitted by the real ECS post-write/refreeze/registry predicate.
    /// The API accepts no raw observed facts and records only Core's private
    /// permit fact.
    #[doc(hidden)]
    pub fn record_next_post_commit_settlement_trusted<Physical>(
        &mut self,
        attempt: TrustedPostCommitSettlementAttempt,
        physical: Physical,
    ) -> TrustedPostCommitSettlementRecordOutcome<Physical> {
        let TrustedPostCommitSettlementAttempt {
            inner: attempt,
            normal_manifest_digest,
            normal_obligation_count,
            authority_manifest_digest,
            authority_obligation_count,
        } = attempt;
        match self.inner.record_next_post_commit_settlement(
            attempt,
            physical,
            |physical, permit| {
                Ok::<_, (Physical, std::convert::Infallible)>(
                    crate::engine::CorePostCommitSettlementAttestation::from_trusted_physical(
                        physical, permit,
                    ),
                )
            },
        ) {
            CorePostCommitSettlementRecordOutcome::Recorded { owner, trace } => {
                TrustedPostCommitSettlementRecordOutcome::Recorded {
                    owner: PostCommitCompoundBatchOwner {
                        inner: owner,
                        normal_manifest_digest,
                        normal_obligation_count,
                        authority_manifest_digest,
                        authority_obligation_count,
                    },
                    trace: PostCommitSettlementTraceView { inner: trace },
                }
            }
            CorePostCommitSettlementRecordOutcome::Mismatch {
                attempt,
                physical,
                evidence,
            } => TrustedPostCommitSettlementRecordOutcome::Mismatch {
                attempt: TrustedPostCommitSettlementAttempt {
                    inner: attempt,
                    normal_manifest_digest,
                    normal_obligation_count,
                    authority_manifest_digest,
                    authority_obligation_count,
                },
                physical,
                evidence: PostCommitSettlementTraceMismatchEvidence { inner: evidence },
            },
            CorePostCommitSettlementRecordOutcome::BindingMismatch {
                attempt: _,
                physical: _,
                bind_error,
            } => match bind_error {},
            CorePostCommitSettlementRecordOutcome::AttestationMismatch { .. } => {
                unreachable!("the trusted wrapper binds only the private Core permit")
            }
        }
    }

    #[doc(hidden)]
    pub(crate) fn prepare_post_commit_compound_completion(
        &mut self,
        batch: PostCommitCompoundBatchOwner,
        inputs: PostCommitCompoundCompletionInputs,
    ) -> PostCommitCompoundCompletionPreparationOutcome {
        let PostCommitCompoundBatchOwner {
            inner: batch,
            normal_manifest_digest,
            normal_obligation_count,
            authority_manifest_digest,
            authority_obligation_count,
        } = batch;
        match self
            .inner
            .prepare_post_commit_compound_completion(batch, inputs.inner)
        {
            CorePostCommitCompoundCompletionPreparationOutcome::Prepared(inner) => {
                PostCommitCompoundCompletionPreparationOutcome::Prepared(
                    PostCommitCompoundCompletionOwner { inner },
                )
            }
            CorePostCommitCompoundCompletionPreparationOutcome::Mismatch {
                batch,
                inputs,
                evidence,
            } => PostCommitCompoundCompletionPreparationOutcome::Mismatch {
                batch: PostCommitCompoundBatchOwner {
                    inner: batch,
                    normal_manifest_digest,
                    normal_obligation_count,
                    authority_manifest_digest,
                    authority_obligation_count,
                },
                inputs: PostCommitCompoundCompletionInputs { inner: inputs },
                evidence: PostCommitCompoundMismatchEvidence { inner: evidence },
            },
        }
    }

    /// Trusted ECS integration SPI. The whole physical compound is accepted
    /// only after ECS has produced its private terminal proof. Authority/H
    /// completion facts are derived inside Core from the exact consumed plan;
    /// no raw expected/observed constructor is exposed here.
    #[doc(hidden)]
    pub fn prepare_post_commit_compound_completion_trusted<Physical>(
        &mut self,
        batch: PostCommitCompoundBatchOwner,
        normal: CleanupCompletion,
        physical: Physical,
    ) -> TrustedPostCommitCompoundCompletionPreparationOutcome<Physical> {
        let PostCommitCompoundBatchOwner {
            inner: batch,
            normal_manifest_digest,
            normal_obligation_count,
            authority_manifest_digest,
            authority_obligation_count,
        } = batch;
        match self
            .inner
            .prepare_post_commit_compound_completion_from_trusted_physical(batch, normal.inner)
        {
            CorePostCommitCompoundCompletionPreparationOutcome::Prepared(inner) => {
                TrustedPostCommitCompoundCompletionPreparationOutcome::Prepared {
                    owner: PostCommitCompoundCompletionOwner { inner },
                    physical,
                }
            }
            CorePostCommitCompoundCompletionPreparationOutcome::Mismatch {
                batch,
                inputs,
                evidence,
            } => TrustedPostCommitCompoundCompletionPreparationOutcome::Mismatch {
                batch: PostCommitCompoundBatchOwner {
                    inner: batch,
                    normal_manifest_digest,
                    normal_obligation_count,
                    authority_manifest_digest,
                    authority_obligation_count,
                },
                physical,
                inputs: PostCommitCompoundCompletionInputs { inner: inputs },
                evidence: PostCommitCompoundMismatchEvidence { inner: evidence },
            },
        }
    }

    #[doc(hidden)]
    pub fn complete_post_commit_authority_compound<PhysicalTerminal, F>(
        &mut self,
        owner: PostCommitCompoundCompletionOwner,
        clear_physical_holder: F,
    ) -> PostCommitCompoundCompletionOutcome<PhysicalTerminal>
    where
        F: FnOnce(RetiredHostAuthorityExpectedView) -> PhysicalTerminal,
    {
        match self
            .inner
            .complete_post_commit_authority_compound(owner.inner, |expected| {
                clear_physical_holder(RetiredHostAuthorityExpectedView { inner: expected })
            }) {
            CorePostCommitCompoundCompletionOutcome::Terminal {
                terminal,
                physical,
                retired_host,
            } => PostCommitCompoundCompletionOutcome::Terminal {
                terminal: PostCommitTerminalEvidence { inner: terminal },
                physical,
                retired_host: RetiredHostAuthorityTerminalEvidence {
                    inner: retired_host,
                },
            },
            CorePostCommitCompoundCompletionOutcome::Mismatch { owner, evidence } => {
                PostCommitCompoundCompletionOutcome::Mismatch {
                    owner: PostCommitCompoundCompletionOwner { inner: owner },
                    evidence: PostCommitCompoundMismatchEvidence { inner: evidence },
                }
            }
        }
    }

    #[cfg(feature = "test-support")]
    #[doc(hidden)]
    pub fn prepare_back_capacity_core_producer_slot(
        &mut self,
    ) -> Option<BackCapacityCoreProducerSlot<'_>> {
        if self.back_capacity_core_producer.is_some() {
            return None;
        }
        Some(BackCapacityCoreProducerSlot {
            slot: &mut self.back_capacity_core_producer,
        })
    }

    #[cfg(any(test, feature = "test-support"))]
    #[doc(hidden)]
    pub fn testing_replace_host_admission_with_non_owning_issuer(&mut self) -> bool {
        let Ok(non_owning) = HostAdmissionIssuer::new() else {
            return false;
        };
        self.admission = non_owning;
        true
    }

    #[cfg(any(test, feature = "test-support"))]
    fn testing_issue_core_host_admission_permit(&mut self) -> CoreHostAdmissionSourcePermit {
        assert!(self.testing_host_admission_owner.is_none());
        match self.inner.issue_host_admission_source_permit() {
            CoreHostAdmissionSourcePermitOutcome::Issued(permit) => permit,
            CoreHostAdmissionSourcePermitOutcome::Ineligible { .. }
            | CoreHostAdmissionSourcePermitOutcome::Outstanding { .. } => {
                panic!("the testing fixture must expose one exact source permit")
            }
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    fn testing_host_admission_operation_input(
        &self,
        source: CoreHostAdmissionSourceProjection,
    ) -> TestingHostAdmissionOperationInput {
        let issuer = self.admission.projection();
        let outstanding = issuer.outstanding.clone().unwrap_or_else(|| {
            let mut probe = HostAdmissionIssuer::new()
                .expect("the test-only projection probe must allocate an issuer");
            let _ = probe
                .issue(source.router)
                .expect("the test-only projection probe must issue once");
            probe
                .projection()
                .outstanding
                .expect("the real issuer predicate produced a requested projection")
        });
        TestingHostAdmissionOperationInput {
            source: HostAdmissionSourceProjection { inner: source },
            expected_issuer: HostAdmissionIssuerProjection {
                inner: issuer.clone(),
            },
            observed_issuer: HostAdmissionIssuerProjection { inner: issuer },
            outstanding: HostAdmissionIssuerOutstandingProjection { inner: outstanding },
            pending: self
                .inner
                .testing_pending_host_admission_projection()
                .map(|inner| PendingHostAdmissionProjection { inner }),
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    fn testing_arm_host_admission_permit_owner(&mut self) -> TestingHostAdmissionOperationInput {
        let permit = self.testing_issue_core_host_admission_permit();
        let source = permit.source.clone();
        self.testing_host_admission_owner = Some(HostAdmissionUnavailablePreStagingOwner::Permit(
            HostAdmissionSourcePermit { inner: permit },
        ));
        self.testing_host_admission_operation_input(source)
    }

    #[cfg(any(test, feature = "test-support"))]
    fn testing_arm_host_admission_token_owner(
        &mut self,
    ) -> (
        CoreHostAdmissionSourceProjection,
        CoreSourceBoundHostAdmissionToken,
    ) {
        let permit = self.testing_issue_core_host_admission_permit();
        let source = permit.source.clone();
        let token = self
            .inner
            .request_host_admission(&mut self.admission, permit)
            .unwrap_or_else(|_| panic!("the exact testing permit must produce one token owner"));
        (source, token)
    }

    #[doc(hidden)]
    #[cfg(any(test, feature = "test-support"))]
    pub fn testing_arm_host_admission_outstanding_request_input(
        &mut self,
    ) -> TestingHostAdmissionOperationInput {
        let source = self.inner.testing_host_admission_source_projection();
        let _ownerless = self
            .admission
            .issue(source.router)
            .expect("the real outstanding-request input must issue once");
        self.testing_arm_host_admission_permit_owner()
    }

    #[doc(hidden)]
    #[cfg(any(test, feature = "test-support"))]
    pub fn testing_arm_host_admission_authority_exists_request_input(
        &mut self,
    ) -> TestingHostAdmissionOperationInput {
        let token = self
            .inner
            .request_legacy_host_admission(&mut self.admission)
            .expect("the authority input must request once");
        let grant = self
            .inner
            .grant_legacy_host_admission(&mut self.admission, &token)
            .expect("the authority input must grant once");
        let confirmation = self
            .admission
            .confirm(&grant)
            .expect("the authority input must confirm once");
        self.testing_host_admission_authority = Some(confirmation.into_lease());
        self.testing_arm_host_admission_permit_owner()
    }

    #[doc(hidden)]
    #[cfg(any(test, feature = "test-support"))]
    pub fn testing_arm_host_admission_foreign_router_grant_input(
        &mut self,
    ) -> TestingHostAdmissionOperationInput {
        let (source, token) = self.testing_arm_host_admission_token_owner();
        assert!(
            self.inner
                .testing_replace_host_allocator_with_foreign_domain()
        );
        self.testing_host_admission_owner = Some(HostAdmissionUnavailablePreStagingOwner::Token(
            SourceBoundHostAdmissionToken { inner: token },
        ));
        self.testing_host_admission_operation_input(source)
    }

    #[doc(hidden)]
    #[cfg(any(test, feature = "test-support"))]
    pub fn testing_arm_host_admission_stale_token_grant_input(
        &mut self,
    ) -> TestingHostAdmissionOperationInput {
        let (source, token) = self.testing_arm_host_admission_token_owner();
        assert!(CoreRouterEngine::testing_cancel_source_bound_token(
            &mut self.admission,
            &token,
        ));
        self.testing_host_admission_owner = Some(HostAdmissionUnavailablePreStagingOwner::Token(
            SourceBoundHostAdmissionToken { inner: token },
        ));
        self.testing_host_admission_operation_input(source)
    }

    #[doc(hidden)]
    #[cfg(any(test, feature = "test-support"))]
    pub fn testing_arm_host_admission_request_sequence_exhaustion_input(
        &mut self,
    ) -> TestingHostAdmissionOperationInput {
        self.admission.testing_exhaust_request_sequence_counter();
        self.testing_arm_host_admission_permit_owner()
    }

    #[doc(hidden)]
    #[cfg(any(test, feature = "test-support"))]
    pub fn testing_arm_host_admission_request_exhaustion_with_allocator_counter_input(
        &mut self,
        counter: HostAdmissionAllocatorCounter,
    ) -> TestingHostAdmissionOperationInput {
        self.admission.testing_exhaust_request_sequence_counter();
        self.testing_host_admission_exhaustion_override =
            Some(Some(HostAdmissionAllocatorExhaustionEvidence {
                counter,
                current: u64::MAX,
                attempted: u64::MAX,
            }));
        self.testing_arm_host_admission_permit_owner()
    }

    #[doc(hidden)]
    #[cfg(any(test, feature = "test-support"))]
    pub fn testing_arm_host_admission_authority_and_allocator_counter_input(
        &mut self,
        counter: HostAdmissionAllocatorCounter,
    ) -> TestingHostAdmissionOperationInput {
        let input = self.testing_arm_host_admission_authority_exists_request_input();
        self.testing_host_admission_exhaustion_override =
            Some(Some(HostAdmissionAllocatorExhaustionEvidence {
                counter,
                current: u64::MAX,
                attempted: u64::MAX,
            }));
        input
    }

    #[doc(hidden)]
    #[cfg(any(test, feature = "test-support"))]
    pub fn testing_take_armed_host_admission_owner(
        &mut self,
    ) -> Option<HostAdmissionUnavailablePreStagingOwner> {
        self.testing_host_admission_owner.take()
    }

    #[doc(hidden)]
    #[cfg(any(test, feature = "test-support"))]
    pub fn testing_take_host_admission_exhaustion_override(
        &mut self,
    ) -> Option<Option<HostAdmissionAllocatorExhaustionEvidence>> {
        self.testing_host_admission_exhaustion_override.take()
    }

    #[doc(hidden)]
    #[cfg(any(test, feature = "test-support"))]
    pub fn testing_pending_host_admission_projection(
        &self,
    ) -> Option<PendingHostAdmissionProjection> {
        self.inner
            .testing_pending_host_admission_projection()
            .map(|inner| PendingHostAdmissionProjection { inner })
    }

    #[doc(hidden)]
    #[cfg(any(test, feature = "test-support"))]
    pub fn testing_drift_prepared_host_confirmation_receiver_input(&mut self) -> bool {
        self.inner
            .testing_drift_prepared_host_confirmation_receiver_input()
    }

    #[doc(hidden)]
    #[cfg(any(test, feature = "test-support"))]
    pub fn testing_drift_host_confirmation_preparation_receiver_input(&mut self) -> bool {
        self.inner
            .testing_drift_host_confirmation_preparation_receiver_input()
    }

    #[doc(hidden)]
    #[cfg(any(test, feature = "test-support"))]
    pub fn testing_host_admission_issuer_projection(&self) -> HostAdmissionIssuerProjection {
        HostAdmissionIssuerProjection {
            inner: self.admission.projection(),
        }
    }

    pub fn request_host_admission(&mut self) -> Result<HostAdmissionToken, IntegrationError> {
        let inner = self
            .inner
            .request_legacy_host_admission(&mut self.admission)
            .map_err(map_admission_error)?;
        self.host_trace.push(HostTraceEvent::project(
            HostTraceEventKind::AdmissionRequested,
            None,
            None,
        ));
        Ok(HostAdmissionToken { inner })
    }

    #[doc(hidden)]
    pub fn issue_host_admission_source_permit(&mut self) -> HostAdmissionSourcePermitOutcome {
        match self.inner.issue_host_admission_source_permit() {
            CoreHostAdmissionSourcePermitOutcome::Issued(inner) => {
                HostAdmissionSourcePermitOutcome::Issued(HostAdmissionSourcePermit { inner })
            }
            CoreHostAdmissionSourcePermitOutcome::Ineligible { source, evidence } => {
                HostAdmissionSourcePermitOutcome::Ineligible {
                    source: HostAdmissionSourceProjection { inner: source },
                    evidence: HostAdmissionSourceIneligibleProjection { inner: evidence },
                }
            }
            CoreHostAdmissionSourcePermitOutcome::Outstanding { source } => {
                HostAdmissionSourcePermitOutcome::Outstanding {
                    source: HostAdmissionSourceProjection { inner: source },
                }
            }
        }
    }

    #[doc(hidden)]
    pub fn request_source_bound_host_admission(
        &mut self,
        permit: HostAdmissionSourcePermit,
    ) -> Result<SourceBoundHostAdmissionToken, AdmissionOwnerError<HostAdmissionSourcePermit>> {
        let source = permit.source_projection();
        let observed_len = self.host_trace.len();
        let observed_capacity = self.host_trace.capacity();
        if self.host_trace.try_reserve(1).is_err() {
            return Err(AdmissionOwnerError {
                owner: permit,
                evidence: AdmissionOwnerFailureEvidence::TraceCapacity(
                    HostAdmissionTraceCapacityFailureEvidence::new(
                        source,
                        None,
                        HostAdmissionTraceReservationStage::Request,
                        1,
                        observed_len,
                        observed_capacity,
                    ),
                ),
            });
        }
        match self
            .inner
            .request_host_admission(&mut self.admission, permit.inner)
        {
            Ok(inner) => {
                self.host_trace.push(HostTraceEvent::project(
                    HostTraceEventKind::AdmissionRequested,
                    None,
                    None,
                ));
                Ok(SourceBoundHostAdmissionToken { inner })
            }
            Err(error) => {
                let evidence = error.evidence;
                #[cfg(any(test, feature = "test-support"))]
                let evidence = match self.testing_host_admission_exhaustion_override.take() {
                    Some(exhaustion) => match evidence {
                        CoreHostAdmissionOperationFailureFreezeOutcome::Frozen(frozen) => {
                            let projection = frozen.projection;
                            crate::engine::freeze_host_admission_operation_failure(
                                projection.source,
                                projection.operation,
                                projection.error,
                                projection.expected_issuer,
                                projection.observed_issuer,
                                exhaustion.map(Into::into),
                            )
                        }
                        mismatch @ CoreHostAdmissionOperationFailureFreezeOutcome::Mismatch(_) => {
                            mismatch
                        }
                    },
                    None => evidence,
                };
                Err(AdmissionOwnerError {
                    owner: HostAdmissionSourcePermit { inner: error.owner },
                    evidence: AdmissionOwnerFailureEvidence::Operation(evidence.into()),
                })
            }
        }
    }

    #[doc(hidden)]
    pub fn grant_source_bound_host_admission(
        &mut self,
        token: SourceBoundHostAdmissionToken,
    ) -> Result<SourceBoundHostGrant, AdmissionOwnerError<SourceBoundHostAdmissionToken>> {
        let source = token.source_projection();
        let observed_len = self.host_trace.len();
        let observed_capacity = self.host_trace.capacity();
        if self.host_trace.try_reserve(1).is_err() {
            return Err(AdmissionOwnerError {
                owner: token,
                evidence: AdmissionOwnerFailureEvidence::TraceCapacity(
                    HostAdmissionTraceCapacityFailureEvidence::new(
                        source,
                        None,
                        HostAdmissionTraceReservationStage::Grant,
                        1,
                        observed_len,
                        observed_capacity,
                    ),
                ),
            });
        }
        match self
            .inner
            .grant_host_admission(&mut self.admission, token.inner)
        {
            Ok(inner) => {
                self.host_trace.push(HostTraceEvent::project(
                    HostTraceEventKind::HostGranted,
                    Some(inner.grant.version().clone()),
                    Some(inner.grant.top_slot()),
                ));
                Ok(SourceBoundHostGrant { inner })
            }
            Err(error) => Err(AdmissionOwnerError {
                owner: SourceBoundHostAdmissionToken { inner: error.owner },
                evidence: AdmissionOwnerFailureEvidence::Operation(error.evidence.into()),
            }),
        }
    }

    #[doc(hidden)]
    pub fn complete_pre_staging_host_admission_unavailable<
        Installation,
        Evidence,
        PhysicalTerminal,
        Consume,
    >(
        &mut self,
        source: HostAdmissionUnavailablePreStagingSource<Installation, Evidence>,
        consume_installation_and_clear_holder: Consume,
    ) -> HostAdmissionUnavailablePreStagingOutcome<Installation, Evidence, PhysicalTerminal>
    where
        Consume: FnOnce(Installation) -> PhysicalTerminal,
    {
        match self.inner.complete_pre_staging_host_admission_unavailable(
            &mut self.admission,
            source.into(),
            consume_installation_and_clear_holder,
        ) {
            CoreHostAdmissionUnavailablePreStagingOutcome::Published { terminal, physical } => {
                HostAdmissionUnavailablePreStagingOutcome::Published {
                    terminal: terminal.into(),
                    physical,
                }
            }
            CoreHostAdmissionUnavailablePreStagingOutcome::Mismatch { source, evidence } => {
                HostAdmissionUnavailablePreStagingOutcome::Mismatch {
                    source: source.into(),
                    evidence: evidence.into(),
                }
            }
        }
    }

    #[doc(hidden)]
    pub fn complete_observed_host_admission_unavailable<Evidence, Clear>(
        &mut self,
        prepared: PreparedHostAdmissionUnavailableCause<Evidence>,
        clear_holder: Clear,
    ) -> HostAdmissionUnavailableObservedOutcome<Evidence>
    where
        Clear: FnOnce(),
    {
        match self.inner.complete_observed_host_admission_unavailable(
            &self.admission,
            prepared.into(),
            clear_holder,
        ) {
            CoreHostAdmissionUnavailableObservedOutcome::Published(terminal) => {
                HostAdmissionUnavailableObservedOutcome::Published(terminal.into())
            }
            CoreHostAdmissionUnavailableObservedOutcome::Stable { source, evidence } => {
                HostAdmissionUnavailableObservedOutcome::Stable {
                    source: HostAdmissionSourceProjection { inner: source },
                    evidence,
                }
            }
            CoreHostAdmissionUnavailableObservedOutcome::Awaiting { source, evidence } => {
                HostAdmissionUnavailableObservedOutcome::Awaiting {
                    source: HostAdmissionSourceProjection { inner: source },
                    evidence,
                }
            }
            CoreHostAdmissionUnavailableObservedOutcome::Mismatch { prepared, evidence } => {
                HostAdmissionUnavailableObservedOutcome::Mismatch {
                    prepared: prepared.into(),
                    evidence: evidence.into(),
                }
            }
        }
    }

    #[doc(hidden)]
    pub fn complete_granted_host_admission_unavailable<
        Installation,
        Evidence,
        PhysicalTerminal,
        Consume,
    >(
        &mut self,
        source: HostAdmissionUnavailableGrantedSource<Installation, Evidence>,
        consume_physical_terminal: Consume,
    ) -> HostAdmissionUnavailableGrantedOutcome<Installation, Evidence, PhysicalTerminal>
    where
        Consume: FnOnce(Installation) -> PhysicalTerminal,
    {
        match self.inner.complete_granted_host_admission_unavailable(
            &mut self.admission,
            source.into(),
            consume_physical_terminal,
        ) {
            CoreHostAdmissionUnavailableGrantedOutcome::Published { terminal, physical } => {
                HostAdmissionUnavailableGrantedOutcome::Published {
                    terminal: terminal.into(),
                    physical,
                }
            }
            CoreHostAdmissionUnavailableGrantedOutcome::Mismatch { source, evidence } => {
                HostAdmissionUnavailableGrantedOutcome::Mismatch {
                    source: source.into(),
                    evidence: evidence.into(),
                }
            }
        }
    }

    #[doc(hidden)]
    pub fn prepare_host_confirmation<Installation>(
        &mut self,
        owner: CoreHostConfirmationSource<Installation>,
    ) -> HostConfirmationPreparationOutcome<Installation> {
        let source = owner.source_projection();
        let receiver = HostConfirmationReceiverProjection {
            inner: owner.inner.receiver_projection(),
        };
        let observed_len = self.host_trace.len();
        let observed_capacity = self.host_trace.capacity();
        if self.host_trace.try_reserve(1).is_err() {
            return HostConfirmationPreparationOutcome::PublicationPreparationRejected {
                owner,
                evidence: HostConfirmationPreparationCapacityFailureEvidence {
                    inner: HostAdmissionTraceCapacityFailureEvidence::new(
                        source,
                        Some(receiver),
                        HostAdmissionTraceReservationStage::ConfirmationPreparation,
                        1,
                        observed_len,
                        observed_capacity,
                    ),
                },
            };
        }
        match self.inner.prepare_host_confirmation(owner.inner) {
            CoreHostConfirmationPreparationOutcome::Prepared(inner) => {
                let ready_trace = HostTraceEvent::project(
                    HostTraceEventKind::HostReady,
                    Some(inner.ready.projection.host.clone()),
                    Some(inner.ready.projection.top_slot),
                );
                HostConfirmationPreparationOutcome::Prepared(CorePreparedHostConfirmation {
                    inner,
                    ready_trace,
                })
            }
            CoreHostConfirmationPreparationOutcome::ReceiverMismatch { owner, evidence } => {
                HostConfirmationPreparationOutcome::ReceiverMismatch {
                    owner: CoreHostConfirmationSource { inner: owner },
                    evidence: evidence.into(),
                }
            }
        }
    }

    #[doc(hidden)]
    pub fn unwind_host_confirmation<Installation>(
        &mut self,
        owner: CorePreparedHostConfirmation<Installation>,
    ) -> HostConfirmationUnwindOutcome<Installation> {
        let CorePreparedHostConfirmation { inner, ready_trace } = owner;
        match self.inner.unwind_host_confirmation(inner) {
            CoreHostConfirmationUnwindOutcome::Grant(inner) => {
                HostConfirmationUnwindOutcome::Grant(CoreHostConfirmationSource { inner })
            }
            CoreHostConfirmationUnwindOutcome::Mismatch { owner, evidence } => {
                HostConfirmationUnwindOutcome::Mismatch {
                    owner: CorePreparedHostConfirmation {
                        ready_trace,
                        inner: owner,
                    },
                    evidence: evidence.into(),
                }
            }
        }
    }

    #[doc(hidden)]
    pub fn confirm_host_admission_with_binding<Installation, Installed, F>(
        &mut self,
        owner: CorePreparedHostConfirmation<Installation>,
        install_binding: F,
    ) -> HostAdmissionConfirmationCoreOutcome<Installation, Installed>
    where
        F: FnOnce(Installation, HostLease) -> Installed,
    {
        let source = HostAdmissionSourceProjection {
            inner: owner.inner.receiver.source.clone(),
        };
        let receiver = HostConfirmationReceiverProjection {
            inner: owner.inner.receiver.clone(),
        };
        let observed_len = self.host_trace.len();
        let observed_capacity = self.host_trace.capacity();
        let CorePreparedHostConfirmation { inner, ready_trace } = owner;
        if self.host_trace.try_reserve(1).is_err() {
            return HostAdmissionConfirmationCoreOutcome::PublicationPreparationRejected {
                owner: CorePreparedHostConfirmation { inner, ready_trace },
                evidence: HostConfirmationPublicationCapacityFailureEvidence {
                    inner: HostAdmissionTraceCapacityFailureEvidence::new(
                        source,
                        Some(receiver),
                        HostAdmissionTraceReservationStage::ConfirmationPublication,
                        1,
                        observed_len,
                        observed_capacity,
                    ),
                },
            };
        }
        match self.inner.confirm_host_admission_with_binding(
            &mut self.admission,
            inner,
            |installation, inner| install_binding(installation, HostLease { inner }),
        ) {
            CoreHostAdmissionConfirmationCoreOutcome::Ready {
                projection,
                installed,
            } => {
                self.host_trace.push(ready_trace);
                HostAdmissionConfirmationCoreOutcome::Ready {
                    projection: projection.into(),
                    installed,
                }
            }
            CoreHostAdmissionConfirmationCoreOutcome::ReceiverMismatch { owner, evidence } => {
                HostAdmissionConfirmationCoreOutcome::ReceiverMismatch {
                    owner: CorePreparedHostConfirmation {
                        ready_trace,
                        inner: owner,
                    },
                    evidence: evidence.into(),
                }
            }
            CoreHostAdmissionConfirmationCoreOutcome::IssuerRejected { owner, evidence } => {
                HostAdmissionConfirmationCoreOutcome::IssuerRejected {
                    owner: CorePreparedHostConfirmation {
                        ready_trace,
                        inner: owner,
                    },
                    evidence: evidence.into(),
                }
            }
        }
    }

    pub fn reject_host_admission(
        &mut self,
        token: HostAdmissionToken,
    ) -> Result<(), IntegrationError> {
        self.admission
            .reject(&token.inner)
            .map_err(map_admission_error)?;
        self.host_trace.push(HostTraceEvent::project(
            HostTraceEventKind::AdmissionRejected,
            None,
            None,
        ));
        Ok(())
    }

    pub fn grant_host_admission(
        &mut self,
        token: &HostAdmissionToken,
    ) -> Result<HostGrant, IntegrationError> {
        let inner = self
            .inner
            .grant_legacy_host_admission(&mut self.admission, &token.inner)
            .map_err(map_admission_error)?;
        self.host_trace.push(HostTraceEvent::project(
            HostTraceEventKind::HostGranted,
            Some(inner.version().clone()),
            Some(inner.top_slot()),
        ));
        Ok(HostGrant { inner })
    }

    pub fn reject_host_after_mutation(&mut self, grant: HostGrant) -> Result<(), IntegrationError> {
        let version = grant.inner.version().clone();
        let slot = grant.inner.top_slot();
        self.admission
            .reject_after_mutation(&grant.inner)
            .map_err(map_admission_error)?;
        self.host_trace.push(HostTraceEvent::project(
            HostTraceEventKind::AdmissionRejectedAfterMutation,
            Some(version),
            Some(slot),
        ));
        Ok(())
    }

    pub fn record_top_slot_registered(
        &mut self,
        grant: &HostGrant,
    ) -> Result<(), IntegrationError> {
        if !self.admission.is_outstanding_grant(&grant.inner) {
            return Err(IntegrationError::Stale);
        }
        let version = grant.inner.version();
        let slot = grant.inner.top_slot();
        if self.host_trace.iter().any(|event| {
            event.kind() == HostTraceEventKind::TopSlotRegistered
                && event.version() == Some(version)
                && event.slot() == Some(slot)
        }) {
            return Err(IntegrationError::Duplicate);
        }
        self.host_trace.push(HostTraceEvent::project(
            HostTraceEventKind::TopSlotRegistered,
            Some(version.clone()),
            Some(slot),
        ));
        Ok(())
    }

    #[doc(hidden)]
    pub fn record_source_bound_top_slot_registered(
        &mut self,
        grant: &SourceBoundHostGrant,
    ) -> Result<(), SourceBoundTopSlotRegistrationError> {
        let source = grant.source_projection();
        let expected_receiver = HostConfirmationReceiverProjection {
            inner: CoreRouterEngine::confirmation_receiver_for(&grant.inner),
        };
        let expected_issuer = HostAdmissionIssuerProjection {
            inner: grant.inner.expected_issuer_projection(),
        };
        let observed_issuer = HostAdmissionIssuerProjection {
            inner: self.admission.projection(),
        };
        let version = grant.inner.grant.version();
        let slot = grant.inner.grant.top_slot();
        let expected_trace = HostTraceEvent::project(
            HostTraceEventKind::TopSlotRegistered,
            Some(version.clone()),
            Some(slot),
        );
        let observed_trace = self.host_trace.iter().find(|event| {
            event.kind() == HostTraceEventKind::TopSlotRegistered
                && event.version() == Some(version)
                && event.slot() == Some(slot)
        });
        if !self.admission.is_outstanding_grant(&grant.inner.grant) {
            return Err(SourceBoundTopSlotRegistrationError::Operation(
                SourceBoundTopSlotRegistrationOperationFailureEvidence {
                    kind: SourceBoundTopSlotRegistrationFailureKind::StaleGrant,
                    source,
                    expected_receiver,
                    expected_issuer,
                    observed_issuer,
                    expected_trace,
                    observed_trace: observed_trace.cloned(),
                },
            ));
        }
        if let Some(observed_trace) = observed_trace.cloned() {
            return Err(SourceBoundTopSlotRegistrationError::Operation(
                SourceBoundTopSlotRegistrationOperationFailureEvidence {
                    kind: SourceBoundTopSlotRegistrationFailureKind::DuplicateTrace,
                    source,
                    expected_receiver,
                    expected_issuer,
                    observed_issuer,
                    expected_trace,
                    observed_trace: Some(observed_trace),
                },
            ));
        }
        let observed_len = self.host_trace.len();
        let observed_capacity = self.host_trace.capacity();
        self.host_trace.try_reserve(1).map_err(|_| {
            SourceBoundTopSlotRegistrationError::TraceCapacity(
                HostTopSlotRegistrationCapacityFailureEvidence {
                    inner: HostAdmissionTraceCapacityFailureEvidence::new(
                        source,
                        Some(expected_receiver),
                        HostAdmissionTraceReservationStage::TopSlotRegistration,
                        1,
                        observed_len,
                        observed_capacity,
                    ),
                },
            )
        })?;
        self.host_trace.push(expected_trace);
        Ok(())
    }

    #[doc(hidden)]
    pub fn confirm_host(&mut self, grant: HostGrant) -> HostLease {
        let confirmation = self
            .admission
            .confirm(&grant.inner)
            .expect("a live sealed host grant has one valid confirmation");
        self.inner
            .confirm_host(&self.admission, &confirmation)
            .expect("the engine cannot change while its sealed host grant is outstanding");
        self.host_trace.push(HostTraceEvent::project(
            HostTraceEventKind::HostReady,
            self.inner.host().cloned(),
            self.inner.top_slot(),
        ));
        HostLease {
            inner: confirmation.into_lease(),
        }
    }

    #[doc(hidden)]
    pub fn failed_authority_trigger_for_task(
        &mut self,
        task: &FrameTask,
    ) -> Result<FailedAuthoritySettlementTrigger, FailedAuthorityTriggerError> {
        self.inner
            .failed_authority_trigger_for_task(task.inner.identity())
            .map(|inner| FailedAuthoritySettlementTrigger { inner })
            .map_err(map_failed_authority_trigger_error)
    }

    #[doc(hidden)]
    pub fn failed_authority_trigger_for_cleanup(
        &mut self,
        request: &FailureCleanupRequest,
    ) -> Result<FailedAuthoritySettlementTrigger, FailedAuthorityTriggerError> {
        self.inner
            .failed_authority_trigger_for_cleanup(&request.inner)
            .map(|inner| FailedAuthoritySettlementTrigger { inner })
            .map_err(map_failed_authority_trigger_error)
    }

    #[doc(hidden)]
    pub fn failed_authority_trigger_for_registration_begin(
        &mut self,
        error: &MountRegistrationBeginError,
    ) -> Result<FailedAuthoritySettlementTrigger, FailedAuthorityTriggerError> {
        self.inner
            .failed_authority_trigger_for_registration_begin(&error.permit)
            .map(|inner| FailedAuthoritySettlementTrigger { inner })
            .map_err(map_failed_authority_trigger_error)
    }

    #[doc(hidden)]
    pub fn begin_authority_settlement_retaining<F>(
        &mut self,
        lease: HostLease,
        manifest: CleanupManifest,
        authority_mutation: F,
    ) -> Result<CleanupBatch, BeginAuthoritySettlementError>
    where
        F: FnOnce(CleanupBatch),
    {
        self.begin_authority_settlement_for_cause_retaining(
            lease,
            PreCommitAuthorityCause::AuthorityLost,
            manifest,
            authority_mutation,
        )
    }

    #[doc(hidden)]
    pub fn authority_settlement_logical_roots(&self) -> Vec<RouteFrameVersion> {
        self.inner.authority_settlement_logical_roots()
    }

    #[doc(hidden)]
    pub fn begin_authority_settlement_for_cause_retaining<F>(
        &mut self,
        lease: HostLease,
        cause: PreCommitAuthorityCause,
        manifest: CleanupManifest,
        authority_mutation: F,
    ) -> Result<CleanupBatch, BeginAuthoritySettlementError>
    where
        F: FnOnce(CleanupBatch),
    {
        self.begin_authority_settlement_with_trigger(
            lease,
            None,
            cause,
            manifest,
            authority_mutation,
        )
    }

    #[doc(hidden)]
    pub fn begin_failed_authority_settlement_retaining<F>(
        &mut self,
        lease: HostLease,
        trigger: FailedAuthoritySettlementTrigger,
        manifest: CleanupManifest,
        authority_mutation: F,
    ) -> Result<CleanupBatch, BeginAuthoritySettlementError>
    where
        F: FnOnce(CleanupBatch),
    {
        self.begin_failed_authority_settlement_for_cause_retaining(
            lease,
            trigger,
            PreCommitAuthorityCause::AuthorityLost,
            manifest,
            authority_mutation,
        )
    }

    #[doc(hidden)]
    pub fn begin_failed_authority_settlement_for_cause_retaining<F>(
        &mut self,
        lease: HostLease,
        trigger: FailedAuthoritySettlementTrigger,
        cause: PreCommitAuthorityCause,
        manifest: CleanupManifest,
        authority_mutation: F,
    ) -> Result<CleanupBatch, BeginAuthoritySettlementError>
    where
        F: FnOnce(CleanupBatch),
    {
        self.begin_authority_settlement_with_trigger(
            lease,
            Some(trigger),
            cause,
            manifest,
            authority_mutation,
        )
    }

    fn begin_authority_settlement_with_trigger<F>(
        &mut self,
        lease: HostLease,
        trigger: Option<FailedAuthoritySettlementTrigger>,
        cause: PreCommitAuthorityCause,
        manifest: CleanupManifest,
        authority_mutation: F,
    ) -> Result<CleanupBatch, BeginAuthoritySettlementError>
    where
        F: FnOnce(CleanupBatch),
    {
        if self.host_trace.try_reserve(2).is_err() {
            return Err(BeginAuthoritySettlementError {
                kind: BeginAuthoritySettlementErrorKind::RetryablePreparation,
                owner: lease,
            });
        }
        let version = self.inner.host().cloned();
        let slot = self.inner.top_slot();
        let manifest_digest = manifest.manifest_digest;
        let obligation_count = manifest.obligation_count;
        let inner_engine = &mut self.inner;
        let admission = &mut self.admission;
        let host_trace = &mut self.host_trace;
        let seal = CleanupManifestSeal::new(
            manifest.logical_roots,
            CleanupManifestDigest::from_opaque(manifest_digest),
            obligation_count,
        );
        let publication = |_: &CoreRouterEngine, inner: CleanupBatchToken| {
            authority_mutation(CleanupBatch {
                inner: inner.clone(),
                manifest_digest,
                obligation_count,
            });
            host_trace.push(HostTraceEvent::project(
                HostTraceEventKind::SettlementStarted,
                version.clone(),
                slot,
            ));
        };
        let result = match trigger {
            Some(trigger) => inner_engine.begin_failed_authority_settlement_for_cause_retaining(
                admission,
                lease.inner,
                trigger.inner,
                cause.inner(),
                seal,
                publication,
            ),
            None => inner_engine.begin_authority_settlement_for_cause_retaining(
                admission,
                lease.inner,
                cause.inner(),
                seal,
                publication,
            ),
        };
        let inner = match result {
            Ok(inner) => inner,
            Err(error) => {
                return Err(BeginAuthoritySettlementError {
                    kind: map_begin_authority_settlement_error(error.kind()),
                    owner: HostLease {
                        inner: error.into_owner(),
                    },
                });
            }
        };
        Ok(CleanupBatch {
            inner,
            manifest_digest,
            obligation_count,
        })
    }

    #[doc(hidden)]
    pub fn fail_closed_authority_protocol_retaining<F>(
        &mut self,
        lease: Option<HostLease>,
        authority_mutation: F,
    ) -> Result<(), Option<HostLease>>
    where
        F: FnOnce(),
    {
        self.inner
            .fail_closed_authority_protocol_retaining(
                &mut self.admission,
                lease.map(|lease| lease.inner),
                authority_mutation,
            )
            .map_err(|inner| inner.map(|inner| HostLease { inner }))
    }

    pub fn push(
        &mut self,
        location: RouteLocation,
    ) -> Result<NavigationRequestId, IntegrationError> {
        self.inner.enqueue_push(location).map_err(map_push_error)
    }

    pub fn replace(
        &mut self,
        location: RouteLocation,
    ) -> Result<NavigationRequestId, IntegrationError> {
        self.inner.enqueue_replace(location).map_err(map_push_error)
    }

    #[doc(hidden)]
    pub fn claim_history_admission_world(
        &mut self,
    ) -> Result<HistoryAdmissionWorldSeal, IntegrationError> {
        self.inner
            .claim_history_admission_world()
            .map_err(|error| match error {
                ClaimHistoryAdmissionWorldError::Exhausted => IntegrationError::Exhausted,
                ClaimHistoryAdmissionWorldError::AlreadyClaimed => IntegrationError::Duplicate,
            })
    }

    #[doc(hidden)]
    pub fn history_admission_stamp(
        &self,
        world: &HistoryAdmissionWorldSeal,
    ) -> Result<HistoryAdmissionStamp, PrepareBackAdmissionErrorKind> {
        self.inner.history_admission_stamp(world)
    }

    #[doc(hidden)]
    pub fn prepare_back_admission(
        &self,
        current_world: &HistoryAdmissionWorldSeal,
        stamp: &HistoryAdmissionStamp,
    ) -> Result<BackAdmissionToken, PrepareBackAdmissionErrorKind> {
        self.inner.prepare_back_admission(current_world, stamp)
    }

    #[doc(hidden)]
    pub fn enqueue_back(
        &mut self,
        admission: BackAdmissionToken,
    ) -> Result<AcceptedBackReservation, EnqueueBackErrorKind> {
        #[cfg(feature = "test-support")]
        {
            let Self {
                inner,
                back_capacity_core_producer,
                ..
            } = self;
            return inner.enqueue_back_observed(admission, |boundary| {
                let Some(producer) = back_capacity_core_producer.as_mut() else {
                    return;
                };
                match boundary {
                    BackAllocatorBoundary::Attempt => producer.allocator_attempt(),
                    BackAllocatorBoundary::Failure => producer.allocator_failure(),
                }
            });
        }

        #[cfg(not(feature = "test-support"))]
        {
            self.inner.enqueue_back(admission)
        }
    }

    #[doc(hidden)]
    pub fn verify_back_resolution_handoff(
        &mut self,
        accepted: BackAcceptedReservationView,
        handoff: BackResolutionOwnerHandoff,
    ) -> BackResolutionVerifyOutcome {
        self.inner.verify_back_resolution_handoff(accepted, handoff)
    }

    #[doc(hidden)]
    pub fn classify_back_resolution_evidence(
        &self,
        evidence: &BackResolutionEvidence,
    ) -> BackResolutionEvidenceClassification {
        self.inner.classify_back_resolution_evidence(evidence)
    }

    #[doc(hidden)]
    pub fn resume_back_target_with_retirement<F>(
        &mut self,
        owner: VerifiedBackTargetExecutionOwner,
        commit_retirement: F,
    ) -> BackTargetCoreResumeOutcome
    where
        F: FnOnce(),
    {
        match self
            .inner
            .resume_back_target_with_retirement(owner, commit_retirement)
        {
            CoreBackTargetResumeOutcome::Resumed(start) => {
                let start = match start {
                    crate::engine::BackTargetStart::Ready(resume) => {
                        let action = match resume.action {
                            CoreBackTargetResumeAction::FrameTask(inner) => {
                                BackTargetResumeAction::FrameTask(FrameTask { inner })
                            }
                            CoreBackTargetResumeAction::MountRequest(inner) => {
                                BackTargetResumeAction::MountRequest(MountRequest { inner })
                            }
                            CoreBackTargetResumeAction::Commit(inner) => {
                                BackTargetResumeAction::Commit(CommitTicket { inner })
                            }
                        };
                        BackTargetStart::Ready(BackTargetResume {
                            action,
                            execution: HistoryExecutionBindingView {
                                inner: resume.execution,
                            },
                            continuation: LifecycleContinuationCapacity {
                                inner: LifecycleContinuationBinding {
                                    scope: resume.continuation,
                                },
                            },
                        })
                    }
                    crate::engine::BackTargetStart::FailureCleanup(inner) => {
                        BackTargetStart::FailureCleanup(FailureCleanupRequest { inner })
                    }
                };
                BackTargetCoreResumeOutcome::Resumed(start)
            }
            CoreBackTargetResumeOutcome::Permanent(owner) => {
                BackTargetCoreResumeOutcome::Permanent(owner)
            }
            CoreBackTargetResumeOutcome::Mismatch(owner) => {
                let evidence = owner.evidence();
                BackTargetCoreResumeOutcome::Mismatch { owner, evidence }
            }
        }
    }

    #[doc(hidden)]
    pub fn settle_back_target_permanent_exact(
        &mut self,
        owner: BackTargetPermanentOwner,
    ) -> BackTargetPermanentSettlementOutcome {
        match self.inner.settle_back_target_permanent_exact(owner) {
            CoreBackTargetPermanentSettlementOutcome::Settled(terminal) => {
                BackTargetPermanentSettlementOutcome::Settled(terminal)
            }
            CoreBackTargetPermanentSettlementOutcome::Mismatch { owner, evidence } => {
                BackTargetPermanentSettlementOutcome::Mismatch { owner, evidence }
            }
        }
    }

    #[doc(hidden)]
    pub fn refreeze_back_no_target_source_with_transfer<F>(
        &mut self,
        candidate: VerifiedBackNoTargetCandidate,
        commit_transfer: F,
    ) -> BackNoTargetSourceRefreezeOutcome
    where
        F: FnOnce(),
    {
        match self
            .inner
            .refreeze_back_no_target_source_with_transfer(candidate, commit_transfer)
        {
            CoreBackNoTargetSourceRefreezeOutcome::Unchanged(candidate) => {
                BackNoTargetSourceRefreezeOutcome::Unchanged(candidate)
            }
            CoreBackNoTargetSourceRefreezeOutcome::Drift(owner) => {
                BackNoTargetSourceRefreezeOutcome::Drift(owner)
            }
            CoreBackNoTargetSourceRefreezeOutcome::Mismatch {
                candidate,
                evidence,
            } => BackNoTargetSourceRefreezeOutcome::Mismatch {
                candidate,
                evidence,
            },
        }
    }

    #[doc(hidden)]
    pub fn accept_back_no_target_authority_lost_with_retirement<F>(
        &mut self,
        candidate: VerifiedBackNoTargetCandidate,
        commit_retirement: F,
    ) -> BackNoTargetAuthorityCoreOutcome
    where
        F: FnOnce(),
    {
        match self
            .inner
            .accept_back_no_target_authority_lost_with_retirement(candidate, commit_retirement)
        {
            CoreBackNoTargetAuthorityCoreOutcome::Accepted(inner) => {
                BackNoTargetAuthorityCoreOutcome::Accepted(BackNoTargetAuthoritySettlementOwner {
                    inner,
                })
            }
            CoreBackNoTargetAuthorityCoreOutcome::Mismatch {
                candidate,
                mismatch,
            } => BackNoTargetAuthorityCoreOutcome::Mismatch {
                candidate,
                mismatch: PreparationReceiverMismatchView { inner: mismatch },
            },
        }
    }

    #[doc(hidden)]
    pub fn begin_back_no_target_authority_settlement<F>(
        &mut self,
        owner: BackNoTargetAuthorityBeginOwner,
        install: F,
    ) -> BackNoTargetAuthorityBeginOutcome
    where
        F: FnOnce(BackNoTargetAuthorityCleanupBatchOwner),
    {
        if self.host_trace.try_reserve(2).is_err() {
            return BackNoTargetAuthorityBeginOutcome::Retryable {
                owner,
                evidence: BackNoTargetAuthorityBeginRetryEvidence::Capacity,
            };
        }
        let BackNoTargetAuthorityBeginOwner {
            settlement,
            host,
            manifest,
        } = owner;
        let CleanupManifest {
            logical_roots,
            manifest_digest,
            obligation_count,
        } = &manifest.cleanup;
        let seal = CleanupManifestSeal::new(
            logical_roots.clone(),
            CleanupManifestDigest::from_opaque(*manifest_digest),
            *obligation_count,
        );
        let core_owner = CoreBackNoTargetAuthorityBeginOwner::new(
            settlement.inner,
            host.inner,
            seal,
            *manifest_digest,
        );
        let version = self.inner.host().cloned();
        let slot = self.inner.top_slot();
        let settlement_started =
            HostTraceEvent::project(HostTraceEventKind::SettlementStarted, version, slot);
        let inner = &mut self.inner;
        let admission = &mut self.admission;
        let host_trace = &mut self.host_trace;
        let outcome =
            inner.begin_back_no_target_authority_settlement(admission, core_owner, |batch| {
                install(BackNoTargetAuthorityCleanupBatchOwner { inner: batch });
            });
        match outcome {
            CoreBackNoTargetAuthorityBeginOutcome::Begun => {
                host_trace.push(settlement_started);
                BackNoTargetAuthorityBeginOutcome::Begun
            }
            CoreBackNoTargetAuthorityBeginOutcome::ReceiverMismatch { owner, mismatch } => {
                let (settlement, host, _seal, _digest) = owner.into_parts();
                BackNoTargetAuthorityBeginOutcome::ReceiverMismatch {
                    owner: BackNoTargetAuthorityBeginOwner {
                        settlement: BackNoTargetAuthoritySettlementOwner { inner: settlement },
                        host: HostLease { inner: host },
                        manifest,
                    },
                    mismatch: PreparationReceiverMismatchView { inner: mismatch },
                }
            }
            CoreBackNoTargetAuthorityBeginOutcome::Retryable { owner, evidence } => {
                let (settlement, host, _seal, _digest) = owner.into_parts();
                let evidence = match evidence {
                    CoreBackNoTargetAuthorityBeginRetryEvidence::Capacity => {
                        BackNoTargetAuthorityBeginRetryEvidence::Capacity
                    }
                };
                BackNoTargetAuthorityBeginOutcome::Retryable {
                    owner: BackNoTargetAuthorityBeginOwner {
                        settlement: BackNoTargetAuthoritySettlementOwner { inner: settlement },
                        host: HostLease { inner: host },
                        manifest,
                    },
                    evidence,
                }
            }
            CoreBackNoTargetAuthorityBeginOutcome::Permanent { owner, evidence } => {
                let (settlement, host, _seal, _digest) = owner.into_parts();
                let evidence = match evidence {
                    CoreBackNoTargetAuthorityBeginPermanentEvidence::HostManifestContradiction => {
                        BackNoTargetAuthorityBeginPermanentEvidence::HostManifestContradiction
                    }
                    CoreBackNoTargetAuthorityBeginPermanentEvidence::PendingStateContradiction => {
                        BackNoTargetAuthorityBeginPermanentEvidence::PendingStateContradiction
                    }
                    CoreBackNoTargetAuthorityBeginPermanentEvidence::HostRetirementRejected => {
                        BackNoTargetAuthorityBeginPermanentEvidence::HostRetirementRejected
                    }
                };
                BackNoTargetAuthorityBeginOutcome::Permanent {
                    owner: BackNoTargetAuthorityBeginOwner {
                        settlement: BackNoTargetAuthoritySettlementOwner { inner: settlement },
                        host: HostLease { inner: host },
                        manifest,
                    },
                    evidence,
                }
            }
        }
    }

    #[doc(hidden)]
    pub fn complete_back_no_target_authority_cleanup<F>(
        &mut self,
        owner: BackNoTargetAuthorityCleanupCompletionOwner,
        commit_physical_and_clear_holder: F,
    ) -> BackNoTargetAuthorityCleanupCompletionOutcome
    where
        F: FnOnce(),
    {
        let version = self.inner.host().cloned();
        let slot = self.inner.top_slot();
        let inner = &mut self.inner;
        let host_trace = &mut self.host_trace;
        match inner.complete_back_no_target_authority_cleanup(
            owner.inner,
            commit_physical_and_clear_holder,
        ) {
            CoreBackNoTargetAuthorityCleanupCompletionOutcome::Terminal(inner) => {
                host_trace.push(HostTraceEvent::project(
                    HostTraceEventKind::SettlementCompleted,
                    version,
                    slot,
                ));
                BackNoTargetAuthorityCleanupCompletionOutcome::Terminal(
                    BackNoTargetAuthorityTerminalEvidence { inner },
                )
            }
            CoreBackNoTargetAuthorityCleanupCompletionOutcome::Mismatch { owner, evidence } => {
                let evidence = match evidence {
                    CoreBackNoTargetAuthorityCleanupMismatchEvidence::Receiver(inner) => {
                        BackNoTargetAuthorityCleanupMismatchEvidence::Receiver(
                            PreparationReceiverMismatchView { inner },
                        )
                    }
                    CoreBackNoTargetAuthorityCleanupMismatchEvidence::Facts(inner) => {
                        BackNoTargetAuthorityCleanupMismatchEvidence::Facts(
                            CoreBackNoTargetAuthorityCompletionFactsMismatchProjection { inner },
                        )
                    }
                };
                BackNoTargetAuthorityCleanupCompletionOutcome::Mismatch {
                    owner: BackNoTargetAuthorityCleanupCompletionOwner { inner: owner },
                    evidence,
                }
            }
        }
    }

    #[doc(hidden)]
    pub fn approve_back_no_target_logical_zero(
        &mut self,
        candidate: VerifiedBackNoTargetCandidate,
    ) -> BackNoTargetApprovalOutcome {
        match self.inner.approve_back_no_target_logical_zero(candidate) {
            CoreBackNoTargetApprovalOutcome::Approved(inner) => {
                let (candidate, approval) = inner.into_parts();
                BackNoTargetApprovalOutcome::Approved(ApprovedBackNoTargetCandidate {
                    candidate,
                    approval: ProvenEmptyApprovalView { inner: approval },
                })
            }
            CoreBackNoTargetApprovalOutcome::Mismatch {
                candidate,
                mismatch,
            } => BackNoTargetApprovalOutcome::Mismatch {
                candidate,
                mismatch: PreparationReceiverMismatchView { inner: mismatch },
            },
        }
    }

    #[doc(hidden)]
    pub fn accept_back_no_target_ready_with_canonical_zero_bind<F>(
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
        let ApprovedBackNoTargetCandidate {
            candidate,
            approval,
        } = approved;
        let approved =
            crate::engine::ApprovedBackNoTargetCandidate::from_parts(candidate, approval.inner);
        match self
            .inner
            .accept_back_no_target_ready_with_canonical_zero_bind(
                approved,
                manifest_digest,
                obligation_count,
                expected_completion_digest,
                commit_canonical_zero,
            ) {
            CoreBackNoTargetReadyCoreOutcome::Prepared(inner) => {
                BackNoTargetReadyCoreOutcome::Prepared(PreparedBackNoTargetPublication { inner })
            }
            CoreBackNoTargetReadyCoreOutcome::Mismatch { approved, mismatch } => {
                let (candidate, approval) = approved.into_parts();
                BackNoTargetReadyCoreOutcome::Mismatch {
                    approved: ApprovedBackNoTargetCandidate {
                        candidate,
                        approval: ProvenEmptyApprovalView { inner: approval },
                    },
                    mismatch: PreparationReceiverMismatchView { inner: mismatch },
                }
            }
        }
    }

    #[doc(hidden)]
    pub fn publish_prepared_back_no_target<F>(
        &mut self,
        prepared: PreparedBackNoTargetPublication,
        install_cleanup: F,
    ) where
        F: FnOnce(BackNoTargetCleanupInstallation),
    {
        self.inner
            .publish_prepared_back_no_target(prepared.inner, |installation| {
                let (batch, continuation, action) = installation.into_parts();
                install_cleanup(BackNoTargetCleanupInstallation {
                    batch: BackNoTargetCleanupBatchOwner { inner: batch },
                    continuation: RejectedBackContinuationOwner {
                        inner: continuation,
                    },
                    action: BackNoTargetCleanupAction { inner: action },
                });
            });
    }

    #[doc(hidden)]
    pub fn complete_back_no_target_cleanup<F>(
        &mut self,
        owner: BackNoTargetCleanupCompletionOwner,
        commit_reclaim: F,
    ) -> BackNoTargetCleanupCompletionOutcome
    where
        F: FnOnce(),
    {
        match self
            .inner
            .complete_back_no_target_cleanup(owner.inner, commit_reclaim)
        {
            CoreBackNoTargetCleanupCompletionCoreOutcome::Terminal(inner) => {
                BackNoTargetCleanupCompletionOutcome::Terminal(BackNoTargetTerminalEvidence {
                    inner,
                })
            }
            CoreBackNoTargetCleanupCompletionCoreOutcome::Mismatch { owner, mismatch } => {
                BackNoTargetCleanupCompletionOutcome::Mismatch {
                    owner: BackNoTargetCleanupCompletionOwner { inner: owner },
                    mismatch: PreparationReceiverMismatchView { inner: mismatch },
                }
            }
        }
    }

    #[doc(hidden)]
    pub fn settle_back_no_target_source_drift_with_retirement<F>(
        &mut self,
        owner: BackNoTargetSourceDriftOwner,
        commit_retirement: F,
    ) -> BackNoTargetSourceDriftSettlementOutcome
    where
        F: FnOnce(),
    {
        match self
            .inner
            .settle_back_no_target_source_drift_with_retirement(owner, commit_retirement)
        {
            CoreBackNoTargetSourceDriftSettlementOutcome::Settled(terminal) => {
                BackNoTargetSourceDriftSettlementOutcome::Settled(terminal)
            }
            CoreBackNoTargetSourceDriftSettlementOutcome::Mismatch { owner, evidence } => {
                BackNoTargetSourceDriftSettlementOutcome::Mismatch { owner, evidence }
            }
        }
    }

    #[doc(hidden)]
    pub fn fail_back_resolution_contradiction_with_retirement<F>(
        &mut self,
        owner: VerifiedBackResolutionContradiction,
        _commit_retirement: F,
    ) -> BackContradictionCoreOutcome
    where
        F: FnOnce(),
    {
        let evidence = owner.evidence();
        BackContradictionCoreOutcome::Mismatch { owner, evidence }
    }

    #[doc(hidden)]
    #[cfg(any(test, feature = "test-support"))]
    pub fn testing_exhaust_reservation_counter(&mut self, counter: TestingReservationCounter) {
        let counter = match counter {
            TestingReservationCounter::Request => crate::ids::ReservationCounter::Request,
            TestingReservationCounter::Transaction => crate::ids::ReservationCounter::Transaction,
            TestingReservationCounter::FrameTask => crate::ids::ReservationCounter::FrameTask,
            TestingReservationCounter::CandidateFrame => {
                crate::ids::ReservationCounter::CandidateFrame
            }
            TestingReservationCounter::CleanupBatch => crate::ids::ReservationCounter::CleanupBatch,
        };
        self.inner.testing_exhaust_reservation_counter(counter);
    }

    pub fn collect(&mut self) {
        self.inner.collect();
    }

    pub fn action(&mut self) -> Option<EngineAction> {
        self.inner.action().map(|action| match action {
            CoreEngineAction::FrameTask(inner) => EngineAction::FrameTask(FrameTask { inner }),
            CoreEngineAction::MountRequest(inner) => {
                EngineAction::MountRequest(MountRequest { inner })
            }
            CoreEngineAction::FailureCleanup(inner) => {
                EngineAction::FailureCleanup(FailureCleanupRequest { inner })
            }
            CoreEngineAction::Commit(inner) => EngineAction::Commit(CommitTicket { inner }),
            CoreEngineAction::BackResolution(inner) => EngineAction::BackResolution(inner),
        })
    }

    #[doc(hidden)]
    pub fn take_back_resolution_action(&mut self) -> Option<BackResolutionOwnerHandoff> {
        match self.inner.action() {
            Some(CoreEngineAction::BackResolution(handoff)) => Some(handoff),
            Some(other) => {
                self.inner.restore_action(other);
                None
            }
            None => None,
        }
    }

    pub fn report_applied_with_outlets(
        &mut self,
        task: FrameTask,
        outlets: Vec<RouteSlotKey>,
    ) -> Result<(), IntegrationError> {
        self.inner
            .accept_applied_report(task.inner.into_applied_report(outlets))
            .map_err(map_report_error)
    }

    pub fn report_applied(
        &mut self,
        task: RootFrameTask,
    ) -> Result<CommitTicket, IntegrationError> {
        self.inner
            .accept_applied_report(task.inner.into_legacy_applied_report())
            .map_err(map_report_error)?;
        match self.inner.action() {
            Some(CoreEngineAction::Commit(inner)) => Ok(CommitTicket { inner }),
            Some(other) => {
                self.inner.restore_action(other);
                Err(IntegrationError::Invalid)
            }
            None => Err(IntegrationError::Invalid),
        }
    }

    pub fn resolve_mount(
        &mut self,
        request: MountRequest,
    ) -> Result<MountGrant, ResolveMountError> {
        self.inner
            .resolve_mount(request.inner)
            .map(MountGrant::sealed)
            .map_err(|error| ResolveMountError {
                kind: map_mount_error(error.kind()),
                owner: match error.into_owner() {
                    CoreResolveMountOwner::Returned(inner) => {
                        ResolveMountOwner::Returned(MountRequest { inner })
                    }
                    CoreResolveMountOwner::Reissued => ResolveMountOwner::Reissued,
                    CoreResolveMountOwner::Consumed => ResolveMountOwner::Consumed,
                },
            })
    }

    #[doc(hidden)]
    pub fn reject_mount(
        &mut self,
        request: MountRequest,
        rejection: MountRejectionKind,
    ) -> Result<FailureCleanupRequest, RejectMountError> {
        self.inner
            .reject_mount(request.inner, rejection)
            .map(|inner| FailureCleanupRequest { inner })
            .map_err(|error| RejectMountError {
                kind: map_mount_error(error.kind()),
                owner: MountRequest {
                    inner: error.into_owner(),
                },
            })
    }

    #[doc(hidden)]
    pub fn reject_mount_after_mutation(
        &mut self,
        artifact: MountMutationArtifact,
    ) -> Result<FailureCleanupRequest, RejectMountMutationError> {
        let artifact = match artifact {
            MountMutationArtifact::Grant(grant) => CoreMountMutationArtifact::Grant(grant.inner),
            MountMutationArtifact::Receipt(receipt) => {
                CoreMountMutationArtifact::Receipt(receipt.inner)
            }
            MountMutationArtifact::RegistrationPermit(permit) => {
                CoreMountMutationArtifact::RegistrationPermit(permit.inner)
            }
        };
        self.inner
            .reject_mount_after_mutation(artifact)
            .map(|inner| FailureCleanupRequest { inner })
            .map_err(|error| RejectMountMutationError {
                kind: map_mount_error(error.kind()),
                owner: match error.into_owner() {
                    CoreMountMutationArtifact::Grant(inner) => {
                        MountMutationArtifact::Grant(MountGrant { inner })
                    }
                    CoreMountMutationArtifact::Receipt(inner) => {
                        MountMutationArtifact::Receipt(MountReceiptAttestation { inner })
                    }
                    CoreMountMutationArtifact::RegistrationPermit(inner) => {
                        MountMutationArtifact::RegistrationPermit(MountRegistrationPermit { inner })
                    }
                },
            })
    }

    pub fn accept_mount_receipt(
        &mut self,
        receipt: MountReceiptAttestation,
    ) -> Result<MountRegistrationPermit, AcceptMountReceiptError> {
        self.inner
            .accept_mount_receipt(receipt.inner)
            .map(MountRegistrationPermit::sealed)
            .map_err(|error| AcceptMountReceiptError {
                kind: map_mount_error(error.kind()),
                owner: MountReceiptAttestation {
                    inner: error.into_owner(),
                },
            })
    }

    pub fn begin_mount_registration<'engine>(
        &'engine mut self,
        permit: MountRegistrationPermit,
        proof: &MountReceiptProof,
    ) -> Result<MountRegistrationSession<'engine>, MountRegistrationBeginError> {
        let slot = match self
            .inner
            .validate_mount_registration(&permit.inner, &proof.inner)
        {
            Ok(slot) => slot,
            Err(error) => {
                return Err(MountRegistrationBeginError {
                    kind: map_mount_error(error),
                    permit: permit.inner,
                });
            }
        };
        Ok(MountRegistrationSession {
            engine: &mut self.inner,
            permit: permit.inner,
            slot,
        })
    }

    #[cfg(test)]
    fn complete_mount_registration(
        &mut self,
        completion: MountRegistrationCompletion,
    ) -> Result<(), IntegrationError> {
        self.inner
            .complete_mount_registration(completion.inner)
            .map_err(map_mount_error)
    }

    #[doc(hidden)]
    #[cfg(any(test, feature = "test-support"))]
    pub fn testing_replay_mount_completion(
        &mut self,
        replays: &MountRegistrationReplays,
    ) -> Result<(), IntegrationError> {
        self.inner
            .complete_mount_registration(replays.completion.clone())
            .map_err(map_mount_error)
    }

    #[doc(hidden)]
    #[cfg(any(test, feature = "test-support"))]
    pub fn testing_replay_mount_confirmation(
        &mut self,
        replays: &MountRegistrationReplays,
    ) -> Result<(), IntegrationError> {
        self.inner
            .consume_mount_confirmation(replays.confirmation.clone())
            .map_err(map_mount_error)
    }

    pub fn validate_report_identity(
        &mut self,
        task: &RootFrameTask,
    ) -> Result<(), IntegrationError> {
        self.inner
            .validate_view_report_identity(task.inner.identity())
            .map_err(map_report_error)
    }

    #[doc(hidden)]
    pub fn inspect_report_identity(&self, task: &RootFrameTask) -> Result<(), IntegrationError> {
        self.inner
            .inspect_view_report_identity(task.inner.identity())
            .map_err(map_report_error)
    }

    #[doc(hidden)]
    pub fn is_current_root_back_frame_task(&self, task: &RootFrameTask) -> bool {
        self.inner
            .is_current_root_back_frame_task(task.inner.identity())
    }

    #[doc(hidden)]
    pub fn expected_child_outlet(
        &self,
        task: &RootFrameTask,
    ) -> Result<Option<RouteSlotKey>, IntegrationError> {
        self.inner
            .expected_child_outlet(task.inner.identity())
            .map_err(map_report_error)
    }

    #[doc(hidden)]
    pub fn validate_mount_request(&self, request: &MountRequest) -> Result<(), IntegrationError> {
        self.inner
            .validate_mount_request(&request.inner)
            .map_err(map_mount_error)
    }

    #[cfg(feature = "test-support")]
    #[doc(hidden)]
    pub fn testing_exhaust_slot_counter(&mut self) {
        self.inner.testing_exhaust_slot_counter();
    }

    #[cfg(any(test, feature = "test-support"))]
    #[doc(hidden)]
    pub fn testing_exhaust_mount_sequence_counter(&mut self) {
        self.inner.testing_exhaust_mount_sequence_counter();
    }

    #[cfg(feature = "test-support")]
    #[doc(hidden)]
    pub fn testing_set_mount_receipt_context_live(&mut self, live: bool) {
        self.inner.testing_set_mount_receipt_context_live(live);
    }

    pub fn report_failed(
        &mut self,
        task: RootFrameTask,
        kind: ViewFailureKind,
        message: &'static str,
    ) -> Result<Option<FailureCleanupRequest>, IntegrationError> {
        let kind = match kind {
            ViewFailureKind::Build => NavigationFailureKind::Build,
            ViewFailureKind::Apply => NavigationFailureKind::Apply,
        };
        self.inner
            .begin_view_report(task.inner.identity())
            .map(|session| {
                session
                    .failed(NavigationFailure::new(kind, message))
                    .map(|inner| FailureCleanupRequest { inner })
            })
            .map_err(map_report_error)
    }

    #[doc(hidden)]
    pub fn report_failed_with_physical_residual(
        &mut self,
        task: RootFrameTask,
        kind: ViewFailureKind,
        message: &'static str,
    ) -> Result<FailureCleanupRequest, IntegrationError> {
        let kind = match kind {
            ViewFailureKind::Build => NavigationFailureKind::Build,
            ViewFailureKind::Apply => NavigationFailureKind::Apply,
        };
        self.inner
            .begin_view_report(task.inner.identity())
            .map(|session| FailureCleanupRequest {
                inner: session.failed_with_physical_residual(NavigationFailure::new(kind, message)),
            })
            .map_err(map_report_error)
    }

    #[cfg(test)]
    #[doc(hidden)]
    pub fn prepare_failure_cleanup(
        &mut self,
        request: FailureCleanupRequest,
        manifest: CleanupManifest,
    ) -> Result<CleanupBatch, IntegrationError> {
        match self.prepare_failure_cleanup_retaining(request, manifest) {
            FailureCleanupPrepareOutcome::Prepared(batch) => Ok(batch),
            FailureCleanupPrepareOutcome::Mismatch(mismatch) => Err(mismatch.evidence.kind()),
            FailureCleanupPrepareOutcome::Permanent(_) => Err(IntegrationError::InvalidManifest),
        }
    }

    #[doc(hidden)]
    pub fn prepare_failure_cleanup_retaining(
        &mut self,
        request: FailureCleanupRequest,
        manifest: CleanupManifest,
    ) -> FailureCleanupPrepareOutcome {
        let manifest_digest = manifest.manifest_digest;
        let obligation_count = manifest.obligation_count;
        match self.inner.prepare_failure_cleanup_retaining(
            request.inner,
            CleanupManifestSeal::new(
                manifest.logical_roots,
                CleanupManifestDigest::from_opaque(manifest_digest),
                obligation_count,
            ),
        ) {
            CoreFailureCleanupPrepareOutcome::Prepared(owner) => {
                FailureCleanupPrepareOutcome::Prepared(CleanupBatch {
                    inner: owner.into_token(),
                    manifest_digest,
                    obligation_count,
                })
            }
            CoreFailureCleanupPrepareOutcome::Mismatch { request, evidence } => {
                FailureCleanupPrepareOutcome::Mismatch(FailureCleanupPrepareMismatch {
                    request: FailureCleanupRequest { inner: request },
                    evidence: FailureCleanupReceiverMismatchView { inner: evidence },
                })
            }
            CoreFailureCleanupPrepareOutcome::Permanent { request, evidence } => {
                FailureCleanupPrepareOutcome::Permanent(PermanentFailureCleanup {
                    request: FailureCleanupRequest { inner: request },
                    evidence: FailureCleanupManifestContradictionView { inner: evidence },
                })
            }
        }
    }

    #[doc(hidden)]
    pub fn bind_failure_cleanup_physical<Physical>(
        &self,
        request: &FailureCleanupRequest,
        observed: CleanupManifest,
        physical: Physical,
    ) -> FailureCleanupPhysicalBinding<Physical> {
        FailureCleanupPhysicalBinding {
            physical,
            observed: request.inner.freeze_zero_physical_projection(
                CleanupManifestDigest::from_opaque(observed.manifest_digest),
                observed.obligation_count,
            ),
        }
    }

    #[doc(hidden)]
    pub fn prepare_failure_cleanup_with_physical<Physical>(
        &mut self,
        request: FailureCleanupRequest,
        manifest: CleanupManifest,
        physical: FailureCleanupPhysicalBinding<Physical>,
    ) -> FailureCleanupPhysicalPrepareOutcome<Physical> {
        if let Some(evidence) = request.inner.physical_receiver_mismatch(&physical.observed) {
            return FailureCleanupPhysicalPrepareOutcome::Mismatch(
                FailureCleanupPhysicalPrepareMismatch {
                    request,
                    evidence: FailureCleanupReceiverMismatchView { inner: evidence },
                    physical,
                },
            );
        }
        let manifest_digest = manifest.manifest_digest;
        let obligation_count = manifest.obligation_count;
        match self.inner.prepare_failure_cleanup_retaining(
            request.inner,
            CleanupManifestSeal::new(
                manifest.logical_roots,
                CleanupManifestDigest::from_opaque(manifest_digest),
                obligation_count,
            ),
        ) {
            CoreFailureCleanupPrepareOutcome::Prepared(owner) => {
                FailureCleanupPhysicalPrepareOutcome::Prepared(PreparedFailureCleanupPhysical {
                    batch: CleanupBatch {
                        inner: owner.into_token(),
                        manifest_digest,
                        obligation_count,
                    },
                    physical: physical.physical,
                })
            }
            CoreFailureCleanupPrepareOutcome::Mismatch { request, evidence } => {
                FailureCleanupPhysicalPrepareOutcome::Mismatch(
                    FailureCleanupPhysicalPrepareMismatch {
                        request: FailureCleanupRequest { inner: request },
                        evidence: FailureCleanupReceiverMismatchView { inner: evidence },
                        physical,
                    },
                )
            }
            CoreFailureCleanupPrepareOutcome::Permanent { request, evidence } => {
                FailureCleanupPhysicalPrepareOutcome::Permanent(FailureCleanupPermanentOwner {
                    inner: CoreFailureCleanupPermanentHolder {
                        request,
                        physical,
                        evidence,
                    },
                })
            }
        }
    }

    #[doc(hidden)]
    pub fn classify_failure_cleanup_permanent<Physical>(
        &mut self,
        holder: FailureCleanupPermanentOwner<Physical>,
    ) -> FailureCleanupPermanentOutcome<Physical> {
        match self
            .inner
            .classify_failure_cleanup_permanent(holder.inner, |physical, expected| {
                let observed = physical.observed.clone();
                CorePreparedFailureCleanupZeroPhysical::bind_zero(physical, expected, observed)
            }) {
            CoreFailureCleanupPermanentOutcome::AuthoritySettlement { physical, owner } => {
                FailureCleanupPermanentOutcome::AuthoritySettlement {
                    physical: FailureCleanupPermanentAuthorityPhysical { inner: physical },
                    owner: FailureCleanupPermanentAuthorityOwner { inner: owner },
                }
            }
            CoreFailureCleanupPermanentOutcome::ProvenZero { zero, owner } => {
                FailureCleanupPermanentOutcome::ProvenZero {
                    zero: PreparedFailureCleanupZeroPhysical { inner: zero },
                    owner: FailureCleanupZeroTerminalOwner { inner: owner },
                }
            }
            CoreFailureCleanupPermanentOutcome::ZeroMismatch { holder, evidence } => {
                FailureCleanupPermanentOutcome::ZeroMismatch {
                    holder: FailureCleanupPermanentOwner { inner: holder },
                    evidence: FailureCleanupZeroPhysicalMismatchView { inner: evidence },
                }
            }
            CoreFailureCleanupPermanentOutcome::Mismatch(holder) => {
                FailureCleanupPermanentOutcome::Mismatch(FailureCleanupPermanentOwner {
                    inner: holder,
                })
            }
        }
    }

    #[doc(hidden)]
    pub fn complete_failure_cleanup_zero_terminal<Physical, ConsumePhysical>(
        &mut self,
        owner: FailureCleanupZeroTerminalOwner,
        zero: PreparedFailureCleanupZeroPhysical<Physical>,
        consume_physical: ConsumePhysical,
    ) -> FailureCleanupZeroTerminalOutcome<Physical>
    where
        ConsumePhysical: FnOnce(Physical),
    {
        match self
            .inner
            .complete_failure_cleanup_zero_terminal(owner.inner, zero.inner, |zero| {
                consume_physical(zero.into_physical().physical)
            }) {
            CoreFailureCleanupZeroTerminalOutcome::Terminal(_terminal) => {
                FailureCleanupZeroTerminalOutcome::Terminal
            }
            CoreFailureCleanupZeroTerminalOutcome::Mismatch {
                zero,
                owner,
                evidence,
            } => FailureCleanupZeroTerminalOutcome::Mismatch {
                zero: PreparedFailureCleanupZeroPhysical { inner: zero },
                owner: FailureCleanupZeroTerminalOwner { inner: owner },
                evidence: FailureCleanupZeroTerminalMismatchView { inner: evidence },
            },
        }
    }

    pub fn prepare_commit(
        &mut self,
        ticket: &CommitTicket,
        manifest: CleanupManifest,
    ) -> Result<PreparedCommit, IntegrationError> {
        let manifest_digest = manifest.manifest_digest;
        let obligation_count = manifest.obligation_count;
        let sealed = self
            .inner
            .seal_commit(
                &ticket.inner,
                CleanupManifestSeal::new(
                    manifest.logical_roots,
                    CleanupManifestDigest::from_opaque(manifest_digest),
                    obligation_count,
                ),
            )
            .map_err(map_commit_error)?;
        let cleanup = CleanupBatch {
            inner: sealed.cleanup().clone(),
            manifest_digest,
            obligation_count,
        };
        let publication = self
            .inner
            .prepare_publication(sealed)
            .map_err(map_commit_error)?;
        Ok(PreparedCommit {
            publication,
            cleanup,
        })
    }

    fn accept_back_target_capacity<Accepted>(
        &mut self,
        capacity: LifecycleContinuationCapacity,
        scope: PreparationBoundaryScopeView,
        matched: impl FnOnce(PreparedPostCommitContinuation) -> Accepted,
    ) -> BackTargetPreparationAcceptance<Accepted> {
        let exact_projection = capacity.inner.scope == scope.inner;
        if !exact_projection {
            let mismatch = PreparationReceiverMismatchView {
                inner: CorePreparationReceiverMismatchProjection::Scope {
                    expected: capacity.inner.scope.clone(),
                    observed: scope.inner,
                },
            };
            return BackTargetPreparationAcceptance::Mismatch { capacity, mismatch };
        }
        if let Err(inner) = self
            .inner
            .inspect_back_target_continuation(&capacity.inner.scope, None)
        {
            return BackTargetPreparationAcceptance::Mismatch {
                capacity,
                mismatch: PreparationReceiverMismatchView { inner },
            };
        }
        BackTargetPreparationAcceptance::Matched(matched(PreparedPostCommitContinuation {
            binding: capacity.inner,
        }))
    }

    #[doc(hidden)]
    pub fn validate_back_target_execution_pairing(
        &self,
        owner: BackTargetExecutionOwnerRef<'_>,
        physical: BackTargetPhysicalOwnerRef<'_>,
    ) -> BackTargetExecutionPairingOutcome {
        let (history, scope) = match owner {
            BackTargetExecutionOwnerRef::Driving {
                history,
                continuation,
            }
            | BackTargetExecutionOwnerRef::Classifying {
                history,
                continuation,
            } => (history, &continuation.inner.scope),
            BackTargetExecutionOwnerRef::Prepared {
                history,
                continuation,
            } => (history, &continuation.inner.binding.scope),
        };
        let physical = match physical {
            BackTargetPhysicalOwnerRef::ResumeAction(action) => match action {
                BackTargetResumeAction::FrameTask(task) => {
                    CoreBackTargetPhysicalReceiverRef::FrameTask(&task.inner)
                }
                BackTargetResumeAction::MountRequest(request) => {
                    CoreBackTargetPhysicalReceiverRef::MountRequest(&request.inner)
                }
                BackTargetResumeAction::Commit(ticket) => {
                    CoreBackTargetPhysicalReceiverRef::Commit(&ticket.inner)
                }
            },
            BackTargetPhysicalOwnerRef::FrameTask(task) => {
                CoreBackTargetPhysicalReceiverRef::FrameTask(&task.inner)
            }
            BackTargetPhysicalOwnerRef::MountRequest(request) => {
                CoreBackTargetPhysicalReceiverRef::MountRequest(&request.inner)
            }
            BackTargetPhysicalOwnerRef::CommitTicket(ticket) => {
                CoreBackTargetPhysicalReceiverRef::Commit(&ticket.inner)
            }
        };
        match self
            .inner
            .inspect_back_target_execution_pairing(scope, &history.inner, physical)
        {
            Ok(()) => BackTargetExecutionPairingOutcome::Matched,
            Err(inner) => {
                BackTargetExecutionPairingOutcome::Mismatch(PreparationReceiverMismatchView {
                    inner,
                })
            }
        }
    }

    #[doc(hidden)]
    pub fn report_back_target_failed(
        &mut self,
        history: &HistoryExecutionBindingView,
        capacity: LifecycleContinuationCapacity,
        task: RootFrameTask,
        kind: ViewFailureKind,
        message: &'static str,
        mode: BackTargetDriverFailureReportMode,
    ) -> BackTargetDriverFailureReportOutcome {
        let failure_kind = match kind {
            ViewFailureKind::Build => NavigationFailureKind::Build,
            ViewFailureKind::Apply => NavigationFailureKind::Apply,
        };
        let core_mode = match mode {
            BackTargetDriverFailureReportMode::Normal => {
                CoreBackTargetDriverFailureReportMode::Normal
            }
            BackTargetDriverFailureReportMode::PhysicalResidual => {
                CoreBackTargetDriverFailureReportMode::PhysicalResidual
            }
        };
        let scope = capacity.inner.scope.clone();
        let reported = self.inner.report_back_target_failed(
            scope.clone(),
            &history.inner,
            &task.inner,
            NavigationFailure::new(failure_kind, message),
            core_mode,
        );
        let reported = match reported {
            Ok(reported) => reported,
            Err(inner) => {
                return BackTargetDriverFailureReportOutcome::Mismatch {
                    capacity,
                    task,
                    mismatch: PreparationReceiverMismatchView { inner },
                };
            }
        };
        drop(capacity);
        drop(task);
        let outcome = match reported {
            CoreBackTargetDriverFailureReport::FailureCleanup(inner) => {
                BackTargetDriverFailureCoreOutcome::FailureCleanup(
                    BackTargetDriverFailureCleanupOwner { inner },
                )
            }
            CoreBackTargetDriverFailureReport::Fallback(inner) => {
                BackTargetDriverFailureCoreOutcome::Fallback(BackTargetDriverFallbackOwner {
                    inner,
                })
            }
            CoreBackTargetDriverFailureReport::Terminal(inner) => {
                BackTargetDriverFailureCoreOutcome::Terminal(BackTargetDriverTerminalOwner {
                    inner,
                })
            }
        };
        BackTargetDriverFailureReportOutcome::Reported(outcome)
    }

    #[doc(hidden)]
    pub fn consume_back_target_driver_failure(
        &mut self,
        owner: BackTargetDriverFailureCoreOutcome,
    ) -> BackTargetDriverFailureConsumeOutcome {
        let mismatch = match &owner {
            BackTargetDriverFailureCoreOutcome::FailureCleanup(owner) => self
                .inner
                .inspect_back_target_driver_failure_cleanup(&owner.inner)
                .err(),
            BackTargetDriverFailureCoreOutcome::Fallback(owner) => self
                .inner
                .inspect_back_target_driver_failure_fallback(&owner.inner)
                .err(),
            BackTargetDriverFailureCoreOutcome::Terminal(owner) => self
                .inner
                .inspect_back_target_driver_failure_terminal(&owner.inner)
                .err(),
        };
        if let Some(inner) = mismatch {
            return BackTargetDriverFailureConsumeOutcome::Mismatch {
                mismatch: PreparationReceiverMismatchView { inner },
                owner,
            };
        }

        match owner {
            BackTargetDriverFailureCoreOutcome::FailureCleanup(owner) => {
                BackTargetDriverFailureConsumeOutcome::FailureCleanup(FailureCleanupRequest {
                    inner: owner.inner.request,
                })
            }
            BackTargetDriverFailureCoreOutcome::Fallback(owner) => {
                drop(owner);
                BackTargetDriverFailureConsumeOutcome::FallbackReleased
            }
            BackTargetDriverFailureCoreOutcome::Terminal(owner) => {
                BackTargetDriverFailureConsumeOutcome::Terminal(BackTargetDriverTerminalEvidence {
                    inner: CoreBackTargetDriverTerminalProjection {
                        scope: owner.inner.scope,
                        request: owner.inner.request,
                    },
                })
            }
        }
    }

    #[doc(hidden)]
    pub fn report_back_target_applied(
        &mut self,
        history: &HistoryExecutionBindingView,
        capacity: LifecycleContinuationCapacity,
        task: RootFrameTask,
        report: BackTargetAppliedReport,
    ) -> BackTargetAppliedReportOutcome {
        let scope = capacity.inner.scope.clone();
        let outlets = match &report {
            BackTargetAppliedReport::LegacyLeaf => None,
            BackTargetAppliedReport::WithOutlets(outlets) => Some(outlets.clone()),
        };
        let outcome =
            match self
                .inner
                .report_back_target_applied(scope, &history.inner, &task.inner, outlets)
            {
                Ok(outcome) => outcome,
                Err(inner) => {
                    return BackTargetAppliedReportOutcome::Mismatch {
                        capacity,
                        task,
                        report,
                        mismatch: PreparationReceiverMismatchView { inner },
                    };
                }
            };
        drop(report);
        drop(task);
        match outcome {
            CoreBackTargetAppliedOutcome::MountRequest(inner) => {
                BackTargetAppliedReportOutcome::MountRequest {
                    capacity,
                    request: MountRequest { inner },
                }
            }
            CoreBackTargetAppliedOutcome::Commit(inner) => BackTargetAppliedReportOutcome::Commit {
                capacity,
                ticket: CommitTicket { inner },
            },
            CoreBackTargetAppliedOutcome::FailureCleanup(inner) => {
                drop(capacity);
                BackTargetAppliedReportOutcome::FailureCleanup(
                    BackTargetDriverFailureCleanupOwner { inner },
                )
            }
        }
    }

    #[doc(hidden)]
    pub fn accept_back_target_ready(
        &mut self,
        capacity: LifecycleContinuationCapacity,
        scope: PreparationBoundaryScopeView,
    ) -> BackTargetPreparationAcceptance<PreparedPostCommitContinuationOwner> {
        self.accept_back_target_capacity(capacity, scope, |inner| {
            PreparedPostCommitContinuationOwner { inner }
        })
    }

    #[doc(hidden)]
    pub fn accept_back_target_pending_rejected(
        &mut self,
        capacity: LifecycleContinuationCapacity,
        scope: PreparationBoundaryScopeView,
    ) -> BackTargetPreparationAcceptance<RouteFailureContinuationOwner> {
        self.accept_back_target_capacity(capacity, scope, |inner| RouteFailureContinuationOwner {
            inner,
        })
    }

    #[doc(hidden)]
    pub fn consume_back_target_pending_rejected(
        &mut self,
        owner: RouteFailureContinuationOwner,
        ticket: CommitTicket,
    ) -> BackTargetPendingRejectedConsumeOutcome {
        let RouteFailureContinuationOwner {
            inner:
                PreparedPostCommitContinuation {
                    binding: LifecycleContinuationBinding { scope },
                },
        } = owner;
        match self
            .inner
            .consume_back_target_pending_rejected(scope, ticket.inner)
        {
            Ok(inner) => {
                BackTargetPendingRejectedConsumeOutcome::FailureCleanup(FailureCleanupRequest {
                    inner,
                })
            }
            Err((scope, ticket, inner)) => BackTargetPendingRejectedConsumeOutcome::Mismatch {
                owner: RouteFailureContinuationOwner {
                    inner: PreparedPostCommitContinuation {
                        binding: LifecycleContinuationBinding { scope },
                    },
                },
                ticket: CommitTicket { inner: ticket },
                mismatch: PreparationReceiverMismatchView { inner },
            },
        }
    }

    #[doc(hidden)]
    pub fn accept_back_target_authority_lost(
        &mut self,
        capacity: LifecycleContinuationCapacity,
        scope: PreparationBoundaryScopeView,
        manifest: &CleanupManifest,
    ) -> BackTargetPreparationAcceptance<TargetAuthoritySettlementOwner> {
        match self.accept_back_target_capacity(capacity, scope, |inner| inner) {
            BackTargetPreparationAcceptance::Matched(inner) => {
                let PreparedPostCommitContinuation {
                    binding: LifecycleContinuationBinding { scope },
                } = inner;
                BackTargetPreparationAcceptance::Matched(TargetAuthoritySettlementOwner {
                    inner: self.inner.prepare_back_target_permanent_settlement_owner(
                        scope,
                        manifest.logical_roots.clone(),
                    ),
                })
            }
            BackTargetPreparationAcceptance::Mismatch { capacity, mismatch } => {
                BackTargetPreparationAcceptance::Mismatch { capacity, mismatch }
            }
        }
    }

    #[doc(hidden)]
    pub fn begin_back_target_permanent_cleanup<Installation, F>(
        &mut self,
        owner: BackTargetPermanentBeginOwner,
        ticket: &CommitTicket,
        installation: Installation,
        install: F,
    ) -> BackTargetPermanentBeginOutcome<Installation>
    where
        F: FnOnce(Installation, BackTargetPermanentCleanupBatchOwner),
    {
        let BackTargetPermanentBeginOwner {
            settlement,
            host,
            manifest,
        } = owner;
        let core_owner = CoreBackTargetPermanentBeginOwner::new(settlement.inner, host.inner);
        let manifest_digest = manifest.manifest_digest;
        let obligation_count = manifest.obligation_count;
        let logical_roots = manifest.logical_roots.clone();
        match self.inner.begin_back_target_permanent_cleanup(
            &mut self.admission,
            core_owner,
            &ticket.inner,
            logical_roots,
            manifest_digest,
            obligation_count,
            installation,
            |installation, inner| {
                install(installation, BackTargetPermanentCleanupBatchOwner { inner })
            },
        ) {
            CoreBackTargetPermanentBeginOutcome::Begun => BackTargetPermanentBeginOutcome::Begun,
            CoreBackTargetPermanentBeginOutcome::ReceiverMismatch {
                owner,
                installation,
                mismatch,
            } => {
                let (settlement, host) = owner.into_parts();
                BackTargetPermanentBeginOutcome::ReceiverMismatch {
                    owner: BackTargetPermanentBeginOwner {
                        settlement: TargetAuthoritySettlementOwner { inner: settlement },
                        host: HostLease { inner: host },
                        manifest,
                    },
                    installation,
                    mismatch: PreparationReceiverMismatchView { inner: mismatch },
                }
            }
            CoreBackTargetPermanentBeginOutcome::LogicalRootsMismatch {
                owner,
                installation,
            } => {
                let (settlement, host) = owner.into_parts();
                BackTargetPermanentBeginOutcome::LogicalRootsMismatch {
                    owner: BackTargetPermanentBeginOwner {
                        settlement: TargetAuthoritySettlementOwner { inner: settlement },
                        host: HostLease { inner: host },
                        manifest,
                    },
                    installation,
                }
            }
            CoreBackTargetPermanentBeginOutcome::Permanent {
                owner,
                installation,
                evidence: inner,
            } => {
                let (settlement, host) = owner.into_parts();
                let evidence = match inner {
                    BackTargetPermanentBeginCorePermanentEvidence::CurrentStateContradiction {
                        expected,
                        observed,
                    } => BackTargetPermanentBeginPermanentEvidence::CurrentStateContradiction(
                        BackTargetPermanentCurrentStateContradictionView {
                            expected: BackTargetPermanentCurrentStateProjectionView {
                                inner: expected,
                            },
                            observed: BackTargetPermanentCurrentStateProjectionView {
                                inner: observed,
                            },
                        },
                    ),
                    BackTargetPermanentBeginCorePermanentEvidence::CanonicalRelationContradiction {
                        expected,
                        observed,
                    } => BackTargetPermanentBeginPermanentEvidence::CanonicalRelationContradiction(
                        BackTargetPermanentCanonicalRelationContradictionView {
                            expected,
                            observed,
                        },
                    ),
                    BackTargetPermanentBeginCorePermanentEvidence::RequestRecordContradiction {
                        expected,
                        observed,
                    } => BackTargetPermanentBeginPermanentEvidence::RequestRecordContradiction(
                        BackTargetPermanentRequestRecordContradictionView {
                            expected,
                            observed,
                        },
                    ),
                    BackTargetPermanentBeginCorePermanentEvidence::TerminalPlanContradiction {
                        source,
                        expected,
                        observed,
                    } => BackTargetPermanentBeginPermanentEvidence::TerminalPlanContradiction(
                        BackTargetPermanentTerminalPlanContradictionView {
                            source: BackTargetPermanentTerminalSourceProjectionView {
                                inner: source,
                            },
                            expected: BackTargetPermanentTerminalExpectedProjectionView {
                                inner: expected,
                            },
                            observed: BackTargetPermanentTerminalObservedProjectionView {
                                inner: observed,
                            },
                        },
                    ),
                    BackTargetPermanentBeginCorePermanentEvidence::HostRetirementRejected => {
                        BackTargetPermanentBeginPermanentEvidence::HostRetirementRejected
                    }
                };
                BackTargetPermanentBeginOutcome::Permanent {
                    owner: BackTargetPermanentBeginOwner {
                        settlement: TargetAuthoritySettlementOwner { inner: settlement },
                        host: HostLease { inner: host },
                        manifest,
                    },
                    installation,
                    evidence,
                }
            }
        }
    }

    #[doc(hidden)]
    pub fn complete_back_target_permanent_cleanup<F>(
        &mut self,
        owner: BackTargetPermanentCleanupCompletionOwner,
        commit_completion: F,
    ) -> BackTargetPermanentCleanupCompletionOutcome
    where
        F: FnOnce(),
    {
        match self
            .inner
            .complete_back_target_permanent_cleanup(owner.inner, commit_completion)
        {
            CoreBackTargetPermanentCleanupCompletionOutcome::Terminal(inner) => {
                BackTargetPermanentCleanupCompletionOutcome::Terminal(
                    BackTargetPermanentCleanupTerminalEvidence { inner },
                )
            }
            CoreBackTargetPermanentCleanupCompletionOutcome::Mismatch { owner, mismatch } => {
                let mismatch = match mismatch {
                    CoreBackTargetPermanentCleanupMismatchEvidence::Receiver(inner) => {
                        BackTargetPermanentCleanupMismatchEvidence::Receiver(
                            BackTargetPermanentCleanupReceiverMismatchView { inner },
                        )
                    }
                    CoreBackTargetPermanentCleanupMismatchEvidence::Facts(inner) => {
                        BackTargetPermanentCleanupMismatchEvidence::Facts(
                            BackTargetPermanentCompletionFactsMismatchView { inner },
                        )
                    }
                };
                BackTargetPermanentCleanupCompletionOutcome::Mismatch {
                    owner: BackTargetPermanentCleanupCompletionOwner { inner: owner },
                    mismatch,
                }
            }
        }
    }

    #[doc(hidden)]
    pub fn prepare_back_target_commit<'a>(
        &'a mut self,
        owner: PreparedPostCommitContinuationOwner,
        ticket: &CommitTicket,
        manifest: CleanupManifest,
    ) -> BackTargetCommitPreparationOutcome<'a> {
        if let Err(inner) = self
            .inner
            .inspect_back_target_continuation(&owner.inner.binding.scope, Some(&ticket.inner))
        {
            return BackTargetCommitPreparationOutcome::Mismatch {
                owner,
                manifest,
                mismatch: PreparationReceiverMismatchView { inner },
            };
        }
        let manifest_digest = manifest.manifest_digest;
        let obligation_count = manifest.obligation_count;
        let seal = CleanupManifestSeal::new(
            manifest.logical_roots.clone(),
            CleanupManifestDigest::from_opaque(manifest_digest),
            obligation_count,
        );
        let PreparedPostCommitContinuationOwner {
            inner:
                PreparedPostCommitContinuation {
                    binding: LifecycleContinuationBinding { scope },
                },
        } = owner;
        match self
            .inner
            .prepare_back_target_commit(scope, &ticket.inner, seal)
        {
            CoreBackTargetCommitOperationOutcome::Validated {
                publication,
                cleanup,
                continuation,
            } => BackTargetCommitPreparationOutcome::Validated(ValidatedCommit {
                publication,
                cleanup: CleanupBatch {
                    inner: cleanup,
                    manifest_digest,
                    obligation_count,
                },
                back_target_continuation: Some(PreparedPostCommitContinuation {
                    binding: LifecycleContinuationBinding {
                        scope: continuation,
                    },
                }),
            }),
            CoreBackTargetCommitOperationOutcome::Failed(failure) => {
                let failure = match failure {
                    CoreBackTargetCommitOperationFailure::RetainedInput {
                        continuation,
                        evidence,
                    } => BackTargetCommitPreparationFailure::RetainedInput(
                        BackTargetCommitRetainedInputOwner {
                            continuation: PreparedPostCommitContinuationOwner {
                                inner: PreparedPostCommitContinuation {
                                    binding: LifecycleContinuationBinding {
                                        scope: continuation,
                                    },
                                },
                            },
                            manifest,
                            evidence,
                        },
                    ),
                    CoreBackTargetCommitOperationFailure::FailureCleanup(inner) => {
                        drop(manifest);
                        BackTargetCommitPreparationFailure::FailureCleanup(
                            BackTargetCommitFailureCleanupOwner { inner },
                        )
                    }
                    CoreBackTargetCommitOperationFailure::RejectedPublication(inner) => {
                        drop(manifest);
                        BackTargetCommitPreparationFailure::RejectedPublication(
                            BackTargetCommitRejectedPublicationOwner { inner },
                        )
                    }
                };
                BackTargetCommitPreparationOutcome::PreparationFailed(failure)
            }
        }
    }

    #[doc(hidden)]
    pub fn inspect_commit_identity(&self, ticket: &CommitTicket) -> Result<(), IntegrationError> {
        self.inner
            .inspect_commit_identity(&ticket.inner)
            .map_err(map_commit_error)
    }

    #[doc(hidden)]
    #[cfg(any(test, feature = "test-support"))]
    pub fn testing_invalidate_commit_action(&mut self) {
        self.inner.testing_invalidate_commit_action();
    }

    #[doc(hidden)]
    #[cfg(any(test, feature = "test-support"))]
    pub fn testing_drift_current_frame_task_identity(&mut self) -> bool {
        self.inner.testing_drift_current_frame_task_identity()
    }

    #[doc(hidden)]
    #[cfg(any(test, feature = "test-support"))]
    pub fn testing_remove_back_target_permanent_current_reconciliation(&mut self) -> bool {
        self.inner
            .testing_remove_back_target_permanent_current_reconciliation()
    }

    #[doc(hidden)]
    #[cfg(any(test, feature = "test-support"))]
    pub fn testing_remove_back_target_permanent_active_branch(&mut self) -> bool {
        self.inner
            .testing_remove_back_target_permanent_active_branch()
    }

    #[doc(hidden)]
    #[cfg(any(test, feature = "test-support"))]
    pub fn testing_back_target_permanent_current_state(
        &self,
    ) -> BackTargetPermanentCurrentStateProjectionView {
        BackTargetPermanentCurrentStateProjectionView {
            inner: self
                .inner
                .testing_back_target_permanent_current_state_projection(),
        }
    }

    #[doc(hidden)]
    #[cfg(any(test, feature = "test-support"))]
    pub fn testing_back_target_permanent_retired_host_matches(
        &self,
        version: &HostVersion,
        top_slot: RouteSlotId,
    ) -> bool {
        self.inner
            .testing_back_target_permanent_retired_host_matches(version, top_slot)
    }

    #[doc(hidden)]
    #[cfg(any(test, feature = "test-support"))]
    pub fn testing_transition_back_target_record_to_matching(
        &mut self,
        request: NavigationRequestId,
    ) -> bool {
        self.inner
            .testing_transition_back_target_record_to_matching(request)
    }

    #[doc(hidden)]
    #[cfg(any(test, feature = "test-support"))]
    pub fn testing_close_back_target_request_trace(
        &mut self,
        request: NavigationRequestId,
    ) -> bool {
        self.inner.testing_close_back_target_request_trace(request)
    }

    #[doc(hidden)]
    #[cfg(any(test, feature = "test-support"))]
    pub fn testing_back_target_permanent_begin_state(&self) -> BackTargetPermanentBeginStateView {
        BackTargetPermanentBeginStateView {
            inner: self.inner.testing_back_target_permanent_begin_projection(),
        }
    }

    pub fn validate_publication(
        &mut self,
        prepared: PreparedCommit,
    ) -> Result<ValidatedCommit<'_>, IntegrationError> {
        let PreparedCommit {
            publication,
            cleanup,
        } = prepared;
        let publication = self
            .inner
            .validate_publication(publication)
            .map_err(map_commit_error)?;
        Ok(ValidatedCommit {
            publication,
            cleanup,
            back_target_continuation: None,
        })
    }

    #[doc(hidden)]
    pub fn cancel_prepared_commit_for_authority_settlement(
        &mut self,
        prepared: PreparedCommit,
    ) -> Result<(), (PreparedCommit, IntegrationError)> {
        let PreparedCommit {
            publication,
            cleanup,
        } = prepared;
        match self
            .inner
            .cancel_prepared_publication_for_authority_settlement(publication)
        {
            Ok(()) => Ok(()),
            Err((publication, error)) => Err((
                PreparedCommit {
                    publication,
                    cleanup,
                },
                map_commit_error(error),
            )),
        }
    }

    pub fn complete_cleanup(
        &mut self,
        completion: CleanupCompletion,
    ) -> Result<(), IntegrationError> {
        self.inner
            .complete_cleanup(completion.inner)
            .map_err(|error| match error {
                crate::engine::CleanupError::Duplicate => IntegrationError::Duplicate,
                crate::engine::CleanupError::Stale => IntegrationError::Stale,
                crate::engine::CleanupError::Invalid => IntegrationError::Invalid,
            })
    }

    #[doc(hidden)]
    pub fn prepare_failure_cleanup_completion(
        &mut self,
        completion: CleanupCompletion,
    ) -> Result<FailureCleanupCompletionOwner, PrepareFailureCleanupCompletionError> {
        self.inner
            .prepare_failure_cleanup_completion(completion.inner)
            .map(|inner| FailureCleanupCompletionOwner { inner })
            .map_err(|error: CoreFailureCleanupCompletionPrepareError| {
                let kind = map_cleanup_error(error.kind());
                PrepareFailureCleanupCompletionError {
                    kind,
                    mismatch: FailureCleanupCompletionPreparationMismatchView {
                        inner: error.into_mismatch(),
                    },
                }
            })
    }

    #[doc(hidden)]
    #[cfg(any(test, feature = "test-support"))]
    pub fn drift_failure_cleanup_completion_receiver_for_test(&mut self) {
        self.inner
            .drift_failure_cleanup_completion_receiver_for_test();
    }

    #[doc(hidden)]
    pub fn complete_prepared_failure_cleanup_with_ledger<Physical, F>(
        &mut self,
        owner: FailureCleanupCompletionOwner,
        physical: Physical,
        commit_ledger_and_clear_holder: F,
    ) -> Result<FailureCleanupCompletionDisposition, CompleteFailureCleanupError<Physical>>
    where
        F: FnOnce(Physical),
    {
        self.inner
            .complete_prepared_failure_cleanup_with_ledger(
                owner.inner,
                physical,
                commit_ledger_and_clear_holder,
            )
            .map(|disposition| match disposition {
                CoreFailureCleanupCompletionDisposition::Settled => {
                    FailureCleanupCompletionDisposition::Settled
                }
                CoreFailureCleanupCompletionDisposition::RetainedPreparedPrefixBound => {
                    FailureCleanupCompletionDisposition::RetainedPreparedPrefixBound
                }
                CoreFailureCleanupCompletionDisposition::RetainedPreparedPrefixCleanup => {
                    FailureCleanupCompletionDisposition::RetainedPreparedPrefixCleanup
                }
            })
            .map_err(|error: CoreFailureCleanupCompletionCommitError<Physical>| {
                let kind = map_cleanup_error(error.kind());
                let (owner, physical, mismatch) = error.into_parts();
                CompleteFailureCleanupError {
                    kind,
                    owner: FailureCleanupCompletionOwner { inner: owner },
                    physical,
                    mismatch: FailureCleanupCompletionReceiverMismatchView { inner: mismatch },
                }
            })
    }

    #[doc(hidden)]
    pub fn complete_authority_cleanup<F>(
        &mut self,
        completion: CleanupCompletion,
        mutation: F,
    ) -> Result<(), IntegrationError>
    where
        F: FnOnce(),
    {
        if self.host_trace.try_reserve(1).is_err() {
            return Err(IntegrationError::Invalid);
        }
        let version = self.inner.host().cloned();
        let slot = self.inner.top_slot();
        let inner = &mut self.inner;
        let host_trace = &mut self.host_trace;
        inner
            .complete_cleanup_with_authority_mutation(completion.inner, || {
                mutation();
                host_trace.push(HostTraceEvent::project(
                    HostTraceEventKind::SettlementCompleted,
                    version,
                    slot,
                ));
            })
            .map_err(|error| match error {
                crate::engine::CleanupError::Duplicate => IntegrationError::Duplicate,
                crate::engine::CleanupError::Stale => IntegrationError::Stale,
                crate::engine::CleanupError::Invalid => IntegrationError::Invalid,
            })
    }

    pub fn observe_cleanup_receipt(
        &mut self,
        batch: &CleanupBatch,
        disposition: CleanupDisposition,
    ) -> Result<(), IntegrationError> {
        self.inner
            .observe_cleanup_receipt(&batch.inner, disposition)
            .map_err(|_| IntegrationError::Stale)
    }

    pub fn snapshot(&self) -> RouterSnapshotProjection {
        let (router, evicted_through, highest_issued) = self.inner.request_retention_facts();
        RouterSnapshotProjection {
            requested: self.inner.requested().cloned(),
            matched: self.inner.matched().cloned(),
            active: self.inner.active().clone(),
            availability: self.inner.availability().clone(),
            pending_phase: self.inner.pending_phase().cloned(),
            history: self.inner.history_snapshot(),
            running_request: self.inner.running_request(),
            earliest_retained_sequence: self.inner.earliest_retained_sequence(),
            request_retention: RequestRetentionClassifier::project(
                router,
                evicted_through,
                highest_issued,
            ),
            diagnostics: self.inner.diagnostic_snapshot(),
            host: HostSnapshot::project(
                self.inner.host().cloned(),
                self.inner.top_slot(),
                self.host_trace.clone(),
            ),
        }
    }

    pub fn active(&self) -> &ActivePresentation {
        self.inner.active()
    }

    #[doc(hidden)]
    pub fn reconciliation_projection_for_task(
        &self,
        task: &FrameTask,
    ) -> Result<ReconciliationProjectionView<'_>, IntegrationError> {
        self.inner
            .reconciliation_projection_for(task.inner.identity())
            .map(|inner| ReconciliationProjectionView { inner })
            .ok_or(IntegrationError::Stale)
    }

    #[doc(hidden)]
    pub fn reconciliation_projection_for_commit(
        &self,
        ticket: &CommitTicket,
    ) -> Result<ReconciliationProjectionView<'_>, IntegrationError> {
        self.inner
            .reconciliation_projection_for(ticket.inner.identity())
            .map(|inner| ReconciliationProjectionView { inner })
            .ok_or(IntegrationError::Stale)
    }

    pub fn history(&self) -> &MemoryHistory {
        self.inner.history()
    }

    pub fn router_diagnostics(&self) -> &[DiagnosticEvent] {
        self.inner.router_diagnostics()
    }

    pub fn request(&self, id: NavigationRequestId) -> Option<NavigationRequestView<'_>> {
        self.inner
            .request(id)
            .map(|record| NavigationRequestView { record })
    }

    pub fn request_lookup(&self, id: NavigationRequestId) -> RequestLookup<'_> {
        match self.inner.private_lookup(id) {
            PrivateRequestLookup::Retained(record) => {
                RequestLookup::Found(NavigationRequestView { record })
            }
            PrivateRequestLookup::Evicted => RequestLookup::NotRetained,
            PrivateRequestLookup::Unknown => RequestLookup::Unknown,
        }
    }

    /// Trusted ECS completion bridge retaining the whole private physical
    /// owner when Core's final receiver refreeze rejects completion.
    #[doc(hidden)]
    pub fn complete_post_commit_authority_compound_trusted<Physical, PhysicalTerminal, F>(
        &mut self,
        owner: PostCommitCompoundCompletionOwner,
        physical: Physical,
        clear_physical_holder: F,
    ) -> TrustedPostCommitCompoundCompletionOutcome<Physical, PhysicalTerminal>
    where
        F: FnOnce(Physical, RetiredHostAuthorityExpectedView) -> PhysicalTerminal,
    {
        let mut physical = Some(physical);
        match self
            .inner
            .complete_post_commit_authority_compound(owner.inner, |expected| {
                clear_physical_holder(
                    physical
                        .take()
                        .expect("the trusted completion callback consumes physical once"),
                    RetiredHostAuthorityExpectedView { inner: expected },
                )
            }) {
            CorePostCommitCompoundCompletionOutcome::Terminal {
                terminal,
                physical,
                retired_host,
            } => TrustedPostCommitCompoundCompletionOutcome::Terminal {
                terminal: PostCommitTerminalEvidence { inner: terminal },
                physical,
                retired_host: RetiredHostAuthorityTerminalEvidence {
                    inner: retired_host,
                },
            },
            CorePostCommitCompoundCompletionOutcome::Mismatch { owner, evidence } => {
                TrustedPostCommitCompoundCompletionOutcome::Mismatch {
                    owner: PostCommitCompoundCompletionOwner { inner: owner },
                    physical: physical
                        .take()
                        .expect("a rejected trusted completion returns physical whole"),
                    evidence: PostCommitCompoundMismatchEvidence { inner: evidence },
                }
            }
        }
    }
}

fn map_install_error(error: CoreEngineInstallError) -> IntegrationError {
    match error {
        CoreEngineInstallError::IdExhausted => IntegrationError::Exhausted,
    }
}

fn map_admission_error(error: CoreAdmissionError) -> IntegrationError {
    match error {
        CoreAdmissionError::OutstandingExists => IntegrationError::OutstandingAdmission,
        CoreAdmissionError::AuthorityExists => IntegrationError::AuthorityExists,
        CoreAdmissionError::ForeignRouter => IntegrationError::ForeignRouter,
        CoreAdmissionError::Stale => IntegrationError::Stale,
        CoreAdmissionError::Exhausted => IntegrationError::Exhausted,
    }
}

fn map_failed_authority_trigger_error(
    error: CoreFailedAuthorityTriggerError,
) -> FailedAuthorityTriggerError {
    match error {
        CoreFailedAuthorityTriggerError::Foreign => FailedAuthorityTriggerError::Foreign,
        CoreFailedAuthorityTriggerError::Stale => FailedAuthorityTriggerError::Stale,
        CoreFailedAuthorityTriggerError::WrongPhase => FailedAuthorityTriggerError::WrongPhase,
        CoreFailedAuthorityTriggerError::Exhausted => FailedAuthorityTriggerError::Exhausted,
    }
}

fn map_begin_authority_settlement_error(
    error: CoreBeginAuthoritySettlementErrorKind,
) -> BeginAuthoritySettlementErrorKind {
    match error {
        CoreBeginAuthoritySettlementErrorKind::Foreign => {
            BeginAuthoritySettlementErrorKind::Foreign
        }
        CoreBeginAuthoritySettlementErrorKind::Stale => BeginAuthoritySettlementErrorKind::Stale,
        CoreBeginAuthoritySettlementErrorKind::WrongPhase => {
            BeginAuthoritySettlementErrorKind::WrongPhase
        }
        CoreBeginAuthoritySettlementErrorKind::InvalidManifest => {
            BeginAuthoritySettlementErrorKind::InvalidManifest
        }
        CoreBeginAuthoritySettlementErrorKind::RetryablePreparation => {
            BeginAuthoritySettlementErrorKind::RetryablePreparation
        }
    }
}

fn map_push_error(error: CorePushError) -> IntegrationError {
    match error {
        CorePushError::AwaitingHost => IntegrationError::AwaitingHost,
        CorePushError::AuthorityRecovering => IntegrationError::AuthorityExists,
        CorePushError::IdExhausted => IntegrationError::Exhausted,
    }
}

fn map_report_error(error: CoreReportError) -> IntegrationError {
    match error {
        CoreReportError::Duplicate => IntegrationError::Duplicate,
        CoreReportError::Stale => IntegrationError::Stale,
        CoreReportError::Invalid => IntegrationError::Invalid,
    }
}

fn map_mount_error(error: crate::mount::MountError) -> IntegrationError {
    match error {
        crate::mount::MountError::Exhausted => IntegrationError::Exhausted,
        crate::mount::MountError::Foreign => IntegrationError::ForeignRouter,
        crate::mount::MountError::Stale => IntegrationError::Stale,
        crate::mount::MountError::WrongPhase => IntegrationError::Invalid,
    }
}

fn map_commit_error(error: CoreCommitError) -> IntegrationError {
    match error {
        CoreCommitError::Stale => IntegrationError::Stale,
        CoreCommitError::InvalidManifest => IntegrationError::InvalidManifest,
    }
}

fn map_failure_cleanup_error(error: CoreFailureCleanupError) -> IntegrationError {
    match error {
        CoreFailureCleanupError::Foreign => IntegrationError::ForeignRouter,
        CoreFailureCleanupError::Stale => IntegrationError::Stale,
        CoreFailureCleanupError::InvalidManifest => IntegrationError::InvalidManifest,
    }
}

fn map_cleanup_error(error: crate::engine::CleanupError) -> IntegrationError {
    match error {
        crate::engine::CleanupError::Duplicate => IntegrationError::Duplicate,
        crate::engine::CleanupError::Stale => IntegrationError::Stale,
        crate::engine::CleanupError::Invalid => IntegrationError::Invalid,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::admission::HostAdmissionIssuer;
    use crate::engine::BackTargetBusyReceiverSlot;
    use crate::frame::RouteFrameVersion;
    use crate::ids::{CoreIdAllocator, RouteGeneration};
    use crate::meta::ErrorBoundaryKey;
    use crate::mount::MountIssuer;
    use crate::prelude::{
        ActivePresentationKind, CleanupDisposition, FrameOrigin, HostTraceEventKind,
        NavigationPhase, NavigationTraceEventKind, RequestStatus, RouteLocation, RouteNodeOptions,
        RoutePattern, RouteTree, RouteViewKey, RouterAvailability,
    };

    fn tree(view: &RouteViewKey) -> RouteTree {
        RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/").unwrap(),
                RouteNodeOptions::new().with_view(view.clone()),
            )
            .unwrap()
            .build()
            .unwrap()
    }

    #[cfg(feature = "test-support")]
    #[test]
    fn back_capacity_core_producer_slot_cannot_be_reacquired_after_install() {
        let view = RouteViewKey::new("back-capacity-core-producer-slot").unwrap();
        let fallback = RouteViewKey::new("back-capacity-core-producer-fallback").unwrap();
        let mut engine = RouterEngine::install(tree(&view), fallback).unwrap();
        let slot = engine
            .prepare_back_capacity_core_producer_slot()
            .expect("a fresh integration engine has one vacant Core producer slot");
        let (_observer, _issuer, core_producer, _facade) =
            crate::test_support::issue_back_capacity_order_channels();

        slot.install(core_producer);

        assert!(engine.prepare_back_capacity_core_producer_slot().is_none());
    }

    fn nested_tree() -> RouteTree {
        let mut builder = RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/").unwrap(),
                RouteNodeOptions::new().with_view(RouteViewKey::new("layout").unwrap()),
            )
            .unwrap();
        let root = builder.root_id().unwrap();
        builder
            .route_with_options(
                root,
                RoutePattern::path("items/:id").unwrap(),
                RouteNodeOptions::new().with_view(RouteViewKey::new("item").unwrap()),
            )
            .unwrap();
        builder
            .route_with_options(
                root,
                RoutePattern::path("side/:id").unwrap(),
                RouteNodeOptions::new()
                    .with_slot(RouteSlotKey::named("side").unwrap())
                    .with_view(RouteViewKey::new("side-item").unwrap()),
            )
            .unwrap();
        builder.build().unwrap()
    }

    fn back_target_nested_tree() -> RouteTree {
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

    fn back_target_bound_nested_tree() -> (RouteTree, crate::ids::RouteId, RouteViewKey) {
        let items_boundary = ErrorBoundaryKey::named("items-boundary").unwrap();
        let items_error_view = RouteViewKey::new("items-error").unwrap();
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
                RouteNodeOptions::new()
                    .with_view(RouteViewKey::new("items").unwrap())
                    .with_error_view(items_boundary, items_error_view.clone()),
            )
            .unwrap();
        builder
            .route_with_options(
                items,
                RoutePattern::path(":id").unwrap(),
                RouteNodeOptions::new().with_view(RouteViewKey::new("item").unwrap()),
            )
            .unwrap();
        (builder.build().unwrap(), items, items_error_view)
    }

    fn confirmed_nested_engine() -> RouterEngine {
        let mut engine =
            RouterEngine::install(nested_tree(), RouteViewKey::new("fallback").unwrap()).unwrap();
        let token = engine.request_host_admission().unwrap();
        let grant = engine.grant_host_admission(&token).unwrap();
        engine.record_top_slot_registered(&grant).unwrap();
        engine.confirm_host(grant);
        engine
    }

    fn confirmed_back_target_nested_engine() -> RouterEngine {
        let mut engine = RouterEngine::install(
            back_target_nested_tree(),
            RouteViewKey::new("fallback").unwrap(),
        )
        .unwrap();
        let token = engine.request_host_admission().unwrap();
        let grant = engine.grant_host_admission(&token).unwrap();
        engine.record_top_slot_registered(&grant).unwrap();
        engine.confirm_host(grant);
        engine
    }

    fn confirmed_back_target_bound_nested_engine()
    -> (RouterEngine, crate::ids::RouteId, RouteViewKey) {
        let (tree, items_route, items_error_view) = back_target_bound_nested_tree();
        let mut engine =
            RouterEngine::install(tree, RouteViewKey::new("fallback").unwrap()).unwrap();
        let token = engine.request_host_admission().unwrap();
        let grant = engine.grant_host_admission(&token).unwrap();
        engine.record_top_slot_registered(&grant).unwrap();
        engine.confirm_host(grant);
        (engine, items_route, items_error_view)
    }

    fn confirmed_nested_engine_with_lease() -> (RouterEngine, HostLease) {
        let mut engine =
            RouterEngine::install(nested_tree(), RouteViewKey::new("fallback").unwrap()).unwrap();
        let token = engine.request_host_admission().unwrap();
        let grant = engine.grant_host_admission(&token).unwrap();
        engine.record_top_slot_registered(&grant).unwrap();
        let lease = engine.confirm_host(grant);
        (engine, lease)
    }

    #[test]
    fn failed_authority_trigger_is_burned_on_rejection_and_cannot_replay() {
        let (mut engine, lease) = confirmed_nested_engine_with_lease();
        engine.push(RouteLocation::new("/items/1")).unwrap();
        engine.collect();
        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("failed terminal fixture requires one exact frame task")
        };
        let trigger = engine.failed_authority_trigger_for_task(&task).unwrap();
        let replay = trigger.testing_duplicate();
        let error = match engine.begin_failed_authority_settlement_retaining(
            lease,
            trigger,
            CleanupManifest::new(vec![task.frame().clone(), task.frame().clone()], 0xFA11, 2),
            |_| panic!("invalid manifest must reject before publication"),
        ) {
            Err(error) => error,
            Ok(_) => panic!("mismatched Core-frozen roots reject publication"),
        };
        assert_eq!(
            error.kind(),
            BeginAuthoritySettlementErrorKind::InvalidManifest
        );
        let returned_lease = error.into_owner();
        let replacement = engine.failed_authority_trigger_for_task(&task).unwrap();
        let replay_error = match engine.begin_failed_authority_settlement_retaining(
            returned_lease,
            replay,
            CleanupManifest::empty(),
            |_| panic!("burned trigger must not publish"),
        ) {
            Err(error) => error,
            Ok(_) => panic!("duplicate trigger is stale even after rejected publication"),
        };
        assert_eq!(
            replay_error.kind(),
            BeginAuthoritySettlementErrorKind::Stale
        );
        let returned_lease = replay_error.into_owner();
        let replacement_error = match engine.begin_failed_authority_settlement_retaining(
            returned_lease,
            replacement,
            CleanupManifest::new(vec![task.frame().clone(), task.frame().clone()], 0xFA12, 2),
            |_| panic!("invalid replacement manifest must not publish"),
        ) {
            Err(error) => error,
            Ok(_) => panic!("the fresh revision must remain live after stale replay rejection"),
        };
        assert_eq!(
            replacement_error.kind(),
            BeginAuthoritySettlementErrorKind::InvalidManifest
        );
        assert_eq!(engine.snapshot().availability(), &RouterAvailability::Ready);
    }

    #[test]
    fn failed_authority_task_trigger_accepts_either_core_frozen_root_set() {
        for include_target in [false, true] {
            let (mut engine, lease) = confirmed_nested_engine_with_lease();
            engine.push(RouteLocation::new("/items/1")).unwrap();
            engine.collect();
            let EngineAction::FrameTask(task) = engine.action().unwrap() else {
                panic!("failed terminal fixture requires one exact frame task")
            };
            let trigger = engine.failed_authority_trigger_for_task(&task).unwrap();
            let manifest = if include_target {
                CleanupManifest::new(vec![task.frame().clone()], 0xFA12, 1)
            } else {
                CleanupManifest::empty()
            };
            let published = std::cell::Cell::new(false);

            if let Err(_) =
                engine.begin_failed_authority_settlement_retaining(lease, trigger, manifest, |_| {
                    published.set(true)
                })
            {
                panic!("either Core-frozen task root set is admissible");
            }

            assert!(published.get());
            assert_eq!(
                engine.snapshot().availability(),
                &RouterAvailability::RecoveringAuthority
            );
        }
    }

    #[test]
    fn failed_authority_task_target_logical_root_allows_zero_physical_obligations() {
        let (mut engine, lease) = confirmed_nested_engine_with_lease();
        engine.push(RouteLocation::new("/items/1")).unwrap();
        engine.collect();
        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("failed terminal fixture requires one exact frame task")
        };
        let trigger = engine.failed_authority_trigger_for_task(&task).unwrap();
        let authority_mutation_ran = std::cell::Cell::new(false);
        let cleanup = engine
            .begin_failed_authority_settlement_retaining(
                lease,
                trigger,
                CleanupManifest::new(vec![task.frame().clone()], 0xFA13, 0),
                |_| authority_mutation_ran.set(true),
            )
            .expect("exact logical roots are independent of physical obligation count");

        assert!(authority_mutation_ran.get());
        assert_eq!(cleanup.obligation_count(), 0);
        assert_eq!(
            engine.snapshot().availability(),
            &RouterAvailability::RecoveringAuthority
        );
    }

    #[test]
    fn failed_authority_cleanup_trigger_does_not_replace_the_terminal_owner() {
        let (mut engine, lease) = confirmed_nested_engine_with_lease();
        let mount = pending_mount_request(&mut engine);
        let cleanup = engine
            .reject_mount(mount, MountRejectionKind::MissingOutlet)
            .unwrap();
        let roots = cleanup.logical_roots().to_vec();
        let trigger = engine
            .failed_authority_trigger_for_cleanup(&cleanup)
            .unwrap();
        let error = match engine.begin_failed_authority_settlement_retaining(
            lease,
            trigger,
            CleanupManifest::new(Vec::new(), 0xFA14, 1),
            |_| panic!("invalid cleanup roots must not publish"),
        ) {
            Err(error) => error,
            Ok(_) => panic!("the failed cleanup trigger freezes its exact logical roots"),
        };
        assert_eq!(
            error.kind(),
            BeginAuthoritySettlementErrorKind::InvalidManifest
        );

        assert!(matches!(
            engine.prepare_failure_cleanup_retaining(
                cleanup,
                CleanupManifest::new(roots, 0xFA15, 1),
            ),
            FailureCleanupPrepareOutcome::Prepared(_)
        ));
    }

    #[test]
    fn failed_authority_registration_trigger_does_not_replace_the_terminal_owner() {
        let (mut engine, lease) = confirmed_nested_engine_with_lease();
        let (request, parent, _, permit, _) =
            pending_mount_registration(&mut engine, "/items/1", 0xFA16);
        let mut foreign = confirmed_nested_engine();
        let (_, _, _, _, foreign_proof) =
            pending_mount_registration(&mut foreign, "/items/1", 0xFA16);
        let registration = match engine.begin_mount_registration(permit, &foreign_proof) {
            Err(error) => error,
            Ok(_) => panic!("foreign proof must retain the exact registration permit"),
        };
        let trigger = engine
            .failed_authority_trigger_for_registration_begin(&registration)
            .unwrap();
        let settlement = match engine.begin_failed_authority_settlement_retaining(
            lease,
            trigger,
            CleanupManifest::new(Vec::new(), 0xFA17, 1),
            |_| panic!("invalid registration roots must not publish"),
        ) {
            Err(error) => error,
            Ok(_) => panic!("registration trigger freezes current applied roots"),
        };
        assert_eq!(
            settlement.kind(),
            BeginAuthoritySettlementErrorKind::InvalidManifest
        );

        let cleanup = registration
            .reject(&mut engine)
            .expect("the caller still owns the consumable registration terminal");
        assert_eq!(cleanup.request(), request);
        assert_eq!(cleanup.logical_roots(), &[parent]);
    }

    fn pending_mount_request(engine: &mut RouterEngine) -> MountRequest {
        engine.push(RouteLocation::new("/items/1")).unwrap();
        engine.collect();
        let EngineAction::FrameTask(parent) = engine.action().unwrap() else {
            panic!("parent frame task expected")
        };
        engine
            .report_applied_with_outlets(parent, vec![RouteSlotKey::default()])
            .unwrap();
        let EngineAction::MountRequest(request) = engine.action().unwrap() else {
            panic!("mount request expected")
        };
        request
    }

    #[test]
    fn reconciliation_projection_view_is_read_only_and_slot_aware() {
        let mut engine = confirmed_nested_engine();
        engine.push(RouteLocation::new("/items/1")).unwrap();
        engine.collect();
        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("first reconciliation action must be a frame task")
        };

        let mut foreign = confirmed_nested_engine();
        foreign.push(RouteLocation::new("/items/1")).unwrap();
        foreign.collect();
        let EngineAction::FrameTask(foreign_task) = foreign.action().unwrap() else {
            panic!("foreign reconciliation action must be a frame task")
        };
        assert!(matches!(
            engine.reconciliation_projection_for_task(&foreign_task),
            Err(IntegrationError::Stale)
        ));

        let projection = engine
            .reconciliation_projection_for_task(&task)
            .expect("exact running task borrows one sealed projection");
        assert_eq!(projection.preserved().len(), 0);
        assert_eq!(projection.leaving().len(), 0);
        let entering = projection.entering().collect::<Vec<_>>();
        assert_eq!(entering.len(), 2);
        assert!(matches!(
            entering[0].slot(),
            ReconciliationEnteringSlotKind::Existing(_)
        ));
        assert_eq!(entering[1].slot(), ReconciliationEnteringSlotKind::MountNew);
        let slots = projection.child_slots().collect::<Vec<_>>();
        assert_eq!(slots.len(), 1);
        assert_eq!(slots[0].kind(), ReconciliationChildSlotRoleKind::MountNew);
        assert!(slots[0].binding().is_none());
    }

    #[test]
    fn equivalent_commit_borrows_the_same_sealed_reconciliation_projection() {
        let mut engine = confirmed_engine();
        settle_push(&mut engine, RouteLocation::new("/"));
        engine.push(RouteLocation::new("/")).unwrap();
        engine.collect();
        let EngineAction::Commit(ticket) = engine.action().unwrap() else {
            panic!("equivalent push must issue a commit ticket")
        };

        let projection = engine
            .reconciliation_projection_for_commit(&ticket)
            .expect("exact commit ticket borrows the current projection");
        assert_eq!(projection.preserved().len(), 1);
        assert_eq!(projection.entering().len(), 0);
        assert_eq!(projection.leaving().len(), 0);
    }

    #[test]
    fn test_support_mount_target_replacement_changes_only_the_detached_task() {
        let mut engine = confirmed_engine();
        engine.push(RouteLocation::new("/")).unwrap();
        engine.collect();
        let EngineAction::FrameTask(mut task) = engine.action().unwrap() else {
            panic!("first engine must issue one root task")
        };
        let identity = task.report_identity();
        let engine_before = engine.snapshot();

        let mut foreign = confirmed_engine();
        foreign.push(RouteLocation::new("/")).unwrap();
        foreign.collect();
        let EngineAction::FrameTask(foreign_task) = foreign.action().unwrap() else {
            panic!("foreign engine must issue one root task")
        };
        let FrameMountTarget::TopSlot { host, slot } = foreign_task.mount_target() else {
            panic!("root task must target a top slot")
        };

        assert!(task.testing_replace_top_slot_mount_target(host.clone(), slot));

        assert!(task.report_identity() == identity);
        assert_eq!(
            task.mount_target(),
            FrameMountTarget::TopSlot { host, slot }
        );
        assert_eq!(engine.snapshot(), engine_before);
        assert_eq!(engine.inspect_report_identity(&task), Ok(()));
    }

    #[test]
    fn test_support_top_slot_replacement_rejects_nested_task_without_mutation() {
        let mut engine = confirmed_nested_engine();
        let (_, _, _, permit, proof) = pending_mount_registration(&mut engine, "/items/1", 307);
        engine
            .begin_mount_registration(permit, &proof)
            .unwrap()
            .complete(309);
        let EngineAction::FrameTask(mut task) = engine.action().unwrap() else {
            panic!("completed registration must issue the child task")
        };
        let target_before = task.mount_target();
        assert!(matches!(
            target_before,
            FrameMountTarget::ChildOutlet { .. }
        ));
        let identity_before = task.report_identity();
        let engine_before = engine.snapshot();

        let mut foreign = confirmed_engine();
        foreign.push(RouteLocation::new("/")).unwrap();
        foreign.collect();
        let EngineAction::FrameTask(foreign_task) = foreign.action().unwrap() else {
            panic!("foreign engine must issue one root task")
        };
        let FrameMountTarget::TopSlot { host, slot } = foreign_task.mount_target() else {
            panic!("root task must target a top slot")
        };

        assert!(!task.testing_replace_top_slot_mount_target(host, slot));

        assert!(task.report_identity() == identity_before);
        assert_eq!(task.mount_target(), target_before);
        assert_eq!(engine.snapshot(), engine_before);
        assert_eq!(engine.inspect_report_identity(&task), Ok(()));
    }

    fn pending_nested_receipt(engine: &mut RouterEngine, digest: u64) -> MountReceiptAttestation {
        engine.push(RouteLocation::new("/items/1")).unwrap();
        engine.collect();
        let EngineAction::FrameTask(parent) = engine.action().unwrap() else {
            panic!("parent frame task expected")
        };
        engine
            .report_applied_with_outlets(parent, vec![RouteSlotKey::default()])
            .unwrap();
        let EngineAction::MountRequest(mount) = engine.action().unwrap() else {
            panic!("mount request expected")
        };
        engine.resolve_mount(mount).unwrap().attest_receipt(digest)
    }

    fn pending_mount_registration(
        engine: &mut RouterEngine,
        location: &str,
        digest: u64,
    ) -> (
        NavigationRequestId,
        RouteFrameVersion,
        RouteSlotId,
        MountRegistrationPermit,
        MountReceiptProof,
    ) {
        let request = engine.push(RouteLocation::new(location)).unwrap();
        engine.collect();
        let (parent_frame, mount) = match engine.action().unwrap() {
            EngineAction::FrameTask(parent) => {
                let parent_frame = parent.frame().clone();
                engine
                    .report_applied_with_outlets(parent, vec![RouteSlotKey::default()])
                    .unwrap();
                let EngineAction::MountRequest(mount) = engine.action().unwrap() else {
                    panic!("mount request expected")
                };
                (parent_frame, mount)
            }
            EngineAction::MountRequest(mount) => (mount.parent().clone(), mount),
            _ => panic!("parent task or direct mount request expected"),
        };
        let grant = engine.resolve_mount(mount).unwrap();
        let slot = grant.slot();
        let (receipt, proof) = grant.attest_receipt_with_proof(digest);
        let permit = engine.accept_mount_receipt(receipt).unwrap();
        (request, parent_frame, slot, permit, proof)
    }

    #[test]
    fn mount_registration_cross_router_proof_is_foreign_and_zero_write() {
        let mut receiver = confirmed_nested_engine();
        let (request, parent, receiver_slot, receiver_permit, _) =
            pending_mount_registration(&mut receiver, "/items/1", 311);
        let mut source = confirmed_nested_engine();
        let (_, _, source_slot, _, source_proof) =
            pending_mount_registration(&mut source, "/items/1", 311);
        assert_eq!(receiver_slot, source_slot);
        let before = receiver.inner.testing_snapshot();

        let error = match receiver.begin_mount_registration(receiver_permit, &source_proof) {
            Ok(_) => panic!("cross-router proof must not open a registration session"),
            Err(error) => error,
        };

        assert_eq!(error.kind(), IntegrationError::ForeignRouter);
        assert_eq!(receiver.inner.testing_snapshot(), before);
        let failure = error.reject(&mut receiver).unwrap();
        assert_eq!(failure.request(), request);
        assert_eq!(failure.logical_roots(), &[parent]);
    }

    #[test]
    fn mount_registration_old_same_router_proof_is_stale_and_zero_write() {
        let mut engine = confirmed_nested_engine();
        let (_, _, _, permit, old_proof) = pending_mount_registration(&mut engine, "/items/1", 313);
        engine
            .begin_mount_registration(permit, &old_proof)
            .unwrap()
            .complete(317);
        let EngineAction::FrameTask(child) = engine.action().unwrap() else {
            panic!("completed registration must issue the child task")
        };
        engine
            .report_applied_with_outlets(child, Vec::new())
            .unwrap();
        let EngineAction::Commit(ticket) = engine.action().unwrap() else {
            panic!("completed child must yield one commit")
        };
        let prepared = engine
            .prepare_commit(&ticket, CleanupManifest::empty())
            .unwrap();
        let cleanup = prepared.cleanup().clone();
        let validated = engine.validate_publication(prepared).unwrap();
        validated.publish(|| {});
        engine.complete_cleanup(cleanup.complete_empty()).unwrap();

        let (request, _parent, _, permit, _) =
            pending_mount_registration(&mut engine, "/side/2", 313);
        let before = engine.inner.testing_snapshot();
        let error = match engine.begin_mount_registration(permit, &old_proof) {
            Ok(_) => panic!("an old same-router proof must not open a registration session"),
            Err(error) => error,
        };

        assert_eq!(error.kind(), IntegrationError::Stale);
        assert_eq!(engine.inner.testing_snapshot(), before);
        let failure = error.reject(&mut engine).unwrap();
        assert_eq!(failure.request(), request);
        assert!(failure.logical_roots().is_empty());
    }

    #[test]
    fn mount_registration_session_withholds_child_until_completion() {
        let mut engine = confirmed_nested_engine();
        let (_, _, slot, permit, proof) = pending_mount_registration(&mut engine, "/items/1", 319);
        assert!(engine.action().is_none());
        let session = engine.begin_mount_registration(permit, &proof).unwrap();

        assert_eq!(session.slot(), slot);
        session.complete(321);

        let EngineAction::FrameTask(child) = engine.action().unwrap() else {
            panic!("only completed registration may issue the child task")
        };
        assert!(matches!(
            child.mount_target(),
            FrameMountTarget::ChildOutlet { .. }
        ));
    }

    #[test]
    fn mount_registration_context_drift_returns_linear_rejection_capability() {
        let mut engine = confirmed_nested_engine();
        let (request, parent, _, permit, proof) =
            pending_mount_registration(&mut engine, "/items/1", 327);
        engine
            .inner
            .testing_set_mount_registration_context_live(false);
        let before = engine.inner.testing_snapshot();

        let error = match engine.begin_mount_registration(permit, &proof) {
            Ok(_) => panic!("drifted engine context must not open a registration session"),
            Err(error) => error,
        };

        assert_eq!(error.kind(), IntegrationError::Invalid);
        assert_eq!(engine.inner.testing_snapshot(), before);
        engine
            .inner
            .testing_set_mount_registration_context_live(true);
        let failure = error.reject(&mut engine).unwrap();
        assert_eq!(failure.request(), request);
        assert_eq!(failure.logical_roots(), &[parent]);
    }

    #[test]
    fn mount_registration_session_rejects_with_exact_failure_cleanup() {
        let mut engine = confirmed_nested_engine();
        let (request, parent, slot, permit, proof) =
            pending_mount_registration(&mut engine, "/items/1", 323);
        let session = engine.begin_mount_registration(permit, &proof).unwrap();
        assert_eq!(session.slot(), slot);

        let failure = session.reject();

        assert_eq!(failure.request(), request);
        assert_eq!(failure.logical_roots(), &[parent]);
    }

    fn pending_nested_completion(
        engine: &mut RouterEngine,
        receipt_digest: u64,
        registration_digest: u64,
    ) -> MountRegistrationCompletion {
        let receipt = pending_nested_receipt(engine, receipt_digest);
        engine
            .accept_mount_receipt(receipt)
            .unwrap()
            .complete(registration_digest)
    }

    #[test]
    fn nested_integration_requires_outlet_report_and_registration_completion() {
        let mut engine = confirmed_nested_engine();
        let request = engine.push(RouteLocation::new("/items/1")).unwrap();
        engine.collect();
        let EngineAction::FrameTask(parent) = engine.action().unwrap() else {
            panic!("parent frame task expected")
        };
        let parent_sequence = parent.view_request_sequence();
        let parent_frame = parent.frame().clone();
        engine
            .report_applied_with_outlets(parent, vec![RouteSlotKey::default()])
            .unwrap();

        let EngineAction::MountRequest(mount) = engine.action().unwrap() else {
            panic!("parent report must yield a mount request")
        };
        assert_eq!(mount.request(), request);
        assert_eq!(mount.parent(), &parent_frame);
        assert!(mount.outlet().is_default());
        let grant = engine.resolve_mount(mount).unwrap();
        let receipt = grant.attest_receipt(101);
        let permit = engine.accept_mount_receipt(receipt).unwrap();
        assert!(engine.action().is_none());
        let completion = permit.complete(103);
        assert!(engine.action().is_none());
        engine.complete_mount_registration(completion).unwrap();

        let EngineAction::FrameTask(child) = engine.action().unwrap() else {
            panic!("registration completion must yield the child frame task")
        };
        assert_ne!(child.view_request_sequence(), parent_sequence);
        assert!(matches!(
            child.mount_target(),
            FrameMountTarget::ChildOutlet { .. }
        ));
        engine
            .report_applied_with_outlets(child, Vec::new())
            .unwrap();
        assert!(matches!(engine.action(), Some(EngineAction::Commit(_))));
    }

    #[test]
    fn expected_child_outlet_query_is_exact_and_read_only_for_foreign_and_stale_tasks() {
        let mut receiver = confirmed_nested_engine();
        receiver.push(RouteLocation::new("/items/1")).unwrap();
        receiver.collect();
        let EngineAction::FrameTask(parent) = receiver.action().unwrap() else {
            panic!("parent frame task expected")
        };
        let stale_parent = FrameTask {
            inner: parent.inner.testing_duplicate(),
        };
        let before_exact = receiver.inner.testing_snapshot();
        assert_eq!(
            receiver.expected_child_outlet(&parent),
            Ok(Some(RouteSlotKey::default()))
        );
        assert_eq!(receiver.inner.testing_snapshot(), before_exact);

        let mut foreign = confirmed_nested_engine();
        foreign.push(RouteLocation::new("/items/1")).unwrap();
        foreign.collect();
        let EngineAction::FrameTask(foreign_parent) = foreign.action().unwrap() else {
            panic!("foreign parent frame task expected")
        };
        assert_eq!(parent.frame().slot(), foreign_parent.frame().slot());
        let before_foreign = receiver.inner.testing_snapshot();
        assert_eq!(
            receiver.expected_child_outlet(&foreign_parent),
            Err(IntegrationError::Invalid)
        );
        assert_eq!(receiver.inner.testing_snapshot(), before_foreign);

        receiver
            .report_applied_with_outlets(parent, vec![RouteSlotKey::default()])
            .unwrap();
        let before_stale = receiver.inner.testing_snapshot();
        assert_eq!(
            receiver.expected_child_outlet(&stale_parent),
            Err(IntegrationError::Duplicate)
        );
        assert_eq!(receiver.inner.testing_snapshot(), before_stale);

        let EngineAction::MountRequest(mount) = receiver.action().unwrap() else {
            panic!("parent report must retain the mount request")
        };
        let receipt = receiver.resolve_mount(mount).unwrap().attest_receipt(401);
        let completion = receiver
            .accept_mount_receipt(receipt)
            .unwrap()
            .complete(409);
        receiver.complete_mount_registration(completion).unwrap();
        let EngineAction::FrameTask(child) = receiver.action().unwrap() else {
            panic!("completed mount must issue the leaf task")
        };
        let before_leaf = receiver.inner.testing_snapshot();
        assert_eq!(receiver.expected_child_outlet(&child), Ok(None));
        assert_eq!(receiver.inner.testing_snapshot(), before_leaf);
    }

    #[test]
    fn report_identity_inspection_is_read_only_for_foreign_duplicate_and_stale_tasks() {
        let mut receiver = confirmed_nested_engine();
        receiver.push(RouteLocation::new("/items/1")).unwrap();
        receiver.collect();
        let EngineAction::FrameTask(parent) = receiver.action().unwrap() else {
            panic!("parent frame task expected")
        };
        let stale_parent = FrameTask {
            inner: parent.inner.testing_duplicate(),
        };
        let before_exact = receiver.inner.testing_snapshot();
        assert_eq!(receiver.inspect_report_identity(&parent), Ok(()));
        assert_eq!(receiver.inner.testing_snapshot(), before_exact);

        let mut foreign = confirmed_nested_engine();
        foreign.push(RouteLocation::new("/items/1")).unwrap();
        foreign.collect();
        let EngineAction::FrameTask(foreign_parent) = foreign.action().unwrap() else {
            panic!("foreign parent frame task expected")
        };
        let before_foreign = receiver.inner.testing_snapshot();
        assert_eq!(
            receiver.inspect_report_identity(&foreign_parent),
            Err(IntegrationError::Invalid)
        );
        assert_eq!(receiver.inner.testing_snapshot(), before_foreign);

        receiver
            .report_applied_with_outlets(parent, vec![RouteSlotKey::default()])
            .unwrap();
        let before_duplicate = receiver.inner.testing_snapshot();
        assert_eq!(
            receiver.inspect_report_identity(&stale_parent),
            Err(IntegrationError::Duplicate)
        );
        assert_eq!(receiver.inner.testing_snapshot(), before_duplicate);

        let EngineAction::MountRequest(mount) = receiver.action().unwrap() else {
            panic!("parent report must retain the mount request")
        };
        let receipt = receiver.resolve_mount(mount).unwrap().attest_receipt(421);
        let completion = receiver
            .accept_mount_receipt(receipt)
            .unwrap()
            .complete(431);
        receiver.complete_mount_registration(completion).unwrap();
        let EngineAction::FrameTask(_child) = receiver.action().unwrap() else {
            panic!("completed mount must issue the leaf task")
        };
        let before_stale = receiver.inner.testing_snapshot();
        assert_eq!(
            receiver.inspect_report_identity(&stale_parent),
            Err(IntegrationError::Stale)
        );
        assert_eq!(receiver.inner.testing_snapshot(), before_stale);
    }

    #[test]
    fn mount_request_validation_is_exact_and_read_only_for_foreign_and_stale_tokens() {
        fn pending_mount(engine: &mut RouterEngine) -> MountRequest {
            engine.push(RouteLocation::new("/items/1")).unwrap();
            engine.collect();
            let EngineAction::FrameTask(parent) = engine.action().unwrap() else {
                panic!("parent frame task expected")
            };
            engine
                .report_applied_with_outlets(parent, vec![RouteSlotKey::default()])
                .unwrap();
            let EngineAction::MountRequest(request) = engine.action().unwrap() else {
                panic!("mount request expected")
            };
            request
        }

        let mut receiver = confirmed_nested_engine();
        let exact = pending_mount(&mut receiver);
        let stale = MountRequest {
            inner: exact.inner.testing_duplicate(),
        };
        let before_exact = receiver.inner.testing_snapshot();
        assert_eq!(receiver.validate_mount_request(&exact), Ok(()));
        assert_eq!(receiver.inner.testing_snapshot(), before_exact);

        let mut foreign = confirmed_nested_engine();
        let foreign = pending_mount(&mut foreign);
        assert_eq!(exact.parent().slot(), foreign.parent().slot());
        let before_foreign = receiver.inner.testing_snapshot();
        assert_eq!(
            receiver.validate_mount_request(&foreign),
            Err(IntegrationError::ForeignRouter)
        );
        assert_eq!(receiver.inner.testing_snapshot(), before_foreign);

        receiver.resolve_mount(exact).unwrap();
        let before_stale = receiver.inner.testing_snapshot();
        assert_eq!(
            receiver.validate_mount_request(&stale),
            Err(IntegrationError::Stale)
        );
        assert_eq!(receiver.inner.testing_snapshot(), before_stale);
    }

    #[test]
    fn foreign_mount_resolution_returns_request_to_source_without_receiver_write() {
        let mut source = confirmed_nested_engine();
        let request = pending_mount_request(&mut source);
        let mut receiver = confirmed_nested_engine();
        let before = receiver.inner.testing_snapshot();

        let error = match receiver.resolve_mount(request) {
            Ok(_) => panic!("a foreign mount request must not resolve"),
            Err(error) => error,
        };

        assert_eq!(error.kind(), IntegrationError::ForeignRouter);
        let request = match error.into_owner() {
            ResolveMountOwner::Returned(request) => request,
            ResolveMountOwner::Reissued => {
                panic!("a foreign request remains owned by the caller")
            }
            ResolveMountOwner::Consumed => {
                panic!("a foreign request cannot be consumed by the receiver")
            }
        };
        assert_eq!(receiver.inner.testing_snapshot(), before);
        assert!(source.resolve_mount(request).is_ok());
    }

    #[test]
    fn exhausted_mount_resolution_reports_core_reissued_request() {
        let mut engine = confirmed_nested_engine();
        let request = pending_mount_request(&mut engine);
        engine.inner.testing_exhaust_slot_counter();
        let before = engine.snapshot();

        let error = match engine.resolve_mount(request) {
            Ok(_) => panic!("an exhausted slot allocator must not issue a grant"),
            Err(error) => error,
        };

        assert_eq!(error.kind(), IntegrationError::Exhausted);
        assert!(matches!(error.into_owner(), ResolveMountOwner::Reissued));
        let Some(EngineAction::MountRequest(reissued)) = engine.action() else {
            panic!("Core must reissue the exact exhausted request")
        };
        assert!(engine.validate_mount_request(&reissued).is_ok());
        assert_eq!(engine.snapshot(), before);
    }

    #[test]
    fn foreign_mount_rejection_returns_request_to_source_without_receiver_write() {
        let mut source = confirmed_nested_engine();
        let request = pending_mount_request(&mut source);
        let mut receiver = confirmed_nested_engine();
        let before = receiver.inner.testing_snapshot();

        let error = match receiver.reject_mount(request, MountRejectionKind::MissingOutlet) {
            Ok(_) => panic!("a foreign mount request must not reject receiver work"),
            Err(error) => error,
        };

        assert_eq!(error.kind(), IntegrationError::ForeignRouter);
        let request = error.into_owner();
        assert_eq!(receiver.inner.testing_snapshot(), before);
        assert!(
            source
                .reject_mount(request, MountRejectionKind::MissingOutlet)
                .is_ok()
        );
    }

    #[test]
    fn foreign_mount_receipt_returns_attestation_to_source_without_receiver_write() {
        let mut source = confirmed_nested_engine();
        let receipt = pending_nested_receipt(&mut source, 439);
        let mut receiver = confirmed_nested_engine();
        let before = receiver.inner.testing_snapshot();

        let error = match receiver.accept_mount_receipt(receipt) {
            Ok(_) => panic!("a foreign receipt must not open receiver registration"),
            Err(error) => error,
        };

        assert_eq!(error.kind(), IntegrationError::ForeignRouter);
        let receipt = error.into_owner();
        assert_eq!(receiver.inner.testing_snapshot(), before);
        assert!(source.accept_mount_receipt(receipt).is_ok());
    }

    #[test]
    fn returned_exact_mount_receipt_is_unique_grant_stage_rejection_authority() {
        let mut source = confirmed_nested_engine();
        let receipt = pending_nested_receipt(&mut source, 440);
        let replay = MountReceiptAttestation {
            inner: receipt.inner.clone(),
        };
        let mut receiver = confirmed_nested_engine();
        let error = match receiver.accept_mount_receipt(receipt) {
            Ok(_) => panic!("a foreign receipt must not open receiver registration"),
            Err(error) => error,
        };
        let receipt = error.into_owner();

        assert!(
            source
                .reject_mount_after_mutation(MountMutationArtifact::Receipt(receipt))
                .is_ok()
        );
        let settled = source.inner.testing_snapshot();
        let error = match source.reject_mount_after_mutation(MountMutationArtifact::Receipt(replay))
        {
            Ok(_) => panic!("a consumed receipt must not reject twice"),
            Err(error) => error,
        };
        assert_eq!(error.kind(), IntegrationError::Stale);
        assert!(matches!(
            error.into_owner(),
            MountMutationArtifact::Receipt(_)
        ));
        assert_eq!(source.inner.testing_snapshot(), settled);
    }

    #[test]
    fn foreign_mount_receipt_rejection_returns_artifact_without_receiver_write() {
        let mut source = confirmed_nested_engine();
        let receipt = pending_nested_receipt(&mut source, 441);
        let mut receiver = confirmed_nested_engine();
        let before = receiver.inner.testing_snapshot();

        let error =
            match receiver.reject_mount_after_mutation(MountMutationArtifact::Receipt(receipt)) {
                Ok(_) => panic!("a foreign receipt must not reject receiver work"),
                Err(error) => error,
            };

        assert_eq!(error.kind(), IntegrationError::ForeignRouter);
        let artifact = error.into_owner();
        assert!(matches!(artifact, MountMutationArtifact::Receipt(_)));
        assert_eq!(receiver.inner.testing_snapshot(), before);
        assert!(source.reject_mount_after_mutation(artifact).is_ok());
    }

    #[test]
    fn accepted_mount_receipt_cannot_reject_the_confirming_stage() {
        let mut engine = confirmed_nested_engine();
        let receipt = pending_nested_receipt(&mut engine, 442);
        let stale = MountReceiptAttestation {
            inner: receipt.inner.clone(),
        };
        let _permit = engine.accept_mount_receipt(receipt).unwrap();
        let before = engine.inner.testing_snapshot();

        let error = match engine.reject_mount_after_mutation(MountMutationArtifact::Receipt(stale))
        {
            Ok(_) => panic!("an accepted receipt must not reject confirming work"),
            Err(error) => error,
        };

        assert_eq!(error.kind(), IntegrationError::Stale);
        assert!(matches!(
            error.into_owner(),
            MountMutationArtifact::Receipt(_)
        ));
        assert_eq!(engine.inner.testing_snapshot(), before);
    }

    #[test]
    fn foreign_mount_grant_rejection_returns_artifact_to_source_without_receiver_write() {
        let mut source = confirmed_nested_engine();
        let request = pending_mount_request(&mut source);
        let grant = source.resolve_mount(request).unwrap();
        let mut receiver = confirmed_nested_engine();
        let before = receiver.inner.testing_snapshot();

        let error = match receiver.reject_mount_after_mutation(MountMutationArtifact::Grant(grant))
        {
            Ok(_) => panic!("a foreign grant must not reject receiver work"),
            Err(error) => error,
        };

        assert_eq!(error.kind(), IntegrationError::ForeignRouter);
        let artifact = error.into_owner();
        assert_eq!(receiver.inner.testing_snapshot(), before);
        assert!(source.reject_mount_after_mutation(artifact).is_ok());
    }

    #[test]
    fn foreign_mount_permit_rejection_returns_artifact_to_source_without_receiver_write() {
        let mut source = confirmed_nested_engine();
        let (_, _, _, permit, _) = pending_mount_registration(&mut source, "/items/1", 443);
        let mut receiver = confirmed_nested_engine();
        let before = receiver.inner.testing_snapshot();

        let error = match receiver
            .reject_mount_after_mutation(MountMutationArtifact::RegistrationPermit(permit))
        {
            Ok(_) => panic!("a foreign permit must not reject receiver work"),
            Err(error) => error,
        };

        assert_eq!(error.kind(), IntegrationError::ForeignRouter);
        let artifact = error.into_owner();
        assert_eq!(receiver.inner.testing_snapshot(), before);
        assert!(source.reject_mount_after_mutation(artifact).is_ok());
    }

    #[test]
    fn foreign_mount_receipt_and_completion_are_stable_in_idle_and_terminal_receivers() {
        let mut idle_receipt_receiver = confirmed_nested_engine();
        let mut receipt_source = confirmed_nested_engine();
        let foreign_receipt = pending_nested_receipt(&mut receipt_source, 251);
        assert!(matches!(
            idle_receipt_receiver.accept_mount_receipt(foreign_receipt),
            Err(error) if error.kind() == IntegrationError::ForeignRouter
        ));

        let mut terminal_receipt_receiver = confirmed_nested_engine();
        settle_push(&mut terminal_receipt_receiver, RouteLocation::new("/"));
        let mut terminal_receipt_source = confirmed_nested_engine();
        let foreign_receipt = pending_nested_receipt(&mut terminal_receipt_source, 257);
        assert!(matches!(
            terminal_receipt_receiver.accept_mount_receipt(foreign_receipt),
            Err(error) if error.kind() == IntegrationError::ForeignRouter
        ));

        let mut idle_completion_receiver = confirmed_nested_engine();
        let mut completion_source = confirmed_nested_engine();
        let foreign_completion = pending_nested_completion(&mut completion_source, 263, 269);
        assert_eq!(
            idle_completion_receiver.complete_mount_registration(foreign_completion),
            Err(IntegrationError::ForeignRouter)
        );

        let mut terminal_completion_receiver = confirmed_nested_engine();
        settle_push(&mut terminal_completion_receiver, RouteLocation::new("/"));
        let mut terminal_completion_source = confirmed_nested_engine();
        let foreign_completion =
            pending_nested_completion(&mut terminal_completion_source, 271, 277);
        assert_eq!(
            terminal_completion_receiver.complete_mount_registration(foreign_completion),
            Err(IntegrationError::ForeignRouter)
        );
    }

    #[test]
    fn consumed_mount_receipt_and_completion_replays_are_stale_without_fallback() {
        let mut engine = confirmed_nested_engine();
        let receipt = pending_nested_receipt(&mut engine, 281);
        let receipt_replay = MountReceiptAttestation {
            inner: receipt.inner.clone(),
        };
        let permit = engine.accept_mount_receipt(receipt).unwrap();
        let after_receipt = engine.snapshot();
        assert!(matches!(
            engine.accept_mount_receipt(receipt_replay),
            Err(error) if error.kind() == IntegrationError::Stale
        ));
        assert_eq!(engine.snapshot(), after_receipt);

        let completion = permit.complete(283);
        let completion_replay = MountRegistrationCompletion {
            inner: completion.inner.clone(),
        };
        engine.complete_mount_registration(completion).unwrap();
        let after_completion = engine.snapshot();
        assert_eq!(
            engine.complete_mount_registration(completion_replay),
            Err(IntegrationError::Stale)
        );
        assert_eq!(engine.snapshot(), after_completion);
        let EngineAction::FrameTask(child) = engine.action().unwrap() else {
            panic!("normal completion must keep the child frame path")
        };
        assert_ne!(child.frame().origin(), &FrameOrigin::RouterFallback);
    }

    #[test]
    fn legacy_report_applied_reissues_nested_parent_without_lifecycle_change() {
        let mut engine = confirmed_nested_engine();
        engine.push(RouteLocation::new("/items/1")).unwrap();
        engine.collect();
        let EngineAction::FrameTask(parent) = engine.action().unwrap() else {
            panic!("parent frame task expected")
        };
        let exact = parent.report_identity();
        let before = engine.snapshot();

        assert!(matches!(
            engine.report_applied(parent),
            Err(IntegrationError::Invalid)
        ));
        assert_eq!(engine.snapshot(), before);
        let EngineAction::FrameTask(reissued) = engine.action().unwrap() else {
            panic!("legacy misuse must restore exactly one parent task")
        };
        assert!(reissued.report_identity() == exact);
    }

    #[test]
    fn outlet_aware_missing_required_key_yields_mount_request_and_failure_cleanup() {
        let mut engine = confirmed_nested_engine();
        let request = engine.push(RouteLocation::new("/items/1")).unwrap();
        engine.collect();
        let EngineAction::FrameTask(parent) = engine.action().unwrap() else {
            panic!("parent frame task expected")
        };
        let parent_frame = parent.frame().clone();

        engine
            .report_applied_with_outlets(parent, Vec::new())
            .unwrap();

        let EngineAction::MountRequest(mount) = engine.action().unwrap() else {
            panic!("outlet-aware report still yields the matched mount request")
        };
        let failure = engine
            .reject_mount(mount, MountRejectionKind::MissingOutlet)
            .unwrap();
        assert_eq!(failure.request(), request);
        assert_eq!(failure.logical_roots(), &[parent_frame.clone()]);
        assert_eq!(failure.failure().kind(), NavigationFailureKind::Mount);
        assert_eq!(
            engine.request(request).unwrap().status(),
            &RequestStatus::Running(NavigationPhase::CleanupPending)
        );

        let cleanup = engine
            .prepare_failure_cleanup(failure, CleanupManifest::new(vec![parent_frame], 181, 1))
            .unwrap();
        assert_eq!(cleanup.manifest_digest(), 181);
        assert_eq!(cleanup.obligation_count(), 1);
        assert!(
            engine
                .request(request)
                .unwrap()
                .trace()
                .iter()
                .all(|event| { event.kind() != NavigationTraceEventKind::RequestTerminal })
        );
        engine.complete_cleanup(cleanup.complete(191, 1)).unwrap();
        assert_eq!(
            engine.request(request).unwrap().status(),
            &RequestStatus::Running(NavigationPhase::Recovering)
        );
        let Some(EngineAction::FrameTask(fallback)) = engine.action() else {
            panic!("missing required outlet cleanup must issue RouterFallback")
        };
        assert_eq!(fallback.request(), request);
        assert_eq!(fallback.frame().origin(), &FrameOrigin::RouterFallback);
        let record = engine.request(request).unwrap();
        let trace = record.trace();
        let settled = trace
            .iter()
            .rposition(|event| event.kind() == NavigationTraceEventKind::CleanupSettled)
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
    fn registration_permit_is_the_only_post_receipt_rejection_capability() {
        let mut engine = confirmed_nested_engine();
        let request = engine.push(RouteLocation::new("/items/1")).unwrap();
        engine.collect();
        let EngineAction::FrameTask(parent) = engine.action().unwrap() else {
            panic!("parent frame task expected")
        };
        let parent_frame = parent.frame().clone();
        engine
            .report_applied_with_outlets(parent, vec![RouteSlotKey::default()])
            .unwrap();
        let EngineAction::MountRequest(mount) = engine.action().unwrap() else {
            panic!("mount request expected")
        };
        let grant = engine.resolve_mount(mount).unwrap();
        let receipt = grant.attest_receipt(193);
        let permit = engine.accept_mount_receipt(receipt).unwrap();

        let failure = engine
            .reject_mount_after_mutation(MountMutationArtifact::RegistrationPermit(permit))
            .unwrap();

        assert_eq!(failure.request(), request);
        assert_eq!(failure.logical_roots(), &[parent_frame.clone()]);
        let cleanup = engine
            .prepare_failure_cleanup(failure, CleanupManifest::new(vec![parent_frame], 197, 2))
            .unwrap();
        assert!(
            engine
                .request(request)
                .unwrap()
                .trace()
                .iter()
                .all(|event| event.kind() != NavigationTraceEventKind::RequestTerminal)
        );
        engine.complete_cleanup(cleanup.complete(199, 2)).unwrap();
        assert_eq!(
            engine.request(request).unwrap().status(),
            &RequestStatus::Running(NavigationPhase::Recovering)
        );
        let Some(EngineAction::FrameTask(fallback)) = engine.action() else {
            panic!("registration rejection cleanup must issue RouterFallback")
        };
        assert_eq!(fallback.request(), request);
        assert_eq!(fallback.frame().origin(), &FrameOrigin::RouterFallback);
        let record = engine.request(request).unwrap();
        let trace = record.trace();
        let settled = trace
            .iter()
            .rposition(|event| event.kind() == NavigationTraceEventKind::CleanupSettled)
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
    fn foreign_failure_cleanup_request_is_rejected_before_receiver_mutation() {
        let mut source = confirmed_nested_engine();
        source.push(RouteLocation::new("/items/1")).unwrap();
        source.collect();
        let EngineAction::FrameTask(parent) = source.action().unwrap() else {
            panic!("parent task expected")
        };
        source
            .report_applied_with_outlets(parent, Vec::new())
            .unwrap();
        let EngineAction::MountRequest(mount) = source.action().unwrap() else {
            panic!("mount request expected")
        };
        let failure = source
            .reject_mount(mount, MountRejectionKind::MissingOutlet)
            .unwrap();
        let roots = failure.logical_roots().to_vec();

        let mut receiver = confirmed_nested_engine();
        let before = receiver.snapshot();
        assert!(matches!(
            receiver.prepare_failure_cleanup(failure, CleanupManifest::new(roots, 229, 1)),
            Err(IntegrationError::ForeignRouter)
        ));
        assert_eq!(receiver.snapshot(), before);
    }

    #[test]
    fn retaining_failure_cleanup_returns_foreign_request_without_receiver_mutation() {
        let mut source = confirmed_nested_engine();
        source.push(RouteLocation::new("/items/1")).unwrap();
        source.collect();
        let EngineAction::FrameTask(parent) = source.action().unwrap() else {
            panic!("parent task expected")
        };
        source
            .report_applied_with_outlets(parent, Vec::new())
            .unwrap();
        let EngineAction::MountRequest(mount) = source.action().unwrap() else {
            panic!("mount request expected")
        };
        let failure = source
            .reject_mount(mount, MountRejectionKind::MissingOutlet)
            .unwrap();
        let request = failure.request();
        let roots = failure.logical_roots().to_vec();

        let mut receiver = confirmed_nested_engine();
        let before = receiver.snapshot();
        let mismatch = match receiver
            .prepare_failure_cleanup_retaining(failure, CleanupManifest::new(roots.clone(), 229, 1))
        {
            FailureCleanupPrepareOutcome::Mismatch(mismatch) => mismatch,
            FailureCleanupPrepareOutcome::Prepared(_)
            | FailureCleanupPrepareOutcome::Permanent(_) => {
                panic!("a foreign cleanup request must be retained as receiver mismatch")
            }
        };

        assert_eq!(mismatch.evidence.kind(), IntegrationError::ForeignRouter);
        let (retained, _) = mismatch.into_parts();
        assert_eq!(retained.request(), request);
        assert_eq!(retained.logical_roots(), roots);
        assert_eq!(receiver.snapshot(), before);
    }

    #[test]
    fn prebound_failure_cleanup_physical_rejects_crossed_request_and_returns_exact_owners() {
        let mut source = confirmed_nested_engine();
        source.push(RouteLocation::new("/items/1")).unwrap();
        source.collect();
        let EngineAction::FrameTask(source_parent) = source.action().unwrap() else {
            panic!("source parent task expected")
        };
        source
            .report_applied_with_outlets(source_parent, Vec::new())
            .unwrap();
        let EngineAction::MountRequest(source_mount) = source.action().unwrap() else {
            panic!("source mount request expected")
        };
        let source_failure = source
            .reject_mount(source_mount, MountRejectionKind::MissingOutlet)
            .unwrap();
        let source_request = source_failure.request();
        let source_roots = source_failure.logical_roots().to_vec();
        let source_before = source.snapshot();
        let physical = Box::new(0xA5_u8);
        let physical_address = (&*physical as *const u8) as usize;
        let bound = source.bind_failure_cleanup_physical(
            &source_failure,
            CleanupManifest::new(source_roots, 0xC201, 1),
            physical,
        );
        assert_eq!(source.snapshot(), source_before);

        let mut receiver = confirmed_nested_engine();
        receiver.push(RouteLocation::new("/items/1")).unwrap();
        receiver.collect();
        let EngineAction::FrameTask(receiver_parent) = receiver.action().unwrap() else {
            panic!("receiver parent task expected")
        };
        receiver
            .report_applied_with_outlets(receiver_parent, Vec::new())
            .unwrap();
        let EngineAction::MountRequest(receiver_mount) = receiver.action().unwrap() else {
            panic!("receiver mount request expected")
        };
        let receiver_failure = receiver
            .reject_mount(receiver_mount, MountRejectionKind::MissingOutlet)
            .unwrap();
        let receiver_request = receiver_failure.request();
        let receiver_roots = receiver_failure.logical_roots().to_vec();
        let before = receiver.snapshot();

        let mismatch = match receiver.prepare_failure_cleanup_with_physical(
            receiver_failure,
            CleanupManifest::new(receiver_roots.clone(), 0xC202, 1),
            bound,
        ) {
            FailureCleanupPhysicalPrepareOutcome::Mismatch(mismatch) => mismatch,
            FailureCleanupPhysicalPrepareOutcome::Prepared(_)
            | FailureCleanupPhysicalPrepareOutcome::Permanent(_) => {
                panic!("a request-A physical binding must reject request B")
            }
        };
        assert_ne!(source_request, receiver_request);
        assert_eq!(mismatch.bound_request(), source_request);
        let (retained, evidence, physical) = mismatch.into_parts();
        assert_eq!(retained.request(), receiver_request);
        assert_eq!(retained.logical_roots(), receiver_roots);
        assert_eq!(evidence.kind(), IntegrationError::ForeignRouter);
        assert_eq!(physical.request(), source_request);
        assert_eq!(
            (&**physical.physical() as *const u8) as usize,
            physical_address
        );
        assert_eq!(receiver.snapshot(), before);
    }

    #[test]
    fn retaining_failure_cleanup_returns_invalid_manifest_request_without_engine_mutation() {
        let mut engine = confirmed_nested_engine();
        engine.push(RouteLocation::new("/items/1")).unwrap();
        engine.collect();
        let EngineAction::FrameTask(parent) = engine.action().unwrap() else {
            panic!("parent task expected")
        };
        engine
            .report_applied_with_outlets(parent, Vec::new())
            .unwrap();
        let EngineAction::MountRequest(mount) = engine.action().unwrap() else {
            panic!("mount request expected")
        };
        let failure = engine
            .reject_mount(mount, MountRejectionKind::MissingOutlet)
            .unwrap();
        let request = failure.request();
        let roots = failure.logical_roots().to_vec();
        let before = engine.snapshot();

        let permanent = match engine
            .prepare_failure_cleanup_retaining(failure, CleanupManifest::new(Vec::new(), 237, 1))
        {
            FailureCleanupPrepareOutcome::Permanent(permanent) => permanent,
            FailureCleanupPrepareOutcome::Prepared(_)
            | FailureCleanupPrepareOutcome::Mismatch(_) => {
                panic!("a logical-root contradiction must be permanently classified")
            }
        };

        let PermanentFailureCleanup {
            request: retained,
            evidence,
        } = permanent;
        assert_eq!(retained.request(), request);
        assert_eq!(retained.logical_roots(), roots);
        assert_eq!(evidence.expected_logical_roots(), roots);
        assert!(evidence.observed_logical_roots().is_empty());
        assert_eq!(evidence.submitted_obligation_count(), 1);
        assert_eq!(engine.snapshot(), before);
    }

    #[test]
    fn mount_wrappers_consume_only_sealed_grant_and_registration_permit_artifacts() {
        let mut ids = CoreIdAllocator::new().unwrap();
        let router = ids.allocate_router_identity().unwrap();
        let mut admission = HostAdmissionIssuer::new().unwrap();
        let host_token = admission.issue(router).unwrap();
        let host_grant = admission.grant(&host_token, &mut ids).unwrap();
        let host = admission.confirm(&host_grant).unwrap();
        let request = ids.allocate_request(router).unwrap();
        let transaction = ids.allocate_transaction().unwrap();
        let parent = RouteFrameVersion::new(
            router,
            host.version().host(),
            host.top_slot(),
            FrameOrigin::RouterFallback,
            ids.allocate_frame().unwrap(),
            RouteGeneration::initial(),
        );
        let outlet = crate::slot::RouteSlotKey::named("details").unwrap();
        let mut issuer = MountIssuer::new(router);
        let request_token = issuer
            .issue(
                &mut ids,
                host.version().clone(),
                request,
                transaction,
                parent.clone(),
                outlet.clone(),
            )
            .unwrap();
        let core_grant = issuer.grant(&request_token, &mut ids).unwrap();

        let grant = MountGrant::sealed(core_grant);
        let mut foreign_ids = CoreIdAllocator::new().unwrap();
        let foreign_parent = RouteFrameVersion::new(
            foreign_ids.allocate_router_identity().unwrap(),
            foreign_ids.allocate_host().unwrap(),
            foreign_ids.allocate_slot().unwrap(),
            FrameOrigin::RouterFallback,
            foreign_ids.allocate_frame().unwrap(),
            RouteGeneration::initial(),
        );
        assert_eq!(parent.router(), foreign_parent.router());
        assert_eq!(parent.host(), foreign_parent.host());
        assert!(mount_grant_matches_target(&grant, &parent, &outlet));
        assert!(!mount_grant_matches_target(
            &grant,
            &parent,
            &crate::slot::RouteSlotKey::named("other").unwrap()
        ));
        assert!(!mount_grant_matches_target(
            &grant,
            &foreign_parent,
            &outlet
        ));
        let slot = grant.slot();
        let receipt = grant.attest_receipt(67);
        let core_permit = issuer.accept_receipt(&receipt.inner).unwrap();
        let permit = MountRegistrationPermit::sealed(core_permit);
        assert_eq!(permit.slot(), slot);

        let completion = permit.complete(71);
        let confirmation = issuer.complete_registration(&completion.inner).unwrap();
        assert_eq!(confirmation.slot(), slot);
        assert!(confirmation.has_valid_binding());
    }

    #[test]
    fn frame_router_match_uses_opaque_identity_not_raw_projection() {
        let mut first = CoreIdAllocator::new().unwrap();
        let first_router = first.allocate_router_identity().unwrap();
        let first_frame = RouteFrameVersion::new(
            first_router,
            first.allocate_host().unwrap(),
            first.allocate_slot().unwrap(),
            FrameOrigin::RouterFallback,
            first.allocate_frame().unwrap(),
            RouteGeneration::initial(),
        );
        let same_router_frame = RouteFrameVersion::new(
            first_router,
            first.allocate_host().unwrap(),
            first.allocate_slot().unwrap(),
            FrameOrigin::RouterFallback,
            first.allocate_frame().unwrap(),
            RouteGeneration::initial(),
        );
        let mut second = CoreIdAllocator::new().unwrap();
        let foreign_frame = RouteFrameVersion::new(
            second.allocate_router_identity().unwrap(),
            second.allocate_host().unwrap(),
            second.allocate_slot().unwrap(),
            FrameOrigin::RouterFallback,
            second.allocate_frame().unwrap(),
            RouteGeneration::initial(),
        );

        assert_eq!(first_frame.router(), foreign_frame.router());
        assert!(frame_versions_share_router(
            &first_frame,
            &same_router_frame
        ));
        assert!(!frame_versions_share_router(&first_frame, &foreign_frame));
    }

    #[test]
    fn host_version_shares_router_with_frame_rejects_cross_world_raw_collision() {
        let mut first_ids = CoreIdAllocator::new().unwrap();
        let first_router = first_ids.allocate_router_identity().unwrap();
        let mut exact_admission = HostAdmissionIssuer::new().unwrap();
        let exact_token = exact_admission.issue(first_router).unwrap();
        let exact_grant = exact_admission.grant(&exact_token, &mut first_ids).unwrap();
        let exact_version = exact_grant.version().clone();
        let exact_frame = RouteFrameVersion::new(
            first_router,
            exact_version.host(),
            exact_grant.top_slot(),
            FrameOrigin::RouterFallback,
            first_ids.allocate_frame().unwrap(),
            RouteGeneration::initial(),
        );

        assert!(host_version_matches_frame(&exact_version, &exact_frame));
        assert!(host_version_shares_router_with_frame(
            &exact_version,
            &exact_frame
        ));

        let mut other_host_admission = HostAdmissionIssuer::new().unwrap();
        let other_host_token = other_host_admission.issue(first_router).unwrap();
        let other_host_grant = other_host_admission
            .grant(&other_host_token, &mut first_ids)
            .unwrap();
        let other_host_frame = RouteFrameVersion::new(
            first_router,
            other_host_grant.version().host(),
            other_host_grant.top_slot(),
            FrameOrigin::RouterFallback,
            first_ids.allocate_frame().unwrap(),
            RouteGeneration::initial(),
        );

        assert_eq!(
            exact_version.router_identity(),
            other_host_frame.router_identity()
        );
        assert_ne!(exact_version.host(), other_host_frame.host());
        assert!(!host_version_matches_frame(
            &exact_version,
            &other_host_frame
        ));

        let mut foreign_ids = CoreIdAllocator::new().unwrap();
        let foreign_router = foreign_ids.allocate_router_identity().unwrap();
        let mut foreign_admission = HostAdmissionIssuer::new().unwrap();
        let foreign_token = foreign_admission.issue(foreign_router).unwrap();
        let foreign_grant = foreign_admission
            .grant(&foreign_token, &mut foreign_ids)
            .unwrap();
        let foreign_version = foreign_grant.version().clone();

        assert_eq!(foreign_version.router().raw(), exact_frame.router().raw());
        assert_eq!(foreign_version.host().raw(), exact_frame.host().raw());
        assert_ne!(
            foreign_version.router_identity(),
            exact_frame.router_identity()
        );
        assert!(!host_version_matches_frame(&foreign_version, &exact_frame));
        assert!(!host_version_shares_router_with_frame(
            &foreign_version,
            &exact_frame
        ));
        assert!(host_version_shares_router_with_frame(
            &exact_version,
            &other_host_frame
        ));
    }

    fn ready_engine() -> (RouterEngine, HostGrant) {
        let view = RouteViewKey::new("root").unwrap();
        let fallback = RouteViewKey::new("fallback").unwrap();
        let mut engine = RouterEngine::install(tree(&view), fallback).unwrap();
        let token = engine.request_host_admission().unwrap();
        let grant = engine.grant_host_admission(&token).unwrap();
        (engine, grant)
    }

    fn confirmed_engine() -> RouterEngine {
        let (mut engine, grant) = ready_engine();
        engine.record_top_slot_registered(&grant).unwrap();
        engine.confirm_host(grant);
        engine
    }

    fn settle_push(engine: &mut RouterEngine, location: RouteLocation) -> NavigationRequestId {
        let request = engine.push(location).unwrap();
        engine.collect();
        let ticket = match engine.action().unwrap() {
            EngineAction::FrameTask(task) => engine.report_applied(task).unwrap(),
            EngineAction::MountRequest(_) => panic!("helper expects a depth-one route"),
            EngineAction::FailureCleanup(_) => {
                panic!("depth-one helper cannot hold nested failure cleanup")
            }
            EngineAction::Commit(ticket) => ticket,
            EngineAction::BackResolution(_) => {
                panic!("push helper cannot receive unresolved back resolution")
            }
        };
        let prepared = engine
            .prepare_commit(&ticket, CleanupManifest::empty())
            .unwrap();
        let cleanup = prepared.cleanup().clone();
        let validated = engine.validate_publication(prepared).unwrap();
        validated.publish(|| {});
        engine.complete_cleanup(cleanup.complete_empty()).unwrap();
        request
    }

    fn settle_changed_history_push(
        engine: &mut RouterEngine,
        location: RouteLocation,
    ) -> NavigationRequestId {
        let request = engine.push(location).unwrap();
        engine.collect();
        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("changed history location must issue one replacement frame task")
        };
        let ticket = engine.report_applied(task).unwrap();
        let logical_roots = engine
            .reconciliation_projection_for_commit(&ticket)
            .unwrap()
            .leaving()
            .map(|leaving| leaving.version().clone())
            .collect::<Vec<_>>();
        assert_eq!(
            logical_roots.len(),
            1,
            "the depth-one changed location has one real logical leaving root"
        );
        let manifest_digest = 0x4144_4d49_5353_494f;
        let obligation_count = logical_roots.len();
        let prepared = engine
            .prepare_commit(
                &ticket,
                CleanupManifest::new(logical_roots, manifest_digest, obligation_count),
            )
            .unwrap();
        let cleanup = prepared.cleanup().clone();
        assert_eq!(cleanup.manifest_digest(), manifest_digest);
        assert_eq!(cleanup.obligation_count(), obligation_count);
        let validated = engine.validate_publication(prepared).unwrap();
        validated.publish(|| {});
        engine
            .observe_cleanup_receipt(&cleanup, CleanupDisposition::Removed)
            .unwrap();
        engine
            .complete_cleanup(cleanup.complete(0x4144_4d49_5353_444f, obligation_count))
            .unwrap();
        request
    }

    #[derive(Clone, Copy)]
    enum HistoryFixtureCommand {
        Push,
        Replace,
    }

    fn settle_nested_history_command(
        engine: &mut RouterEngine,
        command: HistoryFixtureCommand,
        location: RouteLocation,
    ) -> NavigationRequestId {
        let request = match command {
            HistoryFixtureCommand::Push => engine.push(location).unwrap(),
            HistoryFixtureCommand::Replace => engine.replace(location).unwrap(),
        };
        engine.collect();
        loop {
            match engine
                .action()
                .expect("a running nested fixture retains one real action")
            {
                EngineAction::FrameTask(task) => {
                    let outlets = engine
                        .expected_child_outlet(&task)
                        .unwrap()
                        .into_iter()
                        .collect();
                    engine.report_applied_with_outlets(task, outlets).unwrap();
                }
                EngineAction::MountRequest(mount) => {
                    let receipt = engine.resolve_mount(mount).unwrap().attest_receipt(0x51);
                    let completion = engine.accept_mount_receipt(receipt).unwrap().complete(0x53);
                    engine.complete_mount_registration(completion).unwrap();
                }
                EngineAction::Commit(ticket) => {
                    let logical_roots = engine
                        .reconciliation_projection_for_commit(&ticket)
                        .unwrap()
                        .leaving()
                        .map(|leaving| leaving.version().clone())
                        .collect::<Vec<_>>();
                    let obligation_count = logical_roots.len();
                    let manifest = if obligation_count == 0 {
                        CleanupManifest::empty()
                    } else {
                        CleanupManifest::new(logical_roots, 0x4241_434b_4e45_5354, obligation_count)
                    };
                    let prepared = engine.prepare_commit(&ticket, manifest).unwrap();
                    let cleanup = prepared.cleanup().clone();
                    let validated = engine.validate_publication(prepared).unwrap();
                    validated.publish(|| {});
                    let completion = if obligation_count == 0 {
                        cleanup.complete_empty()
                    } else {
                        cleanup.complete(0x4241_434b_444f_4e45, obligation_count)
                    };
                    engine.complete_cleanup(completion).unwrap();
                    return request;
                }
                EngineAction::FailureCleanup(_) => {
                    panic!("a successful nested history fixture cannot enter failure cleanup")
                }
                EngineAction::BackResolution(_) => {
                    panic!("push and replace fixtures cannot produce Back resolution")
                }
            }
        }
    }

    fn nested_mount_back_fixture() -> RouterEngine {
        let mut engine = confirmed_back_target_nested_engine();
        settle_nested_history_command(
            &mut engine,
            HistoryFixtureCommand::Push,
            RouteLocation::new("/items/1"),
        );
        settle_nested_history_command(
            &mut engine,
            HistoryFixtureCommand::Push,
            RouteLocation::new("/"),
        );
        settle_nested_history_command(
            &mut engine,
            HistoryFixtureCommand::Replace,
            RouteLocation::new("/items"),
        );
        engine
    }

    fn nested_bound_mount_back_fixture() -> (RouterEngine, crate::ids::RouteId, RouteViewKey) {
        let (mut engine, items_route, items_error_view) =
            confirmed_back_target_bound_nested_engine();
        settle_nested_history_command(
            &mut engine,
            HistoryFixtureCommand::Push,
            RouteLocation::new("/items/1"),
        );
        settle_nested_history_command(
            &mut engine,
            HistoryFixtureCommand::Push,
            RouteLocation::new("/"),
        );
        settle_nested_history_command(
            &mut engine,
            HistoryFixtureCommand::Replace,
            RouteLocation::new("/items"),
        );
        (engine, items_route, items_error_view)
    }

    #[cfg(feature = "test-support")]
    #[test]
    fn back_core_allocator_failure_is_post_refreeze_private_zero_write() {
        use crate::test_support::{BackCapacityOrderEvent, issue_back_capacity_order_channels};

        for counter in TestingReservationCounter::ALL {
            let mut engine = confirmed_engine();
            let (observer, _issuer, core, _facade) = issue_back_capacity_order_channels();
            engine
                .prepare_back_capacity_core_producer_slot()
                .expect("each fresh engine has one vacant sealed Core producer slot")
                .install(core);
            settle_push(
                &mut engine,
                RouteLocation::new("/").with_fragment("back-target"),
            );
            settle_changed_history_push(
                &mut engine,
                RouteLocation::new("/").with_fragment("back-source"),
            );
            assert!(engine.snapshot().history().can_back());

            let world = engine
                .claim_history_admission_world()
                .expect("the fresh confirmed engine issues one exact world seal");
            let stamp = engine
                .history_admission_stamp(&world)
                .expect("the settled history publishes can_back=true");
            let admission = engine
                .prepare_back_admission(&world, &stamp)
                .expect("the exact live history verifies Back admission");

            engine.testing_exhaust_reservation_counter(counter);
            let before = engine.inner.testing_snapshot();
            let error = match engine.enqueue_back(admission) {
                Ok(_) => panic!("a real exhausted reservation counter cannot accept Back"),
                Err(error) => error,
            };

            assert_eq!(error, EnqueueBackErrorKind::Exhausted);
            assert_eq!(engine.inner.testing_snapshot(), before);
            assert_eq!(
                observer.snapshot().events(),
                &[
                    (0, BackCapacityOrderEvent::CoreAllocatorAttempt),
                    (1, BackCapacityOrderEvent::CoreAllocatorFailure),
                ]
            );
        }

        let mut stale = confirmed_engine();
        let (observer, _issuer, core, _facade) = issue_back_capacity_order_channels();
        stale
            .prepare_back_capacity_core_producer_slot()
            .expect("the separate fresh engine has one vacant sealed Core producer slot")
            .install(core);
        settle_push(
            &mut stale,
            RouteLocation::new("/").with_fragment("stale-back-target"),
        );
        settle_changed_history_push(
            &mut stale,
            RouteLocation::new("/").with_fragment("stale-back-source"),
        );
        assert!(stale.snapshot().history().can_back());
        let world = stale
            .claim_history_admission_world()
            .expect("the separate engine issues its exact world seal");
        let stamp = stale
            .history_admission_stamp(&world)
            .expect("the separate settled history publishes can_back=true");
        let admission = stale
            .prepare_back_admission(&world, &stamp)
            .expect("the exact pre-drift history verifies Back admission");

        settle_changed_history_push(
            &mut stale,
            RouteLocation::new("/").with_fragment("new-live-current"),
        );
        let before = stale.inner.testing_snapshot();
        let error = match stale.enqueue_back(admission) {
            Ok(_) => panic!("a stale Back admission token cannot reserve allocator capacity"),
            Err(error) => error,
        };

        assert_eq!(error, EnqueueBackErrorKind::AuthorityRecovering);
        assert_eq!(stale.inner.testing_snapshot(), before);
        assert!(observer.snapshot().events().is_empty());
    }

    #[test]
    fn admission_stamp_cannot_authorize_or_reconstruct_target() {
        let mut engine = confirmed_engine();
        settle_push(&mut engine, RouteLocation::new("/").with_fragment("first"));
        settle_changed_history_push(
            &mut engine,
            RouteLocation::new("/").with_fragment("published-current"),
        );
        let published_preview = RouteLocation::new("/").with_fragment("first");
        let world = engine
            .claim_history_admission_world()
            .expect("an installed engine must issue one opaque world seal");
        let stamp = engine
            .history_admission_stamp(&world)
            .expect("published can_back=true must produce an admission-only stamp");

        settle_changed_history_push(
            &mut engine,
            RouteLocation::new("/").with_fragment("new-live-current"),
        );
        let expected_live_target = RouteLocation::new("/").with_fragment("published-current");
        let admission = engine
            .prepare_back_admission(&world, &stamp)
            .expect("a lagging same-provenance positive stamp still admits back");
        let accepted = engine
            .enqueue_back(admission)
            .expect("live history, not the stamp, reserves the back request");

        assert_eq!(
            engine.request(accepted.request()).unwrap().requested(),
            None
        );
        engine.collect();
        assert_eq!(
            engine.request(accepted.request()).unwrap().requested(),
            Some(&expected_live_target)
        );
        assert_ne!(expected_live_target, published_preview);
    }

    #[test]
    fn back_resolution_evidence_classifies_current_record_and_foreign_engine() {
        let mut exact = confirmed_engine();
        settle_push(
            &mut exact,
            RouteLocation::new("/").with_fragment("back-target"),
        );
        settle_changed_history_push(
            &mut exact,
            RouteLocation::new("/").with_fragment("back-source"),
        );
        let world = exact
            .claim_history_admission_world()
            .expect("the exact engine issues one real world seal");
        let stamp = exact
            .history_admission_stamp(&world)
            .expect("the exact published history admits back");
        let admission = exact
            .prepare_back_admission(&world, &stamp)
            .expect("the exact live authority verifies admission");
        let _accepted = exact
            .enqueue_back(admission)
            .expect("the exact engine reserves one real back branch");

        exact.collect();
        let evidence = match exact.action() {
            Some(EngineAction::BackResolution(handoff)) => handoff.evidence(),
            Some(_) | None => panic!("the accepted request must yield resolution evidence"),
        };

        assert_eq!(
            exact.classify_back_resolution_evidence(&evidence),
            BackResolutionEvidenceClassification::Current
        );

        let foreign = confirmed_engine();
        assert_eq!(
            foreign.classify_back_resolution_evidence(&evidence),
            BackResolutionEvidenceClassification::Foreign
        );
    }

    #[test]
    fn accepted_back_view_exact_lineage_rejects_cross_scope() {
        let mut exact = confirmed_engine();
        settle_push(
            &mut exact,
            RouteLocation::new("/").with_fragment("back-target"),
        );
        settle_changed_history_push(
            &mut exact,
            RouteLocation::new("/").with_fragment("back-source"),
        );
        let world = exact
            .claim_history_admission_world()
            .expect("the exact engine issues one real world seal");
        let stamp = exact
            .history_admission_stamp(&world)
            .expect("the exact published history admits back");
        let admission = exact
            .prepare_back_admission(&world, &stamp)
            .expect("the exact live authority verifies admission");
        let accepted = exact
            .enqueue_back(admission)
            .expect("the exact engine reserves one real back branch");
        let request = accepted.request();
        let (_, accepted) = accepted.into_parts();

        exact.collect();
        let Some(EngineAction::BackResolution(handoff)) = exact.action() else {
            panic!("the accepted request must yield its real raw resolution handoff")
        };
        let saved_evidence = handoff.evidence();
        assert_eq!(
            exact.classify_back_resolution_evidence(&saved_evidence),
            BackResolutionEvidenceClassification::Current
        );

        let mut foreign = confirmed_engine();
        let (accepted, handoff, mismatch_evidence) =
            match foreign.verify_back_resolution_handoff(accepted, handoff) {
                BackResolutionVerifyOutcome::Mismatch {
                    accepted,
                    handoff,
                    evidence,
                } => (accepted, handoff, evidence),
                BackResolutionVerifyOutcome::Matched(_) => {
                    panic!("a foreign engine must not verify accepted back lineage")
                }
            };
        assert_eq!(
            exact.classify_back_resolution_evidence(&saved_evidence),
            BackResolutionEvidenceClassification::Current
        );
        assert_eq!(
            exact.classify_back_resolution_evidence(&mismatch_evidence),
            BackResolutionEvidenceClassification::Foreign
        );
        assert_eq!(
            foreign.classify_back_resolution_evidence(&mismatch_evidence),
            BackResolutionEvidenceClassification::Foreign
        );

        let before = exact
            .inner
            .testing_snapshot()
            .without_back_resolution_state_for(request);
        match exact.verify_back_resolution_handoff(accepted, handoff) {
            BackResolutionVerifyOutcome::Matched(verified) => {
                let (_accepted, _verified_owner) = verified.into_parts();
            }
            BackResolutionVerifyOutcome::Mismatch { .. } => {
                panic!("the exact engine must verify the original accepted and raw owners")
            }
        }
        let after = exact
            .inner
            .testing_snapshot()
            .without_back_resolution_state_for(request);
        assert_eq!(after, before);
        assert_eq!(
            exact.classify_back_resolution_evidence(&saved_evidence),
            BackResolutionEvidenceClassification::ConsumedReplay
        );
    }

    fn take_verified_back_target(exact: &mut RouterEngine) -> VerifiedBackTargetExecutionOwner {
        let world = exact
            .claim_history_admission_world()
            .expect("the exact engine issues one real world seal");
        let stamp = exact
            .history_admission_stamp(&world)
            .expect("the exact published history admits back");
        let admission = exact
            .prepare_back_admission(&world, &stamp)
            .expect("the exact live authority verifies admission");
        let accepted = exact
            .enqueue_back(admission)
            .expect("the exact engine reserves one real back branch");
        let (_, accepted) = accepted.into_parts();

        exact.collect();
        let Some(EngineAction::BackResolution(handoff)) = exact.action() else {
            panic!("the accepted request must yield its real raw resolution handoff")
        };
        let verified = match exact.verify_back_resolution_handoff(accepted, handoff) {
            BackResolutionVerifyOutcome::Matched(verified) => verified,
            BackResolutionVerifyOutcome::Mismatch { .. } => {
                panic!("the exact engine must verify the accepted view and raw handoff")
            }
        };
        let (_, owner) = verified.into_parts();
        match owner {
            VerifiedBackResolutionOwnerHandoff::Target(owner) => owner,
            VerifiedBackResolutionOwnerHandoff::NoTarget(_) => {
                panic!("two published history entries must resolve one live predecessor")
            }
            VerifiedBackResolutionOwnerHandoff::Contradiction(_) => {
                panic!("an unchanged live predecessor must not contradict its accepted branch")
            }
        }
    }

    fn take_refrozen_back_no_target_candidate() -> (RouterEngine, VerifiedBackNoTargetCandidate) {
        take_refrozen_back_no_target_candidate_from(confirmed_engine())
    }

    fn take_refrozen_back_no_target_candidate_with_lease()
    -> (RouterEngine, VerifiedBackNoTargetCandidate, HostLease) {
        let (mut engine, grant) = ready_engine();
        engine.record_top_slot_registered(&grant).unwrap();
        let lease = engine.confirm_host(grant);
        let (engine, candidate) = take_refrozen_back_no_target_candidate_from(engine);
        (engine, candidate, lease)
    }

    fn take_refrozen_back_no_target_candidate_from(
        engine: RouterEngine,
    ) -> (RouterEngine, VerifiedBackNoTargetCandidate) {
        let (engine, candidate, _world) =
            take_refrozen_back_no_target_candidate_from_with_world(engine);
        (engine, candidate)
    }

    fn take_refrozen_back_no_target_candidate_from_with_world(
        mut engine: RouterEngine,
    ) -> (
        RouterEngine,
        VerifiedBackNoTargetCandidate,
        HistoryAdmissionWorldSeal,
    ) {
        settle_push(
            &mut engine,
            RouteLocation::new("/").with_fragment("logical-zero-target"),
        );
        settle_changed_history_push(
            &mut engine,
            RouteLocation::new("/").with_fragment("logical-zero-source"),
        );
        let world = engine
            .claim_history_admission_world()
            .expect("the exact engine issues one real world seal");
        let stamp = engine
            .history_admission_stamp(&world)
            .expect("the two-entry history admits Back");
        let first_admission = engine
            .prepare_back_admission(&world, &stamp)
            .expect("the first Back exact-refreezes the live history");
        let second_admission = engine
            .prepare_back_admission(&world, &stamp)
            .expect("the second Back shares the same pre-publication source");
        let first_accepted = engine
            .enqueue_back(first_admission)
            .expect("the first live Back reserves one real branch");
        let second_accepted = engine
            .enqueue_back(second_admission)
            .expect("the second live Back reserves one real branch");

        engine.collect();
        let Some(EngineAction::BackResolution(first_handoff)) = engine.action() else {
            panic!("the first accepted Back must produce a real resolution handoff")
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
        let first_target = match first_owner {
            VerifiedBackResolutionOwnerHandoff::Target(owner) => owner,
            VerifiedBackResolutionOwnerHandoff::NoTarget(_)
            | VerifiedBackResolutionOwnerHandoff::Contradiction(_) => {
                panic!("the first Back from a two-entry history must resolve its predecessor")
            }
        };
        let first_resume = match engine.resume_back_target_with_retirement(first_target, || {}) {
            BackTargetCoreResumeOutcome::Resumed(BackTargetStart::Ready(resume)) => resume,
            BackTargetCoreResumeOutcome::Resumed(BackTargetStart::FailureCleanup(_)) => {
                panic!("the real predecessor must reach a normal target action")
            }
            BackTargetCoreResumeOutcome::Permanent(_)
            | BackTargetCoreResumeOutcome::Mismatch { .. } => {
                panic!("the exact first Back target must resume")
            }
        };
        let (action, _execution, _continuation) = first_resume.into_parts();
        let BackTargetResumeAction::FrameTask(task) = action else {
            panic!("the changed depth-one Back target must issue one real frame task")
        };
        let ticket = engine
            .report_applied(task)
            .expect("the resumed Back target must accept its real frame report");
        let logical_roots = engine
            .reconciliation_projection_for_commit(&ticket)
            .expect("the resumed Back target retains its exact reconciliation projection")
            .leaving()
            .map(|leaving| leaving.version().clone())
            .collect::<Vec<_>>();
        let obligation_count = logical_roots.len();
        let manifest = if obligation_count == 0 {
            CleanupManifest::empty()
        } else {
            CleanupManifest::new(logical_roots, 0x4c4f_4749_4341_4c30, obligation_count)
        };
        let prepared = engine
            .prepare_commit(&ticket, manifest)
            .expect("the resumed Back target prepares its real cleanup manifest");
        let cleanup = prepared.cleanup().clone();
        let validated = engine
            .validate_publication(prepared)
            .expect("the resumed Back target validates its real publication");
        validated.publish(|| {});
        if obligation_count != 0 {
            engine
                .observe_cleanup_receipt(&cleanup, CleanupDisposition::Removed)
                .expect("the resumed Back target records its real cleanup receipt");
        }
        let completion = if obligation_count == 0 {
            cleanup.complete_empty()
        } else {
            cleanup.complete(0x4c4f_4749_4341_4c31, obligation_count)
        };
        engine
            .complete_cleanup(completion)
            .expect("the resumed Back target settles before the queued no-target Back resolves");

        let Some(EngineAction::BackResolution(second_handoff)) = engine.action() else {
            panic!("the queued second Back must resolve after the first terminal")
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
        let candidate = match engine.refreeze_back_no_target_source_with_transfer(candidate, || {})
        {
            BackNoTargetSourceRefreezeOutcome::Unchanged(candidate) => candidate,
            BackNoTargetSourceRefreezeOutcome::Drift(_)
            | BackNoTargetSourceRefreezeOutcome::Mismatch { .. } => {
                panic!("the exact no-target source must refreeze before logical-zero approval")
            }
        };

        (engine, candidate, world)
    }

    fn take_accepted_back_no_target_authority() -> (
        RouterEngine,
        NavigationRequestId,
        HostLease,
        BackNoTargetAuthoritySettlementOwner,
    ) {
        let (mut engine, candidate, lease) = take_refrozen_back_no_target_candidate_with_lease();
        let request = candidate.evidence().request();
        let callback_calls = std::cell::Cell::new(0usize);
        let owner = match engine
            .accept_back_no_target_authority_lost_with_retirement(candidate, || {
                callback_calls.set(callback_calls.get() + 1)
            }) {
            BackNoTargetAuthorityCoreOutcome::Accepted(owner) => owner,
            BackNoTargetAuthorityCoreOutcome::Mismatch { .. } => {
                panic!("the exact no-target authority candidate must be accepted")
            }
        };
        assert_eq!(callback_calls.get(), 1);
        (engine, request, lease, owner)
    }

    fn exact_back_no_target_authority_begin_owner(
        engine: &RouterEngine,
        lease: HostLease,
        settlement: BackNoTargetAuthoritySettlementOwner,
        manifest_digest: u64,
    ) -> (BackNoTargetAuthorityBeginOwner, usize) {
        let obligation_count = engine.active().frames().len();
        let roots = engine.active().frames().iter().rev().cloned().collect();
        (
            BackNoTargetAuthorityBeginOwner::new(
                settlement,
                lease,
                BackNoTargetAuthorityManifest::new(CleanupManifest::new(
                    roots,
                    manifest_digest,
                    obligation_count,
                )),
            ),
            obligation_count,
        )
    }

    fn begin_exact_back_no_target_authority() -> (
        RouterEngine,
        NavigationRequestId,
        BackNoTargetAuthorityCleanupBatchOwner,
        u64,
        usize,
    ) {
        let (mut engine, request, lease, settlement) = take_accepted_back_no_target_authority();
        let manifest_digest = 0x4155_5448_4e4f_5441;
        let (owner, obligation_count) =
            exact_back_no_target_authority_begin_owner(&engine, lease, settlement, manifest_digest);
        let installed = std::cell::RefCell::new(None);
        assert!(
            engine.inner.testing_back_no_target_authority_is_accepted(),
            "begin must start from the private Accepted state"
        );
        match engine.begin_back_no_target_authority_settlement(owner, |batch| {
            installed.replace(Some(batch));
        }) {
            BackNoTargetAuthorityBeginOutcome::Begun => {}
            BackNoTargetAuthorityBeginOutcome::ReceiverMismatch { .. }
            | BackNoTargetAuthorityBeginOutcome::Retryable { .. }
            | BackNoTargetAuthorityBeginOutcome::Permanent { .. } => {
                panic!("the exact begin owner must enter dedicated authority cleanup")
            }
        }
        assert!(engine.inner.testing_back_no_target_authority_is_cleanup());
        (
            engine,
            request,
            installed
                .into_inner()
                .expect("begin must hand its unique batch to ECS"),
            manifest_digest,
            obligation_count,
        )
    }

    #[test]
    fn back_no_target_authority_accept_is_exact_and_installs_only_private_accepted() {
        let (mut exact, candidate, _lease) = take_refrozen_back_no_target_candidate_with_lease();
        let mut foreign = confirmed_engine();
        let exact_before = exact.snapshot();
        let foreign_before = foreign.snapshot();
        let foreign_calls = std::cell::Cell::new(0usize);
        let (candidate, mismatch) = match foreign
            .accept_back_no_target_authority_lost_with_retirement(candidate, || {
                foreign_calls.set(foreign_calls.get() + 1)
            }) {
            BackNoTargetAuthorityCoreOutcome::Mismatch {
                candidate,
                mismatch,
            } => (candidate, mismatch),
            BackNoTargetAuthorityCoreOutcome::Accepted(_) => {
                panic!("a foreign engine must not consume the candidate")
            }
        };
        assert_eq!(foreign_calls.get(), 0);
        assert!(matches!(
            mismatch.inner,
            CorePreparationReceiverMismatchProjection::LogicalZero { .. }
        ));
        assert_eq!(exact.snapshot(), exact_before);
        assert_eq!(foreign.snapshot(), foreign_before);

        let exact_calls = std::cell::Cell::new(0usize);
        let owner = match exact
            .accept_back_no_target_authority_lost_with_retirement(candidate, || {
                exact_calls.set(exact_calls.get() + 1)
            }) {
            BackNoTargetAuthorityCoreOutcome::Accepted(owner) => owner,
            BackNoTargetAuthorityCoreOutcome::Mismatch { .. } => {
                panic!("the returned candidate must remain exact for its own engine")
            }
        };
        assert_eq!(exact_calls.get(), 1);
        assert_eq!(exact.snapshot(), exact_before);
        assert!(exact.inner.testing_back_no_target_authority_is_accepted());
        drop(owner);
    }

    #[test]
    fn back_no_target_authority_begin_foreign_receiver_returns_whole_owner() {
        let (mut exact, request, lease, settlement) = take_accepted_back_no_target_authority();
        let manifest_digest = 0x4155_5448_464f_5245;
        let (owner, obligation_count) =
            exact_back_no_target_authority_begin_owner(&exact, lease, settlement, manifest_digest);
        let mut foreign = confirmed_engine();
        let foreign_before = foreign.snapshot();
        let callback_calls = std::cell::Cell::new(0usize);

        let owner = match foreign.begin_back_no_target_authority_settlement(owner, |_| {
            callback_calls.set(callback_calls.get() + 1);
        }) {
            BackNoTargetAuthorityBeginOutcome::ReceiverMismatch { owner, mismatch } => {
                assert!(matches!(
                    mismatch.inner,
                    CorePreparationReceiverMismatchProjection::BackNoTargetAuthorityBegin { .. }
                ));
                owner
            }
            BackNoTargetAuthorityBeginOutcome::Begun
            | BackNoTargetAuthorityBeginOutcome::Retryable { .. }
            | BackNoTargetAuthorityBeginOutcome::Permanent { .. } => {
                panic!("a foreign receiver must return the whole begin owner")
            }
        };

        assert_eq!(callback_calls.get(), 0);
        assert_eq!(foreign.snapshot(), foreign_before);
        assert!(exact.inner.testing_back_no_target_authority_is_accepted());
        assert_eq!(
            exact.request(request).unwrap().status(),
            &RequestStatus::Queued
        );

        let (settlement, lease, manifest) = owner.into_parts();
        assert_eq!(manifest.cleanup.manifest_digest, manifest_digest);
        assert_eq!(manifest.cleanup.obligation_count, obligation_count);
        let owner = BackNoTargetAuthorityBeginOwner::new(settlement, lease, manifest);
        let installed = std::cell::RefCell::new(None);
        assert!(matches!(
            exact.begin_back_no_target_authority_settlement(owner, |batch| {
                installed.replace(Some(batch));
            }),
            BackNoTargetAuthorityBeginOutcome::Begun
        ));
        let batch = installed.into_inner().unwrap();
        assert_eq!(batch.batch().manifest_digest(), manifest_digest);
        assert_eq!(batch.batch().obligation_count(), obligation_count);
    }

    #[test]
    fn back_no_target_authority_host_retirement_rejection_returns_whole_owner() {
        let (mut engine, request, lease, settlement) = take_accepted_back_no_target_authority();
        let manifest_digest = 0x4155_5448_5245_5449;
        let (owner, obligation_count) =
            exact_back_no_target_authority_begin_owner(&engine, lease, settlement, manifest_digest);
        let snapshot_before = engine.snapshot();
        let owning_admission =
            std::mem::replace(&mut engine.admission, HostAdmissionIssuer::new().unwrap());
        let callback_calls = std::cell::Cell::new(0usize);

        let owner = match engine.begin_back_no_target_authority_settlement(owner, |_| {
            callback_calls.set(callback_calls.get() + 1);
        }) {
            BackNoTargetAuthorityBeginOutcome::Permanent { owner, evidence } => {
                assert_eq!(
                    evidence,
                    BackNoTargetAuthorityBeginPermanentEvidence::HostRetirementRejected
                );
                owner
            }
            BackNoTargetAuthorityBeginOutcome::Begun
            | BackNoTargetAuthorityBeginOutcome::ReceiverMismatch { .. }
            | BackNoTargetAuthorityBeginOutcome::Retryable { .. } => {
                panic!("the real non-owning issuer must reject exact host retirement")
            }
        };

        assert_eq!(callback_calls.get(), 0);
        assert_eq!(engine.snapshot(), snapshot_before);
        assert!(engine.inner.testing_back_no_target_authority_is_accepted());
        assert!(!engine.inner.testing_back_no_target_authority_is_cleanup());
        assert_eq!(engine.snapshot().availability(), &RouterAvailability::Ready);
        assert_eq!(
            engine.request(request).unwrap().status(),
            &RequestStatus::Queued
        );

        engine.admission = owning_admission;
        let installed = std::cell::RefCell::new(None);
        assert!(matches!(
            engine.begin_back_no_target_authority_settlement(owner, |batch| {
                installed.replace(Some(batch));
            }),
            BackNoTargetAuthorityBeginOutcome::Begun
        ));
        let batch = installed.into_inner().unwrap();
        assert_eq!(batch.batch().manifest_digest(), manifest_digest);
        assert_eq!(batch.batch().obligation_count(), obligation_count);
    }

    #[test]
    fn back_no_target_authority_accepted_blocks_collect_and_new_admission() {
        let (mut engine, grant) = ready_engine();
        engine.record_top_slot_registered(&grant).unwrap();
        let lease = engine.confirm_host(grant);
        let (mut engine, candidate, world) =
            take_refrozen_back_no_target_candidate_from_with_world(engine);
        let request = candidate.evidence().request();
        let stamp = engine.history_admission_stamp(&world).unwrap();
        let queued = engine
            .push(RouteLocation::new("/").with_fragment("blocked-by-accepted"))
            .unwrap();
        let settlement =
            match engine.accept_back_no_target_authority_lost_with_retirement(candidate, || {}) {
                BackNoTargetAuthorityCoreOutcome::Accepted(owner) => owner,
                BackNoTargetAuthorityCoreOutcome::Mismatch { .. } => {
                    panic!("the exact authority candidate must install Accepted")
                }
            };

        engine.collect();

        assert!(engine.action().is_none());
        assert_eq!(
            engine.request(request).unwrap().status(),
            &RequestStatus::Queued
        );
        assert_eq!(
            engine.request(queued).unwrap().status(),
            &RequestStatus::Queued
        );
        assert_eq!(
            engine.push(RouteLocation::new("/blocked")),
            Err(IntegrationError::AuthorityExists)
        );
        assert!(matches!(
            engine.prepare_back_admission(&world, &stamp),
            Err(PrepareBackAdmissionErrorKind::AuthorityRecovering)
        ));
        assert!(engine.inner.testing_back_no_target_authority_is_accepted());
        drop((lease, settlement));
    }

    #[test]
    fn back_no_target_authority_completion_foreign_receiver_returns_whole_owner() {
        let (mut exact, request, batch, manifest_digest, obligation_count) =
            begin_exact_back_no_target_authority();
        let completion = batch.complete(
            manifest_digest,
            obligation_count,
            0x4155_5448_464f_5243,
            obligation_count,
        );
        let exact_before = exact.snapshot();
        let mut foreign = confirmed_engine();
        let foreign_before = foreign.snapshot();
        let callback_calls = std::cell::Cell::new(0usize);

        let completion = match foreign.complete_back_no_target_authority_cleanup(completion, || {
            callback_calls.set(callback_calls.get() + 1);
        }) {
            BackNoTargetAuthorityCleanupCompletionOutcome::Mismatch { owner, evidence } => {
                assert!(matches!(
                    evidence,
                    BackNoTargetAuthorityCleanupMismatchEvidence::Receiver(_)
                ));
                owner
            }
            BackNoTargetAuthorityCleanupCompletionOutcome::Terminal(_) => {
                panic!("a foreign receiver must return the whole completion owner")
            }
        };

        assert_eq!(callback_calls.get(), 0);
        assert_eq!(foreign.snapshot(), foreign_before);
        assert_eq!(exact.snapshot(), exact_before);
        assert!(exact.inner.testing_back_no_target_authority_is_cleanup());

        let exact_calls = std::cell::Cell::new(0usize);
        assert!(matches!(
            exact.complete_back_no_target_authority_cleanup(completion, || {
                exact_calls.set(exact_calls.get() + 1);
            }),
            BackNoTargetAuthorityCleanupCompletionOutcome::Terminal(_)
        ));
        assert_eq!(exact_calls.get(), 1);
        assert!(
            !exact
                .inner
                .testing_back_no_target_authority_is_outstanding()
        );
        assert!(matches!(
            exact.request(request).unwrap().status(),
            RequestStatus::Failed(failure)
                if failure.kind() == crate::request::NavigationFailureKind::AuthorityLost
        ));
    }

    #[test]
    fn back_no_target_authority_begin_orders_callback_before_cleanup_assignment() {
        let (engine, request, batch, manifest_digest, obligation_count) =
            begin_exact_back_no_target_authority();
        assert_eq!(batch.batch().manifest_digest(), manifest_digest);
        assert_eq!(batch.batch().obligation_count(), obligation_count);
        assert_eq!(
            engine.request(request).unwrap().status(),
            &RequestStatus::Running(NavigationPhase::Recovering)
        );
        assert_eq!(engine.snapshot().requested(), None);
    }

    #[test]
    fn back_no_target_authority_completion_is_typed_and_consumes_retired_host_once() {
        let (mut mismatch_engine, _request, batch, manifest_digest, obligation_count) =
            begin_exact_back_no_target_authority();
        let wrong = batch.complete(
            manifest_digest,
            obligation_count,
            0x4155_5448_4241_4444,
            obligation_count.saturating_sub(1),
        );
        let mismatch_calls = std::cell::Cell::new(0usize);
        let (wrong, evidence) = match mismatch_engine
            .complete_back_no_target_authority_cleanup(wrong, || {
                mismatch_calls.set(mismatch_calls.get() + 1)
            }) {
            BackNoTargetAuthorityCleanupCompletionOutcome::Mismatch { owner, evidence } => {
                (owner, evidence)
            }
            BackNoTargetAuthorityCleanupCompletionOutcome::Terminal(_) => {
                panic!("wrong settlement facts must fail closed")
            }
        };
        assert_eq!(mismatch_calls.get(), 0);
        assert!(matches!(
            evidence,
            BackNoTargetAuthorityCleanupMismatchEvidence::Facts(_)
        ));
        assert!(
            mismatch_engine
                .inner
                .testing_back_no_target_authority_is_cleanup()
        );
        drop(wrong);

        let (mut engine, request, batch, manifest_digest, obligation_count) =
            begin_exact_back_no_target_authority();
        let completion = batch.complete(
            manifest_digest,
            obligation_count,
            0x4155_5448_4449_4745,
            obligation_count,
        );
        let callback_calls = std::cell::Cell::new(0usize);
        assert!(engine.inner.testing_back_no_target_authority_is_cleanup());
        match engine.complete_back_no_target_authority_cleanup(completion, || {
            callback_calls.set(callback_calls.get() + 1);
        }) {
            BackNoTargetAuthorityCleanupCompletionOutcome::Terminal(_) => {}
            BackNoTargetAuthorityCleanupCompletionOutcome::Mismatch { .. } => {
                panic!("exact structural completion must terminal once")
            }
        }
        assert_eq!(callback_calls.get(), 1);
        assert!(
            !engine
                .inner
                .testing_back_no_target_authority_is_outstanding()
        );
        let record = engine.request(request).unwrap();
        let RequestStatus::Failed(failure) = record.status() else {
            panic!("authority completion must publish the original terminal")
        };
        assert_eq!(
            failure.kind(),
            crate::request::NavigationFailureKind::AuthorityLost
        );
        assert_eq!(engine.snapshot().running_request(), None);
    }

    fn assert_logical_zero_mismatch(mismatch: PreparationReceiverMismatchView) {
        match mismatch.inner {
            CorePreparationReceiverMismatchProjection::LogicalZero {
                expected: _,
                observed: _,
            } => {}
            _ => panic!("logical-zero rejection must retain its typed receiver/source evidence"),
        }
    }

    #[test]
    fn back_no_target_logical_zero_approval_requires_exact_core_candidate() {
        let (mut engine, candidate) = take_refrozen_back_no_target_candidate();
        let mut foreign = confirmed_engine();
        let exact_before_foreign = engine.inner.testing_snapshot();
        let foreign_before = foreign.inner.testing_snapshot();
        let (candidate, mismatch) = match foreign.approve_back_no_target_logical_zero(candidate) {
            BackNoTargetApprovalOutcome::Mismatch {
                candidate,
                mismatch,
            } => (candidate, mismatch),
            BackNoTargetApprovalOutcome::Approved(_) => {
                panic!("a foreign engine must not approve another engine's candidate")
            }
        };
        assert_logical_zero_mismatch(mismatch);
        assert_eq!(engine.inner.testing_snapshot(), exact_before_foreign);
        assert_eq!(foreign.inner.testing_snapshot(), foreign_before);

        let exact_before = engine.inner.testing_snapshot();
        match engine.approve_back_no_target_logical_zero(candidate) {
            BackNoTargetApprovalOutcome::Approved(_) => {
                assert_eq!(engine.inner.testing_snapshot(), exact_before);
            }
            BackNoTargetApprovalOutcome::Mismatch { .. } => {
                assert_eq!(engine.inner.testing_snapshot(), exact_before);
                panic!("exact core logical-zero candidate must receive opaque approval");
            }
        }

        let (mut current_owner_drift, candidate) = take_refrozen_back_no_target_candidate();
        current_owner_drift
            .push(RouteLocation::new("/").with_fragment("logical-zero-owner-drift"))
            .expect("the real drift request reserves one normal current owner");
        current_owner_drift.collect();
        let before_current_owner_mismatch = current_owner_drift.inner.testing_snapshot();
        let mismatch = match current_owner_drift.approve_back_no_target_logical_zero(candidate) {
            BackNoTargetApprovalOutcome::Mismatch {
                candidate,
                mismatch,
            } => {
                drop(candidate);
                mismatch
            }
            BackNoTargetApprovalOutcome::Approved(_) => {
                panic!("a current execution owner must reject logical-zero approval")
            }
        };
        assert_logical_zero_mismatch(mismatch);
        assert_eq!(
            current_owner_drift.inner.testing_snapshot(),
            before_current_owner_mismatch
        );

        let (mut source_drift, candidate) = take_refrozen_back_no_target_candidate();
        settle_changed_history_push(
            &mut source_drift,
            RouteLocation::new("/").with_fragment("logical-zero-predecessor-drift"),
        );
        let before_source_mismatch = source_drift.inner.testing_snapshot();
        let mismatch = match source_drift.approve_back_no_target_logical_zero(candidate) {
            BackNoTargetApprovalOutcome::Mismatch {
                candidate,
                mismatch,
            } => {
                drop(candidate);
                mismatch
            }
            BackNoTargetApprovalOutcome::Approved(_) => {
                panic!("a changed source with a live predecessor must reject approval")
            }
        };
        assert_logical_zero_mismatch(mismatch);
        assert_eq!(
            source_drift.inner.testing_snapshot(),
            before_source_mismatch
        );
    }

    #[test]
    fn back_no_target_ready_batch_projection_preserves_exact_resolution_evidence() {
        let (mut exact, candidate) = take_refrozen_back_no_target_candidate();
        let expected_evidence = candidate.evidence();
        let approved = match exact.approve_back_no_target_logical_zero(candidate) {
            BackNoTargetApprovalOutcome::Approved(approved) => approved,
            BackNoTargetApprovalOutcome::Mismatch { .. } => {
                panic!("the real exact candidate must receive logical-zero approval")
            }
        };
        let exact_before = exact.inner.testing_snapshot();
        let exact_callback_calls = std::cell::Cell::new(0usize);
        let manifest_digest = 0x4e4f_5441_5247_4554;
        let expected_completion_digest = 0x434f_4d50_4c45_5445;
        let prepared = match exact.accept_back_no_target_ready_with_canonical_zero_bind(
            approved,
            manifest_digest,
            0,
            expected_completion_digest,
            || exact_callback_calls.set(exact_callback_calls.get() + 1),
        ) {
            BackNoTargetReadyCoreOutcome::Prepared(prepared) => prepared,
            BackNoTargetReadyCoreOutcome::Mismatch { .. } => {
                panic!("the unchanged exact approved candidate must prepare publication")
            }
        };
        assert_eq!(
            exact_callback_calls.get(),
            1,
            "the exact Ready receiver must invoke the canonical-zero callback once"
        );
        assert_eq!(
            exact.inner.testing_snapshot(),
            exact_before,
            "sealed Ready preparation must leave Core public and semantic state zero-write"
        );

        let (mut owner_engine, candidate) = take_refrozen_back_no_target_candidate();
        let fresh_approved = match owner_engine.approve_back_no_target_logical_zero(candidate) {
            BackNoTargetApprovalOutcome::Approved(approved) => approved,
            BackNoTargetApprovalOutcome::Mismatch { .. } => {
                panic!("the fresh real candidate must receive logical-zero approval")
            }
        };
        let mut foreign = confirmed_engine();
        let owner_before_foreign = owner_engine.inner.testing_snapshot();
        let foreign_before = foreign.inner.testing_snapshot();
        let foreign_callback_calls = std::cell::Cell::new(0usize);
        let (returned_approved, mismatch) = match foreign
            .accept_back_no_target_ready_with_canonical_zero_bind(
                fresh_approved,
                manifest_digest,
                0,
                expected_completion_digest,
                || foreign_callback_calls.set(foreign_callback_calls.get() + 1),
            ) {
            BackNoTargetReadyCoreOutcome::Mismatch { approved, mismatch } => (approved, mismatch),
            BackNoTargetReadyCoreOutcome::Prepared(_) => {
                panic!("a foreign Ready receiver must not prepare another engine's publication")
            }
        };
        assert_eq!(
            foreign_callback_calls.get(),
            0,
            "foreign Ready receiver mismatch must invoke the callback zero times"
        );
        match mismatch.inner {
            CorePreparationReceiverMismatchProjection::LogicalZero {
                expected: _,
                observed: _,
            } => {}
            _ => panic!("foreign Ready mismatch must retain logical-zero expected/observed"),
        }
        assert_eq!(owner_engine.inner.testing_snapshot(), owner_before_foreign);
        assert_eq!(foreign.inner.testing_snapshot(), foreign_before);

        let returned_callback_calls = std::cell::Cell::new(0usize);
        match owner_engine.accept_back_no_target_ready_with_canonical_zero_bind(
            returned_approved,
            manifest_digest,
            0,
            expected_completion_digest,
            || returned_callback_calls.set(returned_callback_calls.get() + 1),
        ) {
            BackNoTargetReadyCoreOutcome::Prepared(_) => {}
            BackNoTargetReadyCoreOutcome::Mismatch { .. } => {
                panic!("the complete returned approved owner must remain exactly consumable")
            }
        }
        assert_eq!(
            returned_callback_calls.get(),
            1,
            "the restored exact receiver must consume the returned approved owner once"
        );

        assert!(
            prepared
                .batch()
                .matches_resolution_evidence(&expected_evidence),
            "RED: prepared no-target cleanup batch projection must freeze the candidate's complete BackResolutionEvidence"
        );
        assert_eq!(prepared.batch().manifest_digest(), manifest_digest);
        assert_eq!(prepared.batch().obligation_count(), 0);
        assert_eq!(
            prepared.batch().expected_completion_digest(),
            expected_completion_digest
        );

        let installed_before_publication = std::cell::Cell::new(false);
        exact.publish_prepared_back_no_target(prepared, |installation| {
            assert!(
                installation
                    .batch()
                    .matches_resolution_evidence(&expected_evidence)
            );
            assert_eq!(installation.batch().manifest_digest(), manifest_digest);
            assert_eq!(installation.batch().obligation_count(), 0);
            assert_eq!(
                installation.batch().expected_completion_digest(),
                expected_completion_digest
            );
            installed_before_publication.set(true);
        });
        assert!(installed_before_publication.get());
        let published = exact
            .request(expected_evidence.request())
            .expect("the published no-target request remains retained");
        assert_eq!(
            published.status(),
            &RequestStatus::Running(NavigationPhase::CleanupPending)
        );
        let published_kinds = published
            .trace()
            .iter()
            .rev()
            .take(2)
            .map(NavigationTraceEvent::kind)
            .collect::<Vec<_>>();
        assert_eq!(
            published_kinds,
            vec![
                NavigationTraceEventKind::RequestPhaseChanged,
                NavigationTraceEventKind::HistoryIntentRejected,
            ]
        );

        let (mut nonzero_engine, candidate) = take_refrozen_back_no_target_candidate();
        let nonzero_request = candidate.evidence().request();
        let approved = match nonzero_engine.approve_back_no_target_logical_zero(candidate) {
            BackNoTargetApprovalOutcome::Approved(approved) => approved,
            BackNoTargetApprovalOutcome::Mismatch { .. } => {
                panic!("the exact nonzero-count fixture must approve before Ready")
            }
        };
        let nonzero_before = nonzero_engine.inner.testing_snapshot();
        let nonzero_callback_calls = std::cell::Cell::new(0usize);
        match nonzero_engine.accept_back_no_target_ready_with_canonical_zero_bind(
            approved,
            manifest_digest,
            1,
            expected_completion_digest,
            || nonzero_callback_calls.set(nonzero_callback_calls.get() + 1),
        ) {
            BackNoTargetReadyCoreOutcome::Mismatch { approved, mismatch } => {
                match mismatch.inner {
                    CorePreparationReceiverMismatchProjection::BackNoTargetReady(
                        crate::engine::CoreBackNoTargetReadyMismatchProjection::Manifest {
                            request,
                            expected,
                            observed,
                        },
                    ) => {
                        assert_eq!(request, nonzero_request);
                        assert_eq!(expected.manifest_digest, manifest_digest);
                        assert_eq!(expected.obligation_count, 0);
                        assert_eq!(observed.manifest_digest, manifest_digest);
                        assert_eq!(observed.obligation_count, 1);
                    }
                    _ => panic!(
                        "nonzero no-target commitment must retain typed expected/observed evidence"
                    ),
                }
                drop(approved);
            }
            BackNoTargetReadyCoreOutcome::Prepared(_) => {
                panic!("a nonzero no-target manifest must remain opaque and reject Ready")
            }
        }
        assert_eq!(nonzero_callback_calls.get(), 0);
        assert_eq!(nonzero_engine.inner.testing_snapshot(), nonzero_before);

        let (mut host_drift_engine, candidate) = take_refrozen_back_no_target_candidate();
        let approved = match host_drift_engine.approve_back_no_target_logical_zero(candidate) {
            BackNoTargetApprovalOutcome::Approved(approved) => approved,
            BackNoTargetApprovalOutcome::Mismatch { .. } => {
                panic!("the host-drift fixture must approve before its host changes")
            }
        };
        assert!(host_drift_engine.inner.testing_remove_back_no_target_host());
        let host_drift_before = host_drift_engine.inner.testing_snapshot();
        let host_drift_callback_calls = std::cell::Cell::new(0usize);
        match host_drift_engine.accept_back_no_target_ready_with_canonical_zero_bind(
            approved,
            manifest_digest,
            0,
            expected_completion_digest,
            || host_drift_callback_calls.set(host_drift_callback_calls.get() + 1),
        ) {
            BackNoTargetReadyCoreOutcome::Mismatch { approved, .. } => drop(approved),
            BackNoTargetReadyCoreOutcome::Prepared(_) => {
                panic!("a changed HostVersion receiver must reject Ready")
            }
        }
        assert_eq!(host_drift_callback_calls.get(), 0);
        assert_eq!(
            host_drift_engine.inner.testing_snapshot(),
            host_drift_before
        );
    }

    fn take_published_back_no_target_cleanup() -> (
        RouterEngine,
        NavigationRequestId,
        BackNoTargetCleanupInstallation,
        u64,
        u64,
    ) {
        let (mut engine, candidate) = take_refrozen_back_no_target_candidate();
        let request = candidate.evidence().request();
        let approved = match engine.approve_back_no_target_logical_zero(candidate) {
            BackNoTargetApprovalOutcome::Approved(approved) => approved,
            BackNoTargetApprovalOutcome::Mismatch { .. } => {
                panic!("the exact completion fixture must receive logical-zero approval")
            }
        };
        let manifest_digest = 0x4e4f_5441_5247_4554;
        let completion_digest = 0x434f_4d50_4c45_5445;
        let prepared = match engine.accept_back_no_target_ready_with_canonical_zero_bind(
            approved,
            manifest_digest,
            0,
            completion_digest,
            || {},
        ) {
            BackNoTargetReadyCoreOutcome::Prepared(prepared) => prepared,
            BackNoTargetReadyCoreOutcome::Mismatch { .. } => {
                panic!("the exact completion fixture must prepare publication")
            }
        };
        let installation = std::cell::RefCell::new(None);
        engine.publish_prepared_back_no_target(prepared, |owner| {
            installation.replace(Some(owner));
        });
        (
            engine,
            request,
            installation
                .into_inner()
                .expect("publication must hand the whole installation to its closure"),
            manifest_digest,
            completion_digest,
        )
    }

    #[test]
    fn back_no_target_exact_zero_completion_is_typed_and_terminal_once() {
        for facts in [
            (0x4241_444d_414e_4946, 0, 0x434f_4d50_4c45_5445, 0),
            (0x4e4f_5441_5247_4554, 1, 0x434f_4d50_4c45_5445, 0),
            (0x4e4f_5441_5247_4554, 0, 0x434f_4d50_4c45_5444, 0),
            (0x4e4f_5441_5247_4554, 0, 0x434f_4d50_4c45_5445, 1),
        ] {
            let (mut engine, _request, installation, _manifest_digest, _completion_digest) =
                take_published_back_no_target_cleanup();
            let owner = installation.complete(facts.0, facts.1, facts.2, facts.3);
            let before = engine.inner.testing_snapshot();
            let callback_calls = std::cell::Cell::new(0usize);
            let (owner, mismatch) = match engine.complete_back_no_target_cleanup(owner, || {
                callback_calls.set(callback_calls.get() + 1)
            }) {
                BackNoTargetCleanupCompletionOutcome::Mismatch { owner, mismatch } => {
                    (owner, mismatch)
                }
                BackNoTargetCleanupCompletionOutcome::Terminal(_) => {
                    panic!("wrong no-target completion facts must fail closed")
                }
            };
            assert_eq!(callback_calls.get(), 0);
            assert_eq!(engine.inner.testing_snapshot(), before);
            assert!(matches!(
                mismatch.inner,
                CorePreparationReceiverMismatchProjection::BackNoTargetCompletion { .. }
            ));

            let replay_callback_calls = std::cell::Cell::new(0usize);
            match engine.complete_back_no_target_cleanup(owner, || {
                replay_callback_calls.set(replay_callback_calls.get() + 1)
            }) {
                BackNoTargetCleanupCompletionOutcome::Mismatch { owner, .. } => drop(owner),
                BackNoTargetCleanupCompletionOutcome::Terminal(_) => {
                    panic!("an immutable wrong-facts owner must remain fail closed")
                }
            }
            assert_eq!(replay_callback_calls.get(), 0);
            assert_eq!(engine.inner.testing_snapshot(), before);
        }

        let (mut exact, request, installation, manifest_digest, completion_digest) =
            take_published_back_no_target_cleanup();
        let owner = installation.complete(manifest_digest, 0, completion_digest, 0);
        let exact_before_foreign = exact.inner.testing_snapshot();
        let mut foreign = confirmed_engine();
        let foreign_before = foreign.inner.testing_snapshot();
        let foreign_callback_calls = std::cell::Cell::new(0usize);
        let (owner, mismatch) = match foreign.complete_back_no_target_cleanup(owner, || {
            foreign_callback_calls.set(foreign_callback_calls.get() + 1)
        }) {
            BackNoTargetCleanupCompletionOutcome::Mismatch { owner, mismatch } => (owner, mismatch),
            BackNoTargetCleanupCompletionOutcome::Terminal(_) => {
                panic!("a foreign engine must return the whole completion owner")
            }
        };
        assert_eq!(foreign_callback_calls.get(), 0);
        assert_eq!(exact.inner.testing_snapshot(), exact_before_foreign);
        assert_eq!(foreign.inner.testing_snapshot(), foreign_before);
        assert!(matches!(
            mismatch.inner,
            CorePreparationReceiverMismatchProjection::BackNoTargetCompletion { .. }
        ));

        let before_public = exact.snapshot();
        let exact_callback_calls = std::cell::Cell::new(0usize);
        match exact.complete_back_no_target_cleanup(owner, || {
            exact_callback_calls.set(exact_callback_calls.get() + 1)
        }) {
            BackNoTargetCleanupCompletionOutcome::Terminal(terminal) => drop(terminal),
            BackNoTargetCleanupCompletionOutcome::Mismatch { .. } => {
                panic!("the returned canonical owner must remain exact for its original engine")
            }
        }
        assert_eq!(exact_callback_calls.get(), 1);
        let after_public = exact.snapshot();
        assert_eq!(after_public.active(), before_public.active());
        assert_eq!(after_public.history(), before_public.history());
        assert_eq!(after_public.running_request(), None);
        let terminal = exact
            .request(request)
            .expect("the exact no-target terminal remains retained");
        let RequestStatus::Failed(failure) = terminal.status() else {
            panic!("exact zero completion must fail the no-target request")
        };
        assert_eq!(
            failure.kind(),
            crate::request::NavigationFailureKind::BackTargetUnavailable
        );
        assert_eq!(
            terminal
                .trace()
                .iter()
                .rev()
                .take(2)
                .map(NavigationTraceEvent::kind)
                .collect::<Vec<_>>(),
            vec![
                NavigationTraceEventKind::RequestTerminal,
                NavigationTraceEventKind::CleanupSettled,
            ]
        );
    }

    fn take_back_target_driver_frame_task() -> (
        RouterEngine,
        HistoryExecutionBindingView,
        LifecycleContinuationCapacity,
        RootFrameTask,
    ) {
        let mut exact = confirmed_engine();
        settle_push(
            &mut exact,
            RouteLocation::new("/").with_fragment("driver-target"),
        );
        settle_changed_history_push(
            &mut exact,
            RouteLocation::new("/").with_fragment("driver-source"),
        );
        let owner = take_verified_back_target(&mut exact);
        let resume = match exact.resume_back_target_with_retirement(owner, || {}) {
            BackTargetCoreResumeOutcome::Resumed(BackTargetStart::Ready(resume)) => resume,
            BackTargetCoreResumeOutcome::Resumed(BackTargetStart::FailureCleanup(_)) => {
                panic!("an unchanged depth-one Back target must not enter failure cleanup")
            }
            BackTargetCoreResumeOutcome::Permanent(_) => {
                panic!("an unchanged depth-one Back target must resume")
            }
            BackTargetCoreResumeOutcome::Mismatch { .. } => {
                panic!("the exact depth-one Back target receiver must match")
            }
        };
        let (action, history, continuation) = resume.into_parts();
        let BackTargetResumeAction::FrameTask(task) = action else {
            panic!("the changed depth-one Back target must issue one real frame task")
        };
        (exact, history, continuation, task)
    }

    fn take_back_target_nested_parent_frame_task() -> (
        RouterEngine,
        HistoryExecutionBindingView,
        LifecycleContinuationCapacity,
        RootFrameTask,
    ) {
        let mut exact = confirmed_back_target_nested_engine();
        settle_nested_history_command(
            &mut exact,
            HistoryFixtureCommand::Push,
            RouteLocation::new("/items/1").with_fragment("applied-nested-target"),
        );
        settle_nested_history_command(
            &mut exact,
            HistoryFixtureCommand::Push,
            RouteLocation::new("/").with_fragment("applied-nested-source"),
        );
        let owner = take_verified_back_target(&mut exact);
        let resume = match exact.resume_back_target_with_retirement(owner, || {}) {
            BackTargetCoreResumeOutcome::Resumed(BackTargetStart::Ready(resume)) => resume,
            BackTargetCoreResumeOutcome::Resumed(BackTargetStart::FailureCleanup(_)) => {
                panic!("the unchanged nested Back target must not enter failure cleanup")
            }
            BackTargetCoreResumeOutcome::Permanent(_) => {
                panic!("the unchanged nested Back target must resume")
            }
            BackTargetCoreResumeOutcome::Mismatch { .. } => {
                panic!("the exact nested Back target receiver must match")
            }
        };
        let (action, history, continuation) = resume.into_parts();
        let BackTargetResumeAction::FrameTask(task) = action else {
            panic!("the changed nested parent must issue one real frame task")
        };
        (exact, history, continuation, task)
    }

    #[derive(Clone, Copy)]
    enum ExpectedBackTargetResumeAction {
        FrameTask,
        MountRequest,
        Commit,
    }

    fn assert_back_target_wrong_receiver_calls_zero(
        owner: VerifiedBackTargetExecutionOwner,
    ) -> VerifiedBackTargetExecutionOwner {
        let callback_calls = std::cell::Cell::new(0);
        let mut foreign = confirmed_engine();
        let owner = match foreign.resume_back_target_with_retirement(owner, || {
            callback_calls.set(callback_calls.get() + 1);
        }) {
            BackTargetCoreResumeOutcome::Mismatch { owner, .. } => owner,
            BackTargetCoreResumeOutcome::Resumed(_) => {
                panic!("a foreign receiver must not resume the exact target owner")
            }
            BackTargetCoreResumeOutcome::Permanent(_) => {
                panic!("a foreign receiver mismatch must preserve the verified target owner")
            }
        };
        assert_eq!(
            callback_calls.get(),
            0,
            "receiver mismatch must not invoke the retirement callback"
        );
        owner
    }

    fn assert_back_target_receiver_fixture(
        mut exact: RouterEngine,
        expected: ExpectedBackTargetResumeAction,
    ) {
        let owner = take_verified_back_target(&mut exact);
        let owner = assert_back_target_wrong_receiver_calls_zero(owner);
        let callback_calls = std::cell::Cell::new(0);

        let outcome = exact.resume_back_target_with_retirement(owner, || {
            callback_calls.set(callback_calls.get() + 1);
        });
        assert_eq!(
            callback_calls.get(),
            1,
            "the exact receiver must invoke the retirement callback exactly once"
        );
        match outcome {
            BackTargetCoreResumeOutcome::Resumed(BackTargetStart::Ready(resume)) => {
                let (action, _execution, _continuation) = resume.into_parts();
                assert!(
                    matches!(
                        (expected, action),
                        (
                            ExpectedBackTargetResumeAction::FrameTask,
                            BackTargetResumeAction::FrameTask(_)
                        ) | (
                            ExpectedBackTargetResumeAction::MountRequest,
                            BackTargetResumeAction::MountRequest(_)
                        ) | (
                            ExpectedBackTargetResumeAction::Commit,
                            BackTargetResumeAction::Commit(_)
                        )
                    ),
                    "the exact receiver must return the real reconciliation action"
                );
            }
            BackTargetCoreResumeOutcome::Resumed(BackTargetStart::FailureCleanup(_)) => {
                panic!("a non-exhausted fixture must not enter failure cleanup")
            }
            BackTargetCoreResumeOutcome::Permanent(_) => {
                panic!("the unchanged live history source must resume, not become permanent")
            }
            BackTargetCoreResumeOutcome::Mismatch { .. } => {
                panic!("the exact receiver must resume its verified target owner")
            }
        }
    }

    fn assert_back_target_permanent_zero_write(
        mut engine: RouterEngine,
        mutate: impl FnOnce(&mut RouterEngine, &mut VerifiedBackTargetExecutionOwner),
    ) {
        let mut owner = take_verified_back_target(&mut engine);
        mutate(&mut engine, &mut owner);
        let before = engine.inner.testing_snapshot();
        let callback_calls = std::cell::Cell::new(0);

        let outcome = engine.resume_back_target_with_retirement(owner, || {
            callback_calls.set(callback_calls.get() + 1);
        });

        assert_eq!(callback_calls.get(), 1);
        assert!(matches!(outcome, BackTargetCoreResumeOutcome::Permanent(_)));
        assert_eq!(engine.inner.testing_snapshot(), before);
        assert!(engine.action().is_none());
    }

    fn direct_back_target_fixture() -> RouterEngine {
        let mut engine = confirmed_engine();
        settle_push(
            &mut engine,
            RouteLocation::new("/").with_fragment("prepared-target"),
        );
        settle_changed_history_push(
            &mut engine,
            RouteLocation::new("/").with_fragment("prepared-source"),
        );
        engine
    }

    fn take_source_refreeze_back_target_permanent_owner(
        engine: &mut RouterEngine,
        fragment: &'static str,
    ) -> BackTargetPermanentOwner {
        use crate::history_authority::BackTargetPermanentCauseSnapshot;

        let owner = take_verified_back_target(engine);
        settle_changed_history_push(engine, RouteLocation::new("/").with_fragment(fragment));
        let retirement_calls = std::cell::Cell::new(0usize);
        let owner = match engine.resume_back_target_with_retirement(owner, || {
            retirement_calls.set(retirement_calls.get() + 1);
        }) {
            BackTargetCoreResumeOutcome::Permanent(owner) => owner,
            BackTargetCoreResumeOutcome::Resumed(_) => {
                panic!("the real history push must form a SourceRefreeze permanent owner")
            }
            BackTargetCoreResumeOutcome::Mismatch { .. } => {
                panic!("the exact receiver must classify the real source refreeze")
            }
        };
        assert_eq!(retirement_calls.get(), 1);
        assert_eq!(
            owner.testing_evidence_snapshot().cause(),
            BackTargetPermanentCauseSnapshot::SourceRefreeze
        );
        owner
    }

    fn assert_back_target_permanent_settlement_mismatch_zero_write(
        mut engine: RouterEngine,
        owner: BackTargetPermanentOwner,
        drift: &str,
    ) {
        let expected_evidence = owner.evidence();
        let expected_target = owner
            .target_location()
            .expect("the real permanent owner retains its target")
            .clone();
        let expected_owner_snapshot = owner.testing_evidence_snapshot();
        let before = engine.inner.testing_snapshot();

        let returned = match engine.settle_back_target_permanent_exact(owner) {
            BackTargetPermanentSettlementOutcome::Mismatch { owner, evidence } => {
                assert_eq!(evidence, expected_evidence);
                owner
            }
            BackTargetPermanentSettlementOutcome::Settled(_) => {
                panic!("{drift} must reject permanent settlement")
            }
        };

        assert_eq!(returned.evidence(), expected_evidence);
        assert_eq!(returned.target_location(), Some(&expected_target));
        assert_eq!(
            returned.testing_evidence_snapshot(),
            expected_owner_snapshot
        );
        assert_eq!(
            engine.inner.testing_snapshot(),
            before,
            "{drift} mismatch must be a semantic zero-write"
        );
    }

    fn back_target_busy_running_donor() -> RouterEngine {
        let mut donor = confirmed_engine();
        donor
            .push(RouteLocation::new("/").with_fragment("busy-donor"))
            .expect("the donor accepts one real request");
        donor.collect();
        donor
    }

    fn back_target_busy_prepared_publication_donor() -> RouterEngine {
        let mut donor = back_target_busy_running_donor();
        let EngineAction::FrameTask(task) = donor.action().expect("the donor has one real action")
        else {
            panic!("the depth-one donor must issue a real frame task")
        };
        let ticket = donor
            .report_applied(task)
            .expect("the real donor frame report reaches commit");
        donor
            .prepare_commit(&ticket, CleanupManifest::empty())
            .expect("the donor installs real prepared publication authority");
        donor
    }

    fn back_target_busy_history_preparation_error_donor() -> RouterEngine {
        let mut donor = confirmed_nested_engine();
        settle_nested_history_command(
            &mut donor,
            HistoryFixtureCommand::Push,
            RouteLocation::new("/items/1"),
        );
        donor
            .replace(RouteLocation::new("/items/2"))
            .expect("the donor accepts a real nested replacement");
        donor.collect();
        let ticket = loop {
            match donor.action().expect("the donor retains one real action") {
                EngineAction::FrameTask(task) => {
                    let outlets = donor
                        .expected_child_outlet(&task)
                        .expect("the donor frame task has a real outlet projection")
                        .into_iter()
                        .collect();
                    donor
                        .report_applied_with_outlets(task, outlets)
                        .expect("the donor accepts its real frame report");
                }
                EngineAction::MountRequest(mount) => {
                    let receipt = donor.resolve_mount(mount).unwrap().attest_receipt(0x71);
                    let completion = donor.accept_mount_receipt(receipt).unwrap().complete(0x73);
                    donor.complete_mount_registration(completion).unwrap();
                }
                EngineAction::Commit(ticket) => break ticket,
                EngineAction::FailureCleanup(_) | EngineAction::BackResolution(_) => {
                    panic!("the unchanged nested donor must reach a real commit")
                }
            }
        };
        let logical_roots = donor
            .reconciliation_projection_for_commit(&ticket)
            .unwrap()
            .leaving()
            .map(|leaving| leaving.version().clone())
            .collect::<Vec<_>>();
        let obligation_count = logical_roots.len();
        assert!(donor.inner.testing_drift_current_history_binding());
        let manifest = if obligation_count == 0 {
            CleanupManifest::empty()
        } else {
            CleanupManifest::new(logical_roots, 0x4255_5359_4552_524f, obligation_count)
        };
        assert!(matches!(
            donor.prepare_commit(&ticket, manifest),
            Err(IntegrationError::Stale)
        ));
        donor
    }

    fn back_target_busy_donor(slot: BackTargetBusyReceiverSlot) -> RouterEngine {
        match slot {
            BackTargetBusyReceiverSlot::Action
            | BackTargetBusyReceiverSlot::Current
            | BackTargetBusyReceiverSlot::CurrentHistoryCapability => {
                back_target_busy_running_donor()
            }
            BackTargetBusyReceiverSlot::PendingNoTargetBack => {
                let mut donor = confirmed_engine();
                settle_push(&mut donor, RouteLocation::new("/"));
                donor
            }
            BackTargetBusyReceiverSlot::PendingBackNoTargetAuthority => {
                take_accepted_back_no_target_authority().0
            }
            BackTargetBusyReceiverSlot::PendingHistoryPreparationError => {
                back_target_busy_history_preparation_error_donor()
            }
            BackTargetBusyReceiverSlot::PreparedPublicationAuthority => {
                back_target_busy_prepared_publication_donor()
            }
            BackTargetBusyReceiverSlot::CurrentRequest => confirmed_engine(),
        }
    }

    fn assert_back_target_busy_receiver_calls_zero(slot: BackTargetBusyReceiverSlot) {
        let mut exact = direct_back_target_fixture();
        let owner = take_verified_back_target(&mut exact);
        let expected_evidence = owner.evidence();
        let expected_target = owner.target_location().clone();
        let mut donor = back_target_busy_donor(slot);
        exact
            .inner
            .testing_move_back_target_busy_receiver_slot_from(&mut donor.inner, slot);
        let before = exact.inner.testing_snapshot();
        let callback_calls = std::cell::Cell::new(0usize);

        let outcome = exact.resume_back_target_with_retirement(owner, || {
            callback_calls.set(callback_calls.get() + 1);
        });

        assert_eq!(
            callback_calls.get(),
            0,
            "busy {slot:?} must reject before retirement"
        );
        let returned = match outcome {
            BackTargetCoreResumeOutcome::Mismatch { owner, evidence } => {
                assert_eq!(evidence, expected_evidence);
                owner
            }
            BackTargetCoreResumeOutcome::Resumed(_) => {
                panic!("busy {slot:?} must return the whole verified owner as Mismatch")
            }
            BackTargetCoreResumeOutcome::Permanent(_) => {
                panic!("busy {slot:?} must not mint a Permanent owner")
            }
        };
        assert_eq!(returned.evidence(), expected_evidence);
        assert_eq!(returned.target_location(), &expected_target);
        assert_eq!(
            exact.classify_back_resolution_evidence(&expected_evidence),
            BackResolutionEvidenceClassification::ConsumedReplay
        );
        assert_eq!(
            exact.inner.testing_snapshot(),
            before,
            "busy {slot:?} mismatch must be a semantic zero-write"
        );
    }

    #[test]
    fn back_target_prepares_all_fallible_work_before_retirement_callback() {
        assert_back_target_permanent_zero_write(direct_back_target_fixture(), |engine, _owner| {
            assert!(engine.inner.testing_begin_back_target_host_drift());
        });

        assert_back_target_permanent_zero_write(direct_back_target_fixture(), |engine, _owner| {
            engine
                .inner
                .testing_set_active_generation(RouteGeneration::testing_max());
        });

        let mut generation = direct_back_target_fixture();
        generation
            .inner
            .testing_set_active_generation(RouteGeneration::testing_max());
        assert_back_target_permanent_zero_write(generation, |_engine, _owner| {});
    }

    #[test]
    fn back_target_reservation_truncation_mismatches_before_retirement() {
        let mut exact = nested_mount_back_fixture();
        let mut owner = take_verified_back_target(&mut exact);
        let expected_evidence = owner.evidence();
        let expected_target = owner.target_location().clone();
        owner.testing_truncate_reserved_frames(1);
        let before = exact.inner.testing_snapshot();
        let callback_calls = std::cell::Cell::new(0usize);

        let outcome = exact.resume_back_target_with_retirement(owner, || {
            callback_calls.set(callback_calls.get() + 1);
        });

        assert_eq!(
            callback_calls.get(),
            0,
            "reservation drift must reject before retirement"
        );
        let returned = match outcome {
            BackTargetCoreResumeOutcome::Mismatch { owner, evidence } => {
                assert_eq!(evidence, expected_evidence);
                owner
            }
            BackTargetCoreResumeOutcome::Resumed(_) => {
                panic!("reservation drift must not resume the changed owner")
            }
            BackTargetCoreResumeOutcome::Permanent(_) => {
                panic!("reservation drift must not mint a Permanent owner")
            }
        };
        assert_eq!(returned.evidence(), expected_evidence);
        assert_eq!(returned.target_location(), &expected_target);
        assert_eq!(
            exact.classify_back_resolution_evidence(&expected_evidence),
            BackResolutionEvidenceClassification::ConsumedReplay
        );
        assert_eq!(
            exact.inner.testing_snapshot(),
            before,
            "reservation mismatch must be a semantic zero-write"
        );
    }

    #[test]
    fn back_target_permanent_settlement_rejects_independent_owner_source_and_record_drift() {
        let mut reservation_engine = direct_back_target_fixture();
        let mut reservation_owner = take_source_refreeze_back_target_permanent_owner(
            &mut reservation_engine,
            "permanent-reservation-drift",
        );
        reservation_owner.testing_truncate_reserved_frames(0);
        assert_back_target_permanent_settlement_mismatch_zero_write(
            reservation_engine,
            reservation_owner,
            "permanent owner reservation drift",
        );

        let mut source_engine = direct_back_target_fixture();
        let source_owner = take_source_refreeze_back_target_permanent_owner(
            &mut source_engine,
            "permanent-current-source-drift",
        );
        source_engine
            .inner
            .testing_move_back_target_current_source_cursor_back();
        assert_back_target_permanent_settlement_mismatch_zero_write(
            source_engine,
            source_owner,
            "current source drift",
        );

        let mut record_engine = direct_back_target_fixture();
        let record_owner = take_source_refreeze_back_target_permanent_owner(
            &mut record_engine,
            "permanent-retained-record-drift",
        );
        let record_request = record_owner.evidence().request();
        assert!(
            record_engine
                .inner
                .testing_transition_back_target_record_to_matching(record_request),
            "the retained Back record must transition through its real status input"
        );
        assert_back_target_permanent_settlement_mismatch_zero_write(
            record_engine,
            record_owner,
            "retained record drift",
        );
    }

    #[test]
    fn back_target_pre_splice_permanent_settlement_terminals_once() {
        use crate::history_authority::{
            BackTargetAuthorityLossCause, BackTargetPermanentCauseSnapshot,
        };

        let mut capability = direct_back_target_fixture();
        let owner = take_verified_back_target(&mut capability);
        let capability_request = owner.evidence().request();
        capability
            .inner
            .testing_request_back_target_source_drift_before_freeze_once();
        let retirement_calls = std::cell::Cell::new(0usize);
        let outcome = capability.resume_back_target_with_retirement(owner, || {
            retirement_calls.set(retirement_calls.get() + 1);
        });
        assert_eq!(retirement_calls.get(), 1);
        let owner = match outcome {
            BackTargetCoreResumeOutcome::Permanent(owner) => owner,
            BackTargetCoreResumeOutcome::Resumed(_) => {
                panic!(
                    "the armed post-refreeze seam must reach the production freeze_back drift predicate"
                )
            }
            BackTargetCoreResumeOutcome::Mismatch { .. } => {
                panic!("the exact receiver must not reject the defensive source recheck seam")
            }
        };
        let expected_evidence = owner.evidence();
        let expected_target = owner
            .target_location()
            .expect("the real SourceCapability owner retains its target")
            .clone();
        let capability_owner_snapshot = owner.testing_evidence_snapshot();
        assert_eq!(
            capability_owner_snapshot.cause(),
            BackTargetPermanentCauseSnapshot::SourceCapability
        );
        let (expected_source, observed_source) = capability_owner_snapshot.sources();
        assert_ne!(
            expected_source, observed_source,
            "the production HistorySourceDrift must retain distinct source facts"
        );
        let (expected_baseline, observed_baseline) = capability_owner_snapshot.baselines();
        assert_eq!(
            expected_baseline, observed_baseline,
            "the source-only seam must not alter unrelated preparation baseline facts"
        );

        let mut capability_foreign = confirmed_engine();
        let foreign_before = capability_foreign.inner.testing_snapshot();
        let exact_before_foreign = capability.inner.testing_snapshot();
        let owner = match capability_foreign.settle_back_target_permanent_exact(owner) {
            BackTargetPermanentSettlementOutcome::Mismatch { owner, evidence } => {
                assert_eq!(evidence, expected_evidence);
                assert_eq!(
                    capability_foreign.classify_back_resolution_evidence(&evidence),
                    BackResolutionEvidenceClassification::Foreign
                );
                owner
            }
            BackTargetPermanentSettlementOutcome::Settled(_) => {
                panic!("a foreign receiver must return the whole SourceCapability owner")
            }
        };
        assert_eq!(capability_foreign.inner.testing_snapshot(), foreign_before);
        assert_eq!(capability.inner.testing_snapshot(), exact_before_foreign);
        assert_eq!(owner.evidence(), expected_evidence);
        assert_eq!(owner.target_location(), Some(&expected_target));
        assert_eq!(owner.testing_evidence_snapshot(), capability_owner_snapshot);

        let before_settlement = capability.snapshot();
        let terminal_evidence = match capability.settle_back_target_permanent_exact(owner) {
            BackTargetPermanentSettlementOutcome::Settled(terminal) => terminal,
            BackTargetPermanentSettlementOutcome::Mismatch { .. } => {
                panic!("the exact receiver must terminal the SourceCapability owner")
            }
        };
        assert_eq!(
            terminal_evidence.testing_evidence_snapshot(),
            capability_owner_snapshot,
            "terminal evidence must conserve the Capability mapping, baselines, and owner facts"
        );
        let after_settlement = capability.snapshot();
        assert_eq!(after_settlement.active(), before_settlement.active());
        assert_eq!(after_settlement.history(), before_settlement.history());
        assert_eq!(after_settlement.running_request(), None);
        assert!(capability.action().is_none());
        let terminal = capability
            .request(capability_request)
            .expect("the SourceCapability terminal request remains retained");
        assert!(matches!(
            terminal.status(),
            RequestStatus::Failed(failure)
                if failure.kind() == crate::request::NavigationFailureKind::AuthorityLost
        ));
        assert_eq!(
            terminal
                .trace()
                .iter()
                .filter(|event| event.kind() == NavigationTraceEventKind::RequestTerminal)
                .count(),
            1
        );
        assert!(
            terminal
                .trace()
                .iter()
                .all(|event| event.kind() != NavigationTraceEventKind::CleanupSettled)
        );

        let mut baseline_drift = direct_back_target_fixture();
        let owner = take_verified_back_target(&mut baseline_drift);
        settle_changed_history_push(
            &mut baseline_drift,
            RouteLocation::new("/").with_fragment("baseline-owner-source-refreeze"),
        );
        let retirement_calls = std::cell::Cell::new(0usize);
        let owner = match baseline_drift.resume_back_target_with_retirement(owner, || {
            retirement_calls.set(retirement_calls.get() + 1);
        }) {
            BackTargetCoreResumeOutcome::Permanent(owner) => owner,
            BackTargetCoreResumeOutcome::Resumed(_) => {
                panic!("a real history push must form a SourceRefreeze permanent owner")
            }
            BackTargetCoreResumeOutcome::Mismatch { .. } => {
                panic!("the exact receiver must classify the real source refreeze")
            }
        };
        assert_eq!(retirement_calls.get(), 1);
        let expected_evidence = owner.evidence();
        let expected_target = owner
            .target_location()
            .expect("the real SourceRefreeze owner retains its target")
            .clone();
        let expected_owner_snapshot = owner.testing_evidence_snapshot();
        assert_eq!(
            expected_owner_snapshot.cause(),
            BackTargetPermanentCauseSnapshot::SourceRefreeze
        );
        assert!(baseline_drift.inner.testing_begin_back_target_host_drift());
        let before_baseline_mismatch = baseline_drift.inner.testing_snapshot();
        let returned = match baseline_drift.settle_back_target_permanent_exact(owner) {
            BackTargetPermanentSettlementOutcome::Mismatch { owner, evidence } => {
                assert_eq!(evidence, expected_evidence);
                owner
            }
            BackTargetPermanentSettlementOutcome::Settled(_) => {
                panic!("a changed complete baseline must return the whole permanent owner")
            }
        };
        assert_eq!(returned.evidence(), expected_evidence);
        assert_eq!(returned.target_location(), Some(&expected_target));
        assert_eq!(
            returned.testing_evidence_snapshot(),
            expected_owner_snapshot
        );
        assert_eq!(
            baseline_drift.classify_back_resolution_evidence(&expected_evidence),
            BackResolutionEvidenceClassification::ConsumedReplay
        );
        assert_eq!(
            baseline_drift.inner.testing_snapshot(),
            before_baseline_mismatch,
            "complete-baseline mismatch must be a semantic zero-write"
        );

        let mut exact = direct_back_target_fixture();
        let owner = take_verified_back_target(&mut exact);
        let request = owner.evidence().request();

        settle_changed_history_push(
            &mut exact,
            RouteLocation::new("/").with_fragment("post-resolution-source-drift"),
        );
        let before_resume = exact.inner.testing_snapshot();
        let retirement_calls = std::cell::Cell::new(0usize);
        let owner = match exact.resume_back_target_with_retirement(owner, || {
            retirement_calls.set(retirement_calls.get() + 1);
        }) {
            BackTargetCoreResumeOutcome::Permanent(owner) => owner,
            BackTargetCoreResumeOutcome::Resumed(_) => {
                panic!("a real post-resolution source drift must settle before ECS splice")
            }
            BackTargetCoreResumeOutcome::Mismatch { .. } => {
                panic!("the exact receiver must classify its source drift as permanent")
            }
        };
        assert_eq!(retirement_calls.get(), 1);
        assert_eq!(exact.inner.testing_snapshot(), before_resume);
        assert!(exact.action().is_none());

        let expected_evidence = owner.evidence();
        assert_eq!(
            exact.classify_back_resolution_evidence(&expected_evidence),
            BackResolutionEvidenceClassification::ConsumedReplay
        );

        let mut foreign = confirmed_engine();
        let foreign_before = foreign.inner.testing_snapshot();
        let exact_before_foreign = exact.inner.testing_snapshot();
        let owner = match foreign.settle_back_target_permanent_exact(owner) {
            BackTargetPermanentSettlementOutcome::Mismatch { owner, evidence } => {
                assert_eq!(evidence, expected_evidence);
                assert_eq!(
                    foreign.classify_back_resolution_evidence(&evidence),
                    BackResolutionEvidenceClassification::Foreign
                );
                owner
            }
            BackTargetPermanentSettlementOutcome::Settled(_) => {
                panic!("a foreign receiver must return the whole permanent owner")
            }
        };
        assert_eq!(foreign.inner.testing_snapshot(), foreign_before);
        assert_eq!(exact.inner.testing_snapshot(), exact_before_foreign);

        let source_owner_snapshot = owner.testing_evidence_snapshot();
        assert_eq!(
            source_owner_snapshot.cause(),
            BackTargetPermanentCauseSnapshot::SourceRefreeze
        );
        let before_settlement = exact.snapshot();
        let terminal_evidence = match exact.settle_back_target_permanent_exact(owner) {
            BackTargetPermanentSettlementOutcome::Settled(terminal) => terminal,
            BackTargetPermanentSettlementOutcome::Mismatch { .. } => {
                panic!("the exact receiver must terminal the returned permanent owner")
            }
        };
        let source_terminal_snapshot = terminal_evidence.testing_evidence_snapshot();

        let after_settlement = exact.snapshot();
        assert_eq!(after_settlement.active(), before_settlement.active());
        assert_eq!(after_settlement.history(), before_settlement.history());
        assert_eq!(after_settlement.running_request(), None);
        assert!(exact.action().is_none());
        let terminal = exact
            .request(request)
            .expect("the terminal Back request must remain retained");
        assert!(matches!(
            terminal.status(),
            RequestStatus::Failed(failure)
                if failure.kind() == crate::request::NavigationFailureKind::AuthorityLost
        ));
        assert_eq!(
            terminal
                .trace()
                .iter()
                .filter(|event| event.kind() == NavigationTraceEventKind::RequestTerminal)
                .count(),
            1
        );
        assert!(
            terminal
                .trace()
                .iter()
                .all(|event| event.kind() != NavigationTraceEventKind::CleanupSettled)
        );

        let mut host = direct_back_target_fixture();
        let owner = take_verified_back_target(&mut host);
        let host_request = owner.evidence().request();
        assert!(host.inner.testing_begin_back_target_host_drift());
        let retirement_calls = std::cell::Cell::new(0usize);
        let owner = match host.resume_back_target_with_retirement(owner, || {
            retirement_calls.set(retirement_calls.get() + 1);
        }) {
            BackTargetCoreResumeOutcome::Permanent(owner) => owner,
            BackTargetCoreResumeOutcome::Resumed(_) => {
                panic!("real Host baseline drift must become permanent before ECS splice")
            }
            BackTargetCoreResumeOutcome::Mismatch { .. } => {
                panic!("the exact receiver must classify its real Host baseline drift")
            }
        };
        assert_eq!(retirement_calls.get(), 1);
        let host_owner_snapshot = owner.testing_evidence_snapshot();
        assert_eq!(
            host_owner_snapshot.cause(),
            BackTargetPermanentCauseSnapshot::AuthorityLost(BackTargetAuthorityLossCause::Host)
        );
        let terminal_evidence = match host.settle_back_target_permanent_exact(owner) {
            BackTargetPermanentSettlementOutcome::Settled(terminal) => terminal,
            BackTargetPermanentSettlementOutcome::Mismatch { .. } => {
                panic!("the exact receiver must terminal the real Host permanent owner")
            }
        };
        let host_terminal_snapshot = terminal_evidence.testing_evidence_snapshot();
        let terminal = host
            .request(host_request)
            .expect("the exact Host settlement must retain its terminal request");
        assert!(matches!(
            terminal.status(),
            RequestStatus::Failed(failure)
                if failure.kind() == crate::request::NavigationFailureKind::AuthorityLost
        ));
        assert_eq!(
            terminal
                .trace()
                .iter()
                .filter(|event| event.kind() == NavigationTraceEventKind::RequestTerminal)
                .count(),
            1
        );

        let mut generation = direct_back_target_fixture();
        generation
            .inner
            .testing_set_active_generation(RouteGeneration::testing_max());
        let owner = take_verified_back_target(&mut generation);
        let generation_request = owner.evidence().request();
        let retirement_calls = std::cell::Cell::new(0usize);
        let owner = match generation.resume_back_target_with_retirement(owner, || {
            retirement_calls.set(retirement_calls.get() + 1);
        }) {
            BackTargetCoreResumeOutcome::Permanent(owner) => owner,
            BackTargetCoreResumeOutcome::Resumed(_) => {
                panic!("real generation exhaustion must become permanent before ECS splice")
            }
            BackTargetCoreResumeOutcome::Mismatch { .. } => {
                panic!("the exact receiver must classify its real generation exhaustion")
            }
        };
        assert_eq!(retirement_calls.get(), 1);
        let generation_owner_snapshot = owner.testing_evidence_snapshot();
        assert_eq!(
            generation_owner_snapshot.cause(),
            BackTargetPermanentCauseSnapshot::AuthorityLost(
                BackTargetAuthorityLossCause::Generation
            )
        );
        let terminal_evidence = match generation.settle_back_target_permanent_exact(owner) {
            BackTargetPermanentSettlementOutcome::Settled(terminal) => terminal,
            BackTargetPermanentSettlementOutcome::Mismatch { .. } => {
                panic!("the exact receiver must terminal the real Generation permanent owner")
            }
        };
        let generation_terminal_snapshot = terminal_evidence.testing_evidence_snapshot();
        let terminal = generation
            .request(generation_request)
            .expect("the exact Generation settlement must retain its terminal request");
        assert!(matches!(
            terminal.status(),
            RequestStatus::Failed(failure)
                if failure.kind() == crate::request::NavigationFailureKind::AuthorityLost
        ));
        assert_eq!(
            terminal
                .trace()
                .iter()
                .filter(|event| event.kind() == NavigationTraceEventKind::RequestTerminal)
                .count(),
            1
        );

        assert_eq!(
            (
                source_terminal_snapshot,
                host_terminal_snapshot,
                generation_terminal_snapshot,
            ),
            (
                source_owner_snapshot,
                host_owner_snapshot,
                generation_owner_snapshot,
            ),
            "each terminal must retain its real cause, expected/observed source mapping, and complete expected/observed baseline"
        );
    }

    fn rust_function_body<'a>(source: &'a str, signature: &str) -> &'a str {
        let start = source
            .find(signature)
            .unwrap_or_else(|| panic!("missing Rust function signature `{signature}`"));
        let open = source[start..]
            .find('{')
            .map(|offset| start + offset)
            .unwrap_or_else(|| panic!("missing body for Rust function `{signature}`"));
        let mut depth = 0usize;
        for (offset, byte) in source.as_bytes()[open..].iter().copied().enumerate() {
            match byte {
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        return &source[open + 1..open + offset];
                    }
                }
                _ => {}
            }
        }
        panic!("unterminated Rust function `{signature}`")
    }

    #[test]
    fn back_target_callback_tail_is_infallible_and_query_free() {
        let source = include_str!("engine.rs");
        let install = rust_function_body(source, "fn install_prepared_back_target_start(");
        let prepare = rust_function_body(source, "fn prepare_back_target_start(");
        let mount_success = prepare
            .split_once("return match self.mount.issue(")
            .and_then(|(_, tail)| tail.split_once("Err(error)"))
            .map(|(success, _)| success)
            .expect("prepared target mount issue keeps explicit success and failure arms");
        let mut violations = Vec::new();

        for forbidden in [
            "self.records[record_index]",
            ".clone()",
            ".to_vec()",
            ".push(",
            ".position(",
            ".get(",
            "matcher.",
            "mount.",
            "history.",
            ".validate",
            "if ",
            ".expect(",
            ".unwrap(",
            "unreachable!(",
        ] {
            if install.contains(forbidden) {
                violations.push(format!("callback tail contains `{forbidden}`"));
            }
        }
        if !install.contains("self.records = records") {
            violations.push(
                "callback tail does not install one prebuilt whole records replacement".to_owned(),
            );
        }
        for forbidden in [
            ".clone()",
            ".to_vec()",
            ".push(",
            "matcher.",
            ".position(",
            ".validate",
            ".expect(",
            ".unwrap(",
        ] {
            if mount_success.contains(forbidden) {
                violations.push(format!(
                    "mount issue success performs late preparation `{forbidden}`"
                ));
            }
        }

        assert!(
            violations.is_empty(),
            "all fallible or allocating work must close before retirement: {violations:?}"
        );
    }

    #[test]
    fn back_target_receiver_mismatch_calls_zero_and_exact_receiver_calls_once() {
        for slot in [
            BackTargetBusyReceiverSlot::Action,
            BackTargetBusyReceiverSlot::Current,
            BackTargetBusyReceiverSlot::PendingNoTargetBack,
            BackTargetBusyReceiverSlot::PendingBackNoTargetAuthority,
            BackTargetBusyReceiverSlot::CurrentHistoryCapability,
            BackTargetBusyReceiverSlot::PendingHistoryPreparationError,
            BackTargetBusyReceiverSlot::PreparedPublicationAuthority,
            BackTargetBusyReceiverSlot::CurrentRequest,
        ] {
            assert_back_target_busy_receiver_calls_zero(slot);
        }

        let mut frame_task = confirmed_engine();
        settle_push(
            &mut frame_task,
            RouteLocation::new("/").with_fragment("back-target"),
        );
        settle_changed_history_push(
            &mut frame_task,
            RouteLocation::new("/").with_fragment("back-source"),
        );
        assert_back_target_receiver_fixture(frame_task, ExpectedBackTargetResumeAction::FrameTask);

        let mut commit = confirmed_engine();
        let duplicate = RouteLocation::new("/");
        settle_push(&mut commit, duplicate.clone());
        settle_push(&mut commit, duplicate);
        assert_back_target_receiver_fixture(commit, ExpectedBackTargetResumeAction::Commit);

        assert_back_target_receiver_fixture(
            nested_mount_back_fixture(),
            ExpectedBackTargetResumeAction::MountRequest,
        );

        let mut exhausted = nested_mount_back_fixture();
        let owner = take_verified_back_target(&mut exhausted);
        let owner = assert_back_target_wrong_receiver_calls_zero(owner);
        exhausted.inner.testing_exhaust_mount_sequence_counter();
        let callback_calls = std::cell::Cell::new(0);
        let outcome = exhausted.resume_back_target_with_retirement(owner, || {
            callback_calls.set(callback_calls.get() + 1);
        });
        assert_eq!(callback_calls.get(), 1);
        assert!(matches!(
            outcome,
            BackTargetCoreResumeOutcome::Resumed(BackTargetStart::FailureCleanup(_))
        ));

        let mut permanent = confirmed_engine();
        settle_push(
            &mut permanent,
            RouteLocation::new("/").with_fragment("permanent-target"),
        );
        settle_changed_history_push(
            &mut permanent,
            RouteLocation::new("/").with_fragment("permanent-source"),
        );
        let owner = take_verified_back_target(&mut permanent);
        let owner = assert_back_target_wrong_receiver_calls_zero(owner);
        settle_changed_history_push(
            &mut permanent,
            RouteLocation::new("/").with_fragment("post-resolution-source-drift"),
        );
        let before = permanent.inner.testing_snapshot();
        let callback_calls = std::cell::Cell::new(0);
        let outcome = permanent.resume_back_target_with_retirement(owner, || {
            callback_calls.set(callback_calls.get() + 1);
        });
        assert_eq!(callback_calls.get(), 1);
        assert!(matches!(outcome, BackTargetCoreResumeOutcome::Permanent(_)));
        assert_eq!(
            permanent.inner.testing_snapshot(),
            before,
            "source drift must be classified before matcher/reconciliation writes"
        );
        assert!(
            permanent.action().is_none(),
            "typed permanent source drift must not install current work or an action"
        );
    }

    #[test]
    fn back_target_execution_pairing_mismatch_freezes_expected_task_and_observed_current() {
        let (mut exact, history, continuation, task) = take_back_target_driver_frame_task();
        let task_identity = task.inner.identity().clone();
        assert!(
            exact.testing_drift_current_frame_task_identity(),
            "the fixture must drift the real current frame-task receiver"
        );
        let before = exact.inner.testing_snapshot();

        let mismatch = match exact.validate_back_target_execution_pairing(
            BackTargetExecutionOwnerRef::Driving {
                history: &history,
                continuation: &continuation,
            },
            BackTargetPhysicalOwnerRef::FrameTask(&task),
        ) {
            BackTargetExecutionPairingOutcome::Mismatch(mismatch) => mismatch,
            BackTargetExecutionPairingOutcome::Matched => {
                panic!("the drifted current receiver must not match the exact task")
            }
        };

        assert_eq!(
            exact.inner.testing_snapshot(),
            before,
            "execution pairing mismatch must be a semantic zero-write"
        );
        match mismatch.inner {
            CorePreparationReceiverMismatchProjection::Execution { expected, observed } => {
                assert_eq!(
                    expected.physical,
                    crate::engine::CoreBackTargetReceiverActionProjection::FrameTask(
                        task_identity.clone()
                    )
                );
                assert_ne!(
                    Some(&task_identity),
                    observed.current.as_ref(),
                    "the projection must retain the distinct expected task and observed current"
                );
            }
            CorePreparationReceiverMismatchProjection::Scope { .. }
            | CorePreparationReceiverMismatchProjection::LogicalZero { .. }
            | CorePreparationReceiverMismatchProjection::BackNoTargetReady(_)
            | CorePreparationReceiverMismatchProjection::BackNoTargetCompletion { .. }
            | CorePreparationReceiverMismatchProjection::BackNoTargetAuthorityBegin { .. }
            | CorePreparationReceiverMismatchProjection::BackNoTargetAuthorityCompletion {
                ..
            }
            | CorePreparationReceiverMismatchProjection::Continuation { .. }
            | CorePreparationReceiverMismatchProjection::Applied { .. }
            | CorePreparationReceiverMismatchProjection::DriverFailureCleanup { .. }
            | CorePreparationReceiverMismatchProjection::DriverFallback { .. }
            | CorePreparationReceiverMismatchProjection::DriverTerminal { .. } => {
                panic!("target execution mismatch must use its tagged expected/observed projection")
            }
        }
    }

    #[test]
    fn back_target_ready_wrong_receiver_freezes_actual_receiver_state() {
        let (_exact, _history, capacity, _task) = take_back_target_driver_frame_task();
        let expected_scope = capacity.inner.scope.clone();
        let boundary = capacity.preparation_boundary_scope();
        let mut foreign = confirmed_engine();
        let before = foreign.inner.testing_snapshot();

        let (capacity, mismatch) = match foreign.accept_back_target_ready(capacity, boundary) {
            BackTargetPreparationAcceptance::Mismatch { capacity, mismatch } => {
                (capacity, mismatch)
            }
            BackTargetPreparationAcceptance::Matched(_) => {
                panic!("a foreign continuation receiver must not accept the exact capacity")
            }
        };
        assert_eq!(
            foreign.inner.testing_snapshot(),
            before,
            "wrong-receiver classification must be a semantic zero-write"
        );
        assert_eq!(capacity.inner.scope, expected_scope);

        match mismatch.inner {
            CorePreparationReceiverMismatchProjection::Continuation { expected, observed } => {
                assert_eq!(&expected.scope, &expected_scope);
                assert!(expected.commit.is_none());
                assert!(
                    !expected.scope.matches_engine_scope(
                        observed.scope.world,
                        observed.scope.router,
                        observed.scope.history,
                    ),
                    "the observed engine scope must be the actual foreign receiver"
                );
                assert!(observed.history.is_none());
                assert!(observed.current.is_none());
                assert_eq!(
                    observed.action,
                    crate::engine::CoreBackTargetReceiverActionProjection::Missing
                );
                assert_eq!(
                    observed.stage,
                    crate::engine::CoreBackTargetReceiverStageProjection::Missing
                );
            }
            CorePreparationReceiverMismatchProjection::Scope { .. }
            | CorePreparationReceiverMismatchProjection::LogicalZero { .. }
            | CorePreparationReceiverMismatchProjection::BackNoTargetReady(_)
            | CorePreparationReceiverMismatchProjection::BackNoTargetCompletion { .. }
            | CorePreparationReceiverMismatchProjection::BackNoTargetAuthorityBegin { .. }
            | CorePreparationReceiverMismatchProjection::BackNoTargetAuthorityCompletion {
                ..
            }
            | CorePreparationReceiverMismatchProjection::Execution { .. }
            | CorePreparationReceiverMismatchProjection::Applied { .. }
            | CorePreparationReceiverMismatchProjection::DriverFailureCleanup { .. }
            | CorePreparationReceiverMismatchProjection::DriverFallback { .. }
            | CorePreparationReceiverMismatchProjection::DriverTerminal { .. } => {
                panic!("wrong receiver must return its typed continuation observation")
            }
        }
    }

    #[test]
    fn back_target_commit_preparation_drift_freezes_actual_receiver_state() {
        let (mut engine, history, capacity, task) = take_back_target_driver_frame_task();
        let (capacity, ticket) = match engine.report_back_target_applied(
            &history,
            capacity,
            task,
            BackTargetAppliedReport::LegacyLeaf,
        ) {
            BackTargetAppliedReportOutcome::Commit { capacity, ticket } => (capacity, ticket),
            BackTargetAppliedReportOutcome::MountRequest { .. }
            | BackTargetAppliedReportOutcome::FailureCleanup(_)
            | BackTargetAppliedReportOutcome::Mismatch { .. } => {
                panic!("the exact leaf report must advance to one commit ticket")
            }
        };
        let boundary = capacity.preparation_boundary_scope();
        let owner = match engine.accept_back_target_ready(capacity, boundary) {
            BackTargetPreparationAcceptance::Matched(owner) => owner,
            BackTargetPreparationAcceptance::Mismatch { .. } => {
                panic!("the unchanged commit receiver must accept its continuation capacity")
            }
        };
        let expected_scope = owner.inner.binding.scope.clone();
        let expected_current = ticket.inner.identity().clone();
        engine.testing_invalidate_commit_action();
        let before = engine.inner.testing_snapshot();

        let (owner, manifest, mismatch) =
            match engine.prepare_back_target_commit(owner, &ticket, CleanupManifest::empty()) {
                BackTargetCommitPreparationOutcome::Mismatch {
                    owner,
                    manifest,
                    mismatch,
                } => (owner, manifest, mismatch),
                BackTargetCommitPreparationOutcome::Validated(_)
                | BackTargetCommitPreparationOutcome::PreparationFailed(_) => {
                    panic!("the drifted commit receiver must retain the whole preparation input")
                }
            };
        assert_eq!(
            engine.inner.testing_snapshot(),
            before,
            "commit receiver drift must be classified before preparation writes"
        );
        assert_eq!(owner.inner.binding.scope, expected_scope);
        assert!(manifest.logical_roots.is_empty());
        assert_eq!(manifest.manifest_digest, 0);
        assert_eq!(manifest.obligation_count, 0);

        match mismatch.inner {
            CorePreparationReceiverMismatchProjection::Continuation { expected, observed } => {
                assert_eq!(&expected.scope, &expected_scope);
                assert_eq!(expected.commit.as_ref(), Some(&expected_current));
                assert!(expected.scope.matches_engine_scope(
                    observed.scope.world,
                    observed.scope.router,
                    observed.scope.history,
                ));
                assert_eq!(observed.history.as_ref(), Some(&history.inner));
                assert!(
                    expected
                        .scope
                        .matches_execution(observed.history.as_ref().unwrap())
                );
                assert_eq!(observed.current.as_ref(), Some(&expected_current));
                assert_eq!(
                    observed.action,
                    crate::engine::CoreBackTargetReceiverActionProjection::Missing
                );
                assert_eq!(
                    observed.stage,
                    crate::engine::CoreBackTargetReceiverStageProjection::Applying
                );
            }
            CorePreparationReceiverMismatchProjection::Scope { .. }
            | CorePreparationReceiverMismatchProjection::LogicalZero { .. }
            | CorePreparationReceiverMismatchProjection::BackNoTargetReady(_)
            | CorePreparationReceiverMismatchProjection::BackNoTargetCompletion { .. }
            | CorePreparationReceiverMismatchProjection::BackNoTargetAuthorityBegin { .. }
            | CorePreparationReceiverMismatchProjection::BackNoTargetAuthorityCompletion {
                ..
            }
            | CorePreparationReceiverMismatchProjection::Execution { .. }
            | CorePreparationReceiverMismatchProjection::Applied { .. }
            | CorePreparationReceiverMismatchProjection::DriverFailureCleanup { .. }
            | CorePreparationReceiverMismatchProjection::DriverFallback { .. }
            | CorePreparationReceiverMismatchProjection::DriverTerminal { .. } => {
                panic!("commit receiver drift must return typed continuation evidence")
            }
        }
    }

    #[test]
    fn back_target_commit_preparation_ambient_action_drift_is_typed_and_zero_write() {
        let (mut engine, history, capacity, task) = take_back_target_driver_frame_task();
        let ambient_identity = task.inner.identity().clone();
        let ambient_action = CoreEngineAction::FrameTask(task.inner.testing_duplicate());
        let (capacity, ticket) = match engine.report_back_target_applied(
            &history,
            capacity,
            task,
            BackTargetAppliedReport::LegacyLeaf,
        ) {
            BackTargetAppliedReportOutcome::Commit { capacity, ticket } => (capacity, ticket),
            BackTargetAppliedReportOutcome::MountRequest { .. }
            | BackTargetAppliedReportOutcome::FailureCleanup(_)
            | BackTargetAppliedReportOutcome::Mismatch { .. } => {
                panic!("the exact leaf report must advance to one commit ticket")
            }
        };
        let logical_roots = engine
            .reconciliation_projection_for_commit(&ticket)
            .unwrap()
            .leaving()
            .map(|leaving| leaving.version().clone())
            .collect::<Vec<_>>();
        let obligation_count = logical_roots.len();
        let manifest = CleanupManifest::new(logical_roots, 0x414d_4249_454e_54, obligation_count);
        let boundary = capacity.preparation_boundary_scope();
        let owner = match engine.accept_back_target_ready(capacity, boundary) {
            BackTargetPreparationAcceptance::Matched(owner) => owner,
            BackTargetPreparationAcceptance::Mismatch { .. } => {
                panic!("the unchanged commit receiver must accept its continuation capacity")
            }
        };
        let expected_scope = owner.inner.binding.scope.clone();
        let expected_commit = ticket.inner.identity().clone();
        engine.inner.restore_action(ambient_action);
        let before = engine.inner.testing_snapshot();

        let (owner, manifest, mismatch) =
            match engine.prepare_back_target_commit(owner, &ticket, manifest) {
                BackTargetCommitPreparationOutcome::Mismatch {
                    owner,
                    manifest,
                    mismatch,
                } => (owner, manifest, mismatch),
                BackTargetCommitPreparationOutcome::Validated(_)
                | BackTargetCommitPreparationOutcome::PreparationFailed(_) => {
                    panic!("ambient action drift must be rejected by the continuation receiver")
                }
            };
        assert_eq!(
            engine.inner.testing_snapshot(),
            before,
            "ambient action drift must be a semantic zero-write"
        );
        assert_eq!(owner.inner.binding.scope, expected_scope);
        assert_eq!(manifest.obligation_count, obligation_count);
        match mismatch.inner {
            CorePreparationReceiverMismatchProjection::Continuation { expected, observed } => {
                assert_eq!(&expected.scope, &expected_scope);
                assert_eq!(expected.commit.as_ref(), Some(&expected_commit));
                assert!(expected.scope.matches_engine_scope(
                    observed.scope.world,
                    observed.scope.router,
                    observed.scope.history,
                ));
                assert_eq!(observed.history.as_ref(), Some(&history.inner));
                assert_eq!(observed.current.as_ref(), Some(&expected_commit));
                assert_eq!(
                    observed.action,
                    crate::engine::CoreBackTargetReceiverActionProjection::FrameTask(
                        ambient_identity
                    )
                );
                assert_eq!(
                    observed.stage,
                    crate::engine::CoreBackTargetReceiverStageProjection::ReadyToCommit
                );
            }
            CorePreparationReceiverMismatchProjection::Scope { .. }
            | CorePreparationReceiverMismatchProjection::LogicalZero { .. }
            | CorePreparationReceiverMismatchProjection::BackNoTargetReady(_)
            | CorePreparationReceiverMismatchProjection::BackNoTargetCompletion { .. }
            | CorePreparationReceiverMismatchProjection::BackNoTargetAuthorityBegin { .. }
            | CorePreparationReceiverMismatchProjection::BackNoTargetAuthorityCompletion {
                ..
            }
            | CorePreparationReceiverMismatchProjection::Execution { .. }
            | CorePreparationReceiverMismatchProjection::Applied { .. }
            | CorePreparationReceiverMismatchProjection::DriverFailureCleanup { .. }
            | CorePreparationReceiverMismatchProjection::DriverFallback { .. }
            | CorePreparationReceiverMismatchProjection::DriverTerminal { .. } => {
                panic!("ambient action drift must return typed continuation evidence")
            }
        }
    }

    #[test]
    fn back_target_applied_same_engine_task_drift_is_zero_write_and_returns_whole_input() {
        let (mut engine, history, capacity, task) = take_back_target_driver_frame_task();
        let expected_scope = capacity.inner.scope.clone();
        let expected_task = task.inner.identity().clone();
        assert!(
            engine.testing_drift_current_frame_task_identity(),
            "the fixture must drift only the real current frame-task receiver"
        );
        let before = engine.inner.testing_snapshot();

        let outcome = engine.report_back_target_applied(
            &history,
            capacity,
            task,
            BackTargetAppliedReport::LegacyLeaf,
        );
        assert_eq!(
            engine.inner.testing_snapshot(),
            before,
            "same-engine task drift must be rejected before applied-report mutation"
        );

        let (capacity, task, report, mismatch) = match outcome {
            BackTargetAppliedReportOutcome::Mismatch {
                capacity,
                task,
                report,
                mismatch,
            } => (capacity, task, report, mismatch),
            BackTargetAppliedReportOutcome::MountRequest { .. }
            | BackTargetAppliedReportOutcome::Commit { .. }
            | BackTargetAppliedReportOutcome::FailureCleanup(_) => {
                panic!("same-engine task drift must return the whole atomic applied input")
            }
        };
        assert_eq!(capacity.inner.scope, expected_scope);
        assert_eq!(task.inner.identity(), &expected_task);
        assert!(matches!(report, BackTargetAppliedReport::LegacyLeaf));
        assert!(matches!(
            mismatch.inner,
            CorePreparationReceiverMismatchProjection::Execution { .. }
        ));
    }

    #[test]
    fn back_target_applied_rejects_same_engine_ambient_action_collision_before_mutation() {
        let (mut engine, history, capacity, task) = take_back_target_driver_frame_task();
        let expected_scope = capacity.inner.scope.clone();
        let expected_task = task.inner.identity().clone();
        engine
            .inner
            .restore_action(CoreEngineAction::FrameTask(task.inner.testing_duplicate()));
        let before = engine.inner.testing_snapshot();

        let outcome = engine.report_back_target_applied(
            &history,
            capacity,
            task,
            BackTargetAppliedReport::LegacyLeaf,
        );
        assert_eq!(
            engine.inner.testing_snapshot(),
            before,
            "an occupied ambient action slot must be rejected before report mutation or action loss"
        );

        let (capacity, task, report, mismatch) = match outcome {
            BackTargetAppliedReportOutcome::Mismatch {
                capacity,
                task,
                report,
                mismatch,
            } => (capacity, task, report, mismatch),
            BackTargetAppliedReportOutcome::MountRequest { .. }
            | BackTargetAppliedReportOutcome::Commit { .. }
            | BackTargetAppliedReportOutcome::FailureCleanup(_) => {
                panic!("an occupied ambient action slot must return the whole applied input")
            }
        };
        assert_eq!(capacity.inner.scope, expected_scope);
        assert_eq!(task.inner.identity(), &expected_task);
        assert!(matches!(report, BackTargetAppliedReport::LegacyLeaf));
        assert!(matches!(
            mismatch.inner,
            CorePreparationReceiverMismatchProjection::Execution { .. }
        ));
    }

    #[test]
    fn back_target_applied_invalid_nested_legacy_report_returns_tagged_evidence_and_whole_input() {
        let (mut engine, history, capacity, task) = take_back_target_nested_parent_frame_task();
        let expected_scope = capacity.inner.scope.clone();
        let expected_task = task.inner.identity().clone();
        let expected_child_outlet = engine
            .expected_child_outlet(&task)
            .expect("the exact task must remain a live report receiver")
            .expect("the nested parent must require one child outlet");
        let before = engine.inner.testing_snapshot();

        let outcome = engine.report_back_target_applied(
            &history,
            capacity,
            task,
            BackTargetAppliedReport::LegacyLeaf,
        );
        assert_eq!(
            engine.inner.testing_snapshot(),
            before,
            "invalid LegacyLeaf input must be rejected before applied-report mutation"
        );

        let (capacity, task, report, mismatch) = match outcome {
            BackTargetAppliedReportOutcome::Mismatch {
                capacity,
                task,
                report,
                mismatch,
            } => (capacity, task, report, mismatch),
            BackTargetAppliedReportOutcome::MountRequest { .. }
            | BackTargetAppliedReportOutcome::Commit { .. }
            | BackTargetAppliedReportOutcome::FailureCleanup(_) => {
                panic!("invalid LegacyLeaf input must return the whole atomic applied input")
            }
        };
        assert_eq!(capacity.inner.scope, expected_scope);
        assert_eq!(task.inner.identity(), &expected_task);
        assert!(matches!(report, BackTargetAppliedReport::LegacyLeaf));
        match mismatch.inner {
            CorePreparationReceiverMismatchProjection::Applied { expected, observed } => {
                assert_eq!(
                    expected.report,
                    crate::engine::CoreBackTargetAppliedReportProjection::LegacyLeaf
                );
                assert_eq!(observed.next_outlet, Some(expected_child_outlet));
            }
            CorePreparationReceiverMismatchProjection::Scope { .. }
            | CorePreparationReceiverMismatchProjection::LogicalZero { .. }
            | CorePreparationReceiverMismatchProjection::BackNoTargetReady(_)
            | CorePreparationReceiverMismatchProjection::BackNoTargetCompletion { .. }
            | CorePreparationReceiverMismatchProjection::BackNoTargetAuthorityBegin { .. }
            | CorePreparationReceiverMismatchProjection::BackNoTargetAuthorityCompletion {
                ..
            }
            | CorePreparationReceiverMismatchProjection::Continuation { .. }
            | CorePreparationReceiverMismatchProjection::Execution { .. }
            | CorePreparationReceiverMismatchProjection::DriverFailureCleanup { .. }
            | CorePreparationReceiverMismatchProjection::DriverFallback { .. }
            | CorePreparationReceiverMismatchProjection::DriverTerminal { .. } => {
                panic!("invalid applied input must use its tagged expected/observed projection")
            }
        }
    }

    #[test]
    fn back_target_driver_report_wrong_receiver_is_zero_write_and_returns_exact_inputs() {
        let (mut exact, history, continuation, task) = take_back_target_driver_frame_task();
        let task_identity = task.inner.identity().clone();
        let mut foreign = confirmed_engine();
        let foreign_before = foreign.inner.testing_snapshot();

        let (continuation, task, mismatch) = match foreign.report_back_target_failed(
            &history,
            continuation,
            task,
            ViewFailureKind::Build,
            "wrong report receiver must return the exact inputs",
            BackTargetDriverFailureReportMode::Normal,
        ) {
            BackTargetDriverFailureReportOutcome::Mismatch {
                capacity,
                task,
                mismatch,
            } => (capacity, task, mismatch),
            BackTargetDriverFailureReportOutcome::Reported(_) => {
                panic!("a foreign report receiver must not consume the exact inputs")
            }
        };
        assert_eq!(
            foreign.inner.testing_snapshot(),
            foreign_before,
            "the foreign report receiver must be a semantic zero-write"
        );
        match mismatch.inner {
            CorePreparationReceiverMismatchProjection::Execution { expected, observed } => {
                assert_eq!(
                    expected.physical,
                    crate::engine::CoreBackTargetReceiverActionProjection::FrameTask(task_identity)
                );
                assert!(
                    observed.current.is_none(),
                    "the foreign receiver must freeze its absent current task"
                );
            }
            CorePreparationReceiverMismatchProjection::Scope { .. }
            | CorePreparationReceiverMismatchProjection::LogicalZero { .. }
            | CorePreparationReceiverMismatchProjection::BackNoTargetReady(_)
            | CorePreparationReceiverMismatchProjection::BackNoTargetCompletion { .. }
            | CorePreparationReceiverMismatchProjection::BackNoTargetAuthorityBegin { .. }
            | CorePreparationReceiverMismatchProjection::BackNoTargetAuthorityCompletion {
                ..
            }
            | CorePreparationReceiverMismatchProjection::Continuation { .. }
            | CorePreparationReceiverMismatchProjection::Applied { .. }
            | CorePreparationReceiverMismatchProjection::DriverFailureCleanup { .. }
            | CorePreparationReceiverMismatchProjection::DriverFallback { .. }
            | CorePreparationReceiverMismatchProjection::DriverTerminal { .. } => {
                panic!("target report mismatch must retain execution receiver facts")
            }
        }

        let outcome = match exact.report_back_target_failed(
            &history,
            continuation,
            task,
            ViewFailureKind::Build,
            "returned report inputs must remain exact",
            BackTargetDriverFailureReportMode::Normal,
        ) {
            BackTargetDriverFailureReportOutcome::Reported(outcome) => outcome,
            BackTargetDriverFailureReportOutcome::Mismatch { .. } => {
                panic!(
                    "the returned capacity and task must remain exact for their original receiver"
                )
            }
        };
        assert!(matches!(
            &outcome,
            BackTargetDriverFailureCoreOutcome::Fallback(_)
        ));
        let exact_before_consume = exact.inner.testing_snapshot();
        assert!(matches!(
            exact.consume_back_target_driver_failure(outcome),
            BackTargetDriverFailureConsumeOutcome::FallbackReleased
        ));
        assert_eq!(
            exact.inner.testing_snapshot(),
            exact_before_consume,
            "exact consumption must leave the installed fallback action engine-held"
        );
    }

    #[test]
    fn back_target_driver_consume_wrong_receiver_is_zero_write_and_returns_whole_owner() {
        let (mut exact, history, continuation, task) = take_back_target_driver_frame_task();
        let outcome = match exact.report_back_target_failed(
            &history,
            continuation,
            task,
            ViewFailureKind::Apply,
            "exact report must create one whole fallback owner",
            BackTargetDriverFailureReportMode::Normal,
        ) {
            BackTargetDriverFailureReportOutcome::Reported(outcome) => outcome,
            BackTargetDriverFailureReportOutcome::Mismatch { .. } => {
                panic!("the exact report receiver must match")
            }
        };
        assert!(matches!(
            &outcome,
            BackTargetDriverFailureCoreOutcome::Fallback(_)
        ));

        let mut foreign = confirmed_engine();
        let foreign_before = foreign.inner.testing_snapshot();
        let outcome = match foreign.consume_back_target_driver_failure(outcome) {
            BackTargetDriverFailureConsumeOutcome::Mismatch { owner, mismatch: _ } => owner,
            BackTargetDriverFailureConsumeOutcome::FailureCleanup(_)
            | BackTargetDriverFailureConsumeOutcome::FallbackReleased
            | BackTargetDriverFailureConsumeOutcome::Terminal(_) => {
                panic!("a foreign consumer must return the whole Core owner")
            }
        };
        assert_eq!(
            foreign.inner.testing_snapshot(),
            foreign_before,
            "the foreign consumer must be a semantic zero-write"
        );
        assert!(matches!(
            &outcome,
            BackTargetDriverFailureCoreOutcome::Fallback(_)
        ));

        let exact_before_consume = exact.inner.testing_snapshot();
        assert!(matches!(
            exact.consume_back_target_driver_failure(outcome),
            BackTargetDriverFailureConsumeOutcome::FallbackReleased
        ));
        assert_eq!(
            exact.inner.testing_snapshot(),
            exact_before_consume,
            "the returned whole owner must remain consumable without taking the fallback action"
        );
    }

    #[test]
    fn back_target_driver_consume_wrong_receiver_uses_tagged_expected_observed_projection() {
        let (mut exact, history, continuation, task) = take_back_target_driver_frame_task();
        let outcome = match exact.report_back_target_failed(
            &history,
            continuation,
            task,
            ViewFailureKind::Apply,
            "exact report must create one whole fallback owner",
            BackTargetDriverFailureReportMode::Normal,
        ) {
            BackTargetDriverFailureReportOutcome::Reported(outcome) => outcome,
            BackTargetDriverFailureReportOutcome::Mismatch { .. } => {
                panic!("the exact report receiver must match")
            }
        };
        assert!(matches!(
            &outcome,
            BackTargetDriverFailureCoreOutcome::Fallback(_)
        ));

        let mut foreign = confirmed_engine();
        let before = foreign.inner.testing_snapshot();
        let (outcome, mismatch) = match foreign.consume_back_target_driver_failure(outcome) {
            BackTargetDriverFailureConsumeOutcome::Mismatch { owner, mismatch } => {
                (owner, mismatch)
            }
            BackTargetDriverFailureConsumeOutcome::FailureCleanup(_)
            | BackTargetDriverFailureConsumeOutcome::FallbackReleased
            | BackTargetDriverFailureConsumeOutcome::Terminal(_) => {
                panic!("a foreign consumer must return the whole fallback owner")
            }
        };
        assert_eq!(
            foreign.inner.testing_snapshot(),
            before,
            "fallback receiver mismatch must be a semantic zero-write"
        );
        match mismatch.inner {
            CorePreparationReceiverMismatchProjection::DriverFallback { expected, observed } => {
                assert_ne!(
                    expected.action, observed.action,
                    "the mismatch must retain expected and observed action facts"
                );
            }
            CorePreparationReceiverMismatchProjection::Scope { .. }
            | CorePreparationReceiverMismatchProjection::LogicalZero { .. }
            | CorePreparationReceiverMismatchProjection::BackNoTargetReady(_)
            | CorePreparationReceiverMismatchProjection::BackNoTargetCompletion { .. }
            | CorePreparationReceiverMismatchProjection::BackNoTargetAuthorityBegin { .. }
            | CorePreparationReceiverMismatchProjection::BackNoTargetAuthorityCompletion {
                ..
            }
            | CorePreparationReceiverMismatchProjection::Continuation { .. }
            | CorePreparationReceiverMismatchProjection::Execution { .. }
            | CorePreparationReceiverMismatchProjection::Applied { .. }
            | CorePreparationReceiverMismatchProjection::DriverFailureCleanup { .. }
            | CorePreparationReceiverMismatchProjection::DriverTerminal { .. } => {
                panic!("fallback mismatch must use its tagged expected/observed projection")
            }
        }

        let exact_before_consume = exact.inner.testing_snapshot();
        assert!(matches!(
            exact.consume_back_target_driver_failure(outcome),
            BackTargetDriverFailureConsumeOutcome::FallbackReleased
        ));
        assert_eq!(
            exact.inner.testing_snapshot(),
            exact_before_consume,
            "the returned whole fallback owner must remain exact"
        );
    }

    #[test]
    fn back_target_driver_cleanup_wrong_receiver_uses_tagged_expected_observed_projection() {
        let (mut exact, history, continuation, task) = take_back_target_driver_frame_task();
        let outcome = match exact.report_back_target_failed(
            &history,
            continuation,
            task,
            ViewFailureKind::Apply,
            "physical residual must create one whole cleanup owner",
            BackTargetDriverFailureReportMode::PhysicalResidual,
        ) {
            BackTargetDriverFailureReportOutcome::Reported(outcome) => outcome,
            BackTargetDriverFailureReportOutcome::Mismatch { .. } => {
                panic!("the exact report receiver must match")
            }
        };
        assert!(matches!(
            &outcome,
            BackTargetDriverFailureCoreOutcome::FailureCleanup(_)
        ));

        let mut foreign = confirmed_engine();
        let before = foreign.inner.testing_snapshot();
        let (outcome, mismatch) = match foreign.consume_back_target_driver_failure(outcome) {
            BackTargetDriverFailureConsumeOutcome::Mismatch { owner, mismatch } => {
                (owner, mismatch)
            }
            BackTargetDriverFailureConsumeOutcome::FailureCleanup(_)
            | BackTargetDriverFailureConsumeOutcome::FallbackReleased
            | BackTargetDriverFailureConsumeOutcome::Terminal(_) => {
                panic!("a foreign consumer must return the whole cleanup owner")
            }
        };
        assert_eq!(
            foreign.inner.testing_snapshot(),
            before,
            "cleanup receiver mismatch must be a semantic zero-write"
        );
        match mismatch.inner {
            CorePreparationReceiverMismatchProjection::DriverFailureCleanup {
                expected,
                observed,
            } => {
                assert_ne!(
                    observed.stage,
                    crate::engine::CoreBackTargetReceiverStageProjection::FailureCleanupManifestPending(
                        expected.request.receiver
                    ),
                    "the mismatch must retain the expected request and observed cleanup stage"
                );
            }
            CorePreparationReceiverMismatchProjection::Scope { .. }
            | CorePreparationReceiverMismatchProjection::LogicalZero { .. }
            | CorePreparationReceiverMismatchProjection::BackNoTargetReady(_)
            | CorePreparationReceiverMismatchProjection::BackNoTargetCompletion { .. }
            | CorePreparationReceiverMismatchProjection::BackNoTargetAuthorityBegin { .. }
            | CorePreparationReceiverMismatchProjection::BackNoTargetAuthorityCompletion {
                ..
            }
            | CorePreparationReceiverMismatchProjection::Continuation { .. }
            | CorePreparationReceiverMismatchProjection::Execution { .. }
            | CorePreparationReceiverMismatchProjection::Applied { .. }
            | CorePreparationReceiverMismatchProjection::DriverFallback { .. }
            | CorePreparationReceiverMismatchProjection::DriverTerminal { .. } => {
                panic!("cleanup mismatch must use its tagged expected/observed projection")
            }
        }

        let exact_before_consume = exact.inner.testing_snapshot();
        assert!(matches!(
            exact.consume_back_target_driver_failure(outcome),
            BackTargetDriverFailureConsumeOutcome::FailureCleanup(_)
        ));
        assert_eq!(
            exact.inner.testing_snapshot(),
            exact_before_consume,
            "the returned whole cleanup owner must remain exact"
        );
    }

    #[test]
    fn back_target_driver_terminal_wrong_receiver_uses_tagged_expected_observed_projection() {
        let (mut exact, history, continuation, task) = take_back_target_driver_frame_task();
        exact.testing_exhaust_reservation_counter(TestingReservationCounter::Transaction);
        let outcome = match exact.report_back_target_failed(
            &history,
            continuation,
            task,
            ViewFailureKind::Build,
            "fallback reservation exhaustion must create one terminal owner",
            BackTargetDriverFailureReportMode::Normal,
        ) {
            BackTargetDriverFailureReportOutcome::Reported(outcome) => outcome,
            BackTargetDriverFailureReportOutcome::Mismatch { .. } => {
                panic!("the exact report receiver must match")
            }
        };
        assert!(matches!(
            &outcome,
            BackTargetDriverFailureCoreOutcome::Terminal(_)
        ));

        let mut foreign = confirmed_engine();
        let before = foreign.inner.testing_snapshot();
        let (outcome, mismatch) = match foreign.consume_back_target_driver_failure(outcome) {
            BackTargetDriverFailureConsumeOutcome::Mismatch { owner, mismatch } => {
                (owner, mismatch)
            }
            BackTargetDriverFailureConsumeOutcome::FailureCleanup(_)
            | BackTargetDriverFailureConsumeOutcome::FallbackReleased
            | BackTargetDriverFailureConsumeOutcome::Terminal(_) => {
                panic!("a foreign consumer must return the whole terminal owner")
            }
        };
        assert_eq!(
            foreign.inner.testing_snapshot(),
            before,
            "terminal receiver mismatch must be a semantic zero-write"
        );
        match mismatch.inner {
            CorePreparationReceiverMismatchProjection::DriverTerminal { expected, observed } => {
                assert!(matches!(
                    expected.status,
                    crate::engine::CoreBackTargetDriverTerminalExpectedStatus::Failed
                ));
                assert!(matches!(
                    observed.request,
                    crate::engine::CoreBackTargetDriverTerminalObservedRequestProjection::Unknown {
                        request,
                        ..
                    } if request == expected.request
                ));
            }
            CorePreparationReceiverMismatchProjection::Scope { .. }
            | CorePreparationReceiverMismatchProjection::LogicalZero { .. }
            | CorePreparationReceiverMismatchProjection::BackNoTargetReady(_)
            | CorePreparationReceiverMismatchProjection::BackNoTargetCompletion { .. }
            | CorePreparationReceiverMismatchProjection::BackNoTargetAuthorityBegin { .. }
            | CorePreparationReceiverMismatchProjection::BackNoTargetAuthorityCompletion {
                ..
            }
            | CorePreparationReceiverMismatchProjection::Continuation { .. }
            | CorePreparationReceiverMismatchProjection::Execution { .. }
            | CorePreparationReceiverMismatchProjection::Applied { .. }
            | CorePreparationReceiverMismatchProjection::DriverFailureCleanup { .. }
            | CorePreparationReceiverMismatchProjection::DriverFallback { .. } => {
                panic!("terminal mismatch must use its tagged expected/observed projection")
            }
        }

        let exact_before_consume = exact.inner.testing_snapshot();
        assert!(matches!(
            exact.consume_back_target_driver_failure(outcome),
            BackTargetDriverFailureConsumeOutcome::Terminal(_)
        ));
        assert_eq!(
            exact.inner.testing_snapshot(),
            exact_before_consume,
            "the returned whole terminal owner must remain exact"
        );
    }

    #[test]
    fn back_target_fallback_consume_rejects_same_engine_original_failure_drift() {
        let (mut engine, history, continuation, task) = take_back_target_driver_frame_task();
        let outcome = match engine.report_back_target_failed(
            &history,
            continuation,
            task,
            ViewFailureKind::Apply,
            "exact report must create one fallback attempt",
            BackTargetDriverFailureReportMode::Normal,
        ) {
            BackTargetDriverFailureReportOutcome::Reported(outcome) => outcome,
            BackTargetDriverFailureReportOutcome::Mismatch { .. } => {
                panic!("the exact report receiver must match")
            }
        };
        assert!(matches!(
            &outcome,
            BackTargetDriverFailureCoreOutcome::Fallback(_)
        ));
        assert!(
            engine.inner.testing_drift_current_fallback_failure(),
            "the seam must change only the current fallback attempt's original failure"
        );
        let before = engine.inner.testing_snapshot();

        let (outcome, mismatch) = match engine.consume_back_target_driver_failure(outcome) {
            BackTargetDriverFailureConsumeOutcome::Mismatch { owner, mismatch } => {
                (owner, mismatch)
            }
            BackTargetDriverFailureConsumeOutcome::FailureCleanup(_)
            | BackTargetDriverFailureConsumeOutcome::FallbackReleased
            | BackTargetDriverFailureConsumeOutcome::Terminal(_) => {
                panic!(
                    "a same-engine fallback whose original failure drifted must retain the whole owner"
                )
            }
        };
        assert_eq!(
            engine.inner.testing_snapshot(),
            before,
            "fallback attempt mismatch must be a semantic zero-write"
        );
        assert!(matches!(
            &outcome,
            BackTargetDriverFailureCoreOutcome::Fallback(_)
        ));
        match mismatch.inner {
            CorePreparationReceiverMismatchProjection::DriverFallback { expected, observed } => {
                assert_eq!(
                    expected.action, observed.action,
                    "the action identity is deliberately unchanged"
                );
                assert_ne!(
                    expected.attempt, observed.attempt,
                    "the evidence must retain the changed fallback attempt fact"
                );
                assert_eq!(
                    observed.stage,
                    crate::engine::CoreBackTargetReceiverStageProjection::Applying
                );
            }
            CorePreparationReceiverMismatchProjection::Scope { .. }
            | CorePreparationReceiverMismatchProjection::LogicalZero { .. }
            | CorePreparationReceiverMismatchProjection::BackNoTargetReady(_)
            | CorePreparationReceiverMismatchProjection::BackNoTargetCompletion { .. }
            | CorePreparationReceiverMismatchProjection::BackNoTargetAuthorityBegin { .. }
            | CorePreparationReceiverMismatchProjection::BackNoTargetAuthorityCompletion {
                ..
            }
            | CorePreparationReceiverMismatchProjection::Continuation { .. }
            | CorePreparationReceiverMismatchProjection::Execution { .. }
            | CorePreparationReceiverMismatchProjection::Applied { .. }
            | CorePreparationReceiverMismatchProjection::DriverFailureCleanup { .. }
            | CorePreparationReceiverMismatchProjection::DriverTerminal { .. } => {
                panic!("fallback attempt drift must use its tagged projection")
            }
        }
    }

    #[test]
    fn back_target_cleanup_consume_freezes_same_engine_pending_request_drift() {
        let (mut engine, history, continuation, task) = take_back_target_driver_frame_task();
        let outcome = match engine.report_back_target_failed(
            &history,
            continuation,
            task,
            ViewFailureKind::Apply,
            "physical residual must create one cleanup request",
            BackTargetDriverFailureReportMode::PhysicalResidual,
        ) {
            BackTargetDriverFailureReportOutcome::Reported(outcome) => outcome,
            BackTargetDriverFailureReportOutcome::Mismatch { .. } => {
                panic!("the exact report receiver must match")
            }
        };
        assert!(matches!(
            &outcome,
            BackTargetDriverFailureCoreOutcome::FailureCleanup(_)
        ));
        assert!(
            engine
                .inner
                .testing_drift_failure_cleanup_manifest_request(),
            "the seam must change only the pending cleanup stage's expected request"
        );
        let before = engine.inner.testing_snapshot();

        let (outcome, mismatch) = match engine.consume_back_target_driver_failure(outcome) {
            BackTargetDriverFailureConsumeOutcome::Mismatch { owner, mismatch } => {
                (owner, mismatch)
            }
            BackTargetDriverFailureConsumeOutcome::FailureCleanup(_)
            | BackTargetDriverFailureConsumeOutcome::FallbackReleased
            | BackTargetDriverFailureConsumeOutcome::Terminal(_) => {
                panic!("a drifted pending cleanup request must retain the whole owner")
            }
        };
        assert_eq!(
            engine.inner.testing_snapshot(),
            before,
            "cleanup request mismatch must be a semantic zero-write"
        );
        assert!(matches!(
            &outcome,
            BackTargetDriverFailureCoreOutcome::FailureCleanup(_)
        ));
        match mismatch.inner {
            CorePreparationReceiverMismatchProjection::DriverFailureCleanup {
                expected,
                observed,
            } => {
                assert_ne!(
                    observed.stage,
                    crate::engine::CoreBackTargetReceiverStageProjection::FailureCleanupManifestPending(
                        expected.request.receiver
                    ),
                    "the evidence must distinguish the owner request from the observed pending request"
                );
            }
            CorePreparationReceiverMismatchProjection::Scope { .. }
            | CorePreparationReceiverMismatchProjection::LogicalZero { .. }
            | CorePreparationReceiverMismatchProjection::BackNoTargetReady(_)
            | CorePreparationReceiverMismatchProjection::BackNoTargetCompletion { .. }
            | CorePreparationReceiverMismatchProjection::BackNoTargetAuthorityBegin { .. }
            | CorePreparationReceiverMismatchProjection::BackNoTargetAuthorityCompletion {
                ..
            }
            | CorePreparationReceiverMismatchProjection::Continuation { .. }
            | CorePreparationReceiverMismatchProjection::Execution { .. }
            | CorePreparationReceiverMismatchProjection::Applied { .. }
            | CorePreparationReceiverMismatchProjection::DriverFallback { .. }
            | CorePreparationReceiverMismatchProjection::DriverTerminal { .. } => {
                panic!("cleanup request drift must use its tagged projection")
            }
        }
    }

    #[test]
    fn back_target_terminal_consume_freezes_same_engine_retained_status_drift() {
        let (mut engine, history, continuation, task) = take_back_target_driver_frame_task();
        engine.testing_exhaust_reservation_counter(TestingReservationCounter::Transaction);
        let outcome = match engine.report_back_target_failed(
            &history,
            continuation,
            task,
            ViewFailureKind::Build,
            "fallback reservation exhaustion must create one terminal owner",
            BackTargetDriverFailureReportMode::Normal,
        ) {
            BackTargetDriverFailureReportOutcome::Reported(outcome) => outcome,
            BackTargetDriverFailureReportOutcome::Mismatch { .. } => {
                panic!("the exact report receiver must match")
            }
        };
        let request = match &outcome {
            BackTargetDriverFailureCoreOutcome::Terminal(owner) => owner.inner.request,
            BackTargetDriverFailureCoreOutcome::FailureCleanup(_)
            | BackTargetDriverFailureCoreOutcome::Fallback(_) => {
                panic!("the exhausted fallback reservation must produce Terminal")
            }
        };
        assert!(
            engine
                .inner
                .testing_drift_retained_terminal_request_status(request),
            "the seam must keep the terminal record retained and change only its observed status"
        );
        let before = engine.inner.testing_snapshot();

        let (outcome, mismatch) = match engine.consume_back_target_driver_failure(outcome) {
            BackTargetDriverFailureConsumeOutcome::Mismatch { owner, mismatch } => {
                (owner, mismatch)
            }
            BackTargetDriverFailureConsumeOutcome::FailureCleanup(_)
            | BackTargetDriverFailureConsumeOutcome::FallbackReleased
            | BackTargetDriverFailureConsumeOutcome::Terminal(_) => {
                panic!("a retained terminal record with status drift must return the whole owner")
            }
        };
        assert_eq!(
            engine.inner.testing_snapshot(),
            before,
            "terminal status mismatch must be a semantic zero-write"
        );
        assert!(matches!(
            &outcome,
            BackTargetDriverFailureCoreOutcome::Terminal(_)
        ));
        match mismatch.inner {
            CorePreparationReceiverMismatchProjection::DriverTerminal { expected, observed } => {
                assert_eq!(expected.request, request);
                assert!(matches!(
                    expected.status,
                    crate::engine::CoreBackTargetDriverTerminalExpectedStatus::Failed
                ));
                assert!(matches!(
                    observed.request,
                    crate::engine::CoreBackTargetDriverTerminalObservedRequestProjection::Retained {
                        request: observed_request,
                        status: RequestStatus::Queued,
                    } if observed_request == request
                ));
            }
            CorePreparationReceiverMismatchProjection::Scope { .. }
            | CorePreparationReceiverMismatchProjection::LogicalZero { .. }
            | CorePreparationReceiverMismatchProjection::BackNoTargetReady(_)
            | CorePreparationReceiverMismatchProjection::BackNoTargetCompletion { .. }
            | CorePreparationReceiverMismatchProjection::BackNoTargetAuthorityBegin { .. }
            | CorePreparationReceiverMismatchProjection::BackNoTargetAuthorityCompletion {
                ..
            }
            | CorePreparationReceiverMismatchProjection::Continuation { .. }
            | CorePreparationReceiverMismatchProjection::Execution { .. }
            | CorePreparationReceiverMismatchProjection::Applied { .. }
            | CorePreparationReceiverMismatchProjection::DriverFailureCleanup { .. }
            | CorePreparationReceiverMismatchProjection::DriverFallback { .. } => {
                panic!("terminal status drift must use its tagged projection")
            }
        }
    }

    #[test]
    fn back_target_mount_pairing_evidence_changes_with_same_request_token_fact() {
        let mut engine = nested_mount_back_fixture();
        let owner = take_verified_back_target(&mut engine);
        let resume = match engine.resume_back_target_with_retirement(owner, || {}) {
            BackTargetCoreResumeOutcome::Resumed(BackTargetStart::Ready(resume)) => resume,
            BackTargetCoreResumeOutcome::Resumed(BackTargetStart::FailureCleanup(_)) => {
                panic!("the live nested mount must resume with one MountRequest")
            }
            BackTargetCoreResumeOutcome::Permanent(_) => {
                panic!("the unchanged nested target must not become permanent")
            }
            BackTargetCoreResumeOutcome::Mismatch { .. } => {
                panic!("the exact nested target receiver must match")
            }
        };
        let (mut action, history, continuation) = resume.into_parts();
        let BackTargetResumeAction::MountRequest(request) = &mut action else {
            panic!("the nested fixture must return a MountRequest")
        };
        request.inner.testing_advance_token_binding();
        let before = engine.inner.testing_snapshot();

        let first = match engine.validate_back_target_execution_pairing(
            BackTargetExecutionOwnerRef::Driving {
                history: &history,
                continuation: &continuation,
            },
            BackTargetPhysicalOwnerRef::ResumeAction(&action),
        ) {
            BackTargetExecutionPairingOutcome::Mismatch(mismatch) => mismatch.inner,
            BackTargetExecutionPairingOutcome::Matched => {
                panic!("the first drifted mount token must fail the real receiver predicate")
            }
        };
        assert_eq!(
            engine.inner.testing_snapshot(),
            before,
            "mount-token pairing mismatch must be a semantic zero-write"
        );

        let BackTargetResumeAction::MountRequest(request) = &mut action else {
            unreachable!("the action variant was already established")
        };
        request.inner.testing_advance_token_binding();
        let second = match engine.validate_back_target_execution_pairing(
            BackTargetExecutionOwnerRef::Driving {
                history: &history,
                continuation: &continuation,
            },
            BackTargetPhysicalOwnerRef::ResumeAction(&action),
        ) {
            BackTargetExecutionPairingOutcome::Mismatch(mismatch) => mismatch.inner,
            BackTargetExecutionPairingOutcome::Matched => {
                panic!("the second drifted mount token must fail the real receiver predicate")
            }
        };
        assert_eq!(
            engine.inner.testing_snapshot(),
            before,
            "repeated mount-token inspection must remain zero-write"
        );
        assert_ne!(
            first, second,
            "exact mismatch evidence must change when the sole drifted mount-token fact changes"
        );
    }

    #[test]
    fn back_target_resume_action_follows_real_reconciliation() {
        let mut frame_task = confirmed_engine();
        settle_push(
            &mut frame_task,
            RouteLocation::new("/").with_fragment("frame-target"),
        );
        settle_changed_history_push(
            &mut frame_task,
            RouteLocation::new("/").with_fragment("frame-source"),
        );
        let owner = take_verified_back_target(&mut frame_task);
        let resume = match frame_task.resume_back_target_with_retirement(owner, || {}) {
            BackTargetCoreResumeOutcome::Resumed(BackTargetStart::Ready(resume)) => resume,
            BackTargetCoreResumeOutcome::Resumed(BackTargetStart::FailureCleanup(_)) => {
                panic!("a non-exhausted direct entering target must not enter failure cleanup")
            }
            BackTargetCoreResumeOutcome::Permanent(_) => {
                panic!("an unchanged nonzero-entering target must resume")
            }
            BackTargetCoreResumeOutcome::Mismatch { .. } => {
                panic!("the exact nonzero-entering target receiver must match")
            }
        };
        let (action, _execution, _continuation) = resume.into_parts();
        assert!(matches!(action, BackTargetResumeAction::FrameTask(_)));

        let mut commit = confirmed_nested_engine();
        let duplicate = RouteLocation::new("/");
        settle_push(&mut commit, duplicate.clone());
        settle_push(&mut commit, duplicate);
        let before = commit.inner.testing_next_frame_identities();
        let owner = take_verified_back_target(&mut commit);
        let resume = match commit.resume_back_target_with_retirement(owner, || {}) {
            BackTargetCoreResumeOutcome::Resumed(BackTargetStart::Ready(resume)) => resume,
            BackTargetCoreResumeOutcome::Resumed(BackTargetStart::FailureCleanup(_)) => {
                panic!("an unchanged zero-entering target must not enter failure cleanup")
            }
            BackTargetCoreResumeOutcome::Permanent(_) => {
                panic!("an unchanged zero-entering target must resume")
            }
            BackTargetCoreResumeOutcome::Mismatch { .. } => {
                panic!("the exact zero-entering target receiver must match")
            }
        };
        let after = commit.inner.testing_next_frame_identities();
        assert_eq!(after.0, before.0 + 2);
        assert_eq!(after.1, before.1 + 2);
        let (action, _execution, _continuation) = resume.into_parts();
        assert!(matches!(action, BackTargetResumeAction::Commit(_)));

        let mut pure_truncate = confirmed_back_target_nested_engine();
        settle_nested_history_command(
            &mut pure_truncate,
            HistoryFixtureCommand::Push,
            RouteLocation::new("/items"),
        );
        settle_nested_history_command(
            &mut pure_truncate,
            HistoryFixtureCommand::Push,
            RouteLocation::new("/items/1"),
        );
        let before = pure_truncate.inner.testing_next_frame_identities();
        let owner = take_verified_back_target(&mut pure_truncate);
        let resume = match pure_truncate.resume_back_target_with_retirement(owner, || {}) {
            BackTargetCoreResumeOutcome::Resumed(BackTargetStart::Ready(resume)) => resume,
            BackTargetCoreResumeOutcome::Resumed(BackTargetStart::FailureCleanup(_)) => {
                panic!("pure truncate must not enter failure cleanup")
            }
            BackTargetCoreResumeOutcome::Permanent(_) => {
                panic!("pure truncate of an unchanged target must resume")
            }
            BackTargetCoreResumeOutcome::Mismatch { .. } => {
                panic!("the exact pure-truncate target receiver must match")
            }
        };
        let after = pure_truncate.inner.testing_next_frame_identities();
        assert_eq!(after.0, before.0 + 3);
        assert_eq!(after.1, before.1 + 3);
        let (action, _execution, _continuation) = resume.into_parts();
        assert!(matches!(action, BackTargetResumeAction::Commit(_)));
    }

    #[test]
    fn back_target_nested_resume_uses_real_mount_request() {
        let mut engine = nested_mount_back_fixture();
        let owner = take_verified_back_target(&mut engine);
        let callback_calls = std::cell::Cell::new(0);

        let start = match engine.resume_back_target_with_retirement(owner, || {
            callback_calls.set(callback_calls.get() + 1);
        }) {
            BackTargetCoreResumeOutcome::Resumed(start) => start,
            BackTargetCoreResumeOutcome::Permanent(_) => {
                panic!("an unchanged nested Back target must resume")
            }
            BackTargetCoreResumeOutcome::Mismatch { .. } => {
                panic!("the exact nested Back target receiver must match")
            }
        };

        assert_eq!(callback_calls.get(), 1);
        let BackTargetStart::Ready(resume) = start else {
            panic!("a live nested mount is a normal Ready start")
        };
        let (action, _execution, _continuation) = resume.into_parts();
        assert!(matches!(action, BackTargetResumeAction::MountRequest(_)));
    }

    #[test]
    fn back_target_mount_exhaustion_is_failure_cleanup_not_resume_action() {
        let mut engine = nested_mount_back_fixture();
        let owner = take_verified_back_target(&mut engine);
        engine.inner.testing_exhaust_mount_sequence_counter();
        let ids_before = engine.inner.testing_id_allocator_snapshot();
        let mount_before = engine.inner.testing_mount_issuer_snapshot();
        let callback_calls = std::cell::Cell::new(0);

        let start = match engine.resume_back_target_with_retirement(owner, || {
            callback_calls.set(callback_calls.get() + 1);
        }) {
            BackTargetCoreResumeOutcome::Resumed(start) => start,
            BackTargetCoreResumeOutcome::Permanent(_) => {
                panic!("mount sequence exhaustion belongs to the failure-cleanup branch")
            }
            BackTargetCoreResumeOutcome::Mismatch { .. } => {
                panic!("the exact nested Back target receiver must match")
            }
        };

        assert_eq!(callback_calls.get(), 1);
        let BackTargetStart::FailureCleanup(failure) = start else {
            panic!("mount issue exhaustion must not be represented as a resume action")
        };
        assert_eq!(failure.failure().kind(), NavigationFailureKind::Mount);
        let request = failure.request();
        assert_eq!(engine.inner.testing_id_allocator_snapshot(), ids_before);
        assert_eq!(engine.inner.testing_mount_issuer_snapshot(), mount_before);
        assert!(
            !engine
                .inner
                .testing_history_capability_snapshot()
                .has_owner()
        );
        let record = engine
            .request(request)
            .expect("failure cleanup request remains retained");
        assert_eq!(
            record.status(),
            &RequestStatus::Running(NavigationPhase::CleanupPending)
        );
        assert!(
            record
                .trace()
                .iter()
                .all(|event| { event.kind() != NavigationTraceEventKind::MountRequested })
        );

        let cleanup = engine
            .prepare_failure_cleanup(failure, CleanupManifest::empty())
            .expect("mount issue exhaustion enters the existing cleanup owner chain");
        engine
            .complete_cleanup(cleanup.complete_empty())
            .expect("empty failure cleanup settles through the existing completion");
        assert_eq!(
            engine
                .request(request)
                .expect("recovering request remains retained")
                .status(),
            &RequestStatus::Running(NavigationPhase::Recovering)
        );
        let Some(EngineAction::FrameTask(fallback)) = engine.action() else {
            panic!("a mount issue without a bound error view must recover through fallback")
        };
        assert_eq!(fallback.request(), request);
        assert_eq!(fallback.view(), &RouteViewKey::new("fallback").unwrap());
        assert_eq!(fallback.frame().origin(), &FrameOrigin::RouterFallback);
    }

    #[test]
    fn back_target_mount_issue_exhaustion_recovers_to_nearest_bound_error_after_zero_cleanup() {
        let (mut engine, items_route, items_error_view) = nested_bound_mount_back_fixture();
        let owner = take_verified_back_target(&mut engine);
        engine.inner.testing_exhaust_mount_sequence_counter();
        let ids_before = engine.inner.testing_id_allocator_snapshot();
        let mount_before = engine.inner.testing_mount_issuer_snapshot();
        let active_before = engine.active().clone();
        let history_before = engine.snapshot().history().clone();
        let callback_calls = std::cell::Cell::new(0);

        let start = match engine.resume_back_target_with_retirement(owner, || {
            callback_calls.set(callback_calls.get() + 1);
        }) {
            BackTargetCoreResumeOutcome::Resumed(start) => start,
            BackTargetCoreResumeOutcome::Permanent(_) => {
                panic!("mount issue exhaustion must enter the failure ledger")
            }
            BackTargetCoreResumeOutcome::Mismatch { .. } => {
                panic!("the exact nested Back target receiver must match")
            }
        };
        assert_eq!(callback_calls.get(), 1);
        let BackTargetStart::FailureCleanup(failure) = start else {
            panic!("mount issue exhaustion must first return failure cleanup")
        };
        let request = failure.request();
        assert!(failure.logical_roots().is_empty());
        assert_eq!(failure.failure().kind(), NavigationFailureKind::Mount);
        let ids_after = engine.inner.testing_id_allocator_snapshot();
        assert_eq!(
            ids_after.next_transaction(),
            ids_before.next_transaction() + 1
        );
        assert_eq!(
            ids_after.next_frame_task(),
            ids_before.next_frame_task() + 1
        );
        assert_eq!(ids_after.next_frame(), ids_before.next_frame() + 1);
        assert_eq!(
            ids_after.next_cleanup_batch(),
            ids_before.next_cleanup_batch() + 1
        );
        assert_eq!(
            ids_after.next_mount_sequence(),
            ids_before.next_mount_sequence()
        );
        assert_eq!(engine.inner.testing_mount_issuer_snapshot(), mount_before);
        assert_eq!(engine.active(), &active_before);
        assert_eq!(engine.snapshot().history(), &history_before);
        assert!(engine.action().is_none());

        let cleanup = engine
            .prepare_failure_cleanup(failure, CleanupManifest::empty())
            .expect("zero-residual mount issue must bind the canonical cleanup ledger");
        assert_eq!(cleanup.obligation_count(), 0);
        assert!(engine.action().is_none());
        engine
            .complete_cleanup(cleanup.complete_empty())
            .expect("canonical zero cleanup must settle before recovery publication");

        let Some(EngineAction::FrameTask(recovery)) = engine.action() else {
            panic!("settled Back mount failure must issue the nearest items error view")
        };
        assert_eq!(recovery.request(), request);
        assert_eq!(
            recovery.frame().origin(),
            &FrameOrigin::RouteError(items_route)
        );
        assert_eq!(recovery.inner.identity().branch_index(), 1);
        assert_eq!(recovery.view(), &items_error_view);
        assert_eq!(engine.active(), &active_before);
        assert_eq!(engine.snapshot().history(), &history_before);
        assert_eq!(engine.inner.testing_mount_issuer_snapshot(), mount_before);
        let request_snapshot = engine.request(request).unwrap();
        let trace = request_snapshot.trace();
        let settled = trace
            .iter()
            .position(|event| event.kind() == NavigationTraceEventKind::CleanupSettled)
            .expect("cleanup settlement must be traced");
        let error_issued = trace
            .iter()
            .rposition(|event| {
                event.kind() == NavigationTraceEventKind::FrameTaskIssued
                    && event.frame().is_some_and(|frame| {
                        frame.origin() == &FrameOrigin::RouteError(items_route)
                    })
            })
            .expect("bound error issue must be traced");
        assert!(settled < error_issued);
    }

    #[test]
    fn back_typed_receivers_keep_branch_specific_owner_signatures() {
        let _: fn(
            &mut RouterEngine,
            VerifiedBackTargetExecutionOwner,
            fn(),
        ) -> BackTargetCoreResumeOutcome = RouterEngine::resume_back_target_with_retirement::<fn()>;
        let _: fn(
            &mut RouterEngine,
            BackTargetPermanentOwner,
        ) -> BackTargetPermanentSettlementOutcome =
            RouterEngine::settle_back_target_permanent_exact;
        let _: fn(
            &mut RouterEngine,
            VerifiedBackNoTargetCandidate,
            fn(),
        ) -> BackNoTargetSourceRefreezeOutcome =
            RouterEngine::refreeze_back_no_target_source_with_transfer::<fn()>;
        let _: fn(
            &mut RouterEngine,
            BackNoTargetSourceDriftOwner,
            fn(),
        ) -> BackNoTargetSourceDriftSettlementOutcome =
            RouterEngine::settle_back_no_target_source_drift_with_retirement::<fn()>;
        let _: fn(
            &mut RouterEngine,
            VerifiedBackResolutionContradiction,
            fn(),
        ) -> BackContradictionCoreOutcome =
            RouterEngine::fail_back_resolution_contradiction_with_retirement::<fn()>;
    }

    #[test]
    fn host_success_trace_is_ordered_and_uses_canonical_ready_state() {
        let (mut engine, grant) = ready_engine();
        let expected_version = grant.version().clone();
        let expected_slot = grant.top_slot();

        engine.record_top_slot_registered(&grant).unwrap();
        let before_replay = engine.snapshot();
        assert_eq!(
            engine.record_top_slot_registered(&grant),
            Err(IntegrationError::Duplicate)
        );
        assert_eq!(engine.snapshot(), before_replay);
        engine.confirm_host(grant);

        let host = engine.snapshot().host().clone();
        assert_eq!(host.version(), Some(&expected_version));
        assert_eq!(host.top_slot(), Some(expected_slot));
        assert_eq!(
            host.trace()
                .iter()
                .map(crate::HostTraceEvent::kind)
                .collect::<Vec<_>>(),
            vec![
                HostTraceEventKind::AdmissionRequested,
                HostTraceEventKind::HostGranted,
                HostTraceEventKind::TopSlotRegistered,
                HostTraceEventKind::HostReady,
            ]
        );
        assert!(host.trace().iter().all(|event| !matches!(
            event.kind(),
            HostTraceEventKind::SettlementStarted | HostTraceEventKind::SettlementCompleted
        )));
    }

    #[test]
    fn host_rejection_trace_records_only_successful_workflow_transitions() {
        let view = RouteViewKey::new("root").unwrap();
        let fallback = RouteViewKey::new("fallback").unwrap();
        let mut engine = RouterEngine::install(tree(&view), fallback).unwrap();

        let token = engine.request_host_admission().unwrap();
        engine.reject_host_admission(token).unwrap();
        let token = engine.request_host_admission().unwrap();
        let grant = engine.grant_host_admission(&token).unwrap();
        engine.reject_host_after_mutation(grant).unwrap();

        let host = engine.snapshot().host().clone();
        assert!(host.version().is_none());
        assert!(host.top_slot().is_none());
        assert_eq!(
            host.trace()
                .iter()
                .map(crate::HostTraceEvent::kind)
                .collect::<Vec<_>>(),
            vec![
                HostTraceEventKind::AdmissionRequested,
                HostTraceEventKind::AdmissionRejected,
                HostTraceEventKind::AdmissionRequested,
                HostTraceEventKind::HostGranted,
                HostTraceEventKind::AdmissionRejectedAfterMutation,
            ]
        );
    }

    #[test]
    fn request_lookup_bridge_preserves_found_not_retained_and_unknown() {
        let mut engine = confirmed_engine();
        let requests = (0..66)
            .map(|_| settle_push(&mut engine, RouteLocation::new("/")))
            .collect::<Vec<_>>();

        assert!(matches!(
            engine.request_lookup(requests[0]),
            RequestLookup::NotRetained
        ));
        assert!(matches!(
            engine.request_lookup(requests[2]),
            RequestLookup::Found(_)
        ));

        let mut foreign = confirmed_engine();
        let unknown = foreign.push(RouteLocation::new("/")).unwrap();
        assert!(matches!(
            engine.request_lookup(unknown),
            RequestLookup::Unknown
        ));
    }

    #[test]
    fn frozen_request_retention_classifier_rejects_foreign_and_future_ids() {
        let mut engine = confirmed_engine();
        let requests = (0..66)
            .map(|_| settle_push(&mut engine, RouteLocation::new("/")))
            .collect::<Vec<_>>();
        let snapshot = engine.snapshot();

        assert_eq!(
            snapshot.request_retention().classify(requests[0]),
            MissingRequestClassification::NotRetained
        );

        let mut foreign = confirmed_engine();
        let foreign_request = foreign.push(RouteLocation::new("/")).unwrap();
        assert_eq!(
            snapshot.request_retention().classify(foreign_request),
            MissingRequestClassification::Unknown
        );

        let future_request = engine.push(RouteLocation::new("/")).unwrap();
        assert_eq!(
            snapshot.request_retention().classify(future_request),
            MissingRequestClassification::Unknown
        );
    }

    #[test]
    fn router_projection_uses_only_current_request_then_clears_at_terminal() {
        let mut engine = confirmed_engine();
        let request = engine.push(RouteLocation::new("/")).unwrap();
        engine.collect();

        let running = engine.snapshot();
        assert_eq!(running.requested(), Some(&RouteLocation::new("/")));
        assert!(running.matched().is_some());
        assert_eq!(running.active().kind(), ActivePresentationKind::None);
        assert_eq!(running.availability(), &RouterAvailability::Ready);
        assert_eq!(
            running.pending_phase(),
            Some(&NavigationPhase::ApplyingFrame(0))
        );
        assert_eq!(running.running_request(), Some(request));
        assert_eq!(running.history().len(), 0);
        assert_eq!(running.earliest_retained_sequence(), None);
        assert!(running.diagnostics().events().is_empty());
        assert!(running.host().version().is_some());

        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("first push must issue a frame task")
        };
        let ticket = engine.report_applied(task).unwrap();
        let prepared = engine
            .prepare_commit(&ticket, CleanupManifest::empty())
            .unwrap();
        let cleanup = prepared.cleanup().clone();
        let validated = engine.validate_publication(prepared).unwrap();
        validated.publish(|| {});
        engine.complete_cleanup(cleanup.complete_empty()).unwrap();

        let terminal = engine.snapshot();
        assert!(terminal.requested().is_none());
        assert!(terminal.matched().is_none());
        assert!(terminal.pending_phase().is_none());
        assert!(terminal.running_request().is_none());
        assert_eq!(terminal.active().kind(), ActivePresentationKind::Route);
        assert_eq!(terminal.history().current(), Some(&RouteLocation::new("/")));
        assert_eq!(
            terminal.earliest_retained_sequence(),
            Some(request.sequence())
        );
    }

    #[test]
    fn cleanup_receipt_observation_precedes_aggregate_and_stale_is_zero_write() {
        let mut engine = confirmed_engine();
        settle_push(&mut engine, RouteLocation::new("/"));
        let leaving = engine.active().frames()[0].clone();

        let request = engine
            .push(RouteLocation::new("/").with_fragment("details"))
            .unwrap();
        engine.collect();
        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("fragment push must issue a frame task")
        };
        let ticket = engine.report_applied(task).unwrap();
        let prepared = engine
            .prepare_commit(&ticket, CleanupManifest::new(vec![leaving], 41, 1))
            .unwrap();
        let cleanup = prepared.cleanup().clone();
        let stale = cleanup.clone();
        let validated = engine.validate_publication(prepared).unwrap();
        validated.publish(|| {});

        engine
            .observe_cleanup_receipt(&cleanup, CleanupDisposition::Removed)
            .unwrap();
        let receipt_trace = engine.request(request).unwrap().trace().to_vec();
        assert_eq!(
            receipt_trace.last().map(NavigationTraceEvent::kind),
            Some(NavigationTraceEventKind::CleanupSettled)
        );
        assert_eq!(
            receipt_trace.last().unwrap().cleanup_disposition(),
            Some(CleanupDisposition::Removed)
        );
        assert_eq!(
            engine.request(request).unwrap().status(),
            &RequestStatus::Running(NavigationPhase::PostCommitCleanup)
        );

        engine.complete_cleanup(cleanup.complete(43, 1)).unwrap();
        let trace = engine.request(request).unwrap().trace().to_vec();
        let cleanup_events = trace
            .iter()
            .filter(|event| event.kind() == NavigationTraceEventKind::CleanupSettled)
            .map(NavigationTraceEvent::cleanup_disposition)
            .collect::<Vec<_>>();
        assert_eq!(
            cleanup_events,
            vec![Some(CleanupDisposition::Removed), None]
        );
        assert_eq!(
            trace.last().map(NavigationTraceEvent::kind),
            Some(NavigationTraceEventKind::RequestTerminal)
        );

        let before_snapshot = engine.snapshot();
        let before_trace = trace;
        assert_eq!(
            engine.observe_cleanup_receipt(&stale, CleanupDisposition::AlreadyAbsent),
            Err(IntegrationError::Stale)
        );
        assert_eq!(engine.snapshot(), before_snapshot);
        assert_eq!(engine.request(request).unwrap().trace(), before_trace);
    }

    #[test]
    fn cleanup_summary_projection_accepts_only_read_only_ledger_facts() {
        let summary = project_cleanup_summary(
            true,
            3,
            Some(CleanupDisposition::QuarantinedForeignDescendant),
        );

        assert!(summary.foreground_pending());
        assert_eq!(summary.quarantine_count(), 3);
        assert_eq!(
            summary.last_disposition(),
            Some(CleanupDisposition::QuarantinedForeignDescendant)
        );
    }

    #[test]
    fn sealed_integration_projects_host_and_root_task_without_raw_authority_mutation() {
        let (mut engine, grant) = ready_engine();
        let expected_host = grant.version().clone();
        let expected_slot = grant.top_slot();

        engine.confirm_host(grant);
        let request = engine.push(RouteLocation::new("/")).unwrap();
        engine.collect();
        let EngineAction::FrameTask(mut task) = engine.action().unwrap() else {
            panic!("first root push should produce one frame task")
        };

        assert_eq!(task.request(), request);
        assert_eq!(task.frame().host(), expected_host.host());
        assert_eq!(task.frame().slot(), expected_slot);
        assert_eq!(task.view(), &RouteViewKey::new("root").unwrap());
        assert_eq!(task.take_input().location(), &RouteLocation::new("/"));
        assert_eq!(
            engine.request(request).unwrap().status(),
            &RequestStatus::Running(NavigationPhase::ApplyingFrame(0))
        );
    }

    #[test]
    fn prepared_publication_runs_the_ecs_mutation_before_infallible_core_publish() {
        let (mut engine, grant) = ready_engine();
        engine.confirm_host(grant);
        let request = engine.push(RouteLocation::new("/")).unwrap();
        engine.collect();
        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("first root push should produce one frame task")
        };
        let ticket = engine.report_applied(task).unwrap();
        let prepared = engine
            .prepare_commit(&ticket, CleanupManifest::empty())
            .unwrap();
        let cleanup = prepared.cleanup().clone();
        let replay = cleanup.clone().complete_empty();
        let malformed = cleanup.clone().complete(7, 0);
        let mut mutation_ran = false;

        let validated = engine.validate_publication(prepared).unwrap();
        validated.publish(|| mutation_ran = true);

        assert!(mutation_ran);
        assert_eq!(engine.active().location(), Some(&RouteLocation::new("/")));
        assert_eq!(engine.history().len(), 1);
        assert_eq!(engine.history().current(), Some(&RouteLocation::new("/")));
        assert_eq!(
            engine.request(request).unwrap().status(),
            &RequestStatus::Running(NavigationPhase::PostCommitCleanup)
        );

        engine.complete_cleanup(cleanup.complete_empty()).unwrap();
        assert_eq!(
            engine.complete_cleanup(replay),
            Err(IntegrationError::Duplicate)
        );
        assert_eq!(
            engine.complete_cleanup(malformed),
            Err(IntegrationError::Invalid)
        );
        assert_eq!(
            engine.request(request).unwrap().status(),
            &RequestStatus::Succeeded
        );
    }

    #[test]
    fn prepared_replace_ticket_cannot_revive_fail_closed_integration_state() {
        let (mut engine, grant) = ready_engine();
        engine.record_top_slot_registered(&grant).unwrap();
        let lease = engine.confirm_host(grant);
        settle_push(&mut engine, RouteLocation::new("/"));
        let leaving = engine.active().frames()[0].clone();
        let request = engine
            .replace(RouteLocation::new("/").with_fragment("details"))
            .unwrap();
        engine.collect();
        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("fragment replace issues the real replacement task")
        };
        let ticket = engine.report_applied(task).unwrap();
        let prepared = engine
            .prepare_commit(
                &ticket,
                CleanupManifest::new(vec![leaving], 0x5354_414c_45, 1),
            )
            .expect("replace prepares before authority loss");

        match engine.fail_closed_authority_protocol_retaining(Some(lease), || {}) {
            Ok(()) => {}
            Err(_) => panic!("the live lease must enter the real fail-closed terminal"),
        }
        let terminal_snapshot = engine.snapshot();
        let terminal_status = engine.request(request).unwrap().status().clone();
        let terminal_trace = engine.request(request).unwrap().trace().to_vec();
        let terminal_owner = engine.inner.testing_history_capability_snapshot();
        assert!(!terminal_owner.has_owner());

        let mutation_ran = std::cell::Cell::new(false);
        assert!(matches!(
            engine.validate_publication(prepared),
            Err(IntegrationError::Stale)
        ));

        assert_eq!(
            (
                mutation_ran.get(),
                engine.snapshot(),
                engine.request(request).unwrap().status().clone(),
                engine.request(request).unwrap().trace().to_vec(),
                engine.inner.testing_history_capability_snapshot(),
            ),
            (
                false,
                terminal_snapshot,
                terminal_status,
                terminal_trace,
                terminal_owner,
            ),
            "the integration ticket is non-authorizing after terminal and cannot restore host/state/current/history"
        );
    }

    #[test]
    fn sealed_install_binding_is_used_for_engine_issued_router_fallback() {
        let (mut engine, grant) = ready_engine();
        engine.confirm_host(grant);
        let request = engine.push(RouteLocation::new("/")).unwrap();
        engine.collect();
        let EngineAction::FrameTask(normal) = engine.action().unwrap() else {
            panic!("normal root task expected")
        };

        let failure = engine
            .report_failed(normal, ViewFailureKind::Build, "normal build failed")
            .unwrap()
            .expect("zero-residual root failure must enter cleanup");
        assert!(failure.logical_roots().is_empty());
        assert!(engine.action().is_none());
        let cleanup = engine
            .prepare_failure_cleanup(failure, CleanupManifest::empty())
            .unwrap();
        engine.complete_cleanup(cleanup.complete_empty()).unwrap();

        let EngineAction::FrameTask(fallback) = engine.action().unwrap() else {
            panic!("router fallback task expected")
        };
        assert_eq!(fallback.request(), request);
        assert_eq!(fallback.view(), &RouteViewKey::new("fallback").unwrap());
        assert_eq!(fallback.frame().origin(), &FrameOrigin::RouterFallback);
        assert_eq!(
            fallback.input().failure().map(NavigationFailure::kind),
            Some(NavigationFailureKind::Build)
        );
    }

    #[test]
    fn physical_root_residual_requires_sealed_cleanup_before_fallback() {
        let (mut engine, grant) = ready_engine();
        engine.confirm_host(grant);
        let request = engine.push(RouteLocation::new("/")).unwrap();
        engine.collect();
        let EngineAction::FrameTask(normal) = engine.action().unwrap() else {
            panic!("normal root task expected")
        };

        let failure = engine
            .report_failed_with_physical_residual(
                normal,
                ViewFailureKind::Apply,
                "partial root apply failed",
            )
            .unwrap();
        assert!(failure.logical_roots().is_empty());
        assert!(engine.action().is_none());
        let cleanup = engine
            .prepare_failure_cleanup(failure, CleanupManifest::new(Vec::new(), 0x51_45_41_4c, 2))
            .unwrap();
        assert_eq!(cleanup.obligation_count(), 2);
        assert!(engine.action().is_none());
        assert_eq!(
            engine.request(request).unwrap().status(),
            &RequestStatus::Running(NavigationPhase::CleanupPending)
        );

        engine
            .complete_cleanup(cleanup.complete(0x43_4f_4d_50, 2))
            .unwrap();

        assert_eq!(
            engine.request(request).unwrap().status(),
            &RequestStatus::Running(NavigationPhase::Recovering)
        );
        assert!(matches!(engine.action(), Some(EngineAction::FrameTask(_))));
    }

    #[test]
    fn rejected_manifest_does_not_consume_the_single_commit_ticket() {
        let (mut engine, grant) = ready_engine();
        engine.confirm_host(grant);
        engine.push(RouteLocation::new("/")).unwrap();
        engine.collect();
        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("first root push should produce one frame task")
        };
        let non_cleanup_root = task.frame().clone();
        let ticket = engine.report_applied(task).unwrap();

        let rejected =
            engine.prepare_commit(&ticket, CleanupManifest::new(vec![non_cleanup_root], 17, 1));
        assert!(matches!(rejected, Err(IntegrationError::InvalidManifest)));

        let prepared = engine
            .prepare_commit(&ticket, CleanupManifest::empty())
            .unwrap();
        let validated = engine.validate_publication(prepared).unwrap();
        validated.publish(|| {});
        assert_eq!(engine.history().len(), 1);
    }

    #[test]
    fn queued_push_has_no_lifecycle_action_until_collect() {
        let (mut engine, grant) = ready_engine();
        engine.confirm_host(grant);

        engine.push(RouteLocation::new("/")).unwrap();
        assert!(engine.action().is_none());

        engine.collect();
        assert!(matches!(engine.action(), Some(EngineAction::FrameTask(_))));
    }
}
