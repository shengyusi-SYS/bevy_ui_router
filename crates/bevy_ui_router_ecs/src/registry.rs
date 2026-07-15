#![allow(dead_code)]

use std::sync::atomic::{AtomicU64, Ordering};

use bevy_ecs::{
    component::ComponentId,
    prelude::{Entity, Resource, World},
};
use bevy_ui_router_core::{
    HostVersion, frame::RouteFrameVersion, ids::RouteSlotId, slot::RouteSlotKey,
};

use crate::host::HostCandidateRevision;
use crate::host::VerifiedQuarantineTransfer;

static NEXT_SEAL_DOMAIN: AtomicU64 = AtomicU64::new(1);
static NEXT_QUARANTINE_DOMAIN: AtomicU64 = AtomicU64::new(1);

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
pub(crate) enum RegistryOwner {
    Committed(EntityOwner),
    Provisional(ProvisionalTopSlotOwner),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct OwnershipSeal {
    domain: u64,
    sequence: u64,
}

impl OwnershipSeal {
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

impl ExactRegistryClaim {
    pub(crate) const fn entity(&self) -> Entity {
        self.entity
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
            seal_domain: NEXT_SEAL_DOMAIN.fetch_add(1, Ordering::Relaxed),
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
        if self.records.iter().any(|record| record.owner == owner) {
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
}

#[derive(Resource)]
pub(crate) struct QuarantineLeaseTable {
    domain: u64,
    next_sequence: u64,
    leases: Vec<QuarantineLease>,
}

impl QuarantineLeaseTable {
    pub(crate) fn new() -> Self {
        Self {
            domain: NEXT_QUARANTINE_DOMAIN.fetch_add(1, Ordering::Relaxed),
            next_sequence: 0,
            leases: Vec::new(),
        }
    }

    fn transfer_verified(
        &mut self,
        registry: &mut RouteEntityRegistry,
        claims: &[ExactRegistryClaim],
        facts: QuarantineFacts,
    ) -> Result<QuarantineLeaseId, QuarantineTransferError> {
        if claims.is_empty() {
            return Err(QuarantineTransferError::EmptyOwnedSet);
        }
        if claims
            .iter()
            .any(|claim| registry.record(claim.entity()).is_none())
        {
            return Err(QuarantineTransferError::UnknownOwnedEntity);
        }
        let records = registry
            .take_exact_set(claims)
            .ok_or(QuarantineTransferError::UnknownOwnedEntity)?;
        self.next_sequence = self
            .next_sequence
            .checked_add(1)
            .expect("quarantine lease sequence exhausted");
        let id = QuarantineLeaseId {
            domain: self.domain,
            sequence: self.next_sequence,
        };
        self.leases.push(QuarantineLease {
            id,
            generation: 1,
            operation_sequence: 0,
            records,
            facts,
        });
        Ok(id)
    }

    pub(crate) fn lease(&self, id: QuarantineLeaseId) -> Option<&QuarantineLease> {
        self.leases.iter().find(|lease| lease.id == id)
    }
    pub(crate) fn leases(&self) -> &[QuarantineLease] {
        &self.leases
    }
}

pub(crate) fn apply_verified_quarantine_transfer(
    table: &mut QuarantineLeaseTable,
    registry: &mut RouteEntityRegistry,
    transfer: VerifiedQuarantineTransfer,
) -> Result<QuarantineLeaseId, QuarantineTransferError> {
    let (claims, facts) = transfer.into_parts();
    table.transfer_verified(registry, &claims, facts)
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
                RegistryOwner::Provisional(owner.clone()),
                first_seal,
                manifest.clone(),
                EntityLifecycle::PreparedInert,
            )
            .unwrap();
        let baseline = registry.records().to_vec();
        assert_eq!(
            registry.register(
                first,
                RegistryOwner::Provisional(other_owner.clone()),
                second_seal,
                manifest.clone(),
                EntityLifecycle::PreparedInert
            ),
            Err(RegistryWriteError::DuplicateEntity)
        );
        assert_eq!(
            registry.register(
                second,
                RegistryOwner::Provisional(owner),
                second_seal,
                manifest.clone(),
                EntityLifecycle::PreparedInert
            ),
            Err(RegistryWriteError::DuplicateOwner)
        );
        assert_eq!(
            registry.register(
                second,
                RegistryOwner::Provisional(other_owner),
                first_seal,
                manifest,
                EntityLifecycle::PreparedInert
            ),
            Err(RegistryWriteError::DuplicateSeal)
        );
        assert_eq!(registry.records(), baseline.as_slice());
    }

    #[test]
    fn registry_rejects_foreign_zero_and_unissued_seals_without_partial_write() {
        let mut registry = RouteEntityRegistry::new();
        let mut foreign = RouteEntityRegistry::new();
        let mut world = World::new();
        let entity = world.spawn_empty().id();
        let owner = RegistryOwner::Provisional(ProvisionalTopSlotOwner::new(
            HostCandidateRevision::fixture(8),
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
        let owner = RegistryOwner::Provisional(ProvisionalTopSlotOwner::new(
            HostCandidateRevision::fixture(9),
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
        let first_owner = RegistryOwner::Provisional(ProvisionalTopSlotOwner::new(
            HostCandidateRevision::fixture(1),
        ));
        let second_owner = RegistryOwner::Provisional(ProvisionalTopSlotOwner::new(
            HostCandidateRevision::fixture(2),
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
            facts,
        );
        let lease =
            apply_verified_quarantine_transfer(&mut table, &mut registry, transfer).unwrap();
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
                RegistryOwner::Provisional(ProvisionalTopSlotOwner::new(
                    HostCandidateRevision::fixture(3),
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
                RegistryOwner::Provisional(ProvisionalTopSlotOwner::new(
                    HostCandidateRevision::fixture(13),
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
            QuarantineFacts::new(None, None, vec![], vec![]),
        );

        assert_eq!(
            apply_verified_quarantine_transfer(&mut table, &mut registry, transfer),
            Err(QuarantineTransferError::UnknownOwnedEntity)
        );
        assert_eq!(registry.records(), baseline);
        assert!(table.leases().is_empty());
    }

    #[test]
    fn quarantine_transfer_requires_the_host_gate_sealed_token() {
        let registry_source = include_str!("registry.rs");
        let host_source = include_str!("host.rs");
        assert!(registry_source.contains("fn transfer_verified("));
        let raw_public_transfer = ["pub(crate) fn ", "transfer("].concat();
        assert!(!registry_source.contains(&raw_public_transfer));
        assert!(registry_source.contains("transfer: VerifiedQuarantineTransfer"));
        assert!(host_source.contains("fn after_world_validation("));
        assert!(!host_source.contains("pub(crate) fn after_world_validation("));
    }
}
