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
        self.visit(self.tree.root(), &segments, 0, Vec::new(), &mut candidates)?;
        select_candidate(candidates)
    }

    fn visit(
        &self,
        route: RouteId,
        segments: &[&str],
        cursor: usize,
        branch: Vec<MatchedRoute>,
        candidates: &mut Vec<MatchedBranch>,
    ) -> Result<(), RouteError> {
        let node = self
            .tree
            .node(route)
            .ok_or_else(|| internal_error("route tree references a missing node"))?;
        let Some((next_cursor, params)) = match_node(node, segments, cursor) else {
            return Ok(());
        };

        let mut next_branch = branch;
        next_branch.push(MatchedRoute::new(route, params));

        if next_cursor == segments.len() {
            candidates.push(MatchedBranch::new(next_branch.clone()));
        }

        for child in node.children() {
            self.visit(
                *child,
                segments,
                next_cursor,
                next_branch.clone(),
                candidates,
            )?;
        }

        Ok(())
    }
}

fn match_node(node: &RouteNode, segments: &[&str], cursor: usize) -> Option<(usize, RouteParams)> {
    match node.pattern().kind() {
        RoutePatternKind::Pathless => Some((cursor, RouteParams::new())),
        RoutePatternKind::Index => (cursor == segments.len()).then(|| (cursor, RouteParams::new())),
        RoutePatternKind::Path => match_path_segments(node, segments, cursor),
    }
}

fn match_path_segments(
    node: &RouteNode,
    segments: &[&str],
    cursor: usize,
) -> Option<(usize, RouteParams)> {
    let mut next_cursor = cursor;
    let mut params = Vec::new();

    for pattern_segment in node.pattern().segments() {
        match pattern_segment {
            RoutePathSegment::Static(expected) => {
                let actual = segments.get(next_cursor)?;
                if actual != expected {
                    return None;
                }
                next_cursor += 1;
            }
            RoutePathSegment::Param(name) => {
                let actual = segments.get(next_cursor)?;
                params.push((name.clone(), (*actual).to_owned()));
                next_cursor += 1;
            }
            RoutePathSegment::Wildcard(name) => {
                params.push((name.clone(), segments[next_cursor..].join("/")));
                next_cursor = segments.len();
                break;
            }
        }
    }

    Some((next_cursor, RouteParams::from_pairs(params)))
}

fn split_location_path(path: &str) -> Vec<&str> {
    path.split('/')
        .filter(|segment| !segment.is_empty())
        .collect()
}

fn select_candidate(mut candidates: Vec<MatchedBranch>) -> Result<MatchedBranch, RouteError> {
    if candidates.is_empty() {
        return Err(RouteError::new(
            RouteErrorKind::NotFound,
            RouteErrorSourceCategory::RouterCore,
            "route location did not match any route",
        ));
    }

    Ok(candidates.remove(0))
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
}
