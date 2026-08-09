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
    pub(crate) fn frame_task_issued(frame: RouteFrameVersion) -> Self {
        Self::new(NavigationTraceEventKind::FrameTaskIssued).with_frame(frame)
    }

    pub(crate) fn shell_prepared(frame: RouteFrameVersion) -> Self {
        Self::new(NavigationTraceEventKind::ShellPrepared).with_frame(frame)
    }

    pub(crate) fn view_applied(frame: RouteFrameVersion) -> Self {
        Self::new(NavigationTraceEventKind::ViewApplied).with_frame(frame)
    }

    pub(crate) fn outlet_registered(frame: RouteFrameVersion, outlet: RouteSlotKey) -> Self {
        Self::new(NavigationTraceEventKind::OutletRegistered)
            .with_frame(frame)
            .with_outlet(outlet)
    }

    pub(crate) fn mount_requested(frame: RouteFrameVersion, outlet: RouteSlotKey) -> Self {
        Self::new(NavigationTraceEventKind::MountRequested)
            .with_frame(frame)
            .with_outlet(outlet)
    }

    pub(crate) fn mount_resolved(frame: RouteFrameVersion, outlet: RouteSlotKey) -> Self {
        Self::new(NavigationTraceEventKind::MountResolved)
            .with_frame(frame)
            .with_outlet(outlet)
    }

    pub(crate) fn mount_granted(
        frame: RouteFrameVersion,
        outlet: RouteSlotKey,
        slot: RouteSlotId,
    ) -> Self {
        Self::new(NavigationTraceEventKind::MountGranted)
            .with_frame(frame)
            .with_outlet(outlet)
            .with_slot(slot)
    }

    pub(crate) fn mount_rejected(
        frame: RouteFrameVersion,
        outlet: RouteSlotKey,
        rejection: MountRejectionKind,
    ) -> Self {
        let mut event = Self::new(NavigationTraceEventKind::MountRejected)
            .with_frame(frame)
            .with_outlet(outlet);
        event.mount_rejection = Some(rejection);
        event
    }

    pub(crate) fn mount_rejected_after_mutation(
        frame: RouteFrameVersion,
        outlet: RouteSlotKey,
        slot: RouteSlotId,
    ) -> Self {
        let mut event = Self::new(NavigationTraceEventKind::MountRejectedAfterMutation)
            .with_frame(frame)
            .with_outlet(outlet)
            .with_slot(slot);
        event.mount_rejection = Some(MountRejectionKind::RejectedAfterMutation);
        event
    }

    pub(crate) fn frame_activated(frame: RouteFrameVersion) -> Self {
        Self::new(NavigationTraceEventKind::FrameActivated).with_frame(frame)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn request_phase_changed(phase: NavigationPhase) -> Self {
        Self::new(NavigationTraceEventKind::RequestPhaseChanged).with_phase(phase)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn commit_published() -> Self {
        Self::new(NavigationTraceEventKind::CommitPublished)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn cleanup_settled(disposition: Option<CleanupDisposition>) -> Self {
        let mut event = Self::new(NavigationTraceEventKind::CleanupSettled);
        event.cleanup_disposition = disposition;
        event
    }

    #[cfg_attr(not(test), allow(dead_code))]
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

    fn with_frame(mut self, frame: RouteFrameVersion) -> Self {
        self.frame = Some(frame);
        self
    }

    fn with_outlet(mut self, outlet: RouteSlotKey) -> Self {
        self.outlet = Some(outlet);
        self
    }

    fn with_slot(mut self, slot: RouteSlotId) -> Self {
        self.slot = Some(slot);
        self
    }

    pub fn kind(&self) -> NavigationTraceEventKind {
        self.kind
    }

    pub fn frame(&self) -> Option<&RouteFrameVersion> {
        self.frame.as_ref()
    }

    pub fn outlet(&self) -> Option<&RouteSlotKey> {
        self.outlet.as_ref()
    }

    pub fn slot(&self) -> Option<RouteSlotId> {
        self.slot
    }

    pub fn phase(&self) -> Option<&NavigationPhase> {
        self.phase.as_ref()
    }

    pub fn mount_rejection(&self) -> Option<MountRejectionKind> {
        self.mount_rejection
    }

    pub fn cleanup_disposition(&self) -> Option<CleanupDisposition> {
        self.cleanup_disposition
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct NavigationTraceLog {
    events: Vec<NavigationTraceEvent>,
    closed: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NavigationTraceError {
    Closed,
}

impl NavigationTraceLog {
    pub(crate) const fn new() -> Self {
        Self {
            events: Vec::new(),
            closed: false,
        }
    }

    pub(crate) fn append(
        &mut self,
        event: NavigationTraceEvent,
    ) -> Result<(), NavigationTraceError> {
        if self.closed {
            return Err(NavigationTraceError::Closed);
        }
        let terminal = event.kind() == NavigationTraceEventKind::RequestTerminal;
        self.events.push(event);
        self.closed = terminal;
        Ok(())
    }

    pub(crate) fn events(&self) -> &[NavigationTraceEvent] {
        &self.events
    }

    pub(crate) const fn is_closed(&self) -> bool {
        self.closed
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn testing_close(&mut self) {
        self.closed = true;
    }
}
