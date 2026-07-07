use std::collections::BTreeMap;

use crate::ids::RouteId;

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
}

impl MatchedRoute {
    #[allow(dead_code)]
    pub(crate) fn new(route: RouteId, params: RouteParams) -> Self {
        Self { route, params }
    }

    pub const fn route(&self) -> RouteId {
        self.route
    }

    pub const fn params(&self) -> &RouteParams {
        &self.params
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MatchedBranch {
    routes: Vec<MatchedRoute>,
}

impl MatchedBranch {
    #[allow(dead_code)]
    pub(crate) fn new(routes: Vec<MatchedRoute>) -> Self {
        Self { routes }
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::RouteId;

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
        let root = MatchedRoute::new(RouteId::new(1), RouteParams::new());
        let child = MatchedRoute::new(
            RouteId::new(2),
            RouteParams::from_pairs([("id".to_owned(), "42".to_owned())]),
        );
        let branch = MatchedBranch::new(vec![root, child]);

        assert_eq!(branch.routes().len(), 2);
        assert_eq!(
            branch.leaf().map(MatchedRoute::route),
            Some(RouteId::new(2))
        );
        assert!(!branch.is_empty());
    }
}
