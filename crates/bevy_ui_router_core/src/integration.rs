use crate::admission::{
    AdmissionError as CoreAdmissionError, HostAdmissionIssuer,
    HostAdmissionToken as CoreHostAdmissionToken, HostGrant as CoreHostGrant,
};
use crate::cleanup::{
    CleanupBatchToken, CleanupCompletion as CoreCleanupCompletion, CleanupCompletionDigest,
    CleanupManifestDigest, CleanupManifestSeal, CleanupSummary,
};
use crate::engine::{
    CommitError as CoreCommitError, CommitTicket as CoreCommitTicket,
    EngineAction as CoreEngineAction, EngineInstallError as CoreEngineInstallError,
    PreparedPublication, PrivateRequestLookup, PushError as CorePushError,
    ReportError as CoreReportError, RootFrameTask as CoreRootFrameTask,
    RouterEngine as CoreRouterEngine,
};
use crate::frame::RouteFrameVersion;
use crate::history::{HistorySnapshot, MemoryHistory};
use crate::ids::{NavigationRequestId, RouterIdentity};
use crate::location::RouteLocation;
use crate::meta::ErrorBoundaryKey;
use crate::presentation::{ActivePresentation, RouterAvailability};
use crate::request::{
    NavigationFailure, NavigationFailureKind, NavigationPhase, NavigationRequestRecord,
    RequestStatus,
};
use crate::route_tree::RouteTree;
use crate::trace::{CleanupDisposition, NavigationTraceEvent};
use crate::view_input::RouteViewInput;
use crate::view_key::RouteViewKey;
use crate::{
    HostSnapshot, HostTraceEvent, HostTraceEventKind,
    diagnostics::{DiagnosticEvent, DiagnosticSnapshot},
    fingerprint::RouteBranchFingerprint,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntegrationError {
    Exhausted,
    AwaitingHost,
    OutstandingAdmission,
    AuthorityExists,
    ForeignRouter,
    Duplicate,
    Stale,
    Invalid,
    InvalidManifest,
}

pub struct HostAdmissionToken {
    inner: CoreHostAdmissionToken,
}

pub struct HostGrant {
    inner: CoreHostGrant,
}

impl HostGrant {
    pub fn version(&self) -> &crate::HostVersion {
        self.inner.version()
    }

    pub fn top_slot(&self) -> crate::ids::RouteSlotId {
        self.inner.top_slot()
    }
}

pub fn host_version_matches_frame(version: &crate::HostVersion, frame: &RouteFrameVersion) -> bool {
    version.router_identity() == frame.router_identity() && version.host() == frame.host()
}

pub fn root_view(tree: &RouteTree) -> Option<&RouteViewKey> {
    tree.node(tree.root())?.options().view_binding()
}

pub fn for_each_route_view_binding(
    tree: &RouteTree,
    mut visit: impl FnMut(Option<&RouteViewKey>, Option<&ErrorBoundaryKey>, Option<&RouteViewKey>),
) {
    for node in tree.nodes_in_declaration_order() {
        let options = node.options();
        visit(
            options.view_binding(),
            options.error_boundary(),
            options.error_view_binding().map(|(_, view)| view),
        );
    }
}

pub struct RootFrameTask {
    inner: CoreRootFrameTask,
}

impl RootFrameTask {
    pub fn request(&self) -> NavigationRequestId {
        self.inner.identity().request()
    }

    pub fn view_request_sequence(&self) -> u64 {
        self.inner.identity().request().sequence()
    }

    pub fn frame(&self) -> &RouteFrameVersion {
        let current = &self.inner;
        // The engine creates the same logical version from this sealed identity
        // and the projection is the only cross-crate representation ECS needs
        current.frame_version()
    }

    pub fn view(&self) -> &RouteViewKey {
        self.inner.view()
    }

    pub fn input(&self) -> &RouteViewInput {
        self.inner.input()
    }

    pub fn take_input(&mut self) -> RouteViewInput {
        self.inner.take_input()
    }
}

pub enum EngineAction {
    FrameTask(RootFrameTask),
    Commit(CommitTicket),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewFailureKind {
    Build,
    Apply,
}

pub struct CommitTicket {
    inner: CoreCommitTicket,
}

pub struct CleanupManifest {
    logical_roots: Vec<RouteFrameVersion>,
    manifest_digest: u64,
    obligation_count: usize,
}

impl CleanupManifest {
    pub fn empty() -> Self {
        Self {
            logical_roots: Vec::new(),
            manifest_digest: 0,
            obligation_count: 0,
        }
    }

    pub fn new(
        logical_roots: Vec<RouteFrameVersion>,
        manifest_digest: u64,
        obligation_count: usize,
    ) -> Self {
        Self {
            logical_roots,
            manifest_digest,
            obligation_count,
        }
    }
}

pub struct CleanupBatch {
    inner: CleanupBatchToken,
    manifest_digest: u64,
    obligation_count: usize,
}

impl Clone for CleanupBatch {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            manifest_digest: self.manifest_digest,
            obligation_count: self.obligation_count,
        }
    }
}

