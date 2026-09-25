#![allow(dead_code)]

use std::{
    collections::HashSet,
    sync::atomic::{AtomicU64, Ordering},
};

use bevy_ecs::{
    component::ComponentId,
    hierarchy::ChildOf,
    prelude::{Entity, Resource, World},
};
use bevy_ui_router_core::{
    HostVersion,
    frame::RouteFrameVersion,
    ids::RouteSlotId,
    integration::{frame_versions_share_router, host_version_matches_frame},
    slot::RouteSlotKey,
};

use crate::app_ext::RouterHandle;
use crate::cleanup::{
    PreparedAuthorityLostResidualProjection, VerifiedCleanupQuarantineTransfer,
    cleanup_claim_has_exact_authority,
};
use crate::host::HostCandidateRevision;
use crate::host::VerifiedQuarantineTransfer;
use crate::presentation::VerifiedFrameQuarantineTransfer;

static NEXT_SEAL_DOMAIN: AtomicU64 = AtomicU64::new(1);
static NEXT_QUARANTINE_DOMAIN: AtomicU64 = AtomicU64::new(1);

#[cfg(test)]
thread_local! {
    static FAIL_NEXT_UNRESOLVED_PREPARATION: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FAIL_NEXT_VERIFIED_PREPARATION: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FAIL_NEXT_AUTHORITY_PUBLICATION_CAPACITY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static AUTHORITY_PUBLICATION_MUTATION_STARTED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static AUTHORITY_POSTMUTATION_RESERVATION_ATTEMPTS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static AUTHORITY_POSTMUTATION_MANIFEST_MATERIALIZATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
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
pub(crate) fn fail_next_authority_publication_capacity_reservation_for_test() {
    FAIL_NEXT_AUTHORITY_PUBLICATION_CAPACITY.set(true);
}

#[cfg(test)]
fn authority_publication_capacity_fault_injected() -> bool {
    FAIL_NEXT_AUTHORITY_PUBLICATION_CAPACITY.replace(false)
}

#[cfg(not(test))]
const fn authority_publication_capacity_fault_injected() -> bool {
    false
}

#[cfg(test)]
pub(crate) struct AuthorityPublicationMutationMarker;

#[cfg(test)]
impl Drop for AuthorityPublicationMutationMarker {
    fn drop(&mut self) {
        AUTHORITY_PUBLICATION_MUTATION_STARTED.set(false);
    }
}

#[cfg(test)]
pub(crate) fn begin_authority_publication_mutation_for_test() -> AuthorityPublicationMutationMarker
{
    AUTHORITY_PUBLICATION_MUTATION_STARTED.set(true);
    AuthorityPublicationMutationMarker
}

#[cfg(test)]
pub(crate) fn take_authority_postmutation_reservation_attempts_for_test() -> usize {
    AUTHORITY_POSTMUTATION_RESERVATION_ATTEMPTS.replace(0)
}

#[cfg(test)]
pub(crate) fn take_authority_postmutation_manifest_materializations_for_test() -> usize {
    AUTHORITY_POSTMUTATION_MANIFEST_MATERIALIZATIONS.replace(0)
}

#[cfg(test)]
fn note_authority_reservation_attempt() {
    if AUTHORITY_PUBLICATION_MUTATION_STARTED.get() {
        AUTHORITY_POSTMUTATION_RESERVATION_ATTEMPTS
            .set(AUTHORITY_POSTMUTATION_RESERVATION_ATTEMPTS.get() + 1);
    }
}

#[cfg(not(test))]
const fn note_authority_reservation_attempt() {}

#[cfg(test)]
fn note_component_manifest_materialization() {
    if AUTHORITY_PUBLICATION_MUTATION_STARTED.get() {
        AUTHORITY_POSTMUTATION_MANIFEST_MATERIALIZATIONS
            .set(AUTHORITY_POSTMUTATION_MANIFEST_MATERIALIZATIONS.get() + 1);
    }
}

#[cfg(not(test))]
const fn note_component_manifest_materialization() {}

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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ComponentManifest {
    component_ids: Vec<ComponentId>,
}

impl ComponentManifest {
    /// Structural component presence excludes only Bevy 0.19.1's transient
    /// clipping result. The official UI system adds/removes CalculatedClip as
    /// authored display/overflow changes; it is not ownership or role evidence.
    /// Keep this iterator allocation-free for resident publication consumers.
    pub(crate) fn observed_component_ids<'a>(
        world: &World,
        components: &'a [ComponentId],
    ) -> impl Iterator<Item = ComponentId> + Clone + 'a {
        let clip = world.components().component_id::<bevy_ui::CalculatedClip>();
        components
            .iter()
            .copied()
            .filter(move |id| Some(*id) != clip)
    }

    pub(crate) fn new(ids: impl IntoIterator<Item = ComponentId>) -> Self {
        note_component_manifest_materialization();
        let mut component_ids = ids.into_iter().collect::<Vec<_>>();
        component_ids.sort_unstable();
        component_ids.dedup();
        Self { component_ids }
    }

    pub(crate) fn try_new(ids: impl IntoIterator<Item = ComponentId>) -> Result<Self, ()> {
        note_component_manifest_materialization();
        let iterator = ids.into_iter();
        let (minimum, maximum) = iterator.size_hint();
        let mut component_ids = Vec::new();
        component_ids
            .try_reserve_exact(maximum.unwrap_or(minimum))
            .map_err(|_| ())?;
        for component in iterator {
            if component_ids.len() == component_ids.capacity() {
                component_ids.try_reserve(1).map_err(|_| ())?;
            }
            component_ids.push(component);
        }
        component_ids.sort_unstable();
        component_ids.dedup();
        Ok(Self { component_ids })
    }

    pub(crate) fn from_world(world: &World, entity: Entity) -> Option<Self> {
        world.get_entity(entity).ok().map(|entity| {
            Self::new(Self::observed_component_ids(
                world,
                entity.archetype().components(),
            ))
        })
    }

    pub(crate) fn try_from_world(world: &World, entity: Entity) -> Result<Option<Self>, ()> {
        let Ok(entity) = world.get_entity(entity) else {
            return Ok(None);
        };
        let components = entity.archetype().components();
        let mut component_ids = Vec::new();
        component_ids
            .try_reserve_exact(components.len())
            .map_err(|_| ())?;
        component_ids.extend(Self::observed_component_ids(world, components));
        component_ids.sort_unstable();
        component_ids.dedup();
        Ok(Some(Self { component_ids }))
    }

    pub(crate) fn try_clone(&self) -> Result<Self, ()> {
        let mut component_ids = Vec::new();
        component_ids
            .try_reserve_exact(self.component_ids.len())
            .map_err(|_| ())?;
        component_ids.extend_from_slice(&self.component_ids);
        Ok(Self { component_ids })
    }

    pub(crate) fn component_ids(&self) -> &[ComponentId] {
        &self.component_ids
    }

    pub(crate) fn matches(&self, world: &World, entity: Entity) -> bool {
        Self::from_world(world, entity).is_some_and(|actual| actual == *self)
    }

    pub(crate) fn matches_without_materialization(&self, world: &World, entity: Entity) -> bool {
        let Ok(entity) = world.get_entity(entity) else {
            return false;
        };
        let mut actual = Self::observed_component_ids(world, entity.archetype().components());
        actual.clone().count() == self.component_ids.len()
            && actual.all(|component| self.component_ids.contains(&component))
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

    pub(crate) fn matches_claim(&self, claim: &ExactRegistryClaim) -> bool {
        self.entity == claim.entity
            && self.owner == claim.owner
            && self.seal == claim.seal
            && self.lifecycle == claim.lifecycle
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
    pub(crate) fn try_clone(&self) -> Result<Self, ()> {
        try_clone_exact_registry_claim(self)
    }

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
    CapacityExhausted,
    SealExhausted,
    DuplicateEntity,
    DuplicateOwner,
}

#[derive(Clone, Debug, Eq, PartialEq, Resource)]
pub(crate) struct RouteEntityRegistry {
    seal_domain: u64,
    next_seal_sequence: u64,
    records: Vec<RouteEntityRecord>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProtectedRegistryProjection {
    seal_domain: u64,
    next_seal_sequence: u64,
    records: Vec<RouteEntityRecord>,
}

impl ProtectedRegistryProjection {
    pub(crate) fn bind_single_registration(&mut self, seal: OwnershipSeal) -> bool {
        if seal.domain != self.seal_domain
            || Some(seal.sequence) != self.next_seal_sequence.checked_add(1)
        {
            return false;
        }
        self.next_seal_sequence = seal.sequence;
        true
    }
}

fn try_clone_route_slot_key(source: &RouteSlotKey) -> Result<RouteSlotKey, ()> {
    if source.is_default() {
        return Ok(RouteSlotKey::default());
    }
    let mut value = String::new();
    value
        .try_reserve_exact(source.as_str().len())
        .map_err(|_| ())?;
    value.push_str(source.as_str());
    RouteSlotKey::named(value).map_err(|_| ())
}

fn try_clone_registry_owner(source: &RegistryOwner) -> Result<RegistryOwner, ()> {
    Ok(match source {
        RegistryOwner::Committed(EntityOwner::HostTopSlot { host, slot }) => {
            RegistryOwner::Committed(EntityOwner::HostTopSlot {
                host: host.clone(),
                slot: *slot,
            })
        }
        RegistryOwner::Committed(EntityOwner::ChildSlot {
            parent,
            outlet,
            slot,
        }) => RegistryOwner::Committed(EntityOwner::ChildSlot {
            parent: parent.clone(),
            outlet: try_clone_route_slot_key(outlet)?,
            slot: *slot,
        }),
        RegistryOwner::Committed(EntityOwner::FrameShell(frame)) => {
            RegistryOwner::Committed(EntityOwner::FrameShell(frame.clone()))
        }
        RegistryOwner::Committed(EntityOwner::FrameContentTarget(frame)) => {
            RegistryOwner::Committed(EntityOwner::FrameContentTarget(frame.clone()))
        }
        RegistryOwner::Committed(EntityOwner::FrameOutput {
            frame,
            output,
            local_index,
        }) => RegistryOwner::Committed(EntityOwner::FrameOutput {
            frame: frame.clone(),
            output: *output,
            local_index: *local_index,
        }),
        RegistryOwner::Provisional(ProvisionalOwner::HostTopSlot(owner)) => {
            RegistryOwner::Provisional(ProvisionalOwner::HostTopSlot(owner.clone()))
        }
        RegistryOwner::Provisional(ProvisionalOwner::ChildSlot {
            parent,
            outlet,
            slot,
        }) => RegistryOwner::Provisional(ProvisionalOwner::ChildSlot {
            parent: parent.clone(),
            outlet: try_clone_route_slot_key(outlet)?,
            slot: *slot,
        }),
    })
}

fn try_clone_route_entity_record(source: &RouteEntityRecord) -> Result<RouteEntityRecord, ()> {
    Ok(RouteEntityRecord {
        entity: source.entity,
        owner: try_clone_registry_owner(&source.owner)?,
        seal: source.seal,
        manifest: source.manifest.try_clone()?,
        lifecycle: source.lifecycle,
    })
}

fn try_clone_exact_registry_claim(source: &ExactRegistryClaim) -> Result<ExactRegistryClaim, ()> {
    Ok(ExactRegistryClaim {
        entity: source.entity,
        owner: try_clone_registry_owner(&source.owner)?,
        seal: source.seal,
        lifecycle: source.lifecycle,
    })
}

impl RouteEntityRegistry {
    pub(crate) fn new() -> Self {
        Self {
            seal_domain: allocate_registry_domain(&NEXT_SEAL_DOMAIN)
                .expect("ownership seal domain exhausted"),
            next_seal_sequence: 0,
            records: Vec::new(),
        }
    }

    pub(crate) fn protected_projection(
        &self,
        excluded_target: Entity,
    ) -> ProtectedRegistryProjection {
        self.protected_projection_excluding(&[excluded_target])
    }

    pub(crate) fn protected_projection_excluding(
        &self,
        excluded_targets: &[Entity],
    ) -> ProtectedRegistryProjection {
        ProtectedRegistryProjection {
            seal_domain: self.seal_domain,
            next_seal_sequence: self.next_seal_sequence,
            records: self
                .records
                .iter()
                .filter(|record| !excluded_targets.contains(&record.entity))
                .cloned()
                .collect(),
        }
    }

    pub(crate) fn try_protected_projection_excluding(
        &self,
        excluded_targets: &[Entity],
    ) -> Result<ProtectedRegistryProjection, ()> {
        let record_count = self
            .records
            .iter()
            .filter(|record| !excluded_targets.contains(&record.entity))
            .count();
        let mut records = Vec::new();
        records.try_reserve_exact(record_count).map_err(|_| ())?;
        for record in self
            .records
            .iter()
            .filter(|record| !excluded_targets.contains(&record.entity))
        {
            records.push(try_clone_route_entity_record(record)?);
        }
        Ok(ProtectedRegistryProjection {
            seal_domain: self.seal_domain,
            next_seal_sequence: self.next_seal_sequence,
            records,
        })
    }

    pub(crate) fn matches_protected_projection_excluding(
        &self,
        expected: &ProtectedRegistryProjection,
        excluded_targets: &[Entity],
    ) -> bool {
        self.seal_domain == expected.seal_domain
            && self.next_seal_sequence == expected.next_seal_sequence
            && self
                .records
                .iter()
                .filter(|record| !excluded_targets.contains(&record.entity))
                .eq(expected.records.iter())
    }

    fn allocate_seal(&mut self) -> Result<OwnershipSeal, RegistryWriteError> {
        self.next_seal_sequence = self
            .next_seal_sequence
            .checked_add(1)
            .ok_or(RegistryWriteError::SealExhausted)?;
        Ok(OwnershipSeal {
            domain: self.seal_domain,
            sequence: self.next_seal_sequence,
        })
    }

    pub(crate) fn try_reserve_registration(
        &mut self,
        owner: &RegistryOwner,
    ) -> Result<(), RegistryWriteError> {
        self.try_reserve_registrations(std::slice::from_ref(owner))
    }

    /// Preflights a complete Scene allocation batch before any Entity or
    /// hook-triggering component is created. Repeating the single-item reserve
    /// without inserting records does not reserve space for the whole batch.
    ///
    /// This is metadata preparation, not ownership. Callers still register
    /// each exact Entity, without running application code between
    /// this check and the registrations, then revalidate after component writes.
    pub(crate) fn try_reserve_registrations(
        &mut self,
        owners: &[RegistryOwner],
    ) -> Result<(), RegistryWriteError> {
        for (index, owner) in owners.iter().enumerate() {
            if self
                .records
                .iter()
                .any(|record| record.owner.conflicts_with(owner))
                || owners[..index]
                    .iter()
                    .any(|prior| prior.conflicts_with(owner))
            {
                return Err(RegistryWriteError::DuplicateOwner);
            }
        }
        let count = u64::try_from(owners.len()).map_err(|_| RegistryWriteError::SealExhausted)?;
        self.next_seal_sequence
            .checked_add(count)
            .ok_or(RegistryWriteError::SealExhausted)?;
        self.records
            .try_reserve(owners.len())
            .map_err(|_| RegistryWriteError::CapacityExhausted)
    }

    #[cfg(test)]
    pub(crate) fn exhaust_seal_sequence_for_test(&mut self) {
        self.next_seal_sequence = u64::MAX;
    }

    #[cfg(test)]
    pub(crate) fn issue_unregistered_seal_for_test(&mut self) -> OwnershipSeal {
        self.allocate_seal()
            .expect("ownership seal sequence exhausted")
    }

    #[cfg(test)]
    pub(crate) fn latest_issued_seal(&self) -> Option<OwnershipSeal> {
        (self.next_seal_sequence != 0).then_some(OwnershipSeal {
            domain: self.seal_domain,
            sequence: self.next_seal_sequence,
        })
    }

    /// Issuance precedes conflict checks, so every failed issued attempt still
    /// consumes its sequence. This only updates registry metadata; the caller
    /// receives the exact claim before any component or hook-triggering write.
    pub(crate) fn register(
        &mut self,
        entity: Entity,
        owner: RegistryOwner,
        manifest: ComponentManifest,
        lifecycle: EntityLifecycle,
    ) -> Result<ExactRegistryClaim, RegistryWriteError> {
        let seal = self.allocate_seal()?;
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
        let record = RouteEntityRecord {
            entity,
            owner,
            seal,
            manifest,
            lifecycle,
        };
        let claim = record.claim();
        self.records.push(record);
        Ok(claim)
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
            .find(|record| record.matches_claim(claim))?;
        record.owner = owner;
        Some(record.claim())
    }

    pub(crate) fn compare_exchange_host_top_slot_owner(
        &mut self,
        claim: &ExactRegistryClaim,
        host: HostVersion,
        slot: RouteSlotId,
    ) -> Option<ExactRegistryClaim> {
        let owner = RegistryOwner::Committed(EntityOwner::HostTopSlot {
            host: host.clone(),
            slot,
        });
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
            .find(|record| record.matches_claim(claim))?;
        record.owner = owner;
        Some(ExactRegistryClaim {
            entity: record.entity,
            owner: RegistryOwner::Committed(EntityOwner::HostTopSlot { host, slot }),
            seal: record.seal,
            lifecycle: record.lifecycle,
        })
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

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct QuarantineLeaseId {
    domain: u64,
    sequence: u64,
}

impl QuarantineLeaseId {
    #[cfg(test)]
    pub(crate) const fn sequence_for_test(self) -> u64 {
        self.sequence
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct QuarantineFacts {
    former_app_host: Option<Entity>,
    protected_parent: Option<Entity>,
    foreign_descendants: Vec<Entity>,
    foreign_components: Vec<ComponentId>,
    missing_components: Vec<ComponentId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct QuarantineOrigin {
    router: RouterHandle,
    former_host: HostVersion,
}

impl QuarantineOrigin {
    pub(crate) const fn provisional(router: RouterHandle, former_host: HostVersion) -> Self {
        Self {
            router,
            former_host,
        }
    }

    pub(crate) const fn router(&self) -> RouterHandle {
        self.router
    }

    pub(crate) const fn former_host(&self) -> &HostVersion {
        &self.former_host
    }
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

    pub(crate) const fn former_app_host(&self) -> Option<Entity> {
        self.former_app_host
    }

    pub(crate) const fn protected_parent(&self) -> Option<Entity> {
        self.protected_parent
    }

    pub(crate) fn foreign_descendants(&self) -> &[Entity] {
        &self.foreign_descendants
    }

    pub(crate) fn foreign_components(&self) -> &[ComponentId] {
        &self.foreign_components
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct QuarantineLeaseRecord {
    record: RouteEntityRecord,
    observed_manifest: ComponentManifest,
    foreign_components: Vec<ComponentId>,
    missing_components: Vec<ComponentId>,
}

impl QuarantineLeaseRecord {
    pub(crate) const fn record(&self) -> &RouteEntityRecord {
        &self.record
    }

    pub(crate) const fn entity(&self) -> Entity {
        self.record.entity
    }

    pub(crate) fn claim(&self) -> ExactRegistryClaim {
        self.record.claim()
    }

    pub(crate) fn try_claim(&self) -> Result<ExactRegistryClaim, ()> {
        Ok(ExactRegistryClaim {
            entity: self.record.entity,
            owner: try_clone_registry_owner(&self.record.owner)?,
            seal: self.record.seal,
            lifecycle: self.record.lifecycle,
        })
    }

    pub(crate) const fn observed_manifest(&self) -> &ComponentManifest {
        &self.observed_manifest
    }

    pub(crate) fn foreign_components(&self) -> &[ComponentId] {
        &self.foreign_components
    }

    pub(crate) fn missing_components(&self) -> &[ComponentId] {
        &self.missing_components
    }
}

fn quarantine_root_rank(owner: &RegistryOwner) -> Option<u8> {
    match owner {
        RegistryOwner::Committed(EntityOwner::HostTopSlot { .. })
        | RegistryOwner::Provisional(ProvisionalOwner::HostTopSlot(_)) => Some(0),
        RegistryOwner::Committed(EntityOwner::ChildSlot { .. })
        | RegistryOwner::Provisional(ProvisionalOwner::ChildSlot { .. }) => Some(1),
        RegistryOwner::Committed(EntityOwner::FrameShell(_)) => Some(2),
        RegistryOwner::Committed(EntityOwner::FrameOutput { .. }) => Some(3),
        RegistryOwner::Committed(EntityOwner::FrameContentTarget(_)) => Some(4),
    }
}

fn quarantine_record_is_nested(
    records: &[QuarantineLeaseRecord],
    record: &QuarantineLeaseRecord,
) -> bool {
    match record.record().owner() {
        RegistryOwner::Committed(EntityOwner::HostTopSlot { .. })
        | RegistryOwner::Provisional(ProvisionalOwner::HostTopSlot(_)) => false,
        RegistryOwner::Committed(EntityOwner::ChildSlot { parent, .. })
        | RegistryOwner::Provisional(ProvisionalOwner::ChildSlot { parent, .. }) => {
            records.iter().any(|candidate| {
                matches!(
                    candidate.record().owner(),
                    RegistryOwner::Committed(EntityOwner::FrameShell(frame)) if frame == parent
                )
            })
        }
        RegistryOwner::Committed(EntityOwner::FrameShell(frame)) => {
            records
                .iter()
                .any(|candidate| match candidate.record().owner() {
                    RegistryOwner::Committed(EntityOwner::HostTopSlot { host, slot }) => {
                        *slot == frame.slot() && host_version_matches_frame(host, frame)
                    }
                    RegistryOwner::Committed(EntityOwner::ChildSlot { parent, slot, .. })
                    | RegistryOwner::Provisional(ProvisionalOwner::ChildSlot {
                        parent,
                        slot,
                        ..
                    }) => *slot == frame.slot() && frame_versions_share_router(parent, frame),
                    _ => false,
                })
        }
        RegistryOwner::Committed(EntityOwner::FrameContentTarget(frame))
        | RegistryOwner::Committed(EntityOwner::FrameOutput { frame, .. }) => {
            records.iter().any(|candidate| {
                matches!(
                    candidate.record().owner(),
                    RegistryOwner::Committed(EntityOwner::FrameShell(shell)) if shell == frame
                )
            })
        }
    }
}

fn quarantine_root_record(records: &[QuarantineLeaseRecord]) -> Option<&QuarantineLeaseRecord> {
    records
        .iter()
        .filter_map(|record| {
            quarantine_root_rank(record.record().owner())
                .filter(|_| !quarantine_record_is_nested(records, record))
                .map(|rank| (rank, record.entity().to_bits(), record))
        })
        .min_by_key(|(rank, entity, _)| (*rank, *entity))
        .map(|(_, _, record)| record)
        .or_else(|| {
            records
                .iter()
                .filter_map(|record| {
                    quarantine_root_rank(record.record().owner())
                        .map(|rank| (rank, record.entity().to_bits(), record))
                })
                .min_by_key(|(rank, entity, _)| (*rank, *entity))
                .map(|(_, _, record)| record)
        })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum QuarantineRootKind {
    HostTopSlot,
    ChildSlot,
    FrameShell,
    FrameOutput,
    ResidualContentTarget,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct QuarantineRootProjection {
    entity: Entity,
    kind: QuarantineRootKind,
    committed_host: Option<HostVersion>,
    frame: Option<RouteFrameVersion>,
    provisional_top: bool,
}

impl QuarantineRootProjection {
    fn from_record(record: &QuarantineLeaseRecord) -> Option<Self> {
        let (kind, committed_host, frame, provisional_top) = match record.record().owner() {
            RegistryOwner::Committed(EntityOwner::HostTopSlot { host, .. }) => (
                QuarantineRootKind::HostTopSlot,
                Some(host.clone()),
                None,
                false,
            ),
            RegistryOwner::Provisional(ProvisionalOwner::HostTopSlot(_)) => {
                (QuarantineRootKind::HostTopSlot, None, None, true)
            }
            RegistryOwner::Committed(EntityOwner::ChildSlot { parent, .. })
            | RegistryOwner::Provisional(ProvisionalOwner::ChildSlot { parent, .. }) => (
                QuarantineRootKind::ChildSlot,
                None,
                Some(parent.clone()),
                false,
            ),
            RegistryOwner::Committed(EntityOwner::FrameShell(frame)) => (
                QuarantineRootKind::FrameShell,
                None,
                Some(frame.clone()),
                false,
            ),
            RegistryOwner::Committed(EntityOwner::FrameOutput { frame, .. }) => (
                QuarantineRootKind::FrameOutput,
                None,
                Some(frame.clone()),
                false,
            ),
            RegistryOwner::Committed(EntityOwner::FrameContentTarget(frame)) => (
                QuarantineRootKind::ResidualContentTarget,
                None,
                Some(frame.clone()),
                false,
            ),
        };
        Some(Self {
            entity: record.entity(),
            kind,
            committed_host,
            frame,
            provisional_top,
        })
    }

    pub(crate) const fn entity(&self) -> Entity {
        self.entity
    }

    pub(crate) const fn kind(&self) -> QuarantineRootKind {
        self.kind
    }

    pub(crate) const fn committed_host(&self) -> Option<&HostVersion> {
        self.committed_host.as_ref()
    }

    pub(crate) const fn frame(&self) -> Option<&RouteFrameVersion> {
        self.frame.as_ref()
    }

    pub(crate) const fn provisional_top(&self) -> bool {
        self.provisional_top
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct QuarantineLease {
    id: QuarantineLeaseId,
    generation: u64,
    operation_sequence: u64,
    root: QuarantineRootProjection,
    entity_count: usize,
    records: Vec<QuarantineLeaseRecord>,
    facts: QuarantineFacts,
    origin: Option<QuarantineOrigin>,
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
        self.records
            .iter()
            .map(QuarantineLeaseRecord::entity)
            .collect()
    }
    pub(crate) fn foreign_descendants(&self) -> &[Entity] {
        &self.facts.foreign_descendants
    }

    pub(crate) const fn id(&self) -> QuarantineLeaseId {
        self.id
    }

    pub(crate) const fn root(&self) -> &QuarantineRootProjection {
        &self.root
    }

    pub(crate) const fn entity_count(&self) -> usize {
        self.entity_count
    }

    pub(crate) fn records(&self) -> &[QuarantineLeaseRecord] {
        &self.records
    }

    pub(crate) const fn facts(&self) -> &QuarantineFacts {
        &self.facts
    }

    pub(crate) const fn origin(&self) -> Option<&QuarantineOrigin> {
        self.origin.as_ref()
    }

    #[cfg(test)]
    pub(crate) const fn id_for_test(&self) -> QuarantineLeaseId {
        self.id
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
    root: QuarantineRootProjection,
    entity_count: usize,
    records: Vec<QuarantineLeaseRecord>,
    facts: QuarantineFacts,
    origin: Option<QuarantineOrigin>,
}

pub(crate) struct PreparedUnresolvedResidualTransfer {
    sequence: u64,
    source_claims: Vec<ExactRegistryClaim>,
    records: Vec<RouteEntityRecord>,
    actual_manifests: Vec<(Entity, ComponentManifest)>,
    facts: QuarantineFacts,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ResidualLeaseId {
    domain: u64,
    sequence: u64,
}

impl ResidualLeaseId {
    #[cfg(test)]
    pub(crate) const fn sequence_for_test(self) -> u64 {
        self.sequence
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AuthorityLostResidual {
    id: ResidualLeaseId,
    generation: u64,
    operation_sequence: u64,
    provenance: AuthorityLostResidualProvenance,
    retired_claim: ExactRegistryClaim,
    frozen_expected_manifest: ComponentManifest,
    observed_manifest: Option<ComponentManifest>,
    entity_present_at_receipt: bool,
    observed_registry_claim: Option<ExactRegistryClaim>,
    actual_parent_at_receipt: Option<Entity>,
    facts: QuarantineFacts,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum AuthorityLostResidualProvenance {
    Cleanup(CleanupAuthorityLostResidualProvenance),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CleanupAuthorityLostResidualProvenance {
    ledger_domain: u64,
    manifest_digest: u64,
    attempt_revision: u64,
    obligation: usize,
    obligation_binding: u64,
    terminal_receipt_binding: u64,
}

#[derive(Clone, Copy)]
pub(crate) struct AuthorityLostTombstone<'a> {
    residual: &'a AuthorityLostResidual,
}

impl AuthorityLostTombstone<'_> {
    pub(crate) const fn id(&self) -> ResidualLeaseId {
        self.residual.id
    }

    pub(crate) const fn generation(&self) -> u64 {
        self.residual.generation
    }

    pub(crate) const fn operation_sequence(&self) -> u64 {
        self.residual.operation_sequence
    }

    pub(crate) const fn former_claim(&self) -> &ExactRegistryClaim {
        &self.residual.retired_claim
    }

    pub(crate) fn matches_prepared_terminal(
        &self,
        expected: &PreparedAuthorityLostResidualTerminalProjection,
    ) -> bool {
        self.residual.id == expected.id
            && self.residual.generation == expected.generation
            && self.residual.operation_sequence == expected.operation_sequence
            && self.residual.provenance
                == AuthorityLostResidualProvenance::Cleanup(expected.provenance.clone())
            && self.residual.retired_claim == expected.retired_claim
            && self.residual.frozen_expected_manifest == expected.frozen_expected_manifest
            && self.residual.observed_manifest == expected.observed_manifest
            && self.residual.entity_present_at_receipt == expected.entity_present_at_receipt
            && self.residual.observed_registry_claim == expected.observed_registry_claim
            && self.residual.actual_parent_at_receipt == expected.actual_parent_at_receipt
            && self.residual.facts == expected.facts
    }
}

pub(crate) struct PreparedAuthorityLostResidualTransfer {
    source_claims: Vec<ExactRegistryClaim>,
    residuals: Vec<AuthorityLostResidual>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedAuthorityLostResidualTerminalProjection {
    id: ResidualLeaseId,
    generation: u64,
    operation_sequence: u64,
    provenance: CleanupAuthorityLostResidualProvenance,
    retired_claim: ExactRegistryClaim,
    frozen_expected_manifest: ComponentManifest,
    observed_manifest: Option<ComponentManifest>,
    entity_present_at_receipt: bool,
    observed_registry_claim: Option<ExactRegistryClaim>,
    actual_parent_at_receipt: Option<Entity>,
    facts: QuarantineFacts,
}

impl PreparedAuthorityLostResidualTerminalProjection {
    pub(crate) const fn id(&self) -> ResidualLeaseId {
        self.id
    }

    pub(crate) const fn generation(&self) -> u64 {
        self.generation
    }

    pub(crate) const fn operation_sequence(&self) -> u64 {
        self.operation_sequence
    }

    pub(crate) const fn ledger_domain(&self) -> u64 {
        self.provenance.ledger_domain
    }

    pub(crate) const fn manifest_digest(&self) -> u64 {
        self.provenance.manifest_digest
    }

    pub(crate) const fn attempt_revision(&self) -> u64 {
        self.provenance.attempt_revision
    }

    pub(crate) const fn obligation(&self) -> usize {
        self.provenance.obligation
    }

    pub(crate) const fn obligation_binding(&self) -> u64 {
        self.provenance.obligation_binding
    }

    pub(crate) const fn terminal_receipt_binding(&self) -> u64 {
        self.provenance.terminal_receipt_binding
    }

    pub(crate) const fn retired_claim(&self) -> &ExactRegistryClaim {
        &self.retired_claim
    }

    pub(crate) const fn frozen_expected_manifest(&self) -> &ComponentManifest {
        &self.frozen_expected_manifest
    }

    pub(crate) const fn observed_manifest(&self) -> Option<&ComponentManifest> {
        self.observed_manifest.as_ref()
    }

    pub(crate) const fn entity_present_at_receipt(&self) -> bool {
        self.entity_present_at_receipt
    }

    pub(crate) const fn observed_registry_claim(&self) -> Option<&ExactRegistryClaim> {
        self.observed_registry_claim.as_ref()
    }

    pub(crate) const fn actual_parent_at_receipt(&self) -> Option<Entity> {
        self.actual_parent_at_receipt
    }
}

impl PreparedAuthorityLostResidualTransfer {
    pub(crate) fn terminal_projections(
        &self,
    ) -> impl ExactSizeIterator<Item = PreparedAuthorityLostResidualTerminalProjection> + '_ {
        self.residuals
            .iter()
            .map(|residual| PreparedAuthorityLostResidualTerminalProjection {
                id: residual.id,
                generation: residual.generation,
                operation_sequence: residual.operation_sequence,
                provenance: match &residual.provenance {
                    AuthorityLostResidualProvenance::Cleanup(provenance) => provenance.clone(),
                },
                retired_claim: residual.retired_claim.clone(),
                frozen_expected_manifest: residual.frozen_expected_manifest.clone(),
                observed_manifest: residual.observed_manifest.clone(),
                entity_present_at_receipt: residual.entity_present_at_receipt,
                observed_registry_claim: residual.observed_registry_claim.clone(),
                actual_parent_at_receipt: residual.actual_parent_at_receipt,
                facts: residual.facts.clone(),
            })
    }

    pub(crate) fn is_current(&self, world: &World) -> bool {
        let Some(registry) = world.get_resource::<RouteEntityRegistry>() else {
            return false;
        };
        let Some(table) = world.get_resource::<QuarantineLeaseTable>() else {
            return false;
        };
        !self.residuals.is_empty()
            && self.residuals.iter().all(|residual| {
                let entity = residual.retired_claim.entity();
                let registry_was_exact = self
                    .source_claims
                    .iter()
                    .any(|claim| claim == &residual.retired_claim);
                let registry_is_current = if registry_was_exact {
                    registry
                        .record(entity)
                        .is_some_and(|record| record.claim() == residual.retired_claim)
                        && residual.observed_registry_claim.is_none()
                } else {
                    registry.record(entity).map(RouteEntityRecord::claim)
                        == residual.observed_registry_claim
                };
                residual.id.domain == table.domain
                    && residual.id.sequence != 0
                    && residual.id.sequence <= table.next_residual_sequence
                    && table.record(entity).is_none()
                    && table.unresolved_record(entity).is_none()
                    && table.authority_lost_tombstone(entity).is_none()
                    && registry_is_current
                    && (!registry_was_exact
                        || !cleanup_claim_has_exact_authority(world, &residual.retired_claim))
                    && (world.get_entity(entity).is_ok() == residual.entity_present_at_receipt)
                    && world.get::<ChildOf>(entity).map(ChildOf::parent)
                        == residual.actual_parent_at_receipt
                    && match &residual.observed_manifest {
                        Some(manifest) => {
                            frozen_manifest_matches_archetype_no_alloc(world, entity, manifest)
                        }
                        None => world.get_entity(entity).is_err(),
                    }
            })
    }
}

fn frozen_manifest_matches_archetype_no_alloc(
    world: &World,
    entity: Entity,
    frozen: &ComponentManifest,
) -> bool {
    frozen.matches_without_materialization(world, entity)
}

pub(crate) struct PreparedExactRegistryRetirement {
    claims: Vec<ExactRegistryClaim>,
}

pub(crate) struct PreparedAuthorityPublicationCapacity {
    table_domain: u64,
    lease_len: usize,
    authority_lost_len: usize,
    lease_capacity: usize,
    authority_lost_capacity: usize,
    lease_sequence_start: u64,
    lease_sequence_end: u64,
    residual_sequence_start: u64,
    residual_sequence_end: u64,
    verified_group_count: usize,
    lost_fact_count: usize,
    group_count: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AuthorityPublicationReserveError {
    RetryableCapacity,
    IdentityExhausted,
    InvalidGroups,
}

impl PreparedAuthorityPublicationCapacity {
    pub(crate) fn reserve(
        table: &mut QuarantineLeaseTable,
        verified_group_count: usize,
        lost_fact_count: usize,
        group_count: usize,
    ) -> Result<Self, QuarantineTransferError> {
        Self::reserve_typed(table, verified_group_count, lost_fact_count, group_count).map_err(
            |error| match error {
                AuthorityPublicationReserveError::InvalidGroups => {
                    QuarantineTransferError::UnknownOwnedEntity
                }
                AuthorityPublicationReserveError::RetryableCapacity
                | AuthorityPublicationReserveError::IdentityExhausted => {
                    QuarantineTransferError::CapacityExhausted
                }
            },
        )
    }

    pub(crate) fn reserve_typed(
        table: &mut QuarantineLeaseTable,
        verified_group_count: usize,
        lost_fact_count: usize,
        group_count: usize,
    ) -> Result<Self, AuthorityPublicationReserveError> {
        note_authority_reservation_attempt();
        let lease_reservation_count = if authority_publication_capacity_fault_injected() {
            usize::MAX
        } else {
            verified_group_count
        };
        if verified_group_count > group_count || (group_count == 0 && lost_fact_count != 0) {
            return Err(AuthorityPublicationReserveError::InvalidGroups);
        }
        let lease_sequence_start = table
            .next_sequence
            .checked_add(1)
            .ok_or(AuthorityPublicationReserveError::IdentityExhausted)?;
        let lease_sequence_end = table
            .next_sequence
            .checked_add(verified_group_count as u64)
            .ok_or(AuthorityPublicationReserveError::IdentityExhausted)?;
        let residual_sequence_start = table
            .next_residual_sequence
            .checked_add(1)
            .ok_or(AuthorityPublicationReserveError::IdentityExhausted)?;
        let residual_sequence_end = table
            .next_residual_sequence
            .checked_add(lost_fact_count as u64)
            .ok_or(AuthorityPublicationReserveError::IdentityExhausted)?;
        table
            .leases
            .try_reserve_exact(lease_reservation_count)
            .map_err(|_| AuthorityPublicationReserveError::RetryableCapacity)?;
        table
            .authority_lost
            .try_reserve_exact(lost_fact_count)
            .map_err(|_| AuthorityPublicationReserveError::RetryableCapacity)?;
        table.next_sequence = lease_sequence_end;
        table.next_residual_sequence = residual_sequence_end;
        Ok(Self {
            table_domain: table.domain,
            lease_len: table.leases.len(),
            authority_lost_len: table.authority_lost.len(),
            lease_capacity: table.leases.capacity(),
            authority_lost_capacity: table.authority_lost.capacity(),
            lease_sequence_start,
            lease_sequence_end,
            residual_sequence_start,
            residual_sequence_end,
            verified_group_count,
            lost_fact_count,
            group_count,
        })
    }

    pub(crate) fn revalidates(&self, table: &QuarantineLeaseTable) -> bool {
        table.domain == self.table_domain
            && table.leases.len() == self.lease_len
            && table.authority_lost.len() == self.authority_lost_len
            && table.leases.capacity() == self.lease_capacity
            && table.authority_lost.capacity() == self.authority_lost_capacity
            && table.next_sequence == self.lease_sequence_end
            && table.next_residual_sequence == self.residual_sequence_end
            && self
                .lease_len
                .checked_add(self.verified_group_count)
                .is_some_and(|required| required <= self.lease_capacity)
            && self
                .authority_lost_len
                .checked_add(self.lost_fact_count)
                .is_some_and(|required| required <= self.authority_lost_capacity)
    }

    const fn verified_group_count(&self) -> usize {
        self.verified_group_count
    }

    const fn lost_fact_count(&self) -> usize {
        self.lost_fact_count
    }

    const fn group_count(&self) -> usize {
        self.group_count
    }

    pub(crate) fn verified_id(&self, ordinal: usize) -> Option<QuarantineLeaseId> {
        (ordinal < self.verified_group_count).then(|| QuarantineLeaseId {
            domain: self.table_domain,
            sequence: self.lease_sequence_start + ordinal as u64,
        })
    }

    pub(crate) fn residual_id(&self, offset: usize) -> Option<ResidualLeaseId> {
        (offset < self.lost_fact_count).then(|| ResidualLeaseId {
            domain: self.table_domain,
            sequence: self.residual_sequence_start + offset as u64,
        })
    }

    fn lease_sequence_range(&self) -> std::ops::RangeInclusive<u64> {
        self.lease_sequence_start..=self.lease_sequence_end
    }

    const fn residual_sequence_start(&self) -> u64 {
        self.residual_sequence_start
    }

    const fn residual_sequence_end(&self) -> u64 {
        self.residual_sequence_end
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Resource)]
pub(crate) struct QuarantineLeaseTable {
    domain: u64,
    next_sequence: u64,
    next_residual_sequence: u64,
    leases: Vec<QuarantineLease>,
    unresolved: Vec<UnresolvedResidual>,
    /// Fact-only terminal partition for retired claims whose physical
    /// authority could not be proven. Exact stale registry records may be
    /// removed during publication, but these records never authorize World
    /// mutation; their provenance is the originating cleanup receipt.
    authority_lost: Vec<AuthorityLostResidual>,
}

fn try_clone_copy_slice<T: Copy>(source: &[T]) -> Result<Vec<T>, ()> {
    let mut cloned = Vec::new();
    cloned.try_reserve_exact(source.len()).map_err(|_| ())?;
    cloned.extend_from_slice(source);
    Ok(cloned)
}

fn try_component_difference(
    left: &ComponentManifest,
    right: &ComponentManifest,
) -> Result<Vec<ComponentId>, ()> {
    let mut difference = Vec::new();
    difference
        .try_reserve_exact(left.component_ids().len())
        .map_err(|_| ())?;
    difference.extend(
        left.component_ids()
            .iter()
            .copied()
            .filter(|component| !right.component_ids().contains(component)),
    );
    Ok(difference)
}

fn try_clone_quarantine_facts(source: &QuarantineFacts) -> Result<QuarantineFacts, ()> {
    Ok(QuarantineFacts {
        former_app_host: source.former_app_host,
        protected_parent: source.protected_parent,
        foreign_descendants: try_clone_copy_slice(&source.foreign_descendants)?,
        foreign_components: try_clone_copy_slice(&source.foreign_components)?,
        missing_components: try_clone_copy_slice(&source.missing_components)?,
    })
}

fn try_clone_registry_records(source: &[RouteEntityRecord]) -> Result<Vec<RouteEntityRecord>, ()> {
    let mut records = Vec::new();
    records.try_reserve_exact(source.len()).map_err(|_| ())?;
    for record in source {
        records.push(try_clone_route_entity_record(record)?);
    }
    Ok(records)
}

fn try_clone_quarantine_records(
    source: &[QuarantineLeaseRecord],
) -> Result<Vec<QuarantineLeaseRecord>, ()> {
    try_clone_quarantine_records_with_capacity(source, source.len())
}

fn try_clone_quarantine_records_with_capacity(
    source: &[QuarantineLeaseRecord],
    requested_capacity: usize,
) -> Result<Vec<QuarantineLeaseRecord>, ()> {
    if requested_capacity < source.len() {
        return Err(());
    }
    let mut records = Vec::new();
    records
        .try_reserve_exact(requested_capacity)
        .map_err(|_| ())?;
    for record in source {
        records.push(QuarantineLeaseRecord {
            record: try_clone_route_entity_record(&record.record)?,
            observed_manifest: record.observed_manifest.try_clone()?,
            foreign_components: try_clone_copy_slice(&record.foreign_components)?,
            missing_components: try_clone_copy_slice(&record.missing_components)?,
        });
    }
    Ok(records)
}

fn try_clone_actual_manifests(
    source: &[(Entity, ComponentManifest)],
) -> Result<Vec<(Entity, ComponentManifest)>, ()> {
    let mut manifests = Vec::new();
    manifests.try_reserve_exact(source.len()).map_err(|_| ())?;
    for (entity, manifest) in source {
        manifests.push((*entity, manifest.try_clone()?));
    }
    Ok(manifests)
}

fn try_clone_quarantine_lease(source: &QuarantineLease) -> Result<QuarantineLease, ()> {
    Ok(QuarantineLease {
        id: source.id,
        generation: source.generation,
        operation_sequence: source.operation_sequence,
        root: source.root.clone(),
        entity_count: source.entity_count,
        records: try_clone_quarantine_records(&source.records)?,
        facts: try_clone_quarantine_facts(&source.facts)?,
        origin: source.origin.clone(),
    })
}

fn try_clone_unresolved_residual(source: &UnresolvedResidual) -> Result<UnresolvedResidual, ()> {
    Ok(UnresolvedResidual {
        sequence: source.sequence,
        records: try_clone_registry_records(&source.records)?,
        actual_manifests: try_clone_actual_manifests(&source.actual_manifests)?,
        facts: try_clone_quarantine_facts(&source.facts)?,
    })
}

fn try_clone_authority_lost_residual(
    source: &AuthorityLostResidual,
) -> Result<AuthorityLostResidual, ()> {
    Ok(AuthorityLostResidual {
        id: source.id,
        generation: source.generation,
        operation_sequence: source.operation_sequence,
        provenance: source.provenance.clone(),
        retired_claim: try_clone_exact_registry_claim(&source.retired_claim)?,
        frozen_expected_manifest: source.frozen_expected_manifest.try_clone()?,
        observed_manifest: source
            .observed_manifest
            .as_ref()
            .map(ComponentManifest::try_clone)
            .transpose()?,
        entity_present_at_receipt: source.entity_present_at_receipt,
        observed_registry_claim: source
            .observed_registry_claim
            .as_ref()
            .map(try_clone_exact_registry_claim)
            .transpose()?,
        actual_parent_at_receipt: source.actual_parent_at_receipt,
        facts: try_clone_quarantine_facts(&source.facts)?,
    })
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
            authority_lost: Vec::new(),
        }
    }

    pub(crate) fn try_clone_projection(&self) -> Result<Self, ()> {
        let mut leases = Vec::new();
        leases
            .try_reserve_exact(self.leases.len())
            .map_err(|_| ())?;
        for lease in &self.leases {
            leases.push(try_clone_quarantine_lease(lease)?);
        }

        let mut unresolved = Vec::new();
        unresolved
            .try_reserve_exact(self.unresolved.len())
            .map_err(|_| ())?;
        for residual in &self.unresolved {
            unresolved.push(try_clone_unresolved_residual(residual)?);
        }

        let mut authority_lost = Vec::new();
        authority_lost
            .try_reserve_exact(self.authority_lost.len())
            .map_err(|_| ())?;
        for residual in &self.authority_lost {
            authority_lost.push(try_clone_authority_lost_residual(residual)?);
        }

        Ok(Self {
            domain: self.domain,
            next_sequence: self.next_sequence,
            next_residual_sequence: self.next_residual_sequence,
            leases,
            unresolved,
            authority_lost,
        })
    }

    fn prepare_verified(
        &mut self,
        registry: &RouteEntityRegistry,
        claims: Vec<ExactRegistryClaim>,
        manifests: Vec<(Entity, ComponentManifest)>,
        facts: QuarantineFacts,
        origin: Option<QuarantineOrigin>,
        assigned_id: Option<QuarantineLeaseId>,
    ) -> Result<PreparedQuarantineTransfer, QuarantineTransferError> {
        let reservation_count = if verified_preparation_fault_injected() {
            usize::MAX
        } else {
            claims.len()
        };
        if claims.is_empty() || manifests.len() != claims.len() {
            return Err(QuarantineTransferError::EmptyOwnedSet);
        }
        let mut records = Vec::new();
        records
            .try_reserve(reservation_count)
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
            let mut record = try_clone_route_entity_record(record)
                .map_err(|_| QuarantineTransferError::CapacityExhausted)?;
            record.lifecycle = EntityLifecycle::Quarantined;
            let foreign_components = try_component_difference(manifest, &record.manifest)
                .map_err(|_| QuarantineTransferError::CapacityExhausted)?;
            let missing_components = try_component_difference(&record.manifest, manifest)
                .map_err(|_| QuarantineTransferError::CapacityExhausted)?;
            records.push(QuarantineLeaseRecord {
                record,
                observed_manifest: manifest
                    .try_clone()
                    .map_err(|_| QuarantineTransferError::CapacityExhausted)?,
                foreign_components,
                missing_components,
            });
        }
        records.sort_by_key(|record| record.entity().to_bits());
        records.dedup_by_key(|record| record.entity());
        if records.len() != claims.len() {
            return Err(QuarantineTransferError::UnknownOwnedEntity);
        }
        let root = quarantine_root_record(&records)
            .and_then(QuarantineRootProjection::from_record)
            .ok_or(QuarantineTransferError::UnknownOwnedEntity)?;
        let entity_count = records.len();
        let id = if let Some(id) = assigned_id {
            if id.domain != self.domain || id.sequence == 0 || id.sequence > self.next_sequence {
                return Err(QuarantineTransferError::UnknownOwnedEntity);
            }
            id
        } else {
            let sequence = self
                .next_sequence
                .checked_add(1)
                .ok_or(QuarantineTransferError::CapacityExhausted)?;
            note_authority_reservation_attempt();
            self.leases
                .try_reserve(1)
                .map_err(|_| QuarantineTransferError::CapacityExhausted)?;
            self.next_sequence = sequence;
            QuarantineLeaseId {
                domain: self.domain,
                sequence,
            }
        };
        Ok(PreparedQuarantineTransfer {
            id,
            source_claims: claims,
            root,
            entity_count,
            records,
            facts,
            origin,
        })
    }

    fn apply_prepared(
        &mut self,
        registry: &mut RouteEntityRegistry,
        prepared: PreparedQuarantineTransfer,
    ) -> QuarantineLeaseId {
        let capacity = self.leases.capacity();
        assert!(
            self.leases.len() < capacity,
            "prepared quarantine publication exhausted reserved capacity"
        );
        registry.records.retain(|record| {
            !prepared.source_claims.iter().any(|claim| {
                record.entity == claim.entity
                    && record.owner == claim.owner
                    && record.seal == claim.seal
                    && record.lifecycle == claim.lifecycle
            })
        });
        self.leases.push(QuarantineLease {
            id: prepared.id,
            generation: 1,
            operation_sequence: 0,
            root: prepared.root,
            entity_count: prepared.entity_count,
            records: prepared.records,
            facts: prepared.facts,
            origin: prepared.origin,
        });
        assert_eq!(self.leases.capacity(), capacity);
        prepared.id
    }

    fn prepare_unresolved(
        &mut self,
        registry: &RouteEntityRegistry,
        claims: Vec<ExactRegistryClaim>,
        actual_manifests: Vec<(Entity, ComponentManifest)>,
        facts: QuarantineFacts,
    ) -> Result<PreparedUnresolvedResidualTransfer, QuarantineTransferError> {
        let reservation_count = if unresolved_preparation_fault_injected() {
            usize::MAX
        } else {
            claims.len()
        };
        if claims.is_empty() || actual_manifests.len() != claims.len() {
            return Err(QuarantineTransferError::EmptyOwnedSet);
        }
        let mut records = Vec::new();
        records
            .try_reserve(reservation_count)
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
        // Residual identities are consumed at preparation and never reused,
        // even if a later revalidation abandons the prepared transfer.
        self.next_residual_sequence = sequence;
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
        self.unresolved.push(UnresolvedResidual {
            sequence: prepared.sequence,
            records: prepared.records,
            actual_manifests: prepared.actual_manifests,
            facts: prepared.facts,
        });
    }

    fn prepare_authority_lost(
        &mut self,
        registry: &RouteEntityRegistry,
        facts: PreparedAuthorityLostResidualProjection,
        quarantine_facts: QuarantineFacts,
        assigned_first_id: Option<ResidualLeaseId>,
    ) -> Result<PreparedAuthorityLostResidualTransfer, QuarantineTransferError> {
        if facts.is_empty() {
            return Err(QuarantineTransferError::EmptyOwnedSet);
        }
        let mut receipt_identities = HashSet::new();
        let mut entities = HashSet::new();
        for fact in facts.facts() {
            let entity = fact.retired_claim().entity();
            let registry_exact = registry
                .record(entity)
                .is_some_and(|record| record.claim() == *fact.retired_claim());
            if fact.manifest_digest() == 0
                || fact.obligation_binding() == 0
                || fact.terminal_receipt_binding() == 0
                || !receipt_identities.insert((
                    fact.ledger_domain(),
                    fact.attempt_revision(),
                    fact.obligation(),
                ))
                || !entities.insert(entity)
                || registry_exact != fact.registry_exact_at_receipt()
                || (registry_exact && fact.observed_registry_claim().is_some())
                || (!registry_exact
                    && registry.record(entity).map(RouteEntityRecord::claim)
                        != fact.observed_registry_claim().cloned())
                || self.record(entity).is_some()
                || self.unresolved_record(entity).is_some()
                || self.authority_lost.iter().any(|residual| {
                    residual.retired_claim.entity() == entity
                        || matches!(
                            &residual.provenance,
                            AuthorityLostResidualProvenance::Cleanup(provenance)
                                if (
                                    provenance.ledger_domain,
                                    provenance.attempt_revision,
                                    provenance.obligation,
                                ) == (
                                    fact.ledger_domain(),
                                    fact.attempt_revision(),
                                    fact.obligation(),
                                )
                        )
                })
            {
                return Err(QuarantineTransferError::UnknownOwnedEntity);
            }
        }
        let first_sequence = match assigned_first_id {
            Some(id) if id.domain == self.domain && id.sequence != 0 => id.sequence,
            Some(_) => return Err(QuarantineTransferError::UnknownOwnedEntity),
            None => self
                .next_residual_sequence
                .checked_add(1)
                .ok_or(QuarantineTransferError::CapacityExhausted)?,
        };
        if first_sequence == 0 {
            return Err(QuarantineTransferError::CapacityExhausted);
        }
        let last_sequence = first_sequence
            .checked_add(facts.facts().len() as u64 - 1)
            .ok_or(QuarantineTransferError::CapacityExhausted)?;
        if assigned_first_id.is_some() && last_sequence > self.next_residual_sequence {
            return Err(QuarantineTransferError::UnknownOwnedEntity);
        }
        let mut source_claims = Vec::new();
        source_claims
            .try_reserve_exact(facts.facts().len())
            .map_err(|_| QuarantineTransferError::CapacityExhausted)?;
        source_claims.extend(
            facts
                .facts()
                .iter()
                .filter(|fact| fact.registry_exact_at_receipt())
                .map(|fact| fact.retired_claim().clone()),
        );
        let mut residuals = Vec::new();
        residuals
            .try_reserve_exact(facts.facts().len())
            .map_err(|_| QuarantineTransferError::CapacityExhausted)?;
        residuals.extend(
            facts
                .into_facts()
                .into_iter()
                .enumerate()
                .map(|(offset, fact)| AuthorityLostResidual {
                    id: ResidualLeaseId {
                        domain: self.domain,
                        sequence: first_sequence + offset as u64,
                    },
                    generation: 1,
                    operation_sequence: 0,
                    provenance: AuthorityLostResidualProvenance::Cleanup(
                        CleanupAuthorityLostResidualProvenance {
                            ledger_domain: fact.ledger_domain(),
                            manifest_digest: fact.manifest_digest(),
                            attempt_revision: fact.attempt_revision(),
                            obligation: fact.obligation(),
                            obligation_binding: fact.obligation_binding(),
                            terminal_receipt_binding: fact.terminal_receipt_binding(),
                        },
                    ),
                    retired_claim: fact.retired_claim().clone(),
                    frozen_expected_manifest: fact.frozen_expected_manifest().clone(),
                    observed_manifest: fact.observed_manifest().cloned(),
                    entity_present_at_receipt: fact.entity_present_at_receipt(),
                    observed_registry_claim: fact.observed_registry_claim().cloned(),
                    actual_parent_at_receipt: fact.actual_parent_at_receipt(),
                    facts: quarantine_facts.clone(),
                }),
        );
        if assigned_first_id.is_none() {
            note_authority_reservation_attempt();
            self.authority_lost
                .try_reserve_exact(residuals.len())
                .map_err(|_| QuarantineTransferError::CapacityExhausted)?;
            self.next_residual_sequence = last_sequence;
        }
        Ok(PreparedAuthorityLostResidualTransfer {
            source_claims,
            residuals,
        })
    }

    fn apply_authority_lost(
        &mut self,
        registry: &mut RouteEntityRegistry,
        prepared: PreparedAuthorityLostResidualTransfer,
    ) {
        let capacity = self.authority_lost.capacity();
        assert!(
            self.authority_lost
                .len()
                .checked_add(prepared.residuals.len())
                .is_some_and(|required| required <= capacity),
            "prepared authority-lost publication exhausted reserved capacity"
        );
        registry.records.retain(|record| {
            !prepared.source_claims.iter().any(|claim| {
                record.entity == claim.entity
                    && record.owner == claim.owner
                    && record.seal == claim.seal
                    && record.lifecycle == claim.lifecycle
            })
        });
        self.authority_lost.extend(prepared.residuals);
        assert_eq!(self.authority_lost.capacity(), capacity);
    }

    pub(crate) fn lease(&self, id: QuarantineLeaseId) -> Option<&QuarantineLease> {
        self.leases.iter().find(|lease| lease.id == id)
    }
    pub(crate) fn leases(&self) -> &[QuarantineLease] {
        &self.leases
    }

    #[cfg(test)]
    pub(crate) const fn next_sequence_for_test(&self) -> u64 {
        self.next_sequence
    }

    #[cfg(test)]
    pub(crate) const fn next_residual_sequence_for_test(&self) -> u64 {
        self.next_residual_sequence
    }

    pub(crate) fn record(&self, entity: Entity) -> Option<&RouteEntityRecord> {
        self.leases
            .iter()
            .flat_map(|lease| lease.records.iter())
            .find(|record| record.entity() == entity)
            .map(QuarantineLeaseRecord::record)
    }

    pub(crate) fn unresolved_record(&self, entity: Entity) -> Option<&RouteEntityRecord> {
        self.unresolved
            .iter()
            .flat_map(|residual| residual.records.iter())
            .find(|record| record.entity == entity)
    }

    /// Returns an immutable retired-claim tombstone. This is audit evidence,
    /// never current lifecycle authority.
    pub(crate) fn authority_lost_tombstone(
        &self,
        entity: Entity,
    ) -> Option<AuthorityLostTombstone<'_>> {
        self.authority_lost
            .iter()
            .find(|residual| residual.retired_claim.entity() == entity)
            .map(|residual| AuthorityLostTombstone { residual })
    }

    pub(crate) fn inert_owned_entities(&self) -> impl Iterator<Item = Entity> + '_ {
        self.leases
            .iter()
            .flat_map(|lease| lease.records.iter())
            .map(QuarantineLeaseRecord::record)
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
    let (claims, manifests, facts, origin) = transfer.into_parts();
    table.prepare_verified(registry, claims, manifests, facts, origin, None)
}

pub(crate) fn prepare_verified_frame_quarantine_transfer(
    table: &mut QuarantineLeaseTable,
    registry: &RouteEntityRegistry,
    transfer: VerifiedFrameQuarantineTransfer,
) -> Result<PreparedQuarantineTransfer, QuarantineTransferError> {
    let (claims, manifests, facts) = transfer.into_parts();
    table.prepare_verified(registry, claims, manifests, facts, None, None)
}

pub(crate) fn prepare_verified_cleanup_quarantine_transfer(
    table: &mut QuarantineLeaseTable,
    registry: &RouteEntityRegistry,
    transfer: VerifiedCleanupQuarantineTransfer,
) -> Result<PreparedQuarantineTransfer, QuarantineTransferError> {
    let (claims, manifests, facts) = transfer.into_parts();
    table.prepare_verified(registry, claims, manifests, facts, None, None)
}

pub(crate) fn prepare_verified_authority_cleanup_quarantine_transfer(
    table: &mut QuarantineLeaseTable,
    registry: &RouteEntityRegistry,
    transfer: VerifiedCleanupQuarantineTransfer,
    id: QuarantineLeaseId,
) -> Result<PreparedQuarantineTransfer, QuarantineTransferError> {
    let (claims, manifests, facts) = transfer.into_parts();
    table.prepare_verified(registry, claims, manifests, facts, None, Some(id))
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

pub(crate) fn prepare_authority_lost_residual_transfer(
    table: &mut QuarantineLeaseTable,
    registry: &RouteEntityRegistry,
    facts: PreparedAuthorityLostResidualProjection,
    quarantine_facts: QuarantineFacts,
) -> Result<PreparedAuthorityLostResidualTransfer, QuarantineTransferError> {
    table.prepare_authority_lost(registry, facts, quarantine_facts, None)
}

pub(crate) fn prepare_authority_lost_residual_transfer_in_publication(
    table: &mut QuarantineLeaseTable,
    registry: &RouteEntityRegistry,
    facts: PreparedAuthorityLostResidualProjection,
    quarantine_facts: QuarantineFacts,
    first_id: ResidualLeaseId,
) -> Result<PreparedAuthorityLostResidualTransfer, QuarantineTransferError> {
    table.prepare_authority_lost(registry, facts, quarantine_facts, Some(first_id))
}

pub(crate) fn apply_prepared_authority_lost_residual_transfer(
    table: &mut QuarantineLeaseTable,
    registry: &mut RouteEntityRegistry,
    prepared: PreparedAuthorityLostResidualTransfer,
) {
    table.apply_authority_lost(registry, prepared);
}

pub(crate) fn prepare_exact_registry_retirement(
    registry: &RouteEntityRegistry,
    claims: Vec<ExactRegistryClaim>,
) -> Result<PreparedExactRegistryRetirement, QuarantineTransferError> {
    let mut entities = HashSet::new();
    if claims.iter().any(|claim| {
        !entities.insert(claim.entity())
            || !registry
                .record(claim.entity())
                .is_some_and(|record| record.claim() == *claim)
    }) {
        return Err(QuarantineTransferError::UnknownOwnedEntity);
    }
    Ok(PreparedExactRegistryRetirement { claims })
}

pub(crate) fn apply_prepared_exact_registry_retirement(
    registry: &mut RouteEntityRegistry,
    prepared: PreparedExactRegistryRetirement,
) {
    registry.records.retain(|record| {
        !prepared.claims.iter().any(|claim| {
            record.entity == claim.entity
                && record.owner == claim.owner
                && record.seal == claim.seal
                && record.lifecycle == claim.lifecycle
        })
    });
}

impl Default for QuarantineLeaseTable {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
pub(crate) mod tests {
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

    #[test]
    fn authority_publication_capacity_owns_three_verified_closures() {
        let mut table = QuarantineLeaseTable::new();
        table.leases.shrink_to_fit();

        let owner = PreparedAuthorityPublicationCapacity::reserve(&mut table, 3, 0, 3)
            .expect("three verified closures must reserve one aggregate publication tail");

        assert_eq!(owner.verified_group_count(), 3);
        assert_eq!(owner.lost_fact_count(), 0);
        assert_eq!(owner.group_count(), 3);
        assert!(owner.revalidates(&table));
        assert!(table.leases.capacity() - table.leases.len() >= 3);
    }

    #[test]
    fn authority_publication_capacity_burns_three_lost_fact_ids() {
        let mut table = QuarantineLeaseTable::new();
        let before = table.next_residual_sequence;

        let owner = PreparedAuthorityPublicationCapacity::reserve(&mut table, 0, 3, 3)
            .expect("three lost-bearing closures must reserve one aggregate fact tail");

        assert_eq!(owner.residual_sequence_start(), before + 1);
        assert_eq!(owner.residual_sequence_end(), before + 3);
        assert_eq!(table.next_residual_sequence, before + 3);
        assert!(owner.revalidates(&table));
    }

    #[test]
    fn authority_publication_capacity_owns_mixed_verified_and_lost_counts() {
        let mut table = QuarantineLeaseTable::new();

        let owner = PreparedAuthorityPublicationCapacity::reserve(&mut table, 2, 3, 4)
            .expect("mixed closures must share one aggregate capacity and identity owner");

        assert_eq!(owner.lease_sequence_range(), 1..=2);
        assert_eq!(owner.residual_sequence_start(), 1);
        assert_eq!(owner.residual_sequence_end(), 3);
        assert_eq!(table.next_sequence, 2);
        assert_eq!(table.next_residual_sequence, 3);
        assert!(owner.revalidates(&table));
    }

    #[test]
    fn authority_publication_reservation_distinguishes_capacity_from_identity_exhaustion() {
        let mut table = QuarantineLeaseTable::new();
        fail_next_authority_publication_capacity_reservation_for_test();
        assert!(matches!(
            PreparedAuthorityPublicationCapacity::reserve_typed(&mut table, 1, 0, 1),
            Err(AuthorityPublicationReserveError::RetryableCapacity)
        ));
        assert_eq!(table.next_sequence, 0);
        let reserved =
            PreparedAuthorityPublicationCapacity::reserve_typed(&mut table, 1, 0, 1).unwrap();
        assert!(reserved.revalidates(&table));
        assert_eq!(table.next_sequence, 1);
        table.next_sequence = u64::MAX;
        assert!(matches!(
            PreparedAuthorityPublicationCapacity::reserve_typed(&mut table, 1, 0, 1),
            Err(AuthorityPublicationReserveError::IdentityExhausted)
        ));
        assert_eq!(table.next_sequence, u64::MAX);
    }

    #[test]
    fn insufficient_aggregate_authority_capacity_is_semantically_zero() {
        let mut table = QuarantineLeaseTable::new();
        let before = (
            table.next_sequence,
            table.next_residual_sequence,
            table.leases.len(),
            table.authority_lost.len(),
        );
        fail_next_authority_publication_capacity_reservation_for_test();

        assert!(matches!(
            PreparedAuthorityPublicationCapacity::reserve(&mut table, 3, 3, 3),
            Err(QuarantineTransferError::CapacityExhausted)
        ));
        assert_eq!(
            (
                table.next_sequence,
                table.next_residual_sequence,
                table.leases.len(),
                table.authority_lost.len(),
            ),
            before
        );
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
    fn structural_manifest_ignores_only_official_transient_clipping_presence() {
        let mut world = World::new();
        let entity = world.spawn((bevy_ui::Node::default(), A)).id();
        let expected = ComponentManifest::from_world(&world, entity).unwrap();
        for clipped in [true, false, true] {
            if clipped {
                world.entity_mut(entity).insert(bevy_ui::CalculatedClip {
                    clip: Default::default(),
                });
            } else {
                world.entity_mut(entity).remove::<bevy_ui::CalculatedClip>();
            }
            assert_eq!(
                ComponentManifest::from_world(&world, entity).as_ref(),
                Some(&expected)
            );
            assert_eq!(
                ComponentManifest::try_from_world(&world, entity)
                    .unwrap()
                    .as_ref(),
                Some(&expected)
            );
            assert!(expected.matches(&world, entity));
            {
                take_authority_postmutation_manifest_materializations_for_test();
                let _mutation = begin_authority_publication_mutation_for_test();
                assert!(expected.matches_without_materialization(&world, entity));
                assert!(frozen_manifest_matches_archetype_no_alloc(
                    &world, entity, &expected
                ));
                assert_eq!(
                    take_authority_postmutation_manifest_materializations_for_test(),
                    0
                );
            }
            assert_eq!(
                world.get::<bevy_ui::CalculatedClip>(entity).is_some(),
                clipped
            );
        }
        world.entity_mut(entity).insert(B);
        assert!(!expected.matches(&world, entity));
        assert!(!expected.matches_without_materialization(&world, entity));
        world.entity_mut(entity).remove::<B>();
        world.entity_mut(entity).insert(bevy_ui::OverrideClip);
        assert!(!expected.matches_without_materialization(&world, entity));
        world.entity_mut(entity).remove::<bevy_ui::OverrideClip>();
        world
            .entity_mut(entity)
            .remove::<bevy_camera::visibility::Visibility>();
        assert!(!expected.matches_without_materialization(&world, entity));
    }

    #[test]
    fn failed_registrations_preserve_records_and_never_reuse_issued_seals() {
        let mut registry = RouteEntityRegistry::new();
        let mut world = World::new();
        let first = world.spawn_empty().id();
        let second = world.spawn_empty().id();
        let owner = RegistryOwner::Provisional(ProvisionalOwner::HostTopSlot(
            ProvisionalTopSlotOwner::new(HostCandidateRevision::fixture(1)),
        ));
        let other_owner = RegistryOwner::Provisional(ProvisionalOwner::HostTopSlot(
            ProvisionalTopSlotOwner::new(HostCandidateRevision::fixture(2)),
        ));
        let manifest = ComponentManifest::new([]);
        let first_claim = registry
            .register(
                first,
                owner.clone(),
                manifest.clone(),
                EntityLifecycle::PreparedInert,
            )
            .unwrap();
        assert_eq!(registry.record(first).unwrap().claim(), first_claim);
        assert_eq!(first_claim.seal().sequence, 1);
        let baseline = registry.records().to_vec();
        assert_eq!(
            registry.register(
                first,
                other_owner.clone(),
                manifest.clone(),
                EntityLifecycle::PreparedInert,
            ),
            Err(RegistryWriteError::DuplicateEntity)
        );
        let duplicate_entity_seal = registry.latest_issued_seal().unwrap();
        assert_eq!(duplicate_entity_seal.sequence, 2);
        assert_eq!(registry.records(), baseline);
        assert_eq!(
            registry.register(
                second,
                owner,
                manifest.clone(),
                EntityLifecycle::PreparedInert,
            ),
            Err(RegistryWriteError::DuplicateOwner)
        );
        let duplicate_owner_seal = registry.latest_issued_seal().unwrap();
        assert_eq!(duplicate_owner_seal.sequence, 3);
        assert_eq!(registry.records(), baseline);
        let second_claim = registry
            .register(
                second,
                other_owner,
                manifest,
                EntityLifecycle::PreparedInert,
            )
            .unwrap();
        assert_eq!(second_claim.seal().sequence, 4);
        assert_ne!(second_claim.seal(), duplicate_entity_seal);
        assert_ne!(second_claim.seal(), duplicate_owner_seal);
    }

    #[test]
    fn registration_batch_reserves_every_record_and_does_not_recycle_retired_seals() {
        let frame = frame_fixture();
        let owners = (0..13)
            .map(|local_index| {
                RegistryOwner::Committed(EntityOwner::FrameOutput {
                    frame: frame.clone(),
                    output: 1,
                    local_index,
                })
            })
            .collect::<Vec<_>>();
        let mut registry = RouteEntityRegistry::new();
        registry.try_reserve_registrations(&owners).unwrap();
        let capacity = registry.records.capacity();
        assert_eq!(
            registry.next_seal_sequence, 0,
            "reservation is not seal issuance"
        );
        assert!(registry.records.is_empty());
        let mut world = World::new();
        for owner in owners {
            let entity = world.spawn_empty().id();
            registry
                .register(
                    entity,
                    owner,
                    ComponentManifest::new([]),
                    EntityLifecycle::PreparedInert,
                )
                .unwrap();
            assert_eq!(
                registry.records.capacity(),
                capacity,
                "every registration must fit the prepared batch capacity"
            );
        }
        let retired = registry.records[0].claim();
        assert!(registry.remove_exact(&retired));
        let replacement = registry
            .register(
                retired.entity(),
                retired.owner().clone(),
                ComponentManifest::new([]),
                retired.lifecycle(),
            )
            .unwrap();
        assert_ne!(replacement.seal(), retired.seal());
        assert_eq!(replacement.seal().sequence, 14);
        let baseline = registry.records().to_vec();
        assert!(
            registry
                .compare_exchange_lifecycle(&retired, EntityLifecycle::Active)
                .is_none()
        );
        assert!(
            registry
                .compare_exchange_owner(&retired, retired.owner().clone())
                .is_none()
        );
        assert!(!registry.compare_exchange_manifest_exact(
            &retired,
            &ComponentManifest::new([]),
            ComponentManifest::new([world.register_component::<A>()]),
        ));
        assert!(!registry.remove_exact(&retired));
        assert_eq!(registry.records(), baseline);
        assert_eq!(registry.records.capacity(), capacity);
    }

    #[test]
    fn protected_projection_detects_registration_even_after_record_removal() {
        let mut registry = RouteEntityRegistry::new();
        let mut world = World::new();
        let entity = world.spawn_empty().id();
        let source = registry.protected_projection_excluding(&[]);
        let mut bound = registry.try_protected_projection_excluding(&[]).unwrap();
        let claim = registry
            .register(
                entity,
                RegistryOwner::Provisional(ProvisionalOwner::HostTopSlot(
                    ProvisionalTopSlotOwner::new(HostCandidateRevision::fixture(1)),
                )),
                ComponentManifest::new([]),
                EntityLifecycle::PreparedInert,
            )
            .unwrap();
        assert!(bound.bind_single_registration(claim.seal()));
        assert!(registry.matches_protected_projection_excluding(&bound, &[entity]));
        assert!(registry.remove_exact(&claim));
        assert!(registry.records().is_empty());
        assert!(!registry.matches_protected_projection_excluding(&source, &[]));
    }

    #[test]
    fn protected_projection_detects_failed_registration_without_record_changes() {
        let mut registry = RouteEntityRegistry::new();
        let mut world = World::new();
        let entity = world.spawn_empty().id();
        let owner = RegistryOwner::Provisional(ProvisionalOwner::HostTopSlot(
            ProvisionalTopSlotOwner::new(HostCandidateRevision::fixture(1)),
        ));
        registry
            .register(
                entity,
                owner.clone(),
                ComponentManifest::new([]),
                EntityLifecycle::PreparedInert,
            )
            .unwrap();
        let source = registry.try_protected_projection_excluding(&[]).unwrap();
        let records = registry.records().to_vec();
        assert_eq!(
            registry.register(
                entity,
                owner,
                ComponentManifest::new([]),
                EntityLifecycle::PreparedInert,
            ),
            Err(RegistryWriteError::DuplicateEntity)
        );
        assert_eq!(registry.records(), records);
        assert!(!registry.matches_protected_projection_excluding(&source, &[]));
    }

    #[test]
    fn protected_projection_detects_registry_replacement_in_the_same_tick() {
        let mut world = World::new();
        world.init_resource::<RouteEntityRegistry>();
        let source = world
            .resource::<RouteEntityRegistry>()
            .protected_projection_excluding(&[]);
        let tick = world.read_change_tick();
        world.insert_resource(RouteEntityRegistry::new());
        assert_eq!(world.read_change_tick(), tick);
        assert!(
            !world
                .resource::<RouteEntityRegistry>()
                .matches_protected_projection_excluding(&source, &[])
        );
    }

    #[test]
    fn registration_batch_rejects_internal_and_existing_owner_conflicts_without_issuance() {
        let (frame, _, slot) = child_owner_fixture();
        let provisional = RegistryOwner::Provisional(ProvisionalOwner::ChildSlot {
            parent: frame.clone(),
            outlet: RouteSlotKey::default(),
            slot,
        });
        let committed = RegistryOwner::Committed(EntityOwner::ChildSlot {
            parent: frame,
            outlet: RouteSlotKey::default(),
            slot,
        });
        let mut registry = RouteEntityRegistry::new();
        let before = registry.clone();
        assert_eq!(
            registry.try_reserve_registrations(&[provisional.clone(), committed.clone()]),
            Err(RegistryWriteError::DuplicateOwner)
        );
        assert_eq!(registry, before);
        let mut world = World::new();
        let entity = world.spawn_empty().id();
        registry
            .register(
                entity,
                provisional,
                ComponentManifest::new([]),
                EntityLifecycle::PreparedInert,
            )
            .unwrap();
        let before = registry.clone();
        assert_eq!(
            registry.try_reserve_registrations(&[committed]),
            Err(RegistryWriteError::DuplicateOwner)
        );
        assert_eq!(registry, before);
    }

    #[test]
    fn registration_batch_seal_exhaustion_is_permanent_and_zero_batch_is_a_no_op() {
        let frame = frame_fixture();
        let owners = [
            RegistryOwner::Committed(EntityOwner::FrameShell(frame.clone())),
            RegistryOwner::Committed(EntityOwner::FrameContentTarget(frame)),
        ];
        let mut registry = RouteEntityRegistry::new();
        registry.next_seal_sequence = u64::MAX - 1;
        let before = registry.clone();
        assert_eq!(
            registry.try_reserve_registrations(&owners),
            Err(RegistryWriteError::SealExhausted)
        );
        assert_eq!(registry, before);
        registry.try_reserve_registrations(&owners[..1]).unwrap();
        let mut world = World::new();
        let last = registry
            .register(
                world.spawn_empty().id(),
                owners[0].clone(),
                ComponentManifest::new([]),
                EntityLifecycle::PreparedInert,
            )
            .unwrap();
        assert_eq!(last.seal().sequence, u64::MAX);
        let exhausted = registry.clone();
        for _ in 0..2 {
            assert_eq!(
                registry.try_reserve_registrations(&owners[1..]),
                Err(RegistryWriteError::SealExhausted)
            );
            assert_eq!(
                registry.register(
                    world.spawn_empty().id(),
                    owners[1].clone(),
                    ComponentManifest::new([]),
                    EntityLifecycle::PreparedInert,
                ),
                Err(RegistryWriteError::SealExhausted)
            );
            assert_eq!(registry, exhausted);
        }
        registry.try_reserve_registrations(&[]).unwrap();
        assert_eq!(registry, exhausted);
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

        registry
            .register(
                first,
                RegistryOwner::Provisional(owner.clone()),
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
        registry
            .register(
                first,
                RegistryOwner::Provisional(ProvisionalOwner::ChildSlot {
                    parent: parent.clone(),
                    outlet: outlet.clone(),
                    slot: child_slot,
                }),
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
                ComponentManifest::new([]),
                EntityLifecycle::PreparedInert,
            ),
            Err(RegistryWriteError::DuplicateOwner)
        );
        assert_eq!(registry.records(), baseline);
    }

    pub(crate) fn opaque_router_registry_partition_fixture() {
        let (first_parent, _, first_child_slot) = child_owner_fixture();
        let (second_parent, _, second_child_slot) = child_owner_fixture();
        assert_eq!(first_parent.router(), second_parent.router());
        assert_eq!(first_child_slot, second_child_slot);
        assert!(!frame_versions_share_router(&first_parent, &second_parent));

        let mut registry = RouteEntityRegistry::new();
        let mut world = World::new();
        let first = world.spawn_empty().id();
        let second = world.spawn_empty().id();
        let outlet = RouteSlotKey::default();
        registry
            .register(
                first,
                RegistryOwner::Provisional(ProvisionalOwner::ChildSlot {
                    parent: first_parent,
                    outlet: outlet.clone(),
                    slot: first_child_slot,
                }),
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
                ComponentManifest::new([]),
                EntityLifecycle::PreparedInert,
            )
            .expect("separate opaque router instances may reuse raw slot projections");
    }

    #[test]
    fn child_owner_uniqueness_keeps_opaque_router_instances_independent() {
        opaque_router_registry_partition_fixture();
    }

    #[test]
    fn exact_manifest_cas_rejects_stale_old_manifest_without_write() {
        let mut registry = RouteEntityRegistry::new();
        let mut world = World::new();
        let entity = world.spawn_empty().id();
        registry
            .register(
                entity,
                RegistryOwner::Provisional(ProvisionalOwner::HostTopSlot(
                    ProvisionalTopSlotOwner::new(HostCandidateRevision::fixture(31)),
                )),
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
            registry
                .register(
                    entity,
                    RegistryOwner::Provisional(ProvisionalOwner::HostTopSlot(
                        ProvisionalTopSlotOwner::new(HostCandidateRevision::fixture(revision)),
                    )),
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
    fn registry_domains_reject_foreign_exact_claims_without_partial_write() {
        let mut registry = RouteEntityRegistry::new();
        let mut foreign = RouteEntityRegistry::new();
        let mut world = World::new();
        let entity = world.spawn_empty().id();
        let (parent, _, slot) = child_owner_fixture();
        let owner = RegistryOwner::Provisional(ProvisionalOwner::ChildSlot {
            parent: parent.clone(),
            outlet: RouteSlotKey::default(),
            slot,
        });
        let manifest = ComponentManifest::new([]);
        let claim = registry
            .register(
                entity,
                owner.clone(),
                manifest.clone(),
                EntityLifecycle::PreparedInert,
            )
            .unwrap();
        let foreign_claim = foreign
            .register(
                entity,
                owner,
                manifest.clone(),
                EntityLifecycle::PreparedInert,
            )
            .unwrap();
        assert_eq!(claim.seal().sequence, foreign_claim.seal().sequence);
        assert_ne!(claim.seal().domain, foreign_claim.seal().domain);
        let baseline = registry.clone();
        assert!(
            registry
                .compare_exchange_lifecycle(&foreign_claim, EntityLifecycle::Active)
                .is_none()
        );
        assert!(
            registry
                .compare_exchange_owner(
                    &foreign_claim,
                    RegistryOwner::Committed(EntityOwner::ChildSlot {
                        parent,
                        outlet: RouteSlotKey::default(),
                        slot,
                    }),
                )
                .is_none()
        );
        assert!(!registry.compare_exchange_manifest_exact(
            &foreign_claim,
            &manifest,
            ComponentManifest::new([world.register_component::<A>()]),
        ));
        assert!(!registry.remove_exact(&foreign_claim));
        assert_eq!(registry, baseline);
    }

    #[test]
    fn registry_lifecycle_and_removal_are_exact_claim_compare_and_swap() {
        let mut registry = RouteEntityRegistry::new();
        let mut world = World::new();
        let entity = world.spawn_empty().id();
        let owner = RegistryOwner::Provisional(ProvisionalOwner::HostTopSlot(
            ProvisionalTopSlotOwner::new(HostCandidateRevision::fixture(9)),
        ));
        registry
            .register(
                entity,
                owner,
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
        let first_claim = registry
            .register(
                first,
                first_owner,
                ComponentManifest::new([]),
                EntityLifecycle::PreparedInert,
            )
            .unwrap();
        registry
            .register(
                second,
                second_owner,
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
        assert_eq!(table.lease(lease).unwrap().entity_count(), 2);
        assert_eq!(table.lease(lease).unwrap().root().entity(), expected[0]);
        assert_eq!(
            table.lease(lease).unwrap().root().kind(),
            QuarantineRootKind::HostTopSlot
        );
        assert_eq!(
            table.lease(lease).unwrap().foreign_descendants(),
            &[foreign]
        );
        assert!(registry.records().is_empty());

        let baseline = table.leases().to_vec();
        let replacement = registry
            .register(
                first,
                first_claim.owner().clone(),
                ComponentManifest::new([]),
                EntityLifecycle::PreparedInert,
            )
            .unwrap();
        assert_ne!(replacement.seal(), first_claim.seal());
        assert!(
            registry
                .compare_exchange_lifecycle(&first_claim, EntityLifecycle::Active)
                .is_none()
        );
        assert!(!registry.remove_exact(&first_claim));
        assert_eq!(registry.record(first).unwrap().claim(), replacement);
        assert_eq!(table.leases(), baseline);
    }

    #[test]
    fn quarantine_transfer_preserves_expected_and_observed_manifests_per_record() {
        let mut world = World::new();
        let entity = world.spawn(A).id();
        let expected = ComponentManifest::from_world(&world, entity).unwrap();
        let mut registry = RouteEntityRegistry::new();
        registry
            .register(
                entity,
                RegistryOwner::Provisional(ProvisionalOwner::HostTopSlot(
                    ProvisionalTopSlotOwner::new(HostCandidateRevision::fixture(7)),
                )),
                expected.clone(),
                EntityLifecycle::PreparedInert,
            )
            .unwrap();
        world.entity_mut(entity).insert(B);
        let observed = ComponentManifest::from_world(&world, entity).unwrap();
        let foreign = world.components().component_id::<B>().unwrap();
        let mut table = QuarantineLeaseTable::new();
        let transfer = VerifiedQuarantineTransfer::fixture(
            vec![registry.record(entity).unwrap().claim()],
            vec![(entity, observed.clone())],
            QuarantineFacts::new(None, None, vec![], vec![foreign]),
        );

        let prepared =
            prepare_verified_quarantine_transfer(&mut table, &registry, transfer).unwrap();
        let lease = apply_prepared_quarantine_transfer(&mut table, &mut registry, prepared);
        let record = &table.lease(lease).unwrap().records[0];

        assert_eq!(record.record().manifest(), &expected);
        assert_eq!(record.observed_manifest(), &observed);
        assert_eq!(record.foreign_components(), &[foreign]);
        assert!(record.missing_components().is_empty());
    }

    #[test]
    fn quarantine_transfer_derives_record_drift_without_trusting_aggregate_facts() {
        let mut world = World::new();
        let entity = world.spawn(A).id();
        let expected = ComponentManifest::from_world(&world, entity).unwrap();
        let mut registry = RouteEntityRegistry::new();
        registry
            .register(
                entity,
                RegistryOwner::Provisional(ProvisionalOwner::HostTopSlot(
                    ProvisionalTopSlotOwner::new(HostCandidateRevision::fixture(8)),
                )),
                expected,
                EntityLifecycle::PreparedInert,
            )
            .unwrap();
        world.entity_mut(entity).insert(B);
        let observed = ComponentManifest::from_world(&world, entity).unwrap();
        let foreign = world.components().component_id::<B>().unwrap();
        let mut table = QuarantineLeaseTable::new();
        let transfer = VerifiedQuarantineTransfer::fixture(
            vec![registry.record(entity).unwrap().claim()],
            vec![(entity, observed)],
            QuarantineFacts::new(None, None, vec![], vec![]),
        );

        let prepared =
            prepare_verified_quarantine_transfer(&mut table, &registry, transfer).unwrap();
        let lease = apply_prepared_quarantine_transfer(&mut table, &mut registry, prepared);
        assert_eq!(
            table.lease(lease).unwrap().records[0].foreign_components(),
            &[foreign]
        );
    }

    #[test]
    fn quarantine_transfer_rejects_stale_exact_claim_without_writes() {
        let mut world = World::new();
        let entity = world.spawn_empty().id();
        let mut registry = RouteEntityRegistry::new();
        registry
            .register(
                entity,
                RegistryOwner::Provisional(ProvisionalOwner::HostTopSlot(
                    ProvisionalTopSlotOwner::new(HostCandidateRevision::fixture(13)),
                )),
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
}
