//! ```compile_fail
//! use bevy_ui_router_core::prelude::BranchDiffRoute;
//!
//! let _ = core::mem::size_of::<BranchDiffRoute>();
//! ```

use crate::branch::{MatchedBranch, MatchedRoute};
use crate::ids::RouteId;
use crate::slot::RouteSlotKey;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BranchDiffRoute {
    route: RouteId,
    index: usize,
    slot: RouteSlotKey,
}

impl BranchDiffRoute {
    fn from_matched(index: usize, route: &MatchedRoute) -> Self {
        Self {
            route: route.route(),
            index,
            slot: route.slot().clone(),
        }
    }

    pub const fn route(&self) -> RouteId {
        self.route
    }

    pub const fn index(&self) -> usize {
        self.index
    }

    pub fn slot(&self) -> &RouteSlotKey {
        &self.slot
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BranchDiff {
    common_prefix_len: usize,
    leaving: Vec<BranchDiffRoute>,
    entering: Vec<BranchDiffRoute>,
    provenance_changed: bool,
    search_changed: bool,
    branch_fingerprint_changed: bool,
}

impl BranchDiff {
    pub fn compare(old: &MatchedBranch, new: &MatchedBranch) -> Self {
        let provenance_changed = old.provenance() != new.provenance();
        let search_changed = old.search_fingerprint() != new.search_fingerprint();
        let common_prefix_len = if provenance_changed || search_changed {
            0
        } else {
            old.routes()
                .iter()
                .zip(new.routes().iter())
                .take_while(|(old_route, new_route)| reusable_segment(old_route, new_route))
                .count()
        };

        let leaving = old.routes()[common_prefix_len..]
            .iter()
            .enumerate()
            .rev()
            .map(|(offset, route)| BranchDiffRoute::from_matched(common_prefix_len + offset, route))
            .collect();
        let entering = new.routes()[common_prefix_len..]
            .iter()
            .enumerate()
            .map(|(offset, route)| BranchDiffRoute::from_matched(common_prefix_len + offset, route))
            .collect();

        Self {
            common_prefix_len,
            leaving,
            entering,
            provenance_changed,
            search_changed,
            branch_fingerprint_changed: old.branch_fingerprint() != new.branch_fingerprint(),
        }
    }

    pub const fn common_prefix_len(&self) -> usize {
        self.common_prefix_len
    }

    pub fn leaving(&self) -> &[BranchDiffRoute] {
        &self.leaving
    }

    pub fn entering(&self) -> &[BranchDiffRoute] {
        &self.entering
    }

    pub const fn provenance_changed(&self) -> bool {
        self.provenance_changed
    }

    pub const fn search_changed(&self) -> bool {
        self.search_changed
    }

    pub const fn branch_fingerprint_changed(&self) -> bool {
        self.branch_fingerprint_changed
    }
}

fn reusable_segment(old: &MatchedRoute, new: &MatchedRoute) -> bool {
    old.route() == new.route()
        && old.slot() == new.slot()
        && old.explicit_key() == new.explicit_key()
        && old.segment_reuse_key() == new.segment_reuse_key()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::location::RouteLocation;
    use crate::matcher::RouteMatcher;
    use crate::meta::RouteExplicitKey;
    use crate::pattern::RoutePattern;
    use crate::route_tree::{RouteNodeOptions, RouteTree};
    use crate::slot::RouteSlotKey;

    fn sample_items_matcher() -> RouteMatcher {
        let mut builder = RouteTree::builder()
            .root(RoutePattern::path("/").unwrap())
            .unwrap();
        let root = builder.root_id().unwrap();
        let layout = builder
            .route(root, RoutePattern::path("items").unwrap())
            .unwrap();
        builder
            .route(layout, RoutePattern::path(":id").unwrap())
            .unwrap();
        RouteMatcher::new(builder.build().unwrap())
    }

    #[test]
    fn diff_preserves_common_prefix_and_orders_suffixes() {
        let matcher = sample_items_matcher();
        let old = matcher
            .match_location(&RouteLocation::new("/items/1"))
            .unwrap();
        let new = matcher
            .match_location(&RouteLocation::new("/items/2"))
            .unwrap();
        let diff = BranchDiff::compare(&old, &new);

        assert_eq!(diff.common_prefix_len(), 2);
        assert_eq!(diff.leaving().len(), 1);
        assert_eq!(diff.entering().len(), 1);
        assert!(diff.branch_fingerprint_changed());
    }

    #[test]
    fn sibling_route_change_preserves_only_shared_parent() {
        let mut builder = RouteTree::builder()
            .root(RoutePattern::path("/").unwrap())
            .unwrap();
        let root = builder.root_id().unwrap();
        builder
            .route(root, RoutePattern::path("items").unwrap())
            .unwrap();
        builder
            .route(root, RoutePattern::path("settings").unwrap())
            .unwrap();
        let matcher = RouteMatcher::new(builder.build().unwrap());

        let old = matcher
            .match_location(&RouteLocation::new("/items"))
            .unwrap();
        let new = matcher
            .match_location(&RouteLocation::new("/settings"))
            .unwrap();
        let diff = BranchDiff::compare(&old, &new);

        assert_eq!(diff.common_prefix_len(), 1);
        assert_eq!(diff.leaving().len(), 1);
        assert_eq!(diff.entering().len(), 1);
    }

    #[test]
    fn child_route_change_enters_new_suffix_from_root_to_leaf() {
        let mut builder = RouteTree::builder()
            .root(RoutePattern::path("/").unwrap())
            .unwrap();
        let root = builder.root_id().unwrap();
        let items = builder
            .route(root, RoutePattern::path("items").unwrap())
            .unwrap();
        builder
            .route(items, RoutePattern::path(":id").unwrap())
            .unwrap();
        let matcher = RouteMatcher::new(builder.build().unwrap());

        let old = matcher
            .match_location(&RouteLocation::new("/items"))
            .unwrap();
        let new = matcher
            .match_location(&RouteLocation::new("/items/42"))
            .unwrap();
        let diff = BranchDiff::compare(&old, &new);

        assert_eq!(diff.common_prefix_len(), 2);
        assert!(diff.leaving().is_empty());
        assert_eq!(diff.entering().len(), 1);
        assert_eq!(diff.entering()[0].index(), 2);
    }

    #[test]
    fn layout_shape_change_uses_provenance_fail_closed() {
        let mut old_builder = RouteTree::builder()
            .root(RoutePattern::path("/").unwrap())
            .unwrap();
        let old_root = old_builder.root_id().unwrap();
        old_builder
            .route(old_root, RoutePattern::path("items").unwrap())
            .unwrap();
        let old = RouteMatcher::new(old_builder.build().unwrap())
            .match_location(&RouteLocation::new("/items"))
            .unwrap();

        let mut new_builder = RouteTree::builder()
            .root(RoutePattern::path("/").unwrap())
            .unwrap();
        let new_root = new_builder.root_id().unwrap();
        let layout = new_builder
            .route(new_root, RoutePattern::pathless())
            .unwrap();
        new_builder
            .route(layout, RoutePattern::path("items").unwrap())
            .unwrap();
        let new = RouteMatcher::new(new_builder.build().unwrap())
            .match_location(&RouteLocation::new("/items"))
            .unwrap();

        let diff = BranchDiff::compare(&old, &new);
        assert!(diff.provenance_changed());
        assert_eq!(diff.common_prefix_len(), 0);
    }

    #[test]
    fn different_tree_provenance_replaces_entire_branch_even_when_route_ids_match() {
        let mut old_builder = RouteTree::builder()
            .root(RoutePattern::path("/").unwrap())
            .unwrap();
        let old_root = old_builder.root_id().unwrap();
        old_builder
            .route(old_root, RoutePattern::path("panel").unwrap())
            .unwrap();
        let old = RouteMatcher::new(old_builder.build().unwrap())
            .match_location(&RouteLocation::new("/panel"))
            .unwrap();

        let mut new_builder = RouteTree::builder()
            .root(RoutePattern::path("/").unwrap())
            .unwrap();
        let new_root = new_builder.root_id().unwrap();
        let options = RouteNodeOptions::new().with_slot(RouteSlotKey::named("side").unwrap());
        new_builder
            .route_with_options(new_root, RoutePattern::path("panel").unwrap(), options)
            .unwrap();
        let new = RouteMatcher::new(new_builder.build().unwrap())
            .match_location(&RouteLocation::new("/panel"))
            .unwrap();

        let diff = BranchDiff::compare(&old, &new);
        assert!(diff.provenance_changed());
        assert_eq!(diff.common_prefix_len(), 0);
        assert_eq!(diff.leaving().len(), old.routes().len());
        assert_eq!(diff.entering().len(), new.routes().len());
    }

    #[test]
    fn explicit_key_change_in_different_tree_also_fails_closed() {
        let mut old_builder = RouteTree::builder()
            .root(RoutePattern::path("/").unwrap())
            .unwrap();
        let old_root = old_builder.root_id().unwrap();
        let old_options =
            RouteNodeOptions::new().with_explicit_key(RouteExplicitKey::new("v1").unwrap());
        old_builder
            .route_with_options(old_root, RoutePattern::path("panel").unwrap(), old_options)
            .unwrap();
        let old = RouteMatcher::new(old_builder.build().unwrap())
            .match_location(&RouteLocation::new("/panel"))
            .unwrap();

        let mut new_builder = RouteTree::builder()
            .root(RoutePattern::path("/").unwrap())
            .unwrap();
        let new_root = new_builder.root_id().unwrap();
        let new_options =
            RouteNodeOptions::new().with_explicit_key(RouteExplicitKey::new("v2").unwrap());
        new_builder
            .route_with_options(new_root, RoutePattern::path("panel").unwrap(), new_options)
            .unwrap();
        let new = RouteMatcher::new(new_builder.build().unwrap())
            .match_location(&RouteLocation::new("/panel"))
            .unwrap();

        let diff = BranchDiff::compare(&old, &new);
        assert!(diff.provenance_changed());
        assert_eq!(diff.common_prefix_len(), 0);
    }

    #[test]
    fn search_change_fails_closed_by_raw_search_policy() {
        let mut builder = RouteTree::builder()
            .root(RoutePattern::path("/").unwrap())
            .unwrap();
        let root = builder.root_id().unwrap();
        builder
            .route(root, RoutePattern::path("items").unwrap())
            .unwrap();
        let tree = builder.build().unwrap();
        let matcher = RouteMatcher::new(tree);

        let first = matcher
            .match_location(&RouteLocation::new("/items").with_search("a=1&a=2"))
            .unwrap();
        let second = matcher
            .match_location(&RouteLocation::new("/items").with_search("a=2&a=1"))
            .unwrap();
        let diff = BranchDiff::compare(&first, &second);

        assert!(diff.search_changed());
        assert!(diff.branch_fingerprint_changed());
        assert_eq!(diff.common_prefix_len(), 0);
        assert_eq!(diff.leaving().len(), first.routes().len());
        assert_eq!(diff.entering().len(), second.routes().len());
    }

    #[test]
    fn fragment_only_change_does_not_change_search_or_branch_diff() {
        let mut builder = RouteTree::builder()
            .root(RoutePattern::path("/").unwrap())
            .unwrap();
        let root = builder.root_id().unwrap();
        builder
            .route(root, RoutePattern::path("items").unwrap())
            .unwrap();
        let matcher = RouteMatcher::new(builder.build().unwrap());

        let first = matcher
            .match_location(&RouteLocation::new("/items").with_search("a=1"))
            .unwrap();
        let second = matcher
            .match_location(
                &RouteLocation::new("/items")
                    .with_search("a=1")
                    .with_fragment("ignored"),
            )
            .unwrap();
        let diff = BranchDiff::compare(&first, &second);

        assert_eq!(first.search_fingerprint(), second.search_fingerprint());
        assert_eq!(first.branch_fingerprint(), second.branch_fingerprint());
        assert!(!diff.search_changed());
        assert!(!diff.branch_fingerprint_changed());
        assert_eq!(diff.common_prefix_len(), first.routes().len());
        assert!(diff.leaving().is_empty());
        assert!(diff.entering().is_empty());
    }

    #[test]
    fn identical_independent_trees_fail_closed_by_provenance() {
        let old_matcher = sample_items_matcher();
        let new_matcher = sample_items_matcher();
        let old = old_matcher
            .match_location(&RouteLocation::new("/items/42"))
            .unwrap();
        let new = new_matcher
            .match_location(&RouteLocation::new("/items/42"))
            .unwrap();

        let diff = BranchDiff::compare(&old, &new);

        assert!(diff.provenance_changed());
        assert!(old.provenance() != new.provenance());
        assert_eq!(old.branch_fingerprint(), new.branch_fingerprint());
        assert_eq!(
            old.leaf().unwrap().segment_reuse_key(),
            new.leaf().unwrap().segment_reuse_key()
        );
        assert_eq!(diff.common_prefix_len(), 0);
        assert_eq!(diff.leaving().len(), old.routes().len());
        assert_eq!(diff.entering().len(), new.routes().len());
    }

    #[test]
    fn multi_level_suffix_orders_leaving_leaf_to_root_and_entering_root_to_leaf() {
        let mut builder = RouteTree::builder()
            .root(RoutePattern::path("/").unwrap())
            .unwrap();
        let root = builder.root_id().unwrap();
        let items = builder
            .route(root, RoutePattern::path("items").unwrap())
            .unwrap();
        let id = builder
            .route(items, RoutePattern::path(":id").unwrap())
            .unwrap();
        let details = builder
            .route(id, RoutePattern::path("details").unwrap())
            .unwrap();
        builder
            .route(details, RoutePattern::path(":tab").unwrap())
            .unwrap();
        let summary = builder
            .route(id, RoutePattern::path("summary").unwrap())
            .unwrap();
        builder
            .route(summary, RoutePattern::path(":tab").unwrap())
            .unwrap();
        let matcher = RouteMatcher::new(builder.build().unwrap());

        let old = matcher
            .match_location(&RouteLocation::new("/items/1/details/files"))
            .unwrap();
        let new = matcher
            .match_location(&RouteLocation::new("/items/2/summary/activity"))
            .unwrap();
        let diff = BranchDiff::compare(&old, &new);

        assert_eq!(diff.common_prefix_len(), 2);
        let leaving_indices: Vec<_> = diff.leaving().iter().map(BranchDiffRoute::index).collect();
        let entering_indices: Vec<_> = diff.entering().iter().map(BranchDiffRoute::index).collect();
        assert_eq!(leaving_indices, vec![4, 3, 2]);
        assert_eq!(entering_indices, vec![2, 3, 4]);
    }
}
