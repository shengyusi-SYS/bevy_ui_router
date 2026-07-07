//! ```compile_fail
//! use bevy_ui_router_core::prelude::{
//!     DiagnosticIdentity, RouteFrameId, RouteGeneration, RouteHostId, RouteId, RouteSlotId,
//!     RouterInstanceId,
//! };
//!
//! fn build(
//!     router: RouterInstanceId,
//!     host: RouteHostId,
//!     slot: RouteSlotId,
//!     frame: RouteFrameId,
//!     generation: RouteGeneration,
//!     route: Option<RouteId>,
//! ) {
//!     let _ = DiagnosticIdentity::new(router, host, slot, frame, generation, route);
//! }
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_core::prelude::{DiagnosticEvent, DiagnosticIdentity};
//!
//! fn emit(identity: DiagnosticIdentity) {
//!     let _ = DiagnosticEvent::pending_overwrite(identity, "hidden");
//! }
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_core::prelude::{DiagnosticIdentity, DiagnosticSnapshot};
//!
//! fn activate(identity: DiagnosticIdentity) {
//!     let _ = DiagnosticSnapshot::new().with_active(identity);
//! }
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_core::prelude::BranchPlanningDiagnostic;
//!
//! let _ = BranchPlanningDiagnostic::new(0, 0, 0, false, false, false);
//! ```

use crate::frame::{RouteFrameIdentity, RouteFrameStatus};
use crate::ids::{
    RouteFrameId, RouteGeneration, RouteHostId, RouteId, RouteSlotId, RouterInstanceId,
};
use crate::location::RouteLocation;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiagnosticIdentity {
    router: RouterInstanceId,
    host: RouteHostId,
    slot: RouteSlotId,
    frame: RouteFrameId,
    generation: RouteGeneration,
    route: Option<RouteId>,
}

impl DiagnosticIdentity {
    #[allow(dead_code)]
    pub(crate) const fn new(
        router: RouterInstanceId,
        host: RouteHostId,
        slot: RouteSlotId,
        frame: RouteFrameId,
        generation: RouteGeneration,
        route: Option<RouteId>,
    ) -> Self {
        Self {
            router,
            host,
            slot,
            frame,
            generation,
            route,
        }
    }

    pub const fn router(self) -> RouterInstanceId {
        self.router
    }

    pub const fn host(self) -> RouteHostId {
        self.host
    }

    pub const fn slot(self) -> RouteSlotId {
        self.slot
    }

    pub const fn frame(self) -> RouteFrameId {
        self.frame
    }

    pub const fn generation(self) -> RouteGeneration {
        self.generation
    }

