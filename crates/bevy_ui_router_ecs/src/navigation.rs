use std::sync::atomic::{AtomicU64, Ordering};

use bevy_ecs::{prelude::World, resource::Resource};
use bevy_input_focus::InputFocus;
use bevy_ui_router_core::{
    integration::{
        CleanupManifest, EngineAction, FailureCleanupRequest, FrameMountTarget, IntegrationError,
        MountMutationArtifact, MountReceiptProof, MountRegistrationBeginError, MountRequest,
        ResolveMountOwner, RootFrameTask, RouterEngine, ViewFailureKind,
    },
    prelude::{MountRejectionKind, NavigationRequestId, RouteFrameVersion, RouteLocation},
};
use bevy_ui_router_view::{RouteViewRegistry, ViewBuildEffect, integration as view_integration};

use crate::{
    app_ext::{RouterHandle, ViewDriverStore},
    branch_transaction::{FailedPreparedBranch, PendingCommit, PreparedBranch, RealizedBranch},
    cleanup::{
        CleanupLedger, FrozenCleanupManifest, RouterCleanupObservation,
        failed_prepared_branch_roots_are_exact,
    },
    content::{ContentPlan, FrameContentWriter},
    driver::{EcsViewReportSession, ReportSeal, ViewBuildReport, ViewBuildReportKind},
    frame_local::{validate_committed_branch, validate_prepared_branch},
    host::{
        HostCandidateClassification, HostCandidateRevisionLatch, HostCandidateRevisionState,
        ProvisionalHostSettlement, ValidatedHostCandidate, activate_granted_top_slot,
        classify_host_candidate, retry_provisional_host_settlement,
        settle_rejected_granted_top_slot, stage_granted_top_slot,
        validated_host_candidate_is_current,
    },
    input::{
        RouteFocusState, RouterInputGate, observed_descendant_closure, prepare_focus_mutation,
        prepare_input_gate_mutation,
    },
    mount::{
        ChildSlotResidual, StagedChildSlot, confirm_child_slot, mounted_child_slot_is_isolated,
        residual_child_slot_is_isolated, stage_child_slot,
    },
    presentation::{
        DeferredRealizationFailure, RealizedFrame, frame_is_isolated_for_settlement,
        isolate_frame_for_settlement, prepare_presentation_mutation, realize_child_frame_deferred,
        realize_frame_deferred, validate_realized_frame,
    },
    registry::{EntityLifecycle, RouteEntityRegistry},
    snapshots::RouterSnapshots,
};

#[cfg(test)]
use crate::presentation::validate_active_frame;

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

struct PendingFailedBranch {
    terminal: PendingFailedBranchTerminal,
    branch: FailedPreparedBranch,
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
}

pub(crate) struct RouterEntry {
    pub(crate) handle: RouterHandle,
    pub(crate) engine: RouterEngine,
    views: RouteViewRegistry,
    pub(crate) host: Option<HostBinding>,
    host_settlement: Option<ProvisionalHostSettlement>,
    pending_commit: Option<PendingCommit>,
    pending_frame_drive: Option<PendingFrameDrive>,
    pending_mount_request: Option<MountRequest>,
    pending_mount_mutation_rejection: Option<PendingMountMutationRejection>,
    pending_failed_branch: Option<PendingFailedBranch>,
    pub(crate) active: Option<RealizedBranch>,
    pub(crate) cleanup_ledger: Option<CleanupLedger>,
    cleanup_observation: RouterCleanupObservation,
    pub(crate) requests: Vec<NavigationRequestId>,
}

