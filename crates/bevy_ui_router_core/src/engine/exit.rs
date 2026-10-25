//! Deferred post-publication disposal. Application animation sessions are not
//! represented here; this owner only delays the original instance disposition.

use super::*;
use crate::cleanup::CleanupBatchIdentity;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CommitSettlementBinding {
    Cleanup(CleanupBatchToken),
    Exit(Box<CoreExitProjection>),
}

impl CommitSettlementBinding {
    pub(crate) fn batch(&self) -> CleanupBatchId {
        match self {
            Self::Cleanup(token) => token.batch(),
            Self::Exit(exit) => exit.batch,
        }
    }
}

impl From<CleanupBatchToken> for CommitSettlementBinding {
    fn from(token: CleanupBatchToken) -> Self {
        Self::Cleanup(token)
    }
}

impl PartialEq<CleanupBatchToken> for CommitSettlementBinding {
    fn eq(&self, token: &CleanupBatchToken) -> bool {
        matches!(self, Self::Cleanup(expected) if expected == token)
    }
}

/// Exact deferred disposition, independent of the later physical manifest.
/// This projection cannot construct the original exit or retained owners.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CoreExitProjection {
    pub(super) identity: RootTaskIdentity,
    pub(super) batch: CleanupBatchId,
    pub(super) settlement: CleanupBatchIdentity,
    pub(super) outgoing: Vec<RouteFrameVersion>,
    pub(super) target: Vec<RouteFrameVersion>,
    pub(super) cleanup_roots: Vec<RouteFrameVersion>,
    pub(super) retained: Vec<retained::RetainedLeafProjection>,
    pub(super) retiring: Vec<retained::RetainedLeafProjection>,
    pub(super) activated: Vec<retained::RetainedLeafProjection>,
    pub(super) host: HostVersion,
    pub(super) top_slot: RouteSlotId,
}

impl CoreExitProjection {
    pub(crate) const fn batch_identity(&self) -> &CleanupBatchIdentity {
        &self.settlement
    }

    pub(crate) fn outgoing(&self) -> &[RouteFrameVersion] {
        &self.outgoing
    }

    pub(crate) fn target(&self) -> &[RouteFrameVersion] {
        &self.target
    }

    pub(crate) fn retained(&self) -> &[retained::RetainedLeafProjection] {
        &self.retained
    }

    pub(crate) fn retiring(&self) -> &[retained::RetainedLeafProjection] {
        &self.retiring
    }

    pub(crate) fn cleanup_roots(&self) -> &[RouteFrameVersion] {
        &self.cleanup_roots
    }
}

#[derive(Debug)]
pub(crate) struct CoreDeferredExitOwner {
    projection: CoreExitProjection,
    reservation: PostCommitCleanupReservation,
    active: ActivePresentation,
    active_child_slots: Vec<LogicalChildSlotBinding>,
    history_revision: HistoryRevision,
    retained_registry: Vec<retained::RetainedRegistryEntry>,
    retained_owners: Vec<retained::RetainedLeafOwner>,
    retiring_owners: Vec<retained::RetainedLeafOwner>,
}

impl CoreDeferredExitOwner {
    pub(crate) const fn projection(&self) -> &CoreExitProjection {
        &self.projection
    }

    pub(crate) fn request(&self) -> NavigationRequestId {
        self.projection.identity.request
    }

    pub(crate) fn cleanup_roots(&self) -> &[RouteFrameVersion] {
        &self.projection.cleanup_roots
    }

    pub(crate) fn retained(&self) -> &[retained::RetainedLeafProjection] {
        &self.projection.retained
    }

    pub(crate) fn retiring(&self) -> &[retained::RetainedLeafProjection] {
        &self.projection.retiring
    }
}

pub(crate) struct CorePreparedExitCommit {
    publication: PublicationPlan,
    source: PreparedPublicationObservation,
    evidence: SealedCommitEvidence,
    owner: CoreDeferredExitOwner,
    activated_owners: Vec<retained::RetainedLeafOwner>,
}

impl std::fmt::Debug for CorePreparedExitCommit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CorePreparedExitCommit")
            .field("evidence", &self.evidence)
            .field("owner", &self.owner)
            .finish_non_exhaustive()
    }
}

impl CorePreparedExitCommit {
    pub(crate) fn request(&self) -> NavigationRequestId {
        self.owner.request()
    }

    pub(crate) fn history_execution(&self) -> Option<&HistoryExecutionBindingProjection> {
        self.evidence.history_execution.as_ref()
    }
    pub(crate) fn history_publication(&self) -> &HistoryPublicationFacts {
        &self.publication.history_publication
    }

