#![allow(dead_code)]

use std::collections::VecDeque;

use crate::admission::{
    AdmissionError, HostAdmissionConfirmation, HostAdmissionIssuer, HostAdmissionToken, HostGrant,
};
use crate::branch::{MatchedBranch, MatchedRoute};
use crate::cleanup::{CleanupBatchToken, CleanupCompletion, CleanupManifestSeal};
use crate::frame::{FrameOrigin, RouteFrameVersion};
use crate::history::{HistoryIntent, MemoryHistory};
use crate::ids::{
    CoreIdAllocator, FrameTaskId, NavigationRequestId, ReservedNavigationIds, RouteFrameId,
    RouteGeneration, RouteHostId, RouteSlotId, RouterIdentity, TransactionId,
};
use crate::location::RouteLocation;
use crate::matcher::RouteMatcher;
use crate::presentation::{ActivePresentation, RouterNavigationState};
use crate::request::{
    NavigationFailure, NavigationFailureKind, NavigationPhase, NavigationRequestRecord,
    RequestStatus,
};
use crate::route_tree::RouteTree;
use crate::trace::{NavigationTraceEvent, NavigationTraceEventKind};
use crate::view_input::RouteViewInput;
use crate::view_key::RouteViewKey;

const TERMINAL_RETENTION: usize = 64;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RootTaskIdentity {
    router: RouterIdentity,
    host: RouteHostId,
    host_epoch: u64,
    top_slot: RouteSlotId,
    request: NavigationRequestId,
    transaction: TransactionId,
    frame_task: FrameTaskId,
    frame: RouteFrameId,
    generation: RouteGeneration,
    origin: FrameOrigin,
}

impl RootTaskIdentity {
    pub(crate) const fn router_identity(&self) -> RouterIdentity {
        self.router
    }

    pub(crate) const fn host(&self) -> RouteHostId {
        self.host
    }

    pub(crate) const fn host_epoch(&self) -> u64 {
        self.host_epoch
    }

    pub(crate) const fn top_slot(&self) -> RouteSlotId {
        self.top_slot
    }

    pub(crate) const fn request(&self) -> NavigationRequestId {
        self.request
    }

    pub(crate) const fn transaction(&self) -> TransactionId {
        self.transaction
    }

    pub(crate) const fn frame_task(&self) -> FrameTaskId {
        self.frame_task
    }

    pub(crate) const fn frame(&self) -> RouteFrameId {
        self.frame
    }

    pub(crate) const fn generation(&self) -> RouteGeneration {
        self.generation
    }

    pub(crate) const fn origin(&self) -> &FrameOrigin {
        &self.origin
    }

    #[cfg(test)]
    fn testing_set_router(&mut self, router: RouterIdentity) {
        self.router = router;
    }

    #[cfg(test)]
    fn testing_set_host(&mut self, host: RouteHostId) {
        self.host = host;
    }

    #[cfg(test)]
    fn testing_set_host_epoch(&mut self, epoch: u64) {
        self.host_epoch = epoch;
    }

    #[cfg(test)]
    fn testing_set_top_slot(&mut self, slot: RouteSlotId) {
        self.top_slot = slot;
    }

    #[cfg(test)]
    fn testing_set_request(&mut self, request: NavigationRequestId) {
        self.request = request;
    }

    #[cfg(test)]
    fn testing_set_transaction(&mut self, transaction: TransactionId) {
        self.transaction = transaction;
    }

    #[cfg(test)]
    fn testing_set_frame_task(&mut self, task: FrameTaskId) {
        self.frame_task = task;
    }

    #[cfg(test)]
    fn testing_set_frame(&mut self, frame: RouteFrameId) {
        self.frame = frame;
    }

    #[cfg(test)]
    fn testing_set_generation(&mut self, generation: RouteGeneration) {
        self.generation = generation;
    }

    #[cfg(test)]
    fn testing_set_origin(&mut self, origin: FrameOrigin) {
        self.origin = origin;
    }
}

pub(crate) struct RootFrameTask {
    identity: RootTaskIdentity,
    frame: RouteFrameVersion,
    location: RouteLocation,
    matched: MatchedRoute,
    view: RouteViewKey,
    input: Option<RouteViewInput>,
}

impl RootFrameTask {
    pub(crate) const fn identity(&self) -> &RootTaskIdentity {
        &self.identity
    }

    pub(crate) const fn frame_version(&self) -> &RouteFrameVersion {
        &self.frame
    }

    pub(crate) const fn location(&self) -> &RouteLocation {
        &self.location
    }

    #[allow(dead_code)]
    pub(crate) const fn matched(&self) -> &MatchedRoute {
        &self.matched
    }

    pub(crate) const fn view(&self) -> &RouteViewKey {
        &self.view
    }

    pub(crate) const fn input(&self) -> &RouteViewInput {
        self.input
            .as_ref()
            .expect("frame task input is available before driver dispatch")
    }

    pub(crate) fn take_input(&mut self) -> RouteViewInput {
        self.input
            .take()
            .expect("frame task input is consumed exactly once")
    }
}

pub(crate) enum EngineAction {
    FrameTask(RootFrameTask),
    Commit(CommitTicket),
}

#[derive(Debug)]
pub(crate) struct CommitTicket {
    identity: RootTaskIdentity,
}

impl CommitTicket {
    #[cfg(test)]
    fn testing_duplicate(&self) -> Self {
        Self {
            identity: self.identity.clone(),
        }
    }

    #[cfg(test)]
    fn testing_set_host_epoch(&mut self, epoch: u64) {
        self.identity.host_epoch = epoch;
    }

    #[cfg(test)]
    const fn identity(&self) -> &RootTaskIdentity {
        &self.identity
    }

    #[cfg(test)]
    fn testing_with_identity(&self, identity: RootTaskIdentity) -> Self {
        Self { identity }
    }
}

#[derive(Debug)]
pub(crate) struct SealedCommit {
    identity: RootTaskIdentity,
    cleanup: CleanupBatchToken,
    publication: PublicationPlan,
}

impl SealedCommit {
    pub(crate) const fn cleanup(&self) -> &CleanupBatchToken {
        &self.cleanup
    }

    #[cfg(test)]
    fn testing_duplicate(&self) -> Self {
        Self {
            identity: self.identity.clone(),
            cleanup: self.cleanup.clone(),
            publication: self.publication.clone(),
        }
    }

    #[cfg(test)]
    fn testing_with_identity(&self, identity: RootTaskIdentity) -> Self {
        Self {
            identity,
            cleanup: self.cleanup.clone(),
            publication: self.publication.clone(),
        }
    }
}

#[derive(Clone, Debug)]
struct PublicationPlan {
    state: RouterNavigationState,
    history: MemoryHistory,
    active_branch: MatchedBranch,
    record_index: usize,
    published_record: NavigationRequestRecord,
    current: CurrentWork,
}

