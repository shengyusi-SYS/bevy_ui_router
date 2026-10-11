//! Physical ownership of one original frame and its own child slots.
//! Mount facts referring to a parent are aliases, never additional payload owners.
use bevy_ui_router_core::integration::RetainedLeafProjection;

use crate::{
    branch_transaction::{PreparedRootRebase, RealizedBranch},
    mount::MountedChildSlot,
    presentation::{
        FrameMutationTarget, RealizedFrame, child_slot_mutation_target, claim_rebind_is_exact,
    },
    registry::{EntityLifecycle, ExactRegistryClaim},
};

/// Borrowed physical facts only: this projection never reconstructs ownership.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PayloadRef<'a> {
    pub(crate) frames: &'a [RealizedFrame],
    pub(crate) child_slots: &'a [MountedChildSlot],
}

impl<'a> From<&'a RealizedBranch> for PayloadRef<'a> {
    fn from(branch: &'a RealizedBranch) -> Self {
        branch.as_payload_ref()
    }
}
impl<'a> From<&'a InstancePayload> for PayloadRef<'a> {
    fn from(instance: &'a InstancePayload) -> Self {
        instance.as_payload_ref()
    }
}
impl<'a> From<&PayloadRef<'a>> for PayloadRef<'a> {
    fn from(payload: &PayloadRef<'a>) -> Self {
        *payload
    }
}

impl<'a> PayloadRef<'a> {
    pub(crate) const fn frames(&self) -> &'a [RealizedFrame] {
        self.frames
    }
    pub(crate) const fn child_slots(&self) -> &'a [MountedChildSlot] {
        self.child_slots
    }
    pub(crate) fn prepare_root_rebase(
        &self,
        leaving: &RealizedBranch,
    ) -> Option<PreparedRootRebase> {
        Some(PreparedRootRebase {
            frame: self
                .first_frame()?
                .prepare_mount_rebase(leaving.first_frame()?)?,
        })
    }
    pub(crate) fn last_frame(&self) -> Option<&'a RealizedFrame> {
        self.frames.last()
    }

    pub(crate) fn first_frame(&self) -> Option<&'a RealizedFrame> {
        self.frames.first()
    }

    pub(crate) fn live_interactive_entities(
        &self,
        world: &bevy_ecs::world::World,
    ) -> Vec<bevy_ecs::prelude::Entity> {
        self.frames
            .iter()
            .flat_map(|frame| frame.live_interactive_entities(world))
            .collect()
    }

    pub(crate) fn claims(self) -> impl Iterator<Item = &'a crate::registry::ExactRegistryClaim> {
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

    pub(crate) fn entering_mutation_targets(
        &self,
        current: EntityLifecycle,
        next: EntityLifecycle,
    ) -> Vec<FrameMutationTarget> {
        self.entering_mutation_targets_excluding_slots(current, next, &[])
    }

    pub(crate) fn entering_mutation_targets_excluding_slots(
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
}

/// The unique physical owner of this frame and slots whose parent is this frame.
/// Its external parent mount remains in RealizedFrame as a read-only alias.
#[derive(Debug)]
pub(crate) struct InstancePayload {
    frame: RealizedFrame,
    own_slots: Vec<MountedChildSlot>,
}

pub(crate) struct PreparedInstanceClaimRebind {
    frame_claims: Vec<ExactRegistryClaim>,
    all_claims: Vec<ExactRegistryClaim>,
}

/// Storage reserved while all source payloads still remain with their owners.
#[derive(Debug)]
pub(crate) struct PreparedInstanceExtraction {
    own_slots: Vec<MountedChildSlot>,
}
impl PreparedInstanceExtraction {
    pub(crate) fn reserve(count: usize) -> Self {
        let mut own_slots = Vec::new();
        own_slots.reserve_exact(count);
        Self { own_slots }
    }
    pub(crate) fn receive_slot(&mut self, slot: MountedChildSlot) {
        self.own_slots.push(slot);
    }
    pub(crate) fn finish(self, frame: RealizedFrame) -> InstancePayload {
        InstancePayload {
            frame,
            own_slots: self.own_slots,
        }
    }
}

