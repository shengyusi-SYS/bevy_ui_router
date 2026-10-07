//! Linear transfer of a normal leaf out of the foreground request.
use super::*;
use crate::fingerprint::RouteSegmentReuseKey;

/// Opaque identity of one retained instance, scoped to its issuing Router.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RetainedLeafId {
    router: RouterIdentity,
    sequence: u64,
}

/// Read-only facts about a retained instance. A projection cannot recreate its owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetainedLeafProjection {
    id: RetainedLeafId,
    frame: RouteFrameVersion,
    parent: RouteFrameVersion,
    outlet: RouteSlotKey,
    slot: RouteSlotId,
    host: HostVersion,
    segment_reuse_key: RouteSegmentReuseKey,
    view: RouteViewKey,
    location: RouteLocation,
}

impl RetainedLeafProjection {
    pub const fn id(&self) -> RetainedLeafId {
        self.id
    }
    pub const fn frame(&self) -> &RouteFrameVersion {
        &self.frame
    }
    pub const fn parent(&self) -> &RouteFrameVersion {
        &self.parent
    }
    pub const fn outlet(&self) -> &RouteSlotKey {
        &self.outlet
    }
    pub const fn slot(&self) -> RouteSlotId {
        self.slot
    }
    pub const fn host(&self) -> &HostVersion {
        &self.host
    }
    pub const fn segment_reuse_key(&self) -> &RouteSegmentReuseKey {
        &self.segment_reuse_key
    }
    pub const fn view(&self) -> &RouteViewKey {
        &self.view
    }
    pub const fn location(&self) -> &RouteLocation {
        &self.location
    }
}

/// Unique long-lived responsibility for one inactive leaf. There is no public constructor.
///
/// ```compile_fail
/// use bevy_ui_router_core::integration::RetainedLeafOwner;
/// fn copy(owner: &RetainedLeafOwner) -> RetainedLeafOwner { owner.clone() }
/// ```
#[derive(Debug)]
pub struct RetainedLeafOwner {
    projection: RetainedLeafProjection,
}

impl RetainedLeafOwner {
    pub const fn projection(&self) -> &RetainedLeafProjection {
        &self.projection
    }
}

/// Frozen current/next partition. Only the issuing Core can turn it into a transfer.
#[derive(Debug)]
pub struct RetainedCommitPlan {
    source: PreparedPublicationObservation,
    original_roots: Vec<RouteFrameVersion>,
    next_slots: Vec<LogicalChildSlotBinding>,
    retained: Option<RetainedLeafProjection>,
    disposable_roots: Vec<RouteFrameVersion>,
    retiring: Vec<RetainedLeafProjection>,
    preserved: Vec<RetainedLeafProjection>,
}

impl RetainedCommitPlan {
    pub const fn retained(&self) -> Option<&RetainedLeafProjection> {
        self.retained.as_ref()
    }
    pub fn disposable_roots(&self) -> &[RouteFrameVersion] {
        &self.disposable_roots
    }
    pub fn retiring(&self) -> &[RetainedLeafProjection] {
        &self.retiring
    }
    pub fn preserved(&self) -> &[RetainedLeafProjection] {
        &self.preserved
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct RetainedRegistryEntry {
    projection: RetainedLeafProjection,
    retirement: Option<CleanupBatchToken>,
}

// Transit responsibility is independent of CurrentWork and survives discarded
// prepared handles and rejected publication. Neither this holder nor its owners is Clone.
pub(super) struct PreparedRetainedTransfer {
    evidence: SealedCommitEvidence,
    retained: Option<RetainedLeafOwner>,
    retiring: Vec<RetainedLeafOwner>,
}

pub(crate) struct ValidatedRetainedPublication<'a> {
    publication: ValidatedPublication<'a>,
}

impl ValidatedRetainedPublication<'_> {
    pub(crate) fn history_publication(&self) -> &HistoryPublicationFacts {
        self.publication.history_publication()
    }

    pub(crate) fn publish<F>(self, mutation: F)
    where
        F: FnOnce(Option<RetainedLeafOwner>),
    {
        let transfer = self
            .publication
            .engine
            .pending_retained_transfer
            .take()
            .expect("validated retention retains its unique transfer holder");
        self.publication
            .publish(move || mutation(transfer.retained));
        drop(transfer.retiring);
    }
}

