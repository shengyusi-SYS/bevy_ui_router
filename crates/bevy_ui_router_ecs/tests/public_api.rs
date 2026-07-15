use bevy_app::App;
use bevy_ui_router_ecs::prelude::*;

fn assert_public<T>() {}

#[test]
fn root_profile_exports_the_new_navigation_surface() {
    assert_public::<BevyUiRouterPlugin>();
    assert_public::<RouterConfig>();
    assert_public::<RouterHandle>();
    assert_public::<RouterHostRegistration>();
    assert_public::<RouterNavigation>();
    assert_public::<RouterSnapshots>();
    assert_public::<RouterInputGate>();
    assert_public::<NavigationRequestSnapshot>();
    assert_public::<ContentColor>();
    assert_public::<SolidRect>();
    assert_public::<ViewBuildReport>();

    let mut app = App::new();
    app.add_plugins(BevyUiRouterPlugin::default());
    assert!(app.world().contains_resource::<RouterNavigation>());
    assert!(app.world().contains_resource::<RouterSnapshots>());
    assert!(app.world().contains_resource::<RouterInputGate>());
}
