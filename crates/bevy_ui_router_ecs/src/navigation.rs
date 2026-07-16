use std::sync::atomic::{AtomicU64, Ordering};

use bevy_ecs::{prelude::World, resource::Resource};
use bevy_input_focus::InputFocus;
use bevy_ui_router_core::{
    integration::{
        CleanupManifest, CommitTicket, EngineAction, IntegrationError, MountRequest, RootFrameTask,
        RouterEngine, ViewFailureKind,
    },
    prelude::{NavigationRequestId, RouteLocation},
};
use bevy_ui_router_view::{RouteViewRegistry, integration as view_integration};

use crate::{
    app_ext::{RouterHandle, ViewDriverStore},
    cleanup::{CleanupLedger, FrozenCleanupManifest, RouterCleanupObservation},
    content::{ContentPlan, FrameContentWriter},
    driver::{EcsViewReportSession, ReportSeal, ViewBuildReport, ViewBuildReportKind},
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
    presentation::{
        DeferredRealizationFailure, RealizedFrame, isolate_frame_for_settlement,
        prepare_presentation_mutation, realize_frame_deferred, validate_active_frame,
        validate_leaving_frame, validate_realized_frame,
    },
    registry::{EntityLifecycle, RouteEntityRegistry},
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
}

struct PendingCommit {
    ticket: Option<CommitTicket>,
    task: Option<RootFrameTask>,
    realized: Option<RealizedFrame>,
}

struct PendingFailureCleanup {
    task: RootFrameTask,
    failure: ViewFailureKind,
    message: &'static str,
    state: FailureCleanupState,
}

enum FailureCleanupState {
    Unisolated(RealizedFrame),
    Unprepared(RealizedFrame),
    Prepared(CleanupLedger),
}

pub(crate) struct RouterEntry {
    pub(crate) handle: RouterHandle,
    pub(crate) engine: RouterEngine,
    views: RouteViewRegistry,
    pub(crate) host: Option<HostBinding>,
    host_settlement: Option<ProvisionalHostSettlement>,
    pending_commit: Option<PendingCommit>,
    pending_mount_request: Option<MountRequest>,
    pub(crate) active: Option<RealizedFrame>,
    pub(crate) cleanup_ledger: Option<CleanupLedger>,
    failure_cleanup: Option<PendingFailureCleanup>,
    cleanup_observation: RouterCleanupObservation,
    pub(crate) requests: Vec<NavigationRequestId>,
}