    pub const fn route(self) -> Option<RouteId> {
        self.route
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticEventKind {
    PendingOverwrite,
    HostOrSlotUnavailable,
    MatchFailed,
    MatchConflict,
    BranchPlanned,
    FrameLifecycle,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrameLifecycleDiagnostic {
    identity: RouteFrameIdentity,
    status: RouteFrameStatus,
}

impl FrameLifecycleDiagnostic {
    #[allow(dead_code)]
    pub(crate) const fn new(identity: RouteFrameIdentity, status: RouteFrameStatus) -> Self {
        Self { identity, status }
    }

    pub const fn identity(&self) -> &RouteFrameIdentity {
        &self.identity
    }

    pub const fn status(&self) -> RouteFrameStatus {
        self.status
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BranchPlanningDiagnostic {
    common_prefix_len: usize,
    leaving_len: usize,
    entering_len: usize,
    provenance_changed: bool,
    search_changed: bool,
    branch_fingerprint_changed: bool,
}

impl BranchPlanningDiagnostic {
    pub const fn new(
        common_prefix_len: usize,
        leaving_len: usize,
        entering_len: usize,
        provenance_changed: bool,
        search_changed: bool,
        branch_fingerprint_changed: bool,
    ) -> Self {
        Self {
            common_prefix_len,
            leaving_len,
            entering_len,
            provenance_changed,
            search_changed,
            branch_fingerprint_changed,
        }
    }

    pub const fn common_prefix_len(&self) -> usize {
        self.common_prefix_len
    }

    pub const fn leaving_len(&self) -> usize {
        self.leaving_len
    }

    pub const fn entering_len(&self) -> usize {
        self.entering_len
    }

    pub const fn provenance_changed(&self) -> bool {
        self.provenance_changed
    }

    pub const fn search_changed(&self) -> bool {
        self.search_changed
    }

    pub const fn branch_fingerprint_changed(&self) -> bool {
        self.branch_fingerprint_changed
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticEvent {
    kind: DiagnosticEventKind,
    identity: Option<DiagnosticIdentity>,
    branch_planning: Option<BranchPlanningDiagnostic>,
    frame_lifecycle: Option<FrameLifecycleDiagnostic>,
    redacted_message: String,
}

pub trait DiagnosticEventFrameLifecycle {
    fn frame_lifecycle(&self) -> Option<&FrameLifecycleDiagnostic>;
}

impl DiagnosticEvent {
    #[allow(dead_code)]
    pub(crate) fn pending_overwrite(
        identity: DiagnosticIdentity,
        redacted_message: impl Into<String>,
    ) -> Self {
        Self {
            kind: DiagnosticEventKind::PendingOverwrite,
            identity: Some(identity),
            branch_planning: None,
            frame_lifecycle: None,
            redacted_message: redacted_message.into(),
        }
    }

    #[allow(dead_code)]
    pub(crate) fn host_or_slot_unavailable(redacted_message: impl Into<String>) -> Self {
        Self {
            kind: DiagnosticEventKind::HostOrSlotUnavailable,
            identity: None,
            branch_planning: None,
            frame_lifecycle: None,
            redacted_message: redacted_message.into(),
        }
    }

    #[allow(dead_code)]
    pub(crate) fn match_failed(redacted_message: impl Into<String>) -> Self {
        Self {
            kind: DiagnosticEventKind::MatchFailed,
            identity: None,
            branch_planning: None,
            frame_lifecycle: None,
            redacted_message: redacted_message.into(),
        }
    }

    #[allow(dead_code)]
    pub(crate) fn match_conflict(redacted_message: impl Into<String>) -> Self {
        Self {
            kind: DiagnosticEventKind::MatchConflict,
            identity: None,
            branch_planning: None,
            frame_lifecycle: None,
            redacted_message: redacted_message.into(),
        }
    }

    #[allow(dead_code)]
    pub(crate) fn branch_planned(branch_planning: BranchPlanningDiagnostic) -> Self {
        Self {
            kind: DiagnosticEventKind::BranchPlanned,
            identity: None,
            branch_planning: Some(branch_planning),
            frame_lifecycle: None,
            redacted_message: "branch planned".to_owned(),
        }
    }

    #[allow(dead_code)]
    pub(crate) fn frame_lifecycle(frame_lifecycle: FrameLifecycleDiagnostic) -> Self {
        let identity = frame_lifecycle.identity();
        Self {
            kind: DiagnosticEventKind::FrameLifecycle,
            identity: Some(DiagnosticIdentity::new(
                identity.router(),
                identity.host(),
                identity.slot(),
                identity.frame(),
                identity.generation(),
                Some(identity.route()),
            )),
            branch_planning: None,
            frame_lifecycle: Some(frame_lifecycle),
            redacted_message: "frame lifecycle changed".to_owned(),
        }
    }

    pub const fn kind(&self) -> DiagnosticEventKind {
        self.kind
    }

    pub const fn identity(&self) -> Option<DiagnosticIdentity> {
        self.identity
    }

    pub fn branch_planning(&self) -> Option<&BranchPlanningDiagnostic> {
        self.branch_planning.as_ref()
    }

    pub fn redacted_message(&self) -> &str {
        &self.redacted_message
    }
}

impl DiagnosticEventFrameLifecycle for DiagnosticEvent {
    fn frame_lifecycle(&self) -> Option<&FrameLifecycleDiagnostic> {
        self.frame_lifecycle.as_ref()
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DiagnosticSnapshot {
    requested: Option<RouteLocation>,
    active: Option<DiagnosticIdentity>,
    events: Vec<DiagnosticEvent>,
}

impl DiagnosticSnapshot {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_requested(mut self, requested: RouteLocation) -> Self {
        self.requested = Some(requested);
        self
    }

    #[allow(dead_code)]
    pub(crate) fn with_active(mut self, active: DiagnosticIdentity) -> Self {
        self.active = Some(active);
        self
    }

    #[allow(dead_code)]
    pub(crate) fn with_event(mut self, event: DiagnosticEvent) -> Self {
        self.events.push(event);
        self
    }

    pub fn requested(&self) -> Option<&RouteLocation> {
        self.requested.as_ref()
    }

    pub const fn active(&self) -> Option<DiagnosticIdentity> {
        self.active
    }

    pub fn events(&self) -> &[DiagnosticEvent] {
        &self.events
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::{RouteFrameIdentity, RouteFrameStatus};

    #[test]
    fn identity_chain_records_router_host_slot_frame_generation_and_route() {
        let identity = DiagnosticIdentity::new(
            RouterInstanceId::new(1),
            RouteHostId::new(2),
            RouteSlotId::new(3),
            RouteFrameId::new(4),
            RouteGeneration::initial(),
            Some(RouteId::new(5)),
        );

        assert_eq!(identity.router().raw(), 1);
        assert_eq!(identity.host().raw(), 2);
        assert_eq!(identity.slot().raw(), 3);
        assert_eq!(identity.frame().raw(), 4);
        assert_eq!(identity.generation().raw(), 0);
        assert_eq!(identity.route().map(RouteId::raw), Some(5));
    }

    #[test]
    fn pending_overwrite_and_missing_slot_are_explicit_events() {
        let identity = DiagnosticIdentity::new(
            RouterInstanceId::new(1),
            RouteHostId::new(2),
            RouteSlotId::new(3),
            RouteFrameId::new(4),
            RouteGeneration::initial(),
            None,
        );

        let pending = DiagnosticEvent::pending_overwrite(identity, "newer navigation won");
        let missing = DiagnosticEvent::host_or_slot_unavailable("slot was not present");

        assert_eq!(pending.kind(), DiagnosticEventKind::PendingOverwrite);
        assert_eq!(
            pending.identity().map(DiagnosticIdentity::frame),
            Some(RouteFrameId::new(4))
        );
        assert_eq!(missing.kind(), DiagnosticEventKind::HostOrSlotUnavailable);
        assert!(missing.identity().is_none());
    }

    #[test]
    fn snapshot_keeps_requested_location_separate_from_active_identity() {
        let snapshot = DiagnosticSnapshot::new()
            .with_requested(RouteLocation::new("/settings"))
            .with_event(DiagnosticEvent::host_or_slot_unavailable("slot missing"));

        assert_eq!(
            snapshot.requested().map(RouteLocation::path),
            Some("/settings")
        );
        assert!(snapshot.active().is_none());
        assert_eq!(snapshot.events().len(), 1);
    }

    #[test]
    fn snapshot_can_report_active_identity_without_changing_requested_location() {
        let identity = DiagnosticIdentity::new(
            RouterInstanceId::new(10),
            RouteHostId::new(20),
            RouteSlotId::new(30),
            RouteFrameId::new(40),
            RouteGeneration::initial(),
            Some(RouteId::new(50)),
        );
        let snapshot = DiagnosticSnapshot::new()
            .with_requested(RouteLocation::new("/settings"))
            .with_active(identity);

        assert_eq!(
            snapshot.requested().map(RouteLocation::path),
            Some("/settings")
        );
        assert_eq!(snapshot.active(), Some(identity));
        assert!(snapshot.events().is_empty());
    }

    #[test]
    fn match_diagnostics_record_requested_location_without_active_identity() {
        let snapshot = DiagnosticSnapshot::new()
            .with_requested(RouteLocation::new("/missing"))
            .with_event(DiagnosticEvent::match_failed(
                "no route matched requested location",
            ));

        assert_eq!(
            snapshot.requested().map(RouteLocation::path),
            Some("/missing")
        );
        assert!(snapshot.active().is_none());
        assert_eq!(
            snapshot.events()[0].kind(),
            DiagnosticEventKind::MatchFailed
        );
        assert!(snapshot.events()[0].identity().is_none());
    }

    #[test]
    fn match_conflict_diagnostics_record_no_active_identity() {
        let event = DiagnosticEvent::match_conflict("ambiguous route match");

        assert_eq!(event.kind(), DiagnosticEventKind::MatchConflict);
        assert!(event.identity().is_none());
        assert_eq!(event.redacted_message(), "ambiguous route match");
    }

    #[test]
    fn branch_planning_event_has_no_active_identity() {
        let payload = BranchPlanningDiagnostic::new(2, 1, 1, true, false, true);
        let event = DiagnosticEvent::branch_planned(payload.clone());

        assert_eq!(event.kind(), DiagnosticEventKind::BranchPlanned);
        assert!(event.identity().is_none());
        assert_eq!(event.branch_planning(), Some(&payload));
        assert_eq!(event.branch_planning().unwrap().common_prefix_len(), 2);
        assert_eq!(event.branch_planning().unwrap().leaving_len(), 1);
        assert_eq!(event.branch_planning().unwrap().entering_len(), 1);
        assert!(event.branch_planning().unwrap().provenance_changed());
        assert!(!event.branch_planning().unwrap().search_changed());
        assert!(
            event
                .branch_planning()
                .unwrap()
                .branch_fingerprint_changed()
        );
        assert_eq!(event.redacted_message(), "branch planned");
        assert!(!event.redacted_message().contains("frame"));
        assert!(!event.redacted_message().contains("generation"));
    }

    #[test]
    fn frame_lifecycle_diagnostic_records_identity_and_status() {
        let identity = RouteFrameIdentity::new(
            RouterInstanceId::new(1),
            RouteHostId::new(2),
            RouteSlotId::new(3),
            RouteId::new(5),
            RouteFrameId::new(4),
            RouteGeneration::initial(),
        );
        let payload = FrameLifecycleDiagnostic::new(identity, RouteFrameStatus::ApplyingView);
        let event = DiagnosticEvent::frame_lifecycle(payload.clone());

        assert_eq!(event.kind(), DiagnosticEventKind::FrameLifecycle);
        assert_eq!(
            event.identity().map(DiagnosticIdentity::frame),
            Some(RouteFrameId::new(4))
        );
        assert_eq!(event.frame_lifecycle(), Some(&payload));
        assert_eq!(event.redacted_message(), "frame lifecycle changed");
        assert!(!event.redacted_message().contains("/"));
    }
}