    pub(crate) const fn projection(&self) -> &CoreExitProjection {
        self.owner.projection()
    }
}

pub(crate) struct CoreValidatedExitCommit<'a> {
    engine: &'a mut RouterEngine,
    prepared: CorePreparedExitCommit,
}

impl CoreValidatedExitCommit<'_> {
    pub(crate) fn history_publication(&self) -> &HistoryPublicationFacts {
        self.prepared.history_publication()
    }

    pub(crate) fn publish(self, install: impl FnOnce(CoreDeferredExitOwner)) {
        let CorePreparedExitCommit {
            publication,
            owner,
            activated_owners,
            source: _,
            evidence: _,
        } = self.prepared;
        // The activated original instances have moved into the prepared active
        // branch. Their inactive owner is consumed before publication begins.
        drop(activated_owners);
        self.engine
            .install_publication_plan(publication, || install(owner));
    }
}

#[derive(Debug)]
pub(crate) struct CoreExitPreparationError {
    pub(crate) plan: Option<retained::RetainedCommitPlan>,
    pub(crate) owners: Vec<retained::RetainedLeafOwner>,
    pub(crate) kind: CommitError,
}

#[derive(Debug)]
pub(crate) struct CoreExitCleanupBeginError {
    pub(crate) owner: CoreDeferredExitOwner,
    pub(crate) manifest: CleanupManifestSeal,
    pub(crate) kind: CommitError,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CoreExitReleaseDemand {
    Current,
    QueuedRequest(NavigationRequestId),
}

impl RouterEngine {
    pub(crate) fn cancel_prepared_exit_commit(
        &mut self,
        prepared: CorePreparedExitCommit,
    ) -> Result<Vec<retained::RetainedLeafOwner>, (CorePreparedExitCommit, CommitError)> {
        let exact = self.current.as_ref().is_some_and(|current| {
            current.identity == prepared.evidence.identity
                && matches!(&current.stage,
                    WorkStage::CommitSealed { token: CommitSettlementBinding::Exit(expected) }
                        if **expected == prepared.owner.projection)
        }) && self.current_history_capability.is_none();
        if !exact {
            return Err((prepared, CommitError::Stale));
        }
        let CorePreparedExitCommit {
            publication,
            owner,
            activated_owners,
            source: _,
            evidence: _,
        } = prepared;
        let mut original_owners = owner.retiring_owners;
        original_owners.extend(activated_owners);
        self.current_history_capability = publication
            .history
            .map(PreparedHistoryPublication::into_capability);
        self.current
            .as_mut()
            .expect("exact original exit preparation")
            .stage = WorkStage::ReadyToCommit;
        Ok(original_owners)
    }

