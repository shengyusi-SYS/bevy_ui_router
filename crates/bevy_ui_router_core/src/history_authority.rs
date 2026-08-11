use std::sync::atomic::{AtomicU64, Ordering};

use crate::HostVersion;
use crate::cleanup::CleanupBatchToken;
use crate::fingerprint::RouteBranchFingerprint;
use crate::frame::{FrameOrigin, LogicalChildSlotBinding, RouteFrameVersion};
use crate::history::{HistorySnapshot, MemoryHistory};
use crate::ids::{
    CleanupBatchId, FrameTaskId, NavigationRequestId, ReservedBranchAttempt, RouteFrameId,
    RouteGeneration, RouteHostId, RouteSlotId, RouterIdentity, TransactionId,
};
use crate::location::RouteLocation;
use crate::presentation::ActivePresentation;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct HistoryIncarnation(RouterIdentity);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct HistoryEntryId {
    incarnation: HistoryIncarnation,
    creator: NavigationRequestId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct HistoryRevision {
    incarnation: HistoryIncarnation,
    committed_by: Option<NavigationRequestId>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct FrozenHistorySource {
    incarnation: HistoryIncarnation,
    revision: HistoryRevision,
    cursor: Option<usize>,
    entry_ids: Vec<HistoryEntryId>,
    locations: Vec<RouteLocation>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CoreIssuedWorldSealProjection {
    identity: u64,
    router: RouterIdentity,
    incarnation: HistoryIncarnation,
}

pub(crate) struct CoreIssuedWorldSeal {
    projection: CoreIssuedWorldSealProjection,
}

#[doc(hidden)]
pub struct HistoryAdmissionWorldSeal {
    inner: CoreIssuedWorldSeal,
}

#[doc(hidden)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoryAdmissionStamp {
    world: CoreIssuedWorldSealProjection,
    router: RouterIdentity,
    incarnation: HistoryIncarnation,
    published_revision: HistoryRevision,
    can_back: bool,
}

#[doc(hidden)]
pub struct BackAdmissionToken {
    verified_stamp: HistoryAdmissionStamp,
    live_revision_at_prepare: HistoryRevision,
}

#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrepareBackAdmissionErrorKind {
    ForeignStamp,
    AuthorityRecovering,
    NoBackEntry,
}

#[doc(hidden)]
pub struct BackAcceptedReservationView {
    inner: BackAcceptedReservationProjection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BackAcceptedReservationProjection {
    world: CoreIssuedWorldSealProjection,
    router: RouterIdentity,
    history: HistoryIncarnation,
    request: NavigationRequestId,
    transaction: TransactionId,
    cleanup_batch: CleanupBatchId,
    branch: CoreAcceptedBranchReservationProjection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CoreAcceptedBranchReservationProjection {
    identity: CoreAcceptedBranchReservationIdentity,
    pool_capacity: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CoreAcceptedBranchReservationIdentity(TransactionId);

#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnqueueBackErrorKind {
    AuthorityRecovering,
    Exhausted,
}

#[doc(hidden)]
pub struct AcceptedBackReservation {
    request: NavigationRequestId,
    accepted: BackAcceptedReservationView,
}

impl AcceptedBackReservation {
    pub fn request(&self) -> NavigationRequestId {
        self.request
    }

    pub fn into_parts(self) -> (NavigationRequestId, BackAcceptedReservationView) {
        (self.request, self.accepted)
    }
}

#[doc(hidden)]
pub struct BackResolutionScopeView {
    inner: BackResolutionScopeProjection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BackResolutionScopeProjection {
    world: CoreIssuedWorldSealProjection,
    router: RouterIdentity,
    history: HistoryIncarnation,
    request: NavigationRequestId,
    transaction: TransactionId,
    cleanup_batch: CleanupBatchId,
    branch: CoreAcceptedBranchReservationProjection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BackResolutionEvidenceProjection {
    expected: Option<BackAcceptedReservationProjection>,
    observed: BackResolutionScopeProjection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CoreFrozenHistorySourceProjection {
    incarnation: HistoryIncarnation,
    revision: HistoryRevision,
    cursor: Option<usize>,
    entry_ids: Vec<HistoryEntryId>,
    locations: Vec<RouteLocation>,
}

#[doc(hidden)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackResolutionEvidence {
    inner: BackResolutionEvidenceProjection,
}

#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BackResolutionEvidenceClassification {
    Current,
    Foreign,
    ConsumedReplay,
}

#[allow(dead_code)]
struct CoreBackTargetExecutionOwner {
    scope: BackResolutionScopeProjection,
    reservation: ReservedBranchAttempt,
    destination: BackDestination,
    preparation_baseline: BackTargetPreparationBaseline,
}

#[doc(hidden)]
pub struct BackTargetExecutionOwner {
    inner: CoreBackTargetExecutionOwner,
}

#[allow(dead_code)]
struct CoreBackNoTargetCandidate {
    scope: BackResolutionScopeProjection,
    reservation: ReservedBranchAttempt,
    source: FrozenHistorySource,
}

#[doc(hidden)]
pub struct BackNoTargetCandidate {
    inner: CoreBackNoTargetCandidate,
}

#[allow(dead_code)]
struct CoreBackResolutionContradiction {
    scope: BackResolutionScopeProjection,
    reservation: ReservedBranchAttempt,
    contradiction: BackPreviewContradiction,
}

#[doc(hidden)]
pub struct BackResolutionContradiction {
    inner: CoreBackResolutionContradiction,
}

#[allow(dead_code)]
struct CoreVerifiedBackTargetExecutionOwner {
    raw: CoreBackTargetExecutionOwner,
}

#[doc(hidden)]
pub struct VerifiedBackTargetExecutionOwner {
    inner: CoreVerifiedBackTargetExecutionOwner,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BackTargetPreparationBaseline {
    ready: bool,
    host: Option<HostVersion>,
    top_slot: Option<RouteSlotId>,
    active: ActivePresentation,
    active_branch: Option<RouteBranchFingerprint>,
    active_child_slots: Vec<LogicalChildSlotBinding>,
}

impl BackTargetPreparationBaseline {
    pub(crate) fn new(
        ready: bool,
        host: Option<HostVersion>,
        top_slot: Option<RouteSlotId>,
        active: ActivePresentation,
        active_branch: Option<RouteBranchFingerprint>,
        active_child_slots: Vec<LogicalChildSlotBinding>,
    ) -> Self {
        Self {
            ready,
            host,
            top_slot,
            active,
            active_branch,
            active_child_slots,
        }
    }

    pub(crate) fn classify_refreeze(
        &self,
        observed: &Self,
    ) -> Result<(), BackTargetAuthorityLossCause> {
        if self.ready != observed.ready
            || self.host != observed.host
            || self.top_slot != observed.top_slot
        {
            return Err(BackTargetAuthorityLossCause::Host);
        }
        if self.active != observed.active
            || self.active_branch != observed.active_branch
            || self.active_child_slots != observed.active_child_slots
        {
            return Err(BackTargetAuthorityLossCause::ActiveBaseline);
        }
        Ok(())
    }
}

#[allow(dead_code)]
struct CoreVerifiedBackNoTargetCandidate {
    raw: CoreBackNoTargetCandidate,
}

#[doc(hidden)]
pub struct VerifiedBackNoTargetCandidate {
    inner: CoreVerifiedBackNoTargetCandidate,
}

#[allow(dead_code)]
struct CoreVerifiedBackResolutionContradiction {
    raw: CoreBackResolutionContradiction,
}

#[doc(hidden)]
pub struct VerifiedBackResolutionContradiction {
    inner: CoreVerifiedBackResolutionContradiction,
}

#[allow(dead_code)]
struct CoreBackTargetPermanentOwner {
    scope: BackResolutionScopeProjection,
    reservation: ReservedBranchAttempt,
    cause: CoreBackTargetPermanentCause,
}

#[allow(dead_code)]
enum CoreBackTargetPermanentCause {
    SourceDrift(CoreBackTargetSourceDriftOwner),
    AuthorityLost {
        destination: BackDestination,
        cause: BackTargetAuthorityLossCause,
        expected: BackTargetPreparationBaseline,
        observed: BackTargetPreparationBaseline,
    },
}

#[allow(dead_code)]
enum CoreBackTargetSourceDriftOwner {
    Refreeze {
        destination: BackDestination,
        observed_source: FrozenHistorySource,
        expected: BackTargetPreparationBaseline,
        observed: BackTargetPreparationBaseline,
    },
    Capability {
        drift: HistorySourceDrift,
        expected: BackTargetPreparationBaseline,
        observed: BackTargetPreparationBaseline,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BackTargetAuthorityLossCause {
    Host,
    TargetMatch,
    MaximumDepth,
    ActiveBaseline,
    Generation,
    ViewBinding,
    RequestRecord,
}

#[doc(hidden)]
pub struct BackTargetPermanentOwner {
    inner: CoreBackTargetPermanentOwner,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CoreBackTargetPermanentTerminalProjection {
    scope: BackResolutionScopeProjection,
    cause: CoreBackTargetPermanentTerminalCauseProjection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum CoreBackTargetPermanentTerminalCauseProjection {
    SourceRefreeze {
        expected_source: CoreFrozenHistorySourceProjection,
        observed_source: CoreFrozenHistorySourceProjection,
        expected_baseline: BackTargetPreparationBaseline,
        observed_baseline: BackTargetPreparationBaseline,
    },
    SourceCapability {
        expected_source: CoreFrozenHistorySourceProjection,
        observed_source: CoreFrozenHistorySourceProjection,
        expected_baseline: BackTargetPreparationBaseline,
        observed_baseline: BackTargetPreparationBaseline,
    },
    AuthorityLost {
        cause: BackTargetAuthorityLossCause,
        expected_and_observed_source: CoreFrozenHistorySourceProjection,
        expected_baseline: BackTargetPreparationBaseline,
        observed_baseline: BackTargetPreparationBaseline,
    },
}

#[doc(hidden)]
pub struct BackTargetPermanentTerminalEvidence {
    inner: CoreBackTargetPermanentTerminalProjection,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BackTargetPermanentCauseSnapshot {
    SourceRefreeze,
    SourceCapability,
    AuthorityLost(BackTargetAuthorityLossCause),
}

#[cfg(test)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BackTargetPermanentEvidenceSnapshot {
    scope: BackResolutionScopeProjection,
    cause: BackTargetPermanentCauseSnapshot,
    expected_source: CoreFrozenHistorySourceProjection,
    observed_source: CoreFrozenHistorySourceProjection,
    expected_baseline: BackTargetPreparationBaseline,
    observed_baseline: BackTargetPreparationBaseline,
}

#[cfg(test)]
impl BackTargetPermanentEvidenceSnapshot {
    pub(crate) const fn cause(&self) -> BackTargetPermanentCauseSnapshot {
        self.cause
    }

    pub(crate) fn sources(
        &self,
    ) -> (
        &CoreFrozenHistorySourceProjection,
        &CoreFrozenHistorySourceProjection,
    ) {
        (&self.expected_source, &self.observed_source)
    }

    pub(crate) fn baselines(
        &self,
    ) -> (
        &BackTargetPreparationBaseline,
        &BackTargetPreparationBaseline,
    ) {
        (&self.expected_baseline, &self.observed_baseline)
    }
}

#[allow(dead_code)]
struct CoreBackNoTargetSourceDriftOwner {
    candidate: CoreVerifiedBackNoTargetCandidate,
    observed: FrozenHistorySource,
}

#[doc(hidden)]
pub struct BackNoTargetSourceDriftOwner {
    inner: CoreBackNoTargetSourceDriftOwner,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CoreBackNoTargetSourceDriftProjection {
    scope: BackResolutionScopeProjection,
    expected: CoreFrozenHistorySourceProjection,
    observed: CoreFrozenHistorySourceProjection,
}

#[doc(hidden)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackNoTargetSourceDriftEvidence {
    inner: CoreBackNoTargetSourceDriftProjection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CoreBackNoTargetSourceDriftTerminalProjection {
    drift: CoreBackNoTargetSourceDriftProjection,
}

#[doc(hidden)]
pub struct BackNoTargetSourceDriftTerminalEvidence {
    inner: CoreBackNoTargetSourceDriftTerminalProjection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CoreBackContradictionTerminalProjection {
    scope: BackResolutionScopeProjection,
    expected: CoreFrozenHistorySourceProjection,
    observed: CoreFrozenHistorySourceProjection,
}

#[doc(hidden)]
pub struct BackContradictionTerminalEvidence {
    inner: CoreBackContradictionTerminalProjection,
}

#[doc(hidden)]
pub enum BackResolutionOwnerHandoff {
    Target(BackTargetExecutionOwner),
    NoTarget(BackNoTargetCandidate),
    Contradiction(BackResolutionContradiction),
}

#[doc(hidden)]
pub enum VerifiedBackResolutionOwnerHandoff {
    Target(VerifiedBackTargetExecutionOwner),
    NoTarget(VerifiedBackNoTargetCandidate),
    Contradiction(VerifiedBackResolutionContradiction),
}

#[doc(hidden)]
pub struct VerifiedBackResolutionHandoff {
    accepted: BackAcceptedReservationView,
    handoff: VerifiedBackResolutionOwnerHandoff,
}

#[doc(hidden)]
pub enum BackResolutionVerifyOutcome {
    Matched(VerifiedBackResolutionHandoff),
    Mismatch {
        accepted: BackAcceptedReservationView,
        handoff: BackResolutionOwnerHandoff,
        evidence: BackResolutionEvidence,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ClaimHistoryAdmissionWorldError {
    Exhausted,
    AlreadyClaimed,
}

impl CoreIssuedWorldSeal {
    pub(crate) fn issue(
        router: RouterIdentity,
        incarnation: HistoryIncarnation,
    ) -> Result<Self, ClaimHistoryAdmissionWorldError> {
        static NEXT_HISTORY_WORLD_IDENTITY: AtomicU64 = AtomicU64::new(1);

        let identity = NEXT_HISTORY_WORLD_IDENTITY
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(1).filter(|_| current != 0)
            })
            .map_err(|_| ClaimHistoryAdmissionWorldError::Exhausted)?;
        Ok(Self {
            projection: CoreIssuedWorldSealProjection {
                identity,
                router,
                incarnation,
            },
        })
    }

    pub(crate) const fn projection(&self) -> CoreIssuedWorldSealProjection {
        self.projection
    }
}

impl HistoryAdmissionWorldSeal {
    pub(crate) const fn issued(inner: CoreIssuedWorldSeal) -> Self {
        Self { inner }
    }

    pub(crate) const fn projection(&self) -> CoreIssuedWorldSealProjection {
        self.inner.projection()
    }
}

impl HistoryAdmissionStamp {
    pub(crate) const fn issued(
        world: CoreIssuedWorldSealProjection,
        router: RouterIdentity,
        incarnation: HistoryIncarnation,
        published_revision: HistoryRevision,
        can_back: bool,
    ) -> Self {
        Self {
            world,
            router,
            incarnation,
            published_revision,
            can_back,
        }
    }

    pub(crate) fn matches(
        &self,
        world: CoreIssuedWorldSealProjection,
        router: RouterIdentity,
        incarnation: HistoryIncarnation,
    ) -> bool {
        self.world == world && self.router == router && self.incarnation == incarnation
    }

    pub(crate) const fn can_back(&self) -> bool {
        self.can_back
    }

    pub(crate) const fn published_revision(&self) -> HistoryRevision {
        self.published_revision
    }
}

impl BackAdmissionToken {
    pub(crate) fn issued(
        verified_stamp: &HistoryAdmissionStamp,
        live_revision_at_prepare: HistoryRevision,
    ) -> Self {
        Self {
            verified_stamp: verified_stamp.clone(),
            live_revision_at_prepare,
        }
    }

    pub(crate) fn matches(
        &self,
        world: CoreIssuedWorldSealProjection,
        router: RouterIdentity,
        incarnation: HistoryIncarnation,
        live_revision: HistoryRevision,
    ) -> bool {
        self.verified_stamp.matches(world, router, incarnation)
            && self.live_revision_at_prepare == live_revision
    }
}

impl CoreAcceptedBranchReservationProjection {
    fn from_reservation(reservation: &ReservedBranchAttempt) -> Self {
        Self {
            identity: CoreAcceptedBranchReservationIdentity(reservation.transaction()),
            pool_capacity: reservation.frames().len(),
        }
    }
}

impl AcceptedBackReservation {
    pub(crate) fn accepted(
        world: CoreIssuedWorldSealProjection,
        router: RouterIdentity,
        history: HistoryIncarnation,
        reservation: &ReservedBranchAttempt,
    ) -> Self {
        let request = reservation.request();
        Self {
            request,
            accepted: BackAcceptedReservationView {
                inner: BackAcceptedReservationProjection {
                    world,
                    router,
                    history,
                    request,
                    transaction: reservation.transaction(),
                    cleanup_batch: reservation.cleanup_batch(),
                    branch: CoreAcceptedBranchReservationProjection::from_reservation(reservation),
                },
            },
        }
    }
}

impl BackResolutionScopeProjection {
    fn from_reservation(
        world: CoreIssuedWorldSealProjection,
        router: RouterIdentity,
        history: HistoryIncarnation,
        reservation: &ReservedBranchAttempt,
    ) -> Self {
        Self {
            world,
            router,
            history,
            request: reservation.request(),
            transaction: reservation.transaction(),
            cleanup_batch: reservation.cleanup_batch(),
            branch: CoreAcceptedBranchReservationProjection::from_reservation(reservation),
        }
    }

    pub(crate) fn matches_engine_scope(
        &self,
        world: Option<CoreIssuedWorldSealProjection>,
        router: RouterIdentity,
        history: HistoryIncarnation,
    ) -> bool {
        world == Some(self.world)
            && self.router == router
            && self.history == history
            && self.request.router_identity() == router
    }

    pub(crate) const fn request(&self) -> NavigationRequestId {
        self.request
    }

    pub(crate) fn matches_execution(&self, execution: &HistoryExecutionBindingProjection) -> bool {
        self.router == execution.router
            && self.request == execution.request
            && self.transaction == execution.transaction
            && self.cleanup_batch == execution.cleanup_batch
            && self.branch.identity.0 == execution.transaction
    }
}

impl BackAcceptedReservationView {
    pub fn matches_resolution_scope(&self, observed: &BackResolutionScopeView) -> bool {
        let expected = BackResolutionScopeProjection {
            world: self.inner.world,
            router: self.inner.router,
            history: self.inner.history,
            request: self.inner.request,
            transaction: self.inner.transaction,
            cleanup_batch: self.inner.cleanup_batch,
            branch: self.inner.branch.clone(),
        };
        expected == observed.inner
    }

    pub(crate) fn matches(
        &self,
        world: Option<CoreIssuedWorldSealProjection>,
        router: RouterIdentity,
        history: HistoryIncarnation,
        observed: &BackResolutionScopeProjection,
    ) -> bool {
        let expected = BackResolutionScopeProjection {
            world: self.inner.world,
            router: self.inner.router,
            history: self.inner.history,
            request: self.inner.request,
            transaction: self.inner.transaction,
            cleanup_batch: self.inner.cleanup_batch,
            branch: self.inner.branch.clone(),
        };
        world == Some(self.inner.world)
            && self.inner.router == router
            && self.inner.history == history
            && &expected == observed
    }
}

impl BackResolutionEvidence {
    pub(crate) fn mismatch(
        accepted: &BackAcceptedReservationView,
        handoff: &BackResolutionOwnerHandoff,
    ) -> Self {
        Self {
            inner: BackResolutionEvidenceProjection {
                expected: Some(accepted.inner.clone()),
                observed: handoff.scope_projection().clone(),
            },
        }
    }

    pub(crate) fn observed(scope: &BackResolutionScopeProjection) -> Self {
        Self {
            inner: BackResolutionEvidenceProjection {
                expected: None,
                observed: scope.clone(),
            },
        }
    }

    pub(crate) fn is_observed(&self) -> bool {
        self.inner.expected.is_none()
    }

    pub(crate) const fn request(&self) -> NavigationRequestId {
        self.inner.observed.request
    }

    pub(crate) fn matches_engine_scope(
        &self,
        world: Option<CoreIssuedWorldSealProjection>,
        router: RouterIdentity,
        history: HistoryIncarnation,
    ) -> bool {
        self.is_observed()
            && world == Some(self.inner.observed.world)
            && self.inner.observed.router == router
            && self.inner.observed.history == history
            && self.inner.observed.request.router_identity() == router
    }
}

impl CoreFrozenHistorySourceProjection {
    pub(crate) fn project(source: &FrozenHistorySource) -> Self {
        Self {
            incarnation: source.incarnation,
            revision: source.revision,
            cursor: source.cursor,
            entry_ids: source.entry_ids.clone(),
            locations: source.locations.clone(),
        }
    }
}

impl VerifiedBackTargetExecutionOwner {
    pub(crate) fn evidence(&self) -> BackResolutionEvidence {
        BackResolutionEvidence::observed(&self.inner.raw.scope)
    }

    pub(crate) const fn target_location(&self) -> &RouteLocation {
        &self.inner.raw.destination.target_location
    }

    pub(crate) fn matches_receiver(
        &self,
        world: Option<CoreIssuedWorldSealProjection>,
        router: RouterIdentity,
        history: HistoryIncarnation,
    ) -> bool {
        let scope = &self.inner.raw.scope;
        scope.matches_engine_scope(world, router, history)
            && scope
                == &BackResolutionScopeProjection::from_reservation(
                    scope.world,
                    scope.router,
                    scope.history,
                    &self.inner.raw.reservation,
                )
    }

    pub(crate) fn into_execution_parts(
        self,
    ) -> (
        BackResolutionScopeProjection,
        ReservedBranchAttempt,
        BackDestination,
        BackTargetPreparationBaseline,
    ) {
        let CoreVerifiedBackTargetExecutionOwner {
            raw:
                CoreBackTargetExecutionOwner {
                    scope,
                    reservation,
                    destination,
                    preparation_baseline,
                },
        } = self.inner;
        (scope, reservation, destination, preparation_baseline)
    }

    #[cfg(test)]
    pub(crate) fn testing_truncate_reserved_frames(&mut self, len: usize) {
        self.inner.raw.reservation.testing_truncate_frames(len);
    }
}

impl VerifiedBackNoTargetCandidate {
    pub(crate) fn evidence(&self) -> BackResolutionEvidence {
        BackResolutionEvidence::observed(&self.inner.raw.scope)
    }

    pub(crate) fn matches_receiver(
        &self,
        world: Option<CoreIssuedWorldSealProjection>,
        router: RouterIdentity,
        history: HistoryIncarnation,
    ) -> bool {
        let scope = &self.inner.raw.scope;
        world == Some(scope.world)
            && scope.router == router
            && scope.history == history
            && scope.request.router_identity() == router
    }

    pub(crate) const fn source(&self) -> &FrozenHistorySource {
        &self.inner.raw.source
    }

    pub(crate) fn into_source_drift_owner(
        self,
        observed: FrozenHistorySource,
    ) -> BackNoTargetSourceDriftOwner {
        BackNoTargetSourceDriftOwner {
            inner: CoreBackNoTargetSourceDriftOwner {
                candidate: self.inner,
                observed,
            },
        }
    }
}

impl VerifiedBackResolutionContradiction {
    pub(crate) fn evidence(&self) -> BackResolutionEvidence {
        BackResolutionEvidence::observed(&self.inner.raw.scope)
    }
}

impl BackTargetPermanentOwner {
    pub(crate) fn source_drift(
        scope: BackResolutionScopeProjection,
        reservation: ReservedBranchAttempt,
        drift: HistorySourceDrift,
        expected: BackTargetPreparationBaseline,
        observed: BackTargetPreparationBaseline,
    ) -> Self {
        Self {
            inner: CoreBackTargetPermanentOwner {
                scope,
                reservation,
                cause: CoreBackTargetPermanentCause::SourceDrift(
                    CoreBackTargetSourceDriftOwner::Capability {
                        drift,
                        expected,
                        observed,
                    },
                ),
            },
        }
    }

    pub(crate) fn source_refreeze_drift(
        scope: BackResolutionScopeProjection,
        reservation: ReservedBranchAttempt,
        destination: BackDestination,
        observed_source: FrozenHistorySource,
        expected: BackTargetPreparationBaseline,
        observed: BackTargetPreparationBaseline,
    ) -> Self {
        Self {
            inner: CoreBackTargetPermanentOwner {
                scope,
                reservation,
                cause: CoreBackTargetPermanentCause::SourceDrift(
                    CoreBackTargetSourceDriftOwner::Refreeze {
                        destination,
                        observed_source,
                        expected,
                        observed,
                    },
                ),
            },
        }
    }

    pub(crate) fn authority_lost(
        scope: BackResolutionScopeProjection,
        reservation: ReservedBranchAttempt,
        destination: BackDestination,
        cause: BackTargetAuthorityLossCause,
        expected: BackTargetPreparationBaseline,
        observed: BackTargetPreparationBaseline,
    ) -> Self {
        Self {
            inner: CoreBackTargetPermanentOwner {
                scope,
                reservation,
                cause: CoreBackTargetPermanentCause::AuthorityLost {
                    destination,
                    cause,
                    expected,
                    observed,
                },
            },
        }
    }

    pub(crate) fn evidence(&self) -> BackResolutionEvidence {
        BackResolutionEvidence::observed(&self.inner.scope)
    }

    pub(crate) const fn request(&self) -> NavigationRequestId {
        self.inner.scope.request
    }

    pub(crate) fn resolution_evidence(&self) -> BackResolutionEvidence {
        BackResolutionEvidence::observed(&self.inner.scope)
    }

    pub(crate) fn matches_receiver(
        &self,
        world: Option<CoreIssuedWorldSealProjection>,
        router: RouterIdentity,
        history: HistoryIncarnation,
    ) -> bool {
        self.inner
            .scope
            .matches_engine_scope(world, router, history)
            && self.inner.scope
                == BackResolutionScopeProjection::from_reservation(
                    self.inner.scope.world,
                    self.inner.scope.router,
                    self.inner.scope.history,
                    &self.inner.reservation,
                )
            && self.source_facts().is_some()
    }

    #[cfg(test)]
    pub(crate) fn testing_truncate_reserved_frames(&mut self, len: usize) {
        self.inner.reservation.testing_truncate_frames(len);
    }

    pub(crate) fn target_location(&self) -> Option<&RouteLocation> {
        self.source_facts().map(|(_, _, target)| target)
    }

    pub(crate) fn observed_source_is_current(&self, history: &CanonicalHistory) -> bool {
        self.source_facts()
            .is_some_and(|(_, observed, _)| observed == &history.freeze_source())
    }

    pub(crate) fn observed_baseline_is_current(
        &self,
        baseline: &BackTargetPreparationBaseline,
    ) -> bool {
        self.observed_baseline()
            .is_some_and(|observed| observed == baseline)
    }

    pub(crate) fn into_terminal_evidence(
        self,
    ) -> Result<BackTargetPermanentTerminalEvidence, Self> {
        let Some(inner) = self.terminal_projection() else {
            return Err(self);
        };
        Ok(BackTargetPermanentTerminalEvidence { inner })
    }

    fn source_facts(&self) -> Option<(&FrozenHistorySource, &FrozenHistorySource, &RouteLocation)> {
        match &self.inner.cause {
            CoreBackTargetPermanentCause::SourceDrift(
                CoreBackTargetSourceDriftOwner::Refreeze {
                    destination,
                    observed_source,
                    ..
                },
            ) => Some((
                &destination.source,
                observed_source,
                &destination.target_location,
            )),
            CoreBackTargetPermanentCause::SourceDrift(
                CoreBackTargetSourceDriftOwner::Capability { drift, .. },
            ) => match &drift.capability.destination {
                FrozenHistoryDestination::Back(destination)
                    if drift.capability.source.cursor == Some(destination.source_cursor)
                        && drift
                            .capability
                            .source
                            .entry_ids
                            .get(destination.source_cursor)
                            == Some(&destination.source_entry)
                        && destination.target_cursor.checked_add(1)
                            == Some(destination.source_cursor)
                        && drift
                            .capability
                            .source
                            .entry_ids
                            .get(destination.target_cursor)
                            == Some(&destination.target_entry)
                        && drift
                            .capability
                            .source
                            .locations
                            .get(destination.target_cursor)
                            == Some(&destination.target) =>
                {
                    Some((
                        &drift.capability.source,
                        &drift.observed,
                        &destination.target,
                    ))
                }
                _ => None,
            },
            CoreBackTargetPermanentCause::AuthorityLost { destination, .. } => Some((
                &destination.source,
                &destination.source,
                &destination.target_location,
            )),
        }
    }

    fn observed_baseline(&self) -> Option<&BackTargetPreparationBaseline> {
        match &self.inner.cause {
            CoreBackTargetPermanentCause::SourceDrift(
                CoreBackTargetSourceDriftOwner::Refreeze { observed, .. }
                | CoreBackTargetSourceDriftOwner::Capability { observed, .. },
            )
            | CoreBackTargetPermanentCause::AuthorityLost { observed, .. } => Some(observed),
        }
    }

    fn terminal_projection(&self) -> Option<CoreBackTargetPermanentTerminalProjection> {
        let (expected_source, observed_source, _) = self.source_facts()?;
        let cause = match &self.inner.cause {
            CoreBackTargetPermanentCause::SourceDrift(
                CoreBackTargetSourceDriftOwner::Refreeze {
                    expected, observed, ..
                },
            ) => CoreBackTargetPermanentTerminalCauseProjection::SourceRefreeze {
                expected_source: CoreFrozenHistorySourceProjection::project(expected_source),
                observed_source: CoreFrozenHistorySourceProjection::project(observed_source),
                expected_baseline: expected.clone(),
                observed_baseline: observed.clone(),
            },
            CoreBackTargetPermanentCause::SourceDrift(
                CoreBackTargetSourceDriftOwner::Capability {
                    expected, observed, ..
                },
            ) => CoreBackTargetPermanentTerminalCauseProjection::SourceCapability {
                expected_source: CoreFrozenHistorySourceProjection::project(expected_source),
                observed_source: CoreFrozenHistorySourceProjection::project(observed_source),
                expected_baseline: expected.clone(),
                observed_baseline: observed.clone(),
            },
            CoreBackTargetPermanentCause::AuthorityLost {
                destination,
                cause,
                expected,
                observed,
            } => CoreBackTargetPermanentTerminalCauseProjection::AuthorityLost {
                cause: *cause,
                expected_and_observed_source: CoreFrozenHistorySourceProjection::project(
                    &destination.source,
                ),
                expected_baseline: expected.clone(),
                observed_baseline: observed.clone(),
            },
        };
        Some(CoreBackTargetPermanentTerminalProjection {
            scope: self.inner.scope.clone(),
            cause,
        })
    }

    #[cfg(test)]
    pub(crate) fn testing_evidence_snapshot(&self) -> BackTargetPermanentEvidenceSnapshot {
        let (cause, expected_source, observed_source, expected_baseline, observed_baseline) =
            match &self.inner.cause {
                CoreBackTargetPermanentCause::SourceDrift(
                    CoreBackTargetSourceDriftOwner::Refreeze {
                        destination,
                        observed_source,
                        expected,
                        observed,
                    },
                ) => (
                    BackTargetPermanentCauseSnapshot::SourceRefreeze,
                    &destination.source,
                    observed_source,
                    expected,
                    observed,
                ),
                CoreBackTargetPermanentCause::SourceDrift(
                    CoreBackTargetSourceDriftOwner::Capability {
                        drift,
                        expected,
                        observed,
                    },
                ) => (
                    BackTargetPermanentCauseSnapshot::SourceCapability,
                    &drift.capability.source,
                    &drift.observed,
                    expected,
                    observed,
                ),
                CoreBackTargetPermanentCause::AuthorityLost {
                    destination,
                    cause,
                    expected,
                    observed,
                } => (
                    BackTargetPermanentCauseSnapshot::AuthorityLost(*cause),
                    &destination.source,
                    &destination.source,
                    expected,
                    observed,
                ),
            };
        BackTargetPermanentEvidenceSnapshot {
            scope: self.inner.scope.clone(),
            cause,
            expected_source: CoreFrozenHistorySourceProjection::project(expected_source),
            observed_source: CoreFrozenHistorySourceProjection::project(observed_source),
            expected_baseline: expected_baseline.clone(),
            observed_baseline: observed_baseline.clone(),
        }
    }
}

#[cfg(test)]
impl BackTargetPermanentTerminalEvidence {
    pub(crate) fn testing_evidence_snapshot(&self) -> BackTargetPermanentEvidenceSnapshot {
        let (cause, expected_source, observed_source, expected_baseline, observed_baseline) =
            match &self.inner.cause {
                CoreBackTargetPermanentTerminalCauseProjection::SourceRefreeze {
                    expected_source,
                    observed_source,
                    expected_baseline,
                    observed_baseline,
                } => (
                    BackTargetPermanentCauseSnapshot::SourceRefreeze,
                    expected_source,
                    observed_source,
                    expected_baseline,
                    observed_baseline,
                ),
                CoreBackTargetPermanentTerminalCauseProjection::SourceCapability {
                    expected_source,
                    observed_source,
                    expected_baseline,
                    observed_baseline,
                } => (
                    BackTargetPermanentCauseSnapshot::SourceCapability,
                    expected_source,
                    observed_source,
                    expected_baseline,
                    observed_baseline,
                ),
                CoreBackTargetPermanentTerminalCauseProjection::AuthorityLost {
                    cause,
                    expected_and_observed_source,
                    expected_baseline,
                    observed_baseline,
                } => (
                    BackTargetPermanentCauseSnapshot::AuthorityLost(*cause),
                    expected_and_observed_source,
                    expected_and_observed_source,
                    expected_baseline,
                    observed_baseline,
                ),
            };
        BackTargetPermanentEvidenceSnapshot {
            scope: self.inner.scope.clone(),
            cause,
            expected_source: expected_source.clone(),
            observed_source: observed_source.clone(),
            expected_baseline: expected_baseline.clone(),
            observed_baseline: observed_baseline.clone(),
        }
    }
}

impl BackNoTargetSourceDriftOwner {
    fn projection(&self) -> CoreBackNoTargetSourceDriftProjection {
        CoreBackNoTargetSourceDriftProjection {
            scope: self.inner.candidate.raw.scope.clone(),
            expected: CoreFrozenHistorySourceProjection::project(&self.inner.candidate.raw.source),
            observed: CoreFrozenHistorySourceProjection::project(&self.inner.observed),
        }
    }

    pub fn evidence(&self) -> BackNoTargetSourceDriftEvidence {
        BackNoTargetSourceDriftEvidence {
            inner: self.projection(),
        }
    }

    pub(crate) const fn request(&self) -> NavigationRequestId {
        self.inner.candidate.raw.scope.request
    }

    pub(crate) fn resolution_evidence(&self) -> BackResolutionEvidence {
        BackResolutionEvidence::observed(&self.inner.candidate.raw.scope)
    }

    pub(crate) fn matches_receiver(
        &self,
        world: Option<CoreIssuedWorldSealProjection>,
        router: RouterIdentity,
        history: HistoryIncarnation,
    ) -> bool {
        let scope = &self.inner.candidate.raw.scope;
        world == Some(scope.world)
            && scope.router == router
            && scope.history == history
            && scope.request.router_identity() == router
    }

    pub(crate) fn observed_source_is_current(&self, history: &CanonicalHistory) -> bool {
        history
            .refreeze_back_no_target_source(&self.inner.observed)
            .is_ok()
    }

    pub(crate) fn into_terminal_evidence(self) -> BackNoTargetSourceDriftTerminalEvidence {
        BackNoTargetSourceDriftTerminalEvidence {
            inner: CoreBackNoTargetSourceDriftTerminalProjection {
                drift: self.projection(),
            },
        }
    }
}

impl BackResolutionOwnerHandoff {
    pub(crate) fn resolved(
        world: CoreIssuedWorldSealProjection,
        router: RouterIdentity,
        history: HistoryIncarnation,
        reservation: ReservedBranchAttempt,
        preparation_baseline: BackTargetPreparationBaseline,
        preview: BackPreview,
    ) -> Self {
        let scope =
            BackResolutionScopeProjection::from_reservation(world, router, history, &reservation);
        match preview {
            BackPreview::Target(destination) => Self::Target(BackTargetExecutionOwner {
                inner: CoreBackTargetExecutionOwner {
                    scope,
                    reservation,
                    destination,
                    preparation_baseline,
                },
            }),
            BackPreview::NoTarget(source) => Self::NoTarget(BackNoTargetCandidate {
                inner: CoreBackNoTargetCandidate {
                    scope,
                    reservation,
                    source,
                },
            }),
            BackPreview::Contradiction(contradiction) => {
                Self::Contradiction(BackResolutionContradiction {
                    inner: CoreBackResolutionContradiction {
                        scope,
                        reservation,
                        contradiction,
                    },
                })
            }
        }
    }

    pub(crate) fn scope_projection(&self) -> &BackResolutionScopeProjection {
        match self {
            Self::Target(owner) => &owner.inner.scope,
            Self::NoTarget(candidate) => &candidate.inner.scope,
            Self::Contradiction(owner) => &owner.inner.scope,
        }
    }

    pub(crate) fn target_location(&self) -> Option<&RouteLocation> {
        match self {
            Self::Target(owner) => Some(&owner.inner.destination.target_location),
            Self::NoTarget(_) | Self::Contradiction(_) => None,
        }
    }

    pub fn scope(&self) -> BackResolutionScopeView {
        BackResolutionScopeView {
            inner: self.scope_projection().clone(),
        }
    }

    pub fn evidence(&self) -> BackResolutionEvidence {
        BackResolutionEvidence {
            inner: BackResolutionEvidenceProjection {
                expected: None,
                observed: self.scope_projection().clone(),
            },
        }
    }

    pub(crate) fn into_verified(
        self,
        accepted: BackAcceptedReservationView,
    ) -> VerifiedBackResolutionHandoff {
        let handoff = match self {
            Self::Target(owner) => {
                VerifiedBackResolutionOwnerHandoff::Target(VerifiedBackTargetExecutionOwner {
                    inner: CoreVerifiedBackTargetExecutionOwner { raw: owner.inner },
                })
            }
            Self::NoTarget(candidate) => {
                VerifiedBackResolutionOwnerHandoff::NoTarget(VerifiedBackNoTargetCandidate {
                    inner: CoreVerifiedBackNoTargetCandidate {
                        raw: candidate.inner,
                    },
                })
            }
            Self::Contradiction(owner) => VerifiedBackResolutionOwnerHandoff::Contradiction(
                VerifiedBackResolutionContradiction {
                    inner: CoreVerifiedBackResolutionContradiction { raw: owner.inner },
                },
            ),
        };
        VerifiedBackResolutionHandoff { accepted, handoff }
    }
}

impl VerifiedBackResolutionHandoff {
    pub fn into_parts(
        self,
    ) -> (
        BackAcceptedReservationView,
        VerifiedBackResolutionOwnerHandoff,
    ) {
        (self.accepted, self.handoff)
    }
}

pub(crate) struct CanonicalHistory {
    projection: MemoryHistory,
    entry_ids: Vec<HistoryEntryId>,
    incarnation: HistoryIncarnation,
    revision: HistoryRevision,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum HistoryEffect {
    Push(RouteLocation),
    Replace(RouteLocation),
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct NormalAttemptIdentity {
    host: RouteHostId,
    host_epoch: u64,
    slot: RouteSlotId,
    root_frame_task: FrameTaskId,
    root_frame: RouteFrameId,
    generation: RouteGeneration,
    origin: FrameOrigin,
    branch_index: usize,
}

impl NormalAttemptIdentity {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        host: RouteHostId,
        host_epoch: u64,
        slot: RouteSlotId,
        root_frame_task: FrameTaskId,
        root_frame: RouteFrameId,
        generation: RouteGeneration,
        origin: FrameOrigin,
        branch_index: usize,
    ) -> Self {
        Self {
            host,
            host_epoch,
            slot,
            root_frame_task,
            root_frame,
            generation,
            origin,
            branch_index,
        }
    }

    fn project(&self) -> NormalAttemptIdentityProjection {
        NormalAttemptIdentityProjection {
            host: self.host,
            host_epoch: self.host_epoch,
            slot: self.slot,
            root_frame_task: self.root_frame_task,
            root_frame: self.root_frame,
            generation: self.generation,
            origin: self.origin.clone(),
            branch_index: self.branch_index,
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct HistoryExecutionBinding {
    router: RouterIdentity,
    request: NavigationRequestId,
    normal_attempt: NormalAttemptIdentity,
    transaction: TransactionId,
    cleanup_batch: CleanupBatchId,
}

impl HistoryExecutionBinding {
    pub(crate) fn new(
        router: RouterIdentity,
        request: NavigationRequestId,
        normal_attempt: NormalAttemptIdentity,
        transaction: TransactionId,
        cleanup_batch: CleanupBatchId,
    ) -> Self {
        Self {
            router,
            request,
            normal_attempt,
            transaction,
            cleanup_batch,
        }
    }

    pub(crate) fn projection(&self) -> HistoryExecutionBindingProjection {
        HistoryExecutionBindingProjection {
            router: self.router,
            request: self.request,
            normal_attempt: self.normal_attempt.project(),
            transaction: self.transaction,
            cleanup_batch: self.cleanup_batch,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HistoryExecutionBindingProjection {
    router: RouterIdentity,
    request: NavigationRequestId,
    normal_attempt: NormalAttemptIdentityProjection,
    transaction: TransactionId,
    cleanup_batch: CleanupBatchId,
}

impl HistoryExecutionBindingProjection {
    pub(crate) fn observed(
        router: RouterIdentity,
        request: NavigationRequestId,
        normal_attempt: NormalAttemptIdentity,
        transaction: TransactionId,
        cleanup_batch: CleanupBatchId,
    ) -> Self {
        Self {
            router,
            request,
            normal_attempt: normal_attempt.project(),
            transaction,
            cleanup_batch,
        }
    }

    #[cfg(test)]
    pub(crate) fn testing_with_router_and_request(
        &self,
        router: RouterIdentity,
        request: NavigationRequestId,
    ) -> Self {
        let mut observed = self.clone();
        observed.router = router;
        observed.request = request;
        observed
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NormalAttemptIdentityProjection {
    host: RouteHostId,
    host_epoch: u64,
    slot: RouteSlotId,
    root_frame_task: FrameTaskId,
    root_frame: RouteFrameId,
    generation: RouteGeneration,
    origin: FrameOrigin,
    branch_index: usize,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum FrozenHistoryDestination {
    Push {
        target: RouteLocation,
    },
    Replace {
        target: RouteLocation,
        replaced: Option<HistoryEntryId>,
    },
    Back(FrozenBackDestination),
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct BackDestination {
    source: FrozenHistorySource,
    target_entry: HistoryEntryId,
    target_cursor: usize,
    target_location: RouteLocation,
}

impl BackDestination {
    pub(crate) const fn target_location(&self) -> &RouteLocation {
        &self.target_location
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct BackPreviewContradiction {
    expected: FrozenHistorySource,
    observed: FrozenHistorySource,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct FrozenBackDestination {
    source_entry: HistoryEntryId,
    source_cursor: usize,
    target_entry: HistoryEntryId,
    target_cursor: usize,
    target: RouteLocation,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum BackPreview {
    Target(BackDestination),
    NoTarget(FrozenHistorySource),
    Contradiction(BackPreviewContradiction),
}

impl BackPreview {
    pub(crate) const fn observed_source(&self) -> &FrozenHistorySource {
        match self {
            Self::Target(destination) => &destination.source,
            Self::NoTarget(source) => source,
            Self::Contradiction(contradiction) => &contradiction.observed,
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct PreparedPresentationBinding {
    execution: HistoryExecutionBindingProjection,
    presentation: ActivePresentation,
    matched: RouteBranchFingerprint,
    target_frames: Vec<RouteFrameVersion>,
    cleanup: CleanupBatchToken,
}

impl PreparedPresentationBinding {
    pub(crate) fn new(
        execution: HistoryExecutionBindingProjection,
        presentation: ActivePresentation,
        matched: RouteBranchFingerprint,
        target_frames: Vec<RouteFrameVersion>,
        cleanup: CleanupBatchToken,
    ) -> Self {
        Self {
            execution,
            presentation,
            matched,
            target_frames,
            cleanup,
        }
    }
}

pub(crate) struct HistoryCommitCapability {
    execution: HistoryExecutionBinding,
    source: FrozenHistorySource,
    destination: FrozenHistoryDestination,
}

pub(crate) struct PreparedHistoryPublication {
    execution: HistoryExecutionBinding,
    source: FrozenHistorySource,
    destination: FrozenHistoryDestination,
    presentation: PreparedPresentationBinding,
    next: CanonicalHistory,
}

impl PreparedHistoryPublication {
    pub(crate) fn matches_canonical_source(&self, history: &CanonicalHistory) -> bool {
        self.source == history.freeze_source()
    }

    pub(crate) fn execution_projection(&self) -> HistoryExecutionBindingProjection {
        self.execution.projection()
    }

    pub(crate) const fn source_revision(&self) -> HistoryRevision {
        self.source.revision
    }

    pub(crate) fn matches_sealed_projection(
        &self,
        execution: &HistoryExecutionBindingProjection,
        source_revision: HistoryRevision,
        target: &RouteLocation,
        presentation: &ActivePresentation,
        matched: &RouteBranchFingerprint,
        target_frames: &[RouteFrameVersion],
        cleanup: &CleanupBatchToken,
    ) -> bool {
        let destination_target = match &self.destination {
            FrozenHistoryDestination::Push { target }
            | FrozenHistoryDestination::Replace { target, .. } => target,
            FrozenHistoryDestination::Back(destination) => &destination.target,
        };
        self.execution.projection() == *execution
            && self.source.revision == source_revision
            && destination_target == target
            && self.presentation.execution == *execution
            && self.presentation.presentation == *presentation
            && self.presentation.matched == *matched
            && self.presentation.target_frames == target_frames
            && self.presentation.cleanup == *cleanup
            && self.next.projection.current() == Some(target)
    }

    #[cfg(test)]
    pub(crate) fn capability_snapshot(&self) -> HistoryCapabilitySnapshot {
        HistoryCapabilitySnapshot {
            owner_present: true,
            execution: Some(self.execution.projection()),
            source: Some(FrozenHistorySourceSnapshot::project(&self.source)),
            destination: Some(FrozenHistoryDestinationSnapshot::project(&self.destination)),
        }
    }
}

pub(crate) struct HistorySourceDrift {
    capability: HistoryCommitCapability,
    observed: FrozenHistorySource,
}

pub(crate) struct HistoryBindingDrift {
    capability: HistoryCommitCapability,
    observed: HistoryExecutionBindingProjection,
}

pub(crate) enum HistoryPreparationError {
    Source(HistorySourceDrift),
    Binding(HistoryBindingDrift),
}

#[cfg(test)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HistoryAuthoritySnapshot {
    projection: HistorySnapshot,
    entry_ids: Vec<HistoryEntryId>,
    revision: HistoryRevision,
}

#[cfg(test)]
impl HistoryAuthoritySnapshot {
    pub(crate) const fn revision(&self) -> HistoryRevision {
        self.revision
    }

    pub(crate) const fn committed_by(&self) -> Option<NavigationRequestId> {
        self.revision.committed_by
    }
}

#[cfg(test)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HistoryCapabilitySnapshot {
    owner_present: bool,
    execution: Option<HistoryExecutionBindingProjection>,
    source: Option<FrozenHistorySourceSnapshot>,
    destination: Option<FrozenHistoryDestinationSnapshot>,
}

#[cfg(test)]
impl HistoryCapabilitySnapshot {
    pub(crate) const fn has_owner(&self) -> bool {
        self.owner_present
    }

    pub(crate) const fn has_exactly_one_owner(&self) -> bool {
        self.has_owner()
    }

    pub(crate) const fn execution(&self) -> Option<&HistoryExecutionBindingProjection> {
        self.execution.as_ref()
    }

    pub(crate) const fn source(&self) -> Option<&FrozenHistorySourceSnapshot> {
        self.source.as_ref()
    }

    pub(crate) const fn none() -> Self {
        Self {
            owner_present: false,
            execution: None,
            source: None,
            destination: None,
        }
    }
}

#[cfg(test)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FrozenHistorySourceSnapshot {
    incarnation: HistoryIncarnation,
    revision: HistoryRevision,
    cursor: Option<usize>,
    entry_ids: Vec<HistoryEntryId>,
    locations: Vec<RouteLocation>,
}

#[cfg(test)]
impl FrozenHistorySourceSnapshot {
    fn project(source: &FrozenHistorySource) -> Self {
        Self {
            incarnation: source.incarnation,
            revision: source.revision,
            cursor: source.cursor,
            entry_ids: source.entry_ids.clone(),
            locations: source.locations.clone(),
        }
    }
}

#[cfg(test)]
#[derive(Clone, Debug, Eq, PartialEq)]
enum FrozenHistoryDestinationSnapshot {
    Push(RouteLocation),
    Replace {
        target: RouteLocation,
        replaced: Option<HistoryEntryId>,
    },
    Back {
        source_entry: HistoryEntryId,
        source_cursor: usize,
        target_entry: HistoryEntryId,
        target_cursor: usize,
        target: RouteLocation,
    },
}

#[cfg(test)]
impl FrozenHistoryDestinationSnapshot {
    fn project(destination: &FrozenHistoryDestination) -> Self {
        match destination {
            FrozenHistoryDestination::Push { target } => Self::Push(target.clone()),
            FrozenHistoryDestination::Replace { target, replaced } => Self::Replace {
                target: target.clone(),
                replaced: *replaced,
            },
            FrozenHistoryDestination::Back(destination) => Self::Back {
                source_entry: destination.source_entry,
                source_cursor: destination.source_cursor,
                target_entry: destination.target_entry,
                target_cursor: destination.target_cursor,
                target: destination.target.clone(),
            },
        }
    }
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HistoryDriftEvidenceKind {
    Source,
    Binding,
}

#[cfg(test)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HistoryDriftEvidenceSnapshot {
    kind: HistoryDriftEvidenceKind,
    capability: HistoryCapabilitySnapshot,
    expected_source: Option<FrozenHistorySourceSnapshot>,
    observed_source: Option<FrozenHistorySourceSnapshot>,
    expected_binding: Option<HistoryExecutionBindingProjection>,
    observed_binding: Option<HistoryExecutionBindingProjection>,
}

#[cfg(test)]
impl HistoryDriftEvidenceSnapshot {
    pub(crate) const fn kind(&self) -> HistoryDriftEvidenceKind {
        self.kind
    }

    pub(crate) const fn capability(&self) -> &HistoryCapabilitySnapshot {
        &self.capability
    }

    pub(crate) const fn expected_source(&self) -> Option<&FrozenHistorySourceSnapshot> {
        self.expected_source.as_ref()
    }

    pub(crate) const fn observed_source(&self) -> Option<&FrozenHistorySourceSnapshot> {
        self.observed_source.as_ref()
    }

    pub(crate) const fn expected_binding(&self) -> Option<&HistoryExecutionBindingProjection> {
        self.expected_binding.as_ref()
    }

    pub(crate) const fn observed_binding(&self) -> Option<&HistoryExecutionBindingProjection> {
        self.observed_binding.as_ref()
    }
}

impl CanonicalHistory {
    pub(crate) fn new(router: RouterIdentity) -> Self {
        let incarnation = HistoryIncarnation(router);
        Self {
            projection: MemoryHistory::new(),
            entry_ids: Vec::new(),
            incarnation,
            revision: HistoryRevision {
                incarnation,
                committed_by: None,
            },
        }
    }

    pub(crate) const fn projection(&self) -> &MemoryHistory {
        &self.projection
    }

    pub(crate) fn snapshot(&self) -> HistorySnapshot {
        HistorySnapshot::project(&self.projection)
    }

    #[cfg(test)]
    pub(crate) fn authority_snapshot(&self) -> HistoryAuthoritySnapshot {
        HistoryAuthoritySnapshot {
            projection: self.snapshot(),
            entry_ids: self.entry_ids.clone(),
            revision: self.revision,
        }
    }

    pub(crate) const fn revision(&self) -> HistoryRevision {
        self.revision
    }

    pub(crate) const fn incarnation(&self) -> HistoryIncarnation {
        self.incarnation
    }

    pub(crate) fn preview_back(&self) -> BackPreview {
        let source = self.freeze_source();
        let Some(source_cursor) = source.cursor else {
            return BackPreview::NoTarget(source);
        };
        let Some(target_cursor) = source_cursor.checked_sub(1) else {
            return BackPreview::NoTarget(source);
        };
        let Some(target_entry) = source.entry_ids.get(target_cursor).copied() else {
            return BackPreview::NoTarget(source);
        };
        let Some(target) = source.locations.get(target_cursor).cloned() else {
            return BackPreview::NoTarget(source);
        };
        BackPreview::Target(BackDestination {
            source,
            target_cursor,
            target_entry,
            target_location: target,
        })
    }

    pub(crate) fn refreeze_back_source(
        &self,
        destination: &BackDestination,
    ) -> Result<(), FrozenHistorySource> {
        let observed = self.freeze_source();
        if destination.source == observed {
            Ok(())
        } else {
            Err(observed)
        }
    }

    pub(crate) fn refreeze_back_no_target_source(
        &self,
        expected: &FrozenHistorySource,
    ) -> Result<(), FrozenHistorySource> {
        let observed = self.freeze_source();
        if expected == &observed {
            Ok(())
        } else {
            Err(observed)
        }
    }

    pub(crate) fn freeze(
        &self,
        execution: HistoryExecutionBinding,
        effect: HistoryEffect,
    ) -> HistoryCommitCapability {
        let snapshot = self.snapshot();
        let destination = match effect {
            HistoryEffect::Push(target) => FrozenHistoryDestination::Push { target },
            HistoryEffect::Replace(target) => FrozenHistoryDestination::Replace {
                target,
                replaced: snapshot
                    .cursor()
                    .and_then(|cursor| self.entry_ids.get(cursor).copied()),
            },
        };
        HistoryCommitCapability {
            execution,
            source: self.freeze_source(),
            destination,
        }
    }

    pub(crate) fn freeze_back(
        &self,
        execution: HistoryExecutionBinding,
        destination: BackDestination,
    ) -> Result<HistoryCommitCapability, HistorySourceDrift> {
        let source_cursor = destination
            .source
            .cursor
            .expect("a resolved back destination has one source cursor");
        let source_entry = destination.source.entry_ids[source_cursor];
        let frozen = FrozenHistoryDestination::Back(FrozenBackDestination {
            source_entry,
            source_cursor,
            target_entry: destination.target_entry,
            target_cursor: destination.target_cursor,
            target: destination.target_location,
        });
        let capability = HistoryCommitCapability {
            execution,
            source: destination.source,
            destination: frozen,
        };
        let observed = self.freeze_source();
        if capability.source != observed {
            return Err(HistorySourceDrift {
                capability,
                observed,
            });
        }
        Ok(capability)
    }

    pub(crate) fn prepare(
        &self,
        capability: HistoryCommitCapability,
        presentation: PreparedPresentationBinding,
    ) -> Result<PreparedHistoryPublication, HistoryPreparationError> {
        let observed_source = self.freeze_source();
        if observed_source != capability.source {
            return Err(HistoryPreparationError::Source(HistorySourceDrift {
                capability,
                observed: observed_source,
            }));
        }

        let expected_binding = capability.execution.projection();
        if presentation.execution != expected_binding {
            let observed = presentation.execution;
            return Err(HistoryPreparationError::Binding(HistoryBindingDrift {
                capability,
                observed,
            }));
        }

        let request = capability.execution.request;
        let mut projection = self.projection.clone();
        let mut entry_ids = self.entry_ids.clone();
        match &capability.destination {
            FrozenHistoryDestination::Push { target } => {
                if let Some(cursor) = capability.source.cursor {
                    entry_ids.truncate(cursor + 1);
                }
                projection.push(target.clone());
                entry_ids.push(HistoryEntryId {
                    incarnation: self.incarnation,
                    creator: request,
                });
            }
            FrozenHistoryDestination::Replace { target, replaced } => {
                let observed_replaced = capability
                    .source
                    .cursor
                    .and_then(|cursor| entry_ids.get(cursor).copied());
                if observed_replaced != *replaced {
                    return Err(HistoryPreparationError::Source(HistorySourceDrift {
                        capability,
                        observed: self.freeze_source(),
                    }));
                }
                projection.replace(target.clone());
                let replacement = HistoryEntryId {
                    incarnation: self.incarnation,
                    creator: request,
                };
                let replacement_cursor = capability.source.cursor.or(Some(0));
                match replacement_cursor {
                    Some(cursor) if cursor < entry_ids.len() => entry_ids[cursor] = replacement,
                    Some(_) => entry_ids.push(replacement),
                    None => unreachable!("replace always creates or preserves a current entry"),
                }
            }
            FrozenHistoryDestination::Back(destination) => {
                let source_entry = capability
                    .source
                    .entry_ids
                    .get(destination.source_cursor)
                    .copied();
                let target_entry = capability
                    .source
                    .entry_ids
                    .get(destination.target_cursor)
                    .copied();
                let target = capability.source.locations.get(destination.target_cursor);
                if source_entry != Some(destination.source_entry)
                    || target_entry != Some(destination.target_entry)
                    || target != Some(&destination.target)
                    || destination.target_cursor.checked_add(1) != Some(destination.source_cursor)
                {
                    return Err(HistoryPreparationError::Source(HistorySourceDrift {
                        capability,
                        observed: self.freeze_source(),
                    }));
                }
                let observed = projection.back();
                if !matches!(observed, Ok(ref target) if target == &destination.target) {
                    return Err(HistoryPreparationError::Source(HistorySourceDrift {
                        capability,
                        observed: self.freeze_source(),
                    }));
                }
            }
        }

        let next = CanonicalHistory {
            projection,
            entry_ids,
            incarnation: self.incarnation,
            revision: HistoryRevision {
                incarnation: self.incarnation,
                committed_by: Some(request),
            },
        };
        Ok(PreparedHistoryPublication {
            execution: capability.execution,
            source: capability.source,
            destination: capability.destination,
            presentation,
            next,
        })
    }

    pub(crate) fn publish(&mut self, prepared: PreparedHistoryPublication) {
        let PreparedHistoryPublication {
            execution: _,
            source: _,
            destination: _,
            presentation: _,
            next,
        } = prepared;
        *self = next;
    }

    fn freeze_source(&self) -> FrozenHistorySource {
        let snapshot = self.snapshot();
        FrozenHistorySource {
            incarnation: self.incarnation,
            revision: self.revision,
            cursor: snapshot.cursor(),
            entry_ids: self.entry_ids.clone(),
            locations: snapshot.entries().to_vec(),
        }
    }

    #[cfg(test)]
    pub(crate) fn testing_move_cursor_back(&mut self) {
        self.projection
            .back()
            .expect("the fixture has a prior canonical entry");
    }
}

impl HistoryCommitCapability {
    pub(crate) fn execution_projection(&self) -> HistoryExecutionBindingProjection {
        self.execution.projection()
    }

    #[cfg(test)]
    pub(crate) fn snapshot(&self) -> HistoryCapabilitySnapshot {
        HistoryCapabilitySnapshot {
            owner_present: true,
            execution: Some(self.execution_projection()),
            source: Some(FrozenHistorySourceSnapshot::project(&self.source)),
            destination: Some(FrozenHistoryDestinationSnapshot::project(&self.destination)),
        }
    }
}

impl HistoryPreparationError {
    pub(crate) fn evidence_is_consistent_for(&self, request: NavigationRequestId) -> bool {
        match self {
            Self::Source(drift) => {
                drift.capability.execution.request == request
                    && drift.capability.source != drift.observed
            }
            Self::Binding(drift) => {
                drift.capability.execution.request == request
                    && drift.capability.execution.projection() != drift.observed
            }
        }
    }
}

#[cfg(test)]
impl HistoryPreparationError {
    pub(crate) fn capability_snapshot(&self) -> HistoryCapabilitySnapshot {
        match self {
            Self::Source(drift) => drift.capability.snapshot(),
            Self::Binding(drift) => drift.capability.snapshot(),
        }
    }

    pub(crate) fn snapshot(&self) -> HistoryDriftEvidenceSnapshot {
        match self {
            Self::Source(drift) => HistoryDriftEvidenceSnapshot {
                kind: HistoryDriftEvidenceKind::Source,
                capability: drift.capability.snapshot(),
                expected_source: Some(FrozenHistorySourceSnapshot::project(
                    &drift.capability.source,
                )),
                observed_source: Some(FrozenHistorySourceSnapshot::project(&drift.observed)),
                expected_binding: None,
                observed_binding: None,
            },
            Self::Binding(drift) => HistoryDriftEvidenceSnapshot {
                kind: HistoryDriftEvidenceKind::Binding,
                capability: drift.capability.snapshot(),
                expected_source: None,
                observed_source: None,
                expected_binding: Some(drift.capability.execution_projection()),
                observed_binding: Some(drift.observed.clone()),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{BackPreview, CanonicalHistory, HistoryEffect, HistoryPreparationError};
    use crate::engine::{CommitError, HistoryAuthorityFixture};
    use crate::location::RouteLocation;
    use crate::trace::NavigationTraceEventKind;

    fn publish(
        history: &mut CanonicalHistory,
        fixture: &mut HistoryAuthorityFixture,
        effect: HistoryEffect,
    ) {
        let (execution, presentation) = fixture.next_history_binding();
        let capability = history.freeze(execution, effect);
        let prepared = match history.prepare(capability, presentation) {
            Ok(prepared) => prepared,
            Err(_) => panic!("an exact source and execution binding prepare once"),
        };
        history.publish(prepared);
    }

    #[test]
    fn back_between_duplicate_locations_uses_entry_identity() {
        let mut fixture = HistoryAuthorityFixture::new();
        let mut history = CanonicalHistory::new(fixture.router_identity());
        let duplicate = RouteLocation::new("/items/1");
        publish(
            &mut history,
            &mut fixture,
            HistoryEffect::Push(duplicate.clone()),
        );
        publish(
            &mut history,
            &mut fixture,
            HistoryEffect::Push(duplicate.clone()),
        );
        let source_cursor = history.snapshot().cursor().unwrap();
        let source_revision = history.revision;
        let source_entry = history.entry_ids[source_cursor];
        let target_entry = history.entry_ids[source_cursor - 1];

        let BackPreview::Target(destination) = history.preview_back() else {
            panic!("two exact entries must produce a live back target")
        };

        assert_eq!(destination.source.cursor, Some(source_cursor));
        assert_eq!(destination.source.revision, source_revision);
        assert_eq!(destination.target_cursor, source_cursor - 1);
        assert_eq!(destination.target_entry, target_entry);
        assert_eq!(destination.target_location, duplicate);
        assert_ne!(destination.target_entry, source_entry);
    }

    #[test]
    fn equivalent_replace_advances_revision_without_changing_public_shape() {
        let mut fixture = HistoryAuthorityFixture::new();
        let mut history = CanonicalHistory::new(fixture.router_identity());
        let target = RouteLocation::new("/items/1");
        publish(
            &mut history,
            &mut fixture,
            HistoryEffect::Push(target.clone()),
        );
        let shape_before = history.snapshot();
        let revision_before = history.revision;
        let entry_before = history.entry_ids[0];

        let (execution, presentation) = fixture.next_history_binding();
        let replacement_request = execution.request;
        let capability = history.freeze(execution, HistoryEffect::Replace(target));
        let prepared = match history.prepare(capability, presentation) {
            Ok(prepared) => prepared,
            Err(_) => panic!("the exact equivalent replace must prepare"),
        };
        history.publish(prepared);

        assert_eq!(history.snapshot(), shape_before);
        assert_ne!(history.revision, revision_before);
        assert_eq!(history.revision.committed_by, Some(replacement_request));
        assert_ne!(history.entry_ids[0], entry_before);
        assert_eq!(history.entry_ids[0].creator, replacement_request);
    }

    #[test]
    fn replace_at_middle_preserves_forward_entries_and_replaces_entry_identity() {
        let mut fixture = HistoryAuthorityFixture::new();
        let mut history = CanonicalHistory::new(fixture.router_identity());
        for location in ["/a", "/b", "/c"] {
            publish(
                &mut history,
                &mut fixture,
                HistoryEffect::Push(RouteLocation::new(location)),
            );
        }
        fixture.move_canonical_cursor_back(&mut history);
        let identities_before = history.entry_ids.clone();

        publish(
            &mut history,
            &mut fixture,
            HistoryEffect::Replace(RouteLocation::new("/b2")),
        );

        assert_eq!(
            history.snapshot().entries(),
            [
                RouteLocation::new("/a"),
                RouteLocation::new("/b2"),
                RouteLocation::new("/c"),
            ]
        );
        assert_eq!(history.snapshot().cursor(), Some(1));
        assert_eq!(history.entry_ids[0], identities_before[0]);
        assert_ne!(history.entry_ids[1], identities_before[1]);
        assert_eq!(history.entry_ids[2], identities_before[2]);
    }

    #[test]
    fn history_source_aba_is_rejected() {
        let mut fixture = HistoryAuthorityFixture::new();
        let mut history = CanonicalHistory::new(fixture.router_identity());
        for location in ["/items/1", "/items/2"] {
            publish(
                &mut history,
                &mut fixture,
                HistoryEffect::Push(RouteLocation::new(location)),
            );
        }
        let frozen_public_shape = history.snapshot();
        let frozen_revision = history.revision;
        let frozen_entry_ids = history.entry_ids.clone();
        let BackPreview::Target(destination) = history.preview_back() else {
            panic!("the two-entry fixture must freeze a real Back destination")
        };
        let (stale_execution, stale_presentation) = fixture.next_history_binding();
        let stale = match history.freeze_back(stale_execution, destination) {
            Ok(stale) => stale,
            Err(_) => panic!("the Back capability must freeze against the exact live source"),
        };

        publish(
            &mut history,
            &mut fixture,
            HistoryEffect::Replace(RouteLocation::new("/temporary")),
        );
        publish(
            &mut history,
            &mut fixture,
            HistoryEffect::Replace(RouteLocation::new("/items/2")),
        );
        assert_eq!(history.snapshot(), frozen_public_shape);
        assert_ne!(history.entry_ids, frozen_entry_ids);
        let shape_before_probe = history.snapshot();
        let revision_before_probe = history.revision;
        let identities_before_probe = history.entry_ids.clone();
        let authority_before_probe = history.authority_snapshot();

        let error = match history.prepare(stale, stale_presentation) {
            Err(error) => error,
            Ok(_) => panic!("equal locations and cursor do not authorize an ABA-stale Back"),
        };
        let capability = error.capability_snapshot();
        let HistoryPreparationError::Source(drift) = &error else {
            panic!("source revision drift must not be flattened into binding drift")
        };

        assert_eq!(drift.capability.source.revision, frozen_revision);
        assert_ne!(drift.observed.revision, frozen_revision);
        assert_eq!(drift.capability.source.locations, drift.observed.locations);
        assert_eq!(drift.capability.source.cursor, drift.observed.cursor);
        assert_ne!(drift.capability.source.entry_ids, drift.observed.entry_ids);
        assert!(capability.has_exactly_one_owner());
        assert_eq!(history.snapshot(), shape_before_probe);
        assert_eq!(history.revision, revision_before_probe);
        assert_eq!(history.entry_ids, identities_before_probe);
        assert_eq!(history.authority_snapshot(), authority_before_probe);
    }

    #[test]
    fn foreign_publication_evidence_probe_is_zero_write() {
        let mut receiver = HistoryAuthorityFixture::running_replace("/items/2");
        let foreign = HistoryAuthorityFixture::running_replace("/foreign");
        let sealed = receiver.seal_current_replace();
        let evidence = sealed.evidence().testing_with_observed_router_and_request(
            foreign.router_identity(),
            foreign.running_request(),
        );
        let before = receiver.engine_snapshot();
        let capability_before = receiver.history_capability_snapshot();

        assert_eq!(
            receiver.probe_sealed_commit_evidence(&evidence),
            Err(CommitError::Stale)
        );

        assert_eq!(receiver.engine_snapshot(), before);
        assert_eq!(receiver.history_capability_snapshot(), capability_before);

        let prepared = receiver
            .prepare_publication(sealed)
            .expect("the projection-only probe leaves the real sealed owner consumable");
        assert!(
            receiver
                .history_capability_snapshot()
                .has_exactly_one_owner()
        );
        let _consumed = receiver.publish_and_return_consumed_evidence(prepared);
        assert!(!receiver.history_capability_snapshot().has_owner());
    }

    #[test]
    fn current_work_pre_retirement_clone_does_not_clone_history_capability() {
        let fixture = HistoryAuthorityFixture::running_replace("/items/2");
        let owner_before = fixture.history_capability_snapshot();
        assert!(owner_before.has_exactly_one_owner());

        fixture.clone_current_work_pre_retirement();

        assert_eq!(fixture.history_capability_snapshot(), owner_before);
    }

    #[test]
    fn consumed_publication_replay_proof_is_zero_write() {
        let mut fixture = HistoryAuthorityFixture::running_replace("/items/2");
        let sealed = fixture.seal_current_replace();
        let prepared = fixture.prepare_publication(sealed).unwrap();
        let consumed = fixture.publish_and_return_consumed_evidence(prepared);
        let after_publish = fixture.engine_snapshot();

        assert_eq!(
            fixture.probe_consumed_publication_evidence(&consumed),
            Err(CommitError::Stale)
        );
        assert_eq!(fixture.engine_snapshot(), after_publish);
        assert!(!fixture.history_capability_snapshot().has_owner());
        assert_eq!(
            fixture
                .request_trace()
                .iter()
                .filter(|event| event.kind() == NavigationTraceEventKind::CommitPublished)
                .count(),
            1
        );
    }
}
