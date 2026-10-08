use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

#[cfg(all(test, feature = "bsn"))]
mod native_tests;

#[cfg(test)]
use std::sync::atomic::AtomicUsize;

use bevy_camera::visibility::Visibility;
use bevy_ecs::{
    entity::Entity,
    hierarchy::{ChildOf, Children},
    prelude::World,
    resource::Resource,
};
use bevy_input_focus::InputFocus;
use bevy_picking::Pickable;
use bevy_ui_router_core::{
    integration as r3_core,
    integration::{
        ApprovedBackNoTargetCandidate, BackNoTargetApprovalOutcome,
        BackNoTargetAuthorityBeginOutcome, BackNoTargetAuthorityBeginOwner,
        BackNoTargetAuthorityBeginPermanentEvidence, BackNoTargetAuthorityBeginRetryEvidence,
        BackNoTargetAuthorityCleanupBatchOwner, BackNoTargetAuthorityCleanupCompletionOutcome,
        BackNoTargetAuthorityCleanupCompletionOwner, BackNoTargetAuthorityCleanupMismatchEvidence,
        BackNoTargetAuthorityCoreOutcome, BackNoTargetAuthorityManifest,
        BackNoTargetAuthoritySettlementOwner, BackNoTargetCleanupCompletionOutcome,
        BackNoTargetCleanupCompletionOwner, BackNoTargetCleanupInstallation,
        BackNoTargetReadyCoreOutcome, BackNoTargetSourceDriftOwner,
        BackNoTargetSourceDriftSettlementOutcome, BackNoTargetSourceDriftTerminalEvidence,
        BackNoTargetSourceRefreezeOutcome, BackResolutionEvidence,
        BackResolutionEvidenceClassification, BackResolutionOwnerHandoff,
        BackResolutionVerifyOutcome, BackTargetAppliedReport, BackTargetAppliedReportOutcome,
        BackTargetCommitPreparationFailure, BackTargetCommitPreparationOutcome,
        BackTargetCoreResumeOutcome, BackTargetDriverFailureConsumeOutcome,
        BackTargetDriverFailureCoreOutcome, BackTargetDriverFailureReportMode,
        BackTargetDriverFailureReportOutcome, BackTargetExecutionOwnerRef,
        BackTargetExecutionPairingOutcome, BackTargetPendingRejectedConsumeOutcome,
        BackTargetPermanentBeginOutcome, BackTargetPermanentBeginOwner,
        BackTargetPermanentBeginPermanentEvidence, BackTargetPermanentCleanupBatchOwner,
        BackTargetPermanentCleanupCompletionOutcome, BackTargetPermanentCleanupCompletionOwner,
        BackTargetPermanentCleanupMismatchEvidence, BackTargetPermanentOwner,
        BackTargetPermanentRequestFailureKind, BackTargetPermanentTerminalFailureKind,
        BackTargetPermanentTerminalOperation, BackTargetPhysicalOwnerRef,
        BackTargetPreparationAcceptance, BackTargetStart, BeginAuthoritySettlementErrorKind,
        CleanupManifest, CleanupManifestView, CommitTicket, EngineAction, EnqueueBackErrorKind,
        FailedAuthoritySettlementTrigger, FailedAuthorityTriggerError,
        FailureCleanupCompletionDisposition, FailureCleanupCompletionOwner,
        FailureCleanupCompletionPreparationMismatchView,
        FailureCleanupCompletionReceiverMismatchView, FailureCleanupPermanentAuthorityOwner,
        FailureCleanupPermanentAuthorityPhysical, FailureCleanupPermanentOutcome,
        FailureCleanupPermanentOwner, FailureCleanupPhysicalPrepareMismatch,
        FailureCleanupPhysicalPrepareOutcome, FailureCleanupPrepareOutcome, FailureCleanupRequest,
        FailureCleanupZeroPhysicalMismatchView, FailureCleanupZeroTerminalMismatchView,
        FailureCleanupZeroTerminalOutcome, FailureCleanupZeroTerminalOwner, FrameMountTarget,
        HistoryAdmissionStamp, HistoryAdmissionWorldSeal, HistoryExecutionBindingView, HostLease,
        IntegrationError, LifecycleContinuationCapacity, MountMutationArtifact, MountReceiptProof,
        MountRegistrationBeginError, MountRequest, PostCommitAuthorityAcceptanceOutcome,
        PostCommitAuthorityBeginOutcome, PostCommitAuthorityBeginPermanentEvidence,
        PostCommitAuthorityBeginRetryEvidence, PostCommitAuthorityCause,
        PostCommitAuthorityPreparationOutcome, PostCommitAuthorityPreparationSource,
        PostCommitCleanupObservedView, PostCommitCleanupReceiverView, PostCommitCompoundBatchOwner,
        PostCommitCompoundCompletionInputs, PostCommitCompoundCompletionOwner,
        PostCommitCompoundMismatchEvidence, PostCommitPlannedCleanupProjection,
        PostCommitPlannedHostEdgeProjection, PostCommitReceiverMismatchView,
        PostCommitSettledOriginalProjection, PostCommitSettlementKeyProjection,
        PostCommitSettlementPlanDescriptor, PostCommitSettlementPlanMismatchEvidence,
        PostCommitSettlementTraceMismatchEvidence, PostCommitSettlementTraceProgress,
        PreCommitAuthorityCause, PreparationBoundaryScopeView, PreparationReceiverMismatchView,
        PrepareBackAdmissionErrorKind, PreparedFailureCleanupZeroPhysical,
        PreparedPostCommitAuthorityTransfer, PreparedPostCommitContinuationOwner,
        ResolveMountOwner, RetiredHostAuthorityExpectedView, RetiredHostAuthorityTerminalEvidence,
        RootFrameTask, RouteFailureContinuationOwner, RouterEngine, TargetAuthoritySettlementOwner,
        TrustedPostCommitCompoundCompletionOutcome,
        TrustedPostCommitCompoundCompletionPreparationOutcome,
        TrustedPostCommitSettlementPermitOutcome, TrustedPostCommitSettlementRecordOutcome,
        VerifiedBackNoTargetCandidate, VerifiedBackResolutionContradiction,
        VerifiedBackResolutionOwnerHandoff, VerifiedBackTargetExecutionOwner, ViewFailureKind,
        ViewReportIdentity,
    },
    prelude::{
        MountRejectionKind, NavigationRequestId, RouteFrameVersion, RouteLocation,
        RouterAvailability,
    },
};
use bevy_ui_router_view::{RouteViewRegistry, ViewBuildEffect, integration as view_integration};

use crate::{
    app_ext::{RouterHandle, RouterHostRegistration, ViewDriverStore},
    branch_transaction::{
        FailedPreparedBranch, PendingCommit, PreparedBranch, PreparedPermanentIsolationClaimRebind,
        RealizedBranch, ReconciliationAuthorityIdentity, ReconciliationProjectionFacts,
        ReconciliationSlotRoleKind, SealedExistingReconciliation, SealedMountNewStagingAllowance,
        SealedReconciliationSplice, seal_reconciliation_staging_mount,
    },
    cleanup::{
        AuthoritySettlementManifestError, BackCapacityDomainIssuer,
        BackCapacityEnqueueRetireResidual, BackCapacityIssuer, BackCapacityIssuerInstallError,
        BackCapacityRetireDisposition, BackCapacityRetireError, BackCapacityRetireOutcome,
        BackCapacityRetirementEvidence, BackNoTargetZeroCompletionPreparationEvidence,
        BackNoTargetZeroCompletionSourceIdentity, BoundBackCapacityReservation, CleanupLedger,
        CleanupLedgerReserveError, CleanupLedgerReserveErrorKind, EnqueueRejected,
        FailedAuthorityBoundaryBinding, FailedBoundaryDrift, FailedBoundarySource,
        FailedManifestBuildError, FailedPreparedTopologyError, FailureCleanupSourceProjection,
        FrozenBackNoTargetAuthorityPartition, FrozenCleanupManifest,
        FrozenOriginalCleanupLedgerTransfer, FrozenPostCommitAuthorityPartition,
        ManifestNeutralLedgerSlot, NoTargetCapacityPermit, PendingBoundaryBinding,
        PostCommitAuthorityFreezeFailure, PostCommitAuthorityFreezePermanentEvidence,
        PostCommitAuthoritySettlementFailure, PostCommitOriginalFreezeFailure,
        PostCommitOriginalFreezePermanentEvidence, PostCommitPhysicalSettlementProof,
        PreparedBackNoTargetAuthorityCleanupLedger, PreparedBackNoTargetZeroCompletion,
        PreparedCleanupLedger, PreparedCleanupLedgerProjection, PreparedPostCommitAuthorityLedger,
        RouterCleanupObservation, TargetPreparationPermanentCompletionDomain,
        TargetPreparationPermanentCompletionFacts,
        TargetPreparationPermanentCompletionPreparationEvidence,
        TargetPreparationPermanentHostEdge, TargetPreparationPermanentReceiptObservationEvidence,
        TargetPreparationPermanentSettlementBoundaryEvidence,
        TargetPreparationPermanentSettlementError, TargetPreparationPermanentTransition,
        TargetPreparationPermanentTransitionStage, canonical_zero_completion_digest,
        canonical_zero_manifest_digest, cleanup_claim_has_exact_authority,
        failed_prepared_branch_roots_are_exact,
    },
    components::{
        ChildSlotAuthority, ChildSlotLifecycle, ProvisionalSlotLifecycle, RouteOwnershipSeal,
        RouterInputBarrier,
    },
    content::{ContentPlan, FrameContentWriter},
    driver::{EcsViewReportSession, ReportSeal, ViewBuildReport, ViewBuildReportKind},
    frame_local::{
        validate_active_branch_for_splice, validate_active_branch_local,
        validate_active_branch_with_entering, validate_branch_descendants,
        validate_entering_mount_parent, validate_leaving_branch_for_splice,
        validate_prepared_branch, validate_preserved_branch_local,
        validate_reconciliation_entering_branch,
    },
    host::{
        HostCandidateCapacityEvidence, HostCandidateClassification, HostCandidateIdentityCounter,
        HostCandidateObservationSource, HostCandidateRevisionLatch, HostCandidateRevisionState,
        PreparedEcsHostCandidateSource, PreparedHostCandidateRejection,
        ProvisionalHostCleanupLedger, ProvisionalHostSettlement, ProvisionalHostSettlementTerminal,
        ProvisionalStagingFailureEvidence, StagedProvisionalTopSlot, ValidatedHostCandidate,
        ValidatedHostCandidateObservation, activate_source_bound_granted_top_slot,
        classify_host_candidate, hide_source_bound_granted_top_slot,
        observe_validated_host_candidate, retain_staged_top_slot_for_settlement,
        stage_source_bound_granted_top_slot, validated_host_candidate_is_current,
    },
    input::{
        RouteFocusState, RouterInputGate, observed_descendant_closure, prepare_focus_mutation,
        prepare_input_gate_mutation,
    },
    lifecycle_preparation::{
        LifecyclePreflightEvidence, LifecyclePreflightExpectation, LifecyclePreflightObserved,
        LifecyclePreflightSource, RequiredResourceSet,
    },
    mount::{
        ChildSlotResidual, StagedChildSlot, confirm_child_slot, mounted_child_slot_is_isolated,
        mounted_child_slot_local_is_isolated, mounted_child_slot_structure_is_isolated,
        residual_child_slot_is_isolated, residual_child_slot_local_is_isolated, stage_child_slot,
    },
    presentation::{
        DeferredRealizationFailure, FrameLifecycle, FrameMountBinding, RealizedFrame,
        expected_children, frame_is_isolated_for_settlement, isolate_frame_for_settlement,
        isolate_frame_for_settlement_guarded, prepare_presentation_mutation,
        prepare_presentation_with_pointer_revocation, realize_child_frame_deferred,
        realize_frame_deferred, realize_reconciliation_child_frame_deferred, validate_active_frame,
        validate_realized_frame,
    },
    registry::{
        ComponentManifest, EntityLifecycle, EntityOwner, ExactRegistryClaim,
        PreparedLifecycleMutation, ProvisionalOwner, RegistryOwner, RouteEntityRegistry,
    },
    snapshots::RouterSnapshots,
};

fn allocate_navigation_domain(counter: &AtomicU64) -> Option<u64> {
    counter
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1).filter(|_| current != 0)
        })
        .ok()
}

pub(crate) struct HostBinding {
    pub(crate) version: bevy_ui_router_core::HostVersion,
    pub(crate) slot_id: bevy_ui_router_core::ids::RouteSlotId,
    pub(crate) host: bevy_ecs::entity::Entity,
    pub(crate) top_slot: bevy_ecs::entity::Entity,
    pub(crate) top_slot_claim: crate::registry::ExactRegistryClaim,
    pub(crate) candidate: ValidatedHostCandidate,
    pub(crate) registration_incarnation: u64,
    lease: HostLease,
}

pub(crate) struct RetiredHostBinding {
    pub(crate) version: bevy_ui_router_core::HostVersion,
    pub(crate) slot_id: bevy_ui_router_core::ids::RouteSlotId,
    pub(crate) host: bevy_ecs::entity::Entity,
    pub(crate) top_slot: bevy_ecs::entity::Entity,
    pub(crate) top_slot_claim: crate::registry::ExactRegistryClaim,
    pub(crate) candidate: ValidatedHostCandidate,
    pub(crate) registration_incarnation: u64,
}

impl HostBinding {
    fn into_retired(self) -> (RetiredHostBinding, HostLease) {
        let Self {
            version,
            slot_id,
            host,
            top_slot,
            top_slot_claim,
            candidate,
            registration_incarnation,
            lease,
        } = self;
        (
            RetiredHostBinding {
                version,
                slot_id,
                host,
                top_slot,
                top_slot_claim,
                candidate,
                registration_incarnation,
            },
            lease,
        )
    }
}

impl RetiredHostBinding {
    fn restore(self, lease: HostLease) -> HostBinding {
        let Self {
            version,
            slot_id,
            host,
            top_slot,
            top_slot_claim,
            candidate,
            registration_incarnation,
        } = self;
        HostBinding {
            version,
            slot_id,
            host,
            top_slot,
            top_slot_claim,
            candidate,
            registration_incarnation,
            lease,
        }
    }
}

#[cfg(test)]
#[derive(Resource)]
struct FailedAuthorityBeginManifestRejection {
    remaining: usize,
}

#[cfg(test)]
#[derive(Resource)]
struct FailedAuthorityTriggerExhaustion {
    remaining: usize,
}

#[cfg(test)]
#[derive(Resource, Default)]
struct ProjectionlessPrepareCommitRejection {
    remaining: usize,
}

#[cfg(test)]
#[derive(Resource, Default)]
struct ProjectionlessFailClosePreparationShortage {
    remaining: usize,
    observed_preparations: usize,
}

#[cfg(test)]
#[derive(Resource)]
struct TargetPreparationPermanentPostWriteDrift {
    target: bevy_ecs::entity::Entity,
}

#[cfg(test)]
#[derive(Resource)]
struct FailureCleanupPermanentZeroBindMismatch {
    request: NavigationRequestId,
    submitted_logical_root: RouteFrameVersion,
    observed_manifest_digest_xor: u64,
    permanent_prepare_calls: usize,
    observed_physical_digest_freezes: usize,
    submitted_manifest_digest: Option<u64>,
    observed_manifest_digest: Option<u64>,
    physical_witness_address: Option<usize>,
    authority_publication_calls: usize,
}

#[cfg(test)]
fn inject_failed_authority_begin_manifest_rejection(
    world: &mut World,
    logical_roots: &mut Vec<RouteFrameVersion>,
) {
    let reject = world
        .get_resource_mut::<FailedAuthorityBeginManifestRejection>()
        .is_some_and(|mut fault| {
            if fault.remaining == 0 {
                return false;
            }
            fault.remaining -= 1;
            true
        });
    if reject {
        let duplicate = logical_roots
            .first()
            .expect("failed authority rejection fixture requires one sealed root")
            .clone();
        logical_roots.push(duplicate);
    }
}

#[cfg(not(test))]
fn inject_failed_authority_begin_manifest_rejection(
    _world: &mut World,
    _logical_roots: &mut Vec<RouteFrameVersion>,
) {
}

#[cfg(test)]
fn inject_failure_cleanup_permanent_logical_root_contradiction(
    world: &mut World,
    request: NavigationRequestId,
    logical_roots: &mut Vec<RouteFrameVersion>,
    manifest_digest: u64,
) {
    let Some(mut fault) = world.get_resource_mut::<FailureCleanupPermanentZeroBindMismatch>()
    else {
        return;
    };
    if fault.request != request {
        return;
    }
    fault.submitted_manifest_digest = Some(manifest_digest);
    logical_roots.push(fault.submitted_logical_root.clone());
}

#[cfg(test)]
#[derive(Resource)]
enum FailureCleanupPrewriteDrift {
    Focus(Entity),
    HostTopSlot(Entity),
}

#[cfg(test)]
fn inject_failure_cleanup_prewrite_drift(world: &mut World, entry: &mut RouterEntry) {
    let Some(drift) = world.remove_resource::<FailureCleanupPrewriteDrift>() else {
        return;
    };
    match drift {
        FailureCleanupPrewriteDrift::Focus(entity) => {
            *world.resource_mut::<InputFocus>() = InputFocus::from_entity(entity);
            world
                .resource_mut::<RouteFocusState>()
                .set_focused(Some(entity));
        }
        FailureCleanupPrewriteDrift::HostTopSlot(entity) => {
            if let Some(host) = entry.host.as_mut() {
                host.top_slot = entity;
            }
        }
    }
}

#[cfg(not(test))]
fn inject_failure_cleanup_permanent_logical_root_contradiction(
    _world: &mut World,
    _request: NavigationRequestId,
    _logical_roots: &mut Vec<RouteFrameVersion>,
    _manifest_digest: u64,
) {
}

#[cfg(test)]
fn freeze_failure_cleanup_observed_physical_manifest_digest(
    world: &mut World,
    request: NavigationRequestId,
    manifest_digest: u64,
) -> u64 {
    let Some(mut fault) = world.get_resource_mut::<FailureCleanupPermanentZeroBindMismatch>()
    else {
        return manifest_digest;
    };
    if fault.request != request {
        return manifest_digest;
    }
    fault.observed_physical_digest_freezes += 1;
    let observed = manifest_digest ^ fault.observed_manifest_digest_xor;
    fault.observed_manifest_digest = Some(observed);
    observed
}

#[cfg(not(test))]
fn freeze_failure_cleanup_observed_physical_manifest_digest(
    _world: &mut World,
    _request: NavigationRequestId,
    manifest_digest: u64,
) -> u64 {
    manifest_digest
}

#[cfg(test)]
fn record_failure_cleanup_permanent_prepare(world: &mut World, request: NavigationRequestId) {
    let Some(mut fault) = world.get_resource_mut::<FailureCleanupPermanentZeroBindMismatch>()
    else {
        return;
    };
    if fault.request == request {
        fault.permanent_prepare_calls += 1;
    }
}

#[cfg(not(test))]
fn record_failure_cleanup_permanent_prepare(_world: &mut World, _request: NavigationRequestId) {}

#[cfg(test)]
fn issue_failure_cleanup_physical_witness(
    world: &mut World,
    request: NavigationRequestId,
) -> Box<u8> {
    let witness = Box::new(0_u8);
    if let Some(mut fault) = world
        .get_resource_mut::<FailureCleanupPermanentZeroBindMismatch>()
        .filter(|fault| fault.request == request)
    {
        fault.physical_witness_address = Some((&*witness as *const u8) as usize);
    }
    witness
}

#[cfg(test)]
fn record_failure_cleanup_permanent_authority_publication(
    world: &mut World,
    request: NavigationRequestId,
) {
    if let Some(mut fault) = world
        .get_resource_mut::<FailureCleanupPermanentZeroBindMismatch>()
        .filter(|fault| fault.request == request)
    {
        fault.authority_publication_calls += 1;
    }
}

#[cfg(not(test))]
fn record_failure_cleanup_permanent_authority_publication(
    _world: &mut World,
    _request: NavigationRequestId,
) {
}

#[cfg(test)]
fn inject_projectionless_prepare_commit_rejection(world: &mut World) -> bool {
    world
        .get_resource_mut::<ProjectionlessPrepareCommitRejection>()
        .is_some_and(|mut fault| {
            if fault.remaining == 0 {
                return false;
            }
            fault.remaining -= 1;
            true
        })
}

#[cfg(not(test))]
fn inject_projectionless_prepare_commit_rejection(_world: &mut World) -> bool {
    false
}

#[cfg(test)]
fn projectionless_fail_close_capacity_probe(world: &mut World) -> usize {
    world
        .get_resource_mut::<ProjectionlessFailClosePreparationShortage>()
        .map_or(0, |mut fault| {
            fault.observed_preparations += 1;
            if fault.remaining == 0 {
                return 0;
            }
            fault.remaining -= 1;
            usize::MAX
        })
}

#[cfg(not(test))]
fn projectionless_fail_close_capacity_probe(_world: &mut World) -> usize {
    0
}

enum PendingFailedBranchTerminal {
    Task {
        task: RootFrameTask,
        failure: ViewFailureKind,
        message: String,
    },
    TaskWithPhysicalResidual {
        task: RootFrameTask,
        failure: ViewFailureKind,
        message: String,
    },
    Request(FailureCleanupRequest),
    BackTargetPendingRejected {
        request: FailureCleanupRequest,
        evidence: LifecyclePreflightEvidence,
    },
    RegistrationBegin(MountRegistrationBeginError),
}

pub(crate) struct PendingFailedBranch {
    terminal: PendingFailedBranchTerminal,
    branch: FailedPreparedBranch,
}

impl PendingFailedBranch {
    #[cfg(feature = "bsn")]
    pub(crate) fn take_native_failure(
        &mut self,
    ) -> Option<crate::presentation::native_failure::NativeFailureOwner> {
        self.branch.take_native_failure()
    }
    pub(crate) const fn branch(&self) -> &FailedPreparedBranch {
        &self.branch
    }

    fn retained_prepared_prefix(&self) -> &[RouteFrameVersion] {
        match &self.terminal {
            PendingFailedBranchTerminal::Request(request)
            | PendingFailedBranchTerminal::BackTargetPendingRejected { request, .. } => {
                request.retained_prepared_prefix()
            }
            PendingFailedBranchTerminal::Task { .. }
            | PendingFailedBranchTerminal::TaskWithPhysicalResidual { .. }
            | PendingFailedBranchTerminal::RegistrationBegin(_) => &[],
        }
    }
}

struct FailureCleanupPhysicalPayload {
    branch: FailedPreparedBranch,
    reserved: PreparedCleanupLedger,
    preflight: Option<LifecyclePreflightEvidence>,
    #[cfg(test)]
    witness: Box<u8>,
}

impl FailureCleanupPhysicalPayload {
    fn branch(&self) -> &FailedPreparedBranch {
        &self.branch
    }

    fn into_ledger(self, batch: bevy_ui_router_core::integration::CleanupBatch) -> CleanupLedger {
        CleanupLedger::activate_failed_with_preflight(
            self.reserved.bind_batch(batch),
            self.branch,
            self.preflight,
        )
    }

    #[cfg(test)]
    fn witness_address(&self) -> usize {
        (&*self.witness as *const u8) as usize
    }
}

struct R1FailureCleanupPermanentAuthorityPair {
    physical: FailureCleanupPermanentAuthorityPhysical<FailureCleanupPhysicalPayload>,
    core: FailureCleanupPermanentAuthorityOwner,
}

enum FailureCleanupPermanentStableInertOwner {
    PrepareMismatch(FailureCleanupPhysicalPrepareMismatch<FailureCleanupPhysicalPayload>),
    CoreMismatch(FailureCleanupPermanentOwner<FailureCleanupPhysicalPayload>),
    ZeroBindMismatch {
        holder: FailureCleanupPermanentOwner<FailureCleanupPhysicalPayload>,
        evidence: FailureCleanupZeroPhysicalMismatchView,
    },
    ZeroTerminalMismatch {
        zero: PreparedFailureCleanupZeroPhysical<FailureCleanupPhysicalPayload>,
        owner: FailureCleanupZeroTerminalOwner,
        evidence: FailureCleanupZeroTerminalMismatchView,
    },
    AuthorityPreparation(R1FailureCleanupPermanentAuthorityPair),
    AuthorityRejected(R1FailureCleanupPermanentAuthorityPair),
}

impl FailureCleanupPermanentStableInertOwner {
    fn physical(&self) -> &FailureCleanupPhysicalPayload {
        match self {
            Self::PrepareMismatch(mismatch) => mismatch.physical(),
            Self::CoreMismatch(holder) | Self::ZeroBindMismatch { holder, .. } => holder.physical(),
            Self::ZeroTerminalMismatch { zero, .. } => zero.physical(),
            Self::AuthorityPreparation(pair) | Self::AuthorityRejected(pair) => {
                pair.physical.physical()
            }
        }
    }
}

#[cfg(test)]
fn pending_failed_branch_preflight_evidence_for_test(
    failed: &PendingFailedBranch,
) -> Option<LifecyclePreflightEvidence> {
    match &failed.terminal {
        PendingFailedBranchTerminal::BackTargetPendingRejected { evidence, .. } => Some(*evidence),
        PendingFailedBranchTerminal::Task { .. }
        | PendingFailedBranchTerminal::TaskWithPhysicalResidual { .. }
        | PendingFailedBranchTerminal::Request(_)
        | PendingFailedBranchTerminal::RegistrationBegin(_) => None,
    }
}

struct PendingFrameDrive {
    task: RootFrameTask,
    effect: ViewBuildEffect,
    pending: PendingCommit,
}

struct AppliedDriverReportArtifact {
    identity: bevy_ui_router_core::integration::ViewReportIdentity,
    kind: ViewBuildReportKind,
    plan: ContentPlan,
}

impl AppliedDriverReportArtifact {
    fn from_report(report: ViewBuildReport, plan: ContentPlan) -> Self {
        Self {
            identity: report.identity().clone(),
            kind: report.kind(),
            plan,
        }
    }
}

enum PendingMountMutationPayload {
    Grant {
        residual: Option<ChildSlotResidual>,
    },
    Receipt {
        staged: StagedChildSlot,
        proof: MountReceiptProof,
    },
}

struct PendingMountMutationRejection {
    pending: PendingCommit,
    artifact: MountMutationArtifact,
    payload: PendingMountMutationPayload,
    allowance: Option<SealedMountNewStagingAllowance>,
}

pub(crate) struct PendingBackTargetDriving {
    history: HistoryExecutionBindingView,
    continuation: LifecycleContinuationCapacity,
}

#[derive(Clone, Eq, PartialEq)]
struct BackTargetDriverTaskPhysicalProjection {
    report: ViewReportIdentity,
    request: NavigationRequestId,
    sequence: u64,
    frame: RouteFrameVersion,
    mount: FrameMountTarget,
}

#[derive(Clone, Eq, PartialEq)]
struct BackTargetDriverHostPhysicalProjection {
    version: bevy_ui_router_core::HostVersion,
    slot_id: bevy_ui_router_core::ids::RouteSlotId,
    host: bevy_ecs::entity::Entity,
    top_slot: bevy_ecs::entity::Entity,
    top_slot_claim: ExactRegistryClaim,
}

#[derive(Clone, Eq, PartialEq)]
enum BackTargetDriverRootPhysicalParentProjection {
    Host(bevy_ecs::entity::Entity),
    Claimed {
        entity: bevy_ecs::entity::Entity,
        claim: ExactRegistryClaim,
    },
}

#[derive(Clone, Eq, PartialEq)]
struct BackTargetDriverRootMountPhysicalProjection {
    frame: RouteFrameVersion,
    slot: bevy_ecs::entity::Entity,
    slot_claim: ExactRegistryClaim,
    physical_parent: BackTargetDriverRootPhysicalParentProjection,
}

#[derive(Clone, Eq, PartialEq)]
enum BackTargetDriverActiveChildSlotRole {
    Occupied(RouteFrameVersion),
    RetainedEmpty,
}

#[derive(Clone, Eq, PartialEq)]
struct BackTargetDriverActiveChildSlotProjection {
    parent: RouteFrameVersion,
    outlet: bevy_ui_router_core::slot::RouteSlotKey,
    slot_id: bevy_ui_router_core::ids::RouteSlotId,
    entity: bevy_ecs::entity::Entity,
    claim: ExactRegistryClaim,
    role: BackTargetDriverActiveChildSlotRole,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum BackTargetDriverObservedClaimState {
    Exact,
    MissingEntity,
    AuthorityDrift,
}

#[derive(Clone, Eq, PartialEq)]
struct BackTargetDriverObservedRegistryClaim {
    claim: ExactRegistryClaim,
    state: BackTargetDriverObservedClaimState,
}

#[derive(Clone, Eq, PartialEq)]
struct BackTargetDriverPhysicalPartitionProjection {
    router: RouterHandle,
    task: BackTargetDriverTaskPhysicalProjection,
    target_claims: Vec<ExactRegistryClaim>,
    host: BackTargetDriverHostPhysicalProjection,
    active_frames: Vec<RouteFrameVersion>,
    active_root_mount: Option<BackTargetDriverRootMountPhysicalProjection>,
    active_child_slots: Vec<BackTargetDriverActiveChildSlotProjection>,
    expected_active_claims: Vec<ExactRegistryClaim>,
    observed_active_claims: Vec<BackTargetDriverObservedRegistryClaim>,
    protected: Vec<bevy_ecs::entity::Entity>,
}

pub(crate) struct BackTargetDriverPhysicalPartitionSource {
    frozen: BackTargetDriverPhysicalPartitionProjection,
}

pub(crate) struct BackTargetDriverPostReportRefreezeSource {
    frozen: BackTargetDriverPhysicalPartitionProjection,
}

impl BackTargetDriverPhysicalPartitionSource {
    fn seal_for_report(self) -> BackTargetDriverPostReportRefreezeSource {
        BackTargetDriverPostReportRefreezeSource {
            frozen: self.frozen,
        }
    }
}

#[derive(Clone, Copy)]
enum BackTargetDriverPhysicalIndeterminateReason {
    MissingHost,
    MissingActive,
    HostAuthorityDrift,
    ActiveTopologyDrift,
    DuplicateActiveClaim,
    SourceDrift,
    UnsupportedRootLineage,
    ActiveRegistryIncomplete,
    PendingPhysicalResidual,
    ResidualTopologyIncomplete,
    TargetRegistryIncomplete,
    UnclassifiedTargetClaims,
    TargetProtectedOverlap,
}

enum BackTargetDriverPhysicalIndeterminateObservation {
    PairingMismatch(PreparationReceiverMismatchView),
    Freeze(BackTargetDriverPhysicalIndeterminateReason),
    Refreeze {
        expected: BackTargetDriverPhysicalPartitionProjection,
        observed: Option<BackTargetDriverPhysicalPartitionProjection>,
        reason: BackTargetDriverPhysicalIndeterminateReason,
    },
    Classification {
        source: BackTargetDriverPhysicalPartitionProjection,
        reason: BackTargetDriverPhysicalIndeterminateReason,
    },
}

pub(crate) struct FrozenBackTargetDriverPhysicalIndeterminateEvidence {
    observation: BackTargetDriverPhysicalIndeterminateObservation,
}

pub(crate) struct FrozenBackTargetDriverZeroPhysicalProof {
    source: BackTargetDriverPhysicalPartitionProjection,
}

pub(crate) struct BackTargetDriverZeroPhysicalOwner {
    physical: BackTargetDriverUnverifiedPhysicalOwner,
    proof: FrozenBackTargetDriverZeroPhysicalProof,
}

pub(crate) enum BackTargetDriverUnverifiedPhysicalOwner {
    Pending(PendingCommit),
    Residual(FailedPreparedBranch),
}

impl BackTargetDriverUnverifiedPhysicalOwner {
    #[cfg(feature = "bsn")]
    fn native_failure(&self) -> Option<&crate::presentation::native_failure::NativeFailureOwner> {
        match self {
            Self::Residual(branch) => branch.native_failure(),
            Self::Pending(_) => None,
        }
    }
}

pub(crate) struct FrozenBackTargetDriverPhysicalPartitionProof {
    source: BackTargetDriverPhysicalPartitionProjection,
    manifest: FrozenCleanupManifest,
    target_claims: Vec<ExactRegistryClaim>,
}

pub(crate) struct BackTargetDriverResidualPhysicalOwner {
    branch: FailedPreparedBranch,
    proof: FrozenBackTargetDriverPhysicalPartitionProof,
}

pub(crate) enum BackTargetDriverReportablePhysicalOwner {
    ProvenZero(BackTargetDriverZeroPhysicalOwner),
    Residual(BackTargetDriverResidualPhysicalOwner),
}

impl BackTargetDriverReportablePhysicalOwner {
    pub(crate) const fn report_mode(&self) -> BackTargetDriverFailureReportMode {
        match self {
            Self::ProvenZero(_) => BackTargetDriverFailureReportMode::Normal,
            Self::Residual(_) => BackTargetDriverFailureReportMode::PhysicalResidual,
        }
    }

    pub(crate) fn into_unverified(self) -> BackTargetDriverUnverifiedPhysicalOwner {
        match self {
            Self::ProvenZero(owner) => owner.physical,
            Self::Residual(owner) => {
                BackTargetDriverUnverifiedPhysicalOwner::Residual(owner.branch)
            }
        }
    }
}

pub(crate) struct BackTargetDriverPhysicalIndeterminateOwner {
    original: BackTargetDriverUnverifiedPhysicalOwner,
    evidence: FrozenBackTargetDriverPhysicalIndeterminateEvidence,
}

pub(crate) enum BackTargetDriverPhysicalClassification {
    Reportable(BackTargetDriverReportablePhysicalOwner),
    Indeterminate(BackTargetDriverPhysicalIndeterminateOwner),
}

pub(crate) struct PendingBackTargetDriverAwaitingReport {
    driving: PendingBackTargetDriving,
    task: RootFrameTask,
    source: BackTargetDriverPhysicalPartitionSource,
    physical: BackTargetDriverUnverifiedPhysicalOwner,
    kind: ViewFailureKind,
    message: String,
}

pub(crate) struct PendingBackTargetDriverReceiverMismatch {
    driving: PendingBackTargetDriving,
    task: RootFrameTask,
    physical: BackTargetDriverUnverifiedPhysicalOwner,
    kind: ViewFailureKind,
    message: String,
    mismatch: PreparationReceiverMismatchView,
}

pub(crate) struct PendingBackTargetDriverPhysicalIndeterminate {
    driving: PendingBackTargetDriving,
    task: RootFrameTask,
    physical: BackTargetDriverPhysicalIndeterminateOwner,
    kind: ViewFailureKind,
    message: String,
}

pub(crate) struct PendingBackTargetAppliedReportInput {
    driving: PendingBackTargetDriving,
    task: RootFrameTask,
    pending: PendingCommit,
    report: BackTargetAppliedReport,
}

pub(crate) struct PendingBackTargetAppliedReportMismatch {
    owner: PendingBackTargetAppliedReportInput,
    mismatch: PreparationReceiverMismatchView,
}

pub(crate) struct PendingBackTargetAppliedStagingMismatch {
    previous: Option<BackTargetDriverStagingOwner>,
    pending: PendingCommit,
    task: RootFrameTask,
    report: BackTargetAppliedReport,
    evidence: BackTargetDriverStagingMismatchEvidence,
}

pub(crate) struct PendingBackTargetAppliedReportAttempt {
    pending: PendingCommit,
    task: RootFrameTask,
    report: BackTargetAppliedReport,
}

pub(crate) struct PendingBackTargetAppliedStagingConflict {
    existing: PendingBackTargetAppliedStagingMismatch,
    incoming: PendingBackTargetAppliedReportAttempt,
    evidence: BackTargetDriverStagingMismatchEvidence,
}

pub(crate) struct PendingBackTargetAppliedMountRequestPairingMismatch {
    driving: PendingBackTargetDriving,
    request: MountRequest,
    pending: PendingCommit,
    mismatch: PreparationReceiverMismatchView,
}

pub(crate) struct PendingBackTargetAppliedCommit {
    driving: PendingBackTargetDriving,
    ticket: CommitTicket,
    pending: PendingCommit,
}

pub(crate) struct PendingBackTargetAppliedCommitPairingMismatch {
    owner: PendingBackTargetAppliedCommit,
    mismatch: PreparationReceiverMismatchView,
}

pub(crate) struct PendingBackTargetAppliedCommitPermanent {
    owner: PendingBackTargetAppliedCommit,
    evidence: LifecyclePreflightEvidence,
}

struct BackTargetDriverFailureReported {
    source: BackTargetDriverPostReportRefreezeSource,
    history: HistoryExecutionBindingView,
    physical: BackTargetDriverReportablePhysicalOwner,
    outcome: BackTargetDriverFailureCoreOutcome,
}

pub(crate) struct PendingBackTargetDriverConsumeMismatch {
    source: BackTargetDriverPostReportRefreezeSource,
    history: HistoryExecutionBindingView,
    physical: BackTargetDriverUnverifiedPhysicalOwner,
    outcome: BackTargetDriverFailureCoreOutcome,
    mismatch: PreparationReceiverMismatchView,
}

pub(crate) struct PendingBackTargetDriverPhysicalMismatch {
    source: BackTargetDriverPostReportRefreezeSource,
    history: HistoryExecutionBindingView,
    physical: BackTargetDriverUnverifiedPhysicalOwner,
    outcome: BackTargetDriverFailureCoreOutcome,
    evidence: BackTargetDriverPhysicalMismatchEvidence,
}

pub(crate) struct PendingBackTargetDriverPostReportPhysicalIndeterminate {
    source: BackTargetDriverPostReportRefreezeSource,
    history: HistoryExecutionBindingView,
    physical: BackTargetDriverUnverifiedPhysicalOwner,
    outcome: BackTargetDriverFailureCoreOutcome,
    evidence: FrozenBackTargetDriverPhysicalIndeterminateEvidence,
}

pub(crate) enum PendingBackTargetDriverFailure {
    PhysicalIndeterminate(PendingBackTargetDriverPhysicalIndeterminate),
    AwaitingReport(PendingBackTargetDriverAwaitingReport),
    ReceiverMismatch(PendingBackTargetDriverReceiverMismatch),
    ConsumeMismatch(PendingBackTargetDriverConsumeMismatch),
    PhysicalMismatch(PendingBackTargetDriverPhysicalMismatch),
    PostReportPhysicalIndeterminate(PendingBackTargetDriverPostReportPhysicalIndeterminate),
}

pub(crate) struct BackTargetDriverPhysicalMismatchEvidence {
    _private: (),
}

pub(crate) struct TargetPreparationOwner {
    splice: SealedReconciliationSplice,
    history: HistoryExecutionBindingView,
    continuation: LifecycleContinuationCapacity,
}

pub(crate) struct PendingBackTargetAuthorityProtocol {
    active: RealizedBranch,
    pending: PendingCommit,
    history: HistoryExecutionBindingView,
    continuation: LifecycleContinuationCapacity,
    evidence: LifecyclePreflightEvidence,
    source: BackTargetAuthorityProtocolSourceProjection,
}

#[derive(Clone, Eq, PartialEq)]
pub(crate) struct BackTargetAuthorityProtocolSourceProjection {
    missing_resources: RequiredResourceSet,
    host: Option<ProjectionlessHostProjection>,
    host_candidate: Option<ValidatedHostCandidateObservation>,
    observed: ProjectionlessObservedProjection,
    authority_loss: Option<LifecyclePreflightEvidence>,
}

pub(crate) struct BackTargetClassifiedPhysicalOwner {
    splice: SealedReconciliationSplice,
    history: HistoryExecutionBindingView,
    authority_preparation: Option<AcceptedBackTargetAuthorityPreparation>,
}

pub(crate) struct AcceptedBackTargetAuthorityPreparation {
    physical: FrozenTargetPreparationPermanentPhysicalManifest,
    ecs_manifest: FrozenCleanupManifest,
    core_binding: BackTargetPermanentAuthorityManifest,
}

pub(crate) struct PreparedBackTargetPhysicalOwner {
    classified: BackTargetClassifiedPhysicalOwner,
    focus_leaving: Vec<bevy_ecs::entity::Entity>,
}

pub(crate) struct BackTargetReadyAcceptanceOwner {
    physical: PreparedBackTargetPhysicalOwner,
    continuation: LifecycleContinuationCapacity,
    frozen_manifest: FrozenCleanupManifest,
    manifest: CleanupManifest,
}

pub(crate) struct PendingBackTargetReadyAccepted {
    physical: PreparedBackTargetPhysicalOwner,
    continuation: PreparedPostCommitContinuationOwner,
    frozen_manifest: FrozenCleanupManifest,
    manifest: CleanupManifest,
}

pub(crate) struct PendingPreparedBackTargetCommit {
    physical: PreparedBackTargetPhysicalOwner,
    continuation: PreparedPostCommitContinuationOwner,
    ledger: PreparedCleanupLedger,
    manifest: CleanupManifest,
}

pub(crate) enum PreparationRetryEvidence {
    RetryableCapacity(LifecyclePreflightEvidence),
    UnfinalizedBoundaryFacts(LifecyclePreflightEvidence),
}

pub(crate) enum PreparationPermanentEvidence {
    PhysicalPreparationContradiction(LifecyclePreflightEvidence),
    #[cfg(feature = "bsn")]
    NativeCleanup(crate::presentation::native_frame::NativeCleanupPreparationError),
}

pub(crate) struct PreparationPermanentOwner<Owner> {
    owner: Owner,
    evidence: PreparationPermanentEvidence,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FrozenTargetPreparationPermanentObservedEntity {
    entity: bevy_ecs::entity::Entity,
    parent: Option<bevy_ecs::entity::Entity>,
    children: Vec<bevy_ecs::entity::Entity>,
    registry_claim: Option<ExactRegistryClaim>,
    registry_manifest: Option<ComponentManifest>,
    ownership_seal: Option<RouteOwnershipSeal>,
    manifest: Option<ComponentManifest>,
    frame_lifecycle: Option<FrameLifecycle>,
    child_slot_lifecycle: Option<ChildSlotLifecycle>,
    child_slot_authority: Option<ChildSlotAuthority>,
    provisional_slot_lifecycle: Option<ProvisionalSlotLifecycle>,
}

#[derive(Clone)]
pub(crate) struct FrozenTargetPreparationPermanentLineage {
    router: RouterHandle,
    host: bevy_ecs::entity::Entity,
    ticket: ViewReportIdentity,
}

#[derive(Clone)]
pub(crate) struct FrozenTargetPreparationPermanentProtectedClosures {
    roots: Vec<bevy_ecs::entity::Entity>,
    observed: Vec<FrozenTargetPreparationPermanentObservedEntity>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FrozenTargetPreparationPermanentAllowedWriteSet {
    splice_targets: Vec<bevy_ecs::entity::Entity>,
    host_edge: FrozenTargetPreparationPermanentHostEdge,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FrozenTargetPreparationPermanentHostEdge {
    host: bevy_ecs::entity::Entity,
    top_slot: bevy_ecs::entity::Entity,
    host_observed: FrozenTargetPreparationPermanentObservedEntity,
    host_children_before: Vec<bevy_ecs::entity::Entity>,
    host_children_after: Vec<bevy_ecs::entity::Entity>,
    top_slot_parent_before: Option<bevy_ecs::entity::Entity>,
    top_slot_parent_after: Option<bevy_ecs::entity::Entity>,
}

pub(crate) enum FrozenTargetPreparationPermanentBranch {
    Classifier {
        physical: BackTargetClassifiedPhysicalOwner,
        evidence: PreparationPermanentEvidence,
    },
    ReadyPlan {
        physical: PreparedBackTargetPhysicalOwner,
        normal_frozen_manifest: FrozenCleanupManifest,
        normal_manifest: CleanupManifest,
        evidence: PreparationPermanentEvidence,
    },
}

#[derive(Clone)]
pub(crate) struct FrozenTargetPreparationPermanentPhysicalManifest {
    lineage: FrozenTargetPreparationPermanentLineage,
    target_observed: Vec<FrozenTargetPreparationPermanentObservedEntity>,
    protected: FrozenTargetPreparationPermanentProtectedClosures,
    allowed_write: FrozenTargetPreparationPermanentAllowedWriteSet,
    repair_missing_pickable: bool,
}

pub(crate) struct TargetPreparationPermanentCleanupOwner {
    physical: FrozenTargetPreparationPermanentPhysicalManifest,
    branch: FrozenTargetPreparationPermanentBranch,
}

pub(crate) struct BackTargetPermanentAuthorityManifest {
    cleanup: CleanupManifest,
}

pub(crate) struct TargetPreparationPermanentDisposition {
    cleanup: TargetPreparationPermanentCleanupOwner,
    settlement: TargetAuthoritySettlementOwner,
    ecs_manifest: FrozenCleanupManifest,
    core_binding: BackTargetPermanentAuthorityManifest,
}

pub(crate) struct PreparedTargetPreparationPermanentLedger {
    ledger: PreparedCleanupLedger,
    validation: FrozenTargetPreparationPermanentPhysicalManifest,
}

pub(crate) struct PreparedTargetPreparationPermanentDisposition {
    cleanup: TargetPreparationPermanentCleanupOwner,
    settlement: TargetAuthoritySettlementOwner,
    ecs_manifest: FrozenCleanupManifest,
    core_binding: BackTargetPermanentAuthorityManifest,
    ledger: PreparedTargetPreparationPermanentLedger,
}

pub(crate) struct CurrentTargetPreparationPermanentLedger {
    ledger: CleanupLedger,
    validation: FrozenTargetPreparationPermanentPhysicalManifest,
}

pub(crate) struct IsolatedTargetPreparationPermanentLedger {
    ledger: CleanupLedger,
    validation: FrozenTargetPreparationPermanentPhysicalManifest,
    expected_input_focus: Option<Option<bevy_ecs::entity::Entity>>,
    expected_route_focus: Option<Option<bevy_ecs::entity::Entity>>,
    completion_domain: Option<TargetPreparationPermanentCompletionDomain>,
}

struct PreparedIsolatedTargetPreparationPermanentCompletion {
    ledger: CleanupLedger,
    validation: FrozenTargetPreparationPermanentPhysicalManifest,
    expected_input_focus: Option<Option<bevy_ecs::entity::Entity>>,
    expected_route_focus: Option<Option<bevy_ecs::entity::Entity>>,
    completion_domain: TargetPreparationPermanentCompletionDomain,
    facts: TargetPreparationPermanentCompletionFacts,
}

impl PreparedIsolatedTargetPreparationPermanentCompletion {
    fn facts(&self) -> TargetPreparationPermanentCompletionFacts {
        self.facts
    }
}

impl IsolatedTargetPreparationPermanentLedger {
    fn prepare_completion(
        self,
    ) -> Result<
        PreparedIsolatedTargetPreparationPermanentCompletion,
        (
            Self,
            TargetPreparationPermanentCompletionPreparationEvidence,
        ),
    > {
        let Self {
            ledger,
            validation,
            expected_input_focus,
            expected_route_focus,
            completion_domain,
        } = self;
        match ledger.into_target_preparation_permanent_completion(completion_domain) {
            Ok((ledger, completion_domain, facts)) => {
                Ok(PreparedIsolatedTargetPreparationPermanentCompletion {
                    ledger,
                    validation,
                    expected_input_focus,
                    expected_route_focus,
                    completion_domain,
                    facts,
                })
            }
            Err((ledger, completion_domain, evidence)) => Err((
                Self {
                    ledger,
                    validation,
                    expected_input_focus,
                    expected_route_focus,
                    completion_domain,
                },
                evidence,
            )),
        }
    }
}

pub(crate) enum ActiveTargetPreparationPermanentLedger {
    Current(CurrentTargetPreparationPermanentLedger),
    Isolated(IsolatedTargetPreparationPermanentLedger),
}

impl PreparedTargetPreparationPermanentLedger {
    fn into_active(self) -> ActiveTargetPreparationPermanentLedger {
        ActiveTargetPreparationPermanentLedger::Current(CurrentTargetPreparationPermanentLedger {
            ledger: CleanupLedger::activate(self.ledger, None),
            validation: self.validation,
        })
    }
}

pub(crate) enum TargetPreparationPermanentLedgerHolder {
    Prepared(PreparedTargetPreparationPermanentLedger),
    Active(ActiveTargetPreparationPermanentLedger),
}

pub(crate) enum TargetPreparationPermanentInertEvidence {
    Physical(LifecyclePreflightEvidence),
    ReceiptPreparationRejected(TargetPreparationPermanentSettlementBoundaryEvidence),
    ReceiptObservationRejected(TargetPreparationPermanentReceiptObservationEvidence),
    AttemptUnavailable(TargetPreparationPermanentSettlementBoundaryEvidence),
    SettlementRejected(TargetPreparationPermanentSettlementBoundaryEvidence),
    PostWriteRefreeze(LifecyclePreflightEvidence),
    RepairPostWriteRefreeze(TargetPreparationPermanentRepairRefreezeEvidence),
    SettlementPrewriteRefreeze(TargetPreparationPermanentSettlementPrewriteEvidence),
    SettlementPostWriteRefreeze(TargetPreparationPermanentSettlementRefreezeEvidence),
    CompletionPreparationRejected(TargetPreparationPermanentCompletionPreparationEvidence),
}

pub(crate) struct TargetPreparationPermanentRepairRefreezeEvidence {
    entity: bevy_ecs::entity::Entity,
    expected: ProjectionlessObservedProjection,
    observed: ProjectionlessObservedProjection,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TargetPreparationPermanentSettlementRefreezeSource {
    ActiveIsolated,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TargetPreparationPermanentSettlementInputExpectation {
    input_focus: Option<bevy_ecs::entity::Entity>,
    route_focus: Option<bevy_ecs::entity::Entity>,
    targets: Vec<bevy_ecs::entity::Entity>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TargetPreparationPermanentSettlementInputObservation {
    input_resource_present: bool,
    input_focus: Option<bevy_ecs::entity::Entity>,
    route_resource_present: bool,
    route_focus: Option<bevy_ecs::entity::Entity>,
    gate_present: bool,
    non_isolated_targets: Vec<bevy_ecs::entity::Entity>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TargetPreparationPermanentSettlementHostExpectation {
    candidate: ValidatedHostCandidate,
    router: RouterHandle,
    host: FrozenTargetPreparationPermanentObservedEntity,
    parent: Option<bevy_ecs::entity::Entity>,
    children: Vec<bevy_ecs::entity::Entity>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TargetPreparationPermanentSettlementHostRegistrationObservation {
    router: RouterHandle,
    incarnation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TargetPreparationPermanentSettlementHostObservation {
    candidate_current: bool,
    registration: Option<TargetPreparationPermanentSettlementHostRegistrationObservation>,
    host: FrozenTargetPreparationPermanentObservedEntity,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TargetPreparationPermanentSettlementEntityExpectation {
    present: bool,
    registry_claim: Option<ExactRegistryClaim>,
    parent: Option<bevy_ecs::entity::Entity>,
    children: Vec<bevy_ecs::entity::Entity>,
    facts: Option<FrozenTargetPreparationPermanentObservedEntity>,
    isolated: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TargetPreparationPermanentSettlementEntityObservation {
    present: bool,
    facts: FrozenTargetPreparationPermanentObservedEntity,
    isolated: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum TargetPreparationPermanentSettlementRefreezeFailure {
    Transition {
        expected: TargetPreparationPermanentTransitionStage,
        observed_settled: Vec<bevy_ecs::entity::Entity>,
        observed_in_flight: Option<bevy_ecs::entity::Entity>,
    },
    InputIsolation {
        expected: TargetPreparationPermanentSettlementInputExpectation,
        observed: TargetPreparationPermanentSettlementInputObservation,
    },
    Registry {
        expected: bool,
        observed: bool,
    },
    HostIncarnation {
        expected: TargetPreparationPermanentSettlementHostExpectation,
        observed: TargetPreparationPermanentSettlementHostObservation,
    },
    Target {
        entity: bevy_ecs::entity::Entity,
        expected: TargetPreparationPermanentSettlementEntityExpectation,
        observed: TargetPreparationPermanentSettlementEntityObservation,
    },
    Protected {
        entity: bevy_ecs::entity::Entity,
        expected: FrozenTargetPreparationPermanentObservedEntity,
        observed: FrozenTargetPreparationPermanentObservedEntity,
    },
    Closure {
        expected: Vec<bevy_ecs::entity::Entity>,
        observed: Vec<bevy_ecs::entity::Entity>,
        relationship_exact: bool,
    },
}

pub(crate) struct TargetPreparationPermanentSettlementRefreezeEvidence {
    source: TargetPreparationPermanentSettlementRefreezeSource,
    stage: TargetPreparationPermanentTransitionStage,
    failure: TargetPreparationPermanentSettlementRefreezeFailure,
}

pub(crate) struct TargetPreparationPermanentSettlementPrewriteEvidence {
    source: TargetPreparationPermanentSettlementRefreezeSource,
    failure: TargetPreparationPermanentSettlementRefreezeFailure,
}

pub(crate) struct PreparedTargetPreparationPermanentPhysicalCompletion {
    cleanup: TargetPreparationPermanentCleanupOwner,
    prepared: PreparedIsolatedTargetPreparationPermanentCompletion,
    retired_host: RetiredHostBinding,
}

pub(crate) struct InstalledTargetPreparationPermanentCleanup {
    cleanup: TargetPreparationPermanentCleanupOwner,
    ledger: TargetPreparationPermanentLedgerHolder,
    batch: BackTargetPermanentCleanupBatchOwner,
    retired_host: RetiredHostBinding,
    inert: Option<TargetPreparationPermanentInertEvidence>,
}

pub(crate) struct PreparedTargetPermanentEcsInstallationOwner {
    cleanup: TargetPreparationPermanentCleanupOwner,
    ledger: PreparedTargetPreparationPermanentLedger,
    retired_host: RetiredHostBinding,
}

impl PreparedTargetPermanentEcsInstallationOwner {
    fn commit(
        self,
        batch: BackTargetPermanentCleanupBatchOwner,
    ) -> PendingTargetPreparationPermanentCleanup {
        PendingTargetPreparationPermanentCleanup::Installed(
            InstalledTargetPreparationPermanentCleanup {
                cleanup: self.cleanup,
                ledger: TargetPreparationPermanentLedgerHolder::Prepared(self.ledger),
                batch,
                retired_host: self.retired_host,
                inert: None,
            },
        )
    }
}

pub(crate) enum PendingTargetPreparationPermanentCleanup {
    Installed(InstalledTargetPreparationPermanentCleanup),
    Completing(PreparedTargetPreparationPermanentPhysicalCompletion),
    Settled {
        prepared: PreparedTargetPreparationPermanentPhysicalCompletion,
        owner: BackTargetPermanentCleanupCompletionOwner,
        mismatch: BackTargetPermanentCleanupMismatchEvidence,
    },
    #[cfg(test)]
    TestOccupied,
}

pub(crate) enum BackTargetPermanentDispatchOutcome {
    Dispatched,
    EntryMismatch {
        disposition: TargetPreparationPermanentDisposition,
        evidence: LifecyclePreflightEvidence,
    },
    LedgerRejected {
        disposition: TargetPreparationPermanentDisposition,
        error: CleanupLedgerReserveError,
    },
    PreparedEntryMismatch {
        disposition: PreparedTargetPreparationPermanentDisposition,
        evidence: LifecyclePreflightEvidence,
    },
    ReceiverMismatch {
        disposition: PreparedTargetPreparationPermanentDisposition,
        mismatch: PreparationReceiverMismatchView,
    },
    LogicalRootsMismatch(PreparedTargetPreparationPermanentDisposition),
    Permanent {
        disposition: PreparedTargetPreparationPermanentDisposition,
        evidence: BackTargetPermanentBeginPermanentEvidence,
    },
}

pub(crate) enum BackTargetClassifierPermanentPreparationOutcome {
    Prepared(TargetPreparationPermanentDisposition),
    PhysicalMismatch {
        owner: PreparationPermanentOwner<TargetPreparationOwner>,
        evidence: LifecyclePreflightEvidence,
    },
    ReceiverMismatch {
        owner: PreparationPermanentOwner<TargetPreparationOwner>,
        mismatch: PreparationReceiverMismatchView,
    },
}

pub(crate) enum BackTargetReadyPlanPermanentPreparationOutcome {
    Prepared(TargetPreparationPermanentDisposition),
    PhysicalMismatch {
        owner: PreparationPermanentOwner<BackTargetReadyAcceptanceOwner>,
        evidence: LifecyclePreflightEvidence,
    },
    ReceiverMismatch {
        owner: PreparationPermanentOwner<BackTargetReadyAcceptanceOwner>,
        mismatch: PreparationReceiverMismatchView,
    },
}

pub(crate) enum PendingBackTargetClassification {
    Active(TargetPreparationOwner),
    PairingMismatch {
        owner: TargetPreparationOwner,
        mismatch: PreparationReceiverMismatchView,
    },
    EntryMismatch {
        owner: TargetPreparationOwner,
        evidence: LifecyclePreflightEvidence,
    },
    ReadyAwaitingAcceptance(BackTargetReadyAcceptanceOwner),
    ReadyPlanRetry {
        owner: BackTargetReadyAcceptanceOwner,
        evidence: PreparationRetryEvidence,
    },
    ReadyPlanPermanent(PreparationPermanentOwner<BackTargetReadyAcceptanceOwner>),
    ReadyAcceptanceMismatch {
        owner: BackTargetReadyAcceptanceOwner,
        mismatch: PreparationReceiverMismatchView,
    },
    ReadyAccepted(PendingBackTargetReadyAccepted),
    ReadyReserveFailure {
        physical: PreparedBackTargetPhysicalOwner,
        continuation: PreparedPostCommitContinuationOwner,
        manifest: CleanupManifest,
        error: CleanupLedgerReserveError,
    },
    PendingRejectedUnaccepted {
        owner: TargetPreparationOwner,
        evidence: LifecyclePreflightEvidence,
    },
    PendingRejectedAcceptanceMismatch {
        owner: TargetPreparationOwner,
        evidence: LifecyclePreflightEvidence,
        mismatch: PreparationReceiverMismatchView,
    },
    PendingRejected {
        physical: BackTargetClassifiedPhysicalOwner,
        continuation: RouteFailureContinuationOwner,
        evidence: LifecyclePreflightEvidence,
    },
    PendingRejectedConsumeMismatch {
        physical: BackTargetClassifiedPhysicalOwner,
        continuation: RouteFailureContinuationOwner,
        evidence: LifecyclePreflightEvidence,
        mismatch: PreparationReceiverMismatchView,
    },
    AuthorityLostUnaccepted {
        owner: TargetPreparationOwner,
        evidence: LifecyclePreflightEvidence,
    },
    AuthorityLostProtocol(PendingBackTargetAuthorityProtocol),
    AuthorityLostProtocolSourceDrift {
        protocol: PendingBackTargetAuthorityProtocol,
        observed: BackTargetAuthorityProtocolSourceProjection,
    },
    AuthorityLostPhysicalMismatch {
        owner: TargetPreparationOwner,
        evidence: LifecyclePreflightEvidence,
        mismatch: LifecyclePreflightEvidence,
    },
    AuthorityLostAcceptanceMismatch {
        owner: TargetPreparationOwner,
        evidence: LifecyclePreflightEvidence,
        mismatch: PreparationReceiverMismatchView,
    },
    AuthorityLost {
        physical: BackTargetClassifiedPhysicalOwner,
        settlement: TargetAuthoritySettlementOwner,
        evidence: LifecyclePreflightEvidence,
    },
    AuthorityLostPreparationMissing {
        physical: BackTargetClassifiedPhysicalOwner,
        settlement: TargetAuthoritySettlementOwner,
        evidence: LifecyclePreflightEvidence,
    },
    AuthorityLostDispatchBlocked(BackTargetPermanentDispatchOutcome),
    Retry {
        owner: TargetPreparationOwner,
        evidence: PreparationRetryEvidence,
    },
    Permanent(PreparationPermanentOwner<TargetPreparationOwner>),
}

pub(crate) enum PendingBackTargetCommitFailure {
    PairingMismatch {
        owner: PendingPreparedBackTargetCommit,
        mismatch: PreparationReceiverMismatchView,
    },
    ReceiverMismatch {
        owner: PendingPreparedBackTargetCommit,
        mismatch: PreparationReceiverMismatchView,
    },
    Failed {
        physical: PreparedBackTargetPhysicalOwner,
        ledger: PreparedCleanupLedger,
        failure: BackTargetCommitPreparationFailure,
    },
}

pub(crate) enum PendingBackTargetResumeBlocked {
    Start {
        start: BackTargetStart,
        retirement: BackCapacityRetirementEvidence,
    },
    PairingMismatch {
        start: BackTargetStart,
        retirement: BackCapacityRetirementEvidence,
        mismatch: PreparationReceiverMismatchView,
    },
    DrivingPairingMismatch {
        owner: PendingBackTargetDriving,
        mismatch: PreparationReceiverMismatchView,
    },
    DriverFailure(PendingBackTargetDriverFailure),
    AppliedReportMismatch(PendingBackTargetAppliedReportMismatch),
    MountFailureMismatch {
        driving: PendingBackTargetDriving,
        mismatch: PreparationReceiverMismatchView,
    },
    CleanupOnlyMismatch {
        history: HistoryExecutionBindingView,
        branch: FailedPreparedBranch,
        outcome: BackTargetDriverFailureCoreOutcome,
        mismatch: PreparationReceiverMismatchView,
    },
    AppliedMountRequestPairingMismatch(PendingBackTargetAppliedMountRequestPairingMismatch),
    AppliedCommitPending(PendingBackTargetAppliedCommit),
    AppliedCommitPairingMismatch(PendingBackTargetAppliedCommitPairingMismatch),
    AppliedCommitPermanent(PendingBackTargetAppliedCommitPermanent),
    Permanent {
        owner: BackTargetPermanentOwner,
        retirement: BackCapacityRetirementEvidence,
    },
    CapacityRejected {
        bound: BoundBackTargetResolution,
        error: BackCapacityRetireError,
    },
    ReceiverMismatch {
        bound: BoundBackTargetResolution,
        evidence: BackResolutionEvidence,
    },
}

pub(crate) enum BackTargetDriverStagingOwner {
    ResumeBlocked(PendingBackTargetResumeBlocked),
    Driving(PendingBackTargetDriving),
    Classifying(PendingBackTargetClassification),
    Prepared(PendingPreparedBackTargetCommit),
    CommitFailure(PendingBackTargetCommitFailure),
    ExistingMismatch(Box<PendingBackTargetDriverStagingMismatch>),
    #[cfg(test)]
    TestDrift(PendingBackTargetDriving),
}

#[derive(Clone, Copy)]
pub(crate) enum BackTargetDriverStagingMismatchObservation {
    Missing,
    PairingMismatch,
    NonCurrentRoot,
    ResumeBlocked,
    Classifying,
    Prepared,
    CommitFailure,
    ExistingMismatch,
    ExistingAppliedMismatch,
    #[cfg(test)]
    TestDrift,
}

pub(crate) struct BackTargetDriverStagingMismatchEvidence {
    observed: BackTargetDriverStagingMismatchObservation,
    pairing: Option<PreparationReceiverMismatchView>,
}

pub(crate) struct PendingBackTargetDriverStagingMismatch {
    previous: Option<BackTargetDriverStagingOwner>,
    physical: BackTargetDriverUnverifiedPhysicalOwner,
    task: RootFrameTask,
    kind: ViewFailureKind,
    message: String,
    evidence: BackTargetDriverStagingMismatchEvidence,
}

pub(crate) enum PendingBackTargetExecution {
    ResumeBlocked(PendingBackTargetResumeBlocked),
    Driving(PendingBackTargetDriving),
    Classifying(PendingBackTargetClassification),
    Prepared(PendingPreparedBackTargetCommit),
    CommitFailure(PendingBackTargetCommitFailure),
    DriverFailureStagingMismatch(PendingBackTargetDriverStagingMismatch),
    AppliedReportStagingMismatch(PendingBackTargetAppliedStagingMismatch),
    AppliedReportStagingConflict(PendingBackTargetAppliedStagingConflict),
    #[cfg(test)]
    DriverFailureStagingDrift(PendingBackTargetDriving),
}

struct MountNewStagingSeed {
    request: NavigationRequestId,
    parent: RouteFrameVersion,
    outlet: bevy_ui_router_core::slot::RouteSlotKey,
    slot: bevy_ui_router_core::ids::RouteSlotId,
    anchor: bevy_ecs::prelude::Entity,
    anchor_binding: crate::presentation::OutletBinding,
    baseline_anchor_children: Vec<bevy_ecs::prelude::Entity>,
}

impl MountNewStagingSeed {
    fn from_validated_request(
        world: &World,
        request: NavigationRequestId,
        parent: &RealizedFrame,
        outlet: &bevy_ui_router_core::slot::RouteSlotKey,
        slot: bevy_ui_router_core::ids::RouteSlotId,
    ) -> Option<Self> {
        let outlet_record = parent.outlet(outlet)?;
        let anchor = outlet_record.anchor();
        let actual = world
            .get::<bevy_ecs::hierarchy::Children>(anchor)
            .map(|children| children.iter().copied().collect::<Vec<_>>())
            .unwrap_or_default();
        let baseline_anchor_children = if outlet_record.anchor_binding().is_native() {
            actual.clone()
        } else {
            expected_children(parent, anchor)
        };
        (outlet_record.parent() == parent.frame()
            && outlet_record.key() == outlet
            && outlet_record
                .anchor_binding()
                .is_current(world, parent.frame(), true)
            && actual == baseline_anchor_children)
            .then(|| Self {
                request,
                parent: parent.frame().clone(),
                outlet: outlet.clone(),
                slot,
                anchor,
                anchor_binding: outlet_record.anchor_binding().clone(),
                baseline_anchor_children,
            })
    }

    fn seal_staged(
        self,
        world: &World,
        staged: &StagedChildSlot,
    ) -> Option<SealedMountNewStagingAllowance> {
        self.seal_entity(world, staged.slot(), staged.slot_claim().clone())
    }

    fn seal_residual(
        self,
        world: &World,
        residual: &ChildSlotResidual,
    ) -> Option<SealedMountNewStagingAllowance> {
        (residual.parent() == &self.parent
            && residual.outlet() == &self.outlet
            && residual.anchor() == self.anchor
            && residual.anchor_binding() == &self.anchor_binding)
            .then_some(())?;
        self.seal_entity(world, residual.entity(), residual.claim().clone())
    }

    fn seal_entity(
        self,
        world: &World,
        entity: bevy_ecs::prelude::Entity,
        claim: crate::registry::ExactRegistryClaim,
    ) -> Option<SealedMountNewStagingAllowance> {
        use bevy_camera::visibility::Visibility;
        use bevy_picking::Pickable;

        let attached = world
            .get::<bevy_ecs::hierarchy::ChildOf>(entity)
            .map(bevy_ecs::hierarchy::ChildOf::parent)
            == Some(self.anchor);
        let mut expected_anchor_children = self.baseline_anchor_children;
        if attached {
            expected_anchor_children.push(entity);
        }
        let actual_anchor_children = world
            .get::<bevy_ecs::hierarchy::Children>(self.anchor)
            .map(|children| children.iter().copied().collect::<Vec<_>>())
            .unwrap_or_default();
        let record_is_exact = world
            .resource::<RouteEntityRegistry>()
            .record(entity)
            .is_some_and(|record| record.claim() == claim);
        let authority_is_exact = world
            .get::<crate::components::ChildSlotAuthority>(entity)
            .is_some_and(|authority| {
                authority.parent() == &self.parent
                    && authority.outlet() == &self.outlet
                    && authority.slot() == self.slot
            });
        let lifecycle_is_inert = matches!(
            world.get::<crate::components::ChildSlotLifecycle>(entity),
            Some(
                crate::components::ChildSlotLifecycle::ProvisionalInert
                    | crate::components::ChildSlotLifecycle::RegisteredInert
            )
        );
        if !record_is_exact
            || world.get::<crate::components::RouteOwnershipSeal>(entity)
                != Some(&crate::components::RouteOwnershipSeal::new(claim.seal()))
            || !authority_is_exact
            || !lifecycle_is_inert
            || world.get::<Visibility>(entity) != Some(&Visibility::Hidden)
            || world.get::<Pickable>(entity) != Some(&Pickable::IGNORE)
            || !world
                .get::<crate::components::RouterInputBarrier>(entity)
                .is_some_and(crate::components::RouterInputBarrier::denies_all)
            || actual_anchor_children != expected_anchor_children
        {
            return None;
        }
        SealedMountNewStagingAllowance::seal(
            self.request,
            self.parent,
            self.outlet,
            self.slot,
            entity,
            claim,
            self.anchor,
            self.anchor_binding,
            expected_anchor_children,
        )
    }
}

struct FailureCleanupCompletionInertOwner {
    owner: Option<FailureCleanupCompletionOwner>,
    ledger: CleanupLedger,
    mismatch: FailureCleanupCompletionMismatchView,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FrozenRetiredHostEdge {
    host: Entity,
    top_slot: Entity,
    top_slot_claim: ExactRegistryClaim,
    registration_incarnation: u64,
    host_present: bool,
    host_non_children_manifest: Option<ComponentManifest>,
    host_parent: Option<Entity>,
    host_ancestry: Vec<Entity>,
    children_before: Vec<Entity>,
    children_after: Vec<Entity>,
    top_parent_before: Option<Entity>,
    obligation_binding: u64,
    physical_source_binding: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RetiredHostEdgePhysicalTerminal {
    Detached,
    HostAbsent,
    EdgeAlreadyAbsent,
}

struct RetiredHostEdgePhysicalProof {
    terminal: RetiredHostEdgePhysicalTerminal,
    binding: u64,
}

enum PostCommitAnyPhysicalProof {
    Cleanup(PostCommitPhysicalSettlementProof),
    HostEdge(RetiredHostEdgePhysicalProof),
}

struct PreparedPostCommitPhysicalOwner {
    original: CleanupLedger,
    original_freeze: FrozenOriginalCleanupLedgerTransfer,
    authority: PreparedPostCommitAuthorityLedger,
    active: RealizedBranch,
    host: RetiredHostBinding,
    edge: FrozenRetiredHostEdge,
    input_isolation: Option<PreparedPostCommitInputIsolation>,
}

struct PreparedPostCommitInputIsolation {
    mutation: PreparedAuthorityIsolation,
    frozen: FrozenPostCommitInputIsolation,
    contradiction: Option<AuthorityIsolationContradiction>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FrozenPostCommitInputIsolation {
    focus: Vec<Entity>,
    gate_targets: Vec<Entity>,
    gate_required: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum PostCommitInputIsolationFailure {
    RetryableCapacity,
    Permanent(AuthorityIsolationContradiction),
    RefreezeRejected,
}

struct PostCommitCompoundPhysicalOwner {
    original: CleanupLedger,
    original_freeze: FrozenOriginalCleanupLedgerTransfer,
    authority: CleanupLedger,
    partition: FrozenPostCommitAuthorityPartition,
    active: RealizedBranch,
    host: RetiredHostBinding,
    edge: FrozenRetiredHostEdge,
    input_isolation: FrozenPostCommitInputIsolation,
    original_cursor: usize,
    authority_non_top_cursor: usize,
    settled_authority: Vec<Entity>,
    edge_terminal: Option<RetiredHostEdgePhysicalTerminal>,
    top_settled: bool,
}

struct ActivePostCommitCompoundOwner {
    core: PostCommitCompoundBatchOwner,
    physical: PostCommitCompoundPhysicalOwner,
}

struct PostCommitSettlementRecordInert {
    attempt: bevy_ui_router_core::integration::TrustedPostCommitSettlementAttempt,
    proof: PostCommitAnyPhysicalProof,
    physical: PostCommitCompoundPhysicalOwner,
    evidence: PostCommitSettlementTraceMismatchEvidence,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum RetiredHostEdgeFreezePermanentEvidence {
    InvalidBinding,
    InvalidRelationship,
    RelationshipCycle,
}

enum PostCommitPreAcceptInertEvidence {
    ReceiptObservationMismatch,
    Original(PostCommitOriginalFreezePermanentEvidence),
    EntryOwnerMissing,
    RetiredHostEdge(RetiredHostEdgeFreezePermanentEvidence),
    Authority(PostCommitAuthorityFreezePermanentEvidence),
    MissingNormalBatch,
    CoreReceiver {
        expected: PostCommitCleanupReceiverView,
        observed: PostCommitCleanupObservedView,
    },
}

struct PostCommitPreAcceptPhysicalOwner {
    original: CleanupLedger,
    original_freeze: Option<FrozenOriginalCleanupLedgerTransfer>,
    authority: Option<PreparedPostCommitAuthorityLedger>,
    edge: Option<FrozenRetiredHostEdge>,
}

enum PostCommitCompoundHolder {
    PreAcceptInert {
        physical: PostCommitPreAcceptPhysicalOwner,
        evidence: PostCommitPreAcceptInertEvidence,
    },
    AcceptedInert {
        source: PostCommitAuthorityPreparationSource,
        physical: Box<PreparedPostCommitPhysicalOwner>,
        evidence: Box<PostCommitAcceptedInertEvidence>,
    },
    PreparedInert {
        owner: PreparedPostCommitAuthorityTransfer,
        physical: PreparedPostCommitPhysicalOwner,
        evidence: PostCommitPlanBindFailure,
    },
    BoundInert {
        owner: PreparedPostCommitAuthorityTransfer,
        physical: Box<PostCommitCompoundPhysicalOwner>,
        evidence: Box<PostCommitBoundInertEvidence>,
    },
    Active(ActivePostCommitCompoundOwner),
    PermitMismatchInert {
        core: PostCommitCompoundBatchOwner,
        physical: PostCommitCompoundPhysicalOwner,
        evidence: PostCommitSettlementTraceMismatchEvidence,
    },
    PermitInert {
        attempt: bevy_ui_router_core::integration::TrustedPostCommitSettlementAttempt,
        physical: PostCommitCompoundPhysicalOwner,
        failure: PostCommitAuthoritySettlementFailure,
    },
    SettlementInert(PostCommitSettlementRecordInert),
    CompletionPreparationInert {
        core: PostCommitCompoundBatchOwner,
        physical: PostCommitCompoundPhysicalOwner,
        inputs: PostCommitCompoundCompletionInputs,
        evidence: PostCommitCompoundMismatchEvidence,
    },
    CompletionInert {
        owner: PostCommitCompoundCompletionOwner,
        physical: PostCommitCompoundPhysicalOwner,
        evidence: PostCommitCompoundMismatchEvidence,
    },
    Inert(ActivePostCommitCompoundOwner),
}

struct SettledRetiredHostEvidence {
    host: RetiredHostBinding,
    edge: FrozenRetiredHostEdge,
    edge_terminal: RetiredHostEdgePhysicalTerminal,
    terminal: bevy_ui_router_core::integration::PostCommitTerminalEvidence,
    retired_host: RetiredHostAuthorityTerminalEvidence,
}

struct PreparedHostAdmissionOperationFailure {
    pub(crate) evidence: bevy_ui_router_core::integration::HostAdmissionUnavailableEvidence,
}

struct HostAdmissionOperationPreStagingSource {
    pub(crate) prepared: PreparedHostAdmissionOperationFailure,
    pub(crate) owner: bevy_ui_router_core::integration::HostAdmissionUnavailablePreStagingOwner,
    pub(crate) installation: PreparedEcsHostCandidateSource,
}

enum HostAdmissionFailureHolder {
    TraceCapacityInert {
        owner: bevy_ui_router_core::integration::HostAdmissionUnavailablePreStagingOwner,
        candidate: PreparedEcsHostCandidateSource,
        evidence: bevy_ui_router_core::integration::HostAdmissionTraceCapacityFailureEvidence,
    },
    OperationEvidenceInert {
        owner: bevy_ui_router_core::integration::HostAdmissionUnavailablePreStagingOwner,
        candidate: PreparedEcsHostCandidateSource,
        evidence: bevy_ui_router_core::integration::HostAdmissionOperationFailureEvidenceMismatch,
    },
    CauseBindObservedInert {
        source: r3_core::HostAdmissionSourceProjection,
        evidence: R3HostAdmissionUnavailableEvidence,
        mismatch: r3_core::HostAdmissionSourceIneligibleProjection,
    },
    CauseBindPreStagingInert {
        owner: r3_core::HostAdmissionUnavailablePreStagingOwner,
        candidate: HostCandidateObservationSource,
        evidence: R3HostAdmissionUnavailableEvidence,
        mismatch: r3_core::HostAdmissionSourceIneligibleProjection,
    },
    CauseBindGrantedCandidateInert {
        grant: r3_core::SourceBoundHostGrant,
        candidate: PreparedEcsHostCandidateSource,
        evidence: R3HostAdmissionUnavailableEvidence,
        mismatch: r3_core::HostAdmissionSourceIneligibleProjection,
    },
    PreStaging(HostAdmissionOperationPreStagingSource),
    GrantedInert {
        grant: bevy_ui_router_core::integration::SourceBoundHostGrant,
        candidate: PreparedEcsHostCandidateSource,
    },
    Provisional(ProvisionalHostAdmissionFailure),
    ConfirmingInert(ConfirmingHostAdmissionInert),
    PublicationInert(HostAdmissionPublicationInert),
}

enum R3HostAdmissionUnavailableEvidence {
    NoCandidate(HostCandidateObservationSource),
    Conflict(HostCandidateObservationSource),
    InvalidTopology(HostCandidateObservationSource),
    IdentityExhausted {
        source: HostCandidateObservationSource,
        counter: HostCandidateIdentityCounter,
        current: u64,
        attempted: u64,
    },
    HostLost(HostCandidateObservationSource),
    Preallocation(HostCandidateObservationSource),
    Confirmation {
        source: HostCandidateObservationSource,
        evidence: r3_core::HostAdmissionConfirmationFailureEvidence,
    },
    AuthorityEstablishment(HostCandidateObservationSource),
    PostAllocation {
        source: HostCandidateObservationSource,
        evidence: Arc<ProvisionalStagingFailureEvidence>,
    },
    RetainedAdmission {
        source: HostCandidateObservationSource,
        evidence: idle_host_loss::IdleHostAdmissionFailure,
    },
    PostScopePhysical {
        source: HostCandidateObservationSource,
        evidence: crate::host::PostScopeHostConfirmationFailure,
    },
}

struct PreparedProvisionalHostInstallation {
    staged: StagedProvisionalTopSlot,
    candidate: PreparedEcsHostCandidateSource,
    claim: ExactRegistryClaim,
    version: bevy_ui_router_core::HostVersion,
    slot_id: bevy_ui_router_core::ids::RouteSlotId,
}

struct PendingHostConfirmation {
    grant: r3_core::SourceBoundHostGrant,
    candidate: PreparedEcsHostCandidateSource,
    staged: StagedProvisionalTopSlot,
    claim: ExactRegistryClaim,
    rejection: PreparedHostCandidateRejection,
}

struct ProvisionalHostAdmissionFailure {
    prepared: r3_core::PreparedHostAdmissionUnavailableCause<R3HostAdmissionUnavailableEvidence>,
    grant: r3_core::SourceBoundHostGrant,
    candidate: PreparedEcsHostCandidateSource,
    ledger: ProvisionalHostCleanupLedger,
    receiver: Option<r3_core::HostConfirmationReceiverMismatchProjection>,
    stable_inert: bool,
}

struct ConfirmingHostAdmissionInert {
    owner: r3_core::CorePreparedHostConfirmation<PreparedProvisionalHostInstallation>,
    evidence: r3_core::HostAdmissionConfirmationFailureEvidence,
    candidate: ValidatedHostCandidate,
    grant_version: bevy_ui_router_core::HostVersion,
    grant_top_slot: bevy_ui_router_core::ids::RouteSlotId,
    slot: Entity,
    claim: ExactRegistryClaim,
}

enum HostAdmissionPublicationInert {
    CauseBindGranted {
        grant: r3_core::SourceBoundHostGrant,
        candidate: PreparedEcsHostCandidateSource,
        settlement: ProvisionalHostSettlement,
        evidence: R3HostAdmissionUnavailableEvidence,
        mismatch: r3_core::HostAdmissionSourceIneligibleProjection,
    },
    PreStaging {
        source: r3_core::HostAdmissionUnavailablePreStagingSource<
            HostCandidateObservationSource,
            R3HostAdmissionUnavailableEvidence,
        >,
        evidence: r3_core::HostAdmissionUnavailableFailureEvidence,
    },
    Observed {
        prepared:
            r3_core::PreparedHostAdmissionUnavailableCause<R3HostAdmissionUnavailableEvidence>,
        evidence: r3_core::HostAdmissionUnavailableFailureEvidence,
    },
    Provisional {
        source: r3_core::HostAdmissionUnavailableGrantedSource<
            ProvisionalHostAdmissionTerminal,
            R3HostAdmissionUnavailableEvidence,
        >,
        evidence: r3_core::HostAdmissionUnavailableFailureEvidence,
    },
    GrantedCandidate {
        source: r3_core::HostAdmissionUnavailableGrantedSource<
            PreparedEcsHostCandidateSource,
            R3HostAdmissionUnavailableEvidence,
        >,
        evidence: r3_core::HostAdmissionUnavailableFailureEvidence,
    },
}

struct ProvisionalHostAdmissionTerminal {
    candidate: PreparedEcsHostCandidateSource,
    terminal: ProvisionalHostSettlementTerminal,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TestingHostConfirmationWholeOwnerStage {
    Absent,
    LegacyProvisionalSettlement,
    OtherAdmissionFailure,
    ProvisionalWholeOwner,
    ConfirmingUnwindMismatchInert,
}

#[cfg(test)]
#[derive(Clone, Debug, Eq, PartialEq)]
struct TestingHostConfirmationReceiverEvidenceProjection {
    expected: r3_core::HostConfirmationReceiverProjection,
    observed: Option<r3_core::HostConfirmationReceiverProjection>,
}

#[cfg(test)]
#[derive(Clone, Debug, Eq, PartialEq)]
struct TestingHostConfirmationWholeOwnerProjection {
    stage: TestingHostConfirmationWholeOwnerStage,
    candidate: Option<ValidatedHostCandidate>,
    grant_version: Option<bevy_ui_router_core::HostVersion>,
    grant_top_slot: Option<bevy_ui_router_core::ids::RouteSlotId>,
    slot: Option<Entity>,
    claim: Option<ExactRegistryClaim>,
    receiver: Option<TestingHostConfirmationReceiverEvidenceProjection>,
}

struct RetiredHostPhysicalTerminal {
    host: RetiredHostBinding,
    edge: FrozenRetiredHostEdge,
    edge_terminal: RetiredHostEdgePhysicalTerminal,
}

enum FailureCleanupCompletionMismatchView {
    Preparation(FailureCleanupCompletionPreparationMismatchView),
    Completion(FailureCleanupCompletionReceiverMismatchView),
}

pub(crate) struct RouterEntry {
    pub(crate) handle: RouterHandle,
    pub(crate) engine: RouterEngine,
    views: RouteViewRegistry,
    pub(crate) host: Option<HostBinding>,
    host_settlement: Option<ProvisionalHostSettlement>,
    pending_host_admission_failure: Option<HostAdmissionFailureHolder>,
    pending_host_confirmation: Option<PendingHostConfirmation>,
    pending_commit: Option<PendingCommit>,
    pending_frame_drive: Option<PendingFrameDrive>,
    #[cfg(feature = "bsn")]
    native_residuals: Vec<crate::presentation::native_failure::NativeTerminalResidual>,
    pending_mount_frame_task: Option<RootFrameTask>,
    pending_mount_request: Option<MountRequest>,
    pending_mount_mutation_rejection: Option<PendingMountMutationRejection>,
    pending_failed_branch: Option<PendingFailedBranch>,
    failure_cleanup_permanent: Option<FailureCleanupPermanentStableInertOwner>,
    projectionless_fail_close: Option<ProjectionlessFailCloseContinuation>,
    pub(crate) active: Option<RealizedBranch>,
    pub(crate) scroll: crate::scroll::ScrollState,
    pub(crate) retained: crate::retained::RetainedLeaves,
    pub(crate) cleanup_ledger: Option<CleanupLedger>,
    post_commit_compound: Option<Box<PostCommitCompoundHolder>>,
    settled_retired_host_evidence: Option<SettledRetiredHostEvidence>,
    idle_host_loss: Option<idle_host_loss::IdleHostLoss>,
    idle_host_terminal: Option<idle_host_loss::IdleHostTerminal>,
    deferred_host_observation: Option<DeferredHostObservation>,
    #[cfg(test)]
    commit_sealed_registration_removal_armed: bool,
    #[cfg(test)]
    commit_sealed_registration_removal_fired: bool,
    failure_cleanup_completion: Option<FailureCleanupCompletionInertOwner>,
    cleanup_observation: RouterCleanupObservation,
    pub(crate) authority_protocol_residual: Option<AuthorityProtocolResidualOwner>,
    back_capacity_enqueue_retire_residual: Option<BackCapacityEnqueueRetireResidual>,
    pub(crate) requests: Vec<NavigationRequestId>,
    back_capacity_reservations: Vec<(NavigationRequestId, BoundBackCapacityReservation)>,
    pending_back_resolution_action: Option<(BackResolutionOwnerHandoff, BackResolutionEvidence)>,
    pending_bound_back_resolution: Option<BoundBackResolutionHandoff>,
    pending_back_target_execution: Option<PendingBackTargetExecution>,
    pending_back_no_target_completion: Option<PendingBackNoTargetCompletion>,
    pending_back_no_target_preparation: Option<PendingBackNoTargetPreparation>,
    pending_back_no_target_authority_cleanup: Option<PendingBackNoTargetAuthorityCleanup>,
    pending_back_target_permanent_cleanup: Option<PendingTargetPreparationPermanentCleanup>,
    history_admission_world: HistoryAdmissionWorldSeal,
    history_admission_stamp: Option<HistoryAdmissionStamp>,
    back_capacity_issuer: Option<BackCapacityIssuer>,
    #[cfg(test)]
    back_resolution_evidence_predicate_calls: AtomicUsize,
    #[cfg(test)]
    back_target_driver_failure_dispatch_calls: AtomicUsize,
    #[cfg(test)]
    back_target_atomic_applied_dispatch_calls: AtomicUsize,
    #[cfg(test)]
    back_target_permanent_begin_callback_calls: AtomicUsize,
    host_admission_operation_counts: (usize, usize, usize, usize),
    #[cfg(test)]
    host_confirmation_receiver_drift_before_confirm_armed: bool,
    #[cfg(test)]
    host_confirmation_preparation_receiver_drift_before_prepare_armed: bool,
    #[cfg(test)]
    back_target_driver_staging_drift_armed: bool,
    #[cfg(test)]
    back_target_driver_report_receiver_drift_armed: bool,
    #[cfg(test)]
    back_target_driver_consume_receiver_drift_armed: bool,
    #[cfg(test)]
    failure_cleanup_completion_preparation_drift_armed: bool,
    #[cfg(test)]
    failure_cleanup_completion_receiver_drift_armed: bool,
    #[cfg(test)]
    back_no_target_completion_fact_drift: Option<BackNoTargetCompletionFactDrift>,
    #[cfg(test)]
    back_capacity_facade_producer:
        Option<bevy_ui_router_core::test_support::BackCapacityFacadeProducer>,
}

pub(crate) struct InstalledBackNoTargetCleanup {
    ledger: CleanupLedger,
    installation: BackNoTargetCleanupInstallation,
    inert: Option<BackNoTargetInstalledInertEvidence>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BackNoTargetInstalledInertEvidence {
    LedgerInvariant(BackNoTargetZeroCompletionPreparationEvidence),
    IssuerMissing {
        source: BackNoTargetZeroCompletionSourceIdentity,
    },
    ReclaimRejected {
        source: BackNoTargetZeroCompletionSourceIdentity,
        error: BackCapacityRetireError,
    },
}

pub(crate) struct SettledBackNoTargetCleanup {
    prepared: PreparedBackNoTargetZeroCompletion,
    completion: BackNoTargetCleanupCompletionOwner,
    mismatch: PreparationReceiverMismatchView,
}

pub(crate) enum PendingBackNoTargetCompletion {
    Installed(InstalledBackNoTargetCleanup),
    Settled(SettledBackNoTargetCleanup),
}

struct FrozenBackNoTargetAuthoritySource {
    request: NavigationRequestId,
    host: ProjectionlessHostProjection,
    active_frames: Vec<RouteFrameVersion>,
    active_claims: Vec<ExactRegistryClaim>,
}

struct BackNoTargetAuthorityPhysicalPreparation {
    source: FrozenBackNoTargetAuthoritySource,
    partition: Option<FrozenBackNoTargetAuthorityPartition>,
    reserved: PreparedCleanupLedger,
    active: RealizedBranch,
    isolation: Option<PreparedAuthorityIsolation>,
    host_children: Vec<bevy_ecs::entity::Entity>,
}

pub(crate) struct BackNoTargetAuthorityPhysicalSettlementOwner {
    preparation: BackNoTargetAuthorityPhysicalPreparation,
}

pub(crate) struct BackNoTargetAuthoritySettlementDisposition {
    settlement: BackNoTargetAuthoritySettlementOwner,
    retirement: BackCapacityRetirementEvidence,
    physical: BackNoTargetAuthorityPhysicalSettlementOwner,
    manifest: BackNoTargetAuthorityManifest,
}

pub(crate) struct InstalledBackNoTargetAuthorityCleanup {
    source: FrozenBackNoTargetAuthoritySource,
    retirement: BackCapacityRetirementEvidence,
    ledger: BackNoTargetAuthorityLedgerHolder,
    batch: Option<BackNoTargetAuthorityCleanupBatchOwner>,
    inert: Option<BackNoTargetAuthorityInertEvidence>,
}

enum BackNoTargetAuthorityLedgerHolder {
    Prepared(PreparedBackNoTargetAuthorityCleanupLedger),
    Active(CleanupLedger),
}

impl BackNoTargetAuthorityLedgerHolder {
    #[cfg(test)]
    fn leaving_frames(&self) -> &[RealizedFrame] {
        match self {
            Self::Prepared(ledger) => ledger.leaving_frames(),
            Self::Active(ledger) => ledger.leaving_frames(),
        }
    }
}

pub(crate) struct PreparedBackNoTargetAuthorityPhysicalCompletion {
    source: FrozenBackNoTargetAuthoritySource,
    retirement: BackCapacityRetirementEvidence,
    ledger: CleanupLedger,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BackNoTargetAuthorityInertEvidence {
    PartitionDrift,
    MissingBatch,
    ReceiptObservationRejected,
    AttemptUnavailable,
    SettlementRejected,
    CompletionFactsUnavailable,
}

pub(crate) struct SettledBackNoTargetAuthorityCleanup {
    prepared: PreparedBackNoTargetAuthorityPhysicalCompletion,
    completion: BackNoTargetAuthorityCleanupCompletionOwner,
    mismatch: BackNoTargetAuthorityCleanupMismatchEvidence,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BackNoTargetAuthorityBeginInertEvidence {
    MissingSlot,
    WrongPhase,
    BatchInstalledOnRejection,
    InertInstallation,
    ActiveLedgerOnRejection,
}

pub(crate) enum BackNoTargetAuthorityBeginInertPhysical {
    Installed(InstalledBackNoTargetAuthorityCleanup),
    Completing(PreparedBackNoTargetAuthorityPhysicalCompletion),
    Settled(SettledBackNoTargetAuthorityCleanup),
    PriorInert(Box<BackNoTargetAuthorityBeginInert>),
    CompletionInert(Box<BackNoTargetAuthorityCompletionInert>),
}

pub(crate) enum BackNoTargetAuthorityBeginInertCause {
    NotDispatched,
    ReceiverMismatch(PreparationReceiverMismatchView),
    Retryable(BackNoTargetAuthorityBeginRetryEvidence),
    Permanent(BackNoTargetAuthorityBeginPermanentEvidence),
}

pub(crate) struct BackNoTargetAuthorityBeginInert {
    begin: BackNoTargetAuthorityBeginOwner,
    physical: Option<BackNoTargetAuthorityBeginInertPhysical>,
    evidence: BackNoTargetAuthorityBeginInertEvidence,
    cause: BackNoTargetAuthorityBeginInertCause,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BackNoTargetAuthorityCompletionInertEvidence {
    MissingCompletingHolder,
    WrongPhase,
}

pub(crate) enum BackNoTargetAuthorityCompletionInertPhysical {
    Installed(InstalledBackNoTargetAuthorityCleanup),
    Completing(PreparedBackNoTargetAuthorityPhysicalCompletion),
    Settled(SettledBackNoTargetAuthorityCleanup),
    AuthorityBeginInert(BackNoTargetAuthorityBeginInert),
    PriorCompletionInert(Box<BackNoTargetAuthorityCompletionInert>),
}

pub(crate) struct BackNoTargetAuthorityCompletionInert {
    completion: BackNoTargetAuthorityCleanupCompletionOwner,
    mismatch: BackNoTargetAuthorityCleanupMismatchEvidence,
    physical: Option<BackNoTargetAuthorityCompletionInertPhysical>,
    evidence: BackNoTargetAuthorityCompletionInertEvidence,
}

pub(crate) enum PendingBackNoTargetAuthorityCleanup {
    Installed(InstalledBackNoTargetAuthorityCleanup),
    Completing(PreparedBackNoTargetAuthorityPhysicalCompletion),
    Settled(SettledBackNoTargetAuthorityCleanup),
    AuthorityBeginInert(BackNoTargetAuthorityBeginInert),
    CompletionInert(BackNoTargetAuthorityCompletionInert),
}

enum PendingBackNoTargetPreparation {
    PhysicalIndeterminate {
        owner: BackPreparationOwner,
        evidence: BackNoTargetPhysicalIndeterminateEvidence,
    },
    AuthorityPermanent(BackNoTargetAuthorityPermanentOwner),
    CapacityRejected {
        owner: BackPreparationOwner,
        error: BackCapacityRetireError,
    },
    ReceiverMismatch {
        owner: BackPreparationOwner,
        mismatch: PreparationReceiverMismatchView,
    },
    ReadyCapacityRejected {
        owner: PreparedBackNoTarget,
        error: BackCapacityRetireError,
    },
    ReadyReceiverMismatch {
        owner: PreparedBackNoTarget,
        mismatch: PreparationReceiverMismatchView,
    },
    AuthorityDispatchBlocked {
        disposition: BackNoTargetAuthoritySettlementDisposition,
    },
    SourceDriftBlocked {
        owner: BackNoTargetSourceDriftPermanentOwner,
    },
    #[cfg(test)]
    TestOccupied,
}

#[cfg(test)]
#[derive(Clone, Copy)]
enum BackNoTargetCompletionFactDrift {
    CompletionDigest,
    SettledCount,
}

pub(crate) struct BoundBackTargetResolution {
    owner: VerifiedBackTargetExecutionOwner,
    reservation: BoundBackCapacityReservation,
}

pub(crate) enum BackTargetResumeOutcome {
    Resumed {
        start: BackTargetStart,
        retirement: BackCapacityRetirementEvidence,
    },
    Permanent {
        owner: BackTargetPermanentOwner,
        retirement: BackCapacityRetirementEvidence,
    },
    CapacityRejected {
        bound: BoundBackTargetResolution,
        error: BackCapacityRetireError,
    },
    ReceiverMismatch {
        bound: BoundBackTargetResolution,
        evidence: BackResolutionEvidence,
    },
}

pub(crate) struct BoundBackNoTargetResolution {
    candidate: VerifiedBackNoTargetCandidate,
    reservation: BoundBackCapacityReservation,
}

pub(crate) struct BackPreparationOwner {
    candidate: VerifiedBackNoTargetCandidate,
    capacity: NoTargetCapacityPermit,
    request: NavigationRequestId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ProvenEmptyChildSlotBindingProjection {
    parent: RouteFrameVersion,
    outlet: bevy_ui_router_core::slot::RouteSlotKey,
    slot_id: bevy_ui_router_core::ids::RouteSlotId,
    anchor_binding: crate::presentation::OutletBinding,
    expected_support_relationships: Vec<bevy_ecs::entity::Entity>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ProvenEmptyOccupiedSlotProjection {
    child: RouteFrameVersion,
    binding: ProvenEmptyChildSlotBindingProjection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ProvenEmptyCurrentRole {
    FrameShell(RouteFrameVersion),
    FrameContentTarget(RouteFrameVersion),
    FrameOutput(RouteFrameVersion),
    OccupiedActiveSlotBinding(ProvenEmptyOccupiedSlotProjection),
    RetainedEmptySlotBinding(ProvenEmptyChildSlotBindingProjection),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ProvenEmptyCurrentClaimProjection {
    claim: ExactRegistryClaim,
    role: ProvenEmptyCurrentRole,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ProvenEmptyProtectedRole {
    AppOwnedHost,
    HostTopSlot,
    Current(ProvenEmptyCurrentRole),
    UnregisteredForeignDescendant,
    OtherAuthorityDescendant {
        expected: BackNoTargetRegisteredExtraExpectedProjection,
        observed: BackNoTargetRegisteredExtraObservedProjection,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ProvenEmptyProtectedEntityProjection {
    entity: bevy_ecs::entity::Entity,
    observed_parent: Option<bevy_ecs::entity::Entity>,
    claim: Option<ExactRegistryClaim>,
    role: ProvenEmptyProtectedRole,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ProvenEmptyAttemptOwnerInventory {
    request: NavigationRequestId,
    host_settlement_vacant: bool,
    pending_commit_vacant: bool,
    pending_frame_drive_vacant: bool,
    pending_mount_frame_task_vacant: bool,
    pending_mount_request_vacant: bool,
    pending_mount_mutation_rejection_vacant: bool,
    pending_failed_branch_vacant: bool,
    failure_cleanup_permanent_vacant: bool,
    projectionless_fail_close_vacant: bool,
    cleanup_ledger_vacant: bool,
    authority_protocol_residual_vacant: bool,
    back_capacity_enqueue_retire_residual_vacant: bool,
    back_capacity_reservations_vacant: bool,
    pending_back_resolution_action_vacant: bool,
    pending_bound_back_resolution_vacant: bool,
    pending_back_target_execution_vacant: bool,
    pending_back_no_target_completion_vacant: bool,
    pending_back_no_target_preparation_vacant: bool,
    pending_back_no_target_authority_cleanup_vacant: bool,
}

impl ProvenEmptyAttemptOwnerInventory {
    fn capture(entry: &RouterEntry, request: NavigationRequestId) -> Self {
        Self {
            request,
            host_settlement_vacant: entry.host_settlement.is_none(),
            pending_commit_vacant: entry.pending_commit.is_none(),
            pending_frame_drive_vacant: entry.pending_frame_drive.is_none(),
            pending_mount_frame_task_vacant: entry.pending_mount_frame_task.is_none(),
            pending_mount_request_vacant: entry.pending_mount_request.is_none(),
            pending_mount_mutation_rejection_vacant: entry
                .pending_mount_mutation_rejection
                .is_none(),
            pending_failed_branch_vacant: entry.pending_failed_branch.is_none(),
            failure_cleanup_permanent_vacant: entry.failure_cleanup_permanent.is_none(),
            projectionless_fail_close_vacant: entry.projectionless_fail_close.is_none(),
            cleanup_ledger_vacant: entry.cleanup_ledger.is_none(),
            authority_protocol_residual_vacant: entry.authority_protocol_residual.is_none(),
            back_capacity_enqueue_retire_residual_vacant: entry
                .back_capacity_enqueue_retire_residual
                .is_none(),
            back_capacity_reservations_vacant: entry.back_capacity_reservations.is_empty(),
            pending_back_resolution_action_vacant: entry.pending_back_resolution_action.is_none(),
            pending_bound_back_resolution_vacant: entry.pending_bound_back_resolution.is_none(),
            pending_back_target_execution_vacant: entry.pending_back_target_execution.is_none(),
            pending_back_no_target_completion_vacant: entry
                .pending_back_no_target_completion
                .is_none(),
            pending_back_no_target_preparation_vacant: entry
                .pending_back_no_target_preparation
                .is_none(),
            pending_back_no_target_authority_cleanup_vacant: entry
                .pending_back_no_target_authority_cleanup
                .is_none(),
        }
    }

    fn is_vacant(&self) -> bool {
        self.host_settlement_vacant
            && self.pending_commit_vacant
            && self.pending_frame_drive_vacant
            && self.pending_mount_frame_task_vacant
            && self.pending_mount_request_vacant
            && self.pending_mount_mutation_rejection_vacant
            && self.pending_failed_branch_vacant
            && self.failure_cleanup_permanent_vacant
            && self.projectionless_fail_close_vacant
            && self.cleanup_ledger_vacant
            && self.authority_protocol_residual_vacant
            && self.back_capacity_enqueue_retire_residual_vacant
            && self.back_capacity_reservations_vacant
            && self.pending_back_resolution_action_vacant
            && self.pending_bound_back_resolution_vacant
            && self.pending_back_target_execution_vacant
            && self.pending_back_no_target_completion_vacant
            && self.pending_back_no_target_preparation_vacant
            && self.pending_back_no_target_authority_cleanup_vacant
    }
}

pub(crate) struct ProvenEmptyAuthorizedTargets {
    private: (),
}

impl ProvenEmptyAuthorizedTargets {
    const fn new() -> Self {
        Self { private: () }
    }

    const fn is_empty(&self) -> bool {
        true
    }
}

pub(crate) struct ProvenEmptyProjection {
    host: ProjectionlessHostProjection,
    request: NavigationRequestId,
    authority_graph: BackNoTargetAuthorityGraphProjection,
    attempt_owner_inventory: ProvenEmptyAttemptOwnerInventory,
    current_claims: Vec<ProvenEmptyCurrentClaimProjection>,
    observed_relationship_closure: Vec<bevy_ecs::entity::Entity>,
    protected: Vec<ProvenEmptyProtectedEntityProjection>,
    authorized_targets: ProvenEmptyAuthorizedTargets,
}

impl ProvenEmptyProjection {
    pub(crate) fn is_canonical_zero(&self) -> bool {
        self.request == self.attempt_owner_inventory.request
            && self.attempt_owner_inventory.is_vacant()
            && self.authorized_targets.is_empty()
    }

    fn canonical_manifest_commitment(&self) -> Option<(u64, usize)> {
        self.is_canonical_zero()
            .then(|| (canonical_zero_manifest_digest(), 0))
    }
}

pub(crate) struct BackNoTargetProvenEmptyOwner {
    owner: BackPreparationOwner,
    projection: ProvenEmptyProjection,
}

pub(crate) struct PreparedBackNoTarget {
    approved: ApprovedBackNoTargetCandidate,
    capacity: NoTargetCapacityPermit,
    projection: ProvenEmptyProjection,
}

pub(crate) enum BackNoTargetProvenEmptyApprovalOutcome {
    Prepared(PreparedBackNoTarget),
    ReceiverMismatch {
        owner: BackNoTargetProvenEmptyOwner,
        mismatch: PreparationReceiverMismatchView,
    },
}

pub(crate) enum BackNoTargetFinalizationOutcome {
    Published,
    CapacityRejected {
        owner: PreparedBackNoTarget,
        error: BackCapacityRetireError,
    },
    ReceiverMismatch {
        owner: PreparedBackNoTarget,
        mismatch: PreparationReceiverMismatchView,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BackNoTargetAuthorityGraphFrameProjection {
    frame: RouteFrameVersion,
    stored_shell: bevy_ecs::entity::Entity,
    mount: FrameMountBinding,
    claims: Vec<ExactRegistryClaim>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BackNoTargetAuthorityGraphSlotProjection {
    claim: ExactRegistryClaim,
    binding: ProvenEmptyChildSlotBindingProjection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BackNoTargetAuthorityGraphIncomingProjection {
    frame: RouteFrameVersion,
    incoming: Vec<ExactRegistryClaim>,
}

struct BackNoTargetAuthorityGraphFrameRow {
    frame: BackNoTargetAuthorityGraphFrameProjection,
    incoming: BackNoTargetAuthorityGraphIncomingProjection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BackNoTargetAuthorityGraphProjection {
    host: ProjectionlessHostProjection,
    frames: Vec<BackNoTargetAuthorityGraphFrameProjection>,
    slots: Vec<BackNoTargetAuthorityGraphSlotProjection>,
    observed_roots: Vec<RouteFrameVersion>,
    observed_incoming: Vec<BackNoTargetAuthorityGraphIncomingProjection>,
    verified_reachable_from_root: Vec<RouteFrameVersion>,
}

impl BackNoTargetAuthorityGraphProjection {
    fn root(&self) -> Option<&RouteFrameVersion> {
        (self.observed_roots.len() == 1).then(|| &self.observed_roots[0])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum BackNoTargetAuthorityGraphViolation {
    RootCount {
        observed_roots: Vec<RouteFrameVersion>,
    },
    DuplicateIncomingSlot {
        slot: ExactRegistryClaim,
        observed_children: Vec<RouteFrameVersion>,
    },
    DuplicateFrameIdentity {
        left: BackNoTargetAuthorityGraphFrameProjection,
        right: BackNoTargetAuthorityGraphFrameProjection,
    },
    ForeignFrameAuthority {
        frame: RouteFrameVersion,
    },
    FrameClaimBoundaryDrift {
        frame: RouteFrameVersion,
        expected_claim: ExactRegistryClaim,
        expected_parent: bevy_ecs::entity::Entity,
        observed: BackNoTargetAuthorityGraphObservedClaimProjection,
    },
    FrameShellBindingDrift {
        frame: RouteFrameVersion,
        stored_shell: bevy_ecs::entity::Entity,
        owner_shell_claims: Vec<ExactRegistryClaim>,
    },
    FrameShellMountBoundaryDrift {
        frame: RouteFrameVersion,
        mount_slot: bevy_ecs::entity::Entity,
        stored_parent: bevy_ecs::entity::Entity,
        observed_shell: BackNoTargetAuthorityGraphObservedClaimProjection,
    },
    DuplicateClaim {
        left: ExactRegistryClaim,
        right: ExactRegistryClaim,
        conflicts: Vec<BackNoTargetAuthorityGraphClaimConflict>,
    },
    RootMountMismatch {
        root: RouteFrameVersion,
    },
    SlotBoundaryDrift {
        expected: BackNoTargetAuthorityGraphSlotProjection,
        observed: BackNoTargetAuthorityGraphObservedSlotProjection,
    },
    IncomingCount {
        frame: RouteFrameVersion,
        observed: Vec<ExactRegistryClaim>,
    },
    Cycle {
        observed: Vec<RouteFrameVersion>,
    },
    Unreachable {
        root: RouteFrameVersion,
        observed: Vec<RouteFrameVersion>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BackNoTargetAuthorityGraphClaimConflict {
    Entity,
    Seal,
    Owner,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BackNoTargetAuthorityGraphEvidence {
    projection: BackNoTargetAuthorityGraphProjection,
    violation: BackNoTargetAuthorityGraphViolation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BackNoTargetAuthorityGraphObservedClaimProjection {
    entity_exists: bool,
    registry_claim: Option<ExactRegistryClaim>,
    registry_manifest: Option<crate::registry::ComponentManifest>,
    physical_manifest: Option<crate::registry::ComponentManifest>,
    ownership_seal: Option<RouteOwnershipSeal>,
    frame_lifecycle: Option<FrameLifecycle>,
    child_slot_lifecycle: Option<ChildSlotLifecycle>,
    child_slot_authority: Option<ChildSlotAuthority>,
    provisional_slot_lifecycle: Option<ProvisionalSlotLifecycle>,
    observed_parent: Option<bevy_ecs::entity::Entity>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BackNoTargetAuthorityGraphObservedOutletProjection {
    parent: RouteFrameVersion,
    outlet: bevy_ui_router_core::slot::RouteSlotKey,
    anchor: bevy_ecs::entity::Entity,
    anchor_binding: crate::presentation::OutletBinding,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BackNoTargetAuthorityGraphObservedSlotProjection {
    slot: BackNoTargetAuthorityGraphObservedClaimProjection,
    anchor: BackNoTargetAuthorityGraphObservedClaimProjection,
    anchor_source: BackNoTargetAuthorityGraphObservedClaimProjection,
    anchor_binding_current: bool,
    owner_outlets: Vec<BackNoTargetAuthorityGraphObservedOutletProjection>,
    observed_anchor_children: Vec<bevy_ecs::entity::Entity>,
}

fn observe_back_no_target_authority_graph_claim(
    world: &World,
    registry: &RouteEntityRegistry,
    entity: bevy_ecs::entity::Entity,
) -> BackNoTargetAuthorityGraphObservedClaimProjection {
    BackNoTargetAuthorityGraphObservedClaimProjection {
        entity_exists: world.get_entity(entity).is_ok(),
        registry_claim: registry.record(entity).map(|record| record.claim()),
        registry_manifest: registry
            .record(entity)
            .map(|record| record.manifest().clone()),
        physical_manifest: crate::registry::ComponentManifest::from_world(world, entity),
        ownership_seal: world.get::<RouteOwnershipSeal>(entity).copied(),
        frame_lifecycle: world.get::<FrameLifecycle>(entity).copied(),
        child_slot_lifecycle: world.get::<ChildSlotLifecycle>(entity).copied(),
        child_slot_authority: world.get::<ChildSlotAuthority>(entity).cloned(),
        provisional_slot_lifecycle: world.get::<ProvisionalSlotLifecycle>(entity).copied(),
        observed_parent: world
            .get::<bevy_ecs::hierarchy::ChildOf>(entity)
            .map(bevy_ecs::hierarchy::ChildOf::parent),
    }
}

fn observe_back_no_target_authority_graph_slot(
    world: &World,
    registry: &RouteEntityRegistry,
    active: &RealizedBranch,
    slot: &crate::mount::MountedChildSlot,
) -> BackNoTargetAuthorityGraphObservedSlotProjection {
    let mut owner_outlets = active
        .frames()
        .iter()
        .filter(|frame| frame.frame() == slot.parent())
        .flat_map(|frame| frame.outlets())
        .map(
            |outlet| BackNoTargetAuthorityGraphObservedOutletProjection {
                parent: outlet.parent().clone(),
                outlet: outlet.key().clone(),
                anchor: outlet.anchor(),
                anchor_binding: outlet.anchor_binding().clone(),
            },
        )
        .collect::<Vec<_>>();
    owner_outlets.sort_by(|left, right| {
        left.outlet
            .cmp(&right.outlet)
            .then_with(|| left.anchor.to_bits().cmp(&right.anchor.to_bits()))
            .then_with(|| {
                left.anchor_binding
                    .anchor()
                    .to_bits()
                    .cmp(&right.anchor_binding.anchor().to_bits())
            })
    });
    let mut observed_anchor_children = world
        .get::<bevy_ecs::hierarchy::Children>(slot.anchor())
        .map(|children| children.iter().copied().collect::<Vec<_>>())
        .unwrap_or_default();
    observed_anchor_children.sort_by_key(|entity| entity.to_bits());
    BackNoTargetAuthorityGraphObservedSlotProjection {
        slot: observe_back_no_target_authority_graph_claim(world, registry, slot.slot()),
        anchor: observe_back_no_target_authority_graph_claim(world, registry, slot.anchor()),
        anchor_source: observe_back_no_target_authority_graph_claim(
            world,
            registry,
            slot.anchor_claim().entity(),
        ),
        anchor_binding_current: slot.anchor_binding().is_current(world, slot.parent(), true),
        owner_outlets,
        observed_anchor_children,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BackNoTargetRegisteredExtraIndeterminateCause {
    RegistryRecordDrift,
    PhysicalManifestDrift,
    OwnershipSealDrift,
    RoleLifecycleDrift,
    OpaqueAuthorityRelationUnavailable,
}

enum BackNoTargetRegisteredExtraAuthoritySource<'a> {
    Frame(&'a RouteFrameVersion),
    Host(&'a bevy_ui_router_core::HostVersion),
    Unavailable,
}

fn back_no_target_registered_extra_authority_source(
    owner: &RegistryOwner,
) -> BackNoTargetRegisteredExtraAuthoritySource<'_> {
    match owner {
        RegistryOwner::Committed(
            EntityOwner::FrameShell(frame)
            | EntityOwner::FrameContentTarget(frame)
            | EntityOwner::FrameOutput { frame, .. },
        )
        | RegistryOwner::Committed(EntityOwner::ChildSlot { parent: frame, .. })
        | RegistryOwner::Provisional(ProvisionalOwner::ChildSlot { parent: frame, .. }) => {
            BackNoTargetRegisteredExtraAuthoritySource::Frame(frame)
        }
        RegistryOwner::Committed(EntityOwner::HostTopSlot { host, .. }) => {
            BackNoTargetRegisteredExtraAuthoritySource::Host(host)
        }
        RegistryOwner::Provisional(ProvisionalOwner::HostTopSlot(_)) => {
            BackNoTargetRegisteredExtraAuthoritySource::Unavailable
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum BackNoTargetRegisteredExtraExpectedRole {
    Frame {
        lifecycle: FrameLifecycle,
    },
    ChildSlot {
        lifecycle: ChildSlotLifecycle,
        authority: ChildSlotAuthority,
    },
    HostTop {
        lifecycle: EntityLifecycle,
        candidate_revision: Option<crate::host::HostCandidateRevision>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum BackNoTargetRegisteredExtraObservedRole {
    Frame {
        lifecycle: Option<FrameLifecycle>,
    },
    ChildSlot {
        lifecycle: Option<ChildSlotLifecycle>,
        authority: Option<ChildSlotAuthority>,
    },
    HostTop {
        lifecycle: Option<ProvisionalSlotLifecycle>,
        candidate_revision: Option<crate::host::HostCandidateRevision>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BackNoTargetRegisteredExtraExpectedProjection {
    claim: ExactRegistryClaim,
    registered_manifest: crate::registry::ComponentManifest,
    ownership_seal: RouteOwnershipSeal,
    role: BackNoTargetRegisteredExtraExpectedRole,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BackNoTargetRegisteredExtraObservedProjection {
    entity_exists: bool,
    registry_claim: Option<ExactRegistryClaim>,
    registry_manifest: Option<crate::registry::ComponentManifest>,
    physical_manifest: Option<crate::registry::ComponentManifest>,
    ownership_seal: Option<RouteOwnershipSeal>,
    role: BackNoTargetRegisteredExtraObservedRole,
    observed_parent: Option<bevy_ecs::entity::Entity>,
}

fn expected_registered_extra_role(
    claim: &ExactRegistryClaim,
) -> BackNoTargetRegisteredExtraExpectedRole {
    match claim.owner() {
        RegistryOwner::Committed(
            EntityOwner::FrameShell(_)
            | EntityOwner::FrameContentTarget(_)
            | EntityOwner::FrameOutput { .. },
        ) => BackNoTargetRegisteredExtraExpectedRole::Frame {
            lifecycle: FrameLifecycle(claim.lifecycle()),
        },
        RegistryOwner::Committed(EntityOwner::ChildSlot {
            parent,
            outlet,
            slot,
        })
        | RegistryOwner::Provisional(ProvisionalOwner::ChildSlot {
            parent,
            outlet,
            slot,
        }) => {
            let lifecycle = match (claim.owner(), claim.lifecycle()) {
                (
                    RegistryOwner::Provisional(ProvisionalOwner::ChildSlot { .. }),
                    EntityLifecycle::PreparedInert,
                ) => ChildSlotLifecycle::ProvisionalInert,
                (
                    RegistryOwner::Committed(EntityOwner::ChildSlot { .. }),
                    EntityLifecycle::PreparedInert,
                ) => ChildSlotLifecycle::RegisteredInert,
                (_, EntityLifecycle::Active) => ChildSlotLifecycle::Active,
                (_, EntityLifecycle::Leaving) => ChildSlotLifecycle::Leaving,
                (_, EntityLifecycle::Quarantined) => ChildSlotLifecycle::Quarantined,
                _ => unreachable!("the outer owner match admits only child-slot owners"),
            };
            BackNoTargetRegisteredExtraExpectedRole::ChildSlot {
                lifecycle,
                authority: ChildSlotAuthority::new(parent.clone(), outlet.clone(), *slot),
            }
        }
        RegistryOwner::Committed(EntityOwner::HostTopSlot { .. }) => {
            BackNoTargetRegisteredExtraExpectedRole::HostTop {
                lifecycle: claim.lifecycle(),
                candidate_revision: None,
            }
        }
        RegistryOwner::Provisional(ProvisionalOwner::HostTopSlot(owner)) => {
            BackNoTargetRegisteredExtraExpectedRole::HostTop {
                lifecycle: claim.lifecycle(),
                candidate_revision: Some(owner.candidate_revision()),
            }
        }
    }
}

fn freeze_back_no_target_registered_extra(
    world: &World,
    registry: &RouteEntityRegistry,
    claim: &ExactRegistryClaim,
    registered_manifest: crate::registry::ComponentManifest,
) -> Result<
    (
        BackNoTargetRegisteredExtraExpectedProjection,
        BackNoTargetRegisteredExtraObservedProjection,
    ),
    (
        BackNoTargetRegisteredExtraIndeterminateCause,
        BackNoTargetRegisteredExtraExpectedProjection,
        BackNoTargetRegisteredExtraObservedProjection,
    ),
> {
    let expected = BackNoTargetRegisteredExtraExpectedProjection {
        claim: claim.clone(),
        registered_manifest,
        ownership_seal: RouteOwnershipSeal::new(claim.seal()),
        role: expected_registered_extra_role(claim),
    };
    let observed_record = registry.record(claim.entity());
    let observed_claim = observed_record.map(|record| record.claim());
    let observed_role = match &expected.role {
        BackNoTargetRegisteredExtraExpectedRole::Frame { .. } => {
            BackNoTargetRegisteredExtraObservedRole::Frame {
                lifecycle: world.get::<FrameLifecycle>(claim.entity()).copied(),
            }
        }
        BackNoTargetRegisteredExtraExpectedRole::ChildSlot { .. } => {
            BackNoTargetRegisteredExtraObservedRole::ChildSlot {
                lifecycle: world.get::<ChildSlotLifecycle>(claim.entity()).copied(),
                authority: world.get::<ChildSlotAuthority>(claim.entity()).cloned(),
            }
        }
        BackNoTargetRegisteredExtraExpectedRole::HostTop { .. } => {
            let candidate_revision =
                observed_claim
                    .as_ref()
                    .and_then(|claim| match claim.owner() {
                        RegistryOwner::Provisional(ProvisionalOwner::HostTopSlot(owner)) => {
                            Some(owner.candidate_revision())
                        }
                        _ => None,
                    });
            BackNoTargetRegisteredExtraObservedRole::HostTop {
                lifecycle: world
                    .get::<ProvisionalSlotLifecycle>(claim.entity())
                    .copied(),
                candidate_revision,
            }
        }
    };
    let observed = BackNoTargetRegisteredExtraObservedProjection {
        entity_exists: world.get_entity(claim.entity()).is_ok(),
        registry_claim: observed_claim,
        registry_manifest: observed_record.map(|record| record.manifest().clone()),
        physical_manifest: crate::registry::ComponentManifest::from_world(world, claim.entity()),
        ownership_seal: world.get::<RouteOwnershipSeal>(claim.entity()).copied(),
        role: observed_role,
        observed_parent: world
            .get::<bevy_ecs::hierarchy::ChildOf>(claim.entity())
            .map(bevy_ecs::hierarchy::ChildOf::parent),
    };
    if observed.registry_claim.as_ref() != Some(&expected.claim)
        || observed.registry_manifest.as_ref() != Some(&expected.registered_manifest)
    {
        return Err((
            BackNoTargetRegisteredExtraIndeterminateCause::RegistryRecordDrift,
            expected,
            observed,
        ));
    }
    if observed.physical_manifest.as_ref().is_none_or(|physical| {
        expected
            .registered_manifest
            .component_ids()
            .iter()
            .any(|component| !physical.component_ids().contains(component))
    }) {
        return Err((
            BackNoTargetRegisteredExtraIndeterminateCause::PhysicalManifestDrift,
            expected,
            observed,
        ));
    }
    if observed.ownership_seal != Some(expected.ownership_seal) {
        return Err((
            BackNoTargetRegisteredExtraIndeterminateCause::OwnershipSealDrift,
            expected,
            observed,
        ));
    }
    let owner_lifecycle_is_legal = match (claim.owner(), claim.lifecycle()) {
        (
            RegistryOwner::Provisional(ProvisionalOwner::ChildSlot { .. }),
            EntityLifecycle::PreparedInert | EntityLifecycle::Quarantined,
        ) => true,
        (RegistryOwner::Provisional(ProvisionalOwner::ChildSlot { .. }), _) => false,
        (RegistryOwner::Committed(EntityOwner::ChildSlot { .. }), _) => true,
        (
            RegistryOwner::Committed(EntityOwner::HostTopSlot { .. })
            | RegistryOwner::Provisional(ProvisionalOwner::HostTopSlot(_)),
            EntityLifecycle::PreparedInert | EntityLifecycle::Quarantined,
        ) => true,
        (
            RegistryOwner::Committed(EntityOwner::HostTopSlot { .. })
            | RegistryOwner::Provisional(ProvisionalOwner::HostTopSlot(_)),
            _,
        ) => false,
        (
            RegistryOwner::Committed(
                EntityOwner::FrameShell(_)
                | EntityOwner::FrameContentTarget(_)
                | EntityOwner::FrameOutput { .. },
            ),
            _,
        ) => true,
    };
    if !owner_lifecycle_is_legal {
        return Err((
            BackNoTargetRegisteredExtraIndeterminateCause::RoleLifecycleDrift,
            expected,
            observed,
        ));
    }
    let role_is_exact = match (&expected.role, &observed.role) {
        (
            BackNoTargetRegisteredExtraExpectedRole::Frame {
                lifecycle: expected,
            },
            BackNoTargetRegisteredExtraObservedRole::Frame {
                lifecycle: observed,
            },
        ) => observed.as_ref() == Some(expected),
        (
            BackNoTargetRegisteredExtraExpectedRole::ChildSlot {
                lifecycle: expected_lifecycle,
                authority: expected_authority,
            },
            BackNoTargetRegisteredExtraObservedRole::ChildSlot {
                lifecycle: observed_lifecycle,
                authority: observed_authority,
            },
        ) => {
            observed_lifecycle.as_ref() == Some(expected_lifecycle)
                && observed_authority.as_ref() == Some(expected_authority)
        }
        (
            BackNoTargetRegisteredExtraExpectedRole::HostTop {
                lifecycle,
                candidate_revision: expected_revision,
            },
            BackNoTargetRegisteredExtraObservedRole::HostTop {
                lifecycle: observed_lifecycle,
                candidate_revision: observed_revision,
            },
        ) => {
            matches!(
                (lifecycle, observed_lifecycle),
                (
                    EntityLifecycle::PreparedInert,
                    Some(ProvisionalSlotLifecycle::PreparedInert)
                ) | (
                    EntityLifecycle::Leaving,
                    Some(ProvisionalSlotLifecycle::Leaving)
                ) | (
                    EntityLifecycle::Quarantined,
                    Some(ProvisionalSlotLifecycle::Quarantined)
                )
            ) && expected_revision == observed_revision
        }
        _ => false,
    };
    if !role_is_exact {
        return Err((
            BackNoTargetRegisteredExtraIndeterminateCause::RoleLifecycleDrift,
            expected,
            observed,
        ));
    }
    Ok((expected, observed))
}

#[derive(Debug)]
enum BackNoTargetPhysicalIndeterminateReason {
    MissingRegistry,
    MissingHost,
    MissingActive,
    RequestIndexDrift,
    Capacity(BackCapacityRetireError),
    ConflictingAttemptOwners(ProvenEmptyAttemptOwnerInventory),
    HostAuthorityDrift,
    ActiveTopologyDrift,
    AuthorityManifest,
    AuthorityLedger,
    AuthorityIsolation,
    ActiveRegistryDrift {
        expected: Vec<ExactRegistryClaim>,
        observed: Vec<ExactRegistryClaim>,
    },
    DuplicateProtectedEntity(bevy_ecs::entity::Entity),
    RelationshipClosureDrift {
        observed: Vec<bevy_ecs::entity::Entity>,
        protected: Vec<ProvenEmptyProtectedEntityProjection>,
    },
    UnexpectedSameAuthorityDescendant {
        entity: bevy_ecs::entity::Entity,
        expected: BackNoTargetRegisteredExtraExpectedProjection,
        observed: BackNoTargetRegisteredExtraObservedProjection,
    },
    RegisteredExtra {
        entity: bevy_ecs::entity::Entity,
        cause: BackNoTargetRegisteredExtraIndeterminateCause,
        expected: BackNoTargetRegisteredExtraExpectedProjection,
        observed: BackNoTargetRegisteredExtraObservedProjection,
    },
    AuthorityGraph(BackNoTargetAuthorityGraphEvidence),
}

pub(crate) struct BackNoTargetPhysicalIndeterminateEvidence {
    request: NavigationRequestId,
    reason: BackNoTargetPhysicalIndeterminateReason,
}

enum BackNoTargetAuthorityPermanentEvidence {
    Manifest(AuthoritySettlementManifestError),
    Partition {
        manifest: FrozenCleanupManifest,
    },
    Isolation {
        manifest: FrozenCleanupManifest,
        contradiction: AuthorityIsolationContradiction,
    },
    Ledger {
        kind: CleanupLedgerReserveErrorKind,
        manifest: FrozenCleanupManifest,
    },
    ActiveOwnerDrift {
        observed_frames: Vec<RouteFrameVersion>,
        observed_claims: Vec<ExactRegistryClaim>,
    },
}

pub(crate) struct BackNoTargetAuthorityPermanentOwner {
    owner: BackPreparationOwner,
    source: FrozenBackNoTargetAuthoritySource,
    partition: Option<FrozenBackNoTargetAuthorityPartition>,
    isolation: Option<PreparedAuthorityIsolation>,
    reserved: Option<PreparedCleanupLedger>,
    evidence: BackNoTargetAuthorityPermanentEvidence,
}

pub(crate) enum BackNoTargetPhysicalFreezeOutcome {
    ProvenEmpty(BackNoTargetProvenEmptyOwner),
    Indeterminate {
        owner: BackPreparationOwner,
        evidence: BackNoTargetPhysicalIndeterminateEvidence,
    },
}

pub(crate) enum BackNoTargetPreparationOutcome {
    Ready(PreparedBackNoTarget),
    AuthoritySettlement(BackNoTargetAuthoritySettlementDisposition),
    AuthorityPermanent(BackNoTargetAuthorityPermanentOwner),
    Retry {
        owner: BackPreparationOwner,
        evidence: BackNoTargetPhysicalIndeterminateEvidence,
    },
    CapacityRejected {
        owner: BackPreparationOwner,
        error: BackCapacityRetireError,
    },
    ReceiverMismatch {
        owner: BackPreparationOwner,
        mismatch: PreparationReceiverMismatchView,
    },
}

fn back_no_target_logical_frame_identity_key(
    version: &RouteFrameVersion,
) -> (u64, u64, u64, u8, u64, u64, u64) {
    let (origin_tag, origin_route) = match version.origin() {
        bevy_ui_router_core::prelude::FrameOrigin::MatchedRoute(route) => (0, route.raw()),
        bevy_ui_router_core::prelude::FrameOrigin::RouteError(route) => (1, route.raw()),
        bevy_ui_router_core::prelude::FrameOrigin::RouterFallback => (2, 0),
    };
    (
        version.router().raw(),
        version.host().raw(),
        version.slot().raw(),
        origin_tag,
        origin_route,
        version.frame().raw(),
        version.generation().raw(),
    )
}

fn classify_back_no_target_authority_topology(
    root_index: usize,
    adjacency: &[Vec<usize>],
) -> Result<Vec<bool>, Vec<usize>> {
    fn visit(
        node: usize,
        adjacency: &[Vec<usize>],
        states: &mut [u8],
        stack: &mut Vec<usize>,
    ) -> Option<Vec<usize>> {
        states[node] = 1;
        stack.push(node);
        for child in adjacency[node].iter().copied() {
            if states[child] == 1 {
                let start = stack
                    .iter()
                    .position(|current| *current == child)
                    .unwrap_or(0);
                return Some(
                    stack[start..]
                        .iter()
                        .copied()
                        .chain(std::iter::once(child))
                        .collect(),
                );
            }
            if states[child] == 0 {
                if let Some(cycle) = visit(child, adjacency, states, stack) {
                    return Some(cycle);
                }
            }
        }
        stack.pop();
        states[node] = 2;
        None
    }

    assert!(
        root_index < adjacency.len(),
        "authority topology root must index one adjacency row"
    );
    let mut states = vec![0; adjacency.len()];
    if let Some(cycle) = visit(root_index, adjacency, &mut states, &mut Vec::new()) {
        return Err(cycle);
    }
    let reachable_from_root = states.iter().map(|state| *state != 0).collect::<Vec<_>>();
    for index in 0..states.len() {
        if states[index] == 0 {
            if let Some(cycle) = visit(index, adjacency, &mut states, &mut Vec::new()) {
                return Err(cycle);
            }
        }
    }
    Ok(reachable_from_root)
}

fn freeze_back_no_target_authority_graph(
    world: &World,
    registry: &RouteEntityRegistry,
    host: &HostBinding,
    active: &RealizedBranch,
) -> Result<BackNoTargetAuthorityGraphProjection, BackNoTargetAuthorityGraphEvidence> {
    let project_frame = |frame: &RealizedFrame| {
        let mut claims = frame.claims().cloned().collect::<Vec<_>>();
        claims.sort_by_key(|claim| claim.entity().to_bits());
        BackNoTargetAuthorityGraphFrameProjection {
            frame: frame.frame().clone(),
            stored_shell: frame.shell(),
            mount: frame.mount_binding().clone(),
            claims,
        }
    };
    let mut frame_rows = active
        .frames()
        .iter()
        .map(|frame| {
            let frame_projection = project_frame(frame);
            let mut incoming = active
                .child_slots()
                .iter()
                .filter(|slot| frame.mount_binding().matches_mounted_child_slot(slot))
                .map(|slot| slot.slot_claim().clone())
                .collect::<Vec<_>>();
            incoming.sort_by_key(|claim| claim.entity().to_bits());
            BackNoTargetAuthorityGraphFrameRow {
                incoming: BackNoTargetAuthorityGraphIncomingProjection {
                    frame: frame.frame().clone(),
                    incoming,
                },
                frame: frame_projection,
            }
        })
        .collect::<Vec<_>>();
    frame_rows.sort_by(|left, right| {
        back_no_target_logical_frame_identity_key(&left.frame.frame)
            .cmp(&back_no_target_logical_frame_identity_key(
                &right.frame.frame,
            ))
            .then_with(|| {
                left.frame
                    .stored_shell
                    .to_bits()
                    .cmp(&right.frame.stored_shell.to_bits())
            })
            .then_with(|| {
                left.frame
                    .mount
                    .slot()
                    .to_bits()
                    .cmp(&right.frame.mount.slot().to_bits())
            })
            .then_with(|| {
                left.frame
                    .claims
                    .iter()
                    .map(|claim| claim.entity().to_bits())
                    .cmp(
                        right
                            .frame
                            .claims
                            .iter()
                            .map(|claim| claim.entity().to_bits()),
                    )
            })
    });
    let (frames, observed_incoming): (Vec<_>, Vec<_>) = frame_rows
        .into_iter()
        .map(|row| (row.frame, row.incoming))
        .unzip();
    let mut slots = active
        .child_slots()
        .iter()
        .map(|slot| {
            let mut expected_support_relationships = slot.current_anchor_children(world);
            expected_support_relationships.sort_by_key(|entity| entity.to_bits());
            BackNoTargetAuthorityGraphSlotProjection {
                claim: slot.slot_claim().clone(),
                binding: ProvenEmptyChildSlotBindingProjection {
                    parent: slot.parent().clone(),
                    outlet: slot.outlet().clone(),
                    slot_id: slot.slot_id(),
                    anchor_binding: slot.anchor_binding().clone(),
                    expected_support_relationships,
                },
            }
        })
        .collect::<Vec<_>>();
    slots.sort_by_key(|slot| slot.claim.entity().to_bits());
    let observed_roots = observed_incoming
        .iter()
        .filter(|incoming| incoming.incoming.is_empty())
        .map(|incoming| incoming.frame.clone())
        .collect::<Vec<_>>();
    let mut projection = BackNoTargetAuthorityGraphProjection {
        host: ProjectionlessHostProjection::from(host),
        frames,
        slots,
        observed_roots,
        observed_incoming,
        verified_reachable_from_root: Vec::new(),
    };
    let reject = |violation| BackNoTargetAuthorityGraphEvidence {
        projection: projection.clone(),
        violation,
    };

    for slot in active.child_slots() {
        let mut observed_children = active
            .frames()
            .iter()
            .filter(|frame| frame.mount_binding().matches_mounted_child_slot(slot))
            .map(|frame| frame.frame().clone())
            .collect::<Vec<_>>();
        observed_children.sort_by_key(back_no_target_logical_frame_identity_key);
        if observed_children.len() > 1 {
            return Err(reject(
                BackNoTargetAuthorityGraphViolation::DuplicateIncomingSlot {
                    slot: slot.slot_claim().clone(),
                    observed_children,
                },
            ));
        }
    }
    let Some(root_version) = projection.root().cloned() else {
        return Err(reject(BackNoTargetAuthorityGraphViolation::RootCount {
            observed_roots: projection.observed_roots.clone(),
        }));
    };
    let Some(root_index) = active
        .frames()
        .iter()
        .position(|frame| frame.frame() == &root_version)
    else {
        return Err(reject(BackNoTargetAuthorityGraphViolation::RootCount {
            observed_roots: projection.observed_roots.clone(),
        }));
    };
    let root = &active.frames()[root_index];

    for (index, frame) in active.frames().iter().enumerate() {
        if let Some(other) = active.frames()[index + 1..].iter().find(|other| {
            other.frame() == frame.frame() || other.frame().frame() == frame.frame().frame()
        }) {
            return Err(reject(
                BackNoTargetAuthorityGraphViolation::DuplicateFrameIdentity {
                    left: project_frame(frame),
                    right: project_frame(other),
                },
            ));
        }
        if !bevy_ui_router_core::integration::frame_versions_share_router(
            &root_version,
            frame.frame(),
        ) || !bevy_ui_router_core::integration::host_version_matches_frame(
            &host.version,
            frame.frame(),
        ) {
            return Err(reject(
                BackNoTargetAuthorityGraphViolation::ForeignFrameAuthority {
                    frame: frame.frame().clone(),
                },
            ));
        }
        let mut owner_shell_claims = frame
            .claims()
            .filter(|claim| {
                matches!(
                    claim.owner(),
                    RegistryOwner::Committed(EntityOwner::FrameShell(owner))
                        if owner == frame.frame()
                )
            })
            .cloned()
            .collect::<Vec<_>>();
        owner_shell_claims.sort_by_key(|claim| claim.entity().to_bits());
        if owner_shell_claims.len() != 1 || owner_shell_claims[0].entity() != frame.shell() {
            return Err(reject(
                BackNoTargetAuthorityGraphViolation::FrameShellBindingDrift {
                    frame: frame.frame().clone(),
                    stored_shell: frame.shell(),
                    owner_shell_claims,
                },
            ));
        }
        let shell_claim = &owner_shell_claims[0];
        let Some(stored_shell_parent) = frame
            .cleanup_members()
            .find(|(claim, _)| *claim == shell_claim)
            .map(|(_, parent)| parent)
        else {
            return Err(reject(
                BackNoTargetAuthorityGraphViolation::FrameShellBindingDrift {
                    frame: frame.frame().clone(),
                    stored_shell: frame.shell(),
                    owner_shell_claims,
                },
            ));
        };
        let mount_slot = frame.mount_binding().slot();
        let observed_shell =
            observe_back_no_target_authority_graph_claim(world, registry, shell_claim.entity());
        if stored_shell_parent != mount_slot || observed_shell.observed_parent != Some(mount_slot) {
            return Err(reject(
                BackNoTargetAuthorityGraphViolation::FrameShellMountBoundaryDrift {
                    frame: frame.frame().clone(),
                    mount_slot,
                    stored_parent: stored_shell_parent,
                    observed_shell,
                },
            ));
        }
        for (claim, parent) in frame.cleanup_members() {
            let role_is_exact = matches!(
                claim.owner(),
                RegistryOwner::Committed(
                    EntityOwner::FrameShell(owner)
                        | EntityOwner::FrameContentTarget(owner)
                        | EntityOwner::FrameOutput { frame: owner, .. }
                ) if owner == frame.frame()
            );
            let record_is_exact = registry.record(claim.entity()).is_some_and(|record| {
                record.claim() == *claim
                    && crate::registry::ComponentManifest::from_world(world, claim.entity())
                        .is_some_and(|actual| {
                            record
                                .manifest()
                                .component_ids()
                                .iter()
                                .all(|component| actual.component_ids().contains(component))
                        })
            });
            if claim.lifecycle() != EntityLifecycle::Active
                || !role_is_exact
                || !record_is_exact
                || !cleanup_claim_has_exact_authority(world, claim)
                || world
                    .get::<bevy_ecs::hierarchy::ChildOf>(claim.entity())
                    .map(bevy_ecs::hierarchy::ChildOf::parent)
                    != Some(parent)
            {
                return Err(reject(
                    BackNoTargetAuthorityGraphViolation::FrameClaimBoundaryDrift {
                        frame: frame.frame().clone(),
                        expected_claim: claim.clone(),
                        expected_parent: parent,
                        observed: observe_back_no_target_authority_graph_claim(
                            world,
                            registry,
                            claim.entity(),
                        ),
                    },
                ));
            }
        }
    }

    if root.frame().slot() != host.slot_id
        || root.mount_binding().slot() != host.top_slot
        || root.mount_binding().slot_claim() != &host.top_slot_claim
        || root.mount_binding().physical_parent() != (host.host, None)
        || !matches!(
            host.top_slot_claim.owner(),
            RegistryOwner::Committed(EntityOwner::HostTopSlot {
                host: version,
                slot,
            }) if version == &host.version && *slot == host.slot_id
        )
    {
        return Err(reject(
            BackNoTargetAuthorityGraphViolation::RootMountMismatch {
                root: root_version.clone(),
            },
        ));
    }

    let mut claims = active
        .frames()
        .iter()
        .flat_map(RealizedFrame::claims)
        .chain(active.child_slots().iter().map(|slot| slot.slot_claim()))
        .collect::<Vec<_>>();
    claims.push(&host.top_slot_claim);
    for (index, claim) in claims.iter().enumerate() {
        if let Some(other) = claims[index + 1..].iter().find(|other| {
            other.entity() == claim.entity()
                || other.seal() == claim.seal()
                || other.owner().conflicts_with(claim.owner())
        }) {
            let mut conflicts = Vec::new();
            if other.entity() == claim.entity() {
                conflicts.push(BackNoTargetAuthorityGraphClaimConflict::Entity);
            }
            if other.seal() == claim.seal() {
                conflicts.push(BackNoTargetAuthorityGraphClaimConflict::Seal);
            }
            if other.owner().conflicts_with(claim.owner()) {
                conflicts.push(BackNoTargetAuthorityGraphClaimConflict::Owner);
            }
            return Err(reject(
                BackNoTargetAuthorityGraphViolation::DuplicateClaim {
                    left: (*claim).clone(),
                    right: (**other).clone(),
                    conflicts,
                },
            ));
        }
    }
    for (index, frame) in active.frames().iter().enumerate() {
        if active.frames()[index + 1..].iter().any(|other| {
            frame
                .mount_binding()
                .same_physical_mount(other.mount_binding())
        }) {
            let observed = projection
                .observed_incoming
                .iter()
                .find(|incoming| incoming.frame == *frame.frame())
                .map(|incoming| incoming.incoming.clone())
                .unwrap_or_default();
            return Err(reject(BackNoTargetAuthorityGraphViolation::IncomingCount {
                frame: frame.frame().clone(),
                observed,
            }));
        }
    }

    let mut adjacency = vec![Vec::new(); active.frames().len()];
    for slot in active.child_slots() {
        let expected_slot = projection
            .slots
            .iter()
            .find(|expected| expected.claim == *slot.slot_claim())
            .cloned()
            .expect("the graph projection retains every owner-held child slot");
        let boundary_drift = || BackNoTargetAuthorityGraphViolation::SlotBoundaryDrift {
            expected: expected_slot.clone(),
            observed: observe_back_no_target_authority_graph_slot(world, registry, active, slot),
        };
        let Some(parent_index) = active
            .frames()
            .iter()
            .position(|frame| frame.frame() == slot.parent())
        else {
            return Err(reject(boundary_drift()));
        };
        let Some(parent) = active.frames().get(parent_index) else {
            return Err(reject(boundary_drift()));
        };
        let observed_children = active
            .frames()
            .iter()
            .enumerate()
            .filter(|(_, frame)| frame.mount_binding().matches_mounted_child_slot(slot))
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        let lifecycle_is_exact = match observed_children.as_slice() {
            [] => slot.slot_claim().lifecycle() == EntityLifecycle::PreparedInert,
            [_] => slot.slot_claim().lifecycle() == EntityLifecycle::Active,
            _ => false,
        };
        let observed_anchor_children = world
            .get::<bevy_ecs::hierarchy::Children>(slot.anchor())
            .map(|children| children.iter().copied().collect::<Vec<_>>())
            .unwrap_or_default();
        let slot_record_is_exact =
            registry.record(slot.slot()).is_some_and(|record| {
                record.claim() == *slot.slot_claim()
                    && crate::registry::ComponentManifest::from_world(world, slot.slot())
                        .is_some_and(|physical_manifest| {
                            record.manifest().component_ids().iter().all(|required| {
                                physical_manifest.component_ids().contains(required)
                            })
                        })
            });
        let binding_is_exact = slot.slot_claim().owner()
            == &RegistryOwner::Committed(EntityOwner::ChildSlot {
                parent: slot.parent().clone(),
                outlet: slot.outlet().clone(),
                slot: slot.slot_id(),
            })
            && lifecycle_is_exact
            && slot_record_is_exact
            && cleanup_claim_has_exact_authority(world, slot.slot_claim())
            && world
                .get::<bevy_ecs::hierarchy::ChildOf>(slot.slot())
                .map(bevy_ecs::hierarchy::ChildOf::parent)
                == Some(slot.anchor())
            && parent.outlet(slot.outlet()).is_some_and(|outlet| {
                outlet.anchor() == slot.anchor()
                    && outlet.anchor_claim() == slot.anchor_claim()
                    && parent.claims().any(|claim| claim == slot.anchor_claim())
            })
            && slot.anchor_binding().is_current(world, slot.parent(), true)
            && slot
                .current_anchor_children(world)
                .iter()
                .all(|expected| observed_anchor_children.contains(expected));
        if !binding_is_exact {
            return Err(reject(boundary_drift()));
        }
        if let Some(child) = observed_children.first().copied() {
            adjacency[parent_index].push(child);
        }
    }
    for children in &mut adjacency {
        children.sort_by_key(|index| {
            back_no_target_logical_frame_identity_key(active.frames()[*index].frame())
        });
    }
    for incoming in &projection.observed_incoming {
        let expected = usize::from(incoming.frame != root_version);
        if incoming.incoming.len() != expected {
            return Err(reject(BackNoTargetAuthorityGraphViolation::IncomingCount {
                frame: incoming.frame.clone(),
                observed: incoming.incoming.clone(),
            }));
        }
    }

    let reachable_from_root =
        match classify_back_no_target_authority_topology(root_index, &adjacency) {
            Ok(reachable) => reachable,
            Err(cycle) => {
                return Err(reject(BackNoTargetAuthorityGraphViolation::Cycle {
                    observed: cycle
                        .into_iter()
                        .map(|index| active.frames()[index].frame().clone())
                        .collect(),
                }));
            }
        };
    let mut unreachable = reachable_from_root
        .iter()
        .enumerate()
        .filter(|(_, reachable)| !**reachable)
        .map(|(index, _)| active.frames()[index].frame().clone())
        .collect::<Vec<_>>();
    unreachable.sort_by_key(back_no_target_logical_frame_identity_key);
    if !unreachable.is_empty() {
        return Err(reject(BackNoTargetAuthorityGraphViolation::Unreachable {
            root: root_version,
            observed: unreachable,
        }));
    }
    projection.verified_reachable_from_root = projection
        .frames
        .iter()
        .map(|frame| frame.frame.clone())
        .collect();
    Ok(projection)
}

pub(crate) fn freeze_back_no_target_proven_empty(
    world: &World,
    entry: &RouterEntry,
    owner: BackPreparationOwner,
) -> BackNoTargetPhysicalFreezeOutcome {
    let indeterminate = |owner: BackPreparationOwner, reason| {
        let request = owner.request;
        BackNoTargetPhysicalFreezeOutcome::Indeterminate {
            owner,
            evidence: BackNoTargetPhysicalIndeterminateEvidence { request, reason },
        }
    };
    let request = owner.request;
    let Some(registry) = world.get_resource::<RouteEntityRegistry>() else {
        return indeterminate(
            owner,
            BackNoTargetPhysicalIndeterminateReason::MissingRegistry,
        );
    };
    let Some(host) = entry.host.as_ref() else {
        return indeterminate(owner, BackNoTargetPhysicalIndeterminateReason::MissingHost);
    };
    let Some(active) = entry.active.as_ref() else {
        return indeterminate(
            owner,
            BackNoTargetPhysicalIndeterminateReason::MissingActive,
        );
    };
    if entry
        .requests
        .iter()
        .filter(|indexed| **indexed == request)
        .count()
        != 1
    {
        return indeterminate(
            owner,
            BackNoTargetPhysicalIndeterminateReason::RequestIndexDrift,
        );
    }
    let Some(issuer) = entry.back_capacity_issuer.as_ref() else {
        return indeterminate(
            owner,
            BackNoTargetPhysicalIndeterminateReason::Capacity(
                BackCapacityRetireError::StaleGeneration,
            ),
        );
    };
    if let Err(error) = issuer.validate_no_target_capacity(&owner.capacity) {
        return indeterminate(
            owner,
            BackNoTargetPhysicalIndeterminateReason::Capacity(error),
        );
    }
    let attempt_owner_inventory = ProvenEmptyAttemptOwnerInventory::capture(entry, request);
    if !attempt_owner_inventory.is_vacant() {
        return indeterminate(
            owner,
            BackNoTargetPhysicalIndeterminateReason::ConflictingAttemptOwners(
                attempt_owner_inventory,
            ),
        );
    }
    if !current_host_binding_is_exact(world, host) {
        return indeterminate(
            owner,
            BackNoTargetPhysicalIndeterminateReason::HostAuthorityDrift,
        );
    }
    let authority_graph = match freeze_back_no_target_authority_graph(world, registry, host, active)
    {
        Ok(projection) => projection,
        Err(evidence) => {
            return indeterminate(
                owner,
                BackNoTargetPhysicalIndeterminateReason::AuthorityGraph(evidence),
            );
        }
    };
    let mut current_claims = Vec::new();
    for frame in active.frames() {
        for claim in frame.claims() {
            if claim.lifecycle() != EntityLifecycle::Active {
                return indeterminate(
                    owner,
                    BackNoTargetPhysicalIndeterminateReason::ActiveTopologyDrift,
                );
            }
            let role = match claim.owner() {
                RegistryOwner::Committed(EntityOwner::FrameShell(claim_frame))
                    if claim_frame == frame.frame() =>
                {
                    ProvenEmptyCurrentRole::FrameShell(claim_frame.clone())
                }
                RegistryOwner::Committed(EntityOwner::FrameContentTarget(claim_frame))
                    if claim_frame == frame.frame() =>
                {
                    ProvenEmptyCurrentRole::FrameContentTarget(claim_frame.clone())
                }
                RegistryOwner::Committed(EntityOwner::FrameOutput {
                    frame: claim_frame, ..
                }) if claim_frame == frame.frame() => {
                    ProvenEmptyCurrentRole::FrameOutput(claim_frame.clone())
                }
                _ => {
                    return indeterminate(
                        owner,
                        BackNoTargetPhysicalIndeterminateReason::ActiveTopologyDrift,
                    );
                }
            };
            current_claims.push(ProvenEmptyCurrentClaimProjection {
                claim: claim.clone(),
                role,
            });
        }
    }
    for slot in active.child_slots() {
        let mut exact_parents = active
            .frames()
            .iter()
            .filter(|frame| frame.frame() == slot.parent());
        let Some(parent_frame) = exact_parents.next() else {
            return indeterminate(
                owner,
                BackNoTargetPhysicalIndeterminateReason::ActiveTopologyDrift,
            );
        };
        if exact_parents.next().is_some()
            || parent_frame.outlet(slot.outlet()).is_none_or(|outlet| {
                outlet.anchor() != slot.anchor() || outlet.anchor_claim() != slot.anchor_claim()
            })
            || slot.slot_claim().owner()
                != &RegistryOwner::Committed(EntityOwner::ChildSlot {
                    parent: slot.parent().clone(),
                    outlet: slot.outlet().clone(),
                    slot: slot.slot_id(),
                })
            || !cleanup_claim_has_exact_authority(world, slot.slot_claim())
            || world
                .get::<bevy_ecs::hierarchy::ChildOf>(slot.slot())
                .map(bevy_ecs::hierarchy::ChildOf::parent)
                != Some(slot.anchor())
            || !parent_frame
                .claims()
                .any(|claim| claim == slot.anchor_claim())
            || !slot.anchor_binding().is_current(world, slot.parent(), true)
        {
            return indeterminate(
                owner,
                BackNoTargetPhysicalIndeterminateReason::ActiveTopologyDrift,
            );
        }
        let observed_anchor_children = world
            .get::<bevy_ecs::hierarchy::Children>(slot.anchor())
            .map(|children| children.iter().copied().collect::<Vec<_>>())
            .unwrap_or_default();
        if slot
            .current_anchor_children(world)
            .iter()
            .any(|expected| !observed_anchor_children.contains(expected))
        {
            return indeterminate(
                owner,
                BackNoTargetPhysicalIndeterminateReason::ActiveTopologyDrift,
            );
        }
        let binding = ProvenEmptyChildSlotBindingProjection {
            parent: slot.parent().clone(),
            outlet: slot.outlet().clone(),
            slot_id: slot.slot_id(),
            anchor_binding: slot.anchor_binding().clone(),
            expected_support_relationships: slot.current_anchor_children(world),
        };
        let mut mounted_children = active
            .frames()
            .iter()
            .filter(|frame| frame.mount_binding().matches_mounted_child_slot(slot));
        let child = mounted_children.next();
        if mounted_children.next().is_some() {
            return indeterminate(
                owner,
                BackNoTargetPhysicalIndeterminateReason::ActiveTopologyDrift,
            );
        }
        let role = match child {
            Some(child) if slot.slot_claim().lifecycle() == EntityLifecycle::Active => {
                ProvenEmptyCurrentRole::OccupiedActiveSlotBinding(
                    ProvenEmptyOccupiedSlotProjection {
                        child: child.frame().clone(),
                        binding,
                    },
                )
            }
            None if slot.slot_claim().lifecycle() == EntityLifecycle::PreparedInert => {
                ProvenEmptyCurrentRole::RetainedEmptySlotBinding(binding)
            }
            Some(_) | None => {
                return indeterminate(
                    owner,
                    BackNoTargetPhysicalIndeterminateReason::ActiveTopologyDrift,
                );
            }
        };
        current_claims.push(ProvenEmptyCurrentClaimProjection {
            claim: slot.slot_claim().clone(),
            role,
        });
    }
    current_claims.sort_by_key(|current| current.claim.entity().to_bits());

    let Some(current_authority_anchor) = authority_graph.root() else {
        return indeterminate(
            owner,
            BackNoTargetPhysicalIndeterminateReason::ActiveTopologyDrift,
        );
    };
    let current_authority_frames = active
        .frames()
        .iter()
        .map(|frame| frame.frame().clone())
        .collect::<Vec<_>>();
    let mut expected_authority_claims = current_claims
        .iter()
        .map(|current| current.claim.clone())
        .chain(std::iter::once(host.top_slot_claim.clone()))
        .collect::<Vec<_>>();
    expected_authority_claims.sort_by_key(|claim| claim.entity().to_bits());
    let mut observed_authority_claims = registry
        .records()
        .iter()
        .filter(|record| {
            back_target_driver_owner_belongs_to_frames(record.owner(), &current_authority_frames)
                || matches!(
                    record.owner(),
                    RegistryOwner::Committed(EntityOwner::HostTopSlot {
                        host: version,
                        slot,
                    }) if version == &host.version && *slot == host.slot_id
                )
        })
        .map(|record| record.claim())
        .collect::<Vec<_>>();
    observed_authority_claims.sort_by_key(|claim| claim.entity().to_bits());
    if observed_authority_claims != expected_authority_claims {
        return indeterminate(
            owner,
            BackNoTargetPhysicalIndeterminateReason::ActiveRegistryDrift {
                expected: expected_authority_claims,
                observed: observed_authority_claims,
            },
        );
    }

    let observed_parent = |entity| {
        world
            .get::<bevy_ecs::hierarchy::ChildOf>(entity)
            .map(bevy_ecs::hierarchy::ChildOf::parent)
    };
    let mut protected = vec![
        ProvenEmptyProtectedEntityProjection {
            entity: host.host,
            observed_parent: observed_parent(host.host),
            claim: None,
            role: ProvenEmptyProtectedRole::AppOwnedHost,
        },
        ProvenEmptyProtectedEntityProjection {
            entity: host.top_slot,
            observed_parent: observed_parent(host.top_slot),
            claim: Some(host.top_slot_claim.clone()),
            role: ProvenEmptyProtectedRole::HostTopSlot,
        },
    ];
    protected.extend(current_claims.iter().map(|current| {
        let entity = current.claim.entity();
        ProvenEmptyProtectedEntityProjection {
            entity,
            observed_parent: observed_parent(entity),
            claim: Some(current.claim.clone()),
            role: ProvenEmptyProtectedRole::Current(current.role.clone()),
        }
    }));
    let mut observed_relationship_closure = observed_descendant_closure(world, [host.host])
        .into_iter()
        .collect::<Vec<_>>();
    observed_relationship_closure.sort_by_key(|entity| entity.to_bits());
    for entity in observed_relationship_closure.iter().copied() {
        if protected.iter().any(|protected| protected.entity == entity) {
            continue;
        }
        if let Some(record) = registry.record(entity) {
            let claim = record.claim();
            let registered_manifest = record.manifest().clone();
            let (expected, observed) = match freeze_back_no_target_registered_extra(
                world,
                registry,
                &claim,
                registered_manifest,
            ) {
                Ok(frozen) => frozen,
                Err((cause, expected, observed)) => {
                    return indeterminate(
                        owner,
                        BackNoTargetPhysicalIndeterminateReason::RegisteredExtra {
                            entity,
                            cause,
                            expected,
                            observed,
                        },
                    );
                }
            };
            let authority_source = back_no_target_registered_extra_authority_source(claim.owner());
            let shares_current_authority = match authority_source {
                BackNoTargetRegisteredExtraAuthoritySource::Frame(frame) => {
                    bevy_ui_router_core::integration::frame_versions_share_router(
                        current_authority_anchor,
                        frame,
                    )
                }
                BackNoTargetRegisteredExtraAuthoritySource::Host(host) => {
                    bevy_ui_router_core::integration::host_version_shares_router_with_frame(
                        host,
                        current_authority_anchor,
                    )
                }
                BackNoTargetRegisteredExtraAuthoritySource::Unavailable => {
                    return indeterminate(
                        owner,
                        BackNoTargetPhysicalIndeterminateReason::RegisteredExtra {
                            entity,
                            cause: BackNoTargetRegisteredExtraIndeterminateCause::OpaqueAuthorityRelationUnavailable,
                            expected,
                            observed,
                        },
                    );
                }
            };
            if shares_current_authority {
                return indeterminate(
                    owner,
                    BackNoTargetPhysicalIndeterminateReason::UnexpectedSameAuthorityDescendant {
                        entity,
                        expected,
                        observed,
                    },
                );
            }
            let observed_parent = observed.observed_parent;
            protected.push(ProvenEmptyProtectedEntityProjection {
                entity,
                observed_parent,
                claim: Some(claim),
                role: ProvenEmptyProtectedRole::OtherAuthorityDescendant { expected, observed },
            });
            continue;
        }
        protected.push(ProvenEmptyProtectedEntityProjection {
            entity,
            observed_parent: observed_parent(entity),
            claim: None,
            role: ProvenEmptyProtectedRole::UnregisteredForeignDescendant,
        });
    }
    protected.sort_by_key(|entity| entity.entity.to_bits());
    if let Some(duplicate) = protected
        .windows(2)
        .find(|pair| pair[0].entity == pair[1].entity)
        .map(|pair| pair[0].entity)
    {
        return indeterminate(
            owner,
            BackNoTargetPhysicalIndeterminateReason::DuplicateProtectedEntity(duplicate),
        );
    }
    if protected
        .iter()
        .map(|protected| protected.entity)
        .ne(observed_relationship_closure.iter().copied())
    {
        return indeterminate(
            owner,
            BackNoTargetPhysicalIndeterminateReason::RelationshipClosureDrift {
                observed: observed_relationship_closure,
                protected,
            },
        );
    }

    BackNoTargetPhysicalFreezeOutcome::ProvenEmpty(BackNoTargetProvenEmptyOwner {
        owner,
        projection: ProvenEmptyProjection {
            host: ProjectionlessHostProjection::from(host),
            request,
            authority_graph,
            attempt_owner_inventory,
            current_claims,
            observed_relationship_closure,
            protected,
            authorized_targets: ProvenEmptyAuthorizedTargets::new(),
        },
    })
}

fn back_no_target_current_authority_is_exact(
    world: &World,
    registry: &RouteEntityRegistry,
    host: &HostBinding,
    active: &RealizedBranch,
) -> bool {
    current_host_binding_is_exact(world, host)
        && active.claims().all(|claim| {
            registry
                .record(claim.entity())
                .is_some_and(|record| record.claim() == *claim)
                && cleanup_claim_has_exact_authority(world, claim)
        })
        && active.child_slots().iter().all(|slot| {
            world
                .get::<bevy_ecs::hierarchy::ChildOf>(slot.slot())
                .map(bevy_ecs::hierarchy::ChildOf::parent)
                == Some(slot.anchor())
                && world
                    .get::<Children>(slot.anchor())
                    .is_some_and(|children| children.contains(&slot.slot()))
                && (slot.anchor_binding().is_native()
                    || slot.expected_anchor_children().iter().all(|expected| {
                        world
                            .get::<bevy_ecs::hierarchy::Children>(slot.anchor())
                            .is_some_and(|children| children.contains(expected))
                    }))
        })
}

pub(crate) fn classify_back_no_target(
    world: &World,
    entry: &mut RouterEntry,
    owner: BackPreparationOwner,
) -> BackNoTargetPreparationOutcome {
    let request = owner.request;
    let Some(registry) = world.get_resource::<RouteEntityRegistry>() else {
        return BackNoTargetPreparationOutcome::Retry {
            owner,
            evidence: BackNoTargetPhysicalIndeterminateEvidence {
                request,
                reason: BackNoTargetPhysicalIndeterminateReason::MissingRegistry,
            },
        };
    };
    let Some(host) = entry.host.as_ref() else {
        return BackNoTargetPreparationOutcome::Retry {
            owner,
            evidence: BackNoTargetPhysicalIndeterminateEvidence {
                request,
                reason: BackNoTargetPhysicalIndeterminateReason::MissingHost,
            },
        };
    };
    let Some(active) = entry.active.as_ref() else {
        return BackNoTargetPreparationOutcome::Retry {
            owner,
            evidence: BackNoTargetPhysicalIndeterminateEvidence {
                request,
                reason: BackNoTargetPhysicalIndeterminateReason::MissingActive,
            },
        };
    };
    if back_no_target_current_authority_is_exact(world, registry, host, active) {
        return match freeze_back_no_target_proven_empty(world, entry, owner) {
            BackNoTargetPhysicalFreezeOutcome::ProvenEmpty(proven) => {
                match approve_back_no_target_proven_empty(entry, proven) {
                    BackNoTargetProvenEmptyApprovalOutcome::Prepared(prepared) => {
                        BackNoTargetPreparationOutcome::Ready(prepared)
                    }
                    BackNoTargetProvenEmptyApprovalOutcome::ReceiverMismatch {
                        owner,
                        mismatch,
                    } => {
                        let BackNoTargetProvenEmptyOwner { owner, .. } = owner;
                        BackNoTargetPreparationOutcome::ReceiverMismatch { owner, mismatch }
                    }
                }
            }
            BackNoTargetPhysicalFreezeOutcome::Indeterminate { owner, evidence } => {
                BackNoTargetPreparationOutcome::Retry { owner, evidence }
            }
        };
    }
    if entry
        .requests
        .iter()
        .filter(|indexed| **indexed == request)
        .count()
        != 1
    {
        return BackNoTargetPreparationOutcome::Retry {
            owner,
            evidence: BackNoTargetPhysicalIndeterminateEvidence {
                request,
                reason: BackNoTargetPhysicalIndeterminateReason::RequestIndexDrift,
            },
        };
    }
    let inventory = ProvenEmptyAttemptOwnerInventory::capture(entry, request);
    if !inventory.is_vacant() {
        return BackNoTargetPreparationOutcome::Retry {
            owner,
            evidence: BackNoTargetPhysicalIndeterminateEvidence {
                request,
                reason: BackNoTargetPhysicalIndeterminateReason::ConflictingAttemptOwners(
                    inventory,
                ),
            },
        };
    }
    let source = FrozenBackNoTargetAuthoritySource {
        request,
        host: ProjectionlessHostProjection::from(host),
        active_frames: active
            .frames()
            .iter()
            .map(|frame| frame.frame().clone())
            .collect(),
        active_claims: active.claims().cloned().collect(),
    };
    let host_children = world
        .get::<bevy_ecs::hierarchy::Children>(host.host)
        .map(|children| children.iter().copied().collect::<Vec<_>>())
        .unwrap_or_default();
    let frozen =
        match FrozenCleanupManifest::from_back_no_target_authority(world, registry, active, host) {
            Ok(frozen) => frozen,
            Err(AuthoritySettlementManifestError::RetryableCapacity) => {
                return BackNoTargetPreparationOutcome::Retry {
                    owner,
                    evidence: BackNoTargetPhysicalIndeterminateEvidence {
                        request,
                        reason: BackNoTargetPhysicalIndeterminateReason::AuthorityManifest,
                    },
                };
            }
            Err(
                error @ (AuthoritySettlementManifestError::InvalidAuthority
                | AuthoritySettlementManifestError::InvalidTopology),
            ) => {
                return BackNoTargetPreparationOutcome::AuthorityPermanent(
                    BackNoTargetAuthorityPermanentOwner {
                        owner,
                        source,
                        partition: None,
                        isolation: None,
                        reserved: None,
                        evidence: BackNoTargetAuthorityPermanentEvidence::Manifest(error),
                    },
                );
            }
        };
    let partition = match frozen.freeze_back_no_target_authority_partition(world, registry, host) {
        Some(partition) => partition,
        None => {
            return BackNoTargetPreparationOutcome::AuthorityPermanent(
                BackNoTargetAuthorityPermanentOwner {
                    owner,
                    source,
                    partition: None,
                    isolation: None,
                    reserved: None,
                    evidence: BackNoTargetAuthorityPermanentEvidence::Partition {
                        manifest: frozen,
                    },
                },
            );
        }
    };
    let manifest_digest = frozen.digest();
    let obligation_count = frozen.len();
    let isolation = match prepare_authority_isolation_targets(
        world,
        active.claims().chain(std::iter::once(&host.top_slot_claim)),
        active
            .live_interactive_entities(world)
            .into_iter()
            .chain(std::iter::once(host.top_slot)),
        0,
    ) {
        AuthorityIsolationPreparation::Ready(isolation) => isolation,
        AuthorityIsolationPreparation::RetryableCapacity => {
            return BackNoTargetPreparationOutcome::Retry {
                owner,
                evidence: BackNoTargetPhysicalIndeterminateEvidence {
                    request,
                    reason: BackNoTargetPhysicalIndeterminateReason::AuthorityIsolation,
                },
            };
        }
        AuthorityIsolationPreparation::PermanentContradiction {
            contradiction,
            fail_closed,
        } => {
            return BackNoTargetPreparationOutcome::AuthorityPermanent(
                BackNoTargetAuthorityPermanentOwner {
                    owner,
                    source,
                    partition: Some(partition),
                    isolation: Some(fail_closed),
                    reserved: None,
                    evidence: BackNoTargetAuthorityPermanentEvidence::Isolation {
                        manifest: frozen,
                        contradiction,
                    },
                },
            );
        }
    };
    let reserved = match CleanupLedger::reserve(frozen) {
        Ok(reserved) => reserved,
        Err(error) => match error.kind() {
            CleanupLedgerReserveErrorKind::RetryableCapacity => {
                return BackNoTargetPreparationOutcome::Retry {
                    owner,
                    evidence: BackNoTargetPhysicalIndeterminateEvidence {
                        request,
                        reason: BackNoTargetPhysicalIndeterminateReason::AuthorityLedger,
                    },
                };
            }
            kind @ (CleanupLedgerReserveErrorKind::DomainExhausted
            | CleanupLedgerReserveErrorKind::UnfinalizedBoundaryFacts) => {
                return BackNoTargetPreparationOutcome::AuthorityPermanent(
                    BackNoTargetAuthorityPermanentOwner {
                        owner,
                        source,
                        partition: Some(partition),
                        isolation: Some(isolation),
                        reserved: None,
                        evidence: BackNoTargetAuthorityPermanentEvidence::Ledger {
                            kind,
                            manifest: error.into_manifest(),
                        },
                    },
                );
            }
        },
    };
    let logical_roots = logical_cleanup_roots(active);
    let BackPreparationOwner {
        candidate,
        capacity,
        request,
    } = owner;
    let Some(issuer) = entry.back_capacity_issuer.as_mut() else {
        return BackNoTargetPreparationOutcome::CapacityRejected {
            owner: BackPreparationOwner {
                candidate,
                capacity,
                request,
            },
            error: BackCapacityRetireError::StaleGeneration,
        };
    };
    let (guard, prepared_retirement) = match issuer
        .preflight_no_target_retirement(&capacity, BackCapacityRetireDisposition::AuthorityLost)
    {
        Ok(prepared) => prepared,
        Err(error) => {
            return BackNoTargetPreparationOutcome::CapacityRejected {
                owner: BackPreparationOwner {
                    candidate,
                    capacity,
                    request,
                },
                error,
            };
        }
    };
    let active = match entry.active.take() {
        Some(active)
            if source
                .active_frames
                .iter()
                .eq(active.frames().iter().map(RealizedFrame::frame))
                && source.active_claims.iter().eq(active.claims()) =>
        {
            active
        }
        observed => {
            let observed_frames = observed
                .as_ref()
                .map(|active| {
                    active
                        .frames()
                        .iter()
                        .map(|frame| frame.frame().clone())
                        .collect()
                })
                .unwrap_or_default();
            let observed_claims = observed
                .as_ref()
                .map(|active| active.claims().cloned().collect())
                .unwrap_or_default();
            entry.active = observed;
            drop(guard);
            drop(prepared_retirement);
            return BackNoTargetPreparationOutcome::AuthorityPermanent(
                BackNoTargetAuthorityPermanentOwner {
                    owner: BackPreparationOwner {
                        candidate,
                        capacity,
                        request,
                    },
                    source,
                    partition: Some(partition),
                    isolation: Some(isolation),
                    reserved: Some(reserved),
                    evidence: BackNoTargetAuthorityPermanentEvidence::ActiveOwnerDrift {
                        observed_frames,
                        observed_claims,
                    },
                },
            );
        }
    };
    match entry
        .engine
        .accept_back_no_target_authority_lost_with_retirement(candidate, move || guard.commit())
    {
        BackNoTargetAuthorityCoreOutcome::Accepted(settlement) => {
            drop(capacity);
            let retirement = prepared_retirement.into_evidence_after_exact_receiver();
            BackNoTargetPreparationOutcome::AuthoritySettlement(
                BackNoTargetAuthoritySettlementDisposition {
                    settlement,
                    retirement,
                    physical: BackNoTargetAuthorityPhysicalSettlementOwner {
                        preparation: BackNoTargetAuthorityPhysicalPreparation {
                            source,
                            partition: Some(partition),
                            reserved,
                            active,
                            isolation: Some(isolation),
                            host_children,
                        },
                    },
                    manifest: BackNoTargetAuthorityManifest::new(CleanupManifest::new(
                        logical_roots,
                        manifest_digest,
                        obligation_count,
                    )),
                },
            )
        }
        BackNoTargetAuthorityCoreOutcome::Mismatch {
            candidate,
            mismatch,
        } => {
            entry.active = Some(active);
            BackNoTargetPreparationOutcome::ReceiverMismatch {
                owner: BackPreparationOwner {
                    candidate,
                    capacity,
                    request,
                },
                mismatch,
            }
        }
    }
}

pub(crate) enum BackNoTargetAuthoritySettlementDispatchOutcome {
    Dispatched,
    Inert,
    EntryMismatch {
        disposition: BackNoTargetAuthoritySettlementDisposition,
    },
    ReceiverMismatch {
        disposition: BackNoTargetAuthoritySettlementDisposition,
        mismatch: PreparationReceiverMismatchView,
    },
    Retryable {
        disposition: BackNoTargetAuthoritySettlementDisposition,
        evidence: BackNoTargetAuthorityBeginRetryEvidence,
    },
    Permanent {
        disposition: BackNoTargetAuthoritySettlementDisposition,
        evidence: BackNoTargetAuthorityBeginPermanentEvidence,
    },
}

fn back_no_target_authority_begin_inert_evidence(
    pending: Option<&PendingBackNoTargetAuthorityCleanup>,
) -> BackNoTargetAuthorityBeginInertEvidence {
    match pending {
        None => BackNoTargetAuthorityBeginInertEvidence::MissingSlot,
        Some(PendingBackNoTargetAuthorityCleanup::Installed(installed)) => {
            if installed.batch.is_some() {
                BackNoTargetAuthorityBeginInertEvidence::BatchInstalledOnRejection
            } else if installed.inert.is_some() {
                BackNoTargetAuthorityBeginInertEvidence::InertInstallation
            } else if matches!(
                &installed.ledger,
                BackNoTargetAuthorityLedgerHolder::Active(_)
            ) {
                BackNoTargetAuthorityBeginInertEvidence::ActiveLedgerOnRejection
            } else {
                BackNoTargetAuthorityBeginInertEvidence::WrongPhase
            }
        }
        Some(
            PendingBackNoTargetAuthorityCleanup::Completing(_)
            | PendingBackNoTargetAuthorityCleanup::Settled(_)
            | PendingBackNoTargetAuthorityCleanup::AuthorityBeginInert(_)
            | PendingBackNoTargetAuthorityCleanup::CompletionInert(_),
        ) => BackNoTargetAuthorityBeginInertEvidence::WrongPhase,
    }
}

fn retain_back_no_target_authority_begin_inert(
    pending: &mut Option<PendingBackNoTargetAuthorityCleanup>,
    begin: BackNoTargetAuthorityBeginOwner,
    current: Option<PendingBackNoTargetAuthorityCleanup>,
    evidence: BackNoTargetAuthorityBeginInertEvidence,
    cause: BackNoTargetAuthorityBeginInertCause,
) {
    let physical = current.map(|current| match current {
        PendingBackNoTargetAuthorityCleanup::Installed(installed) => {
            BackNoTargetAuthorityBeginInertPhysical::Installed(installed)
        }
        PendingBackNoTargetAuthorityCleanup::Completing(completing) => {
            BackNoTargetAuthorityBeginInertPhysical::Completing(completing)
        }
        PendingBackNoTargetAuthorityCleanup::Settled(settled) => {
            BackNoTargetAuthorityBeginInertPhysical::Settled(settled)
        }
        PendingBackNoTargetAuthorityCleanup::AuthorityBeginInert(inert) => {
            BackNoTargetAuthorityBeginInertPhysical::PriorInert(Box::new(inert))
        }
        PendingBackNoTargetAuthorityCleanup::CompletionInert(inert) => {
            BackNoTargetAuthorityBeginInertPhysical::CompletionInert(Box::new(inert))
        }
    });
    *pending = Some(PendingBackNoTargetAuthorityCleanup::AuthorityBeginInert(
        BackNoTargetAuthorityBeginInert {
            begin,
            physical,
            evidence,
            cause,
        },
    ));
}

fn rebuild_unstarted_back_no_target_authority_disposition(
    pending: &mut Option<PendingBackNoTargetAuthorityCleanup>,
    host: &mut Option<HostBinding>,
    begin: BackNoTargetAuthorityBeginOwner,
) -> Result<
    BackNoTargetAuthoritySettlementDisposition,
    (
        BackNoTargetAuthorityBeginOwner,
        Option<PendingBackNoTargetAuthorityCleanup>,
        BackNoTargetAuthorityBeginInertEvidence,
    ),
> {
    let current = pending.take();
    let evidence = back_no_target_authority_begin_inert_evidence(current.as_ref());
    let (source, retirement, ledger) = match current {
        Some(PendingBackNoTargetAuthorityCleanup::Installed(
            InstalledBackNoTargetAuthorityCleanup {
                source,
                retirement,
                ledger: BackNoTargetAuthorityLedgerHolder::Prepared(ledger),
                batch: None,
                inert: None,
            },
        )) => (source, retirement, ledger),
        current => return Err((begin, current, evidence)),
    };
    let (reserved, owner, host_children, partition) = ledger.into_unstarted_parts();
    let (active, retired_host) = owner.into_parts();
    let (settlement, lease, manifest) = begin.into_parts();
    *host = Some(retired_host.restore(lease));
    Ok(BackNoTargetAuthoritySettlementDisposition {
        settlement,
        retirement,
        physical: BackNoTargetAuthorityPhysicalSettlementOwner {
            preparation: BackNoTargetAuthorityPhysicalPreparation {
                source,
                partition: Some(partition),
                reserved,
                active,
                isolation: None,
                host_children,
            },
        },
        manifest,
    })
}

pub(crate) fn dispatch_back_no_target_authority_settlement(
    world: &mut World,
    entry: &mut RouterEntry,
    disposition: BackNoTargetAuthoritySettlementDisposition,
) -> BackNoTargetAuthoritySettlementDispatchOutcome {
    let BackNoTargetAuthoritySettlementDisposition {
        settlement,
        retirement,
        physical,
        manifest,
    } = disposition;
    let BackNoTargetAuthorityPhysicalSettlementOwner { mut preparation } = physical;
    let exact_entry = entry.pending_back_no_target_authority_cleanup.is_none()
        && entry.pending_back_no_target_completion.is_none()
        && entry.cleanup_ledger.is_none()
        && entry.active.is_none()
        && entry.host.as_ref().is_some_and(|host| {
            ProjectionlessHostProjection::from(host) == preparation.source.host
        })
        && entry
            .requests
            .iter()
            .filter(|request| **request == preparation.source.request)
            .count()
            == 1
        && entry
            .engine
            .request(preparation.source.request)
            .is_some_and(|record| {
                matches!(
                    record.status(),
                    bevy_ui_router_core::prelude::RequestStatus::Queued
                )
            })
        && preparation.source.active_frames.iter().eq(preparation
            .active
            .frames()
            .iter()
            .map(RealizedFrame::frame))
        && preparation
            .source
            .active_claims
            .iter()
            .eq(preparation.active.claims())
        && preparation
            .partition
            .as_ref()
            .is_some_and(|partition| partition.refreeze_unstarted(world));
    if !exact_entry {
        return BackNoTargetAuthoritySettlementDispatchOutcome::EntryMismatch {
            disposition: BackNoTargetAuthoritySettlementDisposition {
                settlement,
                retirement,
                physical: BackNoTargetAuthorityPhysicalSettlementOwner { preparation },
                manifest,
            },
        };
    }
    if let Some(isolation) = preparation.isolation.take() {
        isolation.apply(world);
    }
    if preparation
        .partition
        .as_ref()
        .is_none_or(|partition| !partition.refreeze_unstarted(world))
    {
        return BackNoTargetAuthoritySettlementDispatchOutcome::EntryMismatch {
            disposition: BackNoTargetAuthoritySettlementDisposition {
                settlement,
                retirement,
                physical: BackNoTargetAuthorityPhysicalSettlementOwner { preparation },
                manifest,
            },
        };
    }
    let Some(partition) = preparation.partition.take() else {
        return BackNoTargetAuthoritySettlementDispatchOutcome::EntryMismatch {
            disposition: BackNoTargetAuthoritySettlementDisposition {
                settlement,
                retirement,
                physical: BackNoTargetAuthorityPhysicalSettlementOwner { preparation },
                manifest,
            },
        };
    };
    let Some(host) = entry.host.take() else {
        preparation.partition = Some(partition);
        return BackNoTargetAuthoritySettlementDispatchOutcome::EntryMismatch {
            disposition: BackNoTargetAuthoritySettlementDisposition {
                settlement,
                retirement,
                physical: BackNoTargetAuthorityPhysicalSettlementOwner { preparation },
                manifest,
            },
        };
    };
    let (retired_host, lease) = host.into_retired();
    let ledger = PreparedBackNoTargetAuthorityCleanupLedger::new(
        preparation.reserved,
        preparation.active,
        retired_host,
        preparation.host_children,
        partition,
    );
    entry.pending_back_no_target_authority_cleanup = Some(
        PendingBackNoTargetAuthorityCleanup::Installed(InstalledBackNoTargetAuthorityCleanup {
            source: preparation.source,
            retirement,
            ledger: BackNoTargetAuthorityLedgerHolder::Prepared(ledger),
            batch: None,
            inert: None,
        }),
    );
    let begin = BackNoTargetAuthorityBeginOwner::new(settlement, lease, manifest);
    let RouterEntry {
        engine,
        host: host_slot,
        pending_back_no_target_authority_cleanup: pending,
        ..
    } = entry;
    let Some(PendingBackNoTargetAuthorityCleanup::Installed(installed)) = pending.as_mut() else {
        let current = pending.take();
        let evidence = back_no_target_authority_begin_inert_evidence(current.as_ref());
        retain_back_no_target_authority_begin_inert(
            pending,
            begin,
            current,
            evidence,
            BackNoTargetAuthorityBeginInertCause::NotDispatched,
        );
        return BackNoTargetAuthoritySettlementDispatchOutcome::Inert;
    };
    let batch_slot = &mut installed.batch;
    let outcome =
        engine.begin_back_no_target_authority_settlement(begin, |batch| *batch_slot = Some(batch));
    match outcome {
        BackNoTargetAuthorityBeginOutcome::Begun => {
            BackNoTargetAuthoritySettlementDispatchOutcome::Dispatched
        }
        BackNoTargetAuthorityBeginOutcome::ReceiverMismatch { owner, mismatch } => {
            match rebuild_unstarted_back_no_target_authority_disposition(pending, host_slot, owner)
            {
                Ok(disposition) => {
                    BackNoTargetAuthoritySettlementDispatchOutcome::ReceiverMismatch {
                        disposition,
                        mismatch,
                    }
                }
                Err((owner, current, evidence)) => {
                    retain_back_no_target_authority_begin_inert(
                        pending,
                        owner,
                        current,
                        evidence,
                        BackNoTargetAuthorityBeginInertCause::ReceiverMismatch(mismatch),
                    );
                    BackNoTargetAuthoritySettlementDispatchOutcome::Inert
                }
            }
        }
        BackNoTargetAuthorityBeginOutcome::Retryable { owner, evidence } => {
            match rebuild_unstarted_back_no_target_authority_disposition(pending, host_slot, owner)
            {
                Ok(disposition) => BackNoTargetAuthoritySettlementDispatchOutcome::Retryable {
                    disposition,
                    evidence,
                },
                Err((owner, current, observed)) => {
                    retain_back_no_target_authority_begin_inert(
                        pending,
                        owner,
                        current,
                        observed,
                        BackNoTargetAuthorityBeginInertCause::Retryable(evidence),
                    );
                    BackNoTargetAuthoritySettlementDispatchOutcome::Inert
                }
            }
        }
        BackNoTargetAuthorityBeginOutcome::Permanent { owner, evidence } => {
            match rebuild_unstarted_back_no_target_authority_disposition(pending, host_slot, owner)
            {
                Ok(disposition) => BackNoTargetAuthoritySettlementDispatchOutcome::Permanent {
                    disposition,
                    evidence,
                },
                Err((owner, current, observed)) => {
                    retain_back_no_target_authority_begin_inert(
                        pending,
                        owner,
                        current,
                        observed,
                        BackNoTargetAuthorityBeginInertCause::Permanent(evidence),
                    );
                    BackNoTargetAuthoritySettlementDispatchOutcome::Inert
                }
            }
        }
    }
}

pub(crate) fn approve_back_no_target_proven_empty(
    entry: &mut RouterEntry,
    owner: BackNoTargetProvenEmptyOwner,
) -> BackNoTargetProvenEmptyApprovalOutcome {
    let BackNoTargetProvenEmptyOwner {
        owner:
            BackPreparationOwner {
                candidate,
                capacity,
                request,
            },
        projection,
    } = owner;
    match entry.engine.approve_back_no_target_logical_zero(candidate) {
        BackNoTargetApprovalOutcome::Approved(approved) => {
            BackNoTargetProvenEmptyApprovalOutcome::Prepared(PreparedBackNoTarget {
                approved,
                capacity,
                projection,
            })
        }
        BackNoTargetApprovalOutcome::Mismatch {
            candidate,
            mismatch,
        } => BackNoTargetProvenEmptyApprovalOutcome::ReceiverMismatch {
            owner: BackNoTargetProvenEmptyOwner {
                owner: BackPreparationOwner {
                    candidate,
                    capacity,
                    request,
                },
                projection,
            },
            mismatch,
        },
    }
}

pub(crate) fn consume_prepared_back_no_target(
    entry: &mut RouterEntry,
    owner: PreparedBackNoTarget,
) -> BackNoTargetFinalizationOutcome {
    consume_prepared_back_no_target_with_observers(entry, owner, |_, _| {}, |_| {})
}

#[cfg(test)]
fn consume_prepared_back_no_target_with_prepared_observer<F>(
    entry: &mut RouterEntry,
    owner: PreparedBackNoTarget,
    observe_prepared: F,
) -> BackNoTargetFinalizationOutcome
where
    F: FnOnce(&RouterEngine, &ManifestNeutralLedgerSlot),
{
    consume_prepared_back_no_target_with_observers(entry, owner, observe_prepared, |_| {})
}

#[cfg(test)]
fn consume_prepared_back_no_target_with_installation_observer<F>(
    entry: &mut RouterEntry,
    owner: PreparedBackNoTarget,
    observe_installed: F,
) -> BackNoTargetFinalizationOutcome
where
    F: FnOnce(&PendingBackNoTargetCompletion),
{
    consume_prepared_back_no_target_with_observers(entry, owner, |_, _| {}, observe_installed)
}

fn consume_prepared_back_no_target_with_observers<PreparedObserver, InstallationObserver>(
    entry: &mut RouterEntry,
    owner: PreparedBackNoTarget,
    observe_prepared: PreparedObserver,
    observe_installed: InstallationObserver,
) -> BackNoTargetFinalizationOutcome
where
    PreparedObserver: FnOnce(&RouterEngine, &ManifestNeutralLedgerSlot),
    InstallationObserver: FnOnce(&PendingBackNoTargetCompletion),
{
    let PreparedBackNoTarget {
        approved,
        capacity,
        projection,
    } = owner;
    let Some((manifest_digest, obligation_count)) = projection.canonical_manifest_commitment()
    else {
        return BackNoTargetFinalizationOutcome::CapacityRejected {
            owner: PreparedBackNoTarget {
                approved,
                capacity,
                projection,
            },
            error: BackCapacityRetireError::StaleGeneration,
        };
    };
    let RouterEntry {
        engine,
        back_capacity_issuer,
        pending_back_no_target_completion,
        ..
    } = entry;
    if pending_back_no_target_completion.is_some() {
        return BackNoTargetFinalizationOutcome::CapacityRejected {
            owner: PreparedBackNoTarget {
                approved,
                capacity,
                projection,
            },
            error: BackCapacityRetireError::StaleGeneration,
        };
    }
    let Some(issuer) = back_capacity_issuer.as_mut() else {
        return BackNoTargetFinalizationOutcome::CapacityRejected {
            owner: PreparedBackNoTarget {
                approved,
                capacity,
                projection,
            },
            error: BackCapacityRetireError::StaleGeneration,
        };
    };
    let (guard, prepared_slot) = match issuer.preflight_back_canonical_zero(&capacity, &projection)
    {
        Ok(prepared) => prepared,
        Err(error) => {
            return BackNoTargetFinalizationOutcome::CapacityRejected {
                owner: PreparedBackNoTarget {
                    approved,
                    capacity,
                    projection,
                },
                error,
            };
        }
    };

    match engine.accept_back_no_target_ready_with_canonical_zero_bind(
        approved,
        manifest_digest,
        obligation_count,
        canonical_zero_completion_digest(),
        move || guard.commit(),
    ) {
        BackNoTargetReadyCoreOutcome::Prepared(publication) => {
            let slot = prepared_slot.into_slot_after_exact_receiver(capacity);
            observe_prepared(engine, &slot);
            engine.publish_prepared_back_no_target(publication, move |installation| {
                let batch = installation.batch();
                let ledger = slot.bind_back_no_target(batch);
                let installed = pending_back_no_target_completion.insert(
                    PendingBackNoTargetCompletion::Installed(InstalledBackNoTargetCleanup {
                        ledger,
                        installation,
                        inert: None,
                    }),
                );
                observe_installed(installed);
            });
            BackNoTargetFinalizationOutcome::Published
        }
        BackNoTargetReadyCoreOutcome::Mismatch { approved, mismatch } => {
            BackNoTargetFinalizationOutcome::ReceiverMismatch {
                owner: PreparedBackNoTarget {
                    approved,
                    capacity,
                    projection,
                },
                mismatch,
            }
        }
    }
}

pub(crate) struct BackNoTargetSourceDriftPermanentOwner {
    drift: BackNoTargetSourceDriftOwner,
    capacity: NoTargetCapacityPermit,
}

pub(crate) enum BackNoTargetSourceDriftPermanentSettlementOutcome {
    Settled {
        retirement: BackCapacityRetirementEvidence,
        terminal: BackNoTargetSourceDriftTerminalEvidence,
    },
    ReceiverMismatch {
        owner: BackNoTargetSourceDriftPermanentOwner,
        evidence: bevy_ui_router_core::integration::BackNoTargetSourceDriftEvidence,
    },
    CapacityRejected {
        owner: BackNoTargetSourceDriftPermanentOwner,
        error: BackCapacityRetireError,
    },
}

pub(crate) enum BackNoTargetBeginOutcome {
    Begun(BackPreparationOwner),
    Permanent(BackNoTargetSourceDriftPermanentOwner),
    CapacityRejected {
        bound: BoundBackNoTargetResolution,
        error: BackCapacityRetireError,
    },
    ReceiverMismatch {
        bound: BoundBackNoTargetResolution,
        evidence: BackResolutionEvidence,
    },
}

pub(crate) struct BoundBackContradictionResolution {
    owner: VerifiedBackResolutionContradiction,
    reservation: BoundBackCapacityReservation,
}

pub(crate) enum BoundBackResolutionHandoff {
    Target(BoundBackTargetResolution),
    NoTarget(BoundBackNoTargetResolution),
    Contradiction(BoundBackContradictionResolution),
}

pub(crate) enum TakeBackResolutionOutcome {
    Matched(BoundBackResolutionHandoff),
    Mismatch {
        reservation: BoundBackCapacityReservation,
        handoff: BackResolutionOwnerHandoff,
        evidence: BackResolutionEvidence,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BackResolutionEvidenceDisposition {
    Current,
    Foreign,
    ConsumedReplay,
}

pub(crate) fn resume_bound_back_target(
    entry: &mut RouterEntry,
    bound: BoundBackTargetResolution,
) -> BackTargetResumeOutcome {
    let BoundBackTargetResolution { owner, reservation } = bound;
    let RouterEntry {
        engine,
        back_capacity_issuer,
        ..
    } = entry;
    let issuer = back_capacity_issuer
        .as_mut()
        .expect("an installed RouterEntry retains its exact back capacity issuer");
    let (guard, prepared_retirement) = match issuer
        .preflight_bound_retirement(&reservation, BackCapacityRetireDisposition::Target)
    {
        Ok(prepared) => prepared,
        Err(error) => {
            return BackTargetResumeOutcome::CapacityRejected {
                bound: BoundBackTargetResolution { owner, reservation },
                error,
            };
        }
    };

    match engine.resume_back_target_with_retirement(owner, move || guard.commit()) {
        BackTargetCoreResumeOutcome::Resumed(start) => {
            drop(reservation);
            let retirement = prepared_retirement.into_evidence_after_exact_receiver();
            BackTargetResumeOutcome::Resumed { start, retirement }
        }
        BackTargetCoreResumeOutcome::Permanent(owner) => {
            drop(reservation);
            let retirement = prepared_retirement.into_evidence_after_exact_receiver();
            BackTargetResumeOutcome::Permanent { owner, retirement }
        }
        BackTargetCoreResumeOutcome::Mismatch { owner, evidence } => {
            BackTargetResumeOutcome::ReceiverMismatch {
                bound: BoundBackTargetResolution { owner, reservation },
                evidence,
            }
        }
    }
}

pub(crate) fn begin_bound_back_no_target(
    entry: &mut RouterEntry,
    bound: BoundBackNoTargetResolution,
) -> BackNoTargetBeginOutcome {
    let BoundBackNoTargetResolution {
        candidate,
        reservation,
    } = bound;
    let RouterEntry {
        engine,
        back_capacity_issuer,
        ..
    } = entry;
    let issuer = back_capacity_issuer
        .as_mut()
        .expect("an installed RouterEntry retains its exact back capacity issuer");
    let (guard, prepared_capacity) = match issuer.preflight_no_target_transfer(&reservation) {
        Ok(prepared) => prepared,
        Err(error) => {
            return BackNoTargetBeginOutcome::CapacityRejected {
                bound: BoundBackNoTargetResolution {
                    candidate,
                    reservation,
                },
                error,
            };
        }
    };
    let request = reservation.request();

    match engine.refreeze_back_no_target_source_with_transfer(candidate, move || guard.commit()) {
        BackNoTargetSourceRefreezeOutcome::Unchanged(candidate) => {
            let capacity = prepared_capacity.into_permit_after_exact_receiver(reservation);
            BackNoTargetBeginOutcome::Begun(BackPreparationOwner {
                candidate,
                capacity,
                request,
            })
        }
        BackNoTargetSourceRefreezeOutcome::Drift(drift) => {
            let capacity = prepared_capacity.into_permit_after_exact_receiver(reservation);
            BackNoTargetBeginOutcome::Permanent(BackNoTargetSourceDriftPermanentOwner {
                drift,
                capacity,
            })
        }
        BackNoTargetSourceRefreezeOutcome::Mismatch {
            candidate,
            evidence,
        } => BackNoTargetBeginOutcome::ReceiverMismatch {
            bound: BoundBackNoTargetResolution {
                candidate,
                reservation,
            },
            evidence,
        },
    }
}

pub(crate) fn settle_back_no_target_source_drift_permanent(
    entry: &mut RouterEntry,
    owner: BackNoTargetSourceDriftPermanentOwner,
) -> BackNoTargetSourceDriftPermanentSettlementOutcome {
    let BackNoTargetSourceDriftPermanentOwner { drift, capacity } = owner;
    let RouterEntry {
        engine,
        back_capacity_issuer,
        ..
    } = entry;
    let issuer = back_capacity_issuer
        .as_mut()
        .expect("an installed RouterEntry retains its exact back capacity issuer");
    let (guard, prepared_retirement) = match issuer
        .preflight_no_target_retirement(&capacity, BackCapacityRetireDisposition::Permanent)
    {
        Ok(prepared) => prepared,
        Err(error) => {
            return BackNoTargetSourceDriftPermanentSettlementOutcome::CapacityRejected {
                owner: BackNoTargetSourceDriftPermanentOwner { drift, capacity },
                error,
            };
        }
    };

    match engine.settle_back_no_target_source_drift_with_retirement(drift, move || guard.commit()) {
        BackNoTargetSourceDriftSettlementOutcome::Mismatch { owner, evidence } => {
            BackNoTargetSourceDriftPermanentSettlementOutcome::ReceiverMismatch {
                owner: BackNoTargetSourceDriftPermanentOwner {
                    drift: owner,
                    capacity,
                },
                evidence,
            }
        }
        BackNoTargetSourceDriftSettlementOutcome::Settled(terminal) => {
            drop(capacity);
            let retirement = prepared_retirement.into_evidence_after_exact_receiver();
            BackNoTargetSourceDriftPermanentSettlementOutcome::Settled {
                retirement,
                terminal,
            }
        }
    }
}

#[derive(Clone, Eq, PartialEq)]
struct ProjectionlessFallbackAdmission {
    frame: RouteFrameVersion,
    mount_target: FrameMountTarget,
    top_slot: bevy_ecs::entity::Entity,
    top_slot_claim: crate::registry::ExactRegistryClaim,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ProjectionlessHostProjection {
    version: bevy_ui_router_core::HostVersion,
    slot_id: bevy_ui_router_core::ids::RouteSlotId,
    host: bevy_ecs::entity::Entity,
    top_slot: bevy_ecs::entity::Entity,
    top_slot_claim: ExactRegistryClaim,
    candidate: ValidatedHostCandidate,
}

impl From<&HostBinding> for ProjectionlessHostProjection {
    fn from(host: &HostBinding) -> Self {
        Self {
            version: host.version.clone(),
            slot_id: host.slot_id,
            host: host.host,
            top_slot: host.top_slot,
            top_slot_claim: host.top_slot_claim.clone(),
            candidate: host.candidate.clone(),
        }
    }
}

#[derive(Clone, Eq, PartialEq)]
struct ProjectionlessObservedEntity {
    entity: bevy_ecs::entity::Entity,
    manifest: Option<crate::registry::ComponentManifest>,
    parent: Option<bevy_ecs::entity::Entity>,
    children: Vec<bevy_ecs::entity::Entity>,
    visibility: Option<bevy_camera::visibility::Visibility>,
    pickable: Option<(bool, bool)>,
    barrier: Option<crate::components::RouterInputBarrier>,
    registry_claim: Option<ExactRegistryClaim>,
    registry_manifest: Option<ComponentManifest>,
    gate: Option<(bool, bool, bool)>,
    ownership_seal: Option<RouteOwnershipSeal>,
    frame_lifecycle: Option<FrameLifecycle>,
    child_slot_lifecycle: Option<ChildSlotLifecycle>,
    child_slot_authority: Option<ChildSlotAuthority>,
    provisional_slot_lifecycle: Option<ProvisionalSlotLifecycle>,
    host_registration: Option<(RouterHandle, u64)>,
}

#[derive(Clone, Eq, PartialEq)]
struct ProjectionlessObservedProjection {
    entities: Vec<ProjectionlessObservedEntity>,
    input_focus: Option<Option<bevy_ecs::entity::Entity>>,
    route_focus: Option<Option<bevy_ecs::entity::Entity>>,
}

#[derive(Clone, Eq, PartialEq)]
struct ProjectionlessContinuationProjection {
    report: Option<ViewReportIdentity>,
    commit_identity_is_current: bool,
    admission: ProjectionlessFallbackAdmission,
    host: Option<ProjectionlessHostProjection>,
    host_binding_is_current: bool,
    active_frames: Vec<RouteFrameVersion>,
    active_claims: Vec<ExactRegistryClaim>,
    pending_frames: Vec<RouteFrameVersion>,
    pending_claims: Vec<ExactRegistryClaim>,
    pending_has_final_task: bool,
    observed: ProjectionlessObservedProjection,
}

struct ProjectionlessSourceDriftEvidence {
    expected: ProjectionlessContinuationProjection,
    observed: ProjectionlessContinuationProjection,
}

struct ProjectionlessFailCloseSource {
    reason: BeginAuthoritySettlementErrorKind,
    projection: ProjectionlessContinuationProjection,
}

enum ProjectionlessFailCloseContinuation {
    Retryable(ProjectionlessFailCloseSource),
}

pub(crate) struct AuthorityProtocolResidualOwner {
    back_target: Option<Box<PendingBackTargetExecution>>,
    kind: AuthorityProtocolResidualKind,
    isolation_contradiction: Option<AuthorityIsolationContradiction>,
    active: Option<RealizedBranch>,
    pending: Option<PendingCommit>,
    failed: Option<PendingFailedBranch>,
    failure_cleanup_completion: Option<FailureCleanupCompletionOwner>,
    failure_cleanup_ledger: Option<CleanupLedger>,
    cleanup_settlement_ledger: Option<CleanupLedger>,
    failure_cleanup_permanent_authority: Option<R1FailureCleanupPermanentAuthorityPair>,
    pub(crate) host: Option<RetiredHostBinding>,
}

#[cfg(test)]
impl AuthorityProtocolResidualOwner {
    pub(crate) fn retained_cleanup_settlement_ledger(&self) -> Option<&CleanupLedger> {
        self.cleanup_settlement_ledger.as_ref()
    }
}

enum AuthorityProtocolResidualKind {
    #[cfg(feature = "bsn")]
    NativeCreation(crate::presentation::native_scene::NativeSceneIsolationFailure),
    #[cfg(feature = "bsn")]
    NativeCleanup(crate::presentation::native_frame::NativeCleanupPreparationError),
    Authority(BeginAuthoritySettlementErrorKind),
    AuthorityIsolation(AuthorityIsolationContradiction),
    CleanupLedger(CleanupLedgerReserveErrorKind),
    AuthorityTrigger(FailedAuthorityTriggerError),
    FailedManifest(FailedPreparedTopologyError),
    PostReportSourceDrift(ProjectionlessSourceDriftEvidence),
    PreflightAuthorityLost(LifecyclePreflightEvidence),
    FailureCleanupCompletion(FailureCleanupCompletionMismatchView),
    CleanupStopped,
    FailureCleanupPermanentAuthority,
}

impl RouterEntry {
    #[cfg(feature = "bsn")]
    pub(crate) fn native_terminal_input_targets<'a>(
        &'a self,
        world: &'a World,
    ) -> impl Iterator<Item = Entity> + 'a {
        self.native_residuals
            .iter()
            .flat_map(move |residual| residual.input_targets(world))
    }

    #[cfg(feature = "bsn")]
    pub(crate) fn native_terminal_shell_claims(&self) -> impl Iterator<Item = &ExactRegistryClaim> {
        self.native_residuals
            .iter()
            .flat_map(|residual| residual.shell_claims())
    }

    pub(crate) fn install_back_capacity_issuer(
        &mut self,
        domains: &mut BackCapacityDomainIssuer,
    ) -> Result<(), BackCapacityIssuerInstallError> {
        if self.back_capacity_issuer.is_some() {
            return Err(BackCapacityIssuerInstallError::AlreadyInstalled);
        }
        let issuer = domains
            .issue()
            .map_err(BackCapacityIssuerInstallError::Reserve)?;
        self.back_capacity_issuer = Some(issuer);
        Ok(())
    }

    pub(crate) fn take_back_resolution_handoff(
        &mut self,
        handoff: BackResolutionOwnerHandoff,
    ) -> TakeBackResolutionOutcome {
        let evidence = handoff.evidence();
        let scope = handoff.scope();
        let reservation_index = self
            .back_capacity_reservations
            .iter()
            .position(|(_, reservation)| reservation.matches_resolution_scope(&scope))
            .expect("a Core back resolution action retains its exact accepted capacity owner");
        let (stored_request, reservation) =
            self.back_capacity_reservations.remove(reservation_index);
        let (request, capacity, accepted) = reservation.into_parts();
        debug_assert_eq!(stored_request, request);

        match self
            .engine
            .verify_back_resolution_handoff(accepted, handoff)
        {
            BackResolutionVerifyOutcome::Matched(verified) => {
                let (accepted, owner) = verified.into_parts();
                let reservation = BoundBackCapacityReservation::bind(request, capacity, accepted);
                let bound = match owner {
                    VerifiedBackResolutionOwnerHandoff::Target(owner) => {
                        BoundBackResolutionHandoff::Target(BoundBackTargetResolution {
                            owner,
                            reservation,
                        })
                    }
                    VerifiedBackResolutionOwnerHandoff::NoTarget(candidate) => {
                        BoundBackResolutionHandoff::NoTarget(BoundBackNoTargetResolution {
                            candidate,
                            reservation,
                        })
                    }
                    VerifiedBackResolutionOwnerHandoff::Contradiction(owner) => {
                        BoundBackResolutionHandoff::Contradiction(
                            BoundBackContradictionResolution { owner, reservation },
                        )
                    }
                };
                assert_eq!(
                    self.validate_back_resolution_evidence(&evidence),
                    BackResolutionEvidenceDisposition::ConsumedReplay,
                    "a matched Core back resolution must consume its exact saved evidence"
                );
                TakeBackResolutionOutcome::Matched(bound)
            }
            BackResolutionVerifyOutcome::Mismatch {
                accepted,
                handoff,
                evidence,
            } => TakeBackResolutionOutcome::Mismatch {
                reservation: BoundBackCapacityReservation::bind(request, capacity, accepted),
                handoff,
                evidence,
            },
        }
    }

    pub(crate) fn validate_back_resolution_evidence(
        &self,
        evidence: &BackResolutionEvidence,
    ) -> BackResolutionEvidenceDisposition {
        #[cfg(test)]
        self.back_resolution_evidence_predicate_calls
            .fetch_add(1, Ordering::Relaxed);

        match self.engine.classify_back_resolution_evidence(evidence) {
            BackResolutionEvidenceClassification::Current => {
                BackResolutionEvidenceDisposition::Current
            }
            BackResolutionEvidenceClassification::Foreign => {
                BackResolutionEvidenceDisposition::Foreign
            }
            BackResolutionEvidenceClassification::ConsumedReplay => {
                BackResolutionEvidenceDisposition::ConsumedReplay
            }
        }
    }

    fn restore_back_capacity_reservation(&mut self, reservation: BoundBackCapacityReservation) {
        self.back_capacity_reservations
            .push((reservation.request(), reservation));
    }

    fn restore_back_resolution_mismatch(
        &mut self,
        reservation: BoundBackCapacityReservation,
        handoff: BackResolutionOwnerHandoff,
        evidence: BackResolutionEvidence,
    ) {
        self.restore_back_capacity_reservation(reservation);
        self.pending_back_resolution_action = Some((handoff, evidence));
    }

    pub(crate) fn publish_history_admission_stamp(&mut self) {
        self.history_admission_stamp = self
            .engine
            .history_admission_stamp(&self.history_admission_world)
            .ok();
    }

    pub(crate) fn current_reconciliation_entering_branch(&self) -> Option<&RealizedBranch> {
        let pending = self.pending_commit.as_ref()?;
        let projection_is_current = if let Some(ticket) = pending.ticket.as_ref() {
            self.engine
                .reconciliation_projection_for_commit(ticket)
                .is_ok()
        } else if let Some(task) = pending.final_task.as_ref() {
            self.engine.reconciliation_projection_for_task(task).is_ok()
        } else if let Some(task) = self.pending_mount_frame_task.as_ref() {
            self.engine
                .reconciliation_projection_for_task(task)
                .ok()
                .and_then(|projection| {
                    seal_reconciliation_staging_mount(
                        &projection,
                        self.active.as_ref(),
                        &pending.branch,
                        task,
                    )
                })
                .is_some_and(|mount| {
                    mount.role() == crate::branch_transaction::ReconciliationSlotRoleKind::MountNew
                        && mount.expected_slot_lifecycle() == EntityLifecycle::PreparedInert
                })
        } else if let Some(request) = self.pending_mount_request.as_ref() {
            self.engine
                .reconciliation_projection_for_mount_request(request)
                .is_ok()
        } else {
            self.engine
                .reconciliation_projection_for_pending_mount()
                .is_ok()
        };
        projection_is_current.then_some(pending.branch.as_realized())
    }

    pub(crate) fn current_mount_new_staging_allowance(
        &self,
    ) -> Option<&SealedMountNewStagingAllowance> {
        if let Some(allowance) = self
            .pending_mount_mutation_rejection
            .as_ref()
            .and_then(|pending| pending.allowance.as_ref())
        {
            return Some(allowance);
        }
        if let Some(failed) = self.pending_failed_branch.as_ref() {
            let allowance = failed.branch.mount_new_allowance()?;
            let terminal_is_exact = match &failed.terminal {
                PendingFailedBranchTerminal::Request(request)
                | PendingFailedBranchTerminal::BackTargetPendingRejected { request, .. } => {
                    allowance.matches_request(request.request())
                }
                PendingFailedBranchTerminal::RegistrationBegin(_) => true,
                PendingFailedBranchTerminal::Task { .. }
                | PendingFailedBranchTerminal::TaskWithPhysicalResidual { .. } => false,
            };
            return terminal_is_exact.then_some(allowance);
        }
        if let Some(permanent) = self.failure_cleanup_permanent.as_ref() {
            return permanent.physical().branch.mount_new_allowance();
        }
        self.cleanup_ledger
            .as_ref()
            .and_then(CleanupLedger::mount_new_staging_allowance)
    }

    pub(crate) fn cleanup_summary(&self) -> bevy_ui_router_core::CleanupSummary {
        self.cleanup_observation.summary(
            self.idle_host_loss.is_some()
                || self.cleanup_ledger.is_some()
                || self.post_commit_compound.is_some()
                || self.pending_failed_branch.is_some()
                || self.failure_cleanup_permanent.is_some()
                || self.pending_mount_mutation_rejection.is_some()
                || self.host_settlement.is_some()
                || self.pending_host_admission_failure.is_some()
                || self.pending_host_confirmation.is_some()
                || self.pending_back_no_target_completion.is_some()
                || self.pending_back_no_target_preparation.is_some()
                || self.pending_back_no_target_authority_cleanup.is_some(),
        )
    }

    fn observe_cleanup_receipts(&mut self, ledger: &mut CleanupLedger) -> bool {
        while let Some((obligation, disposition)) = ledger.next_unobserved_receipt() {
            if let Some(batch) = ledger.batch()
                && self
                    .engine
                    .observe_cleanup_receipt(batch, disposition.public())
                    .is_err()
            {
                return false;
            }
            if !ledger.mark_receipt_observed(obligation, disposition) {
                return false;
            }
            let terminal_quarantine = ledger.mark_quarantine_event_observed(disposition);
            self.cleanup_observation
                .record(disposition, terminal_quarantine);
        }
        true
    }
}

#[derive(Resource)]
pub struct RouterNavigation {
    domain: u64,
    next_router: u64,
    back_capacity_domain_issuer: BackCapacityDomainIssuer,
    pub(crate) entries: Vec<RouterEntry>,
}

impl RouterNavigation {
    pub(crate) fn new() -> Self {
        static NEXT_DOMAIN: AtomicU64 = AtomicU64::new(1);

        let domain =
            allocate_navigation_domain(&NEXT_DOMAIN).expect("router navigation domain exhausted");
        Self {
            domain,
            next_router: 1,
            back_capacity_domain_issuer: BackCapacityDomainIssuer::new(),
            entries: Vec::new(),
        }
    }

    /// Arms the test-only timing seam that removes the next committed host's real
    /// registration after Core has sealed the commit and before ECS publication.
    #[cfg(test)]
    pub(crate) fn testing_remove_host_registration_after_next_commit_seal(
        &mut self,
        router: RouterHandle,
    ) -> bool {
        let Some(entry) = self.entries.iter_mut().find(|entry| entry.handle == router) else {
            return false;
        };
        entry.commit_sealed_registration_removal_armed = true;
        entry.commit_sealed_registration_removal_fired = false;
        true
    }

    /// Reports whether the test-only post-seal/pre-publication seam removed the
    /// real host registration for this router.
    #[cfg(test)]
    pub(crate) fn testing_commit_sealed_registration_removal_fired(
        &self,
        router: RouterHandle,
    ) -> bool {
        self.entries
            .iter()
            .find(|entry| entry.handle == router)
            .is_some_and(|entry| entry.commit_sealed_registration_removal_fired)
    }

    pub(crate) fn install(
        &mut self,
        routes: bevy_ui_router_core::prelude::RouteTree,
        views: RouteViewRegistry,
        fallback: bevy_ui_router_core::prelude::RouteViewKey,
        keep_alive_capacity: usize,
    ) -> Result<RouterHandle, IntegrationError> {
        let handle = RouterHandle::from_parts(self.domain, self.next_router);
        self.next_router = self
            .next_router
            .checked_add(1)
            .ok_or(IntegrationError::Exhausted)?;
        let mut engine =
            RouterEngine::install(routes, fallback)?.with_keep_alive_capacity(keep_alive_capacity);
        let history_admission_world = engine.claim_history_admission_world()?;
        let mut entry = RouterEntry {
            handle,
            engine,
            views,
            host: None,
            host_settlement: None,
            pending_host_admission_failure: None,
            pending_host_confirmation: None,
            pending_commit: None,
            pending_frame_drive: None,
            #[cfg(feature = "bsn")]
            native_residuals: Vec::new(),
            pending_mount_frame_task: None,
            pending_mount_request: None,
            pending_mount_mutation_rejection: None,
            pending_failed_branch: None,
            failure_cleanup_permanent: None,
            projectionless_fail_close: None,
            active: None,
            scroll: crate::scroll::ScrollState::default(),
            retained: crate::retained::RetainedLeaves::default(),
            cleanup_ledger: None,
            post_commit_compound: None,
            settled_retired_host_evidence: None,
            idle_host_loss: None,
            idle_host_terminal: None,
            deferred_host_observation: None,
            #[cfg(test)]
            commit_sealed_registration_removal_armed: false,
            #[cfg(test)]
            commit_sealed_registration_removal_fired: false,
            failure_cleanup_completion: None,
            cleanup_observation: RouterCleanupObservation::default(),
            authority_protocol_residual: None,
            back_capacity_enqueue_retire_residual: None,
            requests: Vec::new(),
            back_capacity_reservations: Vec::new(),
            pending_back_resolution_action: None,
            pending_bound_back_resolution: None,
            pending_back_target_execution: None,
            pending_back_no_target_completion: None,
            pending_back_no_target_preparation: None,
            pending_back_no_target_authority_cleanup: None,
            pending_back_target_permanent_cleanup: None,
            history_admission_world,
            history_admission_stamp: None,
            back_capacity_issuer: None,
            #[cfg(test)]
            back_resolution_evidence_predicate_calls: AtomicUsize::new(0),
            #[cfg(test)]
            back_target_driver_failure_dispatch_calls: AtomicUsize::new(0),
            #[cfg(test)]
            back_target_atomic_applied_dispatch_calls: AtomicUsize::new(0),
            #[cfg(test)]
            back_target_permanent_begin_callback_calls: AtomicUsize::new(0),
            host_admission_operation_counts: (0, 0, 0, 0),
            #[cfg(test)]
            host_confirmation_receiver_drift_before_confirm_armed: false,
            #[cfg(test)]
            host_confirmation_preparation_receiver_drift_before_prepare_armed: false,
            #[cfg(test)]
            back_target_driver_staging_drift_armed: false,
            #[cfg(test)]
            back_target_driver_report_receiver_drift_armed: false,
            #[cfg(test)]
            back_target_driver_consume_receiver_drift_armed: false,
            #[cfg(test)]
            failure_cleanup_completion_preparation_drift_armed: false,
            #[cfg(test)]
            failure_cleanup_completion_receiver_drift_armed: false,
            #[cfg(test)]
            back_no_target_completion_fact_drift: None,
            #[cfg(test)]
            back_capacity_facade_producer: None,
        };
        entry
            .install_back_capacity_issuer(&mut self.back_capacity_domain_issuer)
            .map_err(|error| match error {
                BackCapacityIssuerInstallError::AlreadyInstalled => IntegrationError::Invalid,
                BackCapacityIssuerInstallError::Reserve(_) => IntegrationError::Exhausted,
            })?;
        self.entries.push(entry);
        Ok(handle)
    }

    pub fn push(
        &mut self,
        router: RouterHandle,
        location: RouteLocation,
    ) -> Result<NavigationRequestId, NavigationEnqueueError> {
        let entry = self
            .entries
            .iter_mut()
            .find(|entry| entry.handle == router)
            .ok_or_else(|| {
                NavigationEnqueueError::new(NavigationEnqueueErrorKind::UnknownRouter)
            })?;
        if entry.authority_protocol_residual.is_some()
            || entry
                .cleanup_ledger
                .as_ref()
                .is_some_and(CleanupLedger::is_stopped)
        {
            return Err(NavigationEnqueueError::new(
                NavigationEnqueueErrorKind::HostUnavailable,
            ));
        }
        let request = entry
            .engine
            .push(location)
            .map_err(map_navigation_enqueue_error)?;
        entry.requests.push(request);
        Ok(request)
    }

    pub fn replace(
        &mut self,
        router: RouterHandle,
        location: RouteLocation,
    ) -> Result<NavigationRequestId, NavigationEnqueueError> {
        let entry = self
            .entries
            .iter_mut()
            .find(|entry| entry.handle == router)
            .ok_or_else(|| {
                NavigationEnqueueError::new(NavigationEnqueueErrorKind::UnknownRouter)
            })?;
        if entry.authority_protocol_residual.is_some()
            || entry
                .cleanup_ledger
                .as_ref()
                .is_some_and(CleanupLedger::is_stopped)
        {
            return Err(NavigationEnqueueError::new(
                NavigationEnqueueErrorKind::HostUnavailable,
            ));
        }
        let request = entry
            .engine
            .replace(location)
            .map_err(map_navigation_enqueue_error)?;
        entry.requests.push(request);
        Ok(request)
    }

    pub fn back(
        &mut self,
        router: RouterHandle,
    ) -> Result<NavigationRequestId, NavigationEnqueueError> {
        let entry_index = self
            .entries
            .iter()
            .position(|entry| entry.handle == router)
            .ok_or_else(|| {
                NavigationEnqueueError::new(NavigationEnqueueErrorKind::UnknownRouter)
            })?;
        let result = (|| {
            let admission = {
                let entry = &self.entries[entry_index];
                if entry.back_capacity_enqueue_retire_residual.is_some() {
                    return Err(NavigationEnqueueError::new(
                        NavigationEnqueueErrorKind::HostUnavailable,
                    ));
                }
                match entry.engine.snapshot().availability() {
                    RouterAvailability::Ready => {}
                    RouterAvailability::RecoveringAuthority => {
                        return Err(NavigationEnqueueError::new(
                            NavigationEnqueueErrorKind::AuthorityRecovering,
                        ));
                    }
                    RouterAvailability::AwaitingHost | RouterAvailability::Unavailable(_) => {
                        return Err(NavigationEnqueueError::new(
                            NavigationEnqueueErrorKind::HostUnavailable,
                        ));
                    }
                }
                if entry.authority_protocol_residual.is_some()
                    || entry
                        .cleanup_ledger
                        .as_ref()
                        .is_some_and(CleanupLedger::is_stopped)
                {
                    return Err(NavigationEnqueueError::new(
                        NavigationEnqueueErrorKind::HostUnavailable,
                    ));
                }
                let stamp = entry.history_admission_stamp.as_ref().ok_or_else(|| {
                    NavigationEnqueueError::new(NavigationEnqueueErrorKind::AuthorityRecovering)
                })?;
                entry
                    .engine
                    .prepare_back_admission(&entry.history_admission_world, stamp)
                    .map_err(|error| {
                        NavigationEnqueueError::new(match error {
                            PrepareBackAdmissionErrorKind::NoBackEntry => {
                                NavigationEnqueueErrorKind::NoBackEntry
                            }
                            PrepareBackAdmissionErrorKind::ForeignStamp
                            | PrepareBackAdmissionErrorKind::AuthorityRecovering => {
                                NavigationEnqueueErrorKind::AuthorityRecovering
                            }
                        })
                    })?
            };

            let entry = &mut self.entries[entry_index];
            entry.requests.try_reserve(1).map_err(|_| {
                NavigationEnqueueError::new(NavigationEnqueueErrorKind::IdentityExhausted)
            })?;
            entry
                .back_capacity_reservations
                .try_reserve(1)
                .map_err(|_| {
                    NavigationEnqueueError::new(NavigationEnqueueErrorKind::IdentityExhausted)
                })?;
            let permit = entry
                .back_capacity_issuer
                .as_mut()
                .expect("installed RouterEntry retains its exact back capacity issuer")
                .reserve()
                .map_err(|_| {
                    NavigationEnqueueError::new(NavigationEnqueueErrorKind::IdentityExhausted)
                })?;
            let accepted = match entry.engine.enqueue_back(admission) {
                Ok(accepted) => accepted,
                Err(error) => {
                    let (kind, rejection) = match error {
                        EnqueueBackErrorKind::AuthorityRecovering => (
                            NavigationEnqueueErrorKind::AuthorityRecovering,
                            EnqueueRejected::CoreTokenDrift,
                        ),
                        EnqueueBackErrorKind::Exhausted => (
                            NavigationEnqueueErrorKind::IdentityExhausted,
                            EnqueueRejected::CoreAllocatorFailure(error),
                        ),
                    };
                    match entry
                        .back_capacity_issuer
                        .as_mut()
                        .expect("installed RouterEntry retains its exact back capacity issuer")
                        .retire_unbound(permit, rejection)
                    {
                        BackCapacityRetireOutcome::Retired(_evidence) => {
                            return Err(NavigationEnqueueError::new(kind));
                        }
                        BackCapacityRetireOutcome::Rejected {
                            owner,
                            rejection,
                            error,
                        } => {
                            entry.back_capacity_enqueue_retire_residual =
                                Some(BackCapacityEnqueueRetireResidual {
                                    owner,
                                    rejection,
                                    retire_error: error,
                                });
                            return Err(NavigationEnqueueError::new(
                                NavigationEnqueueErrorKind::HostUnavailable,
                            ));
                        }
                    }
                }
            };
            let (request, execution) = accepted.into_parts();
            let bound = BoundBackCapacityReservation::bind(request, permit, execution);
            entry.back_capacity_reservations.push((request, bound));
            entry.requests.push(request);
            Ok(request)
        })();

        #[cfg(test)]
        if result.is_err() {
            if let Some(producer) = self.entries[entry_index]
                .back_capacity_facade_producer
                .as_mut()
            {
                producer.typed_return();
            }
        }

        result
    }
}

#[cfg(test)]
pub(crate) fn install_back_capacity_order_observer_with<F>(
    navigation: &mut RouterNavigation,
    router: RouterHandle,
    factory: F,
) -> Result<
    bevy_ui_router_core::test_support::BackCapacityOrderObserver,
    crate::test_support::BackCapacityOrderInstallError,
>
where
    F: FnOnce() -> (
        bevy_ui_router_core::test_support::BackCapacityOrderObserver,
        bevy_ui_router_core::test_support::BackCapacityIssuerProducer,
        bevy_ui_router_core::test_support::BackCapacityCoreProducer,
        bevy_ui_router_core::test_support::BackCapacityFacadeProducer,
    ),
{
    let domain_witness = RouterHandle::from_parts(navigation.domain, 0);
    if !router.is_same_domain_as(domain_witness) {
        return Err(crate::test_support::BackCapacityOrderInstallError::ForeignRouter);
    }

    let entry_index = navigation
        .entries
        .iter()
        .position(|entry| entry.handle == router)
        .ok_or(crate::test_support::BackCapacityOrderInstallError::UnknownRouter)?;

    let RouterEntry {
        engine,
        back_capacity_issuer,
        back_capacity_facade_producer,
        ..
    } = &mut navigation.entries[entry_index];
    let core_slot = engine
        .prepare_back_capacity_core_producer_slot()
        .ok_or(crate::test_support::BackCapacityOrderInstallError::AlreadyInstalled)?;
    let issuer_slot = back_capacity_issuer
        .as_mut()
        .filter(|issuer| issuer.back_capacity_order_producer_slot_is_vacant())
        .ok_or(crate::test_support::BackCapacityOrderInstallError::AlreadyInstalled)?;
    if back_capacity_facade_producer.is_some() {
        return Err(crate::test_support::BackCapacityOrderInstallError::AlreadyInstalled);
    }

    let (observer, issuer, core, facade) = factory();
    core_slot.install(core);
    issuer_slot.install_back_capacity_order_producer(issuer);
    *back_capacity_facade_producer = Some(facade);
    Ok(observer)
}

fn map_navigation_enqueue_error(error: IntegrationError) -> NavigationEnqueueError {
    let kind = match error {
        IntegrationError::AwaitingHost => NavigationEnqueueErrorKind::HostUnavailable,
        IntegrationError::Exhausted => NavigationEnqueueErrorKind::IdentityExhausted,
        IntegrationError::OutstandingAdmission
        | IntegrationError::AuthorityExists
        | IntegrationError::ForeignRouter
        | IntegrationError::Duplicate
        | IntegrationError::Stale
        | IntegrationError::Invalid
        | IntegrationError::InvalidManifest => NavigationEnqueueErrorKind::AuthorityRecovering,
    };
    NavigationEnqueueError::new(kind)
}

#[cfg(test)]
impl RouterEntry {
    fn testing_host_admission_operation_counts(&self) -> (usize, usize, usize, usize) {
        self.host_admission_operation_counts
    }

    fn testing_host_confirmation_whole_owner_projection(
        &self,
    ) -> TestingHostConfirmationWholeOwnerProjection {
        if let Some(HostAdmissionFailureHolder::Provisional(holder)) =
            &self.pending_host_admission_failure
        {
            return TestingHostConfirmationWholeOwnerProjection {
                stage: TestingHostConfirmationWholeOwnerStage::ProvisionalWholeOwner,
                candidate: Some(holder.candidate.candidate.clone()),
                grant_version: Some(holder.grant.version().clone()),
                grant_top_slot: Some(holder.grant.top_slot()),
                slot: Some(holder.ledger.slot()),
                claim: Some(holder.ledger.claim()),
                receiver: holder.receiver.as_ref().map(|evidence| {
                    TestingHostConfirmationReceiverEvidenceProjection {
                        expected: evidence.expected.clone(),
                        observed: evidence.observed.clone(),
                    }
                }),
            };
        }
        if let Some(HostAdmissionFailureHolder::ConfirmingInert(holder)) =
            &self.pending_host_admission_failure
        {
            let receiver = match &holder.evidence {
                r3_core::HostAdmissionConfirmationFailureEvidence::Receiver(evidence) => {
                    Some(TestingHostConfirmationReceiverEvidenceProjection {
                        expected: evidence.expected.clone(),
                        observed: evidence.observed.clone(),
                    })
                }
                r3_core::HostAdmissionConfirmationFailureEvidence::Issuer(_)
                | r3_core::HostAdmissionConfirmationFailureEvidence::TopSlotRegistrationOperationRejected(
                    _,
                )
                | r3_core::HostAdmissionConfirmationFailureEvidence::TopSlotRegistrationRejected(
                    _,
                )
                | r3_core::HostAdmissionConfirmationFailureEvidence::ConfirmationPreparationRejected(
                    _,
                )
                | r3_core::HostAdmissionConfirmationFailureEvidence::ConfirmationPublicationRejected(
                    _,
                ) => None,
            };
            return TestingHostConfirmationWholeOwnerProjection {
                stage: TestingHostConfirmationWholeOwnerStage::ConfirmingUnwindMismatchInert,
                candidate: Some(holder.candidate.clone()),
                grant_version: Some(holder.grant_version.clone()),
                grant_top_slot: Some(holder.grant_top_slot),
                slot: Some(holder.slot),
                claim: Some(holder.claim.clone()),
                receiver,
            };
        }
        if let Some(settlement) = &self.host_settlement {
            return TestingHostConfirmationWholeOwnerProjection {
                stage: TestingHostConfirmationWholeOwnerStage::LegacyProvisionalSettlement,
                candidate: None,
                grant_version: None,
                grant_top_slot: None,
                slot: Some(settlement.slot()),
                claim: Some(settlement.claim()),
                receiver: None,
            };
        }
        TestingHostConfirmationWholeOwnerProjection {
            stage: if self.pending_host_admission_failure.is_some() {
                TestingHostConfirmationWholeOwnerStage::OtherAdmissionFailure
            } else {
                TestingHostConfirmationWholeOwnerStage::Absent
            },
            candidate: None,
            grant_version: None,
            grant_top_slot: None,
            slot: None,
            claim: None,
            receiver: None,
        }
    }
}

#[cfg(test)]
impl RouterNavigation {
    pub(crate) fn pending_shape_for_test(&self) -> Option<(usize, usize, bool)> {
        self.entries
            .first()?
            .pending_commit
            .as_ref()
            .map(|pending| {
                (
                    pending.branch.frames().len(),
                    pending.branch.child_slots().len(),
                    pending.final_task.is_some(),
                )
            })
    }

    pub(crate) fn stale_pending_projection_for_test(&mut self) {
        let entry = self
            .entries
            .first_mut()
            .expect("test router entry must remain installed");
        let pending = entry
            .pending_commit
            .as_mut()
            .expect("test pending branch must remain staged");
        let task = pending
            .final_task
            .take()
            .expect("test pending branch must retain its final task authority");
        let _ = entry.engine.report_failed(
            task,
            ViewFailureKind::Apply,
            "test consumes the pending projection authority",
        );
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NavigationEnqueueErrorKind {
    UnknownRouter,
    HostUnavailable,
    AuthorityRecovering,
    NoBackEntry,
    IdentityExhausted,
}

#[derive(Debug)]
pub struct NavigationEnqueueError {
    kind: NavigationEnqueueErrorKind,
}

impl NavigationEnqueueError {
    const fn new(kind: NavigationEnqueueErrorKind) -> Self {
        Self { kind }
    }

    pub fn kind(&self) -> NavigationEnqueueErrorKind {
        self.kind
    }
}

impl std::fmt::Display for NavigationEnqueueError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self.kind {
            NavigationEnqueueErrorKind::UnknownRouter => "router is not installed",
            NavigationEnqueueErrorKind::HostUnavailable => "router host is unavailable",
            NavigationEnqueueErrorKind::AuthorityRecovering => "router authority is recovering",
            NavigationEnqueueErrorKind::NoBackEntry => "router history has no back entry",
            NavigationEnqueueErrorKind::IdentityExhausted => {
                "router navigation identity capacity is exhausted"
            }
        })
    }
}

impl std::error::Error for NavigationEnqueueError {}

/// The earliest local observation not yet submitted to Core. This is comparison
/// evidence only: the current candidate is always rebuilt from World.
#[derive(Clone, Debug, PartialEq, Eq)]
struct DeferredHostObservation {
    previous: Option<HostCandidateObservationSource>,
}

#[derive(Clone, Copy)]
enum HostObservationDelivery {
    DeferredBeforeCore,
    CoreHandled,
    OwnerTransferred,
}

impl HostObservationDelivery {
    fn after_core(entry: &RouterEntry) -> Self {
        if entry.pending_host_admission_failure.is_some()
            || entry.host.is_some()
            || entry.pending_host_confirmation.is_some()
        {
            Self::OwnerTransferred
        } else {
            Self::CoreHandled
        }
    }

    fn reconcile(self, entry: &mut RouterEntry) {
        if !matches!(self, Self::DeferredBeforeCore) {
            entry.deferred_host_observation = None;
        }
    }
}

pub(crate) fn observe_authority_system(world: &mut World) {
    let count = world.resource::<RouterNavigation>().entries.len();
    for index in 0..count {
        world.resource_scope(|world, mut revisions: bevy_ecs::change_detection::Mut<HostCandidateRevisionState>| {
        world.resource_scope(|world, mut latch: bevy_ecs::change_detection::Mut<HostCandidateRevisionLatch>| {
            world.resource_scope(|world, mut navigation: bevy_ecs::change_detection::Mut<RouterNavigation>| {
                'entry_work: {
                    let entry = &mut navigation.entries[index];
                    if entry.pending_host_confirmation.is_some() {
                        break 'entry_work;
                    }
                    if idle_host_loss::observe(world, entry) {
                        break 'entry_work;
                    }
                    if entry.pending_host_admission_failure.is_some() {
                        drive_pending_r3_host_admission_failure(world, entry);
                        break 'entry_work;
                    }
                    if entry.host_settlement.is_some() {
                        break 'entry_work;
                    }
                    if entry.host.is_some()
                        || entry.post_commit_compound.is_some()
                        || entry.authority_protocol_residual.is_some()
                        || entry.cleanup_ledger.as_ref().is_some_and(CleanupLedger::is_stopped)
                        || entry
                            .cleanup_ledger
                            .as_ref()
                            .is_some_and(CleanupLedger::is_authority_settlement)
                        || entry.pending_back_no_target_authority_cleanup.is_some()
                        || entry.pending_back_no_target_preparation.is_some()
                    {
                        break 'entry_work;
                    }
                    let previous_candidate_source = match revisions.current_source(entry.handle) {
                        Ok(source) => source,
                        Err(_) => break 'entry_work,
                    };
                    let previous_candidate_source = entry.deferred_host_observation
                        .get_or_insert(DeferredHostObservation { previous: previous_candidate_source })
                        .previous.clone();
                    let classification =
                        classify_host_candidate(world, entry.handle, &mut revisions);
                    let candidate = match classification {
                        HostCandidateClassification::None(source) => {
                            if entry.engine.snapshot().availability()
                                != &RouterAvailability::AwaitingHost
                            {
                                drive_observed_candidate_rejection(
                                    entry,
                                    source.clone(),
                                    R3HostAdmissionUnavailableEvidence::NoCandidate(source),
                                    previous_candidate_source.as_ref().map(|source| source.digest()),
                                );
                            } else {
                                entry.deferred_host_observation = None;
                            }
                            break 'entry_work;
                        }
                        HostCandidateClassification::InvalidTopology(candidate) => {
                            let source = candidate.observation_source();
                            drive_observed_candidate_rejection(
                                entry,
                                source.clone(),
                                R3HostAdmissionUnavailableEvidence::InvalidTopology(source),
                                previous_candidate_source.as_ref().map(|source| source.digest()),
                            );
                            break 'entry_work;
                        }
                        HostCandidateClassification::Conflict(candidate) => {
                            let source = candidate.observation_source();
                            drive_observed_candidate_rejection(
                                entry,
                                source.clone(),
                                R3HostAdmissionUnavailableEvidence::Conflict(source),
                                previous_candidate_source.as_ref().map(|source| source.digest()),
                            );
                            break 'entry_work;
                        }
                        HostCandidateClassification::RetryableCapacity(_) => break 'entry_work,
                        HostCandidateClassification::IdentityExhausted {
                            source,
                            counter,
                            current,
                            attempted,
                        } => {
                            drive_observed_candidate_rejection(
                                entry,
                                source.clone(),
                                R3HostAdmissionUnavailableEvidence::IdentityExhausted {
                                    source,
                                    counter,
                                    current,
                                    attempted,
                                },
                                previous_candidate_source.as_ref().map(|source| source.digest()),
                            );
                            break 'entry_work;
                        }
                        HostCandidateClassification::Unique(candidate) => candidate,
                    };
                    if !latch.allows(candidate.revision()) {
                        // This exact revision already has a delivered rejection.
                        entry.deferred_host_observation = None;
                        break 'entry_work;
                    }
                    let prepared_rejection = match latch
                        .prepare_rejection(entry.handle, candidate.revision())
                    {
                        Ok(prepared) => prepared,
                        Err(_) => break 'entry_work,
                    };
                    #[cfg(test)]
                    if let Some(owner) = entry.engine.testing_take_armed_host_admission_owner() {
                        drive_armed_source_bound_host_admission(
                            entry,
                            PreparedEcsHostCandidateSource::prepare(candidate),
                            owner,
                        );
                        entry.deferred_host_observation = None;
                        break 'entry_work;
                    }
                    let delivery = drive_r3_unique_host_candidate(
                        world,
                        entry,
                        PreparedEcsHostCandidateSource::prepare(candidate),
                        previous_candidate_source.as_ref().map(|source| source.digest()),
                        &mut latch,
                        prepared_rejection,
                    );
                    if let Ok(delivery) = delivery {
                        delivery.reconcile(entry);
                    }
                }
            });
        });
    });
        complete_pending_host_confirmations(world);
    }
    idle_host_loss::complete_ready(world);
}

fn r3_unavailable_projection(
    core_source: &r3_core::HostAdmissionSourceProjection,
    evidence: &R3HostAdmissionUnavailableEvidence,
    expected_observation_digest: Option<u64>,
) -> r3_core::HostAdmissionUnavailableEvidenceProjection {
    match evidence {
        R3HostAdmissionUnavailableEvidence::NoCandidate(source) => {
            r3_core::HostAdmissionUnavailableEvidenceProjection::no_candidate(
                core_source,
                Some(source.revision().raw()),
                expected_observation_digest.unwrap_or_else(|| source.digest()),
                source.digest(),
                None,
            )
        }
        R3HostAdmissionUnavailableEvidence::Conflict(source) => {
            r3_core::HostAdmissionUnavailableEvidenceProjection::conflict(
                core_source,
                Some(source.revision().raw()),
                expected_observation_digest.unwrap_or_else(|| source.digest()),
                source.digest(),
            )
        }
        R3HostAdmissionUnavailableEvidence::InvalidTopology(source) => {
            r3_core::HostAdmissionUnavailableEvidenceProjection::invalid_topology(
                core_source,
                Some(source.revision().raw()),
                expected_observation_digest.unwrap_or_else(|| source.digest()),
                source.digest(),
            )
        }
        R3HostAdmissionUnavailableEvidence::IdentityExhausted {
            source,
            counter,
            current,
            attempted,
        } => r3_core::HostAdmissionUnavailableEvidenceProjection::authority_establishment_failed(
            core_source,
            Some(source.revision().raw()),
            expected_observation_digest.unwrap_or_else(|| source.digest()),
            source.digest(),
            Some(r3_core::HostAdmissionAllocatorExhaustionEvidence {
                counter: match counter {
                    HostCandidateIdentityCounter::RegistrationIncarnation => {
                        r3_core::HostAdmissionAllocatorCounter::RegistrationIncarnation
                    }
                    HostCandidateIdentityCounter::CandidateRevision => {
                        r3_core::HostAdmissionAllocatorCounter::CandidateRevision
                    }
                },
                current: *current,
                attempted: *attempted,
            }),
        ),
        R3HostAdmissionUnavailableEvidence::HostLost(source) => {
            r3_core::HostAdmissionUnavailableEvidenceProjection::host_lost(
                core_source,
                Some(source.revision().raw()),
                expected_observation_digest.unwrap_or_else(|| source.digest()),
                source.digest(),
            )
        }
        R3HostAdmissionUnavailableEvidence::Preallocation(source) => {
            r3_core::HostAdmissionUnavailableEvidenceProjection::registry_invariant_violation(
                core_source,
                Some(source.revision().raw()),
                source.digest(),
                source.digest(),
            )
        }
        R3HostAdmissionUnavailableEvidence::Confirmation { source, evidence } => {
            r3_core::HostAdmissionUnavailableEvidenceProjection::confirmation_failed(
                core_source,
                Some(source.revision().raw()),
                evidence.expected_digest(),
                evidence.observed_digest(),
            )
        }
        R3HostAdmissionUnavailableEvidence::AuthorityEstablishment(source) => {
            r3_core::HostAdmissionUnavailableEvidenceProjection::authority_establishment_failed(
                core_source,
                Some(source.revision().raw()),
                expected_observation_digest.unwrap_or_else(|| source.digest()),
                source.digest(),
                None,
            )
        }
        R3HostAdmissionUnavailableEvidence::PostAllocation { source, evidence } => {
            r3_core::HostAdmissionUnavailableEvidenceProjection::authority_establishment_failed(
                core_source,
                Some(source.revision().raw()),
                evidence.expected_digest(),
                evidence.observed_digest(),
                None,
            )
        }
        R3HostAdmissionUnavailableEvidence::RetainedAdmission { source, evidence } => {
            r3_core::HostAdmissionUnavailableEvidenceProjection::authority_establishment_failed(
                core_source,
                Some(source.revision().raw()),
                evidence.expected_digest(),
                evidence.observed_digest(),
                None,
            )
        }
        R3HostAdmissionUnavailableEvidence::PostScopePhysical { source, evidence } => {
            r3_core::HostAdmissionUnavailableEvidenceProjection::authority_establishment_failed(
                core_source,
                Some(source.revision().raw()),
                evidence.expected_digest(),
                evidence.observed_digest(),
                None,
            )
        }
    }
}

fn complete_observed_r3_unavailable(
    entry: &mut RouterEntry,
    source: r3_core::HostAdmissionSourceProjection,
    evidence: R3HostAdmissionUnavailableEvidence,
    expected_observation_digest: Option<u64>,
) -> bool {
    let projection = r3_unavailable_projection(&source, &evidence, expected_observation_digest);
    entry.host_admission_operation_counts.0 += 1;
    match entry
        .engine
        .bind_host_admission_unavailable_cause(source, &projection, evidence)
    {
        r3_core::HostAdmissionUnavailableCauseBindOutcome::Bound(prepared) => {
            match entry
                .engine
                .complete_observed_host_admission_unavailable(prepared, || {})
            {
                r3_core::HostAdmissionUnavailableObservedOutcome::Published(_) => {
                    entry.host_admission_operation_counts.1 += 1;
                    entry.host_admission_operation_counts.2 += 1;
                    return true;
                }
                r3_core::HostAdmissionUnavailableObservedOutcome::Stable { .. } => {
                    entry.host_admission_operation_counts.1 += 1;
                }
                r3_core::HostAdmissionUnavailableObservedOutcome::Awaiting { .. } => {
                    entry.host_admission_operation_counts.1 += 1;
                }
                r3_core::HostAdmissionUnavailableObservedOutcome::Mismatch {
                    prepared,
                    evidence,
                } => {
                    entry.pending_host_admission_failure =
                        Some(HostAdmissionFailureHolder::PublicationInert(
                            HostAdmissionPublicationInert::Observed { prepared, evidence },
                        ));
                }
            }
        }
        r3_core::HostAdmissionUnavailableCauseBindOutcome::Mismatch {
            source,
            evidence,
            mismatch,
            ..
        } => {
            entry.pending_host_admission_failure =
                Some(HostAdmissionFailureHolder::CauseBindObservedInert {
                    source,
                    evidence,
                    mismatch,
                });
        }
    }
    false
}

fn settled_retired_host_allows_candidate(
    world: &World,
    evidence: &SettledRetiredHostEvidence,
    candidate: &PreparedEcsHostCandidateSource,
) -> bool {
    if candidate.candidate.host() != evidence.host.host {
        return true;
    }
    let allowed = candidate.registration_incarnation != evidence.host.registration_incarnation
        && evidence.edge.host == evidence.host.host
        && evidence.edge.top_slot == evidence.host.top_slot
        && evidence.edge.top_slot_claim == evidence.host.top_slot_claim
        && evidence.edge.registration_incarnation == evidence.host.registration_incarnation
        && matches!(
            evidence.edge_terminal,
            RetiredHostEdgePhysicalTerminal::Detached
                | RetiredHostEdgePhysicalTerminal::HostAbsent
                | RetiredHostEdgePhysicalTerminal::EdgeAlreadyAbsent
        )
        && (world.get_entity(evidence.host.top_slot).is_err()
            || world
                .get::<ChildOf>(evidence.host.top_slot)
                .is_none_or(|parent| parent.parent() != evidence.host.host));
    allowed
}

fn complete_pre_staging_r3_unavailable(
    entry: &mut RouterEntry,
    owner: r3_core::HostAdmissionUnavailablePreStagingOwner,
    source_evidence: HostCandidateObservationSource,
    evidence: R3HostAdmissionUnavailableEvidence,
) {
    let core_source = match &owner {
        r3_core::HostAdmissionUnavailablePreStagingOwner::Permit(permit) => {
            permit.source_projection()
        }
        r3_core::HostAdmissionUnavailablePreStagingOwner::Token(token) => token.source_projection(),
    };
    let projection = r3_unavailable_projection(&core_source, &evidence, None);
    entry.host_admission_operation_counts.0 += 1;
    let prepared =
        match entry
            .engine
            .bind_host_admission_unavailable_cause(core_source, &projection, evidence)
        {
            r3_core::HostAdmissionUnavailableCauseBindOutcome::Bound(prepared) => prepared,
            r3_core::HostAdmissionUnavailableCauseBindOutcome::Mismatch {
                evidence,
                mismatch,
                ..
            } => {
                entry.pending_host_admission_failure =
                    Some(HostAdmissionFailureHolder::CauseBindPreStagingInert {
                        owner,
                        candidate: source_evidence,
                        evidence,
                        mismatch,
                    });
                return;
            }
        };
    let source = r3_core::HostAdmissionUnavailablePreStagingSource {
        prepared,
        owner,
        installation: source_evidence,
    };
    match entry
        .engine
        .complete_pre_staging_host_admission_unavailable(source, |evidence| evidence)
    {
        r3_core::HostAdmissionUnavailablePreStagingOutcome::Published { .. } => {
            entry.host_admission_operation_counts.1 += 1;
            entry.host_admission_operation_counts.2 += 1;
        }
        r3_core::HostAdmissionUnavailablePreStagingOutcome::Mismatch { source, evidence } => {
            entry.pending_host_admission_failure =
                Some(HostAdmissionFailureHolder::PublicationInert(
                    HostAdmissionPublicationInert::PreStaging { source, evidence },
                ));
        }
    }
}

fn drive_observed_candidate_rejection(
    entry: &mut RouterEntry,
    source_evidence: HostCandidateObservationSource,
    evidence: R3HostAdmissionUnavailableEvidence,
    expected_observation_digest: Option<u64>,
) {
    let delivery = match entry.engine.issue_host_admission_source_permit() {
        r3_core::HostAdmissionSourcePermitOutcome::Issued(permit) => {
            complete_pre_staging_r3_unavailable(
                entry,
                r3_core::HostAdmissionUnavailablePreStagingOwner::Permit(permit),
                source_evidence,
                evidence,
            );
            HostObservationDelivery::after_core(entry)
        }
        r3_core::HostAdmissionSourcePermitOutcome::Ineligible { source, .. } => {
            complete_observed_r3_unavailable(entry, source, evidence, expected_observation_digest);
            HostObservationDelivery::after_core(entry)
        }
        r3_core::HostAdmissionSourcePermitOutcome::Outstanding { .. } => {
            HostObservationDelivery::DeferredBeforeCore
        }
    };
    delivery.reconcile(entry);
}

fn drive_r3_unique_host_candidate(
    world: &mut World,
    entry: &mut RouterEntry,
    mut candidate: PreparedEcsHostCandidateSource,
    expected_observation_digest: Option<u64>,
    latch: &mut HostCandidateRevisionLatch,
    prepared_rejection: PreparedHostCandidateRejection,
) -> Result<HostObservationDelivery, HostCandidateCapacityEvidence> {
    if entry
        .settled_retired_host_evidence
        .as_ref()
        .is_some_and(|evidence| !settled_retired_host_allows_candidate(world, evidence, &candidate))
    {
        return Ok(HostObservationDelivery::DeferredBeforeCore);
    }
    if entry
        .idle_host_terminal
        .as_ref()
        .is_some_and(|terminal| !idle_host_loss::allows_candidate(world, terminal, &candidate))
    {
        return Ok(HostObservationDelivery::DeferredBeforeCore);
    }
    if !candidate.prepare_staging_revalidation(world)? {
        return Ok(HostObservationDelivery::DeferredBeforeCore);
    }
    let permit = match entry.engine.issue_host_admission_source_permit() {
        r3_core::HostAdmissionSourcePermitOutcome::Issued(permit) => permit,
        r3_core::HostAdmissionSourcePermitOutcome::Ineligible { source, .. } => {
            let observation = candidate.observation_source();
            let evidence = match entry.engine.snapshot().availability() {
                RouterAvailability::Unavailable(
                    bevy_ui_router_core::prelude::RouterUnavailableReason::HostConflict,
                ) => R3HostAdmissionUnavailableEvidence::Conflict(observation),
                RouterAvailability::Unavailable(
                    bevy_ui_router_core::prelude::RouterUnavailableReason::InvalidHostUiTopology,
                ) => R3HostAdmissionUnavailableEvidence::InvalidTopology(observation),
                RouterAvailability::Unavailable(
                    bevy_ui_router_core::prelude::RouterUnavailableReason::HostLost,
                ) => R3HostAdmissionUnavailableEvidence::HostLost(observation),
                _ => R3HostAdmissionUnavailableEvidence::AuthorityEstablishment(observation),
            };
            let _ = complete_observed_r3_unavailable(
                entry,
                source,
                evidence,
                expected_observation_digest,
            );
            return Ok(HostObservationDelivery::after_core(entry));
        }
        r3_core::HostAdmissionSourcePermitOutcome::Outstanding { .. } => {
            return Ok(HostObservationDelivery::DeferredBeforeCore);
        }
    };
    let token = match entry.engine.request_source_bound_host_admission(permit) {
        Ok(token) => token,
        Err(error) => {
            retain_host_admission_operation_failure(
                entry,
                candidate,
                r3_core::HostAdmissionUnavailablePreStagingOwner::Permit(error.owner),
                error.evidence,
            );
            latch.commit_rejection(prepared_rejection);
            return Ok(HostObservationDelivery::OwnerTransferred);
        }
    };
    let grant = match entry.engine.grant_source_bound_host_admission(token) {
        Ok(grant) => grant,
        Err(error) => {
            retain_host_admission_operation_failure(
                entry,
                candidate,
                r3_core::HostAdmissionUnavailablePreStagingOwner::Token(error.owner),
                error.evidence,
            );
            latch.commit_rejection(prepared_rejection);
            return Ok(HostObservationDelivery::OwnerTransferred);
        }
    };
    if !world.contains_resource::<RouteEntityRegistry>()
        || !world.contains_resource::<crate::registry::QuarantineLeaseTable>()
    {
        let observation = candidate.observation_source();
        complete_granted_candidate_unavailable(
            entry,
            grant,
            candidate,
            R3HostAdmissionUnavailableEvidence::Preallocation(observation),
        );
        latch.commit_rejection(prepared_rejection);
        return Ok(HostObservationDelivery::after_core(entry));
    }
    match stage_source_bound_granted_top_slot(world, &mut candidate, &grant) {
        Ok(staged) => {
            let claim = world
                .resource::<RouteEntityRegistry>()
                .record(staged.entity())
                .expect("source-bound staging retained its exact claim")
                .claim();
            if activate_source_bound_granted_top_slot(world, &candidate, &grant, &staged) {
                entry.pending_host_confirmation = Some(PendingHostConfirmation {
                    grant,
                    candidate,
                    staged,
                    claim,
                    rejection: prepared_rejection,
                });
            } else {
                let settlement =
                    retain_staged_top_slot_for_settlement(world, &candidate.candidate, staged);
                retain_r3_provisional_failure(entry, grant, candidate, settlement, None, false);
                latch.commit_rejection(prepared_rejection);
            }
        }
        Err(failure) => {
            let post_allocation = failure.entity().is_some();
            if let Some(settlement) = failure.into_settlement() {
                retain_r3_provisional_failure(entry, grant, candidate, settlement, None, false);
            } else if post_allocation {
                entry.pending_host_admission_failure =
                    Some(HostAdmissionFailureHolder::GrantedInert { grant, candidate });
            } else {
                let observation = candidate.observation_source();
                complete_granted_candidate_unavailable(
                    entry,
                    grant,
                    candidate,
                    R3HostAdmissionUnavailableEvidence::Preallocation(observation),
                );
            }
            latch.commit_rejection(prepared_rejection);
        }
    }
    Ok(HostObservationDelivery::after_core(entry))
}

fn complete_granted_candidate_unavailable(
    entry: &mut RouterEntry,
    grant: r3_core::SourceBoundHostGrant,
    candidate: PreparedEcsHostCandidateSource,
    evidence: R3HostAdmissionUnavailableEvidence,
) {
    let source = grant.source_projection();
    let projection = r3_unavailable_projection(&source, &evidence, None);
    entry.host_admission_operation_counts.0 += 1;
    let prepared =
        match entry
            .engine
            .bind_host_admission_unavailable_cause(source, &projection, evidence)
        {
            r3_core::HostAdmissionUnavailableCauseBindOutcome::Bound(prepared) => prepared,
            r3_core::HostAdmissionUnavailableCauseBindOutcome::Mismatch {
                evidence,
                mismatch,
                ..
            } => {
                entry.pending_host_admission_failure =
                    Some(HostAdmissionFailureHolder::CauseBindGrantedCandidateInert {
                        grant,
                        candidate,
                        evidence,
                        mismatch,
                    });
                return;
            }
        };
    let source = r3_core::HostAdmissionUnavailableGrantedSource {
        prepared,
        owner: r3_core::bind_granted_host_admission_source(grant, candidate),
    };
    match entry
        .engine
        .complete_granted_host_admission_unavailable(source, |candidate| candidate)
    {
        r3_core::HostAdmissionUnavailableGrantedOutcome::Published { .. } => {
            entry.host_admission_operation_counts.1 += 1;
            entry.host_admission_operation_counts.2 += 1;
        }
        r3_core::HostAdmissionUnavailableGrantedOutcome::Mismatch { source, evidence } => {
            entry.pending_host_admission_failure =
                Some(HostAdmissionFailureHolder::PublicationInert(
                    HostAdmissionPublicationInert::GrantedCandidate { source, evidence },
                ));
        }
    }
}

enum PendingHostRejectionCause {
    Physical(crate::host::PostScopeHostConfirmationFailure),
    Retained(idle_host_loss::IdleHostAdmissionFailure),
    Core(r3_core::HostAdmissionConfirmationFailureEvidence),
}

fn complete_pending_host_confirmations(world: &mut World) {
    let count = world.resource::<RouterNavigation>().entries.len();
    for index in 0..count {
        let entry = &world.resource::<RouterNavigation>().entries[index];
        let Some(pending) = entry.pending_host_confirmation.as_ref() else {
            continue;
        };
        let cause = entry
            .idle_host_terminal
            .as_ref()
            .and_then(|terminal| {
                idle_host_loss::admission_failure(world, terminal, &pending.candidate)
            })
            .map(PendingHostRejectionCause::Retained)
            .or_else(|| {
                crate::host::post_scope_host_confirmation_failure(
                    world,
                    &pending.candidate,
                    &pending.grant,
                    &pending.staged,
                )
                .map(PendingHostRejectionCause::Physical)
            });
        let (rejected, rejection) = {
            let mut navigation = world.resource_mut::<RouterNavigation>();
            let entry = &mut navigation.entries[index];
            let PendingHostConfirmation {
                grant,
                candidate,
                staged,
                claim,
                rejection,
                ..
            } = entry
                .pending_host_confirmation
                .take()
                .expect("observed pending confirmation");
            let rejected = match cause {
                Some(cause) => Some(HostConfirmationRejection::Granted {
                    grant,
                    candidate,
                    staged,
                    cause,
                }),
                None => confirm_prepared_host_after_scopes(entry, grant, candidate, staged, claim),
            };
            (rejected, rejection)
        };
        let Some(rejected) = rejected else {
            continue;
        };
        world
            .resource_mut::<HostCandidateRevisionLatch>()
            .commit_rejection(rejection);
        // Only rejection performs further World effects/scopes. The successful
        // path above has already ended with its final resident publication.
        world.resource_scope(|world, _: bevy_ecs::change_detection::Mut<HostCandidateRevisionState>| {
        world.resource_scope(|world, _: bevy_ecs::change_detection::Mut<HostCandidateRevisionLatch>| {
        world.resource_scope(|world, mut navigation: bevy_ecs::change_detection::Mut<RouterNavigation>| {
            let entry = &mut navigation.entries[index];
            match rejected {
                HostConfirmationRejection::Granted { grant, candidate, staged, cause } => {
                    let stable_inert = !hide_source_bound_granted_top_slot(world, &candidate, &grant, &staged);
                    let settlement = retain_staged_top_slot_for_settlement(world, &candidate.candidate, staged);
                    match cause {
                        PendingHostRejectionCause::Retained(evidence) => {
                            let source = candidate.observation_source();
                            retain_r3_provisional_failure_with_cause(entry, grant, candidate, settlement,
                                R3HostAdmissionUnavailableEvidence::RetainedAdmission { source, evidence }, None, stable_inert);
                        }
                        PendingHostRejectionCause::Core(evidence) => retain_r3_provisional_failure(entry, grant, candidate, settlement, Some(evidence), stable_inert),
                        PendingHostRejectionCause::Physical(evidence) => {
                            let source = candidate.observation_source();
                            retain_r3_provisional_failure_with_cause(entry, grant, candidate, settlement,
                                R3HostAdmissionUnavailableEvidence::PostScopePhysical { source, evidence }, None, stable_inert);
                        }
                    }
                }
                HostConfirmationRejection::Confirming { owner, evidence, candidate, version, slot_id, entity, claim } => {
                    retain_r3_confirmation_failure(world, entry, owner, evidence, candidate, version, slot_id, entity, claim);
                }
            }
        });
        });
        });
    }
}

enum HostConfirmationRejection {
    Granted {
        grant: r3_core::SourceBoundHostGrant,
        candidate: PreparedEcsHostCandidateSource,
        staged: StagedProvisionalTopSlot,
        cause: PendingHostRejectionCause,
    },
    Confirming {
        owner: r3_core::CorePreparedHostConfirmation<PreparedProvisionalHostInstallation>,
        evidence: r3_core::HostAdmissionConfirmationFailureEvidence,
        candidate: ValidatedHostCandidate,
        version: bevy_ui_router_core::HostVersion,
        slot_id: bevy_ui_router_core::ids::RouteSlotId,
        entity: Entity,
        claim: ExactRegistryClaim,
    },
}

// This tail has no World access. All successful-path effects and validation
// precede its resident-resource borrow; errors return the original real owner.
fn confirm_prepared_host_after_scopes(
    entry: &mut RouterEntry,
    grant: r3_core::SourceBoundHostGrant,
    mut candidate: PreparedEcsHostCandidateSource,
    staged: StagedProvisionalTopSlot,
    claim: ExactRegistryClaim,
) -> Option<HostConfirmationRejection> {
    let staged_entity = staged.entity();
    if let Err(error) = entry.engine.record_source_bound_top_slot_registered(&grant) {
        let failure = match error {
            r3_core::SourceBoundTopSlotRegistrationError::Operation(evidence) => Some(
                r3_core::HostAdmissionConfirmationFailureEvidence::TopSlotRegistrationOperationRejected(
                    evidence,
                ),
            ),
            r3_core::SourceBoundTopSlotRegistrationError::TraceCapacity(evidence) => Some(
                r3_core::HostAdmissionConfirmationFailureEvidence::TopSlotRegistrationRejected(
                    evidence,
                ),
            ),
        };
        return Some(HostConfirmationRejection::Granted {
            grant,
            candidate,
            staged,
            cause: PendingHostRejectionCause::Core(failure.expect("actual registration error")),
        });
    }
    #[cfg(test)]
    if std::mem::take(&mut entry.host_confirmation_preparation_receiver_drift_before_prepare_armed)
    {
        let _ = entry
            .engine
            .testing_drift_host_confirmation_preparation_receiver_input();
    }
    let cached_candidate = candidate
        .take_confirmation_failure_candidate()
        .expect("confirmation failure projection was prepared before admission");
    let cached_grant_version = grant.version().clone();
    let cached_grant_top_slot = grant.top_slot();
    let installation = PreparedProvisionalHostInstallation {
        staged,
        claim: claim.clone(),
        version: grant.version().clone(),
        slot_id: grant.top_slot(),
        candidate,
    };
    let owner = r3_core::bind_granted_host_admission_source(grant, installation);
    let prepared = match entry.engine.prepare_host_confirmation(owner) {
        r3_core::HostConfirmationPreparationOutcome::Prepared(prepared) => prepared,
        r3_core::HostConfirmationPreparationOutcome::PublicationPreparationRejected {
            owner,
            evidence,
        } => {
            let (grant, installation) = owner.into_parts();
            let PreparedProvisionalHostInstallation {
                staged, candidate, ..
            } = installation;
            return Some(HostConfirmationRejection::Granted {
                grant, candidate, staged,
                cause: PendingHostRejectionCause::Core(
                    r3_core::HostAdmissionConfirmationFailureEvidence::ConfirmationPreparationRejected(
                        evidence,
                    ),
                ),
            });
        }
        r3_core::HostConfirmationPreparationOutcome::ReceiverMismatch { owner, evidence } => {
            let rejected =
                r3_core::consume_host_confirmation_preparation_mismatch(owner, evidence.clone());
            let (grant, installation, _) = rejected.into_parts();
            let PreparedProvisionalHostInstallation {
                staged, candidate, ..
            } = installation;
            return Some(HostConfirmationRejection::Granted {
                grant,
                candidate,
                staged,
                cause: PendingHostRejectionCause::Core(
                    r3_core::HostAdmissionConfirmationFailureEvidence::Receiver(evidence),
                ),
            });
        }
    };
    #[cfg(test)]
    if std::mem::take(&mut entry.host_confirmation_receiver_drift_before_confirm_armed) {
        let _ = entry
            .engine
            .testing_drift_prepared_host_confirmation_receiver_input();
    }
    let confirmation = {
        let RouterEntry {
            engine,
            host,
            settled_retired_host_evidence,
            ..
        } = entry;
        engine.confirm_host_admission_with_binding(prepared, |installation, lease| {
            *host = Some(HostBinding {
                version: installation.version,
                slot_id: installation.slot_id,
                host: installation.candidate.candidate.host(),
                top_slot: installation.staged.entity(),
                top_slot_claim: installation.claim,
                registration_incarnation: installation.candidate.registration_incarnation,
                candidate: installation.candidate.candidate,
                lease,
            });
            let _ = settled_retired_host_evidence.take();
        })
    };
    match confirmation {
        r3_core::HostAdmissionConfirmationCoreOutcome::Ready { installed: (), .. } => None,
        r3_core::HostAdmissionConfirmationCoreOutcome::ReceiverMismatch { owner, evidence } => {
            Some(HostConfirmationRejection::Confirming {
                owner,
                evidence: r3_core::HostAdmissionConfirmationFailureEvidence::Receiver(evidence),
                candidate: cached_candidate,
                version: cached_grant_version,
                slot_id: cached_grant_top_slot,
                entity: staged_entity,
                claim,
            })
        }
        r3_core::HostAdmissionConfirmationCoreOutcome::IssuerRejected { owner, evidence } => {
            Some(HostConfirmationRejection::Confirming {
                owner,
                evidence: r3_core::HostAdmissionConfirmationFailureEvidence::Issuer(evidence),
                candidate: cached_candidate,
                version: cached_grant_version,
                slot_id: cached_grant_top_slot,
                entity: staged_entity,
                claim,
            })
        }
        r3_core::HostAdmissionConfirmationCoreOutcome::PublicationPreparationRejected {
            owner,
            evidence,
        } => Some(HostConfirmationRejection::Confirming {
            owner,
            evidence:
                r3_core::HostAdmissionConfirmationFailureEvidence::ConfirmationPublicationRejected(
                    evidence,
                ),
            candidate: cached_candidate,
            version: cached_grant_version,
            slot_id: cached_grant_top_slot,
            entity: staged_entity,
            claim,
        }),
    }
}

fn retain_r3_provisional_failure(
    entry: &mut RouterEntry,
    grant: r3_core::SourceBoundHostGrant,
    candidate: PreparedEcsHostCandidateSource,
    settlement: ProvisionalHostSettlement,
    failure: Option<r3_core::HostAdmissionConfirmationFailureEvidence>,
    stable_inert: bool,
) {
    let receiver = failure.as_ref().and_then(|failure| match failure {
        r3_core::HostAdmissionConfirmationFailureEvidence::Receiver(receiver) => {
            Some(receiver.clone())
        }
        r3_core::HostAdmissionConfirmationFailureEvidence::Issuer(_)
        | r3_core::HostAdmissionConfirmationFailureEvidence::TopSlotRegistrationOperationRejected(
            _,
        )
        | r3_core::HostAdmissionConfirmationFailureEvidence::TopSlotRegistrationRejected(_)
        | r3_core::HostAdmissionConfirmationFailureEvidence::ConfirmationPreparationRejected(_)
        | r3_core::HostAdmissionConfirmationFailureEvidence::ConfirmationPublicationRejected(_) => {
            None
        }
    });
    let source_observation = candidate.observation_source();
    let evidence = if let Some(failure) = failure {
        R3HostAdmissionUnavailableEvidence::Confirmation {
            source: source_observation,
            evidence: failure,
        }
    } else {
        R3HostAdmissionUnavailableEvidence::PostAllocation {
            source: source_observation,
            evidence: settlement.failure_evidence(),
        }
    };
    retain_r3_provisional_failure_with_cause(
        entry,
        grant,
        candidate,
        settlement,
        evidence,
        receiver,
        stable_inert,
    );
}

fn retain_r3_provisional_failure_with_cause(
    entry: &mut RouterEntry,
    grant: r3_core::SourceBoundHostGrant,
    candidate: PreparedEcsHostCandidateSource,
    mut settlement: ProvisionalHostSettlement,
    evidence: R3HostAdmissionUnavailableEvidence,
    receiver: Option<r3_core::HostConfirmationReceiverMismatchProjection>,
    mut stable_inert: bool,
) {
    if !settlement.bind_quarantine_origin(entry.handle, grant.version().clone()) {
        stable_inert = true;
    }
    let source = grant.source_projection();
    let projection = r3_unavailable_projection(&source, &evidence, None);
    entry.host_admission_operation_counts.0 += 1;
    match entry
        .engine
        .bind_host_admission_unavailable_cause(source, &projection, evidence)
    {
        r3_core::HostAdmissionUnavailableCauseBindOutcome::Bound(prepared) => {
            entry.pending_host_admission_failure = Some(HostAdmissionFailureHolder::Provisional(
                ProvisionalHostAdmissionFailure {
                    prepared,
                    grant,
                    candidate,
                    ledger: ProvisionalHostCleanupLedger::new(settlement),
                    receiver,
                    stable_inert,
                },
            ));
        }
        r3_core::HostAdmissionUnavailableCauseBindOutcome::Mismatch {
            evidence, mismatch, ..
        } => {
            entry.pending_host_admission_failure =
                Some(HostAdmissionFailureHolder::PublicationInert(
                    HostAdmissionPublicationInert::CauseBindGranted {
                        grant,
                        candidate,
                        settlement,
                        evidence,
                        mismatch,
                    },
                ));
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn retain_r3_confirmation_failure(
    world: &mut World,
    entry: &mut RouterEntry,
    owner: r3_core::CorePreparedHostConfirmation<PreparedProvisionalHostInstallation>,
    failure: r3_core::HostAdmissionConfirmationFailureEvidence,
    candidate: ValidatedHostCandidate,
    grant_version: bevy_ui_router_core::HostVersion,
    grant_top_slot: bevy_ui_router_core::ids::RouteSlotId,
    slot: Entity,
    claim: ExactRegistryClaim,
) {
    match entry.engine.unwind_host_confirmation(owner) {
        r3_core::HostConfirmationUnwindOutcome::Grant(source) => {
            let (grant, installation) = source.into_parts();
            let PreparedProvisionalHostInstallation {
                staged, candidate, ..
            } = installation;
            let settlement =
                retain_staged_top_slot_for_settlement(world, &candidate.candidate, staged);
            retain_r3_provisional_failure(
                entry,
                grant,
                candidate,
                settlement,
                Some(failure),
                false,
            );
        }
        r3_core::HostConfirmationUnwindOutcome::Mismatch { owner, evidence } => {
            entry.pending_host_admission_failure = Some(
                HostAdmissionFailureHolder::ConfirmingInert(ConfirmingHostAdmissionInert {
                    owner,
                    evidence: r3_core::HostAdmissionConfirmationFailureEvidence::Receiver(evidence),
                    candidate,
                    grant_version,
                    grant_top_slot,
                    slot,
                    claim,
                }),
            );
        }
    }
}

fn drive_pending_r3_host_admission_failure(world: &mut World, entry: &mut RouterEntry) {
    let Some(holder) = entry.pending_host_admission_failure.take() else {
        return;
    };
    match holder {
        HostAdmissionFailureHolder::Provisional(mut holder) if !holder.stable_inert => {
            entry.host_admission_operation_counts.3 += 1;
            if !holder.ledger.settle(world) {
                holder.stable_inert = holder.ledger.is_inert();
                entry.pending_host_admission_failure =
                    Some(HostAdmissionFailureHolder::Provisional(holder));
                return;
            }
            let terminal = match holder.ledger.into_terminal() {
                Ok(terminal) => terminal,
                Err(ledger) => {
                    holder.ledger = ledger;
                    holder.stable_inert = true;
                    entry.pending_host_admission_failure =
                        Some(HostAdmissionFailureHolder::Provisional(holder));
                    return;
                }
            };
            let owner = r3_core::bind_granted_host_admission_source(
                holder.grant,
                ProvisionalHostAdmissionTerminal {
                    candidate: holder.candidate,
                    terminal,
                },
            );
            let source = r3_core::HostAdmissionUnavailableGrantedSource {
                prepared: holder.prepared,
                owner,
            };
            match entry
                .engine
                .complete_granted_host_admission_unavailable(source, |terminal| terminal)
            {
                r3_core::HostAdmissionUnavailableGrantedOutcome::Published { .. } => {
                    entry.host_admission_operation_counts.1 += 1;
                    entry.host_admission_operation_counts.2 += 1;
                }
                r3_core::HostAdmissionUnavailableGrantedOutcome::Mismatch { source, evidence } => {
                    entry.pending_host_admission_failure =
                        Some(HostAdmissionFailureHolder::PublicationInert(
                            HostAdmissionPublicationInert::Provisional { source, evidence },
                        ));
                }
            }
        }
        holder => {
            entry.pending_host_admission_failure = Some(holder);
        }
    }
}

#[cfg(test)]
fn drive_armed_source_bound_host_admission(
    entry: &mut RouterEntry,
    candidate: PreparedEcsHostCandidateSource,
    owner: bevy_ui_router_core::integration::HostAdmissionUnavailablePreStagingOwner,
) {
    match owner {
        bevy_ui_router_core::integration::HostAdmissionUnavailablePreStagingOwner::Permit(
            permit,
        ) => match entry.engine.request_source_bound_host_admission(permit) {
            Ok(token) => drive_armed_source_bound_host_grant(entry, candidate, token),
            Err(error) => retain_host_admission_operation_failure(
                entry,
                candidate,
                bevy_ui_router_core::integration::HostAdmissionUnavailablePreStagingOwner::Permit(
                    error.owner,
                ),
                error.evidence,
            ),
        },
        bevy_ui_router_core::integration::HostAdmissionUnavailablePreStagingOwner::Token(token) => {
            drive_armed_source_bound_host_grant(entry, candidate, token);
        }
    }
}

#[cfg(test)]
fn drive_armed_source_bound_host_grant(
    entry: &mut RouterEntry,
    candidate: PreparedEcsHostCandidateSource,
    token: bevy_ui_router_core::integration::SourceBoundHostAdmissionToken,
) {
    match entry.engine.grant_source_bound_host_admission(token) {
        Ok(grant) => {
            entry.pending_host_admission_failure =
                Some(HostAdmissionFailureHolder::GrantedInert { grant, candidate });
        }
        Err(error) => retain_host_admission_operation_failure(
            entry,
            candidate,
            bevy_ui_router_core::integration::HostAdmissionUnavailablePreStagingOwner::Token(
                error.owner,
            ),
            error.evidence,
        ),
    }
}

fn retain_host_admission_operation_failure(
    entry: &mut RouterEntry,
    candidate: PreparedEcsHostCandidateSource,
    owner: bevy_ui_router_core::integration::HostAdmissionUnavailablePreStagingOwner,
    evidence: bevy_ui_router_core::integration::AdmissionOwnerFailureEvidence,
) {
    entry.pending_host_admission_failure = Some(match evidence {
        bevy_ui_router_core::integration::AdmissionOwnerFailureEvidence::Operation(evidence) => {
            match evidence {
                bevy_ui_router_core::integration::HostAdmissionOperationFailureFreezeOutcome::Frozen(
                    evidence,
                ) => HostAdmissionFailureHolder::PreStaging(HostAdmissionOperationPreStagingSource {
                    prepared: PreparedHostAdmissionOperationFailure {
                        evidence:
                            bevy_ui_router_core::integration::HostAdmissionUnavailableEvidence::Operation(
                                evidence,
                            ),
                    },
                    owner,
                    installation: candidate,
                }),
                bevy_ui_router_core::integration::HostAdmissionOperationFailureFreezeOutcome::Mismatch(
                    evidence,
                ) => HostAdmissionFailureHolder::OperationEvidenceInert {
                    owner,
                    candidate,
                    evidence,
                },
            }
        }
        bevy_ui_router_core::integration::AdmissionOwnerFailureEvidence::TraceCapacity(evidence) => {
            HostAdmissionFailureHolder::TraceCapacityInert {
                owner,
                candidate,
                evidence,
            }
        }
    });
}

pub(crate) fn collect_navigation_system(world: &mut World) {
    for entry in &mut world.resource_mut::<RouterNavigation>().entries {
        if entry.pending_back_resolution_action.is_none()
            && entry.pending_bound_back_resolution.is_none()
            && entry.pending_back_target_execution.is_none()
            && entry.pending_back_no_target_completion.is_none()
            && entry.pending_back_no_target_preparation.is_none()
            && entry.pending_back_no_target_authority_cleanup.is_none()
        {
            entry.engine.collect();
        }
    }
}

fn bind_back_resolution_handoff(
    entry: &mut RouterEntry,
    handoff: BackResolutionOwnerHandoff,
) -> bool {
    match entry.take_back_resolution_handoff(handoff) {
        TakeBackResolutionOutcome::Matched(bound) => {
            entry.pending_bound_back_resolution = Some(bound);
            true
        }
        TakeBackResolutionOutcome::Mismatch {
            reservation,
            handoff,
            evidence,
        } => {
            entry.restore_back_resolution_mismatch(reservation, handoff, evidence);
            false
        }
    }
}

fn bind_post_cleanup_back_resolution(entry: &mut RouterEntry) {
    if entry.pending_back_resolution_action.is_some()
        || entry.pending_bound_back_resolution.is_some()
        || entry.pending_back_target_execution.is_some()
    {
        return;
    }
    let Some(handoff) = entry.engine.take_back_resolution_action() else {
        return;
    };
    let _ = bind_back_resolution_handoff(entry, handoff);
}

enum DriverReportApplicationError {
    Stale,
    Realization(DeferredRealizationFailure),
}

#[cfg(feature = "bsn")]
enum NativeFailureTransferError {
    Capacity,
    Isolation(crate::presentation::native_scene::NativeSceneIsolationFailure),
}

#[cfg(all(test, feature = "bsn"))]
#[derive(Resource)]
struct NativeFailureTransferCapacity;

#[cfg(feature = "bsn")]
fn prepare_native_failure_transfer(
    world: &World,
    destination: &mut Vec<crate::presentation::native_failure::NativeTerminalResidual>,
    owner: Option<&crate::presentation::native_failure::NativeFailureOwner>,
) -> Result<(), NativeFailureTransferError> {
    let Some(owner) = owner else {
        return Ok(());
    };
    owner
        .validate_settled(world)
        .map_err(NativeFailureTransferError::Isolation)?;
    if owner.residual().is_some() {
        #[cfg(test)]
        let requested = if world.contains_resource::<NativeFailureTransferCapacity>() {
            usize::MAX
        } else {
            1
        };
        #[cfg(not(test))]
        let requested = 1;
        destination
            .try_reserve(requested)
            .map_err(|_| NativeFailureTransferError::Capacity)?;
    }
    Ok(())
}

#[cfg(feature = "bsn")]
fn complete_native_failure_transfer(
    destination: &mut Vec<crate::presentation::native_failure::NativeTerminalResidual>,
    owner: Option<crate::presentation::native_failure::NativeFailureOwner>,
) {
    if let Some(residual) = owner.and_then(|owner| owner.into_terminal()) {
        destination.push(residual);
    }
}

#[cfg(feature = "bsn")]
fn prepare_native_completed_ledger_transfer(
    world: &mut World,
    entry: &mut RouterEntry,
    ledger: CleanupLedger,
) -> Option<CleanupLedger> {
    match prepare_native_failure_transfer(
        world,
        &mut entry.native_residuals,
        ledger.native_failure(),
    ) {
        Ok(()) => Some(ledger),
        Err(NativeFailureTransferError::Capacity) => {
            entry.cleanup_ledger = Some(ledger);
            None
        }
        Err(NativeFailureTransferError::Isolation(reason)) => {
            if let Err(ledger) = authority_isolation::fail_closed_cleanup_ledger_protocol(
                world,
                entry,
                ledger,
                Some(AuthorityProtocolResidualKind::NativeCreation(reason)),
            ) {
                entry.cleanup_ledger = Some(ledger);
            }
            None
        }
    }
}

#[cfg(feature = "bsn")]
fn reject_native_task(
    world: &mut World,
    entry: &mut RouterEntry,
    pending: PendingCommit,
    task: RootFrameTask,
    kind: ViewFailureKind,
    failure: crate::presentation::native_failure::NativeFailureOwner,
) {
    let message = failure.error.to_string();
    let has_residual = failure.residual().is_some();
    let isolation = failure.validate_settled(world);
    let branch = failed_prepared_branch(pending, None).with_native_failure(failure);
    if let Err(reason) = isolation {
        let failed = PendingFailedBranch {
            terminal: PendingFailedBranchTerminal::TaskWithPhysicalResidual {
                task,
                failure: kind,
                message,
            },
            branch,
        };
        if let Err(failed) = fail_closed_failed_owner_protocol(
            world,
            entry,
            failed,
            AuthorityProtocolResidualKind::NativeCreation(reason),
        ) {
            entry.pending_failed_branch = Some(failed);
        }
        return;
    }
    match route_back_target_frame_failure(
        world,
        entry,
        BackTargetDriverUnverifiedPhysicalOwner::Residual(branch),
        task,
        kind,
        message.clone(),
    ) {
        RouteBackTargetFrameFailureOutcome::Handled => {}
        RouteBackTargetFrameFailureOutcome::NonTarget {
            physical: BackTargetDriverUnverifiedPhysicalOwner::Residual(branch),
            task,
        } => {
            let terminal = if has_residual {
                PendingFailedBranchTerminal::TaskWithPhysicalResidual {
                    task,
                    failure: kind,
                    message,
                }
            } else if branch.frames().is_empty() {
                PendingFailedBranchTerminal::Task {
                    task,
                    failure: kind,
                    message,
                }
            } else {
                nested_failure_terminal(&mut entry.engine, task, kind, message)
            };
            entry.pending_failed_branch = Some(PendingFailedBranch { terminal, branch });
        }
        _ => unreachable!("the native pre-frame failure retains its pending physical owner"),
    }
}

#[cfg(feature = "bsn")]
fn realize_native_task(
    world: &mut World,
    engine: &RouterEngine,
    host: ValidatedHostCandidate,
    top_slot: Entity,
    top_claim: ExactRegistryClaim,
    branch: Option<&PreparedBranch>,
    active: Option<&RealizedBranch>,
    task: &RootFrameTask,
    prepared: crate::bsn::PreparedBsnScene,
) -> Result<RealizedFrame, crate::presentation::native_frame::NativeFrameFailure> {
    use crate::presentation::RealizationError;
    use crate::presentation::native_frame::*;
    match task.mount_target() {
        FrameMountTarget::TopSlot { .. } => realize_native_root(
            world,
            host,
            top_slot,
            top_claim,
            task.frame().clone(),
            prepared,
        ),
        FrameMountTarget::ChildOutlet { .. } => {
            let Some(branch) = branch else {
                return Err(NativeFrameFailure::before_mount(
                    RealizationError::InvalidMount,
                    prepared,
                ));
            };
            if active.is_none() {
                let (Some(parent), Some(slot)) = (branch.last_frame(), branch.child_slots().last())
                else {
                    return Err(NativeFrameFailure::before_mount(
                        RealizationError::InvalidMount,
                        prepared,
                    ));
                };
                return realize_native_child(world, parent, slot, task, prepared);
            }
            let Ok(projection) = engine.reconciliation_projection_for_task(task) else {
                return Err(NativeFrameFailure::before_mount(
                    RealizationError::InvalidMount,
                    prepared,
                ));
            };
            let Some(mount) =
                seal_staging_mount_with_retained(world, &projection, active, branch, task)
            else {
                return Err(NativeFrameFailure::before_mount(
                    RealizationError::InvalidMount,
                    prepared,
                ));
            };
            realize_native_reconciliation_child(world, &mount, task, prepared)
        }
    }
}

fn seal_staging_mount_with_retained<'a>(
    world: &World,
    projection: &r3_core::ReconciliationProjectionView<'_>,
    active: Option<&'a RealizedBranch>,
    entering: &'a PreparedBranch,
    task: &RootFrameTask,
) -> Option<crate::branch_transaction::SealedReconciliationStagingMount<'a>> {
    let retained = match task.mount_target() {
        FrameMountTarget::ChildOutlet { parent, outlet } => world
            .get_resource::<RouteEntityRegistry>()?
            .retained_shell_claims_for_mount(world, &parent, &outlet, task.frame().slot())
            .ok()?,
        FrameMountTarget::TopSlot { .. } => Vec::new(),
    };
    crate::branch_transaction::seal_reconciliation_staging_mount_with_retained(
        projection, active, entering, task, &retained,
    )
}

fn apply_applied_driver_report_to_branch(
    world: &mut World,
    engine: &mut RouterEngine,
    host_candidate: ValidatedHostCandidate,
    top_slot: bevy_ecs::entity::Entity,
    top_slot_claim: crate::registry::ExactRegistryClaim,
    branch: Option<&PreparedBranch>,
    active: Option<&RealizedBranch>,
    task: &RootFrameTask,
    artifact: AppliedDriverReportArtifact,
) -> Result<RealizedFrame, DriverReportApplicationError> {
    if artifact.kind != ViewBuildReportKind::Applied
        || artifact.identity != task.report_identity()
        || engine.inspect_report_identity(task).is_err()
    {
        return Err(DriverReportApplicationError::Stale);
    }
    let AppliedDriverReportArtifact { plan, .. } = artifact;

    match task.mount_target() {
        FrameMountTarget::TopSlot { .. } if branch.is_none() => realize_frame_deferred(
            world,
            host_candidate,
            top_slot,
            top_slot_claim,
            task.frame().clone(),
            task.view_request_sequence(),
            plan.accept(),
        ),
        FrameMountTarget::ChildOutlet { .. } => {
            let Some(branch) = branch else {
                return Err(DriverReportApplicationError::Stale);
            };
            if active.is_none() {
                let (Some(parent), Some(mounted)) =
                    (branch.last_frame(), branch.child_slots().last())
                else {
                    return Err(DriverReportApplicationError::Stale);
                };
                return realize_child_frame_deferred(
                    world,
                    parent,
                    mounted,
                    task,
                    task.view_request_sequence(),
                    plan.accept(),
                )
                .map_err(DriverReportApplicationError::Realization);
            }
            let projection = engine
                .reconciliation_projection_for_task(task)
                .map_err(|_| DriverReportApplicationError::Stale)?;
            let Some(mount) =
                seal_staging_mount_with_retained(world, &projection, active, branch, task)
            else {
                return Err(DriverReportApplicationError::Stale);
            };
            realize_reconciliation_child_frame_deferred(
                world,
                &mount,
                task,
                task.view_request_sequence(),
                plan.accept(),
            )
        }
        FrameMountTarget::TopSlot { .. } => return Err(DriverReportApplicationError::Stale),
    }
    .map_err(DriverReportApplicationError::Realization)
}

fn realized_outlet_keys(frame: &RealizedFrame) -> Vec<bevy_ui_router_core::slot::RouteSlotKey> {
    let mut outlets = frame
        .outlets()
        .iter()
        .map(|outlet| outlet.key().clone())
        .collect::<Vec<_>>();
    outlets.sort();
    outlets.dedup();
    outlets
}

fn failed_prepared_branch(
    pending: PendingCommit,
    residual: Option<crate::mount::ChildSlotResidual>,
) -> FailedPreparedBranch {
    match residual {
        Some(residual) => FailedPreparedBranch::with_mount_residual(pending.branch, residual),
        None => FailedPreparedBranch::new(pending.branch),
    }
}

fn install_failure_cleanup_permanent_authority(
    world: &mut World,
    entry: &mut RouterEntry,
    pair: R1FailureCleanupPermanentAuthorityPair,
) {
    let (isolation, isolation_contradiction) = match prepare_failed_branch_authority_isolation(
        world,
        entry.active.as_ref(),
        pair.physical.physical().branch(),
        entry.host.as_ref(),
    ) {
        AuthorityIsolationPreparation::Ready(isolation) => (isolation, None),
        AuthorityIsolationPreparation::RetryableCapacity => {
            entry.failure_cleanup_permanent =
                Some(FailureCleanupPermanentStableInertOwner::AuthorityPreparation(pair));
            return;
        }
        AuthorityIsolationPreparation::PermanentContradiction {
            contradiction,
            fail_closed,
        } => (fail_closed, Some(contradiction)),
    };
    let active = entry.active.take();
    let request = pair.core.request();
    let (retired_host, lease) = match entry.host.take() {
        Some(host) => {
            let (retired, lease) = host.into_retired();
            (Some(retired), Some(lease))
        }
        None => (None, None),
    };
    let mut residual = Some(AuthorityProtocolResidualOwner {
        back_target: None,
        kind: AuthorityProtocolResidualKind::FailureCleanupPermanentAuthority,
        isolation_contradiction,
        active,
        pending: None,
        failed: None,
        failure_cleanup_completion: None,
        failure_cleanup_ledger: None,
        cleanup_settlement_ledger: None,
        failure_cleanup_permanent_authority: Some(pair),
        host: retired_host,
    });
    let protocol_residual = &mut entry.authority_protocol_residual;
    if let Err(returned_lease) =
        entry
            .engine
            .fail_closed_authority_protocol_retaining(lease, || {
                record_failure_cleanup_permanent_authority_publication(world, request);
                isolation.apply(world);
                *protocol_residual = residual.take();
            })
    {
        let mut owner = residual
            .take()
            .expect("rejected R1 permanent fail-close retains its exact owner pair");
        match (owner.host.take(), returned_lease) {
            (Some(retired), Some(lease)) => entry.host = Some(retired.restore(lease)),
            (None, None) => {}
            _ => unreachable!("R1 permanent fail-close returns the exact optional host lease"),
        }
        entry.active = owner.active.take();
        entry.failure_cleanup_permanent =
            Some(FailureCleanupPermanentStableInertOwner::AuthorityRejected(
                owner
                    .failure_cleanup_permanent_authority
                    .take()
                    .expect("rejected R1 fail-close returns the exact Core/physical pair"),
            ));
    }
}

fn install_failure_cleanup_permanent(
    world: &mut World,
    entry: &mut RouterEntry,
    holder: FailureCleanupPermanentOwner<FailureCleanupPhysicalPayload>,
) {
    match entry.engine.classify_failure_cleanup_permanent(holder) {
        FailureCleanupPermanentOutcome::AuthoritySettlement { physical, owner } => {
            install_failure_cleanup_permanent_authority(
                world,
                entry,
                R1FailureCleanupPermanentAuthorityPair {
                    physical,
                    core: owner,
                },
            );
        }
        FailureCleanupPermanentOutcome::ProvenZero { zero, owner } => {
            // The caller prepared the native destination before submitting the
            // manifest; the Core classification above performs no World writes.
            #[cfg(feature = "bsn")]
            let native_residuals = &mut entry.native_residuals;
            match entry.engine.complete_failure_cleanup_zero_terminal(
                owner,
                zero,
                |mut physical| {
                    #[cfg(feature = "bsn")]
                    complete_native_failure_transfer(
                        native_residuals,
                        physical.branch.take_native_failure(),
                    );
                },
            ) {
                FailureCleanupZeroTerminalOutcome::Terminal => {}
                FailureCleanupZeroTerminalOutcome::Mismatch {
                    zero,
                    owner,
                    evidence,
                } => {
                    entry.failure_cleanup_permanent = Some(
                        FailureCleanupPermanentStableInertOwner::ZeroTerminalMismatch {
                            zero,
                            owner,
                            evidence,
                        },
                    );
                }
            }
        }
        FailureCleanupPermanentOutcome::ZeroMismatch { holder, evidence } => {
            entry.failure_cleanup_permanent =
                Some(FailureCleanupPermanentStableInertOwner::ZeroBindMismatch {
                    holder,
                    evidence,
                });
        }
        FailureCleanupPermanentOutcome::Mismatch(holder) => {
            entry.failure_cleanup_permanent = Some(
                FailureCleanupPermanentStableInertOwner::CoreMismatch(holder),
            );
        }
    }
}

fn complete_zero_physical_failure(
    world: &mut World,
    entry: &mut RouterEntry,
    request: FailureCleanupRequest,
    mut branch: FailedPreparedBranch,
) -> Result<(), PendingFailedBranch> {
    if entry.pending_commit.is_some() {
        return Err(PendingFailedBranch {
            terminal: PendingFailedBranchTerminal::Request(request),
            branch,
        });
    }
    if !branch.retain_prepared_prefix(request.retained_prepared_prefix()) {
        return Err(PendingFailedBranch {
            terminal: PendingFailedBranchTerminal::Request(request),
            branch,
        });
    }
    #[cfg(feature = "bsn")]
    if let Err(error) =
        prepare_native_failure_transfer(world, &mut entry.native_residuals, branch.native_failure())
    {
        let failed = PendingFailedBranch {
            terminal: PendingFailedBranchTerminal::Request(request),
            branch,
        };
        return match error {
            NativeFailureTransferError::Capacity => Err(failed),
            NativeFailureTransferError::Isolation(reason) => fail_closed_failed_owner_protocol(
                world,
                entry,
                failed,
                AuthorityProtocolResidualKind::NativeCreation(reason),
            ),
        };
    }
    let manifest = world
        .get_resource::<RouteEntityRegistry>()
        .and_then(|registry| {
            FrozenCleanupManifest::from_failed_prepared_branch(world, registry, &branch).ok()
        });
    let Some(manifest) = manifest.filter(|manifest| manifest.len() == 0) else {
        return Err(PendingFailedBranch {
            terminal: PendingFailedBranchTerminal::Request(request),
            branch,
        });
    };
    let manifest_digest = manifest.digest();
    let reserved = match CleanupLedger::reserve(manifest) {
        Ok(reserved) => reserved,
        Err(error) => {
            let failed = PendingFailedBranch {
                terminal: PendingFailedBranchTerminal::Request(request),
                branch,
            };
            return match error.kind() {
                CleanupLedgerReserveErrorKind::RetryableCapacity => Err(failed),
                kind @ (CleanupLedgerReserveErrorKind::DomainExhausted
                | CleanupLedgerReserveErrorKind::UnfinalizedBoundaryFacts) => {
                    let _returned_manifest = error.into_manifest();
                    fail_closed_failed_owner_protocol(
                        world,
                        entry,
                        failed,
                        AuthorityProtocolResidualKind::CleanupLedger(kind),
                    )
                }
            };
        }
    };
    let observed_manifest_digest = freeze_failure_cleanup_observed_physical_manifest_digest(
        world,
        request.request(),
        manifest_digest,
    );
    let observed = CleanupManifest::new(
        request.logical_roots().to_vec(),
        observed_manifest_digest,
        0,
    );
    let physical = FailureCleanupPhysicalPayload {
        branch,
        reserved,
        preflight: None,
        #[cfg(test)]
        witness: issue_failure_cleanup_physical_witness(world, request.request()),
    };
    let physical = entry
        .engine
        .bind_failure_cleanup_physical(&request, observed, physical);
    let mut logical_roots = request.logical_roots().to_vec();
    inject_failure_cleanup_permanent_logical_root_contradiction(
        world,
        request.request(),
        &mut logical_roots,
        manifest_digest,
    );
    let prepared = match entry.engine.prepare_failure_cleanup_with_physical(
        request,
        CleanupManifest::new(logical_roots, manifest_digest, 0),
        physical,
    ) {
        FailureCleanupPhysicalPrepareOutcome::Prepared(prepared) => prepared,
        FailureCleanupPhysicalPrepareOutcome::Mismatch(mismatch) => {
            entry.failure_cleanup_permanent = Some(
                FailureCleanupPermanentStableInertOwner::PrepareMismatch(mismatch),
            );
            return Ok(());
        }
        FailureCleanupPhysicalPrepareOutcome::Permanent(permanent) => {
            record_failure_cleanup_permanent_prepare(world, permanent.request());
            install_failure_cleanup_permanent(world, entry, permanent);
            return Ok(());
        }
    };
    let (batch, physical) = prepared.into_parts();
    let mut ledger = physical.into_ledger(batch);
    if !ledger.is_settled() {
        let Some(attempt) = ledger.attempt(world) else {
            unreachable!("an exact zero-physical cleanup ledger issues one attempt when needed")
        };
        assert!(ledger.settle(world, attempt));
    }
    let completion = ledger
        .pending_completion()
        .expect("an empty exact cleanup ledger settles immediately");
    let owner = entry
        .engine
        .prepare_failure_cleanup_completion(completion)
        .expect("an exact empty cleanup completion is current");
    let mut retained_prepared_prefix = None;
    #[cfg(feature = "bsn")]
    let native_residuals = &mut entry.native_residuals;
    let disposition = entry
        .engine
        .complete_prepared_failure_cleanup_with_ledger(owner, ledger, |mut ledger| {
            let completed = ledger
                .take_failed_prepared_owner()
                .expect("the zero ledger retains its typed physical owner")
                .into_completed();
            retained_prepared_prefix = completed.retained_prepared_prefix;
            #[cfg(feature = "bsn")]
            complete_native_failure_transfer(native_residuals, completed.native_failure);
        })
        .expect("the zero ledger commits before Core continuation");
    match disposition {
        FailureCleanupCompletionDisposition::RetainedPreparedPrefixBound => {
            let branch = retained_prepared_prefix
                .expect("a bound retained-prefix continuation keeps its physical owner");
            entry.pending_commit = Some(PendingCommit {
                ticket: None,
                final_task: None,
                retained_hit: None,
                rejected_retained_publication: None,
                branch,
            });
        }
        FailureCleanupCompletionDisposition::RetainedPreparedPrefixCleanup => {
            let branch = retained_prepared_prefix
                .expect("a retained-prefix cleanup continuation keeps its physical owner");
            let Some(EngineAction::FailureCleanup(request)) = entry.engine.action() else {
                unreachable!("a typed retained-prefix cleanup disposition owns its cleanup request")
            };
            entry.pending_failed_branch = Some(PendingFailedBranch {
                terminal: PendingFailedBranchTerminal::Request(request),
                branch: FailedPreparedBranch::new(branch),
            });
        }
        FailureCleanupCompletionDisposition::Settled => {
            assert!(
                retained_prepared_prefix.is_none(),
                "a settled continuation cannot return a retained physical prefix",
            );
        }
    }
    Ok(())
}

fn retry_pending_mount_mutation_rejection(
    world: &mut World,
    entry: &mut RouterEntry,
    pending_rejection: PendingMountMutationRejection,
) -> Option<PendingMountMutationRejection> {
    let PendingMountMutationRejection {
        pending,
        artifact,
        payload,
        allowance,
    } = pending_rejection;
    match entry.engine.reject_mount_after_mutation(artifact) {
        Ok(request) => {
            let residual = match payload {
                PendingMountMutationPayload::Grant { residual } => residual,
                PendingMountMutationPayload::Receipt { staged, proof } => {
                    drop(proof);
                    Some(staged.into_residual(world))
                }
            };
            entry.pending_failed_branch = Some(PendingFailedBranch {
                terminal: PendingFailedBranchTerminal::Request(request),
                branch: failed_prepared_branch(pending, residual)
                    .with_mount_new_allowance(allowance),
            });
            None
        }
        Err(error) => Some(PendingMountMutationRejection {
            pending,
            artifact: error.into_owner(),
            payload,
            allowance,
        }),
    }
}

fn stale_driver_report_owner(pending: PendingCommit, task: RootFrameTask) -> PendingFailedBranch {
    PendingFailedBranch {
        terminal: PendingFailedBranchTerminal::Task {
            task,
            failure: ViewFailureKind::Apply,
            message: "route view report application became stale".into(),
        },
        branch: failed_prepared_branch(pending, None),
    }
}

fn nested_failure_terminal(
    engine: &mut RouterEngine,
    task: RootFrameTask,
    failure: ViewFailureKind,
    message: impl Into<String>,
) -> PendingFailedBranchTerminal {
    let message = message.into();
    if engine.inspect_report_identity(&task).is_err() {
        return PendingFailedBranchTerminal::Task {
            task,
            failure,
            message,
        };
    }
    let request = engine
        .report_failed(task, failure, message)
        .expect("validated nested task must remain reportable")
        .expect("nested task failure must issue sealed cleanup authority");
    PendingFailedBranchTerminal::Request(request)
}

fn failed_branch_primary_claims(branch: &FailedPreparedBranch) -> Vec<&ExactRegistryClaim> {
    let claims = branch
        .frames()
        .iter()
        .chain(branch.detached_frames())
        .flat_map(RealizedFrame::claims)
        .chain(branch.child_slots().iter().map(|slot| slot.slot_claim()))
        .chain(
            branch
                .mount_residuals()
                .iter()
                .map(crate::mount::ChildSlotResidual::claim),
        )
        .chain(
            branch
                .rejected_child_slots()
                .iter()
                .map(|slot| slot.slot().slot_claim()),
        )
        .collect::<Vec<_>>();
    #[cfg(feature = "bsn")]
    let claims = claims
        .into_iter()
        .chain(
            branch
                .native_failure()
                .into_iter()
                .flat_map(|failure| failure.claims()),
        )
        .collect();
    claims
}

fn failed_cleanup_claim_is_current(world: &World, claim: &ExactRegistryClaim) -> bool {
    world
        .get_resource::<RouteEntityRegistry>()
        .and_then(|registry| registry.record(claim.entity()))
        .is_some_and(|record| record.claim() == *claim)
        && cleanup_claim_has_exact_authority(world, claim)
}

fn failed_branch_focus_closure(
    world: &World,
    exact_isolation_claims: &[ExactRegistryClaim],
) -> Vec<bevy_ecs::prelude::Entity> {
    let roots = exact_isolation_claims
        .iter()
        .map(ExactRegistryClaim::entity)
        .collect::<Vec<_>>();
    let mut closure = observed_descendant_closure(world, roots)
        .into_iter()
        .collect::<Vec<_>>();
    closure.sort_by_key(|entity| entity.to_bits());
    closure
}

fn release_focus_for_closure(world: &mut World, closure: &[bevy_ecs::prelude::Entity]) {
    world.resource_scope(
        |world, input_focus: bevy_ecs::change_detection::Mut<InputFocus>| {
            world.resource_scope(
                |_world, route_focus: bevy_ecs::change_detection::Mut<RouteFocusState>| {
                    prepare_focus_mutation(input_focus, route_focus, closure).apply();
                },
            );
        },
    );
}

fn focus_is_outside_closure(world: &World, closure: &[bevy_ecs::prelude::Entity]) -> bool {
    world
        .get_resource::<InputFocus>()
        .and_then(InputFocus::get)
        .is_none_or(|focused| !closure.contains(&focused))
        && world
            .get_resource::<RouteFocusState>()
            .and_then(RouteFocusState::focused)
            .is_none_or(|focused| !closure.contains(&focused))
}

fn retained_prepared_prefix_protected_closure(
    world: &World,
    branch: &FailedPreparedBranch,
    target_boundaries: &[Entity],
) -> Option<Vec<Entity>> {
    let Some(prepared) = branch.retained_prepared_prefix() else {
        return Some(Vec::new());
    };
    validate_prepared_branch(world, prepared.as_realized()).ok()?;
    let mut protected = validate_branch_descendants(world, prepared.as_realized()).ok()?;
    protected.extend(
        prepared
            .as_realized()
            .claims()
            .map(ExactRegistryClaim::entity),
    );
    protected.sort_by_key(|entity| entity.to_bits());
    protected.dedup();
    (!protected
        .iter()
        .any(|entity| target_boundaries.contains(entity)))
    .then_some(protected)
}

fn target_boundary_parent_bindings(
    world: &World,
    target_boundaries: &[Entity],
) -> Option<Vec<(Entity, Option<Entity>)>> {
    target_boundaries
        .iter()
        .map(|boundary| {
            if world.get_entity(*boundary).is_err() {
                return None;
            }
            let parent = world.get::<ChildOf>(*boundary).map(ChildOf::parent);
            if parent.is_some_and(|parent| {
                world
                    .get::<Children>(parent)
                    .is_none_or(|children| !children.contains(boundary))
            }) {
                return None;
            }
            Some((*boundary, parent))
        })
        .collect()
}

fn isolate_failed_prepared_branch(
    world: &mut World,
    branch: &FailedPreparedBranch,
    manifest: &FrozenCleanupManifest,
    active: Option<&RealizedBranch>,
) -> bool {
    if !failed_prepared_branch_roots_are_exact(world, branch, manifest) {
        return false;
    }
    let active_claims = active
        .map(|active| active.claims().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    let exact_isolation_claims = failed_branch_primary_claims(branch)
        .into_iter()
        .filter(|claim| {
            #[cfg(feature = "bsn")]
            if branch
                .native_failure()
                .and_then(|failure| failure.residual())
                .is_some_and(|residual| residual.claim_identity_is_exact(world, claim))
            {
                return true;
            }
            failed_cleanup_claim_is_current(world, claim)
        })
        .filter(|claim| !active_claims.contains(claim))
        .cloned()
        .collect::<Vec<_>>();
    let mut target_boundaries = exact_isolation_claims
        .iter()
        .map(ExactRegistryClaim::entity)
        .collect::<Vec<Entity>>();
    target_boundaries.sort_by_key(|entity| entity.to_bits());
    target_boundaries.dedup();
    let Some(target_boundary_parents_before) =
        target_boundary_parent_bindings(world, &target_boundaries)
    else {
        return false;
    };
    let Some(retained_protected_before) =
        retained_prepared_prefix_protected_closure(world, branch, &target_boundaries)
    else {
        return false;
    };
    let requires_exact_isolation =
        |claim: &ExactRegistryClaim| exact_isolation_claims.contains(claim);
    let mut focus_closure = failed_branch_focus_closure(world, &exact_isolation_claims);
    let active_focus_before = match active {
        Some(active) => {
            let Ok(closure) = validate_active_branch_local(world, active, &target_boundaries)
            else {
                return false;
            };
            closure
        }
        None => Vec::new(),
    };
    if focus_closure.iter().any(|entity| {
        active_focus_before.contains(entity) || retained_protected_before.contains(entity)
    }) {
        return false;
    }
    let protected_partition_is_exact = |world: &World| {
        target_boundary_parent_bindings(world, &target_boundaries).as_ref()
            == Some(&target_boundary_parents_before)
            && retained_prepared_prefix_protected_closure(world, branch, &target_boundaries)
                .as_ref()
                == Some(&retained_protected_before)
            && active
                .map(|active| validate_active_branch_local(world, active, &target_boundaries).ok())
                .unwrap_or_else(|| Some(Vec::new()))
                .as_ref()
                == Some(&active_focus_before)
    };
    let mut frames_are_isolated = true;
    for frame in branch.frames().iter().chain(branch.detached_frames()) {
        if !isolate_frame_for_settlement_guarded(
            world,
            frame,
            &requires_exact_isolation,
            &protected_partition_is_exact,
        ) || !protected_partition_is_exact(world)
        {
            frames_are_isolated = false;
            break;
        }
    }
    if !frames_are_isolated {
        return false;
    }
    let child_slots_are_isolated = branch
        .child_slots()
        .iter()
        .filter(|slot| requires_exact_isolation(slot.slot_claim()))
        .all(|slot| {
            if manifest.allows_pending_boundary_loss(slot.slot_claim()) {
                mounted_child_slot_local_is_isolated(world, slot)
            } else {
                mounted_child_slot_structure_is_isolated(world, slot)
            }
        })
        && branch
            .rejected_child_slots()
            .iter()
            .filter(|slot| requires_exact_isolation(slot.slot().slot_claim()))
            .all(|slot| {
                if manifest.allows_pending_boundary_loss(slot.slot().slot_claim()) {
                    mounted_child_slot_local_is_isolated(world, slot.slot())
                } else {
                    mounted_child_slot_structure_is_isolated(world, slot.slot())
                }
            });
    let residuals_are_isolated = branch
        .mount_residuals()
        .iter()
        .filter(|residual| requires_exact_isolation(residual.claim()))
        .all(|slot| {
            if manifest.allows_pending_boundary_loss(slot.claim()) {
                residual_child_slot_local_is_isolated(world, slot)
            } else {
                residual_child_slot_is_isolated(world, slot)
            }
        });
    if residuals_are_isolated {
        let residual_roots = branch
            .mount_residuals()
            .iter()
            .filter(|residual| {
                world.get_entity(residual.entity()).is_ok()
                    && (requires_exact_isolation(residual.claim())
                        || (!manifest.allows_pending_boundary_loss(residual.claim())
                            && residual_child_slot_is_isolated(world, residual)))
            })
            .map(ChildSlotResidual::entity)
            .collect::<Vec<_>>();
        focus_closure.extend(observed_descendant_closure(world, residual_roots));
        focus_closure.sort_by_key(|entity| entity.to_bits());
        focus_closure.dedup();
    }
    let mut revalidated_focus_closure = failed_branch_focus_closure(world, &exact_isolation_claims);
    let residual_roots = branch
        .mount_residuals()
        .iter()
        .filter(|residual| {
            world.get_entity(residual.entity()).is_ok()
                && (requires_exact_isolation(residual.claim())
                    || (!manifest.allows_pending_boundary_loss(residual.claim())
                        && residual_child_slot_is_isolated(world, residual)))
        })
        .map(ChildSlotResidual::entity)
        .collect::<Vec<_>>();
    revalidated_focus_closure.extend(observed_descendant_closure(world, residual_roots));
    revalidated_focus_closure.sort_by_key(|entity| entity.to_bits());
    revalidated_focus_closure.dedup();
    let active_focus_after = match active {
        Some(active) => {
            let Ok(closure) = validate_active_branch_local(world, active, &target_boundaries)
            else {
                return false;
            };
            closure
        }
        None => Vec::new(),
    };
    let retained_protected_after =
        retained_prepared_prefix_protected_closure(world, branch, &target_boundaries);
    if revalidated_focus_closure != focus_closure
        || active_focus_after != active_focus_before
        || retained_protected_after.as_ref() != Some(&retained_protected_before)
        || revalidated_focus_closure.iter().any(|entity| {
            active_focus_after.contains(entity) || retained_protected_before.contains(entity)
        })
    {
        return false;
    }
    release_focus_for_closure(world, &focus_closure);
    if retained_prepared_prefix_protected_closure(world, branch, &target_boundaries).as_ref()
        != Some(&retained_protected_before)
        || active
            .map(|active| validate_active_branch_local(world, active, &target_boundaries).ok())
            .unwrap_or_else(|| Some(Vec::new()))
            .as_ref()
            != Some(&active_focus_before)
    {
        return false;
    }
    let primary_claim_snapshot_is_current = branch
        .frames()
        .iter()
        .chain(branch.detached_frames())
        .flat_map(RealizedFrame::claims)
        .chain(branch.child_slots().iter().map(|slot| slot.slot_claim()))
        .chain(
            branch
                .rejected_child_slots()
                .iter()
                .map(|slot| slot.slot().slot_claim()),
        )
        .all(|claim| {
            if active_claims.contains(claim) {
                failed_cleanup_claim_is_current(world, claim)
            } else if requires_exact_isolation(claim) {
                failed_cleanup_claim_is_current(world, claim)
            } else {
                !failed_cleanup_claim_is_current(world, claim)
                    && manifest.allows_pending_boundary_loss(claim)
            }
        });
    let residual_snapshot_is_current = branch.mount_residuals().iter().all(|residual| {
        if requires_exact_isolation(residual.claim()) {
            failed_cleanup_claim_is_current(world, residual.claim())
        } else if manifest.allows_pending_boundary_loss(residual.claim()) {
            !failed_cleanup_claim_is_current(world, residual.claim())
        } else {
            residual_child_slot_is_isolated(world, residual)
        }
    });
    let isolation_snapshot_is_current =
        primary_claim_snapshot_is_current && residual_snapshot_is_current;
    let frames_remain_isolated = branch
        .frames()
        .iter()
        .chain(branch.detached_frames())
        .all(|frame| frame_is_isolated_for_settlement(world, frame, &requires_exact_isolation));
    let focus_is_outside = focus_is_outside_closure(world, &focus_closure);
    let child_slots_remain_isolated = branch.child_slots().iter().all(|slot| {
        if manifest.allows_pending_boundary_loss(slot.slot_claim()) {
            mounted_child_slot_local_is_isolated(world, slot)
        } else {
            mounted_child_slot_is_isolated(world, slot)
        }
    }) && branch.rejected_child_slots().iter().all(|slot| {
        if manifest.allows_pending_boundary_loss(slot.slot().slot_claim()) {
            mounted_child_slot_local_is_isolated(world, slot.slot())
        } else {
            mounted_child_slot_is_isolated(world, slot.slot())
        }
    });
    frames_are_isolated
        && child_slots_are_isolated
        && residuals_are_isolated
        && isolation_snapshot_is_current
        && frames_remain_isolated
        && child_slots_remain_isolated
        && focus_is_outside
}

fn isolate_exact_failed_prepared_branch(
    world: &mut World,
    branch: &FailedPreparedBranch,
    active: Option<&RealizedBranch>,
) -> bool {
    let mut exact_isolation_claims = failed_branch_primary_claims(branch)
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    if !exact_isolation_claims
        .iter()
        .all(|claim| failed_cleanup_claim_is_current(world, claim))
        || !branch
            .child_slots()
            .iter()
            .all(|slot| mounted_child_slot_structure_is_isolated(world, slot))
        || !branch
            .rejected_child_slots()
            .iter()
            .all(|slot| mounted_child_slot_structure_is_isolated(world, slot.slot()))
        || !branch
            .mount_residuals()
            .iter()
            .all(|slot| residual_child_slot_is_isolated(world, slot))
    {
        return false;
    }
    let Some(validated_focus_before) = validate_branch_descendants(world, branch.realized()).ok()
    else {
        return false;
    };
    let mut protected_active_focus = Vec::new();
    if let Some(active) = active {
        let active_claims = active.claims().cloned().collect::<Vec<_>>();
        protected_active_focus = active_claims
            .iter()
            .map(ExactRegistryClaim::entity)
            .chain(active.live_interactive_entities(world).iter().copied())
            .collect();
        exact_isolation_claims.retain(|claim| !active_claims.contains(claim));
    }
    let focus_closure = failed_branch_focus_closure(world, &exact_isolation_claims);
    if focus_closure
        .iter()
        .any(|entity| protected_active_focus.contains(entity))
    {
        return false;
    }
    let requires_exact_isolation =
        |claim: &ExactRegistryClaim| exact_isolation_claims.contains(claim);
    let frames_are_isolated = branch
        .frames()
        .iter()
        .chain(branch.detached_frames())
        .all(|frame| isolate_frame_for_settlement(world, frame, &requires_exact_isolation));
    if validate_branch_descendants(world, branch.realized())
        .ok()
        .as_ref()
        != Some(&validated_focus_before)
        || failed_branch_focus_closure(world, &exact_isolation_claims) != focus_closure
    {
        return false;
    }
    release_focus_for_closure(world, &focus_closure);
    frames_are_isolated
        && exact_isolation_claims
            .iter()
            .all(|claim| failed_cleanup_claim_is_current(world, claim))
        && branch
            .frames()
            .iter()
            .chain(branch.detached_frames())
            .all(|frame| frame_is_isolated_for_settlement(world, frame, &requires_exact_isolation))
        && branch
            .child_slots()
            .iter()
            .all(|slot| mounted_child_slot_is_isolated(world, slot))
        && branch
            .rejected_child_slots()
            .iter()
            .all(|slot| mounted_child_slot_is_isolated(world, slot.slot()))
        && branch
            .mount_residuals()
            .iter()
            .all(|slot| residual_child_slot_is_isolated(world, slot))
        && focus_is_outside_closure(world, &focus_closure)
}

enum BackTargetDispatchOutcome {
    NoTargetPending,
    Action(EngineAction),
    SafeStopped,
}

fn dispatch_pending_back_target(entry: &mut RouterEntry) -> BackTargetDispatchOutcome {
    match entry.pending_back_target_execution.as_ref() {
        Some(PendingBackTargetExecution::Driving(_)) => {
            return BackTargetDispatchOutcome::NoTargetPending;
        }
        Some(
            PendingBackTargetExecution::ResumeBlocked(_)
            | PendingBackTargetExecution::Classifying(_)
            | PendingBackTargetExecution::Prepared(_)
            | PendingBackTargetExecution::CommitFailure(_)
            | PendingBackTargetExecution::DriverFailureStagingMismatch(_)
            | PendingBackTargetExecution::AppliedReportStagingMismatch(_)
            | PendingBackTargetExecution::AppliedReportStagingConflict(_),
        ) => {
            return BackTargetDispatchOutcome::SafeStopped;
        }
        #[cfg(test)]
        Some(PendingBackTargetExecution::DriverFailureStagingDrift(_)) => {
            return BackTargetDispatchOutcome::SafeStopped;
        }
        None => {}
    }
    if !matches!(
        entry.pending_bound_back_resolution.as_ref(),
        Some(BoundBackResolutionHandoff::Target(_))
    ) {
        return BackTargetDispatchOutcome::NoTargetPending;
    }
    let physical_slots_are_vacant = entry.pending_commit.is_none()
        && entry.pending_frame_drive.is_none()
        && entry.pending_mount_frame_task.is_none()
        && entry.pending_mount_request.is_none()
        && entry.pending_mount_mutation_rejection.is_none()
        && entry.pending_failed_branch.is_none()
        && entry.failure_cleanup_permanent.is_none();
    if !physical_slots_are_vacant {
        return BackTargetDispatchOutcome::SafeStopped;
    }
    let Some(BoundBackResolutionHandoff::Target(bound)) =
        entry.pending_bound_back_resolution.take()
    else {
        return BackTargetDispatchOutcome::NoTargetPending;
    };
    match resume_bound_back_target(entry, bound) {
        BackTargetResumeOutcome::Resumed { start, retirement } => {
            entry.pending_back_target_execution = Some(PendingBackTargetExecution::ResumeBlocked(
                PendingBackTargetResumeBlocked::Start { start, retirement },
            ));
            let Some(PendingBackTargetExecution::ResumeBlocked(
                PendingBackTargetResumeBlocked::Start { start, retirement },
            )) = entry.pending_back_target_execution.take()
            else {
                return BackTargetDispatchOutcome::SafeStopped;
            };
            let retirement_is_exact = entry.back_capacity_issuer.as_ref().is_some_and(|issuer| {
                matches!(
                    issuer.classify_retirement_evidence(&retirement),
                    Err(BackCapacityRetireError::ConsumedReplay)
                )
            });
            if !retirement_is_exact {
                entry.pending_back_target_execution =
                    Some(PendingBackTargetExecution::ResumeBlocked(
                        PendingBackTargetResumeBlocked::Start { start, retirement },
                    ));
                return BackTargetDispatchOutcome::SafeStopped;
            }
            let resume = match start {
                BackTargetStart::Ready(resume) => resume,
                start => {
                    entry.pending_back_target_execution =
                        Some(PendingBackTargetExecution::ResumeBlocked(
                            PendingBackTargetResumeBlocked::Start { start, retirement },
                        ));
                    return BackTargetDispatchOutcome::SafeStopped;
                }
            };
            let (action, history, continuation) = resume.parts();
            if let BackTargetExecutionPairingOutcome::Mismatch(mismatch) =
                entry.engine.validate_back_target_execution_pairing(
                    BackTargetExecutionOwnerRef::Driving {
                        history,
                        continuation,
                    },
                    BackTargetPhysicalOwnerRef::ResumeAction(action),
                )
            {
                entry.pending_back_target_execution =
                    Some(PendingBackTargetExecution::ResumeBlocked(
                        PendingBackTargetResumeBlocked::PairingMismatch {
                            start: BackTargetStart::Ready(resume),
                            retirement,
                            mismatch,
                        },
                    ));
                return BackTargetDispatchOutcome::SafeStopped;
            }
            match resume.into_frame_task_parts() {
                Ok((task, history, continuation)) => {
                    drop(retirement);
                    entry.pending_back_target_execution = Some(
                        PendingBackTargetExecution::Driving(PendingBackTargetDriving {
                            history,
                            continuation,
                        }),
                    );
                    BackTargetDispatchOutcome::Action(EngineAction::FrameTask(task))
                }
                Err(resume) => match resume.into_commit_parts() {
                    Ok((ticket, history, continuation)) => {
                        drop(retirement);
                        entry.pending_back_target_execution = Some(
                            PendingBackTargetExecution::Driving(PendingBackTargetDriving {
                                history,
                                continuation,
                            }),
                        );
                        BackTargetDispatchOutcome::Action(EngineAction::Commit(ticket))
                    }
                    Err(resume) => match resume.into_mount_request_parts() {
                        Ok((request, history, continuation)) => {
                            drop(retirement);
                            entry.pending_back_target_execution = Some(
                                PendingBackTargetExecution::Driving(PendingBackTargetDriving {
                                    history,
                                    continuation,
                                }),
                            );
                            BackTargetDispatchOutcome::Action(EngineAction::MountRequest(request))
                        }
                        Err(resume) => {
                            entry.pending_back_target_execution =
                                Some(PendingBackTargetExecution::ResumeBlocked(
                                    PendingBackTargetResumeBlocked::Start {
                                        start: BackTargetStart::Ready(resume),
                                        retirement,
                                    },
                                ));
                            BackTargetDispatchOutcome::SafeStopped
                        }
                    },
                },
            }
        }
        BackTargetResumeOutcome::Permanent { owner, retirement } => {
            entry.pending_back_target_execution = Some(PendingBackTargetExecution::ResumeBlocked(
                PendingBackTargetResumeBlocked::Permanent { owner, retirement },
            ));
            BackTargetDispatchOutcome::SafeStopped
        }
        BackTargetResumeOutcome::CapacityRejected { bound, error } => {
            entry.pending_back_target_execution = Some(PendingBackTargetExecution::ResumeBlocked(
                PendingBackTargetResumeBlocked::CapacityRejected { bound, error },
            ));
            BackTargetDispatchOutcome::SafeStopped
        }
        BackTargetResumeOutcome::ReceiverMismatch { bound, evidence } => {
            entry.pending_back_target_execution = Some(PendingBackTargetExecution::ResumeBlocked(
                PendingBackTargetResumeBlocked::ReceiverMismatch { bound, evidence },
            ));
            BackTargetDispatchOutcome::SafeStopped
        }
    }
}

pub(crate) fn drive_navigation_system(world: &mut World) {
    let redispatch = drive_navigation_pass(world, None);
    if !redispatch.is_empty() {
        let _ = drive_navigation_pass(world, Some(&redispatch));
    }
}

fn retain_frame_task_after_authority_retry(
    entry: &mut RouterEntry,
    had_active: bool,
    active: RealizedBranch,
    pending: PendingCommit,
    mut task: RootFrameTask,
    retry_effect: Option<ViewBuildEffect>,
) {
    let effect = retry_effect.unwrap_or_else(|| {
        view_integration::build_effect(
            task.view().clone(),
            task.take_input(),
            task.view_request_sequence(),
        )
    });
    entry.active = had_active.then_some(active);
    entry.pending_frame_drive = Some(PendingFrameDrive {
        task,
        effect,
        pending,
    });
}

fn drive_navigation_pass(world: &mut World, only_entries: Option<&[usize]>) -> Vec<usize> {
    let mut newly_bound_entries = Vec::new();
    world.resource_scope(
        |world, mut navigation: bevy_ecs::change_detection::Mut<RouterNavigation>| {
            world.resource_scope(
                |world, mut drivers: bevy_ecs::change_detection::Mut<ViewDriverStore>| {
                    for (entry_index, entry) in navigation.entries.iter_mut().enumerate() {
                        if only_entries.is_some_and(|entries| !entries.contains(&entry_index)) {
                            continue;
                        }
                        if let Some(pending_rejection) =
                            entry.pending_mount_mutation_rejection.take()
                        {
                            entry.pending_mount_mutation_rejection =
                                retry_pending_mount_mutation_rejection(
                                    world,
                                    entry,
                                    pending_rejection,
                                );
                            continue;
                        }
                        if entry.pending_failed_branch.is_some()
                            || entry.failure_cleanup_permanent.is_some()
                        {
                            continue;
                        }
                        let target_action = match dispatch_pending_back_target(entry) {
                            BackTargetDispatchOutcome::Action(action) => Some(action),
                            BackTargetDispatchOutcome::SafeStopped => continue,
                            BackTargetDispatchOutcome::NoTargetPending => None,
                        };
                        if entry.pending_back_no_target_preparation.is_some()
                            || entry.pending_back_no_target_authority_cleanup.is_some()
                            || entry.pending_back_no_target_completion.is_some()
                        {
                            continue;
                        }
                        if matches!(
                            entry.pending_bound_back_resolution.as_ref(),
                            Some(BoundBackResolutionHandoff::NoTarget(_))
                        ) {
                            let Some(BoundBackResolutionHandoff::NoTarget(bound)) =
                                entry.pending_bound_back_resolution.take()
                            else {
                                continue;
                            };
                            match begin_bound_back_no_target(entry, bound) {
                                BackNoTargetBeginOutcome::Begun(owner) => {
                                    match classify_back_no_target(world, entry, owner) {
                                        BackNoTargetPreparationOutcome::Ready(prepared) => {
                                            match consume_prepared_back_no_target(entry, prepared) {
                                                BackNoTargetFinalizationOutcome::Published => {}
                                                BackNoTargetFinalizationOutcome::CapacityRejected {
                                                    owner,
                                                    error,
                                                } => {
                                                    entry.pending_back_no_target_preparation =
                                                        Some(
                                                            PendingBackNoTargetPreparation::ReadyCapacityRejected {
                                                                owner,
                                                                error,
                                                            },
                                                        );
                                                }
                                                BackNoTargetFinalizationOutcome::ReceiverMismatch {
                                                    owner,
                                                    mismatch,
                                                } => {
                                                    entry.pending_back_no_target_preparation =
                                                        Some(
                                                            PendingBackNoTargetPreparation::ReadyReceiverMismatch {
                                                                owner,
                                                                mismatch,
                                                            },
                                                        );
                                                }
                                            }
                                        }
                                        BackNoTargetPreparationOutcome::AuthoritySettlement(
                                            disposition,
                                        ) => {
                                            match dispatch_back_no_target_authority_settlement(
                                                world,
                                                entry,
                                                disposition,
                                            ) {
                                                BackNoTargetAuthoritySettlementDispatchOutcome::Dispatched
                                                | BackNoTargetAuthoritySettlementDispatchOutcome::Inert => {}
                                                BackNoTargetAuthoritySettlementDispatchOutcome::EntryMismatch {
                                                    disposition,
                                                }
                                                | BackNoTargetAuthoritySettlementDispatchOutcome::ReceiverMismatch {
                                                    disposition,
                                                    ..
                                                }
                                                | BackNoTargetAuthoritySettlementDispatchOutcome::Retryable {
                                                    disposition,
                                                    ..
                                                }
                                                | BackNoTargetAuthoritySettlementDispatchOutcome::Permanent {
                                                    disposition,
                                                    ..
                                                } => {
                                                    entry.pending_back_no_target_preparation = Some(
                                                        PendingBackNoTargetPreparation::AuthorityDispatchBlocked {
                                                            disposition,
                                                        },
                                                    );
                                                }
                                            }
                                        }
                                        BackNoTargetPreparationOutcome::AuthorityPermanent(
                                            permanent,
                                        ) => {
                                            entry.pending_back_no_target_preparation = Some(
                                                PendingBackNoTargetPreparation::AuthorityPermanent(
                                                    permanent,
                                                ),
                                            );
                                        }
                                        BackNoTargetPreparationOutcome::Retry {
                                            owner,
                                            evidence,
                                        } => {
                                            entry.pending_back_no_target_preparation = Some(
                                                PendingBackNoTargetPreparation::PhysicalIndeterminate {
                                                    owner,
                                                    evidence,
                                                },
                                            );
                                        }
                                        BackNoTargetPreparationOutcome::CapacityRejected {
                                            owner,
                                            error,
                                        } => {
                                            entry.pending_back_no_target_preparation = Some(
                                                PendingBackNoTargetPreparation::CapacityRejected {
                                                    owner,
                                                    error,
                                                },
                                            );
                                        }
                                        BackNoTargetPreparationOutcome::ReceiverMismatch {
                                            owner,
                                            mismatch,
                                        } => {
                                            entry.pending_back_no_target_preparation = Some(
                                                PendingBackNoTargetPreparation::ReceiverMismatch {
                                                    owner,
                                                    mismatch,
                                                },
                                            );
                                        }
                                    }
                                }
                                BackNoTargetBeginOutcome::Permanent(owner) => {
                                    match settle_back_no_target_source_drift_permanent(entry, owner)
                                    {
                                        BackNoTargetSourceDriftPermanentSettlementOutcome::Settled {
                                            ..
                                        } => {}
                                        BackNoTargetSourceDriftPermanentSettlementOutcome::ReceiverMismatch {
                                            owner,
                                            ..
                                        }
                                        | BackNoTargetSourceDriftPermanentSettlementOutcome::CapacityRejected {
                                            owner,
                                            ..
                                        } => {
                                            entry.pending_back_no_target_preparation = Some(
                                                PendingBackNoTargetPreparation::SourceDriftBlocked {
                                                    owner,
                                                },
                                            );
                                        }
                                    }
                                }
                                BackNoTargetBeginOutcome::CapacityRejected { bound, .. }
                                | BackNoTargetBeginOutcome::ReceiverMismatch { bound, .. } => {
                                    entry.pending_bound_back_resolution =
                                        Some(BoundBackResolutionHandoff::NoTarget(bound));
                                }
                            }
                            continue;
                        }
                        if entry.pending_back_resolution_action.is_some()
                            || entry.pending_bound_back_resolution.is_some()
                        {
                            continue;
                        }
                        if entry.pending_commit.as_ref().is_some_and(|pending| {
                            pending.ticket.is_some() || pending.final_task.is_some()
                        }) {
                            continue;
                        }
                        let retry_pairing_mismatch =
                            match (
                                entry.pending_back_target_execution.as_ref(),
                                entry.pending_frame_drive.as_ref(),
                            ) {
                                (
                                    Some(PendingBackTargetExecution::Driving(owner)),
                                    Some(retry),
                                ) => match entry.engine.validate_back_target_execution_pairing(
                                    BackTargetExecutionOwnerRef::Driving {
                                        history: &owner.history,
                                        continuation: &owner.continuation,
                                    },
                                    BackTargetPhysicalOwnerRef::FrameTask(&retry.task),
                                ) {
                                    BackTargetExecutionPairingOutcome::Matched => None,
                                    BackTargetExecutionPairingOutcome::Mismatch(mismatch) => {
                                        Some(mismatch)
                                    }
                                },
                                _ => None,
                            };
                        if let Some(mismatch) = retry_pairing_mismatch {
                            let Some(PendingBackTargetExecution::Driving(owner)) =
                                entry.pending_back_target_execution.take()
                            else {
                                continue;
                            };
                            entry.pending_back_target_execution =
                                Some(PendingBackTargetExecution::ResumeBlocked(
                                    PendingBackTargetResumeBlocked::DrivingPairingMismatch {
                                        owner,
                                        mismatch,
                                    },
                                ));
                            continue;
                        }
                        let (action, retry_context) =
                            if let Some(action) = target_action {
                                (Some(action), None)
                            } else if let Some(retry) = entry.pending_frame_drive.take() {
                                (
                                    Some(EngineAction::FrameTask(retry.task)),
                                    Some((retry.pending, retry.effect)),
                                )
                            } else if let Some(task) = entry.pending_mount_frame_task.take() {
                                (Some(EngineAction::FrameTask(task)), None)
                            } else {
                                (
                                    entry
                                        .pending_mount_request
                                        .take()
                                        .map(EngineAction::MountRequest)
                                        .or_else(|| entry.engine.action()),
                                    None,
                                )
                            };
                        let Some(action) = action else {
                            continue;
                        };
                        match action {
                            EngineAction::Commit(ticket) => {
                                let mut pending = PendingCommit::new();
                                pending.ticket = Some(ticket);
                                entry.pending_commit = Some(pending);
                            }
                            EngineAction::FrameTask(mut task) => {
                                let task_mount_target = task.mount_target();
                                let realizing_root =
                                    matches!(task_mount_target, FrameMountTarget::TopSlot { .. });
                                let (mut pending, retry_effect) = match retry_context {
                                    Some((pending, effect)) => (pending, Some(effect)),
                                    None => (
                                        entry
                                            .pending_commit
                                            .take()
                                            .unwrap_or_else(PendingCommit::new),
                                        None,
                                    ),
                                };
                                if retry_effect.is_none()
                                    && pending.branch.frames().is_empty()
                                    && pending.branch.child_slots().is_empty()
                                    && !entry.retained.is_empty()
                                {
                                    let hit = entry.retained.inventory().ok_or(IntegrationError::Stale)
                                        .and_then(|inventory| entry.engine.prepare_retained_hit_plan(&task, &inventory));
                                    match hit {
                                        Ok(Some(plan)) => {
                                            pending.retained_hit = Some(Box::new(plan));
                                            pending.final_task = Some(task);
                                            entry.pending_commit = Some(pending);
                                            continue;
                                        }
                                        Ok(None) => {}
                                        Err(_) => {
                                            entry.pending_failed_branch = Some(PendingFailedBranch {
                                                terminal: PendingFailedBranchTerminal::Task {
                                                    task,
                                                    failure: ViewFailureKind::Apply,
                                                    message: "retained hit selection became stale".into(),
                                                },
                                                branch: failed_prepared_branch(pending, None),
                                            });
                                            continue;
                                        }
                                    }
                                }
                                let expected_child_outlet =
                                    match entry.engine.expected_child_outlet(&task) {
                                        Ok(expected) => expected,
                                        Err(_) => {
                                            match route_back_target_frame_failure(
                                                world,
                                                entry,
                                                BackTargetDriverUnverifiedPhysicalOwner::Pending(
                                                    pending,
                                                ),
                                                task,
                                                ViewFailureKind::Apply,
                                                "route view report application became stale",
                                            ) {
                                                RouteBackTargetFrameFailureOutcome::Handled => {}
                                                RouteBackTargetFrameFailureOutcome::NonTarget {
                                                    physical:
                                                        BackTargetDriverUnverifiedPhysicalOwner::Pending(
                                                            pending,
                                                        ),
                                                    task,
                                                } => {
                                                    entry.pending_failed_branch = Some(
                                                        stale_driver_report_owner(pending, task),
                                                    );
                                                }
                                                RouteBackTargetFrameFailureOutcome::NonTarget {
                                                    physical:
                                                        BackTargetDriverUnverifiedPhysicalOwner::Residual(
                                                            _,
                                                        ),
                                                    ..
                                                } => unreachable!(
                                                    "a Pending stale report must remain Pending when proven non-target"
                                                ),
                                            }
                                            continue;
                                        }
                                    };
                                let Some(host) = entry.host.as_ref() else {
                                    let (pending, task) =
                                        match route_back_target_frame_failure(
                                            world,
                                            entry,
                                            BackTargetDriverUnverifiedPhysicalOwner::Pending(
                                                pending,
                                            ),
                                            task,
                                            ViewFailureKind::Apply,
                                            "router host unavailable during frame apply",
                                        ) {
                                            RouteBackTargetFrameFailureOutcome::Handled => {
                                                continue;
                                            }
                                            RouteBackTargetFrameFailureOutcome::NonTarget {
                                                physical:
                                                    BackTargetDriverUnverifiedPhysicalOwner::Pending(
                                                        pending,
                                                    ),
                                                task,
                                            } => (pending, task),
                                            RouteBackTargetFrameFailureOutcome::NonTarget {
                                                physical:
                                                    BackTargetDriverUnverifiedPhysicalOwner::Residual(
                                                        _,
                                                    ),
                                                ..
                                            } => unreachable!(
                                                "a Pending host failure must remain Pending when proven non-target"
                                            ),
                                        };
                                    let failure = entry.engine.report_failed(
                                        task,
                                        ViewFailureKind::Apply,
                                        "router host unavailable during frame apply",
                                    );
                                    if let Ok(Some(request)) = failure {
                                        entry.pending_failed_branch = Some(PendingFailedBranch {
                                            terminal: PendingFailedBranchTerminal::Request(request),
                                            branch: failed_prepared_branch(pending, None),
                                        });
                                    }
                                    continue;
                                };
                                let host_cause = precommit_authority_cause(world, host);
                                if host_cause == PreCommitAuthorityCause::HostLost
                                    && !current_host_binding_is_exact(world, host)
                                {
                                    let had_active = entry.active.is_some();
                                    let active = entry
                                        .active
                                        .take()
                                        .unwrap_or_else(RealizedBranch::empty);
                                    let reconciled = entry.engine.reconciliation_projection_for_task(&task).is_ok();
                                    let outcome = if reconciled {
                                        let logical_roots = entry.engine.authority_settlement_logical_roots();
                                        begin_authority_settlement(
                                        world,
                                        entry,
                                        active,
                                        pending,
                                        Some(logical_roots),
                                        )
                                    } else {
                                        begin_unstarted_fallback_host_loss(world, entry, active, pending, &task)
                                    };
                                    match outcome {
                                        AuthoritySettlementBeginOutcome::Begun => {}
                                        #[cfg(feature = "bsn")]
                                        AuthoritySettlementBeginOutcome::NativeCleanupContradiction { active, pending, error } => {
                                            if let Err((active, pending)) = fail_closed_normal_owner_protocol(
                                                world, entry, active, pending, AuthorityProtocolResidualKind::NativeCleanup(error),
                                            ) {
                                                retain_frame_task_after_authority_retry(entry, had_active, active, pending, task, retry_effect);
                                            }
                                        }
                                        AuthoritySettlementBeginOutcome::TriggerContradiction { active, pending, kind } => {
                                            if let Err((active, pending)) = fail_closed_normal_owner_protocol(
                                                world, entry, active, pending, AuthorityProtocolResidualKind::AuthorityTrigger(kind),
                                            ) {
                                                retain_frame_task_after_authority_retry(entry, had_active, active, pending, task, retry_effect);
                                            }
                                        }
                                        AuthoritySettlementBeginOutcome::RetryablePreparation {
                                            active,
                                            pending,
                                        } => retain_frame_task_after_authority_retry(
                                            entry,
                                            had_active,
                                            active,
                                            pending,
                                            task,
                                            retry_effect,
                                        ),
                                        AuthoritySettlementBeginOutcome::ProtocolContradiction {
                                            active,
                                            pending,
                                            kind,
                                        } => {
                                            if let Err((active, pending)) =
                                                fail_closed_normal_authority_protocol(
                                                    world, entry, active, pending, kind,
                                                )
                                            {
                                                retain_frame_task_after_authority_retry(
                                                    entry,
                                                    had_active,
                                                    active,
                                                    pending,
                                                    task,
                                                    retry_effect,
                                                );
                                            }
                                        }
                                        AuthoritySettlementBeginOutcome::IsolationContradiction {
                                            active,
                                            pending,
                                            contradiction,
                                            isolation,
                                        } => {
                                            if let Err((active, pending)) =
                                                fail_closed_normal_authority_protocol_prepared(
                                                    world,
                                                    entry,
                                                    active,
                                                    pending,
                                                    AuthorityProtocolResidualKind::AuthorityIsolation(
                                                        contradiction.clone(),
                                                    ),
                                                    isolation,
                                                    Some(contradiction),
                                                )
                                            {
                                                retain_frame_task_after_authority_retry(
                                                    entry,
                                                    had_active,
                                                    active,
                                                    pending,
                                                    task,
                                                    retry_effect,
                                                );
                                            }
                                        }
                                        AuthoritySettlementBeginOutcome::LedgerContradiction {
                                            active,
                                            pending,
                                            kind,
                                        } => {
                                            if let Err((active, pending)) =
                                                fail_closed_normal_owner_protocol(
                                                    world,
                                                    entry,
                                                    active,
                                                    pending,
                                                    AuthorityProtocolResidualKind::CleanupLedger(
                                                        kind,
                                                    ),
                                                )
                                            {
                                                retain_frame_task_after_authority_retry(
                                                    entry,
                                                    had_active,
                                                    active,
                                                    pending,
                                                    task,
                                                    retry_effect,
                                                );
                                            }
                                        }
                                    }
                                    continue;
                                }
                                if !validated_host_candidate_is_current(world, &host.candidate) {
                                    let (pending, task) =
                                        match route_back_target_frame_failure(
                                            world,
                                            entry,
                                            BackTargetDriverUnverifiedPhysicalOwner::Pending(
                                                pending,
                                            ),
                                            task,
                                            ViewFailureKind::Apply,
                                            "router host authority changed before frame realization",
                                        ) {
                                            RouteBackTargetFrameFailureOutcome::Handled => {
                                                continue;
                                            }
                                            RouteBackTargetFrameFailureOutcome::NonTarget {
                                                physical:
                                                    BackTargetDriverUnverifiedPhysicalOwner::Pending(
                                                        pending,
                                                    ),
                                                task,
                                            } => (pending, task),
                                            RouteBackTargetFrameFailureOutcome::NonTarget {
                                                physical:
                                                    BackTargetDriverUnverifiedPhysicalOwner::Residual(
                                                        _,
                                                    ),
                                                ..
                                            } => unreachable!(
                                                "a Pending host drift must remain Pending when proven non-target"
                                            ),
                                        };
                                    let failure = entry.engine.report_failed(
                                        task,
                                        ViewFailureKind::Apply,
                                        "router host authority changed before frame realization",
                                    );
                                    if let Ok(Some(request)) = failure {
                                        entry.pending_failed_branch = Some(PendingFailedBranch {
                                            terminal: PendingFailedBranchTerminal::Request(request),
                                            branch: failed_prepared_branch(pending, None),
                                        });
                                    }
                                    continue;
                                }
                                let host_candidate = host.candidate.clone();
                                let top_slot = host.top_slot;
                                let top_slot_claim = host.top_slot_claim.clone();
                                let Some(driver_key) =
                                    view_integration::driver_key(&entry.views, task.view())
                                        .cloned()
                                else {
                                    let (pending, task) =
                                        match route_back_target_frame_failure(
                                            world,
                                            entry,
                                            BackTargetDriverUnverifiedPhysicalOwner::Pending(
                                                pending,
                                            ),
                                            task,
                                            ViewFailureKind::Build,
                                            "route view binding unavailable",
                                        ) {
                                            RouteBackTargetFrameFailureOutcome::Handled => {
                                                continue;
                                            }
                                            RouteBackTargetFrameFailureOutcome::NonTarget {
                                                physical:
                                                    BackTargetDriverUnverifiedPhysicalOwner::Pending(
                                                        pending,
                                                    ),
                                                task,
                                            } => (pending, task),
                                            RouteBackTargetFrameFailureOutcome::NonTarget {
                                                physical:
                                                    BackTargetDriverUnverifiedPhysicalOwner::Residual(
                                                        _,
                                                    ),
                                                ..
                                            } => unreachable!(
                                                "a Pending binding failure must remain Pending when proven non-target"
                                            ),
                                        };
                                    let failure = entry.engine.report_failed(
                                        task,
                                        ViewFailureKind::Build,
                                        "route view binding unavailable",
                                    );
                                    if let Ok(Some(request)) = failure {
                                        entry.pending_failed_branch = Some(PendingFailedBranch {
                                            terminal: PendingFailedBranchTerminal::Request(request),
                                            branch: failed_prepared_branch(pending, None),
                                        });
                                    }
                                    continue;
                                };
                                #[cfg(feature = "bsn")]
                                let native_result = if let Some(factory) = drivers.native_mut(&driver_key) {
                                    if entry.engine.inspect_report_identity(&task).is_err() {
                                        entry.pending_failed_branch = Some(stale_driver_report_owner(pending, task));
                                        continue;
                                    }
                                    let focus_owner = match crate::native_focus::begin(world, &task) {
                                        Ok(owner) => owner,
                                        Err(error) => {
                                            let failure = crate::presentation::native_failure::NativeFailureOwner::preparation(&task, crate::bsn::BsnPreparationError::Focus(error).into());
                                            reject_native_task(world, entry, pending, task, ViewFailureKind::Build, failure);
                                            continue;
                                        }
                                    };
                                    let prepared = match crate::bsn::prepare_bsn_view(world, factory, task.input()) {
                                        Ok(prepared) => prepared,
                                        Err(failure) => {
                                            let failure = crate::presentation::native_failure::NativeFailureOwner::preparation(&task, failure);
                                            reject_native_task(world, entry, pending, task, ViewFailureKind::Build, failure);
                                            continue;
                                        }
                                    };
                                    let realized = realize_native_task(world, &entry.engine, host_candidate.clone(), top_slot, top_slot_claim.clone(), (!realizing_root).then_some(&pending.branch), entry.active.as_ref(), &task, prepared);
                                    match crate::presentation::native_frame::finish_focus(world, focus_owner, realized) {
                                        Ok(frame) => Some(Ok(frame)),
                                        Err(crate::presentation::native_frame::NativeFrameFailure::Mount(failure)) => Some(Err(DriverReportApplicationError::Realization(failure))),
                                        Err(crate::presentation::native_frame::NativeFrameFailure::Creation(failure)) => {
                                            let failure = crate::presentation::native_failure::NativeFailureOwner::creation(&task, failure);
                                            reject_native_task(world, entry, pending, task, ViewFailureKind::Apply, failure);
                                            continue;
                                        }
                                    }
                                } else { None };
                                #[cfg(not(feature = "bsn"))]
                                let native_result: Option<Result<RealizedFrame, DriverReportApplicationError>> = None;
                                let realized = if let Some(result) = native_result { result } else {
                                let Some(driver) = drivers.get_mut(&driver_key) else {
                                    let (pending, task) =
                                        match route_back_target_frame_failure(
                                            world,
                                            entry,
                                            BackTargetDriverUnverifiedPhysicalOwner::Pending(
                                                pending,
                                            ),
                                            task,
                                            ViewFailureKind::Build,
                                            "route view driver unavailable",
                                        ) {
                                            RouteBackTargetFrameFailureOutcome::Handled => {
                                                continue;
                                            }
                                            RouteBackTargetFrameFailureOutcome::NonTarget {
                                                physical:
                                                    BackTargetDriverUnverifiedPhysicalOwner::Pending(
                                                        pending,
                                                    ),
                                                task,
                                            } => (pending, task),
                                            RouteBackTargetFrameFailureOutcome::NonTarget {
                                                physical:
                                                    BackTargetDriverUnverifiedPhysicalOwner::Residual(
                                                        _,
                                                    ),
                                                ..
                                            } => unreachable!(
                                                "a Pending missing driver must remain Pending when proven non-target"
                                            ),
                                        };
                                    let failure = entry.engine.report_failed(
                                        task,
                                        ViewFailureKind::Build,
                                        "route view driver unavailable",
                                    );
                                    if let Ok(Some(request)) = failure {
                                        entry.pending_failed_branch = Some(PendingFailedBranch {
                                            terminal: PendingFailedBranchTerminal::Request(request),
                                            branch: failed_prepared_branch(pending, None),
                                        });
                                    }
                                    continue;
                                };
                                let effect = retry_effect.unwrap_or_else(|| {
                                    view_integration::build_effect(
                                        task.view().clone(),
                                        task.take_input(),
                                        task.view_request_sequence(),
                                    )
                                });
                                let artifact = {
                                    let mut plan = ContentPlan::new();
                                    let mut writer = FrameContentWriter::new(&mut plan);
                                    let mut seal = ReportSeal::new();
                                    let report_identity = task.report_identity();
                                    let report = driver.apply(
                                        &effect,
                                        &mut writer,
                                        EcsViewReportSession::new(&mut seal, report_identity),
                                    );
                                    if report.identity() != &task.report_identity() {
                                        entry.pending_frame_drive = Some(PendingFrameDrive {
                                            task,
                                            effect,
                                            pending,
                                        });
                                        continue;
                                    }
                                    if report.kind() != ViewBuildReportKind::Applied {
                                    if entry.engine.inspect_report_identity(&task).is_err() {
                                        match route_back_target_frame_failure(
                                            world,
                                            entry,
                                            BackTargetDriverUnverifiedPhysicalOwner::Pending(
                                                pending,
                                            ),
                                            task,
                                            ViewFailureKind::Apply,
                                            "route view report identity became stale",
                                        ) {
                                            RouteBackTargetFrameFailureOutcome::Handled => {}
                                            RouteBackTargetFrameFailureOutcome::NonTarget {
                                                physical:
                                                    BackTargetDriverUnverifiedPhysicalOwner::Pending(
                                                        pending,
                                                    ),
                                                task,
                                            } => {
                                                entry.pending_failed_branch =
                                                    Some(PendingFailedBranch {
                                                        terminal:
                                                            PendingFailedBranchTerminal::Task {
                                                                task,
                                                                failure: ViewFailureKind::Apply,
                                                                message: "route view report identity became stale".into(),
                                                            },
                                                        branch: failed_prepared_branch(pending, None),
                                                    });
                                            }
                                            RouteBackTargetFrameFailureOutcome::NonTarget {
                                                physical:
                                                    BackTargetDriverUnverifiedPhysicalOwner::Residual(
                                                        _,
                                                    ),
                                                ..
                                            } => unreachable!(
                                                "a Pending stale report must remain Pending when proven non-target"
                                            ),
                                        }
                                        continue;
                                    }
                                    let failure = match report.kind() {
                                        ViewBuildReportKind::BuildFailed => ViewFailureKind::Build,
                                        ViewBuildReportKind::ApplyFailed => ViewFailureKind::Apply,
                                        ViewBuildReportKind::Applied => unreachable!(),
                                    };
                                    #[cfg(test)]
                                    inject_back_target_driver_staging_drift(entry);
                                    let (pending, task) =
                                        match route_back_target_frame_failure(
                                            world,
                                            entry,
                                            BackTargetDriverUnverifiedPhysicalOwner::Pending(
                                                pending,
                                            ),
                                            task,
                                            failure,
                                            "route view driver rejected the frame",
                                        ) {
                                            RouteBackTargetFrameFailureOutcome::Handled => {
                                                continue;
                                            }
                                            RouteBackTargetFrameFailureOutcome::NonTarget {
                                                physical:
                                                    BackTargetDriverUnverifiedPhysicalOwner::Pending(
                                                        pending,
                                                    ),
                                                task,
                                            } => (pending, task),
                                            RouteBackTargetFrameFailureOutcome::NonTarget {
                                                physical:
                                                    BackTargetDriverUnverifiedPhysicalOwner::Residual(
                                                        _,
                                                    ),
                                                ..
                                            } => unreachable!(
                                                "a Pending driver failure must remain Pending when proven non-target"
                                            ),
                                        };
                                    let failure = entry.engine.report_failed(
                                        task,
                                        failure,
                                        "route view driver rejected the frame",
                                    );
                                    if let Ok(Some(request)) = failure {
                                        let branch = failed_prepared_branch(pending, None);
                                        if let Err(failed) = complete_zero_physical_failure(
                                            world, entry, request, branch,
                                        ) {
                                            entry.pending_failed_branch = Some(failed);
                                        }
                                    }
                                        continue;
                                    }
                                    AppliedDriverReportArtifact::from_report(report, plan)
                                };
                                apply_applied_driver_report_to_branch(
                                    world,
                                    &mut entry.engine,
                                    host_candidate,
                                    top_slot,
                                    top_slot_claim,
                                    (!realizing_root).then_some(&pending.branch),
                                    entry.active.as_ref(),
                                    &task,
                                    artifact,
                                )
                                };
                                let realized = match realized {
                                    Ok(realized) => realized,
                                    Err(DriverReportApplicationError::Stale) => {
                                        match route_back_target_frame_failure(
                                            world,
                                            entry,
                                            BackTargetDriverUnverifiedPhysicalOwner::Pending(
                                                pending,
                                            ),
                                            task,
                                            ViewFailureKind::Apply,
                                            "route view report application became stale",
                                        ) {
                                            RouteBackTargetFrameFailureOutcome::Handled => {}
                                            RouteBackTargetFrameFailureOutcome::NonTarget {
                                                physical:
                                                    BackTargetDriverUnverifiedPhysicalOwner::Pending(
                                                        pending,
                                                    ),
                                                task,
                                            } => {
                                                entry.pending_failed_branch = Some(
                                                    stale_driver_report_owner(pending, task),
                                                );
                                            }
                                            RouteBackTargetFrameFailureOutcome::NonTarget {
                                                physical:
                                                    BackTargetDriverUnverifiedPhysicalOwner::Residual(
                                                        _,
                                                    ),
                                                ..
                                            } => unreachable!(
                                                "a Pending stale application must remain Pending when proven non-target"
                                            ),
                                        }
                                        continue;
                                    }
                                    Err(DriverReportApplicationError::Realization(failure)) => {
                                        let (error, residual) = failure.into_parts();
                                        let message = format!("frame realization rejected: {error:?}");
                                        if let Some(residual) = residual {
                                            let failed_branch = match pending.branch.push_frame(residual) {
                                                Ok(()) => failed_prepared_branch(pending, None),
                                                Err(residual) => {
                                                    FailedPreparedBranch::with_detached_frame(
                                                        pending.branch,
                                                        residual,
                                                    )
                                                }
                                            };
                                            match route_back_target_frame_failure(
                                                world,
                                                entry,
                                                BackTargetDriverUnverifiedPhysicalOwner::Residual(
                                                    failed_branch,
                                                ),
                                                task,
                                                ViewFailureKind::Apply,
                                                message.clone(),
                                            ) {
                                                RouteBackTargetFrameFailureOutcome::Handled => {}
                                                RouteBackTargetFrameFailureOutcome::NonTarget {
                                                    physical:
                                                        BackTargetDriverUnverifiedPhysicalOwner::Residual(
                                                            failed_branch,
                                                        ),
                                                    task,
                                                } => {
                                                    let terminal = if realizing_root {
                                                        PendingFailedBranchTerminal::TaskWithPhysicalResidual {
                                                            task,
                                                            failure: ViewFailureKind::Apply,
                                                            message: message.clone(),
                                                        }
                                                    } else {
                                                        nested_failure_terminal(
                                                            &mut entry.engine,
                                                            task,
                                                            ViewFailureKind::Apply,
                                                            message.clone(),
                                                        )
                                                    };
                                                    entry.pending_failed_branch = Some(
                                                        PendingFailedBranch {
                                                            terminal,
                                                            branch: failed_branch,
                                                        },
                                                    );
                                                }
                                                RouteBackTargetFrameFailureOutcome::NonTarget {
                                                    physical:
                                                        BackTargetDriverUnverifiedPhysicalOwner::Pending(
                                                            _,
                                                        ),
                                                    ..
                                                } => unreachable!(
                                                    "a realized residual must remain Residual when proven non-target"
                                                ),
                                            }
                                        } else {
                                            let (pending, task) =
                                                match route_back_target_frame_failure(
                                                    world,
                                                    entry,
                                                    BackTargetDriverUnverifiedPhysicalOwner::Pending(
                                                        pending,
                                                    ),
                                                    task,
                                                    ViewFailureKind::Apply,
                                                    message.clone(),
                                                ) {
                                                    RouteBackTargetFrameFailureOutcome::Handled => {
                                                        continue;
                                                    }
                                                    RouteBackTargetFrameFailureOutcome::NonTarget {
                                                        physical:
                                                            BackTargetDriverUnverifiedPhysicalOwner::Pending(
                                                                pending,
                                                            ),
                                                        task,
                                                    } => (pending, task),
                                                    RouteBackTargetFrameFailureOutcome::NonTarget {
                                                        physical:
                                                            BackTargetDriverUnverifiedPhysicalOwner::Residual(
                                                                _,
                                                            ),
                                                        ..
                                                    } => unreachable!(
                                                        "a Pending realization failure must remain Pending when proven non-target"
                                                    ),
                                                };
                                            let failure = entry.engine.report_failed(
                                                task,
                                                ViewFailureKind::Apply,
                                                message,
                                            );
                                            if let Ok(Some(request)) = failure {
                                                entry.pending_failed_branch =
                                                    Some(PendingFailedBranch {
                                                        terminal:
                                                            PendingFailedBranchTerminal::Request(
                                                                request,
                                                            ),
                                                        branch: failed_prepared_branch(pending, None),
                                                    });
                                            }
                                        }
                                        continue;
                                    }
                                };
                                if let Err(realized) = pending.branch.push_frame(realized) {
                                    let failed_branch = FailedPreparedBranch::with_detached_frame(
                                        pending.branch,
                                        realized,
                                    );
                                    match route_back_target_frame_failure(
                                        world,
                                        entry,
                                        BackTargetDriverUnverifiedPhysicalOwner::Residual(
                                            failed_branch,
                                        ),
                                        task,
                                        ViewFailureKind::Apply,
                                        "frame branch append rejected",
                                    ) {
                                        RouteBackTargetFrameFailureOutcome::Handled => {}
                                        RouteBackTargetFrameFailureOutcome::NonTarget {
                                            physical:
                                                BackTargetDriverUnverifiedPhysicalOwner::Residual(
                                                    failed_branch,
                                                ),
                                            task,
                                        } => {
                                            let terminal = if realizing_root {
                                                PendingFailedBranchTerminal::TaskWithPhysicalResidual {
                                                    task,
                                                    failure: ViewFailureKind::Apply,
                                                    message: "frame branch append rejected".into(),
                                                }
                                            } else {
                                                nested_failure_terminal(
                                                    &mut entry.engine,
                                                    task,
                                                    ViewFailureKind::Apply,
                                                    "frame branch append rejected",
                                                )
                                            };
                                            entry.pending_failed_branch =
                                                Some(PendingFailedBranch {
                                                    terminal,
                                                    branch: failed_branch,
                                                });
                                        }
                                        RouteBackTargetFrameFailureOutcome::NonTarget {
                                            physical:
                                                BackTargetDriverUnverifiedPhysicalOwner::Pending(
                                                    _,
                                                ),
                                            ..
                                        } => unreachable!(
                                            "an appended residual must remain Residual when proven non-target"
                                        ),
                                    }
                                    continue;
                                }
                                if expected_child_outlet.is_some() {
                                    let Some(parent) = pending.branch.last_frame() else {
                                        match route_back_target_frame_failure(
                                            world,
                                            entry,
                                            BackTargetDriverUnverifiedPhysicalOwner::Pending(
                                                pending,
                                            ),
                                            task,
                                            ViewFailureKind::Apply,
                                            "parent frame unavailable after realization",
                                        ) {
                                            RouteBackTargetFrameFailureOutcome::Handled => {}
                                            RouteBackTargetFrameFailureOutcome::NonTarget {
                                                physical:
                                                    BackTargetDriverUnverifiedPhysicalOwner::Pending(
                                                        pending,
                                                    ),
                                                task: _task,
                                            } => {
                                                entry.pending_commit = Some(pending);
                                            }
                                            RouteBackTargetFrameFailureOutcome::NonTarget {
                                                physical:
                                                    BackTargetDriverUnverifiedPhysicalOwner::Residual(
                                                        _,
                                                    ),
                                                ..
                                            } => unreachable!(
                                                "a Pending parent retry must remain Pending when proven non-target"
                                            ),
                                        }
                                        continue;
                                    };
                                    if !validate_realized_frame(world, parent) {
                                        match route_back_target_frame_failure(
                                            world,
                                            entry,
                                            BackTargetDriverUnverifiedPhysicalOwner::Pending(
                                                pending,
                                            ),
                                            task,
                                            ViewFailureKind::Apply,
                                            "parent frame preflight rejected",
                                        ) {
                                            RouteBackTargetFrameFailureOutcome::Handled => {}
                                            RouteBackTargetFrameFailureOutcome::NonTarget {
                                                physical:
                                                    BackTargetDriverUnverifiedPhysicalOwner::Pending(
                                                        pending,
                                                    ),
                                                task,
                                            } => {
                                                entry.pending_failed_branch =
                                                    Some(PendingFailedBranch {
                                                        terminal:
                                                            PendingFailedBranchTerminal::Task {
                                                                task,
                                                                failure: ViewFailureKind::Apply,
                                                                message: "parent frame preflight rejected".into(),
                                                            },
                                                        branch: failed_prepared_branch(pending, None),
                                                    });
                                            }
                                            RouteBackTargetFrameFailureOutcome::NonTarget {
                                                physical:
                                                    BackTargetDriverUnverifiedPhysicalOwner::Residual(
                                                        _,
                                                    ),
                                                ..
                                            } => unreachable!(
                                                "a Pending parent failure must remain Pending when proven non-target"
                                            ),
                                        }
                                        continue;
                                    }
                                    let report = BackTargetAppliedReport::WithOutlets(
                                        realized_outlet_keys(parent),
                                    );
                                    let (returned_pending, task, outlets) =
                                        match route_back_target_applied_report(
                                            entry, pending, task, report,
                                        ) {
                                            RouteBackTargetAppliedOutcome::Handled => continue,
                                            RouteBackTargetAppliedOutcome::NonTarget {
                                                pending,
                                                task,
                                                report:
                                                    BackTargetAppliedReport::WithOutlets(outlets),
                                            } => (pending, task, outlets),
                                            RouteBackTargetAppliedOutcome::NonTarget {
                                                report: BackTargetAppliedReport::LegacyLeaf,
                                                ..
                                            } => unreachable!(
                                                "an intermediate outlet report remains WithOutlets"
                                            ),
                                        };
                                    pending = returned_pending;
                                    if entry
                                        .engine
                                        .report_applied_with_outlets(task, outlets)
                                        .is_err()
                                    {
                                        let terminal = match entry.engine.action() {
                                            Some(EngineAction::FrameTask(task)) => {
                                                PendingFailedBranchTerminal::Task {
                                                    task,
                                                    failure: ViewFailureKind::Apply,
                                                    message: "parent outlet report rejected".into(),
                                                }
                                            }
                                            Some(EngineAction::FailureCleanup(request)) => {
                                                PendingFailedBranchTerminal::Request(request)
                                            }
                                            _ => {
                                                entry.pending_commit = Some(pending);
                                                continue;
                                            }
                                        };
                                        entry.pending_failed_branch = Some(PendingFailedBranch {
                                            terminal,
                                            branch: failed_prepared_branch(pending, None),
                                        });
                                        continue;
                                    }
                                } else {
                                    pending.final_task = Some(task);
                                }
                                entry.pending_commit = Some(pending);
                            }
                            EngineAction::MountRequest(request) => {
                                entry.pending_mount_request = Some(request);
                                let mut pending = entry
                                    .pending_commit
                                    .take()
                                    .unwrap_or_else(PendingCommit::new);
                                if let Some(PendingBackTargetExecution::Driving(owner)) =
                                    entry.pending_back_target_execution.as_ref()
                                    && let BackTargetExecutionPairingOutcome::Mismatch(mismatch) =
                                        entry.engine.validate_back_target_execution_pairing(
                                            BackTargetExecutionOwnerRef::Driving {
                                                history: &owner.history,
                                                continuation: &owner.continuation,
                                            },
                                            BackTargetPhysicalOwnerRef::MountRequest(
                                                entry.pending_mount_request.as_ref().unwrap(),
                                            ),
                                        )
                                {
                                    let Some(PendingBackTargetExecution::Driving(owner)) =
                                        entry.pending_back_target_execution.take()
                                    else { unreachable!("the borrowed target owner remains held") };
                                    entry.pending_back_target_execution = Some(
                                        PendingBackTargetExecution::ResumeBlocked(
                                            PendingBackTargetResumeBlocked::DrivingPairingMismatch { owner, mismatch },
                                        ),
                                    );
                                    entry.pending_commit = Some(pending);
                                    continue;
                                }
                                let request_is_exact = entry
                                    .pending_mount_request
                                    .as_ref()
                                    .is_some_and(|request| {
                                        entry.engine.validate_mount_request(request).is_ok()
                                    });
                                if !request_is_exact {
                                    entry.pending_commit = Some(pending);
                                    continue;
                                }
                                let request_identity = entry
                                    .pending_mount_request
                                    .as_ref()
                                    .expect("validated mount request remains held")
                                    .request();
                                let (
                                    parent_is_exact,
                                    parent_is_active,
                                    outlet_exists,
                                    outlet,
                                    mount_parent,
                                ) = {
                                    let request = entry
                                        .pending_mount_request
                                        .as_ref()
                                        .expect("validated mount request remains held");
                                    let entering_parent = pending
                                        .branch
                                        .frames()
                                        .iter()
                                        .find(|frame| frame.frame() == request.parent());
                                    let active_parent = entry.active.as_ref().and_then(|branch| {
                                        branch
                                            .frames()
                                            .iter()
                                            .find(|frame| frame.frame() == request.parent())
                                    });
                                    let exact_entering = entering_parent.is_some_and(|parent| {
                                        validate_entering_mount_parent(
                                            world, parent, pending.branch.as_realized(), entry.active.as_ref(),
                                        )
                                    });
                                    let exact_active = active_parent
                                        .is_some_and(|parent| validate_active_frame(world, parent));
                                    let parent = entering_parent.or(active_parent);
                                    (
                                        exact_entering ^ exact_active,
                                        exact_active,
                                        parent.is_some_and(|parent| {
                                            parent.outlet(request.outlet()).is_some()
                                        }),
                                        request.outlet().clone(),
                                        request.parent().clone(),
                                    )
                                };
                                if !parent_is_exact || !outlet_exists {
                                    let request = entry
                                        .pending_mount_request
                                        .take()
                                        .expect("validated mount request remains held");
                                    let rejection = if !parent_is_exact {
                                        MountRejectionKind::InvalidOutlet
                                    } else {
                                        MountRejectionKind::MissingOutlet
                                    };
                                    match entry.engine.reject_mount(request, rejection) {
                                        Ok(request) => {
                                            entry.pending_failed_branch =
                                                Some(PendingFailedBranch {
                                                    terminal:
                                                        PendingFailedBranchTerminal::Request(
                                                            request,
                                                        ),
                                                    branch: failed_prepared_branch(pending, None),
                                                });
                                        }
                                        Err(error) => {
                                            entry.pending_mount_request = Some(error.into_owner());
                                            entry.pending_commit = Some(pending);
                                        }
                                    }
                                    continue;
                                }
                                let request = entry
                                    .pending_mount_request
                                    .take()
                                    .expect("validated mount request remains held");
                                let grant = match entry.engine.resolve_mount(request) {
                                    Ok(grant) => grant,
                                    Err(error) => {
                                        if let ResolveMountOwner::Returned(request) =
                                            error.into_owner()
                                        {
                                            entry.pending_mount_request = Some(request);
                                        }
                                        entry.pending_commit = Some(pending);
                                        continue;
                                    }
                                };
                                let allowance_seed = if parent_is_active {
                                    entry
                                        .active
                                        .as_ref()
                                        .and_then(|branch| {
                                            branch.frames().iter().find(|frame| {
                                                frame.frame() == &mount_parent
                                            })
                                        })
                                        .and_then(|parent| {
                                            MountNewStagingSeed::from_validated_request(
                                                world,
                                                request_identity,
                                                parent,
                                                &outlet,
                                                grant.slot(),
                                            )
                                        })
                                } else {
                                    None
                                };
                                let staged_result = {
                                    let parent = if parent_is_active {
                                        entry
                                            .active
                                            .as_ref()
                                            .and_then(|branch| {
                                                branch.frames().iter().find(|frame| {
                                                    frame.frame() == &mount_parent
                                                })
                                            })
                                            .expect("validated active mount parent remains exact")
                                    } else {
                                        pending
                                            .branch
                                            .frames()
                                            .iter()
                                            .find(|frame| frame.frame() == &mount_parent)
                                            .expect("validated entering mount parent remains exact")
                                    };
                                    stage_child_slot(world, parent, &outlet, &grant)
                                };
                                let (staged, allowance) = match staged_result {
                                    Ok(staged) => {
                                        let allowance = allowance_seed
                                            .and_then(|seed| seed.seal_staged(world, &staged));
                                        (staged, allowance)
                                    }
                                    Err(failure) => {
                                        let residual = failure.into_residual();
                                        let allowance = allowance_seed.and_then(|seed| {
                                            residual
                                                .as_ref()
                                                .and_then(|residual| seed.seal_residual(world, residual))
                                        });
                                        match entry.engine.reject_mount_after_mutation(
                                            MountMutationArtifact::Grant(grant),
                                        ) {
                                            Ok(request) => {
                                                entry.pending_failed_branch =
                                                    Some(PendingFailedBranch {
                                                        terminal:
                                                            PendingFailedBranchTerminal::Request(
                                                                request,
                                                            ),
                                                        branch: failed_prepared_branch(pending, residual)
                                                            .with_mount_new_allowance(allowance),
                                                    });
                                            }
                                            Err(error) => {
                                                entry.pending_mount_mutation_rejection =
                                                    Some(PendingMountMutationRejection {
                                                        pending,
                                                        artifact: error.into_owner(),
                                                        payload:
                                                            PendingMountMutationPayload::Grant {
                                                                residual,
                                                            },
                                                        allowance,
                                                    });
                                            }
                                        }
                                        continue;
                                    }
                                };
                                let receipt_digest = staged.receipt().digest();
                                let (attestation, proof) =
                                    grant.attest_receipt_with_proof(receipt_digest);
                                let permit = match entry.engine.accept_mount_receipt(attestation) {
                                    Ok(permit) => permit,
                                    Err(error) => {
                                        let pending_rejection =
                                            PendingMountMutationRejection {
                                                pending,
                                                artifact: MountMutationArtifact::Receipt(
                                                    error.into_owner(),
                                                ),
                                                payload:
                                                    PendingMountMutationPayload::Receipt {
                                                        staged,
                                                        proof,
                                                    },
                                                allowance,
                                            };
                                        entry.pending_mount_mutation_rejection =
                                            retry_pending_mount_mutation_rejection(
                                                world,
                                                entry,
                                                pending_rejection,
                                            );
                                        continue;
                                    }
                                };
                                match confirm_child_slot(
                                    world,
                                    &mut entry.engine,
                                    staged,
                                    permit,
                                    &proof,
                                ) {
                                    Ok(mounted) => {
                                        let pushed = if parent_is_active {
                                            let parent = entry
                                                .active
                                                .as_ref()
                                                .and_then(|branch| {
                                                    branch.frames().iter().find(|frame| {
                                                        frame.frame() == &mount_parent
                                                    })
                                                })
                                                .expect(
                                                    "confirmed active mount parent remains exact",
                                                );
                                            pending
                                                .branch
                                                .push_reconciliation_child_slot(mounted, parent)
                                        } else {
                                            pending.branch.push_child_slot(mounted)
                                        };
                                        match pushed {
                                            Ok(()) => {
                                                let Some(EngineAction::FrameTask(task)) =
                                                    entry.engine.action()
                                                else {
                                                    entry.pending_commit = Some(pending);
                                                    continue;
                                                };
                                                entry.pending_mount_frame_task = Some(task);
                                                entry.pending_commit = Some(pending);
                                            }
                                            Err(rejected) => {
                                                let Some(EngineAction::FrameTask(task)) =
                                                    entry.engine.action()
                                                else {
                                                    panic!(
                                                        "confirmed child slot append rejection must retain its issued child task"
                                                    )
                                                };
                                                let terminal = nested_failure_terminal(
                                                    &mut entry.engine,
                                                    task,
                                                    ViewFailureKind::Apply,
                                                    "child slot branch append rejected",
                                                );
                                                entry.pending_failed_branch =
                                                    Some(PendingFailedBranch {
                                                        terminal,
                                                        branch: FailedPreparedBranch::with_rejected_child_slot(
                                                            pending.branch,
                                                            rejected,
                                                        ),
                                                    });
                                            }
                                        }
                                    }
                                    Err(failure) => {
                                        let (cleanup, begin, residual) = failure.into_parts();
                                        let allowance = allowance
                                            .and_then(|allowance| allowance.rebind_residual(&residual));
                                        let terminal = if let Some(request) = cleanup {
                                            PendingFailedBranchTerminal::Request(request)
                                        } else if let Some(error) = begin {
                                            PendingFailedBranchTerminal::RegistrationBegin(error)
                                        } else {
                                            unreachable!("confirmation failure retains one terminal")
                                        };
                                        entry.pending_failed_branch = Some(PendingFailedBranch {
                                            terminal,
                                            branch: failed_prepared_branch(pending, Some(residual))
                                                .with_mount_new_allowance(allowance),
                                        });
                                    }
                                }
                            }
                            EngineAction::FailureCleanup(request) => {
                                let Some(pending) = entry.pending_commit.take() else {
                                    continue;
                                };
                                entry.pending_failed_branch = Some(PendingFailedBranch {
                                    terminal: PendingFailedBranchTerminal::Request(request),
                                    branch: failed_prepared_branch(pending, None),
                                });
                            }
                            EngineAction::BackResolution(handoff) => {
                                if bind_back_resolution_handoff(entry, handoff) {
                                    newly_bound_entries.push(entry_index);
                                }
                            }
                        }
                    }
                },
            );
        },
    );
    #[cfg(feature = "bsn")]
    let _ = crate::native_focus::observe(world, &[]);
    newly_bound_entries
}

enum ReconciliationPreflightOutcome {
    Ready {
        splice: SealedReconciliationSplice,
        focus_leaving: Vec<bevy_ecs::entity::Entity>,
    },
    PendingRejected {
        splice: SealedReconciliationSplice,
        evidence: LifecyclePreflightEvidence,
    },
    AuthorityLost {
        splice: SealedReconciliationSplice,
        evidence: LifecyclePreflightEvidence,
    },
}

pub(crate) enum ReconciliationPreparationDecision {
    #[cfg(feature = "bsn")]
    NativeCleanupRetry {
        owner: TargetPreparationOwner,
        evidence: crate::presentation::native_frame::NativeCleanupPreparationError,
    },
    Ready(BackTargetReadyAcceptanceOwner),
    PendingRejected {
        owner: TargetPreparationOwner,
        evidence: LifecyclePreflightEvidence,
    },
    AuthorityLost {
        owner: TargetPreparationOwner,
        evidence: LifecyclePreflightEvidence,
    },
    Permanent(PreparationPermanentOwner<TargetPreparationOwner>),
}

pub(crate) enum ReconciliationPreparationOutcome {
    Matched(ReconciliationPreparationDecision),
    PairingMismatch {
        owner: TargetPreparationOwner,
        mismatch: PreparationReceiverMismatchView,
    },
    EntryMismatch {
        owner: TargetPreparationOwner,
        evidence: LifecyclePreflightEvidence,
    },
}

pub(crate) fn current_host_binding_is_exact(world: &World, host: &HostBinding) -> bool {
    let Some(registry) = world.get_resource::<RouteEntityRegistry>() else {
        return false;
    };
    current_host_binding_is_exact_with_registry(world, registry, host)
}

fn precommit_authority_cause(world: &World, host: &HostBinding) -> PreCommitAuthorityCause {
    if world.get_entity(host.host).is_err()
        || world
            .get::<crate::app_ext::RouterHostRegistration>(host.host)
            .is_none()
    {
        PreCommitAuthorityCause::HostLost
    } else {
        PreCommitAuthorityCause::AuthorityLost
    }
}

fn current_host_binding_is_exact_with_registry(
    world: &World,
    registry: &RouteEntityRegistry,
    host: &HostBinding,
) -> bool {
    let claim_owner_is_exact = matches!(
        host.top_slot_claim.owner(),
        crate::registry::RegistryOwner::Committed(
            crate::registry::EntityOwner::HostTopSlot { host: version, slot }
        ) if version == &host.version && *slot == host.slot_id
    );
    let record_is_exact = registry.record(host.top_slot).is_some_and(|record| {
        record.claim() == host.top_slot_claim
            && crate::registry::ComponentManifest::from_world(world, host.top_slot).is_some_and(
                |actual| {
                    let child_of = world
                        .components()
                        .component_id::<bevy_ecs::hierarchy::ChildOf>();
                    let children = world
                        .components()
                        .component_id::<bevy_ecs::hierarchy::Children>();
                    let normalized = |manifest: &crate::registry::ComponentManifest| {
                        crate::registry::ComponentManifest::new(
                            manifest
                                .component_ids()
                                .iter()
                                .copied()
                                .filter(|component| Some(*component) != child_of)
                                .filter(|component| Some(*component) != children),
                        )
                    };
                    normalized(&actual) == normalized(record.manifest())
                },
            )
    });
    let parent_is_exact = world
        .get::<bevy_ecs::hierarchy::ChildOf>(host.top_slot)
        .map(bevy_ecs::hierarchy::ChildOf::parent)
        == Some(host.host);
    let membership_is_exact = world
        .get::<bevy_ecs::hierarchy::Children>(host.host)
        .is_some_and(|children| {
            children
                .iter()
                .filter(|child| **child == host.top_slot)
                .count()
                == 1
        });
    claim_owner_is_exact
        && host.candidate.host() == host.host
        && world.get_entity(host.host).is_ok()
        && world
            .get::<crate::app_ext::RouterHostRegistration>(host.host)
            .is_some_and(|registration| registration.incarnation() == host.registration_incarnation)
        && validated_host_candidate_is_current(world, &host.candidate)
        && record_is_exact
        && cleanup_claim_has_exact_authority(world, &host.top_slot_claim)
        && parent_is_exact
        && membership_is_exact
}

#[cfg(test)]
fn inject_commit_sealed_registration_removal(world: &mut World, entry: &mut RouterEntry) {
    if !std::mem::take(&mut entry.commit_sealed_registration_removal_armed) {
        return;
    }
    let Some(host) = entry.host.as_ref().map(|binding| binding.host) else {
        return;
    };
    if let Ok(mut host) = world.get_entity_mut(host)
        && host
            .take::<crate::app_ext::RouterHostRegistration>()
            .is_some()
    {
        entry.commit_sealed_registration_removal_fired = true;
    }
}

#[cfg(not(test))]
fn inject_commit_sealed_registration_removal(_world: &mut World, _entry: &mut RouterEntry) {}

#[cfg(test)]
fn commit_sealed_registration_removal_is_armed(entry: &RouterEntry) -> bool {
    entry.commit_sealed_registration_removal_armed
}

#[cfg(not(test))]
fn commit_sealed_registration_removal_is_armed(_entry: &RouterEntry) -> bool {
    false
}

enum ReconciliationPreflightDecision {
    Ready(Vec<Entity>),
    PendingRejected(LifecyclePreflightEvidence),
    AuthorityLost(LifecyclePreflightEvidence),
}

fn reconciliation_preflight(
    world: &World,
    host_is_current: bool,
    has_leaving: bool,
    splice: SealedReconciliationSplice,
) -> ReconciliationPreflightOutcome {
    match reconciliation_preflight_view(world, host_is_current, has_leaving, splice.view()) {
        ReconciliationPreflightDecision::Ready(focus_leaving) => {
            ReconciliationPreflightOutcome::Ready {
                splice,
                focus_leaving,
            }
        }
        ReconciliationPreflightDecision::PendingRejected(evidence) => {
            ReconciliationPreflightOutcome::PendingRejected { splice, evidence }
        }
        ReconciliationPreflightDecision::AuthorityLost(evidence) => {
            ReconciliationPreflightOutcome::AuthorityLost { splice, evidence }
        }
    }
}

fn reconciliation_preflight_view(
    world: &World,
    host_is_current: bool,
    has_leaving: bool,
    splice: crate::branch_transaction::ReconciliationSpliceView<'_>,
) -> ReconciliationPreflightDecision {
    let entering_boundaries = splice
        .entering()
        .claims()
        .map(ExactRegistryClaim::entity)
        .collect::<Vec<_>>();
    let current_loss = if world.get_resource::<RouteEntityRegistry>().is_none() {
        Some(LifecyclePreflightEvidence::new(
            LifecyclePreflightSource::RequiredResources(RequiredResourceSet::ROUTE_ENTITY_REGISTRY),
            LifecyclePreflightExpectation::ResourcePresent,
            LifecyclePreflightObserved::Missing,
        ))
    } else if !host_is_current {
        Some(LifecyclePreflightEvidence::new(
            LifecyclePreflightSource::HostBinding,
            LifecyclePreflightExpectation::ExactAuthority,
            LifecyclePreflightObserved::Drifted,
        ))
    } else if validate_preserved_branch_local(
        world,
        splice.preserved(),
        splice.slot_roles(),
        &entering_boundaries,
    )
    .is_err()
    {
        Some(LifecyclePreflightEvidence::new(
            LifecyclePreflightSource::PreservedCurrent,
            LifecyclePreflightExpectation::ExactAuthority,
            LifecyclePreflightObserved::Drifted,
        ))
    } else if has_leaving
        && validate_leaving_branch_for_splice(world, splice.leaving(), splice.slot_roles()).is_err()
    {
        Some(LifecyclePreflightEvidence::new(
            LifecyclePreflightSource::LeavingCurrent,
            LifecyclePreflightExpectation::ExactAuthority,
            LifecyclePreflightObserved::Drifted,
        ))
    } else {
        None
    };
    let pending_rejection = if splice.validate_entering(world).is_err() {
        Some(LifecyclePreflightEvidence::new(
            LifecyclePreflightSource::PendingEntering,
            LifecyclePreflightExpectation::ExactAuthority,
            LifecyclePreflightObserved::Drifted,
        ))
    } else {
        None
    };
    let leaving_closure = validate_branch_descendants(world, splice.leaving());
    let current_loss = current_loss.or_else(|| {
        leaving_closure.as_ref().err().map(|_| {
            LifecyclePreflightEvidence::new(
                LifecyclePreflightSource::LeavingCurrent,
                LifecyclePreflightExpectation::ExactRelationshipClosure,
                LifecyclePreflightObserved::Drifted,
            )
        })
    });
    if let Some(evidence) = current_loss {
        ReconciliationPreflightDecision::AuthorityLost(evidence)
    } else if let Some(evidence) = pending_rejection {
        ReconciliationPreflightDecision::PendingRejected(evidence)
    } else if splice.validate_active_for_splice(world).is_err() {
        ReconciliationPreflightDecision::PendingRejected(LifecyclePreflightEvidence::new(
            LifecyclePreflightSource::CrossRoleRebase,
            LifecyclePreflightExpectation::RebaseCompatible,
            LifecyclePreflightObserved::Contradictory,
        ))
    } else {
        ReconciliationPreflightDecision::Ready(
            leaving_closure.expect("current closure was independently validated"),
        )
    }
}

pub(crate) fn classify_back_target(
    world: &mut World,
    entry: &mut RouterEntry,
    owner: TargetPreparationOwner,
) -> ReconciliationPreparationOutcome {
    let Some(ticket) = entry
        .pending_commit
        .as_ref()
        .and_then(|pending| pending.ticket.as_ref())
    else {
        return ReconciliationPreparationOutcome::EntryMismatch {
            owner,
            evidence: LifecyclePreflightEvidence::new(
                LifecyclePreflightSource::PendingAdmission,
                LifecyclePreflightExpectation::ExactAuthority,
                LifecyclePreflightObserved::Drifted,
            ),
        };
    };
    if let BackTargetExecutionPairingOutcome::Mismatch(mismatch) =
        entry.engine.validate_back_target_execution_pairing(
            BackTargetExecutionOwnerRef::Classifying {
                history: &owner.history,
                continuation: &owner.continuation,
            },
            BackTargetPhysicalOwnerRef::CommitTicket(ticket),
        )
    {
        return ReconciliationPreparationOutcome::PairingMismatch { owner, mismatch };
    }
    let TargetPreparationOwner {
        splice,
        history,
        continuation,
    } = owner;
    let has_leaving =
        !splice.leaving().frames().is_empty() || !splice.leaving().child_slots().is_empty();
    let host_is_current = entry
        .host
        .as_ref()
        .is_some_and(|host| current_host_binding_is_exact(world, host));
    match reconciliation_preflight(world, host_is_current, has_leaving, splice) {
        ReconciliationPreflightOutcome::PendingRejected { splice, evidence } => {
            ReconciliationPreparationOutcome::Matched(
                ReconciliationPreparationDecision::PendingRejected {
                    owner: TargetPreparationOwner {
                        splice,
                        history,
                        continuation,
                    },
                    evidence,
                },
            )
        }
        ReconciliationPreflightOutcome::AuthorityLost { splice, evidence } => {
            ReconciliationPreparationOutcome::Matched(
                ReconciliationPreparationDecision::AuthorityLost {
                    owner: TargetPreparationOwner {
                        splice,
                        history,
                        continuation,
                    },
                    evidence,
                },
            )
        }
        ReconciliationPreflightOutcome::Ready {
            mut splice,
            focus_leaving,
        } => {
            #[cfg(feature = "bsn")]
            if let Err(error) = splice.prepare_native_cleanup(world) {
                let owner = TargetPreparationOwner {
                    splice,
                    history,
                    continuation,
                };
                return ReconciliationPreparationOutcome::Matched(if error.is_retryable() {
                    ReconciliationPreparationDecision::NativeCleanupRetry {
                        owner,
                        evidence: error,
                    }
                } else {
                    ReconciliationPreparationDecision::Permanent(PreparationPermanentOwner {
                        owner,
                        evidence: PreparationPermanentEvidence::NativeCleanup(error),
                    })
                });
            }
            let focus_leaving = complete_back_target_focus_source(world, &splice, focus_leaving);

            let transitions = splice.lifecycle_transitions();
            let Some(registry) = world.get_resource::<RouteEntityRegistry>() else {
                let evidence = LifecyclePreflightEvidence::new(
                    LifecyclePreflightSource::RequiredResources(
                        RequiredResourceSet::ROUTE_ENTITY_REGISTRY,
                    ),
                    LifecyclePreflightExpectation::ResourcePresent,
                    LifecyclePreflightObserved::Missing,
                );
                return ReconciliationPreparationOutcome::Matched(
                    ReconciliationPreparationDecision::AuthorityLost {
                        owner: TargetPreparationOwner {
                            splice,
                            history,
                            continuation,
                        },
                        evidence,
                    },
                );
            };
            let Some(registry_plan) = registry.prepare_lifecycle_mutation(&transitions) else {
                let evidence = LifecyclePreflightEvidence::new(
                    LifecyclePreflightSource::TargetPartition,
                    LifecyclePreflightExpectation::ExactAuthority,
                    LifecyclePreflightObserved::Contradictory,
                );
                return ReconciliationPreparationOutcome::Matched(
                    ReconciliationPreparationDecision::Permanent(PreparationPermanentOwner {
                        owner: TargetPreparationOwner {
                            splice,
                            history,
                            continuation,
                        },
                        evidence: PreparationPermanentEvidence::PhysicalPreparationContradiction(
                            evidence,
                        ),
                    }),
                );
            };
            let resulting_claims = registry_plan.resulting_claims();
            let leaving_count = splice.leaving().obligation_count();
            let frozen_manifest = if leaving_count == 0 {
                FrozenCleanupManifest::empty()
            } else {
                let Some(manifest) = FrozenCleanupManifest::from_leaving_branch(
                    world,
                    registry,
                    splice.leaving(),
                    resulting_claims,
                    (!splice.entering().frames().is_empty()).then_some(splice.entering()),
                ) else {
                    let evidence = LifecyclePreflightEvidence::new(
                        LifecyclePreflightSource::TargetPartition,
                        LifecyclePreflightExpectation::ExactRelationshipClosure,
                        LifecyclePreflightObserved::Contradictory,
                    );
                    return ReconciliationPreparationOutcome::Matched(
                        ReconciliationPreparationDecision::Permanent(PreparationPermanentOwner {
                            owner: TargetPreparationOwner {
                                splice,
                                history,
                                continuation,
                            },
                            evidence:
                                PreparationPermanentEvidence::PhysicalPreparationContradiction(
                                    evidence,
                                ),
                        }),
                    );
                };
                manifest
            };
            let manifest = CleanupManifest::new(
                logical_cleanup_roots(splice.leaving()),
                frozen_manifest.digest(),
                frozen_manifest.len(),
            );
            ReconciliationPreparationOutcome::Matched(ReconciliationPreparationDecision::Ready(
                BackTargetReadyAcceptanceOwner {
                    physical: PreparedBackTargetPhysicalOwner {
                        classified: BackTargetClassifiedPhysicalOwner {
                            splice,
                            history,
                            authority_preparation: None,
                        },
                        focus_leaving,
                    },
                    continuation,
                    frozen_manifest,
                    manifest,
                },
            ))
        }
    }
}

struct FrozenBackTargetPreparationPermanentPhysicalFacts {
    physical: FrozenTargetPreparationPermanentPhysicalManifest,
    normal_frozen_manifest: FrozenCleanupManifest,
    ecs_manifest: FrozenCleanupManifest,
    scope: PreparationBoundaryScopeView,
}

fn projected_target_preparation_permanent_claims(
    registry: &RouteEntityRegistry,
    splice: &SealedReconciliationSplice,
    top_slot_claim: &ExactRegistryClaim,
) -> Option<Vec<ExactRegistryClaim>> {
    let transitions = target_preparation_permanent_lifecycle_transitions(splice, top_slot_claim)?;
    let plan = registry.prepare_lifecycle_mutation(&transitions)?;
    let mut projected = plan.resulting_claims().to_vec();
    projected.sort_by_key(|claim| claim.entity().to_bits());
    Some(projected)
}

fn target_preparation_permanent_lifecycle_transitions(
    splice: &SealedReconciliationSplice,
    top_slot_claim: &ExactRegistryClaim,
) -> Option<Vec<(ExactRegistryClaim, EntityLifecycle)>> {
    fn retain(
        candidates: &mut std::collections::HashMap<
            bevy_ecs::entity::Entity,
            (ExactRegistryClaim, EntityLifecycle, u8),
        >,
        claim: &ExactRegistryClaim,
        lifecycle: EntityLifecycle,
        priority: u8,
    ) -> Option<()> {
        match candidates.entry(claim.entity()) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert((claim.clone(), lifecycle, priority));
            }
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                if entry.get().0 != *claim {
                    return None;
                }
                if priority > entry.get().2 {
                    entry.insert((claim.clone(), lifecycle, priority));
                }
            }
        }
        Some(())
    }

    let mut candidates = std::collections::HashMap::new();
    for claim in splice.entering().claims() {
        retain(&mut candidates, claim, EntityLifecycle::PreparedInert, 0)?;
    }
    for role in splice.slot_roles() {
        let current_or_shared = role.current() == EntityLifecycle::Active;
        retain(
            &mut candidates,
            role.claim(),
            if current_or_shared {
                EntityLifecycle::Leaving
            } else {
                EntityLifecycle::PreparedInert
            },
            u8::from(current_or_shared),
        )?;
    }
    for claim in splice.preserved().claims().chain(splice.leaving().claims()) {
        retain(&mut candidates, claim, EntityLifecycle::Leaving, 2)?;
    }
    retain(&mut candidates, top_slot_claim, EntityLifecycle::Leaving, 3)?;

    let mut transitions = candidates
        .into_values()
        .map(|(claim, lifecycle, _)| (claim, lifecycle))
        .collect::<Vec<_>>();
    transitions.sort_by_key(|(claim, _)| claim.entity().to_bits());
    Some(transitions)
}

fn target_preparation_permanent_mismatch(
    source: LifecyclePreflightSource,
    expectation: LifecyclePreflightExpectation,
    observed: LifecyclePreflightObserved,
) -> LifecyclePreflightEvidence {
    LifecyclePreflightEvidence::new(source, expectation, observed)
}

fn observe_target_preparation_permanent_entity(
    world: &World,
    registry: &RouteEntityRegistry,
    entity: bevy_ecs::entity::Entity,
) -> FrozenTargetPreparationPermanentObservedEntity {
    FrozenTargetPreparationPermanentObservedEntity {
        entity,
        parent: world
            .get::<bevy_ecs::hierarchy::ChildOf>(entity)
            .map(bevy_ecs::hierarchy::ChildOf::parent),
        children: world
            .get::<bevy_ecs::hierarchy::Children>(entity)
            .map(|children| children.iter().copied().collect())
            .unwrap_or_default(),
        registry_claim: registry.record(entity).map(|record| record.claim()),
        registry_manifest: registry
            .record(entity)
            .map(|record| record.manifest().clone()),
        ownership_seal: world.get::<RouteOwnershipSeal>(entity).copied(),
        manifest: ComponentManifest::from_world(world, entity),
        frame_lifecycle: world.get::<FrameLifecycle>(entity).copied(),
        child_slot_lifecycle: world.get::<ChildSlotLifecycle>(entity).copied(),
        child_slot_authority: world.get::<ChildSlotAuthority>(entity).cloned(),
        provisional_slot_lifecycle: world.get::<ProvisionalSlotLifecycle>(entity).copied(),
    }
}

fn target_preparation_permanent_structure_is_exact(
    observed: &FrozenTargetPreparationPermanentObservedEntity,
    expected: &FrozenTargetPreparationPermanentObservedEntity,
) -> bool {
    observed.entity == expected.entity
        && observed.parent == expected.parent
        && observed.children == expected.children
        && observed.registry_claim == expected.registry_claim
        && observed.registry_manifest == expected.registry_manifest
        && observed.ownership_seal == expected.ownership_seal
        && observed.manifest == expected.manifest
}

fn target_preparation_permanent_authority_values_are_exact(
    observed: &FrozenTargetPreparationPermanentObservedEntity,
    expected: &FrozenTargetPreparationPermanentObservedEntity,
) -> bool {
    observed.frame_lifecycle == expected.frame_lifecycle
        && observed.child_slot_lifecycle == expected.child_slot_lifecycle
        && observed.child_slot_authority == expected.child_slot_authority
        && observed.provisional_slot_lifecycle == expected.provisional_slot_lifecycle
}

fn target_preparation_permanent_claim_belongs_to_host(
    host: &HostBinding,
    claim: &ExactRegistryClaim,
) -> bool {
    match claim.owner() {
        RegistryOwner::Committed(
            EntityOwner::FrameShell(frame)
            | EntityOwner::FrameContentTarget(frame)
            | EntityOwner::FrameOutput { frame, .. },
        )
        | RegistryOwner::Committed(EntityOwner::ChildSlot { parent: frame, .. })
        | RegistryOwner::Provisional(ProvisionalOwner::ChildSlot { parent: frame, .. }) => {
            bevy_ui_router_core::integration::host_version_matches_frame(&host.version, frame)
        }
        RegistryOwner::Committed(EntityOwner::HostTopSlot {
            host: version,
            slot,
        }) => version == &host.version && *slot == host.slot_id,
        RegistryOwner::Provisional(ProvisionalOwner::HostTopSlot(_)) => false,
    }
}

fn target_preparation_permanent_relationship_closure_is_exact(
    world: &World,
    closure: &std::collections::HashSet<bevy_ecs::entity::Entity>,
) -> bool {
    for parent in closure.iter().copied() {
        let children = world
            .get::<bevy_ecs::hierarchy::Children>(parent)
            .map(|children| children.iter().copied().collect::<Vec<_>>())
            .unwrap_or_default();
        let unique = children
            .iter()
            .copied()
            .collect::<std::collections::HashSet<_>>();
        if unique.len() != children.len()
            || children.iter().any(|child| {
                !closure.contains(child)
                    || world
                        .get::<bevy_ecs::hierarchy::ChildOf>(*child)
                        .map(bevy_ecs::hierarchy::ChildOf::parent)
                        != Some(parent)
            })
        {
            return false;
        }
    }
    world.iter_entities().all(|entity| {
        let Some(parent) = entity
            .get::<bevy_ecs::hierarchy::ChildOf>()
            .map(bevy_ecs::hierarchy::ChildOf::parent)
        else {
            return true;
        };
        !closure.contains(&parent)
            || closure.contains(&entity.id())
                && world
                    .get::<bevy_ecs::hierarchy::Children>(parent)
                    .is_some_and(|children| {
                        children
                            .iter()
                            .filter(|child| **child == entity.id())
                            .count()
                            == 1
                    })
    })
}

fn refreeze_back_target_authority_loss(
    world: &World,
    entry: &RouterEntry,
    splice: &SealedReconciliationSplice,
) -> Option<LifecyclePreflightEvidence> {
    let entering_boundaries = splice
        .entering()
        .claims()
        .map(ExactRegistryClaim::entity)
        .collect::<Vec<_>>();
    let has_leaving =
        !splice.leaving().frames().is_empty() || !splice.leaving().child_slots().is_empty();
    let current_loss = if world.get_resource::<RouteEntityRegistry>().is_none() {
        Some(LifecyclePreflightEvidence::new(
            LifecyclePreflightSource::RequiredResources(RequiredResourceSet::ROUTE_ENTITY_REGISTRY),
            LifecyclePreflightExpectation::ResourcePresent,
            LifecyclePreflightObserved::Missing,
        ))
    } else if entry
        .host
        .as_ref()
        .is_none_or(|host| !current_host_binding_is_exact(world, host))
    {
        Some(LifecyclePreflightEvidence::new(
            LifecyclePreflightSource::HostBinding,
            LifecyclePreflightExpectation::ExactAuthority,
            LifecyclePreflightObserved::Drifted,
        ))
    } else if validate_preserved_branch_local(
        world,
        splice.preserved(),
        splice.slot_roles(),
        &entering_boundaries,
    )
    .is_err()
    {
        Some(LifecyclePreflightEvidence::new(
            LifecyclePreflightSource::PreservedCurrent,
            LifecyclePreflightExpectation::ExactAuthority,
            LifecyclePreflightObserved::Drifted,
        ))
    } else if has_leaving
        && validate_leaving_branch_for_splice(world, splice.leaving(), splice.slot_roles()).is_err()
    {
        Some(LifecyclePreflightEvidence::new(
            LifecyclePreflightSource::LeavingCurrent,
            LifecyclePreflightExpectation::ExactAuthority,
            LifecyclePreflightObserved::Drifted,
        ))
    } else {
        None
    };
    current_loss.or_else(|| {
        validate_branch_descendants(world, splice.leaving())
            .err()
            .map(|_| {
                LifecyclePreflightEvidence::new(
                    LifecyclePreflightSource::LeavingCurrent,
                    LifecyclePreflightExpectation::ExactRelationshipClosure,
                    LifecyclePreflightObserved::Drifted,
                )
            })
    })
}

fn freeze_back_target_preparation_permanent_physical(
    world: &World,
    entry: &RouterEntry,
    splice: &SealedReconciliationSplice,
    continuation: &LifecycleContinuationCapacity,
) -> Result<FrozenBackTargetPreparationPermanentPhysicalFacts, LifecyclePreflightEvidence> {
    freeze_back_target_preparation_physical(world, entry, splice, continuation, None)
}

fn freeze_back_target_authority_lost_physical(
    world: &World,
    entry: &RouterEntry,
    splice: &SealedReconciliationSplice,
    continuation: &LifecycleContinuationCapacity,
    expected_evidence: LifecyclePreflightEvidence,
) -> Result<FrozenBackTargetPreparationPermanentPhysicalFacts, LifecyclePreflightEvidence> {
    freeze_back_target_preparation_physical(
        world,
        entry,
        splice,
        continuation,
        Some(expected_evidence),
    )
}

fn freeze_back_target_preparation_physical(
    world: &World,
    entry: &RouterEntry,
    splice: &SealedReconciliationSplice,
    continuation: &LifecycleContinuationCapacity,
    authority_loss: Option<LifecyclePreflightEvidence>,
) -> Result<FrozenBackTargetPreparationPermanentPhysicalFacts, LifecyclePreflightEvidence> {
    let mismatch = target_preparation_permanent_mismatch;
    if entry.pending_back_target_execution.is_some()
        || entry.active.is_some()
        || entry.pending_commit.as_ref().is_none_or(|pending| {
            pending.ticket.is_none()
                || pending.final_task.is_some()
                || !pending.branch.frames().is_empty()
                || !pending.branch.child_slots().is_empty()
        })
    {
        return Err(mismatch(
            LifecyclePreflightSource::PendingAdmission,
            LifecyclePreflightExpectation::ExactTaskShape,
            LifecyclePreflightObserved::Drifted,
        ));
    }
    let Some(ticket) = entry
        .pending_commit
        .as_ref()
        .and_then(|pending| pending.ticket.as_ref())
        .map(CommitTicket::report_identity)
    else {
        return Err(mismatch(
            LifecyclePreflightSource::PendingAdmission,
            LifecyclePreflightExpectation::ExactAuthority,
            LifecyclePreflightObserved::Drifted,
        ));
    };
    let Some(registry) = world.get_resource::<RouteEntityRegistry>() else {
        return Err(mismatch(
            LifecyclePreflightSource::RequiredResources(RequiredResourceSet::ROUTE_ENTITY_REGISTRY),
            LifecyclePreflightExpectation::ResourcePresent,
            LifecyclePreflightObserved::Missing,
        ));
    };
    let Some(host) = entry.host.as_ref() else {
        return Err(mismatch(
            LifecyclePreflightSource::HostBinding,
            LifecyclePreflightExpectation::ExactAuthority,
            LifecyclePreflightObserved::Missing,
        ));
    };
    if world
        .get::<crate::app_ext::RouterHostRegistration>(host.host)
        .is_none_or(|registration| registration.router() != entry.handle)
        || !current_host_binding_is_exact(world, host)
    {
        return Err(mismatch(
            LifecyclePreflightSource::HostBinding,
            LifecyclePreflightExpectation::ExactAuthority,
            LifecyclePreflightObserved::Drifted,
        ));
    }
    if registry.record(host.host).is_some() || world.get::<RouteOwnershipSeal>(host.host).is_some()
    {
        return Err(mismatch(
            LifecyclePreflightSource::HostBinding,
            LifecyclePreflightExpectation::ExactAuthority,
            LifecyclePreflightObserved::Drifted,
        ));
    }

    if let Some(expected) = authority_loss {
        if refreeze_back_target_authority_loss(world, entry, splice) != Some(expected) {
            return Err(mismatch(
                LifecyclePreflightSource::TargetPartition,
                LifecyclePreflightExpectation::ExactAuthority,
                LifecyclePreflightObserved::Drifted,
            ));
        }
    } else {
        let entering_boundaries = splice
            .entering()
            .claims()
            .map(ExactRegistryClaim::entity)
            .collect::<Vec<_>>();
        if validate_preserved_branch_local(
            world,
            splice.preserved(),
            splice.slot_roles(),
            &entering_boundaries,
        )
        .is_err()
        {
            return Err(mismatch(
                LifecyclePreflightSource::PreservedCurrent,
                LifecyclePreflightExpectation::ExactAuthority,
                LifecyclePreflightObserved::Drifted,
            ));
        }
        if validate_leaving_branch_for_splice(world, splice.leaving(), splice.slot_roles()).is_err()
        {
            return Err(mismatch(
                LifecyclePreflightSource::LeavingCurrent,
                LifecyclePreflightExpectation::ExactAuthority,
                LifecyclePreflightObserved::Drifted,
            ));
        }
        if validate_reconciliation_entering_branch(
            world,
            splice.entering(),
            splice.preserved(),
            splice.leaving(),
        )
        .is_err()
        {
            return Err(mismatch(
                LifecyclePreflightSource::PendingEntering,
                LifecyclePreflightExpectation::ExactAuthority,
                LifecyclePreflightObserved::Drifted,
            ));
        }
        if validate_active_branch_for_splice(
            world,
            splice.preserved(),
            splice.entering(),
            splice.slot_roles(),
        )
        .is_err()
        {
            return Err(mismatch(
                LifecyclePreflightSource::CrossRoleRebase,
                LifecyclePreflightExpectation::RebaseCompatible,
                LifecyclePreflightObserved::Contradictory,
            ));
        }
    }

    let normal_transitions = splice.lifecycle_transitions();
    let Some(normal_registry_plan) = registry.prepare_lifecycle_mutation(&normal_transitions)
    else {
        return Err(mismatch(
            LifecyclePreflightSource::TargetPartition,
            LifecyclePreflightExpectation::ExactAuthority,
            LifecyclePreflightObserved::Contradictory,
        ));
    };
    let host_closure = observed_descendant_closure(world, [host.host]);
    if !host_closure.contains(&host.top_slot)
        || !target_preparation_permanent_relationship_closure_is_exact(world, &host_closure)
    {
        return Err(mismatch(
            LifecyclePreflightSource::TargetPartition,
            LifecyclePreflightExpectation::ExactRelationshipClosure,
            LifecyclePreflightObserved::Drifted,
        ));
    }

    let mut accepted_roles = std::collections::HashMap::new();
    for claim in splice
        .preserved()
        .claims()
        .chain(splice.entering().claims())
        .chain(splice.leaving().claims())
        .chain(splice.slot_roles().iter().map(|role| role.claim()))
        .chain(std::iter::once(&host.top_slot_claim))
    {
        if accepted_roles
            .insert(claim.entity(), claim.clone())
            .is_some_and(|previous| previous != *claim)
        {
            return Err(mismatch(
                LifecyclePreflightSource::TargetPartition,
                LifecyclePreflightExpectation::ExactAuthority,
                LifecyclePreflightObserved::Contradictory,
            ));
        }
    }
    if accepted_roles.iter().any(|(entity, claim)| {
        let target_authority_is_usable = if authority_loss.is_some() {
            registry
                .record(*entity)
                .is_some_and(|record| record.claim() == *claim)
                && cleanup_claim_has_exact_authority(world, claim)
        } else {
            registered_claim_has_exact_authority_and_manifest_coverage(
                world,
                registry,
                claim,
                *entity == host.top_slot,
            )
        };
        *entity == host.host
            || (authority_loss.is_none() && !host_closure.contains(entity))
            || !target_preparation_permanent_claim_belongs_to_host(host, claim)
            || !target_authority_is_usable
    }) {
        return Err(mismatch(
            LifecyclePreflightSource::TargetPartition,
            LifecyclePreflightExpectation::ExactAuthority,
            LifecyclePreflightObserved::Drifted,
        ));
    }

    if normal_transitions.iter().any(|(claim, _)| {
        let target_authority_is_usable = if authority_loss.is_some() {
            registry
                .record(claim.entity())
                .is_some_and(|record| record.claim() == *claim)
                && cleanup_claim_has_exact_authority(world, claim)
        } else {
            registered_claim_has_exact_authority_and_manifest_coverage(
                world, registry, claim, false,
            )
        };
        accepted_roles.get(&claim.entity()) != Some(claim) || !target_authority_is_usable
    }) {
        return Err(mismatch(
            LifecyclePreflightSource::TargetPartition,
            LifecyclePreflightExpectation::ExactAuthority,
            LifecyclePreflightObserved::Drifted,
        ));
    }

    let Some(projected_claims) =
        projected_target_preparation_permanent_claims(registry, splice, &host.top_slot_claim)
    else {
        return Err(mismatch(
            LifecyclePreflightSource::TargetPartition,
            LifecyclePreflightExpectation::ExactAuthority,
            LifecyclePreflightObserved::Contradictory,
        ));
    };
    if projected_claims.len() != accepted_roles.len()
        || projected_claims.iter().any(|projected| {
            accepted_roles
                .get(&projected.entity())
                .is_none_or(|source| {
                    source.owner() != projected.owner() || source.seal() != projected.seal()
                })
        })
    {
        return Err(mismatch(
            LifecyclePreflightSource::TargetPartition,
            LifecyclePreflightExpectation::ExactAuthority,
            LifecyclePreflightObserved::Contradictory,
        ));
    }
    let splice_targets = projected_claims
        .iter()
        .filter(|claim| claim.entity() != host.top_slot)
        .map(ExactRegistryClaim::entity)
        .collect::<Vec<_>>();
    if splice_targets.iter().any(|target| {
        *target == host.host
            || *target == host.top_slot
            || (authority_loss.is_none() && !host_closure.contains(target))
    }) {
        return Err(mismatch(
            LifecyclePreflightSource::TargetPartition,
            LifecyclePreflightExpectation::ExactAuthority,
            LifecyclePreflightObserved::Drifted,
        ));
    }

    let target_set = projected_claims
        .iter()
        .map(ExactRegistryClaim::entity)
        .collect::<std::collections::HashSet<_>>();
    let mut target_observed = projected_claims
        .iter()
        .map(ExactRegistryClaim::entity)
        .map(|entity| observe_target_preparation_permanent_entity(world, registry, entity))
        .collect::<Vec<_>>();
    target_observed.sort_by_key(|observed| observed.entity.to_bits());

    let mut closure_entities = host_closure.iter().copied().collect::<Vec<_>>();
    closure_entities.sort_by_key(|entity| entity.to_bits());
    let mut protected_entities = Vec::new();
    for entity in closure_entities {
        if entity == host.host || target_set.contains(&entity) {
            continue;
        }
        if accepted_roles.contains_key(&entity) {
            protected_entities.push(entity);
            continue;
        }
        match registry.record(entity) {
            None if world.get::<RouteOwnershipSeal>(entity).is_none() => {
                protected_entities.push(entity);
            }
            None => {
                return Err(mismatch(
                    LifecyclePreflightSource::TargetPartition,
                    LifecyclePreflightExpectation::ExactAuthority,
                    LifecyclePreflightObserved::Drifted,
                ));
            }
            Some(record) => {
                let claim = record.claim();
                if !registered_claim_has_exact_authority_and_manifest_coverage(
                    world, registry, &claim, false,
                ) {
                    return Err(mismatch(
                        LifecyclePreflightSource::TargetPartition,
                        LifecyclePreflightExpectation::ExactAuthority,
                        LifecyclePreflightObserved::Drifted,
                    ));
                }
                if target_preparation_permanent_claim_belongs_to_host(host, &claim) {
                    return Err(mismatch(
                        LifecyclePreflightSource::TargetPartition,
                        LifecyclePreflightExpectation::ExactAuthority,
                        LifecyclePreflightObserved::Contradictory,
                    ));
                }
                protected_entities.push(entity);
            }
        }
    }
    protected_entities.sort_by_key(|entity| entity.to_bits());
    let protected_set = protected_entities
        .iter()
        .copied()
        .collect::<std::collections::HashSet<_>>();
    let subtree_has_target = |entity| {
        observed_descendant_closure(world, [entity])
            .iter()
            .any(|descendant| target_set.contains(descendant))
    };
    let mut protected_roots = protected_entities
        .iter()
        .copied()
        .filter(|entity| {
            if subtree_has_target(*entity) {
                return false;
            }
            world
                .get::<bevy_ecs::hierarchy::ChildOf>(*entity)
                .map(bevy_ecs::hierarchy::ChildOf::parent)
                .is_none_or(|parent| !protected_set.contains(&parent) || subtree_has_target(parent))
        })
        .collect::<Vec<_>>();
    protected_roots.sort_by_key(|entity| entity.to_bits());
    let protected_root_closure =
        observed_descendant_closure(world, protected_roots.iter().copied());
    if protected_root_closure
        .iter()
        .any(|entity| target_set.contains(entity) || !protected_set.contains(entity))
        || protected_entities
            .iter()
            .any(|entity| !subtree_has_target(*entity) && !protected_root_closure.contains(entity))
    {
        return Err(mismatch(
            LifecyclePreflightSource::TargetPartition,
            LifecyclePreflightExpectation::ExactRelationshipClosure,
            LifecyclePreflightObserved::Contradictory,
        ));
    }
    let protected_observed = protected_entities
        .iter()
        .copied()
        .map(|entity| observe_target_preparation_permanent_entity(world, registry, entity))
        .collect::<Vec<_>>();

    let normal_frozen_manifest =
        if authority_loss.is_some() || splice.leaving().obligation_count() == 0 {
            FrozenCleanupManifest::empty()
        } else {
            let Some(manifest) = FrozenCleanupManifest::from_leaving_branch(
                world,
                registry,
                splice.leaving(),
                normal_registry_plan.resulting_claims(),
                (!splice.entering().frames().is_empty()).then_some(splice.entering()),
            ) else {
                return Err(mismatch(
                    LifecyclePreflightSource::TargetPartition,
                    LifecyclePreflightExpectation::ExactRelationshipClosure,
                    LifecyclePreflightObserved::Contradictory,
                ));
            };
            manifest
        };
    let ecs_manifest = FrozenCleanupManifest::from_target_preparation_permanent(
        world,
        registry,
        splice,
        &projected_claims,
        host,
    )
    .map_err(|_| {
        mismatch(
            LifecyclePreflightSource::TargetPartition,
            LifecyclePreflightExpectation::ExactRelationshipClosure,
            LifecyclePreflightObserved::Contradictory,
        )
    })?;
    let host_children_before = world
        .get::<bevy_ecs::hierarchy::Children>(host.host)
        .map(|children| children.iter().copied().collect::<Vec<_>>())
        .unwrap_or_default();
    let mut host_children_after = host_children_before.clone();
    let Some(top_slot_index) = host_children_after
        .iter()
        .position(|child| *child == host.top_slot)
    else {
        return Err(mismatch(
            LifecyclePreflightSource::HostBinding,
            LifecyclePreflightExpectation::ExactRelationshipClosure,
            LifecyclePreflightObserved::Drifted,
        ));
    };
    host_children_after.remove(top_slot_index);
    let host_edge = FrozenTargetPreparationPermanentHostEdge {
        host: host.host,
        top_slot: host.top_slot,
        host_observed: observe_target_preparation_permanent_entity(world, registry, host.host),
        host_children_before,
        host_children_after,
        top_slot_parent_before: Some(host.host),
        top_slot_parent_after: None,
    };
    let scope = continuation.preparation_boundary_scope();
    Ok(FrozenBackTargetPreparationPermanentPhysicalFacts {
        physical: FrozenTargetPreparationPermanentPhysicalManifest {
            lineage: FrozenTargetPreparationPermanentLineage {
                router: entry.handle,
                host: host.host,
                ticket,
            },
            target_observed,
            protected: FrozenTargetPreparationPermanentProtectedClosures {
                roots: protected_roots,
                observed: protected_observed,
            },
            allowed_write: FrozenTargetPreparationPermanentAllowedWriteSet {
                splice_targets,
                host_edge,
            },
            repair_missing_pickable: authority_loss.is_some(),
        },
        normal_frozen_manifest,
        ecs_manifest,
        scope,
    })
}

fn accept_classified_back_target_authority_lost(
    world: &World,
    entry: &mut RouterEntry,
    owner: TargetPreparationOwner,
    evidence: LifecyclePreflightEvidence,
) -> PendingBackTargetClassification {
    let frozen = match freeze_back_target_authority_lost_physical(
        world,
        entry,
        &owner.splice,
        &owner.continuation,
        evidence,
    ) {
        Ok(frozen) => frozen,
        Err(mismatch) => {
            return PendingBackTargetClassification::AuthorityLostPhysicalMismatch {
                owner,
                evidence,
                mismatch,
            };
        }
    };
    let FrozenBackTargetPreparationPermanentPhysicalFacts {
        physical,
        normal_frozen_manifest: _,
        ecs_manifest,
        scope,
    } = frozen;
    let TargetPreparationOwner {
        splice,
        history,
        continuation,
    } = owner;
    let manifest = CleanupManifest::new(
        logical_cleanup_roots(splice.leaving()),
        ecs_manifest.digest(),
        ecs_manifest.len(),
    );
    match entry
        .engine
        .accept_back_target_authority_lost(continuation, scope, &manifest)
    {
        BackTargetPreparationAcceptance::Matched(settlement) => {
            PendingBackTargetClassification::AuthorityLost {
                physical: BackTargetClassifiedPhysicalOwner {
                    splice,
                    history,
                    authority_preparation: Some(AcceptedBackTargetAuthorityPreparation {
                        physical,
                        ecs_manifest,
                        core_binding: BackTargetPermanentAuthorityManifest { cleanup: manifest },
                    }),
                },
                settlement,
                evidence,
            }
        }
        BackTargetPreparationAcceptance::Mismatch { capacity, mismatch } => {
            PendingBackTargetClassification::AuthorityLostAcceptanceMismatch {
                owner: TargetPreparationOwner {
                    splice,
                    history,
                    continuation: capacity,
                },
                evidence,
                mismatch,
            }
        }
    }
}

fn accept_classified_back_target_pending_rejected(
    entry: &mut RouterEntry,
    owner: TargetPreparationOwner,
    evidence: LifecyclePreflightEvidence,
) -> PendingBackTargetClassification {
    let scope = owner.continuation.preparation_boundary_scope();
    let TargetPreparationOwner {
        splice,
        history,
        continuation,
    } = owner;
    match entry
        .engine
        .accept_back_target_pending_rejected(continuation, scope)
    {
        BackTargetPreparationAcceptance::Matched(continuation) => {
            PendingBackTargetClassification::PendingRejected {
                physical: BackTargetClassifiedPhysicalOwner {
                    splice,
                    history,
                    authority_preparation: None,
                },
                continuation,
                evidence,
            }
        }
        BackTargetPreparationAcceptance::Mismatch { capacity, mismatch } => {
            PendingBackTargetClassification::PendingRejectedAcceptanceMismatch {
                owner: TargetPreparationOwner {
                    splice,
                    history,
                    continuation: capacity,
                },
                evidence,
                mismatch,
            }
        }
    }
}

pub(crate) fn prepare_back_target_classifier_permanent(
    world: &World,
    entry: &mut RouterEntry,
    owner: PreparationPermanentOwner<TargetPreparationOwner>,
) -> BackTargetClassifierPermanentPreparationOutcome {
    let Some(ticket) = entry
        .pending_commit
        .as_ref()
        .and_then(|pending| pending.ticket.as_ref())
    else {
        return BackTargetClassifierPermanentPreparationOutcome::PhysicalMismatch {
            owner,
            evidence: target_preparation_permanent_mismatch(
                LifecyclePreflightSource::PendingAdmission,
                LifecyclePreflightExpectation::ExactAuthority,
                LifecyclePreflightObserved::Drifted,
            ),
        };
    };
    if let BackTargetExecutionPairingOutcome::Mismatch(mismatch) =
        entry.engine.validate_back_target_execution_pairing(
            BackTargetExecutionOwnerRef::Classifying {
                history: &owner.owner.history,
                continuation: &owner.owner.continuation,
            },
            BackTargetPhysicalOwnerRef::CommitTicket(ticket),
        )
    {
        return BackTargetClassifierPermanentPreparationOutcome::ReceiverMismatch {
            owner,
            mismatch,
        };
    }

    let physical = freeze_back_target_preparation_permanent_physical(
        world,
        entry,
        &owner.owner.splice,
        &owner.owner.continuation,
    );

    let FrozenBackTargetPreparationPermanentPhysicalFacts {
        physical,
        normal_frozen_manifest: _,
        ecs_manifest,
        scope,
    } = match physical {
        Ok(physical) => physical,
        Err(evidence) => {
            return BackTargetClassifierPermanentPreparationOutcome::PhysicalMismatch {
                owner,
                evidence,
            };
        }
    };
    let PreparationPermanentOwner {
        owner:
            TargetPreparationOwner {
                splice,
                history,
                continuation,
            },
        evidence,
    } = owner;
    let manifest = CleanupManifest::new(
        logical_cleanup_roots(splice.leaving()),
        ecs_manifest.digest(),
        ecs_manifest.len(),
    );
    let cleanup = TargetPreparationPermanentCleanupOwner {
        physical,
        branch: FrozenTargetPreparationPermanentBranch::Classifier {
            physical: BackTargetClassifiedPhysicalOwner {
                splice,
                history,
                authority_preparation: None,
            },
            evidence,
        },
    };
    match entry
        .engine
        .accept_back_target_authority_lost(continuation, scope, &manifest)
    {
        BackTargetPreparationAcceptance::Matched(settlement) => {
            BackTargetClassifierPermanentPreparationOutcome::Prepared(
                TargetPreparationPermanentDisposition {
                    cleanup,
                    settlement,
                    ecs_manifest,
                    core_binding: BackTargetPermanentAuthorityManifest { cleanup: manifest },
                },
            )
        }
        BackTargetPreparationAcceptance::Mismatch { capacity, mismatch } => {
            let TargetPreparationPermanentCleanupOwner {
                physical: _,
                branch,
            } = cleanup;
            let FrozenTargetPreparationPermanentBranch::Classifier {
                physical:
                    BackTargetClassifiedPhysicalOwner {
                        splice,
                        history,
                        authority_preparation: None,
                    },
                evidence,
            } = branch
            else {
                unreachable!("the classifier adapter retains its branch-specific owner")
            };
            BackTargetClassifierPermanentPreparationOutcome::ReceiverMismatch {
                owner: PreparationPermanentOwner {
                    owner: TargetPreparationOwner {
                        splice,
                        history,
                        continuation: capacity,
                    },
                    evidence,
                },
                mismatch,
            }
        }
    }
}

pub(crate) fn prepare_back_target_ready_plan_permanent(
    world: &World,
    entry: &mut RouterEntry,
    owner: PreparationPermanentOwner<BackTargetReadyAcceptanceOwner>,
) -> BackTargetReadyPlanPermanentPreparationOutcome {
    let Some(ticket) = entry
        .pending_commit
        .as_ref()
        .and_then(|pending| pending.ticket.as_ref())
    else {
        return BackTargetReadyPlanPermanentPreparationOutcome::PhysicalMismatch {
            owner,
            evidence: target_preparation_permanent_mismatch(
                LifecyclePreflightSource::PendingAdmission,
                LifecyclePreflightExpectation::ExactAuthority,
                LifecyclePreflightObserved::Drifted,
            ),
        };
    };
    if let BackTargetExecutionPairingOutcome::Mismatch(mismatch) =
        entry.engine.validate_back_target_execution_pairing(
            BackTargetExecutionOwnerRef::Classifying {
                history: &owner.owner.physical.classified.history,
                continuation: &owner.owner.continuation,
            },
            BackTargetPhysicalOwnerRef::CommitTicket(ticket),
        )
    {
        return BackTargetReadyPlanPermanentPreparationOutcome::ReceiverMismatch {
            owner,
            mismatch,
        };
    }

    let refrozen = match freeze_back_target_preparation_permanent_physical(
        world,
        entry,
        &owner.owner.physical.classified.splice,
        &owner.owner.continuation,
    ) {
        Ok(refrozen) => refrozen,
        Err(evidence) => {
            return BackTargetReadyPlanPermanentPreparationOutcome::PhysicalMismatch {
                owner,
                evidence,
            };
        }
    };
    if refrozen.normal_frozen_manifest != owner.owner.frozen_manifest {
        return BackTargetReadyPlanPermanentPreparationOutcome::PhysicalMismatch {
            owner,
            evidence: target_preparation_permanent_mismatch(
                LifecyclePreflightSource::TargetPartition,
                LifecyclePreflightExpectation::ExactAuthority,
                LifecyclePreflightObserved::Drifted,
            ),
        };
    }

    let FrozenBackTargetPreparationPermanentPhysicalFacts {
        physical: refrozen_physical,
        normal_frozen_manifest: _,
        ecs_manifest,
        scope,
    } = refrozen;
    let PreparationPermanentOwner { owner, evidence } = owner;
    let BackTargetReadyAcceptanceOwner {
        physical,
        continuation,
        frozen_manifest,
        manifest,
    } = owner;
    let dedicated_manifest = CleanupManifest::new(
        logical_cleanup_roots(physical.classified.splice.leaving()),
        ecs_manifest.digest(),
        ecs_manifest.len(),
    );
    let cleanup = TargetPreparationPermanentCleanupOwner {
        physical: refrozen_physical,
        branch: FrozenTargetPreparationPermanentBranch::ReadyPlan {
            physical,
            normal_frozen_manifest: frozen_manifest,
            normal_manifest: manifest,
            evidence,
        },
    };
    match entry
        .engine
        .accept_back_target_authority_lost(continuation, scope, &dedicated_manifest)
    {
        BackTargetPreparationAcceptance::Matched(settlement) => {
            BackTargetReadyPlanPermanentPreparationOutcome::Prepared(
                TargetPreparationPermanentDisposition {
                    cleanup,
                    settlement,
                    ecs_manifest,
                    core_binding: BackTargetPermanentAuthorityManifest {
                        cleanup: dedicated_manifest,
                    },
                },
            )
        }
        BackTargetPreparationAcceptance::Mismatch { capacity, mismatch } => {
            let TargetPreparationPermanentCleanupOwner {
                physical: _,
                branch,
            } = cleanup;
            let FrozenTargetPreparationPermanentBranch::ReadyPlan {
                physical,
                normal_frozen_manifest: frozen_manifest,
                normal_manifest: manifest,
                evidence,
            } = branch
            else {
                unreachable!("the Ready-plan adapter retains its branch-specific owner")
            };
            BackTargetReadyPlanPermanentPreparationOutcome::ReceiverMismatch {
                owner: PreparationPermanentOwner {
                    owner: BackTargetReadyAcceptanceOwner {
                        physical,
                        continuation: capacity,
                        frozen_manifest,
                        manifest,
                    },
                    evidence,
                },
                mismatch,
            }
        }
    }
}

fn target_preparation_permanent_dispatch_entry_preflight(
    world: &World,
    entry: &RouterEntry,
    physical: &FrozenTargetPreparationPermanentPhysicalManifest,
) -> Result<(), LifecyclePreflightEvidence> {
    let Some(host) = entry.host.as_ref() else {
        return Err(target_preparation_permanent_mismatch(
            LifecyclePreflightSource::HostBinding,
            LifecyclePreflightExpectation::ExactAuthority,
            LifecyclePreflightObserved::Missing,
        ));
    };
    let Some(pending) = entry.pending_commit.as_ref() else {
        return Err(target_preparation_permanent_mismatch(
            LifecyclePreflightSource::PendingAdmission,
            LifecyclePreflightExpectation::ExactAuthority,
            LifecyclePreflightObserved::Missing,
        ));
    };
    let Some(ticket) = pending.ticket.as_ref() else {
        return Err(target_preparation_permanent_mismatch(
            LifecyclePreflightSource::PendingAdmission,
            LifecyclePreflightExpectation::ExactAuthority,
            LifecyclePreflightObserved::Missing,
        ));
    };
    let Some(_registry) = world.get_resource::<RouteEntityRegistry>() else {
        return Err(target_preparation_permanent_mismatch(
            LifecyclePreflightSource::RequiredResources(RequiredResourceSet::ROUTE_ENTITY_REGISTRY),
            LifecyclePreflightExpectation::ResourcePresent,
            LifecyclePreflightObserved::Missing,
        ));
    };
    if entry.handle != physical.lineage.router || host.host != physical.lineage.host {
        return Err(target_preparation_permanent_mismatch(
            LifecyclePreflightSource::HostBinding,
            LifecyclePreflightExpectation::ExactAuthority,
            LifecyclePreflightObserved::Drifted,
        ));
    }
    if ticket.report_identity() != physical.lineage.ticket {
        return Err(target_preparation_permanent_mismatch(
            LifecyclePreflightSource::PendingAdmission,
            LifecyclePreflightExpectation::ExactAuthority,
            LifecyclePreflightObserved::Drifted,
        ));
    }
    let current_entry_inputs_are_vacant =
        entry.active.is_none() && entry.back_capacity_reservations.is_empty();
    if pending.final_task.is_some()
        || !pending.branch.frames().is_empty()
        || !pending.branch.child_slots().is_empty()
        || !current_entry_inputs_are_vacant
        || entry.host_settlement.is_some()
        || entry.pending_frame_drive.is_some()
        || entry.pending_mount_frame_task.is_some()
        || entry.pending_mount_request.is_some()
        || entry.pending_mount_mutation_rejection.is_some()
        || entry.pending_failed_branch.is_some()
        || entry.failure_cleanup_permanent.is_some()
        || entry.projectionless_fail_close.is_some()
        || entry.pending_back_target_execution.is_some()
        || entry.pending_back_target_permanent_cleanup.is_some()
        || entry.cleanup_ledger.is_some()
        || entry.authority_protocol_residual.is_some()
        || entry.back_capacity_enqueue_retire_residual.is_some()
        || entry.pending_back_resolution_action.is_some()
        || entry.pending_bound_back_resolution.is_some()
        || entry.pending_back_no_target_completion.is_some()
        || entry.pending_back_no_target_preparation.is_some()
        || entry.pending_back_no_target_authority_cleanup.is_some()
    {
        return Err(target_preparation_permanent_mismatch(
            LifecyclePreflightSource::PendingAdmission,
            LifecyclePreflightExpectation::ExactTaskShape,
            LifecyclePreflightObserved::Contradictory,
        ));
    }

    if !current_host_binding_is_exact(world, host) {
        return Err(target_preparation_permanent_mismatch(
            LifecyclePreflightSource::HostBinding,
            LifecyclePreflightExpectation::ExactAuthority,
            LifecyclePreflightObserved::Drifted,
        ));
    }
    target_preparation_permanent_prewrite_refreeze(world, physical)
}

pub(crate) fn dispatch_back_target_permanent(
    world: &World,
    entry: &mut RouterEntry,
    disposition: TargetPreparationPermanentDisposition,
) -> BackTargetPermanentDispatchOutcome {
    dispatch_back_target_permanent_with_installation_observer(world, entry, disposition, |_| {})
}

#[cfg(test)]
fn dispatch_back_target_permanent_with_test_installation_observer<F>(
    world: &World,
    entry: &mut RouterEntry,
    disposition: TargetPreparationPermanentDisposition,
    observe_installed: F,
) -> BackTargetPermanentDispatchOutcome
where
    F: FnOnce(&PendingTargetPreparationPermanentCleanup),
{
    dispatch_back_target_permanent_with_installation_observer(
        world,
        entry,
        disposition,
        observe_installed,
    )
}

fn dispatch_back_target_permanent_with_installation_observer<F>(
    world: &World,
    entry: &mut RouterEntry,
    disposition: TargetPreparationPermanentDisposition,
    observe_installed: F,
) -> BackTargetPermanentDispatchOutcome
where
    F: FnOnce(&PendingTargetPreparationPermanentCleanup),
{
    if let Err(evidence) = target_preparation_permanent_dispatch_entry_preflight(
        world,
        entry,
        &disposition.cleanup.physical,
    ) {
        return BackTargetPermanentDispatchOutcome::EntryMismatch {
            disposition,
            evidence,
        };
    }

    let ledger = match CleanupLedger::reserve(disposition.ecs_manifest.clone()) {
        Ok(ledger) => PreparedTargetPreparationPermanentLedger {
            ledger,
            validation: disposition.cleanup.physical.clone(),
        },
        Err(error) => {
            return BackTargetPermanentDispatchOutcome::LedgerRejected { disposition, error };
        }
    };
    let TargetPreparationPermanentDisposition {
        cleanup,
        settlement,
        ecs_manifest,
        core_binding: BackTargetPermanentAuthorityManifest { cleanup: manifest },
    } = disposition;
    dispatch_prepared_back_target_permanent_with_installation_observer(
        world,
        entry,
        PreparedTargetPreparationPermanentDisposition {
            cleanup,
            settlement,
            ecs_manifest,
            core_binding: BackTargetPermanentAuthorityManifest { cleanup: manifest },
            ledger,
        },
        observe_installed,
    )
}

fn dispatch_prepared_back_target_permanent(
    world: &World,
    entry: &mut RouterEntry,
    disposition: PreparedTargetPreparationPermanentDisposition,
) -> BackTargetPermanentDispatchOutcome {
    dispatch_prepared_back_target_permanent_with_installation_observer(
        world,
        entry,
        disposition,
        |_| {},
    )
}

fn dispatch_prepared_back_target_permanent_with_installation_observer<F>(
    world: &World,
    entry: &mut RouterEntry,
    disposition: PreparedTargetPreparationPermanentDisposition,
    observe_installed: F,
) -> BackTargetPermanentDispatchOutcome
where
    F: FnOnce(&PendingTargetPreparationPermanentCleanup),
{
    if let Err(evidence) = target_preparation_permanent_dispatch_entry_preflight(
        world,
        entry,
        &disposition.cleanup.physical,
    ) {
        return BackTargetPermanentDispatchOutcome::PreparedEntryMismatch {
            disposition,
            evidence,
        };
    }

    let ledger_projection = disposition.ledger.ledger.projection();
    let manifest_digest = disposition.core_binding.cleanup.manifest_digest();
    let manifest_obligation_count = disposition.core_binding.cleanup.obligation_count();
    let frozen_manifest = &disposition.ecs_manifest;
    let validation = &disposition.ledger.validation;
    if validation.lineage.router != disposition.cleanup.physical.lineage.router
        || validation.lineage.host != disposition.cleanup.physical.lineage.host
    {
        return BackTargetPermanentDispatchOutcome::PreparedEntryMismatch {
            disposition,
            evidence: target_preparation_permanent_mismatch(
                LifecyclePreflightSource::HostBinding,
                LifecyclePreflightExpectation::ExactAuthority,
                LifecyclePreflightObserved::Drifted,
            ),
        };
    }
    if validation.lineage.ticket != disposition.cleanup.physical.lineage.ticket {
        return BackTargetPermanentDispatchOutcome::PreparedEntryMismatch {
            disposition,
            evidence: target_preparation_permanent_mismatch(
                LifecyclePreflightSource::PendingAdmission,
                LifecyclePreflightExpectation::ExactAuthority,
                LifecyclePreflightObserved::Drifted,
            ),
        };
    }
    if validation.target_observed != disposition.cleanup.physical.target_observed
        || validation.protected.roots != disposition.cleanup.physical.protected.roots
        || validation.protected.observed != disposition.cleanup.physical.protected.observed
    {
        return BackTargetPermanentDispatchOutcome::PreparedEntryMismatch {
            disposition,
            evidence: target_preparation_permanent_mismatch(
                LifecyclePreflightSource::TargetPartition,
                LifecyclePreflightExpectation::ExactRelationshipClosure,
                LifecyclePreflightObserved::Drifted,
            ),
        };
    }
    let protected = validation
        .protected
        .observed
        .iter()
        .map(|observed| observed.entity)
        .collect::<std::collections::HashSet<_>>();
    if validation
        .allowed_write
        .splice_targets
        .iter()
        .any(|target| protected.contains(target))
        || protected.contains(&validation.allowed_write.host_edge.top_slot)
    {
        return BackTargetPermanentDispatchOutcome::PreparedEntryMismatch {
            disposition,
            evidence: target_preparation_permanent_mismatch(
                LifecyclePreflightSource::TargetPartition,
                LifecyclePreflightExpectation::DisjointClosure,
                LifecyclePreflightObserved::Contradictory,
            ),
        };
    }
    if validation.allowed_write != disposition.cleanup.physical.allowed_write {
        return BackTargetPermanentDispatchOutcome::PreparedEntryMismatch {
            disposition,
            evidence: target_preparation_permanent_mismatch(
                LifecyclePreflightSource::TargetPartition,
                LifecyclePreflightExpectation::ExactTaskShape,
                LifecyclePreflightObserved::Contradictory,
            ),
        };
    }
    if validation.repair_missing_pickable != disposition.cleanup.physical.repair_missing_pickable {
        return BackTargetPermanentDispatchOutcome::PreparedEntryMismatch {
            disposition,
            evidence: target_preparation_permanent_mismatch(
                LifecyclePreflightSource::TargetPartition,
                LifecyclePreflightExpectation::ExactTaskShape,
                LifecyclePreflightObserved::Contradictory,
            ),
        };
    }
    if ledger_projection.manifest_digest() != frozen_manifest.digest()
        || ledger_projection.obligation_count() != frozen_manifest.len()
        || manifest_digest != frozen_manifest.digest()
        || manifest_obligation_count != frozen_manifest.len()
    {
        return BackTargetPermanentDispatchOutcome::PreparedEntryMismatch {
            disposition,
            evidence: target_preparation_permanent_mismatch(
                LifecyclePreflightSource::PendingAdmission,
                LifecyclePreflightExpectation::ExactTaskShape,
                LifecyclePreflightObserved::Contradictory,
            ),
        };
    }
    let PreparedTargetPreparationPermanentDisposition {
        cleanup,
        settlement,
        ecs_manifest,
        core_binding: BackTargetPermanentAuthorityManifest { cleanup: manifest },
        ledger,
    } = disposition;
    let RouterEntry {
        engine,
        host: host_slot,
        pending_commit,
        pending_back_target_permanent_cleanup,
        #[cfg(test)]
        back_target_permanent_begin_callback_calls,
        ..
    } = entry;
    let Some(host) = host_slot.take() else {
        return BackTargetPermanentDispatchOutcome::PreparedEntryMismatch {
            disposition: PreparedTargetPreparationPermanentDisposition {
                cleanup,
                settlement,
                ecs_manifest,
                core_binding: BackTargetPermanentAuthorityManifest { cleanup: manifest },
                ledger,
            },
            evidence: target_preparation_permanent_mismatch(
                LifecyclePreflightSource::HostBinding,
                LifecyclePreflightExpectation::ExactAuthority,
                LifecyclePreflightObserved::Missing,
            ),
        };
    };
    let (retired_host, lease) = host.into_retired();
    let begin = BackTargetPermanentBeginOwner::new(settlement, lease, manifest);
    let installation = PreparedTargetPermanentEcsInstallationOwner {
        cleanup,
        ledger,
        retired_host,
    };
    let ticket = pending_commit
        .as_ref()
        .and_then(|pending| pending.ticket.as_ref())
        .expect("the dispatch preflight retained the exact current CommitTicket");
    let outcome = engine.begin_back_target_permanent_cleanup(
        begin,
        ticket,
        installation,
        |installation, batch| {
            #[cfg(test)]
            back_target_permanent_begin_callback_calls.fetch_add(1, Ordering::Relaxed);
            let installed =
                pending_back_target_permanent_cleanup.insert(installation.commit(batch));
            observe_installed(installed);
        },
    );
    match outcome {
        BackTargetPermanentBeginOutcome::Begun => BackTargetPermanentDispatchOutcome::Dispatched,
        BackTargetPermanentBeginOutcome::ReceiverMismatch {
            owner,
            installation,
            mismatch,
        } => {
            let (settlement, lease, manifest) = owner.into_parts();
            let PreparedTargetPermanentEcsInstallationOwner {
                cleanup,
                ledger,
                retired_host,
            } = installation;
            *host_slot = Some(retired_host.restore(lease));
            BackTargetPermanentDispatchOutcome::ReceiverMismatch {
                disposition: PreparedTargetPreparationPermanentDisposition {
                    cleanup,
                    settlement,
                    ecs_manifest,
                    core_binding: BackTargetPermanentAuthorityManifest { cleanup: manifest },
                    ledger,
                },
                mismatch,
            }
        }
        BackTargetPermanentBeginOutcome::LogicalRootsMismatch {
            owner,
            installation,
        } => {
            let (settlement, lease, manifest) = owner.into_parts();
            let PreparedTargetPermanentEcsInstallationOwner {
                cleanup,
                ledger,
                retired_host,
            } = installation;
            *host_slot = Some(retired_host.restore(lease));
            BackTargetPermanentDispatchOutcome::LogicalRootsMismatch(
                PreparedTargetPreparationPermanentDisposition {
                    cleanup,
                    settlement,
                    ecs_manifest,
                    core_binding: BackTargetPermanentAuthorityManifest { cleanup: manifest },
                    ledger,
                },
            )
        }
        BackTargetPermanentBeginOutcome::Permanent {
            owner,
            installation,
            evidence,
        } => {
            let (settlement, lease, manifest) = owner.into_parts();
            let PreparedTargetPermanentEcsInstallationOwner {
                cleanup,
                ledger,
                retired_host,
            } = installation;
            *host_slot = Some(retired_host.restore(lease));
            BackTargetPermanentDispatchOutcome::Permanent {
                disposition: PreparedTargetPreparationPermanentDisposition {
                    cleanup,
                    settlement,
                    ecs_manifest,
                    core_binding: BackTargetPermanentAuthorityManifest { cleanup: manifest },
                    ledger,
                },
                evidence,
            }
        }
    }
}

fn registered_claim_has_exact_authority_and_manifest_coverage(
    world: &World,
    registry: &RouteEntityRegistry,
    claim: &ExactRegistryClaim,
    relationship_carrier: bool,
) -> bool {
    let Some(record) = registry.record(claim.entity()) else {
        return false;
    };
    let Some(actual) = ComponentManifest::from_world(world, claim.entity()) else {
        return false;
    };
    let child_of = world
        .components()
        .component_id::<bevy_ecs::hierarchy::ChildOf>();
    let children = world
        .components()
        .component_id::<bevy_ecs::hierarchy::Children>();
    let normalized = |manifest: &ComponentManifest| {
        ComponentManifest::new(
            manifest
                .component_ids()
                .iter()
                .copied()
                .filter(|component| Some(*component) != child_of)
                .filter(|component| Some(*component) != children),
        )
    };
    let required_manifest_is_covered =
        |required: &ComponentManifest, actual: &ComponentManifest| {
            required
                .component_ids()
                .iter()
                .all(|component| actual.component_ids().contains(component))
        };
    let manifest_has_required_coverage = if relationship_carrier {
        required_manifest_is_covered(&normalized(record.manifest()), &normalized(&actual))
    } else {
        required_manifest_is_covered(record.manifest(), &actual)
    };
    record.claim() == *claim
        && cleanup_claim_has_exact_authority(world, claim)
        && manifest_has_required_coverage
}

fn normalize_back_target_driver_claims(
    mut claims: Vec<ExactRegistryClaim>,
) -> Result<Vec<ExactRegistryClaim>, BackTargetDriverPhysicalIndeterminateReason> {
    claims.sort_by_key(|claim| claim.entity().to_bits());
    if claims
        .windows(2)
        .any(|pair| pair[0].entity() == pair[1].entity())
    {
        return Err(BackTargetDriverPhysicalIndeterminateReason::DuplicateActiveClaim);
    }
    Ok(claims)
}

fn back_target_driver_owner_belongs_to_frames(
    owner: &RegistryOwner,
    frames: &[RouteFrameVersion],
) -> bool {
    match owner {
        RegistryOwner::Committed(
            EntityOwner::FrameShell(frame)
            | EntityOwner::FrameContentTarget(frame)
            | EntityOwner::FrameOutput { frame, .. },
        )
        | RegistryOwner::Committed(EntityOwner::ChildSlot { parent: frame, .. })
        | RegistryOwner::Provisional(ProvisionalOwner::ChildSlot { parent: frame, .. }) => {
            frames.iter().any(|expected| expected == frame)
        }
        RegistryOwner::Committed(EntityOwner::HostTopSlot { .. })
        | RegistryOwner::Provisional(ProvisionalOwner::HostTopSlot(_)) => false,
    }
}

fn observe_back_target_driver_registry_claim(
    world: &World,
    claim: ExactRegistryClaim,
) -> BackTargetDriverObservedRegistryClaim {
    let state = if world.get_entity(claim.entity()).is_err() {
        BackTargetDriverObservedClaimState::MissingEntity
    } else if cleanup_claim_has_exact_authority(world, &claim) {
        BackTargetDriverObservedClaimState::Exact
    } else {
        BackTargetDriverObservedClaimState::AuthorityDrift
    };
    BackTargetDriverObservedRegistryClaim { claim, state }
}

fn observe_back_target_driver_physical_partition(
    world: &World,
    entry: &RouterEntry,
    task: BackTargetDriverTaskPhysicalProjection,
    physical: Option<&BackTargetDriverUnverifiedPhysicalOwner>,
) -> Result<BackTargetDriverPhysicalPartitionProjection, BackTargetDriverPhysicalIndeterminateReason>
{
    let host = entry
        .host
        .as_ref()
        .ok_or(BackTargetDriverPhysicalIndeterminateReason::MissingHost)?;
    if !validated_host_candidate_is_current(world, &host.candidate) {
        return Err(BackTargetDriverPhysicalIndeterminateReason::HostAuthorityDrift);
    }
    let registry = world
        .get_resource::<RouteEntityRegistry>()
        .ok_or(BackTargetDriverPhysicalIndeterminateReason::HostAuthorityDrift)?;
    if registry
        .record(host.top_slot)
        .is_none_or(|record| record.claim() != host.top_slot_claim)
        || !cleanup_claim_has_exact_authority(world, &host.top_slot_claim)
        || world
            .get::<bevy_ecs::hierarchy::ChildOf>(host.top_slot)
            .map(bevy_ecs::hierarchy::ChildOf::parent)
            != Some(host.host)
    {
        return Err(BackTargetDriverPhysicalIndeterminateReason::HostAuthorityDrift);
    }
    let target_frames = std::slice::from_ref(&task.frame);
    let target_claims = normalize_back_target_driver_claims(
        registry
            .records()
            .iter()
            .filter(|record| {
                back_target_driver_owner_belongs_to_frames(record.owner(), target_frames)
            })
            .map(|record| record.claim().clone())
            .collect(),
    )?;
    if target_claims.iter().any(|claim| {
        #[cfg(feature = "bsn")]
        if let Some(failure) =
            physical.and_then(BackTargetDriverUnverifiedPhysicalOwner::native_failure)
        {
            if failure.source.report == task.report
                && failure.source.frame == task.frame
                && failure.source.mount == task.mount
                && failure
                    .residual()
                    .is_some_and(|residual| residual.claim_identity_is_exact(world, claim))
            {
                return false;
            }
        }
        !cleanup_claim_has_exact_authority(world, claim)
    }) {
        return Err(BackTargetDriverPhysicalIndeterminateReason::TargetRegistryIncomplete);
    }
    let target_boundaries = registry
        .records()
        .iter()
        .filter_map(|record| {
            matches!(
                record.owner(),
                RegistryOwner::Committed(EntityOwner::FrameShell(frame))
                    if frame == &task.frame
            )
            .then_some(record.entity())
        })
        .collect::<Vec<_>>();
    if target_boundaries.len() > 1 {
        return Err(BackTargetDriverPhysicalIndeterminateReason::TargetRegistryIncomplete);
    }
    let active = entry
        .active
        .as_ref()
        .ok_or(BackTargetDriverPhysicalIndeterminateReason::MissingActive)?;
    let mut protected = validate_active_branch_local(world, active, &target_boundaries)
        .map_err(|_| BackTargetDriverPhysicalIndeterminateReason::ActiveTopologyDrift)?;
    if active.frames().len() == 1
        && active.child_slots().is_empty()
        && !validate_active_frame(world, &active.frames()[0])
    {
        return Err(BackTargetDriverPhysicalIndeterminateReason::ActiveTopologyDrift);
    }
    protected.extend([host.host, host.top_slot]);
    protected.sort_by_key(|entity| entity.to_bits());
    protected.dedup();

    let active_frames = active
        .frames()
        .iter()
        .map(|frame| frame.frame().clone())
        .collect::<Vec<_>>();
    let active_root_mount = active.frames().first().map(|frame| {
        let (physical_parent, claim) = frame.mount_binding().physical_parent();
        BackTargetDriverRootMountPhysicalProjection {
            frame: frame.frame().clone(),
            slot: frame.mount_binding().slot(),
            slot_claim: frame.mount_binding().slot_claim().clone(),
            physical_parent: match claim {
                Some(claim) => BackTargetDriverRootPhysicalParentProjection::Claimed {
                    entity: physical_parent,
                    claim: claim.clone(),
                },
                None => BackTargetDriverRootPhysicalParentProjection::Host(physical_parent),
            },
        }
    });
    let mut active_child_slots = active
        .child_slots()
        .iter()
        .map(|slot| BackTargetDriverActiveChildSlotProjection {
            parent: slot.parent().clone(),
            outlet: slot.outlet().clone(),
            slot_id: slot.slot_id(),
            entity: slot.slot(),
            claim: slot.slot_claim().clone(),
            role: active
                .frames()
                .iter()
                .find(|frame| frame.mount_binding().matches_mounted_child_slot(slot))
                .map_or(
                    BackTargetDriverActiveChildSlotRole::RetainedEmpty,
                    |frame| BackTargetDriverActiveChildSlotRole::Occupied(frame.frame().clone()),
                ),
        })
        .collect::<Vec<_>>();
    active_child_slots.sort_by_key(|slot| slot.entity.to_bits());
    let expected_active_claims = normalize_back_target_driver_claims(
        active
            .claims()
            .cloned()
            .chain(std::iter::once(host.top_slot_claim.clone()))
            .collect(),
    )?;
    let mut observed_active_claims = registry
        .records()
        .iter()
        .filter(|record| {
            back_target_driver_owner_belongs_to_frames(record.owner(), &active_frames)
                || matches!(
                    record.owner(),
                    RegistryOwner::Committed(EntityOwner::HostTopSlot {
                        host: version,
                        slot,
                    }) if version == &host.version && *slot == host.slot_id
                )
        })
        .map(|record| observe_back_target_driver_registry_claim(world, record.claim()))
        .collect::<Vec<_>>();
    observed_active_claims.sort_by_key(|observed| observed.claim.entity().to_bits());

    Ok(BackTargetDriverPhysicalPartitionProjection {
        router: entry.handle,
        task,
        target_claims,
        host: BackTargetDriverHostPhysicalProjection {
            version: host.version.clone(),
            slot_id: host.slot_id,
            host: host.host,
            top_slot: host.top_slot,
            top_slot_claim: host.top_slot_claim.clone(),
        },
        active_frames,
        active_root_mount,
        active_child_slots,
        expected_active_claims,
        observed_active_claims,
        protected,
    })
}

pub(crate) fn freeze_back_target_driver_physical_partition_source(
    world: &World,
    entry: &RouterEntry,
    driving: &PendingBackTargetDriving,
    task: &RootFrameTask,
) -> Result<
    BackTargetDriverPhysicalPartitionSource,
    FrozenBackTargetDriverPhysicalIndeterminateEvidence,
> {
    freeze_back_target_driver_physical_partition_source_with_owner(
        world, entry, driving, task, None,
    )
}

fn freeze_back_target_driver_physical_partition_source_with_owner(
    world: &World,
    entry: &RouterEntry,
    driving: &PendingBackTargetDriving,
    task: &RootFrameTask,
    physical: Option<&BackTargetDriverUnverifiedPhysicalOwner>,
) -> Result<
    BackTargetDriverPhysicalPartitionSource,
    FrozenBackTargetDriverPhysicalIndeterminateEvidence,
> {
    match entry.engine.validate_back_target_execution_pairing(
        BackTargetExecutionOwnerRef::Driving {
            history: &driving.history,
            continuation: &driving.continuation,
        },
        BackTargetPhysicalOwnerRef::FrameTask(task),
    ) {
        BackTargetExecutionPairingOutcome::Matched => {}
        BackTargetExecutionPairingOutcome::Mismatch(mismatch) => {
            return Err(FrozenBackTargetDriverPhysicalIndeterminateEvidence {
                observation: BackTargetDriverPhysicalIndeterminateObservation::PairingMismatch(
                    mismatch,
                ),
            });
        }
    }
    let task = BackTargetDriverTaskPhysicalProjection {
        report: task.report_identity(),
        request: task.request(),
        sequence: task.view_request_sequence(),
        frame: task.frame().clone(),
        mount: task.mount_target(),
    };
    let frozen = observe_back_target_driver_physical_partition(world, entry, task, physical)
        .map_err(
            |reason| FrozenBackTargetDriverPhysicalIndeterminateEvidence {
                observation: BackTargetDriverPhysicalIndeterminateObservation::Freeze(reason),
            },
        )?;
    Ok(BackTargetDriverPhysicalPartitionSource { frozen })
}

fn back_target_driver_indeterminate(
    original: BackTargetDriverUnverifiedPhysicalOwner,
    evidence: FrozenBackTargetDriverPhysicalIndeterminateEvidence,
) -> BackTargetDriverPhysicalClassification {
    BackTargetDriverPhysicalClassification::Indeterminate(
        BackTargetDriverPhysicalIndeterminateOwner { original, evidence },
    )
}

fn back_target_driver_root_lineage_is_exact(
    source: &BackTargetDriverPhysicalPartitionProjection,
) -> bool {
    let FrameMountTarget::TopSlot { host, slot } = &source.task.mount else {
        return false;
    };
    let Some(root) = source.active_root_mount.as_ref() else {
        return false;
    };
    source.active_frames.len() == 1
        && source.active_child_slots.is_empty()
        && host == &source.host.version
        && *slot == source.host.slot_id
        && source.task.frame.slot() == source.host.slot_id
        && root.frame == source.active_frames[0]
        && root.frame.slot() == source.host.slot_id
        && bevy_ui_router_core::integration::frame_versions_share_router(
            &root.frame,
            &source.task.frame,
        )
        && root.frame.host() == source.task.frame.host()
        && root.slot == source.host.top_slot
        && root.slot_claim == source.host.top_slot_claim
        && root.physical_parent
            == BackTargetDriverRootPhysicalParentProjection::Host(source.host.host)
}

fn back_target_driver_active_registry_is_complete(
    source: &BackTargetDriverPhysicalPartitionProjection,
) -> bool {
    source.expected_active_claims.len() == source.observed_active_claims.len()
        && source
            .expected_active_claims
            .iter()
            .zip(&source.observed_active_claims)
            .all(|(expected, observed)| {
                expected == &observed.claim
                    && observed.state == BackTargetDriverObservedClaimState::Exact
            })
}

fn back_target_driver_residual_reportable(
    world: &World,
    source: &BackTargetDriverPhysicalPartitionProjection,
    branch: FailedPreparedBranch,
) -> Result<
    BackTargetDriverResidualPhysicalOwner,
    (
        FailedPreparedBranch,
        BackTargetDriverPhysicalIndeterminateReason,
    ),
> {
    #[cfg(feature = "bsn")]
    if let Some(failure) = branch.native_failure() {
        let exact_source = failure.source.report == source.task.report
            && failure.source.frame == source.task.frame
            && failure.source.mount == source.task.mount;
        let claims = normalize_back_target_driver_claims(failure.claims().cloned().collect());
        let no_other_targets = branch.frames().is_empty()
            && branch.child_slots().is_empty()
            && branch.mount_residuals().is_empty()
            && branch.detached_frames().is_empty()
            && branch.rejected_child_slots().is_empty()
            && branch.mount_new_allowance().is_none();
        if !exact_source
            || !no_other_targets
            || failure.residual().is_none()
            || failure.validate_settled(world).is_err()
            || claims.as_ref().map_or(true, |claims| {
                claims != &source.target_claims
                    || claims
                        .iter()
                        .any(|claim| source.protected.contains(&claim.entity()))
            })
        {
            return Err((
                branch,
                BackTargetDriverPhysicalIndeterminateReason::ResidualTopologyIncomplete,
            ));
        }
        let manifest = world
            .get_resource::<RouteEntityRegistry>()
            .and_then(|registry| {
                FrozenCleanupManifest::from_failed_prepared_branch(world, registry, &branch).ok()
            });
        let Some(manifest) =
            manifest.filter(|manifest| manifest.len() == 0 && manifest.preflight(world))
        else {
            return Err((
                branch,
                BackTargetDriverPhysicalIndeterminateReason::ResidualTopologyIncomplete,
            ));
        };
        let target_claims = match claims {
            Ok(claims) => claims,
            Err(_) => unreachable!("native target claims were checked before moving their owner"),
        };
        return Ok(BackTargetDriverResidualPhysicalOwner {
            branch,
            proof: FrozenBackTargetDriverPhysicalPartitionProof {
                source: source.clone(),
                manifest,
                target_claims,
            },
        });
    }
    if branch.frames().len() != 1
        || !branch.child_slots().is_empty()
        || !branch.mount_residuals().is_empty()
        || !branch.detached_frames().is_empty()
        || !branch.rejected_child_slots().is_empty()
        || branch.mount_new_allowance().is_some()
        || branch.frames()[0].frame() != &source.task.frame
        || validate_prepared_branch(world, branch.realized()).is_err()
    {
        return Err((
            branch,
            BackTargetDriverPhysicalIndeterminateReason::ResidualTopologyIncomplete,
        ));
    }
    let frame = &branch.frames()[0];
    let (physical_parent, parent_claim) = frame.mount_binding().physical_parent();
    if frame.mount_binding().slot() != source.host.top_slot
        || frame.mount_binding().slot_claim() != &source.host.top_slot_claim
        || physical_parent != source.host.host
        || parent_claim.is_some()
    {
        return Err((
            branch,
            BackTargetDriverPhysicalIndeterminateReason::ResidualTopologyIncomplete,
        ));
    }
    let Some(registry) = world.get_resource::<RouteEntityRegistry>() else {
        return Err((
            branch,
            BackTargetDriverPhysicalIndeterminateReason::TargetRegistryIncomplete,
        ));
    };
    let manifest =
        match FrozenCleanupManifest::from_failed_prepared_branch(world, registry, &branch) {
            Ok(manifest) => manifest,
            Err(_) => {
                return Err((
                    branch,
                    BackTargetDriverPhysicalIndeterminateReason::ResidualTopologyIncomplete,
                ));
            }
        };
    let target_claims = match normalize_back_target_driver_claims(frame.claims().cloned().collect())
    {
        Ok(claims) => claims,
        Err(_) => {
            return Err((
                branch,
                BackTargetDriverPhysicalIndeterminateReason::ResidualTopologyIncomplete,
            ));
        }
    };
    if manifest.len() == 0
        || manifest.len() != target_claims.len()
        || !manifest.boundary_drifts().is_empty()
        || !manifest.preflight(world)
        || !failed_prepared_branch_roots_are_exact(world, &branch, &manifest)
    {
        return Err((
            branch,
            BackTargetDriverPhysicalIndeterminateReason::ResidualTopologyIncomplete,
        ));
    }
    if target_claims
        .iter()
        .any(|claim| source.protected.contains(&claim.entity()))
    {
        return Err((
            branch,
            BackTargetDriverPhysicalIndeterminateReason::TargetProtectedOverlap,
        ));
    }
    let task_frames = [source.task.frame.clone()];
    let mut observed_target_claims = registry
        .records()
        .iter()
        .filter(|record| back_target_driver_owner_belongs_to_frames(record.owner(), &task_frames))
        .map(|record| observe_back_target_driver_registry_claim(world, record.claim()))
        .collect::<Vec<_>>();
    observed_target_claims.sort_by_key(|observed| observed.claim.entity().to_bits());
    if observed_target_claims.len() != target_claims.len()
        || target_claims
            .iter()
            .zip(&observed_target_claims)
            .any(|(expected, observed)| {
                expected != &observed.claim
                    || observed.state != BackTargetDriverObservedClaimState::Exact
            })
    {
        return Err((
            branch,
            BackTargetDriverPhysicalIndeterminateReason::TargetRegistryIncomplete,
        ));
    }
    Ok(BackTargetDriverResidualPhysicalOwner {
        branch,
        proof: FrozenBackTargetDriverPhysicalPartitionProof {
            source: source.clone(),
            manifest,
            target_claims,
        },
    })
}

fn classify_back_target_driver_frozen_physical(
    world: &World,
    entry: &RouterEntry,
    frozen: &BackTargetDriverPhysicalPartitionProjection,
    owner: BackTargetDriverUnverifiedPhysicalOwner,
) -> BackTargetDriverPhysicalClassification {
    let observed = match observe_back_target_driver_physical_partition(
        world,
        entry,
        frozen.task.clone(),
        Some(&owner),
    ) {
        Ok(observed) => observed,
        Err(reason) => {
            return back_target_driver_indeterminate(
                owner,
                FrozenBackTargetDriverPhysicalIndeterminateEvidence {
                    observation: BackTargetDriverPhysicalIndeterminateObservation::Refreeze {
                        expected: frozen.clone(),
                        observed: None,
                        reason,
                    },
                },
            );
        }
    };
    if observed != *frozen {
        return back_target_driver_indeterminate(
            owner,
            FrozenBackTargetDriverPhysicalIndeterminateEvidence {
                observation: BackTargetDriverPhysicalIndeterminateObservation::Refreeze {
                    expected: frozen.clone(),
                    observed: Some(observed),
                    reason: BackTargetDriverPhysicalIndeterminateReason::SourceDrift,
                },
            },
        );
    }
    let classify_indeterminate = |original, reason| {
        back_target_driver_indeterminate(
            original,
            FrozenBackTargetDriverPhysicalIndeterminateEvidence {
                observation: BackTargetDriverPhysicalIndeterminateObservation::Classification {
                    source: frozen.clone(),
                    reason,
                },
            },
        )
    };
    if !back_target_driver_root_lineage_is_exact(frozen) {
        return classify_indeterminate(
            owner,
            BackTargetDriverPhysicalIndeterminateReason::UnsupportedRootLineage,
        );
    }
    if !back_target_driver_active_registry_is_complete(frozen) {
        return classify_indeterminate(
            owner,
            BackTargetDriverPhysicalIndeterminateReason::ActiveRegistryIncomplete,
        );
    }
    match owner {
        BackTargetDriverUnverifiedPhysicalOwner::Pending(pending)
            if pending.ticket.is_none()
                && pending.final_task.is_none()
                && pending.branch.frames().is_empty()
                && pending.branch.child_slots().is_empty() =>
        {
            if !frozen.target_claims.is_empty() {
                return classify_indeterminate(
                    BackTargetDriverUnverifiedPhysicalOwner::Pending(pending),
                    BackTargetDriverPhysicalIndeterminateReason::UnclassifiedTargetClaims,
                );
            }
            BackTargetDriverPhysicalClassification::Reportable(
                BackTargetDriverReportablePhysicalOwner::ProvenZero(
                    BackTargetDriverZeroPhysicalOwner {
                        physical: BackTargetDriverUnverifiedPhysicalOwner::Pending(pending),
                        proof: FrozenBackTargetDriverZeroPhysicalProof {
                            source: frozen.clone(),
                        },
                    },
                ),
            )
        }
        BackTargetDriverUnverifiedPhysicalOwner::Pending(pending) => classify_indeterminate(
            BackTargetDriverUnverifiedPhysicalOwner::Pending(pending),
            BackTargetDriverPhysicalIndeterminateReason::PendingPhysicalResidual,
        ),
        BackTargetDriverUnverifiedPhysicalOwner::Residual(branch) => {
            #[cfg(feature = "bsn")]
            if let Some(failure) = branch.native_failure()
                && failure.residual().is_none()
                && failure.source.report == frozen.task.report
                && failure.source.frame == frozen.task.frame
                && failure.source.mount == frozen.task.mount
                && branch.frames().is_empty()
                && branch.child_slots().is_empty()
                && branch.mount_residuals().is_empty()
                && branch.detached_frames().is_empty()
                && branch.rejected_child_slots().is_empty()
                && branch.mount_new_allowance().is_none()
                && frozen.target_claims.is_empty()
            {
                return BackTargetDriverPhysicalClassification::Reportable(
                    BackTargetDriverReportablePhysicalOwner::ProvenZero(
                        BackTargetDriverZeroPhysicalOwner {
                            physical: BackTargetDriverUnverifiedPhysicalOwner::Residual(branch),
                            proof: FrozenBackTargetDriverZeroPhysicalProof {
                                source: frozen.clone(),
                            },
                        },
                    ),
                );
            }
            match back_target_driver_residual_reportable(world, frozen, branch) {
                Ok(owner) => BackTargetDriverPhysicalClassification::Reportable(
                    BackTargetDriverReportablePhysicalOwner::Residual(owner),
                ),
                Err((branch, reason)) => classify_indeterminate(
                    BackTargetDriverUnverifiedPhysicalOwner::Residual(branch),
                    reason,
                ),
            }
        }
    }
}

pub(crate) fn classify_back_target_driver_physical(
    world: &World,
    entry: &RouterEntry,
    source: &BackTargetDriverPhysicalPartitionSource,
    owner: BackTargetDriverUnverifiedPhysicalOwner,
) -> BackTargetDriverPhysicalClassification {
    classify_back_target_driver_frozen_physical(world, entry, &source.frozen, owner)
}

fn reclassify_back_target_driver_post_report(
    world: &World,
    entry: &RouterEntry,
    source: &BackTargetDriverPostReportRefreezeSource,
    owner: BackTargetDriverUnverifiedPhysicalOwner,
) -> BackTargetDriverPhysicalClassification {
    classify_back_target_driver_frozen_physical(world, entry, &source.frozen, owner)
}

fn store_back_target_driver_failure(
    entry: &mut RouterEntry,
    owner: PendingBackTargetDriverFailure,
) {
    entry.pending_back_target_execution = Some(PendingBackTargetExecution::ResumeBlocked(
        PendingBackTargetResumeBlocked::DriverFailure(owner),
    ));
}

#[cfg(test)]
fn inject_back_target_driver_staging_drift(entry: &mut RouterEntry) {
    if !entry.back_target_driver_staging_drift_armed {
        return;
    }
    let Some(stage) = entry.pending_back_target_execution.take() else {
        return;
    };
    let PendingBackTargetExecution::Driving(owner) = stage else {
        entry.pending_back_target_execution = Some(stage);
        return;
    };
    entry.back_target_driver_staging_drift_armed = false;
    entry.pending_back_target_execution =
        Some(PendingBackTargetExecution::DriverFailureStagingDrift(owner));
}

fn finish_back_target_mount_failure(entry: &mut RouterEntry) {
    if !matches!(
        entry.pending_back_target_execution,
        Some(PendingBackTargetExecution::Driving(_))
    ) || entry.pending_failed_branch.is_none()
    {
        return;
    }
    let failed = entry.pending_failed_branch.take().unwrap();
    let PendingFailedBranch { terminal, branch } = failed;
    let request = match terminal {
        PendingFailedBranchTerminal::Request(request) => request,
        PendingFailedBranchTerminal::RegistrationBegin(error) => {
            match error.reject(&mut entry.engine) {
                Ok(request) => request,
                Err(error) => {
                    entry.pending_failed_branch = Some(PendingFailedBranch {
                        terminal: PendingFailedBranchTerminal::RegistrationBegin(error),
                        branch,
                    });
                    return;
                }
            }
        }
        terminal => {
            entry.pending_failed_branch = Some(PendingFailedBranch { terminal, branch });
            return;
        }
    };
    let Some(PendingBackTargetExecution::Driving(driving)) =
        entry.pending_back_target_execution.take()
    else {
        unreachable!("target mount failure retains its original driving owner")
    };
    let PendingBackTargetDriving {
        history,
        continuation,
    } = driving;
    let request = match entry
        .engine
        .finish_back_target_mount_failure(continuation, request)
    {
        r3_core::FinishBackTargetMountFailureOutcome::Matched(request) => {
            drop(history);
            request
        }
        r3_core::FinishBackTargetMountFailureOutcome::Mismatch {
            capacity,
            request,
            mismatch,
        } => {
            entry.pending_back_target_execution = Some(PendingBackTargetExecution::ResumeBlocked(
                PendingBackTargetResumeBlocked::MountFailureMismatch {
                    driving: PendingBackTargetDriving {
                        history,
                        continuation: capacity,
                    },
                    mismatch,
                },
            ));
            request
        }
    };
    entry.pending_failed_branch = Some(PendingFailedBranch {
        terminal: PendingFailedBranchTerminal::Request(request),
        branch,
    });
}

fn consume_back_target_cleanup_only(
    entry: &mut RouterEntry,
    history: HistoryExecutionBindingView,
    branch: FailedPreparedBranch,
    outcome: BackTargetDriverFailureCoreOutcome,
) {
    // PhysicalResidual and applied mount failures always require cleanup. The complete
    // branch (including slots/native residuals) is proved by the existing cleanup path.
    assert!(entry.pending_failed_branch.is_none());
    match entry.engine.consume_back_target_driver_failure(outcome) {
        BackTargetDriverFailureConsumeOutcome::FailureCleanup(request) => {
            drop(history);
            entry.pending_failed_branch = Some(PendingFailedBranch {
                terminal: PendingFailedBranchTerminal::Request(request),
                branch,
            });
        }
        BackTargetDriverFailureConsumeOutcome::Mismatch {
            owner: outcome,
            mismatch,
        } => {
            entry.pending_back_target_execution = Some(PendingBackTargetExecution::ResumeBlocked(
                PendingBackTargetResumeBlocked::CleanupOnlyMismatch {
                    history,
                    branch,
                    outcome,
                    mismatch,
                },
            ));
        }
        BackTargetDriverFailureConsumeOutcome::FallbackReleased
        | BackTargetDriverFailureConsumeOutcome::Terminal(_) => {
            unreachable!("cleanup-only failure cannot publish fallback or terminal before cleanup")
        }
    }
}

fn consume_back_target_driver_reported(
    entry: &mut RouterEntry,
    reported: BackTargetDriverFailureReported,
) {
    let BackTargetDriverFailureReported {
        source,
        history,
        physical,
        outcome,
    } = reported;

    if matches!(
        (&physical, &outcome),
        (
            BackTargetDriverReportablePhysicalOwner::Residual(_),
            BackTargetDriverFailureCoreOutcome::Fallback(_)
                | BackTargetDriverFailureCoreOutcome::Terminal(_)
        )
    ) {
        store_back_target_driver_failure(
            entry,
            PendingBackTargetDriverFailure::PhysicalMismatch(
                PendingBackTargetDriverPhysicalMismatch {
                    source,
                    history,
                    physical: physical.into_unverified(),
                    outcome,
                    evidence: BackTargetDriverPhysicalMismatchEvidence { _private: () },
                },
            ),
        );
        return;
    }

    #[cfg(test)]
    if entry.back_target_driver_consume_receiver_drift_armed {
        entry.back_target_driver_consume_receiver_drift_armed = false;
        assert!(
            entry.engine.testing_drift_current_frame_task_identity(),
            "the consume receiver drift seam requires one current fallback Applying FrameTask"
        );
    }
    match entry.engine.consume_back_target_driver_failure(outcome) {
        BackTargetDriverFailureConsumeOutcome::Mismatch {
            owner: outcome,
            mismatch,
        } => {
            store_back_target_driver_failure(
                entry,
                PendingBackTargetDriverFailure::ConsumeMismatch(
                    PendingBackTargetDriverConsumeMismatch {
                        source,
                        history,
                        physical: physical.into_unverified(),
                        outcome,
                        mismatch,
                    },
                ),
            );
        }
        BackTargetDriverFailureConsumeOutcome::FailureCleanup(request) => match physical {
            BackTargetDriverReportablePhysicalOwner::ProvenZero(owner) => {
                drop(source);
                drop(history);
                drop(owner.proof);
                entry.pending_failed_branch = Some(PendingFailedBranch {
                    terminal: PendingFailedBranchTerminal::Request(request),
                    branch: match owner.physical {
                        BackTargetDriverUnverifiedPhysicalOwner::Pending(pending) => {
                            failed_prepared_branch(pending, None)
                        }
                        BackTargetDriverUnverifiedPhysicalOwner::Residual(branch) => branch,
                    },
                });
            }
            BackTargetDriverReportablePhysicalOwner::Residual(owner) => {
                drop(source);
                drop(history);
                drop(owner.proof);
                entry.pending_failed_branch = Some(PendingFailedBranch {
                    terminal: PendingFailedBranchTerminal::Request(request),
                    branch: owner.branch,
                });
            }
        },
        BackTargetDriverFailureConsumeOutcome::FallbackReleased => {
            let BackTargetDriverReportablePhysicalOwner::ProvenZero(owner) = physical else {
                unreachable!("a residual driver owner is fail-closed before fallback consumption");
            };
            drop(source);
            drop(history);
            drop(owner);
        }
        BackTargetDriverFailureConsumeOutcome::Terminal(evidence) => {
            let BackTargetDriverReportablePhysicalOwner::ProvenZero(owner) = physical else {
                unreachable!("a residual driver owner is fail-closed before terminal consumption");
            };
            drop(source);
            drop(history);
            drop(owner);
            drop(evidence);
        }
    }
}

pub(crate) fn drive_back_target_driver_failure(world: &mut World, entry: &mut RouterEntry) {
    let Some(stage) = entry.pending_back_target_execution.take() else {
        return;
    };
    let PendingBackTargetExecution::ResumeBlocked(PendingBackTargetResumeBlocked::DriverFailure(
        owner,
    )) = stage
    else {
        entry.pending_back_target_execution = Some(stage);
        return;
    };
    #[cfg(test)]
    entry
        .back_target_driver_failure_dispatch_calls
        .fetch_add(1, Ordering::Relaxed);
    let PendingBackTargetDriverFailure::AwaitingReport(owner) = owner else {
        store_back_target_driver_failure(entry, owner);
        return;
    };
    let PendingBackTargetDriverAwaitingReport {
        driving,
        task,
        source,
        physical,
        kind,
        message,
    } = owner;
    let physical = match classify_back_target_driver_physical(world, entry, &source, physical) {
        BackTargetDriverPhysicalClassification::Reportable(physical) => physical,
        BackTargetDriverPhysicalClassification::Indeterminate(physical) => {
            store_back_target_driver_failure(
                entry,
                PendingBackTargetDriverFailure::PhysicalIndeterminate(
                    PendingBackTargetDriverPhysicalIndeterminate {
                        driving,
                        task,
                        physical,
                        kind,
                        message,
                    },
                ),
            );
            return;
        }
    };

    let report_mode = physical.report_mode();
    let post_source = source.seal_for_report();
    let PendingBackTargetDriving {
        history,
        continuation,
    } = driving;
    #[cfg(test)]
    if entry.back_target_driver_report_receiver_drift_armed {
        entry.back_target_driver_report_receiver_drift_armed = false;
        assert!(
            entry.engine.testing_drift_current_frame_task_identity(),
            "the report receiver drift seam requires one current Applying FrameTask"
        );
    }
    let outcome = match entry.engine.report_back_target_failed(
        &history,
        continuation,
        task,
        kind,
        &message,
        report_mode,
    ) {
        BackTargetDriverFailureReportOutcome::Reported(outcome) => outcome,
        BackTargetDriverFailureReportOutcome::Mismatch {
            capacity,
            task,
            mismatch,
        } => {
            drop(post_source);
            store_back_target_driver_failure(
                entry,
                PendingBackTargetDriverFailure::ReceiverMismatch(
                    PendingBackTargetDriverReceiverMismatch {
                        driving: PendingBackTargetDriving {
                            history,
                            continuation: capacity,
                        },
                        task,
                        physical: physical.into_unverified(),
                        kind,
                        message,
                        mismatch,
                    },
                ),
            );
            return;
        }
    };
    consume_back_target_driver_reported(
        entry,
        BackTargetDriverFailureReported {
            source: post_source,
            history,
            physical,
            outcome,
        },
    );
}

enum RouteBackTargetAppliedOutcome {
    Handled,
    NonTarget {
        pending: PendingCommit,
        task: RootFrameTask,
        report: BackTargetAppliedReport,
    },
}

fn store_back_target_applied_staging_mismatch(
    entry: &mut RouterEntry,
    previous: Option<BackTargetDriverStagingOwner>,
    observed: BackTargetDriverStagingMismatchObservation,
    pending: PendingCommit,
    task: RootFrameTask,
    report: BackTargetAppliedReport,
) -> RouteBackTargetAppliedOutcome {
    entry.pending_back_target_execution =
        Some(PendingBackTargetExecution::AppliedReportStagingMismatch(
            PendingBackTargetAppliedStagingMismatch {
                previous,
                pending,
                task,
                report,
                evidence: BackTargetDriverStagingMismatchEvidence {
                    observed,
                    pairing: None,
                },
            },
        ));
    RouteBackTargetAppliedOutcome::Handled
}

fn route_back_target_applied_report(
    entry: &mut RouterEntry,
    pending: PendingCommit,
    task: RootFrameTask,
    report: BackTargetAppliedReport,
) -> RouteBackTargetAppliedOutcome {
    if !entry.engine.is_current_root_back_frame_task(&task) {
        return RouteBackTargetAppliedOutcome::NonTarget {
            pending,
            task,
            report,
        };
    }
    let Some(stage) = entry.pending_back_target_execution.take() else {
        return store_back_target_applied_staging_mismatch(
            entry,
            None,
            BackTargetDriverStagingMismatchObservation::Missing,
            pending,
            task,
            report,
        );
    };
    let stage = match stage {
        PendingBackTargetExecution::AppliedReportStagingMismatch(existing) => {
            entry.pending_back_target_execution =
                Some(PendingBackTargetExecution::AppliedReportStagingConflict(
                    PendingBackTargetAppliedStagingConflict {
                        existing,
                        incoming: PendingBackTargetAppliedReportAttempt {
                            pending,
                            task,
                            report,
                        },
                        evidence: BackTargetDriverStagingMismatchEvidence {
                            observed:
                                BackTargetDriverStagingMismatchObservation::ExistingAppliedMismatch,
                            pairing: None,
                        },
                    },
                ));
            return RouteBackTargetAppliedOutcome::Handled;
        }
        PendingBackTargetExecution::AppliedReportStagingConflict(_) => {
            unreachable!(
                "a flat applied staging conflict already owns the unique current root task"
            )
        }
        stage => stage,
    };
    let PendingBackTargetExecution::Driving(driving) = stage else {
        let (previous, observed) = back_target_driver_non_driving_owner(stage);
        return store_back_target_applied_staging_mismatch(
            entry,
            Some(previous),
            observed,
            pending,
            task,
            report,
        );
    };
    // Applying a report only advances Core's exact request and returns its next
    // owner. Mount, publication and failure cleanup validate the complete branch
    // in their respective execution paths; a root-only cleanup proof does not
    // authorize this transition and would reject valid nested entering frames.
    let PendingBackTargetDriving {
        history,
        continuation,
    } = driving;
    #[cfg(test)]
    entry
        .back_target_atomic_applied_dispatch_calls
        .fetch_add(1, Ordering::Relaxed);
    match entry
        .engine
        .report_back_target_applied(&history, continuation, task, report)
    {
        BackTargetAppliedReportOutcome::MountRequest { capacity, request } => {
            let driving = PendingBackTargetDriving {
                history,
                continuation: capacity,
            };
            match entry.engine.validate_back_target_execution_pairing(
                BackTargetExecutionOwnerRef::Driving {
                    history: &driving.history,
                    continuation: &driving.continuation,
                },
                BackTargetPhysicalOwnerRef::MountRequest(&request),
            ) {
                BackTargetExecutionPairingOutcome::Matched => {
                    entry.pending_back_target_execution =
                        Some(PendingBackTargetExecution::Driving(driving));
                    entry.pending_mount_request = Some(request);
                    entry.pending_commit = Some(pending);
                }
                BackTargetExecutionPairingOutcome::Mismatch(mismatch) => {
                    entry.pending_back_target_execution =
                        Some(PendingBackTargetExecution::ResumeBlocked(
                            PendingBackTargetResumeBlocked::AppliedMountRequestPairingMismatch(
                                PendingBackTargetAppliedMountRequestPairingMismatch {
                                    driving,
                                    request,
                                    pending,
                                    mismatch,
                                },
                            ),
                        ));
                }
            }
        }
        BackTargetAppliedReportOutcome::Commit { capacity, ticket } => {
            let owner = PendingBackTargetAppliedCommit {
                driving: PendingBackTargetDriving {
                    history,
                    continuation: capacity,
                },
                ticket,
                pending,
            };
            match entry.engine.validate_back_target_execution_pairing(
                BackTargetExecutionOwnerRef::Driving {
                    history: &owner.driving.history,
                    continuation: &owner.driving.continuation,
                },
                BackTargetPhysicalOwnerRef::CommitTicket(&owner.ticket),
            ) {
                BackTargetExecutionPairingOutcome::Matched => {
                    entry.pending_back_target_execution =
                        Some(PendingBackTargetExecution::ResumeBlocked(
                            PendingBackTargetResumeBlocked::AppliedCommitPending(owner),
                        ));
                }
                BackTargetExecutionPairingOutcome::Mismatch(mismatch) => {
                    entry.pending_back_target_execution =
                        Some(PendingBackTargetExecution::ResumeBlocked(
                            PendingBackTargetResumeBlocked::AppliedCommitPairingMismatch(
                                PendingBackTargetAppliedCommitPairingMismatch { owner, mismatch },
                            ),
                        ));
                }
            }
        }
        BackTargetAppliedReportOutcome::FailureCleanup(owner) => {
            consume_back_target_cleanup_only(
                entry,
                history,
                failed_prepared_branch(pending, None),
                BackTargetDriverFailureCoreOutcome::FailureCleanup(owner),
            );
        }
        BackTargetAppliedReportOutcome::Mismatch {
            capacity,
            task,
            report,
            mismatch,
        } => {
            entry.pending_back_target_execution = Some(PendingBackTargetExecution::ResumeBlocked(
                PendingBackTargetResumeBlocked::AppliedReportMismatch(
                    PendingBackTargetAppliedReportMismatch {
                        owner: PendingBackTargetAppliedReportInput {
                            driving: PendingBackTargetDriving {
                                history,
                                continuation: capacity,
                            },
                            task,
                            pending,
                            report,
                        },
                        mismatch,
                    },
                ),
            ));
        }
    }
    RouteBackTargetAppliedOutcome::Handled
}

enum RouteBackTargetFrameFailureOutcome {
    Handled,
    NonTarget {
        physical: BackTargetDriverUnverifiedPhysicalOwner,
        task: RootFrameTask,
    },
}

fn back_target_driver_staging_mismatch(
    previous: Option<BackTargetDriverStagingOwner>,
    observed: BackTargetDriverStagingMismatchObservation,
    pairing: Option<PreparationReceiverMismatchView>,
    physical: BackTargetDriverUnverifiedPhysicalOwner,
    task: RootFrameTask,
    kind: ViewFailureKind,
    message: impl Into<String>,
) -> PendingBackTargetDriverStagingMismatch {
    let message = message.into();
    PendingBackTargetDriverStagingMismatch {
        previous,
        physical,
        task,
        kind,
        message,
        evidence: BackTargetDriverStagingMismatchEvidence { observed, pairing },
    }
}

fn store_back_target_driver_staging_mismatch(
    entry: &mut RouterEntry,
    owner: PendingBackTargetDriverStagingMismatch,
) -> RouteBackTargetFrameFailureOutcome {
    entry.pending_back_target_execution = Some(
        PendingBackTargetExecution::DriverFailureStagingMismatch(owner),
    );
    RouteBackTargetFrameFailureOutcome::Handled
}

fn back_target_driver_non_driving_owner(
    stage: PendingBackTargetExecution,
) -> (
    BackTargetDriverStagingOwner,
    BackTargetDriverStagingMismatchObservation,
) {
    match stage {
        PendingBackTargetExecution::ResumeBlocked(owner) => (
            BackTargetDriverStagingOwner::ResumeBlocked(owner),
            BackTargetDriverStagingMismatchObservation::ResumeBlocked,
        ),
        PendingBackTargetExecution::Classifying(owner) => (
            BackTargetDriverStagingOwner::Classifying(owner),
            BackTargetDriverStagingMismatchObservation::Classifying,
        ),
        PendingBackTargetExecution::Prepared(owner) => (
            BackTargetDriverStagingOwner::Prepared(owner),
            BackTargetDriverStagingMismatchObservation::Prepared,
        ),
        PendingBackTargetExecution::CommitFailure(owner) => (
            BackTargetDriverStagingOwner::CommitFailure(owner),
            BackTargetDriverStagingMismatchObservation::CommitFailure,
        ),
        PendingBackTargetExecution::DriverFailureStagingMismatch(owner) => (
            BackTargetDriverStagingOwner::ExistingMismatch(Box::new(owner)),
            BackTargetDriverStagingMismatchObservation::ExistingMismatch,
        ),
        PendingBackTargetExecution::AppliedReportStagingMismatch(_) => {
            unreachable!("an applied staging mismatch is safe-stopped before generic wrapping")
        }
        PendingBackTargetExecution::AppliedReportStagingConflict(_) => {
            unreachable!("an applied staging conflict is safe-stopped before generic wrapping")
        }
        #[cfg(test)]
        PendingBackTargetExecution::DriverFailureStagingDrift(owner) => (
            BackTargetDriverStagingOwner::TestDrift(owner),
            BackTargetDriverStagingMismatchObservation::TestDrift,
        ),
        PendingBackTargetExecution::Driving(_) => {
            unreachable!("Driving is separated before staging mismatch ownership")
        }
    }
}

fn route_back_target_frame_failure(
    world: &mut World,
    entry: &mut RouterEntry,
    physical: BackTargetDriverUnverifiedPhysicalOwner,
    task: RootFrameTask,
    kind: ViewFailureKind,
    message: impl Into<String>,
) -> RouteBackTargetFrameFailureOutcome {
    let message = message.into();
    let current_root_back_task = entry.engine.is_current_root_back_frame_task(&task);
    let Some(stage) = entry.pending_back_target_execution.take() else {
        return if current_root_back_task {
            let owner = back_target_driver_staging_mismatch(
                None,
                BackTargetDriverStagingMismatchObservation::Missing,
                None,
                physical,
                task,
                kind,
                message,
            );
            store_back_target_driver_staging_mismatch(entry, owner)
        } else {
            RouteBackTargetFrameFailureOutcome::NonTarget { physical, task }
        };
    };
    let PendingBackTargetExecution::Driving(driving) = stage else {
        let (previous, observed) = back_target_driver_non_driving_owner(stage);
        let owner = back_target_driver_staging_mismatch(
            Some(previous),
            observed,
            None,
            physical,
            task,
            kind,
            message,
        );
        return store_back_target_driver_staging_mismatch(entry, owner);
    };
    match entry.engine.validate_back_target_execution_pairing(
        BackTargetExecutionOwnerRef::Driving {
            history: &driving.history,
            continuation: &driving.continuation,
        },
        BackTargetPhysicalOwnerRef::FrameTask(&task),
    ) {
        BackTargetExecutionPairingOutcome::Mismatch(pairing) => {
            let owner = back_target_driver_staging_mismatch(
                Some(BackTargetDriverStagingOwner::Driving(driving)),
                BackTargetDriverStagingMismatchObservation::PairingMismatch,
                Some(pairing),
                physical,
                task,
                kind,
                message,
            );
            return store_back_target_driver_staging_mismatch(entry, owner);
        }
        BackTargetExecutionPairingOutcome::Matched
            if matches!(task.mount_target(), FrameMountTarget::TopSlot { .. })
                && !current_root_back_task =>
        {
            let owner = back_target_driver_staging_mismatch(
                Some(BackTargetDriverStagingOwner::Driving(driving)),
                BackTargetDriverStagingMismatchObservation::NonCurrentRoot,
                None,
                physical,
                task,
                kind,
                message,
            );
            return store_back_target_driver_staging_mismatch(entry, owner);
        }
        BackTargetExecutionPairingOutcome::Matched => {}
    }

    let has_nested_topology = !matches!(task.mount_target(), FrameMountTarget::TopSlot { .. })
        || entry
            .active
            .as_ref()
            .is_some_and(|active| active.frames().len() != 1 || !active.child_slots().is_empty());
    if has_nested_topology {
        let PendingBackTargetDriving {
            history,
            continuation,
        } = driving;
        match entry.engine.report_back_target_failed(
            &history,
            continuation,
            task,
            kind,
            message.clone(),
            BackTargetDriverFailureReportMode::PhysicalResidual,
        ) {
            BackTargetDriverFailureReportOutcome::Reported(outcome) => {
                let branch = match physical {
                    BackTargetDriverUnverifiedPhysicalOwner::Pending(pending) => {
                        failed_prepared_branch(pending, None)
                    }
                    BackTargetDriverUnverifiedPhysicalOwner::Residual(branch) => branch,
                };
                consume_back_target_cleanup_only(entry, history, branch, outcome);
            }
            BackTargetDriverFailureReportOutcome::Mismatch {
                capacity,
                task,
                mismatch,
            } => {
                let owner = back_target_driver_staging_mismatch(
                    Some(BackTargetDriverStagingOwner::Driving(
                        PendingBackTargetDriving {
                            history,
                            continuation: capacity,
                        },
                    )),
                    BackTargetDriverStagingMismatchObservation::PairingMismatch,
                    Some(mismatch),
                    physical,
                    task,
                    kind,
                    message,
                );
                return store_back_target_driver_staging_mismatch(entry, owner);
            }
        }
        return RouteBackTargetFrameFailureOutcome::Handled;
    }

    let source = match freeze_back_target_driver_physical_partition_source_with_owner(
        world,
        entry,
        &driving,
        &task,
        Some(&physical),
    ) {
        Ok(source) => source,
        Err(evidence) => {
            entry.pending_back_target_execution = Some(PendingBackTargetExecution::ResumeBlocked(
                PendingBackTargetResumeBlocked::DriverFailure(
                    PendingBackTargetDriverFailure::PhysicalIndeterminate(
                        PendingBackTargetDriverPhysicalIndeterminate {
                            driving,
                            task,
                            physical: BackTargetDriverPhysicalIndeterminateOwner {
                                original: physical,
                                evidence,
                            },
                            kind,
                            message,
                        },
                    ),
                ),
            ));
            drive_back_target_driver_failure(world, entry);
            return RouteBackTargetFrameFailureOutcome::Handled;
        }
    };
    let failure = match classify_back_target_driver_physical(world, entry, &source, physical) {
        BackTargetDriverPhysicalClassification::Reportable(physical) => {
            PendingBackTargetDriverFailure::AwaitingReport(PendingBackTargetDriverAwaitingReport {
                driving,
                task,
                source,
                physical: physical.into_unverified(),
                kind,
                message,
            })
        }
        BackTargetDriverPhysicalClassification::Indeterminate(physical) => {
            PendingBackTargetDriverFailure::PhysicalIndeterminate(
                PendingBackTargetDriverPhysicalIndeterminate {
                    driving,
                    task,
                    physical,
                    kind,
                    message,
                },
            )
        }
    };
    entry.pending_back_target_execution = Some(PendingBackTargetExecution::ResumeBlocked(
        PendingBackTargetResumeBlocked::DriverFailure(failure),
    ));
    drive_back_target_driver_failure(world, entry);
    RouteBackTargetFrameFailureOutcome::Handled
}

fn store_back_target_applied_commit_pairing_mismatch(
    entry: &mut RouterEntry,
    owner: PendingBackTargetAppliedCommit,
    mismatch: PreparationReceiverMismatchView,
) {
    entry.pending_back_target_execution = Some(PendingBackTargetExecution::ResumeBlocked(
        PendingBackTargetResumeBlocked::AppliedCommitPairingMismatch(
            PendingBackTargetAppliedCommitPairingMismatch { owner, mismatch },
        ),
    ));
}

fn store_back_target_applied_commit_permanent(
    entry: &mut RouterEntry,
    owner: PendingBackTargetAppliedCommit,
    evidence: LifecyclePreflightEvidence,
) {
    entry.pending_back_target_execution = Some(PendingBackTargetExecution::ResumeBlocked(
        PendingBackTargetResumeBlocked::AppliedCommitPermanent(
            PendingBackTargetAppliedCommitPermanent { owner, evidence },
        ),
    ));
}

#[inline(never)]
fn drive_back_target_applied_classification(world: &mut World, entry: &mut RouterEntry) {
    if !matches!(
        entry.pending_back_target_execution.as_ref(),
        Some(PendingBackTargetExecution::Classifying(
            PendingBackTargetClassification::Active(_)
        ))
    ) {
        return;
    }
    let Some(PendingBackTargetExecution::Classifying(PendingBackTargetClassification::Active(
        owner,
    ))) = entry.pending_back_target_execution.take()
    else {
        unreachable!("the observed active classification owner remains whole");
    };
    match classify_back_target(world, entry, owner) {
        #[cfg(feature = "bsn")]
        ReconciliationPreparationOutcome::Matched(
            ReconciliationPreparationDecision::NativeCleanupRetry { owner, evidence: _ },
        ) => {
            entry.pending_back_target_execution = Some(PendingBackTargetExecution::Classifying(
                PendingBackTargetClassification::Active(owner),
            ));
        }
        ReconciliationPreparationOutcome::Matched(ReconciliationPreparationDecision::Ready(
            owner,
        )) => {
            entry.pending_back_target_execution = Some(PendingBackTargetExecution::Classifying(
                PendingBackTargetClassification::ReadyAwaitingAcceptance(owner),
            ));
        }
        ReconciliationPreparationOutcome::Matched(
            ReconciliationPreparationDecision::PendingRejected { owner, evidence },
        ) => {
            entry.pending_back_target_execution = Some(PendingBackTargetExecution::Classifying(
                PendingBackTargetClassification::PendingRejectedUnaccepted { owner, evidence },
            ));
        }
        ReconciliationPreparationOutcome::Matched(
            ReconciliationPreparationDecision::AuthorityLost { owner, evidence },
        ) => {
            let classification =
                if evidence.is_missing_resource() || evidence.is_host_binding_authority_loss() {
                    PendingBackTargetClassification::AuthorityLostUnaccepted { owner, evidence }
                } else {
                    accept_classified_back_target_authority_lost(world, entry, owner, evidence)
                };
            entry.pending_back_target_execution =
                Some(PendingBackTargetExecution::Classifying(classification));
        }
        ReconciliationPreparationOutcome::Matched(
            ReconciliationPreparationDecision::Permanent(owner),
        ) => {
            entry.pending_back_target_execution = Some(PendingBackTargetExecution::Classifying(
                PendingBackTargetClassification::Permanent(owner),
            ));
        }
        ReconciliationPreparationOutcome::PairingMismatch { owner, mismatch } => {
            entry.pending_back_target_execution = Some(PendingBackTargetExecution::Classifying(
                PendingBackTargetClassification::PairingMismatch { owner, mismatch },
            ));
        }
        ReconciliationPreparationOutcome::EntryMismatch { owner, evidence } => {
            entry.pending_back_target_execution = Some(PendingBackTargetExecution::Classifying(
                PendingBackTargetClassification::EntryMismatch { owner, evidence },
            ));
        }
    }
}

#[inline(never)]
fn advance_back_target_applied_commit(_world: &mut World, entry: &mut RouterEntry) -> bool {
    if !matches!(
        entry.pending_back_target_execution.as_ref(),
        Some(PendingBackTargetExecution::ResumeBlocked(
            PendingBackTargetResumeBlocked::AppliedCommitPending(_)
        ))
    ) {
        return false;
    }
    let Some(PendingBackTargetExecution::ResumeBlocked(
        PendingBackTargetResumeBlocked::AppliedCommitPending(mut owner),
    )) = entry.pending_back_target_execution.take()
    else {
        unreachable!("the observed target applied Commit owner remains whole");
    };
    match entry.engine.validate_back_target_execution_pairing(
        BackTargetExecutionOwnerRef::Driving {
            history: &owner.driving.history,
            continuation: &owner.driving.continuation,
        },
        BackTargetPhysicalOwnerRef::CommitTicket(&owner.ticket),
    ) {
        BackTargetExecutionPairingOutcome::Matched => {}
        BackTargetExecutionPairingOutcome::Mismatch(mismatch) => {
            store_back_target_applied_commit_pairing_mismatch(entry, owner, mismatch);
            return true;
        }
    }

    let authority_root = entry
        .active
        .as_ref()
        .and_then(RealizedBranch::first_frame)
        .or_else(|| owner.pending.branch.first_frame());
    let Some(authority) = authority_root.and_then(|root| {
        ReconciliationAuthorityIdentity::from_frame(
            root.frame(),
            root.frame().host(),
            root.frame().slot(),
        )
    }) else {
        store_back_target_applied_commit_permanent(
            entry,
            owner,
            LifecyclePreflightEvidence::new(
                LifecyclePreflightSource::TargetPartition,
                LifecyclePreflightExpectation::ExactAuthority,
                LifecyclePreflightObserved::Missing,
            ),
        );
        return true;
    };
    let Ok(projection) = entry
        .engine
        .reconciliation_projection_for_commit(&owner.ticket)
    else {
        store_back_target_applied_commit_permanent(
            entry,
            owner,
            LifecyclePreflightEvidence::new(
                LifecyclePreflightSource::TargetPartition,
                LifecyclePreflightExpectation::ExactAuthority,
                LifecyclePreflightObserved::Drifted,
            ),
        );
        return true;
    };
    let Some(facts) = ReconciliationProjectionFacts::from_core(&projection, authority) else {
        store_back_target_applied_commit_permanent(
            entry,
            owner,
            LifecyclePreflightEvidence::new(
                LifecyclePreflightSource::TargetPartition,
                LifecyclePreflightExpectation::ExactRelationshipClosure,
                LifecyclePreflightObserved::Contradictory,
            ),
        );
        return true;
    };

    let had_active = entry.active.is_some();
    let active = entry.active.take().unwrap_or_else(RealizedBranch::empty);
    let sealed = match SealedExistingReconciliation::prepare_from_projection(active, facts) {
        Ok(sealed) => sealed,
        Err(failure) => {
            let (active, _) = failure.into_parts();
            entry.active = had_active.then_some(active);
            store_back_target_applied_commit_permanent(
                entry,
                owner,
                LifecyclePreflightEvidence::new(
                    LifecyclePreflightSource::PreservedCurrent,
                    LifecyclePreflightExpectation::ExactAuthority,
                    LifecyclePreflightObserved::Drifted,
                ),
            );
            return true;
        }
    };
    let entering = std::mem::replace(&mut owner.pending.branch, PreparedBranch::empty());
    let splice = match sealed.bind_entering(entering) {
        Ok(splice) => splice,
        Err(failure) => {
            let (sealed, entering, _) = failure.into_parts();
            let (active, _, _) = sealed.reject().into_parts();
            entry.active = had_active.then_some(active);
            owner.pending.branch = entering;
            store_back_target_applied_commit_permanent(
                entry,
                owner,
                LifecyclePreflightEvidence::new(
                    LifecyclePreflightSource::CrossRoleRebase,
                    LifecyclePreflightExpectation::RebaseCompatible,
                    LifecyclePreflightObserved::Contradictory,
                ),
            );
            return true;
        }
    };

    let PendingBackTargetAppliedCommit {
        driving,
        ticket,
        mut pending,
    } = owner;
    pending.ticket = Some(ticket);
    let PendingBackTargetDriving {
        history,
        continuation,
    } = driving;
    entry.pending_commit = Some(pending);
    entry.pending_back_target_execution = Some(PendingBackTargetExecution::Classifying(
        PendingBackTargetClassification::Active(TargetPreparationOwner {
            splice,
            history,
            continuation,
        }),
    ));
    true
}

#[cfg(test)]
fn drive_back_target_applied_commit(world: &mut World, entry: &mut RouterEntry) -> bool {
    let handled = advance_back_target_applied_commit(world, entry);
    if handled {
        drive_back_target_applied_classification(world, entry);
    }
    handled
}

fn drive_accepted_back_target_authority_lost(world: &World, entry: &mut RouterEntry) -> bool {
    if !matches!(
        entry.pending_back_target_execution.as_ref(),
        Some(PendingBackTargetExecution::Classifying(
            PendingBackTargetClassification::AuthorityLost { .. }
        ))
    ) {
        return false;
    }
    let Some(PendingBackTargetExecution::Classifying(
        PendingBackTargetClassification::AuthorityLost {
            physical,
            settlement,
            evidence,
        },
    )) = entry.pending_back_target_execution.take()
    else {
        return true;
    };
    let BackTargetClassifiedPhysicalOwner {
        splice,
        history,
        authority_preparation,
    } = physical;
    let Some(AcceptedBackTargetAuthorityPreparation {
        physical,
        ecs_manifest,
        core_binding,
    }) = authority_preparation
    else {
        entry.pending_back_target_execution = Some(PendingBackTargetExecution::Classifying(
            PendingBackTargetClassification::AuthorityLostPreparationMissing {
                physical: BackTargetClassifiedPhysicalOwner {
                    splice,
                    history,
                    authority_preparation: None,
                },
                settlement,
                evidence,
            },
        ));
        return true;
    };
    let cleanup = TargetPreparationPermanentCleanupOwner {
        physical,
        branch: FrozenTargetPreparationPermanentBranch::Classifier {
            physical: BackTargetClassifiedPhysicalOwner {
                splice,
                history,
                authority_preparation: None,
            },
            evidence: PreparationPermanentEvidence::PhysicalPreparationContradiction(evidence),
        },
    };
    let outcome = dispatch_back_target_permanent(
        world,
        entry,
        TargetPreparationPermanentDisposition {
            cleanup,
            settlement,
            ecs_manifest,
            core_binding,
        },
    );
    if !matches!(outcome, BackTargetPermanentDispatchOutcome::Dispatched) {
        entry.pending_back_target_execution = Some(PendingBackTargetExecution::Classifying(
            PendingBackTargetClassification::AuthorityLostDispatchBlocked(outcome),
        ));
    }
    true
}

fn store_back_target_authority_protocol(
    entry: &mut RouterEntry,
    active: RealizedBranch,
    pending: PendingCommit,
    history: HistoryExecutionBindingView,
    continuation: LifecycleContinuationCapacity,
    evidence: LifecyclePreflightEvidence,
    source: BackTargetAuthorityProtocolSourceProjection,
) {
    entry.pending_back_target_execution = Some(PendingBackTargetExecution::Classifying(
        PendingBackTargetClassification::AuthorityLostProtocol(
            PendingBackTargetAuthorityProtocol {
                active,
                pending,
                history,
                continuation,
                evidence,
                source,
            },
        ),
    ));
}

fn observe_back_target_authority_protocol_source(
    world: &World,
    entry: &RouterEntry,
    active: &RealizedBranch,
    pending: &PendingCommit,
) -> BackTargetAuthorityProtocolSourceProjection {
    let missing_resources = RequiredResourceSet::missing_from(world);
    let authority_loss = if !world.contains_resource::<RouteEntityRegistry>() {
        Some(LifecyclePreflightEvidence::new(
            LifecyclePreflightSource::RequiredResources(RequiredResourceSet::ROUTE_ENTITY_REGISTRY),
            LifecyclePreflightExpectation::ResourcePresent,
            LifecyclePreflightObserved::Missing,
        ))
    } else if entry
        .host
        .as_ref()
        .is_none_or(|host| !current_host_binding_is_exact(world, host))
    {
        Some(LifecyclePreflightEvidence::new(
            LifecyclePreflightSource::HostBinding,
            LifecyclePreflightExpectation::ExactAuthority,
            LifecyclePreflightObserved::Drifted,
        ))
    } else {
        None
    };
    let host = entry.host.as_ref().map(ProjectionlessHostProjection::from);
    let host_candidate = entry
        .host
        .as_ref()
        .map(|host| observe_validated_host_candidate(world, &host.candidate));
    let mut roots = active
        .claims()
        .chain(pending.branch.as_realized().claims())
        .map(ExactRegistryClaim::entity)
        .collect::<Vec<_>>();
    if let Some(host) = entry.host.as_ref() {
        roots.push(host.host);
        roots.push(host.top_slot);
    }
    roots.sort_by_key(|entity| entity.to_bits());
    roots.dedup();
    BackTargetAuthorityProtocolSourceProjection {
        missing_resources,
        host,
        host_candidate,
        observed: ProjectionlessObservedProjection::capture(world, roots),
        authority_loss,
    }
}

fn drive_back_target_authority_protocol(world: &mut World, entry: &mut RouterEntry) -> bool {
    if matches!(
        entry.pending_back_target_execution.as_ref(),
        Some(PendingBackTargetExecution::Classifying(
            PendingBackTargetClassification::AuthorityLostUnaccepted { .. }
        ))
    ) {
        let Some(PendingBackTargetExecution::Classifying(
            PendingBackTargetClassification::AuthorityLostUnaccepted { owner, evidence },
        )) = entry.pending_back_target_execution.take()
        else {
            return true;
        };
        if !evidence.is_missing_resource() && !evidence.is_host_binding_authority_loss() {
            entry.pending_back_target_execution = Some(PendingBackTargetExecution::Classifying(
                PendingBackTargetClassification::AuthorityLostUnaccepted { owner, evidence },
            ));
            return true;
        }
        let Some(mut pending) = entry.pending_commit.take() else {
            entry.pending_back_target_execution = Some(PendingBackTargetExecution::Classifying(
                PendingBackTargetClassification::AuthorityLostUnaccepted { owner, evidence },
            ));
            return true;
        };
        let TargetPreparationOwner {
            splice,
            history,
            continuation,
        } = owner;
        let (active, entering, _roles) = splice.reject().into_parts();
        pending.branch = entering;
        let source = observe_back_target_authority_protocol_source(world, entry, &active, &pending);
        if source.authority_loss != Some(evidence) {
            let protocol = PendingBackTargetAuthorityProtocol {
                active,
                pending,
                history,
                continuation,
                evidence,
                source: BackTargetAuthorityProtocolSourceProjection {
                    authority_loss: Some(evidence),
                    ..source.clone()
                },
            };
            entry.pending_back_target_execution = Some(PendingBackTargetExecution::Classifying(
                PendingBackTargetClassification::AuthorityLostProtocolSourceDrift {
                    protocol,
                    observed: source,
                },
            ));
            return true;
        }
        store_back_target_authority_protocol(
            entry,
            active,
            pending,
            history,
            continuation,
            evidence,
            source,
        );
    }

    if !matches!(
        entry.pending_back_target_execution.as_ref(),
        Some(PendingBackTargetExecution::Classifying(
            PendingBackTargetClassification::AuthorityLostProtocol(_)
        ))
    ) {
        return false;
    }
    let Some(pending_execution) = entry.pending_back_target_execution.take() else {
        return false;
    };
    let protocol = match pending_execution {
        PendingBackTargetExecution::Classifying(
            PendingBackTargetClassification::AuthorityLostProtocol(protocol),
        ) => protocol,
        other => {
            entry.pending_back_target_execution = Some(other);
            return false;
        }
    };
    let observed = observe_back_target_authority_protocol_source(
        world,
        entry,
        &protocol.active,
        &protocol.pending,
    );
    if observed != protocol.source {
        entry.pending_back_target_execution = Some(PendingBackTargetExecution::Classifying(
            PendingBackTargetClassification::AuthorityLostProtocolSourceDrift {
                protocol,
                observed,
            },
        ));
        return true;
    }
    let PendingBackTargetAuthorityProtocol {
        active,
        pending,
        history,
        continuation,
        evidence,
        source,
    } = protocol;

    if evidence.is_missing_resource() {
        match fail_closed_normal_owner_protocol(
            world,
            entry,
            active,
            pending,
            AuthorityProtocolResidualKind::PreflightAuthorityLost(evidence),
        ) {
            Ok(()) => {
                drop(history);
                drop(continuation);
            }
            Err((active, pending)) => store_back_target_authority_protocol(
                entry,
                active,
                pending,
                history,
                continuation,
                evidence,
                source,
            ),
        }
        return true;
    }

    if !evidence.is_host_binding_authority_loss() {
        store_back_target_authority_protocol(
            entry,
            active,
            pending,
            history,
            continuation,
            evidence,
            source,
        );
        return true;
    }

    match begin_authority_settlement(world, entry, active, pending, None) {
        #[cfg(feature = "bsn")]
        AuthoritySettlementBeginOutcome::NativeCleanupContradiction {
            active,
            pending,
            error,
        } => {
            match fail_closed_normal_owner_protocol(
                world,
                entry,
                active,
                pending,
                AuthorityProtocolResidualKind::NativeCleanup(error),
            ) {
                Ok(()) => {
                    drop(history);
                    drop(continuation);
                }
                Err((active, pending)) => store_back_target_authority_protocol(
                    entry,
                    active,
                    pending,
                    history,
                    continuation,
                    evidence,
                    source,
                ),
            }
        }
        AuthoritySettlementBeginOutcome::TriggerContradiction {
            active,
            pending,
            kind,
        } => {
            match fail_closed_normal_owner_protocol(
                world,
                entry,
                active,
                pending,
                AuthorityProtocolResidualKind::AuthorityTrigger(kind),
            ) {
                Ok(()) => {
                    drop(history);
                    drop(continuation);
                }
                Err((active, pending)) => store_back_target_authority_protocol(
                    entry,
                    active,
                    pending,
                    history,
                    continuation,
                    evidence,
                    source,
                ),
            }
        }
        AuthoritySettlementBeginOutcome::Begun => {
            drop(history);
            drop(continuation);
        }
        AuthoritySettlementBeginOutcome::RetryablePreparation { active, pending } => {
            store_back_target_authority_protocol(
                entry,
                active,
                pending,
                history,
                continuation,
                evidence,
                source,
            );
        }
        AuthoritySettlementBeginOutcome::ProtocolContradiction {
            active,
            pending,
            kind,
        } => match fail_closed_normal_authority_protocol(world, entry, active, pending, kind) {
            Ok(()) => {
                drop(history);
                drop(continuation);
            }
            Err((active, pending)) => store_back_target_authority_protocol(
                entry,
                active,
                pending,
                history,
                continuation,
                evidence,
                source,
            ),
        },
        AuthoritySettlementBeginOutcome::IsolationContradiction {
            active,
            pending,
            contradiction,
            isolation,
        } => match fail_closed_normal_authority_protocol_prepared(
            world,
            entry,
            active,
            pending,
            AuthorityProtocolResidualKind::AuthorityIsolation(contradiction.clone()),
            isolation,
            Some(contradiction),
        ) {
            Ok(()) => {
                drop(history);
                drop(continuation);
            }
            Err((active, pending)) => store_back_target_authority_protocol(
                entry,
                active,
                pending,
                history,
                continuation,
                evidence,
                source,
            ),
        },
        AuthoritySettlementBeginOutcome::LedgerContradiction {
            active,
            pending,
            kind,
        } => match fail_closed_normal_owner_protocol(
            world,
            entry,
            active,
            pending,
            AuthorityProtocolResidualKind::CleanupLedger(kind),
        ) {
            Ok(()) => {
                drop(history);
                drop(continuation);
            }
            Err((active, pending)) => store_back_target_authority_protocol(
                entry,
                active,
                pending,
                history,
                continuation,
                evidence,
                source,
            ),
        },
    }
    true
}

fn drive_back_target_pending_rejected(entry: &mut RouterEntry) -> bool {
    if matches!(
        entry.pending_back_target_execution.as_ref(),
        Some(PendingBackTargetExecution::Classifying(
            PendingBackTargetClassification::PendingRejectedUnaccepted { .. }
        ))
    ) {
        let Some(PendingBackTargetExecution::Classifying(
            PendingBackTargetClassification::PendingRejectedUnaccepted { owner, evidence },
        )) = entry.pending_back_target_execution.take()
        else {
            return true;
        };
        let classification = accept_classified_back_target_pending_rejected(entry, owner, evidence);
        let matched = matches!(
            classification,
            PendingBackTargetClassification::PendingRejected { .. }
        );
        entry.pending_back_target_execution =
            Some(PendingBackTargetExecution::Classifying(classification));
        if !matched {
            return true;
        }
    }
    let Some(PendingBackTargetExecution::Classifying(
        PendingBackTargetClassification::PendingRejected { physical, .. },
    )) = entry.pending_back_target_execution.as_ref()
    else {
        return false;
    };
    let pending_is_exact = entry.pending_commit.as_ref().is_some_and(|pending| {
        pending.ticket.is_some()
            && pending.final_task.is_none()
            && pending.branch.frames().is_empty()
            && pending.branch.child_slots().is_empty()
    });
    if physical.authority_preparation.is_some()
        || !pending_is_exact
        || entry.active.is_some()
        || entry.pending_failed_branch.is_some()
        || entry.failure_cleanup_permanent.is_some()
        || entry.pending_back_target_permanent_cleanup.is_some()
        || entry.cleanup_ledger.is_some()
        || entry.authority_protocol_residual.is_some()
        || entry.pending_back_no_target_completion.is_some()
        || entry.pending_back_no_target_preparation.is_some()
        || entry.pending_back_no_target_authority_cleanup.is_some()
    {
        return true;
    }
    let Some(PendingBackTargetExecution::Classifying(
        PendingBackTargetClassification::PendingRejected {
            physical,
            continuation,
            evidence,
        },
    )) = entry.pending_back_target_execution.take()
    else {
        return true;
    };
    let BackTargetClassifiedPhysicalOwner {
        splice,
        history,
        authority_preparation,
    } = physical;
    if let Some(authority_preparation) = authority_preparation {
        entry.pending_back_target_execution = Some(PendingBackTargetExecution::Classifying(
            PendingBackTargetClassification::PendingRejected {
                physical: BackTargetClassifiedPhysicalOwner {
                    splice,
                    history,
                    authority_preparation: Some(authority_preparation),
                },
                continuation,
                evidence,
            },
        ));
        return true;
    }
    let Some(mut pending) = entry.pending_commit.take() else {
        entry.pending_back_target_execution = Some(PendingBackTargetExecution::Classifying(
            PendingBackTargetClassification::PendingRejected {
                physical: BackTargetClassifiedPhysicalOwner {
                    splice,
                    history,
                    authority_preparation: None,
                },
                continuation,
                evidence,
            },
        ));
        return true;
    };
    let Some(ticket) = pending.ticket.take() else {
        entry.pending_commit = Some(pending);
        entry.pending_back_target_execution = Some(PendingBackTargetExecution::Classifying(
            PendingBackTargetClassification::PendingRejected {
                physical: BackTargetClassifiedPhysicalOwner {
                    splice,
                    history,
                    authority_preparation: None,
                },
                continuation,
                evidence,
            },
        ));
        return true;
    };
    match entry
        .engine
        .consume_back_target_pending_rejected(continuation, ticket)
    {
        BackTargetPendingRejectedConsumeOutcome::FailureCleanup(request) => {
            drop(history);
            let (active, entering, _roles) = splice.reject().into_parts();
            entry.active = Some(active);
            pending.branch = entering;
            entry.pending_failed_branch = Some(PendingFailedBranch {
                terminal: PendingFailedBranchTerminal::BackTargetPendingRejected {
                    request,
                    evidence,
                },
                branch: failed_prepared_branch(pending, None),
            });
        }
        BackTargetPendingRejectedConsumeOutcome::Mismatch {
            owner,
            ticket,
            mismatch,
        } => {
            pending.ticket = Some(ticket);
            entry.pending_commit = Some(pending);
            entry.pending_back_target_execution = Some(PendingBackTargetExecution::Classifying(
                PendingBackTargetClassification::PendingRejectedConsumeMismatch {
                    physical: BackTargetClassifiedPhysicalOwner {
                        splice,
                        history,
                        authority_preparation: None,
                    },
                    continuation: owner,
                    evidence,
                    mismatch,
                },
            ));
        }
    }
    true
}

fn drive_back_target_root_commit(world: &mut World, entry: &mut RouterEntry) -> bool {
    if advance_back_target_applied_commit(world, entry) {
        drive_back_target_applied_classification(world, entry);
        let _ = drive_back_target_authority_protocol(world, entry);
        let _ = drive_accepted_back_target_authority_lost(world, entry);
        return true;
    }
    if drive_back_target_authority_protocol(world, entry) {
        return true;
    }
    if drive_accepted_back_target_authority_lost(world, entry) {
        return true;
    }
    if drive_back_target_pending_rejected(entry) {
        return true;
    }
    if !matches!(
        entry.pending_back_target_execution.as_ref(),
        Some(PendingBackTargetExecution::Driving(_))
    ) {
        return entry.pending_back_target_execution.is_some();
    }
    let Some(mut pending) = entry.pending_commit.take() else {
        return true;
    };
    if pending.ticket.is_some() {
        let Some(PendingBackTargetExecution::Driving(driving)) =
            entry.pending_back_target_execution.take()
        else {
            entry.pending_commit = Some(pending);
            return true;
        };
        let ticket = pending
            .ticket
            .take()
            .expect("the observed target Commit ticket remains held");
        entry.pending_back_target_execution = Some(PendingBackTargetExecution::ResumeBlocked(
            PendingBackTargetResumeBlocked::AppliedCommitPending(PendingBackTargetAppliedCommit {
                driving,
                ticket,
                pending,
            }),
        ));
        let _ = advance_back_target_applied_commit(world, entry);
        drive_back_target_applied_classification(world, entry);
        return true;
    }
    let Some(task) = pending.final_task.take() else {
        entry.pending_commit = Some(pending);
        return true;
    };
    match route_back_target_applied_report(
        entry,
        pending,
        task,
        BackTargetAppliedReport::LegacyLeaf,
    ) {
        RouteBackTargetAppliedOutcome::Handled => {
            let _ = advance_back_target_applied_commit(world, entry);
            drive_back_target_applied_classification(world, entry);
            true
        }
        RouteBackTargetAppliedOutcome::NonTarget {
            mut pending,
            task,
            report,
        } => {
            drop(report);
            pending.final_task = Some(task);
            entry.pending_commit = Some(pending);
            false
        }
    }
}

fn complete_back_target_focus_source(
    world: &World,
    splice: &SealedReconciliationSplice,
    mut focus_leaving: Vec<Entity>,
) -> Vec<Entity> {
    focus_leaving.extend(observed_descendant_closure(
        world,
        splice
            .slot_roles()
            .iter()
            .filter(|role| role.kind() == ReconciliationSlotRoleKind::RetainEmpty)
            .map(|role| role.entity()),
    ));
    focus_leaving.sort_by_key(|entity| entity.to_bits());
    focus_leaving.dedup();
    focus_leaving
}

fn publish_ready_back_target(
    world: &mut World,
    handle: RouterHandle,
    mut pending: PendingCommit,
    owner: BackTargetReadyAcceptanceOwner,
) {
    let BackTargetReadyAcceptanceOwner {
        physical,
        continuation,
        frozen_manifest,
        manifest,
    } = owner;
    let PreparedBackTargetPhysicalOwner {
        classified,
        focus_leaving,
    } = physical;
    let BackTargetClassifiedPhysicalOwner {
        splice,
        history,
        authority_preparation,
    } = classified;
    let host_is_current = world
        .resource::<RouterNavigation>()
        .entries
        .iter()
        .find(|entry| entry.handle == handle)
        .and_then(|entry| entry.host.as_ref())
        .is_some_and(|host| current_host_binding_is_exact(world, host));
    let has_leaving =
        !splice.leaving().frames().is_empty() || !splice.leaving().child_slots().is_empty();
    let final_preflight =
        match reconciliation_preflight(world, host_is_current, has_leaving, splice) {
            ReconciliationPreflightOutcome::Ready {
                splice,
                focus_leaving: observed_focus,
            } => {
                let observed_focus =
                    complete_back_target_focus_source(world, &splice, observed_focus);
                if observed_focus == focus_leaving {
                    ReconciliationPreflightOutcome::Ready {
                        splice,
                        focus_leaving: observed_focus,
                    }
                } else {
                    // The issued Ready owner freezes the physical focus source, not
                    // the focused value. Return its original capability for the
                    // normal pending-rejection cleanup/fallback continuation.
                    ReconciliationPreflightOutcome::PendingRejected {
                        splice,
                        evidence: LifecyclePreflightEvidence::new(
                            LifecyclePreflightSource::TargetPartition,
                            LifecyclePreflightExpectation::ExactRelationshipClosure,
                            LifecyclePreflightObserved::Drifted,
                        ),
                    }
                }
            }
            rejected => rejected,
        };
    let splice = match final_preflight {
        ReconciliationPreflightOutcome::Ready { splice, .. } => splice,
        rejected => {
            let mut navigation = world.resource_mut::<RouterNavigation>();
            let entry = navigation
                .entries
                .iter_mut()
                .find(|entry| entry.handle == handle)
                .unwrap();
            entry.pending_back_target_execution =
                Some(PendingBackTargetExecution::Classifying(match rejected {
                    ReconciliationPreflightOutcome::PendingRejected { splice, evidence } => {
                        PendingBackTargetClassification::PendingRejectedUnaccepted {
                            owner: TargetPreparationOwner {
                                splice,
                                history,
                                continuation,
                            },
                            evidence,
                        }
                    }
                    ReconciliationPreflightOutcome::AuthorityLost { splice, evidence } => {
                        PendingBackTargetClassification::AuthorityLostUnaccepted {
                            owner: TargetPreparationOwner {
                                splice,
                                history,
                                continuation,
                            },
                            evidence,
                        }
                    }
                    ReconciliationPreflightOutcome::Ready { .. } => unreachable!(),
                }));
            entry.pending_commit = Some(pending);
            // Return the current branch held by a rejected splice before a
            // future input Refresh observes the entry's temporary vacancy.
            let _ = drive_back_target_pending_rejected(entry);
            return;
        }
    };
    let owner = BackTargetReadyAcceptanceOwner {
        physical: PreparedBackTargetPhysicalOwner {
            classified: BackTargetClassifiedPhysicalOwner {
                splice,
                history,
                authority_preparation,
            },
            focus_leaving,
        },
        continuation,
        frozen_manifest,
        manifest,
    };
    let missing = RequiredResourceSet::missing_from(world);
    if !missing.is_empty() {
        let evidence = LifecyclePreflightEvidence::new(
            LifecyclePreflightSource::RequiredResources(missing),
            LifecyclePreflightExpectation::ResourcePresent,
            LifecyclePreflightObserved::Missing,
        );
        let mut navigation = world.resource_mut::<RouterNavigation>();
        let entry = navigation
            .entries
            .iter_mut()
            .find(|entry| entry.handle == handle)
            .unwrap();
        entry.pending_back_target_execution = Some(PendingBackTargetExecution::Classifying(
            PendingBackTargetClassification::ReadyPlanPermanent(PreparationPermanentOwner {
                owner,
                evidence: PreparationPermanentEvidence::PhysicalPreparationContradiction(evidence),
            }),
        ));
        entry.pending_commit = Some(pending);
        return;
    }
    let Some(real_ticket) = pending.ticket.take() else {
        let evidence = LifecyclePreflightEvidence::new(
            LifecyclePreflightSource::PendingAdmission,
            LifecyclePreflightExpectation::ExactTaskShape,
            LifecyclePreflightObserved::Contradictory,
        );
        let mut navigation = world.resource_mut::<RouterNavigation>();
        let entry = navigation
            .entries
            .iter_mut()
            .find(|entry| entry.handle == handle)
            .unwrap();
        entry.pending_back_target_execution = Some(PendingBackTargetExecution::Classifying(
            PendingBackTargetClassification::ReadyPlanPermanent(PreparationPermanentOwner {
                owner,
                evidence: PreparationPermanentEvidence::PhysicalPreparationContradiction(evidence),
            }),
        ));
        entry.pending_commit = Some(pending);
        return;
    };

    let splice = &owner.physical.classified.splice;
    let scroll_eligible = crate::scroll::eligible_containers(world);
    let scroll_observation = crate::scroll::ScrollObservation::capture(
        world,
        splice
            .preserved()
            .frames()
            .iter()
            .chain(splice.leaving().frames()),
        splice
            .preserved()
            .frames()
            .iter()
            .chain(splice.entering().frames()),
        &scroll_eligible,
    );
    let scroll_targets = scroll_observation.target_entities();
    let gate_before = splice.current_live_interactive_entities(world);
    let (targets, isolate_count) = splice.presentation_mutation_targets();
    let gate_after = splice.resulting_live_interactive_entities(world, &targets);
    let transitions = splice.lifecycle_transitions();
    let registry_changes = transitions
        .iter()
        .any(|(claim, next)| claim.lifecycle() != *next);
    let root_rebase = if !splice.leaving().frames().is_empty()
        && !splice.entering().frames().is_empty()
    {
        let Some(rebase) = splice.prepare_root_rebase() else {
            let evidence = LifecyclePreflightEvidence::new(
                LifecyclePreflightSource::CrossRoleRebase,
                LifecyclePreflightExpectation::RebaseCompatible,
                LifecyclePreflightObserved::Contradictory,
            );
            let mut navigation = world.resource_mut::<RouterNavigation>();
            let entry = navigation
                .entries
                .iter_mut()
                .find(|entry| entry.handle == handle)
                .unwrap();
            entry.pending_back_target_execution = Some(PendingBackTargetExecution::Classifying(
                PendingBackTargetClassification::ReadyPlanPermanent(PreparationPermanentOwner {
                    owner,
                    evidence: PreparationPermanentEvidence::PhysicalPreparationContradiction(
                        evidence,
                    ),
                }),
            ));
            pending.ticket = Some(real_ticket);
            entry.pending_commit = Some(pending);
            return;
        };
        Some(rebase)
    } else {
        None
    };

    #[cfg(feature = "bsn")]
    let native_focus = crate::native_focus::prepare_publication(
        world,
        owner.physical.classified.splice.entering().frames(),
        &gate_after,
    );
    let frozen_is_exact = (|| {
        let registry = world.get_resource::<RouteEntityRegistry>()?;
        let ready = &owner;
        let splice = &ready.physical.classified.splice;
        let registry_plan = registry.prepare_lifecycle_mutation(&transitions)?;
        let resulting_claims = registry_plan.resulting_claims().to_vec();
        let frozen_again = if splice.leaving().obligation_count() == 0 {
            FrozenCleanupManifest::empty()
        } else {
            FrozenCleanupManifest::from_leaving_branch(
                world,
                &registry,
                splice.leaving(),
                &resulting_claims,
                (!splice.entering().frames().is_empty()).then_some(splice.entering()),
            )?
        };
        if frozen_again != ready.frozen_manifest {
            return None;
        }

        Some(())
    })()
    .is_some();
    let mut ready_owner = Some(owner);
    let mut ticket = Some(real_ticket);
    let mut next_stage = None;
    let committed = (|| {
        if !frozen_is_exact {
            return None;
        }
        let pointer_targets = prepare_input_gate_mutation(
            world.get_resource_mut::<RouterInputGate>()?,
            &gate_before,
            gate_after.clone(),
        )?
        .revoked_targets()
        .to_vec();
        let (presentation, resources) =
            crate::presentation::prepare_publication_access_with_scroll(
                world,
                &targets,
                &pointer_targets,
                &scroll_targets,
            )?;
        let crate::presentation::PublicationResources {
            mut navigation,
            mut registry,
            gate,
            input_focus,
            route_focus,
        } = resources;
        let entry = navigation
            .entries
            .iter_mut()
            .find(|entry| entry.handle == handle)?;
        let ready = ready_owner.as_ref()?;
        let splice = &ready.physical.classified.splice;
        let registry_plan = registry.prepare_lifecycle_mutation(&transitions)?;
        let resulting_claims = registry_plan.resulting_claims().to_vec();
        let gate_plan = prepare_input_gate_mutation(gate, &gate_before, gate_after)?;
        let focus_plan =
            prepare_focus_mutation(input_focus, route_focus, &ready.physical.focus_leaving);
        #[cfg(feature = "bsn")]
        let focus_plan = focus_plan.with_native_publication(native_focus.as_ref()?)?;
        let claim_rebind = splice.prepare_claim_rebind(&resulting_claims)?;
        let presentation_plan = presentation.seal_commit(isolate_count)?;
        let BackTargetReadyAcceptanceOwner {
            physical,
            continuation,
            frozen_manifest,
            manifest,
        } = ready_owner
            .take()
            .expect("the scoped Ready owner is consumed once");
        let scope = continuation.preparation_boundary_scope();
        let continuation = match entry.engine.accept_back_target_ready(continuation, scope) {
            BackTargetPreparationAcceptance::Matched(continuation) => continuation,
            BackTargetPreparationAcceptance::Mismatch { capacity, mismatch } => {
                next_stage = Some(PendingBackTargetExecution::Classifying(
                    PendingBackTargetClassification::ReadyAcceptanceMismatch {
                        owner: BackTargetReadyAcceptanceOwner {
                            physical,
                            continuation: capacity,
                            frozen_manifest,
                            manifest,
                        },
                        mismatch,
                    },
                ));
                return None;
            }
        };
        entry.pending_back_target_execution = Some(PendingBackTargetExecution::Classifying(
            PendingBackTargetClassification::ReadyAccepted(PendingBackTargetReadyAccepted {
                physical,
                continuation,
                frozen_manifest,
                manifest,
            }),
        ));
        let Some(PendingBackTargetExecution::Classifying(
            PendingBackTargetClassification::ReadyAccepted(accepted),
        )) = entry.pending_back_target_execution.take()
        else {
            return None;
        };
        let ledger = match CleanupLedger::reserve(accepted.frozen_manifest) {
            Ok(ledger) => ledger,
            Err(error) => {
                next_stage = Some(PendingBackTargetExecution::Classifying(
                    PendingBackTargetClassification::ReadyReserveFailure {
                        physical: accepted.physical,
                        continuation: accepted.continuation,
                        manifest: accepted.manifest,
                        error,
                    },
                ));
                return None;
            }
        };
        entry.pending_back_target_execution = Some(PendingBackTargetExecution::Prepared(
            PendingPreparedBackTargetCommit {
                physical: accepted.physical,
                continuation: accepted.continuation,
                ledger,
                manifest: accepted.manifest,
            },
        ));

        let ticket_ref = ticket
            .as_ref()
            .expect("the scoped Ready driver retains its real ticket");
        let pairing = {
            let Some(PendingBackTargetExecution::Prepared(prepared)) =
                entry.pending_back_target_execution.as_ref()
            else {
                return None;
            };
            entry.engine.validate_back_target_execution_pairing(
                BackTargetExecutionOwnerRef::Prepared {
                    history: &prepared.physical.classified.history,
                    continuation: &prepared.continuation,
                },
                BackTargetPhysicalOwnerRef::CommitTicket(ticket_ref),
            )
        };
        if let BackTargetExecutionPairingOutcome::Mismatch(mismatch) = pairing {
            let Some(PendingBackTargetExecution::Prepared(owner)) =
                entry.pending_back_target_execution.take()
            else {
                return None;
            };
            next_stage = Some(PendingBackTargetExecution::CommitFailure(
                PendingBackTargetCommitFailure::PairingMismatch { owner, mismatch },
            ));
            return None;
        }
        let Some(PendingBackTargetExecution::Prepared(prepared)) =
            entry.pending_back_target_execution.take()
        else {
            return None;
        };
        let PendingPreparedBackTargetCommit {
            physical,
            continuation,
            ledger,
            manifest,
        } = prepared;
        match entry
            .engine
            .prepare_back_target_commit(continuation, ticket_ref, manifest)
        {
            BackTargetCommitPreparationOutcome::Mismatch {
                owner,
                manifest,
                mismatch,
            } => {
                next_stage = Some(PendingBackTargetExecution::CommitFailure(
                    PendingBackTargetCommitFailure::ReceiverMismatch {
                        owner: PendingPreparedBackTargetCommit {
                            physical,
                            continuation: owner,
                            ledger,
                            manifest,
                        },
                        mismatch,
                    },
                ));
                None
            }
            BackTargetCommitPreparationOutcome::PreparationFailed(failure) => {
                next_stage = Some(PendingBackTargetExecution::CommitFailure(
                    PendingBackTargetCommitFailure::Failed {
                        physical,
                        ledger,
                        failure,
                    },
                ));
                None
            }
            BackTargetCommitPreparationOutcome::Validated(engine) => {
                let scroll = entry
                    .scroll
                    .prepare(engine.history_publication(), scroll_observation);
                let presentation_plan = presentation_plan.with_scroll(&scroll.writes);
                let cleanup = ledger.bind_batch(engine.cleanup().clone());
                drop(
                    ticket
                        .take()
                        .expect("successful publication ends the real ticket"),
                );
                let mut splice = physical.classified.splice;
                splice.apply_claim_rebind(claim_rebind);
                if let Some(rebase) = root_rebase {
                    splice.apply_root_rebase(rebase);
                }
                let outcome = splice.apply();
                let (mut active, leaving) = outcome.into_active_and_leaving();
                active.consume_native_focus_owners();
                let next_active = (!active.frames().is_empty()).then_some(active);
                let next_cleanup = Some(CleanupLedger::activate_leaving_branch(
                    cleanup,
                    (!leaving.frames().is_empty() || !leaving.child_slots().is_empty())
                        .then_some(leaving),
                ));
                let active_owner = &mut entry.active;
                let cleanup_owner = &mut entry.cleanup_ledger;
                let scroll_owner = &mut entry.scroll;
                engine.publish(|| {
                    presentation_plan.apply_staged(
                        |_| focus_plan.apply(),
                        |_| {
                            if registry_changes {
                                registry_plan.apply(&mut registry);
                            }
                        },
                        |_| gate_plan.apply(),
                    );
                    *active_owner = next_active;
                    *cleanup_owner = next_cleanup;
                    *scroll_owner = scroll.state;
                });
                Some(())
            }
        }
    })();
    if committed.is_some() {
        return;
    }
    let mut navigation = world.resource_mut::<RouterNavigation>();
    let entry = navigation
        .entries
        .iter_mut()
        .find(|entry| entry.handle == handle)
        .unwrap();
    if committed.is_none() {
        pending.ticket = ticket.take();
        if let Some(stage) = next_stage {
            entry.pending_back_target_execution = Some(stage);
        } else if let Some(owner) = ready_owner {
            let evidence = LifecyclePreflightEvidence::new(
                LifecyclePreflightSource::TargetPartition,
                LifecyclePreflightExpectation::ExactAuthority,
                LifecyclePreflightObserved::Drifted,
            );
            entry.pending_back_target_execution = Some(PendingBackTargetExecution::Classifying(
                PendingBackTargetClassification::ReadyPlanPermanent(PreparationPermanentOwner {
                    owner,
                    evidence: PreparationPermanentEvidence::PhysicalPreparationContradiction(
                        evidence,
                    ),
                }),
            ));
        }
        entry.pending_commit = Some(pending);
    }
}

struct SealedNormalPreparation {
    prepared: r3_core::PreparedCommit,
    manifest: FrozenCleanupManifest,
}

enum PendingPublicationWork {
    Normal {
        pending: PendingCommit,
        splice: NormalPublicationSplice,
        had_active: bool,
        has_leaving: bool,
        sealed: Option<SealedNormalPreparation>,
    },
    Back {
        pending: PendingCommit,
        owner: BackTargetReadyAcceptanceOwner,
    },
    Projectionless(ProjectionlessPublicationWork),
}

pub(crate) fn commit_presentation_system(world: &mut World) {
    #[cfg(feature = "bsn")]
    let _ = crate::native_focus::observe(world, &[]);
    let handles = world
        .resource::<RouterNavigation>()
        .entries
        .iter()
        .map(|entry| entry.handle)
        .collect::<Vec<_>>();
    for handle in handles {
        let work = world.resource_scope(
            |world, mut navigation: bevy_ecs::change_detection::Mut<RouterNavigation>| {
                let entry = navigation
                    .entries
                    .iter_mut()
                    .find(|entry| entry.handle == handle)?;
                prepare_commit_entry(world, entry)
            },
        );
        match work {
            Some(PendingPublicationWork::Normal {
                pending,
                splice,
                had_active,
                has_leaving,
                sealed,
            }) => publish_normal_work(
                world,
                handle,
                pending,
                splice,
                had_active,
                has_leaving,
                sealed,
            ),
            Some(PendingPublicationWork::Back { pending, owner }) => {
                publish_ready_back_target(world, handle, pending, owner);
            }
            Some(PendingPublicationWork::Projectionless(work)) => {
                publish_projectionless_work(world, handle, work)
            }
            None => {}
        }
    }
}

fn prepare_commit_entry(
    world: &mut World,
    entry: &mut RouterEntry,
) -> Option<PendingPublicationWork> {
    if entry.authority_protocol_residual.is_some() {
        return None;
    }
    if drive_back_target_root_commit(world, entry) {
        let ready = matches!(
            entry.pending_back_target_execution.as_ref(),
            Some(PendingBackTargetExecution::Classifying(
                PendingBackTargetClassification::ReadyAwaitingAcceptance(_)
            ))
        );
        if ready && entry.pending_commit.is_some() {
            let Some(PendingBackTargetExecution::Classifying(
                PendingBackTargetClassification::ReadyAwaitingAcceptance(owner),
            )) = entry.pending_back_target_execution.take()
            else {
                unreachable!()
            };
            return Some(PendingPublicationWork::Back {
                pending: entry.pending_commit.take().unwrap(),
                owner,
            });
        }
        return None;
    }
    if let Some(continuation) = entry.projectionless_fail_close.take() {
        let source = match continuation {
            ProjectionlessFailCloseContinuation::Retryable(source) => source,
        };
        let active = entry.active.take();
        let pending = entry.pending_commit.take();
        let (active, pending) = match (active, pending) {
            (Some(active), Some(pending)) => (active, pending),
            (active, pending) => {
                entry.active = active;
                entry.pending_commit = pending;
                return None;
            }
        };
        match settle_projectionless_fail_close(world, entry, active, pending, source) {
            Ok(()) => {}
            Err((active, pending, source)) => {
                entry.active = Some(active);
                entry.pending_commit = Some(pending);
                entry.projectionless_fail_close =
                    Some(ProjectionlessFailCloseContinuation::Retryable(source));
            }
        }
        return None;
    }
    let Some(mut pending) = entry.pending_commit.take() else {
        return None;
    };
    if let Some(prepared) = pending.rejected_retained_publication.take() {
        match entry
            .engine
            .cancel_prepared_retained_commit_for_authority_settlement(*prepared)
        {
            Ok(owners) => {
                assert!(
                    entry.retained.restore_owners(owners).is_ok(),
                    "cancelled publication returns its original cache capabilities"
                );
            }
            Err((prepared, _)) => pending.rejected_retained_publication = Some(Box::new(prepared)),
        }
        let had_active = entry.active.is_some();
        let active = entry.active.take().unwrap_or_else(RealizedBranch::empty);
        if let Err((active, pending)) = fail_closed_normal_owner_protocol(
            world,
            entry,
            active,
            pending,
            AuthorityProtocolResidualKind::Authority(BeginAuthoritySettlementErrorKind::Stale),
        ) {
            entry.active = had_active.then_some(active);
            entry.pending_commit = Some(pending);
        }
        return None;
    }
    if !pending.branch.frames().is_empty()
        && pending.ticket.is_none()
        && pending.final_task.is_none()
    {
        entry.pending_commit = Some(pending);
        return None;
    }
    let Some(authority_root) = entry
        .active
        .as_ref()
        .and_then(RealizedBranch::first_frame)
        .or_else(|| pending.branch.first_frame())
    else {
        entry.pending_commit = Some(pending);
        return None;
    };
    let Some(authority) = ReconciliationAuthorityIdentity::from_frame(
        authority_root.frame(),
        authority_root.frame().host(),
        authority_root.frame().slot(),
    ) else {
        entry.pending_commit = Some(pending);
        return None;
    };
    let (facts, has_leaving) = {
        let projection = if let Some(plan) = pending.retained_hit.as_ref() {
            Ok(plan.reconciliation())
        } else {
            match (pending.ticket.as_ref(), pending.final_task.as_ref()) {
                (Some(ticket), _) => entry.engine.reconciliation_projection_for_commit(ticket),
                (None, Some(task)) => entry.engine.reconciliation_projection_for_task(task),
                (None, None) => {
                    entry.pending_commit = Some(pending);
                    return None;
                }
            }
        };
        let Ok(projection) = projection else {
            let mut work = None;
            if let Err(pending) = prepare_projectionless_branch(world, entry, pending, &mut work) {
                entry.pending_commit = Some(pending);
            }
            return work.map(PendingPublicationWork::Projectionless);
        };
        let has_leaving = projection.leaving().len() != 0;
        let Some(facts) = ReconciliationProjectionFacts::from_core(&projection, authority) else {
            entry.pending_commit = Some(pending);
            return None;
        };
        (facts, has_leaving)
    };
    let had_active = entry.active.is_some();
    let active_owner = entry.active.take().unwrap_or_else(RealizedBranch::empty);
    let sealed = match if let Some(plan) = pending.retained_hit.as_ref() {
        SealedExistingReconciliation::prepare_retained_from_projection(
            active_owner,
            facts,
            plan.hit(),
        )
    } else {
        SealedExistingReconciliation::prepare_from_projection(active_owner, facts)
    } {
        Ok(sealed) => sealed,
        Err(failure) => {
            let (active, _) = failure.into_parts();
            entry.active = had_active.then_some(active);
            entry.pending_commit = Some(pending);
            return None;
        }
    };
    let splice = if let Some(plan) = pending.retained_hit.as_ref() {
        let Some(branch) = entry.retained.branch_for(plan.hit()) else {
            let (active, _, _) = sealed.reject().into_parts();
            entry.active = had_active.then_some(active);
            entry.pending_commit = Some(pending);
            return None;
        };
        match sealed.prepare_retained_entering(world, branch, plan.hit()) {
            Ok(prepared) => NormalPublicationSplice::Retained {
                prepared,
                hit: plan.hit().clone(),
            },
            Err((sealed, _)) => {
                let (active, _, _) = sealed.reject().into_parts();
                entry.active = had_active.then_some(active);
                let task = pending.final_task.take()?;
                entry.pending_failed_branch = Some(PendingFailedBranch {
                    terminal: PendingFailedBranchTerminal::Task {
                        task,
                        failure: ViewFailureKind::Apply,
                        message: "retained entering preparation rejected".into(),
                    },
                    branch: failed_prepared_branch(pending, None),
                });
                return None;
            }
        }
    } else {
        let entering_owner = std::mem::replace(&mut pending.branch, PreparedBranch::empty());
        match sealed.bind_entering(entering_owner) {
            Ok(splice) => NormalPublicationSplice::Fresh(splice),
            Err(failure) => {
                let (sealed, entering, _) = failure.into_parts();
                let (active, _, _roles) = sealed.reject().into_parts();
                entry.active = had_active.then_some(active);
                pending.branch = entering;
                entry.pending_commit = Some(pending);
                return None;
            }
        }
    };

    let mut splice = splice;
    let sealed = if let NormalPublicationSplice::Fresh(fresh) = splice {
        let (fresh, sealed) = if commit_sealed_registration_removal_is_armed(entry) {
            let host_is_current = entry
                .host
                .as_ref()
                .is_some_and(|host| current_host_binding_is_exact(world, host));
            let (observed_splice, ready) =
                match reconciliation_preflight(world, host_is_current, has_leaving, fresh) {
                    ReconciliationPreflightOutcome::Ready { splice, .. } => (splice, true),
                    ReconciliationPreflightOutcome::PendingRejected { splice, .. }
                    | ReconciliationPreflightOutcome::AuthorityLost { splice, .. } => {
                        (splice, false)
                    }
                };
            let sealed = if ready {
                prepare_normal_registration_probe(world, entry, &mut pending, &observed_splice)
            } else {
                None
            };
            (observed_splice, sealed)
        } else {
            (fresh, None)
        };
        splice = NormalPublicationSplice::Fresh(fresh);
        sealed
    } else {
        None
    };
    Some(PendingPublicationWork::Normal {
        pending,
        splice,
        had_active,
        has_leaving,
        sealed,
    })
}

fn prepare_normal_registration_probe(
    world: &mut World,
    entry: &mut RouterEntry,
    pending: &mut PendingCommit,
    splice: &SealedReconciliationSplice,
) -> Option<SealedNormalPreparation> {
    let registry = world.get_resource::<RouteEntityRegistry>()?;
    let transitions = splice.lifecycle_transitions();
    let registry_plan = registry.prepare_lifecycle_mutation(&transitions)?;
    let manifest = if splice.leaving().obligation_count() == 0 {
        FrozenCleanupManifest::empty()
    } else {
        FrozenCleanupManifest::from_leaving_branch(
            world,
            registry,
            splice.leaving(),
            registry_plan.resulting_claims(),
            (!splice.entering().frames().is_empty()).then_some(splice.entering()),
        )?
    };
    if pending.ticket.is_none() {
        pending.ticket = Some(
            entry
                .engine
                .report_applied(pending.final_task.take()?)
                .expect("the preflighted probe retains its exact reportable task"),
        );
    }
    let prepared = entry
        .engine
        .prepare_commit(
            pending.ticket.as_ref()?,
            CleanupManifest::new(
                logical_cleanup_roots(splice.leaving()),
                manifest.digest(),
                manifest.len(),
            ),
        )
        .ok()?;
    // The probe changes a real authority input only after the owned Core seal.
    // Its World writes and resource-scope exit precede the final refreeze.
    inject_commit_sealed_registration_removal(world, entry);
    Some(SealedNormalPreparation { prepared, manifest })
}

fn settle_prepublication_authority_loss(
    world: &mut World,
    entry: &mut RouterEntry,
    active: RealizedBranch,
    pending: PendingCommit,
    had_active: bool,
    evidence: LifecyclePreflightEvidence,
    initial_fallback: bool,
) {
    if evidence.is_missing_resource() {
        if let Err((active, pending)) = fail_closed_normal_owner_protocol(
            world,
            entry,
            active,
            pending,
            AuthorityProtocolResidualKind::PreflightAuthorityLost(evidence),
        ) {
            entry.active = had_active.then_some(active);
            entry.pending_commit = Some(pending);
        }
        return;
    }
    let outcome = if initial_fallback {
        begin_initial_fallback_authority_settlement(world, entry, pending)
    } else {
        begin_authority_settlement(world, entry, active, pending, None)
    };
    match outcome {
        #[cfg(feature = "bsn")]
        AuthoritySettlementBeginOutcome::NativeCleanupContradiction {
            active,
            pending,
            error,
        } => {
            if let Err((active, pending)) = fail_closed_normal_owner_protocol(
                world,
                entry,
                active,
                pending,
                AuthorityProtocolResidualKind::NativeCleanup(error),
            ) {
                entry.active = had_active.then_some(active);
                entry.pending_commit = Some(pending);
            }
        }
        AuthoritySettlementBeginOutcome::TriggerContradiction {
            active,
            pending,
            kind,
        } => {
            if let Err((active, pending)) = fail_closed_normal_owner_protocol(
                world,
                entry,
                active,
                pending,
                AuthorityProtocolResidualKind::AuthorityTrigger(kind),
            ) {
                entry.active = had_active.then_some(active);
                entry.pending_commit = Some(pending);
            }
        }
        AuthoritySettlementBeginOutcome::Begun => {}
        AuthoritySettlementBeginOutcome::RetryablePreparation { active, pending } => {
            entry.active = had_active.then_some(active);
            entry.pending_commit = Some(pending);
        }
        AuthoritySettlementBeginOutcome::ProtocolContradiction {
            active,
            pending,
            kind,
        } => {
            if let Err((active, pending)) =
                fail_closed_normal_authority_protocol(world, entry, active, pending, kind)
            {
                entry.active = had_active.then_some(active);
                entry.pending_commit = Some(pending);
                return;
            }
        }
        AuthoritySettlementBeginOutcome::IsolationContradiction {
            active,
            pending,
            contradiction,
            isolation,
        } => {
            if let Err((active, pending)) = fail_closed_normal_authority_protocol_prepared(
                world,
                entry,
                active,
                pending,
                AuthorityProtocolResidualKind::AuthorityIsolation(contradiction.clone()),
                isolation,
                Some(contradiction),
            ) {
                entry.active = had_active.then_some(active);
                entry.pending_commit = Some(pending);
                return;
            }
        }
        AuthoritySettlementBeginOutcome::LedgerContradiction {
            active,
            pending,
            kind,
        } => {
            if let Err((active, pending)) = fail_closed_normal_owner_protocol(
                world,
                entry,
                active,
                pending,
                AuthorityProtocolResidualKind::CleanupLedger(kind),
            ) {
                entry.active = had_active.then_some(active);
                entry.pending_commit = Some(pending);
                return;
            }
        }
    }
}

mod normal_publication;
use normal_publication::{NormalPublicationSplice, publish_normal_work};

mod authority_isolation;
mod idle_host_loss;
#[cfg(test)]
use authority_isolation::{
    AuthorityIsolationComponent, AuthorityIsolationPredicate, AuthorityIsolationViolation,
    target_authority_missing_pickable_repair_is_exact,
};
use authority_isolation::{
    AuthorityIsolationContradiction, AuthorityIsolationPreparation,
    AuthoritySettlementBeginOutcome, PreparedAuthorityIsolation, begin_authority_settlement,
    begin_initial_fallback_authority_settlement, begin_unstarted_fallback_host_loss,
    fail_closed_cleanup_settlement_permanent, fail_closed_failed_authority_protocol,
    fail_closed_failed_manifest_protocol, fail_closed_failed_owner_protocol,
    fail_closed_failed_owner_protocol_prepared, fail_closed_failure_cleanup_completion_protocol,
    fail_closed_normal_authority_protocol, fail_closed_normal_authority_protocol_prepared,
    fail_closed_normal_owner_protocol, fail_closed_projectionless_owner_protocol,
    prepare_authority_isolation_targets, prepare_failed_branch_authority_isolation,
    prepare_failed_manifest_protocol_isolation, prepare_normal_authority_isolation,
    prepare_post_commit_input_isolation, prepare_target_preparation_permanent_isolation,
};

enum ProjectionlessFallbackPreflight {
    Ready {
        admission: ProjectionlessFallbackAdmission,
        focus_leaving: Vec<bevy_ecs::entity::Entity>,
    },
    PendingRejected(LifecyclePreflightEvidence),
    AuthorityLost(LifecyclePreflightEvidence),
}

enum ProjectionlessCommitFailure {
    PostReportAuthorityLost {
        admission: ProjectionlessFallbackAdmission,
        reason: BeginAuthoritySettlementErrorKind,
    },
    InitialPostReportAuthorityLost {
        reason: BeginAuthoritySettlementErrorKind,
    },
}

impl ProjectionlessCommitFailure {
    fn post_report(
        admission: Option<ProjectionlessFallbackAdmission>,
        reason: BeginAuthoritySettlementErrorKind,
    ) -> Self {
        match admission {
            Some(admission) => Self::PostReportAuthorityLost { admission, reason },
            None => Self::InitialPostReportAuthorityLost { reason },
        }
    }
}

impl ProjectionlessFailCloseSource {
    fn freeze(
        world: &World,
        entry: &RouterEntry,
        admission: ProjectionlessFallbackAdmission,
        reason: BeginAuthoritySettlementErrorKind,
        active: &RealizedBranch,
        pending: &PendingCommit,
    ) -> Self {
        Self {
            reason,
            projection: ProjectionlessContinuationProjection::capture(
                world, entry, admission, active, pending,
            ),
        }
    }

    fn refreezes_exactly(
        &self,
        world: &World,
        entry: &RouterEntry,
        active: &RealizedBranch,
        pending: &PendingCommit,
    ) -> Result<(), ProjectionlessSourceDriftEvidence> {
        let observed = ProjectionlessContinuationProjection::capture(
            world,
            entry,
            self.projection.admission.clone(),
            active,
            pending,
        );
        if observed == self.projection {
            Ok(())
        } else {
            Err(ProjectionlessSourceDriftEvidence {
                expected: self.projection.clone(),
                observed,
            })
        }
    }
}

impl ProjectionlessContinuationProjection {
    fn capture(
        world: &World,
        entry: &RouterEntry,
        admission: ProjectionlessFallbackAdmission,
        active: &RealizedBranch,
        pending: &PendingCommit,
    ) -> Self {
        let commit_identity_is_current = pending
            .ticket
            .as_ref()
            .is_some_and(|ticket| entry.engine.inspect_commit_identity(ticket).is_ok());
        let host_binding_is_current = entry
            .host
            .as_ref()
            .is_some_and(|host| current_host_binding_is_exact(world, host));
        let active_claims = active.claims().cloned().collect::<Vec<_>>();
        let pending_claims = pending
            .branch
            .as_realized()
            .claims()
            .cloned()
            .collect::<Vec<_>>();
        let host = entry.host.as_ref().map(ProjectionlessHostProjection::from);
        let mut roots = active_claims
            .iter()
            .chain(&pending_claims)
            .map(ExactRegistryClaim::entity)
            .collect::<Vec<_>>();
        if let Some(host) = host.as_ref() {
            roots.push(host.host);
            roots.push(host.top_slot);
        }
        roots.sort_by_key(|entity| entity.to_bits());
        roots.dedup();
        Self {
            report: pending
                .ticket
                .as_ref()
                .map(|ticket| ticket.report_identity()),
            commit_identity_is_current,
            admission,
            host,
            host_binding_is_current,
            active_frames: active
                .frames()
                .iter()
                .map(|frame| frame.frame().clone())
                .collect(),
            active_claims,
            pending_frames: pending
                .branch
                .frames()
                .iter()
                .map(|frame| frame.frame().clone())
                .collect(),
            pending_claims,
            pending_has_final_task: pending.final_task.is_some(),
            observed: ProjectionlessObservedProjection::capture(world, roots),
        }
    }
}

impl ProjectionlessObservedProjection {
    fn capture(world: &World, roots: Vec<bevy_ecs::entity::Entity>) -> Self {
        let mut closure = observed_descendant_closure(world, roots)
            .into_iter()
            .collect::<Vec<_>>();
        closure.sort_by_key(|entity| entity.to_bits());
        let registry = world.get_resource::<RouteEntityRegistry>();
        let gate = world.get_resource::<RouterInputGate>();
        let entities = closure
            .into_iter()
            .map(|entity| ProjectionlessObservedEntity {
                entity,
                manifest: crate::registry::ComponentManifest::from_world(world, entity),
                parent: world
                    .get::<bevy_ecs::hierarchy::ChildOf>(entity)
                    .map(bevy_ecs::hierarchy::ChildOf::parent),
                children: world
                    .get::<bevy_ecs::hierarchy::Children>(entity)
                    .map(|children| children.iter().copied().collect())
                    .unwrap_or_default(),
                visibility: world
                    .get::<bevy_camera::visibility::Visibility>(entity)
                    .copied(),
                pickable: world
                    .get::<bevy_picking::Pickable>(entity)
                    .map(|pickable| (pickable.should_block_lower, pickable.is_hoverable)),
                barrier: world
                    .get::<crate::components::RouterInputBarrier>(entity)
                    .copied(),
                registry_claim: registry
                    .and_then(|registry| registry.record(entity))
                    .map(|record| record.claim()),
                registry_manifest: registry
                    .and_then(|registry| registry.record(entity))
                    .map(|record| record.manifest().clone()),
                gate: gate.map(|gate| {
                    (
                        gate.allows_pointer(entity),
                        gate.allows_keyboard(entity),
                        gate.allows_gamepad(entity),
                    )
                }),
                ownership_seal: world.get::<RouteOwnershipSeal>(entity).copied(),
                frame_lifecycle: world.get::<FrameLifecycle>(entity).copied(),
                child_slot_lifecycle: world.get::<ChildSlotLifecycle>(entity).copied(),
                child_slot_authority: world.get::<ChildSlotAuthority>(entity).cloned(),
                provisional_slot_lifecycle: world.get::<ProvisionalSlotLifecycle>(entity).copied(),
                host_registration: world
                    .get::<crate::app_ext::RouterHostRegistration>(entity)
                    .map(|registration| (registration.router(), registration.incarnation())),
            })
            .collect();
        Self {
            entities,
            input_focus: world.get_resource::<InputFocus>().map(InputFocus::get),
            route_focus: world
                .get_resource::<RouteFocusState>()
                .map(RouteFocusState::focused),
        }
    }
}

fn settle_projectionless_fail_close(
    world: &mut World,
    entry: &mut RouterEntry,
    active: RealizedBranch,
    pending: PendingCommit,
    source: ProjectionlessFailCloseSource,
) -> Result<(), (RealizedBranch, PendingCommit, ProjectionlessFailCloseSource)> {
    let kind = match source.refreezes_exactly(world, entry, &active, &pending) {
        Ok(()) => AuthorityProtocolResidualKind::Authority(source.reason),
        Err(evidence) => AuthorityProtocolResidualKind::PostReportSourceDrift(evidence),
    };
    let (isolation, isolation_contradiction) = match prepare_normal_authority_isolation(
        world,
        &active,
        &pending.branch,
        entry.host.as_ref(),
    ) {
        AuthorityIsolationPreparation::Ready(isolation) => (isolation, None),
        AuthorityIsolationPreparation::RetryableCapacity => {
            return Err((active, pending, source));
        }
        AuthorityIsolationPreparation::PermanentContradiction {
            contradiction,
            fail_closed,
        } => (fail_closed, Some(contradiction)),
    };
    match fail_closed_normal_authority_protocol_prepared(
        world,
        entry,
        active,
        pending,
        kind,
        isolation,
        isolation_contradiction,
    ) {
        Ok(()) => Ok(()),
        Err((active, pending)) => Err((active, pending, source)),
    }
}

fn branch_owner_projection_is_exact(world: &World, branch: &RealizedBranch) -> bool {
    let Some(registry) = world.get_resource::<RouteEntityRegistry>() else {
        return false;
    };
    let claims_are_exact = branch.claims().all(|claim| {
        registry.record(claim.entity()).is_some_and(|record| {
            record.claim() == *claim
                && crate::registry::ComponentManifest::from_world(world, claim.entity())
                    .is_some_and(|actual| {
                        record
                            .manifest()
                            .component_ids()
                            .iter()
                            .all(|component| actual.component_ids().contains(component))
                    })
        }) && cleanup_claim_has_exact_authority(world, claim)
    });
    let frame_relationships_are_exact = branch.frames().iter().all(|frame| {
        frame.cleanup_members().all(|(claim, parent)| {
            world
                .get::<bevy_ecs::hierarchy::ChildOf>(claim.entity())
                .map(bevy_ecs::hierarchy::ChildOf::parent)
                == Some(parent)
        })
    });
    let slot_relationships_are_exact = branch.child_slots().iter().all(|slot| {
        world
            .get::<bevy_ecs::hierarchy::ChildOf>(slot.slot())
            .map(bevy_ecs::hierarchy::ChildOf::parent)
            == Some(slot.anchor())
    });
    claims_are_exact && frame_relationships_are_exact && slot_relationships_are_exact
}

fn projectionless_fallback_preflight(
    world: &World,
    entry: &RouterEntry,
    pending: &PendingCommit,
) -> ProjectionlessFallbackPreflight {
    let missing_resources = RequiredResourceSet::missing_from(world);
    let active = entry.active.as_ref();
    let task = pending.final_task.as_ref();
    let host = entry.host.as_ref();
    let current_loss = if !missing_resources.is_empty() {
        Some(LifecyclePreflightEvidence::new(
            LifecyclePreflightSource::RequiredResources(missing_resources),
            LifecyclePreflightExpectation::ResourcePresent,
            LifecyclePreflightObserved::Missing,
        ))
    } else if active.is_none() || host.is_none() {
        Some(LifecyclePreflightEvidence::new(
            LifecyclePreflightSource::HostBinding,
            LifecyclePreflightExpectation::ExactAuthority,
            LifecyclePreflightObserved::Missing,
        ))
    } else if active.is_some_and(|active| active.frames().is_empty())
        || host.is_some_and(|host| !current_host_binding_is_exact(world, host))
    {
        Some(LifecyclePreflightEvidence::new(
            LifecyclePreflightSource::HostBinding,
            LifecyclePreflightExpectation::ExactAuthority,
            LifecyclePreflightObserved::Drifted,
        ))
    } else {
        None
    };
    if !missing_resources.is_empty() {
        return ProjectionlessFallbackPreflight::AuthorityLost(
            current_loss.expect("missing Resource must retain typed authority evidence"),
        );
    }
    let (Some(active), Some(task), Some(host)) = (active, task, host) else {
        return ProjectionlessFallbackPreflight::AuthorityLost(current_loss.unwrap_or(
            LifecyclePreflightEvidence::new(
                LifecyclePreflightSource::PendingAdmission,
                LifecyclePreflightExpectation::ExactTaskShape,
                LifecyclePreflightObserved::Missing,
            ),
        ));
    };
    let mount_target = task.mount_target();
    let admission_rejection = (pending.ticket.is_some()
        || pending.branch.frames().len() != 1
        || !pending.branch.child_slots().is_empty()
        || task.frame().origin() != &bevy_ui_router_core::prelude::FrameOrigin::RouterFallback
        || pending.branch.frames()[0].frame() != task.frame()
        || !matches!(
            &mount_target,
            FrameMountTarget::TopSlot { host: task_host, slot }
                if task_host == &host.version && slot == &host.slot_id
        ))
    .then(|| {
        LifecyclePreflightEvidence::new(
            LifecyclePreflightSource::PendingAdmission,
            LifecyclePreflightExpectation::ExactTaskShape,
            LifecyclePreflightObserved::Drifted,
        )
    });
    let entering = pending.branch.as_realized();
    let entering_boundaries = entering
        .claims()
        .map(ExactRegistryClaim::entity)
        .collect::<Vec<_>>();
    let entering_is_valid = validate_prepared_branch(world, entering).is_ok();
    let entering_closure = validate_branch_descendants(world, entering);
    let active_local = validate_active_branch_local(world, active, &entering_boundaries);
    let active_closure = (active_local.is_ok() && entering_is_valid)
        .then(|| validate_active_branch_with_entering(world, active, Some(entering), None));
    let current_loss = current_loss
        .or_else(|| {
            (!branch_owner_projection_is_exact(world, active)).then(|| {
                LifecyclePreflightEvidence::new(
                    LifecyclePreflightSource::PreservedCurrent,
                    LifecyclePreflightExpectation::ExactAuthority,
                    LifecyclePreflightObserved::Drifted,
                )
            })
        })
        .or_else(|| {
            active_local.as_ref().err().map(|_| {
                LifecyclePreflightEvidence::new(
                    LifecyclePreflightSource::PreservedCurrent,
                    LifecyclePreflightExpectation::ExactRelationshipClosure,
                    LifecyclePreflightObserved::Drifted,
                )
            })
        });
    let cross_role_is_valid = active_closure
        .as_ref()
        .is_some_and(|closure| closure.is_ok());
    let pending_rejection = admission_rejection
        .or_else(|| {
            (!entering_is_valid).then(|| {
                LifecyclePreflightEvidence::new(
                    LifecyclePreflightSource::PendingEntering,
                    LifecyclePreflightExpectation::ExactAuthority,
                    LifecyclePreflightObserved::Drifted,
                )
            })
        })
        .or_else(|| {
            entering_closure.as_ref().err().map(|_| {
                LifecyclePreflightEvidence::new(
                    LifecyclePreflightSource::PendingEntering,
                    LifecyclePreflightExpectation::ExactRelationshipClosure,
                    LifecyclePreflightObserved::Drifted,
                )
            })
        })
        .or_else(|| {
            entering.prepare_root_rebase(active).is_none().then(|| {
                LifecyclePreflightEvidence::new(
                    LifecyclePreflightSource::CrossRoleRebase,
                    LifecyclePreflightExpectation::RebaseCompatible,
                    LifecyclePreflightObserved::Contradictory,
                )
            })
        })
        .or_else(|| {
            (!cross_role_is_valid).then(|| {
                LifecyclePreflightEvidence::new(
                    LifecyclePreflightSource::CrossRoleRebase,
                    LifecyclePreflightExpectation::RebaseCompatible,
                    LifecyclePreflightObserved::Contradictory,
                )
            })
        })
        .or_else(|| {
            active_closure
                .as_ref()
                .and_then(|closure| closure.as_ref().ok())
                .zip(entering_closure.as_ref().ok())
                .is_some_and(|(active, entering)| {
                    active.iter().any(|entity| entering.contains(entity))
                })
                .then(|| {
                    LifecyclePreflightEvidence::new(
                        LifecyclePreflightSource::TargetPartition,
                        LifecyclePreflightExpectation::DisjointClosure,
                        LifecyclePreflightObserved::Contradictory,
                    )
                })
        });
    if let Some(evidence) = current_loss {
        return ProjectionlessFallbackPreflight::AuthorityLost(evidence);
    }
    if let Some(evidence) = pending_rejection {
        return ProjectionlessFallbackPreflight::PendingRejected(evidence);
    }
    ProjectionlessFallbackPreflight::Ready {
        admission: ProjectionlessFallbackAdmission {
            frame: task.frame().clone(),
            mount_target,
            top_slot: host.top_slot,
            top_slot_claim: host.top_slot_claim.clone(),
        },
        focus_leaving: active_closure
            .expect("both local sides must produce one coupled closure")
            .expect("cross-role closure was independently validated"),
    }
}

fn reject_projectionless_fallback_pending(
    entry: &mut RouterEntry,
    mut pending: PendingCommit,
    message: impl Into<String>,
) {
    let message = message.into();
    let task = pending
        .final_task
        .take()
        .expect("projectionless fallback rejection retains its exact task");
    entry.pending_failed_branch = Some(PendingFailedBranch {
        terminal: PendingFailedBranchTerminal::TaskWithPhysicalResidual {
            task,
            failure: ViewFailureKind::Apply,
            message,
        },
        branch: failed_prepared_branch(pending, None),
    });
}

fn fail_closed_projectionless_replacement_contradiction_with_evidence(
    world: &mut World,
    entry: &mut RouterEntry,
    pending: PendingCommit,
    evidence: LifecyclePreflightEvidence,
) -> Result<(), PendingCommit> {
    let active = entry
        .active
        .take()
        .expect("projectionless fallback contradiction retains existing active");
    match fail_closed_normal_owner_protocol(
        world,
        entry,
        active,
        pending,
        AuthorityProtocolResidualKind::PreflightAuthorityLost(evidence),
    ) {
        Ok(()) => Ok(()),
        Err((active, pending)) => {
            entry.active = Some(active);
            Err(pending)
        }
    }
}

fn fail_closed_projectionless_replacement_contradiction(
    world: &mut World,
    entry: &mut RouterEntry,
    pending: PendingCommit,
) -> Result<(), PendingCommit> {
    fail_closed_projectionless_replacement_contradiction_with_evidence(
        world,
        entry,
        pending,
        LifecyclePreflightEvidence::new(
            LifecyclePreflightSource::LeavingCurrent,
            LifecyclePreflightExpectation::ExactAuthority,
            LifecyclePreflightObserved::Contradictory,
        ),
    )
}

struct ProjectionlessPublicationWork {
    pending: PendingCommit,
    admission: Option<ProjectionlessFallbackAdmission>,
    host: Option<ProjectionlessHostProjection>,
    focus_leaving: Vec<Entity>,
    gate_before: Vec<Entity>,
    gate_after: Vec<Entity>,
    targets: Vec<crate::presentation::FrameMutationTarget>,
    isolate_count: usize,
    transitions: Vec<(ExactRegistryClaim, EntityLifecycle)>,
    resulting_claims: Vec<ExactRegistryClaim>,
    entering_rebind: crate::branch_transaction::PreparedBranchClaimRebind,
    leaving_rebind: Option<crate::branch_transaction::PreparedBranchClaimRebind>,
    expected_manifest: FrozenCleanupManifest,
    reserved_ledger: PreparedCleanupLedger,
    logical_roots: Vec<RouteFrameVersion>,
    manifest_digest: u64,
    obligation_count: usize,
    reject_prepare_commit: bool,
}

#[cfg(test)]
fn commit_projectionless_branch(
    world: &mut World,
    entry: &mut RouterEntry,
    pending: PendingCommit,
) -> Result<(), PendingCommit> {
    let mut work = None;
    let result = prepare_projectionless_branch(world, entry, pending, &mut work);
    assert!(
        work.is_none(),
        "this rejection-only helper cannot publish through a resource scope"
    );
    result
}

fn prepare_projectionless_branch(
    world: &mut World,
    entry: &mut RouterEntry,
    mut pending: PendingCommit,
    work: &mut Option<ProjectionlessPublicationWork>,
) -> Result<(), PendingCommit> {
    if pending.ticket.is_some()
        || pending.final_task.is_none()
        || pending.branch.frames().is_empty()
    {
        return Err(pending);
    }
    let task_is_current = pending
        .final_task
        .as_ref()
        .is_some_and(|task| entry.engine.inspect_report_identity(task).is_ok());
    if !task_is_current {
        return Err(pending);
    }
    let replaces_active = entry.active.is_some();
    if replaces_active
        && pending.final_task.as_ref().is_none_or(|task| {
            task.frame().origin() != &bevy_ui_router_core::prelude::FrameOrigin::RouterFallback
        })
    {
        return Err(pending);
    }
    let missing = RequiredResourceSet::missing_from(world);
    if !replaces_active && !missing.is_empty() {
        let evidence = LifecyclePreflightEvidence::new(
            LifecyclePreflightSource::RequiredResources(missing),
            LifecyclePreflightExpectation::ResourcePresent,
            LifecyclePreflightObserved::Missing,
        );
        return match fail_closed_normal_owner_protocol(
            world,
            entry,
            RealizedBranch::empty(),
            pending,
            AuthorityProtocolResidualKind::PreflightAuthorityLost(evidence),
        ) {
            Ok(()) => Ok(()),
            Err((active, pending)) => {
                entry.active = (!active.frames().is_empty()).then_some(active);
                Err(pending)
            }
        };
    }
    let (admission, focus_leaving) = if replaces_active {
        match projectionless_fallback_preflight(world, entry, &pending) {
            ProjectionlessFallbackPreflight::Ready {
                admission,
                focus_leaving,
            } => (Some(admission), focus_leaving),
            ProjectionlessFallbackPreflight::PendingRejected(evidence) => {
                reject_projectionless_fallback_pending(entry, pending, evidence.pending_message());
                return Ok(());
            }
            ProjectionlessFallbackPreflight::AuthorityLost(evidence) => {
                return fail_closed_projectionless_replacement_contradiction_with_evidence(
                    world, entry, pending, evidence,
                );
            }
        }
    } else {
        (None, Vec::new())
    };
    #[cfg(feature = "bsn")]
    if let Some(active) = entry.active.as_mut()
        && let Err(error) = active.prepare_native_cleanup(world)
    {
        if error.is_retryable() {
            return Err(pending);
        }
        let active = entry
            .active
            .take()
            .expect("the leaving native owner is retained");
        return match fail_closed_normal_owner_protocol(
            world,
            entry,
            active,
            pending,
            AuthorityProtocolResidualKind::NativeCleanup(error),
        ) {
            Ok(()) => Ok(()),
            Err((active, pending)) => {
                entry.active = Some(active);
                Err(pending)
            }
        };
    }
    let gate_before = entry
        .active
        .as_ref()
        .map(|branch| branch.live_interactive_entities(world))
        .unwrap_or_default();
    let mut targets = match entry
        .active
        .as_ref()
        .map(|leaving| leaving.active_sparse_leaving_mutation_targets(EntityLifecycle::Leaving))
    {
        Some(Some(targets)) => targets,
        Some(None) => {
            return fail_closed_projectionless_replacement_contradiction_with_evidence(
                world,
                entry,
                pending,
                LifecyclePreflightEvidence::new(
                    LifecyclePreflightSource::LeavingCurrent,
                    LifecyclePreflightExpectation::ExactAuthority,
                    LifecyclePreflightObserved::Contradictory,
                ),
            );
        }
        None => Vec::new(),
    };
    let isolate_count = targets.len();
    targets.extend(
        pending
            .branch
            .as_realized()
            .entering_mutation_targets(EntityLifecycle::PreparedInert, EntityLifecycle::Active),
    );
    let gate_after: Vec<_> = pending
        .branch
        .frames()
        .iter()
        .flat_map(|frame| frame.content_input_facts(world, &targets).1)
        .collect();
    let mut transitions = entry
        .active
        .as_ref()
        .map(|leaving| leaving.lifecycle_transitions(EntityLifecycle::Leaving, true))
        .unwrap_or_default();
    transitions.extend(
        pending
            .branch
            .as_realized()
            .lifecycle_transitions(EntityLifecycle::Active, false),
    );
    let logical_roots = entry
        .active
        .as_ref()
        .map(logical_cleanup_roots)
        .unwrap_or_default();
    let leaving_transition_count = entry
        .active
        .as_ref()
        .map(RealizedBranch::obligation_count)
        .unwrap_or_default();

    let prevalidated = {
        let registry = world.resource::<RouteEntityRegistry>();
        let leaving_exact = registry
            .prepare_lifecycle_mutation(&transitions[..leaving_transition_count])
            .is_some();
        let entering_exact = registry
            .prepare_lifecycle_mutation(&transitions[leaving_transition_count..])
            .is_some();
        let combined = registry.prepare_lifecycle_mutation(&transitions);
        (leaving_exact, entering_exact, combined)
    };
    if replaces_active && !prevalidated.0 {
        return fail_closed_projectionless_replacement_contradiction(world, entry, pending);
    }
    if !prevalidated.1 {
        if replaces_active {
            reject_projectionless_fallback_pending(
                entry,
                pending,
                "projectionless fallback entering registry drifted",
            );
            return Ok(());
        }
        return Err(pending);
    }
    let Some(registry_plan) = prevalidated.2 else {
        return if replaces_active {
            fail_closed_projectionless_replacement_contradiction(world, entry, pending)
        } else {
            Err(pending)
        };
    };
    let resulting_claims = registry_plan.resulting_claims().to_vec();
    drop(registry_plan);

    let entering_claims = pending
        .branch
        .as_realized()
        .claims()
        .map(|current| {
            resulting_claims
                .iter()
                .find(|next| next.entity() == current.entity())
                .cloned()
        })
        .collect::<Option<Vec<_>>>();
    let entering_rebind = entering_claims.as_ref().and_then(|claims| {
        pending.branch.as_realized().prepare_claim_rebind(
            claims,
            EntityLifecycle::PreparedInert,
            EntityLifecycle::Active,
        )
    });
    let Some(entering_rebind) = entering_rebind else {
        if replaces_active {
            reject_projectionless_fallback_pending(
                entry,
                pending,
                "projectionless fallback entering claim drifted",
            );
            return Ok(());
        }
        return Err(pending);
    };
    let leaving_rebind = entry.active.as_ref().map(|leaving| {
        let claims = leaving
            .claims()
            .map(|current| {
                resulting_claims
                    .iter()
                    .find(|next| next.entity() == current.entity())
                    .cloned()
            })
            .collect::<Option<Vec<_>>>()?;
        leaving.prepare_active_sparse_claim_rebind(&claims, EntityLifecycle::Leaving)
    });
    if matches!(leaving_rebind, Some(None)) {
        return fail_closed_projectionless_replacement_contradiction(world, entry, pending);
    }
    let leaving_rebind = leaving_rebind.flatten();

    let frozen_manifest = match entry.active.as_ref() {
        Some(leaving) => FrozenCleanupManifest::from_leaving_branch(
            world,
            world.resource::<RouteEntityRegistry>(),
            leaving,
            &resulting_claims,
            Some(pending.branch.as_realized()),
        ),
        None => Some(FrozenCleanupManifest::empty()),
    };
    let Some(frozen_manifest) = frozen_manifest else {
        return fail_closed_projectionless_replacement_contradiction(world, entry, pending);
    };
    let manifest_digest = frozen_manifest.digest();
    let obligation_count = frozen_manifest.len();

    if prepare_input_gate_mutation(
        world.resource_mut::<RouterInputGate>(),
        &gate_before,
        Vec::new(),
    )
    .is_none()
    {
        return if replaces_active {
            fail_closed_projectionless_replacement_contradiction(world, entry, pending)
        } else {
            Err(pending)
        };
    }
    if prepare_input_gate_mutation(
        world.resource_mut::<RouterInputGate>(),
        &gate_before,
        gate_after.clone(),
    )
    .is_none()
    {
        if replaces_active {
            reject_projectionless_fallback_pending(
                entry,
                pending,
                "projectionless fallback entering input authority drifted",
            );
            return Ok(());
        }
        return Err(pending);
    }
    if prepare_presentation_mutation(world, &targets[..isolate_count]).is_none() {
        return if replaces_active {
            fail_closed_projectionless_replacement_contradiction(world, entry, pending)
        } else {
            Err(pending)
        };
    }
    if prepare_presentation_mutation(world, &targets[isolate_count..]).is_none() {
        if replaces_active {
            reject_projectionless_fallback_pending(
                entry,
                pending,
                "projectionless fallback entering presentation drifted",
            );
            return Ok(());
        }
        return Err(pending);
    }
    if prepare_presentation_mutation(world, &targets)
        .and_then(|prepared| prepared.seal_commit(isolate_count))
        .is_none()
    {
        return if replaces_active {
            fail_closed_projectionless_replacement_contradiction(world, entry, pending)
        } else {
            Err(pending)
        };
    }

    let expected_manifest = frozen_manifest.clone();
    let reserved_ledger = match CleanupLedger::reserve(frozen_manifest) {
        Ok(reserved) => reserved,
        Err(error) => match error.kind() {
            CleanupLedgerReserveErrorKind::RetryableCapacity => return Err(pending),
            kind @ (CleanupLedgerReserveErrorKind::DomainExhausted
            | CleanupLedgerReserveErrorKind::UnfinalizedBoundaryFacts) => {
                let _returned_manifest = error.into_manifest();
                return if replaces_active {
                    let active = entry
                        .active
                        .take()
                        .expect("projectionless replacement retains active owner");
                    match fail_closed_normal_owner_protocol(
                        world,
                        entry,
                        active,
                        pending,
                        AuthorityProtocolResidualKind::CleanupLedger(kind),
                    ) {
                        Ok(()) => Ok(()),
                        Err((active, pending)) => {
                            entry.active = Some(active);
                            Err(pending)
                        }
                    }
                } else {
                    fail_closed_projectionless_owner_protocol(world, entry, pending, kind)
                };
            }
        },
    };
    let reject_prepare_commit = inject_projectionless_prepare_commit_rejection(world);
    *work = Some(ProjectionlessPublicationWork {
        pending,
        admission,
        host: entry.host.as_ref().map(ProjectionlessHostProjection::from),
        focus_leaving,
        gate_before,
        gate_after,
        targets,
        isolate_count,
        transitions,
        resulting_claims,
        entering_rebind,
        leaving_rebind,
        expected_manifest,
        reserved_ledger,
        logical_roots,
        manifest_digest,
        obligation_count,
        reject_prepare_commit,
    });
    Ok(())
}

fn publish_projectionless_work(
    world: &mut World,
    handle: RouterHandle,
    work: ProjectionlessPublicationWork,
) {
    let ProjectionlessPublicationWork {
        mut pending,
        admission,
        host,
        focus_leaving,
        gate_before,
        gate_after,
        targets,
        isolate_count,
        transitions,
        resulting_claims,
        entering_rebind,
        leaving_rebind,
        expected_manifest,
        reserved_ledger,
        logical_roots,
        manifest_digest,
        obligation_count,
        reject_prepare_commit,
    } = work;
    let preflight = {
        let navigation = world.resource::<RouterNavigation>();
        let entry = navigation
            .entries
            .iter()
            .find(|entry| entry.handle == handle)
            .unwrap();
        if admission.is_some() {
            Some(projectionless_fallback_preflight(world, entry, &pending))
        } else {
            let missing = RequiredResourceSet::missing_from(world);
            if !missing.is_empty() {
                Some(ProjectionlessFallbackPreflight::AuthorityLost(
                    LifecyclePreflightEvidence::new(
                        LifecyclePreflightSource::RequiredResources(missing),
                        LifecyclePreflightExpectation::ResourcePresent,
                        LifecyclePreflightObserved::Missing,
                    ),
                ))
            } else if entry.host.as_ref().map(ProjectionlessHostProjection::from) != host
                || entry
                    .host
                    .as_ref()
                    .is_none_or(|host| !current_host_binding_is_exact(world, host))
                || entry.active.is_some()
            {
                Some(ProjectionlessFallbackPreflight::AuthorityLost(
                    LifecyclePreflightEvidence::new(
                        LifecyclePreflightSource::HostBinding,
                        LifecyclePreflightExpectation::ExactAuthority,
                        LifecyclePreflightObserved::Drifted,
                    ),
                ))
            } else if validate_prepared_branch(world, pending.branch.as_realized()).is_err()
                || validate_branch_descendants(world, pending.branch.as_realized()).is_err()
            {
                Some(ProjectionlessFallbackPreflight::PendingRejected(
                    LifecyclePreflightEvidence::new(
                        LifecyclePreflightSource::PendingEntering,
                        LifecyclePreflightExpectation::ExactRelationshipClosure,
                        LifecyclePreflightObserved::Drifted,
                    ),
                ))
            } else {
                None
            }
        }
    };
    let evidence = match preflight {
        Some(ProjectionlessFallbackPreflight::Ready {
            admission: current,
            focus_leaving: current_focus,
        }) if Some(&current) == admission.as_ref() && current_focus == focus_leaving => None,
        Some(ProjectionlessFallbackPreflight::PendingRejected(evidence)) => Some((false, evidence)),
        Some(ProjectionlessFallbackPreflight::AuthorityLost(evidence)) => Some((true, evidence)),
        Some(ProjectionlessFallbackPreflight::Ready { .. }) => Some((
            true,
            LifecyclePreflightEvidence::new(
                LifecyclePreflightSource::TargetPartition,
                LifecyclePreflightExpectation::ExactAuthority,
                LifecyclePreflightObserved::Drifted,
            ),
        )),
        None => None,
    };
    if let Some((authority, evidence)) = evidence {
        world.resource_scope(
            |world, mut navigation: bevy_ecs::change_detection::Mut<RouterNavigation>| {
                let entry = navigation
                    .entries
                    .iter_mut()
                    .find(|entry| entry.handle == handle)
                    .unwrap();
                if authority {
                    if entry.active.is_none() {
                        settle_prepublication_authority_loss(
                            world,
                            entry,
                            RealizedBranch::empty(),
                            pending,
                            false,
                            evidence,
                            true,
                        );
                    } else if let Err(pending) =
                        fail_closed_projectionless_replacement_contradiction_with_evidence(
                            world, entry, pending, evidence,
                        )
                    {
                        entry.pending_commit = Some(pending);
                    }
                } else {
                    reject_projectionless_fallback_pending(
                        entry,
                        pending,
                        evidence.pending_message(),
                    );
                }
            },
        );
        return;
    }
    let frozen_is_exact = (|| {
        let navigation = world.get_resource::<RouterNavigation>()?;
        let entry = navigation
            .entries
            .iter()
            .find(|entry| entry.handle == handle)?;
        let registry = world.get_resource::<RouteEntityRegistry>()?;
        entry
            .engine
            .inspect_report_identity(pending.final_task.as_ref()?)
            .ok()?;
        let current = registry.prepare_lifecycle_mutation(&transitions)?;
        if current.resulting_claims() != resulting_claims {
            return None;
        }
        let frozen = match entry.active.as_ref() {
            Some(leaving) => FrozenCleanupManifest::from_leaving_branch(
                world,
                registry,
                leaving,
                &resulting_claims,
                Some(pending.branch.as_realized()),
            )?,
            None => FrozenCleanupManifest::empty(),
        };
        (frozen == expected_manifest).then_some(())
    })()
    .is_some();
    #[cfg(feature = "bsn")]
    let native_focus =
        crate::native_focus::prepare_publication(world, pending.branch.frames(), &gate_after);
    let pointer_targets = world
        .get_resource_mut::<RouterInputGate>()
        .and_then(|gate| prepare_input_gate_mutation(gate, &gate_before, gate_after.clone()))
        .map(|plan| plan.revoked_targets().to_vec());
    let scroll_eligible = crate::scroll::eligible_containers(world);
    let scroll_observation = {
        let navigation = world.resource::<RouterNavigation>();
        let entry = navigation
            .entries
            .iter()
            .find(|entry| entry.handle == handle)
            .unwrap();
        crate::scroll::ScrollObservation::capture(
            world,
            entry.active.iter().flat_map(|branch| branch.frames()),
            pending.branch.frames().iter(),
            &scroll_eligible,
        )
    };
    let scroll_targets = scroll_observation.target_entities();
    let mut branch_owners = None;
    let mut failure = None;
    let committed = (|| {
        if !frozen_is_exact {
            return None;
        }
        let (presentation, resources) =
            crate::presentation::prepare_publication_access_with_scroll(
                world,
                &targets,
                pointer_targets.as_ref()?,
                &scroll_targets,
            )?;
        let crate::presentation::PublicationResources {
            mut navigation,
            mut registry,
            gate,
            input_focus,
            route_focus,
        } = resources;
        let entry = navigation
            .entries
            .iter_mut()
            .find(|entry| entry.handle == handle)?;
        let registry_plan = registry.prepare_lifecycle_mutation(&transitions)?;
        let gate_plan = prepare_input_gate_mutation(gate, &gate_before, gate_after)?;
        let focus_plan = prepare_focus_mutation(input_focus, route_focus, &focus_leaving);
        #[cfg(feature = "bsn")]
        let focus_plan = focus_plan.with_native_publication(native_focus.as_ref()?)?;
        let presentation_plan = presentation.seal_commit(isolate_count)?;
        let root_rebase = match entry.active.as_ref() {
            Some(leaving) => Some(pending.branch.as_realized().prepare_root_rebase(leaving)?),
            None => None,
        };
        let leaving = entry.active.take();
        let entering =
            std::mem::replace(&mut pending.branch, PreparedBranch::empty()).into_realized();
        branch_owners = Some((leaving, entering));
        let task = pending
            .final_task
            .take()
            .expect("projectionless fallback task remains owned");
        let ticket = entry
            .engine
            .report_applied(task)
            .expect("preflighted fallback task remains reportable");
        pending.ticket = Some(ticket);
        let mut manifest_logical_roots = logical_roots.clone();
        if reject_prepare_commit {
            let duplicate = manifest_logical_roots
                .first()
                .expect("replacement rejection fixture has one leaving root")
                .clone();
            manifest_logical_roots.push(duplicate);
        }
        let prepared = match entry.engine.prepare_commit(
            pending
                .ticket
                .as_ref()
                .expect("reported fallback task owns a commit ticket"),
            CleanupManifest::new(manifest_logical_roots, manifest_digest, obligation_count),
        ) {
            Ok(prepared) => prepared,
            Err(_) => {
                failure = Some(ProjectionlessCommitFailure::post_report(
                    admission.clone(),
                    BeginAuthoritySettlementErrorKind::InvalidManifest,
                ));
                return None;
            }
        };
        let engine = match entry.engine.validate_publication(prepared) {
            Ok(validated) => validated,
            Err(error) => {
                failure = Some(ProjectionlessCommitFailure::post_report(
                    admission.clone(),
                    match error {
                        IntegrationError::InvalidManifest => {
                            BeginAuthoritySettlementErrorKind::InvalidManifest
                        }
                        IntegrationError::Exhausted
                        | IntegrationError::AwaitingHost
                        | IntegrationError::OutstandingAdmission
                        | IntegrationError::AuthorityExists
                        | IntegrationError::ForeignRouter
                        | IntegrationError::Duplicate
                        | IntegrationError::Stale
                        | IntegrationError::Invalid => BeginAuthoritySettlementErrorKind::Stale,
                    },
                ));
                return None;
            }
        };
        let scroll = entry
            .scroll
            .prepare(engine.history_publication(), scroll_observation);
        let presentation_plan = presentation_plan.with_scroll(&scroll.writes);
        let cleanup = engine.cleanup().clone();
        let cleanup_ledger = reserved_ledger.bind_batch(cleanup);
        pending.ticket.take();
        let (mut leaving, mut entering) = branch_owners
            .take()
            .expect("fully prepared projectionless commit retains its branch owners");
        if let (Some(leaving), Some(rebind)) = (leaving.as_mut(), leaving_rebind) {
            leaving.apply_claim_rebind(rebind);
        }
        entering.apply_claim_rebind(entering_rebind);
        if let Some(rebase) = root_rebase {
            entering.apply_root_rebase(rebase);
        }
        entering.consume_native_focus_owners();
        let next_active = Some(entering);
        let next_cleanup = Some(CleanupLedger::activate_leaving_branch(
            cleanup_ledger,
            leaving,
        ));
        let active_owner = &mut entry.active;
        let cleanup_owner = &mut entry.cleanup_ledger;
        let scroll_owner = &mut entry.scroll;
        engine.publish(|| {
            presentation_plan.apply_staged(
                |_| focus_plan.apply(),
                |_| registry_plan.apply(&mut registry),
                |_| gate_plan.apply(),
            );
            *active_owner = next_active;
            *cleanup_owner = next_cleanup;
            *scroll_owner = scroll.state;
        });
        Some(())
    })();
    if committed.is_some() {
        return;
    }
    world.resource_scope(
        |world, mut navigation: bevy_ecs::change_detection::Mut<RouterNavigation>| {
            let entry = navigation
                .entries
                .iter_mut()
                .find(|entry| entry.handle == handle)
                .unwrap();
            if branch_owners.is_none() {
                let result = if entry.active.is_some() {
                    fail_closed_projectionless_replacement_contradiction(world, entry, pending)
                } else {
                    reject_projectionless_fallback_pending(
                        entry,
                        pending,
                        "initial fallback final preparation rejected",
                    );
                    Ok(())
                };
                if let Err(pending) = result {
                    entry.pending_commit = Some(pending);
                }
                return;
            }
            let result = (|| {
                let Some(()) = committed else {
                    let (leaving, entering) = branch_owners
                        .take()
                        .expect("failed projectionless commit retains its branch owners");
                    pending.branch = PreparedBranch::from_realized(entering);
                    return match failure
                        .expect("projectionless commit returns None only with typed failure")
                    {
                        ProjectionlessCommitFailure::InitialPostReportAuthorityLost { reason } => {
                            match fail_closed_normal_owner_protocol(
                                world,
                                entry,
                                leaving.unwrap_or_else(RealizedBranch::empty),
                                pending,
                                AuthorityProtocolResidualKind::Authority(reason),
                            ) {
                                Ok(()) => Ok(()),
                                Err((active, pending)) => {
                                    entry.active = (!active.frames().is_empty()).then_some(active);
                                    Err(pending)
                                }
                            }
                        }
                        ProjectionlessCommitFailure::PostReportAuthorityLost {
                            admission,
                            reason,
                        } => {
                            let active = leaving.expect(
                                "projectionless replacement continuation retains active owner",
                            );
                            let source = ProjectionlessFailCloseSource::freeze(
                                world, entry, admission, reason, &active, &pending,
                            );
                            match settle_projectionless_fail_close(
                                world, entry, active, pending, source,
                            ) {
                                Ok(()) => Ok(()),
                                Err((active, pending, source)) => {
                                    entry.active = Some(active);
                                    entry.pending_commit = Some(pending);
                                    entry.projectionless_fail_close = Some(
                                        ProjectionlessFailCloseContinuation::Retryable(source),
                                    );
                                    Ok(())
                                }
                            }
                        }
                    };
                };
                Ok(())
            })();
            if let Err(pending) = result {
                entry.pending_commit = Some(pending);
            }
        },
    );
}

fn focus_targets_for_commit(
    replaces_active: bool,
    active_descendants: Vec<bevy_ecs::entity::Entity>,
) -> Vec<bevy_ecs::entity::Entity> {
    if replaces_active {
        active_descendants
    } else {
        Vec::new()
    }
}

fn logical_cleanup_roots(branch: &RealizedBranch) -> Vec<RouteFrameVersion> {
    branch
        .frames()
        .iter()
        .rev()
        .map(|frame| frame.frame().clone())
        .collect()
}

fn authority_settlement_logical_roots(
    pending: &PreparedBranch,
    active: &RealizedBranch,
) -> Vec<RouteFrameVersion> {
    let mut roots = pending
        .frames()
        .iter()
        .rev()
        .chain(active.frames().iter().rev())
        .map(|frame| frame.frame().clone())
        .collect::<Vec<_>>();
    let mut unique = Vec::with_capacity(roots.len());
    for frame in roots.drain(..) {
        if !unique.contains(&frame) {
            unique.push(frame);
        }
    }
    roots = unique;
    roots
}

#[derive(Debug, Eq, PartialEq)]
enum FailedBoundaryDisposition {
    CurrentAuthorityLoss(FailedBoundaryDrift),
    PendingOnlyDrift(FailedBoundaryDrift),
    AmbiguousInvalidTopology(FailedBoundaryDrift),
}

enum FailedManifestDisposition {
    Ready {
        failed: PendingFailedBranch,
        manifest: FrozenCleanupManifest,
    },
    CurrentAuthorityLoss {
        failed: PendingFailedBranch,
        manifest: FrozenCleanupManifest,
        bindings: Vec<FailedAuthorityBoundaryBinding>,
    },
    AmbiguousInvalidTopology {
        failed: PendingFailedBranch,
        manifest: FrozenCleanupManifest,
    },
}

fn exact_failed_source_obligation<'a>(
    drift: &FailedBoundaryDrift,
    failed: &'a FailedPreparedBranch,
) -> Option<&'a ExactRegistryClaim> {
    let expected = drift.expected.exact_claim();
    match &drift.source {
        FailedBoundarySource::FrameMountSlot {
            frame,
            removed_child,
        } => {
            let mut matches = failed
                .frames()
                .iter()
                .chain(failed.detached_frames())
                .filter(|candidate| {
                    candidate.frame() == frame
                        && candidate.shell() == *removed_child
                        && candidate.mount_binding().slot_claim() == expected
                });
            let candidate = matches.next()?;
            if matches.next().is_some() {
                return None;
            }
            candidate.claims().find(|claim| {
                claim.entity() == *removed_child
                    && matches!(
                        claim.owner(),
                        RegistryOwner::Committed(EntityOwner::FrameShell(owner))
                            if owner == frame
                    )
            })
        }
        FailedBoundarySource::FramePhysicalParent {
            frame,
            mount_slot,
            removed_child,
        } => {
            let mut matches = failed
                .frames()
                .iter()
                .chain(failed.detached_frames())
                .filter(|candidate| {
                    let (physical_parent, physical_claim) =
                        candidate.mount_binding().physical_parent();
                    candidate.frame() == frame
                        && candidate.shell() == *removed_child
                        && candidate.mount_binding().slot() == *mount_slot
                        && physical_parent == drift.expected.physical_entity()
                        && candidate
                            .mount_binding()
                            .outlet_binding()
                            .is_some_and(|binding| drift.expected.matches_outlet_binding(binding))
                        && physical_claim == Some(expected)
                });
            let candidate = matches.next()?;
            if matches.next().is_some() {
                return None;
            }
            candidate.claims().find(|claim| {
                claim.entity() == *removed_child
                    && matches!(
                        claim.owner(),
                        RegistryOwner::Committed(EntityOwner::FrameShell(owner))
                            if owner == frame
                    )
            })
        }
        FailedBoundarySource::MountedSlotAnchor {
            parent,
            outlet,
            slot,
            removed_child,
        } => {
            let mut matches = failed.child_slots().iter().filter(|candidate| {
                candidate.parent() == parent
                    && candidate.outlet() == outlet
                    && candidate.slot_id() == *slot
                    && candidate.slot() == *removed_child
                    && candidate.anchor_claim() == expected
                    && candidate.anchor() == drift.expected.physical_entity()
                    && drift
                        .expected
                        .matches_outlet_binding(candidate.anchor_binding())
            });
            let candidate = matches.next()?;
            if matches.next().is_some() {
                return None;
            }
            Some(candidate.slot_claim())
        }
        FailedBoundarySource::ResidualAnchor {
            parent,
            outlet,
            slot,
            removed_child,
        } => {
            let mut matches = failed.mount_residuals().iter().filter(|candidate| {
                candidate.parent() == parent
                    && candidate.outlet() == outlet
                    && candidate.entity() == *removed_child
                    && candidate.anchor() == drift.expected.physical_entity()
                    && drift
                        .expected
                        .matches_outlet_binding(candidate.anchor_binding())
                    && candidate.anchor_claim() == expected
                    && matches!(
                        candidate.claim().owner(),
                        RegistryOwner::Committed(EntityOwner::ChildSlot {
                            parent: owner_parent,
                            outlet: owner_outlet,
                            slot: owner_slot,
                        }) | RegistryOwner::Provisional(ProvisionalOwner::ChildSlot {
                            parent: owner_parent,
                            outlet: owner_outlet,
                            slot: owner_slot,
                        }) if owner_parent == parent
                            && owner_outlet == outlet
                            && *owner_slot == *slot
                    )
            });
            let candidate = matches.next()?;
            if matches.next().is_some() {
                return None;
            }
            Some(candidate.claim())
        }
        FailedBoundarySource::RejectedAnchor {
            parent,
            outlet,
            slot,
            removed_child,
        } => {
            let mut matches = failed.rejected_child_slots().iter().filter(|rejected| {
                let candidate = rejected.slot();
                candidate.slot() == *removed_child
                    && rejected.canonical_anchor() == Some(drift.expected.physical_entity())
                    && rejected
                        .canonical_anchor_binding()
                        .is_some_and(|binding| drift.expected.matches_outlet_binding(binding))
                    && rejected.canonical_anchor_claim() == Some(expected)
                    && matches!(
                        candidate.slot_claim().owner(),
                        RegistryOwner::Committed(EntityOwner::ChildSlot {
                            parent: owner_parent,
                            outlet: owner_outlet,
                            slot: owner_slot,
                        }) if owner_parent == parent
                            && owner_outlet == outlet
                            && *owner_slot == *slot
                    )
            });
            let candidate = matches.next()?;
            if matches.next().is_some() {
                return None;
            }
            Some(candidate.slot().slot_claim())
        }
    }
}

fn realized_branch_owns_exact_boundary(
    branch: &RealizedBranch,
    drift: &FailedBoundaryDrift,
) -> bool {
    if let crate::cleanup::FailedBoundaryExpectation::OutletBinding { parent, binding } =
        &drift.expected
    {
        return branch.frames().iter().any(|frame| {
            frame.frame() == parent
                && frame.claims().any(|claim| claim == binding.source_claim())
                && frame
                    .outlets()
                    .iter()
                    .any(|outlet| outlet.anchor_binding() == binding)
        });
    }
    let claim = drift.expected.exact_claim();
    match claim.owner() {
        RegistryOwner::Committed(EntityOwner::ChildSlot {
            parent,
            outlet,
            slot,
        }) => branch.child_slots().iter().any(|candidate| {
            candidate.slot_claim() == claim
                && candidate.parent() == parent
                && candidate.outlet() == outlet
                && candidate.slot_id() == *slot
        }),
        RegistryOwner::Committed(EntityOwner::FrameOutput {
            frame, local_index, ..
        }) => branch.frames().iter().any(|candidate| {
            candidate.frame() == frame
                && candidate.claims().any(|candidate| candidate == claim)
                && match &drift.source {
                    FailedBoundarySource::FramePhysicalParent { .. } => true,
                    FailedBoundarySource::MountedSlotAnchor { parent, outlet, .. }
                    | FailedBoundarySource::ResidualAnchor { parent, outlet, .. }
                    | FailedBoundarySource::RejectedAnchor { parent, outlet, .. } => {
                        parent == frame
                            && candidate.outlet(outlet).is_some_and(|binding| {
                                binding.anchor() == claim.entity()
                                    && binding.anchor_claim() == claim
                                    && matches!(
                                        claim.owner(),
                                        RegistryOwner::Committed(EntityOwner::FrameOutput {
                                            frame: output_frame,
                                            local_index: output_index,
                                            ..
                                        }) if output_frame == frame
                                            && output_index == local_index
                                    )
                            })
                    }
                    FailedBoundarySource::FrameMountSlot { .. } => false,
                }
        }),
        _ => false,
    }
}

fn failed_branch_owns_exact_boundary(
    failed: &FailedPreparedBranch,
    drift: &FailedBoundaryDrift,
) -> bool {
    if let crate::cleanup::FailedBoundaryExpectation::OutletBinding { parent, binding } =
        &drift.expected
    {
        return failed
            .frames()
            .iter()
            .chain(failed.detached_frames())
            .any(|frame| {
                frame.frame() == parent
                    && frame.claims().any(|claim| claim == binding.source_claim())
                    && frame
                        .outlets()
                        .iter()
                        .any(|outlet| outlet.anchor_binding() == binding)
            });
    }
    let claim = drift.expected.exact_claim();
    match claim.owner() {
        RegistryOwner::Committed(EntityOwner::FrameOutput {
            frame, local_index, ..
        }) => failed
            .frames()
            .iter()
            .chain(failed.detached_frames())
            .any(|candidate| {
                candidate.frame() == frame
                    && candidate.claims().any(|candidate| candidate == claim)
                    && match &drift.source {
                        FailedBoundarySource::FramePhysicalParent { .. } => true,
                        FailedBoundarySource::MountedSlotAnchor { parent, outlet, .. }
                        | FailedBoundarySource::ResidualAnchor { parent, outlet, .. }
                        | FailedBoundarySource::RejectedAnchor { parent, outlet, .. } => {
                            parent == frame
                                && candidate.outlet(outlet).is_some_and(|binding| {
                                    binding.anchor_claim() == claim
                                        && binding.anchor() == claim.entity()
                                        && matches!(
                                            claim.owner(),
                                            RegistryOwner::Committed(EntityOwner::FrameOutput {
                                                frame: output_frame,
                                                local_index: output_index,
                                                ..
                                            }) if output_frame == frame
                                                && output_index == local_index
                                        )
                                })
                        }
                        FailedBoundarySource::FrameMountSlot { .. } => false,
                    }
            }),
        RegistryOwner::Committed(EntityOwner::ChildSlot {
            parent,
            outlet,
            slot,
        })
        | RegistryOwner::Provisional(ProvisionalOwner::ChildSlot {
            parent,
            outlet,
            slot,
        }) => {
            let exact_primary = failed
                .child_slots()
                .iter()
                .map(|slot| slot.slot_claim())
                .chain(failed.mount_residuals().iter().map(|slot| slot.claim()))
                .chain(
                    failed
                        .rejected_child_slots()
                        .iter()
                        .map(|slot| slot.slot().slot_claim()),
                )
                .any(|candidate| candidate == claim);
            exact_primary
                || failed
                    .frames()
                    .iter()
                    .chain(failed.detached_frames())
                    .any(|candidate| {
                        candidate.frame() == parent
                            && candidate.outlet(outlet).is_some()
                            && matches!(
                                &drift.source,
                                FailedBoundarySource::FrameMountSlot { frame, .. }
                                    if failed
                                        .frames()
                                        .iter()
                                        .chain(failed.detached_frames())
                                        .any(|child| child.frame() == frame
                                            && child.mount_binding().slot_claim() == claim
                                            && child.frame().slot() == *slot)
                            )
                    })
        }
        _ => false,
    }
}

fn failed_boundary_source_accepts_owner(drift: &FailedBoundaryDrift) -> bool {
    use crate::registry::{EntityOwner, RegistryOwner};

    if let crate::cleanup::FailedBoundaryExpectation::OutletBinding { parent, binding } =
        &drift.expected
    {
        return binding.is_native()
            && matches!(binding.source_claim().owner(), RegistryOwner::Committed(EntityOwner::FrameContentTarget(frame)) if frame == parent)
            && match &drift.source {
                // The failed-source receiver separately checks this child's
                // exact shell, mount slot, anchor and native binding.
                FailedBoundarySource::FramePhysicalParent { .. } => true,
                FailedBoundarySource::MountedSlotAnchor { parent: source, .. }
                | FailedBoundarySource::ResidualAnchor { parent: source, .. }
                | FailedBoundarySource::RejectedAnchor { parent: source, .. } => source == parent,
                FailedBoundarySource::FrameMountSlot { .. } => false,
            };
    }

    match (&drift.source, drift.expected.exact_claim().owner()) {
        (
            FailedBoundarySource::FrameMountSlot { .. },
            RegistryOwner::Committed(
                EntityOwner::HostTopSlot { .. } | EntityOwner::ChildSlot { .. },
            ),
        ) => true,
        (
            FailedBoundarySource::FrameMountSlot { .. },
            RegistryOwner::Provisional(crate::registry::ProvisionalOwner::ChildSlot { .. }),
        ) => true,
        (
            FailedBoundarySource::FramePhysicalParent { .. }
            | FailedBoundarySource::MountedSlotAnchor { .. }
            | FailedBoundarySource::ResidualAnchor { .. }
            | FailedBoundarySource::RejectedAnchor { .. },
            RegistryOwner::Committed(EntityOwner::FrameOutput { .. }),
        ) => true,
        _ => false,
    }
}

fn current_host_owns_failed_boundary(
    host: Option<&HostBinding>,
    claim: &ExactRegistryClaim,
) -> bool {
    host.is_some_and(|host| {
        host.top_slot_claim == *claim
            && matches!(
                claim.owner(),
                crate::registry::RegistryOwner::Committed(
                    crate::registry::EntityOwner::HostTopSlot { host: version, slot }
                ) if version == &host.version && *slot == host.slot_id
            )
            && claim.entity() == host.top_slot
    })
}

fn classify_failed_boundary(
    drift: FailedBoundaryDrift,
    failed: &FailedPreparedBranch,
    active: Option<&RealizedBranch>,
    host: Option<&HostBinding>,
) -> FailedBoundaryDisposition {
    let Some(_local_obligation) = exact_failed_source_obligation(&drift, failed) else {
        return FailedBoundaryDisposition::AmbiguousInvalidTopology(drift);
    };
    if !failed_boundary_source_accepts_owner(&drift) {
        return FailedBoundaryDisposition::AmbiguousInvalidTopology(drift);
    }
    let claim = drift.expected.exact_claim();
    let failed_owns = failed_branch_owns_exact_boundary(failed, &drift);
    let current_owns = current_host_owns_failed_boundary(host, claim)
        || active.is_some_and(|active| realized_branch_owns_exact_boundary(active, &drift));
    match (current_owns, failed_owns) {
        (true, false) => FailedBoundaryDisposition::CurrentAuthorityLoss(drift),
        (false, true) => FailedBoundaryDisposition::PendingOnlyDrift(drift),
        (true, true) | (false, false) => FailedBoundaryDisposition::AmbiguousInvalidTopology(drift),
    }
}

/// A native task may fail before allocating any output, while its preserved
/// mount still belongs to current. Bind this observation to the original task
/// and Core projection; it must not manufacture a failed physical obligation.
#[cfg(feature = "bsn")]
fn exact_current_task_outlet<'a>(
    engine: &RouterEngine,
    failed: &PendingFailedBranch,
    active: Option<&'a RealizedBranch>,
) -> Option<&'a crate::mount::MountedChildSlot> {
    use bevy_ui_router_core::integration::ReconciliationEnteringSlotKind;
    let PendingFailedBranchTerminal::Task { task, .. } = &failed.terminal else {
        return None;
    };
    let failure = failed.branch.native_failure()?;
    if failure.source != crate::presentation::native_failure::NativeFailureSource::from_task(task)
        || engine.inspect_report_identity(task).is_err()
        || !failed_branch_primary_claims(&failed.branch).is_empty()
    {
        return None;
    }
    let FrameMountTarget::ChildOutlet { parent, outlet } = task.mount_target() else {
        return None;
    };
    let projection = engine.reconciliation_projection_for_task(task).ok()?;
    let projected_parent = projection
        .preserved()
        .find(|frame| frame.version() == &parent)?;
    if !projection.entering().any(|frame| {
        frame.frame() == task.frame().frame()
            && frame.generation() == task.frame().generation()
            && frame.origin() == task.frame().origin()
            && frame.branch_index().checked_sub(1) == Some(projected_parent.branch_index())
            && frame.slot() == ReconciliationEnteringSlotKind::Existing(task.frame().slot())
    }) || !projection.child_slots().any(|role| {
        role.binding().is_some_and(|binding| {
            binding.parent_branch_index() == projected_parent.branch_index()
                && binding.parent() == &parent
                && binding.outlet() == &outlet
                && binding.slot() == task.frame().slot()
        })
    }) {
        return None;
    }
    let active = active?;
    let mut parents = active
        .frames()
        .iter()
        .filter(|frame| frame.frame() == &parent);
    let parent_frame = parents.next()?;
    if parents.next().is_some() {
        return None;
    }
    let mut slots = active.child_slots().iter().filter(|slot| {
        slot.parent() == &parent
            && slot.outlet() == &outlet
            && slot.slot_id() == task.frame().slot()
    });
    let slot = slots.next()?;
    if slots.next().is_some()
        || !slot.anchor_binding().is_native()
        || !parent_frame
            .claims()
            .any(|claim| claim == slot.anchor_claim())
        || parent_frame
            .outlet(&outlet)
            .is_none_or(|declared| declared.anchor_binding() != slot.anchor_binding())
        || !matches!(slot.slot_claim().owner(), RegistryOwner::Committed(EntityOwner::ChildSlot { parent: owner, outlet: key, slot: identity })
            if owner == &parent && key == &outlet && *identity == slot.slot_id())
    {
        return None;
    }
    Some(slot)
}

#[cfg(feature = "bsn")]
fn current_task_outlet_matches_drift(
    slot: &crate::mount::MountedChildSlot,
    drift: &FailedBoundaryDrift,
) -> bool {
    matches!(&drift.source, FailedBoundarySource::MountedSlotAnchor { parent, outlet, slot: identity, removed_child }
        if parent == slot.parent() && outlet == slot.outlet() && *identity == slot.slot_id() && *removed_child == slot.slot())
        && drift.expected.physical_entity() == slot.anchor()
        && drift.expected.exact_claim() == slot.anchor_claim()
        && drift.expected.matches_outlet_binding(slot.anchor_binding())
        && failed_boundary_source_accepts_owner(drift)
}

fn classify_failed_manifest(
    world: &World,
    engine: &RouterEngine,
    manifest: FrozenCleanupManifest,
    failed: PendingFailedBranch,
    active: Option<&RealizedBranch>,
    host: Option<&HostBinding>,
) -> FailedManifestDisposition {
    #[cfg(feature = "bsn")]
    let current_task_outlet = exact_current_task_outlet(engine, &failed, active);
    #[cfg(not(feature = "bsn"))]
    let _ = engine;
    let dispositions = manifest
        .boundary_drifts()
        .iter()
        .cloned()
        .map(|drift| {
            let binding =
                exact_failed_source_obligation(&drift, &failed.branch).map(|obligation| {
                    PendingBoundaryBinding {
                        source: drift.source.clone(),
                        obligation: obligation.clone(),
                    }
                });
            #[cfg(feature = "bsn")]
            if current_task_outlet
                .is_some_and(|slot| current_task_outlet_matches_drift(slot, &drift))
                && active.is_some_and(|active| realized_branch_owns_exact_boundary(active, &drift))
                && !failed_branch_owns_exact_boundary(&failed.branch, &drift)
            {
                return (FailedBoundaryDisposition::CurrentAuthorityLoss(drift), None);
            }
            (
                classify_failed_boundary(drift, &failed.branch, active, host),
                binding,
            )
        })
        .collect::<Vec<_>>();
    if dispositions.iter().any(|disposition| {
        matches!(
            disposition.0,
            FailedBoundaryDisposition::AmbiguousInvalidTopology(_)
        )
    }) {
        return FailedManifestDisposition::AmbiguousInvalidTopology { failed, manifest };
    }
    // Physical mount claims can remain exact after the application removes its
    // host registration. Classify that authority source before consuming the
    // original failed task into ordinary recovery/fallback.
    let host_lost = host.is_some_and(|host| {
        precommit_authority_cause(world, host) == PreCommitAuthorityCause::HostLost
            && !current_host_binding_is_exact(world, host)
    });
    if host_lost
        || dispositions.iter().any(|disposition| {
            matches!(
                disposition.0,
                FailedBoundaryDisposition::CurrentAuthorityLoss(_)
            )
        })
    {
        let bindings = dispositions
            .into_iter()
            .map(|(disposition, pending)| match disposition {
                FailedBoundaryDisposition::CurrentAuthorityLoss(drift) => {
                    FailedAuthorityBoundaryBinding::Current {
                        source: drift.source,
                        authority: drift.expected.exact_claim().clone(),
                    }
                }
                FailedBoundaryDisposition::PendingOnlyDrift(_) => {
                    FailedAuthorityBoundaryBinding::Pending(
                        pending.expect("classified pending boundary retains its local obligation"),
                    )
                }
                FailedBoundaryDisposition::AmbiguousInvalidTopology(_) => {
                    unreachable!("ambiguous failed boundary returned before authority binding")
                }
            })
            .collect();
        return FailedManifestDisposition::CurrentAuthorityLoss {
            failed,
            manifest,
            bindings,
        };
    }
    let bindings = dispositions
        .into_iter()
        .filter_map(|(_, binding)| binding)
        .collect();
    match manifest.finalize_pending_only_boundaries(bindings) {
        Ok(manifest) => FailedManifestDisposition::Ready { failed, manifest },
        Err((_, manifest)) => {
            FailedManifestDisposition::AmbiguousInvalidTopology { failed, manifest }
        }
    }
}

enum FailedAuthoritySettlementBeginOutcome {
    Begun,
    Retryable(PendingFailedBranch),
    TriggerContradiction {
        failed: PendingFailedBranch,
        kind: FailedAuthorityTriggerError,
    },
    ProtocolContradiction {
        failed: PendingFailedBranch,
        kind: BeginAuthoritySettlementErrorKind,
    },
    IsolationContradiction {
        failed: PendingFailedBranch,
        contradiction: AuthorityIsolationContradiction,
        isolation: PreparedAuthorityIsolation,
    },
    LedgerContradiction {
        failed: PendingFailedBranch,
        kind: CleanupLedgerReserveErrorKind,
    },
    #[cfg(feature = "bsn")]
    NativeCleanupContradiction {
        failed: PendingFailedBranch,
        error: crate::presentation::native_frame::NativeCleanupPreparationError,
    },
}

fn failed_branch_logical_roots(branch: &FailedPreparedBranch) -> Vec<RouteFrameVersion> {
    let mut roots = Vec::new();
    for frame in branch.frames().iter().rev() {
        if !roots.contains(frame.frame()) {
            roots.push(frame.frame().clone());
        }
    }
    roots
}

fn failed_authority_logical_roots(
    failed: &PendingFailedBranch,
    manifest: &FrozenCleanupManifest,
) -> Vec<RouteFrameVersion> {
    match &failed.terminal {
        PendingFailedBranchTerminal::Request(request)
        | PendingFailedBranchTerminal::BackTargetPendingRejected { request, .. } => {
            request.logical_roots().to_vec()
        }
        PendingFailedBranchTerminal::RegistrationBegin(_) => {
            failed_branch_logical_roots(&failed.branch)
        }
        PendingFailedBranchTerminal::Task { task, .. }
        | PendingFailedBranchTerminal::TaskWithPhysicalResidual { task, .. } => {
            let mut current = failed_branch_logical_roots(&failed.branch);
            if manifest.contains_frame_obligation(task.frame()) {
                current.retain(|frame| frame != task.frame());
                current.insert(0, task.frame().clone());
            }
            current
        }
    }
}

fn mint_failed_authority_trigger(
    world: &mut World,
    engine: &mut RouterEngine,
    terminal: &PendingFailedBranchTerminal,
) -> Result<FailedAuthoritySettlementTrigger, FailedAuthorityTriggerError> {
    #[cfg(test)]
    if world
        .get_resource_mut::<FailedAuthorityTriggerExhaustion>()
        .is_some_and(|mut fault| {
            if fault.remaining == 0 {
                return false;
            }
            fault.remaining -= 1;
            true
        })
    {
        return Err(FailedAuthorityTriggerError::Exhausted);
    }
    #[cfg(not(test))]
    let _ = world;
    match terminal {
        PendingFailedBranchTerminal::Task { task, .. }
        | PendingFailedBranchTerminal::TaskWithPhysicalResidual { task, .. } => {
            engine.failed_authority_trigger_for_task(task)
        }
        PendingFailedBranchTerminal::Request(request)
        | PendingFailedBranchTerminal::BackTargetPendingRejected { request, .. } => {
            engine.failed_authority_trigger_for_cleanup(request)
        }
        PendingFailedBranchTerminal::RegistrationBegin(error) => {
            engine.failed_authority_trigger_for_registration_begin(error)
        }
    }
}

fn begin_failed_authority_settlement(
    world: &mut World,
    entry: &mut RouterEntry,
    failed: PendingFailedBranch,
    manifest: FrozenCleanupManifest,
    bindings: Vec<FailedAuthorityBoundaryBinding>,
) -> FailedAuthoritySettlementBeginOutcome {
    let Some(host) = entry.host.as_ref() else {
        return FailedAuthoritySettlementBeginOutcome::ProtocolContradiction {
            failed,
            kind: BeginAuthoritySettlementErrorKind::Stale,
        };
    };
    let cause = precommit_authority_cause(world, host);
    #[cfg(feature = "bsn")]
    if let Some(active) = entry.active.as_mut()
        && let Err(error) = active.prepare_native_cleanup(world)
    {
        return if error.is_retryable() {
            FailedAuthoritySettlementBeginOutcome::Retryable(failed)
        } else {
            FailedAuthoritySettlementBeginOutcome::NativeCleanupContradiction { failed, error }
        };
    }
    let frozen = {
        let registry = world.resource::<RouteEntityRegistry>();
        match manifest.finalize_failed_authority_settlement(
            world,
            registry,
            entry.active.as_ref(),
            host,
            bindings,
        ) {
            Ok(frozen) => frozen,
            Err(AuthoritySettlementManifestError::RetryableCapacity) => {
                return FailedAuthoritySettlementBeginOutcome::Retryable(failed);
            }
            Err(
                AuthoritySettlementManifestError::InvalidAuthority
                | AuthoritySettlementManifestError::InvalidTopology,
            ) => {
                return FailedAuthoritySettlementBeginOutcome::ProtocolContradiction {
                    failed,
                    kind: BeginAuthoritySettlementErrorKind::InvalidManifest,
                };
            }
        }
    };
    let mut logical_roots = failed_authority_logical_roots(&failed, &frozen);
    let manifest_digest = frozen.digest();
    let obligation_count = frozen.len();
    let reserved = match CleanupLedger::reserve(frozen) {
        Ok(reserved) => reserved,
        Err(error) => match error.kind() {
            CleanupLedgerReserveErrorKind::RetryableCapacity => {
                return FailedAuthoritySettlementBeginOutcome::Retryable(failed);
            }
            kind @ (CleanupLedgerReserveErrorKind::DomainExhausted
            | CleanupLedgerReserveErrorKind::UnfinalizedBoundaryFacts) => {
                let _returned_manifest = error.into_manifest();
                return FailedAuthoritySettlementBeginOutcome::LedgerContradiction { failed, kind };
            }
        },
    };
    let host_children = world
        .get::<bevy_ecs::hierarchy::Children>(host.host)
        .map(|children| children.iter().copied().collect::<Vec<_>>())
        .unwrap_or_default();
    let top_slot_parent = world
        .get::<bevy_ecs::hierarchy::ChildOf>(host.top_slot)
        .map(bevy_ecs::hierarchy::ChildOf::parent);
    let top_slot_memberships = host_children
        .iter()
        .filter(|child| **child == host.top_slot)
        .count();
    if (top_slot_parent == Some(host.host) && top_slot_memberships != 1)
        || (top_slot_parent != Some(host.host) && top_slot_memberships != 0)
    {
        return FailedAuthoritySettlementBeginOutcome::ProtocolContradiction {
            failed,
            kind: BeginAuthoritySettlementErrorKind::InvalidManifest,
        };
    }
    let isolation = match prepare_failed_manifest_protocol_isolation(
        world,
        entry.active.as_ref(),
        &failed,
        entry.host.as_ref(),
    ) {
        AuthorityIsolationPreparation::Ready(isolation) => isolation,
        AuthorityIsolationPreparation::RetryableCapacity => {
            return FailedAuthoritySettlementBeginOutcome::Retryable(failed);
        }
        AuthorityIsolationPreparation::PermanentContradiction {
            contradiction,
            fail_closed,
        } => {
            return FailedAuthoritySettlementBeginOutcome::IsolationContradiction {
                failed,
                contradiction,
                isolation: fail_closed,
            };
        }
    };
    let trigger = match mint_failed_authority_trigger(world, &mut entry.engine, &failed.terminal) {
        Ok(trigger) => trigger,
        Err(kind) => {
            return FailedAuthoritySettlementBeginOutcome::TriggerContradiction { failed, kind };
        }
    };
    inject_failed_authority_begin_manifest_rejection(world, &mut logical_roots);

    let (retired_host, lease) = entry
        .host
        .take()
        .expect("failed authority publication retains its exact host")
        .into_retired();
    let active = entry.active.take();
    let mut settlement_owner = Some((reserved, active, failed, retired_host));
    let engine = &mut entry.engine;
    let cleanup_ledger = &mut entry.cleanup_ledger;
    match engine.begin_failed_authority_settlement_for_cause_retaining(
        lease,
        trigger,
        cause,
        CleanupManifest::new(logical_roots, manifest_digest, obligation_count),
        |batch| {
            isolation.apply(world);
            let (reserved, active, failed, retired_host) = settlement_owner
                .take()
                .expect("failed authority publication consumes its composed owner once");
            *cleanup_ledger = Some(CleanupLedger::activate_failed_authority_settlement(
                reserved.bind_batch(batch),
                active,
                failed,
                retired_host,
                host_children,
            ));
        },
    ) {
        Ok(_) => FailedAuthoritySettlementBeginOutcome::Begun,
        Err(error) => {
            let kind = error.kind();
            let (reserved, active, failed, retired_host) = settlement_owner
                .take()
                .expect("rejected failed authority publication returns its composed owner");
            drop(reserved);
            entry.host = Some(retired_host.restore(error.into_owner()));
            entry.active = active;
            match kind {
                BeginAuthoritySettlementErrorKind::RetryablePreparation => {
                    FailedAuthoritySettlementBeginOutcome::Retryable(failed)
                }
                BeginAuthoritySettlementErrorKind::Foreign
                | BeginAuthoritySettlementErrorKind::Stale
                | BeginAuthoritySettlementErrorKind::WrongPhase
                | BeginAuthoritySettlementErrorKind::InvalidManifest => {
                    FailedAuthoritySettlementBeginOutcome::ProtocolContradiction { failed, kind }
                }
            }
        }
    }
}

fn retain_installed_back_no_target_authority_cleanup(
    entry: &mut RouterEntry,
    source: FrozenBackNoTargetAuthoritySource,
    retirement: BackCapacityRetirementEvidence,
    ledger: CleanupLedger,
    batch: BackNoTargetAuthorityCleanupBatchOwner,
    inert: Option<BackNoTargetAuthorityInertEvidence>,
) {
    entry.pending_back_no_target_authority_cleanup = Some(
        PendingBackNoTargetAuthorityCleanup::Installed(InstalledBackNoTargetAuthorityCleanup {
            source,
            retirement,
            ledger: BackNoTargetAuthorityLedgerHolder::Active(ledger),
            batch: Some(batch),
            inert,
        }),
    );
}

fn retain_back_no_target_authority_completion_inert(
    pending: &mut Option<PendingBackNoTargetAuthorityCleanup>,
    completion: BackNoTargetAuthorityCleanupCompletionOwner,
    mismatch: BackNoTargetAuthorityCleanupMismatchEvidence,
    current: Option<PendingBackNoTargetAuthorityCleanup>,
) {
    let evidence = if current.is_none() {
        BackNoTargetAuthorityCompletionInertEvidence::MissingCompletingHolder
    } else {
        BackNoTargetAuthorityCompletionInertEvidence::WrongPhase
    };
    let physical = current.map(|current| match current {
        PendingBackNoTargetAuthorityCleanup::Installed(installed) => {
            BackNoTargetAuthorityCompletionInertPhysical::Installed(installed)
        }
        PendingBackNoTargetAuthorityCleanup::Completing(completing) => {
            BackNoTargetAuthorityCompletionInertPhysical::Completing(completing)
        }
        PendingBackNoTargetAuthorityCleanup::Settled(settled) => {
            BackNoTargetAuthorityCompletionInertPhysical::Settled(settled)
        }
        PendingBackNoTargetAuthorityCleanup::AuthorityBeginInert(inert) => {
            BackNoTargetAuthorityCompletionInertPhysical::AuthorityBeginInert(inert)
        }
        PendingBackNoTargetAuthorityCleanup::CompletionInert(inert) => {
            BackNoTargetAuthorityCompletionInertPhysical::PriorCompletionInert(Box::new(inert))
        }
    });
    *pending = Some(PendingBackNoTargetAuthorityCleanup::CompletionInert(
        BackNoTargetAuthorityCompletionInert {
            completion,
            mismatch,
            physical,
            evidence,
        },
    ));
}

fn drive_back_no_target_authority_cleanup(world: &mut World, entry: &mut RouterEntry) -> bool {
    match entry.pending_back_no_target_authority_cleanup.as_ref() {
        None => return false,
        Some(PendingBackNoTargetAuthorityCleanup::Installed(installed))
            if installed.inert.is_none() && installed.batch.is_some() => {}
        Some(
            PendingBackNoTargetAuthorityCleanup::Installed(_)
            | PendingBackNoTargetAuthorityCleanup::Completing(_)
            | PendingBackNoTargetAuthorityCleanup::Settled(_)
            | PendingBackNoTargetAuthorityCleanup::AuthorityBeginInert(_)
            | PendingBackNoTargetAuthorityCleanup::CompletionInert(_),
        ) => return true,
    }
    let current = entry.pending_back_no_target_authority_cleanup.take();
    let installed = match current {
        Some(PendingBackNoTargetAuthorityCleanup::Installed(installed)) => installed,
        current => {
            entry.pending_back_no_target_authority_cleanup = current;
            return true;
        }
    };
    let InstalledBackNoTargetAuthorityCleanup {
        source,
        retirement,
        ledger,
        batch,
        inert: _,
    } = installed;
    let Some(batch) = batch else {
        entry.pending_back_no_target_authority_cleanup = Some(
            PendingBackNoTargetAuthorityCleanup::Installed(InstalledBackNoTargetAuthorityCleanup {
                source,
                retirement,
                ledger,
                batch: None,
                inert: Some(BackNoTargetAuthorityInertEvidence::MissingBatch),
            }),
        );
        return true;
    };
    let mut ledger = match ledger {
        BackNoTargetAuthorityLedgerHolder::Prepared(ledger) => ledger.into_active(),
        BackNoTargetAuthorityLedgerHolder::Active(ledger) => ledger,
    };
    if source.active_frames.is_empty()
        || source.active_claims.is_empty()
        || !ledger.back_no_target_authority_partition_is_current(world)
    {
        retain_installed_back_no_target_authority_cleanup(
            entry,
            source,
            retirement,
            ledger,
            batch,
            Some(BackNoTargetAuthorityInertEvidence::PartitionDrift),
        );
        return true;
    }
    let receipts_observed = entry.observe_cleanup_receipts(&mut ledger);
    let partition_is_current = ledger.back_no_target_authority_partition_is_current(world);
    if !receipts_observed || !partition_is_current {
        retain_installed_back_no_target_authority_cleanup(
            entry,
            source,
            retirement,
            ledger,
            batch,
            Some(if !receipts_observed {
                BackNoTargetAuthorityInertEvidence::ReceiptObservationRejected
            } else {
                BackNoTargetAuthorityInertEvidence::PartitionDrift
            }),
        );
        return true;
    }
    if !ledger.is_settled() {
        let Some(attempt) = ledger.attempt(world) else {
            retain_installed_back_no_target_authority_cleanup(
                entry,
                source,
                retirement,
                ledger,
                batch,
                Some(BackNoTargetAuthorityInertEvidence::AttemptUnavailable),
            );
            return true;
        };
        let settled = ledger.settle(world, attempt);
        let receipts_observed = entry.observe_cleanup_receipts(&mut ledger);
        let partition_is_current = ledger.back_no_target_authority_partition_is_current(world);
        if !receipts_observed || !settled || !partition_is_current {
            let inert = if !receipts_observed {
                BackNoTargetAuthorityInertEvidence::ReceiptObservationRejected
            } else if !settled {
                BackNoTargetAuthorityInertEvidence::SettlementRejected
            } else {
                BackNoTargetAuthorityInertEvidence::PartitionDrift
            };
            retain_installed_back_no_target_authority_cleanup(
                entry,
                source,
                retirement,
                ledger,
                batch,
                Some(inert),
            );
            return true;
        }
    }
    let Some(facts) = ledger.back_no_target_authority_completion_facts() else {
        retain_installed_back_no_target_authority_cleanup(
            entry,
            source,
            retirement,
            ledger,
            batch,
            Some(BackNoTargetAuthorityInertEvidence::CompletionFactsUnavailable),
        );
        return true;
    };
    if !ledger.back_no_target_authority_partition_is_current(world) {
        retain_installed_back_no_target_authority_cleanup(
            entry,
            source,
            retirement,
            ledger,
            batch,
            Some(BackNoTargetAuthorityInertEvidence::PartitionDrift),
        );
        return true;
    }
    let completion = batch.complete(
        facts.manifest_digest(),
        facts.obligation_count(),
        facts.completion_digest(),
        facts.settled_count(),
    );
    entry.pending_back_no_target_authority_cleanup =
        Some(PendingBackNoTargetAuthorityCleanup::Completing(
            PreparedBackNoTargetAuthorityPhysicalCompletion {
                source,
                retirement,
                ledger,
            },
        ));
    let RouterEntry {
        engine,
        pending_back_no_target_authority_cleanup: pending,
        ..
    } = entry;
    let outcome = engine.complete_back_no_target_authority_cleanup(completion, || *pending = None);
    match outcome {
        BackNoTargetAuthorityCleanupCompletionOutcome::Terminal(_) => {}
        BackNoTargetAuthorityCleanupCompletionOutcome::Mismatch { owner, evidence } => {
            let current = pending.take();
            match current {
                Some(PendingBackNoTargetAuthorityCleanup::Completing(prepared)) => {
                    *pending = Some(PendingBackNoTargetAuthorityCleanup::Settled(
                        SettledBackNoTargetAuthorityCleanup {
                            prepared,
                            completion: owner,
                            mismatch: evidence,
                        },
                    ));
                }
                current => retain_back_no_target_authority_completion_inert(
                    pending, owner, evidence, current,
                ),
            }
        }
    }
    true
}

fn drive_back_no_target_zero_completion(entry: &mut RouterEntry) -> bool {
    match entry.pending_back_no_target_completion.as_ref() {
        None => return false,
        Some(PendingBackNoTargetCompletion::Installed(installed)) if installed.inert.is_none() => {}
        Some(
            PendingBackNoTargetCompletion::Installed(_) | PendingBackNoTargetCompletion::Settled(_),
        ) => return true,
    }
    #[cfg(test)]
    let fact_drift = entry.back_no_target_completion_fact_drift.take();
    let RouterEntry {
        engine,
        back_capacity_issuer,
        pending_back_no_target_completion,
        ..
    } = entry;
    let Some(PendingBackNoTargetCompletion::Installed(InstalledBackNoTargetCleanup {
        ledger,
        installation,
        inert: None,
    })) = pending_back_no_target_completion.take()
    else {
        return true;
    };
    let prepared = match ledger.prepare_back_no_target_zero_completion() {
        Ok(prepared) => prepared,
        Err(failure) => {
            let (ledger, evidence) = failure.into_parts();
            *pending_back_no_target_completion = Some(PendingBackNoTargetCompletion::Installed(
                InstalledBackNoTargetCleanup {
                    ledger,
                    installation,
                    inert: Some(BackNoTargetInstalledInertEvidence::LedgerInvariant(
                        evidence,
                    )),
                },
            ));
            return true;
        }
    };

    let source = prepared.source_identity();
    let Some(issuer) = back_capacity_issuer.as_mut() else {
        *pending_back_no_target_completion = Some(PendingBackNoTargetCompletion::Installed(
            InstalledBackNoTargetCleanup {
                ledger: prepared.into_ledger(),
                installation,
                inert: Some(BackNoTargetInstalledInertEvidence::IssuerMissing { source }),
            },
        ));
        return true;
    };
    let guard = match issuer.preflight_installed_back_capacity_reclaim(&prepared) {
        Ok(guard) => guard,
        Err(error) => {
            *pending_back_no_target_completion = Some(PendingBackNoTargetCompletion::Installed(
                InstalledBackNoTargetCleanup {
                    ledger: prepared.into_ledger(),
                    installation,
                    inert: Some(BackNoTargetInstalledInertEvidence::ReclaimRejected {
                        source,
                        error,
                    }),
                },
            ));
            return true;
        }
    };
    let facts = prepared.facts();
    let manifest_digest = facts.manifest_digest();
    let obligation_count = facts.obligation_count();
    let completion_digest = facts.completion_digest();
    let settled_count = facts.settled_count();
    #[cfg(test)]
    let (completion_digest, settled_count) = match fact_drift {
        Some(BackNoTargetCompletionFactDrift::CompletionDigest) => {
            (completion_digest ^ 1, settled_count)
        }
        Some(BackNoTargetCompletionFactDrift::SettledCount) => {
            (completion_digest, settled_count.saturating_add(1))
        }
        None => (completion_digest, settled_count),
    };
    let completion = installation.complete(
        manifest_digest,
        obligation_count,
        completion_digest,
        settled_count,
    );

    match engine.complete_back_no_target_cleanup(completion, move || guard.commit()) {
        BackNoTargetCleanupCompletionOutcome::Terminal(_terminal) => {}
        BackNoTargetCleanupCompletionOutcome::Mismatch { owner, mismatch } => {
            *pending_back_no_target_completion = Some(PendingBackNoTargetCompletion::Settled(
                SettledBackNoTargetCleanup {
                    prepared,
                    completion: owner,
                    mismatch,
                },
            ));
        }
    }
    true
}

fn target_preparation_permanent_prewrite_refreeze(
    world: &World,
    validation: &FrozenTargetPreparationPermanentPhysicalManifest,
) -> Result<(), LifecyclePreflightEvidence> {
    let Some(registry) = world.get_resource::<RouteEntityRegistry>() else {
        return Err(LifecyclePreflightEvidence::new(
            LifecyclePreflightSource::RequiredResources(RequiredResourceSet::ROUTE_ENTITY_REGISTRY),
            LifecyclePreflightExpectation::ResourcePresent,
            LifecyclePreflightObserved::Missing,
        ));
    };
    let target_entities = validation
        .target_observed
        .iter()
        .map(|observed| observed.entity)
        .collect::<Vec<_>>();
    let protected_entities = validation
        .protected
        .observed
        .iter()
        .map(|observed| observed.entity)
        .collect::<Vec<_>>();
    let target_entity_set = target_entities
        .iter()
        .copied()
        .collect::<std::collections::HashSet<_>>();
    let protected_entity_set = protected_entities
        .iter()
        .copied()
        .collect::<std::collections::HashSet<_>>();
    let allowed_write_entity_set = validation
        .allowed_write
        .splice_targets
        .iter()
        .copied()
        .collect::<std::collections::HashSet<_>>();
    if target_entity_set.len() != target_entities.len()
        || protected_entity_set.len() != protected_entities.len()
    {
        return Err(LifecyclePreflightEvidence::new(
            LifecyclePreflightSource::TargetPartition,
            LifecyclePreflightExpectation::ExactTaskShape,
            LifecyclePreflightObserved::Contradictory,
        ));
    }
    let observed_host_descendants = observed_descendant_closure(world, [validation.lineage.host]);
    let mut expected_host_descendants = std::collections::HashSet::new();
    expected_host_descendants.insert(validation.lineage.host);
    expected_host_descendants.extend(target_entities.iter().copied());
    expected_host_descendants.extend(protected_entities.iter().copied());
    if observed_host_descendants != expected_host_descendants {
        return Err(LifecyclePreflightEvidence::new(
            LifecyclePreflightSource::TargetPartition,
            LifecyclePreflightExpectation::ExactRelationshipClosure,
            LifecyclePreflightObserved::Drifted,
        ));
    }
    if !target_preparation_permanent_relationship_closure_is_exact(
        world,
        &observed_host_descendants,
    ) {
        return Err(LifecyclePreflightEvidence::new(
            LifecyclePreflightSource::TargetPartition,
            LifecyclePreflightExpectation::ExactRelationshipClosure,
            LifecyclePreflightObserved::Drifted,
        ));
    }
    let host_edge = &validation.allowed_write.host_edge;
    let observed_host =
        observe_target_preparation_permanent_entity(world, registry, host_edge.host);
    let observed_top_slot =
        observe_target_preparation_permanent_entity(world, registry, host_edge.top_slot);
    let mut expected_host_children_after = Vec::new();
    let mut top_slot_memberships = 0;
    for child in &host_edge.host_children_before {
        if *child == host_edge.top_slot {
            top_slot_memberships += 1;
        } else {
            expected_host_children_after.push(*child);
        }
    }
    if host_edge.host != validation.lineage.host
        || host_edge.top_slot_parent_before != Some(host_edge.host)
        || host_edge.top_slot_parent_after != None
        || top_slot_memberships != 1
        || expected_host_children_after != host_edge.host_children_after
        || observed_host.children != host_edge.host_children_before
        || observed_top_slot.parent != host_edge.top_slot_parent_before
        || !target_preparation_permanent_structure_is_exact(
            &observed_host,
            &host_edge.host_observed,
        )
        || !target_preparation_permanent_authority_values_are_exact(
            &observed_host,
            &host_edge.host_observed,
        )
    {
        return Err(LifecyclePreflightEvidence::new(
            LifecyclePreflightSource::HostBinding,
            LifecyclePreflightExpectation::ExactRelationshipClosure,
            LifecyclePreflightObserved::Drifted,
        ));
    }
    let observed_targets = target_entities
        .iter()
        .copied()
        .map(|entity| observe_target_preparation_permanent_entity(world, registry, entity))
        .collect::<Vec<_>>();
    if observed_targets
        .iter()
        .zip(&validation.target_observed)
        .any(|pair| !target_preparation_permanent_structure_is_exact(pair.0, pair.1))
    {
        return Err(LifecyclePreflightEvidence::new(
            LifecyclePreflightSource::TargetPartition,
            LifecyclePreflightExpectation::ExactRelationshipClosure,
            LifecyclePreflightObserved::Drifted,
        ));
    }
    if observed_targets
        .iter()
        .zip(&validation.target_observed)
        .any(|pair| !target_preparation_permanent_authority_values_are_exact(pair.0, pair.1))
    {
        return Err(LifecyclePreflightEvidence::new(
            LifecyclePreflightSource::TargetPartition,
            LifecyclePreflightExpectation::ExactAuthority,
            LifecyclePreflightObserved::Drifted,
        ));
    }
    let observed_protected = protected_entities
        .iter()
        .copied()
        .map(|entity| observe_target_preparation_permanent_entity(world, registry, entity))
        .collect::<Vec<_>>();
    if observed_protected
        .iter()
        .zip(&validation.protected.observed)
        .any(|pair| !target_preparation_permanent_structure_is_exact(pair.0, pair.1))
    {
        return Err(LifecyclePreflightEvidence::new(
            LifecyclePreflightSource::TargetPartition,
            LifecyclePreflightExpectation::ExactRelationshipClosure,
            LifecyclePreflightObserved::Drifted,
        ));
    }
    if observed_protected
        .iter()
        .zip(&validation.protected.observed)
        .any(|pair| !target_preparation_permanent_authority_values_are_exact(pair.0, pair.1))
    {
        return Err(LifecyclePreflightEvidence::new(
            LifecyclePreflightSource::TargetPartition,
            LifecyclePreflightExpectation::ExactAuthority,
            LifecyclePreflightObserved::Drifted,
        ));
    }
    let subtree_has_target = |entity| {
        observed_descendant_closure(world, [entity])
            .iter()
            .any(|descendant| target_entity_set.contains(descendant))
    };
    let mut observed_protected_roots = Vec::new();
    for observed in &observed_protected {
        if subtree_has_target(observed.entity) {
            continue;
        }
        let is_root = match observed.parent {
            Some(parent) => !protected_entity_set.contains(&parent) || subtree_has_target(parent),
            None => true,
        };
        if is_root {
            observed_protected_roots.push(observed.entity);
        }
    }
    observed_protected_roots.sort_by_key(|entity| entity.to_bits());
    if observed_protected_roots != validation.protected.roots {
        return Err(LifecyclePreflightEvidence::new(
            LifecyclePreflightSource::TargetPartition,
            LifecyclePreflightExpectation::ExactRelationshipClosure,
            LifecyclePreflightObserved::Drifted,
        ));
    }
    if !target_entity_set.is_disjoint(&protected_entity_set) {
        return Err(LifecyclePreflightEvidence::new(
            LifecyclePreflightSource::TargetPartition,
            LifecyclePreflightExpectation::DisjointClosure,
            LifecyclePreflightObserved::Contradictory,
        ));
    }
    if !allowed_write_entity_set.is_disjoint(&protected_entity_set) {
        return Err(LifecyclePreflightEvidence::new(
            LifecyclePreflightSource::TargetPartition,
            LifecyclePreflightExpectation::DisjointClosure,
            LifecyclePreflightObserved::Contradictory,
        ));
    }
    let mut expected_target_entities = validation.allowed_write.splice_targets.clone();
    expected_target_entities.push(validation.allowed_write.host_edge.top_slot);
    expected_target_entities.sort_by_key(|entity| entity.to_bits());
    if target_entities != expected_target_entities {
        return Err(LifecyclePreflightEvidence::new(
            LifecyclePreflightSource::TargetPartition,
            LifecyclePreflightExpectation::ExactTaskShape,
            LifecyclePreflightObserved::Contradictory,
        ));
    }
    Ok(())
}

enum PreparedTargetPreparationPermanentPhysicalLifecycle {
    Frame(bevy_ecs::entity::Entity),
    ChildSlot(bevy_ecs::entity::Entity),
    TopSlot(bevy_ecs::entity::Entity),
}

struct PreparedTargetPreparationPermanentPhysicalMutation {
    lifecycles: Vec<PreparedTargetPreparationPermanentPhysicalLifecycle>,
}

impl PreparedTargetPreparationPermanentPhysicalMutation {
    fn apply(self, world: &mut World) {
        for lifecycle in self.lifecycles {
            match lifecycle {
                PreparedTargetPreparationPermanentPhysicalLifecycle::Frame(entity) => {
                    *world
                        .get_mut::<FrameLifecycle>(entity)
                        .expect("prepared permanent Current frame retains its lifecycle") =
                        FrameLifecycle(EntityLifecycle::Leaving);
                }
                PreparedTargetPreparationPermanentPhysicalLifecycle::ChildSlot(entity) => {
                    *world
                        .get_mut::<ChildSlotLifecycle>(entity)
                        .expect("prepared permanent Current child slot retains its lifecycle") =
                        ChildSlotLifecycle::Leaving;
                }
                PreparedTargetPreparationPermanentPhysicalLifecycle::TopSlot(entity) => {
                    *world
                        .get_mut::<ProvisionalSlotLifecycle>(entity)
                        .expect("prepared permanent Current top slot retains its lifecycle") =
                        ProvisionalSlotLifecycle::Leaving;
                }
            }
        }
    }
}

struct PreparedTargetPreparationPermanentCurrentIsolation {
    registry: PreparedLifecycleMutation,
    rebind: PreparedPermanentIsolationClaimRebind,
    physical: PreparedTargetPreparationPermanentPhysicalMutation,
    isolation: PreparedAuthorityIsolation,
    pickable_repairs: Vec<PreparedTargetPreparationPermanentPickableRepair>,
    projected_claims: Vec<ExactRegistryClaim>,
    expected_validation: FrozenTargetPreparationPermanentPhysicalManifest,
    expected_gate: Option<Vec<(bevy_ecs::entity::Entity, bool)>>,
    expected_input_focus: Option<Option<bevy_ecs::entity::Entity>>,
    expected_route_focus: Option<Option<bevy_ecs::entity::Entity>>,
    expected_top_slot_claim: ExactRegistryClaim,
}

struct PreparedTargetPreparationPermanentPickableRepair {
    entity: bevy_ecs::entity::Entity,
    expected_validation: FrozenTargetPreparationPermanentPhysicalManifest,
    roots: Vec<bevy_ecs::entity::Entity>,
    expected_input: ProjectionlessObservedProjection,
}

enum TargetPreparationPermanentCurrentIsolationApplyError {
    Physical {
        evidence: LifecyclePreflightEvidence,
        validation: FrozenTargetPreparationPermanentPhysicalManifest,
    },
    Repair {
        evidence: TargetPreparationPermanentRepairRefreezeEvidence,
        validation: FrozenTargetPreparationPermanentPhysicalManifest,
    },
}

struct AppliedTargetPreparationPermanentCurrentIsolation {
    projected_claims: Vec<ExactRegistryClaim>,
    expected_validation: FrozenTargetPreparationPermanentPhysicalManifest,
    expected_gate: Option<Vec<(bevy_ecs::entity::Entity, bool)>>,
    expected_input_focus: Option<Option<bevy_ecs::entity::Entity>>,
    expected_route_focus: Option<Option<bevy_ecs::entity::Entity>>,
}

fn target_preparation_permanent_splice(
    cleanup: &TargetPreparationPermanentCleanupOwner,
) -> &SealedReconciliationSplice {
    match &cleanup.branch {
        FrozenTargetPreparationPermanentBranch::Classifier { physical, .. } => &physical.splice,
        FrozenTargetPreparationPermanentBranch::ReadyPlan { physical, .. } => {
            &physical.classified.splice
        }
    }
}

fn target_preparation_permanent_splice_mut(
    cleanup: &mut TargetPreparationPermanentCleanupOwner,
) -> &mut SealedReconciliationSplice {
    match &mut cleanup.branch {
        FrozenTargetPreparationPermanentBranch::Classifier { physical, .. } => &mut physical.splice,
        FrozenTargetPreparationPermanentBranch::ReadyPlan { physical, .. } => {
            &mut physical.classified.splice
        }
    }
}

fn prepare_target_preparation_permanent_physical_mutation(
    validation: &FrozenTargetPreparationPermanentPhysicalManifest,
    projected_claims: &[ExactRegistryClaim],
) -> Option<PreparedTargetPreparationPermanentPhysicalMutation> {
    if validation.target_observed.len() != projected_claims.len() {
        return None;
    }
    let mut lifecycles = Vec::new();
    lifecycles.try_reserve_exact(projected_claims.len()).ok()?;
    for observed in &validation.target_observed {
        let source = observed.registry_claim.as_ref()?;
        let projected = projected_claims
            .iter()
            .find(|claim| claim.entity() == observed.entity)?;
        if source.entity() != projected.entity()
            || source.owner() != projected.owner()
            || source.seal() != projected.seal()
        {
            return None;
        }
        match projected.lifecycle() {
            EntityLifecycle::PreparedInert if projected == source => {}
            EntityLifecycle::Leaving => {
                let lifecycle = match (
                    observed.frame_lifecycle,
                    observed.child_slot_lifecycle,
                    observed.provisional_slot_lifecycle,
                ) {
                    (Some(_), None, None) => {
                        PreparedTargetPreparationPermanentPhysicalLifecycle::Frame(observed.entity)
                    }
                    (None, Some(_), None) => {
                        PreparedTargetPreparationPermanentPhysicalLifecycle::ChildSlot(
                            observed.entity,
                        )
                    }
                    (None, None, Some(_)) => {
                        PreparedTargetPreparationPermanentPhysicalLifecycle::TopSlot(
                            observed.entity,
                        )
                    }
                    _ => return None,
                };
                lifecycles.push(lifecycle);
            }
            _ => return None,
        }
    }
    let leaving_count = projected_claims
        .iter()
        .filter(|claim| claim.lifecycle() == EntityLifecycle::Leaving)
        .count();
    (lifecycles.len() == leaving_count)
        .then_some(PreparedTargetPreparationPermanentPhysicalMutation { lifecycles })
}

fn target_preparation_permanent_expected_isolated_validation(
    source: &FrozenTargetPreparationPermanentPhysicalManifest,
    projected_claims: &[ExactRegistryClaim],
    inserted_pickable: &[bevy_ecs::entity::Entity],
    pickable_component: Option<bevy_ecs::component::ComponentId>,
) -> Option<FrozenTargetPreparationPermanentPhysicalManifest> {
    let mut expected = source.clone();
    for observed in &mut expected.target_observed {
        let projected = projected_claims
            .iter()
            .find(|claim| claim.entity() == observed.entity)?
            .clone();
        observed.registry_claim = Some(projected.clone());
        if inserted_pickable.contains(&observed.entity) {
            let pickable = pickable_component?;
            let manifest = observed.manifest.take()?;
            if manifest.component_ids().contains(&pickable) {
                return None;
            }
            observed.manifest = Some(ComponentManifest::new(
                manifest
                    .component_ids()
                    .iter()
                    .copied()
                    .chain(std::iter::once(pickable)),
            ));
        }
        if projected.lifecycle() != EntityLifecycle::Leaving {
            continue;
        }
        match (
            observed.frame_lifecycle.as_mut(),
            observed.child_slot_lifecycle.as_mut(),
            observed.provisional_slot_lifecycle.as_mut(),
        ) {
            (Some(lifecycle), None, None) => {
                *lifecycle = FrameLifecycle(EntityLifecycle::Leaving);
            }
            (None, Some(lifecycle), None) => {
                *lifecycle = ChildSlotLifecycle::Leaving;
            }
            (None, None, Some(lifecycle)) => {
                *lifecycle = ProvisionalSlotLifecycle::Leaving;
            }
            _ => return None,
        }
    }
    Some(expected)
}

fn target_preparation_permanent_expected_pickable_repair_validation(
    source: &FrozenTargetPreparationPermanentPhysicalManifest,
    entity: bevy_ecs::entity::Entity,
    pickable: bevy_ecs::component::ComponentId,
) -> Option<FrozenTargetPreparationPermanentPhysicalManifest> {
    let mut expected = source.clone();
    let observed = expected
        .target_observed
        .iter_mut()
        .find(|observed| observed.entity == entity)?;
    let manifest = observed.manifest.take()?;
    if manifest.component_ids().contains(&pickable) {
        return None;
    }
    observed.manifest = Some(ComponentManifest::new(
        manifest
            .component_ids()
            .iter()
            .copied()
            .chain(std::iter::once(pickable)),
    ));
    Some(expected)
}

fn target_preparation_permanent_expected_pickable_repair_input(
    mut source: ProjectionlessObservedProjection,
    entity: bevy_ecs::entity::Entity,
    pickable: bevy_ecs::component::ComponentId,
) -> Option<ProjectionlessObservedProjection> {
    let observed = source
        .entities
        .iter_mut()
        .find(|observed| observed.entity == entity)?;
    let manifest = observed.manifest.take()?;
    if manifest.component_ids().contains(&pickable) || observed.pickable.is_some() {
        return None;
    }
    observed.manifest = Some(ComponentManifest::new(
        manifest
            .component_ids()
            .iter()
            .copied()
            .chain(std::iter::once(pickable)),
    ));
    observed.pickable = Some((
        bevy_picking::Pickable::IGNORE.should_block_lower,
        bevy_picking::Pickable::IGNORE.is_hoverable,
    ));
    Some(source)
}

fn target_preparation_permanent_physical_error() -> LifecyclePreflightEvidence {
    LifecyclePreflightEvidence::new(
        LifecyclePreflightSource::TargetPartition,
        LifecyclePreflightExpectation::ExactAuthority,
        LifecyclePreflightObserved::Contradictory,
    )
}

fn target_preparation_permanent_retired_host_is_current(
    world: &World,
    validation: &FrozenTargetPreparationPermanentPhysicalManifest,
    candidate: &ValidatedHostCandidate,
) -> Result<(), LifecyclePreflightEvidence> {
    let host = validation.allowed_write.host_edge.host;
    if candidate.host() == host
        && validated_host_candidate_is_current(world, candidate)
        && world
            .get::<crate::app_ext::RouterHostRegistration>(host)
            .is_some_and(|registration| registration.router() == validation.lineage.router)
    {
        Ok(())
    } else {
        Err(target_preparation_permanent_physical_error())
    }
}

fn prepare_target_preparation_permanent_current_isolation(
    world: &World,
    cleanup: &TargetPreparationPermanentCleanupOwner,
    retired_host: &RetiredHostBinding,
    validation: &FrozenTargetPreparationPermanentPhysicalManifest,
) -> Result<PreparedTargetPreparationPermanentCurrentIsolation, LifecyclePreflightEvidence> {
    let input_focus = world
        .get_resource::<InputFocus>()
        .ok_or_else(target_preparation_permanent_physical_error)?;
    let route_focus = world
        .get_resource::<RouteFocusState>()
        .ok_or_else(target_preparation_permanent_physical_error)?;
    let input_gate = world
        .get_resource::<RouterInputGate>()
        .ok_or_else(target_preparation_permanent_physical_error)?;
    let splice = target_preparation_permanent_splice(cleanup);
    let transitions =
        target_preparation_permanent_lifecycle_transitions(splice, &retired_host.top_slot_claim)
            .ok_or_else(target_preparation_permanent_physical_error)?;
    let registry = world
        .get_resource::<RouteEntityRegistry>()
        .ok_or_else(target_preparation_permanent_physical_error)?;
    let registry = registry
        .prepare_lifecycle_mutation(&transitions)
        .ok_or_else(target_preparation_permanent_physical_error)?;
    let mut projected_claims = registry.resulting_claims().to_vec();
    projected_claims.sort_by_key(|claim| claim.entity().to_bits());
    let rebind = splice
        .prepare_permanent_isolation_claim_rebind(&transitions, &projected_claims)
        .ok_or_else(target_preparation_permanent_physical_error)?;
    let physical =
        prepare_target_preparation_permanent_physical_mutation(validation, &projected_claims)
            .ok_or_else(target_preparation_permanent_physical_error)?;
    let expected_top_slot_claim = projected_claims
        .iter()
        .find(|claim| claim.entity() == retired_host.top_slot)
        .filter(|claim| claim.lifecycle() == EntityLifecycle::Leaving)
        .cloned()
        .ok_or_else(target_preparation_permanent_physical_error)?;
    let current_interactive = splice.current_live_interactive_entities(world);
    let mut isolation = match prepare_target_preparation_permanent_isolation(
        world,
        validation
            .target_observed
            .iter()
            .filter_map(|observed| observed.registry_claim.as_ref()),
        current_interactive.iter().copied(),
        validation.target_observed.len(),
        validation,
    ) {
        AuthorityIsolationPreparation::Ready(prepared) => prepared,
        AuthorityIsolationPreparation::RetryableCapacity
        | AuthorityIsolationPreparation::PermanentContradiction { .. } => {
            return Err(target_preparation_permanent_physical_error());
        }
    };
    let mut inserted_pickable = isolation
        .entities
        .iter()
        .filter(|entity| entity.insert_pickable)
        .map(|entity| entity.entity)
        .collect::<Vec<_>>();
    inserted_pickable.sort_by_key(|entity| entity.to_bits());
    let pickable_component = world
        .components()
        .component_id::<Pickable>()
        .ok_or_else(target_preparation_permanent_physical_error)?;
    let mut pickable_repairs = Vec::new();
    pickable_repairs
        .try_reserve_exact(inserted_pickable.len())
        .map_err(|_| target_preparation_permanent_physical_error())?;
    let mut repair_roots = validation
        .target_observed
        .iter()
        .map(|observed| observed.entity)
        .chain(validation.protected.roots.iter().copied())
        .chain([
            validation.allowed_write.host_edge.host,
            validation.allowed_write.host_edge.top_slot,
        ])
        .collect::<Vec<_>>();
    repair_roots.sort_by_key(|entity| entity.to_bits());
    repair_roots.dedup();
    let mut repair_input = ProjectionlessObservedProjection::capture(world, repair_roots.clone());
    let mut repair_validation = validation.clone();
    for entity in &inserted_pickable {
        repair_validation = target_preparation_permanent_expected_pickable_repair_validation(
            &repair_validation,
            *entity,
            pickable_component,
        )
        .ok_or_else(target_preparation_permanent_physical_error)?;
        repair_input = target_preparation_permanent_expected_pickable_repair_input(
            repair_input,
            *entity,
            pickable_component,
        )
        .ok_or_else(target_preparation_permanent_physical_error)?;
        pickable_repairs.push(PreparedTargetPreparationPermanentPickableRepair {
            entity: *entity,
            expected_validation: repair_validation.clone(),
            roots: repair_roots.clone(),
            expected_input: repair_input.clone(),
        });
    }
    for entity in &mut isolation.entities {
        if entity.insert_pickable {
            entity.insert_pickable = false;
        }
    }
    let expected_validation = target_preparation_permanent_expected_isolated_validation(
        validation,
        &projected_claims,
        &inserted_pickable,
        Some(pickable_component),
    )
    .ok_or_else(target_preparation_permanent_physical_error)?;
    let expected_gate = Some({
        world
            .iter_entities()
            .map(|entity| {
                let entity = entity.id();
                (
                    entity,
                    input_gate.allows_pointer(entity) && !current_interactive.contains(&entity),
                )
            })
            .collect::<Vec<_>>()
    });
    let expected_input_focus = Some(
        input_focus
            .get()
            .filter(|focused| !isolation.focus.contains(focused)),
    );
    let expected_route_focus = Some(
        route_focus
            .focused()
            .filter(|focused| !isolation.focus.contains(focused)),
    );
    Ok(PreparedTargetPreparationPermanentCurrentIsolation {
        registry,
        rebind,
        physical,
        isolation,
        pickable_repairs,
        projected_claims,
        expected_validation,
        expected_gate,
        expected_input_focus,
        expected_route_focus,
        expected_top_slot_claim,
    })
}

impl PreparedTargetPreparationPermanentCurrentIsolation {
    fn apply(
        self,
        world: &mut World,
        cleanup: &mut TargetPreparationPermanentCleanupOwner,
        retired_host: &mut RetiredHostBinding,
    ) -> Result<
        AppliedTargetPreparationPermanentCurrentIsolation,
        TargetPreparationPermanentCurrentIsolationApplyError,
    > {
        let Self {
            registry,
            rebind,
            physical,
            isolation,
            pickable_repairs,
            projected_claims,
            expected_validation,
            expected_gate,
            expected_input_focus,
            expected_route_focus,
            expected_top_slot_claim,
        } = self;
        for repair in pickable_repairs {
            world
                .entity_mut(repair.entity)
                .insert(bevy_picking::Pickable::IGNORE);
            world.flush();
            let observed_input =
                ProjectionlessObservedProjection::capture(world, repair.roots.clone());
            if observed_input != repair.expected_input {
                return Err(
                    TargetPreparationPermanentCurrentIsolationApplyError::Repair {
                        evidence: TargetPreparationPermanentRepairRefreezeEvidence {
                            entity: repair.entity,
                            expected: repair.expected_input,
                            observed: observed_input,
                        },
                        validation: repair.expected_validation,
                    },
                );
            }
            if let Err(evidence) =
                target_preparation_permanent_prewrite_refreeze(world, &repair.expected_validation)
            {
                return Err(
                    TargetPreparationPermanentCurrentIsolationApplyError::Physical {
                        evidence,
                        validation: repair.expected_validation,
                    },
                );
            }
        }
        registry.apply(&mut world.resource_mut::<RouteEntityRegistry>());
        physical.apply(world);
        isolation.apply(world);
        target_preparation_permanent_splice_mut(cleanup)
            .apply_permanent_isolation_claim_rebind(rebind);
        retired_host.top_slot_claim = expected_top_slot_claim;
        Ok(AppliedTargetPreparationPermanentCurrentIsolation {
            projected_claims,
            expected_validation,
            expected_gate,
            expected_input_focus,
            expected_route_focus,
        })
    }
}

fn target_preparation_permanent_postwrite_refreeze(
    world: &World,
    cleanup: &TargetPreparationPermanentCleanupOwner,
    retired_host: &RetiredHostBinding,
    applied: &AppliedTargetPreparationPermanentCurrentIsolation,
) -> Result<(), LifecyclePreflightEvidence> {
    target_preparation_permanent_prewrite_refreeze(world, &applied.expected_validation)?;
    target_preparation_permanent_retired_host_is_current(
        world,
        &applied.expected_validation,
        &retired_host.candidate,
    )?;
    if retired_host.top_slot_claim
        != applied
            .projected_claims
            .iter()
            .find(|claim| claim.entity() == retired_host.top_slot)
            .cloned()
            .ok_or_else(target_preparation_permanent_physical_error)?
        || !target_preparation_permanent_splice(cleanup)
            .all_claim_aliases_match(&applied.projected_claims)
        || applied
            .expected_validation
            .target_observed
            .iter()
            .any(|observed| {
                world.get::<Visibility>(observed.entity) != Some(&Visibility::Hidden)
                    || world.get::<Pickable>(observed.entity) != Some(&Pickable::IGNORE)
                    || world
                        .get::<RouterInputBarrier>(observed.entity)
                        .is_none_or(|barrier| !barrier.denies_all())
            })
    {
        return Err(target_preparation_permanent_physical_error());
    }
    let gate_is_exact = match (
        &applied.expected_gate,
        world.get_resource::<RouterInputGate>(),
    ) {
        (Some(expected), Some(gate)) => expected
            .iter()
            .all(|(entity, allowed)| gate.allows_pointer(*entity) == *allowed),
        _ => false,
    };
    let input_focus = world.get_resource::<InputFocus>().map(InputFocus::get);
    let route_focus = world
        .get_resource::<RouteFocusState>()
        .map(RouteFocusState::focused);
    if !gate_is_exact
        || input_focus != applied.expected_input_focus
        || route_focus != applied.expected_route_focus
    {
        return Err(target_preparation_permanent_physical_error());
    }
    Ok(())
}

fn target_preparation_permanent_relationship_manifest_is_exact(
    world: &World,
    observed: &FrozenTargetPreparationPermanentObservedEntity,
    expected: &FrozenTargetPreparationPermanentObservedEntity,
    expected_parent: Option<bevy_ecs::entity::Entity>,
    expected_children: &[bevy_ecs::entity::Entity],
) -> bool {
    let (Some(observed_manifest), Some(expected_manifest)) =
        (observed.manifest.as_ref(), expected.manifest.as_ref())
    else {
        return false;
    };
    let child_of = world
        .components()
        .component_id::<bevy_ecs::hierarchy::ChildOf>();
    let children = world
        .components()
        .component_id::<bevy_ecs::hierarchy::Children>();
    let non_relationship = |manifest: &ComponentManifest| {
        manifest
            .component_ids()
            .iter()
            .copied()
            .filter(|component| Some(*component) != child_of && Some(*component) != children)
            .collect::<Vec<_>>()
    };
    observed.entity == expected.entity
        && observed.parent == expected_parent
        && observed.children == expected_children
        && observed.registry_claim == expected.registry_claim
        && observed.ownership_seal == expected.ownership_seal
        && non_relationship(observed_manifest) == non_relationship(expected_manifest)
        && child_of.is_none_or(|component| {
            observed_manifest.component_ids().contains(&component) == expected_parent.is_some()
        })
        && children.is_none_or(|component| {
            observed_manifest.component_ids().contains(&component) == !expected_children.is_empty()
        })
}

fn target_preparation_permanent_input_is_isolated(
    world: &World,
    validation: &FrozenTargetPreparationPermanentPhysicalManifest,
    expected_input_focus: Option<Option<bevy_ecs::entity::Entity>>,
    expected_route_focus: Option<Option<bevy_ecs::entity::Entity>>,
) -> bool {
    let observed = target_preparation_permanent_input_observation(world, validation);
    target_preparation_permanent_input_observation_is_exact(
        &observed,
        expected_input_focus,
        expected_route_focus,
    )
}

fn target_preparation_permanent_input_observation_is_exact(
    observed: &TargetPreparationPermanentSettlementInputObservation,
    expected_input_focus: Option<Option<bevy_ecs::entity::Entity>>,
    expected_route_focus: Option<Option<bevy_ecs::entity::Entity>>,
) -> bool {
    expected_input_focus.is_some()
        && expected_route_focus.is_some()
        && observed.input_resource_present
        && observed.input_focus == expected_input_focus.flatten()
        && observed.route_resource_present
        && observed.route_focus == expected_route_focus.flatten()
        && observed.gate_present
        && observed.non_isolated_targets.is_empty()
}

fn target_preparation_permanent_input_observation(
    world: &World,
    validation: &FrozenTargetPreparationPermanentPhysicalManifest,
) -> TargetPreparationPermanentSettlementInputObservation {
    let gate = world.get_resource::<RouterInputGate>();
    let non_isolated_targets = validation
        .target_observed
        .iter()
        .filter(|observed| {
            world.get_entity(observed.entity).is_ok()
                && (world.get::<Visibility>(observed.entity) != Some(&Visibility::Hidden)
                    || world.get::<Pickable>(observed.entity) != Some(&Pickable::IGNORE)
                    || world
                        .get::<RouterInputBarrier>(observed.entity)
                        .is_none_or(|barrier| !barrier.denies_all())
                    || gate.is_none_or(|gate| {
                        gate.allows_pointer(observed.entity)
                            || gate.allows_keyboard(observed.entity)
                            || gate.allows_gamepad(observed.entity)
                    }))
        })
        .map(|observed| observed.entity)
        .collect();
    TargetPreparationPermanentSettlementInputObservation {
        input_resource_present: world.contains_resource::<InputFocus>(),
        input_focus: world.get_resource::<InputFocus>().and_then(InputFocus::get),
        route_resource_present: world.contains_resource::<RouteFocusState>(),
        route_focus: world
            .get_resource::<RouteFocusState>()
            .and_then(RouteFocusState::focused),
        gate_present: gate.is_some(),
        non_isolated_targets,
    }
}

fn target_preparation_permanent_settlement_prewrite_error(
    failure: TargetPreparationPermanentSettlementRefreezeFailure,
) -> TargetPreparationPermanentSettlementPrewriteEvidence {
    TargetPreparationPermanentSettlementPrewriteEvidence {
        source: TargetPreparationPermanentSettlementRefreezeSource::ActiveIsolated,
        failure,
    }
}

fn target_preparation_permanent_settlement_prewrite_refreeze(
    world: &World,
    validation: &FrozenTargetPreparationPermanentPhysicalManifest,
    candidate: &ValidatedHostCandidate,
    expected_input_focus: Option<Option<bevy_ecs::entity::Entity>>,
    expected_route_focus: Option<Option<bevy_ecs::entity::Entity>>,
) -> Result<(), TargetPreparationPermanentSettlementPrewriteEvidence> {
    let Some(registry) = world.get_resource::<RouteEntityRegistry>() else {
        return Err(target_preparation_permanent_settlement_prewrite_error(
            TargetPreparationPermanentSettlementRefreezeFailure::Registry {
                expected: true,
                observed: false,
            },
        ));
    };
    let host_edge = &validation.allowed_write.host_edge;
    let observed_host =
        observe_target_preparation_permanent_entity(world, registry, host_edge.host);
    let candidate_current = validated_host_candidate_is_current(world, candidate);
    let registration = world
        .get::<crate::app_ext::RouterHostRegistration>(host_edge.host)
        .map(
            |registration| TargetPreparationPermanentSettlementHostRegistrationObservation {
                router: registration.router(),
                incarnation: registration.incarnation(),
            },
        );
    if candidate.host() != host_edge.host
        || !candidate_current
        || registration
            .as_ref()
            .map(|registration| registration.router)
            != Some(validation.lineage.router)
        || !target_preparation_permanent_relationship_manifest_is_exact(
            world,
            &observed_host,
            &host_edge.host_observed,
            host_edge.host_observed.parent,
            &host_edge.host_children_before,
        )
        || !target_preparation_permanent_authority_values_are_exact(
            &observed_host,
            &host_edge.host_observed,
        )
    {
        return Err(target_preparation_permanent_settlement_prewrite_error(
            TargetPreparationPermanentSettlementRefreezeFailure::HostIncarnation {
                expected: TargetPreparationPermanentSettlementHostExpectation {
                    candidate: candidate.clone(),
                    router: validation.lineage.router,
                    host: host_edge.host_observed.clone(),
                    parent: host_edge.host_observed.parent,
                    children: host_edge.host_children_before.clone(),
                },
                observed: TargetPreparationPermanentSettlementHostObservation {
                    candidate_current,
                    registration,
                    host: observed_host,
                },
            },
        ));
    }

    let observed = target_preparation_permanent_input_observation(world, validation);
    if !target_preparation_permanent_input_observation_is_exact(
        &observed,
        expected_input_focus,
        expected_route_focus,
    ) {
        return Err(target_preparation_permanent_settlement_prewrite_error(
            TargetPreparationPermanentSettlementRefreezeFailure::InputIsolation {
                expected: TargetPreparationPermanentSettlementInputExpectation {
                    input_focus: expected_input_focus.flatten(),
                    route_focus: expected_route_focus.flatten(),
                    targets: validation
                        .target_observed
                        .iter()
                        .map(|observed| observed.entity)
                        .collect(),
                },
                observed,
            },
        ));
    }
    Ok(())
}

fn target_preparation_permanent_settlement_refreeze_error(
    stage: TargetPreparationPermanentTransitionStage,
    failure: TargetPreparationPermanentSettlementRefreezeFailure,
) -> TargetPreparationPermanentSettlementRefreezeEvidence {
    TargetPreparationPermanentSettlementRefreezeEvidence {
        source: TargetPreparationPermanentSettlementRefreezeSource::ActiveIsolated,
        stage,
        failure,
    }
}

fn target_preparation_permanent_settlement_refreeze(
    world: &World,
    validation: &FrozenTargetPreparationPermanentPhysicalManifest,
    candidate: &ValidatedHostCandidate,
    expected_input_focus: Option<Option<bevy_ecs::entity::Entity>>,
    expected_route_focus: Option<Option<bevy_ecs::entity::Entity>>,
    transition: TargetPreparationPermanentTransition<'_>,
) -> Result<(), TargetPreparationPermanentSettlementRefreezeEvidence> {
    let stage = transition.stage();
    let stage_is_exact = match transition.stage() {
        TargetPreparationPermanentTransitionStage::HostEdgeDetached => {
            transition.settled().is_empty() && transition.in_flight().is_none()
        }
        TargetPreparationPermanentTransitionStage::PhysicalAbsent(entity) => {
            transition.in_flight() == Some(entity) && !transition.settled().contains(&entity)
        }
        TargetPreparationPermanentTransitionStage::ReceiptCommitted(entity) => {
            transition.in_flight().is_none() && transition.settled().last() == Some(&entity)
        }
    };
    if !stage_is_exact {
        return Err(target_preparation_permanent_settlement_refreeze_error(
            stage,
            TargetPreparationPermanentSettlementRefreezeFailure::Transition {
                expected: stage,
                observed_settled: transition.settled().to_vec(),
                observed_in_flight: transition.in_flight(),
            },
        ));
    }
    let input_observation = target_preparation_permanent_input_observation(world, validation);
    if !target_preparation_permanent_input_is_isolated(
        world,
        validation,
        expected_input_focus,
        expected_route_focus,
    ) {
        return Err(target_preparation_permanent_settlement_refreeze_error(
            stage,
            TargetPreparationPermanentSettlementRefreezeFailure::InputIsolation {
                expected: TargetPreparationPermanentSettlementInputExpectation {
                    input_focus: expected_input_focus.flatten(),
                    route_focus: expected_route_focus.flatten(),
                    targets: validation
                        .target_observed
                        .iter()
                        .map(|observed| observed.entity)
                        .collect(),
                },
                observed: input_observation,
            },
        ));
    }
    let Some(registry) = world.get_resource::<RouteEntityRegistry>() else {
        return Err(target_preparation_permanent_settlement_refreeze_error(
            stage,
            TargetPreparationPermanentSettlementRefreezeFailure::Registry {
                expected: true,
                observed: false,
            },
        ));
    };
    let target_entities = validation
        .target_observed
        .iter()
        .map(|observed| observed.entity)
        .collect::<std::collections::HashSet<_>>();
    let settled = transition
        .settled()
        .iter()
        .copied()
        .collect::<std::collections::HashSet<_>>();
    if settled.len() != transition.settled().len()
        || !settled.is_subset(&target_entities)
        || transition
            .in_flight()
            .is_some_and(|entity| !target_entities.contains(&entity) || settled.contains(&entity))
    {
        return Err(target_preparation_permanent_settlement_refreeze_error(
            stage,
            TargetPreparationPermanentSettlementRefreezeFailure::Transition {
                expected: stage,
                observed_settled: transition.settled().to_vec(),
                observed_in_flight: transition.in_flight(),
            },
        ));
    }
    let mut removed = settled.clone();
    removed.extend(transition.in_flight());
    let host_edge = &validation.allowed_write.host_edge;
    let observed_host =
        observe_target_preparation_permanent_entity(world, registry, host_edge.host);
    if candidate.host() != host_edge.host
        || !validated_host_candidate_is_current(world, candidate)
        || world
            .get::<crate::app_ext::RouterHostRegistration>(host_edge.host)
            .is_none_or(|registration| registration.router() != validation.lineage.router)
        || !target_preparation_permanent_relationship_manifest_is_exact(
            world,
            &observed_host,
            &host_edge.host_observed,
            host_edge.host_observed.parent,
            &host_edge.host_children_after,
        )
        || !target_preparation_permanent_authority_values_are_exact(
            &observed_host,
            &host_edge.host_observed,
        )
    {
        return Err(target_preparation_permanent_settlement_refreeze_error(
            stage,
            TargetPreparationPermanentSettlementRefreezeFailure::HostIncarnation {
                expected: TargetPreparationPermanentSettlementHostExpectation {
                    candidate: candidate.clone(),
                    router: validation.lineage.router,
                    host: host_edge.host_observed.clone(),
                    parent: host_edge.host_observed.parent,
                    children: host_edge.host_children_after.clone(),
                },
                observed: TargetPreparationPermanentSettlementHostObservation {
                    candidate_current: validated_host_candidate_is_current(world, candidate),
                    registration: world
                        .get::<crate::app_ext::RouterHostRegistration>(host_edge.host)
                        .map(|registration| {
                            TargetPreparationPermanentSettlementHostRegistrationObservation {
                                router: registration.router(),
                                incarnation: registration.incarnation(),
                            }
                        }),
                    host: observed_host,
                },
            },
        ));
    }

    for expected in &validation.target_observed {
        if removed.contains(&expected.entity) {
            let observed_claim = registry
                .record(expected.entity)
                .map(|record| record.claim());
            let expected_claim = if transition.in_flight() == Some(expected.entity) {
                expected.registry_claim.clone()
            } else {
                None
            };
            let claim_is_exact = if transition.in_flight() == Some(expected.entity) {
                observed_claim == expected.registry_claim
            } else {
                observed_claim.is_none()
            };
            if world.get_entity(expected.entity).is_ok() || !claim_is_exact {
                let observed =
                    observe_target_preparation_permanent_entity(world, registry, expected.entity);
                return Err(target_preparation_permanent_settlement_refreeze_error(
                    stage,
                    TargetPreparationPermanentSettlementRefreezeFailure::Target {
                        entity: expected.entity,
                        expected: TargetPreparationPermanentSettlementEntityExpectation {
                            present: false,
                            registry_claim: expected_claim,
                            parent: None,
                            children: Vec::new(),
                            facts: None,
                            isolated: true,
                        },
                        observed: TargetPreparationPermanentSettlementEntityObservation {
                            present: world.get_entity(expected.entity).is_ok(),
                            isolated: !input_observation
                                .non_isolated_targets
                                .contains(&expected.entity),
                            facts: observed,
                        },
                    },
                ));
            }
            continue;
        }
        let observed =
            observe_target_preparation_permanent_entity(world, registry, expected.entity);
        let expected_parent = if expected.entity == host_edge.top_slot {
            host_edge.top_slot_parent_after
        } else {
            expected.parent
        };
        let expected_children = expected
            .children
            .iter()
            .copied()
            .filter(|child| !removed.contains(child))
            .collect::<Vec<_>>();
        if !target_preparation_permanent_relationship_manifest_is_exact(
            world,
            &observed,
            expected,
            expected_parent,
            &expected_children,
        ) || !target_preparation_permanent_authority_values_are_exact(&observed, expected)
        {
            return Err(target_preparation_permanent_settlement_refreeze_error(
                stage,
                TargetPreparationPermanentSettlementRefreezeFailure::Target {
                    entity: expected.entity,
                    expected: TargetPreparationPermanentSettlementEntityExpectation {
                        present: true,
                        registry_claim: expected.registry_claim.clone(),
                        parent: expected_parent,
                        children: expected_children,
                        facts: Some(expected.clone()),
                        isolated: true,
                    },
                    observed: TargetPreparationPermanentSettlementEntityObservation {
                        present: world.get_entity(expected.entity).is_ok(),
                        isolated: !input_observation
                            .non_isolated_targets
                            .contains(&expected.entity),
                        facts: observed,
                    },
                },
            ));
        }
    }
    for expected in &validation.protected.observed {
        if expected.entity == host_edge.host {
            continue;
        }
        let observed =
            observe_target_preparation_permanent_entity(world, registry, expected.entity);
        if !target_preparation_permanent_structure_is_exact(&observed, expected)
            || !target_preparation_permanent_authority_values_are_exact(&observed, expected)
        {
            return Err(target_preparation_permanent_settlement_refreeze_error(
                stage,
                TargetPreparationPermanentSettlementRefreezeFailure::Protected {
                    entity: expected.entity,
                    expected: expected.clone(),
                    observed,
                },
            ));
        }
    }

    let remaining_targets = target_entities
        .iter()
        .copied()
        .filter(|entity| !removed.contains(entity))
        .collect::<std::collections::HashSet<_>>();
    let mut expected_closure = validation
        .protected
        .observed
        .iter()
        .map(|observed| observed.entity)
        .collect::<std::collections::HashSet<_>>();
    expected_closure.insert(host_edge.host);
    expected_closure.extend(remaining_targets.iter().copied());
    let mut closure_roots = remaining_targets.iter().copied().collect::<Vec<_>>();
    closure_roots.push(host_edge.host);
    let observed_closure = observed_descendant_closure(world, closure_roots);
    let relationship_exact =
        target_preparation_permanent_relationship_closure_is_exact(world, &observed_closure);
    if observed_closure != expected_closure || !relationship_exact {
        let mut expected = expected_closure.into_iter().collect::<Vec<_>>();
        expected.sort_by_key(|entity| entity.to_bits());
        let mut observed = observed_closure.into_iter().collect::<Vec<_>>();
        observed.sort_by_key(|entity| entity.to_bits());
        return Err(target_preparation_permanent_settlement_refreeze_error(
            stage,
            TargetPreparationPermanentSettlementRefreezeFailure::Closure {
                expected,
                observed,
                relationship_exact,
            },
        ));
    }
    Ok(())
}

fn drive_target_preparation_permanent_cleanup(world: &mut World, entry: &mut RouterEntry) -> bool {
    match entry.pending_back_target_permanent_cleanup.as_ref() {
        None => return false,
        Some(PendingTargetPreparationPermanentCleanup::Installed(installed)) => {
            if installed.inert.is_some() {
                return true;
            }
        }
        Some(
            PendingTargetPreparationPermanentCleanup::Completing(_)
            | PendingTargetPreparationPermanentCleanup::Settled { .. },
        ) => return true,
        #[cfg(test)]
        Some(PendingTargetPreparationPermanentCleanup::TestOccupied) => return false,
    }

    let Some(pending) = entry.pending_back_target_permanent_cleanup.take() else {
        return true;
    };
    let installed = match pending {
        PendingTargetPreparationPermanentCleanup::Installed(installed) => installed,
        PendingTargetPreparationPermanentCleanup::Completing(prepared) => {
            entry.pending_back_target_permanent_cleanup = Some(
                PendingTargetPreparationPermanentCleanup::Completing(prepared),
            );
            return true;
        }
        PendingTargetPreparationPermanentCleanup::Settled {
            prepared,
            owner,
            mismatch,
        } => {
            entry.pending_back_target_permanent_cleanup =
                Some(PendingTargetPreparationPermanentCleanup::Settled {
                    prepared,
                    owner,
                    mismatch,
                });
            return true;
        }
        #[cfg(test)]
        PendingTargetPreparationPermanentCleanup::TestOccupied => {
            entry.pending_back_target_permanent_cleanup =
                Some(PendingTargetPreparationPermanentCleanup::TestOccupied);
            return false;
        }
    };
    let InstalledTargetPreparationPermanentCleanup {
        cleanup,
        ledger,
        batch,
        retired_host,
        inert,
    } = installed;
    let activated_now;
    let current = match ledger {
        TargetPreparationPermanentLedgerHolder::Prepared(prepared) => {
            activated_now = true;
            match prepared.into_active() {
                ActiveTargetPreparationPermanentLedger::Current(current) => current,
                ActiveTargetPreparationPermanentLedger::Isolated(_) => {
                    unreachable!("Prepared permanent cleanup activates only into Current")
                }
            }
        }
        TargetPreparationPermanentLedgerHolder::Active(
            ActiveTargetPreparationPermanentLedger::Current(current),
        ) => {
            activated_now = false;
            current
        }
        TargetPreparationPermanentLedgerHolder::Active(
            ActiveTargetPreparationPermanentLedger::Isolated(isolated),
        ) => {
            if isolated.ledger.is_settled() {
                let prepared = match isolated.prepare_completion() {
                    Ok(prepared) => prepared,
                    Err((isolated, evidence)) => {
                        entry.pending_back_target_permanent_cleanup =
                            Some(PendingTargetPreparationPermanentCleanup::Installed(
                                InstalledTargetPreparationPermanentCleanup {
                                    cleanup,
                                    ledger: TargetPreparationPermanentLedgerHolder::Active(
                                        ActiveTargetPreparationPermanentLedger::Isolated(isolated),
                                    ),
                                    batch,
                                    retired_host,
                                    inert: Some(
                                        TargetPreparationPermanentInertEvidence::CompletionPreparationRejected(
                                            evidence,
                                        ),
                                    ),
                                },
                            ));
                        return true;
                    }
                };
                let facts = prepared.facts();
                let completion = batch.complete(
                    facts.manifest_digest(),
                    facts.obligation_count(),
                    facts.completion_digest(),
                    facts.settled_count(),
                );
                entry.pending_back_target_permanent_cleanup =
                    Some(PendingTargetPreparationPermanentCleanup::Completing(
                        PreparedTargetPreparationPermanentPhysicalCompletion {
                            cleanup,
                            prepared,
                            retired_host,
                        },
                    ));
                let RouterEntry {
                    engine,
                    pending_back_target_permanent_cleanup: pending,
                    ..
                } = entry;
                let outcome = {
                    let pending = &mut *pending;
                    engine
                        .complete_back_target_permanent_cleanup(completion, move || *pending = None)
                };
                match outcome {
                    BackTargetPermanentCleanupCompletionOutcome::Terminal(_) => {}
                    BackTargetPermanentCleanupCompletionOutcome::Mismatch { owner, mismatch } => {
                        let current = pending.take();
                        let Some(PendingTargetPreparationPermanentCleanup::Completing(prepared)) =
                            current
                        else {
                            unreachable!(
                                "target-permanent completion mismatch preserves the preinstalled Completing owner"
                            )
                        };
                        *pending = Some(PendingTargetPreparationPermanentCleanup::Settled {
                            prepared,
                            owner,
                            mismatch,
                        });
                    }
                }
                return true;
            }
            let IsolatedTargetPreparationPermanentLedger {
                mut ledger,
                validation,
                expected_input_focus,
                expected_route_focus,
                mut completion_domain,
            } = isolated;
            let inert = match inert {
                Some(inert) => Some(inert),
                None => match target_preparation_permanent_prewrite_refreeze(world, &validation) {
                    Err(evidence) => {
                        Some(TargetPreparationPermanentInertEvidence::Physical(evidence))
                    }
                    Ok(()) => match target_preparation_permanent_settlement_prewrite_refreeze(
                        world,
                        &validation,
                        &retired_host.candidate,
                        expected_input_focus,
                        expected_route_focus,
                    ) {
                        Err(evidence) => Some(
                            TargetPreparationPermanentInertEvidence::SettlementPrewriteRefreeze(
                                evidence,
                            ),
                        ),
                        Ok(()) => {
                            let edge = &validation.allowed_write.host_edge;
                            let settlement = ledger.settle_target_preparation_permanent(
                                world,
                                TargetPreparationPermanentHostEdge::new(
                                    edge.host,
                                    edge.top_slot,
                                    &retired_host.top_slot_claim,
                                    &edge.host_children_before,
                                    &edge.host_children_after,
                                ),
                                |world, transition| {
                                    target_preparation_permanent_settlement_refreeze(
                                        world,
                                        &validation,
                                        &retired_host.candidate,
                                        expected_input_focus,
                                        expected_route_focus,
                                        transition,
                                    )
                                },
                            );
                            let receipts_observed = entry.observe_cleanup_receipts(&mut ledger);
                            if !receipts_observed {
                                Some(
                                TargetPreparationPermanentInertEvidence::ReceiptObservationRejected(
                                    ledger.target_preparation_permanent_receipt_observation_evidence(),
                                ),
                            )
                            } else {
                                match settlement {
                            Ok(domain) => {
                                completion_domain = Some(domain);
                                None
                            }
                            Err(
                                TargetPreparationPermanentSettlementError::AttemptUnavailable(
                                    evidence,
                                ),
                            ) => Some(
                                TargetPreparationPermanentInertEvidence::AttemptUnavailable(
                                    evidence,
                                ),
                            ),
                            Err(
                                TargetPreparationPermanentSettlementError::ReceiptPreparationRejected(
                                    evidence,
                                ),
                            ) => Some(
                                TargetPreparationPermanentInertEvidence::ReceiptPreparationRejected(
                                    evidence,
                                ),
                            ),
                            Err(
                                TargetPreparationPermanentSettlementError::SettlementRejected(
                                    evidence,
                                ),
                            ) => Some(
                                TargetPreparationPermanentInertEvidence::SettlementRejected(
                                    evidence,
                                ),
                            ),
                            Err(
                                TargetPreparationPermanentSettlementError::PostWriteRefreeze(
                                    evidence,
                                ),
                            ) => Some(
                                TargetPreparationPermanentInertEvidence::SettlementPostWriteRefreeze(
                                    evidence,
                                ),
                            ),
                            }
                            }
                        }
                    },
                },
            };
            entry.pending_back_target_permanent_cleanup =
                Some(PendingTargetPreparationPermanentCleanup::Installed(
                    InstalledTargetPreparationPermanentCleanup {
                        cleanup,
                        ledger: TargetPreparationPermanentLedgerHolder::Active(
                            ActiveTargetPreparationPermanentLedger::Isolated(
                                IsolatedTargetPreparationPermanentLedger {
                                    ledger,
                                    validation,
                                    expected_input_focus,
                                    expected_route_focus,
                                    completion_domain,
                                },
                            ),
                        ),
                        batch,
                        retired_host,
                        inert,
                    },
                ));
            return true;
        }
    };
    match inert {
        Some(inert) => {
            entry.pending_back_target_permanent_cleanup =
                Some(PendingTargetPreparationPermanentCleanup::Installed(
                    InstalledTargetPreparationPermanentCleanup {
                        cleanup,
                        ledger: TargetPreparationPermanentLedgerHolder::Active(
                            ActiveTargetPreparationPermanentLedger::Current(current),
                        ),
                        batch,
                        retired_host,
                        inert: Some(inert),
                    },
                ));
            return true;
        }
        None => {}
    }
    if let Err(evidence) =
        target_preparation_permanent_prewrite_refreeze(world, &current.validation)
    {
        entry.pending_back_target_permanent_cleanup =
            Some(PendingTargetPreparationPermanentCleanup::Installed(
                InstalledTargetPreparationPermanentCleanup {
                    cleanup,
                    ledger: TargetPreparationPermanentLedgerHolder::Active(
                        ActiveTargetPreparationPermanentLedger::Current(current),
                    ),
                    batch,
                    retired_host,
                    inert: Some(TargetPreparationPermanentInertEvidence::Physical(evidence)),
                },
            ));
        return true;
    }
    if let Err(evidence) = target_preparation_permanent_retired_host_is_current(
        world,
        &current.validation,
        &retired_host.candidate,
    ) {
        entry.pending_back_target_permanent_cleanup =
            Some(PendingTargetPreparationPermanentCleanup::Installed(
                InstalledTargetPreparationPermanentCleanup {
                    cleanup,
                    ledger: TargetPreparationPermanentLedgerHolder::Active(
                        ActiveTargetPreparationPermanentLedger::Current(current),
                    ),
                    batch,
                    retired_host,
                    inert: Some(TargetPreparationPermanentInertEvidence::Physical(evidence)),
                },
            ));
        return true;
    }
    if activated_now {
        entry.pending_back_target_permanent_cleanup =
            Some(PendingTargetPreparationPermanentCleanup::Installed(
                InstalledTargetPreparationPermanentCleanup {
                    cleanup,
                    ledger: TargetPreparationPermanentLedgerHolder::Active(
                        ActiveTargetPreparationPermanentLedger::Current(current),
                    ),
                    batch,
                    retired_host,
                    inert: None,
                },
            ));
        return true;
    }
    let prepared = match prepare_target_preparation_permanent_current_isolation(
        world,
        &cleanup,
        &retired_host,
        &current.validation,
    ) {
        Ok(prepared) => prepared,
        Err(evidence) => {
            entry.pending_back_target_permanent_cleanup =
                Some(PendingTargetPreparationPermanentCleanup::Installed(
                    InstalledTargetPreparationPermanentCleanup {
                        cleanup,
                        ledger: TargetPreparationPermanentLedgerHolder::Active(
                            ActiveTargetPreparationPermanentLedger::Current(current),
                        ),
                        batch,
                        retired_host,
                        inert: Some(TargetPreparationPermanentInertEvidence::Physical(evidence)),
                    },
                ));
            return true;
        }
    };
    let CurrentTargetPreparationPermanentLedger {
        ledger,
        validation: _,
    } = current;
    let mut cleanup = cleanup;
    let mut retired_host = retired_host;
    let applied = match prepared.apply(world, &mut cleanup, &mut retired_host) {
        Ok(applied) => applied,
        Err(error) => {
            let (validation, inert) = match error {
                TargetPreparationPermanentCurrentIsolationApplyError::Physical {
                    evidence,
                    validation,
                } => (
                    validation,
                    TargetPreparationPermanentInertEvidence::PostWriteRefreeze(evidence),
                ),
                TargetPreparationPermanentCurrentIsolationApplyError::Repair {
                    evidence,
                    validation,
                } => (
                    validation,
                    TargetPreparationPermanentInertEvidence::RepairPostWriteRefreeze(evidence),
                ),
            };
            entry.pending_back_target_permanent_cleanup =
                Some(PendingTargetPreparationPermanentCleanup::Installed(
                    InstalledTargetPreparationPermanentCleanup {
                        cleanup,
                        ledger: TargetPreparationPermanentLedgerHolder::Active(
                            ActiveTargetPreparationPermanentLedger::Current(
                                CurrentTargetPreparationPermanentLedger { ledger, validation },
                            ),
                        ),
                        batch,
                        retired_host,
                        inert: Some(inert),
                    },
                ));
            return true;
        }
    };
    world.flush();
    #[cfg(test)]
    if let Some(drift) = world.remove_resource::<TargetPreparationPermanentPostWriteDrift>() {
        *world
            .get_mut::<Visibility>(drift.target)
            .expect("the permanent postwrite drift target retains Visibility") =
            Visibility::Inherited;
    }
    let postwrite =
        target_preparation_permanent_postwrite_refreeze(world, &cleanup, &retired_host, &applied);
    let AppliedTargetPreparationPermanentCurrentIsolation {
        expected_validation: validation,
        expected_input_focus,
        expected_route_focus,
        ..
    } = applied;
    let (active, inert) = match postwrite {
        Ok(()) => (
            ActiveTargetPreparationPermanentLedger::Isolated(
                IsolatedTargetPreparationPermanentLedger {
                    ledger,
                    validation,
                    expected_input_focus,
                    expected_route_focus,
                    completion_domain: None,
                },
            ),
            None,
        ),
        Err(evidence) => (
            ActiveTargetPreparationPermanentLedger::Current(
                CurrentTargetPreparationPermanentLedger { ledger, validation },
            ),
            Some(TargetPreparationPermanentInertEvidence::PostWriteRefreeze(
                evidence,
            )),
        ),
    };
    entry.pending_back_target_permanent_cleanup =
        Some(PendingTargetPreparationPermanentCleanup::Installed(
            InstalledTargetPreparationPermanentCleanup {
                cleanup,
                ledger: TargetPreparationPermanentLedgerHolder::Active(active),
                batch,
                retired_host,
                inert,
            },
        ));
    true
}

const POST_COMMIT_L_OBLIGATION_KEY: u64 = 0x5232_4c4f_424c;
const POST_COMMIT_L_RECEIPT_KEY: u64 = 0x5232_4c52_4350;
const POST_COMMIT_L_SOURCE_KEY: u64 = 0x5232_4c53_5243;
const POST_COMMIT_A_OBLIGATION_KEY: u64 = 0x5232_414f_424c;
const POST_COMMIT_A_SOURCE_KEY: u64 = 0x5232_4153_5243;
const POST_COMMIT_H_OBLIGATION_KEY: u64 = 0x5232_484f_424c;
const POST_COMMIT_H_SOURCE_KEY: u64 = 0x5232_4853_5243;

impl FrozenRetiredHostEdge {
    fn freeze(
        world: &World,
        host: &HostBinding,
    ) -> Result<Self, RetiredHostEdgeFreezePermanentEvidence> {
        if host.top_slot_claim.entity() != host.top_slot
            || host.candidate.host() != host.host
            || !matches!(
                host.top_slot_claim.owner(),
                RegistryOwner::Committed(EntityOwner::HostTopSlot { host: version, slot })
                    if version == &host.version && *slot == host.slot_id
            )
        {
            return Err(RetiredHostEdgeFreezePermanentEvidence::InvalidBinding);
        }
        let host_present = world.get_entity(host.host).is_ok();
        let children_component = world.components().component_id::<Children>();
        let host_non_children_manifest =
            ComponentManifest::from_world(world, host.host).map(|manifest| {
                ComponentManifest::new(
                    manifest
                        .component_ids()
                        .iter()
                        .copied()
                        .filter(|component| Some(*component) != children_component),
                )
            });
        let children_before = world
            .get::<Children>(host.host)
            .map(|children| children.iter().copied().collect::<Vec<_>>())
            .unwrap_or_default();
        let top_parent_before = world.get::<ChildOf>(host.top_slot).map(ChildOf::parent);
        let membership = children_before
            .iter()
            .filter(|child| **child == host.top_slot)
            .count();
        if host_present
            && ((top_parent_before == Some(host.host) && membership != 1)
                || (top_parent_before != Some(host.host) && membership != 0))
        {
            return Err(RetiredHostEdgeFreezePermanentEvidence::InvalidRelationship);
        }
        let mut children_after = children_before.clone();
        children_after.retain(|child| *child != host.top_slot);
        let host_parent = world.get::<ChildOf>(host.host).map(ChildOf::parent);
        let mut host_ancestry = Vec::new();
        let mut cursor = host_parent;
        while let Some(entity) = cursor {
            if entity == host.host || host_ancestry.contains(&entity) {
                return Err(RetiredHostEdgeFreezePermanentEvidence::RelationshipCycle);
            }
            host_ancestry.push(entity);
            cursor = world.get::<ChildOf>(entity).map(ChildOf::parent);
        }
        let obligation_binding = host.host.to_bits().rotate_left(11)
            ^ host.top_slot.to_bits().rotate_left(29)
            ^ host.registration_incarnation;
        let physical_source_binding = obligation_binding
            ^ (children_before.len() as u64).rotate_left(17)
            ^ top_parent_before.map_or(0, |entity| entity.to_bits());
        Ok(Self {
            host: host.host,
            top_slot: host.top_slot,
            top_slot_claim: host.top_slot_claim.clone(),
            registration_incarnation: host.registration_incarnation,
            host_present,
            host_non_children_manifest,
            host_parent,
            host_ancestry,
            children_before,
            children_after,
            top_parent_before,
            obligation_binding,
            physical_source_binding,
        })
    }

    fn host_projection_is_current(&self, world: &World, detached: bool) -> bool {
        if !self.host_present {
            return world.get_entity(self.host).is_err();
        }
        if world.get_entity(self.host).is_err()
            || world.get::<ChildOf>(self.host).map(ChildOf::parent) != self.host_parent
        {
            return false;
        }
        let children_component = world.components().component_id::<Children>();
        let observed_host_manifest =
            ComponentManifest::from_world(world, self.host).map(|manifest| {
                ComponentManifest::new(
                    manifest
                        .component_ids()
                        .iter()
                        .copied()
                        .filter(|component| Some(*component) != children_component),
                )
            });
        if observed_host_manifest != self.host_non_children_manifest {
            return false;
        }
        let mut ancestry = Vec::new();
        let mut cursor = self.host_parent;
        while let Some(entity) = cursor {
            if entity == self.host || ancestry.contains(&entity) {
                return false;
            }
            ancestry.push(entity);
            cursor = world.get::<ChildOf>(entity).map(ChildOf::parent);
        }
        if ancestry != self.host_ancestry {
            return false;
        }
        let expected_children = if detached {
            &self.children_after
        } else {
            &self.children_before
        };
        let observed_children = world
            .get::<Children>(self.host)
            .map(|children| children.iter().copied().collect::<Vec<_>>())
            .unwrap_or_default();
        let observed_parent = world.get::<ChildOf>(self.top_slot).map(ChildOf::parent);
        observed_children == *expected_children
            && if detached {
                observed_parent != Some(self.host)
            } else {
                observed_parent == self.top_parent_before
            }
    }
}

fn post_commit_cleanup_plan_entry(
    target: &crate::cleanup::FrozenPostCommitAuthorityTarget,
) -> PostCommitPlannedCleanupProjection {
    PostCommitPlannedCleanupProjection::from_trusted_ecs_freeze(
        PostCommitSettlementKeyProjection::from_trusted_ecs_freeze(
            POST_COMMIT_A_OBLIGATION_KEY,
            vec![
                target.index() as u64,
                target.entity().to_bits(),
                target.obligation_binding(),
            ],
        ),
        PostCommitSettlementKeyProjection::from_trusted_ecs_freeze(
            POST_COMMIT_A_SOURCE_KEY,
            vec![
                target.index() as u64,
                target.entity().to_bits(),
                target.physical_source_binding(),
            ],
        ),
        target.disposition().public(),
    )
}

fn bind_post_commit_settlement_plan(
    original: &FrozenOriginalCleanupLedgerTransfer,
    authority: &FrozenPostCommitAuthorityPartition,
    edge: &FrozenRetiredHostEdge,
    builder: bevy_ui_router_core::integration::PostCommitSettlementPlanBuilder,
) -> Result<
    bevy_ui_router_core::integration::PostCommitSettlementPlan,
    PostCommitSettlementPlanMismatchEvidence,
> {
    let settled_original = original
        .indices()
        .iter()
        .filter_map(|entry| {
            let receipt = entry.settled_receipt()?;
            Some(
                PostCommitSettledOriginalProjection::from_trusted_ecs_freeze(
                    PostCommitSettlementKeyProjection::from_trusted_ecs_freeze(
                        POST_COMMIT_L_OBLIGATION_KEY,
                        vec![
                            original.ledger_domain(),
                            entry.index() as u64,
                            entry.obligation_binding(),
                        ],
                    ),
                    PostCommitSettlementKeyProjection::from_trusted_ecs_freeze(
                        POST_COMMIT_L_RECEIPT_KEY,
                        vec![
                            original.ledger_domain(),
                            receipt.obligation() as u64,
                            receipt.attempt_revision(),
                            receipt.binding(),
                        ],
                    ),
                    receipt.disposition().public(),
                ),
            )
        })
        .collect::<Vec<_>>();
    let residual_original = original
        .indices()
        .iter()
        .filter(|entry| entry.settled_receipt().is_none())
        .map(|entry| {
            PostCommitPlannedCleanupProjection::from_trusted_ecs_freeze(
                PostCommitSettlementKeyProjection::from_trusted_ecs_freeze(
                    POST_COMMIT_L_OBLIGATION_KEY,
                    vec![
                        original.ledger_domain(),
                        entry.index() as u64,
                        entry.obligation_binding(),
                    ],
                ),
                PostCommitSettlementKeyProjection::from_trusted_ecs_freeze(
                    POST_COMMIT_L_SOURCE_KEY,
                    vec![
                        original.ledger_domain(),
                        entry.index() as u64,
                        entry.physical_source_binding(),
                    ],
                ),
                entry.disposition().public(),
            )
        })
        .collect::<Vec<_>>();
    let authority_non_top = authority
        .non_top_targets()
        .map(post_commit_cleanup_plan_entry)
        .collect::<Vec<_>>();
    let authority_top_target = authority
        .top_target()
        .expect("the frozen authority partition owns exactly one top slot");
    let authority_top = post_commit_cleanup_plan_entry(authority_top_target);
    let host_obligation = PostCommitSettlementKeyProjection::from_trusted_ecs_freeze(
        POST_COMMIT_H_OBLIGATION_KEY,
        vec![
            edge.host.to_bits(),
            edge.top_slot.to_bits(),
            edge.registration_incarnation,
            edge.obligation_binding,
        ],
    );
    let host_edge = PostCommitPlannedHostEdgeProjection::from_trusted_ecs_freeze(
        host_obligation.clone(),
        PostCommitSettlementKeyProjection::from_trusted_ecs_freeze(
            POST_COMMIT_H_SOURCE_KEY,
            vec![
                edge.host.to_bits(),
                edge.top_slot.to_bits(),
                edge.physical_source_binding,
            ],
        ),
    );
    let prior_trace_start = builder
        .prior_trace_len()
        .checked_sub(settled_original.len())
        .unwrap_or(usize::MAX);
    builder.bind_trusted_ecs(PostCommitSettlementPlanDescriptor::from_trusted_ecs_freeze(
        prior_trace_start,
        settled_original,
        residual_original,
        authority_non_top,
        authority_top,
        host_obligation,
        host_edge,
    ))
}

enum PostCommitPlanBindFailure {
    Plan(PostCommitSettlementPlanMismatchEvidence),
    Authority(crate::cleanup::PreparedAuthorityLedgerBindingMismatch),
    Isolation(PostCommitInputIsolationFailure),
}

enum PostCommitAcceptedInertEvidence {
    Receiver(PostCommitReceiverMismatchView),
    Retryable(PostCommitAuthorityBeginRetryEvidence),
    Permanent(PostCommitAuthorityBeginPermanentEvidence),
}

impl PostCommitAcceptedInertEvidence {
    fn is_retryable(&self) -> bool {
        matches!(self, Self::Retryable(_))
    }
}

enum PostCommitBoundInertEvidence {
    Binding(PostCommitPlanBindFailure),
    Receiver(PostCommitReceiverMismatchView),
    Retryable(PostCommitAuthorityBeginRetryEvidence),
    Permanent(PostCommitAuthorityBeginPermanentEvidence),
}

impl PostCommitBoundInertEvidence {
    fn is_retryable(&self) -> bool {
        matches!(self, Self::Retryable(_))
    }
}

fn post_commit_authority_loss_cause(
    world: &World,
    host: &HostBinding,
) -> Option<PostCommitAuthorityCause> {
    if current_host_binding_is_exact(world, host) {
        return None;
    }
    Some(if world.get_resource::<RouteEntityRegistry>().is_none() {
        PostCommitAuthorityCause::RequiredResourcesLost
    } else if world.get_entity(host.host).is_err() {
        PostCommitAuthorityCause::HostEntityMissing
    } else if world
        .get::<RouterHostRegistration>(host.host)
        .is_none_or(|registration| registration.incarnation() != host.registration_incarnation)
    {
        PostCommitAuthorityCause::HostRegistrationLost
    } else {
        return None;
    })
}

fn begin_post_commit_compound(
    world: &mut World,
    entry: &mut RouterEntry,
    cause: PostCommitAuthorityCause,
) -> bool {
    if entry.post_commit_compound.is_some() {
        return true;
    }
    let Some(mut original) = entry.cleanup_ledger.take() else {
        return false;
    };
    if matches!(
        original.preflight_post_commit_transfer_source_kind(),
        Err(PostCommitOriginalFreezeFailure::IneligibleSource)
    ) {
        entry.cleanup_ledger = Some(original);
        return false;
    }
    if original.post_commit_transfer_requires_ordinary_settlement(world) {
        let Some(attempt) = original.attempt(world) else {
            entry.cleanup_ledger = Some(original);
            return true;
        };
        if !original.settle(world, attempt) {
            entry.cleanup_ledger = Some(original);
            return true;
        }
    }
    if !entry.observe_cleanup_receipts(&mut original) {
        entry.post_commit_compound = Some(
            PostCommitCompoundHolder::PreAcceptInert {
                physical: PostCommitPreAcceptPhysicalOwner {
                    original,
                    original_freeze: None,
                    authority: None,
                    edge: None,
                },
                evidence: PostCommitPreAcceptInertEvidence::ReceiptObservationMismatch,
            }
            .into(),
        );
        return true;
    }
    let original_freeze = match original.freeze_for_post_commit_transfer(world) {
        Ok(freeze) => freeze,
        Err(PostCommitOriginalFreezeFailure::IneligibleSource) => {
            entry.cleanup_ledger = Some(original);
            return false;
        }
        Err(PostCommitOriginalFreezeFailure::OrdinarySettlementRequired) => {
            entry.cleanup_ledger = Some(original);
            return true;
        }
        Err(PostCommitOriginalFreezeFailure::RetryableCapacity) => {
            entry.cleanup_ledger = Some(original);
            return true;
        }
        Err(PostCommitOriginalFreezeFailure::Permanent(evidence)) => {
            entry.post_commit_compound = Some(
                PostCommitCompoundHolder::PreAcceptInert {
                    physical: PostCommitPreAcceptPhysicalOwner {
                        original,
                        original_freeze: None,
                        authority: None,
                        edge: None,
                    },
                    evidence: PostCommitPreAcceptInertEvidence::Original(evidence),
                }
                .into(),
            );
            return true;
        }
    };
    let (Some(active), Some(host)) = (entry.active.as_ref(), entry.host.as_ref()) else {
        entry.post_commit_compound = Some(
            PostCommitCompoundHolder::PreAcceptInert {
                physical: PostCommitPreAcceptPhysicalOwner {
                    original,
                    original_freeze: Some(original_freeze),
                    authority: None,
                    edge: None,
                },
                evidence: PostCommitPreAcceptInertEvidence::EntryOwnerMissing,
            }
            .into(),
        );
        return true;
    };
    let edge = match FrozenRetiredHostEdge::freeze(world, host) {
        Ok(edge) => edge,
        Err(evidence) => {
            entry.post_commit_compound = Some(
                PostCommitCompoundHolder::PreAcceptInert {
                    physical: PostCommitPreAcceptPhysicalOwner {
                        original,
                        original_freeze: Some(original_freeze),
                        authority: None,
                        edge: None,
                    },
                    evidence: PostCommitPreAcceptInertEvidence::RetiredHostEdge(evidence),
                }
                .into(),
            );
            return true;
        }
    };
    let authority = match PreparedPostCommitAuthorityLedger::freeze_and_reserve(
        world,
        active,
        host,
        &original_freeze,
    ) {
        Ok(authority) => authority,
        Err(PostCommitAuthorityFreezeFailure::RetryableCapacity) => {
            entry.cleanup_ledger = Some(original);
            return true;
        }
        Err(PostCommitAuthorityFreezeFailure::Permanent(evidence)) => {
            entry.post_commit_compound = Some(
                PostCommitCompoundHolder::PreAcceptInert {
                    physical: PostCommitPreAcceptPhysicalOwner {
                        original,
                        original_freeze: Some(original_freeze),
                        authority: None,
                        edge: Some(edge),
                    },
                    evidence: PostCommitPreAcceptInertEvidence::Authority(evidence),
                }
                .into(),
            );
            return true;
        }
    };
    let Some(normal_batch) = original.batch() else {
        entry.post_commit_compound = Some(
            PostCommitCompoundHolder::PreAcceptInert {
                physical: PostCommitPreAcceptPhysicalOwner {
                    original,
                    original_freeze: Some(original_freeze),
                    authority: Some(authority),
                    edge: Some(edge),
                },
                evidence: PostCommitPreAcceptInertEvidence::MissingNormalBatch,
            }
            .into(),
        );
        return true;
    };
    let transfer = match entry
        .engine
        .accept_post_commit_authority_transfer(normal_batch, cause)
    {
        PostCommitAuthorityAcceptanceOutcome::Accepted(transfer) => transfer,
        PostCommitAuthorityAcceptanceOutcome::Mismatch { expected, observed } => {
            entry.post_commit_compound = Some(
                PostCommitCompoundHolder::PreAcceptInert {
                    physical: PostCommitPreAcceptPhysicalOwner {
                        original,
                        original_freeze: Some(original_freeze),
                        authority: Some(authority),
                        edge: Some(edge),
                    },
                    evidence: PostCommitPreAcceptInertEvidence::CoreReceiver { expected, observed },
                }
                .into(),
            );
            return true;
        }
    };

    let active = entry
        .active
        .take()
        .expect("the accepted post-commit receiver retains active physical ownership");
    let (host, lease) = entry
        .host
        .take()
        .expect("the accepted post-commit receiver retains host ownership")
        .into_retired();
    let logical_roots = active
        .frames()
        .iter()
        .rev()
        .map(|frame| frame.frame().clone())
        .collect::<Vec<_>>();
    let physical = PreparedPostCommitPhysicalOwner {
        original,
        original_freeze,
        authority,
        active,
        host,
        edge,
        input_isolation: None,
    };
    let source = PostCommitAuthorityPreparationSource::new(transfer, lease);
    let (prepared, _authority_batch) = match entry.engine.prepare_post_commit_authority_transfer(
        source,
        CleanupManifest::new(
            logical_roots,
            physical.authority.manifest_digest(),
            physical.authority.obligation_count(),
        ),
    ) {
        PostCommitAuthorityPreparationOutcome::Prepared { owner, batch } => (owner, batch),
        PostCommitAuthorityPreparationOutcome::Mismatch { source, evidence } => {
            entry.post_commit_compound = Some(
                PostCommitCompoundHolder::AcceptedInert {
                    source,
                    physical: Box::new(physical),
                    evidence: Box::new(PostCommitAcceptedInertEvidence::Receiver(evidence)),
                }
                .into(),
            );
            return true;
        }
        PostCommitAuthorityPreparationOutcome::Retryable { source, evidence } => {
            entry.post_commit_compound = Some(
                PostCommitCompoundHolder::AcceptedInert {
                    source,
                    physical: Box::new(physical),
                    evidence: Box::new(PostCommitAcceptedInertEvidence::Retryable(evidence)),
                }
                .into(),
            );
            return true;
        }
        PostCommitAuthorityPreparationOutcome::Permanent { source, evidence } => {
            entry.post_commit_compound = Some(
                PostCommitCompoundHolder::AcceptedInert {
                    source,
                    physical: Box::new(physical),
                    evidence: Box::new(PostCommitAcceptedInertEvidence::Permanent(evidence)),
                }
                .into(),
            );
            return true;
        }
    };
    let mut physical = physical;
    physical.input_isolation = match prepare_post_commit_input_isolation(
        world,
        &physical.active,
        &physical.original_freeze,
        &physical.authority,
    ) {
        Ok(isolation) => Some(isolation),
        Err(evidence) => {
            entry.post_commit_compound = Some(
                PostCommitCompoundHolder::PreparedInert {
                    owner: prepared,
                    physical,
                    evidence: PostCommitPlanBindFailure::Isolation(evidence),
                }
                .into(),
            );
            return true;
        }
    };

    let mut installed = None;
    let outcome = entry.engine.begin_post_commit_authority_transfer(
        prepared,
        Box::new(physical),
        |mut physical, observed_batch, builder| {
            let plan = match bind_post_commit_settlement_plan(
                &physical.original_freeze,
                physical.authority.partition(),
                &physical.edge,
                builder,
            ) {
                Ok(plan) => plan,
                Err(evidence) => {
                    return Err((physical, PostCommitPlanBindFailure::Plan(evidence)));
                }
            };
            if !physical.authority.batch_matches(observed_batch) {
                return Err((
                    physical,
                    PostCommitPlanBindFailure::Authority(
                        crate::cleanup::PreparedAuthorityLedgerBindingMismatch::WrongBatch,
                    ),
                ));
            }
            let Some(isolation) = physical.input_isolation.take() else {
                return Err((
                    physical,
                    PostCommitPlanBindFailure::Isolation(
                        PostCommitInputIsolationFailure::RefreezeRejected,
                    ),
                ));
            };
            let Some(original_removed) = physical
                .original_freeze
                .removed_entities_for_authority_refreeze(world, &physical.original, None)
            else {
                return Err((
                    physical,
                    PostCommitPlanBindFailure::Isolation(
                        PostCommitInputIsolationFailure::RefreezeRejected,
                    ),
                ));
            };
            let input_isolation = match isolation.apply_guarded(world, &|world| {
                physical
                    .original_freeze
                    .refreeze(world, &physical.original, None)
                    && physical
                        .authority
                        .partition()
                        .refreeze_allowing_input_isolation(world, &original_removed)
                    && physical.edge.host_projection_is_current(world, false)
            }) {
                Ok(isolation) => isolation,
                Err(evidence) => {
                    return Err((physical, PostCommitPlanBindFailure::Isolation(evidence)));
                }
            };
            if !physical.authority.commit_input_isolation(
                world,
                &physical.original_freeze,
                &physical.original,
            ) {
                return Err((
                    physical,
                    PostCommitPlanBindFailure::Isolation(
                        PostCommitInputIsolationFailure::RefreezeRejected,
                    ),
                ));
            }
            let PreparedPostCommitPhysicalOwner {
                original,
                original_freeze,
                authority,
                active,
                host,
                edge,
                input_isolation: _,
            } = *physical;
            let authority = authority.bind_verified_batch(observed_batch.clone());
            let (authority, partition) = authority.into_parts();
            Ok((
                Box::new(PostCommitCompoundPhysicalOwner {
                    original,
                    original_freeze,
                    authority,
                    partition,
                    active,
                    host,
                    edge,
                    input_isolation,
                    original_cursor: 0,
                    authority_non_top_cursor: 0,
                    settled_authority: Vec::new(),
                    edge_terminal: None,
                    top_settled: false,
                }),
                plan,
            ))
        },
        |physical, core| installed = Some((physical, core)),
    );
    match outcome {
        PostCommitAuthorityBeginOutcome::Begun => {
            let (physical, core) =
                installed.expect("the infallible install callback publishes the compound owner");
            entry.post_commit_compound = Some(
                PostCommitCompoundHolder::Active(ActivePostCommitCompoundOwner {
                    core,
                    physical: *physical,
                })
                .into(),
            );
        }
        PostCommitAuthorityBeginOutcome::BindingMismatch {
            owner,
            unbound,
            evidence,
        } => {
            entry.post_commit_compound = Some(
                PostCommitCompoundHolder::PreparedInert {
                    owner,
                    physical: *unbound,
                    evidence,
                }
                .into(),
            );
        }
        PostCommitAuthorityBeginOutcome::ReceiverMismatch {
            owner,
            bound,
            evidence,
        } => {
            entry.post_commit_compound = Some(
                PostCommitCompoundHolder::BoundInert {
                    owner,
                    physical: bound,
                    evidence: Box::new(PostCommitBoundInertEvidence::Receiver(evidence)),
                }
                .into(),
            );
        }
        PostCommitAuthorityBeginOutcome::Retryable {
            owner,
            bound,
            evidence,
        } => {
            entry.post_commit_compound = Some(
                PostCommitCompoundHolder::BoundInert {
                    owner,
                    physical: bound,
                    evidence: Box::new(PostCommitBoundInertEvidence::Retryable(evidence)),
                }
                .into(),
            );
        }
        PostCommitAuthorityBeginOutcome::Permanent {
            owner,
            bound,
            evidence,
        } => {
            entry.post_commit_compound = Some(
                PostCommitCompoundHolder::BoundInert {
                    owner,
                    physical: bound,
                    evidence: Box::new(PostCommitBoundInertEvidence::Permanent(evidence)),
                }
                .into(),
            );
        }
    }
    true
}

fn resume_accepted_post_commit_compound(
    world: &mut World,
    entry: &mut RouterEntry,
    source: PostCommitAuthorityPreparationSource,
    physical: PreparedPostCommitPhysicalOwner,
) -> Box<PostCommitCompoundHolder> {
    let logical_roots = physical
        .active
        .frames()
        .iter()
        .rev()
        .map(|frame| frame.frame().clone())
        .collect::<Vec<_>>();
    match entry.engine.prepare_post_commit_authority_transfer(
        source,
        CleanupManifest::new(
            logical_roots,
            physical.authority.manifest_digest(),
            physical.authority.obligation_count(),
        ),
    ) {
        PostCommitAuthorityPreparationOutcome::Prepared { owner, .. } => {
            resume_prepared_post_commit_compound(world, entry, owner, physical)
        }
        PostCommitAuthorityPreparationOutcome::Mismatch { source, evidence } => {
            Box::new(PostCommitCompoundHolder::AcceptedInert {
                source,
                physical: Box::new(physical),
                evidence: Box::new(PostCommitAcceptedInertEvidence::Receiver(evidence)),
            })
        }
        PostCommitAuthorityPreparationOutcome::Retryable { source, evidence } => {
            Box::new(PostCommitCompoundHolder::AcceptedInert {
                source,
                physical: Box::new(physical),
                evidence: Box::new(PostCommitAcceptedInertEvidence::Retryable(evidence)),
            })
        }
        PostCommitAuthorityPreparationOutcome::Permanent { source, evidence } => {
            Box::new(PostCommitCompoundHolder::AcceptedInert {
                source,
                physical: Box::new(physical),
                evidence: Box::new(PostCommitAcceptedInertEvidence::Permanent(evidence)),
            })
        }
    }
}

fn resume_prepared_post_commit_compound(
    world: &mut World,
    entry: &mut RouterEntry,
    owner: PreparedPostCommitAuthorityTransfer,
    mut physical: PreparedPostCommitPhysicalOwner,
) -> Box<PostCommitCompoundHolder> {
    if physical.input_isolation.is_none() {
        physical.input_isolation = match prepare_post_commit_input_isolation(
            world,
            &physical.active,
            &physical.original_freeze,
            &physical.authority,
        ) {
            Ok(isolation) => Some(isolation),
            Err(evidence) => {
                return Box::new(PostCommitCompoundHolder::PreparedInert {
                    owner,
                    physical,
                    evidence: PostCommitPlanBindFailure::Isolation(evidence),
                });
            }
        };
    }

    let mut installed = None;
    let outcome = entry.engine.begin_post_commit_authority_transfer(
        owner,
        Box::new(physical),
        |mut physical, observed_batch, builder| {
            let plan = match bind_post_commit_settlement_plan(
                &physical.original_freeze,
                physical.authority.partition(),
                &physical.edge,
                builder,
            ) {
                Ok(plan) => plan,
                Err(evidence) => {
                    return Err((physical, PostCommitPlanBindFailure::Plan(evidence)));
                }
            };
            if !physical.authority.batch_matches(observed_batch) {
                return Err((
                    physical,
                    PostCommitPlanBindFailure::Authority(
                        crate::cleanup::PreparedAuthorityLedgerBindingMismatch::WrongBatch,
                    ),
                ));
            }
            let Some(isolation) = physical.input_isolation.take() else {
                return Err((
                    physical,
                    PostCommitPlanBindFailure::Isolation(
                        PostCommitInputIsolationFailure::RefreezeRejected,
                    ),
                ));
            };
            let Some(original_removed) = physical
                .original_freeze
                .removed_entities_for_authority_refreeze(world, &physical.original, None)
            else {
                return Err((
                    physical,
                    PostCommitPlanBindFailure::Isolation(
                        PostCommitInputIsolationFailure::RefreezeRejected,
                    ),
                ));
            };
            let input_isolation = match isolation.apply_guarded(world, &|world| {
                physical
                    .original_freeze
                    .refreeze(world, &physical.original, None)
                    && physical
                        .authority
                        .partition()
                        .refreeze_allowing_input_isolation(world, &original_removed)
                    && physical.edge.host_projection_is_current(world, false)
            }) {
                Ok(isolation) => isolation,
                Err(evidence) => {
                    return Err((physical, PostCommitPlanBindFailure::Isolation(evidence)));
                }
            };
            if !physical.authority.commit_input_isolation(
                world,
                &physical.original_freeze,
                &physical.original,
            ) {
                return Err((
                    physical,
                    PostCommitPlanBindFailure::Isolation(
                        PostCommitInputIsolationFailure::RefreezeRejected,
                    ),
                ));
            }
            let PreparedPostCommitPhysicalOwner {
                original,
                original_freeze,
                authority,
                active,
                host,
                edge,
                input_isolation: _,
            } = *physical;
            let authority = authority.bind_verified_batch(observed_batch.clone());
            let (authority, partition) = authority.into_parts();
            Ok((
                Box::new(PostCommitCompoundPhysicalOwner {
                    original,
                    original_freeze,
                    authority,
                    partition,
                    active,
                    host,
                    edge,
                    input_isolation,
                    original_cursor: 0,
                    authority_non_top_cursor: 0,
                    settled_authority: Vec::new(),
                    edge_terminal: None,
                    top_settled: false,
                }),
                plan,
            ))
        },
        |physical, core| installed = Some((physical, core)),
    );
    post_commit_begin_outcome_holder(outcome, installed)
}

fn resume_bound_post_commit_compound(
    entry: &mut RouterEntry,
    owner: PreparedPostCommitAuthorityTransfer,
    physical: PostCommitCompoundPhysicalOwner,
) -> Box<PostCommitCompoundHolder> {
    let mut installed = None;
    let outcome = entry.engine.begin_post_commit_authority_transfer(
        owner,
        Box::new(physical),
        |physical, observed_batch, builder| {
            let plan = match bind_post_commit_settlement_plan(
                &physical.original_freeze,
                &physical.partition,
                &physical.edge,
                builder,
            ) {
                Ok(plan) => plan,
                Err(evidence) => {
                    return Err((physical, PostCommitPlanBindFailure::Plan(evidence)));
                }
            };
            if !physical.authority.post_commit_batch_matches(observed_batch) {
                return Err((
                    physical,
                    PostCommitPlanBindFailure::Authority(
                        crate::cleanup::PreparedAuthorityLedgerBindingMismatch::WrongBatch,
                    ),
                ));
            }
            Ok((physical, plan))
        },
        |physical, core| installed = Some((physical, core)),
    );
    post_commit_bound_retry_outcome_holder(outcome, installed)
}

fn post_commit_begin_outcome_holder(
    outcome: PostCommitAuthorityBeginOutcome<
        Box<PreparedPostCommitPhysicalOwner>,
        Box<PostCommitCompoundPhysicalOwner>,
        PostCommitPlanBindFailure,
    >,
    installed: Option<(
        Box<PostCommitCompoundPhysicalOwner>,
        PostCommitCompoundBatchOwner,
    )>,
) -> Box<PostCommitCompoundHolder> {
    match outcome {
        PostCommitAuthorityBeginOutcome::Begun => {
            let (physical, core) =
                installed.expect("the infallible install callback publishes the compound owner");
            Box::new(PostCommitCompoundHolder::Active(
                ActivePostCommitCompoundOwner {
                    core,
                    physical: *physical,
                },
            ))
        }
        PostCommitAuthorityBeginOutcome::BindingMismatch {
            owner,
            unbound,
            evidence,
        } => Box::new(PostCommitCompoundHolder::PreparedInert {
            owner,
            physical: *unbound,
            evidence,
        }),
        PostCommitAuthorityBeginOutcome::ReceiverMismatch {
            owner,
            bound,
            evidence,
        } => Box::new(PostCommitCompoundHolder::BoundInert {
            owner,
            physical: bound,
            evidence: Box::new(PostCommitBoundInertEvidence::Receiver(evidence)),
        }),
        PostCommitAuthorityBeginOutcome::Retryable {
            owner,
            bound,
            evidence,
        } => Box::new(PostCommitCompoundHolder::BoundInert {
            owner,
            physical: bound,
            evidence: Box::new(PostCommitBoundInertEvidence::Retryable(evidence)),
        }),
        PostCommitAuthorityBeginOutcome::Permanent {
            owner,
            bound,
            evidence,
        } => Box::new(PostCommitCompoundHolder::BoundInert {
            owner,
            physical: bound,
            evidence: Box::new(PostCommitBoundInertEvidence::Permanent(evidence)),
        }),
    }
}

fn post_commit_bound_retry_outcome_holder(
    outcome: PostCommitAuthorityBeginOutcome<
        Box<PostCommitCompoundPhysicalOwner>,
        Box<PostCommitCompoundPhysicalOwner>,
        PostCommitPlanBindFailure,
    >,
    installed: Option<(
        Box<PostCommitCompoundPhysicalOwner>,
        PostCommitCompoundBatchOwner,
    )>,
) -> Box<PostCommitCompoundHolder> {
    match outcome {
        PostCommitAuthorityBeginOutcome::Begun => {
            let (physical, core) =
                installed.expect("the infallible install callback publishes the compound owner");
            Box::new(PostCommitCompoundHolder::Active(
                ActivePostCommitCompoundOwner {
                    core,
                    physical: *physical,
                },
            ))
        }
        PostCommitAuthorityBeginOutcome::BindingMismatch {
            owner,
            unbound,
            evidence,
        } => Box::new(PostCommitCompoundHolder::BoundInert {
            owner,
            physical: unbound,
            evidence: Box::new(PostCommitBoundInertEvidence::Binding(evidence)),
        }),
        PostCommitAuthorityBeginOutcome::ReceiverMismatch {
            owner,
            bound,
            evidence,
        } => Box::new(PostCommitCompoundHolder::BoundInert {
            owner,
            physical: bound,
            evidence: Box::new(PostCommitBoundInertEvidence::Receiver(evidence)),
        }),
        PostCommitAuthorityBeginOutcome::Retryable {
            owner,
            bound,
            evidence,
        } => Box::new(PostCommitCompoundHolder::BoundInert {
            owner,
            physical: bound,
            evidence: Box::new(PostCommitBoundInertEvidence::Retryable(evidence)),
        }),
        PostCommitAuthorityBeginOutcome::Permanent {
            owner,
            bound,
            evidence,
        } => Box::new(PostCommitCompoundHolder::BoundInert {
            owner,
            physical: bound,
            evidence: Box::new(PostCommitBoundInertEvidence::Permanent(evidence)),
        }),
    }
}

fn resume_retryable_post_commit_holder(
    world: &mut World,
    entry: &mut RouterEntry,
    holder: Box<PostCommitCompoundHolder>,
) -> Box<PostCommitCompoundHolder> {
    let retryable = match holder.as_ref() {
        PostCommitCompoundHolder::AcceptedInert { evidence, .. } => evidence.is_retryable(),
        PostCommitCompoundHolder::PreparedInert {
            evidence:
                PostCommitPlanBindFailure::Isolation(PostCommitInputIsolationFailure::RetryableCapacity),
            ..
        } => true,
        PostCommitCompoundHolder::BoundInert { evidence, .. } => evidence.is_retryable(),
        _ => false,
    };
    if !retryable {
        return holder;
    }
    match *holder {
        PostCommitCompoundHolder::AcceptedInert {
            source, physical, ..
        } => resume_accepted_post_commit_compound(world, entry, source, *physical),
        PostCommitCompoundHolder::PreparedInert {
            owner, physical, ..
        } => resume_prepared_post_commit_compound(world, entry, owner, physical),
        PostCommitCompoundHolder::BoundInert {
            owner, physical, ..
        } => resume_bound_post_commit_compound(entry, owner, *physical),
        _ => unreachable!("only a retryable post-accept holder is resumed"),
    }
}

fn settle_post_commit_retired_host_edge(
    world: &mut World,
    physical: &PostCommitCompoundPhysicalOwner,
) -> Result<RetiredHostEdgePhysicalProof, PostCommitAuthoritySettlementFailure> {
    let edge = &physical.edge;
    let original_removed = physical
        .original_freeze
        .removed_entities_for_authority_refreeze(world, &physical.original, None)
        .ok_or(PostCommitAuthoritySettlementFailure::PrewriteDrift)?;
    if physical.edge_terminal.is_some()
        || !physical.input_isolation.is_current(world)
        || !physical
            .original_freeze
            .terminals_are_current(world, &physical.original)
        || physical.partition.top_target().is_none_or(|top| {
            top.entity() != edge.top_slot || top.registered_claim() != Some(&edge.top_slot_claim)
        })
        || !physical.partition.refreeze(
            world,
            &physical.settled_authority,
            None,
            false,
            &original_removed,
        )
    {
        return Err(PostCommitAuthoritySettlementFailure::PrewriteDrift);
    }
    let terminal = if !edge.host_present {
        if !edge.host_projection_is_current(world, false) {
            return Err(PostCommitAuthoritySettlementFailure::PrewriteDrift);
        }
        RetiredHostEdgePhysicalTerminal::HostAbsent
    } else if edge.top_parent_before != Some(edge.host) {
        if edge.children_before != edge.children_after
            || !edge.host_projection_is_current(world, false)
        {
            return Err(PostCommitAuthoritySettlementFailure::PrewriteDrift);
        }
        RetiredHostEdgePhysicalTerminal::EdgeAlreadyAbsent
    } else {
        if !edge.host_projection_is_current(world, false)
            || world.get_entity(edge.top_slot).is_err()
        {
            return Err(PostCommitAuthoritySettlementFailure::PrewriteDrift);
        }
        world.entity_mut(edge.top_slot).remove::<ChildOf>();
        world.flush();
        if !edge.host_projection_is_current(world, true)
            || !physical.input_isolation.is_current(world)
            || !physical
                .original_freeze
                .terminals_are_current(world, &physical.original)
            || !physical.partition.refreeze(
                world,
                &physical.settled_authority,
                None,
                true,
                &original_removed,
            )
        {
            return Err(PostCommitAuthoritySettlementFailure::PostwriteDrift);
        }
        RetiredHostEdgePhysicalTerminal::Detached
    };
    let host_edge_detached = terminal == RetiredHostEdgePhysicalTerminal::Detached;
    if !physical
        .original_freeze
        .terminals_are_current(world, &physical.original)
        || !physical.input_isolation.is_current(world)
        || !physical.partition.refreeze(
            world,
            &physical.settled_authority,
            None,
            host_edge_detached,
            &original_removed,
        )
    {
        return Err(PostCommitAuthoritySettlementFailure::PostwriteDrift);
    }
    Ok(RetiredHostEdgePhysicalProof {
        terminal,
        binding: edge.physical_source_binding
            ^ match terminal {
                RetiredHostEdgePhysicalTerminal::Detached => 1,
                RetiredHostEdgePhysicalTerminal::HostAbsent => 2,
                RetiredHostEdgePhysicalTerminal::EdgeAlreadyAbsent => 3,
            },
    })
}

#[inline(never)]
fn drive_post_commit_original_residual(
    world: &mut World,
    entry: &mut RouterEntry,
    mut active: ActivePostCommitCompoundOwner,
) {
    if !active.physical.input_isolation.is_current(world) {
        entry.post_commit_compound = Some(PostCommitCompoundHolder::Inert(active).into());
        return;
    }
    while active.physical.original_cursor < active.physical.original_freeze.indices().len()
        && active.physical.original_freeze.indices()[active.physical.original_cursor]
            .settled_receipt()
            .is_some()
    {
        active.physical.original_cursor += 1;
    }
    let Some(frozen) = active
        .physical
        .original_freeze
        .indices()
        .get(active.physical.original_cursor)
    else {
        entry.post_commit_compound = Some(PostCommitCompoundHolder::Inert(active).into());
        return;
    };
    let obligation = frozen.index();
    let attempt = match entry
        .engine
        .prepare_next_post_commit_settlement_trusted(active.core)
    {
        TrustedPostCommitSettlementPermitOutcome::Prepared(attempt) => attempt,
        TrustedPostCommitSettlementPermitOutcome::Mismatch { owner, evidence } => {
            entry.post_commit_compound = Some(
                PostCommitCompoundHolder::PermitMismatchInert {
                    core: owner,
                    physical: active.physical,
                    evidence,
                }
                .into(),
            );
            return;
        }
    };
    let proof = match active.physical.original.post_commit_original_receipt_proof(
        world,
        &active.physical.original_freeze,
        obligation,
    ) {
        Some(proof) => proof,
        None => match active.physical.original.settle_post_commit_original_target(
            world,
            &active.physical.original_freeze,
            obligation,
            &active.physical.partition,
            &active.physical.settled_authority,
            &|world| active.physical.input_isolation.is_current(world),
        ) {
            Ok(proof) => proof,
            Err(failure) => {
                entry.post_commit_compound = Some(
                    PostCommitCompoundHolder::PermitInert {
                        attempt,
                        physical: active.physical,
                        failure,
                    }
                    .into(),
                );
                return;
            }
        },
    };
    match entry
        .engine
        .record_next_post_commit_settlement_trusted(attempt, proof)
    {
        TrustedPostCommitSettlementRecordOutcome::Recorded { owner, .. } => {
            active.core = owner;
            active.physical.original_cursor += 1;
            entry.post_commit_compound = Some(PostCommitCompoundHolder::Active(active).into());
        }
        TrustedPostCommitSettlementRecordOutcome::Mismatch {
            attempt,
            physical: proof,
            evidence,
        } => {
            entry.post_commit_compound = Some(
                PostCommitCompoundHolder::SettlementInert(PostCommitSettlementRecordInert {
                    attempt,
                    proof: PostCommitAnyPhysicalProof::Cleanup(proof),
                    physical: active.physical,
                    evidence,
                })
                .into(),
            );
        }
    }
}

#[inline(never)]
fn drive_post_commit_authority_target(
    world: &mut World,
    entry: &mut RouterEntry,
    mut active: ActivePostCommitCompoundOwner,
    top: bool,
) {
    if !active
        .physical
        .original_freeze
        .terminals_are_current(world, &active.physical.original)
        || !active.physical.input_isolation.is_current(world)
    {
        entry.post_commit_compound = Some(PostCommitCompoundHolder::Inert(active).into());
        return;
    }
    let target = if top {
        active.physical.partition.top_target()
    } else {
        active
            .physical
            .partition
            .non_top_targets()
            .nth(active.physical.authority_non_top_cursor)
    };
    let Some(target) = target else {
        entry.post_commit_compound = Some(PostCommitCompoundHolder::Inert(active).into());
        return;
    };
    let target_index = target.index();
    let target_entity = target.entity();
    let host_edge_detached = matches!(
        active.physical.edge_terminal,
        Some(RetiredHostEdgePhysicalTerminal::Detached)
    );
    let attempt = match entry
        .engine
        .prepare_next_post_commit_settlement_trusted(active.core)
    {
        TrustedPostCommitSettlementPermitOutcome::Prepared(attempt) => attempt,
        TrustedPostCommitSettlementPermitOutcome::Mismatch { owner, evidence } => {
            entry.post_commit_compound = Some(
                PostCommitCompoundHolder::PermitMismatchInert {
                    core: owner,
                    physical: active.physical,
                    evidence,
                }
                .into(),
            );
            return;
        }
    };
    let proof = active
        .physical
        .authority
        .settle_post_commit_authority_target(
            world,
            &mut active.physical.partition,
            &active.physical.original,
            &active.physical.original_freeze,
            target_index,
            &active.physical.settled_authority,
            host_edge_detached,
            &|world| active.physical.input_isolation.is_current(world),
        );
    let proof = match proof {
        Ok(proof) => proof,
        Err(failure) => {
            entry.post_commit_compound = Some(
                PostCommitCompoundHolder::PermitInert {
                    attempt,
                    physical: active.physical,
                    failure,
                }
                .into(),
            );
            return;
        }
    };
    match entry
        .engine
        .record_next_post_commit_settlement_trusted(attempt, proof)
    {
        TrustedPostCommitSettlementRecordOutcome::Recorded { owner, .. } => {
            active.core = owner;
            if top {
                active.physical.top_settled = true;
            } else {
                active.physical.authority_non_top_cursor += 1;
            }
            active.physical.settled_authority.push(target_entity);
            entry.post_commit_compound = Some(PostCommitCompoundHolder::Active(active).into());
        }
        TrustedPostCommitSettlementRecordOutcome::Mismatch {
            attempt,
            physical: proof,
            evidence,
        } => {
            entry.post_commit_compound = Some(
                PostCommitCompoundHolder::SettlementInert(PostCommitSettlementRecordInert {
                    attempt,
                    proof: PostCommitAnyPhysicalProof::Cleanup(proof),
                    physical: active.physical,
                    evidence,
                })
                .into(),
            );
        }
    }
}

#[inline(never)]
fn drive_post_commit_retired_host_edge(
    world: &mut World,
    entry: &mut RouterEntry,
    mut active: ActivePostCommitCompoundOwner,
) {
    if !active
        .physical
        .original_freeze
        .terminals_are_current(world, &active.physical.original)
        || !active.physical.input_isolation.is_current(world)
    {
        entry.post_commit_compound = Some(PostCommitCompoundHolder::Inert(active).into());
        return;
    }
    let attempt = match entry
        .engine
        .prepare_next_post_commit_settlement_trusted(active.core)
    {
        TrustedPostCommitSettlementPermitOutcome::Prepared(attempt) => attempt,
        TrustedPostCommitSettlementPermitOutcome::Mismatch { owner, evidence } => {
            entry.post_commit_compound = Some(
                PostCommitCompoundHolder::PermitMismatchInert {
                    core: owner,
                    physical: active.physical,
                    evidence,
                }
                .into(),
            );
            return;
        }
    };
    let proof = match settle_post_commit_retired_host_edge(world, &active.physical) {
        Ok(proof) => proof,
        Err(failure) => {
            entry.post_commit_compound = Some(
                PostCommitCompoundHolder::PermitInert {
                    attempt,
                    physical: active.physical,
                    failure,
                }
                .into(),
            );
            return;
        }
    };
    active.physical.edge_terminal = Some(proof.terminal);
    match entry
        .engine
        .record_next_post_commit_settlement_trusted(attempt, proof)
    {
        TrustedPostCommitSettlementRecordOutcome::Recorded { owner, .. } => {
            active.core = owner;
            entry.post_commit_compound = Some(PostCommitCompoundHolder::Active(active).into());
        }
        TrustedPostCommitSettlementRecordOutcome::Mismatch {
            attempt,
            physical: proof,
            evidence,
        } => {
            entry.post_commit_compound = Some(
                PostCommitCompoundHolder::SettlementInert(PostCommitSettlementRecordInert {
                    attempt,
                    proof: PostCommitAnyPhysicalProof::HostEdge(proof),
                    physical: active.physical,
                    evidence,
                })
                .into(),
            );
        }
    }
}

#[inline(never)]
fn drive_post_commit_completion(
    world: &World,
    entry: &mut RouterEntry,
    active: ActivePostCommitCompoundOwner,
) {
    if !active.physical.original.is_settled()
        || !active
            .physical
            .original_freeze
            .terminals_are_current(world, &active.physical.original)
        || !active.physical.input_isolation.is_current(world)
        || !active.physical.authority.authority_completion_is_exact()
        || !active.physical.top_settled
        || active.physical.edge_terminal.is_none()
    {
        entry.post_commit_compound = Some(PostCommitCompoundHolder::Inert(active).into());
        return;
    }
    let completion = active
        .physical
        .original
        .pending_completion()
        .expect("the exact original ledger retains its Core completion");
    match entry
        .engine
        .prepare_post_commit_compound_completion_trusted(active.core, completion, active.physical)
    {
        TrustedPostCommitCompoundCompletionPreparationOutcome::Prepared { owner, physical } => {
            match entry
                .engine
                .complete_post_commit_authority_compound_trusted(
                    owner,
                    physical,
                    |physical, _expected: RetiredHostAuthorityExpectedView| {
                        let PostCommitCompoundPhysicalOwner {
                            original,
                            authority,
                            active,
                            host,
                            edge,
                            edge_terminal,
                            ..
                        } = physical;
                        drop((original, authority, active));
                        RetiredHostPhysicalTerminal {
                            host,
                            edge,
                            edge_terminal: edge_terminal
                                .expect("joint completion consumes the exact H terminal fact"),
                        }
                    },
                ) {
                TrustedPostCommitCompoundCompletionOutcome::Terminal {
                    terminal,
                    physical,
                    retired_host,
                } => {
                    entry.settled_retired_host_evidence = Some(SettledRetiredHostEvidence {
                        host: physical.host,
                        edge: physical.edge,
                        edge_terminal: physical.edge_terminal,
                        terminal,
                        retired_host,
                    });
                    entry.post_commit_compound = None;
                }
                TrustedPostCommitCompoundCompletionOutcome::Mismatch {
                    owner,
                    physical,
                    evidence,
                } => {
                    entry.post_commit_compound = Some(
                        PostCommitCompoundHolder::CompletionInert {
                            owner,
                            physical,
                            evidence,
                        }
                        .into(),
                    );
                }
            }
        }
        TrustedPostCommitCompoundCompletionPreparationOutcome::Mismatch {
            batch,
            physical,
            inputs,
            evidence,
        } => {
            entry.post_commit_compound = Some(
                PostCommitCompoundHolder::CompletionPreparationInert {
                    core: batch,
                    physical,
                    inputs,
                    evidence,
                }
                .into(),
            );
        }
    }
}

fn drive_post_commit_compound(world: &mut World, entry: &mut RouterEntry) -> bool {
    if entry.post_commit_compound.is_none() {
        let Some(host) = entry.host.as_ref() else {
            return false;
        };
        let Some(cause) = post_commit_authority_loss_cause(world, host) else {
            return false;
        };
        if !begin_post_commit_compound(world, entry, cause) {
            return false;
        }
    }

    let budget = entry
        .post_commit_compound
        .as_ref()
        .and_then(|holder| match holder.as_ref() {
            PostCommitCompoundHolder::Active(active) => Some(
                active.physical.original_freeze.indices().len()
                    + active.physical.partition.targets().len()
                    + 4,
            ),
            _ => None,
        })
        .unwrap_or(1);
    for _ in 0..budget {
        let Some(holder) = entry.post_commit_compound.take() else {
            return true;
        };
        let holder = resume_retryable_post_commit_holder(world, entry, holder);
        let PostCommitCompoundHolder::Active(active) = *holder else {
            entry.post_commit_compound = Some(holder);
            return true;
        };
        match active.core.settlement().progress() {
            PostCommitSettlementTraceProgress::OriginalResidual => {
                drive_post_commit_original_residual(world, entry, active);
            }
            PostCommitSettlementTraceProgress::AuthorityNonTop => {
                drive_post_commit_authority_target(world, entry, active, false);
            }
            PostCommitSettlementTraceProgress::RetiredHostEdge => {
                drive_post_commit_retired_host_edge(world, entry, active);
            }
            PostCommitSettlementTraceProgress::AuthorityTop => {
                drive_post_commit_authority_target(world, entry, active, true);
            }
            PostCommitSettlementTraceProgress::Complete => {
                drive_post_commit_completion(world, entry, active);
            }
        }
    }
    true
}

pub(crate) fn settle_cleanup_system(world: &mut World) {
    world.resource_scope(
        |world, mut navigation: bevy_ecs::change_detection::Mut<RouterNavigation>| {
            for entry in &mut navigation.entries {
                if drive_post_commit_compound(world, entry) {
                    continue;
                }
                if drive_target_preparation_permanent_cleanup(world, entry) {
                    continue;
                }
                if drive_back_no_target_authority_cleanup(world, entry) {
                    continue;
                }
                if drive_back_no_target_zero_completion(entry) {
                    continue;
                }
                settle_branch_cleanup(world, entry);
            }
        },
    );
}

// Keep the ordinary failure/ledger stack separate from the specialized drivers.
// Each entry still executes one branch in the same original ordering.
fn settle_branch_cleanup(world: &mut World, entry: &mut RouterEntry) {
    if let Some(inert) = entry.failure_cleanup_completion.take() {
        if let Err(inert) = fail_closed_failure_cleanup_completion_protocol(world, entry, inert) {
            entry.failure_cleanup_completion = Some(inert);
        }
        return;
    }
    finish_back_target_mount_failure(entry);
    if entry.pending_back_target_execution.is_some() {
        return;
    }
    if entry.failure_cleanup_permanent.is_some() {
        return;
    }
    if let Some(mut failed) = entry.pending_failed_branch.take() {
        if entry.cleanup_ledger.is_some() {
            entry.pending_failed_branch = Some(failed);
            return;
        }
        let terminal_is_current = match &failed.terminal {
            PendingFailedBranchTerminal::Task { task, .. }
            | PendingFailedBranchTerminal::TaskWithPhysicalResidual { task, .. } => {
                entry.engine.inspect_report_identity(task).is_ok()
            }
            PendingFailedBranchTerminal::RegistrationBegin(error) => {
                error.validate_rejection(&entry.engine).is_ok()
            }
            PendingFailedBranchTerminal::Request(_)
            | PendingFailedBranchTerminal::BackTargetPendingRejected { .. } => true,
        };
        if !terminal_is_current {
            entry.pending_failed_branch = Some(failed);
            return;
        }
        let retained_prepared_prefix = failed.retained_prepared_prefix().to_vec();
        if !failed
            .branch
            .retain_prepared_prefix(&retained_prepared_prefix)
        {
            if let Err(failed) = fail_closed_failed_manifest_protocol(
                world,
                entry,
                failed,
                FailedPreparedTopologyError::AmbiguousBoundaryOwnership,
            ) {
                entry.pending_failed_branch = Some(failed);
            }
            return;
        }
        let manifest = {
            #[cfg(feature = "bsn")]
            if let Err(error) = failed.branch.prepare_native_cleanup(world) {
                if error.is_retryable() {
                    entry.pending_failed_branch = Some(failed);
                } else if let Err(failed) = fail_closed_failed_owner_protocol(
                    world,
                    entry,
                    failed,
                    AuthorityProtocolResidualKind::NativeCleanup(error),
                ) {
                    entry.pending_failed_branch = Some(failed);
                }
                return;
            }
            let Some(registry) = world.get_resource::<RouteEntityRegistry>() else {
                entry.pending_failed_branch = Some(failed);
                return;
            };
            FrozenCleanupManifest::from_failed_prepared_branch(world, registry, &failed.branch)
                .and_then(|mut manifest| {
                    #[cfg(feature = "bsn")]
                    if let Some(slot) =
                        exact_current_task_outlet(&entry.engine, &failed, entry.active.as_ref())
                    {
                        manifest.observe_current_task_outlet(world, registry, slot)?;
                    }
                    Ok(manifest)
                })
        };
        let manifest = match manifest {
            Ok(manifest) => manifest,
            Err(FailedManifestBuildError::InvalidTopology(kind)) => {
                if let Err(failed) =
                    fail_closed_failed_manifest_protocol(world, entry, failed, kind)
                {
                    entry.pending_failed_branch = Some(failed);
                }
                return;
            }
            Err(FailedManifestBuildError::MountParentFreeze(_typed_error)) => {
                if let Err(failed) = fail_closed_failed_manifest_protocol(
                    world,
                    entry,
                    failed,
                    FailedPreparedTopologyError::AmbiguousBoundaryOwnership,
                ) {
                    entry.pending_failed_branch = Some(failed);
                }
                return;
            }
            Err(FailedManifestBuildError::RetryableCapacity) => {
                entry.pending_failed_branch = Some(failed);
                return;
            }
        };
        let (failed, manifest) = match classify_failed_manifest(
            world,
            &entry.engine,
            manifest,
            failed,
            entry.active.as_ref(),
            entry.host.as_ref(),
        ) {
            FailedManifestDisposition::Ready { failed, manifest } => (failed, manifest),
            FailedManifestDisposition::CurrentAuthorityLoss {
                failed,
                manifest,
                bindings,
            } => {
                match begin_failed_authority_settlement(world, entry, failed, manifest, bindings) {
                    #[cfg(feature = "bsn")]
                    FailedAuthoritySettlementBeginOutcome::NativeCleanupContradiction {
                        failed,
                        error,
                    } => {
                        if let Err(failed) = fail_closed_failed_owner_protocol(
                            world,
                            entry,
                            failed,
                            AuthorityProtocolResidualKind::NativeCleanup(error),
                        ) {
                            entry.pending_failed_branch = Some(failed);
                        }
                    }
                    FailedAuthoritySettlementBeginOutcome::Begun => {}
                    FailedAuthoritySettlementBeginOutcome::Retryable(failed) => {
                        entry.pending_failed_branch = Some(failed);
                    }
                    FailedAuthoritySettlementBeginOutcome::TriggerContradiction {
                        failed,
                        kind,
                    } => {
                        if let Err(failed) = fail_closed_failed_owner_protocol(
                            world,
                            entry,
                            failed,
                            AuthorityProtocolResidualKind::AuthorityTrigger(kind),
                        ) {
                            entry.pending_failed_branch = Some(failed);
                        }
                    }
                    FailedAuthoritySettlementBeginOutcome::ProtocolContradiction {
                        failed,
                        kind,
                    } => {
                        if let Err(failed) =
                            fail_closed_failed_authority_protocol(world, entry, failed, kind)
                        {
                            entry.pending_failed_branch = Some(failed);
                        }
                    }
                    FailedAuthoritySettlementBeginOutcome::IsolationContradiction {
                        failed,
                        contradiction,
                        isolation,
                    } => {
                        if let Err(failed) = fail_closed_failed_owner_protocol_prepared(
                            world,
                            entry,
                            failed,
                            AuthorityProtocolResidualKind::AuthorityIsolation(
                                contradiction.clone(),
                            ),
                            isolation,
                            Some(contradiction),
                        ) {
                            entry.pending_failed_branch = Some(failed);
                        }
                    }
                    FailedAuthoritySettlementBeginOutcome::LedgerContradiction { failed, kind } => {
                        if let Err(failed) = fail_closed_failed_owner_protocol(
                            world,
                            entry,
                            failed,
                            AuthorityProtocolResidualKind::CleanupLedger(kind),
                        ) {
                            entry.pending_failed_branch = Some(failed);
                        }
                    }
                }
                return;
            }
            FailedManifestDisposition::AmbiguousInvalidTopology { failed, manifest } => {
                let _bounded_contradiction_evidence = manifest;
                if let Err(failed) = fail_closed_failed_manifest_protocol(
                    world,
                    entry,
                    failed,
                    FailedPreparedTopologyError::AmbiguousBoundaryOwnership,
                ) {
                    entry.pending_failed_branch = Some(failed);
                }
                return;
            }
        };
        let preflight_before_isolation = manifest.preflight(world);
        if !preflight_before_isolation {
            entry.pending_failed_branch = Some(failed);
            return;
        }
        let isolated =
            isolate_failed_prepared_branch(world, &failed.branch, &manifest, entry.active.as_ref());
        if !isolated {
            entry.pending_failed_branch = Some(failed);
            return;
        }
        let preflight_after_isolation = manifest.preflight(world);
        if !preflight_after_isolation {
            entry.pending_failed_branch = Some(failed);
            return;
        }
        let partition = {
            let Some(registry) = world.get_resource::<RouteEntityRegistry>() else {
                entry.pending_failed_branch = Some(failed);
                return;
            };
            let Some(host) = entry.host.as_ref() else {
                if let Err(failed) = fail_closed_failed_manifest_protocol(
                    world,
                    entry,
                    failed,
                    FailedPreparedTopologyError::AmbiguousBoundaryOwnership,
                ) {
                    entry.pending_failed_branch = Some(failed);
                }
                return;
            };
            manifest.freeze_failure_cleanup_partition_retaining(
                world,
                registry,
                entry.active.as_ref(),
                failed
                    .branch
                    .retained_prepared_prefix()
                    .map(PreparedBranch::as_realized),
                host,
            )
        };
        let Some(partition) = partition else {
            if let Err(failed) = fail_closed_failed_manifest_protocol(
                world,
                entry,
                failed,
                FailedPreparedTopologyError::AmbiguousBoundaryOwnership,
            ) {
                entry.pending_failed_branch = Some(failed);
            }
            return;
        };
        let manifest_digest = manifest.digest();
        let obligation_count = manifest.len();
        let reserved = match CleanupLedger::reserve(manifest) {
            Ok(reserved) => reserved.bind_failure_cleanup_partition(partition),
            Err(error) => match error.kind() {
                CleanupLedgerReserveErrorKind::RetryableCapacity => {
                    entry.pending_failed_branch = Some(failed);
                    return;
                }
                kind @ (CleanupLedgerReserveErrorKind::DomainExhausted
                | CleanupLedgerReserveErrorKind::UnfinalizedBoundaryFacts) => {
                    let _returned_manifest = error.into_manifest();
                    if let Err(failed) = fail_closed_failed_owner_protocol(
                        world,
                        entry,
                        failed,
                        AuthorityProtocolResidualKind::CleanupLedger(kind),
                    ) {
                        entry.pending_failed_branch = Some(failed);
                    }
                    return;
                }
            },
        };
        #[cfg(feature = "bsn")]
        if let Err(error) = prepare_native_failure_transfer(
            world,
            &mut entry.native_residuals,
            failed.branch.native_failure(),
        ) {
            match error {
                NativeFailureTransferError::Capacity => entry.pending_failed_branch = Some(failed),
                NativeFailureTransferError::Isolation(reason) => {
                    if let Err(failed) = fail_closed_failed_owner_protocol(
                        world,
                        entry,
                        failed,
                        AuthorityProtocolResidualKind::NativeCreation(reason),
                    ) {
                        entry.pending_failed_branch = Some(failed);
                    }
                }
            }
            return;
        }
        let PendingFailedBranch {
            terminal,
            mut branch,
        } = failed;
        let (request, preflight_evidence) = match terminal {
            PendingFailedBranchTerminal::Task {
                task,
                failure,
                message,
            } => match entry.engine.report_failed(task, failure, message) {
                Ok(Some(request)) => (request, None),
                Ok(None) if obligation_count == 0 => {
                    #[cfg(feature = "bsn")]
                    complete_native_failure_transfer(
                        &mut entry.native_residuals,
                        branch.take_native_failure(),
                    );
                    return;
                }
                Ok(None) | Err(_) => {
                    unreachable!(
                        "an exact failed branch with physical obligations must issue cleanup"
                    )
                }
            },
            PendingFailedBranchTerminal::TaskWithPhysicalResidual {
                task,
                failure,
                message,
            } => (
                entry
                    .engine
                    .report_failed_with_physical_residual(task, failure, message)
                    .expect("an exact isolated physical residual retains its report task"),
                None,
            ),
            PendingFailedBranchTerminal::Request(request) => (request, None),
            PendingFailedBranchTerminal::BackTargetPendingRejected { request, evidence } => {
                (request, Some(evidence))
            }
            PendingFailedBranchTerminal::RegistrationBegin(error) => {
                match error.reject(&mut entry.engine) {
                    Ok(request) => (request, None),
                    Err(error) => {
                        entry.pending_failed_branch = Some(PendingFailedBranch {
                            terminal: PendingFailedBranchTerminal::RegistrationBegin(error),
                            branch,
                        });
                        return;
                    }
                }
            }
        };
        let observed_manifest_digest = freeze_failure_cleanup_observed_physical_manifest_digest(
            world,
            request.request(),
            manifest_digest,
        );
        let observed = CleanupManifest::new(
            request.logical_roots().to_vec(),
            observed_manifest_digest,
            obligation_count,
        );
        let physical = FailureCleanupPhysicalPayload {
            branch,
            reserved,
            preflight: preflight_evidence,
            #[cfg(test)]
            witness: issue_failure_cleanup_physical_witness(world, request.request()),
        };
        let physical = entry
            .engine
            .bind_failure_cleanup_physical(&request, observed, physical);
        let mut logical_roots = request.logical_roots().to_vec();
        inject_failure_cleanup_permanent_logical_root_contradiction(
            world,
            request.request(),
            &mut logical_roots,
            manifest_digest,
        );
        match entry.engine.prepare_failure_cleanup_with_physical(
            request,
            CleanupManifest::new(logical_roots, manifest_digest, obligation_count),
            physical,
        ) {
            FailureCleanupPhysicalPrepareOutcome::Prepared(prepared) => {
                let (batch, physical) = prepared.into_parts();
                entry.cleanup_ledger = Some(physical.into_ledger(batch));
            }
            FailureCleanupPhysicalPrepareOutcome::Mismatch(mismatch) => {
                entry.failure_cleanup_permanent = Some(
                    FailureCleanupPermanentStableInertOwner::PrepareMismatch(mismatch),
                );
                return;
            }
            FailureCleanupPhysicalPrepareOutcome::Permanent(permanent) => {
                record_failure_cleanup_permanent_prepare(world, permanent.request());
                install_failure_cleanup_permanent(world, entry, permanent);
                return;
            }
        }
    }
    #[cfg(test)]
    inject_failure_cleanup_prewrite_drift(world, entry);
    let Some(mut ledger) = entry.cleanup_ledger.take() else {
        return;
    };
    if ledger.is_stopped() {
        if let Err(ledger) = fail_closed_cleanup_settlement_permanent(world, entry, ledger) {
            entry.cleanup_ledger = Some(ledger);
        }
        return;
    }
    if !entry.observe_cleanup_receipts(&mut ledger) {
        entry.cleanup_ledger = Some(ledger);
        return;
    }
    if !ledger.is_settled() {
        let Some(attempt) = ledger.attempt(world) else {
            if ledger.is_stopped() {
                if let Err(ledger) = fail_closed_cleanup_settlement_permanent(world, entry, ledger)
                {
                    entry.cleanup_ledger = Some(ledger);
                }
            } else {
                entry.cleanup_ledger = Some(ledger);
            }
            return;
        };
        let settled = if ledger.is_failed_prepared() {
            let source = FailureCleanupSourceProjection::observe_with_prepared(
                entry.host.as_ref(),
                entry.active.as_ref(),
                ledger
                    .failed_prepared_branch()
                    .and_then(FailedPreparedBranch::retained_prepared_prefix)
                    .map(PreparedBranch::as_realized),
            );
            ledger.settle_failure_cleanup(world, attempt, &source)
        } else {
            ledger.settle(world, attempt)
        };
        if !entry.observe_cleanup_receipts(&mut ledger) {
            entry.cleanup_ledger = Some(ledger);
            return;
        }
        if !settled {
            if ledger.is_stopped() {
                if let Err(ledger) = fail_closed_cleanup_settlement_permanent(world, entry, ledger)
                {
                    entry.cleanup_ledger = Some(ledger);
                }
            } else {
                entry.cleanup_ledger = Some(ledger);
            }
            return;
        }
    }
    let completion = ledger
        .pending_completion()
        .expect("successful settlement has a completion");
    #[cfg(feature = "bsn")]
    let Some(mut ledger) = prepare_native_completed_ledger_transfer(world, entry, ledger) else {
        return;
    };
    if ledger.is_authority_settlement() {
        #[cfg(feature = "bsn")]
        let native_residuals = &mut entry.native_residuals;
        match entry.engine.complete_authority_cleanup(completion, || {
            let mut owner = ledger
                .take_authority_settlement_owner()
                .expect("authority publication retains its composed owner");
            #[cfg(feature = "bsn")]
            complete_native_failure_transfer(native_residuals, owner.take_native_failure());
        }) {
            Ok(()) => {}
            Err(_) => entry.cleanup_ledger = Some(ledger),
        }
        return;
    }
    if ledger.is_failed_prepared() {
        if entry.pending_commit.is_some() {
            entry.cleanup_ledger = Some(ledger);
            return;
        }
        #[cfg(test)]
        if entry.failure_cleanup_completion_preparation_drift_armed {
            entry.failure_cleanup_completion_preparation_drift_armed = false;
            entry
                .engine
                .drift_failure_cleanup_completion_receiver_for_test();
        }
        let owner = match entry.engine.prepare_failure_cleanup_completion(completion) {
            Ok(owner) => owner,
            Err(error) => {
                let inert = FailureCleanupCompletionInertOwner {
                    owner: None,
                    ledger,
                    mismatch: FailureCleanupCompletionMismatchView::Preparation(
                        error.into_mismatch(),
                    ),
                };
                if let Err(inert) =
                    fail_closed_failure_cleanup_completion_protocol(world, entry, inert)
                {
                    entry.failure_cleanup_completion = Some(inert);
                }
                return;
            }
        };
        #[cfg(test)]
        if entry.failure_cleanup_completion_receiver_drift_armed {
            entry.failure_cleanup_completion_receiver_drift_armed = false;
            entry
                .engine
                .drift_failure_cleanup_completion_receiver_for_test();
        }
        let mut retained_prepared_prefix = None;
        #[cfg(feature = "bsn")]
        let native_residuals = &mut entry.native_residuals;
        match entry.engine.complete_prepared_failure_cleanup_with_ledger(
            owner,
            ledger,
            |mut ledger| {
                let completed = ledger
                    .take_failed_prepared_owner()
                    .expect("failure continuation retains its physical owner")
                    .into_completed();
                retained_prepared_prefix = completed.retained_prepared_prefix;
                #[cfg(feature = "bsn")]
                complete_native_failure_transfer(native_residuals, completed.native_failure);
            },
        ) {
            Ok(FailureCleanupCompletionDisposition::RetainedPreparedPrefixBound) => {
                let branch = retained_prepared_prefix
                    .expect("a bound retained-prefix continuation keeps its physical owner");
                entry.pending_commit = Some(PendingCommit {
                    ticket: None,
                    final_task: None,
                    retained_hit: None,
                    rejected_retained_publication: None,
                    branch,
                });
                bind_post_cleanup_back_resolution(entry);
            }
            Ok(FailureCleanupCompletionDisposition::RetainedPreparedPrefixCleanup) => {
                let branch = retained_prepared_prefix
                    .expect("a retained-prefix cleanup continuation keeps its physical owner");
                let Some(EngineAction::FailureCleanup(request)) = entry.engine.action() else {
                    unreachable!(
                        "a typed retained-prefix cleanup disposition owns its cleanup request"
                    )
                };
                entry.pending_failed_branch = Some(PendingFailedBranch {
                    terminal: PendingFailedBranchTerminal::Request(request),
                    branch: FailedPreparedBranch::new(branch),
                });
                bind_post_cleanup_back_resolution(entry);
            }
            Ok(FailureCleanupCompletionDisposition::Settled) => {
                assert!(
                    retained_prepared_prefix.is_none(),
                    "a settled continuation cannot return a retained physical prefix",
                );
                bind_post_cleanup_back_resolution(entry);
            }
            Err(error) => {
                let (owner, ledger, mismatch) = error.into_parts();
                let inert = FailureCleanupCompletionInertOwner {
                    owner: Some(owner),
                    ledger,
                    mismatch: FailureCleanupCompletionMismatchView::Completion(mismatch),
                };
                if let Err(inert) =
                    fail_closed_failure_cleanup_completion_protocol(world, entry, inert)
                {
                    entry.failure_cleanup_completion = Some(inert);
                }
            }
        }
        return;
    }
    match entry.engine.complete_cleanup(completion) {
        Ok(()) | Err(IntegrationError::Duplicate) => {
            bind_post_cleanup_back_resolution(entry);
        }
        Err(_) => entry.cleanup_ledger = Some(ledger),
    }
}

pub(crate) fn publish_snapshots_system(world: &mut World) {
    world.resource_scope(
        |world, mut navigation: bevy_ecs::change_detection::Mut<RouterNavigation>| {
            world.resource_scope(
                |_world, mut snapshots: bevy_ecs::change_detection::Mut<RouterSnapshots>| {
                    snapshots.publish(&mut navigation.entries);
                },
            );
        },
    );
}

#[cfg(test)]
#[path = "navigation_tests.rs"]
pub(crate) mod tests;
