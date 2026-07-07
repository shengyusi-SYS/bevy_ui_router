//! ```compile_fail
//! use bevy_ui_router_core::prelude::RouteId;
//!
//! let _ = RouteId::new(99);
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_core::route_tree::RouteNode;
//!
//! fn requires_partial_eq<T: PartialEq>() {}
//!
//! requires_partial_eq::<RouteNode>();
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_core::route_tree::RouteTree;
//!
//! fn requires_partial_eq<T: PartialEq>() {}
//!
//! requires_partial_eq::<RouteTree>();
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_core::route_tree::RouteTreeBuilder;
//!
//! fn requires_partial_eq<T: PartialEq>() {}
//!
//! requires_partial_eq::<RouteTreeBuilder>();
//! ```

use std::{collections::BTreeMap, fmt};

use crate::error::RouteError;
use crate::fingerprint::RouteBranchProvenance;
use crate::ids::RouteId;
use crate::meta::{ErrorBoundaryKey, RouteExplicitKey, RouteMeta};
use crate::pattern::RoutePattern;
use crate::slot::RouteSlotKey;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouteNodeOptions {
    slot: RouteSlotKey,
    explicit_key: Option<RouteExplicitKey>,
    error_boundary: Option<ErrorBoundaryKey>,
    meta: RouteMeta,
}

impl RouteNodeOptions {
    pub fn new() -> Self {
        Self {
            slot: RouteSlotKey::default(),
            explicit_key: None,
            error_boundary: None,
            meta: RouteMeta::new(),
        }
    }

    pub fn with_slot(mut self, slot: RouteSlotKey) -> Self {
        self.slot = slot;
        self
    }

    pub fn with_explicit_key(mut self, key: RouteExplicitKey) -> Self {
        self.explicit_key = Some(key);
        self
    }

    pub fn with_error_boundary(mut self, key: ErrorBoundaryKey) -> Self {
        self.error_boundary = Some(key);
        self
    }

    pub fn with_meta(mut self, meta: RouteMeta) -> Self {
        self.meta = meta;
        self
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
}

impl Default for RouteNodeOptions {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
pub struct RouteNode {
    id: RouteId,
    parent: Option<RouteId>,
    pattern: RoutePattern,
    options: RouteNodeOptions,
    declaration_index: usize,
    children: Vec<RouteId>,
}

impl fmt::Debug for RouteNode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RouteNode")
            .field("id", &self.id)
            .field("parent", &self.parent)
            .field("pattern", &self.pattern)
            .finish()
    }
}

impl RouteNode {
    pub const fn id(&self) -> RouteId {
        self.id
    }

    pub const fn parent(&self) -> Option<RouteId> {
        self.parent
    }

    pub const fn pattern(&self) -> &RoutePattern {
        &self.pattern
    }

    pub const fn options(&self) -> &RouteNodeOptions {
        &self.options
    }

    #[allow(dead_code)]
    pub(crate) const fn declaration_index(&self) -> usize {
        self.declaration_index
    }

    #[allow(dead_code)]
    pub(crate) fn children(&self) -> &[RouteId] {
        &self.children
    }
}

#[derive(Clone)]
pub struct RouteTree {
    root: RouteId,
    nodes: BTreeMap<RouteId, RouteNode>,
    provenance: RouteBranchProvenance,
}

impl fmt::Debug for RouteTree {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RouteTree")
            .field("root", &self.root)
            .field("route_count", &self.nodes.len())
            .finish()
    }
}

impl RouteTree {
    pub fn builder() -> RouteTreeBuilder {
        RouteTreeBuilder::new()
    }

    pub const fn root(&self) -> RouteId {
        self.root
    }

    pub fn node(&self, id: RouteId) -> Option<&RouteNode> {
        self.nodes.get(&id)
    }

    pub(crate) fn provenance(&self) -> &RouteBranchProvenance {
        &self.provenance
    }
}

#[derive(Clone)]
pub struct RouteTreeBuilder {
    root: Option<RouteId>,
    nodes: BTreeMap<RouteId, RouteNode>,
    next_id: u64,
    next_declaration_index: usize,
}

impl fmt::Debug for RouteTreeBuilder {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RouteTreeBuilder")
            .field("root", &self.root)
            .field("route_count", &self.nodes.len())
            .finish()
    }
}