/// Receives original selected instances in target order. Both vectors are
/// allocated before acceptance; finish only moves already owned payloads.
#[derive(Debug)]
pub(crate) struct PreparedInstanceBranch {
    frames: Vec<RealizedFrame>,
    slots: Vec<MountedChildSlot>,
}
impl PreparedInstanceBranch {
    pub(crate) fn reserve_for(frame_count: usize, slot_count: usize) -> Self {
        let mut frames = Vec::new();
        frames.reserve_exact(frame_count);
        let mut slots = Vec::new();
        slots.reserve_exact(slot_count);
        Self { frames, slots }
    }
    pub(crate) fn finish_instances(mut self, instances: Vec<InstancePayload>) -> RealizedBranch {
        for instance in instances {
            let (frame, mut slots) = instance.into_owned_parts();
            self.frames.push(frame);
            self.slots.append(&mut slots);
        }
        RealizedBranch::from_owned_parts(self.frames, self.slots)
    }
}

impl InstancePayload {
    pub(crate) const fn frame(&self) -> &RealizedFrame {
        &self.frame
    }
    pub(crate) fn frames(&self) -> &[RealizedFrame] {
        std::slice::from_ref(&self.frame)
    }
    pub(crate) fn child_slots(&self) -> &[MountedChildSlot] {
        &self.own_slots
    }
    pub(crate) fn as_payload_ref(&self) -> PayloadRef<'_> {
        PayloadRef {
            frames: self.frames(),
            child_slots: &self.own_slots,
        }
    }
    pub(crate) fn claims(&self) -> impl Iterator<Item = &ExactRegistryClaim> {
        self.frame
            .claims()
            .chain(self.own_slots.iter().map(MountedChildSlot::slot_claim))
    }
    pub(crate) fn obligation_count(&self) -> usize {
        self.frame.obligation_count() + self.own_slots.len()
    }
    pub(crate) fn live_interactive_entities(
        &self,
        world: &bevy_ecs::world::World,
    ) -> Vec<bevy_ecs::entity::Entity> {
        self.frame.live_interactive_entities(world)
    }
    #[cfg(feature = "bsn")]
    pub(crate) fn prepare_native_cleanup(
        &mut self,
        world: &mut bevy_ecs::world::World,
    ) -> Result<(), crate::presentation::native_frame::NativeCleanupPreparationError> {
        crate::presentation::native_frame::prepare_content_cleanup(world, &mut self.frame)
    }
    pub(crate) fn lifecycle_transitions(
        &self,
        next: EntityLifecycle,
        leaf_to_root: bool,
    ) -> Vec<(ExactRegistryClaim, EntityLifecycle)> {
        self.as_payload_ref()
            .lifecycle_transitions(next, leaf_to_root)
    }
    pub(crate) fn retained_leaving_mutation_targets(&self) -> Option<Vec<FrameMutationTarget>> {
        if self
            .frame
            .claims()
            .any(|claim| claim.lifecycle() != EntityLifecycle::Retained)
            || self.own_slots.iter().any(|slot| {
                slot.parent() != self.frame.frame()
                    || slot.slot_claim().lifecycle() != EntityLifecycle::Retained
            })
        {
            return None;
        }
        let mut targets = self
            .own_slots
            .iter()
            .map(|slot| {
                child_slot_mutation_target(
                    slot,
                    EntityLifecycle::Retained,
                    EntityLifecycle::Leaving,
                    false,
                )
            })
            .collect::<Vec<_>>();
        targets.extend(self.frame.mutation_targets(
            EntityLifecycle::Retained,
            EntityLifecycle::Leaving,
            false,
        ));
        Some(targets)
    }
    pub(crate) fn prepare_retained_claim_rebind(
        &self,
        claims: &[ExactRegistryClaim],
        expected_next: EntityLifecycle,
    ) -> Option<PreparedInstanceClaimRebind> {
        if self
            .own_slots
            .iter()
            .any(|slot| slot.parent() != self.frame.frame())
            || claims.iter().enumerate().any(|(index, claim)| {
                claims[index + 1..]
                    .iter()
                    .any(|other| other.entity() == claim.entity())
            })
        {
            return None;
        }
        let exact_projection = |source: &ExactRegistryClaim| -> Option<ExactRegistryClaim> {
            claims
                .iter()
                .find(|next| next.entity() == source.entity())
                .filter(|next| {
                    claim_rebind_is_exact(source, next, EntityLifecycle::Retained, expected_next)
                })
                .cloned()
        };
        let frame_claims = self
            .frame
            .claims()
            .map(exact_projection)
            .collect::<Option<Vec<_>>>()?;
        if self
            .own_slots
            .iter()
            .any(|slot| exact_projection(slot.slot_claim()).is_none())
        {
            return None;
        }
        // A parent-owned alias may remain Active/PreparedInert when absent from
        // this own write set. Only aliases actually included in the plan rebind.
        let alias_is_exact = |alias: &ExactRegistryClaim| {
            claims
                .iter()
                .find(|next| next.entity() == alias.entity())
                .is_none_or(|next| {
                    alias.entity() == next.entity()
                        && alias.owner() == next.owner()
                        && alias.seal() == next.seal()
                        && next.lifecycle() == expected_next
                })
        };
        if !alias_is_exact(self.frame.mount_binding().slot_claim())
            || self
                .frame
                .mount_binding()
                .physical_parent()
                .1
                .is_some_and(|claim| !alias_is_exact(claim))
            || self
                .frame
                .outlets()
                .iter()
                .any(|outlet| !alias_is_exact(outlet.anchor_claim()))
            || self
                .own_slots
                .iter()
                .any(|slot| !alias_is_exact(slot.anchor_claim()))
        {
            return None;
        }
        Some(PreparedInstanceClaimRebind {
            frame_claims,
            all_claims: claims.to_vec(),
        })
    }
    pub(crate) fn apply_claim_rebind(&mut self, prepared: PreparedInstanceClaimRebind) {
        self.frame
            .apply_claim_rebind(&prepared.frame_claims, &prepared.all_claims);
        for slot in &mut self.own_slots {
            slot.apply_internal_claim_rebind(&prepared.all_claims);
        }
    }
    pub(crate) fn into_owned_parts(self) -> (RealizedFrame, Vec<MountedChildSlot>) {
        (self.frame, self.own_slots)
    }
    pub(crate) fn apply_activation(
        &mut self,
        activation: crate::presentation::PreparedRetainedFrameActivation,
    ) {
        self.frame.apply_retained_activation(activation);
    }

    #[cfg(test)]
    pub(crate) fn from_frame_for_test(frame: RealizedFrame) -> Self {
        Self {
            frame,
            own_slots: Vec::new(),
        }
    }
}

