use bevy_ui_router_core::{
    frame::{FrameOrigin, RouteFrameVersion},
    ids::{NavigationRequestId, RouteFrameId, RouteGeneration, RouteHostId, RouteId, RouteSlotId},
    integration::{
        CommitTicket, FrameMountTarget, ReconciliationChildSlotRoleKind,
        ReconciliationEnteringSlotKind, ReconciliationProjectionView, RootFrameTask,
    },
    slot::RouteSlotKey,
};

use crate::{
    mount::{ChildSlotResidual, MountedChildSlot},
    presentation::{
        FrameMutationTarget, PreparedFrameMountRebase, RealizedFrame, child_slot_mutation_target,
        claim_rebind_is_exact,
    },
    registry::{EntityLifecycle, EntityOwner, ExactRegistryClaim, RegistryOwner},
};

#[cfg(feature = "test-support")]
pub(crate) struct TrackedChildSlot {
    entity: bevy_ecs::prelude::Entity,
    claim: crate::registry::ExactRegistryClaim,
    parent: bevy_ui_router_core::frame::RouteFrameVersion,
    outlet: bevy_ui_router_core::slot::RouteSlotKey,
    slot: bevy_ui_router_core::ids::RouteSlotId,
    anchor: bevy_ecs::prelude::Entity,
    anchor_claim: crate::registry::ExactRegistryClaim,
    expected_anchor_children: Vec<bevy_ecs::prelude::Entity>,
}

#[cfg(feature = "test-support")]
impl TrackedChildSlot {
    fn from_mounted(slot: &MountedChildSlot) -> Self {
        Self {
            entity: slot.slot(),
            claim: slot.slot_claim().clone(),
            parent: slot.parent().clone(),
            outlet: slot.outlet().clone(),
            slot: slot.slot_id(),
            anchor: slot.anchor(),
            anchor_claim: slot.anchor_claim().clone(),
            expected_anchor_children: slot.expected_anchor_children().to_vec(),
        }
    }

    pub(crate) const fn entity(&self) -> bevy_ecs::prelude::Entity {
        self.entity
    }

    pub(crate) const fn parent(&self) -> &bevy_ui_router_core::frame::RouteFrameVersion {
        &self.parent
    }

    pub(crate) const fn outlet(&self) -> &bevy_ui_router_core::slot::RouteSlotKey {
        &self.outlet
    }

    pub(crate) const fn slot(&self) -> bevy_ui_router_core::ids::RouteSlotId {
        self.slot
    }

    pub(crate) const fn anchor(&self) -> bevy_ecs::prelude::Entity {
        self.anchor
    }

    pub(crate) fn expected_anchor_children(&self) -> &[bevy_ecs::prelude::Entity] {
        &self.expected_anchor_children
    }

    pub(crate) fn is_current(&self, world: &bevy_ecs::prelude::World) -> bool {
        use crate::{
            components::{
                ChildSlotAuthority, ChildSlotLifecycle, RouteOwnershipSeal, RouterInputBarrier,
            },
            registry::{EntityOwner, RegistryOwner, RouteEntityRegistry},
        };
        use bevy_camera::visibility::Visibility;
        use bevy_ecs::hierarchy::{ChildOf, Children};
        use bevy_picking::Pickable;
        use bevy_ui::{Node, Val};

        let Some(record) = world
            .get_resource::<RouteEntityRegistry>()
            .and_then(|registry| registry.record(self.entity))
        else {
            return false;
        };
        let (component_lifecycle, visibility) = match self.claim.lifecycle() {
            crate::registry::EntityLifecycle::PreparedInert => {
                (ChildSlotLifecycle::RegisteredInert, Visibility::Hidden)
            }
            crate::registry::EntityLifecycle::Active => {
                (ChildSlotLifecycle::Active, Visibility::Visible)
            }
            crate::registry::EntityLifecycle::Leaving => {
                (ChildSlotLifecycle::Leaving, Visibility::Hidden)
            }
            crate::registry::EntityLifecycle::Quarantined => return false,
        };
        let anchor_is_exact = world
            .get_resource::<RouteEntityRegistry>()
            .and_then(|registry| registry.record(self.anchor))
            .is_some_and(|record| {
                record.claim() == self.anchor_claim && record.manifest().matches(world, self.anchor)
            });
        record.claim() == self.claim
            && record.lifecycle() == self.claim.lifecycle()
            && record.manifest().matches(world, self.entity)
            && matches!(
                record.owner(),
                RegistryOwner::Committed(EntityOwner::ChildSlot { parent, outlet, slot })
                    if parent == &self.parent && outlet == &self.outlet && *slot == self.slot
            )
            && world.get::<RouteOwnershipSeal>(self.entity)
                == Some(&RouteOwnershipSeal::new(self.claim.seal()))
            && world.get::<ChildSlotLifecycle>(self.entity) == Some(&component_lifecycle)
            && world
                .get::<ChildSlotAuthority>(self.entity)
                .is_some_and(|authority| {
                    authority.parent() == &self.parent
                        && authority.outlet() == &self.outlet
                        && authority.slot() == self.slot
                })
            && world.get::<ChildOf>(self.entity).map(ChildOf::parent) == Some(self.anchor)
            && world.get::<Visibility>(self.entity) == Some(&visibility)
            && world.get::<Pickable>(self.entity) == Some(&Pickable::IGNORE)
            && world.get::<Node>(self.entity)
                == Some(&Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..Node::default()
                })
            && world
                .get::<RouterInputBarrier>(self.entity)
                .is_some_and(RouterInputBarrier::denies_all)
            && world
                .get::<Children>(self.anchor)
                .map(|children| children.iter().copied().collect::<Vec<_>>())
                .unwrap_or_default()
                == self.expected_anchor_children
            && anchor_is_exact
    }
}