impl RouteTreeBuilder {
    fn new() -> Self {
        Self {
            root: None,
            nodes: BTreeMap::new(),
            next_id: 1,
            next_declaration_index: 0,
        }
    }

    pub fn root(mut self, pattern: RoutePattern) -> Result<Self, RouteError> {
        if self.root.is_some() {
            return Err(invalid_tree("route tree root is already declared"));
        }

        let id = self.allocate_id();
        let node = self.node(id, None, pattern, RouteNodeOptions::new());
        self.root = Some(id);
        self.nodes.insert(id, node);
        Ok(self)
    }

    pub const fn root_id(&self) -> Option<RouteId> {
        self.root
    }

    pub fn route_with_options(
        &mut self,
        parent: RouteId,
        pattern: RoutePattern,
        options: RouteNodeOptions,
    ) -> Result<RouteId, RouteError> {
        self.route_inner(parent, pattern, options)
    }

    pub fn route(&mut self, parent: RouteId, pattern: RoutePattern) -> Result<RouteId, RouteError> {
        self.route_inner(parent, pattern, RouteNodeOptions::new())
    }

    fn route_inner(
        &mut self,
        parent: RouteId,
        pattern: RoutePattern,
        options: RouteNodeOptions,
    ) -> Result<RouteId, RouteError> {
        if !self.nodes.contains_key(&parent) {
            return Err(invalid_tree("route parent does not exist"));
        }

        let id = self.allocate_id();
        let node = self.node(id, Some(parent), pattern, options);
        self.nodes.insert(id, node);
        self.nodes.get_mut(&parent).unwrap().children.push(id);
        Ok(id)
    }

    pub fn build(self) -> Result<RouteTree, RouteError> {
        let root = self
            .root
            .ok_or_else(|| invalid_tree("route tree root is missing"))?;
        Ok(RouteTree {
            root,
            nodes: self.nodes,
            provenance: RouteBranchProvenance::next_tree_source(),
        })
    }

    fn allocate_id(&mut self) -> RouteId {
        let id = RouteId::new(self.next_id);
        self.next_id += 1;
        id
    }

    fn node(
        &mut self,
        id: RouteId,
        parent: Option<RouteId>,
        pattern: RoutePattern,
        options: RouteNodeOptions,
    ) -> RouteNode {
        let declaration_index = self.next_declaration_index;
        self.next_declaration_index += 1;
        RouteNode {
            id,
            parent,
            pattern,
            options,
            declaration_index,
            children: Vec::new(),
        }
    }
}

