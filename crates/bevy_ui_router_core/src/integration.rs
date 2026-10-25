mod exit;
pub use exit::{
    DeferredExitOwner, ExitCleanupBeginError, ExitCommitPreparationError, ExitReleaseDemand,
    PreparedExitCommit, ValidatedExitCommit,
};

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
    CanonicalHistoryView, EnqueueBackErrorKind, HistoryAdmissionStamp, HistoryAdmissionWorldSeal,
    HistoryEntryKey, HistoryFrameFact, HistoryPublicationFacts, PrepareBackAdmissionErrorKind,
    VerifiedBackNoTargetCandidate, VerifiedBackResolutionContradiction,
    VerifiedBackResolutionHandoff, VerifiedBackResolutionOwnerHandoff,
    VerifiedBackTargetExecutionOwner,
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
use crate::view_input::{RouteInputRef, RouteViewInput};
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

pub use crate::engine::idle_host_loss::{
    IdleHostLossErrorKind, IdleHostLossPhysicalDisposition, IdleHostLossPhysicalFacts,
    RetainedIdleHostLossPlan,
};

/// An unsuccessful idle begin returns the complete original transfer.
pub struct BeginRetainedIdleHostLossError {
    lease: HostLease,
    plan: RetainedIdleHostLossPlan,
    owners: Vec<RetainedLeafOwner>,
    kind: IdleHostLossErrorKind,
}

impl std::fmt::Debug for BeginRetainedIdleHostLossError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BeginRetainedIdleHostLossError")
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}

impl BeginRetainedIdleHostLossError {
    pub fn into_parts(
        self,
    ) -> (
        HostLease,
        RetainedIdleHostLossPlan,
        Vec<RetainedLeafOwner>,
        IdleHostLossErrorKind,
    ) {
        (self.lease, self.plan, self.owners, self.kind)
    }
}

#[doc(hidden)]
pub struct IdleHostLossOwner {
    inner: crate::engine::idle_host_loss::IdleHostLossOwner,
}

#[doc(hidden)]
pub struct IdleHostLossTerminal {
    inner: crate::engine::idle_host_loss::IdleHostLossTerminal,
}

impl IdleHostLossTerminal {
    pub fn matches_host(&self, version: &HostVersion, top_slot: RouteSlotId) -> bool {
        self.inner.grant.version() == version && self.inner.grant.top_slot() == top_slot
    }

    pub const fn physical_facts(&self) -> IdleHostLossPhysicalFacts {
        self.inner.physical
    }
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
    Consumed,
}

#[doc(hidden)]
#[must_use = "a failed mount resolution returns or consumes its request capability"]
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

impl<'engine> MountRegistrationSession<'engine> {
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

    /// Rejects this original permit while preserving the selected-instance continuation.
    #[doc(hidden)]
    pub fn reject_retained_invalidation(
        self,
        invalid: RouteFrameVersion,
    ) -> Result<FailureCleanupRequest, MountRegistrationSessionRejectionError<'engine>> {
        let Self {
            engine,
            permit,
            slot,
        } = self;
        match engine.reject_retained_mount_after_mutation(
            CoreMountMutationArtifact::RegistrationPermit(permit),
            invalid,
        ) {
            Ok(inner) => Ok(FailureCleanupRequest { inner }),
            Err(error) => {
                let kind = map_mount_error(error.kind());
                let CoreMountMutationArtifact::RegistrationPermit(permit) = error.into_owner()
                else {
                    unreachable!("rejection returns the original registration permit")
                };
                Err(MountRegistrationSessionRejectionError {
                    kind,
                    session: Self {
                        engine,
                        permit,
                        slot,
                    },
                })
            }
        }
    }

    pub fn reject(self) -> FailureCleanupRequest {
        FailureCleanupRequest {
            inner: self
                .engine
                .reject_preflighted_mount_registration(self.permit),
        }
    }
}

#[must_use = "a rejected selected-instance continuation returns the original registration session"]
pub struct MountRegistrationSessionRejectionError<'engine> {
    kind: IntegrationError,
    session: MountRegistrationSession<'engine>,
}
impl<'engine> MountRegistrationSessionRejectionError<'engine> {
    pub fn kind(&self) -> IntegrationError {
        self.kind
    }
    pub fn into_session(self) -> MountRegistrationSession<'engine> {
        self.session
    }
}
impl std::fmt::Debug for MountRegistrationSessionRejectionError<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MountRegistrationSessionRejectionError")
            .field("kind", &self.kind)
            .finish_non_exhaustive()
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
    pub fn parent_source(&self) -> Option<TargetInstanceSourceKind> {
        self.inner.parent_source()
    }
    pub fn instance_selection_pending(&self) -> bool {
        self.inner.instance_selection_pending()
    }

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
#[derive(Debug, Eq, PartialEq)]
pub enum FailureCleanupCompletionDisposition {
    ResumeInstanceInvalidation(RetainedInvalidationContinuation),
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
    pub fn request(&self) -> NavigationRequestId {
        self.inner.identity().request()
    }

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

    pub fn retained_logical_roots(&self) -> Option<&[RouteFrameVersion]> {
        self.inner.retained_logical_roots()
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

pub use crate::engine::retained::{
    InstanceSelectionPlan, RetainedAuthoritySettlementPlan, RetainedCommitPlan,
    RetainedInvalidationPlan, RetainedLeafOwner, RetainedLeafProjection,
    RetainedPostCommitAuthorityPlan, TargetInstanceStepProjection,
};

/// Rejected authority transfer returns the unchanged host and every retained owner.
#[must_use = "rejected authority transfer retains all original lifecycle responsibility"]
pub struct RetainedAuthoritySettlementError {
    lease: HostLease,
    plan: RetainedAuthoritySettlementPlan,
    manifest: CleanupManifest,
    owners: Vec<RetainedLeafOwner>,
    kind: BeginAuthoritySettlementErrorKind,
}

impl std::fmt::Debug for RetainedAuthoritySettlementError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RetainedAuthoritySettlementError")
            .field("plan", &self.plan)
            .field("manifest", &self.manifest.projection())
            .field("owners", &self.owners)
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}

impl RetainedAuthoritySettlementError {
    pub fn into_parts(
        self,
    ) -> (
        HostLease,
        RetainedAuthoritySettlementPlan,
        CleanupManifest,
        Vec<RetainedLeafOwner>,
        BeginAuthoritySettlementErrorKind,
    ) {
        (self.lease, self.plan, self.manifest, self.owners, self.kind)
    }
}

pub use crate::engine::instances::{RetainedOwnSlotProjection, TargetInstanceSourceKind};

impl InstanceSelectionPlan {
    pub fn reconciliation(&self) -> ReconciliationProjectionView<'_> {
        ReconciliationProjectionView {
            inner: self.reconciliation_projection(),
        }
    }
}

/// Unique cache-only cleanup continuation holding the original fresh task.
pub struct RetainedMissCleanupOwner {
    inner: crate::engine::retained::RetainedMissCleanupOwner,
    cleanup: CleanupBatch,
}

pub use crate::engine::retained::RetainedMissCleanupAuthorityFailCloseErrorKind;

