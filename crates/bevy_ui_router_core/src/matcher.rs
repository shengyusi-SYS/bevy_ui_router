use std::cmp::Ordering;

use crate::branch::{MatchedBranch, MatchedRoute, RouteParams};
use crate::error::{RouteError, RouteErrorKind, RouteErrorSourceCategory};
use crate::ids::RouteId;
use crate::location::RouteLocation;
use crate::pattern::{RoutePathSegment, RoutePatternKind};
use crate::route_tree::{RouteNode, RouteTree};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouteMatcher {
    tree: RouteTree,
}

impl RouteMatcher {
    pub fn new(tree: RouteTree) -> Self {
        Self { tree }
    }

    pub fn match_location(&self, location: &RouteLocation) -> Result<MatchedBranch, RouteError> {
        let segments = split_location_path(location.path());
        let mut candidates = Vec::new();
        self.visit(
            self.tree.root(),
            &segments,
            0,
            Vec::new(),
            SpecificityKey::default(),
            &mut candidates,
        )?;
        select_candidate(candidates)
    }

    fn visit(
        &self,
        route: RouteId,
        segments: &[&str],
        cursor: usize,
        branch: Vec<MatchedRoute>,
        specificity: SpecificityKey,
        candidates: &mut Vec<Candidate>,
    ) -> Result<(), RouteError> {
        let node = self
            .tree
            .node(route)
            .ok_or_else(|| internal_error("route tree references a missing node"))?;
        let Some(matched) = match_node(node, segments, cursor) else {
            return Ok(());
        };

        let mut next_branch = branch;
        next_branch.push(MatchedRoute::new(route, matched.params));
        let next_specificity = specificity.add(matched.specificity);

        if matched.next_cursor == segments.len() {
            candidates.push(Candidate {
                branch: MatchedBranch::new(next_branch.clone()),
                specificity: next_specificity,
            });
        }

        for child in node.children() {
            self.visit(
                *child,
                segments,
                matched.next_cursor,
                next_branch.clone(),
                next_specificity,
                candidates,
            )?;
        }

        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Candidate {
    branch: MatchedBranch,
    specificity: SpecificityKey,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct SpecificityKey {
    consumed_segments: usize,
    static_segments: usize,
    param_segments: usize,
    wildcard_segments: usize,
    index_bonus: usize,
    pathless_segments: usize,
}

impl SpecificityKey {
    fn add(self, other: Self) -> Self {
        Self {
            consumed_segments: self.consumed_segments + other.consumed_segments,
            static_segments: self.static_segments + other.static_segments,
            param_segments: self.param_segments + other.param_segments,
            wildcard_segments: self.wildcard_segments + other.wildcard_segments,
            index_bonus: self.index_bonus + other.index_bonus,
            pathless_segments: self.pathless_segments + other.pathless_segments,
        }
    }

    fn preference_cmp(&self, other: &Self) -> Ordering {
        self.consumed_segments
            .cmp(&other.consumed_segments)
            .then_with(|| self.static_segments.cmp(&other.static_segments))
            .then_with(|| self.param_segments.cmp(&other.param_segments))
            .then_with(|| self.index_bonus.cmp(&other.index_bonus))
            .then_with(|| other.wildcard_segments.cmp(&self.wildcard_segments))
            .then_with(|| other.pathless_segments.cmp(&self.pathless_segments))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct NodeMatch {
    next_cursor: usize,
    params: RouteParams,
    specificity: SpecificityKey,
}

fn match_node(node: &RouteNode, segments: &[&str], cursor: usize) -> Option<NodeMatch> {
    match node.pattern().kind() {
        RoutePatternKind::Pathless => Some(NodeMatch {
            next_cursor: cursor,
            params: RouteParams::new(),
            specificity: SpecificityKey {
                pathless_segments: 1,
                ..SpecificityKey::default()
            },
        }),
        RoutePatternKind::Index => (cursor == segments.len()).then(|| NodeMatch {
            next_cursor: cursor,
            params: RouteParams::new(),
            specificity: SpecificityKey {
                index_bonus: 1,
                ..SpecificityKey::default()
            },
        }),
        RoutePatternKind::Path => match_path_segments(node, segments, cursor),
    }
}

fn match_path_segments(node: &RouteNode, segments: &[&str], cursor: usize) -> Option<NodeMatch> {
    let mut next_cursor = cursor;
    let mut params = Vec::new();
    let mut specificity = SpecificityKey::default();

    for pattern_segment in node.pattern().segments() {
        match pattern_segment {
            RoutePathSegment::Static(expected) => {
                let actual = segments.get(next_cursor)?;
                if actual != expected {
                    return None;
                }
                next_cursor += 1;
                specificity.static_segments += 1;
            }
            RoutePathSegment::Param(name) => {
                let actual = segments.get(next_cursor)?;
                params.push((name.clone(), (*actual).to_owned()));
                next_cursor += 1;
                specificity.param_segments += 1;
            }
            RoutePathSegment::Wildcard(name) => {
                params.push((name.clone(), segments[next_cursor..].join("/")));
                next_cursor = segments.len();
                specificity.wildcard_segments += 1;
                break;
            }
        }
    }

    specificity.consumed_segments = next_cursor - cursor;

    Some(NodeMatch {
        next_cursor,
        params: RouteParams::from_pairs(params),
        specificity,
    })
}

fn split_location_path(path: &str) -> Vec<&str> {
    path.split('/')
        .filter(|segment| !segment.is_empty())
        .collect()
}

fn select_candidate(mut candidates: Vec<Candidate>) -> Result<MatchedBranch, RouteError> {
    let Some(mut best) = candidates.pop() else {
        return Err(RouteError::new(
            RouteErrorKind::NotFound,
            RouteErrorSourceCategory::RouterCore,
            "route location did not match any route",
        ));
    };

    for candidate in candidates {
        match candidate.specificity.preference_cmp(&best.specificity) {
            Ordering::Greater => best = candidate,
            Ordering::Equal => {
                if candidate.branch.leaf().map(MatchedRoute::route)
                    != best.branch.leaf().map(MatchedRoute::route)
                {
                    return Err(RouteError::new(
                        RouteErrorKind::MatchConflict,
                        RouteErrorSourceCategory::RouterCore,
                        "ambiguous route match",
                    ));
                }
            }
            Ordering::Less => {}
        }
    }

    Ok(best.branch)
}

fn internal_error(message: &'static str) -> RouteError {
    RouteError::new(
        RouteErrorKind::Internal,
        RouteErrorSourceCategory::RouterCore,
        message,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::branch::MatchedRoute;
    use crate::error::{RouteErrorKind, RouteErrorSourceCategory};
    use crate::ids::RouteId;
    use crate::location::RouteLocation;
    use crate::pattern::RoutePattern;
    use crate::route_tree::RouteTree;

    fn sample_tree() -> (RouteTree, RouteId, RouteId, RouteId, RouteId, RouteId) {
        let mut builder = RouteTree::builder()
            .root(RoutePattern::path("/").unwrap())
            .unwrap();
        let root = builder.root_id().unwrap();
        let layout = builder.route(root, RoutePattern::pathless()).unwrap();
        let projects = builder
            .route(layout, RoutePattern::path("projects").unwrap())
            .unwrap();
        let project = builder
            .route(projects, RoutePattern::path(":project_id").unwrap())
            .unwrap();
        let files = builder
            .route(project, RoutePattern::path("files/*rest").unwrap())
            .unwrap();
        (
            builder.build().unwrap(),
            root,
            layout,
            projects,
            project,
            files,
        )
    }

    #[test]
    fn matcher_returns_ordered_branch_with_params() {
        let (tree, root, layout, projects, project, files) = sample_tree();
        let branch = RouteMatcher::new(tree)
            .match_location(&RouteLocation::new("/projects/alpha/files/src/lib.rs"))
            .unwrap();

        assert_eq!(
            branch
                .routes()
                .iter()
                .map(MatchedRoute::route)
                .collect::<Vec<_>>(),
            vec![root, layout, projects, project, files]
        );
        let project_params = branch.routes()[3].params();
        let leaf_params = branch.leaf().unwrap().params();
        assert_eq!(project_params.get("project_id"), Some("alpha"));
        assert_eq!(leaf_params.get("project_id"), None);
        assert_eq!(leaf_params.get("rest"), Some("src/lib.rs"));
    }

    #[test]
    fn static_routes_outrank_param_and_param_outranks_wildcard() {
        let mut builder = RouteTree::builder()
            .root(RoutePattern::path("/").unwrap())
            .unwrap();
        let root = builder.root_id().unwrap();
        let wildcard = builder
            .route(root, RoutePattern::path("settings/*rest").unwrap())
            .unwrap();
        let param = builder
            .route(root, RoutePattern::path("settings/:section").unwrap())
            .unwrap();
        let exact = builder
            .route(root, RoutePattern::path("settings/profile").unwrap())
            .unwrap();
        let matcher = RouteMatcher::new(builder.build().unwrap());

        assert_eq!(
            matcher
                .match_location(&RouteLocation::new("/settings/profile"))
                .unwrap()
                .leaf()
                .map(MatchedRoute::route),
            Some(exact)
        );
        assert_eq!(
            matcher
                .match_location(&RouteLocation::new("/settings/security"))
                .unwrap()
                .leaf()
                .map(MatchedRoute::route),
            Some(param)
        );
        assert_eq!(
            matcher
                .match_location(&RouteLocation::new("/settings/security/advanced"))
                .unwrap()
                .leaf()
                .map(MatchedRoute::route),
            Some(wildcard)
        );
    }

    #[test]
    fn index_route_matches_only_when_parent_consumed_the_path() {
        let mut builder = RouteTree::builder()
            .root(RoutePattern::path("/").unwrap())
            .unwrap();
        let root = builder.root_id().unwrap();
        let settings = builder
            .route(root, RoutePattern::path("settings").unwrap())
            .unwrap();
        let pathless = builder.route(settings, RoutePattern::pathless()).unwrap();
        let index = builder.route(settings, RoutePattern::index()).unwrap();
        let matcher = RouteMatcher::new(builder.build().unwrap());

        assert_eq!(
            matcher
                .match_location(&RouteLocation::new("/settings"))
                .unwrap()
                .leaf()
                .map(MatchedRoute::route),
            Some(index)
        );
        assert_ne!(pathless, index);
    }

    #[test]
    fn matcher_returns_not_found_when_no_candidate_consumes_location() {
        let mut builder = RouteTree::builder()
            .root(RoutePattern::path("/").unwrap())
            .unwrap();
        let root = builder.root_id().unwrap();
        builder
            .route(root, RoutePattern::path("settings").unwrap())
            .unwrap();
        let matcher = RouteMatcher::new(builder.build().unwrap());

        let error = matcher
            .match_location(&RouteLocation::new("/missing"))
            .unwrap_err();

        assert_eq!(error.kind(), RouteErrorKind::NotFound);
        assert_eq!(
            error.source_category(),
            RouteErrorSourceCategory::RouterCore
        );
    }

    #[test]
    fn equally_specific_full_matches_return_conflict() {
        let mut builder = RouteTree::builder()
            .root(RoutePattern::path("/").unwrap())
            .unwrap();
        let root = builder.root_id().unwrap();
        let first = builder
            .route(root, RoutePattern::path("projects/:id").unwrap())
            .unwrap();
        let second = builder
            .route(root, RoutePattern::path("projects/:slug").unwrap())
            .unwrap();
        let matcher = RouteMatcher::new(builder.build().unwrap());

        let error = matcher
            .match_location(&RouteLocation::new("/projects/alpha"))
            .unwrap_err();

        assert_ne!(first, second);
        assert_eq!(error.kind(), RouteErrorKind::MatchConflict);
        assert_eq!(
            error.source_category(),
            RouteErrorSourceCategory::RouterCore
        );
    }
}
