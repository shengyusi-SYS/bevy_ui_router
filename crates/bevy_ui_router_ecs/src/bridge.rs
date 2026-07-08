use bevy_ui_router_core::frame::RouteFrameRecord;
use bevy_ui_router_core::transaction::FrameApplyRequest;
use bevy_ui_router_view::{
    RouteViewDescriptor, ViewApplyOutcome, ViewBuildOutcome, ViewBuildRequest, ViewFailure,
    ViewPartialOutput, ViewReportSession, ViewReportedOutput,
};

#[derive(Debug, Eq, PartialEq)]
pub struct EcsFrameApplySession {
    request: FrameApplyRequest,
    report_session: ViewReportSession,
    finalized: bool,
    report: Option<EcsFrameApplyReport>,
}

impl EcsFrameApplySession {
    pub(crate) fn new(request: &FrameApplyRequest, descriptor: RouteViewDescriptor) -> Self {
        Self {
            request: request.clone(),
            report_session: ViewReportSession::from_frame_request(request, descriptor),
            finalized: false,
            report: None,
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

    pub fn report(&self) -> Option<&EcsFrameApplyReport> {
        self.report.as_ref()
    }

    pub const fn finalized(&self) -> bool {
        self.finalized
    }

    pub fn accepts_report_session(&self, report_session: &ViewReportSession) -> bool {
        report_session.frame_request() == &self.request
            && report_session.build_request().request_id()
                == self.report_session.build_request().request_id()
    }

    pub fn record_build_failure(
        &mut self,
        redacted_message: impl Into<String>,
    ) -> &EcsFrameApplyReport {
        let outcome = self.report_session.build_failed(redacted_message);
        self.record_build_outcome(outcome)
    }

    pub fn record_apply_success(
        &mut self,
        disposal_hint: Option<impl Into<String>>,
    ) -> &EcsFrameApplyReport {
        let output = self.report_session.applied(disposal_hint);
        self.record_reported_output(output)
    }

    pub fn record_apply_failure(
        &mut self,
        redacted_message: impl Into<String>,
        disposal_hint: Option<impl Into<String>>,
    ) -> &EcsFrameApplyReport {
        let outcome = self
            .report_session
            .apply_failed(redacted_message, disposal_hint);
        self.record_apply_outcome(outcome)
    }

    pub fn discard_stale(&mut self, redacted_message: impl Into<String>) -> &EcsFrameApplyReport {
        let outcome = self.report_session.stale(redacted_message);
        self.record_apply_outcome(outcome)
    }

    fn record_build_outcome(&mut self, outcome: ViewBuildOutcome) -> &EcsFrameApplyReport {
        let report = match outcome {
            ViewBuildOutcome::Built(_) => EcsFrameApplyReport::pending(),
            ViewBuildOutcome::Failed(failure) => EcsFrameApplyReport::failed(failure),
            ViewBuildOutcome::Stale(failure) => EcsFrameApplyReport::stale_discarded(failure),
        };
        self.report = Some(report);
        self.report
            .as_ref()
            .expect("local apply report was just recorded")
    }

    fn record_apply_outcome(&mut self, outcome: ViewApplyOutcome) -> &EcsFrameApplyReport {
        let report = match outcome {
            ViewApplyOutcome::Applied(output) => EcsFrameApplyReport::applied(output),
            ViewApplyOutcome::Failed(failure) => EcsFrameApplyReport::failed(failure),
            ViewApplyOutcome::Stale(failure) => EcsFrameApplyReport::stale_discarded(failure),
        };
        self.report = Some(report);
        self.report
            .as_ref()
            .expect("local apply report was just recorded")
    }

    fn record_reported_output(&mut self, output: ViewReportedOutput) -> &EcsFrameApplyReport {
        self.record_apply_outcome(output.outcome().clone())
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