impl CleanupBatch {
    pub fn manifest_digest(&self) -> u64 {
        self.manifest_digest
    }

    pub fn obligation_count(&self) -> usize {
        self.obligation_count
    }

    pub fn complete_empty(self) -> CleanupCompletion {
        assert_eq!(self.obligation_count, 0, "cleanup batch is not empty");
        CleanupCompletion {
            inner: CoreCleanupCompletion::attest(
                self.inner,
                CleanupCompletionDigest::canonical_empty(),
                0,
            ),
        }
    }

    pub fn complete(self, completion_digest: u64, settled_count: usize) -> CleanupCompletion {
        CleanupCompletion {
            inner: CoreCleanupCompletion::attest(
                self.inner,
                CleanupCompletionDigest::from_opaque(completion_digest),
                settled_count,
            ),
        }
    }
}

pub struct CleanupCompletion {
    inner: CoreCleanupCompletion,
}

pub struct PreparedCommit {
    publication: PreparedPublication,
    cleanup: CleanupBatch,
}

impl PreparedCommit {
    pub fn cleanup(&self) -> &CleanupBatch {
        &self.cleanup
    }
}

pub struct NavigationRequestView<'a> {
    record: &'a NavigationRequestRecord,
}

pub enum RequestLookup<'a> {
    Found(NavigationRequestView<'a>),
    NotRetained,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MissingRequestClassification {
    NotRetained,
    Unknown,
}

#[derive(Clone, Eq, PartialEq)]
pub struct RequestRetentionClassifier {
    router: RouterIdentity,
    evicted_through: Option<u64>,
    highest_issued: Option<u64>,
}

impl std::fmt::Debug for RequestRetentionClassifier {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("RequestRetentionClassifier { .. }")
    }
}

impl RequestRetentionClassifier {
    pub(crate) const fn project(
        router: RouterIdentity,
        evicted_through: Option<u64>,
        highest_issued: Option<u64>,
    ) -> Self {
        Self {
            router,
            evicted_through,
            highest_issued,
        }
    }