impl RouterEngine {
    pub(crate) fn prepare_retained_commit_plan(
        &mut self,
        ticket: &CommitTicket,
        inventory: &[&RetainedLeafOwner],
    ) -> Result<RetainedCommitPlan, CommitError> {
        self.inspect_commit_identity(ticket)?;
        if self.pending_retained_transfer.is_some()
            || !self.current_reconciliation_matches_active()
            || self
                .retained_leaves
                .iter()
                .any(|entry| entry.retirement.is_some())
        {
            return Err(CommitError::Stale);
        }
        let mut observed = inventory
            .iter()
            .map(|owner| owner.projection.clone())
            .collect::<Vec<_>>();
        observed.sort_by_key(RetainedLeafProjection::id);
        let expected = self
            .retained_leaves
            .iter()
            .map(|entry| entry.projection.clone())
            .collect::<Vec<_>>();
        if observed != expected {
            return Err(CommitError::InvalidManifest);
        }
        let current = self.current.as_ref().ok_or(CommitError::Stale)?;
        let next_slots = publication_slots(current);
        let host = self.state.host().ok_or(CommitError::Stale)?;
        let mut retiring = Vec::new();
        let mut preserved = Vec::new();
        for entry in &self.retained_leaves {
            if scope_survives(&entry.projection, host, &current.target_frames, &next_slots) {
                preserved.push(entry.projection.clone());
            } else {
                retiring.push(entry.projection.clone());
            }
        }
        let retained = if preserved.len() < self.keep_alive_capacity {
            self.retention_candidate(current, &next_slots)
        } else {
            None
        };
        let retained = if let Some(mut candidate) = retained {
            let sequence = self.next_retained_instance;
            self.next_retained_instance = sequence.checked_add(1).ok_or(CommitError::Stale)?;
            candidate.id = RetainedLeafId {
                router: self.router,
                sequence,
            };
            Some(candidate)
        } else {
            None
        };
        let current = self.current.as_ref().ok_or(CommitError::Stale)?;
        let original_roots = current.cleanup_roots.clone();
        let mut disposable_roots = original_roots
            .iter()
            .filter(|frame| {
                retained
                    .as_ref()
                    .is_none_or(|retained| retained.frame() != *frame)
            })
            .cloned()
            .collect::<Vec<_>>();
        disposable_roots.extend(retiring.iter().map(|owner| owner.frame.clone()));
        if disposable_roots
            .iter()
            .enumerate()
            .any(|(index, frame)| disposable_roots[..index].contains(frame))
        {
            return Err(CommitError::InvalidManifest);
        }
        let record_index = self
            .records
            .iter()
            .position(|record| record.id() == ticket.identity.request)
            .ok_or(CommitError::Stale)?;
        Ok(RetainedCommitPlan {
            source: self.observe_prepared_publication_source(record_index),
            original_roots,
            next_slots,
            retained,
            disposable_roots,
            retiring,
            preserved,
        })
    }

    pub(super) fn retention_candidate(
        &self,
        current: &CurrentWork,
        next_slots: &[LogicalChildSlotBinding],
    ) -> Option<RetainedLeafProjection> {
        if self.keep_alive_capacity == 0
            || !matches!(current.attempt, AttemptKind::Normal(_))
            || self.state.active().kind() != ActivePresentationKind::Route
        {
            return None;
        }
        let frame = self.state.active().frames().last()?;
        let FrameOrigin::MatchedRoute(route) = frame.origin() else {
            return None;
        };
        if !self.matcher.keep_alive(*route)
            || !current.cleanup_roots.contains(frame)
            || self
                .active_child_slots
                .iter()
                .any(|binding| binding.parent() == frame && binding.child().is_some())
        {
            return None;
        }
        let mut parents = self
            .active_child_slots
            .iter()
            .filter(|binding| binding.child() == Some(frame));
        let binding = parents.next()?;
        if parents.next().is_some() {
            return None;
        }
        let host = self.state.host()?;
        let matched = self
            .active_branch
            .as_ref()?
            .routes()
            .iter()
            .find(|matched| matched.route() == *route)?;
        let candidate = RetainedLeafProjection {
            id: RetainedLeafId {
                router: self.router,
                sequence: 0,
            },
            frame: frame.clone(),
            parent: binding.parent().clone(),
            outlet: binding.outlet().clone(),
            slot: binding.slot(),
            host: host.clone(),
            segment_reuse_key: matched.segment_reuse_key().clone(),
            view: self.matcher.view_binding(*route)?.clone(),
            location: self.state.active().location()?.clone(),
        };
        scope_survives(&candidate, host, &current.target_frames, next_slots).then_some(candidate)
    }

