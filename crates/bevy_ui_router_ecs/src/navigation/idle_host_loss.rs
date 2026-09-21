//! Host-grant-scoped physical settlement after all foreground requests end.
use super::*;
use crate::cleanup::{
    AuthorityCleanupFailure, AuthorityCleanupFailureKind, IdleHostResidualObservation,
};
use bevy_ui_router_core::integration::{
    IdleHostLossOwner, IdleHostLossPhysicalDisposition, IdleHostLossPhysicalFacts,
    IdleHostLossTerminal,
};

pub(super) enum IdleHostLoss {
    Reserving(ReservingIdleHostLoss),
    Settling(SettlingIdleHostLoss),
}

pub(super) struct ReservingIdleHostLoss {
    core: IdleHostLossOwner,
    physical: UnreservedIdleHost,
    permanent: Option<crate::cleanup::CleanupLedgerReserveErrorKind>,
}

struct UnreservedIdleHost {
    active: Option<RealizedBranch>,
    host: RetiredHostBinding,
    frozen: FrozenCleanupManifest,
    partition: crate::cleanup::FrozenBackNoTargetAuthorityPartition,
    host_children: Vec<Entity>,
}

pub(super) struct SettlingIdleHostLoss {
    core: IdleHostLossOwner,
    ledger: CleanupLedger,
    manifest: CleanupManifest,
    failure: Option<AuthorityCleanupFailure>,
    ready: Option<crate::cleanup::BackNoTargetAuthorityCompletionFacts>,
    residual: Option<IdleHostResidualObservation>,
}

#[cfg(test)]
impl SettlingIdleHostLoss {
    pub(super) fn ledger_projection(&self) -> crate::cleanup::PreparedCleanupLedgerProjection {
        self.ledger.projection()
    }
}

pub(super) struct IdleHostTerminal {
    physical: IdleHostPhysicalTerminal,
    terminal: IdleHostLossTerminal,
    previous: Option<Box<IdleHostTerminal>>,
}

#[cfg(test)]
impl IdleHostTerminal {
    pub(super) fn failure_kind(&self) -> Option<AuthorityCleanupFailureKind> {
        match &self.physical {
            IdleHostPhysicalTerminal::Residual { failure, .. } => Some(failure.kind),
            _ => None,
        }
    }

    pub(super) fn disposition(&self) -> IdleHostLossPhysicalDisposition {
        self.terminal.physical_facts().disposition
    }
}

enum IdleHostPhysicalTerminal {
    Settled {
        host: RetiredHostBinding,
        observed: IdleHostResidualObservation,
        retained: Option<crate::cleanup::IdleHostCompletedRetention>,
    },
    Residual {
        ledger: CleanupLedger,
        observed: IdleHostResidualObservation,
        failure: AuthorityCleanupFailure,
    },
    Unreserved {
        physical: UnreservedIdleHost,
        failure: crate::cleanup::CleanupLedgerReserveErrorKind,
    },
}

fn physically_idle(entry: &RouterEntry) -> bool {
    entry.engine.idle_host_loss_eligible()
        && entry.host_settlement.is_none()
        && entry.pending_host_admission_failure.is_none()
        && entry.pending_host_confirmation.is_none()
        && entry.pending_commit.is_none()
        && entry.pending_frame_drive.is_none()
        && entry.pending_stale_output_cleanup.is_none()
        && entry.pending_mount_frame_task.is_none()
        && entry.pending_mount_request.is_none()
        && entry.pending_mount_mutation_rejection.is_none()
        && entry.pending_failed_branch.is_none()
        && entry.failure_cleanup_permanent.is_none()
        && entry.cleanup_settlement_permanent_inert.is_none()
        && entry.projectionless_fail_close.is_none()
        && entry.cleanup_ledger.is_none()
        && entry.post_commit_compound.is_none()
        && entry.failure_cleanup_completion.is_none()
        && entry.authority_protocol_residual.is_none()
        && entry.back_capacity_enqueue_retire_residual.is_none()
        && entry.back_capacity_reservations.is_empty()
        && entry.pending_back_resolution_action.is_none()
        && entry.pending_bound_back_resolution.is_none()
        && entry.pending_back_target_execution.is_none()
        && entry.pending_back_no_target_completion.is_none()
        && entry.pending_back_no_target_preparation.is_none()
        && entry.pending_back_no_target_authority_cleanup.is_none()
        && entry.pending_back_target_permanent_cleanup.is_none()
}

