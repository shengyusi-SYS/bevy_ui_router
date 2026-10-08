use bevy_camera::visibility::Visibility;
use bevy_ecs::prelude::World;
use bevy_input_focus::InputFocus;
use bevy_picking::Pickable;
use bevy_ui_router_core::{
    integration::{
        BackTargetExecutionOwnerRef, BackTargetExecutionPairingOutcome, BackTargetPhysicalOwnerRef,
        BeginAuthoritySettlementErrorKind, CleanupManifest, FailedAuthorityTriggerError,
    },
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
        revoke_router_pointer_targets, try_prepare_input_gate_isolation,
    },
    mount::ChildSlotResidual,
    presentation::RealizedFrame,
    registry::{ComponentManifest, ExactRegistryClaim, RouteEntityRegistry},
};

use super::{
    AuthorityProtocolResidualKind, AuthorityProtocolResidualOwner,
    FailureCleanupCompletionInertOwner, FrozenPostCommitInputIsolation,
    FrozenTargetPreparationPermanentPhysicalManifest, HostBinding, PendingBackTargetExecution,
    PendingFailedBranch, PendingFailedBranchTerminal, PostCommitInputIsolationFailure,
    PreparedPostCommitInputIsolation, RetiredHostBinding, RouterEntry,
    authority_settlement_logical_roots, precommit_authority_cause,
    projectionless_fail_close_capacity_probe,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::tests::{
        active_atomic_app, assert_pointer_target_preserved, assert_pointer_target_revoked,
        seed_pointer_history,
    };

    #[test]
    fn authority_focus_isolation_preserves_original_queue_and_cause() {
        for guarded in [false, true] {
            let (mut app, target) = active_atomic_app("authority-focus-queue");
            let application_target = app.world_mut().spawn_empty().id();
            let claims = app
                .world()
                .resource::<crate::navigation::RouterNavigation>()
                .entries[0]
                .active
                .as_ref()
                .unwrap()
                .claims()
                .cloned()
                .collect::<Vec<_>>();
            let mut expected = app.world().resource::<InputFocus>().clone();
            expected.set(application_target, bevy_input_focus::FocusCause::Pressed);
            expected.set(target, bevy_input_focus::FocusCause::Navigated);
            *app.world_mut().resource_mut::<InputFocus>() = expected.clone();
            expected.clear();
            let prepared =
                prepare_authority_isolation_targets(app.world(), claims.iter(), [target], 0);
            let AuthorityIsolationPreparation::Ready(prepared) = prepared else {
                panic!("real active frame supplies exact isolation authority");
            };
            if guarded {
                assert!(prepared.apply_guarded(app.world_mut(), &|_| true));
            } else {
                prepared.apply(app.world_mut());
            }
            assert_eq!(
                app.world().resource::<InputFocus>(),
                &expected,
                "isolation must append a target-local clear, preserving the original queue and causes"
            );
        }
    }

    #[test]
    fn authority_pointer_isolation_without_gate_requires_exact_owned_targets() {
        let (mut app, target) = active_atomic_app("pointer-isolation-without-gate");
        let application_target = app.world_mut().spawn_empty().id();
        seed_pointer_history(app.world_mut(), &[target, application_target]);
        let before = app
            .world()
            .resource::<bevy_picking::events::PointerState>()
            .clone();
        let claims = app
            .world()
            .resource::<crate::navigation::RouterNavigation>()
            .entries[0]
            .active
            .as_ref()
            .unwrap()
            .claims()
            .cloned()
            .collect::<Vec<_>>();
        app.world_mut().remove_resource::<RouterInputGate>();
        let prepared = prepare_authority_isolation_targets(
            app.world(),
            claims.iter(),
            [target, application_target],
            0,
        );
        let AuthorityIsolationPreparation::Ready(prepared) = prepared else {
            panic!("the fixture must retain exact owned physical targets without a gate");
        };
        prepared.apply(app.world_mut());
        assert_pointer_target_revoked(app.world(), target);
        assert_pointer_target_preserved(app.world(), application_target, &before);
        assert!(!app.world().contains_resource::<RouterInputGate>());
    }
}

