//! Retire an invalid selected instance and its inventory dependencies before resuming.
use super::*;
use crate::presentation::{RetainedInvalidationEvidence, RetainedPayloadClassification};

pub(super) struct Preparation {
    pub(super) receiver: r3_core::RetainedInvalidationReceiver,
    pub(super) selection: Option<r3_core::InstanceSelectionPlan>,
    pub(super) plan: Option<r3_core::RetainedInvalidationPlan>,
    pub(super) pending: PendingCommit,
    // The first-stage failure cause is never replaced by a successor observation.
    pub(super) evidence: RetainedInvalidationEvidence,
    pub(super) successor: Option<RetainedInvalidationEvidence>,
}

pub(super) struct Installed {
    pub(super) owner: r3_core::RetainedMissCleanupOwner,
    pub(super) ledger: CleanupLedger,
    pub(super) removal: Option<crate::cleanup::PreparedRetainedInvalidationRemovalStep>,
    pub(super) pending: PendingCommit,
    pub(super) boundaries: crate::registry::PreparedRetainedBoundaries,
    pub(super) cause: RetainedInvalidationEvidence,
    pub(super) invalidation: Vec<RetainedInvalidationEvidence>,
}

pub(super) struct ProtocolResidual {
    pub(super) owners: Vec<r3_core::RetainedLeafOwner>,
    pub(super) removal: Option<crate::cleanup::PreparedRetainedInvalidationRemovalStep>,
    pub(super) boundaries: crate::registry::PreparedRetainedBoundaries,
    pub(super) invalidation: Vec<RetainedInvalidationEvidence>,
}

pub(super) enum Rejection {
    PhysicalSource,
    AuthorityLost(RetainedInvalidationEvidence),
    Ledger(CleanupLedgerReserveErrorKind),
    Removal(crate::cleanup::RetainedInvalidationRemovalPreparationError),
    Core(IntegrationError),
    Resume(r3_core::RetainedInvalidationResumeFailure),
    Publication(r3_core::RetainedPublicationError),
    #[cfg(feature = "bsn")]
    Native(crate::presentation::native_frame::NativeCleanupPreparationError),
    Receipt(IntegrationError),
    Settlement,
    AuthorityTerminal {
        prior: Box<Rejection>,
        kind: r3_core::RetainedMissCleanupAuthorityFailCloseErrorKind,
    },
}

pub(super) enum RetainedMissCleanup {
    Preparing(Preparation),
    Cleaning(Installed),
    PreparationRejected {
        owner: Preparation,
        evidence: Rejection,
    },
    CleanupRejected {
        owner: Installed,
        evidence: Rejection,
    },
    AuthorityProtocol {
        owner: Preparation,
        authority: RetainedInvalidationEvidence,
        reason: AuthorityProtocolResidualKind,
        isolation: Option<(
            PreparedAuthorityIsolation,
            Option<AuthorityIsolationContradiction>,
        )>,
        rejection: Option<r3_core::RetainedInvalidationAuthorityFailCloseErrorKind>,
    },
}

/// Selected instances are a parent-to-child prefix. The earliest invalid
/// instance is the seed whose Core dependency closure covers later descendants.
/// Shared authority loss retains its existing reconciliation/recovery path.
pub(super) fn classify(
    world: &World,
    entry: &RouterEntry,
    selected: &[r3_core::RetainedLeafProjection],
) -> Option<RetainedPayloadClassification> {
    let registry = world.get_resource::<RouteEntityRegistry>()?;
    for projection in selected {
        let payload = entry.retained.payload_for(projection)?;
        match registry.classify_retained_payload(
            world,
            projection,
            payload.frame(),
            payload.child_slots(),
        )? {
            RetainedPayloadClassification::Current => {}
            invalid => return Some(invalid),
        }
    }
    Some(RetainedPayloadClassification::Current)
}

pub(super) fn begin(
    entry: &mut RouterEntry,
    receiver: r3_core::RetainedInvalidationReceiver,
    selection: Option<r3_core::InstanceSelectionPlan>,
    pending: PendingCommit,
    evidence: RetainedInvalidationEvidence,
) {
    let mut source = Preparation {
        receiver,
        selection,
        plan: None,
        pending,
        evidence,
        successor: None,
    };
    let Some(inventory) = entry.retained.inventory() else {
        reject_preparation(entry, source, Rejection::PhysicalSource);
        return;
    };
    match entry.engine.prepare_retained_invalidation_plan(
        &source.receiver,
        source.selection.as_ref(),
        source.evidence.projection().frame(),
        &inventory,
    ) {
        Ok(plan) => source.plan = Some(plan),
        Err(error) => {
            reject_preparation(entry, source, Rejection::Core(error));
            return;
        }
    }
    entry.retained_miss_cleanup = Some(Box::new(RetainedMissCleanup::Preparing(source)));
}

