use std::collections::VecDeque;

use crate::HostVersion;
use crate::admission::{HostLease, RetiredHostAuthority, RetiredHostAuthorityProjection};
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
use crate::request::{NavigationRequestRecord, RequestStatus};
use crate::trace::NavigationTraceEvent;

use super::{
    CompletedCleanupTombstone, FailureCleanupZeroTerminalAttemptProjection,
    FailureCleanupZeroTerminalBranchProjection, FailureCleanupZeroTerminalPendingMountProjection,
    FrameMountTarget, FrameTaskIdentity, ReportDispositionRecord, RootTaskIdentity,
    navigation_trace_digest, post_commit_settlement_trace_digest,
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
