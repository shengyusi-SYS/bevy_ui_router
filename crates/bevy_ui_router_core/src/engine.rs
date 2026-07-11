#![allow(dead_code)]

use std::collections::VecDeque;

use crate::admission::{
    AdmissionError, HostAdmissionConfirmation, HostAdmissionIssuer, HostAdmissionToken, HostGrant,
};
use crate::branch::{MatchedBranch, MatchedRoute};
use crate::cleanup::{CleanupBatchToken, CleanupCompletion};
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

#[derive(Clone, Debug)]
pub(crate) struct RootFrameTask {
    identity: RootTaskIdentity,
    location: RouteLocation,
    matched: MatchedRoute,
}

impl RootFrameTask {
    pub(crate) const fn identity(&self) -> &RootTaskIdentity {
        &self.identity
    }

    pub(crate) const fn location(&self) -> &RouteLocation {
        &self.location
    }

    #[allow(dead_code)]
    pub(crate) const fn matched(&self) -> &MatchedRoute {
        &self.matched
    }
}

#[derive(Debug)]
pub(crate) enum EngineAction {
    FrameTask(RootFrameTask),
    Commit(CommitTicket),
}

#[derive(Debug)]
pub(crate) struct CommitTicket {
    identity: RootTaskIdentity,
    cleanup: CleanupBatchToken,
}

impl CommitTicket {
    #[cfg(test)]
    fn testing_duplicate(&self) -> Self {
        Self {
            identity: self.identity.clone(),
            cleanup: self.cleanup.clone(),
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
        Self {
            identity,
            cleanup: self.cleanup.clone(),
        }
    }

    pub(crate) const fn cleanup(&self) -> &CleanupBatchToken {
        &self.cleanup
    }
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
    PostCommitCleanup,
}

#[derive(Clone, Debug)]
struct CurrentWork {
    identity: RootTaskIdentity,
    branch: MatchedBranch,
    target_frame: RouteFrameVersion,
    history_intent: Option<HistoryIntent>,
    stage: WorkStage,
    cleanup: CleanupBatchToken,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct QueuedWork {
    request: NavigationRequestId,
    reserved: ReservedNavigationIds,
}

#[derive(Debug)]
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
        if self.current.is_none() {
            self.start_next();
        }
        Ok(request)
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

    pub(crate) fn commit<F>(
        &mut self,
        ticket: CommitTicket,
        mutation: F,
    ) -> Result<CommitReceipt, CommitError>
    where
        F: FnOnce(),
    {
        let valid = self.current.as_ref().is_some_and(|current| {
            current.stage == WorkStage::ReadyToCommit
                && current.identity == ticket.identity
                && current.cleanup == ticket.cleanup
        });
        if !valid {
            return Err(CommitError::Stale);
        }
        mutation();
        Ok(self.publish(ticket.cleanup))
    }

