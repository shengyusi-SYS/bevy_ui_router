use std::cmp::Ordering;

use crate::branch::{FrozenErrorViewBinding, MatchedBranch, MatchedRoute, RouteParams};
use crate::error::{RouteError, RouteErrorKind, RouteErrorSourceCategory};
use crate::fingerprint::{
    RouteBranchFingerprint, RouteParamsFingerprint, RouteSearchFingerprint, RouteSegmentReuseKey,
};
use crate::ids::RouteId;
use crate::location::RouteLocation;
use crate::pattern::{RoutePathSegment, RoutePatternKind};
use crate::route_tree::{RouteNode, RouteTree};

#[derive(Clone, Debug)]
pub struct RouteMatcher {
    tree: RouteTree,
}

impl RouteMatcher {
    pub fn new(tree: RouteTree) -> Self {
        Self { tree }
    }

    pub fn match_location(&self, location: &RouteLocation) -> Result<MatchedBranch, RouteError> {
        let segments = split_location_path(location.path());
        let search_fingerprint = RouteSearchFingerprint::from_location(location);
        let mut candidates = Vec::new();
        self.visit(
            self.tree.root(),
            &segments,
            0,
            &[],
            None,
            SpecificityKey::default(),
            &search_fingerprint,
            &mut candidates,
        )?;
        select_candidate(candidates)
    }

    pub(crate) fn view_binding(&self, route: RouteId) -> Option<&crate::view_key::RouteViewKey> {
        self.tree.node(route)?.options().view_binding()
    }

    pub(crate) fn keep_alive(&self, route: RouteId) -> bool {
        self.tree
            .node(route)
            .is_some_and(|node| node.options().keep_alive())
    }

    pub(crate) fn max_branch_depth(&self) -> usize {
        self.tree.max_branch_depth()
    }

    fn visit(
        &self,
        route: RouteId,
        segments: &[&str],
        cursor: usize,
        branch: &[MatchedRoute],
        nearest_error_view: Option<FrozenErrorViewBinding>,
        specificity: SpecificityKey,
        search_fingerprint: &RouteSearchFingerprint,
        candidates: &mut Vec<Candidate>,
    ) -> Result<(), RouteError> {
        let node = self
            .tree
            .node(route)
            .ok_or_else(|| internal_error("route tree references a missing node"))?;
        let Some(matched) = match_node(node, segments, cursor) else {
            return Ok(());
        };

        let options = node.options();
        let params_fingerprint = RouteParamsFingerprint::from_route_params(route, &matched.params);
        let segment_reuse_key = RouteSegmentReuseKey::from_parts(
            route,
            options.slot(),
            options.explicit_key(),
            &params_fingerprint,
        );
        let branch_index = branch.len();
        let nearest_error_view =
            options
                .error_view_binding()
                .map_or(nearest_error_view, |binding| {
                    Some(FrozenErrorViewBinding::new(
                        route,
                        branch_index,
                        binding.0.clone(),
                        binding.1.clone(),
                        options.slot().clone(),
                        self.tree.provenance().clone(),
                    ))
                });
        let mut next_branch = branch.to_vec();
        next_branch.push(
            MatchedRoute::new(
                route,
                matched.params,
                options.slot().clone(),
                options.explicit_key().cloned(),
                options.error_boundary().cloned(),
                options.meta().clone(),
                params_fingerprint,
                segment_reuse_key,
            )
            .with_nearest_error_view(nearest_error_view.clone())
            .with_preserve_on_search(options.preserve_on_search()),
        );
        let next_specificity = specificity.add(matched.specificity);

        if matched.next_cursor == segments.len() {
            let branch_fingerprint = RouteBranchFingerprint::from_parts(
                next_branch.iter().map(MatchedRoute::segment_reuse_key),
                search_fingerprint,
            );
            candidates.push(Candidate {
                branch: MatchedBranch::new(
                    next_branch.clone(),
                    self.tree.provenance().clone(),
                    search_fingerprint.clone(),
                    branch_fingerprint,
                ),
                specificity: next_specificity,
            });
        }

        for child in node.children() {
            self.visit(
                *child,
                segments,
                matched.next_cursor,
                &next_branch,
                nearest_error_view.clone(),
                next_specificity,
                search_fingerprint,
                candidates,
            )?;
        }

        Ok(())
    }
}

#[derive(Clone, Debug)]
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

