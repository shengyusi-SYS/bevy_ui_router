use bevy_ui_router::prelude::*;

fn main() {
    let mut builder = RouteTree::builder().root(RoutePattern::pathless()).unwrap();
    let root = builder.root_id().unwrap();
    let user = builder
        .route(root, RoutePattern::path("users/:id").unwrap())
        .unwrap();
    let matcher = RouteMatcher::new(builder.build().unwrap());
    let location = RouteLocation::new("/users/42")
        .with_search("sort=name&sort=date")
        .with_fragment("details");
    let matched = matcher.match_location(&location).unwrap();
    assert_eq!(matched.leaf().unwrap().route(), user);
    assert_eq!(matched.leaf().unwrap().params().get("id"), Some("42"));
    assert_eq!(location.search(), Some("sort=name&sort=date"));
    assert_eq!(location.fragment(), Some("details"));

    let mut history = MemoryHistory::new();
    history.push(location.clone());
    history.push(RouteLocation::new("/users/43"));
    history.replace(RouteLocation::new("/users/44"));
    assert_eq!(history.back().unwrap(), location);
}