/// Rejection preserves the installed cache token, every original owner, and
/// the original host lease for the trusted physical receiver.
#[must_use = "rejected authority termination retains the installed cache owner"]
pub struct RetainedMissCleanupAuthorityFailCloseError {
    owner: RetainedMissCleanupOwner,
    lease: HostLease,
    kind: RetainedMissCleanupAuthorityFailCloseErrorKind,
}

impl RetainedMissCleanupAuthorityFailCloseError {
    pub fn kind(&self) -> RetainedMissCleanupAuthorityFailCloseErrorKind {
        self.kind
    }

    pub fn into_parts(
        self,
    ) -> (
        RetainedMissCleanupOwner,
        HostLease,
        RetainedMissCleanupAuthorityFailCloseErrorKind,
    ) {
        (self.owner, self.lease, self.kind)
    }
}

impl std::fmt::Debug for RetainedMissCleanupAuthorityFailCloseError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RetainedMissCleanupAuthorityFailCloseError")
            .field("owner", &self.owner)
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}

impl RetainedMissCleanupOwner {
    pub fn cleanup(&self) -> &CleanupBatch {
        &self.cleanup
    }
}

impl std::fmt::Debug for RetainedMissCleanupOwner {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RetainedMissCleanupOwner")
            .field("inner", &self.inner)
            .finish()
    }
}

/// A rejected completion keeps the sole task continuation and physical owner.
#[must_use = "rejected cache cleanup completion retains its original owner"]
pub struct RetainedMissCleanupCompletionError<Physical> {
    owner: RetainedMissCleanupOwner,
    completion: CleanupCompletion,
    physical: Physical,
    error: IntegrationError,
}

impl<Physical> RetainedMissCleanupCompletionError<Physical> {
    pub fn into_parts(
        self,
    ) -> (
        RetainedMissCleanupOwner,
        CleanupCompletion,
        Physical,
        IntegrationError,
    ) {
        (self.owner, self.completion, self.physical, self.error)
    }
}

impl<Physical> std::fmt::Debug for RetainedMissCleanupCompletionError<Physical> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RetainedMissCleanupCompletionError")
            .field("owner", &self.owner)
            .field("error", &self.error)
            .finish()
    }
}

/// A publication rejection preserves the original Back receiver evidence.
#[derive(Debug, Eq, PartialEq)]
pub enum RetainedPublicationError {
    Integration(IntegrationError),
    BackTargetMismatch(PreparationReceiverMismatchView),
}

/// The original mount failure's capability, returned only after its physical obligations settle.
#[derive(Debug, Eq, PartialEq)]
pub struct RetainedInvalidationContinuation {
    inner: crate::engine::retained::RetainedInvalidationContinuation,
}
impl RetainedInvalidationContinuation {
    pub fn invalid(&self) -> &RouteFrameVersion {
        self.inner.invalid()
    }
}
pub use crate::engine::retained::RetainedInvalidationResumeFailure;
#[must_use]
pub struct RetainedInvalidationResumeError {
    continuation: RetainedInvalidationContinuation,
    failure: RetainedInvalidationResumeFailure,
}
impl RetainedInvalidationResumeError {
    pub fn failure(&self) -> RetainedInvalidationResumeFailure {
        self.failure
    }
    pub fn into_parts(
        self,
    ) -> (
        RetainedInvalidationContinuation,
        RetainedInvalidationResumeFailure,
    ) {
        (self.continuation, self.failure)
    }
}
impl std::fmt::Debug for RetainedInvalidationResumeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RetainedInvalidationResumeError")
            .field("continuation", &self.continuation)
            .field("failure", &self.failure)
            .finish()
    }
}
#[must_use]
pub struct RetainedInvalidationAuthoritySettlementError {
    continuation: RetainedInvalidationContinuation,
    settlement: RetainedAuthoritySettlementError,
}
impl RetainedInvalidationAuthoritySettlementError {
    pub fn into_parts(
        self,
    ) -> (
        RetainedInvalidationContinuation,
        RetainedAuthoritySettlementError,
    ) {
        (self.continuation, self.settlement)
    }
}
impl std::fmt::Debug for RetainedInvalidationAuthoritySettlementError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RetainedInvalidationAuthoritySettlementError")
            .field("continuation", &self.continuation)
            .field("settlement", &self.settlement)
            .finish()
    }
}
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetainedInvalidationAuthorityFailCloseErrorKind {
    SourceMismatch,
    ProtocolRejected,
}
#[doc(hidden)]
#[must_use]
pub struct RetainedInvalidationAuthorityFailCloseError {
    continuation: RetainedInvalidationContinuation,
    lease: Option<HostLease>,
    kind: RetainedInvalidationAuthorityFailCloseErrorKind,
}
impl RetainedInvalidationAuthorityFailCloseError {
    pub fn kind(&self) -> RetainedInvalidationAuthorityFailCloseErrorKind {
        self.kind
    }
    pub fn into_parts(
        self,
    ) -> (
        RetainedInvalidationContinuation,
        Option<HostLease>,
        RetainedInvalidationAuthorityFailCloseErrorKind,
    ) {
        (self.continuation, self.lease, self.kind)
    }
}
impl std::fmt::Debug for RetainedInvalidationAuthorityFailCloseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RetainedInvalidationAuthorityFailCloseError")
            .field("continuation", &self.continuation)
            .field("has_lease", &self.lease.is_some())
            .field("kind", &self.kind)
            .finish()
    }
}
pub enum RetainedInvalidationReceiver {
    FrameTask(FrameTask),
    Commit(CommitTicket),
    AfterFailure(RetainedInvalidationContinuation),
}
impl RetainedInvalidationReceiver {
    fn into_inner(self) -> crate::engine::retained::CoreRetainedInvalidationReceiver {
        match self {
            Self::FrameTask(task) => {
                crate::engine::retained::CoreRetainedInvalidationReceiver::FrameTask(task.inner)
            }
            Self::Commit(ticket) => {
                crate::engine::retained::CoreRetainedInvalidationReceiver::Commit(ticket.inner)
            }
            Self::AfterFailure(owner) => {
                crate::engine::retained::CoreRetainedInvalidationReceiver::AfterFailure(owner.inner)
            }
        }
    }
    fn from_inner(inner: crate::engine::retained::CoreRetainedInvalidationReceiver) -> Self {
        match inner {
            crate::engine::retained::CoreRetainedInvalidationReceiver::FrameTask(inner) => {
                Self::FrameTask(FrameTask { inner })
            }
            crate::engine::retained::CoreRetainedInvalidationReceiver::Commit(inner) => {
                Self::Commit(CommitTicket { inner })
            }
            crate::engine::retained::CoreRetainedInvalidationReceiver::AfterFailure(inner) => {
                Self::AfterFailure(RetainedInvalidationContinuation { inner })
            }
        }
    }
}
impl std::fmt::Debug for RetainedInvalidationReceiver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FrameTask(task) => f
                .debug_tuple("FrameTask")
                .field(task.inner.identity())
                .finish(),
            Self::Commit(ticket) => f
                .debug_tuple("Commit")
                .field(ticket.inner.identity())
                .finish(),
            Self::AfterFailure(owner) => f.debug_tuple("AfterFailure").field(owner).finish(),
        }
    }
}

