#[cfg(test)]
use bevy_ui_router_core::frame::RouteFrameVersion;
use bevy_ui_router_core::integration::{RetainedLeafOwner, RetainedLeafProjection};

use crate::instance::InstancePayload;

/// The physical payload and the sole Core-issued lifetime capability stay
/// together outside a request. An owner is temporarily absent only while the
/// Core publication holder owns its exact retirement transfer.
pub(crate) struct RetainedLeaf {
    projection: RetainedLeafProjection,
    owner: Option<RetainedLeafOwner>,
    payload: InstancePayload,
}

#[derive(Default)]
pub(crate) struct RetainedLeaves {
    entries: Vec<RetainedLeaf>,
}

/// One exact selection and its transfer storage. Preparation keeps the original
/// entries in the cache; only Core acceptance permits physical handoff.
pub(crate) struct PreparedRetainedChange {
    indices: Vec<usize>,
    activated: Vec<usize>,
    removals: Vec<usize>,
    owners: Vec<RetainedLeafOwner>,
    payloads: Vec<InstancePayload>,
    activated_payloads: Vec<InstancePayload>,
    retiring_receivers: Vec<Option<InstancePayload>>,
    activated_receivers: Vec<Option<InstancePayload>>,
}

pub(crate) struct RetainedPayloads {
    pub(crate) activated: Vec<InstancePayload>,
    pub(crate) retiring: Vec<InstancePayload>,
}

impl PreparedRetainedChange {
    pub(crate) fn selected<'a>(
        &'a self,
        leaves: &'a RetainedLeaves,
    ) -> impl Iterator<Item = (&'a RetainedLeafProjection, &'a InstancePayload)> {
        self.indices.iter().map(move |index| {
            let entry = &leaves.entries[*index];
            (&entry.projection, &entry.payload)
        })
    }

    pub(crate) fn selected_in_storage_order<'a>(
        &'a self,
        leaves: &'a RetainedLeaves,
    ) -> impl Iterator<Item = (&'a RetainedLeafProjection, &'a InstancePayload)> {
        leaves
            .entries
            .iter()
            .enumerate()
            .filter_map(move |(index, entry)| {
                self.indices
                    .contains(&index)
                    .then_some((&entry.projection, &entry.payload))
            })
    }

    pub(crate) fn selected_mut<'a>(
        &'a self,
        leaves: &'a mut RetainedLeaves,
    ) -> impl Iterator<Item = (&'a RetainedLeafProjection, &'a mut InstancePayload)> {
        leaves
            .entries
            .iter_mut()
            .enumerate()
            .filter_map(move |(index, entry)| {
                self.indices
                    .contains(&index)
                    .then_some((&entry.projection, &mut entry.payload))
            })
    }

    /// Cache membership is frozen through the accepted handoff. Rejection
    /// restores these same capabilities without moving any physical payload.
    pub(crate) fn take_owners(&mut self, leaves: &mut RetainedLeaves) -> Vec<RetainedLeafOwner> {
        for index in &self.removals {
            self.owners.push(
                leaves.entries[*index]
                    .owner
                    .take()
                    .expect("prepared retained entry still owns its capability"),
            );
        }
        std::mem::take(&mut self.owners)
    }

    pub(crate) fn into_payloads(self, leaves: &mut RetainedLeaves) -> Vec<InstancePayload> {
        debug_assert!(self.activated.is_empty());
        self.into_activated_and_payloads(leaves).retiring
    }

    /// All receivers and output vectors were reserved while entries still
    /// held their original owners. Consume by exact prepared index, then
    /// deliver in the Core plan's dependency/target order rather than Vec order.
    pub(crate) fn into_activated_and_payloads(
        mut self,
        leaves: &mut RetainedLeaves,
    ) -> RetainedPayloads {
        for index in self.removals.into_iter().rev() {
            let payload = leaves.entries.remove(index).payload;
            if let Some(position) = self
                .activated
                .iter()
                .position(|selected| *selected == index)
            {
                self.activated_receivers[position] = Some(payload);
            } else {
                let position = self
                    .indices
                    .iter()
                    .position(|selected| *selected == index)
                    .expect("every removal has one prepared destination");
                self.retiring_receivers[position] = Some(payload);
            }
        }
        for receiver in &mut self.activated_receivers {
            self.activated_payloads.push(
                receiver
                    .take()
                    .expect("activated original payload received"),
            );
        }
        for receiver in &mut self.retiring_receivers {
            self.payloads
                .push(receiver.take().expect("retiring original payload received"));
        }
        RetainedPayloads {
            activated: self.activated_payloads,
            retiring: self.payloads,
        }
    }
}

impl RetainedLeaves {
    pub(crate) fn prepare_all(
        &mut self,
        retiring: &[RetainedLeafProjection],
    ) -> Option<PreparedRetainedChange> {
        if retiring.len() != self.entries.len() {
            return None;
        }
        self.prepare_change(retiring, false)
    }

