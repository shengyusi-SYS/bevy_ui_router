use bevy_camera::visibility::Visibility;
use bevy_ecs::prelude::World;
use bevy_input_focus::InputFocus;
use bevy_picking::Pickable;
use bevy_ui_router_core::{
    integration::{BeginAuthoritySettlementErrorKind, CleanupManifest},
    prelude::RouteFrameVersion,
};

use crate::{
    branch_transaction::{FailedPreparedBranch, PendingCommit, PreparedBranch, RealizedBranch},
    cleanup::{
        AuthoritySettlementManifestError, CleanupLedger, CleanupLedgerReserveErrorKind,
        FailedPreparedTopologyError, FrozenCleanupManifest, FrozenOriginalCleanupLedgerTransfer,
        PreparedPostCommitAuthorityLedger, cleanup_claim_has_exact_authority,
    },
    components::{RouteOwnershipSeal, RouterInputBarrier},
    input::{
        PreparedInputGateIsolation, RouteFocusState, RouterInputGate, observed_descendant_closure,
        try_prepare_input_gate_isolation,
    },
    mount::ChildSlotResidual,
    presentation::RealizedFrame,
    registry::{ComponentManifest, ExactRegistryClaim, RouteEntityRegistry},
};

use super::{
    AuthorityProtocolResidualKind, AuthorityProtocolResidualOwner,
    CleanupSettlementPermanentInertOwner, FailureCleanupCompletionInertOwner,
    FrozenPostCommitInputIsolation, FrozenTargetPreparationPermanentPhysicalManifest, HostBinding,
    PendingFailedBranch, PostCommitInputIsolationFailure, PreparedPostCommitInputIsolation,
    RetiredHostBinding, RouterEntry, authority_settlement_logical_roots, precommit_authority_cause,
    projectionless_fail_close_capacity_probe,
};

