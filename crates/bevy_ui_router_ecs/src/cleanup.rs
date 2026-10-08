use std::{
    collections::{HashMap, HashSet},
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
    ids::RouteSlotId,
    integration::{
        BackNoTargetCleanupBatchView, CleanupBatch, CleanupCompletion, PostCommitCleanupBatchView,
        RetainedLeafProjection,
    },
    slot::RouteSlotKey,
};

use crate::{
    app_ext::{RouterHandle, RouterHostRegistration},
    branch_transaction::{
        FailedPreparedBranch, PreparedBranch, RealizedBranch, RejectedChildSlot,
        SealedReconciliationSlotRole, SealedReconciliationSplice,
    },
    components::{
        ChildSlotAuthority, ChildSlotLifecycle, ProvisionalSlotLifecycle, RouteOwnershipSeal,
        RouterInputBarrier,
    },
    input::{RouteFocusState, observed_descendant_closure, prepare_focus_mutation},
    lifecycle_preparation::LifecyclePreflightEvidence,
    mount::{ChildSlotResidual, MountedChildSlot, residual_child_slot_is_isolated},
    navigation::{HostBinding, PendingFailedBranch, RetiredHostBinding},
    presentation::{FrameContentTarget, FrameLifecycle, FrameOutput, RealizedFrame},
    registry::{
        ComponentManifest, EntityLifecycle, EntityOwner, ExactRegistryClaim,
        PreparedAuthorityLostResidualTerminalProjection, PreparedAuthorityLostResidualTransfer,
        PreparedAuthorityPublicationCapacity, PreparedExactRegistryRetirement,
        PreparedQuarantineTransfer, PreparedUnresolvedResidualTransfer, ProvisionalOwner,
        QuarantineFacts, QuarantineLeaseId, QuarantineLeaseTable, RegistryOwner, ResidualLeaseId,
        RouteEntityRegistry, apply_prepared_authority_lost_residual_transfer,
        apply_prepared_exact_registry_retirement, apply_prepared_quarantine_transfer,
        apply_prepared_unresolved_residual_transfer, prepare_authority_lost_residual_transfer,
        prepare_authority_lost_residual_transfer_in_publication, prepare_exact_registry_retirement,
        prepare_unresolved_residual_transfer,
        prepare_verified_authority_cleanup_quarantine_transfer,
        prepare_verified_cleanup_quarantine_transfer,
    },
};
use bevy_camera::visibility::Visibility;
use bevy_input_focus::InputFocus;
use bevy_picking::Pickable;
use bevy_ui::{ComputedUiRenderTargetInfo, ComputedUiTargetCamera, Node, Val};

const MANIFEST_DOMAIN: u64 = 0x4543_534d_414e_4946;
const ATTEMPT_DOMAIN: u64 = 0x4543_5341_5454_454d;
const RECEIPT_DOMAIN: u64 = 0x4543_5352_4543_5054;
const OBLIGATION_DOMAIN: u64 = 0x4543_534f_424c_4947;
const COMPLETION_DOMAIN: u64 = 0x4543_5343_4f4d_504c;
const POST_COMMIT_PHYSICAL_SOURCE_DOMAIN: u64 = 0x5232_5048_5953_4943;
static NEXT_LEDGER_DOMAIN: AtomicU64 = AtomicU64::new(1);

mod capacity;
#[cfg(test)]
pub(crate) use capacity::BackCapacityCellDispositionForTest;
#[cfg(test)]
use capacity::prepared_installed_back_zero_completion;
use capacity::{BackCapacityCellId, ManifestNeutralLedgerStorage};
pub(crate) use capacity::{
    BackCapacityDomainIssuer, BackCapacityEnqueueRetireResidual, BackCapacityIssuer,
    BackCapacityIssuerInstallError, BackCapacityOwnerId, BackCapacityRetireDisposition,
    BackCapacityRetireError, BackCapacityRetireOutcome, BackCapacityRetirementEvidence,
    BoundBackCapacityReservation, EnqueueRejected, ManifestNeutralLedgerSlot,
    NoTargetCapacityPermit, PreparationCapacityDomain, PreparationReleaseGeneration,
};

