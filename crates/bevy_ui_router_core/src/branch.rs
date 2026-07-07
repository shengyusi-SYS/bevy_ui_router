use std::{collections::BTreeMap, fmt};

use crate::fingerprint::{
    RouteBranchFingerprint, RouteBranchProvenance, RouteParamsFingerprint, RouteSearchFingerprint,
    RouteSegmentReuseKey,
};
use crate::ids::RouteId;
use crate::meta::{ErrorBoundaryKey, RouteExplicitKey, RouteMeta};
use crate::slot::RouteSlotKey;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouteParams {
    values: BTreeMap<String, String>,
}

impl RouteParams {
    #[allow(dead_code)]
    pub(crate) fn new() -> Self {
        Self {
            values: BTreeMap::new(),
        }
    }

    #[allow(dead_code)]
    pub(crate) fn from_pairs(pairs: impl IntoIterator<Item = (String, String)>) -> Self {
        Self {
            values: pairs.into_iter().collect(),
        }
    }

    pub fn get(&self, name: &str) -> Option<&str> {
        self.values.get(name).map(String::as_str)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.values
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MatchedRoute {
    route: RouteId,
    params: RouteParams,
    slot: RouteSlotKey,
    explicit_key: Option<RouteExplicitKey>,
    error_boundary: Option<ErrorBoundaryKey>,
    meta: RouteMeta,
    params_fingerprint: RouteParamsFingerprint,
    segment_reuse_key: RouteSegmentReuseKey,
}

impl MatchedRoute {
    #[allow(dead_code)]
    pub(crate) fn new(
        route: RouteId,
        params: RouteParams,
        slot: RouteSlotKey,
        explicit_key: Option<RouteExplicitKey>,
        error_boundary: Option<ErrorBoundaryKey>,
        meta: RouteMeta,
        params_fingerprint: RouteParamsFingerprint,
        segment_reuse_key: RouteSegmentReuseKey,
    ) -> Self {
        Self {
            route,
            params,
            slot,
            explicit_key,
            error_boundary,
            meta,
            params_fingerprint,
            segment_reuse_key,
        }
    }

    pub const fn route(&self) -> RouteId {
        self.route
    }

    pub const fn params(&self) -> &RouteParams {
        &self.params
    }

    pub const fn slot(&self) -> &RouteSlotKey {
        &self.slot
    }

    pub const fn explicit_key(&self) -> Option<&RouteExplicitKey> {
        self.explicit_key.as_ref()
    }

    pub const fn error_boundary(&self) -> Option<&ErrorBoundaryKey> {
        self.error_boundary.as_ref()
    }

    pub const fn meta(&self) -> &RouteMeta {
        &self.meta
    }

    pub const fn params_fingerprint(&self) -> &RouteParamsFingerprint {
        &self.params_fingerprint
    }

    pub const fn segment_reuse_key(&self) -> &RouteSegmentReuseKey {
        &self.segment_reuse_key
    }
}

#[derive(Clone)]
pub struct MatchedBranch {
    routes: Vec<MatchedRoute>,
    #[allow(dead_code)]
    provenance: RouteBranchProvenance,
    search_fingerprint: RouteSearchFingerprint,
    branch_fingerprint: RouteBranchFingerprint,
}

impl fmt::Debug for MatchedBranch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MatchedBranch")
            .field("routes", &self.routes)
            .field("search_fingerprint", &self.search_fingerprint)
            .field("branch_fingerprint", &self.branch_fingerprint)
            .finish()
    }
}

impl MatchedBranch {
    #[allow(dead_code)]
    pub(crate) fn new(
        routes: Vec<MatchedRoute>,
        provenance: RouteBranchProvenance,
        search_fingerprint: RouteSearchFingerprint,
        branch_fingerprint: RouteBranchFingerprint,
    ) -> Self {
        Self {
            routes,
            provenance,
            search_fingerprint,
            branch_fingerprint,
        }
    }

    pub fn routes(&self) -> &[MatchedRoute] {
        &self.routes
    }

    pub fn leaf(&self) -> Option<&MatchedRoute> {
        self.routes.last()
    }

    pub fn is_empty(&self) -> bool {
        self.routes.is_empty()
    }

    #[allow(dead_code)]
    pub(crate) fn provenance(&self) -> &RouteBranchProvenance {
        &self.provenance
    }

    pub fn search_fingerprint(&self) -> &RouteSearchFingerprint {
        &self.search_fingerprint
    }

    pub fn branch_fingerprint(&self) -> &RouteBranchFingerprint {
        &self.branch_fingerprint
    }

    pub fn nearest_error_boundary_for(&self, route: RouteId) -> Option<&ErrorBoundaryKey> {
        let route_index = self
            .routes
            .iter()
            .position(|matched| matched.route() == route)?;
        self.routes[..=route_index]
            .iter()
            .rev()
            .find_map(MatchedRoute::error_boundary)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fingerprint::{
        RouteBranchFingerprint, RouteBranchProvenance, RouteParamsFingerprint,
        RouteSearchFingerprint, RouteSegmentReuseKey,
    };
    use crate::ids::RouteId;
    use crate::meta::RouteMeta;
    use crate::slot::RouteSlotKey;

    fn matched_route(route: RouteId, params: RouteParams) -> MatchedRoute {
        let params_fingerprint = RouteParamsFingerprint::from_route_params(route, &params);
        let segment_reuse_key = RouteSegmentReuseKey::from_parts(
            route,
            &RouteSlotKey::default(),
            None,
            &params_fingerprint,
        );

        MatchedRoute::new(
            route,
            params,
            RouteSlotKey::default(),
            None,
            None,
            RouteMeta::new(),
            params_fingerprint,
            segment_reuse_key,
        )
    }

    fn matched_branch(routes: Vec<MatchedRoute>) -> MatchedBranch {
        let search_fingerprint =
            RouteSearchFingerprint::from_location(&crate::location::RouteLocation::new("/"));
        let branch_fingerprint = RouteBranchFingerprint::from_parts(
            routes.iter().map(MatchedRoute::segment_reuse_key),
            &search_fingerprint,
        );

        MatchedBranch::new(
            routes,
            RouteBranchProvenance::next_tree_source(),
            search_fingerprint,
            branch_fingerprint,
        )
    }

    #[test]
    fn params_preserve_deterministic_name_order() {
        let params = RouteParams::from_pairs([
            ("project".to_owned(), "alpha".to_owned()),
            ("document".to_owned(), "intro".to_owned()),
        ]);

        assert_eq!(params.get("project"), Some("alpha"));
        assert_eq!(
            params.iter().collect::<Vec<_>>(),
            vec![("document", "intro"), ("project", "alpha")]
        );
    }

    #[test]
    fn matched_branch_exposes_ordered_route_chain() {
        let root = matched_route(RouteId::new(1), RouteParams::new());
        let child = matched_route(
            RouteId::new(2),
            RouteParams::from_pairs([("id".to_owned(), "42".to_owned())]),
        );
        let branch = matched_branch(vec![root, child]);

        assert_eq!(branch.routes().len(), 2);
        assert_eq!(
            branch.leaf().map(MatchedRoute::route),
            Some(RouteId::new(2))
        );
        assert!(!branch.is_empty());
    }
}
