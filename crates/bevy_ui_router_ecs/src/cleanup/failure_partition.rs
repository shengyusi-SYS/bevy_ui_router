//! Freeze and revalidate the source, target, and protected partition of failed cleanup.
//!
//! The ledger retains attempts, receipts, and completion ownership; this module
//! only validates that each physical transition stays inside the frozen partition.

use std::collections::{HashMap, HashSet};

use bevy_ecs::{
    hierarchy::ChildOf,
    prelude::{Entity, World},
};
use bevy_input_focus::InputFocus;
use bevy_ui::Node;
use bevy_ui_router_core::{
    frame::RouteFrameVersion, ids::RouteSlotId, integration::RetainedLeafProjection,
};

use super::{
    CleanupMountParentSettlement, CleanupObligation, CleanupReceipt, CleanupTargetIdentity,
    FrozenCleanupManifest, FrozenFailureCleanupObservedEntity, cleanup_claim_has_exact_authority,
    mount_parent_physical_matches, observed_children,
    observed_manifest_differs_only_by_allowed_relationships,
};
use crate::{
    branch_transaction::RealizedBranch,
    input::{RouteFocusState, observed_descendant_closure},
    instance::InstancePayload,
    navigation::HostBinding,
    registry::{ComponentManifest, EntityLifecycle, ExactRegistryClaim, RouteEntityRegistry},
};