    pub fn classify(&self, id: NavigationRequestId) -> MissingRequestClassification {
        if id.router_identity() != self.router
            || !self
                .highest_issued
                .is_some_and(|highest| id.sequence() <= highest)
        {
            return MissingRequestClassification::Unknown;
        }
        if self
            .evicted_through
            .is_some_and(|sequence| id.sequence() <= sequence)
        {
            MissingRequestClassification::NotRetained
        } else {
            MissingRequestClassification::Unknown
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouterSnapshotProjection {
    requested: Option<RouteLocation>,
    matched: Option<RouteBranchFingerprint>,
    active: ActivePresentation,
    availability: RouterAvailability,
    pending_phase: Option<NavigationPhase>,
    history: HistorySnapshot,
    running_request: Option<NavigationRequestId>,
    earliest_retained_sequence: Option<u64>,
    request_retention: RequestRetentionClassifier,
    diagnostics: DiagnosticSnapshot,
    host: HostSnapshot,
}

impl RouterSnapshotProjection {
    pub fn requested(&self) -> Option<&RouteLocation> {
        self.requested.as_ref()
    }

    pub fn matched(&self) -> Option<&RouteBranchFingerprint> {
        self.matched.as_ref()
    }

    pub fn active(&self) -> &ActivePresentation {
        &self.active
    }

    pub fn availability(&self) -> &RouterAvailability {
        &self.availability
    }

    pub fn pending_phase(&self) -> Option<&NavigationPhase> {
        self.pending_phase.as_ref()
    }

    pub fn history(&self) -> &HistorySnapshot {
        &self.history
    }

    pub fn running_request(&self) -> Option<NavigationRequestId> {
        self.running_request
    }

    pub fn earliest_retained_sequence(&self) -> Option<u64> {
        self.earliest_retained_sequence
    }

    pub fn request_retention(&self) -> &RequestRetentionClassifier {
        &self.request_retention
    }

    pub fn diagnostics(&self) -> &DiagnosticSnapshot {
        &self.diagnostics
    }

    pub fn host(&self) -> &HostSnapshot {
        &self.host
    }
}

pub fn project_cleanup_summary(
    foreground_pending: bool,
    quarantine_count: usize,
    last_disposition: Option<CleanupDisposition>,
) -> CleanupSummary {
    CleanupSummary::project(foreground_pending, quarantine_count, last_disposition)
}

impl NavigationRequestView<'_> {
    pub fn id(&self) -> NavigationRequestId {
        self.record.id()
    }

    pub fn requested(&self) -> &RouteLocation {
        self.record.requested()
    }

    pub fn matched(&self) -> Option<&RouteBranchFingerprint> {
        self.record.matched()
    }

    pub fn status(&self) -> &RequestStatus {
        self.record.status()
    }

    pub fn trace(&self) -> &[NavigationTraceEvent] {
        self.record.trace()
    }

    pub fn diagnostics(&self) -> &[DiagnosticEvent] {
        self.record.diagnostics()
    }
}

pub struct RouterEngine {
    inner: CoreRouterEngine,
    admission: HostAdmissionIssuer,
    host_trace: Vec<HostTraceEvent>,
}

impl RouterEngine {
    pub fn install(tree: RouteTree, fallback: RouteViewKey) -> Result<Self, IntegrationError> {
        let inner = CoreRouterEngine::install(tree, fallback).map_err(map_install_error)?;
        let admission = HostAdmissionIssuer::new().map_err(|_| IntegrationError::Exhausted)?;
        Ok(Self {
            inner,
            admission,
            host_trace: Vec::new(),
        })
    }

    pub fn request_host_admission(&mut self) -> Result<HostAdmissionToken, IntegrationError> {
        let inner = self
            .inner
            .request_host_admission(&mut self.admission)
            .map_err(map_admission_error)?;
        self.host_trace.push(HostTraceEvent::project(
            HostTraceEventKind::AdmissionRequested,
            None,
            None,
        ));
        Ok(HostAdmissionToken { inner })
    }

    pub fn reject_host_admission(
        &mut self,
        token: HostAdmissionToken,
    ) -> Result<(), IntegrationError> {
        self.admission
            .reject(&token.inner)
            .map_err(map_admission_error)?;
        self.host_trace.push(HostTraceEvent::project(
            HostTraceEventKind::AdmissionRejected,
            None,
            None,
        ));
        Ok(())
    }

    pub fn grant_host_admission(
        &mut self,
        token: &HostAdmissionToken,
    ) -> Result<HostGrant, IntegrationError> {
        let inner = self
            .inner
            .grant_host_admission(&mut self.admission, &token.inner)
            .map_err(map_admission_error)?;
        self.host_trace.push(HostTraceEvent::project(
            HostTraceEventKind::HostGranted,
            Some(inner.version().clone()),
            Some(inner.top_slot()),
        ));
        Ok(HostGrant { inner })
    }

    pub fn reject_host_after_mutation(&mut self, grant: HostGrant) -> Result<(), IntegrationError> {
        let version = grant.inner.version().clone();
        let slot = grant.inner.top_slot();
        self.admission
            .reject_after_mutation(&grant.inner)
            .map_err(map_admission_error)?;
        self.host_trace.push(HostTraceEvent::project(
            HostTraceEventKind::AdmissionRejectedAfterMutation,
            Some(version),
            Some(slot),
        ));
        Ok(())
    }

    pub fn record_top_slot_registered(
        &mut self,
        grant: &HostGrant,
    ) -> Result<(), IntegrationError> {
        if !self.admission.is_outstanding_grant(&grant.inner) {
            return Err(IntegrationError::Stale);
        }
        let version = grant.inner.version();
        let slot = grant.inner.top_slot();
        if self.host_trace.iter().any(|event| {
            event.kind() == HostTraceEventKind::TopSlotRegistered
                && event.version() == Some(version)
                && event.slot() == Some(slot)
        }) {
            return Err(IntegrationError::Duplicate);
        }
        self.host_trace.push(HostTraceEvent::project(
            HostTraceEventKind::TopSlotRegistered,
            Some(version.clone()),
            Some(slot),
        ));
        Ok(())
    }

    pub fn confirm_host(&mut self, grant: HostGrant) {
        let confirmation = self
            .admission
            .confirm(&grant.inner)
            .expect("a live sealed host grant has one valid confirmation");
        self.inner
            .confirm_host(&self.admission, &confirmation)
            .expect("the engine cannot change while its sealed host grant is outstanding");
        self.host_trace.push(HostTraceEvent::project(
            HostTraceEventKind::HostReady,
            self.inner.host().cloned(),
            self.inner.top_slot(),
        ));
    }

    pub fn push(
        &mut self,
        location: RouteLocation,
    ) -> Result<NavigationRequestId, IntegrationError> {
        self.inner.enqueue_push(location).map_err(map_push_error)
    }

    pub fn collect(&mut self) {
        self.inner.collect();
    }

    pub fn action(&mut self) -> Option<EngineAction> {
        self.inner.action().map(|action| match action {
            CoreEngineAction::FrameTask(inner) => EngineAction::FrameTask(RootFrameTask { inner }),
            CoreEngineAction::Commit(inner) => EngineAction::Commit(CommitTicket { inner }),
        })
    }

    pub fn report_applied(
        &mut self,
        task: RootFrameTask,
    ) -> Result<CommitTicket, IntegrationError> {
        self.inner
            .begin_view_report(task.inner.identity())
            .map(|session| CommitTicket {
                inner: session.applied(),
            })
            .map_err(map_report_error)
    }

    pub fn report_failed(
        &mut self,
        task: RootFrameTask,
        kind: ViewFailureKind,
        message: &'static str,
    ) -> Result<(), IntegrationError> {
        let kind = match kind {
            ViewFailureKind::Build => NavigationFailureKind::Build,
            ViewFailureKind::Apply => NavigationFailureKind::Apply,
        };
        self.inner
            .begin_view_report(task.inner.identity())
            .map(|session| session.failed(NavigationFailure::new(kind, message)))
            .map_err(map_report_error)
    }

    pub fn prepare_commit(
        &mut self,
        ticket: &CommitTicket,
        manifest: CleanupManifest,
    ) -> Result<PreparedCommit, IntegrationError> {
        let manifest_digest = manifest.manifest_digest;
        let obligation_count = manifest.obligation_count;
        let sealed = self
            .inner
            .seal_commit(
                &ticket.inner,
                CleanupManifestSeal::new(
                    manifest.logical_roots,
                    CleanupManifestDigest::from_opaque(manifest_digest),
                    obligation_count,
                ),
            )
            .map_err(map_commit_error)?;
        let cleanup = CleanupBatch {
            inner: sealed.cleanup().clone(),
            manifest_digest,
            obligation_count,
        };
        let publication = self
            .inner
            .prepare_publication(sealed)
            .map_err(map_commit_error)?;
        Ok(PreparedCommit {
            publication,
            cleanup,
        })
    }

    pub fn publish<F>(&mut self, prepared: PreparedCommit, mutation: F)
    where
        F: FnOnce(),
    {
        self.inner.publish_prepared(prepared.publication, mutation)
    }

    pub fn complete_cleanup(
        &mut self,
        completion: CleanupCompletion,
    ) -> Result<(), IntegrationError> {
        self.inner
            .complete_cleanup(completion.inner)
            .map_err(|error| match error {
                crate::engine::CleanupError::Duplicate => IntegrationError::Duplicate,
                crate::engine::CleanupError::Stale => IntegrationError::Stale,
                crate::engine::CleanupError::Invalid => IntegrationError::Invalid,
            })
    }

    pub fn observe_cleanup_receipt(
        &mut self,
        batch: &CleanupBatch,
        disposition: CleanupDisposition,
    ) -> Result<(), IntegrationError> {
        self.inner
            .observe_cleanup_receipt(&batch.inner, disposition)
            .map_err(|_| IntegrationError::Stale)
    }

    pub fn snapshot(&self) -> RouterSnapshotProjection {
        let (router, evicted_through, highest_issued) = self.inner.request_retention_facts();
        RouterSnapshotProjection {
            requested: self.inner.requested().cloned(),
            matched: self.inner.matched().cloned(),
            active: self.inner.active().clone(),
            availability: self.inner.availability().clone(),
            pending_phase: self.inner.pending_phase().cloned(),
            history: self.inner.history_snapshot(),
            running_request: self.inner.running_request(),
            earliest_retained_sequence: self.inner.earliest_retained_sequence(),
            request_retention: RequestRetentionClassifier::project(
                router,
                evicted_through,
                highest_issued,
            ),
            diagnostics: self.inner.diagnostic_snapshot(),
            host: HostSnapshot::project(
                self.inner.host().cloned(),
                self.inner.top_slot(),
                self.host_trace.clone(),
            ),
        }
    }

    pub fn active(&self) -> &ActivePresentation {
        self.inner.active()
    }

    pub fn history(&self) -> &MemoryHistory {
        self.inner.history()
    }

    pub fn router_diagnostics(&self) -> &[DiagnosticEvent] {
        self.inner.router_diagnostics()
    }

    pub fn request(&self, id: NavigationRequestId) -> Option<NavigationRequestView<'_>> {
        self.inner
            .request(id)
            .map(|record| NavigationRequestView { record })
    }

    pub fn request_lookup(&self, id: NavigationRequestId) -> RequestLookup<'_> {
        match self.inner.private_lookup(id) {
            PrivateRequestLookup::Retained(record) => {
                RequestLookup::Found(NavigationRequestView { record })
            }
            PrivateRequestLookup::Evicted => RequestLookup::NotRetained,
            PrivateRequestLookup::Unknown => RequestLookup::Unknown,
        }
    }
}