pub(crate) struct PreparedPublication {
    sealed: SealedCommit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EngineInstallError {
    IdExhausted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PushError {
    AwaitingHost,
    IdExhausted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReportError {
    Stale,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CommitError {
    Stale,
    InvalidManifest,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CleanupError {
    Stale,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PrivateRequestLookup<'a> {
    Retained(&'a NavigationRequestRecord),
    Evicted,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum WorkStage {
    Applying,
    ReadyToCommit,
    CommitSealed { token: CleanupBatchToken },
    PostCommitCleanup { outstanding: CleanupBatchToken },
}

#[derive(Clone, Debug)]
struct CurrentWork {
    identity: RootTaskIdentity,
    branch: MatchedBranch,
    target_frame: RouteFrameVersion,
    history_intent: Option<HistoryIntent>,
    stage: WorkStage,
    cleanup_batch: crate::ids::CleanupBatchId,
    cleanup_roots: Vec<RouteFrameVersion>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct QueuedWork {
    request: NavigationRequestId,
    reserved: ReservedNavigationIds,
}

pub(crate) struct RouterEngine {
    matcher: RouteMatcher,
    ids: CoreIdAllocator,
    router: RouterIdentity,
    state: RouterNavigationState,
    history: MemoryHistory,
    active_branch: Option<MatchedBranch>,
    records: VecDeque<NavigationRequestRecord>,
    evicted_through: Option<u64>,
    highest_issued: Option<u64>,
    queue: VecDeque<QueuedWork>,
    current: Option<CurrentWork>,
    action: Option<EngineAction>,
}

impl RouterEngine {
    pub(crate) fn install(tree: RouteTree) -> Result<Self, EngineInstallError> {
        let mut ids = CoreIdAllocator::new().map_err(|_| EngineInstallError::IdExhausted)?;
        let router = ids
            .allocate_router_identity()
            .map_err(|_| EngineInstallError::IdExhausted)?;
        Ok(Self {
            matcher: RouteMatcher::new(tree),
            ids,
            router,
            state: RouterNavigationState::new(router),
            history: MemoryHistory::new(),
            active_branch: None,
            records: VecDeque::new(),
            evicted_through: None,
            highest_issued: None,
            queue: VecDeque::new(),
            current: None,
            action: None,
        })
    }

    pub(crate) fn request_host_admission(
        &mut self,
        issuer: &mut HostAdmissionIssuer,
    ) -> Result<HostAdmissionToken, AdmissionError> {
        if self.state.host().is_some() {
            return Err(AdmissionError::AuthorityExists);
        }
        issuer.issue(self.router)
    }

    pub(crate) fn grant_host_admission(
        &mut self,
        issuer: &mut HostAdmissionIssuer,
        token: &HostAdmissionToken,
    ) -> Result<HostGrant, AdmissionError> {
        issuer.grant(token, &mut self.ids)
    }

    pub(crate) fn confirm_host(
        &mut self,
        issuer: &HostAdmissionIssuer,
        confirmation: &HostAdmissionConfirmation,
    ) -> Result<(), crate::presentation::AuthorityConfirmationError> {
        self.state.confirm_host(issuer, confirmation)
    }

    pub(crate) fn push(
        &mut self,
        location: RouteLocation,
    ) -> Result<NavigationRequestId, PushError> {
        let request = self.enqueue_push(location)?;
        self.collect();
        Ok(request)
    }

    pub(crate) fn enqueue_push(
        &mut self,
        location: RouteLocation,
    ) -> Result<NavigationRequestId, PushError> {
        if self.state.host().is_none() {
            return Err(PushError::AwaitingHost);
        }
        let reserved = self
            .ids
            .reserve_navigation(self.router)
            .map_err(|_| PushError::IdExhausted)?;
        let request = reserved.request();
        self.records
            .push_back(NavigationRequestRecord::new(request, location));
        self.highest_issued = Some(request.sequence());
        self.queue.push_back(QueuedWork { request, reserved });
        Ok(request)
    }

    pub(crate) fn collect(&mut self) {
        if self.current.is_none() {
            self.start_next();
        }
    }

    pub(crate) fn action(&mut self) -> Option<EngineAction> {
        self.action.take()
    }

    pub(crate) fn begin_view_report<'a>(
        &'a mut self,
        identity: &RootTaskIdentity,
    ) -> Result<ViewReportSession<'a>, ReportError> {
        let valid = self.current.as_ref().is_some_and(|current| {
            current.stage == WorkStage::Applying && current.identity == *identity
        });
        if !valid {
            return Err(ReportError::Stale);
        }
        Ok(ViewReportSession {
            engine: self,
            identity: identity.clone(),
        })
    }

    pub(crate) fn seal_commit(
        &mut self,
        ticket: &CommitTicket,
        seal: CleanupManifestSeal,
    ) -> Result<SealedCommit, CommitError> {
        let Some(current) = self.current.as_ref() else {
            return Err(CommitError::Stale);
        };
        if current.stage != WorkStage::ReadyToCommit || current.identity != ticket.identity {
            return Err(CommitError::Stale);
        }

        if current.cleanup_roots != seal.logical_roots()
            || current.cleanup_roots.is_empty() != (seal.obligation_count() == 0)
        {
            return Err(CommitError::InvalidManifest);
        }

        let cleanup = CleanupBatchToken::sealed(
            current.identity.clone(),
            current.cleanup_batch,
            &current.cleanup_roots,
            seal.manifest_digest(),
            seal.obligation_count(),
        );
        let publication = self.precompute_publication(cleanup.clone());
        self.current
            .as_mut()
            .expect("current work was validated")
            .stage = WorkStage::CommitSealed {
            token: cleanup.clone(),
        };
        Ok(SealedCommit {
            identity: ticket.identity.clone(),
            cleanup,
            publication,
        })
    }

    pub(crate) fn prepare_publication(
        &mut self,
        sealed: SealedCommit,
    ) -> Result<PreparedPublication, CommitError> {
        let valid = self.current.as_ref().is_some_and(|current| {
            current.identity == sealed.identity
                && matches!(
                    &current.stage,
                    WorkStage::CommitSealed { token } if token == &sealed.cleanup
                )
        });
        if !valid {
            return Err(CommitError::Stale);
        }
        let Some(record_slot) = self.records.get(sealed.publication.record_index) else {
            return Err(CommitError::Stale);
        };
        if record_slot.id() != sealed.identity.request {
            return Err(CommitError::Stale);
        }
        Ok(PreparedPublication { sealed })
    }

    pub(crate) fn publish_prepared<F>(&mut self, prepared: PreparedPublication, mutation: F)
    where
        F: FnOnce(),
    {
        let SealedCommit {
            identity: _,
            cleanup: _,
            publication,
        } = prepared.sealed;
        let PublicationPlan {
            state,
            history,
            active_branch,
            record_index,
            published_record,
            current,
        } = publication;
        mutation();
        self.records[record_index] = published_record;
        self.state = state;
        self.history = history;
        self.active_branch = Some(active_branch);
        self.current = Some(current);
    }

    pub(crate) fn complete_cleanup(
        &mut self,
        completion: CleanupCompletion,
    ) -> Result<(), CleanupError> {
        let valid = self.current.as_ref().is_some_and(|current| {
            matches!(
                &current.stage,
                WorkStage::PostCommitCleanup { outstanding }
                    if completion.matches(outstanding)
            )
        });
        if !valid {
            return Err(CleanupError::Stale);
        }

        let request = self
            .current
            .as_ref()
            .expect("validated current work")
            .identity
            .request;
        let record = self
            .record_mut(request)
            .expect("running record is retained");
        record
            .append_trace(NavigationTraceEvent::cleanup_settled(None))
            .expect("terminal trace is open");
        record
            .transition(RequestStatus::Succeeded)
            .expect("request is running");
        record
            .append_trace(NavigationTraceEvent::request_terminal())
            .expect("terminal status precedes terminal event");
        self.current = None;
        self.state.set_request(None);
        self.retain_terminal_records();
        self.start_next();
        Ok(())
    }

    pub(crate) const fn active(&self) -> &ActivePresentation {
        self.state.active()
    }

    pub(crate) const fn history(&self) -> &MemoryHistory {
        &self.history
    }

    pub(crate) fn request(&self, id: NavigationRequestId) -> Option<&NavigationRequestRecord> {
        self.records.iter().find(|record| record.id() == id)
    }

    pub(crate) fn private_lookup(&self, id: NavigationRequestId) -> PrivateRequestLookup<'_> {
        if let Some(record) = self.request(id) {
            PrivateRequestLookup::Retained(record)
        } else if id.router_identity() == self.router
            && self
                .evicted_through
                .is_some_and(|sequence| id.sequence() <= sequence)
        {
            PrivateRequestLookup::Evicted
        } else {
            PrivateRequestLookup::Unknown
        }
    }

    #[cfg(test)]
    fn current_status(&self) -> Option<&RequestStatus> {
        self.state.current_request()
    }

    fn start_next(&mut self) {
        let Some(queued) = self.queue.pop_front() else {
            self.state.set_request(None);
            return;
        };
        let request = queued.request;
        let location = self
            .request(request)
            .expect("queued record is retained")
            .requested()
            .clone();
        let branch = match self.matcher.match_location(&location) {
            Ok(branch) => branch,
            Err(error) => {
                let failure =
                    NavigationFailure::new(NavigationFailureKind::Match, error.redacted_message());
                let record = self
                    .record_mut(request)
                    .expect("running record is retained");
                record
                    .transition(RequestStatus::Failed(failure))
                    .expect("request is running");
                record
                    .append_trace(NavigationTraceEvent::request_terminal())
                    .expect("terminal status precedes terminal event");
                self.state.set_request(None);
                self.retain_terminal_records();
                self.start_next();
                return;
            }
        };
        self.record_mut(request)
            .expect("running record is retained")
            .set_matched(branch.branch_fingerprint().clone())
            .expect("match is written once");
        let active_location = self.state.active().location();
        let active_frame = self.state.active().frames().first();
        let equivalent = active_location == Some(&location);
        let fragment_only = !equivalent
            && active_location.is_some_and(|active| {
                active.path() == location.path()
                    && active.search() == location.search()
                    && active.fragment() != location.fragment()
            })
            && self
                .current_active_branch()
                .is_some_and(|active| active.branch_fingerprint() == branch.branch_fingerprint());

        let (frame, generation) = if equivalent {
            let active = active_frame.expect("route presentation has one active root");
            (active.frame(), active.generation())
        } else if fragment_only {
            let active = active_frame.expect("fragment remount has an active root");
            (
                active.frame(),
                match active.generation().checked_next() {
                    Ok(generation) => generation,
                    Err(_) => {
                        self.fail_reserved_request(
                            request,
                            NavigationFailure::new(
                                NavigationFailureKind::AuthorityLost,
                                "route generation exhausted",
                            ),
                        );
                        self.start_next();
                        return;
                    }
                },
            )
        } else {
            (
                queued.reserved.candidate_frame(),
                RouteGeneration::initial(),
            )
        };

        let leaf = branch
            .leaf()
            .expect("route tree matches a nonempty branch")
            .clone();
        let origin = FrameOrigin::MatchedRoute(leaf.route());
        let view = self
            .matcher
            .view_binding(leaf.route())
            .expect("installed route tree has a root view")
            .clone();
        let input = RouteViewInput::new(
            location.clone(),
            Some(leaf.route()),
            leaf.params().clone(),
            origin.clone(),
            None,
        );
        let host = self.state.host().expect("host was checked before push");
        let top_slot = self.state.top_slot().expect("confirmed host has top slot");
        let identity = RootTaskIdentity {
            router: self.router,
            host: host.host(),
            host_epoch: host.epoch(),
            top_slot,
            request,
            transaction: queued.reserved.transaction(),
            frame_task: queued.reserved.frame_task(),
            frame,
            generation,
            origin: origin.clone(),
        };
        let target_frame = RouteFrameVersion::new(
            self.router,
            host.host(),
            top_slot,
            origin,
            frame,
            generation,
        );
        let cleanup_roots = if active_frame.is_some() && !equivalent {
            vec![active_frame.expect("checked active frame").clone()]
        } else {
            Vec::new()
        };
        let stage = if equivalent {
            WorkStage::ReadyToCommit
        } else {
            WorkStage::Applying
        };
        self.current = Some(CurrentWork {
            identity: identity.clone(),
            branch,
            target_frame: target_frame.clone(),
            history_intent: Some(HistoryIntent::Push(location.clone())),
            stage,
            cleanup_batch: queued.reserved.cleanup_batch(),
            cleanup_roots,
        });

        if equivalent {
            self.transition_with_trace(request, NavigationPhase::ReadyToSwitch);
            self.action = Some(EngineAction::Commit(CommitTicket { identity }));
        } else {
            let record = self
                .record_mut(request)
                .expect("running record is retained");
            record
                .append_trace(NavigationTraceEvent::new(
                    NavigationTraceEventKind::FrameTaskIssued,
                ))
                .expect("running trace is open");
            record
                .transition(RequestStatus::Running(NavigationPhase::ApplyingFrame(0)))
                .expect("request is running");
            record
                .append_trace(NavigationTraceEvent::request_phase_changed(
                    NavigationPhase::ApplyingFrame(0),
                ))
                .expect("running trace is open");
            self.state.set_request(Some(RequestStatus::Running(
                NavigationPhase::ApplyingFrame(0),
            )));
            self.action = Some(EngineAction::FrameTask(RootFrameTask {
                identity,
                frame: target_frame,
                location,
                matched: leaf,
                view,
                input: Some(input),
            }));
        }
    }

    fn current_active_branch(&self) -> Option<&MatchedBranch> {
        self.active_branch.as_ref()
    }

    fn transition_with_trace(&mut self, request: NavigationRequestId, phase: NavigationPhase) {
        let record = self.record_mut(request).expect("request is retained");
        record
            .transition(RequestStatus::Running(phase.clone()))
            .expect("request is not terminal");
        record
            .append_trace(NavigationTraceEvent::request_phase_changed(phase.clone()))
            .expect("running trace is open");
        self.state.set_request(Some(RequestStatus::Running(phase)));
    }

    fn record_mut(&mut self, id: NavigationRequestId) -> Option<&mut NavigationRequestRecord> {
        self.records.iter_mut().find(|record| record.id() == id)
    }

    fn fail_reserved_request(&mut self, request: NavigationRequestId, failure: NavigationFailure) {
        let record = self
            .record_mut(request)
            .expect("accepted request is retained");
        record
            .transition(RequestStatus::Failed(failure))
            .expect("request is not terminal");
        record
            .append_trace(NavigationTraceEvent::request_terminal())
            .expect("terminal status precedes terminal event");
        self.state.set_request(None);
        self.retain_terminal_records();
    }

    fn precompute_publication(&self, cleanup: CleanupBatchToken) -> PublicationPlan {
        let current = self
            .current
            .as_ref()
            .expect("sealing validated current work");
        let request = current.identity.request;
        let location = self
            .request(request)
            .expect("running record is retained")
            .requested()
            .clone();

        let mut history = self.history.clone();
        current
            .history_intent
            .clone()
            .expect("commit has one private history intent")
            .publish(&mut history);

        let mut state = self.state.clone();
        state.set_active(ActivePresentation::route(
            location,
            vec![current.target_frame.clone()],
        ));
        state.set_request(Some(RequestStatus::Running(
            NavigationPhase::PostCommitCleanup,
        )));

        let record_index = self
            .records
            .iter()
            .position(|record| record.id() == request)
            .expect("running record is retained");
        let mut published_record = self.records[record_index].clone();
        published_record
            .append_trace(NavigationTraceEvent::commit_published())
            .expect("running trace is open");
        published_record
            .transition(RequestStatus::Running(NavigationPhase::PostCommitCleanup))
            .expect("request is running");
        published_record
            .append_trace(NavigationTraceEvent::request_phase_changed(
                NavigationPhase::PostCommitCleanup,
            ))
            .expect("running trace is open");

        let mut current = current.clone();
        current.history_intent = None;
        current.stage = WorkStage::PostCommitCleanup {
            outstanding: cleanup,
        };

        PublicationPlan {
            state,
            history,
            active_branch: current.branch.clone(),
            record_index,
            published_record,
            current,
        }
    }

    fn retain_terminal_records(&mut self) {
        while self
            .records
            .iter()
            .filter(|record| is_terminal(record.status()))
            .count()
            > TERMINAL_RETENTION
        {
            let index = self
                .records
                .iter()
                .position(|record| is_terminal(record.status()))
                .expect("terminal count was nonzero");
            let evicted = self.records.remove(index).expect("index came from records");
            let sequence = evicted.id().sequence();
            self.evicted_through = Some(
                self.evicted_through
                    .map_or(sequence, |current| current.max(sequence)),
            );
        }
    }

    #[cfg(test)]
    fn snapshot(&self) -> EngineSnapshot {
        EngineSnapshot {
            state: self.state.snapshot(),
            ids: self.ids.snapshot(),
            history: self.history.clone(),
            active_branch: self
                .active_branch
                .as_ref()
                .map(|branch| format!("{branch:?}")),
            records: self.records.clone(),
            evicted_through: self.evicted_through,
            highest_issued: self.highest_issued,
            queue: self.queue.clone(),
            current: self.current.as_ref().map(|current| format!("{current:?}")),
            action: self.action.as_ref().map(ActionSnapshot::from),
        }
    }

    #[cfg(test)]
    fn testing_exhaust_reservation_counter(&mut self, counter: crate::ids::ReservationCounter) {
        self.ids
            .testing_exhaust_reservation_counter(self.router, counter);
    }

    #[cfg(test)]
    const fn testing_evicted_watermark(&self) -> Option<u64> {
        self.evicted_through
    }

    #[cfg(test)]
    fn testing_set_active_generation(&mut self, generation: RouteGeneration) {
        let active = self
            .state
            .active()
            .frames()
            .first()
            .expect("test requires active route")
            .clone();
        self.state.set_active(ActivePresentation::route(
            self.state
                .active()
                .location()
                .expect("test requires active route")
                .clone(),
            vec![RouteFrameVersion::new(
                active.router_identity(),
                active.host(),
                active.slot(),
                active.origin().clone(),
                active.frame(),
                generation,
            )],
        ));
    }
}

fn is_terminal(status: &RequestStatus) -> bool {
    matches!(status, RequestStatus::Succeeded | RequestStatus::Failed(_))
}

pub(crate) struct ViewReportSession<'a> {
    engine: &'a mut RouterEngine,
    identity: RootTaskIdentity,
}

impl std::fmt::Debug for ViewReportSession<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ViewReportSession(..)")
    }
}

impl ViewReportSession<'_> {
    pub(crate) fn applied(self) -> CommitTicket {
        let request = self.identity.request;
        let current = self
            .engine
            .current
            .as_mut()
            .expect("report was preflighted");
        current.stage = WorkStage::ReadyToCommit;
        let record = self
            .engine
            .record_mut(request)
            .expect("running record is retained");
        record
            .append_trace(NavigationTraceEvent::new(
                NavigationTraceEventKind::ViewApplied,
            ))
            .expect("running trace is open");
        self.engine
            .transition_with_trace(request, NavigationPhase::ReadyToSwitch);
        CommitTicket {
            identity: self.identity,
        }
    }

    pub(crate) fn failed(self, failure: NavigationFailure) {
        let request = self.identity.request;
        self.engine.current = None;
        self.engine.action = None;
        self.engine.fail_reserved_request(request, failure);
        self.engine.start_next();
    }
}

#[cfg(test)]
#[derive(Clone, Debug, Eq, PartialEq)]
enum ActionSnapshot {
    FrameTask(RootTaskIdentity),
    Commit(RootTaskIdentity),
}

#[cfg(test)]
impl From<&EngineAction> for ActionSnapshot {
    fn from(action: &EngineAction) -> Self {
        match action {
            EngineAction::FrameTask(task) => Self::FrameTask(task.identity.clone()),
            EngineAction::Commit(ticket) => Self::Commit(ticket.identity.clone()),
        }
    }
}

#[cfg(test)]
#[derive(Clone, Debug, PartialEq)]
struct EngineSnapshot {
    state: RouterNavigationState,
    ids: crate::ids::CoreIdAllocatorSnapshot,
    history: MemoryHistory,
    active_branch: Option<String>,
    records: VecDeque<NavigationRequestRecord>,
    evicted_through: Option<u64>,
    highest_issued: Option<u64>,
    queue: VecDeque<QueuedWork>,
    current: Option<String>,
    action: Option<ActionSnapshot>,
}

#[cfg(test)]
mod tests {
    use super::{
        CommitError, EngineAction, PrivateRequestLookup, ReportError, RootTaskIdentity,
        RouterEngine, WorkStage,
    };
    use crate::admission::HostAdmissionIssuer;
    use crate::cleanup::{
        CleanupBatchToken, CleanupCompletion, CleanupCompletionDigest, CleanupManifestDigest,
        CleanupManifestSeal,
    };
    use crate::frame::{FrameOrigin, RouteFrameVersion};
    use crate::ids::{
        CleanupBatchId, ReservationCounter, RouteFrameId, RouteGeneration, RouteHostId, RouteSlotId,
    };
    use crate::location::RouteLocation;
    use crate::pattern::RoutePattern;
    use crate::presentation::ActivePresentationKind;
    use crate::request::{
        NavigationFailure, NavigationFailureKind, NavigationPhase, RequestStatus,
    };
    use crate::route_tree::{RouteNodeOptions, RouteTree};
    use crate::trace::NavigationTraceEventKind;
    use crate::view_key::RouteViewKey;