    pub(crate) fn prepare_exit_commit(
        &mut self,
        ticket: &CommitTicket,
        plan: Option<retained::RetainedCommitPlan>,
        owners: Vec<retained::RetainedLeafOwner>,
    ) -> Result<CorePreparedExitCommit, CoreExitPreparationError> {
        let validation = (|| {
            self.inspect_commit_identity(ticket)?;
            if self.prepared_publication_authority.is_some()
                || self.pending_retained_transfer.is_some()
                || !self.current_reconciliation_matches_active()
            {
                return Err(CommitError::Stale);
            }
            let record_index = self
                .records
                .iter()
                .position(|record| record.id() == ticket.identity.request)
                .ok_or(CommitError::Stale)?;
            if let Some(plan) = &plan {
                if !self.prepared_publication_source_matches(record_index, &plan.source)
                    || !self.retained_plan_partition_is_current(plan)
                    || !retained::owner_projections_match(
                        owners.iter(),
                        plan.retiring().iter().chain(plan.activated()),
                    )
                {
                    return Err(CommitError::InvalidManifest);
                }
            } else if !owners.is_empty()
                || !self.retained_leaves.is_empty()
                || self
                    .current
                    .as_ref()
                    .is_some_and(|current| !self.retention_candidates(current).is_empty())
            {
                return Err(CommitError::InvalidManifest);
            }
            let current = self
                .current
                .as_ref()
                .and_then(CurrentWork::clone_pre_retirement)
                .ok_or(CommitError::Stale)?;
            if current.cleanup_roots.is_empty() {
                return Err(CommitError::InvalidManifest);
            }
            let host = self.state.host().cloned().ok_or(CommitError::Stale)?;
            let top_slot = self.state.top_slot().ok_or(CommitError::Stale)?;
            Ok((current, host, top_slot))
        })();
        let (mut current, host, top_slot) = match validation {
            Ok(source) => source,
            Err(kind) => return Err(CoreExitPreparationError { plan, owners, kind }),
        };
        let outgoing = current.cleanup_roots.clone();
        if let Some(plan) = &plan {
            current.cleanup_roots = plan.disposable_roots().to_vec();
        }
        let reservation = PostCommitCleanupReservation::new(
            current.identity.clone(),
            current.cleanup_batch,
            current.cleanup_roots.clone(),
        );
        let projection = CoreExitProjection {
            identity: current.identity.clone(),
            batch: current.cleanup_batch,
            settlement: reservation.identity().clone(),
            outgoing,
            target: current.target_frames.clone(),
            cleanup_roots: current.cleanup_roots.clone(),
            retained: plan
                .as_ref()
                .map(|plan| plan.retained().to_vec())
                .unwrap_or_default(),
            retiring: plan
                .as_ref()
                .map(|plan| plan.retiring().to_vec())
                .unwrap_or_default(),
            activated: plan
                .as_ref()
                .map(|plan| plan.activated().to_vec())
                .unwrap_or_default(),
            host,
            top_slot,
        };
        let history_capability = if matches!(current.attempt, AttemptKind::Normal(_)) {
            match self.current_history_capability.take() {
                Some(capability) => Some(capability),
                None => {
                    return Err(CoreExitPreparationError {
                        plan,
                        owners,
                        kind: CommitError::Stale,
                    });
                }
            }
        } else {
            None
        };
        let binding = CommitSettlementBinding::Exit(Box::new(projection.clone()));
        let mut publication = match self.precompute_disposition_publication(
            current,
            binding.clone(),
            history_capability,
            None,
        ) {
            Ok(publication) => publication,
            Err(error) => {
                self.pending_history_preparation_error = Some(error);
                let failure = self.begin_failure_cleanup(
                    PendingFailureKind::AuthorityLost,
                    NavigationFailure::new(
                        NavigationFailureKind::AuthorityLost,
                        "canonical history source or execution binding drifted",
                    ),
                );
                self.action = Some(EngineAction::FailureCleanup(failure));
                return Err(CoreExitPreparationError {
                    plan,
                    owners,
                    kind: CommitError::Stale,
                });
            }
        };
        let mut registry = self
            .retained_leaves
            .iter()
            .filter(|entry| !projection.activated.contains(entry.projection()))
            .cloned()
            .collect::<Vec<_>>();
        for entry in &mut registry {
            if projection.retiring.contains(entry.projection()) {
                entry.reserve_exit_retirement(projection.settlement.clone());
            }
        }
        let retained_owners = projection
            .retained
            .iter()
            .cloned()
            .map(retained::RetainedLeafOwner::prepared)
            .collect();
        let (activated_owners, retiring_owners) = owners
            .into_iter()
            .partition(|owner| projection.activated.contains(owner.projection()));
        publication.retained_update = Some(registry.clone());
        let owner = CoreDeferredExitOwner {
            projection,
            reservation,
            active: publication.state.active().clone(),
            active_child_slots: publication.active_child_slots.clone(),
            history_revision: publication.history.as_ref().map_or(
                self.history.revision(),
                PreparedHistoryPublication::target_revision,
            ),
            retained_registry: registry,
            retained_owners,
            retiring_owners,
        };
        let evidence = SealedCommitEvidence {
            identity: ticket.identity.clone(),
            cleanup_batch: owner.projection.batch,
            history_execution: publication
                .history
                .as_ref()
                .map(PreparedHistoryPublication::execution_projection),
            source_revision: publication
                .history
                .as_ref()
                .map(PreparedHistoryPublication::source_revision),
        };
        self.current
            .as_mut()
            .expect("validated original commit")
            .stage = WorkStage::CommitSealed { token: binding };
        let source = self.observe_prepared_publication_source(publication.record_index);
        Ok(CorePreparedExitCommit {
            publication,
            source,
            evidence,
            owner,
            activated_owners,
        })
    }

    pub(crate) fn validate_exit_publication(
        &mut self,
        prepared: CorePreparedExitCommit,
    ) -> Result<CoreValidatedExitCommit<'_>, (CorePreparedExitCommit, CommitError)> {
        let valid = self.validate_exit_publication_source(&prepared);
        if let Err(error) = valid {
            return Err((prepared, error));
        }
        Ok(CoreValidatedExitCommit {
            engine: self,
            prepared,
        })
    }

