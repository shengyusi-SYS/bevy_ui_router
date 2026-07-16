#![forbid(unsafe_code)]

mod admission;
mod cleanup;
mod engine;
#[doc(hidden)]
pub mod integration;
mod mount;
pub use admission::{HostSnapshot, HostTraceEvent, HostTraceEventKind, HostVersion};
pub use cleanup::CleanupSummary;
pub mod back;
pub mod branch;
pub mod branch_diff;
pub mod diagnostics;
pub mod error;
pub mod fingerprint;
pub mod frame;
pub mod history;
pub mod ids;
pub mod location;
pub mod matcher;
pub mod meta;
pub mod ownership;
pub mod pattern;
pub mod presentation;
pub mod request;
pub mod route_tree;
pub mod slot;
#[cfg(feature = "test-support")]
pub mod test_support;
pub mod trace;
pub mod view_input;
pub mod view_key;

pub mod prelude {
    pub use crate::back::{BackCapabilitySnapshot, BackIntent, BackPolicy};
    pub use crate::branch::{MatchedBranch, MatchedRoute, RouteParams};
    pub use crate::branch_diff::BranchDiff;
    pub use crate::diagnostics::{
        DiagnosticEvent, DiagnosticEventKind, DiagnosticIdentity, DiagnosticSeverity,
        DiagnosticSnapshot, ReportDisposition,
    };
    pub use crate::error::{RouteError, RouteErrorKind, RouteErrorSourceCategory};
    pub use crate::frame::{
        FrameCleanupPlan, FrameOrigin, FrameReconciliationPlan, RouteFrameBranch,
        RouteFrameIdentity, RouteFrameRecord, RouteFrameScope, RouteFrameStatus, RouteFrameVersion,
    };
    pub use crate::history::{HistoryError, HistorySnapshot, MemoryHistory};
    pub use crate::ids::{
        NavigationRequestId, RouteFrameId, RouteGeneration, RouteHostId, RouteId, RouteSlotId,
        RouterInstanceId,
    };
    pub use crate::location::RouteLocation;
    pub use crate::matcher::RouteMatcher;
    pub use crate::meta::{ErrorBoundaryKey, RouteExplicitKey, RouteMeta};
    pub use crate::ownership::{CleanupScope, OwnedRegion};
    pub use crate::pattern::{RoutePathSegment, RoutePattern, RoutePatternKind};
    pub use crate::presentation::{
        ActivePresentation, ActivePresentationKind, RouterAvailability, RouterUnavailableReason,
    };
    pub use crate::request::{
        NavigationFailure, NavigationFailureKind, NavigationPhase, RequestStatus,
    };
    pub use crate::route_tree::{RouteNodeOptions, RouteTree, RouteTreeBuilder};
    pub use crate::slot::RouteSlotKey;
    pub use crate::trace::{
        CleanupDisposition, MountRejectionKind, NavigationTraceEvent, NavigationTraceEventKind,
    };
    pub use crate::view_input::RouteViewInput;
    pub use crate::view_key::{RouteViewKey, RouteViewKeyError};
    pub use crate::{
        CleanupSummary, HostSnapshot, HostTraceEvent, HostTraceEventKind, HostVersion,
    };
}

#[cfg(test)]
mod root_navigation_state_tests {
    use crate::admission::HostAdmissionIssuer;
    use crate::frame::{FrameOrigin, RouteFrameVersion};
    use crate::ids::CoreIdAllocator;
    use crate::ids::RouteGeneration;
    use crate::presentation::{ActivePresentationKind, RouterAvailability, RouterNavigationState};
    use crate::request::NavigationPhase;
    use crate::trace::{NavigationTraceEvent, NavigationTraceEventKind, NavigationTraceLog};

