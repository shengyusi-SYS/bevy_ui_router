use std::collections::BTreeMap;

use crate::error::RouteError;
use crate::ids::RouteId;
use crate::pattern::RoutePattern;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouteNode {
    id: RouteId,
    parent: Option<RouteId>,
    pattern: RoutePattern,
    declaration_index: usize,
    children: Vec<RouteId>,
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

    #[allow(dead_code)]
    pub(crate) const fn declaration_index(&self) -> usize {
        self.declaration_index
    }

    #[allow(dead_code)]
    pub(crate) fn children(&self) -> &[RouteId] {
        &self.children
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouteTree {
    root: RouteId,
    nodes: BTreeMap<RouteId, RouteNode>,
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouteTreeBuilder {
    root: Option<RouteId>,
    nodes: BTreeMap<RouteId, RouteNode>,
    next_id: u64,
    next_declaration_index: usize,
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
        let node = self.node(id, None, pattern);
        self.root = Some(id);
        self.nodes.insert(id, node);
        Ok(self)
    }

    pub const fn root_id(&self) -> Option<RouteId> {
        self.root
    }

    pub fn route(&mut self, parent: RouteId, pattern: RoutePattern) -> Result<RouteId, RouteError> {
        if !self.nodes.contains_key(&parent) {
            return Err(invalid_tree("route parent does not exist"));
        }

        let id = self.allocate_id();
        let node = self.node(id, Some(parent), pattern);
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
        })
    }

    fn allocate_id(&mut self) -> RouteId {
        let id = RouteId::new(self.next_id);
        self.next_id += 1;
        id
    }

    fn node(&mut self, id: RouteId, parent: Option<RouteId>, pattern: RoutePattern) -> RouteNode {
        let declaration_index = self.next_declaration_index;
        self.next_declaration_index += 1;
        RouteNode {
            id,
            parent,
            pattern,
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
    use crate::pattern::RoutePattern;

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
}