    const NONZERO_OBLIGATION_COUNT: usize = 3;

    fn tree() -> RouteTree {
        let mut builder = RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/").unwrap(),
                RouteNodeOptions::new().with_view(RouteViewKey::new("root").unwrap()),
            )
            .unwrap();
        let root = builder.root_id().unwrap();
        builder
            .route_with_options(
                root,
                RoutePattern::path("items/:id").unwrap(),
                RouteNodeOptions::new().with_view(RouteViewKey::new("items").unwrap()),
            )
            .unwrap();
        builder
            .route_with_options(
                root,
                RoutePattern::path("about").unwrap(),
                RouteNodeOptions::new().with_view(RouteViewKey::new("about").unwrap()),
            )
            .unwrap();
        builder.build().unwrap()
    }

    fn ready_engine() -> RouterEngine {
        let mut engine = RouterEngine::install(tree()).unwrap();
        let mut issuer = HostAdmissionIssuer::new().unwrap();
        let token = engine.request_host_admission(&mut issuer).unwrap();
        let grant = engine.grant_host_admission(&mut issuer, &token).unwrap();
        let confirmation = issuer.confirm(&grant).unwrap();
        engine.confirm_host(&issuer, &confirmation).unwrap();
        engine
    }

    fn publish_current(engine: &mut RouterEngine) -> crate::cleanup::CleanupBatchToken {
        let action = engine.action().unwrap();
        publish_action(engine, action)
    }