fn prepare(world: &mut World, entry: &mut RouterEntry, mut source: Preparation) {
    if matches!(
        &source.receiver,
        r3_core::RetainedInvalidationReceiver::AfterFailure(_)
    ) {
        // The first-stage failure receipts authorize removal of fresh suffix
        // children that may occur in the original observation. Observe only the
        // same original inventory instance after that cleanup; retain its cause.
        let observed = world
            .get_resource::<RouteEntityRegistry>()
            .and_then(|registry| {
                let payload = entry.retained.payload_for(source.evidence.projection())?;
                registry.classify_retained_payload(
                    world,
                    source.evidence.projection(),
                    payload.frame(),
                    payload.child_slots(),
                )
            });
        match observed {
            Some(RetainedPayloadClassification::OwnInvalid(observed)) => {
                source.successor = Some(observed);
            }
            Some(RetainedPayloadClassification::AuthorityLost(observed)) => {
                reject_preparation(entry, source, Rejection::AuthorityLost(observed));
                return;
            }
            Some(RetainedPayloadClassification::Current) => {
                resume_recovered(entry, source);
                return;
            }
            None => {
                reject_preparation(entry, source, Rejection::PhysicalSource);
                return;
            }
        }
    } else if !source.evidence.is_current(world) {
        // No capability has moved. Return the exact original dispatch receiver
        // so the ordinary current-authority path observes the changed facts.
        let Preparation {
            receiver, pending, ..
        } = source;
        resume_receiver(entry, receiver, pending);
        return;
    }
    let Some(plan) = source.plan.as_ref() else {
        reject_preparation(entry, source, Rejection::PhysicalSource);
        return;
    };
    let retiring = plan.retiring().to_vec();
    let logical_roots = plan.logical_roots().to_vec();
    if retiring.is_empty()
        || retiring
            .iter()
            .any(|projection| entry.retained.payload_for(projection).is_none())
    {
        reject_preparation(entry, source, Rejection::PhysicalSource);
        return;
    }
    let Some(mut selection) = entry.retained.prepare_change(&retiring, false) else {
        entry.retained_miss_cleanup = Some(Box::new(RetainedMissCleanup::Preparing(source)));
        return;
    };
    #[cfg(feature = "bsn")]
    {
        let prepared = selection
            .selected_mut(&mut entry.retained)
            .try_for_each(|(_, payload)| payload.prepare_native_cleanup(world));
        if let Err(error) = prepared {
            if error.is_retryable() {
                entry.retained_miss_cleanup =
                    Some(Box::new(RetainedMissCleanup::Preparing(source)));
            } else {
                reject_preparation(entry, source, Rejection::Native(error));
            }
            return;
        }
    }
    let Some(registry) = world.get_resource::<RouteEntityRegistry>() else {
        reject_preparation(entry, source, Rejection::PhysicalSource);
        return;
    };
    let branches = selection.selected(&entry.retained).collect::<Vec<_>>();
    let seed = source.successor.as_ref().unwrap_or(&source.evidence);
    let mut invalidation = Vec::with_capacity(branches.len());
    invalidation.push(seed.clone());
    for (projection, payload) in &branches {
        if *projection == seed.projection() {
            continue;
        }
        match registry.classify_retained_payload(
            world,
            projection,
            payload.frame(),
            payload.child_slots(),
        ) {
            Some(RetainedPayloadClassification::Current) => {}
            Some(
                RetainedPayloadClassification::OwnInvalid(observed)
                | RetainedPayloadClassification::AuthorityLost(observed),
            ) => invalidation.push(observed),
            None => {
                reject_preparation(entry, source, Rejection::PhysicalSource);
                return;
            }
        }
    }
    let Some(manifest) = FrozenCleanupManifest::from_retained_invalidation(
        world,
        registry,
        &branches,
        &invalidation,
    ) else {
        reject_preparation(entry, source, Rejection::PhysicalSource);
        return;
    };
    let core_manifest = CleanupManifest::new(logical_roots, manifest.digest(), manifest.len());
    let Ok(boundaries) = registry.prepare_retained_boundaries(&[], &retiring) else {
        reject_preparation(entry, source, Rejection::PhysicalSource);
        return;
    };
    let frames = branches
        .iter()
        .map(|(_, payload)| payload.frame())
        .collect::<Vec<_>>();
    let reserved = match CleanupLedger::reserve_retained_invalidation(world, manifest, &frames) {
        Ok(reserved) => reserved,
        Err(error) if error.kind() == CleanupLedgerReserveErrorKind::RetryableCapacity => {
            entry.retained_miss_cleanup = Some(Box::new(RetainedMissCleanup::Preparing(source)));
            return;
        }
        Err(error) => {
            reject_preparation(entry, source, Rejection::Ledger(error.kind()));
            return;
        }
    };
    let prepared = match reserved.prepare_retained_invalidation_removal(world, &branches) {
        Ok(prepared) => prepared,
        Err(error) if error.is_retryable() => {
            entry.retained_miss_cleanup = Some(Box::new(RetainedMissCleanup::Preparing(source)));
            return;
        }
        Err(error) => {
            reject_preparation(entry, source, Rejection::Removal(error));
            return;
        }
    };
    let owners = selection.take_owners(&mut entry.retained);
    let Preparation {
        receiver,
        selection: original_selection,
        plan,
        pending,
        evidence,
        successor,
    } = source;
    let retained = &mut entry.retained;
    let destination = &mut entry.retained_miss_cleanup;
    let mut pending = Some(pending);
    let mut cause = Some(evidence);
    let mut invalidation = Some(invalidation);
    let result = entry.engine.begin_retained_miss_cleanup(
        receiver,
        plan.expect("a prepared retirement owns its Core plan"),
        core_manifest,
        owners,
        |owner| {
            let payloads = selection.into_payloads(retained);
            let (ledger, removal) = prepared.activate(owner.cleanup().clone(), payloads);
            *destination = Some(Box::new(RetainedMissCleanup::Cleaning(Installed {
                owner,
                ledger,
                removal: Some(removal),
                pending: pending
                    .take()
                    .expect("the original dispatch continuation moves once"),
                boundaries,
                cause: cause
                    .take()
                    .expect("the original invalidation cause moves once"),
                invalidation: invalidation
                    .take()
                    .expect("the original forest observation moves once"),
            })));
        },
    );
    if let Err(error) = result {
        let (receiver, plan, _, owners, error) = error.into_parts();
        assert!(
            retained.restore_owners(owners).is_ok(),
            "rejected retirement returns every original inventory owner"
        );
        *destination = Some(Box::new(RetainedMissCleanup::PreparationRejected {
            owner: Preparation {
                receiver,
                selection: original_selection,
                plan: Some(plan),
                pending: pending.take().unwrap(),
                evidence: cause.take().unwrap(),
                successor,
            },
            evidence: Rejection::Publication(error),
        }));
    }
}

