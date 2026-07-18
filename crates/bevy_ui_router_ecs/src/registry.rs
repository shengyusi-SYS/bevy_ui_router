#![allow(dead_code)]

use std::sync::atomic::{AtomicU64, Ordering};

use bevy_ecs::{
    component::ComponentId,
    prelude::{Entity, Resource, World},
};
use bevy_ui_router_core::{
    HostVersion, frame::RouteFrameVersion, ids::RouteSlotId,
    integration::frame_versions_share_router, slot::RouteSlotKey,
};

use crate::cleanup::VerifiedCleanupQuarantineTransfer;
use crate::host::HostCandidateRevision;
use crate::host::VerifiedQuarantineTransfer;
use crate::presentation::VerifiedFrameQuarantineTransfer;

static NEXT_SEAL_DOMAIN: AtomicU64 = AtomicU64::new(1);
static NEXT_QUARANTINE_DOMAIN: AtomicU64 = AtomicU64::new(1);

#[cfg(test)]
thread_local! {
    static FAIL_NEXT_UNRESOLVED_PREPARATION: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FAIL_NEXT_VERIFIED_PREPARATION: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
pub(crate) fn fail_next_unresolved_preparation_for_test() {
    FAIL_NEXT_UNRESOLVED_PREPARATION.set(true);
}

#[cfg(test)]
pub(crate) fn fail_next_verified_preparation_for_test() {
    FAIL_NEXT_VERIFIED_PREPARATION.set(true);
}

#[cfg(test)]
fn verified_preparation_fault_injected() -> bool {
    FAIL_NEXT_VERIFIED_PREPARATION.replace(false)
}

#[cfg(not(test))]
const fn verified_preparation_fault_injected() -> bool {
    false
}

#[cfg(test)]
fn unresolved_preparation_fault_injected() -> bool {
    FAIL_NEXT_UNRESOLVED_PREPARATION.replace(false)
}

#[cfg(not(test))]
const fn unresolved_preparation_fault_injected() -> bool {
    false
}

fn allocate_registry_domain(counter: &AtomicU64) -> Option<u64> {
    counter
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1).filter(|_| current != 0)
        })
        .ok()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum EntityOwner {
    HostTopSlot {
        host: HostVersion,
        slot: RouteSlotId,
    },
    ChildSlot {
        parent: RouteFrameVersion,
        outlet: RouteSlotKey,
        slot: RouteSlotId,
    },
    FrameShell(RouteFrameVersion),
    FrameContentTarget(RouteFrameVersion),
    FrameOutput {
        frame: RouteFrameVersion,
        output: u64,
        local_index: usize,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProvisionalTopSlotOwner {
    candidate_revision: HostCandidateRevision,
}

impl ProvisionalTopSlotOwner {
    pub(crate) const fn new(candidate_revision: HostCandidateRevision) -> Self {
        Self { candidate_revision }
    }
    pub(crate) const fn candidate_revision(&self) -> HostCandidateRevision {
        self.candidate_revision
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ProvisionalOwner {
    HostTopSlot(ProvisionalTopSlotOwner),
    ChildSlot {
        parent: RouteFrameVersion,
        outlet: RouteSlotKey,
        slot: RouteSlotId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RegistryOwner {
    Committed(EntityOwner),
    Provisional(ProvisionalOwner),
}

impl RegistryOwner {
    fn child_binding(&self) -> Option<(&RouteFrameVersion, &RouteSlotKey, RouteSlotId)> {
        match self {
            Self::Committed(EntityOwner::ChildSlot {
                parent,
                outlet,
                slot,
            })
            | Self::Provisional(ProvisionalOwner::ChildSlot {
                parent,
                outlet,
                slot,
            }) => Some((parent, outlet, *slot)),
            _ => None,
        }
    }

    pub(crate) fn conflicts_with(&self, other: &Self) -> bool {
        if self == other {
            return true;
        }
        let (
            Some((left_parent, left_outlet, left_slot)),
            Some((right_parent, right_outlet, right_slot)),
        ) = (self.child_binding(), other.child_binding())
        else {
            return false;
        };
        (left_parent == right_parent && left_outlet == right_outlet)
            || (frame_versions_share_router(left_parent, right_parent) && left_slot == right_slot)
    }

    fn allows_exact_owner_transition(&self, next: &Self) -> bool {
        match (self, next) {
            (
                Self::Provisional(ProvisionalOwner::ChildSlot {
                    parent: current_parent,
                    outlet: current_outlet,
                    slot: current_slot,
                }),
                Self::Committed(EntityOwner::ChildSlot {
                    parent: next_parent,
                    outlet: next_outlet,
                    slot: next_slot,
                }),
            ) => {
                current_parent == next_parent
                    && current_outlet == next_outlet
                    && current_slot == next_slot
            }
            (Self::Provisional(ProvisionalOwner::ChildSlot { .. }), _) => false,
            _ => true,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct OwnershipSeal {
    domain: u64,
    sequence: u64,
}

impl OwnershipSeal {
    pub(crate) const fn stable_identity(self) -> (u64, u64) {
        (self.domain, self.sequence)
    }

    #[cfg(test)]
    const fn fixture(domain: u64, sequence: u64) -> Self {
        Self { domain, sequence }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ComponentManifest {
    component_ids: Vec<ComponentId>,
}

impl ComponentManifest {
    pub(crate) fn new(ids: impl IntoIterator<Item = ComponentId>) -> Self {
        let mut component_ids = ids.into_iter().collect::<Vec<_>>();
        component_ids.sort_unstable();
        component_ids.dedup();
        Self { component_ids }
    }

    pub(crate) fn from_world(world: &World, entity: Entity) -> Option<Self> {
        world
            .get_entity(entity)
            .ok()
            .map(|entity| Self::new(entity.archetype().components().iter().copied()))
    }

    pub(crate) fn component_ids(&self) -> &[ComponentId] {
        &self.component_ids
    }

    pub(crate) fn matches(&self, world: &World, entity: Entity) -> bool {
        Self::from_world(world, entity).is_some_and(|actual| actual == *self)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EntityLifecycle {
    PreparedInert,
    Active,
    Leaving,
    Quarantined,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RouteEntityRecord {
    entity: Entity,
    owner: RegistryOwner,
    seal: OwnershipSeal,
    manifest: ComponentManifest,
    lifecycle: EntityLifecycle,
}

impl RouteEntityRecord {
    pub(crate) const fn entity(&self) -> Entity {
        self.entity
    }
    pub(crate) const fn owner(&self) -> &RegistryOwner {
        &self.owner
    }
    pub(crate) const fn seal(&self) -> OwnershipSeal {
        self.seal
    }
    pub(crate) const fn manifest(&self) -> &ComponentManifest {
        &self.manifest
    }
    pub(crate) const fn lifecycle(&self) -> EntityLifecycle {
        self.lifecycle
    }
    pub(crate) fn claim(&self) -> ExactRegistryClaim {
        ExactRegistryClaim {
            entity: self.entity,
            owner: self.owner.clone(),
            seal: self.seal,
            lifecycle: self.lifecycle,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ExactRegistryClaim {
    entity: Entity,
    owner: RegistryOwner,
    seal: OwnershipSeal,
    lifecycle: EntityLifecycle,
}

pub(crate) struct PreparedLifecycleMutation {
    updates: Vec<(usize, EntityLifecycle)>,
    resulting_claims: Vec<ExactRegistryClaim>,
}

impl PreparedLifecycleMutation {
    pub(crate) fn resulting_claims(&self) -> &[ExactRegistryClaim] {
        &self.resulting_claims
    }

    pub(crate) fn apply(self, registry: &mut RouteEntityRegistry) {
        for (index, lifecycle) in self.updates {
            registry.records[index].lifecycle = lifecycle;
        }
    }
}

impl ExactRegistryClaim {
    pub(crate) const fn entity(&self) -> Entity {
        self.entity
    }

    pub(crate) const fn owner(&self) -> &RegistryOwner {
        &self.owner
    }

    pub(crate) const fn seal(&self) -> OwnershipSeal {
        self.seal
    }

    pub(crate) const fn lifecycle(&self) -> EntityLifecycle {
        self.lifecycle
    }

    pub(crate) const fn opaque_digest_part(&self) -> u64 {
        self.entity.to_bits()
            ^ self.seal.domain.rotate_left(11)
            ^ self.seal.sequence.rotate_left(29)
            ^ (self.lifecycle as u64).rotate_left(47)
    }

    #[cfg(test)]
    pub(crate) fn with_entity_for_test(&self, entity: Entity) -> Self {
        Self {
            entity,
            ..self.clone()
        }
    }

    #[cfg(test)]
    pub(crate) fn with_owner_for_test(&self, owner: RegistryOwner) -> Self {
        Self {
            owner,
            ..self.clone()
        }
    }

    #[cfg(test)]
    pub(crate) fn with_seal_for_test(&self, seal: OwnershipSeal) -> Self {
        Self {
            seal,
            ..self.clone()
        }
    }

    #[cfg(test)]
    pub(crate) fn with_lifecycle_for_test(&self, lifecycle: EntityLifecycle) -> Self {
        Self {
            lifecycle,
            ..self.clone()
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RegistryWriteError {
    ForeignOrUnissuedSeal,
    DuplicateEntity,
    DuplicateOwner,
    DuplicateSeal,
}

#[derive(Resource)]
pub(crate) struct RouteEntityRegistry {
    seal_domain: u64,
    next_seal_sequence: u64,
    consumed_seal_sequences: Vec<u64>,
    records: Vec<RouteEntityRecord>,
}

impl RouteEntityRegistry {
    pub(crate) fn new() -> Self {
        Self {
            seal_domain: allocate_registry_domain(&NEXT_SEAL_DOMAIN)
                .expect("ownership seal domain exhausted"),
            next_seal_sequence: 0,
            consumed_seal_sequences: Vec::new(),
            records: Vec::new(),
        }
    }

    pub(crate) fn issue_seal(&mut self) -> OwnershipSeal {
        self.next_seal_sequence = self
            .next_seal_sequence
            .checked_add(1)
            .expect("ownership seal sequence exhausted");
        OwnershipSeal {
            domain: self.seal_domain,
            sequence: self.next_seal_sequence,
        }
    }

    #[cfg(test)]
    pub(crate) fn latest_issued_seal(&self) -> Option<OwnershipSeal> {
        (self.next_seal_sequence != 0).then_some(OwnershipSeal {
            domain: self.seal_domain,
            sequence: self.next_seal_sequence,
        })
    }

    pub(crate) fn register(
        &mut self,
        entity: Entity,
        owner: RegistryOwner,
        seal: OwnershipSeal,
        manifest: ComponentManifest,
        lifecycle: EntityLifecycle,
    ) -> Result<(), RegistryWriteError> {
        if seal.domain != self.seal_domain
            || seal.sequence == 0
            || seal.sequence > self.next_seal_sequence
        {
            return Err(RegistryWriteError::ForeignOrUnissuedSeal);
        }
        if self.consumed_seal_sequences.contains(&seal.sequence) {
            return Err(RegistryWriteError::DuplicateSeal);
        }
        if self.records.iter().any(|record| record.entity == entity) {
            return Err(RegistryWriteError::DuplicateEntity);
        }
        if self
            .records
            .iter()
            .any(|record| record.owner.conflicts_with(&owner))
        {
            return Err(RegistryWriteError::DuplicateOwner);
        }
        self.consumed_seal_sequences.push(seal.sequence);
        self.records.push(RouteEntityRecord {
            entity,
            owner,
            seal,
            manifest,
            lifecycle,
        });
        Ok(())
    }

    pub(crate) fn record(&self, entity: Entity) -> Option<&RouteEntityRecord> {
        self.records.iter().find(|record| record.entity == entity)
    }

    pub(crate) fn records(&self) -> &[RouteEntityRecord] {
        &self.records
    }

    pub(crate) fn compare_exchange_lifecycle(
        &mut self,
        claim: &ExactRegistryClaim,
        lifecycle: EntityLifecycle,
    ) -> Option<ExactRegistryClaim> {
        let Some(record) = self
            .records
            .iter_mut()
            .find(|record| record.claim() == *claim)
        else {
            return None;
        };
        record.lifecycle = lifecycle;
        Some(record.claim())
    }

    pub(crate) fn prepare_lifecycle_mutation(
        &self,
        transitions: &[(ExactRegistryClaim, EntityLifecycle)],
    ) -> Option<PreparedLifecycleMutation> {
        let mut updates = Vec::with_capacity(transitions.len());
        let mut resulting_claims = Vec::with_capacity(transitions.len());
        for (claim, lifecycle) in transitions {
            let index = self
                .records
                .iter()
                .position(|record| record.claim() == *claim)?;
            if updates
                .iter()
                .any(|(stored_index, _)| *stored_index == index)
            {
                return None;
            }
            let mut resulting = claim.clone();
            resulting.lifecycle = *lifecycle;
            updates.push((index, *lifecycle));
            resulting_claims.push(resulting);
        }
        Some(PreparedLifecycleMutation {
            updates,
            resulting_claims,
        })
    }

    pub(crate) fn compare_exchange_owner(
        &mut self,
        claim: &ExactRegistryClaim,
        owner: RegistryOwner,
    ) -> Option<ExactRegistryClaim> {
        if !claim.owner.allows_exact_owner_transition(&owner)
            || self
                .records
                .iter()
                .any(|record| record.entity != claim.entity && record.owner.conflicts_with(&owner))
        {
            return None;
        }
        let record = self
            .records
            .iter_mut()
            .find(|record| record.claim() == *claim)?;
        record.owner = owner;
        Some(record.claim())
    }

    pub(crate) fn compare_exchange_manifest(
        &mut self,
        claim: &ExactRegistryClaim,
        manifest: ComponentManifest,
    ) -> Option<ExactRegistryClaim> {
        let record = self
            .records
            .iter_mut()
            .find(|record| record.claim() == *claim)?;
        record.manifest = manifest;
        Some(record.claim())
    }

    pub(crate) fn compare_exchange_manifest_exact(
        &mut self,
        claim: &ExactRegistryClaim,
        expected_old: &ComponentManifest,
        manifest: ComponentManifest,
    ) -> bool {
        self.compare_exchange_manifests_exact(&[(claim.clone(), expected_old.clone(), manifest)])
    }

    pub(crate) fn compare_exchange_manifests_exact(
        &mut self,
        exchanges: &[(ExactRegistryClaim, ComponentManifest, ComponentManifest)],
    ) -> bool {
        let mut indices = Vec::with_capacity(exchanges.len());
        for (claim, expected_old, _) in exchanges {
            let Some(index) = self
                .records
                .iter()
                .position(|record| record.claim() == *claim && record.manifest() == expected_old)
            else {
                return false;
            };
            if indices.contains(&index) {
                return false;
            }
            indices.push(index);
        }
        for (index, (_, _, manifest)) in indices.into_iter().zip(exchanges) {
            self.records[index].manifest = manifest.clone();
        }
        true
    }

    pub(crate) fn remove_exact(&mut self, claim: &ExactRegistryClaim) -> bool {
        let before = self.records.len();
        self.records.retain(|record| record.claim() != *claim);
        self.records.len() != before
    }

    fn take_exact_set(&mut self, claims: &[ExactRegistryClaim]) -> Option<Vec<RouteEntityRecord>> {
        if claims
            .iter()
            .any(|claim| !self.records.iter().any(|record| record.claim() == *claim))
        {
            return None;
        }
        let mut records = claims
            .iter()
            .filter_map(|claim| {
                self.records
                    .iter()
                    .find(|record| record.claim() == *claim)
                    .cloned()
            })
            .collect::<Vec<_>>();
        records.sort_by_key(|record| record.entity.to_bits());
        records.dedup_by_key(|record| record.entity);
        if records.len() != claims.len() {
            return None;
        }
        self.records
            .retain(|record| !claims.iter().any(|claim| record.claim() == *claim));
        Some(records)
    }
}

impl Default for RouteEntityRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct QuarantineLeaseId {
    domain: u64,
    sequence: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct QuarantineFacts {
    former_app_host: Option<Entity>,
    protected_parent: Option<Entity>,
    foreign_descendants: Vec<Entity>,
    foreign_components: Vec<ComponentId>,
    missing_components: Vec<ComponentId>,
}

impl QuarantineFacts {
    pub(crate) fn new(
        former_app_host: Option<Entity>,
        protected_parent: Option<Entity>,
        mut foreign_descendants: Vec<Entity>,
        mut foreign_components: Vec<ComponentId>,
    ) -> Self {
        foreign_descendants.sort_by_key(|entity| entity.to_bits());
        foreign_descendants.dedup();
        foreign_components.sort_unstable();
        foreign_components.dedup();
        Self {
            former_app_host,
            protected_parent,
            foreign_descendants,
            foreign_components,
            missing_components: Vec::new(),
        }
    }

    pub(crate) fn with_component_drift(
        former_app_host: Option<Entity>,
        protected_parent: Option<Entity>,
        mut foreign_descendants: Vec<Entity>,
        mut foreign_components: Vec<ComponentId>,
        mut missing_components: Vec<ComponentId>,
    ) -> Self {
        foreign_descendants.sort_by_key(|entity| entity.to_bits());
        foreign_descendants.dedup();
        foreign_components.sort_unstable();
        foreign_components.dedup();
        missing_components.sort_unstable();
        missing_components.dedup();
        Self {
            former_app_host,
            protected_parent,
            foreign_descendants,
            foreign_components,
            missing_components,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct QuarantineLease {
    id: QuarantineLeaseId,
    generation: u64,
    operation_sequence: u64,
    records: Vec<RouteEntityRecord>,
    facts: QuarantineFacts,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct UnresolvedResidual {
    sequence: u64,
    records: Vec<RouteEntityRecord>,
    actual_manifests: Vec<(Entity, ComponentManifest)>,
    facts: QuarantineFacts,
}

impl UnresolvedResidual {
    #[cfg(test)]
    pub(crate) fn owned_entities(&self) -> Vec<Entity> {
        self.records.iter().map(RouteEntityRecord::entity).collect()
    }
}

impl QuarantineLease {
    pub(crate) fn owned_entities(&self) -> Vec<Entity> {
        self.records.iter().map(RouteEntityRecord::entity).collect()
    }
    pub(crate) fn foreign_descendants(&self) -> &[Entity] {
        &self.facts.foreign_descendants
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum QuarantineTransferError {
    EmptyOwnedSet,
    UnknownOwnedEntity,
    CapacityExhausted,
}

pub(crate) struct PreparedQuarantineTransfer {
    id: QuarantineLeaseId,
    source_claims: Vec<ExactRegistryClaim>,
    records: Vec<RouteEntityRecord>,
    facts: QuarantineFacts,
}

pub(crate) struct PreparedUnresolvedResidualTransfer {
    sequence: u64,
    source_claims: Vec<ExactRegistryClaim>,
    records: Vec<RouteEntityRecord>,
    actual_manifests: Vec<(Entity, ComponentManifest)>,
    facts: QuarantineFacts,
}

#[derive(Resource)]
pub(crate) struct QuarantineLeaseTable {
    domain: u64,
    next_sequence: u64,
    next_residual_sequence: u64,
    leases: Vec<QuarantineLease>,
    unresolved: Vec<UnresolvedResidual>,
}

impl QuarantineLeaseTable {
    pub(crate) fn new() -> Self {
        Self {
            domain: allocate_registry_domain(&NEXT_QUARANTINE_DOMAIN)
                .expect("quarantine lease domain exhausted"),
            next_sequence: 0,
            next_residual_sequence: 0,
            leases: Vec::new(),
            unresolved: Vec::new(),
        }
    }

    fn prepare_verified(
        &mut self,
        registry: &RouteEntityRegistry,
        claims: Vec<ExactRegistryClaim>,
        manifests: Vec<(Entity, ComponentManifest)>,
        facts: QuarantineFacts,
    ) -> Result<PreparedQuarantineTransfer, QuarantineTransferError> {
        if verified_preparation_fault_injected() {
            return Err(QuarantineTransferError::CapacityExhausted);
        }
        if claims.is_empty() || manifests.len() != claims.len() {
            return Err(QuarantineTransferError::EmptyOwnedSet);
        }
        let mut records = Vec::new();
        records
            .try_reserve(claims.len())
            .map_err(|_| QuarantineTransferError::CapacityExhausted)?;
        for claim in &claims {
            let Some(record) = registry
                .records
                .iter()
                .find(|record| record.claim() == *claim)
            else {
                return Err(QuarantineTransferError::UnknownOwnedEntity);
            };
            let mut matching_manifests = manifests
                .iter()
                .filter(|(entity, _)| *entity == claim.entity());
            let Some((_, manifest)) = matching_manifests.next() else {
                return Err(QuarantineTransferError::UnknownOwnedEntity);
            };
            if matching_manifests.next().is_some() {
                return Err(QuarantineTransferError::UnknownOwnedEntity);
            }
            let mut record = record.clone();
            record.lifecycle = EntityLifecycle::Quarantined;
            record.manifest = manifest.clone();
            records.push(record);
        }
        records.sort_by_key(|record| record.entity.to_bits());
        records.dedup_by_key(|record| record.entity);
        if records.len() != claims.len() {
            return Err(QuarantineTransferError::UnknownOwnedEntity);
        }
        let sequence = self
            .next_sequence
            .checked_add(1)
            .ok_or(QuarantineTransferError::CapacityExhausted)?;
        self.leases
            .try_reserve(1)
            .map_err(|_| QuarantineTransferError::CapacityExhausted)?;
        let id = QuarantineLeaseId {
            domain: self.domain,
            sequence,
        };
        Ok(PreparedQuarantineTransfer {
            id,
            source_claims: claims,
            records,
            facts,
        })
    }

    fn apply_prepared(
        &mut self,
        registry: &mut RouteEntityRegistry,
        prepared: PreparedQuarantineTransfer,
    ) -> QuarantineLeaseId {
        registry.records.retain(|record| {
            !prepared.source_claims.iter().any(|claim| {
                record.entity == claim.entity
                    && record.owner == claim.owner
                    && record.seal == claim.seal
                    && record.lifecycle == claim.lifecycle
            })
        });
        self.next_sequence = prepared.id.sequence;
        self.leases.push(QuarantineLease {
            id: prepared.id,
            generation: 1,
            operation_sequence: 0,
            records: prepared.records,
            facts: prepared.facts,
        });
        prepared.id
    }

    fn prepare_unresolved(
        &mut self,
        registry: &RouteEntityRegistry,
        claims: Vec<ExactRegistryClaim>,
        actual_manifests: Vec<(Entity, ComponentManifest)>,
        facts: QuarantineFacts,
    ) -> Result<PreparedUnresolvedResidualTransfer, QuarantineTransferError> {
        if unresolved_preparation_fault_injected() {
            return Err(QuarantineTransferError::CapacityExhausted);
        }
        if claims.is_empty() || actual_manifests.len() != claims.len() {
            return Err(QuarantineTransferError::EmptyOwnedSet);
        }
        let mut records = Vec::new();
        records
            .try_reserve(claims.len())
            .map_err(|_| QuarantineTransferError::CapacityExhausted)?;
        for claim in &claims {
            let Some(record) = registry
                .records
                .iter()
                .find(|record| record.claim() == *claim)
            else {
                return Err(QuarantineTransferError::UnknownOwnedEntity);
            };
            if actual_manifests
                .iter()
                .filter(|(entity, _)| *entity == claim.entity())
                .count()
                != 1
            {
                return Err(QuarantineTransferError::UnknownOwnedEntity);
            }
            records.push(record.clone());
        }
        records.sort_by_key(|record| record.entity.to_bits());
        records.dedup_by_key(|record| record.entity);
        if records.len() != claims.len() {
            return Err(QuarantineTransferError::UnknownOwnedEntity);
        }
        let sequence = self
            .next_residual_sequence
            .checked_add(1)
            .ok_or(QuarantineTransferError::CapacityExhausted)?;
        self.unresolved
            .try_reserve(1)
            .map_err(|_| QuarantineTransferError::CapacityExhausted)?;
        Ok(PreparedUnresolvedResidualTransfer {
            sequence,
            source_claims: claims,
            records,
            actual_manifests,
            facts,
        })
    }

    fn apply_unresolved(
        &mut self,
        registry: &mut RouteEntityRegistry,
        prepared: PreparedUnresolvedResidualTransfer,
    ) {
        registry.records.retain(|record| {
            !prepared.source_claims.iter().any(|claim| {
                record.entity == claim.entity
                    && record.owner == claim.owner
                    && record.seal == claim.seal
                    && record.lifecycle == claim.lifecycle
            })
        });
        self.next_residual_sequence = prepared.sequence;
        self.unresolved.push(UnresolvedResidual {
            sequence: prepared.sequence,
            records: prepared.records,
            actual_manifests: prepared.actual_manifests,
            facts: prepared.facts,
        });
    }

    pub(crate) fn lease(&self, id: QuarantineLeaseId) -> Option<&QuarantineLease> {
        self.leases.iter().find(|lease| lease.id == id)
    }
    pub(crate) fn leases(&self) -> &[QuarantineLease] {
        &self.leases
    }

    pub(crate) fn record(&self, entity: Entity) -> Option<&RouteEntityRecord> {
        self.leases
            .iter()
            .flat_map(|lease| lease.records.iter())
            .find(|record| record.entity == entity)
    }

    pub(crate) fn unresolved_record(&self, entity: Entity) -> Option<&RouteEntityRecord> {
        self.unresolved
            .iter()
            .flat_map(|residual| residual.records.iter())
            .find(|record| record.entity == entity)
    }

    pub(crate) fn inert_owned_entities(&self) -> impl Iterator<Item = Entity> + '_ {
        self.leases
            .iter()
            .flat_map(|lease| lease.records.iter())
            .chain(
                self.unresolved
                    .iter()
                    .flat_map(|residual| residual.records.iter()),
            )
            .map(RouteEntityRecord::entity)
    }

    #[cfg(test)]
    pub(crate) fn unresolved_residuals(&self) -> &[UnresolvedResidual] {
        &self.unresolved
    }
}

pub(crate) fn prepare_verified_quarantine_transfer(
    table: &mut QuarantineLeaseTable,
    registry: &RouteEntityRegistry,
    transfer: VerifiedQuarantineTransfer,
) -> Result<PreparedQuarantineTransfer, QuarantineTransferError> {
    let (claims, manifests, facts) = transfer.into_parts();
    table.prepare_verified(registry, claims, manifests, facts)
}

pub(crate) fn prepare_verified_frame_quarantine_transfer(
    table: &mut QuarantineLeaseTable,
    registry: &RouteEntityRegistry,
    transfer: VerifiedFrameQuarantineTransfer,
) -> Result<PreparedQuarantineTransfer, QuarantineTransferError> {
    let (claims, manifests, facts) = transfer.into_parts();
    table.prepare_verified(registry, claims, manifests, facts)
}

pub(crate) fn prepare_verified_cleanup_quarantine_transfer(
    table: &mut QuarantineLeaseTable,
    registry: &RouteEntityRegistry,
    transfer: VerifiedCleanupQuarantineTransfer,
) -> Result<PreparedQuarantineTransfer, QuarantineTransferError> {
    let (claims, manifests, facts) = transfer.into_parts();
    table.prepare_verified(registry, claims, manifests, facts)
}

pub(crate) fn apply_prepared_quarantine_transfer(
    table: &mut QuarantineLeaseTable,
    registry: &mut RouteEntityRegistry,
    prepared: PreparedQuarantineTransfer,
) -> QuarantineLeaseId {
    table.apply_prepared(registry, prepared)
}

pub(crate) fn prepare_unresolved_residual_transfer(
    table: &mut QuarantineLeaseTable,
    registry: &RouteEntityRegistry,
    claims: Vec<ExactRegistryClaim>,
    actual_manifests: Vec<(Entity, ComponentManifest)>,
    facts: QuarantineFacts,
) -> Result<PreparedUnresolvedResidualTransfer, QuarantineTransferError> {
    table.prepare_unresolved(registry, claims, actual_manifests, facts)
}

pub(crate) fn apply_prepared_unresolved_residual_transfer(
    table: &mut QuarantineLeaseTable,
    registry: &mut RouteEntityRegistry,
    prepared: PreparedUnresolvedResidualTransfer,
) {
    table.apply_unresolved(registry, prepared);
}

impl Default for QuarantineLeaseTable {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::HostCandidateRevision;
    use bevy_ui_router_core::{
        integration::{EngineAction, RouterEngine},
        prelude::{RouteLocation, RoutePattern, RouteTree, RouteViewKey},
    };

    fn frame_fixture() -> RouteFrameVersion {
        let tree = RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/").unwrap(),
                bevy_ui_router_core::prelude::RouteNodeOptions::new()
                    .with_view(RouteViewKey::new("root").unwrap()),
            )
            .unwrap()
            .build()
            .unwrap();
        let mut engine =
            RouterEngine::install(tree, RouteViewKey::new("fallback").unwrap()).unwrap();
        let token = engine.request_host_admission().unwrap();
        let grant = engine.grant_host_admission(&token).unwrap();
        engine.confirm_host(grant);
        engine.push(RouteLocation::new("/")).unwrap();
        engine.collect();
        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("frame task expected")
        };
        task.frame().clone()
    }

    fn child_owner_fixture() -> (RouteFrameVersion, RouteSlotId, RouteSlotId) {
        let mut engine = RouterEngine::install(
            bevy_ui_router_core::test_support::nested_items_view_route_tree(),
            RouteViewKey::new("fallback").unwrap(),
        )
        .unwrap();
        let token = engine.request_host_admission().unwrap();
        let grant = engine.grant_host_admission(&token).unwrap();
        engine.confirm_host(grant);
        engine.push(RouteLocation::new("/items/1")).unwrap();
        engine.collect();
        let EngineAction::FrameTask(parent) = engine.action().unwrap() else {
            panic!("parent frame task expected")
        };
        let parent_frame = parent.frame().clone();
        let top_slot = parent_frame.slot();
        engine
            .report_applied_with_outlets(parent, vec![RouteSlotKey::default()])
            .unwrap();
        let EngineAction::MountRequest(request) = engine.action().unwrap() else {
            panic!("child mount request expected")
        };
        let child_slot = engine.resolve_mount(request).unwrap().slot();
        (parent_frame, top_slot, child_slot)
    }

    #[test]
    fn exhausted_registry_domains_never_wrap_or_reuse_authority() {
        for counter in [AtomicU64::new(u64::MAX - 1), AtomicU64::new(u64::MAX - 1)] {
            assert_eq!(allocate_registry_domain(&counter), Some(u64::MAX - 1));
            assert_eq!(counter.load(Ordering::Relaxed), u64::MAX);
            assert_eq!(allocate_registry_domain(&counter), None);
            assert_eq!(allocate_registry_domain(&counter), None);
            assert_eq!(counter.load(Ordering::Relaxed), u64::MAX);
        }
    }
    use bevy_ecs::{component::Component, prelude::World};

    #[derive(Component)]
    struct A;
    #[derive(Component)]
    struct B;

    #[test]
    fn manifest_is_sorted_deduplicated_and_matches_exact_archetype() {
        let mut world = World::new();
        let entity = world.spawn((A, B)).id();
        let a = world.components().component_id::<A>().unwrap();
        let b = world.components().component_id::<B>().unwrap();
        let manifest = ComponentManifest::new([b, a, b]);
        assert_eq!(manifest.component_ids(), &[a, b]);
        assert!(manifest.matches(&world, entity));
        world.entity_mut(entity).remove::<B>();
        assert!(!manifest.matches(&world, entity));
    }

    #[test]
    fn registry_rejects_duplicate_entity_owner_and_seal_without_partial_write() {
        let mut registry = RouteEntityRegistry::new();
        let mut world = World::new();
        let first = world.spawn_empty().id();
        let second = world.spawn_empty().id();
        let owner = ProvisionalTopSlotOwner::new(HostCandidateRevision::fixture(1));
        let other_owner = ProvisionalTopSlotOwner::new(HostCandidateRevision::fixture(2));
        let first_seal = registry.issue_seal();
        let second_seal = registry.issue_seal();
        let manifest = ComponentManifest::new([]);

        registry
            .register(
                first,
                RegistryOwner::Provisional(ProvisionalOwner::HostTopSlot(owner.clone())),
                first_seal,
                manifest.clone(),
                EntityLifecycle::PreparedInert,
            )
            .unwrap();
        let baseline = registry.records().to_vec();
        assert_eq!(
            registry.register(
                first,
                RegistryOwner::Provisional(ProvisionalOwner::HostTopSlot(other_owner.clone())),
                second_seal,
                manifest.clone(),
                EntityLifecycle::PreparedInert
            ),
            Err(RegistryWriteError::DuplicateEntity)
        );
        assert_eq!(
            registry.register(
                second,
                RegistryOwner::Provisional(ProvisionalOwner::HostTopSlot(owner)),
                second_seal,
                manifest.clone(),
                EntityLifecycle::PreparedInert
            ),
            Err(RegistryWriteError::DuplicateOwner)
        );
        assert_eq!(
            registry.register(
                second,
                RegistryOwner::Provisional(ProvisionalOwner::HostTopSlot(other_owner)),
                first_seal,
                manifest,
                EntityLifecycle::PreparedInert
            ),
            Err(RegistryWriteError::DuplicateSeal)
        );
        assert_eq!(registry.records(), baseline.as_slice());
    }

    #[test]
    fn provisional_child_owner_preserves_unique_owner_seal_and_exact_cas() {
        let mut registry = RouteEntityRegistry::new();
        let mut world = World::new();
        let first = world.spawn_empty().id();
        let duplicate = world.spawn_empty().id();
        let parent = frame_fixture();
        let slot = parent.slot();
        let outlet = RouteSlotKey::default();
        let owner = ProvisionalOwner::ChildSlot {
            parent: parent.clone(),
            outlet: outlet.clone(),
            slot,
        };
        let first_seal = registry.issue_seal();
        let duplicate_seal = registry.issue_seal();

        registry
            .register(
                first,
                RegistryOwner::Provisional(owner.clone()),
                first_seal,
                ComponentManifest::new([]),
                EntityLifecycle::PreparedInert,
            )
            .unwrap();
        let provisional = registry.record(first).unwrap().claim();
        let baseline = registry.records().to_vec();
        assert_eq!(
            registry.register(
                duplicate,
                RegistryOwner::Provisional(owner),
                duplicate_seal,
                ComponentManifest::new([]),
                EntityLifecycle::PreparedInert,
            ),
            Err(RegistryWriteError::DuplicateOwner)
        );
        assert_eq!(registry.records(), baseline);

        let committed_owner = RegistryOwner::Committed(EntityOwner::ChildSlot {
            parent,
            outlet,
            slot,
        });
        let committed = registry
            .compare_exchange_owner(&provisional, committed_owner.clone())
            .expect("exact provisional child claim should commit once");
        assert!(
            registry
                .compare_exchange_owner(&provisional, committed_owner)
                .is_none()
        );
        assert_eq!(registry.record(first).unwrap().claim(), committed);
    }

    #[test]
    fn child_owner_uniqueness_crosses_provisional_committed_location_and_router_slot() {
        let (parent, other_slot, child_slot) = child_owner_fixture();
        assert_ne!(other_slot, child_slot);
        let outlet = RouteSlotKey::default();
        let other_outlet = RouteSlotKey::named("other").unwrap();

        let mut registry = RouteEntityRegistry::new();
        let mut world = World::new();
        let first = world.spawn_empty().id();
        let same_location = world.spawn_empty().id();
        let same_slot = world.spawn_empty().id();
        let first_seal = registry.issue_seal();
        let location_seal = registry.issue_seal();
        let slot_seal = registry.issue_seal();
        registry
            .register(
                first,
                RegistryOwner::Provisional(ProvisionalOwner::ChildSlot {
                    parent: parent.clone(),
                    outlet: outlet.clone(),
                    slot: child_slot,
                }),
                first_seal,
                ComponentManifest::new([]),
                EntityLifecycle::PreparedInert,
            )
            .unwrap();
        let baseline = registry.records().to_vec();
        assert_eq!(
            registry.register(
                same_location,
                RegistryOwner::Committed(EntityOwner::ChildSlot {
                    parent: parent.clone(),
                    outlet: outlet.clone(),
                    slot: other_slot,
                }),
                location_seal,
                ComponentManifest::new([]),
                EntityLifecycle::PreparedInert,
            ),
            Err(RegistryWriteError::DuplicateOwner)
        );
        assert_eq!(registry.records(), baseline);
        assert_eq!(
            registry.register(
                same_slot,
                RegistryOwner::Committed(EntityOwner::ChildSlot {
                    parent,
                    outlet: other_outlet,
                    slot: child_slot,
                }),
                slot_seal,
                ComponentManifest::new([]),
                EntityLifecycle::PreparedInert,
            ),
            Err(RegistryWriteError::DuplicateOwner)
        );
        assert_eq!(registry.records(), baseline);
    }

    #[test]
    fn child_owner_uniqueness_keeps_opaque_router_instances_independent() {
        let (first_parent, _, first_child_slot) = child_owner_fixture();
        let (second_parent, _, second_child_slot) = child_owner_fixture();
        assert_eq!(first_parent.router(), second_parent.router());
        assert_eq!(first_child_slot, second_child_slot);
        assert!(!frame_versions_share_router(&first_parent, &second_parent));

        let mut registry = RouteEntityRegistry::new();
        let mut world = World::new();
        let first = world.spawn_empty().id();
        let second = world.spawn_empty().id();
        let first_seal = registry.issue_seal();
        let second_seal = registry.issue_seal();
        let outlet = RouteSlotKey::default();
        registry
            .register(
                first,
                RegistryOwner::Provisional(ProvisionalOwner::ChildSlot {
                    parent: first_parent,
                    outlet: outlet.clone(),
                    slot: first_child_slot,
                }),
                first_seal,
                ComponentManifest::new([]),
                EntityLifecycle::PreparedInert,
            )
            .unwrap();
        registry
            .register(
                second,
                RegistryOwner::Provisional(ProvisionalOwner::ChildSlot {
                    parent: second_parent,
                    outlet,
                    slot: second_child_slot,
                }),
                second_seal,
                ComponentManifest::new([]),
                EntityLifecycle::PreparedInert,
            )
            .expect("separate opaque router instances may reuse raw slot projections");
    }

    #[test]
    fn exact_manifest_cas_rejects_stale_old_manifest_without_write() {
        let mut registry = RouteEntityRegistry::new();
        let mut world = World::new();
        let entity = world.spawn_empty().id();
        let seal = registry.issue_seal();
        registry
            .register(
                entity,
                RegistryOwner::Provisional(ProvisionalOwner::HostTopSlot(
                    ProvisionalTopSlotOwner::new(HostCandidateRevision::fixture(31)),
                )),
                seal,
                ComponentManifest::new([]),
                EntityLifecycle::PreparedInert,
            )
            .unwrap();
        let claim = registry.record(entity).unwrap().claim();
        let old = registry.record(entity).unwrap().manifest().clone();
        let component = world.register_component::<A>();
        let next = ComponentManifest::new([component]);

        assert!(!registry.compare_exchange_manifest_exact(&claim, &next, next.clone()));
        assert_eq!(registry.record(entity).unwrap().manifest(), &old);
        assert!(registry.compare_exchange_manifest_exact(&claim, &old, next.clone()));
        assert_eq!(registry.record(entity).unwrap().manifest(), &next);
        assert!(!registry.compare_exchange_manifest_exact(&claim, &old, old.clone()));
        assert_eq!(registry.record(entity).unwrap().manifest(), &next);
    }

    #[test]
    fn exact_manifest_batch_is_atomic_and_rejects_duplicate_claims() {
        let mut registry = RouteEntityRegistry::new();
        let mut world = World::new();
        let first = world.spawn_empty().id();
        let second = world.spawn_empty().id();
        for (entity, revision) in [(first, 41), (second, 43)] {
            let seal = registry.issue_seal();
            registry
                .register(
                    entity,
                    RegistryOwner::Provisional(ProvisionalOwner::HostTopSlot(
                        ProvisionalTopSlotOwner::new(HostCandidateRevision::fixture(revision)),
                    )),
                    seal,
                    ComponentManifest::new([]),
                    EntityLifecycle::PreparedInert,
                )
                .unwrap();
        }
        let first_claim = registry.record(first).unwrap().claim();
        let second_claim = registry.record(second).unwrap().claim();
        let empty = ComponentManifest::new([]);
        let a = ComponentManifest::new([world.register_component::<A>()]);
        let b = ComponentManifest::new([world.register_component::<B>()]);
        let stale = ComponentManifest::new([
            world.register_component::<A>(),
            world.register_component::<B>(),
        ]);
        let baseline = registry.records().to_vec();

        assert!(!registry.compare_exchange_manifests_exact(&[
            (first_claim.clone(), empty.clone(), a.clone()),
            (second_claim.clone(), stale, b.clone()),
        ]));
        assert_eq!(registry.records(), baseline);
        assert!(!registry.compare_exchange_manifests_exact(&[
            (first_claim.clone(), empty.clone(), a.clone()),
            (first_claim, empty.clone(), b),
        ]));
        assert_eq!(registry.records(), baseline);
        assert!(registry.compare_exchange_manifests_exact(&[
            (registry.record(first).unwrap().claim(), empty.clone(), a),
            (second_claim, empty, ComponentManifest::new([])),
        ]));
    }

    #[test]
    fn registry_rejects_foreign_zero_and_unissued_seals_without_partial_write() {
        let mut registry = RouteEntityRegistry::new();
        let mut foreign = RouteEntityRegistry::new();
        let mut world = World::new();
        let entity = world.spawn_empty().id();
        let owner = RegistryOwner::Provisional(ProvisionalOwner::HostTopSlot(
            ProvisionalTopSlotOwner::new(HostCandidateRevision::fixture(8)),
        ));
        let foreign_seal = foreign.issue_seal();
        for seal in [
            foreign_seal,
            OwnershipSeal::fixture(registry.seal_domain, 0),
            OwnershipSeal::fixture(registry.seal_domain, 1),
        ] {
            let baseline = registry.records().to_vec();
            assert_eq!(
                registry.register(
                    entity,
                    owner.clone(),
                    seal,
                    ComponentManifest::new([]),
                    EntityLifecycle::PreparedInert,
                ),
                Err(RegistryWriteError::ForeignOrUnissuedSeal)
            );
            assert_eq!(registry.records(), baseline);
        }
    }

    #[test]
    fn registry_lifecycle_and_removal_are_exact_claim_compare_and_swap() {
        let mut registry = RouteEntityRegistry::new();
        let mut world = World::new();
        let entity = world.spawn_empty().id();
        let seal = registry.issue_seal();
        let owner = RegistryOwner::Provisional(ProvisionalOwner::HostTopSlot(
            ProvisionalTopSlotOwner::new(HostCandidateRevision::fixture(9)),
        ));
        registry
            .register(
                entity,
                owner,
                seal,
                ComponentManifest::new([]),
                EntityLifecycle::PreparedInert,
            )
            .unwrap();
        let prepared = registry.record(entity).unwrap().claim();
        let quarantined = registry
            .compare_exchange_lifecycle(&prepared, EntityLifecycle::Quarantined)
            .expect("exact claim should advance");
        let baseline = registry.records().to_vec();
        assert!(
            registry
                .compare_exchange_lifecycle(&prepared, EntityLifecycle::Active)
                .is_none()
        );
        assert!(!registry.remove_exact(&prepared));
        assert_eq!(registry.records(), baseline);
        assert!(registry.remove_exact(&quarantined));
        assert!(registry.records().is_empty());
    }

    #[test]
    fn quarantine_transfer_freezes_an_immutable_exact_owned_set() {
        let mut world = World::new();
        let first = world.spawn_empty().id();
        let second = world.spawn_empty().id();
        let foreign = world.spawn_empty().id();
        let mut registry = RouteEntityRegistry::new();
        let first_owner = RegistryOwner::Provisional(ProvisionalOwner::HostTopSlot(
            ProvisionalTopSlotOwner::new(HostCandidateRevision::fixture(1)),
        ));
        let second_owner = RegistryOwner::Provisional(ProvisionalOwner::HostTopSlot(
            ProvisionalTopSlotOwner::new(HostCandidateRevision::fixture(2)),
        ));
        let first_seal = registry.issue_seal();
        let second_seal = registry.issue_seal();
        registry
            .register(
                first,
                first_owner,
                first_seal,
                ComponentManifest::new([]),
                EntityLifecycle::PreparedInert,
            )
            .unwrap();
        registry
            .register(
                second,
                second_owner,
                second_seal,
                ComponentManifest::new([]),
                EntityLifecycle::PreparedInert,
            )
            .unwrap();
        let facts = QuarantineFacts::new(None, None, vec![foreign], vec![]);
        let mut table = QuarantineLeaseTable::new();
        let transfer = VerifiedQuarantineTransfer::fixture(
            vec![
                registry.record(second).unwrap().claim(),
                registry.record(first).unwrap().claim(),
            ],
            vec![
                (second, ComponentManifest::new([])),
                (first, ComponentManifest::new([])),
            ],
            facts,
        );
        let transfer =
            prepare_verified_quarantine_transfer(&mut table, &registry, transfer).unwrap();
        let lease = apply_prepared_quarantine_transfer(&mut table, &mut registry, transfer);
        let mut expected = vec![first, second];
        expected.sort_by_key(|entity| entity.to_bits());
        assert_eq!(table.lease(lease).unwrap().owned_entities(), expected);
        assert_eq!(
            table.lease(lease).unwrap().foreign_descendants(),
            &[foreign]
        );
        assert!(registry.records().is_empty());

        let replacement = world.spawn_empty().id();
        let baseline = table.leases().to_vec();
        assert_eq!(
            registry.register(
                replacement,
                RegistryOwner::Provisional(ProvisionalOwner::HostTopSlot(
                    ProvisionalTopSlotOwner::new(HostCandidateRevision::fixture(3)),
                )),
                first_seal,
                ComponentManifest::new([]),
                EntityLifecycle::PreparedInert,
            ),
            Err(RegistryWriteError::DuplicateSeal)
        );
        assert!(registry.records().is_empty());
        assert_eq!(table.leases(), baseline);
    }

    #[test]
    fn quarantine_transfer_rejects_stale_exact_claim_without_writes() {
        let mut world = World::new();
        let entity = world.spawn_empty().id();
        let mut registry = RouteEntityRegistry::new();
        let seal = registry.issue_seal();
        registry
            .register(
                entity,
                RegistryOwner::Provisional(ProvisionalOwner::HostTopSlot(
                    ProvisionalTopSlotOwner::new(HostCandidateRevision::fixture(13)),
                )),
                seal,
                ComponentManifest::new([]),
                EntityLifecycle::PreparedInert,
            )
            .unwrap();
        let stale = registry.record(entity).unwrap().claim();
        registry
            .compare_exchange_lifecycle(&stale, EntityLifecycle::Quarantined)
            .unwrap();
        let baseline = registry.records().to_vec();
        let mut table = QuarantineLeaseTable::new();
        let transfer = VerifiedQuarantineTransfer::fixture(
            vec![stale],
            vec![(entity, ComponentManifest::new([]))],
            QuarantineFacts::new(None, None, vec![], vec![]),
        );

        assert_eq!(
            prepare_verified_quarantine_transfer(&mut table, &registry, transfer).map(|_| ()),
            Err(QuarantineTransferError::UnknownOwnedEntity)
        );
        assert_eq!(registry.records(), baseline);
        assert!(table.leases().is_empty());
    }

    #[test]
    fn quarantine_transfer_requires_owner_specific_sealed_tokens() {
        let registry_source = include_str!("registry.rs");
        let host_source = include_str!("host.rs");
        let presentation_source = include_str!("presentation.rs");
        assert!(registry_source.contains("fn prepare_verified("));
        assert!(registry_source.contains("fn apply_prepared("));
        let raw_public_transfer = ["pub(crate) fn ", "transfer("].concat();
        assert!(!registry_source.contains(&raw_public_transfer));
        assert!(registry_source.contains("transfer: VerifiedQuarantineTransfer"));
        assert!(registry_source.contains("transfer: VerifiedFrameQuarantineTransfer"));
        assert!(host_source.contains("fn after_world_validation("));
        assert!(!host_source.contains("pub(crate) fn after_world_validation("));
        assert!(presentation_source.contains("fn after_world_validation("));
        assert!(!presentation_source.contains("pub(crate) fn after_world_validation("));
    }
}