impl RouterEntry {
    pub(crate) fn cleanup_summary(&self) -> bevy_ui_router_core::CleanupSummary {
        self.cleanup_observation.summary(
            self.cleanup_ledger.is_some()
                || self.failure_cleanup.is_some()
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
            .and_then(|pending| pending.realized.as_ref())
    }

    #[cfg(feature = "test-support")]
    pub(crate) fn leaving_frame(&self) -> Option<&RealizedFrame> {
        self.cleanup_ledger
            .as_ref()
            .and_then(CleanupLedger::leaving)
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
            pending_mount_request: None,
            active: None,
            cleanup_ledger: None,
            failure_cleanup: None,
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
    if report.kind() != ViewBuildReportKind::Applied
        || report.identity() != &task.report_identity()
        || engine.validate_report_identity(task).is_err()
    {
        return Err(DriverReportApplicationError::Stale);
    }

    realize_frame_deferred(
        world,
        host_candidate,
        top_slot,
        top_slot_claim,
        task.frame().clone(),
        task.view_request_sequence(),
        plan.accept(),
    )
    .map_err(DriverReportApplicationError::Realization)
}

pub(crate) fn drive_navigation_system(world: &mut World) {
    world.resource_scope(
        |world, mut navigation: bevy_ecs::change_detection::Mut<RouterNavigation>| {
            world.resource_scope(
                |world, mut drivers: bevy_ecs::change_detection::Mut<ViewDriverStore>| {
                    for entry in &mut navigation.entries {
                        if entry.pending_commit.is_some() || entry.failure_cleanup.is_some() {
                            continue;
                        }
                        let pending_mount_request = match &mut entry.pending_mount_request {
                            vacant @ None => vacant,
                            Some(_) => continue,
                        };
                        let Some(action) = entry.engine.action() else {
                            continue;
                        };
                        match action {
                            EngineAction::Commit(ticket) => {
                                entry.pending_commit = Some(PendingCommit {
                                    ticket: Some(ticket),
                                    task: None,
                                    realized: None,
                                });
                            }
                            EngineAction::FrameTask(mut task) => {
                                let Some(host) = entry.host.as_ref() else {
                                    let _ = entry.engine.report_failed(
                                        task,
                                        ViewFailureKind::Apply,
                                        "router host unavailable during frame apply",
                                    );
                                    continue;
                                };
                                if !validated_host_candidate_is_current(world, &host.candidate) {
                                    let _ = entry.engine.report_failed(
                                        task,
                                        ViewFailureKind::Apply,
                                        "router host authority changed before frame realization",
                                    );
                                    continue;
                                }
                                let Some(driver_key) =
                                    view_integration::driver_key(&entry.views, task.view())
                                        .cloned()
                                else {
                                    let _ = entry.engine.report_failed(
                                        task,
                                        ViewFailureKind::Build,
                                        "route view binding unavailable",
                                    );
                                    continue;
                                };
                                let Some(driver) = drivers.get_mut(&driver_key) else {
                                    let _ = entry.engine.report_failed(
                                        task,
                                        ViewFailureKind::Build,
                                        "route view driver unavailable",
                                    );
                                    continue;
                                };
                                let effect = view_integration::build_effect(
                                    task.view().clone(),
                                    task.take_input(),
                                    task.view_request_sequence(),
                                );
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
                                    continue;
                                }
                                if report.kind() != ViewBuildReportKind::Applied {
                                    if entry.engine.validate_report_identity(&task).is_err() {
                                        continue;
                                    }
                                    let failure = match report.kind() {
                                        ViewBuildReportKind::BuildFailed => ViewFailureKind::Build,
                                        ViewBuildReportKind::ApplyFailed => ViewFailureKind::Apply,
                                        ViewBuildReportKind::Applied => unreachable!(),
                                    };
                                    let _ = entry.engine.report_failed(
                                        task,
                                        failure,
                                        "route view driver rejected the frame",
                                    );
                                    continue;
                                }
                                let realized = apply_applied_driver_report(
                                    world,
                                    &mut entry.engine,
                                    host.candidate.clone(),
                                    host.top_slot,
                                    host.top_slot_claim.clone(),
                                    &task,
                                    &report,
                                    plan,
                                );
                                let realized = match realized {
                                    Ok(realized) => realized,
                                    Err(DriverReportApplicationError::Stale) => continue,
                                    Err(DriverReportApplicationError::Realization(failure)) => {
                                        let (_error, residual) = failure.into_parts();
                                        if let Some(residual) = residual {
                                            let state = match CleanupLedger::for_failed_realization(
                                                world, residual,
                                            ) {
                                                Ok(ledger) => FailureCleanupState::Prepared(ledger),
                                                Err(residual) => {
                                                    FailureCleanupState::Unprepared(residual)
                                                }
                                            };
                                            entry.failure_cleanup = Some(PendingFailureCleanup {
                                                task,
                                                failure: ViewFailureKind::Apply,
                                                message: "frame realization rejected",
                                                state,
                                            });
                                        } else {
                                            let _ = entry.engine.report_failed(
                                                task,
                                                ViewFailureKind::Apply,
                                                "frame realization rejected",
                                            );
                                        }
                                        continue;
                                    }
                                };
                                entry.pending_commit = Some(PendingCommit {
                                    ticket: None,
                                    task: Some(task),
                                    realized: Some(realized),
                                });
                            }
                            EngineAction::MountRequest(request) => {
                                // The vacant-slot borrow is acquired before taking the sealed
                                // action, so an existing request fails closed without either
                                // capability being removed, overwritten, or discarded.
                                *pending_mount_request = Some(request);
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
                if pending
                    .realized
                    .as_ref()
                    .is_some_and(|frame| !validate_realized_frame(world, frame))
                {
                    let frame = pending
                        .realized
                        .take()
                        .expect("the invalid pending preflight identified its realized frame");
                    let Some(task) = pending.task.take() else {
                        entry.pending_commit = Some(pending);
                        continue;
                    };
                    let state = if isolate_frame_for_settlement(world, &frame) {
                        match CleanupLedger::for_failed_realization(world, frame) {
                            Ok(ledger) => FailureCleanupState::Prepared(ledger),
                            Err(frame) => FailureCleanupState::Unprepared(frame),
                        }
                    } else {
                        FailureCleanupState::Unisolated(frame)
                    };
                    entry.failure_cleanup = Some(PendingFailureCleanup {
                        task,
                        failure: ViewFailureKind::Apply,
                        message: "frame final preflight rejected",
                        state,
                    });
                    continue;
                }
                let active_is_valid_for_role = entry.active.as_ref().is_none_or(|frame| {
                    if pending.realized.is_some() {
                        validate_leaving_frame(world, frame)
                    } else {
                        validate_active_frame(world, frame)
                    }
                });
                if !active_is_valid_for_role {
                    entry.pending_commit = Some(pending);
                    continue;
                }

                let mut entering = pending.realized.take();
                let mut leaving = if entering.is_some() {
                    entry.active.take()
                } else {
                    None
                };
                let logical_roots = leaving
                    .as_ref()
                    .map(|frame| vec![frame.frame().clone()])
                    .unwrap_or_default();
                let gate_before = leaving
                    .as_ref()
                    .or(entry.active.as_ref())
                    .map(RealizedFrame::interactive_entities)
                    .unwrap_or_default();
                let gate_after = entering
                    .as_ref()
                    .or(entry.active.as_ref())
                    .map(RealizedFrame::interactive_entities)
                    .unwrap_or_default();
                let focus_leaving = leaving
                    .as_ref()
                    .map(|frame| {
                        let mut closure = observed_descendant_closure(
                            world,
                            frame.claims().map(|claim| claim.entity()),
                        )
                        .into_iter()
                        .collect::<Vec<_>>();
                        closure.sort_by_key(|entity| entity.to_bits());
                        closure
                    })
                    .unwrap_or_default();
                let mut targets = Vec::new();
                let mut transitions = Vec::new();
                if let Some(frame) = leaving.as_ref() {
                    targets.extend(frame.mutation_targets(
                        EntityLifecycle::Active,
                        EntityLifecycle::Leaving,
                        false,
                    ));
                    transitions.extend(frame.lifecycle_transitions(EntityLifecycle::Leaving));
                }
                let leaving_count = transitions.len();
                if let Some(frame) = entering.as_ref() {
                    targets.extend(frame.mutation_targets(
                        EntityLifecycle::PreparedInert,
                        EntityLifecycle::Active,
                        true,
                    ));
                    transitions.extend(frame.lifecycle_transitions(EntityLifecycle::Active));
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
                                                    Some(frame) => {
                                                        match FrozenCleanupManifest::from_frame(
                                                            world,
                                                            &registry,
                                                            frame,
                                                            &resulting_claims[..leaving_count],
                                                        ) {
                                                            Some(manifest) => manifest,
                                                            None => return None,
                                                        }
                                                    }
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
                                                let presentation_plan =
                                                    prepare_presentation_mutation(world, &targets)?;
                                                if pending.ticket.is_none() {
                                                    let task = pending.task.take()?;
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
                                                entry.engine.publish(prepared, || {
                                                    presentation_plan
                                                        .apply_with_split(leaving_count, || {
                                                            focus_plan.apply()
                                                        });
                                                    registry_plan.apply(&mut registry);
                                                    gate_plan.apply();
                                                });
                                                Some((cleanup_ledger, resulting_claims))
                                            },
                                        )
                                    },
                                )
                            },
                        )
                    },
                );
                let Some((cleanup_ledger, resulting_claims)) = committed else {
                    if let Some(frame) = leaving {
                        entry.active = Some(frame);
                    }
                    pending.realized = entering;
                    if pending.ticket.is_some() || pending.task.is_some() {
                        entry.pending_commit = Some(pending);
                    }
                    continue;
                };
                if let Some(frame) = leaving.as_mut() {
                    frame.replace_claims(&resulting_claims[..leaving_count]);
                }
                if let Some(frame) = entering.as_mut() {
                    frame.replace_claims(&resulting_claims[leaving_count..]);
                }
                if let Some(frame) = entering {
                    entry.active = Some(frame);
                }
                entry.cleanup_ledger = Some(CleanupLedger::activate(cleanup_ledger, leaving));
            }
        },
    );
    refresh_authority_audits(world);
}

