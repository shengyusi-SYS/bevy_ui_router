use bevy_ui_router_core::integration::{RetainedLeafId, RetainedLeafOwner, RetainedLeafProjection};

use crate::branch_transaction::RealizedBranch;

pub(crate) enum PreparedNormalPublication {
    Ordinary(bevy_ui_router_core::integration::PreparedCommit),
    Retained(bevy_ui_router_core::integration::PreparedRetainedCommit),
}

impl PreparedNormalPublication {
    pub(crate) fn validate(
        self,
        engine: &mut bevy_ui_router_core::integration::RouterEngine,
    ) -> Result<
        ValidatedNormalPublication<'_>,
        (
            Option<bevy_ui_router_core::integration::PreparedRetainedCommit>,
            bevy_ui_router_core::integration::IntegrationError,
        ),
    > {
        match self {
            Self::Ordinary(prepared) => engine
                .validate_publication(prepared)
                .map(ValidatedNormalPublication::Ordinary)
                .map_err(|error| (None, error)),
            Self::Retained(prepared) => engine
                .validate_retained_publication(prepared)
                .map(ValidatedNormalPublication::Retained)
                .map_err(|(prepared, error)| (Some(prepared), error)),
        }
    }
}

pub(crate) enum ValidatedNormalPublication<'a> {
    Ordinary(bevy_ui_router_core::integration::ValidatedCommit<'a>),
    Retained(bevy_ui_router_core::integration::ValidatedRetainedCommit<'a>),
}

impl ValidatedNormalPublication<'_> {
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

/// Storage and exact entry indices are prepared before Core installs Ready.
/// The caller consumes this only in the same publication that transfers the
/// corresponding original owners to Core.
pub(crate) struct PreparedRetiredPayloads {
    indices: Vec<usize>,
    payloads: Vec<RealizedBranch>,
}

pub(crate) struct RetainedSettlementStorage {
    branches: Vec<RealizedBranch>,
}

/// Keeps the original physical holder intact until Core accepts the idle
/// settlement. A rejected begin restores this same holder and its owners.
pub(crate) struct RetainedSettlementPayload {
    leaves: RetainedLeaves,
    branches: Vec<RealizedBranch>,
}

impl RetainedSettlementPayload {
    pub(crate) fn restore(self) -> RetainedLeaves {
        self.leaves
    }

    pub(crate) fn into_branches(mut self) -> Vec<RealizedBranch> {
        for entry in self.leaves.entries {
            self.branches.push(entry.branch);
        }
        self.branches
    }
}

impl RetainedLeaves {
    pub(crate) fn prepare_settlement(
        &self,
        retiring: &[RetainedLeafProjection],
    ) -> Option<RetainedSettlementStorage> {
        if retiring.len() != self.entries.len()
            || retiring.iter().enumerate().any(|(index, projection)| {
                retiring[index + 1..].contains(projection)
                    || self
                        .entries
                        .iter()
                        .filter(|entry| &entry.projection == projection && entry.owner.is_some())
                        .count()
                        != 1
            })
        {
            return None;
        }
        let mut branches = Vec::new();
        branches.try_reserve_exact(retiring.len()).ok()?;
        Some(RetainedSettlementStorage { branches })
    }

    pub(crate) fn take_settlement(
        &mut self,
        storage: RetainedSettlementStorage,
    ) -> RetainedSettlementPayload {
        RetainedSettlementPayload {
            leaves: std::mem::take(self),
            branches: storage.branches,
        }
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

    pub(crate) fn branch(&self, id: RetainedLeafId) -> Option<&RealizedBranch> {
        self.entries
            .iter()
            .find(|entry| entry.projection.id() == id)
            .map(|entry| &entry.branch)
    }

    pub(crate) fn branch_mut(&mut self, id: RetainedLeafId) -> Option<&mut RealizedBranch> {
        self.entries
            .iter_mut()
            .find(|entry| entry.projection.id() == id)
            .map(|entry| &mut entry.branch)
    }

    pub(crate) fn reserve_addition(&mut self) -> Result<(), ()> {
        self.entries.try_reserve(1).map_err(|_| ())
    }

    /// Validate the whole transfer and reserve its storage before moving any
    /// capability. An unsuccessful preparation leaves every owner in place.
    pub(crate) fn take_retiring_owners(
        &mut self,
        retiring: &[RetainedLeafProjection],
    ) -> Result<Vec<RetainedLeafOwner>, ()> {
        let mut indices = Vec::new();
        indices.try_reserve_exact(retiring.len()).map_err(|_| ())?;
        for projection in retiring {
            let index = self
                .entries
                .iter()
                .position(|entry| &entry.projection == projection && entry.owner.is_some())
                .ok_or(())?;
            if indices.contains(&index) {
                return Err(());
            }
            indices.push(index);
        }
        let mut owners = Vec::new();
        owners.try_reserve_exact(indices.len()).map_err(|_| ())?;
        for index in indices {
            owners.push(
                self.entries[index]
                    .owner
                    .take()
                    .expect("transfer was fully checked"),
            );
        }
        Ok(owners)
    }

    pub(crate) fn restore_owners(
        &mut self,
        owners: Vec<RetainedLeafOwner>,
    ) -> Result<(), Vec<RetainedLeafOwner>> {
        if owners.iter().enumerate().any(|(index, owner)| {
            owners[index + 1..]
                .iter()
                .any(|other| owner.projection().id() == other.projection().id())
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

    pub(crate) fn prepare_retired_payloads(
        &self,
        retiring: &[RetainedLeafProjection],
    ) -> Option<PreparedRetiredPayloads> {
        let mut indices = Vec::new();
        indices.try_reserve_exact(retiring.len()).ok()?;
        for projection in retiring {
            let index = self
                .entries
                .iter()
                .position(|entry| &entry.projection == projection && entry.owner.is_some())?;
            if indices.contains(&index) {
                return None;
            }
            indices.push(index);
        }
        let mut payloads = Vec::new();
        payloads.try_reserve_exact(indices.len()).ok()?;
        indices.sort_unstable_by(|left, right| right.cmp(left));
        Some(PreparedRetiredPayloads { indices, payloads })
    }

    pub(crate) fn take_retired_payloads(
        &mut self,
        mut prepared: PreparedRetiredPayloads,
    ) -> Vec<RealizedBranch> {
        for index in prepared.indices {
            prepared.payloads.push(self.entries.remove(index).branch);
        }
        prepared.payloads
    }
}