#[derive(Clone, Debug, Eq, PartialEq)]
struct FrozenFailureCleanupSource {
    manifest_digest: u64,
    targets: Vec<CleanupTargetIdentity>,
    logical: FailureCleanupSourceProjection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FailureCleanupHostSourceProjection {
    version: bevy_ui_router_core::HostVersion,
    slot: RouteSlotId,
    host: Entity,
    top_slot: Entity,
    top_slot_claim: ExactRegistryClaim,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FailureCleanupActiveSourceProjection {
    frames: Vec<RouteFrameVersion>,
    claims: Vec<ExactRegistryClaim>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FailureCleanupRetainedSourceProjection {
    projection: RetainedLeafProjection,
    claims: Vec<ExactRegistryClaim>,
    mount: crate::presentation::FrameMountBinding,
    own_slots: Vec<(
        ExactRegistryClaim,
        crate::presentation::OutletBinding,
        Vec<Entity>,
    )>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FailureCleanupSourceProjection {
    host: Option<FailureCleanupHostSourceProjection>,
    active: Option<FailureCleanupActiveSourceProjection>,
    prepared: Option<FailureCleanupActiveSourceProjection>,
    retained: Vec<FailureCleanupRetainedSourceProjection>,
}

impl FailureCleanupSourceProjection {
    pub(crate) fn observe(host: Option<&HostBinding>, active: Option<&RealizedBranch>) -> Self {
        Self::observe_with_prepared(host, active, None)
    }

    pub(crate) fn observe_with_prepared(
        host: Option<&HostBinding>,
        active: Option<&RealizedBranch>,
        prepared: Option<&RealizedBranch>,
    ) -> Self {
        Self::observe_with_selected(host, active, prepared, &[])
    }

    pub(crate) fn observe_with_selected(
        host: Option<&HostBinding>,
        active: Option<&RealizedBranch>,
        prepared: Option<&RealizedBranch>,
        selected: &[(&RetainedLeafProjection, &InstancePayload)],
    ) -> Self {
        let observe_branch = |branch: &RealizedBranch| FailureCleanupActiveSourceProjection {
            frames: branch
                .frames()
                .iter()
                .map(|frame| frame.frame().clone())
                .collect(),
            claims: branch.claims().cloned().collect(),
        };
        Self {
            host: host.map(|host| FailureCleanupHostSourceProjection {
                version: host.version.clone(),
                slot: host.slot_id,
                host: host.host,
                top_slot: host.top_slot,
                top_slot_claim: host.top_slot_claim.clone(),
            }),
            active: active.map(observe_branch),
            prepared: prepared.map(observe_branch),
            retained: selected
                .iter()
                .map(
                    |(projection, payload)| FailureCleanupRetainedSourceProjection {
                        projection: (*projection).clone(),
                        claims: payload.claims().cloned().collect(),
                        mount: payload.frame().mount_binding().clone(),
                        own_slots: payload
                            .child_slots()
                            .iter()
                            .map(|slot| {
                                (
                                    slot.slot_claim().clone(),
                                    slot.anchor_binding().clone(),
                                    slot.expected_anchor_children().to_vec(),
                                )
                            })
                            .collect(),
                    },
                )
                .collect(),
        }
    }

    fn host_entity(&self) -> Option<Entity> {
        self.host.as_ref().map(|host| host.host)
    }

    fn top_slot(&self) -> Option<Entity> {
        self.host.as_ref().map(|host| host.top_slot)
    }
}

pub(crate) struct FrozenFailureCleanupPartition {
    source: FrozenFailureCleanupSource,
    roots: Vec<Entity>,
    target_observed: Vec<FrozenFailureCleanupObservedEntity>,
    protected_observed: Vec<FrozenFailureCleanupObservedEntity>,
    allowed_writes: Vec<Entity>,
    input_focus: Option<Option<Entity>>,
    route_focus: Option<Option<Entity>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FailureCleanupTransitionStage {
    Prewrite(Entity),
    PhysicalAbsent(Entity),
    ReceiptCommitted(Entity),
    MountParentsCommitted,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum FailureCleanupPartitionRefreezeFailure {
    Source {
        expected: FailureCleanupSourceProjection,
        observed: Option<FailureCleanupSourceProjection>,
    },
    Target {
        entity: Entity,
        expected: FrozenFailureCleanupObservedEntity,
        observed: Option<FrozenFailureCleanupObservedEntity>,
    },
    Protected {
        entity: Entity,
        expected: FrozenFailureCleanupObservedEntity,
        observed: Option<FrozenFailureCleanupObservedEntity>,
    },
    Closure {
        expected: Vec<Entity>,
        observed: Vec<Entity>,
    },
    Focus {
        expected_input: Option<Option<Entity>>,
        observed_input: Option<Option<Entity>>,
        expected_route: Option<Option<Entity>>,
        observed_route: Option<Option<Entity>>,
    },
    MountParents,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FailureCleanupSettlementRefreezeEvidence {
    source: FrozenFailureCleanupSource,
    stage: FailureCleanupTransitionStage,
    failure: FailureCleanupPartitionRefreezeFailure,
}

impl FailureCleanupSettlementRefreezeEvidence {
    pub(crate) const fn stage(&self) -> FailureCleanupTransitionStage {
        self.stage
    }

    pub(crate) const fn failure(&self) -> &FailureCleanupPartitionRefreezeFailure {
        &self.failure
    }
}

impl FrozenCleanupManifest {
    pub(crate) fn freeze_failure_cleanup_partition(
        &self,
        world: &World,
        registry: &RouteEntityRegistry,
        active: Option<&RealizedBranch>,
        host: &HostBinding,
    ) -> Option<FrozenFailureCleanupPartition> {
        self.freeze_failure_cleanup_partition_retaining(world, registry, active, None, host)
    }

    pub(crate) fn freeze_failure_cleanup_partition_retaining(
        &self,
        world: &World,
        registry: &RouteEntityRegistry,
        active: Option<&RealizedBranch>,
        prepared: Option<&RealizedBranch>,
        host: &HostBinding,
    ) -> Option<FrozenFailureCleanupPartition> {
        self.freeze_failure_cleanup_partition_with_selected(
            world,
            registry,
            active,
            prepared,
            host,
            &[],
        )
    }

    pub(crate) fn freeze_failure_cleanup_partition_with_selected(
        &self,
        world: &World,
        registry: &RouteEntityRegistry,
        active: Option<&RealizedBranch>,
        prepared: Option<&RealizedBranch>,
        host: &HostBinding,
        selected: &[(&RetainedLeafProjection, &InstancePayload)],
    ) -> Option<FrozenFailureCleanupPartition> {
        let targets = self
            .obligations
            .iter()
            .map(|obligation| obligation.identity.clone())
            .collect::<Vec<_>>();
        let mut allowed_writes = targets
            .iter()
            .map(CleanupTargetIdentity::entity)
            .collect::<Vec<_>>();
        let target_count = allowed_writes.len();
        allowed_writes.sort_by_key(|entity| entity.to_bits());
        allowed_writes.dedup();
        if allowed_writes.len() != target_count
            || allowed_writes.contains(&host.host)
            || allowed_writes.contains(&host.top_slot)
            || !cleanup_claim_has_exact_authority(world, &host.top_slot_claim)
        {
            return None;
        }
        let active_claims = active
            .map(|active| active.claims().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        let prepared_claims = prepared
            .map(|prepared| prepared.claims().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        let retained_claims = selected
            .iter()
            .flat_map(|(_, payload)| payload.claims())
            .cloned()
            .collect::<Vec<_>>();
        if selected
            .iter()
            .enumerate()
            .any(|(index, (projection, payload))| {
                payload.frame().frame() != projection.frame()
                    || !payload
                        .frame()
                        .mount_binding()
                        .matches_retained_scope(projection)
                    || selected[..index]
                        .iter()
                        .any(|(prior, _)| prior.frame() == projection.frame())
            })
            || retained_claims
                .iter()
                .any(|claim| claim.lifecycle() != EntityLifecycle::Retained)
        {
            return None;
        }
        // Selected stock is a non-target source. Its original identity stays
        // frozen even when its physical payload needs local invalidation; the
        // observed protected state below must remain exact through fresh cleanup.
        if prepared_claims.iter().any(|claim| {
            claim.lifecycle() != EntityLifecycle::PreparedInert
                || registry
                    .record(claim.entity())
                    .is_none_or(|record| record.claim() != *claim)
                || !cleanup_claim_has_exact_authority(world, claim)
        }) {
            return None;
        }
        if active_claims
            .iter()
            .chain(&prepared_claims)
            .chain(&retained_claims)
            .any(|claim| allowed_writes.contains(&claim.entity()))
            || active_claims.iter().any(|claim| {
                prepared_claims
                    .iter()
                    .chain(&retained_claims)
                    .any(|other| other.entity() == claim.entity())
            })
            || prepared_claims.iter().any(|claim| {
                retained_claims
                    .iter()
                    .any(|other| other.entity() == claim.entity())
            })
            || retained_claims.iter().enumerate().any(|(index, claim)| {
                retained_claims[index + 1..]
                    .iter()
                    .any(|other| other.entity() == claim.entity())
            })
        {
            return None;
        }
        let mut roots = vec![host.host, host.top_slot];
        roots.extend(allowed_writes.iter().copied());
        roots.extend(active_claims.iter().map(ExactRegistryClaim::entity));
        roots.extend(prepared_claims.iter().map(ExactRegistryClaim::entity));
        roots.extend(retained_claims.iter().map(ExactRegistryClaim::entity));
        for (_, payload) in selected {
            roots.push(payload.frame().mount_binding().slot());
            roots.push(payload.frame().mount_binding().physical_parent().0);
        }
        roots.extend(
            self.obligations
                .iter()
                .flat_map(|obligation| [obligation.expected_parent, obligation.authorized_parent]),
        );
        roots.extend(self.mount_parents.iter().map(|parent| parent.physical));
        roots.sort_by_key(|entity| entity.to_bits());
        roots.dedup();
        let target_observed = allowed_writes
            .iter()
            .copied()
            .map(|entity| FrozenFailureCleanupObservedEntity::observe(world, registry, entity))
            .collect::<Vec<_>>();
        let frozen_absent_targets = target_observed
            .iter()
            .filter_map(|observed| (!observed.present).then_some(observed.entity))
            .collect::<Vec<_>>();
        let closure_roots = roots
            .iter()
            .copied()
            .filter(|entity| !frozen_absent_targets.contains(entity))
            .collect::<Vec<_>>();
        let closure = freeze_failure_cleanup_closure(world, &closure_roots, &allowed_writes)?;
        if closure_roots.iter().any(|root| !closure.contains(root)) {
            return None;
        }
        if frozen_absent_targets
            .iter()
            .any(|entity| closure.contains(entity))
        {
            return None;
        }
        let protected_entities = closure
            .iter()
            .copied()
            .filter(|entity| !allowed_writes.contains(entity))
            .collect::<Vec<_>>();
        if !protected_entities.contains(&host.host)
            || !protected_entities.contains(&host.top_slot)
            || active_claims
                .iter()
                .chain(&prepared_claims)
                .chain(&retained_claims)
                .any(|claim| !protected_entities.contains(&claim.entity()))
        {
            return None;
        }
        let protected_observed = protected_entities
            .iter()
            .copied()
            .map(|entity| FrozenFailureCleanupObservedEntity::observe(world, registry, entity))
            .collect::<Vec<_>>();
        if protected_observed.iter().any(|observed| {
            !observed.present
                && !retained_claims
                    .iter()
                    .any(|claim| claim.entity() == observed.entity)
        }) {
            return None;
        }
        Some(FrozenFailureCleanupPartition {
            source: FrozenFailureCleanupSource {
                manifest_digest: self.digest,
                targets,
                logical: FailureCleanupSourceProjection::observe_with_selected(
                    Some(host),
                    active,
                    prepared,
                    selected,
                ),
            },
            roots,
            target_observed,
            protected_observed,
            allowed_writes,
            input_focus: world.get_resource::<InputFocus>().map(InputFocus::get),
            route_focus: world
                .get_resource::<RouteFocusState>()
                .map(RouteFocusState::focused),
        })
    }
}

pub(super) fn freeze_failure_cleanup_closure(
    world: &World,
    roots: &[Entity],
    cycle_permitted: &[Entity],
) -> Option<Vec<Entity>> {
    let mut closure = observed_descendant_closure(world, roots.iter().copied())
        .into_iter()
        .collect::<Vec<_>>();
    closure.sort_by_key(|entity| entity.to_bits());
    let closure_set = closure.iter().copied().collect::<HashSet<_>>();
    let mut observed_parents = HashMap::<Entity, Entity>::new();
    for parent in &closure {
        let children = observed_children(world, *parent);
        let mut unique = HashSet::new();
        for child in children {
            if !unique.insert(child)
                || !closure_set.contains(&child)
                || world.get::<ChildOf>(child).map(ChildOf::parent) != Some(*parent)
                || observed_parents
                    .insert(child, *parent)
                    .is_some_and(|previous| previous != *parent)
            {
                return None;
            }
        }
    }
    for entity in &closure {
        let mut path = Vec::new();
        let mut current = *entity;
        while closure_set.contains(&current) {
            if let Some(cycle_start) = path.iter().position(|observed| *observed == current) {
                // Failed cleanup may quarantine a malformed cycle wholly inside its exact
                // target set; a cycle that reaches protected state is still invalid.
                if path[cycle_start..]
                    .iter()
                    .all(|observed| cycle_permitted.contains(observed))
                {
                    break;
                }
                return None;
            }
            path.push(current);
            let Some(parent) = world.get::<ChildOf>(current).map(ChildOf::parent) else {
                break;
            };
            current = parent;
        }
    }
    Some(closure)
}

fn failure_cleanup_observed_entity_matches(
    world: &World,
    expected: &FrozenFailureCleanupObservedEntity,
    observed: &FrozenFailureCleanupObservedEntity,
    removed: &[Entity],
    registry_manifest: Option<&ComponentManifest>,
    app_owned_host: Option<Entity>,
) -> bool {
    let expected_children = expected
        .children
        .iter()
        .copied()
        .filter(|child| !removed.contains(child))
        .collect::<Vec<_>>();
    let children_changed = expected_children != expected.children;
    let expected_children_component_present = if children_changed {
        !expected_children.is_empty()
    } else {
        expected.children_component_present
    };
    // An application owns its host components. Only routing facts constrain
    // preservation here; unrelated application markers are not cleanup authority.
    let manifest_is_exact = if app_owned_host == Some(expected.entity)
        && expected.registry_claim.is_none()
        && expected.ownership_seal.is_none()
    {
        world.get::<Node>(expected.entity).is_some()
    } else {
        match expected.manifest.as_ref() {
            Some(manifest) => observed_manifest_differs_only_by_allowed_relationships(
                world,
                expected.entity,
                manifest,
                children_changed,
                false,
            ),
            None => !expected.present && observed.manifest.is_none(),
        }
    };
    observed.entity == expected.entity
        && observed.present == expected.present
        && !expected
            .parent
            .is_some_and(|parent| removed.contains(&parent))
        && observed.parent == expected.parent
        && observed.children == expected_children
        && observed.children_component_present == expected_children_component_present
        && observed.registry_claim == expected.registry_claim
        && observed.registry_manifest.as_ref()
            == registry_manifest.or(expected.registry_manifest.as_ref())
        && observed.ownership_seal == expected.ownership_seal
        && manifest_is_exact
        && observed.visibility == expected.visibility
        && observed.pickable == expected.pickable
        && observed.barrier == expected.barrier
        && observed.frame_lifecycle == expected.frame_lifecycle
        && observed.child_slot_lifecycle == expected.child_slot_lifecycle
        && observed.child_slot_authority == expected.child_slot_authority
        && observed.provisional_slot_lifecycle == expected.provisional_slot_lifecycle
        && observed.host_registration == expected.host_registration
}

impl FrozenFailureCleanupPartition {
    fn evidence(
        &self,
        stage: FailureCleanupTransitionStage,
        failure: FailureCleanupPartitionRefreezeFailure,
    ) -> FailureCleanupSettlementRefreezeEvidence {
        FailureCleanupSettlementRefreezeEvidence {
            source: self.source.clone(),
            stage,
            failure,
        }
    }

    pub(super) fn refreeze(
        &self,
        world: &World,
        current_source: Option<&FailureCleanupSourceProjection>,
        obligations: &[CleanupObligation],
        mount_parents: &[CleanupMountParentSettlement],
        receipts: &[Option<CleanupReceipt>],
        manifest_digest: u64,
        mount_parent_next: Option<&[(ExactRegistryClaim, ComponentManifest)]>,
        stage: FailureCleanupTransitionStage,
    ) -> Result<(), FailureCleanupSettlementRefreezeEvidence> {
        let obligation_claims = obligations
            .iter()
            .map(|obligation| obligation.identity.clone())
            .collect::<Vec<_>>();
        let obligation_entities = obligation_claims
            .iter()
            .map(CleanupTargetIdentity::entity)
            .collect::<Vec<_>>();
        if current_source != Some(&self.source.logical) {
            return Err(self.evidence(
                stage,
                FailureCleanupPartitionRefreezeFailure::Source {
                    expected: self.source.logical.clone(),
                    observed: current_source.cloned(),
                },
            ));
        }
        if self.source.manifest_digest != manifest_digest
            || self.source.targets != obligation_claims
            || receipts.len() != obligations.len()
            || self.allowed_writes.len() != obligations.len()
            || obligation_entities
                .iter()
                .any(|entity| !self.allowed_writes.contains(entity))
            || self
                .target_observed
                .iter()
                .map(|observed| observed.entity)
                .collect::<Vec<_>>()
                != self.allowed_writes
            || self
                .protected_observed
                .iter()
                .any(|observed| self.allowed_writes.contains(&observed.entity))
            || !self
                .protected_observed
                .iter()
                .any(|observed| Some(observed.entity) == self.source.logical.host_entity())
            || !self
                .protected_observed
                .iter()
                .any(|observed| Some(observed.entity) == self.source.logical.top_slot())
        {
            return Err(self.evidence(
                stage,
                FailureCleanupPartitionRefreezeFailure::Source {
                    expected: self.source.logical.clone(),
                    observed: current_source.cloned(),
                },
            ));
        }
        let transition_entity = match stage {
            FailureCleanupTransitionStage::Prewrite(entity)
            | FailureCleanupTransitionStage::PhysicalAbsent(entity)
            | FailureCleanupTransitionStage::ReceiptCommitted(entity) => Some(entity),
            FailureCleanupTransitionStage::MountParentsCommitted => None,
        };
        if transition_entity.is_none() && receipts.iter().any(Option::is_none) {
            return Err(self.evidence(
                stage,
                FailureCleanupPartitionRefreezeFailure::Source {
                    expected: self.source.logical.clone(),
                    observed: current_source.cloned(),
                },
            ));
        }
        if let Some(transition_entity) = transition_entity {
            let Some(transition_index) = obligations
                .iter()
                .position(|obligation| obligation.entity() == transition_entity)
            else {
                return Err(self.evidence(
                    stage,
                    FailureCleanupPartitionRefreezeFailure::Source {
                        expected: self.source.logical.clone(),
                        observed: current_source.cloned(),
                    },
                ));
            };
            let transition_expected = self
                .target_observed
                .iter()
                .find(|expected| expected.entity == transition_entity)
                .expect("the frozen allowed-write set retains every exact target");
            let transition_is_exact = match stage {
                FailureCleanupTransitionStage::Prewrite(_) => {
                    receipts[transition_index].is_none()
                        && world.get_entity(transition_entity).is_ok()
                            == transition_expected.present
                }
                FailureCleanupTransitionStage::PhysicalAbsent(_) => {
                    receipts[transition_index].is_none()
                        && world.get_entity(transition_entity).is_err()
                }
                FailureCleanupTransitionStage::ReceiptCommitted(_) => {
                    receipts[transition_index].is_some()
                        && world.get_entity(transition_entity).is_err()
                }
                FailureCleanupTransitionStage::MountParentsCommitted => {
                    unreachable!("mount-parent final refreeze has no single target transition")
                }
            };
            if !transition_is_exact {
                let expected = transition_expected.clone();
                let observed = FrozenFailureCleanupObservedEntity::observe(
                    world,
                    world.resource::<RouteEntityRegistry>(),
                    transition_entity,
                );
                return Err(self.evidence(
                    stage,
                    FailureCleanupPartitionRefreezeFailure::Target {
                        entity: transition_entity,
                        expected,
                        observed: Some(observed),
                    },
                ));
            }
        }
        let mut removed = receipts
            .iter()
            .enumerate()
            .filter_map(|(index, receipt)| receipt.is_some().then_some(obligations[index].entity()))
            .collect::<Vec<_>>();
        if let FailureCleanupTransitionStage::PhysicalAbsent(entity) = stage {
            removed.push(entity);
        }
        removed.sort_by_key(|entity| entity.to_bits());
        removed.dedup();
        let mut unavailable = removed.clone();
        unavailable.extend(
            self.target_observed
                .iter()
                .filter_map(|observed| (!observed.present).then_some(observed.entity)),
        );
        unavailable.sort_by_key(|entity| entity.to_bits());
        unavailable.dedup();
        let registry = world.resource::<RouteEntityRegistry>();
        for expected in &self.target_observed {
            if removed.contains(&expected.entity) {
                let registry_is_exact = if matches!(
                    stage,
                    FailureCleanupTransitionStage::PhysicalAbsent(entity)
                        if entity == expected.entity
                ) {
                    registry
                        .record(expected.entity)
                        .map(|record| record.claim())
                        == expected.registry_claim
                } else {
                    registry.record(expected.entity).is_none()
                };
                if world.get_entity(expected.entity).is_ok() || !registry_is_exact {
                    let observed = FrozenFailureCleanupObservedEntity::observe(
                        world,
                        &registry,
                        expected.entity,
                    );
                    return Err(self.evidence(
                        stage,
                        FailureCleanupPartitionRefreezeFailure::Target {
                            entity: expected.entity,
                            expected: expected.clone(),
                            observed: Some(observed),
                        },
                    ));
                }
                continue;
            }
            let observed =
                FrozenFailureCleanupObservedEntity::observe(world, &registry, expected.entity);
            if !failure_cleanup_observed_entity_matches(
                world,
                expected,
                &observed,
                &unavailable,
                None,
                None,
            ) {
                let observed = Some(observed);
                return Err(self.evidence(
                    stage,
                    FailureCleanupPartitionRefreezeFailure::Target {
                        entity: expected.entity,
                        expected: expected.clone(),
                        observed,
                    },
                ));
            }
        }
        for expected in &self.protected_observed {
            let observed =
                FrozenFailureCleanupObservedEntity::observe(world, &registry, expected.entity);
            let registry_manifest = mount_parent_next.and_then(|next| {
                next.iter().find_map(|(claim, manifest)| {
                    (claim.entity() == expected.entity).then_some(manifest)
                })
            });
            if !failure_cleanup_observed_entity_matches(
                world,
                expected,
                &observed,
                &unavailable,
                registry_manifest,
                self.source.logical.host_entity(),
            ) {
                let observed = Some(observed);
                return Err(self.evidence(
                    stage,
                    FailureCleanupPartitionRefreezeFailure::Protected {
                        entity: expected.entity,
                        expected: expected.clone(),
                        observed,
                    },
                ));
            }
        }
        let mut expected_closure = self
            .protected_observed
            .iter()
            .map(|observed| observed.entity)
            .chain(
                self.target_observed
                    .iter()
                    .map(|observed| observed.entity)
                    .filter(|entity| !unavailable.contains(entity)),
            )
            .collect::<Vec<_>>();
        expected_closure.sort_by_key(|entity| entity.to_bits());
        expected_closure.dedup();
        let observed_roots = self
            .roots
            .iter()
            .copied()
            .filter(|entity| !unavailable.contains(entity))
            .collect::<Vec<_>>();
        let observed_closure =
            freeze_failure_cleanup_closure(world, &observed_roots, &self.allowed_writes)
                .unwrap_or_else(|| {
                    let mut observed = observed_descendant_closure(world, observed_roots)
                        .into_iter()
                        .collect::<Vec<_>>();
                    observed.sort_by_key(|entity| entity.to_bits());
                    observed
                });
        if observed_closure != expected_closure {
            return Err(self.evidence(
                stage,
                FailureCleanupPartitionRefreezeFailure::Closure {
                    expected: expected_closure,
                    observed: observed_closure,
                },
            ));
        }
        let observed_input = world.get_resource::<InputFocus>().map(InputFocus::get);
        let observed_route = world
            .get_resource::<RouteFocusState>()
            .map(RouteFocusState::focused);
        if observed_input != self.input_focus || observed_route != self.route_focus {
            return Err(self.evidence(
                stage,
                FailureCleanupPartitionRefreezeFailure::Focus {
                    expected_input: self.input_focus,
                    observed_input,
                    expected_route: self.route_focus,
                    observed_route,
                },
            ));
        }
        let mount_parent_next_is_exact = mount_parent_next.is_none_or(|next| {
            next.len()
                == mount_parents
                    .iter()
                    .filter(|parent| parent.bookkeeping_claim().is_some())
                    .count()
                && mount_parents
                    .iter()
                    .filter(|parent| parent.bookkeeping_claim().is_some())
                    .all(|parent| {
                        next.iter()
                            .filter(|(claim, _)| claim == &parent.claim)
                            .count()
                            == 1
                    })
        });
        if !mount_parent_next_is_exact
            || mount_parents.iter().any(|parent| {
                let expected_manifest = mount_parent_next
                    .and_then(|next| {
                        next.iter().find_map(|(claim, manifest)| {
                            (claim == &parent.claim).then_some(manifest)
                        })
                    })
                    .unwrap_or(&parent.manifest);
                parent.bookkeeping_claim().is_some()
                    && registry.record(parent.physical).is_none_or(|record| {
                        record.claim() != parent.claim || record.manifest() != expected_manifest
                    })
                    || !mount_parent_physical_matches(world, &registry, parent)
            })
        {
            return Err(self.evidence(stage, FailureCleanupPartitionRefreezeFailure::MountParents));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