#[must_use]
pub struct RetainedInvalidationReceiverError {
    receiver: RetainedInvalidationReceiver,
    error: IntegrationError,
}
impl RetainedInvalidationReceiverError {
    pub fn into_parts(self) -> (RetainedInvalidationReceiver, IntegrationError) {
        (self.receiver, self.error)
    }
}
impl std::fmt::Debug for RetainedInvalidationReceiverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RetainedInvalidationReceiverError")
            .field("receiver", &self.receiver)
            .field("error", &self.error)
            .finish()
    }
}

#[must_use]
pub struct InstanceSelectionPreparationError {
    task: FrameTask,
    plan: InstanceSelectionPlan,
    error: IntegrationError,
}
impl std::fmt::Debug for InstanceSelectionPreparationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InstanceSelectionPreparationError")
            .field("plan", &self.plan)
            .field("error", &self.error)
            .finish()
    }
}
impl InstanceSelectionPreparationError {
    pub fn into_parts(self) -> (FrameTask, InstanceSelectionPlan, IntegrationError) {
        (self.task, self.plan, self.error)
    }
}

/// A rejected hit returns the original task and every original cache owner.
pub struct RetainedInvalidationPreparationError {
    receiver: RetainedInvalidationReceiver,
    plan: RetainedInvalidationPlan,
    manifest: CleanupManifest,
    owners: Vec<RetainedLeafOwner>,
    error: RetainedPublicationError,
}

impl std::fmt::Debug for RetainedInvalidationPreparationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RetainedInvalidationPreparationError")
            .field("receiver", &self.receiver)
            .field("plan", &self.plan)
            .field("manifest", &self.manifest.projection())
            .field("owners", &self.owners)
            .field("error", &self.error)
            .finish()
    }
}

impl RetainedInvalidationPreparationError {
    pub fn into_parts(
        self,
    ) -> (
        RetainedInvalidationReceiver,
        RetainedInvalidationPlan,
        CleanupManifest,
        Vec<RetainedLeafOwner>,
        RetainedPublicationError,
    ) {
        (
            self.receiver,
            self.plan,
            self.manifest,
            self.owners,
            self.error,
        )
    }
}

/// Preparation rejection returns the complete original physical responsibility.
pub struct RetainedCommitPreparationError {
    plan: RetainedCommitPlan,
    manifest: CleanupManifest,
    owners: Vec<RetainedLeafOwner>,
    error: RetainedPublicationError,
}

impl std::fmt::Debug for RetainedCommitPreparationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RetainedCommitPreparationError")
            .field("plan", &self.plan)
            .field("manifest", &self.manifest.projection())
            .field("owners", &self.owners)
            .field("error", &self.error)
            .finish()
    }
}

impl RetainedCommitPreparationError {
    pub fn into_parts(
        self,
    ) -> (
        RetainedCommitPlan,
        CleanupManifest,
        Vec<RetainedLeafOwner>,
        RetainedPublicationError,
    ) {
        (self.plan, self.manifest, self.owners, self.error)
    }
}

/// A prepared handle. The unique transit owners remain held by the issuing Core.
pub struct PreparedRetainedCommit {
    publication: PreparedPublication,
    cleanup: CleanupBatch,
    retained: Vec<RetainedLeafProjection>,
    back_target: Option<(
        BackResolutionScopeProjection,
        HistoryExecutionBindingProjection,
    )>,
}

impl std::fmt::Debug for PreparedRetainedCommit {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PreparedRetainedCommit")
            .field("history", self.publication.history_publication())
            .field("retained", &self.retained)
            .field("manifest_digest", &self.cleanup.manifest_digest)
            .field("obligation_count", &self.cleanup.obligation_count)
            .finish()
    }
}

impl PreparedRetainedCommit {
    pub fn history_publication(&self) -> &HistoryPublicationFacts {
        self.publication.history_publication()
    }
    pub fn cleanup(&self) -> &CleanupBatch {
        &self.cleanup
    }
    pub fn retained(&self) -> &[RetainedLeafProjection] {
        &self.retained
    }
}

pub struct ValidatedRetainedCommit<'a> {
    publication: crate::engine::retained::ValidatedRetainedPublication<'a>,
    cleanup: CleanupBatch,
    back_target_continuation: Option<LifecycleContinuationCapacity>,
}

impl ValidatedRetainedCommit<'_> {
    pub fn history_publication(&self) -> &HistoryPublicationFacts {
        self.publication.history_publication()
    }
    pub fn cleanup(&self) -> &CleanupBatch {
        &self.cleanup
    }
    pub fn publish<F>(self, mutation: F)
    where
        F: FnOnce(Vec<RetainedLeafOwner>),
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

pub struct PreparedCommit {
    publication: PreparedPublication,
    cleanup: CleanupBatch,
}

