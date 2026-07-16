use bevy_ui_router::prelude::{RouteLocation, RouteTree, RouteViewRegistry};

#[test]
fn default_facade_keeps_core_and_view_available() {
    let _ = RouteLocation::new("/");
    let _ = RouteTree::builder();
    let _ = RouteViewRegistry::new();
}
