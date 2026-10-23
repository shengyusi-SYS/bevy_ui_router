//! Post-publication cleanup source proofs and physical settlement.

use super::*;

const POST_COMMIT_PHYSICAL_SOURCE_DOMAIN: u64 = 0x5232_5048_5953_4943;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FrozenCleanupReceiptIdentity {
    ledger_domain: u64,
    attempt_revision: u64,
    obligation: usize,
    disposition: CleanupDisposition,
    binding: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FrozenCleanupTerminalProjection {
    entity: Entity,
    disposition: CleanupDisposition,
    receipt_binding: u64,
    authority: FrozenCleanupTerminalAuthority,
    observed: FrozenPostCommitObservedEntity,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum FrozenCleanupTerminalAuthority {
    Absent,
    Contained {
        lease: crate::registry::ContainedResidualLeaseIdentity,
        record: crate::registry::ContainedResidualRecord,
    },
    Quarantine(ExactRegistryClaim),
    Unresolved(ExactRegistryClaim),
    AuthorityLost {
        id: ResidualLeaseId,
        generation: u64,
        operation_sequence: u64,
        source_identity: CleanupTargetIdentity,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum FrozenOriginalCleanupIndex {
    Unsettled {
        index: usize,
        entity: Entity,
        obligation_binding: u64,
        physical_source_binding: u64,
        disposition: CleanupDisposition,
    },
    Settled {
        index: usize,
        entity: Entity,
        obligation_binding: u64,
        receipt: FrozenCleanupReceiptIdentity,
        terminal: FrozenCleanupTerminalProjection,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FrozenOriginalCleanupLedgerTransfer {
    batch_manifest_digest: u64,
    batch_obligation_count: usize,
    indices: Vec<FrozenOriginalCleanupIndex>,
    immutable_entities: Vec<Entity>,
    ledger_domain: u64,
    next_attempt: u64,
    superseded_attempt_revision: Option<u64>,
    completion_digest: Option<u64>,
    target_observed: Vec<FrozenPostCommitObservedEntity>,
    protected: Vec<FrozenPostCommitObservedEntity>,
    partition_entities: Vec<Entity>,
    roots: Vec<Entity>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PostCommitOriginalFreezePermanentEvidence {
    InvalidLedgerShape,
    InvalidOwner,
    AttemptRevisionExhausted,
    ReceiptMismatch { index: usize, entity: Entity },
    ClassificationMismatch { index: usize, entity: Entity },
    TerminalMismatch { index: usize, entity: Entity },
    DuplicateEntity,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PostCommitOriginalFreezeFailure {
    IneligibleSource,
    OrdinarySettlementRequired,
    RetryableCapacity,
    Permanent(PostCommitOriginalFreezePermanentEvidence),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PostCommitAuthorityFreezePermanentEvidence {
    RequiredRegistryMissing,
    Manifest(AuthoritySettlementManifestError),
    InvalidTargetSet,
    ClassificationMismatch { index: usize, entity: Entity },
    ExactAuthorityMismatch { index: usize, entity: Entity },
    ProtectedClosureMismatch { index: usize, entity: Entity },
    InvalidTopDependency,
    ProtectedTargetOverlap,
    InvalidUnionCoverage,
    Reserve(CleanupLedgerReserveErrorKind),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PostCommitAuthorityFreezeFailure {
    RetryableCapacity,
    Permanent(PostCommitAuthorityFreezePermanentEvidence),
}

impl FrozenOriginalCleanupLedgerTransfer {
    pub(crate) const fn batch_manifest_digest(&self) -> u64 {
        self.batch_manifest_digest
    }

    pub(crate) const fn batch_obligation_count(&self) -> usize {
        self.batch_obligation_count
    }

    pub(crate) fn indices(&self) -> &[FrozenOriginalCleanupIndex] {
        &self.indices
    }

    pub(crate) fn immutable_entities(&self) -> &[Entity] {
        &self.immutable_entities
    }

    pub(crate) fn partition_entities(&self) -> &[Entity] {
        &self.partition_entities
    }

    fn removal_is_expected(&self, entity: Entity) -> bool {
        self.indices.iter().any(|frozen| {
            frozen.entity() == entity
                && matches!(
                    frozen,
                    FrozenOriginalCleanupIndex::Unsettled {
                        disposition: CleanupDisposition::Removed
                            | CleanupDisposition::RemovedDetached
                            | CleanupDisposition::AlreadyAbsent,
                        ..
                    } | FrozenOriginalCleanupIndex::Settled {
                        terminal: FrozenCleanupTerminalProjection {
                            disposition: CleanupDisposition::Removed
                                | CleanupDisposition::RemovedDetached
                                | CleanupDisposition::AlreadyAbsent,
                            ..
                        },
                        ..
                    }
                )
        })
    }

    fn compound_quarantine_disposition(&self) -> Option<CleanupDisposition> {
        let settled_dispositions = self.indices.iter().filter_map(|frozen| match frozen {
            FrozenOriginalCleanupIndex::Settled { terminal, .. } => Some(terminal.disposition),
            FrozenOriginalCleanupIndex::Unsettled { .. } => None,
        });
        let mut has_foreign_descendant = false;
        let mut has_foreign_component = false;
        for disposition in settled_dispositions {
            match disposition {
                CleanupDisposition::QuarantinedProtectedParent => return Some(disposition),
                CleanupDisposition::QuarantinedForeignDescendant => {
                    has_foreign_descendant = true;
                }
                CleanupDisposition::QuarantinedForeignComponent => {
                    has_foreign_component = true;
                }
                CleanupDisposition::Removed
                | CleanupDisposition::RemovedDetached
                | CleanupDisposition::AlreadyAbsent
                | CleanupDisposition::RejectedProtected => {}
            }
        }
        has_foreign_descendant
            .then_some(CleanupDisposition::QuarantinedForeignDescendant)
            .or_else(|| {
                has_foreign_component.then_some(CleanupDisposition::QuarantinedForeignComponent)
            })
    }

    pub(crate) const fn ledger_domain(&self) -> u64 {
        self.ledger_domain
    }

    pub(crate) const fn next_attempt(&self) -> u64 {
        self.next_attempt
    }

    pub(crate) const fn superseded_attempt_revision(&self) -> Option<u64> {
        self.superseded_attempt_revision
    }

    pub(crate) const fn completion_digest(&self) -> Option<u64> {
        self.completion_digest
    }

    pub(crate) fn refreeze(
        &self,
        world: &World,
        ledger: &CleanupLedger,
        in_flight_absent: Option<Entity>,
    ) -> bool {
        let Some(registry) = world.get_resource::<RouteEntityRegistry>() else {
            return false;
        };
        if ledger.ledger_domain != self.ledger_domain
            || ledger.manifest_digest != self.batch_manifest_digest
            || ledger.obligations.len() != self.batch_obligation_count
            || ledger.receipts.len() != self.indices.len()
        {
            return false;
        }

        let mut removed = HashSet::new();
        if let Some(entity) = in_flight_absent {
            removed.insert(entity);
        }
        for frozen in &self.indices {
            let Some(receipt) = ledger.receipts.get(frozen.index()).and_then(Option::as_ref) else {
                continue;
            };
            if matches!(
                receipt.disposition,
                CleanupDisposition::Removed
                    | CleanupDisposition::RemovedDetached
                    | CleanupDisposition::AlreadyAbsent
            ) {
                removed.insert(frozen.entity());
            }
        }

        let mut expected_present = self
            .target_observed
            .iter()
            .filter(|expected| !removed.contains(&expected.entity) && expected.present)
            .map(|expected| expected.entity)
            .collect::<Vec<_>>();
        expected_present.extend(self.indices.iter().filter_map(|frozen| match frozen {
            FrozenOriginalCleanupIndex::Settled { terminal, .. } if terminal.observed.present => {
                Some(terminal.entity)
            }
            _ => None,
        }));
        expected_present.extend(
            self.protected
                .iter()
                .filter(|expected| expected.present)
                .map(|expected| expected.entity),
        );
        expected_present.sort_by_key(|entity| entity.to_bits());
        expected_present.dedup();
        let mut observed_closure = observed_descendant_closure(
            world,
            self.roots
                .iter()
                .copied()
                .filter(|entity| world.get_entity(*entity).is_ok()),
        )
        .into_iter()
        .collect::<Vec<_>>();
        observed_closure.sort_by_key(|entity| entity.to_bits());
        if observed_closure != expected_present {
            return false;
        }

        for frozen in &self.indices {
            let Some(obligation) = ledger.obligations.get(frozen.index()) else {
                return false;
            };
            if cleanup_obligation_binding(obligation) != frozen.obligation_binding() {
                return false;
            }
            match frozen {
                FrozenOriginalCleanupIndex::Settled {
                    receipt, terminal, ..
                } => {
                    let Some(current) =
                        ledger.receipts.get(frozen.index()).and_then(Option::as_ref)
                    else {
                        return false;
                    };
                    if current.ledger_domain != receipt.ledger_domain
                        || current.attempt_revision != receipt.attempt_revision
                        || current.obligation != receipt.obligation
                        || current.disposition != receipt.disposition
                        || current.binding != receipt.binding
                        || terminal.disposition != receipt.disposition
                        || terminal.receipt_binding != receipt.binding
                        || !cleanup_terminal_projection_is_current(world, terminal)
                    {
                        return false;
                    }
                }
                FrozenOriginalCleanupIndex::Unsettled {
                    entity,
                    disposition,
                    ..
                } => {
                    let expected = self
                        .target_observed
                        .iter()
                        .find(|expected| expected.entity == *entity);
                    if in_flight_absent == Some(*entity) {
                        let Some(expected) = expected else {
                            return false;
                        };
                        let observed =
                            FrozenPostCommitObservedEntity::observe(world, &registry, *entity);
                        if observed.present
                            || observed.registry_claim != expected.registry_claim
                            || observed.registry_manifest != expected.registry_manifest
                        {
                            return false;
                        }
                        continue;
                    }
                    if let Some(receipt) =
                        ledger.receipts.get(frozen.index()).and_then(Option::as_ref)
                    {
                        if receipt.ledger_domain != self.ledger_domain
                            || receipt.obligation != frozen.index()
                            || receipt.disposition != *disposition
                            || receipt.binding
                                != receipt_binding(
                                    receipt.ledger_domain,
                                    receipt.attempt_revision,
                                    receipt.obligation,
                                    receipt.disposition,
                                    obligation,
                                )
                            || !matches!(
                                disposition,
                                CleanupDisposition::Removed
                                    | CleanupDisposition::RemovedDetached
                                    | CleanupDisposition::AlreadyAbsent
                            )
                            || world.get_entity(*entity).is_ok()
                            || registry.record(*entity).is_some()
                        {
                            return false;
                        }
                    } else if expected.is_none_or(|expected| {
                        !expected.matches_with_removed_relationships(world, &registry, &removed)
                    }) {
                        return false;
                    }
                }
            }
        }
        self.protected
            .iter()
            .all(|expected| expected.matches_with_removed_relationships(world, &registry, &removed))
    }

    pub(crate) fn terminals_are_current(&self, world: &World, ledger: &CleanupLedger) -> bool {
        self.refreeze(world, ledger, None)
            && self.indices.iter().all(|frozen| {
                ledger
                    .receipts
                    .get(frozen.index())
                    .and_then(Option::as_ref)
                    .is_some()
            })
    }

    pub(crate) fn removed_entities_for_authority_refreeze(
        &self,
        world: &World,
        ledger: &CleanupLedger,
        in_flight_absent: Option<Entity>,
    ) -> Option<HashSet<Entity>> {
        if in_flight_absent.is_some_and(|entity| {
            !self.indices.iter().any(|frozen| {
                frozen.entity() == entity
                    && matches!(frozen, FrozenOriginalCleanupIndex::Unsettled { .. })
                    && ledger
                        .receipts
                        .get(frozen.index())
                        .is_some_and(Option::is_none)
            })
        }) || !self.refreeze(world, ledger, in_flight_absent)
        {
            return None;
        }

        let mut removed = HashSet::new();
        if let Some(entity) = in_flight_absent {
            removed.insert(entity);
        }
        for frozen in &self.indices {
            let Some(receipt) = ledger.receipts.get(frozen.index()).and_then(Option::as_ref) else {
                continue;
            };
            if matches!(
                receipt.disposition,
                CleanupDisposition::Removed
                    | CleanupDisposition::RemovedDetached
                    | CleanupDisposition::AlreadyAbsent
            ) {
                removed.insert(frozen.entity());
            }
        }
        Some(removed)
    }
}

impl FrozenCleanupReceiptIdentity {
    pub(crate) const fn attempt_revision(&self) -> u64 {
        self.attempt_revision
    }

    pub(crate) const fn obligation(&self) -> usize {
        self.obligation
    }

    pub(crate) const fn disposition(&self) -> CleanupDisposition {
        self.disposition
    }

    pub(crate) const fn binding(&self) -> u64 {
        self.binding
    }
}

impl FrozenOriginalCleanupIndex {
    pub(crate) const fn index(&self) -> usize {
        match self {
            Self::Unsettled { index, .. } | Self::Settled { index, .. } => *index,
        }
    }

    pub(crate) const fn entity(&self) -> Entity {
        match self {
            Self::Unsettled { entity, .. } | Self::Settled { entity, .. } => *entity,
        }
    }

    pub(crate) const fn obligation_binding(&self) -> u64 {
        match self {
            Self::Unsettled {
                obligation_binding, ..
            }
            | Self::Settled {
                obligation_binding, ..
            } => *obligation_binding,
        }
    }

    pub(crate) const fn physical_source_binding(&self) -> u64 {
        match self {
            Self::Unsettled {
                physical_source_binding,
                ..
            } => *physical_source_binding,
            Self::Settled { terminal, .. } => terminal.receipt_binding,
        }
    }

    pub(crate) const fn disposition(&self) -> CleanupDisposition {
        match self {
            Self::Unsettled { disposition, .. } => *disposition,
            Self::Settled { receipt, .. } => receipt.disposition,
        }
    }

    pub(crate) const fn settled_receipt(&self) -> Option<&FrozenCleanupReceiptIdentity> {
        match self {
            Self::Settled { receipt, .. } => Some(receipt),
            Self::Unsettled { .. } => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FrozenPostCommitObservedEntity {
    entity: Entity,
    present: bool,
    parent: Option<Entity>,
    children: Vec<Entity>,
    registry_claim: Option<ExactRegistryClaim>,
    registry_manifest: Option<ComponentManifest>,
    ownership_seal: Option<RouteOwnershipSeal>,
    non_relationship_manifest: Option<ComponentManifest>,
    visibility: Option<Visibility>,
    pickable: Option<(bool, bool)>,
    barrier: Option<RouterInputBarrier>,
    frame_lifecycle: Option<FrameLifecycle>,
    child_slot_lifecycle: Option<ChildSlotLifecycle>,
    child_slot_authority: Option<ChildSlotAuthority>,
    provisional_slot_lifecycle: Option<ProvisionalSlotLifecycle>,
    host_registration: Option<(RouterHandle, u64)>,
}

impl FrozenPostCommitObservedEntity {
    fn observe(world: &World, registry: &RouteEntityRegistry, entity: Entity) -> Self {
        let child_of = world.components().component_id::<ChildOf>();
        let children = world.components().component_id::<Children>();
        let non_relationship_manifest =
            ComponentManifest::from_world(world, entity).map(|manifest| {
                ComponentManifest::new(
                    manifest
                        .component_ids()
                        .iter()
                        .copied()
                        .filter(|component| Some(*component) != child_of)
                        .filter(|component| Some(*component) != children),
                )
            });
        Self {
            entity,
            present: world.get_entity(entity).is_ok(),
            parent: world.get::<ChildOf>(entity).map(ChildOf::parent),
            children: observed_children(world, entity),
            registry_claim: registry.record(entity).map(|record| record.claim()),
            registry_manifest: registry
                .record(entity)
                .map(|record| record.manifest().clone()),
            ownership_seal: world.get::<RouteOwnershipSeal>(entity).copied(),
            non_relationship_manifest,
            visibility: world.get::<Visibility>(entity).copied(),
            pickable: world
                .get::<Pickable>(entity)
                .map(|pickable| (pickable.should_block_lower, pickable.is_hoverable)),
            barrier: world.get::<RouterInputBarrier>(entity).copied(),
            frame_lifecycle: world.get::<FrameLifecycle>(entity).copied(),
            child_slot_lifecycle: world.get::<ChildSlotLifecycle>(entity).copied(),
            child_slot_authority: world.get::<ChildSlotAuthority>(entity).cloned(),
            provisional_slot_lifecycle: world.get::<ProvisionalSlotLifecycle>(entity).copied(),
            host_registration: world
                .get::<RouterHostRegistration>(entity)
                .map(|registration| (registration.router(), registration.incarnation())),
        }
    }

    fn matches_exact(&self, world: &World, registry: &RouteEntityRegistry) -> bool {
        Self::observe(world, registry, self.entity) == *self
    }

    fn matches_with_detached_parent(
        &self,
        world: &World,
        registry: &RouteEntityRegistry,
        former_parent: Entity,
    ) -> bool {
        if self.parent != Some(former_parent) {
            return false;
        }
        let mut expected = self.clone();
        expected.parent = None;
        Self::observe(world, registry, self.entity) == expected
    }

    fn matches_with_removed_relationships(
        &self,
        world: &World,
        registry: &RouteEntityRegistry,
        removed: &HashSet<Entity>,
    ) -> bool {
        let observed = Self::observe(world, registry, self.entity);
        let mut expected_children = self.children.clone();
        expected_children.retain(|child| !removed.contains(child));
        let expected_parent = self.parent.filter(|parent| !removed.contains(parent));
        observed.present == self.present
            && observed.parent == expected_parent
            && observed.children == expected_children
            && observed.registry_claim == self.registry_claim
            && observed.registry_manifest == self.registry_manifest
            && observed.ownership_seal == self.ownership_seal
            && observed.non_relationship_manifest == self.non_relationship_manifest
            && observed.visibility == self.visibility
            && observed.pickable == self.pickable
            && observed.barrier == self.barrier
            && observed.frame_lifecycle == self.frame_lifecycle
            && observed.child_slot_lifecycle == self.child_slot_lifecycle
            && observed.child_slot_authority == self.child_slot_authority
            && observed.provisional_slot_lifecycle == self.provisional_slot_lifecycle
            && observed.host_registration == self.host_registration
    }

    fn matches_allowing_input_isolation(
        &self,
        world: &World,
        registry: &RouteEntityRegistry,
        removed: &HashSet<Entity>,
    ) -> bool {
        let observed = Self::observe(world, registry, self.entity);
        let mut expected_children = self.children.clone();
        expected_children.retain(|child| !removed.contains(child));
        let expected_parent = self.parent.filter(|parent| !removed.contains(parent));
        let isolated_pickable = Some((
            Pickable::IGNORE.should_block_lower,
            Pickable::IGNORE.is_hoverable,
        ));
        observed.present == self.present
            && observed.parent == expected_parent
            && observed.children == expected_children
            && observed.registry_claim == self.registry_claim
            && observed.registry_manifest == self.registry_manifest
            && observed.ownership_seal == self.ownership_seal
            && observed.non_relationship_manifest == self.non_relationship_manifest
            && matches!(
                observed.visibility,
                value if value == self.visibility || value == Some(Visibility::Hidden)
            )
            && (observed.pickable == self.pickable || observed.pickable == isolated_pickable)
            && (observed.barrier == self.barrier
                || observed.barrier == Some(RouterInputBarrier::deny_all()))
            && observed.frame_lifecycle == self.frame_lifecycle
            && observed.child_slot_lifecycle == self.child_slot_lifecycle
            && observed.child_slot_authority == self.child_slot_authority
            && observed.provisional_slot_lifecycle == self.provisional_slot_lifecycle
            && observed.host_registration == self.host_registration
    }
}

fn freeze_cleanup_terminal_projection(
    world: &World,
    obligation: &CleanupObligation,
    disposition: CleanupDisposition,
    receipt_binding: u64,
) -> Option<FrozenCleanupTerminalProjection> {
    let registry = world.get_resource::<RouteEntityRegistry>()?;
    let table = world.get_resource::<QuarantineLeaseTable>()?;
    let entity = obligation.entity();
    let observed = FrozenPostCommitObservedEntity::observe(world, &registry, entity);
    let authority = match disposition {
        CleanupDisposition::Removed
        | CleanupDisposition::RemovedDetached
        | CleanupDisposition::AlreadyAbsent => {
            if observed.present || observed.registry_claim.is_some() {
                return None;
            }
            FrozenCleanupTerminalAuthority::Absent
        }
        CleanupDisposition::QuarantinedForeignDescendant
        | CleanupDisposition::QuarantinedForeignComponent
        | CleanupDisposition::QuarantinedProtectedParent => {
            if let Some(record) = table.record(entity) {
                FrozenCleanupTerminalAuthority::Quarantine(record.claim())
            } else if let Some(record) = table.unresolved_record(entity) {
                FrozenCleanupTerminalAuthority::Unresolved(record.claim())
            } else if let Some((lease, record)) = table.contained_record(entity) {
                if record.fact.identity != obligation.identity
                    || record.fact.receipt_binding != receipt_binding
                {
                    return None;
                }
                FrozenCleanupTerminalAuthority::Contained {
                    lease,
                    record: record.clone(),
                }
            } else {
                return None;
            }
        }
        CleanupDisposition::RejectedProtected => {
            let tombstone = table.authority_lost_tombstone(entity)?;
            FrozenCleanupTerminalAuthority::AuthorityLost {
                id: tombstone.id(),
                generation: tombstone.generation(),
                operation_sequence: tombstone.operation_sequence(),
                source_identity: tombstone.source_identity().clone(),
            }
        }
    };
    Some(FrozenCleanupTerminalProjection {
        entity,
        disposition,
        receipt_binding,
        authority,
        observed,
    })
}

fn cleanup_terminal_projection_is_current(
    world: &World,
    terminal: &FrozenCleanupTerminalProjection,
) -> bool {
    let Some(registry) = world.get_resource::<RouteEntityRegistry>() else {
        return false;
    };
    if !terminal.observed.matches_exact(world, &registry) {
        return false;
    }
    cleanup_terminal_authority_is_current(world, &registry, terminal)
}

fn cleanup_terminal_authority_is_current(
    world: &World,
    registry: &RouteEntityRegistry,
    terminal: &FrozenCleanupTerminalProjection,
) -> bool {
    let Some(table) = world.get_resource::<QuarantineLeaseTable>() else {
        return false;
    };
    match &terminal.authority {
        FrozenCleanupTerminalAuthority::Absent => {
            world.get_entity(terminal.entity).is_err() && registry.record(terminal.entity).is_none()
        }
        FrozenCleanupTerminalAuthority::Contained { lease, record } => table
            .contained_record(terminal.entity)
            .is_some_and(|(current_lease, current)| current_lease == *lease && current == record),
        FrozenCleanupTerminalAuthority::Quarantine(claim) => table
            .record(terminal.entity)
            .is_some_and(|record| record.claim() == *claim),
        FrozenCleanupTerminalAuthority::Unresolved(claim) => table
            .unresolved_record(terminal.entity)
            .is_some_and(|record| record.claim() == *claim),
        FrozenCleanupTerminalAuthority::AuthorityLost {
            id,
            generation,
            operation_sequence,
            source_identity,
        } => table
            .authority_lost_tombstone(terminal.entity)
            .is_some_and(|tombstone| {
                tombstone.id() == *id
                    && tombstone.generation() == *generation
                    && tombstone.operation_sequence() == *operation_sequence
                    && tombstone.source_identity() == source_identity
            }),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FrozenPostCommitAuthorityTarget {
    index: usize,
    entity: Entity,
    identity: CleanupTargetIdentity,
    obligation_binding: u64,
    physical_source_binding: u64,
    disposition: CleanupDisposition,
    top_slot_dependency: bool,
    terminal: Option<FrozenCleanupTerminalProjection>,
}

impl FrozenPostCommitAuthorityTarget {
    pub(crate) const fn index(&self) -> usize {
        self.index
    }

    pub(crate) const fn entity(&self) -> Entity {
        self.entity
    }

    pub(crate) const fn obligation_binding(&self) -> u64 {
        self.obligation_binding
    }

    pub(crate) const fn physical_source_binding(&self) -> u64 {
        self.physical_source_binding
    }

    pub(crate) const fn disposition(&self) -> CleanupDisposition {
        self.disposition
    }

    pub(crate) const fn is_top_slot(&self) -> bool {
        self.top_slot_dependency
    }

    pub(crate) fn registered_claim(&self) -> Option<&ExactRegistryClaim> {
        self.identity.registered()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FrozenPostCommitAuthorityPartition {
    targets: Vec<FrozenPostCommitAuthorityTarget>,
    target_observed: Vec<FrozenPostCommitObservedEntity>,
    protected_non_target_current: Vec<FrozenPostCommitObservedEntity>,
    protected_preserved: Vec<FrozenPostCommitObservedEntity>,
    protected_app_foreign: Vec<FrozenPostCommitObservedEntity>,
    protected_other_holders: Vec<FrozenPostCommitObservedEntity>,
    original_entities: Vec<Entity>,
    union_coverage: Vec<Entity>,
    roots: Vec<Entity>,
}

impl FrozenPostCommitAuthorityPartition {
    pub(crate) fn non_top_targets(&self) -> impl Iterator<Item = &FrozenPostCommitAuthorityTarget> {
        self.targets.iter().filter(|target| !target.is_top_slot())
    }

    pub(crate) fn top_target(&self) -> Option<&FrozenPostCommitAuthorityTarget> {
        self.targets.iter().find(|target| target.is_top_slot())
    }

    pub(crate) fn targets(&self) -> &[FrozenPostCommitAuthorityTarget] {
        &self.targets
    }

    pub(crate) fn target(&self, index: usize) -> Option<&FrozenPostCommitAuthorityTarget> {
        self.targets.iter().find(|target| target.index == index)
    }

    fn requires_bulk_quarantine(&self) -> bool {
        self.targets
            .iter()
            .any(|target| target.disposition.is_quarantine())
    }

    fn freeze_bulk_quarantine_terminals(&mut self, world: &World, ledger: &CleanupLedger) -> bool {
        if !self.requires_bulk_quarantine()
            || ledger.receipts.len() != self.targets.len()
            || ledger.completion_digest.is_none()
        {
            return false;
        }
        let mut terminals = Vec::new();
        if terminals.try_reserve_exact(self.targets.len()).is_err() {
            return false;
        }
        for target in &self.targets {
            let Some(obligation) = ledger.obligations.get(target.index) else {
                return false;
            };
            let Some(receipt) = ledger.receipts.get(target.index).and_then(Option::as_ref) else {
                return false;
            };
            if receipt.observed
                || obligation.identity != target.identity
                || cleanup_obligation_binding(obligation) != target.obligation_binding
                || receipt.ledger_domain != ledger.ledger_domain
                || receipt.obligation != target.index
                || receipt.disposition != target.disposition
                || receipt.binding
                    != receipt_binding(
                        receipt.ledger_domain,
                        receipt.attempt_revision,
                        target.index,
                        receipt.disposition,
                        obligation,
                    )
            {
                return false;
            }
            let Some(terminal) = freeze_cleanup_terminal_projection(
                world,
                obligation,
                receipt.disposition,
                receipt.binding,
            ) else {
                return false;
            };
            terminals.push((target.entity, terminal));
        }
        for target in &mut self.targets {
            let Some((_, terminal)) = terminals
                .iter()
                .find(|(entity, _)| *entity == target.entity)
            else {
                return false;
            };
            target.terminal = Some(terminal.clone());
        }
        for observed in &mut self.target_observed {
            let Some((_, terminal)) = terminals
                .iter()
                .find(|(entity, _)| *entity == observed.entity)
            else {
                return false;
            };
            *observed = terminal.observed.clone();
        }
        true
    }
}

pub(crate) struct PreparedPostCommitAuthorityLedger {
    reserved: PreparedCleanupLedger,
    partition: FrozenPostCommitAuthorityPartition,
    manifest_digest: u64,
    obligation_count: usize,
}

pub(crate) struct PreparedAuthorityLedgerBinding {
    prepared: PreparedPostCommitAuthorityLedger,
    batch: PostCommitCleanupBatchView,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PreparedAuthorityLedgerBindingMismatch {
    WrongBatch,
}

pub(crate) struct PostCommitPhysicalSettlementProof {
    entity: Option<Entity>,
    disposition: Option<CleanupDisposition>,
    binding: u64,
}

impl PostCommitPhysicalSettlementProof {
    fn from_exact_receipt(entity: Entity, disposition: CleanupDisposition, binding: u64) -> Self {
        Self {
            entity: Some(entity),
            disposition: Some(disposition),
            binding,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PostCommitAuthoritySettlementFailure {
    PrewriteDrift,
    PhysicalWriteRejected,
    PostwriteDrift,
    ReceiptCommitRejected,
}

impl CleanupLedger {
    pub(crate) fn post_commit_transfer_requires_ordinary_settlement(&self, world: &World) -> bool {
        self.completion_digest.is_none()
            && quarantine_disposition(
                &collect_evidence(world, &self.obligations, &self.receipts),
                &self.obligations,
            )
            .is_some()
    }

    pub(crate) fn preflight_post_commit_transfer_source_kind(
        &self,
    ) -> Result<(), PostCommitOriginalFreezeFailure> {
        if !matches!(self.owner, Some(CleanupOwner::Leaving(_)) | None)
            || self.back_no_target_batch.is_some()
            || self.back_no_target_capacity.is_some()
            || self.authority_host_children.is_some()
            || self.back_no_target_authority_partition.is_some()
            || self.failure_cleanup_partition.is_some()
            || self.stopped.is_some()
        {
            return Err(PostCommitOriginalFreezeFailure::IneligibleSource);
        }
        Ok(())
    }

    /// Freezes the complete original cleanup owner before it crosses into the
    /// post-commit compound. Existing receipts must already have gone through
    /// the sole legacy Core consumer; this boundary never replays them.
    pub(crate) fn freeze_for_post_commit_transfer(
        &mut self,
        world: &World,
    ) -> Result<FrozenOriginalCleanupLedgerTransfer, PostCommitOriginalFreezeFailure> {
        self.preflight_post_commit_transfer_source_kind()?;
        let batch = self.batch.as_ref().ok_or_else(|| {
            PostCommitOriginalFreezeFailure::Permanent(
                PostCommitOriginalFreezePermanentEvidence::InvalidLedgerShape,
            )
        })?;
        if batch.manifest_digest() != self.manifest_digest
            || batch.obligation_count() != self.obligations.len()
            || self.receipts.len() != self.obligations.len()
            || self.mount_parents.len() > self.obligations.len()
        {
            return Err(PostCommitOriginalFreezeFailure::Permanent(
                PostCommitOriginalFreezePermanentEvidence::InvalidLedgerShape,
            ));
        }
        if !self.obligations.is_empty() && !matches!(self.owner, Some(CleanupOwner::Leaving(_))) {
            return Err(PostCommitOriginalFreezeFailure::Permanent(
                PostCommitOriginalFreezePermanentEvidence::InvalidOwner,
            ));
        }
        if self.obligations.is_empty() && self.owner.is_some() {
            return Err(PostCommitOriginalFreezeFailure::Permanent(
                PostCommitOriginalFreezePermanentEvidence::InvalidOwner,
            ));
        }

        let superseded_attempt_revision =
            self.issued_attempt.as_ref().map(|attempt| attempt.revision);
        let next_attempt = match superseded_attempt_revision {
            Some(revision) if self.next_attempt <= revision => {
                revision.checked_add(1).ok_or_else(|| {
                    PostCommitOriginalFreezeFailure::Permanent(
                        PostCommitOriginalFreezePermanentEvidence::AttemptRevisionExhausted,
                    )
                })?
            }
            _ => self.next_attempt,
        };

        let mut indices = Vec::new();
        indices
            .try_reserve_exact(self.obligations.len())
            .map_err(|_| PostCommitOriginalFreezeFailure::RetryableCapacity)?;
        let mut immutable_entities = Vec::new();
        immutable_entities
            .try_reserve_exact(self.obligations.len())
            .map_err(|_| PostCommitOriginalFreezeFailure::RetryableCapacity)?;
        let current_evidence = collect_evidence(world, &self.obligations, &self.receipts);
        if quarantine_disposition(&current_evidence, &self.obligations).is_some() {
            return Err(PostCommitOriginalFreezeFailure::OrdinarySettlementRequired);
        }
        for (index, obligation) in self.obligations.iter().enumerate() {
            let entity = obligation.entity();
            immutable_entities.push(entity);
            let obligation_binding = cleanup_obligation_binding(obligation);
            match self.receipts[index].as_ref() {
                Some(receipt) => {
                    if !receipt.observed
                        || receipt.ledger_domain != self.ledger_domain
                        || receipt.obligation != index
                        || receipt.binding
                            != receipt_binding(
                                receipt.ledger_domain,
                                receipt.attempt_revision,
                                index,
                                receipt.disposition,
                                obligation,
                            )
                    {
                        return Err(PostCommitOriginalFreezeFailure::Permanent(
                            PostCommitOriginalFreezePermanentEvidence::ReceiptMismatch {
                                index,
                                entity,
                            },
                        ));
                    }
                    let terminal = freeze_cleanup_terminal_projection(
                        world,
                        obligation,
                        receipt.disposition,
                        receipt.binding,
                    )
                    .ok_or_else(|| {
                        PostCommitOriginalFreezeFailure::Permanent(
                            PostCommitOriginalFreezePermanentEvidence::TerminalMismatch {
                                index,
                                entity,
                            },
                        )
                    })?;
                    indices.push(FrozenOriginalCleanupIndex::Settled {
                        index,
                        entity,
                        obligation_binding,
                        receipt: FrozenCleanupReceiptIdentity {
                            ledger_domain: receipt.ledger_domain,
                            attempt_revision: receipt.attempt_revision,
                            obligation: receipt.obligation,
                            disposition: receipt.disposition,
                            binding: receipt.binding,
                        },
                        terminal,
                    });
                }
                None => {
                    let evidence = current_evidence
                        .iter()
                        .find_map(|(candidate, evidence)| {
                            (*candidate == index).then_some(evidence.clone())
                        })
                        .ok_or_else(|| {
                            PostCommitOriginalFreezeFailure::Permanent(
                                PostCommitOriginalFreezePermanentEvidence::ClassificationMismatch {
                                    index,
                                    entity,
                                },
                            )
                        })?;
                    let disposition =
                        post_commit_original_disposition(&evidence, obligation.expected_parent)
                            .ok_or_else(|| {
                                PostCommitOriginalFreezeFailure::Permanent(
                            PostCommitOriginalFreezePermanentEvidence::ClassificationMismatch {
                                index,
                                entity,
                            },
                        )
                            })?;
                    indices.push(FrozenOriginalCleanupIndex::Unsettled {
                        index,
                        entity,
                        obligation_binding,
                        physical_source_binding: post_commit_physical_source_binding(
                            self.ledger_domain,
                            index,
                            obligation,
                            disposition,
                        ),
                        disposition,
                    });
                }
            }
        }
        immutable_entities.sort_by_key(|entity| entity.to_bits());
        immutable_entities.dedup();
        if immutable_entities.len() != self.obligations.len() {
            return Err(PostCommitOriginalFreezeFailure::Permanent(
                PostCommitOriginalFreezePermanentEvidence::DuplicateEntity,
            ));
        }
        let registry = world.get_resource::<RouteEntityRegistry>().ok_or_else(|| {
            PostCommitOriginalFreezeFailure::Permanent(
                PostCommitOriginalFreezePermanentEvidence::InvalidLedgerShape,
            )
        })?;
        let target_entities = indices
            .iter()
            .filter_map(|index| {
                matches!(index, FrozenOriginalCleanupIndex::Unsettled { .. })
                    .then_some(index.entity())
            })
            .collect::<Vec<_>>();
        let target_observed = target_entities
            .iter()
            .copied()
            .map(|entity| FrozenPostCommitObservedEntity::observe(world, &registry, entity))
            .collect::<Vec<_>>();
        let roots = immutable_entities
            .iter()
            .copied()
            .filter(|entity| world.get_entity(*entity).is_ok())
            .collect::<Vec<_>>();
        let mut closure = observed_descendant_closure(world, roots.iter().copied())
            .into_iter()
            .collect::<Vec<_>>();
        closure.sort_by_key(|entity| entity.to_bits());
        let protected = closure
            .iter()
            .copied()
            .filter(|entity| !immutable_entities.contains(entity))
            .map(|entity| FrozenPostCommitObservedEntity::observe(world, &registry, entity))
            .collect::<Vec<_>>();
        let mut partition_entities = Vec::new();
        partition_entities
            .try_reserve_exact(immutable_entities.len() + protected.len())
            .map_err(|_| PostCommitOriginalFreezeFailure::RetryableCapacity)?;
        partition_entities.extend(immutable_entities.iter().copied());
        partition_entities.extend(protected.iter().map(|entity| entity.entity));
        partition_entities.sort_by_key(|entity| entity.to_bits());
        partition_entities.dedup();
        self.issued_attempt = None;
        self.next_attempt = next_attempt;
        Ok(FrozenOriginalCleanupLedgerTransfer {
            batch_manifest_digest: self.manifest_digest,
            batch_obligation_count: self.obligations.len(),
            indices,
            immutable_entities,
            ledger_domain: self.ledger_domain,
            next_attempt,
            superseded_attempt_revision,
            completion_digest: self.completion_digest,
            target_observed,
            protected,
            partition_entities,
            roots,
        })
    }

    pub(crate) fn post_commit_original_receipt_proof(
        &self,
        world: &World,
        frozen: &FrozenOriginalCleanupLedgerTransfer,
        index: usize,
    ) -> Option<PostCommitPhysicalSettlementProof> {
        let FrozenOriginalCleanupIndex::Unsettled {
            entity,
            disposition,
            obligation_binding,
            ..
        } = frozen.indices.iter().find(|entry| entry.index() == index)?
        else {
            return None;
        };
        let obligation = self.obligations.get(index)?;
        let receipt = self.receipts.get(index)?.as_ref()?;
        if receipt.observed
            || receipt.ledger_domain != frozen.ledger_domain
            || receipt.obligation != index
            || receipt.disposition != *disposition
            || cleanup_obligation_binding(obligation) != *obligation_binding
            || receipt.binding
                != receipt_binding(
                    receipt.ledger_domain,
                    receipt.attempt_revision,
                    index,
                    receipt.disposition,
                    obligation,
                )
        {
            return None;
        }
        if !frozen.refreeze(world, self, None) {
            return None;
        }
        Some(PostCommitPhysicalSettlementProof::from_exact_receipt(
            *entity,
            *disposition,
            receipt.binding,
        ))
    }

    pub(crate) fn settle_post_commit_original_target(
        &mut self,
        world: &mut World,
        frozen: &FrozenOriginalCleanupLedgerTransfer,
        index: usize,
        authority_partition: &FrozenPostCommitAuthorityPartition,
        settled_authority: &[Entity],
        input_isolation_is_current: &impl Fn(&World) -> bool,
    ) -> Result<PostCommitPhysicalSettlementProof, PostCommitAuthoritySettlementFailure> {
        let FrozenOriginalCleanupIndex::Unsettled {
            disposition,
            obligation_binding,
            ..
        } = frozen
            .indices
            .iter()
            .find(|entry| entry.index() == index)
            .ok_or(PostCommitAuthoritySettlementFailure::PrewriteDrift)?
        else {
            return Err(PostCommitAuthoritySettlementFailure::PrewriteDrift);
        };
        let original_removed = frozen
            .removed_entities_for_authority_refreeze(world, self, None)
            .ok_or(PostCommitAuthoritySettlementFailure::PrewriteDrift)?;
        if self.receipts.get(index).is_none_or(Option::is_some)
            || self.obligations.get(index).is_none_or(|obligation| {
                cleanup_obligation_binding(obligation) != *obligation_binding
            })
            || !authority_partition.refreeze(
                world,
                settled_authority,
                None,
                false,
                &original_removed,
            )
            || !input_isolation_is_current(world)
        {
            return Err(PostCommitAuthoritySettlementFailure::PrewriteDrift);
        }
        let attempt = self
            .attempt(world)
            .ok_or(PostCommitAuthoritySettlementFailure::PrewriteDrift)?;
        let evidence = attempt
            .evidence
            .iter()
            .find(|(candidate, _)| *candidate == index)
            .map(|(_, evidence)| evidence)
            .ok_or(PostCommitAuthoritySettlementFailure::PrewriteDrift)?;
        let obligation = &self.obligations[index];
        if post_commit_original_disposition(evidence, obligation.expected_parent)
            != Some(*disposition)
        {
            self.issued_attempt = None;
            return Err(PostCommitAuthoritySettlementFailure::PrewriteDrift);
        }
        let receipt_value = CleanupReceipt::new(
            self.ledger_domain,
            attempt.revision,
            index,
            *disposition,
            obligation,
        );
        let receipt = self
            .prepare_receipt_batch(&attempt, vec![receipt_value])
            .ok_or(PostCommitAuthoritySettlementFailure::ReceiptCommitRejected)?;
        let observed_claim = world
            .get_resource::<RouteEntityRegistry>()
            .and_then(|registry| registry.record(obligation.entity()))
            .map(|record| record.claim());
        let retirement = match observed_claim {
            Some(claim) if obligation.identity.registered() == Some(&claim) => Some(
                prepare_exact_registry_retirement(
                    world.resource::<RouteEntityRegistry>(),
                    vec![claim],
                )
                .map_err(|_| PostCommitAuthoritySettlementFailure::PrewriteDrift)?,
            ),
            None => None,
            Some(_) if *disposition == CleanupDisposition::RejectedProtected => None,
            Some(_) => return Err(PostCommitAuthoritySettlementFailure::PrewriteDrift),
        };
        if matches!(
            disposition,
            CleanupDisposition::Removed | CleanupDisposition::RemovedDetached
        ) {
            if !cleanup_obligation_has_exact_authority(world, obligation)
                || !world.despawn(obligation.entity())
            {
                self.issued_attempt = None;
                return Err(PostCommitAuthoritySettlementFailure::PhysicalWriteRejected);
            }
            world.flush();
            let Some(original_removed) = frozen.removed_entities_for_authority_refreeze(
                world,
                self,
                Some(obligation.entity()),
            ) else {
                self.issued_attempt = None;
                return Err(PostCommitAuthoritySettlementFailure::PostwriteDrift);
            };
            if world.get_entity(obligation.entity()).is_ok()
                || world
                    .resource::<RouteEntityRegistry>()
                    .record(obligation.entity())
                    .is_some_and(|record| obligation.identity.registered() != Some(&record.claim()))
                || (obligation.identity.registered().is_some()
                    && world
                        .resource::<RouteEntityRegistry>()
                        .record(obligation.entity())
                        .is_none())
                || !authority_partition.refreeze(
                    world,
                    settled_authority,
                    None,
                    false,
                    &original_removed,
                )
                || !input_isolation_is_current(world)
            {
                self.issued_attempt = None;
                return Err(PostCommitAuthoritySettlementFailure::PostwriteDrift);
            }
        } else if *disposition == CleanupDisposition::AlreadyAbsent
            && world.get_entity(obligation.entity()).is_ok()
        {
            self.issued_attempt = None;
            return Err(PostCommitAuthoritySettlementFailure::PrewriteDrift);
        }
        if let Some(retirement) = retirement {
            apply_prepared_exact_registry_retirement(
                &mut world.resource_mut::<RouteEntityRegistry>(),
                retirement,
            );
            if world
                .resource::<RouteEntityRegistry>()
                .record(obligation.entity())
                .is_some()
            {
                self.issued_attempt = None;
                return Err(PostCommitAuthoritySettlementFailure::PostwriteDrift);
            }
        }
        self.commit_receipt_batch(receipt);
        let Some(original_removed) =
            frozen.removed_entities_for_authority_refreeze(world, self, None)
        else {
            self.issued_attempt = None;
            return Err(PostCommitAuthoritySettlementFailure::PostwriteDrift);
        };
        if !authority_partition.refreeze(world, settled_authority, None, false, &original_removed)
            || !input_isolation_is_current(world)
        {
            self.issued_attempt = None;
            return Err(PostCommitAuthoritySettlementFailure::PostwriteDrift);
        }
        self.finish_if_settled();
        self.post_commit_original_receipt_proof(world, frozen, index)
            .ok_or(PostCommitAuthoritySettlementFailure::ReceiptCommitRejected)
    }

    pub(crate) fn authority_completion_is_exact(&self) -> bool {
        self.batch.is_none()
            && self.back_no_target_batch.is_none()
            && self.back_no_target_capacity.is_none()
            && self.owner.is_none()
            && self.mount_parents.is_empty()
            && self.receipts.iter().all(Option::is_some)
            && self.completion_digest.is_some()
            && self.issued_attempt.is_none()
    }
}

impl PreparedPostCommitAuthorityLedger {
    pub(crate) fn freeze_and_reserve(
        world: &World,
        active: &RealizedBranch,
        host: &HostBinding,
        original: &FrozenOriginalCleanupLedgerTransfer,
        retiring: &[(&RetainedLeafProjection, &crate::instance::InstancePayload)],
    ) -> Result<Self, PostCommitAuthorityFreezeFailure> {
        let original_entities = original.partition_entities();
        let registry = world.get_resource::<RouteEntityRegistry>().ok_or_else(|| {
            PostCommitAuthorityFreezeFailure::Permanent(
                PostCommitAuthorityFreezePermanentEvidence::RequiredRegistryMissing,
            )
        })?;
        let manifest = FrozenCleanupManifest::from_idle_host_authority_with_retained(
            world,
            &registry,
            Some(active),
            host,
            retiring,
        )
        .map_err(|error| match error {
            AuthoritySettlementManifestError::RetryableCapacity => {
                PostCommitAuthorityFreezeFailure::RetryableCapacity
            }
            error => PostCommitAuthorityFreezeFailure::Permanent(
                PostCommitAuthorityFreezePermanentEvidence::Manifest(error),
            ),
        })?;
        let manifest_digest = manifest.digest;
        let obligation_count = manifest.obligations.len();
        let mut target_entities = manifest
            .obligations
            .iter()
            .map(|obligation| obligation.entity())
            .collect::<Vec<_>>();
        target_entities.sort_by_key(|entity| entity.to_bits());
        target_entities.dedup();
        if target_entities.len() != obligation_count
            || !target_entities.contains(&host.top_slot)
            || target_entities
                .iter()
                .any(|entity| original_entities.contains(entity))
        {
            return Err(PostCommitAuthorityFreezeFailure::Permanent(
                PostCommitAuthorityFreezePermanentEvidence::InvalidTargetSet,
            ));
        }

        let mut targets = Vec::new();
        targets
            .try_reserve_exact(obligation_count)
            .map_err(|_| PostCommitAuthorityFreezeFailure::RetryableCapacity)?;
        let empty_receipts = vec![None; obligation_count];
        let evidence = manifest
            .obligations
            .iter()
            .enumerate()
            .map(|(index, _)| {
                (
                    index,
                    classify_obligation(world, &manifest.obligations, &empty_receipts, index),
                )
            })
            .collect::<Vec<_>>();
        let compound_quarantine = quarantine_disposition_excluding_authorized_absence(
            &evidence,
            &manifest.obligations,
            &|entity| original.removal_is_expected(entity),
        )
        .or_else(|| original.compound_quarantine_disposition());
        for (index, obligation) in manifest.obligations.iter().enumerate() {
            let observed = &evidence[index].1;
            let Some(mut disposition) = post_commit_authority_batch_disposition(
                observed,
                obligation.expected_parent,
                compound_quarantine,
            ) else {
                return Err(PostCommitAuthorityFreezeFailure::Permanent(
                    PostCommitAuthorityFreezePermanentEvidence::ClassificationMismatch {
                        index,
                        entity: obligation.entity(),
                    },
                ));
            };
            if obligation.entity() == host.top_slot && disposition == CleanupDisposition::Removed {
                // H is an explicit predecessor of A-top, so the top receipt is
                // frozen against the authorized post-H detached projection.
                disposition = CleanupDisposition::RemovedDetached;
            }
            if world.get_entity(obligation.entity()).is_ok()
                && (!matches!(observed, CleanupEvidence::Exact { .. })
                    || !cleanup_obligation_has_exact_authority(world, obligation))
            {
                return Err(PostCommitAuthorityFreezeFailure::Permanent(
                    PostCommitAuthorityFreezePermanentEvidence::ExactAuthorityMismatch {
                        index,
                        entity: obligation.entity(),
                    },
                ));
            }
            if let CleanupEvidence::Exact {
                foreign_components,
                missing_components,
                protected_parent,
                relationship_cycle,
                ..
            } = observed
                && (!foreign_components.is_empty()
                    || !missing_components.is_empty()
                    || protected_parent.is_some()
                    || *relationship_cycle)
            {
                return Err(PostCommitAuthorityFreezeFailure::Permanent(
                    PostCommitAuthorityFreezePermanentEvidence::ProtectedClosureMismatch {
                        index,
                        entity: obligation.entity(),
                    },
                ));
            }
            targets.push(FrozenPostCommitAuthorityTarget {
                index,
                entity: obligation.entity(),
                identity: obligation.identity.clone(),
                obligation_binding: cleanup_obligation_binding(obligation),
                physical_source_binding: post_commit_physical_source_binding(
                    manifest_digest,
                    index,
                    obligation,
                    disposition,
                ),
                disposition,
                top_slot_dependency: obligation.entity() == host.top_slot,
                terminal: None,
            });
        }
        targets.sort_by_key(|target| {
            (
                target.top_slot_dependency,
                std::cmp::Reverse(authority_observed_depth(world, target.entity)),
                target.entity.to_bits(),
            )
        });
        if targets
            .iter()
            .filter(|target| target.top_slot_dependency)
            .count()
            != 1
        {
            return Err(PostCommitAuthorityFreezeFailure::Permanent(
                PostCommitAuthorityFreezePermanentEvidence::InvalidTopDependency,
            ));
        }

        let mut roots = target_entities.clone();
        roots.push(host.host);
        roots.sort_by_key(|entity| entity.to_bits());
        roots.dedup();
        let mut closure = observed_descendant_closure(world, roots.iter().copied())
            .into_iter()
            .collect::<Vec<_>>();
        closure.sort_by_key(|entity| entity.to_bits());
        closure.dedup();
        let target_observed = target_entities
            .iter()
            .copied()
            .map(|entity| FrozenPostCommitObservedEntity::observe(world, &registry, entity))
            .collect::<Vec<_>>();
        let protected = closure
            .iter()
            .copied()
            .filter(|entity| {
                !target_entities.contains(entity) && !original_entities.contains(entity)
            })
            .map(|entity| FrozenPostCommitObservedEntity::observe(world, &registry, entity))
            .collect::<Vec<_>>();
        if protected
            .iter()
            .any(|entity| target_entities.contains(&entity.entity))
        {
            return Err(PostCommitAuthorityFreezeFailure::Permanent(
                PostCommitAuthorityFreezePermanentEvidence::ProtectedTargetOverlap,
            ));
        }

        let mut original = original_entities.to_vec();
        original.sort_by_key(|entity| entity.to_bits());
        original.dedup();
        let mut union_coverage = original.clone();
        union_coverage.extend(target_entities.iter().copied());
        union_coverage.sort_by_key(|entity| entity.to_bits());
        union_coverage.dedup();
        if union_coverage.len() != original.len() + target_entities.len() {
            return Err(PostCommitAuthorityFreezeFailure::Permanent(
                PostCommitAuthorityFreezePermanentEvidence::InvalidUnionCoverage,
            ));
        }

        let reserved = CleanupLedger::reserve(manifest).map_err(|error| match error.kind() {
            CleanupLedgerReserveErrorKind::RetryableCapacity => {
                PostCommitAuthorityFreezeFailure::RetryableCapacity
            }
            kind => PostCommitAuthorityFreezeFailure::Permanent(
                PostCommitAuthorityFreezePermanentEvidence::Reserve(kind),
            ),
        })?;
        Ok(Self {
            reserved,
            partition: FrozenPostCommitAuthorityPartition {
                targets,
                target_observed,
                protected_non_target_current: Vec::new(),
                protected_preserved: Vec::new(),
                protected_app_foreign: protected,
                protected_other_holders: Vec::new(),
                original_entities: original,
                union_coverage,
                roots,
            },
            manifest_digest,
            obligation_count,
        })
    }

    pub(crate) const fn manifest_digest(&self) -> u64 {
        self.manifest_digest
    }

    pub(crate) const fn obligation_count(&self) -> usize {
        self.obligation_count
    }

    pub(crate) fn partition(&self) -> &FrozenPostCommitAuthorityPartition {
        &self.partition
    }

    pub(crate) fn batch_matches(&self, batch: &PostCommitCleanupBatchView) -> bool {
        batch.manifest_digest() == self.manifest_digest
            && batch.obligation_count() == self.obligation_count
    }

    pub(crate) fn commit_input_isolation(
        &mut self,
        world: &World,
        original_freeze: &FrozenOriginalCleanupLedgerTransfer,
        original: &CleanupLedger,
    ) -> bool {
        let Some(original_removed) =
            original_freeze.removed_entities_for_authority_refreeze(world, original, None)
        else {
            return false;
        };
        if !self
            .partition
            .refreeze_allowing_input_isolation(world, &original_removed)
        {
            return false;
        }
        for expected in &mut self.partition.target_observed {
            #[cfg(feature = "bsn")]
            if crate::presentation::native_frame::is_native_content(world, expected.entity) {
                // Isolation writes the Router shell, preserving Scene-authored
                // content values and their exact frozen observation.
                continue;
            }
            if expected.visibility.is_some() {
                expected.visibility = Some(Visibility::Hidden);
            }
            if expected.pickable.is_some() {
                expected.pickable = Some((
                    Pickable::IGNORE.should_block_lower,
                    Pickable::IGNORE.is_hoverable,
                ));
            }
            if expected.barrier.is_some() {
                expected.barrier = Some(RouterInputBarrier::deny_all());
            }
        }
        self.partition
            .refreeze(world, &[], None, false, &original_removed)
    }

    pub(crate) fn bind_batch(
        self,
        batch: PostCommitCleanupBatchView,
    ) -> Result<PreparedAuthorityLedgerBinding, (Self, PreparedAuthorityLedgerBindingMismatch)>
    {
        if batch.manifest_digest() != self.manifest_digest
            || batch.obligation_count() != self.obligation_count
        {
            return Err((self, PreparedAuthorityLedgerBindingMismatch::WrongBatch));
        }
        Ok(PreparedAuthorityLedgerBinding {
            prepared: self,
            batch,
        })
    }

    pub(crate) fn bind_verified_batch(
        self,
        batch: PostCommitCleanupBatchView,
    ) -> PreparedAuthorityLedgerBinding {
        debug_assert!(self.batch_matches(&batch));
        PreparedAuthorityLedgerBinding {
            prepared: self,
            batch,
        }
    }
}

impl PreparedAuthorityLedgerBinding {
    pub(crate) fn into_parts(self) -> (CleanupLedger, FrozenPostCommitAuthorityPartition) {
        let _authority_batch_identity = self.batch;
        let PreparedPostCommitAuthorityLedger {
            reserved,
            partition,
            ..
        } = self.prepared;
        (CleanupLedger::activate_owner(reserved, None), partition)
    }
}

impl FrozenPostCommitAuthorityPartition {
    fn cleanup_terminal_is_current(
        &self,
        world: &World,
        terminal: &FrozenCleanupTerminalProjection,
        host_edge_detached: bool,
    ) -> bool {
        let Some(top) = self
            .top_target()
            .map(FrozenPostCommitAuthorityTarget::entity)
        else {
            return false;
        };
        if !host_edge_detached || terminal.entity != top {
            return cleanup_terminal_projection_is_current(world, terminal);
        }
        let Some(host) = self
            .roots
            .iter()
            .copied()
            .find(|entity| !self.targets.iter().any(|target| target.entity == *entity))
        else {
            return false;
        };
        let Some(registry) = world.get_resource::<RouteEntityRegistry>() else {
            return false;
        };
        terminal
            .observed
            .matches_with_detached_parent(world, &registry, host)
            && cleanup_terminal_authority_is_current(world, &registry, terminal)
    }

    pub(crate) fn refreeze_allowing_input_isolation(
        &self,
        world: &World,
        original_removed: &HashSet<Entity>,
    ) -> bool {
        let Some(registry) = world.get_resource::<RouteEntityRegistry>() else {
            return false;
        };
        let mut expected_closure = self
            .target_observed
            .iter()
            .chain(&self.protected_non_target_current)
            .chain(&self.protected_preserved)
            .chain(&self.protected_app_foreign)
            .chain(&self.protected_other_holders)
            .filter(|observed| observed.present)
            .map(|observed| observed.entity)
            .collect::<Vec<_>>();
        expected_closure.sort_by_key(|entity| entity.to_bits());
        expected_closure.dedup();
        let mut observed_closure = observed_descendant_closure(
            world,
            self.roots
                .iter()
                .copied()
                .filter(|entity| world.get_entity(*entity).is_ok()),
        )
        .into_iter()
        .filter(|entity| !self.original_entities.contains(entity))
        .collect::<Vec<_>>();
        observed_closure.sort_by_key(|entity| entity.to_bits());
        observed_closure.dedup();
        observed_closure == expected_closure
            && self.target_observed.iter().all(|expected| {
                expected.matches_allowing_input_isolation(world, &registry, original_removed)
            })
            && self
                .protected_non_target_current
                .iter()
                .chain(&self.protected_preserved)
                .chain(&self.protected_app_foreign)
                .chain(&self.protected_other_holders)
                .all(|expected| {
                    expected.matches_with_removed_relationships(world, &registry, original_removed)
                })
    }

    pub(crate) fn refreeze(
        &self,
        world: &World,
        settled: &[Entity],
        in_flight_absent: Option<Entity>,
        host_edge_detached: bool,
        original_removed: &HashSet<Entity>,
    ) -> bool {
        let Some(registry) = world.get_resource::<RouteEntityRegistry>() else {
            return false;
        };
        let removed = |entity: Entity| {
            in_flight_absent == Some(entity)
                || original_removed.contains(&entity)
                || (settled.contains(&entity)
                    && self.targets.iter().any(|target| {
                        target.entity == entity && !target.disposition.is_quarantine()
                    }))
        };
        let mut expected_present = self
            .target_observed
            .iter()
            .filter(|observed| observed.present && !removed(observed.entity))
            .map(|observed| observed.entity)
            .collect::<Vec<_>>();
        expected_present.extend(
            self.protected_non_target_current
                .iter()
                .chain(&self.protected_preserved)
                .chain(&self.protected_app_foreign)
                .chain(&self.protected_other_holders)
                .filter(|observed| observed.present)
                .map(|observed| observed.entity),
        );
        expected_present.sort_by_key(|entity| entity.to_bits());
        expected_present.dedup();
        let mut observed_closure = observed_descendant_closure(
            world,
            self.roots
                .iter()
                .copied()
                .filter(|entity| world.get_entity(*entity).is_ok()),
        )
        .into_iter()
        .filter(|entity| !self.original_entities.contains(entity))
        .collect::<Vec<_>>();
        observed_closure.sort_by_key(|entity| entity.to_bits());
        observed_closure.dedup();
        if observed_closure != expected_present {
            return false;
        }

        let top = self
            .top_target()
            .map(FrozenPostCommitAuthorityTarget::entity);
        let host = self
            .roots
            .iter()
            .copied()
            .find(|entity| !self.targets.iter().any(|target| target.entity == *entity));
        let observed_matches = |expected: &FrozenPostCommitObservedEntity, target: bool| {
            let observed =
                FrozenPostCommitObservedEntity::observe(world, &registry, expected.entity);
            if target
                && let Some(terminal) = self
                    .targets
                    .iter()
                    .find(|candidate| candidate.entity == expected.entity)
                    .and_then(|candidate| candidate.terminal.as_ref())
            {
                return self.cleanup_terminal_is_current(world, terminal, host_edge_detached);
            }
            if in_flight_absent == Some(expected.entity) {
                return !observed.present
                    && observed.registry_claim == expected.registry_claim
                    && observed.registry_manifest == expected.registry_manifest;
            }
            if settled.contains(&expected.entity) {
                return !observed.present
                    && observed.registry_claim.is_none()
                    && observed.registry_manifest.is_none();
            }
            let mut expected_children = expected.children.clone();
            expected_children.retain(|child| !removed(*child));
            if host_edge_detached && Some(expected.entity) == host {
                expected_children.retain(|child| Some(*child) != top);
            }
            let expected_parent = if host_edge_detached && Some(expected.entity) == top {
                None
            } else {
                expected.parent.filter(|parent| !removed(*parent))
            };
            observed.present == expected.present
                && observed.parent == expected_parent
                && observed.children == expected_children
                && observed.registry_claim == expected.registry_claim
                && observed.registry_manifest == expected.registry_manifest
                && observed.ownership_seal == expected.ownership_seal
                && observed.non_relationship_manifest == expected.non_relationship_manifest
                && observed.visibility == expected.visibility
                && observed.pickable == expected.pickable
                && observed.barrier == expected.barrier
                && observed.frame_lifecycle == expected.frame_lifecycle
                && observed.child_slot_lifecycle == expected.child_slot_lifecycle
                && observed.child_slot_authority == expected.child_slot_authority
                && observed.provisional_slot_lifecycle == expected.provisional_slot_lifecycle
                && observed.host_registration == expected.host_registration
                && (!target
                    || self.targets.iter().any(|candidate| {
                        candidate.entity == expected.entity
                            && expected
                                .registry_claim
                                .as_ref()
                                .is_none_or(|claim| candidate.identity.registered() == Some(claim))
                    }))
        };
        self.target_observed
            .iter()
            .all(|expected| observed_matches(expected, true))
            && self
                .protected_non_target_current
                .iter()
                .chain(&self.protected_preserved)
                .chain(&self.protected_app_foreign)
                .chain(&self.protected_other_holders)
                .all(|expected| observed_matches(expected, false))
            && self
                .original_entities
                .iter()
                .all(|entity| !self.targets.iter().any(|target| target.entity == *entity))
            && self.union_coverage.len() == self.original_entities.len() + self.targets.len()
    }
}

impl CleanupLedger {
    pub(crate) fn settle_post_commit_authority_target(
        &mut self,
        world: &mut World,
        partition: &mut FrozenPostCommitAuthorityPartition,
        original: &CleanupLedger,
        original_freeze: &FrozenOriginalCleanupLedgerTransfer,
        target_index: usize,
        settled: &[Entity],
        host_edge_detached: bool,
        input_isolation_is_current: &impl Fn(&World) -> bool,
    ) -> Result<PostCommitPhysicalSettlementProof, PostCommitAuthoritySettlementFailure> {
        let target = partition
            .target(target_index)
            .cloned()
            .ok_or(PostCommitAuthoritySettlementFailure::PrewriteDrift)?;
        let Some(original_removed) =
            original_freeze.removed_entities_for_authority_refreeze(world, original, None)
        else {
            return Err(PostCommitAuthoritySettlementFailure::PrewriteDrift);
        };
        if let Some(terminal) = target.terminal.as_ref() {
            let Some(obligation) = self.obligations.get(target.index) else {
                return Err(PostCommitAuthoritySettlementFailure::PrewriteDrift);
            };
            let Some(receipt) = self.receipts.get(target.index).and_then(Option::as_ref) else {
                return Err(PostCommitAuthoritySettlementFailure::PrewriteDrift);
            };
            if receipt.observed
                || receipt.disposition != target.disposition
                || receipt.binding != terminal.receipt_binding
                || cleanup_obligation_binding(obligation) != target.obligation_binding
                || !partition.cleanup_terminal_is_current(world, terminal, host_edge_detached)
                || !original_freeze.terminals_are_current(world, original)
                || !partition.refreeze(world, settled, None, host_edge_detached, &original_removed)
                || !input_isolation_is_current(world)
            {
                return Err(PostCommitAuthoritySettlementFailure::PrewriteDrift);
            }
            return Ok(PostCommitPhysicalSettlementProof::from_exact_receipt(
                target.entity,
                target.disposition,
                receipt.binding,
            ));
        }
        if self.receipts.get(target.index).is_none_or(Option::is_some)
            || self.obligations.get(target.index).is_none_or(|obligation| {
                obligation.identity != target.identity
                    || cleanup_obligation_binding(obligation) != target.obligation_binding
            })
            || !original_freeze.terminals_are_current(world, original)
            || !partition.refreeze(world, settled, None, host_edge_detached, &original_removed)
            || !input_isolation_is_current(world)
        {
            return Err(PostCommitAuthoritySettlementFailure::PrewriteDrift);
        }
        let Some(attempt) = self.attempt(world) else {
            return Err(PostCommitAuthoritySettlementFailure::PrewriteDrift);
        };
        if partition.requires_bulk_quarantine() {
            let Some(observed_quarantine) = quarantine_disposition_excluding_authorized_absence(
                &attempt.evidence,
                &self.obligations,
                &|entity| original_removed.contains(&entity),
            )
            .or_else(|| original_freeze.compound_quarantine_disposition()) else {
                self.issued_attempt = None;
                return Err(PostCommitAuthoritySettlementFailure::PrewriteDrift);
            };
            let dispositions_are_exact = partition.targets().iter().all(|target| {
                let Some(obligation) = self.obligations.get(target.index) else {
                    return false;
                };
                let Some(evidence) = attempt
                    .evidence
                    .iter()
                    .find(|(index, _)| *index == target.index)
                    .map(|(_, evidence)| evidence)
                else {
                    return false;
                };
                let mut expected = post_commit_authority_batch_disposition(
                    evidence,
                    obligation.expected_parent,
                    Some(observed_quarantine),
                );
                if target.is_top_slot() && expected == Some(CleanupDisposition::Removed) {
                    expected = Some(CleanupDisposition::RemovedDetached);
                }
                expected == Some(target.disposition)
            });
            if !dispositions_are_exact {
                self.issued_attempt = None;
                return Err(PostCommitAuthoritySettlementFailure::PrewriteDrift);
            }
            if !self.settle_with_failure_source(world, attempt, None, Some(observed_quarantine)) {
                return Err(PostCommitAuthoritySettlementFailure::PhysicalWriteRejected);
            }
            let original_is_current = original_freeze.terminals_are_current(world, original);
            let input_is_current = input_isolation_is_current(world);
            let terminals_frozen = partition.freeze_bulk_quarantine_terminals(world, self);
            let partition_is_current = terminals_frozen
                && partition.refreeze(world, settled, None, host_edge_detached, &original_removed);
            if !original_is_current
                || !input_is_current
                || !terminals_frozen
                || !partition_is_current
            {
                return Err(PostCommitAuthoritySettlementFailure::PostwriteDrift);
            }
            let receipt = self.receipts[target.index]
                .as_ref()
                .ok_or(PostCommitAuthoritySettlementFailure::ReceiptCommitRejected)?;
            return Ok(PostCommitPhysicalSettlementProof::from_exact_receipt(
                target.entity,
                target.disposition,
                receipt.binding,
            ));
        }
        let evidence = attempt
            .evidence
            .iter()
            .find(|(index, _)| *index == target.index)
            .map(|(_, evidence)| evidence);
        let Some(evidence) = evidence else {
            return Err(PostCommitAuthoritySettlementFailure::PrewriteDrift);
        };
        let obligation = &self.obligations[target.index];
        let observed_disposition =
            post_commit_authority_disposition(evidence, obligation.expected_parent);
        if observed_disposition != Some(target.disposition) {
            self.issued_attempt = None;
            return Err(PostCommitAuthoritySettlementFailure::PrewriteDrift);
        }
        let receipt_value = CleanupReceipt::new(
            self.ledger_domain,
            attempt.revision,
            target.index,
            target.disposition,
            obligation,
        );
        let Some(receipt) = self.prepare_receipt_batch(&attempt, vec![receipt_value]) else {
            return Err(PostCommitAuthoritySettlementFailure::ReceiptCommitRejected);
        };
        let observed_claim = world
            .get_resource::<RouteEntityRegistry>()
            .and_then(|registry| registry.record(target.entity))
            .map(|record| record.claim());
        let retirement = match observed_claim {
            Some(claim) if target.identity.registered() == Some(&claim) => Some(
                prepare_exact_registry_retirement(
                    world.resource::<RouteEntityRegistry>(),
                    vec![claim],
                )
                .map_err(|_| PostCommitAuthoritySettlementFailure::PrewriteDrift)?,
            ),
            None => None,
            Some(_) => return Err(PostCommitAuthoritySettlementFailure::PrewriteDrift),
        };

        if world.get_entity(target.entity).is_ok() {
            if !cleanup_obligation_has_exact_authority(world, obligation)
                || !world.despawn(target.entity)
            {
                self.issued_attempt = None;
                return Err(PostCommitAuthoritySettlementFailure::PhysicalWriteRejected);
            }
            world.flush();
            if !original_freeze.terminals_are_current(world, original)
                || !partition.refreeze(
                    world,
                    settled,
                    Some(target.entity),
                    host_edge_detached,
                    &original_removed,
                )
                || !input_isolation_is_current(world)
            {
                self.issued_attempt = None;
                return Err(PostCommitAuthoritySettlementFailure::PostwriteDrift);
            }
        } else if target.disposition != CleanupDisposition::AlreadyAbsent {
            self.issued_attempt = None;
            return Err(PostCommitAuthoritySettlementFailure::PrewriteDrift);
        }

        if let Some(retirement) = retirement {
            apply_prepared_exact_registry_retirement(
                &mut world.resource_mut::<RouteEntityRegistry>(),
                retirement,
            );
        }
        self.commit_receipt_batch(receipt);
        let mut completed = settled.to_vec();
        completed.push(target.entity);
        if !original_freeze.terminals_are_current(world, original)
            || !partition.refreeze(
                world,
                &completed,
                None,
                host_edge_detached,
                &original_removed,
            )
            || !input_isolation_is_current(world)
        {
            self.issued_attempt = None;
            return Err(PostCommitAuthoritySettlementFailure::PostwriteDrift);
        }
        self.finish_if_settled();
        let receipt = self.receipts[target.index]
            .as_ref()
            .ok_or(PostCommitAuthoritySettlementFailure::ReceiptCommitRejected)?;
        Ok(PostCommitPhysicalSettlementProof::from_exact_receipt(
            target.entity,
            target.disposition,
            receipt.binding,
        ))
    }
}

fn post_commit_original_disposition(
    evidence: &CleanupEvidence,
    expected_parent: Entity,
) -> Option<CleanupDisposition> {
    match evidence {
        CleanupEvidence::Exact {
            actual_parent,
            foreign_descendants,
            foreign_components,
            missing_components,
            protected_parent,
            relationship_cycle,
            ..
        } => Some(if protected_parent.is_some() {
            CleanupDisposition::QuarantinedProtectedParent
        } else if !foreign_descendants.is_empty() || *relationship_cycle {
            CleanupDisposition::QuarantinedForeignDescendant
        } else if !foreign_components.is_empty() || !missing_components.is_empty() {
            CleanupDisposition::QuarantinedForeignComponent
        } else if *actual_parent == Some(expected_parent) {
            CleanupDisposition::Removed
        } else {
            CleanupDisposition::RemovedDetached
        }),
        evidence => post_commit_authority_disposition(evidence, expected_parent),
    }
}

fn post_commit_authority_disposition(
    evidence: &CleanupEvidence,
    expected_parent: Entity,
) -> Option<CleanupDisposition> {
    match evidence {
        CleanupEvidence::AlreadyAbsent => Some(CleanupDisposition::AlreadyAbsent),
        CleanupEvidence::Exact { actual_parent, .. } => {
            Some(if *actual_parent == Some(expected_parent) {
                CleanupDisposition::Removed
            } else {
                CleanupDisposition::RemovedDetached
            })
        }
        CleanupEvidence::AuthorityLostResidual { entity_present, .. } => Some(if *entity_present {
            CleanupDisposition::RejectedProtected
        } else {
            CleanupDisposition::AlreadyAbsent
        }),
        CleanupEvidence::StaleClaim => Some(CleanupDisposition::RejectedProtected),
    }
}

fn post_commit_authority_batch_disposition(
    evidence: &CleanupEvidence,
    expected_parent: Entity,
    compound_quarantine: Option<CleanupDisposition>,
) -> Option<CleanupDisposition> {
    let base = post_commit_authority_disposition(evidence, expected_parent)?;
    if base == CleanupDisposition::AlreadyAbsent {
        Some(base)
    } else {
        compound_quarantine.or(Some(base))
    }
}

fn post_commit_physical_source_binding(
    ledger_domain: u64,
    index: usize,
    obligation: &CleanupObligation,
    disposition: CleanupDisposition,
) -> u64 {
    let mut digest = StructuredDigest::new(POST_COMMIT_PHYSICAL_SOURCE_DOMAIN);
    digest.push(ledger_domain);
    digest.push(index as u64);
    digest.push(disposition.tag());
    digest.obligation(obligation);
    digest.finish()
}