fn resume_recovered(entry: &mut RouterEntry, source: Preparation) {
    let Some(inventory) = entry.retained.inventory() else {
        reject_preparation(entry, source, Rejection::PhysicalSource);
        return;
    };
    let Preparation {
        receiver,
        selection,
        plan,
        pending,
        evidence,
        successor,
    } = source;
    let r3_core::RetainedInvalidationReceiver::AfterFailure(continuation) = receiver else {
        unreachable!("only the original after-failure capability resumes a recovered instance")
    };
    match entry
        .engine
        .resume_retained_invalidation(continuation, &inventory)
    {
        Ok(action) => resume_action(entry, action, pending),
        Err(error) => {
            let (continuation, error) = error.into_parts();
            reject_preparation(
                entry,
                Preparation {
                    receiver: r3_core::RetainedInvalidationReceiver::AfterFailure(continuation),
                    selection,
                    plan,
                    pending,
                    evidence,
                    successor,
                },
                Rejection::Resume(error),
            );
        }
    }
}

fn resume_receiver(
    entry: &mut RouterEntry,
    receiver: r3_core::RetainedInvalidationReceiver,
    mut pending: PendingCommit,
) {
    match receiver {
        r3_core::RetainedInvalidationReceiver::FrameTask(task) => {
            entry.pending_mount_frame_task = Some(task);
        }
        r3_core::RetainedInvalidationReceiver::Commit(ticket) => pending.ticket = Some(ticket),
        r3_core::RetainedInvalidationReceiver::AfterFailure(_) => {
            unreachable!("after-failure capability stays in its original preparation holder")
        }
    }
    entry.pending_commit = Some(pending);
}

fn reject_preparation(entry: &mut RouterEntry, owner: Preparation, evidence: Rejection) {
    entry.retained_miss_cleanup = Some(Box::new(RetainedMissCleanup::PreparationRejected {
        owner,
        evidence,
    }));
}

