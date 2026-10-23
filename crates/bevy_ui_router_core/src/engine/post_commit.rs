//! Post-publication authority transfer, settlement, and terminal completion.

use std::collections::VecDeque;

use crate::HostVersion;
use crate::admission::{
    HostAdmissionIssuer, HostLease, RetiredHostAuthority, RetiredHostAuthorityProjection,
};
use crate::cleanup::{
    CleanupBatchSourceProjection, CleanupBatchToken, CleanupCompletion, CleanupCompletionDigest,
    CleanupManifestDigest, CleanupManifestSeal,
};
use crate::diagnostics::{DiagnosticEvent, PostCommitCleanupAnomalyCause};
use crate::frame::{LogicalChildSlotBinding, RouteFrameVersion, RouteReconciliationProjection};
use crate::ids::{
    CleanupBatchId, NavigationRequestId, ReservedFrameAttempt, RouteSlotId, RouterIdentity,
};
use crate::location::RouteLocation;
use crate::presentation::{ActivePresentation, RouterNavigationState};
use crate::request::{
    NavigationFailure, NavigationFailureKind, NavigationPhase, NavigationRequestRecord,
    RequestStatus,
};
use crate::trace::NavigationTraceEvent;

use super::{
    CompletedCleanupTombstone, CurrentWork, DiagnosticTarget,
    FailureCleanupZeroTerminalAttemptProjection, FailureCleanupZeroTerminalBranchProjection,
    FailureCleanupZeroTerminalPendingMountProjection, FrameMountTarget, FrameTaskIdentity,
    ReportDispositionRecord, RootTaskIdentity, RouterEngine, TERMINAL_RETENTION, WorkStage,
    append_router_diagnostic, cleanup_disposition_digest_tag, digest_u64,
    failure_cleanup_zero_terminal_branch_projection, is_terminal, navigation_trace_digest,
    retained,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CleanupBatchProjection {
    pub(super) identity: RootTaskIdentity,
    pub(super) batch: CleanupBatchId,
    pub(super) manifest_digest: CleanupManifestDigest,
    pub(super) obligation_count: usize,
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
    pub(super) entries: Vec<RouteLocation>,
    pub(super) cursor: Option<usize>,
    pub(super) digest: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FrozenActiveBranchProjection {
    pub(super) identity: RootTaskIdentity,
    pub(super) presentation: ActivePresentation,
    pub(super) frames: Vec<RouteFrameVersion>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CorePostCommitCleanupReceiverProjection {
    pub(super) router: RouterIdentity,
    pub(super) request: NavigationRequestId,
    pub(super) identity: RootTaskIdentity,
    pub(super) normal_batch: CleanupBatchProjection,
    pub(super) current_host: HostVersion,
    pub(super) current_top_slot: RouteSlotId,
    pub(super) committed_history: FrozenCommittedHistoryProjection,
    pub(super) active: FrozenActiveBranchProjection,
    pub(super) retained_registry: Vec<super::retained::RetainedRegistryEntry>,
    pub(super) stage: CorePostCommitCleanupStageProjection,
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
    pub(super) identity: RootTaskIdentity,
    pub(super) batch: CleanupBatchProjection,
    pub(super) stage: CoreCompetingCleanupOwnerStage,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CorePostCommitCleanupObservedProjection {
    pub(super) receiver: Option<CorePostCommitCleanupReceiverProjection>,
    pub(super) retired_host: Option<RetiredHostAuthorityProjection>,
    pub(super) competing_cleanup: Option<CoreCompetingCleanupOwnerProjection>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CorePostCommitCompoundProjection {
    pub(super) receiver: CorePostCommitCleanupReceiverProjection,
    pub(super) authority_batch: CleanupBatchProjection,
    pub(super) retired_host: RetiredHostAuthorityProjection,
    pub(super) cause: PostCommitAuthorityCause,
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
    pub(super) expected: CorePostCommitCleanupReceiverProjection,
    pub(super) observed: CorePostCommitCleanupObservedProjection,
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
    pub(super) receiver: CorePostCommitCleanupReceiverProjection,
    pub(super) normal_source: CleanupBatchSourceProjection,
    pub(super) cause: PostCommitAuthorityCause,
    pub(super) retained: Option<super::retained::PostCommitRetainedTransfer>,
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
    pub(super) receiver: CorePostCommitCleanupReceiverProjection,
    pub(super) retired_host: RetiredHostAuthorityProjection,
    pub(super) cause: PostCommitAuthorityCause,
}

impl CoreRetiredHostAuthorityExpectedProjection {
    pub(crate) const fn cause(&self) -> PostCommitAuthorityCause {
        self.cause
    }
}

#[derive(Debug)]
pub(crate) struct CoreRetiredHostAuthorityTerminalEvidence {
    pub(super) expected: CoreRetiredHostAuthorityExpectedProjection,
    pub(super) consumed: RetiredHostAuthorityProjection,
}

impl CoreRetiredHostAuthorityTerminalEvidence {
    pub(crate) const fn cause(&self) -> PostCommitAuthorityCause {
        self.expected.cause
    }
}

pub(crate) struct CoreRetiredHostEdgeSettlementOwner {
    pub(super) authority: RetiredHostAuthority,
    pub(super) expected: CoreRetiredHostAuthorityExpectedProjection,
    pub(super) expected_physical_digest: u64,
}

pub(crate) struct CorePostCommitAuthorityPreparationSource {
    pub(super) transfer: CorePostCommitAuthorityTransferOwner,
    pub(super) host: HostLease,
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

    pub(crate) fn with_retention(
        mut self,
        plan: super::retained::RetainedPostCommitAuthorityPlan,
        owners: Vec<super::retained::RetainedLeafOwner>,
    ) -> Self {
        // Preserve every input even if a caller supplies an already-attached source.
        // Exact inventory validation below rejects an invalid combined transfer.
        let owners = match self.transfer.retained.take() {
            Some(mut previous) => {
                previous.owners.extend(owners);
                previous.owners
            }
            None => owners,
        };
        self.transfer.retained = Some(super::retained::PostCommitRetainedTransfer { plan, owners });
        self
    }

    pub(crate) fn retained_logical_roots(&self) -> Option<&[RouteFrameVersion]> {
        self.transfer
            .retained
            .as_ref()
            .map(|retained| retained.plan.logical_roots())
    }
}

pub(crate) struct CorePreparedPostCommitAuthorityTransfer {
    pub(super) transfer: CorePostCommitAuthorityTransferOwner,
    pub(super) host: HostLease,
    pub(super) authority_batch: CleanupBatchToken,
    pub(super) authority_manifest: CleanupManifestSeal,
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
pub(super) enum CorePostCommitTerminalPreparationFailure {
    Retryable(CorePostCommitCapacityKind),
    Permanent(CorePostCommitAuthorityContradiction),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CorePostCommitAuthorityBeginRetryEvidence {
    pub(super) expected: CorePostCommitCleanupReceiverProjection,
    pub(super) observed: CorePostCommitCleanupObservedProjection,
    pub(super) cause: PostCommitAuthorityCause,
    pub(super) missing_capacity: CorePostCommitCapacityKind,
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
    pub(super) expected: CorePostCommitCleanupReceiverProjection,
    pub(super) observed: CorePostCommitCleanupObservedProjection,
    pub(super) cause: PostCommitAuthorityCause,
    pub(super) contradiction: CorePostCommitAuthorityContradiction,
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
    pub(super) receiver: CorePostCommitCompoundProjection,
    pub(super) authority_batch: CleanupBatchToken,
    pub(super) host_edge: CoreRetiredHostEdgeSettlementOwner,
    pub(super) settlement: Option<CorePostCommitSettlementCapability>,
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
    pub(super) batch: CleanupBatchProjection,
    pub(super) manifest_digest: CleanupManifestDigest,
    pub(super) completion_digest: CleanupCompletionDigest,
    pub(super) settled_count: usize,
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
    pub(super) batch: CleanupBatchProjection,
    pub(super) completion_digest: CleanupCompletionDigest,
    pub(super) settled_count: usize,
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
    pub(super) expected: CoreRetiredHostAuthorityExpectedProjection,
    pub(super) physical_digest: u64,
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
    pub(super) cause: PostCommitAuthorityCause,
    pub(super) normal: CoreNormalCleanupCompletionFacts,
    pub(super) authority: CoreAuthorityCleanupCompletionFacts,
    pub(super) host_edge: CoreRetiredHostEdgeCompletionFacts,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CorePostCommitSettlementKeyProjection {
    pub(super) namespace: u64,
    pub(super) identity: Vec<u64>,
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
    pub(super) obligation: CorePostCommitSettlementKeyProjection,
    pub(super) physical_source: CorePostCommitSettlementKeyProjection,
    pub(super) disposition: crate::trace::CleanupDisposition,
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
    pub(super) obligation: CorePostCommitSettlementKeyProjection,
    pub(super) receipt: CorePostCommitSettlementKeyProjection,
    pub(super) disposition: crate::trace::CleanupDisposition,
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
    pub(super) obligation: CorePostCommitSettlementKeyProjection,
    pub(super) physical_source: CorePostCommitSettlementKeyProjection,
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
    pub(super) prior_trace_start: usize,
    pub(super) settled_original: Vec<CorePostCommitSettledOriginalProjection>,
    pub(super) residual_original: Vec<CorePostCommitPlannedCleanupProjection>,
    pub(super) authority_non_top: Vec<CorePostCommitPlannedCleanupProjection>,
    pub(super) authority_top: CorePostCommitPlannedCleanupProjection,
    pub(super) authority_top_dependency: CorePostCommitSettlementKeyProjection,
    pub(super) host_edge: CorePostCommitPlannedHostEdgeProjection,
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
    pub(super) kind: CorePostCommitSettlementPlanMismatchKind,
    pub(super) descriptor: CorePostCommitSettlementPlanDescriptor,
}

impl CorePostCommitSettlementPlanMismatchEvidence {
    pub(crate) const fn kind(&self) -> CorePostCommitSettlementPlanMismatchKind {
        self.kind
    }
}

pub(crate) struct CorePostCommitSettlementPlanBuilder {
    pub(super) compound: CorePostCommitCompoundProjection,
    pub(super) normal_batch: CleanupBatchSourceProjection,
    pub(super) authority_batch: CleanupBatchSourceProjection,
    pub(super) normal_obligation_count: usize,
    pub(super) authority_obligation_count: usize,
    pub(super) current_trace: Vec<NavigationTraceEvent>,
}

pub(crate) struct CoreBoundPostCommitSettlementPlan {
    pub(super) compound: CorePostCommitCompoundProjection,
    pub(super) normal_batch: CleanupBatchSourceProjection,
    pub(super) authority_batch: CleanupBatchSourceProjection,
    pub(super) descriptor: CorePostCommitSettlementPlanDescriptor,
    pub(super) prior_trace_prefix_digest: u64,
    pub(super) prior_trace_digest: u64,
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
pub(super) struct CorePostCommitSettlementPlanBindingProjection {
    pub(super) compound: CorePostCommitCompoundProjection,
    pub(super) normal_batch: CleanupBatchSourceProjection,
    pub(super) authority_batch: CleanupBatchSourceProjection,
    pub(super) prior_trace_start: usize,
    pub(super) prior_trace_prefix_digest: u64,
    pub(super) prior_trace_digest: u64,
    pub(super) settled_original: Vec<CorePostCommitSettledOriginalProjection>,
    pub(super) authority_top_dependency: CorePostCommitSettlementKeyProjection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CorePostCommitSettlementExpectedProjection {
    pub(super) ordinal: usize,
    pub(super) role: CorePostCommitSettlementRole,
    pub(super) obligation: CorePostCommitSettlementKeyProjection,
    pub(super) physical_source: CorePostCommitSettlementKeyProjection,
    pub(super) disposition: Option<crate::trace::CleanupDisposition>,
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
    pub(super) binding: CorePostCommitSettlementPlanBindingProjection,
    pub(super) expected: Vec<CorePostCommitSettlementExpectedProjection>,
    pub(super) cursor: usize,
    pub(super) recorded: Vec<CorePostCommitSettlementExpectedProjection>,
    pub(super) settlement_event_start: usize,
    pub(super) digest: u64,
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

    pub(super) fn is_complete(&self) -> bool {
        self.cursor == self.expected.len() && self.recorded == self.expected
    }
}

pub(super) struct CorePostCommitSettlementCapability {
    pub(super) projection: CorePostCommitSettlementTraceProjection,
}

impl CorePostCommitSettlementCapability {
    pub(super) fn from_plan(
        plan: CoreBoundPostCommitSettlementPlan,
        settlement_event_start: usize,
    ) -> Self {
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
    pub(super) expected: Option<CorePostCommitSettlementTraceProjection>,
    pub(super) observed_receiver: Option<CorePostCommitCompoundProjection>,
    pub(super) submitted: CorePostCommitCompoundProjection,
    pub(super) submitted_normal: CleanupBatchSourceProjection,
    pub(super) submitted_trace: CorePostCommitSettlementTraceProjection,
    pub(super) expected_next: Option<CorePostCommitSettlementExpectedProjection>,
    pub(super) submitted_next: Option<CorePostCommitSettlementExpectedProjection>,
    pub(super) kind: CorePostCommitSettlementTraceMismatchKind,
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
    pub(super) capability: CorePostCommitSettlementCapability,
    pub(super) expected: CorePostCommitSettlementExpectedProjection,
    pub(super) binding: u64,
}

impl CorePostCommitSettlementPermit {
    pub(crate) const fn expected(&self) -> &CorePostCommitSettlementExpectedProjection {
        &self.expected
    }
}

pub(crate) struct CorePreparedPostCommitSettlement {
    pub(super) owner: CorePostCommitCompoundBatchOwner,
    pub(super) permit: CorePostCommitSettlementPermit,
}

pub(crate) enum CorePostCommitSettlementPermitOutcome {
    Prepared(CorePreparedPostCommitSettlement),
    Mismatch {
        owner: CorePostCommitCompoundBatchOwner,
        evidence: CorePostCommitSettlementTraceMismatchEvidence,
    },
}

pub(crate) struct CorePostCommitSettlementAttestation<Physical> {
    pub(super) physical: Physical,
    pub(super) permit_binding: u64,
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
    pub(super) expected_permit_binding: u64,
    pub(super) observed_permit_binding: u64,
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
    pub(super) compound: CorePostCommitCompoundProjection,
    pub(super) cause: PostCommitAuthorityCause,
    pub(super) plan: CorePostCommitSettlementTraceProjection,
    pub(super) required_progress: CorePostCommitSettlementTraceProgress,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CorePostCommitCurrentSourceProjection {
    pub(super) identity: FrameTaskIdentity,
    pub(super) branch: Option<FailureCleanupZeroTerminalBranchProjection>,
    pub(super) location: RouteLocation,
    pub(super) reservation: Vec<ReservedFrameAttempt>,
    pub(super) target_frames: Vec<RouteFrameVersion>,
    pub(super) selected_retained_frames: Vec<RouteFrameVersion>,
    pub(super) applied_frames: Vec<RouteFrameVersion>,
    pub(super) current_target: FrameMountTarget,
    pub(super) pending_mount: Option<FailureCleanupZeroTerminalPendingMountProjection>,
    pub(super) authority_cleanup: Option<CorePostCommitAuthorityCleanupSourceProjection>,
    pub(super) cleanup_batch: CleanupBatchId,
    pub(super) cleanup_roots: Vec<RouteFrameVersion>,
    pub(super) attempt: FailureCleanupZeroTerminalAttemptProjection,
    pub(super) reconciliation: Option<RouteReconciliationProjection>,
    pub(super) retired_host: Option<RetiredHostAuthorityProjection>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CorePostCommitAuthorityCleanupSourceProjection {
    pub(super) normal: CleanupBatchSourceProjection,
    pub(super) expected: CorePostCommitCompoundProjection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CoreCompletedCleanupSourceProjection {
    pub(super) request: NavigationRequestId,
    pub(super) token: CleanupBatchSourceProjection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CorePreparedPostCommitTerminalSourceProjection {
    pub(super) cause: PostCommitAuthorityCause,
    pub(super) request_status: RequestStatus,
    pub(super) history_before: FrozenCommittedHistoryProjection,
    pub(super) recovering_record: NavigationTraceEvent,
    pub(super) aggregate_cleanup: NavigationTraceEvent,
    pub(super) diagnostic: DiagnosticEvent,
    pub(super) request_terminal: NavigationTraceEvent,
    pub(super) queued_terminals: Vec<NavigationTraceEvent>,
    pub(super) final_state: crate::presentation::RouterAvailability,
    pub(super) recovering_state: RouterNavigationState,
    pub(super) recovering_records: VecDeque<NavigationRequestRecord>,
    pub(super) final_navigation_state: RouterNavigationState,
    pub(super) final_records: VecDeque<NavigationRequestRecord>,
    pub(super) final_evicted_through: Option<u64>,
    pub(super) final_issued_view_tasks: Vec<FrameTaskIdentity>,
    pub(super) final_report_dispositions: Vec<ReportDispositionRecord>,
    pub(super) final_completed_cleanup: Vec<CoreCompletedCleanupSourceProjection>,
    pub(super) final_router_diagnostics: Vec<DiagnosticEvent>,
    pub(super) retain_completed_current: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CorePostCommitCompletionSourceProjection {
    pub(super) receiver: Option<CorePostCommitCompoundProjection>,
    pub(super) history: FrozenCommittedHistoryProjection,
    pub(super) state: RouterNavigationState,
    pub(super) records: VecDeque<NavigationRequestRecord>,
    pub(super) evicted_through: Option<u64>,
    pub(super) issued_view_tasks: Vec<FrameTaskIdentity>,
    pub(super) report_dispositions: Vec<ReportDispositionRecord>,
    pub(super) completed_cleanup: Vec<CoreCompletedCleanupSourceProjection>,
    pub(super) router_diagnostics: Vec<DiagnosticEvent>,
    pub(super) active_branch: Option<FailureCleanupZeroTerminalBranchProjection>,
    pub(super) active_child_slots: Vec<LogicalChildSlotBinding>,
    pub(super) current: Option<CorePostCommitCurrentSourceProjection>,
    pub(super) action_present: bool,
    pub(super) terminal_plan: Option<CorePreparedPostCommitTerminalSourceProjection>,
    pub(super) settlement_trace: Option<CorePostCommitSettlementTraceProjection>,
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
    pub(super) normal: CleanupCompletion,
    pub(super) normal_facts: CoreNormalCleanupCompletionFacts,
    pub(super) authority_expected: CoreAuthorityCleanupCompletionFacts,
    pub(super) authority: CoreAuthorityCleanupCompletionFacts,
    pub(super) host_edge: CoreRetiredHostEdgeCompletionFacts,
    pub(super) settlement: CorePostCommitSettlementTraceProjection,
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
    pub(super) batch: CorePostCommitCompoundBatchOwner,
    pub(super) normal: CleanupCompletion,
    pub(super) authority: CoreAuthorityCleanupCompletionFacts,
    pub(super) host_edge: CoreRetiredHostEdgeCompletionFacts,
    pub(super) source: CorePostCommitCompletionSourceProjection,
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
    pub(super) router: RouterIdentity,
    pub(super) request: NavigationRequestId,
    pub(super) cause: PostCommitAuthorityCause,
    pub(super) history: FrozenCommittedHistoryProjection,
    pub(super) availability: crate::presentation::RouterAvailability,
    pub(super) trace_digest: u64,
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
pub(super) struct PreparedPostCommitTerminalPlan {
    pub(super) cause: PostCommitAuthorityCause,
    pub(super) request_status: RequestStatus,
    pub(super) history_before: FrozenCommittedHistoryProjection,
    pub(super) recovering_record: NavigationTraceEvent,
    pub(super) aggregate_cleanup: NavigationTraceEvent,
    pub(super) diagnostic: DiagnosticEvent,
    pub(super) request_terminal: NavigationTraceEvent,
    pub(super) queued_terminals: Vec<NavigationTraceEvent>,
    pub(super) final_state: crate::presentation::RouterAvailability,
    pub(super) recovering_state: RouterNavigationState,
    pub(super) recovering_records: VecDeque<NavigationRequestRecord>,
    pub(super) final_navigation_state: RouterNavigationState,
    pub(super) final_records: VecDeque<NavigationRequestRecord>,
    pub(super) final_evicted_through: Option<u64>,
    pub(super) final_issued_view_tasks: Vec<FrameTaskIdentity>,
    pub(super) final_report_dispositions: Vec<ReportDispositionRecord>,
    pub(super) final_completed_cleanup: Vec<CompletedCleanupTombstone>,
    pub(super) final_router_diagnostics: Vec<DiagnosticEvent>,
    pub(super) retain_completed_current: bool,
}

impl From<&PreparedPostCommitTerminalPlan> for CorePreparedPostCommitTerminalSourceProjection {
    fn from(terminal: &PreparedPostCommitTerminalPlan) -> Self {
        Self {
            cause: terminal.cause,
            request_status: terminal.request_status.clone(),
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
            selected_retained_frames: current.selected_retained_frames.clone(),
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

impl RouterEngine {
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
            && self.prepared_publication_authority.is_none()
            && self.pending_retained_transfer.is_none();
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
            retained: None,
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
        let logical_roots = source
            .retained_logical_roots()
            .map(|roots| roots.to_vec())
            .unwrap_or_else(|| current.target_frames.iter().rev().cloned().collect());
        if !self.retained_post_commit_source_is_current(&source.transfer)
            || (source.transfer.retained.is_none()
                && self
                    .retained_leaves
                    .iter()
                    .any(retained::RetainedRegistryEntry::is_live))
            || manifest.logical_roots() != logical_roots
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
        mut owner: CorePreparedPostCommitAuthorityTransfer,
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
        let mut retained_after = self.retained_leaves.clone();
        if let Some(retained) = &owner.transfer.retained {
            Self::bind_retained_retirements(
                &mut retained_after,
                retained.plan.retiring(),
                &owner.authority_batch,
            );
        }
        cleanup_receiver.retained_registry = retained_after.clone();
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
        if observed.receiver.as_ref() != Some(&expected)
            || !self.retained_post_commit_source_is_current(&owner.transfer)
        {
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
        self.retained_leaves = retained_after;
        drop(owner.transfer.retained.take());

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
            retained_registry: self.retained_leaves.clone(),
            stage,
        })
    }

    pub(super) fn observe_post_commit_cleanup(&self) -> CorePostCommitCleanupObservedProjection {
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
        let request_status = self
            .current
            .as_ref()
            .expect("post-commit preparation retains the published attempt")
            .attempt
            .published_request_status();
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
        if record.transition(request_status.clone()).is_err() {
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
        let mut final_issued_view_tasks = self.issued_view_tasks.clone();
        let mut final_report_dispositions = self.report_dispositions.clone();
        append_router_diagnostic(
            &mut final_router_diagnostics,
            &mut final_report_dispositions,
            diagnostic.clone(),
        );
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
            request_status,
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
            request_status,
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
                terminal.request_status.clone(),
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
            .transition(request_status)
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
        let mut final_issued_view_tasks = self.issued_view_tasks.clone();
        let mut final_report_dispositions = self.report_dispositions.clone();
        append_router_diagnostic(
            &mut final_router_diagnostics,
            &mut final_report_dispositions,
            diagnostic,
        );
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
        self.settle_retained_cleanup(&normal);
        self.settle_retained_cleanup(&authority_batch);
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

pub(super) fn post_commit_settlement_trace_digest(
    trace: &CorePostCommitSettlementTraceProjection,
) -> u64 {
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