    fn publish_action(
        engine: &mut RouterEngine,
        action: EngineAction,
    ) -> crate::cleanup::CleanupBatchToken {
        let ticket = match action {
            EngineAction::FrameTask(task) => {
                engine.begin_view_report(task.identity()).unwrap().applied()
            }
            EngineAction::Commit(ticket) => ticket,
        };
        let sealed = seal_current(engine, ticket);
        let cleanup = sealed.cleanup().clone();
        publish_sealed(engine, sealed, || {}).unwrap();
        cleanup
    }

    fn publish_sealed<F>(
        engine: &mut RouterEngine,
        sealed: super::SealedCommit,
        mutation: F,
    ) -> Result<(), CommitError>
    where
        F: FnOnce(),
    {
        let prepared = engine.prepare_publication(sealed)?;
        engine.publish_prepared(prepared, mutation);
        Ok(())
    }

    #[test]
    fn failed_view_report_terminates_without_publishing_history_or_presentation() {
        let mut engine = ready_engine();
        let request = engine.enqueue_push(RouteLocation::new("/")).unwrap();
        engine.collect();
        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("root frame task expected")
        };

        engine
            .begin_view_report(task.identity())
            .unwrap()
            .failed(NavigationFailure::new(
                NavigationFailureKind::Build,
                "view build rejected",
            ));

