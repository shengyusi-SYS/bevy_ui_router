use bevy_ui_router_core::integration::{CommitTicket, RootFrameTask};

use crate::{
    mount::{ChildSlotResidual, MountedChildSlot},
    presentation::RealizedFrame,
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
        let anchor_is_exact = world
            .get_resource::<RouteEntityRegistry>()
            .and_then(|registry| registry.record(self.anchor))
            .is_some_and(|record| {
                record.claim() == self.anchor_claim && record.manifest().matches(world, self.anchor)
            });
        record.claim() == self.claim
            && record.manifest().matches(world, self.entity)
            && matches!(
                record.owner(),
                RegistryOwner::Committed(EntityOwner::ChildSlot { parent, outlet, slot })
                    if parent == &self.parent && outlet == &self.outlet && *slot == self.slot
            )
            && world.get::<RouteOwnershipSeal>(self.entity)
                == Some(&RouteOwnershipSeal::new(self.claim.seal()))
            && world.get::<ChildSlotLifecycle>(self.entity)
                == Some(&ChildSlotLifecycle::RegisteredInert)
            && world
                .get::<ChildSlotAuthority>(self.entity)
                .is_some_and(|authority| {
                    authority.parent() == &self.parent
                        && authority.outlet() == &self.outlet
                        && authority.slot() == self.slot
                })
            && world.get::<ChildOf>(self.entity).map(ChildOf::parent) == Some(self.anchor)
            && world.get::<Visibility>(self.entity) == Some(&Visibility::Hidden)
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

pub(crate) struct RealizedBranch {
    frames: Vec<RealizedFrame>,
    child_slots: Vec<MountedChildSlot>,
}

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

    pub(crate) fn child_slots(&self) -> &[MountedChildSlot] {
        &self.child_slots
    }

    #[cfg(feature = "test-support")]
    pub(crate) fn tracked_child_slots(&self) -> Vec<TrackedChildSlot> {
        self.child_slots
            .iter()
            .map(TrackedChildSlot::from_mounted)
            .collect()
    }

    pub(crate) fn push_frame(&mut self, frame: RealizedFrame) -> Result<(), RealizedFrame> {
        if self.frames.len() != self.child_slots.len() {
            return Err(frame);
        }
        if let Some(slot) = self.child_slots.last()
            && (frame.frame().slot() != slot.slot_id()
                || !frame.mount_binding().matches_mounted_child_slot(slot))
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
        let canonical = self.frames.last().and_then(|parent| {
            (parent.frame() == slot.parent())
                .then(|| parent.outlet(slot.outlet()))
                .flatten()
                .map(|outlet| {
                    (
                        outlet.anchor(),
                        outlet.anchor_claim().clone(),
                        parent.expected_owned_children(outlet.anchor()),
                    )
                })
        });
        if self.frames.len() != self.child_slots.len() + 1 {
            return Err(RejectedChildSlot {
                slot,
                canonical_anchor: canonical.as_ref().map(|(anchor, _, _)| *anchor),
                canonical_anchor_claim: canonical.as_ref().map(|(_, claim, _)| claim.clone()),
                canonical_anchor_children: canonical.map(|(_, _, children)| children),
            });
        }
        let Some(parent) = self.frames.last() else {
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
        if parent.frame() != slot.parent() || !parent_outlet_is_exact {
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

    pub(crate) fn last_frame(&self) -> Option<&RealizedFrame> {
        self.frames.last()
    }

    pub(crate) fn first_frame(&self) -> Option<&RealizedFrame> {
        self.frames.first()
    }

    pub(crate) fn interactive_entities(&self) -> Vec<bevy_ecs::prelude::Entity> {
        self.frames
            .iter()
            .flat_map(RealizedFrame::interactive_entities)
            .collect()
    }

    pub(crate) fn claims(&self) -> impl Iterator<Item = &crate::registry::ExactRegistryClaim> {
        self.frames.iter().flat_map(RealizedFrame::claims)
    }

    pub(crate) fn replace_frame_claims(
        &mut self,
        claims: &[crate::registry::ExactRegistryClaim],
    ) -> bool {
        let expected = self
            .frames
            .iter()
            .map(RealizedFrame::obligation_count)
            .sum::<usize>();
        if expected != claims.len() {
            return false;
        }
        let mut offset = 0;
        for frame in &mut self.frames {
            let next = offset + frame.obligation_count();
            frame.replace_claims(&claims[offset..next]);
            offset = next;
        }
        true
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

    pub(crate) fn last_frame(&self) -> Option<&RealizedFrame> {
        self.realized.last_frame()
    }

    pub(crate) fn first_frame(&self) -> Option<&RealizedFrame> {
        self.realized.first_frame()
    }

    pub(crate) fn into_realized(self) -> RealizedBranch {
        self.realized
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
}

impl FailedPreparedBranch {
    pub(crate) fn new(branch: PreparedBranch) -> Self {
        Self {
            realized: branch.into_realized(),
            mount_residuals: Vec::new(),
            detached_frames: Vec::new(),
            rejected_child_slots: Vec::new(),
        }
    }

    pub(crate) fn with_mount_residual(branch: PreparedBranch, residual: ChildSlotResidual) -> Self {
        Self {
            realized: branch.into_realized(),
            mount_residuals: vec![residual],
            detached_frames: Vec::new(),
            rejected_child_slots: Vec::new(),
        }
    }

    pub(crate) fn with_detached_frame(branch: PreparedBranch, frame: RealizedFrame) -> Self {
        Self {
            realized: branch.into_realized(),
            mount_residuals: Vec::new(),
            detached_frames: vec![frame],
            rejected_child_slots: Vec::new(),
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
        }
    }

    pub(crate) fn frames(&self) -> &[RealizedFrame] {
        self.realized.frames()
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
}
