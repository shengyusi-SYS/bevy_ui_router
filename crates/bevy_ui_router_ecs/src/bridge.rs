use std::sync::{Arc, Mutex};

use bevy_ui_router_core::frame::RouteFrameRecord;
use bevy_ui_router_core::transaction::FrameApplyRequest;
use bevy_ui_router_view::{
    RouteViewDescriptor, ViewApplyOutcome, ViewBuildOutcome, ViewBuildRequest, ViewFailure,
    ViewPartialOutput, ViewReportSession, ViewReportedOutput, ViewRequestId,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BeginFrameApplySessionError {
    RequestAlreadyPending,
    RequestNotPending,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FinishFrameApplySessionError {
    SessionNotRegistered,
    SessionNotTerminal,
}

pub struct EcsFrameApplySession {
    request: FrameApplyRequest,
    report_session: ViewReportSession,
    state: EcsFrameApplySessionStateHandle,
}

impl core::fmt::Debug for EcsFrameApplySession {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("EcsFrameApplySession")
            .field("request", &self.request)
            .field("report_session", &self.report_session)
            .field("state", &self.state.snapshot())
            .finish()
    }
}

impl PartialEq for EcsFrameApplySession {
    fn eq(&self, other: &Self) -> bool {
        self.request == other.request
            && self.report_session == other.report_session
            && self.state == other.state
    }
}

impl Eq for EcsFrameApplySession {}

#[derive(Clone, Debug)]
pub(crate) struct EcsFrameApplySessionStateHandle {
    inner: Arc<Mutex<EcsFrameApplySessionState>>,
}

impl PartialEq for EcsFrameApplySessionStateHandle {
    fn eq(&self, other: &Self) -> bool {
        self.snapshot() == other.snapshot()
    }
}

impl Eq for EcsFrameApplySessionStateHandle {}

impl EcsFrameApplySessionStateHandle {
    fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(EcsFrameApplySessionState::default())),
        }
    }

    fn snapshot(&self) -> EcsFrameApplySessionState {
        self.inner
            .lock()
            .expect("ecs frame apply session state lock should not be poisoned")
            .clone()
    }

    fn set_report(&self, report: EcsFrameApplyReport) {
        self.inner
            .lock()
            .expect("ecs frame apply session state lock should not be poisoned")
            .report = Some(report);
    }

    fn update_report_status(&self, status: EcsFrameApplyStatus) {
        if let Some(report) = self
            .inner
            .lock()
            .expect("ecs frame apply session state lock should not be poisoned")
            .report
            .as_mut()
        {
            report.status = status;
        }
    }

    pub(crate) fn report(&self) -> Option<EcsFrameApplyReport> {
        self.snapshot().report
    }

    pub(crate) fn finalized(&self) -> bool {
        self.snapshot().finalized
    }

    pub(crate) fn status(&self) -> Option<EcsFrameApplyStatus> {
        self.report().map(|report| report.status())
    }

    pub(crate) fn can_finish(&self) -> bool {
        self.finalized()
            || matches!(
                self.status(),
                Some(
                    EcsFrameApplyStatus::Finalized
                        | EcsFrameApplyStatus::Failed
                        | EcsFrameApplyStatus::StaleDiscarded
                )
            )
    }

    pub(crate) fn mark_reported_to_core(&self) {
        self.update_report_status(EcsFrameApplyStatus::ReportedToCore);
    }

    pub(crate) fn mark_finalized(&self) {
        let mut state = self
            .inner
            .lock()
            .expect("ecs frame apply session state lock should not be poisoned");
        state.finalized = true;
        if let Some(report) = state.report.as_mut() {
            report.status = EcsFrameApplyStatus::Finalized;
        }
    }

    pub(crate) fn mark_terminal_cleanup_complete(&self) {
        self.inner
            .lock()
            .expect("ecs frame apply session state lock should not be poisoned")
            .finalized = true;
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct EcsFrameApplySessionState {
    finalized: bool,
    report: Option<EcsFrameApplyReport>,
}

impl EcsFrameApplySession {
    pub(crate) fn new(request: &FrameApplyRequest, descriptor: RouteViewDescriptor) -> Self {
        Self {
            request: request.clone(),
            report_session: ViewReportSession::from_frame_request(request, descriptor),
            state: EcsFrameApplySessionStateHandle::new(),
        }
    }

    pub fn request(&self) -> &FrameApplyRequest {
        &self.request
    }

    pub fn frame(&self) -> &RouteFrameRecord {
        self.request.frame()
    }

    pub fn build_request(&self) -> ViewBuildRequest {
        self.report_session.build_request()
    }

    pub fn report(&self) -> Option<EcsFrameApplyReport> {
        self.state.report()
    }

    pub fn finalized(&self) -> bool {
        self.state.finalized()
    }

    pub fn status(&self) -> Option<EcsFrameApplyStatus> {
        self.state.status()
    }

    pub fn accepts_report_session(&self, report_session: &ViewReportSession) -> bool {
        report_session.frame_request() == &self.request
            && report_session.build_request().request_id()
                == self.report_session.build_request().request_id()
    }

    pub fn record_build_failure(
        &mut self,
        redacted_message: impl Into<String>,
    ) -> EcsFrameApplyReport {
        let outcome = self.report_session.build_failed(redacted_message);
        self.record_build_outcome(outcome)
    }

    pub fn record_apply_success(
        &mut self,
        disposal_hint: Option<impl Into<String>>,
    ) -> EcsFrameApplyReport {
        let output = self.report_session.applied(disposal_hint);
        self.record_reported_output(output)
    }

    pub fn record_apply_failure(
        &mut self,
        redacted_message: impl Into<String>,
        disposal_hint: Option<impl Into<String>>,
    ) -> EcsFrameApplyReport {
        let outcome = self
            .report_session
            .apply_failed(redacted_message, disposal_hint);
        self.record_apply_outcome(outcome)
    }

    pub fn discard_stale(&mut self, redacted_message: impl Into<String>) -> EcsFrameApplyReport {
        let outcome = self.report_session.stale(redacted_message);
        self.record_apply_outcome(outcome)
    }

    fn record_build_outcome(&mut self, outcome: ViewBuildOutcome) -> EcsFrameApplyReport {
        let report = match outcome {
            ViewBuildOutcome::Built(_) => EcsFrameApplyReport::pending(),
            ViewBuildOutcome::Failed(failure) => EcsFrameApplyReport::failed(failure),
            ViewBuildOutcome::Stale(failure) => EcsFrameApplyReport::stale_discarded(failure),
        };
        self.state.set_report(report.clone());
        report
    }

    fn record_apply_outcome(&mut self, outcome: ViewApplyOutcome) -> EcsFrameApplyReport {
        let report = match outcome {
            ViewApplyOutcome::Applied(output) => EcsFrameApplyReport::applied(output),
            ViewApplyOutcome::Failed(failure) => EcsFrameApplyReport::failed(failure),
            ViewApplyOutcome::Stale(failure) => EcsFrameApplyReport::stale_discarded(failure),
        };
        self.state.set_report(report.clone());
        report
    }

    fn record_reported_output(&mut self, output: ViewReportedOutput) -> EcsFrameApplyReport {
        self.record_apply_outcome(output.outcome().clone())
    }

    pub(crate) fn view_request_id(&self) -> ViewRequestId {
        self.report_session.build_request().request_id()
    }

    pub(crate) fn state_handle(&self) -> EcsFrameApplySessionStateHandle {
        self.state.clone()
    }

    pub(crate) fn can_finish(&self) -> bool {
        self.state.can_finish()
    }

    pub(crate) fn mark_reported_to_core(&self) {
        self.state.mark_reported_to_core();
    }

    pub(crate) fn mark_terminal_cleanup_complete(&self) {
        self.state.mark_terminal_cleanup_complete();
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EcsFrameApplyStatus {
    Pending,
    Finalized,
    ReportedToCore,
    StaleDiscarded,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EcsFrameApplyReport {
    status: EcsFrameApplyStatus,
    failure: Option<ViewFailure>,
    partial_output: Option<ViewPartialOutput>,
}

impl EcsFrameApplyReport {
    const fn pending() -> Self {
        Self {
            status: EcsFrameApplyStatus::Pending,
            failure: None,
            partial_output: None,
        }
    }

    fn applied(output: ViewPartialOutput) -> Self {
        Self {
            status: EcsFrameApplyStatus::Pending,
            failure: None,
            partial_output: Some(output),
        }
    }

    fn failed(failure: ViewFailure) -> Self {
        Self {
            status: EcsFrameApplyStatus::Failed,
            partial_output: failure.partial_output().cloned(),
            failure: Some(failure),
        }
    }

    fn stale_discarded(failure: ViewFailure) -> Self {
        Self {
            status: EcsFrameApplyStatus::StaleDiscarded,
            partial_output: None,
            failure: Some(failure),
        }
    }

    pub const fn status(&self) -> EcsFrameApplyStatus {
        self.status
    }

    pub fn failure(&self) -> Option<&ViewFailure> {
        self.failure.as_ref()
    }

    pub fn partial_output(&self) -> Option<&ViewPartialOutput> {
        self.partial_output.as_ref()
    }
}
