use std::fs;
use std::path::Path;

use bevy_app::App;
use bevy_ui_router_core::matcher::RouteMatcher;
use bevy_ui_router_core::pattern::RoutePattern;
use bevy_ui_router_core::route_tree::RouteTree;
use bevy_ui_router_core::runtime::RouterRuntime;
use bevy_ui_router_ecs::prelude::{
    BevyUiRouterPlugin, RouteFrameRoot, RouteHost, RouteSlot, RouterEcsState, RouterScheduleSet,
};

#[test]
fn ecs_prelude_exposes_host_slot_frame_root_plugin_and_schedule_labels() {
    fn assert_public<T>() {}

    assert_public::<RouteHost>();
    assert_public::<RouteSlot>();
    assert_public::<RouteFrameRoot>();
    assert_public::<RouterEcsState>();
    assert_public::<BevyUiRouterPlugin>();

    let _plugin = BevyUiRouterPlugin;
    assert_eq!(
        RouterScheduleSet::CollectNavigation,
        RouterScheduleSet::CollectNavigation
    );
    assert_eq!(RouterScheduleSet::Cleanup, RouterScheduleSet::Cleanup);
}

#[test]
fn route_host_and_slot_observe_core_issued_identity() {
    let runtime = RouterRuntime::new();
    let host = RouteHost::from_runtime(&runtime);
    let slot = RouteSlot::from_runtime(&runtime);

    assert_eq!(host.router(), runtime.router());
    assert_eq!(host.host(), runtime.host());
    assert_eq!(slot.router(), runtime.router());
    assert_eq!(slot.host(), runtime.host());
    assert_eq!(slot.slot(), runtime.slot());
}

#[test]
fn frame_root_records_router_host_slot_route_frame_generation_and_branch_index() {
    let matcher = RouteMatcher::new(
        RouteTree::builder()
            .root(RoutePattern::path("/").unwrap())
            .unwrap()
            .build()
            .unwrap(),
    );
    let target = matcher
        .match_location(&bevy_ui_router_core::location::RouteLocation::new("/"))
        .unwrap();
    let mut runtime = RouterRuntime::new();
    let snapshot = runtime.begin_first_navigation(&target).unwrap();
    let request = &snapshot.requests()[0];
    let frame = RouteFrameRoot::from_frame_request(request);

    assert_eq!(frame.identity().router(), runtime.router());
    assert_eq!(frame.identity().host(), runtime.host());
    assert_eq!(frame.identity().slot(), runtime.slot());
    assert_eq!(frame.identity().route(), request.frame().identity().route());
    assert_eq!(frame.identity().frame(), request.frame().identity().frame());
    assert_eq!(
        frame.identity().generation(),
        request.frame().identity().generation()
    );
    assert_eq!(frame.branch_index(), request.frame().branch_index());
}

#[test]
fn route_host_does_not_provide_raw_id_constructors() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = fs::read_to_string(manifest_dir.join("src").join("components.rs"))
        .expect("components source should be readable");

    for forbidden in [
        "impl RouteHost {\n    pub fn new(",
        "impl RouteHost {\n    pub const fn new(",
        "impl RouteHost {\n    pub fn from_raw(",
        "impl RouteSlot {\n    pub fn new(",
        "impl RouteSlot {\n    pub const fn new(",
        "impl RouteSlot {\n    pub fn from_raw(",
    ] {
        assert!(
            !source.contains(forbidden),
            "components.rs exposes forbidden raw identity constructor fragment `{forbidden}`"
        );
    }
}

#[test]
fn route_frame_root_does_not_offer_raw_identity_constructor() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = fs::read_to_string(manifest_dir.join("src").join("components.rs"))
        .expect("components source should be readable");

    for forbidden in [
        "impl RouteFrameRoot {\n    pub fn new(",
        "impl RouteFrameRoot {\n    pub const fn new(",
        "impl RouteFrameRoot {\n    pub fn from_identity(",
        "impl RouteFrameRoot {\n    pub fn from_observed_identity(",
    ] {
        assert!(
            !source.contains(forbidden),
            "components.rs exposes forbidden frame root authority fragment `{forbidden}`"
        );
    }
}

#[test]
fn plugin_registers_router_resources_without_creating_host_entities() {
    let mut app = App::new();
    let initial_entities = app.world().entities().count_spawned();

    app.add_plugins(BevyUiRouterPlugin);

    assert!(app.world().contains_resource::<RouterEcsState>());
    assert_eq!(app.world().entities().count_spawned(), initial_entities + 2);
}

#[test]
fn plugin_source_keeps_focus_registration_and_schedule_order_internal() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = fs::read_to_string(manifest_dir.join("src").join("plugin.rs"))
        .expect("plugin source should be readable");

    assert!(
        source.contains("init_resource::<RouterEcsState>()"),
        "plugin should initialize RouterEcsState"
    );
    assert!(
        source.contains("init_resource::<RouteFocusState>()"),
        "plugin should initialize the crate-private RouteFocusState"
    );
    assert!(
        source.contains(
            "(\n                    RouterScheduleSet::CollectNavigation,\n                    RouterScheduleSet::PlanFrames,\n                    RouterScheduleSet::BuildViews,\n                    RouterScheduleSet::ApplyViews,\n                    RouterScheduleSet::PrepareInert,\n                    RouterScheduleSet::ReportToCore,\n                    RouterScheduleSet::ActivateCommitted,\n                    RouterScheduleSet::Cleanup,\n                    RouterScheduleSet::AcknowledgeFailures,\n                )\n                    .chain()"
        ),
        "plugin should configure RouterScheduleSet in the required stable order"
    );
    assert!(
        !source.contains("bsn"),
        "plugin must not register BSN systems or mention BSN integration"
    );
}
