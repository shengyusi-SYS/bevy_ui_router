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
