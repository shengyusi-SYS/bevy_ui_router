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
    #[cfg_attr(not(test), allow(dead_code))]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructor_projects_only_stage_authorized_view_data() {
        let route = RouteId::new(7);
        let location = RouteLocation::new("/items/7")
            .with_search("?tab=details")
            .with_fragment("#summary");
        let params = RouteParams::from_pairs([("id".to_owned(), "7".to_owned())]);
        let input = RouteViewInput::new(
            location.clone(),
            Some(route),
            params.clone(),
            FrameOrigin::MatchedRoute(route),
            None,
        );

        assert_eq!(input.location(), &location);
        assert_eq!(input.route(), Some(route));
        assert_eq!(input.params(), &params);
        assert_eq!(input.raw_search(), Some("?tab=details"));
        assert_eq!(input.fragment(), Some("#summary"));
        assert_eq!(input.origin(), &FrameOrigin::MatchedRoute(route));
        assert!(input.failure().is_none());
    }
}