pub(super) enum AuthoritySettlementBeginOutcome {
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

pub(super) fn begin_authority_settlement(
    world: &mut World,
    entry: &mut RouterEntry,
    active: RealizedBranch,
    pending: PendingCommit,
    logical_roots_override: Option<Vec<RouteFrameVersion>>,
) -> AuthoritySettlementBeginOutcome {
    let Some(host) = entry.host.as_ref() else {
        return AuthoritySettlementBeginOutcome::ProtocolContradiction {
            active,
            pending,
            kind: BeginAuthoritySettlementErrorKind::Stale,
        };
    };
    let cause = precommit_authority_cause(world, host);
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
        registration_incarnation,
        lease,
    } = entry
        .host
        .take()
        .expect("authority settlement retains its exact host binding");
    let logical_roots = logical_roots_override
        .unwrap_or_else(|| authority_settlement_logical_roots(&pending.branch, &active));
    let retired_host = RetiredHostBinding {
        version: version.clone(),
        slot_id,
        host: host_entity,
        top_slot,
        top_slot_claim: top_slot_claim.clone(),
        candidate: candidate.clone(),
        registration_incarnation,
    };
    let mut settlement_owner = Some((reserved, active, pending, retired_host));
    let engine = &mut entry.engine;
    let cleanup_ledger = &mut entry.cleanup_ledger;
    match engine.begin_authority_settlement_for_cause_retaining(
        lease,
        cause,
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
                registration_incarnation,
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
pub(super) enum AuthorityIsolationComponent {
    Visibility,
    Pickable,
    RouterInputBarrier,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AuthorityIsolationPredicate {
    PresentForHiddenWrite,
    PresentForIgnoreWrite,
    PresentForDenyWrite,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct AuthorityIsolationViolation {
    pub(super) entity: bevy_ecs::entity::Entity,
    pub(super) component: AuthorityIsolationComponent,
    pub(super) predicate: AuthorityIsolationPredicate,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct AuthorityIsolationContradiction {
    pub(super) violations: Vec<AuthorityIsolationViolation>,
}

pub(super) struct PreparedAuthorityEntityIsolation {
    pub(super) entity: bevy_ecs::entity::Entity,
    pub(super) visibility: bool,
    pub(super) pickable: bool,
    pub(super) insert_pickable: bool,
    pub(super) barrier: bool,
}

pub(super) struct PreparedAuthorityIsolation {
    pub(super) entities: Vec<PreparedAuthorityEntityIsolation>,
    pub(super) focus: Vec<bevy_ecs::entity::Entity>,
    pub(super) gate: Option<PreparedInputGateIsolation>,
}

impl PreparedAuthorityIsolation {
    pub(super) fn apply(self, world: &mut World) {
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

    pub(super) fn apply_guarded(self, world: &mut World, guard: &impl Fn(&World) -> bool) -> bool {
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
        if !guard(world) {
            return false;
        }
        for entity in self.entities {
            if entity.visibility {
                *world
                    .get_mut::<Visibility>(entity.entity)
                    .expect("prepared post-commit isolation retains Visibility") =
                    Visibility::Hidden;
                world.flush();
                if !guard(world) {
                    return false;
                }
            }
            if entity.pickable {
                *world
                    .get_mut::<Pickable>(entity.entity)
                    .expect("prepared post-commit isolation retains Pickable") = Pickable::IGNORE;
                world.flush();
                if !guard(world) {
                    return false;
                }
            }
            if entity.barrier {
                *world
                    .get_mut::<RouterInputBarrier>(entity.entity)
                    .expect("prepared post-commit isolation retains input barrier") =
                    RouterInputBarrier::deny_all();
                world.flush();
                if !guard(world) {
                    return false;
                }
            }
        }
        guard(world)
    }
}

impl FrozenPostCommitInputIsolation {
    pub(super) fn is_current(&self, world: &World) -> bool {
        (!self.gate_required
            || world.get_resource::<RouterInputGate>().is_some_and(|gate| {
                self.gate_targets.iter().all(|entity| {
                    !gate.allows_pointer(*entity)
                        && !gate.allows_keyboard(*entity)
                        && !gate.allows_gamepad(*entity)
                })
            }))
            && world
                .get_resource::<InputFocus>()
                .and_then(InputFocus::get)
                .is_none_or(|focused| !self.focus.contains(&focused))
            && world
                .get_resource::<RouteFocusState>()
                .and_then(RouteFocusState::focused)
                .is_none_or(|focused| !self.focus.contains(&focused))
    }
}

impl PreparedPostCommitInputIsolation {
    pub(super) fn apply_guarded(
        self,
        world: &mut World,
        guard: &impl Fn(&World) -> bool,
    ) -> Result<FrozenPostCommitInputIsolation, PostCommitInputIsolationFailure> {
        let frozen = self.frozen;
        let combined_guard = |world: &World| frozen.is_current(world) && guard(world);
        if !self.mutation.apply_guarded(world, &combined_guard) || !combined_guard(world) {
            return Err(PostCommitInputIsolationFailure::RefreezeRejected);
        }
        if let Some(contradiction) = self.contradiction {
            return Err(PostCommitInputIsolationFailure::Permanent(contradiction));
        }
        Ok(frozen)
    }
}

pub(super) enum AuthorityIsolationPreparation {
    Ready(PreparedAuthorityIsolation),
    RetryableCapacity,
    PermanentContradiction {
        contradiction: AuthorityIsolationContradiction,
        fail_closed: PreparedAuthorityIsolation,
    },
}

pub(super) fn try_push<T>(values: &mut Vec<T>, value: T) -> Result<(), ()> {
    values.try_reserve(1).map_err(|_| ())?;
    values.push(value);
    Ok(())
}

pub(super) fn prepare_authority_isolation_targets<'a>(
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
                        .is_some_and(|record| record.claim() == claim.clone())
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
                    insert_pickable: false,
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

pub(super) fn prepare_post_commit_input_isolation(
    world: &World,
    active: &RealizedBranch,
    original: &FrozenOriginalCleanupLedgerTransfer,
    authority: &PreparedPostCommitAuthorityLedger,
) -> Result<PreparedPostCommitInputIsolation, PostCommitInputIsolationFailure> {
    let mut gate_targets = active.interactive_entities();
    gate_targets.extend(original.partition_entities().iter().copied());
    gate_targets.sort_by_key(|entity| entity.to_bits());
    gate_targets.dedup();
    let gate_required = world.get_resource::<RouterInputGate>().is_some();
    let preparation = prepare_authority_isolation_targets(
        world,
        authority
            .partition()
            .targets()
            .iter()
            .map(|target| target.claim()),
        gate_targets.iter().copied(),
        authority.partition().targets().len(),
    );
    let (mut mutation, contradiction) = match preparation {
        AuthorityIsolationPreparation::Ready(prepared) => (prepared, None),
        AuthorityIsolationPreparation::RetryableCapacity => {
            return Err(PostCommitInputIsolationFailure::RetryableCapacity);
        }
        AuthorityIsolationPreparation::PermanentContradiction {
            contradiction,
            fail_closed,
        } => (fail_closed, Some(contradiction)),
    };
    let original_focus = observed_descendant_closure(
        world,
        original
            .immutable_entities()
            .iter()
            .copied()
            .filter(|entity| world.get_entity(*entity).is_ok()),
    );
    mutation
        .focus
        .try_reserve(original_focus.len())
        .map_err(|_| PostCommitInputIsolationFailure::RetryableCapacity)?;
    mutation.focus.extend(original_focus);
    mutation.focus.sort_by_key(|entity| entity.to_bits());
    mutation.focus.dedup();
    let frozen = FrozenPostCommitInputIsolation {
        focus: mutation.focus.clone(),
        gate_targets,
        gate_required,
    };
    Ok(PreparedPostCommitInputIsolation {
        mutation,
        frozen,
        contradiction,
    })
}

pub(super) fn target_authority_missing_pickable_repair_is_exact(
    world: &World,
    registry: &RouteEntityRegistry,
    validation: &FrozenTargetPreparationPermanentPhysicalManifest,
    entity: bevy_ecs::entity::Entity,
) -> bool {
    if !validation.repair_missing_pickable {
        return false;
    }
    let Some(frozen) = validation
        .target_observed
        .iter()
        .find(|observed| observed.entity == entity)
    else {
        return false;
    };
    let Some(record) = registry.record(entity) else {
        return false;
    };
    let Some(actual) = ComponentManifest::from_world(world, entity) else {
        return false;
    };
    let Some(frozen_actual) = frozen.manifest.as_ref() else {
        return false;
    };
    let Some(pickable) = world.components().component_id::<Pickable>() else {
        return false;
    };
    let projected_without_pickable = ComponentManifest::new(
        record
            .manifest()
            .component_ids()
            .iter()
            .copied()
            .filter(|component| *component != pickable),
    );
    frozen.registry_claim.as_ref() == Some(&record.claim())
        && cleanup_claim_has_exact_authority(world, &record.claim())
        && frozen.ownership_seal == Some(RouteOwnershipSeal::new(record.claim().seal()))
        && actual == *frozen_actual
        && !actual.component_ids().contains(&pickable)
        && record.manifest().component_ids().contains(&pickable)
        && actual == projected_without_pickable
}

pub(super) fn prepare_target_preparation_permanent_isolation<'a>(
    world: &World,
    stored_claims: impl IntoIterator<Item = &'a ExactRegistryClaim>,
    gate_targets: impl IntoIterator<Item = bevy_ecs::entity::Entity>,
    capacity_probe: usize,
    validation: &FrozenTargetPreparationPermanentPhysicalManifest,
) -> AuthorityIsolationPreparation {
    let prepared =
        prepare_authority_isolation_targets(world, stored_claims, gate_targets, capacity_probe);
    let AuthorityIsolationPreparation::PermanentContradiction {
        contradiction,
        mut fail_closed,
    } = prepared
    else {
        return prepared;
    };
    let Some(registry) = world.get_resource::<RouteEntityRegistry>() else {
        return AuthorityIsolationPreparation::PermanentContradiction {
            contradiction,
            fail_closed,
        };
    };
    if contradiction.violations.is_empty()
        || contradiction.violations.iter().any(|violation| {
            violation.component != AuthorityIsolationComponent::Pickable
                || violation.predicate != AuthorityIsolationPredicate::PresentForIgnoreWrite
                || !target_authority_missing_pickable_repair_is_exact(
                    world,
                    registry,
                    validation,
                    violation.entity,
                )
        })
    {
        return AuthorityIsolationPreparation::PermanentContradiction {
            contradiction,
            fail_closed,
        };
    }
    for violation in contradiction.violations {
        if let Some(entity) = fail_closed
            .entities
            .iter_mut()
            .find(|entity| entity.entity == violation.entity)
        {
            entity.insert_pickable = true;
            continue;
        }
        if try_push(
            &mut fail_closed.entities,
            PreparedAuthorityEntityIsolation {
                entity: violation.entity,
                visibility: false,
                pickable: false,
                insert_pickable: true,
                barrier: false,
            },
        )
        .is_err()
        {
            return AuthorityIsolationPreparation::RetryableCapacity;
        }
    }
    AuthorityIsolationPreparation::Ready(fail_closed)
}

pub(super) fn prepare_normal_authority_isolation(
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

pub(super) fn prepare_failed_authority_isolation(
    world: &World,
    active: Option<&RealizedBranch>,
    failed: &PendingFailedBranch,
    host: Option<&HostBinding>,
) -> AuthorityIsolationPreparation {
    prepare_failed_branch_authority_isolation(world, active, &failed.branch, host)
}

pub(super) fn prepare_failed_branch_authority_isolation(
    world: &World,
    active: Option<&RealizedBranch>,
    branch: &FailedPreparedBranch,
    host: Option<&HostBinding>,
) -> AuthorityIsolationPreparation {
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

pub(super) fn prepare_failed_manifest_protocol_isolation(
    world: &World,
    active: Option<&RealizedBranch>,
    failed: &PendingFailedBranch,
    host: Option<&HostBinding>,
) -> AuthorityIsolationPreparation {
    prepare_failed_authority_isolation(world, active, failed, host)
}

pub(super) fn prepare_projectionless_authority_isolation(
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

pub(super) fn fail_closed_cleanup_settlement_permanent(
    world: &mut World,
    entry: &mut RouterEntry,
    mut ledger: CleanupLedger,
) -> Result<(), CleanupLedger> {
    let preparation = prepare_authority_isolation_targets(
        world,
        ledger
            .exact_claims()
            .chain(entry.active.iter().flat_map(RealizedBranch::claims))
            .chain(entry.host.iter().map(|host| &host.top_slot_claim)),
        ledger
            .exact_claims()
            .map(ExactRegistryClaim::entity)
            .chain(
                entry
                    .active
                    .iter()
                    .flat_map(RealizedBranch::interactive_entities),
            )
            .chain(entry.host.iter().map(|host| host.top_slot)),
        0,
    );
    let (isolation, isolation_contradiction) = match preparation {
        AuthorityIsolationPreparation::Ready(isolation) => (isolation, None),
        AuthorityIsolationPreparation::RetryableCapacity => return Err(ledger),
        AuthorityIsolationPreparation::PermanentContradiction {
            contradiction,
            fail_closed,
        } => (fail_closed, Some(contradiction)),
    };
    let Some(evidence) = ledger.take_cleanup_settlement_permanent_evidence() else {
        return Err(ledger);
    };
    let active = entry.active.take();
    let (retired_host, lease) = match entry.host.take() {
        Some(host) => {
            let (retired, lease) = host.into_retired();
            (Some(retired), Some(lease))
        }
        None => (None, None),
    };
    let mut residual = Some(AuthorityProtocolResidualOwner {
        kind: AuthorityProtocolResidualKind::CleanupSettlementPermanent(evidence),
        isolation_contradiction,
        active,
        pending: None,
        failed: None,
        failure_cleanup_completion: None,
        failure_cleanup_ledger: None,
        cleanup_settlement_ledger: Some(ledger),
        failure_cleanup_permanent_authority: None,
        host: retired_host,
    });
    let mut isolation = Some(isolation);
    let protocol_residual = &mut entry.authority_protocol_residual;
    if let Err(returned_lease) =
        entry
            .engine
            .fail_closed_authority_protocol_retaining(lease, || {
                isolation
                    .take()
                    .expect("cleanup permanent fail-close applies isolation once")
                    .apply(world);
                *protocol_residual = residual.take();
            })
    {
        let mut owner = residual
            .take()
            .expect("rejected cleanup permanent fail-close retains its exact owner");
        match (owner.host.take(), returned_lease) {
            (Some(retired), Some(lease)) => entry.host = Some(retired.restore(lease)),
            (None, None) => {}
            _ => unreachable!("cleanup permanent fail-close returns the exact optional host lease"),
        }
        entry.active = owner.active.take();
        isolation
            .take()
            .expect("rejected cleanup permanent fail-close retains prepared isolation")
            .apply(world);
        let AuthorityProtocolResidualKind::CleanupSettlementPermanent(evidence) = owner.kind else {
            unreachable!("cleanup permanent residual retains its typed evidence")
        };
        entry.cleanup_settlement_permanent_inert = Some(CleanupSettlementPermanentInertOwner {
            evidence,
            ledger: owner
                .cleanup_settlement_ledger
                .take()
                .expect("cleanup permanent residual retains its physical ledger"),
            isolation_contradiction: owner.isolation_contradiction.take(),
        });
    }
    Ok(())
}

/*
    The owner-specific functions above only select stored claims and gate targets. All authority,
    component-presence, focus, gate, and in-place mutation preparation is shared by
    `prepare_authority_isolation_targets`.
*/

pub(super) fn fail_closed_normal_authority_protocol(
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

pub(super) fn fail_closed_normal_owner_protocol(
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

pub(super) fn fail_closed_normal_authority_protocol_prepared(
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

pub(super) fn fail_closed_pending_owner_protocol_prepared(
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
        failure_cleanup_completion: None,
        failure_cleanup_ledger: None,
        cleanup_settlement_ledger: None,
        failure_cleanup_permanent_authority: None,
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

pub(super) fn fail_closed_failed_manifest_protocol(
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

pub(super) fn fail_closed_failed_authority_protocol(
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

pub(super) fn fail_closed_failed_owner_protocol(
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

pub(super) fn fail_closed_failed_owner_protocol_prepared(
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
        failure_cleanup_completion: None,
        failure_cleanup_ledger: None,
        cleanup_settlement_ledger: None,
        failure_cleanup_permanent_authority: None,
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

pub(super) fn fail_closed_failure_cleanup_completion_protocol(
    world: &mut World,
    entry: &mut RouterEntry,
    inert: FailureCleanupCompletionInertOwner,
) -> Result<(), FailureCleanupCompletionInertOwner> {
    let branch = inert
        .ledger
        .failed_prepared_branch()
        .expect("failure completion mismatch retains its failed physical owner");
    let (isolation, isolation_contradiction) = match prepare_failed_branch_authority_isolation(
        world,
        entry.active.as_ref(),
        branch,
        entry.host.as_ref(),
    ) {
        AuthorityIsolationPreparation::Ready(isolation) => (isolation, None),
        AuthorityIsolationPreparation::RetryableCapacity => return Err(inert),
        AuthorityIsolationPreparation::PermanentContradiction {
            contradiction,
            fail_closed,
        } => (fail_closed, Some(contradiction)),
    };

    let FailureCleanupCompletionInertOwner {
        owner: completion,
        ledger,
        mismatch,
    } = inert;
    let active = entry.active.take();
    let (retired_host, lease) = match entry.host.take() {
        Some(host) => {
            let (retired, lease) = host.into_retired();
            (Some(retired), Some(lease))
        }
        None => (None, None),
    };
    let mut residual = Some(AuthorityProtocolResidualOwner {
        kind: AuthorityProtocolResidualKind::FailureCleanupCompletion(mismatch),
        isolation_contradiction,
        active,
        pending: None,
        failed: None,
        failure_cleanup_completion: completion,
        failure_cleanup_ledger: Some(ledger),
        cleanup_settlement_ledger: None,
        failure_cleanup_permanent_authority: None,
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
            .expect("rejected completion-mismatch fail-close retains its paired owners");
        match (owner.host.take(), returned_lease) {
            (Some(retired), Some(lease)) => entry.host = Some(retired.restore(lease)),
            (None, None) => {}
            _ => unreachable!("completion-mismatch fail-close returns the exact host lease"),
        }
        entry.active = owner.active.take();
        let AuthorityProtocolResidualKind::FailureCleanupCompletion(mismatch) = owner.kind else {
            unreachable!("completion mismatch retains its typed evidence")
        };
        return Err(FailureCleanupCompletionInertOwner {
            owner: owner.failure_cleanup_completion.take(),
            ledger: owner
                .failure_cleanup_ledger
                .take()
                .expect("completion mismatch retains its physical owner"),
            mismatch,
        });
    }
    Ok(())
}

pub(super) fn fail_closed_projectionless_owner_protocol(
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