    #[test]
    fn fresh_state_awaits_a_confirmed_host_without_an_active_presentation() {
        let mut ids = CoreIdAllocator::new().unwrap();
        let router = ids.allocate_router_identity().unwrap();
        let mut state = RouterNavigationState::new(router);

        assert_eq!(state.availability(), &RouterAvailability::AwaitingHost);
        assert_eq!(state.active().kind(), ActivePresentationKind::None);
        assert!(state.current_request().is_none());
        assert!(state.host().is_none());

        let mut admission = HostAdmissionIssuer::new().unwrap();
        let token = admission.issue(router).unwrap();
        let grant = admission.grant(&token, &mut ids).unwrap();

        assert!(state.host().is_none());
        assert!(state.top_slot().is_none());

        let confirmation = admission.confirm(&grant).unwrap();
        let mut foreign = HostAdmissionIssuer::new().unwrap();
        assert!(state.confirm_host(&foreign, &confirmation).is_err());
        assert!(state.host().is_none());
        assert!(state.top_slot().is_none());

        state.confirm_host(&admission, &confirmation).unwrap();

        assert_eq!(state.host(), Some(confirmation.version()));
        assert_eq!(state.top_slot(), Some(confirmation.top_slot()));
        assert_eq!(confirmation.version().router(), router.router());
        assert_eq!(confirmation.version().host(), grant.version().host());
        assert_eq!(confirmation.version().epoch(), 1);
        let frame = RouteFrameVersion::new(
            confirmation.version().router_identity(),
            confirmation.version().host(),
            confirmation.top_slot(),
            FrameOrigin::RouterFallback,
            ids.allocate_frame().unwrap(),
            RouteGeneration::initial(),
        );
        assert_eq!(frame.router(), router.router());
        assert_eq!(frame.router_identity(), router);
        assert_eq!(frame.host(), confirmation.version().host());
        assert_eq!(frame.slot(), confirmation.top_slot());
        let ready = state.snapshot();
        assert!(state.confirm_host(&admission, &confirmation).is_err());
        assert_eq!(state.snapshot(), ready);

        let foreign_token = foreign.issue(router).unwrap();
        let foreign_grant = foreign.grant(&foreign_token, &mut ids).unwrap();
        let foreign_confirmation = foreign.confirm(&foreign_grant).unwrap();
        assert!(state.confirm_host(&foreign, &foreign_confirmation).is_err());
        assert_eq!(state.snapshot(), ready);
    }

    #[test]
    fn request_ids_bind_the_allocator_domain_router_and_monotonic_sequence() {
        let mut first = CoreIdAllocator::new().unwrap();
        let first_router = first.allocate_router_identity().unwrap();
        let first_request = first.allocate_request(first_router).unwrap();
        let second_request = first.allocate_request(first_router).unwrap();

        let mut second = CoreIdAllocator::new().unwrap();
        let second_router = second.allocate_router_identity().unwrap();
        let foreign_request = second.allocate_request(second_router).unwrap();

        assert!(first.owns_request(first_request, first_router));
        assert!(first.owns_request(second_request, first_router));
        assert!(!first.owns_request(foreign_request, second_router));
        assert_ne!(first_request, second_request);
        assert_ne!(first_request, foreign_request);
    }

    #[test]
    fn navigation_state_rejects_same_raw_router_from_foreign_world_without_change() {
        let mut ids = CoreIdAllocator::new().unwrap();
        let router = ids.allocate_router_identity().unwrap();
        let mut foreign_ids = CoreIdAllocator::new().unwrap();
        let foreign_router = foreign_ids.allocate_router_identity().unwrap();
        let mut issuer = HostAdmissionIssuer::new().unwrap();

        let token = issuer.issue(foreign_router).unwrap();
        let grant = issuer.grant(&token, &mut foreign_ids).unwrap();
        let confirmation = issuer.confirm(&grant).unwrap();

        assert_eq!(router.router(), foreign_router.router());
        let mut state = RouterNavigationState::new(router);
        let before = state.snapshot();
        assert!(state.confirm_host(&issuer, &confirmation).is_err());
        assert_eq!(state.snapshot(), before);
    }

    #[test]
    fn navigation_state_rejects_a_different_router_in_the_same_world_without_change() {
        let mut ids = CoreIdAllocator::new().unwrap();
        let router = ids.allocate_router_identity().unwrap();
        let other_router = ids.allocate_router_identity().unwrap();
        let mut issuer = HostAdmissionIssuer::new().unwrap();

        let token = issuer.issue(other_router).unwrap();
        let grant = issuer.grant(&token, &mut ids).unwrap();
        let confirmation = issuer.confirm(&grant).unwrap();

        let mut state = RouterNavigationState::new(router);
        let before = state.snapshot();
        assert!(state.confirm_host(&issuer, &confirmation).is_err());
        assert_eq!(state.snapshot(), before);
    }

    #[test]
    fn trace_observations_preserve_emission_order_within_one_update() {
        let mut trace = NavigationTraceLog::new();
        trace
            .append(NavigationTraceEvent::request_phase_changed(
                NavigationPhase::Matching,
            ))
            .unwrap();
        trace
            .append(NavigationTraceEvent::commit_published())
            .unwrap();
        trace
            .append(NavigationTraceEvent::request_terminal())
            .unwrap();

        assert_eq!(
            trace
                .events()
                .iter()
                .map(NavigationTraceEvent::kind)
                .collect::<Vec<_>>(),
            vec![
                NavigationTraceEventKind::RequestPhaseChanged,
                NavigationTraceEventKind::CommitPublished,
                NavigationTraceEventKind::RequestTerminal,
            ]
        );
    }
}