fn observe_receipts(
    entry: &mut RouterEntry,
    installed: &mut Installed,
) -> Result<(), IntegrationError> {
    while let Some((obligation, disposition)) = installed.ledger.next_unobserved_receipt() {
        entry
            .engine
            .observe_retained_miss_cleanup_receipt(&mut installed.owner, disposition.public())?;
        if !installed
            .ledger
            .mark_receipt_observed(obligation, disposition)
        {
            return Err(IntegrationError::Stale);
        }
        let quarantine = installed.ledger.mark_quarantine_event_observed(disposition);
        entry.cleanup_observation.record(disposition, quarantine);
    }
    Ok(())
}

/// This responsibility always runs before normal/postcommit cleanup. A cache
/// token cannot be interpreted as the foreground request's normal token.
pub(super) fn drive(world: &mut World, entry: &mut RouterEntry) -> bool {
    let Some(work) = entry.retained_miss_cleanup.take() else {
        return false;
    };
    let mut installed = match *work {
        RetainedMissCleanup::Preparing(source) => {
            prepare(world, entry, source);
            let Some(prepared) = entry.retained_miss_cleanup.take() else {
                return true;
            };
            match *prepared {
                RetainedMissCleanup::Cleaning(installed) => installed,
                other => {
                    entry.retained_miss_cleanup = Some(Box::new(other));
                    return true;
                }
            }
        }
        RetainedMissCleanup::Cleaning(installed) => installed,
        RetainedMissCleanup::CleanupRejected { owner, evidence }
            if owner.ledger.is_stopped()
                && entry
                    .host
                    .as_ref()
                    .is_some_and(|host| !current_host_binding_is_exact(world, host)) =>
        {
            settle_authority_protocol(world, entry, owner, evidence);
            return true;
        }
        rejected => {
            entry.retained_miss_cleanup = Some(Box::new(rejected));
            return true;
        }
    };
    if let Err(error) = observe_receipts(entry, &mut installed) {
        reject_cleanup(entry, installed, Rejection::Receipt(error));
        return true;
    }
    if let Some(removal) = installed.removal.take() {
        installed
            .ledger
            .settle_prepared_retained_invalidation(world, removal);
        if let Err(error) = observe_receipts(entry, &mut installed) {
            reject_cleanup(entry, installed, Rejection::Receipt(error));
            return true;
        }
    }
    if !installed.ledger.is_settled() && installed.ledger.is_stopped() {
        // The stopped removal cannot resume. Settle only the original token's
        // removed prefix and exact inert shell residual, preserving its source.
        installed.ledger.settle_stopped_retained_invalidation(
            world,
            installed
                .invalidation
                .first()
                .expect("retirement preserves its seed observation"),
        );
        if let Err(error) = observe_receipts(entry, &mut installed) {
            reject_cleanup(entry, installed, Rejection::Receipt(error));
            return true;
        }
    }
    if !installed.ledger.is_settled() {
        reject_cleanup(entry, installed, Rejection::Settlement);
        return true;
    }
    let completion = installed
        .ledger
        .pending_completion()
        .expect("settled cache ledger owns its exact completion");
    let Some(mut registry) = world.get_resource_mut::<RouteEntityRegistry>() else {
        reject_cleanup(entry, installed, Rejection::PhysicalSource);
        return true;
    };
    let Installed {
        owner,
        ledger,
        removal: _,
        pending,
        boundaries,
        cause,
        invalidation,
    } = installed;
    let mut resumed = None;
    match entry.engine.complete_retained_miss_cleanup(
        owner,
        completion,
        (ledger, pending, boundaries, cause, invalidation),
        |(_, pending, boundaries, _, _)| {
            boundaries.apply(&mut registry);
            resumed = Some(pending);
        },
    ) {
        Ok(action) => {
            resume_action(
                entry,
                action,
                resumed.expect("accepted retirement returns its physical continuation"),
            );
        }
        Err(error) => {
            let (owner, _, (ledger, pending, boundaries, cause, invalidation), error) =
                error.into_parts();
            reject_cleanup(
                entry,
                Installed {
                    owner,
                    ledger,
                    removal: None,
                    pending,
                    boundaries,
                    cause,
                    invalidation,
                },
                Rejection::Core(error),
            );
        }
    }
    true
}

fn reject_cleanup(entry: &mut RouterEntry, owner: Installed, evidence: Rejection) {
    entry.retained_miss_cleanup = Some(Box::new(RetainedMissCleanup::CleanupRejected {
        owner,
        evidence,
    }));
}