impl RouterEntry {
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
            pending_mount_request: None,
            pending_mount_mutation_rejection: None,
            pending_failed_branch: None,
            active: None,
            cleanup_ledger: None,
            cleanup_observation: RouterCleanupObservation::default(),
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
                    if entry.host.is_some() {
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
                                entry.engine.confirm_host(grant);
                                entry.host = Some(HostBinding {
                                    version,
                                    slot_id,
                                    host,
                                    top_slot,
                                    top_slot_claim,
                                    candidate,
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
            let (Some(parent), Some(mounted)) = (branch.last_frame(), branch.child_slots().last())
            else {
                return Err(DriverReportApplicationError::Stale);
            };
            realize_child_frame_deferred(
                world,
                parent,
                mounted,
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

fn retry_pending_mount_mutation_rejection(
    world: &mut World,
    entry: &mut RouterEntry,
    pending_rejection: PendingMountMutationRejection,
) -> Option<PendingMountMutationRejection> {
    let PendingMountMutationRejection {
        pending,
        artifact,
        payload,
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
                branch: failed_prepared_branch(pending, residual),
            });
            None
        }
        Err(error) => Some(PendingMountMutationRejection {
            pending,
            artifact: error.into_owner(),
            payload,
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

fn failed_branch_focus_closure(
    world: &World,
    branch: &FailedPreparedBranch,
) -> Vec<bevy_ecs::prelude::Entity> {
    let roots = branch
        .frames()
        .iter()
        .chain(branch.detached_frames())
        .flat_map(RealizedFrame::claims)
        .map(crate::registry::ExactRegistryClaim::entity)
        .chain(branch.child_slots().iter().map(|slot| slot.slot()))
        .chain(
            branch
                .mount_residuals()
                .iter()
                .map(crate::mount::ChildSlotResidual::entity),
        )
        .chain(
            branch
                .rejected_child_slots()
                .iter()
                .map(|slot| slot.slot().slot()),
        )
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

fn isolate_failed_prepared_branch(world: &mut World, branch: &FailedPreparedBranch) -> bool {
    if !failed_prepared_branch_roots_are_exact(world, branch) {
        return false;
    }
    let focus_closure = failed_branch_focus_closure(world, branch);
    release_focus_for_closure(world, &focus_closure);
    let frames_are_isolated = branch
        .frames()
        .iter()
        .chain(branch.detached_frames())
        .all(|frame| isolate_frame_for_settlement(world, frame));
    let child_slots_are_isolated = branch
        .child_slots()
        .iter()
        .all(|slot| mounted_child_slot_is_isolated(world, slot))
        && branch
            .rejected_child_slots()
            .iter()
            .all(|slot| mounted_child_slot_is_isolated(world, slot.slot()));
    let residuals_are_isolated = branch
        .mount_residuals()
        .iter()
        .all(|slot| residual_child_slot_is_isolated(world, slot));
    release_focus_for_closure(world, &focus_closure);
    frames_are_isolated
        && child_slots_are_isolated
        && residuals_are_isolated
        && branch
            .frames()
            .iter()
            .chain(branch.detached_frames())
            .all(|frame| frame_is_isolated_for_settlement(world, frame))
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
                                        entry.pending_failed_branch = Some(PendingFailedBranch {
                                            terminal: PendingFailedBranchTerminal::Request(request),
                                            branch: failed_prepared_branch(pending, None),
                                        });
                                    }
                                    continue;
                                }
                                let realized = apply_applied_driver_report_to_branch(
                                    world,
                                    &mut entry.engine,
                                    host.candidate.clone(),
                                    host.top_slot,
                                    host.top_slot_claim.clone(),
                                    (!pending.branch.frames().is_empty()).then_some(&pending.branch),
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
                                let Some(mut pending) = entry.pending_commit.take() else {
                                    continue;
                                };
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
                                let (parent_is_exact, outlet_exists, outlet) = {
                                    let request = entry
                                        .pending_mount_request
                                        .as_ref()
                                        .expect("validated mount request remains held");
                                    let parent = pending.branch.last_frame();
                                    (
                                        parent.is_some_and(|parent| {
                                            parent.frame() == request.parent()
                                                && validate_realized_frame(world, parent)
                                        }),
                                        parent.is_some_and(|parent| {
                                            parent.outlet(request.outlet()).is_some()
                                        }),
                                        request.outlet().clone(),
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
                                let parent = pending
                                    .branch
                                    .last_frame()
                                    .expect("validated mount request retains its parent frame");
                                let staged = match stage_child_slot(world, parent, &outlet, &grant) {
                                    Ok(staged) => staged,
                                    Err(failure) => {
                                        let residual = failure.into_residual();
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
                                                        branch: failed_prepared_branch(
                                                            pending, residual,
                                                        ),
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
                                        match pending.branch.push_child_slot(mounted) {
                                            Ok(()) => entry.pending_commit = Some(pending),
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
                                        let terminal = if let Some(request) = cleanup {
                                            PendingFailedBranchTerminal::Request(request)
                                        } else if let Some(error) = begin {
                                            PendingFailedBranchTerminal::RegistrationBegin(error)
                                        } else {
                                            unreachable!("confirmation failure retains one terminal")
                                        };
                                        entry.pending_failed_branch = Some(PendingFailedBranch {
                                            terminal,
                                            branch: failed_prepared_branch(
                                                pending,
                                                Some(residual),
                                            ),
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

pub(crate) fn commit_presentation_system(world: &mut World) {
    world.resource_scope(
        |world, mut navigation: bevy_ecs::change_detection::Mut<RouterNavigation>| {
            for entry in &mut navigation.entries {
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
                let Some(host) = entry.host.as_ref() else {
                    entry.pending_commit = Some(pending);
                    continue;
                };
                if world.get_entity(host.host).is_err()
                    || !validated_host_candidate_is_current(world, &host.candidate)
                {
                    entry.pending_commit = Some(pending);
                    continue;
                }
                let replaces_active = !pending.branch.frames().is_empty();
                if replaces_active
                    && validate_prepared_branch(world, pending.branch.as_realized()).is_err()
                {
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
                    let _ = isolate_failed_prepared_branch(world, &failed.branch);
                    entry.pending_failed_branch = Some(failed);
                    continue;
                }
                let active_descendants = match entry.active.as_ref() {
                    Some(branch) => match validate_committed_branch(world, branch, replaces_active) {
                        Ok(descendants) => descendants,
                        Err(_) => {
                            entry.pending_commit = Some(pending);
                            continue;
                        }
                    },
                    None => Vec::new(),
                };

                let mut entering = replaces_active.then(|| {
                    std::mem::replace(&mut pending.branch, PreparedBranch::empty()).into_realized()
                });
                let mut leaving = if entering.is_some() {
                    entry.active.take()
                } else {
                    None
                };
                let logical_roots = leaving
                    .as_ref()
                    .map(logical_cleanup_roots)
                    .unwrap_or_default();
                let gate_before = leaving
                    .as_ref()
                    .or(entry.active.as_ref())
                    .map(RealizedBranch::interactive_entities)
                    .unwrap_or_default();
                let gate_after = entering.as_ref().map_or_else(
                    || {
                        entry
                            .active
                            .as_ref()
                            .map(RealizedBranch::interactive_entities)
                            .unwrap_or_default()
                    },
                    RealizedBranch::interactive_entities,
                );
                let focus_leaving = focus_targets_for_commit(replaces_active, active_descendants);
                let mut targets = Vec::new();
                let mut transitions = Vec::new();
                if let Some(branch) = leaving.as_ref() {
                    targets.extend(branch.leaving_mutation_targets(
                        EntityLifecycle::Active,
                        EntityLifecycle::Leaving,
                    ));
                    transitions.extend(
                        branch.lifecycle_transitions(EntityLifecycle::Leaving, true),
                    );
                }
                let leaving_count = transitions.len();
                if let Some(branch) = entering.as_ref() {
                    targets.extend(branch.entering_mutation_targets(
                        EntityLifecycle::PreparedInert,
                        EntityLifecycle::Active,
                    ));
                    transitions.extend(
                        branch.lifecycle_transitions(EntityLifecycle::Active, false),
                    );
                }
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
                                                let frozen_manifest = match leaving.as_ref() {
                                                    Some(branch) => match FrozenCleanupManifest::from_leaving_branch(
                                                        world,
                                                        &registry,
                                                        branch,
                                                        &resulting_claims[..leaving_count],
                                                        entering.as_ref(),
                                                    ) {
                                                        Some(manifest) => manifest,
                                                        None => return None,
                                                    },
                                                    None => FrozenCleanupManifest::empty(),
                                                };
                                                let manifest_digest = frozen_manifest.digest();
                                                let obligation_count = frozen_manifest.len();
                                                let manifest = CleanupManifest::new(
                                                    logical_roots.clone(),
                                                    manifest_digest,
                                                    obligation_count,
                                                );
                                                let reserved_ledger =
                                                    CleanupLedger::reserve(frozen_manifest)?;
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
                                                let leaving_rebind = match leaving.as_ref() {
                                                    Some(branch) => Some(branch.prepare_claim_rebind(
                                                        &resulting_claims[..leaving_count],
                                                        EntityLifecycle::Active,
                                                        EntityLifecycle::Leaving,
                                                    )?),
                                                    None => None,
                                                };
                                                let entering_rebind = match entering.as_ref() {
                                                    Some(branch) => Some(branch.prepare_claim_rebind(
                                                        &resulting_claims[leaving_count..],
                                                        EntityLifecycle::PreparedInert,
                                                        EntityLifecycle::Active,
                                                    )?),
                                                    None => None,
                                                };
                                                let root_rebase = match (
                                                    leaving.as_ref(),
                                                    entering.as_ref(),
                                                ) {
                                                    (Some(leaving), Some(entering)) => Some(
                                                        entering.prepare_root_rebase(leaving)?,
                                                    ),
                                                    (None, _) => None,
                                                    (Some(_), None) => return None,
                                                };
                                                let presentation_plan =
                                                    prepare_presentation_mutation(world, &targets)?
                                                        .seal_commit(leaving_count)?;
                                                if pending.ticket.is_none() {
                                                    let task = pending.final_task.take()?;
                                                    pending.ticket =
                                                        entry.engine.report_applied(task).ok();
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
                                                if let (Some(branch), Some(rebind)) =
                                                    (leaving.as_mut(), leaving_rebind)
                                                {
                                                    branch.apply_claim_rebind(rebind);
                                                }
                                                if let (Some(branch), Some(rebind)) =
                                                    (entering.as_mut(), entering_rebind)
                                                {
                                                    branch.apply_claim_rebind(rebind);
                                                }
                                                if let (Some(branch), Some(rebase)) =
                                                    (entering.as_mut(), root_rebase)
                                                {
                                                    branch.apply_root_rebase(rebase);
                                                }
                                                entry.engine.publish(prepared, || {
                                                    presentation_plan.apply_staged(
                                                        |_| focus_plan.apply(),
                                                        |_| registry_plan.apply(&mut registry),
                                                        |_| gate_plan.apply(),
                                                    );
                                                });
                                                Some(cleanup_ledger)
                                            },
                                        )
                                    },
                                )
                            },
                        )
                    },
                );
                let Some(cleanup_ledger) = committed else {
                    if let Some(frame) = leaving {
                        entry.active = Some(frame);
                    }
                    if let Some(branch) = entering {
                        pending.branch = PreparedBranch::from_realized(branch);
                    }
                    if pending.ticket.is_some() || pending.final_task.is_some() {
                        entry.pending_commit = Some(pending);
                    }
                    continue;
                };
                if let Some(branch) = entering {
                    entry.active = Some(branch);
                }
                entry.cleanup_ledger = Some(CleanupLedger::activate_leaving_branch(
                    cleanup_ledger,
                    leaving,
                ));
            }
        },
    );
    refresh_authority_audits(world);
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
                    let Some(manifest) = manifest else {
                        entry.pending_failed_branch = Some(failed);
                        continue;
                    };
                    if !manifest.preflight(world)
                        || !isolate_failed_prepared_branch(world, &failed.branch)
                        || !manifest.preflight(world)
                    {
                        entry.pending_failed_branch = Some(failed);
                        continue;
                    }
                    let manifest_digest = manifest.digest();
                    let obligation_count = manifest.len();
                    let Some(reserved) = CleanupLedger::reserve(manifest) else {
                        entry.pending_failed_branch = Some(failed);
                        continue;
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
    use crate::{
        app_ext::{AppRouterExt, RouterConfig, RouterHostRegistration},
        cleanup::{CleanupPostcheckDrift, CleanupPostcheckPhase},
        components::{
            ChildSlotAuthorityProbe, ChildSlotBarrierProbe, ChildSlotLifecycle,
            ProvisionalPostWriteProbe, ProvisionalSlotLifecycle, RouteOwnershipSeal,
            RouterInputBarrier,
        },
        content::{ContentColor, SolidRect},
        driver::{EcsViewDriver, ViewBuildReport},
        host::HostCandidateRevision,
        input::{RouteFocusState, RouterInputGate},
        plugin::{BevyUiRouterPlugin, RouterScheduleSet},
        presentation::{
            FrameOutput, TestCommitMutation, arm_test_commit_mutation_trace,
            take_test_commit_mutation_trace,
        },
        registry::{
            ComponentManifest, EntityOwner, ProvisionalOwner, ProvisionalTopSlotOwner,
            QuarantineLeaseTable, RegistryOwner, fail_next_verified_preparation_for_test,
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
        FrameOrigin, NavigationFailureKind, NavigationPhase, NavigationTraceEventKind,
        RequestStatus, RouteNodeOptions, RoutePattern, RouteTree, RouteViewKey,
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
        let pending = app.world().resource::<RouterNavigation>().entries[0]
            .pending_commit
            .as_ref()
            .unwrap();
        assert_eq!(pending.branch.frames().len(), 2);
        assert_eq!(pending.branch.child_slots().len(), 1);
        assert!(pending.final_task.is_some());
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
        let router = install_nested_router(&mut app, observed, true);
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
    fn nested_commit_schedule_probe_is_old_old_then_new_new() {
        let source = include_str!("navigation.rs");
        assert!(!source.contains(concat!(
            "old_shells.into_iter().rev()",
            ".map(NestedCommitOrder::Leaving)"
        )));
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
                    physical: NestedBranchSide::New,
                    gate: NestedBranchSide::New,
                    registry: NestedBranchSide::New,
                    ecs: NestedBranchSide::New,
                    core: NestedBranchSide::New,
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
                NestedCommitOrder::Leaving(old_shells[0]),
                NestedCommitOrder::Entering(new_shells[0]),
                NestedCommitOrder::Entering(new_shells[1]),
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
                old.obligation_count(),
                old.frames()
                    .iter()
                    .map(|frame| frame.frame().clone())
                    .collect::<Vec<_>>(),
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
            assert_eq!(ledger.leaving_frames().len(), 2);
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
        assert_eq!(
            cleanup_units,
            vec![
                NestedCleanupUnit::ChildFrame,
                NestedCleanupUnit::ChildSlot,
                NestedCleanupUnit::ParentFrame,
            ]
        );
        assert!(app.world().get_entity(child_slot).is_err());

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
        for drift_unit in [
            NestedCleanupUnit::ChildFrame,
            NestedCleanupUnit::ChildSlot,
            NestedCleanupUnit::ParentFrame,
        ] {
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
    fn nested_replacement_switches_the_whole_branch_and_releases_child_focus() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let router = install_nested_router(&mut app, observed, true);
        app.update();
        commit_initial_nested_branch(&mut app, router);
        let (old_claims, old_slots, old_interactive) = {
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
                old.child_slots()
                    .iter()
                    .map(|slot| slot.slot())
                    .collect::<Vec<_>>(),
                old.interactive_entities(),
            )
        };
        let old_focus = *old_interactive.last().unwrap();
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
        assert_eq!(
            active
                .claims()
                .map(|claim| claim.entity())
                .collect::<Vec<_>>(),
            new_claims
                .iter()
                .map(|claim| claim.entity())
                .collect::<Vec<_>>()
        );
        assert_eq!(entry.engine.history().current(), Some(&replacement));
        assert!(entry.cleanup_ledger.is_some());
        assert!(old_claims.iter().all(|claim| {
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
            app.world().get::<ChildSlotLifecycle>(*slot) == Some(&ChildSlotLifecycle::Leaving)
                && app.world().get::<Visibility>(*slot) == Some(&Visibility::Hidden)
                && app.world().get::<Pickable>(*slot) == Some(&Pickable::IGNORE)
        }));
        let gate = app.world().resource::<RouterInputGate>();
        assert!(
            old_interactive
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
    fn nested_fragment_push_replaces_the_whole_branch() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let router = install_nested_router(&mut app, observed, true);
        app.update();
        commit_initial_nested_branch(&mut app, router);
        let (old_frames, old_slots) = {
            let navigation = app.world().resource::<RouterNavigation>();
            let old = navigation.entries[0].active.as_ref().unwrap();
            (
                old.frames()
                    .iter()
                    .map(RealizedFrame::shell)
                    .collect::<Vec<_>>(),
                old.child_slots()
                    .iter()
                    .map(|slot| slot.slot())
                    .collect::<Vec<_>>(),
            )
        };
        let replacement = RouteLocation::new("/items/1").with_fragment("details");

        prepare_nested_branch_for_commit(&mut app, router, replacement.clone());
        commit_presentation_system(app.world_mut());

        let navigation = app.world().resource::<RouterNavigation>();
        let entry = &navigation.entries[0];
        let active = entry.active.as_ref().unwrap();
        assert_ne!(
            active
                .frames()
                .iter()
                .map(RealizedFrame::shell)
                .collect::<Vec<_>>(),
            old_frames
        );
        assert_ne!(
            active
                .child_slots()
                .iter()
                .map(|slot| slot.slot())
                .collect::<Vec<_>>(),
            old_slots
        );
        assert_eq!(entry.engine.history().current(), Some(&replacement));
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
            .frames()[1]
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
            let EngineAction::FrameTask(task) = entry.engine.action().unwrap() else {
                panic!("child task must be issued")
            };
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
            let EngineAction::FrameTask(task) = entry.engine.action().unwrap() else {
                panic!("child task must be issued")
            };
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
    fn stale_external_parent_is_world_zero_and_preserves_foreign_descendant_focus() {
        let mut app = App::new();
        let (request, _router) = prepare_exact_child_stale_failure(&mut app);
        let (frame_shell, top_slot_claim) = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            (
                entry
                    .pending_failed_branch
                    .as_ref()
                    .unwrap()
                    .branch
                    .frames()[0]
                    .shell(),
                entry.host.as_ref().unwrap().top_slot_claim.clone(),
            )
        };
        let foreign = app.world_mut().spawn(ChildOf(frame_shell)).id();
        *app.world_mut().resource_mut::<InputFocus>() = InputFocus::from_entity(foreign);
        app.world_mut()
            .resource_mut::<RouteFocusState>()
            .set_focused(Some(foreign));
        app.world_mut()
            .resource_mut::<RouteEntityRegistry>()
            .compare_exchange_lifecycle(&top_slot_claim, EntityLifecycle::Quarantined)
            .unwrap();

        settle_cleanup_system(app.world_mut());

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert!(entry.pending_failed_branch.is_some());
        assert!(entry.cleanup_ledger.is_none());
        assert!(matches!(
            entry.engine.request(request).unwrap().status(),
            RequestStatus::Running(_)
        ));
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(foreign));
        assert_eq!(
            app.world().resource::<RouteFocusState>().focused(),
            Some(foreign)
        );
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
    pub(crate) fn failed_branch_relationship_cycle_is_world_zero_and_retains_focus() {
        let mut app = App::new();
        let (request, _router) = prepare_exact_child_stale_failure(&mut app);
        let (cycle_child, cycle_parent, claims) = {
            let failed = app.world().resource::<RouterNavigation>().entries[0]
                .pending_failed_branch
                .as_ref()
                .unwrap();
            let frame = &failed.branch.frames()[0];
            let cycle_child = frame.outlets()[0].anchor();
            let cycle_parent = app
                .world()
                .get::<ChildOf>(cycle_child)
                .map(ChildOf::parent)
                .unwrap();
            (
                cycle_child,
                cycle_parent,
                frame.claims().cloned().collect::<Vec<_>>(),
            )
        };
        app.world_mut()
            .entity_mut(cycle_parent)
            .insert(ChildOf(cycle_child));
        app.world_mut().flush();
        *app.world_mut().resource_mut::<InputFocus>() = InputFocus::from_entity(cycle_child);
        app.world_mut()
            .resource_mut::<RouteFocusState>()
            .set_focused(Some(cycle_child));
        let registry_before = app
            .world()
            .resource::<RouteEntityRegistry>()
            .records()
            .to_vec();
        let values_before = claims
            .iter()
            .map(|claim| {
                (
                    claim.entity(),
                    *app.world().get::<Visibility>(claim.entity()).unwrap(),
                    *app.world().get::<Pickable>(claim.entity()).unwrap(),
                    *app.world()
                        .get::<RouterInputBarrier>(claim.entity())
                        .unwrap(),
                )
            })
            .collect::<Vec<_>>();

        settle_cleanup_system(app.world_mut());

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert!(entry.pending_failed_branch.is_some());
        assert!(entry.cleanup_ledger.is_none());
        assert!(matches!(
            entry.engine.request(request).unwrap().status(),
            RequestStatus::Running(_)
        ));
        assert_eq!(
            app.world().resource::<RouteEntityRegistry>().records(),
            registry_before
        );
        assert_eq!(
            app.world().resource::<InputFocus>().get(),
            Some(cycle_child)
        );
        assert_eq!(
            app.world().resource::<RouteFocusState>().focused(),
            Some(cycle_child)
        );
        assert_eq!(
            claims
                .iter()
                .map(|claim| {
                    (
                        claim.entity(),
                        *app.world().get::<Visibility>(claim.entity()).unwrap(),
                        *app.world().get::<Pickable>(claim.entity()).unwrap(),
                        *app.world()
                            .get::<RouterInputBarrier>(claim.entity())
                            .unwrap(),
                    )
                })
                .collect::<Vec<_>>(),
            values_before
        );
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
                    TestCommitMutation::Leaving(_) | TestCommitMutation::Entering(_) => None,
                }),
        );
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

        for _ in 0..4 {
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
    fn leaving_foreign_descendant_does_not_block_root_commit_and_is_quarantined() {
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
    fn leaving_foreign_component_is_terminal_and_quarantined_without_hooks() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        app.init_resource::<ForeignComponentHooks>();
        app.world_mut()
            .register_component_hooks::<FinalPreflightForeignComponent>()
            .on_remove(|mut world, HookContext { .. }| {
                world.resource_mut::<ForeignComponentHooks>().removed += 1;
            })
            .on_despawn(|mut world, HookContext { .. }| {
                world.resource_mut::<ForeignComponentHooks>().despawned += 1;
            });
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

    fn nested_leaving_child_slot_boundary_drift_commits(
        drift: ChildSlotBoundaryDrift,
        fragment: &'static str,
    ) {
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
            .insert(NestedAppHostSentinel(0xA0));
        let sibling = app.world_mut().spawn(NestedAppHostSentinel(0xA1)).id();
        app.world_mut().entity_mut(host).add_child(sibling);
        app.update();
        commit_initial_nested_branch(&mut app, router);

        let (leaving_slot, leaving_shell) = {
            let active = app.world().resource::<RouterNavigation>().entries[0]
                .active
                .as_ref()
                .unwrap();
            (active.child_slots()[0].slot(), active.frames()[0].shell())
        };
        let request = prepare_nested_branch_for_commit(
            &mut app,
            router,
            RouteLocation::new("/items/2").with_fragment(fragment),
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
        assert_ne!(
            entry.active.as_ref().unwrap().frames()[0].shell(),
            leaving_shell
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
                .any(|event| { event.cleanup_disposition() == Some(expected) })
        );
        assert!(
            app.world()
                .resource::<QuarantineLeaseTable>()
                .record(leaving_slot)
                .is_some(),
            "the leaving child-slot unit must transfer to foreground quarantine"
        );
        assert_child_slot_boundary_drift_remains_foreign(app.world(), leaving_slot, foreign, drift);
        assert_eq!(nested_protected_fingerprint(app.world(), host), host_before);
        assert_eq!(
            nested_protected_fingerprint(app.world(), sibling),
            sibling_before
        );
    }

    #[test]
    fn nested_leaving_child_slot_foreign_component_commits_and_quarantines_slot() {
        nested_leaving_child_slot_boundary_drift_commits(
            ChildSlotBoundaryDrift::ForeignComponent,
            "slot-component",
        );
    }

    #[test]
    fn nested_leaving_child_slot_foreign_descendant_commits_and_quarantines_slot() {
        nested_leaving_child_slot_boundary_drift_commits(
            ChildSlotBoundaryDrift::ForeignDescendant,
            "slot-descendant",
        );
    }

    fn nested_entering_child_slot_boundary_drift_is_rejected(
        drift: ChildSlotBoundaryDrift,
        fragment: &'static str,
    ) {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let router = install_nested_router(&mut app, observed, true);
        app.update();
        commit_initial_nested_branch(&mut app, router);
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
        let request = prepare_nested_branch_for_commit(
            &mut app,
            router,
            RouteLocation::new("/items/2").with_fragment(fragment),
        );
        let entering_slot = app.world().resource::<RouterNavigation>().entries[0]
            .pending_commit
            .as_ref()
            .unwrap()
            .branch
            .child_slots()[0]
            .slot();
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
        let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let router = install_nested_router(&mut app, observed, true);
        app.update();
        commit_initial_nested_branch(&mut app, router);
        let leaving_slot = app.world().resource::<RouterNavigation>().entries[0]
            .active
            .as_ref()
            .unwrap()
            .child_slots()[0]
            .slot();
        let request = prepare_nested_branch_for_commit(
            &mut app,
            router,
            RouteLocation::new("/items/2").with_fragment("retry-transfer"),
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

        commit_presentation_system(app.world_mut());
        settle_cleanup_system(app.world_mut());
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
    fn sealed_commit_tail_contains_no_fallible_cleanup_binding_branch() {
        let source = include_str!("navigation.rs");
        let commit = source
            .split_once("pub(crate) fn commit_presentation_system")
            .unwrap()
            .1
            .split_once("pub(crate) fn settle_cleanup_system")
            .unwrap()
            .0;

        assert!(!commit.contains("cleanup_batch_matches"));
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
        app.world_mut()
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
        assert!(
            app.world().resource::<RouterNavigation>().entries[0]
                .pending_commit
                .is_some()
        );
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
            let RequestStatus::Failed(failure) = request.status() else {
                panic!("fallback publication must retain the original build failure")
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
    fn stale_cleanup_mount_parent_claim_is_zero_write_and_retains_the_owner() {
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

        let (top_slot, top_slot_claim, frame_claims) = {
            let navigation = app.world().resource::<RouterNavigation>();
            let failure = navigation.entries[0]
                .pending_failed_branch
                .as_ref()
                .unwrap();
            let frame = &failure.branch.frames()[0];
            (
                frame.top_slot(),
                frame.mount_binding().slot_claim().clone(),
                frame.claims().cloned().collect::<Vec<_>>(),
            )
        };
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

        settle_cleanup_system(app.world_mut());

        let navigation = app.world().resource::<RouterNavigation>();
        let entry = &navigation.entries[0];
        let failed = entry.pending_failed_branch.as_ref().unwrap();
        assert_eq!(failed.branch.frames().len(), 1);
        assert!(entry.cleanup_ledger.is_none());
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
        assert!(matches!(
            entry.engine.request(request).unwrap().status(),
            RequestStatus::Running(_)
        ));
        assert!(entry.active.is_none());
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