#[derive(Debug)]
pub(crate) struct RealizedBranch {
    frames: Vec<RealizedFrame>,
    child_slots: Vec<MountedChildSlot>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReconciliationSlotRoleKind {
    PreservedActive,
    RetainEmpty,
    ActivateExisting,
    MountNew,
    Leaving,
}

impl ReconciliationSlotRoleKind {
    fn from_core(kind: ReconciliationChildSlotRoleKind) -> Option<Self> {
        match kind {
            ReconciliationChildSlotRoleKind::PreservedActive => Some(Self::PreservedActive),
            ReconciliationChildSlotRoleKind::RetainEmpty => Some(Self::RetainEmpty),
            ReconciliationChildSlotRoleKind::ActivateExisting => Some(Self::ActivateExisting),
            ReconciliationChildSlotRoleKind::MountNew => Some(Self::MountNew),
            ReconciliationChildSlotRoleKind::Leaving => Some(Self::Leaving),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReconciliationAuthorityIdentity {
    router_anchor: RouteFrameVersion,
    host: RouteHostId,
    top_slot: RouteSlotId,
}

impl ReconciliationAuthorityIdentity {
    pub(crate) fn from_frame(
        router_anchor: &RouteFrameVersion,
        host: RouteHostId,
        top_slot: RouteSlotId,
    ) -> Option<Self> {
        (router_anchor.host() == host).then(|| Self {
            router_anchor: router_anchor.clone(),
            host,
            top_slot,
        })
    }

    fn matches(&self, frame: &RouteFrameVersion) -> bool {
        bevy_ui_router_core::integration::frame_versions_share_router(&self.router_anchor, frame)
            && frame.host() == self.host
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ReconciliationExistingFrame {
    branch_index: usize,
    version: RouteFrameVersion,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ReconciliationExistingSlot {
    parent_branch_index: usize,
    parent: RouteFrameVersion,
    outlet: RouteSlotKey,
    slot: RouteSlotId,
    child: Option<RouteFrameVersion>,
    role: ReconciliationSlotRoleKind,
}

#[derive(Debug)]
struct ReconciliationExistingProjection {
    preserved: Vec<ReconciliationExistingFrame>,
    leaving: Vec<ReconciliationExistingFrame>,
    child_slots: Vec<ReconciliationExistingSlot>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReconciliationEnteringSlot {
    Existing(RouteSlotId),
    MountNew,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ReconciliationEnteringFrame {
    branch_index: usize,
    route: RouteId,
    frame: RouteFrameId,
    generation: RouteGeneration,
    slot: ReconciliationEnteringSlot,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ReconciliationMountNewSlot {
    parent_branch_index: usize,
    outlet: RouteSlotKey,
}

#[derive(Debug)]
struct ReconciliationEnteringProjection {
    frames: Vec<ReconciliationEnteringFrame>,
    mount_new_slots: Vec<ReconciliationMountNewSlot>,
}

pub(crate) struct ReconciliationProjectionFacts {
    authority: ReconciliationAuthorityIdentity,
    existing: ReconciliationExistingProjection,
    entering: ReconciliationEnteringProjection,
}

pub(crate) struct SealedReconciliationStagingMount<'a> {
    parent: &'a RealizedFrame,
    slot: &'a MountedChildSlot,
    role: ReconciliationSlotRoleKind,
    expected_slot_lifecycle: EntityLifecycle,
    allowed_existing_shells: Vec<ExactRegistryClaim>,
}

impl<'a> SealedReconciliationStagingMount<'a> {
    pub(crate) const fn parent(&self) -> &'a RealizedFrame {
        self.parent
    }

    pub(crate) const fn slot(&self) -> &'a MountedChildSlot {
        self.slot
    }

    pub(crate) const fn role(&self) -> ReconciliationSlotRoleKind {
        self.role
    }

    pub(crate) const fn expected_slot_lifecycle(&self) -> EntityLifecycle {
        self.expected_slot_lifecycle
    }

    pub(crate) fn allowed_existing_shells(&self) -> &[ExactRegistryClaim] {
        &self.allowed_existing_shells
    }
}

impl ReconciliationProjectionFacts {
    pub(crate) fn from_core(
        projection: &ReconciliationProjectionView<'_>,
        authority: ReconciliationAuthorityIdentity,
    ) -> Option<Self> {
        let preserved = projection
            .preserved()
            .map(|frame| ReconciliationExistingFrame {
                branch_index: frame.branch_index(),
                version: frame.version().clone(),
            })
            .collect();
        let leaving = projection
            .leaving()
            .map(|frame| ReconciliationExistingFrame {
                branch_index: frame.branch_index(),
                version: frame.version().clone(),
            })
            .collect();
        let frames = projection
            .entering()
            .map(|frame| ReconciliationEnteringFrame {
                branch_index: frame.branch_index(),
                route: frame.route(),
                frame: frame.frame(),
                generation: frame.generation(),
                slot: match frame.slot() {
                    ReconciliationEnteringSlotKind::Existing(slot) => {
                        ReconciliationEnteringSlot::Existing(slot)
                    }
                    ReconciliationEnteringSlotKind::MountNew => {
                        ReconciliationEnteringSlot::MountNew
                    }
                },
            })
            .collect();
        let mut child_slots = Vec::new();
        let mut mount_new_slots = Vec::new();
        for slot in projection.child_slots() {
            if slot.kind() == ReconciliationChildSlotRoleKind::MountNew {
                if slot.binding().is_some() {
                    return None;
                }
                mount_new_slots.push(ReconciliationMountNewSlot {
                    parent_branch_index: slot.parent_branch_index(),
                    outlet: slot.outlet().clone(),
                });
                continue;
            }
            let binding = slot.binding()?;
            if slot.parent_branch_index() != binding.parent_branch_index()
                || slot.outlet() != binding.outlet()
            {
                return None;
            }
            child_slots.push(ReconciliationExistingSlot {
                parent_branch_index: binding.parent_branch_index(),
                parent: binding.parent().clone(),
                outlet: binding.outlet().clone(),
                slot: binding.slot(),
                child: binding.child().cloned(),
                role: ReconciliationSlotRoleKind::from_core(slot.kind())?,
            });
        }
        Some(Self {
            authority,
            existing: ReconciliationExistingProjection {
                preserved,
                leaving,
                child_slots,
            },
            entering: ReconciliationEnteringProjection {
                frames,
                mount_new_slots,
            },
        })
    }
}

pub(crate) fn seal_reconciliation_staging_mount<'a>(
    projection: &ReconciliationProjectionView<'_>,
    active: Option<&'a RealizedBranch>,
    entering: &'a PreparedBranch,
    task: &RootFrameTask,
) -> Option<SealedReconciliationStagingMount<'a>> {
    let root = active
        .and_then(RealizedBranch::first_frame)
        .or_else(|| entering.first_frame())?;
    let authority = ReconciliationAuthorityIdentity::from_frame(
        root.frame(),
        root.frame().host(),
        root.frame().slot(),
    )?;
    let facts = ReconciliationProjectionFacts::from_core(projection, authority)?;
    let projected = facts.entering.frames.iter().find(|projected| {
        projected.frame == task.frame().frame()
            && projected.generation == task.frame().generation()
            && matches!(
                task.frame().origin(),
                FrameOrigin::MatchedRoute(route) if *route == projected.route
            )
    })?;
    let FrameMountTarget::ChildOutlet { parent, outlet } = task.mount_target() else {
        return None;
    };
    let parent_index = projected.branch_index.checked_sub(1)?;
    let parents = entering
        .frames()
        .iter()
        .chain(active.into_iter().flat_map(RealizedBranch::frames))
        .filter(|frame| frame.frame() == &parent)
        .collect::<Vec<_>>();
    let slots = entering
        .child_slots()
        .iter()
        .chain(active.into_iter().flat_map(RealizedBranch::child_slots))
        .filter(|slot| {
            slot.parent() == &parent
                && slot.outlet() == &outlet
                && slot.slot_id() == task.frame().slot()
        })
        .collect::<Vec<_>>();
    if parents.len() != 1 || slots.len() != 1 {
        return None;
    }
    let role = match projected.slot {
        ReconciliationEnteringSlot::Existing(slot) if slot == slots[0].slot_id() => {
            facts
                .existing
                .child_slots
                .iter()
                .find(|candidate| {
                    candidate.parent_branch_index == parent_index
                        && candidate.parent == parent
                        && candidate.outlet == outlet
                        && candidate.slot == slot
                })?
                .role
        }
        ReconciliationEnteringSlot::MountNew => {
            facts.entering.mount_new_slots.iter().find(|candidate| {
                candidate.parent_branch_index == parent_index && candidate.outlet == outlet
            })?;
            ReconciliationSlotRoleKind::MountNew
        }
        ReconciliationEnteringSlot::Existing(_) => return None,
    };
    let expected_slot_lifecycle = match role {
        ReconciliationSlotRoleKind::PreservedActive => EntityLifecycle::Active,
        ReconciliationSlotRoleKind::ActivateExisting | ReconciliationSlotRoleKind::MountNew => {
            EntityLifecycle::PreparedInert
        }
        ReconciliationSlotRoleKind::RetainEmpty | ReconciliationSlotRoleKind::Leaving => {
            return None;
        }
    };
    let allowed_existing_shells: Vec<ExactRegistryClaim> = active
        .into_iter()
        .flat_map(RealizedBranch::frames)
        .filter(|frame| {
            frame.mount_binding().matches_mounted_child_slot(slots[0])
                && facts
                    .existing
                    .leaving
                    .iter()
                    .any(|leaving| leaving.version == *frame.frame())
        })
        .filter_map(|frame| {
            frame
                .claims()
                .find(|claim| {
                    claim.entity() == frame.shell()
                        && matches!(
                            claim.owner(),
                            RegistryOwner::Committed(EntityOwner::FrameShell(owner))
                                if owner == frame.frame()
                        )
                })
                .cloned()
        })
        .collect();
    (slots[0].slot_claim().lifecycle() == expected_slot_lifecycle).then_some(
        SealedReconciliationStagingMount {
            parent: parents[0],
            slot: slots[0],
            role,
            expected_slot_lifecycle,
            allowed_existing_shells,
        },
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReconciliationSpliceError {
    InvalidAuthorityIdentity,
    InvalidFramePartition,
    InvalidSlotPartition,
    DuplicateAuthority,
    InvalidEnteringBranch,
}

#[derive(Debug)]
pub(crate) struct ReconciliationPreparationFailure {
    branch: RealizedBranch,
    error: ReconciliationSpliceError,
}

impl ReconciliationPreparationFailure {
    pub(crate) fn into_parts(self) -> (RealizedBranch, ReconciliationSpliceError) {
        (self.branch, self.error)
    }
}

#[derive(Debug)]
pub(crate) struct SealedReconciliationSlotRole {
    claim: ExactRegistryClaim,
    parent: RouteFrameVersion,
    outlet: RouteSlotKey,
    slot: RouteSlotId,
    anchor: bevy_ecs::prelude::Entity,
    anchor_claim: ExactRegistryClaim,
    kind: ReconciliationSlotRoleKind,
    next: Option<EntityLifecycle>,
    entering_use: ReconciliationSlotEnteringUse,
}

#[derive(Debug)]
pub(crate) struct SealedMountNewStagingAllowance {
    request: NavigationRequestId,
    parent: RouteFrameVersion,
    outlet: RouteSlotKey,
    slot: RouteSlotId,
    entity: bevy_ecs::prelude::Entity,
    claim: ExactRegistryClaim,
    anchor: bevy_ecs::prelude::Entity,
    anchor_claim: ExactRegistryClaim,
    expected_anchor_children: Vec<bevy_ecs::prelude::Entity>,
}

impl SealedMountNewStagingAllowance {
    pub(crate) fn seal(
        request: NavigationRequestId,
        parent: RouteFrameVersion,
        outlet: RouteSlotKey,
        slot: RouteSlotId,
        entity: bevy_ecs::prelude::Entity,
        claim: ExactRegistryClaim,
        anchor: bevy_ecs::prelude::Entity,
        anchor_claim: ExactRegistryClaim,
        expected_anchor_children: Vec<bevy_ecs::prelude::Entity>,
    ) -> Option<Self> {
        let allowance = Self {
            request,
            parent,
            outlet,
            slot,
            entity,
            claim,
            anchor,
            anchor_claim,
            expected_anchor_children,
        };
        allowance
            .matches_binding(
                &allowance.parent,
                &allowance.outlet,
                allowance.slot,
                allowance.entity,
                &allowance.claim,
                allowance.anchor,
                &allowance.anchor_claim,
            )
            .then_some(allowance)
    }

    pub(crate) const fn parent(&self) -> &RouteFrameVersion {
        &self.parent
    }

    pub(crate) const fn outlet(&self) -> &RouteSlotKey {
        &self.outlet
    }

    pub(crate) const fn slot(&self) -> RouteSlotId {
        self.slot
    }

    pub(crate) const fn entity(&self) -> bevy_ecs::prelude::Entity {
        self.entity
    }

    pub(crate) const fn claim(&self) -> &ExactRegistryClaim {
        &self.claim
    }

    pub(crate) const fn anchor(&self) -> bevy_ecs::prelude::Entity {
        self.anchor
    }

    pub(crate) const fn anchor_claim(&self) -> &ExactRegistryClaim {
        &self.anchor_claim
    }

    pub(crate) fn expected_anchor_children(&self) -> &[bevy_ecs::prelude::Entity] {
        &self.expected_anchor_children
    }

    pub(crate) fn matches_request(&self, request: NavigationRequestId) -> bool {
        self.request == request
    }

    pub(crate) fn matches_binding(
        &self,
        parent: &RouteFrameVersion,
        outlet: &RouteSlotKey,
        slot: RouteSlotId,
        entity: bevy_ecs::prelude::Entity,
        claim: &ExactRegistryClaim,
        anchor: bevy_ecs::prelude::Entity,
        anchor_claim: &ExactRegistryClaim,
    ) -> bool {
        self.parent == *parent
            && self.outlet == *outlet
            && self.slot == slot
            && self.entity == entity
            && self.claim == *claim
            && self.anchor == anchor
            && self.anchor_claim == *anchor_claim
            && claim.lifecycle() == EntityLifecycle::PreparedInert
            && matches!(
                claim.owner(),
                RegistryOwner::Provisional(crate::registry::ProvisionalOwner::ChildSlot {
                    parent: owner_parent,
                    outlet: owner_outlet,
                    slot: owner_slot,
                }) | RegistryOwner::Committed(EntityOwner::ChildSlot {
                    parent: owner_parent,
                    outlet: owner_outlet,
                    slot: owner_slot,
                }) if owner_parent == parent && owner_outlet == outlet && *owner_slot == slot
            )
    }

    pub(crate) fn rebind_residual(mut self, residual: &ChildSlotResidual) -> Option<Self> {
        if self.entity != residual.entity()
            || self.parent != *residual.parent()
            || self.outlet != *residual.outlet()
            || self.anchor != residual.anchor()
            || self.anchor_claim != *residual.anchor_claim()
            || !slot_claim_matches_mount_new(
                residual.claim(),
                &self.parent,
                &self.outlet,
                self.slot,
                self.entity,
            )
        {
            return None;
        }
        self.claim = residual.claim().clone();
        Some(self)
    }

    #[cfg(test)]
    fn for_test(
        request: NavigationRequestId,
        parent: RouteFrameVersion,
        outlet: RouteSlotKey,
        slot: RouteSlotId,
        entity: bevy_ecs::prelude::Entity,
        claim: ExactRegistryClaim,
        anchor: bevy_ecs::prelude::Entity,
        anchor_claim: ExactRegistryClaim,
        expected_anchor_children: Vec<bevy_ecs::prelude::Entity>,
    ) -> Self {
        Self {
            request,
            parent,
            outlet,
            slot,
            entity,
            claim,
            anchor,
            anchor_claim,
            expected_anchor_children,
        }
    }
}

fn slot_claim_matches_mount_new(
    claim: &ExactRegistryClaim,
    parent: &RouteFrameVersion,
    outlet: &RouteSlotKey,
    slot: RouteSlotId,
    entity: bevy_ecs::prelude::Entity,
) -> bool {
    claim.entity() == entity
        && claim.lifecycle() == EntityLifecycle::PreparedInert
        && matches!(
            claim.owner(),
            RegistryOwner::Provisional(crate::registry::ProvisionalOwner::ChildSlot {
                parent: owner_parent,
                outlet: owner_outlet,
                slot: owner_slot,
            }) | RegistryOwner::Committed(EntityOwner::ChildSlot {
                parent: owner_parent,
                outlet: owner_outlet,
                slot: owner_slot,
            }) if owner_parent == parent && owner_outlet == outlet && *owner_slot == slot
        )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReconciliationSlotEnteringUse {
    Required,
    Forbidden,
}

impl SealedReconciliationSlotRole {
    pub(crate) const fn entity(&self) -> bevy_ecs::prelude::Entity {
        self.claim.entity()
    }

    pub(crate) const fn claim(&self) -> &ExactRegistryClaim {
        &self.claim
    }

    pub(crate) const fn current(&self) -> EntityLifecycle {
        self.claim.lifecycle()
    }

    pub(crate) const fn next(&self) -> Option<EntityLifecycle> {
        self.next
    }

    pub(crate) const fn parent(&self) -> &RouteFrameVersion {
        &self.parent
    }

    pub(crate) const fn outlet(&self) -> &RouteSlotKey {
        &self.outlet
    }

    pub(crate) const fn slot(&self) -> RouteSlotId {
        self.slot
    }

    pub(crate) const fn anchor(&self) -> bevy_ecs::prelude::Entity {
        self.anchor
    }

    pub(crate) const fn anchor_claim(&self) -> &ExactRegistryClaim {
        &self.anchor_claim
    }

    pub(crate) const fn kind(&self) -> ReconciliationSlotRoleKind {
        self.kind
    }

    fn matches_mount_new_slot(&self, slot: &MountedChildSlot) -> bool {
        self.kind == ReconciliationSlotRoleKind::MountNew
            && self.current() == EntityLifecycle::PreparedInert
            && self.next == Some(EntityLifecycle::Active)
            && self.claim == *slot.slot_claim()
            && self.parent == *slot.parent()
            && self.outlet == *slot.outlet()
            && self.slot == slot.slot_id()
            && self.anchor == slot.anchor()
            && self.anchor_claim == *slot.anchor_claim()
    }
}

#[derive(Debug)]
pub(crate) struct SealedExistingReconciliation {
    authority: ReconciliationAuthorityIdentity,
    preserved: RealizedBranch,
    leaving: RealizedBranch,
    slot_roles: Vec<SealedReconciliationSlotRole>,
    entering: ReconciliationEnteringProjection,
}

#[derive(Debug)]
pub(crate) struct ReconciliationEnteringFailure {
    sealed: SealedExistingReconciliation,
    entering: PreparedBranch,
    error: ReconciliationSpliceError,
}

impl ReconciliationEnteringFailure {
    pub(crate) fn into_parts(
        self,
    ) -> (
        SealedExistingReconciliation,
        PreparedBranch,
        ReconciliationSpliceError,
    ) {
        (self.sealed, self.entering, self.error)
    }
}

#[derive(Debug)]
pub(crate) struct SealedReconciliationSplice {
    preserved: RealizedBranch,
    entering: RealizedBranch,
    leaving: RealizedBranch,
    slot_roles: Vec<SealedReconciliationSlotRole>,
}

pub(crate) struct ReconciliationSpliceOutcome {
    preserved: RealizedBranch,
    entering: RealizedBranch,
    leaving: RealizedBranch,
    slot_roles: Vec<SealedReconciliationSlotRole>,
}

pub(crate) struct PreparedReconciliationClaimRebind {
    preserved_frames: Vec<Vec<ExactRegistryClaim>>,
    entering_frames: Vec<Vec<ExactRegistryClaim>>,
    leaving_frames: Vec<Vec<ExactRegistryClaim>>,
    all_claims: Vec<ExactRegistryClaim>,
}

pub(crate) struct RejectedReconciliationOwners {
    active: RealizedBranch,
    entering: PreparedBranch,
    slot_roles: Vec<SealedReconciliationSlotRole>,
}

impl RejectedReconciliationOwners {
    pub(crate) fn into_parts(
        self,
    ) -> (
        RealizedBranch,
        PreparedBranch,
        Vec<SealedReconciliationSlotRole>,
    ) {
        (self.active, self.entering, self.slot_roles)
    }
}

impl ReconciliationSpliceOutcome {
    pub(crate) const fn preserved(&self) -> &RealizedBranch {
        &self.preserved
    }

    pub(crate) const fn entering(&self) -> &RealizedBranch {
        &self.entering
    }

    pub(crate) const fn leaving(&self) -> &RealizedBranch {
        &self.leaving
    }

    pub(crate) fn slot_roles(&self) -> &[SealedReconciliationSlotRole] {
        &self.slot_roles
    }

    pub(crate) fn into_active_and_leaving(self) -> (RealizedBranch, RealizedBranch) {
        let mut active = self.preserved;
        let mut entering = self.entering;
        active.frames.append(&mut entering.frames);
        active.child_slots.append(&mut entering.child_slots);
        sort_child_slots(&mut active);
        (active, self.leaving)
    }
}

impl SealedReconciliationSplice {
    pub(crate) const fn preserved(&self) -> &RealizedBranch {
        &self.preserved
    }

    pub(crate) const fn entering(&self) -> &RealizedBranch {
        &self.entering
    }

    pub(crate) const fn leaving(&self) -> &RealizedBranch {
        &self.leaving
    }

    pub(crate) fn slot_roles(&self) -> &[SealedReconciliationSlotRole] {
        &self.slot_roles
    }

    pub(crate) fn lifecycle_transitions(&self) -> Vec<(ExactRegistryClaim, EntityLifecycle)> {
        let mut transitions = self
            .leaving
            .lifecycle_transitions(EntityLifecycle::Leaving, true);
        transitions.extend(self.slot_roles.iter().filter_map(|role| {
            matches!(
                role.kind(),
                ReconciliationSlotRoleKind::RetainEmpty
                    | ReconciliationSlotRoleKind::ActivateExisting
            )
            .then_some(role.next())
            .flatten()
            .map(|next| (role.claim().clone(), next))
        }));
        let mount_new_entities = self
            .slot_roles
            .iter()
            .filter(|role| role.kind() == ReconciliationSlotRoleKind::MountNew)
            .map(SealedReconciliationSlotRole::entity)
            .collect::<Vec<_>>();
        transitions.extend(
            self.entering
                .lifecycle_transitions(EntityLifecycle::Active, false)
                .into_iter()
                .filter(|(claim, _)| !mount_new_entities.contains(&claim.entity())),
        );
        transitions.extend(self.slot_roles.iter().filter_map(|role| {
            (role.kind() == ReconciliationSlotRoleKind::MountNew)
                .then_some(role.next())
                .flatten()
                .map(|next| (role.claim().clone(), next))
        }));
        transitions
    }

    pub(crate) fn presentation_mutation_targets(&self) -> (Vec<FrameMutationTarget>, usize) {
        let mut targets = Vec::new();
        for frame_index in (0..self.leaving.frames.len()).rev() {
            targets.extend(
                self.leaving
                    .child_slots
                    .iter()
                    .filter(|slot| slot.parent() == self.leaving.frames[frame_index].frame())
                    .map(|slot| {
                        child_slot_mutation_target(
                            slot,
                            slot.slot_claim().lifecycle(),
                            EntityLifecycle::Leaving,
                            false,
                        )
                    }),
            );
            targets.extend(self.leaving.frames[frame_index].mutation_targets(
                EntityLifecycle::Active,
                EntityLifecycle::Leaving,
                false,
            ));
        }
        targets.extend(self.slot_roles.iter().filter_map(|role| {
            if role.kind() != ReconciliationSlotRoleKind::RetainEmpty {
                return None;
            }
            let next = role.next()?;
            Some(child_slot_mutation_target(
                self.preserved
                    .child_slots
                    .iter()
                    .find(|slot| slot.slot() == role.entity())
                    .expect("sealed retained-empty role retains its exact slot owner"),
                role.current(),
                next,
                false,
            ))
        }));
        let isolate_count = targets.len();
        targets.extend(self.slot_roles.iter().filter_map(|role| {
            if role.kind() != ReconciliationSlotRoleKind::ActivateExisting {
                return None;
            }
            let next = role.next()?;
            Some(child_slot_mutation_target(
                self.preserved
                    .child_slots
                    .iter()
                    .find(|slot| slot.slot() == role.entity())
                    .expect("sealed activate-existing role retains its exact slot owner"),
                role.current(),
                next,
                true,
            ))
        }));
        let mount_new_entities = self
            .slot_roles
            .iter()
            .filter(|role| role.kind() == ReconciliationSlotRoleKind::MountNew)
            .map(SealedReconciliationSlotRole::entity)
            .collect::<Vec<_>>();
        targets.extend(self.entering.entering_mutation_targets_excluding_slots(
            EntityLifecycle::PreparedInert,
            EntityLifecycle::Active,
            &mount_new_entities,
        ));
        targets.extend(self.slot_roles.iter().filter_map(|role| {
            if role.kind() != ReconciliationSlotRoleKind::MountNew {
                return None;
            }
            let slot = self
                .entering
                .child_slots
                .iter()
                .find(|slot| role.matches_mount_new_slot(slot))?;
            Some(child_slot_mutation_target(
                slot,
                role.current(),
                role.next()?,
                true,
            ))
        }));
        (targets, isolate_count)
    }

    pub(crate) fn current_interactive_entities(&self) -> Vec<bevy_ecs::prelude::Entity> {
        self.preserved
            .interactive_entities()
            .into_iter()
            .chain(self.leaving.interactive_entities())
            .collect()
    }

    pub(crate) fn resulting_interactive_entities(&self) -> Vec<bevy_ecs::prelude::Entity> {
        self.preserved
            .interactive_entities()
            .into_iter()
            .chain(self.entering.interactive_entities())
            .collect()
    }

    pub(crate) fn prepare_root_rebase(&self) -> Option<PreparedRootRebase> {
        self.entering.prepare_root_rebase(&self.leaving)
    }

    pub(crate) fn apply_root_rebase(&mut self, prepared: PreparedRootRebase) {
        self.entering.apply_root_rebase(prepared);
    }

    pub(crate) fn prepare_claim_rebind(
        &self,
        claims: &[ExactRegistryClaim],
    ) -> Option<PreparedReconciliationClaimRebind> {
        let transitions = self.lifecycle_transitions();
        if claims.len() != transitions.len()
            || claims.iter().enumerate().any(|(index, claim)| {
                claims[index + 1..]
                    .iter()
                    .any(|other| other.entity() == claim.entity())
            })
            || transitions.iter().any(|(current, next)| {
                claims
                    .iter()
                    .find(|claim| claim.entity() == current.entity())
                    .is_none_or(|claim| {
                        !claim_rebind_is_exact(current, claim, current.lifecycle(), *next)
                    })
            })
        {
            return None;
        }
        let aliases_are_exact = self
            .preserved
            .frames
            .iter()
            .chain(&self.entering.frames)
            .chain(&self.leaving.frames)
            .all(|frame| frame_aliases_match_transitions(frame, &transitions))
            && self
                .preserved
                .child_slots
                .iter()
                .chain(&self.entering.child_slots)
                .chain(&self.leaving.child_slots)
                .all(|slot| slot_aliases_match_transitions(slot, &transitions));
        if !aliases_are_exact {
            return None;
        }
        Some(PreparedReconciliationClaimRebind {
            preserved_frames: prepared_frame_claims(&self.preserved, claims),
            entering_frames: prepared_frame_claims(&self.entering, claims),
            leaving_frames: prepared_frame_claims(&self.leaving, claims),
            all_claims: claims.to_vec(),
        })
    }

    pub(crate) fn apply_claim_rebind(&mut self, prepared: PreparedReconciliationClaimRebind) {
        let PreparedReconciliationClaimRebind {
            preserved_frames,
            entering_frames,
            leaving_frames,
            all_claims,
        } = prepared;
        apply_reconciliation_claim_rebind(&mut self.preserved, preserved_frames, &all_claims);
        apply_reconciliation_claim_rebind(&mut self.entering, entering_frames, &all_claims);
        apply_reconciliation_claim_rebind(&mut self.leaving, leaving_frames, &all_claims);
    }

    pub(crate) fn apply(self) -> ReconciliationSpliceOutcome {
        ReconciliationSpliceOutcome {
            preserved: self.preserved,
            entering: self.entering,
            leaving: self.leaving,
            slot_roles: self.slot_roles,
        }
    }

    pub(crate) fn reject(self) -> RejectedReconciliationOwners {
        RejectedReconciliationOwners {
            active: restore_active_branch(self.preserved, self.leaving),
            entering: PreparedBranch::from_realized(self.entering),
            slot_roles: self.slot_roles,
        }
    }
}

fn prepared_frame_claims(
    branch: &RealizedBranch,
    claims: &[ExactRegistryClaim],
) -> Vec<Vec<ExactRegistryClaim>> {
    branch
        .frames
        .iter()
        .map(|frame| {
            frame
                .claims()
                .map(|current| {
                    claims
                        .iter()
                        .find(|claim| claim.entity() == current.entity())
                        .unwrap_or(current)
                        .clone()
                })
                .collect()
        })
        .collect()
}

fn apply_reconciliation_claim_rebind(
    branch: &mut RealizedBranch,
    frame_claims: Vec<Vec<ExactRegistryClaim>>,
    all_claims: &[ExactRegistryClaim],
) {
    for (frame, primary) in branch.frames.iter_mut().zip(frame_claims) {
        frame.apply_claim_rebind(&primary, all_claims);
    }
    for slot in &mut branch.child_slots {
        slot.apply_internal_claim_rebind(all_claims);
    }
}

fn alias_matches_transitions(
    alias: &ExactRegistryClaim,
    transitions: &[(ExactRegistryClaim, EntityLifecycle)],
) -> bool {
    transitions
        .iter()
        .find(|(current, _)| current.entity() == alias.entity())
        .is_none_or(|(current, _)| current == alias)
}

fn frame_aliases_match_transitions(
    frame: &RealizedFrame,
    transitions: &[(ExactRegistryClaim, EntityLifecycle)],
) -> bool {
    alias_matches_transitions(frame.mount_binding().slot_claim(), transitions)
        && frame
            .mount_binding()
            .physical_parent()
            .1
            .is_none_or(|claim| alias_matches_transitions(claim, transitions))
        && frame
            .mount_binding()
            .staging_existing_shells()
            .iter()
            .all(|claim| alias_matches_transitions(claim, transitions))
        && frame
            .outlets()
            .iter()
            .all(|outlet| alias_matches_transitions(outlet.anchor_claim(), transitions))
}

fn slot_aliases_match_transitions(
    slot: &MountedChildSlot,
    transitions: &[(ExactRegistryClaim, EntityLifecycle)],
) -> bool {
    alias_matches_transitions(slot.slot_claim(), transitions)
        && alias_matches_transitions(slot.anchor_claim(), transitions)
}

impl SealedExistingReconciliation {
    fn prepare(
        branch: RealizedBranch,
        existing: ReconciliationExistingProjection,
    ) -> Result<Self, ReconciliationPreparationFailure> {
        let Some(root) = branch.frames.first() else {
            return Err(ReconciliationPreparationFailure {
                branch,
                error: ReconciliationSpliceError::InvalidAuthorityIdentity,
            });
        };
        let authority = ReconciliationAuthorityIdentity {
            router_anchor: root.frame().clone(),
            host: root.frame().host(),
            top_slot: root.frame().slot(),
        };
        Self::prepare_with_entering(
            branch,
            authority,
            existing,
            ReconciliationEnteringProjection {
                frames: Vec::new(),
                mount_new_slots: Vec::new(),
            },
        )
    }

    pub(crate) fn prepare_from_projection(
        branch: RealizedBranch,
        projection: ReconciliationProjectionFacts,
    ) -> Result<Self, ReconciliationPreparationFailure> {
        Self::prepare_with_entering(
            branch,
            projection.authority,
            projection.existing,
            projection.entering,
        )
    }

    fn prepare_with_entering(
        mut branch: RealizedBranch,
        authority: ReconciliationAuthorityIdentity,
        existing: ReconciliationExistingProjection,
        entering: ReconciliationEnteringProjection,
    ) -> Result<Self, ReconciliationPreparationFailure> {
        let preparation = validate_existing_reconciliation(&branch, &authority, &existing);
        let (preserved_frames, slot_role_specs) = match preparation {
            Ok(preparation) => preparation,
            Err(error) => {
                return Err(ReconciliationPreparationFailure { branch, error });
            }
        };

        let mut preserved = RealizedBranch::empty();
        let mut leaving = RealizedBranch::empty();
        for (index, frame) in branch.frames.drain(..).enumerate() {
            if preserved_frames[index] {
                preserved.frames.push(frame);
            } else {
                leaving.frames.push(frame);
            }
        }
        let mut slot_roles = Vec::with_capacity(branch.child_slots.len());
        for (slot, (kind, entering_use)) in branch.child_slots.drain(..).zip(slot_role_specs) {
            slot_roles.push(sealed_slot_role(&slot, kind, entering_use));
            if kind == ReconciliationSlotRoleKind::Leaving {
                leaving.child_slots.push(slot);
            } else {
                preserved.child_slots.push(slot);
            }
        }
        sort_child_slots(&mut preserved);
        sort_child_slots(&mut leaving);
        Ok(Self {
            authority,
            preserved,
            leaving,
            slot_roles,
            entering,
        })
    }

    pub(crate) const fn preserved(&self) -> &RealizedBranch {
        &self.preserved
    }

    pub(crate) const fn leaving(&self) -> &RealizedBranch {
        &self.leaving
    }

    pub(crate) fn slot_roles(&self) -> &[SealedReconciliationSlotRole] {
        &self.slot_roles
    }

    pub(crate) fn bind_entering(
        mut self,
        entering: PreparedBranch,
    ) -> Result<SealedReconciliationSplice, ReconciliationEnteringFailure> {
        let mount_new_roles = match validate_entering_reconciliation(&self, entering.as_realized())
        {
            Ok(roles) => roles,
            Err(error) => {
                return Err(ReconciliationEnteringFailure {
                    sealed: self,
                    entering,
                    error,
                });
            }
        };
        if let Err(error) = validate_reconciliation_closure_authorities(
            &self.preserved,
            entering.as_realized(),
            &self.leaving,
        ) {
            return Err(ReconciliationEnteringFailure {
                sealed: self,
                entering,
                error,
            });
        }
        self.slot_roles.extend(mount_new_roles);
        Ok(SealedReconciliationSplice {
            preserved: self.preserved,
            entering: entering.into_realized(),
            leaving: self.leaving,
            slot_roles: self.slot_roles,
        })
    }

    pub(crate) fn reject(self) -> RejectedReconciliationOwners {
        RejectedReconciliationOwners {
            active: restore_active_branch(self.preserved, self.leaving),
            entering: PreparedBranch::empty(),
            slot_roles: self.slot_roles,
        }
    }
}

fn restore_active_branch(
    mut preserved: RealizedBranch,
    mut leaving: RealizedBranch,
) -> RealizedBranch {
    preserved.frames.append(&mut leaving.frames);
    preserved.child_slots.append(&mut leaving.child_slots);
    sort_child_slots(&mut preserved);
    preserved
}

fn sealed_slot_role(
    slot: &MountedChildSlot,
    kind: ReconciliationSlotRoleKind,
    entering_use: ReconciliationSlotEnteringUse,
) -> SealedReconciliationSlotRole {
    let next = slot_role_next(kind, slot.slot_claim().lifecycle())
        .expect("sealed role lifecycle was validated before ownership moved");
    SealedReconciliationSlotRole {
        claim: slot.slot_claim().clone(),
        parent: slot.parent().clone(),
        outlet: slot.outlet().clone(),
        slot: slot.slot_id(),
        anchor: slot.anchor(),
        anchor_claim: slot.anchor_claim().clone(),
        kind,
        next,
        entering_use,
    }
}

fn slot_role_next(
    kind: ReconciliationSlotRoleKind,
    current: EntityLifecycle,
) -> Option<Option<EntityLifecycle>> {
    match (kind, current) {
        (ReconciliationSlotRoleKind::PreservedActive, EntityLifecycle::Active) => Some(None),
        (ReconciliationSlotRoleKind::RetainEmpty, EntityLifecycle::Active) => {
            Some(Some(EntityLifecycle::PreparedInert))
        }
        (ReconciliationSlotRoleKind::RetainEmpty, EntityLifecycle::PreparedInert) => Some(None),
        (ReconciliationSlotRoleKind::ActivateExisting, EntityLifecycle::PreparedInert)
        | (ReconciliationSlotRoleKind::MountNew, EntityLifecycle::PreparedInert) => {
            Some(Some(EntityLifecycle::Active))
        }
        (
            ReconciliationSlotRoleKind::Leaving,
            EntityLifecycle::Active | EntityLifecycle::PreparedInert,
        ) => Some(Some(EntityLifecycle::Leaving)),
        _ => None,
    }
}

fn existing_slot_role_next(
    kind: ReconciliationSlotRoleKind,
    occupied: bool,
    current: EntityLifecycle,
) -> Option<Option<EntityLifecycle>> {
    match (kind, occupied, current) {
        (ReconciliationSlotRoleKind::PreservedActive, true, EntityLifecycle::Active) => Some(None),
        (ReconciliationSlotRoleKind::RetainEmpty, true, EntityLifecycle::Active) => {
            Some(Some(EntityLifecycle::PreparedInert))
        }
        (
            ReconciliationSlotRoleKind::ActivateExisting | ReconciliationSlotRoleKind::RetainEmpty,
            false,
            EntityLifecycle::PreparedInert,
        ) => match kind {
            ReconciliationSlotRoleKind::ActivateExisting => Some(Some(EntityLifecycle::Active)),
            ReconciliationSlotRoleKind::RetainEmpty => Some(None),
            _ => unreachable!(),
        },
        (ReconciliationSlotRoleKind::Leaving, true, EntityLifecycle::Active)
        | (ReconciliationSlotRoleKind::Leaving, false, EntityLifecycle::PreparedInert) => {
            Some(Some(EntityLifecycle::Leaving))
        }
        _ => None,
    }
}

fn validate_existing_reconciliation(
    branch: &RealizedBranch,
    authority: &ReconciliationAuthorityIdentity,
    projection: &ReconciliationExistingProjection,
) -> Result<
    (
        Vec<bool>,
        Vec<(ReconciliationSlotRoleKind, ReconciliationSlotEnteringUse)>,
    ),
    ReconciliationSpliceError,
> {
    validate_realized_branch(branch)?;
    if branch.frames.first().is_some_and(|root| {
        !authority.matches(root.frame()) || root.frame().slot() != authority.top_slot
    }) || branch
        .frames
        .iter()
        .any(|frame| !authority.matches(frame.frame()))
    {
        return Err(ReconciliationSpliceError::InvalidAuthorityIdentity);
    }
    if branch.frames.iter().any(|frame| {
        frame
            .claims()
            .any(|claim| claim.lifecycle() != EntityLifecycle::Active)
    }) {
        return Err(ReconciliationSpliceError::InvalidFramePartition);
    }
    let frame_count = branch.frames.len();
    if projection.preserved.len() + projection.leaving.len() != frame_count {
        return Err(ReconciliationSpliceError::InvalidFramePartition);
    }
    for (index, projected) in projection.preserved.iter().enumerate() {
        if projected.branch_index != index || branch.frames[index].frame() != &projected.version {
            return Err(ReconciliationSpliceError::InvalidFramePartition);
        }
    }
    let preserved_count = projection.preserved.len();
    for (offset, projected) in projection.leaving.iter().enumerate() {
        let index = frame_count
            .checked_sub(offset + 1)
            .ok_or(ReconciliationSpliceError::InvalidFramePartition)?;
        if index < preserved_count
            || projected.branch_index != index
            || branch.frames[index].frame() != &projected.version
        {
            return Err(ReconciliationSpliceError::InvalidFramePartition);
        }
    }

    if projection.child_slots.len() != branch.child_slots.len() {
        return Err(ReconciliationSpliceError::InvalidSlotPartition);
    }
    let mut slot_kinds = Vec::with_capacity(branch.child_slots.len());
    let mut consumed_projection = vec![false; projection.child_slots.len()];
    for slot in &branch.child_slots {
        let matches = projection
            .child_slots
            .iter()
            .enumerate()
            .filter(|(_, projected)| {
                projected.slot == slot.slot_id()
                    && projected.parent == *slot.parent()
                    && projected.outlet == *slot.outlet()
            })
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            return Err(ReconciliationSpliceError::InvalidSlotPartition);
        }
        let (projection_index, projected) = matches[0];
        if consumed_projection[projection_index]
            || projected.parent_branch_index >= frame_count
            || branch.frames[projected.parent_branch_index].frame() != &projected.parent
        {
            return Err(ReconciliationSpliceError::InvalidSlotPartition);
        }
        consumed_projection[projection_index] = true;
        let child = child_for_slot(branch, slot).map(|frame| frame.frame());
        if child != projected.child.as_ref() {
            return Err(ReconciliationSpliceError::InvalidSlotPartition);
        }
        let parent_is_preserved = projected.parent_branch_index < preserved_count;
        let child_is_preserved = child.is_some_and(|child| {
            projection
                .preserved
                .iter()
                .any(|frame| &frame.version == child)
        });
        let child_is_leaving = child.is_some_and(|child| {
            projection
                .leaving
                .iter()
                .any(|frame| &frame.version == child)
        });
        let role_is_valid = match projected.role {
            ReconciliationSlotRoleKind::PreservedActive => {
                parent_is_preserved && child.is_some() && (child_is_preserved || child_is_leaving)
            }
            ReconciliationSlotRoleKind::RetainEmpty => parent_is_preserved && !child_is_preserved,
            ReconciliationSlotRoleKind::ActivateExisting => parent_is_preserved && child.is_none(),
            ReconciliationSlotRoleKind::Leaving => {
                !parent_is_preserved && child.is_none_or(|_| child_is_leaving)
            }
            ReconciliationSlotRoleKind::MountNew => false,
        };
        if !role_is_valid
            || existing_slot_role_next(
                projected.role,
                child.is_some(),
                slot.slot_claim().lifecycle(),
            )
            .is_none()
        {
            return Err(ReconciliationSpliceError::InvalidSlotPartition);
        }
        let entering_use = match projected.role {
            ReconciliationSlotRoleKind::ActivateExisting => ReconciliationSlotEnteringUse::Required,
            ReconciliationSlotRoleKind::PreservedActive if child_is_leaving => {
                ReconciliationSlotEnteringUse::Required
            }
            ReconciliationSlotRoleKind::PreservedActive
            | ReconciliationSlotRoleKind::RetainEmpty
            | ReconciliationSlotRoleKind::Leaving => ReconciliationSlotEnteringUse::Forbidden,
            ReconciliationSlotRoleKind::MountNew => {
                return Err(ReconciliationSpliceError::InvalidSlotPartition);
            }
        };
        slot_kinds.push((projected.role, entering_use));
    }
    if consumed_projection.iter().any(|consumed| !consumed) {
        return Err(ReconciliationSpliceError::InvalidSlotPartition);
    }
    Ok((
        (0..frame_count)
            .map(|index| index < preserved_count)
            .collect(),
        slot_kinds,
    ))
}

fn validate_entering_reconciliation(
    sealed: &SealedExistingReconciliation,
    entering: &RealizedBranch,
) -> Result<Vec<SealedReconciliationSlotRole>, ReconciliationSpliceError> {
    if entering.frames.iter().any(|frame| {
        frame
            .claims()
            .any(|claim| claim.lifecycle() != EntityLifecycle::PreparedInert)
    }) || entering
        .child_slots
        .iter()
        .any(|slot| slot.slot_claim().lifecycle() != EntityLifecycle::PreparedInert)
    {
        return Err(ReconciliationSpliceError::InvalidEnteringBranch);
    }
    if entering.frames.len() != sealed.entering.frames.len() {
        return Err(ReconciliationSpliceError::InvalidEnteringBranch);
    }
    let preserved_count = sealed.preserved.frames.len();
    let mut final_frames = sealed
        .preserved
        .frames
        .iter()
        .map(|frame| frame.frame())
        .collect::<Vec<_>>();
    for (offset, (frame, projected)) in entering
        .frames
        .iter()
        .zip(&sealed.entering.frames)
        .enumerate()
    {
        if projected.branch_index != preserved_count + offset
            || !sealed.authority.matches(frame.frame())
            || frame.frame().frame() != projected.frame
            || frame.frame().generation() != projected.generation
            || !matches!(
                frame.frame().origin(),
                FrameOrigin::MatchedRoute(route) if *route == projected.route
            )
        {
            return Err(ReconciliationSpliceError::InvalidEnteringBranch);
        }
        match projected.slot {
            ReconciliationEnteringSlot::Existing(slot)
                if frame.frame().slot() == slot
                    && (projected.branch_index != 0 || slot == sealed.authority.top_slot) => {}
            ReconciliationEnteringSlot::MountNew if projected.branch_index != 0 => {}
            ReconciliationEnteringSlot::Existing(_) => {
                return Err(ReconciliationSpliceError::InvalidEnteringBranch);
            }
            ReconciliationEnteringSlot::MountNew => {
                return Err(ReconciliationSpliceError::InvalidEnteringBranch);
            }
        }
        final_frames.push(frame.frame());
    }

    let existing_slots = sealed.preserved.child_slots.iter().collect::<Vec<_>>();
    let mut used_existing = Vec::new();
    let mut used_mount_new = Vec::new();
    let mut used_entering_slots = Vec::new();
    let mut mount_new_roles = Vec::new();
    for (frame, projected) in entering.frames.iter().zip(&sealed.entering.frames) {
        if projected.branch_index == 0 {
            continue;
        }
        let parent_index = projected.branch_index - 1;
        let Some(parent) = final_frames.get(parent_index) else {
            return Err(ReconciliationSpliceError::InvalidEnteringBranch);
        };
        match projected.slot {
            ReconciliationEnteringSlot::Existing(slot_id) => {
                let candidates = existing_slots
                    .iter()
                    .enumerate()
                    .filter(|(_, slot)| {
                        slot.slot_id() == slot_id
                            && slot.parent() == *parent
                            && frame.mount_binding().matches_mounted_child_slot(slot)
                    })
                    .collect::<Vec<_>>();
                if candidates.len() != 1 || used_existing.contains(&candidates[0].0) {
                    return Err(ReconciliationSpliceError::InvalidEnteringBranch);
                }
                let role = sealed
                    .slot_roles
                    .iter()
                    .find(|role| role.entity() == candidates[0].1.slot());
                if role.is_none_or(|role| {
                    role.entering_use != ReconciliationSlotEnteringUse::Required
                        || !matches!(
                            role.kind(),
                            ReconciliationSlotRoleKind::PreservedActive
                                | ReconciliationSlotRoleKind::ActivateExisting
                        )
                }) {
                    return Err(ReconciliationSpliceError::InvalidEnteringBranch);
                }
                used_existing.push(candidates[0].0);
            }
            ReconciliationEnteringSlot::MountNew => {
                let candidates = entering
                    .child_slots
                    .iter()
                    .enumerate()
                    .filter(|(_, slot)| {
                        slot.parent() == *parent
                            && slot.slot_id() == frame.frame().slot()
                            && frame.mount_binding().matches_mounted_child_slot(slot)
                    })
                    .collect::<Vec<_>>();
                if candidates.len() != 1 || used_entering_slots.contains(&candidates[0].0) {
                    return Err(ReconciliationSpliceError::InvalidEnteringBranch);
                }
                let specs = sealed
                    .entering
                    .mount_new_slots
                    .iter()
                    .enumerate()
                    .filter(|(_, spec)| {
                        spec.parent_branch_index == parent_index
                            && spec.outlet == *candidates[0].1.outlet()
                    })
                    .collect::<Vec<_>>();
                if specs.len() != 1 || used_mount_new.contains(&specs[0].0) {
                    return Err(ReconciliationSpliceError::InvalidEnteringBranch);
                }
                if slot_role_next(
                    ReconciliationSlotRoleKind::MountNew,
                    candidates[0].1.slot_claim().lifecycle(),
                )
                .is_none()
                {
                    return Err(ReconciliationSpliceError::InvalidEnteringBranch);
                }
                used_entering_slots.push(candidates[0].0);
                used_mount_new.push(specs[0].0);
                mount_new_roles.push(sealed_slot_role(
                    candidates[0].1,
                    ReconciliationSlotRoleKind::MountNew,
                    ReconciliationSlotEnteringUse::Required,
                ));
            }
        }
    }
    if used_entering_slots.len() != entering.child_slots.len()
        || used_mount_new.len() != sealed.entering.mount_new_slots.len()
    {
        return Err(ReconciliationSpliceError::InvalidEnteringBranch);
    }

    if sealed.slot_roles.iter().any(|role| {
        let used = used_existing
            .iter()
            .any(|index| existing_slots[*index].slot() == role.entity());
        match role.entering_use {
            ReconciliationSlotEnteringUse::Required => !used,
            ReconciliationSlotEnteringUse::Forbidden => used,
        }
    }) {
        return Err(ReconciliationSpliceError::InvalidEnteringBranch);
    }

    let claims = sealed
        .preserved
        .frames
        .iter()
        .chain(entering.frames.iter())
        .flat_map(RealizedFrame::claims)
        .chain(
            sealed
                .preserved
                .child_slots
                .iter()
                .chain(entering.child_slots.iter())
                .map(MountedChildSlot::slot_claim),
        )
        .collect::<Vec<_>>();
    validate_unique_authorities(&claims)?;
    Ok(mount_new_roles)
}

fn validate_reconciliation_closure_authorities(
    preserved: &RealizedBranch,
    entering: &RealizedBranch,
    leaving: &RealizedBranch,
) -> Result<(), ReconciliationSpliceError> {
    let claims = preserved
        .frames
        .iter()
        .chain(entering.frames.iter())
        .chain(leaving.frames.iter())
        .flat_map(RealizedFrame::claims)
        .chain(
            preserved
                .child_slots
                .iter()
                .chain(entering.child_slots.iter())
                .chain(leaving.child_slots.iter())
                .map(MountedChildSlot::slot_claim),
        )
        .collect::<Vec<_>>();
    validate_unique_authorities(&claims)
}

fn validate_realized_branch(branch: &RealizedBranch) -> Result<(), ReconciliationSpliceError> {
    let claims = branch
        .frames
        .iter()
        .flat_map(RealizedFrame::claims)
        .chain(branch.child_slots.iter().map(MountedChildSlot::slot_claim))
        .collect::<Vec<_>>();
    validate_unique_authorities(&claims)?;

    if branch.frames.iter().enumerate().any(|(index, frame)| {
        branch.frames[index + 1..].iter().any(|other| {
            other.frame() == frame.frame() || other.frame().frame() == frame.frame().frame()
        })
    }) {
        return Err(ReconciliationSpliceError::DuplicateAuthority);
    }
    for (index, slot) in branch.child_slots.iter().enumerate() {
        if branch.child_slots[index + 1..].iter().any(|other| {
            other.slot_id() == slot.slot_id()
                || (other.parent() == slot.parent() && other.outlet() == slot.outlet())
        }) {
            return Err(ReconciliationSpliceError::DuplicateAuthority);
        }
        if slot.slot_claim().owner()
            != &RegistryOwner::Committed(EntityOwner::ChildSlot {
                parent: slot.parent().clone(),
                outlet: slot.outlet().clone(),
                slot: slot.slot_id(),
            })
        {
            return Err(ReconciliationSpliceError::InvalidSlotPartition);
        }
        let Some(parent) = branch
            .frames
            .iter()
            .find(|frame| frame.frame() == slot.parent())
        else {
            return Err(ReconciliationSpliceError::InvalidSlotPartition);
        };
        let parent_outlet_is_exact = parent.outlet(slot.outlet()).is_some_and(|outlet| {
            outlet.anchor() == slot.anchor()
                && outlet.anchor_claim() == slot.anchor_claim()
                && parent.claims().any(|claim| claim == slot.anchor_claim())
        });
        if !parent_outlet_is_exact {
            return Err(ReconciliationSpliceError::InvalidSlotPartition);
        }
        if branch
            .frames
            .iter()
            .filter(|frame| frame.mount_binding().matches_mounted_child_slot(slot))
            .count()
            > 1
        {
            return Err(ReconciliationSpliceError::InvalidSlotPartition);
        }
    }
    for frame in branch.frames.iter().skip(1) {
        if branch
            .child_slots
            .iter()
            .filter(|slot| frame.mount_binding().matches_mounted_child_slot(slot))
            .count()
            != 1
        {
            return Err(ReconciliationSpliceError::InvalidSlotPartition);
        }
    }
    Ok(())
}

fn validate_unique_authorities(
    claims: &[&ExactRegistryClaim],
) -> Result<(), ReconciliationSpliceError> {
    for (index, claim) in claims.iter().enumerate() {
        if claims[index + 1..].iter().any(|other| {
            other.entity() == claim.entity()
                || other.owner().conflicts_with(claim.owner())
                || other.seal() == claim.seal()
        }) {
            return Err(ReconciliationSpliceError::DuplicateAuthority);
        }
    }
    Ok(())
}

fn child_for_slot<'a>(
    branch: &'a RealizedBranch,
    slot: &MountedChildSlot,
) -> Option<&'a RealizedFrame> {
    branch
        .frames
        .iter()
        .find(|frame| frame.mount_binding().matches_mounted_child_slot(slot))
}

fn sort_child_slots(branch: &mut RealizedBranch) {
    let frames = &branch.frames;
    branch.child_slots.sort_by(|left, right| {
        let left_index = frames
            .iter()
            .position(|frame| frame.frame() == left.parent())
            .unwrap_or(usize::MAX);
        let right_index = frames
            .iter()
            .position(|frame| frame.frame() == right.parent())
            .unwrap_or(usize::MAX);
        left_index
            .cmp(&right_index)
            .then_with(|| left.outlet().cmp(right.outlet()))
    });
}

pub(crate) struct PreparedBranchClaimRebind {
    frame_claims: Vec<Vec<ExactRegistryClaim>>,
    all_claims: Vec<ExactRegistryClaim>,
    expected_current: Vec<(bevy_ecs::prelude::Entity, EntityLifecycle)>,
    expected_next: EntityLifecycle,
}

pub(crate) struct PreparedRootRebase {
    frame: PreparedFrameMountRebase,
}

#[derive(Debug)]
pub(crate) struct RejectedChildSlot {
    slot: MountedChildSlot,
    canonical_anchor: Option<bevy_ecs::prelude::Entity>,
    canonical_anchor_claim: Option<crate::registry::ExactRegistryClaim>,
    canonical_anchor_children: Option<Vec<bevy_ecs::prelude::Entity>>,
}

impl RejectedChildSlot {
    pub(crate) const fn slot(&self) -> &MountedChildSlot {
        &self.slot
    }

    pub(crate) const fn canonical_anchor(&self) -> Option<bevy_ecs::prelude::Entity> {
        self.canonical_anchor
    }

    pub(crate) const fn canonical_anchor_claim(
        &self,
    ) -> Option<&crate::registry::ExactRegistryClaim> {
        self.canonical_anchor_claim.as_ref()
    }

    pub(crate) fn canonical_anchor_children(&self) -> Option<&[bevy_ecs::prelude::Entity]> {
        self.canonical_anchor_children.as_deref()
    }
}

impl RealizedBranch {
    pub(crate) const fn empty() -> Self {
        Self {
            frames: Vec::new(),
            child_slots: Vec::new(),
        }
    }

    pub(crate) fn frames(&self) -> &[RealizedFrame] {
        &self.frames
    }

    #[cfg(test)]
    pub(crate) fn frames_mut_for_test(&mut self) -> &mut [RealizedFrame] {
        &mut self.frames
    }

    #[cfg(test)]
    pub(crate) fn retain_last_frame_for_test(&mut self) {
        let last = self.frames.pop();
        self.frames.clear();
        self.frames.extend(last);
        self.child_slots.clear();
    }

    pub(crate) fn child_slots(&self) -> &[MountedChildSlot] {
        &self.child_slots
    }

    #[cfg(test)]
    pub(crate) fn pop_frame_for_test(&mut self) -> Option<RealizedFrame> {
        self.frames.pop()
    }

    #[cfg(test)]
    pub(crate) fn reverse_frames_for_test(&mut self) {
        self.frames.reverse();
    }

    #[cfg(test)]
    pub(crate) fn reverse_child_slots_for_test(&mut self) {
        self.child_slots.reverse();
    }

    #[cfg(test)]
    pub(crate) fn clear_child_slots_for_test(&mut self) {
        self.child_slots.clear();
    }

    #[cfg(test)]
    pub(crate) fn push_child_slot_unchecked_for_test(&mut self, slot: MountedChildSlot) {
        self.child_slots.push(slot);
    }

    #[cfg(test)]
    pub(crate) fn replace_frame_mount_from_for_test(&mut self, target: usize, source: usize) {
        let mount = self.frames[source].mount_binding().clone();
        self.frames[target].replace_mount_binding_for_test(mount);
    }

    #[cfg(test)]
    pub(crate) fn replace_frame_claim_for_test(
        &mut self,
        frame: usize,
        entity: bevy_ecs::prelude::Entity,
        claim: ExactRegistryClaim,
    ) {
        self.frames[frame].replace_primary_claim_for_test(entity, claim);
    }

    #[cfg(test)]
    pub(crate) fn retain_cycle_for_test(&mut self, frame: usize, slot: usize, mount_source: usize) {
        let mount = self.frames[mount_source].mount_binding().clone();
        let mut frame = self.frames.remove(frame);
        let slot = self.child_slots.remove(slot);
        frame.replace_mount_binding_for_test(mount);
        self.frames.clear();
        self.frames.push(frame);
        self.child_slots.clear();
        self.child_slots.push(slot);
    }

    #[cfg(feature = "test-support")]
    pub(crate) fn tracked_child_slots(&self) -> Vec<TrackedChildSlot> {
        self.child_slots
            .iter()
            .map(TrackedChildSlot::from_mounted)
            .collect()
    }

    pub(crate) fn push_frame(&mut self, frame: RealizedFrame) -> Result<(), RealizedFrame> {
        if self.frames.iter().any(|current| {
            current.frame() == frame.frame()
                || current.frame().frame() == frame.frame().frame()
                || current
                    .claims()
                    .any(|claim| frame.claims().any(|other| other.entity() == claim.entity()))
        }) {
            return Err(frame);
        }
        let matching_slots = self
            .child_slots
            .iter()
            .filter(|slot| frame.mount_binding().matches_mounted_child_slot(slot))
            .collect::<Vec<_>>();
        if !self.frames.is_empty()
            && (matching_slots.len() != 1
                || !self
                    .frames
                    .iter()
                    .any(|parent| parent.frame() == matching_slots[0].parent()))
        {
            return Err(frame);
        }
        self.frames.push(frame);
        Ok(())
    }

    pub(crate) fn push_child_slot(
        &mut self,
        slot: MountedChildSlot,
    ) -> Result<(), RejectedChildSlot> {
        let canonical = self
            .frames
            .iter()
            .find(|parent| parent.frame() == slot.parent())
            .and_then(|parent| {
                parent.outlet(slot.outlet()).map(|outlet| {
                    (
                        outlet.anchor(),
                        outlet.anchor_claim().clone(),
                        parent.expected_owned_children(outlet.anchor()),
                    )
                })
            });
        if self.child_slots.iter().any(|current| {
            current.slot() == slot.slot()
                || current.slot_id() == slot.slot_id()
                || (current.parent() == slot.parent() && current.outlet() == slot.outlet())
                || current
                    .slot_claim()
                    .owner()
                    .conflicts_with(slot.slot_claim().owner())
        }) {
            return Err(RejectedChildSlot {
                slot,
                canonical_anchor: canonical.as_ref().map(|(anchor, _, _)| *anchor),
                canonical_anchor_claim: canonical.as_ref().map(|(_, claim, _)| claim.clone()),
                canonical_anchor_children: canonical.map(|(_, _, children)| children),
            });
        }
        let Some(parent) = self
            .frames
            .iter()
            .find(|parent| parent.frame() == slot.parent())
        else {
            return Err(RejectedChildSlot {
                slot,
                canonical_anchor: None,
                canonical_anchor_claim: None,
                canonical_anchor_children: None,
            });
        };
        let parent_outlet_is_exact = parent.outlet(slot.outlet()).is_some_and(|outlet| {
            outlet.anchor() == slot.anchor() && outlet.anchor_claim() == slot.anchor_claim()
        });
        let slot_owner_is_exact = slot.slot_claim().owner()
            == &RegistryOwner::Committed(EntityOwner::ChildSlot {
                parent: slot.parent().clone(),
                outlet: slot.outlet().clone(),
                slot: slot.slot_id(),
            });
        if !parent_outlet_is_exact || !slot_owner_is_exact {
            return Err(RejectedChildSlot {
                slot,
                canonical_anchor: canonical.as_ref().map(|(anchor, _, _)| *anchor),
                canonical_anchor_claim: canonical.as_ref().map(|(_, claim, _)| claim.clone()),
                canonical_anchor_children: canonical.map(|(_, _, children)| children),
            });
        }
        self.child_slots.push(slot);
        Ok(())
    }

    fn push_reconciliation_child_slot(
        &mut self,
        slot: MountedChildSlot,
        preserved_parent: &RealizedFrame,
    ) -> Result<(), RejectedChildSlot> {
        let canonical: Option<(
            bevy_ecs::prelude::Entity,
            ExactRegistryClaim,
            Vec<bevy_ecs::prelude::Entity>,
        )> = preserved_parent.outlet(slot.outlet()).map(|outlet| {
            (
                outlet.anchor(),
                outlet.anchor_claim().clone(),
                preserved_parent.expected_owned_children(outlet.anchor()),
            )
        });
        let reject = |slot,
                      canonical: &Option<(
            bevy_ecs::prelude::Entity,
            ExactRegistryClaim,
            Vec<bevy_ecs::prelude::Entity>,
        )>| RejectedChildSlot {
            slot,
            canonical_anchor: canonical.as_ref().map(|(anchor, _, _)| *anchor),
            canonical_anchor_claim: canonical.as_ref().map(|(_, claim, _)| claim.clone()),
            canonical_anchor_children: canonical.as_ref().map(|(_, _, children)| children.clone()),
        };
        if self.child_slots.iter().any(|current| {
            current.slot() == slot.slot()
                || current.slot_id() == slot.slot_id()
                || (current.parent() == slot.parent() && current.outlet() == slot.outlet())
                || current
                    .slot_claim()
                    .owner()
                    .conflicts_with(slot.slot_claim().owner())
        }) {
            return Err(reject(slot, &canonical));
        }
        let parent_outlet_is_exact = preserved_parent
            .outlet(slot.outlet())
            .is_some_and(|outlet| {
                outlet.anchor() == slot.anchor()
                    && outlet.anchor_claim() == slot.anchor_claim()
                    && preserved_parent
                        .claims()
                        .any(|claim| claim == slot.anchor_claim())
            });
        let slot_owner_is_exact = slot.slot_claim().owner()
            == &RegistryOwner::Committed(EntityOwner::ChildSlot {
                parent: slot.parent().clone(),
                outlet: slot.outlet().clone(),
                slot: slot.slot_id(),
            });
        if preserved_parent.frame() != slot.parent()
            || !parent_outlet_is_exact
            || !slot_owner_is_exact
        {
            return Err(reject(slot, &canonical));
        }
        self.child_slots.push(slot);
        Ok(())
    }

    pub(crate) fn last_frame(&self) -> Option<&RealizedFrame> {
        self.frames.last()
    }

    pub(crate) fn first_frame(&self) -> Option<&RealizedFrame> {
        self.frames.first()
    }

    pub(crate) fn prepare_root_rebase(&self, leaving: &Self) -> Option<PreparedRootRebase> {
        let entering_root = self.frames.first()?;
        let leaving_root = leaving.frames.first()?;
        Some(PreparedRootRebase {
            frame: entering_root.prepare_mount_rebase(leaving_root)?,
        })
    }

    pub(crate) fn apply_root_rebase(&mut self, prepared: PreparedRootRebase) {
        self.frames[0].apply_mount_rebase(prepared.frame);
    }

    pub(crate) fn interactive_entities(&self) -> Vec<bevy_ecs::prelude::Entity> {
        self.frames
            .iter()
            .flat_map(RealizedFrame::interactive_entities)
            .collect()
    }

    pub(crate) fn claims(&self) -> impl Iterator<Item = &crate::registry::ExactRegistryClaim> {
        self.frames
            .iter()
            .flat_map(RealizedFrame::claims)
            .chain(self.child_slots.iter().map(MountedChildSlot::slot_claim))
    }

    pub(crate) fn obligation_count(&self) -> usize {
        self.frames
            .iter()
            .map(RealizedFrame::obligation_count)
            .sum::<usize>()
            + self.child_slots.len()
    }

    pub(crate) fn active_sparse_leaving_mutation_targets(
        &self,
        next: EntityLifecycle,
    ) -> Option<Vec<FrameMutationTarget>> {
        let legal_slot = |lifecycle| {
            matches!(
                lifecycle,
                EntityLifecycle::Active | EntityLifecycle::PreparedInert
            )
        };
        let mut targets = Vec::new();
        for frame_index in (0..self.frames.len()).rev() {
            for slot in self
                .child_slots
                .iter()
                .filter(|slot| slot.parent() == self.frames[frame_index].frame())
            {
                let current = slot.slot_claim().lifecycle();
                if !legal_slot(current) {
                    return None;
                }
                targets.push(child_slot_mutation_target(slot, current, next, false));
            }
            let frame = &self.frames[frame_index];
            let current = frame.claims().next()?.lifecycle();
            if current != EntityLifecycle::Active
                || frame.claims().any(|claim| claim.lifecycle() != current)
            {
                return None;
            }
            targets.extend(frame.mutation_targets(current, next, false));
        }
        (targets.len() == self.obligation_count()).then_some(targets)
    }

    pub(crate) fn entering_mutation_targets(
        &self,
        current: EntityLifecycle,
        next: EntityLifecycle,
    ) -> Vec<FrameMutationTarget> {
        self.entering_mutation_targets_excluding_slots(current, next, &[])
    }

    fn entering_mutation_targets_excluding_slots(
        &self,
        current: EntityLifecycle,
        next: EntityLifecycle,
        excluded_slots: &[bevy_ecs::prelude::Entity],
    ) -> Vec<FrameMutationTarget> {
        let mut targets = Vec::new();
        for frame_index in 0..self.frames.len() {
            targets.extend(self.frames[frame_index].mutation_targets(current, next, true));
            targets.extend(
                self.child_slots
                    .iter()
                    .filter(|slot| slot.parent() == self.frames[frame_index].frame())
                    .filter(|slot| !excluded_slots.contains(&slot.slot()))
                    .map(|slot| child_slot_mutation_target(slot, current, next, true)),
            );
        }
        targets
    }

    pub(crate) fn lifecycle_transitions(
        &self,
        next: EntityLifecycle,
        leaf_to_root: bool,
    ) -> Vec<(ExactRegistryClaim, EntityLifecycle)> {
        let mut transitions = Vec::with_capacity(self.obligation_count());
        if leaf_to_root {
            for frame_index in (0..self.frames.len()).rev() {
                transitions.extend(
                    self.child_slots
                        .iter()
                        .filter(|slot| slot.parent() == self.frames[frame_index].frame())
                        .map(|slot| (slot.slot_claim().clone(), next)),
                );
                transitions.extend(self.frames[frame_index].lifecycle_transitions(next));
            }
        } else {
            for frame_index in 0..self.frames.len() {
                transitions.extend(self.frames[frame_index].lifecycle_transitions(next));
                transitions.extend(
                    self.child_slots
                        .iter()
                        .filter(|slot| slot.parent() == self.frames[frame_index].frame())
                        .map(|slot| (slot.slot_claim().clone(), next)),
                );
            }
        }
        transitions
    }

    pub(crate) fn prepare_claim_rebind(
        &self,
        claims: &[ExactRegistryClaim],
        expected_current: EntityLifecycle,
        expected_next: EntityLifecycle,
    ) -> Option<PreparedBranchClaimRebind> {
        if claims.len() != self.obligation_count()
            || claims.iter().enumerate().any(|(index, claim)| {
                claims[index + 1..]
                    .iter()
                    .any(|other| other.entity() == claim.entity())
            })
        {
            return None;
        }
        let frame_claims = self
            .frames
            .iter()
            .map(|frame| {
                frame
                    .claims()
                    .map(|current| {
                        claims
                            .iter()
                            .find(|next| next.entity() == current.entity())
                            .filter(|next| {
                                claim_rebind_is_exact(
                                    current,
                                    next,
                                    expected_current,
                                    expected_next,
                                )
                            })
                            .cloned()
                    })
                    .collect::<Option<Vec<_>>>()
            })
            .collect::<Option<Vec<_>>>();
        let frame_claims = frame_claims?;
        if self.child_slots.iter().any(|slot| {
            claims
                .iter()
                .find(|next| next.entity() == slot.slot_claim().entity())
                .is_none_or(|next| {
                    !claim_rebind_is_exact(slot.slot_claim(), next, expected_current, expected_next)
                })
        }) {
            return None;
        }
        if self.frames.iter().zip(&frame_claims).any(|(frame, next)| {
            !frame.claims_can_rebind(next, claims, expected_current, expected_next)
        }) || self
            .child_slots
            .iter()
            .any(|slot| !slot.internal_claims_can_rebind(claims, expected_current, expected_next))
        {
            return None;
        }
        Some(PreparedBranchClaimRebind {
            frame_claims,
            all_claims: claims.to_vec(),
            expected_current: self
                .claims()
                .map(|claim| (claim.entity(), expected_current))
                .collect(),
            expected_next,
        })
    }

    pub(crate) fn prepare_active_sparse_claim_rebind(
        &self,
        claims: &[ExactRegistryClaim],
        expected_next: EntityLifecycle,
    ) -> Option<PreparedBranchClaimRebind> {
        let legal_slot = |lifecycle| {
            matches!(
                lifecycle,
                EntityLifecycle::Active | EntityLifecycle::PreparedInert
            )
        };
        if claims.len() != self.obligation_count()
            || claims.iter().enumerate().any(|(index, claim)| {
                claims[index + 1..]
                    .iter()
                    .any(|other| other.entity() == claim.entity())
            })
        {
            return None;
        }
        let exact_next = |current: &ExactRegistryClaim, next: &ExactRegistryClaim| {
            current.entity() == next.entity()
                && current.owner() == next.owner()
                && current.seal() == next.seal()
                && next.lifecycle() == expected_next
        };
        let frame_claims = self
            .frames
            .iter()
            .map(|frame| {
                frame
                    .claims()
                    .map(|current| {
                        claims
                            .iter()
                            .find(|next| next.entity() == current.entity())
                            .filter(|next| {
                                current.lifecycle() == EntityLifecycle::Active
                                    && exact_next(current, next)
                            })
                            .cloned()
                    })
                    .collect::<Option<Vec<_>>>()
            })
            .collect::<Option<Vec<_>>>()?;
        for (frame, next) in self.frames.iter().zip(&frame_claims) {
            if frame
                .claims()
                .any(|claim| claim.lifecycle() != EntityLifecycle::Active)
                || !frame.claims_can_rebind(next, claims, EntityLifecycle::Active, expected_next)
            {
                return None;
            }
        }
        for slot in &self.child_slots {
            let next = claims
                .iter()
                .find(|claim| claim.entity() == slot.slot_claim().entity())?;
            if !legal_slot(slot.slot_claim().lifecycle()) || !exact_next(slot.slot_claim(), next) {
                return None;
            }
            let next_anchor = claims
                .iter()
                .find(|claim| claim.entity() == slot.anchor_claim().entity())?;
            if slot.anchor_claim().lifecycle() != EntityLifecycle::Active
                || !exact_next(slot.anchor_claim(), next_anchor)
            {
                return None;
            }
        }
        Some(PreparedBranchClaimRebind {
            frame_claims,
            all_claims: claims.to_vec(),
            expected_current: self
                .claims()
                .map(|claim| (claim.entity(), claim.lifecycle()))
                .collect(),
            expected_next,
        })
    }

    pub(crate) fn apply_claim_rebind(&mut self, prepared: PreparedBranchClaimRebind) {
        let PreparedBranchClaimRebind {
            frame_claims,
            all_claims,
            expected_current,
            expected_next,
        } = prepared;
        let _bound_lifecycle_transition = (expected_current, expected_next);
        for (frame, next) in self.frames.iter_mut().zip(frame_claims) {
            frame.apply_claim_rebind(&next, &all_claims);
        }
        for slot in &mut self.child_slots {
            slot.apply_internal_claim_rebind(&all_claims);
        }
    }

    pub(crate) fn into_single_frame(mut self) -> Result<Option<RealizedFrame>, Self> {
        if !self.child_slots.is_empty() || self.frames.len() > 1 {
            return Err(self);
        }
        Ok(self.frames.pop())
    }

    pub(crate) fn from_single_frame(frame: RealizedFrame) -> Self {
        Self {
            frames: vec![frame],
            child_slots: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub(crate) struct PreparedBranch {
    realized: RealizedBranch,
}

impl PreparedBranch {
    pub(crate) const fn empty() -> Self {
        Self {
            realized: RealizedBranch::empty(),
        }
    }

    pub(crate) fn frames(&self) -> &[RealizedFrame] {
        self.realized.frames()
    }

    pub(crate) fn child_slots(&self) -> &[MountedChildSlot] {
        self.realized.child_slots()
    }

    #[cfg(feature = "test-support")]
    pub(crate) fn tracked_child_slots(&self) -> Vec<TrackedChildSlot> {
        self.realized.tracked_child_slots()
    }

    pub(crate) fn push_frame(&mut self, frame: RealizedFrame) -> Result<(), RealizedFrame> {
        self.realized.push_frame(frame)
    }

    pub(crate) fn push_child_slot(
        &mut self,
        slot: MountedChildSlot,
    ) -> Result<(), RejectedChildSlot> {
        self.realized.push_child_slot(slot)
    }

    pub(crate) fn push_reconciliation_child_slot(
        &mut self,
        slot: MountedChildSlot,
        preserved_parent: &RealizedFrame,
    ) -> Result<(), RejectedChildSlot> {
        self.realized
            .push_reconciliation_child_slot(slot, preserved_parent)
    }

    pub(crate) fn last_frame(&self) -> Option<&RealizedFrame> {
        self.realized.last_frame()
    }

    pub(crate) fn first_frame(&self) -> Option<&RealizedFrame> {
        self.realized.first_frame()
    }

    pub(crate) const fn as_realized(&self) -> &RealizedBranch {
        &self.realized
    }

    pub(crate) fn into_realized(self) -> RealizedBranch {
        self.realized
    }

    pub(crate) const fn from_realized(realized: RealizedBranch) -> Self {
        Self { realized }
    }

    pub(crate) fn into_single_frame(self) -> Result<Option<RealizedFrame>, Self> {
        match self.realized.into_single_frame() {
            Ok(frame) => Ok(frame),
            Err(realized) => Err(Self { realized }),
        }
    }
}

pub(crate) struct PendingCommit {
    pub(crate) ticket: Option<CommitTicket>,
    pub(crate) final_task: Option<RootFrameTask>,
    pub(crate) branch: PreparedBranch,
}

impl PendingCommit {
    pub(crate) const fn new() -> Self {
        Self {
            ticket: None,
            final_task: None,
            branch: PreparedBranch::empty(),
        }
    }

    pub(crate) fn frame(&self) -> Option<&RealizedFrame> {
        self.branch.first_frame()
    }

    pub(crate) fn take_single_frame(&mut self) -> Option<RealizedFrame> {
        let branch = std::mem::replace(&mut self.branch, PreparedBranch::empty());
        match branch.into_single_frame() {
            Ok(frame) => frame,
            Err(branch) => {
                self.branch = branch;
                None
            }
        }
    }

    pub(crate) fn restore_single_frame(&mut self, frame: RealizedFrame) {
        debug_assert!(self.branch.frames().is_empty());
        self.branch
            .push_frame(frame)
            .expect("restored Root frame must retain its empty branch owner");
    }
}

pub(crate) struct FailedPreparedBranch {
    realized: RealizedBranch,
    mount_residuals: Vec<ChildSlotResidual>,
    detached_frames: Vec<RealizedFrame>,
    rejected_child_slots: Vec<RejectedChildSlot>,
    mount_new_allowance: Option<SealedMountNewStagingAllowance>,
}

impl FailedPreparedBranch {
    pub(crate) fn new(branch: PreparedBranch) -> Self {
        Self {
            realized: branch.into_realized(),
            mount_residuals: Vec::new(),
            detached_frames: Vec::new(),
            rejected_child_slots: Vec::new(),
            mount_new_allowance: None,
        }
    }

    pub(crate) fn with_mount_residual(branch: PreparedBranch, residual: ChildSlotResidual) -> Self {
        Self {
            realized: branch.into_realized(),
            mount_residuals: vec![residual],
            detached_frames: Vec::new(),
            rejected_child_slots: Vec::new(),
            mount_new_allowance: None,
        }
    }

    pub(crate) fn with_detached_frame(branch: PreparedBranch, frame: RealizedFrame) -> Self {
        Self {
            realized: branch.into_realized(),
            mount_residuals: Vec::new(),
            detached_frames: vec![frame],
            rejected_child_slots: Vec::new(),
            mount_new_allowance: None,
        }
    }

    pub(crate) fn with_rejected_child_slot(
        branch: PreparedBranch,
        slot: RejectedChildSlot,
    ) -> Self {
        Self {
            realized: branch.into_realized(),
            mount_residuals: Vec::new(),
            detached_frames: Vec::new(),
            rejected_child_slots: vec![slot],
            mount_new_allowance: None,
        }
    }

    pub(crate) fn with_mount_new_allowance(
        mut self,
        allowance: Option<SealedMountNewStagingAllowance>,
    ) -> Self {
        self.mount_new_allowance = allowance;
        self
    }

    pub(crate) const fn mount_new_allowance(&self) -> Option<&SealedMountNewStagingAllowance> {
        self.mount_new_allowance.as_ref()
    }

    pub(crate) fn frames(&self) -> &[RealizedFrame] {
        self.realized.frames()
    }

    pub(crate) const fn realized(&self) -> &RealizedBranch {
        &self.realized
    }

    pub(crate) fn child_slots(&self) -> &[MountedChildSlot] {
        self.realized.child_slots()
    }

    pub(crate) fn mount_residuals(&self) -> &[ChildSlotResidual] {
        &self.mount_residuals
    }

    pub(crate) fn detached_frames(&self) -> &[RealizedFrame] {
        &self.detached_frames
    }

    pub(crate) fn rejected_child_slots(&self) -> &[RejectedChildSlot] {
        &self.rejected_child_slots
    }

    #[cfg(feature = "test-support")]
    pub(crate) fn tracked_child_slots(&self) -> Vec<TrackedChildSlot> {
        self.realized.tracked_child_slots()
    }

    #[cfg(test)]
    pub(crate) const fn realized_for_test(&self) -> &RealizedBranch {
        &self.realized
    }

    #[cfg(test)]
    pub(crate) fn into_realized_for_test(self) -> RealizedBranch {
        self.realized
    }

    #[cfg(test)]
    pub(crate) fn duplicate_first_child_slot_for_test(&mut self) {
        let duplicate = self.realized.child_slots()[0].clone_for_test();
        self.realized.push_child_slot_unchecked_for_test(duplicate);
    }

    #[cfg(test)]
    pub(crate) fn replace_frame_claim_for_test(
        &mut self,
        frame: usize,
        entity: bevy_ecs::prelude::Entity,
        claim: ExactRegistryClaim,
    ) {
        self.realized
            .replace_frame_claim_for_test(frame, entity, claim);
    }

    #[cfg(test)]
    pub(crate) fn push_rejected_child_slot_for_test(&mut self, slot: RejectedChildSlot) {
        self.rejected_child_slots.push(slot);
    }
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "test-support")]
    use bevy_camera::visibility::Visibility;

    use super::*;
    use crate::navigation::tests::failed_nested_branch_fixture;
    #[cfg(feature = "test-support")]
    use crate::{components::ChildSlotLifecycle, registry::RouteEntityRegistry};

    fn reconciliation_authority(branch: &RealizedBranch) -> ReconciliationAuthorityIdentity {
        let root = branch.frames().first().expect("fixture has a root").frame();
        ReconciliationAuthorityIdentity::from_frame(root, root.host(), root.slot())
            .expect("root binds its own host and top slot")
    }

    fn active_sparse_branch_fixture() -> RealizedBranch {
        let (mut app, branch, _, _, _) = failed_nested_branch_fixture();
        let mut branch = branch.into_realized_for_test();
        activate_branch(&mut app, &mut branch);
        branch.pop_frame_for_test();
        transition_slot_claims(&mut branch, EntityLifecycle::PreparedInert);
        branch
    }

    #[test]
    fn active_sparse_leaving_accepts_active_frames_and_prepared_empty_slots() {
        let branch = active_sparse_branch_fixture();
        let targets = branch
            .active_sparse_leaving_mutation_targets(EntityLifecycle::Leaving)
            .expect("active sparse branch must retain its mixed frame and empty-slot roles");
        assert_eq!(targets.len(), branch.obligation_count());

        let resulting = branch
            .lifecycle_transitions(EntityLifecycle::Leaving, true)
            .into_iter()
            .map(|(claim, next)| claim.with_lifecycle_for_test(next))
            .collect::<Vec<_>>();
        assert!(
            branch
                .prepare_active_sparse_claim_rebind(&resulting, EntityLifecycle::Leaving)
                .is_some()
        );
    }

    #[test]
    fn active_sparse_leaving_rejects_prepared_frame_authority() {
        let mut branch = active_sparse_branch_fixture();
        transition_frame_claims(&mut branch, EntityLifecycle::PreparedInert);
        assert!(
            branch
                .active_sparse_leaving_mutation_targets(EntityLifecycle::Leaving)
                .is_none()
        );

        let resulting = branch
            .lifecycle_transitions(EntityLifecycle::Leaving, true)
            .into_iter()
            .map(|(claim, next)| claim.with_lifecycle_for_test(next))
            .collect::<Vec<_>>();
        assert!(
            branch
                .prepare_active_sparse_claim_rebind(&resulting, EntityLifecycle::Leaving)
                .is_none()
        );
    }

    fn activate_branch(app: &mut bevy_app::App, branch: &mut RealizedBranch) {
        let current = branch.claims().cloned().collect::<Vec<_>>();
        let next = current
            .iter()
            .map(|claim| {
                app.world_mut()
                    .resource_mut::<crate::registry::RouteEntityRegistry>()
                    .compare_exchange_lifecycle(claim, EntityLifecycle::Active)
                    .expect("fixture claim remains exact")
            })
            .collect::<Vec<_>>();
        let prepared = branch
            .prepare_claim_rebind(
                &next,
                EntityLifecycle::PreparedInert,
                EntityLifecycle::Active,
            )
            .expect("all branch aliases rebind atomically");
        branch.apply_claim_rebind(prepared);
    }

    fn transition_frame_claims(branch: &mut RealizedBranch, next: EntityLifecycle) {
        let rebound = branch
            .frames
            .iter()
            .flat_map(RealizedFrame::claims)
            .map(|claim| claim.with_lifecycle_for_test(next))
            .collect::<Vec<_>>();
        let mut offset = 0;
        for frame in &mut branch.frames {
            let end = offset + frame.claims().len();
            frame.apply_claim_rebind(&rebound[offset..end], &rebound);
            offset = end;
        }
        for slot in &mut branch.child_slots {
            slot.apply_internal_claim_rebind(&rebound);
        }
    }

    fn transition_slot_claims(branch: &mut RealizedBranch, next: EntityLifecycle) {
        let rebound = branch
            .child_slots
            .iter()
            .map(|slot| slot.slot_claim().with_lifecycle_for_test(next))
            .collect::<Vec<_>>();
        for slot in &mut branch.child_slots {
            slot.apply_internal_claim_rebind(&rebound);
        }
        for frame in &mut branch.frames {
            let primary = frame.claims().cloned().collect::<Vec<_>>();
            frame.apply_claim_rebind(&primary, &rebound);
        }
    }

    fn transition_detached_frame_claims(
        frame: &mut RealizedFrame,
        next: EntityLifecycle,
        aliases: &[ExactRegistryClaim],
    ) {
        let primary = frame
            .claims()
            .map(|claim| claim.with_lifecycle_for_test(next))
            .collect::<Vec<_>>();
        let mut all = primary.clone();
        all.extend_from_slice(aliases);
        frame.apply_claim_rebind(&primary, &all);
    }

    fn reconciliation_existing_projection(
        branch: &RealizedBranch,
        slot_role: ReconciliationSlotRoleKind,
    ) -> ReconciliationExistingProjection {
        let parent = branch.frames()[0].frame().clone();
        let child = branch.frames()[1].frame().clone();
        let slot = &branch.child_slots()[0];
        ReconciliationExistingProjection {
            preserved: vec![ReconciliationExistingFrame {
                branch_index: 0,
                version: parent.clone(),
            }],
            leaving: vec![ReconciliationExistingFrame {
                branch_index: 1,
                version: child.clone(),
            }],
            child_slots: vec![ReconciliationExistingSlot {
                parent_branch_index: 0,
                parent,
                outlet: slot.outlet().clone(),
                slot: slot.slot_id(),
                child: Some(child),
                role: slot_role,
            }],
        }
    }

    #[test]
    fn reconciliation_preserved_active_keeps_exact_boundary_slot() {
        let (mut app, branch, _, slot_entity, _) = failed_nested_branch_fixture();
        let mut branch = branch.into_realized_for_test();
        activate_branch(&mut app, &mut branch);
        let projection = reconciliation_existing_projection(
            &branch,
            ReconciliationSlotRoleKind::PreservedActive,
        );

        let sealed = SealedExistingReconciliation::prepare(branch, projection)
            .expect("exact common-prefix boundary must seal");

        assert_eq!(sealed.preserved().frames().len(), 1);
        assert_eq!(sealed.leaving().frames().len(), 1);
        assert_eq!(sealed.preserved().child_slots()[0].slot(), slot_entity);
        assert_eq!(
            sealed.slot_roles()[0].kind(),
            ReconciliationSlotRoleKind::PreservedActive
        );
    }

    #[test]
    fn reconciliation_retain_empty_truncate_keeps_registered_slot_out_of_leaving() {
        let (mut app, branch, _, slot_entity, _) = failed_nested_branch_fixture();
        let mut branch = branch.into_realized_for_test();
        activate_branch(&mut app, &mut branch);
        let projection =
            reconciliation_existing_projection(&branch, ReconciliationSlotRoleKind::RetainEmpty);

        let sealed = SealedExistingReconciliation::prepare(branch, projection)
            .expect("exact truncate boundary must seal");
        let splice = sealed
            .bind_entering(PreparedBranch::empty())
            .expect("truncate has no entering suffix");
        let outcome = splice.apply();
        assert_eq!(outcome.preserved().frames().len(), 1);
        assert_eq!(outcome.preserved().child_slots()[0].slot(), slot_entity);
        assert!(outcome.leaving().child_slots().is_empty());
        assert_eq!(outcome.leaving().frames().len(), 1);
    }

    #[test]
    fn reconciliation_existing_reject_restores_old_active_and_role_owners() {
        let (mut app, branch, _, slot_entity, _) = failed_nested_branch_fixture();
        let mut branch = branch.into_realized_for_test();
        activate_branch(&mut app, &mut branch);
        let original_frames = branch
            .frames()
            .iter()
            .map(|frame| frame.frame().clone())
            .collect::<Vec<_>>();
        let projection =
            reconciliation_existing_projection(&branch, ReconciliationSlotRoleKind::RetainEmpty);
        let sealed = SealedExistingReconciliation::prepare(branch, projection)
            .expect("exact truncate boundary must seal");

        let rejected = sealed.reject();
        let (restored, pending_entering, roles) = rejected.into_parts();
        assert_eq!(
            restored
                .frames()
                .iter()
                .map(|frame| frame.frame())
                .collect::<Vec<_>>(),
            original_frames.iter().collect::<Vec<_>>()
        );
        assert_eq!(restored.child_slots().len(), 1);
        assert!(pending_entering.frames().is_empty());
        assert_eq!(roles.len(), 1);
        assert_eq!(roles[0].entity(), slot_entity);
        assert_eq!(roles[0].kind(), ReconciliationSlotRoleKind::RetainEmpty);
    }

    #[test]
    fn reconciliation_role_rebind_keeps_preserved_primary_and_updates_mixed_aliases() {
        let (mut app, branch, _, slot_entity, _) = failed_nested_branch_fixture();
        let mut branch = branch.into_realized_for_test();
        activate_branch(&mut app, &mut branch);
        let projection =
            reconciliation_existing_projection(&branch, ReconciliationSlotRoleKind::RetainEmpty);
        let mut splice = SealedExistingReconciliation::prepare(branch, projection)
            .expect("exact truncate boundary must seal")
            .bind_entering(PreparedBranch::empty())
            .expect("truncate has no entering suffix");
        let preserved_primary_before = splice.preserved().frames()[0]
            .claims()
            .cloned()
            .collect::<Vec<_>>();

        let transitions = splice.lifecycle_transitions();
        assert!(transitions.iter().all(|(claim, next)| {
            if claim.entity() == slot_entity {
                claim.lifecycle() == EntityLifecycle::Active
                    && *next == EntityLifecycle::PreparedInert
            } else {
                claim.lifecycle() == EntityLifecycle::Active && *next == EntityLifecycle::Leaving
            }
        }));
        let resulting = transitions
            .iter()
            .map(|(claim, next)| claim.with_lifecycle_for_test(*next))
            .collect::<Vec<_>>();
        let prepared = splice
            .prepare_claim_rebind(&resulting)
            .expect("mixed role claims and aliases must prepare atomically");
        splice.apply_claim_rebind(prepared);

        assert_eq!(
            splice.preserved().frames()[0]
                .claims()
                .cloned()
                .collect::<Vec<_>>(),
            preserved_primary_before
        );
        assert_eq!(
            splice.preserved().child_slots()[0].slot_claim().lifecycle(),
            EntityLifecycle::PreparedInert
        );
        assert!(
            splice.leaving().frames()[0]
                .claims()
                .all(|claim| claim.lifecycle() == EntityLifecycle::Leaving)
        );
        assert_eq!(
            splice.leaving().frames()[0]
                .mount_binding()
                .slot_claim()
                .lifecycle(),
            EntityLifecycle::PreparedInert
        );
    }

    #[test]
    fn reconciliation_rejects_duplicate_and_cross_boundary_roles() {
        let (_app, branch, _, _, _) = failed_nested_branch_fixture();
        let branch = branch.into_realized_for_test();
        let mut duplicate =
            reconciliation_existing_projection(&branch, ReconciliationSlotRoleKind::RetainEmpty);
        duplicate.leaving.push(duplicate.preserved[0].clone());
        assert!(SealedExistingReconciliation::prepare(branch, duplicate).is_err());

        let (_app, branch, _, _, _) = failed_nested_branch_fixture();
        let branch = branch.into_realized_for_test();
        let mut crossing =
            reconciliation_existing_projection(&branch, ReconciliationSlotRoleKind::RetainEmpty);
        crossing.preserved.clear();
        crossing.leaving.insert(
            0,
            ReconciliationExistingFrame {
                branch_index: 0,
                version: branch.frames()[0].frame().clone(),
            },
        );
        assert!(SealedExistingReconciliation::prepare(branch, crossing).is_err());
    }

    #[test]
    fn reconciliation_existing_frames_must_all_be_active_and_return_branch_on_failure() {
        let (_app, branch, _, _, _) = failed_nested_branch_fixture();
        let branch = branch.into_realized_for_test();
        let projection =
            reconciliation_existing_projection(&branch, ReconciliationSlotRoleKind::RetainEmpty);

        let failure = SealedExistingReconciliation::prepare(branch, projection)
            .expect_err("prepared fixture frames cannot masquerade as existing active frames");
        let (branch, error) = failure.into_parts();

        assert_eq!(error, ReconciliationSpliceError::InvalidFramePartition);
        assert_eq!(branch.frames().len(), 2);
        assert_eq!(branch.child_slots().len(), 1);
    }

    #[test]
    fn reconciliation_occupied_existing_slot_must_be_active() {
        let (_app, branch, _, _, _) = failed_nested_branch_fixture();
        let mut branch = branch.into_realized_for_test();
        transition_frame_claims(&mut branch, EntityLifecycle::Active);
        let projection =
            reconciliation_existing_projection(&branch, ReconciliationSlotRoleKind::RetainEmpty);

        let failure = SealedExistingReconciliation::prepare(branch, projection)
            .expect_err("occupied PreparedInert slot cannot retain an active child");
        let (branch, error) = failure.into_parts();

        assert_eq!(error, ReconciliationSpliceError::InvalidSlotPartition);
        assert_eq!(
            branch.child_slots()[0].slot_claim().lifecycle(),
            EntityLifecycle::PreparedInert
        );
    }

    #[test]
    fn reconciliation_empty_existing_slot_must_be_prepared_inert() {
        let (mut app, branch, _, _, _) = failed_nested_branch_fixture();
        let mut branch = branch.into_realized_for_test();
        activate_branch(&mut app, &mut branch);
        branch.frames.pop().expect("fixture has one child frame");
        let parent = branch.frames()[0].frame().clone();
        let slot = &branch.child_slots()[0];
        let projection = ReconciliationExistingProjection {
            preserved: vec![ReconciliationExistingFrame {
                branch_index: 0,
                version: parent.clone(),
            }],
            leaving: Vec::new(),
            child_slots: vec![ReconciliationExistingSlot {
                parent_branch_index: 0,
                parent,
                outlet: slot.outlet().clone(),
                slot: slot.slot_id(),
                child: None,
                role: ReconciliationSlotRoleKind::RetainEmpty,
            }],
        };

        let failure = SealedExistingReconciliation::prepare(branch, projection)
            .expect_err("empty Active slot cannot masquerade as retained inert state");
        let (branch, error) = failure.into_parts();

        assert_eq!(error, ReconciliationSpliceError::InvalidSlotPartition);
        assert_eq!(
            branch.child_slots()[0].slot_claim().lifecycle(),
            EntityLifecycle::Active
        );
    }

    #[test]
    fn reconciliation_entering_frames_must_all_be_prepared_and_return_inputs_on_failure() {
        let (_app, branch, _, _, _) = failed_nested_branch_fixture();
        let mut entering = branch.into_realized_for_test();
        transition_frame_claims(&mut entering, EntityLifecycle::Active);
        let root = entering.frames()[0].frame().clone();
        let child = entering.frames()[1].frame().clone();
        let slot = &entering.child_slots()[0];
        let facts = ReconciliationProjectionFacts {
            authority: reconciliation_authority(&entering),
            existing: ReconciliationExistingProjection {
                preserved: Vec::new(),
                leaving: Vec::new(),
                child_slots: Vec::new(),
            },
            entering: ReconciliationEnteringProjection {
                frames: vec![
                    ReconciliationEnteringFrame {
                        branch_index: 0,
                        route: matched_route(&root),
                        frame: root.frame(),
                        generation: root.generation(),
                        slot: ReconciliationEnteringSlot::Existing(root.slot()),
                    },
                    ReconciliationEnteringFrame {
                        branch_index: 1,
                        route: matched_route(&child),
                        frame: child.frame(),
                        generation: child.generation(),
                        slot: ReconciliationEnteringSlot::MountNew,
                    },
                ],
                mount_new_slots: vec![ReconciliationMountNewSlot {
                    parent_branch_index: 0,
                    outlet: slot.outlet().clone(),
                }],
            },
        };
        let sealed =
            SealedExistingReconciliation::prepare_from_projection(RealizedBranch::empty(), facts)
                .expect("empty existing partition seals independently");

        let failure = sealed
            .bind_entering(PreparedBranch::from_realized(entering))
            .expect_err("Active frames cannot masquerade as prepared entering frames");
        let (sealed, entering, error) = failure.into_parts();

        assert_eq!(error, ReconciliationSpliceError::InvalidEnteringBranch);
        assert!(sealed.preserved().frames().is_empty());
        assert_eq!(entering.frames().len(), 2);
    }

    #[test]
    fn reconciliation_entering_child_slots_must_all_be_prepared_inert() {
        let (_app, branch, _, _, _) = failed_nested_branch_fixture();
        let mut entering = branch.into_realized_for_test();
        transition_slot_claims(&mut entering, EntityLifecycle::Active);
        let root = entering.frames()[0].frame().clone();
        let child = entering.frames()[1].frame().clone();
        let slot = &entering.child_slots()[0];
        let facts = ReconciliationProjectionFacts {
            authority: reconciliation_authority(&entering),
            existing: ReconciliationExistingProjection {
                preserved: Vec::new(),
                leaving: Vec::new(),
                child_slots: Vec::new(),
            },
            entering: ReconciliationEnteringProjection {
                frames: vec![
                    ReconciliationEnteringFrame {
                        branch_index: 0,
                        route: matched_route(&root),
                        frame: root.frame(),
                        generation: root.generation(),
                        slot: ReconciliationEnteringSlot::Existing(root.slot()),
                    },
                    ReconciliationEnteringFrame {
                        branch_index: 1,
                        route: matched_route(&child),
                        frame: child.frame(),
                        generation: child.generation(),
                        slot: ReconciliationEnteringSlot::MountNew,
                    },
                ],
                mount_new_slots: vec![ReconciliationMountNewSlot {
                    parent_branch_index: 0,
                    outlet: slot.outlet().clone(),
                }],
            },
        };
        let sealed =
            SealedExistingReconciliation::prepare_from_projection(RealizedBranch::empty(), facts)
                .expect("empty existing partition seals independently");

        let failure = sealed
            .bind_entering(PreparedBranch::from_realized(entering))
            .expect_err("Active entering child slot cannot masquerade as MountNew");
        let (_, entering, error) = failure.into_parts();

        assert_eq!(error, ReconciliationSpliceError::InvalidEnteringBranch);
        assert_eq!(
            entering.as_realized().child_slots()[0]
                .slot_claim()
                .lifecycle(),
            EntityLifecycle::Active
        );
    }

    #[test]
    fn reconciliation_boundary_mount_rejects_foreign_anchor_alias_and_returns_slot() {
        let (_app, branch, _, _, _) = failed_nested_branch_fixture();
        let mut branch = branch.into_realized_for_test();
        let child = branch.frames.pop().expect("fixture has one child frame");
        let mut slot = branch
            .child_slots
            .pop()
            .expect("fixture has one boundary slot");
        let parent = &mut branch.frames[0];
        let primary = parent.claims().cloned().collect::<Vec<_>>();
        let foreign_anchor = slot
            .anchor_claim()
            .with_seal_for_test(child.claims().next().expect("child has claims").seal());
        parent.apply_claim_rebind(&primary, std::slice::from_ref(&foreign_anchor));
        slot.apply_internal_claim_rebind(std::slice::from_ref(&foreign_anchor));
        assert_eq!(
            parent.outlet(slot.outlet()).unwrap().anchor_claim(),
            &foreign_anchor
        );
        assert_eq!(slot.anchor_claim(), &foreign_anchor);
        assert!(!parent.claims().any(|claim| claim == &foreign_anchor));

        let mut entering = PreparedBranch::empty();
        let rejected = entering
            .push_reconciliation_child_slot(slot, parent)
            .expect_err("matching foreign aliases are not preserved-parent authority");

        assert_eq!(rejected.slot().anchor_claim(), &foreign_anchor);
        assert!(entering.as_realized().child_slots().is_empty());
    }

    fn matched_route(frame: &RouteFrameVersion) -> RouteId {
        match frame.origin() {
            FrameOrigin::MatchedRoute(route) => *route,
            _ => panic!("nested fixture must contain matched routes"),
        }
    }

    #[test]
    fn reconciliation_append_activates_exact_retained_empty_slot() {
        let (mut app, branch, _, slot_entity, _) = failed_nested_branch_fixture();
        let mut branch = branch.into_realized_for_test();
        activate_branch(&mut app, &mut branch);
        let truncate =
            reconciliation_existing_projection(&branch, ReconciliationSlotRoleKind::RetainEmpty);
        let mut truncate =
            SealedExistingReconciliation::prepare(branch, truncate).expect("truncate must seal");
        let mut child = truncate.leaving.frames.pop().expect("one leaving child");
        let truncate = truncate
            .bind_entering(PreparedBranch::empty())
            .expect("truncate has no entering suffix")
            .apply();
        let (mut active, leaving) = truncate.into_active_and_leaving();
        assert!(leaving.frames().is_empty());
        transition_slot_claims(&mut active, EntityLifecycle::PreparedInert);
        let slot_claim = active.child_slots()[0].slot_claim().clone();
        transition_detached_frame_claims(
            &mut child,
            EntityLifecycle::PreparedInert,
            std::slice::from_ref(&slot_claim),
        );
        let parent = active.frames()[0].frame().clone();
        let child_version = child.frame().clone();
        let retained = &active.child_slots()[0];
        let facts = ReconciliationProjectionFacts {
            authority: reconciliation_authority(&active),
            existing: ReconciliationExistingProjection {
                preserved: vec![ReconciliationExistingFrame {
                    branch_index: 0,
                    version: parent.clone(),
                }],
                leaving: Vec::new(),
                child_slots: vec![ReconciliationExistingSlot {
                    parent_branch_index: 0,
                    parent,
                    outlet: retained.outlet().clone(),
                    slot: retained.slot_id(),
                    child: None,
                    role: ReconciliationSlotRoleKind::ActivateExisting,
                }],
            },
            entering: ReconciliationEnteringProjection {
                frames: vec![ReconciliationEnteringFrame {
                    branch_index: 1,
                    route: matched_route(&child_version),
                    frame: child_version.frame(),
                    generation: child_version.generation(),
                    slot: ReconciliationEnteringSlot::Existing(retained.slot_id()),
                }],
                mount_new_slots: Vec::new(),
            },
        };
        let sealed = SealedExistingReconciliation::prepare_from_projection(active, facts)
            .expect("retained empty slot remains exact");
        let mut entering = PreparedBranch::empty();
        entering
            .push_frame(child)
            .expect("entering suffix stores its physical frame");

        let outcome = sealed
            .bind_entering(entering)
            .expect("existing slot and entering mount binding must agree")
            .apply();
        let (active, _) = outcome.into_active_and_leaving();

        assert_eq!(active.frames().len(), 2);
        assert_eq!(active.child_slots().len(), 1);
        assert_eq!(active.child_slots()[0].slot(), slot_entity);
    }

    #[test]
    fn reconciliation_mount_new_accepts_only_exact_mounted_slot_binding() {
        let (_app, branch, _, slot_entity, _) = failed_nested_branch_fixture();
        let entering = branch.into_realized_for_test();
        let root = entering.frames()[0].frame().clone();
        let child = entering.frames()[1].frame().clone();
        let slot = &entering.child_slots()[0];
        let facts = ReconciliationProjectionFacts {
            authority: reconciliation_authority(&entering),
            existing: ReconciliationExistingProjection {
                preserved: Vec::new(),
                leaving: Vec::new(),
                child_slots: Vec::new(),
            },
            entering: ReconciliationEnteringProjection {
                frames: vec![
                    ReconciliationEnteringFrame {
                        branch_index: 0,
                        route: matched_route(&root),
                        frame: root.frame(),
                        generation: root.generation(),
                        slot: ReconciliationEnteringSlot::Existing(root.slot()),
                    },
                    ReconciliationEnteringFrame {
                        branch_index: 1,
                        route: matched_route(&child),
                        frame: child.frame(),
                        generation: child.generation(),
                        slot: ReconciliationEnteringSlot::MountNew,
                    },
                ],
                mount_new_slots: vec![ReconciliationMountNewSlot {
                    parent_branch_index: 0,
                    outlet: slot.outlet().clone(),
                }],
            },
        };
        let sealed =
            SealedExistingReconciliation::prepare_from_projection(RealizedBranch::empty(), facts)
                .expect("empty active partition must seal");

        let outcome = sealed
            .bind_entering(PreparedBranch::from_realized(entering))
            .expect("exact mounted slot and frame binding must agree")
            .apply();

        let (active, leaving) = outcome.into_active_and_leaving();
        assert_eq!(active.child_slots()[0].slot(), slot_entity);
        assert!(leaving.frames().is_empty());
    }

    #[test]
    fn reconciliation_splice_reject_returns_independent_entering_and_role_owners() {
        let (_app, branch, _, slot_entity, _) = failed_nested_branch_fixture();
        let entering = branch.into_realized_for_test();
        let root = entering.frames()[0].frame().clone();
        let child = entering.frames()[1].frame().clone();
        let slot = &entering.child_slots()[0];
        let facts = ReconciliationProjectionFacts {
            authority: reconciliation_authority(&entering),
            existing: ReconciliationExistingProjection {
                preserved: Vec::new(),
                leaving: Vec::new(),
                child_slots: Vec::new(),
            },
            entering: ReconciliationEnteringProjection {
                frames: vec![
                    ReconciliationEnteringFrame {
                        branch_index: 0,
                        route: matched_route(&root),
                        frame: root.frame(),
                        generation: root.generation(),
                        slot: ReconciliationEnteringSlot::Existing(root.slot()),
                    },
                    ReconciliationEnteringFrame {
                        branch_index: 1,
                        route: matched_route(&child),
                        frame: child.frame(),
                        generation: child.generation(),
                        slot: ReconciliationEnteringSlot::MountNew,
                    },
                ],
                mount_new_slots: vec![ReconciliationMountNewSlot {
                    parent_branch_index: 0,
                    outlet: slot.outlet().clone(),
                }],
            },
        };
        let sealed =
            SealedExistingReconciliation::prepare_from_projection(RealizedBranch::empty(), facts)
                .expect("empty active partition must seal");
        let splice = sealed
            .bind_entering(PreparedBranch::from_realized(entering))
            .expect("exact mounted slot and frame binding must agree");

        let rejected = splice.reject();
        let (restored, pending_entering, roles) = rejected.into_parts();
        assert!(restored.frames().is_empty());
        assert_eq!(pending_entering.frames().len(), 2);
        assert_eq!(pending_entering.child_slots().len(), 1);
        assert_eq!(pending_entering.child_slots()[0].slot(), slot_entity);
        assert_eq!(roles.len(), 1);
        assert_eq!(roles[0].entity(), slot_entity);
        assert_eq!(roles[0].kind(), ReconciliationSlotRoleKind::MountNew);
    }

    #[test]
    fn reconciliation_mount_new_slot_only_failure_freezes_one_cleanup_obligation() {
        let (app, branch, _, slot_entity, _) = failed_nested_branch_fixture();
        let mut realized = branch.into_realized_for_test();
        let mounted = realized
            .child_slots
            .pop()
            .expect("nested fixture has one mounted child slot");
        let parent = &realized.frames[0];
        let mut pending = PreparedBranch::empty();
        pending
            .push_reconciliation_child_slot(mounted, parent)
            .expect("MountNew boundary slot binds the preserved parent");
        let failed = FailedPreparedBranch::new(pending);

        let manifest = crate::cleanup::FrozenCleanupManifest::from_failed_prepared_branch(
            app.world(),
            app.world()
                .resource::<crate::registry::RouteEntityRegistry>(),
            &failed,
        )
        .expect("slot-only MountNew residual is an independent cleanup owner");
        assert_eq!(manifest.len(), 1);
        assert_eq!(failed.child_slots()[0].slot(), slot_entity);
    }

    #[test]
    fn reconciliation_mount_new_boundary_accepts_exact_preserved_parent() {
        let (_app, branch, _, slot_entity, _) = failed_nested_branch_fixture();
        let mut branch = branch.into_realized_for_test();
        let mut child = branch.frames.pop().expect("one entering child");
        let mut mounted = branch.child_slots.pop().expect("one boundary slot");
        transition_frame_claims(&mut branch, EntityLifecycle::Active);
        let parent_claims = branch.frames()[0].claims().cloned().collect::<Vec<_>>();
        mounted.apply_internal_claim_rebind(&parent_claims);
        let child_claims = child.claims().cloned().collect::<Vec<_>>();
        child.apply_claim_rebind(&child_claims, &parent_claims);
        let parent = branch.frames()[0].frame().clone();
        let child_version = child.frame().clone();
        let facts = ReconciliationProjectionFacts {
            authority: reconciliation_authority(&branch),
            existing: ReconciliationExistingProjection {
                preserved: vec![ReconciliationExistingFrame {
                    branch_index: 0,
                    version: parent,
                }],
                leaving: Vec::new(),
                child_slots: Vec::new(),
            },
            entering: ReconciliationEnteringProjection {
                frames: vec![ReconciliationEnteringFrame {
                    branch_index: 1,
                    route: matched_route(&child_version),
                    frame: child_version.frame(),
                    generation: child_version.generation(),
                    slot: ReconciliationEnteringSlot::MountNew,
                }],
                mount_new_slots: vec![ReconciliationMountNewSlot {
                    parent_branch_index: 0,
                    outlet: mounted.outlet().clone(),
                }],
            },
        };
        let sealed = SealedExistingReconciliation::prepare_from_projection(branch, facts)
            .expect("preserved parent must seal");
        let mut entering = PreparedBranch::empty();
        entering
            .push_reconciliation_child_slot(mounted, &sealed.preserved().frames()[0])
            .expect("exact preserved parent admits its new boundary slot");
        entering
            .push_frame(child)
            .expect("mounted boundary slot admits its entering child");

        let mut splice = sealed
            .bind_entering(entering)
            .expect("boundary MountNew binding must remain exact");
        assert_eq!(splice.preserved().frames().len(), 1);
        assert_eq!(splice.entering().frames().len(), 1);
        let mount_new = splice
            .slot_roles()
            .iter()
            .find(|role| role.kind() == ReconciliationSlotRoleKind::MountNew)
            .expect("boundary slot remains an entering obligation");
        assert_eq!(mount_new.entity(), slot_entity);
        assert_eq!(mount_new.claim().entity(), slot_entity);
        assert_eq!(mount_new.current(), EntityLifecycle::PreparedInert);
        assert_eq!(mount_new.next(), Some(EntityLifecycle::Active));
        let entering_slot = splice.entering().child_slots()[0].clone_for_test();
        assert_eq!(mount_new.claim(), entering_slot.slot_claim());
        assert_eq!(mount_new.parent(), entering_slot.parent());
        assert_eq!(mount_new.outlet(), entering_slot.outlet());
        assert_eq!(mount_new.slot(), entering_slot.slot_id());
        assert_eq!(mount_new.anchor(), entering_slot.anchor());
        assert_eq!(mount_new.anchor_claim(), entering_slot.anchor_claim());

        let transitions = splice.lifecycle_transitions();
        assert_eq!(
            transitions
                .iter()
                .filter(|(claim, next)| {
                    claim.entity() == slot_entity && *next == EntityLifecycle::Active
                })
                .count(),
            1,
            "the sealed MountNew role owns exactly one PreparedInert-to-Active transition",
        );
        let resulting = transitions
            .iter()
            .map(|(claim, next)| claim.with_lifecycle_for_test(*next))
            .collect::<Vec<_>>();
        assert!(splice.prepare_claim_rebind(&resulting).is_some());

        assert!(crate::frame_local::sealed_mount_new_expectation_is_unique(
            splice.slot_roles(),
            &entering_slot,
        ));
        let wrong_role = sealed_slot_role(
            &entering_slot,
            ReconciliationSlotRoleKind::ActivateExisting,
            ReconciliationSlotEnteringUse::Required,
        );
        assert!(!crate::frame_local::sealed_mount_new_expectation_is_unique(
            std::slice::from_ref(&wrong_role),
            &entering_slot,
        ));
        let duplicate = sealed_slot_role(
            &entering_slot,
            ReconciliationSlotRoleKind::MountNew,
            ReconciliationSlotEnteringUse::Required,
        );
        let mut duplicated_roles = splice
            .slot_roles
            .drain(..)
            .collect::<Vec<SealedReconciliationSlotRole>>();
        duplicated_roles.push(duplicate);
        assert!(!crate::frame_local::sealed_mount_new_expectation_is_unique(
            &duplicated_roles,
            &entering_slot,
        ));
        splice.slot_roles = duplicated_roles;
        let duplicate_transitions = splice.lifecycle_transitions();
        let duplicate_resulting = duplicate_transitions
            .iter()
            .map(|(claim, next)| claim.with_lifecycle_for_test(*next))
            .collect::<Vec<_>>();
        assert!(
            splice.prepare_claim_rebind(&duplicate_resulting).is_none(),
            "duplicate role-owned transitions must fail closed before physical mutation",
        );
        let mut duplicated_roles = splice.slot_roles.drain(..).collect::<Vec<_>>();
        duplicated_roles.pop();
        splice.slot_roles = duplicated_roles;

        let outcome = splice.apply();

        assert_eq!(outcome.preserved().frames().len(), 1);
        assert_eq!(outcome.entering().frames().len(), 1);
        assert_eq!(outcome.entering().child_slots()[0].slot(), slot_entity);
        assert_eq!(
            outcome
                .slot_roles()
                .iter()
                .filter(|role| role.kind() == ReconciliationSlotRoleKind::MountNew)
                .count(),
            1
        );
    }

    #[test]
    fn reconciliation_boundary_preserved_active_requires_entering_consumption() {
        let (mut app, branch, _, _, _) = failed_nested_branch_fixture();
        let mut branch = branch.into_realized_for_test();
        activate_branch(&mut app, &mut branch);
        let projection = reconciliation_existing_projection(
            &branch,
            ReconciliationSlotRoleKind::PreservedActive,
        );
        let sealed = SealedExistingReconciliation::prepare(branch, projection)
            .expect("boundary role itself is exact");

        let failure = sealed
            .bind_entering(PreparedBranch::empty())
            .expect_err("boundary role cannot remain unconsumed");
        let (sealed, entering, error) = failure.into_parts();
        assert_eq!(error, ReconciliationSpliceError::InvalidEnteringBranch);
        assert_eq!(sealed.preserved().frames().len(), 1);
        assert_eq!(sealed.leaving().frames().len(), 1);
        assert!(entering.frames().is_empty());
    }

    #[test]
    fn reconciliation_activate_existing_requires_entering_and_retain_empty_forbids_it() {
        let (mut app, branch, _, _, _) = failed_nested_branch_fixture();
        let mut branch = branch.into_realized_for_test();
        activate_branch(&mut app, &mut branch);
        let truncate =
            reconciliation_existing_projection(&branch, ReconciliationSlotRoleKind::RetainEmpty);
        let truncate = SealedExistingReconciliation::prepare(branch, truncate)
            .unwrap()
            .bind_entering(PreparedBranch::empty())
            .unwrap()
            .apply();
        let (mut active, mut leaving) = truncate.into_active_and_leaving();
        let mut child = leaving.frames.pop().unwrap();
        transition_slot_claims(&mut active, EntityLifecycle::PreparedInert);
        let slot_claim = active.child_slots()[0].slot_claim().clone();
        transition_detached_frame_claims(
            &mut child,
            EntityLifecycle::PreparedInert,
            std::slice::from_ref(&slot_claim),
        );
        let retained = &active.child_slots()[0];
        let parent = active.frames()[0].frame().clone();

        let activate = ReconciliationProjectionFacts {
            authority: reconciliation_authority(&active),
            existing: ReconciliationExistingProjection {
                preserved: vec![ReconciliationExistingFrame {
                    branch_index: 0,
                    version: parent.clone(),
                }],
                leaving: Vec::new(),
                child_slots: vec![ReconciliationExistingSlot {
                    parent_branch_index: 0,
                    parent: parent.clone(),
                    outlet: retained.outlet().clone(),
                    slot: retained.slot_id(),
                    child: None,
                    role: ReconciliationSlotRoleKind::ActivateExisting,
                }],
            },
            entering: ReconciliationEnteringProjection {
                frames: Vec::new(),
                mount_new_slots: Vec::new(),
            },
        };
        let failure = SealedExistingReconciliation::prepare_from_projection(active, activate)
            .unwrap()
            .bind_entering(PreparedBranch::empty())
            .expect_err("ActivateExisting must be consumed");
        let (sealed, _, _) = failure.into_parts();

        let retained_branch = sealed.preserved;
        let retained = &retained_branch.child_slots()[0];
        let child_version = child.frame().clone();
        let retain = ReconciliationProjectionFacts {
            authority: reconciliation_authority(&retained_branch),
            existing: ReconciliationExistingProjection {
                preserved: vec![ReconciliationExistingFrame {
                    branch_index: 0,
                    version: parent.clone(),
                }],
                leaving: Vec::new(),
                child_slots: vec![ReconciliationExistingSlot {
                    parent_branch_index: 0,
                    parent,
                    outlet: retained.outlet().clone(),
                    slot: retained.slot_id(),
                    child: None,
                    role: ReconciliationSlotRoleKind::RetainEmpty,
                }],
            },
            entering: ReconciliationEnteringProjection {
                frames: vec![ReconciliationEnteringFrame {
                    branch_index: 1,
                    route: matched_route(&child_version),
                    frame: child_version.frame(),
                    generation: child_version.generation(),
                    slot: ReconciliationEnteringSlot::Existing(retained.slot_id()),
                }],
                mount_new_slots: Vec::new(),
            },
        };
        let sealed =
            SealedExistingReconciliation::prepare_from_projection(retained_branch, retain).unwrap();
        let mut entering = PreparedBranch::empty();
        entering.push_frame(child).unwrap();
        assert!(sealed.bind_entering(entering).is_err());
    }

    #[test]
    fn reconciliation_internal_preserved_active_forbids_entering_and_needs_no_consumption() {
        let (mut app, branch, _, _, _) = failed_nested_branch_fixture();
        let mut branch = branch.into_realized_for_test();
        activate_branch(&mut app, &mut branch);
        let root = branch.frames()[0].frame().clone();
        let child = branch.frames()[1].frame().clone();
        let slot = &branch.child_slots()[0];
        let projection = ReconciliationExistingProjection {
            preserved: vec![
                ReconciliationExistingFrame {
                    branch_index: 0,
                    version: root.clone(),
                },
                ReconciliationExistingFrame {
                    branch_index: 1,
                    version: child.clone(),
                },
            ],
            leaving: Vec::new(),
            child_slots: vec![ReconciliationExistingSlot {
                parent_branch_index: 0,
                parent: root,
                outlet: slot.outlet().clone(),
                slot: slot.slot_id(),
                child: Some(child),
                role: ReconciliationSlotRoleKind::PreservedActive,
            }],
        };
        let splice = SealedExistingReconciliation::prepare(branch, projection)
            .unwrap()
            .bind_entering(PreparedBranch::empty())
            .expect("internal preserved slot needs no entering consumer");
        assert_eq!(splice.preserved().frames().len(), 2);
        assert!(splice.entering().frames().is_empty());
    }

    #[test]
    fn reconciliation_rejects_physical_duplicate_entity_owner_and_seal() {
        let (_app, branch, _, _, _) = failed_nested_branch_fixture();
        let branch = branch.into_realized_for_test();
        let claims = branch.frames()[0]
            .claims()
            .take(2)
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(claims.len(), 2);

        let duplicate_entity = claims[1].with_entity_for_test(claims[0].entity());
        assert_eq!(
            validate_unique_authorities(&[&claims[0], &duplicate_entity]),
            Err(ReconciliationSpliceError::DuplicateAuthority)
        );

        let duplicate_owner = claims[1].with_owner_for_test(claims[0].owner().clone());
        assert_eq!(
            validate_unique_authorities(&[&claims[0], &duplicate_owner]),
            Err(ReconciliationSpliceError::DuplicateAuthority)
        );

        let duplicate_seal = claims[1].with_seal_for_test(claims[0].seal());
        assert_eq!(
            validate_unique_authorities(&[&claims[0], &duplicate_seal]),
            Err(ReconciliationSpliceError::DuplicateAuthority)
        );
    }

    #[test]
    fn reconciliation_realized_branch_rejects_duplicate_slot_id_and_parent_outlet() {
        let (mut app, branch, _, _, _) = failed_nested_branch_fixture();
        let mut branch = branch.into_realized_for_test();
        let original = &branch.child_slots()[0];
        let parent = original.parent().clone();
        let original_outlet = original.outlet().clone();
        let original_slot = original.slot_id();
        let mut duplicate_slot_id = original.clone_for_test();
        let mut duplicate_mapping = original.clone_for_test();
        let distinct_slot = branch.frames()[0].frame().slot();
        assert_ne!(original_slot, distinct_slot);
        let template = branch.frames()[0].claims().next().unwrap().clone();

        let entity = app.world_mut().spawn_empty().id();
        let different_outlet = RouteSlotKey::named("duplicate-id").unwrap();
        let owner = RegistryOwner::Committed(EntityOwner::ChildSlot {
            parent: parent.clone(),
            outlet: different_outlet.clone(),
            slot: original_slot,
        });
        let claim = template
            .with_entity_for_test(entity)
            .with_owner_for_test(owner);
        duplicate_slot_id.replace_slot_authority_for_test(
            entity,
            claim,
            different_outlet,
            original_slot,
        );
        assert!(branch.push_child_slot(duplicate_slot_id).is_err());

        let entity = app.world_mut().spawn_empty().id();
        let owner = RegistryOwner::Committed(EntityOwner::ChildSlot {
            parent,
            outlet: original_outlet.clone(),
            slot: distinct_slot,
        });
        let claim = template
            .with_entity_for_test(entity)
            .with_owner_for_test(owner);
        duplicate_mapping.replace_slot_authority_for_test(
            entity,
            claim,
            original_outlet,
            distinct_slot,
        );
        assert!(branch.push_child_slot(duplicate_mapping).is_err());
    }

    #[test]
    fn reconciliation_rejects_foreign_router_and_wrong_top_slot_identity() {
        let (mut app, branch, _, _, _) = failed_nested_branch_fixture();
        let mut branch = branch.into_realized_for_test();
        activate_branch(&mut app, &mut branch);
        let (_foreign_app, foreign, _, _, _) = failed_nested_branch_fixture();
        let foreign = foreign.into_realized_for_test();
        let projection =
            reconciliation_existing_projection(&branch, ReconciliationSlotRoleKind::RetainEmpty);
        let foreign_authority = ReconciliationAuthorityIdentity::from_frame(
            foreign.frames()[0].frame(),
            branch.frames()[0].frame().host(),
            branch.frames()[0].frame().slot(),
        )
        .expect("fixture raw host ids may coincide while router provenance differs");
        let facts = ReconciliationProjectionFacts {
            authority: foreign_authority,
            existing: projection,
            entering: ReconciliationEnteringProjection {
                frames: Vec::new(),
                mount_new_slots: Vec::new(),
            },
        };
        let failure = SealedExistingReconciliation::prepare_from_projection(branch, facts)
            .expect_err("foreign opaque router identity must reject");
        let (branch, error) = failure.into_parts();
        assert_eq!(error, ReconciliationSpliceError::InvalidAuthorityIdentity);
        assert_eq!(branch.frames().len(), 2);

        let projection =
            reconciliation_existing_projection(&branch, ReconciliationSlotRoleKind::RetainEmpty);
        let root = branch.frames()[0].frame();
        let wrong_top = ReconciliationAuthorityIdentity::from_frame(
            root,
            root.host(),
            branch.child_slots()[0].slot_id(),
        )
        .unwrap();
        let facts = ReconciliationProjectionFacts {
            authority: wrong_top,
            existing: projection,
            entering: ReconciliationEnteringProjection {
                frames: Vec::new(),
                mount_new_slots: Vec::new(),
            },
        };
        assert!(SealedExistingReconciliation::prepare_from_projection(branch, facts).is_err());
    }

    #[test]
    fn reconciliation_slot_role_lifecycle_pairs_are_closed() {
        assert_eq!(
            slot_role_next(
                ReconciliationSlotRoleKind::PreservedActive,
                EntityLifecycle::Active
            ),
            Some(None)
        );
        assert_eq!(
            slot_role_next(
                ReconciliationSlotRoleKind::ActivateExisting,
                EntityLifecycle::PreparedInert
            ),
            Some(Some(EntityLifecycle::Active))
        );
        assert_eq!(
            slot_role_next(
                ReconciliationSlotRoleKind::MountNew,
                EntityLifecycle::PreparedInert
            ),
            Some(Some(EntityLifecycle::Active))
        );
        assert!(
            slot_role_next(
                ReconciliationSlotRoleKind::PreservedActive,
                EntityLifecycle::PreparedInert
            )
            .is_none()
        );
        assert!(
            slot_role_next(
                ReconciliationSlotRoleKind::ActivateExisting,
                EntityLifecycle::Active
            )
            .is_none()
        );
        assert!(
            slot_role_next(
                ReconciliationSlotRoleKind::MountNew,
                EntityLifecycle::Active
            )
            .is_none()
        );
    }

    #[test]
    fn reconciliation_leaving_parent_slot_obligation_is_between_child_and_parent() {
        let (mut app, branch, _, slot_entity, _) = failed_nested_branch_fixture();
        let mut branch = branch.into_realized_for_test();
        activate_branch(&mut app, &mut branch);
        let parent_entities = branch.frames()[0]
            .claims()
            .map(ExactRegistryClaim::entity)
            .collect::<Vec<_>>();
        let child_entities = branch.frames()[1]
            .claims()
            .map(ExactRegistryClaim::entity)
            .collect::<Vec<_>>();
        let slot = &branch.child_slots()[0];
        let projection = ReconciliationExistingProjection {
            preserved: Vec::new(),
            leaving: vec![
                ReconciliationExistingFrame {
                    branch_index: 1,
                    version: branch.frames()[1].frame().clone(),
                },
                ReconciliationExistingFrame {
                    branch_index: 0,
                    version: branch.frames()[0].frame().clone(),
                },
            ],
            child_slots: vec![ReconciliationExistingSlot {
                parent_branch_index: 0,
                parent: branch.frames()[0].frame().clone(),
                outlet: slot.outlet().clone(),
                slot: slot.slot_id(),
                child: Some(branch.frames()[1].frame().clone()),
                role: ReconciliationSlotRoleKind::Leaving,
            }],
        };
        let sealed = SealedExistingReconciliation::prepare(branch, projection)
            .expect("full leaving suffix must seal");
        let order = sealed
            .leaving()
            .lifecycle_transitions(EntityLifecycle::Leaving, true)
            .into_iter()
            .map(|(claim, _)| claim.entity())
            .collect::<Vec<_>>();
        let slot_index = order
            .iter()
            .position(|entity| *entity == slot_entity)
            .expect("slot obligation remains present");

        assert!(child_entities.iter().all(|entity| {
            order.iter().position(|current| current == entity).unwrap() < slot_index
        }));
        assert!(parent_entities.iter().all(|entity| {
            order.iter().position(|current| current == entity).unwrap() > slot_index
        }));
    }

    #[cfg(feature = "test-support")]
    #[test]
    fn tracked_child_slot_follows_active_and_leaving_claim_lifecycles() {
        let (mut app, branch, _, _, _) = failed_nested_branch_fixture();
        let mut tracked = branch
            .tracked_child_slots()
            .pop()
            .expect("nested branch retains one child slot");

        let active = app
            .world_mut()
            .resource_mut::<RouteEntityRegistry>()
            .compare_exchange_lifecycle(&tracked.claim, EntityLifecycle::Active)
            .expect("prepared slot claim remains exact");
        tracked.claim = active;
        app.world_mut()
            .entity_mut(tracked.entity)
            .insert((ChildSlotLifecycle::Active, Visibility::Visible));
        assert!(tracked.is_current(app.world()));

        let leaving = app
            .world_mut()
            .resource_mut::<RouteEntityRegistry>()
            .compare_exchange_lifecycle(&tracked.claim, EntityLifecycle::Leaving)
            .expect("active slot claim remains exact");
        tracked.claim = leaving;
        app.world_mut()
            .entity_mut(tracked.entity)
            .insert((ChildSlotLifecycle::Leaving, Visibility::Hidden));
        assert!(tracked.is_current(app.world()));
    }

    #[test]
    fn mount_new_staging_allowance_rejects_wrong_parent_and_slot() {
        let (app, branch, _, _, _) = failed_nested_branch_fixture();
        let request = app
            .world()
            .resource::<crate::navigation::RouterNavigation>()
            .entries[0]
            .requests[0];
        let slot = &branch.child_slots()[0];
        let allowance = SealedMountNewStagingAllowance::for_test(
            request,
            slot.parent().clone(),
            slot.outlet().clone(),
            slot.slot_id(),
            slot.slot(),
            slot.slot_claim().clone(),
            slot.anchor(),
            slot.anchor_claim().clone(),
            slot.expected_anchor_children().to_vec(),
        );

        assert!(allowance.matches_binding(
            slot.parent(),
            slot.outlet(),
            slot.slot_id(),
            slot.slot(),
            slot.slot_claim(),
            slot.anchor(),
            slot.anchor_claim(),
        ));
        assert!(!allowance.matches_binding(
            branch.frames()[1].frame(),
            slot.outlet(),
            slot.slot_id(),
            slot.slot(),
            slot.slot_claim(),
            slot.anchor(),
            slot.anchor_claim(),
        ));
        assert!(!allowance.matches_binding(
            slot.parent(),
            slot.outlet(),
            branch.frames()[0].frame().slot(),
            slot.slot(),
            slot.slot_claim(),
            slot.anchor(),
            slot.anchor_claim(),
        ));
    }

    #[test]
    fn mount_new_staging_allowance_rejects_stale_request() {
        let (mut app, branch, _, _, _) = failed_nested_branch_fixture();
        let (router, request) = {
            let navigation = app
                .world()
                .resource::<crate::navigation::RouterNavigation>();
            (
                navigation.entries[0].handle,
                navigation.entries[0].requests[0],
            )
        };
        let stale = app
            .world_mut()
            .resource_mut::<crate::navigation::RouterNavigation>()
            .push(
                router,
                bevy_ui_router_core::prelude::RouteLocation::new("/stale"),
            )
            .expect("same router can retain a later request identity");
        let slot = &branch.child_slots()[0];
        let allowance = SealedMountNewStagingAllowance::for_test(
            request,
            slot.parent().clone(),
            slot.outlet().clone(),
            slot.slot_id(),
            slot.slot(),
            slot.slot_claim().clone(),
            slot.anchor(),
            slot.anchor_claim().clone(),
            slot.expected_anchor_children().to_vec(),
        );

        assert!(allowance.matches_request(request));
        assert!(!allowance.matches_request(stale));
    }
}