pub(super) enum AuthoritySettlementBeginOutcome {
    Begun,
    #[cfg(feature = "bsn")]
    NativeCleanupContradiction {
        active: RealizedBranch,
        pending: PendingCommit,
        error: crate::presentation::native_frame::NativeCleanupPreparationError,
    },
    RetryablePreparation {
        active: RealizedBranch,
        pending: PendingCommit,
    },
    ProtocolContradiction {
        active: RealizedBranch,
        pending: PendingCommit,
        kind: BeginAuthoritySettlementErrorKind,
    },
    TriggerContradiction {
        active: RealizedBranch,
        pending: PendingCommit,
        kind: FailedAuthorityTriggerError,
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
    begin_authority_settlement_from_source(
        world,
        entry,
        active,
        pending,
        logical_roots_override,
        AuthoritySettlementSource::Reconciliation,
    )
}

pub(super) fn begin_initial_fallback_authority_settlement(
    world: &mut World,
    entry: &mut RouterEntry,
    pending: PendingCommit,
) -> AuthoritySettlementBeginOutcome {
    begin_authority_settlement_from_source(
        world,
        entry,
        RealizedBranch::empty(),
        pending,
        None,
        AuthoritySettlementSource::InitialFallbackTask,
    )
}

pub(super) fn begin_unstarted_fallback_host_loss(
    world: &mut World,
    entry: &mut RouterEntry,
    active: RealizedBranch,
    pending: PendingCommit,
    task: &bevy_ui_router_core::integration::RootFrameTask,
) -> AuthoritySettlementBeginOutcome {
    begin_authority_settlement_from_source(
        world,
        entry,
        active,
        pending,
        None,
        AuthoritySettlementSource::UnstartedFallbackTask(task),
    )
}

enum AuthoritySettlementSource<'a> {
    Reconciliation,
    InitialFallbackTask,
    UnstartedFallbackTask(&'a bevy_ui_router_core::integration::RootFrameTask),
}

fn begin_authority_settlement_from_source(
    world: &mut World,
    entry: &mut RouterEntry,
    mut active: RealizedBranch,
    mut pending: PendingCommit,
    logical_roots_override: Option<Vec<RouteFrameVersion>>,
    source: AuthoritySettlementSource<'_>,
) -> AuthoritySettlementBeginOutcome {
    let Some(host) = entry.host.as_ref() else {
        return AuthoritySettlementBeginOutcome::ProtocolContradiction {
            active,
            pending,
            kind: BeginAuthoritySettlementErrorKind::Stale,
        };
    };
    let cause = precommit_authority_cause(world, host);
    let retained_plan = if entry.retained.is_empty() {
        None
    } else {
        if !matches!(source, AuthoritySettlementSource::Reconciliation) {
            return AuthoritySettlementBeginOutcome::ProtocolContradiction {
                active,
                pending,
                kind: BeginAuthoritySettlementErrorKind::WrongPhase,
            };
        }
        let Some(inventory) = entry.retained.inventory() else {
            return AuthoritySettlementBeginOutcome::ProtocolContradiction {
                active,
                pending,
                kind: BeginAuthoritySettlementErrorKind::InvalidManifest,
            };
        };
        match entry
            .engine
            .prepare_retained_authority_settlement_plan(&host.lease, &inventory)
        {
            Ok(plan) => Some(plan),
            Err(BeginAuthoritySettlementErrorKind::RetryablePreparation) => {
                return AuthoritySettlementBeginOutcome::RetryablePreparation { active, pending };
            }
            Err(kind) => {
                return AuthoritySettlementBeginOutcome::ProtocolContradiction {
                    active,
                    pending,
                    kind,
                };
            }
        }
    };
    let retiring = retained_plan
        .as_ref()
        .map(|plan| plan.retiring().to_vec())
        .unwrap_or_default();
    let Some(mut retained_change) = entry.retained.prepare_all(&retiring) else {
        return AuthoritySettlementBeginOutcome::RetryablePreparation { active, pending };
    };
    #[cfg(feature = "bsn")]
    for (_, branch) in retained_change.selected_mut(&mut entry.retained) {
        if let Err(error) = branch.prepare_native_cleanup(world) {
            if error.is_retryable() {
                return AuthoritySettlementBeginOutcome::RetryablePreparation { active, pending };
            }
            return AuthoritySettlementBeginOutcome::NativeCleanupContradiction {
                active,
                pending,
                error,
            };
        }
    }
    #[cfg(feature = "bsn")]
    if let Err(error) = active
        .prepare_native_cleanup(world)
        .and_then(|_| pending.branch.prepare_native_cleanup(world))
    {
        if error.is_retryable() {
            return AuthoritySettlementBeginOutcome::RetryablePreparation { active, pending };
        }
        return AuthoritySettlementBeginOutcome::NativeCleanupContradiction {
            active,
            pending,
            error,
        };
    }
    let (frozen, retained_boundaries) = {
        let registry = world.resource::<RouteEntityRegistry>();
        let retired_branches = retained_change
            .selected(&entry.retained)
            .collect::<Vec<_>>();
        let frozen = match FrozenCleanupManifest::from_authority_settlement_with_retained(
            world,
            registry,
            &pending.branch,
            &active,
            host,
            &retired_branches,
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
        };
        let boundaries = if retained_plan.is_some() {
            match registry.prepare_retained_boundaries(None, &retiring) {
                Ok(boundaries) => Some(boundaries),
                Err(()) => {
                    return AuthoritySettlementBeginOutcome::ProtocolContradiction {
                        active,
                        pending,
                        kind: BeginAuthoritySettlementErrorKind::InvalidManifest,
                    };
                }
            }
        } else {
            None
        };
        (frozen, boundaries)
    };
    let manifest_digest = frozen.digest();
    let obligation_count = frozen.len();
    let frozen_targets = frozen.obligation_entities();
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

    let isolation_preparation = if retained_plan.is_some() {
        let capacity_probe = projectionless_fail_close_capacity_probe(world);
        prepare_frozen_authority_isolation(
            world,
            pending
                .branch
                .as_realized()
                .claims()
                .chain(active.claims())
                .chain(
                    retained_change
                        .selected(&entry.retained)
                        .flat_map(|(_, branch)| branch.claims()),
                )
                .chain(std::iter::once(&host.top_slot_claim)),
            frozen_targets,
            capacity_probe,
        )
    } else {
        prepare_normal_authority_isolation(world, &active, &pending.branch, Some(host))
    };
    let isolation = match isolation_preparation {
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

    // All physical preparation precedes minting the one-shot trigger. The
    // original task remains in pending until the retaining callback succeeds.
    let trigger = match source {
        AuthoritySettlementSource::Reconciliation => None,
        AuthoritySettlementSource::InitialFallbackTask
        | AuthoritySettlementSource::UnstartedFallbackTask(_) => {
            let result = match source {
                AuthoritySettlementSource::InitialFallbackTask => pending
                    .final_task
                    .as_ref()
                    .ok_or(FailedAuthorityTriggerError::WrongPhase)
                    .and_then(|task| entry.engine.failed_authority_trigger_for_task(task)),
                AuthoritySettlementSource::UnstartedFallbackTask(task) => {
                    if entry.engine.active().kind()
                        == bevy_ui_router_core::prelude::ActivePresentationKind::None
                    {
                        entry.engine.failed_authority_trigger_for_task(task)
                    } else {
                        entry
                            .engine
                            .host_loss_trigger_for_unstarted_fallback_with_active(task)
                    }
                }
                AuthoritySettlementSource::Reconciliation => unreachable!(),
            };
            match result {
                Ok(trigger) => Some(trigger),
                Err(kind) => {
                    return AuthoritySettlementBeginOutcome::TriggerContradiction {
                        active,
                        pending,
                        kind,
                    };
                }
            }
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
    let logical_roots = retained_plan
        .as_ref()
        .map(|plan| plan.logical_roots().to_vec())
        .unwrap_or_else(|| {
            logical_roots_override
                .unwrap_or_else(|| authority_settlement_logical_roots(&pending.branch, &active))
        });
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
    let owners = retained_change.take_owners(&mut entry.retained);
    let engine = &mut entry.engine;
    let cleanup_ledger = &mut entry.cleanup_ledger;
    let retained = &mut entry.retained;
    let manifest = CleanupManifest::new(logical_roots, manifest_digest, obligation_count);
    let result = world.resource_scope(
        |world, mut registry: bevy_ecs::change_detection::Mut<RouteEntityRegistry>| {
            let publish = |batch| {
                isolation.apply(world);
                let retained = retained_change.into_payloads(retained);
                if let Some(boundaries) = retained_boundaries {
                    boundaries.apply(&mut registry);
                }
                let (reserved, active, pending, owner) = settlement_owner
                    .take()
                    .expect("authority publication consumes its composed owner once");
                *cleanup_ledger = Some(CleanupLedger::activate_authority_settlement_with_retained(
                    reserved.bind_batch(batch),
                    active,
                    FailedPreparedBranch::new(pending.branch),
                    owner,
                    host_children,
                    retained,
                ));
            };
            if let Some(plan) = retained_plan {
                engine
                    .begin_authority_settlement_with_retention(
                        lease, cause, plan, manifest, owners, publish,
                    )
                    .map_err(|error| {
                        let (lease, _plan, _manifest, owners, kind) = error.into_parts();
                        (lease, owners, kind)
                    })
            } else {
                let result = match trigger {
                    Some(trigger) => engine.begin_failed_authority_settlement_for_cause_retaining(
                        lease, trigger, cause, manifest, publish,
                    ),
                    None => engine.begin_authority_settlement_for_cause_retaining(
                        lease, cause, manifest, publish,
                    ),
                };
                result.map_err(|error| {
                    let kind = error.kind();
                    (error.into_owner(), owners, kind)
                })
            }
        },
    );
    match result {
        Ok(_) => AuthoritySettlementBeginOutcome::Begun,
        Err((lease, owners, kind)) => {
            assert!(
                entry.retained.restore_owners(owners).is_ok(),
                "rejected authority transfer returns the complete original cache capabilities"
            );
            entry.host = Some(HostBinding {
                version,
                slot_id,
                host: host_entity,
                top_slot,
                top_slot_claim,
                candidate,
                registration_incarnation,
                lease,
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
    pub(super) native_shell_node: bool,
}

pub(super) struct PreparedAuthorityIsolation {
    pub(super) entities: Vec<PreparedAuthorityEntityIsolation>,
    pub(super) focus: Vec<bevy_ecs::entity::Entity>,
    pub(super) gate: Option<PreparedInputGateIsolation>,
    pointer_targets: std::collections::HashSet<bevy_ecs::entity::Entity>,
}

impl PreparedAuthorityIsolation {
    pub(super) fn apply(self, world: &mut World) {
        revoke_router_pointer_targets(world, &self.pointer_targets);
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
            focus.clear();
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
            if entity.native_shell_node {
                world
                    .get_mut::<bevy_ui::Node>(entity.entity)
                    .expect("prepared native shell retains Node")
                    .display = bevy_ui::Display::None;
            }
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
        revoke_router_pointer_targets(world, &self.pointer_targets);
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
            focus.clear();
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
            if entity.native_shell_node {
                world
                    .get_mut::<bevy_ui::Node>(entity.entity)
                    .expect("prepared native shell retains Node")
                    .display = bevy_ui::Display::None;
                if !guard(world) {
                    return false;
                }
            }
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

pub(super) fn prepare_idle_host_isolation<'a>(
    world: &World,
    claims: impl IntoIterator<Item = &'a ExactRegistryClaim>,
    targets: Vec<bevy_ecs::entity::Entity>,
) -> AuthorityIsolationPreparation {
    prepare_frozen_authority_isolation(world, claims, targets, 0)
}

fn prepare_frozen_authority_isolation<'a>(
    world: &World,
    claims: impl IntoIterator<Item = &'a ExactRegistryClaim>,
    targets: Vec<bevy_ecs::entity::Entity>,
    capacity_probe: usize,
) -> AuthorityIsolationPreparation {
    let mut preparation =
        prepare_authority_isolation_targets(world, claims, targets.iter().copied(), capacity_probe);
    // Authority cleanup owns the frozen target set. A protected descendant is not an
    // authorized focus target simply because an old owned ancestor reaches it.
    match &mut preparation {
        AuthorityIsolationPreparation::Ready(prepared)
        | AuthorityIsolationPreparation::PermanentContradiction {
            fail_closed: prepared,
            ..
        } => {
            prepared.focus.retain(|entity| targets.contains(entity));
        }
        AuthorityIsolationPreparation::RetryableCapacity => {}
    }
    preparation
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
    for entity in exact.iter().copied() {
        #[cfg(feature = "bsn")]
        if crate::presentation::native_frame::is_native_content(world, entity) {
            continue;
        }
        #[cfg(feature = "bsn")]
        let native_shell_node = crate::presentation::native_frame::is_native_shell(world, entity)
            && world
                .get::<bevy_ui::Node>(entity)
                .is_some_and(|node| node.display != bevy_ui::Display::None);
        #[cfg(not(feature = "bsn"))]
        let native_shell_node = false;
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
        if (visibility || pickable || barrier || native_shell_node)
            && try_push(
                &mut entities,
                PreparedAuthorityEntityIsolation {
                    entity,
                    visibility,
                    pickable,
                    insert_pickable: false,
                    barrier,
                    native_shell_node,
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
    let mut pointer_targets = std::collections::HashSet::new();
    let revoked = gate
        .as_ref()
        .map(PreparedInputGateIsolation::revoked_targets)
        .unwrap_or_default();
    let Some(pointer_capacity) = exact.len().checked_add(revoked.len()) else {
        return AuthorityIsolationPreparation::RetryableCapacity;
    };
    if pointer_targets.try_reserve(pointer_capacity).is_err() {
        return AuthorityIsolationPreparation::RetryableCapacity;
    }
    // A gate-removal candidate may be a protected app descendant, for which
    // removing a nonexistent Router permission was a no-op. Native writes need
    // actual revoked permission history or an exact target ownership claim.
    // Exact targets still provide authority when the gate resource is absent.
    pointer_targets.extend(exact);
    pointer_targets.extend(revoked.iter().copied());
    let prepared = PreparedAuthorityIsolation {
        entities,
        focus,
        gate,
        pointer_targets,
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
    let mut gate_targets = active.live_interactive_entities(world);
    // Protected descendants participate in read-only partition validation, not
    // input revocation. Only the original owned cleanup obligations are targets.
    gate_targets.extend(original.immutable_entities().iter().copied());
    gate_targets.sort_by_key(|entity| entity.to_bits());
    gate_targets.dedup();
    let gate_required = world.get_resource::<RouterInputGate>().is_some();
    let preparation = prepare_authority_isolation_targets(
        world,
        authority
            .partition()
            .targets()
            .iter()
            .filter_map(|target| target.registered_claim()),
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
                native_shell_node: false,
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
            .live_interactive_entities(world)
            .into_iter()
            .chain(pending.as_realized().live_interactive_entities(world)),
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
    prepare_failed_branch_authority_isolation_with_retained(world, active, branch, host, &[], None)
}

pub(super) fn prepare_failed_manifest_authority_isolation_with_retained(
    world: &World,
    active: Option<&RealizedBranch>,
    failed: &PendingFailedBranch,
    host: Option<&HostBinding>,
    retained: &[&RealizedBranch],
    targets: Vec<bevy_ecs::entity::Entity>,
) -> AuthorityIsolationPreparation {
    prepare_failed_branch_authority_isolation_with_retained(
        world,
        active,
        &failed.branch,
        host,
        retained,
        Some(targets),
    )
}

fn prepare_failed_branch_authority_isolation_with_retained(
    world: &World,
    active: Option<&RealizedBranch>,
    branch: &FailedPreparedBranch,
    host: Option<&HostBinding>,
    retained: &[&RealizedBranch],
    frozen_targets: Option<Vec<bevy_ecs::entity::Entity>>,
) -> AuthorityIsolationPreparation {
    #[cfg(feature = "bsn")]
    let frozen_targets = if let Some(mut targets) = frozen_targets {
        if let Some(failure) = branch.native_failure() {
            if targets.try_reserve(failure.claims().count()).is_err() {
                return AuthorityIsolationPreparation::RetryableCapacity;
            }
            targets.extend(failure.entities());
            targets.sort_by_key(|entity| entity.to_bits());
            targets.dedup();
        }
        Some(targets)
    } else {
        None
    };
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
        .chain(retained.iter().flat_map(|branch| branch.claims()))
        .chain(host.into_iter().map(|host| &host.top_slot_claim));
    #[cfg(feature = "bsn")]
    let stored_claims = stored_claims.chain(
        branch
            .native_failure()
            .into_iter()
            .flat_map(|failure| failure.claims())
            .filter(|claim| {
                matches!(
                    claim.owner(),
                    crate::registry::RegistryOwner::Committed(
                        crate::registry::EntityOwner::FrameShell(_)
                    )
                )
            }),
    );
    let gate_targets = active
        .into_iter()
        .flat_map(|branch| branch.live_interactive_entities(world))
        .chain(
            branch
                .frames()
                .iter()
                .chain(branch.detached_frames())
                .flat_map(|frame| frame.live_interactive_entities(world)),
        )
        .chain(
            retained
                .iter()
                .flat_map(|branch| branch.live_interactive_entities(world)),
        )
        .chain(host.into_iter().map(|host| host.top_slot));
    #[cfg(feature = "bsn")]
    let gate_targets = gate_targets.chain(
        branch
            .native_failure()
            .into_iter()
            .flat_map(|failure| failure.entities()),
    );
    let mut preparation = if let Some(targets) = frozen_targets {
        prepare_frozen_authority_isolation(world, stored_claims, targets, 0)
    } else {
        prepare_authority_isolation_targets(world, stored_claims, gate_targets, 0)
    };
    #[cfg(feature = "bsn")]
    if let Some(failure) = branch.native_failure() {
        let isolation = match &mut preparation {
            AuthorityIsolationPreparation::Ready(isolation) => Some(isolation),
            AuthorityIsolationPreparation::PermanentContradiction { fail_closed, .. } => {
                Some(fail_closed)
            }
            AuthorityIsolationPreparation::RetryableCapacity => None,
        };
        if let Some(isolation) = isolation {
            let claims = failure
                .claims()
                .filter(|claim| {
                    world
                        .get_resource::<RouteEntityRegistry>()
                        .and_then(|registry| registry.record(claim.entity()))
                        .is_some_and(|record| record.claim() == **claim)
                })
                .collect::<Vec<_>>();
            if isolation.focus.try_reserve(claims.len()).is_err()
                || isolation.pointer_targets.try_reserve(claims.len()).is_err()
            {
                return AuthorityIsolationPreparation::RetryableCapacity;
            }
            for claim in claims {
                isolation.focus.push(claim.entity());
                isolation.pointer_targets.insert(claim.entity());
            }
            isolation.focus.sort_by_key(|entity| entity.to_bits());
            isolation.focus.dedup();
        }
    }
    preparation
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
        pending.as_realized().live_interactive_entities(world),
        0,
    )
}

pub(super) fn fail_closed_cleanup_settlement_permanent(
    world: &mut World,
    entry: &mut RouterEntry,
    ledger: CleanupLedger,
) -> Result<(), CleanupLedger> {
    fail_closed_cleanup_ledger_protocol(world, entry, ledger, None)
}

pub(super) fn fail_closed_cleanup_ledger_protocol(
    world: &mut World,
    entry: &mut RouterEntry,
    mut ledger: CleanupLedger,
    kind: Option<AuthorityProtocolResidualKind>,
) -> Result<(), CleanupLedger> {
    let preparation = if let Some(branch) = ledger.failure_owner_branch() {
        prepare_failed_branch_authority_isolation(
            world,
            entry.active.as_ref(),
            branch,
            entry.host.as_ref(),
        )
    } else {
        prepare_authority_isolation_targets(
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
                        .flat_map(|branch| branch.live_interactive_entities(world)),
                )
                .chain(entry.host.iter().map(|host| host.top_slot)),
            0,
        )
    };
    let (isolation, isolation_contradiction) = match preparation {
        AuthorityIsolationPreparation::Ready(isolation) => (isolation, None),
        AuthorityIsolationPreparation::RetryableCapacity => return Err(ledger),
        AuthorityIsolationPreparation::PermanentContradiction {
            contradiction,
            fail_closed,
        } => (fail_closed, Some(contradiction)),
    };
    let kind = match kind {
        Some(kind) => kind,
        None if ledger.is_stopped() => AuthorityProtocolResidualKind::CleanupStopped,
        None => return Err(ledger),
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
        back_target: None,
        kind,
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
        return Err(owner
            .cleanup_settlement_ledger
            .take()
            .expect("rejected cleanup termination returns its ledger and stop evidence"));
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
        back_target: None,
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
    if let Some(stage) = entry.pending_back_target_execution.as_ref() {
        let PendingBackTargetExecution::Driving(driving) = stage else {
            return Err(failed);
        };
        let task = match &failed.terminal {
            PendingFailedBranchTerminal::Task { task, .. }
            | PendingFailedBranchTerminal::TaskWithPhysicalResidual { task, .. } => task,
            _ => return Err(failed),
        };
        if !matches!(
            entry.engine.validate_back_target_execution_pairing(
                BackTargetExecutionOwnerRef::Driving {
                    history: &driving.history,
                    continuation: &driving.continuation
                },
                BackTargetPhysicalOwnerRef::FrameTask(task),
            ),
            BackTargetExecutionPairingOutcome::Matched
        ) {
            return Err(failed);
        }
    }
    let active = entry.active.take();
    let (retired_host, lease) = match entry.host.take() {
        Some(host) => {
            let (retired, lease) = host.into_retired();
            (Some(retired), Some(lease))
        }
        None => (None, None),
    };
    let mut residual = Some(AuthorityProtocolResidualOwner {
        back_target: entry.pending_back_target_execution.take().map(Box::new),
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
        entry.pending_back_target_execution = owner.back_target.take().map(|owner| *owner);
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
        back_target: None,
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
