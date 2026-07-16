use crate::{
    meta::ErrorBoundaryKey,
    pattern::RoutePattern,
    route_tree::{RouteNodeOptions, RouteTree},
    view_key::RouteViewKey,
};

pub fn options_with_error_boundary(
    options: RouteNodeOptions,
    boundary: ErrorBoundaryKey,
) -> RouteNodeOptions {
    options.with_error_boundary(boundary)
}

pub fn options_with_error_view(
    options: RouteNodeOptions,
    boundary: ErrorBoundaryKey,
    view: RouteViewKey,
) -> RouteNodeOptions {
    options.with_error_view(boundary, view)
}

pub fn nested_items_route_tree() -> RouteTree {
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
    builder.build().unwrap()
}

pub fn nested_items_view_route_tree() -> RouteTree {
    let mut builder = RouteTree::builder()
        .root_with_options(
            RoutePattern::path("/").unwrap(),
            RouteNodeOptions::new().with_view(RouteViewKey::new("layout").unwrap()),
        )
        .unwrap();
    let root = builder.root_id().unwrap();
    builder
        .route_with_options(
            root,
            RoutePattern::path("items/:id").unwrap(),
            RouteNodeOptions::new().with_view(RouteViewKey::new("item").unwrap()),
        )
        .unwrap();
    builder.build().unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_items_view_fixture_has_two_stable_view_bound_levels() {
        let tree = nested_items_view_route_tree();
        let nodes = tree.nodes_in_declaration_order();

        assert_eq!(nodes.len(), 2);
        assert_eq!(
            nodes[0].options().view_binding(),
            Some(&RouteViewKey::new("layout").unwrap())
        );
        assert_eq!(
            nodes[1].options().view_binding(),
            Some(&RouteViewKey::new("item").unwrap())
        );
    }
}
