use bevy_camera::visibility::Visibility;
use bevy_ecs::{
    component::ComponentId,
    entity::Entity,
    hierarchy::{ChildOf, Children},
    prelude::World,
};
use bevy_input_focus::InputFocus;
use bevy_picking::Pickable;

use crate::{
    cleanup::{cleanup_claim_has_exact_authority, cleanup_record_has_exact_authority},
    components::RouterInputBarrier,
    input::{RouteFocusState, RouterInputGate},
};

use super::{
    ComponentManifest, QuarantineBarrierKind, QuarantineBoundaryObservation, QuarantineLease,
    QuarantineLeaseRecord, QuarantineLeaseTable, QuarantineReapAttempt, QuarantineReapBlockReason,
    QuarantineReapEvidence, QuarantineReapOutcome, QuarantineReapPostwrite,
    QuarantineReapProjectionPhase, try_clone_quarantine_lease_for_reap,
    try_clone_quarantine_lease_for_reap_with_record_capacity,
};

#[cfg(test)]
use super::{
    QuarantineFacts, QuarantineRootProjection, RouteEntityRecord, quarantine_root_record,
    try_component_difference,
};

#[cfg(test)]
thread_local! {
    static FAIL_NEXT_CANDIDATE_CLONE_RESERVATION: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FAIL_NEXT_INITIAL_PREPARATION_RESERVATION: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FAIL_NEXT_POSTWRITE_PREPARATION_RESERVATION: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FAIL_NEXT_POSTWRITE_REFREEZE_RESERVATION: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn initial_preparation_clone_record_capacity(actual: usize) -> usize {
    #[cfg(test)]
    if FAIL_NEXT_INITIAL_PREPARATION_RESERVATION.replace(false) {
        usize::MAX
    } else {
        actual
    }
    #[cfg(not(test))]
    {
        actual
    }
}

fn candidate_clone_record_capacity(actual: usize) -> usize {
    #[cfg(test)]
    if FAIL_NEXT_CANDIDATE_CLONE_RESERVATION.replace(false) {
        usize::MAX
    } else {
        actual
    }
    #[cfg(not(test))]
    {
        actual
    }
}

fn postwrite_clone_record_capacity(actual: usize, refreeze: bool) -> usize {
    #[cfg(test)]
    if if refreeze {
        FAIL_NEXT_POSTWRITE_REFREEZE_RESERVATION.replace(false)
    } else {
        FAIL_NEXT_POSTWRITE_PREPARATION_RESERVATION.replace(false)
    } {
        usize::MAX
    } else {
        actual
    }
    #[cfg(not(test))]
    {
        let _ = refreeze;
        actual
    }
}

enum ReapDisposition {
    Blocked {
        target: Option<super::ExactRegistryClaim>,
        reason: QuarantineReapBlockReason,
    },
    Terminal {
        target: super::ExactRegistryClaim,
    },
    Despawn {
        target: super::ExactRegistryClaim,
    },
}

struct PreparedQuarantineReap {
    lease: QuarantineLease,
    exact_entities: Vec<Entity>,
    focus_targets: Vec<Entity>,
    observations: Vec<QuarantineBoundaryObservation>,
    disposition: ReapDisposition,
}

#[derive(Clone, Copy)]
struct QuarantineReapCandidateIdentity {
    id: super::QuarantineLeaseId,
    generation: u64,
    operation_sequence: u64,
}

fn reap_source_matches(current: &QuarantineLease, candidate: &QuarantineLease) -> bool {
    current.id == candidate.id
        && current.generation == candidate.generation
        && current.operation_sequence == candidate.operation_sequence
        && current.root == candidate.root
        && current.entity_count == candidate.entity_count
        && current.records == candidate.records
        && current.facts == candidate.facts
}

fn outcome_evidence(outcome: &QuarantineReapOutcome) -> &QuarantineReapEvidence {
    match outcome {
        QuarantineReapOutcome::Blocked { evidence, .. }
        | QuarantineReapOutcome::Terminal { evidence, .. }
        | QuarantineReapOutcome::Despawned { evidence, .. }
        | QuarantineReapOutcome::DespawnRejected { evidence, .. } => evidence,
    }
}

fn evidence_matches_attempt(
    evidence: &QuarantineReapEvidence,
    candidate: &QuarantineLease,
    attempt: &QuarantineReapAttempt,
) -> bool {
    let QuarantineReapEvidence::Observed(observations) = evidence else {
        return true;
    };
    !observations.is_empty()
        && observations.iter().all(|observation| {
            observation.lease_id == attempt.lease_id
                && observation.generation == attempt.generation
                && observation.operation_sequence == attempt.operation_sequence
                && candidate
                    .records
                    .iter()
                    .any(|record| record.record().matches_claim(&observation.target))
        })
}

fn evidence_proves_terminal(
    evidence: &QuarantineReapEvidence,
    target: &super::ExactRegistryClaim,
    despawned: bool,
) -> bool {
    let QuarantineReapEvidence::Observed(observations) = evidence else {
        return false;
    };
    observations.iter().any(|observation| {
        observation.target == *target
            && if despawned {
                !observation.authority.entity_present
            } else {
                !observation.exact_authority
            }
    })
}

impl QuarantineLeaseTable {
    fn next_reap_candidate_identity(&self) -> Option<QuarantineReapCandidateIdentity> {
        let selected = self
            .leases
            .iter()
            .filter(|lease| {
                self.reap_cursor
                    .is_none_or(|cursor| lease.id.sequence > cursor.sequence)
            })
            .min_by_key(|lease| lease.id.sequence)
            .or_else(|| self.leases.iter().min_by_key(|lease| lease.id.sequence));
        selected.map(|lease| QuarantineReapCandidateIdentity {
            id: lease.id,
            generation: lease.generation,
            operation_sequence: lease.operation_sequence,
        })
    }

    fn try_clone_reap_candidate(
        &self,
        identity: QuarantineReapCandidateIdentity,
    ) -> Result<Option<QuarantineLease>, ()> {
        let Some(source) = self.leases.iter().find(|lease| {
            lease.id == identity.id
                && lease.generation == identity.generation
                && lease.operation_sequence == identity.operation_sequence
        }) else {
            return Ok(None);
        };
        let requested_capacity = candidate_clone_record_capacity(source.records.len());
        try_clone_quarantine_lease_for_reap_with_record_capacity(source, requested_capacity)
            .map(Some)
    }

    fn candidate_is_current(&self, candidate: &QuarantineLease) -> bool {
        self.leases
            .iter()
            .any(|lease| reap_source_matches(lease, candidate))
    }

    fn settle_reap_attempt(
        &mut self,
        candidate: &QuarantineLease,
        attempt: QuarantineReapAttempt,
    ) -> bool {
        let Some(index) = self
            .leases
            .iter()
            .position(|lease| reap_source_matches(lease, candidate))
        else {
            return false;
        };
        let Some(next_operation) = self.leases[index].operation_sequence.checked_add(1) else {
            return false;
        };
        if attempt.lease_id != candidate.id
            || attempt.generation != candidate.generation
            || attempt.operation_sequence != next_operation
        {
            return false;
        }
        if !evidence_matches_attempt(outcome_evidence(&attempt.outcome), candidate, &attempt) {
            return false;
        }
        let target = match &attempt.outcome {
            QuarantineReapOutcome::Blocked { target, .. } => target.as_ref(),
            QuarantineReapOutcome::Terminal { target, .. }
            | QuarantineReapOutcome::Despawned { target, .. }
            | QuarantineReapOutcome::DespawnRejected { target, .. } => Some(target),
        };
        if target.is_some_and(|target| {
            !candidate
                .records
                .iter()
                .any(|record| record.record().matches_claim(target))
        }) {
            return false;
        }
        let terminal = match &attempt.outcome {
            QuarantineReapOutcome::Terminal {
                target, evidence, ..
            } if evidence_proves_terminal(evidence, target, false) => Some(target.entity()),
            QuarantineReapOutcome::Despawned {
                target, evidence, ..
            } if evidence_proves_terminal(evidence, target, true) => Some(target.entity()),
            QuarantineReapOutcome::Blocked { .. }
            | QuarantineReapOutcome::Terminal { .. }
            | QuarantineReapOutcome::Despawned { .. }
            | QuarantineReapOutcome::DespawnRejected { .. } => None,
        };
        self.reap_cursor = Some(candidate.id);
        let lease = &mut self.leases[index];
        lease.operation_sequence = next_operation;
        if let Some(entity) = terminal {
            lease.records.retain(|record| record.entity() != entity);
        }
        lease.last_attempt = Some(attempt);
        if lease.records.is_empty() {
            let closed = self.leases.remove(index);
            self.last_closed_reap_attempt = closed.last_attempt;
        }
        true
    }

    fn settle_candidate_clone_failure(
        &mut self,
        identity: QuarantineReapCandidateIdentity,
    ) -> bool {
        let Some(index) = self.leases.iter().position(|lease| {
            lease.id == identity.id
                && lease.generation == identity.generation
                && lease.operation_sequence == identity.operation_sequence
        }) else {
            return false;
        };
        let Some(operation_sequence) = identity.operation_sequence.checked_add(1) else {
            return false;
        };
        let phase = QuarantineReapProjectionPhase::InitialPreparation;
        let attempt = QuarantineReapAttempt {
            lease_id: identity.id,
            generation: identity.generation,
            operation_sequence,
            outcome: QuarantineReapOutcome::Blocked {
                target: None,
                reason: QuarantineReapBlockReason::ProjectionUnavailable { phase },
                evidence: QuarantineReapEvidence::Unavailable { phase },
            },
        };
        self.reap_cursor = Some(identity.id);
        self.leases[index].operation_sequence = operation_sequence;
        self.leases[index].last_attempt = Some(attempt);
        true
    }
}

fn try_push_unique(values: &mut Vec<Entity>, entity: Entity) -> Result<(), ()> {
    if values.contains(&entity) {
        return Ok(());
    }
    values.try_reserve(1).map_err(|_| ())?;
    values.push(entity);
    Ok(())
}

fn try_children(world: &World, entity: Entity) -> Result<Vec<Entity>, ()> {
    let Some(children) = world.get::<Children>(entity) else {
        return Ok(Vec::new());
    };
    let mut result = Vec::<Entity>::new();
    result.try_reserve_exact(children.len()).map_err(|_| ())?;
    result.extend(children.iter());
    result.sort_by_key(|child| child.to_bits());
    result.dedup();
    Ok(result)
}

fn try_focus_closure(world: &World, roots: &[Entity]) -> Result<Vec<Entity>, ()> {
    let mut stack = Vec::new();
    stack.try_reserve_exact(roots.len()).map_err(|_| ())?;
    stack.extend_from_slice(roots);
    stack.sort_by_key(|entity| entity.to_bits());
    stack.reverse();
    let mut visited = Vec::new();
    while let Some(entity) = stack.pop() {
        if visited.contains(&entity) {
            continue;
        }
        try_push_unique(&mut visited, entity)?;
        let mut children = try_children(world, entity)?;
        children.reverse();
        stack.try_reserve(children.len()).map_err(|_| ())?;
        stack.extend(children);
    }
    visited.sort_by_key(|entity| entity.to_bits());
    Ok(visited)
}

fn manifest_is_reap_safe(
    actual: &ComponentManifest,
    record: &QuarantineLeaseRecord,
    relationship_components: &[Option<ComponentId>; 2],
) -> bool {
    let actual = actual.component_ids();
    let expected = record.record().manifest().component_ids();
    actual.iter().all(|component| {
        expected.contains(component)
            || relationship_components
                .iter()
                .flatten()
                .any(|relationship| relationship == component)
    }) && expected.iter().all(|component| {
        actual.contains(component)
            || relationship_components
                .iter()
                .flatten()
                .any(|relationship| relationship == component)
    })
}

fn component_ids_are_stable(values: &[ComponentId]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn try_boundary_observations(
    world: &World,
    lease: &QuarantineLease,
    exact_entities: &[Entity],
    focus_targets: &[Entity],
) -> Result<Vec<QuarantineBoundaryObservation>, ()> {
    let mut observations = Vec::new();
    observations
        .try_reserve_exact(lease.records.len())
        .map_err(|_| ())?;
    for record in &lease.records {
        observations.push(QuarantineBoundaryObservation::try_from_world(
            world,
            lease,
            record,
            exact_entities,
            focus_targets,
        )?);
    }
    Ok(observations)
}

fn try_current_partition_observations(
    world: &World,
    lease: &QuarantineLease,
) -> Result<Vec<QuarantineBoundaryObservation>, ()> {
    let mut exact_entities = Vec::new();
    exact_entities
        .try_reserve_exact(lease.records.len())
        .map_err(|_| ())?;
    for record in &lease.records {
        let claim = record.try_claim()?;
        if cleanup_claim_has_exact_authority(world, &claim) {
            exact_entities.push(record.entity());
        }
    }
    exact_entities.sort_by_key(|entity| entity.to_bits());
    let focus_targets = try_focus_closure(world, &exact_entities)?;
    world.get_resource::<RouterInputGate>().ok_or(())?;
    try_boundary_observations(world, lease, &exact_entities, &focus_targets)
}

fn try_prepare_reap(world: &World, lease: QuarantineLease) -> Result<PreparedQuarantineReap, ()> {
    if !component_ids_are_stable(&lease.facts.foreign_components)
        || !component_ids_are_stable(&lease.facts.missing_components)
        || lease.records.iter().any(|record| {
            !component_ids_are_stable(record.foreign_components())
                || !component_ids_are_stable(record.missing_components())
        })
    {
        return Err(());
    }
    let mut exact_entities = Vec::new();
    let mut terminal_records = Vec::new();
    exact_entities
        .try_reserve_exact(lease.records.len())
        .map_err(|_| ())?;
    terminal_records
        .try_reserve_exact(lease.records.len())
        .map_err(|_| ())?;
    for record in &lease.records {
        let target = record.try_claim()?;
        if world.get_entity(record.entity()).is_err()
            || !cleanup_claim_has_exact_authority(world, &target)
        {
            terminal_records.push((record.entity(), target));
        } else {
            exact_entities.push(record.entity());
        }
    }
    exact_entities.sort_by_key(|entity| entity.to_bits());
    terminal_records.sort_by_key(|(entity, _)| entity.to_bits());

    let focus_targets = try_focus_closure(world, &exact_entities)?;
    world.get_resource::<RouterInputGate>().ok_or(())?;
    let observations = try_boundary_observations(world, &lease, &exact_entities, &focus_targets)?;

    let disposition =
        if let Some((_, target)) = terminal_records.into_iter().next() {
            ReapDisposition::Terminal { target }
        } else {
            let relationship_components = [
                world.components().component_id::<ChildOf>(),
                world.components().component_id::<Children>(),
            ];
            let mut blocked = None;
            let mut leaf = None;
            for record in &lease.records {
                let actual = ComponentManifest::try_from_world(world, record.entity())?.ok_or(())?;
                let target = record.try_claim()?;
                if world.get::<Visibility>(record.entity()).is_none() && blocked.is_none() {
                    blocked = Some((
                        Some(super::try_clone_exact_registry_claim(&target)?),
                        QuarantineReapBlockReason::MissingBarrier {
                            entity: record.entity(),
                            barrier: QuarantineBarrierKind::Visibility,
                        },
                    ));
                }
                if world.get::<Pickable>(record.entity()).is_none() && blocked.is_none() {
                    blocked = Some((
                        Some(super::try_clone_exact_registry_claim(&target)?),
                        QuarantineReapBlockReason::MissingBarrier {
                            entity: record.entity(),
                            barrier: QuarantineBarrierKind::Pickable,
                        },
                    ));
                }
                if world.get::<RouterInputBarrier>(record.entity()).is_none() && blocked.is_none() {
                    blocked = Some((
                        Some(super::try_clone_exact_registry_claim(&target)?),
                        QuarantineReapBlockReason::MissingBarrier {
                            entity: record.entity(),
                            barrier: QuarantineBarrierKind::Input,
                        },
                    ));
                }
                if !manifest_is_reap_safe(&actual, record, &relationship_components)
                    && blocked.is_none()
                {
                    blocked = Some((
                        Some(super::try_clone_exact_registry_claim(&target)?),
                        QuarantineReapBlockReason::ManifestDrift {
                            entity: record.entity(),
                            expected: record.record().manifest().try_clone()?,
                            observed: actual.try_clone()?,
                        },
                    ));
                }

                if let Some(parent) = world.get::<ChildOf>(record.entity()).map(ChildOf::parent)
                    && !exact_entities.contains(&parent)
                    && world.get_entity(parent).is_ok()
                    && blocked.is_none()
                {
                    blocked = Some((
                        Some(super::try_clone_exact_registry_claim(&target)?),
                        QuarantineReapBlockReason::ProtectedParent {
                            entity: record.entity(),
                            parent,
                        },
                    ));
                }

                let children = try_children(world, record.entity())?;
                if let Some(child) = children.iter().copied().find(|child| {
                    !exact_entities.contains(child) && world.get_entity(*child).is_ok()
                }) && blocked.is_none()
                {
                    blocked = Some((
                        Some(super::try_clone_exact_registry_claim(&target)?),
                        QuarantineReapBlockReason::ForeignChild {
                            entity: record.entity(),
                            child,
                        },
                    ));
                }
                if children
                    .iter()
                    .all(|child| world.get_entity(*child).is_err())
                    && leaf.is_none()
                {
                    leaf = Some(target);
                }
            }
            if let Some((target, reason)) = blocked {
                ReapDisposition::Blocked { target, reason }
            } else if let Some(target) = leaf {
                ReapDisposition::Despawn { target }
            } else {
                ReapDisposition::Blocked {
                    target: None,
                    reason: QuarantineReapBlockReason::NoOwnedLeaf,
                }
            }
        };

    Ok(PreparedQuarantineReap {
        lease,
        exact_entities,
        focus_targets,
        observations,
        disposition,
    })
}

fn lease_record_is_current_exact(
    world: &World,
    lease: &QuarantineLease,
    entity: Entity,
    excluded: Option<Entity>,
) -> bool {
    excluded != Some(entity)
        && lease.records.iter().any(|record| {
            record.entity() == entity && cleanup_record_has_exact_authority(world, record.record())
        })
}

fn is_in_current_exact_subtree(
    world: &World,
    focused: Entity,
    lease: &QuarantineLease,
    excluded: Option<Entity>,
) -> bool {
    let mut current = focused;
    for _ in 0..=world.entities().len() {
        if lease_record_is_current_exact(world, lease, current, excluded) {
            return true;
        }
        let Some(parent) = world.get::<ChildOf>(current).map(ChildOf::parent) else {
            return false;
        };
        current = parent;
    }
    false
}

fn apply_current_exact_inert_barrier(
    world: &mut World,
    lease: &QuarantineLease,
    excluded: Option<Entity>,
) {
    for record in &lease.records {
        let entity = record.entity();
        if lease_record_is_current_exact(world, lease, entity, excluded)
            && let Some(mut gate) = world.get_resource_mut::<RouterInputGate>()
        {
            gate.isolate_targets_in_place(&[entity]);
        }
    }
    let input_focus_is_owned = world
        .get_resource::<InputFocus>()
        .and_then(InputFocus::get)
        .is_some_and(|focused| is_in_current_exact_subtree(world, focused, lease, excluded));
    if input_focus_is_owned && let Some(mut focus) = world.get_resource_mut::<InputFocus>() {
        *focus = InputFocus::default();
    }
    let route_focus_is_owned = world
        .get_resource::<RouteFocusState>()
        .and_then(RouteFocusState::focused)
        .is_some_and(|focused| is_in_current_exact_subtree(world, focused, lease, excluded));
    if route_focus_is_owned && let Some(mut focus) = world.get_resource_mut::<RouteFocusState>() {
        focus.set_focused(None);
    }
    for record in &lease.records {
        let entity = record.entity();
        if !lease_record_is_current_exact(world, lease, entity, excluded) {
            continue;
        }
        if let Some(mut visibility) = world.get_mut::<Visibility>(entity) {
            *visibility = Visibility::Hidden;
        }
        if let Some(mut pickable) = world.get_mut::<Pickable>(entity) {
            *pickable = Pickable::IGNORE;
        }
        if let Some(mut barrier) = world.get_mut::<RouterInputBarrier>(entity) {
            *barrier = RouterInputBarrier::deny_all();
        }
    }
}

fn apply_inert_barrier(world: &mut World, prepared: &PreparedQuarantineReap) {
    apply_current_exact_inert_barrier(world, &prepared.lease, None);
}

fn identity_record_count(
    world: &World,
    identity: QuarantineReapCandidateIdentity,
) -> Option<usize> {
    world
        .get_resource::<QuarantineLeaseTable>()?
        .leases
        .iter()
        .find(|lease| {
            lease.id == identity.id
                && lease.generation == identity.generation
                && lease.operation_sequence == identity.operation_sequence
        })
        .map(|lease| lease.records.len())
}

fn identity_record_is_current_exact(
    world: &World,
    identity: QuarantineReapCandidateIdentity,
    index: usize,
) -> Option<Entity> {
    let table = world.get_resource::<QuarantineLeaseTable>()?;
    let lease = table.leases.iter().find(|lease| {
        lease.id == identity.id
            && lease.generation == identity.generation
            && lease.operation_sequence == identity.operation_sequence
    })?;
    let record = lease.records.get(index)?;
    cleanup_record_has_exact_authority(world, record.record()).then_some(record.entity())
}

fn is_in_current_identity_subtree(
    world: &World,
    focused: Entity,
    identity: QuarantineReapCandidateIdentity,
    record_count: usize,
) -> bool {
    let mut current = focused;
    for _ in 0..=world.entities().len() {
        if (0..record_count)
            .any(|index| identity_record_is_current_exact(world, identity, index) == Some(current))
        {
            return true;
        }
        let Some(parent) = world.get::<ChildOf>(current).map(ChildOf::parent) else {
            return false;
        };
        current = parent;
    }
    false
}

fn apply_current_identity_inert_barrier(
    world: &mut World,
    identity: QuarantineReapCandidateIdentity,
) {
    let Some(record_count) = identity_record_count(world, identity) else {
        return;
    };
    for index in 0..record_count {
        if let Some(entity) = identity_record_is_current_exact(world, identity, index)
            && let Some(mut gate) = world.get_resource_mut::<RouterInputGate>()
        {
            gate.isolate_targets_in_place(&[entity]);
        }
    }
    let input_focus_is_owned = world
        .get_resource::<InputFocus>()
        .and_then(InputFocus::get)
        .is_some_and(|focused| {
            is_in_current_identity_subtree(world, focused, identity, record_count)
        });
    if input_focus_is_owned && let Some(mut focus) = world.get_resource_mut::<InputFocus>() {
        *focus = InputFocus::default();
    }
    let route_focus_is_owned = world
        .get_resource::<RouteFocusState>()
        .and_then(RouteFocusState::focused)
        .is_some_and(|focused| {
            is_in_current_identity_subtree(world, focused, identity, record_count)
        });
    if route_focus_is_owned && let Some(mut focus) = world.get_resource_mut::<RouteFocusState>() {
        focus.set_focused(None);
    }
    for index in 0..record_count {
        let Some(entity) = identity_record_is_current_exact(world, identity, index) else {
            continue;
        };
        if let Some(mut visibility) = world.get_mut::<Visibility>(entity) {
            *visibility = Visibility::Hidden;
        }
        if let Some(mut pickable) = world.get_mut::<Pickable>(entity) {
            *pickable = Pickable::IGNORE;
        }
        if let Some(mut barrier) = world.get_mut::<RouterInputBarrier>(entity) {
            *barrier = RouterInputBarrier::deny_all();
        }
    }
}

fn postwrite_refreeze(
    world: &mut World,
    candidate: &QuarantineLease,
    removed: Entity,
) -> QuarantineReapPostwrite {
    apply_current_exact_inert_barrier(world, candidate, Some(removed));
    world.flush();
    let requested_capacity = postwrite_clone_record_capacity(candidate.records.len(), false);
    let mut remaining = match try_clone_quarantine_lease_for_reap_with_record_capacity(
        candidate,
        requested_capacity,
    ) {
        Ok(remaining) => remaining,
        Err(()) => {
            apply_current_exact_inert_barrier(world, candidate, Some(removed));
            world.flush();
            return QuarantineReapPostwrite::Superseded {
                target: None,
                reason: QuarantineReapBlockReason::ProjectionUnavailable {
                    phase: QuarantineReapProjectionPhase::PostwritePreparation,
                },
                evidence: QuarantineReapEvidence::Unavailable {
                    phase: QuarantineReapProjectionPhase::PostwritePreparation,
                },
            };
        }
    };
    remaining
        .records
        .retain(|record| record.entity() != removed);
    if remaining.records.is_empty() {
        return QuarantineReapPostwrite::Refrozen {
            survivors: Vec::new(),
        };
    }
    let prepared = match try_prepare_reap(world, remaining) {
        Ok(prepared) => prepared,
        Err(()) => {
            apply_current_exact_inert_barrier(world, candidate, Some(removed));
            world.flush();
            return QuarantineReapPostwrite::Superseded {
                target: None,
                reason: QuarantineReapBlockReason::ProjectionUnavailable {
                    phase: QuarantineReapProjectionPhase::PostwritePreparation,
                },
                evidence: QuarantineReapEvidence::Unavailable {
                    phase: QuarantineReapProjectionPhase::PostwritePreparation,
                },
            };
        }
    };
    apply_inert_barrier(world, &prepared);
    world.flush();
    let requested_capacity = postwrite_clone_record_capacity(prepared.lease.records.len(), true);
    let refrozen = match try_clone_quarantine_lease_for_reap_with_record_capacity(
        &prepared.lease,
        requested_capacity,
    )
    .and_then(|lease| try_prepare_reap(world, lease))
    {
        Ok(refrozen) => refrozen,
        Err(()) => {
            apply_current_exact_inert_barrier(world, &prepared.lease, None);
            world.flush();
            return QuarantineReapPostwrite::Superseded {
                target: None,
                reason: QuarantineReapBlockReason::ProjectionUnavailable {
                    phase: QuarantineReapProjectionPhase::PostwriteRefreeze,
                },
                evidence: QuarantineReapEvidence::Unavailable {
                    phase: QuarantineReapProjectionPhase::PostwriteRefreeze,
                },
            };
        }
    };
    match refrozen {
        PreparedQuarantineReap {
            disposition: ReapDisposition::Despawn { .. },
            observations,
            ..
        } => QuarantineReapPostwrite::Refrozen {
            survivors: observations,
        },
        PreparedQuarantineReap {
            disposition: ReapDisposition::Blocked { target, reason },
            observations,
            ..
        } => QuarantineReapPostwrite::Superseded {
            target,
            reason,
            evidence: QuarantineReapEvidence::Observed(observations),
        },
        PreparedQuarantineReap {
            disposition: ReapDisposition::Terminal { target },
            observations,
            ..
        } => {
            let entity = target.entity();
            QuarantineReapPostwrite::Superseded {
                target: Some(target),
                reason: QuarantineReapBlockReason::AuthorityDrift { entity },
                evidence: QuarantineReapEvidence::Observed(observations),
            }
        }
    }
}

fn settle_projection_unavailable(
    world: &mut World,
    candidate: &QuarantineLease,
    phase: QuarantineReapProjectionPhase,
) {
    let Some(operation_sequence) = candidate.operation_sequence.checked_add(1) else {
        return;
    };
    let attempt = QuarantineReapAttempt {
        lease_id: candidate.id,
        generation: candidate.generation,
        operation_sequence,
        outcome: QuarantineReapOutcome::Blocked {
            target: None,
            reason: QuarantineReapBlockReason::ProjectionUnavailable { phase },
            evidence: QuarantineReapEvidence::Unavailable { phase },
        },
    };
    world
        .resource_mut::<QuarantineLeaseTable>()
        .settle_reap_attempt(candidate, attempt);
}

pub(crate) fn reap_quarantines_system(world: &mut World) {
    let Some(identity) = world
        .get_resource::<QuarantineLeaseTable>()
        .and_then(QuarantineLeaseTable::next_reap_candidate_identity)
    else {
        return;
    };
    let candidate = match world
        .resource::<QuarantineLeaseTable>()
        .try_clone_reap_candidate(identity)
    {
        Ok(Some(candidate)) => candidate,
        Ok(None) => return,
        Err(()) => {
            apply_current_identity_inert_barrier(world, identity);
            world.flush();
            world
                .resource_mut::<QuarantineLeaseTable>()
                .settle_candidate_clone_failure(identity);
            return;
        }
    };
    let requested_capacity = initial_preparation_clone_record_capacity(candidate.records.len());
    let prepared = match try_clone_quarantine_lease_for_reap_with_record_capacity(
        &candidate,
        requested_capacity,
    )
    .and_then(|candidate| try_prepare_reap(world, candidate))
    {
        Ok(prepared) => prepared,
        Err(()) => {
            apply_current_exact_inert_barrier(world, &candidate, None);
            world.flush();
            settle_projection_unavailable(
                world,
                &candidate,
                QuarantineReapProjectionPhase::InitialPreparation,
            );
            return;
        }
    };
    if !world
        .resource::<QuarantineLeaseTable>()
        .candidate_is_current(&prepared.lease)
    {
        return;
    }

    apply_inert_barrier(world, &prepared);
    world.flush();
    let refreshed = match try_clone_quarantine_lease_for_reap(&prepared.lease)
        .and_then(|lease| try_prepare_reap(world, lease))
    {
        Ok(refreshed) => refreshed,
        Err(()) => {
            apply_current_exact_inert_barrier(world, &prepared.lease, None);
            world.flush();
            settle_projection_unavailable(
                world,
                &prepared.lease,
                QuarantineReapProjectionPhase::PrewriteRefreeze,
            );
            return;
        }
    };
    if !world
        .resource::<QuarantineLeaseTable>()
        .candidate_is_current(&refreshed.lease)
    {
        return;
    }
    let Some(operation_sequence) = refreshed.lease.operation_sequence.checked_add(1) else {
        return;
    };
    let observations = QuarantineReapEvidence::Observed(refreshed.observations);
    let outcome = match refreshed.disposition {
        ReapDisposition::Blocked { target, reason } => QuarantineReapOutcome::Blocked {
            target,
            reason,
            evidence: observations,
        },
        ReapDisposition::Terminal { target } => QuarantineReapOutcome::Terminal {
            target,
            evidence: observations,
        },
        ReapDisposition::Despawn { target } => {
            let entity = target.entity();
            let _ = world.despawn(entity);
            world.flush();
            let evidence = match try_current_partition_observations(world, &refreshed.lease) {
                Ok(observed) => QuarantineReapEvidence::Observed(observed),
                Err(()) => QuarantineReapEvidence::Unavailable {
                    phase: QuarantineReapProjectionPhase::PostdespawnObservation,
                },
            };
            if world.get_entity(entity).is_err() {
                QuarantineReapOutcome::Despawned {
                    target,
                    evidence,
                    postwrite: postwrite_refreeze(world, &refreshed.lease, entity),
                }
            } else {
                QuarantineReapOutcome::DespawnRejected { target, evidence }
            }
        }
    };
    let attempt = QuarantineReapAttempt {
        lease_id: refreshed.lease.id,
        generation: refreshed.lease.generation,
        operation_sequence,
        outcome,
    };
    world
        .resource_mut::<QuarantineLeaseTable>()
        .settle_reap_attempt(&refreshed.lease, attempt);
}

#[cfg(test)]
mod tests {
    use bevy_ecs::{
        component::Component,
        hierarchy::ChildOf,
        lifecycle::Remove,
        observer::On,
        prelude::{Commands, ResMut},
    };
    use bevy_ui_router_core::{
        frame::RouteFrameVersion,
        integration::{EngineAction, RouterEngine},
        prelude::{RouteLocation, RouteNodeOptions, RoutePattern, RouteTree, RouteViewKey},
    };

    use crate::{
        components::RouteOwnershipSeal,
        presentation::FrameLifecycle,
        registry::{EntityLifecycle, EntityOwner, OwnershipSeal, QuarantineLeaseId, RegistryOwner},
    };

    use super::*;

    #[derive(Component)]
    struct ForeignComponent;

    fn expected_record_before_foreign_insert(
        world: &mut World,
        frame: &RouteFrameVersion,
        seal_sequence: u64,
    ) -> RouteEntityRecord {
        let (record, foreign) = spawn_record(world, frame, seal_sequence, false, None);
        assert!(foreign.is_empty());
        world.entity_mut(record.entity).insert(ForeignComponent);
        record
    }

    fn frame_fixture() -> RouteFrameVersion {
        let tree = RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/").unwrap(),
                RouteNodeOptions::new().with_view(RouteViewKey::new("root").unwrap()),
            )
            .unwrap()
            .build()
            .unwrap();
        let mut engine = RouterEngine::install(tree, RouteViewKey::new("fallback").unwrap())
            .expect("frame fixture installs");
        let admission = engine.request_host_admission().unwrap();
        let grant = engine.grant_host_admission(&admission).unwrap();
        engine.confirm_host(grant);
        engine.push(RouteLocation::new("/")).unwrap();
        engine.collect();
        let EngineAction::FrameTask(task) = engine.action().expect("frame task expected") else {
            panic!("frame task expected")
        };
        task.frame().clone()
    }

    fn install_reaper_resources(world: &mut World) {
        world.insert_resource(RouterInputGate::new());
        world.insert_resource(InputFocus::default());
        world.insert_resource(RouteFocusState::default());
    }

    fn spawn_record(
        world: &mut World,
        frame: &RouteFrameVersion,
        seal_sequence: u64,
        foreign: bool,
        parent: Option<Entity>,
    ) -> (RouteEntityRecord, Vec<ComponentId>) {
        let seal = OwnershipSeal {
            domain: 41,
            sequence: seal_sequence,
        };
        let entity = world
            .spawn((
                RouteOwnershipSeal::new(seal),
                FrameLifecycle(EntityLifecycle::Quarantined),
                Visibility::Hidden,
                Pickable::IGNORE,
                RouterInputBarrier::deny_all(),
            ))
            .id();
        if let Some(parent) = parent {
            world.entity_mut(entity).insert(ChildOf(parent));
            world.flush();
        }
        let manifest = ComponentManifest::from_world(world, entity).expect("entity is present");
        if foreign {
            world.entity_mut(entity).insert(ForeignComponent);
        }
        let foreign_components = foreign
            .then(|| {
                world
                    .components()
                    .component_id::<ForeignComponent>()
                    .expect("foreign component is registered")
            })
            .into_iter()
            .collect();
        (
            RouteEntityRecord {
                entity,
                owner: RegistryOwner::Committed(EntityOwner::FrameShell(frame.clone())),
                seal,
                manifest,
                lifecycle: EntityLifecycle::Quarantined,
            },
            foreign_components,
        )
    }

    fn lease(
        world: &World,
        table: &QuarantineLeaseTable,
        sequence: u64,
        record: RouteEntityRecord,
        foreign_components: Vec<ComponentId>,
        protected_parent: Option<Entity>,
    ) -> QuarantineLease {
        let record = quarantine_record(world, record);
        let root = QuarantineRootProjection::from_record(&record).expect("record has audit root");
        QuarantineLease {
            id: QuarantineLeaseId {
                domain: table.domain,
                sequence,
            },
            generation: 1,
            operation_sequence: 0,
            root,
            entity_count: 1,
            records: vec![record],
            facts: QuarantineFacts::new(None, protected_parent, Vec::new(), foreign_components),
            last_attempt: None,
        }
    }

    fn quarantine_record(world: &World, record: RouteEntityRecord) -> QuarantineLeaseRecord {
        let observed_manifest = ComponentManifest::from_world(world, record.entity)
            .expect("quarantine record entity is present");
        let foreign_components =
            try_component_difference(&observed_manifest, &record.manifest).unwrap();
        let missing_components =
            try_component_difference(&record.manifest, &observed_manifest).unwrap();
        QuarantineLeaseRecord {
            record,
            observed_manifest,
            foreign_components,
            missing_components,
        }
    }

    fn refresh_fixture_lease_metadata(lease: &mut QuarantineLease) {
        lease.entity_count = lease.records.len();
        lease.root = QuarantineRootProjection::from_record(
            quarantine_root_record(&lease.records).expect("fixture lease has a root"),
        )
        .expect("fixture root has an audit projection");
    }

    #[test]
    fn safe_quarantine_lease_reaps_exact_owned_entity() {
        let mut world = World::new();
        install_reaper_resources(&mut world);
        let frame = frame_fixture();
        let (record, foreign) = spawn_record(&mut world, &frame, 1, false, None);
        let entity = record.entity;
        let mut table = QuarantineLeaseTable::new();
        table.next_sequence = 1;
        table
            .leases
            .push(lease(&world, &table, 1, record, foreign, None));
        world.insert_resource(table);

        reap_quarantines_system(&mut world);

        assert!(world.get_entity(entity).is_err());
        assert!(world.resource::<QuarantineLeaseTable>().leases.is_empty());
    }

    #[test]
    fn reaper_input_gate_isolation_does_not_revoke_foreign_target_authority() {
        let mut world = World::new();
        install_reaper_resources(&mut world);
        let frame = frame_fixture();
        let (record, _) = spawn_record(&mut world, &frame, 1, false, None);
        let owned = record.entity;
        let foreign = world.spawn(ChildOf(owned)).id();
        crate::input::prepare_input_gate_mutation(
            world.resource_mut::<RouterInputGate>(),
            &[],
            vec![owned, foreign],
        )
        .expect("both targets begin with independent input authority")
        .apply();
        let mut table = QuarantineLeaseTable::new();
        table.next_sequence = 1;
        let record = quarantine_record(&world, record);
        let root = QuarantineRootProjection::from_record(&record).expect("record has audit root");
        table.leases.push(QuarantineLease {
            id: QuarantineLeaseId {
                domain: table.domain,
                sequence: 1,
            },
            generation: 1,
            operation_sequence: 0,
            root,
            entity_count: 1,
            records: vec![record],
            facts: QuarantineFacts::new(None, None, vec![foreign], Vec::new()),
            last_attempt: None,
        });
        world.insert_resource(table);

        reap_quarantines_system(&mut world);

        let gate = world.resource::<RouterInputGate>();
        assert!(!gate.allows_pointer(owned));
        assert!(gate.allows_pointer(foreign));
        assert!(world.get_entity(owned).is_ok());
        assert!(world.get_entity(foreign).is_ok());
    }

    #[test]
    fn blocked_foreign_component_reap_reapplies_inert_barrier_then_closes_after_app_removal() {
        let mut world = World::new();
        install_reaper_resources(&mut world);
        let frame = frame_fixture();
        let (record, foreign) = spawn_record(&mut world, &frame, 1, true, None);
        let entity = record.entity;
        *world.get_mut::<Visibility>(entity).unwrap() = Visibility::Visible;
        *world.get_mut::<Pickable>(entity).unwrap() = Pickable::default();
        *world.get_mut::<RouterInputBarrier>(entity).unwrap() = RouterInputBarrier::allow_all();
        let mut table = QuarantineLeaseTable::new();
        table.next_sequence = 1;
        table
            .leases
            .push(lease(&world, &table, 1, record, foreign, None));
        world.insert_resource(table);

        reap_quarantines_system(&mut world);

        assert_eq!(world.get::<Visibility>(entity), Some(&Visibility::Hidden));
        assert_eq!(world.get::<Pickable>(entity), Some(&Pickable::IGNORE));
        assert!(
            world
                .get::<RouterInputBarrier>(entity)
                .is_some_and(RouterInputBarrier::denies_all)
        );
        assert_eq!(
            world.resource::<QuarantineLeaseTable>().leases[0].operation_sequence,
            1
        );

        world.entity_mut(entity).remove::<ForeignComponent>();
        reap_quarantines_system(&mut world);

        assert!(world.get_entity(entity).is_err());
        assert!(world.resource::<QuarantineLeaseTable>().leases.is_empty());
    }

    #[test]
    fn protected_parent_relationship_is_never_rewritten_by_reaper() {
        let mut world = World::new();
        install_reaper_resources(&mut world);
        let frame = frame_fixture();
        let parent = world.spawn_empty().id();
        let (record, foreign) = spawn_record(&mut world, &frame, 1, false, Some(parent));
        let entity = record.entity;
        let mut table = QuarantineLeaseTable::new();
        table.next_sequence = 1;
        table
            .leases
            .push(lease(&world, &table, 1, record, foreign, Some(parent)));
        world.insert_resource(table);

        reap_quarantines_system(&mut world);

        assert_eq!(
            world.get::<ChildOf>(entity).map(ChildOf::parent),
            Some(parent)
        );
        assert!(world.get_entity(parent).is_ok());
        assert!(world.get_entity(entity).is_ok());

        world.entity_mut(entity).remove::<ChildOf>();
        world.flush();
        reap_quarantines_system(&mut world);

        assert!(world.get_entity(parent).is_ok());
        assert!(world.get_entity(entity).is_err());
        assert!(world.resource::<QuarantineLeaseTable>().leases.is_empty());
    }

    #[test]
    fn round_robin_cursor_does_not_let_permanent_lease_starve_reapable_successor() {
        let mut world = World::new();
        install_reaper_resources(&mut world);
        let frame = frame_fixture();
        let (blocked, blocked_foreign) = spawn_record(&mut world, &frame, 1, true, None);
        let blocked_entity = blocked.entity;
        let (reapable, reapable_foreign) = spawn_record(&mut world, &frame, 2, false, None);
        let reapable_entity = reapable.entity;
        let mut table = QuarantineLeaseTable::new();
        table.next_sequence = 2;
        table
            .leases
            .push(lease(&world, &table, 1, blocked, blocked_foreign, None));
        table
            .leases
            .push(lease(&world, &table, 2, reapable, reapable_foreign, None));
        world.insert_resource(table);

        reap_quarantines_system(&mut world);
        assert!(world.get_entity(blocked_entity).is_ok());
        assert!(world.get_entity(reapable_entity).is_ok());

        reap_quarantines_system(&mut world);
        assert!(world.get_entity(blocked_entity).is_ok());
        assert!(world.get_entity(reapable_entity).is_err());
        let table = world.resource::<QuarantineLeaseTable>();
        assert_eq!(table.leases.len(), 1);
        assert_eq!(table.leases[0].id.sequence, 1);
        assert_eq!(table.leases[0].operation_sequence, 1);
    }

    fn assert_initial_capacity_failure_reapplies_inert_barrier(candidate_clone: bool) {
        let mut world = World::new();
        install_reaper_resources(&mut world);
        let frame = frame_fixture();
        let (first, first_foreign) = spawn_record(&mut world, &frame, 1, false, None);
        let first_entity = first.entity;
        let (second, second_foreign) = spawn_record(&mut world, &frame, 2, false, None);
        let second_entity = second.entity;
        let mut table = QuarantineLeaseTable::new();
        table.next_sequence = 2;
        table
            .leases
            .push(lease(&world, &table, 1, first, first_foreign, None));
        table
            .leases
            .push(lease(&world, &table, 2, second, second_foreign, None));
        world.insert_resource(table);
        world.entity_mut(first_entity).insert((
            Visibility::Visible,
            Pickable::default(),
            RouterInputBarrier::allow_all(),
        ));
        crate::input::prepare_input_gate_mutation(
            world.resource_mut::<RouterInputGate>(),
            &[],
            vec![first_entity, second_entity],
        )
        .expect("both leases can be reactivated before the retry")
        .apply();
        *world.resource_mut::<InputFocus>() = InputFocus::from_entity(first_entity);
        world
            .resource_mut::<RouteFocusState>()
            .set_focused(Some(first_entity));
        if candidate_clone {
            FAIL_NEXT_CANDIDATE_CLONE_RESERVATION.set(true);
        } else {
            FAIL_NEXT_INITIAL_PREPARATION_RESERVATION.set(true);
        }

        reap_quarantines_system(&mut world);

        assert!(world.get_entity(first_entity).is_ok());
        assert!(world.get_entity(second_entity).is_ok());
        assert_eq!(
            world.get::<Visibility>(first_entity),
            Some(&Visibility::Hidden)
        );
        assert_eq!(world.get::<Pickable>(first_entity), Some(&Pickable::IGNORE));
        assert!(
            world
                .get::<RouterInputBarrier>(first_entity)
                .is_some_and(RouterInputBarrier::denies_all)
        );
        assert_eq!(world.resource::<InputFocus>().get(), None);
        assert_eq!(world.resource::<RouteFocusState>().focused(), None);
        let gate = world.resource::<RouterInputGate>();
        assert!(!gate.allows_pointer(first_entity));
        assert!(gate.allows_pointer(second_entity));
        let table = world.resource::<QuarantineLeaseTable>();
        assert_eq!(table.reap_cursor.map(|cursor| cursor.sequence), Some(1));
        assert!(matches!(
            table.leases[0]
                .last_attempt
                .as_ref()
                .map(|attempt| &attempt.outcome),
            Some(QuarantineReapOutcome::Blocked {
                reason: QuarantineReapBlockReason::ProjectionUnavailable {
                    phase: QuarantineReapProjectionPhase::InitialPreparation,
                },
                evidence: QuarantineReapEvidence::Unavailable {
                    phase: QuarantineReapProjectionPhase::InitialPreparation,
                },
                ..
            })
        ));

        reap_quarantines_system(&mut world);

        assert!(world.get_entity(first_entity).is_ok());
        assert!(world.get_entity(second_entity).is_err());
    }

    #[test]
    fn initial_capacity_failures_are_typed_inert_and_advance_fairness_cursor() {
        assert_initial_capacity_failure_reapplies_inert_barrier(true);
        assert_initial_capacity_failure_reapplies_inert_barrier(false);
    }

    #[test]
    fn component_drift_is_scoped_to_the_exact_lease_record() {
        let mut world = World::new();
        install_reaper_resources(&mut world);
        let frame = frame_fixture();
        let foreign_record = expected_record_before_foreign_insert(&mut world, &frame, 1);
        let foreign_entity = foreign_record.entity;
        let (mut legitimate_record, _) = spawn_record(&mut world, &frame, 2, false, None);
        world
            .entity_mut(legitimate_record.entity)
            .insert(ForeignComponent);
        legitimate_record.manifest =
            ComponentManifest::from_world(&world, legitimate_record.entity).unwrap();
        let legitimate_entity = legitimate_record.entity;
        let component = world
            .components()
            .component_id::<ForeignComponent>()
            .expect("foreign component is registered");
        let mut table = QuarantineLeaseTable::new();
        table.next_sequence = 1;
        let records = vec![
            quarantine_record(&world, foreign_record),
            quarantine_record(&world, legitimate_record),
        ];
        let root =
            QuarantineRootProjection::from_record(&records[0]).expect("record has audit root");
        let entity_count = records.len();
        table.leases.push(QuarantineLease {
            id: QuarantineLeaseId {
                domain: table.domain,
                sequence: 1,
            },
            generation: 1,
            operation_sequence: 0,
            root,
            entity_count,
            records,
            facts: QuarantineFacts::new(None, None, Vec::new(), vec![component]),
            last_attempt: None,
        });
        world.insert_resource(table);

        world
            .entity_mut(foreign_entity)
            .remove::<ForeignComponent>();
        reap_quarantines_system(&mut world);

        assert!(
            world.get_entity(foreign_entity).is_err()
                || world.get_entity(legitimate_entity).is_err(),
            "a component that is foreign on one record must not block the same component when it is expected on another record"
        );
    }

    #[test]
    fn missing_component_fact_does_not_authorize_an_app_owned_component_on_another_record() {
        let mut world = World::new();
        install_reaper_resources(&mut world);
        let frame = frame_fixture();
        let (missing_record, _) = spawn_record(&mut world, &frame, 1, true, None);
        let missing_entity = missing_record.entity;
        let (protected_record, _) = spawn_record(&mut world, &frame, 2, false, None);
        let protected_entity = protected_record.entity;
        world.entity_mut(protected_entity).insert(ForeignComponent);
        let component = world
            .components()
            .component_id::<ForeignComponent>()
            .expect("foreign component is registered");
        let mut table = QuarantineLeaseTable::new();
        table.next_sequence = 1;
        let records = vec![
            quarantine_record(&world, missing_record),
            quarantine_record(&world, protected_record),
        ];
        let root =
            QuarantineRootProjection::from_record(&records[0]).expect("record has audit root");
        let entity_count = records.len();
        table.leases.push(QuarantineLease {
            id: QuarantineLeaseId {
                domain: table.domain,
                sequence: 1,
            },
            generation: 1,
            operation_sequence: 0,
            root,
            entity_count,
            records,
            facts: QuarantineFacts::with_component_drift(
                None,
                None,
                Vec::new(),
                Vec::new(),
                vec![component],
            ),
            last_attempt: None,
        });
        world.insert_resource(table);
        let _ = world.despawn(missing_entity);
        world.flush();

        reap_quarantines_system(&mut world);
        reap_quarantines_system(&mut world);

        assert!(world.get_entity(protected_entity).is_ok());
        assert_eq!(
            world.resource::<QuarantineLeaseTable>().leases[0]
                .records
                .len(),
            1
        );
    }

    #[test]
    fn missing_barriers_keep_the_lease_inert_without_panicking() {
        for missing in [
            QuarantineBarrierKind::Visibility,
            QuarantineBarrierKind::Pickable,
            QuarantineBarrierKind::Input,
        ] {
            let mut world = World::new();
            install_reaper_resources(&mut world);
            let frame = frame_fixture();
            let (record, foreign) = spawn_record(&mut world, &frame, 1, false, None);
            let entity = record.entity;
            match missing {
                QuarantineBarrierKind::Visibility => {
                    world.entity_mut(entity).remove::<Visibility>();
                }
                QuarantineBarrierKind::Pickable => {
                    world.entity_mut(entity).remove::<Pickable>();
                }
                QuarantineBarrierKind::Input => {
                    world.entity_mut(entity).remove::<RouterInputBarrier>();
                }
            }
            let mut table = QuarantineLeaseTable::new();
            table.next_sequence = 1;
            table
                .leases
                .push(lease(&world, &table, 1, record, foreign, None));
            world.insert_resource(table);

            reap_quarantines_system(&mut world);

            assert!(world.get_entity(entity).is_ok());
            assert_eq!(
                world.resource::<QuarantineLeaseTable>().leases[0].operation_sequence,
                1
            );
            let attempt = world.resource::<QuarantineLeaseTable>().leases[0]
                .last_attempt
                .as_ref()
                .expect("blocked attempt is retained as typed evidence");
            let QuarantineReapEvidence::Observed(observations) = (match &attempt.outcome {
                QuarantineReapOutcome::Blocked { evidence, .. } => evidence,
                other => panic!("expected blocked evidence, observed {other:?}"),
            }) else {
                panic!("ordinary boundary drift must retain its observed projection")
            };
            assert_eq!(observations.len(), 1);
            assert_eq!(observations[0].lease_id, attempt.lease_id);
            assert_eq!(observations[0].generation, attempt.generation);
            assert_eq!(
                observations[0].operation_sequence,
                attempt.operation_sequence
            );
            assert_eq!(observations[0].target.entity(), entity);
            assert!(matches!(
                &attempt.outcome,
                QuarantineReapOutcome::Blocked {
                    reason: QuarantineReapBlockReason::MissingBarrier {
                        entity: observed_entity,
                        barrier,
                    },
                    ..
                } if *observed_entity == entity && *barrier == missing
            ));
            if missing != QuarantineBarrierKind::Visibility {
                assert_eq!(world.get::<Visibility>(entity), Some(&Visibility::Hidden));
            }
            if missing != QuarantineBarrierKind::Pickable {
                assert_eq!(world.get::<Pickable>(entity), Some(&Pickable::IGNORE));
            }
            if missing != QuarantineBarrierKind::Input {
                assert!(
                    world
                        .get::<RouterInputBarrier>(entity)
                        .is_some_and(RouterInputBarrier::denies_all)
                );
            }
        }
    }

    #[test]
    fn terminal_record_settlement_preserves_and_later_reaps_the_survivor() {
        let mut world = World::new();
        install_reaper_resources(&mut world);
        let frame = frame_fixture();
        let (first, first_foreign) = spawn_record(&mut world, &frame, 1, false, None);
        let first_entity = first.entity;
        let (second, second_foreign) = spawn_record(&mut world, &frame, 2, false, None);
        let second_entity = second.entity;
        let mut table = QuarantineLeaseTable::new();
        table.next_sequence = 1;
        let mut combined = lease(&world, &table, 1, first, first_foreign, None);
        combined.records.push(quarantine_record(&world, second));
        refresh_fixture_lease_metadata(&mut combined);
        let frozen_root = combined.root.clone();
        let frozen_entity_count = combined.entity_count;
        assert!(second_foreign.is_empty());
        table.leases.push(combined);
        world.insert_resource(table);
        let _ = world.despawn(first_entity);
        world.flush();

        reap_quarantines_system(&mut world);

        let table = world.resource::<QuarantineLeaseTable>();
        assert_eq!(table.leases[0].records.len(), 1);
        assert_eq!(table.leases[0].records[0].entity(), second_entity);
        assert_eq!(table.leases[0].root, frozen_root);
        assert_eq!(table.leases[0].entity_count, frozen_entity_count);
        assert!(matches!(
            table.leases[0].last_attempt.as_ref().map(|attempt| &attempt.outcome),
            Some(QuarantineReapOutcome::Terminal { target, .. }) if target.entity() == first_entity
        ));

        reap_quarantines_system(&mut world);

        let table = world.resource::<QuarantineLeaseTable>();
        assert!(table.leases.is_empty());
        assert!(world.get_entity(second_entity).is_err());
        assert!(matches!(
            table
                .last_closed_reap_attempt
                .as_ref()
                .map(|attempt| &attempt.outcome),
            Some(QuarantineReapOutcome::Despawned { target, .. })
                if target.entity() == second_entity
        ));
    }

    #[test]
    fn reused_entity_slot_cannot_authorize_reaping_the_replacement_or_survivor() {
        let mut world = World::new();
        install_reaper_resources(&mut world);
        let frame = frame_fixture();
        let (retired, retired_foreign) = spawn_record(&mut world, &frame, 1, false, None);
        let retired_entity = retired.entity;
        let (survivor, survivor_foreign) = spawn_record(&mut world, &frame, 2, false, None);
        let survivor_entity = survivor.entity;
        let mut table = QuarantineLeaseTable::new();
        table.next_sequence = 1;
        let mut combined = lease(&world, &table, 1, retired, retired_foreign, None);
        combined.records.push(quarantine_record(&world, survivor));
        refresh_fixture_lease_metadata(&mut combined);
        assert!(survivor_foreign.is_empty());
        table.leases.push(combined);
        world.insert_resource(table);
        let _ = world.despawn(retired_entity);
        world.flush();
        let replacement = world.entities().resolve_from_index(retired_entity.index());
        world
            .spawn_at(replacement, ForeignComponent)
            .expect("the retired EntityIndex can be reused at its current generation");
        assert_ne!(replacement, retired_entity);
        assert_eq!(replacement.index(), retired_entity.index());

        reap_quarantines_system(&mut world);

        assert!(world.get_entity(replacement).is_ok());
        assert!(world.get::<ForeignComponent>(replacement).is_some());
        assert!(world.get_entity(survivor_entity).is_ok());
        let table = world.resource::<QuarantineLeaseTable>();
        assert_eq!(table.leases[0].records.len(), 1);
        assert_eq!(table.leases[0].records[0].entity(), survivor_entity);
    }

    #[test]
    fn stale_generation_and_operation_attempts_cannot_settle_a_current_lease() {
        let mut world = World::new();
        install_reaper_resources(&mut world);
        let frame = frame_fixture();
        let (record, foreign) = spawn_record(&mut world, &frame, 1, false, None);
        let mut table = QuarantineLeaseTable::new();
        table.next_sequence = 1;
        table
            .leases
            .push(lease(&world, &table, 1, record, foreign, None));
        let candidate = try_clone_quarantine_lease_for_reap(&table.leases[0]).unwrap();
        let baseline = table.leases.clone();
        let blocked = QuarantineReapOutcome::Blocked {
            target: None,
            reason: QuarantineReapBlockReason::NoOwnedLeaf,
            evidence: QuarantineReapEvidence::Observed(Vec::new()),
        };

        assert!(!table.settle_reap_attempt(
            &candidate,
            QuarantineReapAttempt {
                lease_id: candidate.id,
                generation: candidate.generation + 1,
                operation_sequence: 1,
                outcome: blocked.clone(),
            }
        ));
        assert!(!table.settle_reap_attempt(
            &candidate,
            QuarantineReapAttempt {
                lease_id: candidate.id,
                generation: candidate.generation,
                operation_sequence: 2,
                outcome: blocked,
            }
        ));
        assert_eq!(table.leases, baseline);
        assert!(table.reap_cursor.is_none());
    }

    #[test]
    fn cursor_keeps_strict_successor_order_across_lease_close_and_insert() {
        let mut world = World::new();
        install_reaper_resources(&mut world);
        let frame = frame_fixture();
        let (blocked, blocked_foreign) = spawn_record(&mut world, &frame, 1, true, None);
        let blocked_entity = blocked.entity;
        let (second, second_foreign) = spawn_record(&mut world, &frame, 2, false, None);
        let second_entity = second.entity;
        let (third, third_foreign) = spawn_record(&mut world, &frame, 3, false, None);
        let third_entity = third.entity;
        let mut table = QuarantineLeaseTable::new();
        table.next_sequence = 3;
        let third_lease = lease(&world, &table, 3, third, third_foreign, None);
        table
            .leases
            .push(lease(&world, &table, 1, blocked, blocked_foreign, None));
        table
            .leases
            .push(lease(&world, &table, 2, second, second_foreign, None));
        world.insert_resource(table);

        reap_quarantines_system(&mut world);
        reap_quarantines_system(&mut world);
        assert!(world.get_entity(second_entity).is_err());
        world
            .resource_mut::<QuarantineLeaseTable>()
            .leases
            .push(third_lease);

        reap_quarantines_system(&mut world);

        assert!(world.get_entity(third_entity).is_err());
        assert!(world.get_entity(blocked_entity).is_ok());
    }

    #[test]
    fn descendant_focus_is_cleared_without_revoking_foreign_input_authority() {
        let mut world = World::new();
        install_reaper_resources(&mut world);
        let frame = frame_fixture();
        let (record, _) = spawn_record(&mut world, &frame, 1, false, None);
        let owned = record.entity;
        let foreign = world.spawn(ChildOf(owned)).id();
        *world.resource_mut::<InputFocus>() = InputFocus::from_entity(foreign);
        world
            .resource_mut::<RouteFocusState>()
            .set_focused(Some(foreign));
        crate::input::prepare_input_gate_mutation(
            world.resource_mut::<RouterInputGate>(),
            &[],
            vec![owned, foreign],
        )
        .unwrap()
        .apply();
        let mut table = QuarantineLeaseTable::new();
        table.next_sequence = 1;
        let record = quarantine_record(&world, record);
        let root = QuarantineRootProjection::from_record(&record).expect("record has audit root");
        table.leases.push(QuarantineLease {
            id: QuarantineLeaseId {
                domain: table.domain,
                sequence: 1,
            },
            generation: 1,
            operation_sequence: 0,
            root,
            entity_count: 1,
            records: vec![record],
            facts: QuarantineFacts::new(None, None, vec![foreign], Vec::new()),
            last_attempt: None,
        });
        world.insert_resource(table);

        reap_quarantines_system(&mut world);

        assert_eq!(world.resource::<InputFocus>().get(), None);
        assert_eq!(world.resource::<RouteFocusState>().focused(), None);
        let gate = world.resource::<RouterInputGate>();
        assert!(!gate.allows_pointer(owned));
        assert!(gate.allows_pointer(foreign));
    }

    #[test]
    fn despawn_observer_drift_refreezes_and_preserves_the_remaining_record() {
        let mut world = World::new();
        install_reaper_resources(&mut world);
        let frame = frame_fixture();
        let (first, first_foreign) = spawn_record(&mut world, &frame, 1, false, None);
        let first_entity = first.entity;
        let (second, second_foreign) = spawn_record(&mut world, &frame, 2, false, None);
        let second_entity = second.entity;
        world.add_observer(
            move |event: On<Remove, RouteOwnershipSeal>, mut commands: Commands| {
                if event.entity == first_entity {
                    commands.entity(second_entity).insert(ForeignComponent);
                }
            },
        );
        let mut table = QuarantineLeaseTable::new();
        table.next_sequence = 1;
        let mut combined = lease(&world, &table, 1, first, first_foreign, None);
        combined.records.push(quarantine_record(&world, second));
        refresh_fixture_lease_metadata(&mut combined);
        assert!(second_foreign.is_empty());
        table.leases.push(combined);
        world.insert_resource(table);

        reap_quarantines_system(&mut world);

        assert!(world.get_entity(first_entity).is_err());
        assert!(world.get_entity(second_entity).is_ok());
        let table = world.resource::<QuarantineLeaseTable>();
        assert_eq!(table.leases[0].records.len(), 1);
        assert!(matches!(
            table.leases[0].last_attempt.as_ref().map(|attempt| &attempt.outcome),
            Some(QuarantineReapOutcome::Despawned {
                postwrite: QuarantineReapPostwrite::Superseded {
                    reason: QuarantineReapBlockReason::ManifestDrift { entity, .. },
                    ..
                },
                ..
            }) if *entity == second_entity
        ));

        reap_quarantines_system(&mut world);
        assert!(world.get_entity(second_entity).is_ok());
    }

    #[test]
    fn postwrite_observer_cannot_restore_interaction_or_descendant_focus() {
        let mut world = World::new();
        install_reaper_resources(&mut world);
        let frame = frame_fixture();
        let (first, first_foreign) = spawn_record(&mut world, &frame, 1, false, None);
        let first_entity = first.entity;
        let (parent, parent_foreign) = spawn_record(&mut world, &frame, 2, false, None);
        let parent_entity = parent.entity;
        let (child, child_foreign) =
            spawn_record(&mut world, &frame, 3, false, Some(parent_entity));
        let child_entity = child.entity;
        world.add_observer(
            move |event: On<Remove, RouteOwnershipSeal>,
                  mut commands: Commands,
                  mut gate: ResMut<RouterInputGate>,
                  mut input_focus: ResMut<InputFocus>,
                  mut route_focus: ResMut<RouteFocusState>| {
                if event.entity == first_entity {
                    commands.entity(parent_entity).insert((
                        Visibility::Visible,
                        Pickable::default(),
                        RouterInputBarrier::allow_all(),
                    ));
                    crate::input::prepare_input_gate_mutation(
                        gate.reborrow(),
                        &[],
                        vec![parent_entity, child_entity],
                    )
                    .expect("observer can attempt to restore exact target input")
                    .apply();
                    *input_focus = InputFocus::from_entity(child_entity);
                    route_focus.set_focused(Some(child_entity));
                }
            },
        );
        let mut table = QuarantineLeaseTable::new();
        table.next_sequence = 1;
        let mut combined = lease(&world, &table, 1, first, first_foreign, None);
        combined.records.push(quarantine_record(&world, parent));
        combined.records.push(quarantine_record(&world, child));
        refresh_fixture_lease_metadata(&mut combined);
        assert!(parent_foreign.is_empty());
        assert!(child_foreign.is_empty());
        table.leases.push(combined);
        world.insert_resource(table);

        reap_quarantines_system(&mut world);

        assert!(world.get_entity(first_entity).is_err());
        assert_eq!(
            world.get::<Visibility>(parent_entity),
            Some(&Visibility::Hidden)
        );
        assert_eq!(
            world.get::<Pickable>(parent_entity),
            Some(&Pickable::IGNORE)
        );
        assert!(
            world
                .get::<RouterInputBarrier>(parent_entity)
                .is_some_and(RouterInputBarrier::denies_all)
        );
        assert_eq!(world.resource::<InputFocus>().get(), None);
        assert_eq!(world.resource::<RouteFocusState>().focused(), None);
        let gate = world.resource::<RouterInputGate>();
        assert!(!gate.allows_pointer(parent_entity));
        assert!(!gate.allows_pointer(child_entity));
        let attempt = world.resource::<QuarantineLeaseTable>().leases[0]
            .last_attempt
            .as_ref()
            .expect("despawn attempt is retained");
        let QuarantineReapOutcome::Despawned {
            postwrite: QuarantineReapPostwrite::Refrozen { survivors },
            ..
        } = &attempt.outcome
        else {
            panic!("postwrite survivor projection must be refrozen")
        };
        assert_eq!(survivors.len(), 2);
        let parent = survivors
            .iter()
            .find(|observation| observation.target.entity() == parent_entity)
            .expect("parent survivor is observed");
        assert_eq!(parent.lease_id, attempt.lease_id);
        assert_eq!(parent.generation, attempt.generation);
        assert_eq!(parent.operation_sequence, attempt.operation_sequence);
        assert_eq!(parent.visibility, Some(Visibility::Hidden));
        assert_eq!(parent.pickable, Some(Pickable::IGNORE));
        assert!(
            parent
                .input_barrier
                .is_some_and(|barrier| barrier.denies_all())
        );
        assert_eq!(parent.input_focus, Some(None));
        assert_eq!(parent.route_focus, Some(None));
        assert!(
            parent
                .input_gate
                .is_some_and(|gate| { !gate.pointer && !gate.keyboard && !gate.gamepad })
        );
        assert_eq!(parent.authority.observed_seal, Some(parent.target.seal()));
        assert_eq!(parent.parent, None);
        assert_eq!(parent.children.as_deref(), Some(&[child_entity][..]));
        let mut expected_partition = vec![parent_entity, child_entity];
        expected_partition.sort_by_key(|entity| entity.to_bits());
        assert_eq!(parent.exact_entities, expected_partition);
        assert_eq!(parent.focus_targets, expected_partition);
        assert_eq!(parent.input_gate_allowed.as_deref(), Some(&[][..]));
        assert!(!parent.expected_manifest.component_ids().is_empty());
        assert_eq!(
            parent.transfer_manifest.component_ids().len(),
            parent.expected_manifest.component_ids().len() + 1,
            "the transfer projection independently retains the later Children relationship"
        );
    }

    #[test]
    fn postwrite_authority_rebind_preserves_the_new_owner_interaction_state() {
        let mut world = World::new();
        install_reaper_resources(&mut world);
        let frame = frame_fixture();
        let (first, first_foreign) = spawn_record(&mut world, &frame, 1, false, None);
        let first_entity = first.entity;
        let (parent, parent_foreign) = spawn_record(&mut world, &frame, 2, false, None);
        let parent_entity = parent.entity;
        let (child, child_foreign) =
            spawn_record(&mut world, &frame, 3, false, Some(parent_entity));
        let child_entity = child.entity;
        let rebound_parent_seal = OwnershipSeal {
            domain: 73,
            sequence: 1,
        };
        let rebound_child_seal = OwnershipSeal {
            domain: 73,
            sequence: 2,
        };
        world.add_observer(
            move |event: On<Remove, RouteOwnershipSeal>,
                  mut commands: Commands,
                  mut gate: ResMut<RouterInputGate>,
                  mut input_focus: ResMut<InputFocus>,
                  mut route_focus: ResMut<RouteFocusState>| {
                if event.entity == first_entity {
                    commands.entity(parent_entity).insert((
                        RouteOwnershipSeal::new(rebound_parent_seal),
                        Visibility::Visible,
                        Pickable::default(),
                        RouterInputBarrier::allow_all(),
                        ForeignComponent,
                    ));
                    commands.entity(child_entity).insert((
                        RouteOwnershipSeal::new(rebound_child_seal),
                        Visibility::Visible,
                        Pickable::default(),
                        RouterInputBarrier::allow_all(),
                    ));
                    crate::input::prepare_input_gate_mutation(
                        gate.reborrow(),
                        &[],
                        vec![parent_entity, child_entity],
                    )
                    .expect("the rebound authority can publish its own input state")
                    .apply();
                    *input_focus = InputFocus::from_entity(child_entity);
                    route_focus.set_focused(Some(child_entity));
                }
            },
        );
        let mut table = QuarantineLeaseTable::new();
        table.next_sequence = 1;
        let mut combined = lease(&world, &table, 1, first, first_foreign, None);
        combined.records.push(quarantine_record(&world, parent));
        combined.records.push(quarantine_record(&world, child));
        refresh_fixture_lease_metadata(&mut combined);
        assert!(parent_foreign.is_empty());
        assert!(child_foreign.is_empty());
        table.leases.push(combined);
        world.insert_resource(table);

        reap_quarantines_system(&mut world);

        assert!(world.get_entity(first_entity).is_err());
        assert!(world.get_entity(parent_entity).is_ok());
        assert!(world.get_entity(child_entity).is_ok());
        assert_eq!(
            world
                .get::<RouteOwnershipSeal>(parent_entity)
                .map(|seal| (*seal).seal()),
            Some(rebound_parent_seal)
        );
        assert_eq!(
            world
                .get::<RouteOwnershipSeal>(child_entity)
                .map(|seal| (*seal).seal()),
            Some(rebound_child_seal)
        );
        for entity in [parent_entity, child_entity] {
            assert_eq!(world.get::<Visibility>(entity), Some(&Visibility::Visible));
            assert_eq!(world.get::<Pickable>(entity), Some(&Pickable::default()));
            assert!(
                world
                    .get::<RouterInputBarrier>(entity)
                    .is_some_and(RouterInputBarrier::allows_all)
            );
        }
        assert!(world.get::<ForeignComponent>(parent_entity).is_some());
        assert_eq!(world.resource::<InputFocus>().get(), Some(child_entity));
        assert_eq!(
            world.resource::<RouteFocusState>().focused(),
            Some(child_entity)
        );
        let gate = world.resource::<RouterInputGate>();
        assert!(gate.allows_pointer(parent_entity));
        assert!(gate.allows_pointer(child_entity));
        assert!(matches!(
            world.resource::<QuarantineLeaseTable>().leases[0]
                .last_attempt
                .as_ref()
                .map(|attempt| &attempt.outcome),
            Some(QuarantineReapOutcome::Despawned {
                postwrite: QuarantineReapPostwrite::Superseded {
                    reason: QuarantineReapBlockReason::AuthorityDrift { entity },
                    ..
                },
                ..
            }) if *entity == parent_entity || *entity == child_entity
        ));
    }

    fn assert_postwrite_capacity_failure_restores_inert_state(refreeze: bool) {
        let mut world = World::new();
        install_reaper_resources(&mut world);
        let frame = frame_fixture();
        let (first, first_foreign) = spawn_record(&mut world, &frame, 1, false, None);
        let first_entity = first.entity;
        let (parent, parent_foreign) = spawn_record(&mut world, &frame, 2, false, None);
        let parent_entity = parent.entity;
        let (child, child_foreign) =
            spawn_record(&mut world, &frame, 3, false, Some(parent_entity));
        let child_entity = child.entity;
        world.add_observer(
            move |event: On<Remove, RouteOwnershipSeal>,
                  mut commands: Commands,
                  mut gate: ResMut<RouterInputGate>,
                  mut input_focus: ResMut<InputFocus>,
                  mut route_focus: ResMut<RouteFocusState>| {
                if event.entity == first_entity {
                    commands.entity(parent_entity).insert((
                        Visibility::Visible,
                        Pickable::default(),
                        RouterInputBarrier::allow_all(),
                    ));
                    crate::input::prepare_input_gate_mutation(
                        gate.reborrow(),
                        &[],
                        vec![parent_entity, child_entity],
                    )
                    .unwrap()
                    .apply();
                    *input_focus = InputFocus::from_entity(child_entity);
                    route_focus.set_focused(Some(child_entity));
                }
            },
        );
        let mut table = QuarantineLeaseTable::new();
        table.next_sequence = 1;
        let mut combined = lease(&world, &table, 1, first, first_foreign, None);
        combined.records.push(quarantine_record(&world, parent));
        combined.records.push(quarantine_record(&world, child));
        refresh_fixture_lease_metadata(&mut combined);
        assert!(parent_foreign.is_empty());
        assert!(child_foreign.is_empty());
        table.leases.push(combined);
        world.insert_resource(table);
        if refreeze {
            FAIL_NEXT_POSTWRITE_REFREEZE_RESERVATION.set(true);
        } else {
            FAIL_NEXT_POSTWRITE_PREPARATION_RESERVATION.set(true);
        }

        reap_quarantines_system(&mut world);

        assert!(world.get_entity(first_entity).is_err());
        assert_eq!(
            world.get::<Visibility>(parent_entity),
            Some(&Visibility::Hidden)
        );
        assert_eq!(
            world.get::<Pickable>(parent_entity),
            Some(&Pickable::IGNORE)
        );
        assert!(
            world
                .get::<RouterInputBarrier>(parent_entity)
                .is_some_and(RouterInputBarrier::denies_all)
        );
        assert_eq!(world.resource::<InputFocus>().get(), None);
        assert_eq!(world.resource::<RouteFocusState>().focused(), None);
        assert!(
            !world
                .resource::<RouterInputGate>()
                .allows_pointer(parent_entity)
        );
        let expected_phase = if refreeze {
            QuarantineReapProjectionPhase::PostwriteRefreeze
        } else {
            QuarantineReapProjectionPhase::PostwritePreparation
        };
        assert!(matches!(
            world.resource::<QuarantineLeaseTable>().leases[0]
                .last_attempt
                .as_ref()
                .map(|attempt| &attempt.outcome),
            Some(QuarantineReapOutcome::Despawned {
                postwrite: QuarantineReapPostwrite::Superseded {
                    reason: QuarantineReapBlockReason::ProjectionUnavailable { phase },
                    evidence: QuarantineReapEvidence::Unavailable { phase: evidence_phase },
                    ..
                },
                ..
            }) if *phase == expected_phase && *evidence_phase == expected_phase
        ));
    }

    #[test]
    fn postwrite_capacity_failures_restore_inert_state_before_returning() {
        assert_postwrite_capacity_failure_restores_inert_state(false);
        assert_postwrite_capacity_failure_restores_inert_state(true);
    }

    #[test]
    fn unavailable_despawn_observation_cannot_terminally_settle_a_record() {
        let mut world = World::new();
        install_reaper_resources(&mut world);
        let frame = frame_fixture();
        let (record, foreign) = spawn_record(&mut world, &frame, 1, false, None);
        let entity = record.entity;
        let mut table = QuarantineLeaseTable::new();
        table.next_sequence = 1;
        table
            .leases
            .push(lease(&world, &table, 1, record, foreign, None));
        let candidate = try_clone_quarantine_lease_for_reap(&table.leases[0]).unwrap();
        let _ = world.despawn(entity);
        world.flush();
        let phase = QuarantineReapProjectionPhase::PostdespawnObservation;
        assert!(table.settle_reap_attempt(
            &candidate,
            QuarantineReapAttempt {
                lease_id: candidate.id,
                generation: candidate.generation,
                operation_sequence: 1,
                outcome: QuarantineReapOutcome::Despawned {
                    target: candidate.records[0].claim(),
                    evidence: QuarantineReapEvidence::Unavailable { phase },
                    postwrite: QuarantineReapPostwrite::Superseded {
                        target: None,
                        reason: QuarantineReapBlockReason::ProjectionUnavailable { phase },
                        evidence: QuarantineReapEvidence::Unavailable { phase },
                    },
                },
            }
        ));
        assert_eq!(table.leases[0].records.len(), 1);
        assert_eq!(table.leases[0].records[0].entity(), entity);
        world.insert_resource(table);

        reap_quarantines_system(&mut world);

        assert!(world.resource::<QuarantineLeaseTable>().leases.is_empty());
    }
}