pub(super) fn allows_candidate(
    world: &World,
    terminal: &IdleHostTerminal,
    candidate: &PreparedEcsHostCandidateSource,
) -> bool {
    if terminal
        .previous
        .as_ref()
        .is_some_and(|previous| !allows_candidate(world, previous, candidate))
    {
        return false;
    }
    let host = match &terminal.physical {
        IdleHostPhysicalTerminal::Settled {
            host,
            observed,
            retained,
        } => {
            if !observed.input_is_revoked(world)
                || retained.as_ref().is_some_and(|retained| {
                    !retained.allows_candidate(world, candidate.candidate.host())
                })
            {
                return false;
            }
            host
        }
        IdleHostPhysicalTerminal::Residual { ledger, .. } => {
            if ledger
                .observe_idle_host_residual(world)
                .is_none_or(|observed| !observed.is_safe())
            {
                return false;
            }
            let Some(host) = ledger.idle_host_authority_host() else {
                return false;
            };
            if crate::input::observed_descendant_closure(
                world,
                ledger.exact_claims().map(|claim| claim.entity()),
            )
            .contains(&candidate.candidate.host())
            {
                return false;
            }
            host
        }
        IdleHostPhysicalTerminal::Unreserved { physical, .. } => {
            if physical
                .frozen
                .observe_idle_host_residual(world, &physical.host)
                .is_none_or(|observed| !observed.is_safe())
            {
                return false;
            }
            if crate::input::observed_descendant_closure(
                world,
                physical.frozen.obligation_entities(),
            )
            .contains(&candidate.candidate.host())
            {
                return false;
            }
            &physical.host
        }
    };
    if !terminal.terminal.matches_host(&host.version, host.slot_id) {
        return false;
    }
    if world
        .get::<ChildOf>(host.top_slot)
        .is_some_and(|parent| parent.parent() == host.host)
        || world
            .get::<Children>(host.host)
            .is_some_and(|children| children.contains(&host.top_slot))
    {
        return false;
    }
    candidate.candidate.host() != host.host
        || candidate.registration_incarnation != host.registration_incarnation
}

#[derive(Debug)]
pub(super) struct IdleHostAdmissionFailure {
    version: bevy_ui_router_core::HostVersion,
    slot: bevy_ui_router_core::ids::RouteSlotId,
    expected: IdleHostLossPhysicalFacts,
    pub(super) detail: IdleHostAdmissionFailedFact,
}

#[derive(Debug)]
pub(super) enum IdleHostAdmissionFailedFact {
    Retained(crate::cleanup::IdleHostRetainedAdmissionEvidence),
    Input {
        expected: IdleHostResidualObservation,
        observed: Vec<crate::input::RouterTargetInputObservation>,
    },
    Residual {
        expected: Option<IdleHostResidualObservation>,
        observed: Option<IdleHostResidualObservation>,
    },
    HostEdge {
        host: Entity,
        top: Entity,
        parent: Option<Entity>,
        children: Vec<Entity>,
        expected_incarnation: u64,
        candidate_host: Entity,
        candidate_incarnation: u64,
        terminal_matches: bool,
    },
}

impl IdleHostAdmissionFailure {
    pub(super) fn expected_digest(&self) -> u64 {
        match &self.detail {
            IdleHostAdmissionFailedFact::Retained(evidence) => evidence.expected_digest(),
            _ => crate::cleanup::admission_evidence_digest(&(
                &self.version,
                self.slot,
                self.expected,
            )),
        }
    }

    pub(super) fn observed_digest(&self) -> u64 {
        match &self.detail {
            IdleHostAdmissionFailedFact::Retained(evidence) => evidence.observed_digest(),
            _ => {
                crate::cleanup::admission_evidence_digest(&(&self.version, self.slot, &self.detail))
            }
        }
    }
}

