use bevy_ui_router::prelude::*;

fn main() {
    let view = RouteViewKey::new("profile").unwrap();
    let driver = ViewDriverKey::new("independent-renderer").unwrap();
    let mut registry = RouteViewRegistry::new();
    registry
        .insert(view.clone(), RouteViewDescriptor::new(driver.clone()))
        .unwrap();
    assert!(
        registry
            .insert(view.clone(), RouteViewDescriptor::new(driver))
            .is_err()
    );
    let tree = RouteTree::builder()
        .root_with_options(
            RoutePattern::path("/profile").unwrap(),
            RouteNodeOptions::new().with_view(view),
        )
        .unwrap()
        .build()
        .unwrap();
    let root = tree.root();
    let matched = RouteMatcher::new(tree)
        .match_location(&RouteLocation::new("/profile"))
        .unwrap();
    assert_eq!(matched.leaf().unwrap().route(), root);
}
