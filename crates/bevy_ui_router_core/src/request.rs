#![allow(dead_code)]

use crate::diagnostics::DiagnosticEvent;
use crate::fingerprint::RouteBranchFingerprint;
use crate::ids::NavigationRequestId;
use crate::location::RouteLocation;
use crate::trace::NavigationTraceEvent;

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
    pub(crate) fn new(kind: NavigationFailureKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub const fn kind(&self) -> NavigationFailureKind {
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
    requested: RouteLocation,
    matched: Option<RouteBranchFingerprint>,
    status: RequestStatus,
    trace: Vec<NavigationTraceEvent>,
    diagnostics: Vec<DiagnosticEvent>,
}

impl NavigationRequestRecord {
    pub(crate) fn new(id: NavigationRequestId, requested: RouteLocation) -> Self {
        Self {
            id,
            requested,
            matched: None,
            status: RequestStatus::Queued,
            trace: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    pub(crate) const fn id(&self) -> NavigationRequestId {
        self.id
    }

    pub(crate) const fn requested(&self) -> &RouteLocation {
        &self.requested
    }

    pub(crate) const fn matched(&self) -> Option<&RouteBranchFingerprint> {
        self.matched.as_ref()
    }

    pub(crate) const fn status(&self) -> &RequestStatus {
        &self.status
    }

    pub(crate) fn trace(&self) -> &[NavigationTraceEvent] {
        &self.trace
    }

    pub(crate) fn diagnostics(&self) -> &[DiagnosticEvent] {
        &self.diagnostics
    }

    pub(crate) fn set_matched(&mut self, matched: RouteBranchFingerprint) {
        self.matched = Some(matched);
    }

    pub(crate) fn set_status(&mut self, status: RequestStatus) {
        self.status = status;
    }

    pub(crate) fn push_trace(&mut self, event: NavigationTraceEvent) {
        self.trace.push(event);
    }

    pub(crate) fn push_diagnostic(&mut self, event: DiagnosticEvent) {
        self.diagnostics.push(event);
    }
}