pub(crate) fn validate_retained_entering_payload(
    world: &bevy_ecs::world::World,
    entering: PayloadRef<'_>,
    projection: &bevy_ui_router_core::integration::RetainedLeafProjection,
) -> Result<(), crate::frame_local::FrameLocalError> {
    let [frame] = entering.frames() else {
        return Err(crate::frame_local::FrameLocalError::OwnershipDrift(
            entering
                .frames()
                .first()
                .map_or(bevy_ecs::entity::Entity::PLACEHOLDER, |frame| frame.shell()),
        ));
    };
    if world
        .get_resource::<crate::registry::RouteEntityRegistry>()
        .is_some_and(|registry| {
            registry.retained_payload_is_current(world, projection, frame, entering.child_slots())
        })
    {
        Ok(())
    } else {
        Err(crate::frame_local::FrameLocalError::OwnershipDrift(
            frame.shell(),
        ))
    }
}

pub(crate) fn validate_retained_entering_payload_with_fresh(
    world: &bevy_ecs::world::World,
    entering: PayloadRef<'_>,
    projection: &RetainedLeafProjection,
    fresh: PayloadRef<'_>,
) -> Result<(), crate::frame_local::FrameLocalError> {
    let [frame] = entering.frames() else {
        return Err(crate::frame_local::FrameLocalError::OwnershipDrift(
            bevy_ecs::entity::Entity::PLACEHOLDER,
        ));
    };
    if world
        .get_resource::<crate::registry::RouteEntityRegistry>()
        .is_some_and(|registry| {
            registry.retained_payload_is_current_with_fresh(
                world,
                projection,
                frame,
                entering.child_slots(),
                fresh,
            )
        })
    {
        Ok(())
    } else {
        Err(crate::frame_local::FrameLocalError::OwnershipDrift(
            frame.shell(),
        ))
    }
}
