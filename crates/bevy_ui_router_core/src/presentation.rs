use crate::admission::{HostAdmissionConfirmation, HostVersion};
use crate::fingerprint::RouteBranchFingerprint;
use crate::frame::RouteFrameVersion;
use crate::location::RouteLocation;
use crate::request::{NavigationFailure, RequestStatus};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RouterUnavailableReason {
    HostConflict,
    InvalidHostUiTopology,
    HostLost,
    AuthorityEstablishmentFailed,
    RegistryInvariantViolation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RouterAvailability {
    AwaitingHost,
    Ready,
    RecoveringAuthority,
    Unavailable(RouterUnavailableReason),
}

impl RouterAvailability {
    pub fn unavailable_reason(&self) -> Option<RouterUnavailableReason> {
        match self {
            Self::Unavailable(reason) => Some(*reason),
            Self::AwaitingHost | Self::Ready | Self::RecoveringAuthority => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActivePresentationKind {
    None,
    Route,
    Error,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ActivePresentationState {
    None,
    Route {
        location: RouteLocation,
        frames: Vec<RouteFrameVersion>,
    },
    Error {
        requested: RouteLocation,
        matched: Option<RouteBranchFingerprint>,
        failure: NavigationFailure,
        frames: Vec<RouteFrameVersion>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActivePresentation {
    state: ActivePresentationState,
}

impl ActivePresentation {
    pub(crate) const fn none() -> Self {
        Self {
            state: ActivePresentationState::None,
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn route(location: RouteLocation, frames: Vec<RouteFrameVersion>) -> Self {
        Self {
            state: ActivePresentationState::Route { location, frames },
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn error(
        requested: RouteLocation,
        matched: Option<RouteBranchFingerprint>,
        failure: NavigationFailure,
        frames: Vec<RouteFrameVersion>,
    ) -> Self {
        Self {
            state: ActivePresentationState::Error {
                requested,
                matched,
                failure,
                frames,
            },
        }
    }

    pub fn kind(&self) -> ActivePresentationKind {
        match self.state {
            ActivePresentationState::None => ActivePresentationKind::None,
            ActivePresentationState::Route { .. } => ActivePresentationKind::Route,
            ActivePresentationState::Error { .. } => ActivePresentationKind::Error,
        }
    }

    pub fn location(&self) -> Option<&RouteLocation> {
        match &self.state {
            ActivePresentationState::Route { location, .. } => Some(location),
            ActivePresentationState::None | ActivePresentationState::Error { .. } => None,
        }
    }

    pub fn requested(&self) -> Option<&RouteLocation> {
        match &self.state {
            ActivePresentationState::Error { requested, .. } => Some(requested),
            ActivePresentationState::None | ActivePresentationState::Route { .. } => None,
        }
    }

    pub fn matched(&self) -> Option<&RouteBranchFingerprint> {
        match &self.state {
            ActivePresentationState::Error { matched, .. } => matched.as_ref(),
            ActivePresentationState::None | ActivePresentationState::Route { .. } => None,
        }
    }

    pub fn frames(&self) -> &[RouteFrameVersion] {
        match &self.state {
            ActivePresentationState::None => &[],
            ActivePresentationState::Route { frames, .. }
            | ActivePresentationState::Error { frames, .. } => frames,
        }
    }

    pub fn failure(&self) -> Option<&NavigationFailure> {
        match &self.state {
            ActivePresentationState::Error { failure, .. } => Some(failure),
            ActivePresentationState::None | ActivePresentationState::Route { .. } => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ConfirmedHostAuthority {
    version: HostVersion,
    top_slot: crate::ids::RouteSlotId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RouterNavigationState {
    router: crate::ids::RouterIdentity,
    availability: RouterAvailability,
    current_request: Option<RequestStatus>,
    active: ActivePresentation,
    authority: Option<ConfirmedHostAuthority>,
}

impl RouterNavigationState {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) const fn new(router: crate::ids::RouterIdentity) -> Self {
        Self {
            router,
            availability: RouterAvailability::AwaitingHost,
            current_request: None,
            active: ActivePresentation::none(),
            authority: None,
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) const fn availability(&self) -> &RouterAvailability {
        &self.availability
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) const fn active(&self) -> &ActivePresentation {
        &self.active
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) const fn current_request(&self) -> Option<&RequestStatus> {
        self.current_request.as_ref()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) const fn host(&self) -> Option<&HostVersion> {
        match &self.authority {
            Some(authority) => Some(&authority.version),
            None => None,
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn top_slot(&self) -> Option<crate::ids::RouteSlotId> {
        self.authority.as_ref().map(|authority| authority.top_slot)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn confirm_host(
        &mut self,
        issuer: &crate::admission::HostAdmissionIssuer,
        confirmation: &HostAdmissionConfirmation,
    ) -> Result<(), AuthorityConfirmationError> {
        if self.authority.is_some()
            || self.availability != RouterAvailability::AwaitingHost
            || confirmation.version().router_identity() != self.router
            || !issuer.is_current_confirmation(confirmation)
        {
            return Err(AuthorityConfirmationError);
        }
        self.authority = Some(ConfirmedHostAuthority {
            version: confirmation.version().clone(),
            top_slot: confirmation.top_slot(),
        });
        self.availability = RouterAvailability::Ready;
        Ok(())
    }

    pub(crate) fn prepare_host_ready(
        &self,
        version: &HostVersion,
        top_slot: crate::ids::RouteSlotId,
    ) -> Option<Self> {
        if self.authority.is_some() || version.router_identity() != self.router {
            return None;
        }
        if !matches!(
            self.availability,
            RouterAvailability::AwaitingHost | RouterAvailability::Unavailable(_)
        ) {
            return None;
        }
        let mut ready = self.clone();
        ready.authority = Some(ConfirmedHostAuthority {
            version: version.clone(),
            top_slot,
        });
        ready.availability = RouterAvailability::Ready;
        Some(ready)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn set_request(&mut self, status: Option<RequestStatus>) {
        self.current_request = status;
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn set_active(&mut self, active: ActivePresentation) {
        self.active = active;
    }

    pub(crate) fn begin_authority_recovery(
        &mut self,
        version: &HostVersion,
        top_slot: crate::ids::RouteSlotId,
    ) -> bool {
        let exact = self.availability == RouterAvailability::Ready
            && self.authority.as_ref().is_some_and(|authority| {
                &authority.version == version && authority.top_slot == top_slot
            });
        if exact {
            self.availability = RouterAvailability::RecoveringAuthority;
        }
        exact
    }

    pub(crate) fn complete_authority_settlement(
        &mut self,
        version: &HostVersion,
        top_slot: crate::ids::RouteSlotId,
    ) -> bool {
        self.complete_authority_settlement_with_availability(
            version,
            top_slot,
            RouterAvailability::AwaitingHost,
        )
    }

    pub(crate) fn complete_host_loss_settlement(
        &mut self,
        version: &HostVersion,
        top_slot: crate::ids::RouteSlotId,
    ) -> bool {
        self.complete_authority_settlement_with_availability(
            version,
            top_slot,
            RouterAvailability::Unavailable(RouterUnavailableReason::HostLost),
        )
    }

    fn complete_authority_settlement_with_availability(
        &mut self,
        version: &HostVersion,
        top_slot: crate::ids::RouteSlotId,
        availability: RouterAvailability,
    ) -> bool {
        let exact = self.availability == RouterAvailability::RecoveringAuthority
            && self.authority.as_ref().is_some_and(|authority| {
                &authority.version == version && authority.top_slot == top_slot
            });
        if exact {
            self.active = ActivePresentation::none();
            self.authority = None;
            self.availability = availability;
        }
        exact
    }

    pub(crate) fn fail_closed_unavailable(&mut self, reason: RouterUnavailableReason) {
        self.active = ActivePresentation::none();
        self.authority = None;
        self.current_request = None;
        self.availability = RouterAvailability::Unavailable(reason);
    }

    pub(crate) fn terminalize_post_commit_host_loss(&mut self) {
        self.active = ActivePresentation::none();
        self.authority = None;
        self.current_request = None;
        self.availability = RouterAvailability::Unavailable(RouterUnavailableReason::HostLost);
    }

    #[cfg(test)]
    pub(crate) fn snapshot(&self) -> Self {
        self.clone()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthorityConfirmationError;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fingerprint::{RouteBranchFingerprint, RouteSearchFingerprint};
    use crate::ids::CoreIdAllocator;
    use crate::request::NavigationFailureKind;

    fn matched_fingerprint() -> RouteBranchFingerprint {
        let location = RouteLocation::new("/items/7");
        let search = RouteSearchFingerprint::from_location(&location);
        RouteBranchFingerprint::from_parts(std::iter::empty(), &search)
    }

    #[test]
    fn only_committed_error_presentations_expose_their_matched_fingerprint() {
        let requested = RouteLocation::new("/items/7");
        let matched = matched_fingerprint();
        let recovered = ActivePresentation::error(
            requested.clone(),
            Some(matched.clone()),
            NavigationFailure::new(NavigationFailureKind::Apply, "view failed"),
            Vec::new(),
        );
        let match_failure = ActivePresentation::error(
            requested.clone(),
            None,
            NavigationFailure::new(NavigationFailureKind::Match, "no match"),
            Vec::new(),
        );
        let route = ActivePresentation::route(requested, Vec::new());

        assert_eq!(recovered.matched(), Some(&matched));
        assert!(match_failure.matched().is_none());
        assert!(route.matched().is_none());
    }

    #[test]
    fn router_state_keeps_request_and_active_presentation_orthogonal() {
        let mut ids = CoreIdAllocator::new().unwrap();
        let router = ids.allocate_router_identity().unwrap();
        let mut state = RouterNavigationState::new(router);
        let status = RequestStatus::Running(crate::request::NavigationPhase::Matching);
        let active = ActivePresentation::route(RouteLocation::new("/ready"), Vec::new());

        state.set_request(Some(status.clone()));
        state.set_active(active.clone());

        assert_eq!(state.current_request(), Some(&status));
        assert_eq!(state.active(), &active);
        assert_eq!(state.availability(), &RouterAvailability::AwaitingHost);
    }
}
