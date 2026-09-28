use std::collections::{
    HashMap,
    hash_map::{Entry, OccupiedEntry},
};

use bevy_ui_router_core::{
    ids::NavigationRequestId,
    integration::{
        BackAcceptedReservationView, BackNoTargetCleanupBatchView, BackResolutionScopeView,
        EnqueueBackErrorKind,
    },
};

use crate::navigation::ProvenEmptyProjection;

#[cfg(test)]
use super::{
    BackNoTargetZeroCompletionFacts, canonical_empty_completion_digest,
    canonical_zero_manifest_digest,
};
use super::{
    CleanupLedger, CleanupMountParentSettlement, CleanupObligation, CleanupReceipt,
    InstalledBackCapacityLease, NEXT_LEDGER_DOMAIN, PreparedBackNoTargetZeroCompletion,
    allocate_ledger_domain, ledger_domain_counter_input, observe_ledger_domain_allocation,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PreparationCapacityDomain(pub(super) u64);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct PreparationReleaseGeneration(pub(super) u64);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct BackCapacityOwnerId(pub(super) u64);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) struct BackCapacityCellId(pub(super) usize);

#[derive(Clone, Copy)]
enum BackCapacityCellState {
    Free,
    Outstanding { owner: BackCapacityOwnerId },
    Leased { owner: BackCapacityOwnerId },
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum BackCapacityReuse {
    Reusable,
    PermanentlyRetired,
}

struct BackCapacityCell {
    generation: PreparationReleaseGeneration,
    state: BackCapacityCellState,
    reuse: BackCapacityReuse,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum BackCapacityLiveState {
    Outstanding,
    Leased,
}

struct BackCapacityLiveRecord {
    domain: PreparationCapacityDomain,
    cell: BackCapacityCellId,
    owner: BackCapacityOwnerId,
    generation: PreparationReleaseGeneration,
    state: BackCapacityLiveState,
}

pub(crate) struct BackCapacityDomainIssuer {
    next_domain: u64,
}

impl BackCapacityDomainIssuer {
    pub(crate) const fn new() -> Self {
        Self { next_domain: 1 }
    }