pub(crate) fn settle_cleanup_system(world: &mut World) {
    world.resource_scope(
        |world, mut navigation: bevy_ecs::change_detection::Mut<RouterNavigation>| {
            for entry in &mut navigation.entries {
                if let Some(mut failure) = entry.failure_cleanup.take() {
                    let mut ledger = match failure.state {
                        FailureCleanupState::Prepared(ledger) => ledger,
                        FailureCleanupState::Unisolated(frame) => {
                            if !isolate_frame_for_settlement(world, &frame) {
                                failure.state = FailureCleanupState::Unisolated(frame);
                                entry.failure_cleanup = Some(failure);
                                continue;
                            }
                            match CleanupLedger::for_failed_realization(world, frame) {
                                Ok(ledger) => ledger,
                                Err(frame) => {
                                    failure.state = FailureCleanupState::Unprepared(frame);
                                    entry.failure_cleanup = Some(failure);
                                    continue;
                                }
                            }
                        }
                        FailureCleanupState::Unprepared(residual) => {
                            match CleanupLedger::for_failed_realization(world, residual) {
                                Ok(ledger) => ledger,
                                Err(residual) => {
                                    failure.state = FailureCleanupState::Unprepared(residual);
                                    entry.failure_cleanup = Some(failure);
                                    continue;
                                }
                            }
                        }
                    };
                    if !entry.observe_cleanup_receipts(&mut ledger) {
                        failure.state = FailureCleanupState::Prepared(ledger);
                        entry.failure_cleanup = Some(failure);
                        continue;
                    }
                    if !ledger.is_settled() {
                        let Some(attempt) = ledger.attempt(world) else {
                            failure.state = FailureCleanupState::Prepared(ledger);
                            entry.failure_cleanup = Some(failure);
                            continue;
                        };
                        let settled = ledger.settle(world, attempt);
                        if !entry.observe_cleanup_receipts(&mut ledger) {
                            failure.state = FailureCleanupState::Prepared(ledger);
                            entry.failure_cleanup = Some(failure);
                            continue;
                        }
                        if !settled {
                            failure.state = FailureCleanupState::Prepared(ledger);
                            entry.failure_cleanup = Some(failure);
                            continue;
                        }
                    }
                    entry
                        .engine
                        .report_failed(failure.task, failure.failure, failure.message)
                        .expect("a physically settled failed realization retains its exact task");
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
        components::{
            ProvisionalPostWriteProbe, ProvisionalSlotLifecycle, RouteOwnershipSeal,
            RouterInputBarrier,
        },
        content::{ContentColor, SolidRect},
        driver::{EcsViewDriver, ViewBuildReport},
        host::HostCandidateRevision,
        input::{RouteFocusState, RouterInputGate},
        plugin::{BevyUiRouterPlugin, RouterScheduleSet},
        presentation::FrameOutput,
        registry::{
            ComponentManifest, EntityOwner, ProvisionalTopSlotOwner, QuarantineLeaseTable,
            RegistryOwner,
        },
    };
    use bevy_app::{App, PostUpdate, PreUpdate, Update};
    use bevy_camera::visibility::{InheritedVisibility, Visibility};
    use bevy_ecs::{
        component::Component, entity::Entity, event::EntityEvent, hierarchy::ChildOf,
        lifecycle::HookContext, observer::On, prelude::Resource, query::With,
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
        NavigationFailureKind, RequestStatus, RouteNodeOptions, RoutePattern, RouteTree,
        RouteViewKey,
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

    struct AtomicDriver {
        key: ViewDriverKey,
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

    struct PanicDriver {
        key: ViewDriverKey,
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
                .and_then(|pending| pending.realized.as_ref())
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
                    entry.active.as_ref().unwrap().frame().clone(),
                    entry.active.as_ref().unwrap().shell(),
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
                entry.active.as_ref().unwrap().frame().clone(),
                entry.active.as_ref().unwrap().shell()
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
            (frame.shell(), frame.interactive_entities()[0])
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
        assert_ne!(entry.active.as_ref().unwrap().shell(), leaving);
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
            (frame.shell(), frame.interactive_entities()[0])
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
        assert_ne!(entry.active.as_ref().unwrap().shell(), leaving_shell);
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
        assert!(entry.pending_commit.as_ref().unwrap().task.is_some());
        assert!(entry.pending_commit.as_ref().unwrap().realized.is_some());
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
        assert!(entry.failure_cleanup.is_some());
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
                .failure_cleanup
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
                .and_then(|pending| pending.realized.as_ref())
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
        assert!(!validate_active_frame(app.world(), active));

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
        assert!(!validate_active_frame(app.world(), active));
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
                    navigation.entries[0].active.as_ref().unwrap(),
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
            .and_then(|pending| pending.realized.as_ref())
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
            ),
            app.world().resource::<RouterNavigation>().entries[0]
                .pending_commit
                .as_ref()
                .and_then(|pending| pending.realized.as_ref())
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
                    navigation.entries[0].active.as_ref().unwrap(),
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
                    RegistryOwner::Provisional(ProvisionalTopSlotOwner::new(revision)),
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
    fn unresolved_pending_residual_does_not_block_the_next_frame() {
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
                .failure_cleanup
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

        assert!(matches!(
            app.world().resource::<RouterNavigation>().entries[0]
                .engine
                .request(first)
                .unwrap()
                .status(),
            RequestStatus::Running(_)
        ));
        assert_eq!(
            app.world()
                .resource::<QuarantineLeaseTable>()
                .unresolved_residuals()
                .len(),
            1
        );
        assert!(
            app.world()
                .resource::<RouteEntityRegistry>()
                .records()
                .iter()
                .all(|record| !matches!(
                    record.owner(),
                    RegistryOwner::Committed(
                        EntityOwner::FrameShell(_)
                            | EntityOwner::FrameContentTarget(_)
                            | EntityOwner::FrameOutput { .. }
                    )
                ))
        );

        app.world_mut()
            .remove_resource::<PersistentOnePickableRemoval>();

        drive_navigation_system(app.world_mut());
        commit_presentation_system(app.world_mut());
        settle_cleanup_system(app.world_mut());
        {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            let request = entry.engine.request(first).unwrap();
            let RequestStatus::Failed(failure) = request.status() else {
                panic!("successful fallback must terminalize the original apply failure")
            };
            assert_eq!(failure.kind(), NavigationFailureKind::Apply);
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
            entry.active.as_ref().unwrap()
        ));
    }
}