fn map_install_error(error: CoreEngineInstallError) -> IntegrationError {
    match error {
        CoreEngineInstallError::IdExhausted => IntegrationError::Exhausted,
    }
}

fn map_admission_error(error: CoreAdmissionError) -> IntegrationError {
    match error {
        CoreAdmissionError::OutstandingExists => IntegrationError::OutstandingAdmission,
        CoreAdmissionError::AuthorityExists => IntegrationError::AuthorityExists,
        CoreAdmissionError::ForeignRouter => IntegrationError::ForeignRouter,
        CoreAdmissionError::Stale => IntegrationError::Stale,
        CoreAdmissionError::Exhausted => IntegrationError::Exhausted,
    }
}

fn map_push_error(error: CorePushError) -> IntegrationError {
    match error {
        CorePushError::AwaitingHost => IntegrationError::AwaitingHost,
        CorePushError::IdExhausted => IntegrationError::Exhausted,
    }
}

fn map_report_error(error: CoreReportError) -> IntegrationError {
    match error {
        CoreReportError::Duplicate | CoreReportError::Stale | CoreReportError::Invalid => {
            IntegrationError::Stale
        }
    }
}

fn map_commit_error(error: CoreCommitError) -> IntegrationError {
    match error {
        CoreCommitError::Stale => IntegrationError::Stale,
        CoreCommitError::InvalidManifest => IntegrationError::InvalidManifest,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::{
        ActivePresentationKind, CleanupDisposition, FrameOrigin, HostTraceEventKind,
        NavigationPhase, NavigationTraceEventKind, RequestStatus, RouteLocation, RouteNodeOptions,
        RoutePattern, RouteTree, RouteViewKey, RouterAvailability,
    };

    fn tree(view: &RouteViewKey) -> RouteTree {
        RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/").unwrap(),
                RouteNodeOptions::new().with_view(view.clone()),
            )
            .unwrap()
            .build()
            .unwrap()
    }

    fn ready_engine() -> (RouterEngine, HostGrant) {
        let view = RouteViewKey::new("root").unwrap();
        let fallback = RouteViewKey::new("fallback").unwrap();
        let mut engine = RouterEngine::install(tree(&view), fallback).unwrap();
        let token = engine.request_host_admission().unwrap();
        let grant = engine.grant_host_admission(&token).unwrap();
        (engine, grant)
    }

    fn confirmed_engine() -> RouterEngine {
        let (mut engine, grant) = ready_engine();
        engine.record_top_slot_registered(&grant).unwrap();
        engine.confirm_host(grant);
        engine
    }

    fn settle_push(engine: &mut RouterEngine, location: RouteLocation) -> NavigationRequestId {
        let request = engine.push(location).unwrap();
        engine.collect();
        let ticket = match engine.action().unwrap() {
            EngineAction::FrameTask(task) => engine.report_applied(task).unwrap(),
            EngineAction::Commit(ticket) => ticket,
        };
        let prepared = engine
            .prepare_commit(&ticket, CleanupManifest::empty())
            .unwrap();
        let cleanup = prepared.cleanup().clone();
        engine.publish(prepared, || {});
        engine.complete_cleanup(cleanup.complete_empty()).unwrap();
        request
    }

    #[test]
    fn host_success_trace_is_ordered_and_uses_canonical_ready_state() {
        let (mut engine, grant) = ready_engine();
        let expected_version = grant.version().clone();
        let expected_slot = grant.top_slot();

        engine.record_top_slot_registered(&grant).unwrap();
        let before_replay = engine.snapshot();
        assert_eq!(
            engine.record_top_slot_registered(&grant),
            Err(IntegrationError::Duplicate)
        );
        assert_eq!(engine.snapshot(), before_replay);
        engine.confirm_host(grant);

        let host = engine.snapshot().host().clone();
        assert_eq!(host.version(), Some(&expected_version));
        assert_eq!(host.top_slot(), Some(expected_slot));
        assert_eq!(
            host.trace()
                .iter()
                .map(crate::HostTraceEvent::kind)
                .collect::<Vec<_>>(),
            vec![
                HostTraceEventKind::AdmissionRequested,
                HostTraceEventKind::HostGranted,
                HostTraceEventKind::TopSlotRegistered,
                HostTraceEventKind::HostReady,
            ]
        );
        assert!(host.trace().iter().all(|event| !matches!(
            event.kind(),
            HostTraceEventKind::SettlementStarted | HostTraceEventKind::SettlementCompleted
        )));
    }

    #[test]
    fn host_rejection_trace_records_only_successful_workflow_transitions() {
        let view = RouteViewKey::new("root").unwrap();
        let fallback = RouteViewKey::new("fallback").unwrap();
        let mut engine = RouterEngine::install(tree(&view), fallback).unwrap();

        let token = engine.request_host_admission().unwrap();
        engine.reject_host_admission(token).unwrap();
        let token = engine.request_host_admission().unwrap();
        let grant = engine.grant_host_admission(&token).unwrap();
        engine.reject_host_after_mutation(grant).unwrap();

        let host = engine.snapshot().host().clone();
        assert!(host.version().is_none());
        assert!(host.top_slot().is_none());
        assert_eq!(
            host.trace()
                .iter()
                .map(crate::HostTraceEvent::kind)
                .collect::<Vec<_>>(),
            vec![
                HostTraceEventKind::AdmissionRequested,
                HostTraceEventKind::AdmissionRejected,
                HostTraceEventKind::AdmissionRequested,
                HostTraceEventKind::HostGranted,
                HostTraceEventKind::AdmissionRejectedAfterMutation,
            ]
        );
    }

    #[test]
    fn request_lookup_bridge_preserves_found_not_retained_and_unknown() {
        let mut engine = confirmed_engine();
        let requests = (0..66)
            .map(|_| settle_push(&mut engine, RouteLocation::new("/")))
            .collect::<Vec<_>>();

        assert!(matches!(
            engine.request_lookup(requests[0]),
            RequestLookup::NotRetained
        ));
        assert!(matches!(
            engine.request_lookup(requests[2]),
            RequestLookup::Found(_)
        ));

        let mut foreign = confirmed_engine();
        let unknown = foreign.push(RouteLocation::new("/")).unwrap();
        assert!(matches!(
            engine.request_lookup(unknown),
            RequestLookup::Unknown
        ));
    }

    #[test]
    fn frozen_request_retention_classifier_rejects_foreign_and_future_ids() {
        let mut engine = confirmed_engine();
        let requests = (0..66)
            .map(|_| settle_push(&mut engine, RouteLocation::new("/")))
            .collect::<Vec<_>>();
        let snapshot = engine.snapshot();

        assert_eq!(
            snapshot.request_retention().classify(requests[0]),
            MissingRequestClassification::NotRetained
        );

        let mut foreign = confirmed_engine();
        let foreign_request = foreign.push(RouteLocation::new("/")).unwrap();
        assert_eq!(
            snapshot.request_retention().classify(foreign_request),
            MissingRequestClassification::Unknown
        );

        let future_request = engine.push(RouteLocation::new("/")).unwrap();
        assert_eq!(
            snapshot.request_retention().classify(future_request),
            MissingRequestClassification::Unknown
        );
    }

    #[test]
    fn router_projection_uses_only_current_request_then_clears_at_terminal() {
        let mut engine = confirmed_engine();
        let request = engine.push(RouteLocation::new("/")).unwrap();
        engine.collect();

        let running = engine.snapshot();
        assert_eq!(running.requested(), Some(&RouteLocation::new("/")));
        assert!(running.matched().is_some());
        assert_eq!(running.active().kind(), ActivePresentationKind::None);
        assert_eq!(running.availability(), &RouterAvailability::Ready);
        assert_eq!(
            running.pending_phase(),
            Some(&NavigationPhase::ApplyingFrame(0))
        );
        assert_eq!(running.running_request(), Some(request));
        assert_eq!(running.history().len(), 0);
        assert_eq!(running.earliest_retained_sequence(), None);
        assert!(running.diagnostics().events().is_empty());
        assert!(running.host().version().is_some());

        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("first push must issue a frame task")
        };
        let ticket = engine.report_applied(task).unwrap();
        let prepared = engine
            .prepare_commit(&ticket, CleanupManifest::empty())
            .unwrap();
        let cleanup = prepared.cleanup().clone();
        engine.publish(prepared, || {});
        engine.complete_cleanup(cleanup.complete_empty()).unwrap();

        let terminal = engine.snapshot();
        assert!(terminal.requested().is_none());
        assert!(terminal.matched().is_none());
        assert!(terminal.pending_phase().is_none());
        assert!(terminal.running_request().is_none());
        assert_eq!(terminal.active().kind(), ActivePresentationKind::Route);
        assert_eq!(terminal.history().current(), Some(&RouteLocation::new("/")));
        assert_eq!(
            terminal.earliest_retained_sequence(),
            Some(request.sequence())
        );
    }

    #[test]
    fn cleanup_receipt_observation_precedes_aggregate_and_stale_is_zero_write() {
        let mut engine = confirmed_engine();
        settle_push(&mut engine, RouteLocation::new("/"));
        let leaving = engine.active().frames()[0].clone();

        let request = engine
            .push(RouteLocation::new("/").with_fragment("details"))
            .unwrap();
        engine.collect();
        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("fragment push must issue a frame task")
        };
        let ticket = engine.report_applied(task).unwrap();
        let prepared = engine
            .prepare_commit(&ticket, CleanupManifest::new(vec![leaving], 41, 1))
            .unwrap();
        let cleanup = prepared.cleanup().clone();
        let stale = cleanup.clone();
        engine.publish(prepared, || {});

        engine
            .observe_cleanup_receipt(&cleanup, CleanupDisposition::Removed)
            .unwrap();
        let receipt_trace = engine.request(request).unwrap().trace().to_vec();
        assert_eq!(
            receipt_trace.last().map(NavigationTraceEvent::kind),
            Some(NavigationTraceEventKind::CleanupSettled)
        );
        assert_eq!(
            receipt_trace.last().unwrap().cleanup_disposition(),
            Some(CleanupDisposition::Removed)
        );
        assert_eq!(
            engine.request(request).unwrap().status(),
            &RequestStatus::Running(NavigationPhase::PostCommitCleanup)
        );

        engine.complete_cleanup(cleanup.complete(43, 1)).unwrap();
        let trace = engine.request(request).unwrap().trace().to_vec();
        let cleanup_events = trace
            .iter()
            .filter(|event| event.kind() == NavigationTraceEventKind::CleanupSettled)
            .map(NavigationTraceEvent::cleanup_disposition)
            .collect::<Vec<_>>();
        assert_eq!(
            cleanup_events,
            vec![Some(CleanupDisposition::Removed), None]
        );
        assert_eq!(
            trace.last().map(NavigationTraceEvent::kind),
            Some(NavigationTraceEventKind::RequestTerminal)
        );

        let before_snapshot = engine.snapshot();
        let before_trace = trace;
        assert_eq!(
            engine.observe_cleanup_receipt(&stale, CleanupDisposition::AlreadyAbsent),
            Err(IntegrationError::Stale)
        );
        assert_eq!(engine.snapshot(), before_snapshot);
        assert_eq!(engine.request(request).unwrap().trace(), before_trace);
    }

    #[test]
    fn cleanup_summary_projection_accepts_only_read_only_ledger_facts() {
        let summary = project_cleanup_summary(
            true,
            3,
            Some(CleanupDisposition::QuarantinedForeignDescendant),
        );

        assert!(summary.foreground_pending());
        assert_eq!(summary.quarantine_count(), 3);
        assert_eq!(
            summary.last_disposition(),
            Some(CleanupDisposition::QuarantinedForeignDescendant)
        );
    }

    #[test]
    fn sealed_integration_projects_host_and_root_task_without_raw_authority_mutation() {
        let (mut engine, grant) = ready_engine();
        let expected_host = grant.version().clone();
        let expected_slot = grant.top_slot();

        engine.confirm_host(grant);
        let request = engine.push(RouteLocation::new("/")).unwrap();
        engine.collect();
        let EngineAction::FrameTask(mut task) = engine.action().unwrap() else {
            panic!("first root push should produce one frame task")
        };

        assert_eq!(task.request(), request);
        assert_eq!(task.frame().host(), expected_host.host());
        assert_eq!(task.frame().slot(), expected_slot);
        assert_eq!(task.view(), &RouteViewKey::new("root").unwrap());
        assert_eq!(task.take_input().location(), &RouteLocation::new("/"));
        assert_eq!(
            engine.request(request).unwrap().status(),
            &RequestStatus::Running(NavigationPhase::ApplyingFrame(0))
        );
    }

    #[test]
    fn prepared_publication_runs_the_ecs_mutation_before_infallible_core_publish() {
        let (mut engine, grant) = ready_engine();
        engine.confirm_host(grant);
        let request = engine.push(RouteLocation::new("/")).unwrap();
        engine.collect();
        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("first root push should produce one frame task")
        };
        let ticket = engine.report_applied(task).unwrap();
        let prepared = engine
            .prepare_commit(&ticket, CleanupManifest::empty())
            .unwrap();
        let cleanup = prepared.cleanup().clone();
        let replay = cleanup.clone().complete_empty();
        let malformed = cleanup.clone().complete(7, 0);
        let mut mutation_ran = false;

        engine.publish(prepared, || mutation_ran = true);

        assert!(mutation_ran);
        assert_eq!(engine.active().location(), Some(&RouteLocation::new("/")));
        assert_eq!(engine.history().len(), 1);
        assert_eq!(engine.history().current(), Some(&RouteLocation::new("/")));
        assert_eq!(
            engine.request(request).unwrap().status(),
            &RequestStatus::Running(NavigationPhase::PostCommitCleanup)
        );

        engine.complete_cleanup(cleanup.complete_empty()).unwrap();
        assert_eq!(
            engine.complete_cleanup(replay),
            Err(IntegrationError::Duplicate)
        );
        assert_eq!(
            engine.complete_cleanup(malformed),
            Err(IntegrationError::Invalid)
        );
        assert_eq!(
            engine.request(request).unwrap().status(),
            &RequestStatus::Succeeded
        );
    }

    #[test]
    fn sealed_install_binding_is_used_for_engine_issued_router_fallback() {
        let (mut engine, grant) = ready_engine();
        engine.confirm_host(grant);
        let request = engine.push(RouteLocation::new("/")).unwrap();
        engine.collect();
        let EngineAction::FrameTask(normal) = engine.action().unwrap() else {
            panic!("normal root task expected")
        };

        engine
            .report_failed(normal, ViewFailureKind::Build, "normal build failed")
            .unwrap();

        let EngineAction::FrameTask(fallback) = engine.action().unwrap() else {
            panic!("router fallback task expected")
        };
        assert_eq!(fallback.request(), request);
        assert_eq!(fallback.view(), &RouteViewKey::new("fallback").unwrap());
        assert_eq!(fallback.frame().origin(), &FrameOrigin::RouterFallback);
        assert_eq!(
            fallback.input().failure().map(NavigationFailure::kind),
            Some(NavigationFailureKind::Build)
        );
    }

    #[test]
    fn rejected_manifest_does_not_consume_the_single_commit_ticket() {
        let (mut engine, grant) = ready_engine();
        engine.confirm_host(grant);
        engine.push(RouteLocation::new("/")).unwrap();
        engine.collect();
        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("first root push should produce one frame task")
        };
        let ticket = engine.report_applied(task).unwrap();

        let rejected = engine.prepare_commit(&ticket, CleanupManifest::new(Vec::new(), 17, 1));
        assert!(matches!(rejected, Err(IntegrationError::InvalidManifest)));

        let prepared = engine
            .prepare_commit(&ticket, CleanupManifest::empty())
            .unwrap();
        engine.publish(prepared, || {});
        assert_eq!(engine.history().len(), 1);
    }

    #[test]
    fn queued_push_has_no_lifecycle_action_until_collect() {
        let (mut engine, grant) = ready_engine();
        engine.confirm_host(grant);

        engine.push(RouteLocation::new("/")).unwrap();
        assert!(engine.action().is_none());

        engine.collect();
        assert!(matches!(engine.action(), Some(EngineAction::FrameTask(_))));
    }
}
