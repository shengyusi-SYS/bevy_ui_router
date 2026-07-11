#![allow(dead_code)]

use crate::branch::RouteParams;
use crate::frame::FrameOrigin;
use crate::ids::RouteId;
use crate::location::RouteLocation;
use crate::request::NavigationFailure;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouteViewInput {
    location: RouteLocation,
    route: Option<RouteId>,
    params: RouteParams,
    origin: FrameOrigin,
    failure: Option<NavigationFailure>,
}

impl RouteViewInput {
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

    pub const fn location(&self) -> &RouteLocation {
        &self.location
    }

    pub const fn route(&self) -> Option<RouteId> {
        self.route
    }

    pub const fn params(&self) -> &RouteParams {
        &self.params
    }

    pub fn raw_search(&self) -> Option<&str> {
        self.location.search()
    }

    pub fn fragment(&self) -> Option<&str> {
        self.location.fragment()
    }

    pub const fn origin(&self) -> &FrameOrigin {
        &self.origin
    }

    pub const fn failure(&self) -> Option<&NavigationFailure> {
        self.failure.as_ref()
    }
}
