use std::sync::atomic::{AtomicU64, Ordering};

use bevy_ecs::{prelude::World, resource::Resource};
use bevy_input_focus::InputFocus;
use bevy_ui_router_core::{
    integration::{
        BeginAuthoritySettlementErrorKind, CleanupManifest, EngineAction,
        FailedAuthoritySettlementTrigger, FailedAuthorityTriggerError, FailureCleanupRequest,
        FrameMountTarget, HostLease, IntegrationError, MountMutationArtifact, MountReceiptProof,
        MountRegistrationBeginError, MountRequest, ResolveMountOwner, RootFrameTask, RouterEngine,
        ViewFailureKind, ViewReportIdentity,
    },
    prelude::{MountRejectionKind, NavigationRequestId, RouteFrameVersion, RouteLocation},
};
use bevy_ui_router_view::{RouteViewRegistry, ViewBuildEffect, integration as view_integration};

use crate::{
    app_ext::{RouterHandle, ViewDriverStore},
    branch_transaction::{
        FailedPreparedBranch, PendingCommit, PreparedBranch, RealizedBranch,
        ReconciliationAuthorityIdentity, ReconciliationProjectionFacts, ReconciliationSlotRoleKind,
        SealedExistingReconciliation, SealedMountNewStagingAllowance, SealedReconciliationSplice,
        seal_reconciliation_staging_mount,
    },
    cleanup::{
        AuthoritySettlementManifestError, CleanupLedger, CleanupLedgerReserveErrorKind,
        FailedAuthorityBoundaryBinding, FailedBoundaryDrift, FailedBoundarySource,
        FailedManifestBuildError, FailedPreparedTopologyError, FrozenCleanupManifest,
        PendingBoundaryBinding, RouterCleanupObservation, cleanup_claim_has_exact_authority,
        failed_prepared_branch_roots_are_exact,
    },
    content::{ContentPlan, FrameContentWriter},
    driver::{EcsViewReportSession, ReportSeal, ViewBuildReport, ViewBuildReportKind},
    frame_local::{
        validate_active_branch_for_splice, validate_active_branch_local,
        validate_active_branch_with_entering, validate_branch_descendants,
        validate_leaving_branch_for_splice, validate_prepared_branch,
        validate_preserved_branch_local, validate_reconciliation_entering_branch,
    },
    host::{
        HostCandidateClassification, HostCandidateRevisionLatch, HostCandidateRevisionState,
        ProvisionalHostSettlement, ValidatedHostCandidate, activate_granted_top_slot,
        classify_host_candidate, retry_provisional_host_settlement,
        settle_rejected_granted_top_slot, stage_granted_top_slot,
        validated_host_candidate_is_current,
    },
    input::{
        PreparedInputGateIsolation, RouteFocusState, RouterInputGate, observed_descendant_closure,
        prepare_focus_mutation, prepare_input_gate_mutation, try_prepare_input_gate_isolation,
    },
    mount::{
        ChildSlotResidual, StagedChildSlot, confirm_child_slot, mounted_child_slot_is_isolated,
        mounted_child_slot_local_is_isolated, mounted_child_slot_structure_is_isolated,
        residual_child_slot_is_isolated, residual_child_slot_local_is_isolated, stage_child_slot,
    },
    presentation::{
        DeferredRealizationFailure, RealizedFrame, expected_children,
        frame_is_isolated_for_settlement, isolate_frame_for_settlement,
        prepare_presentation_mutation, realize_child_frame_deferred, realize_frame_deferred,
        realize_reconciliation_child_frame_deferred, validate_active_frame,
        validate_realized_frame,
    },
    registry::{
        EntityLifecycle, EntityOwner, ExactRegistryClaim, ProvisionalOwner, RegistryOwner,
        RouteEntityRegistry,
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
    lease: HostLease,
}

pub(crate) struct RetiredHostBinding {
    pub(crate) version: bevy_ui_router_core::HostVersion,
    pub(crate) slot_id: bevy_ui_router_core::ids::RouteSlotId,
    pub(crate) host: bevy_ecs::entity::Entity,
    pub(crate) top_slot: bevy_ecs::entity::Entity,
    pub(crate) top_slot_claim: crate::registry::ExactRegistryClaim,
    pub(crate) candidate: ValidatedHostCandidate,
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
        } = self;
        HostBinding {
            version,
            slot_id,
            host,
            top_slot,
            top_slot_claim,
            candidate,
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
        message: &'static str,
    },
    TaskWithPhysicalResidual {
        task: RootFrameTask,
        failure: ViewFailureKind,
        message: &'static str,
    },
    Request(FailureCleanupRequest),
    RegistrationBegin(MountRegistrationBeginError),
}

pub(crate) struct PendingFailedBranch {
    terminal: PendingFailedBranchTerminal,
    branch: FailedPreparedBranch,
}

impl PendingFailedBranch {
    pub(crate) const fn branch(&self) -> &FailedPreparedBranch {
        &self.branch
    }
}

struct PendingFrameDrive {
    task: RootFrameTask,
    effect: ViewBuildEffect,
    pending: PendingCommit,
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

struct MountNewStagingSeed {
    request: NavigationRequestId,
    parent: RouteFrameVersion,
    outlet: bevy_ui_router_core::slot::RouteSlotKey,
    slot: bevy_ui_router_core::ids::RouteSlotId,
    anchor: bevy_ecs::prelude::Entity,
    anchor_claim: crate::registry::ExactRegistryClaim,
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
        let baseline_anchor_children = expected_children(parent, anchor);
        let actual = world
            .get::<bevy_ecs::hierarchy::Children>(anchor)
            .map(|children| children.iter().copied().collect::<Vec<_>>())
            .unwrap_or_default();
        (outlet_record.parent() == parent.frame()
            && outlet_record.key() == outlet
            && actual == baseline_anchor_children)
            .then(|| Self {
                request,
                parent: parent.frame().clone(),
                outlet: outlet.clone(),
                slot,
                anchor,
                anchor_claim: outlet_record.anchor_claim().clone(),
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
            && residual.anchor_claim() == &self.anchor_claim)
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
            self.anchor_claim,
            expected_anchor_children,
        )
    }
}

pub(crate) struct RouterEntry {
    pub(crate) handle: RouterHandle,
    pub(crate) engine: RouterEngine,
    views: RouteViewRegistry,
    pub(crate) host: Option<HostBinding>,
    host_settlement: Option<ProvisionalHostSettlement>,
    pending_commit: Option<PendingCommit>,
    pending_frame_drive: Option<PendingFrameDrive>,
    pending_mount_frame_task: Option<RootFrameTask>,
    pending_mount_request: Option<MountRequest>,
    pending_mount_mutation_rejection: Option<PendingMountMutationRejection>,
    pending_failed_branch: Option<PendingFailedBranch>,
    projectionless_fail_close: Option<ProjectionlessFailCloseContinuation>,
    pub(crate) active: Option<RealizedBranch>,
    pub(crate) cleanup_ledger: Option<CleanupLedger>,
    cleanup_observation: RouterCleanupObservation,
    pub(crate) authority_protocol_residual: Option<AuthorityProtocolResidualOwner>,
    pub(crate) requests: Vec<NavigationRequestId>,
}

#[derive(Clone, Eq, PartialEq)]
struct ProjectionlessFallbackAdmission {
    frame: RouteFrameVersion,
    mount_target: FrameMountTarget,
    top_slot: bevy_ecs::entity::Entity,
    top_slot_claim: crate::registry::ExactRegistryClaim,
}

#[derive(Clone, Eq, PartialEq)]
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
    gate: Option<(bool, bool, bool)>,
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
    kind: AuthorityProtocolResidualKind,
    isolation_contradiction: Option<AuthorityIsolationContradiction>,
    active: Option<RealizedBranch>,
    pending: Option<PendingCommit>,
    failed: Option<PendingFailedBranch>,
    pub(crate) host: Option<RetiredHostBinding>,
}

enum AuthorityProtocolResidualKind {
    Authority(BeginAuthoritySettlementErrorKind),
    AuthorityIsolation(AuthorityIsolationContradiction),
    CleanupLedger(CleanupLedgerReserveErrorKind),
    AuthorityTrigger(FailedAuthorityTriggerError),
    FailedManifest(FailedPreparedTopologyError),
    PostReportSourceDrift(ProjectionlessSourceDriftEvidence),
    PreflightAuthorityLost(LifecyclePreflightEvidence),
}

impl RouterEntry {
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
        } else {
            false
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
                PendingFailedBranchTerminal::Request(request) => {
                    allowance.matches_request(request.request())
                }
                PendingFailedBranchTerminal::RegistrationBegin(_) => true,
                PendingFailedBranchTerminal::Task { .. }
                | PendingFailedBranchTerminal::TaskWithPhysicalResidual { .. } => false,
            };
            return terminal_is_exact.then_some(allowance);
        }
        self.cleanup_ledger
            .as_ref()
            .and_then(CleanupLedger::mount_new_staging_allowance)
    }

    pub(crate) fn cleanup_summary(&self) -> bevy_ui_router_core::CleanupSummary {
        self.cleanup_observation.summary(
            self.cleanup_ledger.is_some()
                || self.pending_failed_branch.is_some()
                || self.pending_mount_mutation_rejection.is_some()
                || self.host_settlement.is_some(),
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

    #[cfg(feature = "test-support")]
    pub(crate) fn pending_frame(&self) -> Option<&RealizedFrame> {
        self.pending_commit
            .as_ref()
            .and_then(|pending| pending.branch.first_frame())
            .or_else(|| {
                self.pending_frame_drive
                    .as_ref()
                    .and_then(|pending| pending.pending.branch.first_frame())
            })
            .or_else(|| {
                self.pending_mount_mutation_rejection
                    .as_ref()
                    .and_then(|pending| pending.pending.branch.first_frame())
            })
    }

    #[cfg(feature = "test-support")]
    pub(crate) fn pending_frames(&self) -> impl Iterator<Item = &RealizedFrame> {
        self.pending_commit
            .iter()
            .flat_map(|pending| pending.branch.frames())
            .chain(
                self.pending_frame_drive
                    .iter()
                    .flat_map(|pending| pending.pending.branch.frames()),
            )
            .chain(
                self.pending_mount_mutation_rejection
                    .iter()
                    .flat_map(|pending| pending.pending.branch.frames()),
            )
            .chain(
                self.pending_failed_branch
                    .iter()
                    .flat_map(|pending| pending.branch.frames()),
            )
    }

    #[cfg(feature = "test-support")]
    pub(crate) fn leaving_cleanup_frames(&self) -> &[RealizedFrame] {
        self.cleanup_ledger
            .as_ref()
            .map(CleanupLedger::leaving_frames)
            .unwrap_or(&[])
    }

    #[cfg(feature = "test-support")]
    pub(crate) fn failed_cleanup_frames(&self) -> &[RealizedFrame] {
        self.cleanup_ledger
            .as_ref()
            .map(CleanupLedger::failed_prepared_frames)
            .unwrap_or(&[])
    }

    #[cfg(feature = "test-support")]
    pub(crate) fn authority_protocol_residual_frames(&self) -> Vec<&RealizedFrame> {
        self.authority_protocol_residual
            .as_ref()
            .map(|residual| {
                residual
                    .active
                    .iter()
                    .flat_map(RealizedBranch::frames)
                    .chain(
                        residual
                            .pending
                            .iter()
                            .flat_map(|pending| pending.branch.frames()),
                    )
                    .chain(
                        residual
                            .failed
                            .iter()
                            .flat_map(|failed| failed.branch.frames()),
                    )
                    .collect()
            })
            .unwrap_or_default()
    }

    #[cfg(feature = "test-support")]
    pub(crate) fn authority_protocol_residual_entities(&self) -> Vec<bevy_ecs::prelude::Entity> {
        let Some(residual) = self.authority_protocol_residual.as_ref() else {
            return Vec::new();
        };
        let mut entities = residual
            .active
            .iter()
            .flat_map(RealizedBranch::claims)
            .chain(
                residual
                    .pending
                    .iter()
                    .flat_map(|pending| pending.branch.as_realized().claims()),
            )
            .chain(residual.failed.iter().flat_map(|failed| {
                failed
                    .branch
                    .frames()
                    .iter()
                    .flat_map(RealizedFrame::claims)
                    .chain(
                        failed
                            .branch
                            .detached_frames()
                            .iter()
                            .flat_map(RealizedFrame::claims),
                    )
            }))
            .map(ExactRegistryClaim::entity)
            .collect::<Vec<_>>();
        if let Some(host) = &residual.host {
            entities.push(host.top_slot);
        }
        entities.sort_by_key(|entity| entity.to_bits());
        entities.dedup();
        entities
    }

    #[cfg(feature = "test-support")]
    pub(crate) fn tracked_child_slots(&self) -> Vec<crate::branch_transaction::TrackedChildSlot> {
        let mut slots = Vec::new();
        if let Some(pending) = &self.pending_commit {
            slots.extend(pending.branch.tracked_child_slots());
        }
        if let Some(pending) = &self.pending_frame_drive {
            slots.extend(pending.pending.branch.tracked_child_slots());
        }
        if let Some(pending) = &self.pending_mount_mutation_rejection {
            slots.extend(pending.pending.branch.tracked_child_slots());
        }
        if let Some(pending) = &self.pending_failed_branch {
            slots.extend(pending.branch.tracked_child_slots());
        }
        if let Some(active) = &self.active {
            slots.extend(active.tracked_child_slots());
        }
        if let Some(ledger) = &self.cleanup_ledger {
            slots.extend(ledger.tracked_child_slots());
        }
        slots
    }
}

#[derive(Resource)]
pub struct RouterNavigation {
    domain: u64,
    next_router: u64,
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
            entries: Vec::new(),
        }
    }

    pub(crate) fn install(
        &mut self,
        routes: bevy_ui_router_core::prelude::RouteTree,
        views: RouteViewRegistry,
        fallback: bevy_ui_router_core::prelude::RouteViewKey,
    ) -> Result<RouterHandle, IntegrationError> {
        let handle = RouterHandle::from_parts(self.domain, self.next_router);
        self.next_router = self
            .next_router
            .checked_add(1)
            .ok_or(IntegrationError::Exhausted)?;
        let engine = RouterEngine::install(routes, fallback)?;
        self.entries.push(RouterEntry {
            handle,
            engine,
            views,
            host: None,
            host_settlement: None,
            pending_commit: None,
            pending_frame_drive: None,
            pending_mount_frame_task: None,
            pending_mount_request: None,
            pending_mount_mutation_rejection: None,
            pending_failed_branch: None,
            projectionless_fail_close: None,
            active: None,
            cleanup_ledger: None,
            cleanup_observation: RouterCleanupObservation::default(),
            authority_protocol_residual: None,
            requests: Vec::new(),
        });
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
        if entry.authority_protocol_residual.is_some() {
            return Err(NavigationEnqueueError::new(
                NavigationEnqueueErrorKind::HostUnavailable,
            ));
        }
        let request = entry.engine.push(location).map_err(|error| match error {
            IntegrationError::AwaitingHost => {
                NavigationEnqueueError::new(NavigationEnqueueErrorKind::HostUnavailable)
            }
            _ => NavigationEnqueueError::new(NavigationEnqueueErrorKind::AuthorityRecovering),
        })?;
        entry.requests.push(request);
        Ok(request)
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
        })
    }
}

impl std::error::Error for NavigationEnqueueError {}

pub(crate) fn observe_authority_system(world: &mut World) {
    world.resource_scope(|world, mut revisions: bevy_ecs::change_detection::Mut<HostCandidateRevisionState>| {
        world.resource_scope(|world, mut latch: bevy_ecs::change_detection::Mut<HostCandidateRevisionLatch>| {
            world.resource_scope(|world, mut navigation: bevy_ecs::change_detection::Mut<RouterNavigation>| {
                for entry in &mut navigation.entries {
                    if let Some(settlement) = entry.host_settlement.take()
                        && !retry_provisional_host_settlement(world, &settlement)
                    {
                        entry.host_settlement = Some(settlement);
                        continue;
                    }
                    if entry.host.is_some()
                        || entry
                            .cleanup_ledger
                            .as_ref()
                            .is_some_and(CleanupLedger::is_authority_settlement)
                    {
                        continue;
                    }
                    let HostCandidateClassification::Unique(candidate) =
                        classify_host_candidate(world, entry.handle, &mut revisions)
                    else {
                        continue;
                    };
                    let revision = candidate.revision();
                    if !latch.allows(revision) {
                        continue;
                    }
                    let host = candidate.host();
                    let Ok(token) = entry.engine.request_host_admission() else {
                        continue;
                    };
                    let Ok(grant) = entry.engine.grant_host_admission(&token) else {
                        let _ = entry.engine.reject_host_admission(token);
                        continue;
                    };
                    let version = grant.version().clone();
                    let slot_id = grant.top_slot();
                    match stage_granted_top_slot(world, candidate.clone(), &grant) {
                        Ok(staged) => {
                            let top_slot = staged.entity();
                            if activate_granted_top_slot(world, &candidate, &grant, staged) {
                                if entry.engine.record_top_slot_registered(&grant).is_err() {
                                    let _ = settle_rejected_granted_top_slot(
                                        world,
                                        &candidate,
                                        staged,
                                    );
                                    let _ = entry.engine.reject_host_after_mutation(grant);
                                    latch.reject(revision);
                                    continue;
                                }
                                let top_slot_claim = world
                                    .resource::<RouteEntityRegistry>()
                                    .record(top_slot)
                                    .expect("activated top slot remains registered")
                                    .claim();
                                let lease = entry.engine.confirm_host(grant);
                                entry.host = Some(HostBinding {
                                    version,
                                    slot_id,
                                    host,
                                    top_slot,
                                    top_slot_claim,
                                    candidate,
                                    lease,
                                });
                            } else {
                                let _ = settle_rejected_granted_top_slot(
                                    world,
                                    &candidate,
                                    staged,
                                );
                                let _ = entry.engine.reject_host_after_mutation(grant);
                                latch.reject(revision);
                            }
                        }
                        Err(failure) => {
                            let settlement = failure.into_settlement();
                            let _ = entry.engine.reject_host_after_mutation(grant);
                            latch.reject(revision);
                            entry.host_settlement = settlement;
                        }
                    }
                }
            });
        });
    });
    refresh_authority_audits(world);
}

pub(crate) fn collect_navigation_system(world: &mut World) {
    for entry in &mut world.resource_mut::<RouterNavigation>().entries {
        entry.engine.collect();
    }
}

enum DriverReportApplicationError {
    Stale,
    Realization(DeferredRealizationFailure),
}

fn apply_applied_driver_report(
    world: &mut World,
    engine: &mut RouterEngine,
    host_candidate: ValidatedHostCandidate,
    top_slot: bevy_ecs::entity::Entity,
    top_slot_claim: crate::registry::ExactRegistryClaim,
    task: &RootFrameTask,
    report: &ViewBuildReport,
    plan: ContentPlan,
) -> Result<RealizedFrame, DriverReportApplicationError> {
    apply_applied_driver_report_to_branch(
        world,
        engine,
        host_candidate,
        top_slot,
        top_slot_claim,
        None,
        None,
        task,
        report,
        plan,
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
    report: &ViewBuildReport,
    plan: ContentPlan,
) -> Result<RealizedFrame, DriverReportApplicationError> {
    if report.kind() != ViewBuildReportKind::Applied
        || report.identity() != &task.report_identity()
        || engine.inspect_report_identity(task).is_err()
    {
        return Err(DriverReportApplicationError::Stale);
    }

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
            let Some(mount) = seal_reconciliation_staging_mount(&projection, active, branch, task)
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

fn complete_zero_physical_failure(
    world: &mut World,
    entry: &mut RouterEntry,
    request: FailureCleanupRequest,
    branch: FailedPreparedBranch,
) -> Result<(), PendingFailedBranch> {
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
    let logical_roots = request.logical_roots().to_vec();
    let cleanup = match entry.engine.prepare_failure_cleanup_retaining(
        request,
        CleanupManifest::new(logical_roots, manifest_digest, 0),
    ) {
        Ok(cleanup) => cleanup,
        Err(error) => {
            return Err(PendingFailedBranch {
                terminal: PendingFailedBranchTerminal::Request(error.into_owner()),
                branch,
            });
        }
    };
    let mut ledger = CleanupLedger::activate_failed(reserved.bind_batch(cleanup), branch);
    if !ledger.is_settled() {
        let Some(attempt) = ledger.attempt(world) else {
            unreachable!("an exact zero-physical cleanup ledger issues one attempt when needed")
        };
        assert!(ledger.settle(world, attempt));
    }
    let completion = ledger
        .pending_completion()
        .expect("an empty exact cleanup ledger settles immediately");
    entry
        .engine
        .complete_cleanup(completion)
        .expect("an exact empty cleanup completion is current");
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
            message: "route view report application became stale",
        },
        branch: failed_prepared_branch(pending, None),
    }
}

fn nested_failure_terminal(
    engine: &mut RouterEngine,
    task: RootFrameTask,
    failure: ViewFailureKind,
    message: &'static str,
) -> PendingFailedBranchTerminal {
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
    branch
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
        .collect()
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
        .filter(|claim| failed_cleanup_claim_is_current(world, claim))
        .filter(|claim| !active_claims.contains(claim))
        .cloned()
        .collect::<Vec<_>>();
    let requires_exact_isolation =
        |claim: &ExactRegistryClaim| exact_isolation_claims.contains(claim);
    let mut focus_closure = failed_branch_focus_closure(world, &exact_isolation_claims);
    let active_focus_before = match active {
        Some(active) => {
            let Ok(closure) = validate_branch_descendants(world, active) else {
                return false;
            };
            closure
        }
        None => Vec::new(),
    };
    if focus_closure
        .iter()
        .any(|entity| active_focus_before.contains(entity))
    {
        return false;
    }
    let frames_are_isolated = branch
        .frames()
        .iter()
        .chain(branch.detached_frames())
        .all(|frame| isolate_frame_for_settlement(world, frame, &requires_exact_isolation));
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
            let Ok(closure) = validate_branch_descendants(world, active) else {
                return false;
            };
            closure
        }
        None => Vec::new(),
    };
    if revalidated_focus_closure != focus_closure
        || active_focus_after != active_focus_before
        || revalidated_focus_closure
            .iter()
            .any(|entity| active_focus_after.contains(entity))
    {
        return false;
    }
    release_focus_for_closure(world, &focus_closure);
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
            .chain(active.interactive_entities().iter().copied())
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

pub(crate) fn drive_navigation_system(world: &mut World) {
    world.resource_scope(
        |world, mut navigation: bevy_ecs::change_detection::Mut<RouterNavigation>| {
            world.resource_scope(
                |world, mut drivers: bevy_ecs::change_detection::Mut<ViewDriverStore>| {
                    for entry in &mut navigation.entries {
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
                        if entry.pending_failed_branch.is_some() {
                            continue;
                        }
                        if entry.pending_commit.as_ref().is_some_and(|pending| {
                            pending.ticket.is_some() || pending.final_task.is_some()
                        }) {
                            continue;
                        }
                        let (action, retry_context) =
                            if let Some(retry) = entry.pending_frame_drive.take() {
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
                                let expected_child_outlet =
                                    match entry.engine.expected_child_outlet(&task) {
                                        Ok(expected) => expected,
                                        Err(_) => {
                                            entry.pending_failed_branch =
                                                Some(stale_driver_report_owner(pending, task));
                                            continue;
                                        }
                                    };
                                let Some(host) = entry.host.as_ref() else {
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
                                if !validated_host_candidate_is_current(world, &host.candidate) {
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
                                let Some(driver_key) =
                                    view_integration::driver_key(&entry.views, task.view())
                                        .cloned()
                                else {
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
                                let Some(driver) = drivers.get_mut(&driver_key) else {
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
                                        entry.pending_failed_branch = Some(PendingFailedBranch {
                                            terminal: PendingFailedBranchTerminal::Task {
                                                task,
                                                failure: ViewFailureKind::Apply,
                                                message: "route view report identity became stale",
                                            },
                                            branch: failed_prepared_branch(pending, None),
                                        });
                                        continue;
                                    }
                                    let failure = match report.kind() {
                                        ViewBuildReportKind::BuildFailed => ViewFailureKind::Build,
                                        ViewBuildReportKind::ApplyFailed => ViewFailureKind::Apply,
                                        ViewBuildReportKind::Applied => unreachable!(),
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
                                let realized = apply_applied_driver_report_to_branch(
                                    world,
                                    &mut entry.engine,
                                    host.candidate.clone(),
                                    host.top_slot,
                                    host.top_slot_claim.clone(),
                                    (!realizing_root).then_some(&pending.branch),
                                    entry.active.as_ref(),
                                    &task,
                                    &report,
                                    plan,
                                );
                                let realized = match realized {
                                    Ok(realized) => realized,
                                    Err(DriverReportApplicationError::Stale) => {
                                        entry.pending_failed_branch =
                                            Some(stale_driver_report_owner(pending, task));
                                        continue;
                                    }
                                    Err(DriverReportApplicationError::Realization(failure)) => {
                                        let (_error, residual) = failure.into_parts();
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
                                            if realizing_root {
                                                entry.pending_failed_branch =
                                                    Some(PendingFailedBranch {
                                                        terminal: PendingFailedBranchTerminal::TaskWithPhysicalResidual {
                                                        task,
                                                        failure: ViewFailureKind::Apply,
                                                        message: "frame realization rejected",
                                                    },
                                                        branch: failed_branch,
                                                    });
                                            } else {
                                                let terminal = nested_failure_terminal(
                                                    &mut entry.engine,
                                                    task,
                                                    ViewFailureKind::Apply,
                                                    "frame realization rejected",
                                                );
                                                entry.pending_failed_branch =
                                                    Some(PendingFailedBranch {
                                                        terminal,
                                                        branch: failed_branch,
                                                    });
                                            }
                                        } else {
                                            let failure = entry.engine.report_failed(
                                                task,
                                                ViewFailureKind::Apply,
                                                "frame realization rejected",
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
                                    if realizing_root {
                                        entry.pending_failed_branch = Some(PendingFailedBranch {
                                            terminal: PendingFailedBranchTerminal::TaskWithPhysicalResidual {
                                                task,
                                                failure: ViewFailureKind::Apply,
                                                message: "frame branch append rejected",
                                            },
                                            branch: failed_branch,
                                        });
                                    } else {
                                        let terminal = nested_failure_terminal(
                                            &mut entry.engine,
                                            task,
                                            ViewFailureKind::Apply,
                                            "frame branch append rejected",
                                        );
                                        entry.pending_failed_branch = Some(PendingFailedBranch {
                                            terminal,
                                            branch: failed_branch,
                                        });
                                    }
                                    continue;
                                }
                                if expected_child_outlet.is_some() {
                                    let Some(parent) = pending.branch.last_frame() else {
                                        entry.pending_commit = Some(pending);
                                        continue;
                                    };
                                    if !validate_realized_frame(world, parent) {
                                        entry.pending_failed_branch = Some(PendingFailedBranch {
                                            terminal: PendingFailedBranchTerminal::Task {
                                                task,
                                                failure: ViewFailureKind::Apply,
                                                message: "parent frame preflight rejected",
                                            },
                                            branch: failed_prepared_branch(pending, None),
                                        });
                                        continue;
                                    }
                                    if entry
                                        .engine
                                        .report_applied_with_outlets(
                                            task,
                                            realized_outlet_keys(parent),
                                        )
                                        .is_err()
                                    {
                                        let terminal = match entry.engine.action() {
                                            Some(EngineAction::FrameTask(task)) => {
                                                PendingFailedBranchTerminal::Task {
                                                    task,
                                                    failure: ViewFailureKind::Apply,
                                                    message: "parent outlet report rejected",
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
                                        validate_realized_frame(world, parent)
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
                        }
                    }
                },
            );
        },
    );
    refresh_authority_audits(world);
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LifecyclePreflightSource {
    RequiredResources(RequiredResourceSet),
    HostBinding,
    PreservedCurrent,
    LeavingCurrent,
    PendingAdmission,
    PendingEntering,
    CrossRoleRebase,
    TargetPartition,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct RequiredResourceSet(u8);

impl RequiredResourceSet {
    const ROUTE_ENTITY_REGISTRY: Self = Self(1 << 0);
    const ROUTER_INPUT_GATE: Self = Self(1 << 1);
    const INPUT_FOCUS: Self = Self(1 << 2);
    const ROUTE_FOCUS_STATE: Self = Self(1 << 3);

    fn missing_from(world: &World) -> Self {
        let mut missing = Self::default();
        if !world.contains_resource::<RouteEntityRegistry>() {
            missing.0 |= Self::ROUTE_ENTITY_REGISTRY.0;
        }
        if !world.contains_resource::<RouterInputGate>() {
            missing.0 |= Self::ROUTER_INPUT_GATE.0;
        }
        if !world.contains_resource::<InputFocus>() {
            missing.0 |= Self::INPUT_FOCUS.0;
        }
        if !world.contains_resource::<RouteFocusState>() {
            missing.0 |= Self::ROUTE_FOCUS_STATE.0;
        }
        missing
    }

    const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LifecyclePreflightExpectation {
    ResourcePresent,
    ExactAuthority,
    ExactTaskShape,
    ExactRelationshipClosure,
    RebaseCompatible,
    DisjointClosure,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LifecyclePreflightObserved {
    Missing,
    Drifted,
    Contradictory,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LifecyclePreflightEvidence {
    source: LifecyclePreflightSource,
    expectation: LifecyclePreflightExpectation,
    observed: LifecyclePreflightObserved,
}

impl LifecyclePreflightEvidence {
    const fn new(
        source: LifecyclePreflightSource,
        expectation: LifecyclePreflightExpectation,
        observed: LifecyclePreflightObserved,
    ) -> Self {
        Self {
            source,
            expectation,
            observed,
        }
    }

    const fn pending_message(self) -> &'static str {
        match self.source {
            LifecyclePreflightSource::PendingAdmission => {
                "projectionless fallback admission became stale"
            }
            LifecyclePreflightSource::PendingEntering => {
                "projectionless fallback entering branch drifted"
            }
            LifecyclePreflightSource::CrossRoleRebase => {
                "projectionless fallback entering branch cannot rebase"
            }
            LifecyclePreflightSource::TargetPartition => {
                "projectionless fallback target overlaps current authority"
            }
            LifecyclePreflightSource::RequiredResources(_)
            | LifecyclePreflightSource::HostBinding
            | LifecyclePreflightSource::PreservedCurrent
            | LifecyclePreflightSource::LeavingCurrent => {
                "projectionless fallback current authority drifted"
            }
        }
    }

    const fn is_missing_resource(self) -> bool {
        matches!(
            (self.source, self.expectation, self.observed),
            (
                LifecyclePreflightSource::RequiredResources(resources),
                LifecyclePreflightExpectation::ResourcePresent,
                LifecyclePreflightObserved::Missing
            ) if !resources.is_empty()
        )
    }
}

fn current_host_binding_is_exact(world: &World, host: &HostBinding) -> bool {
    let claim_owner_is_exact = matches!(
        host.top_slot_claim.owner(),
        crate::registry::RegistryOwner::Committed(
            crate::registry::EntityOwner::HostTopSlot { host: version, slot }
        ) if version == &host.version && *slot == host.slot_id
    );
    let record_is_exact = world
        .get_resource::<RouteEntityRegistry>()
        .and_then(|registry| registry.record(host.top_slot))
        .is_some_and(|record| {
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
        && validated_host_candidate_is_current(world, &host.candidate)
        && record_is_exact
        && cleanup_claim_has_exact_authority(world, &host.top_slot_claim)
        && parent_is_exact
        && membership_is_exact
}

fn reconciliation_preflight(
    world: &World,
    host_is_current: bool,
    has_leaving: bool,
    splice: SealedReconciliationSplice,
) -> ReconciliationPreflightOutcome {
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
    let pending_rejection = if validate_reconciliation_entering_branch(
        world,
        splice.entering(),
        splice.preserved(),
        splice.leaving(),
    )
    .is_err()
    {
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
        ReconciliationPreflightOutcome::AuthorityLost { splice, evidence }
    } else if let Some(evidence) = pending_rejection {
        ReconciliationPreflightOutcome::PendingRejected { splice, evidence }
    } else if validate_active_branch_for_splice(
        world,
        splice.preserved(),
        splice.entering(),
        splice.slot_roles(),
    )
    .is_err()
    {
        ReconciliationPreflightOutcome::PendingRejected {
            splice,
            evidence: LifecyclePreflightEvidence::new(
                LifecyclePreflightSource::CrossRoleRebase,
                LifecyclePreflightExpectation::RebaseCompatible,
                LifecyclePreflightObserved::Contradictory,
            ),
        }
    } else {
        ReconciliationPreflightOutcome::Ready {
            splice,
            focus_leaving: leaving_closure.expect("current closure was independently validated"),
        }
    }
}

pub(crate) fn commit_presentation_system(world: &mut World) {
    world.resource_scope(
        |world, mut navigation: bevy_ecs::change_detection::Mut<RouterNavigation>| {
            for entry in &mut navigation.entries {
                if entry.authority_protocol_residual.is_some() {
                    continue;
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
                            continue;
                        }
                    };
                    match settle_projectionless_fail_close(world, entry, active, pending, source) {
                        Ok(()) => {}
                        Err((active, pending, source)) => {
                            entry.active = Some(active);
                            entry.pending_commit = Some(pending);
                            entry.projectionless_fail_close = Some(
                                ProjectionlessFailCloseContinuation::Retryable(source),
                            );
                        }
                    }
                    continue;
                }
                let Some(mut pending) = entry.pending_commit.take() else {
                    continue;
                };
                if !pending.branch.frames().is_empty()
                        && pending.ticket.is_none()
                        && pending.final_task.is_none()
                {
                    entry.pending_commit = Some(pending);
                    continue;
                }
                let Some(authority_root) = entry
                    .active
                    .as_ref()
                    .and_then(RealizedBranch::first_frame)
                    .or_else(|| pending.branch.first_frame())
                else {
                    entry.pending_commit = Some(pending);
                    continue;
                };
                let Some(authority) = ReconciliationAuthorityIdentity::from_frame(
                    authority_root.frame(),
                    authority_root.frame().host(),
                    authority_root.frame().slot(),
                ) else {
                    entry.pending_commit = Some(pending);
                    continue;
                };
                let (facts, has_leaving) = {
                    let projection = match (pending.ticket.as_ref(), pending.final_task.as_ref()) {
                        (Some(ticket), _) => {
                            entry.engine.reconciliation_projection_for_commit(ticket)
                        }
                        (None, Some(task)) => {
                            entry.engine.reconciliation_projection_for_task(task)
                        }
                        (None, None) => {
                            entry.pending_commit = Some(pending);
                            continue;
                        }
                    };
                    let Ok(projection) = projection else {
                        match commit_projectionless_branch(world, entry, pending) {
                            Ok(()) => continue,
                            Err(owner) => {
                                entry.pending_commit = Some(owner);
                                continue;
                            }
                        }
                    };
                    let has_leaving = projection.leaving().len() != 0;
                    let Some(facts) = ReconciliationProjectionFacts::from_core(&projection, authority)
                    else {
                        entry.pending_commit = Some(pending);
                        continue;
                    };
                    (facts, has_leaving)
                };
                let had_active = entry.active.is_some();
                let active_owner = entry.active.take().unwrap_or_else(RealizedBranch::empty);
                let sealed = match SealedExistingReconciliation::prepare_from_projection(
                    active_owner,
                    facts,
                ) {
                    Ok(sealed) => sealed,
                    Err(failure) => {
                        let (active, _) = failure.into_parts();
                        entry.active = had_active.then_some(active);
                        entry.pending_commit = Some(pending);
                        continue;
                    }
                };
                let entering_owner =
                    std::mem::replace(&mut pending.branch, PreparedBranch::empty());
                let splice = match sealed.bind_entering(entering_owner) {
                    Ok(splice) => splice,
                    Err(failure) => {
                        let (sealed, entering, _) = failure.into_parts();
                        let (active, _, _roles) = sealed.reject().into_parts();
                        entry.active = had_active.then_some(active);
                        pending.branch = entering;
                        entry.pending_commit = Some(pending);
                        continue;
                    }
                };
                let host_is_current = entry
                    .host
                    .as_ref()
                    .is_some_and(|host| current_host_binding_is_exact(world, host));
                let (splice, mut focus_leaving) = match reconciliation_preflight(
                    world,
                    host_is_current,
                    has_leaving,
                    splice,
                ) {
                    ReconciliationPreflightOutcome::Ready {
                        splice,
                        focus_leaving,
                    } => (splice, focus_leaving),
                    ReconciliationPreflightOutcome::PendingRejected {
                        splice,
                        evidence: _,
                    } => {
                        let (active, entering, _roles) = splice.reject().into_parts();
                        entry.active = had_active.then_some(active);
                        pending.branch = entering;
                        let Some(task) = pending.final_task.take() else {
                            entry.pending_commit = Some(pending);
                            continue;
                        };
                        let failed = PendingFailedBranch {
                            terminal: PendingFailedBranchTerminal::TaskWithPhysicalResidual {
                                task,
                                failure: ViewFailureKind::Apply,
                                message: "frame final preflight rejected",
                            },
                            branch: failed_prepared_branch(pending, None),
                        };
                        let _ = isolate_exact_failed_prepared_branch(
                            world,
                            &failed.branch,
                            entry.active.as_ref(),
                        );
                        entry.pending_failed_branch = Some(failed);
                        continue;
                    }
                    ReconciliationPreflightOutcome::AuthorityLost {
                        splice,
                        evidence,
                    } => {
                        let (active, entering, _roles) = splice.reject().into_parts();
                        pending.branch = entering;
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
                            continue;
                        }
                        match begin_authority_settlement(
                            world,
                            entry,
                            active,
                            pending,
                        ) {
                            AuthoritySettlementBeginOutcome::Begun => {}
                            AuthoritySettlementBeginOutcome::RetryablePreparation {
                                active,
                                pending,
                            } => {
                                entry.active = had_active.then_some(active);
                                entry.pending_commit = Some(pending);
                            }
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
                                    entry.active = had_active.then_some(active);
                                    entry.pending_commit = Some(pending);
                                    continue;
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
                                    entry.active = had_active.then_some(active);
                                    entry.pending_commit = Some(pending);
                                    continue;
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
                                        AuthorityProtocolResidualKind::CleanupLedger(kind),
                                    )
                                {
                                    entry.active = had_active.then_some(active);
                                    entry.pending_commit = Some(pending);
                                    continue;
                                }
                            }
                        }
                        continue;
                    }
                };
                let retained_empty_focus = observed_descendant_closure(
                    world,
                    splice
                        .slot_roles()
                        .iter()
                        .filter(|role| role.kind() == ReconciliationSlotRoleKind::RetainEmpty)
                        .map(|role| role.entity()),
                );
                focus_leaving.extend(retained_empty_focus);
                focus_leaving.sort_by_key(|entity| entity.to_bits());
                focus_leaving.dedup();
                let logical_roots = logical_cleanup_roots(splice.leaving());
                let gate_before = splice.current_interactive_entities();
                let gate_after = splice.resulting_interactive_entities();
                let (targets, isolate_count) = splice.presentation_mutation_targets();
                let transitions = splice.lifecycle_transitions();
                let leaving_count = splice.leaving().obligation_count();
                let root_rebase = if !splice.leaving().frames().is_empty()
                    && !splice.entering().frames().is_empty()
                {
                    match splice.prepare_root_rebase() {
                        Some(prepared) => Some(prepared),
                        None => {
                            let (active, entering, _roles) = splice.reject().into_parts();
                            entry.active = had_active.then_some(active);
                            pending.branch = entering;
                            entry.pending_commit = Some(pending);
                            continue;
                        }
                    }
                } else {
                    None
                };
                let mut splice_owner = Some(splice);
                let mut cleanup_reserve_contradiction = None;
                let committed = world.resource_scope(
                    |world, gate: bevy_ecs::change_detection::Mut<RouterInputGate>| {
                        world.resource_scope(
                            |world, input_focus: bevy_ecs::change_detection::Mut<InputFocus>| {
                                world.resource_scope(
                                    |world,
                                     route_focus: bevy_ecs::change_detection::Mut<
                                        RouteFocusState,
                                    >| {
                                        world.resource_scope(
                                            |world,
                                             mut registry: bevy_ecs::change_detection::Mut<
                                                RouteEntityRegistry,
                                            >| {
                                                let registry_plan = registry
                                                    .prepare_lifecycle_mutation(&transitions)?;
                                                let resulting_claims =
                                                    registry_plan.resulting_claims().to_vec();
                                                let frozen_manifest = if leaving_count == 0 {
                                                    FrozenCleanupManifest::empty()
                                                } else {
                                                    let splice = splice_owner.as_ref()?;
                                                    match FrozenCleanupManifest::from_leaving_branch(
                                                        world,
                                                        &registry,
                                                        splice.leaving(),
                                                        &resulting_claims,
                                                        (!splice.entering().frames().is_empty())
                                                            .then_some(splice.entering()),
                                                    ) {
                                                        Some(manifest) => manifest,
                                                        None => {
                                                            return None;
                                                        }
                                                    }
                                                };
                                                let manifest_digest = frozen_manifest.digest();
                                                let obligation_count = frozen_manifest.len();
                                                let manifest = CleanupManifest::new(
                                                    logical_roots.clone(),
                                                    manifest_digest,
                                                    obligation_count,
                                                );
                                                let reserved_ledger =
                                                    match CleanupLedger::reserve(frozen_manifest) {
                                                        Ok(reserved) => reserved,
                                                        Err(error) => match error.kind() {
                                                            CleanupLedgerReserveErrorKind::RetryableCapacity => {
                                                                return None;
                                                            }
                                                            kind @ (CleanupLedgerReserveErrorKind::DomainExhausted
                                                            | CleanupLedgerReserveErrorKind::UnfinalizedBoundaryFacts) => {
                                                                let _returned_manifest =
                                                                    error.into_manifest();
                                                                cleanup_reserve_contradiction =
                                                                    Some(kind);
                                                                return None;
                                                            }
                                                        },
                                                    };
                                                let gate_plan = prepare_input_gate_mutation(
                                                    gate,
                                                    &gate_before,
                                                    gate_after,
                                                )?;
                                                let focus_plan = prepare_focus_mutation(
                                                    input_focus,
                                                    route_focus,
                                                    &focus_leaving,
                                                );
                                                let claim_rebind = splice_owner
                                                    .as_ref()?
                                                    .prepare_claim_rebind(&resulting_claims)?;
                                                let presentation_plan =
                                                    prepare_presentation_mutation(world, &targets)?
                                                        .seal_commit(isolate_count)?;
                                                if pending.ticket.is_none() {
                                                    let task = pending.final_task.take().expect(
                                                        "fully preflighted legacy leaf task remains owned",
                                                    );
                                                    pending.ticket = Some(
                                                        entry.engine.report_applied(task).expect(
                                                            "sealed task projection must report exactly once",
                                                        ),
                                                    );
                                                }
                                                let ticket = pending.ticket.as_ref()?;
                                                let prepared = match entry
                                                    .engine
                                                    .prepare_commit(ticket, manifest)
                                                {
                                                    Ok(prepared) => prepared,
                                                    Err(_) => return None,
                                                };
                                                let cleanup = prepared.cleanup().clone();
                                                let cleanup_ledger =
                                                    reserved_ledger.bind_batch(cleanup);
                                                pending.ticket.take();
                                                let mut splice = splice_owner
                                                    .take()
                                                    .expect("fully prepared commit retains its sealed splice");
                                                splice.apply_claim_rebind(claim_rebind);
                                                if let Some(rebase) = root_rebase {
                                                    splice.apply_root_rebase(rebase);
                                                }
                                                let outcome = splice.apply();
                                                let (active, leaving) =
                                                    outcome.into_active_and_leaving();
                                                let next_active = (!active.frames().is_empty())
                                                    .then_some(active);
                                                let next_cleanup = Some(
                                                    CleanupLedger::activate_leaving_branch(
                                                        cleanup_ledger,
                                                        (!leaving.frames().is_empty()
                                                            || !leaving.child_slots().is_empty())
                                                        .then_some(leaving),
                                                    ),
                                                );
                                                let engine = &mut entry.engine;
                                                let active_owner = &mut entry.active;
                                                let cleanup_owner = &mut entry.cleanup_ledger;
                                                engine.publish(prepared, || {
                                                    presentation_plan.apply_staged(
                                                        |_| focus_plan.apply(),
                                                        |_| registry_plan.apply(&mut registry),
                                                        |_| gate_plan.apply(),
                                                    );
                                                    *active_owner = next_active;
                                                    *cleanup_owner = next_cleanup;
                                                });
                                                Some(())
                                            },
                                        )
                                    },
                                )
                            },
                        )
                    },
                );
                let Some(()) = committed else {
                    let splice = splice_owner
                        .take()
                        .expect("failed commit retains its sealed splice owner");
                    let (active, entering, _roles) = splice.reject().into_parts();
                    pending.branch = entering;
                    if let Some(kind) = cleanup_reserve_contradiction {
                        if let Err((active, pending)) = fail_closed_normal_owner_protocol(
                            world,
                            entry,
                            active,
                            pending,
                            AuthorityProtocolResidualKind::CleanupLedger(kind),
                        ) {
                            entry.active = had_active.then_some(active);
                            entry.pending_commit = Some(pending);
                        }
                        continue;
                    }
                    entry.active = had_active.then_some(active);
                    if pending.ticket.is_some() || pending.final_task.is_some() {
                        entry.pending_commit = Some(pending);
                    }
                    continue;
                };
            }
        },
    );
    refresh_authority_audits(world);
}

enum AuthoritySettlementBeginOutcome {
    Begun,
    RetryablePreparation {
        active: RealizedBranch,
        pending: PendingCommit,
    },
    ProtocolContradiction {
        active: RealizedBranch,
        pending: PendingCommit,
        kind: BeginAuthoritySettlementErrorKind,
    },
    IsolationContradiction {
        active: RealizedBranch,
        pending: PendingCommit,
        contradiction: AuthorityIsolationContradiction,
        isolation: PreparedAuthorityIsolation,
    },
    LedgerContradiction {
        active: RealizedBranch,
        pending: PendingCommit,
        kind: CleanupLedgerReserveErrorKind,
    },
}

fn begin_authority_settlement(
    world: &mut World,
    entry: &mut RouterEntry,
    active: RealizedBranch,
    pending: PendingCommit,
) -> AuthoritySettlementBeginOutcome {
    let Some(host) = entry.host.as_ref() else {
        return AuthoritySettlementBeginOutcome::ProtocolContradiction {
            active,
            pending,
            kind: BeginAuthoritySettlementErrorKind::Stale,
        };
    };
    let frozen = {
        let registry = world.resource::<RouteEntityRegistry>();
        match FrozenCleanupManifest::from_authority_settlement(
            world,
            registry,
            &pending.branch,
            &active,
            host,
        ) {
            Ok(frozen) => frozen,
            Err(AuthoritySettlementManifestError::RetryableCapacity) => {
                return AuthoritySettlementBeginOutcome::RetryablePreparation { active, pending };
            }
            Err(
                AuthoritySettlementManifestError::InvalidAuthority
                | AuthoritySettlementManifestError::InvalidTopology,
            ) => {
                return AuthoritySettlementBeginOutcome::ProtocolContradiction {
                    active,
                    pending,
                    kind: BeginAuthoritySettlementErrorKind::InvalidManifest,
                };
            }
        }
    };
    let manifest_digest = frozen.digest();
    let obligation_count = frozen.len();
    let reserved = match CleanupLedger::reserve(frozen) {
        Ok(reserved) => reserved,
        Err(error) => match error.kind() {
            CleanupLedgerReserveErrorKind::RetryableCapacity => {
                return AuthoritySettlementBeginOutcome::RetryablePreparation { active, pending };
            }
            kind @ (CleanupLedgerReserveErrorKind::DomainExhausted
            | CleanupLedgerReserveErrorKind::UnfinalizedBoundaryFacts) => {
                let _returned_manifest = error.into_manifest();
                return AuthoritySettlementBeginOutcome::LedgerContradiction {
                    active,
                    pending,
                    kind,
                };
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
        return AuthoritySettlementBeginOutcome::ProtocolContradiction {
            active,
            pending,
            kind: BeginAuthoritySettlementErrorKind::InvalidManifest,
        };
    }

    let isolation =
        match prepare_normal_authority_isolation(world, &active, &pending.branch, Some(host)) {
            AuthorityIsolationPreparation::Ready(isolation) => isolation,
            AuthorityIsolationPreparation::RetryableCapacity => {
                return AuthoritySettlementBeginOutcome::RetryablePreparation { active, pending };
            }
            AuthorityIsolationPreparation::PermanentContradiction {
                contradiction,
                fail_closed,
            } => {
                return AuthoritySettlementBeginOutcome::IsolationContradiction {
                    active,
                    pending,
                    contradiction,
                    isolation: fail_closed,
                };
            }
        };

    let HostBinding {
        version,
        slot_id,
        host: host_entity,
        top_slot,
        top_slot_claim,
        candidate,
        lease,
    } = entry
        .host
        .take()
        .expect("authority settlement retains its exact host binding");
    let logical_roots = authority_settlement_logical_roots(&pending.branch, &active);
    let retired_host = RetiredHostBinding {
        version: version.clone(),
        slot_id,
        host: host_entity,
        top_slot,
        top_slot_claim: top_slot_claim.clone(),
        candidate: candidate.clone(),
    };
    let mut settlement_owner = Some((reserved, active, pending, retired_host));
    let engine = &mut entry.engine;
    let cleanup_ledger = &mut entry.cleanup_ledger;
    match engine.begin_authority_settlement_retaining(
        lease,
        CleanupManifest::new(logical_roots, manifest_digest, obligation_count),
        |batch| {
            isolation.apply(world);
            let (reserved, active, pending, owner) = settlement_owner
                .take()
                .expect("authority publication consumes its composed owner once");
            *cleanup_ledger = Some(CleanupLedger::activate_authority_settlement(
                reserved.bind_batch(batch),
                active,
                FailedPreparedBranch::new(pending.branch),
                owner,
                host_children,
            ));
        },
    ) {
        Ok(_) => AuthoritySettlementBeginOutcome::Begun,
        Err(error) => {
            let kind = error.kind();
            entry.host = Some(HostBinding {
                version,
                slot_id,
                host: host_entity,
                top_slot,
                top_slot_claim,
                candidate,
                lease: error.into_owner(),
            });
            let (_, active, pending, _) = settlement_owner
                .take()
                .expect("rejected publication returns its composed owner");
            return match kind {
                BeginAuthoritySettlementErrorKind::RetryablePreparation => {
                    AuthoritySettlementBeginOutcome::RetryablePreparation { active, pending }
                }
                BeginAuthoritySettlementErrorKind::Foreign
                | BeginAuthoritySettlementErrorKind::Stale
                | BeginAuthoritySettlementErrorKind::WrongPhase
                | BeginAuthoritySettlementErrorKind::InvalidManifest => {
                    AuthoritySettlementBeginOutcome::ProtocolContradiction {
                        active,
                        pending,
                        kind,
                    }
                }
            };
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AuthorityIsolationComponent {
    Visibility,
    Pickable,
    RouterInputBarrier,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AuthorityIsolationPredicate {
    PresentForHiddenWrite,
    PresentForIgnoreWrite,
    PresentForDenyWrite,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AuthorityIsolationViolation {
    entity: bevy_ecs::entity::Entity,
    component: AuthorityIsolationComponent,
    predicate: AuthorityIsolationPredicate,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AuthorityIsolationContradiction {
    violations: Vec<AuthorityIsolationViolation>,
}

struct PreparedAuthorityEntityIsolation {
    entity: bevy_ecs::entity::Entity,
    visibility: bool,
    pickable: bool,
    barrier: bool,
}

struct PreparedAuthorityIsolation {
    entities: Vec<PreparedAuthorityEntityIsolation>,
    focus: Vec<bevy_ecs::entity::Entity>,
    gate: Option<PreparedInputGateIsolation>,
}

impl PreparedAuthorityIsolation {
    fn apply(self, world: &mut World) {
        if let Some(gate) = self.gate
            && let Some(mut current) = world.get_resource_mut::<RouterInputGate>()
        {
            gate.apply(&mut current);
        }
        if world
            .get_resource::<InputFocus>()
            .and_then(InputFocus::get)
            .is_some_and(|focused| self.focus.contains(&focused))
            && let Some(mut focus) = world.get_resource_mut::<InputFocus>()
        {
            *focus = InputFocus::default();
        }
        if world
            .get_resource::<RouteFocusState>()
            .and_then(RouteFocusState::focused)
            .is_some_and(|focused| self.focus.contains(&focused))
            && let Some(mut focus) = world.get_resource_mut::<RouteFocusState>()
        {
            focus.set_focused(None);
        }
        for entity in self.entities {
            if entity.visibility {
                *world
                    .get_mut::<bevy_camera::visibility::Visibility>(entity.entity)
                    .expect("prepared authority isolation retains present Visibility") =
                    bevy_camera::visibility::Visibility::Hidden;
            }
            if entity.pickable {
                *world
                    .get_mut::<bevy_picking::Pickable>(entity.entity)
                    .expect("prepared authority isolation retains present Pickable") =
                    bevy_picking::Pickable::IGNORE;
            }
            if entity.barrier {
                *world
                    .get_mut::<crate::components::RouterInputBarrier>(entity.entity)
                    .expect("prepared authority isolation retains present input barrier") =
                    crate::components::RouterInputBarrier::deny_all();
            }
        }
    }
}

enum AuthorityIsolationPreparation {
    Ready(PreparedAuthorityIsolation),
    RetryableCapacity,
    PermanentContradiction {
        contradiction: AuthorityIsolationContradiction,
        fail_closed: PreparedAuthorityIsolation,
    },
}

fn try_push<T>(values: &mut Vec<T>, value: T) -> Result<(), ()> {
    values.try_reserve(1).map_err(|_| ())?;
    values.push(value);
    Ok(())
}

fn prepare_authority_isolation_targets<'a>(
    world: &World,
    stored_claims: impl IntoIterator<Item = &'a ExactRegistryClaim>,
    gate_targets: impl IntoIterator<Item = bevy_ecs::entity::Entity>,
    capacity_probe: usize,
) -> AuthorityIsolationPreparation {
    let mut capacity_guard = Vec::<u8>::new();
    if capacity_guard.try_reserve_exact(capacity_probe).is_err() {
        return AuthorityIsolationPreparation::RetryableCapacity;
    }
    let mut known = Vec::new();
    let mut exact = Vec::new();
    for claim in stored_claims {
        if !known.contains(&claim.entity()) && try_push(&mut known, claim.entity()).is_err() {
            return AuthorityIsolationPreparation::RetryableCapacity;
        }
        if !exact.contains(&claim.entity())
            && world
                .get_resource::<RouteEntityRegistry>()
                .is_none_or(|registry| {
                    registry
                        .record(claim.entity())
                        .is_some_and(|record| record.claim() == *claim)
                })
            && cleanup_claim_has_exact_authority(world, claim)
            && try_push(&mut exact, claim.entity()).is_err()
        {
            return AuthorityIsolationPreparation::RetryableCapacity;
        }
    }

    let mut entities = Vec::new();
    let mut violations = Vec::new();
    for entity in exact {
        let visibility = match world.get::<bevy_camera::visibility::Visibility>(entity) {
            Some(visibility) => *visibility != bevy_camera::visibility::Visibility::Hidden,
            None => {
                if try_push(
                    &mut violations,
                    AuthorityIsolationViolation {
                        entity,
                        component: AuthorityIsolationComponent::Visibility,
                        predicate: AuthorityIsolationPredicate::PresentForHiddenWrite,
                    },
                )
                .is_err()
                {
                    return AuthorityIsolationPreparation::RetryableCapacity;
                }
                false
            }
        };
        let pickable = match world.get::<bevy_picking::Pickable>(entity) {
            Some(pickable) => *pickable != bevy_picking::Pickable::IGNORE,
            None => {
                if try_push(
                    &mut violations,
                    AuthorityIsolationViolation {
                        entity,
                        component: AuthorityIsolationComponent::Pickable,
                        predicate: AuthorityIsolationPredicate::PresentForIgnoreWrite,
                    },
                )
                .is_err()
                {
                    return AuthorityIsolationPreparation::RetryableCapacity;
                }
                false
            }
        };
        let barrier = match world.get::<crate::components::RouterInputBarrier>(entity) {
            Some(barrier) => !barrier.denies_all(),
            None => {
                if try_push(
                    &mut violations,
                    AuthorityIsolationViolation {
                        entity,
                        component: AuthorityIsolationComponent::RouterInputBarrier,
                        predicate: AuthorityIsolationPredicate::PresentForDenyWrite,
                    },
                )
                .is_err()
                {
                    return AuthorityIsolationPreparation::RetryableCapacity;
                }
                false
            }
        };
        if (visibility || pickable || barrier)
            && try_push(
                &mut entities,
                PreparedAuthorityEntityIsolation {
                    entity,
                    visibility,
                    pickable,
                    barrier,
                },
            )
            .is_err()
        {
            return AuthorityIsolationPreparation::RetryableCapacity;
        }
    }

    let focus_set = observed_descendant_closure(world, known);
    let mut focus = Vec::new();
    if focus.try_reserve_exact(focus_set.len()).is_err() {
        return AuthorityIsolationPreparation::RetryableCapacity;
    }
    focus.extend(focus_set);
    focus.sort_by_key(|entity| entity.to_bits());
    let mut gate_targets_vec = Vec::new();
    for entity in gate_targets {
        if !gate_targets_vec.contains(&entity) && try_push(&mut gate_targets_vec, entity).is_err() {
            return AuthorityIsolationPreparation::RetryableCapacity;
        }
    }
    let gate = match world.get_resource::<RouterInputGate>() {
        Some(gate) => match try_prepare_input_gate_isolation(gate, &gate_targets_vec) {
            Ok(gate) => Some(gate),
            Err(()) => return AuthorityIsolationPreparation::RetryableCapacity,
        },
        None => None,
    };
    let prepared = PreparedAuthorityIsolation {
        entities,
        focus,
        gate,
    };
    if violations.is_empty() {
        AuthorityIsolationPreparation::Ready(prepared)
    } else {
        AuthorityIsolationPreparation::PermanentContradiction {
            contradiction: AuthorityIsolationContradiction { violations },
            fail_closed: prepared,
        }
    }
}

fn prepare_normal_authority_isolation(
    world: &mut World,
    active: &RealizedBranch,
    pending: &PreparedBranch,
    host: Option<&HostBinding>,
) -> AuthorityIsolationPreparation {
    let capacity_probe = projectionless_fail_close_capacity_probe(world);
    prepare_authority_isolation_targets(
        world,
        pending
            .as_realized()
            .claims()
            .chain(active.claims())
            .chain(host.into_iter().map(|host| &host.top_slot_claim)),
        active
            .interactive_entities()
            .into_iter()
            .chain(pending.as_realized().interactive_entities()),
        capacity_probe,
    )
}

fn prepare_failed_authority_isolation(
    world: &World,
    active: Option<&RealizedBranch>,
    failed: &PendingFailedBranch,
    host: Option<&HostBinding>,
) -> AuthorityIsolationPreparation {
    let branch = &failed.branch;
    let stored_claims = branch
        .frames()
        .iter()
        .flat_map(RealizedFrame::claims)
        .chain(
            branch
                .detached_frames()
                .iter()
                .flat_map(RealizedFrame::claims),
        )
        .chain(branch.child_slots().iter().map(|slot| slot.slot_claim()))
        .chain(
            branch
                .mount_residuals()
                .iter()
                .map(ChildSlotResidual::claim),
        )
        .chain(
            branch
                .rejected_child_slots()
                .iter()
                .map(|rejected| rejected.slot().slot_claim()),
        )
        .chain(active.into_iter().flat_map(RealizedBranch::claims))
        .chain(host.into_iter().map(|host| &host.top_slot_claim));
    let gate_targets = active
        .into_iter()
        .flat_map(RealizedBranch::interactive_entities)
        .chain(
            branch
                .frames()
                .iter()
                .chain(branch.detached_frames())
                .flat_map(RealizedFrame::interactive_entities),
        )
        .chain(host.into_iter().map(|host| host.top_slot));
    prepare_authority_isolation_targets(world, stored_claims, gate_targets, 0)
}

fn prepare_failed_manifest_protocol_isolation(
    world: &World,
    active: Option<&RealizedBranch>,
    failed: &PendingFailedBranch,
    host: Option<&HostBinding>,
) -> AuthorityIsolationPreparation {
    prepare_failed_authority_isolation(world, active, failed, host)
}

fn prepare_projectionless_authority_isolation(
    world: &World,
    pending: &PreparedBranch,
    host: Option<&HostBinding>,
) -> AuthorityIsolationPreparation {
    prepare_authority_isolation_targets(
        world,
        pending
            .as_realized()
            .claims()
            .chain(host.into_iter().map(|host| &host.top_slot_claim)),
        pending.as_realized().interactive_entities(),
        0,
    )
}

/*
    The owner-specific functions above only select stored claims and gate targets. All authority,
    component-presence, focus, gate, and in-place mutation preparation is shared by
    `prepare_authority_isolation_targets`.
*/

fn fail_closed_normal_authority_protocol(
    world: &mut World,
    entry: &mut RouterEntry,
    active: RealizedBranch,
    pending: PendingCommit,
    kind: BeginAuthoritySettlementErrorKind,
) -> Result<(), (RealizedBranch, PendingCommit)> {
    fail_closed_normal_owner_protocol(
        world,
        entry,
        active,
        pending,
        AuthorityProtocolResidualKind::Authority(kind),
    )
}

fn fail_closed_normal_owner_protocol(
    world: &mut World,
    entry: &mut RouterEntry,
    active: RealizedBranch,
    pending: PendingCommit,
    kind: AuthorityProtocolResidualKind,
) -> Result<(), (RealizedBranch, PendingCommit)> {
    let (isolation, isolation_contradiction) = match prepare_normal_authority_isolation(
        world,
        &active,
        &pending.branch,
        entry.host.as_ref(),
    ) {
        AuthorityIsolationPreparation::Ready(isolation) => (isolation, None),
        AuthorityIsolationPreparation::RetryableCapacity => return Err((active, pending)),
        AuthorityIsolationPreparation::PermanentContradiction {
            contradiction,
            fail_closed,
        } => (fail_closed, Some(contradiction)),
    };
    fail_closed_normal_authority_protocol_prepared(
        world,
        entry,
        active,
        pending,
        kind,
        isolation,
        isolation_contradiction,
    )
}

fn fail_closed_normal_authority_protocol_prepared(
    world: &mut World,
    entry: &mut RouterEntry,
    active: RealizedBranch,
    pending: PendingCommit,
    kind: AuthorityProtocolResidualKind,
    isolation: PreparedAuthorityIsolation,
    isolation_contradiction: Option<AuthorityIsolationContradiction>,
) -> Result<(), (RealizedBranch, PendingCommit)> {
    match fail_closed_pending_owner_protocol_prepared(
        world,
        entry,
        Some(active),
        pending,
        kind,
        isolation,
        isolation_contradiction,
    ) {
        Ok(()) => Ok(()),
        Err((active, pending)) => Err((
            active.expect("normal protocol residual retains its active owner"),
            pending,
        )),
    }
}

fn fail_closed_pending_owner_protocol_prepared(
    world: &mut World,
    entry: &mut RouterEntry,
    active: Option<RealizedBranch>,
    mut pending: PendingCommit,
    kind: AuthorityProtocolResidualKind,
    isolation: PreparedAuthorityIsolation,
    isolation_contradiction: Option<AuthorityIsolationContradiction>,
) -> Result<(), (Option<RealizedBranch>, PendingCommit)> {
    pending.ticket.take();
    pending.final_task.take();
    let (retired_host, lease) = match entry.host.take() {
        Some(host) => {
            let (retired, lease) = host.into_retired();
            (Some(retired), Some(lease))
        }
        None => (None, None),
    };
    let mut residual = Some(AuthorityProtocolResidualOwner {
        kind,
        isolation_contradiction,
        active,
        pending: Some(pending),
        failed: None,
        host: retired_host,
    });
    let protocol_residual = &mut entry.authority_protocol_residual;
    if let Err(returned_lease) =
        entry
            .engine
            .fail_closed_authority_protocol_retaining(lease, || {
                isolation.apply(world);
                *protocol_residual = residual.take();
            })
    {
        let mut owner = residual
            .take()
            .expect("rejected normal protocol publication retains its exact owner");
        match (owner.host.take(), returned_lease) {
            (Some(retired), Some(lease)) => entry.host = Some(retired.restore(lease)),
            (None, None) => {}
            _ => unreachable!("authority fail-closed returns the exact optional host lease"),
        }
        return Err((
            owner.active.take(),
            owner
                .pending
                .take()
                .expect("normal protocol residual retains pending"),
        ));
    }
    Ok(())
}

fn fail_closed_failed_manifest_protocol(
    world: &mut World,
    entry: &mut RouterEntry,
    failed: PendingFailedBranch,
    kind: FailedPreparedTopologyError,
) -> Result<(), PendingFailedBranch> {
    fail_closed_failed_owner_protocol(
        world,
        entry,
        failed,
        AuthorityProtocolResidualKind::FailedManifest(kind),
    )
}

fn fail_closed_failed_authority_protocol(
    world: &mut World,
    entry: &mut RouterEntry,
    failed: PendingFailedBranch,
    kind: BeginAuthoritySettlementErrorKind,
) -> Result<(), PendingFailedBranch> {
    fail_closed_failed_owner_protocol(
        world,
        entry,
        failed,
        AuthorityProtocolResidualKind::Authority(kind),
    )
}

fn fail_closed_failed_owner_protocol(
    world: &mut World,
    entry: &mut RouterEntry,
    failed: PendingFailedBranch,
    kind: AuthorityProtocolResidualKind,
) -> Result<(), PendingFailedBranch> {
    let (isolation, isolation_contradiction) = match prepare_failed_manifest_protocol_isolation(
        world,
        entry.active.as_ref(),
        &failed,
        entry.host.as_ref(),
    ) {
        AuthorityIsolationPreparation::Ready(isolation) => (isolation, None),
        AuthorityIsolationPreparation::RetryableCapacity => return Err(failed),
        AuthorityIsolationPreparation::PermanentContradiction {
            contradiction,
            fail_closed,
        } => (fail_closed, Some(contradiction)),
    };
    fail_closed_failed_owner_protocol_prepared(
        world,
        entry,
        failed,
        kind,
        isolation,
        isolation_contradiction,
    )
}

fn fail_closed_failed_owner_protocol_prepared(
    world: &mut World,
    entry: &mut RouterEntry,
    failed: PendingFailedBranch,
    kind: AuthorityProtocolResidualKind,
    isolation: PreparedAuthorityIsolation,
    isolation_contradiction: Option<AuthorityIsolationContradiction>,
) -> Result<(), PendingFailedBranch> {
    let active = entry.active.take();
    let (retired_host, lease) = match entry.host.take() {
        Some(host) => {
            let (retired, lease) = host.into_retired();
            (Some(retired), Some(lease))
        }
        None => (None, None),
    };
    let mut residual = Some(AuthorityProtocolResidualOwner {
        kind,
        isolation_contradiction,
        active,
        pending: None,
        failed: Some(failed),
        host: retired_host,
    });
    let protocol_residual = &mut entry.authority_protocol_residual;
    if let Err(returned_lease) =
        entry
            .engine
            .fail_closed_authority_protocol_retaining(lease, || {
                isolation.apply(world);
                *protocol_residual = residual.take();
            })
    {
        let mut owner = residual
            .take()
            .expect("rejected failed-owner publication retains its exact owner");
        match (owner.host.take(), returned_lease) {
            (Some(retired), Some(lease)) => entry.host = Some(retired.restore(lease)),
            (None, None) => {}
            _ => unreachable!("authority fail-closed returns the exact optional host lease"),
        }
        entry.active = owner.active.take();
        return Err(owner
            .failed
            .take()
            .expect("failed-manifest residual retains pending failed owner"));
    }
    Ok(())
}

fn fail_closed_projectionless_owner_protocol(
    world: &mut World,
    entry: &mut RouterEntry,
    pending: PendingCommit,
    kind: CleanupLedgerReserveErrorKind,
) -> Result<(), PendingCommit> {
    let (isolation, isolation_contradiction) = match prepare_projectionless_authority_isolation(
        world,
        &pending.branch,
        entry.host.as_ref(),
    ) {
        AuthorityIsolationPreparation::Ready(isolation) => (isolation, None),
        AuthorityIsolationPreparation::RetryableCapacity => return Err(pending),
        AuthorityIsolationPreparation::PermanentContradiction {
            contradiction,
            fail_closed,
        } => (fail_closed, Some(contradiction)),
    };
    match fail_closed_pending_owner_protocol_prepared(
        world,
        entry,
        None,
        pending,
        AuthorityProtocolResidualKind::CleanupLedger(kind),
        isolation,
        isolation_contradiction,
    ) {
        Ok(()) => Ok(()),
        Err((active, pending)) => {
            debug_assert!(active.is_none());
            Err(pending)
        }
    }
}

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
                gate: gate.map(|gate| {
                    (
                        gate.allows_pointer(entity),
                        gate.allows_keyboard(entity),
                        gate.allows_gamepad(entity),
                    )
                }),
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
    world: &mut World,
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
    message: &'static str,
) {
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

fn commit_projectionless_branch(
    world: &mut World,
    entry: &mut RouterEntry,
    mut pending: PendingCommit,
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
    let gate_before = entry
        .active
        .as_ref()
        .map(RealizedBranch::interactive_entities)
        .unwrap_or_default();
    let gate_after = pending.branch.as_realized().interactive_entities();
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
    let leaving = entry.active.take();
    let entering = std::mem::replace(&mut pending.branch, PreparedBranch::empty()).into_realized();
    let root_rebase = leaving.as_ref().map(|leaving| {
        entering
            .prepare_root_rebase(leaving)
            .expect("preflighted fallback replacement retains exact root rebase")
    });
    let mut branch_owners = Some((leaving, entering));
    let mut failure = None;
    let committed = world.resource_scope(
        |world, gate: bevy_ecs::change_detection::Mut<RouterInputGate>| {
            world.resource_scope(
                |world, input_focus: bevy_ecs::change_detection::Mut<InputFocus>| {
                    world.resource_scope(
                        |world, route_focus: bevy_ecs::change_detection::Mut<RouteFocusState>| {
                            world.resource_scope(
                                |world,
                                 mut registry: bevy_ecs::change_detection::Mut<
                                    RouteEntityRegistry,
                                >| {
                                    let registry_plan = registry
                                        .prepare_lifecycle_mutation(&transitions)
                                        .expect("prevalidated projectionless registry plan remains exact");
                                    let gate_plan = prepare_input_gate_mutation(
                                        gate,
                                        &gate_before,
                                        gate_after,
                                    )
                                    .expect("prevalidated projectionless gate plan remains exact");
                                    let focus_plan =
                                        prepare_focus_mutation(input_focus, route_focus, &focus_leaving);
                                    let presentation_plan = prepare_presentation_mutation(
                                        world,
                                        &targets,
                                    )
                                    .and_then(|prepared| prepared.seal_commit(isolate_count))
                                    .expect(
                                        "prevalidated projectionless presentation plan remains exact",
                                    );
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
                                        CleanupManifest::new(
                                            manifest_logical_roots,
                                            manifest_digest,
                                            obligation_count,
                                        ),
                                    ) {
                                        Ok(prepared) => prepared,
                                        Err(_) => {
                                            failure = Some(
                                                ProjectionlessCommitFailure::PostReportAuthorityLost {
                                                    admission: admission.clone().expect(
                                                        "fallback replacement owns frozen admission",
                                                    ),
                                                    reason: BeginAuthoritySettlementErrorKind::InvalidManifest,
                                                },
                                            );
                                            return None;
                                        }
                                    };
                                    let cleanup = prepared.cleanup().clone();
                                    let cleanup_ledger = reserved_ledger.bind_batch(cleanup);
                                    pending.ticket.take();
                                    let (mut leaving, mut entering) = branch_owners.take().expect(
                                        "fully prepared projectionless commit retains its branch owners",
                                    );
                                    if let (Some(leaving), Some(rebind)) =
                                        (leaving.as_mut(), leaving_rebind)
                                    {
                                        leaving.apply_claim_rebind(rebind);
                                    }
                                    entering.apply_claim_rebind(entering_rebind);
                                    if let Some(rebase) = root_rebase {
                                        entering.apply_root_rebase(rebase);
                                    }
                                    let next_active = Some(entering);
                                    let next_cleanup = Some(
                                        CleanupLedger::activate_leaving_branch(
                                            cleanup_ledger,
                                            leaving,
                                        ),
                                    );
                                    let engine = &mut entry.engine;
                                    let active_owner = &mut entry.active;
                                    let cleanup_owner = &mut entry.cleanup_ledger;
                                    engine.publish(prepared, || {
                                        presentation_plan.apply_staged(
                                            |_| focus_plan.apply(),
                                            |_| registry_plan.apply(&mut registry),
                                            |_| gate_plan.apply(),
                                        );
                                        *active_owner = next_active;
                                        *cleanup_owner = next_cleanup;
                                    });
                                    Some(())
                                },
                            )
                        },
                    )
                },
            )
        },
    );
    let Some(()) = committed else {
        let (leaving, entering) = branch_owners
            .take()
            .expect("failed projectionless commit retains its branch owners");
        pending.branch = PreparedBranch::from_realized(entering);
        return match failure.expect("projectionless commit returns None only with typed failure") {
            ProjectionlessCommitFailure::PostReportAuthorityLost { admission, reason } => {
                let active =
                    leaving.expect("projectionless replacement continuation retains active owner");
                let source = ProjectionlessFailCloseSource::freeze(
                    world, entry, admission, reason, &active, &pending,
                );
                match settle_projectionless_fail_close(world, entry, active, pending, source) {
                    Ok(()) => Ok(()),
                    Err((active, pending, source)) => {
                        entry.active = Some(active);
                        entry.pending_commit = Some(pending);
                        entry.projectionless_fail_close =
                            Some(ProjectionlessFailCloseContinuation::Retryable(source));
                        Ok(())
                    }
                }
            }
        };
    };
    Ok(())
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
                        && physical_parent == expected.entity()
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
                    && candidate.anchor() == expected.entity()
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
                    && candidate.anchor() == expected.entity()
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
                    && rejected.canonical_anchor() == Some(expected.entity())
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

fn classify_failed_manifest(
    manifest: FrozenCleanupManifest,
    failed: PendingFailedBranch,
    active: Option<&RealizedBranch>,
    host: Option<&HostBinding>,
) -> FailedManifestDisposition {
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
    if dispositions.iter().any(|disposition| {
        matches!(
            disposition.0,
            FailedBoundaryDisposition::CurrentAuthorityLoss(_)
        )
    }) {
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
        PendingFailedBranchTerminal::Request(request) => request.logical_roots().to_vec(),
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
        PendingFailedBranchTerminal::Request(request) => {
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
    match engine.begin_failed_authority_settlement_retaining(
        lease,
        trigger,
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

pub(crate) fn settle_cleanup_system(world: &mut World) {
    world.resource_scope(
        |world, mut navigation: bevy_ecs::change_detection::Mut<RouterNavigation>| {
            for entry in &mut navigation.entries {
                if let Some(failed) = entry.pending_failed_branch.take() {
                    if entry.cleanup_ledger.is_some() {
                        entry.pending_failed_branch = Some(failed);
                        continue;
                    }
                    let terminal_is_current = match &failed.terminal {
                        PendingFailedBranchTerminal::Task { task, .. }
                        | PendingFailedBranchTerminal::TaskWithPhysicalResidual { task, .. } => {
                            entry.engine.inspect_report_identity(task).is_ok()
                        }
                        PendingFailedBranchTerminal::RegistrationBegin(error) => {
                            error.validate_rejection(&entry.engine).is_ok()
                        }
                        PendingFailedBranchTerminal::Request(_) => true,
                    };
                    if !terminal_is_current {
                        entry.pending_failed_branch = Some(failed);
                        continue;
                    }
                    let manifest = {
                        let Some(registry) = world.get_resource::<RouteEntityRegistry>() else {
                            entry.pending_failed_branch = Some(failed);
                            continue;
                        };
                        FrozenCleanupManifest::from_failed_prepared_branch(
                            world,
                            registry,
                            &failed.branch,
                        )
                    };
                    let manifest = match manifest {
                        Ok(manifest) => manifest,
                        Err(FailedManifestBuildError::InvalidTopology(kind)) => {
                            if let Err(failed) =
                                fail_closed_failed_manifest_protocol(world, entry, failed, kind)
                            {
                                entry.pending_failed_branch = Some(failed);
                            }
                            continue;
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
                            continue;
                        }
                        Err(FailedManifestBuildError::RetryableCapacity) => {
                            entry.pending_failed_branch = Some(failed);
                            continue;
                        }
                    };
                    let (failed, manifest) = match classify_failed_manifest(
                        manifest,
                        failed,
                        entry.active.as_ref(),
                        entry.host.as_ref(),
                    ) {
                        FailedManifestDisposition::Ready { failed, manifest } => {
                            (failed, manifest)
                        }
                        FailedManifestDisposition::CurrentAuthorityLoss {
                            failed,
                            manifest,
                            bindings,
                        } => {
                            match begin_failed_authority_settlement(
                                world, entry, failed, manifest, bindings,
                            ) {
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
                                    if let Err(failed) = fail_closed_failed_authority_protocol(
                                        world, entry, failed, kind,
                                    ) {
                                        entry.pending_failed_branch = Some(failed);
                                    }
                                }
                                FailedAuthoritySettlementBeginOutcome::IsolationContradiction {
                                    failed,
                                    contradiction,
                                    isolation,
                                } => {
                                    if let Err(failed) =
                                        fail_closed_failed_owner_protocol_prepared(
                                            world,
                                            entry,
                                            failed,
                                            AuthorityProtocolResidualKind::AuthorityIsolation(
                                                contradiction.clone(),
                                            ),
                                            isolation,
                                            Some(contradiction),
                                        )
                                    {
                                        entry.pending_failed_branch = Some(failed);
                                    }
                                }
                                FailedAuthoritySettlementBeginOutcome::LedgerContradiction {
                                    failed,
                                    kind,
                                } => {
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
                            continue;
                        }
                        FailedManifestDisposition::AmbiguousInvalidTopology {
                            failed,
                            manifest,
                        } => {
                            let _bounded_contradiction_evidence = manifest;
                            if let Err(failed) = fail_closed_failed_manifest_protocol(
                                world,
                                entry,
                                failed,
                                FailedPreparedTopologyError::AmbiguousBoundaryOwnership,
                            ) {
                                entry.pending_failed_branch = Some(failed);
                            }
                            continue;
                        }
                    };
                    let preflight_before_isolation = manifest.preflight(world);
                    if !preflight_before_isolation {
                        entry.pending_failed_branch = Some(failed);
                        continue;
                    }
                    let isolated = isolate_failed_prepared_branch(
                        world,
                        &failed.branch,
                        &manifest,
                        entry.active.as_ref(),
                    );
                    if !isolated {
                        entry.pending_failed_branch = Some(failed);
                        continue;
                    }
                    let preflight_after_isolation = manifest.preflight(world);
                    if !preflight_after_isolation {
                        entry.pending_failed_branch = Some(failed);
                        continue;
                    }
                    let manifest_digest = manifest.digest();
                    let obligation_count = manifest.len();
                    let reserved = match CleanupLedger::reserve(manifest) {
                        Ok(reserved) => reserved,
                        Err(error) => match error.kind() {
                            CleanupLedgerReserveErrorKind::RetryableCapacity => {
                                entry.pending_failed_branch = Some(failed);
                                continue;
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
                                continue;
                            }
                        },
                    };
                    let PendingFailedBranch { terminal, branch } = failed;
                    let request = match terminal {
                        PendingFailedBranchTerminal::Task {
                            task,
                            failure,
                            message,
                        } => {
                            match entry.engine.report_failed(task, failure, message) {
                                Ok(Some(request)) => request,
                                Ok(None) if obligation_count == 0 => continue,
                                Ok(None) | Err(_) => {
                                    unreachable!(
                                        "an exact failed branch with physical obligations must issue cleanup"
                                    )
                                }
                            }
                        }
                        PendingFailedBranchTerminal::TaskWithPhysicalResidual {
                            task,
                            failure,
                            message,
                        } => entry
                            .engine
                            .report_failed_with_physical_residual(task, failure, message)
                            .expect(
                                "an exact isolated physical residual retains its report task",
                            ),
                        PendingFailedBranchTerminal::Request(request) => request,
                        PendingFailedBranchTerminal::RegistrationBegin(error) => {
                            match error.reject(&mut entry.engine) {
                                Ok(request) => request,
                                Err(error) => {
                                    entry.pending_failed_branch = Some(PendingFailedBranch {
                                        terminal: PendingFailedBranchTerminal::RegistrationBegin(
                                            error,
                                        ),
                                        branch,
                                    });
                                    continue;
                                }
                            }
                        }
                    };
                    let logical_roots = request.logical_roots().to_vec();
                    let cleanup = match entry.engine.prepare_failure_cleanup_retaining(
                        request,
                        CleanupManifest::new(
                            logical_roots,
                            manifest_digest,
                            obligation_count,
                        ),
                    ) {
                        Ok(cleanup) => cleanup,
                        Err(error) => {
                            entry.pending_failed_branch = Some(PendingFailedBranch {
                                terminal: PendingFailedBranchTerminal::Request(error.into_owner()),
                                branch,
                            });
                            continue;
                        }
                    };
                    entry.cleanup_ledger = Some(CleanupLedger::activate_failed(
                        reserved.bind_batch(cleanup),
                        branch,
                    ));
                }
                let Some(mut ledger) = entry.cleanup_ledger.take() else {
                    continue;
                };
                if !entry.observe_cleanup_receipts(&mut ledger) {
                    entry.cleanup_ledger = Some(ledger);
                    continue;
                }
                if !ledger.is_settled() {
                    let Some(attempt) = ledger.attempt(world) else {
                        entry.cleanup_ledger = Some(ledger);
                        continue;
                    };
                    let settled = ledger.settle(world, attempt);
                    if !entry.observe_cleanup_receipts(&mut ledger) {
                        entry.cleanup_ledger = Some(ledger);
                        continue;
                    }
                    if !settled {
                        entry.cleanup_ledger = Some(ledger);
                        continue;
                    }
                }
                let completion = ledger
                    .pending_completion()
                    .expect("successful settlement has a completion");
                if ledger.is_authority_settlement() {
                    match entry.engine.complete_authority_cleanup(completion, || {
                        drop(
                            ledger
                                .take_authority_settlement_owner()
                                .expect("authority publication retains its composed owner"),
                        );
                    }) {
                        Ok(()) => {}
                        Err(_) => entry.cleanup_ledger = Some(ledger),
                    }
                    continue;
                }
                match entry.engine.complete_cleanup(completion) {
                    Ok(()) | Err(IntegrationError::Duplicate) => {}
                    Err(_) => entry.cleanup_ledger = Some(ledger),
                }
            }
        },
    );
    refresh_authority_audits(world);
}

pub(crate) fn reap_quarantines_system(_world: &mut World) {}

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
    refresh_authority_audits(world);
}

#[cfg(feature = "test-support")]
fn refresh_authority_audits(world: &mut World) {
    world.resource_scope(
        |world, navigation: bevy_ecs::change_detection::Mut<RouterNavigation>| {
            world.resource_scope(
                |world,
                 mut audits: bevy_ecs::change_detection::Mut<
                    crate::audit::RouterAuthorityAudits,
                >| {
                    audits.refresh(world, &navigation.entries);
                },
            );
        },
    );
}

#[cfg(not(feature = "test-support"))]
fn refresh_authority_audits(_world: &mut World) {}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    #[cfg(feature = "test-support")]
    use crate::audit::RouterAuthorityAudits;
    use crate::{
        app_ext::{AppRouterExt, RouterConfig, RouterHostRegistration},
        cleanup::{
            AuthorityLostResidualPhysicalRestoreDrift, AuthorityLostResidualPostwriteDrift,
            AuthorityRemovalRegistryPostwriteDrift, AuthorityResidualPostwriteDrift,
            CleanupPostcheckDrift, CleanupPostcheckPhase, FailedBoundaryExpectation,
        },
        components::{
            ChildSlotAuthority, ChildSlotAuthorityProbe, ChildSlotBarrierProbe, ChildSlotLifecycle,
            ProvisionalPostWriteProbe, ProvisionalSlotLifecycle, RouteOwnershipSeal,
            RouterInputBarrier,
        },
        content::{ContentColor, SolidRect},
        driver::{EcsViewDriver, ViewBuildReport},
        host::HostCandidateRevision,
        input::{RouteFocusState, RouterInputGate},
        plugin::{BevyUiRouterPlugin, RouterScheduleSet},
        presentation::{
            FrameLifecycle, FrameOutput, TestCommitMutation, arm_test_commit_mutation_trace,
            take_test_commit_mutation_trace,
        },
        registry::{
            ComponentManifest, EntityOwner, ProvisionalOwner, ProvisionalTopSlotOwner,
            QuarantineLeaseTable, RegistryOwner,
            fail_next_authority_publication_capacity_reservation_for_test,
            fail_next_verified_preparation_for_test,
            take_authority_postmutation_manifest_materializations_for_test,
            take_authority_postmutation_reservation_attempts_for_test,
        },
    };
    use bevy_app::{App, PostUpdate, PreUpdate, Update};
    use bevy_camera::visibility::{InheritedVisibility, Visibility};
    use bevy_ecs::{
        component::Component,
        entity::Entity,
        event::EntityEvent,
        hierarchy::{ChildOf, Children},
        lifecycle::HookContext,
        observer::On,
        prelude::Resource,
        query::With,
        schedule::IntoScheduleConfigs,
    };
    use bevy_input_focus::InputFocus;
    use bevy_internal::{
        DefaultPlugins,
        camera::{Camera2d, NormalizedRenderTarget},
        math::Vec2,
        picking::{
            hover::HoverMap,
            pointer::{Location, PointerId, PointerLocation},
        },
        window::{PrimaryWindow, Window, WindowRef},
    };
    use bevy_picking::Pickable;
    use bevy_picking::events::{Over, Pointer};
    use bevy_ui::{ComputedNode, Node, UiGlobalTransform, Val};
    use bevy_ui_router_core::integration::ViewReportIdentity;
    use bevy_ui_router_core::prelude::{
        ActivePresentationKind, FrameOrigin, NavigationFailureKind, NavigationPhase,
        NavigationTraceEventKind, RequestStatus, RouteNodeOptions, RoutePattern, RouteTree,
        RouteViewKey, RouterAvailability, RouterUnavailableReason,
    };
    use bevy_ui_router_view::{
        RouteViewDescriptor, RouteViewRegistry, ViewBuildEffect, ViewDriverKey,
    };

    #[test]
    fn exhausted_navigation_domain_never_wraps_or_reuses_authority() {
        let counter = AtomicU64::new(u64::MAX - 1);

        assert_eq!(allocate_navigation_domain(&counter), Some(u64::MAX - 1));
        assert_eq!(counter.load(Ordering::Relaxed), u64::MAX);
        assert_eq!(allocate_navigation_domain(&counter), None);
        assert_eq!(allocate_navigation_domain(&counter), None);
        assert_eq!(counter.load(Ordering::Relaxed), u64::MAX);
    }

    #[test]
    fn commit_without_a_leaving_branch_has_no_focus_release_targets() {
        let mut world = World::new();
        let active_focus = world.spawn_empty().id();

        assert!(focus_targets_for_commit(false, vec![active_focus]).is_empty());
        assert_eq!(
            focus_targets_for_commit(true, vec![active_focus]),
            vec![active_focus]
        );
    }

    struct AtomicDriver {
        key: ViewDriverKey,
    }

    struct ReplayIdentityDriver {
        key: ViewDriverKey,
        replay: std::sync::Arc<std::sync::Mutex<Option<ViewReportIdentity>>>,
        observed: std::sync::Arc<std::sync::Mutex<Vec<ViewReportIdentity>>>,
    }

    impl EcsViewDriver for AtomicDriver {
        fn key(&self) -> ViewDriverKey {
            self.key.clone()
        }

        fn apply(
            &mut self,
            _effect: &ViewBuildEffect,
            content: &mut FrameContentWriter<'_>,
            report: EcsViewReportSession<'_>,
        ) -> ViewBuildReport {
            content
                .spawn_solid_rect(
                    SolidRect::new(24, 16, ContentColor::srgba8(1, 2, 3, 255)).unwrap(),
                )
                .unwrap();
            report.applied()
        }
    }

    impl EcsViewDriver for ReplayIdentityDriver {
        fn key(&self) -> ViewDriverKey {
            self.key.clone()
        }

        fn apply(
            &mut self,
            _effect: &ViewBuildEffect,
            content: &mut FrameContentWriter<'_>,
            report: EcsViewReportSession<'_>,
        ) -> ViewBuildReport {
            self.observed
                .lock()
                .unwrap()
                .push(report.testing_identity());
            content
                .spawn_solid_rect(
                    SolidRect::new(24, 16, ContentColor::srgba8(1, 2, 3, 255)).unwrap(),
                )
                .unwrap();
            let replay = self.replay.lock().unwrap().take();
            if let Some(identity) = replay {
                let mut seal = ReportSeal::new();
                EcsViewReportSession::new(&mut seal, identity).applied()
            } else {
                report.applied()
            }
        }
    }

    struct PanicDriver {
        key: ViewDriverKey,
    }

    struct NestedDriver {
        key: ViewDriverKey,
        observed: std::sync::Arc<std::sync::Mutex<Vec<RouteViewKey>>>,
        declare_outlet: bool,
    }

    struct DeepNestedDriver {
        key: ViewDriverKey,
    }

    struct FailDeepPendingDriver {
        key: ViewDriverKey,
        calls: usize,
        fail_on: usize,
    }

    struct FailRootReplacementDriver {
        key: ViewDriverKey,
        calls: usize,
        fail_on: usize,
    }

    struct FailRetainedRootReplacementDriver {
        key: ViewDriverKey,
        calls: usize,
        fail_on: usize,
        root: RouteViewKey,
    }

    impl EcsViewDriver for NestedDriver {
        fn key(&self) -> ViewDriverKey {
            self.key.clone()
        }

        fn apply(
            &mut self,
            effect: &ViewBuildEffect,
            content: &mut FrameContentWriter<'_>,
            report: EcsViewReportSession<'_>,
        ) -> ViewBuildReport {
            self.observed.lock().unwrap().push(effect.view().clone());
            if effect.view() == &RouteViewKey::new("layout").unwrap() && self.declare_outlet {
                let anchor = content.spawn_empty().unwrap();
                content
                    .declare_outlet(anchor, bevy_ui_router_core::slot::RouteSlotKey::default())
                    .unwrap();
            } else {
                content
                    .spawn_solid_rect(
                        SolidRect::new(24, 16, ContentColor::srgba8(1, 2, 3, 255)).unwrap(),
                    )
                    .unwrap();
            }
            report.applied()
        }
    }

    impl EcsViewDriver for DeepNestedDriver {
        fn key(&self) -> ViewDriverKey {
            self.key.clone()
        }

        fn apply(
            &mut self,
            effect: &ViewBuildEffect,
            content: &mut FrameContentWriter<'_>,
            report: EcsViewReportSession<'_>,
        ) -> ViewBuildReport {
            if effect.view() == &RouteViewKey::new("deep-layout").unwrap()
                || effect.view() == &RouteViewKey::new("deep-section").unwrap()
            {
                let anchor = content.spawn_empty().unwrap();
                content
                    .declare_outlet(anchor, bevy_ui_router_core::slot::RouteSlotKey::default())
                    .unwrap();
            } else {
                content
                    .spawn_solid_rect(
                        SolidRect::new(24, 16, ContentColor::srgba8(1, 2, 3, 255)).unwrap(),
                    )
                    .unwrap();
            }
            report.applied()
        }
    }

    impl EcsViewDriver for FailDeepPendingDriver {
        fn key(&self) -> ViewDriverKey {
            self.key.clone()
        }

        fn apply(
            &mut self,
            effect: &ViewBuildEffect,
            content: &mut FrameContentWriter<'_>,
            report: EcsViewReportSession<'_>,
        ) -> ViewBuildReport {
            let call = self.calls;
            self.calls += 1;
            if call == self.fail_on {
                return report.build_failed("expected pending deep failure");
            }
            if effect.view() == &RouteViewKey::new("fail-deep-layout").unwrap()
                || effect.view() == &RouteViewKey::new("fail-deep-section").unwrap()
            {
                content
                    .spawn_solid_rect(
                        SolidRect::new(17, 13, ContentColor::srgba8(9, 8, 7, 255)).unwrap(),
                    )
                    .unwrap();
                let anchor = content.spawn_empty().unwrap();
                content
                    .declare_outlet(anchor, bevy_ui_router_core::slot::RouteSlotKey::default())
                    .unwrap();
            } else {
                content
                    .spawn_solid_rect(
                        SolidRect::new(24, 16, ContentColor::srgba8(1, 2, 3, 255)).unwrap(),
                    )
                    .unwrap();
            }
            report.applied()
        }
    }

    impl EcsViewDriver for FailRootReplacementDriver {
        fn key(&self) -> ViewDriverKey {
            self.key.clone()
        }

        fn apply(
            &mut self,
            _effect: &ViewBuildEffect,
            content: &mut FrameContentWriter<'_>,
            report: EcsViewReportSession<'_>,
        ) -> ViewBuildReport {
            let call = self.calls;
            self.calls += 1;
            if call == self.fail_on {
                return report.build_failed("expected root replacement failure");
            }
            content
                .spawn_solid_rect(
                    SolidRect::new(24, 16, ContentColor::srgba8(1, 2, 3, 255)).unwrap(),
                )
                .unwrap();
            report.applied()
        }
    }

    impl EcsViewDriver for FailRetainedRootReplacementDriver {
        fn key(&self) -> ViewDriverKey {
            self.key.clone()
        }

        fn apply(
            &mut self,
            effect: &ViewBuildEffect,
            content: &mut FrameContentWriter<'_>,
            report: EcsViewReportSession<'_>,
        ) -> ViewBuildReport {
            let call = self.calls;
            self.calls += 1;
            if call == self.fail_on {
                return report.build_failed("expected retained-empty root replacement failure");
            }
            if effect.view() == &self.root {
                let anchor = content.spawn_empty().unwrap();
                content
                    .declare_outlet(anchor, bevy_ui_router_core::slot::RouteSlotKey::default())
                    .unwrap();
            } else {
                content
                    .spawn_solid_rect(
                        SolidRect::new(24, 16, ContentColor::srgba8(1, 2, 3, 255)).unwrap(),
                    )
                    .unwrap();
            }
            report.applied()
        }
    }

    impl EcsViewDriver for PanicDriver {
        fn key(&self) -> ViewDriverKey {
            self.key.clone()
        }

        fn apply(
            &mut self,
            _effect: &ViewBuildEffect,
            _content: &mut FrameContentWriter<'_>,
            _report: EcsViewReportSession<'_>,
        ) -> ViewBuildReport {
            panic!("stale host authority must be rejected before the driver runs")
        }
    }

    struct FailOnceDriver {
        key: ViewDriverKey,
        failed: bool,
    }

    #[derive(Resource)]
    struct PersistentOnePickableRemoval {
        remaining_before_target: usize,
        target: Option<Entity>,
    }

    #[derive(Resource)]
    struct OnePickableRemoval {
        remaining_before_target: usize,
        removed: bool,
    }

    #[derive(Clone)]
    enum CleanupDespawnDriftAction {
        TargetRegistryClaim(crate::registry::ExactRegistryClaim),
        ExternalParentSeal(Entity),
    }

    #[derive(Resource)]
    struct CleanupDespawnDrift {
        target: Entity,
        action: CleanupDespawnDriftAction,
    }

    #[derive(Resource, Default)]
    struct PickableRepairParentDrift {
        target: Option<Entity>,
        parent: Option<Entity>,
        fired: bool,
    }

    fn install_cleanup_despawn_drift_hook(app: &mut App) {
        app.world_mut()
            .register_component_hooks::<RouteOwnershipSeal>()
            .on_despawn(|mut world, HookContext { entity, .. }| {
                let action = world
                    .get_resource::<CleanupDespawnDrift>()
                    .filter(|drift| drift.target == entity)
                    .map(|drift| drift.action.clone());
                match action {
                    Some(CleanupDespawnDriftAction::TargetRegistryClaim(claim)) => {
                        world
                            .resource_mut::<RouteEntityRegistry>()
                            .compare_exchange_lifecycle(&claim, EntityLifecycle::Quarantined);
                    }
                    Some(CleanupDespawnDriftAction::ExternalParentSeal(parent)) => {
                        let seal = world.resource_mut::<RouteEntityRegistry>().issue_seal();
                        world
                            .commands()
                            .entity(parent)
                            .insert(RouteOwnershipSeal::new(seal));
                    }
                    None => {}
                }
            });
    }

    #[derive(Resource)]
    struct HostGrantObserverDrift {
        enabled: bool,
        observed_slots: Vec<Entity>,
    }

    #[derive(Resource, Default)]
    struct PendingPreflightDrift {
        armed: bool,
        drifted: Vec<Entity>,
    }

    #[derive(Resource, Default)]
    struct FallbackReplacementDrift {
        armed: bool,
        drifted: Option<Entity>,
    }

    #[derive(Resource, Default)]
    struct NativeTargetDeliveries(std::collections::BTreeMap<Entity, usize>);

    fn install_native_pointer_observer(app: &mut App) {
        app.world_mut().add_observer(
            |event: On<Pointer<Over>>,
             mut deliveries: bevy_ecs::prelude::ResMut<NativeTargetDeliveries>| {
                *deliveries.0.entry(event.event_target()).or_default() += 1;
            },
        );
    }

    #[derive(Resource, Default)]
    struct ProvisionalDetachReentry {
        enabled: bool,
        host: Option<Entity>,
        removals: usize,
    }

    #[derive(Component)]
    struct CleanupDriftMarker;

    #[derive(Resource, Default)]
    struct ChildFrameDriftArmed(bool);

    #[derive(Component)]
    struct FinalPreflightForeignComponent;

    #[derive(Resource, Default)]
    struct ForeignComponentHooks {
        removed: usize,
        despawned: usize,
    }

    #[derive(Clone, Copy, Debug)]
    struct CommitWindowSample {
        old_gate_denied: bool,
        new_gate_allowed: bool,
        input_focus: Option<Entity>,
        route_focus: Option<Entity>,
    }

    #[derive(Resource, Default)]
    struct CommitWindowObservation {
        armed: bool,
        leaving_output: Option<Entity>,
        samples: Vec<CommitWindowSample>,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum NestedBranchSide {
        Old,
        New,
        Mixed,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct NestedCommitScheduleSample {
        physical: NestedBranchSide,
        gate: NestedBranchSide,
        registry: NestedBranchSide,
        ecs: NestedBranchSide,
        core: NestedBranchSide,
        history: NestedBranchSide,
        input_focus: Option<Entity>,
        route_focus: Option<Entity>,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum NestedCommitOrder {
        Leaving(Entity),
        Entering(Entity),
    }

    #[derive(Resource, Default)]
    struct NestedCommitScheduleProbe {
        armed: bool,
        old_entities: Vec<Entity>,
        old_interactive: Vec<Entity>,
        old_frames: Vec<RouteFrameVersion>,
        old_shells: Vec<Entity>,
        old_location: Option<RouteLocation>,
        new_entities: Vec<Entity>,
        new_interactive: Vec<Entity>,
        new_frames: Vec<RouteFrameVersion>,
        new_shells: Vec<Entity>,
        new_location: Option<RouteLocation>,
        old_focus: Option<Entity>,
        samples: Vec<NestedCommitScheduleSample>,
        order: Vec<NestedCommitOrder>,
    }

    #[derive(Resource, Default)]
    struct MountNewRoleScheduleProbe {
        armed: bool,
        roles: Option<Vec<bevy_ui_router_core::integration::ReconciliationChildSlotRoleKind>>,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum NestedCleanupUnit {
        ChildFrame,
        ChildSlot,
        ParentFrame,
    }

    #[derive(Resource, Default)]
    struct NestedLeavingCleanupTrace {
        units: std::collections::BTreeMap<Entity, NestedCleanupUnit>,
        despawned: Vec<NestedCleanupUnit>,
    }

    #[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
    struct NestedAppHostSentinel(u64);

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct NestedProtectedFingerprint {
        manifest: ComponentManifest,
        parent: Option<Entity>,
        children: Vec<Entity>,
        visibility: Option<Visibility>,
        pickable: Option<(bool, bool)>,
        barrier: Option<(bool, bool)>,
        sentinel: Option<NestedAppHostSentinel>,
    }

    impl EcsViewDriver for FailOnceDriver {
        fn key(&self) -> ViewDriverKey {
            self.key.clone()
        }

        fn apply(
            &mut self,
            _effect: &ViewBuildEffect,
            content: &mut FrameContentWriter<'_>,
            report: EcsViewReportSession<'_>,
        ) -> ViewBuildReport {
            if !self.failed {
                self.failed = true;
                return report.build_failed("expected first-attempt failure");
            }
            content
                .spawn_solid_rect(
                    SolidRect::new(24, 16, ContentColor::srgba8(1, 2, 3, 255)).unwrap(),
                )
                .unwrap();
            report.applied()
        }
    }

    fn install_atomic_router(app: &mut App, label: &str) -> RouterHandle {
        let driver = ViewDriverKey::new(format!("{label}-driver")).unwrap();
        app.register_ui_view_driver(AtomicDriver {
            key: driver.clone(),
        })
        .unwrap();
        let view = RouteViewKey::new(format!("{label}-view")).unwrap();
        let routes = RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/").unwrap(),
                RouteNodeOptions::new().with_view(view.clone()),
            )
            .unwrap()
            .build()
            .unwrap();
        let mut views = RouteViewRegistry::new();
        views
            .insert(view.clone(), RouteViewDescriptor::new(driver))
            .unwrap();
        let router = app
            .add_ui_router(RouterConfig::new(routes, views, view))
            .unwrap();
        app.world_mut()
            .spawn((Node::default(), RouterHostRegistration::new(router)));
        router
    }

    fn install_replay_router(
        app: &mut App,
        label: &str,
        replay: std::sync::Arc<std::sync::Mutex<Option<ViewReportIdentity>>>,
        observed: std::sync::Arc<std::sync::Mutex<Vec<ViewReportIdentity>>>,
    ) -> RouterHandle {
        let driver = ViewDriverKey::new(format!("{label}-driver")).unwrap();
        app.register_ui_view_driver(ReplayIdentityDriver {
            key: driver.clone(),
            replay,
            observed,
        })
        .unwrap();
        let view = RouteViewKey::new(format!("{label}-view")).unwrap();
        let routes = RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/:id").unwrap(),
                RouteNodeOptions::new().with_view(view.clone()),
            )
            .unwrap()
            .build()
            .unwrap();
        let mut views = RouteViewRegistry::new();
        views
            .insert(view.clone(), RouteViewDescriptor::new(driver))
            .unwrap();
        let router = app
            .add_ui_router(RouterConfig::new(routes, views, view))
            .unwrap();
        app.world_mut()
            .spawn((Node::default(), RouterHostRegistration::new(router)));
        router
    }

    fn install_nested_router(
        app: &mut App,
        observed: std::sync::Arc<std::sync::Mutex<Vec<RouteViewKey>>>,
        declare_outlet: bool,
    ) -> RouterHandle {
        let key = ViewDriverKey::new("nested-driver").unwrap();
        app.register_ui_view_driver(NestedDriver {
            key: key.clone(),
            observed,
            declare_outlet,
        })
        .unwrap();
        let layout = RouteViewKey::new("layout").unwrap();
        let item = RouteViewKey::new("item").unwrap();
        let fallback = RouteViewKey::new("fallback").unwrap();
        let mut views = RouteViewRegistry::new();
        for view in [&layout, &item, &fallback] {
            views
                .insert(view.clone(), RouteViewDescriptor::new(key.clone()))
                .unwrap();
        }
        let router = app
            .add_ui_router(RouterConfig::new(
                bevy_ui_router_core::test_support::nested_items_view_route_tree(),
                views,
                fallback,
            ))
            .unwrap();
        app.world_mut()
            .spawn((Node::default(), RouterHostRegistration::new(router)));
        router
    }

    fn install_deep_nested_router(app: &mut App) -> RouterHandle {
        let key = ViewDriverKey::new("deep-nested-driver").unwrap();
        app.register_ui_view_driver(DeepNestedDriver { key: key.clone() })
            .unwrap();
        let layout = RouteViewKey::new("deep-layout").unwrap();
        let section = RouteViewKey::new("deep-section").unwrap();
        let item = RouteViewKey::new("deep-item").unwrap();
        let fallback = RouteViewKey::new("deep-fallback").unwrap();
        let mut views = RouteViewRegistry::new();
        for view in [&layout, &section, &item, &fallback] {
            views
                .insert(view.clone(), RouteViewDescriptor::new(key.clone()))
                .unwrap();
        }
        let mut builder = RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/").unwrap(),
                RouteNodeOptions::new().with_view(layout),
            )
            .unwrap();
        let root = builder.root_id().unwrap();
        let section_route = builder
            .route_with_options(
                root,
                RoutePattern::path("sections/:section").unwrap(),
                RouteNodeOptions::new().with_view(section),
            )
            .unwrap();
        builder
            .route_with_options(
                section_route,
                RoutePattern::path("items/:id").unwrap(),
                RouteNodeOptions::new().with_view(item),
            )
            .unwrap();
        let router = app
            .add_ui_router(RouterConfig::new(builder.build().unwrap(), views, fallback))
            .unwrap();
        app.world_mut()
            .spawn((Node::default(), RouterHostRegistration::new(router)));
        router
    }

    fn install_failing_deep_pending_router(app: &mut App) -> RouterHandle {
        let key = ViewDriverKey::new("fail-deep-pending-driver").unwrap();
        app.register_ui_view_driver(FailDeepPendingDriver {
            key: key.clone(),
            calls: 0,
            fail_on: 3,
        })
        .unwrap();
        let layout = RouteViewKey::new("fail-deep-layout").unwrap();
        let section = RouteViewKey::new("fail-deep-section").unwrap();
        let item = RouteViewKey::new("fail-deep-item").unwrap();
        let fallback = RouteViewKey::new("fail-deep-fallback").unwrap();
        let mut views = RouteViewRegistry::new();
        for view in [&layout, &section, &item, &fallback] {
            views
                .insert(view.clone(), RouteViewDescriptor::new(key.clone()))
                .unwrap();
        }
        let mut builder = RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/").unwrap(),
                RouteNodeOptions::new().with_view(layout),
            )
            .unwrap();
        let root = builder.root_id().unwrap();
        let section_route = builder
            .route_with_options(
                root,
                RoutePattern::path("sections/:section").unwrap(),
                RouteNodeOptions::new().with_view(section),
            )
            .unwrap();
        let item_route = builder
            .route_with_options(
                section_route,
                RoutePattern::path("items/:id").unwrap(),
                RouteNodeOptions::new().with_view(item.clone()),
            )
            .unwrap();
        builder
            .route_with_options(
                item_route,
                RoutePattern::path("detail").unwrap(),
                RouteNodeOptions::new().with_view(item),
            )
            .unwrap();
        let router = app
            .add_ui_router(RouterConfig::new(builder.build().unwrap(), views, fallback))
            .unwrap();
        app.world_mut()
            .spawn((Node::default(), RouterHostRegistration::new(router)));
        router
    }

    fn install_failing_root_replacement_router(app: &mut App, label: &str) -> RouterHandle {
        let key = ViewDriverKey::new(format!("{label}-driver")).unwrap();
        app.register_ui_view_driver(FailRootReplacementDriver {
            key: key.clone(),
            calls: 0,
            fail_on: 1,
        })
        .unwrap();
        let view = RouteViewKey::new(format!("{label}-view")).unwrap();
        let fallback = RouteViewKey::new(format!("{label}-fallback")).unwrap();
        let routes = RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/:id").unwrap(),
                RouteNodeOptions::new().with_view(view.clone()),
            )
            .unwrap()
            .build()
            .unwrap();
        let mut views = RouteViewRegistry::new();
        views
            .insert(view, RouteViewDescriptor::new(key.clone()))
            .unwrap();
        views
            .insert(fallback.clone(), RouteViewDescriptor::new(key))
            .unwrap();
        let router = app
            .add_ui_router(RouterConfig::new(routes, views, fallback))
            .unwrap();
        app.world_mut()
            .spawn((Node::default(), RouterHostRegistration::new(router)));
        router
    }

    fn install_failing_retained_root_replacement_router(
        app: &mut App,
        label: &str,
    ) -> RouterHandle {
        let key = ViewDriverKey::new(format!("{label}-driver")).unwrap();
        let root = RouteViewKey::new(format!("{label}-root")).unwrap();
        let item = RouteViewKey::new(format!("{label}-item")).unwrap();
        let fallback = RouteViewKey::new(format!("{label}-fallback")).unwrap();
        app.register_ui_view_driver(FailRetainedRootReplacementDriver {
            key: key.clone(),
            calls: 0,
            fail_on: 2,
            root: root.clone(),
        })
        .unwrap();
        let mut builder = RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/:root").unwrap(),
                RouteNodeOptions::new().with_view(root.clone()),
            )
            .unwrap();
        let root_route = builder.root_id().unwrap();
        builder
            .route_with_options(
                root_route,
                RoutePattern::path("items/:id").unwrap(),
                RouteNodeOptions::new().with_view(item.clone()),
            )
            .unwrap();
        let mut views = RouteViewRegistry::new();
        for view in [root, item, fallback.clone()] {
            views
                .insert(view, RouteViewDescriptor::new(key.clone()))
                .unwrap();
        }
        let router = app
            .add_ui_router(RouterConfig::new(builder.build().unwrap(), views, fallback))
            .unwrap();
        app.world_mut()
            .spawn((Node::default(), RouterHostRegistration::new(router)));
        router
    }

    fn drive_until_projectionless_fallback_pending(app: &mut App) {
        for _ in 0..8 {
            drive_navigation_system(app.world_mut());
            if app.world().resource::<RouterNavigation>().entries[0]
                .pending_commit
                .as_ref()
                .and_then(|pending| pending.final_task.as_ref())
                .is_some_and(|task| task.frame().origin() == &FrameOrigin::RouterFallback)
            {
                return;
            }
        }
        panic!("projectionless fallback task did not reach pending commit");
    }

    fn prepare_deep_nested_branch_for_commit(
        app: &mut App,
        router: RouterHandle,
        location: RouteLocation,
    ) -> NavigationRequestId {
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, location)
            .unwrap();
        collect_navigation_system(app.world_mut());
        for _ in 0..5 {
            drive_navigation_system(app.world_mut());
        }
        assert!(
            app.world().resource::<RouterNavigation>().entries[0]
                .pending_commit
                .is_some()
        );
        request
    }

    fn commit_initial_deep_nested_branch(app: &mut App, router: RouterHandle) {
        prepare_deep_nested_branch_for_commit(
            app,
            router,
            RouteLocation::new("/sections/a/items/1"),
        );
        commit_presentation_system(app.world_mut());
        settle_cleanup_system(app.world_mut());
        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert!(entry.pending_commit.is_none());
        assert_eq!(entry.active.as_ref().unwrap().frames().len(), 3);
        assert_eq!(entry.active.as_ref().unwrap().child_slots().len(), 2);
        assert!(entry.cleanup_ledger.is_none());
    }

    fn prepare_nested_branch_for_commit(
        app: &mut App,
        router: RouterHandle,
        location: RouteLocation,
    ) -> NavigationRequestId {
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, location)
            .unwrap();
        collect_navigation_system(app.world_mut());
        for _ in 0..3 {
            drive_navigation_system(app.world_mut());
        }
        let navigation = app.world().resource::<RouterNavigation>();
        let entry = &navigation.entries[0];
        let pending = entry.pending_commit.as_ref().unwrap();
        let projection = match (pending.ticket.as_ref(), pending.final_task.as_ref()) {
            (Some(ticket), _) => entry
                .engine
                .reconciliation_projection_for_commit(ticket)
                .expect("pending commit ticket retains its sealed projection"),
            (None, Some(final_task)) => entry
                .engine
                .reconciliation_projection_for_task(final_task)
                .expect("pending leaf retains its sealed projection"),
            (None, None) => panic!("pending branch retains a ticket or final task"),
        };
        assert_eq!(pending.branch.frames().len(), projection.entering().len());
        assert_eq!(
            pending.branch.child_slots().len(),
            projection
                .child_slots()
                .filter(|slot| {
                    slot.kind()
                        == bevy_ui_router_core::integration::ReconciliationChildSlotRoleKind::MountNew
                })
                .count()
        );
        request
    }

    fn commit_initial_nested_branch(app: &mut App, router: RouterHandle) {
        prepare_nested_branch_for_commit(app, router, RouteLocation::new("/items/1"));
        commit_presentation_system(app.world_mut());
        settle_cleanup_system(app.world_mut());
        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert!(entry.pending_commit.is_none());
        assert_eq!(entry.active.as_ref().unwrap().frames().len(), 2);
        assert!(entry.cleanup_ledger.is_none());
    }

    fn run_updates_until_succeeded(app: &mut App, request: NavigationRequestId) {
        for _ in 0..8 {
            app.update();
            if app
                .world()
                .resource::<RouterNavigation>()
                .entries
                .iter()
                .any(|entry| {
                    entry
                        .engine
                        .request(request)
                        .is_some_and(|request| matches!(request.status(), RequestStatus::Succeeded))
                })
            {
                return;
            }
        }
        panic!("scheduled navigation request did not succeed within eight updates");
    }

    #[allow(clippy::too_many_arguments)]
    fn assert_reconciliation_child_slot_state(
        world: &World,
        child_slot: &crate::mount::MountedChildSlot,
        baseline: &ExactRegistryClaim,
        logical_parent: &RouteFrameVersion,
        anchor: Entity,
        physical_parent: Entity,
        parent_children: &[Entity],
        lifecycle: EntityLifecycle,
        slot_lifecycle: ChildSlotLifecycle,
        visibility: Visibility,
    ) -> ExactRegistryClaim {
        assert_eq!(child_slot.slot(), baseline.entity());
        assert_eq!(child_slot.parent(), logical_parent);
        assert_eq!(child_slot.anchor(), anchor);
        let claim = child_slot.slot_claim().clone();
        assert_eq!(claim.entity(), baseline.entity());
        assert_eq!(claim.owner(), baseline.owner());
        assert_eq!(claim.seal(), baseline.seal());
        assert_eq!(claim.lifecycle(), lifecycle);
        assert_eq!(
            world
                .get::<RouteOwnershipSeal>(claim.entity())
                .copied()
                .map(RouteOwnershipSeal::seal),
            Some(claim.seal())
        );
        let authority = world
            .get::<ChildSlotAuthority>(claim.entity())
            .expect("mounted child slot retains its exact authority binding");
        assert_eq!(authority.parent(), child_slot.parent());
        assert_eq!(authority.outlet(), child_slot.outlet());
        assert_eq!(authority.slot(), child_slot.slot_id());
        assert_eq!(
            world
                .resource::<RouteEntityRegistry>()
                .record(claim.entity())
                .map(|record| record.claim()),
            Some(claim.clone())
        );
        assert_eq!(
            world.get::<ChildSlotLifecycle>(claim.entity()),
            Some(&slot_lifecycle)
        );
        assert_eq!(world.get::<Visibility>(claim.entity()), Some(&visibility));
        assert_eq!(
            world.get::<Pickable>(claim.entity()),
            Some(&Pickable::IGNORE)
        );
        assert!(
            world
                .get::<RouterInputBarrier>(claim.entity())
                .is_some_and(RouterInputBarrier::denies_all)
        );
        let gate = world.resource::<RouterInputGate>();
        assert!(!gate.allows_pointer(claim.entity()));
        assert!(!gate.allows_keyboard(claim.entity()));
        assert!(!gate.allows_gamepad(claim.entity()));
        assert_eq!(
            world.get::<ChildOf>(claim.entity()).map(ChildOf::parent),
            Some(physical_parent)
        );
        assert_eq!(
            world
                .get::<Children>(physical_parent)
                .map(|children| children.iter().copied().collect::<Vec<_>>())
                .unwrap_or_default(),
            parent_children
        );
        claim
    }

    #[derive(Clone, Copy)]
    enum ChildSlotBoundaryDrift {
        ForeignComponent,
        ForeignDescendant,
    }

    fn apply_child_slot_boundary_drift(
        app: &mut App,
        slot: Entity,
        drift: ChildSlotBoundaryDrift,
    ) -> Option<Entity> {
        match drift {
            ChildSlotBoundaryDrift::ForeignComponent => {
                app.world_mut()
                    .entity_mut(slot)
                    .insert(FinalPreflightForeignComponent);
                None
            }
            ChildSlotBoundaryDrift::ForeignDescendant => {
                let foreign = app.world_mut().spawn(NestedAppHostSentinel(0x51_07)).id();
                app.world_mut().entity_mut(slot).add_child(foreign);
                Some(foreign)
            }
        }
    }

    fn install_foreign_component_hooks(app: &mut App) {
        app.init_resource::<ForeignComponentHooks>();
        app.world_mut()
            .register_component_hooks::<FinalPreflightForeignComponent>()
            .on_remove(|mut world, HookContext { .. }| {
                world.resource_mut::<ForeignComponentHooks>().removed += 1;
            })
            .on_despawn(|mut world, HookContext { .. }| {
                world.resource_mut::<ForeignComponentHooks>().despawned += 1;
            });
    }

    fn assert_child_slot_boundary_drift_remains_foreign(
        world: &World,
        slot: Entity,
        foreign: Option<Entity>,
        drift: ChildSlotBoundaryDrift,
    ) {
        match drift {
            ChildSlotBoundaryDrift::ForeignComponent => assert!(
                world.get::<FinalPreflightForeignComponent>(slot).is_some(),
                "cleanup must not remove the app-owned component"
            ),
            ChildSlotBoundaryDrift::ForeignDescendant => {
                let foreign = foreign.expect("descendant drift records its foreign entity");
                assert!(world.get_entity(foreign).is_ok());
                assert_eq!(
                    world.get::<ChildOf>(foreign).map(ChildOf::parent),
                    Some(slot)
                );
                assert_eq!(
                    world.get::<NestedAppHostSentinel>(foreign),
                    Some(&NestedAppHostSentinel(0x51_07))
                );
            }
        }
    }

    fn returned_receipt_bundle_fixture() -> (App, NavigationRequestId, Entity, Vec<Entity>) {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let router = install_nested_router(&mut app, observed.clone(), true);
        observe_authority_system(app.world_mut());
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/items/1"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());

        let (request, slot, frame_entities) = app.world_mut().resource_scope(
            |world, mut navigation: bevy_ecs::change_detection::Mut<RouterNavigation>| {
                let entry = &mut navigation.entries[0];
                let pending = entry.pending_commit.take().unwrap();
                let EngineAction::MountRequest(mount) = entry.engine.action().unwrap() else {
                    panic!("realized parent must retain its mount request")
                };
                let request = mount.request();
                let outlet = mount.outlet().clone();
                let grant = entry.engine.resolve_mount(mount).unwrap();
                let parent = pending.branch.last_frame().unwrap();
                let staged = stage_child_slot(world, parent, &outlet, &grant).unwrap();
                let slot = staged.slot();
                let frame_entities = pending
                    .branch
                    .frames()
                    .iter()
                    .flat_map(RealizedFrame::claims)
                    .map(crate::registry::ExactRegistryClaim::entity)
                    .collect::<Vec<_>>();
                let (receipt, proof) = grant.attest_receipt_with_proof(staged.receipt().digest());
                let mut foreign = RouterEngine::install(
                    bevy_ui_router_core::test_support::nested_items_view_route_tree(),
                    RouteViewKey::new("fallback").unwrap(),
                )
                .unwrap();
                let error = match foreign.accept_mount_receipt(receipt) {
                    Ok(_) => panic!("foreign engine must return the receipt capability"),
                    Err(error) => error,
                };
                entry.pending_mount_mutation_rejection = Some(PendingMountMutationRejection {
                    pending,
                    artifact: MountMutationArtifact::Receipt(error.into_owner()),
                    payload: PendingMountMutationPayload::Receipt { staged, proof },
                    allowance: None,
                });
                (request, slot, frame_entities)
            },
        );
        (app, request, slot, frame_entities)
    }

    #[test]
    fn returned_receipt_bundle_rejects_into_sealed_failure_cleanup() {
        let (mut app, request, slot, frame_entities) = returned_receipt_bundle_fixture();
        {
            let navigation = app.world().resource::<RouterNavigation>();
            let entry = &navigation.entries[0];
            let bundle = entry.pending_mount_mutation_rejection.as_ref().unwrap();
            let PendingMountMutationPayload::Receipt { staged, proof } = &bundle.payload else {
                panic!("returned receipt must retain its staged receipt payload")
            };
            assert_eq!(staged.slot(), slot);
            assert!(matches!(bundle.artifact, MountMutationArtifact::Receipt(_)));
            let _proof = proof;
            assert!(entry.pending_commit.is_none());
        }
        assert!(app.world().get_entity(slot).is_ok());
        assert!(
            app.world()
                .resource::<RouteEntityRegistry>()
                .record(slot)
                .is_some()
        );

        drive_navigation_system(app.world_mut());
        {
            let navigation = app.world().resource::<RouterNavigation>();
            let entry = &navigation.entries[0];
            assert!(entry.pending_mount_mutation_rejection.is_none());
            let failed = entry.pending_failed_branch.as_ref().unwrap();
            assert_eq!(failed.branch.frames().len(), 1);
            assert_eq!(failed.branch.mount_residuals().len(), 1);
            assert!(matches!(
                entry.engine.request(request).unwrap().status(),
                RequestStatus::Running(NavigationPhase::CleanupPending)
            ));
        }

        settle_cleanup_system(app.world_mut());

        let navigation = app.world().resource::<RouterNavigation>();
        let entry = &navigation.entries[0];
        assert!(entry.pending_failed_branch.is_none());
        assert!(entry.cleanup_ledger.is_none());
        let request = entry.engine.request(request).unwrap();
        assert!(matches!(request.status(), RequestStatus::Failed(_)));
        assert!(request.trace().iter().any(|event| {
            event.kind() == NavigationTraceEventKind::CleanupSettled
                && event.cleanup_disposition().is_some()
        }));
        assert_eq!(
            request.trace().last().map(|event| event.kind()),
            Some(NavigationTraceEventKind::RequestTerminal)
        );
        assert!(app.world().get_entity(slot).is_err());
        assert!(
            frame_entities
                .iter()
                .all(|entity| app.world().get_entity(*entity).is_err())
        );
    }

    #[test]
    fn receipt_rejection_error_retains_one_bundle_until_exact_retry() {
        let (mut app, request, slot, frame_entities) = returned_receipt_bundle_fixture();
        let registry_before = app
            .world()
            .resource::<RouteEntityRegistry>()
            .records()
            .to_vec();
        let world_before = authority_world_fingerprint(app.world());
        {
            let mut navigation = app.world_mut().resource_mut::<RouterNavigation>();
            navigation.entries[0]
                .engine
                .testing_set_mount_receipt_context_live(false);
        }

        drive_navigation_system(app.world_mut());

        {
            let navigation = app.world().resource::<RouterNavigation>();
            let entry = &navigation.entries[0];
            let bundle = entry.pending_mount_mutation_rejection.as_ref().unwrap();
            let PendingMountMutationPayload::Receipt { staged, proof } = &bundle.payload else {
                panic!("receipt rejection retry must retain the receipt payload")
            };
            assert_eq!(staged.slot(), slot);
            assert_eq!(
                bundle
                    .pending
                    .branch
                    .frames()
                    .iter()
                    .flat_map(RealizedFrame::claims)
                    .map(crate::registry::ExactRegistryClaim::entity)
                    .collect::<Vec<_>>(),
                frame_entities
            );
            assert!(matches!(bundle.artifact, MountMutationArtifact::Receipt(_)));
            let _proof = proof;
            assert!(entry.pending_failed_branch.is_none());
            assert!(entry.cleanup_ledger.is_none());
            assert!(matches!(
                entry.engine.request(request).unwrap().status(),
                RequestStatus::Running(NavigationPhase::ApplyingFrame(_))
                    | RequestStatus::Running(NavigationPhase::CleanupPending)
            ));
        }
        assert_eq!(authority_world_fingerprint(app.world()), world_before);
        assert_eq!(
            app.world().resource::<RouteEntityRegistry>().records(),
            registry_before
        );

        {
            let mut navigation = app.world_mut().resource_mut::<RouterNavigation>();
            navigation.entries[0]
                .engine
                .testing_set_mount_receipt_context_live(true);
        }
        drive_navigation_system(app.world_mut());
        settle_cleanup_system(app.world_mut());

        let navigation = app.world().resource::<RouterNavigation>();
        let entry = &navigation.entries[0];
        assert!(entry.pending_mount_mutation_rejection.is_none());
        assert!(entry.pending_failed_branch.is_none());
        assert!(entry.cleanup_ledger.is_none());
        assert!(matches!(
            entry.engine.request(request).unwrap().status(),
            RequestStatus::Failed(_)
        ));
        assert_eq!(
            entry
                .engine
                .request(request)
                .unwrap()
                .trace()
                .iter()
                .filter(|event| {
                    event.kind()
                        == bevy_ui_router_core::prelude::NavigationTraceEventKind::MountRejectedAfterMutation
                })
                .count(),
            1
        );
    }

    pub(crate) fn failed_nested_branch_fixture()
    -> (App, FailedPreparedBranch, Vec<Entity>, Entity, Vec<Entity>) {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let router = install_nested_router(&mut app, observed, true);
        observe_authority_system(app.world_mut());
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/items/1"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        let (branch, child_entities, child_slot, parent_entities) = {
            let mut navigation = app.world_mut().resource_mut::<RouterNavigation>();
            let pending = navigation.entries[0].pending_commit.take().unwrap();
            let child_entities = pending.branch.frames()[1]
                .claims()
                .map(|claim| claim.entity())
                .collect::<Vec<_>>();
            let child_slot = pending.branch.child_slots()[0].slot();
            let parent_entities = pending.branch.frames()[0]
                .claims()
                .map(|claim| claim.entity())
                .collect::<Vec<_>>();
            (
                FailedPreparedBranch::new(pending.branch),
                child_entities,
                child_slot,
                parent_entities,
            )
        };
        (app, branch, child_entities, child_slot, parent_entities)
    }

    pub(crate) fn failed_deep_nested_branch_fixture() -> (App, FailedPreparedBranch) {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let router = install_deep_nested_router(&mut app);
        observe_authority_system(app.world_mut());
        prepare_deep_nested_branch_for_commit(
            &mut app,
            router,
            RouteLocation::new("/sections/a/items/1"),
        );
        let branch = {
            let mut navigation = app.world_mut().resource_mut::<RouterNavigation>();
            FailedPreparedBranch::new(navigation.entries[0].pending_commit.take().unwrap().branch)
        };
        (app, branch)
    }

    #[test]
    fn failed_boundary_classifier_binds_exact_current_host_and_pending_slot_output() {
        let (app, branch) = failed_deep_nested_branch_fixture();
        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        let host = entry.host.as_ref().unwrap();
        let active = branch.realized_for_test();

        let host_fact = FailedBoundaryDrift {
            source: FailedBoundarySource::FrameMountSlot {
                frame: active.frames()[0].frame().clone(),
                removed_child: active.frames()[0].shell(),
            },
            expected: FailedBoundaryExpectation::ExactClaim(host.top_slot_claim.clone()),
            state: crate::cleanup::FailedManifestClaimState::Replaced,
            predicate: crate::cleanup::FailedMountParentFreezePredicate::ClaimAuthority,
        };
        assert!(matches!(
            classify_failed_boundary(host_fact, &branch, None, Some(host),),
            FailedBoundaryDisposition::CurrentAuthorityLoss(_)
        ));

        let child_slot = &active.child_slots()[0];
        let child_frame = active
            .frames()
            .iter()
            .find(|frame| frame.mount_binding().matches_mounted_child_slot(child_slot))
            .unwrap();
        let child_slot_fact = FailedBoundaryDrift {
            source: FailedBoundarySource::FrameMountSlot {
                frame: child_frame.frame().clone(),
                removed_child: child_frame.shell(),
            },
            expected: FailedBoundaryExpectation::ExactClaim(child_slot.slot_claim().clone()),
            state: crate::cleanup::FailedManifestClaimState::PhysicalDrift,
            predicate: crate::cleanup::FailedMountParentFreezePredicate::ClaimAuthority,
        };
        assert!(matches!(
            classify_failed_boundary(child_slot_fact, &branch, None, Some(host),),
            FailedBoundaryDisposition::PendingOnlyDrift(_)
        ));

        let output = active.frames()[0]
            .claims()
            .find(|claim| {
                matches!(
                    claim.owner(),
                    RegistryOwner::Committed(EntityOwner::FrameOutput { .. })
                )
            })
            .unwrap();
        let output_fact = FailedBoundaryDrift {
            source: FailedBoundarySource::MountedSlotAnchor {
                parent: active.frames()[0].frame().clone(),
                outlet: active.frames()[0].outlets()[0].key().clone(),
                slot: child_slot.slot_id(),
                removed_child: child_slot.slot(),
            },
            expected: FailedBoundaryExpectation::ExactClaim(output.clone()),
            state: crate::cleanup::FailedManifestClaimState::Missing,
            predicate: crate::cleanup::FailedMountParentFreezePredicate::ClaimAuthority,
        };
        assert!(matches!(
            classify_failed_boundary(output_fact, &branch, None, Some(host),),
            FailedBoundaryDisposition::PendingOnlyDrift(_)
        ));
    }

    #[test]
    fn failed_boundary_classifier_distinguishes_pending_only_and_simultaneous_ownership() {
        let (app, branch) = failed_deep_nested_branch_fixture();
        let host = app.world().resource::<RouterNavigation>().entries[0]
            .host
            .as_ref()
            .unwrap();
        let active = branch.realized_for_test();
        let child_slot = &active.child_slots()[0];
        let child_frame = active
            .frames()
            .iter()
            .find(|frame| frame.mount_binding().matches_mounted_child_slot(child_slot))
            .unwrap();
        let fact = FailedBoundaryDrift {
            source: FailedBoundarySource::FrameMountSlot {
                frame: child_frame.frame().clone(),
                removed_child: child_frame.shell(),
            },
            expected: FailedBoundaryExpectation::ExactClaim(child_slot.slot_claim().clone()),
            state: crate::cleanup::FailedManifestClaimState::Replaced,
            predicate: crate::cleanup::FailedMountParentFreezePredicate::ClaimAuthority,
        };

        assert!(matches!(
            classify_failed_boundary(fact.clone(), &branch, None, Some(host)),
            FailedBoundaryDisposition::PendingOnlyDrift(_)
        ));
        assert!(matches!(
            classify_failed_boundary(fact, &branch, Some(active), Some(host)),
            FailedBoundaryDisposition::AmbiguousInvalidTopology(_)
        ));
    }

    #[test]
    fn failed_boundary_classifier_rejects_wrong_source_tuple_for_same_frame_output_owner() {
        let (app, branch) = failed_deep_nested_branch_fixture();
        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        let active = branch.realized_for_test();
        let parent = &active.frames()[0];
        let child_slot = &active.child_slots()[0];
        let output = parent
            .claims()
            .find(|claim| {
                matches!(
                    claim.owner(),
                    RegistryOwner::Committed(EntityOwner::FrameOutput { frame, .. })
                        if frame == parent.frame()
                )
            })
            .unwrap()
            .clone();
        let wrong_outlet =
            bevy_ui_router_core::slot::RouteSlotKey::named("review-wrong-outlet").unwrap();
        let wrong_output = active.frames()[1]
            .claims()
            .find(|claim| {
                matches!(
                    claim.owner(),
                    RegistryOwner::Committed(EntityOwner::FrameOutput { .. })
                )
            })
            .unwrap()
            .clone();
        let wrong_local_index = match output.owner() {
            RegistryOwner::Committed(EntityOwner::FrameOutput {
                frame,
                output: output_id,
                local_index,
            }) => output.with_owner_for_test(RegistryOwner::Committed(EntityOwner::FrameOutput {
                frame: frame.clone(),
                output: *output_id,
                local_index: local_index + 1,
            })),
            _ => unreachable!(),
        };
        let exact_outlet = parent.outlets()[0].key().clone();
        let wrong_slot = active.child_slots()[1].slot_id();
        let facts = [
            FailedBoundaryDrift {
                source: FailedBoundarySource::MountedSlotAnchor {
                    parent: parent.frame().clone(),
                    outlet: wrong_outlet,
                    slot: child_slot.slot_id(),
                    removed_child: child_slot.slot(),
                },
                expected: FailedBoundaryExpectation::ExactClaim(output.clone()),
                state: crate::cleanup::FailedManifestClaimState::Missing,
                predicate: crate::cleanup::FailedMountParentFreezePredicate::ClaimAuthority,
            },
            FailedBoundaryDrift {
                source: FailedBoundarySource::MountedSlotAnchor {
                    parent: parent.frame().clone(),
                    outlet: exact_outlet.clone(),
                    slot: wrong_slot,
                    removed_child: child_slot.slot(),
                },
                expected: FailedBoundaryExpectation::ExactClaim(output.clone()),
                state: crate::cleanup::FailedManifestClaimState::Missing,
                predicate: crate::cleanup::FailedMountParentFreezePredicate::ClaimAuthority,
            },
            FailedBoundaryDrift {
                source: FailedBoundarySource::MountedSlotAnchor {
                    parent: parent.frame().clone(),
                    outlet: exact_outlet.clone(),
                    slot: child_slot.slot_id(),
                    removed_child: parent.shell(),
                },
                expected: FailedBoundaryExpectation::ExactClaim(output.clone()),
                state: crate::cleanup::FailedManifestClaimState::Missing,
                predicate: crate::cleanup::FailedMountParentFreezePredicate::ClaimAuthority,
            },
            FailedBoundaryDrift {
                source: FailedBoundarySource::MountedSlotAnchor {
                    parent: parent.frame().clone(),
                    outlet: exact_outlet.clone(),
                    slot: child_slot.slot_id(),
                    removed_child: child_slot.slot(),
                },
                expected: FailedBoundaryExpectation::ExactClaim(wrong_output),
                state: crate::cleanup::FailedManifestClaimState::Missing,
                predicate: crate::cleanup::FailedMountParentFreezePredicate::ClaimAuthority,
            },
            FailedBoundaryDrift {
                source: FailedBoundarySource::MountedSlotAnchor {
                    parent: parent.frame().clone(),
                    outlet: exact_outlet,
                    slot: child_slot.slot_id(),
                    removed_child: child_slot.slot(),
                },
                expected: FailedBoundaryExpectation::ExactClaim(wrong_local_index),
                state: crate::cleanup::FailedManifestClaimState::Missing,
                predicate: crate::cleanup::FailedMountParentFreezePredicate::ClaimAuthority,
            },
        ];

        for fact in facts {
            assert!(matches!(
                classify_failed_boundary(
                    fact,
                    &FailedPreparedBranch::new(PreparedBranch::empty()),
                    Some(active),
                    entry.host.as_ref(),
                ),
                FailedBoundaryDisposition::AmbiguousInvalidTopology(_)
            ));
        }
    }

    #[test]
    fn ambiguous_boundary_owner_is_fail_closed_once_with_exact_owner_retained() {
        let mut app = App::new();
        let (request, _router) = prepare_exact_child_stale_failure(&mut app);
        app.world_mut().resource_scope(
            |world, mut navigation: bevy_ecs::change_detection::Mut<RouterNavigation>| {
                let entry = &mut navigation.entries[0];
                let failed = entry.pending_failed_branch.take().unwrap();
                let frame = &failed.branch.frames()[0];
                let output = frame
                    .claims()
                    .find(|claim| {
                        matches!(
                            claim.owner(),
                            RegistryOwner::Committed(EntityOwner::FrameOutput { .. })
                        )
                    })
                    .unwrap()
                    .clone();
                let mut manifest = FrozenCleanupManifest::from_failed_prepared_branch(
                    world,
                    world.resource::<RouteEntityRegistry>(),
                    &failed.branch,
                )
                .unwrap();
                manifest.replace_boundary_drifts_for_test(vec![FailedBoundaryDrift {
                    source: FailedBoundarySource::FrameMountSlot {
                        frame: frame.frame().clone(),
                        removed_child: frame.shell(),
                    },
                    expected: FailedBoundaryExpectation::ExactClaim(output),
                    state: crate::cleanup::FailedManifestClaimState::Replaced,
                    predicate: crate::cleanup::FailedMountParentFreezePredicate::ClaimAuthority,
                }]);
                let FailedManifestDisposition::AmbiguousInvalidTopology { failed, manifest } =
                    classify_failed_manifest(
                        manifest,
                        failed,
                        entry.active.as_ref(),
                        entry.host.as_ref(),
                    )
                else {
                    panic!("source/owner-kind contradiction must retain an ambiguous owner")
                };
                assert_eq!(manifest.boundary_drifts().len(), 1);
                assert!(
                    fail_closed_failed_manifest_protocol(
                        world,
                        entry,
                        failed,
                        FailedPreparedTopologyError::AmbiguousBoundaryOwnership,
                    )
                    .is_ok()
                );
            },
        );

        let trace_len = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            let residual = entry.authority_protocol_residual.as_ref().unwrap();
            assert!(matches!(
                residual.kind,
                AuthorityProtocolResidualKind::FailedManifest(
                    FailedPreparedTopologyError::AmbiguousBoundaryOwnership
                )
            ));
            assert!(residual.failed.is_some());
            assert!(residual.host.is_some());
            assert!(entry.host.is_none());
            assert!(entry.pending_failed_branch.is_none());
            assert_eq!(
                entry.engine.snapshot().availability(),
                &RouterAvailability::Unavailable(
                    RouterUnavailableReason::RegistryInvariantViolation
                )
            );
            entry.engine.request(request).unwrap().trace().len()
        };

        settle_cleanup_system(app.world_mut());

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert_eq!(
            entry.engine.request(request).unwrap().trace().len(),
            trace_len
        );
        assert!(entry.authority_protocol_residual.is_some());
        assert!(entry.pending_failed_branch.is_none());
        assert!(entry.cleanup_ledger.is_none());
    }

    #[test]
    fn current_authority_loss_host_top_slot_enters_unique_authority_publisher() {
        let mut app = App::new();
        let (request, _router) = prepare_exact_child_stale_failure(&mut app);
        let (host, top_slot, host_children_before) = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            let host = entry.host.as_ref().unwrap();
            (
                host.host,
                host.top_slot,
                app.world()
                    .get::<bevy_ecs::hierarchy::Children>(host.host)
                    .map(|children| children.iter().copied().collect::<Vec<_>>())
                    .unwrap_or_default(),
            )
        };
        let stale_seal = app
            .world_mut()
            .resource_mut::<RouteEntityRegistry>()
            .issue_seal();
        app.world_mut()
            .entity_mut(top_slot)
            .insert(RouteOwnershipSeal::new(stale_seal));

        settle_cleanup_system(app.world_mut());

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert_eq!(
            entry.engine.snapshot().availability(),
            &RouterAvailability::RecoveringAuthority
        );
        assert!(matches!(
            entry.engine.request(request).unwrap().status(),
            RequestStatus::Running(NavigationPhase::Recovering)
        ));
        assert!(entry.pending_failed_branch.is_none());
        assert!(entry.active.is_none());
        assert!(entry.host.is_none());
        assert!(
            entry
                .cleanup_ledger
                .as_ref()
                .is_some_and(CleanupLedger::is_authority_settlement),
            "current authority loss must enter the one authority cleanup ledger"
        );
        assert!(entry.authority_protocol_residual.is_none());
        assert_eq!(
            app.world()
                .get::<bevy_ecs::hierarchy::ChildOf>(top_slot)
                .map(bevy_ecs::hierarchy::ChildOf::parent),
            Some(host)
        );
        assert_eq!(
            app.world()
                .get::<bevy_ecs::hierarchy::Children>(host)
                .map(|children| children.iter().copied().collect::<Vec<_>>())
                .unwrap_or_default(),
            host_children_before
        );

        for _ in 0..8 {
            settle_cleanup_system(app.world_mut());
            if app.world().resource::<RouterNavigation>().entries[0]
                .cleanup_ledger
                .is_none()
            {
                break;
            }
        }

        let trace_len = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            assert!(matches!(
                entry.engine.request(request).unwrap().status(),
                RequestStatus::Failed(failure)
                    if failure.kind() == NavigationFailureKind::AuthorityLost
            ));
            assert!(entry.cleanup_ledger.is_none());
            assert!(app.world().get_entity(top_slot).is_ok());
            assert_eq!(
                app.world()
                    .get::<bevy_ecs::hierarchy::ChildOf>(top_slot)
                    .map(bevy_ecs::hierarchy::ChildOf::parent),
                Some(host)
            );
            assert_eq!(
                app.world()
                    .get::<bevy_ecs::hierarchy::Children>(host)
                    .map(|children| children.iter().copied().collect::<Vec<_>>())
                    .unwrap_or_default(),
                host_children_before
            );
            assert!(
                app.world()
                    .resource::<QuarantineLeaseTable>()
                    .authority_lost_tombstone(top_slot)
                    .is_some()
            );
            entry.engine.request(request).unwrap().trace().len()
        };
        settle_cleanup_system(app.world_mut());
        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert_eq!(
            entry.engine.request(request).unwrap().trace().len(),
            trace_len
        );
        assert!(entry.cleanup_ledger.is_none());
    }

    #[test]
    fn failed_task_missing_pickable_after_current_boundary_loss_fail_closes_once() {
        let mut app = App::new();
        let (request, _router) = prepare_reconciliation_stale_failure_with_active(&mut app);
        let (current_boundary, target, focused, stored_claims, history_before) = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            let active = entry
                .active
                .as_ref()
                .expect("fixture retains an exact current active owner");
            let failed = entry.pending_failed_branch.as_ref().unwrap();
            assert!(matches!(
                &failed.terminal,
                PendingFailedBranchTerminal::Task { .. }
            ));
            let current_boundary = failed.branch.frames()[0]
                .mount_binding()
                .physical_parent()
                .1
                .expect("failed frame mounts below one exact current boundary")
                .clone();
            let focused = *active
                .interactive_entities()
                .first()
                .expect("active branch has one current interactive focus target");
            let target = active
                .claims()
                .find(|claim| {
                    claim.entity() != current_boundary.entity()
                        && app.world().get::<Pickable>(claim.entity()).is_some()
                })
                .expect("active branch has one exact non-boundary isolation target")
                .entity();
            let mut stored_claims = failed_branch_primary_claims(&failed.branch)
                .into_iter()
                .cloned()
                .chain(active.claims().cloned())
                .collect::<Vec<_>>();
            stored_claims.push(entry.host.as_ref().unwrap().top_slot_claim.clone());
            (
                current_boundary,
                target,
                focused,
                stored_claims,
                entry.engine.history().clone(),
            )
        };
        assert_eq!(app.world().get::<Pickable>(target).is_some(), true);
        assert!(stored_claims.iter().any(|claim| claim.entity() == target));

        let stale_seal = app
            .world_mut()
            .resource_mut::<RouteEntityRegistry>()
            .issue_seal();
        app.world_mut()
            .entity_mut(current_boundary.entity())
            .insert(RouteOwnershipSeal::new(stale_seal));
        app.world_mut().entity_mut(target).remove::<Pickable>();
        *app.world_mut().resource_mut::<InputFocus>() = InputFocus::from_entity(focused);
        app.world_mut()
            .resource_mut::<RouteFocusState>()
            .set_focused(Some(focused));
        assert!(
            app.world()
                .resource::<RouterInputGate>()
                .allows_pointer(focused)
        );

        let exact_entities = stored_claims
            .iter()
            .filter(|claim| {
                app.world()
                    .resource::<RouteEntityRegistry>()
                    .record(claim.entity())
                    .is_some_and(|record| record.claim() == **claim)
                    && cleanup_claim_has_exact_authority(app.world(), claim)
            })
            .map(ExactRegistryClaim::entity)
            .collect::<Vec<_>>();
        assert!(exact_entities.contains(&target));
        assert!(!exact_entities.contains(&current_boundary.entity()));
        let mut relationship_entities = stored_claims
            .iter()
            .map(ExactRegistryClaim::entity)
            .collect::<Vec<_>>();
        relationship_entities.push(
            app.world().resource::<RouterNavigation>().entries[0]
                .host
                .as_ref()
                .unwrap()
                .host,
        );
        relationship_entities.sort_by_key(|entity| entity.to_bits());
        relationship_entities.dedup();
        let relationships_before = relationship_entities
            .iter()
            .map(|entity| {
                (
                    *entity,
                    app.world().get::<ChildOf>(*entity).map(ChildOf::parent),
                    app.world()
                        .get::<Children>(*entity)
                        .map(|children| children.iter().copied().collect::<Vec<_>>())
                        .unwrap_or_default(),
                )
            })
            .collect::<Vec<_>>();
        let registry_before = app
            .world()
            .resource::<RouteEntityRegistry>()
            .records()
            .to_vec();

        settle_cleanup_system(app.world_mut());

        let trace_len = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            let residual = entry
                .authority_protocol_residual
                .as_ref()
                .unwrap_or_else(|| {
                    panic!(
                        "missing exact isolation capability is terminal: pending_failed={} active={} host={} cleanup={} status={:?} availability={:?}",
                        entry.pending_failed_branch.is_some(),
                        entry.active.is_some(),
                        entry.host.is_some(),
                        entry.cleanup_ledger.is_some(),
                        entry.engine.request(request).unwrap().status(),
                        entry.engine.snapshot().availability(),
                    )
                });
            let expected_violation = AuthorityIsolationViolation {
                entity: target,
                component: AuthorityIsolationComponent::Pickable,
                predicate: AuthorityIsolationPredicate::PresentForIgnoreWrite,
            };
            assert!(matches!(
                &residual.kind,
                AuthorityProtocolResidualKind::AuthorityIsolation(contradiction)
                    if contradiction.violations.contains(&expected_violation)
            ));
            assert!(
                residual
                    .isolation_contradiction
                    .as_ref()
                    .is_some_and(|contradiction| {
                        contradiction.violations.contains(&expected_violation)
                    })
            );
            assert!(residual.active.is_some());
            assert!(residual.failed.is_some());
            assert!(residual.host.is_some());
            assert!(entry.active.is_none());
            assert!(entry.pending_failed_branch.is_none());
            assert!(entry.host.is_none());
            assert!(entry.cleanup_ledger.is_none());
            assert_eq!(entry.engine.history(), &history_before);
            assert!(matches!(
                entry.engine.request(request).unwrap().status(),
                RequestStatus::Failed(failure)
                    if failure.kind() == NavigationFailureKind::AuthorityLost
            ));
            entry.engine.request(request).unwrap().trace().len()
        };
        assert_eq!(app.world().resource::<InputFocus>().get(), None);
        assert_eq!(app.world().resource::<RouteFocusState>().focused(), None);
        let gate = app.world().resource::<RouterInputGate>();
        assert!(!gate.allows_pointer(focused));
        assert!(!gate.allows_keyboard(focused));
        assert!(!gate.allows_gamepad(focused));
        assert_eq!(app.world().get::<Pickable>(target), None);
        assert!(exact_entities.iter().all(|entity| {
            app.world()
                .get::<Visibility>(*entity)
                .is_none_or(|visibility| *visibility == Visibility::Hidden)
                && app
                    .world()
                    .get::<Pickable>(*entity)
                    .is_none_or(|pickable| *pickable == Pickable::IGNORE)
                && app
                    .world()
                    .get::<RouterInputBarrier>(*entity)
                    .is_none_or(RouterInputBarrier::denies_all)
        }));
        assert_eq!(
            app.world().resource::<RouteEntityRegistry>().records(),
            registry_before
        );
        let relationships_after = relationship_entities
            .iter()
            .map(|entity| {
                (
                    *entity,
                    app.world().get::<ChildOf>(*entity).map(ChildOf::parent),
                    app.world()
                        .get::<Children>(*entity)
                        .map(|children| children.iter().copied().collect::<Vec<_>>())
                        .unwrap_or_default(),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(relationships_after, relationships_before);

        settle_cleanup_system(app.world_mut());

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert_eq!(
            entry.engine.request(request).unwrap().trace().len(),
            trace_len
        );
        assert_eq!(entry.engine.history(), &history_before);
        assert!(entry.authority_protocol_residual.is_some());
        assert!(entry.cleanup_ledger.is_none());
    }

    fn assert_failed_authority_reserve_contradiction_fail_closes_once(
        expected: CleanupLedgerReserveErrorKind,
        inject: fn(),
    ) {
        let mut app = App::new();
        let (request, _router) = prepare_exact_child_stale_failure(&mut app);
        let top_slot = app.world().resource::<RouterNavigation>().entries[0]
            .host
            .as_ref()
            .unwrap()
            .top_slot;
        let stale_seal = app
            .world_mut()
            .resource_mut::<RouteEntityRegistry>()
            .issue_seal();
        app.world_mut()
            .entity_mut(top_slot)
            .insert(RouteOwnershipSeal::new(stale_seal));
        crate::cleanup::reset_ledger_domain_allocations_for_test();
        inject();

        settle_cleanup_system(app.world_mut());

        let trace_len = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            let residual = entry
                .authority_protocol_residual
                .as_ref()
                .expect("permanent ledger reserve contradiction must fail closed once");
            assert!(matches!(
                residual.kind,
                AuthorityProtocolResidualKind::CleanupLedger(kind) if kind == expected
            ));
            assert!(residual.failed.is_some());
            assert!(residual.host.is_some());
            assert!(entry.pending_failed_branch.is_none());
            assert!(entry.host.is_none());
            assert!(entry.cleanup_ledger.is_none());
            assert!(matches!(
                entry.engine.request(request).unwrap().status(),
                RequestStatus::Failed(failure)
                    if failure.kind() == NavigationFailureKind::AuthorityLost
            ));
            entry.engine.request(request).unwrap().trace().len()
        };
        assert_eq!(crate::cleanup::ledger_domain_allocations_for_test(), 0);

        settle_cleanup_system(app.world_mut());

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert_eq!(
            entry.engine.request(request).unwrap().trace().len(),
            trace_len
        );
        assert!(entry.authority_protocol_residual.is_some());
        assert!(entry.cleanup_ledger.is_none());
    }

    #[test]
    fn failed_authority_ledger_domain_exhaustion_fail_closes_once_without_replay() {
        assert_failed_authority_reserve_contradiction_fail_closes_once(
            CleanupLedgerReserveErrorKind::DomainExhausted,
            crate::cleanup::fail_next_ledger_domain_for_test,
        );
    }

    #[test]
    fn failed_authority_unfinalized_ledger_facts_fail_close_once_without_replay() {
        assert_failed_authority_reserve_contradiction_fail_closes_once(
            CleanupLedgerReserveErrorKind::UnfinalizedBoundaryFacts,
            crate::cleanup::fail_next_unfinalized_ledger_for_test,
        );
    }

    #[test]
    fn normal_authority_missing_visibility_fail_closes_full_owner_once() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let router = install_atomic_router(&mut app, "normal-missing-visibility");
        observe_authority_system(app.world_mut());
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        commit_presentation_system(app.world_mut());
        settle_cleanup_system(app.world_mut());

        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(
                router,
                RouteLocation::new("/").with_fragment("missing-visibility"),
            )
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());

        let (boundary, target, target_claim, focused, history_before) = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            let active = entry.active.as_ref().expect("fixture retains active owner");
            let boundary = entry
                .host
                .as_ref()
                .expect("fixture retains host owner")
                .top_slot_claim
                .clone();
            let target_claim = active
                .claims()
                .find(|claim| {
                    claim.entity() != boundary.entity()
                        && app.world().get::<Visibility>(claim.entity()).is_some()
                })
                .expect("active owner has one exact non-boundary Visibility target")
                .clone();
            let focused = *active
                .interactive_entities()
                .first()
                .expect("active owner has an interactive focus target");
            (
                boundary,
                target_claim.entity(),
                target_claim,
                focused,
                entry.engine.history().clone(),
            )
        };
        assert!(cleanup_claim_has_exact_authority(
            app.world(),
            &target_claim
        ));
        assert!(
            app.world()
                .resource::<RouteEntityRegistry>()
                .record(target)
                .is_some_and(|record| record.claim() == target_claim)
        );

        let stale_seal = app
            .world_mut()
            .resource_mut::<RouteEntityRegistry>()
            .issue_seal();
        app.world_mut()
            .entity_mut(boundary.entity())
            .insert(RouteOwnershipSeal::new(stale_seal));
        app.world_mut().entity_mut(target).remove::<Visibility>();
        *app.world_mut().resource_mut::<InputFocus>() = InputFocus::from_entity(focused);
        app.world_mut()
            .resource_mut::<RouteFocusState>()
            .set_focused(Some(focused));
        assert!(
            app.world()
                .resource::<RouterInputGate>()
                .allows_pointer(focused)
        );

        commit_presentation_system(app.world_mut());

        let trace_len = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            let residual = entry
                .authority_protocol_residual
                .as_ref()
                .expect("normal missing isolation capability must fail closed once");
            let expected_violation = AuthorityIsolationViolation {
                entity: target,
                component: AuthorityIsolationComponent::Visibility,
                predicate: AuthorityIsolationPredicate::PresentForHiddenWrite,
            };
            assert!(matches!(
                &residual.kind,
                AuthorityProtocolResidualKind::AuthorityIsolation(contradiction)
                    if contradiction.violations.contains(&expected_violation)
            ));
            assert!(
                residual
                    .isolation_contradiction
                    .as_ref()
                    .is_some_and(|contradiction| contradiction
                        .violations
                        .contains(&expected_violation))
            );
            assert!(residual.active.is_some());
            assert!(residual.pending.is_some());
            assert!(residual.host.is_some());
            assert!(entry.active.is_none());
            assert!(entry.pending_commit.is_none());
            assert!(entry.host.is_none());
            assert!(entry.cleanup_ledger.is_none());
            assert_eq!(entry.engine.history(), &history_before);
            assert!(matches!(
                entry.engine.request(request).unwrap().status(),
                RequestStatus::Failed(failure)
                    if failure.kind() == NavigationFailureKind::AuthorityLost
            ));
            entry.engine.request(request).unwrap().trace().len()
        };
        assert_eq!(app.world().resource::<InputFocus>().get(), None);
        assert_eq!(app.world().resource::<RouteFocusState>().focused(), None);
        let gate = app.world().resource::<RouterInputGate>();
        assert!(!gate.allows_pointer(focused));
        assert!(!gate.allows_keyboard(focused));
        assert!(!gate.allows_gamepad(focused));
        assert_eq!(app.world().get::<Visibility>(target), None);
        assert_eq!(app.world().get::<Pickable>(target), Some(&Pickable::IGNORE));
        assert!(
            app.world()
                .get::<RouterInputBarrier>(target)
                .is_some_and(RouterInputBarrier::denies_all)
        );

        commit_presentation_system(app.world_mut());

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert_eq!(
            entry.engine.request(request).unwrap().trace().len(),
            trace_len
        );
        assert_eq!(entry.engine.history(), &history_before);
        assert!(entry.authority_protocol_residual.is_some());
        assert!(entry.cleanup_ledger.is_none());
    }

    #[test]
    fn normal_commit_ledger_domain_exhaustion_fail_closes_full_owner_once() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let router = install_atomic_router(&mut app, "normal-ledger-domain-exhausted");
        observe_authority_system(app.world_mut());
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        commit_presentation_system(app.world_mut());
        settle_cleanup_system(app.world_mut());
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(
                router,
                RouteLocation::new("/").with_fragment("domain-exhausted"),
            )
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        crate::cleanup::reset_ledger_domain_allocations_for_test();
        crate::cleanup::fail_next_ledger_domain_for_test();

        commit_presentation_system(app.world_mut());

        let trace_len = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            let residual = entry
                .authority_protocol_residual
                .as_ref()
                .expect("normal permanent ledger exhaustion must fail closed");
            assert!(matches!(
                residual.kind,
                AuthorityProtocolResidualKind::CleanupLedger(
                    CleanupLedgerReserveErrorKind::DomainExhausted
                )
            ));
            assert!(residual.active.is_some());
            assert!(residual.pending.is_some());
            assert!(residual.host.is_some());
            assert!(entry.active.is_none());
            assert!(entry.pending_commit.is_none());
            assert!(entry.host.is_none());
            assert!(entry.cleanup_ledger.is_none());
            assert!(matches!(
                entry.engine.request(request).unwrap().status(),
                RequestStatus::Failed(failure)
                    if failure.kind() == NavigationFailureKind::AuthorityLost
            ));
            entry.engine.request(request).unwrap().trace().len()
        };
        assert_eq!(crate::cleanup::ledger_domain_allocations_for_test(), 0);

        commit_presentation_system(app.world_mut());

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert_eq!(
            entry.engine.request(request).unwrap().trace().len(),
            trace_len
        );
        assert!(entry.authority_protocol_residual.is_some());
        assert!(entry.cleanup_ledger.is_none());
    }

    fn assert_failed_terminal_missing_component_fail_closes_once(
        mut app: App,
        request: NavigationRequestId,
        component: AuthorityIsolationComponent,
    ) {
        let (boundary, target, target_claim, focused, had_active) = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            let failed = entry.pending_failed_branch.as_ref().unwrap();
            let boundary = failed
                .branch
                .frames()
                .iter()
                .find_map(|frame| frame.mount_binding().physical_parent().1.cloned())
                .filter(|claim| {
                    entry
                        .active
                        .as_ref()
                        .is_some_and(|active| active.claims().any(|current| current == claim))
                })
                .unwrap_or_else(|| entry.host.as_ref().unwrap().top_slot_claim.clone());
            let target_claim = failed_branch_primary_claims(&failed.branch)
                .into_iter()
                .chain(entry.active.iter().flat_map(RealizedBranch::claims))
                .find(|claim| {
                    claim.entity() != boundary.entity()
                        && cleanup_claim_has_exact_authority(app.world(), claim)
                        && match component {
                            AuthorityIsolationComponent::Visibility => {
                                app.world().get::<Visibility>(claim.entity()).is_some()
                            }
                            AuthorityIsolationComponent::Pickable => {
                                app.world().get::<Pickable>(claim.entity()).is_some()
                            }
                            AuthorityIsolationComponent::RouterInputBarrier => app
                                .world()
                                .get::<RouterInputBarrier>(claim.entity())
                                .is_some(),
                        }
                })
                .expect("fixture retains an exact non-boundary isolation target")
                .clone();
            let focused = entry
                .active
                .as_ref()
                .into_iter()
                .flat_map(RealizedBranch::interactive_entities)
                .chain(
                    failed
                        .branch
                        .frames()
                        .iter()
                        .chain(failed.branch.detached_frames())
                        .flat_map(RealizedFrame::interactive_entities),
                )
                .next()
                .unwrap_or(target_claim.entity());
            (
                boundary,
                target_claim.entity(),
                target_claim,
                focused,
                entry.active.is_some(),
            )
        };
        let stale_seal = app
            .world_mut()
            .resource_mut::<RouteEntityRegistry>()
            .issue_seal();
        app.world_mut()
            .entity_mut(boundary.entity())
            .insert(RouteOwnershipSeal::new(stale_seal));
        match component {
            AuthorityIsolationComponent::Visibility => {
                app.world_mut().entity_mut(target).remove::<Visibility>();
            }
            AuthorityIsolationComponent::Pickable => {
                app.world_mut().entity_mut(target).remove::<Pickable>();
            }
            AuthorityIsolationComponent::RouterInputBarrier => {
                app.world_mut()
                    .entity_mut(target)
                    .remove::<RouterInputBarrier>();
            }
        }
        assert!(
            app.world()
                .resource::<RouteEntityRegistry>()
                .record(target)
                .is_some_and(|record| record.claim() == target_claim)
        );
        assert!(cleanup_claim_has_exact_authority(
            app.world(),
            &target_claim
        ));
        *app.world_mut().resource_mut::<InputFocus>() = InputFocus::from_entity(focused);
        app.world_mut()
            .resource_mut::<RouteFocusState>()
            .set_focused(Some(focused));

        settle_cleanup_system(app.world_mut());

        let predicate = match component {
            AuthorityIsolationComponent::Visibility => {
                AuthorityIsolationPredicate::PresentForHiddenWrite
            }
            AuthorityIsolationComponent::Pickable => {
                AuthorityIsolationPredicate::PresentForIgnoreWrite
            }
            AuthorityIsolationComponent::RouterInputBarrier => {
                AuthorityIsolationPredicate::PresentForDenyWrite
            }
        };
        let expected = AuthorityIsolationViolation {
            entity: target,
            component,
            predicate,
        };
        let trace_len = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            let residual = entry
                .authority_protocol_residual
                .as_ref()
                .expect("missing exact component must fail closed through the formal caller");
            assert!(matches!(
                &residual.kind,
                AuthorityProtocolResidualKind::AuthorityIsolation(contradiction)
                    if contradiction.violations.contains(&expected)
            ));
            assert!(
                residual
                    .isolation_contradiction
                    .as_ref()
                    .is_some_and(|contradiction| contradiction.violations.contains(&expected))
            );
            assert_eq!(residual.active.is_some(), had_active);
            assert!(residual.failed.is_some());
            assert!(residual.host.is_some());
            assert!(entry.pending_failed_branch.is_none());
            assert!(entry.active.is_none());
            assert!(entry.host.is_none());
            assert!(entry.cleanup_ledger.is_none());
            assert!(matches!(
                entry.engine.request(request).unwrap().status(),
                RequestStatus::Failed(failure)
                    if failure.kind() == NavigationFailureKind::AuthorityLost
            ));
            entry.engine.request(request).unwrap().trace().len()
        };
        assert_eq!(app.world().resource::<InputFocus>().get(), None);
        assert_eq!(app.world().resource::<RouteFocusState>().focused(), None);
        let gate = app.world().resource::<RouterInputGate>();
        assert!(!gate.allows_pointer(focused));
        assert!(!gate.allows_keyboard(focused));
        assert!(!gate.allows_gamepad(focused));
        match component {
            AuthorityIsolationComponent::Visibility => {
                assert_eq!(app.world().get::<Visibility>(target), None);
                assert_eq!(app.world().get::<Pickable>(target), Some(&Pickable::IGNORE));
                assert!(
                    app.world()
                        .get::<RouterInputBarrier>(target)
                        .is_some_and(RouterInputBarrier::denies_all)
                );
            }
            AuthorityIsolationComponent::Pickable => {
                assert_eq!(app.world().get::<Pickable>(target), None);
                assert_eq!(
                    app.world().get::<Visibility>(target),
                    Some(&Visibility::Hidden)
                );
                assert!(
                    app.world()
                        .get::<RouterInputBarrier>(target)
                        .is_some_and(RouterInputBarrier::denies_all)
                );
            }
            AuthorityIsolationComponent::RouterInputBarrier => {
                assert_eq!(app.world().get::<RouterInputBarrier>(target), None);
                assert_eq!(
                    app.world().get::<Visibility>(target),
                    Some(&Visibility::Hidden)
                );
                assert_eq!(app.world().get::<Pickable>(target), Some(&Pickable::IGNORE));
            }
        }

        settle_cleanup_system(app.world_mut());

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert_eq!(
            entry.engine.request(request).unwrap().trace().len(),
            trace_len
        );
        assert!(entry.authority_protocol_residual.is_some());
        assert!(entry.cleanup_ledger.is_none());
    }

    #[test]
    fn failed_physical_task_missing_visibility_fail_closes_once() {
        let mut app = App::new();
        let (request, _router) = prepare_exact_child_physical_residual_failure(&mut app);
        assert!(matches!(
            &app.world().resource::<RouterNavigation>().entries[0]
                .pending_failed_branch
                .as_ref()
                .unwrap()
                .terminal,
            PendingFailedBranchTerminal::TaskWithPhysicalResidual { .. }
        ));
        assert_failed_terminal_missing_component_fail_closes_once(
            app,
            request,
            AuthorityIsolationComponent::Visibility,
        );
    }

    #[test]
    fn failed_cleanup_request_missing_input_barrier_fail_closes_once() {
        let mut app = App::new();
        let (request, _router) = prepare_exact_child_stale_failure(&mut app);
        {
            let mut navigation = app.world_mut().resource_mut::<RouterNavigation>();
            let entry = &mut navigation.entries[0];
            let failed = entry.pending_failed_branch.take().unwrap();
            let PendingFailedBranch {
                terminal:
                    PendingFailedBranchTerminal::Task {
                        task,
                        failure,
                        message,
                    },
                branch,
            } = failed
            else {
                panic!("request fixture starts from a task terminal")
            };
            let cleanup = entry
                .engine
                .report_failed(task, failure, message)
                .unwrap()
                .expect("task failure issues its exact cleanup request");
            entry.pending_failed_branch = Some(PendingFailedBranch {
                terminal: PendingFailedBranchTerminal::Request(cleanup),
                branch,
            });
        }
        assert_failed_terminal_missing_component_fail_closes_once(
            app,
            request,
            AuthorityIsolationComponent::RouterInputBarrier,
        );
    }

    #[test]
    fn failed_registration_begin_missing_pickable_fail_closes_once() {
        let mut app = App::new();
        let (request, _router) = prepare_registration_begin_failure(&mut app);
        assert!(matches!(
            &app.world().resource::<RouterNavigation>().entries[0]
                .pending_failed_branch
                .as_ref()
                .unwrap()
                .terminal,
            PendingFailedBranchTerminal::RegistrationBegin(_)
        ));
        assert_failed_terminal_missing_component_fail_closes_once(
            app,
            request,
            AuthorityIsolationComponent::Pickable,
        );
    }

    #[test]
    fn failed_authority_begin_rejection_restores_owner_and_fresh_trigger_can_publish() {
        let mut app = App::new();
        let (request, _router) = prepare_exact_child_stale_failure(&mut app);
        let (host_version, host_slot, host_entity, top_slot, top_slot_claim, failed_entities) = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            let host = entry.host.as_ref().unwrap();
            (
                host.version.clone(),
                host.slot_id,
                host.host,
                host.top_slot,
                host.top_slot_claim.clone(),
                failed_branch_primary_claims(&entry.pending_failed_branch.as_ref().unwrap().branch)
                    .into_iter()
                    .map(ExactRegistryClaim::entity)
                    .collect::<Vec<_>>(),
            )
        };
        let stale_seal = app
            .world_mut()
            .resource_mut::<RouteEntityRegistry>()
            .issue_seal();
        app.world_mut()
            .entity_mut(top_slot)
            .insert(RouteOwnershipSeal::new(stale_seal));
        let before = failed_entities
            .iter()
            .map(|entity| {
                (
                    *entity,
                    app.world().get::<Visibility>(*entity).copied(),
                    app.world()
                        .get::<Pickable>(*entity)
                        .map(|pickable| (pickable.should_block_lower, pickable.is_hoverable)),
                    app.world().get::<RouterInputBarrier>(*entity).copied(),
                    (
                        app.world()
                            .resource::<RouterInputGate>()
                            .allows_pointer(*entity),
                        app.world()
                            .resource::<RouterInputGate>()
                            .allows_keyboard(*entity),
                        app.world()
                            .resource::<RouterInputGate>()
                            .allows_gamepad(*entity),
                    ),
                    app.world()
                        .resource::<RouteEntityRegistry>()
                        .record(*entity)
                        .map(|record| record.claim()),
                )
            })
            .collect::<Vec<_>>();
        let focus_before = (
            app.world().resource::<InputFocus>().get(),
            app.world().resource::<RouteFocusState>().focused(),
        );
        let trace_len = app.world().resource::<RouterNavigation>().entries[0]
            .engine
            .request(request)
            .unwrap()
            .trace()
            .len();
        app.world_mut()
            .insert_resource(FailedAuthorityBeginManifestRejection { remaining: 1 });
        crate::cleanup::reset_ledger_domain_allocations_for_test();

        let first = app.world_mut().resource_scope(
            |world, mut navigation: bevy_ecs::change_detection::Mut<RouterNavigation>| {
                let entry = &mut navigation.entries[0];
                let failed = entry.pending_failed_branch.take().unwrap();
                let manifest = FrozenCleanupManifest::from_failed_prepared_branch(
                    world,
                    world.resource::<RouteEntityRegistry>(),
                    &failed.branch,
                )
                .unwrap();
                let FailedManifestDisposition::CurrentAuthorityLoss {
                    failed,
                    manifest,
                    bindings,
                } = classify_failed_manifest(
                    manifest,
                    failed,
                    entry.active.as_ref(),
                    entry.host.as_ref(),
                )
                else {
                    panic!("rejection fixture must classify current authority loss")
                };
                begin_failed_authority_settlement(world, entry, failed, manifest, bindings)
            },
        );
        let FailedAuthoritySettlementBeginOutcome::ProtocolContradiction { failed, kind } = first
        else {
            panic!("one-shot invalid roots must reject before publication")
        };
        assert_eq!(kind, BeginAuthoritySettlementErrorKind::InvalidManifest);
        {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            let host = entry
                .host
                .as_ref()
                .expect("rejection restores exact host owner");
            assert_eq!(host.version, host_version);
            assert_eq!(host.slot_id, host_slot);
            assert_eq!(host.host, host_entity);
            assert_eq!(host.top_slot, top_slot);
            assert_eq!(host.top_slot_claim, top_slot_claim);
            assert!(entry.active.is_none());
            assert!(entry.cleanup_ledger.is_none());
            assert_eq!(
                entry.engine.snapshot().availability(),
                &RouterAvailability::Ready
            );
            assert_eq!(
                entry.engine.request(request).unwrap().trace().len(),
                trace_len
            );
        }
        let after = failed_entities
            .iter()
            .map(|entity| {
                (
                    *entity,
                    app.world().get::<Visibility>(*entity).copied(),
                    app.world()
                        .get::<Pickable>(*entity)
                        .map(|pickable| (pickable.should_block_lower, pickable.is_hoverable)),
                    app.world().get::<RouterInputBarrier>(*entity).copied(),
                    (
                        app.world()
                            .resource::<RouterInputGate>()
                            .allows_pointer(*entity),
                        app.world()
                            .resource::<RouterInputGate>()
                            .allows_keyboard(*entity),
                        app.world()
                            .resource::<RouterInputGate>()
                            .allows_gamepad(*entity),
                    ),
                    app.world()
                        .resource::<RouteEntityRegistry>()
                        .record(*entity)
                        .map(|record| record.claim()),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(after, before);
        assert_eq!(
            crate::cleanup::ledger_domain_allocations_for_test(),
            1,
            "a rejected begin burns its already allocated ledger domain"
        );
        assert_eq!(
            (
                app.world().resource::<InputFocus>().get(),
                app.world().resource::<RouteFocusState>().focused(),
            ),
            focus_before
        );

        let second = app.world_mut().resource_scope(
            |world, mut navigation: bevy_ecs::change_detection::Mut<RouterNavigation>| {
                let entry = &mut navigation.entries[0];
                let manifest = FrozenCleanupManifest::from_failed_prepared_branch(
                    world,
                    world.resource::<RouteEntityRegistry>(),
                    &failed.branch,
                )
                .unwrap();
                let FailedManifestDisposition::CurrentAuthorityLoss {
                    failed,
                    manifest,
                    bindings,
                } = classify_failed_manifest(
                    manifest,
                    failed,
                    entry.active.as_ref(),
                    entry.host.as_ref(),
                )
                else {
                    panic!("restored owner must remain classifiable")
                };
                begin_failed_authority_settlement(world, entry, failed, manifest, bindings)
            },
        );
        assert!(matches!(
            second,
            FailedAuthoritySettlementBeginOutcome::Begun
        ));
        assert_eq!(
            crate::cleanup::ledger_domain_allocations_for_test(),
            2,
            "the fresh begin allocates a distinct domain instead of reusing the rejected one"
        );
        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert_eq!(
            entry.engine.snapshot().availability(),
            &RouterAvailability::RecoveringAuthority
        );
        assert!(
            entry
                .cleanup_ledger
                .as_ref()
                .is_some_and(CleanupLedger::is_authority_settlement)
        );
    }

    #[test]
    fn failed_authority_begin_contradiction_fail_closes_once_through_settle_caller() {
        let mut app = App::new();
        let (request, _router) = prepare_reconciliation_stale_failure_with_active(&mut app);
        let (current_boundary, active_targets, history_before) = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            let active = entry
                .active
                .as_ref()
                .expect("fixture retains current active UI");
            let failed = entry.pending_failed_branch.as_ref().unwrap();
            let current_boundary = failed.branch.frames()[0]
                .mount_binding()
                .physical_parent()
                .1
                .expect("entering reconciliation frame mounts below current active output")
                .clone();
            assert!(active.claims().any(|claim| claim == &current_boundary));
            assert!(
                !failed_branch_primary_claims(&failed.branch)
                    .into_iter()
                    .any(|claim| claim == &current_boundary)
            );
            (
                current_boundary,
                active.interactive_entities(),
                entry.engine.history().clone(),
            )
        };
        let focused = *active_targets
            .first()
            .expect("committed active branch has an interactive target");
        *app.world_mut().resource_mut::<InputFocus>() = InputFocus::from_entity(focused);
        app.world_mut()
            .resource_mut::<RouteFocusState>()
            .set_focused(Some(focused));
        let stale_seal = app
            .world_mut()
            .resource_mut::<RouteEntityRegistry>()
            .issue_seal();
        app.world_mut()
            .entity_mut(current_boundary.entity())
            .insert(RouteOwnershipSeal::new(stale_seal));
        app.world_mut()
            .insert_resource(FailedAuthorityBeginManifestRejection { remaining: 1 });

        settle_cleanup_system(app.world_mut());

        let trace_len = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            let residual = entry
                .authority_protocol_residual
                .as_ref()
                .expect("caller fail-closes the returned composed owner exactly once");
            assert!(matches!(
                residual.kind,
                AuthorityProtocolResidualKind::Authority(
                    BeginAuthoritySettlementErrorKind::InvalidManifest
                )
            ));
            assert!(residual.active.is_some());
            assert!(residual.failed.is_some());
            assert!(residual.host.is_some());
            assert!(entry.active.is_none());
            assert!(entry.pending_failed_branch.is_none());
            assert!(entry.host.is_none());
            assert!(entry.cleanup_ledger.is_none());
            assert_eq!(entry.engine.history(), &history_before);
            assert!(matches!(
                entry.engine.request(request).unwrap().status(),
                RequestStatus::Failed(failure)
                    if failure.kind() == NavigationFailureKind::AuthorityLost
            ));
            assert_eq!(app.world().resource::<InputFocus>().get(), None);
            assert_eq!(app.world().resource::<RouteFocusState>().focused(), None);
            let gate = app.world().resource::<RouterInputGate>();
            assert!(active_targets.iter().all(|entity| {
                !gate.allows_pointer(*entity)
                    && !gate.allows_keyboard(*entity)
                    && !gate.allows_gamepad(*entity)
            }));
            entry.engine.request(request).unwrap().trace().len()
        };

        settle_cleanup_system(app.world_mut());

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert_eq!(
            entry.engine.request(request).unwrap().trace().len(),
            trace_len
        );
        assert_eq!(entry.engine.history(), &history_before);
        assert!(entry.authority_protocol_residual.is_some());
        assert!(entry.cleanup_ledger.is_none());
        assert!(entry.pending_failed_branch.is_none());
    }

    #[test]
    fn failed_authority_trigger_exhaustion_fail_closes_once_with_full_owner() {
        let mut app = App::new();
        let (request, _router) = prepare_reconciliation_stale_failure_with_active(&mut app);
        let (current_boundary, history_before) = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            let active = entry
                .active
                .as_ref()
                .expect("fixture retains current active UI");
            let failed = entry.pending_failed_branch.as_ref().unwrap();
            let current_boundary = failed.branch.frames()[0]
                .mount_binding()
                .physical_parent()
                .1
                .expect("entering reconciliation frame mounts below current active output")
                .clone();
            assert!(active.claims().any(|claim| claim == &current_boundary));
            assert!(
                !failed_branch_primary_claims(&failed.branch)
                    .into_iter()
                    .any(|claim| claim == &current_boundary)
            );
            (current_boundary, entry.engine.history().clone())
        };
        let stale_seal = app
            .world_mut()
            .resource_mut::<RouteEntityRegistry>()
            .issue_seal();
        app.world_mut()
            .entity_mut(current_boundary.entity())
            .insert(RouteOwnershipSeal::new(stale_seal));
        app.world_mut()
            .insert_resource(FailedAuthorityTriggerExhaustion { remaining: 1 });

        settle_cleanup_system(app.world_mut());

        let trace_len = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            let residual = entry
                .authority_protocol_residual
                .as_ref()
                .expect("permanent trigger exhaustion must fail closed once");
            assert!(matches!(
                residual.kind,
                AuthorityProtocolResidualKind::AuthorityTrigger(
                    FailedAuthorityTriggerError::Exhausted
                )
            ));
            assert!(residual.active.is_some());
            assert!(residual.failed.is_some());
            assert!(residual.host.is_some());
            assert!(entry.active.is_none());
            assert!(entry.pending_failed_branch.is_none());
            assert!(entry.host.is_none());
            assert!(entry.cleanup_ledger.is_none());
            assert_eq!(entry.engine.history(), &history_before);
            assert!(matches!(
                entry.engine.request(request).unwrap().status(),
                RequestStatus::Failed(failure)
                    if failure.kind() == NavigationFailureKind::AuthorityLost
            ));
            entry.engine.request(request).unwrap().trace().len()
        };

        settle_cleanup_system(app.world_mut());

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert_eq!(
            entry.engine.request(request).unwrap().trace().len(),
            trace_len
        );
        assert_eq!(entry.engine.history(), &history_before);
        assert!(entry.authority_protocol_residual.is_some());
        assert!(entry.pending_failed_branch.is_none());
        assert!(entry.cleanup_ledger.is_none());
    }

    #[test]
    fn failed_authority_physical_task_target_selects_target_and_current_roots() {
        let mut app = App::new();
        let (request, _router) = prepare_exact_child_physical_residual_failure(&mut app);
        let (target, top_slot) = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            let failed = entry.pending_failed_branch.as_ref().unwrap();
            let PendingFailedBranchTerminal::TaskWithPhysicalResidual { task, .. } =
                &failed.terminal
            else {
                panic!("fixture must retain the physical-residual task terminal")
            };
            let manifest = FrozenCleanupManifest::from_failed_prepared_branch(
                app.world(),
                app.world().resource::<RouteEntityRegistry>(),
                &failed.branch,
            )
            .unwrap();
            assert!(manifest.contains_frame_obligation(task.frame()));
            let roots = failed_authority_logical_roots(failed, &manifest);
            assert_eq!(roots.first(), Some(task.frame()));
            (task.frame().clone(), entry.host.as_ref().unwrap().top_slot)
        };
        let stale_seal = app
            .world_mut()
            .resource_mut::<RouteEntityRegistry>()
            .issue_seal();
        app.world_mut()
            .entity_mut(top_slot)
            .insert(RouteOwnershipSeal::new(stale_seal));

        settle_cleanup_system(app.world_mut());

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert_eq!(
            entry.engine.snapshot().availability(),
            &RouterAvailability::RecoveringAuthority
        );
        assert!(matches!(
            entry.engine.request(request).unwrap().status(),
            RequestStatus::Running(NavigationPhase::Recovering)
        ));
        assert!(
            entry
                .cleanup_ledger
                .as_ref()
                .is_some_and(CleanupLedger::is_authority_settlement)
        );
        assert!(
            entry
                .cleanup_ledger
                .as_ref()
                .unwrap()
                .failed_prepared_frames()
                .iter()
                .any(|frame| frame.frame() == &target)
        );
    }

    #[test]
    fn failed_authority_cleanup_request_uses_its_exact_sealed_roots() {
        let mut app = App::new();
        let (request, _router) = prepare_exact_child_stale_failure(&mut app);
        let top_slot = {
            let mut navigation = app.world_mut().resource_mut::<RouterNavigation>();
            let entry = &mut navigation.entries[0];
            let failed = entry.pending_failed_branch.take().unwrap();
            let PendingFailedBranch {
                terminal:
                    PendingFailedBranchTerminal::Task {
                        task,
                        failure,
                        message,
                    },
                branch,
            } = failed
            else {
                panic!("cleanup-request fixture starts from an exact task terminal")
            };
            let cleanup = entry
                .engine
                .report_failed(task, failure, message)
                .expect("task terminal remains reportable")
                .expect("nested task failure retains an exact cleanup request");
            entry.pending_failed_branch = Some(PendingFailedBranch {
                terminal: PendingFailedBranchTerminal::Request(cleanup),
                branch,
            });
            entry.host.as_ref().unwrap().top_slot
        };
        {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            let failed = entry.pending_failed_branch.as_ref().unwrap();
            let PendingFailedBranchTerminal::Request(cleanup) = &failed.terminal else {
                panic!("fixture retains the cleanup request terminal")
            };
            let manifest = FrozenCleanupManifest::from_failed_prepared_branch(
                app.world(),
                app.world().resource::<RouteEntityRegistry>(),
                &failed.branch,
            )
            .unwrap();
            assert_eq!(
                failed_authority_logical_roots(failed, &manifest),
                cleanup.logical_roots()
            );
        }
        let stale_seal = app
            .world_mut()
            .resource_mut::<RouteEntityRegistry>()
            .issue_seal();
        app.world_mut()
            .entity_mut(top_slot)
            .insert(RouteOwnershipSeal::new(stale_seal));

        settle_cleanup_system(app.world_mut());

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert_eq!(
            entry.engine.snapshot().availability(),
            &RouterAvailability::RecoveringAuthority
        );
        assert!(matches!(
            entry.engine.request(request).unwrap().status(),
            RequestStatus::Running(NavigationPhase::Recovering)
        ));
        assert!(
            entry
                .cleanup_ledger
                .as_ref()
                .is_some_and(CleanupLedger::is_authority_settlement)
        );
    }

    #[test]
    fn failed_authority_registration_begin_uses_current_branch_roots() {
        let mut app = App::new();
        let (request, _router) = prepare_registration_begin_failure(&mut app);
        let top_slot = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            let failed = entry.pending_failed_branch.as_ref().unwrap();
            let PendingFailedBranchTerminal::RegistrationBegin(error) = &failed.terminal else {
                panic!("fixture retains the exact registration-begin terminal")
            };
            assert!(error.validate_rejection(&entry.engine).is_ok());
            let manifest = FrozenCleanupManifest::from_failed_prepared_branch(
                app.world(),
                app.world().resource::<RouteEntityRegistry>(),
                &failed.branch,
            )
            .unwrap();
            assert_eq!(
                failed_authority_logical_roots(failed, &manifest),
                failed_branch_logical_roots(&failed.branch)
            );
            entry.host.as_ref().unwrap().top_slot
        };
        let stale_seal = app
            .world_mut()
            .resource_mut::<RouteEntityRegistry>()
            .issue_seal();
        app.world_mut()
            .entity_mut(top_slot)
            .insert(RouteOwnershipSeal::new(stale_seal));

        settle_cleanup_system(app.world_mut());

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert_eq!(
            entry.engine.snapshot().availability(),
            &RouterAvailability::RecoveringAuthority
        );
        assert!(matches!(
            entry.engine.request(request).unwrap().status(),
            RequestStatus::Running(NavigationPhase::Recovering)
        ));
        assert!(entry.pending_failed_branch.is_none());
        assert!(
            entry
                .cleanup_ledger
                .as_ref()
                .is_some_and(CleanupLedger::is_authority_settlement)
        );
    }

    #[test]
    fn failed_authority_composed_owner_isolates_optional_active_branch() {
        let mut app = App::new();
        let (request, _router) = prepare_reconciliation_stale_failure_with_active(&mut app);
        let (
            current_boundary,
            active_targets,
            active_frame_count,
            failed_frame_count,
            retired_host,
            history_before,
        ) = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            let active = entry
                .active
                .as_ref()
                .expect("fixture retains committed active UI");
            let failed = entry.pending_failed_branch.as_ref().unwrap();
            let host = entry.host.as_ref().unwrap();
            let current_boundary = failed.branch.frames()[0]
                .mount_binding()
                .physical_parent()
                .1
                .expect("reconciliation entering frame mounts below an active output")
                .clone();
            assert!(active.claims().any(|claim| claim == &current_boundary));
            assert!(
                !failed_branch_primary_claims(&failed.branch)
                    .into_iter()
                    .any(|claim| claim == &current_boundary)
            );
            (
                current_boundary,
                active.interactive_entities(),
                active.frames().len(),
                failed.branch.frames().len(),
                (
                    host.version.clone(),
                    host.slot_id,
                    host.host,
                    host.top_slot,
                    host.top_slot_claim.clone(),
                ),
                entry.engine.history().clone(),
            )
        };
        let focused = *active_targets
            .first()
            .expect("committed active branch has an interactive target");
        *app.world_mut().resource_mut::<InputFocus>() = InputFocus::from_entity(focused);
        app.world_mut()
            .resource_mut::<RouteFocusState>()
            .set_focused(Some(focused));
        let stale_seal = app
            .world_mut()
            .resource_mut::<RouteEntityRegistry>()
            .issue_seal();
        app.world_mut()
            .entity_mut(current_boundary.entity())
            .insert(RouteOwnershipSeal::new(stale_seal));

        settle_cleanup_system(app.world_mut());

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert_eq!(
            entry.engine.snapshot().availability(),
            &RouterAvailability::RecoveringAuthority
        );
        assert!(matches!(
            entry.engine.request(request).unwrap().status(),
            RequestStatus::Running(NavigationPhase::Recovering)
        ));
        assert!(entry.active.is_none());
        assert!(entry.host.is_none());
        let ledger = entry.cleanup_ledger.as_ref().unwrap();
        assert!(ledger.is_authority_settlement());
        assert_eq!(ledger.leaving_frames().len(), active_frame_count);
        assert_eq!(ledger.failed_prepared_frames().len(), failed_frame_count);
        let retained_host = ledger
            .authority_settlement_host()
            .expect("the same authority ledger retains the retired host owner");
        assert_eq!(
            (
                &retained_host.version,
                retained_host.slot_id,
                retained_host.host,
                retained_host.top_slot,
                &retained_host.top_slot_claim,
            ),
            (
                &retired_host.0,
                retired_host.1,
                retired_host.2,
                retired_host.3,
                &retired_host.4,
            )
        );
        let batch = ledger.batch().unwrap();
        let stream = (batch.manifest_digest(), batch.obligation_count());
        let mut receipt_count = ledger.receipt_count();
        assert_eq!(app.world().resource::<InputFocus>().get(), None);
        assert_eq!(app.world().resource::<RouteFocusState>().focused(), None);
        let gate = app.world().resource::<RouterInputGate>();
        assert!(active_targets.iter().all(|entity| {
            !gate.allows_pointer(*entity)
                && !gate.allows_keyboard(*entity)
                && !gate.allows_gamepad(*entity)
        }));

        for _ in 0..8 {
            settle_cleanup_system(app.world_mut());
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            let Some(ledger) = entry.cleanup_ledger.as_ref() else {
                break;
            };
            assert!(ledger.is_authority_settlement());
            assert_eq!(ledger.leaving_frames().len(), active_frame_count);
            assert_eq!(ledger.failed_prepared_frames().len(), failed_frame_count);
            assert_eq!(
                ledger.authority_settlement_host().map(|host| (
                    &host.version,
                    host.slot_id,
                    host.host,
                    host.top_slot,
                    &host.top_slot_claim,
                )),
                Some((
                    &retired_host.0,
                    retired_host.1,
                    retired_host.2,
                    retired_host.3,
                    &retired_host.4,
                ))
            );
            let batch = ledger.batch().unwrap();
            assert_eq!((batch.manifest_digest(), batch.obligation_count()), stream);
            assert!(ledger.receipt_count() >= receipt_count);
            receipt_count = ledger.receipt_count();
        }

        let trace_len = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            assert!(entry.cleanup_ledger.is_none());
            assert!(entry.active.is_none());
            assert!(entry.host.is_none());
            assert!(entry.authority_protocol_residual.is_none());
            assert!(matches!(
                entry.engine.request(request).unwrap().status(),
                RequestStatus::Failed(failure)
                    if failure.kind() == NavigationFailureKind::AuthorityLost
            ));
            assert_eq!(entry.engine.history(), &history_before);
            let request_record = entry.engine.request(request).unwrap();
            let trace = request_record.trace();
            assert_eq!(
                trace
                    .iter()
                    .filter(|event| event.cleanup_disposition().is_some())
                    .count(),
                stream.1
            );
            assert_eq!(
                trace
                    .iter()
                    .filter(|event| {
                        event.kind() == NavigationTraceEventKind::CleanupSettled
                            && event.cleanup_disposition().is_none()
                    })
                    .count(),
                1
            );
            assert_eq!(
                trace
                    .iter()
                    .filter(|event| event.kind() == NavigationTraceEventKind::RequestTerminal)
                    .count(),
                1
            );
            trace.len()
        };

        settle_cleanup_system(app.world_mut());

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert_eq!(
            entry.engine.request(request).unwrap().trace().len(),
            trace_len
        );
        assert_eq!(entry.engine.history(), &history_before);
        assert!(entry.cleanup_ledger.is_none());
    }

    #[cfg(feature = "test-support")]
    #[test]
    fn failed_cleanup_ledger_audits_frames_as_prepared_inert() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let router = install_atomic_router(&mut app, "failed-cleanup-audit");
        observe_authority_system(app.world_mut());
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/first"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        let branch = {
            let mut navigation = app.world_mut().resource_mut::<RouterNavigation>();
            FailedPreparedBranch::new(navigation.entries[0].pending_commit.take().unwrap().branch)
        };
        let manifest = FrozenCleanupManifest::from_failed_prepared_branch(
            app.world(),
            app.world().resource::<RouteEntityRegistry>(),
            &branch,
        )
        .unwrap();
        let prepared = CleanupLedger::reserve(manifest).unwrap();
        app.world_mut().resource_mut::<RouterNavigation>().entries[0].cleanup_ledger =
            Some(CleanupLedger::activate_failed(prepared, branch));

        refresh_authority_audits(app.world_mut());

        let snapshot = app
            .world()
            .resource::<crate::audit::RouterAuthorityAudits>()
            .get(router)
            .unwrap();
        assert_eq!(snapshot.frames().len(), 1);
        assert!(snapshot.frames().iter().all(|frame| {
            frame.lifecycle() == crate::audit::AuthorityFrameLifecycle::PreparedInert
        }));
    }

    #[cfg(feature = "test-support")]
    #[test]
    fn tracked_child_slot_is_current_on_first_fresh_audit() {
        let (mut app, branch, _child_entities, child_slot_entity, _parent_entities) =
            failed_nested_branch_fixture();
        let router = app.world().resource::<RouterNavigation>().entries[0].handle;
        let manifest = FrozenCleanupManifest::from_failed_prepared_branch(
            app.world(),
            app.world().resource::<RouteEntityRegistry>(),
            &branch,
        )
        .unwrap();
        let expected_frames = branch
            .frames()
            .iter()
            .map(|frame| frame.frame().clone())
            .collect::<Vec<_>>();
        let frame_validity = branch
            .frames()
            .iter()
            .map(|frame| validate_realized_frame(app.world(), frame))
            .collect::<Vec<_>>();
        let prepared = CleanupLedger::reserve(manifest).unwrap();
        app.world_mut().resource_mut::<RouterNavigation>().entries[0].cleanup_ledger =
            Some(CleanupLedger::activate_failed(prepared, branch));
        app.world_mut()
            .insert_resource(crate::audit::RouterAuthorityAudits::new());

        refresh_authority_audits(app.world_mut());

        let snapshot = app
            .world()
            .resource::<crate::audit::RouterAuthorityAudits>()
            .get(router)
            .unwrap();
        assert_eq!(
            snapshot
                .frames()
                .iter()
                .map(|frame| frame.frame().clone())
                .collect::<Vec<_>>(),
            expected_frames,
            "standalone frame validity: {frame_validity:?}"
        );
        assert_eq!(snapshot.slots().len(), 2);
        assert_eq!(
            snapshot.slots()[1].kind(),
            crate::audit::AuthoritySlotKind::ChildOutlet
        );
        assert!(snapshot.orphan_slots().is_empty());
        let host_slot = snapshot
            .slots()
            .iter()
            .find(|slot| slot.kind() == crate::audit::AuthoritySlotKind::HostTop)
            .unwrap();
        let child_slot_mapping = snapshot
            .slots()
            .iter()
            .find(|slot| slot.kind() == crate::audit::AuthoritySlotKind::ChildOutlet)
            .unwrap();
        let parent_frame = snapshot
            .frames()
            .iter()
            .find(|frame| frame.frame() == &expected_frames[0])
            .unwrap();
        let child_frame = snapshot
            .frames()
            .iter()
            .find(|frame| frame.frame() == &expected_frames[1])
            .unwrap();
        assert_eq!(parent_frame.slot_entity(), host_slot.entity());
        assert_eq!(child_frame.slot_entity(), child_slot_mapping.entity());
        assert_ne!(child_frame.slot_entity(), host_slot.entity());
        assert_ne!(child_frame.root(), child_frame.slot_entity());
        let child_slot = child_slot_mapping.id();

        app.world_mut().entity_mut(child_slot_entity).insert(Node {
            width: Val::Percent(99.0),
            height: Val::Percent(100.0),
            ..Node::default()
        });
        app.world_mut().flush();
        refresh_authority_audits(app.world_mut());
        refresh_authority_audits(app.world_mut());
        let drifted = app
            .world()
            .resource::<crate::audit::RouterAuthorityAudits>()
            .get(router)
            .unwrap();
        assert_eq!(drifted.slots().len(), 1);
        assert!(drifted.orphan_slots().contains(&child_slot));

        app.world_mut().resource_mut::<RouterNavigation>().entries[0].cleanup_ledger = None;
        refresh_authority_audits(app.world_mut());
        refresh_authority_audits(app.world_mut());

        let registry_only = app
            .world()
            .resource::<crate::audit::RouterAuthorityAudits>()
            .get(router)
            .unwrap();
        assert_eq!(registry_only.slots().len(), 1);
        assert!(registry_only.orphan_slots().contains(&child_slot));
    }

    #[test]
    fn nested_branch_drives_parent_mount_and_child_before_commit() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let layout = RouteViewKey::new("layout").unwrap();
        let item = RouteViewKey::new("item").unwrap();
        let router = install_nested_router(&mut app, observed.clone(), true);

        observe_authority_system(app.world_mut());
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/items/1"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        assert_eq!(*observed.lock().unwrap(), vec![layout.clone()]);
        {
            let navigation = app.world().resource::<RouterNavigation>();
            let pending = navigation.entries[0].pending_commit.as_ref().unwrap();
            assert_eq!(pending.branch.frames().len(), 1);
            assert!(pending.branch.child_slots().is_empty());
            assert!(pending.branch.frames()[0].claims().all(|claim| {
                app.world().get::<Visibility>(claim.entity()) == Some(&Visibility::Hidden)
                    && app.world().get::<Pickable>(claim.entity()) == Some(&Pickable::IGNORE)
                    && app
                        .world()
                        .get::<RouterInputBarrier>(claim.entity())
                        .is_some_and(RouterInputBarrier::denies_all)
            }));
        }

        drive_navigation_system(app.world_mut());
        assert_eq!(*observed.lock().unwrap(), vec![layout.clone()]);
        {
            let navigation = app.world().resource::<RouterNavigation>();
            let pending = navigation.entries[0].pending_commit.as_ref().unwrap();
            assert_eq!(pending.branch.frames().len(), 1);
            assert_eq!(pending.branch.child_slots().len(), 1);
            let slot = pending.branch.child_slots()[0].slot();
            assert_eq!(
                app.world().get::<Visibility>(slot),
                Some(&Visibility::Hidden)
            );
            assert_eq!(app.world().get::<Pickable>(slot), Some(&Pickable::IGNORE));
            assert!(
                app.world()
                    .get::<RouterInputBarrier>(slot)
                    .is_some_and(RouterInputBarrier::denies_all)
            );
            assert_ne!(app.world().resource::<InputFocus>().get(), Some(slot));
        }

        drive_navigation_system(app.world_mut());
        assert_eq!(*observed.lock().unwrap(), vec![layout, item]);
        let navigation = app.world().resource::<RouterNavigation>();
        let entry = &navigation.entries[0];
        let pending = entry.pending_commit.as_ref().unwrap();
        assert_eq!(pending.branch.frames().len(), 2);
        assert_eq!(pending.branch.child_slots().len(), 1);
        assert!(pending.final_task.is_some());
        assert!(entry.active.is_none());
    }

    #[test]
    fn nested_commit_publishes_whole_branch_and_keeps_child_slot_input_inert() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let router = install_nested_router(&mut app, observed, true);
        app.update();
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/items/1"))
            .unwrap();

        app.update();
        {
            let navigation = app.world().resource::<RouterNavigation>();
            let pending = navigation.entries[0].pending_commit.as_ref().unwrap();
            assert_eq!(pending.branch.frames().len(), 1);
            assert!(pending.branch.child_slots().is_empty());
            assert!(pending.final_task.is_none());
        }
        app.update();
        {
            let navigation = app.world().resource::<RouterNavigation>();
            let pending = navigation.entries[0].pending_commit.as_ref().unwrap();
            assert_eq!(pending.branch.frames().len(), 1);
            assert_eq!(pending.branch.child_slots().len(), 1);
        }
        app.update();

        let navigation = app.world().resource::<RouterNavigation>();
        let entry = &navigation.entries[0];
        assert!(entry.pending_commit.is_none());
        let active = entry.active.as_ref().unwrap();
        assert_eq!(active.frames().len(), 2);
        assert_eq!(active.child_slots().len(), 1);
        assert_eq!(entry.engine.active().frames().len(), 2);
        assert!(active.frames().iter().all(|frame| {
            let interactive = frame.interactive_entities();
            frame.claims().all(|claim| {
                let pickable = app.world().get::<Pickable>(claim.entity());
                app.world().get::<Visibility>(claim.entity()) == Some(&Visibility::Visible)
                    && if interactive.contains(&claim.entity()) {
                        pickable.is_some_and(|pickable| {
                            pickable.should_block_lower && pickable.is_hoverable
                        })
                    } else {
                        pickable == Some(&Pickable::IGNORE)
                    }
            })
        }));
        let child_slot = active.child_slots()[0].slot();
        assert_eq!(
            app.world().get::<Visibility>(child_slot),
            Some(&Visibility::Visible)
        );
        assert_eq!(
            app.world().get::<Pickable>(child_slot),
            Some(&Pickable::IGNORE)
        );
        assert!(
            app.world()
                .get::<RouterInputBarrier>(child_slot)
                .is_some_and(RouterInputBarrier::denies_all)
        );
    }

    #[test]
    fn reconciliation_journey_b_schedule_probe_is_old_then_atomic_suffix_splice() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default())
            .init_resource::<NestedCommitScheduleProbe>()
            .add_systems(
                Update,
                observe_nested_commit_before
                    .after(RouterScheduleSet::DriveNavigation)
                    .before(RouterScheduleSet::CommitPresentation),
            )
            .add_systems(
                Update,
                observe_nested_commit_after
                    .after(RouterScheduleSet::CommitPresentation)
                    .before(RouterScheduleSet::SettleCleanup),
            );
        let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let router = install_nested_router(&mut app, observed, true);
        app.update();
        commit_initial_nested_branch(&mut app, router);
        let old_location = RouteLocation::new("/items/1");
        let (old_entities, old_interactive, old_frames, old_shells) = {
            let navigation = app.world().resource::<RouterNavigation>();
            let old = navigation.entries[0].active.as_ref().unwrap();
            (
                old.claims().map(|claim| claim.entity()).collect::<Vec<_>>(),
                old.interactive_entities(),
                old.frames()
                    .iter()
                    .map(|frame| frame.frame().clone())
                    .collect::<Vec<_>>(),
                old.frames()
                    .iter()
                    .map(RealizedFrame::shell)
                    .collect::<Vec<_>>(),
            )
        };
        let old_focus = *old_interactive.last().unwrap();
        *app.world_mut().resource_mut::<InputFocus>() = InputFocus::from_entity(old_focus);
        app.world_mut()
            .resource_mut::<RouteFocusState>()
            .set_focused(Some(old_focus));

        let new_location = RouteLocation::new("/items/2");
        let request = prepare_nested_branch_for_commit(&mut app, router, new_location.clone());
        let (new_entities, new_interactive, new_frames, new_shells) = {
            let navigation = app.world().resource::<RouterNavigation>();
            let new = &navigation.entries[0]
                .pending_commit
                .as_ref()
                .unwrap()
                .branch;
            (
                new.as_realized()
                    .claims()
                    .map(|claim| claim.entity())
                    .collect::<Vec<_>>(),
                new.as_realized().interactive_entities(),
                new.frames()
                    .iter()
                    .map(|frame| frame.frame().clone())
                    .collect::<Vec<_>>(),
                new.frames()
                    .iter()
                    .map(RealizedFrame::shell)
                    .collect::<Vec<_>>(),
            )
        };
        {
            let mut probe = app.world_mut().resource_mut::<NestedCommitScheduleProbe>();
            probe.armed = true;
            probe.old_entities = old_entities;
            probe.old_interactive = old_interactive;
            probe.old_frames = old_frames;
            probe.old_shells = old_shells.clone();
            probe.old_location = Some(old_location);
            probe.new_entities = new_entities;
            probe.new_interactive = new_interactive;
            probe.new_frames = new_frames;
            probe.new_shells = new_shells.clone();
            probe.new_location = Some(new_location.clone());
            probe.old_focus = Some(old_focus);
        }
        arm_test_commit_mutation_trace();

        app.update();

        let probe = app.world().resource::<NestedCommitScheduleProbe>();
        assert_eq!(
            probe.samples,
            vec![
                NestedCommitScheduleSample {
                    physical: NestedBranchSide::Old,
                    gate: NestedBranchSide::Old,
                    registry: NestedBranchSide::Old,
                    ecs: NestedBranchSide::Old,
                    core: NestedBranchSide::Old,
                    history: NestedBranchSide::Old,
                    input_focus: Some(old_focus),
                    route_focus: Some(old_focus),
                },
                NestedCommitScheduleSample {
                    physical: NestedBranchSide::Mixed,
                    gate: NestedBranchSide::New,
                    registry: NestedBranchSide::Mixed,
                    ecs: NestedBranchSide::Mixed,
                    core: NestedBranchSide::Mixed,
                    history: NestedBranchSide::New,
                    input_focus: None,
                    route_focus: None,
                },
            ]
        );
        assert_eq!(
            probe.order,
            vec![
                NestedCommitOrder::Leaving(old_shells[1]),
                NestedCommitOrder::Entering(new_shells[0]),
            ]
        );
        assert!(matches!(
            app.world().resource::<RouterNavigation>().entries[0]
                .engine
                .request(request)
                .unwrap()
                .status(),
            RequestStatus::Succeeded
        ));
        assert_eq!(
            app.world().resource::<RouterNavigation>().entries[0]
                .engine
                .history()
                .current(),
            Some(&new_location)
        );
    }

    #[test]
    fn reconciliation_journey_b_official_schedule_preserves_prefix_and_settles_once() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let router = install_nested_router(&mut app, observed, true);
        let host = app
            .world()
            .iter_entities()
            .find(|entity| {
                entity
                    .get::<RouterHostRegistration>()
                    .is_some_and(|registration| registration.router() == router)
            })
            .unwrap()
            .id();
        app.world_mut()
            .entity_mut(host)
            .insert(NestedAppHostSentinel(0xB));
        let sentinel = app.world_mut().spawn(NestedAppHostSentinel(0xB1)).id();
        app.world_mut().entity_mut(host).add_child(sentinel);

        app.update();
        let initial = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/items/1"))
            .unwrap();
        run_updates_until_succeeded(&mut app, initial);

        let (
            preserved_frame,
            preserved_shell,
            preserved_claims,
            preserved_relationships,
            preserved_outlet_anchor,
            preserved_slot,
            _preserved_slot_id,
            preserved_slot_claim,
            preserved_slot_parent,
            _preserved_slot_outlet,
            preserved_slot_anchor,
            preserved_slot_child_of,
            preserved_slot_parent_children,
            old_leaf_claims,
            old_leaf_focus,
            host_before,
            sentinel_before,
        ) = {
            let navigation = app.world().resource::<RouterNavigation>();
            let active = navigation.entries[0].active.as_ref().unwrap();
            let preserved = &active.frames()[0];
            let slot = &active.child_slots()[0];
            let slot_parent = app
                .world()
                .get::<ChildOf>(slot.slot())
                .expect("active child slot has an exact physical parent")
                .parent();
            let claims = preserved.claims().cloned().collect::<Vec<_>>();
            let relationships = claims
                .iter()
                .map(|claim| {
                    (
                        claim.entity(),
                        app.world()
                            .get::<ChildOf>(claim.entity())
                            .map(ChildOf::parent),
                    )
                })
                .collect::<Vec<_>>();
            (
                preserved.frame().clone(),
                preserved.shell(),
                claims,
                relationships,
                preserved.outlets()[0].anchor(),
                slot.slot(),
                slot.slot_id(),
                slot.slot_claim().clone(),
                slot.parent().clone(),
                slot.outlet().clone(),
                slot.anchor(),
                Some(slot_parent),
                app.world()
                    .get::<Children>(slot_parent)
                    .map(|children| children.iter().copied().collect::<Vec<_>>())
                    .unwrap_or_default(),
                active.frames()[1].claims().cloned().collect::<Vec<_>>(),
                active.frames()[1].interactive_entities()[0],
                nested_protected_fingerprint(app.world(), host),
                nested_protected_fingerprint(app.world(), sentinel),
            )
        };
        *app.world_mut().resource_mut::<InputFocus>() = InputFocus::from_entity(old_leaf_focus);
        app.world_mut()
            .resource_mut::<RouteFocusState>()
            .set_focused(Some(old_leaf_focus));

        let replacement = RouteLocation::new("/items/2");
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, replacement.clone())
            .unwrap();
        run_updates_until_succeeded(&mut app, request);

        #[cfg(feature = "test-support")]
        {
            observe_authority_system(app.world_mut());
            let audits = app.world().resource::<RouterAuthorityAudits>();
            let audit = audits.get(router).unwrap();
            assert!(audit.orphan_slots().is_empty());
            let slot = audit
                .slots()
                .iter()
                .find(|slot| slot.id() == _preserved_slot_id)
                .expect("preserved child slot remains in the private authority audit");
            assert_eq!(slot.parent_frame(), Some(&preserved_slot_parent));
            assert_eq!(slot.outlet(), Some(&_preserved_slot_outlet));
        }

        let (trace_after, history_after, active_claims_after, active_facts_after) = {
            let navigation = app.world().resource::<RouterNavigation>();
            let entry = &navigation.entries[0];
            let request_view = entry.engine.request(request).unwrap();
            assert!(matches!(request_view.status(), RequestStatus::Succeeded));
            let active = entry.active.as_ref().unwrap();
            assert_eq!(active.frames().len(), 2);
            assert_eq!(active.child_slots().len(), 1);
            assert_eq!(active.frames()[0].frame(), &preserved_frame);
            assert_eq!(active.frames()[0].shell(), preserved_shell);
            assert_eq!(
                active.frames()[0].outlets()[0].anchor(),
                preserved_outlet_anchor
            );
            assert_eq!(active.child_slots()[0].slot(), preserved_slot);
            assert_eq!(active.child_slots()[0].slot_claim(), &preserved_slot_claim);
            assert_eq!(active.child_slots()[0].parent(), &preserved_slot_parent);
            assert_eq!(active.child_slots()[0].anchor(), preserved_slot_anchor);
            assert_eq!(
                app.world()
                    .get::<ChildOf>(preserved_slot)
                    .map(ChildOf::parent),
                preserved_slot_child_of
            );
            assert_eq!(
                app.world()
                    .get::<Children>(preserved_slot_child_of.unwrap())
                    .map(|children| children.iter().copied().collect::<Vec<_>>())
                    .unwrap_or_default(),
                preserved_slot_parent_children
            );
            assert!(preserved_claims.iter().all(|claim| {
                app.world()
                    .resource::<RouteEntityRegistry>()
                    .record(claim.entity())
                    .is_some_and(|record| record.claim() == *claim)
            }));
            assert_eq!(
                preserved_relationships,
                preserved_claims
                    .iter()
                    .map(|claim| {
                        (
                            claim.entity(),
                            app.world()
                                .get::<ChildOf>(claim.entity())
                                .map(ChildOf::parent),
                        )
                    })
                    .collect::<Vec<_>>()
            );
            assert!(old_leaf_claims.iter().all(|claim| {
                app.world().get_entity(claim.entity()).is_err()
                    && app
                        .world()
                        .resource::<RouteEntityRegistry>()
                        .record(claim.entity())
                        .is_none()
            }));
            assert_eq!(
                request_view
                    .trace()
                    .iter()
                    .filter_map(|event| event.cleanup_disposition())
                    .collect::<Vec<_>>(),
                vec![
                    bevy_ui_router_core::prelude::CleanupDisposition::Removed;
                    old_leaf_claims.len()
                ]
            );
            assert_eq!(entry.engine.history().len(), 2);
            assert_eq!(entry.engine.history().current(), Some(&replacement));
            assert!(entry.cleanup_ledger.is_none());
            assert_eq!(app.world().resource::<InputFocus>().get(), None);
            assert_eq!(app.world().resource::<RouteFocusState>().focused(), None);
            assert_eq!(nested_protected_fingerprint(app.world(), host), host_before);
            assert_eq!(
                nested_protected_fingerprint(app.world(), sentinel),
                sentinel_before
            );
            let active_claims = active.claims().cloned().collect::<Vec<_>>();
            let mut related = active_claims
                .iter()
                .map(ExactRegistryClaim::entity)
                .collect::<Vec<_>>();
            if !related.contains(&preserved_slot) {
                related.push(preserved_slot);
            }
            let active_facts = related
                .into_iter()
                .map(|entity| {
                    (
                        entity,
                        app.world()
                            .resource::<RouteEntityRegistry>()
                            .record(entity)
                            .map(|record| record.claim()),
                        nested_protected_fingerprint(app.world(), entity),
                    )
                })
                .collect::<Vec<_>>();
            (
                request_view.trace().to_vec(),
                entry.engine.history().clone(),
                active_claims,
                active_facts,
            )
        };

        app.update();

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert_eq!(entry.engine.request(request).unwrap().trace(), trace_after);
        assert_eq!(entry.engine.history(), &history_after);
        assert_eq!(
            entry
                .active
                .as_ref()
                .unwrap()
                .claims()
                .cloned()
                .collect::<Vec<_>>(),
            active_claims_after
        );
        assert_eq!(
            active_facts_after,
            active_facts_after
                .iter()
                .map(|(entity, _, _)| {
                    (
                        *entity,
                        app.world()
                            .resource::<RouteEntityRegistry>()
                            .record(*entity)
                            .map(|record| record.claim()),
                        nested_protected_fingerprint(app.world(), *entity),
                    )
                })
                .collect::<Vec<_>>()
        );
        assert!(entry.cleanup_ledger.is_none());
        assert!(old_leaf_claims.iter().all(|claim| {
            app.world().get_entity(claim.entity()).is_err()
                && app
                    .world()
                    .resource::<RouteEntityRegistry>()
                    .record(claim.entity())
                    .is_none()
        }));
        assert_eq!(nested_protected_fingerprint(app.world(), host), host_before);
        assert_eq!(
            nested_protected_fingerprint(app.world(), sentinel),
            sentinel_before
        );
    }

    #[test]
    fn reconciliation_scheduled_mount_new_retain_empty_and_activate_existing_reuse_exact_slot() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default())
            .init_resource::<MountNewRoleScheduleProbe>()
            .add_systems(
                Update,
                observe_mount_new_role_before_commit
                    .after(RouterScheduleSet::DriveNavigation)
                    .before(RouterScheduleSet::CommitPresentation),
            );
        let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let router = install_nested_router(&mut app, observed, true);
        app.update();

        let prefix = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        run_updates_until_succeeded(&mut app, prefix);
        {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            let active = entry.active.as_ref().unwrap();
            assert_eq!(active.frames().len(), 1);
            assert!(active.child_slots().is_empty());
        }
        app.world_mut()
            .resource_mut::<MountNewRoleScheduleProbe>()
            .armed = true;

        let mounted = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/items/1"))
            .unwrap();
        run_updates_until_succeeded(&mut app, mounted);
        assert_eq!(
            app.world().resource::<MountNewRoleScheduleProbe>().roles,
            Some(vec![
                bevy_ui_router_core::integration::ReconciliationChildSlotRoleKind::MountNew
            ])
        );
        let (
            slot,
            mounted_claim,
            logical_parent,
            anchor,
            physical_parent,
            parent_children,
            leaf_focus,
        ) = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            let active = entry.active.as_ref().unwrap();
            let child_slot = &active.child_slots()[0];
            let mounted_claim = child_slot.slot_claim().clone();
            let logical_parent = child_slot.parent().clone();
            let anchor = child_slot.anchor();
            let physical_parent = app
                .world()
                .get::<ChildOf>(child_slot.slot())
                .unwrap()
                .parent();
            let parent_children = app
                .world()
                .get::<Children>(physical_parent)
                .map(|children| children.iter().copied().collect::<Vec<_>>())
                .unwrap_or_default();
            let mounted_claim = assert_reconciliation_child_slot_state(
                app.world(),
                child_slot,
                &mounted_claim,
                &logical_parent,
                anchor,
                physical_parent,
                &parent_children,
                EntityLifecycle::Active,
                ChildSlotLifecycle::Active,
                Visibility::Visible,
            );
            (
                child_slot.slot(),
                mounted_claim,
                logical_parent,
                anchor,
                physical_parent,
                parent_children,
                active.frames()[1].interactive_entities()[0],
            )
        };
        *app.world_mut().resource_mut::<InputFocus>() = InputFocus::from_entity(leaf_focus);
        app.world_mut()
            .resource_mut::<RouteFocusState>()
            .set_focused(Some(leaf_focus));

        let truncated = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        run_updates_until_succeeded(&mut app, truncated);
        let retained_claim = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            let active = entry.active.as_ref().unwrap();
            let child_slot = &active.child_slots()[0];
            let claim = assert_reconciliation_child_slot_state(
                app.world(),
                child_slot,
                &mounted_claim,
                &logical_parent,
                anchor,
                physical_parent,
                &parent_children,
                EntityLifecycle::PreparedInert,
                ChildSlotLifecycle::RegisteredInert,
                Visibility::Hidden,
            );
            assert_eq!(app.world().resource::<InputFocus>().get(), None);
            assert_eq!(app.world().resource::<RouteFocusState>().focused(), None);
            assert!(entry.cleanup_ledger.is_none());
            claim
        };

        let activated = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/items/2"))
            .unwrap();
        run_updates_until_succeeded(&mut app, activated);

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        let active = entry.active.as_ref().unwrap();
        let child_slot = &active.child_slots()[0];
        assert_reconciliation_child_slot_state(
            app.world(),
            child_slot,
            &retained_claim,
            &logical_parent,
            anchor,
            physical_parent,
            &parent_children,
            EntityLifecycle::Active,
            ChildSlotLifecycle::Active,
            Visibility::Visible,
        );
        let child_shell = active.frames()[1].shell();
        assert_eq!(
            app.world().get::<ChildOf>(child_shell).map(ChildOf::parent),
            Some(slot)
        );
        assert_eq!(
            app.world().get::<Children>(slot).map(|children| children
                .iter()
                .filter(|entity| **entity == child_shell)
                .count()),
            Some(1)
        );
        assert_eq!(app.world().resource::<InputFocus>().get(), None);
        assert_eq!(app.world().resource::<RouteFocusState>().focused(), None);
        assert!(entry.cleanup_ledger.is_none());
    }

    #[test]
    fn nested_leaving_cleanup_observes_receipts_before_core_completion() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        install_nested_leaving_cleanup_trace(&mut app);
        let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let router = install_nested_router(&mut app, observed, true);
        let host = app
            .world()
            .iter_entities()
            .find(|entity| entity.contains::<RouterHostRegistration>())
            .unwrap()
            .id();
        app.world_mut()
            .entity_mut(host)
            .insert(NestedAppHostSentinel(41));
        let sentinel = app.world_mut().spawn(NestedAppHostSentinel(42)).id();
        app.world_mut().entity_mut(host).add_child(sentinel);
        app.update();
        commit_initial_nested_branch(&mut app, router);

        let (expected_receipts, old_frames, child_slot, units) = {
            let navigation = app.world().resource::<RouterNavigation>();
            let old = navigation.entries[0].active.as_ref().unwrap();
            let child_entities = old.frames()[1]
                .claims()
                .map(|claim| claim.entity())
                .collect::<Vec<_>>();
            let parent_entities = old.frames()[0]
                .claims()
                .map(|claim| claim.entity())
                .collect::<Vec<_>>();
            let child_slot = old.child_slots()[0].slot();
            let mut units = std::collections::BTreeMap::new();
            units.extend(
                child_entities
                    .iter()
                    .copied()
                    .map(|entity| (entity, NestedCleanupUnit::ChildFrame)),
            );
            units.insert(child_slot, NestedCleanupUnit::ChildSlot);
            units.extend(
                parent_entities
                    .iter()
                    .copied()
                    .map(|entity| (entity, NestedCleanupUnit::ParentFrame)),
            );
            (
                child_entities.len(),
                vec![old.frames()[1].frame().clone()],
                child_slot,
                units,
            )
        };
        app.world_mut()
            .resource_mut::<NestedLeavingCleanupTrace>()
            .units = units;
        let host_manifest_before = ComponentManifest::from_world(app.world(), host).unwrap();
        let host_parent_before = app.world().get::<ChildOf>(host).map(ChildOf::parent);
        let host_children_before = app
            .world()
            .get::<Children>(host)
            .map(|children| children.iter().copied().collect::<Vec<_>>())
            .unwrap_or_default();
        let sentinel_manifest_before =
            ComponentManifest::from_world(app.world(), sentinel).unwrap();
        let replacement = RouteLocation::new("/items/2");
        let request = prepare_nested_branch_for_commit(&mut app, router, replacement.clone());

        commit_presentation_system(app.world_mut());

        {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            let ledger = entry.cleanup_ledger.as_ref().unwrap();
            assert_eq!(ledger.leaving_frames().len(), 1);
            assert_eq!(
                ledger
                    .leaving_frames()
                    .iter()
                    .map(|frame| frame.frame().clone())
                    .collect::<Vec<_>>(),
                old_frames
            );
            assert_eq!(ledger.receipt_count(), 0);
            assert!(matches!(
                entry.engine.request(request).unwrap().status(),
                RequestStatus::Running(NavigationPhase::PostCommitCleanup)
            ));
        }

        let mut ledger = app.world_mut().resource_mut::<RouterNavigation>().entries[0]
            .cleanup_ledger
            .take()
            .unwrap();
        let attempt = ledger.attempt(app.world()).unwrap();
        assert!(ledger.settle(app.world_mut(), attempt));
        assert!(ledger.is_settled());
        assert_eq!(ledger.receipt_count(), expected_receipts);
        let cleanup_units = app
            .world()
            .resource::<NestedLeavingCleanupTrace>()
            .despawned
            .iter()
            .copied()
            .fold(Vec::new(), |mut units, unit| {
                if units.last() != Some(&unit) {
                    units.push(unit);
                }
                units
            });
        assert_eq!(cleanup_units, vec![NestedCleanupUnit::ChildFrame]);
        assert!(app.world().get_entity(child_slot).is_ok());
        assert!(
            app.world()
                .resource::<RouteEntityRegistry>()
                .record(child_slot)
                .is_some_and(|record| record.lifecycle() == EntityLifecycle::Active)
        );

        {
            let mut navigation = app.world_mut().resource_mut::<RouterNavigation>();
            let entry = &mut navigation.entries[0];
            assert!(entry.observe_cleanup_receipts(&mut ledger));
            let request_view = entry.engine.request(request).unwrap();
            assert!(matches!(
                request_view.status(),
                RequestStatus::Running(NavigationPhase::PostCommitCleanup)
            ));
            assert_eq!(
                request_view
                    .trace()
                    .iter()
                    .filter(|event| event.cleanup_disposition().is_some())
                    .count(),
                expected_receipts
            );
            assert!(
                request_view
                    .trace()
                    .iter()
                    .all(|event| { event.kind() != NavigationTraceEventKind::RequestTerminal })
            );
            entry.cleanup_ledger = Some(ledger);
        }
        assert_eq!(
            app.world().get::<NestedAppHostSentinel>(host),
            Some(&NestedAppHostSentinel(41))
        );
        assert_eq!(
            ComponentManifest::from_world(app.world(), host),
            Some(host_manifest_before.clone())
        );
        assert_eq!(
            app.world().get::<ChildOf>(host).map(ChildOf::parent),
            host_parent_before
        );
        assert_eq!(
            app.world()
                .get::<Children>(host)
                .map(|children| children.iter().copied().collect::<Vec<_>>())
                .unwrap_or_default(),
            host_children_before
        );
        assert_eq!(
            ComponentManifest::from_world(app.world(), sentinel),
            Some(sentinel_manifest_before.clone())
        );
        assert_eq!(
            app.world().get::<ChildOf>(sentinel).map(ChildOf::parent),
            Some(host)
        );

        settle_cleanup_system(app.world_mut());

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        let request_view = entry.engine.request(request).unwrap();
        assert!(matches!(request_view.status(), RequestStatus::Succeeded));
        assert_eq!(entry.engine.history().current(), Some(&replacement));
        assert!(entry.cleanup_ledger.is_none());
        assert_eq!(
            request_view
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
        assert_eq!(
            app.world().get::<NestedAppHostSentinel>(host),
            Some(&NestedAppHostSentinel(41))
        );
        assert_eq!(
            ComponentManifest::from_world(app.world(), host),
            Some(host_manifest_before)
        );
        assert_eq!(
            app.world()
                .get::<Children>(host)
                .map(|children| children.iter().copied().collect::<Vec<_>>())
                .unwrap_or_default(),
            host_children_before
        );
        assert_eq!(
            ComponentManifest::from_world(app.world(), sentinel),
            Some(sentinel_manifest_before)
        );
        assert_eq!(
            app.world().get::<ChildOf>(sentinel).map(ChildOf::parent),
            Some(host)
        );
    }

    #[test]
    fn nested_leaving_unit_drift_is_quarantined_without_crossing_authority() {
        for drift_unit in [NestedCleanupUnit::ChildFrame] {
            let mut app = App::new();
            app.add_plugins(BevyUiRouterPlugin::default());
            let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
            let router = install_nested_router(&mut app, observed, true);
            let host = app
                .world()
                .iter_entities()
                .find(|entity| entity.contains::<RouterHostRegistration>())
                .unwrap()
                .id();
            app.world_mut()
                .entity_mut(host)
                .insert(NestedAppHostSentinel(71));
            let sentinel = app.world_mut().spawn(NestedAppHostSentinel(72)).id();
            app.world_mut().entity_mut(host).add_child(sentinel);
            let sibling = install_atomic_router(&mut app, "nested-cleanup-sibling");
            app.update();
            commit_initial_nested_branch(&mut app, router);
            app.world_mut()
                .resource_mut::<RouterNavigation>()
                .push(sibling, RouteLocation::new("/"))
                .unwrap();
            collect_navigation_system(app.world_mut());
            drive_navigation_system(app.world_mut());
            commit_presentation_system(app.world_mut());
            settle_cleanup_system(app.world_mut());

            let target = {
                let navigation = app.world().resource::<RouterNavigation>();
                let old = navigation
                    .entries
                    .iter()
                    .find(|entry| entry.handle == router)
                    .unwrap()
                    .active
                    .as_ref()
                    .unwrap();
                match drift_unit {
                    NestedCleanupUnit::ChildFrame => old.frames()[1].interactive_entities()[0],
                    NestedCleanupUnit::ChildSlot => old.child_slots()[0].slot(),
                    NestedCleanupUnit::ParentFrame => old.frames()[0].shell(),
                }
            };
            let replacement = RouteLocation::new("/items/2");
            let request = prepare_nested_branch_for_commit(&mut app, router, replacement.clone());
            commit_presentation_system(app.world_mut());
            assert!(
                app.world()
                    .resource::<RouteEntityRegistry>()
                    .record(target)
                    .is_some_and(|record| record.lifecycle() == EntityLifecycle::Leaving)
            );
            app.world_mut()
                .entity_mut(target)
                .insert(CleanupDriftMarker);
            let foreign = app
                .world_mut()
                .spawn((
                    NestedAppHostSentinel(73),
                    Visibility::Visible,
                    Pickable {
                        should_block_lower: true,
                        is_hoverable: true,
                    },
                    RouterInputBarrier::allow_all(),
                ))
                .id();
            let host_before = nested_protected_fingerprint(app.world(), host);
            let sentinel_before = nested_protected_fingerprint(app.world(), sentinel);
            let foreign_before = nested_protected_fingerprint(app.world(), foreign);
            let (sibling_before, sibling_core_before, sibling_history_before) = {
                let navigation = app.world().resource::<RouterNavigation>();
                let entry = navigation
                    .entries
                    .iter()
                    .find(|entry| entry.handle == sibling)
                    .unwrap();
                let active = entry.active.as_ref().unwrap();
                (
                    active
                        .claims()
                        .map(|claim| {
                            (
                                claim.clone(),
                                nested_protected_fingerprint(app.world(), claim.entity()),
                            )
                        })
                        .collect::<Vec<_>>(),
                    entry.engine.active().frames().to_vec(),
                    entry.engine.history().clone(),
                )
            };

            settle_cleanup_system(app.world_mut());

            let entry = app
                .world()
                .resource::<RouterNavigation>()
                .entries
                .iter()
                .find(|entry| entry.handle == router)
                .unwrap();
            assert!(matches!(
                entry.engine.request(request).unwrap().status(),
                RequestStatus::Succeeded
            ));
            assert_eq!(entry.engine.history().current(), Some(&replacement));
            assert!(entry.cleanup_ledger.is_none());
            assert!(app.world().get_entity(target).is_ok());
            assert!(app.world().get::<CleanupDriftMarker>(target).is_some());
            assert_eq!(
                app.world().get::<Visibility>(target),
                Some(&Visibility::Hidden)
            );
            assert_eq!(app.world().get::<Pickable>(target), Some(&Pickable::IGNORE));
            assert!(
                app.world()
                    .get::<RouterInputBarrier>(target)
                    .is_some_and(RouterInputBarrier::denies_all)
            );
            assert!(
                app.world()
                    .resource::<QuarantineLeaseTable>()
                    .record(target)
                    .is_some_and(|record| record.lifecycle() == EntityLifecycle::Quarantined)
            );
            assert!(
                !app.world()
                    .resource::<RouterInputGate>()
                    .allows_pointer(target)
            );

            assert_eq!(nested_protected_fingerprint(app.world(), host), host_before);
            assert_eq!(
                nested_protected_fingerprint(app.world(), sentinel),
                sentinel_before
            );
            assert_eq!(
                nested_protected_fingerprint(app.world(), foreign),
                foreign_before
            );
            let navigation = app.world().resource::<RouterNavigation>();
            let sibling_entry = navigation
                .entries
                .iter()
                .find(|entry| entry.handle == sibling)
                .unwrap();
            assert_eq!(sibling_entry.engine.active().frames(), sibling_core_before);
            assert_eq!(sibling_entry.engine.history(), &sibling_history_before);
            assert!(sibling_before.iter().all(|(claim, fingerprint)| {
                app.world()
                    .resource::<RouteEntityRegistry>()
                    .record(claim.entity())
                    .is_some_and(|record| record.claim() == *claim)
                    && nested_protected_fingerprint(app.world(), claim.entity()) == *fingerprint
            }));
        }
    }

    #[test]
    fn reconciliation_suffix_replace_preserves_prefix_and_releases_child_focus() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let router = install_nested_router(&mut app, observed, true);
        app.update();
        commit_initial_nested_branch(&mut app, router);
        let (
            old_claims,
            preserved_claims,
            leaving_claims,
            preserved_slot_claim,
            leaving_frame,
            old_slots,
            preserved_interactive,
            leaving_interactive,
        ) = {
            let navigation = app.world().resource::<RouterNavigation>();
            let old = navigation.entries[0].active.as_ref().unwrap();
            assert_eq!(
                logical_cleanup_roots(old),
                old.frames()
                    .iter()
                    .rev()
                    .map(|frame| frame.frame().clone())
                    .collect::<Vec<_>>()
            );
            (
                old.claims().cloned().collect::<Vec<_>>(),
                old.frames()[0].claims().cloned().collect::<Vec<_>>(),
                old.frames()[1].claims().cloned().collect::<Vec<_>>(),
                old.child_slots()[0].slot_claim().clone(),
                old.frames()[1].frame().clone(),
                old.child_slots()
                    .iter()
                    .map(|slot| slot.slot())
                    .collect::<Vec<_>>(),
                old.frames()[0].interactive_entities(),
                old.frames()[1].interactive_entities(),
            )
        };
        let old_focus = leaving_interactive[0];
        *app.world_mut().resource_mut::<InputFocus>() = InputFocus::from_entity(old_focus);
        app.world_mut()
            .resource_mut::<RouteFocusState>()
            .set_focused(Some(old_focus));

        let replacement = RouteLocation::new("/items/2");
        prepare_nested_branch_for_commit(&mut app, router, replacement.clone());
        let (new_claims, new_slots, new_interactive) = {
            let navigation = app.world().resource::<RouterNavigation>();
            let pending = &navigation.entries[0]
                .pending_commit
                .as_ref()
                .unwrap()
                .branch;
            (
                pending.as_realized().claims().cloned().collect::<Vec<_>>(),
                pending
                    .child_slots()
                    .iter()
                    .map(|slot| slot.slot())
                    .collect::<Vec<_>>(),
                pending
                    .frames()
                    .iter()
                    .flat_map(RealizedFrame::interactive_entities)
                    .collect::<Vec<_>>(),
            )
        };
        assert!(
            old_claims
                .iter()
                .all(|old| { new_claims.iter().all(|new| old.entity() != new.entity()) })
        );
        assert_ne!(old_slots, new_slots);
        assert_eq!(
            app.world().resource::<RouterNavigation>().entries[0]
                .engine
                .history()
                .len(),
            1
        );

        commit_presentation_system(app.world_mut());

        let navigation = app.world().resource::<RouterNavigation>();
        let entry = &navigation.entries[0];
        assert!(
            entry.pending_commit.is_none(),
            "replacement retained pending: ticket={}, final_task={}",
            entry
                .pending_commit
                .as_ref()
                .is_some_and(|pending| pending.ticket.is_some()),
            entry
                .pending_commit
                .as_ref()
                .is_some_and(|pending| pending.final_task.is_some()),
        );
        let active = entry.active.as_ref().unwrap();
        let expected_active = preserved_claims
            .iter()
            .chain(&new_claims)
            .map(|claim| claim.entity())
            .chain(std::iter::once(preserved_slot_claim.entity()))
            .collect::<Vec<_>>();
        assert_eq!(
            active
                .claims()
                .map(|claim| claim.entity())
                .collect::<Vec<_>>(),
            expected_active
        );
        assert_eq!(entry.engine.history().current(), Some(&replacement));
        let ledger = entry.cleanup_ledger.as_ref().unwrap();
        assert_eq!(ledger.leaving_frames().len(), 1);
        assert_eq!(ledger.leaving_frames()[0].frame(), &leaving_frame);
        assert_eq!(
            ledger.batch().unwrap().obligation_count(),
            leaving_claims.len()
        );
        assert!(preserved_claims.iter().all(|claim| {
            app.world()
                .resource::<RouteEntityRegistry>()
                .record(claim.entity())
                .is_some_and(|record| record.lifecycle() == EntityLifecycle::Active)
                && app.world().get::<Visibility>(claim.entity()) == Some(&Visibility::Visible)
        }));
        assert!(leaving_claims.iter().all(|claim| {
            app.world()
                .resource::<RouteEntityRegistry>()
                .record(claim.entity())
                .is_some_and(|record| record.lifecycle() == EntityLifecycle::Leaving)
                && app.world().get::<Visibility>(claim.entity()) == Some(&Visibility::Hidden)
                && app.world().get::<Pickable>(claim.entity()) == Some(&Pickable::IGNORE)
                && app
                    .world()
                    .get::<RouterInputBarrier>(claim.entity())
                    .is_some_and(RouterInputBarrier::denies_all)
        }));
        assert!(old_slots.iter().all(|slot| {
            app.world().get::<ChildSlotLifecycle>(*slot) == Some(&ChildSlotLifecycle::Active)
                && app.world().get::<Visibility>(*slot) == Some(&Visibility::Visible)
                && app.world().get::<Pickable>(*slot) == Some(&Pickable::IGNORE)
        }));
        let gate = app.world().resource::<RouterInputGate>();
        assert!(
            preserved_interactive
                .iter()
                .all(|entity| gate.allows_pointer(*entity))
        );
        assert!(
            leaving_interactive
                .iter()
                .all(|entity| !gate.allows_pointer(*entity))
        );
        assert!(
            new_interactive
                .iter()
                .all(|entity| gate.allows_pointer(*entity))
        );
        assert_eq!(app.world().resource::<InputFocus>().get(), None);
        assert_eq!(app.world().resource::<RouteFocusState>().focused(), None);
    }

    #[test]
    fn reconciliation_fragment_splices_once_with_same_frame_and_next_generation() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let router = install_nested_router(&mut app, observed.clone(), true);
        app.update();
        commit_initial_nested_branch(&mut app, router);
        let (
            preserved_claims,
            leaving_claims,
            old_frames,
            leaving_frame,
            preserved_slot_claim,
            history_len_before,
            effect_count_before,
        ) = {
            let navigation = app.world().resource::<RouterNavigation>();
            let entry = &navigation.entries[0];
            let old = entry.active.as_ref().unwrap();
            (
                old.frames()[0].claims().cloned().collect::<Vec<_>>(),
                old.frames()[1].claims().cloned().collect::<Vec<_>>(),
                old.frames()
                    .iter()
                    .map(RealizedFrame::shell)
                    .collect::<Vec<_>>(),
                old.frames()[1].frame().clone(),
                old.child_slots()[0].slot_claim().clone(),
                entry.engine.history().len(),
                observed.lock().unwrap().len(),
            )
        };
        let replacement = RouteLocation::new("/items/1").with_fragment("details");

        let request = prepare_nested_branch_for_commit(&mut app, router, replacement.clone());
        let (entering_claims, sealed_entering, pending_entering) = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            let pending = entry.pending_commit.as_ref().unwrap();
            let projection = match (pending.ticket.as_ref(), pending.final_task.as_ref()) {
                (Some(ticket), _) => entry
                    .engine
                    .reconciliation_projection_for_commit(ticket)
                    .unwrap(),
                (None, Some(task)) => entry
                    .engine
                    .reconciliation_projection_for_task(task)
                    .unwrap(),
                (None, None) => panic!("fragment request retains a sealed projection"),
            };
            assert_eq!(projection.entering().len(), 1);
            let entering = projection.entering().next().unwrap();
            let sealed = (entering.frame(), entering.generation(), entering.slot());
            let pending_identity = pending.branch.frames()[0].frame().clone();
            assert_eq!(pending_identity.frame(), sealed.0);
            assert_eq!(pending_identity.generation(), sealed.1);
            assert_eq!(
                sealed.2,
                bevy_ui_router_core::integration::ReconciliationEnteringSlotKind::Existing(
                    pending_identity.slot()
                )
            );
            (
                pending
                    .branch
                    .as_realized()
                    .claims()
                    .cloned()
                    .collect::<Vec<_>>(),
                sealed,
                pending_identity,
            )
        };
        commit_presentation_system(app.world_mut());

        {
            let navigation = app.world().resource::<RouterNavigation>();
            let entry = &navigation.entries[0];
            let active = entry.active.as_ref().unwrap();
            assert_eq!(active.frames()[0].shell(), old_frames[0]);
            assert_ne!(active.frames()[1].shell(), old_frames[1]);
            assert_eq!(active.frames()[1].frame(), &pending_entering);
            assert_eq!(active.frames()[1].frame().frame(), sealed_entering.0);
            assert_eq!(active.frames()[1].frame().generation(), sealed_entering.1);
            assert_eq!(
                sealed_entering.2,
                bevy_ui_router_core::integration::ReconciliationEnteringSlotKind::Existing(
                    active.frames()[1].frame().slot()
                )
            );
            assert_eq!(active.frames()[1].frame().frame(), leaving_frame.frame());
            assert_eq!(
                active.frames()[1].frame().generation().raw(),
                leaving_frame.generation().raw() + 1
            );
            assert_eq!(
                active.child_slots()[0].slot(),
                preserved_slot_claim.entity()
            );
            let expected_active = preserved_claims
                .iter()
                .chain(&entering_claims)
                .map(|claim| claim.entity())
                .chain(std::iter::once(preserved_slot_claim.entity()))
                .collect::<Vec<_>>();
            assert_eq!(
                active
                    .claims()
                    .map(|claim| claim.entity())
                    .collect::<Vec<_>>(),
                expected_active
            );
            let ledger = entry.cleanup_ledger.as_ref().unwrap();
            assert_eq!(ledger.leaving_frames().len(), 1);
            assert_eq!(ledger.leaving_frames()[0].frame(), &leaving_frame);
            assert_eq!(ledger.receipt_count(), 0);
            assert!(preserved_claims.iter().all(|claim| {
                app.world()
                    .resource::<RouteEntityRegistry>()
                    .record(claim.entity())
                    .is_some_and(|record| {
                        record.owner() == claim.owner()
                            && record.seal() == claim.seal()
                            && record.lifecycle() == EntityLifecycle::Active
                    })
            }));
            assert!(leaving_claims.iter().all(|claim| {
                app.world()
                    .resource::<RouteEntityRegistry>()
                    .record(claim.entity())
                    .is_some_and(|record| {
                        record.owner() == claim.owner()
                            && record.seal() == claim.seal()
                            && record.lifecycle() == EntityLifecycle::Leaving
                    })
            }));
            assert!(matches!(
                entry.engine.request(request).unwrap().status(),
                RequestStatus::Running(NavigationPhase::PostCommitCleanup)
            ));
            assert_eq!(entry.engine.history().len(), history_len_before + 1);
            assert_eq!(entry.engine.history().current(), Some(&replacement));
        }

        settle_cleanup_system(app.world_mut());

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert!(entry.cleanup_ledger.is_none());
        let expected_receipts = leaving_claims.len();
        assert!(leaving_claims.iter().all(|claim| {
            app.world().get_entity(claim.entity()).is_err()
                && app
                    .world()
                    .resource::<RouteEntityRegistry>()
                    .record(claim.entity())
                    .is_none()
        }));
        assert!(preserved_claims.iter().all(|claim| {
            app.world().get_entity(claim.entity()).is_ok()
                && app
                    .world()
                    .resource::<RouteEntityRegistry>()
                    .record(claim.entity())
                    .is_some_and(|record| {
                        record.owner() == claim.owner()
                            && record.seal() == claim.seal()
                            && record.lifecycle() == EntityLifecycle::Active
                    })
        }));
        assert!(
            app.world()
                .get_entity(preserved_slot_claim.entity())
                .is_ok()
        );
        let request = entry.engine.request(request).unwrap();
        assert!(matches!(request.status(), RequestStatus::Succeeded));
        assert_eq!(
            request
                .trace()
                .iter()
                .filter(|event| event.cleanup_disposition().is_some())
                .count(),
            expected_receipts
        );
        assert_eq!(
            request.trace().last().map(|event| event.kind()),
            Some(NavigationTraceEventKind::RequestTerminal)
        );
        assert_eq!(entry.engine.history().len(), history_len_before + 1);
        assert_eq!(entry.engine.history().current(), Some(&replacement));
        assert_eq!(observed.lock().unwrap().len(), effect_count_before + 1);
        assert_eq!(
            observed.lock().unwrap().last(),
            Some(&RouteViewKey::new("item").unwrap())
        );
    }

    #[test]
    fn reconciliation_truncate_retain_empty_commits_once_without_retry() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let router = install_nested_router(&mut app, observed, true);
        app.update();
        commit_initial_nested_branch(&mut app, router);
        let retained_slot = app.world().resource::<RouterNavigation>().entries[0]
            .active
            .as_ref()
            .unwrap()
            .child_slots()[0]
            .slot();

        let request = prepare_nested_branch_for_commit(&mut app, router, RouteLocation::new("/"));
        commit_presentation_system(app.world_mut());

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert!(
            entry.pending_commit.is_none(),
            "a sealed RetainEmpty truncate must not remain in retry"
        );
        let active = entry.active.as_ref().unwrap();
        assert_eq!(active.frames().len(), 1);
        assert_eq!(active.child_slots().len(), 1);
        assert_eq!(active.child_slots()[0].slot(), retained_slot);
        assert_eq!(
            app.world()
                .resource::<RouteEntityRegistry>()
                .record(retained_slot)
                .map(|record| record.lifecycle()),
            Some(EntityLifecycle::PreparedInert),
        );
        assert!(entry.cleanup_ledger.is_some());

        assert!(matches!(
            entry.engine.request(request).unwrap().status(),
            RequestStatus::Running(NavigationPhase::PostCommitCleanup)
        ));
    }

    #[test]
    fn reconciliation_equivalent_retained_empty_is_physical_no_op() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let router = install_nested_router(&mut app, observed.clone(), true);
        app.update();
        commit_initial_nested_branch(&mut app, router);
        let truncated = RouteLocation::new("/");

        let first = prepare_nested_branch_for_commit(&mut app, router, truncated.clone());
        commit_presentation_system(app.world_mut());
        settle_cleanup_system(app.world_mut());

        let (
            frame_before,
            shell_before,
            slot_before,
            claims_before,
            exact_claims_before,
            physical_facts_before,
            input_focus_before,
            route_focus_before,
            effect_count_before,
            history_len_before,
        ) = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            assert!(matches!(
                entry.engine.request(first).unwrap().status(),
                RequestStatus::Succeeded
            ));
            let active = entry.active.as_ref().unwrap();
            assert_eq!(active.frames().len(), 1);
            assert_eq!(active.child_slots().len(), 1);
            assert_eq!(
                active.child_slots()[0].slot_claim().lifecycle(),
                EntityLifecycle::PreparedInert
            );
            let claims = active.claims().cloned().collect::<Vec<_>>();
            let entities = claims
                .iter()
                .map(ExactRegistryClaim::entity)
                .collect::<Vec<_>>();
            (
                active.frames()[0].frame().clone(),
                active.frames()[0].shell(),
                active.child_slots()[0].clone_for_test(),
                claims,
                entities
                    .iter()
                    .map(|entity| {
                        (
                            *entity,
                            app.world()
                                .resource::<RouteEntityRegistry>()
                                .record(*entity)
                                .map(|record| record.claim()),
                        )
                    })
                    .collect::<Vec<_>>(),
                entities
                    .iter()
                    .map(|entity| nested_protected_fingerprint(app.world(), *entity))
                    .collect::<Vec<_>>(),
                app.world().resource::<InputFocus>().get(),
                app.world().resource::<RouteFocusState>().focused(),
                observed.lock().unwrap().len(),
                entry.engine.history().len(),
            )
        };

        let repeated = prepare_nested_branch_for_commit(&mut app, router, truncated.clone());
        {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            let pending = entry.pending_commit.as_ref().unwrap();
            let projection = match (pending.ticket.as_ref(), pending.final_task.as_ref()) {
                (Some(ticket), _) => entry
                    .engine
                    .reconciliation_projection_for_commit(ticket)
                    .unwrap(),
                (None, Some(task)) => entry
                    .engine
                    .reconciliation_projection_for_task(task)
                    .unwrap(),
                (None, None) => panic!("equivalent request retains a sealed projection"),
            };
            assert_eq!(projection.entering().len(), 0);
            assert_eq!(projection.leaving().len(), 0);
            assert!(entry.cleanup_ledger.is_none());
        }
        arm_test_commit_mutation_trace();
        commit_presentation_system(app.world_mut());
        assert_eq!(
            take_test_commit_mutation_trace(),
            Vec::<TestCommitMutation>::new(),
            "an unchanged PreparedInert RetainEmpty role must not reach presentation apply"
        );
        settle_cleanup_system(app.world_mut());

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert!(entry.pending_commit.is_none());
        assert!(entry.cleanup_ledger.is_none());
        assert!(matches!(
            entry.engine.request(repeated).unwrap().status(),
            RequestStatus::Succeeded
        ));
        assert_eq!(entry.engine.history().len(), history_len_before + 1);
        assert_eq!(entry.engine.history().current(), Some(&truncated));
        let active = entry.active.as_ref().unwrap();
        assert_eq!(active.frames().len(), 1);
        assert_eq!(active.child_slots().len(), 1);
        assert_eq!(active.frames()[0].frame(), &frame_before);
        assert_eq!(active.frames()[0].shell(), shell_before);
        assert_eq!(active.claims().cloned().collect::<Vec<_>>(), claims_before);
        let slot_after = &active.child_slots()[0];
        assert_eq!(slot_after.slot(), slot_before.slot());
        assert_eq!(slot_after.slot_claim(), slot_before.slot_claim());
        assert_eq!(slot_after.parent(), slot_before.parent());
        assert_eq!(slot_after.outlet(), slot_before.outlet());
        assert_eq!(slot_after.anchor(), slot_before.anchor());
        assert_eq!(slot_after.anchor_claim(), slot_before.anchor_claim());
        assert_eq!(slot_after.slot_id(), slot_before.slot_id());
        assert_eq!(
            app.world()
                .resource::<RouteEntityRegistry>()
                .record(slot_after.slot())
                .map(|record| (record.owner(), record.seal(), record.lifecycle())),
            Some((
                slot_before.slot_claim().owner(),
                slot_before.slot_claim().seal(),
                EntityLifecycle::PreparedInert,
            )),
        );
        assert_eq!(
            exact_claims_before,
            exact_claims_before
                .iter()
                .map(|(entity, _)| {
                    (
                        *entity,
                        app.world()
                            .resource::<RouteEntityRegistry>()
                            .record(*entity)
                            .map(|record| record.claim()),
                    )
                })
                .collect::<Vec<_>>()
        );
        assert_eq!(
            physical_facts_before,
            exact_claims_before
                .iter()
                .map(|(entity, _)| nested_protected_fingerprint(app.world(), *entity))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            app.world().resource::<InputFocus>().get(),
            input_focus_before
        );
        assert_eq!(
            app.world().resource::<RouteFocusState>().focused(),
            route_focus_before
        );
        assert_eq!(observed.lock().unwrap().len(), effect_count_before);
        assert!(entry.cleanup_ledger.is_none());
    }

    #[test]
    fn nested_truncate_retain_empty_releases_direct_and_descendant_focus() {
        for focus_descendant in [false, true] {
            let mut app = App::new();
            app.add_plugins(BevyUiRouterPlugin::default());
            let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
            let router = install_nested_router(&mut app, observed, true);
            app.update();
            commit_initial_nested_branch(&mut app, router);
            let retained_slot = app.world().resource::<RouterNavigation>().entries[0]
                .active
                .as_ref()
                .unwrap()
                .child_slots()[0]
                .slot();
            let focused = if focus_descendant {
                app.world_mut().spawn(ChildOf(retained_slot)).id()
            } else {
                retained_slot
            };
            *app.world_mut().resource_mut::<InputFocus>() = InputFocus::from_entity(focused);
            app.world_mut()
                .resource_mut::<RouteFocusState>()
                .set_focused(Some(focused));

            prepare_nested_branch_for_commit(&mut app, router, RouteLocation::new("/"));
            commit_presentation_system(app.world_mut());

            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            assert!(entry.pending_commit.is_none());
            assert_eq!(app.world().resource::<InputFocus>().get(), None);
            assert_eq!(app.world().resource::<RouteFocusState>().focused(), None);
            if focus_descendant {
                assert_eq!(
                    app.world().get::<ChildOf>(focused).map(ChildOf::parent),
                    Some(retained_slot),
                    "focus release must not mutate a foreign descendant"
                );
            }
        }
    }

    #[test]
    fn replacement_cleans_terminal_retained_empty_slot_by_exact_binding() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let router = install_deep_nested_router(&mut app);
        app.update();
        commit_initial_deep_nested_branch(&mut app, router);

        let truncate = prepare_deep_nested_branch_for_commit(
            &mut app,
            router,
            RouteLocation::new("/sections/a"),
        );
        commit_presentation_system(app.world_mut());
        settle_cleanup_system(app.world_mut());
        let (retained_slot, app_host) = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            let request = entry.engine.request(truncate).unwrap();
            let status = request.status();
            assert!(
                matches!(status, RequestStatus::Succeeded),
                "truncate cleanup did not settle: {status:?}; receipts={:?}; settled={:?}",
                entry
                    .cleanup_ledger
                    .as_ref()
                    .map(CleanupLedger::receipt_count),
                entry.cleanup_ledger.as_ref().map(CleanupLedger::is_settled),
            );
            let active = entry.active.as_ref().unwrap();
            let retained_slot = active
                .child_slots()
                .iter()
                .find(|slot| slot.parent() == active.frames()[1].frame())
                .unwrap()
                .slot();
            let app_host = app
                .world()
                .get::<ChildOf>(active.frames()[0].top_slot())
                .unwrap()
                .parent();
            (retained_slot, app_host)
        };

        let replacement =
            prepare_deep_nested_branch_for_commit(&mut app, router, RouteLocation::new("/"));
        commit_presentation_system(app.world_mut());

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert!(
            entry.pending_commit.is_none(),
            "terminal retained-empty slots must not make exact cleanup preflight retry"
        );
        assert!(entry.cleanup_ledger.is_some());
        settle_cleanup_system(app.world_mut());
        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert!(matches!(
            entry.engine.request(replacement).unwrap().status(),
            RequestStatus::Succeeded
        ));
        assert!(app.world().get_entity(retained_slot).is_err());
        assert!(app.world().get_entity(app_host).is_ok());
    }

    #[test]
    fn nested_replacement_failed_preflight_is_immediately_inert_and_preserves_active() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let router = install_nested_router(&mut app, observed, true);
        app.update();
        commit_initial_nested_branch(&mut app, router);
        let (old_shells, old_history) = {
            let navigation = app.world().resource::<RouterNavigation>();
            let entry = &navigation.entries[0];
            (
                entry
                    .active
                    .as_ref()
                    .unwrap()
                    .frames()
                    .iter()
                    .map(RealizedFrame::shell)
                    .collect::<Vec<_>>(),
                entry.engine.history().clone(),
            )
        };
        prepare_nested_branch_for_commit(&mut app, router, RouteLocation::new("/items/2"));
        let target = app.world().resource::<RouterNavigation>().entries[0]
            .pending_commit
            .as_ref()
            .unwrap()
            .branch
            .frames()[0]
            .interactive_entities()[0];
        app.world_mut().entity_mut(target).insert((
            Visibility::Visible,
            Pickable {
                should_block_lower: true,
                is_hoverable: true,
            },
            RouterInputBarrier::allow_all(),
        ));
        *app.world_mut().resource_mut::<InputFocus>() = InputFocus::from_entity(target);
        app.world_mut()
            .resource_mut::<RouteFocusState>()
            .set_focused(Some(target));

        commit_presentation_system(app.world_mut());

        let navigation = app.world().resource::<RouterNavigation>();
        let entry = &navigation.entries[0];
        assert!(entry.pending_failed_branch.is_some());
        assert_eq!(
            entry
                .active
                .as_ref()
                .unwrap()
                .frames()
                .iter()
                .map(RealizedFrame::shell)
                .collect::<Vec<_>>(),
            old_shells
        );
        assert_eq!(entry.engine.history(), &old_history);
        assert_eq!(
            app.world().get::<Visibility>(target),
            Some(&Visibility::Hidden)
        );
        assert_eq!(app.world().get::<Pickable>(target), Some(&Pickable::IGNORE));
        assert!(
            app.world()
                .get::<RouterInputBarrier>(target)
                .is_some_and(RouterInputBarrier::denies_all)
        );
        assert_eq!(app.world().resource::<InputFocus>().get(), None);
        assert_eq!(app.world().resource::<RouteFocusState>().focused(), None);
    }

    #[test]
    fn eager_failed_isolation_revalidates_relationships_before_releasing_focus() {
        #[derive(Resource)]
        struct ReparentAfterBarrierInsert {
            target: Entity,
            active_parent: Entity,
        }

        let mut app = App::new();
        app.world_mut()
            .register_component_hooks::<RouterInputBarrier>()
            .on_insert(|mut world, HookContext { entity, .. }| {
                let Some(drift) = world.get_resource::<ReparentAfterBarrierInsert>() else {
                    return;
                };
                if drift.target == entity {
                    let active_parent = drift.active_parent;
                    world.commands().entity(active_parent).add_child(entity);
                }
            });
        app.add_plugins(BevyUiRouterPlugin::default());
        let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let router = install_nested_router(&mut app, observed, true);
        app.update();
        commit_initial_nested_branch(&mut app, router);
        let active_parent = app.world().resource::<RouterNavigation>().entries[0]
            .active
            .as_ref()
            .unwrap()
            .frames()[0]
            .shell();
        prepare_nested_branch_for_commit(&mut app, router, RouteLocation::new("/items/2"));
        let target = app.world().resource::<RouterNavigation>().entries[0]
            .pending_commit
            .as_ref()
            .unwrap()
            .branch
            .frames()[0]
            .interactive_entities()[0];
        app.world_mut().entity_mut(target).insert((
            Visibility::Visible,
            Pickable {
                should_block_lower: true,
                is_hoverable: true,
            },
        ));
        app.world_mut()
            .entity_mut(target)
            .remove::<RouterInputBarrier>();
        app.world_mut().insert_resource(ReparentAfterBarrierInsert {
            target,
            active_parent,
        });
        *app.world_mut().resource_mut::<InputFocus>() = InputFocus::from_entity(target);
        app.world_mut()
            .resource_mut::<RouteFocusState>()
            .set_focused(Some(target));

        commit_presentation_system(app.world_mut());

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert!(entry.pending_failed_branch.is_some());
        assert!(entry.cleanup_ledger.is_none());
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(target));
        assert_eq!(
            app.world().resource::<RouteFocusState>().focused(),
            Some(target)
        );
        assert_eq!(
            app.world().get::<ChildOf>(target).map(ChildOf::parent),
            Some(active_parent)
        );
    }

    #[test]
    fn failed_settle_revalidates_active_descendants_before_releasing_focus() {
        #[derive(Resource)]
        struct ReparentFailedRootDuringSettle {
            target: Entity,
            active_descendant: Entity,
        }

        let mut app = App::new();
        app.world_mut()
            .register_component_hooks::<RouterInputBarrier>()
            .on_insert(|mut world, HookContext { entity, .. }| {
                let Some(drift) = world.get_resource::<ReparentFailedRootDuringSettle>() else {
                    return;
                };
                if drift.target == entity {
                    let active_descendant = drift.active_descendant;
                    world.commands().entity(active_descendant).add_child(entity);
                }
            });
        app.add_plugins(BevyUiRouterPlugin::default());
        let router = install_failing_deep_pending_router(&mut app);
        observe_authority_system(app.world_mut());
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        for _ in 0..5 {
            drive_navigation_system(app.world_mut());
        }
        commit_presentation_system(app.world_mut());
        settle_cleanup_system(app.world_mut());

        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/sections/a/items/1/detail"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        for _ in 0..10 {
            drive_navigation_system(app.world_mut());
            if app.world().resource::<RouterNavigation>().entries[0]
                .pending_failed_branch
                .is_some()
            {
                break;
            }
        }
        {
            let mut navigation = app.world_mut().resource_mut::<RouterNavigation>();
            let entry = &mut navigation.entries[0];
            let PendingFailedBranch { terminal, branch } = entry
                .pending_failed_branch
                .take()
                .expect("driver failure owner");
            let mut realized = branch.into_realized_for_test();
            realized.retain_last_frame_for_test();
            for frame in realized.frames_mut_for_test() {
                frame.mark_prewrite_incomplete_for_test();
            }
            entry.pending_failed_branch = Some(PendingFailedBranch {
                terminal,
                branch: FailedPreparedBranch::new(PreparedBranch::from_realized(realized)),
            });
        }
        let (active_root, failed_root) = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            (
                entry.active.as_ref().unwrap().frames()[0].shell(),
                entry
                    .pending_failed_branch
                    .as_ref()
                    .unwrap()
                    .branch
                    .frames()
                    .iter()
                    .chain(
                        entry
                            .pending_failed_branch
                            .as_ref()
                            .unwrap()
                            .branch
                            .detached_frames(),
                    )
                    .flat_map(RealizedFrame::claims)
                    .find(|claim| failed_cleanup_claim_is_current(app.world(), claim))
                    .expect("failed branch must retain an exact frame isolation target")
                    .entity(),
            )
        };
        let active_descendant = app.world_mut().spawn(ChildOf(active_root)).id();
        let active_closure_before = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            validate_branch_descendants(app.world(), entry.active.as_ref().unwrap()).unwrap()
        };
        assert!(
            !active_closure_before.contains(&failed_root),
            "the hook fixture needs a failed root outside the active closure before isolation"
        );
        {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            let active_claims = entry
                .active
                .as_ref()
                .unwrap()
                .claims()
                .cloned()
                .collect::<Vec<_>>();
            let failed_claims =
                failed_branch_primary_claims(&entry.pending_failed_branch.as_ref().unwrap().branch)
                    .into_iter()
                    .filter(|claim| failed_cleanup_claim_is_current(app.world(), claim))
                    .filter(|claim| !active_claims.contains(claim))
                    .cloned()
                    .collect::<Vec<_>>();
            let failed_closure = failed_branch_focus_closure(app.world(), &failed_claims);
            assert!(
                failed_closure
                    .iter()
                    .all(|entity| !active_closure_before.contains(entity)),
                "the complete failed closure must be disjoint before isolation"
            );
        }
        app.world_mut()
            .entity_mut(failed_root)
            .remove::<RouterInputBarrier>();
        {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            let registry = app.world().resource::<RouteEntityRegistry>();
            let manifest = FrozenCleanupManifest::from_failed_prepared_branch(
                app.world(),
                registry,
                &entry.pending_failed_branch.as_ref().unwrap().branch,
            )
            .expect("formal settle fixture must freeze");
            assert!(
                manifest.preflight(app.world()),
                "formal settle fixture must pass manifest preflight before isolation"
            );
            assert!(
                failed_prepared_branch_roots_are_exact(
                    app.world(),
                    &entry.pending_failed_branch.as_ref().unwrap().branch,
                    &manifest,
                ),
                "formal settle fixture must reach the isolation write path"
            );
        }
        app.world_mut().resource_scope(
            |world, mut navigation: bevy_ecs::change_detection::Mut<RouterNavigation>| {
                let entry = &mut navigation.entries[0];
                let failed = entry
                    .pending_failed_branch
                    .take()
                    .expect("formal settle fixture owner");
                let manifest = FrozenCleanupManifest::from_failed_prepared_branch(
                    world,
                    world.resource::<RouteEntityRegistry>(),
                    &failed.branch,
                )
                .expect("formal settle fixture must freeze for classification");
                match classify_failed_manifest(
                    manifest,
                    failed,
                    entry.active.as_ref(),
                    entry.host.as_ref(),
                ) {
                    FailedManifestDisposition::Ready { failed, .. } => {
                        entry.pending_failed_branch = Some(failed);
                    }
                    FailedManifestDisposition::CurrentAuthorityLoss { failed, .. } => {
                        entry.pending_failed_branch = Some(failed);
                        panic!("formal settle fixture classified current authority loss");
                    }
                    FailedManifestDisposition::AmbiguousInvalidTopology { failed, .. } => {
                        entry.pending_failed_branch = Some(failed);
                        panic!("formal settle fixture classified ambiguous topology");
                    }
                }
            },
        );
        app.world_mut()
            .insert_resource(ReparentFailedRootDuringSettle {
                target: failed_root,
                active_descendant,
            });
        *app.world_mut().resource_mut::<InputFocus>() = InputFocus::from_entity(failed_root);
        app.world_mut()
            .resource_mut::<RouteFocusState>()
            .set_focused(Some(failed_root));

        settle_cleanup_system(app.world_mut());

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert!(entry.pending_failed_branch.is_some());
        assert!(entry.cleanup_ledger.is_none());
        assert_eq!(
            app.world().resource::<InputFocus>().get(),
            Some(failed_root)
        );
        assert_eq!(
            app.world().resource::<RouteFocusState>().focused(),
            Some(failed_root)
        );
        assert_eq!(
            app.world().get::<ChildOf>(failed_root).map(ChildOf::parent),
            Some(active_descendant)
        );
    }

    #[test]
    fn nested_missing_outlet_rejects_before_slot_or_registry_write() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let router = install_nested_router(&mut app, observed, false);
        observe_authority_system(app.world_mut());
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/items/1"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        let world_before = authority_world_fingerprint(app.world());
        let registry_before = app
            .world()
            .resource::<RouteEntityRegistry>()
            .records()
            .to_vec();
        let seal_before = app
            .world()
            .resource::<RouteEntityRegistry>()
            .latest_issued_seal();

        drive_navigation_system(app.world_mut());

        assert_eq!(authority_world_fingerprint(app.world()), world_before);
        assert_eq!(
            app.world().resource::<RouteEntityRegistry>().records(),
            registry_before
        );
        assert_eq!(
            app.world()
                .resource::<RouteEntityRegistry>()
                .latest_issued_seal(),
            seal_before
        );
        let navigation = app.world().resource::<RouterNavigation>();
        let entry = &navigation.entries[0];
        assert!(entry.pending_commit.is_none());
        assert!(entry.pending_mount_request.is_none());
        assert!(entry.pending_failed_branch.is_some());
        let request_view = entry.engine.request(request).unwrap();
        let trace = request_view.trace();
        assert!(
            trace.iter().any(|event| {
                event.mount_rejection() == Some(MountRejectionKind::MissingOutlet)
            })
        );
        assert!(trace.iter().all(|event| {
            event.kind() != bevy_ui_router_core::prelude::NavigationTraceEventKind::MountGranted
        }));
    }

    #[test]
    fn nested_invalid_outlet_rejects_before_slot_or_registry_write() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let router = install_nested_router(&mut app, observed, true);
        observe_authority_system(app.world_mut());
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/items/1"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        let anchor = app.world().resource::<RouterNavigation>().entries[0]
            .pending_commit
            .as_ref()
            .unwrap()
            .branch
            .frames()[0]
            .outlets()[0]
            .anchor();
        app.world_mut()
            .entity_mut(anchor)
            .remove::<RouteOwnershipSeal>();
        app.world_mut().flush();
        let world_before = authority_world_fingerprint(app.world());
        let registry_before = app
            .world()
            .resource::<RouteEntityRegistry>()
            .records()
            .to_vec();
        let seal_before = app
            .world()
            .resource::<RouteEntityRegistry>()
            .latest_issued_seal();

        drive_navigation_system(app.world_mut());

        assert_eq!(authority_world_fingerprint(app.world()), world_before);
        assert_eq!(
            app.world().resource::<RouteEntityRegistry>().records(),
            registry_before
        );
        assert_eq!(
            app.world()
                .resource::<RouteEntityRegistry>()
                .latest_issued_seal(),
            seal_before
        );
        let navigation = app.world().resource::<RouterNavigation>();
        let entry = &navigation.entries[0];
        assert!(entry.pending_failed_branch.is_some());
        let request_view = entry.engine.request(request).unwrap();
        let trace = request_view.trace();
        assert!(
            trace.iter().any(|event| {
                event.mount_rejection() == Some(MountRejectionKind::InvalidOutlet)
            })
        );
        assert!(trace.iter().all(|event| {
            event.kind() != bevy_ui_router_core::prelude::NavigationTraceEventKind::MountGranted
        }));
    }

    fn assert_nested_slot_residual_settles(
        app: &mut App,
        request: NavigationRequestId,
        expected_state: crate::mount::ChildSlotResidualState,
    ) {
        let (residual_claim, expected_receipts) = {
            let navigation = app.world().resource::<RouterNavigation>();
            let entry = &navigation.entries[0];
            assert!(matches!(
                entry.engine.request(request).unwrap().status(),
                RequestStatus::Running(_)
            ));
            let failed = entry.pending_failed_branch.as_ref().unwrap();
            let residual = &failed.branch.mount_residuals()[0];
            assert_eq!(residual.state(), expected_state);
            let manifest = FrozenCleanupManifest::from_failed_prepared_branch(
                app.world(),
                app.world().resource::<RouteEntityRegistry>(),
                &failed.branch,
            )
            .unwrap();
            (residual.claim().clone(), manifest.len())
        };

        settle_cleanup_system(app.world_mut());

        assert_ne!(
            app.world()
                .resource::<RouteEntityRegistry>()
                .record(residual_claim.entity())
                .map(|record| record.claim()),
            Some(residual_claim)
        );
        let mut navigation = app.world_mut().resource_mut::<RouterNavigation>();
        let entry = &mut navigation.entries[0];
        assert!(entry.pending_failed_branch.is_none());
        let request = entry.engine.request(request).unwrap();
        assert!(matches!(request.status(), RequestStatus::Failed(_)));
        let receipt_count = request
            .trace()
            .iter()
            .filter(|event| event.cleanup_disposition().is_some())
            .count();
        assert_eq!(receipt_count, expected_receipts);
        assert_eq!(
            request
                .trace()
                .iter()
                .map(|event| event.kind())
                .rev()
                .take(2)
                .collect::<Vec<_>>(),
            vec![
                bevy_ui_router_core::prelude::NavigationTraceEventKind::RequestTerminal,
                bevy_ui_router_core::prelude::NavigationTraceEventKind::CleanupSettled,
            ]
        );
    }

    #[test]
    fn nested_provisional_slot_residual_settles_before_core_advances() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let router = install_nested_router(&mut app, observed, true);
        observe_authority_system(app.world_mut());
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/items/1"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        app.world_mut()
            .register_component_hooks::<ChildSlotBarrierProbe>()
            .on_insert(|mut world, HookContext { entity, .. }| {
                world
                    .commands()
                    .entity(entity)
                    .remove::<RouterInputBarrier>();
            });

        drive_navigation_system(app.world_mut());

        assert_nested_slot_residual_settles(
            &mut app,
            request,
            crate::mount::ChildSlotResidualState::Provisional,
        );
    }

    #[test]
    fn provisional_slot_with_foreign_component_converges_as_unresolved() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let router = install_nested_router(&mut app, observed, true);
        observe_authority_system(app.world_mut());
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/items/1"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        app.world_mut()
            .register_component_hooks::<ChildSlotBarrierProbe>()
            .on_insert(|mut world, HookContext { entity, .. }| {
                world.commands().entity(entity).insert(CleanupDriftMarker);
            });

        drive_navigation_system(app.world_mut());

        let residual = app.world().resource::<RouterNavigation>().entries[0]
            .pending_failed_branch
            .as_ref()
            .unwrap()
            .branch
            .mount_residuals()[0]
            .entity();
        let actual_parent = app
            .world()
            .get::<bevy_ecs::hierarchy::ChildOf>(residual)
            .map(bevy_ecs::hierarchy::ChildOf::parent);
        let source_claim = app
            .world()
            .resource::<RouteEntityRegistry>()
            .record(residual)
            .unwrap()
            .claim();
        assert!(app.world().get::<CleanupDriftMarker>(residual).is_some());
        assert!(app.world().get::<RouteOwnershipSeal>(residual).is_none());
        assert!(app.world().get::<ChildSlotLifecycle>(residual).is_none());
        assert!(
            app.world()
                .get::<crate::components::ChildSlotAuthority>(residual)
                .is_none()
        );

        *app.world_mut().resource_mut::<InputFocus>() = InputFocus::from_entity(residual);
        app.world_mut()
            .resource_mut::<RouteFocusState>()
            .set_focused(Some(residual));
        crate::registry::fail_next_unresolved_preparation_for_test();
        settle_cleanup_system(app.world_mut());

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert_eq!(entry.cleanup_ledger.as_ref().unwrap().receipt_count(), 0);
        assert!(
            app.world()
                .resource::<RouteEntityRegistry>()
                .record(residual)
                .is_some_and(|record| record.claim() == source_claim)
        );
        assert!(
            app.world()
                .resource::<QuarantineLeaseTable>()
                .unresolved_record(residual)
                .is_none()
        );
        assert_eq!(app.world().resource::<InputFocus>().get(), None);
        assert_eq!(app.world().resource::<RouteFocusState>().focused(), None);
        assert_eq!(
            app.world()
                .get::<bevy_ecs::hierarchy::ChildOf>(residual)
                .map(bevy_ecs::hierarchy::ChildOf::parent),
            actual_parent
        );

        settle_cleanup_system(app.world_mut());

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert!(entry.pending_failed_branch.is_none());
        assert!(entry.cleanup_ledger.is_none());
        assert!(
            matches!(
                entry.engine.request(request).unwrap().status(),
                RequestStatus::Failed(_)
            ),
            "status: {:?}",
            entry.engine.request(request).unwrap().status()
        );
        assert!(
            app.world()
                .resource::<RouteEntityRegistry>()
                .record(residual)
                .is_none()
        );
        assert!(
            app.world()
                .resource::<QuarantineLeaseTable>()
                .unresolved_record(residual)
                .is_some()
        );
        assert!(app.world().get::<CleanupDriftMarker>(residual).is_some());
        assert!(app.world().get::<RouteOwnershipSeal>(residual).is_none());
        assert!(app.world().get::<ChildSlotLifecycle>(residual).is_none());
        assert!(
            app.world()
                .get::<crate::components::ChildSlotAuthority>(residual)
                .is_none()
        );
        assert_eq!(
            app.world()
                .get::<bevy_ecs::hierarchy::ChildOf>(residual)
                .map(bevy_ecs::hierarchy::ChildOf::parent),
            actual_parent
        );
        assert_eq!(app.world().resource::<InputFocus>().get(), None);
        assert_eq!(app.world().resource::<RouteFocusState>().focused(), None);
        assert_eq!(
            app.world().get::<Visibility>(residual),
            Some(&Visibility::Hidden)
        );
        assert_eq!(
            app.world().get::<Pickable>(residual),
            Some(&Pickable::IGNORE)
        );
        assert!(
            app.world()
                .get::<RouterInputBarrier>(residual)
                .is_some_and(RouterInputBarrier::denies_all)
        );
    }

    #[test]
    fn nested_registered_slot_residual_settles_before_core_advances() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let router = install_nested_router(&mut app, observed, true);
        observe_authority_system(app.world_mut());
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/items/1"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        app.world_mut()
            .register_component_hooks::<ChildSlotLifecycle>()
            .on_insert(|mut world, HookContext { entity, .. }| {
                if world.get::<ChildSlotLifecycle>(entity)
                    == Some(&ChildSlotLifecycle::RegisteredInert)
                {
                    world.commands().entity(entity).insert(Node {
                        width: Val::Percent(99.0),
                        height: Val::Percent(100.0),
                        ..Node::default()
                    });
                }
            });

        drive_navigation_system(app.world_mut());

        assert_nested_slot_residual_settles(
            &mut app,
            request,
            crate::mount::ChildSlotResidualState::Registered,
        );
    }

    #[test]
    fn nested_absent_slot_residual_settles_before_core_advances() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let router = install_nested_router(&mut app, observed, true);
        observe_authority_system(app.world_mut());
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/items/1"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        app.world_mut()
            .register_component_hooks::<ChildSlotAuthorityProbe>()
            .on_insert(|mut world, HookContext { entity, .. }| {
                world.commands().entity(entity).despawn();
            });

        drive_navigation_system(app.world_mut());

        assert_nested_slot_residual_settles(
            &mut app,
            request,
            crate::mount::ChildSlotResidualState::Absent,
        );
    }

    #[test]
    fn nested_partial_child_frame_settles_before_core_advances() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        app.world_mut()
            .insert_resource(ChildFrameDriftArmed::default());
        app.world_mut()
            .register_component_hooks::<FrameOutput>()
            .on_add(|mut world, HookContext { entity, .. }| {
                if world
                    .get_resource::<ChildFrameDriftArmed>()
                    .is_some_and(|armed| armed.0)
                {
                    world.commands().entity(entity).insert(CleanupDriftMarker);
                }
            });
        let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let router = install_nested_router(&mut app, observed, true);
        observe_authority_system(app.world_mut());
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/items/1"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        app.world_mut().resource_mut::<ChildFrameDriftArmed>().0 = true;

        drive_navigation_system(app.world_mut());

        let (claims, child_slot, expected_receipts) = {
            let navigation = app.world().resource::<RouterNavigation>();
            let entry = &navigation.entries[0];
            assert!(matches!(
                entry.engine.request(request).unwrap().status(),
                RequestStatus::Running(_)
            ));
            let failed = entry.pending_failed_branch.as_ref().unwrap();
            assert_eq!(failed.branch.frames().len(), 2);
            assert_eq!(failed.branch.child_slots().len(), 1);
            assert!(failed.branch.mount_residuals().is_empty());
            let manifest = FrozenCleanupManifest::from_failed_prepared_branch(
                app.world(),
                app.world().resource::<RouteEntityRegistry>(),
                &failed.branch,
            )
            .unwrap();
            let claims = failed
                .branch
                .frames()
                .iter()
                .flat_map(RealizedFrame::claims)
                .cloned()
                .chain(
                    failed
                        .branch
                        .child_slots()
                        .iter()
                        .map(|slot| slot.slot_claim().clone()),
                )
                .collect::<Vec<_>>();
            (
                claims,
                failed.branch.child_slots()[0].slot(),
                manifest.len(),
            )
        };

        settle_cleanup_system(app.world_mut());

        for claim in claims {
            assert_ne!(
                app.world()
                    .resource::<RouteEntityRegistry>()
                    .record(claim.entity())
                    .map(|record| record.claim()),
                Some(claim)
            );
        }
        assert_eq!(
            app.world().get::<ChildSlotLifecycle>(child_slot),
            Some(&ChildSlotLifecycle::Quarantined)
        );
        assert_eq!(
            app.world()
                .resource::<QuarantineLeaseTable>()
                .record(child_slot)
                .unwrap()
                .lifecycle(),
            EntityLifecycle::Quarantined
        );
        let navigation = app.world().resource::<RouterNavigation>();
        let entry = &navigation.entries[0];
        assert!(entry.pending_failed_branch.is_none());
        assert!(entry.cleanup_ledger.is_none());
        let request = entry.engine.request(request).unwrap();
        assert!(matches!(request.status(), RequestStatus::Failed(_)));
        assert_eq!(
            request
                .trace()
                .iter()
                .filter(|event| event.cleanup_disposition().is_some())
                .count(),
            expected_receipts
        );
        assert_eq!(
            request
                .trace()
                .iter()
                .map(|event| event.kind())
                .rev()
                .take(2)
                .collect::<Vec<_>>(),
            vec![
                bevy_ui_router_core::prelude::NavigationTraceEventKind::RequestTerminal,
                bevy_ui_router_core::prelude::NavigationTraceEventKind::CleanupSettled,
            ]
        );
    }

    #[test]
    fn nested_mount_resolution_exhaustion_reissues_without_driver_rerun() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let layout = RouteViewKey::new("layout").unwrap();
        let router = install_nested_router(&mut app, observed.clone(), true);
        observe_authority_system(app.world_mut());
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/items/1"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        {
            let mut navigation = app.world_mut().resource_mut::<RouterNavigation>();
            navigation.entries[0].engine.testing_exhaust_slot_counter();
        }
        let world_before = authority_world_fingerprint(app.world());
        let registry_before = app
            .world()
            .resource::<RouteEntityRegistry>()
            .records()
            .to_vec();

        drive_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());

        assert_eq!(*observed.lock().unwrap(), vec![layout]);
        assert_eq!(authority_world_fingerprint(app.world()), world_before);
        assert_eq!(
            app.world().resource::<RouteEntityRegistry>().records(),
            registry_before
        );
        let navigation = app.world().resource::<RouterNavigation>();
        let pending = navigation.entries[0].pending_commit.as_ref().unwrap();
        assert_eq!(pending.branch.frames().len(), 1);
        assert!(pending.branch.child_slots().is_empty());
        assert!(pending.final_task.is_none());
    }

    fn authority_world_fingerprint(
        world: &World,
    ) -> Vec<(Entity, Vec<bevy_ecs::component::ComponentId>)> {
        let mut fingerprint = world
            .iter_entities()
            .map(|entity| (entity.id(), entity.archetype().components().to_vec()))
            .collect::<Vec<_>>();
        fingerprint.sort_by_key(|(entity, _)| entity.to_bits());
        fingerprint
    }

    fn applied_report_with_identity(
        identity: ViewReportIdentity,
    ) -> (ContentPlan, ViewBuildReport) {
        let mut plan = ContentPlan::new();
        let mut writer = FrameContentWriter::new(&mut plan);
        writer
            .spawn_solid_rect(SolidRect::new(24, 16, ContentColor::srgba8(1, 2, 3, 255)).unwrap())
            .unwrap();
        let mut seal = ReportSeal::new();
        let report = EcsViewReportSession::new(&mut seal, identity).applied();
        (plan, report)
    }

    fn foreign_report_identity(label: &str) -> ViewReportIdentity {
        let view = RouteViewKey::new(format!("{label}-view")).unwrap();
        let routes = RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/").unwrap(),
                RouteNodeOptions::new().with_view(view.clone()),
            )
            .unwrap()
            .build()
            .unwrap();
        let mut engine = RouterEngine::install(routes, view).unwrap();
        let token = engine.request_host_admission().unwrap();
        let grant = engine.grant_host_admission(&token).unwrap();
        engine.confirm_host(grant);
        engine.push(RouteLocation::new("/")).unwrap();
        engine.collect();
        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("foreign engine must issue one frame task")
        };
        task.report_identity()
    }

    fn foreign_fallback_task(label: &str) -> RootFrameTask {
        let view = RouteViewKey::new(format!("{label}-view")).unwrap();
        let routes = RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/").unwrap(),
                RouteNodeOptions::new().with_view(view.clone()),
            )
            .unwrap()
            .build()
            .unwrap();
        let mut engine = RouterEngine::install(routes, view).unwrap();
        let token = engine.request_host_admission().unwrap();
        let grant = engine.grant_host_admission(&token).unwrap();
        engine.confirm_host(grant);
        engine.push(RouteLocation::new("/")).unwrap();
        engine.collect();
        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("foreign engine must issue one normal frame task")
        };
        engine
            .report_failed(task, ViewFailureKind::Build, "foreign fixture failure")
            .unwrap();
        let EngineAction::FrameTask(fallback) = engine.action().unwrap() else {
            panic!("foreign engine must issue one fallback task")
        };
        assert_eq!(fallback.frame().origin(), &FrameOrigin::RouterFallback);
        fallback
    }

    fn commit_nested_parent_only(app: &mut App, router: RouterHandle) {
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        commit_presentation_system(app.world_mut());
        settle_cleanup_system(app.world_mut());
        let entry = app
            .world()
            .resource::<RouterNavigation>()
            .entries
            .iter()
            .find(|entry| entry.handle == router)
            .unwrap();
        assert_eq!(entry.active.as_ref().unwrap().frames().len(), 1);
        assert!(entry.pending_commit.is_none());
    }

    #[test]
    fn confirmed_mount_frame_task_is_consumed_exactly_once_into_final_task() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let router = install_nested_router(&mut app, observed.clone(), true);
        observe_authority_system(app.world_mut());
        commit_nested_parent_only(&mut app, router);

        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/items/1"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        let task_identity = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            assert!(entry.pending_commit.is_some());
            entry
                .pending_mount_frame_task
                .as_ref()
                .expect("confirmed MountNew owns its issued task")
                .report_identity()
        };
        let driver_calls_before = observed.lock().unwrap().len();

        drive_navigation_system(app.world_mut());

        {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            assert!(entry.pending_mount_frame_task.is_none());
            assert!(
                entry
                    .pending_commit
                    .as_ref()
                    .and_then(|pending| pending.final_task.as_ref())
                    .map(RootFrameTask::report_identity)
                    == Some(task_identity.clone())
            );
        }
        assert_eq!(observed.lock().unwrap().len(), driver_calls_before + 1);

        drive_navigation_system(app.world_mut());

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert!(entry.pending_mount_frame_task.is_none());
        assert!(
            entry
                .pending_commit
                .as_ref()
                .and_then(|pending| pending.final_task.as_ref())
                .map(RootFrameTask::report_identity)
                == Some(task_identity)
        );
        assert_eq!(observed.lock().unwrap().len(), driver_calls_before + 1);
    }

    #[test]
    fn stale_confirmed_mount_frame_task_transfers_once_without_authority_write() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let source = install_atomic_router(&mut app, "mount-task-stale-source");
        let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let target = install_nested_router(&mut app, observed, true);
        observe_authority_system(app.world_mut());
        commit_nested_parent_only(&mut app, target);

        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(source, RouteLocation::new("/"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        let source_task = {
            let mut navigation = app.world_mut().resource_mut::<RouterNavigation>();
            let source_entry = navigation
                .entries
                .iter_mut()
                .find(|entry| entry.handle == source)
                .unwrap();
            let EngineAction::FrameTask(task) = source_entry.engine.action().unwrap() else {
                panic!("source router owns one exact task")
            };
            task
        };

        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(target, RouteLocation::new("/items/1"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        let (source_identity, target_identity) = {
            let mut navigation = app.world_mut().resource_mut::<RouterNavigation>();
            let target_entry = navigation
                .entries
                .iter_mut()
                .find(|entry| entry.handle == target)
                .unwrap();
            let target_task = target_entry
                .pending_mount_frame_task
                .take()
                .expect("confirmed target mount owns its child task");
            let source_identity = source_task.report_identity();
            let target_identity = target_task.report_identity();
            target_entry.pending_mount_frame_task = Some(source_task);
            let source_entry = navigation
                .entries
                .iter_mut()
                .find(|entry| entry.handle == source)
                .unwrap();
            source_entry.pending_mount_frame_task = Some(target_task);
            (source_identity, target_identity)
        };
        let before_world = authority_world_fingerprint(app.world());
        let before_registry = app
            .world()
            .resource::<RouteEntityRegistry>()
            .records()
            .to_vec();

        drive_navigation_system(app.world_mut());

        let navigation = app.world().resource::<RouterNavigation>();
        let source_entry = navigation
            .entries
            .iter()
            .find(|entry| entry.handle == source)
            .unwrap();
        let target_entry = navigation
            .entries
            .iter()
            .find(|entry| entry.handle == target)
            .unwrap();
        assert!(source_entry.pending_mount_frame_task.is_none());
        assert!(target_entry.pending_mount_frame_task.is_none());
        assert!(matches!(
            source_entry.pending_failed_branch.as_ref().map(|failed| &failed.terminal),
            Some(PendingFailedBranchTerminal::Task { task, .. })
                if task.report_identity() == target_identity
        ));
        assert!(matches!(
            target_entry.pending_failed_branch.as_ref().map(|failed| &failed.terminal),
            Some(PendingFailedBranchTerminal::Task { task, .. })
                if task.report_identity() == source_identity
        ));
        assert_eq!(authority_world_fingerprint(app.world()), before_world);
        assert_eq!(
            app.world().resource::<RouteEntityRegistry>().records(),
            before_registry,
        );
    }

    #[test]
    fn exact_root_stale_application_retains_task_outside_final_task() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let router = install_atomic_router(&mut app, "root-stale-owner");
        observe_authority_system(app.world_mut());
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        let (task, host_candidate, top_slot, top_slot_claim) = {
            let mut navigation = app.world_mut().resource_mut::<RouterNavigation>();
            let entry = &mut navigation.entries[0];
            let EngineAction::FrameTask(task) = entry.engine.action().unwrap() else {
                panic!("root task must be issued")
            };
            let host = entry.host.as_ref().unwrap();
            (
                task,
                host.candidate.clone(),
                host.top_slot,
                host.top_slot_claim.clone(),
            )
        };
        let before_world = authority_world_fingerprint(app.world());
        let before_registry = app
            .world()
            .resource::<RouteEntityRegistry>()
            .records()
            .to_vec();
        let pending = PendingCommit::new();
        let (plan, report) = applied_report_with_identity(task.report_identity());
        app.world_mut().resource_scope(
            |world, mut navigation: bevy_ecs::change_detection::Mut<RouterNavigation>| {
                let entry = &mut navigation.entries[0];
                let result = apply_applied_driver_report_to_branch(
                    world,
                    &mut entry.engine,
                    host_candidate,
                    top_slot,
                    top_slot_claim,
                    Some(&pending.branch),
                    None,
                    &task,
                    &report,
                    plan,
                );
                assert!(matches!(result, Err(DriverReportApplicationError::Stale)));
                entry.pending_failed_branch = Some(stale_driver_report_owner(pending, task));
            },
        );
        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert!(entry.pending_commit.is_none());
        let failed = entry.pending_failed_branch.as_ref().unwrap();
        assert!(matches!(
            failed.terminal,
            PendingFailedBranchTerminal::Task { .. }
        ));
        assert!(failed.branch.frames().is_empty());
        assert_eq!(authority_world_fingerprint(app.world()), before_world);
        assert_eq!(
            app.world().resource::<RouteEntityRegistry>().records(),
            before_registry
        );

        settle_cleanup_system(app.world_mut());

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert!(entry.pending_failed_branch.is_none());
        assert!(entry.cleanup_ledger.is_none());
        let status = entry.engine.request(request).unwrap().status().clone();
        assert_eq!(status, RequestStatus::Running(NavigationPhase::Recovering));
        assert_eq!(authority_world_fingerprint(app.world()), before_world);
    }

    #[test]
    fn exact_child_stale_application_retains_nonleaf_branch_for_cleanup() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let router = install_nested_router(&mut app, observed, true);
        observe_authority_system(app.world_mut());
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/items/1"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        let (task, pending, host_candidate, top_slot, top_slot_claim) = {
            let mut navigation = app.world_mut().resource_mut::<RouterNavigation>();
            let entry = &mut navigation.entries[0];
            let task = entry
                .pending_mount_frame_task
                .take()
                .expect("confirmed child mount must own its issued task");
            let host = entry.host.as_ref().unwrap();
            let host_candidate = host.candidate.clone();
            let top_slot = host.top_slot;
            let top_slot_claim = host.top_slot_claim.clone();
            (
                task,
                entry.pending_commit.take().unwrap(),
                host_candidate,
                top_slot,
                top_slot_claim,
            )
        };
        let owned = pending
            .branch
            .frames()
            .iter()
            .flat_map(RealizedFrame::claims)
            .map(crate::registry::ExactRegistryClaim::entity)
            .chain(pending.branch.child_slots().iter().map(|slot| slot.slot()))
            .collect::<Vec<_>>();
        let (plan, report) = applied_report_with_identity(task.report_identity());
        app.world_mut().resource_scope(
            |world, mut navigation: bevy_ecs::change_detection::Mut<RouterNavigation>| {
                let entry = &mut navigation.entries[0];
                let result = apply_applied_driver_report_to_branch(
                    world,
                    &mut entry.engine,
                    host_candidate,
                    top_slot,
                    top_slot_claim,
                    None,
                    None,
                    &task,
                    &report,
                    plan,
                );
                assert!(matches!(result, Err(DriverReportApplicationError::Stale)));
                entry.pending_failed_branch = Some(stale_driver_report_owner(pending, task));
            },
        );
        let failed = app.world().resource::<RouterNavigation>().entries[0]
            .pending_failed_branch
            .as_ref()
            .unwrap();
        assert_eq!(failed.branch.frames().len(), 1);
        assert_eq!(failed.branch.child_slots().len(), 1);

        settle_cleanup_system(app.world_mut());

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert!(entry.pending_failed_branch.is_none());
        assert!(entry.cleanup_ledger.is_none());
        assert!(matches!(
            entry.engine.request(request).unwrap().status(),
            RequestStatus::Failed(_)
        ));
        assert!(owned.iter().all(|entity| {
            app.world()
                .resource::<RouteEntityRegistry>()
                .record(*entity)
                .is_none()
        }));
    }

    fn prepare_exact_child_stale_failure(app: &mut App) -> (NavigationRequestId, RouterHandle) {
        app.add_plugins(BevyUiRouterPlugin::default());
        let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let router = install_nested_router(app, observed, true);
        observe_authority_system(app.world_mut());
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/items/1"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        let (task, pending, host_candidate, top_slot, top_slot_claim) = {
            let mut navigation = app.world_mut().resource_mut::<RouterNavigation>();
            let entry = &mut navigation.entries[0];
            let task = entry
                .pending_mount_frame_task
                .take()
                .expect("confirmed child mount must own its issued task");
            let host = entry.host.as_ref().unwrap();
            (
                task,
                entry.pending_commit.take().unwrap(),
                host.candidate.clone(),
                host.top_slot,
                host.top_slot_claim.clone(),
            )
        };
        let (plan, report) = applied_report_with_identity(task.report_identity());
        app.world_mut().resource_scope(
            |world, mut navigation: bevy_ecs::change_detection::Mut<RouterNavigation>| {
                let entry = &mut navigation.entries[0];
                let result = apply_applied_driver_report_to_branch(
                    world,
                    &mut entry.engine,
                    host_candidate,
                    top_slot,
                    top_slot_claim,
                    None,
                    None,
                    &task,
                    &report,
                    plan,
                );
                assert!(matches!(result, Err(DriverReportApplicationError::Stale)));
                entry.pending_failed_branch = Some(stale_driver_report_owner(pending, task));
            },
        );
        (request, router)
    }

    fn prepare_exact_child_physical_residual_failure(
        app: &mut App,
    ) -> (NavigationRequestId, RouterHandle) {
        app.add_plugins(BevyUiRouterPlugin::default());
        let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let router = install_nested_router(app, observed, true);
        observe_authority_system(app.world_mut());
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/items/1"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        let (task, mut pending, host_candidate, top_slot, top_slot_claim) = {
            let mut navigation = app.world_mut().resource_mut::<RouterNavigation>();
            let entry = &mut navigation.entries[0];
            let task = entry
                .pending_mount_frame_task
                .take()
                .expect("confirmed child mount must own its issued task");
            let host = entry.host.as_ref().unwrap();
            (
                task,
                entry.pending_commit.take().unwrap(),
                host.candidate.clone(),
                host.top_slot,
                host.top_slot_claim.clone(),
            )
        };
        let (plan, report) = applied_report_with_identity(task.report_identity());
        app.world_mut().resource_scope(
            |world, mut navigation: bevy_ecs::change_detection::Mut<RouterNavigation>| {
                let entry = &mut navigation.entries[0];
                let realized = match apply_applied_driver_report_to_branch(
                    world,
                    &mut entry.engine,
                    host_candidate,
                    top_slot,
                    top_slot_claim,
                    Some(&pending.branch),
                    None,
                    &task,
                    &report,
                    plan,
                ) {
                    Ok(realized) => realized,
                    Err(_) => {
                        panic!("physical-residual fixture must realize the unreported task target")
                    }
                };
                pending
                    .branch
                    .push_frame(realized)
                    .expect("physical-residual target extends the prepared branch");
                entry.pending_failed_branch = Some(PendingFailedBranch {
                    terminal: PendingFailedBranchTerminal::TaskWithPhysicalResidual {
                        task,
                        failure: ViewFailureKind::Apply,
                        message: "test retains an unreported physical task target",
                    },
                    branch: failed_prepared_branch(pending, None),
                });
            },
        );
        (request, router)
    }

    fn foreign_mount_receipt_proof() -> MountReceiptProof {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let router = install_nested_router(&mut app, observed, true);
        observe_authority_system(app.world_mut());
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/items/1"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        app.world_mut().resource_scope(
            |world, mut navigation: bevy_ecs::change_detection::Mut<RouterNavigation>| {
                let entry = &mut navigation.entries[0];
                let pending = entry
                    .pending_commit
                    .take()
                    .expect("foreign proof fixture retains its prepared parent");
                let EngineAction::MountRequest(mount) = entry.engine.action().unwrap() else {
                    panic!("foreign proof fixture requires a mount request")
                };
                let outlet = mount.outlet().clone();
                let grant = entry.engine.resolve_mount(mount).unwrap();
                let staged =
                    stage_child_slot(world, pending.branch.last_frame().unwrap(), &outlet, &grant)
                        .unwrap();
                let (_, proof) = grant.attest_receipt_with_proof(staged.receipt().digest());
                proof
            },
        )
    }

    fn prepare_registration_begin_failure(app: &mut App) -> (NavigationRequestId, RouterHandle) {
        app.add_plugins(BevyUiRouterPlugin::default());
        let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let router = install_nested_router(app, observed, true);
        observe_authority_system(app.world_mut());
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/items/1"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        let foreign_proof = foreign_mount_receipt_proof();

        app.world_mut().resource_scope(
            |world, mut navigation: bevy_ecs::change_detection::Mut<RouterNavigation>| {
                let entry = &mut navigation.entries[0];
                let pending = entry
                    .pending_commit
                    .take()
                    .expect("registration-begin fixture retains its prepared parent");
                let EngineAction::MountRequest(mount) = entry
                    .engine
                    .action()
                    .expect("prepared parent retains its exact mount request")
                else {
                    panic!("registration-begin fixture requires a mount request")
                };
                let outlet = mount.outlet().clone();
                let grant = entry
                    .engine
                    .resolve_mount(mount)
                    .expect("exact mount request resolves once");
                let parent = pending
                    .branch
                    .last_frame()
                    .expect("prepared parent remains owned");
                let staged = stage_child_slot(world, parent, &outlet, &grant)
                    .expect("registration-begin fixture stages one inert slot");
                let (receipt, _proof) = grant.attest_receipt_with_proof(staged.receipt().digest());
                let permit = entry
                    .engine
                    .accept_mount_receipt(receipt)
                    .expect("exact receipt yields one registration permit");

                let begin = match entry
                    .engine
                    .begin_mount_registration(permit, &foreign_proof)
                {
                    Err(error) => error,
                    Ok(_) => panic!("foreign proof must retain the current registration permit"),
                };
                assert!(begin.validate_rejection(&entry.engine).is_ok());
                let residual = staged.into_residual(world);
                entry.pending_failed_branch = Some(PendingFailedBranch {
                    terminal: PendingFailedBranchTerminal::RegistrationBegin(begin),
                    branch: failed_prepared_branch(pending, Some(residual)),
                });
            },
        );
        (request, router)
    }

    fn prepare_reconciliation_stale_failure_with_active(
        app: &mut App,
    ) -> (NavigationRequestId, RouterHandle) {
        app.add_plugins(BevyUiRouterPlugin::default());
        let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let router = install_nested_router(app, observed, true);
        observe_authority_system(app.world_mut());
        commit_initial_nested_branch(app, router);
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/items/2"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        let (task, pending, host_candidate, top_slot, top_slot_claim) = {
            let mut navigation = app.world_mut().resource_mut::<RouterNavigation>();
            let entry = &mut navigation.entries[0];
            let mut pending = entry.pending_commit.take().unwrap();
            let task = pending
                .final_task
                .take()
                .or_else(|| entry.pending_mount_frame_task.take())
                .expect("reconciliation pending branch must retain its exact issued task");
            let host = entry.host.as_ref().unwrap();
            (
                task,
                pending,
                host.candidate.clone(),
                host.top_slot,
                host.top_slot_claim.clone(),
            )
        };
        let (plan, report) = applied_report_with_identity(task.report_identity());
        app.world_mut().resource_scope(
            |world, mut navigation: bevy_ecs::change_detection::Mut<RouterNavigation>| {
                let entry = &mut navigation.entries[0];
                let result = apply_applied_driver_report_to_branch(
                    world,
                    &mut entry.engine,
                    host_candidate,
                    top_slot,
                    top_slot_claim,
                    None,
                    None,
                    &task,
                    &report,
                    plan,
                );
                assert!(matches!(result, Err(DriverReportApplicationError::Stale)));
                entry.pending_failed_branch = Some(stale_driver_report_owner(pending, task));
            },
        );
        (request, router)
    }

    pub(crate) fn failed_mount_residual_fixture() -> (App, FailedPreparedBranch) {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let router = install_nested_router(&mut app, observed, true);
        observe_authority_system(app.world_mut());
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/items/1"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        app.world_mut()
            .register_component_hooks::<ChildSlotBarrierProbe>()
            .on_insert(|mut world, HookContext { entity, .. }| {
                world
                    .commands()
                    .entity(entity)
                    .remove::<RouterInputBarrier>();
            });
        drive_navigation_system(app.world_mut());
        let branch = app.world_mut().resource_mut::<RouterNavigation>().entries[0]
            .pending_failed_branch
            .take()
            .expect("observer drift retains one failed residual owner")
            .branch;
        (app, branch)
    }

    #[test]
    fn invalid_failed_manifest_topology_is_bounded_unavailable_without_physical_write() {
        let mut app = App::new();
        let (request, router) = prepare_exact_child_stale_failure(&mut app);
        {
            let mut navigation = app.world_mut().resource_mut::<RouterNavigation>();
            navigation.entries[0]
                .pending_failed_branch
                .as_mut()
                .unwrap()
                .branch
                .duplicate_first_child_slot_for_test();
        }
        let (top_slot, failed_owned, focus) = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            let failed = entry.pending_failed_branch.as_ref().unwrap();
            let mut owned = failed
                .branch
                .frames()
                .iter()
                .chain(failed.branch.detached_frames())
                .flat_map(RealizedFrame::claims)
                .map(ExactRegistryClaim::entity)
                .chain(failed.branch.child_slots().iter().map(|slot| slot.slot()))
                .collect::<Vec<_>>();
            owned.sort_by_key(|entity| entity.to_bits());
            owned.dedup();
            (
                entry.host.as_ref().unwrap().top_slot,
                owned,
                failed.branch.frames()[0].shell(),
            )
        };
        *app.world_mut().get_mut::<Visibility>(top_slot).unwrap() = Visibility::Visible;
        *app.world_mut().get_mut::<Pickable>(top_slot).unwrap() = Pickable::default();
        *app.world_mut()
            .get_mut::<RouterInputBarrier>(top_slot)
            .unwrap() = RouterInputBarrier::allow_all();
        *app.world_mut().resource_mut::<InputFocus>() = InputFocus::from_entity(top_slot);
        app.world_mut()
            .resource_mut::<RouteFocusState>()
            .set_focused(Some(focus));
        let registry_before = app
            .world()
            .resource::<RouteEntityRegistry>()
            .records()
            .to_vec();
        let physical_before = app
            .world()
            .iter_entities()
            .map(|entity| {
                (
                    entity.id(),
                    entity.archetype().components().to_vec(),
                    app.world().get::<ChildOf>(entity.id()).map(ChildOf::parent),
                    app.world()
                        .get::<Children>(entity.id())
                        .map(|children| children.iter().copied().collect::<Vec<_>>()),
                )
            })
            .collect::<Vec<_>>();

        settle_cleanup_system(app.world_mut());

        {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            assert!(entry.pending_failed_branch.is_none());
            assert!(entry.cleanup_ledger.is_none());
            assert!(entry.active.is_none());
            assert!(entry.host.is_none());
            let residual = entry.authority_protocol_residual.as_ref().unwrap();
            assert!(matches!(
                residual.kind,
                AuthorityProtocolResidualKind::FailedManifest(
                    FailedPreparedTopologyError::DuplicateEntity
                )
            ));
            assert_eq!(
                residual.failed.as_ref().unwrap().branch.child_slots().len(),
                2
            );
            assert!(residual.host.is_some());
            assert_eq!(
                entry.engine.snapshot().availability(),
                &RouterAvailability::Unavailable(
                    RouterUnavailableReason::RegistryInvariantViolation
                )
            );
            assert!(matches!(
                entry.engine.request(request).unwrap().status(),
                RequestStatus::Failed(failure)
                    if failure.kind() == NavigationFailureKind::AuthorityLost
            ));
        }
        assert_eq!(
            app.world().resource::<RouteEntityRegistry>().records(),
            registry_before
        );
        let physical_after = app
            .world()
            .iter_entities()
            .map(|entity| {
                (
                    entity.id(),
                    entity.archetype().components().to_vec(),
                    app.world().get::<ChildOf>(entity.id()).map(ChildOf::parent),
                    app.world()
                        .get::<Children>(entity.id())
                        .map(|children| children.iter().copied().collect::<Vec<_>>()),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(physical_after, physical_before);
        assert_eq!(app.world().resource::<InputFocus>().get(), None);
        assert_eq!(app.world().resource::<RouteFocusState>().focused(), None);
        assert_eq!(
            app.world().get::<Pickable>(top_slot),
            Some(&Pickable::IGNORE)
        );
        assert!(
            app.world()
                .get::<RouterInputBarrier>(top_slot)
                .is_some_and(RouterInputBarrier::denies_all)
        );
        let gate = app.world().resource::<RouterInputGate>();
        assert!(failed_owned.iter().all(|entity| {
            app.world().get::<Visibility>(*entity) == Some(&Visibility::Hidden)
                && app.world().get::<Pickable>(*entity) == Some(&Pickable::IGNORE)
                && app
                    .world()
                    .get::<RouterInputBarrier>(*entity)
                    .is_some_and(RouterInputBarrier::denies_all)
                && !gate.allows_pointer(*entity)
                && !gate.allows_keyboard(*entity)
                && !gate.allows_gamepad(*entity)
        }));

        settle_cleanup_system(app.world_mut());
        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert!(entry.authority_protocol_residual.is_some());
        assert!(entry.pending_failed_branch.is_none());
        assert!(entry.cleanup_ledger.is_none());
        assert_eq!(
            entry
                .engine
                .request(request)
                .unwrap()
                .trace()
                .iter()
                .filter(|event| event.cleanup_disposition().is_some())
                .count(),
            0
        );
        assert_eq!(
            app.world_mut()
                .resource_mut::<RouterNavigation>()
                .push(router, RouteLocation::new("/blocked"))
                .unwrap_err()
                .kind(),
            NavigationEnqueueErrorKind::HostUnavailable
        );
    }

    #[test]
    fn invalid_failed_manifest_topology_isolates_retired_top_and_active_owner() {
        let mut app = App::new();
        let (request, _router) = prepare_exact_child_stale_failure(&mut app);
        {
            let mut navigation = app.world_mut().resource_mut::<RouterNavigation>();
            let entry = &mut navigation.entries[0];
            entry
                .pending_failed_branch
                .as_mut()
                .unwrap()
                .branch
                .duplicate_first_child_slot_for_test();
            let PendingFailedBranch { terminal, branch } =
                entry.pending_failed_branch.take().unwrap();
            let mut realized = branch.into_realized_for_test();
            let active = RealizedBranch::from_single_frame(
                realized
                    .pop_frame_for_test()
                    .expect("stored owner supplies one exact active frame"),
            );
            entry.active = Some(active);
            entry.pending_failed_branch = Some(PendingFailedBranch {
                terminal,
                branch: FailedPreparedBranch::new(PreparedBranch::from_realized(realized)),
            });
        }
        let (top_slot, active_targets) = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            (
                entry.host.as_ref().unwrap().top_slot,
                entry.active.as_ref().unwrap().interactive_entities(),
            )
        };
        for target in active_targets
            .iter()
            .copied()
            .chain(std::iter::once(top_slot))
        {
            *app.world_mut().get_mut::<Visibility>(target).unwrap() = Visibility::Visible;
            *app.world_mut().get_mut::<Pickable>(target).unwrap() = Pickable::default();
            *app.world_mut()
                .get_mut::<RouterInputBarrier>(target)
                .unwrap() = RouterInputBarrier::allow_all();
        }
        *app.world_mut().resource_mut::<InputFocus>() = InputFocus::from_entity(top_slot);
        app.world_mut()
            .resource_mut::<RouteFocusState>()
            .set_focused(active_targets.first().copied());
        let registry_before = app
            .world()
            .resource::<RouteEntityRegistry>()
            .records()
            .to_vec();
        let structure_before = app
            .world()
            .iter_entities()
            .map(|entity| {
                (
                    entity.id(),
                    entity.archetype().components().to_vec(),
                    app.world().get::<ChildOf>(entity.id()).map(ChildOf::parent),
                    app.world()
                        .get::<Children>(entity.id())
                        .map(|children| children.iter().copied().collect::<Vec<_>>()),
                )
            })
            .collect::<Vec<_>>();

        settle_cleanup_system(app.world_mut());

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert!(entry.active.is_none());
        assert!(entry.host.is_none());
        assert!(
            matches!(
                entry.engine.request(request).unwrap().status(),
                RequestStatus::Failed(failure)
                    if failure.kind() == NavigationFailureKind::AuthorityLost
            ),
            "status={:?} ledger={:?}",
            entry.engine.request(request).unwrap().status(),
            entry
                .cleanup_ledger
                .as_ref()
                .map(|ledger| (ledger.receipt_count(), ledger.is_settled()))
        );
        let residual = entry.authority_protocol_residual.as_ref().unwrap();
        assert!(residual.active.is_some());
        assert!(matches!(
            residual.kind,
            AuthorityProtocolResidualKind::FailedManifest(
                FailedPreparedTopologyError::DuplicateEntity
            )
        ));
        assert_eq!(
            app.world().resource::<RouteEntityRegistry>().records(),
            registry_before
        );
        let structure_after = app
            .world()
            .iter_entities()
            .map(|entity| {
                (
                    entity.id(),
                    entity.archetype().components().to_vec(),
                    app.world().get::<ChildOf>(entity.id()).map(ChildOf::parent),
                    app.world()
                        .get::<Children>(entity.id())
                        .map(|children| children.iter().copied().collect::<Vec<_>>()),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(structure_after, structure_before);
        assert_eq!(app.world().resource::<InputFocus>().get(), None);
        assert_eq!(app.world().resource::<RouteFocusState>().focused(), None);
        let gate = app.world().resource::<RouterInputGate>();
        assert!(
            active_targets
                .iter()
                .copied()
                .chain(std::iter::once(top_slot))
                .all(|target| {
                    app.world().get::<Visibility>(target) == Some(&Visibility::Hidden)
                        && app.world().get::<Pickable>(target) == Some(&Pickable::IGNORE)
                        && app
                            .world()
                            .get::<RouterInputBarrier>(target)
                            .is_some_and(RouterInputBarrier::denies_all)
                        && !gate.allows_pointer(target)
                        && !gate.allows_keyboard(target)
                        && !gate.allows_gamepad(target)
                })
        );
    }

    #[test]
    fn nested_pending_clean_removal_releases_child_slot_focus() {
        let mut app = App::new();
        let (_request, _router) = prepare_exact_child_stale_failure(&mut app);
        let child_slot = app.world().resource::<RouterNavigation>().entries[0]
            .pending_failed_branch
            .as_ref()
            .unwrap()
            .branch
            .child_slots()[0]
            .slot();
        *app.world_mut().resource_mut::<InputFocus>() = InputFocus::from_entity(child_slot);
        app.world_mut()
            .resource_mut::<RouteFocusState>()
            .set_focused(Some(child_slot));

        settle_cleanup_system(app.world_mut());

        assert_eq!(app.world().resource::<InputFocus>().get(), None);
        assert_eq!(app.world().resource::<RouteFocusState>().focused(), None);
        assert!(app.world().get_entity(child_slot).is_err());
    }

    #[test]
    fn stale_current_host_parent_settles_without_mutating_app_relationships() {
        let mut app = App::new();
        let (request, _router) = prepare_exact_child_stale_failure(&mut app);
        let (
            frame_shell,
            top_slot_claim,
            host,
            top_slot,
            host_children_before,
            host_manifest_before,
            history_before,
        ) = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            let host = entry.host.as_ref().unwrap();
            (
                entry
                    .pending_failed_branch
                    .as_ref()
                    .unwrap()
                    .branch
                    .frames()[0]
                    .shell(),
                host.top_slot_claim.clone(),
                host.host,
                host.top_slot,
                app.world()
                    .get::<Children>(host.host)
                    .map(|children| children.iter().copied().collect::<Vec<_>>())
                    .unwrap_or_default(),
                ComponentManifest::from_world(app.world(), host.host).unwrap(),
                entry.engine.history().clone(),
            )
        };
        let foreign = app.world_mut().spawn(ChildOf(frame_shell)).id();
        let foreign_manifest_before = ComponentManifest::from_world(app.world(), foreign).unwrap();
        *app.world_mut().resource_mut::<InputFocus>() = InputFocus::from_entity(foreign);
        app.world_mut()
            .resource_mut::<RouteFocusState>()
            .set_focused(Some(foreign));
        app.world_mut()
            .resource_mut::<RouteEntityRegistry>()
            .compare_exchange_lifecycle(&top_slot_claim, EntityLifecycle::Quarantined)
            .unwrap();
        app.world_mut().resource_scope(
            |world, mut navigation: bevy_ecs::change_detection::Mut<RouterNavigation>| {
                let entry = &mut navigation.entries[0];
                let failed = entry.pending_failed_branch.take().unwrap();
                let manifest = FrozenCleanupManifest::from_failed_prepared_branch(
                    world,
                    world.resource::<RouteEntityRegistry>(),
                    &failed.branch,
                )
                .unwrap();
                match classify_failed_manifest(
                    manifest,
                    failed,
                    entry.active.as_ref(),
                    entry.host.as_ref(),
                ) {
                    FailedManifestDisposition::CurrentAuthorityLoss {
                        failed, bindings, ..
                    } => {
                        assert!(bindings.iter().any(|binding| {
                            matches!(
                                binding,
                                FailedAuthorityBoundaryBinding::Current { authority, .. }
                                    if authority == &top_slot_claim
                            )
                        }));
                        entry.pending_failed_branch = Some(failed);
                    }
                    FailedManifestDisposition::Ready { failed, .. }
                    | FailedManifestDisposition::AmbiguousInvalidTopology { failed, .. } => {
                        entry.pending_failed_branch = Some(failed);
                        panic!("stale HostTopSlot claim must classify as current authority loss")
                    }
                }
            },
        );

        settle_cleanup_system(app.world_mut());

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert!(entry.pending_failed_branch.is_none());
        assert!(
            entry
                .cleanup_ledger
                .as_ref()
                .is_some_and(CleanupLedger::is_authority_settlement)
        );
        assert!(matches!(
            entry.engine.request(request).unwrap().status(),
            RequestStatus::Running(NavigationPhase::Recovering)
        ));
        assert_eq!(app.world().resource::<InputFocus>().get(), None);
        assert_eq!(app.world().resource::<RouteFocusState>().focused(), None);
        assert_eq!(
            app.world().get::<Visibility>(frame_shell),
            Some(&Visibility::Hidden)
        );
        assert_eq!(
            app.world().get::<Pickable>(frame_shell),
            Some(&Pickable::IGNORE)
        );
        assert!(
            app.world()
                .get::<RouterInputBarrier>(frame_shell)
                .is_some_and(RouterInputBarrier::denies_all)
        );
        assert!(app.world().get_entity(host).is_ok());
        assert!(app.world().get_entity(foreign).is_ok());
        assert_eq!(
            ComponentManifest::from_world(app.world(), host).as_ref(),
            Some(&host_manifest_before)
        );
        assert_eq!(
            ComponentManifest::from_world(app.world(), foreign).as_ref(),
            Some(&foreign_manifest_before)
        );
        assert_eq!(
            app.world().get::<ChildOf>(top_slot).map(ChildOf::parent),
            Some(host)
        );
        assert_eq!(
            app.world()
                .get::<Children>(host)
                .map(|children| children.iter().copied().collect::<Vec<_>>())
                .unwrap_or_default(),
            host_children_before
        );
        assert_eq!(
            app.world().get::<ChildOf>(foreign).map(ChildOf::parent),
            Some(frame_shell)
        );

        for _ in 0..8 {
            settle_cleanup_system(app.world_mut());
            if app.world().resource::<RouterNavigation>().entries[0]
                .cleanup_ledger
                .is_none()
            {
                break;
            }
        }

        let trace_len = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            assert!(matches!(
                entry.engine.request(request).unwrap().status(),
                RequestStatus::Failed(failure)
                    if failure.kind() == NavigationFailureKind::AuthorityLost
            ));
            assert!(entry.cleanup_ledger.is_none());
            assert!(entry.authority_protocol_residual.is_none());
            assert_eq!(entry.engine.history(), &history_before);
            assert!(app.world().get_entity(host).is_ok());
            assert!(app.world().get_entity(foreign).is_ok());
            assert_eq!(
                ComponentManifest::from_world(app.world(), host).as_ref(),
                Some(&host_manifest_before)
            );
            assert_eq!(
                ComponentManifest::from_world(app.world(), foreign).as_ref(),
                Some(&foreign_manifest_before)
            );
            assert_eq!(
                app.world().get::<ChildOf>(top_slot).map(ChildOf::parent),
                Some(host)
            );
            assert_eq!(
                app.world()
                    .get::<Children>(host)
                    .map(|children| children.iter().copied().collect::<Vec<_>>())
                    .unwrap_or_default(),
                host_children_before
            );
            assert_eq!(
                app.world().get::<ChildOf>(foreign).map(ChildOf::parent),
                Some(frame_shell)
            );
            entry.engine.request(request).unwrap().trace().len()
        };

        settle_cleanup_system(app.world_mut());

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert_eq!(
            entry.engine.request(request).unwrap().trace().len(),
            trace_len
        );
        assert_eq!(entry.engine.history(), &history_before);
        assert!(entry.cleanup_ledger.is_none());
    }

    #[test]
    fn stale_failed_branch_root_preserves_app_descendant_focus_without_a_ledger() {
        let mut app = App::new();
        let (request, _router) = prepare_exact_child_stale_failure(&mut app);
        let (root, claim) = {
            let failed = app.world().resource::<RouterNavigation>().entries[0]
                .pending_failed_branch
                .as_ref()
                .unwrap();
            let root = failed.branch.frames()[0].shell();
            let claim = app
                .world()
                .resource::<RouteEntityRegistry>()
                .record(root)
                .unwrap()
                .claim();
            (root, claim)
        };
        let app_descendant = app.world_mut().spawn(ChildOf(root)).id();
        *app.world_mut().resource_mut::<InputFocus>() = InputFocus::from_entity(app_descendant);
        app.world_mut()
            .resource_mut::<RouteFocusState>()
            .set_focused(Some(app_descendant));
        app.world_mut()
            .resource_mut::<RouteEntityRegistry>()
            .compare_exchange_lifecycle(&claim, EntityLifecycle::Quarantined)
            .unwrap();
        let root_before = nested_protected_fingerprint(app.world(), root);
        let replacement_before = app
            .world()
            .resource::<RouteEntityRegistry>()
            .record(root)
            .map(|record| record.claim());
        let failed = app.world_mut().resource_mut::<RouterNavigation>().entries[0]
            .pending_failed_branch
            .take()
            .unwrap();
        assert!(!isolate_exact_failed_prepared_branch(
            app.world_mut(),
            &failed.branch,
            None,
        ));
        app.world_mut().resource_mut::<RouterNavigation>().entries[0].pending_failed_branch =
            Some(failed);
        assert_eq!(nested_protected_fingerprint(app.world(), root), root_before);
        assert_eq!(
            app.world()
                .resource::<RouteEntityRegistry>()
                .record(root)
                .map(|record| record.claim()),
            replacement_before
        );
        assert_eq!(
            app.world().resource::<InputFocus>().get(),
            Some(app_descendant)
        );
        assert_eq!(
            app.world().resource::<RouteFocusState>().focused(),
            Some(app_descendant)
        );

        settle_cleanup_system(app.world_mut());

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert!(entry.pending_failed_branch.is_some());
        assert!(entry.cleanup_ledger.is_none());
        assert!(matches!(
            entry.engine.request(request).unwrap().status(),
            RequestStatus::Running(_)
        ));
        assert_eq!(
            app.world().resource::<InputFocus>().get(),
            Some(app_descendant)
        );
        assert_eq!(
            app.world().resource::<RouteFocusState>().focused(),
            Some(app_descendant)
        );
    }

    #[test]
    fn pending_only_missing_and_replaced_boundaries_use_ordinary_failed_cleanup() {
        for replaced in [false, true] {
            let mut app = App::new();
            let (request, _router) = prepare_exact_child_stale_failure(&mut app);
            let (boundary, host_version, history_before) = {
                let mut navigation = app.world_mut().resource_mut::<RouterNavigation>();
                let entry = &mut navigation.entries[0];
                let mut failed = entry.pending_failed_branch.take().unwrap();
                let mut realized = failed.branch.into_realized_for_test();
                let boundary = realized.child_slots()[0].slot_claim().clone();
                realized.clear_child_slots_for_test();
                failed.branch = FailedPreparedBranch::new(PreparedBranch::from_realized(realized));
                entry.pending_failed_branch = Some(failed);
                let host = entry.host.as_ref().unwrap();
                (
                    boundary,
                    host.version.clone(),
                    entry.engine.history().clone(),
                )
            };
            let foreign_focus = app.world_mut().spawn_empty().id();
            *app.world_mut().resource_mut::<InputFocus>() = InputFocus::from_entity(foreign_focus);
            app.world_mut()
                .resource_mut::<RouteFocusState>()
                .set_focused(Some(foreign_focus));
            if replaced {
                app.world_mut()
                    .resource_mut::<RouteEntityRegistry>()
                    .compare_exchange_lifecycle(&boundary, EntityLifecycle::Quarantined)
                    .unwrap();
            } else {
                assert!(
                    app.world_mut()
                        .resource_mut::<RouteEntityRegistry>()
                        .remove_exact(&boundary)
                );
            }

            settle_cleanup_system(app.world_mut());

            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            assert!(entry.pending_failed_branch.is_none());
            assert!(entry.cleanup_ledger.is_none());
            assert!(entry.authority_protocol_residual.is_none());
            assert_eq!(entry.host.as_ref().unwrap().version, host_version);
            assert!(entry.active.is_none());
            assert_eq!(entry.engine.history(), &history_before);
            assert!(matches!(
                entry.engine.request(request).unwrap().status(),
                RequestStatus::Failed(_)
            ));
            assert_ne!(
                entry.engine.snapshot().availability(),
                &RouterAvailability::RecoveringAuthority
            );
            assert_eq!(
                app.world().resource::<InputFocus>().get(),
                Some(foreign_focus)
            );
            assert_eq!(
                app.world().resource::<RouteFocusState>().focused(),
                Some(foreign_focus)
            );
        }
    }

    #[test]
    fn active_pending_only_boundary_is_built_and_settled_without_touching_active() {
        for replaced in [false, true] {
            let mut app = App::new();
            app.add_plugins(BevyUiRouterPlugin::default());
            let router = install_failing_deep_pending_router(&mut app);
            observe_authority_system(app.world_mut());

            app.world_mut()
                .resource_mut::<RouterNavigation>()
                .push(router, RouteLocation::new("/"))
                .unwrap();
            collect_navigation_system(app.world_mut());
            for _ in 0..5 {
                drive_navigation_system(app.world_mut());
            }
            commit_presentation_system(app.world_mut());
            settle_cleanup_system(app.world_mut());
            assert!(
                app.world().resource::<RouterNavigation>().entries[0]
                    .active
                    .is_some()
            );

            let request = app
                .world_mut()
                .resource_mut::<RouterNavigation>()
                .push(router, RouteLocation::new("/sections/a/items/1/detail"))
                .unwrap();
            collect_navigation_system(app.world_mut());
            for _ in 0..10 {
                drive_navigation_system(app.world_mut());
                if app.world().resource::<RouterNavigation>().entries[0]
                    .pending_failed_branch
                    .is_some()
                {
                    break;
                }
            }

            let (slot_boundary, physical_parent_boundary) = {
                let mut navigation = app.world_mut().resource_mut::<RouterNavigation>();
                let entry = &mut navigation.entries[0];
                let PendingFailedBranch { terminal, branch } = entry
                    .pending_failed_branch
                    .take()
                    .expect("driver failure owner");
                let mut realized = branch.into_realized_for_test();
                assert!(realized.frames().len() >= 2);
                assert!(!realized.child_slots().is_empty());
                let failed_frames = realized
                    .frames()
                    .iter()
                    .map(|frame| frame.frame().clone())
                    .collect::<Vec<_>>();
                let boundary_frame = realized
                    .frames()
                    .iter()
                    .find(|frame| {
                        let claim = frame.mount_binding().slot_claim();
                        matches!(
                            claim.owner(),
                            RegistryOwner::Committed(EntityOwner::ChildSlot { parent, .. })
                                if failed_frames.iter().any(|frame| frame == parent)
                        )
                    })
                    .expect("failed-only child frame retains its internal mount boundary");
                let slot_boundary = boundary_frame.mount_binding().slot_claim().clone();
                let physical_parent_boundary = boundary_frame
                    .mount_binding()
                    .physical_parent()
                    .1
                    .expect("failed-only child frame has an owned physical parent")
                    .clone();
                realized.clear_child_slots_for_test();
                entry.pending_failed_branch = Some(PendingFailedBranch {
                    terminal,
                    branch: FailedPreparedBranch::new(PreparedBranch::from_realized(realized)),
                });
                (slot_boundary, physical_parent_boundary)
            };
            for boundary in [&slot_boundary, &physical_parent_boundary] {
                if replaced {
                    app.world_mut()
                        .resource_mut::<RouteEntityRegistry>()
                        .compare_exchange_lifecycle(boundary, EntityLifecycle::Quarantined)
                        .unwrap();
                } else {
                    assert!(
                        app.world_mut()
                            .resource_mut::<RouteEntityRegistry>()
                            .remove_exact(boundary)
                    );
                }
            }

            let (
                active_claims,
                active_fingerprint,
                active_focus,
                host_version,
                history_before,
                lost_boundary_fingerprint,
                lost_boundary_registry,
            ) = {
                let entry = &app.world().resource::<RouterNavigation>().entries[0];
                let active = entry.active.as_ref().unwrap();
                let active_claims = active.claims().cloned().collect::<Vec<_>>();
                let active_fingerprint = active_claims
                    .iter()
                    .map(|claim| {
                        (
                            claim.entity(),
                            nested_protected_fingerprint(app.world(), claim.entity()),
                        )
                    })
                    .collect::<Vec<_>>();
                let active_focus = *active.interactive_entities().first().unwrap();
                let failed = entry.pending_failed_branch.as_ref().unwrap();
                let manifest = FrozenCleanupManifest::from_failed_prepared_branch(
                    app.world(),
                    app.world().resource::<RouteEntityRegistry>(),
                    &failed.branch,
                )
                .unwrap();
                assert!(manifest.boundary_drifts().iter().any(|drift| {
                    drift.expected.exact_claim() == &slot_boundary
                        && matches!(drift.source, FailedBoundarySource::FrameMountSlot { .. })
                }));
                assert!(manifest.boundary_drifts().iter().any(|drift| {
                    drift.expected.exact_claim() == &physical_parent_boundary
                        && matches!(
                            drift.source,
                            FailedBoundarySource::FramePhysicalParent { .. }
                        )
                }));
                for boundary in [&slot_boundary, &physical_parent_boundary] {
                    let drift = manifest
                        .boundary_drifts()
                        .iter()
                        .find(|drift| drift.expected.exact_claim() == boundary)
                        .expect("both stable boundary facts are frozen");
                    assert!(matches!(
                        classify_failed_boundary(
                            drift.clone(),
                            &failed.branch,
                            entry.active.as_ref(),
                            entry.host.as_ref(),
                        ),
                        FailedBoundaryDisposition::PendingOnlyDrift(_)
                    ));
                }
                (
                    active_claims,
                    active_fingerprint,
                    active_focus,
                    entry.host.as_ref().unwrap().version.clone(),
                    entry.engine.history().clone(),
                    nested_protected_fingerprint(app.world(), physical_parent_boundary.entity()),
                    app.world()
                        .resource::<RouteEntityRegistry>()
                        .record(physical_parent_boundary.entity())
                        .map(|record| record.claim()),
                )
            };
            *app.world_mut().resource_mut::<InputFocus>() = InputFocus::from_entity(active_focus);
            app.world_mut()
                .resource_mut::<RouteFocusState>()
                .set_focused(Some(active_focus));
            let gate_before = {
                let gate = app.world().resource::<RouterInputGate>();
                (
                    gate.allows_pointer(active_focus),
                    gate.allows_keyboard(active_focus),
                    gate.allows_gamepad(active_focus),
                )
            };

            settle_cleanup_system(app.world_mut());

            {
                let entry = &app.world().resource::<RouterNavigation>().entries[0];
                assert!(entry.pending_failed_branch.is_none());
                let ledger = entry
                    .cleanup_ledger
                    .as_ref()
                    .expect("ordinary cleanup retains its partial receipt owner");
                assert!(ledger.receipt_count() > 0);
                assert!(!ledger.is_settled());
                assert!(matches!(
                    entry.engine.request(request).unwrap().status(),
                    RequestStatus::Running(_)
                ));
            }
            for (entity, before) in &active_fingerprint {
                assert_eq!(&nested_protected_fingerprint(app.world(), *entity), before);
            }
            assert_eq!(
                nested_protected_fingerprint(app.world(), physical_parent_boundary.entity(),),
                lost_boundary_fingerprint
            );
            assert_eq!(
                app.world()
                    .resource::<RouteEntityRegistry>()
                    .record(physical_parent_boundary.entity())
                    .map(|record| record.claim()),
                lost_boundary_registry
            );
            assert_eq!(
                app.world().resource::<InputFocus>().get(),
                Some(active_focus)
            );
            assert_eq!(
                app.world().resource::<RouteFocusState>().focused(),
                Some(active_focus)
            );

            settle_cleanup_system(app.world_mut());

            let trace_len = {
                let entry = &app.world().resource::<RouterNavigation>().entries[0];
                assert!(entry.pending_failed_branch.is_none());
                assert!(entry.cleanup_ledger.is_none());
                assert!(entry.authority_protocol_residual.is_none());
                assert_eq!(entry.host.as_ref().unwrap().version, host_version);
                assert_eq!(entry.engine.history(), &history_before);
                assert_eq!(
                    entry.active.as_ref().unwrap().claims().collect::<Vec<_>>(),
                    active_claims.iter().collect::<Vec<_>>()
                );
                assert!(matches!(
                    entry.engine.request(request).unwrap().status(),
                    RequestStatus::Failed(_)
                ));
                assert_ne!(
                    entry.engine.snapshot().availability(),
                    &RouterAvailability::RecoveringAuthority
                );
                entry.engine.request(request).unwrap().trace().len()
            };
            for (entity, before) in &active_fingerprint {
                assert_eq!(&nested_protected_fingerprint(app.world(), *entity), before);
            }
            assert_eq!(
                nested_protected_fingerprint(app.world(), physical_parent_boundary.entity(),),
                lost_boundary_fingerprint
            );
            assert_eq!(
                app.world()
                    .resource::<RouteEntityRegistry>()
                    .record(physical_parent_boundary.entity())
                    .map(|record| record.claim()),
                lost_boundary_registry
            );
            let gate = app.world().resource::<RouterInputGate>();
            assert_eq!(
                (
                    gate.allows_pointer(active_focus),
                    gate.allows_keyboard(active_focus),
                    gate.allows_gamepad(active_focus),
                ),
                gate_before
            );
            assert_eq!(
                app.world().resource::<InputFocus>().get(),
                Some(active_focus)
            );
            assert_eq!(
                app.world().resource::<RouteFocusState>().focused(),
                Some(active_focus)
            );

            settle_cleanup_system(app.world_mut());
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            assert_eq!(
                entry.engine.request(request).unwrap().trace().len(),
                trace_len
            );
            assert!(entry.pending_failed_branch.is_none());
            assert!(entry.cleanup_ledger.is_none());
            assert_eq!(
                nested_protected_fingerprint(app.world(), physical_parent_boundary.entity(),),
                lost_boundary_fingerprint
            );
            assert_eq!(
                app.world()
                    .resource::<RouteEntityRegistry>()
                    .record(physical_parent_boundary.entity())
                    .map(|record| record.claim()),
                lost_boundary_registry
            );
        }
    }

    #[test]
    fn active_ambiguous_residual_boundary_is_built_and_fails_closed_once() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let router = install_failing_deep_pending_router(&mut app);
        observe_authority_system(app.world_mut());
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        for _ in 0..5 {
            drive_navigation_system(app.world_mut());
        }
        commit_presentation_system(app.world_mut());
        settle_cleanup_system(app.world_mut());

        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/sections/a/items/1/detail"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        for _ in 0..10 {
            drive_navigation_system(app.world_mut());
            if app.world().resource::<RouterNavigation>().entries[0]
                .pending_failed_branch
                .is_some()
            {
                break;
            }
        }

        let (mut failed, active_frame, active_focus) = {
            let mut navigation = app.world_mut().resource_mut::<RouterNavigation>();
            let entry = &mut navigation.entries[0];
            let active = entry.active.as_ref().unwrap();
            let active_frame = active.frames()[0].frame().clone();
            let active_focus = *active.interactive_entities().first().unwrap();
            (
                entry.pending_failed_branch.take().unwrap(),
                active_frame,
                active_focus,
            )
        };
        let source_slot = failed
            .branch
            .child_slots()
            .iter()
            .find(|slot| slot.parent() != &active_frame)
            .expect("real failed staging retains one failed-parent internal slot");
        let source_parent = source_slot.parent().clone();
        let failed_parent = failed
            .branch
            .frames()
            .iter()
            .find(|frame| frame.frame() == &source_parent)
            .unwrap();
        let exact_outlet_claim = failed_parent
            .outlet(source_slot.outlet())
            .unwrap()
            .anchor_claim();
        let wrong_boundary = failed_parent
            .claims()
            .find(|claim| {
                claim != &exact_outlet_claim
                    && matches!(
                        claim.owner(),
                        RegistryOwner::Committed(EntityOwner::FrameOutput { frame, .. })
                            if frame == &source_parent
                    )
            })
            .expect("failed parent retains a second exact FrameOutput")
            .clone();
        let residual = ChildSlotResidual::from_mounted_with_boundary_for_test(
            app.world(),
            source_slot,
            wrong_boundary.entity(),
            wrong_boundary.clone(),
        );
        let terminal = failed.terminal;
        let mut realized = failed.branch.into_realized_for_test();
        while realized.frames().len() > 1 {
            realized.pop_frame_for_test().unwrap();
        }
        assert_eq!(realized.frames()[0].frame(), &source_parent);
        realized.clear_child_slots_for_test();
        failed = PendingFailedBranch {
            terminal,
            branch: FailedPreparedBranch::with_mount_residual(
                PreparedBranch::from_realized(realized),
                residual,
            ),
        };
        assert!(
            app.world_mut()
                .resource_mut::<RouteEntityRegistry>()
                .remove_exact(&wrong_boundary)
        );
        {
            let manifest = FrozenCleanupManifest::from_failed_prepared_branch(
                app.world(),
                app.world().resource::<RouteEntityRegistry>(),
                &failed.branch,
            )
            .unwrap();
            assert_eq!(manifest.boundary_drifts().len(), 1);
            assert!(matches!(
                manifest.boundary_drifts()[0].source,
                FailedBoundarySource::ResidualAnchor { .. }
            ));
            assert_eq!(
                manifest.boundary_drifts()[0].expected.exact_claim(),
                &wrong_boundary
            );
        }
        app.world_mut().resource_mut::<RouterNavigation>().entries[0].pending_failed_branch =
            Some(failed);
        *app.world_mut().resource_mut::<InputFocus>() = InputFocus::from_entity(active_focus);
        app.world_mut()
            .resource_mut::<RouteFocusState>()
            .set_focused(Some(active_focus));

        settle_cleanup_system(app.world_mut());

        let trace_len = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            let residual = entry.authority_protocol_residual.as_ref().unwrap();
            assert!(matches!(
                residual.kind,
                AuthorityProtocolResidualKind::FailedManifest(
                    FailedPreparedTopologyError::AmbiguousBoundaryOwnership
                )
            ));
            assert!(residual.failed.is_some());
            assert!(residual.active.is_some());
            assert!(residual.host.is_some());
            assert!(entry.pending_failed_branch.is_none());
            assert!(entry.active.is_none());
            assert!(entry.host.is_none());
            assert!(entry.cleanup_ledger.is_none());
            assert!(entry.pending_commit.is_none());
            assert!(entry.pending_frame_drive.is_none());
            assert!(entry.pending_mount_frame_task.is_none());
            assert!(entry.pending_mount_request.is_none());
            entry.engine.request(request).unwrap().trace().len()
        };

        settle_cleanup_system(app.world_mut());

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert_eq!(
            entry.engine.request(request).unwrap().trace().len(),
            trace_len
        );
        assert!(entry.authority_protocol_residual.is_some());
        assert!(entry.cleanup_ledger.is_none());
        assert!(entry.pending_failed_branch.is_none());
        assert!(entry.pending_commit.is_none());
    }

    #[test]
    pub(crate) fn failed_branch_relationship_cycle_quarantines_exact_set_and_releases_focus() {
        reconciliation_entering_cycle_is_rejected_without_retiring_active_branch();
    }

    #[test]
    fn nested_pending_isolation_observer_drift_retains_inert_owner_for_retry() {
        let mut app = App::new();
        app.world_mut()
            .register_component_hooks::<Pickable>()
            .on_insert(|mut world, HookContext { entity, .. }| {
                let remove = world
                    .get_resource::<PersistentOnePickableRemoval>()
                    .is_some_and(|drift| drift.target == Some(entity));
                if remove {
                    world.commands().entity(entity).remove::<Pickable>();
                }
            });
        let (request, _router) = prepare_exact_child_stale_failure(&mut app);
        let target = app.world().resource::<RouterNavigation>().entries[0]
            .pending_failed_branch
            .as_ref()
            .unwrap()
            .branch
            .frames()[0]
            .shell();
        app.world_mut().entity_mut(target).remove::<Pickable>();
        app.world_mut()
            .insert_resource(PersistentOnePickableRemoval {
                remaining_before_target: 0,
                target: Some(target),
            });

        for _ in 0..2 {
            settle_cleanup_system(app.world_mut());
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            assert!(entry.pending_failed_branch.is_some());
            assert!(entry.cleanup_ledger.is_none());
            assert!(matches!(
                entry.engine.request(request).unwrap().status(),
                RequestStatus::Running(_)
            ));
            assert_eq!(
                app.world().get::<Visibility>(target),
                Some(&Visibility::Hidden)
            );
            assert!(
                app.world()
                    .get::<RouterInputBarrier>(target)
                    .is_some_and(RouterInputBarrier::denies_all)
            );
            assert!(
                app.world()
                    .get::<Pickable>(target)
                    .is_none_or(|pickable| *pickable == Pickable::IGNORE)
            );
        }
    }

    #[test]
    fn post_despawn_registry_claim_cas_cannot_mint_a_cleanup_receipt() {
        let mut app = App::new();
        install_cleanup_despawn_drift_hook(&mut app);
        let (request, _router) = prepare_exact_child_stale_failure(&mut app);
        let (target, claim) = {
            let failed = app.world().resource::<RouterNavigation>().entries[0]
                .pending_failed_branch
                .as_ref()
                .unwrap();
            let manifest = FrozenCleanupManifest::from_failed_prepared_branch(
                app.world(),
                app.world().resource::<RouteEntityRegistry>(),
                &failed.branch,
            )
            .unwrap();
            let target = manifest.entities()[0];
            let claim = app
                .world()
                .resource::<RouteEntityRegistry>()
                .record(target)
                .unwrap()
                .claim();
            (target, claim)
        };
        app.world_mut().insert_resource(CleanupDespawnDrift {
            target,
            action: CleanupDespawnDriftAction::TargetRegistryClaim(claim),
        });

        settle_cleanup_system(app.world_mut());

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        let ledger = entry
            .cleanup_ledger
            .as_ref()
            .expect("failed receipt CAS must retain the activated owner");
        assert_eq!(ledger.receipt_count(), 0);
        assert!(!ledger.is_settled());
        assert!(!ledger.failed_prepared_frames().is_empty());
        assert!(matches!(
            entry.engine.request(request).unwrap().status(),
            RequestStatus::Running(_)
        ));
        assert!(app.world().get_entity(target).is_err());
        assert_eq!(
            app.world()
                .resource::<RouteEntityRegistry>()
                .record(target)
                .unwrap()
                .lifecycle(),
            EntityLifecycle::Quarantined
        );
    }

    #[test]
    fn post_despawn_external_parent_drift_stops_before_the_next_obligation() {
        let mut app = App::new();
        install_cleanup_despawn_drift_hook(&mut app);
        let (_request, _router) = prepare_exact_child_stale_failure(&mut app);
        let (first, later, top_slot) = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            let failed = entry.pending_failed_branch.as_ref().unwrap();
            let manifest = FrozenCleanupManifest::from_failed_prepared_branch(
                app.world(),
                app.world().resource::<RouteEntityRegistry>(),
                &failed.branch,
            )
            .unwrap();
            let entities = manifest.entities();
            (
                entities[0],
                entities[1],
                entry.host.as_ref().unwrap().top_slot,
            )
        };
        app.world_mut().insert_resource(CleanupDespawnDrift {
            target: first,
            action: CleanupDespawnDriftAction::ExternalParentSeal(top_slot),
        });

        settle_cleanup_system(app.world_mut());

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        let ledger = entry.cleanup_ledger.as_ref().unwrap();
        assert_eq!(ledger.receipt_count(), 1);
        assert!(!ledger.is_settled());
        assert!(app.world().get_entity(first).is_err());
        assert!(app.world().get_entity(later).is_ok());
        assert!(
            app.world()
                .resource::<RouteEntityRegistry>()
                .record(later)
                .is_some()
        );

        settle_cleanup_system(app.world_mut());

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        let ledger = entry.cleanup_ledger.as_ref().unwrap();
        assert_eq!(ledger.receipt_count(), 1);
        assert!(!ledger.is_settled());
        assert!(app.world().get_entity(later).is_ok());
    }

    #[test]
    fn foreign_stale_task_is_held_before_manifest_reservation_zero_write() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let first = install_atomic_router(&mut app, "foreign-stale-source");
        let second = install_atomic_router(&mut app, "foreign-stale-target");
        observe_authority_system(app.world_mut());
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(first, RouteLocation::new("/"))
            .unwrap();
        let _second_request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(second, RouteLocation::new("/"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        {
            let mut navigation = app.world_mut().resource_mut::<RouterNavigation>();
            let source = navigation
                .entries
                .iter_mut()
                .find(|entry| entry.handle == first)
                .unwrap();
            let EngineAction::FrameTask(task) = source.engine.action().unwrap() else {
                panic!("source task must be issued")
            };
            let target = navigation
                .entries
                .iter_mut()
                .find(|entry| entry.handle == second)
                .unwrap();
            target.pending_failed_branch =
                Some(stale_driver_report_owner(PendingCommit::new(), task));
        }
        let before_world = authority_world_fingerprint(app.world());
        let before_registry = app
            .world()
            .resource::<RouteEntityRegistry>()
            .records()
            .to_vec();
        let before_engine = app
            .world()
            .resource::<RouterNavigation>()
            .entries
            .iter()
            .find(|entry| entry.handle == second)
            .unwrap()
            .engine
            .snapshot();
        crate::cleanup::fail_next_reservation_for_test();

        settle_cleanup_system(app.world_mut());
        settle_cleanup_system(app.world_mut());

        let entry = app
            .world()
            .resource::<RouterNavigation>()
            .entries
            .iter()
            .find(|entry| entry.handle == second)
            .unwrap();
        assert!(entry.pending_failed_branch.is_some());
        assert!(entry.cleanup_ledger.is_none());
        assert_eq!(entry.engine.snapshot(), before_engine);
        assert_eq!(authority_world_fingerprint(app.world()), before_world);
        assert_eq!(
            app.world().resource::<RouteEntityRegistry>().records(),
            before_registry
        );

        {
            let mut navigation = app.world_mut().resource_mut::<RouterNavigation>();
            let target = navigation
                .entries
                .iter_mut()
                .find(|entry| entry.handle == second)
                .unwrap();
            let EngineAction::FrameTask(task) = target.engine.action().unwrap() else {
                panic!("target task must remain current")
            };
            target.pending_failed_branch =
                Some(stale_driver_report_owner(PendingCommit::new(), task));
        }
        settle_cleanup_system(app.world_mut());
        assert!(
            app.world()
                .resource::<RouterNavigation>()
                .entries
                .iter()
                .find(|entry| entry.handle == second)
                .unwrap()
                .pending_failed_branch
                .is_some(),
            "the unconsumed reservation fault must reject the first exact retry"
        );
        settle_cleanup_system(app.world_mut());
        assert!(
            app.world()
                .resource::<RouterNavigation>()
                .entries
                .iter()
                .find(|entry| entry.handle == second)
                .unwrap()
                .pending_failed_branch
                .is_none()
        );
    }

    #[test]
    fn production_foreign_report_retains_exact_task_and_retries_without_authority_write() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let replay = std::sync::Arc::new(std::sync::Mutex::new(Some(foreign_report_identity(
            "production-foreign-report",
        ))));
        let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let router = install_replay_router(
            &mut app,
            "production-foreign-target",
            replay,
            observed.clone(),
        );
        observe_authority_system(app.world_mut());
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/first"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        let world_before = authority_world_fingerprint(app.world());
        let registry_before = app
            .world()
            .resource::<RouteEntityRegistry>()
            .records()
            .to_vec();
        let (active_before, history_before, status_before) = {
            let navigation = app.world().resource::<RouterNavigation>();
            let entry = &navigation.entries[0];
            (
                entry.engine.active().clone(),
                entry.engine.history().clone(),
                entry.engine.request(request).unwrap().status().clone(),
            )
        };

        drive_navigation_system(app.world_mut());

        assert_eq!(authority_world_fingerprint(app.world()), world_before);
        assert_eq!(
            app.world().resource::<RouteEntityRegistry>().records(),
            registry_before
        );
        {
            let navigation = app.world().resource::<RouterNavigation>();
            let entry = &navigation.entries[0];
            assert_eq!(entry.engine.active(), &active_before);
            assert_eq!(entry.engine.history(), &history_before);
            assert_eq!(
                entry.engine.request(request).unwrap().status(),
                &status_before
            );
            assert!(entry.pending_failed_branch.is_none());
            assert!(entry.cleanup_ledger.is_none());
        }

        drive_navigation_system(app.world_mut());

        let observed = observed.lock().unwrap();
        assert_eq!(observed.len(), 2);
        assert!(observed[0] == observed[1]);
        drop(observed);
        commit_presentation_system(app.world_mut());
        settle_cleanup_system(app.world_mut());
        assert!(matches!(
            app.world().resource::<RouterNavigation>().entries[0]
                .engine
                .request(request)
                .unwrap()
                .status(),
            RequestStatus::Succeeded
        ));
    }

    #[test]
    fn production_same_router_old_report_retains_new_exact_task_and_retries() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let replay = std::sync::Arc::new(std::sync::Mutex::new(None));
        let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let router = install_replay_router(
            &mut app,
            "production-old-report",
            replay.clone(),
            observed.clone(),
        );
        observe_authority_system(app.world_mut());
        let first = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/first"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        commit_presentation_system(app.world_mut());
        settle_cleanup_system(app.world_mut());
        assert!(matches!(
            app.world().resource::<RouterNavigation>().entries[0]
                .engine
                .request(first)
                .unwrap()
                .status(),
            RequestStatus::Succeeded
        ));
        let old_identity = observed.lock().unwrap()[0].clone();
        *replay.lock().unwrap() = Some(old_identity.clone());
        let second = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/second"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        let world_before = authority_world_fingerprint(app.world());
        let registry_before = app
            .world()
            .resource::<RouteEntityRegistry>()
            .records()
            .to_vec();
        let status_before = app.world().resource::<RouterNavigation>().entries[0]
            .engine
            .request(second)
            .unwrap()
            .status()
            .clone();

        drive_navigation_system(app.world_mut());

        assert_eq!(authority_world_fingerprint(app.world()), world_before);
        assert_eq!(
            app.world().resource::<RouteEntityRegistry>().records(),
            registry_before
        );
        assert_eq!(
            app.world().resource::<RouterNavigation>().entries[0]
                .engine
                .request(second)
                .unwrap()
                .status(),
            &status_before
        );

        drive_navigation_system(app.world_mut());

        let observed = observed.lock().unwrap();
        assert_eq!(observed.len(), 3);
        assert!(observed[0] == old_identity);
        assert!(observed[1] != observed[0]);
        assert!(observed[1] == observed[2]);
        drop(observed);
        commit_presentation_system(app.world_mut());
        settle_cleanup_system(app.world_mut());
        assert!(matches!(
            app.world().resource::<RouterNavigation>().entries[0]
                .engine
                .request(second)
                .unwrap()
                .status(),
            RequestStatus::Succeeded
        ));
    }

    #[test]
    pub(crate) fn production_report_application_rejects_opaque_or_foreign_identity_before_world_write()
     {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let first = install_atomic_router(&mut app, "stale-sealed-first");
        let second = install_atomic_router(&mut app, "stale-sealed-second");
        observe_authority_system(app.world_mut());
        let first_request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(first, RouteLocation::new("/"))
            .unwrap();
        let second_request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(second, RouteLocation::new("/"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        let (foreign_task, current_task) = {
            let mut navigation = app.world_mut().resource_mut::<RouterNavigation>();
            let first_entry = navigation
                .entries
                .iter_mut()
                .find(|entry| entry.handle == first)
                .unwrap();
            let EngineAction::FrameTask(task) = first_entry.engine.action().unwrap() else {
                panic!("first router must issue a frame task")
            };
            let second_entry = navigation
                .entries
                .iter_mut()
                .find(|entry| entry.handle == second)
                .unwrap();
            let EngineAction::FrameTask(current_task) = second_entry.engine.action().unwrap()
            else {
                panic!("second router must issue its own frame task")
            };
            (task, current_task)
        };
        let world_before = authority_world_fingerprint(app.world());
        let registry_before = app
            .world()
            .resource::<RouteEntityRegistry>()
            .records()
            .iter()
            .map(|record| record.claim())
            .collect::<Vec<_>>();
        let (active_before, history_before, status_before) = {
            let navigation = app.world().resource::<RouterNavigation>();
            let entry = navigation
                .entries
                .iter()
                .find(|entry| entry.handle == second)
                .unwrap();
            (
                entry.engine.active().clone(),
                entry.engine.history().clone(),
                entry
                    .engine
                    .request(second_request)
                    .unwrap()
                    .status()
                    .clone(),
            )
        };

        let (plan, report) = applied_report_with_identity(foreign_task.report_identity());
        let opaque_mismatch = app.world_mut().resource_scope(
            |world, mut navigation: bevy_ecs::change_detection::Mut<RouterNavigation>| {
                let entry = navigation
                    .entries
                    .iter_mut()
                    .find(|entry| entry.handle == second)
                    .unwrap();
                let host = entry.host.as_ref().unwrap();
                apply_applied_driver_report(
                    world,
                    &mut entry.engine,
                    host.candidate.clone(),
                    host.top_slot,
                    host.top_slot_claim.clone(),
                    &current_task,
                    &report,
                    plan,
                )
            },
        );
        assert!(matches!(
            opaque_mismatch,
            Err(DriverReportApplicationError::Stale)
        ));
        assert_eq!(authority_world_fingerprint(app.world()), world_before);
        assert_eq!(
            app.world()
                .resource::<RouteEntityRegistry>()
                .records()
                .iter()
                .map(|record| record.claim())
                .collect::<Vec<_>>(),
            registry_before
        );

        let (plan, report) = applied_report_with_identity(foreign_task.report_identity());

        let result = app.world_mut().resource_scope(
            |world, mut navigation: bevy_ecs::change_detection::Mut<RouterNavigation>| {
                let entry = navigation
                    .entries
                    .iter_mut()
                    .find(|entry| entry.handle == second)
                    .unwrap();
                let host = entry.host.as_ref().unwrap();
                apply_applied_driver_report(
                    world,
                    &mut entry.engine,
                    host.candidate.clone(),
                    host.top_slot,
                    host.top_slot_claim.clone(),
                    &foreign_task,
                    &report,
                    plan,
                )
            },
        );

        assert!(matches!(result, Err(DriverReportApplicationError::Stale)));
        assert_eq!(authority_world_fingerprint(app.world()), world_before);
        assert_eq!(
            app.world()
                .resource::<RouteEntityRegistry>()
                .records()
                .iter()
                .map(|record| record.claim())
                .collect::<Vec<_>>(),
            registry_before
        );
        let navigation = app.world().resource::<RouterNavigation>();
        let entry = navigation
            .entries
            .iter()
            .find(|entry| entry.handle == second)
            .unwrap();
        assert_eq!(entry.engine.active(), &active_before);
        assert_eq!(entry.engine.history(), &history_before);
        assert_eq!(
            entry.engine.request(second_request).unwrap().status(),
            &status_before
        );
        assert_eq!(
            navigation
                .entries
                .iter()
                .find(|entry| entry.handle == first)
                .unwrap()
                .engine
                .request(first_request)
                .unwrap()
                .status(),
            &RequestStatus::Running(
                bevy_ui_router_core::prelude::NavigationPhase::ApplyingFrame(0)
            )
        );
    }

    fn install_host_grant_observer_drift(app: &mut App) {
        app.world_mut().insert_resource(HostGrantObserverDrift {
            enabled: true,
            observed_slots: Vec::new(),
        });
        app.world_mut()
            .register_component_hooks::<ProvisionalPostWriteProbe>()
            .on_add(|mut world, HookContext { entity, .. }| {
                let enabled = {
                    let mut drift = world.resource_mut::<HostGrantObserverDrift>();
                    drift.observed_slots.push(entity);
                    drift.enabled
                };
                if enabled {
                    world
                        .commands()
                        .entity(entity)
                        .remove::<RouterInputBarrier>();
                }
            });
    }

    fn install_provisional_detach_reentry(app: &mut App) {
        app.init_resource::<ProvisionalDetachReentry>();
        app.world_mut()
            .register_component_hooks::<ChildOf>()
            .on_remove(|mut world, HookContext { entity, .. }| {
                if world.get::<ProvisionalSlotLifecycle>(entity).is_none() {
                    return;
                }
                let host = {
                    let mut reentry = world.resource_mut::<ProvisionalDetachReentry>();
                    reentry.removals += 1;
                    reentry.enabled.then_some(reentry.host).flatten()
                };
                if let Some(host) = host {
                    world.commands().entity(entity).insert(ChildOf(host));
                }
            });
    }

    fn drift_pending_output_before_commit(world: &mut World) {
        if !world.resource::<PendingPreflightDrift>().armed {
            return;
        }
        let output = {
            let navigation = world.resource::<RouterNavigation>();
            navigation.entries[0]
                .pending_commit
                .as_ref()
                .and_then(PendingCommit::frame)
                .and_then(|frame| frame.interactive_entities().first().copied())
        };
        let Some(output) = output else { return };
        *world.get_mut::<Visibility>(output).unwrap() = Visibility::Visible;
        *world.get_mut::<Pickable>(output).unwrap() = Pickable::default();
        *world.get_mut::<RouterInputBarrier>(output).unwrap() = RouterInputBarrier::allow_all();
        let mut drift = world.resource_mut::<PendingPreflightDrift>();
        drift.armed = false;
        drift.drifted.push(output);
    }

    fn drift_fallback_active_before_commit(world: &mut World) {
        if !world.resource::<FallbackReplacementDrift>().armed {
            return;
        }
        let target = {
            let navigation = world.resource::<RouterNavigation>();
            let entry = &navigation.entries[0];
            let is_fallback = entry
                .pending_commit
                .as_ref()
                .and_then(|pending| pending.final_task.as_ref())
                .is_some_and(|task| task.frame().origin() == &FrameOrigin::RouterFallback);
            is_fallback
                .then(|| {
                    entry
                        .active
                        .as_ref()
                        .and_then(RealizedBranch::first_frame)
                        .and_then(|frame| frame.interactive_entities().first().copied())
                })
                .flatten()
        };
        let Some(target) = target else { return };
        world.entity_mut(target).remove::<Pickable>();
        let mut drift = world.resource_mut::<FallbackReplacementDrift>();
        drift.armed = false;
        drift.drifted = Some(target);
    }

    fn observe_commit_before_cleanup(world: &mut World) {
        let (armed, leaving_output) = {
            let observation = world.resource::<CommitWindowObservation>();
            (observation.armed, observation.leaving_output)
        };
        if !armed {
            return;
        }
        let leaving_output = leaving_output.expect("armed observation has a leaving output");
        let new_output = world.resource::<RouterNavigation>().entries[0]
            .active
            .as_ref()
            .expect("commit published the entering frame")
            .interactive_entities()[0];
        let gate = world.resource::<RouterInputGate>();
        let sample = CommitWindowSample {
            old_gate_denied: !gate.allows_pointer(leaving_output)
                && !gate.allows_keyboard(leaving_output)
                && !gate.allows_gamepad(leaving_output),
            new_gate_allowed: gate.allows_pointer(new_output)
                && gate.allows_keyboard(new_output)
                && gate.allows_gamepad(new_output),
            input_focus: world.resource::<InputFocus>().get(),
            route_focus: world.resource::<RouteFocusState>().focused(),
        };
        let mut observation = world.resource_mut::<CommitWindowObservation>();
        observation.armed = false;
        observation.samples.push(sample);
    }

    fn nested_presentation_side(
        world: &World,
        old_entities: &[Entity],
        old_interactive: &[Entity],
        new_entities: &[Entity],
        new_interactive: &[Entity],
    ) -> NestedBranchSide {
        let matches = |entities: &[Entity], interactive: &[Entity], active: bool| {
            entities.iter().all(|entity| {
                world.get::<Visibility>(*entity)
                    == Some(if active {
                        &Visibility::Visible
                    } else {
                        &Visibility::Hidden
                    })
                    && world.get::<Pickable>(*entity)
                        == Some(if active && interactive.contains(entity) {
                            &Pickable {
                                should_block_lower: true,
                                is_hoverable: true,
                            }
                        } else {
                            &Pickable::IGNORE
                        })
                    && world
                        .get::<RouterInputBarrier>(*entity)
                        .is_some_and(|barrier| {
                            if active && interactive.contains(entity) {
                                barrier.allows_all()
                            } else {
                                barrier.denies_all()
                            }
                        })
            })
        };
        match (
            matches(old_entities, old_interactive, true)
                && matches(new_entities, new_interactive, false),
            matches(old_entities, old_interactive, false)
                && matches(new_entities, new_interactive, true),
        ) {
            (true, false) => NestedBranchSide::Old,
            (false, true) => NestedBranchSide::New,
            _ => NestedBranchSide::Mixed,
        }
    }

    fn nested_gate_side(
        gate: &RouterInputGate,
        old_interactive: &[Entity],
        new_interactive: &[Entity],
    ) -> NestedBranchSide {
        let allows = |entities: &[Entity]| {
            entities.iter().all(|entity| {
                gate.allows_pointer(*entity)
                    && gate.allows_keyboard(*entity)
                    && gate.allows_gamepad(*entity)
            })
        };
        let denies = |entities: &[Entity]| {
            entities.iter().all(|entity| {
                !gate.allows_pointer(*entity)
                    && !gate.allows_keyboard(*entity)
                    && !gate.allows_gamepad(*entity)
            })
        };
        match (
            allows(old_interactive) && denies(new_interactive),
            denies(old_interactive) && allows(new_interactive),
        ) {
            (true, false) => NestedBranchSide::Old,
            (false, true) => NestedBranchSide::New,
            _ => NestedBranchSide::Mixed,
        }
    }

    fn nested_registry_side(
        registry: &RouteEntityRegistry,
        old_entities: &[Entity],
        new_entities: &[Entity],
    ) -> NestedBranchSide {
        let all = |entities: &[Entity], lifecycle| {
            entities.iter().all(|entity| {
                registry
                    .record(*entity)
                    .is_some_and(|record| record.lifecycle() == lifecycle)
            })
        };
        match (
            all(old_entities, EntityLifecycle::Active)
                && all(new_entities, EntityLifecycle::PreparedInert),
            all(old_entities, EntityLifecycle::Leaving)
                && all(new_entities, EntityLifecycle::Active),
        ) {
            (true, false) => NestedBranchSide::Old,
            (false, true) => NestedBranchSide::New,
            _ => NestedBranchSide::Mixed,
        }
    }

    fn nested_commit_schedule_sample(world: &World) -> NestedCommitScheduleSample {
        let probe = world.resource::<NestedCommitScheduleProbe>();
        let entry = &world.resource::<RouterNavigation>().entries[0];
        let active_entities = entry
            .active
            .as_ref()
            .map(|branch| {
                branch
                    .claims()
                    .map(|claim| claim.entity())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let ecs = if active_entities == probe.old_entities {
            NestedBranchSide::Old
        } else if active_entities == probe.new_entities {
            NestedBranchSide::New
        } else {
            NestedBranchSide::Mixed
        };
        let core = if entry.engine.active().frames() == probe.old_frames {
            NestedBranchSide::Old
        } else if entry.engine.active().frames() == probe.new_frames {
            NestedBranchSide::New
        } else {
            NestedBranchSide::Mixed
        };
        let history = if entry.engine.history().current() == probe.old_location.as_ref() {
            NestedBranchSide::Old
        } else if entry.engine.history().current() == probe.new_location.as_ref() {
            NestedBranchSide::New
        } else {
            NestedBranchSide::Mixed
        };
        NestedCommitScheduleSample {
            physical: nested_presentation_side(
                world,
                &probe.old_entities,
                &probe.old_interactive,
                &probe.new_entities,
                &probe.new_interactive,
            ),
            gate: nested_gate_side(
                world.resource::<RouterInputGate>(),
                &probe.old_interactive,
                &probe.new_interactive,
            ),
            registry: nested_registry_side(
                world.resource::<RouteEntityRegistry>(),
                &probe.old_entities,
                &probe.new_entities,
            ),
            ecs,
            core,
            history,
            input_focus: world.resource::<InputFocus>().get(),
            route_focus: world.resource::<RouteFocusState>().focused(),
        }
    }

    fn observe_nested_commit_before(world: &mut World) {
        let should_observe = {
            let probe = world.resource::<NestedCommitScheduleProbe>();
            probe.armed && probe.samples.is_empty()
        };
        if should_observe {
            let sample = nested_commit_schedule_sample(world);
            world
                .resource_mut::<NestedCommitScheduleProbe>()
                .samples
                .push(sample);
        }
    }

    fn observe_nested_commit_after(world: &mut World) {
        let should_observe = {
            let probe = world.resource::<NestedCommitScheduleProbe>();
            probe.armed && probe.samples.len() == 1
        };
        if !should_observe {
            return;
        }
        let sample = nested_commit_schedule_sample(world);
        let actual_mutations = take_test_commit_mutation_trace();
        let mut probe = world.resource_mut::<NestedCommitScheduleProbe>();
        probe.samples.push(sample);
        let old_shells = probe.old_shells.clone();
        let new_shells = probe.new_shells.clone();
        probe.order.extend(
            actual_mutations
                .into_iter()
                .filter_map(|mutation| match mutation {
                    TestCommitMutation::Leaving(entity) if old_shells.contains(&entity) => {
                        Some(NestedCommitOrder::Leaving(entity))
                    }
                    TestCommitMutation::Entering(entity) if new_shells.contains(&entity) => {
                        Some(NestedCommitOrder::Entering(entity))
                    }
                    TestCommitMutation::Leaving(_)
                    | TestCommitMutation::Entering(_)
                    | TestCommitMutation::PreparedInert(_) => None,
                }),
        );
        probe.armed = false;
    }

    fn observe_mount_new_role_before_commit(world: &mut World) {
        if !world.resource::<MountNewRoleScheduleProbe>().armed {
            return;
        }
        let roles = {
            let navigation = world.resource::<RouterNavigation>();
            let Some(entry) = navigation.entries.first() else {
                return;
            };
            let Some(pending) = entry.pending_commit.as_ref() else {
                return;
            };
            let projection = match (pending.ticket.as_ref(), pending.final_task.as_ref()) {
                (Some(ticket), _) => entry.engine.reconciliation_projection_for_commit(ticket),
                (None, Some(task)) => entry.engine.reconciliation_projection_for_task(task),
                (None, None) => return,
            };
            let Ok(projection) = projection else {
                return;
            };
            projection
                .child_slots()
                .map(|role| role.kind())
                .collect::<Vec<_>>()
        };
        if roles.is_empty() {
            return;
        }
        let mut probe = world.resource_mut::<MountNewRoleScheduleProbe>();
        probe.roles = Some(roles);
        probe.armed = false;
    }

    fn install_nested_leaving_cleanup_trace(app: &mut App) {
        app.init_resource::<NestedLeavingCleanupTrace>();
        app.world_mut()
            .register_component_hooks::<RouteOwnershipSeal>()
            .on_despawn(|mut world, HookContext { entity, .. }| {
                let unit = world
                    .get_resource::<NestedLeavingCleanupTrace>()
                    .and_then(|trace| trace.units.get(&entity).copied());
                if let Some(unit) = unit {
                    world
                        .resource_mut::<NestedLeavingCleanupTrace>()
                        .despawned
                        .push(unit);
                }
            });
    }

    fn nested_protected_fingerprint(world: &World, entity: Entity) -> NestedProtectedFingerprint {
        NestedProtectedFingerprint {
            manifest: ComponentManifest::from_world(world, entity).unwrap(),
            parent: world.get::<ChildOf>(entity).map(ChildOf::parent),
            children: world
                .get::<Children>(entity)
                .map(|children| children.iter().copied().collect())
                .unwrap_or_default(),
            visibility: world.get::<Visibility>(entity).copied(),
            pickable: world
                .get::<Pickable>(entity)
                .map(|pickable| (pickable.should_block_lower, pickable.is_hoverable)),
            barrier: world
                .get::<RouterInputBarrier>(entity)
                .map(|barrier| (barrier.allows_all(), barrier.denies_all())),
            sentinel: world.get::<NestedAppHostSentinel>(entity).copied(),
        }
    }

    #[test]
    fn pending_preflight_drift_settles_and_terminates_without_replacing_active() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        app.init_resource::<PendingPreflightDrift>().add_systems(
            Update,
            drift_pending_output_before_commit
                .after(RouterScheduleSet::DriveNavigation)
                .before(RouterScheduleSet::CommitPresentation),
        );
        let router = install_atomic_router(&mut app, "pending-preflight-drift");
        app.update();
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        app.update();

        let (active_before, history_before) = {
            let navigation = app.world().resource::<RouterNavigation>();
            let entry = &navigation.entries[0];
            (
                (
                    entry
                        .active
                        .as_ref()
                        .unwrap()
                        .first_frame()
                        .unwrap()
                        .frame()
                        .clone(),
                    entry
                        .active
                        .as_ref()
                        .unwrap()
                        .first_frame()
                        .unwrap()
                        .shell(),
                ),
                entry.engine.history().clone(),
            )
        };
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(
                router,
                RouteLocation::new("/").with_fragment("drifted-pending"),
            )
            .unwrap();
        app.world_mut()
            .resource_mut::<PendingPreflightDrift>()
            .armed = true;

        app.update();

        let drifted = app
            .world()
            .resource::<PendingPreflightDrift>()
            .drifted
            .clone();
        assert!(!drifted.is_empty());
        let navigation = app.world().resource::<RouterNavigation>();
        let entry = &navigation.entries[0];
        assert!(
            entry.pending_commit.is_none(),
            "invalid pending work must not be requeued"
        );
        assert_eq!(
            (
                entry
                    .active
                    .as_ref()
                    .unwrap()
                    .first_frame()
                    .unwrap()
                    .frame()
                    .clone(),
                entry
                    .active
                    .as_ref()
                    .unwrap()
                    .first_frame()
                    .unwrap()
                    .shell()
            ),
            active_before
        );
        assert_eq!(entry.engine.history(), &history_before);
        assert!(drifted.iter().all(|entity| {
            app.world().get_entity(*entity).is_err()
                || app
                    .world()
                    .resource::<QuarantineLeaseTable>()
                    .record(*entity)
                    .is_some()
        }));

        for _ in 0..16 {
            app.update();
        }
        assert!(matches!(
            app.world().resource::<RouterNavigation>().entries[0]
                .engine
                .request(request)
                .unwrap()
                .status(),
            RequestStatus::Succeeded | RequestStatus::Failed(_)
        ));
    }

    #[test]
    fn reconciliation_mixed_entering_and_leaving_drift_prefers_authority_loss() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let router = install_nested_router(&mut app, observed, true);
        app.update();
        commit_initial_nested_branch(&mut app, router);
        let request = prepare_nested_branch_for_commit(
            &mut app,
            router,
            RouteLocation::new("/items/2").with_fragment("mixed-preflight-drift"),
        );
        let (leaving, entering) = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            (
                entry.active.as_ref().unwrap().frames()[1].interactive_entities()[0],
                entry.pending_commit.as_ref().unwrap().branch.frames()[0].interactive_entities()[0],
            )
        };
        {
            let mut pickable = app.world_mut().get_mut::<Pickable>(leaving).unwrap();
            pickable.should_block_lower = !pickable.should_block_lower;
            pickable.is_hoverable = !pickable.is_hoverable;
        }
        app.world_mut().entity_mut(entering).remove::<Pickable>();

        commit_presentation_system(app.world_mut());

        {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            assert!(entry.pending_failed_branch.is_none());
            assert!(entry.authority_protocol_residual.is_some() || entry.cleanup_ledger.is_some());
        }
        for _ in 0..16 {
            settle_cleanup_system(app.world_mut());
        }
        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert!(matches!(
            entry.engine.request(request).unwrap().status(),
            RequestStatus::Failed(failure)
                if failure.kind() == NavigationFailureKind::AuthorityLost
        ));
    }

    #[test]
    fn reconciliation_entering_drift_does_not_hide_preserved_relationship_cycle() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let router = install_nested_router(&mut app, observed, true);
        app.update();
        commit_initial_nested_branch(&mut app, router);
        let history_before = app.world().resource::<RouterNavigation>().entries[0]
            .engine
            .history()
            .clone();
        let request = prepare_nested_branch_for_commit(
            &mut app,
            router,
            RouteLocation::new("/items/2").with_fragment("preserved-cycle-and-entering-drift"),
        );
        let (cycle_child, cycle_parent, entering) = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            let preserved = &entry.active.as_ref().unwrap().frames()[0];
            let owned = preserved
                .claims()
                .map(|claim| claim.entity())
                .collect::<std::collections::HashSet<_>>();
            let (cycle_child, cycle_parent) = preserved
                .cleanup_members()
                .find_map(|(claim, parent)| {
                    owned.contains(&parent).then_some((claim.entity(), parent))
                })
                .unwrap();
            (
                cycle_child,
                cycle_parent,
                entry.pending_commit.as_ref().unwrap().branch.frames()[0].interactive_entities()[0],
            )
        };
        app.world_mut()
            .entity_mut(cycle_parent)
            .insert(ChildOf(cycle_child));
        app.world_mut().entity_mut(entering).remove::<Pickable>();
        app.world_mut().flush();

        commit_presentation_system(app.world_mut());

        {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            assert!(entry.pending_failed_branch.is_none());
            assert!(entry.authority_protocol_residual.is_some() || entry.cleanup_ledger.is_some());
            assert_eq!(entry.engine.history(), &history_before);
        }
        for _ in 0..16 {
            settle_cleanup_system(app.world_mut());
        }
        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert!(matches!(
            entry.engine.request(request).unwrap().status(),
            RequestStatus::Failed(failure)
                if failure.kind() == NavigationFailureKind::AuthorityLost
        ));
    }

    #[test]
    fn reconciliation_entering_only_drift_remains_pending_rejected() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let router = install_nested_router(&mut app, observed, true);
        app.update();
        commit_initial_nested_branch(&mut app, router);
        let history_before = app.world().resource::<RouterNavigation>().entries[0]
            .engine
            .history()
            .clone();
        prepare_nested_branch_for_commit(
            &mut app,
            router,
            RouteLocation::new("/items/2").with_fragment("entering-only-preflight-drift"),
        );
        let (current_output, entering_shell) = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            (
                entry.active.as_ref().unwrap().frames()[0].shell(),
                entry.pending_commit.as_ref().unwrap().branch.frames()[0].shell(),
            )
        };
        app.world_mut()
            .entity_mut(current_output)
            .add_child(entering_shell);

        commit_presentation_system(app.world_mut());

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert!(entry.authority_protocol_residual.is_none());
        assert!(entry.active.is_some());
        assert!(entry.pending_commit.is_none());
        assert!(matches!(
            entry
                .pending_failed_branch
                .as_ref()
                .map(|failed| &failed.terminal),
            Some(PendingFailedBranchTerminal::TaskWithPhysicalResidual { .. })
        ));
        assert_eq!(entry.engine.history(), &history_before);
    }

    #[test]
    fn reconciliation_authority_loss_app_despawned_host_settles_and_readmits_without_replay() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let router = install_atomic_router(&mut app, "authority-loss-despawned-host");
        app.update();
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        app.update();

        let (host, history_before) = {
            let navigation = app.world().resource::<RouterNavigation>();
            let entry = &navigation.entries[0];
            (
                entry.host.as_ref().unwrap().host,
                entry.engine.history().clone(),
            )
        };
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(
                router,
                RouteLocation::new("/").with_fragment("despawned-host"),
            )
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        let frozen_obligation_count = {
            let navigation = app.world().resource::<RouterNavigation>();
            let entry = &navigation.entries[0];
            let pending = entry.pending_commit.as_ref().unwrap();
            let active = entry.active.as_ref().unwrap();
            let binding = entry.host.as_ref().unwrap();
            FrozenCleanupManifest::from_authority_settlement(
                app.world(),
                app.world().resource::<RouteEntityRegistry>(),
                &pending.branch,
                active,
                binding,
            )
            .unwrap()
            .len()
        };

        assert!(app.world_mut().despawn(host));
        assert!(app.world().get_entity(host).is_err());
        commit_presentation_system(app.world_mut());

        {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            assert_eq!(
                entry.engine.snapshot().availability(),
                &RouterAvailability::RecoveringAuthority
            );
            assert_eq!(
                entry.engine.request(request).unwrap().status(),
                &RequestStatus::Running(NavigationPhase::Recovering)
            );
            assert!(entry.active.is_none());
            assert!(entry.host.is_none());
            assert!(
                entry
                    .cleanup_ledger
                    .as_ref()
                    .is_some_and(CleanupLedger::is_authority_settlement)
            );
        }

        for _ in 0..16 {
            settle_cleanup_system(app.world_mut());
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            if entry.cleanup_ledger.is_none()
                && matches!(
                    entry.engine.request(request).unwrap().status(),
                    RequestStatus::Failed(_)
                )
            {
                break;
            }
        }

        let terminal_trace_shape = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            assert!(
                entry.cleanup_ledger.is_none(),
                "despawned-host authority cleanup remained nonterminal: status={:?}, ledger={:?}",
                entry.engine.request(request).unwrap().status(),
                entry
                    .cleanup_ledger
                    .as_ref()
                    .map(|ledger| (ledger.is_settled(), ledger.receipt_count()))
            );
            assert!(matches!(
                entry.engine.request(request).unwrap().status(),
                RequestStatus::Failed(failure)
                    if failure.kind() == NavigationFailureKind::AuthorityLost
            ));
            assert!(entry.active.is_none());
            assert!(entry.host.is_none());
            assert_eq!(
                entry.engine.snapshot().active().kind(),
                ActivePresentationKind::None
            );
            assert_eq!(entry.engine.history(), &history_before);
            let request_inspection = entry.engine.request(request).unwrap();
            let trace = request_inspection.trace();
            let receipts = trace
                .iter()
                .filter(|event| event.cleanup_disposition().is_some())
                .count();
            let terminals = trace
                .iter()
                .filter(|event| event.kind() == NavigationTraceEventKind::RequestTerminal)
                .count();
            assert_eq!(receipts, frozen_obligation_count);
            assert_eq!(terminals, 1);
            (trace.len(), receipts, terminals)
        };
        assert!(app.world().get_entity(host).is_err());

        for _ in 0..3 {
            collect_navigation_system(app.world_mut());
            drive_navigation_system(app.world_mut());
            commit_presentation_system(app.world_mut());
            settle_cleanup_system(app.world_mut());
        }
        {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            let request_inspection = entry.engine.request(request).unwrap();
            let trace = request_inspection.trace();
            assert_eq!(
                (
                    trace.len(),
                    trace
                        .iter()
                        .filter(|event| event.cleanup_disposition().is_some())
                        .count(),
                    trace
                        .iter()
                        .filter(|event| event.kind() == NavigationTraceEventKind::RequestTerminal)
                        .count(),
                ),
                terminal_trace_shape
            );
            assert_eq!(entry.engine.history(), &history_before);
        }
        assert!(app.world().get_entity(host).is_err());

        let replacement_host = app
            .world_mut()
            .spawn((Node::default(), RouterHostRegistration::new(router)))
            .id();
        assert_ne!(replacement_host, host);
        for _ in 0..4 {
            observe_authority_system(app.world_mut());
            if app.world().resource::<RouterNavigation>().entries[0]
                .host
                .as_ref()
                .is_some_and(|binding| binding.host == replacement_host)
            {
                break;
            }
        }

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert_eq!(
            entry.host.as_ref().map(|binding| binding.host),
            Some(replacement_host)
        );
        assert_eq!(
            entry.engine.snapshot().availability(),
            &RouterAvailability::Ready
        );
        assert_eq!(
            entry.engine.snapshot().active().kind(),
            ActivePresentationKind::None
        );
        assert!(entry.active.is_none());
        assert!(entry.cleanup_ledger.is_none());
        assert_eq!(entry.engine.history(), &history_before);
        assert!(entry.engine.snapshot().running_request().is_none());
        let request_inspection = entry.engine.request(request).unwrap();
        let trace = request_inspection.trace();
        assert_eq!(
            (
                trace.len(),
                trace
                    .iter()
                    .filter(|event| event.cleanup_disposition().is_some())
                    .count(),
                trace
                    .iter()
                    .filter(|event| event.kind() == NavigationTraceEventKind::RequestTerminal)
                    .count(),
            ),
            terminal_trace_shape
        );
        assert!(
            app.world_mut().resource_mut::<RouterNavigation>().entries[0]
                .engine
                .action()
                .is_none()
        );
        assert!(app.world().get_entity(host).is_err());
    }

    #[test]
    fn reconciliation_authority_loss_host_drift_settles_and_readmits_without_replay() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let router = install_atomic_router(&mut app, "authority-loss-host-drift");
        app.update();
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        app.update();

        let (host, history_before) = {
            let navigation = app.world().resource::<RouterNavigation>();
            let entry = &navigation.entries[0];
            (
                entry.host.as_ref().unwrap().host,
                entry.engine.history().clone(),
            )
        };
        let current = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(
                router,
                RouteLocation::new("/").with_fragment("authority-loss-current"),
            )
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        let queued = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(
                router,
                RouteLocation::new("/").with_fragment("authority-loss-queued"),
            )
            .unwrap();
        let registration = app
            .world_mut()
            .entity_mut(host)
            .take::<RouterHostRegistration>()
            .unwrap();
        app.world_mut().entity_mut(host).insert(registration);

        commit_presentation_system(app.world_mut());

        {
            let navigation = app.world().resource::<RouterNavigation>();
            let entry = &navigation.entries[0];
            assert_eq!(
                entry.engine.snapshot().availability(),
                &RouterAvailability::RecoveringAuthority
            );
            assert_eq!(
                entry.engine.request(current).unwrap().status(),
                &RequestStatus::Running(NavigationPhase::Recovering)
            );
            assert!(matches!(
                entry.engine.request(queued).unwrap().status(),
                RequestStatus::Failed(failure)
                    if failure.kind() == NavigationFailureKind::AuthorityLost
            ));
            assert!(entry.active.is_none());
            assert!(
                !entry
                    .cleanup_ledger
                    .as_ref()
                    .unwrap()
                    .leaving_frames()
                    .is_empty()
            );
        }

        for _ in 0..8 {
            settle_cleanup_system(app.world_mut());
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            if entry.cleanup_ledger.is_none()
                && matches!(
                    entry.engine.request(current).unwrap().status(),
                    RequestStatus::Failed(_)
                )
            {
                break;
            }
        }

        {
            let navigation = app.world().resource::<RouterNavigation>();
            let entry = &navigation.entries[0];
            assert!(
                entry.cleanup_ledger.is_none(),
                "authority cleanup did not settle: status={:?}, ledger={:?}",
                entry.engine.request(current).unwrap().status(),
                entry
                    .cleanup_ledger
                    .as_ref()
                    .map(|ledger| (ledger.is_settled(), ledger.receipt_count()))
            );
            let request = entry.engine.request(current).unwrap();
            let trace = request.trace();
            let receipts = trace
                .iter()
                .enumerate()
                .filter_map(|(index, event)| event.cleanup_disposition().is_some().then_some(index))
                .collect::<Vec<_>>();
            let cleanup_settled = trace
                .iter()
                .rposition(|event| {
                    event.kind() == NavigationTraceEventKind::CleanupSettled
                        && event.cleanup_disposition().is_none()
                })
                .unwrap();
            let terminal = trace
                .iter()
                .position(|event| event.kind() == NavigationTraceEventKind::RequestTerminal)
                .unwrap();
            assert!(!receipts.is_empty());
            assert!(receipts.iter().all(|receipt| *receipt < cleanup_settled));
            assert!(cleanup_settled < terminal);
            assert!(matches!(
                entry.engine.request(current).unwrap().status(),
                RequestStatus::Failed(failure)
                    if failure.kind() == NavigationFailureKind::AuthorityLost
            ));
            assert!(entry.active.is_none());
            assert!(entry.host.is_none());
            assert_eq!(
                entry.engine.snapshot().active().kind(),
                ActivePresentationKind::None
            );
            assert_eq!(entry.engine.history(), &history_before);
            assert!(entry.cleanup_ledger.is_none());
        }

        observe_authority_system(app.world_mut());

        {
            let navigation = app.world().resource::<RouterNavigation>();
            let entry = &navigation.entries[0];
            assert!(entry.host.is_some());
            assert_eq!(
                entry.engine.snapshot().availability(),
                &RouterAvailability::Ready
            );
            assert_eq!(
                entry.engine.snapshot().active().kind(),
                ActivePresentationKind::None
            );
            assert!(entry.active.is_none());
            assert_eq!(entry.engine.history(), &history_before);
            assert!(entry.engine.snapshot().running_request().is_none());
        }
        assert!(
            app.world_mut().resource_mut::<RouterNavigation>().entries[0]
                .engine
                .action()
                .is_none()
        );
    }

    #[test]
    fn reconciliation_authority_loss_cycle_settles_top_slot_without_touching_app_host() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let router = install_nested_router(&mut app, observed, true);
        app.update();
        commit_initial_nested_branch(&mut app, router);
        let (host, top_slot, cycle_child, cycle_parent) = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            let host = entry.host.as_ref().unwrap();
            let frame = &entry.active.as_ref().unwrap().frames()[0];
            let cycle_child = frame.outlets()[0].anchor();
            (
                host.host,
                host.top_slot,
                cycle_child,
                app.world()
                    .get::<ChildOf>(cycle_child)
                    .map(ChildOf::parent)
                    .unwrap(),
            )
        };
        app.world_mut()
            .entity_mut(host)
            .insert(NestedAppHostSentinel(0x5eed));
        let sibling_a = app.world_mut().spawn(NestedAppHostSentinel(1)).id();
        let sibling_b = app.world_mut().spawn(NestedAppHostSentinel(2)).id();
        app.world_mut().entity_mut(host).add_child(sibling_a);
        app.world_mut().entity_mut(host).add_child(sibling_b);
        let host_children_before = app.world().get::<Children>(host).unwrap().to_vec();

        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(
                router,
                RouteLocation::new("/items/2").with_fragment("authority-cycle"),
            )
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            assert!(matches!(
                entry.engine.request(request).unwrap().status(),
                RequestStatus::Running(_)
            ));
            assert!(entry.pending_commit.is_some());
        }
        app.world_mut()
            .entity_mut(cycle_parent)
            .insert(ChildOf(cycle_child));
        app.world_mut().flush();
        let registration = app
            .world_mut()
            .entity_mut(host)
            .take::<RouterHostRegistration>()
            .unwrap();
        app.world_mut().entity_mut(host).insert(registration);
        commit_presentation_system(app.world_mut());
        {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            assert_eq!(
                entry.engine.snapshot().availability(),
                &RouterAvailability::RecoveringAuthority
            );
            assert!(
                entry
                    .cleanup_ledger
                    .as_ref()
                    .is_some_and(crate::cleanup::CleanupLedger::is_authority_settlement)
            );
        }

        for _ in 0..8 {
            settle_cleanup_system(app.world_mut());
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            if entry.cleanup_ledger.is_none()
                && matches!(
                    entry.engine.request(request).unwrap().status(),
                    RequestStatus::Failed(_)
                )
            {
                break;
            }
        }

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert!(
            matches!(
                entry.engine.request(request).unwrap().status(),
                RequestStatus::Failed(failure)
                    if failure.kind() == NavigationFailureKind::AuthorityLost
            ),
            "status={:?}, availability={:?}, ledger={:?}, protocol_residual={}",
            entry.engine.request(request).unwrap().status(),
            entry.engine.snapshot().availability(),
            entry
                .cleanup_ledger
                .as_ref()
                .map(|ledger| (ledger.is_settled(), ledger.receipt_count())),
            entry.authority_protocol_residual.is_some(),
        );
        assert!(entry.cleanup_ledger.is_none());
        assert!(entry
            .engine
            .request(request)
            .unwrap()
            .trace()
            .iter()
            .any(|event| {
                event.cleanup_disposition()
                    == Some(
                        bevy_ui_router_core::prelude::CleanupDisposition::QuarantinedForeignDescendant,
                    )
            }));
        assert!(app.world().get_entity(top_slot).is_err());
        assert!(
            app.world()
                .resource::<QuarantineLeaseTable>()
                .authority_lost_tombstone(top_slot)
                .is_none()
        );
        let quarantine = app.world().resource::<QuarantineLeaseTable>();
        assert!(quarantine.record(cycle_child).is_some());
        assert!(quarantine.record(cycle_parent).is_some());
        assert_eq!(
            quarantine
                .leases()
                .iter()
                .filter(|lease| lease.owned_entities().contains(&cycle_child))
                .count(),
            1
        );
        assert_eq!(
            quarantine
                .leases()
                .iter()
                .filter(|lease| lease.owned_entities().contains(&cycle_parent))
                .count(),
            1
        );
        assert!(quarantine.authority_lost_tombstone(cycle_child).is_none());
        assert!(quarantine.authority_lost_tombstone(cycle_parent).is_none());
        assert!(
            app.world()
                .resource::<RouteEntityRegistry>()
                .record(cycle_child)
                .is_none()
        );
        assert!(
            app.world()
                .resource::<RouteEntityRegistry>()
                .record(cycle_parent)
                .is_none()
        );
        assert_eq!(
            app.world().get::<NestedAppHostSentinel>(host),
            Some(&NestedAppHostSentinel(0x5eed))
        );
        assert_eq!(
            app.world().get::<NestedAppHostSentinel>(sibling_a),
            Some(&NestedAppHostSentinel(1))
        );
        assert_eq!(
            app.world().get::<NestedAppHostSentinel>(sibling_b),
            Some(&NestedAppHostSentinel(2))
        );
        let host_children = app.world().get::<Children>(host).unwrap();
        let expected_host_children = host_children_before
            .into_iter()
            .filter(|child| *child != top_slot)
            .collect::<Vec<_>>();
        assert_eq!(host_children.to_vec(), expected_host_children);
        assert!(app.world().get::<ChildOf>(top_slot).is_none());
        let sibling_a_index = host_children
            .iter()
            .position(|child| *child == sibling_a)
            .unwrap();
        let sibling_b_index = host_children
            .iter()
            .position(|child| *child == sibling_b)
            .unwrap();
        assert!(sibling_a_index < sibling_b_index);
    }

    #[test]
    fn reconciliation_entering_cycle_is_rejected_without_retiring_active_branch() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let router = install_nested_router(&mut app, observed, true);
        app.update();
        commit_initial_nested_branch(&mut app, router);
        let (active_frames, active_claims, active_values, history_before) = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            let active = entry.active.as_ref().unwrap();
            let claims = active
                .frames()
                .iter()
                .flat_map(RealizedFrame::claims)
                .cloned()
                .collect::<Vec<_>>();
            let values = claims
                .iter()
                .map(|claim| {
                    (
                        claim.entity(),
                        *app.world().get::<Visibility>(claim.entity()).unwrap(),
                        *app.world().get::<Pickable>(claim.entity()).unwrap(),
                        *app.world()
                            .get::<RouterInputBarrier>(claim.entity())
                            .unwrap(),
                        *app.world().get::<FrameLifecycle>(claim.entity()).unwrap(),
                    )
                })
                .collect::<Vec<_>>();
            (
                active
                    .frames()
                    .iter()
                    .map(|frame| frame.frame().clone())
                    .collect::<Vec<_>>(),
                claims,
                values,
                entry.engine.history().clone(),
            )
        };
        let _request = prepare_nested_branch_for_commit(
            &mut app,
            router,
            RouteLocation::new("/items/2").with_fragment("entering-cycle"),
        );
        let (cycle_child, cycle_parent, failed_claims, mount, mount_claim) = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            let frame = &entry.pending_commit.as_ref().unwrap().branch.frames()[0];
            let owned = frame
                .claims()
                .map(|claim| claim.entity())
                .collect::<std::collections::HashSet<_>>();
            let (cycle_child, cycle_parent) = frame
                .cleanup_members()
                .find_map(|(claim, parent)| {
                    owned.contains(&parent).then_some((claim.entity(), parent))
                })
                .unwrap();
            let (mount, mount_claim) = frame.mount_binding().physical_parent();
            (
                cycle_child,
                cycle_parent,
                frame.claims().cloned().collect::<Vec<_>>(),
                mount,
                mount_claim.cloned(),
            )
        };
        assert_eq!(
            app.world().get::<ChildOf>(cycle_child).map(ChildOf::parent),
            Some(cycle_parent)
        );
        app.world_mut()
            .entity_mut(cycle_parent)
            .insert(ChildOf(cycle_child));
        app.world_mut().flush();
        let mount_children = app
            .world()
            .get::<Children>(mount)
            .map(|children| children.to_vec())
            .unwrap_or_default();
        let mount_record = app
            .world()
            .resource::<RouteEntityRegistry>()
            .record(mount)
            .cloned()
            .unwrap();
        *app.world_mut().resource_mut::<InputFocus>() = InputFocus::from_entity(cycle_child);
        app.world_mut()
            .resource_mut::<RouteFocusState>()
            .set_focused(Some(cycle_child));

        commit_presentation_system(app.world_mut());

        {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            assert_eq!(
                entry.engine.snapshot().availability(),
                &RouterAvailability::Ready
            );
            assert!(entry.pending_failed_branch.is_some());
            assert!(entry.cleanup_ledger.is_none());
            assert!(!matches!(
                entry.engine.request(_request).unwrap().status(),
                RequestStatus::Succeeded
            ));
            assert_eq!(entry.engine.history(), &history_before);
            assert_eq!(
                entry
                    .active
                    .as_ref()
                    .unwrap()
                    .frames()
                    .iter()
                    .map(|frame| frame.frame().clone())
                    .collect::<Vec<_>>(),
                active_frames
            );
        }
        assert!(app.world().get_entity(cycle_child).is_ok());
        assert!(app.world().get_entity(cycle_parent).is_ok());
        for _ in 0..12 {
            settle_cleanup_system(app.world_mut());
            if matches!(
                app.world().resource::<RouterNavigation>().entries[0]
                    .engine
                    .request(_request)
                    .unwrap()
                    .status(),
                RequestStatus::Failed(_)
            ) {
                break;
            }
        }
        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert!(
            matches!(
                entry.engine.request(_request).unwrap().status(),
                RequestStatus::Failed(_)
            ),
            "status={:?}, pending_failed={}, ledger={:?}",
            entry.engine.request(_request).unwrap().status(),
            entry.pending_failed_branch.is_some(),
            entry
                .cleanup_ledger
                .as_ref()
                .map(|ledger| (ledger.is_settled(), ledger.receipt_count()))
        );
        assert_eq!(
            entry.engine.snapshot().availability(),
            &RouterAvailability::Ready
        );
        assert_eq!(
            entry
                .active
                .as_ref()
                .unwrap()
                .frames()
                .iter()
                .map(|frame| frame.frame().clone())
                .collect::<Vec<_>>(),
            active_frames
        );
        assert_eq!(entry.engine.history(), &history_before);
        assert!(entry.pending_failed_branch.is_none());
        assert!(entry.cleanup_ledger.is_none());
        assert_eq!(app.world().resource::<InputFocus>().get(), None);
        assert_eq!(app.world().resource::<RouteFocusState>().focused(), None);
        let registry = app.world().resource::<RouteEntityRegistry>();
        let quarantine = app.world().resource::<QuarantineLeaseTable>();
        for claim in &failed_claims {
            assert!(app.world().get_entity(claim.entity()).is_ok());
            assert_eq!(
                app.world().get::<Visibility>(claim.entity()),
                Some(&Visibility::Hidden)
            );
            assert_eq!(
                app.world().get::<Pickable>(claim.entity()),
                Some(&Pickable::IGNORE)
            );
            assert!(
                app.world()
                    .get::<RouterInputBarrier>(claim.entity())
                    .is_some_and(RouterInputBarrier::denies_all)
            );
            assert_eq!(
                app.world().get::<FrameLifecycle>(claim.entity()),
                Some(&FrameLifecycle(EntityLifecycle::Quarantined))
            );
            assert!(registry.record(claim.entity()).is_none());
            assert!(quarantine.record(claim.entity()).is_some());
            assert_eq!(
                quarantine
                    .leases()
                    .iter()
                    .filter(|lease| lease.owned_entities().contains(&claim.entity()))
                    .count(),
                1
            );
            assert!(
                quarantine
                    .authority_lost_tombstone(claim.entity())
                    .is_none()
            );
        }
        for claim in &active_claims {
            assert!(
                registry
                    .record(claim.entity())
                    .is_some_and(|record| record.claim() == *claim)
            );
            assert!(quarantine.record(claim.entity()).is_none());
        }
        assert_eq!(
            active_claims
                .iter()
                .map(|claim| {
                    (
                        claim.entity(),
                        *app.world().get::<Visibility>(claim.entity()).unwrap(),
                        *app.world().get::<Pickable>(claim.entity()).unwrap(),
                        *app.world()
                            .get::<RouterInputBarrier>(claim.entity())
                            .unwrap(),
                        *app.world().get::<FrameLifecycle>(claim.entity()).unwrap(),
                    )
                })
                .collect::<Vec<_>>(),
            active_values
        );
        assert_eq!(
            app.world()
                .get::<Children>(mount)
                .map(|children| children.to_vec())
                .unwrap_or_default(),
            mount_children
        );
        assert_eq!(registry.record(mount), Some(&mount_record));
        if let Some(mount_claim) = mount_claim {
            assert_eq!(mount_record.claim(), mount_claim);
        }
        let terminal_trace_len = entry.engine.request(_request).unwrap().trace().len();
        let terminal_lease_count = quarantine.leases().len();
        for _ in 0..3 {
            settle_cleanup_system(app.world_mut());
        }
        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert_eq!(
            entry.engine.request(_request).unwrap().trace().len(),
            terminal_trace_len
        );
        assert_eq!(entry.engine.history(), &history_before);
        assert_eq!(
            app.world()
                .resource::<QuarantineLeaseTable>()
                .leases()
                .len(),
            terminal_lease_count
        );
        assert_eq!(
            app.world()
                .get::<Children>(mount)
                .map(|children| children.to_vec())
                .unwrap_or_default(),
            mount_children
        );
    }

    #[test]
    fn reconciliation_authority_loss_retention_fixed_point_preserves_foreign_exact_child() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let router = install_nested_router(&mut app, observed, true);
        app.update();
        commit_initial_nested_branch(&mut app, router);
        let (host, child, parent) = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            let frame = &entry.active.as_ref().unwrap().frames()[0];
            let child = frame.outlets()[0].anchor();
            let parent = app
                .world()
                .get::<ChildOf>(child)
                .map(ChildOf::parent)
                .unwrap();
            assert!(frame.claims().any(|claim| claim.entity() == parent));
            (entry.host.as_ref().unwrap().host, child, parent)
        };
        let request = prepare_nested_branch_for_commit(
            &mut app,
            router,
            RouteLocation::new("/items/2").with_fragment("retention-fixed-point"),
        );
        app.world_mut()
            .entity_mut(child)
            .insert(NestedAppHostSentinel(0xc1));
        let registration = app
            .world_mut()
            .entity_mut(host)
            .take::<RouterHostRegistration>()
            .unwrap();
        app.world_mut().entity_mut(host).insert(registration);
        commit_presentation_system(app.world_mut());

        for _ in 0..12 {
            settle_cleanup_system(app.world_mut());
            if app.world().resource::<RouterNavigation>().entries[0]
                .cleanup_ledger
                .is_none()
            {
                break;
            }
        }

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert!(
            matches!(
                entry.engine.request(request).unwrap().status(),
                RequestStatus::Failed(failure)
                    if failure.kind() == NavigationFailureKind::AuthorityLost
            ),
            "status={:?}, availability={:?}, ledger={:?}",
            entry.engine.request(request).unwrap().status(),
            entry.engine.snapshot().availability(),
            entry
                .cleanup_ledger
                .as_ref()
                .map(|ledger| (ledger.is_settled(), ledger.receipt_count()))
        );
        assert!(entry.cleanup_ledger.is_none());
        assert_eq!(
            app.world().get::<NestedAppHostSentinel>(child),
            Some(&NestedAppHostSentinel(0xc1))
        );
        assert!(app.world().get_entity(parent).is_ok());
        let registry = app.world().resource::<RouteEntityRegistry>();
        assert!(registry.record(child).is_none());
        assert!(registry.record(parent).is_none());
        let quarantine = app.world().resource::<QuarantineLeaseTable>();
        assert!(quarantine.record(child).is_some());
        assert!(quarantine.record(parent).is_some());
        assert_eq!(
            quarantine
                .leases()
                .iter()
                .filter(|lease| lease.owned_entities().contains(&child))
                .count(),
            1
        );
        assert_eq!(
            quarantine
                .leases()
                .iter()
                .filter(|lease| lease.owned_entities().contains(&parent))
                .count(),
            1
        );
    }

    #[test]
    fn reconciliation_authority_loss_disjoint_quarantine_closures_keep_separate_facts() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let router = install_nested_router(&mut app, observed, true);
        app.update();
        commit_initial_nested_branch(&mut app, router);
        let request = prepare_nested_branch_for_commit(
            &mut app,
            router,
            RouteLocation::new("/items/2").with_fragment("disjoint-quarantine"),
        );
        let (host, top_slot, first, second, third) = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            let mut candidates = entry
                .active
                .as_ref()
                .unwrap()
                .claims()
                .map(|claim| claim.entity())
                .filter(|entity| app.world().get::<ChildOf>(*entity).is_some());
            let first = candidates.next().unwrap();
            let second = candidates.find(|entity| *entity != first).unwrap();
            let third = candidates
                .find(|entity| *entity != first && *entity != second)
                .unwrap();
            (
                entry.host.as_ref().unwrap().host,
                entry.host.as_ref().unwrap().top_slot,
                first,
                second,
                third,
            )
        };
        app.world_mut().entity_mut(first).remove::<ChildOf>();
        app.world_mut().entity_mut(second).remove::<ChildOf>();
        app.world_mut().entity_mut(third).remove::<ChildOf>();
        app.world_mut().flush();
        app.world_mut()
            .entity_mut(first)
            .insert(NestedAppHostSentinel(0xd1));
        app.world_mut()
            .entity_mut(second)
            .insert(NestedAppHostSentinel(0xd2));
        app.world_mut()
            .entity_mut(third)
            .insert(NestedAppHostSentinel(0xd3));
        let foreign = app.world_mut().spawn(NestedAppHostSentinel(0xdf)).id();
        app.world_mut().entity_mut(first).add_child(foreign);
        let registration = app
            .world_mut()
            .entity_mut(host)
            .take::<RouterHostRegistration>()
            .unwrap();
        app.world_mut().entity_mut(host).insert(registration);
        commit_presentation_system(app.world_mut());

        let registry_before = app
            .world()
            .resource::<RouteEntityRegistry>()
            .records()
            .to_vec();
        let top_parent_before = app.world().get::<ChildOf>(top_slot).map(ChildOf::parent);
        let presentation_before = [first, second, third].map(|entity| {
            (
                app.world().get::<Visibility>(entity).copied(),
                app.world().get::<Pickable>(entity).copied(),
                app.world().get::<RouterInputBarrier>(entity).copied(),
            )
        });
        let trace_before = app.world().resource::<RouterNavigation>().entries[0]
            .engine
            .request(request)
            .unwrap()
            .trace()
            .len();
        fail_next_authority_publication_capacity_reservation_for_test();
        settle_cleanup_system(app.world_mut());
        {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            assert_eq!(entry.cleanup_ledger.as_ref().unwrap().receipt_count(), 0);
            assert_eq!(
                entry.engine.request(request).unwrap().trace().len(),
                trace_before
            );
        }
        assert_eq!(
            app.world().resource::<RouteEntityRegistry>().records(),
            registry_before
        );
        assert!(
            app.world()
                .resource::<QuarantineLeaseTable>()
                .leases()
                .is_empty()
        );
        assert_eq!(
            app.world().get::<ChildOf>(top_slot).map(ChildOf::parent),
            top_parent_before
        );
        assert_eq!(
            [first, second, third].map(|entity| {
                (
                    app.world().get::<Visibility>(entity).copied(),
                    app.world().get::<Pickable>(entity).copied(),
                    app.world().get::<RouterInputBarrier>(entity).copied(),
                )
            }),
            presentation_before
        );

        for _ in 0..16 {
            settle_cleanup_system(app.world_mut());
            if app.world().resource::<RouterNavigation>().entries[0]
                .cleanup_ledger
                .is_none()
            {
                break;
            }
        }

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert!(
            matches!(
                entry.engine.request(request).unwrap().status(),
                RequestStatus::Failed(failure)
                    if failure.kind() == NavigationFailureKind::AuthorityLost
            ),
            "status={:?} ledger={:?}",
            entry.engine.request(request).unwrap().status(),
            entry
                .cleanup_ledger
                .as_ref()
                .map(|ledger| (ledger.receipt_count(), ledger.is_settled()))
        );
        let quarantine = app.world().resource::<QuarantineLeaseTable>();
        let first_lease = quarantine
            .leases()
            .iter()
            .find(|lease| lease.owned_entities().contains(&first))
            .unwrap();
        let second_lease = quarantine
            .leases()
            .iter()
            .find(|lease| lease.owned_entities().contains(&second))
            .unwrap();
        let third_lease = quarantine
            .leases()
            .iter()
            .find(|lease| lease.owned_entities().contains(&third))
            .unwrap();
        assert!(!std::ptr::eq(first_lease, second_lease));
        assert!(!std::ptr::eq(first_lease, third_lease));
        assert!(!std::ptr::eq(second_lease, third_lease));
        let first_owned = first_lease
            .owned_entities()
            .into_iter()
            .collect::<std::collections::HashSet<_>>();
        let second_owned = second_lease
            .owned_entities()
            .into_iter()
            .collect::<std::collections::HashSet<_>>();
        let third_owned = third_lease
            .owned_entities()
            .into_iter()
            .collect::<std::collections::HashSet<_>>();
        assert!(first_owned.is_disjoint(&second_owned));
        assert!(first_owned.is_disjoint(&third_owned));
        assert!(second_owned.is_disjoint(&third_owned));
        assert!(first_lease.foreign_descendants().contains(&foreign));
        assert!(!second_lease.foreign_descendants().contains(&foreign));
        assert!(!third_lease.foreign_descendants().contains(&foreign));
        assert_eq!(
            app.world().get::<NestedAppHostSentinel>(first),
            Some(&NestedAppHostSentinel(0xd1))
        );
        assert_eq!(
            app.world().get::<NestedAppHostSentinel>(second),
            Some(&NestedAppHostSentinel(0xd2))
        );
        assert_eq!(
            app.world().get::<NestedAppHostSentinel>(third),
            Some(&NestedAppHostSentinel(0xd3))
        );
        assert_eq!(
            app.world().get::<ChildOf>(foreign).map(ChildOf::parent),
            Some(first)
        );
        assert_eq!(
            take_authority_postmutation_reservation_attempts_for_test(),
            0,
            "an exact-capacity authority tail must not reserve or grow after mutation starts"
        );
        assert_eq!(
            take_authority_postmutation_manifest_materializations_for_test(),
            0,
            "verified closure checkpoints must not materialize, collect, sort, or deduplicate manifests after mutation starts"
        );
    }

    #[test]
    fn reconciliation_authority_first_residual_drift_preserves_receipts_and_burns_unused_ids() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let router = install_nested_router(&mut app, observed, true);
        app.update();
        commit_initial_nested_branch(&mut app, router);
        let request = prepare_nested_branch_for_commit(
            &mut app,
            router,
            RouteLocation::new("/items/2").with_fragment("aggregate-first-closure-drift"),
        );
        let (host, first, second, third) = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            let mut candidates = entry
                .active
                .as_ref()
                .unwrap()
                .claims()
                .map(|claim| claim.entity())
                .filter(|entity| app.world().get::<ChildOf>(*entity).is_some());
            let first = candidates.next().unwrap();
            let second = candidates.find(|entity| *entity != first).unwrap();
            let third = candidates
                .find(|entity| *entity != first && *entity != second)
                .unwrap();
            (entry.host.as_ref().unwrap().host, first, second, third)
        };
        for (entity, marker) in [(first, 0xe1), (second, 0xe2), (third, 0xe3)] {
            app.world_mut().entity_mut(entity).remove::<ChildOf>();
            app.world_mut()
                .entity_mut(entity)
                .insert(NestedAppHostSentinel(marker));
        }
        app.world_mut().flush();
        let registration = app
            .world_mut()
            .entity_mut(host)
            .take::<RouterHostRegistration>()
            .unwrap();
        app.world_mut().entity_mut(host).insert(registration);
        commit_presentation_system(app.world_mut());
        app.world_mut().insert_resource(
            AuthorityResidualPostwriteDrift::invalidate_after_first_closure([first, second, third]),
        );

        for _ in 0..16 {
            settle_cleanup_system(app.world_mut());
            if !app
                .world()
                .resource::<QuarantineLeaseTable>()
                .leases()
                .is_empty()
            {
                break;
            }
        }

        let (first_receipts, first_burn_end, first_lease_sequences) = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            let table = app.world().resource::<QuarantineLeaseTable>();
            (
                entry.cleanup_ledger.as_ref().unwrap().receipt_count(),
                table.next_sequence_for_test(),
                table
                    .leases()
                    .iter()
                    .map(|lease| lease.id_for_test().sequence_for_test())
                    .collect::<Vec<_>>(),
            )
        };
        assert!(first_receipts > 0);
        assert_eq!(first_lease_sequences.len(), 1);
        assert!(first_burn_end > first_lease_sequences[0]);

        for _ in 0..16 {
            settle_cleanup_system(app.world_mut());
            if app.world().resource::<RouterNavigation>().entries[0]
                .cleanup_ledger
                .is_none()
            {
                break;
            }
        }
        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        let fixed_receipts = entry.cleanup_ledger.as_ref().unwrap().receipt_count();
        assert!(fixed_receipts >= first_receipts);
        assert!(matches!(
            entry.engine.request(request).unwrap().status(),
            RequestStatus::Running(NavigationPhase::Recovering)
        ));
        let table = app.world().resource::<QuarantineLeaseTable>();
        let sequences = table
            .leases()
            .iter()
            .map(|lease| lease.id_for_test().sequence_for_test())
            .collect::<Vec<_>>();
        assert!(
            sequences
                .iter()
                .filter(|sequence| !first_lease_sequences.contains(sequence))
                .all(|sequence| *sequence > first_burn_end)
        );
        assert!(table.next_sequence_for_test() > sequences.len() as u64);
        settle_cleanup_system(app.world_mut());
        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert_eq!(
            entry.cleanup_ledger.as_ref().unwrap().receipt_count(),
            fixed_receipts,
            "the stopped attempt must not replay already completed receipts"
        );
    }

    #[test]
    fn reconciliation_authority_three_lost_closures_revalidate_later_facts_and_burn_unused_ids() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let router = install_nested_router(&mut app, observed, true);
        app.update();
        commit_initial_nested_branch(&mut app, router);
        let request = prepare_nested_branch_for_commit(
            &mut app,
            router,
            RouteLocation::new("/items/2").with_fragment("three-lost-closure-drift"),
        );
        let (host, lost) = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            let mut candidates = entry
                .active
                .as_ref()
                .unwrap()
                .claims()
                .map(|claim| claim.entity())
                .filter(|entity| app.world().get::<ChildOf>(*entity).is_some());
            let first = candidates.next().unwrap();
            let second = candidates.find(|entity| *entity != first).unwrap();
            let third = candidates
                .find(|entity| *entity != first && *entity != second)
                .unwrap();
            (entry.host.as_ref().unwrap().host, [first, second, third])
        };
        for entity in lost {
            app.world_mut().entity_mut(entity).remove::<ChildOf>();
            let stale_seal = app
                .world_mut()
                .resource_mut::<RouteEntityRegistry>()
                .issue_seal();
            app.world_mut()
                .entity_mut(entity)
                .insert(RouteOwnershipSeal::new(stale_seal));
        }
        app.world_mut().flush();
        let drift_parent = app.world_mut().spawn(NestedAppHostSentinel(0xef)).id();
        let registration = app
            .world_mut()
            .entity_mut(host)
            .take::<RouterHostRegistration>()
            .unwrap();
        app.world_mut().entity_mut(host).insert(registration);
        commit_presentation_system(app.world_mut());
        app.world_mut().insert_resource(
            AuthorityLostResidualPostwriteDrift::reparent_after_first_closure(lost, drift_parent),
        );

        settle_cleanup_system(app.world_mut());

        let first_tombstones = lost
            .iter()
            .filter_map(|entity| {
                app.world()
                    .resource::<QuarantineLeaseTable>()
                    .authority_lost_tombstone(*entity)
                    .map(|tombstone| (*entity, tombstone.id()))
            })
            .collect::<Vec<_>>();
        assert_eq!(
            first_tombstones.len(),
            1,
            "the old attempt must stop before publishing stale later lost facts"
        );
        let burn_end = app
            .world()
            .resource::<QuarantineLeaseTable>()
            .next_residual_sequence_for_test();
        assert!(first_tombstones[0].1.sequence_for_test() < burn_end);
        assert!(matches!(
            app.world().resource::<RouterNavigation>().entries[0]
                .engine
                .request(request)
                .unwrap()
                .status(),
            RequestStatus::Running(NavigationPhase::Recovering)
        ));
        let fixed_receipts = app.world().resource::<RouterNavigation>().entries[0]
            .cleanup_ledger
            .as_ref()
            .unwrap()
            .receipt_count();
        settle_cleanup_system(app.world_mut());
        let table = app.world().resource::<QuarantineLeaseTable>();
        assert_eq!(
            table
                .authority_lost_tombstone(first_tombstones[0].0)
                .map(|tombstone| tombstone.id()),
            Some(first_tombstones[0].1),
            "a truthful completed lost receipt must not replay"
        );
        for entity in lost
            .iter()
            .filter(|entity| **entity != first_tombstones[0].0)
        {
            assert!(
                table.authority_lost_tombstone(*entity).is_none(),
                "the stopped attempt must not publish a later stale tombstone"
            );
        }
        assert!(table.next_residual_sequence_for_test() >= burn_end);
        assert_eq!(
            app.world().resource::<RouterNavigation>().entries[0]
                .cleanup_ledger
                .as_ref()
                .unwrap()
                .receipt_count(),
            fixed_receipts,
            "the stopped attempt must not replay or add a stale receipt"
        );
    }

    #[test]
    fn reconciliation_authority_later_lost_closure_restored_exact_stops_stale_tombstone() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let router = install_nested_router(&mut app, observed, true);
        app.update();
        commit_initial_nested_branch(&mut app, router);
        let request = prepare_nested_branch_for_commit(
            &mut app,
            router,
            RouteLocation::new("/items/2").with_fragment("later-lost-restored-exact"),
        );
        let (host, lost_claims) = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            let mut claims = entry
                .active
                .as_ref()
                .unwrap()
                .claims()
                .filter(|claim| {
                    app.world().get::<ChildOf>(claim.entity()).is_some()
                        && app.world().get::<FrameLifecycle>(claim.entity()).is_some()
                })
                .take(3)
                .cloned()
                .collect::<Vec<_>>();
            assert_eq!(claims.len(), 3);
            let obligation_order =
                crate::cleanup::FrozenCleanupManifest::from_authority_settlement(
                    app.world(),
                    app.world().resource::<RouteEntityRegistry>(),
                    &entry.pending_commit.as_ref().unwrap().branch,
                    entry.active.as_ref().unwrap(),
                    entry.host.as_ref().unwrap(),
                )
                .expect("three lost frame claims must freeze into one authority manifest")
                .entities()
                .to_vec();
            claims.sort_by_key(|claim| {
                obligation_order
                    .iter()
                    .position(|entity| *entity == claim.entity())
                    .unwrap()
            });
            (entry.host.as_ref().unwrap().host, claims)
        };
        for claim in &lost_claims {
            app.world_mut()
                .entity_mut(claim.entity())
                .remove::<ChildOf>();
            let stale_seal = app
                .world_mut()
                .resource_mut::<RouteEntityRegistry>()
                .issue_seal();
            app.world_mut().entity_mut(claim.entity()).insert((
                RouteOwnershipSeal::new(stale_seal),
                FrameLifecycle(EntityLifecycle::Leaving),
            ));
        }
        app.world_mut().flush();
        let later_claim = lost_claims[1].clone();
        let registration = app
            .world_mut()
            .entity_mut(host)
            .take::<RouterHostRegistration>()
            .unwrap();
        app.world_mut().entity_mut(host).insert(registration);
        commit_presentation_system(app.world_mut());
        app.world_mut().insert_resource(
            AuthorityLostResidualPhysicalRestoreDrift::restore_after_first_closure(
                later_claim.clone(),
            ),
        );

        settle_cleanup_system(app.world_mut());

        let first_tombstones = lost_claims
            .iter()
            .filter_map(|claim| {
                app.world()
                    .resource::<QuarantineLeaseTable>()
                    .authority_lost_tombstone(claim.entity())
                    .map(|tombstone| (claim.entity(), tombstone.id()))
            })
            .collect::<Vec<_>>();
        assert_eq!(
            first_tombstones.len(),
            1,
            "the old attempt must stop when a later lost claim becomes physically exact"
        );
        let table = app.world().resource::<QuarantineLeaseTable>();
        let burn_end = table.next_residual_sequence_for_test();
        assert!(first_tombstones[0].1.sequence_for_test() < burn_end);
        assert!(
            table
                .authority_lost_tombstone(later_claim.entity())
                .is_none()
        );
        assert!(table.record(later_claim.entity()).is_none());
        assert!(app.world().get_entity(later_claim.entity()).is_ok());
        assert_eq!(
            app.world()
                .resource::<RouteEntityRegistry>()
                .record(later_claim.entity())
                .map(|record| record.claim()),
            Some(later_claim.clone())
        );
        assert_eq!(
            app.world().get::<RouteOwnershipSeal>(later_claim.entity()),
            Some(&RouteOwnershipSeal::new(later_claim.seal()))
        );
        assert_eq!(
            app.world().get::<FrameLifecycle>(later_claim.entity()),
            Some(&FrameLifecycle(later_claim.lifecycle()))
        );
        assert!(matches!(
            app.world().resource::<RouterNavigation>().entries[0]
                .engine
                .request(request)
                .unwrap()
                .status(),
            RequestStatus::Running(NavigationPhase::Recovering)
        ));
        let fixed_receipts = app.world().resource::<RouterNavigation>().entries[0]
            .cleanup_ledger
            .as_ref()
            .unwrap()
            .receipt_count();
        settle_cleanup_system(app.world_mut());
        let table = app.world().resource::<QuarantineLeaseTable>();
        assert_eq!(
            table
                .authority_lost_tombstone(first_tombstones[0].0)
                .map(|tombstone| tombstone.id()),
            Some(first_tombstones[0].1),
            "the truthful first receipt must not replay"
        );
        assert!(
            table
                .authority_lost_tombstone(later_claim.entity())
                .is_none()
        );
        assert!(table.next_residual_sequence_for_test() >= burn_end);
        assert_eq!(
            app.world().resource::<RouterNavigation>().entries[0]
                .cleanup_ledger
                .as_ref()
                .unwrap()
                .receipt_count(),
            fixed_receipts,
            "the stopped attempt must not add a stale later receipt"
        );
        assert!(app.world().get_entity(later_claim.entity()).is_ok());
        assert_eq!(
            app.world()
                .resource::<RouteEntityRegistry>()
                .record(later_claim.entity())
                .map(|record| record.claim()),
            Some(later_claim)
        );
    }

    #[test]
    fn reconciliation_authority_mixed_verified_lost_closure_revalidates_both_local_sides() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let router = install_deep_nested_router(&mut app);
        app.update();
        commit_initial_deep_nested_branch(&mut app, router);
        let request = prepare_deep_nested_branch_for_commit(
            &mut app,
            router,
            RouteLocation::new("/sections/a/items/2").with_fragment("mixed-verified-lost-drift"),
        );
        let (host, first_verified, mixed_verified, mixed_lost) = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            let active = entry.active.as_ref().unwrap();
            let first_verified = active.frames()[2]
                .claims()
                .map(|claim| claim.entity())
                .find(|entity| {
                    *entity != active.frames()[2].shell()
                        && app.world().get::<ChildOf>(*entity).is_some()
                })
                .unwrap();
            let mixed_verified = active.frames()[0].shell();
            let mixed_lost = active.frames()[0]
                .claims()
                .map(|claim| claim.entity())
                .find(|entity| {
                    app.world().get::<ChildOf>(*entity).map(ChildOf::parent) == Some(mixed_verified)
                })
                .unwrap();
            (
                entry.host.as_ref().unwrap().host,
                first_verified,
                mixed_verified,
                mixed_lost,
            )
        };
        app.world_mut()
            .entity_mut(first_verified)
            .remove::<ChildOf>();
        app.world_mut()
            .entity_mut(first_verified)
            .insert(NestedAppHostSentinel(0xf1));
        app.world_mut()
            .entity_mut(mixed_verified)
            .insert(NestedAppHostSentinel(0xf2));
        let stable_mixed_child = app.world_mut().spawn(NestedAppHostSentinel(0xf3)).id();
        app.world_mut()
            .entity_mut(mixed_verified)
            .add_child(stable_mixed_child);
        let stale_seal = app
            .world_mut()
            .resource_mut::<RouteEntityRegistry>()
            .issue_seal();
        app.world_mut()
            .entity_mut(mixed_lost)
            .insert(RouteOwnershipSeal::new(stale_seal));
        app.world_mut().flush();
        let drift_parent = app.world_mut().spawn(NestedAppHostSentinel(0xff)).id();
        let registration = app
            .world_mut()
            .entity_mut(host)
            .take::<RouterHostRegistration>()
            .unwrap();
        app.world_mut().entity_mut(host).insert(registration);
        commit_presentation_system(app.world_mut());
        app.world_mut().insert_resource(
            AuthorityLostResidualPostwriteDrift::reparent_after_first_closure(
                [mixed_lost, mixed_lost, mixed_lost],
                drift_parent,
            ),
        );
        take_authority_postmutation_manifest_materializations_for_test();

        settle_cleanup_system(app.world_mut());

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert!(matches!(
            entry.engine.request(request).unwrap().status(),
            RequestStatus::Running(NavigationPhase::Recovering)
        ));
        let table = app.world().resource::<QuarantineLeaseTable>();
        assert!(table.record(first_verified).is_some());
        assert!(
            table.record(mixed_verified).is_none(),
            "the later mixed closure's verified lease must stay closure-local and unpublished"
        );
        assert!(
            table.authority_lost_tombstone(mixed_lost).is_none(),
            "the later mixed closure's drifted lost fact must stay unpublished"
        );
        assert!(
            app.world()
                .resource::<RouteEntityRegistry>()
                .record(mixed_verified)
                .is_some(),
            "stopping on the lost side must preserve the verified side's exact current claim"
        );
        assert_eq!(
            take_authority_postmutation_manifest_materializations_for_test(),
            0,
            "mixed closure checkpoints must use only frozen no-allocation manifest evidence"
        );
    }

    #[test]
    fn reconciliation_authority_loss_postwrite_reparent_stops_old_revision_after_one_receipt() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let router = install_nested_router(&mut app, observed, true);
        app.update();
        commit_initial_nested_branch(&mut app, router);
        let request = prepare_nested_branch_for_commit(
            &mut app,
            router,
            RouteLocation::new("/items/2").with_fragment("postwrite-reparent"),
        );
        let (host, top_slot) = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            let host = entry.host.as_ref().unwrap();
            (host.host, host.top_slot)
        };
        let sentinel = app.world_mut().spawn(NestedAppHostSentinel(0xc2)).id();
        let registration = app
            .world_mut()
            .entity_mut(host)
            .take::<RouterHostRegistration>()
            .unwrap();
        app.world_mut().entity_mut(host).insert(registration);
        commit_presentation_system(app.world_mut());
        app.world_mut().insert_resource(
            crate::cleanup::AuthorityRemovalPostwriteDrift::reparent_after_first_removal(
                sentinel, top_slot,
            ),
        );

        settle_cleanup_system(app.world_mut());

        {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            let ledger = entry.cleanup_ledger.as_ref().unwrap();
            assert_eq!(ledger.receipt_count(), 1);
            assert_eq!(
                entry.engine.snapshot().availability(),
                &RouterAvailability::RecoveringAuthority
            );
        }
        assert_eq!(
            app.world().get::<ChildOf>(sentinel).map(ChildOf::parent),
            Some(top_slot)
        );
        assert!(app.world().get_entity(top_slot).is_ok());

        for _ in 0..16 {
            settle_cleanup_system(app.world_mut());
            if app.world().resource::<RouterNavigation>().entries[0]
                .cleanup_ledger
                .is_none()
            {
                break;
            }
        }
        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert!(matches!(
            entry.engine.request(request).unwrap().status(),
            RequestStatus::Failed(failure)
                if failure.kind() == NavigationFailureKind::AuthorityLost
        ));
        assert!(entry.cleanup_ledger.is_none());
        assert!(app.world().get_entity(top_slot).is_ok());
        assert!(app.world().get_entity(sentinel).is_ok());
        assert_eq!(
            app.world().get::<ChildOf>(sentinel).map(ChildOf::parent),
            Some(top_slot)
        );
        assert!(
            app.world()
                .resource::<QuarantineLeaseTable>()
                .record(top_slot)
                .is_some()
        );
        let terminal_trace_len = entry.engine.request(request).unwrap().trace().len();
        for _ in 0..3 {
            settle_cleanup_system(app.world_mut());
        }
        assert_eq!(
            app.world().resource::<RouterNavigation>().entries[0]
                .engine
                .request(request)
                .unwrap()
                .trace()
                .len(),
            terminal_trace_len
        );
    }

    #[test]
    fn reconciliation_authority_first_removal_registry_replacement_preserves_later_entity_and_claim()
     {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let router = install_nested_router(&mut app, observed, true);
        app.update();
        commit_initial_nested_branch(&mut app, router);
        let request = prepare_nested_branch_for_commit(
            &mut app,
            router,
            RouteLocation::new("/items/2").with_fragment("postwrite-registry-replacement"),
        );
        let (host, top_slot, old_claim) = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            let host = entry.host.as_ref().unwrap();
            let claim = app
                .world()
                .resource::<RouteEntityRegistry>()
                .record(host.top_slot)
                .unwrap()
                .claim();
            (host.host, host.top_slot, claim)
        };
        let registration = app
            .world_mut()
            .entity_mut(host)
            .take::<RouterHostRegistration>()
            .unwrap();
        app.world_mut().entity_mut(host).insert(registration);
        commit_presentation_system(app.world_mut());
        app.world_mut().insert_resource(
            AuthorityRemovalRegistryPostwriteDrift::replace_after_first_removal(top_slot),
        );

        settle_cleanup_system(app.world_mut());

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert_eq!(entry.cleanup_ledger.as_ref().unwrap().receipt_count(), 1);
        assert!(matches!(
            entry.engine.request(request).unwrap().status(),
            RequestStatus::Running(NavigationPhase::Recovering)
        ));
        assert!(app.world().get_entity(top_slot).is_ok());
        let replacement = app
            .world()
            .resource::<RouteEntityRegistry>()
            .record(top_slot)
            .expect("later removal's replacement claim must remain current")
            .claim();
        assert_ne!(replacement, old_claim);

        assert_eq!(entry.cleanup_ledger.as_ref().unwrap().receipt_count(), 1);
    }

    #[test]
    fn reconciliation_authority_loss_top_slot_reattach_drift_stays_nonterminal() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let router = install_atomic_router(&mut app, "authority-top-slot-reattach");
        app.update();
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        app.update();
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(
                router,
                RouteLocation::new("/").with_fragment("top-slot-reattach"),
            )
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        let (host, top_slot) = {
            let navigation = app.world().resource::<RouterNavigation>();
            let binding = navigation.entries[0].host.as_ref().unwrap();
            (binding.host, binding.top_slot)
        };
        let registration = app
            .world_mut()
            .entity_mut(host)
            .take::<RouterHostRegistration>()
            .unwrap();
        app.world_mut().entity_mut(host).insert(registration);
        commit_presentation_system(app.world_mut());
        app.world_mut()
            .insert_resource(crate::cleanup::AuthorityHostEdgePostwriteDrift::reattach(
                top_slot, host,
            ));

        settle_cleanup_system(app.world_mut());

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert_eq!(
            entry.engine.snapshot().availability(),
            &RouterAvailability::RecoveringAuthority,
            "status={:?}, ledger={:?}, top_parent={:?}",
            entry.engine.request(request).unwrap().status(),
            entry
                .cleanup_ledger
                .as_ref()
                .map(|ledger| (ledger.is_settled(), ledger.receipt_count())),
            app.world().get::<ChildOf>(top_slot).map(ChildOf::parent),
        );
        assert!(entry.cleanup_ledger.is_some());
        assert!(matches!(
            entry.engine.request(request).unwrap().status(),
            RequestStatus::Running(NavigationPhase::Recovering)
        ));
        assert_eq!(
            app.world().get::<ChildOf>(top_slot).map(ChildOf::parent),
            Some(host)
        );
        assert!(
            app.world()
                .get::<Children>(host)
                .unwrap()
                .contains(&top_slot)
        );

        for _ in 0..16 {
            settle_cleanup_system(app.world_mut());
            if app.world().resource::<RouterNavigation>().entries[0]
                .cleanup_ledger
                .is_none()
            {
                break;
            }
        }
        assert!(
            app.world().resource::<RouterNavigation>().entries[0]
                .cleanup_ledger
                .is_none(),
            "status={:?} ledger={:?} parent={:?}",
            app.world().resource::<RouterNavigation>().entries[0]
                .engine
                .request(request)
                .unwrap()
                .status(),
            app.world().resource::<RouterNavigation>().entries[0]
                .cleanup_ledger
                .as_ref()
                .map(|ledger| (ledger.receipt_count(), ledger.is_settled())),
            app.world().get::<ChildOf>(top_slot).map(ChildOf::parent)
        );
    }

    #[test]
    fn reconciliation_authority_loss_top_slot_protected_parent_is_zero_relationship_write() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let router = install_atomic_router(&mut app, "authority-top-slot-protected");
        app.update();
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        app.update();
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(
                router,
                RouteLocation::new("/").with_fragment("top-slot-protected"),
            )
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        let (host, top_slot) = {
            let navigation = app.world().resource::<RouterNavigation>();
            let binding = navigation.entries[0].host.as_ref().unwrap();
            (binding.host, binding.top_slot)
        };
        let protected_parent = app.world_mut().spawn(NestedAppHostSentinel(0xa11)).id();
        let sibling = app.world_mut().spawn(NestedAppHostSentinel(0xa12)).id();
        app.world_mut()
            .entity_mut(protected_parent)
            .add_child(sibling);
        app.world_mut()
            .entity_mut(protected_parent)
            .add_child(top_slot);
        let protected_children = app
            .world()
            .get::<Children>(protected_parent)
            .unwrap()
            .to_vec();
        let registration = app
            .world_mut()
            .entity_mut(host)
            .take::<RouterHostRegistration>()
            .unwrap();
        app.world_mut().entity_mut(host).insert(registration);
        commit_presentation_system(app.world_mut());
        for _ in 0..6 {
            settle_cleanup_system(app.world_mut());
            if app.world().resource::<RouterNavigation>().entries[0]
                .cleanup_ledger
                .is_none()
            {
                break;
            }
        }

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert!(matches!(
            entry.engine.request(request).unwrap().status(),
            RequestStatus::Failed(failure)
                if failure.kind() == NavigationFailureKind::AuthorityLost
        ));
        assert_eq!(
            app.world().get::<ChildOf>(top_slot).map(ChildOf::parent),
            Some(protected_parent)
        );
        assert_eq!(
            app.world()
                .get::<Children>(protected_parent)
                .unwrap()
                .to_vec(),
            protected_children
        );
        assert!(
            app.world()
                .resource::<QuarantineLeaseTable>()
                .record(top_slot)
                .is_some()
        );
    }

    #[test]
    fn reconciliation_authority_loss_retains_lost_top_slot_edge_without_host_write() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let router = install_atomic_router(&mut app, "authority-top-slot-lost");
        app.update();
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        app.update();
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(
                router,
                RouteLocation::new("/").with_fragment("top-slot-lost"),
            )
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        let (host, top_slot, host_children_before) = {
            let navigation = app.world().resource::<RouterNavigation>();
            let binding = navigation.entries[0].host.as_ref().unwrap();
            (
                binding.host,
                binding.top_slot,
                app.world().get::<Children>(binding.host).unwrap().to_vec(),
            )
        };
        let stale_seal = app
            .world_mut()
            .resource_mut::<RouteEntityRegistry>()
            .issue_seal();
        app.world_mut()
            .entity_mut(top_slot)
            .insert(RouteOwnershipSeal::new(stale_seal));
        commit_presentation_system(app.world_mut());

        settle_cleanup_system(app.world_mut());

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert!(matches!(
            entry.engine.request(request).unwrap().status(),
            RequestStatus::Failed(failure)
                if failure.kind() == NavigationFailureKind::AuthorityLost
        ));
        assert!(entry.cleanup_ledger.is_none());
        assert_eq!(
            app.world().get::<ChildOf>(top_slot).map(ChildOf::parent),
            Some(host)
        );

        for _ in 0..4 {
            settle_cleanup_system(app.world_mut());
            if app.world().resource::<RouterNavigation>().entries[0]
                .cleanup_ledger
                .is_none()
            {
                break;
            }
        }
        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert!(matches!(
            entry.engine.request(request).unwrap().status(),
            RequestStatus::Failed(failure)
                if failure.kind() == NavigationFailureKind::AuthorityLost
        ));
        assert!(app.world().get_entity(top_slot).is_ok());
        assert_eq!(
            app.world().get::<ChildOf>(top_slot).map(ChildOf::parent),
            Some(host)
        );
        assert_eq!(
            app.world().get::<Children>(host).unwrap().to_vec(),
            host_children_before
        );
        assert!(
            app.world()
                .resource::<QuarantineLeaseTable>()
                .authority_lost_tombstone(top_slot)
                .is_some()
        );
    }

    #[test]
    fn reconciliation_authority_loss_detaches_old_top_when_registration_is_absent() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let router = install_nested_router(&mut app, observed, true);
        app.update();
        commit_initial_nested_branch(&mut app, router);
        let request = prepare_nested_branch_for_commit(
            &mut app,
            router,
            RouteLocation::new("/items/2").with_fragment("registration-absent"),
        );
        let (host, top_slot) = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            let host = entry.host.as_ref().unwrap();
            (host.host, host.top_slot)
        };
        app.world_mut()
            .entity_mut(host)
            .insert(NestedAppHostSentinel(0xc3));
        let sibling = app.world_mut().spawn(NestedAppHostSentinel(0xc4)).id();
        app.world_mut().entity_mut(host).add_child(sibling);
        let children_before = app.world().get::<Children>(host).unwrap().to_vec();
        app.world_mut()
            .entity_mut(host)
            .remove::<RouterHostRegistration>();
        commit_presentation_system(app.world_mut());

        for _ in 0..16 {
            settle_cleanup_system(app.world_mut());
            if app.world().resource::<RouterNavigation>().entries[0]
                .cleanup_ledger
                .is_none()
            {
                break;
            }
        }

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert!(matches!(
            entry.engine.request(request).unwrap().status(),
            RequestStatus::Failed(failure)
                if failure.kind() == NavigationFailureKind::AuthorityLost
        ));
        assert!(entry.cleanup_ledger.is_none());
        assert!(app.world().get_entity(top_slot).is_err());
        assert_eq!(
            app.world().get::<NestedAppHostSentinel>(host),
            Some(&NestedAppHostSentinel(0xc3))
        );
        assert_eq!(
            app.world().get::<NestedAppHostSentinel>(sibling),
            Some(&NestedAppHostSentinel(0xc4))
        );
        let expected = children_before
            .into_iter()
            .filter(|child| *child != top_slot)
            .collect::<Vec<_>>();
        assert_eq!(
            app.world().get::<Children>(host).unwrap().to_vec(),
            expected
        );
        assert!(app.world().get::<RouterHostRegistration>(host).is_none());
    }

    #[test]
    fn reconciliation_authority_loss_preserved_seal_drift_clears_authority_without_crossing_it() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let router = install_nested_router(&mut app, observed, true);
        app.update();
        commit_initial_nested_branch(&mut app, router);
        let (preserved_shell, history_before) = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            (
                entry.active.as_ref().unwrap().frames()[0].shell(),
                entry.engine.history().clone(),
            )
        };
        let current = prepare_nested_branch_for_commit(
            &mut app,
            router,
            RouteLocation::new("/items/2").with_fragment("preserved-seal-drift"),
        );
        let queued = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(
                router,
                RouteLocation::new("/items/3").with_fragment("queued-after-seal-drift"),
            )
            .unwrap();
        let stale_seal = app
            .world_mut()
            .resource_mut::<RouteEntityRegistry>()
            .issue_seal();
        app.world_mut()
            .entity_mut(preserved_shell)
            .insert(RouteOwnershipSeal::new(stale_seal));

        commit_presentation_system(app.world_mut());

        {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            assert_eq!(
                entry.engine.snapshot().availability(),
                &RouterAvailability::RecoveringAuthority
            );
            assert!(matches!(
                entry.engine.request(queued).unwrap().status(),
                RequestStatus::Failed(failure)
                    if failure.kind() == NavigationFailureKind::AuthorityLost
            ));
            assert_eq!(entry.engine.history(), &history_before);
        }

        for _ in 0..8 {
            settle_cleanup_system(app.world_mut());
            if matches!(
                app.world().resource::<RouterNavigation>().entries[0]
                    .engine
                    .request(current)
                    .unwrap()
                    .status(),
                RequestStatus::Failed(_)
            ) {
                break;
            }
        }

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert!(matches!(
            entry.engine.request(current).unwrap().status(),
            RequestStatus::Failed(failure)
                if failure.kind() == NavigationFailureKind::AuthorityLost
        ));
        assert!(entry.active.is_none());
        assert!(entry.host.is_none());
        assert_eq!(entry.engine.history(), &history_before);
        assert!(app.world().get_entity(preserved_shell).is_ok());
        assert_eq!(
            app.world().get::<RouteOwnershipSeal>(preserved_shell),
            Some(&RouteOwnershipSeal::new(stale_seal))
        );
    }

    #[test]
    fn reconciliation_authority_loss_inner_content_seal_drift_retires_claim_without_despawn() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let router = install_nested_router(&mut app, observed, true);
        app.update();
        commit_initial_nested_branch(&mut app, router);
        let (inner_content, history_before) = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            let frame = &entry.active.as_ref().unwrap().frames()[0];
            (
                frame
                    .claims()
                    .find(|claim| claim.entity() != frame.shell())
                    .unwrap()
                    .entity(),
                entry.engine.history().clone(),
            )
        };
        let current = prepare_nested_branch_for_commit(
            &mut app,
            router,
            RouteLocation::new("/items/2").with_fragment("inner-content-seal-drift"),
        );
        let stale_seal = app
            .world_mut()
            .resource_mut::<RouteEntityRegistry>()
            .issue_seal();
        app.world_mut()
            .entity_mut(inner_content)
            .insert(RouteOwnershipSeal::new(stale_seal));

        commit_presentation_system(app.world_mut());
        for _ in 0..8 {
            settle_cleanup_system(app.world_mut());
            if matches!(
                app.world().resource::<RouterNavigation>().entries[0]
                    .engine
                    .request(current)
                    .unwrap()
                    .status(),
                RequestStatus::Failed(_)
            ) {
                break;
            }
        }

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert!(matches!(
            entry.engine.request(current).unwrap().status(),
            RequestStatus::Failed(failure)
                if failure.kind() == NavigationFailureKind::AuthorityLost
        ));
        assert!(entry.active.is_none());
        assert!(entry.host.is_none());
        assert_eq!(entry.engine.history(), &history_before);
        assert!(app.world().get_entity(inner_content).is_ok());
        assert_eq!(
            app.world().get::<RouteOwnershipSeal>(inner_content),
            Some(&RouteOwnershipSeal::new(stale_seal))
        );
        assert!(
            app.world()
                .resource::<RouteEntityRegistry>()
                .record(inner_content)
                .is_none()
        );
        assert!(
            app.world()
                .resource::<QuarantineLeaseTable>()
                .authority_lost_tombstone(inner_content)
                .is_some()
        );
        let former_owner = app
            .world()
            .resource::<QuarantineLeaseTable>()
            .authority_lost_tombstone(inner_content)
            .unwrap()
            .former_claim()
            .owner()
            .clone();
        let manifest = ComponentManifest::from_world(app.world(), inner_content).unwrap();
        app.world_mut()
            .resource_mut::<RouteEntityRegistry>()
            .register(
                inner_content,
                former_owner,
                stale_seal,
                manifest,
                EntityLifecycle::Active,
            )
            .unwrap();
        assert!(
            app.world()
                .resource::<QuarantineLeaseTable>()
                .authority_lost_tombstone(inner_content)
                .is_some()
        );
        assert_eq!(
            crate::registry::observed_owned_state(app.world(), inner_content),
            crate::registry::AuditEntityState::PresentOwned
        );
    }

    #[test]
    fn reconciliation_protocol_contradiction_is_unavailable_residual_without_new_staging() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let router = install_atomic_router(&mut app, "authority-protocol-contradiction");
        app.update();
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        app.update();
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(
                router,
                RouteLocation::new("/").with_fragment("protocol-contradiction"),
            )
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        let (host_entity, old_targets) = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            let mut targets = entry.active.as_ref().unwrap().interactive_entities();
            targets.extend(
                entry
                    .pending_commit
                    .as_ref()
                    .unwrap()
                    .branch
                    .as_realized()
                    .interactive_entities(),
            );
            targets.sort_by_key(|entity| entity.to_bits());
            targets.dedup();
            (entry.host.as_ref().unwrap().host, targets)
        };
        let focus = old_targets[0];
        *app.world_mut().resource_mut::<InputFocus>() = InputFocus::from_entity(focus);
        app.world_mut()
            .resource_mut::<RouteFocusState>()
            .set_focused(Some(focus));
        {
            let mut navigation = app.world_mut().resource_mut::<RouterNavigation>();
            let entry = &mut navigation.entries[0];
            let foreign_owner = entry
                .active
                .as_ref()
                .unwrap()
                .claims()
                .next()
                .unwrap()
                .owner()
                .clone();
            let host = entry.host.as_mut().unwrap();
            host.top_slot_claim = host.top_slot_claim.with_owner_for_test(foreign_owner);
        }
        let registration = app
            .world_mut()
            .entity_mut(host_entity)
            .take::<RouterHostRegistration>()
            .unwrap();
        app.world_mut().entity_mut(host_entity).insert(registration);

        commit_presentation_system(app.world_mut());

        let entity_count = app.world().iter_entities().count();
        {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            assert_eq!(
                entry.engine.snapshot().availability(),
                &RouterAvailability::Unavailable(
                    RouterUnavailableReason::RegistryInvariantViolation
                )
            );
            assert!(matches!(
                entry.engine.request(request).unwrap().status(),
                RequestStatus::Failed(failure)
                    if failure.kind() == NavigationFailureKind::AuthorityLost
            ));
            assert!(entry.active.is_none());
            assert!(entry.pending_commit.is_none());
            assert!(entry.host.is_none());
            assert!(entry.authority_protocol_residual.is_some());
        }
        assert_eq!(app.world().resource::<InputFocus>().get(), None);
        assert_eq!(app.world().resource::<RouteFocusState>().focused(), None);
        let gate = app.world().resource::<RouterInputGate>();
        assert!(old_targets.iter().all(|entity| {
            !gate.allows_pointer(*entity)
                && !gate.allows_keyboard(*entity)
                && !gate.allows_gamepad(*entity)
        }));
        observe_authority_system(app.world_mut());
        assert_eq!(app.world().iter_entities().count(), entity_count);
        let error = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/blocked"))
            .unwrap_err();
        assert_eq!(error.kind(), NavigationEnqueueErrorKind::HostUnavailable);
        assert_eq!(app.world().iter_entities().count(), entity_count);
    }

    #[test]
    fn reconciliation_protocol_host_absent_is_bounded_unavailable_without_owner_loss() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let router = install_atomic_router(&mut app, "authority-protocol-host-absent");
        app.update();
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        app.update();
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/").with_fragment("host-absent"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        let dropped_host = app.world_mut().resource_mut::<RouterNavigation>().entries[0]
            .host
            .take()
            .expect("test begins with the exact linear host owner");
        drop(dropped_host);

        commit_presentation_system(app.world_mut());

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert_eq!(
            entry.engine.snapshot().availability(),
            &RouterAvailability::Unavailable(RouterUnavailableReason::RegistryInvariantViolation)
        );
        assert!(matches!(
            entry.engine.request(request).unwrap().status(),
            RequestStatus::Failed(failure)
                if failure.kind() == NavigationFailureKind::AuthorityLost
        ));
        let residual = entry.authority_protocol_residual.as_ref().unwrap();
        assert!(residual.host.is_none());
        assert!(!residual.active.as_ref().unwrap().frames().is_empty());
        assert!(
            !residual
                .pending
                .as_ref()
                .unwrap()
                .branch
                .frames()
                .is_empty()
        );
        let error = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/blocked"))
            .unwrap_err();
        assert_eq!(error.kind(), NavigationEnqueueErrorKind::HostUnavailable);
    }

    #[test]
    fn authority_lost_parent_replacement_preserves_exact_child_and_new_parent_claim() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let router = install_nested_router(&mut app, observed, true);
        app.update();
        commit_initial_nested_branch(&mut app, router);
        let (parent, parent_claim, child, child_claim, children_before) = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            let frame = &entry.active.as_ref().unwrap().frames()[0];
            let parent = frame.shell();
            let parent_claim = frame
                .claims()
                .find(|claim| claim.entity() == parent)
                .unwrap()
                .clone();
            let child_claim = frame
                .claims()
                .find(|claim| {
                    app.world()
                        .get::<ChildOf>(claim.entity())
                        .map(ChildOf::parent)
                        == Some(parent)
                })
                .unwrap()
                .clone();
            let child = child_claim.entity();
            let children = app.world().get::<Children>(parent).unwrap().to_vec();
            (parent, parent_claim, child, child_claim, children)
        };
        let request = prepare_nested_branch_for_commit(
            &mut app,
            router,
            RouteLocation::new("/items/2").with_fragment("parent-claim-replaced"),
        );
        let (new_seal, manifest) = {
            let mut registry = app.world_mut().resource_mut::<RouteEntityRegistry>();
            let manifest = registry.record(parent).unwrap().manifest().clone();
            let new_seal = registry.issue_seal();
            assert!(registry.remove_exact(&parent_claim));
            registry
                .register(
                    parent,
                    parent_claim.owner().clone(),
                    new_seal,
                    manifest.clone(),
                    EntityLifecycle::Active,
                )
                .unwrap();
            (new_seal, manifest)
        };
        app.world_mut()
            .entity_mut(parent)
            .insert(RouteOwnershipSeal::new(new_seal));
        let child_is_obligated = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            crate::cleanup::FrozenCleanupManifest::from_authority_settlement(
                app.world(),
                app.world().resource::<RouteEntityRegistry>(),
                &entry.pending_commit.as_ref().unwrap().branch,
                entry.active.as_ref().unwrap(),
                entry.host.as_ref().unwrap(),
            )
            .expect("parent claim replacement must still produce a terminal settlement manifest")
            .entities()
            .contains(&child)
        };

        commit_presentation_system(app.world_mut());
        for _ in 0..8 {
            settle_cleanup_system(app.world_mut());
            if matches!(
                app.world().resource::<RouterNavigation>().entries[0]
                    .engine
                    .request(request)
                    .unwrap()
                    .status(),
                RequestStatus::Failed(_)
            ) {
                break;
            }
        }

        assert!(app.world().get_entity(child).is_ok());
        assert_eq!(
            app.world().get::<ChildOf>(child).map(ChildOf::parent),
            Some(parent)
        );
        assert_eq!(
            app.world().get::<Children>(parent).unwrap().to_vec(),
            children_before
        );
        let parent_record = app
            .world()
            .resource::<RouteEntityRegistry>()
            .record(parent)
            .unwrap();
        assert_eq!(parent_record.seal(), new_seal);
        assert_eq!(parent_record.manifest(), &manifest);
        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert!(matches!(
            entry.engine.request(request).unwrap().status(),
            RequestStatus::Failed(failure)
                if failure.kind() == NavigationFailureKind::AuthorityLost
        ));
        let registry_current_exact = app
            .world()
            .resource::<RouteEntityRegistry>()
            .record(child)
            .is_some_and(|record| record.claim() == child_claim);
        let table = app.world().resource::<QuarantineLeaseTable>();
        let verified = table.record(child).is_some();
        let unresolved = table.unresolved_record(child).is_some();
        let authority_lost = table.authority_lost_tombstone(child).is_some();
        assert!(
            child_is_obligated,
            "an exact child below a replaced parent must remain in the frozen settlement"
        );
        assert!(!registry_current_exact);
        assert!(verified);
        assert!(!unresolved && !authority_lost);
    }

    #[test]
    fn authority_completion_transfers_owner_and_does_not_replay_receipts() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let router = install_atomic_router(&mut app, "authority-completion-error");
        app.update();
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        app.update();
        let host = app.world().resource::<RouterNavigation>().entries[0]
            .host
            .as_ref()
            .unwrap()
            .host;
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(
                router,
                RouteLocation::new("/").with_fragment("completion-error"),
            )
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        let registration = app
            .world_mut()
            .entity_mut(host)
            .take::<RouterHostRegistration>()
            .unwrap();
        app.world_mut().entity_mut(host).insert(registration);
        commit_presentation_system(app.world_mut());

        settle_cleanup_system(app.world_mut());

        let receipt_events = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            assert!(
                entry.cleanup_ledger.is_none(),
                "status={:?} ledger={:?}",
                entry.engine.request(request).unwrap().status(),
                entry
                    .cleanup_ledger
                    .as_ref()
                    .map(|ledger| (ledger.receipt_count(), ledger.is_settled()))
            );
            assert!(matches!(
                entry.engine.request(request).unwrap().status(),
                RequestStatus::Failed(failure)
                    if failure.kind() == NavigationFailureKind::AuthorityLost
            ));
            entry
                .engine
                .request(request)
                .unwrap()
                .trace()
                .iter()
                .filter(|event| event.cleanup_disposition().is_some())
                .count()
        };
        settle_cleanup_system(app.world_mut());
        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert!(entry.cleanup_ledger.is_none());
        assert!(matches!(
            entry.engine.request(request).unwrap().status(),
            RequestStatus::Failed(failure)
                if failure.kind() == NavigationFailureKind::AuthorityLost
        ));
        assert_eq!(
            entry
                .engine
                .request(request)
                .unwrap()
                .trace()
                .iter()
                .filter(|event| event.cleanup_disposition().is_some())
                .count(),
            receipt_events
        );
    }

    #[test]
    fn reconciliation_leaving_foreign_descendant_commits_and_quarantines_owned_target() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let router = install_atomic_router(&mut app, "leaving-foreign-descendant");
        app.update();
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        app.update();
        let (leaving, leaving_output) = {
            let navigation = app.world().resource::<RouterNavigation>();
            let frame = navigation.entries[0].active.as_ref().unwrap();
            (
                frame.first_frame().unwrap().shell(),
                frame.interactive_entities()[0],
            )
        };
        let foreign = app.world_mut().spawn_empty().id();
        app.world_mut()
            .entity_mut(leaving_output)
            .add_child(foreign);
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/").with_fragment("replacement"))
            .unwrap();

        app.update();

        let navigation = app.world().resource::<RouterNavigation>();
        let entry = &navigation.entries[0];
        assert!(
            entry.pending_commit.is_none(),
            "leaving quarantine must not poll forever"
        );
        assert_ne!(
            entry
                .active
                .as_ref()
                .unwrap()
                .first_frame()
                .unwrap()
                .shell(),
            leaving
        );
        assert!(matches!(
            entry.engine.request(request).unwrap().status(),
            RequestStatus::Succeeded
        ));
        assert!(app.world().get_entity(foreign).is_ok());
        assert_eq!(
            app.world().get::<ChildOf>(foreign).map(ChildOf::parent),
            Some(leaving_output)
        );
        assert!(
            app.world()
                .resource::<QuarantineLeaseTable>()
                .record(leaving_output)
                .is_some()
        );
    }

    #[test]
    fn reconciliation_leaving_foreign_component_quarantines_without_app_hooks() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        install_foreign_component_hooks(&mut app);
        let router = install_atomic_router(&mut app, "leaving-foreign-component");
        app.update();
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        app.update();

        let (leaving_shell, leaving_output) = {
            let navigation = app.world().resource::<RouterNavigation>();
            let frame = navigation.entries[0].active.as_ref().unwrap();
            (
                frame.first_frame().unwrap().shell(),
                frame.interactive_entities()[0],
            )
        };
        app.world_mut()
            .entity_mut(leaving_output)
            .insert(FinalPreflightForeignComponent);
        assert!(
            validate_active_frame(
                app.world(),
                app.world().resource::<RouterNavigation>().entries[0]
                    .active
                    .as_ref()
                    .unwrap()
                    .first_frame()
                    .unwrap()
            ),
            "a protected foreign component must not invalidate a preserved active frame"
        );
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(
                router,
                RouteLocation::new("/").with_fragment("foreign-component-replacement"),
            )
            .unwrap();

        app.update();

        let navigation = app.world().resource::<RouterNavigation>();
        let entry = &navigation.entries[0];
        assert!(
            entry.pending_commit.is_none(),
            "request must not poll forever"
        );
        assert_ne!(
            entry
                .active
                .as_ref()
                .unwrap()
                .first_frame()
                .unwrap()
                .shell(),
            leaving_shell
        );
        assert!(matches!(
            entry.engine.request(request).unwrap().status(),
            RequestStatus::Succeeded
        ));
        assert!(
            app.world()
                .get::<FinalPreflightForeignComponent>(leaving_output)
                .is_some()
        );
        assert_eq!(app.world().resource::<ForeignComponentHooks>().removed, 0);
        assert_eq!(app.world().resource::<ForeignComponentHooks>().despawned, 0);
        assert!(
            app.world()
                .resource::<QuarantineLeaseTable>()
                .record(leaving_output)
                .is_some()
        );
        let gate = app.world().resource::<RouterInputGate>();
        assert!(!gate.allows_pointer(leaving_output));
        assert!(gate.allows_pointer(entry.active.as_ref().unwrap().interactive_entities()[0]));
    }

    fn nested_preserved_child_slot_boundary_drift_commits_without_mutation(
        drift: ChildSlotBoundaryDrift,
        fragment: &'static str,
    ) {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        install_foreign_component_hooks(&mut app);
        let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let router = install_nested_router(&mut app, observed, true);
        let host = app
            .world()
            .iter_entities()
            .find(|entity| entity.contains::<RouterHostRegistration>())
            .unwrap()
            .id();
        app.world_mut()
            .entity_mut(host)
            .insert(NestedAppHostSentinel(0xA0));
        let sibling = app.world_mut().spawn(NestedAppHostSentinel(0xA1)).id();
        app.world_mut().entity_mut(host).add_child(sibling);
        app.update();
        commit_initial_nested_branch(&mut app, router);

        let (preserved_slot, preserved_parent_shell, leaving_child_shell) = {
            let active = app.world().resource::<RouterNavigation>().entries[0]
                .active
                .as_ref()
                .unwrap();
            (
                active.child_slots()[0].slot(),
                active.frames()[0].shell(),
                active.frames()[1].shell(),
            )
        };
        let request = prepare_nested_branch_for_commit(
            &mut app,
            router,
            RouteLocation::new("/items/2").with_fragment(fragment),
        );
        let host_before = nested_protected_fingerprint(app.world(), host);
        let sibling_before = nested_protected_fingerprint(app.world(), sibling);
        let foreign = apply_child_slot_boundary_drift(&mut app, preserved_slot, drift);

        commit_presentation_system(app.world_mut());
        for _ in 0..4 {
            settle_cleanup_system(app.world_mut());
            if app.world().resource::<RouterNavigation>().entries[0]
                .cleanup_ledger
                .is_none()
            {
                break;
            }
        }

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert!(entry.pending_commit.is_none());
        assert_eq!(
            entry.active.as_ref().unwrap().frames()[0].shell(),
            preserved_parent_shell
        );
        assert_ne!(
            entry.active.as_ref().unwrap().frames()[1].shell(),
            leaving_child_shell,
            "preserved-slot splice did not replace child: status={:?}, failed_branch={}, ledger={}",
            entry.engine.request(request).unwrap().status(),
            entry.pending_failed_branch.is_some(),
            entry.cleanup_ledger.is_some(),
        );
        let request = entry.engine.request(request).unwrap();
        assert!(matches!(request.status(), RequestStatus::Succeeded));
        let expected = match drift {
            ChildSlotBoundaryDrift::ForeignComponent => {
                bevy_ui_router_core::prelude::CleanupDisposition::QuarantinedForeignComponent
            }
            ChildSlotBoundaryDrift::ForeignDescendant => {
                bevy_ui_router_core::prelude::CleanupDisposition::QuarantinedForeignDescendant
            }
        };
        assert!(
            request
                .trace()
                .iter()
                .all(|event| event.cleanup_disposition() != Some(expected))
        );
        assert!(
            app.world()
                .resource::<QuarantineLeaseTable>()
                .record(preserved_slot)
                .is_none(),
            "a preserved child-slot must not transfer to quarantine"
        );
        assert!(
            app.world()
                .resource::<RouteEntityRegistry>()
                .record(preserved_slot)
                .is_some_and(|record| record.lifecycle() == EntityLifecycle::Active)
        );
        assert_child_slot_boundary_drift_remains_foreign(
            app.world(),
            preserved_slot,
            foreign,
            drift,
        );
        assert_eq!(app.world().resource::<ForeignComponentHooks>().removed, 0);
        assert_eq!(app.world().resource::<ForeignComponentHooks>().despawned, 0);
        assert_eq!(nested_protected_fingerprint(app.world(), host), host_before);
        assert_eq!(
            nested_protected_fingerprint(app.world(), sibling),
            sibling_before
        );
    }

    #[test]
    fn nested_preserved_child_slot_foreign_component_commits_without_mutating_slot() {
        nested_preserved_child_slot_boundary_drift_commits_without_mutation(
            ChildSlotBoundaryDrift::ForeignComponent,
            "slot-component",
        );
    }

    #[test]
    fn nested_preserved_child_slot_foreign_descendant_commits_without_mutating_slot() {
        nested_preserved_child_slot_boundary_drift_commits_without_mutation(
            ChildSlotBoundaryDrift::ForeignDescendant,
            "slot-descendant",
        );
    }

    fn nested_deep_suffix_leaving_child_slot_boundary_drift_quarantines_slot(
        drift: ChildSlotBoundaryDrift,
        fragment: &'static str,
    ) {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        install_foreign_component_hooks(&mut app);
        let router = install_deep_nested_router(&mut app);
        let host = app
            .world()
            .iter_entities()
            .find(|entity| entity.contains::<RouterHostRegistration>())
            .unwrap()
            .id();
        app.world_mut()
            .entity_mut(host)
            .insert(NestedAppHostSentinel(0xB0));
        let sibling = app.world_mut().spawn(NestedAppHostSentinel(0xB1)).id();
        app.world_mut().entity_mut(host).add_child(sibling);
        app.update();
        commit_initial_deep_nested_branch(&mut app, router);

        let (preserved_root, leaving_slot, leaving_shells) = {
            let active = app.world().resource::<RouterNavigation>().entries[0]
                .active
                .as_ref()
                .unwrap();
            (
                active.frames()[0].shell(),
                active.child_slots()[1].slot(),
                active
                    .frames()
                    .iter()
                    .skip(1)
                    .map(RealizedFrame::shell)
                    .collect::<Vec<_>>(),
            )
        };
        let request = prepare_deep_nested_branch_for_commit(
            &mut app,
            router,
            RouteLocation::new("/sections/b/items/2").with_fragment(fragment),
        );
        let host_before = nested_protected_fingerprint(app.world(), host);
        let sibling_before = nested_protected_fingerprint(app.world(), sibling);
        let foreign = apply_child_slot_boundary_drift(&mut app, leaving_slot, drift);

        commit_presentation_system(app.world_mut());
        for _ in 0..4 {
            settle_cleanup_system(app.world_mut());
            if app.world().resource::<RouterNavigation>().entries[0]
                .cleanup_ledger
                .is_none()
            {
                break;
            }
        }

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert!(entry.pending_commit.is_none());
        assert!(entry.pending_failed_branch.is_none());
        assert_eq!(entry.active.as_ref().unwrap().frames().len(), 3);
        assert_eq!(
            entry.active.as_ref().unwrap().frames()[0].shell(),
            preserved_root
        );
        assert!(leaving_shells.iter().all(|leaving| {
            entry
                .active
                .as_ref()
                .unwrap()
                .frames()
                .iter()
                .all(|active| active.shell() != *leaving)
        }));
        let request = entry.engine.request(request).unwrap();
        assert!(matches!(request.status(), RequestStatus::Succeeded));
        let expected = match drift {
            ChildSlotBoundaryDrift::ForeignComponent => {
                bevy_ui_router_core::prelude::CleanupDisposition::QuarantinedForeignComponent
            }
            ChildSlotBoundaryDrift::ForeignDescendant => {
                bevy_ui_router_core::prelude::CleanupDisposition::QuarantinedForeignDescendant
            }
        };
        assert!(
            request
                .trace()
                .iter()
                .any(|event| event.cleanup_disposition() == Some(expected))
        );
        assert!(
            app.world()
                .resource::<QuarantineLeaseTable>()
                .record(leaving_slot)
                .is_some(),
            "the deep leaving child-slot must transfer to quarantine"
        );
        assert_child_slot_boundary_drift_remains_foreign(app.world(), leaving_slot, foreign, drift);
        assert_eq!(app.world().resource::<ForeignComponentHooks>().removed, 0);
        assert_eq!(app.world().resource::<ForeignComponentHooks>().despawned, 0);
        assert_eq!(nested_protected_fingerprint(app.world(), host), host_before);
        assert_eq!(
            nested_protected_fingerprint(app.world(), sibling),
            sibling_before
        );
    }

    #[test]
    fn nested_deep_suffix_leaving_child_slot_foreign_component_is_quarantined() {
        nested_deep_suffix_leaving_child_slot_boundary_drift_quarantines_slot(
            ChildSlotBoundaryDrift::ForeignComponent,
            "leaving-slot-component",
        );
    }

    #[test]
    fn nested_deep_suffix_leaving_child_slot_foreign_descendant_is_quarantined() {
        nested_deep_suffix_leaving_child_slot_boundary_drift_quarantines_slot(
            ChildSlotBoundaryDrift::ForeignDescendant,
            "leaving-slot-descendant",
        );
    }

    fn nested_entering_child_slot_boundary_drift_is_rejected(
        drift: ChildSlotBoundaryDrift,
        fragment: &'static str,
    ) {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let router = install_deep_nested_router(&mut app);
        app.update();
        commit_initial_deep_nested_branch(&mut app, router);
        let (active_shells, history_before) = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            (
                entry
                    .active
                    .as_ref()
                    .unwrap()
                    .frames()
                    .iter()
                    .map(RealizedFrame::shell)
                    .collect::<Vec<_>>(),
                entry.engine.history().clone(),
            )
        };
        let request = prepare_deep_nested_branch_for_commit(
            &mut app,
            router,
            RouteLocation::new("/sections/b/items/2").with_fragment(fragment),
        );
        let entering_slot = {
            let pending = app.world().resource::<RouterNavigation>().entries[0]
                .pending_commit
                .as_ref()
                .unwrap();
            assert_eq!(pending.branch.frames().len(), 2);
            assert_eq!(pending.branch.child_slots().len(), 1);
            pending.branch.child_slots()[0].slot()
        };
        let foreign = apply_child_slot_boundary_drift(&mut app, entering_slot, drift);

        commit_presentation_system(app.world_mut());

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert!(entry.pending_commit.is_none());
        assert!(entry.pending_failed_branch.is_some());
        assert_eq!(
            entry
                .active
                .as_ref()
                .unwrap()
                .frames()
                .iter()
                .map(RealizedFrame::shell)
                .collect::<Vec<_>>(),
            active_shells
        );
        assert_eq!(entry.engine.history(), &history_before);
        assert!(matches!(
            entry.engine.request(request).unwrap().status(),
            RequestStatus::Running(_)
        ));
        assert_child_slot_boundary_drift_remains_foreign(
            app.world(),
            entering_slot,
            foreign,
            drift,
        );
    }

    #[test]
    fn nested_entering_child_slot_foreign_component_is_pending_rejected() {
        nested_entering_child_slot_boundary_drift_is_rejected(
            ChildSlotBoundaryDrift::ForeignComponent,
            "entering-slot-component",
        );
    }

    #[test]
    fn nested_entering_child_slot_foreign_descendant_is_pending_rejected() {
        nested_entering_child_slot_boundary_drift_is_rejected(
            ChildSlotBoundaryDrift::ForeignDescendant,
            "entering-slot-descendant",
        );
    }

    #[test]
    fn nested_failed_transfer_keeps_the_same_ledger_for_a_higher_revision() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let router = install_deep_nested_router(&mut app);
        app.update();
        commit_initial_deep_nested_branch(&mut app, router);
        let leaving_slot = app.world().resource::<RouterNavigation>().entries[0]
            .active
            .as_ref()
            .unwrap()
            .child_slots()[1]
            .slot();
        let request = prepare_deep_nested_branch_for_commit(
            &mut app,
            router,
            RouteLocation::new("/sections/b/items/2").with_fragment("retry-transfer"),
        );
        let foreign = apply_child_slot_boundary_drift(
            &mut app,
            leaving_slot,
            ChildSlotBoundaryDrift::ForeignDescendant,
        )
        .unwrap();
        fail_next_verified_preparation_for_test();

        commit_presentation_system(app.world_mut());
        settle_cleanup_system(app.world_mut());

        let ledger = app.world().resource::<RouterNavigation>().entries[0]
            .cleanup_ledger
            .as_ref()
            .unwrap();
        assert_eq!(ledger.receipt_count(), 0);
        assert_eq!(ledger.leaving_frames().len(), 2);
        assert!(
            app.world()
                .resource::<QuarantineLeaseTable>()
                .record(leaving_slot)
                .is_none()
        );
        assert_eq!(
            app.world().get::<ChildOf>(foreign).map(ChildOf::parent),
            Some(leaving_slot)
        );

        settle_cleanup_system(app.world_mut());

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert!(entry.cleanup_ledger.is_none());
        assert!(matches!(
            entry.engine.request(request).unwrap().status(),
            RequestStatus::Succeeded
        ));
        assert!(
            app.world()
                .resource::<QuarantineLeaseTable>()
                .record(leaving_slot)
                .is_some()
        );
        assert_eq!(
            app.world().get::<ChildOf>(foreign).map(ChildOf::parent),
            Some(leaving_slot)
        );
    }

    #[test]
    fn refresh_does_not_cross_a_stale_terminal_root_after_quarantine_retry() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let router = install_atomic_router(&mut app, "quarantine-parent-postcheck");
        app.update();
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        app.update();

        let (leaving_output, obligation_count, top_slot, top_slot_seal) = {
            let navigation = app.world().resource::<RouterNavigation>();
            let frame = navigation.entries[0]
                .active
                .as_ref()
                .unwrap()
                .first_frame()
                .unwrap();
            (
                frame.interactive_entities()[0],
                frame.obligation_count(),
                frame.top_slot(),
                frame.mount_binding().slot_claim().seal(),
            )
        };
        app.world_mut()
            .entity_mut(leaving_output)
            .insert(FinalPreflightForeignComponent);
        let foreign = app.world_mut().spawn(ChildOf(leaving_output)).id();
        app.world_mut()
            .insert_resource(CleanupPostcheckDrift::external_parent(
                CleanupPostcheckPhase::VerifiedTransfer,
                top_slot,
            ));
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/").with_fragment("replacement"))
            .unwrap();

        app.update();

        let lease_count = app
            .world()
            .resource::<QuarantineLeaseTable>()
            .leases()
            .len();
        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        let ledger = entry.cleanup_ledger.as_ref().unwrap();
        assert_eq!(ledger.receipt_count(), obligation_count);
        assert!(!ledger.is_settled());
        assert!(!ledger.leaving_frames().is_empty());
        assert_eq!(lease_count, 1);
        assert!(matches!(
            entry.engine.request(request).unwrap().status(),
            RequestStatus::Running(NavigationPhase::PostCommitCleanup)
        ));

        app.update();
        let ledger = app.world().resource::<RouterNavigation>().entries[0]
            .cleanup_ledger
            .as_ref()
            .unwrap();
        assert_eq!(ledger.receipt_count(), obligation_count);
        assert_eq!(
            app.world()
                .resource::<QuarantineLeaseTable>()
                .leases()
                .len(),
            lease_count
        );

        let stale_root_seal = app
            .world_mut()
            .resource_mut::<RouteEntityRegistry>()
            .issue_seal();
        app.world_mut()
            .entity_mut(leaving_output)
            .insert(RouteOwnershipSeal::new(stale_root_seal));
        *app.world_mut().resource_mut::<InputFocus>() = InputFocus::from_entity(foreign);
        app.world_mut()
            .resource_mut::<RouteFocusState>()
            .set_focused(Some(foreign));

        app.world_mut()
            .entity_mut(top_slot)
            .insert(RouteOwnershipSeal::new(top_slot_seal));
        app.update();
        app.update();

        assert!(matches!(
            app.world().resource::<RouterNavigation>().entries[0]
                .engine
                .request(request)
                .unwrap()
                .status(),
            RequestStatus::Succeeded
        ));
        assert_eq!(
            app.world()
                .resource::<QuarantineLeaseTable>()
                .leases()
                .len(),
            lease_count
        );
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(foreign));
        assert_eq!(
            app.world().resource::<RouteFocusState>().focused(),
            Some(foreign)
        );
        let completion_count = app.world().resource::<RouterNavigation>().entries[0]
            .engine
            .request(request)
            .unwrap()
            .trace()
            .iter()
            .filter(|event| {
                event.kind() == NavigationTraceEventKind::CleanupSettled
                    && event.cleanup_disposition().is_none()
            })
            .count();
        assert_eq!(completion_count, 1);
        app.update();
        assert_eq!(
            app.world().resource::<RouterNavigation>().entries[0]
                .engine
                .request(request)
                .unwrap()
                .trace()
                .iter()
                .filter(|event| {
                    event.kind() == NavigationTraceEventKind::CleanupSettled
                        && event.cleanup_disposition().is_none()
                })
                .count(),
            completion_count
        );
    }

    #[test]
    fn failed_single_frame_transfer_keeps_ledger_for_a_higher_revision() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let router = install_atomic_router(&mut app, "fallback-focus-postcheck");
        app.update();
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        app.update();

        let leaving_output = {
            let navigation = app.world().resource::<RouterNavigation>();
            let frame = navigation.entries[0]
                .active
                .as_ref()
                .unwrap()
                .first_frame()
                .unwrap();
            frame.interactive_entities()[0]
        };
        let foreign = app.world_mut().spawn(CleanupDriftMarker).id();
        app.world_mut()
            .entity_mut(leaving_output)
            .add_child(foreign);
        fail_next_verified_preparation_for_test();
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/").with_fragment("replacement"))
            .unwrap();

        app.update();

        let lease_count = app
            .world()
            .resource::<QuarantineLeaseTable>()
            .leases()
            .len();
        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        let ledger = entry.cleanup_ledger.as_ref().unwrap();
        assert_eq!(ledger.receipt_count(), 0);
        assert!(!ledger.is_settled());
        assert_eq!(ledger.leaving_frames().len(), 1);
        assert_eq!(lease_count, 0);
        assert!(matches!(
            entry.engine.request(request).unwrap().status(),
            RequestStatus::Running(NavigationPhase::PostCommitCleanup)
        ));

        app.update();

        assert_eq!(app.world().resource::<InputFocus>().get(), None);
        assert_eq!(app.world().resource::<RouteFocusState>().focused(), None);
        assert!(matches!(
            app.world().resource::<RouterNavigation>().entries[0]
                .engine
                .request(request)
                .unwrap()
                .status(),
            RequestStatus::Succeeded
        ));
        assert_eq!(
            app.world()
                .resource::<QuarantineLeaseTable>()
                .leases()
                .len(),
            1
        );
        let completion_count = app.world().resource::<RouterNavigation>().entries[0]
            .engine
            .request(request)
            .unwrap()
            .trace()
            .iter()
            .filter(|event| {
                event.kind() == NavigationTraceEventKind::CleanupSettled
                    && event.cleanup_disposition().is_none()
            })
            .count();
        assert_eq!(completion_count, 1);
        app.update();
        assert_eq!(
            app.world().resource::<RouterNavigation>().entries[0]
                .engine
                .request(request)
                .unwrap()
                .trace()
                .iter()
                .filter(|event| {
                    event.kind() == NavigationTraceEventKind::CleanupSettled
                        && event.cleanup_disposition().is_none()
                })
                .count(),
            completion_count
        );
    }

    #[test]
    fn pickable_repair_parent_drift_is_zero_transfer_and_retryable() {
        let mut app = App::new();
        app.world_mut()
            .insert_resource(PickableRepairParentDrift::default());
        app.world_mut()
            .register_component_hooks::<Pickable>()
            .on_insert(|mut world, HookContext { entity, .. }| {
                let parent = {
                    let Some(mut drift) = world.get_resource_mut::<PickableRepairParentDrift>()
                    else {
                        return;
                    };
                    if drift.fired || drift.target != Some(entity) {
                        return;
                    }
                    drift.fired = true;
                    drift.parent
                };
                let Some(parent) = parent else { return };
                let stale_seal = world.resource_mut::<RouteEntityRegistry>().issue_seal();
                world
                    .commands()
                    .entity(parent)
                    .insert(RouteOwnershipSeal::new(stale_seal));
            });
        app.add_plugins(BevyUiRouterPlugin::default());
        let router = install_atomic_router(&mut app, "repair-parent-drift");
        app.update();
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        app.update();

        let (target, leaving_entities, top_slot, top_slot_seal) = {
            let navigation = app.world().resource::<RouterNavigation>();
            let frame = navigation.entries[0]
                .active
                .as_ref()
                .unwrap()
                .first_frame()
                .unwrap();
            let target = frame
                .claims()
                .map(|claim| claim.entity())
                .find(|entity| *entity != frame.shell())
                .unwrap();
            (
                target,
                frame
                    .claims()
                    .map(|claim| claim.entity())
                    .collect::<Vec<_>>(),
                frame.top_slot(),
                frame.mount_binding().slot_claim().seal(),
            )
        };
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/").with_fragment("replacement"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        commit_presentation_system(app.world_mut());
        let leaving_claims = leaving_entities
            .iter()
            .map(|entity| {
                app.world()
                    .resource::<RouteEntityRegistry>()
                    .record(*entity)
                    .unwrap()
                    .claim()
            })
            .collect::<Vec<_>>();
        {
            let mut drift = app.world_mut().resource_mut::<PickableRepairParentDrift>();
            drift.target = Some(target);
            drift.parent = Some(top_slot);
        }
        app.world_mut().entity_mut(target).remove::<Pickable>();

        settle_cleanup_system(app.world_mut());

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        let ledger = entry.cleanup_ledger.as_ref().unwrap();
        assert_eq!(ledger.receipt_count(), 0);
        assert!(!ledger.leaving_frames().is_empty());
        assert_eq!(
            app.world()
                .resource::<QuarantineLeaseTable>()
                .leases()
                .len(),
            0
        );
        assert_eq!(
            app.world()
                .resource::<QuarantineLeaseTable>()
                .unresolved_residuals()
                .len(),
            0
        );
        assert!(leaving_claims.iter().all(|claim| {
            app.world()
                .resource::<RouteEntityRegistry>()
                .record(claim.entity())
                .is_some_and(|record| record.claim() == *claim)
        }));
        assert!(matches!(
            entry.engine.request(request).unwrap().status(),
            RequestStatus::Running(NavigationPhase::PostCommitCleanup)
        ));

        app.world_mut()
            .entity_mut(top_slot)
            .insert(RouteOwnershipSeal::new(top_slot_seal));
        settle_cleanup_system(app.world_mut());

        assert!(matches!(
            app.world().resource::<RouterNavigation>().entries[0]
                .engine
                .request(request)
                .unwrap()
                .status(),
            RequestStatus::Succeeded
        ));
        assert_eq!(
            app.world()
                .resource::<QuarantineLeaseTable>()
                .leases()
                .len(),
            0
        );
    }

    #[test]
    fn atomic_switch_clears_foreign_descendant_focus_before_cleanup() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        app.init_resource::<CommitWindowObservation>().add_systems(
            Update,
            observe_commit_before_cleanup
                .after(RouterScheduleSet::CommitPresentation)
                .before(RouterScheduleSet::SettleCleanup),
        );
        let router = install_atomic_router(&mut app, "atomic-foreign-focus");
        app.update();
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        app.update();

        let leaving_output = app.world().resource::<RouterNavigation>().entries[0]
            .active
            .as_ref()
            .unwrap()
            .interactive_entities()[0];
        let foreign = app.world_mut().spawn(CleanupDriftMarker).id();
        app.world_mut()
            .entity_mut(leaving_output)
            .add_child(foreign);
        let foreign_components_before = app
            .world()
            .entity(foreign)
            .archetype()
            .components()
            .to_vec();
        *app.world_mut().resource_mut::<InputFocus>() = InputFocus::from_entity(foreign);
        app.world_mut()
            .resource_mut::<RouteFocusState>()
            .set_focused(Some(foreign));
        {
            let mut observation = app.world_mut().resource_mut::<CommitWindowObservation>();
            observation.armed = true;
            observation.leaving_output = Some(leaving_output);
        }
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(
                router,
                RouteLocation::new("/").with_fragment("foreign-focus-replacement"),
            )
            .unwrap();

        app.update();

        let samples = &app.world().resource::<CommitWindowObservation>().samples;
        assert_eq!(samples.len(), 1);
        let sample = samples[0];
        assert!(sample.old_gate_denied, "old gate must be closed at commit");
        assert!(sample.new_gate_allowed, "new gate must be open at commit");
        assert_eq!(sample.input_focus, None);
        assert_eq!(sample.route_focus, None);
        assert!(matches!(
            app.world().resource::<RouterNavigation>().entries[0]
                .engine
                .request(request)
                .unwrap()
                .status(),
            RequestStatus::Succeeded
        ));
        assert!(app.world().get_entity(foreign).is_ok());
        assert_eq!(
            app.world().get::<ChildOf>(foreign).map(ChildOf::parent),
            Some(leaving_output)
        );
        assert_eq!(
            app.world()
                .entity(foreign)
                .archetype()
                .components()
                .to_vec(),
            foreign_components_before,
            "commit and cleanup must not write foreign components"
        );
    }

    #[test]
    fn production_admission_retains_and_retries_detach_blocked_provisional_settlement() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        install_host_grant_observer_drift(&mut app);
        install_provisional_detach_reentry(&mut app);
        let router = install_atomic_router(&mut app, "detach-blocked-admission");
        let host = registered_host(&mut app, router);
        let host_node_before = app.world().get::<Node>(host).unwrap().clone();
        let foreign_host_child = app.world_mut().spawn(CleanupDriftMarker).id();
        app.world_mut()
            .entity_mut(host)
            .add_child(foreign_host_child);
        {
            let mut reentry = app.world_mut().resource_mut::<ProvisionalDetachReentry>();
            reentry.enabled = true;
            reentry.host = Some(host);
        }

        app.update();

        let revision = current_candidate_revision(&mut app, router);
        let residual = app
            .world()
            .resource::<HostGrantObserverDrift>()
            .observed_slots[0];
        let trace_before = app.world().resource::<RouterNavigation>().entries[0]
            .engine
            .snapshot()
            .host()
            .trace()
            .to_vec();
        let rejected = trace_before
            .iter()
            .find(|event| {
                event.kind()
                    == bevy_ui_router_core::HostTraceEventKind::AdmissionRejectedAfterMutation
            })
            .unwrap()
            .clone();
        assert_eq!(
            app.world().get::<ChildOf>(residual).map(ChildOf::parent),
            Some(host)
        );
        assert!(
            app.world()
                .resource::<RouteEntityRegistry>()
                .record(residual)
                .is_some()
        );
        assert_eq!(
            app.world().get::<Visibility>(residual),
            Some(&Visibility::Hidden)
        );
        assert_eq!(
            app.world().get::<Pickable>(residual),
            Some(&Pickable::IGNORE)
        );
        assert!(
            app.world()
                .get::<RouterInputBarrier>(residual)
                .is_some_and(RouterInputBarrier::denies_all)
        );
        assert_eq!(
            app.world().get::<ProvisionalSlotLifecycle>(residual),
            Some(&ProvisionalSlotLifecycle::PreparedInert)
        );
        assert_eq!(
            app.world()
                .resource::<RouteEntityRegistry>()
                .record(residual)
                .unwrap()
                .lifecycle(),
            EntityLifecycle::PreparedInert
        );
        assert!(
            app.world().resource::<RouterNavigation>().entries[0]
                .host_settlement
                .is_some(),
            "the exact residual must retain an explicit settlement owner"
        );
        assert!(
            app.world().resource::<RouterNavigation>().entries[0]
                .cleanup_summary()
                .foreground_pending()
        );
        assert!(
            !app.world()
                .resource::<HostCandidateRevisionLatch>()
                .allows(revision)
        );
        app.world_mut()
            .resource_mut::<ProvisionalDetachReentry>()
            .enabled = false;
        app.world_mut()
            .resource_mut::<HostGrantObserverDrift>()
            .enabled = false;
        app.update();

        assert!(
            app.world().get_entity(residual).is_err()
                || app
                    .world()
                    .resource::<QuarantineLeaseTable>()
                    .record(residual)
                    .is_some(),
            "the retained exact settlement must finish after detach reentry stops"
        );
        assert_eq!(
            app.world()
                .resource::<HostGrantObserverDrift>()
                .observed_slots
                .len(),
            1,
            "the latched candidate must not be admitted again"
        );
        assert_eq!(
            app.world().resource::<RouterNavigation>().entries[0]
                .engine
                .snapshot()
                .host()
                .trace(),
            trace_before
        );
        assert!(
            !app.world()
                .resource::<HostCandidateRevisionLatch>()
                .allows(revision)
        );
        assert!(
            app.world().resource::<RouterNavigation>().entries[0]
                .host_settlement
                .is_none()
        );
        assert!(
            !app.world().resource::<RouterNavigation>().entries[0]
                .cleanup_summary()
                .foreground_pending()
        );
        assert!(app.world().get::<Node>(host).is_some());
        assert_eq!(app.world().get::<Node>(host), Some(&host_node_before));
        assert!(app.world().get::<RouterHostRegistration>(host).is_some());
        assert!(app.world().get_entity(foreign_host_child).is_ok());
        assert!(
            app.world()
                .get::<CleanupDriftMarker>(foreign_host_child)
                .is_some()
        );
        assert_eq!(
            app.world()
                .get::<ChildOf>(foreign_host_child)
                .map(ChildOf::parent),
            Some(host)
        );

        let registration = app
            .world_mut()
            .entity_mut(host)
            .take::<RouterHostRegistration>()
            .unwrap();
        app.world_mut().entity_mut(host).insert(registration);
        app.update();
        let snapshot = app.world().resource::<RouterNavigation>().entries[0]
            .engine
            .snapshot()
            .host()
            .clone();
        assert_ne!(snapshot.version(), rejected.version());
        assert_ne!(snapshot.top_slot(), rejected.slot());
    }

    #[test]
    fn refresh_clears_focus_from_foreign_descendant_of_invalid_active_frame() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let router = install_atomic_router(&mut app, "invalid-active-foreign-focus");
        app.update();
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        app.update();
        let output = app.world().resource::<RouterNavigation>().entries[0]
            .active
            .as_ref()
            .unwrap()
            .interactive_entities()[0];
        let foreign = app.world_mut().spawn_empty().id();
        app.world_mut().entity_mut(output).add_child(foreign);
        *app.world_mut().get_mut::<Visibility>(output).unwrap() = Visibility::Hidden;
        *app.world_mut().resource_mut::<InputFocus>() = InputFocus::from_entity(foreign);
        app.world_mut()
            .resource_mut::<RouteFocusState>()
            .set_focused(Some(foreign));
        let foreign_components = app
            .world()
            .entity(foreign)
            .archetype()
            .components()
            .to_vec();

        app.update();

        assert_eq!(app.world().resource::<InputFocus>().get(), None);
        assert_eq!(app.world().resource::<RouteFocusState>().focused(), None);
        assert!(app.world().get_entity(foreign).is_ok());
        assert_eq!(
            app.world().get::<ChildOf>(foreign).map(ChildOf::parent),
            Some(output)
        );
        assert_eq!(
            app.world()
                .entity(foreign)
                .archetype()
                .components()
                .to_vec(),
            foreign_components
        );
    }

    #[test]
    fn refresh_clears_focus_from_foreign_descendant_of_quarantined_frame() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let router = install_atomic_router(&mut app, "quarantine-foreign-focus");
        app.update();
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        app.update();
        let quarantined_output = app.world().resource::<RouterNavigation>().entries[0]
            .active
            .as_ref()
            .unwrap()
            .interactive_entities()[0];
        let foreign = app.world_mut().spawn_empty().id();
        app.world_mut()
            .entity_mut(quarantined_output)
            .add_child(foreign);
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/").with_fragment("replacement"))
            .unwrap();
        app.update();
        assert!(
            app.world()
                .resource::<QuarantineLeaseTable>()
                .record(quarantined_output)
                .is_some()
        );
        *app.world_mut().resource_mut::<InputFocus>() = InputFocus::from_entity(foreign);
        app.world_mut()
            .resource_mut::<RouteFocusState>()
            .set_focused(Some(foreign));
        let foreign_components = app
            .world()
            .entity(foreign)
            .archetype()
            .components()
            .to_vec();

        app.update();

        assert_eq!(app.world().resource::<InputFocus>().get(), None);
        assert_eq!(app.world().resource::<RouteFocusState>().focused(), None);
        assert!(app.world().get_entity(foreign).is_ok());
        assert_eq!(
            app.world().get::<ChildOf>(foreign).map(ChildOf::parent),
            Some(quarantined_output)
        );
        assert_eq!(
            app.world()
                .entity(foreign)
                .archetype()
                .components()
                .to_vec(),
            foreign_components
        );
    }

    fn registered_host(app: &mut App, router: RouterHandle) -> Entity {
        app.world_mut()
            .query::<(Entity, &RouterHostRegistration)>()
            .iter(app.world())
            .find_map(|(entity, registration)| (registration.router() == router).then_some(entity))
            .expect("router fixture must have one registered host")
    }

    fn current_candidate_revision(app: &mut App, router: RouterHandle) -> HostCandidateRevision {
        app.world_mut().resource_scope(
            |world, mut revisions: bevy_ecs::change_detection::Mut<HostCandidateRevisionState>| {
                let HostCandidateClassification::Unique(candidate) =
                    classify_host_candidate(world, router, &mut revisions)
                else {
                    panic!("router fixture must classify one valid host")
                };
                candidate.revision()
            },
        )
    }

    #[test]
    fn enqueue_error_has_the_frozen_kinds_and_error_traits() {
        fn assert_error_traits<
            T: std::fmt::Debug + std::fmt::Display + std::error::Error + Send + Sync + 'static,
        >() {
        }
        assert_error_traits::<NavigationEnqueueError>();
        for kind in [
            NavigationEnqueueErrorKind::UnknownRouter,
            NavigationEnqueueErrorKind::HostUnavailable,
            NavigationEnqueueErrorKind::AuthorityRecovering,
            NavigationEnqueueErrorKind::NoBackEntry,
        ] {
            assert_eq!(NavigationEnqueueError::new(kind).kind(), kind);
        }
    }

    #[test]
    fn physical_top_slot_registration_precedes_host_confirmation_trace() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        install_atomic_router(&mut app, "host-trace");

        observe_authority_system(app.world_mut());

        let host = app.world().resource::<RouterNavigation>().entries[0]
            .engine
            .snapshot()
            .host()
            .clone();
        let kinds = host
            .trace()
            .iter()
            .map(bevy_ui_router_core::HostTraceEvent::kind)
            .collect::<Vec<_>>();
        let registered = kinds
            .iter()
            .position(|kind| *kind == bevy_ui_router_core::HostTraceEventKind::TopSlotRegistered)
            .expect("physical top-slot registration must be traced");
        let ready = kinds
            .iter()
            .position(|kind| *kind == bevy_ui_router_core::HostTraceEventKind::HostReady)
            .expect("host confirmation must be traced");
        assert!(registered < ready);
    }

    #[test]
    fn cleanup_reservation_failure_precedes_core_seal_and_is_retryable_zero_write() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let router = install_atomic_router(&mut app, "ledger-reservation");
        observe_authority_system(app.world_mut());

        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        commit_presentation_system(app.world_mut());
        settle_cleanup_system(app.world_mut());

        let replacement = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/").with_fragment("replacement"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());

        let before_projection = app.world().resource::<RouterNavigation>().entries[0]
            .engine
            .snapshot();
        let before_records = app
            .world()
            .resource::<RouteEntityRegistry>()
            .records()
            .to_vec();
        let mut before_entities = app
            .world()
            .iter_entities()
            .map(|entity| entity.id().to_bits())
            .collect::<Vec<_>>();
        before_entities.sort_unstable();

        crate::cleanup::reset_ledger_domain_allocations_for_test();
        crate::cleanup::fail_next_reservation_for_test();
        commit_presentation_system(app.world_mut());

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert_eq!(entry.engine.snapshot(), before_projection);
        assert!(entry.pending_commit.is_some());
        assert!(entry.pending_commit.as_ref().unwrap().ticket.is_none());
        assert!(entry.pending_commit.as_ref().unwrap().final_task.is_some());
        assert!(entry.pending_commit.as_ref().unwrap().frame().is_some());
        assert_eq!(
            app.world().resource::<RouteEntityRegistry>().records(),
            before_records
        );
        let mut after_entities = app
            .world()
            .iter_entities()
            .map(|entity| entity.id().to_bits())
            .collect::<Vec<_>>();
        after_entities.sort_unstable();
        assert_eq!(after_entities, before_entities);
        assert_eq!(
            crate::cleanup::ledger_domain_allocations_for_test(),
            0,
            "retryable receipt capacity must not consume a ledger domain"
        );

        commit_presentation_system(app.world_mut());
        settle_cleanup_system(app.world_mut());
        assert_eq!(
            crate::cleanup::ledger_domain_allocations_for_test(),
            1,
            "the single successful retry publishes exactly one fresh ledger domain"
        );
        assert!(matches!(
            app.world().resource::<RouterNavigation>().entries[0]
                .engine
                .request(replacement)
                .unwrap()
                .status(),
            RequestStatus::Succeeded
        ));
    }

    #[test]
    fn root_fallback_existing_active_commits_projectionlessly_and_settles_old_branch() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let router = install_failing_root_replacement_router(&mut app, "root-fallback-replace");
        app.update();

        let initial = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/one"))
            .unwrap();
        run_updates_until_succeeded(&mut app, initial);

        let (old_claims, history_before) = {
            let navigation = app.world().resource::<RouterNavigation>();
            let entry = &navigation.entries[0];
            (
                entry
                    .active
                    .as_ref()
                    .unwrap()
                    .claims()
                    .cloned()
                    .collect::<Vec<_>>(),
                entry.engine.history().clone(),
            )
        };
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/two"))
            .unwrap();

        for _ in 0..16 {
            app.update();
            if matches!(
                app.world().resource::<RouterNavigation>().entries[0]
                    .engine
                    .request(request)
                    .unwrap()
                    .status(),
                RequestStatus::Failed(_)
            ) {
                break;
            }
        }

        let (trace_before_replay, active_before_replay) = {
            let navigation = app.world().resource::<RouterNavigation>();
            let entry = &navigation.entries[0];
            let request = entry.engine.request(request).unwrap();
            assert!(matches!(
                request.status(),
                RequestStatus::Failed(failure)
                    if failure.kind() == NavigationFailureKind::Build
            ));
            let active = entry.active.as_ref().unwrap();
            assert_eq!(active.frames().len(), 1);
            assert_eq!(
                active.frames()[0].frame().origin(),
                &FrameOrigin::RouterFallback
            );
            assert!(entry.pending_commit.is_none());
            assert!(entry.cleanup_ledger.is_none());
            assert_eq!(entry.engine.history(), &history_before);
            assert!(old_claims.iter().all(|claim| {
                app.world().get_entity(claim.entity()).is_err()
                    && app
                        .world()
                        .resource::<RouteEntityRegistry>()
                        .record(claim.entity())
                        .is_none()
            }));
            let trace = request.trace();
            let commit = trace
                .iter()
                .position(|event| event.kind() == NavigationTraceEventKind::CommitPublished)
                .unwrap();
            let first_receipt = trace
                .iter()
                .position(|event| event.cleanup_disposition().is_some())
                .unwrap();
            let settled = trace
                .iter()
                .position(|event| {
                    event.kind() == NavigationTraceEventKind::CleanupSettled
                        && event.cleanup_disposition().is_none()
                })
                .unwrap();
            let terminal = trace
                .iter()
                .position(|event| event.kind() == NavigationTraceEventKind::RequestTerminal)
                .unwrap();
            assert!(commit < first_receipt && first_receipt < settled && settled < terminal);
            assert_eq!(
                trace
                    .iter()
                    .filter(|event| event.cleanup_disposition().is_some())
                    .count(),
                old_claims.len()
            );
            assert_eq!(
                trace
                    .iter()
                    .filter(|event| event.kind() == NavigationTraceEventKind::RequestTerminal)
                    .count(),
                1
            );
            (trace.to_vec(), active.frames()[0].frame().clone())
        };

        app.update();

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert_eq!(
            entry.engine.request(request).unwrap().trace(),
            trace_before_replay
        );
        assert_eq!(
            entry.active.as_ref().unwrap().frames()[0].frame(),
            &active_before_replay
        );
        assert_eq!(entry.engine.history(), &history_before);
    }

    #[test]
    fn root_fallback_existing_active_foreign_component_commits_and_quarantines_cleanup() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        install_foreign_component_hooks(&mut app);
        let router =
            install_failing_root_replacement_router(&mut app, "root-fallback-foreign-component");
        app.update();

        let initial = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/one"))
            .unwrap();
        run_updates_until_succeeded(&mut app, initial);
        let old_output = app.world().resource::<RouterNavigation>().entries[0]
            .active
            .as_ref()
            .unwrap()
            .interactive_entities()[0];
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/two"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_until_projectionless_fallback_pending(&mut app);
        app.world_mut()
            .entity_mut(old_output)
            .insert(FinalPreflightForeignComponent);

        commit_presentation_system(app.world_mut());

        {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            assert!(entry.authority_protocol_residual.is_none());
            assert!(entry.pending_commit.is_none());
            assert!(entry.cleanup_ledger.is_some());
            assert_eq!(
                entry
                    .active
                    .as_ref()
                    .unwrap()
                    .first_frame()
                    .unwrap()
                    .frame()
                    .origin(),
                &FrameOrigin::RouterFallback
            );
        }
        for _ in 0..8 {
            settle_cleanup_system(app.world_mut());
        }

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert!(matches!(
            entry.engine.request(request).unwrap().status(),
            RequestStatus::Failed(failure)
                if failure.kind() == NavigationFailureKind::Build
        ));
        assert!(
            app.world()
                .get::<FinalPreflightForeignComponent>(old_output)
                .is_some()
        );
        assert_eq!(app.world().resource::<ForeignComponentHooks>().removed, 0);
        assert_eq!(app.world().resource::<ForeignComponentHooks>().despawned, 0);
        assert!(
            app.world()
                .resource::<QuarantineLeaseTable>()
                .record(old_output)
                .is_some()
        );
        assert!(
            app.world()
                .resource::<RouteEntityRegistry>()
                .record(old_output)
                .is_none()
        );
    }

    #[test]
    fn root_fallback_existing_active_rejects_non_fallback_projectionless_task() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let router = install_atomic_router(&mut app, "projectionless-normal-reject");
        observe_authority_system(app.world_mut());
        let initial = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        commit_presentation_system(app.world_mut());
        settle_cleanup_system(app.world_mut());
        assert!(matches!(
            app.world().resource::<RouterNavigation>().entries[0]
                .engine
                .request(initial)
                .unwrap()
                .status(),
            RequestStatus::Succeeded
        ));

        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(
                router,
                RouteLocation::new("/").with_fragment("normal-replacement"),
            )
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());

        let (
            active_before,
            active_claims,
            active_facts,
            host,
            host_before,
            top_slot,
            history_before,
            trace_before,
        ) = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            let pending = entry.pending_commit.as_ref().unwrap();
            assert!(matches!(
                pending.final_task.as_ref().unwrap().frame().origin(),
                FrameOrigin::MatchedRoute(_)
            ));
            let active = entry.active.as_ref().unwrap();
            let claims = active.claims().cloned().collect::<Vec<_>>();
            let host = entry.host.as_ref().unwrap();
            (
                active.frames()[0].frame().clone(),
                claims.clone(),
                claims
                    .iter()
                    .map(|claim| nested_protected_fingerprint(app.world(), claim.entity()))
                    .collect::<Vec<_>>(),
                host.host,
                nested_protected_fingerprint(app.world(), host.host),
                (
                    host.top_slot,
                    host.top_slot_claim.clone(),
                    nested_protected_fingerprint(app.world(), host.top_slot),
                ),
                entry.engine.history().clone(),
                entry.engine.request(request).unwrap().trace().to_vec(),
            )
        };
        let focus = app.world().resource::<RouterNavigation>().entries[0]
            .active
            .as_ref()
            .unwrap()
            .interactive_entities()[0];
        *app.world_mut().resource_mut::<InputFocus>() = InputFocus::from_entity(focus);
        app.world_mut()
            .resource_mut::<RouteFocusState>()
            .set_focused(Some(focus));
        assert!(
            app.world()
                .resource::<RouterInputGate>()
                .allows_pointer(focus)
        );
        let world_before = authority_world_fingerprint(app.world());
        let registry_before = app
            .world()
            .resource::<RouteEntityRegistry>()
            .records()
            .to_vec();

        let foreign_fallback = foreign_fallback_task("projectionless-foreign-fallback");
        app.world_mut().resource_scope(
            |world, mut navigation: bevy_ecs::change_detection::Mut<RouterNavigation>| {
                let entry = &mut navigation.entries[0];
                let pending = entry.pending_commit.take().unwrap();
                let mut pending = commit_projectionless_branch(world, entry, pending)
                    .expect_err("normal MatchedRoute cannot use projectionless replacement");
                let current_task = pending
                    .final_task
                    .replace(foreign_fallback)
                    .expect("normal rejection returns its exact current task");
                pending = commit_projectionless_branch(world, entry, pending)
                    .expect_err("foreign fallback-shaped task cannot isolate current authority");
                pending.final_task = Some(current_task);
                entry.pending_commit = Some(pending);
            },
        );

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert_eq!(
            entry.active.as_ref().unwrap().frames()[0].frame(),
            &active_before
        );
        assert_eq!(entry.engine.history(), &history_before);
        assert_eq!(entry.engine.request(request).unwrap().trace(), trace_before);
        assert_eq!(authority_world_fingerprint(app.world()), world_before);
        assert_eq!(
            entry
                .active
                .as_ref()
                .unwrap()
                .claims()
                .cloned()
                .collect::<Vec<_>>(),
            active_claims
        );
        assert_eq!(
            active_claims
                .iter()
                .map(|claim| nested_protected_fingerprint(app.world(), claim.entity()))
                .collect::<Vec<_>>(),
            active_facts
        );
        assert_eq!(entry.host.as_ref().unwrap().host, host);
        assert_eq!(nested_protected_fingerprint(app.world(), host), host_before);
        assert_eq!(entry.host.as_ref().unwrap().top_slot, top_slot.0);
        assert_eq!(entry.host.as_ref().unwrap().top_slot_claim, top_slot.1);
        assert_eq!(
            nested_protected_fingerprint(app.world(), top_slot.0),
            top_slot.2
        );
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(focus));
        assert_eq!(
            app.world().resource::<RouteFocusState>().focused(),
            Some(focus)
        );
        assert!(
            app.world()
                .resource::<RouterInputGate>()
                .allows_pointer(focus)
        );
        assert!(
            app.world()
                .resource::<RouterInputGate>()
                .allows_keyboard(focus)
        );
        assert!(
            app.world()
                .resource::<RouterInputGate>()
                .allows_gamepad(focus)
        );
        assert_eq!(
            app.world().resource::<RouteEntityRegistry>().records(),
            registry_before
        );
    }

    #[test]
    fn root_fallback_existing_active_permanent_drift_fail_closes_without_retry() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        app.init_resource::<FallbackReplacementDrift>().add_systems(
            Update,
            drift_fallback_active_before_commit
                .after(RouterScheduleSet::DriveNavigation)
                .before(RouterScheduleSet::CommitPresentation),
        );
        let router = install_failing_root_replacement_router(&mut app, "fallback-drift");
        let foreign = app.world_mut().spawn(NestedAppHostSentinel(0xFA11)).id();
        app.update();
        let initial = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/one"))
            .unwrap();
        run_updates_until_succeeded(&mut app, initial);
        let foreign_before = nested_protected_fingerprint(app.world(), foreign);
        let history_before = app.world().resource::<RouterNavigation>().entries[0]
            .engine
            .history()
            .clone();
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/two"))
            .unwrap();
        app.world_mut()
            .resource_mut::<FallbackReplacementDrift>()
            .armed = true;

        for _ in 0..4 {
            app.update();
            if app.world().resource::<RouterNavigation>().entries[0]
                .authority_protocol_residual
                .is_some()
            {
                break;
            }
        }

        let trace_before_replay = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            assert!(
                app.world()
                    .resource::<FallbackReplacementDrift>()
                    .drifted
                    .is_some()
            );
            assert!(entry.authority_protocol_residual.is_some());
            assert!(entry.pending_commit.is_none());
            assert!(entry.active.is_none());
            assert_eq!(entry.engine.history(), &history_before);
            let request_inspection = entry.engine.request(request).unwrap();
            let trace = request_inspection.trace();
            assert!(
                trace
                    .iter()
                    .all(|event| event.kind() != NavigationTraceEventKind::CommitPublished)
            );
            assert_eq!(
                nested_protected_fingerprint(app.world(), foreign),
                foreign_before
            );
            trace.to_vec()
        };

        for _ in 0..3 {
            app.update();
        }

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert!(entry.authority_protocol_residual.is_some());
        assert_eq!(
            entry.engine.request(request).unwrap().trace(),
            trace_before_replay
        );
        assert_eq!(entry.engine.history(), &history_before);
        assert_eq!(
            nested_protected_fingerprint(app.world(), foreign),
            foreign_before
        );
    }

    #[test]
    fn root_fallback_active_root_mount_drift_is_authority_loss() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let router = install_failing_root_replacement_router(&mut app, "fallback-root-mount-drift");
        let app_parent = app.world_mut().spawn(NestedAppHostSentinel(0xFA13)).id();
        app.update();

        let initial = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/one"))
            .unwrap();
        run_updates_until_succeeded(&mut app, initial);
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/two"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_until_projectionless_fallback_pending(&mut app);
        let history_before = app.world().resource::<RouterNavigation>().entries[0]
            .engine
            .history()
            .clone();
        let active_root = app.world().resource::<RouterNavigation>().entries[0]
            .active
            .as_ref()
            .unwrap()
            .first_frame()
            .unwrap()
            .shell();
        app.world_mut()
            .entity_mut(active_root)
            .insert(bevy_ecs::hierarchy::ChildOf(app_parent));

        commit_presentation_system(app.world_mut());

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert!(entry.authority_protocol_residual.is_some());
        assert!(entry.pending_failed_branch.is_none());
        assert!(entry.pending_commit.is_none());
        assert!(entry.active.is_none());
        assert_eq!(entry.engine.history(), &history_before);
        assert!(matches!(
            entry.engine.request(request).unwrap().status(),
            RequestStatus::Failed(failure)
                if failure.kind() == NavigationFailureKind::AuthorityLost
        ));
        assert_eq!(
            app.world().get::<NestedAppHostSentinel>(app_parent),
            Some(&NestedAppHostSentinel(0xFA13))
        );
        assert!(
            app.world()
                .get::<bevy_ecs::hierarchy::Children>(app_parent)
                .is_some_and(|children| children
                    .iter()
                    .filter(|child| **child == active_root)
                    .count()
                    == 1)
        );
    }

    #[test]
    fn root_fallback_mixed_entering_and_current_drift_prefers_authority_loss() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let router = install_failing_root_replacement_router(&mut app, "fallback-mixed-drift");
        app.update();

        let initial = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/one"))
            .unwrap();
        run_updates_until_succeeded(&mut app, initial);
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/two"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_until_projectionless_fallback_pending(&mut app);

        let (current, entering) = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            (
                entry.active.as_ref().unwrap().interactive_entities()[0],
                entry
                    .pending_commit
                    .as_ref()
                    .unwrap()
                    .branch
                    .as_realized()
                    .interactive_entities()[0],
            )
        };
        {
            let mut pickable = app.world_mut().get_mut::<Pickable>(current).unwrap();
            pickable.should_block_lower = !pickable.should_block_lower;
            pickable.is_hoverable = !pickable.is_hoverable;
        }
        app.world_mut().entity_mut(entering).remove::<Pickable>();

        commit_presentation_system(app.world_mut());

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert!(entry.pending_failed_branch.is_none());
        assert!(entry.authority_protocol_residual.is_some());
        assert!(entry.pending_commit.is_none());
        assert!(entry.active.is_none());
        assert!(matches!(
            entry.engine.request(request).unwrap().status(),
            RequestStatus::Failed(failure)
                if failure.kind() == NavigationFailureKind::AuthorityLost
        ));
    }

    #[test]
    fn root_fallback_entering_drift_does_not_hide_active_relationship_cycle() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let router = install_failing_root_replacement_router(
            &mut app,
            "fallback-active-cycle-and-entering-drift",
        );
        app.update();

        let initial = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/one"))
            .unwrap();
        run_updates_until_succeeded(&mut app, initial);
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/two"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_until_projectionless_fallback_pending(&mut app);
        let history_before = app.world().resource::<RouterNavigation>().entries[0]
            .engine
            .history()
            .clone();
        let (cycle_child, cycle_parent, entering) = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            let active = entry.active.as_ref().unwrap().first_frame().unwrap();
            let owned = active
                .claims()
                .map(|claim| claim.entity())
                .collect::<std::collections::HashSet<_>>();
            let (cycle_child, cycle_parent) = active
                .cleanup_members()
                .find_map(|(claim, parent)| {
                    owned.contains(&parent).then_some((claim.entity(), parent))
                })
                .unwrap();
            (
                cycle_child,
                cycle_parent,
                entry
                    .pending_commit
                    .as_ref()
                    .unwrap()
                    .branch
                    .as_realized()
                    .interactive_entities()[0],
            )
        };
        app.world_mut()
            .entity_mut(cycle_parent)
            .insert(ChildOf(cycle_child));
        app.world_mut().entity_mut(entering).remove::<Pickable>();
        app.world_mut().flush();

        commit_presentation_system(app.world_mut());

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert!(entry.pending_failed_branch.is_none());
        assert!(entry.authority_protocol_residual.is_some());
        assert!(entry.pending_commit.is_none());
        assert!(entry.active.is_none());
        assert_eq!(entry.engine.history(), &history_before);
        assert!(matches!(
            entry.engine.request(request).unwrap().status(),
            RequestStatus::Failed(failure)
                if failure.kind() == NavigationFailureKind::AuthorityLost
        ));
    }

    #[test]
    fn root_fallback_missing_resources_fail_closes_without_retry() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let router = install_failing_root_replacement_router(&mut app, "fallback-missing-resource");
        app.update();

        let initial = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/one"))
            .unwrap();
        run_updates_until_succeeded(&mut app, initial);
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/two"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_until_projectionless_fallback_pending(&mut app);

        app.world_mut().remove_resource::<RouteEntityRegistry>();
        app.world_mut().resource_scope(
            |world, mut navigation: bevy_ecs::change_detection::Mut<RouterNavigation>| {
                let entry = &mut navigation.entries[0];
                let pending = entry.pending_commit.take().unwrap();
                assert!(
                    commit_projectionless_branch(world, entry, pending).is_ok(),
                    "missing Resources are a permanent fail-close fact, not retry"
                );
            },
        );

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert!(entry.projectionless_fail_close.is_none());
        assert!(entry.active.is_none());
        assert!(entry.pending_commit.is_none());
        let residual = entry.authority_protocol_residual.as_ref().unwrap();
        assert!(matches!(
            residual.kind,
            AuthorityProtocolResidualKind::PreflightAuthorityLost(evidence)
                if evidence.is_missing_resource()
                    && evidence.source == LifecyclePreflightSource::RequiredResources(
                        RequiredResourceSet::ROUTE_ENTITY_REGISTRY,
                    )
        ));
        assert!(
            residual
                .pending
                .as_ref()
                .is_some_and(|pending| pending.ticket.is_none() && pending.final_task.is_none())
        );
        assert!(matches!(
            entry.engine.request(request).unwrap().status(),
            RequestStatus::Failed(failure)
                if failure.kind() == NavigationFailureKind::AuthorityLost
        ));
    }

    #[test]
    fn root_fallback_missing_input_gate_retains_exact_resource_evidence() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let router = install_failing_root_replacement_router(
            &mut app,
            "fallback-missing-input-gate-resource",
        );
        app.update();

        let initial = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/one"))
            .unwrap();
        run_updates_until_succeeded(&mut app, initial);
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/two"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_until_projectionless_fallback_pending(&mut app);

        app.world_mut().remove_resource::<RouterInputGate>();
        app.world_mut().resource_scope(
            |world, mut navigation: bevy_ecs::change_detection::Mut<RouterNavigation>| {
                let entry = &mut navigation.entries[0];
                let pending = entry.pending_commit.take().unwrap();
                assert!(
                    commit_projectionless_branch(world, entry, pending).is_ok(),
                    "missing input gate is a permanent fail-close fact, not retry"
                );
            },
        );

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert!(entry.projectionless_fail_close.is_none());
        assert!(entry.active.is_none());
        assert!(entry.pending_commit.is_none());
        let residual = entry.authority_protocol_residual.as_ref().unwrap();
        let AuthorityProtocolResidualKind::PreflightAuthorityLost(evidence) = residual.kind else {
            panic!("missing input gate must retain typed preflight evidence")
        };
        assert_eq!(
            evidence.expectation,
            LifecyclePreflightExpectation::ResourcePresent
        );
        assert_eq!(evidence.observed, LifecyclePreflightObserved::Missing);
        assert_eq!(
            evidence.source,
            LifecyclePreflightSource::RequiredResources(RequiredResourceSet::ROUTER_INPUT_GATE,)
        );
        assert!(
            residual
                .pending
                .as_ref()
                .is_some_and(|pending| pending.ticket.is_none() && pending.final_task.is_none())
        );
        assert!(matches!(
            entry.engine.request(request).unwrap().status(),
            RequestStatus::Failed(failure)
                if failure.kind() == NavigationFailureKind::AuthorityLost
        ));
    }

    #[test]
    fn root_fallback_existing_active_retained_empty_slot_commits_and_cleans() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let router =
            install_failing_retained_root_replacement_router(&mut app, "retained-fallback");
        app.update();

        let initial = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/one/items/1"))
            .unwrap();
        run_updates_until_succeeded(&mut app, initial);
        let truncate = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/one"))
            .unwrap();
        run_updates_until_succeeded(&mut app, truncate);

        let (retained_slot, old_claims, focus_target) = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            let active = entry.active.as_ref().unwrap();
            assert_eq!(active.frames().len(), 1);
            assert_eq!(active.child_slots().len(), 1);
            let slot = &active.child_slots()[0];
            assert_eq!(
                slot.slot_claim().lifecycle(),
                EntityLifecycle::PreparedInert
            );
            (
                slot.slot(),
                active.claims().cloned().collect::<Vec<_>>(),
                active.frames()[0].shell(),
            )
        };
        *app.world_mut().resource_mut::<InputFocus>() = InputFocus::from_entity(focus_target);
        app.world_mut()
            .resource_mut::<RouteFocusState>()
            .set_focused(Some(focus_target));

        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/two"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_until_projectionless_fallback_pending(&mut app);
        commit_presentation_system(app.world_mut());

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert!(entry.pending_commit.is_none());
        assert!(entry.authority_protocol_residual.is_none());
        assert!(entry.cleanup_ledger.is_some());
        assert_eq!(
            app.world()
                .resource::<RouteEntityRegistry>()
                .record(retained_slot)
                .map(|record| record.lifecycle()),
            Some(EntityLifecycle::Leaving)
        );
        assert_eq!(
            app.world().get::<ChildSlotLifecycle>(retained_slot),
            Some(&ChildSlotLifecycle::Leaving)
        );
        assert_eq!(app.world().resource::<InputFocus>().get(), None);
        assert_eq!(app.world().resource::<RouteFocusState>().focused(), None);

        for _ in 0..4 {
            settle_cleanup_system(app.world_mut());
        }
        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        let request = entry.engine.request(request).unwrap();
        assert!(matches!(
            request.status(),
            RequestStatus::Failed(failure) if failure.kind() == NavigationFailureKind::Build
        ));
        assert!(old_claims.iter().all(|claim| {
            app.world().get_entity(claim.entity()).is_err()
                && app
                    .world()
                    .resource::<RouteEntityRegistry>()
                    .record(claim.entity())
                    .is_none()
        }));
        assert_eq!(
            request
                .trace()
                .iter()
                .filter(|event| event.cleanup_disposition().is_some())
                .count(),
            old_claims.len()
        );
    }

    #[test]
    fn root_fallback_entering_drift_rejects_pending_and_preserves_active() {
        for wrong_top_slot in [true, false] {
            let mut app = App::new();
            app.add_plugins(BevyUiRouterPlugin::default());
            let router = install_failing_root_replacement_router(
                &mut app,
                if wrong_top_slot {
                    "fallback-wrong-top-slot"
                } else {
                    "fallback-entering-component-drift"
                },
            );
            let foreign_router = install_atomic_router(
                &mut app,
                if wrong_top_slot {
                    "fallback-foreign-top-slot"
                } else {
                    "fallback-foreign-unused"
                },
            );
            app.update();
            let initial = app
                .world_mut()
                .resource_mut::<RouterNavigation>()
                .push(router, RouteLocation::new("/one"))
                .unwrap();
            run_updates_until_succeeded(&mut app, initial);
            let foreign_initial = app
                .world_mut()
                .resource_mut::<RouterNavigation>()
                .push(foreign_router, RouteLocation::new("/"))
                .unwrap();
            run_updates_until_succeeded(&mut app, foreign_initial);
            let request = app
                .world_mut()
                .resource_mut::<RouterNavigation>()
                .push(router, RouteLocation::new("/two"))
                .unwrap();
            collect_navigation_system(app.world_mut());
            drive_until_projectionless_fallback_pending(&mut app);

            let (active_claims, active_facts, pending_claims, host_facts, top_slot, history_before) = {
                let entry = &app.world().resource::<RouterNavigation>().entries[0];
                let active = entry.active.as_ref().unwrap();
                let claims = active.claims().cloned().collect::<Vec<_>>();
                let host = entry.host.as_ref().unwrap();
                (
                    claims.clone(),
                    claims
                        .iter()
                        .map(|claim| nested_protected_fingerprint(app.world(), claim.entity()))
                        .collect::<Vec<_>>(),
                    entry
                        .pending_commit
                        .as_ref()
                        .unwrap()
                        .branch
                        .as_realized()
                        .claims()
                        .cloned()
                        .collect::<Vec<_>>(),
                    nested_protected_fingerprint(app.world(), host.host),
                    (
                        host.top_slot,
                        host.top_slot_claim.clone(),
                        nested_protected_fingerprint(app.world(), host.top_slot),
                    ),
                    entry.engine.history().clone(),
                )
            };
            let focus = app.world().resource::<RouterNavigation>().entries[0]
                .active
                .as_ref()
                .unwrap()
                .interactive_entities()[0];
            let trace_prefix_len = app.world().resource::<RouterNavigation>().entries[0]
                .engine
                .request(request)
                .unwrap()
                .trace()
                .len();
            *app.world_mut().resource_mut::<InputFocus>() = InputFocus::from_entity(focus);
            app.world_mut()
                .resource_mut::<RouteFocusState>()
                .set_focused(Some(focus));
            if wrong_top_slot {
                let (foreign_host, foreign_slot) = {
                    let navigation = app.world().resource::<RouterNavigation>();
                    let host = navigation
                        .entries
                        .iter()
                        .find(|entry| entry.handle == foreign_router)
                        .and_then(|entry| entry.host.as_ref())
                        .unwrap();
                    (host.version.clone(), host.slot_id)
                };
                let mut navigation = app.world_mut().resource_mut::<RouterNavigation>();
                assert!(
                    navigation
                        .entries
                        .iter_mut()
                        .find(|entry| entry.handle == router)
                        .and_then(|entry| entry.pending_commit.as_mut())
                        .and_then(|pending| pending.final_task.as_mut())
                        .unwrap()
                        .testing_replace_top_slot_mount_target(foreign_host, foreign_slot)
                );
            } else {
                let entering = pending_claims
                    .iter()
                    .find(|claim| app.world().get::<Pickable>(claim.entity()).is_some())
                    .unwrap()
                    .entity();
                app.world_mut().entity_mut(entering).remove::<Pickable>();
            }

            commit_presentation_system(app.world_mut());

            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            let authority_evidence =
                entry
                    .authority_protocol_residual
                    .as_ref()
                    .and_then(|residual| match &residual.kind {
                        AuthorityProtocolResidualKind::PreflightAuthorityLost(evidence) => {
                            Some(*evidence)
                        }
                        _ => None,
                    });
            assert!(
                entry.authority_protocol_residual.is_none(),
                "entering-only drift must not become authority loss: wrong_top_slot={wrong_top_slot}, evidence={authority_evidence:?}"
            );
            assert!(matches!(
                entry
                    .pending_failed_branch
                    .as_ref()
                    .map(|failed| &failed.terminal),
                Some(PendingFailedBranchTerminal::TaskWithPhysicalResidual { .. })
            ));
            assert_eq!(entry.engine.history(), &history_before);
            assert_eq!(
                entry
                    .active
                    .as_ref()
                    .unwrap()
                    .claims()
                    .cloned()
                    .collect::<Vec<_>>(),
                active_claims
            );
            assert_eq!(
                active_facts,
                active_claims
                    .iter()
                    .map(|claim| nested_protected_fingerprint(app.world(), claim.entity()))
                    .collect::<Vec<_>>()
            );
            assert_eq!(
                nested_protected_fingerprint(app.world(), entry.host.as_ref().unwrap().host),
                host_facts
            );
            assert_eq!(entry.host.as_ref().unwrap().top_slot, top_slot.0);
            assert_eq!(entry.host.as_ref().unwrap().top_slot_claim, top_slot.1);
            assert_eq!(
                nested_protected_fingerprint(app.world(), top_slot.0),
                top_slot.2
            );
            assert_eq!(app.world().resource::<InputFocus>().get(), Some(focus));
            assert_eq!(
                app.world().resource::<RouteFocusState>().focused(),
                Some(focus)
            );
            assert!(
                app.world()
                    .resource::<RouterInputGate>()
                    .allows_pointer(focus)
            );
            assert!(
                entry
                    .engine
                    .request(request)
                    .unwrap()
                    .trace()
                    .iter()
                    .all(|event| event.kind() != NavigationTraceEventKind::CommitPublished)
            );

            for _ in 0..8 {
                settle_cleanup_system(app.world_mut());
            }
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            assert!(matches!(
                entry.engine.request(request).unwrap().status(),
                RequestStatus::Failed(failure)
                    if failure.kind() == NavigationFailureKind::RecoveryFailed
            ));
            assert!(entry.active.is_some());
            assert_eq!(entry.engine.history(), &history_before);
            assert!(active_claims.iter().all(|claim| {
                app.world()
                    .resource::<RouteEntityRegistry>()
                    .record(claim.entity())
                    .is_some_and(|record| record.claim() == *claim)
            }));
            let request_inspection = entry.engine.request(request).unwrap();
            let request_trace = request_inspection.trace();
            let terminal_trace = &request_trace[trace_prefix_len..];
            assert_eq!(
                terminal_trace
                    .iter()
                    .filter(|event| event.cleanup_disposition().is_some())
                    .count(),
                pending_claims.len()
            );
            assert_eq!(
                terminal_trace
                    .iter()
                    .filter(|event| event.kind() == NavigationTraceEventKind::RequestTerminal)
                    .count(),
                1
            );
            for claim in &pending_claims {
                match app
                    .world()
                    .resource::<RouteEntityRegistry>()
                    .record(claim.entity())
                {
                    None => assert!(app.world().get_entity(claim.entity()).is_err()),
                    Some(record) => {
                        assert_eq!(record.lifecycle(), EntityLifecycle::Quarantined);
                        assert_eq!(record.claim().owner(), claim.owner());
                        assert_eq!(record.claim().seal(), claim.seal());
                        let facts = nested_protected_fingerprint(app.world(), claim.entity());
                        assert_eq!(facts.visibility, Some(Visibility::Hidden));
                        assert!(facts.pickable.is_none() || facts.pickable == Some((false, false)));
                        assert_eq!(facts.barrier, Some((false, true)));
                    }
                }
            }
            let trace_before_extra_update = request_trace.to_vec();
            {
                let mut navigation = app.world_mut().resource_mut::<RouterNavigation>();
                assert!(navigation.entries[0].engine.action().is_none());
            }
            app.update();
            assert_eq!(
                app.world().resource::<RouterNavigation>().entries[0]
                    .engine
                    .request(request)
                    .unwrap()
                    .trace(),
                trace_before_extra_update
            );
        }
    }

    #[test]
    fn root_fallback_post_report_contradiction_retries_fail_close_without_stuck_ticket() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        app.insert_resource(ProjectionlessPrepareCommitRejection { remaining: 1 });
        app.insert_resource(ProjectionlessFailClosePreparationShortage {
            remaining: 1,
            observed_preparations: 0,
        });
        let router = install_failing_root_replacement_router(&mut app, "fallback-post-report");
        let protected = app.world_mut().spawn(NestedAppHostSentinel(0xFA12)).id();
        app.update();
        let initial = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/one"))
            .unwrap();
        run_updates_until_succeeded(&mut app, initial);
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/two"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_until_projectionless_fallback_pending(&mut app);
        let (history_before, host, protected_before) = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            (
                entry.engine.history().clone(),
                entry.host.as_ref().unwrap().host,
                nested_protected_fingerprint(app.world(), protected),
            )
        };
        let host_before = nested_protected_fingerprint(app.world(), host);

        commit_presentation_system(app.world_mut());

        let trace_after_shortage = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            assert_eq!(
                app.world()
                    .resource::<ProjectionlessFailClosePreparationShortage>()
                    .observed_preparations,
                1
            );
            assert!(entry.authority_protocol_residual.is_none());
            assert!(entry.active.is_some());
            let pending = entry.pending_commit.as_ref().unwrap();
            assert!(pending.ticket.is_some());
            assert!(pending.final_task.is_none());
            let trace = entry.engine.request(request).unwrap().trace().to_vec();
            assert_eq!(
                trace
                    .iter()
                    .filter(|event| {
                        event.kind() == NavigationTraceEventKind::ViewApplied
                            && event
                                .frame()
                                .is_some_and(|frame| frame.origin() == &FrameOrigin::RouterFallback)
                    })
                    .count(),
                1
            );
            assert!(
                trace
                    .iter()
                    .all(|event| event.kind() != NavigationTraceEventKind::CommitPublished)
            );
            trace
        };

        commit_presentation_system(app.world_mut());

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert!(entry.authority_protocol_residual.is_some());
        assert!(entry.active.is_none());
        assert!(entry.pending_commit.is_none());
        assert_eq!(entry.engine.history(), &history_before);
        assert_eq!(nested_protected_fingerprint(app.world(), host), host_before);
        assert_eq!(
            nested_protected_fingerprint(app.world(), protected),
            protected_before
        );
        let trace = entry.engine.request(request).unwrap().trace().to_vec();
        assert_eq!(
            trace
                .iter()
                .filter(|event| event.kind() == NavigationTraceEventKind::ViewApplied)
                .count(),
            trace_after_shortage
                .iter()
                .filter(|event| event.kind() == NavigationTraceEventKind::ViewApplied)
                .count()
        );
        assert!(
            trace
                .iter()
                .all(|event| event.kind() != NavigationTraceEventKind::CommitPublished)
        );
        assert!(matches!(
            entry.engine.request(request).unwrap().status(),
            RequestStatus::Failed(failure)
                if failure.kind() == NavigationFailureKind::AuthorityLost
        ));
        assert_eq!(
            trace
                .iter()
                .filter(|event| event.kind() == NavigationTraceEventKind::RequestTerminal)
                .count(),
            1
        );
        assert!(
            entry
                .authority_protocol_residual
                .as_ref()
                .and_then(|residual| residual.pending.as_ref())
                .is_some_and(|pending| pending.ticket.is_none() && pending.final_task.is_none())
        );

        {
            let mut navigation = app.world_mut().resource_mut::<RouterNavigation>();
            assert!(navigation.entries[0].engine.action().is_none());
        }
        app.update();
        assert_eq!(
            app.world().resource::<RouterNavigation>().entries[0]
                .engine
                .request(request)
                .unwrap()
                .trace(),
            trace
        );
    }

    #[test]
    fn root_fallback_post_report_source_drift_fail_closes_without_ticket_or_replay() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        app.insert_resource(ProjectionlessPrepareCommitRejection { remaining: 1 });
        app.insert_resource(ProjectionlessFailClosePreparationShortage {
            remaining: 1,
            observed_preparations: 0,
        });
        let router =
            install_failing_root_replacement_router(&mut app, "fallback-post-report-drift");
        app.update();
        let initial = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/one"))
            .unwrap();
        run_updates_until_succeeded(&mut app, initial);
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/two"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_until_projectionless_fallback_pending(&mut app);
        commit_presentation_system(app.world_mut());

        let (active_target, active_claims, pending_claims) = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            (
                entry.active.as_ref().unwrap().interactive_entities()[0],
                entry
                    .active
                    .as_ref()
                    .unwrap()
                    .claims()
                    .cloned()
                    .collect::<Vec<_>>(),
                entry
                    .pending_commit
                    .as_ref()
                    .unwrap()
                    .branch
                    .as_realized()
                    .claims()
                    .cloned()
                    .collect::<Vec<_>>(),
            )
        };
        app.world_mut()
            .entity_mut(active_target)
            .remove::<Pickable>();
        let active_after_drift = nested_protected_fingerprint(app.world(), active_target);
        commit_presentation_system(app.world_mut());

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert!(entry.projectionless_fail_close.is_none());
        assert!(entry.authority_protocol_residual.is_some());
        assert!(entry.active.is_none());
        assert!(entry.pending_commit.is_none());
        assert_eq!(
            app.world()
                .resource::<ProjectionlessFailClosePreparationShortage>()
                .observed_preparations,
            2
        );
        let isolated_active = nested_protected_fingerprint(app.world(), active_target);
        assert_eq!(isolated_active.manifest, active_after_drift.manifest);
        assert_eq!(isolated_active.parent, active_after_drift.parent);
        assert_eq!(isolated_active.children, active_after_drift.children);
        assert_eq!(isolated_active.visibility, Some(Visibility::Hidden));
        assert_eq!(isolated_active.pickable, None);
        assert_eq!(isolated_active.barrier, Some((false, true)));
        let residual = entry.authority_protocol_residual.as_ref().unwrap();
        assert_eq!(
            residual
                .active
                .as_ref()
                .unwrap()
                .claims()
                .cloned()
                .collect::<Vec<_>>(),
            active_claims
        );
        assert_eq!(
            residual
                .pending
                .as_ref()
                .unwrap()
                .branch
                .as_realized()
                .claims()
                .cloned()
                .collect::<Vec<_>>(),
            pending_claims
        );
        assert!(
            residual
                .pending
                .as_ref()
                .is_some_and(|pending| pending.ticket.is_none())
        );
        assert!(matches!(
            entry.engine.request(request).unwrap().status(),
            RequestStatus::Failed(failure)
                if failure.kind() == NavigationFailureKind::AuthorityLost
        ));
        let trace_after_terminal = entry.engine.request(request).unwrap().trace().to_vec();
        assert_eq!(
            trace_after_terminal
                .iter()
                .filter(|event| event.kind() == NavigationTraceEventKind::RequestTerminal)
                .count(),
            1
        );
        assert!(
            trace_after_terminal
                .iter()
                .all(|event| event.cleanup_disposition().is_none())
        );
        {
            let mut navigation = app.world_mut().resource_mut::<RouterNavigation>();
            assert!(navigation.entries[0].engine.action().is_none());
        }

        for _ in 0..3 {
            app.update();
        }
        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert_eq!(
            entry.engine.request(request).unwrap().trace(),
            trace_after_terminal
        );
        assert_eq!(
            app.world()
                .resource::<ProjectionlessFailClosePreparationShortage>()
                .observed_preparations,
            2
        );
    }

    #[test]
    fn root_fallback_live_commit_identity_drift_is_source_drift() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        app.insert_resource(ProjectionlessPrepareCommitRejection { remaining: 1 });
        app.insert_resource(ProjectionlessFailClosePreparationShortage {
            remaining: 1,
            observed_preparations: 0,
        });
        let router = install_failing_root_replacement_router(
            &mut app,
            "fallback-live-commit-identity-drift",
        );
        app.update();
        let initial = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/one"))
            .unwrap();
        run_updates_until_succeeded(&mut app, initial);
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/two"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_until_projectionless_fallback_pending(&mut app);
        commit_presentation_system(app.world_mut());

        {
            let mut navigation = app.world_mut().resource_mut::<RouterNavigation>();
            let entry = &mut navigation.entries[0];
            let ticket = entry
                .pending_commit
                .as_ref()
                .and_then(|pending| pending.ticket.as_ref())
                .unwrap();
            assert_eq!(entry.engine.inspect_commit_identity(ticket), Ok(()));
            entry.engine.testing_invalidate_commit_action();
            let ticket = entry
                .pending_commit
                .as_ref()
                .and_then(|pending| pending.ticket.as_ref())
                .unwrap();
            assert_eq!(
                entry.engine.inspect_commit_identity(ticket),
                Err(IntegrationError::Stale)
            );
        }

        commit_presentation_system(app.world_mut());

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        let residual = entry.authority_protocol_residual.as_ref().unwrap();
        let AuthorityProtocolResidualKind::PostReportSourceDrift(evidence) = &residual.kind else {
            panic!("live commit identity drift must retain source evidence")
        };
        assert!(evidence.expected.report == evidence.observed.report);
        assert!(evidence.expected.observed == evidence.observed.observed);
        assert!(evidence.expected.commit_identity_is_current);
        assert!(!evidence.observed.commit_identity_is_current);
        assert!(
            residual
                .pending
                .as_ref()
                .is_some_and(|pending| pending.ticket.is_none() && pending.final_task.is_none())
        );
        assert!(matches!(
            entry.engine.request(request).unwrap().status(),
            RequestStatus::Failed(failure)
                if failure.kind() == NavigationFailureKind::AuthorityLost
        ));
        let trace = entry.engine.request(request).unwrap().trace().to_vec();
        assert_eq!(
            trace
                .iter()
                .filter(|event| event.kind() == NavigationTraceEventKind::RequestTerminal)
                .count(),
            1
        );
        {
            let mut navigation = app.world_mut().resource_mut::<RouterNavigation>();
            assert!(navigation.entries[0].engine.action().is_none());
        }
        app.update();
        assert_eq!(
            app.world().resource::<RouterNavigation>().entries[0]
                .engine
                .request(request)
                .unwrap()
                .trace(),
            trace
        );
    }

    #[test]
    fn root_fallback_live_host_binding_drift_is_source_drift() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        app.insert_resource(ProjectionlessPrepareCommitRejection { remaining: 1 });
        app.insert_resource(ProjectionlessFailClosePreparationShortage {
            remaining: 1,
            observed_preparations: 0,
        });
        let router = install_failing_root_replacement_router(&mut app, "fallback-live-host-drift");
        app.update();
        let initial = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/one"))
            .unwrap();
        run_updates_until_succeeded(&mut app, initial);
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/two"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_until_projectionless_fallback_pending(&mut app);
        commit_presentation_system(app.world_mut());

        let host = app.world().resource::<RouterNavigation>().entries[0]
            .host
            .as_ref()
            .unwrap()
            .host;
        let before = nested_protected_fingerprint(app.world(), host);
        app.world_mut()
            .entity_mut(host)
            .insert(RouterHostRegistration::new(router));
        assert_eq!(nested_protected_fingerprint(app.world(), host), before);

        commit_presentation_system(app.world_mut());

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        let residual = entry.authority_protocol_residual.as_ref().unwrap();
        let AuthorityProtocolResidualKind::PostReportSourceDrift(evidence) = &residual.kind else {
            panic!("live host binding drift must retain source evidence")
        };
        assert!(evidence.expected.host == evidence.observed.host);
        assert!(evidence.expected.observed == evidence.observed.observed);
        assert!(evidence.expected.host_binding_is_current);
        assert!(!evidence.observed.host_binding_is_current);
        assert!(
            residual
                .pending
                .as_ref()
                .is_some_and(|pending| pending.ticket.is_none() && pending.final_task.is_none())
        );
        assert!(matches!(
            entry.engine.request(request).unwrap().status(),
            RequestStatus::Failed(failure)
                if failure.kind() == NavigationFailureKind::AuthorityLost
        ));
        let trace = entry.engine.request(request).unwrap().trace().to_vec();
        assert_eq!(
            trace
                .iter()
                .filter(|event| event.kind() == NavigationTraceEventKind::RequestTerminal)
                .count(),
            1
        );
        {
            let mut navigation = app.world_mut().resource_mut::<RouterNavigation>();
            assert!(navigation.entries[0].engine.action().is_none());
        }
        app.update();
        assert_eq!(
            app.world().resource::<RouterNavigation>().entries[0]
                .engine
                .request(request)
                .unwrap()
                .trace(),
            trace
        );
    }

    #[test]
    fn core_publish_is_the_last_semantic_commit_step_on_both_paths() {
        let source = include_str!("navigation.rs");
        let commit = source
            .split_once("pub(crate) fn commit_presentation_system")
            .unwrap()
            .1
            .split_once("pub(crate) fn settle_cleanup_system")
            .unwrap()
            .0;

        assert!(!commit.contains("cleanup_batch_matches"));
        let publish_tails = commit
            .match_indices("engine.publish(prepared, || {")
            .map(|(start, _)| {
                let tail = &commit[start..];
                let end = tail
                    .find("Some(())")
                    .expect("each publish must return immediately through its prepared success");
                &tail[..end]
            })
            .collect::<Vec<_>>();
        assert_eq!(publish_tails.len(), 2, "splice and projectionless paths");
        for tail in publish_tails {
            assert!(tail.contains("*active_owner = next_active;"));
            assert!(tail.contains("*cleanup_owner = next_cleanup;"));
            for forbidden in [
                "?",
                ".sort",
                "Vec::",
                "into_active_and_leaving",
                "CleanupLedger::activate",
                "apply_claim_rebind",
                "entry.active =",
                "entry.cleanup_ledger =",
                "world",
                ".query",
                ".get",
                ".resource",
                ".expect",
                ".unwrap",
                ".insert",
                ".remove",
                "despawn",
            ] {
                assert!(
                    !tail.contains(forbidden),
                    "publish tail must contain only prepared in-place mutations: {forbidden}"
                );
            }
        }
    }

    #[test]
    fn failed_realization_residual_survives_ledger_reservation_failure() {
        let mut app = App::new();
        app.world_mut()
            .register_component_hooks::<FrameOutput>()
            .on_add(|mut world, HookContext { entity, .. }| {
                world.commands().entity(entity).insert(CleanupDriftMarker);
            });
        app.add_plugins(BevyUiRouterPlugin::default());
        let router = install_atomic_router(&mut app, "failed-residual-reservation");
        observe_authority_system(app.world_mut());
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        collect_navigation_system(app.world_mut());

        crate::cleanup::fail_next_reservation_for_test();
        drive_navigation_system(app.world_mut());

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert!(entry.pending_failed_branch.is_some());
        assert!(matches!(
            entry.engine.request(request).unwrap().status(),
            RequestStatus::Running(_)
        ));
        let residual_entities = app
            .world()
            .resource::<RouteEntityRegistry>()
            .records()
            .iter()
            .filter_map(|record| {
                matches!(
                    record.owner(),
                    RegistryOwner::Committed(
                        EntityOwner::FrameShell(_)
                            | EntityOwner::FrameContentTarget(_)
                            | EntityOwner::FrameOutput { .. }
                    )
                )
                .then_some(record.entity())
            })
            .collect::<Vec<_>>();
        assert!(!residual_entities.is_empty());
        for entity in &residual_entities {
            assert_eq!(
                app.world().get::<Visibility>(*entity),
                Some(&Visibility::Hidden)
            );
            assert_eq!(
                app.world().get::<Pickable>(*entity),
                Some(&Pickable::IGNORE)
            );
            assert!(
                app.world()
                    .get::<RouterInputBarrier>(*entity)
                    .is_some_and(RouterInputBarrier::denies_all)
            );
        }

        settle_cleanup_system(app.world_mut());
        assert!(
            app.world().resource::<RouterNavigation>().entries[0]
                .pending_failed_branch
                .is_some()
        );
        settle_cleanup_system(app.world_mut());

        assert!(
            app.world().resource::<RouterNavigation>().entries[0]
                .pending_failed_branch
                .is_none()
        );
        let action = app.world_mut().resource_mut::<RouterNavigation>().entries[0]
            .engine
            .action();
        assert!(matches!(action, Some(EngineAction::FrameTask(_))));
        assert!(residual_entities.iter().all(|entity| {
            app.world()
                .resource::<QuarantineLeaseTable>()
                .record(*entity)
                .is_some()
        }));
        let navigation = app.world().resource::<RouterNavigation>();
        let request_view = navigation.entries[0].engine.request(request).unwrap();
        let trace = request_view.trace();
        let settled = trace
            .iter()
            .position(|event| {
                event.kind() == NavigationTraceEventKind::CleanupSettled
                    && event.cleanup_disposition().is_none()
            })
            .unwrap();
        let fallback = trace
            .iter()
            .rposition(|event| {
                event.kind() == NavigationTraceEventKind::FrameTaskIssued
                    && event
                        .frame()
                        .is_some_and(|frame| frame.origin() == &FrameOrigin::RouterFallback)
            })
            .unwrap();
        assert!(settled < fallback);
    }

    #[test]
    fn failed_fallback_root_partial_residual_waits_for_core_cleanup_before_terminal() {
        let mut app = App::new();
        app.world_mut()
            .register_component_hooks::<FrameOutput>()
            .on_add(|mut world, HookContext { entity, .. }| {
                world.commands().entity(entity).insert(CleanupDriftMarker);
            });
        app.add_plugins(BevyUiRouterPlugin::default());
        let router = install_atomic_router(&mut app, "fallback-partial-cleanup");
        observe_authority_system(app.world_mut());
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        collect_navigation_system(app.world_mut());

        drive_navigation_system(app.world_mut());

        {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            assert!(entry.pending_failed_branch.is_some());
            assert!(entry.cleanup_ledger.is_none());
            assert_eq!(
                entry.engine.request(request).unwrap().status(),
                &RequestStatus::Running(NavigationPhase::ApplyingFrame(0))
            );
        }

        settle_cleanup_system(app.world_mut());

        {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            assert!(entry.pending_failed_branch.is_none());
            assert!(entry.cleanup_ledger.is_none());
            assert_eq!(
                entry.engine.request(request).unwrap().status(),
                &RequestStatus::Running(NavigationPhase::Recovering)
            );
        }

        drive_navigation_system(app.world_mut());

        {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            assert!(entry.pending_failed_branch.is_some());
            assert!(entry.cleanup_ledger.is_none());
            assert!(matches!(
                entry.engine.request(request).unwrap().status(),
                RequestStatus::Running(_)
            ));
        }

        settle_cleanup_system(app.world_mut());

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert!(entry.pending_failed_branch.is_none());
        assert!(entry.cleanup_ledger.is_none());
        assert!(matches!(
            entry.engine.request(request).unwrap().status(),
            RequestStatus::Failed(failure)
                if failure.kind() == NavigationFailureKind::RecoveryFailed
        ));
        assert_eq!(
            entry
                .engine
                .request(request)
                .unwrap()
                .trace()
                .iter()
                .filter(|event| {
                    event.kind() == NavigationTraceEventKind::CleanupSettled
                        && event.cleanup_disposition().is_none()
                })
                .count(),
            2
        );
    }

    #[test]
    pub(crate) fn commit_switches_registry_pickable_gate_and_core_history_in_one_system() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let router = install_atomic_router(&mut app, "atomic-replacement");
        observe_authority_system(app.world_mut());
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        commit_presentation_system(app.world_mut());
        settle_cleanup_system(app.world_mut());

        let (old_claims, old_interactive) = {
            let navigation = app.world().resource::<RouterNavigation>();
            let active = navigation.entries[0].active.as_ref().unwrap();
            (
                active.claims().cloned().collect::<Vec<_>>(),
                active.interactive_entities(),
            )
        };
        let old_entities = old_claims
            .iter()
            .map(|claim| claim.entity())
            .collect::<Vec<_>>();
        let old_focus = old_interactive[0];
        *app.world_mut().resource_mut::<InputFocus>() = InputFocus::from_entity(old_focus);
        app.world_mut()
            .resource_mut::<RouteFocusState>()
            .set_focused(Some(old_focus));

        let replacement = RouteLocation::new("/").with_fragment("replacement");
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, replacement.clone())
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());

        let (new_claims, new_interactive) = {
            let navigation = app.world().resource::<RouterNavigation>();
            let pending = navigation.entries[0]
                .pending_commit
                .as_ref()
                .and_then(PendingCommit::frame)
                .unwrap();
            (
                pending.claims().cloned().collect::<Vec<_>>(),
                pending.interactive_entities(),
            )
        };
        let new_entities = new_claims
            .iter()
            .map(|claim| claim.entity())
            .collect::<Vec<_>>();
        assert!(
            old_entities
                .iter()
                .all(|entity| !new_entities.contains(entity))
        );
        assert!(old_claims.iter().all(|claim| {
            app.world()
                .resource::<RouteEntityRegistry>()
                .record(claim.entity())
                .is_some_and(|record| record.lifecycle() == EntityLifecycle::Active)
        }));
        assert!(new_claims.iter().all(|claim| {
            app.world()
                .resource::<RouteEntityRegistry>()
                .record(claim.entity())
                .is_some_and(|record| record.lifecycle() == EntityLifecycle::PreparedInert)
        }));
        assert_eq!(
            app.world().resource::<RouterNavigation>().entries[0]
                .engine
                .history()
                .len(),
            1
        );

        commit_presentation_system(app.world_mut());

        let registry = app.world().resource::<RouteEntityRegistry>();
        assert!(old_claims.iter().all(|claim| {
            registry
                .record(claim.entity())
                .is_some_and(|record| record.lifecycle() == EntityLifecycle::Leaving)
                && app.world().get::<Visibility>(claim.entity()) == Some(&Visibility::Hidden)
                && app.world().get::<Pickable>(claim.entity()) == Some(&Pickable::IGNORE)
                && app
                    .world()
                    .get::<RouterInputBarrier>(claim.entity())
                    .is_some_and(RouterInputBarrier::denies_all)
        }));
        assert!(new_claims.iter().all(|claim| {
            let interactive = new_interactive.contains(&claim.entity());
            registry
                .record(claim.entity())
                .is_some_and(|record| record.lifecycle() == EntityLifecycle::Active)
                && app.world().get::<Visibility>(claim.entity()) == Some(&Visibility::Visible)
                && app.world().get::<Pickable>(claim.entity())
                    == Some(if interactive {
                        &Pickable {
                            should_block_lower: true,
                            is_hoverable: true,
                        }
                    } else {
                        &Pickable::IGNORE
                    })
                && app
                    .world()
                    .get::<RouterInputBarrier>(claim.entity())
                    .is_some_and(|barrier| {
                        if interactive {
                            barrier.allows_all()
                        } else {
                            barrier.denies_all()
                        }
                    })
        }));
        let gate = app.world().resource::<RouterInputGate>();
        assert!(old_interactive.iter().all(|entity| {
            !gate.allows_pointer(*entity)
                && !gate.allows_keyboard(*entity)
                && !gate.allows_gamepad(*entity)
        }));
        assert!(new_interactive.iter().all(|entity| {
            gate.allows_pointer(*entity)
                && gate.allows_keyboard(*entity)
                && gate.allows_gamepad(*entity)
        }));
        assert_eq!(app.world().resource::<InputFocus>().get(), None);
        assert_eq!(app.world().resource::<RouteFocusState>().focused(), None);
        let navigation = app.world().resource::<RouterNavigation>();
        assert_eq!(navigation.entries[0].engine.history().len(), 2);
        assert_eq!(
            navigation.entries[0].engine.history().current(),
            Some(&replacement)
        );
        assert_eq!(
            navigation.entries[0]
                .active
                .as_ref()
                .unwrap()
                .interactive_entities(),
            new_interactive
        );
    }

    #[test]
    fn commits_preserve_other_router_gate_permissions() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let first = install_atomic_router(&mut app, "first");
        let second = install_atomic_router(&mut app, "second");
        observe_authority_system(app.world_mut());
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(first, RouteLocation::new("/"))
            .unwrap();
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(second, RouteLocation::new("/"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        commit_presentation_system(app.world_mut());

        let outputs = app
            .world()
            .resource::<RouterNavigation>()
            .entries
            .iter()
            .map(|entry| entry.active.as_ref().unwrap().interactive_entities()[0])
            .collect::<Vec<_>>();
        let gate = app.world().resource::<RouterInputGate>();
        assert_eq!(outputs.len(), 2);
        assert!(outputs.iter().all(|entity| gate.allows_pointer(*entity)));
    }

    #[test]
    fn atomic_switch_releases_focus_from_the_leaving_frame() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let router = install_atomic_router(&mut app, "focus");
        observe_authority_system(app.world_mut());
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        commit_presentation_system(app.world_mut());
        settle_cleanup_system(app.world_mut());
        assert!(validate_active_frame(
            app.world(),
            app.world().resource::<RouterNavigation>().entries[0]
                .active
                .as_ref()
                .unwrap()
                .first_frame()
                .unwrap()
        ));
        let leaving = app.world().resource::<RouterNavigation>().entries[0]
            .active
            .as_ref()
            .unwrap()
            .interactive_entities()[0];
        *app.world_mut().resource_mut::<InputFocus>() = InputFocus::from_entity(leaving);
        app.world_mut()
            .resource_mut::<RouteFocusState>()
            .set_focused(Some(leaving));

        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/").with_fragment("revision-2"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        commit_presentation_system(app.world_mut());

        let active = app.world().resource::<RouterNavigation>().entries[0]
            .active
            .as_ref()
            .unwrap()
            .interactive_entities()[0];
        assert_ne!(active, leaving);
        assert_eq!(app.world().resource::<InputFocus>().get(), None);
        assert_eq!(app.world().resource::<RouteFocusState>().focused(), None);
        let gate = app.world().resource::<RouterInputGate>();
        assert!(!gate.allows_pointer(leaving));
        assert!(gate.allows_pointer(active));
        assert_eq!(
            app.world().get::<Visibility>(leaving),
            Some(&Visibility::Hidden)
        );
        assert!(validate_active_frame(
            app.world(),
            app.world().resource::<RouterNavigation>().entries[0]
                .active
                .as_ref()
                .unwrap()
                .first_frame()
                .unwrap()
        ));
        *app.world_mut().resource_mut::<InputFocus>() = InputFocus::from_entity(leaving);
        app.world_mut()
            .resource_mut::<RouteFocusState>()
            .set_focused(Some(leaving));
        crate::input::refresh_input_gate_system(app.world_mut());
        assert_eq!(app.world().resource::<InputFocus>().get(), None);
        assert_eq!(app.world().resource::<RouteFocusState>().focused(), None);
        let gate = app.world().resource::<RouterInputGate>();
        assert!(!gate.allows_pointer(leaving));
        assert!(gate.allows_pointer(active));

        settle_cleanup_system(app.world_mut());
        assert!(app.world().get_entity(leaving).is_err());
        assert!(
            app.world()
                .resource::<RouteEntityRegistry>()
                .record(leaving)
                .is_none()
        );
        assert!(validate_active_frame(
            app.world(),
            app.world().resource::<RouterNavigation>().entries[0]
                .active
                .as_ref()
                .unwrap()
                .first_frame()
                .unwrap()
        ));
        assert_eq!(app.world().resource::<InputFocus>().get(), None);
        assert_eq!(app.world().resource::<RouteFocusState>().focused(), None);
        let gate = app.world().resource::<RouterInputGate>();
        assert!(!gate.allows_pointer(leaving));
        assert!(gate.allows_pointer(active));
    }

    #[test]
    fn refresh_clears_focus_from_an_active_claim_whose_frame_validation_failed() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let router = install_atomic_router(&mut app, "invalid-active-focus");
        observe_authority_system(app.world_mut());
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        commit_presentation_system(app.world_mut());
        settle_cleanup_system(app.world_mut());

        let active = app.world().resource::<RouterNavigation>().entries[0]
            .active
            .as_ref()
            .unwrap()
            .interactive_entities()[0];
        let app_owned = app.world_mut().spawn_empty().id();
        assert!(validate_active_frame(
            app.world(),
            app.world().resource::<RouterNavigation>().entries[0]
                .active
                .as_ref()
                .unwrap()
                .first_frame()
                .unwrap()
        ));

        *app.world_mut().resource_mut::<InputFocus>() = InputFocus::from_entity(active);
        app.world_mut()
            .resource_mut::<RouteFocusState>()
            .set_focused(Some(app_owned));
        crate::input::refresh_input_gate_system(app.world_mut());
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(active));
        assert_eq!(
            app.world().resource::<RouteFocusState>().focused(),
            Some(app_owned)
        );

        *app.world_mut().get_mut::<Visibility>(active).unwrap() = Visibility::Hidden;
        assert!(!validate_active_frame(
            app.world(),
            app.world().resource::<RouterNavigation>().entries[0]
                .active
                .as_ref()
                .unwrap()
                .first_frame()
                .unwrap()
        ));
        *app.world_mut().resource_mut::<InputFocus>() = InputFocus::from_entity(active);
        app.world_mut()
            .resource_mut::<RouteFocusState>()
            .set_focused(Some(active));

        crate::input::refresh_input_gate_system(app.world_mut());

        assert_eq!(app.world().resource::<InputFocus>().get(), None);
        assert_eq!(app.world().resource::<RouteFocusState>().focused(), None);
    }

    #[test]
    fn refresh_clears_focus_from_a_detached_quarantined_exact_entity() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let router = install_atomic_router(&mut app, "detached-quarantine-focus");
        observe_authority_system(app.world_mut());
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        commit_presentation_system(app.world_mut());
        settle_cleanup_system(app.world_mut());
        let quarantined = app.world().resource::<RouterNavigation>().entries[0]
            .active
            .as_ref()
            .unwrap()
            .interactive_entities()[0];

        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/").with_fragment("replacement"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        commit_presentation_system(app.world_mut());
        app.world_mut()
            .entity_mut(quarantined)
            .insert(CleanupDriftMarker);
        settle_cleanup_system(app.world_mut());
        let active = app.world().resource::<RouterNavigation>().entries[0]
            .active
            .as_ref()
            .unwrap()
            .interactive_entities()[0];
        assert!(
            app.world()
                .resource::<QuarantineLeaseTable>()
                .record(quarantined)
                .is_some()
        );
        app.world_mut()
            .entity_mut(quarantined)
            .remove::<bevy_ecs::hierarchy::ChildOf>();
        let app_owned = app.world_mut().spawn_empty().id();
        *app.world_mut().resource_mut::<InputFocus>() = InputFocus::from_entity(quarantined);
        app.world_mut()
            .resource_mut::<RouteFocusState>()
            .set_focused(Some(quarantined));

        crate::input::refresh_input_gate_system(app.world_mut());

        assert_eq!(app.world().resource::<InputFocus>().get(), None);
        assert_eq!(app.world().resource::<RouteFocusState>().focused(), None);

        *app.world_mut().resource_mut::<InputFocus>() = InputFocus::from_entity(active);
        app.world_mut()
            .resource_mut::<RouteFocusState>()
            .set_focused(Some(app_owned));
        crate::input::refresh_input_gate_system(app.world_mut());
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(active));
        assert_eq!(
            app.world().resource::<RouteFocusState>().focused(),
            Some(app_owned)
        );
    }

    #[test]
    fn active_frame_and_input_gate_reject_reassigned_host_authority() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let router = install_atomic_router(&mut app, "reassigned-host");
        observe_authority_system(app.world_mut());
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        commit_presentation_system(app.world_mut());
        settle_cleanup_system(app.world_mut());

        let (host, output) = {
            let navigation = app.world().resource::<RouterNavigation>();
            let entry = &navigation.entries[0];
            (
                entry.host.as_ref().unwrap().host,
                entry.active.as_ref().unwrap().interactive_entities()[0],
            )
        };
        assert!(
            app.world()
                .resource::<RouterInputGate>()
                .allows_pointer(output)
        );

        let registration = app
            .world_mut()
            .entity_mut(host)
            .take::<RouterHostRegistration>()
            .unwrap();
        app.world_mut().entity_mut(host).insert(registration);
        let active = app.world().resource::<RouterNavigation>().entries[0]
            .active
            .as_ref()
            .unwrap();
        assert!(!validate_active_frame(
            app.world(),
            active.first_frame().unwrap()
        ));

        crate::input::refresh_input_gate_system(app.world_mut());
        assert!(
            !app.world()
                .resource::<RouterInputGate>()
                .allows_pointer(output)
        );
        assert!(app.world().get_entity(host).is_ok());
    }

    #[test]
    fn refresh_physically_isolates_an_exact_active_frame_after_host_incarnation_changes() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let router = install_atomic_router(&mut app, "isolate-invalid-active");
        observe_authority_system(app.world_mut());
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        commit_presentation_system(app.world_mut());
        settle_cleanup_system(app.world_mut());

        let (host, claims, output) = {
            let navigation = app.world().resource::<RouterNavigation>();
            let entry = &navigation.entries[0];
            let active = entry.active.as_ref().unwrap();
            (
                entry.host.as_ref().unwrap().host,
                active.claims().cloned().collect::<Vec<_>>(),
                active.interactive_entities()[0],
            )
        };
        let foreign = app
            .world_mut()
            .spawn((
                Visibility::Visible,
                Pickable {
                    should_block_lower: true,
                    is_hoverable: true,
                },
                RouterInputBarrier::allow_all(),
            ))
            .id();
        *app.world_mut().resource_mut::<InputFocus>() = InputFocus::from_entity(output);
        app.world_mut()
            .resource_mut::<RouteFocusState>()
            .set_focused(Some(output));

        let registration = app
            .world_mut()
            .entity_mut(host)
            .take::<RouterHostRegistration>()
            .unwrap();
        app.world_mut().entity_mut(host).insert(registration);
        assert!(!validate_active_frame(
            app.world(),
            app.world().resource::<RouterNavigation>().entries[0]
                .active
                .as_ref()
                .unwrap()
                .first_frame()
                .unwrap()
        ));

        crate::input::refresh_input_gate_system(app.world_mut());

        assert!(claims.iter().all(|claim| {
            app.world()
                .resource::<RouteEntityRegistry>()
                .record(claim.entity())
                .is_some_and(|record| record.claim() == *claim)
                && app.world().get::<Visibility>(claim.entity()) == Some(&Visibility::Hidden)
                && app.world().get::<Pickable>(claim.entity()) == Some(&Pickable::IGNORE)
                && app
                    .world()
                    .get::<RouterInputBarrier>(claim.entity())
                    .is_some_and(RouterInputBarrier::denies_all)
        }));
        assert!(
            !app.world()
                .resource::<RouterInputGate>()
                .allows_pointer(output)
        );
        assert_eq!(app.world().resource::<InputFocus>().get(), None);
        assert_eq!(app.world().resource::<RouteFocusState>().focused(), None);
        assert_eq!(
            app.world().get::<Visibility>(foreign),
            Some(&Visibility::Visible)
        );
        assert!(
            app.world()
                .get::<Pickable>(foreign)
                .is_some_and(|pickable| pickable.should_block_lower && pickable.is_hoverable)
        );
        assert!(
            app.world()
                .get::<RouterInputBarrier>(foreign)
                .is_some_and(RouterInputBarrier::allows_all)
        );
        assert!(app.world().get::<RouterHostRegistration>(host).is_some());
        assert_eq!(
            app.world().resource::<RouterNavigation>().entries[0]
                .engine
                .history()
                .len(),
            1
        );
    }

    #[test]
    fn refresh_narrows_every_existing_capability_when_one_exact_entity_lacks_pickable() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let router = install_atomic_router(&mut app, "isolate-partial-capability");
        observe_authority_system(app.world_mut());
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        commit_presentation_system(app.world_mut());
        settle_cleanup_system(app.world_mut());

        let (host, claims, output) = {
            let navigation = app.world().resource::<RouterNavigation>();
            let entry = &navigation.entries[0];
            let active = entry.active.as_ref().unwrap();
            (
                entry.host.as_ref().unwrap().host,
                active.claims().cloned().collect::<Vec<_>>(),
                active.interactive_entities()[0],
            )
        };
        let missing_pickable = claims
            .iter()
            .map(|claim| claim.entity())
            .find(|entity| *entity != output)
            .unwrap();
        app.world_mut()
            .entity_mut(missing_pickable)
            .remove::<Pickable>();
        let foreign = app
            .world_mut()
            .spawn((
                Visibility::Visible,
                Pickable {
                    should_block_lower: true,
                    is_hoverable: true,
                },
                RouterInputBarrier::allow_all(),
            ))
            .id();
        let registration = app
            .world_mut()
            .entity_mut(host)
            .take::<RouterHostRegistration>()
            .unwrap();
        app.world_mut().entity_mut(host).insert(registration);

        crate::input::refresh_input_gate_system(app.world_mut());

        assert_eq!(app.world().get::<Pickable>(missing_pickable), None);
        assert!(claims.iter().all(|claim| {
            app.world().get::<Visibility>(claim.entity()) == Some(&Visibility::Hidden)
                && app
                    .world()
                    .get::<RouterInputBarrier>(claim.entity())
                    .is_some_and(RouterInputBarrier::denies_all)
                && (claim.entity() == missing_pickable
                    || app.world().get::<Pickable>(claim.entity()) == Some(&Pickable::IGNORE))
        }));
        assert_eq!(
            app.world().get::<Visibility>(foreign),
            Some(&Visibility::Visible)
        );
        assert!(
            app.world()
                .get::<Pickable>(foreign)
                .is_some_and(|pickable| pickable.should_block_lower && pickable.is_hoverable)
        );
        assert!(
            app.world()
                .get::<RouterInputBarrier>(foreign)
                .is_some_and(RouterInputBarrier::allows_all)
        );
        assert!(app.world().get::<RouterHostRegistration>(host).is_some());
    }

    #[test]
    fn active_frame_rejects_reregistered_top_slot_claim() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let router = install_atomic_router(&mut app, "reregistered-slot");
        observe_authority_system(app.world_mut());
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        commit_presentation_system(app.world_mut());
        settle_cleanup_system(app.world_mut());

        let top_slot = app.world().resource::<RouterNavigation>().entries[0]
            .host
            .as_ref()
            .unwrap()
            .top_slot;
        let (old_claim, owner, manifest) = {
            let record = app
                .world()
                .resource::<RouteEntityRegistry>()
                .record(top_slot)
                .unwrap();
            (
                record.claim(),
                record.owner().clone(),
                record.manifest().clone(),
            )
        };
        let new_seal = {
            let mut registry = app.world_mut().resource_mut::<RouteEntityRegistry>();
            assert!(registry.remove_exact(&old_claim));
            let seal = registry.issue_seal();
            registry
                .register(
                    top_slot,
                    owner,
                    seal,
                    manifest,
                    EntityLifecycle::PreparedInert,
                )
                .unwrap();
            seal
        };
        app.world_mut()
            .entity_mut(top_slot)
            .insert(RouteOwnershipSeal::new(new_seal));

        let active = app.world().resource::<RouterNavigation>().entries[0]
            .active
            .as_ref()
            .unwrap();
        assert!(!validate_active_frame(
            app.world(),
            active.first_frame().unwrap()
        ));
    }

    #[test]
    pub(crate) fn locked_bevy_plugins_produce_visible_layout_and_native_picking() {
        let mut app = App::new();
        app.add_plugins(DefaultPlugins);
        app.add_plugins(BevyUiRouterPlugin::default());
        app.world_mut()
            .insert_resource(NativeTargetDeliveries::default());
        install_native_pointer_observer(&mut app);

        let primary_windows = app
            .world_mut()
            .query_filtered::<Entity, With<PrimaryWindow>>()
            .iter(app.world())
            .collect::<Vec<_>>();
        assert_eq!(primary_windows.len(), 1);
        app.world_mut()
            .get_mut::<Window>(primary_windows[0])
            .unwrap()
            .resolution
            .set(200.0, 120.0);
        app.world_mut().spawn(Camera2d);

        let driver = ViewDriverKey::new("physical-driver").unwrap();
        app.register_ui_view_driver(AtomicDriver {
            key: driver.clone(),
        })
        .unwrap();
        let view = RouteViewKey::new("root").unwrap();
        let routes = RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/").unwrap(),
                RouteNodeOptions::new().with_view(view.clone()),
            )
            .unwrap()
            .build()
            .unwrap();
        let mut views = RouteViewRegistry::new();
        views
            .insert(view.clone(), RouteViewDescriptor::new(driver))
            .unwrap();
        let router = app
            .add_ui_router(RouterConfig::new(routes, views, view))
            .unwrap();
        app.world_mut().spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..Node::default()
            },
            RouterHostRegistration::new(router),
        ));

        app.update();
        assert!(
            app.world().resource::<RouterNavigation>().entries[0]
                .host
                .is_some(),
            "physical host admission failed: records={:?} leases={:?}",
            app.world()
                .resource::<RouteEntityRegistry>()
                .records()
                .iter()
                .map(|record| (
                    record.entity(),
                    record.owner(),
                    record.manifest().component_ids()
                ))
                .collect::<Vec<_>>(),
            app.world()
                .resource::<QuarantineLeaseTable>()
                .leases()
                .iter()
                .map(|lease| lease.owned_entities())
                .collect::<Vec<_>>()
        );
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        app.update();

        let output = app
            .world()
            .resource::<RouteEntityRegistry>()
            .records()
            .iter()
            .find_map(|record| {
                matches!(
                    record.owner(),
                    RegistryOwner::Committed(EntityOwner::FrameOutput { .. })
                )
                .then_some(record.entity())
            })
            .unwrap_or_else(|| {
                let navigation = app.world().resource::<RouterNavigation>();
                let entry = &navigation.entries[0];
                let request_state = match app
                    .world()
                    .resource::<RouterSnapshots>()
                    .request(router, request)
                {
                    crate::snapshots::RequestLookup::Found(snapshot) => {
                        format!("{:?} {:?}", snapshot.status(), snapshot.trace())
                    }
                    lookup => format!("{lookup:?}"),
                };
                panic!(
                    "physical output missing: host={} pending={} active={} cleanup={} history={} request={} records={:?}",
                    entry.host.is_some(),
                    entry.pending_commit.is_some(),
                    entry.active.is_some(),
                    entry.cleanup_ledger.is_some(),
                    entry.engine.history().len(),
                    request_state,
                    app.world()
                        .resource::<RouteEntityRegistry>()
                        .records()
                        .iter()
                        .map(|record| record.owner())
                        .collect::<Vec<_>>()
                )
            });
        app.update();
        assert!(
            app.world()
                .resource::<RouteEntityRegistry>()
                .records()
                .iter()
                .all(|record| record.manifest().matches(app.world(), record.entity()))
        );
        app.world_mut().resource_scope(
            |world, navigation: bevy_ecs::change_detection::Mut<RouterNavigation>| {
                crate::presentation::assert_locked_component_manifest_closure(
                    world,
                    navigation.entries[0]
                        .active
                        .as_ref()
                        .unwrap()
                        .first_frame()
                        .unwrap(),
                );
            },
        );
        let computed = app.world().get::<ComputedNode>(output).unwrap();
        assert!(
            computed.size().cmpgt(Vec2::ZERO).all(),
            "UI layout remained zero: {:?}",
            app.world()
                .iter_entities()
                .filter_map(|entity| {
                    let node = entity.get::<Node>()?;
                    Some((
                        entity.id(),
                        node.width,
                        node.height,
                        entity.get::<ComputedNode>().map(ComputedNode::size),
                        entity
                            .get::<bevy_ui::ComputedUiTargetCamera>()
                            .and_then(bevy_ui::ComputedUiTargetCamera::get),
                        entity
                            .get::<bevy_ecs::hierarchy::ChildOf>()
                            .map(|parent| parent.parent()),
                    ))
                })
                .collect::<Vec<_>>()
        );
        assert!(
            app.world()
                .get::<InheritedVisibility>(output)
                .is_some_and(|visibility| visibility.get())
        );

        let center = *app.world().get::<UiGlobalTransform>(output).unwrap() * Vec2::ZERO;
        let pointer = app
            .world_mut()
            .query::<(Entity, &PointerId)>()
            .iter(app.world())
            .find_map(|(entity, id)| (*id == PointerId::Mouse).then_some(entity))
            .unwrap();
        *app.world_mut().get_mut::<PointerLocation>(pointer).unwrap() =
            PointerLocation::new(Location {
                target: NormalizedRenderTarget::Window(
                    WindowRef::Primary
                        .normalize(Some(primary_windows[0]))
                        .unwrap(),
                ),
                position: center,
            });

        app.update();

        assert!(
            app.world()
                .resource::<HoverMap>()
                .get(&PointerId::Mouse)
                .is_some_and(|hits| hits.contains_key(&output))
        );
        assert_eq!(
            app.world()
                .resource::<NativeTargetDeliveries>()
                .0
                .get(&output),
            Some(&1),
            "active primitive must receive the native target event"
        );

        let replacement_request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(
                router,
                RouteLocation::new("/").with_fragment("native-picking-replacement"),
            )
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        let pending = app.world().resource::<RouterNavigation>().entries[0]
            .pending_commit
            .as_ref()
            .and_then(PendingCommit::frame)
            .unwrap()
            .interactive_entities()[0];
        app.world_mut().run_schedule(PostUpdate);
        app.world_mut().run_schedule(PreUpdate);
        let pending_hover = app
            .world()
            .resource::<HoverMap>()
            .get(&PointerId::Mouse)
            .cloned()
            .unwrap_or_default();
        assert!(pending_hover.contains_key(&output));
        assert!(!pending_hover.contains_key(&pending));
        assert_eq!(
            app.world()
                .resource::<NativeTargetDeliveries>()
                .0
                .get(&pending),
            None,
            "pending primitive must not receive a native target event"
        );

        commit_presentation_system(app.world_mut());
        let leaving_hover = {
            app.world_mut().run_schedule(PreUpdate);
            app.world()
                .resource::<HoverMap>()
                .get(&PointerId::Mouse)
                .cloned()
                .unwrap_or_default()
        };
        assert!(!leaving_hover.contains_key(&output));
        assert!(!leaving_hover.contains_key(&pending));
        assert_eq!(
            app.world()
                .resource::<NativeTargetDeliveries>()
                .0
                .get(&output),
            Some(&1),
            "leaving primitive must not receive another native target event"
        );
        assert_eq!(
            app.world()
                .resource::<NativeTargetDeliveries>()
                .0
                .get(&pending),
            None,
            "entering primitive remains event-isolated until it is active"
        );
        settle_cleanup_system(app.world_mut());
        app.world_mut().run_schedule(PostUpdate);
        app.world_mut().run_schedule(PreUpdate);
        let replacement = app.world().resource::<RouterNavigation>().entries[0]
            .active
            .as_ref()
            .unwrap()
            .interactive_entities()[0];
        assert_ne!(
            replacement,
            output,
            "replacement did not publish: status={:?} pending={} cleanup={} active_valid={} pending_valid={} host_valid={} records={:?}",
            app.world().resource::<RouterNavigation>().entries[0]
                .engine
                .request(replacement_request)
                .unwrap()
                .status(),
            app.world().resource::<RouterNavigation>().entries[0]
                .pending_commit
                .is_some(),
            app.world().resource::<RouterNavigation>().entries[0]
                .cleanup_ledger
                .is_some(),
            validate_active_frame(
                app.world(),
                app.world().resource::<RouterNavigation>().entries[0]
                    .active
                    .as_ref()
                    .unwrap()
                    .first_frame()
                    .unwrap()
            ),
            app.world().resource::<RouterNavigation>().entries[0]
                .pending_commit
                .as_ref()
                .and_then(PendingCommit::frame)
                .is_some_and(|frame| validate_realized_frame(app.world(), frame)),
            app.world().resource::<RouterNavigation>().entries[0]
                .host
                .as_ref()
                .is_some_and(|host| validated_host_candidate_is_current(
                    app.world(),
                    &host.candidate
                )),
            app.world()
                .resource::<RouteEntityRegistry>()
                .records()
                .iter()
                .map(|record| {
                    (
                        record.entity(),
                        record.owner(),
                        record.lifecycle(),
                        ComponentManifest::from_world(app.world(), record.entity()).map(|actual| {
                            actual
                                .component_ids()
                                .iter()
                                .filter(|id| !record.manifest().component_ids().contains(id))
                                .filter_map(|id| {
                                    app.world()
                                        .components()
                                        .get_info(*id)
                                        .map(|info| info.name().to_owned())
                                })
                                .collect::<Vec<_>>()
                        }),
                        ComponentManifest::from_world(app.world(), record.entity()).map(|actual| {
                            record
                                .manifest()
                                .component_ids()
                                .iter()
                                .filter(|id| !actual.component_ids().contains(id))
                                .filter_map(|id| {
                                    app.world()
                                        .components()
                                        .get_info(*id)
                                        .map(|info| info.name().to_owned())
                                })
                                .collect::<Vec<_>>()
                        }),
                    )
                })
                .collect::<Vec<_>>()
        );
        let hover = app
            .world()
            .resource::<HoverMap>()
            .get(&PointerId::Mouse)
            .cloned()
            .unwrap_or_default();
        assert!(!hover.contains_key(&output));
        assert!(hover.contains_key(&replacement));
        assert_eq!(
            app.world()
                .resource::<NativeTargetDeliveries>()
                .0
                .get(&replacement),
            Some(&1),
            "the committed replacement must receive the native target event"
        );
        app.world_mut().resource_scope(
            |world, navigation: bevy_ecs::change_detection::Mut<RouterNavigation>| {
                crate::presentation::assert_locked_component_manifest_closure(
                    world,
                    navigation.entries[0]
                        .active
                        .as_ref()
                        .unwrap()
                        .first_frame()
                        .unwrap(),
                );
            },
        );
        assert!(app.world().get_entity(output).is_err());
        assert!(
            app.world()
                .resource::<RouteEntityRegistry>()
                .record(output)
                .is_none()
        );
        assert!(
            app.world()
                .resource::<QuarantineLeaseTable>()
                .record(output)
                .is_none()
        );
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<PrimaryWindow>>()
                .iter(app.world())
                .count(),
            1
        );
    }

    #[test]
    pub(crate) fn host_grant_observer_drift_is_rejected_by_the_real_authority_system() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        install_host_grant_observer_drift(&mut app);
        let router = install_atomic_router(&mut app, "host-grant-observer");
        let host = registered_host(&mut app, router);
        let children_before = app
            .world()
            .get::<bevy_ecs::hierarchy::Children>(host)
            .map(|children| children.iter().copied().collect::<Vec<_>>())
            .unwrap_or_default();

        observe_authority_system(app.world_mut());

        let first_trace = app.world().resource::<RouterNavigation>().entries[0]
            .engine
            .snapshot()
            .host()
            .trace()
            .to_vec();
        assert_eq!(
            first_trace
                .iter()
                .map(bevy_ui_router_core::HostTraceEvent::kind)
                .collect::<Vec<_>>(),
            vec![
                bevy_ui_router_core::HostTraceEventKind::AdmissionRequested,
                bevy_ui_router_core::HostTraceEventKind::HostGranted,
                bevy_ui_router_core::HostTraceEventKind::AdmissionRejectedAfterMutation,
            ]
        );
        let rejected = first_trace
            .iter()
            .find(|event| {
                event.kind()
                    == bevy_ui_router_core::HostTraceEventKind::AdmissionRejectedAfterMutation
            })
            .unwrap()
            .clone();
        assert!(rejected.version().is_some());
        assert!(rejected.slot().is_some());
        assert_eq!(
            app.world()
                .resource::<HostGrantObserverDrift>()
                .observed_slots
                .len(),
            1
        );
        let rejected_slot_entity = app
            .world()
            .resource::<HostGrantObserverDrift>()
            .observed_slots[0];
        assert!(app.world().get_entity(rejected_slot_entity).is_err());
        assert!(
            app.world().resource::<RouterNavigation>().entries[0]
                .host
                .is_none()
        );
        assert!(
            app.world()
                .resource::<RouteEntityRegistry>()
                .records()
                .is_empty()
        );
        assert_eq!(
            app.world()
                .get::<bevy_ecs::hierarchy::Children>(host)
                .map(|children| children.iter().copied().collect::<Vec<_>>())
                .unwrap_or_default(),
            children_before
        );
        let rejected_revision = current_candidate_revision(&mut app, router);
        assert!(
            !app.world()
                .resource::<HostCandidateRevisionLatch>()
                .allows(rejected_revision)
        );

        observe_authority_system(app.world_mut());
        assert_eq!(
            app.world()
                .resource::<HostGrantObserverDrift>()
                .observed_slots
                .len(),
            1,
            "the rejected revision must not allocate or write a second provisional slot"
        );
        assert_eq!(
            app.world().resource::<RouterNavigation>().entries[0]
                .engine
                .snapshot()
                .host()
                .trace(),
            first_trace
        );

        app.world_mut()
            .resource_mut::<HostGrantObserverDrift>()
            .enabled = false;
        let registration = app
            .world_mut()
            .entity_mut(host)
            .take::<RouterHostRegistration>()
            .unwrap();
        app.world_mut().entity_mut(host).insert(registration);
        let fresh_revision = current_candidate_revision(&mut app, router);
        assert_ne!(fresh_revision, rejected_revision);
        observe_authority_system(app.world_mut());

        let navigation = app.world().resource::<RouterNavigation>();
        let entry = &navigation.entries[0];
        let host_snapshot = entry.engine.snapshot().host().clone();
        assert!(entry.host.is_some());
        assert_eq!(
            host_snapshot
                .trace()
                .iter()
                .filter(|event| {
                    event.kind() == bevy_ui_router_core::HostTraceEventKind::HostReady
                })
                .count(),
            1
        );
        assert_ne!(host_snapshot.version(), rejected.version());
        assert_ne!(host_snapshot.top_slot(), rejected.slot());
        assert!(host_snapshot.trace().iter().all(|event| {
            event.kind() != bevy_ui_router_core::HostTraceEventKind::HostReady
                || event.version() != rejected.version()
                || event.slot() != rejected.slot()
        }));
    }

    #[test]
    pub(crate) fn persistent_host_observer_latches_each_revision_to_one_attempt() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        install_host_grant_observer_drift(&mut app);
        let router = install_atomic_router(&mut app, "persistent-host-observer");
        let host = registered_host(&mut app, router);
        let children_before = app
            .world()
            .get::<bevy_ecs::hierarchy::Children>(host)
            .map(|children| children.iter().copied().collect::<Vec<_>>())
            .unwrap_or_default();

        observe_authority_system(app.world_mut());
        let first_revision = current_candidate_revision(&mut app, router);
        let live_entities_after_first = app.world().iter_entities().count();
        for _ in 0..3 {
            observe_authority_system(app.world_mut());
        }
        assert_eq!(
            app.world()
                .resource::<HostGrantObserverDrift>()
                .observed_slots
                .len(),
            1
        );
        assert_eq!(
            app.world().iter_entities().count(),
            live_entities_after_first
        );
        assert!(
            !app.world()
                .resource::<HostCandidateRevisionLatch>()
                .allows(first_revision)
        );

        let registration = app
            .world_mut()
            .entity_mut(host)
            .take::<RouterHostRegistration>()
            .unwrap();
        app.world_mut().entity_mut(host).insert(registration);
        let second_revision = current_candidate_revision(&mut app, router);
        assert_ne!(second_revision, first_revision);
        observe_authority_system(app.world_mut());
        let live_entities_after_second = app.world().iter_entities().count();
        for _ in 0..3 {
            observe_authority_system(app.world_mut());
        }

        let observed_slots = &app
            .world()
            .resource::<HostGrantObserverDrift>()
            .observed_slots;
        assert_eq!(observed_slots.len(), 2);
        assert_ne!(observed_slots[0], observed_slots[1]);
        assert_eq!(
            app.world().iter_entities().count(),
            live_entities_after_second
        );
        assert!(
            !app.world()
                .resource::<HostCandidateRevisionLatch>()
                .allows(second_revision)
        );
        assert!(
            app.world().resource::<RouterNavigation>().entries[0]
                .host
                .is_none()
        );
        assert!(
            app.world()
                .resource::<RouteEntityRegistry>()
                .records()
                .is_empty()
        );
        assert_eq!(
            app.world()
                .get::<bevy_ecs::hierarchy::Children>(host)
                .map(|children| children.iter().copied().collect::<Vec<_>>())
                .unwrap_or_default(),
            children_before
        );

        let trace = app.world().resource::<RouterNavigation>().entries[0]
            .engine
            .snapshot()
            .host()
            .trace()
            .to_vec();
        let rejected = trace
            .iter()
            .filter(|event| {
                event.kind()
                    == bevy_ui_router_core::HostTraceEventKind::AdmissionRejectedAfterMutation
            })
            .collect::<Vec<_>>();
        assert_eq!(rejected.len(), 2);
        assert_ne!(rejected[0].version(), rejected[1].version());
        assert_ne!(rejected[0].slot(), rejected[1].slot());
        assert_eq!(
            trace
                .iter()
                .filter(|event| {
                    event.kind() == bevy_ui_router_core::HostTraceEventKind::HostReady
                })
                .count(),
            0
        );
    }

    #[test]
    fn failed_host_revision_is_latched_before_the_next_observation() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let driver = ViewDriverKey::new("latch-driver").unwrap();
        app.register_ui_view_driver(AtomicDriver {
            key: driver.clone(),
        })
        .unwrap();
        let view = RouteViewKey::new("root").unwrap();
        let routes = RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/").unwrap(),
                RouteNodeOptions::new().with_view(view.clone()),
            )
            .unwrap()
            .build()
            .unwrap();
        let mut views = RouteViewRegistry::new();
        views
            .insert(view.clone(), RouteViewDescriptor::new(driver))
            .unwrap();
        let router = app
            .add_ui_router(RouterConfig::new(routes, views, view))
            .unwrap();
        app.world_mut()
            .spawn((Node::default(), RouterHostRegistration::new(router)));
        let revision = app.world_mut().resource_scope(
            |world, mut revisions: bevy_ecs::change_detection::Mut<HostCandidateRevisionState>| {
                let HostCandidateClassification::Unique(candidate) =
                    classify_host_candidate(world, router, &mut revisions)
                else {
                    panic!("host should classify")
                };
                candidate.revision()
            },
        );
        let blocker = app.world_mut().spawn_empty().id();
        {
            let mut registry = app.world_mut().resource_mut::<RouteEntityRegistry>();
            let seal = registry.issue_seal();
            registry
                .register(
                    blocker,
                    RegistryOwner::Provisional(ProvisionalOwner::HostTopSlot(
                        ProvisionalTopSlotOwner::new(revision),
                    )),
                    seal,
                    ComponentManifest::new(Vec::new()),
                    EntityLifecycle::PreparedInert,
                )
                .unwrap();
        }

        observe_authority_system(app.world_mut());

        assert!(
            !app.world()
                .resource::<crate::host::HostCandidateRevisionLatch>()
                .allows(revision)
        );
        assert!(
            app.world().resource::<RouterNavigation>().entries[0]
                .host
                .is_none()
        );
    }

    #[test]
    fn changed_host_incarnation_rejects_commit_before_any_visible_write() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let driver = ViewDriverKey::new("incarnation-driver").unwrap();
        app.register_ui_view_driver(AtomicDriver {
            key: driver.clone(),
        })
        .unwrap();
        let view = RouteViewKey::new("root").unwrap();
        let routes = RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/").unwrap(),
                RouteNodeOptions::new().with_view(view.clone()),
            )
            .unwrap()
            .build()
            .unwrap();
        let mut views = RouteViewRegistry::new();
        views
            .insert(view.clone(), RouteViewDescriptor::new(driver))
            .unwrap();
        let router = app
            .add_ui_router(RouterConfig::new(routes, views, view))
            .unwrap();
        let host = app
            .world_mut()
            .spawn((Node::default(), RouterHostRegistration::new(router)))
            .id();
        observe_authority_system(app.world_mut());
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        let output = app
            .world()
            .resource::<RouteEntityRegistry>()
            .records()
            .iter()
            .find_map(|record| {
                matches!(
                    record.owner(),
                    RegistryOwner::Committed(EntityOwner::FrameOutput { .. })
                )
                .then_some(record.entity())
            })
            .unwrap();
        let registration = app
            .world_mut()
            .entity_mut(host)
            .take::<RouterHostRegistration>()
            .unwrap();
        app.world_mut().entity_mut(host).insert(registration);

        commit_presentation_system(app.world_mut());

        assert_eq!(
            app.world()
                .resource::<RouteEntityRegistry>()
                .record(output)
                .unwrap()
                .lifecycle(),
            EntityLifecycle::PreparedInert
        );
        assert_eq!(app.world().get::<Pickable>(output), Some(&Pickable::IGNORE));
        assert_eq!(
            app.world().resource::<RouterNavigation>().entries[0]
                .engine
                .history()
                .len(),
            0
        );
        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert!(entry.pending_commit.is_none());
        assert!(entry.active.is_none());
        assert!(
            entry
                .cleanup_ledger
                .as_ref()
                .is_some_and(CleanupLedger::is_authority_settlement)
        );
        assert_eq!(
            entry.engine.snapshot().availability(),
            &RouterAvailability::RecoveringAuthority
        );
        assert!(matches!(
            entry.engine.request(request).unwrap().status(),
            RequestStatus::Running(NavigationPhase::Recovering)
        ));
        assert!(app.world().get_entity(host).is_ok());
    }

    #[test]
    fn changed_host_incarnation_rejects_before_driver_or_realization() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let driver = ViewDriverKey::new("stale-host-driver").unwrap();
        app.register_ui_view_driver(PanicDriver {
            key: driver.clone(),
        })
        .unwrap();
        let view = RouteViewKey::new("stale-host-view").unwrap();
        let routes = RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/").unwrap(),
                RouteNodeOptions::new().with_view(view.clone()),
            )
            .unwrap()
            .build()
            .unwrap();
        let mut views = RouteViewRegistry::new();
        views
            .insert(view.clone(), RouteViewDescriptor::new(driver))
            .unwrap();
        let router = app
            .add_ui_router(RouterConfig::new(routes, views, view))
            .unwrap();
        let host = app
            .world_mut()
            .spawn((Node::default(), RouterHostRegistration::new(router)))
            .id();
        observe_authority_system(app.world_mut());
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        let records_before = app
            .world()
            .resource::<RouteEntityRegistry>()
            .records()
            .len();
        let registration = app
            .world_mut()
            .entity_mut(host)
            .take::<RouterHostRegistration>()
            .unwrap();
        app.world_mut().entity_mut(host).insert(registration);

        drive_navigation_system(app.world_mut());

        let navigation = app.world().resource::<RouterNavigation>();
        let entry = &navigation.entries[0];
        assert!(entry.pending_commit.is_none());
        assert!(entry.active.is_none());
        assert_eq!(
            app.world()
                .resource::<RouteEntityRegistry>()
                .records()
                .len(),
            records_before
        );
        let request = entry.engine.request(request).unwrap();
        assert!(matches!(request.status(), RequestStatus::Running(_)));
        assert_eq!(entry.engine.history().len(), 0);
    }

    #[test]
    fn driver_failure_terminates_one_request_and_preserves_the_next_action() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let driver = ViewDriverKey::new("fail-once-driver").unwrap();
        app.register_ui_view_driver(FailOnceDriver {
            key: driver.clone(),
            failed: false,
        })
        .unwrap();
        let view = RouteViewKey::new("fail-once-view").unwrap();
        let routes = RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/").unwrap(),
                RouteNodeOptions::new().with_view(view.clone()),
            )
            .unwrap()
            .build()
            .unwrap();
        let mut views = RouteViewRegistry::new();
        views
            .insert(view.clone(), RouteViewDescriptor::new(driver))
            .unwrap();
        let router = app
            .add_ui_router(RouterConfig::new(routes, views, view))
            .unwrap();
        app.world_mut()
            .spawn((Node::default(), RouterHostRegistration::new(router)));
        observe_authority_system(app.world_mut());
        let first = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        let second = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(
                router,
                RouteLocation::new("/").with_fragment("second-request"),
            )
            .unwrap();
        collect_navigation_system(app.world_mut());

        drive_navigation_system(app.world_mut());

        {
            let navigation = app.world().resource::<RouterNavigation>();
            let entry = &navigation.entries[0];
            let first = entry.engine.request(first).unwrap();
            assert!(matches!(first.status(), RequestStatus::Running(_)));
            assert!(entry.pending_commit.is_none());
            assert_eq!(entry.engine.history().len(), 0);
        }

        drive_navigation_system(app.world_mut());
        commit_presentation_system(app.world_mut());
        settle_cleanup_system(app.world_mut());

        {
            let navigation = app.world().resource::<RouterNavigation>();
            let entry = &navigation.entries[0];
            let request = entry.engine.request(first).unwrap();
            let status = request.status();
            let RequestStatus::Failed(failure) = status else {
                panic!(
                    "fallback publication must retain the original build failure: {status:?}; pending={:?} projection={:?} failed={} cleanup={} active={}",
                    entry.pending_commit.as_ref().map(|pending| (
                        pending.ticket.is_some(),
                        pending.final_task.is_some(),
                        pending.branch.frames().len(),
                        pending.branch.child_slots().len(),
                    )),
                    entry
                        .pending_commit
                        .as_ref()
                        .and_then(|pending| pending.final_task.as_ref())
                        .map(|task| entry
                            .engine
                            .reconciliation_projection_for_task(task)
                            .is_ok()),
                    entry.pending_failed_branch.is_some(),
                    entry.cleanup_ledger.is_some(),
                    entry.active.is_some(),
                )
            };
            assert_eq!(failure.kind(), NavigationFailureKind::Build);
            assert_eq!(entry.engine.history().len(), 0);
            assert!(entry.active.is_some());
        }

        drive_navigation_system(app.world_mut());
        commit_presentation_system(app.world_mut());
        settle_cleanup_system(app.world_mut());

        let navigation = app.world().resource::<RouterNavigation>();
        let entry = &navigation.entries[0];
        assert!(matches!(
            entry.engine.request(second).unwrap().status(),
            RequestStatus::Succeeded
        ));
        assert!(entry.active.is_some());
        assert_eq!(entry.engine.history().len(), 1);
    }

    #[test]
    fn current_host_boundary_loss_with_missing_pickable_fail_closes_typed_zero_write() {
        let mut app = App::new();
        app.world_mut()
            .register_component_hooks::<Pickable>()
            .on_insert(|mut world, HookContext { entity, .. }| {
                let remove = {
                    let Some(mut drift) = world.get_resource_mut::<OnePickableRemoval>() else {
                        return;
                    };
                    if drift.removed {
                        false
                    } else if drift.remaining_before_target == 0 {
                        drift.removed = true;
                        true
                    } else {
                        drift.remaining_before_target -= 1;
                        false
                    }
                };
                if remove {
                    world.commands().entity(entity).remove::<Pickable>();
                }
            });
        app.add_plugins(BevyUiRouterPlugin::default());
        let router = install_atomic_router(&mut app, "stale-cleanup-parent");
        observe_authority_system(app.world_mut());
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        app.world_mut().insert_resource(OnePickableRemoval {
            remaining_before_target: 2,
            removed: false,
        });

        drive_navigation_system(app.world_mut());

        let (top_slot, top_slot_claim, frame_claims, gate_targets, host_entity) = {
            let navigation = app.world().resource::<RouterNavigation>();
            let entry = &navigation.entries[0];
            let failure = entry.pending_failed_branch.as_ref().unwrap();
            let frame = &failure.branch.frames()[0];
            (
                frame.top_slot(),
                frame.mount_binding().slot_claim().clone(),
                frame.claims().cloned().collect::<Vec<_>>(),
                frame.interactive_entities(),
                entry.host.as_ref().unwrap().host,
            )
        };
        let missing_pickable = frame_claims
            .iter()
            .find(|claim| app.world().get::<Pickable>(claim.entity()).is_none())
            .expect("fixture leaves one exact frame claim without Pickable")
            .entity();
        let focused = *gate_targets
            .first()
            .expect("failed frame retains one interactive gate target");
        let parent_manifest = app
            .world()
            .resource::<RouteEntityRegistry>()
            .record(top_slot)
            .unwrap()
            .manifest()
            .clone();
        let stale_parent_claim = app
            .world_mut()
            .resource_mut::<RouteEntityRegistry>()
            .compare_exchange_lifecycle(&top_slot_claim, EntityLifecycle::Quarantined)
            .unwrap();

        let manifest = {
            let navigation = app.world().resource::<RouterNavigation>();
            let entry = &navigation.entries[0];
            let failed = entry.pending_failed_branch.as_ref().unwrap();
            let manifest = FrozenCleanupManifest::from_failed_prepared_branch(
                app.world(),
                app.world().resource::<RouteEntityRegistry>(),
                &failed.branch,
            )
            .unwrap();
            assert_eq!(manifest.boundary_drifts().len(), 1);
            let drift = &manifest.boundary_drifts()[0];
            assert!(matches!(
                drift.source,
                FailedBoundarySource::FrameMountSlot { .. }
            ));
            assert_eq!(drift.expected.exact_claim(), &top_slot_claim);
            assert!(matches!(
                top_slot_claim.owner(),
                RegistryOwner::Committed(EntityOwner::HostTopSlot { .. })
            ));
            assert!(!failed_branch_owns_exact_boundary(&failed.branch, drift));
            assert!(current_host_owns_failed_boundary(
                entry.host.as_ref(),
                &top_slot_claim
            ));
            assert!(matches!(
                classify_failed_boundary(
                    drift.clone(),
                    &failed.branch,
                    entry.active.as_ref(),
                    entry.host.as_ref(),
                ),
                FailedBoundaryDisposition::CurrentAuthorityLoss(_)
            ));
            manifest
        };
        assert!(!manifest.boundary_drifts().is_empty());
        let registry_before = app
            .world()
            .resource::<RouteEntityRegistry>()
            .records()
            .to_vec();
        let mut relationship_entities = frame_claims
            .iter()
            .map(ExactRegistryClaim::entity)
            .chain([top_slot, host_entity])
            .collect::<Vec<_>>();
        relationship_entities.sort_by_key(|entity| entity.to_bits());
        relationship_entities.dedup();
        let relationships_before = relationship_entities
            .iter()
            .map(|entity| {
                (
                    *entity,
                    app.world().get::<ChildOf>(*entity).map(ChildOf::parent),
                    app.world()
                        .get::<Children>(*entity)
                        .map(|children| children.iter().copied().collect::<Vec<_>>())
                        .unwrap_or_default(),
                )
            })
            .collect::<Vec<_>>();
        *app.world_mut().resource_mut::<InputFocus>() = InputFocus::from_entity(focused);
        app.world_mut()
            .resource_mut::<RouteFocusState>()
            .set_focused(Some(focused));

        settle_cleanup_system(app.world_mut());

        let trace_len = {
            let navigation = app.world().resource::<RouterNavigation>();
            let entry = &navigation.entries[0];
            let residual = entry
                .authority_protocol_residual
                .as_ref()
                .expect("current authority loss plus missing capability is terminal");
            let expected_violation = AuthorityIsolationViolation {
                entity: missing_pickable,
                component: AuthorityIsolationComponent::Pickable,
                predicate: AuthorityIsolationPredicate::PresentForIgnoreWrite,
            };
            assert!(matches!(
                &residual.kind,
                AuthorityProtocolResidualKind::AuthorityIsolation(contradiction)
                    if contradiction.violations.contains(&expected_violation)
            ));
            assert!(
                residual
                    .isolation_contradiction
                    .as_ref()
                    .is_some_and(|contradiction| contradiction
                        .violations
                        .contains(&expected_violation))
            );
            assert!(residual.active.is_none());
            assert!(residual.failed.is_some());
            assert!(residual.host.is_some());
            assert!(entry.active.is_none());
            assert!(entry.pending_failed_branch.is_none());
            assert!(entry.host.is_none());
            assert!(entry.cleanup_ledger.is_none());
            assert!(matches!(
                entry.engine.request(request).unwrap().status(),
                RequestStatus::Failed(failure)
                    if failure.kind() == NavigationFailureKind::AuthorityLost
            ));
            entry.engine.request(request).unwrap().trace().len()
        };
        assert_eq!(app.world().resource::<InputFocus>().get(), None);
        assert_eq!(app.world().resource::<RouteFocusState>().focused(), None);
        let gate = app.world().resource::<RouterInputGate>();
        assert!(gate_targets.iter().all(|entity| {
            !gate.allows_pointer(*entity)
                && !gate.allows_keyboard(*entity)
                && !gate.allows_gamepad(*entity)
        }));
        assert_eq!(app.world().get::<Pickable>(missing_pickable), None);
        assert!(frame_claims.iter().all(|claim| {
            app.world()
                .get::<Visibility>(claim.entity())
                .is_none_or(|visibility| *visibility == Visibility::Hidden)
                && app
                    .world()
                    .get::<Pickable>(claim.entity())
                    .is_none_or(|pickable| *pickable == Pickable::IGNORE)
                && app
                    .world()
                    .get::<RouterInputBarrier>(claim.entity())
                    .is_none_or(RouterInputBarrier::denies_all)
        }));
        assert!(frame_claims.iter().all(|claim| {
            app.world().get_entity(claim.entity()).is_ok()
                && app
                    .world()
                    .resource::<RouteEntityRegistry>()
                    .record(claim.entity())
                    .is_some_and(|record| record.claim() == *claim)
        }));
        let parent_record = app
            .world()
            .resource::<RouteEntityRegistry>()
            .record(top_slot)
            .unwrap();
        assert_eq!(parent_record.claim(), stale_parent_claim);
        assert_eq!(parent_record.manifest(), &parent_manifest);
        assert!(parent_manifest.matches(app.world(), top_slot));
        assert_eq!(
            app.world().resource::<RouteEntityRegistry>().records(),
            registry_before
        );
        let relationships_after = relationship_entities
            .iter()
            .map(|entity| {
                (
                    *entity,
                    app.world().get::<ChildOf>(*entity).map(ChildOf::parent),
                    app.world()
                        .get::<Children>(*entity)
                        .map(|children| children.iter().copied().collect::<Vec<_>>())
                        .unwrap_or_default(),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(relationships_after, relationships_before);

        settle_cleanup_system(app.world_mut());

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert_eq!(
            entry.engine.request(request).unwrap().trace().len(),
            trace_len
        );
        assert!(entry.authority_protocol_residual.is_some());
        assert!(entry.cleanup_ledger.is_none());
    }

    #[test]
    fn observer_drift_retains_failure_owner_without_blocking_the_next_frame() {
        let mut app = App::new();
        app.world_mut()
            .register_component_hooks::<Pickable>()
            .on_insert(|mut world, HookContext { entity, .. }| {
                let remove = {
                    let Some(mut drift) = world.get_resource_mut::<PersistentOnePickableRemoval>()
                    else {
                        return;
                    };
                    if let Some(target) = drift.target {
                        target == entity
                    } else if drift.remaining_before_target == 0 {
                        drift.target = Some(entity);
                        true
                    } else {
                        drift.remaining_before_target -= 1;
                        false
                    }
                };
                if remove {
                    world.commands().entity(entity).remove::<Pickable>();
                }
            });
        app.add_plugins(BevyUiRouterPlugin::default());
        let router = install_atomic_router(&mut app, "residual-retry");
        observe_authority_system(app.world_mut());
        let first = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        app.world_mut()
            .insert_resource(PersistentOnePickableRemoval {
                remaining_before_target: 2,
                target: None,
            });

        drive_navigation_system(app.world_mut());

        assert!(matches!(
            app.world().resource::<RouterNavigation>().entries[0]
                .engine
                .request(first)
                .unwrap()
                .status(),
            RequestStatus::Running(_)
        ));
        assert!(
            app.world().resource::<RouterNavigation>().entries[0]
                .pending_failed_branch
                .is_some()
        );
        assert_eq!(
            app.world()
                .resource::<QuarantineLeaseTable>()
                .unresolved_residuals()
                .len(),
            0
        );

        settle_cleanup_system(app.world_mut());

        let navigation = app.world().resource::<RouterNavigation>();
        let entry = &navigation.entries[0];
        assert!(matches!(
            entry.engine.request(first).unwrap().status(),
            RequestStatus::Running(_)
        ));
        assert!(entry.pending_failed_branch.is_some());
        assert!(entry.cleanup_ledger.is_none());
        assert!(entry.active.is_none());
        assert_eq!(entry.engine.history().len(), 0);
        let retained = entry
            .pending_failed_branch
            .as_ref()
            .unwrap()
            .branch
            .frames()
            .iter()
            .flat_map(RealizedFrame::interactive_entities)
            .collect::<Vec<_>>();
        assert!(!retained.is_empty());
        assert!(retained.iter().all(|entity| {
            app.world().get::<Visibility>(*entity) == Some(&Visibility::Hidden)
                && app
                    .world()
                    .get::<RouterInputBarrier>(*entity)
                    .is_some_and(RouterInputBarrier::denies_all)
        }));
        assert_eq!(
            app.world()
                .resource::<QuarantineLeaseTable>()
                .unresolved_residuals()
                .len(),
            0
        );
        let retained_claims = entry
            .pending_failed_branch
            .as_ref()
            .unwrap()
            .branch
            .frames()
            .iter()
            .flat_map(|frame| frame.claims().cloned())
            .collect::<Vec<_>>();
        assert!(!retained_claims.is_empty());
        assert!(retained_claims.iter().all(|claim| {
            app.world()
                .resource::<RouteEntityRegistry>()
                .record(claim.entity())
                .is_some_and(|record| record.claim() == *claim)
        }));

        app.world_mut()
            .remove_resource::<PersistentOnePickableRemoval>();

        settle_cleanup_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        commit_presentation_system(app.world_mut());
        settle_cleanup_system(app.world_mut());
        {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            let request = entry.engine.request(first).unwrap();
            let RequestStatus::Failed(failure) = request.status() else {
                panic!("successful fallback must terminalize the original apply failure")
            };
            assert_eq!(
                failure.kind(),
                NavigationFailureKind::Apply,
                "{}",
                failure.message()
            );
            assert_eq!(entry.engine.history().len(), 0);
            assert!(entry.active.is_some());
            assert!(
                app.world()
                    .get_entity(entry.active.as_ref().unwrap().interactive_entities()[0])
                    .is_ok()
            );
        }

        let second = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(
                router,
                RouteLocation::new("/").with_fragment("after-residual"),
            )
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        commit_presentation_system(app.world_mut());
        settle_cleanup_system(app.world_mut());

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert!(matches!(
            entry.engine.request(second).unwrap().status(),
            RequestStatus::Succeeded
        ));
        assert!(entry.active.is_some());
        assert!(validate_active_frame(
            app.world(),
            entry.active.as_ref().unwrap().first_frame().unwrap()
        ));
    }
}