        assert!(matches!(
            engine.request(request).unwrap().status(),
            RequestStatus::Failed(failure) if failure.kind() == NavigationFailureKind::Build
        ));
        assert_eq!(engine.history().len(), 0);
        assert!(engine.state.active().location().is_none());
        assert!(engine.current.is_none());
        assert!(engine.action.is_none());
    }

    fn seal_current(engine: &mut RouterEngine, ticket: super::CommitTicket) -> super::SealedCommit {
        let roots = engine
            .current
            .as_ref()
            .expect("test requires current work")
            .cleanup_roots
            .clone();
        let count = if roots.is_empty() {
            0
        } else {
            NONZERO_OBLIGATION_COUNT
        };
        engine
            .seal_commit(
                &ticket,
                CleanupManifestSeal::new(roots, manifest_digest(41), count),
            )
            .unwrap()
    }

    fn manifest_digest(tag: u64) -> CleanupManifestDigest {
        CleanupManifestDigest::from_opaque(0x4d41_4e49_4645_5354 ^ tag)
    }

    fn settle_current(engine: &mut RouterEngine) {
        let cleanup = publish_current(engine);
        engine.complete_cleanup(valid_completion(&cleanup)).unwrap();
    }

    fn valid_completion(cleanup: &CleanupBatchToken) -> CleanupCompletion {
        let digest = if cleanup.obligation_count() == 0 {
            CleanupCompletionDigest::canonical_empty()
        } else {
            CleanupCompletionDigest::from_opaque(0x5445_524d_494e_414c)
        };
        CleanupCompletion::attest(cleanup.clone(), digest, cleanup.obligation_count())
    }

    fn distinct_identity() -> RootTaskIdentity {
        let mut engine = ready_engine();
        engine.push(RouteLocation::new("/items/1")).unwrap();
        settle_current(&mut engine);
        engine.push(RouteLocation::new("/about")).unwrap();
        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("expected second frame task");
        };
        task.identity().clone()
    }

    fn identity_mismatches(
        exact: &RootTaskIdentity,
        distinct: &RootTaskIdentity,
    ) -> Vec<RootTaskIdentity> {
        let mut mismatches = Vec::new();
        let mut value = exact.clone();
        value.testing_set_router(distinct.router_identity());
        mismatches.push(value);
        let mut value = exact.clone();
        value.testing_set_host(RouteHostId::new(exact.host().raw() + 1));
        mismatches.push(value);
        let mut value = exact.clone();
        value.testing_set_host_epoch(exact.host_epoch() + 1);
        mismatches.push(value);
        let mut value = exact.clone();
        value.testing_set_top_slot(RouteSlotId::new(exact.top_slot().raw() + 1));
        mismatches.push(value);
        let mut value = exact.clone();
        value.testing_set_request(distinct.request());
        mismatches.push(value);
        let mut value = exact.clone();
        value.testing_set_transaction(distinct.transaction());
        mismatches.push(value);
        let mut value = exact.clone();
        value.testing_set_frame_task(distinct.frame_task());
        mismatches.push(value);
        let mut value = exact.clone();
        value.testing_set_frame(distinct.frame());
        mismatches.push(value);
        let mut value = exact.clone();
        value.testing_set_generation(RouteGeneration::initial().next());
        mismatches.push(value);
        let mut value = exact.clone();
        value.testing_set_origin(FrameOrigin::RouterFallback);
        mismatches.push(value);
        mismatches
    }

    #[test]
    fn no_host_push_rejects_without_allocating_or_changing_state() {
        let mut engine = RouterEngine::install(tree()).unwrap();
        let before = engine.snapshot();

        assert!(engine.push(RouteLocation::new("/items/1")).is_err());
        assert_eq!(engine.snapshot(), before);
        assert_eq!(engine.active().kind(), ActivePresentationKind::None);
    }

    #[test]
    fn confirmed_host_push_matches_internally_and_queues_one_running_request() {
        let mut engine = ready_engine();
        let request = engine.push(RouteLocation::new("/items//1/")).unwrap();
        let queued = engine.push(RouteLocation::new("/about")).unwrap();

        assert_ne!(request, queued);
        assert_eq!(
            engine.request(request).unwrap().requested().path(),
            "/items/1"
        );
        assert_eq!(
            engine.request(request).unwrap().status(),
            &RequestStatus::Running(NavigationPhase::ApplyingFrame(0))
        );
        assert_eq!(
            engine.request(queued).unwrap().status(),
            &RequestStatus::Queued
        );
        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("first root push must issue a frame task");
        };
        assert_eq!(task.location().path(), "/items/1");
        assert!(matches!(
            task.identity().origin(),
            FrameOrigin::MatchedRoute(_)
        ));
    }

    #[test]
    fn every_root_task_identity_field_is_checked_before_report_mutation() {
        let mut engine = ready_engine();
        engine.push(RouteLocation::new("/items/1")).unwrap();
        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("expected frame task");
        };
        let exact = task.identity().clone();
        let foreign = distinct_identity();

        for mismatch in identity_mismatches(&exact, &foreign) {
            let before = engine.snapshot();
            assert_eq!(
                engine.begin_view_report(&mismatch).unwrap_err(),
                ReportError::Stale
            );
            assert_eq!(engine.snapshot(), before);
        }
    }

    #[test]
    fn successful_report_does_not_publish_until_preflighted_commit_returns_normally() {
        let mut engine = ready_engine();
        engine.push(RouteLocation::new("/items/1")).unwrap();
        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("expected frame task");
        };
        let ticket = engine.begin_view_report(task.identity()).unwrap().applied();

        assert_eq!(engine.active().kind(), ActivePresentationKind::None);
        assert!(engine.history().is_empty());

        let mut stale = ticket.testing_duplicate();
        stale.testing_set_host_epoch(stale.identity().host_epoch() + 1);
        let before = engine.snapshot();
        assert_eq!(
            engine
                .seal_commit(
                    &stale,
                    CleanupManifestSeal::new(vec![], manifest_digest(1), 0),
                )
                .unwrap_err(),
            CommitError::Stale
        );
        assert_eq!(engine.snapshot(), before);

        let sealed = seal_current(&mut engine, ticket);
        assert_eq!(engine.active().kind(), ActivePresentationKind::None);
        assert!(engine.history().is_empty());
        let cleanup = sealed.cleanup().clone();
        publish_sealed(&mut engine, sealed, || {}).unwrap();
        assert_eq!(engine.active().kind(), ActivePresentationKind::Route);
        assert_eq!(engine.history().len(), 1);
        assert_eq!(
            engine.current_status(),
            Some(&RequestStatus::Running(NavigationPhase::PostCommitCleanup))
        );
        assert_eq!(cleanup.obligation_count(), 0);
    }

    #[test]
    fn every_root_task_identity_field_is_checked_before_commit_mutation() {
        let mut engine = ready_engine();
        engine.push(RouteLocation::new("/items/1")).unwrap();
        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("expected frame task");
        };
        let exact = engine.begin_view_report(task.identity()).unwrap().applied();
        let foreign = distinct_identity();

        for mismatch in identity_mismatches(exact.identity(), &foreign) {
            let before = engine.snapshot();
            let stale = exact.testing_with_identity(mismatch);
            assert_eq!(
                engine
                    .seal_commit(
                        &stale,
                        CleanupManifestSeal::new(vec![], manifest_digest(2), 0),
                    )
                    .unwrap_err(),
                CommitError::Stale
            );
            assert_eq!(engine.snapshot(), before);
        }

        let sealed = seal_current(&mut engine, exact);
        let cleanup = sealed.cleanup().clone();
        publish_sealed(&mut engine, sealed, || {}).unwrap();
        engine.complete_cleanup(valid_completion(&cleanup)).unwrap();
    }

    #[test]
    fn equivalent_fragment_and_changed_root_pushes_have_exact_final_semantics() {
        let mut engine = ready_engine();
        engine.push(RouteLocation::new("/items/1")).unwrap();
        settle_current(&mut engine);
        let first = engine.active().frames()[0].clone();

        engine.push(RouteLocation::new("items//1/")).unwrap();
        let equivalent = engine.action().unwrap();
        assert!(matches!(equivalent, EngineAction::Commit(_)));
        let equivalent_cleanup = publish_action(&mut engine, equivalent);
        assert_eq!(equivalent_cleanup.obligation_count(), 0);
        engine
            .complete_cleanup(valid_completion(&equivalent_cleanup))
            .unwrap();
        assert_eq!(engine.active().frames()[0], first);

        engine
            .push(RouteLocation::new("/items/1").with_fragment("details"))
            .unwrap();
        let EngineAction::FrameTask(fragment) = engine.action().unwrap() else {
            panic!("fragment change must issue one task");
        };
        assert_eq!(fragment.identity().frame(), first.frame());
        assert_eq!(fragment.identity().generation(), first.generation().next());
        let fragment_cleanup = publish_action(&mut engine, EngineAction::FrameTask(fragment));
        assert_eq!(
            fragment_cleanup.obligation_count(),
            NONZERO_OBLIGATION_COUNT
        );
        engine
            .complete_cleanup(valid_completion(&fragment_cleanup))
            .unwrap();

        engine.push(RouteLocation::new("/about")).unwrap();
        let EngineAction::FrameTask(changed) = engine.action().unwrap() else {
            panic!("changed root must issue one entering task");
        };
        assert_ne!(changed.identity().frame(), first.frame());
        assert_eq!(changed.identity().generation(), RouteGeneration::initial());
        let changed_cleanup = publish_action(&mut engine, EngineAction::FrameTask(changed));
        assert_eq!(changed_cleanup.obligation_count(), NONZERO_OBLIGATION_COUNT);
        engine
            .complete_cleanup(valid_completion(&changed_cleanup))
            .unwrap();
        assert_eq!(engine.history().len(), 4);
    }

    #[test]
    fn cleanup_completion_is_exact_single_use_and_terminal_event_is_last() {
        let mut engine = ready_engine();
        let request = engine.push(RouteLocation::new("/items/1")).unwrap();
        let cleanup = publish_current(&mut engine);
        let replay = valid_completion(&cleanup);

        assert_eq!(
            engine.request(request).unwrap().status(),
            &RequestStatus::Running(NavigationPhase::PostCommitCleanup)
        );
        assert_eq!(
            engine
                .request(request)
                .unwrap()
                .trace()
                .iter()
                .filter(|event| event.kind() == NavigationTraceEventKind::CleanupSettled)
                .count(),
            0
        );
        engine.complete_cleanup(valid_completion(&cleanup)).unwrap();
        let terminal = engine.snapshot();
        assert_eq!(
            engine.request(request).unwrap().status(),
            &RequestStatus::Succeeded
        );
        assert_eq!(
            engine
                .request(request)
                .unwrap()
                .trace()
                .iter()
                .map(|event| event.kind())
                .rev()
                .take(3)
                .collect::<Vec<_>>(),
            vec![
                NavigationTraceEventKind::RequestTerminal,
                NavigationTraceEventKind::CleanupSettled,
                NavigationTraceEventKind::RequestPhaseChanged,
            ]
        );
        let settled = engine
            .request(request)
            .unwrap()
            .trace()
            .iter()
            .filter(|event| event.kind() == NavigationTraceEventKind::CleanupSettled)
            .collect::<Vec<_>>();
        assert_eq!(settled.len(), 1);
        assert_eq!(settled[0].cleanup_disposition(), None);
        assert!(engine.complete_cleanup(replay).is_err());
        assert_eq!(engine.snapshot(), terminal);
    }

    #[test]
    fn cleanup_rejects_each_foreign_identity_field_and_digest_without_change() {
        let mut engine = ready_engine();
        engine.push(RouteLocation::new("/items/1")).unwrap();
        let cleanup = publish_current(&mut engine);
        let foreign = distinct_identity();

        for mismatch in identity_mismatches(cleanup.testing_identity(), &foreign) {
            let before = engine.snapshot();
            let stale = cleanup.testing_with_identity(mismatch);
            assert!(engine.complete_cleanup(valid_completion(&stale)).is_err());
            assert_eq!(engine.snapshot(), before);
        }

        let mut wrong_digest = valid_completion(&cleanup);
        wrong_digest.testing_set_binding(u64::MAX);
        let before = engine.snapshot();
        assert!(engine.complete_cleanup(wrong_digest).is_err());
        assert_eq!(engine.snapshot(), before);
        engine.complete_cleanup(valid_completion(&cleanup)).unwrap();
    }

    #[test]
    fn private_retention_keeps_64_terminal_records_and_distinguishes_lookup_facts() {
        let mut engine = ready_engine();
        let mut requests = Vec::new();
        for index in 0..66 {
            let request = engine
                .push(RouteLocation::new(format!("/items/{index}")))
                .unwrap();
            requests.push(request);
            settle_current(&mut engine);
        }

        assert!(matches!(
            engine.private_lookup(requests[0]),
            PrivateRequestLookup::Evicted
        ));
        assert!(matches!(
            engine.private_lookup(requests[1]),
            PrivateRequestLookup::Evicted
        ));
        assert!(matches!(
            engine.private_lookup(requests[2]),
            PrivateRequestLookup::Retained(_)
        ));
        assert!(matches!(
            engine.private_lookup(*requests.last().unwrap()),
            PrivateRequestLookup::Retained(_)
        ));

        let mut foreign = ready_engine();
        let unknown = foreign.push(RouteLocation::new("/items/0")).unwrap();
        assert!(matches!(
            engine.private_lookup(unknown),
            PrivateRequestLookup::Unknown
        ));
        assert_eq!(
            engine.testing_evicted_watermark(),
            Some(requests[1].sequence())
        );
    }

    #[test]
    fn confirmed_engine_rejects_new_host_admission_before_touching_fresh_issuer() {
        let mut engine = ready_engine();
        let mut issuer = HostAdmissionIssuer::new().unwrap();
        let before = engine.snapshot();

        assert_eq!(
            engine.request_host_admission(&mut issuer),
            Err(crate::admission::AdmissionError::AuthorityExists)
        );
        assert!(!issuer.has_outstanding());
        assert_eq!(engine.snapshot(), before);
    }

    #[test]
    fn push_reservation_exhaustion_is_zero_change_for_every_counter() {
        for counter in [
            ReservationCounter::Request,
            ReservationCounter::Transaction,
            ReservationCounter::FrameTask,
            ReservationCounter::CandidateFrame,
            ReservationCounter::CleanupBatch,
        ] {
            let mut engine = ready_engine();
            engine.testing_exhaust_reservation_counter(counter);
            let before = engine.snapshot();

            assert_eq!(
                engine.push(RouteLocation::new("/items/1")),
                Err(super::PushError::IdExhausted),
                "counter: {counter:?}"
            );
            assert_eq!(engine.snapshot(), before, "counter: {counter:?}");
        }
    }

    #[test]
    fn commit_ticket_is_logical_only_and_sealed_token_is_tracked_before_publication() {
        let mut engine = ready_engine();
        engine.push(RouteLocation::new("/items/1")).unwrap();
        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("expected frame task");
        };
        let ticket = engine.begin_view_report(task.identity()).unwrap().applied();
        let super::CommitTicket { identity } = &ticket;
        assert_eq!(identity, task.identity());
        let ids_after_action = engine.ids.snapshot();
        let sealed = engine
            .seal_commit(
                &ticket,
                CleanupManifestSeal::new(vec![], manifest_digest(3), 0),
            )
            .unwrap();
        let expected_cleanup = sealed.cleanup().clone();
        assert!(matches!(
            &engine.current.as_ref().unwrap().stage,
            WorkStage::CommitSealed { token } if token == &expected_cleanup
        ));
        assert_eq!(engine.active().kind(), ActivePresentationKind::None);
        assert!(engine.history().is_empty());

        let mut mutated = false;
        publish_sealed(&mut engine, sealed, || {
            mutated = true;
        })
        .unwrap();

        assert!(mutated);
        assert_eq!(engine.ids.snapshot(), ids_after_action);
        assert_eq!(engine.active().kind(), ActivePresentationKind::Route);
    }

    #[test]
    fn sealed_publication_preserves_later_queue_and_starts_next_request() {
        let mut engine = ready_engine();
        let first = engine.push(RouteLocation::new("/items/1")).unwrap();
        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("expected first frame task");
        };
        let ticket = engine.begin_view_report(task.identity()).unwrap().applied();
        let sealed = seal_current(&mut engine, ticket);
        let cleanup = sealed.cleanup().clone();

        let second = engine.push(RouteLocation::new("/about")).unwrap();
        let ids_after_second = engine.ids.snapshot();
        assert_eq!(
            engine.request(second).unwrap().status(),
            &RequestStatus::Queued
        );
        assert_eq!(engine.queue.front().unwrap().request, second);

        publish_sealed(&mut engine, sealed, || {}).unwrap();

        assert_eq!(engine.ids.snapshot(), ids_after_second);
        assert_eq!(
            engine.request(second).unwrap().status(),
            &RequestStatus::Queued
        );
        assert_eq!(engine.queue.front().unwrap().request, second);

        engine.complete_cleanup(valid_completion(&cleanup)).unwrap();

        assert_eq!(engine.ids.snapshot(), ids_after_second);
        assert_eq!(
            engine.request(first).unwrap().status(),
            &RequestStatus::Succeeded
        );
        assert_eq!(
            engine.request(second).unwrap().status(),
            &RequestStatus::Running(NavigationPhase::ApplyingFrame(0))
        );
        let EngineAction::FrameTask(second_task) = engine.action().unwrap() else {
            panic!("second request must start after first cleanup");
        };
        assert_eq!(second_task.identity().request(), second);
    }

    #[test]
    fn logical_cleanup_roots_are_retained_and_verified_in_exact_order() {
        let mut engine = ready_engine();
        engine.push(RouteLocation::new("/items/1")).unwrap();
        settle_current(&mut engine);
        let first = engine.active().frames()[0].clone();

        engine
            .push(RouteLocation::new("/items/1").with_fragment("details"))
            .unwrap();
        let EngineAction::FrameTask(fragment) = engine.action().unwrap() else {
            panic!("expected fragment task");
        };
        let fragment_ticket = engine
            .begin_view_report(fragment.identity())
            .unwrap()
            .applied();
        assert_eq!(
            engine.current.as_ref().unwrap().cleanup_roots,
            vec![first.clone()]
        );

        let second = RouteFrameVersion::new(
            first.router_identity(),
            first.host(),
            first.slot(),
            first.origin().clone(),
            RouteFrameId::new(first.frame().raw() + 10_000),
            first.generation(),
        );
        let foreign = RouteFrameVersion::new(
            first.router_identity(),
            first.host(),
            first.slot(),
            first.origin().clone(),
            RouteFrameId::new(first.frame().raw() + 20_000),
            first.generation(),
        );
        engine.current.as_mut().unwrap().cleanup_roots = vec![first.clone(), second.clone()];

        for invalid in [
            vec![],
            vec![first.clone()],
            vec![first.clone(), second.clone(), first.clone()],
            vec![first.clone(), first.clone()],
            vec![first.clone(), foreign],
            vec![second.clone(), first.clone()],
        ] {
            let before = engine.snapshot();
            assert_eq!(
                engine
                    .seal_commit(
                        &fragment_ticket.testing_duplicate(),
                        CleanupManifestSeal::new(invalid, manifest_digest(4), 7),
                    )
                    .unwrap_err(),
                CommitError::InvalidManifest
            );
            assert_eq!(engine.snapshot(), before);
        }

        let sealed = engine
            .seal_commit(
                &fragment_ticket,
                CleanupManifestSeal::new(vec![first, second], manifest_digest(4), 7),
            )
            .unwrap();
        assert_eq!(sealed.cleanup().obligation_count(), 7);
    }

    #[test]
    fn root_count_coherence_is_rejected_before_any_mutation() {
        let mut engine = ready_engine();
        engine.push(RouteLocation::new("/items/1")).unwrap();
        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("expected frame task");
        };
        let ticket = engine.begin_view_report(task.identity()).unwrap().applied();

        let before = engine.snapshot();
        assert_eq!(
            engine
                .seal_commit(
                    &ticket.testing_duplicate(),
                    CleanupManifestSeal::new(vec![], manifest_digest(5), 1),
                )
                .unwrap_err(),
            CommitError::InvalidManifest
        );
        assert_eq!(engine.snapshot(), before);

        let root = engine.current.as_ref().unwrap().target_frame.clone();
        engine.current.as_mut().unwrap().cleanup_roots = vec![root.clone()];
        let before = engine.snapshot();
        assert_eq!(
            engine
                .seal_commit(
                    &ticket,
                    CleanupManifestSeal::new(vec![root], manifest_digest(5), 0),
                )
                .unwrap_err(),
            CommitError::InvalidManifest
        );
        assert_eq!(engine.snapshot(), before);
    }

    #[test]
    fn duplicate_and_foreign_sealed_commit_capabilities_are_zero_write() {
        let mut engine = ready_engine();
        engine.push(RouteLocation::new("/items/1")).unwrap();
        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("expected frame task");
        };
        let ticket = engine.begin_view_report(task.identity()).unwrap().applied();
        let sealed = seal_current(&mut engine, ticket);
        let duplicate = sealed.testing_duplicate();
        let foreign_identity = distinct_identity();
        let foreign = sealed.testing_with_identity(foreign_identity);

        for invalid in [foreign] {
            let before = engine.snapshot();
            let mut mutated = false;
            assert_eq!(
                publish_sealed(&mut engine, invalid, || {
                    mutated = true;
                })
                .unwrap_err(),
                CommitError::Stale
            );
            assert!(!mutated);
            assert_eq!(engine.snapshot(), before);
        }

        publish_sealed(&mut engine, sealed, || {}).unwrap();
        let before = engine.snapshot();
        let mut mutated = false;
        assert_eq!(
            publish_sealed(&mut engine, duplicate, || {
                mutated = true;
            })
            .unwrap_err(),
            CommitError::Stale
        );
        assert!(!mutated);
        assert_eq!(engine.snapshot(), before);
    }

    #[test]
    fn manifest_digest_count_and_batch_are_bound_into_the_token() {
        let mut engine = ready_engine();
        engine.push(RouteLocation::new("/items/1")).unwrap();
        settle_current(&mut engine);
        engine.push(RouteLocation::new("/about")).unwrap();
        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("expected replacement task");
        };
        let ticket = engine.begin_view_report(task.identity()).unwrap().applied();
        let current = engine.current.as_ref().unwrap();
        let exact = CleanupBatchToken::sealed(
            ticket.identity().clone(),
            current.cleanup_batch,
            &current.cleanup_roots,
            manifest_digest(10),
            NONZERO_OBLIGATION_COUNT,
        );
        let different_manifest = CleanupBatchToken::sealed(
            ticket.identity().clone(),
            current.cleanup_batch,
            &current.cleanup_roots,
            manifest_digest(11),
            NONZERO_OBLIGATION_COUNT,
        );
        let different_count = CleanupBatchToken::sealed(
            ticket.identity().clone(),
            current.cleanup_batch,
            &current.cleanup_roots,
            manifest_digest(10),
            NONZERO_OBLIGATION_COUNT + 1,
        );
        let different_batch = CleanupBatchToken::sealed(
            ticket.identity().clone(),
            CleanupBatchId::new(current.cleanup_batch.raw() + 1),
            &current.cleanup_roots,
            manifest_digest(10),
            NONZERO_OBLIGATION_COUNT,
        );

        for different in [different_manifest, different_count, different_batch] {
            assert_ne!(exact, different);
            assert_ne!(exact.testing_binding(), different.testing_binding());
        }

        let sealed = engine
            .seal_commit(
                &ticket,
                CleanupManifestSeal::new(
                    current.cleanup_roots.clone(),
                    manifest_digest(10),
                    NONZERO_OBLIGATION_COUNT,
                ),
            )
            .unwrap();
        assert_eq!(sealed.cleanup(), &exact);
    }

    #[test]
    fn nonzero_completion_is_aggregate_attestation_without_core_receipts() {
        let mut engine = ready_engine();
        engine.push(RouteLocation::new("/items/1")).unwrap();
        settle_current(&mut engine);
        let request = engine.push(RouteLocation::new("/about")).unwrap();
        let cleanup = publish_current(&mut engine);
        assert_eq!(cleanup.obligation_count(), NONZERO_OBLIGATION_COUNT);

        engine.complete_cleanup(valid_completion(&cleanup)).unwrap();
        let settled = engine
            .request(request)
            .unwrap()
            .trace()
            .iter()
            .filter(|event| event.kind() == NavigationTraceEventKind::CleanupSettled)
            .collect::<Vec<_>>();
        assert_eq!(settled.len(), 1);
        assert_eq!(settled[0].cleanup_disposition(), None);
    }

    #[test]
    fn completion_rejects_wrong_token_count_zero_representation_and_binding() {
        let mut engine = ready_engine();
        engine.push(RouteLocation::new("/items/1")).unwrap();
        let cleanup = publish_current(&mut engine);

        let mut foreign_engine = ready_engine();
        foreign_engine.push(RouteLocation::new("/items/1")).unwrap();
        let foreign = publish_current(&mut foreign_engine);

        let mut wrong_binding = valid_completion(&cleanup);
        wrong_binding.testing_set_binding(u64::MAX);
        let wrong_batch =
            cleanup.testing_with_batch(CleanupBatchId::new(cleanup.testing_batch().raw() + 1));
        for invalid in [
            CleanupCompletion::attest(foreign, CleanupCompletionDigest::canonical_empty(), 0),
            CleanupCompletion::attest(cleanup.clone(), CleanupCompletionDigest::from_opaque(76), 1),
            CleanupCompletion::attest(cleanup.clone(), CleanupCompletionDigest::from_opaque(77), 0),
            CleanupCompletion::attest(wrong_batch, CleanupCompletionDigest::canonical_empty(), 0),
            wrong_binding,
        ] {
            let before = engine.snapshot();
            assert!(engine.complete_cleanup(invalid).is_err());
            assert_eq!(engine.snapshot(), before);
        }

        engine.complete_cleanup(valid_completion(&cleanup)).unwrap();
    }

    #[test]
    fn generation_overflow_settles_accepted_request_and_does_not_poison_prior_cleanup() {
        let mut engine = ready_engine();
        let first = engine.push(RouteLocation::new("/items/1")).unwrap();
        let cleanup = publish_current(&mut engine);
        engine.testing_set_active_generation(RouteGeneration::testing_max());
        let overflow = engine
            .push(RouteLocation::new("/items/1").with_fragment("details"))
            .unwrap();

        engine.complete_cleanup(valid_completion(&cleanup)).unwrap();

        assert_eq!(
            engine.request(first).unwrap().status(),
            &RequestStatus::Succeeded
        );
        assert!(matches!(
            engine.request(overflow).unwrap().status(),
            RequestStatus::Failed(_)
        ));
        assert_eq!(engine.current_status(), None);
        assert!(engine.action().is_none());
    }

    #[test]
    fn match_failure_clears_running_summary_and_advances_queue_consistently() {
        let mut engine = ready_engine();
        let failed = engine.push(RouteLocation::new("/missing")).unwrap();

        assert!(matches!(
            engine.request(failed).unwrap().status(),
            RequestStatus::Failed(_)
        ));
        assert_eq!(engine.current_status(), None);

        let next = engine.push(RouteLocation::new("/items/1")).unwrap();
        assert_eq!(
            engine.request(next).unwrap().status(),
            &RequestStatus::Running(NavigationPhase::ApplyingFrame(0))
        );
    }

    #[test]
    fn queued_match_failure_is_settled_before_next_reserved_request_starts() {
        let mut engine = ready_engine();
        engine.push(RouteLocation::new("/items/1")).unwrap();
        let cleanup = publish_current(&mut engine);
        let failed = engine.push(RouteLocation::new("/missing")).unwrap();
        let next = engine.push(RouteLocation::new("/about")).unwrap();

        engine.complete_cleanup(valid_completion(&cleanup)).unwrap();

        assert!(matches!(
            engine.request(failed).unwrap().status(),
            RequestStatus::Failed(_)
        ));
        assert_eq!(
            engine.request(next).unwrap().status(),
            &RequestStatus::Running(NavigationPhase::ApplyingFrame(0))
        );
        assert!(matches!(engine.action(), Some(EngineAction::FrameTask(_))));
    }

    #[test]
    fn frame_task_issued_is_the_first_trace_event_for_normal_push() {
        let mut engine = ready_engine();
        let request = engine.push(RouteLocation::new("/items/1")).unwrap();

        assert_eq!(
            engine.request(request).unwrap().trace()[0].kind(),
            NavigationTraceEventKind::FrameTaskIssued
        );
    }
}