impl PreparedCommit {
    pub fn history_publication(&self) -> &HistoryPublicationFacts {
        self.publication.history_publication()
    }

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
    pub fn history_publication(&self) -> &HistoryPublicationFacts {
        self.publication.history_publication()
    }

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
#[derive(Debug, Eq, PartialEq)]
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

/// Completes the Back lifecycle owner after the ordinary mount failure algorithm.
#[doc(hidden)]
pub enum FinishBackTargetMountFailureOutcome {
    Matched(FailureCleanupRequest),
    Mismatch {
        capacity: LifecycleContinuationCapacity,
        request: FailureCleanupRequest,
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

    pub fn into_mount_request_parts(
        self,
    ) -> Result<
        (
            MountRequest,
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
            BackTargetResumeAction::MountRequest(request) => Ok((request, execution, continuation)),
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
    pub fn retained(&self) -> impl ExactSizeIterator<Item = &'a RetainedLeafProjection> {
        self.inner.retained().iter()
    }
    pub fn source_at(&self, index: usize) -> Option<TargetInstanceSourceKind> {
        self.inner.source_at(index)
    }
    pub fn instances_selected(&self) -> bool {
        self.inner.instances_selected()
    }

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
    pub fn kind(&self) -> crate::request::NavigationKind {
        self.record.kind()
    }

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

const HOST_TRACE_RETENTION: usize = 64;

#[derive(Default)]
struct HostTraceLog {
    events: Vec<HostTraceEvent>,
}

impl HostTraceLog {
    fn len(&self) -> usize {
        self.events.len()
    }

    fn capacity(&self) -> usize {
        self.events.capacity()
    }

    fn try_reserve(&mut self, additional: usize) -> Result<(), std::collections::TryReserveError> {
        let needed = additional.min(HOST_TRACE_RETENTION.saturating_sub(self.events.len()));
        self.events.try_reserve(needed)
    }

    fn push(&mut self, event: HostTraceEvent) {
        if self.events.len() == HOST_TRACE_RETENTION {
            self.events.remove(0);
        }
        self.events.push(event);
    }
}

pub struct RouterEngine {
    inner: CoreRouterEngine,
    admission: HostAdmissionIssuer,
    host_trace: HostTraceLog,
    // Registration deduplication is exact grant evidence, independent of log retention.
    registered_top_slot: Option<HostTraceEvent>,
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
    /// Maximum inactive normal instances. Zero disables keep-alive.
    pub fn with_keep_alive_capacity(mut self, capacity: usize) -> Self {
        self.inner = self.inner.with_keep_alive_capacity(capacity);
        self
    }

    pub const fn keep_alive_capacity(&self) -> usize {
        self.inner.keep_alive_capacity()
    }

    #[doc(hidden)]
    pub fn idle_host_loss_eligible(&self) -> bool {
        self.inner.idle_host_loss_eligible()
    }

    #[doc(hidden)]
    pub fn begin_idle_host_loss<F>(
        &mut self,
        lease: HostLease,
        manifest: &CleanupManifest,
        install: F,
    ) -> Result<(), (IdleHostLossErrorKind, HostLease)>
    where
        F: FnOnce(IdleHostLossOwner),
    {
        self.inner
            .begin_idle_host_loss(
                &mut self.admission,
                lease.inner,
                &manifest.logical_roots,
                manifest.manifest_digest,
                manifest.obligation_count,
                |inner| install(IdleHostLossOwner { inner }),
            )
            .map_err(|(kind, inner)| (kind, HostLease { inner }))
    }

    #[doc(hidden)]
    pub fn prepare_retained_idle_host_loss_plan(
        &self,
        lease: &HostLease,
        inventory: &[&RetainedLeafOwner],
    ) -> Result<RetainedIdleHostLossPlan, IdleHostLossErrorKind> {
        self.inner
            .prepare_retained_idle_host_loss_plan(&lease.inner, inventory)
    }

    #[doc(hidden)]
    pub fn begin_idle_host_loss_with_retention<F>(
        &mut self,
        lease: HostLease,
        plan: RetainedIdleHostLossPlan,
        manifest: &CleanupManifest,
        owners: Vec<RetainedLeafOwner>,
        install: F,
    ) -> Result<(), BeginRetainedIdleHostLossError>
    where
        F: FnOnce(IdleHostLossOwner),
    {
        self.inner
            .begin_idle_host_loss_with_retention(
                &mut self.admission,
                lease.inner,
                plan,
                &manifest.logical_roots,
                manifest.manifest_digest,
                manifest.obligation_count,
                owners,
                |inner| install(IdleHostLossOwner { inner }),
            )
            .map_err(
                |(lease, plan, owners, kind)| BeginRetainedIdleHostLossError {
                    lease: HostLease { inner: lease },
                    plan,
                    owners,
                    kind,
                },
            )
    }

    #[doc(hidden)]
    pub fn complete_idle_host_loss<F, T>(
        &mut self,
        owner: IdleHostLossOwner,
        facts: IdleHostLossPhysicalFacts,
        install: F,
    ) -> Result<T, IdleHostLossOwner>
    where
        F: FnOnce(IdleHostLossTerminal) -> T,
    {
        self.inner
            .complete_idle_host_loss(owner.inner, facts, |inner| {
                install(IdleHostLossTerminal { inner })
            })
            .map_err(|inner| IdleHostLossOwner { inner })
    }

    pub fn install(tree: RouteTree, fallback: RouteViewKey) -> Result<Self, IntegrationError> {
        let inner = CoreRouterEngine::install(tree, fallback).map_err(map_install_error)?;
        let admission = HostAdmissionIssuer::new().map_err(|_| IntegrationError::Exhausted)?;
        Ok(Self {
            inner,
            admission,
            host_trace: HostTraceLog::default(),
            registered_top_slot: None,
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
    pub fn prepare_retained_post_commit_authority_plan(
        &self,
        source: &PostCommitAuthorityPreparationSource,
        inventory: &[&RetainedLeafOwner],
    ) -> Result<RetainedPostCommitAuthorityPlan, IntegrationError> {
        self.inner
            .prepare_retained_post_commit_authority_plan(&source.inner, inventory)
            .map_err(map_commit_error)
    }

    #[doc(hidden)]
    pub fn prepare_post_commit_authority_transfer_with_retention(
        &mut self,
        mut source: PostCommitAuthorityPreparationSource,
        plan: RetainedPostCommitAuthorityPlan,
        manifest: CleanupManifest,
        owners: Vec<RetainedLeafOwner>,
    ) -> PostCommitAuthorityPreparationOutcome {
        source.inner = source.inner.with_retention(plan, owners);
        self.prepare_post_commit_authority_transfer(source, manifest)
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
        if self
            .registered_top_slot
            .as_ref()
            .is_some_and(|event| event.version() == Some(version) && event.slot() == Some(slot))
        {
            return Err(IntegrationError::Duplicate);
        }
        let event = HostTraceEvent::project(
            HostTraceEventKind::TopSlotRegistered,
            Some(version.clone()),
            Some(slot),
        );
        self.registered_top_slot = Some(event.clone());
        self.host_trace.push(event);
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
        let observed_trace = self
            .registered_top_slot
            .as_ref()
            .filter(|event| event.version() == Some(version) && event.slot() == Some(slot));
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
        self.registered_top_slot = Some(expected_trace.clone());
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

    /// Covers an unstarted, unbound fallback and the exact committed active
    /// presentation. Only a HostLost settlement may consume this source.
    #[doc(hidden)]
    pub fn host_loss_trigger_for_unstarted_fallback_with_active(
        &mut self,
        task: &FrameTask,
    ) -> Result<FailedAuthoritySettlementTrigger, FailedAuthorityTriggerError> {
        self.inner
            .host_loss_trigger_for_unstarted_fallback_with_active(task.inner.identity())
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
    pub fn prepare_retained_authority_settlement_plan(
        &self,
        lease: &HostLease,
        inventory: &[&RetainedLeafOwner],
    ) -> Result<RetainedAuthoritySettlementPlan, BeginAuthoritySettlementErrorKind> {
        self.inner
            .prepare_retained_authority_settlement_plan(&lease.inner, inventory)
            .map_err(map_begin_authority_settlement_error)
    }

    /// Borrows the current failed trigger after physical preparation. The plan
    /// freezes its exact revision and preserves the failed source's base roots.
    #[doc(hidden)]
    pub fn prepare_retained_failed_authority_settlement_plan(
        &self,
        lease: &HostLease,
        trigger: &FailedAuthoritySettlementTrigger,
        base_logical_roots: &[RouteFrameVersion],
        inventory: &[&RetainedLeafOwner],
    ) -> Result<RetainedAuthoritySettlementPlan, BeginAuthoritySettlementErrorKind> {
        self.inner
            .prepare_retained_failed_authority_settlement_plan(
                &lease.inner,
                &trigger.inner,
                base_logical_roots,
                inventory,
            )
            .map_err(map_begin_authority_settlement_error)
    }

    #[doc(hidden)]
    pub fn begin_authority_settlement_with_retention<F>(
        &mut self,
        lease: HostLease,
        cause: PreCommitAuthorityCause,
        plan: RetainedAuthoritySettlementPlan,
        manifest: CleanupManifest,
        owners: Vec<RetainedLeafOwner>,
        authority_mutation: F,
    ) -> Result<CleanupBatch, RetainedAuthoritySettlementError>
    where
        F: FnOnce(CleanupBatch),
    {
        self.begin_authority_settlement_with_retained_trigger(
            lease,
            None,
            cause,
            plan,
            manifest,
            owners,
            authority_mutation,
        )
    }

    /// Consumes one failed-trigger attempt; rejection returns every long-lived
    /// owner while the caller retains its original failed task or cleanup request.
    #[doc(hidden)]
    pub fn begin_failed_authority_settlement_with_retention<F>(
        &mut self,
        lease: HostLease,
        trigger: FailedAuthoritySettlementTrigger,
        cause: PreCommitAuthorityCause,
        plan: RetainedAuthoritySettlementPlan,
        manifest: CleanupManifest,
        owners: Vec<RetainedLeafOwner>,
        authority_mutation: F,
    ) -> Result<CleanupBatch, RetainedAuthoritySettlementError>
    where
        F: FnOnce(CleanupBatch),
    {
        self.begin_authority_settlement_with_retained_trigger(
            lease,
            Some(trigger),
            cause,
            plan,
            manifest,
            owners,
            authority_mutation,
        )
    }

    fn begin_authority_settlement_with_retained_trigger<F>(
        &mut self,
        lease: HostLease,
        trigger: Option<FailedAuthoritySettlementTrigger>,
        cause: PreCommitAuthorityCause,
        plan: RetainedAuthoritySettlementPlan,
        manifest: CleanupManifest,
        owners: Vec<RetainedLeafOwner>,
        authority_mutation: F,
    ) -> Result<CleanupBatch, RetainedAuthoritySettlementError>
    where
        F: FnOnce(CleanupBatch),
    {
        if self.host_trace.try_reserve(2).is_err() {
            return Err(RetainedAuthoritySettlementError {
                lease,
                plan,
                manifest,
                owners,
                kind: BeginAuthoritySettlementErrorKind::RetryablePreparation,
            });
        }
        let version = self.inner.host().cloned();
        let slot = self.inner.top_slot();
        let manifest_digest = manifest.manifest_digest;
        let obligation_count = manifest.obligation_count;
        let seal = CleanupManifestSeal::new(
            manifest.logical_roots.clone(),
            CleanupManifestDigest::from_opaque(manifest_digest),
            obligation_count,
        );
        let host_trace = &mut self.host_trace;
        let publication = |_: &CoreRouterEngine, inner| {
            authority_mutation(CleanupBatch {
                inner,
                manifest_digest,
                obligation_count,
            });
            host_trace.push(HostTraceEvent::project(
                HostTraceEventKind::SettlementStarted,
                version,
                slot,
            ));
        };
        let result = match trigger {
            Some(trigger) => self.inner.begin_failed_authority_settlement_with_retention(
                &mut self.admission,
                lease.inner,
                trigger.inner,
                cause.inner(),
                &plan,
                seal,
                owners,
                publication,
            ),
            None => self.inner.begin_authority_settlement_with_retention(
                &mut self.admission,
                lease.inner,
                cause.inner(),
                &plan,
                seal,
                owners,
                publication,
            ),
        };
        match result {
            Ok(inner) => Ok(CleanupBatch {
                inner,
                manifest_digest,
                obligation_count,
            }),
            Err((lease, owners, kind)) => Err(RetainedAuthoritySettlementError {
                lease: HostLease { inner: lease },
                plan,
                manifest,
                owners,
                kind: map_begin_authority_settlement_error(kind),
            }),
        }
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

    /// Observes pending execution work without moving its lifecycle owner.
    #[doc(hidden)]
    pub fn has_action(&self) -> bool {
        self.inner.has_action()
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

    /// Settles the original mount request before retiring an invalid selected instance.
    #[doc(hidden)]
    pub fn reject_retained_mount(
        &mut self,
        request: MountRequest,
        invalid: RouteFrameVersion,
    ) -> Result<FailureCleanupRequest, RejectMountError> {
        self.inner
            .reject_retained_mount(request.inner, invalid)
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
        self.reject_mount_mutation_impl(artifact, None)
    }

    /// Settles the original staged mount before retiring an invalid selected instance.
    #[doc(hidden)]
    pub fn reject_retained_mount_after_mutation(
        &mut self,
        artifact: MountMutationArtifact,
        invalid: RouteFrameVersion,
    ) -> Result<FailureCleanupRequest, RejectMountMutationError> {
        self.reject_mount_mutation_impl(artifact, Some(invalid))
    }

    fn reject_mount_mutation_impl(
        &mut self,
        artifact: MountMutationArtifact,
        invalid: Option<RouteFrameVersion>,
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
        let result = match invalid {
            Some(invalid) => self
                .inner
                .reject_retained_mount_after_mutation(artifact, invalid),
            None => self.inner.reject_mount_after_mutation(artifact),
        };
        result
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
        message: impl Into<String>,
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
        message: impl Into<String>,
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

    /// Freezes the exact current/next partition and complete live retained inventory.
    pub fn prepare_retained_commit_plan(
        &mut self,
        ticket: &CommitTicket,
        inventory: &[&RetainedLeafOwner],
    ) -> Result<RetainedCommitPlan, IntegrationError> {
        self.inner
            .prepare_retained_commit_plan(&ticket.inner, inventory)
            .map_err(map_commit_error)
    }

    pub fn selected_retained_instances(&self) -> &[RetainedLeafProjection] {
        self.inner.selected_retained_instances()
    }

    /// Freezes one complete Current/Retained/Fresh choice before the first factory dispatch.
    pub fn prepare_instance_selection(
        &mut self,
        task: &FrameTask,
        inventory: &[&RetainedLeafOwner],
    ) -> Result<InstanceSelectionPlan, IntegrationError> {
        self.inner
            .prepare_instance_selection(&task.inner, inventory)
            .map_err(map_commit_error)
    }
    /// Accepts this request's exact selected prefix without moving its original owners.
    pub fn accept_instance_selection(
        &mut self,
        task: FrameTask,
        plan: InstanceSelectionPlan,
    ) -> Result<EngineAction, InstanceSelectionPreparationError> {
        match self.inner.accept_instance_selection(task.inner, &plan) {
            Ok(action) => Ok(match action {
                CoreEngineAction::FrameTask(inner) => EngineAction::FrameTask(FrameTask { inner }),
                CoreEngineAction::MountRequest(inner) => {
                    EngineAction::MountRequest(MountRequest { inner })
                }
                CoreEngineAction::Commit(inner) => EngineAction::Commit(CommitTicket { inner }),
                CoreEngineAction::FailureCleanup(inner) => {
                    EngineAction::FailureCleanup(FailureCleanupRequest { inner })
                }
                CoreEngineAction::BackResolution(inner) => EngineAction::BackResolution(inner),
            }),
            Err((task, error)) => Err(InstanceSelectionPreparationError {
                task: FrameTask { inner: task },
                plan,
                error: map_commit_error(error),
            }),
        }
    }

    /// Reissues only the cleared fresh suffix after the original selected instances are valid again.
    #[doc(hidden)]
    pub fn resume_retained_invalidation(
        &mut self,
        continuation: RetainedInvalidationContinuation,
        inventory: &[&RetainedLeafOwner],
    ) -> Result<EngineAction, RetainedInvalidationResumeError> {
        match self
            .inner
            .resume_retained_invalidation(continuation.inner, inventory)
        {
            Ok(action) => Ok(match action {
                CoreEngineAction::FrameTask(inner) => EngineAction::FrameTask(FrameTask { inner }),
                CoreEngineAction::MountRequest(inner) => {
                    EngineAction::MountRequest(MountRequest { inner })
                }
                CoreEngineAction::Commit(inner) => EngineAction::Commit(CommitTicket { inner }),
                CoreEngineAction::FailureCleanup(inner) => {
                    EngineAction::FailureCleanup(FailureCleanupRequest { inner })
                }
                CoreEngineAction::BackResolution(inner) => EngineAction::BackResolution(inner),
            }),
            Err((inner, failure)) => Err(RetainedInvalidationResumeError {
                continuation: RetainedInvalidationContinuation { inner },
                failure,
            }),
        }
    }

    /// Transfers the original local continuation only when the existing authority settlement accepts it.
    #[doc(hidden)]
    pub fn begin_retained_invalidation_authority_settlement<F>(
        &mut self,
        continuation: RetainedInvalidationContinuation,
        lease: HostLease,
        cause: PreCommitAuthorityCause,
        plan: RetainedAuthoritySettlementPlan,
        manifest: CleanupManifest,
        owners: Vec<RetainedLeafOwner>,
        authority_mutation: F,
    ) -> Result<CleanupBatch, RetainedInvalidationAuthoritySettlementError>
    where
        F: FnOnce(CleanupBatch),
    {
        if !self
            .inner
            .inspect_retained_invalidation_continuation(&continuation.inner)
        {
            return Err(RetainedInvalidationAuthoritySettlementError {
                continuation,
                settlement: RetainedAuthoritySettlementError {
                    lease,
                    plan,
                    manifest,
                    owners,
                    kind: BeginAuthoritySettlementErrorKind::Stale,
                },
            });
        }
        let mut original = Some(continuation);
        let result =
            self.begin_authority_settlement_with_retention(
                lease,
                cause,
                plan,
                manifest,
                owners,
                |batch| {
                    drop(original.take().expect(
                        "only the accepted authority tail consumes the original continuation",
                    ));
                    authority_mutation(batch);
                },
            );
        match result {
            Ok(batch) => Ok(batch),
            Err(settlement) => Err(RetainedInvalidationAuthoritySettlementError {
                continuation: original
                    .take()
                    .expect("rejection preserves the original continuation"),
                settlement,
            }),
        }
    }

    /// Ends an unaccepted local continuation through the existing authority
    /// fail-close protocol. Its original owner is consumed only by acceptance;
    /// an installed miss retains its separate cleanup owner and cannot enter here.
    #[doc(hidden)]
    pub fn fail_closed_retained_invalidation_authority_protocol_retaining<F>(
        &mut self,
        continuation: RetainedInvalidationContinuation,
        lease: Option<HostLease>,
        authority_mutation: F,
    ) -> Result<(), RetainedInvalidationAuthorityFailCloseError>
    where
        F: FnOnce(),
    {
        if lease.is_none() {
            return Err(RetainedInvalidationAuthorityFailCloseError {
                continuation,
                lease,
                kind: RetainedInvalidationAuthorityFailCloseErrorKind::ProtocolRejected,
            });
        }
        if !self
            .inner
            .inspect_retained_invalidation_continuation(&continuation.inner)
        {
            return Err(RetainedInvalidationAuthorityFailCloseError {
                continuation,
                lease,
                kind: RetainedInvalidationAuthorityFailCloseErrorKind::SourceMismatch,
            });
        }
        let mut original = Some(continuation);
        match self.fail_closed_authority_protocol_retaining(lease, || {
            drop(
                original.take().expect(
                    "only accepted authority fail-close consumes the original continuation",
                ),
            );
            authority_mutation();
        }) {
            Ok(()) => Ok(()),
            Err(lease) => Err(RetainedInvalidationAuthorityFailCloseError {
                continuation: original
                    .take()
                    .expect("rejected authority fail-close preserves its original continuation"),
                lease,
                kind: RetainedInvalidationAuthorityFailCloseErrorKind::ProtocolRejected,
            }),
        }
    }

    /// Settles fresh obligations under their original receiver before invalidating a selected instance.
    #[doc(hidden)]
    pub fn reject_retained_invalidation(
        &mut self,
        receiver: RetainedInvalidationReceiver,
        invalid: RouteFrameVersion,
    ) -> Result<FailureCleanupRequest, RetainedInvalidationReceiverError> {
        match self
            .inner
            .reject_retained_invalidation(receiver.into_inner(), invalid)
        {
            Ok(inner) => Ok(FailureCleanupRequest { inner }),
            Err((receiver, error)) => Err(RetainedInvalidationReceiverError {
                receiver: RetainedInvalidationReceiver::from_inner(receiver),
                error: map_commit_error(error),
            }),
        }
    }

    /// Freezes OwnInvalid responsibility from either the unaccepted first selection or the accepted prefix.
    pub fn prepare_retained_invalidation_plan(
        &self,
        receiver: &RetainedInvalidationReceiver,
        selection: Option<&InstanceSelectionPlan>,
        invalid: &RouteFrameVersion,
        inventory: &[&RetainedLeafOwner],
    ) -> Result<RetainedInvalidationPlan, IntegrationError> {
        match receiver {
            RetainedInvalidationReceiver::FrameTask(task) => {
                self.inner.prepare_retained_invalidation_plan(
                    crate::engine::retained::CoreRetainedInvalidationReceiverRef::FrameTask(
                        &task.inner,
                    ),
                    selection,
                    invalid,
                    inventory,
                )
            }
            RetainedInvalidationReceiver::Commit(ticket) => {
                self.inner.prepare_retained_invalidation_plan(
                    crate::engine::retained::CoreRetainedInvalidationReceiverRef::Commit(
                        &ticket.inner,
                    ),
                    selection,
                    invalid,
                    inventory,
                )
            }
            RetainedInvalidationReceiver::AfterFailure(owner) => {
                self.inner.prepare_retained_invalidation_plan(
                    crate::engine::retained::CoreRetainedInvalidationReceiverRef::AfterFailure(
                        &owner.inner,
                    ),
                    selection,
                    invalid,
                    inventory,
                )
            }
        }
        .map_err(map_commit_error)
    }
    /// Retires only the selected cache instance before applying the original fresh task.
    pub fn begin_retained_miss_cleanup<F>(
        &mut self,
        receiver: RetainedInvalidationReceiver,
        plan: RetainedInvalidationPlan,
        manifest: CleanupManifest,
        owners: Vec<RetainedLeafOwner>,
        install: F,
    ) -> Result<(), RetainedInvalidationPreparationError>
    where
        F: FnOnce(RetainedMissCleanupOwner),
    {
        let seal = CleanupManifestSeal::new(
            manifest.logical_roots.clone(),
            CleanupManifestDigest::from_opaque(manifest.manifest_digest),
            manifest.obligation_count,
        );
        let manifest_digest = manifest.manifest_digest;
        let obligation_count = manifest.obligation_count;
        match self.inner.begin_retained_miss_cleanup(
            receiver.into_inner(),
            &plan,
            seal,
            owners,
            |inner| {
                let cleanup = CleanupBatch {
                    inner: inner.cleanup().clone(),
                    manifest_digest,
                    obligation_count,
                };
                install(RetainedMissCleanupOwner { inner, cleanup });
            },
        ) {
            Ok(()) => Ok(()),
            Err((receiver, owners, error)) => Err(RetainedInvalidationPreparationError {
                receiver: RetainedInvalidationReceiver::from_inner(receiver),
                plan,
                manifest,
                owners,
                error: RetainedPublicationError::Integration(map_commit_error(error)),
            }),
        }
    }

    pub fn observe_retained_miss_cleanup_receipt(
        &mut self,
        owner: &mut RetainedMissCleanupOwner,
        disposition: CleanupDisposition,
    ) -> Result<(), IntegrationError> {
        self.inner
            .observe_retained_miss_cleanup_receipt(&mut owner.inner, disposition)
            .map_err(map_cleanup_error)
    }

    /// The trusted ECS receiver establishes shared authority loss and keeps
    /// the stopped physical ledger. Acceptance ends its original foreground
    /// receiver and transfers the original cache owners into that residual;
    /// it does not assert physical completion of the cache token.
    #[doc(hidden)]
    pub fn fail_closed_retained_miss_cleanup_authority_protocol_retaining<F>(
        &mut self,
        owner: RetainedMissCleanupOwner,
        lease: HostLease,
        install: F,
    ) -> Result<(), RetainedMissCleanupAuthorityFailCloseError>
    where
        F: FnOnce(Vec<RetainedLeafOwner>),
    {
        let RetainedMissCleanupOwner { inner, cleanup } = owner;
        self.inner
            .fail_closed_retained_miss_cleanup_authority_protocol_retaining(
                &mut self.admission,
                inner,
                lease.inner,
                install,
            )
            .map_err(
                |(inner, lease, kind)| RetainedMissCleanupAuthorityFailCloseError {
                    owner: RetainedMissCleanupOwner { inner, cleanup },
                    lease: HostLease { inner: lease },
                    kind,
                },
            )
    }

    pub fn complete_retained_miss_cleanup<Physical, F>(
        &mut self,
        owner: RetainedMissCleanupOwner,
        completion: CleanupCompletion,
        physical: Physical,
        reclaim: F,
    ) -> Result<EngineAction, RetainedMissCleanupCompletionError<Physical>>
    where
        F: FnOnce(Physical),
    {
        let RetainedMissCleanupOwner {
            inner: owner,
            cleanup,
        } = owner;
        let CleanupCompletion {
            inner: completion,
            manifest_digest,
            completion_digest,
            settled_count,
        } = completion;
        match self
            .inner
            .complete_retained_miss_cleanup(owner, completion, physical, reclaim)
        {
            Ok(action) => Ok(match action {
                CoreEngineAction::FrameTask(inner) => EngineAction::FrameTask(FrameTask { inner }),
                CoreEngineAction::MountRequest(inner) => {
                    EngineAction::MountRequest(MountRequest { inner })
                }
                CoreEngineAction::Commit(inner) => EngineAction::Commit(CommitTicket { inner }),
                CoreEngineAction::FailureCleanup(inner) => {
                    EngineAction::FailureCleanup(FailureCleanupRequest { inner })
                }
                CoreEngineAction::BackResolution(inner) => EngineAction::BackResolution(inner),
            }),
            Err((owner, completion, physical, error)) => Err(RetainedMissCleanupCompletionError {
                owner: RetainedMissCleanupOwner {
                    inner: owner,
                    cleanup,
                },
                completion: CleanupCompletion {
                    inner: completion,
                    manifest_digest,
                    completion_digest,
                    settled_count,
                },
                physical,
                error: map_cleanup_error(error),
            }),
        }
    }

    fn prepare_retained_back_binding(
        &self,
        back: Option<(&HistoryExecutionBindingView, &LifecycleContinuationCapacity)>,
        physical: CoreBackTargetPhysicalReceiverRef<'_>,
    ) -> Result<
        Option<(
            BackResolutionScopeProjection,
            HistoryExecutionBindingProjection,
        )>,
        RetainedPublicationError,
    > {
        let Some((history, continuation)) = back else {
            return Ok(None);
        };
        self.inner
            .inspect_back_target_execution_pairing(
                &continuation.inner.scope,
                &history.inner,
                physical,
            )
            .map_err(|inner| {
                RetainedPublicationError::BackTargetMismatch(PreparationReceiverMismatchView {
                    inner,
                })
            })?;
        Ok(Some((
            continuation.inner.scope.clone(),
            history.inner.clone(),
        )))
    }

    pub fn prepare_commit_with_retention(
        &mut self,
        ticket: &CommitTicket,
        plan: RetainedCommitPlan,
        manifest: CleanupManifest,
        owners: Vec<RetainedLeafOwner>,
        back: Option<(&HistoryExecutionBindingView, &LifecycleContinuationCapacity)>,
    ) -> Result<PreparedRetainedCommit, RetainedCommitPreparationError> {
        let back_target = match self.prepare_retained_back_binding(
            back,
            CoreBackTargetPhysicalReceiverRef::Commit(&ticket.inner),
        ) {
            Ok(binding) => binding,
            Err(error) => {
                return Err(RetainedCommitPreparationError {
                    plan,
                    manifest,
                    owners,
                    error,
                });
            }
        };
        let seal = CleanupManifestSeal::new(
            manifest.logical_roots.clone(),
            CleanupManifestDigest::from_opaque(manifest.manifest_digest),
            manifest.obligation_count,
        );
        // All owner/projection composition precedes installation of Ready.
        let retained = plan.retained().to_vec();
        match self
            .inner
            .prepare_retained_publication(&ticket.inner, &plan, seal, owners)
        {
            Ok((publication, token)) => Ok(PreparedRetainedCommit {
                publication,
                cleanup: CleanupBatch {
                    inner: token,
                    manifest_digest: manifest.manifest_digest,
                    obligation_count: manifest.obligation_count,
                },
                retained,
                back_target,
            }),
            Err((owners, error)) => Err(RetainedCommitPreparationError {
                plan,
                manifest,
                owners,
                error: RetainedPublicationError::Integration(map_commit_error(error)),
            }),
        }
    }

    pub fn validate_retained_publication(
        &mut self,
        prepared: PreparedRetainedCommit,
    ) -> Result<ValidatedRetainedCommit<'_>, (PreparedRetainedCommit, IntegrationError)> {
        if prepared.back_target.is_some() || prepared.history_publication().restores_back() {
            return Err((prepared, IntegrationError::Stale));
        }
        let PreparedRetainedCommit {
            publication,
            cleanup,
            retained,
            back_target,
        } = prepared;
        match self.inner.validate_retained_publication(publication) {
            Ok(publication) => Ok(ValidatedRetainedCommit {
                publication,
                cleanup,
                back_target_continuation: None,
            }),
            Err((publication, error)) => Err((
                PreparedRetainedCommit {
                    publication,
                    cleanup,
                    retained,
                    back_target,
                },
                map_commit_error(error),
            )),
        }
    }

    /// Consumes the original Back continuation only after its sealed binding and
    /// the retained publication owner both validate. Rejection returns both owners.
    pub fn validate_retained_publication_for_back(
        &mut self,
        prepared: PreparedRetainedCommit,
        continuation: LifecycleContinuationCapacity,
    ) -> Result<
        ValidatedRetainedCommit<'_>,
        (
            PreparedRetainedCommit,
            LifecycleContinuationCapacity,
            RetainedPublicationError,
        ),
    > {
        let Some((scope, execution)) = prepared.back_target.as_ref() else {
            return Err((
                prepared,
                continuation,
                RetainedPublicationError::Integration(IntegrationError::Stale),
            ));
        };
        if &continuation.inner.scope != scope {
            let mismatch = PreparationReceiverMismatchView {
                inner: CorePreparationReceiverMismatchProjection::Scope {
                    expected: scope.clone(),
                    observed: continuation.inner.scope.clone(),
                },
            };
            return Err((
                prepared,
                continuation,
                RetainedPublicationError::BackTargetMismatch(mismatch),
            ));
        }
        if let Err(inner) =
            self.inner
                .inspect_retained_back_publication(&prepared.publication, scope, execution)
        {
            return Err((
                prepared,
                continuation,
                RetainedPublicationError::BackTargetMismatch(PreparationReceiverMismatchView {
                    inner,
                }),
            ));
        }
        let PreparedRetainedCommit {
            publication,
            cleanup,
            retained,
            back_target,
        } = prepared;
        match self.inner.validate_retained_publication(publication) {
            Ok(publication) => Ok(ValidatedRetainedCommit {
                publication,
                cleanup,
                back_target_continuation: Some(continuation),
            }),
            Err((publication, error)) => Err((
                PreparedRetainedCommit {
                    publication,
                    cleanup,
                    retained,
                    back_target,
                },
                continuation,
                RetainedPublicationError::Integration(map_commit_error(error)),
            )),
        }
    }

    pub fn cancel_prepared_retained_commit_for_authority_settlement(
        &mut self,
        prepared: PreparedRetainedCommit,
    ) -> Result<Vec<RetainedLeafOwner>, (PreparedRetainedCommit, IntegrationError)> {
        let PreparedRetainedCommit {
            publication,
            cleanup,
            retained,
            back_target,
        } = prepared;
        self.inner
            .cancel_prepared_retained_publication_for_authority_settlement(publication)
            .map_err(|(publication, error)| {
                (
                    PreparedRetainedCommit {
                        publication,
                        cleanup,
                        retained,
                        back_target,
                    },
                    map_commit_error(error),
                )
            })
    }

    /// Recovers original owners only after the unpublished authority has been
    /// moved to its existing failure/authority settlement path.
    pub fn take_unpublished_retained_owners_for_settlement(
        &mut self,
    ) -> Result<Vec<RetainedLeafOwner>, IntegrationError> {
        self.inner
            .take_unpublished_retained_owners_for_settlement()
            .map_err(map_commit_error)
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
        message: impl Into<String>,
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
    pub fn finish_back_target_mount_failure(
        &mut self,
        capacity: LifecycleContinuationCapacity,
        request: FailureCleanupRequest,
    ) -> FinishBackTargetMountFailureOutcome {
        let owner = CoreBackTargetDriverFailureCleanup {
            scope: capacity.inner.scope.clone(),
            request: request.inner,
        };
        if let Err(inner) = self
            .inner
            .inspect_back_target_driver_failure_cleanup(&owner)
        {
            return FinishBackTargetMountFailureOutcome::Mismatch {
                capacity,
                request: FailureCleanupRequest {
                    inner: owner.request,
                },
                mismatch: PreparationReceiverMismatchView { inner },
            };
        }
        drop(capacity);
        FinishBackTargetMountFailureOutcome::Matched(FailureCleanupRequest {
            inner: owner.request,
        })
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
                CoreFailureCleanupCompletionDisposition::ResumeInstanceInvalidation(inner) => {
                    FailureCleanupCompletionDisposition::ResumeInstanceInvalidation(
                        RetainedInvalidationContinuation { inner },
                    )
                }
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
                self.host_trace.events.clone(),
            ),
        }
    }

    /// Compares the complete frozen projection without cloning its collections.
    #[doc(hidden)]
    pub fn snapshot_matches(&self, snapshot: &RouterSnapshotProjection) -> bool {
        let (router, evicted_through, highest_issued) = self.inner.request_retention_facts();
        self.inner.requested() == snapshot.requested()
            && self.inner.matched() == snapshot.matched()
            && self.inner.active() == snapshot.active()
            && self.inner.availability() == snapshot.availability()
            && self.inner.pending_phase() == snapshot.pending_phase()
            && snapshot.history.matches(self.inner.history())
            && self.inner.running_request() == snapshot.running_request()
            && self.inner.earliest_retained_sequence() == snapshot.earliest_retained_sequence()
            && snapshot.request_retention
                == RequestRetentionClassifier::project(router, evicted_through, highest_issued)
            && snapshot.diagnostics.requested().is_none()
            && snapshot.diagnostics.active().is_none()
            && self.inner.router_diagnostics() == snapshot.diagnostics.events()
            && self.inner.host() == snapshot.host.version()
            && self.inner.top_slot() == snapshot.host.top_slot()
            && self.host_trace.events == snapshot.host.trace()
    }

    pub fn active(&self) -> &ActivePresentation {
        self.inner.active()
    }

    pub fn availability(&self) -> &RouterAvailability {
        self.inner.availability()
    }

    pub fn current_input(&self, frame: &RouteFrameVersion) -> Option<RouteInputRef<'_>> {
        self.inner.current_input(frame)
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
    pub fn reconciliation_projection_for_pending_mount(
        &self,
    ) -> Result<ReconciliationProjectionView<'_>, IntegrationError> {
        self.inner
            .reconciliation_projection_for_pending_mount()
            .map(|inner| ReconciliationProjectionView { inner })
            .map_err(map_mount_error)
    }

    #[doc(hidden)]
    pub fn reconciliation_projection_for_mount_request(
        &self,
        request: &MountRequest,
    ) -> Result<ReconciliationProjectionView<'_>, IntegrationError> {
        self.inner
            .reconciliation_projection_for_mount_request(&request.inner)
            .map(|inner| ReconciliationProjectionView { inner })
            .map_err(map_mount_error)
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

    /// Read-only canonical identities; these keys cannot authorize history mutation.
    pub fn canonical_history(&self) -> CanonicalHistoryView {
        self.inner.canonical_history()
    }

    /// Reads the current canonical key without projecting all history entries.
    /// The key cannot authorize history mutation.
    pub fn current_history_entry(&self) -> Option<HistoryEntryKey> {
        self.inner.current_history_entry()
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

    /// Borrows retained records, including queued requests, without cloning.
    #[doc(hidden)]
    pub fn requests(&self) -> impl ExactSizeIterator<Item = NavigationRequestView<'_>> {
        self.inner
            .requests()
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
mod tests;

#[cfg(test)]
#[path = "integration_retained_tests.rs"]
mod retained_tests;
