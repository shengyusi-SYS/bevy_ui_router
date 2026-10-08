use bevy_ui_router_core::prelude::{
    FrameOrigin, NavigationFailure, RouteId, RouteInputRef, RouteLocation, RouteParams,
    RouteViewInput, RouteViewKey,
};
use bevy_ui_router_view::{
    RouteViewDescriptor, RouteViewRegistry, ViewBuildEffect, ViewDriverKey, ViewRequestId,
};

#[test]
fn current_input_ref_is_a_borrowed_renderer_neutral_projection() {
    fn project<'a>(
        input: RouteInputRef<'a>,
    ) -> (
        &'a RouteLocation,
        &'a RouteParams,
        &'a FrameOrigin,
        Option<&'a NavigationFailure>,
    ) {
        let _: Option<RouteId> = input.route();
        (
            input.location(),
            input.params(),
            input.origin(),
            input.failure(),
        )
    }
    let _ = project;
}

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