    pub(crate) fn prepare_change(
        &mut self,
        retiring: &[RetainedLeafProjection],
        reserve_addition: bool,
    ) -> Option<PreparedRetainedChange> {
        self.prepare_change_with_selected(retiring, &[], usize::from(reserve_addition))
    }

    pub(crate) fn prepare_change_with_selected(
        &mut self,
        retiring: &[RetainedLeafProjection],
        activated: &[RetainedLeafProjection],
        addition_count: usize,
    ) -> Option<PreparedRetainedChange> {
        let locate = |projections: &[RetainedLeafProjection]| -> Option<Vec<usize>> {
            let mut indices = Vec::new();
            indices.try_reserve_exact(projections.len()).ok()?;
            for projection in projections {
                let mut matches = self.entries.iter().enumerate().filter(|(_, entry)| {
                    &entry.projection == projection
                        && entry
                            .owner
                            .as_ref()
                            .is_some_and(|owner| owner.projection() == projection)
                });
                let (index, _) = matches.next()?;
                if matches.next().is_some() || indices.contains(&index) {
                    return None;
                }
                indices.push(index);
            }
            Some(indices)
        };
        let indices = locate(retiring)?;
        let activated = locate(activated)?;
        if activated.iter().any(|index| indices.contains(index)) {
            return None;
        }
        let transfer_count = indices.len().checked_add(activated.len())?;
        let mut removals = Vec::new();
        removals.try_reserve_exact(transfer_count).ok()?;
        removals.extend(indices.iter().copied());
        removals.extend(activated.iter().copied());
        removals.sort_unstable();
        let mut owners = Vec::new();
        owners.try_reserve_exact(transfer_count).ok()?;
        let mut payloads = Vec::new();
        payloads.try_reserve_exact(indices.len()).ok()?;
        let mut activated_payloads = Vec::new();
        activated_payloads.try_reserve_exact(activated.len()).ok()?;
        let mut retiring_receivers = Vec::new();
        retiring_receivers.try_reserve_exact(indices.len()).ok()?;
        retiring_receivers.resize_with(indices.len(), || None);
        let mut activated_receivers = Vec::new();
        activated_receivers
            .try_reserve_exact(activated.len())
            .ok()?;
        activated_receivers.resize_with(activated.len(), || None);
        self.entries.try_reserve(addition_count).ok()?;
        Some(PreparedRetainedChange {
            indices,
            activated,
            removals,
            owners,
            payloads,
            activated_payloads,
            retiring_receivers,
            activated_receivers,
        })
    }

    pub(crate) fn payload_for(
        &self,
        projection: &RetainedLeafProjection,
    ) -> Option<&InstancePayload> {
        self.entries
            .iter()
            .find(|entry| {
                &entry.projection == projection
                    && entry
                        .owner
                        .as_ref()
                        .is_some_and(|owner| owner.projection() == projection)
            })
            .map(|entry| &entry.payload)
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(crate) fn payloads(&self) -> impl Iterator<Item = &InstancePayload> {
        self.entries.iter().map(|entry| &entry.payload)
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    pub(crate) fn inventory(&self) -> Option<Vec<&RetainedLeafOwner>> {
        self.entries
            .iter()
            .map(|entry| entry.owner.as_ref())
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn payload(&self, frame: &RouteFrameVersion) -> Option<&InstancePayload> {
        self.entries
            .iter()
            .find(|entry| entry.projection.frame() == frame)
            .map(|entry| &entry.payload)
    }

    pub(crate) fn restore_owners(
        &mut self,
        owners: Vec<RetainedLeafOwner>,
    ) -> Result<(), Vec<RetainedLeafOwner>> {
        if owners.iter().enumerate().any(|(index, owner)| {
            owners[index + 1..]
                .iter()
                .any(|other| owner.projection().frame() == other.projection().frame())
                || self
                    .entries
                    .iter()
                    .filter(|entry| {
                        entry.owner.is_none() && &entry.projection == owner.projection()
                    })
                    .count()
                    != 1
        }) {
            return Err(owners);
        }
        for owner in owners {
            let entry = self
                .entries
                .iter_mut()
                .find(|entry| &entry.projection == owner.projection())
                .expect("the complete owner return was checked before mutation");
            entry.owner = Some(owner);
        }
        Ok(())
    }

    /// The payload and optional capability come from the same validated Core
    /// plan. This is only the final, preallocated installation step.
    pub(crate) fn install_published(
        &mut self,
        projection: RetainedLeafProjection,
        owner: Option<RetainedLeafOwner>,
        payload: InstancePayload,
    ) {
        self.entries.push(RetainedLeaf {
            projection,
            owner,
            payload,
        });
    }
}
