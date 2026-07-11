#![allow(dead_code)]

use crate::frame::RouteFrameVersion;
use crate::ids::RouteSlotId;
use crate::request::NavigationPhase;
use crate::slot::RouteSlotKey;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MountRejectionKind {
    MissingOutlet,
    InvalidOutlet,
    ParentAuthorityLost,
    RejectedAfterMutation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CleanupDisposition {
    Removed,
    RemovedDetached,
    AlreadyAbsent,
    RejectedProtected,
    QuarantinedForeignDescendant,
    QuarantinedProtectedParent,
    QuarantinedForeignComponent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NavigationTraceEventKind {
    FrameTaskIssued,
    RequestPhaseChanged,
    HistoryIntentRejected,
    MountRequested,
    MountResolved,
    MountGranted,
    MountRejected,
    MountRejectedAfterMutation,
    ShellPrepared,
    ViewApplied,
    OutletRegistered,
    FrameIsolated,
    FrameActivated,
    CommitPublished,
    CleanupSettled,
    RequestTerminal,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NavigationTraceEvent {
    kind: NavigationTraceEventKind,
    frame: Option<RouteFrameVersion>,
    outlet: Option<RouteSlotKey>,
    slot: Option<RouteSlotId>,
    phase: Option<NavigationPhase>,
    mount_rejection: Option<MountRejectionKind>,
    cleanup_disposition: Option<CleanupDisposition>,
}

impl NavigationTraceEvent {
    pub(crate) fn request_phase_changed(phase: NavigationPhase) -> Self {
        Self::new(NavigationTraceEventKind::RequestPhaseChanged).with_phase(phase)
    }

    pub(crate) fn commit_published() -> Self {
        Self::new(NavigationTraceEventKind::CommitPublished)
    }

    pub(crate) fn request_terminal() -> Self {
        Self::new(NavigationTraceEventKind::RequestTerminal)
    }

    pub(crate) const fn new(kind: NavigationTraceEventKind) -> Self {
        Self {
            kind,
            frame: None,
            outlet: None,
            slot: None,
            phase: None,
            mount_rejection: None,
            cleanup_disposition: None,
        }
    }

    pub(crate) fn with_phase(mut self, phase: NavigationPhase) -> Self {
        self.phase = Some(phase);
        self
    }

    pub const fn kind(&self) -> NavigationTraceEventKind {
        self.kind
    }

    pub const fn frame(&self) -> Option<&RouteFrameVersion> {
        self.frame.as_ref()
    }

    pub const fn outlet(&self) -> Option<&RouteSlotKey> {
        self.outlet.as_ref()
    }

    pub const fn slot(&self) -> Option<RouteSlotId> {
        self.slot
    }

    pub const fn phase(&self) -> Option<&NavigationPhase> {
        self.phase.as_ref()
    }

    pub const fn mount_rejection(&self) -> Option<MountRejectionKind> {
        self.mount_rejection
    }

    pub const fn cleanup_disposition(&self) -> Option<CleanupDisposition> {
        self.cleanup_disposition
    }
}

#[derive(Debug, Default)]
pub(crate) struct NavigationTraceLog {
    events: Vec<NavigationTraceEvent>,
}

impl NavigationTraceLog {
    pub(crate) const fn new() -> Self {
        Self { events: Vec::new() }
    }

    pub(crate) fn append(&mut self, event: NavigationTraceEvent) {
        self.events.push(event);
    }

    pub(crate) fn events(&self) -> &[NavigationTraceEvent] {
        &self.events
    }
}