    pub(crate) fn issue(&mut self) -> Result<BackCapacityIssuer, BackCapacityReserveError> {
        let next_domain = self
            .next_domain
            .checked_add(1)
            .ok_or(BackCapacityReserveError::DomainExhausted)?;
        let domain = PreparationCapacityDomain(self.next_domain);
        self.next_domain = next_domain;
        Ok(BackCapacityIssuer {
            domain,
            cells: Vec::new(),
            live: HashMap::new(),
            next_owner: BackCapacityOwnerId(1),
            #[cfg(test)]
            back_capacity_order_producer: None,
            #[cfg(test)]
            next_cell_growth_additional_capacity: None,
            #[cfg(test)]
            next_new_cell_initial_generation: None,
            #[cfg(test)]
            drift_next_retire_unbound_generation: false,
        })
    }
}

pub(crate) struct BackCapacityIssuer {
    domain: PreparationCapacityDomain,
    cells: Vec<BackCapacityCell>,
    live: HashMap<BackCapacityOwnerId, BackCapacityLiveRecord>,
    next_owner: BackCapacityOwnerId,
    #[cfg(test)]
    back_capacity_order_producer:
        Option<bevy_ui_router_core::test_support::BackCapacityIssuerProducer>,
    #[cfg(test)]
    next_cell_growth_additional_capacity: Option<usize>,
    #[cfg(test)]
    next_new_cell_initial_generation: Option<PreparationReleaseGeneration>,
    #[cfg(test)]
    drift_next_retire_unbound_generation: bool,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct BackCapacityIssuerSemanticSnapshot {
    cell_count: usize,
    outstanding_count: usize,
    next_owner: BackCapacityOwnerId,
    next_generation: PreparationReleaseGeneration,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BackCapacityCellDispositionForTest {
    Outstanding { owner: u64 },
    Leased { owner: u64 },
    ReusableFree,
    PermanentlyRetired,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct BackCapacityCellProjectionForTest {
    pub(crate) current_generation: u64,
    pub(crate) original_owner_live: bool,
    pub(crate) disposition: BackCapacityCellDispositionForTest,
}

#[cfg(test)]
impl BackCapacityIssuerSemanticSnapshot {
    pub(crate) const fn cell_count(&self) -> usize {
        self.cell_count
    }

    pub(crate) const fn outstanding_count(&self) -> usize {
        self.outstanding_count
    }

    pub(crate) const fn next_owner(&self) -> u64 {
        self.next_owner.0
    }

    pub(crate) const fn next_generation(&self) -> u64 {
        self.next_generation.0
    }
}

impl BackCapacityIssuer {
    #[cfg(test)]
    pub(crate) fn back_capacity_order_producer_slot_is_vacant(&self) -> bool {
        self.back_capacity_order_producer.is_none()
    }

    #[cfg(test)]
    pub(crate) fn install_back_capacity_order_producer(
        &mut self,
        producer: bevy_ui_router_core::test_support::BackCapacityIssuerProducer,
    ) {
        self.back_capacity_order_producer = Some(producer);
    }

    #[cfg(test)]
    pub(crate) const fn domain_for_test(&self) -> u64 {
        self.domain.0
    }

    #[cfg(test)]
    pub(super) fn set_domain_for_test(&mut self, domain: u64) {
        self.domain = PreparationCapacityDomain(domain);
    }

    #[cfg(test)]
    pub(crate) fn set_next_cell_growth_additional_capacity_for_test(
        &mut self,
        additional_capacity: usize,
    ) {
        self.next_cell_growth_additional_capacity = Some(additional_capacity);
    }

    #[cfg(test)]
    pub(crate) fn set_next_new_cell_initial_generation_for_test(&mut self, generation: u64) {
        self.next_new_cell_initial_generation = Some(PreparationReleaseGeneration(generation));
    }

    #[cfg(test)]
    pub(crate) fn drift_next_retire_unbound_generation_for_test(&mut self) {
        self.drift_next_retire_unbound_generation = true;
    }

    #[cfg(test)]
    pub(crate) fn backing_capacities_for_test(&self) -> (usize, usize) {
        (self.cells.capacity(), self.live.capacity())
    }

    #[cfg(test)]
    pub(crate) fn cell_projection_for_test(
        &self,
        identity: (u64, usize, u64, u64),
    ) -> Option<BackCapacityCellProjectionForTest> {
        let (domain, cell, owner, _generation) = identity;
        if domain != self.domain.0 {
            return None;
        }
        let cell = self.cells.get(cell)?;
        let disposition = match (cell.state, cell.reuse) {
            (BackCapacityCellState::Outstanding { owner }, _) => {
                BackCapacityCellDispositionForTest::Outstanding { owner: owner.0 }
            }
            (BackCapacityCellState::Leased { owner }, _) => {
                BackCapacityCellDispositionForTest::Leased { owner: owner.0 }
            }
            (BackCapacityCellState::Free, BackCapacityReuse::Reusable) => {
                BackCapacityCellDispositionForTest::ReusableFree
            }
            (BackCapacityCellState::Free, BackCapacityReuse::PermanentlyRetired) => {
                BackCapacityCellDispositionForTest::PermanentlyRetired
            }
        };
        Some(BackCapacityCellProjectionForTest {
            current_generation: cell.generation.0,
            original_owner_live: self.live.contains_key(&BackCapacityOwnerId(owner)),
            disposition,
        })
    }

    #[cfg(test)]
    pub(crate) fn semantic_snapshot_for_test(&self) -> BackCapacityIssuerSemanticSnapshot {
        #[cfg(test)]
        {
            let _ = self.next_cell_growth_additional_capacity;
            let _ = self.next_new_cell_initial_generation;
            let _ = self.drift_next_retire_unbound_generation;
        }
        BackCapacityIssuerSemanticSnapshot {
            cell_count: self.cells.len(),
            outstanding_count: self.live.len(),
            next_owner: self.next_owner,
            next_generation: self
                .cells
                .iter()
                .find_map(|cell| {
                    (matches!(cell.state, BackCapacityCellState::Free)
                        && cell.reuse == BackCapacityReuse::Reusable)
                        .then_some(cell.generation)
                })
                .unwrap_or(PreparationReleaseGeneration(1)),
        }
    }

    pub(crate) fn reserve(
        &mut self,
    ) -> Result<EcsCleanupCapacityReservation, BackCapacityReserveError> {
        let free_cell = self.cells.iter().position(|cell| {
            matches!(cell.state, BackCapacityCellState::Free)
                && cell.reuse == BackCapacityReuse::Reusable
        });
        let owner = self.next_owner;
        let next_owner = BackCapacityOwnerId(
            owner
                .0
                .checked_add(1)
                .ok_or(BackCapacityReserveError::OwnerExhausted)?,
        );
        if self.live.contains_key(&owner) {
            return Err(BackCapacityReserveError::OwnerExhausted);
        }

        let cell = BackCapacityCellId(free_cell.unwrap_or(self.cells.len()));
        let generation = free_cell
            .map(|index| self.cells[index].generation)
            .unwrap_or_else(|| {
                #[cfg(test)]
                {
                    self.next_new_cell_initial_generation
                        .unwrap_or(PreparationReleaseGeneration(1))
                }
                #[cfg(not(test))]
                {
                    PreparationReleaseGeneration(1)
                }
            });
        let vacant_cell = free_cell.is_none().then(|| BackCapacityCell {
            generation,
            state: BackCapacityCellState::Free,
            reuse: BackCapacityReuse::Reusable,
        });
        let live = BackCapacityLiveRecord {
            domain: self.domain,
            cell,
            owner,
            generation,
            state: BackCapacityLiveState::Outstanding,
        };

        if vacant_cell.is_some() {
            #[cfg(test)]
            let additional_capacity = self
                .next_cell_growth_additional_capacity
                .take()
                .unwrap_or(1);
            #[cfg(not(test))]
            let additional_capacity = 1;
            self.cells
                .try_reserve_exact(additional_capacity)
                .map_err(|_| BackCapacityReserveError::LedgerSlotUnavailable)?;
        }
        self.live
            .try_reserve(1)
            .map_err(|_| BackCapacityReserveError::LedgerSlotUnavailable)?;
        let Some(ledger_domain) =
            allocate_ledger_domain(ledger_domain_counter_input(&NEXT_LEDGER_DOMAIN))
        else {
            return Err(BackCapacityReserveError::DomainExhausted);
        };
        observe_ledger_domain_allocation();
        let vacant_ledger = ManifestNeutralLedgerStorage::new(ledger_domain);

        #[cfg(test)]
        if vacant_cell.is_some() {
            self.next_new_cell_initial_generation = None;
        }
        if let Some(vacant_cell) = vacant_cell {
            self.cells.push(vacant_cell);
        }
        let cell_state = &mut self.cells[cell.0].state;
        debug_assert!(matches!(cell_state, BackCapacityCellState::Free));
        *cell_state = BackCapacityCellState::Outstanding { owner };
        let permit = EcsCleanupCapacityReservation {
            domain: self.domain,
            cell,
            owner,
            generation,
            vacant_ledger,
        };
        let replaced = self.live.insert(owner, live);
        debug_assert!(replaced.is_none());
        debug_assert!(self.live.get(&owner).is_some_and(|record| {
            record.domain == permit.domain
                && record.cell == permit.cell
                && record.owner == permit.owner
                && record.generation == permit.generation
                && record.state == BackCapacityLiveState::Outstanding
                && matches!(
                    self.cells[record.cell.0].state,
                    BackCapacityCellState::Outstanding { owner: cell_owner }
                        if cell_owner == record.owner
                )
                && self.cells[record.cell.0].generation == record.generation
        }));
        self.next_owner = next_owner;
        #[cfg(test)]
        if let Some(producer) = self.back_capacity_order_producer.as_mut() {
            producer.record(bevy_ui_router_core::test_support::BackCapacityIssuerEvent::Issued);
        }
        Ok(permit)
    }

    pub(crate) fn retire_unbound(
        &mut self,
        owner: EcsCleanupCapacityReservation,
        rejection: EnqueueRejected,
    ) -> BackCapacityRetireOutcome<EcsCleanupCapacityReservation> {
        #[cfg(test)]
        if std::mem::take(&mut self.drift_next_retire_unbound_generation) {
            if let Some(cell) = self.cells.get_mut(owner.cell.0) {
                cell.generation = PreparationReleaseGeneration(cell.generation.0.wrapping_add(1));
            }
        }
        if let Some(error) = self.retire_unbound_error(&owner) {
            return BackCapacityRetireOutcome::Rejected {
                owner,
                rejection,
                error,
            };
        }

        let removed = self
            .live
            .remove(&owner.owner)
            .expect("exact preflight retained the outstanding record");
        debug_assert!(
            removed.domain == owner.domain
                && removed.cell == owner.cell
                && removed.owner == owner.owner
                && removed.generation == owner.generation
        );
        let cell = &mut self.cells[owner.cell.0];
        if let Some(next_generation) = owner.generation.0.checked_add(1) {
            cell.generation = PreparationReleaseGeneration(next_generation);
            cell.state = BackCapacityCellState::Free;
            cell.reuse = BackCapacityReuse::Reusable;
        } else {
            cell.state = BackCapacityCellState::Free;
            cell.reuse = BackCapacityReuse::PermanentlyRetired;
        }
        let evidence = BackCapacityRetirementEvidence {
            domain: owner.domain,
            cell: owner.cell,
            owner: owner.owner,
            generation: owner.generation,
            reason: BackCapacityRetirementReason::EnqueueRejected(rejection),
        };
        #[cfg(test)]
        if let Some(producer) = self.back_capacity_order_producer.as_mut() {
            producer.record(bevy_ui_router_core::test_support::BackCapacityIssuerEvent::Retired);
        }
        BackCapacityRetireOutcome::Retired(evidence)
    }

    pub(crate) fn preflight_bound_retirement<'issuer, 'owner>(
        &'issuer mut self,
        bound: &'owner BoundBackCapacityReservation,
        disposition: BackCapacityRetireDisposition,
    ) -> Result<
        (
            ExactBackCapacityCommitGuard<'issuer, 'owner, BoundBackCapacityReservation>,
            PreparedBackCapacityRetirementEvidence,
        ),
        BackCapacityRetireError,
    > {
        let permit = &bound.permit;
        if permit.domain != self.domain {
            return Err(BackCapacityRetireError::ForeignDomain);
        }

        let (live_registry, cells) = (&mut self.live, &mut self.cells);
        let live = match live_registry.entry(permit.owner) {
            Entry::Occupied(live) => live,
            Entry::Vacant(_) => {
                let replayed = cells
                    .get(permit.cell.0)
                    .is_some_and(|cell| retired_identity_is_current(cell, permit.generation));
                return Err(if replayed {
                    BackCapacityRetireError::ConsumedReplay
                } else {
                    BackCapacityRetireError::StaleGeneration
                });
            }
        };
        let record = live.get();
        if record.domain != permit.domain
            || record.cell != permit.cell
            || record.owner != permit.owner
            || record.generation != permit.generation
            || record.state != BackCapacityLiveState::Outstanding
        {
            return Err(BackCapacityRetireError::StaleGeneration);
        }
        let Some(cell) = cells.get_mut(permit.cell.0) else {
            return Err(BackCapacityRetireError::StaleGeneration);
        };
        if cell.generation != permit.generation
            || cell.reuse != BackCapacityReuse::Reusable
            || !matches!(
                cell.state,
                BackCapacityCellState::Outstanding { owner }
                    if owner == permit.owner
            )
        {
            return Err(BackCapacityRetireError::StaleGeneration);
        }

        let next = match permit.generation.0.checked_add(1) {
            Some(next_generation) => PreparedRetiredCell::ReusableFree {
                next_generation: PreparationReleaseGeneration(next_generation),
            },
            None => PreparedRetiredCell::PermanentlyRetired {
                generation: permit.generation,
            },
        };
        let prepared = PreparedBackCapacityRetirementEvidence {
            evidence: BackCapacityRetirementEvidence {
                domain: permit.domain,
                cell: permit.cell,
                owner: permit.owner,
                generation: permit.generation,
                reason: BackCapacityRetirementReason::Bound(disposition),
            },
        };
        Ok((
            ExactBackCapacityCommitGuard {
                transition: BackCapacityCommitTransition::OutstandingRetire { live, cell, next },
                owner: bound,
            },
            prepared,
        ))
    }

    pub(crate) fn preflight_no_target_transfer<'issuer, 'owner>(
        &'issuer mut self,
        bound: &'owner BoundBackCapacityReservation,
    ) -> Result<
        (
            ExactBackCapacityCommitGuard<'issuer, 'owner, BoundBackCapacityReservation>,
            PreparedNoTargetCapacityPermit,
        ),
        BackCapacityRetireError,
    > {
        let capacity = &bound.permit;
        if capacity.domain != self.domain {
            return Err(BackCapacityRetireError::ForeignDomain);
        }

        let (live_registry, cells) = (&mut self.live, &mut self.cells);
        let live = match live_registry.entry(capacity.owner) {
            Entry::Occupied(live) => live,
            Entry::Vacant(_) => {
                let replayed = cells
                    .get(capacity.cell.0)
                    .is_some_and(|cell| retired_identity_is_current(cell, capacity.generation));
                return Err(if replayed {
                    BackCapacityRetireError::ConsumedReplay
                } else {
                    BackCapacityRetireError::StaleGeneration
                });
            }
        };
        let record = live.get();
        if record.domain != capacity.domain
            || record.cell != capacity.cell
            || record.owner != capacity.owner
            || record.generation != capacity.generation
            || record.state != BackCapacityLiveState::Outstanding
        {
            return Err(BackCapacityRetireError::StaleGeneration);
        }
        let Some(cell) = cells.get_mut(capacity.cell.0) else {
            return Err(BackCapacityRetireError::StaleGeneration);
        };
        if cell.generation != capacity.generation
            || cell.reuse != BackCapacityReuse::Reusable
            || !matches!(
                cell.state,
                BackCapacityCellState::Outstanding { owner }
                    if owner == capacity.owner
            )
        {
            return Err(BackCapacityRetireError::StaleGeneration);
        }

        Ok((
            ExactBackCapacityCommitGuard {
                transition: BackCapacityCommitTransition::OutstandingToLeased {
                    live,
                    cell,
                    owner: capacity.owner,
                },
                owner: bound,
            },
            PreparedNoTargetCapacityPermit { private: () },
        ))
    }

    pub(crate) fn validate_no_target_capacity(
        &self,
        permit: &NoTargetCapacityPermit,
    ) -> Result<(), BackCapacityRetireError> {
        let capacity = &permit.capacity;
        if capacity.domain != self.domain {
            return Err(BackCapacityRetireError::ForeignDomain);
        }
        let Some(record) = self.live.get(&capacity.owner) else {
            return Err(BackCapacityRetireError::StaleGeneration);
        };
        if record.domain != capacity.domain
            || record.cell != capacity.cell
            || record.owner != capacity.owner
            || record.generation != capacity.generation
            || record.state != BackCapacityLiveState::Leased
        {
            return Err(BackCapacityRetireError::StaleGeneration);
        }
        let Some(cell) = self.cells.get(capacity.cell.0) else {
            return Err(BackCapacityRetireError::StaleGeneration);
        };
        if cell.generation != capacity.generation
            || cell.reuse != BackCapacityReuse::Reusable
            || !matches!(
                cell.state,
                BackCapacityCellState::Leased { owner }
                    if owner == capacity.owner
            )
        {
            return Err(BackCapacityRetireError::StaleGeneration);
        }
        Ok(())
    }

    pub(crate) fn preflight_back_canonical_zero<'issuer, 'owner>(
        &'issuer mut self,
        permit: &'owner NoTargetCapacityPermit,
        projection: &ProvenEmptyProjection,
    ) -> Result<
        (
            ExactBackCapacityCommitGuard<'issuer, 'owner, NoTargetCapacityPermit>,
            PreparedManifestNeutralLedgerSlot,
        ),
        BackCapacityRetireError,
    > {
        let capacity = &permit.capacity;
        if capacity.domain != self.domain {
            return Err(BackCapacityRetireError::ForeignDomain);
        }
        if !projection.is_canonical_zero() {
            return Err(BackCapacityRetireError::StaleGeneration);
        }

        let (live_registry, cells) = (&mut self.live, &mut self.cells);
        let live = match live_registry.entry(capacity.owner) {
            Entry::Occupied(live) => live,
            Entry::Vacant(_) => {
                let replayed = cells
                    .get(capacity.cell.0)
                    .is_some_and(|cell| retired_identity_is_current(cell, capacity.generation));
                return Err(if replayed {
                    BackCapacityRetireError::ConsumedReplay
                } else {
                    BackCapacityRetireError::StaleGeneration
                });
            }
        };
        let record = live.get();
        if record.domain != capacity.domain
            || record.cell != capacity.cell
            || record.owner != capacity.owner
            || record.generation != capacity.generation
            || record.state != BackCapacityLiveState::Leased
        {
            return Err(BackCapacityRetireError::StaleGeneration);
        }
        let Some(cell) = cells.get_mut(capacity.cell.0) else {
            return Err(BackCapacityRetireError::StaleGeneration);
        };
        if cell.generation != capacity.generation
            || cell.reuse != BackCapacityReuse::Reusable
            || !matches!(
                cell.state,
                BackCapacityCellState::Leased { owner }
                    if owner == capacity.owner
            )
        {
            return Err(BackCapacityRetireError::StaleGeneration);
        }

        Ok((
            ExactBackCapacityCommitGuard {
                transition: BackCapacityCommitTransition::LeasedToCanonicalZeroSlot { live, cell },
                owner: permit,
            },
            PreparedManifestNeutralLedgerSlot { private: () },
        ))
    }

    pub(crate) fn preflight_installed_back_capacity_reclaim<'issuer, 'owner>(
        &'issuer mut self,
        prepared: &'owner PreparedBackNoTargetZeroCompletion,
    ) -> Result<
        ExactBackCapacityCommitGuard<'issuer, 'owner, PreparedBackNoTargetZeroCompletion>,
        BackCapacityRetireError,
    > {
        let Some(capacity) = prepared.ledger.back_no_target_capacity.as_ref() else {
            return Err(BackCapacityRetireError::StaleGeneration);
        };
        if capacity.domain != self.domain {
            return Err(BackCapacityRetireError::ForeignDomain);
        }
        if self.live.contains_key(&capacity.owner)
            || self
                .live
                .values()
                .any(|record| record.cell == capacity.cell)
        {
            return Err(BackCapacityRetireError::StaleGeneration);
        }
        let Some(cell) = self.cells.get_mut(capacity.cell.0) else {
            return Err(BackCapacityRetireError::StaleGeneration);
        };
        if cell.generation != capacity.generation
            || cell.reuse != BackCapacityReuse::Reusable
            || !matches!(
                cell.state,
                BackCapacityCellState::Leased { owner }
                    if owner == capacity.owner
            )
        {
            return Err(if retired_identity_is_current(cell, capacity.generation) {
                BackCapacityRetireError::ConsumedReplay
            } else {
                BackCapacityRetireError::StaleGeneration
            });
        }

        let next = match capacity.generation.0.checked_add(1) {
            Some(next_generation) => PreparedRetiredCell::ReusableFree {
                next_generation: PreparationReleaseGeneration(next_generation),
            },
            None => PreparedRetiredCell::PermanentlyRetired {
                generation: capacity.generation,
            },
        };
        Ok(ExactBackCapacityCommitGuard {
            transition: BackCapacityCommitTransition::InstalledLeaseReclaim { cell, next },
            owner: prepared,
        })
    }

    pub(crate) fn preflight_no_target_retirement<'issuer, 'owner>(
        &'issuer mut self,
        permit: &'owner NoTargetCapacityPermit,
        disposition: BackCapacityRetireDisposition,
    ) -> Result<
        (
            ExactBackCapacityCommitGuard<'issuer, 'owner, NoTargetCapacityPermit>,
            PreparedBackCapacityRetirementEvidence,
        ),
        BackCapacityRetireError,
    > {
        let capacity = &permit.capacity;
        if capacity.domain != self.domain {
            return Err(BackCapacityRetireError::ForeignDomain);
        }

        let (live_registry, cells) = (&mut self.live, &mut self.cells);
        let live = match live_registry.entry(capacity.owner) {
            Entry::Occupied(live) => live,
            Entry::Vacant(_) => {
                let replayed = cells
                    .get(capacity.cell.0)
                    .is_some_and(|cell| retired_identity_is_current(cell, capacity.generation));
                return Err(if replayed {
                    BackCapacityRetireError::ConsumedReplay
                } else {
                    BackCapacityRetireError::StaleGeneration
                });
            }
        };
        let record = live.get();
        if record.domain != capacity.domain
            || record.cell != capacity.cell
            || record.owner != capacity.owner
            || record.generation != capacity.generation
            || record.state != BackCapacityLiveState::Leased
        {
            return Err(BackCapacityRetireError::StaleGeneration);
        }
        let Some(cell) = cells.get_mut(capacity.cell.0) else {
            return Err(BackCapacityRetireError::StaleGeneration);
        };
        if cell.generation != capacity.generation
            || cell.reuse != BackCapacityReuse::Reusable
            || !matches!(
                cell.state,
                BackCapacityCellState::Leased { owner }
                    if owner == capacity.owner
            )
        {
            return Err(BackCapacityRetireError::StaleGeneration);
        }

        let next = match capacity.generation.0.checked_add(1) {
            Some(next_generation) => PreparedRetiredCell::ReusableFree {
                next_generation: PreparationReleaseGeneration(next_generation),
            },
            None => PreparedRetiredCell::PermanentlyRetired {
                generation: capacity.generation,
            },
        };
        let prepared = PreparedBackCapacityRetirementEvidence {
            evidence: BackCapacityRetirementEvidence {
                domain: capacity.domain,
                cell: capacity.cell,
                owner: capacity.owner,
                generation: capacity.generation,
                reason: BackCapacityRetirementReason::Bound(disposition),
            },
        };
        Ok((
            ExactBackCapacityCommitGuard {
                transition: BackCapacityCommitTransition::LeasedRetire { live, cell, next },
                owner: permit,
            },
            prepared,
        ))
    }

    pub(crate) fn classify_retirement_evidence(
        &self,
        evidence: &BackCapacityRetirementEvidence,
    ) -> Result<(), BackCapacityRetireError> {
        if evidence.domain != self.domain {
            return Err(BackCapacityRetireError::ForeignDomain);
        }
        let _ = &evidence.reason;
        if self.live.contains_key(&evidence.owner) {
            return Err(BackCapacityRetireError::StaleGeneration);
        }
        let Some(cell) = self.cells.get(evidence.cell.0) else {
            return Err(BackCapacityRetireError::StaleGeneration);
        };
        if retired_identity_is_current(cell, evidence.generation) {
            Err(BackCapacityRetireError::ConsumedReplay)
        } else {
            Err(BackCapacityRetireError::StaleGeneration)
        }
    }

    fn retire_unbound_error(
        &self,
        owner: &EcsCleanupCapacityReservation,
    ) -> Option<BackCapacityRetireError> {
        if owner.domain != self.domain {
            return Some(BackCapacityRetireError::ForeignDomain);
        }
        let Some(record) = self.live.get(&owner.owner) else {
            let replayed = self
                .cells
                .get(owner.cell.0)
                .is_some_and(|cell| retired_identity_is_current(cell, owner.generation));
            return Some(if replayed {
                BackCapacityRetireError::ConsumedReplay
            } else {
                BackCapacityRetireError::StaleGeneration
            });
        };
        if record.domain != owner.domain
            || record.cell != owner.cell
            || record.owner != owner.owner
            || record.generation != owner.generation
            || record.state != BackCapacityLiveState::Outstanding
        {
            return Some(BackCapacityRetireError::StaleGeneration);
        }
        let Some(cell) = self.cells.get(owner.cell.0) else {
            return Some(BackCapacityRetireError::StaleGeneration);
        };
        if cell.generation != owner.generation
            || cell.reuse != BackCapacityReuse::Reusable
            || !matches!(
                cell.state,
                BackCapacityCellState::Outstanding { owner: cell_owner }
                    if cell_owner == owner.owner
            )
        {
            return Some(BackCapacityRetireError::StaleGeneration);
        }
        None
    }
}

fn retired_identity_is_current(
    cell: &BackCapacityCell,
    retired_generation: PreparationReleaseGeneration,
) -> bool {
    match cell.state {
        BackCapacityCellState::Free => match cell.reuse {
            BackCapacityReuse::Reusable => retired_generation
                .0
                .checked_add(1)
                .is_some_and(|successor| cell.generation.0 == successor),
            BackCapacityReuse::PermanentlyRetired => {
                retired_generation.0 == u64::MAX && cell.generation == retired_generation
            }
        },
        BackCapacityCellState::Outstanding { .. } | BackCapacityCellState::Leased { .. } => false,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BackCapacityReserveError {
    DomainExhausted,
    OwnerExhausted,
    LedgerSlotUnavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BackCapacityIssuerInstallError {
    AlreadyInstalled,
    Reserve(BackCapacityReserveError),
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum EnqueueRejected {
    CoreTokenDrift,
    CoreAllocatorFailure(EnqueueBackErrorKind),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BackCapacityRetireDisposition {
    Target,
    Contradiction,
    AuthorityLost,
    Permanent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BackCapacityRetireError {
    ForeignDomain,
    StaleGeneration,
    ConsumedReplay,
}

enum BackCapacityRetirementReason {
    EnqueueRejected(EnqueueRejected),
    Bound(BackCapacityRetireDisposition),
}

pub(crate) struct BackCapacityRetirementEvidence {
    domain: PreparationCapacityDomain,
    cell: BackCapacityCellId,
    owner: BackCapacityOwnerId,
    generation: PreparationReleaseGeneration,
    reason: BackCapacityRetirementReason,
}

enum PreparedRetiredCell {
    ReusableFree {
        next_generation: PreparationReleaseGeneration,
    },
    PermanentlyRetired {
        generation: PreparationReleaseGeneration,
    },
}

pub(crate) struct PreparedBackCapacityRetirementEvidence {
    evidence: BackCapacityRetirementEvidence,
}

pub(crate) struct PreparedManifestNeutralLedgerSlot {
    private: (),
}

pub(crate) struct ManifestNeutralLedgerSlot {
    capacity: EcsCleanupCapacityReservation,
}

pub(super) struct ManifestNeutralLedgerStorage {
    pub(super) obligations: Vec<CleanupObligation>,
    pub(super) mount_parents: Vec<CleanupMountParentSettlement>,
    pub(super) receipts: Vec<Option<CleanupReceipt>>,
    pub(super) ledger_domain: u64,
}

impl ManifestNeutralLedgerStorage {
    fn new(ledger_domain: u64) -> Self {
        Self {
            obligations: Vec::new(),
            mount_parents: Vec::new(),
            receipts: Vec::new(),
            ledger_domain,
        }
    }
}

enum BackCapacityCommitTransition<'issuer> {
    OutstandingRetire {
        live: OccupiedEntry<'issuer, BackCapacityOwnerId, BackCapacityLiveRecord>,
        cell: &'issuer mut BackCapacityCell,
        next: PreparedRetiredCell,
    },
    OutstandingToLeased {
        live: OccupiedEntry<'issuer, BackCapacityOwnerId, BackCapacityLiveRecord>,
        cell: &'issuer mut BackCapacityCell,
        owner: BackCapacityOwnerId,
    },
    LeasedRetire {
        live: OccupiedEntry<'issuer, BackCapacityOwnerId, BackCapacityLiveRecord>,
        cell: &'issuer mut BackCapacityCell,
        next: PreparedRetiredCell,
    },
    LeasedToCanonicalZeroSlot {
        live: OccupiedEntry<'issuer, BackCapacityOwnerId, BackCapacityLiveRecord>,
        cell: &'issuer mut BackCapacityCell,
    },
    InstalledLeaseReclaim {
        cell: &'issuer mut BackCapacityCell,
        next: PreparedRetiredCell,
    },
}

pub(crate) struct ExactBackCapacityCommitGuard<'issuer, 'owner, Owner: ?Sized> {
    transition: BackCapacityCommitTransition<'issuer>,
    owner: &'owner Owner,
}

impl<Owner: ?Sized> ExactBackCapacityCommitGuard<'_, '_, Owner> {
    pub(crate) fn commit(self) {
        match self.transition {
            BackCapacityCommitTransition::OutstandingRetire { live, cell, next } => {
                let _ = live.remove();
                cell.state = BackCapacityCellState::Free;
                match next {
                    PreparedRetiredCell::ReusableFree { next_generation } => {
                        cell.generation = next_generation;
                        cell.reuse = BackCapacityReuse::Reusable;
                    }
                    PreparedRetiredCell::PermanentlyRetired { generation } => {
                        cell.generation = generation;
                        cell.reuse = BackCapacityReuse::PermanentlyRetired;
                    }
                }
            }
            BackCapacityCommitTransition::OutstandingToLeased { live, cell, owner } => {
                live.into_mut().state = BackCapacityLiveState::Leased;
                cell.state = BackCapacityCellState::Leased { owner };
            }
            BackCapacityCommitTransition::LeasedRetire { live, cell, next } => {
                let _ = live.remove();
                cell.state = BackCapacityCellState::Free;
                match next {
                    PreparedRetiredCell::ReusableFree { next_generation } => {
                        cell.generation = next_generation;
                        cell.reuse = BackCapacityReuse::Reusable;
                    }
                    PreparedRetiredCell::PermanentlyRetired { generation } => {
                        cell.generation = generation;
                        cell.reuse = BackCapacityReuse::PermanentlyRetired;
                    }
                }
            }
            BackCapacityCommitTransition::LeasedToCanonicalZeroSlot { live, cell } => {
                let _ = live.remove();
                let _ = cell;
            }
            BackCapacityCommitTransition::InstalledLeaseReclaim { cell, next } => {
                cell.state = BackCapacityCellState::Free;
                match next {
                    PreparedRetiredCell::ReusableFree { next_generation } => {
                        cell.generation = next_generation;
                        cell.reuse = BackCapacityReuse::Reusable;
                    }
                    PreparedRetiredCell::PermanentlyRetired { generation } => {
                        cell.generation = generation;
                        cell.reuse = BackCapacityReuse::PermanentlyRetired;
                    }
                }
            }
        }
    }
}

impl PreparedBackCapacityRetirementEvidence {
    pub(crate) fn into_evidence_after_exact_receiver(self) -> BackCapacityRetirementEvidence {
        self.evidence
    }
}

impl PreparedManifestNeutralLedgerSlot {
    pub(crate) fn into_slot_after_exact_receiver(
        self,
        stale_permit: NoTargetCapacityPermit,
    ) -> ManifestNeutralLedgerSlot {
        let Self { private: () } = self;
        let NoTargetCapacityPermit { capacity } = stale_permit;
        ManifestNeutralLedgerSlot { capacity }
    }
}

impl ManifestNeutralLedgerSlot {
    pub(crate) fn bind_back_no_target(self, batch: BackNoTargetCleanupBatchView) -> CleanupLedger {
        let Self { capacity } = self;
        let EcsCleanupCapacityReservation {
            domain,
            cell,
            owner,
            generation,
            vacant_ledger,
        } = capacity;
        CleanupLedger::activate_back_no_target(
            vacant_ledger,
            batch,
            InstalledBackCapacityLease {
                domain,
                cell,
                owner,
                generation,
            },
        )
    }

    #[cfg(test)]
    pub(crate) const fn capacity_identity_for_test(&self) -> (u64, usize, u64, u64) {
        self.capacity.ordinal_identity_for_test()
    }
}

#[cfg(test)]
impl BackCapacityRetirementEvidence {
    pub(crate) fn enqueue_rejection_for_test(&self) -> Option<&EnqueueRejected> {
        match &self.reason {
            BackCapacityRetirementReason::EnqueueRejected(rejection) => Some(rejection),
            BackCapacityRetirementReason::Bound(_) => None,
        }
    }

    pub(crate) fn bound_disposition_for_test(&self) -> Option<BackCapacityRetireDisposition> {
        match &self.reason {
            BackCapacityRetirementReason::EnqueueRejected(_) => None,
            BackCapacityRetirementReason::Bound(disposition) => Some(*disposition),
        }
    }
}

pub(crate) enum BackCapacityRetireOutcome<Owner> {
    Retired(BackCapacityRetirementEvidence),
    Rejected {
        owner: Owner,
        rejection: EnqueueRejected,
        error: BackCapacityRetireError,
    },
}

pub(crate) struct EcsCleanupCapacityReservation {
    pub(super) domain: PreparationCapacityDomain,
    pub(super) cell: BackCapacityCellId,
    pub(super) owner: BackCapacityOwnerId,
    pub(super) generation: PreparationReleaseGeneration,
    vacant_ledger: ManifestNeutralLedgerStorage,
}

pub(crate) struct BackCapacityEnqueueRetireResidual {
    pub(crate) owner: EcsCleanupCapacityReservation,
    pub(crate) rejection: EnqueueRejected,
    pub(crate) retire_error: BackCapacityRetireError,
}

impl EcsCleanupCapacityReservation {
    #[cfg(test)]
    pub(crate) const fn ordinal_identity_for_test(&self) -> (u64, usize, u64, u64) {
        (self.domain.0, self.cell.0, self.owner.0, self.generation.0)
    }
}

pub(crate) struct BoundBackCapacityReservation {
    request: NavigationRequestId,
    permit: EcsCleanupCapacityReservation,
    accepted: BackAcceptedReservationView,
}

impl BoundBackCapacityReservation {
    pub(crate) fn bind(
        request: NavigationRequestId,
        permit: EcsCleanupCapacityReservation,
        accepted: BackAcceptedReservationView,
    ) -> Self {
        Self {
            request,
            permit,
            accepted,
        }
    }

    pub(crate) fn matches_resolution_scope(&self, scope: &BackResolutionScopeView) -> bool {
        self.accepted.matches_resolution_scope(scope)
    }

    pub(crate) const fn request(&self) -> NavigationRequestId {
        self.request
    }

    #[cfg(test)]
    pub(crate) const fn capacity_identity_for_test(&self) -> (u64, usize, u64, u64) {
        self.permit.ordinal_identity_for_test()
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        NavigationRequestId,
        EcsCleanupCapacityReservation,
        BackAcceptedReservationView,
    ) {
        (self.request, self.permit, self.accepted)
    }
}

pub(crate) struct PreparedNoTargetCapacityPermit {
    private: (),
}

pub(crate) struct NoTargetCapacityPermit {
    capacity: EcsCleanupCapacityReservation,
}

impl PreparedNoTargetCapacityPermit {
    pub(crate) fn into_permit_after_exact_receiver(
        self,
        stale_bound: BoundBackCapacityReservation,
    ) -> NoTargetCapacityPermit {
        let Self { private: () } = self;
        let (_request, capacity, _accepted) = stale_bound.into_parts();
        NoTargetCapacityPermit { capacity }
    }
}

#[cfg(test)]
impl NoTargetCapacityPermit {
    pub(crate) const fn capacity_identity_for_test(&self) -> (u64, usize, u64, u64) {
        self.capacity.ordinal_identity_for_test()
    }
}

#[cfg(test)]
pub(super) fn prepared_installed_back_zero_completion(
    initial_generation: u64,
) -> (
    BackCapacityIssuer,
    PreparedBackNoTargetZeroCompletion,
    (u64, usize, u64, u64),
) {
    let mut domains = BackCapacityDomainIssuer::new();
    let mut issuer = domains.issue().unwrap();
    issuer.set_next_new_cell_initial_generation_for_test(initial_generation);
    let capacity = issuer.reserve().unwrap();
    let identity = capacity.ordinal_identity_for_test();
    let EcsCleanupCapacityReservation {
        domain,
        cell,
        owner,
        generation,
        vacant_ledger,
    } = capacity;
    issuer
        .live
        .get_mut(&owner)
        .expect("the reserved owner remains live")
        .state = BackCapacityLiveState::Leased;
    issuer.cells[cell.0].state = BackCapacityCellState::Leased { owner };
    let removed = issuer
        .live
        .remove(&owner)
        .expect("canonical-zero installation consumes the live entry");
    assert_eq!(removed.domain, domain);
    assert_eq!(removed.cell, cell);
    assert_eq!(removed.owner, owner);
    assert_eq!(removed.generation, generation);

    let ManifestNeutralLedgerStorage {
        obligations,
        mount_parents,
        receipts,
        ledger_domain,
    } = vacant_ledger;
    let ledger = CleanupLedger {
        batch: None,
        back_no_target_batch: None,
        back_no_target_capacity: Some(InstalledBackCapacityLease {
            domain,
            cell,
            owner,
            generation,
        }),
        owner: None,
        obligations,
        mount_parents,
        manifest_digest: canonical_zero_manifest_digest(),
        receipts,
        ledger_domain,
        next_attempt: 1,
        issued_attempt: None,
        completion_digest: Some(canonical_empty_completion_digest()),
        quarantine_event_observed: false,
        related_protected: None,
        post_settlement_focus: Vec::new(),
        authority_host_children: None,
        back_no_target_authority_partition: None,
        failure_cleanup_partition: None,
        stopped: None,
        quarantine_detachment: None,
        cleanup_settlement_permanent_pending: None,
    };
    let source = ledger.back_no_target_zero_completion_source_identity();
    let prepared = PreparedBackNoTargetZeroCompletion {
        ledger,
        facts: BackNoTargetZeroCompletionFacts {
            manifest_digest: canonical_zero_manifest_digest(),
            obligation_count: 0,
            completion_digest: canonical_empty_completion_digest(),
            settled_count: 0,
        },
        source,
    };
    (issuer, prepared, identity)
}