// Freeze the actual rejected prerequisite before subsequent hide/settlement
// writes. This cause is separate from the new slot's cleanup partition.
pub(super) fn admission_failure(
    world: &World,
    terminal: &IdleHostTerminal,
    candidate: &PreparedEcsHostCandidateSource,
) -> Option<IdleHostAdmissionFailure> {
    if let Some(previous) = terminal.previous.as_ref()
        && let Some(failure) = admission_failure(world, previous, candidate)
    {
        return Some(failure);
    }
    if allows_candidate(world, terminal, candidate) {
        return None;
    }
    let (host, detail) = match &terminal.physical {
        IdleHostPhysicalTerminal::Settled {
            host,
            observed,
            retained,
        } => {
            let detail = if !observed.input_is_revoked(world) {
                Some(IdleHostAdmissionFailedFact::Input {
                    expected: observed.clone(),
                    observed: observed.current_input(world),
                })
            } else if let Some(retained) = retained
                .as_ref()
                .filter(|retained| !retained.allows_candidate(world, candidate.candidate.host()))
            {
                Some(IdleHostAdmissionFailedFact::Retained(
                    retained.admission_failure_evidence(world, candidate.candidate.host()),
                ))
            } else {
                None
            };
            (host, detail)
        }
        IdleHostPhysicalTerminal::Residual {
            ledger, observed, ..
        } => (
            ledger
                .idle_host_authority_host()
                .expect("retained idle ledger owns its host"),
            Some(IdleHostAdmissionFailedFact::Residual {
                expected: Some(observed.clone()),
                observed: ledger.observe_idle_host_residual(world),
            }),
        ),
        IdleHostPhysicalTerminal::Unreserved { physical, .. } => (
            &physical.host,
            Some(IdleHostAdmissionFailedFact::Residual {
                expected: None,
                observed: physical
                    .frozen
                    .observe_idle_host_residual(world, &physical.host),
            }),
        ),
    };
    Some(IdleHostAdmissionFailure {
        version: host.version.clone(),
        slot: host.slot_id,
        expected: terminal.terminal.physical_facts(),
        detail: detail.unwrap_or_else(|| IdleHostAdmissionFailedFact::HostEdge {
            host: host.host,
            top: host.top_slot,
            parent: world.get::<ChildOf>(host.top_slot).map(ChildOf::parent),
            children: world
                .get::<Children>(host.host)
                .map(|children| children.iter().copied().collect())
                .unwrap_or_default(),
            expected_incarnation: host.registration_incarnation,
            candidate_host: candidate.candidate.host(),
            candidate_incarnation: candidate.registration_incarnation,
            terminal_matches: terminal.terminal.matches_host(&host.version, host.slot_id),
        }),
    })
}