    fn validate_exit_publication_source(
        &self,
        prepared: &CorePreparedExitCommit,
    ) -> Result<(), CommitError> {
        self.validate_sealed_commit_evidence(&prepared.evidence)?;
        if !self.prepared_publication_source_matches(
            prepared.publication.record_index,
            &prepared.source,
        ) || !self.current_reconciliation_matches_active()
            || !matches!(self.current.as_ref().map(|current| &current.stage),
                Some(WorkStage::CommitSealed { token: CommitSettlementBinding::Exit(expected) })
                    if **expected == prepared.owner.projection)
        {
            return Err(CommitError::Stale);
        }
        let current = self.current.as_ref().ok_or(CommitError::Stale)?;
        let history_matches = match (
            &prepared.publication.history,
            &prepared.evidence.history_execution,
            prepared.evidence.source_revision,
        ) {
            (Some(history), Some(execution), Some(revision)) => {
                let branch = current.branch.as_ref().ok_or(CommitError::Stale)?;
                history.matches_canonical_source(&self.history)
                    && history.matches_disposition_projection(
                        execution,
                        revision,
                        &current.location,
                        prepared.publication.state.active(),
                        branch.branch_fingerprint(),
                        &current.target_frames,
                        &CommitSettlementBinding::Exit(Box::new(prepared.owner.projection.clone())),
                    )
            }
            (None, None, None) => true,
            _ => false,
        };
        history_matches.then_some(()).ok_or(CommitError::Stale)
    }

    fn exit_owner_is_current(&self, owner: &CoreDeferredExitOwner) -> bool {
        let expected = &owner.projection;
        self.current.as_ref().is_some_and(|current| {
            current.identity == expected.identity
                && matches!(&current.stage,
                    WorkStage::PostCommitExit { expected: current } if **current == *expected)
                && current.cleanup_roots == expected.cleanup_roots
                && current.target_frames == expected.target
        }) && self.state.host() == Some(&expected.host)
            && self.state.top_slot() == Some(expected.top_slot)
            && self.state.active() == &owner.active
            && self.active_child_slots == owner.active_child_slots
            && self.history.revision() == owner.history_revision
            && self.retained_leaves == owner.retained_registry
            && self.action.is_none()
    }

    pub(crate) fn exit_release_demand(
        &self,
        owner: &CoreDeferredExitOwner,
    ) -> Result<CoreExitReleaseDemand, CommitError> {
        if !self.exit_owner_is_current(owner) {
            return Err(CommitError::Stale);
        }
        Ok(self
            .queue
            .front()
            .map_or(CoreExitReleaseDemand::Current, |queued| {
                CoreExitReleaseDemand::QueuedRequest(queued.request)
            }))
    }

    pub(crate) fn begin_exit_cleanup(
        &mut self,
        owner: CoreDeferredExitOwner,
        manifest: CleanupManifestSeal,
        install: impl FnOnce(CleanupBatchToken, Vec<retained::RetainedLeafOwner>),
    ) -> Result<(), CoreExitCleanupBeginError> {
        if !self.exit_owner_is_current(&owner) {
            return Err(CoreExitCleanupBeginError {
                owner,
                manifest,
                kind: CommitError::Stale,
            });
        }
        if manifest.logical_roots() != owner.cleanup_roots() {
            return Err(CoreExitCleanupBeginError {
                owner,
                manifest,
                kind: CommitError::InvalidManifest,
            });
        }
        let CoreDeferredExitOwner {
            projection,
            reservation,
            active,
            active_child_slots,
            history_revision,
            mut retained_registry,
            retained_owners,
            retiring_owners,
        } = owner;
        let (token, logical_roots) = match reservation.seal(manifest) {
            Ok(sealed) => sealed,
            Err((reservation, manifest)) => {
                return Err(CoreExitCleanupBeginError {
                    owner: CoreDeferredExitOwner {
                        projection,
                        reservation,
                        active,
                        active_child_slots,
                        history_revision,
                        retained_registry,
                        retained_owners,
                        retiring_owners,
                    },
                    manifest,
                    kind: CommitError::InvalidManifest,
                });
            }
        };
        for entry in &mut retained_registry {
            entry.seal_exit_retirement(&projection.settlement, &token);
        }
        retained_registry.extend(
            projection
                .retained
                .iter()
                .cloned()
                .map(retained::RetainedRegistryEntry::live),
        );
        let stage = WorkStage::PostCommitCleanup {
            outstanding: token.clone(),
        };
        // All collections and the exact concrete token are prepared before the
        // physical installation callback. Original retired owners are consumed
        // only after their responsibility is covered by that actual token.
        drop(retiring_owners);
        install(token, retained_owners);
        self.retained_leaves = retained_registry;
        let current = self.current.as_mut().expect("exit receiver was validated");
        current.cleanup_roots = logical_roots;
        current.stage = stage;
        Ok(())
    }
}