#[cfg(test)]
thread_local! {
    static FAIL_NEXT_RESERVATION: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FAIL_NEXT_LEDGER_DOMAIN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FAIL_NEXT_UNFINALIZED_LEDGER: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static LEDGER_DOMAIN_ALLOCATIONS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    static NESTED_AUTHORITY_CAPACITY: std::cell::Cell<Option<AuthorityPreparationCapacitySite>> = const { std::cell::Cell::new(None) };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AuthorityPreparationCapacitySite {
    Dispositions,
    RetainedEntities,
    ResidualGroups,
    PickableRepair,
    LostProjection,
    CompletedRetention,
}

#[cfg(test)]
pub(crate) fn fail_next_authority_preparation_capacity_for_test(
    site: AuthorityPreparationCapacitySite,
) {
    NESTED_AUTHORITY_CAPACITY.set(Some(site));
}

fn authority_preparation_capacity_input(
    site: AuthorityPreparationCapacitySite,
    count: usize,
) -> usize {
    #[cfg(test)]
    if NESTED_AUTHORITY_CAPACITY.get() == Some(site) {
        NESTED_AUTHORITY_CAPACITY.set(None);
        return usize::MAX;
    }
    let _ = site;
    count
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
        let RegistryOwner::Committed(EntityOwner::FrameShell(frame)) = obligation
            .identity
            .registered()
            .expect("registered test fixture")
            .owner()
        else {
            return None;
        };
        Some((
            frame.clone(),
            obligation
                .identity
                .registered()
                .expect("registered test fixture")
                .clone(),
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
pub(crate) enum CleanupPrepublicationPhase {
    VerifiedTransfer,
}

#[cfg(test)]
#[derive(bevy_ecs::prelude::Resource)]
pub(crate) struct CleanupPrepublicationDrift {
    phase: CleanupPrepublicationPhase,
    external_parent: Option<Entity>,
    focus: Option<Entity>,
    fired: bool,
}

#[cfg(test)]
impl CleanupPrepublicationDrift {
    pub(crate) const fn external_parent(phase: CleanupPrepublicationPhase, parent: Entity) -> Self {
        Self {
            phase,
            external_parent: Some(parent),
            focus: None,
            fired: false,
        }
    }
}

#[cfg(test)]
fn apply_cleanup_prepublication_drift(world: &mut World, phase: CleanupPrepublicationPhase) {
    let Some((external_parent, focus)) = world
        .get_resource_mut::<CleanupPrepublicationDrift>()
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
        let seal = world
            .resource_mut::<RouteEntityRegistry>()
            .issue_unregistered_seal_for_test();
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
        registry
            .register(entity, owner, manifest, lifecycle)
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
            let stale_seal = world
                .resource_mut::<RouteEntityRegistry>()
                .issue_unregistered_seal_for_test();
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
    identity: CleanupTargetIdentity,
    manifest: ComponentManifest,
    expected_parent: Entity,
    unit_root: Entity,
    authorized_parent: Entity,
    allow_missing_components: bool,
    terminal_mode: CleanupTerminalMode,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CleanupTargetIdentity {
    Registered(ExactRegistryClaim),
    ContainedContent {
        entity: Entity,
        root: ExactRegistryClaim,
        shell: ExactRegistryClaim,
    },
}

impl CleanupTargetIdentity {
    pub(crate) fn entity(&self) -> Entity {
        match self {
            Self::Registered(claim) => claim.entity(),
            Self::ContainedContent { entity, .. } => *entity,
        }
    }

    pub(crate) fn registered(&self) -> Option<&ExactRegistryClaim> {
        match self {
            Self::Registered(claim) => Some(claim),
            Self::ContainedContent { .. } => None,
        }
    }
}

impl CleanupObligation {
    fn entity(&self) -> Entity {
        self.identity.entity()
    }
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
enum CleanupParentBinding {
    Structural {
        upstream: Option<crate::presentation::OutletBinding>,
    },
    Outlet(crate::presentation::OutletBinding),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CleanupMountParentSettlement {
    physical: Entity,
    binding: CleanupParentBinding,
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

impl CleanupMountParentSettlement {
    fn try_clone(&self) -> Result<Self, ()> {
        let try_clone_entities = |source: &[Entity]| {
            let mut cloned = Vec::new();
            cloned.try_reserve_exact(source.len()).map_err(|_| ())?;
            cloned.extend_from_slice(source);
            Ok::<_, ()>(cloned)
        };
        Ok(Self {
            physical: self.physical,
            binding: self.binding.clone(),
            claim: self.claim.try_clone()?,
            manifest: self.manifest.try_clone()?,
            physical_manifest: self.physical_manifest.try_clone()?,
            expected_parent: self.expected_parent,
            expected_parent_claim: self
                .expected_parent_claim
                .as_ref()
                .map(ExactRegistryClaim::try_clone)
                .transpose()?,
            removed_child: self.removed_child,
            removed_child_was_attached: self.removed_child_was_attached,
            children_before: try_clone_entities(&self.children_before)?,
            children_after: try_clone_entities(&self.children_after)?,
        })
    }

    fn bookkeeping_claim(&self) -> Option<&ExactRegistryClaim> {
        (self.physical == self.claim.entity()).then_some(&self.claim)
    }

    fn next_manifest_base(&self) -> &ComponentManifest {
        match &self.binding {
            CleanupParentBinding::Outlet(binding) if binding.is_native() => &self.physical_manifest,
            _ => &self.manifest,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CleanupMountParentCandidate {
    physical: Entity,
    binding: CleanupParentBinding,
    claim: ExactRegistryClaim,
    failed_claim_source: Option<FailedBoundarySource>,
    supplied_parent: Option<(Entity, Option<ExactRegistryClaim>)>,
    failed_parent_source: Option<FailedBoundarySource>,
    removed_child: Entity,
    removed_child_was_attached: bool,
    children_before: Vec<Entity>,
    children_after: Vec<Entity>,
}

#[derive(Clone, Debug)]
struct PreparedCleanupQuarantineMountParentDetachment {
    parent: CleanupMountParentSettlement,
    parent_next_manifest: ComponentManifest,
    parent_physical_manifest_after: ComponentManifest,
    child_claim: ExactRegistryClaim,
    child_manifest_before: ComponentManifest,
    child_manifest_after: ComponentManifest,
}

#[derive(Clone, Debug)]
struct PreparedCleanupQuarantineDetachment {
    targets_after_inert: Vec<FrozenFailureCleanupObservedEntity>,
    protected_before: Vec<FrozenFailureCleanupObservedEntity>,
    edges: Vec<PreparedCleanupQuarantineMountParentDetachment>,
}

#[derive(Clone, Debug, Default)]
struct FrozenCleanupQuarantineDetachment {
    edges: Vec<PreparedCleanupQuarantineMountParentDetachment>,
    protected_after: Vec<FrozenFailureCleanupObservedEntity>,
    targets_after: Vec<FrozenFailureCleanupObservedEntity>,
}

#[derive(Debug)]
struct CleanupQuarantineDetachmentMergeConflict {
    existing: FrozenCleanupQuarantineDetachment,
    newer: FrozenCleanupQuarantineDetachment,
    kind: CleanupQuarantineDetachmentMergeConflictKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CleanupQuarantineDetachmentMergeConflictKind {
    DuplicateParent(Entity),
    DuplicateChild(Entity),
    ExistingTargetBecameProtected(Entity),
    ExistingProtectedBecameTarget(Entity),
    ProtectedProjectionDrift(Entity),
}

fn detachment_edges_intrinsic_conflict(
    edges: &[PreparedCleanupQuarantineMountParentDetachment],
) -> Option<CleanupQuarantineDetachmentMergeConflictKind> {
    for (index, edge) in edges.iter().enumerate() {
        if edges[..index]
            .iter()
            .any(|prior| prior.parent.claim == edge.parent.claim)
        {
            return Some(
                CleanupQuarantineDetachmentMergeConflictKind::DuplicateParent(edge.parent.physical),
            );
        }
        if edges[..index]
            .iter()
            .any(|prior| prior.child_claim.entity() == edge.child_claim.entity())
        {
            return Some(
                CleanupQuarantineDetachmentMergeConflictKind::DuplicateChild(
                    edge.child_claim.entity(),
                ),
            );
        }
    }
    None
}

#[derive(Debug)]
struct FrozenCleanupMountParentDrift {
    expected: CleanupMountParentSettlement,
    observed_parent: FrozenFailureCleanupObservedEntity,
    observed_removed_child: FrozenFailureCleanupObservedEntity,
}

#[derive(Debug)]
enum CleanupSettlementPermanentCause {
    RelatedProtectedPartition {
        expected: Vec<CleanupProtectedEntity>,
        targets: Vec<Entity>,
    },
    DetachmentState {
        expected: FrozenCleanupQuarantineDetachment,
        consumed_projection_matches: bool,
        unconsumed_mount_parent_drifts: Vec<FrozenCleanupMountParentDrift>,
    },
    MergeConflict(CleanupQuarantineDetachmentMergeConflict),
}

#[derive(Debug)]
pub(crate) struct CleanupSettlementPermanentEvidence {
    ledger_domain: u64,
    manifest_digest: u64,
    obligation_count: usize,
    cause: Box<CleanupSettlementPermanentCause>,
    observed: Vec<FrozenFailureCleanupObservedEntity>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CleanupQuarantineDetachmentApplyOutcome {
    applied_edges: usize,
    exact: bool,
}

#[derive(Clone, Debug)]
struct PreparedCleanupQuarantinePublicationGuard {
    targets: Vec<FrozenFailureCleanupObservedEntity>,
    protected: Vec<FrozenFailureCleanupObservedEntity>,
}

impl PreparedCleanupQuarantinePublicationGuard {
    fn matches(&self, world: &World) -> bool {
        let registry = world.resource::<RouteEntityRegistry>();
        self.targets
            .iter()
            .chain(&self.protected)
            .all(|expected| expected.matches_exact(world, &registry))
    }

    fn verified_manifests(
        &self,
        claims: &[ExactRegistryClaim],
    ) -> Option<Vec<(Entity, ComponentManifest)>> {
        let mut manifests = Vec::new();
        manifests.try_reserve_exact(claims.len()).ok()?;
        for claim in claims {
            let target = self
                .targets
                .iter()
                .find(|target| target.entity == claim.entity())?;
            manifests.push((claim.entity(), target.manifest.clone()?));
        }
        Some(manifests)
    }
}

impl PreparedCleanupQuarantineDetachment {
    fn intrinsic_conflict_kind(&self) -> Option<CleanupQuarantineDetachmentMergeConflictKind> {
        detachment_edges_intrinsic_conflict(&self.edges)
    }

    fn into_frozen_prefix(mut self, applied_edges: usize) -> FrozenCleanupQuarantineDetachment {
        debug_assert!(applied_edges <= self.edges.len());
        self.edges.truncate(applied_edges);
        FrozenCleanupQuarantineDetachment {
            edges: self.edges,
            protected_after: self.protected_before,
            targets_after: self.targets_after_inert,
        }
    }

    fn prepared_parent_manifests(&self) -> Option<Vec<ComponentManifest>> {
        let mut manifests = Vec::new();
        manifests.try_reserve_exact(self.edges.len()).ok()?;
        for edge in &self.edges {
            manifests.push(edge.parent_next_manifest.try_clone().ok()?);
        }
        Some(manifests)
    }

    fn after_inert_matches(&self, world: &World) -> bool {
        let registry = world.resource::<RouteEntityRegistry>();
        self.targets_after_inert
            .iter()
            .all(|expected| expected.matches_exact(world, &registry))
            && self
                .protected_before
                .iter()
                .all(|expected| expected.matches_exact(world, &registry))
    }

    fn apply_and_refreeze(
        &self,
        world: &mut World,
        parent_manifests: Vec<ComponentManifest>,
    ) -> CleanupQuarantineDetachmentApplyOutcome {
        debug_assert_eq!(parent_manifests.len(), self.edges.len());
        if !self.after_inert_matches(world) {
            return CleanupQuarantineDetachmentApplyOutcome {
                applied_edges: 0,
                exact: false,
            };
        }
        for (edge_index, (edge, parent_next_manifest)) in
            self.edges.iter().zip(parent_manifests).enumerate()
        {
            let edge_is_current = {
                let registry = world.resource::<RouteEntityRegistry>();
                mount_parent_frozen_manifest_matches(&registry, &edge.parent)
                    && mount_parent_physical_matches(world, &registry, &edge.parent)
                    && world
                        .get::<ChildOf>(edge.child_claim.entity())
                        .map(ChildOf::parent)
                        == Some(edge.parent.physical)
                    && edge
                        .child_manifest_before
                        .matches_without_materialization(world, edge.child_claim.entity())
            };
            if !edge_is_current {
                return CleanupQuarantineDetachmentApplyOutcome {
                    applied_edges: edge_index,
                    exact: false,
                };
            }

            world
                .entity_mut(edge.child_claim.entity())
                .remove::<ChildOf>();
            world.flush();

            if edge.parent.bookkeeping_claim().is_some()
                && !world
                    .resource_mut::<RouteEntityRegistry>()
                    .compare_exchange_manifest_exact(
                        &edge.parent.claim,
                        &edge.parent.manifest,
                        parent_next_manifest,
                    )
            {
                return CleanupQuarantineDetachmentApplyOutcome {
                    applied_edges: edge_index + 1,
                    exact: false,
                };
            }
            if !self.after_detach_matches(world, edge_index + 1) {
                return CleanupQuarantineDetachmentApplyOutcome {
                    applied_edges: edge_index + 1,
                    exact: false,
                };
            }
        }
        CleanupQuarantineDetachmentApplyOutcome {
            applied_edges: self.edges.len(),
            exact: true,
        }
    }

    fn after_detach_matches(&self, world: &World, detached_edges: usize) -> bool {
        let registry = world.resource::<RouteEntityRegistry>();
        self.targets_after_inert.iter().all(|expected| {
            match self
                .edges
                .iter()
                .take(detached_edges)
                .find(|edge| edge.child_claim.entity() == expected.entity)
            {
                Some(edge) => expected.matches_projection(
                    world,
                    &registry,
                    None,
                    &expected.children,
                    expected.children_component_present,
                    expected.registry_manifest.as_ref(),
                    edge.child_manifest_after
                        .matches_without_materialization(world, expected.entity),
                ),
                None => expected.matches_exact(world, &registry),
            }
        }) && self.protected_before.iter().all(|expected| {
            match self
                .edges
                .iter()
                .take(detached_edges)
                .find(|edge| edge.parent.physical == expected.entity)
            {
                Some(edge) => expected.matches_projection(
                    world,
                    &registry,
                    expected.parent,
                    &edge.parent.children_after,
                    !edge.parent.children_after.is_empty(),
                    edge.parent
                        .bookkeeping_claim()
                        .map(|_| &edge.parent_next_manifest),
                    edge.parent_physical_manifest_after
                        .matches_without_materialization(world, expected.entity),
                ),
                None => expected.matches_exact(world, &registry),
            }
        })
    }
}

impl FrozenCleanupQuarantineDetachment {
    fn reserve_merge_from_prepared(
        &mut self,
        prepared: &PreparedCleanupQuarantineDetachment,
    ) -> bool {
        self.edges.try_reserve(prepared.edges.len()).is_ok()
            && self
                .protected_after
                .try_reserve(prepared.protected_before.len())
                .is_ok()
            && self
                .targets_after
                .try_reserve(prepared.targets_after_inert.len())
                .is_ok()
    }

    #[cfg(test)]
    fn from_prepared(
        detachment: &PreparedCleanupQuarantineDetachment,
        applied_edges: usize,
    ) -> Self {
        let edges = detachment
            .edges
            .iter()
            .take(applied_edges)
            .cloned()
            .collect::<Vec<_>>();
        Self {
            edges,
            protected_after: detachment.protected_before.clone(),
            targets_after: detachment.targets_after_inert.clone(),
        }
    }

    fn contains_parent(&self, parent: &CleanupMountParentSettlement) -> bool {
        self.edges.iter().any(|edge| {
            edge.parent.physical == parent.physical && edge.parent.claim == parent.claim
        })
    }

    fn intrinsic_conflict_kind(&self) -> Option<CleanupQuarantineDetachmentMergeConflictKind> {
        detachment_edges_intrinsic_conflict(&self.edges)
    }

    fn merge_conflict_with_prepared(
        &self,
        newer: &PreparedCleanupQuarantineDetachment,
    ) -> Option<CleanupQuarantineDetachmentMergeConflictKind> {
        if let Some(kind) = newer.intrinsic_conflict_kind() {
            return Some(kind);
        }
        for newer_edge in &newer.edges {
            if self
                .edges
                .iter()
                .any(|existing| existing.parent.physical == newer_edge.parent.physical)
            {
                return Some(
                    CleanupQuarantineDetachmentMergeConflictKind::DuplicateParent(
                        newer_edge.parent.physical,
                    ),
                );
            }
            if self
                .edges
                .iter()
                .any(|existing| existing.child_claim.entity() == newer_edge.child_claim.entity())
            {
                return Some(
                    CleanupQuarantineDetachmentMergeConflictKind::DuplicateChild(
                        newer_edge.child_claim.entity(),
                    ),
                );
            }
        }
        for newer_protected in &newer.protected_before {
            if self
                .targets_after
                .iter()
                .any(|target| target.entity == newer_protected.entity)
            {
                return Some(
                    CleanupQuarantineDetachmentMergeConflictKind::ExistingTargetBecameProtected(
                        newer_protected.entity,
                    ),
                );
            }
            if !newer
                .edges
                .iter()
                .any(|edge| edge.parent.physical == newer_protected.entity)
                && self.protected_after.iter().any(|existing| {
                    existing.entity == newer_protected.entity && *existing != *newer_protected
                })
            {
                return Some(
                    CleanupQuarantineDetachmentMergeConflictKind::ProtectedProjectionDrift(
                        newer_protected.entity,
                    ),
                );
            }
        }
        newer.targets_after_inert.iter().find_map(|newer_target| {
            self.protected_after
                .iter()
                .any(|protected| protected.entity == newer_target.entity)
                .then_some(
                    CleanupQuarantineDetachmentMergeConflictKind::ExistingProtectedBecameTarget(
                        newer_target.entity,
                    ),
                )
        })
    }

    fn merge_conflict_kind(
        &self,
        newer: &Self,
    ) -> Option<CleanupQuarantineDetachmentMergeConflictKind> {
        if let Some(kind) = newer.intrinsic_conflict_kind() {
            return Some(kind);
        }
        for newer_edge in &newer.edges {
            if self
                .edges
                .iter()
                .any(|existing| existing.parent.physical == newer_edge.parent.physical)
            {
                return Some(
                    CleanupQuarantineDetachmentMergeConflictKind::DuplicateParent(
                        newer_edge.parent.physical,
                    ),
                );
            }
            if self
                .edges
                .iter()
                .any(|existing| existing.child_claim.entity() == newer_edge.child_claim.entity())
            {
                return Some(
                    CleanupQuarantineDetachmentMergeConflictKind::DuplicateChild(
                        newer_edge.child_claim.entity(),
                    ),
                );
            }
        }
        for newer_protected in &newer.protected_after {
            if self
                .targets_after
                .iter()
                .any(|target| target.entity == newer_protected.entity)
            {
                return Some(
                    CleanupQuarantineDetachmentMergeConflictKind::ExistingTargetBecameProtected(
                        newer_protected.entity,
                    ),
                );
            }
            if !newer
                .edges
                .iter()
                .any(|edge| edge.parent.physical == newer_protected.entity)
                && self.protected_after.iter().any(|existing| {
                    existing.entity == newer_protected.entity && *existing != *newer_protected
                })
            {
                return Some(
                    CleanupQuarantineDetachmentMergeConflictKind::ProtectedProjectionDrift(
                        newer_protected.entity,
                    ),
                );
            }
        }
        newer.targets_after.iter().find_map(|newer_target| {
            self.protected_after
                .iter()
                .any(|protected| protected.entity == newer_target.entity)
                .then_some(
                    CleanupQuarantineDetachmentMergeConflictKind::ExistingProtectedBecameTarget(
                        newer_target.entity,
                    ),
                )
        })
    }

    fn merge_prechecked(&mut self, mut newer: Self) {
        debug_assert!(self.merge_conflict_kind(&newer).is_none());

        for newer_protected in newer.protected_after.drain(..) {
            match self
                .protected_after
                .iter_mut()
                .find(|protected| protected.entity == newer_protected.entity)
            {
                Some(existing)
                    if newer
                        .edges
                        .iter()
                        .any(|edge| edge.parent.physical == newer_protected.entity) =>
                {
                    *existing = newer_protected;
                }
                Some(existing) => debug_assert_eq!(*existing, newer_protected),
                None => self.protected_after.push(newer_protected),
            }
        }
        for newer_target in newer.targets_after.drain(..) {
            if !self
                .targets_after
                .iter()
                .any(|target| target.entity == newer_target.entity)
            {
                self.targets_after.push(newer_target);
            }
        }
        self.edges.append(&mut newer.edges);
    }

    fn matches_exact(&self, world: &World) -> bool {
        let registry = world.resource::<RouteEntityRegistry>();
        self.protected_after.iter().all(|expected| {
            match self
                .edges
                .iter()
                .find(|edge| edge.parent.physical == expected.entity)
            {
                Some(edge) => expected.matches_projection(
                    world,
                    &registry,
                    expected.parent,
                    &edge.parent.children_after,
                    !edge.parent.children_after.is_empty(),
                    edge.parent
                        .bookkeeping_claim()
                        .map(|_| &edge.parent_next_manifest),
                    edge.parent_physical_manifest_after
                        .matches_without_materialization(world, expected.entity),
                ),
                None => expected.matches_exact(world, &registry),
            }
        }) && self.targets_after.iter().all(|expected| {
            match self
                .edges
                .iter()
                .find(|edge| edge.child_claim.entity() == expected.entity)
            {
                Some(edge) => expected.matches_projection(
                    world,
                    &registry,
                    None,
                    &expected.children,
                    expected.children_component_present,
                    expected.registry_manifest.as_ref(),
                    edge.child_manifest_after
                        .matches_without_materialization(world, expected.entity),
                ),
                None => expected.matches_exact(world, &registry),
            }
        })
    }

    fn matches(&self, world: &World) -> bool {
        let registry = world.resource::<RouteEntityRegistry>();
        self.edges.iter().all(|edge| {
            edge.parent.bookkeeping_claim().is_none_or(|claim| {
                registry.record(edge.parent.physical).is_some_and(|record| {
                    record.claim() == *claim && record.manifest() == &edge.parent_next_manifest
                })
            }) && mount_parent_physical_matches_after_detachment(world, &registry, &edge.parent)
                && world
                    .get::<ChildOf>(edge.child_claim.entity())
                    .map(ChildOf::parent)
                    != Some(edge.parent.physical)
        }) && self.protected_after.iter().all(|expected| {
            match self
                .edges
                .iter()
                .find(|edge| edge.parent.physical == expected.entity)
            {
                Some(edge) => expected.matches_projection(
                    world,
                    &registry,
                    expected.parent,
                    &edge.parent.children_after,
                    !edge.parent.children_after.is_empty(),
                    edge.parent
                        .bookkeeping_claim()
                        .map(|_| &edge.parent_next_manifest),
                    edge.parent_physical_manifest_after
                        .matches_without_materialization(world, expected.entity),
                ),
                None => expected.matches_exact(world, &registry),
            }
        }) && self
            .targets_after
            .iter()
            .all(|expected| expected.matches_allowing_parent_relationship(world, &registry))
    }
}

impl CleanupSettlementPermanentEvidence {
    fn freeze(
        world: &World,
        ledger_domain: u64,
        manifest_digest: u64,
        obligation_count: usize,
        cause: CleanupSettlementPermanentCause,
    ) -> Result<Self, CleanupSettlementPermanentCause> {
        let Some(registry) = world.get_resource::<RouteEntityRegistry>() else {
            return Err(cause);
        };
        let detachment_capacity = |detachment: &FrozenCleanupQuarantineDetachment| {
            detachment
                .edges
                .len()
                .checked_mul(2)
                .and_then(|count| count.checked_add(detachment.protected_after.len()))
                .and_then(|count| count.checked_add(detachment.targets_after.len()))
        };
        let capacity = match &cause {
            CleanupSettlementPermanentCause::RelatedProtectedPartition { expected, targets } => {
                expected.len().checked_add(targets.len())
            }
            CleanupSettlementPermanentCause::DetachmentState {
                expected,
                unconsumed_mount_parent_drifts,
                ..
            } => detachment_capacity(expected).and_then(|count| {
                unconsumed_mount_parent_drifts
                    .len()
                    .checked_mul(2)
                    .and_then(|drifts| count.checked_add(drifts))
            }),
            CleanupSettlementPermanentCause::MergeConflict(conflict) => {
                detachment_capacity(&conflict.existing).and_then(|count| {
                    detachment_capacity(&conflict.newer).and_then(|newer| count.checked_add(newer))
                })
            }
        };
        let Some(capacity) = capacity else {
            return Err(cause);
        };
        let mut entities = Vec::new();
        if entities.try_reserve_exact(capacity).is_err() {
            return Err(cause);
        }
        let append_detachment_entities =
            |entities: &mut Vec<Entity>, detachment: &FrozenCleanupQuarantineDetachment| {
                for edge in &detachment.edges {
                    entities.push(edge.parent.physical);
                    entities.push(edge.child_claim.entity());
                }
                entities.extend(
                    detachment
                        .protected_after
                        .iter()
                        .map(|entity| entity.entity),
                );
                entities.extend(detachment.targets_after.iter().map(|entity| entity.entity));
            };
        match &cause {
            CleanupSettlementPermanentCause::RelatedProtectedPartition { expected, targets } => {
                entities.extend(expected.iter().map(|fact| fact.observed.entity));
                entities.extend(targets.iter().copied());
            }
            CleanupSettlementPermanentCause::DetachmentState {
                expected,
                unconsumed_mount_parent_drifts,
                ..
            } => {
                append_detachment_entities(&mut entities, expected);
                for drift in unconsumed_mount_parent_drifts {
                    entities.push(drift.expected.claim.entity());
                    entities.push(drift.expected.removed_child);
                }
            }
            CleanupSettlementPermanentCause::MergeConflict(conflict) => {
                append_detachment_entities(&mut entities, &conflict.existing);
                append_detachment_entities(&mut entities, &conflict.newer);
            }
        }
        entities.sort_by_key(|entity| entity.to_bits());
        entities.dedup();

        let mut observed = Vec::new();
        if observed.try_reserve_exact(entities.len()).is_err() {
            return Err(cause);
        }
        observed.extend(
            entities
                .into_iter()
                .map(|entity| FrozenFailureCleanupObservedEntity::observe(world, registry, entity)),
        );
        Ok(Self {
            ledger_domain,
            manifest_digest,
            obligation_count,
            cause: Box::new(cause),
            observed,
        })
    }
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
pub(crate) struct FrozenFailureCleanupObservedEntity {
    entity: Entity,
    present: bool,
    parent: Option<Entity>,
    children: Vec<Entity>,
    children_component_present: bool,
    registry_claim: Option<ExactRegistryClaim>,
    registry_manifest: Option<ComponentManifest>,
    ownership_seal: Option<RouteOwnershipSeal>,
    manifest: Option<ComponentManifest>,
    visibility: Option<Visibility>,
    pickable: Option<(bool, bool)>,
    barrier: Option<RouterInputBarrier>,
    frame_lifecycle: Option<FrameLifecycle>,
    child_slot_lifecycle: Option<ChildSlotLifecycle>,
    child_slot_authority: Option<ChildSlotAuthority>,
    provisional_slot_lifecycle: Option<ProvisionalSlotLifecycle>,
    host_registration: Option<(RouterHandle, u64)>,
}

impl FrozenFailureCleanupObservedEntity {
    pub(crate) fn observe(world: &World, registry: &RouteEntityRegistry, entity: Entity) -> Self {
        Self {
            entity,
            present: world.get_entity(entity).is_ok(),
            parent: world.get::<ChildOf>(entity).map(ChildOf::parent),
            children: observed_children(world, entity),
            children_component_present: world.get::<Children>(entity).is_some(),
            registry_claim: registry.record(entity).map(|record| record.claim()),
            registry_manifest: registry
                .record(entity)
                .map(|record| record.manifest().clone()),
            ownership_seal: world.get::<RouteOwnershipSeal>(entity).copied(),
            manifest: ComponentManifest::from_world(world, entity),
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

    pub(crate) const fn is_present(&self) -> bool {
        self.present
    }

    pub(crate) const fn registry_claim(&self) -> Option<&ExactRegistryClaim> {
        self.registry_claim.as_ref()
    }

    pub(crate) fn matches_exact(&self, world: &World, registry: &RouteEntityRegistry) -> bool {
        self.matches_projection(
            world,
            registry,
            self.parent,
            &self.children,
            self.children_component_present,
            self.registry_manifest.as_ref(),
            self.manifest.as_ref().is_some_and(|manifest| {
                manifest.matches_without_materialization(world, self.entity)
            }) || self.manifest.is_none() && world.get_entity(self.entity).is_err(),
        )
    }

    fn matches_projection(
        &self,
        world: &World,
        registry: &RouteEntityRegistry,
        parent: Option<Entity>,
        children: &[Entity],
        children_component_present: bool,
        registry_manifest: Option<&ComponentManifest>,
        manifest_is_exact: bool,
    ) -> bool {
        let children_are_exact = match world.get::<Children>(self.entity) {
            Some(observed_children) => {
                children_component_present
                    && observed_children.len() == children.len()
                    && observed_children
                        .iter()
                        .copied()
                        .eq(children.iter().copied())
            }
            None => !children_component_present && children.is_empty(),
        };
        let registry_is_exact = match (registry.record(self.entity), &self.registry_claim) {
            (Some(record), Some(claim)) => {
                record.matches_claim(claim) && Some(record.manifest()) == registry_manifest
            }
            (None, None) => registry_manifest.is_none(),
            _ => false,
        };
        world.get_entity(self.entity).is_ok() == self.present
            && world.get::<ChildOf>(self.entity).map(ChildOf::parent) == parent
            && children_are_exact
            && registry_is_exact
            && world.get::<RouteOwnershipSeal>(self.entity).copied() == self.ownership_seal
            && manifest_is_exact
            && world.get::<Visibility>(self.entity).copied() == self.visibility
            && world
                .get::<Pickable>(self.entity)
                .map(|pickable| (pickable.should_block_lower, pickable.is_hoverable))
                == self.pickable
            && world.get::<RouterInputBarrier>(self.entity).copied() == self.barrier
            && world.get::<FrameLifecycle>(self.entity).copied() == self.frame_lifecycle
            && world.get::<ChildSlotLifecycle>(self.entity).copied() == self.child_slot_lifecycle
            && world.get::<ChildSlotAuthority>(self.entity) == self.child_slot_authority.as_ref()
            && world.get::<ProvisionalSlotLifecycle>(self.entity).copied()
                == self.provisional_slot_lifecycle
            && world
                .get::<RouterHostRegistration>(self.entity)
                .map(|registration| (registration.router(), registration.incarnation()))
                == self.host_registration
    }

    fn matches_allowing_parent_relationship(
        &self,
        world: &World,
        registry: &RouteEntityRegistry,
    ) -> bool {
        let manifest_is_current = match &self.manifest {
            Some(manifest) => observed_manifest_differs_only_by_allowed_relationships(
                world,
                self.entity,
                manifest,
                false,
                true,
            ),
            None => world.get_entity(self.entity).is_err(),
        };
        self.matches_projection(
            world,
            registry,
            world.get::<ChildOf>(self.entity).map(ChildOf::parent),
            &self.children,
            self.children_component_present,
            self.registry_manifest.as_ref(),
            manifest_is_current,
        )
    }
}

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
pub(crate) struct FailureCleanupSourceProjection {
    host: Option<FailureCleanupHostSourceProjection>,
    active: Option<FailureCleanupActiveSourceProjection>,
    prepared: Option<FailureCleanupActiveSourceProjection>,
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
    ui_detachment: DetachedUiInheritance,
}

/// Bevy's two UI propagation observers remove these derived components when
/// ChildOf is removed, except when the entity supplies its own Propagate source.
/// Freeze only components already present; this never permits runtime additions.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct DetachedUiInheritance {
    removed: [Option<ComponentId>; 2],
}

impl DetachedUiInheritance {
    fn capture(world: &World, entity: Entity) -> Self {
        Self {
            removed: [
                (world
                    .get::<bevy_app::Inherited<ComputedUiTargetCamera>>(entity)
                    .is_some()
                    && world
                        .get::<bevy_app::Propagate<ComputedUiTargetCamera>>(entity)
                        .is_none())
                .then(|| {
                    world
                        .components()
                        .component_id::<bevy_app::Inherited<ComputedUiTargetCamera>>()
                })
                .flatten(),
                (world
                    .get::<bevy_app::Inherited<ComputedUiRenderTargetInfo>>(entity)
                    .is_some()
                    && world
                        .get::<bevy_app::Propagate<ComputedUiRenderTargetInfo>>(entity)
                        .is_none())
                .then(|| {
                    world
                        .components()
                        .component_id::<bevy_app::Inherited<ComputedUiRenderTargetInfo>>()
                })
                .flatten(),
            ],
        }
    }

    fn removes(self, component: ComponentId) -> bool {
        self.removed.contains(&Some(component))
    }

    fn is_applied(self, world: &World, entity: Entity) -> bool {
        self.removed.iter().flatten().all(|component| {
            world
                .get_entity(entity)
                .is_ok_and(|entity| !entity.contains_id(*component))
        })
    }
}

/// A retained Node detached by this cleanup becomes a UI root. The UI plugin
/// may later add these four derived components. This projection is used only by
/// read-only admission; the receipt and every mutation manifest remain exact.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct FrozenRetainedUiRootAdditions {
    component_ids: [Option<ComponentId>; 4],
}

impl FrozenRetainedUiRootAdditions {
    fn capture(world: &World, entity: Entity, projected: &ComponentManifest) -> Self {
        if world.get::<Node>(entity).is_none() {
            return Self::default();
        }
        let mut component_ids = [
            world
                .components()
                .component_id::<bevy_app::Inherited<ComputedUiTargetCamera>>(),
            world
                .components()
                .component_id::<bevy_app::Propagate<ComputedUiTargetCamera>>(),
            world
                .components()
                .component_id::<bevy_app::Inherited<ComputedUiRenderTargetInfo>>(),
            world
                .components()
                .component_id::<bevy_app::Propagate<ComputedUiRenderTargetInfo>>(),
        ];
        for component in &mut component_ids {
            if component.is_some_and(|component| projected.component_ids().contains(&component)) {
                *component = None;
            }
        }
        Self { component_ids }
    }

    fn matches_for_admission(
        self,
        world: &World,
        entity: Entity,
        expected: &ComponentManifest,
    ) -> bool {
        let Some(current) = ComponentManifest::from_world(world, entity) else {
            return false;
        };
        let child_of = world.components().component_id::<ChildOf>();
        let children = world.components().component_id::<Children>();
        let non_relationship = |component: &&ComponentId| {
            Some(**component) != child_of && Some(**component) != children
        };
        current
            .component_ids()
            .iter()
            .filter(non_relationship)
            .filter(|component| !self.component_ids.contains(&Some(**component)))
            .eq(expected.component_ids().iter().filter(non_relationship))
    }
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
    Contained {
        lease: QuarantineLeaseId,
        record: crate::registry::ContainedResidualRecord,
    },
    Quarantine {
        lease: QuarantineLeaseId,
        lease_entities: Vec<Entity>,
        owner: RegistryOwner,
        seal: crate::registry::OwnershipSeal,
        registered_manifest: ComponentManifest,
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
    admission_ui_root_additions: FrozenRetainedUiRootAdditions,
}

/// Receipt-complete physical responsibility. Quarantine tables own the content;
/// this keeps only its exact receipt/table-bound proof, never the old branch.
pub(crate) struct IdleHostCompletedRetention {
    terminals: Vec<PreparedBackNoTargetAuthorityTerminalProjection>,
}

#[derive(Debug)]
pub(crate) struct IdleHostRetainedAdmissionEvidence {
    expected: Vec<PreparedBackNoTargetAuthorityTerminalProjection>,
    observed: Vec<IdleHostRetainedTargetObservation>,
    candidate: Entity,
    closure: Vec<Entity>,
}

#[derive(Debug)]
struct IdleHostRetainedTargetObservation {
    entity: FrozenBackNoTargetAuthorityEntity,
    visibility: Option<Visibility>,
    pickable: Option<Pickable>,
    barrier: Option<RouterInputBarrier>,
    lifecycle: Option<ProvisionalSlotLifecycle>,
    lease: Option<crate::registry::QuarantineLease>,
    unresolved: Option<crate::registry::RouteEntityRecord>,
    tombstone: Option<(ResidualLeaseId, u64, u64, CleanupTargetIdentity)>,
    authority_matches: bool,
}

impl IdleHostRetainedAdmissionEvidence {
    pub(crate) fn expected_digest(&self) -> u64 {
        admission_evidence_digest(&self.expected)
    }
    pub(crate) fn observed_digest(&self) -> u64 {
        admission_evidence_digest(&(&self.observed, self.candidate, &self.closure))
    }

    #[cfg(test)]
    pub(crate) fn observed_visibility(&self, entity: Entity) -> Option<Visibility> {
        self.observed
            .iter()
            .find(|fact| fact.entity.entity == entity)
            .and_then(|fact| fact.visibility)
    }
}

pub(crate) fn admission_evidence_digest(value: &impl std::fmt::Debug) -> u64 {
    use std::{
        fmt::Write,
        hash::{DefaultHasher, Hasher},
    };
    struct DigestWriter(DefaultHasher);
    impl std::fmt::Write for DigestWriter {
        fn write_str(&mut self, text: &str) -> std::fmt::Result {
            self.0.write(text.as_bytes());
            Ok(())
        }
    }
    let mut writer = DigestWriter(DefaultHasher::new());
    write!(&mut writer, "{value:?}").expect("infallible digest writer");
    writer.0.finish().max(1)
}

impl IdleHostCompletedRetention {
    pub(crate) fn admission_failure_evidence(
        &self,
        world: &World,
        candidate: Entity,
    ) -> IdleHostRetainedAdmissionEvidence {
        let registry = world.resource::<RouteEntityRegistry>();
        let table = world.get_resource::<QuarantineLeaseTable>();
        let observed = self
            .terminals
            .iter()
            .map(|terminal| IdleHostRetainedTargetObservation {
                entity: FrozenBackNoTargetAuthorityEntity::observe(
                    world,
                    registry,
                    terminal.entity,
                ),
                visibility: world.get::<Visibility>(terminal.entity).copied(),
                pickable: world.get::<Pickable>(terminal.entity).copied(),
                barrier: world.get::<RouterInputBarrier>(terminal.entity).copied(),
                lifecycle: world
                    .get::<ProvisionalSlotLifecycle>(terminal.entity)
                    .copied(),
                lease: table
                    .and_then(|table| {
                        table.leases().iter().find(|lease| {
                            lease
                                .records()
                                .iter()
                                .any(|record| record.entity() == terminal.entity)
                        })
                    })
                    .cloned(),
                unresolved: table
                    .and_then(|table| table.unresolved_record(terminal.entity))
                    .cloned(),
                tombstone: table
                    .and_then(|table| table.authority_lost_tombstone(terminal.entity))
                    .map(|tombstone| {
                        (
                            tombstone.id(),
                            tombstone.generation(),
                            tombstone.operation_sequence(),
                            tombstone.source_identity().clone(),
                        )
                    }),
                authority_matches: terminal.authority_is_current(world),
            })
            .collect();
        let mut closure = observed_descendant_closure(
            world,
            self.terminals
                .iter()
                .filter(|terminal| world.get_entity(terminal.entity).is_ok())
                .map(|terminal| terminal.entity),
        )
        .into_iter()
        .collect::<Vec<_>>();
        closure.sort_by_key(|entity| entity.to_bits());
        IdleHostRetainedAdmissionEvidence {
            expected: self.terminals.clone(),
            observed,
            candidate,
            closure,
        }
    }
    pub(crate) fn has_live_targets(&self, world: &World) -> bool {
        self.terminals
            .iter()
            .any(|terminal| world.get_entity(terminal.entity).is_ok())
    }

    pub(crate) fn allows_candidate(&self, world: &World, candidate: Entity) -> bool {
        self.terminals.iter().all(|terminal| {
            world.get_entity(terminal.entity).is_err() || terminal.is_current_for_admission(world)
        }) && !observed_descendant_closure(
            world,
            self.terminals
                .iter()
                .filter(|terminal| world.get_entity(terminal.entity).is_ok())
                .map(|terminal| terminal.entity),
        )
        .contains(&candidate)
    }
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
        self.authority_is_current(world)
    }

    // A completed mutation's old child set is not a future admission contract.
    // Retain the original receipt/table manifests while re-observing only the
    // current relationship components and closure for the read-only consumer.
    fn is_current_for_admission(&self, world: &World) -> bool {
        if world
            .resource::<RouteEntityRegistry>()
            .record(self.entity)
            .map(|record| record.claim())
            != self.registry_claim
        {
            return false;
        }
        let presence_is_current = match &self.presence {
            PreparedBackNoTargetAuthorityTerminalPresence::Absent => {
                world.get_entity(self.entity).is_err()
            }
            PreparedBackNoTargetAuthorityTerminalPresence::Retained {
                ownership_seal,
                manifest,
                ..
            } => {
                world.get::<RouteOwnershipSeal>(self.entity).copied() == *ownership_seal
                    && self.admission_ui_root_additions.matches_for_admission(
                        world,
                        self.entity,
                        manifest,
                    )
            }
        };
        presence_is_current && self.authority_is_current(world)
    }

    fn authority_is_current(&self, world: &World) -> bool {
        let Some(table) = world.get_resource::<QuarantineLeaseTable>() else {
            return false;
        };
        match &self.authority {
            PreparedBackNoTargetAuthorityTerminalAuthority::None => {
                table.record(self.entity).is_none()
                    && table.unresolved_record(self.entity).is_none()
                    && table.authority_lost_tombstone(self.entity).is_none()
            }
            PreparedBackNoTargetAuthorityTerminalAuthority::Contained { lease, record } =>
                table.contained_record(self.entity).is_some_and(|(identity, observed)|
                    matches!(identity, crate::registry::ContainedResidualLeaseIdentity::Verified { id, .. } if id == *lease)
                        && observed == record && contained_terminal_scope_is_current(world, &record.terminal_identity)),
            PreparedBackNoTargetAuthorityTerminalAuthority::Quarantine {
                lease,
                lease_entities,
                owner,
                seal,
                registered_manifest,
                manifest,
            } => {
                let Some(record) = table.record(self.entity) else {
                    return false;
                };
                table.lease(*lease).is_some_and(|lease| {
                    lease.owned_entities() == *lease_entities
                        && lease.records().iter().any(|record| {
                            record.entity() == self.entity && record.observed_manifest() == manifest
                        })
                }) && record.owner() == owner
                    && record.seal() == *seal
                    && record.lifecycle() == EntityLifecycle::Quarantined
                    && record.manifest() == registered_manifest
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
                            lost_terminal_scope_is_current(world, projection.source_identity())
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
                    ui_detachment: if parent == Some(host.host) {
                        DetachedUiInheritance::capture(world, host.top_slot)
                    } else {
                        DetachedUiInheritance::default()
                    },
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
                    || projection.entity != obligation.entity()
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
                    || projection.entity != obligation.entity()
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
                    .then_some(obligation.entity())
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
                            obligation.entity() == *child
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
                    && expected
                        .manifest
                        .as_ref()
                        .map_or(observed.manifest.is_none(), |manifest| {
                            observed_manifest_matches_relationship_projection(
                                world,
                                expected.entity,
                                manifest,
                                children_changed,
                                child_of_changed,
                                if child_of_changed {
                                    self.allowed.host_edge.ui_detachment
                                } else {
                                    DetachedUiInheritance::default()
                                },
                            )
                        })
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
    OutletBinding {
        parent: bevy_ui_router_core::frame::RouteFrameVersion,
        binding: crate::presentation::OutletBinding,
    },
}

impl FailedBoundaryExpectation {
    pub(crate) fn exact_claim(&self) -> &ExactRegistryClaim {
        match self {
            Self::ExactClaim(claim) => claim,
            Self::OutletBinding { binding, .. } => binding.source_claim(),
        }
    }

    pub(crate) fn physical_entity(&self) -> Entity {
        match self {
            Self::ExactClaim(claim) => claim.entity(),
            Self::OutletBinding { binding, .. } => binding.anchor(),
        }
    }

    pub(crate) fn matches_outlet_binding(
        &self,
        binding: &crate::presentation::OutletBinding,
    ) -> bool {
        match self {
            Self::ExactClaim(claim) => !binding.is_native() && binding.source_claim() == claim,
            Self::OutletBinding {
                binding: expected, ..
            } => expected == binding,
        }
    }
}

fn observe_failed_outlet_binding(
    world: &World,
    registry: &RouteEntityRegistry,
    source: FailedBoundarySource,
    parent: &bevy_ui_router_core::frame::RouteFrameVersion,
    binding: &crate::presentation::OutletBinding,
) -> Option<FailedBoundaryDrift> {
    if !binding.is_native() {
        return observe_failed_boundary_claim(world, registry, source, binding.source_claim());
    }
    let expected = FailedBoundaryExpectation::OutletBinding {
        parent: parent.clone(),
        binding: binding.clone(),
    };
    if let Some(mut drift) =
        observe_failed_boundary_claim(world, registry, source.clone(), binding.source_claim())
    {
        drift.expected = expected;
        return Some(drift);
    }
    (!binding.is_current(world, parent, true)).then_some(FailedBoundaryDrift {
        source,
        expected,
        state: FailedManifestClaimState::PhysicalDrift,
        predicate: FailedMountParentFreezePredicate::ParentAuthority,
    })
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
    pub(crate) fn observe_idle_host_residual(
        &self,
        world: &World,
        host: &RetiredHostBinding,
    ) -> Option<IdleHostResidualObservation> {
        let registry = world.get_resource::<RouteEntityRegistry>()?;
        let targets = self.obligation_entities();
        let mut roots = targets.clone();
        roots.push(host.host);
        let mut closure = observed_descendant_closure(world, roots.iter().copied())
            .into_iter()
            .collect::<Vec<_>>();
        closure.sort_by_key(|entity| entity.to_bits());
        let inert = self
            .obligations
            .iter()
            .map(|obligation| {
                world.get_entity(obligation.entity()).is_err()
                    || match &obligation.identity {
                        CleanupTargetIdentity::Registered(claim) => {
                            registry
                                .record(obligation.entity())
                                .is_some_and(|record| record.claim() == *claim)
                                && terminal_unresolved_root_is_inert(world, claim, false)
                        }
                        identity @ CleanupTargetIdentity::ContainedContent { .. } => {
                            contained_terminal_scope_is_current(world, identity)
                        }
                    }
            })
            .collect();
        let edge_absent = world.get_entity(host.host).is_err()
            || (world
                .get::<ChildOf>(host.top_slot)
                .is_none_or(|parent| parent.parent() != host.host)
                && !observed_children(world, host.host).contains(&host.top_slot));
        Some(IdleHostResidualObservation {
            input: targets
                .iter()
                .map(|target| crate::input::observe_router_target_input(world, *target))
                .collect(),
            roots,
            targets,
            closure: closure
                .into_iter()
                .map(|entity| FrozenBackNoTargetAuthorityEntity::observe(world, registry, entity))
                .collect(),
            inert,
            edge_absent,
            manifest_digest: self.digest,
            obligation_count: self.obligations.len(),
        })
    }

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

    pub(crate) fn from_retained_partition(
        world: &World,
        registry: &RouteEntityRegistry,
        leaving: &RealizedBranch,
        retained: Option<&RetainedLeafProjection>,
        retiring: &[(&RetainedLeafProjection, &RealizedBranch)],
        projected_claims: &[ExactRegistryClaim],
        successor: Option<&RealizedBranch>,
    ) -> Option<Self> {
        if let Some(candidate) = retained {
            let mut frames = leaving
                .frames()
                .iter()
                .filter(|frame| frame.frame() == candidate.frame());
            let frame = frames.next()?;
            if frames.next().is_some() || !frame.mount_binding().matches_retained_scope(candidate) {
                return None;
            }
        }
        let mut frames = leaving
            .frames()
            .iter()
            .filter(|frame| retained.is_none_or(|candidate| frame.frame() != candidate.frame()))
            .collect::<Vec<_>>();
        let mut slots = leaving
            .child_slots()
            .iter()
            .filter(|slot| retained.is_none_or(|candidate| slot.parent() != candidate.frame()))
            .collect::<Vec<_>>();
        let mut authorized_retired = Vec::with_capacity(retiring.len());
        for (projection, branch) in retiring {
            let [frame] = branch.frames() else {
                return None;
            };
            if frame.frame() != projection.frame()
                || !frame.mount_binding().matches_retained_scope(projection)
                || branch
                    .child_slots()
                    .iter()
                    .any(|slot| slot.parent() != frame.frame())
                || authorized_retired.contains(&projection.frame())
            {
                return None;
            }
            let exact_shells = registry
                .retained_shell_claims_for_mount(
                    world,
                    projection.parent(),
                    projection.outlet(),
                    projection.slot(),
                )
                .ok()?;
            if !frame
                .claims()
                .any(|claim| claim.entity() == frame.shell() && exact_shells.contains(claim))
            {
                return None;
            }
            authorized_retired.push(projection.frame());
            frames.push(frame);
            slots.extend(branch.child_slots());
        }
        Self::from_leaving_frame_refs(
            world,
            registry,
            &frames,
            &slots,
            projected_claims,
            successor,
            &authorized_retired,
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
        let frames = frames.iter().collect::<Vec<_>>();
        let child_slots = child_slots.iter().collect::<Vec<_>>();
        Self::from_leaving_frame_refs(
            world,
            registry,
            &frames,
            &child_slots,
            projected_claims,
            successor,
            &[],
        )
    }

    fn from_leaving_frame_refs(
        world: &World,
        registry: &RouteEntityRegistry,
        frames: &[&RealizedFrame],
        child_slots: &[&MountedChildSlot],
        projected_claims: &[ExactRegistryClaim],
        successor: Option<&RealizedBranch>,
        authorized_retired: &[&RouteFrameVersion],
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
            .flat_map(|frame| frame.claims())
            .map(ExactRegistryClaim::entity)
            .chain(child_slots.iter().map(|slot| slot.slot()))
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
                let children = frames
                    .iter()
                    .enumerate()
                    .filter(|(_, frame)| frame.mount_binding().matches_mounted_child_slot(slot))
                    .collect::<Vec<_>>();
                // Multiple target children require an explicit retained-owner
                // transfer. The ordinary branch proof remains single-child.
                if children
                    .iter()
                    .filter(|(_, frame)| !authorized_retired.contains(&frame.frame()))
                    .count()
                    > 1
                    || children.iter().any(|(child_index, _)| {
                        incoming[*child_index].replace(slot_index).is_some()
                    })
                {
                    return None;
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
                    authorized_retired,
                    root,
                    &mut visit,
                    &mut obligations,
                )?;
            }
            if visit.iter().any(|state| *state != 2) {
                return None;
            }
        }
        #[cfg(feature = "bsn")]
        let obligation_count = obligation_count
            + frames
                .iter()
                .map(|frame| {
                    crate::presentation::native_frame::contained_cleanup_targets(frame).len()
                })
                .sum::<usize>();
        if obligations.len() != obligation_count
            || obligations
                .iter()
                .map(|obligation| obligation.entity())
                .collect::<HashSet<_>>()
                .len()
                != obligation_count
        {
            return None;
        }

        let mut parent_candidates = frames
            .iter()
            .map(|frame| {
                if authorized_retired.contains(&frame.frame()) {
                    retained_frame_mount_parent_candidate(world, frame)
                } else {
                    let successor = exact_successor_for_mount(frame, successor)?;
                    frame_mount_parent_candidate(frame, successor)
                }
            })
            .collect::<Option<Vec<_>>>()?;
        parent_candidates.extend(
            child_slots
                .iter()
                .map(|slot| mounted_anchor_parent_candidate(world, slot))
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
        Self::from_authority_settlement_with_retained(world, registry, pending, active, host, &[])
    }

    pub(crate) fn from_authority_settlement_with_retained(
        world: &World,
        registry: &RouteEntityRegistry,
        pending: &crate::branch_transaction::PreparedBranch,
        active: &RealizedBranch,
        host: &HostBinding,
        retiring: &[(&RetainedLeafProjection, &RealizedBranch)],
    ) -> Result<Self, AuthoritySettlementManifestError> {
        let mut obligations = Vec::new();
        append_exact_authority_branch(
            world,
            registry,
            pending.frames(),
            pending.child_slots(),
            &mut obligations,
        )?;
        append_retained_authority_branches(
            world,
            registry,
            Some(active),
            host,
            retiring,
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

    pub(crate) fn from_back_no_target_authority(
        world: &World,
        registry: &RouteEntityRegistry,
        active: &RealizedBranch,
        host: &HostBinding,
    ) -> Result<Self, AuthoritySettlementManifestError> {
        Self::from_idle_host_authority(world, registry, Some(active), host)
    }

    pub(crate) fn from_idle_host_authority(
        world: &World,
        registry: &RouteEntityRegistry,
        active: Option<&RealizedBranch>,
        host: &HostBinding,
    ) -> Result<Self, AuthoritySettlementManifestError> {
        Self::from_idle_host_authority_with_retained(world, registry, active, host, &[])
    }

    pub(crate) fn from_idle_host_authority_with_retained(
        world: &World,
        registry: &RouteEntityRegistry,
        active: Option<&RealizedBranch>,
        host: &HostBinding,
        retiring: &[(&RetainedLeafProjection, &RealizedBranch)],
    ) -> Result<Self, AuthoritySettlementManifestError> {
        let mut obligations = Vec::new();
        append_retained_authority_branches(
            world,
            registry,
            active,
            host,
            retiring,
            &mut obligations,
        )?;
        if let Some(active) = active {
            append_exact_authority_branch(
                world,
                registry,
                active.frames(),
                active.child_slots(),
                &mut obligations,
            )?;
        }
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

    pub(crate) fn from_target_preparation_permanent(
        world: &World,
        registry: &RouteEntityRegistry,
        splice: &SealedReconciliationSplice,
        projected_claims: &[ExactRegistryClaim],
        host: &HostBinding,
    ) -> Result<Self, AuthoritySettlementManifestError> {
        let projected = projected_claims
            .iter()
            .map(|claim| (claim.entity(), claim))
            .collect::<HashMap<_, _>>();
        if projected.len() != projected_claims.len()
            || host.top_slot_claim.entity() != host.top_slot
            || host.candidate.host() != host.host
        {
            return Err(AuthoritySettlementManifestError::InvalidAuthority);
        }

        let mut source_claims = HashMap::new();
        for claim in splice
            .preserved()
            .claims()
            .chain(splice.leaving().claims())
            .chain(splice.entering().claims())
            .chain(
                splice
                    .slot_roles()
                    .iter()
                    .map(SealedReconciliationSlotRole::claim),
            )
            .chain(std::iter::once(&host.top_slot_claim))
        {
            if source_claims
                .insert(claim.entity(), claim)
                .is_some_and(|previous| previous != claim)
            {
                return Err(AuthoritySettlementManifestError::InvalidAuthority);
            }
        }
        if source_claims.len() != projected.len()
            || source_claims.iter().any(|(entity, source)| {
                projected.get(entity).is_none_or(|projected| {
                    projected.owner() != source.owner() || projected.seal() != source.seal()
                })
            })
        {
            return Err(AuthoritySettlementManifestError::InvalidAuthority);
        }

        let mut obligations = Vec::new();
        for branch in [splice.preserved(), splice.leaving(), splice.entering()] {
            append_projected_target_permanent_branch(
                registry,
                branch,
                projected_claims,
                &mut obligations,
            )?;
        }
        for role in splice.slot_roles() {
            let projected = projected
                .get(&role.entity())
                .ok_or(AuthoritySettlementManifestError::InvalidTopology)?;
            obligations.push(
                target_permanent_slot_role_obligation(registry, role, projected)
                    .ok_or(AuthoritySettlementManifestError::InvalidTopology)?,
            );
        }
        let projected_top_slot = projected
            .get(&host.top_slot)
            .ok_or(AuthoritySettlementManifestError::InvalidAuthority)?;
        obligations.push(
            target_permanent_host_obligation(world, registry, host, projected_top_slot)
                .ok_or(AuthoritySettlementManifestError::InvalidAuthority)?,
        );
        normalize_authority_obligations(&mut obligations)?;
        if obligations
            .iter()
            .filter(|obligation| obligation.identity.registered().is_some())
            .count()
            != projected_claims.len()
            || projected_claims.iter().any(|claim| {
                !obligations
                    .iter()
                    .any(|obligation| obligation.identity.registered() == Some(claim))
            })
            || obligations
                .iter()
                .any(|obligation| obligation.entity() == host.host)
        {
            return Err(AuthoritySettlementManifestError::InvalidTopology);
        }
        order_leaf_to_root(&mut obligations)
            .ok_or(AuthoritySettlementManifestError::InvalidTopology)?;
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

    pub(crate) fn finalize_failed_authority_settlement_with_retained(
        mut self,
        world: &World,
        registry: &RouteEntityRegistry,
        active: Option<&RealizedBranch>,
        host: &HostBinding,
        bindings: Vec<FailedAuthorityBoundaryBinding>,
        retiring: &[(&RetainedLeafProjection, &RealizedBranch)],
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
                    .find(|obligation| {
                        obligation.identity.registered() == Some(&binding.obligation)
                    })
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
        append_retained_authority_branches(
            world,
            registry,
            active,
            host,
            retiring,
            &mut self.obligations,
        )?;
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
                    .any(|obligation| obligation.entity() == parent.removed_child)
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
            .map(|obligation| obligation.entity())
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

    #[cfg(feature = "bsn")]
    pub(crate) fn observe_current_task_outlet(
        &mut self,
        world: &World,
        registry: &RouteEntityRegistry,
        slot: &MountedChildSlot,
    ) -> Result<(), FailedManifestBuildError> {
        let source = FailedBoundarySource::MountedSlotAnchor {
            parent: slot.parent().clone(),
            outlet: slot.outlet().clone(),
            slot: slot.slot_id(),
            removed_child: slot.slot(),
        };
        if let Some(drift) = observe_failed_outlet_binding(
            world,
            registry,
            source,
            slot.parent(),
            slot.anchor_binding(),
        ) {
            self.boundary_drifts
                .try_reserve(1)
                .map_err(|_| FailedManifestBuildError::RetryableCapacity)?;
            self.boundary_drifts.push(drift);
        }
        Ok(())
    }

    pub(crate) fn contains_exact_obligation(&self, claim: &ExactRegistryClaim) -> bool {
        self.obligations
            .iter()
            .any(|obligation| obligation.identity.registered() == Some(claim))
    }

    pub(crate) fn contains_frame_obligation(&self, frame: &RouteFrameVersion) -> bool {
        self.obligations
            .iter()
            .filter_map(|obligation| obligation.identity.registered())
            .any(|claim| {
                matches!(
                    claim.owner(),
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
                .find(|obligation| obligation.identity.registered() == Some(&binding.obligation))
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
            .any(|claim| allowed_writes.contains(&claim.entity()))
            || active_claims.iter().any(|claim| {
                prepared_claims
                    .iter()
                    .any(|prepared| prepared.entity() == claim.entity())
            })
        {
            return None;
        }
        let mut roots = vec![host.host, host.top_slot];
        roots.extend(allowed_writes.iter().copied());
        roots.extend(active_claims.iter().map(ExactRegistryClaim::entity));
        roots.extend(prepared_claims.iter().map(ExactRegistryClaim::entity));
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
                .any(|claim| !protected_entities.contains(&claim.entity()))
        {
            return None;
        }
        let protected_observed = protected_entities
            .iter()
            .copied()
            .map(|entity| FrozenFailureCleanupObservedEntity::observe(world, registry, entity))
            .collect::<Vec<_>>();
        if protected_observed.iter().any(|observed| !observed.present) {
            return None;
        }
        Some(FrozenFailureCleanupPartition {
            source: FrozenFailureCleanupSource {
                manifest_digest: self.digest,
                targets,
                logical: FailureCleanupSourceProjection::observe_with_prepared(
                    Some(host),
                    active,
                    prepared,
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

    #[cfg(test)]
    pub(crate) fn entities(&self) -> Vec<Entity> {
        self.obligations
            .iter()
            .map(|obligation| obligation.entity())
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
    binding: &'a crate::presentation::OutletBinding,
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
                || residual.anchor() != residual.anchor_binding().anchor()
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
                    binding: residual.anchor_binding(),
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
                rejected.canonical_anchor_binding(),
            ) {
                (Some(entity), Some(binding)) if entity == binding.anchor() => {
                    Some(FailedPreparedBoundaryNode {
                        entity,
                        claim: binding.source_claim(),
                        binding,
                    })
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
                if let Some(drift) = observe_failed_outlet_binding(
                    world,
                    registry,
                    FailedBoundarySource::MountedSlotAnchor {
                        parent: slot.parent().clone(),
                        outlet: slot.outlet().clone(),
                        slot: slot.slot_id(),
                        removed_child: slot.slot(),
                    },
                    slot.parent(),
                    slot.anchor_binding(),
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
                    let source = FailedBoundarySource::FramePhysicalParent {
                        frame: frame.frame().clone(),
                        mount_slot: mount.slot(),
                        removed_child: frame.shell(),
                    };
                    let drift = if let Some(binding) = mount.outlet_binding() {
                        let parent = match binding.source_claim().owner() {
                            RegistryOwner::Committed(
                                EntityOwner::FrameContentTarget(frame)
                                | EntityOwner::FrameOutput { frame, .. },
                            ) => frame,
                            _ => {
                                return Err(FailedManifestBuildError::InvalidTopology(
                                    FailedPreparedTopologyError::ConflictingOwner,
                                ));
                            }
                        };
                        observe_failed_outlet_binding(world, registry, source, parent, binding)
                    } else {
                        observe_failed_boundary_claim(world, registry, source, parent_claim)
                    };
                    if let Some(drift) = drift {
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
                if let Some(drift) = observe_failed_outlet_binding(
                    world,
                    registry,
                    source,
                    node.parent,
                    boundary.binding,
                ) {
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
                count
                    .checked_add(frame.frame.obligation_count())
                    .and_then(|count| count.checked_add(contained_frame_target_count(frame.frame)))
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
                if let Some(mut candidate) = mounted_anchor_parent_candidate(world, node.slot) {
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
            physical: mount_slot,
            binding: CleanupParentBinding::Structural {
                upstream: frame.mount_binding().outlet_binding().cloned(),
            },
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

fn contained_frame_target_count(frame: &RealizedFrame) -> usize {
    #[cfg(feature = "bsn")]
    {
        crate::presentation::native_frame::contained_cleanup_targets(frame).len()
    }
    #[cfg(not(feature = "bsn"))]
    {
        let _ = frame;
        0
    }
}

fn append_contained_frame_obligations(
    frame: &RealizedFrame,
    claims: &[ExactRegistryClaim],
    obligations: &mut Vec<CleanupObligation>,
) -> Option<()> {
    #[cfg(feature = "bsn")]
    if let Some(root_entity) = frame.native_root() {
        let targets = crate::presentation::native_frame::contained_cleanup_targets(frame);
        if targets.is_empty() {
            return Some(());
        }
        let root = claims
            .iter()
            .find(|claim| claim.entity() == root_entity)?
            .clone();
        let shell = claims
            .iter()
            .find(|claim| claim.entity() == frame.shell())?
            .clone();
        let source_modes = obligations
            .iter()
            .filter(|obligation| {
                obligation.entity() == root_entity || obligation.entity() == frame.shell()
            })
            .map(|obligation| obligation.terminal_mode)
            .collect::<Vec<_>>();
        let mode = if source_modes.contains(&CleanupTerminalMode::AuthorityLost) {
            CleanupTerminalMode::AuthorityLost
        } else if source_modes.contains(&CleanupTerminalMode::ForceUnresolved) {
            CleanupTerminalMode::ForceUnresolved
        } else if source_modes.contains(&CleanupTerminalMode::AuthorityExact) {
            CleanupTerminalMode::AuthorityExact
        } else {
            CleanupTerminalMode::Normal
        };
        obligations.try_reserve(targets.len()).ok()?;
        for (entity, expected_parent, manifest) in targets {
            obligations.push(CleanupObligation {
                identity: CleanupTargetIdentity::ContainedContent {
                    entity,
                    root: root.clone(),
                    shell: shell.clone(),
                },
                manifest,
                expected_parent,
                unit_root: frame.shell(),
                authorized_parent: frame.top_slot(),
                allow_missing_components: false,
                terminal_mode: mode,
            });
        }
    }
    #[cfg(not(feature = "bsn"))]
    let _ = (frame, claims, obligations);
    Some(())
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
            identity: CleanupTargetIdentity::Registered(claim.clone()),
            manifest,
            expected_parent,
            unit_root: frame.shell(),
            authorized_parent: frame.top_slot(),
            allow_missing_components: absent || !frame.prewrite_complete(),
            terminal_mode,
        });
    }
    append_contained_frame_obligations(
        frame,
        &frame.claims().cloned().collect::<Vec<_>>(),
        &mut obligations,
    )
    .ok_or(FailedManifestBuildError::RetryableCapacity)?;
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
        identity: CleanupTargetIdentity::Registered(slot.slot_claim().clone()),
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
        identity: CleanupTargetIdentity::Registered(residual.claim().clone()),
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
        identity: CleanupTargetIdentity::Registered(slot.slot_claim().clone()),
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
        identity: CleanupTargetIdentity::Registered(host.top_slot_claim.clone()),
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

fn append_projected_target_permanent_branch(
    registry: &RouteEntityRegistry,
    branch: &RealizedBranch,
    projected_claims: &[ExactRegistryClaim],
    obligations: &mut Vec<CleanupObligation>,
) -> Result<(), AuthoritySettlementManifestError> {
    for frame in branch.frames() {
        let frame_projected = frame
            .claims()
            .map(|source| {
                projected_claims
                    .iter()
                    .find(|projected| projected.entity() == source.entity())
                    .cloned()
            })
            .collect::<Option<Vec<_>>>()
            .ok_or(AuthoritySettlementManifestError::InvalidTopology)?;
        let mut frame_obligations = frame_obligations(registry, frame, &frame_projected)
            .ok_or(AuthoritySettlementManifestError::InvalidTopology)?;
        for obligation in &mut frame_obligations {
            obligation.terminal_mode = CleanupTerminalMode::AuthorityExact;
        }
        obligations.extend(frame_obligations);
    }
    for slot in branch.child_slots() {
        let projected = projected_claims
            .iter()
            .find(|projected| projected.entity() == slot.slot())
            .ok_or(AuthoritySettlementManifestError::InvalidTopology)?;
        let mut obligation = mounted_slot_obligation_with_claim(registry, slot, projected)
            .ok_or(AuthoritySettlementManifestError::InvalidTopology)?;
        obligation.terminal_mode = CleanupTerminalMode::AuthorityExact;
        obligations.push(obligation);
    }
    Ok(())
}

fn target_permanent_slot_role_obligation(
    registry: &RouteEntityRegistry,
    role: &SealedReconciliationSlotRole,
    projected: &ExactRegistryClaim,
) -> Option<CleanupObligation> {
    let record = registry.record(role.entity())?;
    let anchor_record = registry.record(role.anchor_claim().entity())?;
    if record.claim() != *role.claim()
        || anchor_record.claim() != *role.anchor_claim()
        || projected.entity() != role.entity()
        || projected.owner() != role.claim().owner()
        || projected.seal() != role.claim().seal()
        || !matches!(
            role.claim().owner(),
            RegistryOwner::Committed(EntityOwner::ChildSlot { parent, outlet, slot })
                | RegistryOwner::Provisional(ProvisionalOwner::ChildSlot {
                    parent,
                    outlet,
                    slot,
                })
                if parent == role.parent()
                    && outlet == role.outlet()
                    && *slot == role.slot()
        )
    {
        return None;
    }
    Some(CleanupObligation {
        identity: CleanupTargetIdentity::Registered(projected.clone()),
        manifest: record.manifest().clone(),
        expected_parent: role.anchor(),
        unit_root: role.entity(),
        authorized_parent: role.anchor(),
        allow_missing_components: false,
        terminal_mode: CleanupTerminalMode::AuthorityExact,
    })
}

fn target_permanent_host_obligation(
    world: &World,
    registry: &RouteEntityRegistry,
    host: &HostBinding,
    projected: &ExactRegistryClaim,
) -> Option<CleanupObligation> {
    let record = registry.record(host.top_slot)?;
    if record.claim() != host.top_slot_claim
        || projected.entity() != host.top_slot
        || projected.owner() != host.top_slot_claim.owner()
        || projected.seal() != host.top_slot_claim.seal()
        || !cleanup_claim_has_exact_authority(world, &host.top_slot_claim)
        || !matches!(
            host.top_slot_claim.owner(),
            RegistryOwner::Committed(EntityOwner::HostTopSlot { host: version, slot })
                if version == &host.version && *slot == host.slot_id
        )
    {
        return None;
    }
    Some(CleanupObligation {
        identity: CleanupTargetIdentity::Registered(projected.clone()),
        manifest: record.manifest().clone(),
        expected_parent: host.host,
        unit_root: host.top_slot,
        authorized_parent: host.host,
        allow_missing_components: false,
        terminal_mode: CleanupTerminalMode::AuthorityExact,
    })
}

fn normalize_authority_obligations(
    obligations: &mut Vec<CleanupObligation>,
) -> Result<(), AuthoritySettlementManifestError> {
    let mut normalized = Vec::new();
    normalized
        .try_reserve_exact(obligations.len())
        .map_err(|_| AuthoritySettlementManifestError::RetryableCapacity)?;
    for obligation in obligations.drain(..) {
        if let Some(existing) = normalized
            .iter()
            .find(|existing: &&CleanupObligation| existing.entity() == obligation.entity())
        {
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

fn leaving_slot_has_exact_parent(frames: &[&RealizedFrame], slot: &MountedChildSlot) -> bool {
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
    frames: &[&RealizedFrame],
    child_slots: &[&MountedChildSlot],
    projected_claims: &[ExactRegistryClaim],
    authorized_retired: &[&RouteFrameVersion],
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
        let children = frames
            .iter()
            .enumerate()
            .filter(|(_, child)| child.mount_binding().matches_mounted_child_slot(slot))
            .collect::<Vec<_>>();
        if children
            .iter()
            .filter(|(_, child)| !authorized_retired.contains(&child.frame()))
            .count()
            > 1
        {
            return None;
        }
        for (child_index, _) in children {
            append_exact_leaving_unit(
                registry,
                frames,
                child_slots,
                projected_claims,
                authorized_retired,
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

// The Core plan selects the complete original cache inventory. These source
// checks bind its physical payloads without requiring a live host or granting
// cleanup authority from a read-only preservation proof.
fn append_retained_authority_branches(
    world: &World,
    registry: &RouteEntityRegistry,
    active: Option<&RealizedBranch>,
    host: &HostBinding,
    retiring: &[(&RetainedLeafProjection, &RealizedBranch)],
    obligations: &mut Vec<CleanupObligation>,
) -> Result<(), AuthoritySettlementManifestError> {
    if retiring.is_empty() {
        return Ok(());
    }
    let mut primary_entities = HashSet::new();
    let primary_count = active
        .map_or(Some(1), |branch| branch.obligation_count().checked_add(1))
        .and_then(|count| {
            retiring.iter().try_fold(count, |count, (_, branch)| {
                count.checked_add(branch.obligation_count())
            })
        })
        .and_then(|count| count.checked_add(obligations.len()))
        .ok_or(AuthoritySettlementManifestError::RetryableCapacity)?;
    primary_entities
        .try_reserve(primary_count)
        .map_err(|_| AuthoritySettlementManifestError::RetryableCapacity)?;
    primary_entities.extend(obligations.iter().map(CleanupObligation::entity));
    primary_entities.insert(host.top_slot);
    if let Some(active) = active {
        primary_entities.extend(active.claims().map(ExactRegistryClaim::entity));
    }
    for (index, (projection, branch)) in retiring.iter().enumerate() {
        let [frame] = branch.frames() else {
            return Err(AuthoritySettlementManifestError::InvalidTopology);
        };
        let active = active.ok_or(AuthoritySettlementManifestError::InvalidTopology)?;
        let mut parents = active
            .frames()
            .iter()
            .filter(|frame| frame.frame() == projection.parent());
        let parent = parents
            .next()
            .ok_or(AuthoritySettlementManifestError::InvalidTopology)?;
        let mut slots = active.child_slots().iter().filter(|slot| {
            slot.parent() == projection.parent()
                && slot.outlet() == projection.outlet()
                && slot.slot_id() == projection.slot()
        });
        let slot = slots
            .next()
            .ok_or(AuthoritySettlementManifestError::InvalidTopology)?;
        let mount = frame.mount_binding();
        if parents.next().is_some()
            || slots.next().is_some()
            || projection.host() != &host.version
            || frame.frame() != projection.frame()
            || !mount.matches_retained_scope(projection)
            || mount.slot() != slot.slot()
            || mount.slot_claim().entity() != slot.slot_claim().entity()
            || mount.slot_claim().owner() != slot.slot_claim().owner()
            || mount.slot_claim().seal() != slot.slot_claim().seal()
            || !matches!(
                mount.slot_claim().lifecycle(),
                EntityLifecycle::Active | EntityLifecycle::PreparedInert
            )
            || !matches!(
                slot.slot_claim().lifecycle(),
                EntityLifecycle::Active | EntityLifecycle::PreparedInert
            )
            || mount.physical_parent().0 != slot.anchor()
            || mount.outlet_binding() != Some(slot.anchor_binding())
            || !parent.outlet(slot.outlet()).is_some_and(|outlet| {
                outlet.anchor() == slot.anchor()
                    && outlet.anchor_claim() == slot.anchor_claim()
                    && parent.claims().any(|claim| claim == slot.anchor_claim())
            })
            || branch
                .child_slots()
                .iter()
                .any(|slot| slot.parent() != frame.frame())
            || branch
                .claims()
                .any(|claim| claim.lifecycle() != EntityLifecycle::Retained)
            || retiring[..index].iter().any(|(prior, _)| {
                prior.id() == projection.id() || prior.frame() == projection.frame()
            })
            || branch
                .claims()
                .any(|claim| !primary_entities.insert(claim.entity()))
        {
            return Err(AuthoritySettlementManifestError::InvalidTopology);
        }
    }
    // Append each cached leaf before its active parent's slot obligation.
    // The ordinary branch validator still rejects multiple owned siblings.
    for (_, branch) in retiring {
        append_exact_authority_branch(
            world,
            registry,
            branch.frames(),
            branch.child_slots(),
            obligations,
        )?;
    }
    Ok(())
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
        identity: CleanupTargetIdentity::Registered(slot.slot_claim().clone()),
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
                identity: CleanupTargetIdentity::Registered(claim.clone()),
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
    append_contained_frame_obligations(
        frame,
        &frame.claims().cloned().collect::<Vec<_>>(),
        &mut obligations,
    )?;
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
                identity: CleanupTargetIdentity::Registered(claim.clone()),
                manifest: record.manifest().clone(),
                expected_parent,
                unit_root: frame.shell(),
                authorized_parent: frame.top_slot(),
                allow_missing_components: !frame.prewrite_complete(),
                terminal_mode: CleanupTerminalMode::AuthorityExact,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    append_contained_frame_obligations(
        frame,
        &frame.claims().cloned().collect::<Vec<_>>(),
        &mut obligations,
    )?;
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
                identity: CleanupTargetIdentity::Registered(projected.clone()),
                manifest: record.manifest().clone(),
                expected_parent,
                unit_root: frame.shell(),
                authorized_parent: frame.top_slot(),
                allow_missing_components: !frame.prewrite_complete(),
                terminal_mode: CleanupTerminalMode::Normal,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    append_contained_frame_obligations(frame, projected_claims, &mut obligations)?;
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
        physical: frame.mount_binding().slot(),
        binding: CleanupParentBinding::Structural {
            upstream: frame.mount_binding().outlet_binding().cloned(),
        },
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

fn retained_frame_mount_parent_candidate(
    world: &World,
    frame: &RealizedFrame,
) -> Option<CleanupMountParentCandidate> {
    // The caller admits only leaves with an exact current retained proof.
    // Their shared slot can have acquired other siblings since first mount.
    let slot = frame.mount_binding().slot();
    let children_before = observed_children(world, slot);
    if !frame.shell_is_attached()
        || world.get::<ChildOf>(frame.shell()).map(ChildOf::parent) != Some(slot)
        || children_before
            .iter()
            .filter(|child| **child == frame.shell())
            .count()
            != 1
    {
        return None;
    }
    let children_after = children_before
        .iter()
        .copied()
        .filter(|child| *child != frame.shell())
        .collect();
    let (expected_parent, expected_parent_claim) = frame.mount_binding().physical_parent();
    Some(CleanupMountParentCandidate {
        physical: slot,
        binding: CleanupParentBinding::Structural {
            upstream: frame.mount_binding().outlet_binding().cloned(),
        },
        claim: frame.mount_binding().slot_claim().clone(),
        failed_claim_source: None,
        supplied_parent: Some((expected_parent, expected_parent_claim.cloned())),
        failed_parent_source: None,
        removed_child: frame.shell(),
        removed_child_was_attached: true,
        children_before,
        children_after,
    })
}

fn mounted_anchor_parent_candidate(
    world: &World,
    slot: &MountedChildSlot,
) -> Option<CleanupMountParentCandidate> {
    let children_before = if slot.anchor_binding().is_native() {
        observed_children(world, slot.anchor())
    } else {
        slot.expected_anchor_children().to_vec()
    };
    if children_before
        .iter()
        .filter(|child| **child == slot.slot())
        .count()
        != 1
    {
        return None;
    }
    let children_after = children_before
        .iter()
        .copied()
        .filter(|child| *child != slot.slot())
        .collect();
    Some(CleanupMountParentCandidate {
        physical: slot.anchor(),
        binding: CleanupParentBinding::Outlet(slot.anchor_binding().clone()),
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
    let removed_child_was_attached =
        world.get::<ChildOf>(slot.entity()).map(ChildOf::parent) == Some(slot.anchor());
    let mut children_before = if slot.anchor_binding().is_native() {
        observed_children(world, slot.anchor())
    } else {
        slot.prior_anchor_children().to_vec()
    };
    if removed_child_was_attached && !slot.anchor_binding().is_native() {
        children_before.push(slot.entity());
    }
    let children_after = if slot.anchor_binding().is_native() {
        children_before
            .iter()
            .copied()
            .filter(|child| *child != slot.entity())
            .collect()
    } else {
        slot.prior_anchor_children().to_vec()
    };
    CleanupMountParentCandidate {
        physical: slot.anchor(),
        binding: CleanupParentBinding::Outlet(slot.anchor_binding().clone()),
        claim: slot.anchor_claim().clone(),
        failed_claim_source: None,
        supplied_parent: None,
        failed_parent_source: None,
        removed_child: slot.entity(),
        removed_child_was_attached,
        children_before,
        children_after,
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
    let native = rejected
        .canonical_anchor_binding()
        .is_some_and(|binding| binding.is_native());
    let mut children_before = if native {
        observed_children(world, anchor)
    } else {
        children_after.to_vec()
    };
    if attached && !native {
        children_before.push(slot);
    }
    let children_after = if native {
        children_before
            .iter()
            .copied()
            .filter(|child| *child != slot)
            .collect()
    } else {
        children_after.to_vec()
    };
    Ok(Some(CleanupMountParentCandidate {
        physical: anchor,
        binding: CleanupParentBinding::Outlet(
            rejected
                .canonical_anchor_binding()
                .expect("canonical source and binding are retained together")
                .clone(),
        ),
        claim: claim.clone(),
        failed_claim_source: None,
        supplied_parent: None,
        failed_parent_source: None,
        removed_child: slot,
        removed_child_was_attached: attached,
        children_before,
        children_after,
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
        .map(|obligation| obligation.entity())
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
        let entity = candidate.physical;
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
        let native_anchor = matches!(&candidate.binding, CleanupParentBinding::Outlet(binding) if binding.is_native());
        if !native_anchor
            && !matches!(
                claim.owner(),
                RegistryOwner::Committed(
                    EntityOwner::HostTopSlot { .. }
                        | EntityOwner::ChildSlot { .. }
                        | EntityOwner::FrameOutput { .. }
                )
            )
        {
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
        if let Some(existing) = parents.iter().find(|parent| parent.physical == entity) {
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
        let Some(record) = registry.record(claim.entity()) else {
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
            || !native_anchor
                && !manifest_contains_required_allowing_children(
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
        let native_authority = match &candidate.binding {
            CleanupParentBinding::Outlet(binding) if binding.is_native() => world
                .get::<ChildOf>(entity)
                .map(|edge| (edge.parent(), None)),
            CleanupParentBinding::Structural {
                upstream: Some(binding),
            } if binding.is_native() => candidate.supplied_parent.clone(),
            _ => {
                resolve_mount_parent_authority(registry, &claim, candidate.supplied_parent.clone())
            }
        };
        let Some((expected_parent, expected_parent_claim)) = native_authority else {
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
            physical: entity,
            binding: candidate.binding,
            claim,
            manifest: if entity == record.entity() {
                record.manifest().clone()
            } else {
                physical_manifest.clone()
            },
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
    parents.sort_by_key(|parent| parent.physical.to_bits());
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
            if let CleanupParentBinding::Outlet(binding) = &mut parent.binding {
                *binding.source_claim_mut() = projected.clone();
            }
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
            if let CleanupParentBinding::Structural {
                upstream: Some(binding),
            } = &mut parent.binding
            {
                *binding.source_claim_mut() = projected.clone();
            }
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
    mount_parent_physical_matches_with_attachment(
        world,
        registry,
        parent,
        parent.removed_child_was_attached,
    )
}

fn mount_parent_physical_matches_after_detachment(
    world: &World,
    registry: &RouteEntityRegistry,
    parent: &CleanupMountParentSettlement,
) -> bool {
    mount_parent_physical_matches_with_attachment(world, registry, parent, false)
}

fn mount_parent_physical_matches_with_attachment(
    world: &World,
    registry: &RouteEntityRegistry,
    parent: &CleanupMountParentSettlement,
    removed_child_was_attached: bool,
) -> bool {
    if let CleanupParentBinding::Outlet(binding) = &parent.binding
        && binding.is_native()
    {
        let RegistryOwner::Committed(EntityOwner::FrameContentTarget(frame)) = parent.claim.owner()
        else {
            return false;
        };
        let expected_children =
            if removed_child_was_attached && world.get_entity(parent.removed_child).is_ok() {
                &parent.children_before
            } else {
                &parent.children_after
            };
        return binding.anchor() == parent.physical
            && binding.source_claim() == &parent.claim
            && binding.is_current(world, frame, false)
            && !matches!(
                parent.claim.lifecycle(),
                EntityLifecycle::Leaving | EntityLifecycle::Quarantined
            )
            && world.get::<ChildOf>(parent.physical).map(ChildOf::parent)
                == Some(parent.expected_parent)
            && observed_parent_manifest_differs_only_by_children(
                world,
                parent.physical,
                &parent.physical_manifest,
            )
            && observed_children(world, parent.physical) == *expected_children;
    }
    let Some(record) = registry.record(parent.physical) else {
        return false;
    };
    if record.claim() != parent.claim
        || world.get::<RouteOwnershipSeal>(parent.physical)
            != Some(&RouteOwnershipSeal::new(parent.claim.seal()))
        || world.get::<ChildOf>(parent.physical).map(ChildOf::parent)
            != Some(parent.expected_parent)
    {
        return false;
    }
    if !observed_parent_manifest_differs_only_by_children(
        world,
        parent.physical,
        &parent.physical_manifest,
    ) {
        return false;
    }
    let owner_value_is_exact = match parent.claim.owner() {
        RegistryOwner::Committed(EntityOwner::HostTopSlot { .. }) => {
            record.lifecycle() == EntityLifecycle::PreparedInert
                && world.get::<ProvisionalSlotLifecycle>(parent.physical)
                    == Some(&ProvisionalSlotLifecycle::PreparedInert)
                && mount_slot_values_match(world, parent.physical, Visibility::Visible)
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
                EntityLifecycle::Retained => (ChildSlotLifecycle::Retained, Visibility::Hidden),
                EntityLifecycle::Leaving => (ChildSlotLifecycle::Leaving, Visibility::Hidden),
                EntityLifecycle::Quarantined => return false,
            };
            world.get::<ChildSlotLifecycle>(parent.physical) == Some(&expected_lifecycle)
                && world
                    .get::<ChildSlotAuthority>(parent.physical)
                    .is_some_and(|authority| {
                        authority.parent() == frame
                            && authority.outlet() == outlet
                            && authority.slot() == *slot
                    })
                && mount_slot_values_match(world, parent.physical, visibility)
        }
        RegistryOwner::Committed(EntityOwner::FrameOutput {
            frame, local_index, ..
        }) => {
            world.get::<FrameLifecycle>(parent.physical)
                == Some(&FrameLifecycle(parent.claim.lifecycle()))
                && world
                    .get::<FrameOutput>(parent.physical)
                    .is_some_and(|output| output.matches_owner(frame, *local_index))
                && world
                    .get::<FrameOutput>(parent.physical)
                    .is_some_and(|output| {
                        output.canonical_values_match(world, parent.physical, record.lifecycle())
                    })
        }
        _ => false,
    };
    if !owner_value_is_exact {
        return false;
    }
    let native_upstream = if let CleanupParentBinding::Structural {
        upstream: Some(binding),
    } = &parent.binding
    {
        binding.is_native().then_some(binding)
    } else {
        None
    };
    if let Some(binding) = native_upstream {
        let RegistryOwner::Committed(EntityOwner::ChildSlot { parent: frame, .. }) =
            parent.claim.owner()
        else {
            return false;
        };
        if binding.anchor() != parent.expected_parent
            || parent.expected_parent_claim.as_ref() != Some(binding.source_claim())
            || !binding.is_current(world, frame, false)
        {
            return false;
        }
    } else if let Some(expected_parent_claim) = &parent.expected_parent_claim {
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
        if removed_child_was_attached && world.get_entity(parent.removed_child).is_ok() {
            &parent.children_before
        } else {
            &parent.children_after
        };
    match world.get::<Children>(parent.physical) {
        Some(children) => {
            children.len() == expected_children.len()
                && children
                    .iter()
                    .copied()
                    .eq(expected_children.iter().copied())
        }
        None => expected_children.is_empty(),
    }
}

fn observed_parent_manifest_differs_only_by_children(
    world: &World,
    entity: Entity,
    frozen: &ComponentManifest,
) -> bool {
    observed_manifest_differs_only_by_allowed_relationships(world, entity, frozen, true, false)
}

fn observed_manifest_differs_only_by_allowed_relationships(
    world: &World,
    entity: Entity,
    frozen: &ComponentManifest,
    allow_children: bool,
    allow_child_of: bool,
) -> bool {
    observed_manifest_matches_relationship_projection(
        world,
        entity,
        frozen,
        allow_children,
        allow_child_of,
        DetachedUiInheritance::default(),
    )
}

fn observed_manifest_matches_relationship_projection(
    world: &World,
    entity: Entity,
    frozen: &ComponentManifest,
    allow_children: bool,
    allow_child_of: bool,
    ui_detachment: DetachedUiInheritance,
) -> bool {
    if !ui_detachment.is_applied(world, entity)
        || ui_detachment
            .removed
            .iter()
            .flatten()
            .any(|component| !frozen.component_ids().contains(component))
    {
        return false;
    }
    let Ok(entity) = world.get_entity(entity) else {
        return false;
    };
    let actual = ComponentManifest::observed_component_ids(world, entity.archetype().components());
    let children = world.components().component_id::<Children>();
    let child_of = world.components().component_id::<ChildOf>();
    let allowed_relationship = |component| {
        (allow_children && Some(component) == children)
            || (allow_child_of && Some(component) == child_of)
    };
    actual
        .clone()
        .filter(|component| !frozen.component_ids().contains(component))
        .all(allowed_relationship)
        && frozen
            .component_ids()
            .iter()
            .copied()
            .filter(|component| !actual.clone().any(|actual| actual == *component))
            .all(|component| allowed_relationship(component) || ui_detachment.removes(component))
}

fn freeze_failure_cleanup_closure(
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

    fn refreeze(
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
    parent.bookkeeping_claim().is_none_or(|claim| {
        registry
            .record(parent.physical)
            .is_some_and(|record| record.claim() == *claim && record.manifest() == &parent.manifest)
    })
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
    let anchor_record = registry.record(slot.anchor_claim().entity())?;
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
        identity: CleanupTargetIdentity::Registered(projected.clone()),
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
        .map(|obligation| obligation.entity())
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
                .find(|candidate| candidate.entity() == parent)?
                .expected_parent;
        }
        depths.push((obligation.entity(), depth));
    }
    obligations.sort_by_key(|obligation| {
        let depth = depths
            .iter()
            .find_map(|(entity, depth)| (*entity == obligation.entity()).then_some(*depth))
            .unwrap_or_default();
        (std::cmp::Reverse(depth), obligation.entity().to_bits())
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
    retired_identity: CleanupTargetIdentity,
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

    pub(crate) fn source_identity(&self) -> &CleanupTargetIdentity {
        &self.retired_identity
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
        ui_detachment: DetachedUiInheritance,
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
        let Self::Existing {
            index,
            detach,
            ui_detachment,
            ..
        } = self
        else {
            return Some(projected);
        };
        if *detach {
            let (
                _,
                CleanupEvidence::Exact {
                    actual_parent,
                    normalized_manifest,
                    ..
                },
            ) = projected
                .evidence
                .iter_mut()
                .find(|(candidate, _)| *candidate == *index)?
            else {
                return None;
            };
            *actual_parent = None;
            *normalized_manifest = ComponentManifest::new(
                normalized_manifest
                    .component_ids()
                    .iter()
                    .copied()
                    .filter(|component| !ui_detachment.removes(*component)),
            );
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
                ui_detachment,
                ..
            } => {
                if !*detach {
                    return self.postwrite_matches(world);
                }
                if world.get::<ChildOf>(*top_slot).map(ChildOf::parent) != Some(*host)
                    || !children_match(world, *host, children_before)
                    || !cleanup_claim_has_exact_authority(world, claim)
                    || DetachedUiInheritance::capture(world, *top_slot) != *ui_detachment
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
                ui_detachment,
                ..
            } => {
                world.get_entity(*host).is_ok()
                    && world.get::<ChildOf>(*top_slot).map(ChildOf::parent) != Some(*host)
                    && children_match(world, *host, children_after)
                    && (!*require_exact || cleanup_claim_has_exact_authority(world, claim))
                    && ui_detachment.is_applied(world, *top_slot)
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

    fn ui_detachment_for(&self, entity: Entity) -> DetachedUiInheritance {
        match self {
            Self::Existing {
                top_slot,
                detach: true,
                ui_detachment,
                ..
            } if *top_slot == entity => *ui_detachment,
            _ => DetachedUiInheritance::default(),
        }
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
    fn prepare(
        world: &World,
        claims: Vec<ExactRegistryClaim>,
    ) -> Result<Self, AuthorityCleanupFailureKind> {
        if !claims
            .iter()
            .all(|claim| cleanup_claim_is_inert_and_exact(world, claim, true))
        {
            return Err(AuthorityCleanupFailureKind::Preparation);
        }
        let mut missing = Vec::new();
        missing
            .try_reserve_exact(authority_preparation_capacity_input(
                AuthorityPreparationCapacitySite::PickableRepair,
                claims.len(),
            ))
            .map_err(|_| AuthorityCleanupFailureKind::RetryableCapacity)?;
        missing.extend(claims.iter().filter_map(|claim| {
            world
                .get::<Pickable>(claim.entity())
                .is_none()
                .then_some(claim.entity())
        }));
        Ok(Self { claims, missing })
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
    manifest_before: Option<ComponentManifest>,
    parent_detached: bool,
    ui_detachment: DetachedUiInheritance,
}

impl PreparedAuthorityRemoval {
    fn is_current(
        &self,
        world: &World,
        obligations: &[CleanupObligation],
        completed_removals: &[Entity],
    ) -> bool {
        let Some(obligation) = obligations.get(self.index) else {
            return false;
        };
        let entity = obligation.entity();
        obligation.identity.registered().is_none_or(|claim| {
            world
                .resource::<RouteEntityRegistry>()
                .record(entity)
                .is_some_and(|record| record.claim() == *claim)
        }) && world.get::<ChildOf>(entity).map(ChildOf::parent) == self.expected_parent
            && children_match_after_removals(
                world,
                entity,
                &self.children_before,
                completed_removals,
            )
            && cleanup_obligation_has_exact_authority(world, obligation)
            && self.manifest_before.as_ref().is_some_and(|manifest| {
                observed_manifest_matches_relationship_projection(
                    world,
                    entity,
                    manifest,
                    self.children_before
                        .iter()
                        .any(|child| completed_removals.contains(child)),
                    self.parent_detached,
                    self.ui_detachment,
                )
            })
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
    frozen.matches_without_materialization(world, entity)
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
    failure_cleanup_partition: Option<FrozenFailureCleanupPartition>,
    manifest_digest: u64,
    receipts: Vec<Option<CleanupReceipt>>,
    ledger_domain: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PreparedCleanupLedgerProjection {
    ledger_domain: u64,
    manifest_digest: u64,
    obligation_count: usize,
    receipt_count: usize,
}

impl PreparedCleanupLedgerProjection {
    pub(crate) const fn manifest_digest(self) -> u64 {
        self.manifest_digest
    }

    pub(crate) const fn obligation_count(self) -> usize {
        self.obligation_count
    }

    pub(crate) const fn receipt_count(self) -> usize {
        self.receipt_count
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
            receipt_count: self.receipts.iter().flatten().count(),
        }
    }

    pub(crate) fn bind_batch(mut self, batch: CleanupBatch) -> Self {
        self.batch = Some(batch);
        self
    }

    pub(crate) fn bind_failure_cleanup_partition(
        mut self,
        partition: FrozenFailureCleanupPartition,
    ) -> Self {
        debug_assert!(self.failure_cleanup_partition.is_none());
        self.failure_cleanup_partition = Some(partition);
        self
    }
}

enum CleanupStopReason {
    Refreeze(FailureCleanupSettlementRefreezeEvidence),
    Settlement(CleanupSettlementPermanentEvidence),
    AttemptRevisionExhausted,
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
    related_protected: Option<Vec<CleanupProtectedEntity>>,
    post_settlement_focus: Vec<PostSettlementFocusRoot>,
    authority_host_children: Option<Vec<Entity>>,
    back_no_target_authority_partition: Option<FrozenBackNoTargetAuthorityPartition>,
    failure_cleanup_partition: Option<FrozenFailureCleanupPartition>,
    stopped: Option<CleanupStopReason>,
    quarantine_detachment: Option<FrozenCleanupQuarantineDetachment>,
    cleanup_settlement_permanent_pending: Option<Box<CleanupSettlementPermanentCause>>,
}

// Operation-local proof of the related protected partition. Ordinary authored
// components are not ownership; source identity, topology and input facts are.
#[derive(Clone, Debug)]
struct CleanupProtectedEntity {
    observed: FrozenFailureCleanupObservedEntity,
    display: Option<bevy_ui::Display>,
}

impl CleanupLedger {
    fn prepare_related_protected(&mut self, world: &World) -> bool {
        if self.related_protected.is_some()
            || !self
                .obligations
                .iter()
                .any(|o| o.identity.registered().is_none())
        {
            return true;
        }
        let mut roots = HashSet::new();
        for obligation in &self.obligations {
            let mut root = obligation.unit_root;
            let mut seen = HashSet::new();
            while let Some(parent) = world.get::<ChildOf>(root).map(ChildOf::parent) {
                if !seen.insert(root) {
                    return false;
                }
                root = parent;
            }
            roots.insert(root);
        }
        let targets = self
            .obligations
            .iter()
            .map(CleanupObligation::entity)
            .collect::<HashSet<_>>();
        let registry = world.resource::<RouteEntityRegistry>();
        self.related_protected = Some(
            observed_descendant_closure(world, roots.into_iter())
                .into_iter()
                .filter(|entity| !targets.contains(entity))
                .map(|entity| CleanupProtectedEntity {
                    observed: FrozenFailureCleanupObservedEntity::observe(world, registry, entity),
                    display: world.get::<bevy_ui::Node>(entity).map(|node| node.display),
                })
                .collect(),
        );
        true
    }

    fn related_protected_is_current(&self, world: &World) -> bool {
        let Some(protected) = &self.related_protected else {
            return true;
        };
        let registry = world.resource::<RouteEntityRegistry>();
        let removed = self
            .obligations
            .iter()
            .enumerate()
            .filter_map(|(index, obligation)| {
                self.receipts[index]
                    .as_ref()
                    .is_some_and(|receipt| {
                        matches!(
                            receipt.disposition,
                            CleanupDisposition::Removed
                                | CleanupDisposition::RemovedDetached
                                | CleanupDisposition::AlreadyAbsent
                        )
                    })
                    .then_some(obligation.entity())
            })
            .collect::<HashSet<_>>();
        protected.iter().all(|fact| {
            let expected = &fact.observed;
            let entity = expected.entity;
            world.get_entity(entity).is_ok() == expected.present
                && world.get::<ChildOf>(entity).map(ChildOf::parent) == expected.parent
                && world
                    .get::<Children>(entity)
                    .into_iter()
                    .flat_map(|children| children.iter().copied())
                    .eq(expected
                        .children
                        .iter()
                        .filter(|entity| !removed.contains(entity))
                        .copied())
                && registry.record(entity).map(|record| record.claim()) == expected.registry_claim
                && world.get::<RouteOwnershipSeal>(entity).copied() == expected.ownership_seal
                && world.get::<Visibility>(entity).copied() == expected.visibility
                && world
                    .get::<Pickable>(entity)
                    .map(|pickable| (pickable.should_block_lower, pickable.is_hoverable))
                    == expected.pickable
                && world.get::<RouterInputBarrier>(entity).copied() == expected.barrier
                && world.get::<FrameLifecycle>(entity).copied() == expected.frame_lifecycle
                && world.get::<ChildSlotLifecycle>(entity).copied() == expected.child_slot_lifecycle
                && world.get::<ChildSlotAuthority>(entity) == expected.child_slot_authority.as_ref()
                && world.get::<ProvisionalSlotLifecycle>(entity).copied()
                    == expected.provisional_slot_lifecycle
                && world
                    .get::<RouterHostRegistration>(entity)
                    .map(|registration| (registration.router(), registration.incarnation()))
                    == expected.host_registration
                && world.get::<bevy_ui::Node>(entity).map(|node| node.display) == fact.display
        })
    }

    fn retain_related_protected_drift(&mut self, world: &World) {
        let Some(expected) = &self.related_protected else {
            return;
        };
        self.cleanup_settlement_permanent_pending = Some(Box::new(
            CleanupSettlementPermanentCause::RelatedProtectedPartition {
                expected: expected.clone(),
                targets: self
                    .obligations
                    .iter()
                    .map(CleanupObligation::entity)
                    .collect(),
            },
        ));
        self.retain_cleanup_settlement_permanent_evidence(world);
    }

    fn current_focus_closure(&self, world: &World, closure: &[Entity]) -> Vec<Entity> {
        let escaped = self
            .obligations
            .iter()
            .filter_map(|obligation| match &obligation.identity {
                CleanupTargetIdentity::ContainedContent { entity, root, .. }
                    if !contained_target_is_inside(world, *entity, root.entity()) =>
                {
                    Some(*entity)
                }
                _ => None,
            })
            .collect::<HashSet<_>>();
        closure
            .iter()
            .copied()
            .filter(|entity| !escaped.contains(entity))
            .collect()
    }
}

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

pub(crate) struct TargetPreparationPermanentHostEdge<'a> {
    host: Entity,
    top_slot: Entity,
    top_slot_claim: &'a ExactRegistryClaim,
    children_before: &'a [Entity],
    children_after: &'a [Entity],
}

impl<'a> TargetPreparationPermanentHostEdge<'a> {
    pub(crate) fn new(
        host: Entity,
        top_slot: Entity,
        top_slot_claim: &'a ExactRegistryClaim,
        children_before: &'a [Entity],
        children_after: &'a [Entity],
    ) -> Self {
        Self {
            host,
            top_slot,
            top_slot_claim,
            children_before,
            children_after,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TargetPreparationPermanentTransitionStage {
    HostEdgeDetached,
    PhysicalAbsent(Entity),
    ReceiptCommitted(Entity),
}

#[derive(Clone, Copy)]
pub(crate) struct TargetPreparationPermanentTransition<'a> {
    stage: TargetPreparationPermanentTransitionStage,
    settled: &'a [Entity],
    in_flight: Option<Entity>,
}

impl<'a> TargetPreparationPermanentTransition<'a> {
    pub(crate) const fn stage(self) -> TargetPreparationPermanentTransitionStage {
        self.stage
    }

    pub(crate) const fn settled(self) -> &'a [Entity] {
        self.settled
    }

    pub(crate) const fn in_flight(self) -> Option<Entity> {
        self.in_flight
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TargetPreparationPermanentSettlementSource {
    ActiveIsolated,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum TargetPreparationPermanentSettlementFailureStage {
    LedgerPreflight,
    AttemptPreflight,
    HostEdgePrewrite,
    FocusPrewrite,
    CapacityPreparation,
    ObligationPreflight { index: usize, entity: Entity },
    ReceiptPreparation { index: usize, entity: Entity },
    RegistryRetirementPreparation { index: usize, entity: Entity },
    RemovalPrewrite { index: usize, entity: Entity },
    Completion,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum TargetPreparationPermanentSettlementExpectation {
    FreshLedger {
        manifest_digest: u64,
        obligation_count: usize,
    },
    IssuedAttempt(CleanupAttempt),
    HostEdgeBefore {
        host: Entity,
        top_slot: Entity,
        top_slot_claim: ExactRegistryClaim,
        host_children: Vec<Entity>,
        top_slot_parent: Option<Entity>,
    },
    FocusReleased {
        targets: Vec<Entity>,
    },
    PreparedCapacity {
        requested: usize,
    },
    ExactObligation {
        index: usize,
        identity: CleanupTargetIdentity,
        expected_parent: Entity,
    },
    VacantReceiptSlot {
        index: usize,
        identity: CleanupTargetIdentity,
    },
    ExactRegistryRetirement {
        index: usize,
        identity: CleanupTargetIdentity,
    },
    RemovableObligation {
        index: usize,
        identity: CleanupTargetIdentity,
        expected_parent: Option<Entity>,
        expected_children: Vec<Entity>,
    },
    FullySettled {
        manifest_digest: u64,
        obligation_count: usize,
        host: Entity,
        host_children_after: Vec<Entity>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum TargetPreparationPermanentSettlementObservation {
    Ledger {
        owner_present: bool,
        normal_batch_present: bool,
        back_no_target_batch_present: bool,
        back_no_target_capacity_present: bool,
        mount_parent_count: usize,
        receipt_slot_count: usize,
        receipt_count: usize,
        completion_digest: Option<u64>,
        authority_host_children_present: bool,
        back_no_target_authority_partition_present: bool,
    },
    AttemptUnavailable {
        next_attempt: u64,
        completion_digest: Option<u64>,
    },
    Attempt {
        issued: Option<CleanupAttempt>,
        observed: CleanupAttempt,
    },
    HostEdge {
        host_exists: bool,
        top_slot_claim_count: usize,
        host_obligation_count: usize,
        host_children: Vec<Entity>,
        top_slot_parent: Option<Entity>,
    },
    Focus {
        remaining: Vec<Entity>,
    },
    CapacityRejected {
        requested: usize,
    },
    Obligation {
        evidence: Option<CleanupEvidence>,
        authority_exact: bool,
    },
    ReceiptSlot {
        existing: Option<(u64, u64, usize, CleanupDisposition, u64, bool)>,
    },
    RegistryRetirementRejected {
        observed_claim: Option<ExactRegistryClaim>,
    },
    Removal {
        entity_exists: bool,
        observed_claim: Option<ExactRegistryClaim>,
        parent: Option<Entity>,
        children: Vec<Entity>,
        authority_exact: bool,
    },
    Completion {
        remaining_focus: Vec<Entity>,
        host_children: Vec<Entity>,
        receipt_count: usize,
        missing_receipts: Vec<usize>,
        completion_digest: Option<u64>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TargetPreparationPermanentSettlementBoundaryEvidence {
    source: TargetPreparationPermanentSettlementSource,
    stage: TargetPreparationPermanentSettlementFailureStage,
    expected: TargetPreparationPermanentSettlementExpectation,
    observed: TargetPreparationPermanentSettlementObservation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TargetPreparationPermanentReceiptObservationEvidence {
    manifest_digest: u64,
    obligation_count: usize,
    receipt_count: usize,
    unobserved: Vec<(usize, CleanupDisposition)>,
}

pub(crate) enum TargetPreparationPermanentSettlementError<Evidence> {
    AttemptUnavailable(TargetPreparationPermanentSettlementBoundaryEvidence),
    ReceiptPreparationRejected(TargetPreparationPermanentSettlementBoundaryEvidence),
    SettlementRejected(TargetPreparationPermanentSettlementBoundaryEvidence),
    PostWriteRefreeze(Evidence),
}

pub(crate) struct TargetPreparationPermanentCompletionDomain {
    ledger_domain: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TargetPreparationPermanentCompletionPreparationSource {
    ActiveIsolated,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TargetPreparationPermanentCompletionPreparationExpectation {
    ledger_domain: u64,
    manifest_digest: u64,
    obligation_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TargetPreparationPermanentCompletionPreparationObservation {
    marker_domain: Option<u64>,
    ledger_domain: u64,
    owner_present: bool,
    normal_batch_present: bool,
    back_no_target_batch_present: bool,
    back_no_target_capacity_present: bool,
    mount_parent_count: usize,
    receipt_slot_count: usize,
    observed_receipt_count: usize,
    unobserved_receipt_count: usize,
    issued_attempt_present: bool,
    completion_digest: Option<u64>,
    authority_host_children_present: bool,
    back_no_target_authority_partition_present: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TargetPreparationPermanentCompletionPreparationEvidence {
    source: TargetPreparationPermanentCompletionPreparationSource,
    expected: TargetPreparationPermanentCompletionPreparationExpectation,
    observed: TargetPreparationPermanentCompletionPreparationObservation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TargetPreparationPermanentCompletionFacts {
    manifest_digest: u64,
    obligation_count: usize,
    completion_digest: u64,
    settled_count: usize,
}

impl TargetPreparationPermanentCompletionFacts {
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

struct PreparedTargetPreparationPermanentRemoval {
    index: usize,
    entity: Entity,
    expected_parent: Option<Entity>,
    children_before: Vec<Entity>,
    retirement: Option<PreparedExactRegistryRetirement>,
    receipt: PreparedCleanupReceiptBatch,
}

// This plan retains expectations, never a World-validity result. Every physical
// write is followed by a fresh comparison of its complete remaining graph.
struct PreparedCleanupRemovalPlan {
    targets: Vec<PreparedCleanupRemovalTarget>,
    entities: HashMap<Entity, usize>,
    order: Vec<usize>,
    sources: Vec<(ExactRegistryClaim, ExactRegistryClaim, Vec<usize>)>,
    focus: HashSet<Entity>,
    contained_focus_roots: HashMap<Entity, Entity>,
}

struct PreparedCleanupRemovalTarget {
    index: usize,
    entity: Entity,
    absent: bool,
    parent: Option<Entity>,
    children: HashSet<Entity>,
    normalized_manifest: Option<ComponentManifest>,
    receipt: Option<PreparedCleanupReceiptBatch>,
}

impl PreparedCleanupRemovalPlan {
    fn prepare(
        world: &World,
        ledger: &CleanupLedger,
        attempt: &CleanupAttempt,
        focus: &[Entity],
    ) -> Option<Self> {
        let mut targets = Vec::new();
        targets.try_reserve_exact(attempt.evidence.len()).ok()?;
        let mut entities = HashMap::new();
        entities.try_reserve(attempt.evidence.len()).ok()?;
        let mut sources: Vec<(ExactRegistryClaim, ExactRegistryClaim, Vec<usize>)> = Vec::new();
        for (index, evidence) in &attempt.evidence {
            let obligation = &ledger.obligations[*index];
            let (absent, parent, normalized_manifest, disposition) = match evidence {
                CleanupEvidence::AlreadyAbsent => {
                    (true, None, None, CleanupDisposition::AlreadyAbsent)
                }
                CleanupEvidence::Exact {
                    actual_parent,
                    normalized_manifest,
                    ..
                } => (
                    false,
                    *actual_parent,
                    obligation
                        .identity
                        .registered()
                        .map(|_| normalized_manifest.clone()),
                    if *actual_parent == Some(obligation.expected_parent) {
                        CleanupDisposition::Removed
                    } else {
                        CleanupDisposition::RemovedDetached
                    },
                ),
                _ => return None,
            };
            let row = targets.len();
            if entities.insert(obligation.entity(), row).is_some() {
                return None;
            }
            if let CleanupTargetIdentity::ContainedContent { root, shell, .. } =
                &obligation.identity
            {
                if let Some((_, _, members)) =
                    sources.iter_mut().find(|(r, s, _)| r == root && s == shell)
                {
                    members.push(row);
                } else {
                    sources.push((root.clone(), shell.clone(), vec![row]));
                }
            }
            let receipt = ledger.prepare_receipt_batch(
                attempt,
                vec![CleanupReceipt::new(
                    ledger.ledger_domain,
                    attempt.revision,
                    *index,
                    disposition,
                    obligation,
                )],
            )?;
            targets.push(PreparedCleanupRemovalTarget {
                index: *index,
                entity: obligation.entity(),
                absent,
                parent,
                children: if absent {
                    HashSet::new()
                } else {
                    observed_children(world, obligation.entity())
                        .into_iter()
                        .collect()
                },
                normalized_manifest,
                receipt: Some(receipt),
            });
        }
        if targets.iter().any(|target| {
            target
                .children
                .iter()
                .any(|child| entities.get(child).is_none_or(|row| targets[*row].absent))
        }) {
            return None;
        }
        // Accepted roles exclude foreign descendants. Use actual direct edges,
        // not the original expected-parent ordering, for non-recursive removal.
        let mut depths = Vec::new();
        depths.try_reserve_exact(targets.len()).ok()?;
        for (row, target) in targets.iter().enumerate() {
            let mut parent = target.parent;
            let mut depth = 0usize;
            while let Some(next) = parent.and_then(|entity| entities.get(&entity).copied()) {
                depth += 1;
                if depth > targets.len() {
                    return None;
                }
                parent = targets[next].parent;
            }
            depths.push((row, depth));
        }
        depths.sort_by_key(|(row, depth)| {
            (std::cmp::Reverse(*depth), targets[*row].entity.to_bits())
        });
        let order = depths.into_iter().map(|(row, _)| row).collect();
        let contained_focus_roots = ledger
            .obligations
            .iter()
            .filter_map(|obligation| {
                if let CleanupTargetIdentity::ContainedContent { entity, root, .. } =
                    &obligation.identity
                {
                    Some((*entity, root.entity()))
                } else {
                    None
                }
            })
            .collect();
        Some(Self {
            targets,
            entities,
            order,
            sources,
            focus: focus.iter().copied().collect(),
            contained_focus_roots,
        })
    }

    fn targets_are_current(&self, world: &World, ledger: &CleanupLedger) -> bool {
        let registry = world.resource::<RouteEntityRegistry>();
        let table = world.get_resource::<QuarantineLeaseTable>();
        for (root, shell, members) in &self.sources {
            if members.iter().any(|row| {
                let target = &self.targets[*row];
                !target.absent && ledger.receipts[target.index].is_none()
            }) && (!contained_source_is_current(world, root, shell)
                || !current_contained_target(world, root.entity(), root.entity(), shell.entity()))
            {
                return false;
            }
        }
        let relationship_components = [
            world.components().component_id::<ChildOf>(),
            world.components().component_id::<Children>(),
        ];
        for target in &self.targets {
            let obligation = &ledger.obligations[target.index];
            if ledger.receipts[target.index].is_some() {
                if world.get_entity(target.entity).is_ok()
                    || registry.record(target.entity).is_some()
                {
                    return false;
                }
                continue;
            }
            if target.absent {
                if world.get_entity(target.entity).is_ok()
                    || registry.record(target.entity).is_some_and(|record| {
                        obligation.identity.registered() != Some(&record.claim())
                    })
                {
                    return false;
                }
                continue;
            }
            if world.get_entity(target.entity).is_err()
                || world.get::<ChildOf>(target.entity).map(ChildOf::parent) != target.parent
            {
                return false;
            }
            let expected_children = target
                .children
                .iter()
                .filter(|child| {
                    self.entities
                        .get(child)
                        .is_none_or(|row| ledger.receipts[self.targets[*row].index].is_none())
                })
                .count();
            let children = world.get::<Children>(target.entity);
            if children.map_or(0, |children| children.len()) != expected_children
                || children
                    .into_iter()
                    .flat_map(|children| children.iter())
                    .any(|child| {
                        !target.children.contains(child)
                            || self.entities.get(child).is_some_and(|row| {
                                ledger.receipts[self.targets[*row].index].is_some()
                            })
                    })
            {
                return false;
            }
            match &obligation.identity {
                CleanupTargetIdentity::Registered(claim) => {
                    if registry
                        .record(target.entity)
                        .is_none_or(|record| record.claim() != *claim)
                        || !registered_cleanup_physical_claim_is_compatible(
                            world, obligation, claim,
                        )
                    {
                        return false;
                    }
                    let Some(actual) = ComponentManifest::from_world(world, target.entity) else {
                        return false;
                    };
                    let Some(expected) = &target.normalized_manifest else {
                        return false;
                    };
                    if !actual
                        .component_ids()
                        .iter()
                        .filter(|component| !relationship_components.contains(&Some(**component)))
                        .eq(expected.component_ids().iter())
                    {
                        return false;
                    }
                }
                CleanupTargetIdentity::ContainedContent { .. } => {
                    if registry.record(target.entity).is_some()
                        || table.is_some_and(|table| {
                            table.record(target.entity).is_some()
                                || table.unresolved_record(target.entity).is_some()
                        })
                    {
                        return false;
                    }
                }
            }
        }
        true
    }

    fn current_focus_targets(&self, world: &World) -> Vec<Entity> {
        let mut targets = Vec::with_capacity(2);
        for entity in [
            world.get_resource::<InputFocus>().and_then(InputFocus::get),
            world
                .get_resource::<RouteFocusState>()
                .and_then(RouteFocusState::focused),
        ]
        .into_iter()
        .flatten()
        {
            if !self.focus.contains(&entity) || targets.contains(&entity) {
                continue;
            }
            let escaped = self
                .contained_focus_roots
                .get(&entity)
                .is_some_and(|root| !contained_target_is_inside(world, entity, *root));
            if !escaped {
                targets.push(entity);
            }
        }
        targets
    }
}

struct PreparedFailureCleanupRemoval {
    index: usize,
    entity: Entity,
    action: FailureCleanupRemovalAction,
    retirement: Option<PreparedExactRegistryRetirement>,
    receipt: PreparedCleanupReceiptBatch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FailureCleanupRemovalAction {
    AlreadyAbsent,
    Despawn,
}

struct PreparedFailureCleanupMountParentExchanges {
    apply: Vec<(ExactRegistryClaim, ComponentManifest)>,
    expected: Vec<(ExactRegistryClaim, ComponentManifest)>,
}

impl PreparedTargetPreparationPermanentRemoval {
    fn is_current(
        &self,
        world: &World,
        obligation: &CleanupObligation,
        completed: &[Entity],
    ) -> bool {
        obligation.identity.registered().is_none_or(|claim| {
            world
                .resource::<RouteEntityRegistry>()
                .record(self.entity)
                .is_some_and(|record| record.claim() == *claim)
        }) && world.get::<ChildOf>(self.entity).map(ChildOf::parent) == self.expected_parent
            && children_match_after_removals(world, self.entity, &self.children_before, completed)
            && cleanup_obligation_has_exact_authority(world, obligation)
    }
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

/// A physical owner container. Its units may share a mount slot; this value
/// makes no active-branch or routing-topology claim.
pub(crate) struct LeavingPayload {
    frames: Vec<RealizedFrame>,
    child_slots: Vec<MountedChildSlot>,
}

/// Reserved capacity for an already checked cleanup partition.
/// The caller consumes the same splice/retained holders before publication;
/// this container neither classifies targets nor grants mutation authority.
pub(crate) struct PreparedLeavingAssembly {
    frames: Vec<RealizedFrame>,
    child_slots: Vec<MountedChildSlot>,
}

impl PreparedLeavingAssembly {
    pub(crate) fn reserve(
        manifest: &FrozenCleanupManifest,
        frames: &[&RealizedFrame],
        slots: &[&MountedChildSlot],
    ) -> Option<Self> {
        let owned_count = frames.iter().try_fold(slots.len(), |count, frame| {
            count.checked_add(frame.obligation_count())
        })?;
        let mut sources = Vec::new();
        sources.try_reserve_exact(owned_count).ok()?;
        for frame in frames {
            for (claim, parent) in frame.cleanup_members() {
                if !matches!(
                    claim.lifecycle(),
                    EntityLifecycle::Active | EntityLifecycle::Retained | EntityLifecycle::Leaving
                ) {
                    return None;
                }
                sources.push((claim, parent, frame.shell()));
            }
        }
        for slot in slots {
            if !matches!(
                slot.slot_claim().lifecycle(),
                EntityLifecycle::Active
                    | EntityLifecycle::PreparedInert
                    | EntityLifecycle::Retained
                    | EntityLifecycle::Leaving
            ) {
                return None;
            }
            sources.push((slot.slot_claim(), slot.anchor(), slot.slot()));
        }
        if sources.iter().enumerate().any(|(index, (claim, _, _))| {
            sources[index + 1..]
                .iter()
                .any(|(other, _, _)| claim.entity() == other.entity())
        }) || manifest
            .obligations
            .iter()
            .filter(|obligation| obligation.identity.registered().is_some())
            .count()
            != sources.len()
        {
            return None;
        }
        for (source, parent, root) in &sources {
            let mut matches = manifest
                .obligations
                .iter()
                .filter(|obligation| obligation.entity() == source.entity());
            let obligation = matches.next()?;
            let claim = obligation.identity.registered()?;
            if matches.next().is_some()
                || claim.owner() != source.owner()
                || claim.seal() != source.seal()
                || claim.lifecycle() != EntityLifecycle::Leaving
                || obligation.expected_parent != *parent
                || obligation.unit_root != *root
            {
                return None;
            }
        }
        let mut contained_sources: Vec<(Entity, Entity)> = Vec::new();
        #[cfg(feature = "bsn")]
        for frame in frames {
            let targets = crate::presentation::native_frame::contained_cleanup_targets(frame);
            contained_sources.try_reserve(targets.len()).ok()?;
            for (entity, parent, target_manifest) in targets {
                if contained_sources
                    .iter()
                    .any(|(existing, _)| *existing == entity)
                {
                    return None;
                }
                let mut matches = manifest
                    .obligations
                    .iter()
                    .filter(|obligation| obligation.entity() == entity);
                let obligation = matches.next()?;
                let CleanupTargetIdentity::ContainedContent { root, shell, .. } =
                    &obligation.identity
                else {
                    return None;
                };
                let source_root = frame
                    .claims()
                    .find(|claim| Some(claim.entity()) == frame.native_root())?;
                let source_shell = frame
                    .claims()
                    .find(|claim| claim.entity() == frame.shell())?;
                if matches.next().is_some()
                    || obligation.expected_parent != parent
                    || obligation.unit_root != frame.shell()
                    || obligation.manifest != target_manifest
                    || root.entity() != source_root.entity()
                    || root.owner() != source_root.owner()
                    || root.seal() != source_root.seal()
                    || root.lifecycle() != EntityLifecycle::Leaving
                    || shell.entity() != source_shell.entity()
                    || shell.owner() != source_shell.owner()
                    || shell.seal() != source_shell.seal()
                    || shell.lifecycle() != EntityLifecycle::Leaving
                {
                    return None;
                }
                contained_sources.push((entity, parent));
            }
        }
        if manifest.obligations.len() != owned_count.checked_add(contained_sources.len())? {
            return None;
        }
        let mut owned_frames = Vec::new();
        owned_frames.try_reserve_exact(frames.len()).ok()?;
        let mut owned_slots = Vec::new();
        owned_slots.try_reserve_exact(slots.len()).ok()?;
        Some(Self {
            frames: owned_frames,
            child_slots: owned_slots,
        })
    }

    /// Consumes only the original payloads observed by reserve. The routing
    /// owner has already checked their IDs and complete source projection.
    /// Called before engine.publish; the atomic tail only installs the result.
    pub(crate) fn assemble(
        mut self,
        leaving: RealizedBranch,
        retired: Vec<RealizedBranch>,
    ) -> LeavingPayload {
        for branch in std::iter::once(leaving).chain(retired) {
            let (mut frames, mut child_slots) = branch.into_owned_parts();
            self.frames.append(&mut frames);
            self.child_slots.append(&mut child_slots);
        }
        LeavingPayload {
            frames: self.frames,
            child_slots: self.child_slots,
        }
    }
}

impl LeavingPayload {
    pub(crate) fn is_empty(&self) -> bool {
        self.frames.is_empty() && self.child_slots.is_empty()
    }

    fn from_branch(branch: RealizedBranch) -> Self {
        let (frames, child_slots) = branch.into_owned_parts();
        Self {
            frames,
            child_slots,
        }
    }

    fn from_frame(frame: RealizedFrame) -> Self {
        Self {
            frames: vec![frame],
            child_slots: Vec::new(),
        }
    }

    pub(crate) fn frames(&self) -> &[RealizedFrame] {
        &self.frames
    }

    #[cfg(test)]
    pub(crate) fn claims(&self) -> impl Iterator<Item = &ExactRegistryClaim> {
        self.frames
            .iter()
            .flat_map(RealizedFrame::claims)
            .chain(self.child_slots.iter().map(MountedChildSlot::slot_claim))
    }
}

pub(crate) enum CleanupOwner {
    Leaving(LeavingPayload),
    FailedPrepared(FailedPreparedCleanupOwner),
    AuthoritySettlement(AuthoritySettlementOwner),
    BackNoTargetAuthority(BackNoTargetAuthorityCleanupOwner),
    IdleHostAuthority(IdleHostAuthorityCleanupOwner),
}

pub(crate) struct IdleHostAuthorityCleanupOwner {
    active: Option<RealizedBranch>,
    retained: Vec<RealizedBranch>,
    pub(crate) host: RetiredHostBinding,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AuthorityCleanupFailureKind {
    RetryableCapacity,
    IdentityExhausted,
    Preparation,
    Attempt,
    TerminalProjection,
    Partition,
    PickableWrite,
    HostEdgeWrite,
    HostEdgePhase,
    Focus,
    AbsentRetirement,
    RemovalPreflight,
    RemovalTerminal,
    RemainingRemovals,
    ResidualPreflight,
    ResidualFocus,
}

#[derive(Debug)]
pub(crate) struct AuthorityCleanupFailure {
    pub(crate) kind: AuthorityCleanupFailureKind,
    expected: Option<CleanupAttempt>,
    observed: Vec<(usize, CleanupEvidence)>,
    manifest_digest: u64,
    obligation_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct IdleHostResidualObservation {
    roots: Vec<Entity>,
    targets: Vec<Entity>,
    closure: Vec<FrozenBackNoTargetAuthorityEntity>,
    inert: Vec<bool>,
    input: Vec<crate::input::RouterTargetInputObservation>,
    edge_absent: bool,
    manifest_digest: u64,
    obligation_count: usize,
}

impl IdleHostResidualObservation {
    pub(crate) fn current_input(
        &self,
        world: &World,
    ) -> Vec<crate::input::RouterTargetInputObservation> {
        self.targets
            .iter()
            .map(|target| crate::input::observe_router_target_input(world, *target))
            .collect()
    }
    pub(crate) fn input_is_revoked(&self, world: &World) -> bool {
        self.targets
            .iter()
            .all(|target| crate::input::observe_router_target_input(world, *target).is_revoked())
    }

    pub(crate) fn is_current(&self, world: &World) -> bool {
        let registry = world.resource::<RouteEntityRegistry>();
        let mut closure = observed_descendant_closure(world, self.roots.iter().copied())
            .into_iter()
            .collect::<Vec<_>>();
        closure.sort_by_key(|entity| entity.to_bits());
        self.closure
            == closure
                .into_iter()
                .map(|entity| FrozenBackNoTargetAuthorityEntity::observe(world, registry, entity))
                .collect::<Vec<_>>()
    }

    pub(crate) fn is_safe(&self) -> bool {
        self.edge_absent
            && self.inert.iter().all(|inert| *inert)
            && self
                .input
                .iter()
                .all(crate::input::RouterTargetInputObservation::is_revoked)
    }

    pub(crate) fn facts(&self) -> bevy_ui_router_core::integration::IdleHostLossPhysicalFacts {
        use bevy_ui_router_core::integration::{
            IdleHostLossPhysicalDisposition, IdleHostLossPhysicalFacts,
        };
        let observation_digest = self.targets.iter().zip(&self.inert).zip(&self.input).fold(
            self.manifest_digest ^ u64::from(self.edge_absent),
            |hash, ((target, inert), input)| {
                hash.rotate_left(7)
                    ^ target.to_bits()
                    ^ u64::from(*inert)
                    ^ (u64::from(input.is_revoked()) << 1)
            },
        );
        IdleHostLossPhysicalFacts {
            manifest_digest: self.manifest_digest,
            obligation_count: self.obligation_count,
            disposition: if self.is_safe() {
                IdleHostLossPhysicalDisposition::SafeResidual { observation_digest }
            } else {
                IdleHostLossPhysicalDisposition::IsolationUnproven { observation_digest }
            },
        }
    }
}

pub(crate) struct FailedPreparedCleanupOwner {
    branch: FailedPreparedBranch,
    preflight: Option<LifecyclePreflightEvidence>,
}

impl FailedPreparedCleanupOwner {
    pub(crate) fn into_completed(mut self) -> CompletedFailedPreparedOwner {
        CompletedFailedPreparedOwner {
            retained_prepared_prefix: self.branch.take_retained_prepared_prefix(),
            #[cfg(feature = "bsn")]
            native_failure: self.branch.take_native_failure(),
        }
    }
}

pub(crate) struct CompletedFailedPreparedOwner {
    pub(crate) retained_prepared_prefix: Option<PreparedBranch>,
    #[cfg(feature = "bsn")]
    pub(crate) native_failure: Option<crate::presentation::native_failure::NativeFailureOwner>,
}

pub(crate) struct AuthoritySettlementOwner {
    active: Option<RealizedBranch>,
    retained: Vec<RealizedBranch>,
    pending: AuthoritySettlementPendingOwner,
    host: RetiredHostBinding,
}

impl AuthoritySettlementOwner {
    #[cfg(feature = "bsn")]
    pub(crate) fn take_native_failure(
        &mut self,
    ) -> Option<crate::presentation::native_failure::NativeFailureOwner> {
        match &mut self.pending {
            AuthoritySettlementPendingOwner::Prepared(branch) => branch.take_native_failure(),
            AuthoritySettlementPendingOwner::Terminal(failed) => failed.take_native_failure(),
        }
    }
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

    #[cfg(test)]
    pub(crate) fn leaving_frames(&self) -> &[RealizedFrame] {
        self.owner.active.frames()
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
    pub(crate) fn prepare_idle_host_completed_retention(
        &self,
        world: &World,
    ) -> Result<Option<IdleHostCompletedRetention>, AuthorityCleanupFailureKind> {
        let terminal = self
            .back_no_target_authority_partition
            .as_ref()
            .and_then(|partition| partition.terminal.as_ref())
            .ok_or(AuthorityCleanupFailureKind::TerminalProjection)?;
        if terminal.len() != self.receipts.len()
            || terminal.iter().any(|terminal| {
                self.receipts
                    .get(terminal.obligation)
                    .and_then(Option::as_ref)
                    .is_none_or(|receipt| !terminal.matches_receipt(receipt))
                    || !terminal.is_current(world)
            })
        {
            return Err(AuthorityCleanupFailureKind::TerminalProjection);
        }
        let count = terminal
            .iter()
            .filter(|terminal| {
                matches!(
                    terminal.presence,
                    PreparedBackNoTargetAuthorityTerminalPresence::Retained { .. }
                )
            })
            .count();
        if count == 0 {
            return Ok(None);
        }
        let mut terminals = Vec::new();
        terminals
            .try_reserve_exact(authority_preparation_capacity_input(
                AuthorityPreparationCapacitySite::CompletedRetention,
                count,
            ))
            .map_err(|_| AuthorityCleanupFailureKind::RetryableCapacity)?;
        terminals.extend(
            terminal
                .iter()
                .filter(|terminal| {
                    matches!(
                        terminal.presence,
                        PreparedBackNoTargetAuthorityTerminalPresence::Retained { .. }
                    )
                })
                .cloned(),
        );
        Ok(Some(IdleHostCompletedRetention { terminals }))
    }

    pub(crate) fn idle_host_failure(
        &self,
        world: &World,
        kind: AuthorityCleanupFailureKind,
    ) -> AuthorityCleanupFailure {
        AuthorityCleanupFailure {
            kind,
            expected: self.issued_attempt.clone(),
            observed: collect_evidence(world, &self.obligations, &self.receipts),
            manifest_digest: self.manifest_digest,
            obligation_count: self.obligations.len(),
        }
    }

    pub(crate) fn observe_idle_host_residual(
        &self,
        world: &World,
    ) -> Option<IdleHostResidualObservation> {
        let host = self.idle_host_authority_host()?;
        let registry = world.resource::<RouteEntityRegistry>();
        let targets = self
            .obligations
            .iter()
            .map(|obligation| obligation.entity())
            .collect::<Vec<_>>();
        let mut roots = targets.clone();
        roots.push(host.host);
        let mut closure = observed_descendant_closure(world, roots.iter().copied())
            .into_iter()
            .collect::<Vec<_>>();
        closure.sort_by_key(|entity| entity.to_bits());
        let inert = self
            .obligations
            .iter()
            .enumerate()
            .map(|(index, obligation)| {
                world.get_entity(obligation.entity()).is_err()
                    || match &obligation.identity {
                        CleanupTargetIdentity::Registered(claim) => {
                            registry
                                .record(obligation.entity())
                                .is_some_and(|record| record.claim() == *claim)
                                && terminal_unresolved_root_is_inert(world, claim, false)
                        }
                        identity @ CleanupTargetIdentity::ContainedContent { .. } => {
                            contained_terminal_scope_is_current(world, identity)
                        }
                    }
                    || self
                        .back_no_target_authority_partition
                        .as_ref()
                        .and_then(|partition| partition.terminal.as_ref())
                        .and_then(|terminal| terminal.get(index))
                        .is_some_and(|terminal| {
                            self.receipts[index]
                                .as_ref()
                                .is_some_and(|receipt| terminal.matches_receipt(receipt))
                                && terminal.is_current(world)
                        })
            })
            .collect();
        let edge_absent = world.get_entity(host.host).is_err()
            || (world
                .get::<ChildOf>(host.top_slot)
                .is_none_or(|parent| parent.parent() != host.host)
                && !observed_children(world, host.host).contains(&host.top_slot));
        Some(IdleHostResidualObservation {
            input: targets
                .iter()
                .map(|target| crate::input::observe_router_target_input(world, *target))
                .collect(),
            roots,
            targets,
            closure: closure
                .into_iter()
                .map(|entity| FrozenBackNoTargetAuthorityEntity::observe(world, registry, entity))
                .collect(),
            inert,
            edge_absent,
            manifest_digest: self.manifest_digest,
            obligation_count: self.obligations.len(),
        })
    }

    pub(crate) fn settle_idle_host_authority(
        &mut self,
        world: &mut World,
    ) -> Result<BackNoTargetAuthorityCompletionFacts, AuthorityCleanupFailure> {
        let attempt = self.attempt(world);
        let result = (|| {
            let attempt = attempt
                .as_ref()
                .ok_or(AuthorityCleanupFailureKind::Attempt)?;
            if !matches!(self.owner, Some(CleanupOwner::IdleHostAuthority(_)))
                || self.batch.is_some()
                || self.back_no_target_batch.is_some()
                || self.back_no_target_capacity.is_some()
                || !self.mount_parents.is_empty()
                || self.issued_attempt.as_ref() != Some(attempt)
            {
                return Err(AuthorityCleanupFailureKind::Attempt);
            }
            if !self.back_no_target_authority_partition_is_current(world) {
                return Err(AuthorityCleanupFailureKind::Partition);
            }
            let observed = collect_evidence(world, &self.obligations, &self.receipts);
            if observed != attempt.evidence
                || observed
                    .iter()
                    .any(|(_, evidence)| matches!(evidence, CleanupEvidence::StaleClaim))
            {
                return Err(AuthorityCleanupFailureKind::Attempt);
            }
            let focus = self.focus_closure_for_attempt(
                world,
                quarantine_focus_closure(world, &observed, &self.obligations),
            );
            self.settle_authority_evidence_typed(world, attempt, focus)?;
            self.post_settlement_focus.clear();
            self.finish_if_settled();
            if !self.back_no_target_authority_partition_is_current(world) {
                return Err(AuthorityCleanupFailureKind::Partition);
            }
            self.back_no_target_authority_completion_facts()
                .ok_or(AuthorityCleanupFailureKind::Attempt)
        })();
        result.map_err(|kind| {
            self.issued_attempt = None;
            AuthorityCleanupFailure {
                kind,
                expected: attempt,
                observed: collect_evidence(world, &self.obligations, &self.receipts),
                manifest_digest: self.manifest_digest,
                obligation_count: self.obligations.len(),
            }
        })
    }

    pub(crate) fn activate_idle_host_authority(
        prepared: PreparedCleanupLedger,
        active: Option<RealizedBranch>,
        host: RetiredHostBinding,
        host_children: Vec<Entity>,
        partition: FrozenBackNoTargetAuthorityPartition,
    ) -> Self {
        Self::activate_idle_host_authority_with_retained(
            prepared,
            active,
            host,
            host_children,
            partition,
            Vec::new(),
        )
    }

    pub(crate) fn activate_idle_host_authority_with_retained(
        prepared: PreparedCleanupLedger,
        active: Option<RealizedBranch>,
        host: RetiredHostBinding,
        host_children: Vec<Entity>,
        partition: FrozenBackNoTargetAuthorityPartition,
        retained: Vec<RealizedBranch>,
    ) -> Self {
        let mut ledger = Self::activate_owner(
            prepared,
            Some(CleanupOwner::IdleHostAuthority(
                IdleHostAuthorityCleanupOwner {
                    active,
                    retained,
                    host,
                },
            )),
        );
        ledger.authority_host_children = Some(host_children);
        ledger.back_no_target_authority_partition = Some(partition);
        ledger
    }

    pub(crate) fn take_idle_host_authority_owner(
        &mut self,
    ) -> Option<IdleHostAuthorityCleanupOwner> {
        match self.owner.take() {
            Some(CleanupOwner::IdleHostAuthority(owner)) => Some(owner),
            owner => {
                self.owner = owner;
                None
            }
        }
    }

    pub(crate) fn idle_host_authority_host(&self) -> Option<&RetiredHostBinding> {
        match self.owner.as_ref() {
            Some(CleanupOwner::IdleHostAuthority(owner)) => Some(&owner.host),
            _ => None,
        }
    }

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
        let reservation_count = if reservation_fault_injected() {
            usize::MAX
        } else {
            manifest.obligations.len()
        };
        let mut receipts = Vec::new();
        if receipts.try_reserve_exact(reservation_count).is_err() {
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
            failure_cleanup_partition: None,
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
            leaving.map(|frame| CleanupOwner::Leaving(LeavingPayload::from_frame(frame))),
        )
    }

    pub(crate) fn activate_failed(
        prepared: PreparedCleanupLedger,
        failed: FailedPreparedBranch,
    ) -> Self {
        Self::activate_failed_with_preflight(prepared, failed, None)
    }

    pub(crate) fn activate_failed_with_preflight(
        prepared: PreparedCleanupLedger,
        failed: FailedPreparedBranch,
        preflight: Option<LifecyclePreflightEvidence>,
    ) -> Self {
        Self::activate_owner(
            prepared,
            Some(CleanupOwner::FailedPrepared(FailedPreparedCleanupOwner {
                branch: failed,
                preflight,
            })),
        )
    }

    pub(crate) fn activate_leaving_branch(
        prepared: PreparedCleanupLedger,
        leaving: Option<RealizedBranch>,
    ) -> Self {
        Self::activate_owner(
            prepared,
            leaving.map(|branch| CleanupOwner::Leaving(LeavingPayload::from_branch(branch))),
        )
    }

    pub(crate) fn activate_leaving_forest(
        prepared: PreparedCleanupLedger,
        leaving: LeavingPayload,
    ) -> Self {
        let owner = (!leaving.is_empty()).then_some(CleanupOwner::Leaving(leaving));
        Self::activate_owner(prepared, owner)
    }

    pub(crate) fn activate_authority_settlement(
        prepared: PreparedCleanupLedger,
        active: RealizedBranch,
        pending: FailedPreparedBranch,
        host: RetiredHostBinding,
        host_children: Vec<Entity>,
    ) -> Self {
        Self::activate_authority_settlement_with_retained(
            prepared,
            active,
            pending,
            host,
            host_children,
            Vec::new(),
        )
    }

    pub(crate) fn activate_authority_settlement_with_retained(
        prepared: PreparedCleanupLedger,
        active: RealizedBranch,
        pending: FailedPreparedBranch,
        host: RetiredHostBinding,
        host_children: Vec<Entity>,
        retained: Vec<RealizedBranch>,
    ) -> Self {
        let mut ledger = Self::activate_owner(
            prepared,
            Some(CleanupOwner::AuthoritySettlement(
                AuthoritySettlementOwner {
                    active: Some(active),
                    retained,
                    pending: AuthoritySettlementPendingOwner::Prepared(pending),
                    host,
                },
            )),
        );
        ledger.authority_host_children = Some(host_children);
        ledger
    }

    pub(crate) fn activate_failed_authority_settlement_with_retained(
        prepared: PreparedCleanupLedger,
        active: Option<RealizedBranch>,
        failed: PendingFailedBranch,
        host: RetiredHostBinding,
        host_children: Vec<Entity>,
        retained: Vec<RealizedBranch>,
    ) -> Self {
        let mut ledger = Self::activate_owner(
            prepared,
            Some(CleanupOwner::AuthoritySettlement(
                AuthoritySettlementOwner {
                    active,
                    retained,
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

    pub(crate) fn is_failed_prepared(&self) -> bool {
        matches!(self.owner, Some(CleanupOwner::FailedPrepared(_)))
    }

    pub(crate) fn failed_prepared_branch(&self) -> Option<&FailedPreparedBranch> {
        match self.owner.as_ref() {
            Some(CleanupOwner::FailedPrepared(owner)) => Some(&owner.branch),
            Some(
                CleanupOwner::Leaving(_)
                | CleanupOwner::AuthoritySettlement(_)
                | CleanupOwner::IdleHostAuthority(_)
                | CleanupOwner::BackNoTargetAuthority(_),
            )
            | None => None,
        }
    }

    #[cfg(feature = "bsn")]
    pub(crate) fn native_failure(
        &self,
    ) -> Option<&crate::presentation::native_failure::NativeFailureOwner> {
        self.failure_owner_branch()
            .and_then(FailedPreparedBranch::native_failure)
    }

    pub(crate) fn failure_owner_branch(&self) -> Option<&FailedPreparedBranch> {
        match self.owner.as_ref() {
            Some(CleanupOwner::FailedPrepared(owner)) => Some(&owner.branch),
            Some(CleanupOwner::AuthoritySettlement(owner)) => Some(owner.pending.branch()),
            _ => None,
        }
    }

    fn is_physical_authority_settlement(&self) -> bool {
        matches!(
            self.owner,
            Some(
                CleanupOwner::AuthoritySettlement(_)
                    | CleanupOwner::BackNoTargetAuthority(_)
                    | CleanupOwner::IdleHostAuthority(_)
            )
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

    pub(crate) fn take_failed_prepared_owner(&mut self) -> Option<FailedPreparedCleanupOwner> {
        match self.owner.take() {
            Some(CleanupOwner::FailedPrepared(owner)) => Some(owner),
            other => {
                self.owner = other;
                None
            }
        }
    }

    fn release_settled_owner(&mut self) {
        if !self.is_physical_authority_settlement()
            && !matches!(
                self.owner,
                Some(CleanupOwner::Leaving(_) | CleanupOwner::FailedPrepared(_))
            )
        {
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
            related_protected: None,
            post_settlement_focus: Vec::new(),
            authority_host_children: None,
            back_no_target_authority_partition: None,
            failure_cleanup_partition: prepared.failure_cleanup_partition,
            stopped: None,
            quarantine_detachment: None,
            cleanup_settlement_permanent_pending: None,
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
            related_protected: None,
            post_settlement_focus: Vec::new(),
            authority_host_children: None,
            back_no_target_authority_partition: None,
            failure_cleanup_partition: None,
            stopped: None,
            quarantine_detachment: None,
            cleanup_settlement_permanent_pending: None,
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
            Some(CleanupOwner::BackNoTargetAuthority(_) | CleanupOwner::IdleHostAuthority(_)) => {
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

    pub(crate) fn into_target_preparation_permanent_completion(
        self,
        domain: Option<TargetPreparationPermanentCompletionDomain>,
    ) -> Result<
        (
            Self,
            TargetPreparationPermanentCompletionDomain,
            TargetPreparationPermanentCompletionFacts,
        ),
        (
            Self,
            Option<TargetPreparationPermanentCompletionDomain>,
            TargetPreparationPermanentCompletionPreparationEvidence,
        ),
    > {
        let expected_domain = domain.as_ref().map(|domain| domain.ledger_domain);
        let observed_receipt_count = self.receipts.iter().flatten().count();
        let unobserved_receipt_count = self
            .receipts
            .iter()
            .flatten()
            .filter(|receipt| !receipt.observed)
            .count();
        let exact = expected_domain == Some(self.ledger_domain)
            && self.owner.is_none()
            && self.batch.is_none()
            && self.back_no_target_batch.is_none()
            && self.back_no_target_capacity.is_none()
            && self.mount_parents.is_empty()
            && self.receipts.len() == self.obligations.len()
            && observed_receipt_count == self.receipts.len()
            && unobserved_receipt_count == 0
            && self.issued_attempt.is_none()
            && self.completion_digest.is_some()
            && self.authority_host_children.is_none()
            && self.back_no_target_authority_partition.is_none();
        let evidence = TargetPreparationPermanentCompletionPreparationEvidence {
            source: TargetPreparationPermanentCompletionPreparationSource::ActiveIsolated,
            expected: TargetPreparationPermanentCompletionPreparationExpectation {
                ledger_domain: self.ledger_domain,
                manifest_digest: self.manifest_digest,
                obligation_count: self.obligations.len(),
            },
            observed: TargetPreparationPermanentCompletionPreparationObservation {
                marker_domain: expected_domain,
                ledger_domain: self.ledger_domain,
                owner_present: self.owner.is_some(),
                normal_batch_present: self.batch.is_some(),
                back_no_target_batch_present: self.back_no_target_batch.is_some(),
                back_no_target_capacity_present: self.back_no_target_capacity.is_some(),
                mount_parent_count: self.mount_parents.len(),
                receipt_slot_count: self.receipts.len(),
                observed_receipt_count,
                unobserved_receipt_count,
                issued_attempt_present: self.issued_attempt.is_some(),
                completion_digest: self.completion_digest,
                authority_host_children_present: self.authority_host_children.is_some(),
                back_no_target_authority_partition_present: self
                    .back_no_target_authority_partition
                    .is_some(),
            },
        };
        if !exact {
            return Err((self, domain, evidence));
        }
        let Some(completion_digest) = self.completion_digest else {
            return Err((self, domain, evidence));
        };
        let Some(domain) = domain else {
            return Err((self, None, evidence));
        };
        let facts = TargetPreparationPermanentCompletionFacts {
            manifest_digest: self.manifest_digest,
            obligation_count: self.obligations.len(),
            completion_digest,
            settled_count: observed_receipt_count,
        };
        Ok((self, domain, facts))
    }

    pub(crate) fn back_no_target_authority_completion_facts(
        &self,
    ) -> Option<BackNoTargetAuthorityCompletionFacts> {
        if !matches!(
            self.owner,
            Some(CleanupOwner::BackNoTargetAuthority(_) | CleanupOwner::IdleHostAuthority(_))
        ) || self.batch.is_some()
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

    pub(crate) fn exact_claims(&self) -> impl Iterator<Item = &ExactRegistryClaim> {
        self.obligations
            .iter()
            .filter_map(|obligation| obligation.identity.registered())
    }

    pub(crate) fn cleanup_settlement_permanent_evidence(
        &self,
    ) -> Option<&CleanupSettlementPermanentEvidence> {
        match self.stopped.as_ref() {
            Some(CleanupStopReason::Settlement(evidence)) => Some(evidence),
            _ => None,
        }
    }

    pub(crate) fn is_stopped(&self) -> bool {
        self.stopped.is_some()
    }

    pub(crate) fn post_commit_batch_matches(&self, batch: &PostCommitCleanupBatchView) -> bool {
        batch.manifest_digest() == self.manifest_digest
            && batch.obligation_count() == self.obligations.len()
    }

    #[cfg(test)]
    pub(crate) fn leaving_frames(&self) -> &[RealizedFrame] {
        match self.owner.as_ref() {
            Some(CleanupOwner::Leaving(branch)) => branch.frames(),
            Some(CleanupOwner::AuthoritySettlement(owner)) => owner
                .active
                .as_ref()
                .map(RealizedBranch::frames)
                .unwrap_or_default(),
            Some(CleanupOwner::BackNoTargetAuthority(owner)) => owner.active.frames(),
            Some(CleanupOwner::IdleHostAuthority(owner)) => owner
                .active
                .as_ref()
                .map(RealizedBranch::frames)
                .unwrap_or_default(),
            Some(CleanupOwner::FailedPrepared(_)) | None => &[],
        }
    }

    #[cfg(test)]
    pub(crate) fn failed_prepared_frames(&self) -> &[RealizedFrame] {
        match self.owner.as_ref() {
            Some(CleanupOwner::FailedPrepared(owner)) => owner.branch.frames(),
            Some(CleanupOwner::AuthoritySettlement(owner)) => owner.pending.branch().frames(),
            Some(
                CleanupOwner::Leaving(_)
                | CleanupOwner::BackNoTargetAuthority(_)
                | CleanupOwner::IdleHostAuthority(_),
            )
            | None => &[],
        }
    }

    #[cfg(test)]
    pub(crate) fn failed_preflight_evidence(&self) -> Option<LifecyclePreflightEvidence> {
        match self.owner.as_ref() {
            Some(CleanupOwner::FailedPrepared(owner)) => owner.preflight,
            Some(
                CleanupOwner::Leaving(_)
                | CleanupOwner::AuthoritySettlement(_)
                | CleanupOwner::IdleHostAuthority(_)
                | CleanupOwner::BackNoTargetAuthority(_),
            )
            | None => None,
        }
    }

    #[cfg(test)]
    pub(crate) fn authority_settlement_host(&self) -> Option<&RetiredHostBinding> {
        match self.owner.as_ref() {
            Some(CleanupOwner::AuthoritySettlement(owner)) => Some(&owner.host),
            Some(CleanupOwner::BackNoTargetAuthority(owner)) => Some(&owner.host),
            Some(CleanupOwner::IdleHostAuthority(owner)) => Some(&owner.host),
            Some(CleanupOwner::Leaving(_) | CleanupOwner::FailedPrepared(_)) | None => None,
        }
    }

    pub(crate) fn mount_new_staging_allowance(
        &self,
    ) -> Option<&crate::branch_transaction::SealedMountNewStagingAllowance> {
        match self.owner.as_ref() {
            Some(CleanupOwner::FailedPrepared(owner)) => owner.branch.mount_new_allowance(),
            Some(
                CleanupOwner::AuthoritySettlement(_)
                | CleanupOwner::BackNoTargetAuthority(_)
                | CleanupOwner::IdleHostAuthority(_)
                | CleanupOwner::Leaving(_),
            )
            | None => None,
        }
    }

    #[cfg(test)]
    pub(crate) fn exhaust_attempt_revision_for_test(&mut self) {
        self.next_attempt = u64::MAX;
    }

    pub(crate) fn attempt(&mut self, world: &World) -> Option<CleanupAttempt> {
        if self.completion_digest.is_some() || self.stopped.is_some() {
            return None;
        }
        let revision = self.next_attempt;
        let Some(next_attempt) = self.next_attempt.checked_add(1) else {
            self.stopped = Some(CleanupStopReason::AttemptRevisionExhausted);
            self.issued_attempt = None;
            return None;
        };
        self.next_attempt = next_attempt;
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

    fn target_preparation_permanent_boundary(
        stage: TargetPreparationPermanentSettlementFailureStage,
        expected: TargetPreparationPermanentSettlementExpectation,
        observed: TargetPreparationPermanentSettlementObservation,
    ) -> TargetPreparationPermanentSettlementBoundaryEvidence {
        TargetPreparationPermanentSettlementBoundaryEvidence {
            source: TargetPreparationPermanentSettlementSource::ActiveIsolated,
            stage,
            expected,
            observed,
        }
    }

    fn target_preparation_permanent_ledger_observation(
        &self,
    ) -> TargetPreparationPermanentSettlementObservation {
        TargetPreparationPermanentSettlementObservation::Ledger {
            owner_present: self.owner.is_some(),
            normal_batch_present: self.batch.is_some(),
            back_no_target_batch_present: self.back_no_target_batch.is_some(),
            back_no_target_capacity_present: self.back_no_target_capacity.is_some(),
            mount_parent_count: self.mount_parents.len(),
            receipt_slot_count: self.receipts.len(),
            receipt_count: self.receipts.iter().flatten().count(),
            completion_digest: self.completion_digest,
            authority_host_children_present: self.authority_host_children.is_some(),
            back_no_target_authority_partition_present: self
                .back_no_target_authority_partition
                .is_some(),
        }
    }

    fn target_preparation_permanent_receipt_slot_observation(
        &self,
        index: usize,
    ) -> TargetPreparationPermanentSettlementObservation {
        TargetPreparationPermanentSettlementObservation::ReceiptSlot {
            existing: self
                .receipts
                .get(index)
                .and_then(Option::as_ref)
                .map(|receipt| {
                    (
                        receipt.ledger_domain,
                        receipt.attempt_revision,
                        receipt.obligation,
                        receipt.disposition,
                        receipt.binding,
                        receipt.observed,
                    )
                }),
        }
    }

    pub(crate) fn target_preparation_permanent_receipt_observation_evidence(
        &self,
    ) -> TargetPreparationPermanentReceiptObservationEvidence {
        TargetPreparationPermanentReceiptObservationEvidence {
            manifest_digest: self.manifest_digest,
            obligation_count: self.obligations.len(),
            receipt_count: self.receipts.iter().flatten().count(),
            unobserved: self
                .receipts
                .iter()
                .enumerate()
                .filter_map(|(index, receipt)| {
                    receipt
                        .as_ref()
                        .filter(|receipt| !receipt.observed)
                        .map(|receipt| (index, receipt.disposition))
                })
                .collect(),
        }
    }

    pub(crate) fn settle_target_preparation_permanent<Evidence>(
        &mut self,
        world: &mut World,
        edge: TargetPreparationPermanentHostEdge<'_>,
        mut refreeze: impl FnMut(
            &World,
            TargetPreparationPermanentTransition<'_>,
        ) -> Result<(), Evidence>,
    ) -> Result<
        TargetPreparationPermanentCompletionDomain,
        TargetPreparationPermanentSettlementError<Evidence>,
    > {
        if self.owner.is_some()
            || self.batch.is_some()
            || self.back_no_target_batch.is_some()
            || self.back_no_target_capacity.is_some()
            || !self.mount_parents.is_empty()
            || self.receipts.iter().any(Option::is_some)
            || self.completion_digest.is_some()
            || self.authority_host_children.is_some()
            || self.back_no_target_authority_partition.is_some()
        {
            return Err(
                TargetPreparationPermanentSettlementError::SettlementRejected(
                    Self::target_preparation_permanent_boundary(
                        TargetPreparationPermanentSettlementFailureStage::LedgerPreflight,
                        TargetPreparationPermanentSettlementExpectation::FreshLedger {
                            manifest_digest: self.manifest_digest,
                            obligation_count: self.obligations.len(),
                        },
                        self.target_preparation_permanent_ledger_observation(),
                    ),
                ),
            );
        }

        let attempt = self.attempt(world).ok_or_else(|| {
            TargetPreparationPermanentSettlementError::AttemptUnavailable(
                Self::target_preparation_permanent_boundary(
                    TargetPreparationPermanentSettlementFailureStage::AttemptPreflight,
                    TargetPreparationPermanentSettlementExpectation::FreshLedger {
                        manifest_digest: self.manifest_digest,
                        obligation_count: self.obligations.len(),
                    },
                    TargetPreparationPermanentSettlementObservation::AttemptUnavailable {
                        next_attempt: self.next_attempt,
                        completion_digest: self.completion_digest,
                    },
                ),
            )
        })?;
        if self.issued_attempt.as_ref() != Some(&attempt)
            || attempt.unsettled_digest
                != unsettled_digest(self.manifest_digest, &self.obligations, &self.receipts)
            || !attempt.mount_parents_exact
            || attempt.evidence.len() != self.obligations.len()
        {
            return Err(
                TargetPreparationPermanentSettlementError::SettlementRejected(
                    Self::target_preparation_permanent_boundary(
                        TargetPreparationPermanentSettlementFailureStage::AttemptPreflight,
                        TargetPreparationPermanentSettlementExpectation::IssuedAttempt(
                            attempt.clone(),
                        ),
                        TargetPreparationPermanentSettlementObservation::Attempt {
                            issued: self.issued_attempt.clone(),
                            observed: attempt,
                        },
                    ),
                ),
            );
        }

        let top_slot_claim_count = self
            .obligations
            .iter()
            .filter(|obligation| obligation.identity.registered() == Some(edge.top_slot_claim))
            .count();
        let host_obligation_count = self
            .obligations
            .iter()
            .filter(|obligation| obligation.entity() == edge.host)
            .count();
        let host_children = observed_children(world, edge.host);
        let top_slot_parent = world.get::<ChildOf>(edge.top_slot).map(ChildOf::parent);
        if world.get_entity(edge.host).is_err()
            || edge.top_slot_claim.entity() != edge.top_slot
            || host_children != edge.children_before
            || top_slot_parent != Some(edge.host)
            || top_slot_claim_count != 1
            || host_obligation_count != 0
        {
            return Err(
                TargetPreparationPermanentSettlementError::SettlementRejected(
                    Self::target_preparation_permanent_boundary(
                        TargetPreparationPermanentSettlementFailureStage::HostEdgePrewrite,
                        TargetPreparationPermanentSettlementExpectation::HostEdgeBefore {
                            host: edge.host,
                            top_slot: edge.top_slot,
                            top_slot_claim: edge.top_slot_claim.clone(),
                            host_children: edge.children_before.to_vec(),
                            top_slot_parent: Some(edge.host),
                        },
                        TargetPreparationPermanentSettlementObservation::HostEdge {
                            host_exists: world.get_entity(edge.host).is_ok(),
                            top_slot_claim_count,
                            host_obligation_count,
                            host_children,
                            top_slot_parent,
                        },
                    ),
                ),
            );
        }

        let focus = quarantine_focus_closure(world, &attempt.evidence, &self.obligations);
        if !focus_is_released(world, &focus) {
            return Err(
                TargetPreparationPermanentSettlementError::SettlementRejected(
                    Self::target_preparation_permanent_boundary(
                        TargetPreparationPermanentSettlementFailureStage::FocusPrewrite,
                        TargetPreparationPermanentSettlementExpectation::FocusReleased {
                            targets: focus.clone(),
                        },
                        TargetPreparationPermanentSettlementObservation::Focus {
                            remaining: focused_targets(world, &focus),
                        },
                    ),
                ),
            );
        }

        let mut removals = Vec::new();
        removals
            .try_reserve_exact(attempt.evidence.len())
            .map_err(|_| {
                TargetPreparationPermanentSettlementError::SettlementRejected(
                    Self::target_preparation_permanent_boundary(
                        TargetPreparationPermanentSettlementFailureStage::CapacityPreparation,
                        TargetPreparationPermanentSettlementExpectation::PreparedCapacity {
                            requested: attempt.evidence.len(),
                        },
                        TargetPreparationPermanentSettlementObservation::CapacityRejected {
                            requested: attempt.evidence.len(),
                        },
                    ),
                )
            })?;
        for (index, evidence) in &attempt.evidence {
            let Some(obligation) = self.obligations.get(*index) else {
                return Err(
                    TargetPreparationPermanentSettlementError::SettlementRejected(
                        Self::target_preparation_permanent_boundary(
                            TargetPreparationPermanentSettlementFailureStage::ObligationPreflight {
                                index: *index,
                                entity: edge.top_slot,
                            },
                            TargetPreparationPermanentSettlementExpectation::ExactObligation {
                                index: *index,
                                identity: CleanupTargetIdentity::Registered(
                                    edge.top_slot_claim.clone(),
                                ),
                                expected_parent: edge.host,
                            },
                            TargetPreparationPermanentSettlementObservation::Obligation {
                                evidence: Some(evidence.clone()),
                                authority_exact: false,
                            },
                        ),
                    ),
                );
            };
            let authority_exact = cleanup_obligation_has_exact_authority(world, obligation);
            let CleanupEvidence::Exact {
                actual_parent,
                foreign_descendants,
                foreign_components,
                missing_components,
                protected_parent,
                relationship_cycle,
                ..
            } = evidence
            else {
                return Err(
                    TargetPreparationPermanentSettlementError::SettlementRejected(
                        Self::target_preparation_permanent_boundary(
                            TargetPreparationPermanentSettlementFailureStage::ObligationPreflight {
                                index: *index,
                                entity: obligation.entity(),
                            },
                            TargetPreparationPermanentSettlementExpectation::ExactObligation {
                                index: *index,
                                identity: obligation.identity.clone(),
                                expected_parent: obligation.expected_parent,
                            },
                            TargetPreparationPermanentSettlementObservation::Obligation {
                                evidence: Some(evidence.clone()),
                                authority_exact,
                            },
                        ),
                    ),
                );
            };
            if obligation.terminal_mode != CleanupTerminalMode::AuthorityExact
                || !foreign_descendants.is_empty()
                || !foreign_components.is_empty()
                || !missing_components.is_empty()
                || protected_parent.is_some()
                || *relationship_cycle
                || !authority_exact
            {
                return Err(
                    TargetPreparationPermanentSettlementError::SettlementRejected(
                        Self::target_preparation_permanent_boundary(
                            TargetPreparationPermanentSettlementFailureStage::ObligationPreflight {
                                index: *index,
                                entity: obligation.entity(),
                            },
                            TargetPreparationPermanentSettlementExpectation::ExactObligation {
                                index: *index,
                                identity: obligation.identity.clone(),
                                expected_parent: obligation.expected_parent,
                            },
                            TargetPreparationPermanentSettlementObservation::Obligation {
                                evidence: Some(evidence.clone()),
                                authority_exact,
                            },
                        ),
                    ),
                );
            }

            let disposition = if *actual_parent == Some(obligation.expected_parent) {
                CleanupDisposition::Removed
            } else {
                CleanupDisposition::RemovedDetached
            };
            let receipt = self
                .prepare_receipt_batch(
                    &attempt,
                    vec![CleanupReceipt::new(
                        self.ledger_domain,
                        attempt.revision,
                        *index,
                        disposition,
                        obligation,
                    )],
                )
                .ok_or_else(|| {
                    TargetPreparationPermanentSettlementError::ReceiptPreparationRejected(
                        Self::target_preparation_permanent_boundary(
                            TargetPreparationPermanentSettlementFailureStage::ReceiptPreparation {
                                index: *index,
                                entity: obligation.entity(),
                            },
                            TargetPreparationPermanentSettlementExpectation::VacantReceiptSlot {
                                index: *index,
                                identity: obligation.identity.clone(),
                            },
                            self.target_preparation_permanent_receipt_slot_observation(*index),
                        ),
                    )
                })?;
            let retirement = obligation.identity.registered().map(|claim| prepare_exact_registry_retirement(
                world.resource::<RouteEntityRegistry>(), vec![claim.clone()],
            )).transpose()
            .map_err(|_| {
                TargetPreparationPermanentSettlementError::SettlementRejected(
                    Self::target_preparation_permanent_boundary(
                        TargetPreparationPermanentSettlementFailureStage::RegistryRetirementPreparation {
                            index: *index,
                            entity: obligation.entity(),
                        },
                        TargetPreparationPermanentSettlementExpectation::ExactRegistryRetirement {
                            index: *index,
                            identity: obligation.identity.clone(),
                        },
                        TargetPreparationPermanentSettlementObservation::RegistryRetirementRejected {
                            observed_claim: world
                                .resource::<RouteEntityRegistry>()
                                .record(obligation.entity())
                                .map(|record| record.claim()),
                        },
                    ),
                )
            })?;
            removals.push(PreparedTargetPreparationPermanentRemoval {
                index: *index,
                entity: obligation.entity(),
                expected_parent: if obligation.identity.registered() == Some(edge.top_slot_claim) {
                    None
                } else {
                    *actual_parent
                },
                children_before: observed_children(world, obligation.entity()),
                retirement,
                receipt,
            });
        }

        let mut settled = Vec::new();
        settled.try_reserve_exact(removals.len()).map_err(|_| {
            TargetPreparationPermanentSettlementError::SettlementRejected(
                Self::target_preparation_permanent_boundary(
                    TargetPreparationPermanentSettlementFailureStage::CapacityPreparation,
                    TargetPreparationPermanentSettlementExpectation::PreparedCapacity {
                        requested: removals.len(),
                    },
                    TargetPreparationPermanentSettlementObservation::CapacityRejected {
                        requested: removals.len(),
                    },
                ),
            )
        })?;

        world.entity_mut(edge.top_slot).remove::<ChildOf>();
        world.flush();
        refreeze(
            world,
            TargetPreparationPermanentTransition {
                stage: TargetPreparationPermanentTransitionStage::HostEdgeDetached,
                settled: &settled,
                in_flight: None,
            },
        )
        .map_err(TargetPreparationPermanentSettlementError::PostWriteRefreeze)?;

        for removal in removals {
            let Some(obligation) = self.obligations.get(removal.index) else {
                return Err(
                    TargetPreparationPermanentSettlementError::SettlementRejected(
                        Self::target_preparation_permanent_boundary(
                            TargetPreparationPermanentSettlementFailureStage::RemovalPrewrite {
                                index: removal.index,
                                entity: removal.entity,
                            },
                            TargetPreparationPermanentSettlementExpectation::RemovableObligation {
                                index: removal.index,
                                identity: CleanupTargetIdentity::Registered(
                                    edge.top_slot_claim.clone(),
                                ),
                                expected_parent: removal.expected_parent,
                                expected_children: removal.children_before.clone(),
                            },
                            TargetPreparationPermanentSettlementObservation::Removal {
                                entity_exists: world.get_entity(removal.entity).is_ok(),
                                observed_claim: None,
                                parent: world.get::<ChildOf>(removal.entity).map(ChildOf::parent),
                                children: observed_children(world, removal.entity),
                                authority_exact: false,
                            },
                        ),
                    ),
                );
            };
            if !removal.is_current(world, obligation, &settled) {
                return Err(
                    TargetPreparationPermanentSettlementError::SettlementRejected(
                        target_preparation_permanent_removal_evidence(
                            world,
                            TargetPreparationPermanentSettlementFailureStage::RemovalPrewrite {
                                index: removal.index,
                                entity: removal.entity,
                            },
                            removal.index,
                            obligation,
                            removal.expected_parent,
                            &removal.children_before,
                        ),
                    ),
                );
            }
            if !world.despawn(removal.entity) {
                return Err(
                    TargetPreparationPermanentSettlementError::SettlementRejected(
                        target_preparation_permanent_removal_evidence(
                            world,
                            TargetPreparationPermanentSettlementFailureStage::RemovalPrewrite {
                                index: removal.index,
                                entity: removal.entity,
                            },
                            removal.index,
                            obligation,
                            removal.expected_parent,
                            &removal.children_before,
                        ),
                    ),
                );
            }

            world.flush();
            refreeze(
                world,
                TargetPreparationPermanentTransition {
                    stage: TargetPreparationPermanentTransitionStage::PhysicalAbsent(
                        removal.entity,
                    ),
                    settled: &settled,
                    in_flight: Some(removal.entity),
                },
            )
            .map_err(TargetPreparationPermanentSettlementError::PostWriteRefreeze)?;

            if let Some(retirement) = removal.retirement {
                apply_prepared_exact_registry_retirement(
                    &mut world.resource_mut::<RouteEntityRegistry>(),
                    retirement,
                );
            }
            self.commit_receipt_batch(removal.receipt);
            settled.push(removal.entity);
            refreeze(
                world,
                TargetPreparationPermanentTransition {
                    stage: TargetPreparationPermanentTransitionStage::ReceiptCommitted(
                        removal.entity,
                    ),
                    settled: &settled,
                    in_flight: None,
                },
            )
            .map_err(TargetPreparationPermanentSettlementError::PostWriteRefreeze)?;
        }

        if !focus_is_released(world, &focus)
            || observed_children(world, edge.host) != edge.children_after
            || self.receipts.iter().any(Option::is_none)
        {
            return Err(
                TargetPreparationPermanentSettlementError::SettlementRejected(
                    Self::target_preparation_permanent_boundary(
                        TargetPreparationPermanentSettlementFailureStage::Completion,
                        TargetPreparationPermanentSettlementExpectation::FullySettled {
                            manifest_digest: self.manifest_digest,
                            obligation_count: self.obligations.len(),
                            host: edge.host,
                            host_children_after: edge.children_after.to_vec(),
                        },
                        TargetPreparationPermanentSettlementObservation::Completion {
                            remaining_focus: focused_targets(world, &focus),
                            host_children: observed_children(world, edge.host),
                            receipt_count: self.receipts.iter().flatten().count(),
                            missing_receipts: self
                                .receipts
                                .iter()
                                .enumerate()
                                .filter_map(|(index, receipt)| receipt.is_none().then_some(index))
                                .collect(),
                            completion_digest: self.completion_digest,
                        },
                    ),
                ),
            );
        }
        self.finish_if_settled();
        if self.completion_digest.is_some() {
            Ok(TargetPreparationPermanentCompletionDomain {
                ledger_domain: self.ledger_domain,
            })
        } else {
            Err(
                TargetPreparationPermanentSettlementError::SettlementRejected(
                    Self::target_preparation_permanent_boundary(
                        TargetPreparationPermanentSettlementFailureStage::Completion,
                        TargetPreparationPermanentSettlementExpectation::FullySettled {
                            manifest_digest: self.manifest_digest,
                            obligation_count: self.obligations.len(),
                            host: edge.host,
                            host_children_after: edge.children_after.to_vec(),
                        },
                        TargetPreparationPermanentSettlementObservation::Completion {
                            remaining_focus: focused_targets(world, &focus),
                            host_children: observed_children(world, edge.host),
                            receipt_count: self.receipts.iter().flatten().count(),
                            missing_receipts: self
                                .receipts
                                .iter()
                                .enumerate()
                                .filter_map(|(index, receipt)| receipt.is_none().then_some(index))
                                .collect(),
                            completion_digest: self.completion_digest,
                        },
                    ),
                ),
            )
        }
    }

    pub(crate) fn settle(&mut self, world: &mut World, attempt: CleanupAttempt) -> bool {
        self.settle_with_failure_source(world, attempt, None, None)
    }

    pub(crate) fn settle_failure_cleanup(
        &mut self,
        world: &mut World,
        attempt: CleanupAttempt,
        source: &FailureCleanupSourceProjection,
    ) -> bool {
        self.settle_with_failure_source(world, attempt, Some(source), None)
    }

    fn retain_cleanup_settlement_permanent_evidence(&mut self, world: &World) {
        if self.stopped.is_some() {
            return;
        }
        if let Some(pending) = self.cleanup_settlement_permanent_pending.take() {
            match CleanupSettlementPermanentEvidence::freeze(
                world,
                self.ledger_domain,
                self.manifest_digest,
                self.obligations.len(),
                *pending,
            ) {
                Ok(evidence) => self.stopped = Some(CleanupStopReason::Settlement(evidence)),
                Err(cause) => {
                    self.cleanup_settlement_permanent_pending = Some(Box::new(cause));
                }
            }
            return;
        }
        let Some(expected) = self.quarantine_detachment.take() else {
            return;
        };
        let Some(registry) = world.get_resource::<RouteEntityRegistry>() else {
            self.quarantine_detachment = Some(expected);
            return;
        };
        let consumed_projection_matches = expected.matches(world);
        let mut unconsumed_mount_parent_drifts = Vec::new();
        if unconsumed_mount_parent_drifts
            .try_reserve_exact(self.mount_parents.len())
            .is_err()
        {
            self.quarantine_detachment = Some(expected);
            return;
        }
        for parent in &self.mount_parents {
            if expected.contains_parent(parent)
                || mount_parent_frozen_manifest_matches(registry, parent)
                    && mount_parent_physical_matches(world, registry, parent)
            {
                continue;
            }
            let Ok(expected_parent) = parent.try_clone() else {
                self.quarantine_detachment = Some(expected);
                return;
            };
            unconsumed_mount_parent_drifts.push(FrozenCleanupMountParentDrift {
                expected: expected_parent,
                observed_parent: FrozenFailureCleanupObservedEntity::observe(
                    world,
                    registry,
                    parent.physical,
                ),
                observed_removed_child: FrozenFailureCleanupObservedEntity::observe(
                    world,
                    registry,
                    parent.removed_child,
                ),
            });
        }
        if consumed_projection_matches && unconsumed_mount_parent_drifts.is_empty() {
            self.quarantine_detachment = Some(expected);
            return;
        }
        let cause = CleanupSettlementPermanentCause::DetachmentState {
            expected,
            consumed_projection_matches,
            unconsumed_mount_parent_drifts,
        };
        match CleanupSettlementPermanentEvidence::freeze(
            world,
            self.ledger_domain,
            self.manifest_digest,
            self.obligations.len(),
            cause,
        ) {
            Ok(evidence) => self.stopped = Some(CleanupStopReason::Settlement(evidence)),
            Err(cause) => {
                self.cleanup_settlement_permanent_pending = Some(Box::new(cause));
            }
        }
    }

    fn settle_with_failure_source(
        &mut self,
        world: &mut World,
        attempt: CleanupAttempt,
        failure_source: Option<&FailureCleanupSourceProjection>,
        inherited_quarantine: Option<CleanupDisposition>,
    ) -> bool {
        if self.issued_attempt.as_ref() != Some(&attempt)
            || attempt.ledger_domain != self.ledger_domain
            || attempt.manifest_digest != self.manifest_digest
            || attempt.unsettled_digest
                != unsettled_digest(self.manifest_digest, &self.obligations, &self.receipts)
        {
            return false;
        }
        if self.cleanup_settlement_permanent_pending.is_some() {
            self.retain_cleanup_settlement_permanent_evidence(world);
            self.issued_attempt = None;
            return false;
        }
        if self.back_no_target_authority_partition.is_some()
            && !self.back_no_target_authority_partition_is_current(world)
        {
            self.issued_attempt = None;
            return false;
        }
        if !self.prepare_related_protected(world) {
            self.issued_attempt = None;
            return false;
        }
        if !self.related_protected_is_current(world) {
            self.retain_related_protected_drift(world);
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
            self.retain_cleanup_settlement_permanent_evidence(world);
            self.issued_attempt = None;
            return false;
        }
        let focus_closure = self.focus_closure_for_attempt(
            world,
            quarantine_focus_closure(world, &current_evidence, &self.obligations),
        );

        if self.is_failed_prepared()
            && self.failure_cleanup_partition.is_some()
            && let Some(settled) = self.settle_failed_prepared_exact(
                world,
                &attempt,
                &current_evidence,
                &focus_closure,
                failure_source,
            )
        {
            return settled;
        }

        if self.is_physical_authority_settlement() {
            if !self.settle_authority_evidence(world, &attempt, focus_closure) {
                self.issued_attempt = None;
                return false;
            }
            self.post_settlement_focus.clear();
            self.finish_if_settled();
            return self.completion_digest.is_some();
        }

        release_cleanup_focus(world, &self.current_focus_closure(world, &focus_closure));

        if let Some(disposition) = inherited_quarantine
            .or_else(|| quarantine_disposition(&attempt.evidence, &self.obligations))
        {
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
            if !focus_is_released(world, &self.current_focus_closure(world, &focus_closure)) {
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

        let Some(mut plan) =
            PreparedCleanupRemovalPlan::prepare(world, self, &attempt, &focus_closure)
        else {
            self.issued_attempt = None;
            return false;
        };
        if !self.related_protected_is_current(world) {
            self.retain_related_protected_drift(world);
            self.issued_attempt = None;
            return false;
        }
        if !plan.targets_are_current(world, self)
            || !self.mount_parents_physical_match(world)
            || !plan.current_focus_targets(world).is_empty()
        {
            self.issued_attempt = None;
            return false;
        }
        for position in 0..plan.order.len() {
            let row_index = plan.order[position];
            let row = &plan.targets[row_index];
            let index = row.index;
            if !row.absent {
                if !world.despawn(row.entity) {
                    self.issued_attempt = None;
                    return false;
                }
                world.flush();
                release_cleanup_focus(world, &plan.current_focus_targets(world));
            }
            if let Some(claim) = self.obligations[index].identity.registered() {
                let registry_has_exact = world
                    .resource::<RouteEntityRegistry>()
                    .record(claim.entity())
                    .is_some_and(|record| record.claim() == *claim);
                if (!row.absent || registry_has_exact)
                    && !world
                        .resource_mut::<RouteEntityRegistry>()
                        .remove_exact(claim)
                {
                    self.issued_attempt = None;
                    return false;
                }
            }
            self.commit_receipt_batch(
                plan.targets[row_index]
                    .receipt
                    .take()
                    .expect("each prepared removal is consumed once"),
            );
            if !self.related_protected_is_current(world) {
                self.retain_related_protected_drift(world);
                self.issued_attempt = None;
                return false;
            }
            if !plan.current_focus_targets(world).is_empty()
                || !self.mount_parents_physical_match(world)
                || !plan.targets_are_current(world, self)
            {
                self.issued_attempt = None;
                return false;
            }
        }
        if self.mount_parent_settlement_required() && !self.settle_mount_parents(world) {
            self.issued_attempt = None;
            return false;
        }
        if !plan.current_focus_targets(world).is_empty() {
            self.issued_attempt = None;
            return false;
        }
        self.post_settlement_focus.clear();
        self.release_settled_owner();
        self.finish_if_settled();
        self.completion_digest.is_some()
    }

    fn retain_failure_cleanup_refreeze_stop(
        &mut self,
        partition: FrozenFailureCleanupPartition,
        evidence: FailureCleanupSettlementRefreezeEvidence,
    ) -> bool {
        self.failure_cleanup_partition = Some(partition);
        self.stopped = Some(CleanupStopReason::Refreeze(evidence));
        self.issued_attempt = None;
        false
    }

    fn settle_failed_prepared_exact(
        &mut self,
        world: &mut World,
        attempt: &CleanupAttempt,
        current_evidence: &[(usize, CleanupEvidence)],
        focus_closure: &[Entity],
        current_source: Option<&FailureCleanupSourceProjection>,
    ) -> Option<bool> {
        if quarantine_disposition(current_evidence, &self.obligations).is_some() {
            return None;
        }
        if current_evidence.iter().any(|(_, evidence)| {
            !matches!(
                evidence,
                CleanupEvidence::AlreadyAbsent | CleanupEvidence::Exact { .. }
            )
        }) {
            return None;
        }
        let partition = self.failure_cleanup_partition.take()?;
        let Some((first_index, _)) = current_evidence.first() else {
            self.failure_cleanup_partition = Some(partition);
            return None;
        };
        let first_entity = self.obligations[*first_index].entity();
        if let Err(evidence) = partition.refreeze(
            world,
            current_source,
            &self.obligations,
            &self.mount_parents,
            &self.receipts,
            self.manifest_digest,
            None,
            FailureCleanupTransitionStage::Prewrite(first_entity),
        ) {
            return Some(self.retain_failure_cleanup_refreeze_stop(partition, evidence));
        }
        release_cleanup_focus(world, &self.current_focus_closure(world, focus_closure));
        let Some(mount_parent_exchanges) =
            self.prepare_failure_cleanup_mount_parent_exchanges(world)
        else {
            self.issued_attempt = None;
            self.failure_cleanup_partition = Some(partition);
            return Some(false);
        };
        let mut removals = Vec::new();
        if removals.try_reserve_exact(current_evidence.len()).is_err() {
            self.issued_attempt = None;
            self.failure_cleanup_partition = Some(partition);
            return Some(false);
        }
        for (index, evidence) in current_evidence {
            let obligation = &self.obligations[*index];
            let (action, disposition, retirement) = match evidence {
                CleanupEvidence::AlreadyAbsent => {
                    let retirement = match world
                        .resource::<RouteEntityRegistry>()
                        .record(obligation.entity())
                    {
                        Some(record)
                            if obligation.identity.registered() == Some(&record.claim()) =>
                        {
                            match prepare_exact_registry_retirement(
                                world.resource::<RouteEntityRegistry>(),
                                vec![record.claim()],
                            ) {
                                Ok(retirement) => Some(retirement),
                                Err(_) => {
                                    self.issued_attempt = None;
                                    self.failure_cleanup_partition = Some(partition);
                                    return Some(false);
                                }
                            }
                        }
                        None => None,
                        Some(_) => {
                            self.issued_attempt = None;
                            self.failure_cleanup_partition = Some(partition);
                            return Some(false);
                        }
                    };
                    (
                        FailureCleanupRemovalAction::AlreadyAbsent,
                        CleanupDisposition::AlreadyAbsent,
                        retirement,
                    )
                }
                CleanupEvidence::Exact { actual_parent, .. } => {
                    let disposition = if *actual_parent == Some(obligation.expected_parent) {
                        CleanupDisposition::Removed
                    } else {
                        CleanupDisposition::RemovedDetached
                    };
                    let retirement = if let Some(claim) = obligation.identity.registered() {
                        match prepare_exact_registry_retirement(
                            world.resource::<RouteEntityRegistry>(),
                            vec![claim.clone()],
                        ) {
                            Ok(retirement) => Some(retirement),
                            Err(_) => {
                                self.issued_attempt = None;
                                self.failure_cleanup_partition = Some(partition);
                                return Some(false);
                            }
                        }
                    } else {
                        None
                    };
                    (
                        FailureCleanupRemovalAction::Despawn,
                        disposition,
                        retirement,
                    )
                }
                CleanupEvidence::AuthorityLostResidual { .. } | CleanupEvidence::StaleClaim => {
                    unreachable!("the failed exact path prevalidates its evidence kinds")
                }
            };
            let Some(receipt) = self.prepare_receipt_batch(
                attempt,
                vec![CleanupReceipt::new(
                    self.ledger_domain,
                    attempt.revision,
                    *index,
                    disposition,
                    obligation,
                )],
            ) else {
                self.issued_attempt = None;
                self.failure_cleanup_partition = Some(partition);
                return Some(false);
            };
            removals.push(PreparedFailureCleanupRemoval {
                index: *index,
                entity: obligation.entity(),
                action,
                retirement,
                receipt,
            });
        }

        for removal in removals {
            if let Err(evidence) = partition.refreeze(
                world,
                current_source,
                &self.obligations,
                &self.mount_parents,
                &self.receipts,
                self.manifest_digest,
                None,
                FailureCleanupTransitionStage::Prewrite(removal.entity),
            ) {
                return Some(self.retain_failure_cleanup_refreeze_stop(partition, evidence));
            }
            let planned = attempt
                .evidence
                .iter()
                .find_map(|(index, evidence)| (*index == removal.index).then_some(evidence));
            if planned.is_none_or(|planned| {
                &classify_obligation(world, &self.obligations, &self.receipts, removal.index)
                    != planned
            }) {
                self.issued_attempt = None;
                self.failure_cleanup_partition = Some(partition);
                return Some(false);
            }
            if removal.action == FailureCleanupRemovalAction::Despawn {
                if !world.despawn(removal.entity) {
                    self.issued_attempt = None;
                    self.failure_cleanup_partition = Some(partition);
                    return Some(false);
                }
                world.flush();
                if let Err(evidence) = partition.refreeze(
                    world,
                    current_source,
                    &self.obligations,
                    &self.mount_parents,
                    &self.receipts,
                    self.manifest_digest,
                    None,
                    FailureCleanupTransitionStage::PhysicalAbsent(removal.entity),
                ) {
                    return Some(self.retain_failure_cleanup_refreeze_stop(partition, evidence));
                }
            }
            if let Some(retirement) = removal.retirement {
                apply_prepared_exact_registry_retirement(
                    &mut world.resource_mut::<RouteEntityRegistry>(),
                    retirement,
                );
            }
            self.commit_receipt_batch(removal.receipt);
            if let Err(evidence) = partition.refreeze(
                world,
                current_source,
                &self.obligations,
                &self.mount_parents,
                &self.receipts,
                self.manifest_digest,
                None,
                FailureCleanupTransitionStage::ReceiptCommitted(removal.entity),
            ) {
                return Some(self.retain_failure_cleanup_refreeze_stop(partition, evidence));
            }
        }
        let mount_parent_next =
            Self::apply_failure_cleanup_mount_parent_exchanges(world, mount_parent_exchanges);
        if let Err(evidence) = partition.refreeze(
            world,
            current_source,
            &self.obligations,
            &self.mount_parents,
            &self.receipts,
            self.manifest_digest,
            Some(&mount_parent_next),
            FailureCleanupTransitionStage::MountParentsCommitted,
        ) {
            return Some(self.retain_failure_cleanup_refreeze_stop(partition, evidence));
        }
        if !focus_is_released(world, focus_closure) {
            self.issued_attempt = None;
            self.failure_cleanup_partition = Some(partition);
            return Some(false);
        }
        self.post_settlement_focus.clear();
        self.finish_if_settled();
        self.failure_cleanup_partition = Some(partition);
        Some(self.completion_digest.is_some())
    }

    fn prepare_authority_host_edge(
        &self,
        world: &World,
        attempt: &CleanupAttempt,
    ) -> Result<PreparedAuthorityHostEdge, ()> {
        let owner = match self.owner.as_ref() {
            Some(CleanupOwner::AuthoritySettlement(owner)) => &owner.host,
            Some(CleanupOwner::BackNoTargetAuthority(owner)) => &owner.host,
            Some(CleanupOwner::IdleHostAuthority(owner)) => &owner.host,
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
            .position(|obligation| obligation.identity.registered() == Some(claim))
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
                ui_detachment: DetachedUiInheritance::default(),
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
            ui_detachment: DetachedUiInheritance::capture(world, top_slot),
        })
    }

    fn settle_authority_evidence(
        &mut self,
        world: &mut World,
        attempt: &CleanupAttempt,
        focus_closure: Vec<Entity>,
    ) -> bool {
        self.settle_authority_evidence_typed(world, attempt, focus_closure)
            .is_ok()
    }

    fn settle_authority_evidence_typed(
        &mut self,
        world: &mut World,
        attempt: &CleanupAttempt,
        focus_closure: Vec<Entity>,
    ) -> Result<(), AuthorityCleanupFailureKind> {
        debug_assert!(self.mount_parents.is_empty());
        let mut prepared =
            self.prepare_authority_settlement_typed(world, attempt, &focus_closure)?;
        if self.issued_attempt.as_ref() != Some(attempt)
            || attempt.unsettled_digest
                != unsettled_digest(self.manifest_digest, &self.obligations, &self.receipts)
            || !prepared
                .publication_capacity
                .revalidates(world.resource::<QuarantineLeaseTable>())
            || collect_evidence(world, &self.obligations, &self.receipts)
                != prepared.prepublication_evidence
        {
            return Err(AuthorityCleanupFailureKind::Attempt);
        }
        if let Some(partition) = self.back_no_target_authority_partition.as_mut() {
            if !partition.install_terminal_projections(
                &self.obligations,
                std::mem::take(&mut prepared.terminal_projections),
            ) {
                return Err(AuthorityCleanupFailureKind::TerminalProjection);
            }
            if !self.back_no_target_authority_partition_is_current(world) {
                return Err(AuthorityCleanupFailureKind::Partition);
            }
        }
        #[cfg(test)]
        let _mutation_marker = crate::registry::begin_authority_publication_mutation_for_test();
        if !prepared.pickable_repair.apply(world) {
            return Err(AuthorityCleanupFailureKind::PickableWrite);
        }
        if self.back_no_target_authority_partition.is_some()
            && !self.back_no_target_authority_partition_is_current(world)
        {
            return Err(AuthorityCleanupFailureKind::Partition);
        }
        if !prepared.host_edge.apply_and_validate(world) {
            return Err(AuthorityCleanupFailureKind::HostEdgeWrite);
        }
        if let Some(partition) = self.back_no_target_authority_partition.as_mut()
            && !partition.commit_host_edge_phase(prepared.host_edge.partition_phase(), world)
        {
            return Err(AuthorityCleanupFailureKind::HostEdgePhase);
        }
        if self.back_no_target_authority_partition.is_some()
            && !self.back_no_target_authority_partition_is_current(world)
        {
            return Err(AuthorityCleanupFailureKind::Partition);
        }
        release_cleanup_focus(world, &self.current_focus_closure(world, &focus_closure));
        if !focus_is_released(world, &self.current_focus_closure(world, &focus_closure)) {
            return Err(AuthorityCleanupFailureKind::Focus);
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
                return Err(AuthorityCleanupFailureKind::AbsentRetirement);
            }
        }

        let mut removals = prepared.removed.into_iter();
        while let Some(removal) = removals.next() {
            let completed_count = prepared.removal_order.len() - removals.len() - 1;
            let completed_before = &prepared.removal_order[..completed_count];
            let entity = self.obligations[removal.index].entity();
            if !removal.is_current(world, &self.obligations, completed_before)
                || !world.despawn(entity)
            {
                return Err(AuthorityCleanupFailureKind::RemovalPreflight);
            }
            world.flush();
            apply_authority_removal_postwrite_drift(world);
            if let Some(retirement) = removal.retirement {
                apply_prepared_exact_registry_retirement(
                    &mut world.resource_mut::<RouteEntityRegistry>(),
                    retirement,
                );
            }
            let removed_is_terminal = world.get_entity(entity).is_err()
                && world
                    .resource::<RouteEntityRegistry>()
                    .record(entity)
                    .is_none();
            if !removed_is_terminal {
                return Err(AuthorityCleanupFailureKind::RemovalTerminal);
            }
            release_cleanup_focus(world, &self.current_focus_closure(world, &focus_closure));
            self.commit_receipt_batch(removal.receipt);
            if self.back_no_target_authority_partition.is_some()
                && !self.back_no_target_authority_partition_is_current(world)
            {
                return Err(AuthorityCleanupFailureKind::Partition);
            }
            let completed_after = &prepared.removal_order[..=completed_count];
            if !focus_is_released(world, &self.current_focus_closure(world, &focus_closure))
                || !removals
                    .as_slice()
                    .iter()
                    .all(|removal| removal.is_current(world, &self.obligations, completed_after))
            {
                return Err(AuthorityCleanupFailureKind::RemainingRemovals);
            }
        }

        for residual in prepared.residuals {
            if !residual.is_current(world) {
                return Err(AuthorityCleanupFailureKind::ResidualPreflight);
            }
            let indices = residual.indices;
            release_cleanup_focus(world, &self.current_focus_closure(world, &focus_closure));
            residual.presentation.apply(world);
            apply_authority_registry_transfers(world, residual.transfers);
            self.commit_receipt_batch(residual.receipts);
            let Some(focus_roots) = self.terminal_focus_roots(world, &indices) else {
                return Err(AuthorityCleanupFailureKind::ResidualFocus);
            };
            self.remember_post_settlement_focus(focus_roots);
            apply_authority_residual_postwrite_drift(world);
            if self.back_no_target_authority_partition.is_some()
                && !self.back_no_target_authority_partition_is_current(world)
            {
                return Err(AuthorityCleanupFailureKind::Partition);
            }
            if !focus_is_released(world, &self.current_focus_closure(world, &focus_closure)) {
                return Err(AuthorityCleanupFailureKind::ResidualFocus);
            }
        }
        Ok(())
    }

    fn prepare_authority_settlement_typed(
        &self,
        world: &mut World,
        attempt: &CleanupAttempt,
        _focus_closure: &[Entity],
    ) -> Result<PreparedAuthoritySettlement, AuthorityCleanupFailureKind> {
        let mut failure = AuthorityCleanupFailureKind::Preparation;
        let prepared = (|| -> Option<PreparedAuthoritySettlement> {
            let host_edge = self.prepare_authority_host_edge(world, attempt).ok()?;
            let projected_attempt = host_edge.project_attempt(attempt)?;
            let dispositions = authority_settlement_dispositions(
                world,
                &self.obligations,
                &projected_attempt.evidence,
            )
            .map_err(|kind| {
                failure = kind;
            })
            .ok()?;
            let groups = authority_residual_groups(world, &self.obligations, &dispositions)
                .map_err(|kind| {
                    failure = kind;
                })
                .ok()?;
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
            let publication_capacity = PreparedAuthorityPublicationCapacity::reserve_typed(
                &mut world.resource_mut::<QuarantineLeaseTable>(),
                verified_group_count,
                lost_fact_count,
                groups.len(),
            )
            .map_err(|error| {
                failure = match error {
                    crate::registry::AuthorityPublicationReserveError::RetryableCapacity => {
                        AuthorityCleanupFailureKind::RetryableCapacity
                    }
                    crate::registry::AuthorityPublicationReserveError::IdentityExhausted => {
                        AuthorityCleanupFailureKind::IdentityExhausted
                    }
                    crate::registry::AuthorityPublicationReserveError::InvalidGroups => {
                        AuthorityCleanupFailureKind::Preparation
                    }
                };
            })
            .ok()?;
            let verified_claims = dispositions
                .iter()
                .filter(|plan| plan.action == AuthoritySettlementAction::VerifiedResidual)
                .filter_map(|plan| self.obligations[plan.index].identity.registered().cloned())
                .collect::<Vec<_>>();
            let pickable_repair =
                PreparedAuthorityPickableRepair::prepare(world, verified_claims.clone())
                    .map_err(|kind| {
                        failure = kind;
                    })
                    .ok()?;
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
                            .is_some_and(|obligation| pickable_repair.repairs(obligation.entity()))
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
                .map(|plan| self.obligations[plan.index].entity())
                .collect::<HashSet<_>>();

            let mut absent = Vec::new();
            let mut removed = Vec::new();
            absent
                .try_reserve_exact(dispositions.len())
                .map_err(|_| {
                    failure = AuthorityCleanupFailureKind::RetryableCapacity;
                })
                .ok()?;
            removed
                .try_reserve_exact(dispositions.len())
                .map_err(|_| {
                    failure = AuthorityCleanupFailureKind::RetryableCapacity;
                })
                .ok()?;
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
                    .record(obligation.entity())
                    .is_some_and(|record| {
                        obligation.identity.registered() == Some(&record.claim())
                    });
                let retirement = obligation
                    .identity
                    .registered()
                    .filter(|_| registry_exact)
                    .and_then(|claim| {
                        prepare_exact_registry_retirement(
                            world.resource::<RouteEntityRegistry>(),
                            vec![claim.clone()],
                        )
                        .ok()
                    });
                if plan.action == AuthoritySettlementAction::Remove
                    && obligation.identity.registered().is_some()
                    && retirement.is_none()
                {
                    return None;
                }
                let expected_parent = if plan.action == AuthoritySettlementAction::Remove {
                    let (_, CleanupEvidence::Exact { actual_parent, .. }) = projected_evidence
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
                    children_before: observed_children(world, obligation.entity()),
                    receipt,
                    retirement,
                    manifest_before: ComponentManifest::from_world(world, obligation.entity()),
                    parent_detached: host_edge.removes_child_of(obligation.entity()),
                    ui_detachment: host_edge.ui_detachment_for(obligation.entity()),
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
                    self.obligations[plan.index].entity(),
                ))
            });
            let mut removal_order = Vec::new();
            removal_order
                .try_reserve_exact(removed.len())
                .map_err(|_| {
                    failure = AuthorityCleanupFailureKind::RetryableCapacity;
                })
                .ok()?;
            removal_order.extend(
                removed
                    .iter()
                    .map(|removal| self.obligations[removal.index].entity()),
            );

            let mut residuals = Vec::new();
            residuals
                .try_reserve_exact(groups.len())
                .map_err(|_| {
                    failure = AuthorityCleanupFailureKind::RetryableCapacity;
                })
                .ok()?;
            let mut verified_ordinal = 0usize;
            let mut lost_offset = 0usize;
            for indices in groups {
                let mut receipt_values = Vec::new();
                receipt_values
                    .try_reserve_exact(indices.len())
                    .map_err(|_| {
                        failure = AuthorityCleanupFailureKind::RetryableCapacity;
                    })
                    .ok()?;
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
                let lost = self
                    .prepare_authority_lost_projection_typed(
                        world,
                        attempt,
                        &receipts,
                        &lost_indices,
                    )
                    .map_err(|kind| {
                        failure = kind;
                    })
                    .ok()?;
                let claims = indices
                    .iter()
                    .filter(|index| {
                        dispositions.iter().any(|plan| {
                            plan.index == **index
                                && plan.action == AuthoritySettlementAction::VerifiedResidual
                        })
                    })
                    .filter_map(|index| self.obligations[*index].identity.registered().cloned())
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
                group_evidence
                    .try_reserve_exact(indices.len())
                    .map_err(|_| {
                        failure = AuthorityCleanupFailureKind::RetryableCapacity;
                    })
                    .ok()?;
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
                let verified_indices = indices
                    .iter()
                    .copied()
                    .filter(|index| {
                        dispositions.iter().any(|plan| {
                            plan.index == *index
                                && plan.action == AuthoritySettlementAction::VerifiedResidual
                        })
                    })
                    .collect::<Vec<_>>();
                let contained =
                    self.contained_residual_facts(attempt, &receipts, &verified_indices);
                let transfers = prepare_authority_registry_transfers(
                    world,
                    &publication_capacity,
                    &mut verified_ordinal,
                    &mut lost_offset,
                    claims,
                    manifests.clone(),
                    contained,
                    lost,
                    quarantine_facts(&group_evidence),
                )?;
                let mut manifests = manifests;
                for index in verified_indices {
                    let obligation = &self.obligations[index];
                    if obligation.identity.registered().is_none() {
                        manifests.push((
                            obligation.entity(),
                            authority_projected_manifest(
                                world,
                                obligation.entity(),
                                &host_edge,
                                &removal_entities,
                                &pickable_repair,
                            )?,
                        ));
                    }
                }
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
                        let entity = obligation.entity();
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
                                            .then_some(self.obligations[*index].entity())
                                    })
                                    .collect::<Vec<_>>();
                                lease_entities.sort_by_key(|entity| entity.to_bits());
                                let manifest = residual.manifests.iter().find_map(
                                    |(candidate, manifest)| {
                                        (*candidate == entity).then_some(manifest.clone())
                                    },
                                )?;
                                let parent = authority_projected_parent(world, entity, &host_edge);
                                let children =
                                    authority_projected_children(world, entity, &removal_entities);
                                let ownership_seal =
                                    world.get::<RouteOwnershipSeal>(entity).copied();
                                let authority =
                                    if let CleanupTargetIdentity::ContainedContent { .. } =
                                        &obligation.identity
                                    {
                                        let record = residual
                                            .transfers
                                            .verified
                                            .as_ref()?
                                            .contained_records()
                                            .iter()
                                            .find(|record| {
                                                record.fact.identity == obligation.identity
                                            })?
                                            .clone();
                                        PreparedBackNoTargetAuthorityTerminalAuthority::Contained {
                                            lease,
                                            record,
                                        }
                                    } else if let CleanupTargetIdentity::Registered(claim) =
                                        &obligation.identity
                                    {
                                        PreparedBackNoTargetAuthorityTerminalAuthority::Quarantine {
                                            lease,
                                            lease_entities,
                                            owner: claim.owner().clone(),
                                            seal: claim.seal(),
                                            registered_manifest: world
                                                .resource::<RouteEntityRegistry>()
                                                .record(entity)?
                                                .manifest()
                                                .clone(),
                                            manifest: manifest.clone(),
                                        }
                                    } else {
                                        return None;
                                    };
                                (
                                    None,
                                    PreparedBackNoTargetAuthorityTerminalPresence::Retained {
                                        parent,
                                        children,
                                        ownership_seal,
                                        manifest,
                                    },
                                    authority,
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
                                    || projection.source_identity() != &obligation.identity
                                {
                                    return None;
                                }
                                let registry_claim = projection.observed_registry_claim().cloned();
                                let presence = if projection.entity_present_at_receipt() {
                                    PreparedBackNoTargetAuthorityTerminalPresence::Retained {
                                        parent: authority_projected_parent(
                                            world, entity, &host_edge,
                                        ),
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
                        let admission_ui_root_additions = if host_edge.removes_child_of(entity) {
                            match &presence {
                                PreparedBackNoTargetAuthorityTerminalPresence::Retained {
                                    manifest,
                                    ..
                                } => {
                                    FrozenRetainedUiRootAdditions::capture(world, entity, manifest)
                                }
                                PreparedBackNoTargetAuthorityTerminalPresence::Absent => {
                                    FrozenRetainedUiRootAdditions::default()
                                }
                            }
                        } else {
                            FrozenRetainedUiRootAdditions::default()
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
                            admission_ui_root_additions,
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
        })();
        prepared.ok_or(failure)
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
        self.quarantine_detachment
            .as_ref()
            .is_none_or(|detachment| detachment.matches(world))
            && self.mount_parents.iter().all(|parent| {
                if self
                    .quarantine_detachment
                    .as_ref()
                    .is_some_and(|detachment| detachment.contains_parent(parent))
                {
                    return true;
                }
                return mount_parent_frozen_manifest_matches(registry, parent)
                    && mount_parent_physical_matches(world, registry, parent);
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
                let Some(claim) = parent.bookkeeping_claim() else {
                    return Some(None);
                };
                let record = registry.record(parent.physical)?;
                if record.claim() != parent.claim || record.manifest() != &parent.manifest {
                    return None;
                }
                let actual = ComponentManifest::from_world(world, parent.physical)?;
                if !observed_parent_manifest_differs_only_by_children(
                    world,
                    parent.physical,
                    &parent.physical_manifest,
                ) {
                    return None;
                }
                let next = ComponentManifest::new(
                    parent
                        .next_manifest_base()
                        .component_ids()
                        .iter()
                        .copied()
                        .filter(|component| Some(*component) != children)
                        .chain(
                            children.filter(|component| actual.component_ids().contains(component)),
                        ),
                );
                Some(Some((claim.clone(), parent.manifest.clone(), next)))
            })
            .collect::<Option<Vec<_>>>()
            .map(|exchanges| exchanges.into_iter().flatten().collect())
    }

    fn prepare_failure_cleanup_mount_parent_exchanges(
        &self,
        world: &World,
    ) -> Option<PreparedFailureCleanupMountParentExchanges> {
        let children = world.components().component_id::<Children>();
        let registry = world.get_resource::<RouteEntityRegistry>()?;
        let mut apply = Vec::new();
        let mut expected = Vec::new();
        apply.try_reserve_exact(self.mount_parents.len()).ok()?;
        expected.try_reserve_exact(self.mount_parents.len()).ok()?;
        for parent in &self.mount_parents {
            if !mount_parent_physical_matches(world, registry, parent) {
                return None;
            }
            let Some(claim) = parent.bookkeeping_claim() else {
                continue;
            };
            let record = registry.record(parent.physical)?;
            if record.claim() != parent.claim || record.manifest() != &parent.manifest {
                return None;
            }
            let next = ComponentManifest::new(
                parent
                    .next_manifest_base()
                    .component_ids()
                    .iter()
                    .copied()
                    .filter(|component| Some(*component) != children)
                    .chain(children.filter(|_| !parent.children_after.is_empty())),
            );
            apply.push((claim.clone(), next.clone()));
            expected.push((claim.clone(), next));
        }
        Some(PreparedFailureCleanupMountParentExchanges { apply, expected })
    }

    fn apply_failure_cleanup_mount_parent_exchanges(
        world: &mut World,
        prepared: PreparedFailureCleanupMountParentExchanges,
    ) -> Vec<(ExactRegistryClaim, ComponentManifest)> {
        for (claim, next) in prepared.apply {
            let applied = world
                .resource_mut::<RouteEntityRegistry>()
                .compare_exchange_manifest(&claim, next);
            debug_assert_eq!(applied.as_ref(), Some(&claim));
        }
        prepared.expected
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
                .find(|parent| parent.physical == claim.entity())
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
                CleanupEvidence::AlreadyAbsent
                | CleanupEvidence::AuthorityLostResidual {
                    entity_present: false,
                    ..
                } => {
                    if let Some(claim) = self.obligations[*index].identity.registered() {
                        let registry_has_exact = world
                            .resource::<RouteEntityRegistry>()
                            .record(claim.entity())
                            .is_some_and(|record| record.claim() == *claim);
                        if registry_has_exact
                            && !world
                                .resource_mut::<RouteEntityRegistry>()
                                .remove_exact(claim)
                        {
                            return false;
                        }
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
                CleanupEvidence::Exact { .. }
                | CleanupEvidence::AuthorityLostResidual {
                    entity_present: true,
                    ..
                } => transfer_indices.push(*index),
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
                        ) || !cleanup_obligation_has_exact_authority(
                            world,
                            &self.obligations[*index],
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
            .filter_map(|index| self.obligations[*index].identity.registered().cloned())
            .collect::<Vec<_>>();
        let unresolved_claims = unresolved_indices
            .iter()
            .filter_map(|index| self.obligations[*index].identity.registered().cloned())
            .collect::<Vec<_>>();
        let facts = quarantine_facts(&attempt.evidence);
        let focus_closure = quarantine_focus_closure(world, &attempt.evidence, &self.obligations);
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
        let verified_content =
            self.contained_residual_facts(attempt, &quarantine_receipts, &verified_indices);
        let unresolved_content =
            self.contained_residual_facts(attempt, &quarantine_receipts, &unresolved_indices);
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
        let transferred = (repair_outcome == PickableRepairOutcome::ExactAfterRepair
            && unresolved_indices.iter().all(|index| {
                self.obligations[*index].terminal_mode == CleanupTerminalMode::AuthorityLost
                    || unresolved_obligation_is_isolated(world, &self.obligations[*index])
            }))
        .then(|| {
            transfer_cleanup_quarantine(
                world,
                verified_claims,
                unresolved_claims,
                verified_content,
                unresolved_content,
                lost_facts,
                facts,
                focus_closure.clone(),
                self,
                quarantine_receipts,
            )
        })
        .flatten();
        let Some(()) = transferred else {
            return false;
        };
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
            let entity = obligation.entity();
            if self
                .receipts
                .get(*index)
                .and_then(Option::as_ref)
                .is_some_and(|receipt| receipt.disposition == CleanupDisposition::RejectedProtected)
            {
                if table
                    .authority_lost_tombstone(entity)
                    .is_some_and(|tombstone| tombstone.source_identity() == &obligation.identity)
                    || table.unresolved_record(entity).is_some_and(|record| {
                        obligation.identity.registered() == Some(&record.claim())
                    })
                {
                    // A lost tombstone is not current authority and therefore
                    // must not become a post-settlement focus root.
                    continue;
                }
                return None;
            }
            let CleanupTargetIdentity::Registered(claim) = &obligation.identity else {
                let (_, record) = table.contained_record(entity)?;
                let receipt = self.receipts.get(*index)?.as_ref()?;
                if record.fact.identity != obligation.identity
                    || record.fact.receipt_binding != receipt.binding
                    || !contained_terminal_scope_is_current(world, &record.terminal_identity)
                {
                    return None;
                }
                // The exact root/shell obligations carry the focus boundary.
                continue;
            };
            let root = match (table.record(entity), table.unresolved_record(entity)) {
                (Some(record), None)
                    if record.owner() == claim.owner()
                        && record.seal() == claim.seal()
                        && record.lifecycle() == EntityLifecycle::Quarantined =>
                {
                    PostSettlementFocusRoot {
                        obligation: *index,
                        source_claim: claim.clone(),
                        terminal_claim: record.claim(),
                        authority: PostSettlementFocusAuthority::Quarantine,
                        allow_missing_components: obligation.allow_missing_components,
                    }
                }
                (None, Some(record))
                    if obligation.identity.registered() == Some(&record.claim()) =>
                {
                    PostSettlementFocusRoot {
                        obligation: *index,
                        source_claim: claim.clone(),
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

    fn contained_residual_facts(
        &self,
        attempt: &CleanupAttempt,
        receipts: &PreparedCleanupReceiptBatch,
        indices: &[usize],
    ) -> Vec<crate::registry::ContainedResidualFact> {
        indices
            .iter()
            .filter_map(|index| {
                let obligation = &self.obligations[*index];
                if obligation.identity.registered().is_some() {
                    return None;
                }
                let receipt = receipts
                    .receipts
                    .iter()
                    .find(|receipt| receipt.obligation == *index)?;
                Some(crate::registry::ContainedResidualFact {
                    identity: obligation.identity.clone(),
                    expected_parent: obligation.expected_parent,
                    ledger_domain: self.ledger_domain,
                    manifest_digest: self.manifest_digest,
                    attempt_revision: attempt.revision,
                    obligation: *index,
                    obligation_binding: cleanup_obligation_binding(obligation),
                    receipt_binding: receipt.binding,
                })
            })
            .collect()
    }

    fn prepare_authority_lost_projection(
        &self,
        world: &World,
        attempt: &CleanupAttempt,
        receipts: &PreparedCleanupReceiptBatch,
        indices: &[usize],
    ) -> Option<PreparedAuthorityLostResidualProjection> {
        self.prepare_authority_lost_projection_typed(world, attempt, receipts, indices)
            .ok()
    }

    fn prepare_authority_lost_projection_typed(
        &self,
        world: &World,
        attempt: &CleanupAttempt,
        receipts: &PreparedCleanupReceiptBatch,
        indices: &[usize],
    ) -> Result<PreparedAuthorityLostResidualProjection, AuthorityCleanupFailureKind> {
        if self.issued_attempt.as_ref() != Some(attempt) {
            return Err(AuthorityCleanupFailureKind::Preparation);
        }
        let mut facts = Vec::new();
        facts
            .try_reserve_exact(authority_preparation_capacity_input(
                AuthorityPreparationCapacitySite::LostProjection,
                indices.len(),
            ))
            .map_err(|_| AuthorityCleanupFailureKind::RetryableCapacity)?;
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
                .find(|(candidate, _)| candidate == index)
                .ok_or(AuthorityCleanupFailureKind::Preparation)?
            else {
                return Err(AuthorityCleanupFailureKind::Preparation);
            };
            let receipt = receipts
                .receipts
                .iter()
                .find(|receipt| receipt.obligation == *index)
                .ok_or(AuthorityCleanupFailureKind::Preparation)?;
            let obligation = self
                .obligations
                .get(*index)
                .ok_or(AuthorityCleanupFailureKind::Preparation)?;
            if receipt.disposition != CleanupDisposition::RejectedProtected {
                return Err(AuthorityCleanupFailureKind::Preparation);
            }
            let observed_registry_claim = world
                .resource::<RouteEntityRegistry>()
                .record(obligation.entity())
                .map(|record| record.claim())
                .filter(|claim| obligation.identity.registered() != Some(claim));
            facts.push(AuthorityLostResidualProjectionFact {
                ledger_domain: self.ledger_domain,
                manifest_digest: self.manifest_digest,
                attempt_revision: attempt.revision,
                obligation: *index,
                obligation_binding: cleanup_obligation_binding(obligation),
                terminal_receipt_binding: receipt.binding,
                retired_identity: obligation.identity.clone(),
                frozen_expected_manifest: obligation.manifest.clone(),
                observed_manifest: ComponentManifest::from_world(world, obligation.entity()),
                entity_present_at_receipt: *entity_present,
                observed_registry_claim,
                actual_parent_at_receipt: *actual_parent,
                registry_exact_at_receipt: *registry_exact,
            });
        }
        Ok(PreparedAuthorityLostResidualProjection { facts })
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

    #[cfg(test)]
    pub(crate) fn obligation_entities_for_test(&self) -> Vec<Entity> {
        self.obligations
            .iter()
            .map(|obligation| obligation.entity())
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn issued_attempt_present_for_test(&self) -> bool {
        self.issued_attempt.is_some()
    }

    #[cfg(test)]
    pub(crate) fn failure_cleanup_refreeze_evidence_for_test(
        &self,
    ) -> Option<&FailureCleanupSettlementRefreezeEvidence> {
        match self.stopped.as_ref() {
            Some(CleanupStopReason::Refreeze(evidence)) => Some(evidence),
            _ => None,
        }
    }

    #[cfg(test)]
    pub(crate) fn projection(&self) -> PreparedCleanupLedgerProjection {
        PreparedCleanupLedgerProjection {
            ledger_domain: self.ledger_domain,
            manifest_digest: self.manifest_digest,
            obligation_count: self.obligations.len(),
            receipt_count: self.receipts.iter().flatten().count(),
        }
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
        retiring: &[(&RetainedLeafProjection, &RealizedBranch)],
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

fn prepare_authority_registry_transfers(
    world: &mut World,
    capacity: &PreparedAuthorityPublicationCapacity,
    verified_ordinal: &mut usize,
    lost_offset: &mut usize,
    verified_claims: Vec<ExactRegistryClaim>,
    verified_manifests: Vec<(Entity, ComponentManifest)>,
    contained: Vec<crate::registry::ContainedResidualFact>,
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
                        let mut prepared = prepare_verified_authority_cleanup_quarantine_transfer(
                            &mut table, &registry, transfer, id,
                        )
                        .ok()?;
                        prepared.attach_contained(&registry, contained).ok()?;
                        Some(prepared)
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

fn relationship_manifest_after(
    manifest: &ComponentManifest,
    relationship: ComponentId,
    present: bool,
) -> ComponentManifest {
    ComponentManifest::new(
        manifest
            .component_ids()
            .iter()
            .copied()
            .filter(|component| *component != relationship)
            .chain(present.then_some(relationship)),
    )
}

fn prepare_cleanup_quarantine_detachment(
    world: &World,
    ledger: &CleanupLedger,
    verified_claims: &[ExactRegistryClaim],
    permitted: bool,
) -> Option<PreparedCleanupQuarantineDetachment> {
    let registry = world.get_resource::<RouteEntityRegistry>()?;
    if verified_claims
        .iter()
        .map(ExactRegistryClaim::entity)
        .collect::<HashSet<_>>()
        .len()
        != verified_claims.len()
    {
        return None;
    }
    let mut targets_after_inert = Vec::new();
    targets_after_inert
        .try_reserve_exact(verified_claims.len())
        .ok()?;
    for claim in verified_claims {
        if registry
            .record(claim.entity())
            .is_none_or(|record| record.claim() != *claim)
            || !cleanup_claim_has_exact_authority(world, claim)
        {
            return None;
        }
        let observed =
            FrozenFailureCleanupObservedEntity::observe(world, &registry, claim.entity());
        if !observed.present || observed.manifest.is_none() {
            return None;
        }
        targets_after_inert.push(observed);
    }

    let mut edges = Vec::new();
    edges.try_reserve_exact(ledger.mount_parents.len()).ok()?;
    if permitted {
        let children = world.components().component_id::<Children>()?;
        let child_of = world.components().component_id::<ChildOf>()?;
        for parent in &ledger.mount_parents {
            if ledger
                .quarantine_detachment
                .as_ref()
                .is_some_and(|detachment| detachment.contains_parent(parent))
            {
                continue;
            }
            if !parent.removed_child_was_attached
                || !verified_claims
                    .iter()
                    .any(|claim| claim.entity() == parent.removed_child)
                || verified_claims
                    .iter()
                    .any(|claim| claim.entity() == parent.physical)
                || !mount_parent_frozen_manifest_matches(&registry, parent)
                || !mount_parent_physical_matches(world, &registry, parent)
                || world
                    .get::<ChildOf>(parent.removed_child)
                    .map(ChildOf::parent)
                    != Some(parent.physical)
            {
                continue;
            }
            let child_claim = verified_claims
                .iter()
                .find(|claim| claim.entity() == parent.removed_child)?;
            if !cleanup_claim_has_exact_authority(world, child_claim)
                || registry
                    .record(child_claim.entity())
                    .is_none_or(|record| record.claim() != *child_claim)
                || edges
                    .iter()
                    .any(|edge: &PreparedCleanupQuarantineMountParentDetachment| {
                        edge.child_claim.entity() == child_claim.entity()
                    })
            {
                return None;
            }
            let child_manifest_before = ComponentManifest::from_world(world, child_claim.entity())?;
            let child_manifest_after =
                relationship_manifest_after(&child_manifest_before, child_of, false);
            let parent_next_manifest = relationship_manifest_after(
                parent.next_manifest_base(),
                children,
                !parent.children_after.is_empty(),
            );
            let parent_physical_manifest_after = relationship_manifest_after(
                &parent.physical_manifest,
                children,
                !parent.children_after.is_empty(),
            );
            edges.push(PreparedCleanupQuarantineMountParentDetachment {
                parent: parent.clone(),
                parent_next_manifest,
                parent_physical_manifest_after,
                child_claim: child_claim.clone(),
                child_manifest_before,
                child_manifest_after,
            });
        }
    }

    let mut protected_entities = Vec::new();
    let protected_capacity = edges.iter().try_fold(0usize, |count, edge| {
        count
            .checked_add(2)?
            .checked_add(edge.parent.children_before.len())
    })?;
    protected_entities
        .try_reserve_exact(protected_capacity)
        .ok()?;
    for edge in &edges {
        protected_entities.push(edge.parent.physical);
        protected_entities.push(edge.parent.expected_parent);
        protected_entities.extend(
            edge.parent
                .children_before
                .iter()
                .copied()
                .filter(|child| *child != edge.child_claim.entity()),
        );
    }
    protected_entities = observed_descendant_closure(world, protected_entities)
        .into_iter()
        .collect();
    protected_entities.sort_by_key(|entity| entity.to_bits());
    protected_entities.retain(|entity| {
        !verified_claims
            .iter()
            .any(|claim| claim.entity() == *entity)
    });
    let mut protected_before = Vec::new();
    protected_before
        .try_reserve_exact(protected_entities.len())
        .ok()?;
    for entity in protected_entities {
        let observed = FrozenFailureCleanupObservedEntity::observe(world, &registry, entity);
        if !observed.present {
            return None;
        }
        protected_before.push(observed);
    }
    Some(PreparedCleanupQuarantineDetachment {
        targets_after_inert,
        protected_before,
        edges,
    })
}

fn prepare_cleanup_quarantine_publication_guard(
    world: &World,
    ledger: &CleanupLedger,
    verified_claims: &[ExactRegistryClaim],
) -> Option<PreparedCleanupQuarantinePublicationGuard> {
    let registry = world.get_resource::<RouteEntityRegistry>()?;
    let target_entities = verified_claims
        .iter()
        .map(ExactRegistryClaim::entity)
        .collect::<HashSet<_>>();
    if target_entities.len() != verified_claims.len() {
        return None;
    }

    let mut targets = Vec::new();
    targets.try_reserve_exact(verified_claims.len()).ok()?;
    let mut protected_roots = Vec::new();
    for claim in verified_claims {
        if registry
            .record(claim.entity())
            .is_none_or(|record| record.claim() != *claim)
        {
            return None;
        }
        let observed =
            FrozenFailureCleanupObservedEntity::observe(world, &registry, claim.entity());
        if !observed.present || !cleanup_claim_has_exact_authority(world, claim) {
            return None;
        }
        if let Some(parent) = observed.parent {
            protected_roots.push(parent);
        }
        protected_roots.extend(observed.children.iter().copied());
        targets.push(observed);
    }
    if let Some(detachment) = ledger.quarantine_detachment.as_ref() {
        for edge in &detachment.edges {
            protected_roots.push(edge.parent.physical);
            protected_roots.push(edge.parent.expected_parent);
            protected_roots.extend(observed_children(world, edge.parent.physical));
        }
    }
    protected_roots.sort_by_key(|entity| entity.to_bits());
    protected_roots.dedup();
    protected_roots.retain(|entity| !target_entities.contains(entity));
    let mut protected_entities = observed_descendant_closure(world, protected_roots)
        .into_iter()
        .filter(|entity| !target_entities.contains(entity))
        .collect::<Vec<_>>();
    protected_entities.sort_by_key(|entity| entity.to_bits());
    protected_entities.dedup();
    let mut protected = Vec::new();
    protected.try_reserve_exact(protected_entities.len()).ok()?;
    for entity in protected_entities {
        let observed = FrozenFailureCleanupObservedEntity::observe(world, &registry, entity);
        if !observed.present {
            return None;
        }
        protected.push(observed);
    }
    Some(PreparedCleanupQuarantinePublicationGuard { targets, protected })
}

struct PreparedCleanupQuarantineTransfers {
    verified: Option<PreparedQuarantineTransfer>,
    unresolved: Option<PreparedUnresolvedResidualTransfer>,
    lost: Option<PreparedAuthorityLostResidualTransfer>,
}

fn prepare_cleanup_quarantine_transfers(
    world: &mut World,
    verified_claims: Vec<ExactRegistryClaim>,
    verified_manifests: Vec<(Entity, ComponentManifest)>,
    unresolved_claims: Vec<ExactRegistryClaim>,
    unresolved_manifests: Vec<(Entity, ComponentManifest)>,
    verified_content: Vec<crate::registry::ContainedResidualFact>,
    unresolved_content: Vec<crate::registry::ContainedResidualFact>,
    lost_facts: PreparedAuthorityLostResidualProjection,
    facts: QuarantineFacts,
) -> Option<PreparedCleanupQuarantineTransfers> {
    world.resource_scope(
        |world, registry: bevy_ecs::change_detection::Mut<RouteEntityRegistry>| {
            world.resource_scope(
                |_world, mut table: bevy_ecs::change_detection::Mut<QuarantineLeaseTable>| {
                    let verified = if verified_claims.is_empty() {
                        None
                    } else {
                        let transfer = VerifiedCleanupQuarantineTransfer::after_world_validation(
                            verified_claims,
                            verified_manifests,
                            facts.clone(),
                        );
                        let mut prepared = prepare_verified_cleanup_quarantine_transfer(
                            &mut table, &registry, transfer,
                        )
                        .ok()?;
                        prepared
                            .attach_contained(&registry, verified_content)
                            .ok()?;
                        Some(prepared)
                    };
                    let unresolved = if unresolved_claims.is_empty() {
                        None
                    } else {
                        let mut prepared = prepare_unresolved_residual_transfer(
                            &mut table,
                            &registry,
                            unresolved_claims,
                            unresolved_manifests,
                            facts.clone(),
                        )
                        .ok()?;
                        prepared
                            .attach_contained(&registry, unresolved_content)
                            .ok()?;
                        Some(prepared)
                    };
                    let lost = if lost_facts.is_empty() {
                        None
                    } else {
                        Some(
                            prepare_authority_lost_residual_transfer(
                                &mut table, &registry, lost_facts, facts,
                            )
                            .ok()?,
                        )
                    };
                    Some(PreparedCleanupQuarantineTransfers {
                        verified,
                        unresolved,
                        lost,
                    })
                },
            )
        },
    )
}

fn apply_cleanup_quarantine_transfers(
    world: &mut World,
    prepared: PreparedCleanupQuarantineTransfers,
) {
    world.resource_scope(
        |world, mut registry: bevy_ecs::change_detection::Mut<RouteEntityRegistry>| {
            world.resource_scope(
                |_world, mut table: bevy_ecs::change_detection::Mut<QuarantineLeaseTable>| {
                    if let Some(transfer) = prepared.verified {
                        apply_prepared_quarantine_transfer(&mut table, &mut registry, transfer);
                    }
                    if let Some(transfer) = prepared.unresolved {
                        apply_prepared_unresolved_residual_transfer(
                            &mut table,
                            &mut registry,
                            transfer,
                        );
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
    verified_content: Vec<crate::registry::ContainedResidualFact>,
    unresolved_content: Vec<crate::registry::ContainedResidualFact>,
    lost_facts: PreparedAuthorityLostResidualProjection,
    facts: QuarantineFacts,
    focus_closure: Vec<Entity>,
    ledger: &mut CleanupLedger,
    receipts: PreparedCleanupReceiptBatch,
) -> Option<()> {
    if ledger.cleanup_settlement_permanent_pending.is_some() {
        return None;
    }
    let detachment = prepare_cleanup_quarantine_detachment(
        world,
        ledger,
        &verified_claims,
        unresolved_claims.is_empty()
            && lost_facts.is_empty()
            && !ledger.mount_parent_settlement_required(),
    )?;
    let conflict_kind = ledger.quarantine_detachment.as_ref().map_or_else(
        || detachment.intrinsic_conflict_kind(),
        |existing| existing.merge_conflict_with_prepared(&detachment),
    );
    if let Some(kind) = conflict_kind {
        let applied_edges = detachment.edges.len();
        let newer = detachment.into_frozen_prefix(applied_edges);
        let existing = ledger.quarantine_detachment.take().unwrap_or_default();
        ledger.cleanup_settlement_permanent_pending =
            Some(Box::new(CleanupSettlementPermanentCause::MergeConflict(
                CleanupQuarantineDetachmentMergeConflict {
                    existing,
                    newer,
                    kind,
                },
            )));
        return None;
    }
    if let Some(existing) = ledger.quarantine_detachment.as_mut()
        && !existing.reserve_merge_from_prepared(&detachment)
    {
        return None;
    }
    let parent_manifests = detachment.prepared_parent_manifests()?;
    let applied = detachment.apply_and_refreeze(world, parent_manifests);
    if applied.applied_edges != 0 {
        let newer = detachment.into_frozen_prefix(applied.applied_edges);
        match ledger.quarantine_detachment.as_mut() {
            Some(existing) => existing.merge_prechecked(newer),
            None => ledger.quarantine_detachment = Some(newer),
        }
    }
    if !applied.exact || !ledger.mount_parents_physical_match(world) {
        return None;
    }

    let guard = prepare_cleanup_quarantine_publication_guard(world, ledger, &verified_claims)?;
    #[cfg(test)]
    apply_cleanup_prepublication_drift(world, CleanupPrepublicationPhase::VerifiedTransfer);
    if !guard.matches(world) || !ledger.mount_parents_physical_match(world) {
        return None;
    }
    let verified_manifests = guard.verified_manifests(&verified_claims)?;
    let mut unresolved_manifests = Vec::new();
    unresolved_manifests
        .try_reserve_exact(unresolved_claims.len())
        .ok()?;
    for claim in &unresolved_claims {
        let Some(manifest) = ComponentManifest::from_world(world, claim.entity()).or_else(|| {
            world
                .resource::<RouteEntityRegistry>()
                .record(claim.entity())
                .filter(|record| record.claim() == *claim)
                .map(|record| record.manifest().clone())
        }) else {
            return None;
        };
        unresolved_manifests.push((claim.entity(), manifest));
    }
    let transfers = prepare_cleanup_quarantine_transfers(
        world,
        verified_claims.clone(),
        verified_manifests,
        unresolved_claims,
        unresolved_manifests,
        verified_content,
        unresolved_content,
        lost_facts,
        facts,
    )?;
    if !guard.matches(world) || !ledger.mount_parents_physical_match(world) {
        return None;
    }
    prepare_cleanup_quarantine_mutation(world, &verified_claims)?.apply();
    world.resource_scope(
        |world, input_focus: bevy_ecs::change_detection::Mut<InputFocus>| {
            world.resource_scope(
                |world, route_focus: bevy_ecs::change_detection::Mut<RouteFocusState>| {
                    let focus = prepare_focus_mutation(input_focus, route_focus, &focus_closure);
                    focus.apply();
                    apply_cleanup_quarantine_transfers(world, transfers);
                    // All capacity, identity, presentation, focus, and detach work was
                    // prepared and refrozen above. This tail only publishes the exact
                    // residual partition and terminal receipts.
                    ledger.commit_receipt_batch(receipts);
                },
            )
        },
    );
    Some(())
}

struct PreparedCleanupQuarantineEntity<'w> {
    visual: Option<(
        Mut<'w, Visibility>,
        Mut<'w, Pickable>,
        Mut<'w, RouterInputBarrier>,
    )>,
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
        for (_, entity) in self.entities {
            if let Some((mut visibility, mut pickable, mut barrier)) = entity.visual {
                *visibility = Visibility::Hidden;
                *pickable = Pickable::IGNORE;
                *barrier = RouterInputBarrier::deny_all();
            }
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
    #[cfg(feature = "bsn")]
    let native_content = claims
        .iter()
        .filter_map(|claim| {
            crate::presentation::native_frame::is_native_content(world, claim.entity())
                .then_some(claim.entity())
        })
        .collect::<std::collections::HashSet<_>>();
    #[cfg(feature = "bsn")]
    if !native_content
        .iter()
        .all(|entity| crate::presentation::native_frame::content_scope_is_inert(world, *entity))
    {
        return None;
    }
    let mut query = world.query::<(
        Entity,
        Option<&mut Visibility>,
        Option<&mut Pickable>,
        Option<&mut RouterInputBarrier>,
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
        if found[index] || seal != &RouteOwnershipSeal::new(claim.seal()) {
            return None;
        }
        #[cfg(feature = "bsn")]
        let authored_content = native_content.contains(&entity);
        #[cfg(not(feature = "bsn"))]
        let authored_content = false;
        let visual = if authored_content {
            None
        } else {
            let (visibility, pickable, barrier) = (visibility?, pickable?, barrier?);
            if *visibility != Visibility::Hidden
                || *pickable != Pickable::IGNORE
                || !barrier.denies_all()
            {
                return None;
            }
            Some((visibility, pickable, barrier))
        };
        found[index] = true;
        entities.push((index, PreparedCleanupQuarantineEntity { visual, lifecycle }));
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
            #[cfg(feature = "bsn")]
            if crate::presentation::native_frame::is_native_content(world, claim.entity()) {
                return None;
            }
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
    #[cfg(feature = "bsn")]
    if crate::presentation::native_frame::is_native_content(world, claim.entity()) {
        return world
            .resource::<RouteEntityRegistry>()
            .record(claim.entity())
            .is_some_and(|record| record.claim() == *claim)
            && cleanup_claim_has_exact_authority(world, claim)
            && crate::presentation::native_frame::content_scope_is_inert(world, claim.entity());
    }
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
        ) | (ProvisionalSlotLifecycle::Leaving, EntityLifecycle::Leaving)
            | (
                ProvisionalSlotLifecycle::Quarantined,
                EntityLifecycle::Quarantined
            )
    )
}

pub(crate) fn cleanup_claim_has_exact_authority(world: &World, claim: &ExactRegistryClaim) -> bool {
    cleanup_authority_components_are_exact(
        world,
        claim.entity(),
        claim.owner(),
        claim.seal(),
        claim.lifecycle(),
    )
}

fn cleanup_authority_components_are_exact(
    world: &World,
    entity: Entity,
    owner: &RegistryOwner,
    seal: crate::registry::OwnershipSeal,
    lifecycle: EntityLifecycle,
) -> bool {
    let seal_is_exact =
        world.get::<RouteOwnershipSeal>(entity) == Some(&RouteOwnershipSeal::new(seal));
    let lifecycle_is_exact = match owner {
        RegistryOwner::Committed(
            EntityOwner::FrameShell(_)
            | EntityOwner::FrameContentTarget(_)
            | EntityOwner::FrameOutput { .. },
        ) => world.get::<FrameLifecycle>(entity) == Some(&FrameLifecycle(lifecycle)),
        RegistryOwner::Committed(EntityOwner::ChildSlot { .. })
        | RegistryOwner::Provisional(ProvisionalOwner::ChildSlot { .. }) => {
            child_slot_components_match(
                world.get::<ChildSlotLifecycle>(entity),
                world.get::<ChildSlotAuthority>(entity),
                owner,
                lifecycle,
            )
        }
        RegistryOwner::Committed(EntityOwner::HostTopSlot { .. }) => world
            .get::<ProvisionalSlotLifecycle>(entity)
            .is_some_and(|observed| lifecycle_is_exact(*observed, lifecycle)),
        _ => false,
    };
    seal_is_exact && lifecycle_is_exact
}

fn terminal_quarantine_root_is_inert(world: &World, claim: &ExactRegistryClaim) -> bool {
    #[cfg(feature = "bsn")]
    if crate::presentation::native_frame::is_native_content(world, claim.entity()) {
        return cleanup_claim_has_exact_authority(world, claim)
            && crate::presentation::native_frame::content_scope_is_inert(world, claim.entity());
    }
    cleanup_claim_has_exact_authority(world, claim)
        && world.get::<Visibility>(claim.entity()) == Some(&Visibility::Hidden)
        && world.get::<Pickable>(claim.entity()) == Some(&Pickable::IGNORE)
        && world
            .get::<RouterInputBarrier>(claim.entity())
            .is_some_and(RouterInputBarrier::denies_all)
}

fn contained_terminal_scope_is_current(world: &World, identity: &CleanupTargetIdentity) -> bool {
    let CleanupTargetIdentity::ContainedContent {
        entity,
        root,
        shell,
    } = identity
    else {
        return false;
    };
    #[cfg(feature = "bsn")]
    {
        crate::presentation::native_frame::current_containment(
            world,
            *entity,
            root.entity(),
            shell.entity(),
        ) && crate::presentation::native_frame::content_scope_is_inert_for(world, root, shell)
    }
    #[cfg(not(feature = "bsn"))]
    {
        let _ = (world, entity, root, shell);
        false
    }
}

fn lost_terminal_scope_is_current(world: &World, identity: &CleanupTargetIdentity) -> bool {
    match identity {
        CleanupTargetIdentity::Registered(claim) => {
            terminal_authority_lost_entity_is_inert(world, claim.entity())
        }
        CleanupTargetIdentity::ContainedContent { entity, root, .. } => {
            !contained_target_is_inside(world, *entity, root.entity())
                || contained_terminal_scope_is_current(world, identity)
        }
    }
}

fn terminal_authority_lost_entity_is_inert(world: &World, entity: Entity) -> bool {
    #[cfg(feature = "bsn")]
    if crate::presentation::native_frame::is_native_content(world, entity) {
        return crate::presentation::native_frame::content_scope_is_inert(world, entity);
    }
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
    #[cfg(feature = "bsn")]
    if crate::presentation::native_frame::is_native_content(world, claim.entity()) {
        return world.get_entity(claim.entity()).is_ok()
            && seal_is_compatible
            && authority_is_compatible
            && crate::presentation::native_frame::content_scope_is_inert(world, claim.entity());
    }
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
    if let CleanupTargetIdentity::ContainedContent {
        entity,
        root,
        shell,
    } = &obligation.identity
    {
        return contained_source_is_current(world, root, shell)
            && current_contained_target(world, *entity, root.entity(), shell.entity())
            && terminal_unresolved_root_is_inert(world, shell, false);
    }
    let CleanupTargetIdentity::Registered(claim) = &obligation.identity else {
        return false;
    };
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
    #[cfg(feature = "bsn")]
    if crate::presentation::native_frame::is_native_content(world, claim.entity()) {
        return record_is_exact
            && seal_is_compatible
            && authority_is_compatible
            && ComponentManifest::from_world(world, claim.entity()).is_some()
            && crate::presentation::native_frame::content_scope_is_inert(world, claim.entity());
    }
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
    child_slot_components_match(lifecycle, authority, claim.owner(), claim.lifecycle())
}

fn child_slot_components_match(
    lifecycle: Option<&ChildSlotLifecycle>,
    authority: Option<&ChildSlotAuthority>,
    owner: &RegistryOwner,
    registered_lifecycle: EntityLifecycle,
) -> bool {
    let expected_lifecycle = match (owner, registered_lifecycle) {
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
        (RegistryOwner::Committed(EntityOwner::ChildSlot { .. }), EntityLifecycle::Retained) => {
            ChildSlotLifecycle::Retained
        }
        (
            RegistryOwner::Provisional(ProvisionalOwner::ChildSlot { .. })
            | RegistryOwner::Committed(EntityOwner::ChildSlot { .. }),
            EntityLifecycle::Quarantined,
        ) => ChildSlotLifecycle::Quarantined,
        _ => return false,
    };
    let (parent, outlet, slot) = match owner {
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
        RegistryOwner::Committed(EntityOwner::ChildSlot {
            parent,
            outlet,
            slot,
        }) if claim.lifecycle() == EntityLifecycle::Retained => {
            (ChildSlotLifecycle::Retained, parent, outlet, *slot)
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
    // Classification only reads this batch. Share its unchanged membership here,
    // and rebuild it on the next collection after any World/receipt mutation.
    // Lazy construction preserves the allocation-free early rejection paths.
    let unsettled = std::cell::OnceCell::new();
    obligations
        .iter()
        .enumerate()
        .filter(|(index, _)| receipts[*index].is_none())
        .map(|(index, _)| {
            (
                index,
                classify_obligation_with_unsettled(world, obligations, receipts, index, &unsettled),
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

fn focused_targets(world: &World, targets: &[Entity]) -> Vec<Entity> {
    let mut focused = Vec::new();
    if let Some(entity) = world.get_resource::<InputFocus>().and_then(InputFocus::get)
        && targets.contains(&entity)
    {
        focused.push(entity);
    }
    if let Some(entity) = world
        .get_resource::<RouteFocusState>()
        .and_then(RouteFocusState::focused)
        && targets.contains(&entity)
        && !focused.contains(&entity)
    {
        focused.push(entity);
    }
    focused
}

fn target_preparation_permanent_removal_evidence(
    world: &World,
    stage: TargetPreparationPermanentSettlementFailureStage,
    index: usize,
    obligation: &CleanupObligation,
    expected_parent: Option<Entity>,
    expected_children: &[Entity],
) -> TargetPreparationPermanentSettlementBoundaryEvidence {
    let observed_claim = world
        .get_resource::<RouteEntityRegistry>()
        .and_then(|registry| registry.record(obligation.entity()))
        .map(|record| record.claim());
    CleanupLedger::target_preparation_permanent_boundary(
        stage,
        TargetPreparationPermanentSettlementExpectation::RemovableObligation {
            index,
            identity: obligation.identity.clone(),
            expected_parent,
            expected_children: expected_children.to_vec(),
        },
        TargetPreparationPermanentSettlementObservation::Removal {
            entity_exists: world.get_entity(obligation.entity()).is_ok(),
            observed_claim,
            parent: world
                .get::<ChildOf>(obligation.entity())
                .map(ChildOf::parent),
            children: observed_children(world, obligation.entity()),
            authority_exact: cleanup_obligation_has_exact_authority(world, obligation),
        },
    )
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
    classify_obligation_with_unsettled(
        world,
        obligations,
        receipts,
        index,
        &std::cell::OnceCell::new(),
    )
}

fn cleanup_obligation_has_exact_authority(world: &World, obligation: &CleanupObligation) -> bool {
    match &obligation.identity {
        CleanupTargetIdentity::Registered(claim) => cleanup_claim_has_exact_authority(world, claim),
        CleanupTargetIdentity::ContainedContent {
            entity,
            root,
            shell,
        } => {
            contained_source_is_current(world, root, shell)
                && current_contained_target(world, *entity, root.entity(), shell.entity())
                && world.get::<ChildOf>(*entity).map(ChildOf::parent)
                    == Some(obligation.expected_parent)
        }
    }
}

fn contained_source_is_current(
    world: &World,
    root: &ExactRegistryClaim,
    shell: &ExactRegistryClaim,
) -> bool {
    let Some(registry) = world.get_resource::<RouteEntityRegistry>() else {
        return false;
    };
    [root, shell].iter().all(|claim| {
        registry
            .record(claim.entity())
            .is_some_and(|record| record.claim() == **claim)
            && cleanup_claim_has_exact_authority(world, claim)
    }) && world.get::<ChildOf>(root.entity()).map(ChildOf::parent) == Some(shell.entity())
}

fn current_contained_target(world: &World, entity: Entity, root: Entity, shell: Entity) -> bool {
    #[cfg(feature = "bsn")]
    {
        crate::presentation::native_frame::current_containment(world, entity, root, shell)
    }
    #[cfg(not(feature = "bsn"))]
    {
        let _ = (world, entity, root, shell);
        false
    }
}

fn contained_target_is_inside(world: &World, entity: Entity, root: Entity) -> bool {
    let mut current = entity;
    let mut seen = HashSet::new();
    while seen.insert(current) {
        if current == root {
            return true;
        }
        let Some(parent) = world.get::<ChildOf>(current).map(ChildOf::parent) else {
            return false;
        };
        if world
            .get::<Children>(parent)
            .is_none_or(|children| !children.contains(&current))
        {
            return false;
        }
        current = parent;
    }
    false
}

fn classify_contained_content(
    world: &World,
    obligations: &[CleanupObligation],
    receipts: &[Option<CleanupReceipt>],
    obligation: &CleanupObligation,
    entity: Entity,
    root: &ExactRegistryClaim,
    shell: &ExactRegistryClaim,
    unsettled: &std::cell::OnceCell<HashSet<Entity>>,
) -> CleanupEvidence {
    let present = world.get_entity(entity).is_ok();
    let parent = world.get::<ChildOf>(entity).map(ChildOf::parent);
    let registered = world
        .resource::<RouteEntityRegistry>()
        .record(entity)
        .is_some();
    if obligation.terminal_mode == CleanupTerminalMode::AuthorityLost {
        return CleanupEvidence::AuthorityLostResidual {
            actual_parent: parent,
            registry_exact: false,
            entity_present: present,
        };
    }
    if !present && !registered {
        return CleanupEvidence::AlreadyAbsent;
    }
    if !present
        || registered
        || !contained_source_is_current(world, root, shell)
        || !current_contained_target(world, entity, root.entity(), shell.entity())
        || parent != Some(obligation.expected_parent)
    {
        return CleanupEvidence::AuthorityLostResidual {
            actual_parent: parent,
            registry_exact: false,
            entity_present: present,
        };
    }
    let targets = unsettled.get_or_init(|| {
        obligations
            .iter()
            .enumerate()
            .filter(|(i, _)| receipts[*i].is_none())
            .map(|(_, obligation)| obligation.entity())
            .collect::<HashSet<_>>()
    });
    let mut foreign_descendants = Vec::new();
    let mut seen = HashSet::new();
    let mut cycle = false;
    let mut stack = observed_children(world, entity);
    while let Some(child) = stack.pop() {
        if !seen.insert(child) {
            cycle = true;
            continue;
        }
        if !targets.contains(&child) {
            foreign_descendants.push(child);
        }
        stack.extend(observed_children(world, child));
    }
    foreign_descendants.sort_by_key(|entity| entity.to_bits());
    CleanupEvidence::Exact {
        actual_parent: parent,
        normalized_manifest: ComponentManifest::new([]),
        foreign_descendants,
        foreign_components: Vec::new(),
        missing_components: Vec::new(),
        protected_parent: None,
        relationship_cycle: cycle,
    }
}

fn registered_cleanup_physical_claim_is_compatible(
    world: &World,
    obligation: &CleanupObligation,
    claim: &ExactRegistryClaim,
) -> bool {
    let actual_seal = world.get::<RouteOwnershipSeal>(obligation.entity());
    let incomplete_obligation_can_lack_seal =
        obligation.allow_missing_components && actual_seal.is_none();
    let physical_claim_is_compatible = match claim.owner() {
        RegistryOwner::Committed(
            EntityOwner::FrameShell(_)
            | EntityOwner::FrameContentTarget(_)
            | EntityOwner::FrameOutput { .. },
        ) => {
            let lifecycle = world.get::<FrameLifecycle>(obligation.entity());
            lifecycle == Some(&FrameLifecycle(claim.lifecycle()))
                || (obligation.allow_missing_components && lifecycle.is_none())
        }
        RegistryOwner::Committed(EntityOwner::HostTopSlot { .. }) => world
            .get::<ProvisionalSlotLifecycle>(obligation.entity())
            .map(|lifecycle| lifecycle_is_exact(*lifecycle, claim.lifecycle()))
            .unwrap_or(obligation.allow_missing_components),
        _ => child_slot_components_are_compatible(
            world.get::<ChildSlotLifecycle>(obligation.entity()),
            world.get::<ChildSlotAuthority>(obligation.entity()),
            claim,
            obligation.allow_missing_components,
        ),
    };
    (actual_seal == Some(&RouteOwnershipSeal::new(claim.seal()))
        || incomplete_obligation_can_lack_seal)
        && physical_claim_is_compatible
}

fn classify_obligation_with_unsettled(
    world: &World,
    obligations: &[CleanupObligation],
    receipts: &[Option<CleanupReceipt>],
    index: usize,
    unsettled: &std::cell::OnceCell<HashSet<Entity>>,
) -> CleanupEvidence {
    let obligation = &obligations[index];
    let claim = match &obligation.identity {
        CleanupTargetIdentity::Registered(claim) => claim,
        CleanupTargetIdentity::ContainedContent {
            entity,
            root,
            shell,
        } => {
            return classify_contained_content(
                world,
                obligations,
                receipts,
                obligation,
                *entity,
                root,
                shell,
                unsettled,
            );
        }
    };
    let authority_settlement = matches!(
        obligation.terminal_mode,
        CleanupTerminalMode::AuthorityExact | CleanupTerminalMode::AuthorityLost
    );
    let entity_present = world.get_entity(obligation.entity()).is_ok();
    let registry_exact = world
        .get_resource::<RouteEntityRegistry>()
        .and_then(|registry| registry.record(obligation.entity()))
        .is_some_and(|record| record.claim() == *claim);
    let lost_evidence = || CleanupEvidence::AuthorityLostResidual {
        actual_parent: world
            .get::<ChildOf>(obligation.entity())
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
                .and_then(|registry| registry.record(obligation.entity()));
            return match current {
                None => CleanupEvidence::AlreadyAbsent,
                Some(record) if record.claim() == *claim => CleanupEvidence::AlreadyAbsent,
                Some(_) => lost_evidence(),
            };
        }
        return match world
            .get_resource::<RouteEntityRegistry>()
            .and_then(|registry| registry.record(obligation.entity()))
        {
            None => CleanupEvidence::AlreadyAbsent,
            Some(record) if record.claim() == *claim => CleanupEvidence::AlreadyAbsent,
            Some(_) => CleanupEvidence::StaleClaim,
        };
    }
    let Some(record) = world
        .get_resource::<RouteEntityRegistry>()
        .and_then(|registry| registry.record(obligation.entity()))
    else {
        return if authority_settlement {
            lost_evidence()
        } else {
            CleanupEvidence::StaleClaim
        };
    };
    if record.claim() != *claim {
        return if authority_settlement {
            lost_evidence()
        } else {
            CleanupEvidence::StaleClaim
        };
    }
    if !registered_cleanup_physical_claim_is_compatible(world, obligation, claim) {
        return if authority_settlement {
            lost_evidence()
        } else {
            CleanupEvidence::StaleClaim
        };
    }
    let Some(actual_manifest) = ComponentManifest::from_world(world, obligation.entity()) else {
        return CleanupEvidence::AlreadyAbsent;
    };
    let relationship_components = [
        world.components().component_id::<ChildOf>(),
        world.components().component_id::<Children>(),
    ];
    let normalized_manifest = ComponentManifest::new(
        actual_manifest
            .component_ids()
            .iter()
            .copied()
            .filter(|component| !relationship_components.contains(&Some(*component))),
    );
    let mut foreign_components = actual_manifest
        .component_ids()
        .iter()
        .copied()
        .filter(|component| {
            !obligation.manifest.component_ids().contains(component)
                && !relationship_components.contains(&Some(*component))
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
                    && !relationship_components.contains(&Some(*component))
            })
            .collect::<Vec<_>>()
    };
    missing_components.sort_unstable();
    missing_components.dedup();

    let unsettled_owned = unsettled.get_or_init(|| {
        obligations
            .iter()
            .enumerate()
            .filter_map(|(candidate, obligation)| {
                receipts[candidate].is_none().then_some(obligation.entity())
            })
            .collect::<HashSet<_>>()
    });
    let mut foreign_descendants = Vec::new();
    let mut visited = HashSet::new();
    let mut relationship_cycle = false;
    let mut stack = world
        .get::<Children>(obligation.entity())
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
        .get::<ChildOf>(obligation.entity())
        .map(ChildOf::parent);
    let protected_parent = if authority_settlement {
        actual_parent.filter(|parent| {
            let retired_host_boundary = matches!(
                claim.owner(),
                RegistryOwner::Committed(EntityOwner::HostTopSlot { .. })
            ) && *parent == obligation.authorized_parent;
            let exact_batch_parent =
                obligations
                    .iter()
                    .enumerate()
                    .any(|(candidate, parent_obligation)| {
                        receipts[candidate].is_none()
                            && parent_obligation.entity() == *parent
                            && parent_obligation.terminal_mode
                                == CleanupTerminalMode::AuthorityExact
                            && cleanup_obligation_has_exact_authority(world, parent_obligation)
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
    quarantine_disposition_excluding_authorized_absence(evidence, obligations, &|_| false)
}

fn quarantine_disposition_excluding_authorized_absence(
    evidence: &[(usize, CleanupEvidence)],
    obligations: &[CleanupObligation],
    authorized_absence: &impl Fn(Entity) -> bool,
) -> Option<CleanupDisposition> {
    if evidence.iter().any(|(_, evidence)| {
        matches!(
            evidence,
            CleanupEvidence::AuthorityLostResidual {
                entity_present: true,
                ..
            }
        )
    }) {
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
            } if foreign_descendants
                .iter()
                .any(|entity| !authorized_absence(*entity))
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
) -> Result<Vec<AuthoritySettlementDisposition>, AuthorityCleanupFailureKind> {
    let mut plans = Vec::new();
    plans
        .try_reserve_exact(authority_preparation_capacity_input(
            AuthorityPreparationCapacitySite::Dispositions,
            evidence.len(),
        ))
        .map_err(|_| AuthorityCleanupFailureKind::RetryableCapacity)?;
    let mut retained_entities = HashSet::new();
    retained_entities
        .try_reserve(authority_preparation_capacity_input(
            AuthorityPreparationCapacitySite::RetainedEntities,
            evidence.len(),
        ))
        .map_err(|_| AuthorityCleanupFailureKind::RetryableCapacity)?;
    for (index, evidence) in evidence {
        let obligation = obligations
            .get(*index)
            .ok_or(AuthorityCleanupFailureKind::Preparation)?;
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
            CleanupEvidence::StaleClaim => return Err(AuthorityCleanupFailureKind::Preparation),
        };
        if action != AuthoritySettlementAction::Remove {
            retained_entities.insert(obligation.entity());
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
                    obligations[plan.index].entity(),
                    &retained_entities,
                )
            })
            .map(|plan| plan.index)
            .collect::<Vec<_>>();
        if promoted.is_empty() {
            break;
        }
        for index in promoted {
            let plan = plans
                .iter_mut()
                .find(|plan| plan.index == index)
                .ok_or(AuthorityCleanupFailureKind::Preparation)?;
            plan.action = AuthoritySettlementAction::VerifiedResidual;
            plan.disposition = CleanupDisposition::QuarantinedForeignDescendant;
            retained_entities.insert(obligations[index].entity());
        }
    }
    Ok(plans)
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
) -> Result<Vec<Vec<usize>>, AuthorityCleanupFailureKind> {
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
    groups
        .try_reserve_exact(authority_preparation_capacity_input(
            AuthorityPreparationCapacitySite::ResidualGroups,
            remaining.len(),
        ))
        .map_err(|_| AuthorityCleanupFailureKind::RetryableCapacity)?;
    while let Some(seed) = remaining.pop() {
        let mut group = vec![seed];
        let mut cursor = 0;
        while cursor < group.len() {
            let entity = obligations
                .get(group[cursor])
                .ok_or(AuthorityCleanupFailureKind::Preparation)?
                .entity();
            let mut candidate = 0;
            while candidate < remaining.len() {
                let other = obligations
                    .get(remaining[candidate])
                    .ok_or(AuthorityCleanupFailureKind::Preparation)?
                    .entity();
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
    Ok(groups)
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
    let ui_detachment = host_edge.ui_detachment_for(entity);
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
            .filter(|component| !ui_detachment.removes(*component))
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
    world: &World,
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
                entities.push(obligations[*index].entity());
                entities.extend(foreign_descendants);
            }
            CleanupEvidence::AuthorityLostResidual { .. } => {
                entities.push(obligations[*index].entity());
            }
            _ => {}
        }
    }
    entities.sort_by_key(|entity| entity.to_bits());
    entities.dedup();
    entities.retain(|entity| !obligations.iter().any(|obligation|
        matches!(&obligation.identity, CleanupTargetIdentity::ContainedContent { entity: target, root, .. }
            if target == entity && !contained_target_is_inside(world, *entity, root.entity()))));
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
        self.push(obligation.entity().to_bits());
        match &obligation.identity {
            CleanupTargetIdentity::Registered(claim) => {
                self.push(0);
                self.bytes(format!("{:?}", claim.owner()).as_bytes());
                self.bytes(format!("{:?}", claim.seal()).as_bytes());
                self.bytes(format!("{:?}", claim.lifecycle()).as_bytes());
            }
            CleanupTargetIdentity::ContainedContent { root, shell, .. } => {
                self.push(1);
                self.bytes(format!("{root:?}/{shell:?}").as_bytes());
            }
        }
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
        self.push(parent.physical.to_bits());
        self.push(parent.claim.entity().to_bits());
        match &parent.binding {
            CleanupParentBinding::Structural { upstream } => {
                self.push(0);
                self.push(
                    upstream
                        .as_ref()
                        .map_or(0, crate::presentation::OutletBinding::opaque_digest_part),
                );
            }
            CleanupParentBinding::Outlet(binding) => {
                self.push(1);
                self.push(binding.opaque_digest_part());
            }
        }
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
#[path = "cleanup_tests.rs"]
pub(crate) mod tests;

#[cfg(test)]
mod retained_assembly_tests {
    use super::*;

    #[test]
    fn leaving_assembly_empty_forest_keeps_zero_obligation_post_commit_transfer_ownerless() {
        use bevy_ui_router_core::{
            integration::{CleanupManifest, EngineAction, RouterEngine},
            prelude::*,
        };
        let view = RouteViewKey::new("empty-retained-forest").unwrap();
        let tree = RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/").unwrap(),
                RouteNodeOptions::new().with_view(view),
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
            panic!("real root task required");
        };
        let ticket = engine.report_applied(task).unwrap();
        let prepared = engine
            .prepare_commit(&ticket, CleanupManifest::empty())
            .unwrap();
        let batch = prepared.cleanup().clone();
        engine
            .validate_publication(prepared)
            .unwrap()
            .publish(|| {});
        let manifest = FrozenCleanupManifest::empty();
        let assembly = PreparedLeavingAssembly::reserve(&manifest, &[], &[]).unwrap();
        let payload = assembly.assemble(RealizedBranch::empty(), Vec::new());
        assert!(payload.is_empty());
        let prepared = CleanupLedger::prepare(batch, manifest).unwrap();
        let mut ledger = CleanupLedger::activate_leaving_forest(prepared, payload);
        let mut world = World::new();
        world.insert_resource(RouteEntityRegistry::new());
        assert!(ledger.freeze_for_post_commit_transfer(&world).is_ok());
        engine
            .complete_cleanup(ledger.pending_completion().unwrap())
            .unwrap();
    }

    fn active_branch() -> (bevy_app::App, RealizedBranch) {
        let (mut app, failed, _, _, _) = crate::navigation::tests::failed_nested_branch_fixture();
        let mut branch = failed.into_realized_for_test();
        let targets = branch
            .entering_mutation_targets(EntityLifecycle::PreparedInert, EntityLifecycle::Active);
        let transitions = branch.lifecycle_transitions(EntityLifecycle::Active, false);
        let mutation = app
            .world()
            .resource::<RouteEntityRegistry>()
            .prepare_lifecycle_mutation(&transitions)
            .unwrap();
        let projected = mutation.resulting_claims().to_vec();
        let rebind = branch
            .prepare_claim_rebind(
                &projected,
                EntityLifecycle::PreparedInert,
                EntityLifecycle::Active,
            )
            .unwrap();
        crate::presentation::prepare_presentation_mutation(app.world_mut(), &targets)
            .unwrap()
            .apply();
        mutation.apply(&mut app.world_mut().resource_mut::<RouteEntityRegistry>());
        branch.apply_claim_rebind(rebind);
        (app, branch)
    }

    fn manifest(app: &bevy_app::App, branch: &RealizedBranch) -> FrozenCleanupManifest {
        let projected = branch
            .claims()
            .map(|claim| claim.with_lifecycle_for_test(EntityLifecycle::Leaving))
            .collect::<Vec<_>>();
        FrozenCleanupManifest::from_leaving_branch(
            app.world(),
            app.world().resource::<RouteEntityRegistry>(),
            branch,
            &projected,
            None,
        )
        .unwrap()
    }

    #[test]
    fn leaving_assembly_preserves_original_owners_with_reserved_storage() {
        let (app, mut branch) = active_branch();
        let manifest = manifest(&app, &branch);
        let frames = branch.frames().iter().collect::<Vec<_>>();
        let slots = branch.child_slots().iter().collect::<Vec<_>>();
        let assembly = PreparedLeavingAssembly::reserve(&manifest, &frames, &slots).unwrap();
        let expected_frames = branch
            .frames()
            .iter()
            .map(|frame| frame.frame().clone())
            .collect::<Vec<_>>();
        let expected_claims = branch
            .claims()
            .map(|claim| claim.with_lifecycle_for_test(EntityLifecycle::Leaving))
            .collect::<Vec<_>>();
        let capacities = (assembly.frames.capacity(), assembly.child_slots.capacity());
        let rebind = branch
            .prepare_claim_rebind(
                &expected_claims,
                EntityLifecycle::Active,
                EntityLifecycle::Leaving,
            )
            .unwrap();
        branch.apply_claim_rebind(rebind);
        let payload = assembly.assemble(branch, Vec::new());
        assert_eq!(
            (payload.frames.capacity(), payload.child_slots.capacity()),
            capacities
        );
        assert_eq!(
            payload
                .frames
                .iter()
                .map(|frame| frame.frame().clone())
                .collect::<Vec<_>>(),
            expected_frames
        );
        let actual = payload
            .frames
            .iter()
            .flat_map(RealizedFrame::claims)
            .chain(payload.child_slots.iter().map(MountedChildSlot::slot_claim))
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(actual, expected_claims);
    }

    #[test]
    fn leaving_assembly_rejects_missing_owner_and_wrong_manifest_seal() {
        let (app, branch) = active_branch();
        let mut manifest = manifest(&app, &branch);
        let frames = branch.frames().iter().collect::<Vec<_>>();
        let slots = branch.child_slots().iter().collect::<Vec<_>>();
        assert!(PreparedLeavingAssembly::reserve(&manifest, &frames[..1], &slots).is_none());
        let original = manifest.obligations[0].identity.registered().unwrap();
        let other_seal = branch
            .claims()
            .find(|claim| claim.seal() != original.seal())
            .unwrap()
            .seal();
        manifest.obligations[0].identity =
            CleanupTargetIdentity::Registered(original.with_seal_for_test(other_seal));
        assert!(PreparedLeavingAssembly::reserve(&manifest, &frames, &slots).is_none());
    }
}