    fn retained_plan_partition_is_current(&self, plan: &RetainedCommitPlan) -> bool {
        let Some(current) = self.current.as_ref() else {
            return false;
        };
        let Some(host) = self.state.host() else {
            return false;
        };
        let next_slots = publication_slots(current);
        if current.cleanup_roots != plan.original_roots || next_slots != plan.next_slots {
            return false;
        }
        // Rebuild the entire role partition from the current Core registry;
        // individually valid owners or a caller's retiring subset are insufficient.
        let (preserved, retiring): (Vec<_>, Vec<_>) = self
            .retained_leaves
            .iter()
            .map(|entry| &entry.projection)
            .partition(|projection| {
                scope_survives(projection, host, &current.target_frames, &next_slots)
            });
        if !preserved.iter().copied().eq(plan.preserved.iter())
            || !retiring.iter().copied().eq(plan.retiring.iter())
        {
            return false;
        }
        let mut candidate = if preserved.len() < self.keep_alive_capacity {
            self.retention_candidate(current, &next_slots)
        } else {
            None
        };
        if let (Some(candidate), Some(frozen)) = (&mut candidate, &plan.retained) {
            if frozen.id.router != self.router
                || frozen.id.sequence == 0
                || frozen.id.sequence >= self.next_retained_instance
            {
                return false;
            }
            candidate.id = frozen.id;
        }
        if candidate != plan.retained {
            return false;
        }
        let mut roots = current
            .cleanup_roots
            .iter()
            .filter(|frame| {
                candidate
                    .as_ref()
                    .is_none_or(|candidate| candidate.frame() != *frame)
            })
            .cloned()
            .collect::<Vec<_>>();
        roots.extend(
            retiring
                .into_iter()
                .map(|projection| projection.frame.clone()),
        );
        roots == plan.disposable_roots
    }

    pub(crate) fn prepare_retained_publication(
        &mut self,
        ticket: &CommitTicket,
        plan: &RetainedCommitPlan,
        seal: CleanupManifestSeal,
        retiring: Vec<RetainedLeafOwner>,
    ) -> Result<(PreparedPublication, CleanupBatchToken), (Vec<RetainedLeafOwner>, CommitError)>
    {
        let mut owned = retiring
            .iter()
            .map(|owner| owner.projection.clone())
            .collect::<Vec<_>>();
        owned.sort_by_key(RetainedLeafProjection::id);
        if owned != plan.retiring || self.pending_retained_transfer.is_some() {
            return Err((retiring, CommitError::InvalidManifest));
        }
        let record_index = match self
            .records
            .iter()
            .position(|record| record.id() == ticket.identity.request)
        {
            Some(index) => index,
            None => return Err((retiring, CommitError::Stale)),
        };
        if !self
            .observe_prepared_publication_source(record_index)
            .same_source(&plan.source)
            || !self.retained_plan_partition_is_current(plan)
        {
            return Err((retiring, CommitError::Stale));
        }
        if seal.logical_roots() != plan.disposable_roots.as_slice() {
            return Err((retiring, CommitError::InvalidManifest));
        }
        let mut next_registry = self.retained_leaves.clone();
        if let Some(projection) = &plan.retained {
            next_registry.push(RetainedRegistryEntry {
                projection: projection.clone(),
                retirement: None,
            });
        }
        let retained = plan
            .retained
            .clone()
            .map(|projection| RetainedLeafOwner { projection });
        let sealed = match self.seal_commit_partition(ticket, seal, Some((plan, next_registry))) {
            Ok(sealed) => sealed,
            Err(error) => return Err((retiring, error)),
        };
        let cleanup = sealed.cleanup().clone();
        self.pending_retained_transfer = Some(PreparedRetainedTransfer {
            evidence: sealed.evidence.clone(),
            retained,
            retiring,
        });
        match self.prepare_publication(sealed) {
            Ok(publication) => Ok((publication, cleanup)),
            Err(error) => {
                let transfer = self
                    .pending_retained_transfer
                    .take()
                    .expect("failed preparation returns the whole transfer");
                Err((transfer.retiring, error))
            }
        }
    }

