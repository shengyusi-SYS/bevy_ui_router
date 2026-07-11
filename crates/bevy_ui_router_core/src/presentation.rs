#![allow(dead_code)]

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
        matched: Option<RouteBranchFingerprint>,
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

    pub(crate) fn route(
        location: RouteLocation,
        matched: Option<RouteBranchFingerprint>,
        frames: Vec<RouteFrameVersion>,
    ) -> Self {
        Self {
            state: ActivePresentationState::Route {
                location,
                matched,
                frames,
            },
        }
    }

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

    pub const fn kind(&self) -> ActivePresentationKind {
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

#[derive(Debug)]
pub(crate) struct RouterNavigationState {
    availability: RouterAvailability,
    current_request: Option<RequestStatus>,
    active: ActivePresentation,
    host: Option<HostVersion>,
}

impl RouterNavigationState {
    pub(crate) const fn new() -> Self {
        Self {
            availability: RouterAvailability::AwaitingHost,
            current_request: None,
            active: ActivePresentation::none(),
            host: None,
        }
    }

    pub(crate) const fn availability(&self) -> &RouterAvailability {
        &self.availability
    }

    pub(crate) const fn active(&self) -> &ActivePresentation {
        &self.active
    }

    pub(crate) const fn host(&self) -> Option<&HostVersion> {
        self.host.as_ref()
    }

    pub(crate) fn confirm_host(&mut self, confirmation: HostAdmissionConfirmation) {
        self.host = Some(confirmation.version().clone());
        self.availability = RouterAvailability::Ready;
    }

    pub(crate) fn set_request(&mut self, status: Option<RequestStatus>) {
        self.current_request = status;
    }

    pub(crate) fn set_active(&mut self, active: ActivePresentation) {
        self.active = active;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fingerprint::{RouteBranchFingerprint, RouteSearchFingerprint};
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
        let route = ActivePresentation::route(requested, Some(matched.clone()), Vec::new());

        assert_eq!(recovered.matched(), Some(&matched));
        assert!(match_failure.matched().is_none());
        assert!(route.matched().is_none());
    }
}
