use std::{
    collections::HashSet,
    sync::atomic::{AtomicU64, Ordering},
};

use bevy_ecs::{
    change_detection::Mut,
    component::ComponentId,
    hierarchy::{ChildOf, Children},
    prelude::{Entity, World},
};
use bevy_ui_router_core::integration::{CleanupBatch, CleanupCompletion};

use crate::{
    branch_transaction::{FailedPreparedBranch, RealizedBranch, RejectedChildSlot},
    components::{
        ChildSlotAuthority, ChildSlotLifecycle, ProvisionalSlotLifecycle, RouteOwnershipSeal,
        RouterInputBarrier,
    },
    input::{RouteFocusState, prepare_focus_mutation},
    mount::{ChildSlotResidual, MountedChildSlot, residual_child_slot_is_isolated},
    presentation::{FrameContentTarget, FrameLifecycle, FrameOutput, RealizedFrame, settle_frame},
    registry::{
        ComponentManifest, EntityLifecycle, EntityOwner, ExactRegistryClaim, ProvisionalOwner,
        QuarantineFacts, QuarantineLeaseTable, RegistryOwner, RouteEntityRegistry,
        apply_prepared_quarantine_transfer, apply_prepared_unresolved_residual_transfer,
        prepare_unresolved_residual_transfer, prepare_verified_cleanup_quarantine_transfer,
    },
};
use bevy_camera::visibility::Visibility;
use bevy_input_focus::InputFocus;
use bevy_picking::Pickable;
use bevy_ui::{Node, Val};

const MANIFEST_DOMAIN: u64 = 0x4543_534d_414e_4946;
const ATTEMPT_DOMAIN: u64 = 0x4543_5341_5454_454d;
const RECEIPT_DOMAIN: u64 = 0x4543_5352_4543_5054;
const COMPLETION_DOMAIN: u64 = 0x4543_5343_4f4d_504c;
static NEXT_LEDGER_DOMAIN: AtomicU64 = AtomicU64::new(1);

