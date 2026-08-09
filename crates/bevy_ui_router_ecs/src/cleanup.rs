use std::{
    collections::{
        HashMap, HashSet,
        hash_map::{Entry, OccupiedEntry},
    },
    sync::atomic::{AtomicU64, Ordering},
};

use bevy_ecs::{
    change_detection::Mut,
    component::ComponentId,
    hierarchy::{ChildOf, Children},
    prelude::{Entity, World},
};
use bevy_ui_router_core::{
    frame::{FrameOrigin, RouteFrameVersion},
    ids::{NavigationRequestId, RouteSlotId},
    integration::{
        BackAcceptedReservationView, BackNoTargetCleanupBatchView, BackResolutionScopeView,
        CleanupBatch, CleanupCompletion, EnqueueBackErrorKind,
    },
    slot::RouteSlotKey,
};

use crate::{
    branch_transaction::{FailedPreparedBranch, RealizedBranch, RejectedChildSlot},
    components::{
        ChildSlotAuthority, ChildSlotLifecycle, ProvisionalSlotLifecycle, RouteOwnershipSeal,
        RouterInputBarrier,
    },
    input::{RouteFocusState, observed_descendant_closure, prepare_focus_mutation},
    mount::{ChildSlotResidual, MountedChildSlot, residual_child_slot_is_isolated},
    navigation::{HostBinding, PendingFailedBranch, ProvenEmptyProjection, RetiredHostBinding},
    presentation::{FrameContentTarget, FrameLifecycle, FrameOutput, RealizedFrame},
    registry::{
        ComponentManifest, EntityLifecycle, EntityOwner, ExactRegistryClaim,
        PreparedAuthorityLostResidualTerminalProjection, PreparedAuthorityLostResidualTransfer,
        PreparedAuthorityPublicationCapacity, PreparedExactRegistryRetirement,
        PreparedQuarantineTransfer, ProvisionalOwner, QuarantineFacts, QuarantineLeaseId,
        QuarantineLeaseTable, RegistryOwner, RouteEntityRegistry,
        apply_prepared_authority_lost_residual_transfer, apply_prepared_exact_registry_retirement,
        apply_prepared_quarantine_transfer, apply_prepared_unresolved_residual_transfer,
        prepare_authority_lost_residual_transfer,
        prepare_authority_lost_residual_transfer_in_publication, prepare_exact_registry_retirement,
        prepare_unresolved_residual_transfer,
        prepare_verified_authority_cleanup_quarantine_transfer,
        prepare_verified_cleanup_quarantine_transfer,
    },
};
use bevy_camera::visibility::Visibility;
use bevy_input_focus::InputFocus;
use bevy_picking::Pickable;
use bevy_ui::{Node, Val};

const MANIFEST_DOMAIN: u64 = 0x4543_534d_414e_4946;
const ATTEMPT_DOMAIN: u64 = 0x4543_5341_5454_454d;
const RECEIPT_DOMAIN: u64 = 0x4543_5352_4543_5054;
const OBLIGATION_DOMAIN: u64 = 0x4543_534f_424c_4947;
const COMPLETION_DOMAIN: u64 = 0x4543_5343_4f4d_504c;
static NEXT_LEDGER_DOMAIN: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PreparationCapacityDomain(u64);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct PreparationReleaseGeneration(u64);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct BackCapacityOwnerId(u64);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct BackCapacityCellId(usize);

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
            #[cfg(feature = "test-support")]
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
    #[cfg(feature = "test-support")]
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
    #[cfg(feature = "test-support")]
    pub(crate) fn back_capacity_order_producer_slot_is_vacant(&self) -> bool {
        self.back_capacity_order_producer.is_none()
    }

    #[cfg(feature = "test-support")]
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
        let _ = self.next_cell_growth_additional_capacity;
        let _ = self.next_new_cell_initial_generation;
        let _ = self.drift_next_retire_unbound_generation;
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
        #[cfg(feature = "test-support")]
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
        #[cfg(feature = "test-support")]
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

struct ManifestNeutralLedgerStorage {
    obligations: Vec<CleanupObligation>,
    mount_parents: Vec<CleanupMountParentSettlement>,
    receipts: Vec<Option<CleanupReceipt>>,
    ledger_domain: u64,
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
    domain: PreparationCapacityDomain,
    cell: BackCapacityCellId,
    owner: BackCapacityOwnerId,
    generation: PreparationReleaseGeneration,
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
thread_local! {
    static FAIL_NEXT_RESERVATION: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FAIL_NEXT_LEDGER_DOMAIN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FAIL_NEXT_UNFINALIZED_LEDGER: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static LEDGER_DOMAIN_ALLOCATIONS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
static EXHAUSTED_LEDGER_DOMAIN_COUNTER: AtomicU64 = AtomicU64::new(u64::MAX);

#[cfg(test)]
pub(crate) fn fail_next_reservation_for_test() {
    FAIL_NEXT_RESERVATION.set(true);
}

#[cfg(test)]
pub(crate) fn fail_next_ledger_domain_for_test() {
    FAIL_NEXT_LEDGER_DOMAIN.set(true);
}

#[cfg(test)]
pub(crate) fn fail_next_unfinalized_ledger_for_test() {
    FAIL_NEXT_UNFINALIZED_LEDGER.set(true);
}

#[cfg(test)]
pub(crate) fn reset_ledger_domain_allocations_for_test() {
    LEDGER_DOMAIN_ALLOCATIONS.set(0);
}

#[cfg(test)]
pub(crate) fn ledger_domain_allocations_for_test() -> u64 {
    LEDGER_DOMAIN_ALLOCATIONS.get()
}

#[cfg(test)]
fn reservation_fault_injected() -> bool {
    FAIL_NEXT_RESERVATION.replace(false)
}

#[cfg(test)]
fn ledger_domain_fault_injected() -> bool {
    FAIL_NEXT_LEDGER_DOMAIN.replace(false)
}

#[cfg(test)]
fn ledger_domain_counter_input(counter: &AtomicU64) -> &AtomicU64 {
    if ledger_domain_fault_injected() {
        &EXHAUSTED_LEDGER_DOMAIN_COUNTER
    } else {
        counter
    }
}

#[cfg(test)]
fn inject_unfinalized_boundary_drift(manifest: &mut FrozenCleanupManifest) {
    if !FAIL_NEXT_UNFINALIZED_LEDGER.replace(false) {
        return;
    }
    let Some((frame, claim, mount_slot)) = manifest.obligations.iter().find_map(|obligation| {
        let RegistryOwner::Committed(EntityOwner::FrameShell(frame)) = obligation.claim.owner()
        else {
            return None;
        };
        Some((
            frame.clone(),
            obligation.claim.clone(),
            obligation.expected_parent,
        ))
    }) else {
        return;
    };
    manifest.boundary_drifts.push(FailedBoundaryDrift {
        source: FailedBoundarySource::FramePhysicalParent {
            frame,
            mount_slot,
            removed_child: claim.entity(),
        },
        expected: FailedBoundaryExpectation::ExactClaim(claim),
        state: FailedManifestClaimState::PhysicalDrift,
        predicate: FailedMountParentFreezePredicate::PhysicalAuthority,
    });
}

#[cfg(test)]
fn observe_ledger_domain_allocation() {
    LEDGER_DOMAIN_ALLOCATIONS.set(LEDGER_DOMAIN_ALLOCATIONS.get() + 1);
}

#[cfg(not(test))]
const fn reservation_fault_injected() -> bool {
    false
}

#[cfg(not(test))]
const fn ledger_domain_counter_input(counter: &AtomicU64) -> &AtomicU64 {
    counter
}

#[cfg(not(test))]
fn inject_unfinalized_boundary_drift(_manifest: &mut FrozenCleanupManifest) {}

#[cfg(not(test))]
const fn observe_ledger_domain_allocation() {}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CleanupPostcheckPhase {
    VerifiedTransfer,
}

#[cfg(test)]
#[derive(bevy_ecs::prelude::Resource)]
pub(crate) struct CleanupPostcheckDrift {
    phase: CleanupPostcheckPhase,
    external_parent: Option<Entity>,
    focus: Option<Entity>,
    fired: bool,
}

#[cfg(test)]
impl CleanupPostcheckDrift {
    pub(crate) const fn external_parent(phase: CleanupPostcheckPhase, parent: Entity) -> Self {
        Self {
            phase,
            external_parent: Some(parent),
            focus: None,
            fired: false,
        }
    }
}

#[cfg(test)]
fn apply_cleanup_postcheck_drift(world: &mut World, phase: CleanupPostcheckPhase) {
    let Some((external_parent, focus)) = world
        .get_resource_mut::<CleanupPostcheckDrift>()
        .and_then(|mut drift| {
            if drift.fired || drift.phase != phase {
                return None;
            }
            drift.fired = true;
            Some((drift.external_parent, drift.focus))
        })
    else {
        return;
    };
    if let Some(parent) = external_parent {
        let seal = world.resource_mut::<RouteEntityRegistry>().issue_seal();
        world
            .commands()
            .entity(parent)
            .insert(RouteOwnershipSeal::new(seal));
        world.flush();
    }
    if let Some(focus) = focus {
        *world.resource_mut::<InputFocus>() = InputFocus::from_entity(focus);
        world
            .resource_mut::<RouteFocusState>()
            .set_focused(Some(focus));
    }
}

#[cfg(test)]
#[derive(bevy_ecs::prelude::Resource)]
pub(crate) struct AuthorityHostEdgePostwriteDrift {
    top_slot: Entity,
    host: Entity,
    fired: bool,
}

#[cfg(test)]
impl AuthorityHostEdgePostwriteDrift {
    pub(crate) const fn reattach(top_slot: Entity, host: Entity) -> Self {
        Self {
            top_slot,
            host,
            fired: false,
        }
    }
}

#[cfg(test)]
fn apply_authority_host_edge_postwrite_drift(world: &mut World, top_slot: Entity, host: Entity) {
    let should_fire = world
        .get_resource_mut::<AuthorityHostEdgePostwriteDrift>()
        .is_some_and(|mut drift| {
            if drift.fired || drift.top_slot != top_slot || drift.host != host {
                return false;
            }
            drift.fired = true;
            true
        });
    if should_fire {
        world.entity_mut(top_slot).insert(ChildOf(host));
        world.flush();
    }
}

#[cfg(not(test))]
fn apply_authority_host_edge_postwrite_drift(_world: &mut World, _top_slot: Entity, _host: Entity) {
}

#[cfg(test)]
#[derive(bevy_ecs::prelude::Resource)]
pub(crate) struct AuthorityRemovalPostwriteDrift {
    child: Entity,
    next_parent: Entity,
    fired: bool,
}

#[cfg(test)]
#[derive(bevy_ecs::prelude::Resource)]
pub(crate) struct AuthorityRemovalRegistryPostwriteDrift {
    entity: Entity,
    fired: bool,
}

#[cfg(test)]
impl AuthorityRemovalRegistryPostwriteDrift {
    pub(crate) const fn replace_after_first_removal(entity: Entity) -> Self {
        Self {
            entity,
            fired: false,
        }
    }
}

#[cfg(test)]
impl AuthorityRemovalPostwriteDrift {
    pub(crate) const fn reparent_after_first_removal(child: Entity, next_parent: Entity) -> Self {
        Self {
            child,
            next_parent,
            fired: false,
        }
    }
}

#[cfg(test)]
fn apply_authority_removal_postwrite_drift(world: &mut World) {
    let reparent = world
        .get_resource_mut::<AuthorityRemovalPostwriteDrift>()
        .and_then(|mut drift| {
            if drift.fired {
                return None;
            }
            drift.fired = true;
            Some((drift.child, drift.next_parent))
        });
    if let Some((child, next_parent)) = reparent {
        world.entity_mut(next_parent).add_child(child);
        world.flush();
    }
    let replacement_entity = world
        .get_resource_mut::<AuthorityRemovalRegistryPostwriteDrift>()
        .and_then(|mut drift| {
            if drift.fired {
                return None;
            }
            drift.fired = true;
            Some(drift.entity)
        });
    if let Some(entity) = replacement_entity {
        let (claim, owner, manifest, lifecycle) = {
            let registry = world.resource::<RouteEntityRegistry>();
            let record = registry
                .record(entity)
                .expect("registry-only drift target must retain its old exact claim");
            (
                record.claim(),
                record.owner().clone(),
                record.manifest().clone(),
                record.lifecycle(),
            )
        };
        let mut registry = world.resource_mut::<RouteEntityRegistry>();
        assert!(registry.remove_exact(&claim));
        let seal = registry.issue_seal();
        registry
            .register(entity, owner, seal, manifest, lifecycle)
            .expect("registry-only drift must install a fresh exact claim");
    }
}

#[cfg(not(test))]
fn apply_authority_removal_postwrite_drift(_world: &mut World) {}

#[cfg(test)]
#[derive(bevy_ecs::prelude::Resource)]
pub(crate) struct AuthorityResidualPostwriteDrift {
    entities: [Entity; 3],
    fired: bool,
}

#[cfg(test)]
#[derive(bevy_ecs::prelude::Resource)]
pub(crate) struct AuthorityLostResidualPostwriteDrift {
    entities: [Entity; 3],
    next_parent: Entity,
    fired: bool,
}

#[cfg(test)]
#[derive(bevy_ecs::prelude::Resource)]
pub(crate) struct AuthorityLostResidualPhysicalRestoreDrift {
    claim: ExactRegistryClaim,
    fired: bool,
}

#[cfg(test)]
impl AuthorityLostResidualPhysicalRestoreDrift {
    pub(crate) const fn restore_after_first_closure(claim: ExactRegistryClaim) -> Self {
        Self {
            claim,
            fired: false,
        }
    }
}

#[cfg(test)]
impl AuthorityLostResidualPostwriteDrift {
    pub(crate) const fn reparent_after_first_closure(
        entities: [Entity; 3],
        next_parent: Entity,
    ) -> Self {
        Self {
            entities,
            next_parent,
            fired: false,
        }
    }
}

#[cfg(test)]
impl AuthorityResidualPostwriteDrift {
    pub(crate) const fn invalidate_after_first_closure(entities: [Entity; 3]) -> Self {
        Self {
            entities,
            fired: false,
        }
    }
}

#[cfg(test)]
fn apply_authority_residual_postwrite_drift(world: &mut World) {
    let entities = world
        .get_resource_mut::<AuthorityResidualPostwriteDrift>()
        .and_then(|mut drift| {
            if drift.fired {
                return None;
            }
            drift.fired = true;
            Some(drift.entities)
        });
    if let Some(entities) = entities {
        for entity in entities {
            let stale_seal = world.resource_mut::<RouteEntityRegistry>().issue_seal();
            world
                .entity_mut(entity)
                .insert(RouteOwnershipSeal::new(stale_seal));
        }
        world.flush();
    }
    let reparent = world
        .get_resource_mut::<AuthorityLostResidualPostwriteDrift>()
        .and_then(|mut drift| {
            if drift.fired {
                return None;
            }
            drift.fired = true;
            Some((drift.entities, drift.next_parent))
        });
    if let Some((entities, next_parent)) = reparent {
        for (index, entity) in entities.into_iter().enumerate() {
            if entities[..index].contains(&entity) {
                continue;
            }
            world.entity_mut(next_parent).add_child(entity);
        }
        world.flush();
    }
    let restored_claim = world
        .get_resource_mut::<AuthorityLostResidualPhysicalRestoreDrift>()
        .and_then(|mut drift| {
            if drift.fired {
                return None;
            }
            drift.fired = true;
            Some(drift.claim.clone())
        });
    if let Some(claim) = restored_claim {
        assert!(matches!(
            claim.owner(),
            RegistryOwner::Committed(
                EntityOwner::FrameShell(_)
                    | EntityOwner::FrameContentTarget(_)
                    | EntityOwner::FrameOutput { .. }
            )
        ));
        world.entity_mut(claim.entity()).insert((
            RouteOwnershipSeal::new(claim.seal()),
            FrameLifecycle(claim.lifecycle()),
        ));
        world.flush();
    }
}

#[cfg(not(test))]
fn apply_authority_residual_postwrite_drift(_world: &mut World) {}

fn allocate_ledger_domain(counter: &AtomicU64) -> Option<u64> {
    counter
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1).filter(|_| current != 0)
        })
        .ok()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CleanupDisposition {
    Removed,
    RemovedDetached,
    AlreadyAbsent,
    RejectedProtected,
    QuarantinedProtectedParent,
    QuarantinedForeignDescendant,
    QuarantinedForeignComponent,
}

impl CleanupDisposition {
    const fn tag(self) -> u64 {
        match self {
            Self::Removed => 1,
            Self::RemovedDetached => 2,
            Self::AlreadyAbsent => 3,
            Self::RejectedProtected => 4,
            Self::QuarantinedForeignDescendant => 5,
            Self::QuarantinedProtectedParent => 6,
            Self::QuarantinedForeignComponent => 7,
        }
    }

    pub(crate) const fn public(self) -> bevy_ui_router_core::prelude::CleanupDisposition {
        use bevy_ui_router_core::prelude::CleanupDisposition as Public;
        match self {
            Self::Removed => Public::Removed,
            Self::RemovedDetached => Public::RemovedDetached,
            Self::AlreadyAbsent => Public::AlreadyAbsent,
            Self::RejectedProtected => Public::RejectedProtected,
            Self::QuarantinedProtectedParent => Public::QuarantinedProtectedParent,
            Self::QuarantinedForeignDescendant => Public::QuarantinedForeignDescendant,
            Self::QuarantinedForeignComponent => Public::QuarantinedForeignComponent,
        }
    }

    const fn is_quarantine(self) -> bool {
        matches!(
            self,
            Self::QuarantinedProtectedParent
                | Self::QuarantinedForeignDescendant
                | Self::QuarantinedForeignComponent
        )
    }
}

#[derive(Default)]
pub(crate) struct RouterCleanupObservation {
    quarantine_count: usize,
    last_disposition: Option<bevy_ui_router_core::prelude::CleanupDisposition>,
}

impl RouterCleanupObservation {
    pub(crate) fn record(&mut self, disposition: CleanupDisposition, terminal_quarantine: bool) {
        self.last_disposition = Some(disposition.public());
        if terminal_quarantine {
            self.quarantine_count = self.quarantine_count.saturating_add(1);
        }
    }

    pub(crate) fn summary(&self, foreground_pending: bool) -> bevy_ui_router_core::CleanupSummary {
        bevy_ui_router_core::integration::project_cleanup_summary(
            foreground_pending,
            self.quarantine_count,
            self.last_disposition,
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CleanupObligation {
    claim: ExactRegistryClaim,
    manifest: ComponentManifest,
    expected_parent: Entity,
    unit_root: Entity,
    authorized_parent: Entity,
    allow_missing_components: bool,
    terminal_mode: CleanupTerminalMode,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CleanupTerminalMode {
    Normal,
    ForceUnresolved,
    AuthorityExact,
    AuthorityLost,
}

impl CleanupTerminalMode {
    const fn digest_tag(self) -> u64 {
        match self {
            Self::Normal => 0,
            Self::ForceUnresolved => 1,
            Self::AuthorityExact => 2,
            Self::AuthorityLost => 3,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CleanupMountParentSettlement {
    claim: ExactRegistryClaim,
    manifest: ComponentManifest,
    physical_manifest: ComponentManifest,
    expected_parent: Entity,
    expected_parent_claim: Option<ExactRegistryClaim>,
    removed_child: Entity,
    removed_child_was_attached: bool,
    children_before: Vec<Entity>,
    children_after: Vec<Entity>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CleanupMountParentCandidate {
    claim: ExactRegistryClaim,
    failed_claim_source: Option<FailedBoundarySource>,
    supplied_parent: Option<(Entity, Option<ExactRegistryClaim>)>,
    failed_parent_source: Option<FailedBoundarySource>,
    removed_child: Entity,
    removed_child_was_attached: bool,
    children_before: Vec<Entity>,
    children_after: Vec<Entity>,
}

pub(crate) struct VerifiedCleanupQuarantineTransfer {
    claims: Vec<ExactRegistryClaim>,
    manifests: Vec<(Entity, ComponentManifest)>,
    facts: QuarantineFacts,
}

impl VerifiedCleanupQuarantineTransfer {
    fn after_world_validation(
        claims: Vec<ExactRegistryClaim>,
        manifests: Vec<(Entity, ComponentManifest)>,
        facts: QuarantineFacts,
    ) -> Self {
        Self {
            claims,
            manifests,
            facts,
        }
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        Vec<ExactRegistryClaim>,
        Vec<(Entity, ComponentManifest)>,
        QuarantineFacts,
    ) {
        (self.claims, self.manifests, self.facts)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FrozenCleanupManifest {
    obligations: Vec<CleanupObligation>,
    mount_parents: Vec<CleanupMountParentSettlement>,
    boundary_drifts: Vec<FailedBoundaryDrift>,
    pending_boundary_loss_claims: Vec<ExactRegistryClaim>,
    digest: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FrozenBackNoTargetAuthorityEntity {
    entity: Entity,
    parent: Option<Entity>,
    children: Vec<Entity>,
    registry_claim: Option<ExactRegistryClaim>,
    ownership_seal: Option<RouteOwnershipSeal>,
    manifest: Option<ComponentManifest>,
}

impl FrozenBackNoTargetAuthorityEntity {
    fn observe(world: &World, registry: &RouteEntityRegistry, entity: Entity) -> Self {
        Self {
            entity,
            parent: world.get::<ChildOf>(entity).map(ChildOf::parent),
            children: world
                .get::<Children>(entity)
                .map(|children| children.iter().copied().collect())
                .unwrap_or_default(),
            registry_claim: registry.record(entity).map(|record| record.claim()),
            ownership_seal: world.get::<RouteOwnershipSeal>(entity).copied(),
            manifest: ComponentManifest::from_world(world, entity),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FrozenBackNoTargetHostEdgeDelta {
    host: Entity,
    top_slot: Entity,
    before: Vec<Entity>,
    after: Vec<Entity>,
    phase: BackNoTargetHostEdgePhase,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BackNoTargetHostEdgePhase {
    UnstartedBefore,
    CommittedAfter,
    CommittedRetainedLostBefore,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FrozenBackNoTargetAllowedWriteSet {
    targets: Vec<Entity>,
    host_edge: FrozenBackNoTargetHostEdgeDelta,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FrozenBackNoTargetAuthorityPartition {
    roots: Vec<Entity>,
    target_observed: Vec<FrozenBackNoTargetAuthorityEntity>,
    protected_observed: Vec<FrozenBackNoTargetAuthorityEntity>,
    allowed: FrozenBackNoTargetAllowedWriteSet,
    terminal: Option<Vec<PreparedBackNoTargetAuthorityTerminalProjection>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum PreparedBackNoTargetAuthorityTerminalPresence {
    Absent,
    Retained {
        parent: Option<Entity>,
        children: Vec<Entity>,
        ownership_seal: Option<RouteOwnershipSeal>,
        manifest: ComponentManifest,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum PreparedBackNoTargetAuthorityTerminalAuthority {
    None,
    Quarantine {
        lease: QuarantineLeaseId,
        lease_entities: Vec<Entity>,
        owner: RegistryOwner,
        seal: crate::registry::OwnershipSeal,
        manifest: ComponentManifest,
    },
    AuthorityLostTombstone {
        projection: PreparedAuthorityLostResidualTerminalProjection,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PreparedBackNoTargetAuthorityTerminalProjection {
    obligation: usize,
    entity: Entity,
    ledger_domain: u64,
    attempt_revision: u64,
    disposition: CleanupDisposition,
    receipt_binding: u64,
    registry_claim: Option<ExactRegistryClaim>,
    presence: PreparedBackNoTargetAuthorityTerminalPresence,
    authority: PreparedBackNoTargetAuthorityTerminalAuthority,
}

impl PreparedBackNoTargetAuthorityTerminalProjection {
    fn matches_receipt(&self, receipt: &CleanupReceipt) -> bool {
        receipt.ledger_domain == self.ledger_domain
            && receipt.attempt_revision == self.attempt_revision
            && receipt.obligation == self.obligation
            && receipt.disposition == self.disposition
            && receipt.binding == self.receipt_binding
    }

    fn is_current(&self, world: &World) -> bool {
        let registry = world.resource::<RouteEntityRegistry>();
        let Some(table) = world.get_resource::<QuarantineLeaseTable>() else {
            return false;
        };
        if registry.record(self.entity).map(|record| record.claim()) != self.registry_claim {
            return false;
        }
        let presence_is_current = match &self.presence {
            PreparedBackNoTargetAuthorityTerminalPresence::Absent => {
                world.get_entity(self.entity).is_err()
            }
            PreparedBackNoTargetAuthorityTerminalPresence::Retained {
                parent,
                children,
                ownership_seal,
                manifest,
            } => {
                world.get_entity(self.entity).is_ok()
                    && world.get::<ChildOf>(self.entity).map(ChildOf::parent) == *parent
                    && observed_children(world, self.entity) == *children
                    && world.get::<RouteOwnershipSeal>(self.entity).copied() == *ownership_seal
                    && ComponentManifest::from_world(world, self.entity).as_ref() == Some(manifest)
            }
        };
        if !presence_is_current {
            return false;
        }
        match &self.authority {
            PreparedBackNoTargetAuthorityTerminalAuthority::None => {
                table.record(self.entity).is_none()
                    && table.unresolved_record(self.entity).is_none()
                    && table.authority_lost_tombstone(self.entity).is_none()
            }
            PreparedBackNoTargetAuthorityTerminalAuthority::Quarantine {
                lease,
                lease_entities,
                owner,
                seal,
                manifest,
            } => {
                let Some(record) = table.record(self.entity) else {
                    return false;
                };
                table
                    .lease(*lease)
                    .is_some_and(|lease| lease.owned_entities() == *lease_entities)
                    && record.owner() == owner
                    && record.seal() == *seal
                    && record.lifecycle() == EntityLifecycle::Quarantined
                    && record.manifest() == manifest
                    && table.unresolved_record(self.entity).is_none()
                    && table.authority_lost_tombstone(self.entity).is_none()
                    && terminal_quarantine_root_is_inert(world, &record.claim())
            }
            PreparedBackNoTargetAuthorityTerminalAuthority::AuthorityLostTombstone {
                projection,
            } => {
                table.record(self.entity).is_none()
                    && table.unresolved_record(self.entity).is_none()
                    && table
                        .authority_lost_tombstone(self.entity)
                        .is_some_and(|tombstone| tombstone.matches_prepared_terminal(projection))
                    && match &self.presence {
                        PreparedBackNoTargetAuthorityTerminalPresence::Absent => true,
                        PreparedBackNoTargetAuthorityTerminalPresence::Retained { .. } => {
                            terminal_authority_lost_entity_is_inert(world, self.entity)
                        }
                    }
            }
        }
    }
}

impl FrozenBackNoTargetAuthorityPartition {
    fn freeze(
        world: &World,
        registry: &RouteEntityRegistry,
        host: &HostBinding,
        mut targets: Vec<Entity>,
    ) -> Option<Self> {
        targets.sort_by_key(|entity| entity.to_bits());
        targets.dedup();
        if !targets.contains(&host.top_slot) {
            return None;
        }
        let before = world
            .get::<Children>(host.host)
            .map(|children| children.iter().copied().collect::<Vec<_>>())
            .unwrap_or_default();
        let mut after = before.clone();
        after.retain(|entity| *entity != host.top_slot);
        let membership = before
            .iter()
            .filter(|entity| **entity == host.top_slot)
            .count();
        let parent = world.get::<ChildOf>(host.top_slot).map(ChildOf::parent);
        if (parent == Some(host.host) && membership != 1)
            || (parent != Some(host.host) && membership != 0)
        {
            return None;
        }
        let mut roots = targets.clone();
        roots.push(host.host);
        roots.sort_by_key(|entity| entity.to_bits());
        roots.dedup();
        let closure = observed_descendant_closure(world, roots.iter().copied());
        let mut closure = closure.into_iter().collect::<Vec<_>>();
        closure.sort_by_key(|entity| entity.to_bits());
        let target_observed = targets
            .iter()
            .copied()
            .map(|entity| FrozenBackNoTargetAuthorityEntity::observe(world, registry, entity))
            .collect::<Vec<_>>();
        let protected_observed = closure
            .iter()
            .copied()
            .filter(|entity| !targets.contains(entity))
            .map(|entity| FrozenBackNoTargetAuthorityEntity::observe(world, registry, entity))
            .collect::<Vec<_>>();
        if protected_observed
            .iter()
            .any(|protected| targets.contains(&protected.entity))
        {
            return None;
        }
        Some(Self {
            roots,
            target_observed,
            protected_observed,
            allowed: FrozenBackNoTargetAllowedWriteSet {
                targets,
                host_edge: FrozenBackNoTargetHostEdgeDelta {
                    host: host.host,
                    top_slot: host.top_slot,
                    before,
                    after,
                    phase: BackNoTargetHostEdgePhase::UnstartedBefore,
                },
            },
            terminal: None,
        })
    }

    fn install_terminal_projections(
        &mut self,
        obligations: &[CleanupObligation],
        mut projections: Vec<PreparedBackNoTargetAuthorityTerminalProjection>,
    ) -> bool {
        projections.sort_by_key(|projection| projection.obligation);
        if self.terminal.is_some()
            || projections.len() != obligations.len()
            || projections.iter().enumerate().any(|(index, projection)| {
                let Some(obligation) = obligations.get(index) else {
                    return true;
                };
                projection.obligation != index
                    || projection.entity != obligation.claim.entity()
                    || !self.allowed.targets.contains(&projection.entity)
                    || projection.receipt_binding
                        != receipt_binding(
                            projection.ledger_domain,
                            projection.attempt_revision,
                            projection.obligation,
                            projection.disposition,
                            obligation,
                        )
            })
        {
            return false;
        }
        self.terminal = Some(projections);
        true
    }

    fn commit_host_edge_phase(&mut self, phase: BackNoTargetHostEdgePhase, world: &World) -> bool {
        if self.allowed.host_edge.phase != BackNoTargetHostEdgePhase::UnstartedBefore
            || phase == BackNoTargetHostEdgePhase::UnstartedBefore
        {
            return false;
        }
        let children = observed_children(world, self.allowed.host_edge.host);
        let parent = world
            .get::<ChildOf>(self.allowed.host_edge.top_slot)
            .map(ChildOf::parent);
        let exact = match phase {
            BackNoTargetHostEdgePhase::UnstartedBefore => false,
            BackNoTargetHostEdgePhase::CommittedAfter => {
                children == self.allowed.host_edge.after
                    && parent != Some(self.allowed.host_edge.host)
            }
            BackNoTargetHostEdgePhase::CommittedRetainedLostBefore => {
                children == self.allowed.host_edge.before
                    && parent == Some(self.allowed.host_edge.host)
            }
        };
        if exact {
            self.allowed.host_edge.phase = phase;
        }
        exact
    }

    pub(crate) fn refreeze_unstarted(&self, world: &World) -> bool {
        if self.allowed.host_edge.phase != BackNoTargetHostEdgePhase::UnstartedBefore
            || self.terminal.is_some()
        {
            return false;
        }
        let registry = world.resource::<RouteEntityRegistry>();
        let mut closure = observed_descendant_closure(world, self.roots.iter().copied())
            .into_iter()
            .collect::<Vec<_>>();
        closure.sort_by_key(|entity| entity.to_bits());
        let mut expected = self.allowed.targets.clone();
        expected.extend(
            self.protected_observed
                .iter()
                .map(|protected| protected.entity),
        );
        expected.sort_by_key(|entity| entity.to_bits());
        expected.dedup();
        closure == expected
            && self.protected_observed.iter().all(|expected| {
                FrozenBackNoTargetAuthorityEntity::observe(world, &registry, expected.entity)
                    == *expected
            })
            && self.target_observed.iter().all(|expected| {
                FrozenBackNoTargetAuthorityEntity::observe(world, &registry, expected.entity)
                    == *expected
            })
            && world
                .get::<Children>(self.allowed.host_edge.host)
                .map(|children| children.iter().copied().collect::<Vec<_>>())
                .unwrap_or_default()
                == self.allowed.host_edge.before
    }

    fn refreeze(
        &self,
        world: &World,
        obligations: &[CleanupObligation],
        receipts: &[Option<CleanupReceipt>],
    ) -> bool {
        let Some(terminal) = self.terminal.as_ref() else {
            return receipts.iter().all(Option::is_none) && self.refreeze_unstarted(world);
        };
        if obligations.len() != receipts.len()
            || terminal.len() != obligations.len()
            || obligations.iter().enumerate().any(|(index, obligation)| {
                let Some(projection) = terminal.get(index) else {
                    return true;
                };
                projection.obligation != index
                    || projection.entity != obligation.claim.entity()
                    || receipts
                        .get(index)
                        .and_then(Option::as_ref)
                        .is_some_and(|receipt| {
                            !projection.matches_receipt(receipt) || !projection.is_current(world)
                        })
            })
        {
            return false;
        }
        let registry = world.resource::<RouteEntityRegistry>();
        let mut remaining = obligations
            .iter()
            .enumerate()
            .filter_map(|(index, obligation)| {
                receipts
                    .get(index)
                    .is_some_and(Option::is_none)
                    .then_some(obligation.claim.entity())
            })
            .collect::<Vec<_>>();
        remaining.sort_by_key(|entity| entity.to_bits());
        remaining.dedup();
        if remaining
            .iter()
            .any(|entity| !self.allowed.targets.contains(entity))
        {
            return false;
        }
        let mut present_targets = self
            .allowed
            .targets
            .iter()
            .copied()
            .filter(|entity| world.get_entity(*entity).is_ok())
            .collect::<Vec<_>>();
        present_targets.sort_by_key(|entity| entity.to_bits());
        let mut roots = present_targets.clone();
        roots.push(self.allowed.host_edge.host);
        roots.sort_by_key(|entity| entity.to_bits());
        roots.dedup();
        let mut closure = observed_descendant_closure(world, roots.iter().copied())
            .into_iter()
            .collect::<Vec<_>>();
        closure.sort_by_key(|entity| entity.to_bits());
        let protected = self
            .protected_observed
            .iter()
            .map(|observed| observed.entity)
            .collect::<Vec<_>>();
        let mut expected_closure = present_targets.clone();
        expected_closure.extend(protected.iter().copied());
        expected_closure.sort_by_key(|entity| entity.to_bits());
        expected_closure.dedup();
        if closure != expected_closure {
            return false;
        }
        if self.protected_observed.iter().any(|expected| {
            let observed =
                FrozenBackNoTargetAuthorityEntity::observe(world, &registry, expected.entity);
            if expected.entity != self.allowed.host_edge.host {
                return observed != *expected;
            }
            let expected_children = match self.allowed.host_edge.phase {
                BackNoTargetHostEdgePhase::UnstartedBefore
                | BackNoTargetHostEdgePhase::CommittedRetainedLostBefore => {
                    &self.allowed.host_edge.before
                }
                BackNoTargetHostEdgePhase::CommittedAfter => &self.allowed.host_edge.after,
            };
            let children_changed = expected_children != &expected.children;
            observed.entity != expected.entity
                || observed.parent != expected.parent
                || observed.children != *expected_children
                || observed.registry_claim != expected.registry_claim
                || observed.ownership_seal != expected.ownership_seal
                || (observed.manifest != expected.manifest
                    && (!children_changed
                        || expected.manifest.as_ref().is_none_or(|manifest| {
                            !observed_parent_manifest_differs_only_by_children(
                                world,
                                expected.entity,
                                manifest,
                            )
                        })))
        }) {
            return false;
        }
        let host_children = world
            .get::<Children>(self.allowed.host_edge.host)
            .map(|children| children.iter().copied().collect::<Vec<_>>())
            .unwrap_or_default();
        let top_parent = world
            .get::<ChildOf>(self.allowed.host_edge.top_slot)
            .map(ChildOf::parent);
        let host_edge_is_current = match self.allowed.host_edge.phase {
            BackNoTargetHostEdgePhase::UnstartedBefore => {
                host_children == self.allowed.host_edge.before
            }
            BackNoTargetHostEdgePhase::CommittedAfter => {
                host_children == self.allowed.host_edge.after
                    && top_parent != Some(self.allowed.host_edge.host)
            }
            BackNoTargetHostEdgePhase::CommittedRetainedLostBefore => {
                host_children == self.allowed.host_edge.before
                    && top_parent == Some(self.allowed.host_edge.host)
            }
        };
        if !host_edge_is_current {
            return false;
        }
        self.target_observed
            .iter()
            .filter(|expected| remaining.contains(&expected.entity))
            .all(|expected| {
                let observed =
                    FrozenBackNoTargetAuthorityEntity::observe(world, &registry, expected.entity);
                let projected_children = expected
                    .children
                    .iter()
                    .copied()
                    .filter(|child| {
                        !obligations.iter().enumerate().any(|(index, obligation)| {
                            obligation.claim.entity() == *child
                                && obligation.expected_parent == expected.entity
                                && receipts.get(index).and_then(Option::as_ref).is_some_and(
                                    |receipt| {
                                        matches!(
                                            receipt.disposition,
                                            CleanupDisposition::Removed
                                                | CleanupDisposition::RemovedDetached
                                                | CleanupDisposition::AlreadyAbsent
                                        )
                                    },
                                )
                        })
                    })
                    .collect::<Vec<_>>();
                let children_changed = projected_children != expected.children;
                let projected_parent = if expected.entity == self.allowed.host_edge.top_slot
                    && expected.parent == Some(self.allowed.host_edge.host)
                    && self.allowed.host_edge.phase == BackNoTargetHostEdgePhase::CommittedAfter
                {
                    None
                } else {
                    expected.parent
                };
                let child_of_changed = projected_parent != expected.parent;
                observed.entity == expected.entity
                    && observed.parent == projected_parent
                    && observed.children == projected_children
                    && observed.registry_claim == expected.registry_claim
                    && observed.ownership_seal == expected.ownership_seal
                    && (observed.manifest == expected.manifest
                        || expected.manifest.as_ref().is_some_and(|manifest| {
                            observed_manifest_differs_only_by_allowed_relationships(
                                world,
                                expected.entity,
                                manifest,
                                children_changed,
                                child_of_changed,
                            )
                        }))
            })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AuthoritySettlementManifestError {
    RetryableCapacity,
    InvalidAuthority,
    InvalidTopology,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FailedManifestClaimState {
    Exact,
    Absent,
    Missing,
    Replaced,
    PhysicalDrift,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FailedMountParentFreezePredicate {
    ClaimAuthority,
    UnsupportedOwner,
    ChildrenProjection,
    ConflictingCandidate,
    MissingRegistryRecord,
    MissingPhysicalManifest,
    RegistryManifestMismatch,
    ParentAuthority,
    PhysicalAuthority,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum FailedBoundarySource {
    FrameMountSlot {
        frame: RouteFrameVersion,
        removed_child: Entity,
    },
    FramePhysicalParent {
        frame: RouteFrameVersion,
        mount_slot: Entity,
        removed_child: Entity,
    },
    MountedSlotAnchor {
        parent: RouteFrameVersion,
        outlet: RouteSlotKey,
        slot: RouteSlotId,
        removed_child: Entity,
    },
    ResidualAnchor {
        parent: RouteFrameVersion,
        outlet: RouteSlotKey,
        slot: RouteSlotId,
        removed_child: Entity,
    },
    RejectedAnchor {
        parent: RouteFrameVersion,
        outlet: RouteSlotKey,
        slot: RouteSlotId,
        removed_child: Entity,
    },
}

impl FailedBoundarySource {
    fn removed_child(&self) -> Entity {
        match self {
            Self::FrameMountSlot { removed_child, .. }
            | Self::FramePhysicalParent { removed_child, .. }
            | Self::MountedSlotAnchor { removed_child, .. }
            | Self::ResidualAnchor { removed_child, .. }
            | Self::RejectedAnchor { removed_child, .. } => *removed_child,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum FailedBoundaryExpectation {
    ExactClaim(ExactRegistryClaim),
}

impl FailedBoundaryExpectation {
    pub(crate) fn exact_claim(&self) -> &ExactRegistryClaim {
        match self {
            Self::ExactClaim(claim) => claim,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FailedBoundaryDrift {
    pub(crate) source: FailedBoundarySource,
    pub(crate) expected: FailedBoundaryExpectation,
    pub(crate) state: FailedManifestClaimState,
    pub(crate) predicate: FailedMountParentFreezePredicate,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FailedMountParentFreezeError {
    pub(crate) source: Option<FailedBoundarySource>,
    pub(crate) expected: FailedBoundaryExpectation,
    pub(crate) state: FailedManifestClaimState,
    pub(crate) predicate: FailedMountParentFreezePredicate,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PendingBoundaryBinding {
    pub(crate) source: FailedBoundarySource,
    pub(crate) obligation: ExactRegistryClaim,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum FailedAuthorityBoundaryBinding {
    Current {
        source: FailedBoundarySource,
        authority: ExactRegistryClaim,
    },
    Pending(PendingBoundaryBinding),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FailedPreparedTopologyError {
    DuplicateEntity,
    ConflictingOwner,
    DuplicateIncoming,
    CycleOrRevisit,
    Rootless,
    AmbiguousBoundaryOwnership,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum FailedManifestBuildError {
    RetryableCapacity,
    InvalidTopology(FailedPreparedTopologyError),
    MountParentFreeze(FailedMountParentFreezeError),
}

impl FrozenCleanupManifest {
    pub(crate) fn empty() -> Self {
        Self {
            obligations: Vec::new(),
            mount_parents: Vec::new(),
            boundary_drifts: Vec::new(),
            pending_boundary_loss_claims: Vec::new(),
            digest: 0,
        }
    }

    pub(crate) fn from_frame(
        world: &World,
        registry: &RouteEntityRegistry,
        frame: &RealizedFrame,
        projected_claims: &[ExactRegistryClaim],
        successor: Option<&RealizedFrame>,
    ) -> Option<Self> {
        let obligations = frame_obligations(registry, frame, projected_claims)?;
        let mount_parents = freeze_external_mount_parents(
            world,
            registry,
            &obligations,
            [frame_mount_parent_candidate(frame, successor)?],
        )?;
        let digest = frozen_manifest_digest(&obligations, &mount_parents);
        Some(Self {
            obligations,
            mount_parents,
            boundary_drifts: Vec::new(),
            pending_boundary_loss_claims: Vec::new(),
            digest,
        })
    }

    pub(crate) fn from_current_frame(
        world: &World,
        registry: &RouteEntityRegistry,
        frame: &RealizedFrame,
    ) -> Option<Self> {
        let claims = frame.claims().cloned().collect::<Vec<_>>();
        Self::from_frame(world, registry, frame, &claims, None)
    }

    pub(crate) fn from_leaving_branch(
        world: &World,
        registry: &RouteEntityRegistry,
        branch: &RealizedBranch,
        projected_claims: &[ExactRegistryClaim],
        successor: Option<&RealizedBranch>,
    ) -> Option<Self> {
        Self::from_leaving_branch_parts(
            world,
            registry,
            branch.frames(),
            branch.child_slots(),
            projected_claims,
            successor,
        )
    }

    fn from_leaving_branch_parts(
        world: &World,
        registry: &RouteEntityRegistry,
        frames: &[RealizedFrame],
        child_slots: &[MountedChildSlot],
        projected_claims: &[ExactRegistryClaim],
        successor: Option<&RealizedBranch>,
    ) -> Option<Self> {
        let obligation_count = frames.iter().try_fold(child_slots.len(), |count, frame| {
            count.checked_add(frame.obligation_count())
        })?;
        let unique = projected_claims
            .iter()
            .map(ExactRegistryClaim::entity)
            .collect::<HashSet<_>>();
        if unique.len() != projected_claims.len() {
            return None;
        }
        let leaving_entities = frames
            .iter()
            .flat_map(RealizedFrame::claims)
            .map(ExactRegistryClaim::entity)
            .chain(child_slots.iter().map(MountedChildSlot::slot))
            .collect::<HashSet<_>>();
        if leaving_entities.len() != obligation_count
            || leaving_entities.iter().any(|entity| {
                projected_claims
                    .iter()
                    .filter(|claim| claim.entity() == *entity)
                    .count()
                    != 1
                    || projected_claims.iter().any(|claim| {
                        claim.entity() == *entity && claim.lifecycle() != EntityLifecycle::Leaving
                    })
            })
        {
            return None;
        }

        let mut obligations = Vec::with_capacity(obligation_count);
        if frames.is_empty() {
            for slot in child_slots {
                let projected = projected_claims
                    .iter()
                    .find(|claim| claim.entity() == slot.slot())?;
                obligations.push(mounted_slot_obligation_with_claim(
                    registry, slot, projected,
                )?);
            }
        } else {
            if child_slots
                .iter()
                .any(|slot| !leaving_slot_has_exact_parent(frames, slot))
            {
                return None;
            }
            let mut incoming = vec![None; frames.len()];
            for (slot_index, slot) in child_slots.iter().enumerate() {
                let mut children = frames
                    .iter()
                    .enumerate()
                    .filter(|(_, frame)| frame.mount_binding().matches_mounted_child_slot(slot));
                if let Some((child_index, _)) = children.next() {
                    if children.next().is_some()
                        || incoming[child_index].replace(slot_index).is_some()
                    {
                        return None;
                    }
                }
            }
            let roots = incoming
                .iter()
                .enumerate()
                .filter_map(|(index, incoming)| incoming.is_none().then_some(index))
                .collect::<Vec<_>>();
            if roots.is_empty() {
                return None;
            }
            let mut visit = vec![0u8; frames.len()];
            for root in roots {
                append_exact_leaving_unit(
                    registry,
                    frames,
                    child_slots,
                    projected_claims,
                    root,
                    &mut visit,
                    &mut obligations,
                )?;
            }
            if visit.iter().any(|state| *state != 2) {
                return None;
            }
        }
        if obligations.len() != obligation_count
            || obligations
                .iter()
                .map(|obligation| obligation.claim.entity())
                .collect::<HashSet<_>>()
                .len()
                != obligation_count
        {
            return None;
        }

        let mut parent_candidates = frames
            .iter()
            .map(|frame| {
                let successor = exact_successor_for_mount(frame, successor)?;
                frame_mount_parent_candidate(frame, successor)
            })
            .collect::<Option<Vec<_>>>()?;
        parent_candidates.extend(
            child_slots
                .iter()
                .map(mounted_anchor_parent_candidate)
                .collect::<Option<Vec<_>>>()?,
        );
        let mut mount_parents =
            freeze_external_mount_parents(world, registry, &obligations, parent_candidates)?;
        rebind_external_mount_parent_claims(&mut mount_parents, projected_claims)?;
        let digest = frozen_manifest_digest(&obligations, &mount_parents);
        Some(Self {
            obligations,
            mount_parents,
            boundary_drifts: Vec::new(),
            pending_boundary_loss_claims: Vec::new(),
            digest,
        })
    }

    pub(crate) fn from_failed_prepared_branch(
        world: &World,
        registry: &RouteEntityRegistry,
        branch: &FailedPreparedBranch,
    ) -> Result<Self, FailedManifestBuildError> {
        if branch.frames().is_empty()
            && branch.child_slots().is_empty()
            && branch.mount_residuals().is_empty()
            && branch.detached_frames().is_empty()
            && branch.rejected_child_slots().is_empty()
        {
            return Ok(Self::empty());
        }
        let topology = FailedPreparedTopology::build(world, registry, branch)?;
        let obligations = topology.freeze_obligations(world, registry)?;
        let (parent_candidates, mut candidate_drifts) = topology.freeze_parent_candidates(world)?;
        let (mount_parents, mut freeze_drifts) = freeze_external_mount_parents_detailed(
            world,
            registry,
            &obligations,
            parent_candidates,
        )
        .map_err(FailedManifestBuildError::MountParentFreeze)?;
        let mut boundary_drifts = topology.boundary_drifts.clone();
        boundary_drifts.append(&mut candidate_drifts);
        boundary_drifts.append(&mut freeze_drifts);
        let digest = frozen_manifest_digest(&obligations, &mount_parents);
        Ok(Self {
            obligations,
            mount_parents,
            boundary_drifts,
            pending_boundary_loss_claims: Vec::new(),
            digest,
        })
    }

    pub(crate) fn from_authority_settlement(
        world: &World,
        registry: &RouteEntityRegistry,
        pending: &crate::branch_transaction::PreparedBranch,
        active: &RealizedBranch,
        host: &HostBinding,
    ) -> Result<Self, AuthoritySettlementManifestError> {
        let mut obligations = Vec::new();
        append_exact_authority_branch(
            world,
            registry,
            pending.frames(),
            pending.child_slots(),
            &mut obligations,
        )?;
        append_exact_authority_branch(
            world,
            registry,
            active.frames(),
            active.child_slots(),
            &mut obligations,
        )?;

        normalize_authority_obligations(&mut obligations)?;

        if !matches!(
            host.top_slot_claim.owner(),
            RegistryOwner::Committed(EntityOwner::HostTopSlot { host: version, slot })
                if version == &host.version && *slot == host.slot_id
        ) {
            return Err(AuthoritySettlementManifestError::InvalidAuthority);
        }
        let top_slot_record = registry.record(host.top_slot);
        let registry_exact =
            top_slot_record.is_some_and(|record| record.claim() == host.top_slot_claim);
        let physical_exact =
            registry_exact && cleanup_claim_has_exact_authority(world, &host.top_slot_claim);
        let manifest = top_slot_record
            .filter(|record| record.claim() == host.top_slot_claim)
            .map(|record| record.manifest().clone())
            .or_else(|| ComponentManifest::from_world(world, host.top_slot))
            .unwrap_or_else(|| ComponentManifest::new([]));
        obligations.push(CleanupObligation {
            claim: host.top_slot_claim.clone(),
            manifest,
            expected_parent: host.host,
            unit_root: host.top_slot,
            authorized_parent: host.host,
            allow_missing_components: false,
            terminal_mode: if physical_exact {
                CleanupTerminalMode::AuthorityExact
            } else {
                CleanupTerminalMode::AuthorityLost
            },
        });
        normalize_authority_obligations(&mut obligations)?;
        let mount_parents = Vec::new();
        let digest = frozen_manifest_digest(&obligations, &mount_parents);
        Ok(Self {
            obligations,
            mount_parents,
            boundary_drifts: Vec::new(),
            pending_boundary_loss_claims: Vec::new(),
            digest,
        })
    }

    pub(crate) fn from_back_no_target_authority(
        world: &World,
        registry: &RouteEntityRegistry,
        active: &RealizedBranch,
        host: &HostBinding,
    ) -> Result<Self, AuthoritySettlementManifestError> {
        let mut obligations = Vec::new();
        append_exact_authority_branch(
            world,
            registry,
            active.frames(),
            active.child_slots(),
            &mut obligations,
        )?;
        normalize_authority_obligations(&mut obligations)?;
        if host.top_slot_claim.entity() != host.top_slot
            || host.candidate.host() != host.host
            || !matches!(
                host.top_slot_claim.owner(),
                RegistryOwner::Committed(EntityOwner::HostTopSlot { host: version, slot })
                    if version == &host.version && *slot == host.slot_id
            )
        {
            return Err(AuthoritySettlementManifestError::InvalidAuthority);
        }
        obligations.push(authority_host_obligation(world, registry, host));
        normalize_authority_obligations(&mut obligations)?;
        let mount_parents = Vec::new();
        let digest = frozen_manifest_digest(&obligations, &mount_parents);
        Ok(Self {
            obligations,
            mount_parents,
            boundary_drifts: Vec::new(),
            pending_boundary_loss_claims: Vec::new(),
            digest,
        })
    }

    pub(crate) fn finalize_failed_authority_settlement(
        mut self,
        world: &World,
        registry: &RouteEntityRegistry,
        active: Option<&RealizedBranch>,
        host: &HostBinding,
        bindings: Vec<FailedAuthorityBoundaryBinding>,
    ) -> Result<Self, AuthoritySettlementManifestError> {
        if !self.pending_boundary_loss_claims.is_empty()
            || bindings.len() != self.boundary_drifts.len()
            || bindings.iter().enumerate().any(|(index, binding)| {
                let drift = &self.boundary_drifts[index];
                let matches = match binding {
                    FailedAuthorityBoundaryBinding::Current { source, authority } => {
                        source == &drift.source && authority == drift.expected.exact_claim()
                    }
                    FailedAuthorityBoundaryBinding::Pending(binding) => {
                        binding.source == drift.source
                            && binding.source.removed_child() == binding.obligation.entity()
                            && self.contains_exact_obligation(&binding.obligation)
                    }
                };
                !matches
                    || bindings[index + 1..].iter().any(|other| match other {
                        FailedAuthorityBoundaryBinding::Current { source, .. } => {
                            source == &drift.source
                        }
                        FailedAuthorityBoundaryBinding::Pending(binding) => {
                            binding.source == drift.source
                        }
                    })
            })
        {
            return Err(AuthoritySettlementManifestError::InvalidTopology);
        }

        for (drift, binding) in self.boundary_drifts.iter().zip(&bindings) {
            if let FailedAuthorityBoundaryBinding::Pending(binding) = binding {
                let obligation = self
                    .obligations
                    .iter_mut()
                    .find(|obligation| obligation.claim == binding.obligation)
                    .ok_or(AuthoritySettlementManifestError::InvalidTopology)?;
                obligation.terminal_mode = CleanupTerminalMode::AuthorityLost;
                for claim in [&binding.obligation, drift.expected.exact_claim()] {
                    if !self.pending_boundary_loss_claims.contains(claim) {
                        self.pending_boundary_loss_claims.push(claim.clone());
                    }
                }
            }
        }
        for obligation in &mut self.obligations {
            obligation.terminal_mode = match obligation.terminal_mode {
                CleanupTerminalMode::Normal => CleanupTerminalMode::AuthorityExact,
                CleanupTerminalMode::AuthorityLost => CleanupTerminalMode::AuthorityLost,
                CleanupTerminalMode::ForceUnresolved | CleanupTerminalMode::AuthorityExact => {
                    return Err(AuthoritySettlementManifestError::InvalidTopology);
                }
            };
        }
        if let Some(active) = active {
            append_exact_authority_branch(
                world,
                registry,
                active.frames(),
                active.child_slots(),
                &mut self.obligations,
            )?;
        }
        if host.top_slot_claim.entity() != host.top_slot
            || host.candidate.host() != host.host
            || !matches!(
                host.top_slot_claim.owner(),
                RegistryOwner::Committed(EntityOwner::HostTopSlot { host: version, slot })
                    if version == &host.version && *slot == host.slot_id
            )
        {
            return Err(AuthoritySettlementManifestError::InvalidAuthority);
        }
        self.obligations
            .push(authority_host_obligation(world, registry, host));
        normalize_authority_obligations(&mut self.obligations)?;

        if bindings.iter().any(|binding| {
            let claim = match binding {
                FailedAuthorityBoundaryBinding::Current { authority, .. } => authority,
                FailedAuthorityBoundaryBinding::Pending(binding) => self
                    .boundary_drifts
                    .iter()
                    .find(|drift| drift.source == binding.source)
                    .expect("prevalidated failed-authority binding retains its drift")
                    .expected
                    .exact_claim(),
            };
            !self.contains_exact_obligation(claim)
        }) || self.mount_parents.iter().any(|parent| {
            !self.contains_exact_obligation(&parent.claim)
                || !self
                    .obligations
                    .iter()
                    .any(|obligation| obligation.claim.entity() == parent.removed_child)
        }) {
            return Err(AuthoritySettlementManifestError::InvalidTopology);
        }

        self.mount_parents.clear();
        self.boundary_drifts.clear();
        self.digest = frozen_manifest_digest(&self.obligations, &self.mount_parents);
        Ok(self)
    }

    pub(crate) const fn digest(&self) -> u64 {
        self.digest
    }

    pub(crate) fn len(&self) -> usize {
        self.obligations.len()
    }

    pub(crate) fn obligation_entities(&self) -> Vec<Entity> {
        self.obligations
            .iter()
            .map(|obligation| obligation.claim.entity())
            .collect()
    }

    pub(crate) fn freeze_back_no_target_authority_partition(
        &self,
        world: &World,
        registry: &RouteEntityRegistry,
        host: &HostBinding,
    ) -> Option<FrozenBackNoTargetAuthorityPartition> {
        FrozenBackNoTargetAuthorityPartition::freeze(
            world,
            registry,
            host,
            self.obligation_entities(),
        )
    }

    pub(crate) fn boundary_drifts(&self) -> &[FailedBoundaryDrift] {
        &self.boundary_drifts
    }

    pub(crate) fn contains_exact_obligation(&self, claim: &ExactRegistryClaim) -> bool {
        self.obligations
            .iter()
            .any(|obligation| obligation.claim == *claim)
    }

    pub(crate) fn contains_frame_obligation(&self, frame: &RouteFrameVersion) -> bool {
        self.obligations.iter().any(|obligation| {
            matches!(
                obligation.claim.owner(),
                RegistryOwner::Committed(
                    EntityOwner::FrameShell(owner)
                        | EntityOwner::FrameContentTarget(owner)
                        | EntityOwner::FrameOutput { frame: owner, .. }
                ) if owner == frame
            )
        })
    }

    pub(crate) fn allows_pending_boundary_loss(&self, claim: &ExactRegistryClaim) -> bool {
        self.pending_boundary_loss_claims.contains(claim)
    }

    pub(crate) fn finalize_pending_only_boundaries(
        mut self,
        bindings: Vec<PendingBoundaryBinding>,
    ) -> Result<Self, (FailedPreparedTopologyError, Self)> {
        if bindings.len() != self.boundary_drifts.len()
            || bindings.iter().enumerate().any(|(index, binding)| {
                let drift = &self.boundary_drifts[index];
                binding.source != drift.source
                    || binding.source.removed_child() != binding.obligation.entity()
                    || !self.contains_exact_obligation(&binding.obligation)
                    || bindings[index + 1..]
                        .iter()
                        .any(|other| other.source == binding.source)
            })
        {
            return Err((
                FailedPreparedTopologyError::AmbiguousBoundaryOwnership,
                self,
            ));
        }
        for (drift, binding) in self.boundary_drifts.iter().zip(&bindings) {
            let obligation = self
                .obligations
                .iter_mut()
                .find(|obligation| obligation.claim == binding.obligation)
                .expect("prevalidated pending boundary retains its exact local obligation");
            obligation.terminal_mode = CleanupTerminalMode::AuthorityLost;
            for claim in [&binding.obligation, drift.expected.exact_claim()] {
                if !self.pending_boundary_loss_claims.contains(claim) {
                    self.pending_boundary_loss_claims.push(claim.clone());
                }
            }
            let boundary = drift.expected.exact_claim();
            let removed_child = binding.source.removed_child();
            self.mount_parents.retain(|parent| {
                !(parent.claim == *boundary && parent.removed_child == removed_child)
            });
        }
        self.boundary_drifts.clear();
        self.digest = frozen_manifest_digest(&self.obligations, &self.mount_parents);
        Ok(self)
    }

    pub(crate) fn preflight(&self, world: &World) -> bool {
        let receipts = (0..self.obligations.len())
            .map(|_| None)
            .collect::<Vec<_>>();
        let evidence = collect_evidence(world, &self.obligations, &receipts);
        !evidence
            .iter()
            .any(|(_, evidence)| matches!(evidence, CleanupEvidence::StaleClaim))
            && world
                .get_resource::<RouteEntityRegistry>()
                .is_some_and(|registry| {
                    self.mount_parents.iter().all(|parent| {
                        mount_parent_frozen_manifest_matches(registry, parent)
                            && mount_parent_physical_matches(world, registry, parent)
                    })
                })
    }

    #[cfg(test)]
    pub(crate) fn entities(&self) -> Vec<Entity> {
        self.obligations
            .iter()
            .map(|obligation| obligation.claim.entity())
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn replace_boundary_drifts_for_test(
        &mut self,
        boundary_drifts: Vec<FailedBoundaryDrift>,
    ) {
        self.boundary_drifts = boundary_drifts;
    }
}

struct FailedPreparedTopology<'a> {
    frames: Vec<FailedPreparedFrameNode<'a>>,
    slots: Vec<FailedPreparedSlotNode<'a>>,
    auxiliary_slots: Vec<FailedPreparedAuxiliarySlotNode<'a>>,
    boundary_drifts: Vec<FailedBoundaryDrift>,
}

struct FailedPreparedFrameNode<'a> {
    frame: &'a RealizedFrame,
    shell_claim: &'a ExactRegistryClaim,
    incoming: Option<usize>,
    external_mount: Option<FailedPreparedExternalMountNode<'a>>,
    outgoing: Vec<usize>,
}

struct FailedPreparedExternalMountNode<'a> {
    slot: Entity,
    slot_claim: &'a ExactRegistryClaim,
    physical_parent: Entity,
    physical_parent_claim: Option<&'a ExactRegistryClaim>,
}

struct FailedPreparedSlotNode<'a> {
    slot: &'a MountedChildSlot,
    parent: Option<usize>,
    child: Option<usize>,
}

enum FailedPreparedAuxiliarySlotSource<'a> {
    Residual(&'a ChildSlotResidual),
    Rejected(&'a RejectedChildSlot),
}

struct FailedPreparedAuxiliarySlotNode<'a> {
    source: FailedPreparedAuxiliarySlotSource<'a>,
    entity: Entity,
    claim: &'a ExactRegistryClaim,
    parent: &'a RouteFrameVersion,
    outlet: &'a RouteSlotKey,
    slot_id: RouteSlotId,
    boundary: Option<FailedPreparedBoundaryNode<'a>>,
}

struct FailedPreparedBoundaryNode<'a> {
    entity: Entity,
    claim: &'a ExactRegistryClaim,
}

#[cfg(test)]
pub(crate) fn failed_prepared_rejected_topology_facts_for_test(
    world: &World,
    registry: &RouteEntityRegistry,
    branch: &FailedPreparedBranch,
) -> Result<
    Vec<(
        Entity,
        RouteFrameVersion,
        RouteSlotKey,
        RouteSlotId,
        Option<Entity>,
    )>,
    FailedManifestBuildError,
> {
    let topology = FailedPreparedTopology::build(world, registry, branch)?;
    Ok(topology
        .auxiliary_slots
        .iter()
        .filter_map(|node| {
            matches!(node.source, FailedPreparedAuxiliarySlotSource::Rejected(_)).then(|| {
                (
                    node.entity,
                    node.parent.clone(),
                    node.outlet.clone(),
                    node.slot_id,
                    node.boundary.as_ref().map(|boundary| boundary.entity),
                )
            })
        })
        .collect())
}

impl<'a> FailedPreparedTopology<'a> {
    fn build(
        world: &World,
        registry: &RouteEntityRegistry,
        branch: &'a FailedPreparedBranch,
    ) -> Result<Self, FailedManifestBuildError> {
        let frame_count = branch
            .frames()
            .len()
            .checked_add(branch.detached_frames().len())
            .ok_or(FailedManifestBuildError::RetryableCapacity)?;
        let mut frames = Vec::new();
        frames
            .try_reserve_exact(frame_count)
            .map_err(|_| FailedManifestBuildError::RetryableCapacity)?;
        for frame in branch.frames().iter().chain(branch.detached_frames()) {
            let mut shell_claims = frame.claims().filter(|claim| {
                claim.entity() == frame.shell()
                    && matches!(
                        claim.owner(),
                        RegistryOwner::Committed(EntityOwner::FrameShell(owner))
                            if owner == frame.frame()
                    )
            });
            let shell_claim =
                shell_claims
                    .next()
                    .ok_or(FailedManifestBuildError::InvalidTopology(
                        FailedPreparedTopologyError::ConflictingOwner,
                    ))?;
            if shell_claims.next().is_some() {
                return Err(FailedManifestBuildError::InvalidTopology(
                    FailedPreparedTopologyError::ConflictingOwner,
                ));
            }
            frames.push(FailedPreparedFrameNode {
                frame,
                shell_claim,
                incoming: None,
                external_mount: None,
                outgoing: Vec::new(),
            });
        }
        frames.sort_unstable_by(|left, right| failed_frame_node_cmp(left, right));
        if frames.windows(2).any(|pair| {
            pair[0].frame.frame() == pair[1].frame.frame()
                || pair[0].shell_claim == pair[1].shell_claim
        }) {
            return Err(FailedManifestBuildError::InvalidTopology(
                FailedPreparedTopologyError::ConflictingOwner,
            ));
        }

        let mut slots = Vec::new();
        slots
            .try_reserve_exact(branch.child_slots().len())
            .map_err(|_| FailedManifestBuildError::RetryableCapacity)?;
        slots.extend(
            branch
                .child_slots()
                .iter()
                .map(|slot| FailedPreparedSlotNode {
                    slot,
                    parent: None,
                    child: None,
                }),
        );
        slots.sort_unstable_by(|left, right| failed_slot_node_cmp(left, right));

        let auxiliary_count = branch
            .mount_residuals()
            .len()
            .checked_add(branch.rejected_child_slots().len())
            .ok_or(FailedManifestBuildError::RetryableCapacity)?;
        let mut auxiliary_slots = Vec::new();
        auxiliary_slots
            .try_reserve_exact(auxiliary_count)
            .map_err(|_| FailedManifestBuildError::RetryableCapacity)?;
        for residual in branch.mount_residuals() {
            if residual.entity() != residual.claim().entity()
                || residual.anchor() != residual.anchor_claim().entity()
            {
                return Err(FailedManifestBuildError::InvalidTopology(
                    FailedPreparedTopologyError::ConflictingOwner,
                ));
            }
            let slot_id = failed_child_slot_owner_binding(
                residual.claim(),
                residual.parent(),
                residual.outlet(),
                true,
            )?;
            auxiliary_slots.push(FailedPreparedAuxiliarySlotNode {
                source: FailedPreparedAuxiliarySlotSource::Residual(residual),
                entity: residual.entity(),
                claim: residual.claim(),
                parent: residual.parent(),
                outlet: residual.outlet(),
                slot_id,
                boundary: Some(FailedPreparedBoundaryNode {
                    entity: residual.anchor(),
                    claim: residual.anchor_claim(),
                }),
            });
        }
        for rejected in branch.rejected_child_slots() {
            let slot = rejected.slot();
            if slot.slot() != slot.slot_claim().entity() {
                return Err(FailedManifestBuildError::InvalidTopology(
                    FailedPreparedTopologyError::ConflictingOwner,
                ));
            }
            let (parent, outlet, slot_id) = match slot.slot_claim().owner() {
                RegistryOwner::Committed(EntityOwner::ChildSlot {
                    parent,
                    outlet,
                    slot,
                }) => (parent, outlet, *slot),
                _ => {
                    return Err(FailedManifestBuildError::InvalidTopology(
                        FailedPreparedTopologyError::ConflictingOwner,
                    ));
                }
            };
            let boundary = match (
                rejected.canonical_anchor(),
                rejected.canonical_anchor_claim(),
            ) {
                (Some(entity), Some(claim)) if entity == claim.entity() => {
                    Some(FailedPreparedBoundaryNode { entity, claim })
                }
                (None, None) => None,
                _ => {
                    return Err(FailedManifestBuildError::InvalidTopology(
                        FailedPreparedTopologyError::ConflictingOwner,
                    ));
                }
            };
            auxiliary_slots.push(FailedPreparedAuxiliarySlotNode {
                source: FailedPreparedAuxiliarySlotSource::Rejected(rejected),
                entity: slot.slot(),
                claim: slot.slot_claim(),
                parent,
                outlet,
                slot_id,
                boundary,
            });
        }
        auxiliary_slots.sort_unstable_by(failed_auxiliary_slot_node_cmp);

        let primary_count = frames
            .iter()
            .try_fold(0usize, |count, node| {
                count.checked_add(node.frame.obligation_count())
            })
            .ok_or(FailedManifestBuildError::RetryableCapacity)?
            .checked_add(slots.len())
            .and_then(|count| count.checked_add(auxiliary_slots.len()))
            .ok_or(FailedManifestBuildError::RetryableCapacity)?;
        let mut primary_claims = Vec::new();
        primary_claims
            .try_reserve_exact(primary_count)
            .map_err(|_| FailedManifestBuildError::RetryableCapacity)?;
        for claim in frames
            .iter()
            .flat_map(|node| node.frame.claims())
            .chain(slots.iter().map(|node| node.slot.slot_claim()))
            .chain(auxiliary_slots.iter().map(|node| node.claim))
        {
            validate_failed_primary_claim(&primary_claims, claim)?;
            primary_claims.push(claim);
        }

        let mut boundary_drifts = Vec::new();

        for slot_index in 0..slots.len() {
            let slot = slots[slot_index].slot;
            if !failed_slot_owner_is_exact(slot) {
                return Err(FailedManifestBuildError::InvalidTopology(
                    FailedPreparedTopologyError::ConflictingOwner,
                ));
            }
            let mut parents = frames
                .iter()
                .enumerate()
                .filter(|(_, frame)| frame.frame.frame() == slot.parent());
            let parent = parents.next();
            if parents.next().is_some() {
                return Err(FailedManifestBuildError::InvalidTopology(
                    FailedPreparedTopologyError::DuplicateIncoming,
                ));
            }
            if let Some((parent_index, parent)) = parent {
                let anchor_is_stored_exact =
                    parent.frame.outlet(slot.outlet()).is_some_and(|outlet| {
                        outlet.anchor() == slot.anchor()
                            && outlet.anchor_claim() == slot.anchor_claim()
                            && parent
                                .frame
                                .claims()
                                .any(|claim| claim == slot.anchor_claim())
                    });
                if !anchor_is_stored_exact {
                    return Err(FailedManifestBuildError::InvalidTopology(
                        FailedPreparedTopologyError::ConflictingOwner,
                    ));
                }
                slots[slot_index].parent = Some(parent_index);
                frames[parent_index]
                    .outgoing
                    .try_reserve(1)
                    .map_err(|_| FailedManifestBuildError::RetryableCapacity)?;
                frames[parent_index].outgoing.push(slot_index);
            } else {
                if primary_claims
                    .iter()
                    .any(|claim| claim.entity() == slot.anchor())
                {
                    return Err(FailedManifestBuildError::InvalidTopology(
                        FailedPreparedTopologyError::ConflictingOwner,
                    ));
                }
                if let Some(drift) = observe_failed_boundary_claim(
                    world,
                    registry,
                    FailedBoundarySource::MountedSlotAnchor {
                        parent: slot.parent().clone(),
                        outlet: slot.outlet().clone(),
                        slot: slot.slot_id(),
                        removed_child: slot.slot(),
                    },
                    slot.anchor_claim(),
                ) {
                    boundary_drifts.push(drift);
                }
            }
        }

        for frame_index in 0..frames.len() {
            let frame = frames[frame_index].frame;
            let mut incoming = slots
                .iter()
                .enumerate()
                .filter(|(_, slot)| frame.mount_binding().matches_mounted_child_slot(slot.slot));
            let matched = incoming.next();
            if incoming.next().is_some() {
                return Err(FailedManifestBuildError::InvalidTopology(
                    FailedPreparedTopologyError::DuplicateIncoming,
                ));
            }
            if let Some((slot_index, _)) = matched {
                if slots[slot_index].child.replace(frame_index).is_some() {
                    return Err(FailedManifestBuildError::InvalidTopology(
                        FailedPreparedTopologyError::DuplicateIncoming,
                    ));
                }
                frames[frame_index].incoming = Some(slot_index);
            } else {
                let mount = frame.mount_binding();
                if primary_claims
                    .iter()
                    .any(|claim| claim.entity() == mount.slot() || claim == &mount.slot_claim())
                {
                    return Err(FailedManifestBuildError::InvalidTopology(
                        FailedPreparedTopologyError::ConflictingOwner,
                    ));
                }
                if frames.iter().any(|node| {
                    node.external_mount.as_ref().is_some_and(|external| {
                        external.slot == mount.slot() || external.slot_claim == mount.slot_claim()
                    })
                }) {
                    return Err(FailedManifestBuildError::InvalidTopology(
                        FailedPreparedTopologyError::DuplicateIncoming,
                    ));
                }
                if let Some(drift) = observe_failed_boundary_claim(
                    world,
                    registry,
                    FailedBoundarySource::FrameMountSlot {
                        frame: frame.frame().clone(),
                        removed_child: frame.shell(),
                    },
                    mount.slot_claim(),
                ) {
                    boundary_drifts.push(drift);
                }
                let (physical_parent, physical_parent_claim) = mount.physical_parent();
                if let Some(parent_claim) = physical_parent_claim {
                    if let Some(drift) = observe_failed_boundary_claim(
                        world,
                        registry,
                        FailedBoundarySource::FramePhysicalParent {
                            frame: frame.frame().clone(),
                            mount_slot: mount.slot(),
                            removed_child: frame.shell(),
                        },
                        parent_claim,
                    ) {
                        boundary_drifts.push(drift);
                    }
                }
                frames[frame_index].external_mount = Some(FailedPreparedExternalMountNode {
                    slot: mount.slot(),
                    slot_claim: mount.slot_claim(),
                    physical_parent,
                    physical_parent_claim,
                });
            }
        }

        for frame in &mut frames {
            frame.outgoing.sort_unstable_by(|left, right| {
                failed_slot_node_cmp(&slots[*left], &slots[*right])
            });
        }

        for node in &auxiliary_slots {
            if let Some(boundary) = &node.boundary {
                let source = match node.source {
                    FailedPreparedAuxiliarySlotSource::Residual(_) => {
                        FailedBoundarySource::ResidualAnchor {
                            parent: node.parent.clone(),
                            outlet: node.outlet.clone(),
                            slot: node.slot_id,
                            removed_child: node.entity,
                        }
                    }
                    FailedPreparedAuxiliarySlotSource::Rejected(_) => {
                        FailedBoundarySource::RejectedAnchor {
                            parent: node.parent.clone(),
                            outlet: node.outlet.clone(),
                            slot: node.slot_id,
                            removed_child: node.entity,
                        }
                    }
                };
                if let Some(drift) =
                    observe_failed_boundary_claim(world, registry, source, boundary.claim)
                {
                    boundary_drifts.push(drift);
                }
            }
        }

        if (!frames.is_empty() || !slots.is_empty())
            && frames.iter().all(|frame| frame.incoming.is_some())
            && slots.iter().all(|slot| slot.parent.is_some())
        {
            return Err(FailedManifestBuildError::InvalidTopology(
                FailedPreparedTopologyError::Rootless,
            ));
        }
        Ok(Self {
            frames,
            slots,
            auxiliary_slots,
            boundary_drifts,
        })
    }

    fn freeze_obligations(
        &self,
        world: &World,
        registry: &RouteEntityRegistry,
    ) -> Result<Vec<CleanupObligation>, FailedManifestBuildError> {
        let obligation_count = self
            .frames
            .iter()
            .try_fold(self.slots.len(), |count, frame| {
                count.checked_add(frame.frame.obligation_count())
            })
            .ok_or(FailedManifestBuildError::RetryableCapacity)?
            .checked_add(self.auxiliary_slots.len())
            .ok_or(FailedManifestBuildError::RetryableCapacity)?;
        let mut obligations = Vec::new();
        obligations
            .try_reserve_exact(obligation_count)
            .map_err(|_| FailedManifestBuildError::RetryableCapacity)?;

        for node in &self.auxiliary_slots {
            obligations.push(match node.source {
                FailedPreparedAuxiliarySlotSource::Residual(residual) => {
                    failed_residual_slot_obligation(world, registry, residual)?
                }
                FailedPreparedAuxiliarySlotSource::Rejected(rejected) => {
                    failed_rejected_slot_obligation(world, registry, rejected)?
                }
            });
        }

        let mut frame_visit = vec![0u8; self.frames.len()];
        let mut slot_visit = vec![0u8; self.slots.len()];
        for frame_index in self
            .frames
            .iter()
            .enumerate()
            .filter_map(|(index, frame)| frame.incoming.is_none().then_some(index))
        {
            self.append_frame_obligations(
                world,
                registry,
                frame_index,
                &mut frame_visit,
                &mut slot_visit,
                &mut obligations,
            )?;
        }
        for slot_index in self
            .slots
            .iter()
            .enumerate()
            .filter_map(|(index, slot)| slot.parent.is_none().then_some(index))
        {
            self.append_slot_obligations(
                world,
                registry,
                slot_index,
                &mut frame_visit,
                &mut slot_visit,
                &mut obligations,
            )?;
        }
        if frame_visit.iter().any(|state| *state != 2) || slot_visit.iter().any(|state| *state != 2)
        {
            return Err(FailedManifestBuildError::InvalidTopology(
                FailedPreparedTopologyError::Rootless,
            ));
        }
        if obligations.len() != obligation_count {
            return Err(FailedManifestBuildError::InvalidTopology(
                FailedPreparedTopologyError::CycleOrRevisit,
            ));
        }
        Ok(obligations)
    }

    fn freeze_parent_candidates(
        &self,
        world: &World,
    ) -> Result<
        (Vec<CleanupMountParentCandidate>, Vec<FailedBoundaryDrift>),
        FailedManifestBuildError,
    > {
        let auxiliary_candidates = self
            .auxiliary_slots
            .len()
            .checked_mul(2)
            .ok_or(FailedManifestBuildError::RetryableCapacity)?;
        let candidate_count = self
            .frames
            .len()
            .checked_add(self.slots.len())
            .and_then(|count| count.checked_add(auxiliary_candidates))
            .ok_or(FailedManifestBuildError::RetryableCapacity)?;
        let mut candidates = Vec::new();
        let mut drifts = Vec::new();
        candidates
            .try_reserve_exact(candidate_count)
            .map_err(|_| FailedManifestBuildError::RetryableCapacity)?;
        for node in &self.frames {
            if let Some(candidate) = self.failed_frame_parent_candidate(world, node)? {
                candidates.push(candidate);
            }
        }
        for node in &self.slots {
            if !self.boundary_drifted(node.slot.anchor_claim()) {
                let source = FailedBoundarySource::MountedSlotAnchor {
                    parent: node.slot.parent().clone(),
                    outlet: node.slot.outlet().clone(),
                    slot: node.slot.slot_id(),
                    removed_child: node.slot.slot(),
                };
                if let Some(mut candidate) = mounted_anchor_parent_candidate(node.slot) {
                    candidate.failed_claim_source = Some(source);
                    candidates.push(candidate);
                } else {
                    drifts.push(FailedBoundaryDrift {
                        source,
                        expected: FailedBoundaryExpectation::ExactClaim(
                            node.slot.anchor_claim().clone(),
                        ),
                        state: FailedManifestClaimState::PhysicalDrift,
                        predicate: FailedMountParentFreezePredicate::PhysicalAuthority,
                    });
                }
            }
        }
        for node in &self.auxiliary_slots {
            match node.source {
                FailedPreparedAuxiliarySlotSource::Residual(residual) => {
                    if !self.boundary_drifted(residual.anchor_claim()) {
                        let mut candidate = residual_anchor_parent_candidate(world, residual);
                        candidate.failed_claim_source =
                            Some(FailedBoundarySource::ResidualAnchor {
                                parent: node.parent.clone(),
                                outlet: node.outlet.clone(),
                                slot: node.slot_id,
                                removed_child: node.entity,
                            });
                        candidates.push(candidate);
                    }
                }
                FailedPreparedAuxiliarySlotSource::Rejected(rejected) => {
                    if rejected
                        .canonical_anchor_claim()
                        .is_none_or(|claim| !self.boundary_drifted(claim))
                        && let Some(canonical) =
                            rejected_canonical_parent_candidate(world, rejected)?
                    {
                        let mut canonical = canonical;
                        canonical.failed_claim_source =
                            Some(FailedBoundarySource::RejectedAnchor {
                                parent: node.parent.clone(),
                                outlet: node.outlet.clone(),
                                slot: node.slot_id,
                                removed_child: node.entity,
                            });
                        candidates.push(canonical);
                    }
                }
            }
        }
        Ok((candidates, drifts))
    }

    fn failed_frame_parent_candidate(
        &self,
        world: &World,
        node: &FailedPreparedFrameNode<'_>,
    ) -> Result<Option<CleanupMountParentCandidate>, FailedManifestBuildError> {
        let (mount_slot, mount_claim, external_parent, external_parent_claim) =
            if let Some(slot_index) = node.incoming {
                let slot = self.slots[slot_index].slot;
                (
                    slot.slot(),
                    slot.slot_claim(),
                    slot.anchor(),
                    Some(slot.anchor_claim()),
                )
            } else {
                let external = node.external_mount.as_ref().ok_or(
                    FailedManifestBuildError::InvalidTopology(
                        FailedPreparedTopologyError::Rootless,
                    ),
                )?;
                (
                    external.slot,
                    external.slot_claim,
                    external.physical_parent,
                    external.physical_parent_claim,
                )
            };
        if self.boundary_drifted(mount_claim)
            || external_parent_claim.is_some_and(|claim| self.boundary_drifted(claim))
        {
            return Ok(None);
        }
        let frame = node.frame;
        let attached = world.get::<ChildOf>(frame.shell()).map(ChildOf::parent) == Some(mount_slot);
        let membership = world
            .get::<Children>(mount_slot)
            .map(|children| {
                children
                    .iter()
                    .filter(|child| **child == frame.shell())
                    .count()
            })
            .unwrap_or_default();
        let _stored_membership_is_consistent =
            (attached && membership == 1) || (!attached && membership == 0);
        let mut children_before = frame.prior_mount_children().to_vec();
        if attached {
            children_before.push(frame.shell());
        }
        Ok(Some(CleanupMountParentCandidate {
            claim: mount_claim.clone(),
            failed_claim_source: Some(FailedBoundarySource::FrameMountSlot {
                frame: frame.frame().clone(),
                removed_child: frame.shell(),
            }),
            supplied_parent: Some((external_parent, external_parent_claim.cloned())),
            failed_parent_source: external_parent_claim.map(|_| {
                FailedBoundarySource::FramePhysicalParent {
                    frame: frame.frame().clone(),
                    mount_slot,
                    removed_child: frame.shell(),
                }
            }),
            removed_child: frame.shell(),
            removed_child_was_attached: attached,
            children_before,
            children_after: frame.prior_mount_children().to_vec(),
        }))
    }

    fn boundary_drifted(&self, claim: &ExactRegistryClaim) -> bool {
        self.boundary_drifts
            .iter()
            .any(|drift| drift.expected.exact_claim() == claim)
    }

    fn append_frame_obligations(
        &self,
        world: &World,
        registry: &RouteEntityRegistry,
        frame_index: usize,
        frame_visit: &mut [u8],
        slot_visit: &mut [u8],
        obligations: &mut Vec<CleanupObligation>,
    ) -> Result<(), FailedManifestBuildError> {
        match frame_visit[frame_index] {
            0 => frame_visit[frame_index] = 1,
            1 | 2 => {
                return Err(FailedManifestBuildError::InvalidTopology(
                    FailedPreparedTopologyError::CycleOrRevisit,
                ));
            }
            _ => unreachable!(),
        }
        let node = &self.frames[frame_index];
        for slot_index in &node.outgoing {
            self.append_slot_obligations(
                world,
                registry,
                *slot_index,
                frame_visit,
                slot_visit,
                obligations,
            )?;
        }
        obligations.extend(failed_frame_obligations(world, registry, node.frame)?);
        frame_visit[frame_index] = 2;
        Ok(())
    }

    fn append_slot_obligations(
        &self,
        world: &World,
        registry: &RouteEntityRegistry,
        slot_index: usize,
        frame_visit: &mut [u8],
        slot_visit: &mut [u8],
        obligations: &mut Vec<CleanupObligation>,
    ) -> Result<(), FailedManifestBuildError> {
        match slot_visit[slot_index] {
            0 => slot_visit[slot_index] = 1,
            1 | 2 => {
                return Err(FailedManifestBuildError::InvalidTopology(
                    FailedPreparedTopologyError::CycleOrRevisit,
                ));
            }
            _ => unreachable!(),
        }
        let node = &self.slots[slot_index];
        if let Some(child) = node.child {
            self.append_frame_obligations(
                world,
                registry,
                child,
                frame_visit,
                slot_visit,
                obligations,
            )?;
        }
        obligations.push(failed_mounted_slot_obligation(world, registry, node.slot)?);
        slot_visit[slot_index] = 2;
        Ok(())
    }
}

fn failed_frame_node_cmp(
    left: &FailedPreparedFrameNode<'_>,
    right: &FailedPreparedFrameNode<'_>,
) -> std::cmp::Ordering {
    failed_frame_identity_cmp(left.frame.frame(), right.frame.frame())
        .then_with(|| {
            left.shell_claim
                .entity()
                .to_bits()
                .cmp(&right.shell_claim.entity().to_bits())
        })
        .then_with(|| {
            left.shell_claim
                .opaque_digest_part()
                .cmp(&right.shell_claim.opaque_digest_part())
        })
}

fn failed_slot_node_cmp(
    left: &FailedPreparedSlotNode<'_>,
    right: &FailedPreparedSlotNode<'_>,
) -> std::cmp::Ordering {
    failed_frame_identity_cmp(left.slot.parent(), right.slot.parent())
        .then_with(|| left.slot.outlet().cmp(right.slot.outlet()))
        .then_with(|| left.slot.slot_id().raw().cmp(&right.slot.slot_id().raw()))
        .then_with(|| left.slot.slot().to_bits().cmp(&right.slot.slot().to_bits()))
        .then_with(|| {
            left.slot
                .slot_claim()
                .opaque_digest_part()
                .cmp(&right.slot.slot_claim().opaque_digest_part())
        })
}

fn failed_auxiliary_slot_node_cmp(
    left: &FailedPreparedAuxiliarySlotNode<'_>,
    right: &FailedPreparedAuxiliarySlotNode<'_>,
) -> std::cmp::Ordering {
    failed_frame_identity_cmp(left.parent, right.parent)
        .then_with(|| left.outlet.cmp(right.outlet))
        .then_with(|| left.slot_id.raw().cmp(&right.slot_id.raw()))
        .then_with(|| left.entity.to_bits().cmp(&right.entity.to_bits()))
        .then_with(|| {
            left.boundary
                .as_ref()
                .map(|boundary| boundary.entity.to_bits())
                .cmp(
                    &right
                        .boundary
                        .as_ref()
                        .map(|boundary| boundary.entity.to_bits()),
                )
        })
        .then_with(|| {
            left.claim
                .opaque_digest_part()
                .cmp(&right.claim.opaque_digest_part())
        })
        .then_with(|| {
            left.boundary
                .as_ref()
                .map(|boundary| boundary.claim.opaque_digest_part())
                .cmp(
                    &right
                        .boundary
                        .as_ref()
                        .map(|boundary| boundary.claim.opaque_digest_part()),
                )
        })
}

fn failed_child_slot_owner_binding(
    claim: &ExactRegistryClaim,
    expected_parent: &RouteFrameVersion,
    expected_outlet: &RouteSlotKey,
    allow_provisional: bool,
) -> Result<RouteSlotId, FailedManifestBuildError> {
    let binding = match claim.owner() {
        RegistryOwner::Committed(EntityOwner::ChildSlot {
            parent,
            outlet,
            slot,
        }) => Some((parent, outlet, *slot)),
        RegistryOwner::Provisional(ProvisionalOwner::ChildSlot {
            parent,
            outlet,
            slot,
        }) if allow_provisional => Some((parent, outlet, *slot)),
        _ => None,
    };
    match binding {
        Some((parent, outlet, slot)) if parent == expected_parent && outlet == expected_outlet => {
            Ok(slot)
        }
        _ => Err(FailedManifestBuildError::InvalidTopology(
            FailedPreparedTopologyError::ConflictingOwner,
        )),
    }
}

fn failed_frame_identity_cmp(
    left: &RouteFrameVersion,
    right: &RouteFrameVersion,
) -> std::cmp::Ordering {
    left.router()
        .raw()
        .cmp(&right.router().raw())
        .then_with(|| left.host().raw().cmp(&right.host().raw()))
        .then_with(|| left.slot().raw().cmp(&right.slot().raw()))
        .then_with(|| failed_origin_key(left.origin()).cmp(&failed_origin_key(right.origin())))
        .then_with(|| left.frame().raw().cmp(&right.frame().raw()))
        .then_with(|| left.generation().raw().cmp(&right.generation().raw()))
}

fn failed_origin_key(origin: &FrameOrigin) -> (u8, u64) {
    match origin {
        FrameOrigin::MatchedRoute(route) => (0, route.raw()),
        FrameOrigin::RouteError(route) => (1, route.raw()),
        FrameOrigin::RouterFallback => (2, 0),
    }
}

fn validate_failed_primary_claim(
    primary_claims: &[&ExactRegistryClaim],
    claim: &ExactRegistryClaim,
) -> Result<(), FailedManifestBuildError> {
    if primary_claims
        .iter()
        .any(|current| current.entity() == claim.entity())
    {
        return Err(FailedManifestBuildError::InvalidTopology(
            FailedPreparedTopologyError::DuplicateEntity,
        ));
    }
    if primary_claims
        .iter()
        .any(|current| current.owner().conflicts_with(claim.owner()))
        || primary_claims
            .iter()
            .any(|current| current.seal() == claim.seal())
    {
        return Err(FailedManifestBuildError::InvalidTopology(
            FailedPreparedTopologyError::ConflictingOwner,
        ));
    }
    Ok(())
}

fn failed_slot_owner_is_exact(slot: &MountedChildSlot) -> bool {
    matches!(
        slot.slot_claim().owner(),
        RegistryOwner::Committed(EntityOwner::ChildSlot { parent, outlet, slot: slot_id })
            if parent == slot.parent()
                && outlet == slot.outlet()
                && *slot_id == slot.slot_id()
    )
}

fn observe_failed_boundary_claim(
    world: &World,
    registry: &RouteEntityRegistry,
    source: FailedBoundarySource,
    claim: &ExactRegistryClaim,
) -> Option<FailedBoundaryDrift> {
    if registry
        .record(claim.entity())
        .is_some_and(|record| record.claim() == *claim)
        && cleanup_claim_has_exact_authority(world, claim)
    {
        return None;
    }
    let state = if world.get_entity(claim.entity()).is_err() {
        FailedManifestClaimState::Absent
    } else {
        match registry.record(claim.entity()) {
            None => FailedManifestClaimState::Missing,
            Some(record) if record.claim() != *claim => FailedManifestClaimState::Replaced,
            Some(_) => FailedManifestClaimState::PhysicalDrift,
        }
    };
    Some(FailedBoundaryDrift {
        source,
        expected: FailedBoundaryExpectation::ExactClaim(claim.clone()),
        state,
        predicate: FailedMountParentFreezePredicate::ClaimAuthority,
    })
}

fn failed_claim_manifest_mode(
    world: &World,
    registry: &RouteEntityRegistry,
    claim: &ExactRegistryClaim,
    incomplete_physical_claim_is_allowed: bool,
) -> (ComponentManifest, CleanupTerminalMode, bool) {
    let record = registry.record(claim.entity());
    if world.get_entity(claim.entity()).is_err() {
        return (
            record
                .filter(|record| record.claim() == *claim)
                .map(|record| record.manifest().clone())
                .unwrap_or_else(|| ComponentManifest::new([])),
            CleanupTerminalMode::Normal,
            true,
        );
    }
    if let Some(record) = record.filter(|record| record.claim() == *claim)
        && (incomplete_physical_claim_is_allowed || cleanup_claim_has_exact_authority(world, claim))
    {
        return (
            record.manifest().clone(),
            CleanupTerminalMode::Normal,
            false,
        );
    }
    (
        record
            .filter(|record| record.claim() == *claim)
            .map(|record| record.manifest().clone())
            .or_else(|| ComponentManifest::from_world(world, claim.entity()))
            .unwrap_or_else(|| ComponentManifest::new([])),
        CleanupTerminalMode::AuthorityLost,
        false,
    )
}

fn failed_frame_obligations(
    world: &World,
    registry: &RouteEntityRegistry,
    frame: &RealizedFrame,
) -> Result<Vec<CleanupObligation>, FailedManifestBuildError> {
    let mut obligations = Vec::new();
    obligations
        .try_reserve_exact(frame.obligation_count())
        .map_err(|_| FailedManifestBuildError::RetryableCapacity)?;
    for (claim, expected_parent) in frame.cleanup_members() {
        let (manifest, terminal_mode, absent) =
            failed_claim_manifest_mode(world, registry, claim, !frame.prewrite_complete());
        obligations.push(CleanupObligation {
            claim: claim.clone(),
            manifest,
            expected_parent,
            unit_root: frame.shell(),
            authorized_parent: frame.top_slot(),
            allow_missing_components: absent || !frame.prewrite_complete(),
            terminal_mode,
        });
    }
    order_leaf_to_root(&mut obligations).ok_or(FailedManifestBuildError::InvalidTopology(
        FailedPreparedTopologyError::CycleOrRevisit,
    ))?;
    Ok(obligations)
}

fn failed_mounted_slot_obligation(
    world: &World,
    registry: &RouteEntityRegistry,
    slot: &MountedChildSlot,
) -> Result<CleanupObligation, FailedManifestBuildError> {
    let (manifest, terminal_mode, absent) =
        failed_claim_manifest_mode(world, registry, slot.slot_claim(), false);
    Ok(CleanupObligation {
        claim: slot.slot_claim().clone(),
        manifest,
        expected_parent: slot.anchor(),
        unit_root: slot.slot(),
        authorized_parent: slot.anchor(),
        allow_missing_components: absent,
        terminal_mode,
    })
}

fn failed_residual_slot_obligation(
    world: &World,
    registry: &RouteEntityRegistry,
    residual: &ChildSlotResidual,
) -> Result<CleanupObligation, FailedManifestBuildError> {
    let incomplete_physical_claim_is_allowed = matches!(
        residual.state(),
        crate::mount::ChildSlotResidualState::Provisional
            | crate::mount::ChildSlotResidualState::Absent
    );
    let (_, terminal_mode, absent) = failed_claim_manifest_mode(
        world,
        registry,
        residual.claim(),
        incomplete_physical_claim_is_allowed,
    );
    Ok(CleanupObligation {
        claim: residual.claim().clone(),
        manifest: residual.authorized_manifest().clone(),
        expected_parent: residual.anchor(),
        unit_root: residual.entity(),
        authorized_parent: residual.anchor(),
        allow_missing_components: absent || incomplete_physical_claim_is_allowed,
        terminal_mode,
    })
}

fn failed_rejected_slot_obligation(
    world: &World,
    registry: &RouteEntityRegistry,
    rejected: &RejectedChildSlot,
) -> Result<CleanupObligation, FailedManifestBuildError> {
    let slot = rejected.slot();
    let (manifest, mut terminal_mode, absent) =
        failed_claim_manifest_mode(world, registry, slot.slot_claim(), false);
    let canonical = rejected
        .canonical_anchor()
        .zip(rejected.canonical_anchor_claim());
    if terminal_mode == CleanupTerminalMode::Normal && canonical.is_none() {
        terminal_mode = CleanupTerminalMode::ForceUnresolved;
    }
    let expected_parent = canonical
        .map(|(anchor, _)| anchor)
        .unwrap_or_else(|| slot.anchor());
    Ok(CleanupObligation {
        claim: slot.slot_claim().clone(),
        manifest,
        expected_parent,
        unit_root: slot.slot(),
        authorized_parent: expected_parent,
        allow_missing_components: absent,
        terminal_mode,
    })
}

fn authority_host_obligation(
    world: &World,
    registry: &RouteEntityRegistry,
    host: &HostBinding,
) -> CleanupObligation {
    let record = registry.record(host.top_slot);
    let registry_exact = record.is_some_and(|record| record.claim() == host.top_slot_claim);
    let physical_exact =
        registry_exact && cleanup_claim_has_exact_authority(world, &host.top_slot_claim);
    let manifest = record
        .filter(|record| record.claim() == host.top_slot_claim)
        .map(|record| record.manifest().clone())
        .or_else(|| ComponentManifest::from_world(world, host.top_slot))
        .unwrap_or_else(|| ComponentManifest::new([]));
    CleanupObligation {
        claim: host.top_slot_claim.clone(),
        manifest,
        expected_parent: host.host,
        unit_root: host.top_slot,
        authorized_parent: host.host,
        allow_missing_components: false,
        terminal_mode: if physical_exact {
            CleanupTerminalMode::AuthorityExact
        } else {
            CleanupTerminalMode::AuthorityLost
        },
    }
}

fn normalize_authority_obligations(
    obligations: &mut Vec<CleanupObligation>,
) -> Result<(), AuthoritySettlementManifestError> {
    let mut normalized = Vec::new();
    normalized
        .try_reserve_exact(obligations.len())
        .map_err(|_| AuthoritySettlementManifestError::RetryableCapacity)?;
    for obligation in obligations.drain(..) {
        if let Some(existing) = normalized.iter().find(|existing: &&CleanupObligation| {
            existing.claim.entity() == obligation.claim.entity()
        }) {
            if existing != &obligation {
                return Err(AuthoritySettlementManifestError::InvalidTopology);
            }
            continue;
        }
        normalized.push(obligation);
    }
    *obligations = normalized;
    Ok(())
}

fn leaving_slot_has_exact_parent(frames: &[RealizedFrame], slot: &MountedChildSlot) -> bool {
    let mut parents = frames.iter().filter(|frame| frame.frame() == slot.parent());
    let Some(parent) = parents.next() else {
        return false;
    };
    parents.next().is_none()
        && parent.outlet(slot.outlet()).is_some_and(|outlet| {
            outlet.anchor() == slot.anchor()
                && outlet.anchor_claim() == slot.anchor_claim()
                && parent.claims().any(|claim| claim == slot.anchor_claim())
        })
        && slot.slot_claim().owner()
            == &RegistryOwner::Committed(EntityOwner::ChildSlot {
                parent: slot.parent().clone(),
                outlet: slot.outlet().clone(),
                slot: slot.slot_id(),
            })
}

fn append_exact_leaving_unit(
    registry: &RouteEntityRegistry,
    frames: &[RealizedFrame],
    child_slots: &[MountedChildSlot],
    projected_claims: &[ExactRegistryClaim],
    frame_index: usize,
    visit: &mut [u8],
    obligations: &mut Vec<CleanupObligation>,
) -> Option<()> {
    match visit.get(frame_index).copied()? {
        0 => visit[frame_index] = 1,
        1 => return None,
        2 => return Some(()),
        _ => return None,
    }
    let frame = frames.get(frame_index)?;
    for slot in child_slots
        .iter()
        .filter(|slot| slot.parent() == frame.frame())
    {
        let mut children = frames
            .iter()
            .enumerate()
            .filter(|(_, child)| child.mount_binding().matches_mounted_child_slot(slot));
        if let Some((child_index, _)) = children.next() {
            if children.next().is_some() {
                return None;
            }
            append_exact_leaving_unit(
                registry,
                frames,
                child_slots,
                projected_claims,
                child_index,
                visit,
                obligations,
            )?;
        }
        let projected = projected_claims
            .iter()
            .find(|claim| claim.entity() == slot.slot())?;
        obligations.push(mounted_slot_obligation_with_claim(
            registry, slot, projected,
        )?);
    }
    let frame_projected = frame
        .claims()
        .map(|current| {
            projected_claims
                .iter()
                .find(|claim| claim.entity() == current.entity())
                .cloned()
        })
        .collect::<Option<Vec<_>>>()?;
    obligations.extend(frame_obligations(registry, frame, &frame_projected)?);
    visit[frame_index] = 2;
    Some(())
}

fn append_exact_authority_branch(
    world: &World,
    registry: &RouteEntityRegistry,
    frames: &[RealizedFrame],
    child_slots: &[MountedChildSlot],
    obligations: &mut Vec<CleanupObligation>,
) -> Result<(), AuthoritySettlementManifestError> {
    obligations
        .try_reserve(
            child_slots.len()
                + frames
                    .iter()
                    .map(|frame| frame.cleanup_members().count())
                    .sum::<usize>(),
        )
        .map_err(|_| AuthoritySettlementManifestError::RetryableCapacity)?;
    let mut visit = Vec::new();
    visit
        .try_reserve_exact(frames.len())
        .map_err(|_| AuthoritySettlementManifestError::RetryableCapacity)?;
    visit.resize(frames.len(), 0u8);
    let mut incoming = Vec::new();
    incoming
        .try_reserve_exact(frames.len())
        .map_err(|_| AuthoritySettlementManifestError::RetryableCapacity)?;
    incoming.resize(frames.len(), None);

    for (slot_index, slot) in child_slots.iter().enumerate() {
        let mut children = frames
            .iter()
            .enumerate()
            .filter(|(_, frame)| frame.mount_binding().matches_mounted_child_slot(slot));
        if let Some((child_index, _)) = children.next() {
            if children.next().is_some() || incoming[child_index].replace(slot_index).is_some() {
                return Err(AuthoritySettlementManifestError::InvalidTopology);
            }
        }
    }

    for slot_index in child_slots.iter().enumerate().filter_map(|(index, slot)| {
        (!frames.iter().any(|frame| frame.frame() == slot.parent())).then_some(index)
    }) {
        append_exact_authority_slot(
            world,
            registry,
            frames,
            child_slots,
            slot_index,
            &mut visit,
            obligations,
        )?;
    }
    for root in incoming
        .iter()
        .enumerate()
        .filter_map(|(index, incoming)| incoming.is_none().then_some(index))
    {
        append_exact_authority_frame(
            world,
            registry,
            frames,
            child_slots,
            root,
            &mut visit,
            obligations,
        )?;
    }
    if visit.iter().all(|state| *state == 2) {
        Ok(())
    } else {
        Err(AuthoritySettlementManifestError::InvalidTopology)
    }
}

fn append_exact_authority_slot(
    world: &World,
    registry: &RouteEntityRegistry,
    frames: &[RealizedFrame],
    child_slots: &[MountedChildSlot],
    slot_index: usize,
    visit: &mut [u8],
    obligations: &mut Vec<CleanupObligation>,
) -> Result<(), AuthoritySettlementManifestError> {
    let slot = child_slots
        .get(slot_index)
        .ok_or(AuthoritySettlementManifestError::InvalidTopology)?;
    if !registry
        .record(slot.slot())
        .is_some_and(|record| record.claim() == *slot.slot_claim())
        || !cleanup_claim_has_exact_authority(world, slot.slot_claim())
    {
        for (index, frame) in frames.iter().enumerate() {
            if frame.mount_binding().matches_mounted_child_slot(slot) {
                append_authority_lost_frame_subtree(
                    world,
                    registry,
                    frames,
                    child_slots,
                    index,
                    visit,
                    obligations,
                )?;
            }
        }
        obligations.push(authority_settlement_slot_obligation(world, registry, slot));
        return Ok(());
    }
    if let Some((child_index, _)) = frames
        .iter()
        .enumerate()
        .find(|(_, frame)| frame.mount_binding().matches_mounted_child_slot(slot))
    {
        append_exact_authority_frame(
            world,
            registry,
            frames,
            child_slots,
            child_index,
            visit,
            obligations,
        )?;
    }
    obligations.push(authority_settlement_slot_obligation(world, registry, slot));
    Ok(())
}

fn append_exact_authority_frame(
    world: &World,
    registry: &RouteEntityRegistry,
    frames: &[RealizedFrame],
    child_slots: &[MountedChildSlot],
    frame_index: usize,
    visit: &mut [u8],
    obligations: &mut Vec<CleanupObligation>,
) -> Result<(), AuthoritySettlementManifestError> {
    match *visit
        .get(frame_index)
        .ok_or(AuthoritySettlementManifestError::InvalidTopology)?
    {
        0 => {}
        1 => return Err(AuthoritySettlementManifestError::InvalidTopology),
        2 => return Err(AuthoritySettlementManifestError::InvalidTopology),
        _ => return Err(AuthoritySettlementManifestError::InvalidTopology),
    }
    let frame = frames
        .get(frame_index)
        .ok_or(AuthoritySettlementManifestError::InvalidTopology)?;
    let frame_unit_is_exact = frame.claims().all(|claim| {
        registry
            .record(claim.entity())
            .is_some_and(|record| record.claim() == *claim)
            && cleanup_claim_has_exact_authority(world, claim)
    });
    if !frame_unit_is_exact {
        return append_authority_lost_frame_subtree(
            world,
            registry,
            frames,
            child_slots,
            frame_index,
            visit,
            obligations,
        );
    }
    visit[frame_index] = 1;
    for (slot_index, _) in child_slots
        .iter()
        .enumerate()
        .filter(|(_, slot)| slot.parent() == frame.frame())
    {
        append_exact_authority_slot(
            world,
            registry,
            frames,
            child_slots,
            slot_index,
            visit,
            obligations,
        )?;
    }
    obligations.extend(
        current_exact_frame_obligations(world, registry, frame)
            .ok_or(AuthoritySettlementManifestError::InvalidTopology)?,
    );
    visit[frame_index] = 2;
    Ok(())
}

fn append_authority_lost_frame_subtree(
    world: &World,
    registry: &RouteEntityRegistry,
    frames: &[RealizedFrame],
    child_slots: &[MountedChildSlot],
    frame_index: usize,
    visit: &mut [u8],
    obligations: &mut Vec<CleanupObligation>,
) -> Result<(), AuthoritySettlementManifestError> {
    match visit
        .get(frame_index)
        .copied()
        .ok_or(AuthoritySettlementManifestError::InvalidTopology)?
    {
        1 | 2 => return Err(AuthoritySettlementManifestError::InvalidTopology),
        0 => visit[frame_index] = 1,
        _ => return Err(AuthoritySettlementManifestError::InvalidTopology),
    }
    let frame = frames
        .get(frame_index)
        .ok_or(AuthoritySettlementManifestError::InvalidTopology)?;
    for slot in child_slots
        .iter()
        .filter(|slot| slot.parent() == frame.frame())
    {
        if let Some((child_index, _)) = frames
            .iter()
            .enumerate()
            .find(|(_, child)| child.mount_binding().matches_mounted_child_slot(slot))
        {
            append_authority_lost_frame_subtree(
                world,
                registry,
                frames,
                child_slots,
                child_index,
                visit,
                obligations,
            )?;
        }
        obligations.push(authority_settlement_slot_obligation(world, registry, slot));
    }
    obligations.extend(
        authority_lost_frame_obligations(world, registry, frame)
            .ok_or(AuthoritySettlementManifestError::InvalidTopology)?,
    );
    visit[frame_index] = 2;
    Ok(())
}

fn authority_settlement_slot_obligation(
    world: &World,
    registry: &RouteEntityRegistry,
    slot: &MountedChildSlot,
) -> CleanupObligation {
    let record = registry.record(slot.slot());
    let registry_exact = record.is_some_and(|record| record.claim() == *slot.slot_claim());
    let authority_lost =
        !registry_exact || !cleanup_claim_has_exact_authority(world, slot.slot_claim());
    let manifest = record
        .filter(|record| record.claim() == *slot.slot_claim())
        .map(|record| record.manifest().clone())
        .or_else(|| ComponentManifest::from_world(world, slot.slot()))
        .unwrap_or_else(|| ComponentManifest::new([]));
    CleanupObligation {
        claim: slot.slot_claim().clone(),
        manifest,
        expected_parent: slot.anchor(),
        unit_root: slot.slot(),
        authorized_parent: slot.anchor(),
        allow_missing_components: false,
        terminal_mode: if authority_lost {
            CleanupTerminalMode::AuthorityLost
        } else {
            CleanupTerminalMode::AuthorityExact
        },
    }
}

fn authority_lost_frame_obligations(
    world: &World,
    registry: &RouteEntityRegistry,
    frame: &RealizedFrame,
) -> Option<Vec<CleanupObligation>> {
    let mut obligations = frame
        .cleanup_members()
        .map(|(claim, expected_parent)| {
            let record = registry.record(claim.entity());
            let registry_exact = record.is_some_and(|record| record.claim() == *claim);
            let manifest = record
                .filter(|record| record.claim() == *claim)
                .map(|record| record.manifest().clone())
                .or_else(|| ComponentManifest::from_world(world, claim.entity()))
                .unwrap_or_else(|| ComponentManifest::new([]));
            CleanupObligation {
                claim: claim.clone(),
                manifest,
                expected_parent,
                unit_root: frame.shell(),
                authorized_parent: frame.top_slot(),
                allow_missing_components: !frame.prewrite_complete(),
                terminal_mode: if registry_exact && cleanup_claim_has_exact_authority(world, claim)
                {
                    CleanupTerminalMode::AuthorityExact
                } else {
                    CleanupTerminalMode::AuthorityLost
                },
            }
        })
        .collect::<Vec<_>>();
    order_leaf_to_root(&mut obligations)?;
    Some(obligations)
}

fn current_exact_frame_obligations(
    world: &World,
    registry: &RouteEntityRegistry,
    frame: &RealizedFrame,
) -> Option<Vec<CleanupObligation>> {
    let mut obligations = frame
        .cleanup_members()
        .map(|(claim, expected_parent)| {
            let record = registry.record(claim.entity())?;
            if record.claim() != *claim || !cleanup_claim_has_exact_authority(world, claim) {
                return None;
            }
            Some(CleanupObligation {
                claim: claim.clone(),
                manifest: record.manifest().clone(),
                expected_parent,
                unit_root: frame.shell(),
                authorized_parent: frame.top_slot(),
                allow_missing_components: !frame.prewrite_complete(),
                terminal_mode: CleanupTerminalMode::AuthorityExact,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    order_leaf_to_root(&mut obligations)?;
    Some(obligations)
}

fn exact_successor_for_mount<'a>(
    frame: &RealizedFrame,
    successor: Option<&'a RealizedBranch>,
) -> Option<Option<&'a RealizedFrame>> {
    let mut matches = successor
        .into_iter()
        .flat_map(RealizedBranch::frames)
        .filter(|candidate| {
            candidate
                .mount_binding()
                .same_physical_mount(frame.mount_binding())
        });
    let successor = matches.next();
    matches.next().is_none().then_some(successor)
}

fn frame_obligations(
    registry: &RouteEntityRegistry,
    frame: &RealizedFrame,
    projected_claims: &[ExactRegistryClaim],
) -> Option<Vec<CleanupObligation>> {
    if projected_claims.len() != frame.obligation_count() {
        return None;
    }
    let mut obligations = frame
        .cleanup_members()
        .map(|(current, expected_parent)| {
            let projected = projected_claims
                .iter()
                .find(|claim| claim.entity() == current.entity())?;
            let record = registry.record(current.entity())?;
            if projected.owner() != current.owner()
                || projected.seal() != current.seal()
                || record.claim() != *current
            {
                return None;
            }
            Some(CleanupObligation {
                claim: projected.clone(),
                manifest: record.manifest().clone(),
                expected_parent,
                unit_root: frame.shell(),
                authorized_parent: frame.top_slot(),
                allow_missing_components: !frame.prewrite_complete(),
                terminal_mode: CleanupTerminalMode::Normal,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    order_leaf_to_root(&mut obligations)?;
    Some(obligations)
}

fn replacement_mount_children(
    leaving_prior: &[Entity],
    leaving: Entity,
    entering_prior: &[Entity],
    entering: Entity,
) -> Option<(Vec<Entity>, Vec<Entity>)> {
    let mut expected_entering_prior = leaving_prior.to_vec();
    expected_entering_prior.push(leaving);
    if entering_prior != expected_entering_prior {
        return None;
    }
    let mut children_before = entering_prior.to_vec();
    children_before.push(entering);
    let mut children_after = leaving_prior.to_vec();
    children_after.push(entering);
    Some((children_before, children_after))
}

fn frame_mount_parent_candidate(
    frame: &RealizedFrame,
    successor: Option<&RealizedFrame>,
) -> Option<CleanupMountParentCandidate> {
    let (children_before, children_after) = if let Some(successor) = successor {
        if !frame.shell_is_attached()
            || !successor.shell_is_attached()
            || !successor
                .mount_binding()
                .same_physical_mount(frame.mount_binding())
        {
            return None;
        }
        replacement_mount_children(
            frame.prior_mount_children(),
            frame.shell(),
            successor.prior_mount_children(),
            successor.shell(),
        )?
    } else if frame.shell_is_attached() {
        let mut children_before = frame.prior_mount_children().to_vec();
        children_before.push(frame.shell());
        (children_before, frame.prior_mount_children().to_vec())
    } else {
        (
            frame.prior_mount_children().to_vec(),
            frame.prior_mount_children().to_vec(),
        )
    };
    let (expected_parent, expected_parent_claim) = frame.mount_binding().physical_parent();
    Some(CleanupMountParentCandidate {
        claim: frame.mount_binding().slot_claim().clone(),
        failed_claim_source: None,
        supplied_parent: Some((expected_parent, expected_parent_claim.cloned())),
        failed_parent_source: None,
        removed_child: frame.shell(),
        removed_child_was_attached: frame.shell_is_attached(),
        children_before,
        children_after,
    })
}

fn mounted_anchor_parent_candidate(slot: &MountedChildSlot) -> Option<CleanupMountParentCandidate> {
    let children_before = slot.expected_anchor_children().to_vec();
    let children_after = children_before.strip_suffix(&[slot.slot()])?.to_vec();
    Some(CleanupMountParentCandidate {
        claim: slot.anchor_claim().clone(),
        failed_claim_source: None,
        supplied_parent: None,
        failed_parent_source: None,
        removed_child: slot.slot(),
        removed_child_was_attached: true,
        children_before,
        children_after,
    })
}

fn residual_anchor_parent_candidate(
    world: &World,
    slot: &ChildSlotResidual,
) -> CleanupMountParentCandidate {
    let mut children_before = slot.prior_anchor_children().to_vec();
    let removed_child_was_attached =
        world.get::<ChildOf>(slot.entity()).map(ChildOf::parent) == Some(slot.anchor());
    if removed_child_was_attached {
        children_before.push(slot.entity());
    }
    CleanupMountParentCandidate {
        claim: slot.anchor_claim().clone(),
        failed_claim_source: None,
        supplied_parent: None,
        failed_parent_source: None,
        removed_child: slot.entity(),
        removed_child_was_attached,
        children_before,
        children_after: slot.prior_anchor_children().to_vec(),
    }
}

fn rejected_canonical_parent_candidate(
    world: &World,
    rejected: &RejectedChildSlot,
) -> Result<Option<CleanupMountParentCandidate>, FailedManifestBuildError> {
    let (Some(anchor), Some(claim), Some(children_after)) = (
        rejected.canonical_anchor(),
        rejected.canonical_anchor_claim(),
        rejected.canonical_anchor_children(),
    ) else {
        return Ok(None);
    };
    let slot = rejected.slot().slot();
    let attached = world.get::<ChildOf>(slot).map(ChildOf::parent) == Some(anchor);
    let membership = world
        .get::<Children>(anchor)
        .map(|children| children.iter().filter(|child| **child == slot).count())
        .unwrap_or_default();
    let _stored_membership_is_consistent =
        (attached && membership == 1) || (!attached && membership == 0);
    let mut children_before = children_after.to_vec();
    if attached {
        children_before.push(slot);
    }
    Ok(Some(CleanupMountParentCandidate {
        claim: claim.clone(),
        failed_claim_source: None,
        supplied_parent: None,
        failed_parent_source: None,
        removed_child: slot,
        removed_child_was_attached: attached,
        children_before,
        children_after: children_after.to_vec(),
    }))
}

fn freeze_external_mount_parents(
    world: &World,
    registry: &RouteEntityRegistry,
    obligations: &[CleanupObligation],
    candidates: impl IntoIterator<Item = CleanupMountParentCandidate>,
) -> Option<Vec<CleanupMountParentSettlement>> {
    freeze_external_mount_parents_detailed(world, registry, obligations, candidates)
        .ok()
        .map(|(parents, _)| parents)
}

fn push_failed_candidate_drift(
    drifts: &mut Vec<FailedBoundaryDrift>,
    source: Option<&FailedBoundarySource>,
    claim: &ExactRegistryClaim,
    world: &World,
    registry: &RouteEntityRegistry,
    predicate: FailedMountParentFreezePredicate,
) -> bool {
    let Some(source) = source else {
        return false;
    };
    let mut drift = observe_failed_boundary_claim(world, registry, source.clone(), claim)
        .unwrap_or(FailedBoundaryDrift {
            source: source.clone(),
            expected: FailedBoundaryExpectation::ExactClaim(claim.clone()),
            state: failed_manifest_claim_state(world, registry, claim),
            predicate,
        });
    drift.predicate = predicate;
    if !drifts.contains(&drift) {
        drifts.push(drift);
    }
    true
}

fn failed_manifest_claim_state(
    world: &World,
    registry: &RouteEntityRegistry,
    claim: &ExactRegistryClaim,
) -> FailedManifestClaimState {
    if world.get_entity(claim.entity()).is_err() {
        FailedManifestClaimState::Absent
    } else {
        match registry.record(claim.entity()) {
            None => FailedManifestClaimState::Missing,
            Some(record) if record.claim() != *claim => FailedManifestClaimState::Replaced,
            Some(_) if cleanup_claim_has_exact_authority(world, claim) => {
                FailedManifestClaimState::Exact
            }
            Some(_) => FailedManifestClaimState::PhysicalDrift,
        }
    }
}

fn failed_mount_parent_freeze_error(
    source: Option<FailedBoundarySource>,
    claim: ExactRegistryClaim,
    world: &World,
    registry: &RouteEntityRegistry,
    predicate: FailedMountParentFreezePredicate,
) -> FailedMountParentFreezeError {
    FailedMountParentFreezeError {
        source,
        state: failed_manifest_claim_state(world, registry, &claim),
        expected: FailedBoundaryExpectation::ExactClaim(claim),
        predicate,
    }
}

fn freeze_external_mount_parents_detailed(
    world: &World,
    registry: &RouteEntityRegistry,
    obligations: &[CleanupObligation],
    candidates: impl IntoIterator<Item = CleanupMountParentCandidate>,
) -> Result<
    (Vec<CleanupMountParentSettlement>, Vec<FailedBoundaryDrift>),
    FailedMountParentFreezeError,
> {
    let obligation_entities = obligations
        .iter()
        .map(|obligation| obligation.claim.entity())
        .collect::<HashSet<_>>();
    let mut parents = Vec::<CleanupMountParentSettlement>::new();
    let mut drifts = Vec::new();
    for mut candidate in candidates {
        let failed_claim_source = candidate.failed_claim_source.clone();
        let failed_parent_source = candidate.failed_parent_source.clone();
        let supplied_parent_claim = candidate
            .supplied_parent
            .as_ref()
            .and_then(|(_, claim)| claim.as_ref())
            .cloned();
        let claim = candidate.claim;
        let entity = claim.entity();
        if obligation_entities.contains(&entity) {
            continue;
        }
        let claim_drifted = failed_claim_source.as_ref().is_some_and(|source| {
            observe_failed_boundary_claim(world, registry, source.clone(), &claim).is_some_and(
                |drift| {
                    if !drifts.contains(&drift) {
                        drifts.push(drift);
                    }
                    true
                },
            )
        });
        let parent_drifted = supplied_parent_claim.as_ref().is_some_and(|parent_claim| {
            failed_parent_source.as_ref().is_some_and(|source| {
                observe_failed_boundary_claim(world, registry, source.clone(), parent_claim)
                    .is_some_and(|drift| {
                        if !drifts.contains(&drift) {
                            drifts.push(drift);
                        }
                        true
                    })
            })
        });
        if claim_drifted || parent_drifted {
            continue;
        }
        if !matches!(
            claim.owner(),
            RegistryOwner::Committed(
                EntityOwner::HostTopSlot { .. }
                    | EntityOwner::ChildSlot { .. }
                    | EntityOwner::FrameOutput { .. }
            )
        ) {
            if push_failed_candidate_drift(
                &mut drifts,
                failed_claim_source.as_ref(),
                &claim,
                world,
                registry,
                FailedMountParentFreezePredicate::UnsupportedOwner,
            ) {
                continue;
            }
            return Err(failed_mount_parent_freeze_error(
                failed_claim_source,
                claim,
                world,
                registry,
                FailedMountParentFreezePredicate::UnsupportedOwner,
            ));
        }
        let actual_children = world
            .get::<Children>(entity)
            .map(|children| children.iter().copied().collect::<Vec<_>>())
            .unwrap_or_default();
        let expected_children = candidate
            .children_before
            .iter()
            .copied()
            .collect::<HashSet<_>>();
        if actual_children
            .iter()
            .copied()
            .filter(|child| expected_children.contains(child))
            .ne(candidate.children_before.iter().copied())
            || actual_children.iter().copied().any(|child| {
                !expected_children.contains(&child)
                    && (registry.record(child).is_some()
                        || world
                            .resource::<QuarantineLeaseTable>()
                            .record(child)
                            .is_some()
                        || world
                            .resource::<QuarantineLeaseTable>()
                            .unresolved_record(child)
                            .is_some())
            })
        {
            if push_failed_candidate_drift(
                &mut drifts,
                failed_claim_source.as_ref(),
                &claim,
                world,
                registry,
                FailedMountParentFreezePredicate::ChildrenProjection,
            ) {
                continue;
            }
            return Err(failed_mount_parent_freeze_error(
                failed_claim_source,
                claim,
                world,
                registry,
                FailedMountParentFreezePredicate::ChildrenProjection,
            ));
        }
        candidate.children_before = actual_children.clone();
        candidate.children_after = if candidate.removed_child_was_attached {
            let mut removed = false;
            actual_children
                .into_iter()
                .filter(|child| {
                    if !removed && *child == candidate.removed_child {
                        removed = true;
                        false
                    } else {
                        true
                    }
                })
                .collect::<Vec<_>>()
        } else {
            actual_children
        };
        if let Some(existing) = parents
            .iter()
            .find(|parent| parent.claim.entity() == entity)
        {
            if existing.claim != claim
                || existing.removed_child != candidate.removed_child
                || existing.removed_child_was_attached != candidate.removed_child_was_attached
                || existing.children_before != candidate.children_before
                || existing.children_after != candidate.children_after
            {
                if push_failed_candidate_drift(
                    &mut drifts,
                    failed_claim_source.as_ref(),
                    &claim,
                    world,
                    registry,
                    FailedMountParentFreezePredicate::ConflictingCandidate,
                ) {
                    continue;
                }
                return Err(failed_mount_parent_freeze_error(
                    failed_claim_source,
                    claim,
                    world,
                    registry,
                    FailedMountParentFreezePredicate::ConflictingCandidate,
                ));
            }
            continue;
        }
        let Some(record) = registry.record(entity) else {
            if push_failed_candidate_drift(
                &mut drifts,
                failed_claim_source.as_ref(),
                &claim,
                world,
                registry,
                FailedMountParentFreezePredicate::MissingRegistryRecord,
            ) {
                continue;
            }
            return Err(failed_mount_parent_freeze_error(
                failed_claim_source,
                claim,
                world,
                registry,
                FailedMountParentFreezePredicate::MissingRegistryRecord,
            ));
        };
        let Some(physical_manifest) = ComponentManifest::from_world(world, entity) else {
            if push_failed_candidate_drift(
                &mut drifts,
                failed_claim_source.as_ref(),
                &claim,
                world,
                registry,
                FailedMountParentFreezePredicate::MissingPhysicalManifest,
            ) {
                continue;
            }
            return Err(failed_mount_parent_freeze_error(
                failed_claim_source,
                claim,
                world,
                registry,
                FailedMountParentFreezePredicate::MissingPhysicalManifest,
            ));
        };
        if record.claim() != claim
            || !manifest_contains_required_allowing_children(
                world,
                &physical_manifest,
                record.manifest(),
            )
        {
            if push_failed_candidate_drift(
                &mut drifts,
                failed_claim_source.as_ref(),
                &claim,
                world,
                registry,
                FailedMountParentFreezePredicate::RegistryManifestMismatch,
            ) {
                continue;
            }
            return Err(failed_mount_parent_freeze_error(
                failed_claim_source,
                claim,
                world,
                registry,
                FailedMountParentFreezePredicate::RegistryManifestMismatch,
            ));
        }
        let Some((expected_parent, expected_parent_claim)) =
            resolve_mount_parent_authority(registry, &claim, candidate.supplied_parent)
        else {
            let (source, failed_claim) = supplied_parent_claim
                .as_ref()
                .filter(|_| failed_parent_source.is_some())
                .map(|claim| (failed_parent_source.as_ref(), claim))
                .unwrap_or((failed_claim_source.as_ref(), &claim));
            if push_failed_candidate_drift(
                &mut drifts,
                source,
                failed_claim,
                world,
                registry,
                FailedMountParentFreezePredicate::ParentAuthority,
            ) {
                continue;
            }
            return Err(failed_mount_parent_freeze_error(
                source.cloned(),
                failed_claim.clone(),
                world,
                registry,
                FailedMountParentFreezePredicate::ParentAuthority,
            ));
        };
        let settlement = CleanupMountParentSettlement {
            claim,
            manifest: record.manifest().clone(),
            physical_manifest,
            expected_parent,
            expected_parent_claim,
            removed_child: candidate.removed_child,
            removed_child_was_attached: candidate.removed_child_was_attached,
            children_before: candidate.children_before,
            children_after: candidate.children_after,
        };
        if !mount_parent_physical_matches(world, registry, &settlement) {
            if push_failed_candidate_drift(
                &mut drifts,
                failed_claim_source.as_ref(),
                &settlement.claim,
                world,
                registry,
                FailedMountParentFreezePredicate::PhysicalAuthority,
            ) {
                continue;
            }
            return Err(failed_mount_parent_freeze_error(
                failed_claim_source,
                settlement.claim,
                world,
                registry,
                FailedMountParentFreezePredicate::PhysicalAuthority,
            ));
        }
        parents.push(settlement);
    }
    parents.sort_by_key(|parent| parent.claim.entity().to_bits());
    Ok((parents, drifts))
}

fn rebind_external_mount_parent_claims(
    parents: &mut [CleanupMountParentSettlement],
    projected_claims: &[ExactRegistryClaim],
) -> Option<()> {
    for parent in parents {
        if let Some(projected) = projected_claims
            .iter()
            .find(|claim| claim.entity() == parent.claim.entity())
        {
            if projected.owner() != parent.claim.owner()
                || projected.seal() != parent.claim.seal()
                || matches!(
                    projected.lifecycle(),
                    EntityLifecycle::Leaving | EntityLifecycle::Quarantined
                )
            {
                return None;
            }
            parent.claim = projected.clone();
        }
        if let Some(expected_parent_claim) = &mut parent.expected_parent_claim
            && let Some(projected) = projected_claims
                .iter()
                .find(|claim| claim.entity() == expected_parent_claim.entity())
        {
            if projected.owner() != expected_parent_claim.owner()
                || projected.seal() != expected_parent_claim.seal()
                || matches!(
                    projected.lifecycle(),
                    EntityLifecycle::Leaving | EntityLifecycle::Quarantined
                )
            {
                return None;
            }
            *expected_parent_claim = projected.clone();
        }
    }
    Some(())
}

fn resolve_mount_parent_authority(
    registry: &RouteEntityRegistry,
    claim: &ExactRegistryClaim,
    supplied: Option<(Entity, Option<ExactRegistryClaim>)>,
) -> Option<(Entity, Option<ExactRegistryClaim>)> {
    match claim.owner() {
        RegistryOwner::Committed(EntityOwner::HostTopSlot { .. }) => {
            let (parent, None) = supplied? else {
                return None;
            };
            Some((parent, None))
        }
        RegistryOwner::Committed(EntityOwner::ChildSlot { parent, .. }) => {
            let (entity, Some(parent_claim)) = supplied? else {
                return None;
            };
            matches!(
                parent_claim.owner(),
                RegistryOwner::Committed(EntityOwner::FrameOutput { frame, .. }) if frame == parent
            )
            .then_some((entity, Some(parent_claim)))
        }
        RegistryOwner::Committed(EntityOwner::FrameOutput { frame, .. }) => {
            if supplied.is_some() {
                return None;
            }
            let mut matches = registry.records().iter().filter(|record| {
                matches!(
                    record.owner(),
                    RegistryOwner::Committed(EntityOwner::FrameContentTarget(candidate))
                        if candidate == frame
                )
            });
            let record = matches.next()?;
            if matches.next().is_some() {
                return None;
            }
            Some((record.entity(), Some(record.claim())))
        }
        _ => None,
    }
}

fn mount_parent_physical_matches(
    world: &World,
    registry: &RouteEntityRegistry,
    parent: &CleanupMountParentSettlement,
) -> bool {
    let Some(record) = registry.record(parent.claim.entity()) else {
        return false;
    };
    if record.claim() != parent.claim
        || world.get::<RouteOwnershipSeal>(parent.claim.entity())
            != Some(&RouteOwnershipSeal::new(parent.claim.seal()))
        || world
            .get::<ChildOf>(parent.claim.entity())
            .map(ChildOf::parent)
            != Some(parent.expected_parent)
    {
        return false;
    }
    if !observed_parent_manifest_differs_only_by_children(
        world,
        parent.claim.entity(),
        &parent.physical_manifest,
    ) {
        return false;
    }
    let owner_value_is_exact = match parent.claim.owner() {
        RegistryOwner::Committed(EntityOwner::HostTopSlot { .. }) => {
            record.lifecycle() == EntityLifecycle::PreparedInert
                && world.get::<ProvisionalSlotLifecycle>(parent.claim.entity())
                    == Some(&ProvisionalSlotLifecycle::PreparedInert)
                && mount_slot_values_match(world, parent.claim.entity(), Visibility::Visible)
        }
        RegistryOwner::Committed(EntityOwner::ChildSlot {
            parent: frame,
            outlet,
            slot,
        }) => {
            let (expected_lifecycle, visibility) = match record.lifecycle() {
                EntityLifecycle::PreparedInert => {
                    (ChildSlotLifecycle::RegisteredInert, Visibility::Hidden)
                }
                EntityLifecycle::Active => (ChildSlotLifecycle::Active, Visibility::Visible),
                EntityLifecycle::Leaving => (ChildSlotLifecycle::Leaving, Visibility::Hidden),
                EntityLifecycle::Quarantined => return false,
            };
            world.get::<ChildSlotLifecycle>(parent.claim.entity()) == Some(&expected_lifecycle)
                && world
                    .get::<ChildSlotAuthority>(parent.claim.entity())
                    .is_some_and(|authority| {
                        authority.parent() == frame
                            && authority.outlet() == outlet
                            && authority.slot() == *slot
                    })
                && mount_slot_values_match(world, parent.claim.entity(), visibility)
        }
        RegistryOwner::Committed(EntityOwner::FrameOutput {
            frame, local_index, ..
        }) => {
            world.get::<FrameLifecycle>(parent.claim.entity())
                == Some(&FrameLifecycle(parent.claim.lifecycle()))
                && world
                    .get::<FrameOutput>(parent.claim.entity())
                    .is_some_and(|output| output.matches_owner(frame, *local_index))
                && world
                    .get::<FrameOutput>(parent.claim.entity())
                    .is_some_and(|output| {
                        output.canonical_values_match(
                            world,
                            parent.claim.entity(),
                            record.lifecycle(),
                        )
                    })
        }
        _ => false,
    };
    if !owner_value_is_exact {
        return false;
    }
    if let Some(expected_parent_claim) = &parent.expected_parent_claim {
        let Some(parent_record) = registry.record(parent.expected_parent) else {
            return false;
        };
        if parent_record.claim() != *expected_parent_claim
            || world.get::<RouteOwnershipSeal>(parent.expected_parent)
                != Some(&RouteOwnershipSeal::new(expected_parent_claim.seal()))
            || world.get::<FrameLifecycle>(parent.expected_parent)
                != Some(&FrameLifecycle(expected_parent_claim.lifecycle()))
        {
            return false;
        }
        let exact_parent_kind = match (parent.claim.owner(), expected_parent_claim.owner()) {
            (
                RegistryOwner::Committed(EntityOwner::ChildSlot { parent: frame, .. }),
                RegistryOwner::Committed(EntityOwner::FrameOutput {
                    frame: owner_frame,
                    local_index,
                    ..
                }),
            ) => {
                frame == owner_frame
                    && world
                        .get::<FrameOutput>(parent.expected_parent)
                        .is_some_and(|output| output.matches_owner(owner_frame, *local_index))
            }
            (
                RegistryOwner::Committed(EntityOwner::FrameOutput { frame, .. }),
                RegistryOwner::Committed(EntityOwner::FrameContentTarget(owner_frame)),
            ) => {
                frame == owner_frame
                    && world
                        .get::<FrameContentTarget>(parent.expected_parent)
                        .is_some_and(|target| target.matches_frame(owner_frame))
            }
            _ => false,
        };
        if !exact_parent_kind {
            return false;
        }
    }
    let expected_children =
        if parent.removed_child_was_attached && world.get_entity(parent.removed_child).is_ok() {
            &parent.children_before
        } else {
            &parent.children_after
        };
    world
        .get::<Children>(parent.claim.entity())
        .map(|children| children.iter().copied().collect::<Vec<_>>())
        .unwrap_or_default()
        == *expected_children
}

fn observed_parent_manifest_differs_only_by_children(
    world: &World,
    entity: Entity,
    frozen: &ComponentManifest,
) -> bool {
    let Some(actual) = ComponentManifest::from_world(world, entity) else {
        return false;
    };
    let children = world.components().component_id::<Children>();
    actual
        .component_ids()
        .iter()
        .filter(|component| !frozen.component_ids().contains(component))
        .chain(
            frozen
                .component_ids()
                .iter()
                .filter(|component| !actual.component_ids().contains(component)),
        )
        .all(|component| Some(*component) == children)
}

fn observed_manifest_differs_only_by_allowed_relationships(
    world: &World,
    entity: Entity,
    frozen: &ComponentManifest,
    allow_children: bool,
    allow_child_of: bool,
) -> bool {
    let Some(actual) = ComponentManifest::from_world(world, entity) else {
        return false;
    };
    let children = world.components().component_id::<Children>();
    let child_of = world.components().component_id::<ChildOf>();
    actual
        .component_ids()
        .iter()
        .filter(|component| !frozen.component_ids().contains(component))
        .chain(
            frozen
                .component_ids()
                .iter()
                .filter(|component| !actual.component_ids().contains(component)),
        )
        .all(|component| {
            (allow_children && Some(*component) == children)
                || (allow_child_of && Some(*component) == child_of)
        })
}

fn manifest_contains_required_allowing_children(
    world: &World,
    actual: &ComponentManifest,
    required: &ComponentManifest,
) -> bool {
    let children = world.components().component_id::<Children>();
    required
        .component_ids()
        .iter()
        .all(|component| Some(*component) == children || actual.component_ids().contains(component))
}

fn mount_parent_frozen_manifest_matches(
    registry: &RouteEntityRegistry,
    parent: &CleanupMountParentSettlement,
) -> bool {
    registry
        .record(parent.claim.entity())
        .is_some_and(|record| record.manifest() == &parent.manifest)
}

fn mount_slot_values_match(world: &World, entity: Entity, visibility: Visibility) -> bool {
    world.get::<Node>(entity)
        == Some(&Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..Node::default()
        })
        && world.get::<Visibility>(entity) == Some(&visibility)
        && world.get::<Pickable>(entity) == Some(&Pickable::IGNORE)
        && world
            .get::<RouterInputBarrier>(entity)
            .is_some_and(RouterInputBarrier::denies_all)
}

fn mounted_slot_obligation(
    _world: &World,
    registry: &RouteEntityRegistry,
    slot: &MountedChildSlot,
) -> Option<CleanupObligation> {
    mounted_slot_obligation_with_claim(registry, slot, slot.slot_claim())
}

fn mounted_slot_obligation_with_claim(
    registry: &RouteEntityRegistry,
    slot: &MountedChildSlot,
    projected: &ExactRegistryClaim,
) -> Option<CleanupObligation> {
    let record = registry.record(slot.slot())?;
    let anchor_record = registry.record(slot.anchor())?;
    if record.claim() != *slot.slot_claim()
        || anchor_record.claim() != *slot.anchor_claim()
        || projected.entity() != slot.slot()
        || projected.owner() != slot.slot_claim().owner()
        || projected.seal() != slot.slot_claim().seal()
        || !matches!(
            slot.slot_claim().owner(),
            RegistryOwner::Committed(EntityOwner::ChildSlot { parent, outlet, slot: slot_id })
                if parent == slot.parent()
                    && outlet == slot.outlet()
                    && *slot_id == slot.slot_id()
        )
    {
        return None;
    }
    Some(CleanupObligation {
        claim: projected.clone(),
        manifest: record.manifest().clone(),
        expected_parent: slot.anchor(),
        unit_root: slot.slot(),
        authorized_parent: slot.anchor(),
        allow_missing_components: false,
        terminal_mode: CleanupTerminalMode::Normal,
    })
}

fn order_leaf_to_root(obligations: &mut [CleanupObligation]) -> Option<()> {
    let entities = obligations
        .iter()
        .map(|obligation| obligation.claim.entity())
        .collect::<HashSet<_>>();
    let mut depths = Vec::with_capacity(obligations.len());
    for obligation in obligations.iter() {
        let mut depth = 0_usize;
        let mut parent = obligation.expected_parent;
        let mut visited = HashSet::new();
        while entities.contains(&parent) {
            if !visited.insert(parent) {
                return None;
            }
            depth = depth.checked_add(1)?;
            parent = obligations
                .iter()
                .find(|candidate| candidate.claim.entity() == parent)?
                .expected_parent;
        }
        depths.push((obligation.claim.entity(), depth));
    }
    obligations.sort_by_key(|obligation| {
        let depth = depths
            .iter()
            .find_map(|(entity, depth)| (*entity == obligation.claim.entity()).then_some(*depth))
            .unwrap_or_default();
        (
            std::cmp::Reverse(depth),
            obligation.claim.entity().to_bits(),
        )
    });
    Some(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum CleanupEvidence {
    AlreadyAbsent,
    AuthorityLostResidual {
        actual_parent: Option<Entity>,
        registry_exact: bool,
        entity_present: bool,
    },
    Exact {
        actual_parent: Option<Entity>,
        normalized_manifest: ComponentManifest,
        foreign_descendants: Vec<Entity>,
        foreign_components: Vec<ComponentId>,
        missing_components: Vec<ComponentId>,
        protected_parent: Option<Entity>,
        relationship_cycle: bool,
    },
    StaleClaim,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PickableRepairOutcome {
    ExactAfterRepair,
    Unsupported,
    Superseded,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CleanupAttempt {
    ledger_domain: u64,
    revision: u64,
    manifest_digest: u64,
    unsettled_digest: u64,
    evidence: Vec<(usize, CleanupEvidence)>,
    mount_parents_exact: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CleanupReceipt {
    ledger_domain: u64,
    attempt_revision: u64,
    obligation: usize,
    disposition: CleanupDisposition,
    binding: u64,
    observed: bool,
}

struct PreparedCleanupReceiptBatch {
    receipts: Vec<CleanupReceipt>,
}

pub(crate) struct PreparedAuthorityLostResidualProjection {
    facts: Vec<AuthorityLostResidualProjectionFact>,
}

pub(crate) struct AuthorityLostResidualProjectionFact {
    ledger_domain: u64,
    manifest_digest: u64,
    attempt_revision: u64,
    obligation: usize,
    obligation_binding: u64,
    terminal_receipt_binding: u64,
    retired_claim: ExactRegistryClaim,
    frozen_expected_manifest: ComponentManifest,
    observed_manifest: Option<ComponentManifest>,
    entity_present_at_receipt: bool,
    observed_registry_claim: Option<ExactRegistryClaim>,
    actual_parent_at_receipt: Option<Entity>,
    registry_exact_at_receipt: bool,
}

impl PreparedAuthorityLostResidualProjection {
    pub(crate) fn is_empty(&self) -> bool {
        self.facts.is_empty()
    }

    pub(crate) fn facts(&self) -> &[AuthorityLostResidualProjectionFact] {
        &self.facts
    }

    pub(crate) fn into_facts(self) -> Vec<AuthorityLostResidualProjectionFact> {
        self.facts
    }
}

impl AuthorityLostResidualProjectionFact {
    pub(crate) const fn ledger_domain(&self) -> u64 {
        self.ledger_domain
    }
    pub(crate) const fn manifest_digest(&self) -> u64 {
        self.manifest_digest
    }
    pub(crate) const fn attempt_revision(&self) -> u64 {
        self.attempt_revision
    }
    pub(crate) const fn obligation(&self) -> usize {
        self.obligation
    }
    pub(crate) const fn obligation_binding(&self) -> u64 {
        self.obligation_binding
    }
    pub(crate) const fn terminal_receipt_binding(&self) -> u64 {
        self.terminal_receipt_binding
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
    pub(crate) const fn registry_exact_at_receipt(&self) -> bool {
        self.registry_exact_at_receipt
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PostSettlementFocusAuthority {
    Quarantine,
    Unresolved,
    RetiredAuthority,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PostSettlementFocusRoot {
    obligation: usize,
    source_claim: ExactRegistryClaim,
    terminal_claim: ExactRegistryClaim,
    authority: PostSettlementFocusAuthority,
    allow_missing_components: bool,
}

enum PreparedAuthorityHostEdge {
    Existing {
        index: usize,
        host: Entity,
        top_slot: Entity,
        claim: ExactRegistryClaim,
        children_before: Vec<Entity>,
        children_after: Vec<Entity>,
        detach: bool,
        require_exact: bool,
    },
    Missing {
        host: Entity,
    },
    RetainedLost {
        host: Entity,
        top_slot: Entity,
        children: Vec<Entity>,
    },
}

impl PreparedAuthorityHostEdge {
    fn partition_phase(&self) -> BackNoTargetHostEdgePhase {
        match self {
            Self::RetainedLost { .. } => BackNoTargetHostEdgePhase::CommittedRetainedLostBefore,
            Self::Existing { .. } | Self::Missing { .. } => {
                BackNoTargetHostEdgePhase::CommittedAfter
            }
        }
    }

    fn project_attempt(&self, attempt: &CleanupAttempt) -> Option<CleanupAttempt> {
        let mut projected = attempt.clone();
        let Self::Existing { index, detach, .. } = self else {
            return Some(projected);
        };
        if *detach {
            let (_, CleanupEvidence::Exact { actual_parent, .. }) =
                projected
                    .evidence
                    .iter_mut()
                    .find(|(candidate, _)| *candidate == *index)?
            else {
                return None;
            };
            *actual_parent = None;
        }
        Some(projected)
    }

    fn apply_and_validate(&self, world: &mut World) -> bool {
        match self {
            Self::Missing { host } => world.get_entity(*host).is_err(),
            Self::RetainedLost {
                host,
                top_slot,
                children,
            } => {
                if world.get_entity(*host).is_err()
                    || world.get::<ChildOf>(*top_slot).map(ChildOf::parent) != Some(*host)
                    || !children_match(world, *host, children)
                {
                    return false;
                }
                self.postwrite_matches(world)
            }
            Self::Existing {
                host,
                top_slot,
                claim,
                children_before,
                detach,
                ..
            } => {
                if !*detach {
                    return self.postwrite_matches(world);
                }
                if world.get::<ChildOf>(*top_slot).map(ChildOf::parent) != Some(*host)
                    || !children_match(world, *host, children_before)
                    || !cleanup_claim_has_exact_authority(world, claim)
                {
                    return false;
                }
                world.entity_mut(*top_slot).remove::<ChildOf>();
                world.flush();
                apply_authority_host_edge_postwrite_drift(world, *top_slot, *host);
                self.postwrite_matches(world)
            }
        }
    }

    fn postwrite_matches(&self, world: &World) -> bool {
        match self {
            Self::Existing {
                host,
                top_slot,
                claim,
                children_after,
                require_exact,
                ..
            } => {
                world.get_entity(*host).is_ok()
                    && world.get::<ChildOf>(*top_slot).map(ChildOf::parent) != Some(*host)
                    && children_match(world, *host, children_after)
                    && (!*require_exact || cleanup_claim_has_exact_authority(world, claim))
            }
            Self::Missing { host } => world.get_entity(*host).is_err(),
            Self::RetainedLost {
                host,
                top_slot,
                children,
            } => {
                world.get_entity(*host).is_ok()
                    && world.get::<ChildOf>(*top_slot).map(ChildOf::parent) == Some(*host)
                    && children_match(world, *host, children)
            }
        }
    }

    fn removes_child_of(&self, entity: Entity) -> bool {
        matches!(
            self,
            Self::Existing {
                top_slot,
                detach: true,
                ..
            } if *top_slot == entity
        )
    }
}

struct PreparedAuthorityPresentation {
    claims: Vec<ExactRegistryClaim>,
}

impl PreparedAuthorityPresentation {
    fn prepare(world: &World, claims: Vec<ExactRegistryClaim>) -> Option<Self> {
        claims
            .iter()
            .all(|claim| cleanup_claim_is_inert_and_exact(world, claim, true))
            .then_some(Self { claims })
    }

    fn apply(self, world: &mut World) {
        for claim in self.claims {
            *world
                .get_mut::<Visibility>(claim.entity())
                .expect("prepared authority quarantine retains Visibility") = Visibility::Hidden;
            *world
                .get_mut::<Pickable>(claim.entity())
                .expect("prepared authority quarantine retains Pickable") = Pickable::IGNORE;
            *world
                .get_mut::<RouterInputBarrier>(claim.entity())
                .expect("prepared authority quarantine retains input barrier") =
                RouterInputBarrier::deny_all();
            match claim.owner() {
                RegistryOwner::Committed(
                    EntityOwner::FrameShell(_)
                    | EntityOwner::FrameContentTarget(_)
                    | EntityOwner::FrameOutput { .. },
                ) => {
                    *world
                        .get_mut::<FrameLifecycle>(claim.entity())
                        .expect("prepared frame quarantine retains lifecycle") =
                        FrameLifecycle(EntityLifecycle::Quarantined);
                }
                RegistryOwner::Committed(EntityOwner::ChildSlot { .. })
                | RegistryOwner::Provisional(ProvisionalOwner::ChildSlot { .. }) => {
                    *world
                        .get_mut::<ChildSlotLifecycle>(claim.entity())
                        .expect("prepared child-slot quarantine retains lifecycle") =
                        ChildSlotLifecycle::Quarantined;
                }
                RegistryOwner::Committed(EntityOwner::HostTopSlot { .. }) => {
                    *world
                        .get_mut::<ProvisionalSlotLifecycle>(claim.entity())
                        .expect("prepared top-slot quarantine retains lifecycle") =
                        ProvisionalSlotLifecycle::Quarantined;
                }
                _ => unreachable!("authority quarantine plan contains a router-owned claim"),
            }
        }
    }

    fn is_current(&self, world: &World) -> bool {
        self.claims
            .iter()
            .all(|claim| cleanup_claim_is_inert_and_exact(world, claim, false))
    }
}

struct PreparedAuthorityPickableRepair {
    claims: Vec<ExactRegistryClaim>,
    missing: Vec<Entity>,
}

impl PreparedAuthorityPickableRepair {
    fn prepare(world: &World, claims: Vec<ExactRegistryClaim>) -> Option<Self> {
        if !claims
            .iter()
            .all(|claim| cleanup_claim_is_inert_and_exact(world, claim, true))
        {
            return None;
        }
        let mut missing = Vec::new();
        missing.try_reserve_exact(claims.len()).ok()?;
        missing.extend(claims.iter().filter_map(|claim| {
            world
                .get::<Pickable>(claim.entity())
                .is_none()
                .then_some(claim.entity())
        }));
        Some(Self { claims, missing })
    }

    fn repairs(&self, entity: Entity) -> bool {
        self.missing.contains(&entity)
    }

    fn apply(&self, world: &mut World) -> bool {
        for entity in &self.missing {
            world.entity_mut(*entity).insert(Pickable::IGNORE);
        }
        if !self.missing.is_empty() {
            world.flush();
        }
        self.claims
            .iter()
            .all(|claim| cleanup_claim_is_inert_and_exact(world, claim, false))
    }
}

struct PreparedAuthorityRegistryTransfers {
    verified: Option<PreparedQuarantineTransfer>,
    lost: Option<PreparedAuthorityLostResidualTransfer>,
}

impl PreparedAuthorityRegistryTransfers {
    fn is_current(&self, world: &World) -> bool {
        self.lost.as_ref().is_none_or(|lost| lost.is_current(world))
    }
}

struct PreparedAuthorityRemoval {
    index: usize,
    expected_parent: Option<Entity>,
    children_before: Vec<Entity>,
    receipt: PreparedCleanupReceiptBatch,
    retirement: Option<PreparedExactRegistryRetirement>,
}

impl PreparedAuthorityRemoval {
    fn is_current(
        &self,
        world: &World,
        obligations: &[CleanupObligation],
        completed_removals: &[Entity],
    ) -> bool {
        let Some(claim) = obligations
            .get(self.index)
            .map(|obligation| &obligation.claim)
        else {
            return false;
        };
        world
            .resource::<RouteEntityRegistry>()
            .record(claim.entity())
            .is_some_and(|record| record.claim() == *claim)
            && world.get::<ChildOf>(claim.entity()).map(ChildOf::parent) == self.expected_parent
            && children_match_after_removals(
                world,
                claim.entity(),
                &self.children_before,
                completed_removals,
            )
            && cleanup_claim_has_exact_authority(world, claim)
    }
}

struct PreparedAuthorityResidualGroup {
    indices: Vec<usize>,
    manifests: Vec<(Entity, ComponentManifest)>,
    verified_lease: Option<QuarantineLeaseId>,
    presentation: PreparedAuthorityPresentation,
    transfers: PreparedAuthorityRegistryTransfers,
    receipts: PreparedCleanupReceiptBatch,
}

impl PreparedAuthorityResidualGroup {
    fn is_current(&self, world: &World) -> bool {
        self.presentation.is_current(world)
            && self.transfers.is_current(world)
            && self.manifests.iter().all(|(entity, manifest)| {
                frozen_manifest_matches_archetype_no_alloc(world, *entity, manifest)
            })
    }
}

fn frozen_manifest_matches_archetype_no_alloc(
    world: &World,
    entity: Entity,
    frozen: &ComponentManifest,
) -> bool {
    let Ok(entity) = world.get_entity(entity) else {
        return false;
    };
    let archetype = entity.archetype();
    if archetype.components().len() != frozen.component_ids().len() {
        return false;
    }
    frozen
        .component_ids()
        .iter()
        .all(|expected| archetype.components().contains(expected))
}

struct PreparedAuthoritySettlement {
    publication_capacity: PreparedAuthorityPublicationCapacity,
    pickable_repair: PreparedAuthorityPickableRepair,
    prepublication_evidence: Vec<(usize, CleanupEvidence)>,
    terminal_projections: Vec<PreparedBackNoTargetAuthorityTerminalProjection>,
    host_edge: PreparedAuthorityHostEdge,
    absent: Vec<PreparedAuthorityRemoval>,
    removed: Vec<PreparedAuthorityRemoval>,
    removal_order: Vec<Entity>,
    residuals: Vec<PreparedAuthorityResidualGroup>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AuthoritySettlementAction {
    AlreadyAbsent,
    Remove,
    VerifiedResidual,
    LostResidual,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AuthoritySettlementDisposition {
    index: usize,
    disposition: CleanupDisposition,
    action: AuthoritySettlementAction,
}

pub(crate) struct PreparedCleanupLedger {
    batch: Option<CleanupBatch>,
    obligations: Vec<CleanupObligation>,
    mount_parents: Vec<CleanupMountParentSettlement>,
    manifest_digest: u64,
    receipts: Vec<Option<CleanupReceipt>>,
    ledger_domain: u64,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct PreparedCleanupLedgerProjection {
    ledger_domain: u64,
    manifest_digest: u64,
    obligation_count: usize,
}

impl PreparedCleanupLedgerProjection {
    pub(crate) const fn manifest_digest(self) -> u64 {
        self.manifest_digest
    }

    pub(crate) const fn obligation_count(self) -> usize {
        self.obligation_count
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CleanupLedgerReserveErrorKind {
    RetryableCapacity,
    DomainExhausted,
    UnfinalizedBoundaryFacts,
}

#[derive(Debug)]
pub(crate) struct CleanupLedgerReserveError {
    kind: CleanupLedgerReserveErrorKind,
    manifest: FrozenCleanupManifest,
}

impl CleanupLedgerReserveError {
    const fn new(kind: CleanupLedgerReserveErrorKind, manifest: FrozenCleanupManifest) -> Self {
        Self { kind, manifest }
    }

    pub(crate) const fn kind(&self) -> CleanupLedgerReserveErrorKind {
        self.kind
    }

    pub(crate) fn into_manifest(self) -> FrozenCleanupManifest {
        self.manifest
    }
}

impl PreparedCleanupLedger {
    pub(crate) fn projection(&self) -> PreparedCleanupLedgerProjection {
        PreparedCleanupLedgerProjection {
            ledger_domain: self.ledger_domain,
            manifest_digest: self.manifest_digest,
            obligation_count: self.obligations.len(),
        }
    }

    pub(crate) fn bind_batch(mut self, batch: CleanupBatch) -> Self {
        self.batch = Some(batch);
        self
    }
}

pub(crate) struct CleanupLedger {
    batch: Option<CleanupBatch>,
    back_no_target_batch: Option<BackNoTargetCleanupBatchView>,
    back_no_target_capacity: Option<InstalledBackCapacityLease>,
    owner: Option<CleanupOwner>,
    obligations: Vec<CleanupObligation>,
    mount_parents: Vec<CleanupMountParentSettlement>,
    manifest_digest: u64,
    receipts: Vec<Option<CleanupReceipt>>,
    ledger_domain: u64,
    next_attempt: u64,
    issued_attempt: Option<CleanupAttempt>,
    completion_digest: Option<u64>,
    quarantine_event_observed: bool,
    post_settlement_focus: Vec<PostSettlementFocusRoot>,
    authority_host_children: Option<Vec<Entity>>,
    back_no_target_authority_partition: Option<FrozenBackNoTargetAuthorityPartition>,
}

struct InstalledBackCapacityLease {
    domain: PreparationCapacityDomain,
    cell: BackCapacityCellId,
    owner: BackCapacityOwnerId,
    generation: PreparationReleaseGeneration,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct BackNoTargetZeroCompletionFacts {
    manifest_digest: u64,
    obligation_count: usize,
    completion_digest: u64,
    settled_count: usize,
}

impl BackNoTargetZeroCompletionFacts {
    pub(crate) const fn manifest_digest(self) -> u64 {
        self.manifest_digest
    }

    pub(crate) const fn obligation_count(self) -> usize {
        self.obligation_count
    }

    pub(crate) const fn completion_digest(self) -> u64 {
        self.completion_digest
    }

    pub(crate) const fn settled_count(self) -> usize {
        self.settled_count
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct BackNoTargetAuthorityCompletionFacts {
    manifest_digest: u64,
    obligation_count: usize,
    completion_digest: u64,
    settled_count: usize,
}

impl BackNoTargetAuthorityCompletionFacts {
    pub(crate) const fn manifest_digest(self) -> u64 {
        self.manifest_digest
    }

    pub(crate) const fn obligation_count(self) -> usize {
        self.obligation_count
    }

    pub(crate) const fn completion_digest(self) -> u64 {
        self.completion_digest
    }

    pub(crate) const fn settled_count(self) -> usize {
        self.settled_count
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BackNoTargetInstalledCapacitySource {
    Absent,
    Present {
        domain: u64,
        cell: usize,
        owner: u64,
        generation: u64,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct BackNoTargetZeroCompletionSourceIdentity {
    ledger_domain: u64,
    capacity: BackNoTargetInstalledCapacitySource,
}

impl BackNoTargetZeroCompletionSourceIdentity {
    pub(crate) const fn ledger_domain(self) -> u64 {
        self.ledger_domain
    }

    pub(crate) const fn capacity(self) -> BackNoTargetInstalledCapacitySource {
        self.capacity
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BackNoTargetDedicatedBatchObservation {
    Absent,
    Present {
        manifest_digest: u64,
        obligation_count: usize,
        expected_completion_digest: u64,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BackNoTargetNormalBatchObservation {
    Absent,
    Present {
        manifest_digest: u64,
        obligation_count: usize,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BackNoTargetCleanupOwnerObservation {
    Absent,
    Leaving,
    FailedPrepared,
    AuthoritySettlement,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BackNoTargetCleanupAttemptObservation {
    Absent,
    Present {
        ledger_domain: u64,
        revision: u64,
        manifest_digest: u64,
        unsettled_digest: u64,
        evidence_count: usize,
        mount_parents_exact: bool,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BackNoTargetAuthorityResidualObservation {
    Absent,
    Present { host_child_count: usize },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct BackNoTargetZeroCompletionObservedFacts {
    dedicated_batch: BackNoTargetDedicatedBatchObservation,
    normal_batch: BackNoTargetNormalBatchObservation,
    owner: BackNoTargetCleanupOwnerObservation,
    attempt: BackNoTargetCleanupAttemptObservation,
    manifest_digest: u64,
    obligation_count: usize,
    mount_parent_count: usize,
    receipt_slot_count: usize,
    settled_count: usize,
    completion_digest: Option<u64>,
    next_attempt: u64,
    quarantine_event_observed: bool,
    focus_residual_count: usize,
    authority_residual: BackNoTargetAuthorityResidualObservation,
}

impl BackNoTargetZeroCompletionObservedFacts {
    pub(crate) const fn dedicated_batch(self) -> BackNoTargetDedicatedBatchObservation {
        self.dedicated_batch
    }

    pub(crate) const fn completion_digest(self) -> Option<u64> {
        self.completion_digest
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct BackNoTargetZeroCompletionPreparationEvidence {
    source: BackNoTargetZeroCompletionSourceIdentity,
    expected: BackNoTargetZeroCompletionFacts,
    observed: BackNoTargetZeroCompletionObservedFacts,
}

impl BackNoTargetZeroCompletionPreparationEvidence {
    pub(crate) const fn source_identity(self) -> BackNoTargetZeroCompletionSourceIdentity {
        self.source
    }

    pub(crate) const fn expected(self) -> BackNoTargetZeroCompletionFacts {
        self.expected
    }

    pub(crate) const fn observed(self) -> BackNoTargetZeroCompletionObservedFacts {
        self.observed
    }

    fn is_exact(self) -> bool {
        let BackNoTargetZeroCompletionFacts {
            manifest_digest,
            obligation_count,
            completion_digest,
            settled_count,
        } = self.expected;
        matches!(
            self.source.capacity,
            BackNoTargetInstalledCapacitySource::Present { .. }
        ) && self.observed.dedicated_batch
            == (BackNoTargetDedicatedBatchObservation::Present {
                manifest_digest,
                obligation_count,
                expected_completion_digest: completion_digest,
            })
            && self.observed.normal_batch == BackNoTargetNormalBatchObservation::Absent
            && self.observed.owner == BackNoTargetCleanupOwnerObservation::Absent
            && self.observed.attempt == BackNoTargetCleanupAttemptObservation::Absent
            && self.observed.manifest_digest == manifest_digest
            && self.observed.obligation_count == obligation_count
            && self.observed.mount_parent_count == 0
            && self.observed.receipt_slot_count == obligation_count
            && self.observed.settled_count == settled_count
            && self.observed.completion_digest == Some(completion_digest)
            && self.observed.next_attempt == 1
            && !self.observed.quarantine_event_observed
            && self.observed.focus_residual_count == 0
            && self.observed.authority_residual == BackNoTargetAuthorityResidualObservation::Absent
    }
}

pub(crate) struct BackNoTargetZeroCompletionPreparationFailure {
    ledger: CleanupLedger,
    evidence: BackNoTargetZeroCompletionPreparationEvidence,
}

impl BackNoTargetZeroCompletionPreparationFailure {
    pub(crate) const fn evidence(&self) -> BackNoTargetZeroCompletionPreparationEvidence {
        self.evidence
    }

    pub(crate) fn into_parts(
        self,
    ) -> (CleanupLedger, BackNoTargetZeroCompletionPreparationEvidence) {
        (self.ledger, self.evidence)
    }
}

pub(crate) struct PreparedBackNoTargetZeroCompletion {
    ledger: CleanupLedger,
    facts: BackNoTargetZeroCompletionFacts,
    source: BackNoTargetZeroCompletionSourceIdentity,
}

impl PreparedBackNoTargetZeroCompletion {
    pub(crate) const fn facts(&self) -> BackNoTargetZeroCompletionFacts {
        self.facts
    }

    pub(crate) fn into_ledger(self) -> CleanupLedger {
        self.ledger
    }

    pub(crate) const fn source_identity(&self) -> BackNoTargetZeroCompletionSourceIdentity {
        self.source
    }

    #[cfg(test)]
    pub(crate) fn capacity_identity_for_test(&self) -> (u64, usize, u64, u64) {
        let capacity = self
            .ledger
            .back_no_target_capacity
            .as_ref()
            .expect("prepared exact-zero completion retains its installed capacity lease");
        (
            capacity.domain.0,
            capacity.cell.0,
            capacity.owner.0,
            capacity.generation.0,
        )
    }
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct BackNoTargetLedgerProjectionForTest {
    pub(crate) header_installed: bool,
    pub(crate) capacity_identity: (u64, usize, u64, u64),
    pub(crate) ledger_domain: u64,
    pub(crate) manifest_digest: u64,
    pub(crate) obligation_count: usize,
    pub(crate) receipt_count: usize,
    pub(crate) completion_digest: u64,
    pub(crate) batch_manifest_digest: u64,
    pub(crate) batch_obligation_count: usize,
    pub(crate) batch_expected_completion_digest: u64,
}

pub(crate) enum CleanupOwner {
    Leaving(RealizedBranch),
    FailedPrepared(FailedPreparedBranch),
    AuthoritySettlement(AuthoritySettlementOwner),
    BackNoTargetAuthority(BackNoTargetAuthorityCleanupOwner),
}

pub(crate) struct AuthoritySettlementOwner {
    active: Option<RealizedBranch>,
    pending: AuthoritySettlementPendingOwner,
    host: RetiredHostBinding,
}

pub(crate) struct BackNoTargetAuthorityCleanupOwner {
    active: RealizedBranch,
    host: RetiredHostBinding,
}

pub(crate) struct PreparedBackNoTargetAuthorityCleanupLedger {
    prepared: PreparedCleanupLedger,
    owner: BackNoTargetAuthorityCleanupOwner,
    host_children: Vec<Entity>,
    partition: FrozenBackNoTargetAuthorityPartition,
}

impl PreparedBackNoTargetAuthorityCleanupLedger {
    pub(crate) fn new(
        prepared: PreparedCleanupLedger,
        active: RealizedBranch,
        host: RetiredHostBinding,
        host_children: Vec<Entity>,
        partition: FrozenBackNoTargetAuthorityPartition,
    ) -> Self {
        Self {
            prepared,
            owner: BackNoTargetAuthorityCleanupOwner { active, host },
            host_children,
            partition,
        }
    }

    pub(crate) fn into_active(self) -> CleanupLedger {
        CleanupLedger::activate_back_no_target_authority(
            self.prepared,
            self.owner.active,
            self.owner.host,
            self.host_children,
            self.partition,
        )
    }

    pub(crate) fn into_unstarted_parts(
        self,
    ) -> (
        PreparedCleanupLedger,
        BackNoTargetAuthorityCleanupOwner,
        Vec<Entity>,
        FrozenBackNoTargetAuthorityPartition,
    ) {
        (
            self.prepared,
            self.owner,
            self.host_children,
            self.partition,
        )
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn leaving_frames(&self) -> &[RealizedFrame] {
        self.owner.active.frames()
    }

    #[cfg(feature = "test-support")]
    pub(crate) fn tracked_child_slots(&self) -> Vec<crate::branch_transaction::TrackedChildSlot> {
        self.owner.active.tracked_child_slots()
    }
}

impl BackNoTargetAuthorityCleanupOwner {
    pub(crate) fn active(&self) -> &RealizedBranch {
        &self.active
    }

    pub(crate) fn host(&self) -> &RetiredHostBinding {
        &self.host
    }

    pub(crate) fn into_parts(self) -> (RealizedBranch, RetiredHostBinding) {
        (self.active, self.host)
    }
}

enum AuthoritySettlementPendingOwner {
    Prepared(FailedPreparedBranch),
    Terminal(PendingFailedBranch),
}

impl AuthoritySettlementPendingOwner {
    fn branch(&self) -> &FailedPreparedBranch {
        match self {
            Self::Prepared(branch) => branch,
            Self::Terminal(failed) => failed.branch(),
        }
    }
}

impl CleanupLedger {
    #[cfg(test)]
    pub(crate) fn prepare(
        batch: CleanupBatch,
        manifest: FrozenCleanupManifest,
    ) -> Result<PreparedCleanupLedger, CleanupLedgerReserveError> {
        assert!(cleanup_batch_matches(
            &batch,
            manifest.digest,
            manifest.obligations.len()
        ));
        Self::reserve(manifest).map(|prepared| prepared.bind_batch(batch))
    }

    pub(crate) fn reserve(
        manifest: FrozenCleanupManifest,
    ) -> Result<PreparedCleanupLedger, CleanupLedgerReserveError> {
        Self::reserve_with_domain_counter(manifest, &NEXT_LEDGER_DOMAIN)
    }

    fn reserve_with_domain_counter(
        mut manifest: FrozenCleanupManifest,
        domain_counter: &AtomicU64,
    ) -> Result<PreparedCleanupLedger, CleanupLedgerReserveError> {
        inject_unfinalized_boundary_drift(&mut manifest);
        if !manifest.boundary_drifts.is_empty() {
            return Err(CleanupLedgerReserveError::new(
                CleanupLedgerReserveErrorKind::UnfinalizedBoundaryFacts,
                manifest,
            ));
        }
        Self::prepare_physical(None, manifest, domain_counter)
    }

    fn prepare_physical(
        batch: Option<CleanupBatch>,
        manifest: FrozenCleanupManifest,
        domain_counter: &AtomicU64,
    ) -> Result<PreparedCleanupLedger, CleanupLedgerReserveError> {
        if reservation_fault_injected() {
            return Err(CleanupLedgerReserveError::new(
                CleanupLedgerReserveErrorKind::RetryableCapacity,
                manifest,
            ));
        }
        let mut receipts = Vec::new();
        if receipts
            .try_reserve_exact(manifest.obligations.len())
            .is_err()
        {
            return Err(CleanupLedgerReserveError::new(
                CleanupLedgerReserveErrorKind::RetryableCapacity,
                manifest,
            ));
        }
        receipts.resize(manifest.obligations.len(), None);
        let allocation_counter = ledger_domain_counter_input(domain_counter);
        let Some(ledger_domain) = allocate_ledger_domain(allocation_counter) else {
            return Err(CleanupLedgerReserveError::new(
                CleanupLedgerReserveErrorKind::DomainExhausted,
                manifest,
            ));
        };
        if std::ptr::eq(allocation_counter, &NEXT_LEDGER_DOMAIN) {
            observe_ledger_domain_allocation();
        }
        Ok(PreparedCleanupLedger {
            batch,
            obligations: manifest.obligations,
            mount_parents: manifest.mount_parents,
            manifest_digest: manifest.digest,
            receipts,
            ledger_domain,
        })
    }

    #[cfg(test)]
    pub(crate) fn for_failed_realization(
        world: &World,
        frame: RealizedFrame,
    ) -> Result<Self, RealizedFrame> {
        let manifest = {
            let Some(registry) = world.get_resource::<RouteEntityRegistry>() else {
                return Err(frame);
            };
            let Some(manifest) = FrozenCleanupManifest::from_current_frame(world, registry, &frame)
            else {
                return Err(frame);
            };
            manifest
        };
        match Self::reserve(manifest) {
            Ok(prepared) => Ok(Self::activate(prepared, Some(frame))),
            Err(error) => match error.kind() {
                CleanupLedgerReserveErrorKind::RetryableCapacity
                | CleanupLedgerReserveErrorKind::DomainExhausted
                | CleanupLedgerReserveErrorKind::UnfinalizedBoundaryFacts => Err(frame),
            },
        }
    }

    #[cfg(test)]
    pub(crate) fn for_failed_prepared_branch(
        world: &World,
        branch: FailedPreparedBranch,
    ) -> Result<Self, FailedPreparedBranch> {
        let manifest = {
            let Some(registry) = world.get_resource::<RouteEntityRegistry>() else {
                return Err(branch);
            };
            let Ok(manifest) =
                FrozenCleanupManifest::from_failed_prepared_branch(world, registry, &branch)
            else {
                return Err(branch);
            };
            manifest
        };
        match Self::reserve(manifest) {
            Ok(prepared) => Ok(Self::activate_failed(prepared, branch)),
            Err(error) => match error.kind() {
                CleanupLedgerReserveErrorKind::RetryableCapacity
                | CleanupLedgerReserveErrorKind::DomainExhausted
                | CleanupLedgerReserveErrorKind::UnfinalizedBoundaryFacts => Err(branch),
            },
        }
    }

    pub(crate) fn activate(
        prepared: PreparedCleanupLedger,
        leaving: Option<RealizedFrame>,
    ) -> Self {
        Self::activate_owner(
            prepared,
            leaving.map(|frame| CleanupOwner::Leaving(RealizedBranch::from_single_frame(frame))),
        )
    }

    pub(crate) fn activate_failed(
        prepared: PreparedCleanupLedger,
        failed: FailedPreparedBranch,
    ) -> Self {
        Self::activate_owner(prepared, Some(CleanupOwner::FailedPrepared(failed)))
    }

    pub(crate) fn activate_leaving_branch(
        prepared: PreparedCleanupLedger,
        leaving: Option<RealizedBranch>,
    ) -> Self {
        Self::activate_owner(prepared, leaving.map(CleanupOwner::Leaving))
    }

    pub(crate) fn activate_authority_settlement(
        prepared: PreparedCleanupLedger,
        active: RealizedBranch,
        pending: FailedPreparedBranch,
        host: RetiredHostBinding,
        host_children: Vec<Entity>,
    ) -> Self {
        let mut ledger = Self::activate_owner(
            prepared,
            Some(CleanupOwner::AuthoritySettlement(
                AuthoritySettlementOwner {
                    active: Some(active),
                    pending: AuthoritySettlementPendingOwner::Prepared(pending),
                    host,
                },
            )),
        );
        ledger.authority_host_children = Some(host_children);
        ledger
    }

    pub(crate) fn activate_failed_authority_settlement(
        prepared: PreparedCleanupLedger,
        active: Option<RealizedBranch>,
        failed: PendingFailedBranch,
        host: RetiredHostBinding,
        host_children: Vec<Entity>,
    ) -> Self {
        let mut ledger = Self::activate_owner(
            prepared,
            Some(CleanupOwner::AuthoritySettlement(
                AuthoritySettlementOwner {
                    active,
                    pending: AuthoritySettlementPendingOwner::Terminal(failed),
                    host,
                },
            )),
        );
        ledger.authority_host_children = Some(host_children);
        ledger
    }

    pub(crate) fn activate_back_no_target_authority(
        prepared: PreparedCleanupLedger,
        active: RealizedBranch,
        host: RetiredHostBinding,
        host_children: Vec<Entity>,
        partition: FrozenBackNoTargetAuthorityPartition,
    ) -> Self {
        let mut ledger = Self::activate_owner(
            prepared,
            Some(CleanupOwner::BackNoTargetAuthority(
                BackNoTargetAuthorityCleanupOwner { active, host },
            )),
        );
        ledger.authority_host_children = Some(host_children);
        ledger.back_no_target_authority_partition = Some(partition);
        ledger
    }

    pub(crate) fn is_authority_settlement(&self) -> bool {
        matches!(self.owner, Some(CleanupOwner::AuthoritySettlement(_)))
    }

    fn is_physical_authority_settlement(&self) -> bool {
        matches!(
            self.owner,
            Some(CleanupOwner::AuthoritySettlement(_) | CleanupOwner::BackNoTargetAuthority(_))
        )
    }

    pub(crate) fn back_no_target_authority_partition_is_current(&self, world: &World) -> bool {
        self.back_no_target_authority_partition
            .as_ref()
            .is_some_and(|partition| partition.refreeze(world, &self.obligations, &self.receipts))
            && self
                .post_settlement_focus
                .iter()
                .all(|root| self.post_settlement_focus_root_is_current(world, root))
    }

    pub(crate) fn take_authority_settlement_owner(&mut self) -> Option<AuthoritySettlementOwner> {
        match self.owner.take() {
            Some(CleanupOwner::AuthoritySettlement(owner)) => Some(owner),
            other => {
                self.owner = other;
                None
            }
        }
    }

    fn release_settled_owner(&mut self) {
        if !self.is_physical_authority_settlement() {
            self.owner.take();
        }
    }

    fn activate_owner(prepared: PreparedCleanupLedger, owner: Option<CleanupOwner>) -> Self {
        let completion_digest = prepared
            .obligations
            .is_empty()
            .then_some(canonical_empty_completion_digest());
        Self {
            batch: prepared.batch,
            back_no_target_batch: None,
            back_no_target_capacity: None,
            owner,
            obligations: prepared.obligations,
            mount_parents: prepared.mount_parents,
            manifest_digest: prepared.manifest_digest,
            receipts: prepared.receipts,
            ledger_domain: prepared.ledger_domain,
            next_attempt: 1,
            issued_attempt: None,
            completion_digest,
            quarantine_event_observed: false,
            post_settlement_focus: Vec::new(),
            authority_host_children: None,
            back_no_target_authority_partition: None,
        }
    }

    fn activate_back_no_target(
        vacant: ManifestNeutralLedgerStorage,
        batch: BackNoTargetCleanupBatchView,
        capacity: InstalledBackCapacityLease,
    ) -> Self {
        let ManifestNeutralLedgerStorage {
            obligations,
            mount_parents,
            receipts,
            ledger_domain,
        } = vacant;
        let manifest_digest = batch.manifest_digest();
        Self {
            batch: None,
            back_no_target_batch: Some(batch),
            back_no_target_capacity: Some(capacity),
            owner: None,
            obligations,
            mount_parents,
            manifest_digest,
            receipts,
            ledger_domain,
            next_attempt: 1,
            issued_attempt: None,
            completion_digest: Some(canonical_zero_completion_digest()),
            quarantine_event_observed: false,
            post_settlement_focus: Vec::new(),
            authority_host_children: None,
            back_no_target_authority_partition: None,
        }
    }

    fn back_no_target_zero_completion_source_identity(
        &self,
    ) -> BackNoTargetZeroCompletionSourceIdentity {
        let capacity = match self.back_no_target_capacity.as_ref() {
            Some(capacity) => BackNoTargetInstalledCapacitySource::Present {
                domain: capacity.domain.0,
                cell: capacity.cell.0,
                owner: capacity.owner.0,
                generation: capacity.generation.0,
            },
            None => BackNoTargetInstalledCapacitySource::Absent,
        };
        BackNoTargetZeroCompletionSourceIdentity {
            ledger_domain: self.ledger_domain,
            capacity,
        }
    }

    fn back_no_target_zero_completion_preparation_evidence(
        &self,
    ) -> BackNoTargetZeroCompletionPreparationEvidence {
        let dedicated_batch = match self.back_no_target_batch.as_ref() {
            Some(batch) => BackNoTargetDedicatedBatchObservation::Present {
                manifest_digest: batch.manifest_digest(),
                obligation_count: batch.obligation_count(),
                expected_completion_digest: batch.expected_completion_digest(),
            },
            None => BackNoTargetDedicatedBatchObservation::Absent,
        };
        let normal_batch = match self.batch.as_ref() {
            Some(batch) => BackNoTargetNormalBatchObservation::Present {
                manifest_digest: batch.manifest_digest(),
                obligation_count: batch.obligation_count(),
            },
            None => BackNoTargetNormalBatchObservation::Absent,
        };
        let owner = match self.owner.as_ref() {
            Some(CleanupOwner::Leaving(_)) => BackNoTargetCleanupOwnerObservation::Leaving,
            Some(CleanupOwner::FailedPrepared(_)) => {
                BackNoTargetCleanupOwnerObservation::FailedPrepared
            }
            Some(CleanupOwner::AuthoritySettlement(_)) => {
                BackNoTargetCleanupOwnerObservation::AuthoritySettlement
            }
            Some(CleanupOwner::BackNoTargetAuthority(_)) => {
                BackNoTargetCleanupOwnerObservation::AuthoritySettlement
            }
            None => BackNoTargetCleanupOwnerObservation::Absent,
        };
        let attempt = match self.issued_attempt.as_ref() {
            Some(attempt) => BackNoTargetCleanupAttemptObservation::Present {
                ledger_domain: attempt.ledger_domain,
                revision: attempt.revision,
                manifest_digest: attempt.manifest_digest,
                unsettled_digest: attempt.unsettled_digest,
                evidence_count: attempt.evidence.len(),
                mount_parents_exact: attempt.mount_parents_exact,
            },
            None => BackNoTargetCleanupAttemptObservation::Absent,
        };
        let authority_residual = match self.authority_host_children.as_ref() {
            Some(children) => BackNoTargetAuthorityResidualObservation::Present {
                host_child_count: children.len(),
            },
            None => BackNoTargetAuthorityResidualObservation::Absent,
        };
        BackNoTargetZeroCompletionPreparationEvidence {
            source: self.back_no_target_zero_completion_source_identity(),
            expected: BackNoTargetZeroCompletionFacts {
                manifest_digest: canonical_zero_manifest_digest(),
                obligation_count: 0,
                completion_digest: canonical_zero_completion_digest(),
                settled_count: 0,
            },
            observed: BackNoTargetZeroCompletionObservedFacts {
                dedicated_batch,
                normal_batch,
                owner,
                attempt,
                manifest_digest: self.manifest_digest,
                obligation_count: self.obligations.len(),
                mount_parent_count: self.mount_parents.len(),
                receipt_slot_count: self.receipts.len(),
                settled_count: self.receipts.iter().flatten().count(),
                completion_digest: self.completion_digest,
                next_attempt: self.next_attempt,
                quarantine_event_observed: self.quarantine_event_observed,
                focus_residual_count: self.post_settlement_focus.len(),
                authority_residual,
            },
        }
    }

    pub(crate) fn prepare_back_no_target_zero_completion(
        self,
    ) -> Result<PreparedBackNoTargetZeroCompletion, BackNoTargetZeroCompletionPreparationFailure>
    {
        let evidence = self.back_no_target_zero_completion_preparation_evidence();
        if !evidence.is_exact() {
            return Err(BackNoTargetZeroCompletionPreparationFailure {
                ledger: self,
                evidence,
            });
        }

        Ok(PreparedBackNoTargetZeroCompletion {
            facts: evidence.expected,
            source: evidence.source,
            ledger: self,
        })
    }

    #[cfg(test)]
    pub(crate) fn drift_back_no_target_completion_digest_for_test(&mut self) {
        self.completion_digest = self.completion_digest.map(|digest| digest ^ 1);
    }

    #[cfg(test)]
    pub(crate) fn back_no_target_projection_for_test(
        &self,
    ) -> Option<BackNoTargetLedgerProjectionForTest> {
        let capacity = self.back_no_target_capacity.as_ref()?;
        let batch = self.back_no_target_batch.as_ref()?;
        let completion_digest = self.completion_digest?;
        let batch_expected_completion_digest = batch.expected_completion_digest();
        assert_eq!(
            completion_digest,
            canonical_zero_completion_digest(),
            "the installed no-target ledger stores the canonical zero completion digest"
        );
        assert_eq!(
            batch_expected_completion_digest, completion_digest,
            "the no-target batch commitment must match the installed ledger completion digest"
        );
        Some(BackNoTargetLedgerProjectionForTest {
            header_installed: true,
            capacity_identity: (
                capacity.domain.0,
                capacity.cell.0,
                capacity.owner.0,
                capacity.generation.0,
            ),
            ledger_domain: self.ledger_domain,
            manifest_digest: self.manifest_digest,
            obligation_count: self.obligations.len(),
            receipt_count: self.receipts.iter().flatten().count(),
            completion_digest,
            batch_manifest_digest: batch.manifest_digest(),
            batch_obligation_count: batch.obligation_count(),
            batch_expected_completion_digest,
        })
    }

    pub(crate) fn pending_completion(&self) -> Option<CleanupCompletion> {
        let completion_digest = self.completion_digest?;
        let batch = self.batch.clone()?;
        Some(if self.obligations.is_empty() {
            batch.complete_empty()
        } else {
            batch.complete(completion_digest, self.obligations.len())
        })
    }

    pub(crate) fn back_no_target_authority_completion_facts(
        &self,
    ) -> Option<BackNoTargetAuthorityCompletionFacts> {
        if !matches!(self.owner, Some(CleanupOwner::BackNoTargetAuthority(_)))
            || self.batch.is_some()
            || self.back_no_target_batch.is_some()
            || self.back_no_target_capacity.is_some()
            || self.mount_parents.len() != 0
            || self.receipts.iter().any(|receipt| receipt.is_none())
        {
            return None;
        }
        Some(BackNoTargetAuthorityCompletionFacts {
            manifest_digest: self.manifest_digest,
            obligation_count: self.obligations.len(),
            completion_digest: self.completion_digest?,
            settled_count: self.receipts.iter().flatten().count(),
        })
    }

    pub(crate) fn batch(&self) -> Option<&CleanupBatch> {
        self.batch.as_ref()
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn leaving_frames(&self) -> &[RealizedFrame] {
        match self.owner.as_ref() {
            Some(CleanupOwner::Leaving(branch)) => branch.frames(),
            Some(CleanupOwner::AuthoritySettlement(owner)) => owner
                .active
                .as_ref()
                .map(RealizedBranch::frames)
                .unwrap_or_default(),
            Some(CleanupOwner::BackNoTargetAuthority(owner)) => owner.active.frames(),
            Some(CleanupOwner::FailedPrepared(_)) | None => &[],
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn failed_prepared_frames(&self) -> &[RealizedFrame] {
        match self.owner.as_ref() {
            Some(CleanupOwner::FailedPrepared(branch)) => branch.frames(),
            Some(CleanupOwner::AuthoritySettlement(owner)) => owner.pending.branch().frames(),
            Some(CleanupOwner::Leaving(_) | CleanupOwner::BackNoTargetAuthority(_)) | None => &[],
        }
    }

    #[cfg(test)]
    pub(crate) fn authority_settlement_host(&self) -> Option<&RetiredHostBinding> {
        match self.owner.as_ref() {
            Some(CleanupOwner::AuthoritySettlement(owner)) => Some(&owner.host),
            Some(CleanupOwner::BackNoTargetAuthority(owner)) => Some(&owner.host),
            Some(CleanupOwner::Leaving(_) | CleanupOwner::FailedPrepared(_)) | None => None,
        }
    }

    pub(crate) fn mount_new_staging_allowance(
        &self,
    ) -> Option<&crate::branch_transaction::SealedMountNewStagingAllowance> {
        match self.owner.as_ref() {
            Some(CleanupOwner::FailedPrepared(branch)) => branch.mount_new_allowance(),
            Some(
                CleanupOwner::AuthoritySettlement(_)
                | CleanupOwner::BackNoTargetAuthority(_)
                | CleanupOwner::Leaving(_),
            )
            | None => None,
        }
    }

    #[cfg(feature = "test-support")]
    pub(crate) fn tracked_child_slots(&self) -> Vec<crate::branch_transaction::TrackedChildSlot> {
        match self.owner.as_ref() {
            Some(CleanupOwner::Leaving(branch)) => branch.tracked_child_slots(),
            Some(CleanupOwner::FailedPrepared(branch)) => branch.tracked_child_slots(),
            Some(CleanupOwner::AuthoritySettlement(owner)) => {
                let mut slots = owner
                    .active
                    .as_ref()
                    .map(RealizedBranch::tracked_child_slots)
                    .unwrap_or_default();
                slots.extend(owner.pending.branch().tracked_child_slots());
                slots
            }
            Some(CleanupOwner::BackNoTargetAuthority(owner)) => owner.active.tracked_child_slots(),
            None => Vec::new(),
        }
    }

    pub(crate) fn attempt(&mut self, world: &World) -> Option<CleanupAttempt> {
        if self.completion_digest.is_some() {
            return None;
        }
        let revision = self.next_attempt;
        self.next_attempt = self
            .next_attempt
            .checked_add(1)
            .expect("cleanup attempt revision exhausted");
        let attempt = CleanupAttempt {
            ledger_domain: self.ledger_domain,
            revision,
            manifest_digest: self.manifest_digest,
            unsettled_digest: unsettled_digest(
                self.manifest_digest,
                &self.obligations,
                &self.receipts,
            ),
            evidence: collect_evidence(world, &self.obligations, &self.receipts),
            mount_parents_exact: self.mount_parents_physical_match(world),
        };
        self.issued_attempt = Some(attempt.clone());
        Some(attempt)
    }

    pub(crate) const fn is_settled(&self) -> bool {
        self.completion_digest.is_some()
    }

    pub(crate) fn settle(&mut self, world: &mut World, attempt: CleanupAttempt) -> bool {
        if self.issued_attempt.as_ref() != Some(&attempt)
            || attempt.ledger_domain != self.ledger_domain
            || attempt.manifest_digest != self.manifest_digest
            || attempt.unsettled_digest
                != unsettled_digest(self.manifest_digest, &self.obligations, &self.receipts)
        {
            return false;
        }
        if self.back_no_target_authority_partition.is_some()
            && !self.back_no_target_authority_partition_is_current(world)
        {
            self.issued_attempt = None;
            return false;
        }
        let current_evidence = collect_evidence(world, &self.obligations, &self.receipts);
        if current_evidence != attempt.evidence
            || current_evidence
                .iter()
                .any(|(_, evidence)| matches!(evidence, CleanupEvidence::StaleClaim))
        {
            self.issued_attempt = None;
            return false;
        }
        if !attempt.mount_parents_exact || !self.mount_parents_physical_match(world) {
            self.issued_attempt = None;
            return false;
        }
        let focus_closure = self.focus_closure_for_attempt(
            world,
            quarantine_focus_closure(&current_evidence, &self.obligations),
        );

        if self.is_physical_authority_settlement() {
            if !self.settle_authority_evidence(world, &attempt, focus_closure) {
                self.issued_attempt = None;
                return false;
            }
            self.post_settlement_focus.clear();
            self.finish_if_settled();
            return self.completion_digest.is_some();
        }

        release_cleanup_focus(world, &focus_closure);

        if let Some(disposition) = quarantine_disposition(&attempt.evidence, &self.obligations) {
            let mount_parent_settlement_required = self.mount_parent_settlement_required()
                || attempt
                    .evidence
                    .iter()
                    .any(|(_, evidence)| matches!(evidence, CleanupEvidence::AlreadyAbsent));
            if mount_parent_settlement_required
                && self.prepare_mount_parent_exchanges(world).is_none()
            {
                self.issued_attempt = None;
                return false;
            }
            if !self.quarantine_residual(world, &attempt, disposition) {
                self.issued_attempt = None;
                return false;
            }
            if self.mount_parent_settlement_required() && !self.settle_mount_parents(world) {
                self.issued_attempt = None;
                return false;
            }
            if !focus_is_released(world, &focus_closure) {
                self.issued_attempt = None;
                return false;
            }
            self.post_settlement_focus.clear();
            self.release_settled_owner();
            self.finish_if_settled();
            return self.completion_digest.is_some();
        }

        if (self.mount_parent_settlement_required() || !attempt.evidence.is_empty())
            && self.prepare_mount_parent_exchanges(world).is_none()
        {
            self.issued_attempt = None;
            return false;
        }

        for (index, planned) in &attempt.evidence {
            if self.receipts[*index].is_some() {
                continue;
            }
            let current = classify_obligation(world, &self.obligations, &self.receipts, *index);
            if &current != planned {
                self.issued_attempt = None;
                return false;
            }
            let disposition = match current {
                CleanupEvidence::AlreadyAbsent => {
                    let registry_has_exact = world
                        .resource::<RouteEntityRegistry>()
                        .record(self.obligations[*index].claim.entity())
                        .is_some_and(|record| record.claim() == self.obligations[*index].claim);
                    if registry_has_exact
                        && !world
                            .resource_mut::<RouteEntityRegistry>()
                            .remove_exact(&self.obligations[*index].claim)
                    {
                        self.issued_attempt = None;
                        return false;
                    }
                    CleanupDisposition::AlreadyAbsent
                }
                CleanupEvidence::Exact { actual_parent, .. } => {
                    if !world.despawn(self.obligations[*index].claim.entity()) {
                        self.issued_attempt = None;
                        return false;
                    }
                    world.flush();
                    release_cleanup_focus(world, &focus_closure);
                    if !world
                        .resource_mut::<RouteEntityRegistry>()
                        .remove_exact(&self.obligations[*index].claim)
                    {
                        self.issued_attempt = None;
                        return false;
                    }
                    if actual_parent == Some(self.obligations[*index].expected_parent) {
                        CleanupDisposition::Removed
                    } else {
                        CleanupDisposition::RemovedDetached
                    }
                }
                CleanupEvidence::AuthorityLostResidual { .. } => unreachable!(
                    "authority-lost evidence must use residual settlement before removal"
                ),
                CleanupEvidence::StaleClaim => unreachable!(),
            };
            let receipt = CleanupReceipt::new(
                self.ledger_domain,
                attempt.revision,
                *index,
                disposition,
                &self.obligations[*index],
            );
            if !self.accept_receipt(&attempt, receipt) {
                self.issued_attempt = None;
                return false;
            }
            let expected_remaining = attempt
                .evidence
                .iter()
                .filter(|(remaining, _)| self.receipts[*remaining].is_none())
                .cloned()
                .collect::<Vec<_>>();
            if !focus_is_released(world, &focus_closure)
                || !self.mount_parents_physical_match(world)
                || collect_evidence(world, &self.obligations, &self.receipts) != expected_remaining
            {
                self.issued_attempt = None;
                return false;
            }
        }
        if self.mount_parent_settlement_required() && !self.settle_mount_parents(world) {
            self.issued_attempt = None;
            return false;
        }
        if !focus_is_released(world, &focus_closure) {
            self.issued_attempt = None;
            return false;
        }
        self.post_settlement_focus.clear();
        self.release_settled_owner();
        self.finish_if_settled();
        self.completion_digest.is_some()
    }

    fn prepare_authority_host_edge(
        &self,
        world: &World,
        attempt: &CleanupAttempt,
    ) -> Result<PreparedAuthorityHostEdge, ()> {
        let owner = match self.owner.as_ref() {
            Some(CleanupOwner::AuthoritySettlement(owner)) => &owner.host,
            Some(CleanupOwner::BackNoTargetAuthority(owner)) => &owner.host,
            Some(CleanupOwner::Leaving(_) | CleanupOwner::FailedPrepared(_)) | None => {
                return Err(());
            }
        };
        let Some(children_before) = self.authority_host_children.as_ref() else {
            return Err(());
        };
        let host = owner.host;
        let top_slot = owner.top_slot;
        let claim = &owner.top_slot_claim;
        if !matches!(
            claim.owner(),
            RegistryOwner::Committed(EntityOwner::HostTopSlot { host, slot })
                if host == &owner.version && *slot == owner.slot_id
        ) || owner.candidate.host() != host
        {
            return Err(());
        }
        if world.get_entity(host).is_err() {
            return Ok(PreparedAuthorityHostEdge::Missing { host });
        }
        let mut expected_after = children_before.clone();
        let membership = expected_after
            .iter()
            .filter(|child| **child == top_slot)
            .count();
        expected_after.retain(|child| *child != top_slot);
        let index = self
            .obligations
            .iter()
            .position(|obligation| obligation.claim == *claim)
            .ok_or(())?;
        let evidence = attempt
            .evidence
            .iter()
            .find(|(candidate, _)| *candidate == index)
            .map(|(_, evidence)| evidence)
            .ok_or(())?;
        let actual_parent = match evidence {
            CleanupEvidence::Exact { actual_parent, .. }
            | CleanupEvidence::AuthorityLostResidual { actual_parent, .. } => *actual_parent,
            CleanupEvidence::AlreadyAbsent => None,
            CleanupEvidence::StaleClaim => return Err(()),
        };
        if actual_parent != Some(host) {
            if observed_children(world, host) != expected_after {
                return Err(());
            }
            return Ok(PreparedAuthorityHostEdge::Existing {
                index,
                host,
                top_slot,
                claim: claim.clone(),
                children_before: children_before.clone(),
                children_after: expected_after,
                detach: false,
                require_exact: matches!(evidence, CleanupEvidence::Exact { .. }),
            });
        }
        if membership != 1 || observed_children(world, host) != *children_before {
            return Err(());
        }
        if matches!(evidence, CleanupEvidence::AuthorityLostResidual { .. }) {
            return Ok(PreparedAuthorityHostEdge::RetainedLost {
                host,
                top_slot,
                children: children_before.clone(),
            });
        }
        if !matches!(evidence, CleanupEvidence::Exact { .. })
            || !cleanup_claim_has_exact_authority(world, claim)
        {
            return Err(());
        }
        Ok(PreparedAuthorityHostEdge::Existing {
            index,
            host,
            top_slot,
            claim: claim.clone(),
            children_before: children_before.clone(),
            children_after: expected_after,
            detach: true,
            require_exact: true,
        })
    }

    fn settle_authority_evidence(
        &mut self,
        world: &mut World,
        attempt: &CleanupAttempt,
        focus_closure: Vec<Entity>,
    ) -> bool {
        debug_assert!(self.mount_parents.is_empty());
        let Some(mut prepared) = self.prepare_authority_settlement(world, attempt, &focus_closure)
        else {
            return false;
        };
        if self.issued_attempt.as_ref() != Some(attempt)
            || attempt.unsettled_digest
                != unsettled_digest(self.manifest_digest, &self.obligations, &self.receipts)
            || !prepared
                .publication_capacity
                .revalidates(world.resource::<QuarantineLeaseTable>())
            || collect_evidence(world, &self.obligations, &self.receipts)
                != prepared.prepublication_evidence
        {
            return false;
        }
        if let Some(partition) = self.back_no_target_authority_partition.as_mut() {
            if !partition.install_terminal_projections(
                &self.obligations,
                std::mem::take(&mut prepared.terminal_projections),
            ) {
                return false;
            }
            if !self.back_no_target_authority_partition_is_current(world) {
                return false;
            }
        }
        #[cfg(test)]
        let _mutation_marker = crate::registry::begin_authority_publication_mutation_for_test();
        if !prepared.pickable_repair.apply(world) {
            return false;
        }
        if self.back_no_target_authority_partition.is_some()
            && !self.back_no_target_authority_partition_is_current(world)
        {
            return false;
        }
        if !prepared.host_edge.apply_and_validate(world) {
            return false;
        }
        if let Some(partition) = self.back_no_target_authority_partition.as_mut()
            && !partition.commit_host_edge_phase(prepared.host_edge.partition_phase(), world)
        {
            return false;
        }
        if self.back_no_target_authority_partition.is_some()
            && !self.back_no_target_authority_partition_is_current(world)
        {
            return false;
        }
        release_cleanup_focus(world, &focus_closure);
        if !focus_is_released(world, &focus_closure) {
            return false;
        }

        for removal in prepared.absent {
            if let Some(retirement) = removal.retirement {
                apply_prepared_exact_registry_retirement(
                    &mut world.resource_mut::<RouteEntityRegistry>(),
                    retirement,
                );
            }
            self.commit_receipt_batch(removal.receipt);
            if self.back_no_target_authority_partition.is_some()
                && !self.back_no_target_authority_partition_is_current(world)
            {
                return false;
            }
        }

        let mut removals = prepared.removed.into_iter();
        while let Some(removal) = removals.next() {
            let completed_count = prepared.removal_order.len() - removals.len() - 1;
            let completed_before = &prepared.removal_order[..completed_count];
            let claim = &self.obligations[removal.index].claim;
            if !removal.is_current(world, &self.obligations, completed_before)
                || !world.despawn(claim.entity())
            {
                return false;
            }
            world.flush();
            apply_authority_removal_postwrite_drift(world);
            if let Some(retirement) = removal.retirement {
                apply_prepared_exact_registry_retirement(
                    &mut world.resource_mut::<RouteEntityRegistry>(),
                    retirement,
                );
            }
            let removed_is_terminal = world.get_entity(claim.entity()).is_err()
                && !world
                    .resource::<RouteEntityRegistry>()
                    .record(claim.entity())
                    .is_some_and(|record| record.claim() == *claim);
            if !removed_is_terminal {
                return false;
            }
            release_cleanup_focus(world, &focus_closure);
            self.commit_receipt_batch(removal.receipt);
            if self.back_no_target_authority_partition.is_some()
                && !self.back_no_target_authority_partition_is_current(world)
            {
                return false;
            }
            let completed_after = &prepared.removal_order[..=completed_count];
            if !focus_is_released(world, &focus_closure)
                || !removals
                    .as_slice()
                    .iter()
                    .all(|removal| removal.is_current(world, &self.obligations, completed_after))
            {
                return false;
            }
        }

        for residual in prepared.residuals {
            if !residual.is_current(world) {
                return false;
            }
            let indices = residual.indices;
            release_cleanup_focus(world, &focus_closure);
            residual.presentation.apply(world);
            apply_authority_registry_transfers(world, residual.transfers);
            self.commit_receipt_batch(residual.receipts);
            let Some(focus_roots) = self.terminal_focus_roots(world, &indices) else {
                return false;
            };
            self.remember_post_settlement_focus(focus_roots);
            apply_authority_residual_postwrite_drift(world);
            if self.back_no_target_authority_partition.is_some()
                && !self.back_no_target_authority_partition_is_current(world)
            {
                return false;
            }
            if !focus_is_released(world, &focus_closure) {
                return false;
            }
        }
        true
    }

    fn prepare_authority_settlement(
        &self,
        world: &mut World,
        attempt: &CleanupAttempt,
        _focus_closure: &[Entity],
    ) -> Option<PreparedAuthoritySettlement> {
        let host_edge = self.prepare_authority_host_edge(world, attempt).ok()?;
        let projected_attempt = host_edge.project_attempt(attempt)?;
        let dispositions = authority_settlement_dispositions(
            world,
            &self.obligations,
            &projected_attempt.evidence,
        )?;
        let groups = authority_residual_groups(world, &self.obligations, &dispositions)?;
        let verified_group_count = groups
            .iter()
            .filter(|indices| {
                indices.iter().any(|index| {
                    dispositions.iter().any(|plan| {
                        plan.index == *index
                            && plan.action == AuthoritySettlementAction::VerifiedResidual
                    })
                })
            })
            .count();
        let lost_fact_count = dispositions
            .iter()
            .filter(|plan| plan.action == AuthoritySettlementAction::LostResidual)
            .count();
        let publication_capacity = PreparedAuthorityPublicationCapacity::reserve(
            &mut world.resource_mut::<QuarantineLeaseTable>(),
            verified_group_count,
            lost_fact_count,
            groups.len(),
        )
        .ok()?;
        let verified_claims = dispositions
            .iter()
            .filter(|plan| plan.action == AuthoritySettlementAction::VerifiedResidual)
            .map(|plan| self.obligations[plan.index].claim.clone())
            .collect::<Vec<_>>();
        let pickable_repair =
            PreparedAuthorityPickableRepair::prepare(world, verified_claims.clone())?;
        let prepublication_evidence = attempt.evidence.clone();
        let mut projected_attempt = host_edge.project_attempt(attempt)?;
        if let Some(pickable) = world.components().component_id::<Pickable>() {
            projected_attempt.evidence = projected_attempt
                .evidence
                .into_iter()
                .map(|(index, evidence)| {
                    let evidence = self
                        .obligations
                        .get(index)
                        .is_some_and(|obligation| {
                            pickable_repair.repairs(obligation.claim.entity())
                        })
                        .then(|| {
                            project_cleanup_evidence_after_pickable_repair(
                                evidence.clone(),
                                pickable,
                            )
                        })
                        .unwrap_or(evidence);
                    (index, evidence)
                })
                .collect();
        }
        let projected_evidence = projected_attempt.evidence;
        let removal_entities = dispositions
            .iter()
            .filter(|plan| plan.action == AuthoritySettlementAction::Remove)
            .map(|plan| self.obligations[plan.index].claim.entity())
            .collect::<HashSet<_>>();

        let mut absent = Vec::new();
        let mut removed = Vec::new();
        absent.try_reserve_exact(dispositions.len()).ok()?;
        removed.try_reserve_exact(dispositions.len()).ok()?;
        for plan in dispositions.iter().filter(|plan| {
            matches!(
                plan.action,
                AuthoritySettlementAction::AlreadyAbsent | AuthoritySettlementAction::Remove
            )
        }) {
            let obligation = &self.obligations[plan.index];
            let receipt = self.prepare_receipt_batch(
                attempt,
                vec![CleanupReceipt::new(
                    self.ledger_domain,
                    attempt.revision,
                    plan.index,
                    plan.disposition,
                    obligation,
                )],
            )?;
            let registry_exact = world
                .resource::<RouteEntityRegistry>()
                .record(obligation.claim.entity())
                .is_some_and(|record| record.claim() == obligation.claim);
            let retirement = registry_exact
                .then(|| {
                    prepare_exact_registry_retirement(
                        world.resource::<RouteEntityRegistry>(),
                        vec![obligation.claim.clone()],
                    )
                    .ok()
                })
                .flatten();
            if plan.action == AuthoritySettlementAction::Remove && retirement.is_none() {
                return None;
            }
            let expected_parent = if plan.action == AuthoritySettlementAction::Remove {
                let (_, CleanupEvidence::Exact { actual_parent, .. }) =
                    projected_evidence
                        .iter()
                        .find(|(index, _)| *index == plan.index)?
                else {
                    return None;
                };
                *actual_parent
            } else {
                None
            };
            let prepared = PreparedAuthorityRemoval {
                index: plan.index,
                expected_parent,
                children_before: observed_children(world, obligation.claim.entity()),
                receipt,
                retirement,
            };
            match plan.action {
                AuthoritySettlementAction::AlreadyAbsent => absent.push(prepared),
                AuthoritySettlementAction::Remove => removed.push(prepared),
                AuthoritySettlementAction::VerifiedResidual
                | AuthoritySettlementAction::LostResidual => unreachable!(),
            }
        }
        removed.sort_by_key(|plan| {
            std::cmp::Reverse(authority_observed_depth(
                world,
                self.obligations[plan.index].claim.entity(),
            ))
        });
        let mut removal_order = Vec::new();
        removal_order.try_reserve_exact(removed.len()).ok()?;
        removal_order.extend(
            removed
                .iter()
                .map(|removal| self.obligations[removal.index].claim.entity()),
        );

        let mut residuals = Vec::new();
        residuals.try_reserve_exact(groups.len()).ok()?;
        let mut verified_ordinal = 0usize;
        let mut lost_offset = 0usize;
        for indices in groups {
            let mut receipt_values = Vec::new();
            receipt_values.try_reserve_exact(indices.len()).ok()?;
            for index in &indices {
                let disposition = dispositions
                    .iter()
                    .find(|plan| plan.index == *index)?
                    .disposition;
                receipt_values.push(CleanupReceipt::new(
                    self.ledger_domain,
                    attempt.revision,
                    *index,
                    disposition,
                    &self.obligations[*index],
                ));
            }
            let receipts = self.prepare_receipt_batch(attempt, receipt_values)?;
            let lost_indices = indices
                .iter()
                .copied()
                .filter(|index| {
                    dispositions.iter().any(|plan| {
                        plan.index == *index
                            && plan.action == AuthoritySettlementAction::LostResidual
                    })
                })
                .collect::<Vec<_>>();
            let lost =
                self.prepare_authority_lost_projection(world, attempt, &receipts, &lost_indices)?;
            let claims = indices
                .iter()
                .filter(|index| {
                    dispositions.iter().any(|plan| {
                        plan.index == **index
                            && plan.action == AuthoritySettlementAction::VerifiedResidual
                    })
                })
                .map(|index| self.obligations[*index].claim.clone())
                .collect::<Vec<_>>();
            let manifests = claims
                .iter()
                .map(|claim| {
                    authority_projected_manifest(
                        world,
                        claim.entity(),
                        &host_edge,
                        &removal_entities,
                        &pickable_repair,
                    )
                    .map(|manifest| (claim.entity(), manifest))
                })
                .collect::<Option<Vec<_>>>()?;
            let presentation = PreparedAuthorityPresentation::prepare(world, claims.clone())?;
            let mut group_evidence = Vec::new();
            group_evidence.try_reserve_exact(indices.len()).ok()?;
            for index in &indices {
                group_evidence.push(
                    projected_evidence
                        .iter()
                        .find(|(candidate, _)| candidate == index)?
                        .clone(),
                );
            }
            let verified_lease = if claims.is_empty() {
                None
            } else {
                publication_capacity.verified_id(verified_ordinal)
            };
            let transfers = prepare_authority_registry_transfers(
                world,
                &publication_capacity,
                &mut verified_ordinal,
                &mut lost_offset,
                claims,
                manifests.clone(),
                lost,
                quarantine_facts(&group_evidence),
            )?;
            residuals.push(PreparedAuthorityResidualGroup {
                indices,
                manifests,
                verified_lease,
                presentation,
                transfers,
                receipts,
            });
        }
        if verified_ordinal != verified_group_count || lost_offset != lost_fact_count {
            return None;
        }
        let terminal_projections = if self.back_no_target_authority_partition.is_some() {
            dispositions
                .iter()
                .map(|plan| {
                    let obligation = self.obligations.get(plan.index)?;
                    let entity = obligation.claim.entity();
                    let receipt_binding = receipt_binding(
                        self.ledger_domain,
                        attempt.revision,
                        plan.index,
                        plan.disposition,
                        obligation,
                    );
                    let (registry_claim, presence, authority) = match plan.action {
                        AuthoritySettlementAction::AlreadyAbsent
                        | AuthoritySettlementAction::Remove => (
                            None,
                            PreparedBackNoTargetAuthorityTerminalPresence::Absent,
                            PreparedBackNoTargetAuthorityTerminalAuthority::None,
                        ),
                        AuthoritySettlementAction::VerifiedResidual => {
                            let residual = residuals
                                .iter()
                                .find(|residual| residual.indices.contains(&plan.index))?;
                            let lease = residual.verified_lease?;
                            let mut lease_entities = residual
                                .indices
                                .iter()
                                .filter_map(|index| {
                                    dispositions
                                        .iter()
                                        .find(|candidate| candidate.index == *index)
                                        .is_some_and(|candidate| {
                                            candidate.action
                                                == AuthoritySettlementAction::VerifiedResidual
                                        })
                                        .then_some(self.obligations[*index].claim.entity())
                                })
                                .collect::<Vec<_>>();
                            lease_entities.sort_by_key(|entity| entity.to_bits());
                            let manifest =
                                residual
                                    .manifests
                                    .iter()
                                    .find_map(|(candidate, manifest)| {
                                        (*candidate == entity).then_some(manifest.clone())
                                    })?;
                            let parent = authority_projected_parent(world, entity, &host_edge);
                            let children =
                                authority_projected_children(world, entity, &removal_entities);
                            let ownership_seal = world.get::<RouteOwnershipSeal>(entity).copied();
                            (
                                None,
                                PreparedBackNoTargetAuthorityTerminalPresence::Retained {
                                    parent,
                                    children,
                                    ownership_seal,
                                    manifest: manifest.clone(),
                                },
                                PreparedBackNoTargetAuthorityTerminalAuthority::Quarantine {
                                    lease,
                                    lease_entities,
                                    owner: obligation.claim.owner().clone(),
                                    seal: obligation.claim.seal(),
                                    manifest,
                                },
                            )
                        }
                        AuthoritySettlementAction::LostResidual => {
                            let residual = residuals
                                .iter()
                                .find(|residual| residual.indices.contains(&plan.index))?;
                            let projection = residual
                                .transfers
                                .lost
                                .as_ref()?
                                .terminal_projections()
                                .find(|projection| projection.obligation() == plan.index)?;
                            if projection.ledger_domain() != self.ledger_domain
                                || projection.manifest_digest() != self.manifest_digest
                                || projection.attempt_revision() != attempt.revision
                                || projection.obligation_binding()
                                    != cleanup_obligation_binding(obligation)
                                || projection.terminal_receipt_binding() != receipt_binding
                                || projection.retired_claim() != &obligation.claim
                            {
                                return None;
                            }
                            let registry_claim = projection.observed_registry_claim().cloned();
                            let presence = if projection.entity_present_at_receipt() {
                                PreparedBackNoTargetAuthorityTerminalPresence::Retained {
                                    parent: authority_projected_parent(world, entity, &host_edge),
                                    children: authority_projected_children(
                                        world,
                                        entity,
                                        &removal_entities,
                                    ),
                                    ownership_seal: world
                                        .get::<RouteOwnershipSeal>(entity)
                                        .copied(),
                                    manifest: authority_projected_manifest(
                                        world,
                                        entity,
                                        &host_edge,
                                        &removal_entities,
                                        &pickable_repair,
                                    )?,
                                }
                            } else {
                                PreparedBackNoTargetAuthorityTerminalPresence::Absent
                            };
                            (
                            registry_claim,
                            presence,
                            PreparedBackNoTargetAuthorityTerminalAuthority::AuthorityLostTombstone {
                                projection,
                            },
                        )
                        }
                    };
                    Some(PreparedBackNoTargetAuthorityTerminalProjection {
                        obligation: plan.index,
                        entity,
                        ledger_domain: self.ledger_domain,
                        attempt_revision: attempt.revision,
                        disposition: plan.disposition,
                        receipt_binding,
                        registry_claim,
                        presence,
                        authority,
                    })
                })
                .collect::<Option<Vec<_>>>()?
        } else {
            Vec::new()
        };
        Some(PreparedAuthoritySettlement {
            publication_capacity,
            pickable_repair,
            prepublication_evidence,
            terminal_projections,
            host_edge,
            absent,
            removed,
            removal_order,
            residuals,
        })
    }

    fn mount_parent_settlement_required(&self) -> bool {
        self.receipts.iter().flatten().any(|receipt| {
            matches!(
                receipt.disposition,
                CleanupDisposition::AlreadyAbsent
                    | CleanupDisposition::Removed
                    | CleanupDisposition::RemovedDetached
            )
        })
    }

    fn mount_parents_physical_match(&self, world: &World) -> bool {
        let Some(registry) = world.get_resource::<RouteEntityRegistry>() else {
            return false;
        };
        self.mount_parents.iter().all(|parent| {
            mount_parent_frozen_manifest_matches(registry, parent)
                && mount_parent_physical_matches(world, registry, parent)
        })
    }

    fn prepare_mount_parent_exchanges(
        &self,
        world: &World,
    ) -> Option<Vec<(ExactRegistryClaim, ComponentManifest, ComponentManifest)>> {
        let children = world.components().component_id::<Children>();
        let registry = world.get_resource::<RouteEntityRegistry>()?;
        self.mount_parents
            .iter()
            .map(|parent| {
                if !mount_parent_physical_matches(world, registry, parent) {
                    return None;
                }
                let record = registry.record(parent.claim.entity())?;
                if record.claim() != parent.claim || record.manifest() != &parent.manifest {
                    return None;
                }
                let actual = ComponentManifest::from_world(world, parent.claim.entity())?;
                if !observed_parent_manifest_differs_only_by_children(
                    world,
                    parent.claim.entity(),
                    &parent.physical_manifest,
                ) {
                    return None;
                }
                let next = ComponentManifest::new(
                    parent
                        .manifest
                        .component_ids()
                        .iter()
                        .copied()
                        .filter(|component| Some(*component) != children)
                        .chain(
                            children.filter(|component| actual.component_ids().contains(component)),
                        ),
                );
                Some((parent.claim.clone(), parent.manifest.clone(), next))
            })
            .collect()
    }

    fn settle_mount_parents(&self, world: &mut World) -> bool {
        let Some(exchanges) = self.prepare_mount_parent_exchanges(world) else {
            return false;
        };
        if !world
            .resource_mut::<RouteEntityRegistry>()
            .compare_exchange_manifests_exact(&exchanges)
        {
            return false;
        }
        exchanges.iter().all(|(claim, _, next)| {
            let Some(parent) = self
                .mount_parents
                .iter()
                .find(|parent| parent.claim.entity() == claim.entity())
            else {
                return false;
            };
            world
                .resource::<RouteEntityRegistry>()
                .record(claim.entity())
                .is_some_and(|record| {
                    record.claim() == *claim
                        && record.manifest() == next
                        && observed_parent_manifest_differs_only_by_children(
                            world,
                            claim.entity(),
                            &parent.physical_manifest,
                        )
                })
        }) && world
            .get_resource::<RouteEntityRegistry>()
            .is_some_and(|registry| {
                self.mount_parents
                    .iter()
                    .all(|parent| mount_parent_physical_matches(world, registry, parent))
            })
    }

    fn quarantine_residual(
        &mut self,
        world: &mut World,
        attempt: &CleanupAttempt,
        disposition: CleanupDisposition,
    ) -> bool {
        let mut transfer_indices = Vec::new();
        for (index, evidence) in &attempt.evidence {
            if self.receipts[*index].is_some() {
                continue;
            }
            match evidence {
                CleanupEvidence::AlreadyAbsent => {
                    let registry_has_exact = world
                        .resource::<RouteEntityRegistry>()
                        .record(self.obligations[*index].claim.entity())
                        .is_some_and(|record| record.claim() == self.obligations[*index].claim);
                    if registry_has_exact
                        && !world
                            .resource_mut::<RouteEntityRegistry>()
                            .remove_exact(&self.obligations[*index].claim)
                    {
                        return false;
                    }
                    let receipt = CleanupReceipt::new(
                        self.ledger_domain,
                        attempt.revision,
                        *index,
                        CleanupDisposition::AlreadyAbsent,
                        &self.obligations[*index],
                    );
                    if !self.accept_receipt(attempt, receipt) {
                        return false;
                    }
                }
                CleanupEvidence::Exact { .. } | CleanupEvidence::AuthorityLostResidual { .. } => {
                    transfer_indices.push(*index)
                }
                CleanupEvidence::StaleClaim => return false,
            }
        }
        if transfer_indices.is_empty() {
            return true;
        }

        let unresolved_indices = transfer_indices
            .iter()
            .copied()
            .filter(|index| {
                match attempt
                    .evidence
                    .iter()
                    .find(|(candidate, _)| candidate == index)
                    .map(|(_, evidence)| evidence)
                {
                    Some(CleanupEvidence::AuthorityLostResidual { .. }) => false,
                    Some(CleanupEvidence::Exact { .. }) => {
                        matches!(
                            self.obligations[*index].terminal_mode,
                            CleanupTerminalMode::ForceUnresolved
                                | CleanupTerminalMode::AuthorityLost
                        ) || !cleanup_claim_has_exact_authority(
                            world,
                            &self.obligations[*index].claim,
                        )
                    }
                    _ => false,
                }
            })
            .collect::<Vec<_>>();
        let verified_indices = transfer_indices
            .iter()
            .copied()
            .filter(|index| {
                matches!(
                    attempt
                        .evidence
                        .iter()
                        .find(|(candidate, _)| candidate == index),
                    Some((_, CleanupEvidence::Exact { .. }))
                ) && !unresolved_indices.contains(index)
            })
            .collect::<Vec<_>>();
        let verified_claims = verified_indices
            .iter()
            .map(|index| self.obligations[*index].claim.clone())
            .collect::<Vec<_>>();
        let unresolved_claims = unresolved_indices
            .iter()
            .map(|index| self.obligations[*index].claim.clone())
            .collect::<Vec<_>>();
        let facts = quarantine_facts(&attempt.evidence);
        let focus_closure = quarantine_focus_closure(&attempt.evidence, &self.obligations);
        let quarantine_receipts = transfer_indices
            .iter()
            .map(|index| {
                let receipt_disposition = if matches!(
                    attempt
                        .evidence
                        .iter()
                        .find(|(candidate, _)| candidate == index),
                    Some((_, CleanupEvidence::AuthorityLostResidual { .. }))
                ) {
                    CleanupDisposition::RejectedProtected
                } else {
                    disposition
                };
                CleanupReceipt::new(
                    self.ledger_domain,
                    attempt.revision,
                    *index,
                    receipt_disposition,
                    &self.obligations[*index],
                )
            })
            .collect::<Vec<_>>();
        let Some(quarantine_receipts) = self.prepare_receipt_batch(attempt, quarantine_receipts)
        else {
            return false;
        };
        let lost_indices = transfer_indices
            .iter()
            .copied()
            .filter(|index| {
                matches!(
                    attempt
                        .evidence
                        .iter()
                        .find(|(candidate, _)| candidate == index),
                    Some((_, CleanupEvidence::AuthorityLostResidual { .. }))
                )
            })
            .collect::<Vec<_>>();
        let Some(lost_facts) = self.prepare_authority_lost_projection(
            world,
            attempt,
            &quarantine_receipts,
            &lost_indices,
        ) else {
            return false;
        };
        let repair_supported = verified_claims.is_empty()
            || repair_missing_pickable_for_cleanup_quarantine(world, &verified_claims);
        let expected_remaining = attempt
            .evidence
            .iter()
            .filter(|(index, _)| self.receipts[*index].is_none())
            .cloned()
            .collect::<Vec<_>>();
        let refreshed = collect_evidence(world, &self.obligations, &self.receipts);
        let evidence_is_exact = refreshed == expected_remaining
            || world
                .components()
                .component_id::<Pickable>()
                .is_some_and(|pickable| {
                    evidence_matches_after_pickable_repair(
                        &expected_remaining,
                        &refreshed,
                        &transfer_indices,
                        pickable,
                    )
                });
        let repair_outcome = if !evidence_is_exact || !self.mount_parents_physical_match(world) {
            PickableRepairOutcome::Superseded
        } else if repair_supported {
            PickableRepairOutcome::ExactAfterRepair
        } else {
            PickableRepairOutcome::Unsupported
        };
        if repair_outcome == PickableRepairOutcome::Superseded {
            return false;
        }
        let transferred = repair_outcome == PickableRepairOutcome::ExactAfterRepair
            && unresolved_indices.iter().all(|index| {
                self.obligations[*index].terminal_mode == CleanupTerminalMode::AuthorityLost
                    || unresolved_obligation_is_isolated(world, &self.obligations[*index])
            })
            && transfer_cleanup_quarantine(
                world,
                verified_claims,
                unresolved_claims,
                lost_facts,
                facts,
                focus_closure.clone(),
                self,
                quarantine_receipts,
            );
        if !transferred {
            return false;
        }
        if self.is_authority_settlement() {
            return true;
        }
        let Some(focus_roots) = self.terminal_focus_roots(world, &transfer_indices) else {
            return false;
        };
        self.remember_post_settlement_focus(focus_roots);
        #[cfg(test)]
        apply_cleanup_postcheck_drift(world, CleanupPostcheckPhase::VerifiedTransfer);
        release_cleanup_focus(world, &focus_closure);
        if !focus_is_released(world, &focus_closure) || !self.mount_parents_physical_match(world) {
            return false;
        }
        true
    }

    fn focus_closure_for_attempt(&self, world: &World, mut closure: Vec<Entity>) -> Vec<Entity> {
        let blocked = self
            .post_settlement_focus
            .iter()
            .filter(|root| !self.post_settlement_focus_root_is_current(world, root))
            .map(|root| root.terminal_claim.entity())
            .collect::<HashSet<_>>();
        let mut visited = HashSet::new();
        let mut stack = self
            .post_settlement_focus
            .iter()
            .filter(|root| self.post_settlement_focus_root_is_current(world, root))
            .map(|root| root.terminal_claim.entity())
            .collect::<Vec<_>>();
        while let Some(entity) = stack.pop() {
            if blocked.contains(&entity) || !visited.insert(entity) {
                continue;
            }
            closure.push(entity);
            if let Some(children) = world.get::<Children>(entity) {
                stack.extend(children.iter().copied());
            }
        }
        closure.sort_by_key(|entity| entity.to_bits());
        closure.dedup();
        closure
    }

    fn terminal_focus_roots(
        &self,
        world: &World,
        indices: &[usize],
    ) -> Option<Vec<PostSettlementFocusRoot>> {
        let table = world.get_resource::<QuarantineLeaseTable>()?;
        let mut roots = Vec::new();
        for index in indices {
            let obligation = self.obligations.get(*index)?;
            let entity = obligation.claim.entity();
            if self
                .receipts
                .get(*index)
                .and_then(Option::as_ref)
                .is_some_and(|receipt| receipt.disposition == CleanupDisposition::RejectedProtected)
            {
                if table
                    .authority_lost_tombstone(entity)
                    .is_some_and(|tombstone| tombstone.former_claim() == &obligation.claim)
                    || table
                        .unresolved_record(entity)
                        .is_some_and(|record| record.claim() == obligation.claim)
                {
                    // A lost tombstone is not current authority and therefore
                    // must not become a post-settlement focus root.
                    continue;
                }
                return None;
            }
            let root = match (table.record(entity), table.unresolved_record(entity)) {
                (Some(record), None)
                    if record.owner() == obligation.claim.owner()
                        && record.seal() == obligation.claim.seal()
                        && record.lifecycle() == EntityLifecycle::Quarantined =>
                {
                    PostSettlementFocusRoot {
                        obligation: *index,
                        source_claim: obligation.claim.clone(),
                        terminal_claim: record.claim(),
                        authority: PostSettlementFocusAuthority::Quarantine,
                        allow_missing_components: obligation.allow_missing_components,
                    }
                }
                (None, Some(record)) if record.claim() == obligation.claim => {
                    PostSettlementFocusRoot {
                        obligation: *index,
                        source_claim: obligation.claim.clone(),
                        terminal_claim: record.claim(),
                        authority: if obligation.terminal_mode == CleanupTerminalMode::AuthorityLost
                        {
                            PostSettlementFocusAuthority::RetiredAuthority
                        } else {
                            PostSettlementFocusAuthority::Unresolved
                        },
                        allow_missing_components: obligation.allow_missing_components,
                    }
                }
                _ => return None,
            };
            roots.push(root);
        }
        Some(roots)
    }

    fn post_settlement_focus_root_is_current(
        &self,
        world: &World,
        root: &PostSettlementFocusRoot,
    ) -> bool {
        let receipt_is_terminal = self
            .receipts
            .get(root.obligation)
            .and_then(Option::as_ref)
            .is_some_and(|receipt| receipt.disposition.is_quarantine());
        if !receipt_is_terminal {
            return false;
        }
        let Some(table) = world.get_resource::<QuarantineLeaseTable>() else {
            return false;
        };
        match root.authority {
            PostSettlementFocusAuthority::Quarantine => {
                table
                    .record(root.terminal_claim.entity())
                    .is_some_and(|record| record.claim() == root.terminal_claim)
                    && root.terminal_claim.owner() == root.source_claim.owner()
                    && root.terminal_claim.seal() == root.source_claim.seal()
                    && root.terminal_claim.lifecycle() == EntityLifecycle::Quarantined
                    && terminal_quarantine_root_is_inert(world, &root.terminal_claim)
            }
            PostSettlementFocusAuthority::Unresolved => {
                table
                    .unresolved_record(root.terminal_claim.entity())
                    .is_some_and(|record| record.claim() == root.terminal_claim)
                    && root.terminal_claim == root.source_claim
                    && terminal_unresolved_root_is_inert(
                        world,
                        &root.source_claim,
                        root.allow_missing_components,
                    )
            }
            PostSettlementFocusAuthority::RetiredAuthority => {
                table
                    .unresolved_record(root.terminal_claim.entity())
                    .is_some_and(|record| record.claim() == root.terminal_claim)
                    && !cleanup_claim_has_exact_authority(world, &root.terminal_claim)
            }
        }
    }

    fn remember_post_settlement_focus(&mut self, roots: Vec<PostSettlementFocusRoot>) {
        self.post_settlement_focus.extend(roots);
        self.post_settlement_focus
            .sort_by_key(|root| root.obligation);
        self.post_settlement_focus
            .dedup_by_key(|root| root.obligation);
    }

    fn prepare_receipt_batch(
        &self,
        attempt: &CleanupAttempt,
        receipts: Vec<CleanupReceipt>,
    ) -> Option<PreparedCleanupReceiptBatch> {
        if self.issued_attempt.as_ref() != Some(attempt) {
            return None;
        }
        let mut obligations = HashSet::with_capacity(receipts.len());
        for receipt in &receipts {
            if receipt.ledger_domain != self.ledger_domain
                || receipt.attempt_revision != attempt.revision
                || receipt.obligation >= self.obligations.len()
                || self.receipts[receipt.obligation].is_some()
                || !obligations.insert(receipt.obligation)
                || receipt.binding
                    != receipt_binding(
                        receipt.ledger_domain,
                        receipt.attempt_revision,
                        receipt.obligation,
                        receipt.disposition,
                        &self.obligations[receipt.obligation],
                    )
            {
                return None;
            }
        }
        Some(PreparedCleanupReceiptBatch { receipts })
    }

    fn prepare_authority_lost_projection(
        &self,
        world: &World,
        attempt: &CleanupAttempt,
        receipts: &PreparedCleanupReceiptBatch,
        indices: &[usize],
    ) -> Option<PreparedAuthorityLostResidualProjection> {
        if self.issued_attempt.as_ref() != Some(attempt) {
            return None;
        }
        let mut facts = Vec::new();
        facts.try_reserve_exact(indices.len()).ok()?;
        for index in indices {
            let (
                _,
                CleanupEvidence::AuthorityLostResidual {
                    actual_parent,
                    registry_exact,
                    entity_present,
                },
            ) = attempt
                .evidence
                .iter()
                .find(|(candidate, _)| candidate == index)?
            else {
                return None;
            };
            let receipt = receipts
                .receipts
                .iter()
                .find(|receipt| receipt.obligation == *index)?;
            let obligation = self.obligations.get(*index)?;
            if receipt.disposition != CleanupDisposition::RejectedProtected {
                return None;
            }
            let observed_registry_claim = world
                .resource::<RouteEntityRegistry>()
                .record(obligation.claim.entity())
                .map(|record| record.claim())
                .filter(|claim| *claim != obligation.claim);
            facts.push(AuthorityLostResidualProjectionFact {
                ledger_domain: self.ledger_domain,
                manifest_digest: self.manifest_digest,
                attempt_revision: attempt.revision,
                obligation: *index,
                obligation_binding: cleanup_obligation_binding(obligation),
                terminal_receipt_binding: receipt.binding,
                retired_claim: obligation.claim.clone(),
                frozen_expected_manifest: obligation.manifest.clone(),
                observed_manifest: ComponentManifest::from_world(world, obligation.claim.entity()),
                entity_present_at_receipt: *entity_present,
                observed_registry_claim,
                actual_parent_at_receipt: *actual_parent,
                registry_exact_at_receipt: *registry_exact,
            });
        }
        Some(PreparedAuthorityLostResidualProjection { facts })
    }

    fn commit_receipt_batch(&mut self, prepared: PreparedCleanupReceiptBatch) {
        for receipt in prepared.receipts {
            let obligation = receipt.obligation;
            debug_assert!(self.receipts[obligation].is_none());
            self.receipts[obligation] = Some(receipt);
        }
    }

    fn accept_receipt(&mut self, attempt: &CleanupAttempt, receipt: CleanupReceipt) -> bool {
        let Some(prepared) = self.prepare_receipt_batch(attempt, vec![receipt]) else {
            return false;
        };
        self.commit_receipt_batch(prepared);
        true
    }

    fn finish_if_settled(&mut self) {
        if self.receipts.iter().all(Option::is_some) {
            self.completion_digest = Some(completion_digest(&self.receipts));
            self.issued_attempt = None;
        }
    }

    #[cfg(test)]
    pub(crate) fn receipt_count(&self) -> usize {
        self.receipts.iter().flatten().count()
    }

    pub(crate) fn next_unobserved_receipt(&self) -> Option<(usize, CleanupDisposition)> {
        self.receipts
            .iter()
            .enumerate()
            .find_map(|(index, receipt)| {
                receipt
                    .as_ref()
                    .filter(|receipt| !receipt.observed)
                    .map(|receipt| (index, receipt.disposition))
            })
    }

    pub(crate) fn mark_receipt_observed(
        &mut self,
        obligation: usize,
        disposition: CleanupDisposition,
    ) -> bool {
        let Some(receipt) = self.receipts.get_mut(obligation).and_then(Option::as_mut) else {
            return false;
        };
        if receipt.observed || receipt.disposition != disposition {
            return false;
        }
        receipt.observed = true;
        true
    }

    pub(crate) fn mark_quarantine_event_observed(
        &mut self,
        disposition: CleanupDisposition,
    ) -> bool {
        if !disposition.is_quarantine() || self.quarantine_event_observed {
            return false;
        }
        self.quarantine_event_observed = true;
        true
    }

    #[cfg(test)]
    fn disposition(&self, obligation: usize) -> Option<CleanupDisposition> {
        self.receipts
            .get(obligation)
            .and_then(Option::as_ref)
            .map(|receipt| receipt.disposition)
    }
}

fn prepare_authority_registry_transfers(
    world: &mut World,
    capacity: &PreparedAuthorityPublicationCapacity,
    verified_ordinal: &mut usize,
    lost_offset: &mut usize,
    verified_claims: Vec<ExactRegistryClaim>,
    verified_manifests: Vec<(Entity, ComponentManifest)>,
    lost_facts: PreparedAuthorityLostResidualProjection,
    facts: QuarantineFacts,
) -> Option<PreparedAuthorityRegistryTransfers> {
    let lost_count = lost_facts.facts().len();
    world.resource_scope(
        |world, registry: bevy_ecs::change_detection::Mut<RouteEntityRegistry>| {
            world.resource_scope(
                |_world, mut table: bevy_ecs::change_detection::Mut<QuarantineLeaseTable>| {
                    let verified = if verified_claims.is_empty() {
                        None
                    } else {
                        let id = capacity.verified_id(*verified_ordinal)?;
                        let transfer = VerifiedCleanupQuarantineTransfer::after_world_validation(
                            verified_claims,
                            verified_manifests,
                            facts.clone(),
                        );
                        Some(
                            prepare_verified_authority_cleanup_quarantine_transfer(
                                &mut table, &registry, transfer, id,
                            )
                            .ok()?,
                        )
                    };
                    let lost = if lost_facts.is_empty() {
                        None
                    } else {
                        let first_id = capacity.residual_id(*lost_offset)?;
                        Some(
                            prepare_authority_lost_residual_transfer_in_publication(
                                &mut table, &registry, lost_facts, facts, first_id,
                            )
                            .ok()?,
                        )
                    };
                    *verified_ordinal += usize::from(verified.is_some());
                    *lost_offset = (*lost_offset).checked_add(lost_count)?;
                    Some(PreparedAuthorityRegistryTransfers { verified, lost })
                },
            )
        },
    )
}

fn apply_authority_registry_transfers(
    world: &mut World,
    prepared: PreparedAuthorityRegistryTransfers,
) {
    world.resource_scope(
        |world, mut registry: bevy_ecs::change_detection::Mut<RouteEntityRegistry>| {
            world.resource_scope(
                |_world, mut table: bevy_ecs::change_detection::Mut<QuarantineLeaseTable>| {
                    if let Some(transfer) = prepared.verified {
                        apply_prepared_quarantine_transfer(&mut table, &mut registry, transfer);
                    }
                    if let Some(transfer) = prepared.lost {
                        apply_prepared_authority_lost_residual_transfer(
                            &mut table,
                            &mut registry,
                            transfer,
                        );
                    }
                },
            );
        },
    );
}

fn transfer_cleanup_quarantine(
    world: &mut World,
    verified_claims: Vec<ExactRegistryClaim>,
    unresolved_claims: Vec<ExactRegistryClaim>,
    lost_facts: PreparedAuthorityLostResidualProjection,
    facts: QuarantineFacts,
    focus_closure: Vec<Entity>,
    ledger: &mut CleanupLedger,
    receipts: PreparedCleanupReceiptBatch,
) -> bool {
    let mut verified_manifests = Vec::with_capacity(verified_claims.len());
    for claim in &verified_claims {
        let Some(manifest) = ComponentManifest::from_world(world, claim.entity()) else {
            return false;
        };
        verified_manifests.push((claim.entity(), manifest));
    }
    let mut unresolved_manifests = Vec::with_capacity(unresolved_claims.len());
    for claim in &unresolved_claims {
        let Some(manifest) = ComponentManifest::from_world(world, claim.entity()).or_else(|| {
            world
                .resource::<RouteEntityRegistry>()
                .record(claim.entity())
                .filter(|record| record.claim() == *claim)
                .map(|record| record.manifest().clone())
        }) else {
            return false;
        };
        unresolved_manifests.push((claim.entity(), manifest));
    }
    world.resource_scope(
        |world, mut registry: bevy_ecs::change_detection::Mut<RouteEntityRegistry>| {
            world.resource_scope(
                |world, mut table: bevy_ecs::change_detection::Mut<QuarantineLeaseTable>| {
                    let verified = if verified_claims.is_empty() {
                        None
                    } else {
                        let transfer = VerifiedCleanupQuarantineTransfer::after_world_validation(
                            verified_claims.clone(),
                            verified_manifests,
                            facts.clone(),
                        );
                        let Ok(transfer) = prepare_verified_cleanup_quarantine_transfer(
                            &mut table, &registry, transfer,
                        ) else {
                            return false;
                        };
                        Some(transfer)
                    };
                    let unresolved = if unresolved_claims.is_empty() {
                        None
                    } else {
                        let Ok(transfer) = prepare_unresolved_residual_transfer(
                            &mut table,
                            &registry,
                            unresolved_claims,
                            unresolved_manifests,
                            facts.clone(),
                        ) else {
                            return false;
                        };
                        Some(transfer)
                    };
                    let lost = if lost_facts.is_empty() {
                        None
                    } else {
                        let Ok(prepared) = prepare_authority_lost_residual_transfer(
                            &mut table, &registry, lost_facts, facts,
                        ) else {
                            return false;
                        };
                        Some(prepared)
                    };
                    world.resource_scope(
                        |world, input_focus: bevy_ecs::change_detection::Mut<InputFocus>| {
                            world.resource_scope(
                                |world,
                                 route_focus: bevy_ecs::change_detection::Mut<RouteFocusState>| {
                                    let focus = prepare_focus_mutation(
                                        input_focus,
                                        route_focus,
                                        &focus_closure,
                                    );
                                    let Some(presentation) =
                                        prepare_cleanup_quarantine_mutation(
                                            world,
                                            &verified_claims,
                                        )
                                    else {
                                        return false;
                                    };
                                    presentation.apply();
                                    focus.apply();
                                    if let Some(transfer) = verified {
                                        apply_prepared_quarantine_transfer(
                                            &mut table,
                                            &mut registry,
                                            transfer,
                                        );
                                    }
                                    if let Some(transfer) = unresolved {
                                        apply_prepared_unresolved_residual_transfer(
                                            &mut table,
                                            &mut registry,
                                            transfer,
                                        );
                                    }
                                    if let Some(prepared) = lost {
                                        apply_prepared_authority_lost_residual_transfer(
                                            &mut table,
                                            &mut registry,
                                            prepared,
                                        );
                                    }
                                    // All capacity, identity, presentation, and focus work was
                                    // prepared above. Publish the residual partition and its
                                    // exact terminal receipts in one infallible commit tail.
                                    ledger.commit_receipt_batch(receipts);
                                    true
                                },
                            )
                        },
                    )
                },
            )
        },
    )
}

struct PreparedCleanupQuarantineEntity<'w> {
    visibility: Mut<'w, Visibility>,
    pickable: Mut<'w, Pickable>,
    barrier: Mut<'w, RouterInputBarrier>,
    lifecycle: PreparedCleanupQuarantineLifecycle<'w>,
}

enum PreparedCleanupQuarantineLifecycle<'w> {
    Frame(Mut<'w, FrameLifecycle>),
    ChildSlot(Mut<'w, ChildSlotLifecycle>),
    HostTopSlot(Mut<'w, ProvisionalSlotLifecycle>),
}

struct PreparedCleanupQuarantineMutation<'w> {
    entities: Vec<(usize, PreparedCleanupQuarantineEntity<'w>)>,
}

impl PreparedCleanupQuarantineMutation<'_> {
    fn apply(self) {
        for (_, mut entity) in self.entities {
            *entity.visibility = Visibility::Hidden;
            *entity.pickable = Pickable::IGNORE;
            *entity.barrier = RouterInputBarrier::deny_all();
            match entity.lifecycle {
                PreparedCleanupQuarantineLifecycle::Frame(mut lifecycle) => {
                    *lifecycle = FrameLifecycle(EntityLifecycle::Quarantined);
                }
                PreparedCleanupQuarantineLifecycle::ChildSlot(mut lifecycle) => {
                    *lifecycle = ChildSlotLifecycle::Quarantined;
                }
                PreparedCleanupQuarantineLifecycle::HostTopSlot(mut lifecycle) => {
                    *lifecycle = ProvisionalSlotLifecycle::Quarantined;
                }
            }
        }
    }
}

fn prepare_cleanup_quarantine_mutation<'w>(
    world: &'w mut World,
    claims: &[ExactRegistryClaim],
) -> Option<PreparedCleanupQuarantineMutation<'w>> {
    let mut query = world.query::<(
        Entity,
        &mut Visibility,
        &mut Pickable,
        &mut RouterInputBarrier,
        Option<&mut FrameLifecycle>,
        Option<&mut ChildSlotLifecycle>,
        Option<&mut ProvisionalSlotLifecycle>,
        Option<&ChildSlotAuthority>,
        &RouteOwnershipSeal,
    )>();
    let mut found = vec![false; claims.len()];
    let mut entities = Vec::with_capacity(claims.len());
    for (
        entity,
        visibility,
        pickable,
        barrier,
        frame_lifecycle,
        child_lifecycle,
        top_slot_lifecycle,
        child_authority,
        seal,
    ) in query.iter_mut(world)
    {
        let Some((index, claim)) = claims
            .iter()
            .enumerate()
            .find(|(_, claim)| claim.entity() == entity)
        else {
            continue;
        };
        let lifecycle = match claim.owner() {
            RegistryOwner::Committed(
                EntityOwner::FrameShell(_)
                | EntityOwner::FrameContentTarget(_)
                | EntityOwner::FrameOutput { .. },
            ) => PreparedCleanupQuarantineLifecycle::Frame(
                frame_lifecycle.filter(|lifecycle| lifecycle.0 == claim.lifecycle())?,
            ),
            RegistryOwner::Committed(EntityOwner::ChildSlot { .. })
            | RegistryOwner::Provisional(ProvisionalOwner::ChildSlot { .. }) => {
                if !child_slot_components_match_claim(
                    child_lifecycle.as_deref(),
                    child_authority,
                    claim,
                ) {
                    return None;
                }
                PreparedCleanupQuarantineLifecycle::ChildSlot(child_lifecycle?)
            }
            RegistryOwner::Committed(EntityOwner::HostTopSlot { .. }) => {
                PreparedCleanupQuarantineLifecycle::HostTopSlot(
                    top_slot_lifecycle
                        .filter(|lifecycle| lifecycle_is_exact(**lifecycle, claim.lifecycle()))?,
                )
            }
            _ => return None,
        };
        if found[index]
            || seal != &RouteOwnershipSeal::new(claim.seal())
            || *visibility != Visibility::Hidden
            || *pickable != Pickable::IGNORE
            || !barrier.denies_all()
        {
            return None;
        }
        found[index] = true;
        entities.push((
            index,
            PreparedCleanupQuarantineEntity {
                visibility,
                pickable,
                barrier,
                lifecycle,
            },
        ));
    }
    entities.sort_by_key(|(index, _)| *index);
    found
        .into_iter()
        .all(|present| present)
        .then_some(PreparedCleanupQuarantineMutation { entities })
}

fn repair_missing_pickable_for_cleanup_quarantine(
    world: &mut World,
    claims: &[ExactRegistryClaim],
) -> bool {
    if !claims
        .iter()
        .all(|claim| cleanup_claim_is_inert_and_exact(world, claim, true))
    {
        return false;
    }
    let missing = claims
        .iter()
        .filter_map(|claim| {
            world
                .get::<Pickable>(claim.entity())
                .is_none()
                .then_some(claim.entity())
        })
        .collect::<Vec<_>>();
    for entity in missing {
        world.entity_mut(entity).insert(Pickable::IGNORE);
    }
    world.flush();
    claims
        .iter()
        .all(|claim| cleanup_claim_is_inert_and_exact(world, claim, false))
}

fn cleanup_claim_is_inert_and_exact(
    world: &World,
    claim: &ExactRegistryClaim,
    allow_missing_pickable: bool,
) -> bool {
    let frame_is_exact = world
        .get::<FrameLifecycle>(claim.entity())
        .is_some_and(|lifecycle| lifecycle.0 == claim.lifecycle());
    let child_slot_is_exact = child_slot_components_match_claim(
        world.get::<ChildSlotLifecycle>(claim.entity()),
        world.get::<ChildSlotAuthority>(claim.entity()),
        claim,
    );
    let top_slot_is_exact = matches!(
        claim.owner(),
        RegistryOwner::Committed(EntityOwner::HostTopSlot { .. })
    ) && world
        .get::<ProvisionalSlotLifecycle>(claim.entity())
        .is_some_and(|lifecycle| lifecycle_is_exact(*lifecycle, claim.lifecycle()));
    world
        .resource::<RouteEntityRegistry>()
        .record(claim.entity())
        .is_some_and(|record| record.claim() == *claim)
        && world.get::<RouteOwnershipSeal>(claim.entity())
            == Some(&RouteOwnershipSeal::new(claim.seal()))
        && world.get::<Visibility>(claim.entity()) == Some(&Visibility::Hidden)
        && world
            .get::<RouterInputBarrier>(claim.entity())
            .is_some_and(RouterInputBarrier::denies_all)
        && (allow_missing_pickable && world.get::<Pickable>(claim.entity()).is_none()
            || world.get::<Pickable>(claim.entity()) == Some(&Pickable::IGNORE))
        && (frame_is_exact || child_slot_is_exact || top_slot_is_exact)
}

fn lifecycle_is_exact(physical: ProvisionalSlotLifecycle, registered: EntityLifecycle) -> bool {
    matches!(
        (physical, registered),
        (
            ProvisionalSlotLifecycle::PreparedInert,
            EntityLifecycle::PreparedInert
        ) | (
            ProvisionalSlotLifecycle::Quarantined,
            EntityLifecycle::Quarantined
        )
    )
}

pub(crate) fn cleanup_claim_has_exact_authority(world: &World, claim: &ExactRegistryClaim) -> bool {
    let seal_is_exact = world.get::<RouteOwnershipSeal>(claim.entity())
        == Some(&RouteOwnershipSeal::new(claim.seal()));
    let lifecycle_is_exact = match claim.owner() {
        RegistryOwner::Committed(
            EntityOwner::FrameShell(_)
            | EntityOwner::FrameContentTarget(_)
            | EntityOwner::FrameOutput { .. },
        ) => {
            world.get::<FrameLifecycle>(claim.entity()) == Some(&FrameLifecycle(claim.lifecycle()))
        }
        RegistryOwner::Committed(EntityOwner::ChildSlot { .. })
        | RegistryOwner::Provisional(ProvisionalOwner::ChildSlot { .. }) => {
            child_slot_components_match_claim(
                world.get::<ChildSlotLifecycle>(claim.entity()),
                world.get::<ChildSlotAuthority>(claim.entity()),
                claim,
            )
        }
        RegistryOwner::Committed(EntityOwner::HostTopSlot { .. }) => world
            .get::<ProvisionalSlotLifecycle>(claim.entity())
            .is_some_and(|lifecycle| lifecycle_is_exact(*lifecycle, claim.lifecycle())),
        _ => false,
    };
    seal_is_exact && lifecycle_is_exact
}

fn terminal_quarantine_root_is_inert(world: &World, claim: &ExactRegistryClaim) -> bool {
    cleanup_claim_has_exact_authority(world, claim)
        && world.get::<Visibility>(claim.entity()) == Some(&Visibility::Hidden)
        && world.get::<Pickable>(claim.entity()) == Some(&Pickable::IGNORE)
        && world
            .get::<RouterInputBarrier>(claim.entity())
            .is_some_and(RouterInputBarrier::denies_all)
}

fn terminal_authority_lost_entity_is_inert(world: &World, entity: Entity) -> bool {
    world.get::<Visibility>(entity) == Some(&Visibility::Hidden)
        && world.get::<Pickable>(entity) == Some(&Pickable::IGNORE)
        && world
            .get::<RouterInputBarrier>(entity)
            .is_some_and(RouterInputBarrier::denies_all)
}

fn terminal_unresolved_root_is_inert(
    world: &World,
    claim: &ExactRegistryClaim,
    allow_missing_components: bool,
) -> bool {
    let seal_is_compatible = world
        .get::<RouteOwnershipSeal>(claim.entity())
        .map(|seal| seal == &RouteOwnershipSeal::new(claim.seal()))
        .unwrap_or(allow_missing_components);
    let authority_is_compatible = match claim.owner() {
        RegistryOwner::Committed(
            EntityOwner::FrameShell(_)
            | EntityOwner::FrameContentTarget(_)
            | EntityOwner::FrameOutput { .. },
        ) => world
            .get::<FrameLifecycle>(claim.entity())
            .map(|lifecycle| lifecycle == &FrameLifecycle(claim.lifecycle()))
            .unwrap_or(allow_missing_components),
        RegistryOwner::Committed(EntityOwner::ChildSlot { .. })
        | RegistryOwner::Provisional(ProvisionalOwner::ChildSlot { .. }) => {
            child_slot_components_are_compatible(
                world.get::<ChildSlotLifecycle>(claim.entity()),
                world.get::<ChildSlotAuthority>(claim.entity()),
                claim,
                allow_missing_components,
            )
        }
        RegistryOwner::Committed(EntityOwner::HostTopSlot { .. }) => world
            .get::<ProvisionalSlotLifecycle>(claim.entity())
            .map(|lifecycle| lifecycle_is_exact(*lifecycle, claim.lifecycle()))
            .unwrap_or(allow_missing_components),
        _ => false,
    };
    world.get_entity(claim.entity()).is_ok()
        && seal_is_compatible
        && authority_is_compatible
        && world.get::<Visibility>(claim.entity()) == Some(&Visibility::Hidden)
        && world.get::<Pickable>(claim.entity()) == Some(&Pickable::IGNORE)
        && world
            .get::<RouterInputBarrier>(claim.entity())
            .is_some_and(RouterInputBarrier::denies_all)
}

pub(crate) fn failed_prepared_branch_roots_are_exact(
    world: &World,
    branch: &FailedPreparedBranch,
    manifest: &FrozenCleanupManifest,
) -> bool {
    let Some(registry) = world.get_resource::<RouteEntityRegistry>() else {
        return false;
    };
    let claim_is_exact = |claim: &ExactRegistryClaim| {
        registry
            .record(claim.entity())
            .is_some_and(|record| record.claim() == *claim)
            && cleanup_claim_has_exact_authority(world, claim)
    };
    let claim_is_registered = |claim: &ExactRegistryClaim| {
        registry
            .record(claim.entity())
            .is_some_and(|record| record.claim() == *claim)
    };
    branch
        .frames()
        .iter()
        .chain(branch.detached_frames())
        .flat_map(RealizedFrame::claims)
        .all(|claim| claim_is_exact(claim) || manifest.allows_pending_boundary_loss(claim))
        && branch.child_slots().iter().all(|slot| {
            claim_is_exact(slot.slot_claim())
                || manifest.allows_pending_boundary_loss(slot.slot_claim())
        })
        && branch.mount_residuals().iter().all(|residual| {
            (claim_is_registered(residual.claim())
                && residual_child_slot_is_isolated(world, residual))
                || manifest.allows_pending_boundary_loss(residual.claim())
        })
        && branch.rejected_child_slots().iter().all(|rejected| {
            claim_is_exact(rejected.slot().slot_claim())
                || manifest.allows_pending_boundary_loss(rejected.slot().slot_claim())
        })
}

fn unresolved_obligation_is_isolated(world: &World, obligation: &CleanupObligation) -> bool {
    let claim = &obligation.claim;
    let record_is_exact = world
        .get_resource::<RouteEntityRegistry>()
        .and_then(|registry| registry.record(claim.entity()))
        .is_some_and(|record| record.claim() == *claim);
    let seal_is_compatible = world
        .get::<RouteOwnershipSeal>(claim.entity())
        .map(|seal| seal == &RouteOwnershipSeal::new(claim.seal()))
        .unwrap_or(obligation.allow_missing_components);
    let authority_is_compatible = match claim.owner() {
        RegistryOwner::Committed(
            EntityOwner::FrameShell(_)
            | EntityOwner::FrameContentTarget(_)
            | EntityOwner::FrameOutput { .. },
        ) => world
            .get::<FrameLifecycle>(claim.entity())
            .map(|lifecycle| lifecycle == &FrameLifecycle(claim.lifecycle()))
            .unwrap_or(obligation.allow_missing_components),
        RegistryOwner::Committed(EntityOwner::ChildSlot { .. })
        | RegistryOwner::Provisional(ProvisionalOwner::ChildSlot { .. }) => {
            child_slot_components_are_compatible(
                world.get::<ChildSlotLifecycle>(claim.entity()),
                world.get::<ChildSlotAuthority>(claim.entity()),
                claim,
                obligation.allow_missing_components,
            )
        }
        _ => false,
    };
    record_is_exact
        && ComponentManifest::from_world(world, claim.entity()).is_some()
        && seal_is_compatible
        && authority_is_compatible
        && world.get::<Visibility>(claim.entity()) == Some(&Visibility::Hidden)
        && world.get::<Pickable>(claim.entity()) == Some(&Pickable::IGNORE)
        && world
            .get::<RouterInputBarrier>(claim.entity())
            .is_some_and(RouterInputBarrier::denies_all)
}

fn child_slot_components_match_claim(
    lifecycle: Option<&ChildSlotLifecycle>,
    authority: Option<&ChildSlotAuthority>,
    claim: &ExactRegistryClaim,
) -> bool {
    let expected_lifecycle = match (claim.owner(), claim.lifecycle()) {
        (
            RegistryOwner::Provisional(ProvisionalOwner::ChildSlot { .. }),
            EntityLifecycle::PreparedInert,
        ) => ChildSlotLifecycle::ProvisionalInert,
        (
            RegistryOwner::Committed(EntityOwner::ChildSlot { .. }),
            EntityLifecycle::PreparedInert,
        ) => ChildSlotLifecycle::RegisteredInert,
        (RegistryOwner::Committed(EntityOwner::ChildSlot { .. }), EntityLifecycle::Active) => {
            ChildSlotLifecycle::Active
        }
        (RegistryOwner::Committed(EntityOwner::ChildSlot { .. }), EntityLifecycle::Leaving) => {
            ChildSlotLifecycle::Leaving
        }
        (
            RegistryOwner::Provisional(ProvisionalOwner::ChildSlot { .. })
            | RegistryOwner::Committed(EntityOwner::ChildSlot { .. }),
            EntityLifecycle::Quarantined,
        ) => ChildSlotLifecycle::Quarantined,
        _ => return false,
    };
    let (parent, outlet, slot) = match claim.owner() {
        RegistryOwner::Provisional(ProvisionalOwner::ChildSlot {
            parent,
            outlet,
            slot,
        })
        | RegistryOwner::Committed(EntityOwner::ChildSlot {
            parent,
            outlet,
            slot,
        }) => (parent, outlet, *slot),
        _ => return false,
    };
    lifecycle == Some(&expected_lifecycle)
        && authority.is_some_and(|authority| {
            authority.parent() == parent && authority.outlet() == outlet && authority.slot() == slot
        })
}

fn child_slot_components_are_compatible(
    lifecycle: Option<&ChildSlotLifecycle>,
    authority: Option<&ChildSlotAuthority>,
    claim: &ExactRegistryClaim,
    allow_missing: bool,
) -> bool {
    let (expected_lifecycle, parent, outlet, slot) = match claim.owner() {
        RegistryOwner::Provisional(ProvisionalOwner::ChildSlot {
            parent,
            outlet,
            slot,
        }) if claim.lifecycle() == EntityLifecycle::PreparedInert => {
            (ChildSlotLifecycle::ProvisionalInert, parent, outlet, *slot)
        }
        RegistryOwner::Committed(EntityOwner::ChildSlot {
            parent,
            outlet,
            slot,
        }) if claim.lifecycle() == EntityLifecycle::PreparedInert => {
            (ChildSlotLifecycle::RegisteredInert, parent, outlet, *slot)
        }
        RegistryOwner::Committed(EntityOwner::ChildSlot {
            parent,
            outlet,
            slot,
        }) if claim.lifecycle() == EntityLifecycle::Active => {
            (ChildSlotLifecycle::Active, parent, outlet, *slot)
        }
        RegistryOwner::Committed(EntityOwner::ChildSlot {
            parent,
            outlet,
            slot,
        }) if claim.lifecycle() == EntityLifecycle::Leaving => {
            (ChildSlotLifecycle::Leaving, parent, outlet, *slot)
        }
        RegistryOwner::Provisional(ProvisionalOwner::ChildSlot {
            parent,
            outlet,
            slot,
        })
        | RegistryOwner::Committed(EntityOwner::ChildSlot {
            parent,
            outlet,
            slot,
        }) if claim.lifecycle() == EntityLifecycle::Quarantined => {
            (ChildSlotLifecycle::Quarantined, parent, outlet, *slot)
        }
        _ => return false,
    };
    let provisional_can_be_partial = allow_missing
        && matches!(
            claim.owner(),
            RegistryOwner::Provisional(ProvisionalOwner::ChildSlot { .. })
        );
    let lifecycle_is_compatible = lifecycle == Some(&expected_lifecycle)
        || (provisional_can_be_partial && lifecycle.is_none());
    let authority_is_compatible = authority.is_some_and(|authority| {
        authority.parent() == parent && authority.outlet() == outlet && authority.slot() == slot
    }) || (provisional_can_be_partial && authority.is_none());
    lifecycle_is_compatible && authority_is_compatible
}

impl CleanupReceipt {
    fn new(
        ledger_domain: u64,
        attempt_revision: u64,
        obligation: usize,
        disposition: CleanupDisposition,
        identity: &CleanupObligation,
    ) -> Self {
        Self {
            ledger_domain,
            attempt_revision,
            obligation,
            disposition,
            binding: receipt_binding(
                ledger_domain,
                attempt_revision,
                obligation,
                disposition,
                identity,
            ),
            observed: false,
        }
    }
}

fn collect_evidence(
    world: &World,
    obligations: &[CleanupObligation],
    receipts: &[Option<CleanupReceipt>],
) -> Vec<(usize, CleanupEvidence)> {
    obligations
        .iter()
        .enumerate()
        .filter(|(index, _)| receipts[*index].is_none())
        .map(|(index, _)| {
            (
                index,
                classify_obligation(world, obligations, receipts, index),
            )
        })
        .collect()
}

fn evidence_matches_after_pickable_repair(
    expected: &[(usize, CleanupEvidence)],
    actual: &[(usize, CleanupEvidence)],
    indices: &[usize],
    pickable: ComponentId,
) -> bool {
    if actual.len() != indices.len() {
        return false;
    }
    indices.iter().all(|index| {
        let Some((_, expected)) = expected.iter().find(|(candidate, _)| candidate == index) else {
            return false;
        };
        let Some((_, actual)) = actual.iter().find(|(candidate, _)| candidate == index) else {
            return false;
        };
        let (
            CleanupEvidence::Exact {
                actual_parent: expected_parent,
                normalized_manifest: expected_manifest,
                foreign_descendants: expected_descendants,
                foreign_components: expected_foreign,
                missing_components: expected_missing,
                protected_parent: expected_protected,
                relationship_cycle: expected_cycle,
            },
            CleanupEvidence::Exact {
                actual_parent,
                normalized_manifest: actual_manifest,
                foreign_descendants: actual_descendants,
                foreign_components: actual_foreign,
                missing_components: actual_missing,
                protected_parent: actual_protected,
                relationship_cycle: actual_cycle,
            },
        ) = (expected, actual)
        else {
            return false;
        };
        expected_parent == actual_parent
            && expected_descendants == actual_descendants
            && expected_protected == actual_protected
            && expected_cycle == actual_cycle
            && components_without(expected_manifest.component_ids(), pickable)
                == components_without(actual_manifest.component_ids(), pickable)
            && components_without(expected_foreign, pickable)
                == components_without(actual_foreign, pickable)
            && components_without(expected_missing, pickable)
                == components_without(actual_missing, pickable)
    })
}

fn components_without(components: &[ComponentId], excluded: ComponentId) -> Vec<ComponentId> {
    components
        .iter()
        .copied()
        .filter(|component| *component != excluded)
        .collect()
}

fn observed_children(world: &World, parent: Entity) -> Vec<Entity> {
    world
        .get::<Children>(parent)
        .map(|children| children.iter().copied().collect())
        .unwrap_or_default()
}

fn children_match(world: &World, parent: Entity, expected: &[Entity]) -> bool {
    match world.get::<Children>(parent) {
        Some(children) => children.iter().copied().eq(expected.iter().copied()),
        None => expected.is_empty(),
    }
}

fn children_match_after_removals(
    world: &World,
    parent: Entity,
    expected: &[Entity],
    completed_removals: &[Entity],
) -> bool {
    let expected = expected
        .iter()
        .copied()
        .filter(|child| !completed_removals.contains(child));
    match world.get::<Children>(parent) {
        Some(children) => children.iter().copied().eq(expected),
        None => expected.count() == 0,
    }
}

fn classify_obligation(
    world: &World,
    obligations: &[CleanupObligation],
    receipts: &[Option<CleanupReceipt>],
    index: usize,
) -> CleanupEvidence {
    let obligation = &obligations[index];
    let authority_settlement = matches!(
        obligation.terminal_mode,
        CleanupTerminalMode::AuthorityExact | CleanupTerminalMode::AuthorityLost
    );
    let entity_present = world.get_entity(obligation.claim.entity()).is_ok();
    let registry_exact = world
        .get_resource::<RouteEntityRegistry>()
        .and_then(|registry| registry.record(obligation.claim.entity()))
        .is_some_and(|record| record.claim() == obligation.claim);
    let lost_evidence = || CleanupEvidence::AuthorityLostResidual {
        actual_parent: world
            .get::<ChildOf>(obligation.claim.entity())
            .map(ChildOf::parent),
        registry_exact,
        entity_present,
    };
    if obligation.terminal_mode == CleanupTerminalMode::AuthorityLost {
        return lost_evidence();
    }
    if !entity_present {
        if authority_settlement {
            let current = world
                .get_resource::<RouteEntityRegistry>()
                .and_then(|registry| registry.record(obligation.claim.entity()));
            return match current {
                None => CleanupEvidence::AlreadyAbsent,
                Some(record) if record.claim() == obligation.claim => {
                    CleanupEvidence::AlreadyAbsent
                }
                Some(_) => lost_evidence(),
            };
        }
        return match world
            .get_resource::<RouteEntityRegistry>()
            .and_then(|registry| registry.record(obligation.claim.entity()))
        {
            None => CleanupEvidence::AlreadyAbsent,
            Some(record) if record.claim() == obligation.claim => CleanupEvidence::AlreadyAbsent,
            Some(_) => CleanupEvidence::StaleClaim,
        };
    }
    let Some(record) = world
        .get_resource::<RouteEntityRegistry>()
        .and_then(|registry| registry.record(obligation.claim.entity()))
    else {
        return if authority_settlement {
            lost_evidence()
        } else {
            CleanupEvidence::StaleClaim
        };
    };
    if record.claim() != obligation.claim {
        return if authority_settlement {
            lost_evidence()
        } else {
            CleanupEvidence::StaleClaim
        };
    }
    let actual_seal = world.get::<RouteOwnershipSeal>(obligation.claim.entity());
    let incomplete_obligation_can_lack_seal =
        obligation.allow_missing_components && actual_seal.is_none();
    let physical_claim_is_compatible = match obligation.claim.owner() {
        RegistryOwner::Committed(
            EntityOwner::FrameShell(_)
            | EntityOwner::FrameContentTarget(_)
            | EntityOwner::FrameOutput { .. },
        ) => {
            let lifecycle = world.get::<FrameLifecycle>(obligation.claim.entity());
            lifecycle == Some(&FrameLifecycle(obligation.claim.lifecycle()))
                || (obligation.allow_missing_components && lifecycle.is_none())
        }
        RegistryOwner::Committed(EntityOwner::HostTopSlot { .. }) => world
            .get::<ProvisionalSlotLifecycle>(obligation.claim.entity())
            .map(|lifecycle| lifecycle_is_exact(*lifecycle, obligation.claim.lifecycle()))
            .unwrap_or(obligation.allow_missing_components),
        _ => child_slot_components_are_compatible(
            world.get::<ChildSlotLifecycle>(obligation.claim.entity()),
            world.get::<ChildSlotAuthority>(obligation.claim.entity()),
            &obligation.claim,
            obligation.allow_missing_components,
        ),
    };
    if (actual_seal != Some(&RouteOwnershipSeal::new(obligation.claim.seal()))
        && !incomplete_obligation_can_lack_seal)
        || !physical_claim_is_compatible
    {
        return if authority_settlement {
            lost_evidence()
        } else {
            CleanupEvidence::StaleClaim
        };
    }
    let Some(actual_manifest) = ComponentManifest::from_world(world, obligation.claim.entity())
    else {
        return CleanupEvidence::AlreadyAbsent;
    };
    let relationship_components = [
        world.components().component_id::<ChildOf>(),
        world.components().component_id::<Children>(),
    ]
    .into_iter()
    .flatten()
    .collect::<HashSet<_>>();
    let normalized_manifest = ComponentManifest::new(
        actual_manifest
            .component_ids()
            .iter()
            .copied()
            .filter(|component| !relationship_components.contains(component)),
    );
    let mut foreign_components = actual_manifest
        .component_ids()
        .iter()
        .copied()
        .filter(|component| {
            !obligation.manifest.component_ids().contains(component)
                && !relationship_components.contains(component)
        })
        .collect::<Vec<_>>();
    foreign_components.sort_unstable();
    foreign_components.dedup();
    let mut missing_components = if obligation.allow_missing_components {
        Vec::new()
    } else {
        obligation
            .manifest
            .component_ids()
            .iter()
            .copied()
            .filter(|component| {
                !actual_manifest.component_ids().contains(component)
                    && !relationship_components.contains(component)
            })
            .collect::<Vec<_>>()
    };
    missing_components.sort_unstable();
    missing_components.dedup();

    let unsettled_owned = obligations
        .iter()
        .enumerate()
        .filter_map(|(candidate, obligation)| {
            receipts[candidate]
                .is_none()
                .then_some(obligation.claim.entity())
        })
        .collect::<HashSet<_>>();
    let mut foreign_descendants = Vec::new();
    let mut visited = HashSet::new();
    let mut relationship_cycle = false;
    let mut stack = world
        .get::<Children>(obligation.claim.entity())
        .into_iter()
        .flat_map(|children| children.iter().copied())
        .collect::<Vec<_>>();
    while let Some(entity) = stack.pop() {
        if !visited.insert(entity) {
            relationship_cycle = true;
            continue;
        }
        if !unsettled_owned.contains(&entity) {
            foreign_descendants.push(entity);
        }
        if let Some(children) = world.get::<Children>(entity) {
            stack.extend(children.iter().copied());
        }
    }
    foreign_descendants.sort_by_key(|entity| entity.to_bits());
    foreign_descendants.dedup();

    let actual_parent = world
        .get::<ChildOf>(obligation.claim.entity())
        .map(ChildOf::parent);
    let protected_parent = if authority_settlement {
        actual_parent.filter(|parent| {
            let retired_host_boundary = matches!(
                obligation.claim.owner(),
                RegistryOwner::Committed(EntityOwner::HostTopSlot { .. })
            ) && *parent == obligation.authorized_parent;
            let exact_batch_parent =
                obligations
                    .iter()
                    .enumerate()
                    .any(|(candidate, parent_obligation)| {
                        receipts[candidate].is_none()
                            && parent_obligation.claim.entity() == *parent
                            && parent_obligation.terminal_mode
                                == CleanupTerminalMode::AuthorityExact
                            && cleanup_claim_has_exact_authority(world, &parent_obligation.claim)
                    });
            !retired_host_boundary && !exact_batch_parent
        })
    } else {
        actual_parent.filter(|parent| {
            *parent != obligation.expected_parent
                && *parent != obligation.authorized_parent
                && !unsettled_owned.contains(parent)
        })
    };
    CleanupEvidence::Exact {
        actual_parent,
        normalized_manifest,
        foreign_descendants,
        foreign_components,
        missing_components,
        protected_parent,
        relationship_cycle,
    }
}

fn quarantine_disposition(
    evidence: &[(usize, CleanupEvidence)],
    obligations: &[CleanupObligation],
) -> Option<CleanupDisposition> {
    if evidence
        .iter()
        .any(|(_, evidence)| matches!(evidence, CleanupEvidence::AuthorityLostResidual { .. }))
    {
        return Some(CleanupDisposition::RejectedProtected);
    }
    if evidence
        .iter()
        .any(|(index, _)| obligations[*index].terminal_mode == CleanupTerminalMode::ForceUnresolved)
    {
        return Some(CleanupDisposition::QuarantinedProtectedParent);
    }
    if evidence.iter().any(|(_, evidence)| {
        matches!(
            evidence,
            CleanupEvidence::Exact {
                protected_parent: Some(_),
                ..
            }
        )
    }) {
        return Some(CleanupDisposition::QuarantinedProtectedParent);
    }
    if evidence.iter().any(|(_, evidence)| {
        matches!(
            evidence,
            CleanupEvidence::Exact {
                relationship_cycle: true,
                ..
            }
        )
    }) {
        return Some(CleanupDisposition::QuarantinedProtectedParent);
    }
    if evidence.iter().any(|(_, evidence)| {
        matches!(
            evidence,
            CleanupEvidence::Exact {
                foreign_descendants,
                ..
            } if !foreign_descendants.is_empty()
        )
    }) {
        return Some(CleanupDisposition::QuarantinedForeignDescendant);
    }
    evidence
        .iter()
        .any(|(_, evidence)| {
            matches!(
                evidence,
                CleanupEvidence::Exact {
                    foreign_components,
                    missing_components,
                    ..
                } if !foreign_components.is_empty() || !missing_components.is_empty()
            )
        })
        .then_some(CleanupDisposition::QuarantinedForeignComponent)
}

fn authority_base_quarantine_disposition(
    obligation: &CleanupObligation,
    evidence: &CleanupEvidence,
) -> Option<CleanupDisposition> {
    let CleanupEvidence::Exact {
        protected_parent,
        foreign_descendants,
        foreign_components,
        missing_components,
        relationship_cycle,
        ..
    } = evidence
    else {
        return None;
    };
    if obligation.terminal_mode == CleanupTerminalMode::ForceUnresolved
        || protected_parent.is_some()
    {
        return Some(CleanupDisposition::QuarantinedProtectedParent);
    }
    if !foreign_descendants.is_empty() {
        return Some(CleanupDisposition::QuarantinedForeignDescendant);
    }
    if *relationship_cycle {
        return Some(CleanupDisposition::QuarantinedForeignDescendant);
    }
    if !foreign_components.is_empty() || !missing_components.is_empty() {
        return Some(CleanupDisposition::QuarantinedForeignComponent);
    }
    None
}

fn authority_settlement_dispositions(
    world: &World,
    obligations: &[CleanupObligation],
    evidence: &[(usize, CleanupEvidence)],
) -> Option<Vec<AuthoritySettlementDisposition>> {
    let mut plans = Vec::new();
    plans.try_reserve_exact(evidence.len()).ok()?;
    let mut retained_entities = HashSet::new();
    retained_entities.try_reserve(evidence.len()).ok()?;
    for (index, evidence) in evidence {
        let obligation = obligations.get(*index)?;
        let (disposition, action) = match evidence {
            CleanupEvidence::AlreadyAbsent => (
                CleanupDisposition::AlreadyAbsent,
                AuthoritySettlementAction::AlreadyAbsent,
            ),
            CleanupEvidence::AuthorityLostResidual { .. } => (
                CleanupDisposition::RejectedProtected,
                AuthoritySettlementAction::LostResidual,
            ),
            CleanupEvidence::Exact { actual_parent, .. } => {
                if let Some(disposition) =
                    authority_base_quarantine_disposition(obligation, evidence)
                {
                    (disposition, AuthoritySettlementAction::VerifiedResidual)
                } else {
                    (
                        if *actual_parent == Some(obligation.expected_parent) {
                            CleanupDisposition::Removed
                        } else {
                            CleanupDisposition::RemovedDetached
                        },
                        AuthoritySettlementAction::Remove,
                    )
                }
            }
            CleanupEvidence::StaleClaim => return None,
        };
        if action != AuthoritySettlementAction::Remove {
            retained_entities.insert(obligation.claim.entity());
        }
        plans.push(AuthoritySettlementDisposition {
            index: *index,
            disposition,
            action,
        });
    }

    loop {
        let promoted = plans
            .iter()
            .filter(|plan| plan.action == AuthoritySettlementAction::Remove)
            .filter(|plan| {
                authority_subtree_requires_retention(
                    world,
                    obligations[plan.index].claim.entity(),
                    &retained_entities,
                )
            })
            .map(|plan| plan.index)
            .collect::<Vec<_>>();
        if promoted.is_empty() {
            break;
        }
        for index in promoted {
            let plan = plans.iter_mut().find(|plan| plan.index == index)?;
            plan.action = AuthoritySettlementAction::VerifiedResidual;
            plan.disposition = CleanupDisposition::QuarantinedForeignDescendant;
            retained_entities.insert(obligations[index].claim.entity());
        }
    }
    Some(plans)
}

fn authority_subtree_requires_retention(
    world: &World,
    root: Entity,
    retained_entities: &HashSet<Entity>,
) -> bool {
    let mut visited = HashSet::new();
    let mut stack = world
        .get::<Children>(root)
        .into_iter()
        .flat_map(|children| children.iter().copied())
        .collect::<Vec<_>>();
    while let Some(entity) = stack.pop() {
        if retained_entities.contains(&entity) || !visited.insert(entity) {
            return true;
        }
        if let Some(children) = world.get::<Children>(entity) {
            stack.extend(children.iter().copied());
        }
    }
    false
}

fn authority_residual_groups(
    world: &World,
    obligations: &[CleanupObligation],
    dispositions: &[AuthoritySettlementDisposition],
) -> Option<Vec<Vec<usize>>> {
    let mut remaining = dispositions
        .iter()
        .filter(|plan| {
            matches!(
                plan.action,
                AuthoritySettlementAction::VerifiedResidual
                    | AuthoritySettlementAction::LostResidual
            )
        })
        .map(|plan| plan.index)
        .collect::<Vec<_>>();
    let mut groups = Vec::new();
    groups.try_reserve_exact(remaining.len()).ok()?;
    while let Some(seed) = remaining.pop() {
        let mut group = vec![seed];
        let mut cursor = 0;
        while cursor < group.len() {
            let entity = obligations.get(group[cursor])?.claim.entity();
            let mut candidate = 0;
            while candidate < remaining.len() {
                let other = obligations.get(remaining[candidate])?.claim.entity();
                if authority_entities_are_adjacent(world, entity, other) {
                    group.push(remaining.swap_remove(candidate));
                } else {
                    candidate += 1;
                }
            }
            cursor += 1;
        }
        group.sort_unstable();
        groups.push(group);
    }
    groups.sort_by_key(|group| group.first().copied().unwrap_or(usize::MAX));
    Some(groups)
}

fn authority_entities_are_adjacent(world: &World, left: Entity, right: Entity) -> bool {
    world.get::<ChildOf>(left).map(ChildOf::parent) == Some(right)
        || world.get::<ChildOf>(right).map(ChildOf::parent) == Some(left)
        || world
            .get::<Children>(left)
            .is_some_and(|children| children.contains(&right))
        || world
            .get::<Children>(right)
            .is_some_and(|children| children.contains(&left))
}

fn authority_projected_manifest(
    world: &World,
    entity: Entity,
    host_edge: &PreparedAuthorityHostEdge,
    removal_entities: &HashSet<Entity>,
    pickable_repair: &PreparedAuthorityPickableRepair,
) -> Option<ComponentManifest> {
    let actual = ComponentManifest::from_world(world, entity)?;
    let child_of = world.components().component_id::<ChildOf>();
    let children = world.components().component_id::<Children>();
    let pickable = world.components().component_id::<Pickable>();
    let removes_host_edge = host_edge.removes_child_of(entity);
    let retains_children_component = world.get::<Children>(entity).is_some_and(|children| {
        children
            .iter()
            .any(|child| !removal_entities.contains(child))
    });
    Some(ComponentManifest::new(
        actual
            .component_ids()
            .iter()
            .copied()
            .filter(|component| !(removes_host_edge && Some(*component) == child_of))
            .filter(|component| Some(*component) != children || retains_children_component)
            .chain(
                pickable
                    .filter(|_| pickable_repair.repairs(entity))
                    .into_iter(),
            ),
    ))
}

fn authority_projected_parent(
    world: &World,
    entity: Entity,
    host_edge: &PreparedAuthorityHostEdge,
) -> Option<Entity> {
    (!host_edge.removes_child_of(entity))
        .then(|| world.get::<ChildOf>(entity).map(ChildOf::parent))
        .flatten()
}

fn authority_projected_children(
    world: &World,
    entity: Entity,
    removal_entities: &HashSet<Entity>,
) -> Vec<Entity> {
    observed_children(world, entity)
        .into_iter()
        .filter(|child| !removal_entities.contains(child))
        .collect()
}

fn project_cleanup_evidence_after_pickable_repair(
    evidence: CleanupEvidence,
    pickable: ComponentId,
) -> CleanupEvidence {
    let CleanupEvidence::Exact {
        actual_parent,
        normalized_manifest,
        foreign_descendants,
        foreign_components,
        missing_components,
        protected_parent,
        relationship_cycle,
    } = evidence
    else {
        return evidence;
    };
    CleanupEvidence::Exact {
        actual_parent,
        normalized_manifest: ComponentManifest::new(
            normalized_manifest
                .component_ids()
                .iter()
                .copied()
                .chain(std::iter::once(pickable)),
        ),
        foreign_descendants,
        foreign_components: foreign_components
            .into_iter()
            .filter(|component| *component != pickable)
            .collect(),
        missing_components: missing_components
            .into_iter()
            .filter(|component| *component != pickable)
            .collect(),
        protected_parent,
        relationship_cycle,
    }
}

fn authority_observed_depth(world: &World, entity: Entity) -> usize {
    let mut depth = 0usize;
    let mut current = entity;
    let mut visited = HashSet::new();
    while visited.insert(current) {
        let Some(parent) = world.get::<ChildOf>(current).map(ChildOf::parent) else {
            break;
        };
        depth = depth.saturating_add(1);
        current = parent;
    }
    depth
}

fn quarantine_facts(evidence: &[(usize, CleanupEvidence)]) -> QuarantineFacts {
    let mut protected_parents: Vec<Entity> = Vec::new();
    let mut foreign_descendants: Vec<Entity> = Vec::new();
    let mut foreign_components: Vec<ComponentId> = Vec::new();
    let mut missing_components: Vec<ComponentId> = Vec::new();
    for (_, evidence) in evidence {
        match evidence {
            CleanupEvidence::Exact {
                protected_parent,
                foreign_descendants: descendants,
                foreign_components: components,
                missing_components: missing,
                ..
            } => {
                protected_parents.extend(protected_parent);
                foreign_descendants.extend(descendants);
                foreign_components.extend(components);
                missing_components.extend(missing);
            }
            CleanupEvidence::AuthorityLostResidual { actual_parent, .. } => {
                protected_parents.extend(actual_parent);
            }
            _ => {}
        }
    }
    protected_parents.sort_by_key(|entity| entity.to_bits());
    protected_parents.dedup();
    QuarantineFacts::with_component_drift(
        None,
        protected_parents.first().copied(),
        foreign_descendants,
        foreign_components,
        missing_components,
    )
}

fn quarantine_focus_closure(
    evidence: &[(usize, CleanupEvidence)],
    obligations: &[CleanupObligation],
) -> Vec<Entity> {
    let mut entities = Vec::new();
    for (index, evidence) in evidence {
        match evidence {
            CleanupEvidence::Exact {
                foreign_descendants,
                ..
            } => {
                entities.push(obligations[*index].claim.entity());
                entities.extend(foreign_descendants);
            }
            CleanupEvidence::AuthorityLostResidual { .. } => {
                entities.push(obligations[*index].claim.entity());
            }
            _ => {}
        }
    }
    entities.sort_by_key(|entity| entity.to_bits());
    entities.dedup();
    entities
}

fn release_cleanup_focus(world: &mut World, closure: &[Entity]) {
    if world.contains_resource::<InputFocus>() && world.contains_resource::<RouteFocusState>() {
        world.resource_scope(|world, input_focus: Mut<InputFocus>| {
            world.resource_scope(|_world, route_focus: Mut<RouteFocusState>| {
                prepare_focus_mutation(input_focus, route_focus, closure).apply();
            });
        });
        return;
    }
    let clear_input = world
        .get_resource::<InputFocus>()
        .and_then(InputFocus::get)
        .is_some_and(|focused| closure.contains(&focused));
    if clear_input {
        world.resource_mut::<InputFocus>().clear();
    }
    let clear_route = world
        .get_resource::<RouteFocusState>()
        .and_then(RouteFocusState::focused)
        .is_some_and(|focused| closure.contains(&focused));
    if clear_route {
        world.resource_mut::<RouteFocusState>().set_focused(None);
    }
}

fn focus_is_released(world: &World, closure: &[Entity]) -> bool {
    world
        .get_resource::<InputFocus>()
        .and_then(InputFocus::get)
        .is_none_or(|focused| !closure.contains(&focused))
        && world
            .get_resource::<RouteFocusState>()
            .and_then(RouteFocusState::focused)
            .is_none_or(|focused| !closure.contains(&focused))
}

fn manifest_digest(obligations: &[CleanupObligation]) -> u64 {
    frozen_manifest_digest(obligations, &[])
}

fn frozen_manifest_digest(
    obligations: &[CleanupObligation],
    mount_parents: &[CleanupMountParentSettlement],
) -> u64 {
    let mut digest = StructuredDigest::new(MANIFEST_DOMAIN);
    digest.push(obligations.len() as u64);
    for obligation in obligations {
        digest.obligation(obligation);
    }
    digest.push(mount_parents.len() as u64);
    for parent in mount_parents {
        digest.mount_parent(parent);
    }
    digest.finish()
}

fn unsettled_digest(
    manifest_digest: u64,
    obligations: &[CleanupObligation],
    receipts: &[Option<CleanupReceipt>],
) -> u64 {
    let mut digest = StructuredDigest::new(ATTEMPT_DOMAIN);
    digest.push(manifest_digest);
    for (index, obligation) in obligations.iter().enumerate() {
        if receipts[index].is_none() {
            digest.push(index as u64);
            digest.obligation(obligation);
        }
    }
    digest.finish()
}

fn receipt_binding(
    ledger_domain: u64,
    attempt_revision: u64,
    obligation: usize,
    disposition: CleanupDisposition,
    identity: &CleanupObligation,
) -> u64 {
    let mut digest = StructuredDigest::new(RECEIPT_DOMAIN);
    digest.push(ledger_domain);
    digest.push(attempt_revision);
    digest.push(obligation as u64);
    digest.push(disposition.tag());
    digest.obligation(identity);
    digest.finish()
}

fn cleanup_obligation_binding(obligation: &CleanupObligation) -> u64 {
    let mut digest = StructuredDigest::new(OBLIGATION_DOMAIN);
    digest.obligation(obligation);
    digest.finish()
}

fn completion_digest(receipts: &[Option<CleanupReceipt>]) -> u64 {
    let mut digest = StructuredDigest::new(COMPLETION_DOMAIN);
    digest.push(receipts.len() as u64);
    for receipt in receipts.iter().flatten() {
        digest.push(receipt.obligation as u64);
        digest.push(receipt.disposition.tag());
        digest.push(receipt.binding);
    }
    digest.finish()
}

fn canonical_empty_completion_digest() -> u64 {
    StructuredDigest::new(COMPLETION_DOMAIN).finish()
}

pub(crate) fn canonical_zero_completion_digest() -> u64 {
    canonical_empty_completion_digest()
}

pub(crate) const fn canonical_zero_manifest_digest() -> u64 {
    0
}

struct StructuredDigest(u64);

impl StructuredDigest {
    const fn new(domain: u64) -> Self {
        Self(0xcbf2_9ce4_8422_2325 ^ domain)
    }

    fn push(&mut self, value: u64) {
        for byte in value.to_le_bytes() {
            self.0 ^= u64::from(byte);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }

    fn bytes(&mut self, bytes: &[u8]) {
        self.push(bytes.len() as u64);
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }

    fn obligation(&mut self, obligation: &CleanupObligation) {
        self.push(obligation.claim.entity().to_bits());
        self.bytes(format!("{:?}", obligation.claim.owner()).as_bytes());
        self.bytes(format!("{:?}", obligation.claim.seal()).as_bytes());
        self.bytes(format!("{:?}", obligation.claim.lifecycle()).as_bytes());
        self.push(obligation.manifest.component_ids().len() as u64);
        for component in obligation.manifest.component_ids() {
            self.push(component.index() as u64);
        }
        self.push(obligation.expected_parent.to_bits());
        self.push(obligation.unit_root.to_bits());
        self.push(obligation.authorized_parent.to_bits());
        self.push(u64::from(obligation.allow_missing_components));
        self.push(obligation.terminal_mode.digest_tag());
    }

    fn mount_parent(&mut self, parent: &CleanupMountParentSettlement) {
        self.push(parent.claim.entity().to_bits());
        self.bytes(format!("{:?}", parent.claim.owner()).as_bytes());
        self.bytes(format!("{:?}", parent.claim.seal()).as_bytes());
        self.bytes(format!("{:?}", parent.claim.lifecycle()).as_bytes());
        self.push(parent.manifest.component_ids().len() as u64);
        for component in parent.manifest.component_ids() {
            self.push(component.index() as u64);
        }
        self.push(parent.physical_manifest.component_ids().len() as u64);
        for component in parent.physical_manifest.component_ids() {
            self.push(component.index() as u64);
        }
        self.push(parent.expected_parent.to_bits());
        if let Some(claim) = &parent.expected_parent_claim {
            self.push(1);
            self.push(claim.entity().to_bits());
            self.bytes(format!("{:?}", claim.owner()).as_bytes());
            self.bytes(format!("{:?}", claim.seal()).as_bytes());
            self.bytes(format!("{:?}", claim.lifecycle()).as_bytes());
        } else {
            self.push(0);
        }
        self.push(parent.removed_child.to_bits());
        self.push(u64::from(parent.removed_child_was_attached));
        self.push(parent.children_before.len() as u64);
        for entity in &parent.children_before {
            self.push(entity.to_bits());
        }
        self.push(parent.children_after.len() as u64);
        for entity in &parent.children_after {
            self.push(entity.to_bits());
        }
    }

    const fn finish(self) -> u64 {
        self.0
    }
}

#[cfg(test)]
pub(crate) fn cleanup_batch_matches(
    batch: &CleanupBatch,
    manifest_digest: u64,
    obligation_count: usize,
) -> bool {
    batch.manifest_digest() == manifest_digest && batch.obligation_count() == obligation_count
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::registry::{
        ComponentManifest, EntityLifecycle, ProvisionalOwner, QuarantineLeaseTable, RegistryOwner,
        RouteEntityRegistry,
    };
    use bevy_ecs::{component::Component, hierarchy::ChildOf};
    use bevy_ui_router_core::{
        frame::RouteFrameVersion,
        integration::{CleanupManifest, EngineAction, RouterEngine},
        prelude::{RouteLocation, RouteNodeOptions, RoutePattern, RouteTree, RouteViewKey},
        slot::RouteSlotKey,
    };

    #[derive(Component)]
    struct ExternalParentDriftMarker;

    #[derive(Component)]
    struct ForeignBoundaryMarker;

    #[test]
    fn back_capacity_reclaimed_cell_advances_generation_and_rejects_aba() {
        let mut domains = BackCapacityDomainIssuer::new();
        let mut issuer = domains.issue().unwrap();
        let mut foreign_issuer = domains.issue().unwrap();

        let first = issuer.reserve().unwrap();
        let first_identity = first.ordinal_identity_for_test();
        let first_backing = issuer.backing_capacities_for_test();
        let first_evidence = match issuer.retire_unbound(first, EnqueueRejected::CoreTokenDrift) {
            BackCapacityRetireOutcome::Retired(evidence) => evidence,
            BackCapacityRetireOutcome::Rejected { .. } => {
                panic!("the exact outstanding owner must retire")
            }
        };
        assert_eq!(
            first_evidence.enqueue_rejection_for_test(),
            Some(&EnqueueRejected::CoreTokenDrift),
            "retirement evidence retains the original rejection"
        );
        assert_eq!(issuer.backing_capacities_for_test(), first_backing);
        assert_eq!(issuer.semantic_snapshot_for_test().cell_count(), 1);
        assert_eq!(
            issuer.semantic_snapshot_for_test().outstanding_count(),
            0,
            "exact retirement removes the outstanding entry"
        );

        let replay_before = issuer.semantic_snapshot_for_test();
        let replay_backing_before = issuer.backing_capacities_for_test();
        assert_eq!(
            issuer.classify_retirement_evidence(&first_evidence),
            Err(BackCapacityRetireError::ConsumedReplay),
            "retirement evidence is observational and cannot authorize a second retirement"
        );
        assert_eq!(issuer.semantic_snapshot_for_test(), replay_before);
        assert_eq!(issuer.backing_capacities_for_test(), replay_backing_before);

        let second = issuer.reserve().unwrap();
        let second_identity = second.ordinal_identity_for_test();
        assert_eq!(second_identity.0, first_identity.0);
        assert_eq!(second_identity.1, first_identity.1);
        assert_ne!(second_identity.2, first_identity.2, "owners are burned");
        assert_eq!(
            second_identity.3,
            first_identity.3.checked_add(1).unwrap(),
            "the reclaimed cell advances by the exact successor generation"
        );
        assert_eq!(issuer.backing_capacities_for_test(), first_backing);

        let stale_before = issuer.semantic_snapshot_for_test();
        let stale_backing_before = issuer.backing_capacities_for_test();
        assert_eq!(
            issuer.classify_retirement_evidence(&first_evidence),
            Err(BackCapacityRetireError::StaleGeneration),
            "the old generation must not authorize the reused ordinal"
        );
        assert_eq!(issuer.semantic_snapshot_for_test(), stale_before);
        assert_eq!(issuer.backing_capacities_for_test(), stale_backing_before);
        assert_eq!(second.ordinal_identity_for_test(), second_identity);

        let foreign = foreign_issuer.reserve().unwrap();
        let foreign_identity = foreign.ordinal_identity_for_test();
        let local_before_foreign = issuer.semantic_snapshot_for_test();
        let local_backing_before_foreign = issuer.backing_capacities_for_test();
        let foreign_before_rejection = foreign_issuer.semantic_snapshot_for_test();
        let foreign_backing_before_rejection = foreign_issuer.backing_capacities_for_test();
        let foreign = match issuer.retire_unbound(foreign, EnqueueRejected::CoreTokenDrift) {
            BackCapacityRetireOutcome::Rejected {
                owner,
                rejection,
                error,
            } => {
                assert_eq!(error, BackCapacityRetireError::ForeignDomain);
                assert_eq!(rejection, EnqueueRejected::CoreTokenDrift);
                owner
            }
            BackCapacityRetireOutcome::Retired(_) => {
                panic!("a foreign issuer must not retire the move-only owner")
            }
        };
        assert_eq!(foreign.ordinal_identity_for_test(), foreign_identity);
        assert_eq!(issuer.semantic_snapshot_for_test(), local_before_foreign);
        assert_eq!(
            issuer.backing_capacities_for_test(),
            local_backing_before_foreign
        );
        assert_eq!(
            foreign_issuer.semantic_snapshot_for_test(),
            foreign_before_rejection
        );
        assert_eq!(
            foreign_issuer.backing_capacities_for_test(),
            foreign_backing_before_rejection
        );
        assert!(matches!(
            foreign_issuer.retire_unbound(foreign, EnqueueRejected::CoreTokenDrift),
            BackCapacityRetireOutcome::Retired(_)
        ));

        issuer.set_next_new_cell_initial_generation_for_test(u64::MAX);
        let terminal = issuer.reserve().unwrap();
        let terminal_identity = terminal.ordinal_identity_for_test();
        assert_ne!(terminal_identity.1, second_identity.1);
        assert_eq!(terminal_identity.3, u64::MAX);
        let terminal_backing = issuer.backing_capacities_for_test();
        let terminal_evidence =
            match issuer.retire_unbound(terminal, EnqueueRejected::CoreTokenDrift) {
                BackCapacityRetireOutcome::Retired(evidence) => evidence,
                BackCapacityRetireOutcome::Rejected { .. } => {
                    panic!("the exact terminal-generation owner must retire and seal its cell")
                }
            };
        assert_eq!(issuer.backing_capacities_for_test(), terminal_backing);
        assert_eq!(issuer.semantic_snapshot_for_test().outstanding_count(), 1);

        let terminal_replay_before = issuer.semantic_snapshot_for_test();
        let terminal_replay_backing_before = issuer.backing_capacities_for_test();
        assert_eq!(
            issuer.classify_retirement_evidence(&terminal_evidence),
            Err(BackCapacityRetireError::ConsumedReplay)
        );
        assert_eq!(issuer.semantic_snapshot_for_test(), terminal_replay_before);
        assert_eq!(
            issuer.backing_capacities_for_test(),
            terminal_replay_backing_before
        );

        let cells_before_successor = issuer.semantic_snapshot_for_test().cell_count();
        let successor = issuer.reserve().unwrap();
        let successor_identity = successor.ordinal_identity_for_test();
        assert_ne!(
            successor_identity.1, terminal_identity.1,
            "a no-successor cell is sealed and cannot be reused"
        );
        assert_eq!(
            issuer.semantic_snapshot_for_test().cell_count(),
            cells_before_successor + 1,
            "reserve must grow a different real cell"
        );
        assert!(matches!(
            issuer.retire_unbound(successor, EnqueueRejected::CoreTokenDrift),
            BackCapacityRetireOutcome::Retired(_)
        ));

        assert_eq!(
            second.ordinal_identity_for_test(),
            second_identity,
            "foreign, stale and replay probes must not alter the live owner"
        );
        assert!(matches!(
            issuer.retire_unbound(second, EnqueueRejected::CoreTokenDrift),
            BackCapacityRetireOutcome::Retired(_)
        ));
        assert_eq!(issuer.semantic_snapshot_for_test().outstanding_count(), 0);
    }

    fn prepared_installed_back_zero_completion(
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
            post_settlement_focus: Vec::new(),
            authority_host_children: None,
            back_no_target_authority_partition: None,
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

    #[test]
    fn back_no_target_zero_completion_reclaim_rejection_is_zero_write() {
        let (issuer, prepared, identity) = prepared_installed_back_zero_completion(1);
        let mut domains = BackCapacityDomainIssuer::new();
        let mut foreign = domains.issue().unwrap();
        foreign.domain = PreparationCapacityDomain(
            identity
                .0
                .checked_add(1)
                .expect("the test issuer domain has a foreign successor"),
        );
        let before = foreign.semantic_snapshot_for_test();
        let backing_before = foreign.backing_capacities_for_test();

        assert!(matches!(
            foreign.preflight_installed_back_capacity_reclaim(&prepared),
            Err(BackCapacityRetireError::ForeignDomain)
        ));
        assert_eq!(foreign.semantic_snapshot_for_test(), before);
        assert_eq!(foreign.backing_capacities_for_test(), backing_before);
        assert_eq!(prepared.capacity_identity_for_test(), identity);
        assert_eq!(issuer.semantic_snapshot_for_test().outstanding_count(), 0);
    }

    #[test]
    fn back_no_target_zero_completion_preparation_rejection_returns_whole_ledger() {
        let (_issuer, prepared, identity) = prepared_installed_back_zero_completion(1);
        let ledger = prepared.into_ledger();
        let (returned, evidence) = match ledger.prepare_back_no_target_zero_completion() {
            Err(failure) => failure.into_parts(),
            Ok(_) => panic!("a ledger without the dedicated batch must fail closed"),
        };
        assert_eq!(
            evidence.source_identity().capacity(),
            BackNoTargetInstalledCapacitySource::Present {
                domain: identity.0,
                cell: identity.1,
                owner: identity.2,
                generation: identity.3,
            }
        );
        assert_eq!(
            evidence.source_identity().ledger_domain(),
            returned.ledger_domain
        );
        assert_eq!(
            evidence.expected(),
            BackNoTargetZeroCompletionFacts {
                manifest_digest: canonical_zero_manifest_digest(),
                obligation_count: 0,
                completion_digest: canonical_zero_completion_digest(),
                settled_count: 0,
            }
        );
        assert_eq!(
            evidence.observed().dedicated_batch(),
            BackNoTargetDedicatedBatchObservation::Absent
        );
        assert_eq!(
            evidence.observed().completion_digest(),
            Some(canonical_zero_completion_digest())
        );
        let capacity = returned
            .back_no_target_capacity
            .as_ref()
            .expect("rejection returns the installed capacity lease");
        assert_eq!(
            (
                capacity.domain.0,
                capacity.cell.0,
                capacity.owner.0,
                capacity.generation.0,
            ),
            identity
        );
        assert!(returned.batch.is_none());
        assert!(returned.back_no_target_batch.is_none());
        assert!(returned.owner.is_none());
        assert!(returned.obligations.is_empty());
        assert!(returned.mount_parents.is_empty());
        assert!(returned.receipts.is_empty());
        assert_eq!(
            returned.completion_digest,
            Some(canonical_empty_completion_digest())
        );
    }

    #[test]
    fn back_no_target_zero_completion_evidence_distinguishes_digest_and_batch_drift() {
        let (_issuer, prepared, identity) = prepared_installed_back_zero_completion(1);
        let missing_batch = match prepared
            .into_ledger()
            .prepare_back_no_target_zero_completion()
        {
            Err(failure) => failure.evidence(),
            Ok(_) => panic!("missing dedicated batch commitment must reject"),
        };

        let (_issuer, prepared, _) = prepared_installed_back_zero_completion(1);
        let mut ledger = prepared.into_ledger();
        ledger.drift_back_no_target_completion_digest_for_test();
        let completion_drift = match ledger.prepare_back_no_target_zero_completion() {
            Err(failure) => failure.evidence(),
            Ok(_) => panic!("noncanonical stored completion digest must reject"),
        };
        let expected = missing_batch.expected();
        let exact_observed = BackNoTargetZeroCompletionObservedFacts {
            dedicated_batch: BackNoTargetDedicatedBatchObservation::Present {
                manifest_digest: expected.manifest_digest(),
                obligation_count: expected.obligation_count(),
                expected_completion_digest: expected.completion_digest(),
            },
            ..missing_batch.observed()
        };
        let exact = BackNoTargetZeroCompletionPreparationEvidence {
            source: missing_batch.source_identity(),
            expected,
            observed: exact_observed,
        };
        assert!(exact.is_exact());
        let batch_commitment_drift = BackNoTargetZeroCompletionPreparationEvidence {
            observed: BackNoTargetZeroCompletionObservedFacts {
                dedicated_batch: BackNoTargetDedicatedBatchObservation::Present {
                    manifest_digest: expected.manifest_digest(),
                    obligation_count: expected.obligation_count(),
                    expected_completion_digest: expected.completion_digest() ^ 1,
                },
                ..exact_observed
            },
            ..exact
        };
        assert!(!batch_commitment_drift.is_exact());

        assert_eq!(
            missing_batch.source_identity().capacity(),
            BackNoTargetInstalledCapacitySource::Present {
                domain: identity.0,
                cell: identity.1,
                owner: identity.2,
                generation: identity.3,
            }
        );
        assert_eq!(
            missing_batch.observed().dedicated_batch(),
            BackNoTargetDedicatedBatchObservation::Absent
        );
        assert_eq!(
            missing_batch.observed().completion_digest(),
            Some(canonical_zero_completion_digest())
        );
        assert_eq!(
            completion_drift.observed().dedicated_batch(),
            BackNoTargetDedicatedBatchObservation::Absent
        );
        assert_eq!(
            completion_drift.observed().completion_digest(),
            Some(canonical_zero_completion_digest() ^ 1)
        );
        assert_eq!(
            batch_commitment_drift.observed().dedicated_batch(),
            BackNoTargetDedicatedBatchObservation::Present {
                manifest_digest: canonical_zero_manifest_digest(),
                obligation_count: 0,
                expected_completion_digest: canonical_zero_completion_digest() ^ 1,
            }
        );
        assert_eq!(
            batch_commitment_drift.observed().completion_digest(),
            Some(canonical_zero_completion_digest())
        );
        assert_ne!(batch_commitment_drift, completion_drift);
        assert_ne!(missing_batch, completion_drift);
    }

    #[test]
    fn back_no_target_zero_completion_exact_reclaim_reuses_the_advanced_cell() {
        let (mut issuer, prepared, first_identity) = prepared_installed_back_zero_completion(1);
        let facts = prepared.facts();
        assert_eq!(facts.manifest_digest(), canonical_zero_manifest_digest());
        assert_eq!(facts.obligation_count(), 0);
        assert_eq!(
            facts.completion_digest(),
            canonical_empty_completion_digest()
        );
        assert_eq!(facts.settled_count(), 0);
        assert_eq!(
            prepared.source_identity().capacity(),
            BackNoTargetInstalledCapacitySource::Present {
                domain: first_identity.0,
                cell: first_identity.1,
                owner: first_identity.2,
                generation: first_identity.3,
            }
        );
        let cells_before = issuer.semantic_snapshot_for_test().cell_count();
        let guard = issuer
            .preflight_installed_back_capacity_reclaim(&prepared)
            .expect("the exact installed lease must preflight");
        guard.commit();

        assert_eq!(issuer.semantic_snapshot_for_test().outstanding_count(), 0);
        let reclaimed = issuer
            .cell_projection_for_test(first_identity)
            .expect("the reclaimed cell remains observable");
        assert_eq!(reclaimed.current_generation, first_identity.3 + 1);
        assert!(!reclaimed.original_owner_live);
        assert_eq!(
            reclaimed.disposition,
            BackCapacityCellDispositionForTest::ReusableFree
        );
        let successor = issuer.reserve().unwrap();
        let successor_identity = successor.ordinal_identity_for_test();
        assert_eq!(successor_identity.1, first_identity.1);
        assert_ne!(successor_identity.2, first_identity.2);
        assert_eq!(successor_identity.3, first_identity.3 + 1);
        assert_eq!(
            issuer.semantic_snapshot_for_test().cell_count(),
            cells_before
        );
    }

    #[test]
    fn back_no_target_zero_completion_reclaim_replay_is_zero_write() {
        let (mut issuer, prepared, identity) = prepared_installed_back_zero_completion(1);
        issuer
            .preflight_installed_back_capacity_reclaim(&prepared)
            .expect("the exact installed lease must preflight")
            .commit();
        let before = issuer.semantic_snapshot_for_test();
        let backing_before = issuer.backing_capacities_for_test();

        assert!(matches!(
            issuer.preflight_installed_back_capacity_reclaim(&prepared),
            Err(BackCapacityRetireError::ConsumedReplay)
        ));
        assert_eq!(issuer.semantic_snapshot_for_test(), before);
        assert_eq!(issuer.backing_capacities_for_test(), backing_before);
        assert_eq!(prepared.capacity_identity_for_test(), identity);
    }

    #[test]
    fn back_no_target_zero_completion_max_generation_permanently_retires_cell() {
        let (mut issuer, prepared, terminal_identity) =
            prepared_installed_back_zero_completion(u64::MAX);
        issuer
            .preflight_installed_back_capacity_reclaim(&prepared)
            .expect("the exact terminal-generation lease must preflight")
            .commit();
        let cells_before = issuer.semantic_snapshot_for_test().cell_count();
        let retired = issuer
            .cell_projection_for_test(terminal_identity)
            .expect("the permanently retired cell remains observable");
        assert_eq!(retired.current_generation, u64::MAX);
        assert!(!retired.original_owner_live);
        assert_eq!(
            retired.disposition,
            BackCapacityCellDispositionForTest::PermanentlyRetired
        );
        assert!(matches!(
            issuer.preflight_installed_back_capacity_reclaim(&prepared),
            Err(BackCapacityRetireError::ConsumedReplay)
        ));

        let successor = issuer.reserve().unwrap();
        let successor_identity = successor.ordinal_identity_for_test();
        assert_ne!(
            successor_identity.1, terminal_identity.1,
            "a no-successor installed lease cell must remain permanently retired"
        );
        assert_eq!(
            issuer.semantic_snapshot_for_test().cell_count(),
            cells_before + 1
        );
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    enum CleanupOwnerSnapshot {
        None,
        Leaving {
            claims: Vec<ExactRegistryClaim>,
        },
        FailedPrepared {
            claims: Vec<ExactRegistryClaim>,
        },
        AuthoritySettlement {
            active_claims: Vec<ExactRegistryClaim>,
            pending_claims: Vec<ExactRegistryClaim>,
            host: Entity,
            top_slot: Entity,
            top_slot_claim: ExactRegistryClaim,
        },
        BackNoTargetAuthority {
            active_claims: Vec<ExactRegistryClaim>,
            host: Entity,
            top_slot: Entity,
            top_slot_claim: ExactRegistryClaim,
        },
    }

    fn cleanup_owner_snapshot(owner: Option<&CleanupOwner>) -> CleanupOwnerSnapshot {
        let claims = |branch: &RealizedBranch| branch.claims().cloned().collect::<Vec<_>>();
        match owner {
            None => CleanupOwnerSnapshot::None,
            Some(CleanupOwner::Leaving(branch)) => CleanupOwnerSnapshot::Leaving {
                claims: claims(branch),
            },
            Some(CleanupOwner::FailedPrepared(branch)) => CleanupOwnerSnapshot::FailedPrepared {
                claims: claims(branch.realized()),
            },
            Some(CleanupOwner::AuthoritySettlement(owner)) => {
                CleanupOwnerSnapshot::AuthoritySettlement {
                    active_claims: owner.active.as_ref().map(&claims).unwrap_or_default(),
                    pending_claims: claims(owner.pending.branch().realized()),
                    host: owner.host.host,
                    top_slot: owner.host.top_slot,
                    top_slot_claim: owner.host.top_slot_claim.clone(),
                }
            }
            Some(CleanupOwner::BackNoTargetAuthority(owner)) => {
                CleanupOwnerSnapshot::BackNoTargetAuthority {
                    active_claims: claims(&owner.active),
                    host: owner.host.host,
                    top_slot: owner.host.top_slot,
                    top_slot_claim: owner.host.top_slot_claim.clone(),
                }
            }
        }
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct CleanupRejectionSnapshot {
        owner: CleanupOwnerSnapshot,
        receipts: Vec<Option<CleanupReceipt>>,
        next_attempt: u64,
        issued_attempt: Option<CleanupAttempt>,
        completion_digest: Option<u64>,
        quarantine_event_observed: bool,
        post_settlement_focus: Vec<PostSettlementFocusRoot>,
        authority_host_children: Option<Vec<Entity>>,
        back_no_target_authority_partition: Option<FrozenBackNoTargetAuthorityPartition>,
        quarantine_sequences: Option<(u64, u64, usize)>,
        entity_claim: Option<ExactRegistryClaim>,
        entity_seal: Option<RouteOwnershipSeal>,
        entity_quarantine_claim: Option<ExactRegistryClaim>,
        entity_unresolved_claim: Option<ExactRegistryClaim>,
        entity_manifest: Option<ComponentManifest>,
        entity_parent: Option<Entity>,
        entity_parent_children: Option<Vec<Entity>>,
        parent_claim: Option<ExactRegistryClaim>,
        parent_seal: Option<RouteOwnershipSeal>,
        parent_quarantine_claim: Option<ExactRegistryClaim>,
        parent_unresolved_claim: Option<ExactRegistryClaim>,
        parent_manifest: Option<ComponentManifest>,
        parent_parent: Option<Entity>,
        parent_parent_children: Option<Vec<Entity>>,
    }

    fn cleanup_rejection_snapshot(
        world: &World,
        ledger: &CleanupLedger,
        entity: Entity,
        parent: Entity,
    ) -> CleanupRejectionSnapshot {
        let physical = |entity| {
            let parent = world.get::<ChildOf>(entity).map(ChildOf::parent);
            (
                world
                    .resource::<RouteEntityRegistry>()
                    .record(entity)
                    .map(|record| record.claim()),
                ComponentManifest::from_world(world, entity),
                parent,
                parent.map(|parent| {
                    world
                        .get::<bevy_ecs::hierarchy::Children>(parent)
                        .map(|children| children.iter().copied().collect::<Vec<_>>())
                        .unwrap_or_default()
                }),
            )
        };
        let entity_physical = physical(entity);
        let parent_physical = physical(parent);
        let quarantine = world.get_resource::<QuarantineLeaseTable>();
        CleanupRejectionSnapshot {
            owner: cleanup_owner_snapshot(ledger.owner.as_ref()),
            receipts: ledger.receipts.clone(),
            next_attempt: ledger.next_attempt,
            issued_attempt: ledger.issued_attempt.clone(),
            completion_digest: ledger.completion_digest,
            quarantine_event_observed: ledger.quarantine_event_observed,
            post_settlement_focus: ledger.post_settlement_focus.clone(),
            authority_host_children: ledger.authority_host_children.clone(),
            back_no_target_authority_partition: ledger.back_no_target_authority_partition.clone(),
            quarantine_sequences: quarantine.map(|table| {
                (
                    table.next_sequence_for_test(),
                    table.next_residual_sequence_for_test(),
                    table.leases().len(),
                )
            }),
            entity_claim: entity_physical.0,
            entity_seal: world.get::<RouteOwnershipSeal>(entity).copied(),
            entity_quarantine_claim: quarantine
                .and_then(|table| table.record(entity))
                .map(|record| record.claim()),
            entity_unresolved_claim: quarantine
                .and_then(|table| table.unresolved_record(entity))
                .map(|record| record.claim()),
            entity_manifest: entity_physical.1,
            entity_parent: entity_physical.2,
            entity_parent_children: entity_physical.3,
            parent_claim: parent_physical.0,
            parent_seal: world.get::<RouteOwnershipSeal>(parent).copied(),
            parent_quarantine_claim: quarantine
                .and_then(|table| table.record(parent))
                .map(|record| record.claim()),
            parent_unresolved_claim: quarantine
                .and_then(|table| table.unresolved_record(parent))
                .map(|record| record.claim()),
            parent_manifest: parent_physical.1,
            parent_parent: parent_physical.2,
            parent_parent_children: parent_physical.3,
        }
    }

    fn assert_cleanup_rejection_is_zero(
        world: &World,
        ledger: &CleanupLedger,
        entity: Entity,
        parent: Entity,
        before: &CleanupRejectionSnapshot,
    ) {
        assert_eq!(
            cleanup_rejection_snapshot(world, ledger, entity, parent),
            *before
        );
    }

    fn published_empty_batch() -> (RouterEngine, CleanupBatch) {
        let view = RouteViewKey::new("root").unwrap();
        let tree = RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/").unwrap(),
                RouteNodeOptions::new().with_view(view.clone()),
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
            panic!("root task expected")
        };
        let ticket = engine.report_applied(task).unwrap();
        let prepared = engine
            .prepare_commit(&ticket, CleanupManifest::empty())
            .unwrap();
        let batch = prepared.cleanup().clone();
        let validated = engine.validate_publication(prepared).unwrap();
        validated.publish(|| {});
        (engine, batch)
    }

    #[test]
    fn cleanup_attempt_identity_is_monotonic_and_private() {
        let source = include_str!("cleanup.rs");
        assert!(source.contains("next_attempt"));
        assert!(!source.contains("\npub struct CleanupAttempt"));
        assert!(!source.contains("\npub fn complete("));
        assert!(!source.contains(concat!("fn into_single_", "leaving_frame")));
        assert!(!source.contains(concat!("match settle_", "frame(world")));
    }

    #[test]
    fn replacement_mount_children_come_from_contiguous_owner_facts() {
        let mut world = World::new();
        let sibling = world.spawn_empty().id();
        let leaving = world.spawn_empty().id();
        let entering = world.spawn_empty().id();

        assert_eq!(
            replacement_mount_children(&[sibling], leaving, &[sibling, leaving], entering),
            Some((vec![sibling, leaving, entering], vec![sibling, entering]))
        );
        assert_eq!(
            replacement_mount_children(&[sibling], leaving, &[leaving, sibling], entering),
            None
        );
    }

    #[test]
    fn failed_prepared_branch_manifest_orders_child_frame_before_slot_and_parent_frame() {
        let (app, branch, child_entities, child_slot, parent_entities) =
            crate::navigation::tests::failed_nested_branch_fixture();
        let manifest = FrozenCleanupManifest::from_failed_prepared_branch(
            app.world(),
            app.world().resource::<RouteEntityRegistry>(),
            &branch,
        )
        .expect("an exact prepared branch must freeze one cleanup manifest");
        let entities = manifest.entities();
        let child_end = child_entities
            .iter()
            .map(|entity| {
                entities
                    .iter()
                    .position(|candidate| candidate == entity)
                    .unwrap()
            })
            .max()
            .unwrap();
        let slot_index = entities
            .iter()
            .position(|entity| *entity == child_slot)
            .unwrap();
        let parent_start = parent_entities
            .iter()
            .map(|entity| {
                entities
                    .iter()
                    .position(|candidate| candidate == entity)
                    .unwrap()
            })
            .min()
            .unwrap();

        assert!(child_end < slot_index);
        assert!(slot_index < parent_start);
        assert_eq!(
            manifest.len(),
            child_entities.len() + 1 + parent_entities.len()
        );
    }

    #[test]
    fn failed_prepared_manifest_accepts_multiple_exact_slot_only_roots() {
        let (app, branch) = crate::navigation::tests::failed_deep_nested_branch_fixture();
        let mut realized = branch.into_realized_for_test();
        while realized.pop_frame_for_test().is_some() {}
        let slot_entities = realized
            .child_slots()
            .iter()
            .map(MountedChildSlot::slot)
            .collect::<Vec<_>>();
        assert_eq!(slot_entities.len(), 2);
        let branch = FailedPreparedBranch::new(
            crate::branch_transaction::PreparedBranch::from_realized(realized),
        );

        let manifest = FrozenCleanupManifest::from_failed_prepared_branch(
            app.world(),
            app.world().resource::<RouteEntityRegistry>(),
            &branch,
        )
        .expect("multiple exact slot-only roots need no synthetic frames");

        assert_eq!(manifest.entities(), slot_entities);
    }

    #[test]
    fn failed_prepared_sparse_external_frame_root_uses_exact_mount_slot_boundary() {
        let (app, branch) = crate::navigation::tests::failed_deep_nested_branch_fixture();
        let mut realized = branch.into_realized_for_test();
        let child = realized.pop_frame_for_test().unwrap();
        let child_entities = child
            .claims()
            .map(ExactRegistryClaim::entity)
            .collect::<Vec<_>>();
        let child = crate::branch_transaction::RealizedBranch::from_single_frame(child);
        let branch = FailedPreparedBranch::new(
            crate::branch_transaction::PreparedBranch::from_realized(child),
        );

        let manifest = FrozenCleanupManifest::from_failed_prepared_branch(
            app.world(),
            app.world().resource::<RouteEntityRegistry>(),
            &branch,
        )
        .expect("an exact external mount-slot boundary proves one sparse frame root");

        assert_eq!(
            manifest.entities().into_iter().collect::<HashSet<_>>(),
            child_entities.into_iter().collect::<HashSet<_>>()
        );
    }

    #[test]
    fn failed_prepared_sparse_retained_empty_slot_is_a_leaf() {
        let (app, branch) = crate::navigation::tests::failed_deep_nested_branch_fixture();
        let mut realized = branch.into_realized_for_test();
        let removed_child = realized.pop_frame_for_test().unwrap();
        let retained_slot = realized.child_slots().last().unwrap().slot();
        let parent_entities = realized
            .frames()
            .last()
            .unwrap()
            .claims()
            .map(ExactRegistryClaim::entity)
            .collect::<Vec<_>>();
        drop(removed_child);
        let branch = FailedPreparedBranch::new(
            crate::branch_transaction::PreparedBranch::from_realized(realized),
        );

        let manifest = FrozenCleanupManifest::from_failed_prepared_branch(
            app.world(),
            app.world().resource::<RouteEntityRegistry>(),
            &branch,
        )
        .expect("a retained-empty slot needs no synthetic child frame");
        let entities = manifest.entities();
        let slot_index = entities
            .iter()
            .position(|entity| *entity == retained_slot)
            .unwrap();
        let parent_start = parent_entities
            .iter()
            .map(|entity| {
                entities
                    .iter()
                    .position(|candidate| candidate == entity)
                    .unwrap()
            })
            .min()
            .unwrap();

        assert!(slot_index < parent_start);
    }

    #[test]
    fn failed_prepared_sparse_multiple_frame_roots_are_permutation_stable() {
        let (app, branch) = crate::navigation::tests::failed_deep_nested_branch_fixture();
        let mut realized = branch.into_realized_for_test();
        realized.clear_child_slots_for_test();
        let branch = FailedPreparedBranch::new(
            crate::branch_transaction::PreparedBranch::from_realized(realized),
        );
        let first = FrozenCleanupManifest::from_failed_prepared_branch(
            app.world(),
            app.world().resource::<RouteEntityRegistry>(),
            &branch,
        )
        .expect("each frame has an exact external sparse boundary");

        let mut realized = branch.into_realized_for_test();
        realized.reverse_frames_for_test();
        let branch = FailedPreparedBranch::new(
            crate::branch_transaction::PreparedBranch::from_realized(realized),
        );
        let permuted = FrozenCleanupManifest::from_failed_prepared_branch(
            app.world(),
            app.world().resource::<RouteEntityRegistry>(),
            &branch,
        )
        .expect("input permutation does not change exact sparse roots");

        assert_eq!(first.entities(), permuted.entities());
        assert_eq!(first.digest(), permuted.digest());
    }

    #[test]
    fn failed_prepared_sparse_slot_permutation_preserves_order_and_digest() {
        let (app, branch) = crate::navigation::tests::failed_deep_nested_branch_fixture();
        let mut realized = branch.into_realized_for_test();
        while realized.pop_frame_for_test().is_some() {}
        let branch = FailedPreparedBranch::new(
            crate::branch_transaction::PreparedBranch::from_realized(realized),
        );
        let first = FrozenCleanupManifest::from_failed_prepared_branch(
            app.world(),
            app.world().resource::<RouteEntityRegistry>(),
            &branch,
        )
        .expect("multiple slot-only roots are exact");

        let mut realized = branch.into_realized_for_test();
        realized.reverse_child_slots_for_test();
        let branch = FailedPreparedBranch::new(
            crate::branch_transaction::PreparedBranch::from_realized(realized),
        );
        let permuted = FrozenCleanupManifest::from_failed_prepared_branch(
            app.world(),
            app.world().resource::<RouteEntityRegistry>(),
            &branch,
        )
        .expect("slot input permutation preserves the frozen topology");

        assert_eq!(first.entities(), permuted.entities());
        assert_eq!(first.digest(), permuted.digest());
    }

    #[test]
    fn failed_prepared_sparse_missing_external_boundary_is_rejected_without_world_write() {
        let (mut app, branch) = crate::navigation::tests::failed_deep_nested_branch_fixture();
        let mut realized = branch.into_realized_for_test();
        let child = realized.pop_frame_for_test().unwrap();
        let child_frame = child.frame().clone();
        let child_shell = child.shell();
        let boundary = child.mount_binding().slot_claim().clone();
        let child = crate::branch_transaction::RealizedBranch::from_single_frame(child);
        let branch = FailedPreparedBranch::new(
            crate::branch_transaction::PreparedBranch::from_realized(child),
        );
        assert!(
            app.world_mut()
                .resource_mut::<RouteEntityRegistry>()
                .remove_exact(&boundary)
        );
        let entities_before = app.world().iter_entities().count();

        let prepared = FrozenCleanupManifest::from_failed_prepared_branch(
            app.world(),
            app.world().resource::<RouteEntityRegistry>(),
            &branch,
        )
        .expect("the builder freezes missing boundary evidence for caller classification");
        assert_eq!(
            prepared.boundary_drifts,
            vec![FailedBoundaryDrift {
                source: FailedBoundarySource::FrameMountSlot {
                    frame: child_frame,
                    removed_child: child_shell,
                },
                expected: FailedBoundaryExpectation::ExactClaim(boundary),
                state: FailedManifestClaimState::Missing,
                predicate: FailedMountParentFreezePredicate::ClaimAuthority,
            }]
        );
        assert_eq!(app.world().iter_entities().count(), entities_before);
    }

    #[test]
    fn failed_prepared_sparse_freezes_every_missing_boundary_in_stable_source_order() {
        let (mut app, branch) = crate::navigation::tests::failed_deep_nested_branch_fixture();
        let mut realized = branch.into_realized_for_test();
        realized.clear_child_slots_for_test();
        let expected = realized.frames()[1..]
            .iter()
            .map(|frame| {
                (
                    frame.frame().clone(),
                    frame.shell(),
                    frame.mount_binding().slot_claim().clone(),
                )
            })
            .collect::<Vec<_>>();
        for (_, _, claim) in &expected {
            assert!(
                app.world_mut()
                    .resource_mut::<RouteEntityRegistry>()
                    .remove_exact(claim)
            );
        }
        let branch = FailedPreparedBranch::new(
            crate::branch_transaction::PreparedBranch::from_realized(realized),
        );

        let manifest = FrozenCleanupManifest::from_failed_prepared_branch(
            app.world(),
            app.world().resource::<RouteEntityRegistry>(),
            &branch,
        )
        .unwrap();

        assert_eq!(manifest.boundary_drifts.len(), expected.len());
        for (drift, (frame, removed_child, claim)) in manifest.boundary_drifts.iter().zip(expected)
        {
            assert_eq!(
                drift,
                &FailedBoundaryDrift {
                    source: FailedBoundarySource::FrameMountSlot {
                        frame,
                        removed_child,
                    },
                    expected: FailedBoundaryExpectation::ExactClaim(claim),
                    state: FailedManifestClaimState::Missing,
                    predicate: FailedMountParentFreezePredicate::ClaimAuthority,
                }
            );
        }
    }

    #[test]
    fn failed_prepared_sparse_detailed_mount_drift_retains_exact_source_and_expectation() {
        let (mut app, branch) = crate::navigation::tests::failed_deep_nested_branch_fixture();
        let mut realized = branch.into_realized_for_test();
        realized.clear_child_slots_for_test();
        let frame = &realized.frames()[1];
        let frame_identity = frame.frame().clone();
        let removed_child = frame.shell();
        let boundary = frame.mount_binding().slot_claim().clone();
        app.world_mut()
            .entity_mut(boundary.entity())
            .remove::<bevy_ui::Node>();
        let branch = FailedPreparedBranch::new(
            crate::branch_transaction::PreparedBranch::from_realized(realized),
        );

        let manifest = FrozenCleanupManifest::from_failed_prepared_branch(
            app.world(),
            app.world().resource::<RouteEntityRegistry>(),
            &branch,
        )
        .unwrap();

        let expected_drift = FailedBoundaryDrift {
            source: FailedBoundarySource::FrameMountSlot {
                frame: frame_identity,
                removed_child,
            },
            expected: FailedBoundaryExpectation::ExactClaim(boundary),
            state: FailedManifestClaimState::Exact,
            predicate: FailedMountParentFreezePredicate::RegistryManifestMismatch,
        };
        assert!(
            manifest.boundary_drifts.contains(&expected_drift),
            "expected={expected_drift:#?} actual={:#?}",
            manifest.boundary_drifts
        );
    }

    #[test]
    fn failed_prepared_sparse_local_claim_loss_does_not_flatten_exact_siblings() {
        let (mut app, branch) = crate::navigation::tests::failed_deep_nested_branch_fixture();
        let target = branch.frames()[1]
            .claims()
            .find(|claim| claim.entity() != branch.frames()[1].shell())
            .unwrap()
            .clone();
        app.world_mut()
            .resource_mut::<RouteEntityRegistry>()
            .compare_exchange_lifecycle(&target, EntityLifecycle::Quarantined)
            .unwrap();

        let manifest = FrozenCleanupManifest::from_failed_prepared_branch(
            app.world(),
            app.world().resource::<RouteEntityRegistry>(),
            &branch,
        )
        .expect("one locally lost claim must not erase exact sibling obligations");

        assert_eq!(
            manifest
                .obligations
                .iter()
                .filter(|obligation| obligation.terminal_mode == CleanupTerminalMode::AuthorityLost)
                .map(|obligation| obligation.claim.entity())
                .collect::<Vec<_>>(),
            vec![target.entity()]
        );
        assert!(manifest.obligations.iter().any(|obligation| {
            obligation.claim.entity() != target.entity()
                && obligation.terminal_mode == CleanupTerminalMode::Normal
        }));
    }

    #[test]
    fn failed_prepared_sparse_duplicate_graph_entity_is_rejected() {
        let (app, branch) = crate::navigation::tests::failed_deep_nested_branch_fixture();
        let mut realized = branch.into_realized_for_test();
        let duplicate = realized.child_slots()[0].clone_for_test();
        realized.push_child_slot_unchecked_for_test(duplicate);
        let branch = FailedPreparedBranch::new(
            crate::branch_transaction::PreparedBranch::from_realized(realized),
        );

        assert_eq!(
            FrozenCleanupManifest::from_failed_prepared_branch(
                app.world(),
                app.world().resource::<RouteEntityRegistry>(),
                &branch,
            ),
            Err(FailedManifestBuildError::InvalidTopology(
                FailedPreparedTopologyError::DuplicateEntity,
            ))
        );
    }

    #[test]
    fn failed_prepared_sparse_materializes_residual_and_rejected_topology_nodes() {
        let (residual_app, residual_branch) =
            crate::navigation::tests::failed_mount_residual_fixture();
        let residual_topology = FailedPreparedTopology::build(
            residual_app.world(),
            residual_app.world().resource::<RouteEntityRegistry>(),
            &residual_branch,
        )
        .unwrap();
        assert_eq!(residual_topology.auxiliary_slots.len(), 1);
        assert!(matches!(
            residual_topology.auxiliary_slots[0].source,
            FailedPreparedAuxiliarySlotSource::Residual(_)
        ));

        let (rejected_app, branch) = crate::navigation::tests::failed_deep_nested_branch_fixture();
        let slot = branch.child_slots()[0].clone_for_test();
        let mut empty = RealizedBranch::empty();
        let rejected = empty
            .push_child_slot(slot)
            .expect_err("a slot without its stored parent becomes a rejected owner");
        let rejected_branch = FailedPreparedBranch::with_rejected_child_slot(
            crate::branch_transaction::PreparedBranch::from_realized(empty),
            rejected,
        );
        let rejected_topology = FailedPreparedTopology::build(
            rejected_app.world(),
            rejected_app.world().resource::<RouteEntityRegistry>(),
            &rejected_branch,
        )
        .unwrap();
        assert_eq!(rejected_topology.auxiliary_slots.len(), 1);
        assert!(matches!(
            rejected_topology.auxiliary_slots[0].source,
            FailedPreparedAuxiliarySlotSource::Rejected(_)
        ));
    }

    #[test]
    fn failed_prepared_sparse_residual_and_rejected_duplicates_cross_primary_once() {
        let (residual_app, mut residual_branch) =
            crate::navigation::tests::failed_mount_residual_fixture();
        let residual_claim = residual_branch.mount_residuals()[0].claim().clone();
        let target = residual_branch.frames()[0]
            .claims()
            .find(|claim| claim.entity() != residual_branch.frames()[0].shell())
            .unwrap()
            .clone();
        residual_branch.replace_frame_claim_for_test(0, target.entity(), residual_claim.clone());
        assert!(matches!(
            FailedPreparedTopology::build(
                residual_app.world(),
                residual_app.world().resource::<RouteEntityRegistry>(),
                &residual_branch,
            ),
            Err(FailedManifestBuildError::InvalidTopology(
                FailedPreparedTopologyError::DuplicateEntity
            ))
        ));

        let (rejected_app, branch) = crate::navigation::tests::failed_deep_nested_branch_fixture();
        let mut realized = branch.into_realized_for_test();
        let duplicate = realized.child_slots()[0].clone_for_test();
        let rejected = realized
            .push_child_slot(duplicate)
            .expect_err("stored primary slot rejects the duplicate owner");
        let rejected_branch = FailedPreparedBranch::with_rejected_child_slot(
            crate::branch_transaction::PreparedBranch::from_realized(realized),
            rejected,
        );
        assert!(matches!(
            FailedPreparedTopology::build(
                rejected_app.world(),
                rejected_app.world().resource::<RouteEntityRegistry>(),
                &rejected_branch,
            ),
            Err(FailedManifestBuildError::InvalidTopology(
                FailedPreparedTopologyError::DuplicateEntity
            ))
        ));
    }

    #[test]
    fn failed_prepared_sparse_residual_and_rejected_duplicate_each_other_once() {
        let (app, mut branch) = crate::navigation::tests::failed_mount_residual_fixture();
        let residual = &branch.mount_residuals()[0];
        let (parent, outlet, slot_id) = match residual.claim().owner() {
            RegistryOwner::Provisional(ProvisionalOwner::ChildSlot {
                parent,
                outlet,
                slot,
            }) => (parent.clone(), outlet.clone(), *slot),
            owner => panic!("fixture must retain a provisional child-slot owner: {owner:?}"),
        };
        let committed = residual
            .claim()
            .with_owner_for_test(RegistryOwner::Committed(EntityOwner::ChildSlot {
                parent: parent.clone(),
                outlet: outlet.clone(),
                slot: slot_id,
            }));
        let (_, source) = crate::navigation::tests::failed_deep_nested_branch_fixture();
        let mut rejected_slot = source.child_slots()[0].clone_for_test();
        rejected_slot.replace_slot_binding_for_test(
            residual.entity(),
            committed,
            parent,
            outlet,
            slot_id,
        );
        let mut empty = RealizedBranch::empty();
        let rejected = empty
            .push_child_slot(rejected_slot)
            .expect_err("slot without its stored parent becomes a rejected owner");
        branch.push_rejected_child_slot_for_test(rejected);

        match FailedPreparedTopology::build(
            app.world(),
            app.world().resource::<RouteEntityRegistry>(),
            &branch,
        ) {
            Err(error) => assert_eq!(
                error,
                FailedManifestBuildError::InvalidTopology(
                    FailedPreparedTopologyError::DuplicateEntity,
                )
            ),
            Ok(_) => panic!("residual/rejected duplicate must fail during topology build"),
        }
    }

    #[test]
    fn failed_prepared_sparse_duplicate_external_incoming_is_typed_during_build() {
        let (app, branch) = crate::navigation::tests::failed_deep_nested_branch_fixture();
        let mut realized = branch.into_realized_for_test();
        realized.clear_child_slots_for_test();
        realized.replace_frame_mount_from_for_test(1, 0);
        let branch = FailedPreparedBranch::new(
            crate::branch_transaction::PreparedBranch::from_realized(realized),
        );

        assert_eq!(
            FrozenCleanupManifest::from_failed_prepared_branch(
                app.world(),
                app.world().resource::<RouteEntityRegistry>(),
                &branch,
            ),
            Err(FailedManifestBuildError::InvalidTopology(
                FailedPreparedTopologyError::DuplicateIncoming,
            ))
        );
    }

    #[test]
    fn failed_prepared_sparse_duplicate_owner_and_seal_are_typed_from_stored_input() {
        let (app, branch) = crate::navigation::tests::failed_deep_nested_branch_fixture();
        let mut realized = branch.into_realized_for_test();
        let source = realized.frames()[0]
            .claims()
            .find(|claim| claim.entity() != realized.frames()[0].shell())
            .unwrap()
            .clone();
        let target = realized.frames()[1]
            .claims()
            .find(|claim| claim.entity() != realized.frames()[1].shell())
            .unwrap()
            .clone();
        realized.replace_frame_claim_for_test(
            1,
            target.entity(),
            target.with_owner_for_test(source.owner().clone()),
        );
        let duplicate_owner = FailedPreparedBranch::new(
            crate::branch_transaction::PreparedBranch::from_realized(realized),
        );
        assert_eq!(
            FrozenCleanupManifest::from_failed_prepared_branch(
                app.world(),
                app.world().resource::<RouteEntityRegistry>(),
                &duplicate_owner,
            ),
            Err(FailedManifestBuildError::InvalidTopology(
                FailedPreparedTopologyError::ConflictingOwner,
            ))
        );

        let mut realized = duplicate_owner.into_realized_for_test();
        realized.replace_frame_claim_for_test(
            1,
            target.entity(),
            target.with_seal_for_test(source.seal()),
        );
        let duplicate_seal = FailedPreparedBranch::new(
            crate::branch_transaction::PreparedBranch::from_realized(realized),
        );
        assert_eq!(
            FrozenCleanupManifest::from_failed_prepared_branch(
                app.world(),
                app.world().resource::<RouteEntityRegistry>(),
                &duplicate_seal,
            ),
            Err(FailedManifestBuildError::InvalidTopology(
                FailedPreparedTopologyError::ConflictingOwner,
            ))
        );
    }

    #[test]
    fn failed_prepared_sparse_stored_cycle_is_rootless_through_complete_builder() {
        let (app, branch) = crate::navigation::tests::failed_deep_nested_branch_fixture();
        let mut realized = branch.into_realized_for_test();
        let slot = &realized.child_slots()[0];
        let frame = realized
            .frames()
            .iter()
            .position(|frame| frame.frame() == slot.parent())
            .unwrap();
        let mount_source = realized
            .frames()
            .iter()
            .position(|frame| frame.mount_binding().matches_mounted_child_slot(slot))
            .unwrap();
        realized.retain_cycle_for_test(frame, 0, mount_source);
        let branch = FailedPreparedBranch::new(
            crate::branch_transaction::PreparedBranch::from_realized(realized),
        );

        assert_eq!(
            FrozenCleanupManifest::from_failed_prepared_branch(
                app.world(),
                app.world().resource::<RouteEntityRegistry>(),
                &branch,
            ),
            Err(FailedManifestBuildError::InvalidTopology(
                FailedPreparedTopologyError::Rootless,
            ))
        );
    }

    #[test]
    fn failed_prepared_sparse_shared_revisit_is_typed_invalid_topology() {
        let (app, branch) = crate::navigation::tests::failed_deep_nested_branch_fixture();
        let mut topology = FailedPreparedTopology::build(
            app.world(),
            app.world().resource::<RouteEntityRegistry>(),
            &branch,
        )
        .unwrap();
        let root = topology
            .frames
            .iter()
            .position(|frame| frame.incoming.is_none() && !frame.outgoing.is_empty())
            .unwrap();
        let shared = topology.frames[root].outgoing[0];
        topology.frames[root].outgoing.push(shared);

        assert!(matches!(
            topology
                .freeze_obligations(app.world(), app.world().resource::<RouteEntityRegistry>(),),
            Err(FailedManifestBuildError::InvalidTopology(
                FailedPreparedTopologyError::CycleOrRevisit
            ))
        ));
    }

    #[test]
    fn failed_prepared_sparse_rootless_stored_component_is_typed_invalid_topology() {
        let (app, branch) = crate::navigation::tests::failed_deep_nested_branch_fixture();
        let mut topology = FailedPreparedTopology::build(
            app.world(),
            app.world().resource::<RouteEntityRegistry>(),
            &branch,
        )
        .unwrap();
        for frame in &mut topology.frames {
            frame.incoming = Some(0);
        }
        for slot in &mut topology.slots {
            slot.parent = Some(0);
        }

        assert!(matches!(
            topology
                .freeze_obligations(app.world(), app.world().resource::<RouteEntityRegistry>(),),
            Err(FailedManifestBuildError::InvalidTopology(
                FailedPreparedTopologyError::Rootless
            ))
        ));
    }

    fn projected_leaving_claims(
        registry: &RouteEntityRegistry,
        frames: &[RealizedFrame],
        child_slots: &[MountedChildSlot],
    ) -> Vec<ExactRegistryClaim> {
        let transitions = frames
            .iter()
            .flat_map(|frame| frame.lifecycle_transitions(EntityLifecycle::Leaving))
            .chain(
                child_slots
                    .iter()
                    .map(|slot| (slot.slot_claim().clone(), EntityLifecycle::Leaving)),
            )
            .collect::<Vec<_>>();
        registry
            .prepare_lifecycle_mutation(&transitions)
            .unwrap()
            .resulting_claims()
            .to_vec()
    }

    #[test]
    fn slot_only_leaving_manifest_freezes_exact_obligation_and_mount_parent() {
        let (app, branch, _child_entities, child_slot, _parent_entities) =
            crate::navigation::tests::failed_nested_branch_fixture();
        let slot = &branch.child_slots()[0];
        let registry = app.world().resource::<RouteEntityRegistry>();
        let projected = projected_leaving_claims(registry, &[], std::slice::from_ref(slot));

        let manifest = FrozenCleanupManifest::from_leaving_branch_parts(
            app.world(),
            registry,
            &[],
            std::slice::from_ref(slot),
            &projected,
            None,
        )
        .expect("a slot-only exact leaving residual must freeze independently");

        assert_eq!(manifest.entities(), vec![child_slot]);
        assert_eq!(manifest.mount_parents.len(), 1);
        assert_eq!(manifest.mount_parents[0].claim, *slot.anchor_claim());
        let reserved = CleanupLedger::reserve(manifest).unwrap();
        assert_eq!(reserved.obligations.len(), 1);
        assert_eq!(reserved.receipts, vec![None]);
    }

    #[test]
    fn exact_external_slot_only_authority_branch_preserves_exact_terminal_mode() {
        let (app, branch, _child_entities, child_slot, _parent_entities) =
            crate::navigation::tests::failed_nested_branch_fixture();
        let slot = &branch.child_slots()[0];
        let mut obligations = Vec::new();

        assert!(
            append_exact_authority_branch(
                app.world(),
                app.world().resource::<RouteEntityRegistry>(),
                &[],
                std::slice::from_ref(slot),
                &mut obligations,
            )
            .is_ok()
        );
        assert_eq!(obligations.len(), 1);
        assert_eq!(obligations[0].claim.entity(), child_slot);
        assert_eq!(
            obligations[0].terminal_mode,
            CleanupTerminalMode::AuthorityExact
        );
    }

    #[test]
    fn authority_obligation_normalization_deduplicates_only_identical_bindings() {
        let (app, branch, _child_entities, _child_slot, _parent_entities) =
            crate::navigation::tests::failed_nested_branch_fixture();
        let slot = &branch.child_slots()[0];
        let mut obligations = Vec::new();
        append_exact_authority_branch(
            app.world(),
            app.world().resource::<RouteEntityRegistry>(),
            &[],
            std::slice::from_ref(slot),
            &mut obligations,
        )
        .unwrap();
        let obligation = obligations.pop().unwrap();
        let mut identical = vec![obligation.clone(), obligation.clone()];
        normalize_authority_obligations(&mut identical).unwrap();
        assert_eq!(identical, vec![obligation.clone()]);

        let mut conflicting = obligation.clone();
        conflicting.terminal_mode = CleanupTerminalMode::AuthorityLost;
        let mut conflicting = vec![obligation, conflicting];
        assert_eq!(
            normalize_authority_obligations(&mut conflicting),
            Err(AuthoritySettlementManifestError::InvalidTopology)
        );
    }

    #[test]
    fn authority_structural_duplicate_is_invalid_topology_without_flat_lost_fallback() {
        let (app, branch, _child_entities, child_slot, _parent_entities) =
            crate::navigation::tests::failed_nested_branch_fixture();
        let slot = &branch.child_slots()[0];
        let duplicate = slot.clone_for_test();
        let registry = app.world().resource::<RouteEntityRegistry>();
        let claim_before = registry.record(child_slot).unwrap().claim().clone();
        let mut obligations = Vec::new();

        assert_eq!(
            append_exact_authority_branch(
                app.world(),
                registry,
                branch.frames(),
                &[slot.clone_for_test(), duplicate],
                &mut obligations,
            ),
            Err(AuthoritySettlementManifestError::InvalidTopology)
        );
        assert_eq!(
            app.world()
                .resource::<RouteEntityRegistry>()
                .record(child_slot)
                .unwrap()
                .claim(),
            claim_before
        );
        assert!(obligations.iter().all(|obligation| {
            obligation.terminal_mode != CleanupTerminalMode::AuthorityLost
                || !cleanup_claim_has_exact_authority(app.world(), &obligation.claim)
        }));
    }

    #[test]
    fn nested_leaving_manifest_orders_child_frame_before_slot_and_parent_frame() {
        let (app, branch, child_entities, child_slot, parent_entities) =
            crate::navigation::tests::failed_nested_branch_fixture();
        let registry = app.world().resource::<RouteEntityRegistry>();
        let projected = projected_leaving_claims(registry, branch.frames(), branch.child_slots());

        let manifest = FrozenCleanupManifest::from_leaving_branch_parts(
            app.world(),
            registry,
            branch.frames(),
            branch.child_slots(),
            &projected,
            None,
        )
        .expect("an exact leaving branch must freeze one cleanup manifest");
        let entities = manifest.entities();
        let child_end = child_entities
            .iter()
            .map(|entity| {
                entities
                    .iter()
                    .position(|candidate| candidate == entity)
                    .unwrap()
            })
            .max()
            .unwrap();
        let slot_index = entities
            .iter()
            .position(|entity| *entity == child_slot)
            .unwrap();
        let parent_start = parent_entities
            .iter()
            .map(|entity| {
                entities
                    .iter()
                    .position(|candidate| candidate == entity)
                    .unwrap()
            })
            .min()
            .unwrap();

        assert!(child_end < slot_index);
        assert!(slot_index < parent_start);
        assert_eq!(
            manifest.len(),
            branch
                .frames()
                .iter()
                .map(RealizedFrame::obligation_count)
                .sum::<usize>()
                + branch.child_slots().len()
        );
    }

    #[test]
    fn nested_leaving_manifest_rejects_non_leaving_or_incomplete_projection() {
        let (app, branch, _child_entities, _child_slot, _parent_entities) =
            crate::navigation::tests::failed_nested_branch_fixture();
        let registry = app.world().resource::<RouteEntityRegistry>();
        let mut projected =
            projected_leaving_claims(registry, branch.frames(), branch.child_slots());
        projected.pop();

        assert!(
            FrozenCleanupManifest::from_leaving_branch_parts(
                app.world(),
                registry,
                branch.frames(),
                branch.child_slots(),
                &projected,
                None,
            )
            .is_none()
        );

        let active = registry
            .prepare_lifecycle_mutation(&[(
                branch.child_slots()[0].slot_claim().clone(),
                EntityLifecycle::Active,
            )])
            .unwrap()
            .resulting_claims()[0]
            .clone();
        let mut projected =
            projected_leaving_claims(registry, branch.frames(), branch.child_slots());
        let child_slot = branch.child_slots()[0].slot();
        *projected
            .iter_mut()
            .find(|claim| claim.entity() == child_slot)
            .unwrap() = active;

        assert!(
            FrozenCleanupManifest::from_leaving_branch_parts(
                app.world(),
                registry,
                branch.frames(),
                branch.child_slots(),
                &projected,
                None,
            )
            .is_none()
        );
    }

    #[test]
    fn nested_leaving_cleanup_settles_exact_slot_and_preserves_external_host() {
        let (mut app, branch, _child_entities, child_slot, _parent_entities) =
            crate::navigation::tests::failed_nested_branch_fixture();
        let top_slot = branch.frames()[0].top_slot();
        let app_host = app.world().get::<ChildOf>(top_slot).unwrap().parent();
        let projected = projected_leaving_claims(
            app.world().resource::<RouteEntityRegistry>(),
            branch.frames(),
            branch.child_slots(),
        );
        let manifest = FrozenCleanupManifest::from_leaving_branch_parts(
            app.world(),
            app.world().resource::<RouteEntityRegistry>(),
            branch.frames(),
            branch.child_slots(),
            &projected,
            None,
        )
        .unwrap();
        let obligation_count = manifest.len();
        let transitions = branch
            .frames()
            .iter()
            .flat_map(|frame| frame.lifecycle_transitions(EntityLifecycle::Leaving))
            .chain(
                branch
                    .child_slots()
                    .iter()
                    .map(|slot| (slot.slot_claim().clone(), EntityLifecycle::Leaving)),
            )
            .collect::<Vec<_>>();
        let registry_plan = app
            .world()
            .resource::<RouteEntityRegistry>()
            .prepare_lifecycle_mutation(&transitions)
            .unwrap();
        for frame in branch.frames() {
            for claim in frame.claims() {
                app.world_mut()
                    .entity_mut(claim.entity())
                    .insert(FrameLifecycle(EntityLifecycle::Leaving));
            }
        }
        app.world_mut()
            .entity_mut(child_slot)
            .insert(ChildSlotLifecycle::Leaving);
        registry_plan.apply(
            app.world_mut()
                .resource_mut::<RouteEntityRegistry>()
                .into_inner(),
        );
        app.world_mut().flush();
        let prepared = CleanupLedger::reserve(manifest).unwrap();
        let mut ledger = CleanupLedger::activate(prepared, None);
        let attempt = ledger.attempt(app.world()).unwrap();

        assert!(ledger.settle(app.world_mut(), attempt));
        assert_eq!(ledger.receipt_count(), obligation_count);
        assert!(app.world().get_entity(top_slot).is_ok());
        assert!(app.world().get_entity(app_host).is_ok());
    }

    #[test]
    fn nested_failed_cleanup_only_crosses_the_frozen_exact_owner_boundary() {
        let (mut app, branch, _child_entities, _child_slot, _parent_entities) =
            crate::navigation::tests::failed_nested_branch_fixture();
        let top_slot = branch.frames()[0].top_slot();
        let app_host = app.world().get::<ChildOf>(top_slot).unwrap().parent();
        let foreign = app
            .world_mut()
            .spawn((ForeignBoundaryMarker, ChildOf(app_host)))
            .id();
        let sibling_router = registered_entity(app.world_mut(), app_host);
        app.world_mut().flush();
        let sibling_claim = app
            .world()
            .resource::<RouteEntityRegistry>()
            .record(sibling_router)
            .unwrap()
            .claim();
        let owned = branch
            .frames()
            .iter()
            .flat_map(|frame| frame.claims().map(ExactRegistryClaim::entity))
            .chain(branch.child_slots().iter().map(MountedChildSlot::slot))
            .collect::<Vec<_>>();
        let manifest = FrozenCleanupManifest::from_failed_prepared_branch(
            app.world(),
            app.world().resource::<RouteEntityRegistry>(),
            &branch,
        )
        .unwrap();
        let prepared = CleanupLedger::reserve(manifest).unwrap();
        let mut ledger = CleanupLedger::activate_failed(prepared, branch);
        let attempt = ledger.attempt(app.world()).unwrap();

        assert!(ledger.settle(app.world_mut(), attempt));
        assert!(
            owned
                .iter()
                .all(|entity| app.world().get_entity(*entity).is_err())
        );
        assert!(app.world().get_entity(app_host).is_ok());
        assert!(app.world().get_entity(top_slot).is_ok());
        assert!(app.world().get::<ForeignBoundaryMarker>(foreign).is_some());
        assert_eq!(
            app.world().get::<ChildOf>(foreign).map(ChildOf::parent),
            Some(app_host)
        );
        assert_eq!(
            app.world()
                .resource::<RouteEntityRegistry>()
                .record(sibling_router)
                .map(|record| record.claim()),
            Some(sibling_claim)
        );
        assert_eq!(
            app.world()
                .get::<ChildOf>(sibling_router)
                .map(ChildOf::parent),
            Some(app_host)
        );
    }

    #[test]
    fn nested_cleanup_exact_claim_drift_in_each_unit_is_world_zero() {
        for unit in 0..3 {
            let (mut app, branch, _child_entities, child_slot, _parent_entities) =
                crate::navigation::tests::failed_nested_branch_fixture();
            let target = match unit {
                0 => branch.frames()[1].shell(),
                1 => child_slot,
                2 => branch.frames()[0].shell(),
                _ => unreachable!(),
            };
            let manifest = FrozenCleanupManifest::from_failed_prepared_branch(
                app.world(),
                app.world().resource::<RouteEntityRegistry>(),
                &branch,
            )
            .unwrap();
            let prepared = CleanupLedger::reserve(manifest).unwrap();
            let mut ledger = CleanupLedger::activate_failed(prepared, branch);
            let frozen_claim = ledger
                .obligations
                .iter()
                .find(|obligation| obligation.claim.entity() == target)
                .unwrap()
                .claim
                .clone();
            let drifted_claim = app
                .world_mut()
                .resource_mut::<RouteEntityRegistry>()
                .compare_exchange_lifecycle(&frozen_claim, EntityLifecycle::Quarantined)
                .unwrap();
            *app.world_mut().resource_mut::<InputFocus>() = InputFocus::from_entity(target);
            app.world_mut()
                .resource_mut::<RouteFocusState>()
                .set_focused(Some(target));
            let registry_before = app
                .world()
                .resource::<RouteEntityRegistry>()
                .records()
                .to_vec();
            let world_before = app
                .world()
                .iter_entities()
                .map(|entity| (entity.id(), entity.archetype().components().to_vec()))
                .collect::<Vec<_>>();
            let attempt = ledger.attempt(app.world()).unwrap();
            let obligation = ledger
                .obligations
                .iter()
                .position(|obligation| obligation.claim.entity() == target)
                .unwrap();

            assert!(
                attempt
                    .evidence
                    .contains(&(obligation, CleanupEvidence::StaleClaim))
            );
            assert!(!ledger.settle(app.world_mut(), attempt));
            assert_eq!(ledger.receipt_count(), 0);
            assert_eq!(ledger.failed_prepared_frames().len(), 2);
            assert_eq!(app.world().resource::<InputFocus>().get(), Some(target));
            assert_eq!(
                app.world().resource::<RouteFocusState>().focused(),
                Some(target)
            );
            assert_eq!(
                app.world().resource::<RouteEntityRegistry>().records(),
                registry_before
            );
            assert_eq!(
                app.world()
                    .resource::<RouteEntityRegistry>()
                    .record(target)
                    .map(|record| record.claim()),
                Some(drifted_claim)
            );
            assert_eq!(
                app.world()
                    .iter_entities()
                    .map(|entity| (entity.id(), entity.archetype().components().to_vec()))
                    .collect::<Vec<_>>(),
                world_before
            );
        }
    }

    #[test]
    fn failed_prepared_branch_manifest_survives_child_slot_relationship_drift() {
        let (mut app, branch, _child_entities, child_slot, _parent_entities) =
            crate::navigation::tests::failed_nested_branch_fixture();
        app.world_mut().entity_mut(child_slot).remove::<ChildOf>();
        app.world_mut().flush();

        let manifest = FrozenCleanupManifest::from_failed_prepared_branch(
            app.world(),
            app.world().resource::<RouteEntityRegistry>(),
            &branch,
        )
        .expect(
            "stored exact ownership must remain cleanup-authoritative after relationship drift",
        );

        assert!(manifest.entities().contains(&child_slot));
    }

    #[test]
    fn failed_prepared_branch_child_slot_lifecycle_drift_is_locally_lost_and_bounded() {
        let (mut app, branch, _child_entities, child_slot, _parent_entities) =
            crate::navigation::tests::failed_nested_branch_fixture();
        let exact_claim = app
            .world()
            .resource::<RouteEntityRegistry>()
            .record(child_slot)
            .unwrap()
            .claim();
        app.world_mut()
            .entity_mut(child_slot)
            .insert(ChildSlotLifecycle::ProvisionalInert);
        app.world_mut().flush();
        let manifest = FrozenCleanupManifest::from_failed_prepared_branch(
            app.world(),
            app.world().resource::<RouteEntityRegistry>(),
            &branch,
        )
        .unwrap();
        let prepared = CleanupLedger::reserve(manifest).unwrap();
        let mut ledger = CleanupLedger::activate_failed(prepared, branch);
        let attempt = ledger.attempt(app.world()).unwrap();

        assert!(!ledger.settle(app.world_mut(), attempt));
        assert_eq!(ledger.receipt_count(), ledger.obligations.len());
        let retry = ledger.attempt(app.world()).unwrap();
        assert!(ledger.settle(app.world_mut(), retry));
        assert!(ledger.is_settled());
        assert_eq!(
            app.world().get::<ChildSlotLifecycle>(child_slot),
            Some(&ChildSlotLifecycle::ProvisionalInert)
        );
        assert!(
            app.world()
                .resource::<RouteEntityRegistry>()
                .record(child_slot)
                .is_none()
        );
        assert!(
            app.world()
                .resource::<QuarantineLeaseTable>()
                .authority_lost_tombstone(child_slot)
                .is_some_and(|tombstone| tombstone.former_claim() == &exact_claim)
        );
    }

    #[test]
    fn nested_cleanup_accepts_exact_active_and_leaving_child_slot_claims() {
        let (app, _branch, _child_entities, child_slot, _parent_entities) =
            crate::navigation::tests::failed_nested_branch_fixture();
        let registry = app.world().resource::<RouteEntityRegistry>();
        let current = registry.record(child_slot).unwrap().claim();
        let active = registry
            .prepare_lifecycle_mutation(&[(current.clone(), EntityLifecycle::Active)])
            .unwrap()
            .resulting_claims()[0]
            .clone();
        let leaving = registry
            .prepare_lifecycle_mutation(&[(current, EntityLifecycle::Leaving)])
            .unwrap()
            .resulting_claims()[0]
            .clone();
        let authority = app.world().get::<ChildSlotAuthority>(child_slot);

        assert!(child_slot_components_match_claim(
            Some(&ChildSlotLifecycle::Active),
            authority,
            &active,
        ));
        assert!(child_slot_components_match_claim(
            Some(&ChildSlotLifecycle::Leaving),
            authority,
            &leaving,
        ));
        assert!(child_slot_components_are_compatible(
            Some(&ChildSlotLifecycle::Active),
            authority,
            &active,
            false,
        ));
        assert!(child_slot_components_are_compatible(
            Some(&ChildSlotLifecycle::Leaving),
            authority,
            &leaving,
            false,
        ));
    }

    fn single_frame_cleanup_ledger(world: &World, frame: &RealizedFrame) -> CleanupLedger {
        let manifest = FrozenCleanupManifest::from_current_frame(
            world,
            world.resource::<RouteEntityRegistry>(),
            frame,
        )
        .unwrap();
        CleanupLedger::activate(CleanupLedger::reserve(manifest).unwrap(), None)
    }

    fn assert_zero_write_frame_cleanup(
        world: &mut World,
        ledger: &mut CleanupLedger,
        frame_entities: &[Entity],
    ) {
        let attempt = ledger.attempt(world).unwrap();
        assert!(!ledger.settle(world, attempt));
        assert_eq!(ledger.receipt_count(), 0);
        assert!(
            frame_entities
                .iter()
                .all(|entity| world.get_entity(*entity).is_ok())
        );
    }

    #[test]
    fn external_host_top_seal_drift_is_zero_write() {
        let (mut app, branch, _child_entities, _child_slot, _parent_entities) =
            crate::navigation::tests::failed_nested_branch_fixture();
        let frame = &branch.frames()[0];
        let top_slot = frame.top_slot();
        let frame_entities = frame
            .claims()
            .map(ExactRegistryClaim::entity)
            .collect::<Vec<_>>();
        let mut ledger = single_frame_cleanup_ledger(app.world(), frame);
        let drifted = app
            .world_mut()
            .resource_mut::<RouteEntityRegistry>()
            .issue_seal();
        app.world_mut()
            .entity_mut(top_slot)
            .insert(RouteOwnershipSeal::new(drifted));

        assert_zero_write_frame_cleanup(app.world_mut(), &mut ledger, &frame_entities);
    }

    #[test]
    fn external_host_top_value_drift_is_zero_write_and_retains_owner() {
        for drift_node in [false, true] {
            let (mut app, branch, _child_entities, _child_slot, _parent_entities) =
                crate::navigation::tests::failed_nested_branch_fixture();
            let frame = &branch.frames()[0];
            let top_slot = frame.top_slot();
            let focus = frame.shell();
            let claims = branch
                .frames()
                .iter()
                .flat_map(|frame| frame.claims().cloned())
                .collect::<Vec<_>>();
            let manifest = FrozenCleanupManifest::from_failed_prepared_branch(
                app.world(),
                app.world().resource::<RouteEntityRegistry>(),
                &branch,
            )
            .unwrap();
            let prepared = CleanupLedger::reserve(manifest).unwrap();
            let mut ledger = CleanupLedger::activate_failed(prepared, branch);
            *app.world_mut().resource_mut::<InputFocus>() = InputFocus::from_entity(focus);
            app.world_mut()
                .resource_mut::<RouteFocusState>()
                .set_focused(Some(focus));
            if drift_node {
                app.world_mut().entity_mut(top_slot).insert(Node {
                    width: Val::Percent(99.0),
                    height: Val::Percent(100.0),
                    ..Node::default()
                });
            } else {
                app.world_mut()
                    .entity_mut(top_slot)
                    .insert(RouterInputBarrier::allow_all());
            }
            app.world_mut().flush();
            let attempt = ledger.attempt(app.world()).unwrap();

            assert!(!ledger.settle(app.world_mut(), attempt));
            assert_eq!(ledger.receipt_count(), 0);
            assert_eq!(ledger.failed_prepared_frames().len(), 2);
            assert_eq!(app.world().resource::<InputFocus>().get(), Some(focus));
            assert_eq!(
                app.world().resource::<RouteFocusState>().focused(),
                Some(focus)
            );
            assert!(claims.iter().all(|claim| {
                app.world().get_entity(claim.entity()).is_ok()
                    && app
                        .world()
                        .resource::<RouteEntityRegistry>()
                        .record(claim.entity())
                        .is_some_and(|record| record.claim() == *claim)
            }));
            if drift_node {
                assert_eq!(
                    app.world().get::<Node>(top_slot).map(|node| node.width),
                    Some(Val::Percent(99.0))
                );
            } else {
                assert!(
                    !app.world()
                        .get::<RouterInputBarrier>(top_slot)
                        .unwrap()
                        .denies_all()
                );
            }
        }
    }

    #[test]
    fn attached_shell_relationship_drift_is_zero_write() {
        let (mut app, branch, _child_entities, _child_slot, _parent_entities) =
            crate::navigation::tests::failed_nested_branch_fixture();
        let frame = &branch.frames()[0];
        let shell = frame.shell();
        let frame_entities = frame
            .claims()
            .map(ExactRegistryClaim::entity)
            .collect::<Vec<_>>();
        let mut ledger = single_frame_cleanup_ledger(app.world(), frame);
        assert!(frame.shell_is_attached());
        app.world_mut().entity_mut(shell).remove::<ChildOf>();
        app.world_mut().flush();

        assert_zero_write_frame_cleanup(app.world_mut(), &mut ledger, &frame_entities);
    }

    #[test]
    fn external_child_slot_lifecycle_and_authority_drift_are_zero_write() {
        for authority_drift in [false, true] {
            let (mut app, branch, _child_entities, _child_slot, _parent_entities) =
                crate::navigation::tests::failed_nested_branch_fixture();
            let frame = &branch.frames()[1];
            let mounted = &branch.child_slots()[0];
            let slot = mounted.slot();
            let frame_entities = frame
                .claims()
                .map(ExactRegistryClaim::entity)
                .collect::<Vec<_>>();
            let mut ledger = single_frame_cleanup_ledger(app.world(), frame);
            if authority_drift {
                app.world_mut()
                    .entity_mut(slot)
                    .insert(ChildSlotAuthority::new(
                        mounted.parent().clone(),
                        RouteSlotKey::named("drifted").unwrap(),
                        mounted.slot_id(),
                    ));
            } else {
                app.world_mut()
                    .entity_mut(slot)
                    .insert(ChildSlotLifecycle::ProvisionalInert);
            }

            assert_zero_write_frame_cleanup(app.world_mut(), &mut ledger, &frame_entities);
        }
    }

    #[test]
    fn external_child_slot_value_drift_is_zero_write_and_retains_owner() {
        for drift_node in [false, true] {
            let (mut app, branch, _child_entities, _child_slot, _parent_entities) =
                crate::navigation::tests::failed_nested_branch_fixture();
            let frame = &branch.frames()[1];
            let slot = branch.child_slots()[0].slot();
            let focus = frame.shell();
            let frame_entities = frame
                .claims()
                .map(ExactRegistryClaim::entity)
                .collect::<Vec<_>>();
            let mut ledger = single_frame_cleanup_ledger(app.world(), frame);
            *app.world_mut().resource_mut::<InputFocus>() = InputFocus::from_entity(focus);
            app.world_mut()
                .resource_mut::<RouteFocusState>()
                .set_focused(Some(focus));
            if drift_node {
                app.world_mut().entity_mut(slot).insert(Node {
                    width: Val::Percent(99.0),
                    height: Val::Percent(100.0),
                    ..Node::default()
                });
            } else {
                app.world_mut()
                    .entity_mut(slot)
                    .insert(RouterInputBarrier::allow_all());
            }
            app.world_mut().flush();

            assert_zero_write_frame_cleanup(app.world_mut(), &mut ledger, &frame_entities);
            assert_eq!(branch.frames().len(), 2);
            assert_eq!(app.world().resource::<InputFocus>().get(), Some(focus));
            assert_eq!(
                app.world().resource::<RouteFocusState>().focused(),
                Some(focus)
            );
            if drift_node {
                assert_eq!(
                    app.world().get::<Node>(slot).map(|node| node.width),
                    Some(Val::Percent(99.0))
                );
            } else {
                assert!(
                    !app.world()
                        .get::<RouterInputBarrier>(slot)
                        .unwrap()
                        .denies_all()
                );
            }
        }
    }

    #[test]
    fn detailed_mount_parent_error_preserves_unsupported_owner_fact() {
        let (app, branch, _child_entities, _child_slot, _parent_entities) =
            crate::navigation::tests::failed_nested_branch_fixture();
        let mounted = &branch.child_slots()[0];
        let obligations = vec![
            mounted_slot_obligation(
                app.world(),
                app.world().resource::<RouteEntityRegistry>(),
                mounted,
            )
            .unwrap(),
        ];
        let shell_claim = branch.frames()[0]
            .claims()
            .find(|claim| {
                matches!(
                    claim.owner(),
                    RegistryOwner::Committed(EntityOwner::FrameShell(_))
                )
            })
            .unwrap()
            .clone();
        let mut candidate = mounted_anchor_parent_candidate(mounted).unwrap();
        candidate.claim = shell_claim.clone();

        let error = freeze_external_mount_parents_detailed(
            app.world(),
            app.world().resource::<RouteEntityRegistry>(),
            &obligations,
            [candidate],
        )
        .unwrap_err();

        assert_eq!(error.source, None);
        assert_eq!(
            error.expected,
            FailedBoundaryExpectation::ExactClaim(shell_claim)
        );
        assert_eq!(error.state, FailedManifestClaimState::Exact);
        assert_eq!(
            error.predicate,
            FailedMountParentFreezePredicate::UnsupportedOwner
        );
    }

    #[test]
    fn detailed_mount_parent_errors_preserve_each_physical_predicate() {
        fn freeze_error(case: usize) -> FailedMountParentFreezeError {
            let (mut app, branch, _child_entities, _child_slot, _parent_entities) =
                crate::navigation::tests::failed_nested_branch_fixture();
            let mounted = &branch.child_slots()[0];
            let anchor = mounted.anchor();
            let anchor_claim = mounted.anchor_claim().clone();
            let obligations = vec![
                mounted_slot_obligation(
                    app.world(),
                    app.world().resource::<RouteEntityRegistry>(),
                    mounted,
                )
                .unwrap(),
            ];
            let mut first = mounted_anchor_parent_candidate(mounted).unwrap();
            let mut second = None;
            match case {
                0 => {
                    let foreign = app.world_mut().spawn_empty().id();
                    first.children_before = vec![foreign];
                }
                1 => {
                    let mut conflict = mounted_anchor_parent_candidate(mounted).unwrap();
                    conflict.removed_child = app.world_mut().spawn_empty().id();
                    conflict.removed_child_was_attached = false;
                    second = Some(conflict);
                }
                2 => {
                    assert!(
                        app.world_mut()
                            .resource_mut::<RouteEntityRegistry>()
                            .remove_exact(&anchor_claim)
                    );
                }
                3 => {
                    first.children_before.clear();
                    first.children_after.clear();
                    first.removed_child_was_attached = false;
                    assert!(app.world_mut().despawn(anchor));
                }
                4 => {
                    app.world_mut()
                        .resource_mut::<RouteEntityRegistry>()
                        .compare_exchange_lifecycle(&anchor_claim, EntityLifecycle::Quarantined)
                        .unwrap();
                }
                5 => {
                    first.supplied_parent = Some((app.world_mut().spawn_empty().id(), None));
                }
                6 => {
                    app.world_mut().entity_mut(anchor).insert(Node {
                        width: Val::Percent(99.0),
                        height: Val::Percent(100.0),
                        ..Node::default()
                    });
                }
                _ => unreachable!(),
            }
            let candidates = match second {
                Some(second) => vec![first, second],
                None => vec![first],
            };
            freeze_external_mount_parents_detailed(
                app.world(),
                app.world().resource::<RouteEntityRegistry>(),
                &obligations,
                candidates,
            )
            .unwrap_err()
        }

        let expected = [
            (
                FailedMountParentFreezePredicate::ChildrenProjection,
                FailedManifestClaimState::Exact,
            ),
            (
                FailedMountParentFreezePredicate::ConflictingCandidate,
                FailedManifestClaimState::Exact,
            ),
            (
                FailedMountParentFreezePredicate::MissingRegistryRecord,
                FailedManifestClaimState::Missing,
            ),
            (
                FailedMountParentFreezePredicate::MissingPhysicalManifest,
                FailedManifestClaimState::Absent,
            ),
            (
                FailedMountParentFreezePredicate::RegistryManifestMismatch,
                FailedManifestClaimState::Replaced,
            ),
            (
                FailedMountParentFreezePredicate::ParentAuthority,
                FailedManifestClaimState::Exact,
            ),
            (
                FailedMountParentFreezePredicate::PhysicalAuthority,
                FailedManifestClaimState::Exact,
            ),
        ];
        for (case, (predicate, state)) in expected.into_iter().enumerate() {
            let error = freeze_error(case);
            assert_eq!(error.source, None, "case={case}");
            assert!(matches!(
                error.expected,
                FailedBoundaryExpectation::ExactClaim(_)
            ));
            assert_eq!(error.state, state, "case={case}");
            assert_eq!(error.predicate, predicate, "case={case}");
        }
    }

    #[test]
    pub(crate) fn external_frame_output_value_and_manifest_drift_is_zero_write_and_retains_focus() {
        for drift in 0..6 {
            let (mut app, branch, _child_entities, _child_slot, _parent_entities) =
                crate::navigation::tests::failed_nested_branch_fixture();
            let mounted = &branch.child_slots()[0];
            let anchor = mounted.anchor();
            let slot = mounted.slot();
            let obligations = vec![
                mounted_slot_obligation(
                    app.world(),
                    app.world().resource::<RouteEntityRegistry>(),
                    mounted,
                )
                .unwrap(),
            ];
            let mount_parents = freeze_external_mount_parents(
                app.world(),
                app.world().resource::<RouteEntityRegistry>(),
                &obligations,
                [mounted_anchor_parent_candidate(mounted).unwrap()],
            )
            .unwrap();
            assert_eq!(mount_parents[0].claim.entity(), anchor);
            let digest = frozen_manifest_digest(&obligations, &mount_parents);
            let manifest = FrozenCleanupManifest {
                obligations,
                mount_parents,
                boundary_drifts: Vec::new(),
                pending_boundary_loss_claims: Vec::new(),
                digest,
            };
            let prepared = CleanupLedger::reserve(manifest).unwrap();
            let mut ledger = CleanupLedger::activate(prepared, None);
            *app.world_mut().resource_mut::<InputFocus>() = InputFocus::from_entity(slot);
            app.world_mut()
                .resource_mut::<RouteFocusState>()
                .set_focused(Some(slot));
            match drift {
                0 => {
                    app.world_mut().entity_mut(anchor).insert(Node {
                        width: Val::Percent(99.0),
                        height: Val::Percent(100.0),
                        ..Node::default()
                    });
                }
                1 => {
                    app.world_mut()
                        .entity_mut(anchor)
                        .insert(Visibility::Visible);
                }
                2 => {
                    app.world_mut().entity_mut(anchor).insert(Pickable {
                        should_block_lower: false,
                        is_hoverable: true,
                    });
                }
                3 => {
                    app.world_mut()
                        .entity_mut(anchor)
                        .insert(RouterInputBarrier::allow_all());
                }
                4 => {
                    app.world_mut()
                        .entity_mut(anchor)
                        .insert(ExternalParentDriftMarker);
                }
                5 => {
                    let foreign = app.world_mut().spawn_empty().id();
                    app.world_mut().entity_mut(foreign).insert(ChildOf(anchor));
                }
                _ => unreachable!(),
            }
            app.world_mut().flush();
            let attempt = ledger.attempt(app.world()).unwrap();

            assert!(
                !ledger.settle(app.world_mut(), attempt),
                "external mount-parent drift case {drift} must reject cleanup",
            );
            assert_eq!(ledger.receipt_count(), 0);
            assert!(app.world().get_entity(slot).is_ok());
            assert_eq!(app.world().resource::<InputFocus>().get(), Some(slot));
            assert_eq!(
                app.world().resource::<RouteFocusState>().focused(),
                Some(slot)
            );
        }
    }

    #[test]
    fn external_child_slot_relationship_and_children_order_drift_are_zero_write() {
        for relationship_drift in [false, true] {
            let (mut app, branch, _child_entities, _child_slot, _parent_entities) =
                crate::navigation::tests::failed_nested_branch_fixture();
            let frame = &branch.frames()[1];
            let slot = branch.child_slots()[0].slot();
            let frame_entities = frame
                .claims()
                .map(ExactRegistryClaim::entity)
                .collect::<Vec<_>>();
            let mut ledger = single_frame_cleanup_ledger(app.world(), frame);
            let foreign = app.world_mut().spawn_empty().id();
            if relationship_drift {
                app.world_mut().entity_mut(slot).insert(ChildOf(foreign));
            } else {
                app.world_mut().entity_mut(foreign).insert(ChildOf(slot));
            }
            app.world_mut().flush();

            assert_zero_write_frame_cleanup(app.world_mut(), &mut ledger, &frame_entities);
        }
    }

    #[test]
    fn frame_quarantine_preserves_foreign_child_slot_lifecycle() {
        let (mut app, branch, _child_entities, _child_slot, _parent_entities) =
            crate::navigation::tests::failed_nested_branch_fixture();
        let frame_shell = branch.frames()[0].shell();
        let manifest = FrozenCleanupManifest::from_failed_prepared_branch(
            app.world(),
            app.world().resource::<RouteEntityRegistry>(),
            &branch,
        )
        .unwrap();
        app.world_mut()
            .entity_mut(frame_shell)
            .insert(ChildSlotLifecycle::ProvisionalInert);
        app.world_mut().flush();
        let prepared = CleanupLedger::reserve(manifest).unwrap();
        let mut ledger = CleanupLedger::activate_failed(prepared, branch);
        let attempt = ledger.attempt(app.world()).unwrap();

        assert!(ledger.settle(app.world_mut(), attempt));
        assert_eq!(
            app.world().get::<FrameLifecycle>(frame_shell),
            Some(&FrameLifecycle(EntityLifecycle::Quarantined))
        );
        assert_eq!(
            app.world().get::<ChildSlotLifecycle>(frame_shell),
            Some(&ChildSlotLifecycle::ProvisionalInert)
        );
    }

    #[test]
    fn exhausted_ledger_domain_never_advances_or_reuses_authority() {
        let counter = AtomicU64::new(u64::MAX - 1);

        assert_eq!(allocate_ledger_domain(&counter), Some(u64::MAX - 1));
        assert_eq!(counter.load(Ordering::Relaxed), u64::MAX);
        assert_eq!(allocate_ledger_domain(&counter), None);
        assert_eq!(allocate_ledger_domain(&counter), None);
        assert_eq!(counter.load(Ordering::Relaxed), u64::MAX);
    }

    #[test]
    fn cleanup_ledger_reserve_returns_manifest_and_classifies_every_failure_domain() {
        let (mut app, branch, _, _, _) = crate::navigation::tests::failed_nested_branch_fixture();
        let top_slot = app
            .world()
            .resource::<crate::navigation::RouterNavigation>()
            .entries[0]
            .host
            .as_ref()
            .unwrap()
            .top_slot;
        let stale_seal = app
            .world_mut()
            .resource_mut::<RouteEntityRegistry>()
            .issue_seal();
        app.world_mut()
            .entity_mut(top_slot)
            .insert(RouteOwnershipSeal::new(stale_seal));
        let unfinalized = FrozenCleanupManifest::from_failed_prepared_branch(
            app.world(),
            app.world().resource::<RouteEntityRegistry>(),
            &branch,
        )
        .unwrap();
        assert!(!unfinalized.boundary_drifts().is_empty());
        let boundary_counter = AtomicU64::new(41);
        let boundary =
            match CleanupLedger::reserve_with_domain_counter(unfinalized, &boundary_counter) {
                Err(error) => error,
                Ok(_) => panic!("unfinalized boundary facts are a permanent reserve contradiction"),
            };
        assert_eq!(
            boundary.kind(),
            CleanupLedgerReserveErrorKind::UnfinalizedBoundaryFacts
        );
        assert!(!boundary.into_manifest().boundary_drifts().is_empty());
        assert_eq!(boundary_counter.load(Ordering::Relaxed), 41);

        let capacity_counter = AtomicU64::new(51);
        fail_next_reservation_for_test();
        let capacity = match CleanupLedger::reserve_with_domain_counter(
            FrozenCleanupManifest::empty(),
            &capacity_counter,
        ) {
            Err(error) => error,
            Ok(_) => panic!("receipt reservation fault is retryable capacity"),
        };
        assert_eq!(
            capacity.kind(),
            CleanupLedgerReserveErrorKind::RetryableCapacity
        );
        assert_eq!(capacity.into_manifest().len(), 0);
        assert_eq!(capacity_counter.load(Ordering::Relaxed), 51);

        let exhausted_counter = AtomicU64::new(u64::MAX - 1);
        let prepared = CleanupLedger::reserve_with_domain_counter(
            FrozenCleanupManifest::empty(),
            &exhausted_counter,
        )
        .unwrap();
        assert_eq!(prepared.ledger_domain, u64::MAX - 1);
        assert_eq!(exhausted_counter.load(Ordering::Relaxed), u64::MAX);
        let exhausted = match CleanupLedger::reserve_with_domain_counter(
            FrozenCleanupManifest::empty(),
            &exhausted_counter,
        ) {
            Err(error) => error,
            Ok(_) => panic!("MAX-1 is the last successful domain allocation"),
        };
        assert_eq!(
            exhausted.kind(),
            CleanupLedgerReserveErrorKind::DomainExhausted
        );
        assert_eq!(exhausted.into_manifest().len(), 0);
        assert_eq!(exhausted_counter.load(Ordering::Relaxed), u64::MAX);
        let repeated = match CleanupLedger::reserve_with_domain_counter(
            FrozenCleanupManifest::empty(),
            &exhausted_counter,
        ) {
            Err(error) => error,
            Ok(_) => panic!("exhausted domain remains terminal and cannot wrap"),
        };
        assert_eq!(
            repeated.kind(),
            CleanupLedgerReserveErrorKind::DomainExhausted
        );
        assert_eq!(exhausted_counter.load(Ordering::Relaxed), u64::MAX);
    }

    #[test]
    fn reconciliation_zero_obligation_completion_is_exact_once() {
        let (mut engine, batch) = published_empty_batch();
        let prepared = CleanupLedger::prepare(batch, FrozenCleanupManifest::empty()).unwrap();
        let ledger = CleanupLedger::activate(prepared, None);
        let first = ledger.pending_completion().unwrap();
        let replay = ledger.pending_completion().unwrap();
        engine.complete_cleanup(first).unwrap();
        assert!(engine.complete_cleanup(replay).is_err());
    }

    #[test]
    fn reconciliation_sealed_batch_must_match_the_frozen_ecs_manifest() {
        let (_engine, batch) = published_empty_batch();
        assert!(cleanup_batch_matches(&batch, 0, 0));
        assert!(!cleanup_batch_matches(&batch, 1, 0));
        assert!(!cleanup_batch_matches(&batch, 0, 1));
    }

    fn one_entity_failure_ledger(
        world: &mut World,
        entity: Entity,
        expected_parent: Entity,
    ) -> CleanupLedger {
        let claim = world
            .resource::<RouteEntityRegistry>()
            .record(entity)
            .unwrap()
            .claim();
        let manifest = world
            .resource::<RouteEntityRegistry>()
            .record(entity)
            .unwrap()
            .manifest()
            .clone();
        let obligations = vec![CleanupObligation {
            claim,
            manifest,
            expected_parent,
            unit_root: entity,
            authorized_parent: expected_parent,
            allow_missing_components: false,
            terminal_mode: CleanupTerminalMode::Normal,
        }];
        let manifest = FrozenCleanupManifest {
            digest: manifest_digest(&obligations),
            obligations,
            mount_parents: Vec::new(),
            boundary_drifts: Vec::new(),
            pending_boundary_loss_claims: Vec::new(),
        };
        let prepared =
            CleanupLedger::prepare_physical(None, manifest, &NEXT_LEDGER_DOMAIN).unwrap();
        CleanupLedger::activate(prepared, None)
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

    fn registered_entity(world: &mut World, parent: Entity) -> Entity {
        let frame = frame_fixture();
        let outlet = RouteSlotKey::default();
        let slot = frame.slot();
        let entity = world.spawn_empty().id();
        let initial_manifest = ComponentManifest::from_world(world, entity).unwrap();
        let (seal, claim) = {
            let mut registry = world.resource_mut::<RouteEntityRegistry>();
            let seal = registry.issue_seal();
            registry
                .register(
                    entity,
                    RegistryOwner::Provisional(ProvisionalOwner::ChildSlot {
                        parent: frame.clone(),
                        outlet: outlet.clone(),
                        slot,
                    }),
                    seal,
                    initial_manifest.clone(),
                    EntityLifecycle::PreparedInert,
                )
                .unwrap();
            let claim = registry.record(entity).unwrap().claim();
            (seal, claim)
        };
        world.entity_mut(entity).insert((
            ChildOf(parent),
            RouteOwnershipSeal::new(seal),
            ChildSlotLifecycle::ProvisionalInert,
            ChildSlotAuthority::new(frame, outlet, slot),
        ));
        world.flush();
        let manifest = ComponentManifest::from_world(world, entity).unwrap();
        assert!(
            world
                .resource_mut::<RouteEntityRegistry>()
                .compare_exchange_manifest_exact(&claim, &initial_manifest, manifest)
        );
        entity
    }

    #[test]
    fn exact_clean_removal_despawns_and_removes_the_registry_claim() {
        let mut world = World::new();
        world.insert_resource(RouteEntityRegistry::new());
        world.insert_resource(QuarantineLeaseTable::new());
        let parent = world.spawn_empty().id();
        let entity = registered_entity(&mut world, parent);
        let mut ledger = one_entity_failure_ledger(&mut world, entity, parent);

        let attempt = ledger.attempt(&world).unwrap();
        assert!(ledger.settle(&mut world, attempt));

        assert!(world.get_entity(entity).is_err());
        assert!(
            world
                .resource::<RouteEntityRegistry>()
                .record(entity)
                .is_none()
        );
        assert_eq!(ledger.disposition(0), Some(CleanupDisposition::Removed));
    }

    #[test]
    fn absent_exact_generation_never_touches_a_reused_index() {
        #[derive(Component)]
        struct Replacement;

        let mut world = World::new();
        world.insert_resource(RouteEntityRegistry::new());
        world.insert_resource(QuarantineLeaseTable::new());
        let parent = world.spawn_empty().id();
        let entity = registered_entity(&mut world, parent);
        let mut ledger = one_entity_failure_ledger(&mut world, entity, parent);
        let replacement = world.despawn_no_free(entity).unwrap();
        world.spawn_at(replacement, Replacement).unwrap();
        assert_eq!(replacement.index(), entity.index());
        assert_ne!(replacement, entity);

        let attempt = ledger.attempt(&world).unwrap();
        assert!(ledger.settle(&mut world, attempt));

        assert!(world.get::<Replacement>(replacement).is_some());
        assert_eq!(
            ledger.disposition(0),
            Some(CleanupDisposition::AlreadyAbsent)
        );
    }

    #[test]
    fn absent_entity_with_an_alternative_registry_claim_is_stale_not_complete() {
        let mut world = World::new();
        world.insert_resource(RouteEntityRegistry::new());
        world.insert_resource(QuarantineLeaseTable::new());
        let parent = world.spawn_empty().id();
        let entity = registered_entity(&mut world, parent);
        let mut ledger = one_entity_failure_ledger(&mut world, entity, parent);
        let original = ledger.obligations[0].claim.clone();
        assert!(world.despawn(entity));
        let alternative = world
            .resource_mut::<RouteEntityRegistry>()
            .compare_exchange_lifecycle(&original, EntityLifecycle::Quarantined)
            .unwrap();

        let attempt = ledger.attempt(&world).unwrap();
        assert!(!ledger.settle(&mut world, attempt));

        assert_eq!(ledger.receipt_count(), 0);
        assert!(!ledger.is_settled());
        assert_eq!(
            world
                .resource::<RouteEntityRegistry>()
                .record(entity)
                .unwrap()
                .claim(),
            alternative
        );
    }

    #[test]
    fn stale_living_claim_never_clears_focus_before_settlement_revalidation() {
        for stale_registry_claim in [false, true] {
            let mut world = World::new();
            world.insert_resource(RouteEntityRegistry::new());
            world.insert_resource(QuarantineLeaseTable::new());
            world.insert_resource(RouteFocusState::default());
            let parent = world.spawn_empty().id();
            let entity = registered_entity(&mut world, parent);
            let mut ledger = one_entity_failure_ledger(&mut world, entity, parent);
            let claim = ledger.obligations[0].claim.clone();
            world.insert_resource(InputFocus::from_entity(entity));
            world
                .resource_mut::<RouteFocusState>()
                .set_focused(Some(entity));
            let attempt = ledger.attempt(&world).unwrap();

            if stale_registry_claim {
                world
                    .resource_mut::<RouteEntityRegistry>()
                    .compare_exchange_lifecycle(&claim, EntityLifecycle::Quarantined)
                    .unwrap();
            } else {
                let stale_seal = world.resource_mut::<RouteEntityRegistry>().issue_seal();
                world
                    .entity_mut(entity)
                    .insert(RouteOwnershipSeal::new(stale_seal));
            }

            assert!(!ledger.settle(&mut world, attempt));
            assert_eq!(world.resource::<InputFocus>().get(), Some(entity));
            assert_eq!(world.resource::<RouteFocusState>().focused(), Some(entity));
            assert_eq!(ledger.receipt_count(), 0);
        }
    }

    #[test]
    fn detached_exact_entity_uses_removed_detached() {
        let mut world = World::new();
        world.insert_resource(RouteEntityRegistry::new());
        world.insert_resource(QuarantineLeaseTable::new());
        let expected_parent = world.spawn_empty().id();
        let entity = registered_entity(&mut world, expected_parent);
        let mut ledger = one_entity_failure_ledger(&mut world, entity, expected_parent);
        world.entity_mut(entity).remove::<ChildOf>();

        let attempt = ledger.attempt(&world).unwrap();
        assert!(ledger.settle(&mut world, attempt));

        assert!(world.get_entity(entity).is_err());
        assert_eq!(
            ledger.disposition(0),
            Some(CleanupDisposition::RemovedDetached)
        );
    }

    #[test]
    fn manifest_receipt_and_completion_digests_are_domain_separated() {
        let manifest = StructuredDigest::new(MANIFEST_DOMAIN).finish();
        let attempt = StructuredDigest::new(ATTEMPT_DOMAIN).finish();
        let receipt = StructuredDigest::new(RECEIPT_DOMAIN).finish();
        let completion = StructuredDigest::new(COMPLETION_DOMAIN).finish();
        let values = [manifest, attempt, receipt, completion];
        for (index, value) in values.iter().enumerate() {
            assert!(!values[index + 1..].contains(value));
        }
    }

    #[test]
    fn nested_cleanup_obligation_digest_binds_exact_claim_lifecycle() {
        let mut world = World::new();
        world.insert_resource(RouteEntityRegistry::new());
        world.insert_resource(QuarantineLeaseTable::new());
        let parent = world.spawn_empty().id();
        let entity = registered_entity(&mut world, parent);
        let ledger = one_entity_failure_ledger(&mut world, entity, parent);
        let prepared = ledger.obligations[0].clone();
        let quarantined_claim = world
            .resource::<RouteEntityRegistry>()
            .prepare_lifecycle_mutation(&[(prepared.claim.clone(), EntityLifecycle::Quarantined)])
            .unwrap()
            .resulting_claims()[0]
            .clone();
        let mut quarantined = prepared.clone();
        quarantined.claim = quarantined_claim;

        assert_ne!(
            manifest_digest(&[prepared]),
            manifest_digest(&[quarantined])
        );
    }

    #[test]
    fn reconciliation_superseded_and_foreign_attempts_are_world_zero() {
        let mut world = World::new();
        world.insert_resource(RouteEntityRegistry::new());
        world.insert_resource(QuarantineLeaseTable::new());
        let parent = world.spawn_empty().id();
        let entity = registered_entity(&mut world, parent);
        let mut ledger = one_entity_failure_ledger(&mut world, entity, parent);
        let old = ledger.attempt(&world).unwrap();
        let current = ledger.attempt(&world).unwrap();
        let mut foreign = current.clone();
        foreign.ledger_domain = foreign.ledger_domain.wrapping_add(1);
        let before_rejections = cleanup_rejection_snapshot(&world, &ledger, entity, parent);

        assert!(!ledger.settle(&mut world, old));
        assert_cleanup_rejection_is_zero(&world, &ledger, entity, parent, &before_rejections);
        assert!(!ledger.settle(&mut world, foreign));
        assert_cleanup_rejection_is_zero(&world, &ledger, entity, parent, &before_rejections);
        assert!(world.get_entity(entity).is_ok());
        assert_eq!(ledger.receipt_count(), 0);

        assert!(ledger.settle(&mut world, current));
        assert!(world.get_entity(entity).is_err());
    }

    pub(crate) fn foreign_and_replayed_receipts_are_ledger_zero() {
        let mut world = World::new();
        world.insert_resource(RouteEntityRegistry::new());
        world.insert_resource(QuarantineLeaseTable::new());
        let parent = world.spawn_empty().id();
        let entity = registered_entity(&mut world, parent);
        let mut ledger = one_entity_failure_ledger(&mut world, entity, parent);
        let attempt = ledger.attempt(&world).unwrap();
        let foreign_domain = ledger.ledger_domain.wrapping_add(1);
        let foreign = CleanupReceipt::new(
            foreign_domain,
            attempt.revision,
            0,
            CleanupDisposition::Removed,
            &ledger.obligations[0],
        );
        let unknown_obligation = CleanupReceipt::new(
            ledger.ledger_domain,
            attempt.revision,
            1,
            CleanupDisposition::Removed,
            &ledger.obligations[0],
        );
        let registry_before = world.resource::<RouteEntityRegistry>().records().to_vec();
        let world_before = world
            .iter_entities()
            .map(|entity| (entity.id(), entity.archetype().components().to_vec()))
            .collect::<Vec<_>>();
        let before = cleanup_rejection_snapshot(&world, &ledger, entity, parent);

        assert!(!ledger.accept_receipt(&attempt, foreign));
        assert_cleanup_rejection_is_zero(&world, &ledger, entity, parent, &before);
        assert!(!ledger.accept_receipt(&attempt, unknown_obligation));
        assert_cleanup_rejection_is_zero(&world, &ledger, entity, parent, &before);
        assert_eq!(ledger.receipt_count(), 0);
        assert_eq!(
            world.resource::<RouteEntityRegistry>().records(),
            registry_before
        );
        assert_eq!(
            world
                .iter_entities()
                .map(|entity| (entity.id(), entity.archetype().components().to_vec()))
                .collect::<Vec<_>>(),
            world_before
        );

        assert!(ledger.settle(&mut world, attempt.clone()));
        let replay = ledger.receipts[0].clone().unwrap();
        let receipts_before = ledger.receipts.clone();
        let registry_before = world.resource::<RouteEntityRegistry>().records().to_vec();
        let world_before = world
            .iter_entities()
            .map(|entity| (entity.id(), entity.archetype().components().to_vec()))
            .collect::<Vec<_>>();
        let before = cleanup_rejection_snapshot(&world, &ledger, entity, parent);
        assert!(!ledger.accept_receipt(&attempt, replay));
        assert_cleanup_rejection_is_zero(&world, &ledger, entity, parent, &before);
        assert_eq!(ledger.receipts, receipts_before);
        assert_eq!(ledger.receipt_count(), 1);
        assert_eq!(
            world.resource::<RouteEntityRegistry>().records(),
            registry_before
        );
        assert_eq!(
            world
                .iter_entities()
                .map(|entity| (entity.id(), entity.archetype().components().to_vec()))
                .collect::<Vec<_>>(),
            world_before
        );
    }

    #[test]
    fn reconciliation_foreign_unknown_and_replayed_receipts_are_world_zero() {
        foreign_and_replayed_receipts_are_ledger_zero();
    }

    #[test]
    fn reconciliation_cleanup_receipt_observation_is_exact_once() {
        let mut world = World::new();
        world.insert_resource(RouteEntityRegistry::new());
        world.insert_resource(QuarantineLeaseTable::new());
        let parent = world.spawn_empty().id();
        let entity = registered_entity(&mut world, parent);
        let mut ledger = one_entity_failure_ledger(&mut world, entity, parent);

        let attempt = ledger.attempt(&world).unwrap();
        assert!(ledger.settle(&mut world, attempt));
        let (obligation, disposition) = ledger
            .next_unobserved_receipt()
            .expect("settlement must expose one unobserved receipt");

        assert!(ledger.mark_receipt_observed(obligation, disposition));
        assert!(ledger.next_unobserved_receipt().is_none());
        let before_duplicate = cleanup_rejection_snapshot(&world, &ledger, entity, parent);
        assert!(!ledger.mark_receipt_observed(obligation, disposition));
        assert_cleanup_rejection_is_zero(&world, &ledger, entity, parent, &before_duplicate);
    }

    #[test]
    fn reconciliation_partial_receipt_precedes_aggregate_completion() {
        let mut world = World::new();
        world.insert_resource(RouteEntityRegistry::new());
        world.insert_resource(QuarantineLeaseTable::new());
        let parent = world.spawn_empty().id();
        let entity = registered_entity(&mut world, parent);
        let mut ledger = one_entity_failure_ledger(&mut world, entity, parent);
        ledger.obligations.push(ledger.obligations[0].clone());
        ledger.receipts.push(None);
        ledger.manifest_digest = manifest_digest(&ledger.obligations);
        let attempt = ledger.attempt(&world).unwrap();
        let receipt = CleanupReceipt::new(
            ledger.ledger_domain,
            attempt.revision,
            0,
            CleanupDisposition::Removed,
            &ledger.obligations[0],
        );

        assert!(ledger.accept_receipt(&attempt, receipt));
        ledger.finish_if_settled();
        assert!(!ledger.is_settled());
        assert_eq!(
            ledger.next_unobserved_receipt(),
            Some((0, CleanupDisposition::Removed))
        );
        assert!(ledger.mark_receipt_observed(0, CleanupDisposition::Removed));
        assert!(ledger.next_unobserved_receipt().is_none());
    }

    #[test]
    fn router_cleanup_observation_counts_terminal_quarantine_events_not_receipts() {
        let mut observation = RouterCleanupObservation::default();
        observation.record(CleanupDisposition::Removed, false);
        observation.record(CleanupDisposition::QuarantinedProtectedParent, true);
        observation.record(CleanupDisposition::QuarantinedProtectedParent, false);

        let pending = observation.summary(true);
        assert!(pending.foreground_pending());
        assert_eq!(pending.quarantine_count(), 1);
        assert_eq!(
            pending.last_disposition(),
            Some(bevy_ui_router_core::prelude::CleanupDisposition::QuarantinedProtectedParent)
        );
    }
}