pub(super) fn observe(world: &mut World, entry: &mut RouterEntry) -> bool {
    if entry.idle_host_loss.is_some() {
        drive(world, entry);
        return true;
    }
    if !physically_idle(entry) {
        return false;
    }
    let Some(host) = entry.host.as_ref() else {
        return false;
    };
    if world.get_entity(host.host).is_ok()
        && world
            .get::<RouterHostRegistration>(host.host)
            .is_some_and(|registration| {
                registration.router() == entry.handle
                    && registration.incarnation() == host.registration_incarnation
            })
    {
        return false;
    }
    let registry = world.resource::<RouteEntityRegistry>();
    let Ok(frozen) = FrozenCleanupManifest::from_idle_host_authority(
        world,
        registry,
        entry.active.as_ref(),
        host,
    ) else {
        return true;
    };
    let Some(partition) = frozen.freeze_back_no_target_authority_partition(world, registry, host)
    else {
        return true;
    };
    let manifest = CleanupManifest::new(
        entry
            .active
            .as_ref()
            .map(|active| {
                active
                    .frames()
                    .iter()
                    .rev()
                    .map(|frame| frame.frame().clone())
                    .collect()
            })
            .unwrap_or_default(),
        frozen.digest(),
        frozen.len(),
    );
    let targets = frozen.obligation_entities();
    let mut claims = entry
        .active
        .as_ref()
        .map(|active| active.claims().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    claims.push(host.top_slot_claim.clone());
    let isolation =
        match authority_isolation::prepare_idle_host_isolation(world, claims.iter(), targets) {
            authority_isolation::AuthorityIsolationPreparation::Ready(isolation) => isolation,
            authority_isolation::AuthorityIsolationPreparation::RetryableCapacity => return true,
            authority_isolation::AuthorityIsolationPreparation::PermanentContradiction {
                fail_closed,
                ..
            } => fail_closed,
        };
    isolation.apply(world);
    if !partition.refreeze_unstarted(world) {
        return true;
    }
    let host_children = world
        .get::<Children>(host.host)
        .map(|children| children.iter().copied().collect())
        .unwrap_or_default();
    let (host, lease) = entry
        .host
        .take()
        .expect("observed exact host owner")
        .into_retired();
    let active = entry.active.take();
    let mut physical = Some(UnreservedIdleHost {
        host,
        active,
        frozen,
        partition,
        host_children,
    });
    let pending = &mut entry.idle_host_loss;
    let result = entry.engine.begin_idle_host_loss(lease, &manifest, |core| {
        *pending = Some(IdleHostLoss::Reserving(ReservingIdleHostLoss {
            core,
            physical: physical.take().expect("single physical owner installation"),
            permanent: None,
        }));
    });
    if let Err((_, lease)) = result {
        let physical = physical
            .take()
            .expect("rejected begin returns physical ownership");
        entry.host = Some(physical.host.restore(lease));
        entry.active = physical.active;
        return true;
    }
    drive(world, entry);
    true
}

fn drive(world: &mut World, entry: &mut RouterEntry) {
    let Some(pending) = entry.idle_host_loss.take() else {
        return;
    };
    let mut pending = match pending {
        IdleHostLoss::Settling(pending) => pending,
        IdleHostLoss::Reserving(pending) => {
            if pending.permanent.is_some() {
                entry.idle_host_loss = Some(IdleHostLoss::Reserving(pending));
                return;
            }
            let ReservingIdleHostLoss { core, physical, .. } = pending;
            let UnreservedIdleHost {
                active,
                host,
                frozen,
                partition,
                host_children,
            } = physical;
            let manifest = CleanupManifest::new(Vec::new(), frozen.digest(), frozen.len());
            match CleanupLedger::reserve(frozen) {
                Ok(reserved) => SettlingIdleHostLoss {
                    core,
                    ledger: CleanupLedger::activate_idle_host_authority(
                        reserved,
                        active,
                        host,
                        host_children,
                        partition,
                    ),
                    manifest,
                    failure: None,
                    ready: None,
                    residual: None,
                },
                Err(error) => {
                    let kind = error.kind();
                    entry.idle_host_loss = Some(IdleHostLoss::Reserving(ReservingIdleHostLoss {
                        core,
                        physical: UnreservedIdleHost {
                            active,
                            host,
                            frozen: error.into_manifest(),
                            partition,
                            host_children,
                        },
                        permanent: (kind
                            != crate::cleanup::CleanupLedgerReserveErrorKind::RetryableCapacity)
                            .then_some(kind),
                    }));
                    return;
                }
            }
        }
    };
    if pending.residual.is_some() {
        entry.idle_host_loss = Some(IdleHostLoss::Settling(pending));
        return;
    }
    if pending.ready.is_some() {
        if pending
            .ledger
            .back_no_target_authority_partition_is_current(world)
        {
            entry.idle_host_loss = Some(IdleHostLoss::Settling(pending));
            return;
        }
        pending.ready = None;
        pending.failure = Some(
            pending
                .ledger
                .idle_host_failure(world, AuthorityCleanupFailureKind::Partition),
        );
    }
    if pending.failure.is_some() {
        prepare_residual(world, &mut pending);
        entry.idle_host_loss = Some(IdleHostLoss::Settling(pending));
        return;
    }
    let result = pending.ledger.settle_idle_host_authority(world);
    // These are actual ledger receipts, including any valid prefix preceding
    // a permanent stop. The batchless observation path does not touch requests.
    if !entry.observe_cleanup_receipts(&mut pending.ledger) {
        pending.failure = Some(
            pending
                .ledger
                .idle_host_failure(world, AuthorityCleanupFailureKind::TerminalProjection),
        );
        prepare_residual(world, &mut pending);
        entry.idle_host_loss = Some(IdleHostLoss::Settling(pending));
        return;
    }
    match result {
        Ok(facts) => {
            pending.ready = Some(facts);
            entry.idle_host_loss = Some(IdleHostLoss::Settling(pending));
        }
        Err(error) => {
            if error.kind != AuthorityCleanupFailureKind::RetryableCapacity {
                pending.failure = Some(error);
                prepare_residual(world, &mut pending);
            }
            entry.idle_host_loss = Some(IdleHostLoss::Settling(pending));
        }
    }
}

fn prepare_residual(world: &mut World, pending: &mut SettlingIdleHostLoss) {
    let Some(before) = pending.ledger.observe_idle_host_residual(world) else {
        return;
    };
    let targets = pending
        .ledger
        .exact_claims()
        .map(|claim| claim.entity())
        .collect();
    let isolation = authority_isolation::prepare_idle_host_isolation(
        world,
        pending.ledger.exact_claims(),
        targets,
    );
    match isolation {
        authority_isolation::AuthorityIsolationPreparation::Ready(prepared)
        | authority_isolation::AuthorityIsolationPreparation::PermanentContradiction {
            fail_closed: prepared,
            ..
        } => {
            let _ = prepared.apply_guarded(world, &|world| before.is_current(world));
        }
        authority_isolation::AuthorityIsolationPreparation::RetryableCapacity => return,
    }
    // This is an observation, not a fabricated cleanup receipt. A failed proof
    // remains explicitly unproven and cannot reopen admission.
    pending.residual = pending.ledger.observe_idle_host_residual(world);
}

// Resource-scope reinsertion can execute hooks. Refreeze after all observation
// scopes have exited, and publish only through the resident navigation resource.
pub(super) fn complete_ready(world: &mut World) {
    let count = world.resource::<RouterNavigation>().entries.len();
    for index in 0..count {
        let entry = &world.resource::<RouterNavigation>().entries[index];
        let Some(pending) = entry.idle_host_loss.as_ref() else {
            continue;
        };
        let (mut facts, mut residual, mut post_scope_failure) = match pending {
            IdleHostLoss::Reserving(pending) => {
                if pending.permanent.is_none() {
                    continue;
                }
                let Some(current) = pending
                    .physical
                    .frozen
                    .observe_idle_host_residual(world, &pending.physical.host)
                else {
                    continue;
                };
                (current.facts(), Some(current), None)
            }
            IdleHostLoss::Settling(pending) => {
                if pending.residual.is_some() {
                    let Some(current) = pending.ledger.observe_idle_host_residual(world) else {
                        continue;
                    };
                    (current.facts(), Some(current), None)
                } else {
                    let Some(facts) = pending.ready else {
                        continue;
                    };
                    let Some(host) = pending.ledger.idle_host_authority_host() else {
                        continue;
                    };
                    if !pending
                        .ledger
                        .back_no_target_authority_partition_is_current(world)
                        || world
                            .get::<ChildOf>(host.top_slot)
                            .is_some_and(|parent| parent.parent() == host.host)
                        || pending
                            .ledger
                            .observe_idle_host_residual(world)
                            .is_none_or(|observed| !observed.is_safe())
                    {
                        let Some(current) = pending.ledger.observe_idle_host_residual(world) else {
                            continue;
                        };
                        (
                            current.facts(),
                            Some(current),
                            Some(
                                pending.ledger.idle_host_failure(
                                    world,
                                    AuthorityCleanupFailureKind::Partition,
                                ),
                            ),
                        )
                    } else {
                        (
                            IdleHostLossPhysicalFacts {
                                manifest_digest: facts.manifest_digest(),
                                obligation_count: facts.obligation_count(),
                                disposition: IdleHostLossPhysicalDisposition::Settled {
                                    completion_digest: facts.completion_digest(),
                                    settled_count: facts.settled_count(),
                                },
                            },
                            None,
                            None,
                        )
                    }
                }
            }
        };
        let settled_observation = match pending {
            IdleHostLoss::Settling(pending)
                if matches!(
                    facts.disposition,
                    IdleHostLossPhysicalDisposition::Settled { .. }
                ) =>
            {
                pending.ledger.observe_idle_host_residual(world)
            }
            _ => None,
        };
        let retained = match pending {
            IdleHostLoss::Settling(pending)
                if matches!(
                    facts.disposition,
                    IdleHostLossPhysicalDisposition::Settled { .. }
                ) =>
            {
                match pending.ledger.prepare_idle_host_completed_retention(world) {
                    Ok(retained) => retained,
                    Err(AuthorityCleanupFailureKind::RetryableCapacity) => continue,
                    Err(kind) => {
                        let Some(current) = pending.ledger.observe_idle_host_residual(world) else {
                            continue;
                        };
                        facts = current.facts();
                        residual = Some(current);
                        post_scope_failure = Some(pending.ledger.idle_host_failure(world, kind));
                        None
                    }
                }
            }
            _ => None,
        };
        let keep_previous =
            entry
                .idle_host_terminal
                .as_ref()
                .is_some_and(|terminal| match &terminal.physical {
                    IdleHostPhysicalTerminal::Settled { retained, .. } => retained
                        .as_ref()
                        .is_some_and(|retained| retained.has_live_targets(world)),
                    _ => true,
                });
        let mut navigation = world.resource_mut::<RouterNavigation>();
        let entry = &mut navigation.entries[index];
        let pending = entry.idle_host_loss.take().expect("validated ready holder");
        // Allocate the retained responsibility link before Core completion's
        // final installation callback. Rejection still returns the original.
        let mut previous = entry.idle_host_terminal.take().map(Box::new);
        let terminal = &mut entry.idle_host_terminal;
        match pending {
            IdleHostLoss::Reserving(pending) => {
                let mut physical = Some(pending.physical);
                let failure = pending
                    .permanent
                    .expect("validated permanent reservation failure");
                let result = entry
                    .engine
                    .complete_idle_host_loss(pending.core, facts, |core| {
                        *terminal = Some(IdleHostTerminal {
                            physical: IdleHostPhysicalTerminal::Unreserved {
                                physical: physical.take().expect("single unreserved owner"),
                                failure,
                            },
                            terminal: core,
                            previous: retain_previous(previous.take(), keep_previous),
                        });
                    });
                if let Err(core) = result {
                    *terminal = previous.map(|previous| *previous);
                    entry.idle_host_loss = Some(IdleHostLoss::Reserving(ReservingIdleHostLoss {
                        core,
                        physical: physical
                            .take()
                            .expect("rejected reservation completion retains owner"),
                        permanent: Some(failure),
                    }));
                }
            }
            IdleHostLoss::Settling(pending) => {
                let mut ledger = Some(pending.ledger);
                let mut failure = pending.failure.or(post_scope_failure);
                let result = entry
                    .engine
                    .complete_idle_host_loss(pending.core, facts, |core| {
                        let physical = if let Some(observed) = residual {
                            IdleHostPhysicalTerminal::Residual {
                                ledger: ledger.take().expect("single residual owner"),
                                observed,
                                failure: failure.take().expect("permanent residual retains cause"),
                            }
                        } else {
                            let owner = ledger
                                .as_mut()
                                .expect("physical owner remains installed")
                                .take_idle_host_authority_owner()
                                .expect("idle ledger owns exact retired host");
                            IdleHostPhysicalTerminal::Settled {
                                host: owner.host,
                                observed: settled_observation
                                    .expect("validated settled input facts"),
                                retained,
                            }
                        };
                        *terminal = Some(IdleHostTerminal {
                            physical,
                            terminal: core,
                            previous: retain_previous(previous.take(), keep_previous),
                        });
                    });
                if let Err(core) = result {
                    *terminal = previous.map(|previous| *previous);
                    entry.idle_host_loss = Some(IdleHostLoss::Settling(SettlingIdleHostLoss {
                        core,
                        ledger: ledger.take().expect("rejected completion retains ledger"),
                        manifest: pending.manifest,
                        failure,
                        ready: pending.ready,
                        residual: pending.residual,
                    }));
                }
            }
        }
    }
}

fn retain_previous(
    previous: Option<Box<IdleHostTerminal>>,
    keep: bool,
) -> Option<Box<IdleHostTerminal>> {
    previous.and_then(|previous| {
        if keep {
            Some(previous)
        } else {
            previous.previous
        }
    })
}
