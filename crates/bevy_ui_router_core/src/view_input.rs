use crate::branch::RouteParams;
use crate::frame::FrameOrigin;
use crate::ids::RouteId;
use crate::location::RouteLocation;
use crate::request::NavigationFailure;

pub struct RouteViewInput {
    location: RouteLocation,
    route: Option<RouteId>,
    params: RouteParams,
    origin: FrameOrigin,
    failure: Option<NavigationFailure>,
}

impl RouteViewInput {
    #[allow(dead_code)]
    pub(crate) fn new(
        location: RouteLocation,
        route: Option<RouteId>,
        params: RouteParams,
        origin: FrameOrigin,
        failure: Option<NavigationFailure>,
    ) -> Self {
        Self {
            location,
            route,
            params,
            origin,
            failure,
        }
    }

    pub fn location(&self) -> &RouteLocation {
        &self.location
    }

    pub fn route(&self) -> Option<RouteId> {
        self.route
    }

    pub fn params(&self) -> &RouteParams {
        &self.params
    }

    pub fn raw_search(&self) -> Option<&str> {
        self.location.search()
    }

    pub fn fragment(&self) -> Option<&str> {
        self.location.fragment()
    }

    pub fn origin(&self) -> &FrameOrigin {
        &self.origin
    }

    pub fn failure(&self) -> Option<&NavigationFailure> {
        self.failure.as_ref()
    }
}
