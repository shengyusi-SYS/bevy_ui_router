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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticEvent {
    kind: DiagnosticEventKind,
    identity: Option<DiagnosticIdentity>,
    redacted_message: String,
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
            redacted_message: redacted_message.into(),
        }
    }

    #[allow(dead_code)]
    pub(crate) fn host_or_slot_unavailable(redacted_message: impl Into<String>) -> Self {
        Self {
            kind: DiagnosticEventKind::HostOrSlotUnavailable,
            identity: None,
            redacted_message: redacted_message.into(),
        }
    }

    #[allow(dead_code)]
    pub(crate) fn match_failed(redacted_message: impl Into<String>) -> Self {
        Self {
            kind: DiagnosticEventKind::MatchFailed,
            identity: None,
            redacted_message: redacted_message.into(),
        }
    }

    #[allow(dead_code)]
    pub(crate) fn match_conflict(redacted_message: impl Into<String>) -> Self {
        Self {
            kind: DiagnosticEventKind::MatchConflict,
            identity: None,
            redacted_message: redacted_message.into(),
        }
    }

    pub const fn kind(&self) -> DiagnosticEventKind {
        self.kind
    }

    pub const fn identity(&self) -> Option<DiagnosticIdentity> {
        self.identity
    }

    pub fn redacted_message(&self) -> &str {
        &self.redacted_message
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
}
