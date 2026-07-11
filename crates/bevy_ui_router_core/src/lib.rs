#![forbid(unsafe_code)]

mod admission;
pub use admission::HostVersion;
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
pub mod runtime;
pub mod slot;
#[cfg(feature = "test-support")]
pub mod test_support;
pub mod trace;
pub mod transaction;
pub mod view_input;
pub mod view_key;

pub mod prelude {
    pub use crate::HostVersion;
    pub use crate::back::{BackCapabilitySnapshot, BackIntent, BackPolicy};
    pub use crate::branch::{MatchedBranch, MatchedRoute, RouteParams};
    pub use crate::branch_diff::BranchDiff;
    pub use crate::diagnostics::{
        DiagnosticEvent, DiagnosticEventKind, DiagnosticIdentity, DiagnosticSnapshot,
    };
    pub use crate::error::{RouteError, RouteErrorKind, RouteErrorSourceCategory};
    pub use crate::frame::{
        FrameCleanupPlan, FrameOrigin, FrameReconciliationPlan, RouteFrameBranch,
        RouteFrameIdentity, RouteFrameRecord, RouteFrameScope, RouteFrameStatus, RouteFrameVersion,
    };
    pub use crate::history::{HistoryError, MemoryHistory};
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
    pub use crate::transaction::{
        FrameApplyOutcome, FrameApplyRequest, FrameApplyToken, NavigationTransactionSnapshot,
        NavigationTransactionState, StaleApplyReason, StaleApplyResult, TransactionCommitError,
        TransactionCommitErrorKind, TransactionFailure,
    };
    pub use crate::view_input::RouteViewInput;
    pub use crate::view_key::{RouteViewKey, RouteViewKeyError};
}

#[cfg(test)]
mod root_navigation_state_tests {
    use crate::admission::HostAdmissionIssuer;
    use crate::ids::CoreIdAllocator;
    use crate::presentation::{ActivePresentationKind, RouterAvailability, RouterNavigationState};
    use crate::request::NavigationPhase;
    use crate::trace::{NavigationTraceEvent, NavigationTraceEventKind, NavigationTraceLog};

    #[test]
    fn fresh_state_awaits_a_confirmed_host_without_an_active_presentation() {
        let mut state = RouterNavigationState::new();

        assert_eq!(state.availability(), &RouterAvailability::AwaitingHost);
        assert_eq!(state.active().kind(), ActivePresentationKind::None);
        assert!(state.current_request().is_none());
        assert!(state.host().is_none());

        let mut ids = CoreIdAllocator::new().unwrap();
        let router = ids.allocate_router().unwrap();
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
        let first_router = first.allocate_router().unwrap();
        let first_request = first.allocate_request(first_router).unwrap();
        let second_request = first.allocate_request(first_router).unwrap();

        let mut second = CoreIdAllocator::new().unwrap();
        let second_router = second.allocate_router().unwrap();
        let foreign_request = second.allocate_request(second_router).unwrap();

        assert!(first.owns_request(first_request, first_router));
        assert!(first.owns_request(second_request, first_router));
        assert!(!first.owns_request(foreign_request, second_router));
        assert_ne!(first_request, second_request);
        assert_ne!(first_request, foreign_request);
    }

    #[test]
    fn navigation_state_accepts_only_the_issuers_current_confirmation() {
        let mut ids = CoreIdAllocator::new().unwrap();
        let router = ids.allocate_router().unwrap();
        let mut issuer = HostAdmissionIssuer::new().unwrap();

        let first_token = issuer.issue(router).unwrap();
        let first_grant = issuer.grant(&first_token, &mut ids).unwrap();
        let first = issuer.confirm(&first_grant).unwrap();

        let second_token = issuer.issue(router).unwrap();
        let second_grant = issuer.grant(&second_token, &mut ids).unwrap();
        let second = issuer.confirm(&second_grant).unwrap();

        let mut state = RouterNavigationState::new();
        let before = state.snapshot();
        assert!(state.confirm_host(&issuer, &first).is_err());
        assert_eq!(state.snapshot(), before);

        state.confirm_host(&issuer, &second).unwrap();
        assert_eq!(state.host(), Some(second.version()));
        assert_eq!(state.top_slot(), Some(second.top_slot()));
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
