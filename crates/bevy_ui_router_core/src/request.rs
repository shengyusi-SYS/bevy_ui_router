use crate::diagnostics::DiagnosticEvent;
use crate::fingerprint::RouteBranchFingerprint;
use crate::ids::NavigationRequestId;
use crate::location::RouteLocation;
use crate::trace::{
    NavigationTraceError, NavigationTraceEvent, NavigationTraceEventKind, NavigationTraceLog,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NavigationFailureKind {
    Match,
    Build,
    Apply,
    Mount,
    HostLost,
    AuthorityLost,
    RecoveryFailed,
    BackTargetUnavailable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NavigationFailure {
    kind: NavigationFailureKind,
    message: String,
}

impl NavigationFailure {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn new(kind: NavigationFailureKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub fn kind(&self) -> NavigationFailureKind {
        self.kind
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NavigationPhase {
    Matching,
    Planning,
    ResolvingMount,
    PreparingFrame(usize),
    ApplyingFrame(usize),
    RegisteringOutlets(usize),
    ReadyToSwitch,
    Recovering,
    CleanupPending,
    PostCommitCleanup,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RequestStatus {
    Queued,
    Running(NavigationPhase),
    Succeeded,
    Failed(NavigationFailure),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NavigationRequestRecord {
    id: NavigationRequestId,
    requested: Option<RouteLocation>,
    matched: Option<RouteBranchFingerprint>,
    status: RequestStatus,
    trace: NavigationTraceLog,
    diagnostics: Vec<DiagnosticEvent>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RequestedTargetAlreadyResolved;

impl NavigationRequestRecord {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn with_resolved_target(id: NavigationRequestId, requested: RouteLocation) -> Self {
        Self {
            id,
            requested: Some(requested),
            matched: None,
            status: RequestStatus::Queued,
            trace: NavigationTraceLog::new(),
            diagnostics: Vec::new(),
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn unresolved_back(id: NavigationRequestId) -> Self {
        Self {
            id,
            requested: None,
            matched: None,
            status: RequestStatus::Queued,
            trace: NavigationTraceLog::new(),
            diagnostics: Vec::new(),
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn resolve_requested_once(
        &mut self,
        requested: RouteLocation,
    ) -> Result<(), RequestedTargetAlreadyResolved> {
        if self.requested.is_some() {
            return Err(RequestedTargetAlreadyResolved);
        }
        self.requested = Some(requested);
        Ok(())
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) const fn id(&self) -> NavigationRequestId {
        self.id
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) const fn requested(&self) -> Option<&RouteLocation> {
        self.requested.as_ref()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) const fn matched(&self) -> Option<&RouteBranchFingerprint> {
        self.matched.as_ref()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) const fn status(&self) -> &RequestStatus {
        &self.status
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn trace(&self) -> &[NavigationTraceEvent] {
        self.trace.events()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn diagnostics(&self) -> &[DiagnosticEvent] {
        &self.diagnostics
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn set_matched(
        &mut self,
        matched: RouteBranchFingerprint,
    ) -> Result<(), RequestRecordError> {
        if self.matched.is_some() || self.status.is_terminal() {
            return Err(RequestRecordError::ImmutableMatch);
        }
        self.matched = Some(matched);
        Ok(())
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn transition(&mut self, status: RequestStatus) -> Result<(), RequestRecordError> {
        if self.status.is_terminal() {
            return Err(RequestRecordError::AlreadyTerminal);
        }
        self.status = status;
        Ok(())
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn append_trace(
        &mut self,
        event: NavigationTraceEvent,
    ) -> Result<(), RequestRecordError> {
        if self.trace.is_closed() {
            return Err(RequestRecordError::TraceClosed);
        }
        let terminal_event = event.kind() == NavigationTraceEventKind::RequestTerminal;
        if terminal_event && !self.status.is_terminal() {
            return Err(RequestRecordError::TerminalStatusRequired);
        }
        if !terminal_event && self.status.is_terminal() {
            return Err(RequestRecordError::TerminalEventRequired);
        }
        self.trace.append(event).map_err(RequestRecordError::from)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn push_diagnostic(
        &mut self,
        event: DiagnosticEvent,
    ) -> Result<(), RequestRecordError> {
        self.diagnostics.push(event);
        Ok(())
    }
}

impl RequestStatus {
    fn is_terminal(&self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed(_))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RequestRecordError {
    AlreadyTerminal,
    ImmutableMatch,
    TerminalStatusRequired,
    TerminalEventRequired,
    TraceClosed,
}

impl From<NavigationTraceError> for RequestRecordError {
    fn from(error: NavigationTraceError) -> Self {
        match error {
            NavigationTraceError::Closed => Self::TraceClosed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fingerprint::RouteSearchFingerprint;
    use crate::ids::CoreIdAllocator;
    use crate::trace::NavigationTraceEventKind;

    fn record() -> NavigationRequestRecord {
        NavigationRequestRecord::with_resolved_target(request_id(), RouteLocation::new("/items/7"))
    }

    fn request_id() -> NavigationRequestId {
        let mut ids = CoreIdAllocator::new().unwrap();
        let router = ids.allocate_router_identity().unwrap();
        ids.allocate_request(router).unwrap()
    }

    #[test]
    fn queued_back_projects_no_requested_target() {
        let queued_back = NavigationRequestRecord::unresolved_back(request_id());

        assert_eq!(queued_back.status(), &RequestStatus::Queued);
        assert_eq!(queued_back.requested(), None);
    }

    #[test]
    fn running_back_projects_resolved_requested_target() {
        let target = RouteLocation::new("/items/1");
        let mut running_back = NavigationRequestRecord::unresolved_back(request_id());
        running_back.resolve_requested_once(target.clone()).unwrap();
        assert_eq!(
            running_back.resolve_requested_once(RouteLocation::new("/items/2")),
            Err(RequestedTargetAlreadyResolved)
        );
        running_back
            .transition(RequestStatus::Running(NavigationPhase::Matching))
            .unwrap();

        assert_eq!(running_back.requested(), Some(&target));

        running_back
            .transition(RequestStatus::Failed(NavigationFailure::new(
                NavigationFailureKind::Build,
                "resolved back target failed",
            )))
            .unwrap();
        assert_eq!(running_back.requested(), Some(&target));
    }

    #[test]
    fn terminal_no_target_back_projects_no_requested_target() {
        let mut terminal_no_target = NavigationRequestRecord::unresolved_back(request_id());
        terminal_no_target
            .transition(RequestStatus::Failed(NavigationFailure::new(
                NavigationFailureKind::BackTargetUnavailable,
                "no previous history entry",
            )))
            .unwrap();

        assert_eq!(terminal_no_target.requested(), None);
    }

    #[test]
    fn terminal_status_cannot_return_to_running() {
        let mut record = record();
        record.transition(RequestStatus::Succeeded).unwrap();
        let before = record.clone();

        assert_eq!(
            record.transition(RequestStatus::Running(NavigationPhase::Recovering)),
            Err(RequestRecordError::AlreadyTerminal)
        );
        assert_eq!(record, before);
    }

    #[test]
    fn request_record_projects_identity_match_status_and_diagnostics() {
        let mut record = record();
        let id = record.id;
        let requested = record.requested().cloned().unwrap();

        assert_eq!(record.id(), id);
        assert_eq!(record.requested(), Some(&requested));
        assert!(record.matched().is_none());
        assert_eq!(record.status(), &RequestStatus::Queued);
        assert!(record.diagnostics().is_empty());

        let search = RouteSearchFingerprint::from_location(&requested);
        let matched = RouteBranchFingerprint::from_parts(std::iter::empty(), &search);
        record.set_matched(matched.clone()).unwrap();
        assert_eq!(record.matched(), Some(&matched));

        let matched_state = record.clone();
        assert_eq!(
            record.set_matched(matched),
            Err(RequestRecordError::ImmutableMatch)
        );
        assert_eq!(record, matched_state);

        let diagnostic = DiagnosticEvent::match_failed("no route");
        record.push_diagnostic(diagnostic.clone()).unwrap();
        assert_eq!(record.diagnostics(), &[diagnostic]);
    }

    #[test]
    fn request_terminal_requires_terminal_status_and_is_always_last() {
        let mut record = record();
        let before = record.clone();
        assert_eq!(
            record.append_trace(NavigationTraceEvent::request_terminal()),
            Err(RequestRecordError::TerminalStatusRequired)
        );
        assert_eq!(record, before);

        record.transition(RequestStatus::Succeeded).unwrap();
        assert_eq!(
            record.append_trace(NavigationTraceEvent::commit_published()),
            Err(RequestRecordError::TerminalEventRequired)
        );
        record
            .append_trace(NavigationTraceEvent::request_terminal())
            .unwrap();
        let terminal = record.clone();

        assert_eq!(
            record.append_trace(NavigationTraceEvent::commit_published()),
            Err(RequestRecordError::TraceClosed)
        );
        assert_eq!(
            record.append_trace(NavigationTraceEvent::request_terminal()),
            Err(RequestRecordError::TraceClosed)
        );
        assert_eq!(record, terminal);
        assert_eq!(
            record.trace().last().map(NavigationTraceEvent::kind),
            Some(NavigationTraceEventKind::RequestTerminal)
        );
    }
}
