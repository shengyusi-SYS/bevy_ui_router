//! Dispose one invalid inactive leaf before resuming its original fresh task.
use super::*;
use crate::presentation::{RetainedInvalidationEvidence, RetainedPayloadClassification};

pub(super) struct Preparation {
    task: RootFrameTask,
    plan: r3_core::RetainedHitPlan,
    pending: PendingCommit,
    evidence: RetainedInvalidationEvidence,
}

pub(super) struct Installed {
    owner: r3_core::RetainedMissCleanupOwner,
    ledger: CleanupLedger,
    removal: Option<crate::cleanup::PreparedRetainedInvalidationRemovalStep>,
    pending: PendingCommit,
    boundaries: crate::registry::PreparedRetainedBoundaries,
    invalidation: RetainedInvalidationEvidence,
}

pub(super) enum Rejection {
    PhysicalSource,
    Ledger(CleanupLedgerReserveErrorKind),
    Removal(crate::cleanup::RetainedInvalidationRemovalPreparationError),
    Core(IntegrationError),
    Publication(r3_core::RetainedPublicationError),
    #[cfg(feature = "bsn")]
    Native(crate::presentation::native_frame::NativeCleanupPreparationError),
    Receipt(IntegrationError),
    Settlement,
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
}

/// Classification precedes the decision to bypass the fresh factory. Shared
/// authority loss keeps the existing reconciliation/recovery path.
pub(super) fn classify(
    world: &World,
    entry: &RouterEntry,
    plan: &r3_core::RetainedHitPlan,
) -> Option<RetainedPayloadClassification> {
    let branch = entry.retained.payload_for(plan.hit())?;
    let frame = branch.frame();
    world
        .get_resource::<RouteEntityRegistry>()?
        .classify_retained_payload(world, plan.hit(), frame, branch.child_slots())
}

pub(super) fn begin(
    entry: &mut RouterEntry,
    task: RootFrameTask,
    plan: r3_core::RetainedHitPlan,
    pending: PendingCommit,
    evidence: RetainedInvalidationEvidence,
) {
    entry.retained_miss_cleanup = Some(Box::new(RetainedMissCleanup::Preparing(Preparation {
        task,
        plan,
        pending,
        evidence,
    })));
}

fn prepare(world: &mut World, entry: &mut RouterEntry, source: Preparation) {
    // No owner has moved yet. If the observed World source changed between
    // classification and preparation, return the original task to the driver
    // so its existing current-authority path observes the new facts.
    if !source.evidence.is_current(world) {
        entry.pending_commit = Some(source.pending);
        entry.pending_mount_frame_task = Some(source.task);
        return;
    }
    let projection = source.plan.hit().clone();
    if entry.retained.payload_for(&projection).is_none() {
        reject_preparation(entry, source, Rejection::PhysicalSource);
        return;
    }
    let Some(mut selection) = entry
        .retained
        .prepare_change(std::slice::from_ref(&projection), false)
    else {
        entry.retained_miss_cleanup = Some(Box::new(RetainedMissCleanup::Preparing(source)));
        return;
    };
    #[cfg(feature = "bsn")]
    {
        let prepared = selection
            .selected_mut(&mut entry.retained)
            .try_for_each(|(_, branch)| branch.prepare_native_cleanup(world));
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
    let branch = entry
        .retained
        .payload_for(&projection)
        .expect("exact selection retains its payload");
    let Some(manifest) = FrozenCleanupManifest::from_retained_invalidation(
        world,
        registry,
        branch,
        &source.evidence,
    ) else {
        reject_preparation(entry, source, Rejection::PhysicalSource);
        return;
    };
    let core_manifest = CleanupManifest::new(
        vec![projection.frame().clone()],
        manifest.digest(),
        manifest.len(),
    );
    let Ok(boundaries) =
        registry.prepare_retained_boundaries(None, std::slice::from_ref(&projection))
    else {
        reject_preparation(entry, source, Rejection::PhysicalSource);
        return;
    };
    let reserved =
        match CleanupLedger::reserve_retained_invalidation(world, manifest, branch.frame()) {
            Ok(reserved) => reserved,
            Err(error) if error.kind() == CleanupLedgerReserveErrorKind::RetryableCapacity => {
                entry.retained_miss_cleanup =
                    Some(Box::new(RetainedMissCleanup::Preparing(source)));
                return;
            }
            Err(error) => {
                reject_preparation(entry, source, Rejection::Ledger(error.kind()));
                return;
            }
        };
    let prepared = match reserved.prepare_retained_invalidation_removal(world, branch) {
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
        task,
        plan,
        pending,
        evidence,
    } = source;
    let retained = &mut entry.retained;
    let destination = &mut entry.retained_miss_cleanup;
    let mut pending = Some(pending);
    let mut evidence = Some(evidence);
    let result =
        entry
            .engine
            .begin_retained_miss_cleanup(task, plan, core_manifest, owners, |owner| {
                let mut payloads = selection.into_payloads(retained);
                let branch = payloads
                    .pop()
                    .expect("the selected original cache owns one payload");
                assert!(payloads.is_empty());
                let (ledger, removal) = prepared.activate(owner.cleanup().clone(), branch);
                *destination = Some(Box::new(RetainedMissCleanup::Cleaning(Installed {
                    owner,
                    ledger,
                    removal: Some(removal),
                    pending: pending
                        .take()
                        .expect("original fresh continuation moves once"),
                    boundaries,
                    invalidation: evidence
                        .take()
                        .expect("original invalidation source moves with its cleanup owner"),
                })));
            });
    if let Err(error) = result {
        let (task, plan, _, owners, error) = error.into_parts();
        assert!(
            retained.restore_owners(owners).is_ok(),
            "rejected disposal returns the original cache owner"
        );
        *destination = Some(Box::new(RetainedMissCleanup::PreparationRejected {
            owner: Preparation {
                task,
                plan,
                pending: pending.take().unwrap(),
                evidence: evidence.take().unwrap(),
            },
            evidence: Rejection::Publication(error),
        }));
    }
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
        installed
            .ledger
            .settle_stopped_retained_invalidation(world, &installed.invalidation);
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
        invalidation,
    } = installed;
    let mut resumed = None;
    match entry.engine.complete_retained_miss_cleanup(
        owner,
        completion,
        (ledger, pending, boundaries, invalidation),
        |(_, pending, boundaries, _)| {
            boundaries.apply(&mut registry);
            resumed = Some(pending);
        },
    ) {
        Ok(task) => {
            entry.pending_commit = resumed;
            entry.pending_mount_frame_task = Some(task);
        }
        Err(error) => {
            let (owner, _, (ledger, pending, boundaries, invalidation), error) = error.into_parts();
            reject_cleanup(
                entry,
                Installed {
                    owner,
                    ledger,
                    removal: None,
                    pending,
                    boundaries,
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