    pub(crate) fn validate_retained_publication(
        &mut self,
        prepared: PreparedPublication,
    ) -> Result<ValidatedRetainedPublication<'_>, (PreparedPublication, CommitError)> {
        if !self
            .pending_retained_transfer
            .as_ref()
            .is_some_and(|transfer| transfer.evidence == prepared.evidence)
        {
            return Err((prepared, CommitError::Stale));
        }
        if let Err(error) = self.validate_publication_projection(&prepared) {
            return Err((prepared, error));
        }
        Ok(ValidatedRetainedPublication {
            publication: ValidatedPublication {
                engine: self,
                prepared,
            },
        })
    }

    pub(crate) fn cancel_prepared_retained_publication_for_authority_settlement(
        &mut self,
        prepared: PreparedPublication,
    ) -> Result<Vec<RetainedLeafOwner>, (PreparedPublication, CommitError)> {
        if !self
            .pending_retained_transfer
            .as_ref()
            .is_some_and(|transfer| transfer.evidence == prepared.evidence)
        {
            return Err((prepared, CommitError::Stale));
        }
        self.cancel_prepared_publication_for_authority_settlement(prepared)?;
        Ok(self
            .pending_retained_transfer
            .take()
            .expect("cancel returns its exact original owners")
            .retiring)
    }

    pub(crate) fn take_unpublished_retained_owners_for_settlement(
        &mut self,
    ) -> Result<Vec<RetainedLeafOwner>, CommitError> {
        if self.prepared_publication_authority.is_some() {
            return Err(CommitError::Stale);
        }
        self.pending_retained_transfer
            .take()
            .map(|transfer| transfer.retiring)
            .ok_or(CommitError::Stale)
    }

    pub(super) fn bind_retained_retirements(
        registry: &mut [RetainedRegistryEntry],
        plan: &RetainedCommitPlan,
        cleanup: &CleanupBatchToken,
    ) {
        for entry in registry {
            if plan
                .retiring
                .iter()
                .any(|retiring| retiring.id == entry.projection.id)
            {
                entry.retirement = Some(cleanup.clone());
            }
        }
    }

    pub(super) fn settle_retained_cleanup(&mut self, completed: &CleanupBatchToken) {
        self.retained_leaves
            .retain(|entry| entry.retirement.as_ref() != Some(completed));
    }

    #[cfg(test)]
    pub(crate) fn testing_retained_transfer_owners(&self) -> Vec<RetainedLeafProjection> {
        self.pending_retained_transfer
            .as_ref()
            .map(|transfer| {
                transfer
                    .retiring
                    .iter()
                    .map(|owner| owner.projection.clone())
                    .collect()
            })
            .unwrap_or_default()
    }
}

pub(super) fn publication_slots(current: &CurrentWork) -> Vec<LogicalChildSlotBinding> {
    match &current.attempt {
        AttemptKind::Normal(_) => reconciled_child_slots(current),
        AttemptKind::Fallback { bound_error, .. } => bound_error
            .as_ref()
            .map(|attempt| attempt.active_child_slots.clone())
            .unwrap_or_default(),
    }
}

fn scope_survives(
    projection: &RetainedLeafProjection,
    host: &HostVersion,
    frames: &[RouteFrameVersion],
    slots: &[LogicalChildSlotBinding],
) -> bool {
    &projection.host == host
        && frames.contains(&projection.parent)
        && slots
            .iter()
            .filter(|binding| {
                binding.parent() == &projection.parent
                    && binding.outlet() == &projection.outlet
                    && binding.slot() == projection.slot
            })
            .count()
            == 1
}