fn settle_authority_protocol(
    world: &mut World,
    entry: &mut RouterEntry,
    installed: Installed,
    evidence: Rejection,
) {
    let (isolation, isolation_contradiction) =
        match authority_isolation::prepare_retained_cleanup_protocol_isolation(
            world,
            entry,
            &installed.pending,
            Some(&installed.ledger),
        ) {
            AuthorityIsolationPreparation::Ready(isolation) => (isolation, None),
            AuthorityIsolationPreparation::PermanentContradiction {
                contradiction,
                fail_closed,
            } => (fail_closed, Some(contradiction)),
            AuthorityIsolationPreparation::RetryableCapacity => {
                reject_cleanup(entry, installed, evidence);
                return;
            }
        };
    let evidence = match evidence {
        Rejection::AuthorityTerminal { prior, .. } => *prior,
        other => other,
    };
    let Installed {
        owner,
        ledger,
        removal,
        pending,
        boundaries,
        cause,
        invalidation,
    } = installed;
    let (host, lease) = entry
        .host
        .take()
        .expect("shared authority loss retains the original installed host lease")
        .into_retired();
    let mut residual = Some(AuthorityProtocolResidualOwner {
        back_target: entry.pending_back_target_execution.take().map(Box::new),
        kind: AuthorityProtocolResidualKind::RetainedCleanupAuthority {
            cause,
            authority: LifecyclePreflightEvidence::new(
                LifecyclePreflightSource::HostBinding,
                LifecyclePreflightExpectation::ExactAuthority,
                LifecyclePreflightObserved::Drifted,
            ),
            reason: Box::new(evidence),
        },
        isolation_contradiction,
        active: entry.active.take(),
        pending: Some(pending),
        failed: None,
        failure_cleanup_completion: None,
        failure_cleanup_ledger: None,
        cleanup_settlement_ledger: Some(ledger),
        failure_cleanup_permanent_authority: None,
        retained: None,
        retained_cleanup: Some(ProtocolResidual {
            owners: Vec::new(),
            removal,
            boundaries,
            invalidation,
        }),
        host: Some(host),
    });
    let retained = &mut entry.retained;
    let destination = &mut entry.authority_protocol_residual;
    if let Err(error) = entry
        .engine
        .fail_closed_retained_miss_cleanup_authority_protocol_retaining(owner, lease, |owners| {
            let mut residual = residual
                .take()
                .expect("accepted terminal transfer moves the original physical owner once");
            residual.retained_cleanup.as_mut().unwrap().owners = owners;
            residual.retained = Some(std::mem::take(retained));
            isolation.apply(world);
            *destination = Some(residual);
        })
    {
        let (owner, lease, kind) = error.into_parts();
        let mut residual = residual
            .take()
            .expect("rejected terminal transfer returns the original physical owner");
        entry.host = Some(residual.host.take().unwrap().restore(lease));
        entry.active = residual.active.take();
        entry.pending_back_target_execution = residual.back_target.take().map(|owner| *owner);
        let AuthorityProtocolResidualKind::RetainedCleanupAuthority { cause, reason, .. } =
            residual.kind
        else {
            unreachable!("the terminal sink preserves its original cleanup cause")
        };
        let physical = residual.retained_cleanup.take().unwrap();
        assert!(
            physical.owners.is_empty(),
            "rejection returns the original owners in the Core capability"
        );
        reject_cleanup(
            entry,
            Installed {
                owner,
                ledger: residual.cleanup_settlement_ledger.take().unwrap(),
                removal: physical.removal,
                pending: residual.pending.take().unwrap(),
                boundaries: physical.boundaries,
                cause,
                invalidation: physical.invalidation,
            },
            Rejection::AuthorityTerminal {
                prior: reason,
                kind,
            },
        );
    }
}

fn resume_action(entry: &mut RouterEntry, action: EngineAction, mut pending: PendingCommit) {
    match action {
        EngineAction::FrameTask(task) => {
            entry.pending_mount_frame_task = Some(task);
            entry.pending_commit = Some(pending);
        }
        EngineAction::MountRequest(request) => {
            entry.pending_mount_request = Some(request);
            entry.pending_commit = Some(pending);
        }
        EngineAction::Commit(ticket) => {
            pending.ticket = Some(ticket);
            entry.pending_commit = Some(pending);
        }
        EngineAction::FailureCleanup(request) => {
            entry.pending_failed_branch = Some(PendingFailedBranch {
                terminal: PendingFailedBranchTerminal::Request(request),
                branch: failed_prepared_branch(pending, None),
            });
        }
        EngineAction::BackResolution(handoff) => {
            entry.pending_commit = Some(pending);
            bind_back_resolution_handoff(entry, handoff);
        }
    }
}
