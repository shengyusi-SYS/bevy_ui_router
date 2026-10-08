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
    empty_child_slots: Vec<LogicalChildSlotBinding>,
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
    pub(crate) fn logical_empty_child_slots(&self) -> &[LogicalChildSlotBinding] {
        &self.empty_child_slots
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

/// Frozen transfer of the current host's inactive leaves to its foreground
/// authority-cleanup batch. It contains no lifecycle capability.
#[derive(Debug)]
pub struct RetainedAuthoritySettlementPlan {
    source: PreparedPublicationObservation,
    inventory: HostAdmissionUnavailableOwnerInventoryProjection,
    failed_base_roots: Option<Vec<RouteFrameVersion>>,
    logical_roots: Vec<RouteFrameVersion>,
    retiring: Vec<RetainedLeafProjection>,
}

impl RetainedAuthoritySettlementPlan {
    pub fn logical_roots(&self) -> &[RouteFrameVersion] {
        &self.logical_roots
    }

    pub fn retiring(&self) -> &[RetainedLeafProjection] {
        &self.retiring
    }

    pub(super) fn failed_base_roots(&self) -> Option<&[RouteFrameVersion]> {
        self.failed_base_roots.as_deref()
    }
}

/// Frozen Live inventory joining the authority half of an existing post-commit pair.
/// Metadata already owned by the normal batch remains bound to its original token.
#[derive(Debug)]
pub struct RetainedPostCommitAuthorityPlan {
    source: PreparedPublicationObservation,
    normal: CleanupBatchToken,
    logical_roots: Vec<RouteFrameVersion>,
    retiring: Vec<RetainedLeafProjection>,
}

impl RetainedPostCommitAuthorityPlan {
    pub fn logical_roots(&self) -> &[RouteFrameVersion] {
        &self.logical_roots
    }

    pub fn retiring(&self) -> &[RetainedLeafProjection] {
        &self.retiring
    }
}

pub(super) struct PostCommitRetainedTransfer {
    pub(super) plan: RetainedPostCommitAuthorityPlan,
    pub(super) owners: Vec<RetainedLeafOwner>,
}

/// A frozen hit before any cached content is moved or the fresh task is consumed.
#[derive(Debug)]
pub struct RetainedHitPlan {
    hit: RetainedLeafProjection,
    target_frames: Vec<RouteFrameVersion>,
    reconciliation: RouteReconciliationProjection,
    commit_plan: RetainedCommitPlan,
}

impl RetainedHitPlan {
    pub const fn hit(&self) -> &RetainedLeafProjection {
        &self.hit
    }
    pub const fn target_frame(&self) -> &RouteFrameVersion {
        self.hit.frame()
    }
    pub const fn commit_plan(&self) -> &RetainedCommitPlan {
        &self.commit_plan
    }
    pub(crate) const fn reconciliation_projection(&self) -> &RouteReconciliationProjection {
        &self.reconciliation
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct RetainedRegistryEntry {
    projection: RetainedLeafProjection,
    retirement: Option<CleanupBatchToken>,
}

impl RetainedRegistryEntry {
    pub(super) const fn projection(&self) -> &RetainedLeafProjection {
        &self.projection
    }

    pub(super) const fn retirement(&self) -> Option<&CleanupBatchToken> {
        self.retirement.as_ref()
    }

    pub(super) fn is_live(&self) -> bool {
        self.retirement.is_none()
    }
}

pub(super) fn owner_projections_match<'owner, 'expected>(
    owners: impl Iterator<Item = &'owner RetainedLeafOwner>,
    expected: impl Iterator<Item = &'expected RetainedLeafProjection>,
) -> bool {
    let mut observed = owners
        .map(RetainedLeafOwner::projection)
        .collect::<Vec<_>>();
    observed.sort_by_key(|projection| projection.id());
    observed.into_iter().eq(expected)
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
    pub(crate) fn prepare_retained_post_commit_authority_plan(
        &self,
        source: &CorePostCommitAuthorityPreparationSource,
        inventory: &[&RetainedLeafOwner],
    ) -> Result<RetainedPostCommitAuthorityPlan, CommitError> {
        if source.transfer.retained.is_some()
            || self.observe_post_commit_cleanup().receiver.as_ref()
                != Some(&source.transfer.receiver)
            || !self.retained_post_commit_source_is_current(&source.transfer)
            || !source.host.matches(
                self.router,
                &source.transfer.receiver.current_host,
                source.transfer.receiver.current_top_slot,
            )
        {
            return Err(CommitError::Stale);
        }
        let current = self.current.as_ref().ok_or(CommitError::Stale)?;
        let WorkStage::PostCommitAuthorityAccepted { normal, .. } = &current.stage else {
            return Err(CommitError::Stale);
        };
        if !owner_projections_match(
            inventory.iter().copied(),
            self.retained_leaves
                .iter()
                .filter(|entry| entry.is_live())
                .map(RetainedRegistryEntry::projection),
        ) {
            return Err(CommitError::InvalidManifest);
        }
        let retiring = self
            .retained_leaves
            .iter()
            .filter(|entry| entry.is_live())
            .map(|entry| entry.projection.clone())
            .collect::<Vec<_>>();
        let mut logical_roots = current
            .target_frames
            .iter()
            .rev()
            .cloned()
            .collect::<Vec<_>>();
        logical_roots.extend(retiring.iter().map(|projection| projection.frame.clone()));
        if logical_roots
            .iter()
            .enumerate()
            .any(|(index, frame)| logical_roots[..index].contains(frame))
        {
            return Err(CommitError::InvalidManifest);
        }
        let record_index = self
            .records
            .iter()
            .position(|record| record.id() == current.identity.request)
            .ok_or(CommitError::Stale)?;
        Ok(RetainedPostCommitAuthorityPlan {
            source: self.observe_prepared_publication_source(record_index),
            normal: normal.clone(),
            logical_roots,
            retiring,
        })
    }

    pub(super) fn retained_post_commit_source_is_current(
        &self,
        transfer: &CorePostCommitAuthorityTransferOwner,
    ) -> bool {
        let Some(current) = self.current.as_ref() else {
            return false;
        };
        let WorkStage::PostCommitAuthorityAccepted { normal, .. } = &current.stage else {
            return false;
        };
        if self.pending_retained_transfer.is_some()
            || self.retained_leaves != transfer.receiver.retained_registry
            || normal.source_projection() != transfer.normal_source
            || self
                .retained_leaves
                .iter()
                .any(|entry| match entry.retirement() {
                    Some(token) => token != normal,
                    None => !scope_survives(
                        &entry.projection,
                        &transfer.receiver.current_host,
                        self.state.active().frames(),
                        &self.active_child_slots,
                    ),
                })
        {
            return false;
        }
        let Some(retained) = &transfer.retained else {
            return true;
        };
        let Some(index) = self
            .records
            .iter()
            .position(|record| record.id() == current.identity.request)
        else {
            return false;
        };
        retained.plan.normal == *normal
            && self
                .observe_prepared_publication_source(index)
                .same_source(&retained.plan.source)
            && owner_projections_match(retained.owners.iter(), retained.plan.retiring.iter())
            && self
                .retained_leaves
                .iter()
                .filter(|entry| entry.is_live())
                .map(RetainedRegistryEntry::projection)
                .eq(retained.plan.retiring.iter())
    }

    fn retained_authority_settlement_source_eligible(&self) -> bool {
        self.current
            .as_ref()
            .is_some_and(|current| current.retired_host.is_none())
            && self.state.availability() == &RouterAvailability::Ready
            && self.action.is_none()
            && self.current_reconciliation_matches_active()
            && self.pending_retained_transfer.is_none()
            && self
                .retained_leaves
                .iter()
                .all(RetainedRegistryEntry::is_live)
            && !matches!(
                self.prepared_publication_authority,
                Some(PreparedPublicationAuthority::Rejected(_))
            )
    }

    fn retained_authority_settlement_eligible(&self) -> bool {
        self.retained_authority_settlement_source_eligible()
            && self.current.as_ref().is_some_and(|current| {
                matches!(current.attempt, AttemptKind::Normal(_))
                    && current.reconciliation.is_some()
                    && matches!(
                        current.stage,
                        WorkStage::Applying
                            | WorkStage::ReadyToCommit
                            | WorkStage::CommitSealed { .. }
                    )
            })
    }

    pub(crate) fn prepare_retained_authority_settlement_plan(
        &self,
        lease: &HostLease,
        inventory: &[&RetainedLeafOwner],
    ) -> Result<RetainedAuthoritySettlementPlan, BeginAuthoritySettlementErrorKind> {
        let (Some(host), Some(slot)) = (self.state.host(), self.state.top_slot()) else {
            return Err(BeginAuthoritySettlementErrorKind::Stale);
        };
        if !lease.matches(self.router, host, slot) {
            return Err(BeginAuthoritySettlementErrorKind::Foreign);
        }
        if !self.retained_authority_settlement_eligible() {
            return Err(BeginAuthoritySettlementErrorKind::WrongPhase);
        }
        self.freeze_retained_authority_settlement_plan(
            host,
            inventory,
            self.authority_settlement_logical_roots(),
            None,
        )
    }

    pub(crate) fn prepare_retained_failed_authority_settlement_plan(
        &self,
        lease: &HostLease,
        trigger: &FailedAuthoritySettlementTrigger,
        base_logical_roots: &[RouteFrameVersion],
        inventory: &[&RetainedLeafOwner],
    ) -> Result<RetainedAuthoritySettlementPlan, BeginAuthoritySettlementErrorKind> {
        let (Some(host), Some(slot)) = (self.state.host(), self.state.top_slot()) else {
            return Err(BeginAuthoritySettlementErrorKind::Stale);
        };
        if !lease.matches(self.router, host, slot) {
            return Err(BeginAuthoritySettlementErrorKind::Foreign);
        }
        if !self.retained_authority_settlement_source_eligible() {
            return Err(BeginAuthoritySettlementErrorKind::WrongPhase);
        }
        self.inspect_failed_authority_trigger(trigger)?;
        if !trigger.terminal.accepts_logical_roots(base_logical_roots) {
            return Err(BeginAuthoritySettlementErrorKind::InvalidManifest);
        }
        self.freeze_retained_authority_settlement_plan(
            host,
            inventory,
            base_logical_roots.to_vec(),
            Some(base_logical_roots.to_vec()),
        )
    }

    fn freeze_retained_authority_settlement_plan(
        &self,
        host: &HostVersion,
        inventory: &[&RetainedLeafOwner],
        mut logical_roots: Vec<RouteFrameVersion>,
        failed_base_roots: Option<Vec<RouteFrameVersion>>,
    ) -> Result<RetainedAuthoritySettlementPlan, BeginAuthoritySettlementErrorKind> {
        if !owner_projections_match(
            inventory.iter().copied(),
            self.retained_leaves
                .iter()
                .map(RetainedRegistryEntry::projection),
        ) {
            return Err(BeginAuthoritySettlementErrorKind::InvalidManifest);
        }
        let mut retiring = Vec::new();
        for entry in &self.retained_leaves {
            if entry.projection.host() == host {
                if !scope_survives(
                    &entry.projection,
                    host,
                    self.state.active().frames(),
                    &self.active_child_slots,
                ) {
                    return Err(BeginAuthoritySettlementErrorKind::InvalidManifest);
                }
                retiring.push(entry.projection.clone());
            }
        }
        logical_roots.extend(retiring.iter().map(|projection| projection.frame().clone()));
        if logical_roots
            .iter()
            .enumerate()
            .any(|(index, frame)| logical_roots[..index].contains(frame))
        {
            return Err(BeginAuthoritySettlementErrorKind::InvalidManifest);
        }
        let current = self
            .current
            .as_ref()
            .expect("the authority source is current");
        let record_index = self
            .records
            .iter()
            .position(|record| record.id() == current.identity.request)
            .ok_or(BeginAuthoritySettlementErrorKind::Stale)?;
        Ok(RetainedAuthoritySettlementPlan {
            source: self.observe_prepared_publication_source(record_index),
            inventory: self.observed_unavailable_owner_inventory(),
            failed_base_roots,
            logical_roots,
            retiring,
        })
    }

    pub(crate) fn begin_authority_settlement_with_retention<F>(
        &mut self,
        issuer: &mut HostAdmissionIssuer,
        lease: HostLease,
        cause: PreCommitAuthorityCause,
        plan: &RetainedAuthoritySettlementPlan,
        seal: CleanupManifestSeal,
        owners: Vec<RetainedLeafOwner>,
        authority_mutation: F,
    ) -> Result<
        CleanupBatchToken,
        (
            HostLease,
            Vec<RetainedLeafOwner>,
            BeginAuthoritySettlementErrorKind,
        ),
    >
    where
        F: FnOnce(&Self, CleanupBatchToken),
    {
        self.begin_retained_authority_settlement_from_source(
            issuer,
            lease,
            None,
            cause,
            plan,
            seal,
            owners,
            authority_mutation,
        )
    }

    pub(crate) fn begin_failed_authority_settlement_with_retention<F>(
        &mut self,
        issuer: &mut HostAdmissionIssuer,
        lease: HostLease,
        trigger: FailedAuthoritySettlementTrigger,
        cause: PreCommitAuthorityCause,
        plan: &RetainedAuthoritySettlementPlan,
        seal: CleanupManifestSeal,
        owners: Vec<RetainedLeafOwner>,
        authority_mutation: F,
    ) -> Result<
        CleanupBatchToken,
        (
            HostLease,
            Vec<RetainedLeafOwner>,
            BeginAuthoritySettlementErrorKind,
        ),
    >
    where
        F: FnOnce(&Self, CleanupBatchToken),
    {
        self.begin_retained_authority_settlement_from_source(
            issuer,
            lease,
            Some(trigger),
            cause,
            plan,
            seal,
            owners,
            authority_mutation,
        )
    }

    fn begin_retained_authority_settlement_from_source<F>(
        &mut self,
        issuer: &mut HostAdmissionIssuer,
        lease: HostLease,
        trigger: Option<FailedAuthoritySettlementTrigger>,
        cause: PreCommitAuthorityCause,
        plan: &RetainedAuthoritySettlementPlan,
        seal: CleanupManifestSeal,
        owners: Vec<RetainedLeafOwner>,
        authority_mutation: F,
    ) -> Result<
        CleanupBatchToken,
        (
            HostLease,
            Vec<RetainedLeafOwner>,
            BeginAuthoritySettlementErrorKind,
        ),
    >
    where
        F: FnOnce(&Self, CleanupBatchToken),
    {
        let record_index = self.current.as_ref().and_then(|current| {
            self.records
                .iter()
                .position(|record| record.id() == current.identity.request)
        });
        let eligible = if trigger.is_some() {
            plan.failed_base_roots.is_some() && self.retained_authority_settlement_source_eligible()
        } else {
            plan.failed_base_roots.is_none() && self.retained_authority_settlement_eligible()
        };
        if !eligible
            || record_index.is_none_or(|index| {
                !self
                    .observe_prepared_publication_source(index)
                    .same_source(&plan.source)
            })
            || self.observed_unavailable_owner_inventory() != plan.inventory
        {
            return Err((lease, owners, BeginAuthoritySettlementErrorKind::Stale));
        }
        if seal.logical_roots() != plan.logical_roots()
            || !owner_projections_match(owners.iter(), plan.retiring.iter())
        {
            return Err((
                lease,
                owners,
                BeginAuthoritySettlementErrorKind::InvalidManifest,
            ));
        }
        let failed = match trigger {
            Some(trigger) => match self.consume_failed_authority_trigger(trigger) {
                Ok(source) => Some(source),
                Err(kind) => return Err((lease, owners, kind)),
            },
            None => None,
        };
        match self.begin_authority_settlement_publication(
            issuer,
            lease,
            cause,
            seal,
            failed,
            Some(plan),
            authority_mutation,
        ) {
            Ok(cleanup) => {
                // The accepted exact batch now owns these lifecycle obligations.
                // CurrentWork retains only its cloneable token, never the owners.
                drop(owners);
                Ok(cleanup)
            }
            Err(error) => {
                let kind = error.kind();
                Err((error.into_owner(), owners, kind))
            }
        }
    }

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
        if !owner_projections_match(
            inventory.iter().copied(),
            self.retained_leaves
                .iter()
                .map(RetainedRegistryEntry::projection),
        ) {
            return Err(CommitError::InvalidManifest);
        }
        let current = self.current.as_ref().ok_or(CommitError::Stale)?;
        let mut plan = self.retained_commit_partition(current, None)?;
        self.issue_retained_candidate(&mut plan)?;
        Ok(plan)
    }

    fn retained_commit_partition(
        &self,
        current: &CurrentWork,
        hit: Option<&RetainedLeafProjection>,
    ) -> Result<RetainedCommitPlan, CommitError> {
        let next_slots = publication_slots(current);
        let host = self.state.host().ok_or(CommitError::Stale)?;
        let mut retiring = Vec::new();
        let mut preserved = Vec::new();
        for entry in &self.retained_leaves {
            if hit.is_some_and(|hit| hit == &entry.projection) {
                continue;
            }
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
            .position(|record| record.id() == current.identity.request)
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

    fn issue_retained_candidate(
        &mut self,
        plan: &mut RetainedCommitPlan,
    ) -> Result<(), CommitError> {
        if let Some(candidate) = &mut plan.retained {
            let sequence = self.next_retained_instance;
            self.next_retained_instance = sequence.checked_add(1).ok_or(CommitError::Stale)?;
            candidate.id = RetainedLeafId {
                router: self.router,
                sequence,
            };
        }
        Ok(())
    }

    pub(crate) fn prepare_retained_hit_plan(
        &mut self,
        task: &FrameTask,
        inventory: &[&RetainedLeafOwner],
    ) -> Result<Option<RetainedHitPlan>, CommitError> {
        self.inspect_view_report_identity(task.identity())
            .map_err(|_| CommitError::Stale)?;
        if self.pending_retained_transfer.is_some()
            || self.prepared_publication_authority.is_some()
            || !self.current_reconciliation_matches_active()
            || self.retained_leaves.iter().any(|entry| !entry.is_live())
        {
            return Err(CommitError::Stale);
        }
        if !owner_projections_match(
            inventory.iter().copied(),
            self.retained_leaves
                .iter()
                .map(RetainedRegistryEntry::projection),
        ) {
            return Err(CommitError::InvalidManifest);
        }
        if !self.retained_hit_task_eligible(task) {
            return Ok(None);
        }
        let mut hits = self
            .retained_leaves
            .iter()
            .map(RetainedRegistryEntry::projection)
            .filter(|projection| self.retained_hit_matches(task, projection));
        let Some(hit) = hits.next().cloned() else {
            return Ok(None);
        };
        if hits.next().is_some() {
            return Err(CommitError::InvalidManifest);
        }
        let next = self.retained_hit_next(task, &hit)?;
        let mut commit_plan = self.retained_commit_partition(&next, Some(&hit))?;
        self.issue_retained_candidate(&mut commit_plan)?;
        Ok(Some(RetainedHitPlan {
            hit,
            target_frames: next.target_frames,
            reconciliation: next
                .reconciliation
                .expect("a hit has a matched reconciliation"),
            commit_plan,
        }))
    }

    fn retained_hit_task_eligible(&self, task: &FrameTask) -> bool {
        let Some(current) = self.current.as_ref() else {
            return false;
        };
        let Some(branch) = current.branch.as_ref() else {
            return false;
        };
        let Some(projection) = current.reconciliation.as_ref() else {
            return false;
        };
        let [entering] = projection.entering() else {
            return false;
        };
        let index = task.identity.branch_index;
        let Some(matched) = branch.routes().get(index) else {
            return false;
        };
        let fragment_only = self.state.active().location().is_some_and(|active| {
            active.path() == task.location.path()
                && active.search() == task.location.search()
                && active.fragment() != task.location.fragment()
        }) && self
            .active_branch
            .as_ref()
            .is_some_and(|active| active.branch_fingerprint() == branch.branch_fingerprint());
        self.keep_alive_capacity > 0
            && matches!(current.attempt, AttemptKind::Normal(_))
            && self.action.is_none()
            && current.stage == WorkStage::Applying
            && current.pending_mount.is_none()
            && current.applied_frames.is_empty()
            && index > 0
            && index + 1 == branch.routes().len()
            && projection.preserved().len() == index
            && entering.branch_index() == index
            && entering.slot() == ProjectedEnteringSlot::Existing(task.frame.slot())
            && entering.frame() == task.frame.frame()
            && entering.generation() == task.frame.generation()
            && current.target_frames.get(index) == Some(&task.frame)
            && current.current_target == task.target
            && current.location == task.location
            && task.matched.as_ref().map(MatchedRoute::segment_reuse_key)
                == Some(matched.segment_reuse_key())
            && self.matcher.view_binding(matched.route()) == Some(&task.view)
            && self.matcher.keep_alive(matched.route())
            && !fragment_only
    }

    fn retained_hit_matches(&self, task: &FrameTask, hit: &RetainedLeafProjection) -> bool {
        let Some(current) = self.current.as_ref() else {
            return false;
        };
        let Some(host) = self.state.host() else {
            return false;
        };
        let Some(matched) = task.matched.as_ref() else {
            return false;
        };
        let Some(parent) = current
            .target_frames
            .get(task.identity.branch_index.saturating_sub(1))
        else {
            return false;
        };
        hit.id.router == self.router
            && hit.frame.origin() == &FrameOrigin::MatchedRoute(matched.route())
            && &hit.parent == parent
            && hit.outlet == *matched.slot()
            && hit.slot == task.frame.slot()
            && &hit.host == host
            && hit.segment_reuse_key == *matched.segment_reuse_key()
            && hit.view == task.view
            && hit.location.search() == task.location.search()
            && hit.location.fragment() == task.location.fragment()
            && scope_survives(
                hit,
                host,
                &current.target_frames,
                &publication_slots(current),
            )
    }

    fn retained_hit_next(
        &self,
        task: &FrameTask,
        hit: &RetainedLeafProjection,
    ) -> Result<CurrentWork, CommitError> {
        if !self.retained_hit_task_eligible(task) || !self.retained_hit_matches(task, hit) {
            return Err(CommitError::Stale);
        }
        let mut next = self
            .current
            .as_ref()
            .and_then(CurrentWork::clone_pre_retirement)
            .ok_or(CommitError::Stale)?;
        next.reconciliation = Some(
            next.reconciliation
                .as_ref()
                .ok_or(CommitError::Stale)?
                .retarget_retained_leaf(&hit.frame, &hit.empty_child_slots)
                .ok_or(CommitError::InvalidManifest)?,
        );
        next.target_frames[task.identity.branch_index] = hit.frame.clone();
        next.identity.frame = hit.frame.frame();
        next.identity.generation = hit.frame.generation();
        next.identity.origin = hit.frame.origin().clone();
        next.applied_frames.push(hit.frame.clone());
        next.stage = WorkStage::ReadyToCommit;
        Ok(next)
    }

    pub(crate) fn prepare_retained_hit_publication(
        &mut self,
        task: FrameTask,
        plan: &RetainedHitPlan,
        seal: CleanupManifestSeal,
        owners: Vec<RetainedLeafOwner>,
    ) -> Result<
        (PreparedPublication, CleanupBatchToken),
        (FrameTask, Vec<RetainedLeafOwner>, CommitError),
    > {
        let record_index = match self
            .records
            .iter()
            .position(|record| record.id() == task.identity.request)
        {
            Some(index) => index,
            None => return Err((task, owners, CommitError::Stale)),
        };
        if self.pending_retained_transfer.is_some()
            || self.prepared_publication_authority.is_some()
            || self.inspect_view_report_identity(task.identity()).is_err()
            || !self
                .observe_prepared_publication_source(record_index)
                .same_source(&plan.commit_plan.source)
        {
            return Err((task, owners, CommitError::Stale));
        }
        let next = match self.retained_hit_next(&task, &plan.hit) {
            Ok(next) => next,
            Err(error) => return Err((task, owners, error)),
        };
        if next.target_frames != plan.target_frames
            || next.reconciliation.as_ref() != Some(&plan.reconciliation)
            || !self.retained_plan_partition_matches(&plan.commit_plan, &next, Some(&plan.hit))
        {
            return Err((task, owners, CommitError::Stale));
        }
        let mut expected = plan
            .commit_plan
            .retiring
            .iter()
            .chain(std::iter::once(&plan.hit))
            .collect::<Vec<_>>();
        expected.sort_by_key(|projection| projection.id());
        if !owner_projections_match(owners.iter(), expected.into_iter())
            || seal.logical_roots() != plan.commit_plan.disposable_roots()
        {
            return Err((task, owners, CommitError::InvalidManifest));
        }
        let mut registry = self
            .retained_leaves
            .iter()
            .filter(|entry| entry.projection.id != plan.hit.id)
            .cloned()
            .collect::<Vec<_>>();
        if let Some(projection) = &plan.commit_plan.retained {
            registry.push(RetainedRegistryEntry {
                projection: projection.clone(),
                retirement: None,
            });
        }
        let retained = plan
            .commit_plan
            .retained
            .clone()
            .map(|projection| RetainedLeafOwner { projection });
        let mut next_record = self.records[record_index].clone();
        next_record
            .transition(RequestStatus::Running(NavigationPhase::ReadyToSwitch))
            .expect("the task's request is open");
        next_record
            .append_trace(NavigationTraceEvent::request_phase_changed(
                NavigationPhase::ReadyToSwitch,
            ))
            .expect("the task's trace is open");
        next_record
            .push_diagnostic(DiagnosticEvent::reported(
                task.identity.request,
                task.frame.clone(),
                ReportDisposition::Accepted,
                "accepted retained leaf handoff",
            ))
            .expect("the task's diagnostics are open");
        let ticket = CommitTicket {
            identity: next.identity.clone(),
        };
        let sealed = match self.seal_prepared_commit_partition(
            &ticket,
            seal,
            Some((&plan.commit_plan, registry)),
            next,
            Some(next_record),
        ) {
            Ok(sealed) => sealed,
            Err(error) => return Err((task, owners, error)),
        };
        self.report_dispositions.push(ReportDispositionRecord {
            identity: task.identity,
            disposition: ReportDisposition::Accepted,
            target: DiagnosticTarget::Request(ticket.identity.request),
        });
        let cleanup = self.hold_retained_transfer(&sealed, retained, owners);
        // The independent next receiver was fully validated by the seal. The
        // original task and cache owners now move once into the existing Ready.
        let publication = self.install_prepared_publication(sealed);
        Ok((publication, cleanup))
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
            empty_child_slots: self
                .active_child_slots
                .iter()
                .filter(|binding| binding.parent() == frame && binding.child().is_none())
                .cloned()
                .collect(),
        };
        scope_survives(&candidate, host, &current.target_frames, next_slots).then_some(candidate)
    }

    fn retained_plan_partition_is_current(&self, plan: &RetainedCommitPlan) -> bool {
        let Some(current) = self.current.as_ref() else {
            return false;
        };
        self.retained_plan_partition_matches(plan, current, None)
    }

    fn retained_plan_partition_matches(
        &self,
        plan: &RetainedCommitPlan,
        current: &CurrentWork,
        hit: Option<&RetainedLeafProjection>,
    ) -> bool {
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
            .filter(|projection| hit != Some(*projection))
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
        if !owner_projections_match(retiring.iter(), plan.retiring.iter())
            || self.pending_retained_transfer.is_some()
        {
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
        self.prepare_retained_transfer(sealed, retained, retiring)
    }

    fn prepare_retained_transfer(
        &mut self,
        sealed: SealedCommit,
        retained: Option<RetainedLeafOwner>,
        retiring: Vec<RetainedLeafOwner>,
    ) -> Result<(PreparedPublication, CleanupBatchToken), (Vec<RetainedLeafOwner>, CommitError)>
    {
        let cleanup = self.hold_retained_transfer(&sealed, retained, retiring);
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

    fn hold_retained_transfer(
        &mut self,
        sealed: &SealedCommit,
        retained: Option<RetainedLeafOwner>,
        retiring: Vec<RetainedLeafOwner>,
    ) -> CleanupBatchToken {
        let cleanup = sealed.cleanup().clone();
        self.pending_retained_transfer = Some(PreparedRetainedTransfer {
            evidence: sealed.evidence.clone(),
            retained,
            retiring,
        });
        cleanup
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
        retiring: &[RetainedLeafProjection],
        cleanup: &CleanupBatchToken,
    ) {
        for entry in registry {
            if retiring
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

pub(super) fn scope_survives(
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
