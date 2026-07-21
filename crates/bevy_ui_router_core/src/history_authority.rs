use crate::cleanup::CleanupBatchToken;
use crate::fingerprint::RouteBranchFingerprint;
use crate::frame::{FrameOrigin, RouteFrameVersion};
use crate::history::{HistorySnapshot, MemoryHistory};
use crate::ids::{
    CleanupBatchId, FrameTaskId, NavigationRequestId, RouteFrameId, RouteGeneration, RouteHostId,
    RouteSlotId, RouterIdentity, TransactionId,
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
    #[cfg(test)]
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
    use super::{CanonicalHistory, HistoryEffect, HistoryPreparationError};
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
    fn source_revision_aba_is_rejected_before_publication() {
        let mut fixture = HistoryAuthorityFixture::new();
        let mut history = CanonicalHistory::new(fixture.router_identity());
        publish(
            &mut history,
            &mut fixture,
            HistoryEffect::Push(RouteLocation::new("/items/1")),
        );
        let frozen_revision = history.revision;
        let (stale_execution, stale_presentation) = fixture.next_history_binding();
        let stale = history.freeze(
            stale_execution,
            HistoryEffect::Replace(RouteLocation::new("/items/2")),
        );

        publish(
            &mut history,
            &mut fixture,
            HistoryEffect::Replace(RouteLocation::new("/temporary")),
        );
        publish(
            &mut history,
            &mut fixture,
            HistoryEffect::Replace(RouteLocation::new("/items/1")),
        );
        let shape_before_probe = history.snapshot();
        let revision_before_probe = history.revision;
        let identities_before_probe = history.entry_ids.clone();

        let error = match history.prepare(stale, stale_presentation) {
            Err(error) => error,
            Ok(_) => panic!("equal locations do not authorize an ABA-stale capability"),
        };
        let HistoryPreparationError::Source(drift) = error else {
            panic!("source revision drift must not be flattened into binding drift")
        };

        assert_eq!(drift.capability.source.revision, frozen_revision);
        assert_ne!(drift.observed.revision, frozen_revision);
        assert_eq!(history.snapshot(), shape_before_probe);
        assert_eq!(history.revision, revision_before_probe);
        assert_eq!(history.entry_ids, identities_before_probe);
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
    fn current_work_clone_does_not_clone_history_capability() {
        let fixture = HistoryAuthorityFixture::running_replace("/items/2");
        let owner_before = fixture.history_capability_snapshot();
        assert!(owner_before.has_exactly_one_owner());

        fixture.clone_current_work();

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
