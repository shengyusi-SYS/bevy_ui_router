use bevy_ui_router_core::prelude::{RouteViewInput, RouteViewKey};
use bevy_ui_router_view::{
    RouteViewDescriptor, RouteViewRegistry, ViewBuildEffect, ViewDriverKey, ViewRequestId,
};

#[test]
fn final_view_registry_and_effect_types_are_public_without_authority() {
    fn assert_request_traits<T: Clone + Copy + std::fmt::Debug + Eq + Ord + std::hash::Hash>() {}
    assert_request_traits::<ViewRequestId>();

    let view = RouteViewKey::new("root").unwrap();
    let driver = ViewDriverKey::new("immediate").unwrap();
    let mut registry = RouteViewRegistry::new();
    registry
        .insert(view, RouteViewDescriptor::new(driver))
        .unwrap();

    let _: for<'a> fn(&'a ViewBuildEffect) -> &'a RouteViewInput = ViewBuildEffect::input;
    let _: fn(&ViewBuildEffect) -> ViewRequestId = ViewBuildEffect::request_id;
}
