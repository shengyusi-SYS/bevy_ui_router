#[cfg(test)]
use bevy_ui_router_core::frame::RouteFrameVersion;
use bevy_ui_router_core::integration::{RetainedLeafOwner, RetainedLeafProjection};

use crate::branch_transaction::RealizedBranch;

pub(crate) enum PreparedPublication {
    Ordinary(bevy_ui_router_core::integration::PreparedCommit),
    Retained(bevy_ui_router_core::integration::PreparedRetainedCommit),
}

impl PreparedPublication {
    pub(crate) fn validate_for_back(
        self,
        engine: &mut bevy_ui_router_core::integration::RouterEngine,
        continuation: bevy_ui_router_core::integration::LifecycleContinuationCapacity,
    ) -> Result<
        ValidatedPublication<'_>,
        (
            bevy_ui_router_core::integration::PreparedRetainedCommit,
            bevy_ui_router_core::integration::LifecycleContinuationCapacity,
            bevy_ui_router_core::integration::RetainedPublicationError,
        ),
    > {
        let Self::Retained(prepared) = self else {
            unreachable!("retention-enabled Back uses a retained publication partition")
        };
        engine
            .validate_retained_publication_for_back(prepared, continuation)
            .map(ValidatedPublication::Retained)
    }

    pub(crate) fn validate(
        self,
        engine: &mut bevy_ui_router_core::integration::RouterEngine,
    ) -> Result<
        ValidatedPublication<'_>,
        (
            Option<bevy_ui_router_core::integration::PreparedRetainedCommit>,
            bevy_ui_router_core::integration::IntegrationError,
        ),
    > {
        match self {
            Self::Ordinary(prepared) => engine
                .validate_publication(prepared)
                .map(ValidatedPublication::Ordinary)
                .map_err(|error| (None, error)),
            Self::Retained(prepared) => engine
                .validate_retained_publication(prepared)
                .map(ValidatedPublication::Retained)
                .map_err(|(prepared, error)| (Some(prepared), error)),
        }
    }
}

pub(crate) enum ValidatedPublication<'a> {
    Ordinary(bevy_ui_router_core::integration::ValidatedCommit<'a>),
    Retained(bevy_ui_router_core::integration::ValidatedRetainedCommit<'a>),
}

impl ValidatedPublication<'_> {
    pub(crate) fn history_publication(
        &self,
    ) -> &bevy_ui_router_core::integration::HistoryPublicationFacts {
        match self {
            Self::Ordinary(commit) => commit.history_publication(),
            Self::Retained(commit) => commit.history_publication(),
        }
    }

    pub(crate) fn cleanup(&self) -> &bevy_ui_router_core::integration::CleanupBatch {
        match self {
            Self::Ordinary(commit) => commit.cleanup(),
            Self::Retained(commit) => commit.cleanup(),
        }
    }

    pub(crate) fn publish(self, mutation: impl FnOnce(Option<RetainedLeafOwner>)) {
        match self {
            Self::Ordinary(commit) => commit.publish(|| mutation(None)),
            Self::Retained(commit) => commit.publish(mutation),
        }
    }
}

/// The physical payload and the sole Core-issued lifetime capability stay
/// together outside a request. An owner is temporarily absent only while the
/// Core publication holder owns its exact retirement transfer.
pub(crate) struct RetainedLeaf {
    projection: RetainedLeafProjection,
    owner: Option<RetainedLeafOwner>,
    branch: RealizedBranch,
}

#[derive(Default)]
pub(crate) struct RetainedLeaves {
    entries: Vec<RetainedLeaf>,
}

/// One exact selection and its transfer storage. Preparation keeps the original
/// entries in the cache; only Core acceptance permits physical handoff.
pub(crate) struct PreparedRetainedChange {
    indices: Vec<usize>,
    hit: Option<usize>,
    removals: Vec<usize>,
    owners: Vec<RetainedLeafOwner>,
    payloads: Vec<RealizedBranch>,
}

pub(crate) struct RetainedPayloads {
    pub(crate) hit: Option<RealizedBranch>,
    pub(crate) retiring: Vec<RealizedBranch>,
}

