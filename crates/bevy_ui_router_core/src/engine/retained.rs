//! Instance selection, independent inventory, and linear lifecycle transfers.
use super::instances::{InstanceFacts, RetainedOwnSlotProjection, TargetInstanceSourceKind};
use super::*;
use crate::cleanup::CleanupBatchIdentity;
use crate::fingerprint::RouteSegmentReuseKey;

/// Read-only facts about an original instance. A projection cannot recreate its owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetainedLeafProjection {
    instance: InstanceFacts,
    segment_reuse_key: RouteSegmentReuseKey,
    view: RouteViewKey,
    location: RouteLocation,
    own_slots: Vec<RetainedOwnSlotProjection>,
}

impl RetainedLeafProjection {
    pub(crate) fn compare_instance(&self, other: &Self) -> std::cmp::Ordering {
        self.instance.compare_identity(&other.instance)
    }
    pub const fn frame(&self) -> &RouteFrameVersion {
        &self.instance.frame
    }
    pub const fn parent(&self) -> &RouteFrameVersion {
        &self.instance.parent
    }
    pub const fn outlet(&self) -> &RouteSlotKey {
        &self.instance.outlet
    }
    pub const fn slot(&self) -> RouteSlotId {
        self.instance.slot
    }
    pub const fn host(&self) -> &HostVersion {
        &self.instance.host
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
    pub fn own_slots(&self) -> &[RetainedOwnSlotProjection] {
        &self.own_slots
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
    pub(super) fn prepared(projection: RetainedLeafProjection) -> Self {
        Self { projection }
    }

    pub const fn projection(&self) -> &RetainedLeafProjection {
        &self.projection
    }
}

#[derive(Debug)]
pub(crate) struct RetainedInvalidationContinuation {
    identity: FrameTaskIdentity,
    invalid: RouteFrameVersion,
    completed: CleanupBatchToken,
    source: PreparedPublicationObservation,
}
impl PartialEq for RetainedInvalidationContinuation {
    fn eq(&self, other: &Self) -> bool {
        self.identity == other.identity
            && self.invalid == other.invalid
            && self.completed == other.completed
            && self.source.same_source(&other.source)
    }
}
impl Eq for RetainedInvalidationContinuation {}
impl RetainedInvalidationContinuation {
    pub(super) fn new(
        identity: FrameTaskIdentity,
        invalid: RouteFrameVersion,
        completed: CleanupBatchToken,
        source: PreparedPublicationObservation,
    ) -> Self {
        Self {
            identity,
            invalid,
            completed,
            source,
        }
    }
    pub(crate) fn invalid(&self) -> &RouteFrameVersion {
        &self.invalid
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetainedInvalidationResumeFailure {
    Stale,
    InvalidTarget,
    IdentityExhausted,
}
pub(crate) enum CoreRetainedInvalidationReceiver {
    FrameTask(FrameTask),
    Commit(CommitTicket),
    AfterFailure(RetainedInvalidationContinuation),
}
#[derive(Clone, Copy)]
pub(crate) enum CoreRetainedInvalidationReceiverRef<'a> {
    FrameTask(&'a FrameTask),
    Commit(&'a CommitTicket),
    AfterFailure(&'a RetainedInvalidationContinuation),
}
impl<'a> CoreRetainedInvalidationReceiverRef<'a> {
    fn identity(self) -> &'a FrameTaskIdentity {
        match self {
            Self::FrameTask(task) => task.identity(),
            Self::Commit(ticket) => ticket.identity(),
            Self::AfterFailure(owner) => &owner.identity,
        }
    }
}
impl CoreRetainedInvalidationReceiver {
    fn as_ref(&self) -> CoreRetainedInvalidationReceiverRef<'_> {
        match self {
            Self::FrameTask(task) => CoreRetainedInvalidationReceiverRef::FrameTask(task),
            Self::Commit(ticket) => CoreRetainedInvalidationReceiverRef::Commit(ticket),
            Self::AfterFailure(owner) => CoreRetainedInvalidationReceiverRef::AfterFailure(owner),
        }
    }
    fn identity(&self) -> &FrameTaskIdentity {
        self.as_ref().identity()
    }
}

/// The original receiver is unavailable until its selected-instance cleanup settles.
pub(crate) struct RetainedMissCleanupOwner {
    receiver: CoreRetainedInvalidationReceiver,
    retained: Vec<RetainedLeafOwner>,
    cleanup: CleanupBatchToken,
    source: PreparedPublicationObservation,
    resume: Option<(usize, Vec<ReservedFrameAttempt>)>,
}

impl std::fmt::Debug for RetainedMissCleanupOwner {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RetainedMissCleanupOwner")
            .field("receiver", &self.receiver.identity())
            .field("retained", &self.retained)
            .field("cleanup", &self.cleanup)
            .finish()
    }
}

impl RetainedMissCleanupOwner {
    pub(crate) const fn cleanup(&self) -> &CleanupBatchToken {
        &self.cleanup
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetainedMissCleanupAuthorityFailCloseErrorKind {
    SourceMismatch,
    ProtocolRejected,
}

/// Frozen current/next partition. Only the issuing Core can turn it into a transfer.
#[derive(Debug)]
pub struct RetainedCommitPlan {
    pub(super) source: PreparedPublicationObservation,
    original_roots: Vec<RouteFrameVersion>,
    next_slots: Vec<LogicalChildSlotBinding>,
    retained: Vec<RetainedLeafProjection>,
    activated: Vec<RetainedLeafProjection>,
    disposable_roots: Vec<RouteFrameVersion>,
    retiring: Vec<RetainedLeafProjection>,
    preserved: Vec<RetainedLeafProjection>,
}

/// Borrowed role selection shared by preparation and exact revalidation.
struct RetainedCommitSelection<'a> {
    retained: Vec<RetainedLeafProjection>,
    activated: Vec<RetainedLeafProjection>,
    disposable_roots: Vec<RouteFrameVersion>,
    retiring: Vec<&'a RetainedLeafProjection>,
    preserved: Vec<&'a RetainedLeafProjection>,
}

impl RetainedCommitPlan {
    pub fn retained(&self) -> &[RetainedLeafProjection] {
        &self.retained
    }
    pub fn activated(&self) -> &[RetainedLeafProjection] {
        &self.activated
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

/// Frozen transfer of the current host's inactive instances to its foreground
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

/// Frozen whole-target source selection, before factory dispatch or owner movement.
#[derive(Debug)]
pub struct InstanceSelectionPlan {
    source: PreparedPublicationObservation,
    selected: Vec<RetainedLeafProjection>,
    reconciliation: RouteReconciliationProjection,
    steps: Vec<TargetInstanceStepProjection>,
}
impl InstanceSelectionPlan {
    pub fn selected(&self) -> &[RetainedLeafProjection] {
        &self.selected
    }
    pub fn target_steps(&self) -> &[TargetInstanceStepProjection] {
        &self.steps
    }
    pub(crate) fn reconciliation_projection(&self) -> &RouteReconciliationProjection {
        &self.reconciliation
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TargetInstanceStepProjection {
    branch_index: usize,
    source: TargetInstanceSourceKind,
    frame: Option<RouteFrameVersion>,
    parent: Option<RouteFrameVersion>,
    outlet: RouteSlotKey,
    slot: Option<RouteSlotId>,
}
impl TargetInstanceStepProjection {
    pub const fn branch_index(&self) -> usize {
        self.branch_index
    }
    pub const fn source(&self) -> TargetInstanceSourceKind {
        self.source
    }
    pub fn frame(&self) -> Option<&RouteFrameVersion> {
        self.frame.as_ref()
    }
    pub fn parent(&self) -> Option<&RouteFrameVersion> {
        self.parent.as_ref()
    }
    pub const fn outlet(&self) -> &RouteSlotKey {
        &self.outlet
    }
    pub const fn slot(&self) -> Option<RouteSlotId> {
        self.slot
    }
}

#[derive(Debug)]
pub struct RetainedInvalidationPlan {
    source: PreparedPublicationObservation,
    invalid: RouteFrameVersion,
    invalid_index: usize,
    retiring: Vec<RetainedLeafProjection>,
    logical_roots: Vec<RouteFrameVersion>,
    accepted: bool,
}
impl RetainedInvalidationPlan {
    pub const fn invalid(&self) -> &RouteFrameVersion {
        &self.invalid
    }
    pub fn retiring(&self) -> &[RetainedLeafProjection] {
        &self.retiring
    }
    pub fn logical_roots(&self) -> &[RouteFrameVersion] {
        &self.logical_roots
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct RetainedRegistryEntry {
    projection: RetainedLeafProjection,
    retirement: Option<RetainedRetirement>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum RetainedRetirement {
    Cleanup(CleanupBatchToken),
    Exit(CleanupBatchIdentity),
}

impl RetainedRegistryEntry {
    pub(super) fn live(projection: RetainedLeafProjection) -> Self {
        Self {
            projection,
            retirement: None,
        }
    }

    pub(super) fn reserve_exit_retirement(&mut self, identity: CleanupBatchIdentity) {
        self.retirement = Some(RetainedRetirement::Exit(identity));
    }

    pub(super) fn seal_exit_retirement(
        &mut self,
        identity: &CleanupBatchIdentity,
        token: &CleanupBatchToken,
    ) {
        if matches!(&self.retirement, Some(RetainedRetirement::Exit(expected)) if expected == identity)
        {
            self.retirement = Some(RetainedRetirement::Cleanup(token.clone()));
        }
    }

    pub(super) const fn projection(&self) -> &RetainedLeafProjection {
        &self.projection
    }

    pub(super) const fn retirement(&self) -> Option<&CleanupBatchToken> {
        match self.retirement.as_ref() {
            Some(RetainedRetirement::Cleanup(token)) => Some(token),
            Some(RetainedRetirement::Exit(_)) | None => None,
        }
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
    let mut expected = expected.collect::<Vec<_>>();
    observed.sort_by(|left, right| left.compare_instance(right));
    expected.sort_by(|left, right| left.compare_instance(right));
    !observed
        .windows(2)
        .any(|pair| pair[0].compare_instance(pair[1]).is_eq())
        && observed == expected
}

// Transit responsibility is independent of CurrentWork and survives discarded
// prepared handles and rejected publication. Neither this holder nor its owners is Clone.
pub(super) struct PreparedRetainedTransfer {
    evidence: SealedCommitEvidence,
    retained: Vec<RetainedLeafOwner>,
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
        F: FnOnce(Vec<RetainedLeafOwner>),
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
        logical_roots.extend(retiring.iter().map(|projection| projection.frame().clone()));
        self.sort_instance_cleanup_roots(&mut logical_roots);
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
                        &self
                            .retained_leaves
                            .iter()
                            .filter(|entry| entry.is_live())
                            .map(RetainedRegistryEntry::projection)
                            .collect::<Vec<_>>(),
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
            && self.prepared_publication_source_matches(index, &retained.plan.source)
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
                    &self
                        .retained_leaves
                        .iter()
                        .filter(|entry| entry.is_live())
                        .map(RetainedRegistryEntry::projection)
                        .collect::<Vec<_>>(),
                ) {
                    return Err(BeginAuthoritySettlementErrorKind::InvalidManifest);
                }
                retiring.push(entry.projection.clone());
            }
        }
        logical_roots.extend(retiring.iter().map(|projection| projection.frame().clone()));
        self.sort_instance_cleanup_roots(&mut logical_roots);
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
            || record_index
                .is_none_or(|index| !self.prepared_publication_source_matches(index, &plan.source))
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

    pub(super) fn sort_instance_cleanup_roots(&self, roots: &mut Vec<RouteFrameVersion>) {
        let mut dependencies = self
            .active_child_slots
            .iter()
            .filter_map(|slot| {
                slot.child()
                    .map(|child| (child.clone(), slot.parent().clone()))
            })
            .collect::<Vec<_>>();
        dependencies.extend(self.retained_leaves.iter().map(|entry| {
            (
                entry.projection.frame().clone(),
                entry.projection.parent().clone(),
            )
        }));
        if let Some(current) = &self.current {
            dependencies.extend(
                current
                    .target_frames
                    .windows(2)
                    .map(|pair| (pair[1].clone(), pair[0].clone())),
            );
        }
        super::instances::sort_roots_child_first(roots, &dependencies);
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
        self.retained_commit_partition(current)
    }

    fn retained_commit_partition(
        &self,
        current: &CurrentWork,
    ) -> Result<RetainedCommitPlan, CommitError> {
        let next_slots = publication_slots(current);
        let selection = self.retained_commit_selection(current, &next_slots)?;
        let record_index = self
            .records
            .iter()
            .position(|record| record.id() == current.identity.request)
            .ok_or(CommitError::Stale)?;
        Ok(RetainedCommitPlan {
            source: self.observe_prepared_publication_source(record_index),
            original_roots: current.cleanup_roots.clone(),
            next_slots,
            retained: selection.retained,
            activated: selection.activated,
            disposable_roots: selection.disposable_roots,
            retiring: selection.retiring.into_iter().cloned().collect(),
            preserved: selection.preserved.into_iter().cloned().collect(),
        })
    }

    fn retained_commit_selection<'a>(
        &'a self,
        current: &CurrentWork,
        next_slots: &[LogicalChildSlotBinding],
    ) -> Result<RetainedCommitSelection<'a>, CommitError> {
        let host = self.state.host().ok_or(CommitError::Stale)?;
        let activated = current
            .reconciliation
            .as_ref()
            .map(|p| p.retained().to_vec())
            .unwrap_or_default();
        if activated.iter().any(|selected| {
            !self
                .retained_leaves
                .iter()
                .any(|entry| entry.is_live() && entry.projection() == selected)
        }) {
            return Err(CommitError::Stale);
        }
        let candidates = self.retention_candidates(current);
        let existing = self
            .retained_leaves
            .iter()
            .map(RetainedRegistryEntry::projection)
            .filter(|instance| !activated.contains(instance))
            .collect::<Vec<_>>();
        let mut live = existing
            .iter()
            .copied()
            .chain(candidates.iter())
            .collect::<Vec<_>>();
        // Removing an invalid parent removes only its exact dependent closure.
        loop {
            let invalid = live
                .iter()
                .filter(|instance| {
                    !scope_survives(instance, host, &current.target_frames, next_slots, &live)
                })
                .map(|instance| instance.frame().clone())
                .collect::<Vec<_>>();
            if invalid.is_empty() {
                break;
            }
            live.retain(|instance| !invalid.contains(instance.frame()));
        }
        while live.len() > self.keep_alive_capacity {
            let oldest = live[0].frame().clone();
            let closure = super::instances::dependency_closure(&oldest, &live);
            live.retain(|instance| !closure.contains(instance.frame()));
        }
        let retained = candidates
            .iter()
            .filter(|instance| live.contains(instance))
            .cloned()
            .collect::<Vec<_>>();
        let mut retiring = existing
            .iter()
            .copied()
            .filter(|instance| !live.contains(instance))
            .collect::<Vec<_>>();
        let preserved = existing
            .iter()
            .copied()
            .filter(|instance| live.contains(instance))
            .collect::<Vec<_>>();
        super::instances::sort_child_first(&mut retiring);
        let mut disposable_roots = current
            .cleanup_roots
            .iter()
            .filter(|frame| !retained.iter().any(|instance| instance.frame() == *frame))
            .cloned()
            .collect::<Vec<_>>();
        disposable_roots.extend(retiring.iter().map(|instance| instance.frame().clone()));
        if disposable_roots
            .iter()
            .enumerate()
            .any(|(index, frame)| disposable_roots[..index].contains(frame))
        {
            return Err(CommitError::InvalidManifest);
        }
        // Existing inactive children can precede their leaving current parent.
        self.sort_instance_cleanup_roots(&mut disposable_roots);
        Ok(RetainedCommitSelection {
            retained,
            activated,
            disposable_roots,
            retiring,
            preserved,
        })
    }

    pub(super) fn retention_candidates(
        &self,
        current: &CurrentWork,
    ) -> Vec<RetainedLeafProjection> {
        if self.keep_alive_capacity == 0
            || !matches!(current.attempt, AttemptKind::Normal(_))
            || self.state.active().kind() != ActivePresentationKind::Route
        {
            return Vec::new();
        }
        let Some(host) = self.state.host() else {
            return Vec::new();
        };
        let Some(branch) = self.active_branch.as_ref() else {
            return Vec::new();
        };
        let Some(location) = self.state.active().location() else {
            return Vec::new();
        };
        let mut candidates = Vec::new();
        for frame in current.cleanup_roots.iter() {
            let FrameOrigin::MatchedRoute(route) = frame.origin() else {
                continue;
            };
            if !self.matcher.keep_alive(*route) {
                continue;
            }
            let mut parents = self
                .active_child_slots
                .iter()
                .filter(|binding| binding.child() == Some(frame));
            let Some(binding) = parents.next() else {
                continue;
            };
            if parents.next().is_some()
                || !matches!(binding.parent().origin(), FrameOrigin::MatchedRoute(_))
            {
                continue;
            }
            let Some(index) = self
                .state
                .active()
                .frames()
                .iter()
                .position(|active| active == frame)
            else {
                continue;
            };
            let Some(matched) = branch
                .routes()
                .get(index)
                .filter(|matched| matched.route() == *route)
            else {
                continue;
            };
            let Some(view) = self.matcher.view_binding(*route) else {
                continue;
            };
            candidates.push(RetainedLeafProjection {
                instance: InstanceFacts {
                    frame: frame.clone(),
                    host: host.clone(),
                    parent: binding.parent().clone(),
                    outlet: binding.outlet().clone(),
                    slot: binding.slot(),
                },
                segment_reuse_key: matched.segment_reuse_key().clone(),
                view: view.clone(),
                location: location.clone(),
                own_slots: self
                    .active_child_slots
                    .iter()
                    .filter(|slot| slot.parent() == frame)
                    .map(|slot| RetainedOwnSlotProjection {
                        parent: frame.clone(),
                        outlet: slot.outlet().clone(),
                        slot: slot.slot(),
                    })
                    .collect(),
            });
        }
        candidates
    }

    pub(super) fn retained_plan_partition_is_current(&self, plan: &RetainedCommitPlan) -> bool {
        self.current
            .as_ref()
            .is_some_and(|current| self.retained_plan_partition_matches(plan, current))
    }
    fn retained_plan_partition_matches(
        &self,
        plan: &RetainedCommitPlan,
        current: &CurrentWork,
    ) -> bool {
        let next_slots = publication_slots(current);
        if current.cleanup_roots != plan.original_roots || next_slots != plan.next_slots {
            return false;
        }
        let Ok(selection) = self.retained_commit_selection(current, &next_slots) else {
            return false;
        };
        selection.retained == plan.retained
            && selection.activated == plan.activated
            && selection.disposable_roots == plan.disposable_roots
            && selection
                .preserved
                .iter()
                .copied()
                .eq(plan.preserved.iter())
            && selection.retiring.iter().copied().eq(plan.retiring.iter())
    }

    pub(crate) fn prepare_instance_selection(
        &mut self,
        task: &FrameTask,
        inventory: &[&RetainedLeafOwner],
    ) -> Result<InstanceSelectionPlan, CommitError> {
        self.inspect_view_report_identity(task.identity())
            .map_err(|_| CommitError::Stale)?;
        if self.pending_retained_transfer.is_some()
            || self.prepared_publication_authority.is_some()
            || !self.current_reconciliation_matches_active()
            || self.retained_leaves.iter().any(|entry| !entry.is_live())
            || !self.instance_selection_eligible(task)
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
        let selected = self.select_retained_prefix(task)?;
        let reconciliation = current
            .reconciliation
            .as_ref()
            .ok_or(CommitError::Stale)?
            .with_instance_selection(
                &selected,
                current.branch.as_ref().ok_or(CommitError::Stale)?,
            )
            .ok_or(CommitError::InvalidManifest)?;
        let steps = self.instance_target_steps(current, &reconciliation)?;
        let index = self
            .records
            .iter()
            .position(|record| record.id() == task.identity.request)
            .ok_or(CommitError::Stale)?;
        Ok(InstanceSelectionPlan {
            source: self.observe_prepared_publication_source(index),
            selected,
            reconciliation,
            steps,
        })
    }

    fn instance_selection_eligible(&self, task: &FrameTask) -> bool {
        let Some(current) = self.current.as_ref() else {
            return false;
        };
        let Some(projection) = current.reconciliation.as_ref() else {
            return false;
        };
        let Some(first) = projection.entering().first() else {
            return false;
        };
        matches!(current.attempt, AttemptKind::Normal(_))
            && self.action.is_none()
            && matches!(current.stage, WorkStage::Applying)
            && current.pending_mount.is_none()
            && current.selected_retained_frames.is_empty()
            && current.applied_frames.is_empty()
            && !projection.instances_selected()
            && first.branch_index() == task.identity.branch_index
            && match first.slot() {
                ProjectedEnteringSlot::Existing(slot) => slot == task.frame.slot(),
                ProjectedEnteringSlot::MountNew => {
                    matches!(&task.target, FrameMountTarget::ChildOutlet { parent, outlet }
                        if task.identity.branch_index.checked_sub(1)
                            .and_then(|index| current.target_frames.get(index)) == Some(parent)
                            && current.branch.as_ref()
                                .and_then(|branch| branch.routes().get(task.identity.branch_index))
                                .is_some_and(|route| route.slot() == outlet))
                }
            }
            && current.target_frames.get(task.identity.branch_index) == Some(&task.frame)
            && current.current_target == task.target
            && current.location == task.location
    }
    fn select_retained_prefix(
        &self,
        task: &FrameTask,
    ) -> Result<Vec<RetainedLeafProjection>, CommitError> {
        let current = self.current.as_ref().ok_or(CommitError::Stale)?;
        let branch = current.branch.as_ref().ok_or(CommitError::Stale)?;
        let projection = current.reconciliation.as_ref().ok_or(CommitError::Stale)?;
        let host = self.state.host().ok_or(CommitError::Stale)?;
        let fragment_replacement = self.requires_fragment_replacement(&task.location, branch);
        let mut selected = Vec::new();
        if self.keep_alive_capacity == 0 || task.identity.branch_index == 0 {
            return Ok(selected);
        }
        for index in task.identity.branch_index..branch.routes().len() {
            let matched = &branch.routes()[index];
            if !self.matcher.keep_alive(matched.route())
                || (fragment_replacement && index + 1 == branch.routes().len())
            {
                break;
            }
            let parent = selected
                .last()
                .map(RetainedLeafProjection::frame)
                .or_else(|| projection.preserved().last().map(|frame| frame.version()))
                .ok_or(CommitError::InvalidManifest)?;
            let slot = if let Some(parent) = selected.last() {
                parent
                    .own_slots()
                    .iter()
                    .find(|slot| slot.outlet() == matched.slot())
                    .map(|slot| slot.slot())
            } else {
                self.active_child_slots
                    .iter()
                    .find(|slot| slot.parent() == parent && slot.outlet() == matched.slot())
                    .map(|slot| slot.slot())
            };
            let Some(slot) = slot else {
                break;
            };
            let Some(view) = self.matcher.view_binding(matched.route()) else {
                return Err(CommitError::InvalidManifest);
            };
            // Registry order is successful-departure order. A failed preparation never refreshes it.
            let hit = self
                .retained_leaves
                .iter()
                .rev()
                .map(RetainedRegistryEntry::projection)
                .find(|hit| {
                    hit.host() == host
                        && hit.frame().router_identity() == self.router
                        && hit.frame().origin() == &FrameOrigin::MatchedRoute(matched.route())
                        && hit.parent() == parent
                        && hit.outlet() == matched.slot()
                        && hit.slot() == slot
                        && hit.segment_reuse_key() == matched.segment_reuse_key()
                        && hit.view() == view
                        && (matched.preserve_on_search()
                            || hit.location().search() == task.location.search())
                        && (index + 1 != branch.routes().len()
                            || hit.location().fragment() == task.location.fragment())
                });
            let Some(hit) = hit else {
                break;
            };
            selected.push(hit.clone());
        }
        Ok(selected)
    }
    fn instance_target_steps(
        &self,
        current: &CurrentWork,
        projection: &RouteReconciliationProjection,
    ) -> Result<Vec<TargetInstanceStepProjection>, CommitError> {
        let branch = current.branch.as_ref().ok_or(CommitError::Stale)?;
        let mut steps = Vec::with_capacity(branch.routes().len());
        for (index, route) in branch.routes().iter().enumerate() {
            let source = projection
                .source_at(index)
                .ok_or(CommitError::InvalidManifest)?;
            let frame = match source {
                TargetInstanceSourceKind::Current => projection
                    .preserved()
                    .iter()
                    .find(|frame| frame.branch_index() == index)
                    .map(|frame| frame.version().clone()),
                TargetInstanceSourceKind::Retained => projection
                    .retained()
                    .get(index - projection.preserved().len())
                    .map(|instance| instance.frame().clone()),
                TargetInstanceSourceKind::Fresh => projection
                    .entering()
                    .iter()
                    .find(|frame| frame.branch_index() == index)
                    .and_then(|frame| match frame.slot() {
                        ProjectedEnteringSlot::Existing(slot) => Some(RouteFrameVersion::new(
                            self.router,
                            current.identity.host,
                            slot,
                            frame.origin().clone(),
                            frame.frame(),
                            frame.generation(),
                        )),
                        ProjectedEnteringSlot::MountNew => None,
                    }),
            };
            let parent = if index == 0 {
                None
            } else {
                steps
                    .last()
                    .and_then(|step: &TargetInstanceStepProjection| step.frame.clone())
            };
            let slot = frame.as_ref().map(RouteFrameVersion::slot);
            steps.push(TargetInstanceStepProjection {
                branch_index: index,
                source,
                frame,
                parent,
                outlet: route.slot().clone(),
                slot,
            });
        }
        Ok(steps)
    }
    pub(crate) fn accept_instance_selection(
        &mut self,
        mut task: FrameTask,
        plan: &InstanceSelectionPlan,
    ) -> Result<EngineAction, (FrameTask, CommitError)> {
        let Some(index) = self
            .records
            .iter()
            .position(|record| record.id() == task.identity.request)
        else {
            return Err((task, CommitError::Stale));
        };
        if !self.instance_selection_eligible(&task)
            || self.inspect_view_report_identity(task.identity()).is_err()
            || !self.prepared_publication_source_matches(index, &plan.source)
            || self.select_retained_prefix(&task).ok().as_ref() != Some(&plan.selected)
        {
            return Err((task, CommitError::Stale));
        }
        let Some(mut next) = self
            .current
            .as_ref()
            .and_then(CurrentWork::clone_pre_retirement)
        else {
            return Err((task, CommitError::Stale));
        };
        let Some(reconciliation) = next.reconciliation.as_ref().and_then(|projection| {
            projection.with_instance_selection(&plan.selected, next.branch.as_ref()?)
        }) else {
            return Err((task, CommitError::InvalidManifest));
        };
        if reconciliation != plan.reconciliation
            || self
                .instance_target_steps(&next, &reconciliation)
                .ok()
                .as_ref()
                != Some(&plan.steps)
        {
            return Err((task, CommitError::Stale));
        }
        next.reconciliation = Some(reconciliation);
        next.selected_retained_frames = plan
            .selected
            .iter()
            .map(|instance| instance.frame().clone())
            .collect();
        if plan.selected.is_empty() {
            task.instance_selection_pending = false;
            self.current = Some(next);
            return Ok(EngineAction::FrameTask(task));
        }
        next.target_frames = next
            .reconciliation
            .as_ref()
            .unwrap()
            .preserved()
            .iter()
            .map(|frame| frame.version().clone())
            .chain(
                plan.selected
                    .iter()
                    .map(|instance| instance.frame().clone()),
            )
            .collect();
        self.current = Some(next);
        Ok(self.dispatch_selected_target())
    }
    fn dispatch_selected_target(&mut self) -> EngineAction {
        let mut current = self.current.take().expect("selection accepted current");
        let host = self.state.host().expect("selection accepted host").clone();
        let branch = current.branch.as_ref().expect("selection is normal");
        let next = current
            .reconciliation
            .as_ref()
            .unwrap()
            .fresh_entering()
            .next()
            .cloned();
        let Some(entering) = next else {
            let index = current.target_frames.len() - 1;
            let frame = current.target_frames[index].clone();
            current.identity.branch_index = index;
            current.identity.frame_task = current.reservation[index].frame_task();
            current.identity.frame = frame.frame();
            current.identity.generation = frame.generation();
            current.identity.slot = frame.slot();
            current.identity.origin = frame.origin().clone();
            current.stage = WorkStage::ReadyToCommit;
            let ticket = CommitTicket {
                identity: current.identity.clone(),
            };
            let request = current.identity.request;
            self.current = Some(current);
            self.transition_with_trace(request, NavigationPhase::ReadyToSwitch);
            return EngineAction::Commit(ticket);
        };
        let index = entering.branch_index();
        let matched = branch.routes()[index].clone();
        let origin = entering.origin().clone();
        if entering.slot() == ProjectedEnteringSlot::MountNew {
            let parent_index = index - 1;
            let parent = current.target_frames[parent_index].clone();
            let outlet = matched.slot().clone();
            current.identity.branch_index = parent_index;
            current.identity.frame_task = current.reservation[index].frame_task();
            current.identity.frame = parent.frame();
            current.identity.generation = parent.generation();
            current.identity.slot = parent.slot();
            current.identity.origin = parent.origin().clone();
            current.current_target = if parent_index == 0 {
                FrameMountTarget::TopSlot {
                    host: host.clone(),
                    slot: parent.slot(),
                }
            } else {
                FrameMountTarget::RegisteredChildSlot {
                    parent: current.target_frames[parent_index - 1].clone(),
                    outlet: branch.routes()[parent_index].slot().clone(),
                }
            };
            current.pending_mount = Some(PendingMount {
                child_index: index,
                parent: parent.clone(),
                outlet: outlet.clone(),
                parent_source: current
                    .reconciliation
                    .as_ref()
                    .and_then(|projection| projection.source_at(parent_index))
                    .expect("selected exact parent"),
                transaction: current.identity.transaction,
                reserved: current.reservation[index],
                kind: PendingMountKind::Normal,
            });
            current.stage = WorkStage::AwaitingMount;
            let request = current.identity.request;
            let transaction = current.identity.transaction;
            self.current = Some(current);
            self.transition_with_trace(request, NavigationPhase::ResolvingMount);
            let token = match self.mount.issue(
                &mut self.ids,
                host,
                request,
                transaction,
                parent.clone(),
                outlet.clone(),
            ) {
                Ok(token) => token,
                Err(_) => {
                    return EngineAction::FailureCleanup(self.begin_failure_cleanup_for_branch(
                        PendingFailureKind::MountIssue,
                        NavigationFailure::new(
                            NavigationFailureKind::Mount,
                            "selected parent mount request rejected",
                        ),
                        index,
                    ));
                }
            };
            self.record_mut(request)
                .unwrap()
                .append_trace(NavigationTraceEvent::mount_requested(
                    parent.clone(),
                    outlet.clone(),
                ))
                .expect("request trace is open");
            return EngineAction::MountRequest(FrameMountRequest {
                token,
                request,
                parent,
                outlet,
            });
        }
        let ProjectedEnteringSlot::Existing(slot) = entering.slot() else {
            unreachable!()
        };
        let frame = RouteFrameVersion::new(
            self.router,
            host.host(),
            slot,
            origin.clone(),
            entering.frame(),
            entering.generation(),
        );
        current.target_frames.push(frame.clone());
        current.identity.branch_index = index;
        current.identity.frame_task = current.reservation[index].frame_task();
        current.identity.frame = frame.frame();
        current.identity.generation = frame.generation();
        current.identity.slot = slot;
        current.identity.origin = origin.clone();
        let target = FrameMountTarget::RegisteredChildSlot {
            parent: current.target_frames[index - 1].clone(),
            outlet: matched.slot().clone(),
        };
        current.current_target = target.clone();
        current.pending_mount = None;
        current.stage = WorkStage::Applying;
        let current_parent_source = current
            .reconciliation
            .as_ref()
            .and_then(|projection| projection.source_at(index - 1));
        let identity = current.identity.clone();
        let location = current.location.clone();
        let view = self
            .matcher
            .view_binding(matched.route())
            .expect("matched view")
            .clone();
        let input = RouteViewInput::new(
            location.clone(),
            Some(matched.route()),
            matched.params().clone(),
            origin,
            None,
        );
        self.current = Some(current);
        self.record_mut(identity.request)
            .unwrap()
            .append_trace(NavigationTraceEvent::frame_task_issued(frame.clone()))
            .expect("request trace is open");
        self.transition_with_trace(identity.request, NavigationPhase::ApplyingFrame(index));
        self.issued_view_tasks.push(identity.clone());
        EngineAction::FrameTask(FrameTask {
            parent_source: current_parent_source,
            instance_selection_pending: false,
            identity,
            frame,
            target,
            location,
            matched: Some(matched),
            view,
            input: Some(input),
        })
    }

    fn retained_invalidation_receiver_is_current(
        &self,
        receiver: CoreRetainedInvalidationReceiverRef<'_>,
    ) -> bool {
        match receiver {
            CoreRetainedInvalidationReceiverRef::FrameTask(task) => {
                self.inspect_view_report_identity(task.identity()).is_ok()
            }
            CoreRetainedInvalidationReceiverRef::Commit(ticket) => {
                self.inspect_commit_identity(ticket).is_ok()
            }
            CoreRetainedInvalidationReceiverRef::AfterFailure(owner) => {
                self.current.as_ref().is_some_and(|current| {
                    current.identity == owner.identity
                        && matches!(current.stage, WorkStage::Applying)
                }) && self
                    .completed_cleanup
                    .iter()
                    .any(|completed| completed.token == owner.completed)
            }
        }
    }
    pub(crate) fn inspect_retained_invalidation_continuation(
        &self,
        owner: &RetainedInvalidationContinuation,
    ) -> bool {
        self.retained_invalidation_receiver_is_current(
            CoreRetainedInvalidationReceiverRef::AfterFailure(owner),
        ) && self.pending_retained_miss_cleanup.is_none()
            && self.pending_retained_transfer.is_none()
            && self.prepared_publication_authority.is_none()
            && self.current_reconciliation_matches_active()
            && self
                .records
                .iter()
                .position(|record| record.id() == owner.identity.request)
                .is_some_and(|index| self.prepared_publication_source_matches(index, &owner.source))
            && self.current.as_ref().is_some_and(|current| {
                current.applied_frames.is_empty()
                    && current.reconciliation.as_ref().is_some_and(|projection| {
                        projection.instances_selected()
                            && projection
                                .retained()
                                .iter()
                                .any(|instance| instance.frame() == owner.invalid())
                            && projection.retained().iter().all(|instance| {
                                self.retained_leaves
                                    .iter()
                                    .any(|entry| entry.is_live() && entry.projection() == instance)
                            })
                    })
            })
    }

    pub(crate) fn resume_retained_invalidation(
        &mut self,
        owner: RetainedInvalidationContinuation,
        inventory: &[&RetainedLeafOwner],
    ) -> Result<
        EngineAction,
        (
            RetainedInvalidationContinuation,
            RetainedInvalidationResumeFailure,
        ),
    > {
        if !self.inspect_retained_invalidation_continuation(&owner)
            || !owner_projections_match(
                inventory.iter().copied(),
                self.retained_leaves
                    .iter()
                    .map(RetainedRegistryEntry::projection),
            )
        {
            return Err((owner, RetainedInvalidationResumeFailure::Stale));
        }
        let Some(mut next) = self
            .current
            .as_ref()
            .and_then(CurrentWork::clone_pre_retirement)
        else {
            return Err((owner, RetainedInvalidationResumeFailure::Stale));
        };
        let Some(branch) = next.branch.as_ref() else {
            return Err((owner, RetainedInvalidationResumeFailure::InvalidTarget));
        };
        let depth = branch.routes().len();
        let prefix = next
            .reconciliation
            .as_ref()
            .expect("continuation verified selection")
            .preserved()
            .len()
            + next.selected_retained_frames.len();
        let reservation_start = if prefix == depth { depth - 1 } else { prefix };
        for index in reservation_start..depth {
            let reserved = match self.ids.reserve_attempt() {
                Ok(reserved) => reserved,
                Err(_) => {
                    return Err((owner, RetainedInvalidationResumeFailure::IdentityExhausted));
                }
            };
            next.reservation[index] =
                ReservedFrameAttempt::new(reserved.frame_task(), reserved.candidate_frame());
        }
        let Some(projection) = next.reconciliation.as_ref().and_then(|projection| {
            projection.reissue_fresh_suffix(
                branch,
                &next
                    .reservation
                    .iter()
                    .map(|reserved| reserved.frame())
                    .collect::<Vec<_>>(),
            )
        }) else {
            return Err((owner, RetainedInvalidationResumeFailure::InvalidTarget));
        };
        next.target_frames.truncate(prefix);
        next.pending_mount = None;
        next.reconciliation = Some(projection);
        self.current = Some(next);
        drop(owner);
        Ok(self.dispatch_selected_target())
    }

    pub(crate) fn reject_retained_invalidation(
        &mut self,
        receiver: CoreRetainedInvalidationReceiver,
        invalid: RouteFrameVersion,
    ) -> Result<FailureCleanupRequest, (CoreRetainedInvalidationReceiver, CommitError)> {
        if matches!(receiver, CoreRetainedInvalidationReceiver::AfterFailure(_))
            || !self.retained_invalidation_receiver_is_current(receiver.as_ref())
            || self.pending_retained_miss_cleanup.is_some()
            || self.pending_retained_transfer.is_some()
            || self.prepared_publication_authority.is_some()
            || !self.current_reconciliation_matches_active()
            || !self.current.as_ref().is_some_and(|current| {
                current.reconciliation.as_ref().is_some_and(|projection| {
                    projection.instances_selected()
                        && projection.retained().iter().any(|instance| {
                            instance.frame() == &invalid
                                && self
                                    .retained_leaves
                                    .iter()
                                    .any(|entry| entry.is_live() && entry.projection() == instance)
                        })
                })
            })
        {
            return Err((receiver, CommitError::Stale));
        }
        let current = self
            .current
            .as_ref()
            .expect("the original receiver was validated");
        let mut roots = current.applied_frames.clone();
        if let CoreRetainedInvalidationReceiver::FrameTask(task) = &receiver {
            if current
                .reconciliation
                .as_ref()
                .and_then(|projection| projection.source_at(task.identity().branch_index))
                == Some(TargetInstanceSourceKind::Fresh)
                && !roots.contains(task.frame_version())
            {
                roots.push(task.frame_version().clone());
            }
        }
        self.sort_instance_cleanup_roots(&mut roots);
        Ok(self.begin_failure_cleanup_with_roots(
            PendingFailureKind::Apply,
            NavigationFailure::new(
                NavigationFailureKind::Apply,
                "selected instance invalidated during frame preparation",
            ),
            FailureCleanupContinuationOwner::ResumeInstanceInvalidation { invalid },
            roots,
        ))
    }

    pub(crate) fn prepare_retained_invalidation_plan(
        &self,
        receiver: CoreRetainedInvalidationReceiverRef<'_>,
        selection: Option<&InstanceSelectionPlan>,
        invalid: &RouteFrameVersion,
        inventory: &[&RetainedLeafOwner],
    ) -> Result<RetainedInvalidationPlan, CommitError> {
        if !self.retained_invalidation_receiver_is_current(receiver)
            || matches!(receiver,CoreRetainedInvalidationReceiverRef::AfterFailure(owner) if !self.inspect_retained_invalidation_continuation(owner))
        {
            return Err(CommitError::Stale);
        }
        if self.pending_retained_miss_cleanup.is_some()
            || self.pending_retained_transfer.is_some()
            || self.prepared_publication_authority.is_some()
            || !self.current_reconciliation_matches_active()
            || !owner_projections_match(
                inventory.iter().copied(),
                self.retained_leaves
                    .iter()
                    .map(RetainedRegistryEntry::projection),
            )
        {
            return Err(CommitError::Stale);
        }
        let current = self.current.as_ref().ok_or(CommitError::Stale)?;
        let record = self
            .records
            .iter()
            .position(|record| record.id() == receiver.identity().request)
            .ok_or(CommitError::Stale)?;
        let source = self.observe_prepared_publication_source(record);
        let selected = match selection {
            Some(plan) if matches!(receiver,CoreRetainedInvalidationReceiverRef::FrameTask(task) if self.instance_selection_eligible(task) && source.same_source(&plan.source) && self.select_retained_prefix(task).ok().as_ref() == Some(&plan.selected)) => {
                &plan.selected
            }
            Some(_) => return Err(CommitError::Stale),
            None => current
                .reconciliation
                .as_ref()
                .filter(|projection| projection.instances_selected())
                .map(|projection| projection.retained())
                .ok_or(CommitError::Stale)?,
        };
        if matches!(receiver,CoreRetainedInvalidationReceiverRef::AfterFailure(owner) if owner.invalid() != invalid)
        {
            return Err(CommitError::InvalidManifest);
        }
        let offset = selected
            .iter()
            .position(|instance| instance.frame() == invalid)
            .ok_or(CommitError::InvalidManifest)?;
        let invalid_index = current
            .reconciliation
            .as_ref()
            .ok_or(CommitError::Stale)?
            .preserved()
            .len()
            + offset;
        let inventory = self
            .retained_leaves
            .iter()
            .map(RetainedRegistryEntry::projection)
            .collect::<Vec<_>>();
        let closure = super::instances::dependency_closure(invalid, &inventory);
        let retiring = inventory
            .into_iter()
            .filter(|instance| closure.contains(instance.frame()))
            .cloned()
            .collect::<Vec<_>>();
        let mut logical_roots = retiring
            .iter()
            .map(|instance| instance.frame().clone())
            .collect::<Vec<_>>();
        if selection.is_none() {
            logical_roots.extend(current.applied_frames.iter().cloned());
            if !matches!(
                receiver,
                CoreRetainedInvalidationReceiverRef::AfterFailure(_)
            ) && current
                .reconciliation
                .as_ref()
                .and_then(|projection| projection.source_at(receiver.identity().branch_index))
                == Some(TargetInstanceSourceKind::Fresh)
            {
                if let Some(frame) = current.target_frames.get(receiver.identity().branch_index) {
                    if !logical_roots.contains(frame) {
                        logical_roots.push(frame.clone());
                    }
                }
            }
        }
        self.sort_instance_cleanup_roots(&mut logical_roots);
        if logical_roots
            .iter()
            .enumerate()
            .any(|(index, frame)| logical_roots[..index].contains(frame))
        {
            return Err(CommitError::InvalidManifest);
        }
        Ok(RetainedInvalidationPlan {
            source,
            invalid: invalid.clone(),
            invalid_index,
            retiring,
            logical_roots,
            accepted: selection.is_none(),
        })
    }

    pub(crate) fn begin_retained_miss_cleanup<F>(
        &mut self,
        receiver: CoreRetainedInvalidationReceiver,
        plan: &RetainedInvalidationPlan,
        seal: CleanupManifestSeal,
        owners: Vec<RetainedLeafOwner>,
        install: F,
    ) -> Result<
        (),
        (
            CoreRetainedInvalidationReceiver,
            Vec<RetainedLeafOwner>,
            CommitError,
        ),
    >
    where
        F: FnOnce(RetainedMissCleanupOwner),
    {
        let Some(index) = self
            .records
            .iter()
            .position(|record| record.id() == receiver.identity().request)
        else {
            return Err((receiver, owners, CommitError::Stale));
        };
        if self.pending_retained_miss_cleanup.is_some()
            || self.pending_retained_transfer.is_some()
            || self.prepared_publication_authority.is_some()
            || self.live_failed_authority_trigger_revision.is_some()
            || !self.retained_invalidation_receiver_is_current(receiver.as_ref())
            || !self.prepared_publication_source_matches(index, &plan.source)
        {
            return Err((receiver, owners, CommitError::Stale));
        }
        if !owner_projections_match(owners.iter(), plan.retiring.iter())
            || seal.logical_roots() != plan.logical_roots
        {
            return Err((receiver, owners, CommitError::InvalidManifest));
        }
        if self.completed_cleanup.try_reserve(1).is_err() {
            return Err((receiver, owners, CommitError::Stale));
        }
        let resume = if plan.accepted {
            let depth = self
                .current
                .as_ref()
                .and_then(|current| current.branch.as_ref())
                .map(|branch| branch.routes().len())
                .ok_or(());
            let Ok(depth) = depth else {
                return Err((receiver, owners, CommitError::Stale));
            };
            let mut fresh = Vec::with_capacity(depth - plan.invalid_index);
            for _ in plan.invalid_index..depth {
                let reserved = match self.ids.reserve_attempt() {
                    Ok(reserved) => reserved,
                    Err(_) => return Err((receiver, owners, CommitError::Stale)),
                };
                fresh.push(ReservedFrameAttempt::new(
                    reserved.frame_task(),
                    reserved.candidate_frame(),
                ));
            }
            Some((plan.invalid_index, fresh))
        } else {
            None
        };
        let batch = match self.ids.reserve_cleanup_batch() {
            Ok(batch) => batch,
            Err(_) => return Err((receiver, owners, CommitError::Stale)),
        };
        let cleanup = CleanupBatchToken::sealed(
            receiver.identity().clone(),
            batch,
            seal.logical_roots(),
            seal.manifest_digest(),
            seal.obligation_count(),
        );
        let mut registry = self.retained_leaves.clone();
        Self::bind_retained_retirements(&mut registry, &plan.retiring, &cleanup);
        let mut source = self.observe_prepared_publication_source(index);
        source.retained_leaves = registry.clone();
        source.pending_retained_miss_cleanup = Some(cleanup.clone());
        let owner = RetainedMissCleanupOwner {
            receiver,
            retained: owners,
            cleanup: cleanup.clone(),
            source,
            resume,
        };
        self.retained_leaves = registry;
        self.pending_retained_miss_cleanup = Some(cleanup);
        install(owner);
        Ok(())
    }
    fn retained_miss_cleanup_record(
        &self,
        owner: &RetainedMissCleanupOwner,
    ) -> Result<usize, CleanupError> {
        let index = self
            .records
            .iter()
            .position(|record| record.id() == owner.receiver.identity().request)
            .ok_or(CleanupError::Stale)?;
        if self.pending_retained_miss_cleanup.as_ref() != Some(&owner.cleanup)
            || !self.retained_invalidation_receiver_is_current(owner.receiver.as_ref())
            || !self.prepared_publication_source_matches(index, &owner.source)
        {
            return Err(CleanupError::Stale);
        }
        Ok(index)
    }

    /// Revokes the foreground continuation while preserving the original
    /// cache owners for the trusted ECS authority residual. No physical
    /// completion or receipt is invented for the stopped cache token.
    pub(crate) fn fail_closed_retained_miss_cleanup_authority_protocol_retaining<F>(
        &mut self,
        issuer: &mut HostAdmissionIssuer,
        owner: RetainedMissCleanupOwner,
        lease: HostLease,
        install: F,
    ) -> Result<
        (),
        (
            RetainedMissCleanupOwner,
            HostLease,
            RetainedMissCleanupAuthorityFailCloseErrorKind,
        ),
    >
    where
        F: FnOnce(Vec<RetainedLeafOwner>),
    {
        if self.retained_miss_cleanup_record(&owner).is_err()
            || !owner_projections_match(
                owner.retained.iter(),
                self.retained_leaves
                    .iter()
                    .filter(|entry| entry.retirement() == Some(&owner.cleanup))
                    .map(RetainedRegistryEntry::projection),
            )
        {
            return Err((
                owner,
                lease,
                RetainedMissCleanupAuthorityFailCloseErrorKind::SourceMismatch,
            ));
        }
        let expected_miss = owner.cleanup.clone();
        let mut original = Some(owner);
        match self.fail_closed_authority_protocol_retaining_with_miss(
            issuer,
            Some(lease),
            Some(&expected_miss),
            || {
                let RetainedMissCleanupOwner {
                    receiver,
                    retained,
                    cleanup: _,
                    source: _,
                    resume: _,
                } = original
                    .take()
                    .expect("accepted authority termination consumes the original cache owner");
                drop(receiver);
                install(retained);
            },
        ) {
            Ok(()) => Ok(()),
            Err(lease) => Err((
                original
                    .take()
                    .expect("rejected authority termination returns its original cache owner"),
                lease.expect("the installed-owner terminal requires the original live lease"),
                RetainedMissCleanupAuthorityFailCloseErrorKind::ProtocolRejected,
            )),
        }
    }

    pub(crate) fn observe_retained_miss_cleanup_receipt(
        &mut self,
        owner: &mut RetainedMissCleanupOwner,
        disposition: crate::trace::CleanupDisposition,
    ) -> Result<(), CleanupError> {
        let index = self.retained_miss_cleanup_record(owner)?;
        let mut record = self.records[index].clone();
        record
            .append_trace(NavigationTraceEvent::cleanup_settled(Some(disposition)))
            .map_err(|_| CleanupError::Invalid)?;
        owner.source.record = Some(record.clone());
        self.records[index] = record;
        Ok(())
    }
    pub(crate) fn complete_retained_miss_cleanup<Physical, F>(
        &mut self,
        owner: RetainedMissCleanupOwner,
        completion: CleanupCompletion,
        physical: Physical,
        reclaim: F,
    ) -> Result<
        EngineAction,
        (
            RetainedMissCleanupOwner,
            CleanupCompletion,
            Physical,
            CleanupError,
        ),
    >
    where
        F: FnOnce(Physical),
    {
        let index = match self.retained_miss_cleanup_record(&owner) {
            Ok(index) => index,
            Err(error) => return Err((owner, completion, physical, error)),
        };
        if !completion.matches(&owner.cleanup) {
            let error = self.cleanup_completion_mismatch(&completion);
            return Err((owner, completion, physical, error));
        }
        let mut record = self.records[index].clone();
        if record
            .append_trace(NavigationTraceEvent::cleanup_settled(None))
            .is_err()
        {
            return Err((owner, completion, physical, CleanupError::Invalid));
        }
        // Prepare the exact resumed target before consuming the accepted cleanup owner.
        let next = if let Some((invalid_index, fresh)) = &owner.resume {
            let Some(mut next) = self
                .current
                .as_ref()
                .and_then(CurrentWork::clone_pre_retirement)
            else {
                return Err((owner, completion, physical, CleanupError::Stale));
            };
            let Some(branch) = next.branch.as_ref() else {
                return Err((owner, completion, physical, CleanupError::Stale));
            };
            for (offset, reserved) in fresh.iter().enumerate() {
                next.reservation[invalid_index + offset] = *reserved;
            }
            let Some(projection) = next.reconciliation.as_ref().and_then(|projection| {
                projection.invalidate_retained_suffix(
                    *invalid_index,
                    branch,
                    &next
                        .reservation
                        .iter()
                        .map(|reserved| reserved.frame())
                        .collect::<Vec<_>>(),
                )
            }) else {
                return Err((owner, completion, physical, CleanupError::Invalid));
            };
            next.target_frames.truncate(*invalid_index);
            next.selected_retained_frames = projection
                .retained()
                .iter()
                .map(|instance| instance.frame().clone())
                .collect();
            next.applied_frames.clear();
            next.reconciliation = Some(projection);
            next.pending_mount = None;
            Some(next)
        } else {
            None
        };
        let RetainedMissCleanupOwner {
            receiver,
            retained,
            cleanup,
            source: _,
            resume: _,
        } = owner;
        reclaim(physical);
        self.records[index] = record;
        self.retained_leaves
            .retain(|entry| entry.retirement() != Some(&cleanup));
        self.pending_retained_miss_cleanup = None;
        self.completed_cleanup.push(CompletedCleanupTombstone {
            request: receiver.identity().request,
            token: cleanup,
        });
        drop(retained);
        if let Some(next) = next {
            self.current = Some(next);
            Ok(self.dispatch_selected_target())
        } else {
            let CoreRetainedInvalidationReceiver::FrameTask(task) = receiver else {
                unreachable!("an unaccepted selection owns its original first task")
            };
            Ok(EngineAction::FrameTask(task))
        }
    }

    pub(crate) fn prepare_retained_publication(
        &mut self,
        ticket: &CommitTicket,
        plan: &RetainedCommitPlan,
        seal: CleanupManifestSeal,
        retiring: Vec<RetainedLeafOwner>,
    ) -> Result<(PreparedPublication, CleanupBatchToken), (Vec<RetainedLeafOwner>, CommitError)>
    {
        if !owner_projections_match(
            retiring.iter(),
            plan.retiring.iter().chain(plan.activated.iter()),
        ) || self.pending_retained_transfer.is_some()
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
        if !self.prepared_publication_source_matches(record_index, &plan.source)
            || !self.retained_plan_partition_is_current(plan)
        {
            return Err((retiring, CommitError::Stale));
        }
        if seal.logical_roots() != plan.disposable_roots.as_slice() {
            return Err((retiring, CommitError::InvalidManifest));
        }
        let mut next_registry = self
            .retained_leaves
            .iter()
            .filter(|entry| !plan.activated.contains(entry.projection()))
            .cloned()
            .collect::<Vec<_>>();
        next_registry.extend(plan.retained.iter().cloned().map(|projection| {
            RetainedRegistryEntry {
                projection,
                retirement: None,
            }
        }));
        let retained = plan
            .retained
            .iter()
            .cloned()
            .map(|projection| RetainedLeafOwner { projection })
            .collect();
        let sealed = match self.seal_commit_partition(ticket, seal, Some((plan, next_registry))) {
            Ok(sealed) => sealed,
            Err(error) => return Err((retiring, error)),
        };
        self.prepare_retained_transfer(sealed, retained, retiring)
    }

    fn prepare_retained_transfer(
        &mut self,
        sealed: SealedCommit,
        retained: Vec<RetainedLeafOwner>,
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
        retained: Vec<RetainedLeafOwner>,
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

    // The history capability moved into the sealed publication. Inspect that
    // original owner rather than the now-empty current_history_capability slot.
    pub(crate) fn inspect_retained_back_publication(
        &self,
        prepared: &PreparedPublication,
        scope: &BackResolutionScopeProjection,
        execution: &HistoryExecutionBindingProjection,
    ) -> Result<(), CorePreparationReceiverMismatchProjection> {
        let current = self.current.as_ref();
        let observed = CoreBackTargetContinuationObservedProjection {
            scope: self.back_target_engine_scope_projection(),
            history: self
                .prepared_publication_authority
                .as_ref()
                .and_then(|authority| authority.owner().evidence.history_execution.clone()),
            current: current.map(|current| current.identity.clone()),
            action: Self::back_target_action_projection(self.action.as_ref()),
            stage: Self::back_target_stage_projection(current),
        };
        if scope.matches_engine_scope(
            observed.scope.world,
            observed.scope.router,
            observed.scope.history,
        ) && scope.matches_execution(execution)
            && prepared.evidence.history_execution.as_ref() == Some(execution)
            && observed.history.as_ref() == Some(execution)
            && observed.current.as_ref() == Some(&prepared.evidence.identity)
            && observed.action == CoreBackTargetReceiverActionProjection::Missing
        {
            Ok(())
        } else {
            Err(CorePreparationReceiverMismatchProjection::Continuation {
                expected: CoreBackTargetContinuationExpectedProjection {
                    scope: scope.clone(),
                    commit: Some(prepared.evidence.identity.clone()),
                },
                observed,
            })
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
        retiring: &[RetainedLeafProjection],
        cleanup: &CleanupBatchToken,
    ) {
        for entry in registry {
            if retiring
                .iter()
                .any(|retiring| retiring == &entry.projection)
            {
                entry.retirement = Some(RetainedRetirement::Cleanup(cleanup.clone()));
            }
        }
    }

    pub(super) fn settle_retained_cleanup(&mut self, completed: &CleanupBatchToken) {
        self.retained_leaves
            .retain(|entry| entry.retirement() != Some(completed));
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
    inventory: &[&RetainedLeafProjection],
) -> bool {
    if projection.host() != host || projection.frame() == projection.parent() {
        return false;
    }
    if frames.contains(projection.parent()) {
        slots
            .iter()
            .filter(|slot| {
                slot.parent() == projection.parent()
                    && slot.outlet() == projection.outlet()
                    && slot.slot() == projection.slot()
            })
            .count()
            == 1
    } else {
        let mut parents = inventory
            .iter()
            .filter(|parent| parent.frame() == projection.parent() && parent.host() == host);
        let Some(parent) = parents.next() else {
            return false;
        };
        parents.next().is_none()
            && parent
                .own_slots()
                .iter()
                .filter(|slot| {
                    slot.parent() == projection.parent()
                        && slot.outlet() == projection.outlet()
                        && slot.slot() == projection.slot()
                })
                .count()
                == 1
    }
}