    pub(crate) fn complete_cleanup(
        &mut self,
        completion: CleanupCompletion,
    ) -> Result<(), CleanupError> {
        let valid = self.current.as_ref().is_some_and(|current| {
            current.stage == WorkStage::PostCommitCleanup && completion.matches(&current.cleanup)
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
        if completion.receipts().is_empty() {
            record
                .append_trace(NavigationTraceEvent::cleanup_settled(None))
                .expect("terminal trace is open");
        } else {
            for receipt in completion.receipts() {
                record
                    .append_trace(NavigationTraceEvent::cleanup_settled(Some(
                        receipt.disposition(),
                    )))
                    .expect("terminal trace is open");
            }
        }
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
        let cleanup_manifest = if active_frame.is_some() && !equivalent {
            vec![active_frame.expect("checked active frame").clone()]
        } else {
            Vec::new()
        };
        let cleanup = CleanupBatchToken::sealed(
            identity.clone(),
            queued.reserved.cleanup_batch(),
            cleanup_manifest,
        );
        let stage = if equivalent {
            WorkStage::ReadyToCommit
        } else {
            WorkStage::Applying
        };
        self.current = Some(CurrentWork {
            identity: identity.clone(),
            branch,
            target_frame,
            history_intent: Some(HistoryIntent::Push(location.clone())),
            stage,
            cleanup: cleanup.clone(),
        });

        if equivalent {
            self.transition_with_trace(request, NavigationPhase::ReadyToSwitch);
            self.action = Some(EngineAction::Commit(CommitTicket { identity, cleanup }));
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
                location,
                matched: leaf,
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

    fn publish(&mut self, cleanup: CleanupBatchToken) -> CommitReceipt {
        let (request, target_frame, branch) = {
            let current = self.current.as_ref().expect("publication was preflighted");
            (
                current.identity.request,
                current.target_frame.clone(),
                current.branch.clone(),
            )
        };
        let location = self
            .request(request)
            .expect("running record is retained")
            .requested()
            .clone();
        let active = ActivePresentation::route(location, vec![target_frame]);
        let current = self.current.as_mut().expect("publication was preflighted");
        current
            .history_intent
            .take()
            .expect("commit has one private history intent")
            .publish(&mut self.history);
        current.stage = WorkStage::PostCommitCleanup;
        debug_assert_eq!(current.cleanup, cleanup);
        self.active_branch = Some(branch);
        self.state.set_active(active);
        let record = self
            .record_mut(request)
            .expect("running record is retained");
        record
            .append_trace(NavigationTraceEvent::commit_published())
            .expect("running trace is open");
        record
            .transition(RequestStatus::Running(NavigationPhase::PostCommitCleanup))
            .expect("request is running");
        record
            .append_trace(NavigationTraceEvent::request_phase_changed(
                NavigationPhase::PostCommitCleanup,
            ))
            .expect("running trace is open");
        self.state.set_request(Some(RequestStatus::Running(
            NavigationPhase::PostCommitCleanup,
        )));
        CommitReceipt { cleanup }
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
            cleanup: self
                .engine
                .current
                .as_ref()
                .expect("report retains current work")
                .cleanup
                .clone(),
        }
    }
}

#[derive(Debug)]
pub(crate) struct CommitReceipt {
    cleanup: CleanupBatchToken,
}

impl CommitReceipt {
    pub(crate) const fn cleanup(&self) -> &CleanupBatchToken {
        &self.cleanup
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
        RouterEngine,
    };
    use crate::admission::HostAdmissionIssuer;
    use crate::frame::{FrameOrigin, RouteFrameVersion};
    use crate::ids::{ReservationCounter, RouteFrameId, RouteGeneration, RouteHostId, RouteSlotId};
    use crate::location::RouteLocation;
    use crate::pattern::RoutePattern;
    use crate::presentation::ActivePresentationKind;
    use crate::request::{NavigationPhase, RequestStatus};
    use crate::route_tree::RouteTree;
    use crate::trace::{CleanupDisposition, NavigationTraceEventKind};

    fn tree() -> RouteTree {
        let mut builder = RouteTree::builder()
            .root(RoutePattern::path("/").unwrap())
            .unwrap();
        let root = builder.root_id().unwrap();
        builder
            .route(root, RoutePattern::path("items/:id").unwrap())
            .unwrap();
        builder
            .route(root, RoutePattern::path("about").unwrap())
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
        engine.commit(ticket, || {}).unwrap().cleanup().clone()
    }

    fn settle_current(engine: &mut RouterEngine) {
        let cleanup = publish_current(engine);
        engine.complete_cleanup(valid_completion(&cleanup)).unwrap();
    }

    fn valid_completion(
        cleanup: &crate::cleanup::CleanupBatchToken,
    ) -> crate::cleanup::CleanupCompletion {
        if cleanup.obligation_count() == 0 {
            cleanup.zero_completion().unwrap()
        } else {
            cleanup.testing_removed_completion().unwrap()
        }
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
        assert_eq!(engine.commit(stale, || {}).unwrap_err(), CommitError::Stale);
        assert_eq!(engine.snapshot(), before);

        let receipt = engine.commit(ticket, || {}).unwrap();
        assert_eq!(engine.active().kind(), ActivePresentationKind::Route);
        assert_eq!(engine.history().len(), 1);
        assert_eq!(
            engine.current_status(),
            Some(&RequestStatus::Running(NavigationPhase::PostCommitCleanup))
        );
        assert_eq!(receipt.cleanup().obligation_count(), 0);
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
            assert_eq!(engine.commit(stale, || {}).unwrap_err(), CommitError::Stale);
            assert_eq!(engine.snapshot(), before);
        }

        let cleanup = engine.commit(exact, || {}).unwrap().cleanup().clone();
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
        assert_eq!(fragment_cleanup.obligation_count(), 1);
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
        assert_eq!(changed_cleanup.obligation_count(), 1);
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
        wrong_digest.testing_set_digest(u64::MAX);
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
    fn commit_runs_unit_mutation_then_automatically_publishes_presealed_cleanup() {
        let mut engine = ready_engine();
        engine.push(RouteLocation::new("/items/1")).unwrap();
        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("expected frame task");
        };
        let ticket = engine.begin_view_report(task.identity()).unwrap().applied();
        let expected_cleanup = ticket.cleanup().clone();
        let ids_after_action = engine.ids.snapshot();
        let mut mutated = false;

        let receipt = engine
            .commit(ticket, || {
                mutated = true;
            })
            .unwrap();

        assert!(mutated);
        assert_eq!(receipt.cleanup(), &expected_cleanup);
        assert_eq!(engine.ids.snapshot(), ids_after_action);
        assert_eq!(engine.active().kind(), ActivePresentationKind::Route);
    }

    #[test]
    fn fragment_and_replacement_freeze_exact_old_frame_cleanup_manifest() {
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
        assert_eq!(fragment_ticket.cleanup().obligations(), &[first.clone()]);
        let fragment_cleanup = engine
            .commit(fragment_ticket, || {})
            .unwrap()
            .cleanup()
            .clone();
        engine
            .complete_cleanup(fragment_cleanup.testing_removed_completion().unwrap())
            .unwrap();
        let fragment_active = engine.active().frames()[0].clone();

        engine.push(RouteLocation::new("/about")).unwrap();
        let EngineAction::FrameTask(replacement) = engine.action().unwrap() else {
            panic!("expected replacement task");
        };
        let replacement_ticket = engine
            .begin_view_report(replacement.identity())
            .unwrap()
            .applied();
        assert_eq!(
            replacement_ticket.cleanup().obligations(),
            &[fragment_active]
        );
    }

    #[test]
    fn nonzero_cleanup_rejects_missing_duplicate_foreign_and_out_of_order_receipts() {
        let mut engine = ready_engine();
        engine.push(RouteLocation::new("/items/1")).unwrap();
        settle_current(&mut engine);
        let request = engine.push(RouteLocation::new("/about")).unwrap();
        let cleanup = publish_current(&mut engine);
        assert_eq!(cleanup.obligation_count(), 1);

        for invalid in [
            cleanup.testing_missing_completion(),
            cleanup.testing_duplicate_completion(CleanupDisposition::Removed),
            cleanup.testing_foreign_completion(CleanupDisposition::Removed),
            cleanup.testing_foreign_batch_completion(CleanupDisposition::Removed),
            cleanup.testing_out_of_order_completion(CleanupDisposition::Removed),
        ] {
            let before = engine.snapshot();
            assert!(engine.complete_cleanup(invalid).is_err());
            assert_eq!(engine.snapshot(), before);
        }

        engine
            .complete_cleanup(cleanup.testing_removed_completion().unwrap())
            .unwrap();
        let settled = engine
            .request(request)
            .unwrap()
            .trace()
            .iter()
            .filter(|event| event.kind() == NavigationTraceEventKind::CleanupSettled)
            .collect::<Vec<_>>();
        assert_eq!(settled.len(), 1);
        assert_eq!(
            settled[0].cleanup_disposition(),
            Some(CleanupDisposition::Removed)
        );
    }

    #[test]
    fn cleanup_ledger_rejects_two_exact_receipts_in_reverse_manifest_order() {
        let mut engine = ready_engine();
        engine.push(RouteLocation::new("/items/1")).unwrap();
        settle_current(&mut engine);
        engine.push(RouteLocation::new("/about")).unwrap();
        let cleanup = publish_current(&mut engine);
        let first = cleanup.obligations()[0].clone();
        let second = RouteFrameVersion::new(
            first.router_identity(),
            first.host(),
            first.slot(),
            first.origin().clone(),
            RouteFrameId::new(first.frame().raw() + 1),
            first.generation(),
        );
        let two = cleanup.testing_with_manifest(vec![first, second]);
        let reversed = cleanup
            .testing_with_manifest(two.obligations().iter().cloned().rev().collect::<Vec<_>>());
        assert_ne!(
            two.testing_manifest_digest(),
            reversed.testing_manifest_digest()
        );
        assert_ne!(
            cleanup.testing_manifest_digest(),
            cleanup.testing_with_next_batch().testing_manifest_digest()
        );
        let receipts = vec![
            two.receipt(1, CleanupDisposition::Removed).unwrap(),
            two.receipt(0, CleanupDisposition::Removed).unwrap(),
        ];

        assert_eq!(
            two.completion_from_receipts(receipts),
            Err(crate::cleanup::CleanupLedgerError::DuplicateOrOutOfOrderReceipt)
        );
        assert_eq!(
            cleanup.zero_completion(),
            Err(crate::cleanup::CleanupLedgerError::MissingReceipt)
        );
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

        engine
            .complete_cleanup(cleanup.zero_completion().unwrap())
            .unwrap();

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

        engine
            .complete_cleanup(cleanup.zero_completion().unwrap())
            .unwrap();

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