impl PreparedRetainedChange {
    pub(crate) fn selected<'a>(
        &'a self,
        leaves: &'a RetainedLeaves,
    ) -> impl Iterator<Item = (&'a RetainedLeafProjection, &'a RealizedBranch)> {
        self.indices.iter().map(move |index| {
            let entry = &leaves.entries[*index];
            (&entry.projection, &entry.branch)
        })
    }

    pub(crate) fn selected_mut<'a>(
        &'a self,
        leaves: &'a mut RetainedLeaves,
    ) -> impl Iterator<Item = (&'a RetainedLeafProjection, &'a mut RealizedBranch)> {
        leaves
            .entries
            .iter_mut()
            .enumerate()
            .filter_map(move |(index, entry)| {
                self.indices
                    .binary_search(&index)
                    .is_ok()
                    .then_some((&entry.projection, &mut entry.branch))
            })
    }

    /// The cache cannot change membership between preparation and handoff.
    /// Rejection returns these same capabilities through restore_owners.
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

    pub(crate) fn into_payloads(mut self, leaves: &mut RetainedLeaves) -> Vec<RealizedBranch> {
        for index in self.removals.into_iter().rev() {
            self.payloads.push(leaves.entries.remove(index).branch);
        }
        self.payloads
    }

    pub(crate) fn into_hit_and_payloads(mut self, leaves: &mut RetainedLeaves) -> RetainedPayloads {
        let mut hit = None;
        for index in self.removals.into_iter().rev() {
            let branch = leaves.entries.remove(index).branch;
            if Some(index) == self.hit {
                hit = Some(branch);
            } else {
                self.payloads.push(branch);
            }
        }
        RetainedPayloads {
            hit,
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
        self.prepare_change_with_hit(retiring, None, reserve_addition)
    }

    pub(crate) fn prepare_change_with_hit(
        &mut self,
        retiring: &[RetainedLeafProjection],
        hit: Option<&RetainedLeafProjection>,
        reserve_addition: bool,
    ) -> Option<PreparedRetainedChange> {
        let mut indices = Vec::new();
        indices.try_reserve_exact(retiring.len()).ok()?;
        for projection in retiring {
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
        indices.sort_unstable();
        let hit = if let Some(projection) = hit {
            let mut matches = self.entries.iter().enumerate().filter(|(_, entry)| {
                &entry.projection == projection
                    && entry
                        .owner
                        .as_ref()
                        .is_some_and(|owner| owner.projection() == projection)
            });
            let (index, _) = matches.next()?;
            if matches.next().is_some() || indices.binary_search(&index).is_ok() {
                return None;
            }
            Some(index)
        } else {
            None
        };
        let transfer_count = indices.len().checked_add(usize::from(hit.is_some()))?;
        let mut removals = Vec::new();
        removals.try_reserve_exact(transfer_count).ok()?;
        removals.extend(indices.iter().copied());
        removals.extend(hit);
        removals.sort_unstable();
        let mut owners = Vec::new();
        owners.try_reserve_exact(transfer_count).ok()?;
        let mut payloads = Vec::new();
        payloads.try_reserve_exact(indices.len()).ok()?;
        if reserve_addition {
            self.entries.try_reserve(1).ok()?;
        }
        Some(PreparedRetainedChange {
            indices,
            hit,
            removals,
            owners,
            payloads,
        })
    }

    pub(crate) fn branch_for(
        &self,
        projection: &RetainedLeafProjection,
    ) -> Option<&RealizedBranch> {
        self.entries
            .iter()
            .find(|entry| {
                &entry.projection == projection
                    && entry
                        .owner
                        .as_ref()
                        .is_some_and(|owner| owner.projection() == projection)
            })
            .map(|entry| &entry.branch)
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

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
    pub(crate) fn branch(&self, frame: &RouteFrameVersion) -> Option<&RealizedBranch> {
        self.entries
            .iter()
            .find(|entry| entry.projection.frame() == frame)
            .map(|entry| &entry.branch)
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
        branch: RealizedBranch,
    ) {
        self.entries.push(RetainedLeaf {
            projection,
            owner,
            branch,
        });
    }
}