fn invalid_tree(message: &'static str) -> RouteError {
    RouteError::invalid_route_tree(message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::{RouteErrorKind, RouteErrorSourceCategory};
    use crate::ids::RouteId;
    use crate::meta::{ErrorBoundaryKey, RouteExplicitKey, RouteMeta};
    use crate::pattern::RoutePattern;
    use crate::slot::RouteSlotKey;

    #[test]
    fn builder_allocates_stable_route_ids_in_declaration_order() {
        let mut builder = RouteTree::builder()
            .root(RoutePattern::path("/").unwrap())
            .unwrap();
        let root = builder.root_id().unwrap();
        let settings = builder
            .route(root, RoutePattern::path("settings").unwrap())
            .unwrap();
        let profile = builder
            .route(settings, RoutePattern::path(":section").unwrap())
            .unwrap();
        let tree = builder.build().unwrap();

        assert_eq!(tree.root(), root);
        assert_eq!(root.raw(), 1);
        assert_eq!(settings.raw(), 2);
        assert_eq!(profile.raw(), 3);
        assert_eq!(tree.node(settings).unwrap().parent(), Some(root));
    }

    #[test]
    fn builder_requires_exactly_one_root() {
        let error = RouteTree::builder().build().unwrap_err();

        assert_eq!(error.kind(), RouteErrorKind::InvalidRouteTree);
        assert_eq!(
            error.source_category(),
            RouteErrorSourceCategory::RouterCore
        );
    }

    #[test]
    fn builder_rejects_second_root_and_missing_parent() {
        let duplicate_root = RouteTree::builder()
            .root(RoutePattern::path("/").unwrap())
            .unwrap()
            .root(RoutePattern::path("other").unwrap())
            .unwrap_err();
        assert_eq!(duplicate_root.kind(), RouteErrorKind::InvalidRouteTree);

        let mut builder = RouteTree::builder()
            .root(RoutePattern::path("/").unwrap())
            .unwrap();
        let missing_parent = builder
            .route(RouteId::new(99), RoutePattern::path("orphan").unwrap())
            .unwrap_err();
        assert_eq!(missing_parent.kind(), RouteErrorKind::InvalidRouteTree);
    }

    #[test]
    fn children_remain_in_explicit_declaration_order() {
        let mut builder = RouteTree::builder()
            .root(RoutePattern::path("/").unwrap())
            .unwrap();
        let root = builder.root_id().unwrap();
        let first = builder
            .route(root, RoutePattern::path("first").unwrap())
            .unwrap();
        let second = builder
            .route(root, RoutePattern::path("second").unwrap())
            .unwrap();
        let tree = builder.build().unwrap();

        assert_eq!(tree.node(root).unwrap().children(), &[first, second]);
    }

    #[test]
    fn debug_output_does_not_expose_internal_matcher_inputs() {
        let mut builder = RouteTree::builder()
            .root(RoutePattern::path("/").unwrap())
            .unwrap();
        let root = builder.root_id().unwrap();
        let _child = builder
            .route(root, RoutePattern::path("settings").unwrap())
            .unwrap();
        let builder_debug = format!("{:?}", builder);
        let tree = builder.build().unwrap();
        let node_debug = format!("{:?}", tree.node(root).unwrap());
        let tree_debug = format!("{:?}", tree);

        for output in [builder_debug, node_debug, tree_debug] {
            assert!(!output.contains("children"));
            assert!(!output.contains("declaration_index"));
            assert!(!output.contains("next_declaration_index"));
        }
    }

    #[test]
    fn route_options_attach_slot_key_boundary_and_meta_to_nodes() {
        let mut builder = RouteTree::builder()
            .root(RoutePattern::path("/").unwrap())
            .unwrap();
        let root = builder.root_id().unwrap();
        let options = RouteNodeOptions::new()
            .with_slot(RouteSlotKey::named("details").unwrap())
            .with_explicit_key(RouteExplicitKey::new("detail-v1").unwrap())
            .with_error_boundary(ErrorBoundaryKey::named("detail-boundary").unwrap())
            .with_meta(RouteMeta::new().with_diagnostic_label("detail").unwrap());

        let detail = builder
            .route_with_options(root, RoutePattern::path("details/:id").unwrap(), options)
            .unwrap();
        let tree = builder.build().unwrap();
        let node = tree.node(detail).unwrap();

        assert_eq!(node.options().slot().as_str(), "details");
        assert_eq!(
            node.options().explicit_key().map(RouteExplicitKey::as_str),
            Some("detail-v1")
        );
        assert_eq!(
            node.options()
                .error_boundary()
                .map(ErrorBoundaryKey::as_str),
            Some("detail-boundary")
        );
        assert_eq!(node.options().meta().diagnostic_label(), Some("detail"));
    }

    #[test]
    fn route_without_options_uses_default_slot_and_empty_meta() {
        let mut builder = RouteTree::builder()
            .root(RoutePattern::path("/").unwrap())
            .unwrap();
        let root = builder.root_id().unwrap();
        let child = builder
            .route(root, RoutePattern::path("settings").unwrap())
            .unwrap();
        let tree = builder.build().unwrap();
        let node = tree.node(child).unwrap();

        assert!(node.options().slot().is_default());
        assert!(node.options().explicit_key().is_none());
        assert!(node.options().error_boundary().is_none());
        assert!(node.options().meta().diagnostic_label().is_none());
    }

    #[test]
    fn route_tree_provenance_is_internal_stable_for_clones_and_distinct_for_builds() {
        let tree = RouteTree::builder()
            .root(RoutePattern::path("/").unwrap())
            .unwrap()
            .build()
            .unwrap();
        let cloned = tree.clone();
        let other = RouteTree::builder()
            .root(RoutePattern::path("/").unwrap())
            .unwrap()
            .build()
            .unwrap();

        assert!(tree.provenance() == cloned.provenance());
        assert!(tree.provenance() != other.provenance());

        for output in [
            format!("{:?}", tree),
            format!("{:?}", cloned),
            format!("{:?}", other),
        ] {
            assert!(!output.contains("provenance"));
            assert!(!output.contains("RouteBranchProvenance"));
        }
    }
}