#[cfg(test)]
thread_local! {
    static FAIL_NEXT_RESERVATION: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
pub(crate) fn fail_next_reservation_for_test() {
    FAIL_NEXT_RESERVATION.set(true);
}

#[cfg(test)]
fn reservation_fault_injected() -> bool {
    FAIL_NEXT_RESERVATION.replace(false)
}

#[cfg(not(test))]
const fn reservation_fault_injected() -> bool {
    false
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CleanupPostcheckPhase {
    VerifiedTransfer,
    Fallback,
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

    pub(crate) const fn focus(phase: CleanupPostcheckPhase, focus: Entity) -> Self {
        Self {
            phase,
            external_parent: None,
            focus: Some(focus),
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
            Self::QuarantinedProtectedParent => 4,
            Self::QuarantinedForeignDescendant => 5,
            Self::QuarantinedForeignComponent => 6,
        }
    }

    pub(crate) const fn public(self) -> bevy_ui_router_core::prelude::CleanupDisposition {
        use bevy_ui_router_core::prelude::CleanupDisposition as Public;
        match self {
            Self::Removed => Public::Removed,
            Self::RemovedDetached => Public::RemovedDetached,
            Self::AlreadyAbsent => Public::AlreadyAbsent,
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
    force_unresolved: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CleanupMountParentSettlement {
    claim: ExactRegistryClaim,
    manifest: ComponentManifest,
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
    supplied_parent: Option<(Entity, Option<ExactRegistryClaim>)>,
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
    digest: u64,
}

impl FrozenCleanupManifest {
    pub(crate) fn empty() -> Self {
        Self {
            obligations: Vec::new(),
            mount_parents: Vec::new(),
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

    pub(crate) fn from_failed_prepared_branch(
        world: &World,
        registry: &RouteEntityRegistry,
        branch: &FailedPreparedBranch,
    ) -> Option<Self> {
        let frames = branch.frames();
        let child_slots = branch.child_slots();
        if frames.is_empty()
            && child_slots.is_empty()
            && branch.mount_residuals().is_empty()
            && branch.detached_frames().is_empty()
            && branch.rejected_child_slots().is_empty()
        {
            return Some(Self::empty());
        }
        if (frames.is_empty() && !child_slots.is_empty())
            || (!frames.is_empty()
                && (child_slots.len() > frames.len() || child_slots.len() + 1 < frames.len()))
        {
            return None;
        }
        let mut obligations = branch
            .detached_frames()
            .iter()
            .rev()
            .map(|frame| current_frame_obligations(registry, frame))
            .collect::<Option<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        obligations.extend(
            branch
                .rejected_child_slots()
                .iter()
                .map(|slot| rejected_slot_obligation(registry, slot))
                .collect::<Option<Vec<_>>>()?,
        );
        if !child_slots.is_empty() && child_slots.len() == frames.len() {
            obligations.push(mounted_slot_obligation(
                world,
                registry,
                &child_slots[child_slots.len() - 1],
            )?);
        }
        for frame_index in (0..frames.len()).rev() {
            obligations.extend(current_frame_obligations(registry, &frames[frame_index])?);
            if frame_index > 0 {
                obligations.push(mounted_slot_obligation(
                    world,
                    registry,
                    &child_slots[frame_index - 1],
                )?);
            }
        }
        let residuals = branch
            .mount_residuals()
            .iter()
            .map(|residual| residual_slot_obligation(world, registry, residual))
            .collect::<Option<Vec<_>>>()?;
        obligations.splice(0..0, residuals);
        let unique = obligations
            .iter()
            .map(|obligation| obligation.claim.entity())
            .collect::<HashSet<_>>();
        if unique.len() != obligations.len() {
            return None;
        }
        let mut parent_candidates = branch
            .detached_frames()
            .iter()
            .chain(frames)
            .map(|frame| frame_mount_parent_candidate(frame, None))
            .collect::<Option<Vec<_>>>()?;
        parent_candidates.extend(
            child_slots
                .iter()
                .map(mounted_anchor_parent_candidate)
                .collect::<Option<Vec<_>>>()?,
        );
        parent_candidates.extend(
            branch
                .mount_residuals()
                .iter()
                .map(|slot| residual_anchor_parent_candidate(world, slot))
                .collect::<Option<Vec<_>>>()?,
        );
        parent_candidates.extend(
            branch
                .rejected_child_slots()
                .iter()
                .map(|slot| {
                    let mut candidates = vec![mounted_anchor_parent_candidate(slot.slot())?];
                    if let Some(canonical) = rejected_canonical_parent_candidate(slot) {
                        candidates.push(canonical);
                    }
                    Some(candidates)
                })
                .collect::<Option<Vec<_>>>()?
                .into_iter()
                .flatten(),
        );
        let mount_parents =
            freeze_external_mount_parents(world, registry, &obligations, parent_candidates)?;
        let digest = frozen_manifest_digest(&obligations, &mount_parents);
        Some(Self {
            obligations,
            mount_parents,
            digest,
        })
    }

    pub(crate) const fn digest(&self) -> u64 {
        self.digest
    }

    pub(crate) fn len(&self) -> usize {
        self.obligations.len()
    }

    pub(crate) fn preflight(&self, world: &World) -> bool {
        let receipts = (0..self.obligations.len())
            .map(|_| None)
            .collect::<Vec<_>>();
        let evidence = collect_evidence(world, &self.obligations, &receipts);
        !evidence.iter().any(|(_, evidence)| {
            matches!(
                evidence,
                CleanupEvidence::RelationshipCycleOrRevisit | CleanupEvidence::StaleClaim
            )
        }) && world
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
}

fn current_frame_obligations(
    registry: &RouteEntityRegistry,
    frame: &RealizedFrame,
) -> Option<Vec<CleanupObligation>> {
    let claims = frame.claims().cloned().collect::<Vec<_>>();
    frame_obligations(registry, frame, &claims)
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
                force_unresolved: false,
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
            || successor.mount_binding() != frame.mount_binding()
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
        supplied_parent: Some((expected_parent, expected_parent_claim.cloned())),
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
        supplied_parent: None,
        removed_child: slot.slot(),
        removed_child_was_attached: true,
        children_before,
        children_after,
    })
}

fn residual_anchor_parent_candidate(
    world: &World,
    slot: &ChildSlotResidual,
) -> Option<CleanupMountParentCandidate> {
    let mut children_before = slot.prior_anchor_children().to_vec();
    let child_of = world.components().component_id::<ChildOf>();
    let removed_child_was_attached = child_of.is_some_and(|component| {
        slot.authorized_manifest()
            .component_ids()
            .contains(&component)
    });
    if removed_child_was_attached {
        children_before.push(slot.entity());
    }
    Some(CleanupMountParentCandidate {
        claim: slot.anchor_claim().clone(),
        supplied_parent: None,
        removed_child: slot.entity(),
        removed_child_was_attached,
        children_before,
        children_after: slot.prior_anchor_children().to_vec(),
    })
}

fn rejected_canonical_parent_candidate(
    rejected: &RejectedChildSlot,
) -> Option<CleanupMountParentCandidate> {
    let anchor = rejected.canonical_anchor()?;
    let claim = rejected.canonical_anchor_claim()?.clone();
    if anchor == rejected.slot().anchor() {
        return mounted_anchor_parent_candidate(rejected.slot());
    }
    let children = rejected.canonical_anchor_children()?.to_vec();
    Some(CleanupMountParentCandidate {
        claim,
        supplied_parent: None,
        removed_child: rejected.slot().slot(),
        removed_child_was_attached: false,
        children_before: children.clone(),
        children_after: children,
    })
}

fn freeze_external_mount_parents(
    world: &World,
    registry: &RouteEntityRegistry,
    obligations: &[CleanupObligation],
    candidates: impl IntoIterator<Item = CleanupMountParentCandidate>,
) -> Option<Vec<CleanupMountParentSettlement>> {
    let obligation_entities = obligations
        .iter()
        .map(|obligation| obligation.claim.entity())
        .collect::<HashSet<_>>();
    let mut parents = Vec::<CleanupMountParentSettlement>::new();
    for candidate in candidates {
        let claim = candidate.claim;
        let entity = claim.entity();
        if obligation_entities.contains(&entity) {
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
            return None;
        }
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
                return None;
            }
            continue;
        }
        let record = registry.record(entity)?;
        if record.claim() != claim || !record.manifest().matches(world, entity) {
            return None;
        }
        let (expected_parent, expected_parent_claim) =
            resolve_mount_parent_authority(registry, &claim, candidate.supplied_parent)?;
        let settlement = CleanupMountParentSettlement {
            claim,
            manifest: record.manifest().clone(),
            expected_parent,
            expected_parent_claim,
            removed_child: candidate.removed_child,
            removed_child_was_attached: candidate.removed_child_was_attached,
            children_before: candidate.children_before,
            children_after: candidate.children_after,
        };
        if !mount_parent_physical_matches(world, registry, &settlement) {
            return None;
        }
        parents.push(settlement);
    }
    parents.sort_by_key(|parent| parent.claim.entity().to_bits());
    Some(parents)
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
    let Some(actual_manifest) = ComponentManifest::from_world(world, parent.claim.entity()) else {
        return false;
    };
    let children = world.components().component_id::<Children>();
    let manifest_change_is_allowed = actual_manifest
        .component_ids()
        .iter()
        .filter(|component| !parent.manifest.component_ids().contains(component))
        .chain(
            parent
                .manifest
                .component_ids()
                .iter()
                .filter(|component| !actual_manifest.component_ids().contains(component)),
        )
        .all(|component| Some(*component) == children);
    if !manifest_change_is_allowed {
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
            record.lifecycle() == EntityLifecycle::PreparedInert
                && world.get::<ChildSlotLifecycle>(parent.claim.entity())
                    == Some(&ChildSlotLifecycle::RegisteredInert)
                && world
                    .get::<ChildSlotAuthority>(parent.claim.entity())
                    .is_some_and(|authority| {
                        authority.parent() == frame
                            && authority.outlet() == outlet
                            && authority.slot() == *slot
                    })
                && mount_slot_values_match(world, parent.claim.entity(), Visibility::Hidden)
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
    let record = registry.record(slot.slot())?;
    let anchor_record = registry.record(slot.anchor())?;
    if record.claim() != *slot.slot_claim()
        || anchor_record.claim() != *slot.anchor_claim()
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
        claim: slot.slot_claim().clone(),
        manifest: record.manifest().clone(),
        expected_parent: slot.anchor(),
        unit_root: slot.slot(),
        authorized_parent: slot.anchor(),
        allow_missing_components: false,
        force_unresolved: false,
    })
}

fn rejected_slot_obligation(
    registry: &RouteEntityRegistry,
    rejected: &RejectedChildSlot,
) -> Option<CleanupObligation> {
    let slot = rejected.slot();
    let record = registry.record(slot.slot())?;
    if record.claim() != *slot.slot_claim() {
        return None;
    }
    let RegistryOwner::Committed(EntityOwner::ChildSlot {
        parent: source_parent,
        outlet: source_outlet,
        slot: source_slot,
    }) = slot.slot_claim().owner()
    else {
        return None;
    };
    let canonical = rejected
        .canonical_anchor()
        .zip(rejected.canonical_anchor_claim())
        .filter(|(anchor, claim)| {
            slot.parent() == source_parent
                && slot.outlet() == source_outlet
                && slot.slot_id() == *source_slot
                && registry
                    .record(*anchor)
                    .is_some_and(|record| record.claim() == **claim)
                && matches!(
                    claim.owner(),
                    RegistryOwner::Committed(EntityOwner::FrameOutput { frame, .. })
                        if frame == source_parent
                )
        });
    let expected_parent = canonical
        .map(|(anchor, _)| anchor)
        .unwrap_or_else(|| slot.anchor());
    Some(CleanupObligation {
        claim: slot.slot_claim().clone(),
        manifest: record.manifest().clone(),
        expected_parent,
        unit_root: slot.slot(),
        authorized_parent: expected_parent,
        allow_missing_components: false,
        force_unresolved: canonical.is_none(),
    })
}

fn residual_slot_obligation(
    _world: &World,
    registry: &RouteEntityRegistry,
    residual: &ChildSlotResidual,
) -> Option<CleanupObligation> {
    let record = registry.record(residual.entity())?;
    let anchor_record = registry.record(residual.anchor())?;
    let owner_is_exact = matches!(
        residual.claim().owner(),
        RegistryOwner::Committed(EntityOwner::ChildSlot { parent, outlet, .. })
            if parent == residual.parent() && outlet == residual.outlet()
    ) || matches!(
        residual.claim().owner(),
        RegistryOwner::Provisional(ProvisionalOwner::ChildSlot { parent, outlet, .. })
            if parent == residual.parent() && outlet == residual.outlet()
    );
    if record.claim() != *residual.claim()
        || anchor_record.claim() != *residual.anchor_claim()
        || !owner_is_exact
    {
        return None;
    }
    Some(CleanupObligation {
        claim: residual.claim().clone(),
        manifest: residual.authorized_manifest().clone(),
        expected_parent: residual.anchor(),
        unit_root: residual.entity(),
        authorized_parent: residual.anchor(),
        allow_missing_components: matches!(
            residual.state(),
            crate::mount::ChildSlotResidualState::Provisional
                | crate::mount::ChildSlotResidualState::Absent
        ),
        force_unresolved: false,
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
    Exact {
        actual_parent: Option<Entity>,
        normalized_manifest: ComponentManifest,
        foreign_descendants: Vec<Entity>,
        foreign_components: Vec<ComponentId>,
        missing_components: Vec<ComponentId>,
        protected_parent: Option<Entity>,
    },
    RelationshipCycleOrRevisit,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PostSettlementFocusAuthority {
    Quarantine,
    Unresolved,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PostSettlementFocusRoot {
    obligation: usize,
    source_claim: ExactRegistryClaim,
    terminal_claim: ExactRegistryClaim,
    authority: PostSettlementFocusAuthority,
    allow_missing_components: bool,
}

pub(crate) struct PreparedCleanupLedger {
    batch: Option<CleanupBatch>,
    obligations: Vec<CleanupObligation>,
    mount_parents: Vec<CleanupMountParentSettlement>,
    manifest_digest: u64,
    receipts: Vec<Option<CleanupReceipt>>,
    ledger_domain: u64,
}

impl PreparedCleanupLedger {
    pub(crate) fn bind_batch(mut self, batch: CleanupBatch) -> Self {
        self.batch = Some(batch);
        self
    }
}

pub(crate) struct CleanupLedger {
    batch: Option<CleanupBatch>,
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
}

pub(crate) enum CleanupOwner {
    Leaving(RealizedBranch),
    FailedPrepared(FailedPreparedBranch),
}

impl CleanupOwner {
    fn into_single_leaving_frame(self) -> Result<RealizedFrame, Self> {
        let Self::Leaving(branch) = self else {
            return Err(self);
        };
        match branch.into_single_frame() {
            Ok(Some(frame)) => Ok(frame),
            Ok(None) => Err(Self::Leaving(RealizedBranch::empty())),
            Err(branch) => Err(Self::Leaving(branch)),
        }
    }
}

impl CleanupLedger {
    #[cfg(test)]
    pub(crate) fn prepare(
        batch: CleanupBatch,
        manifest: FrozenCleanupManifest,
    ) -> Option<PreparedCleanupLedger> {
        if !cleanup_batch_matches(&batch, manifest.digest, manifest.obligations.len()) {
            return None;
        }
        Some(Self::reserve(manifest)?.bind_batch(batch))
    }

    pub(crate) fn reserve(manifest: FrozenCleanupManifest) -> Option<PreparedCleanupLedger> {
        Self::prepare_physical(None, manifest)
    }

    fn prepare_physical(
        batch: Option<CleanupBatch>,
        manifest: FrozenCleanupManifest,
    ) -> Option<PreparedCleanupLedger> {
        if reservation_fault_injected() {
            return None;
        }
        let mut receipts = Vec::new();
        receipts
            .try_reserve_exact(manifest.obligations.len())
            .ok()?;
        receipts.resize(manifest.obligations.len(), None);
        let ledger_domain = allocate_ledger_domain(&NEXT_LEDGER_DOMAIN)?;
        Some(PreparedCleanupLedger {
            batch,
            obligations: manifest.obligations,
            mount_parents: manifest.mount_parents,
            manifest_digest: manifest.digest,
            receipts,
            ledger_domain,
        })
    }

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
        let Some(prepared) = Self::reserve(manifest) else {
            return Err(frame);
        };
        Ok(Self::activate(prepared, Some(frame)))
    }

    pub(crate) fn for_failed_prepared_branch(
        world: &World,
        branch: FailedPreparedBranch,
    ) -> Result<Self, FailedPreparedBranch> {
        let manifest = {
            let Some(registry) = world.get_resource::<RouteEntityRegistry>() else {
                return Err(branch);
            };
            let Some(manifest) =
                FrozenCleanupManifest::from_failed_prepared_branch(world, registry, &branch)
            else {
                return Err(branch);
            };
            manifest
        };
        let Some(prepared) = Self::reserve(manifest) else {
            return Err(branch);
        };
        Ok(Self::activate_failed(prepared, branch))
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

    fn activate_owner(prepared: PreparedCleanupLedger, owner: Option<CleanupOwner>) -> Self {
        let completion_digest = prepared
            .obligations
            .is_empty()
            .then_some(canonical_empty_completion_digest());
        Self {
            batch: prepared.batch,
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
        }
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

    pub(crate) fn batch(&self) -> Option<&CleanupBatch> {
        self.batch.as_ref()
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn leaving_frames(&self) -> &[RealizedFrame] {
        match self.owner.as_ref() {
            Some(CleanupOwner::Leaving(branch)) => branch.frames(),
            Some(CleanupOwner::FailedPrepared(_)) | None => &[],
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn failed_prepared_frames(&self) -> &[RealizedFrame] {
        match self.owner.as_ref() {
            Some(CleanupOwner::FailedPrepared(branch)) => branch.frames(),
            Some(CleanupOwner::Leaving(_)) | None => &[],
        }
    }

    #[cfg(feature = "test-support")]
    pub(crate) fn tracked_child_slots(&self) -> Vec<crate::branch_transaction::TrackedChildSlot> {
        match self.owner.as_ref() {
            Some(CleanupOwner::Leaving(branch)) => branch.tracked_child_slots(),
            Some(CleanupOwner::FailedPrepared(branch)) => branch.tracked_child_slots(),
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
        let current_evidence = collect_evidence(world, &self.obligations, &self.receipts);
        if current_evidence != attempt.evidence
            || current_evidence.iter().any(|(_, evidence)| {
                matches!(
                    evidence,
                    CleanupEvidence::RelationshipCycleOrRevisit | CleanupEvidence::StaleClaim
                )
            })
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
            self.owner.take();
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
                CleanupEvidence::RelationshipCycleOrRevisit | CleanupEvidence::StaleClaim => {
                    unreachable!()
                }
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
        self.owner.take();
        self.finish_if_settled();
        self.completion_digest.is_some()
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
                let change_is_allowed = actual
                    .component_ids()
                    .iter()
                    .filter(|component| !parent.manifest.component_ids().contains(component))
                    .chain(
                        parent
                            .manifest
                            .component_ids()
                            .iter()
                            .filter(|component| !actual.component_ids().contains(component)),
                    )
                    .all(|component| Some(*component) == children);
                change_is_allowed.then_some((parent.claim.clone(), parent.manifest.clone(), actual))
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
        exchanges.iter().all(|(claim, _, actual)| {
            world
                .resource::<RouteEntityRegistry>()
                .record(claim.entity())
                .is_some_and(|record| {
                    record.claim() == *claim
                        && record.manifest() == actual
                        && actual.matches(world, claim.entity())
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
                CleanupEvidence::Exact { .. } => transfer_indices.push(*index),
                CleanupEvidence::RelationshipCycleOrRevisit | CleanupEvidence::StaleClaim => {
                    return false;
                }
            }
        }
        if transfer_indices.is_empty() {
            return true;
        }

        let (unresolved_indices, verified_indices): (Vec<_>, Vec<_>) =
            transfer_indices.iter().copied().partition(|index| {
                self.obligations[*index].force_unresolved
                    || !cleanup_claim_has_exact_authority(world, &self.obligations[*index].claim)
            });
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
                CleanupReceipt::new(
                    self.ledger_domain,
                    attempt.revision,
                    *index,
                    disposition,
                    &self.obligations[*index],
                )
            })
            .collect::<Vec<_>>();
        let Some(quarantine_receipts) = self.prepare_receipt_batch(attempt, quarantine_receipts)
        else {
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
            && unresolved_indices
                .iter()
                .all(|index| unresolved_obligation_is_isolated(world, &self.obligations[*index]))
            && transfer_cleanup_quarantine(
                world,
                verified_claims,
                unresolved_claims,
                facts,
                focus_closure.clone(),
            );
        if !transferred {
            if self.receipts.iter().all(Option::is_none)
                && transfer_indices.len() == self.obligations.len()
            {
                let Some(owner) = self.owner.take() else {
                    return false;
                };
                let frame = match owner.into_single_leaving_frame() {
                    Ok(frame) => frame,
                    Err(owner) => {
                        self.owner = Some(owner);
                        return false;
                    }
                };
                match settle_frame(world, frame) {
                    Ok(()) => {
                        let Some(focus_roots) = self.terminal_focus_roots(world, &transfer_indices)
                        else {
                            return false;
                        };
                        self.commit_receipt_batch(quarantine_receipts);
                        self.remember_post_settlement_focus(focus_roots);
                        release_cleanup_focus(world, &focus_closure);
                        #[cfg(test)]
                        apply_cleanup_postcheck_drift(world, CleanupPostcheckPhase::Fallback);
                        if !focus_is_released(world, &focus_closure)
                            || !self.mount_parents_physical_match(world)
                        {
                            return false;
                        }
                        return true;
                    }
                    Err(frame) => {
                        self.owner = Some(CleanupOwner::Leaving(
                            RealizedBranch::from_single_frame(frame),
                        ));
                    }
                }
            }
            return false;
        }
        let Some(focus_roots) = self.terminal_focus_roots(world, &transfer_indices) else {
            return false;
        };
        self.commit_receipt_batch(quarantine_receipts);
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
        indices
            .iter()
            .map(|index| {
                let obligation = self.obligations.get(*index)?;
                let entity = obligation.claim.entity();
                match (table.record(entity), table.unresolved_record(entity)) {
                    (Some(record), None)
                        if record.owner() == obligation.claim.owner()
                            && record.seal() == obligation.claim.seal()
                            && record.lifecycle() == EntityLifecycle::Quarantined =>
                    {
                        Some(PostSettlementFocusRoot {
                            obligation: *index,
                            source_claim: obligation.claim.clone(),
                            terminal_claim: record.claim(),
                            authority: PostSettlementFocusAuthority::Quarantine,
                            allow_missing_components: obligation.allow_missing_components,
                        })
                    }
                    (None, Some(record)) if record.claim() == obligation.claim => {
                        Some(PostSettlementFocusRoot {
                            obligation: *index,
                            source_claim: obligation.claim.clone(),
                            terminal_claim: record.claim(),
                            authority: PostSettlementFocusAuthority::Unresolved,
                            allow_missing_components: obligation.allow_missing_components,
                        })
                    }
                    _ => None,
                }
            })
            .collect()
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

fn transfer_cleanup_quarantine(
    world: &mut World,
    verified_claims: Vec<ExactRegistryClaim>,
    unresolved_claims: Vec<ExactRegistryClaim>,
    facts: QuarantineFacts,
    focus_closure: Vec<Entity>,
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
        let Some(manifest) = ComponentManifest::from_world(world, claim.entity()) else {
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
                            facts,
                        ) else {
                            return false;
                        };
                        Some(transfer)
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
        && (frame_is_exact || child_slot_is_exact)
}

fn cleanup_claim_has_exact_authority(world: &World, claim: &ExactRegistryClaim) -> bool {
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
        .all(&claim_is_exact)
        && branch
            .child_slots()
            .iter()
            .all(|slot| claim_is_exact(slot.slot_claim()))
        && branch.mount_residuals().iter().all(|residual| {
            claim_is_registered(residual.claim())
                && residual_child_slot_is_isolated(world, residual)
        })
        && branch
            .rejected_child_slots()
            .iter()
            .all(|rejected| claim_is_exact(rejected.slot().slot_claim()))
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
            },
            CleanupEvidence::Exact {
                actual_parent,
                normalized_manifest: actual_manifest,
                foreign_descendants: actual_descendants,
                foreign_components: actual_foreign,
                missing_components: actual_missing,
                protected_parent: actual_protected,
            },
        ) = (expected, actual)
        else {
            return false;
        };
        expected_parent == actual_parent
            && expected_descendants == actual_descendants
            && expected_protected == actual_protected
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

fn classify_obligation(
    world: &World,
    obligations: &[CleanupObligation],
    receipts: &[Option<CleanupReceipt>],
    index: usize,
) -> CleanupEvidence {
    let obligation = &obligations[index];
    if world.get_entity(obligation.claim.entity()).is_err() {
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
        return CleanupEvidence::StaleClaim;
    };
    if record.claim() != obligation.claim {
        return CleanupEvidence::StaleClaim;
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
        return CleanupEvidence::StaleClaim;
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
    let mut stack = world
        .get::<Children>(obligation.claim.entity())
        .into_iter()
        .flat_map(|children| children.iter().copied())
        .collect::<Vec<_>>();
    while let Some(entity) = stack.pop() {
        if !visited.insert(entity) {
            return CleanupEvidence::RelationshipCycleOrRevisit;
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
    let protected_parent = actual_parent.filter(|parent| {
        *parent != obligation.expected_parent
            && *parent != obligation.authorized_parent
            && !unsettled_owned.contains(parent)
    });
    CleanupEvidence::Exact {
        actual_parent,
        normalized_manifest,
        foreign_descendants,
        foreign_components,
        missing_components,
        protected_parent,
    }
}

fn quarantine_disposition(
    evidence: &[(usize, CleanupEvidence)],
    obligations: &[CleanupObligation],
) -> Option<CleanupDisposition> {
    if evidence
        .iter()
        .any(|(index, _)| obligations[*index].force_unresolved)
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

fn quarantine_facts(evidence: &[(usize, CleanupEvidence)]) -> QuarantineFacts {
    let mut protected_parents: Vec<Entity> = Vec::new();
    let mut foreign_descendants: Vec<Entity> = Vec::new();
    let mut foreign_components: Vec<ComponentId> = Vec::new();
    let mut missing_components: Vec<ComponentId> = Vec::new();
    for (_, evidence) in evidence {
        if let CleanupEvidence::Exact {
            protected_parent,
            foreign_descendants: descendants,
            foreign_components: components,
            missing_components: missing,
            ..
        } = evidence
        {
            protected_parents.extend(protected_parent);
            foreign_descendants.extend(descendants);
            foreign_components.extend(components);
            missing_components.extend(missing);
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
        if let CleanupEvidence::Exact {
            foreign_descendants,
            ..
        } = evidence
        {
            entities.push(obligations[*index].claim.entity());
            entities.extend(foreign_descendants);
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
        self.push(obligation.manifest.component_ids().len() as u64);
        for component in obligation.manifest.component_ids() {
            self.push(component.index() as u64);
        }
        self.push(obligation.expected_parent.to_bits());
        self.push(obligation.unit_root.to_bits());
        self.push(obligation.authorized_parent.to_bits());
        self.push(u64::from(obligation.allow_missing_components));
        self.push(u64::from(obligation.force_unresolved));
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
        engine.publish(prepared, || {});
        (engine, batch)
    }

    #[test]
    fn cleanup_attempt_identity_is_monotonic_and_private() {
        let source = include_str!("cleanup.rs");
        assert!(source.contains("next_attempt"));
        assert!(!source.contains("\npub struct CleanupAttempt"));
        assert!(!source.contains("\npub fn complete("));
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
    fn failed_prepared_branch_child_slot_lifecycle_drift_is_zero_mutation() {
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
        assert_eq!(ledger.receipt_count(), 0);
        assert_eq!(
            app.world().get::<ChildSlotLifecycle>(child_slot),
            Some(&ChildSlotLifecycle::ProvisionalInert)
        );
        assert_eq!(
            app.world()
                .resource::<RouteEntityRegistry>()
                .record(child_slot)
                .unwrap()
                .claim(),
            exact_claim
        );
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
    pub(crate) fn external_frame_output_value_and_manifest_drift_is_zero_write_and_retains_focus() {
        for drift in 0..5 {
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
                _ => unreachable!(),
            }
            app.world_mut().flush();
            let attempt = ledger.attempt(app.world()).unwrap();

            assert!(!ledger.settle(app.world_mut(), attempt));
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
    fn settled_completion_remains_replayable_until_core_acknowledges_it() {
        let (mut engine, batch) = published_empty_batch();
        let prepared = CleanupLedger::prepare(batch, FrozenCleanupManifest::empty()).unwrap();
        let ledger = CleanupLedger::activate(prepared, None);
        let first = ledger.pending_completion().unwrap();
        let replay = ledger.pending_completion().unwrap();
        engine.complete_cleanup(first).unwrap();
        assert!(engine.complete_cleanup(replay).is_err());
    }

    #[test]
    fn sealed_batch_must_match_the_frozen_ecs_manifest() {
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
            force_unresolved: false,
        }];
        let manifest = FrozenCleanupManifest {
            digest: manifest_digest(&obligations),
            obligations,
            mount_parents: Vec::new(),
        };
        let prepared = CleanupLedger::prepare_physical(None, manifest).unwrap();
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
    fn old_and_foreign_attempts_are_world_zero() {
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

        assert!(!ledger.settle(&mut world, old));
        assert!(!ledger.settle(&mut world, foreign));
        assert!(world.get_entity(entity).is_ok());
        assert_eq!(ledger.receipt_count(), 0);

        assert!(ledger.settle(&mut world, current));
        assert!(world.get_entity(entity).is_err());
    }

    #[test]
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

        assert!(!ledger.accept_receipt(&attempt, foreign));
        assert_eq!(ledger.receipt_count(), 0);

        assert!(ledger.settle(&mut world, attempt.clone()));
        let replay = ledger.receipts[0].clone().unwrap();
        let receipts_before = ledger.receipts.clone();
        let registry_before = world.resource::<RouteEntityRegistry>().records().to_vec();
        let world_before = world
            .iter_entities()
            .map(|entity| (entity.id(), entity.archetype().components().to_vec()))
            .collect::<Vec<_>>();
        assert!(!ledger.accept_receipt(&attempt, replay));
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
    fn cleanup_receipt_observation_is_exact_once() {
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
        assert!(!ledger.mark_receipt_observed(obligation, disposition));
    }

    #[test]
    fn partial_receipt_remains_observable_before_aggregate_completion() {
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
