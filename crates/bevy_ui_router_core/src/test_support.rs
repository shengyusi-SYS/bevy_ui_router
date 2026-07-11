use crate::transaction::FrameApplyRequest;
use crate::{pattern::RoutePattern, route_tree::RouteTree};

pub fn frame_apply_request_with_next_generation(request: &FrameApplyRequest) -> FrameApplyRequest {
    request.clone_with_next_generation()
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