fn select_candidate(candidates: Vec<Candidate>) -> Result<MatchedBranch, RouteError> {
    let Some(best_key) = candidates
        .iter()
        .map(|candidate| candidate.specificity)
        .max_by(|left, right| left.preference_cmp(right))
    else {
        return Err(RouteError::not_found("no route matched requested location"));
    };

    let mut best_candidates = candidates
        .into_iter()
        .filter(|candidate| candidate.specificity.preference_cmp(&best_key) == Ordering::Equal);
    let best = best_candidates
        .next()
        .expect("best key came from at least one candidate");
    let best_leaf = best.branch.leaf().map(MatchedRoute::route);

    for candidate in best_candidates {
        if candidate.branch.leaf().map(MatchedRoute::route) != best_leaf {
            return Err(RouteError::match_conflict("ambiguous route match"));
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
    use crate::branch::{MatchedBranch, MatchedRoute};
    use crate::error::{RouteErrorKind, RouteErrorSourceCategory};
    use crate::ids::RouteId;
    use crate::location::RouteLocation;
    use crate::meta::{ErrorBoundaryKey, RouteExplicitKey, RouteMeta};
    use crate::pattern::RoutePattern;
    use crate::route_tree::{RouteNodeOptions, RouteTree};
    use crate::slot::RouteSlotKey;

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
    fn back_reserves_installed_maximum_branch_depth() {
        let mut builder = RouteTree::builder()
            .root(RoutePattern::path("/").unwrap())
            .unwrap();
        let root = builder.root_id().unwrap();
        builder
            .route(root, RoutePattern::path("short").unwrap())
            .unwrap();
        let deep = builder
            .route(root, RoutePattern::path("deep").unwrap())
            .unwrap();
        builder
            .route(deep, RoutePattern::path("leaf").unwrap())
            .unwrap();
        let matcher = RouteMatcher::new(builder.build().unwrap());

        assert_eq!(
            matcher.max_branch_depth(),
            3,
            "back must reserve the installed immutable maximum before target resolution"
        );
        assert_eq!(
            matcher
                .match_location(&RouteLocation::new("/short"))
                .unwrap()
                .routes()
                .len(),
            2,
            "later matching consumes only the resolved target prefix"
        );
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

    #[test]
    fn higher_specificity_candidate_wins_before_conflict_detection() {
        let mut builder = RouteTree::builder()
            .root(RoutePattern::path("/").unwrap())
            .unwrap();
        let root = builder.root_id().unwrap();
        let _first_dynamic = builder
            .route(root, RoutePattern::path("settings/:id").unwrap())
            .unwrap();
        let exact = builder
            .route(root, RoutePattern::path("settings/profile").unwrap())
            .unwrap();
        let _second_dynamic = builder
            .route(root, RoutePattern::path("settings/:slug").unwrap())
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
    }

    #[test]
    fn matched_branch_carries_slot_metadata_boundaries_and_fingerprints() {
        let mut builder = RouteTree::builder()
            .root(RoutePattern::path("/").unwrap())
            .unwrap();
        let root = builder.root_id().unwrap();
        let layout_options = RouteNodeOptions::new()
            .with_error_boundary(ErrorBoundaryKey::named("root-boundary").unwrap())
            .with_meta(RouteMeta::new().with_diagnostic_label("layout").unwrap());
        let layout = builder
            .route_with_options(root, RoutePattern::pathless(), layout_options)
            .unwrap();
        let detail_options = RouteNodeOptions::new()
            .with_slot(RouteSlotKey::named("details").unwrap())
            .with_explicit_key(RouteExplicitKey::new("detail-v1").unwrap())
            .with_error_boundary(ErrorBoundaryKey::named("detail-boundary").unwrap())
            .with_meta(
                RouteMeta::new()
                    .with_diagnostic_label("detail-label")
                    .unwrap(),
            );
        builder
            .route_with_options(
                layout,
                RoutePattern::path("items/:id").unwrap(),
                detail_options,
            )
            .unwrap();

        let branch = RouteMatcher::new(builder.build().unwrap())
            .match_location(&RouteLocation::new("/items/42").with_search("tab=files"))
            .unwrap();
        let detail = branch.leaf().unwrap();

        assert_eq!(detail.slot().as_str(), "details");
        assert_eq!(
            detail.explicit_key().map(RouteExplicitKey::as_str),
            Some("detail-v1")
        );
        assert_eq!(detail.meta().diagnostic_label(), Some("detail-label"));
        assert_eq!(
            branch
                .nearest_error_boundary_for(detail.route())
                .map(ErrorBoundaryKey::as_str),
            Some("detail-boundary")
        );
        assert!(
            detail
                .params_fingerprint()
                .as_str()
                .contains("param:2:id=2:42")
        );
        assert!(branch.search_fingerprint().as_str().contains("tab=files"));
        assert!(detail.segment_reuse_key().as_str().contains("detail-v1"));
        assert!(!detail.segment_reuse_key().as_str().contains("detail-label"));
        assert!(
            !detail
                .segment_reuse_key()
                .as_str()
                .contains("detail-boundary")
        );
        assert!(
            !branch
                .branch_fingerprint()
                .as_str()
                .contains("detail-label")
        );
        assert!(
            !branch
                .branch_fingerprint()
                .as_str()
                .contains("detail-boundary")
        );
    }

    #[test]
    fn matched_branch_fingerprints_do_not_merge_ancestor_params() {
        let mut builder = RouteTree::builder()
            .root(RoutePattern::path("/").unwrap())
            .unwrap();
        let root = builder.root_id().unwrap();
        let project = builder
            .route(root, RoutePattern::path("projects/:project_id").unwrap())
            .unwrap();
        builder
            .route(project, RoutePattern::path("files/:file_id").unwrap())
            .unwrap();

        let branch = RouteMatcher::new(builder.build().unwrap())
            .match_location(&RouteLocation::new("/projects/alpha/files/readme"))
            .unwrap();
        let project_route = &branch.routes()[1];
        let leaf = branch.leaf().unwrap();

        assert_eq!(project_route.params().get("project_id"), Some("alpha"));
        assert_eq!(leaf.params().get("project_id"), None);
        assert!(
            project_route
                .params_fingerprint()
                .as_str()
                .contains("param:10:project_id=5:alpha")
        );
        assert!(
            leaf.params_fingerprint()
                .as_str()
                .contains("param:7:file_id=6:readme")
        );
        assert!(!leaf.params_fingerprint().as_str().contains("project_id"));
    }

    fn branch_with_metadata(label: &str, boundary: &str) -> MatchedBranch {
        let mut builder = RouteTree::builder()
            .root(RoutePattern::path("/").unwrap())
            .unwrap();
        let root = builder.root_id().unwrap();
        let options = RouteNodeOptions::new()
            .with_slot(RouteSlotKey::named("details").unwrap())
            .with_explicit_key(RouteExplicitKey::new("detail-v1").unwrap())
            .with_error_boundary(ErrorBoundaryKey::named(boundary).unwrap())
            .with_meta(RouteMeta::new().with_diagnostic_label(label).unwrap());
        builder
            .route_with_options(root, RoutePattern::path("items/:id").unwrap(), options)
            .unwrap();

        RouteMatcher::new(builder.build().unwrap())
            .match_location(&RouteLocation::new("/items/42").with_search("tab=files"))
            .unwrap()
    }

    #[test]
    fn metadata_and_error_boundaries_do_not_change_reuse_or_branch_fingerprints() {
        let first = branch_with_metadata("alpha-label", "alpha-boundary");
        let second = branch_with_metadata("beta-label", "beta-boundary");

        assert!(first.provenance() != second.provenance());
        assert_eq!(
            first.leaf().unwrap().segment_reuse_key(),
            second.leaf().unwrap().segment_reuse_key()
        );
        assert_eq!(first.branch_fingerprint(), second.branch_fingerprint());
    }

    #[test]
    fn nearest_error_boundary_uses_matched_ancestor_chain_only() {
        let mut builder = RouteTree::builder()
            .root(RoutePattern::path("/").unwrap())
            .unwrap();
        let root = builder.root_id().unwrap();
        let root_layout = builder
            .route_with_options(
                root,
                RoutePattern::pathless(),
                RouteNodeOptions::new()
                    .with_error_boundary(ErrorBoundaryKey::named("root").unwrap()),
            )
            .unwrap();
        let section = builder
            .route_with_options(
                root_layout,
                RoutePattern::path("settings").unwrap(),
                RouteNodeOptions::new()
                    .with_error_boundary(ErrorBoundaryKey::named("settings").unwrap()),
            )
            .unwrap();
        let leaf = builder
            .route(section, RoutePattern::path("profile").unwrap())
            .unwrap();
        let billing = builder
            .route_with_options(
                root_layout,
                RoutePattern::path("billing").unwrap(),
                RouteNodeOptions::new()
                    .with_error_boundary(ErrorBoundaryKey::named("billing").unwrap()),
            )
            .unwrap();

        let branch = RouteMatcher::new(builder.build().unwrap())
            .match_location(&RouteLocation::new("/settings/profile"))
            .unwrap();

        assert_eq!(
            branch
                .nearest_error_boundary_for(leaf)
                .map(ErrorBoundaryKey::as_str),
            Some("settings")
        );
        assert_eq!(
            branch
                .nearest_error_boundary_for(root_layout)
                .map(ErrorBoundaryKey::as_str),
            Some("root")
        );
        assert_eq!(branch.nearest_error_boundary_for(billing), None);
        assert_eq!(branch.nearest_error_boundary_for(RouteId::new(999)), None);
    }
}
