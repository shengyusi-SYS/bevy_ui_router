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
use bevy_ui_router_ecs::{
    BeginFrameApplySessionError, EcsFrameApplyReport, EcsFrameApplySession, EcsFrameApplyStatus,
    FinishFrameApplySessionError,
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
fn ecs_root_exports_apply_bridge_types_without_prelude_promotion() {
    fn assert_public<T>() {}

    assert_public::<EcsFrameApplySession>();
    assert_public::<EcsFrameApplyReport>();
    assert_public::<EcsFrameApplyStatus>();
    assert_public::<BeginFrameApplySessionError>();
    assert_public::<FinishFrameApplySessionError>();

    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        fs::read_to_string(manifest_dir.join("src").join("lib.rs")).expect("lib source readable");

    for exported in [
        "BeginFrameApplySessionError",
        "EcsFrameApplyReport",
        "EcsFrameApplySession",
        "EcsFrameApplyStatus",
        "FinishFrameApplySessionError",
    ] {
        assert!(
            source.contains(exported),
            "crate root should export apply bridge foundation type `{exported}`"
        );
    }
    let prelude_start = source
        .find("pub mod prelude {")
        .expect("lib.rs should define a prelude block");
    let prelude = &source[prelude_start..];
    assert!(
        !prelude.contains("EcsFrameApplySession")
            && !prelude.contains("EcsFrameApplyReport")
            && !prelude.contains("EcsFrameApplyStatus"),
        "prelude should stay focused and not promote apply bridge foundation types"
    );
}

#[test]
fn cleanup_stays_crate_private_while_report_exposes_read_only_ordered_evidence() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let lib_source =
        fs::read_to_string(manifest_dir.join("src").join("lib.rs")).expect("lib source readable");
    let cleanup_source = fs::read_to_string(manifest_dir.join("src").join("cleanup.rs"))
        .expect("cleanup source should be readable");
    let core_lib_source = fs::read_to_string(
        manifest_dir
            .parent()
            .expect("ecs crate should live under crates/")
            .join("bevy_ui_router_core")
            .join("src")
            .join("lib.rs"),
    )
    .expect("core lib source should be readable");

    assert!(
        lib_source.contains("pub(crate) mod cleanup;"),
        "cleanup module should stay crate-private"
    );
    for forbidden in [
        "pub mod cleanup;",
        "pub use cleanup::CleanupCapability",
        "pub use cleanup::cleanup_router_owned_subtree",
        "pub use cleanup::CleanupReport",
        "CleanupCapability",
        "cleanup_router_owned_subtree",
    ] {
        let prelude_start = lib_source
            .find("pub mod prelude {")
            .expect("lib.rs should define a prelude block");
        let prelude = &lib_source[prelude_start..];
        assert!(
            !prelude.contains(forbidden),
            "prelude should not expose cleanup authority fragment `{forbidden}`"
        );
    }
    for required in [
        "pub(crate) struct CleanupCapability;",
        "pub(crate) fn cleanup_router_owned_subtree(",
        "pub struct CleanupReport",
        "pub fn removed(&self) -> &[Entity]",
        "pub fn skipped_mismatches(&self) -> &[Entity]",
    ] {
        assert!(
            cleanup_source.contains(required),
            "cleanup source should contain `{required}`"
        );
    }
    assert!(
        core_lib_source.contains("#[cfg(feature = \"test-support\")]\npub mod test_support;"),
        "core should gate test support behind the narrow feature flag"
    );
    let core_prelude_start = core_lib_source
        .find("pub mod prelude {")
        .expect("core lib.rs should define a prelude block");
    let core_prelude = &core_lib_source[core_prelude_start..];
    for forbidden in ["test_support", "frame_apply_request_with_next_generation"] {
        assert!(
            !core_prelude.contains(forbidden),
            "core prelude should not expose test-support fragment `{forbidden}`"
        );
    }
}

#[test]
fn route_host_and_slot_observe_core_issued_identity() {
    let state = RouterEcsState::new();
    let host = state.route_host();
    let slot = state.route_slot();

    assert_eq!(host.router(), state.router());
    assert_eq!(host.host(), state.host());
    assert_eq!(slot.router(), state.router());
    assert_eq!(slot.host(), state.host());
    assert_eq!(slot.slot(), state.slot());
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
        "impl RouteHost {\n    pub fn from_runtime(",
        "impl RouteHost {\n    pub fn from_state(",
        "impl RouteSlot {\n    pub fn new(",
        "impl RouteSlot {\n    pub const fn new(",
        "impl RouteSlot {\n    pub fn from_raw(",
        "impl RouteSlot {\n    pub fn from_runtime(",
        "impl RouteSlot {\n    pub fn from_state(",
    ] {
        assert!(
            !source.contains(forbidden),
            "components.rs exposes forbidden raw identity constructor fragment `{forbidden}`"
        );
    }
}

#[test]
fn router_ecs_state_exposes_identity_accessors_without_public_runtime_bridge() {
    let state = RouterEcsState::new();
    let host = state.route_host();
    let slot = state.route_slot();

    assert_eq!(host.router(), state.router());
    assert_eq!(host.host(), state.host());
    assert_eq!(slot.router(), state.router());
    assert_eq!(slot.host(), state.host());
    assert_eq!(slot.slot(), state.slot());
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
fn ecs_public_sources_do_not_expose_router_runtime_signatures() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let components = fs::read_to_string(manifest_dir.join("src").join("components.rs"))
        .expect("components source should be readable");
    let bridge = fs::read_to_string(manifest_dir.join("src").join("bridge.rs"))
        .expect("bridge source should be readable");
    let state = fs::read_to_string(manifest_dir.join("src").join("state.rs"))
        .expect("state source should be readable");

    for forbidden in [
        "pub fn from_runtime(",
        "pub fn from_state(",
        "pub const fn runtime(&self) -> &RouterRuntime",
        "pub fn runtime_mut(&mut self) -> &mut RouterRuntime",
        "pub fn from_runtime(runtime: RouterRuntime) -> Self",
    ] {
        assert!(
            !components.contains(forbidden)
                && !bridge.contains(forbidden)
                && !state.contains(forbidden),
            "ecs public source exposes forbidden runtime authority fragment `{forbidden}`"
        );
    }
}

#[test]
fn plugin_registers_router_resources_without_creating_host_entities() {
    use bevy_ecs::entity::Entity;
    use bevy_ecs::query::With;

    let mut app = App::new();
    app.add_plugins(BevyUiRouterPlugin);

    assert!(app.world().contains_resource::<RouterEcsState>());
    let host_components = {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<RouteHost>>();
        query.iter(world).count()
    };
    let slot_components = {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<RouteSlot>>();
        query.iter(world).count()
    };
    let frame_root_components = {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<RouteFrameRoot>>();
        query.iter(world).count()
    };

    assert_eq!(host_components, 0);
    assert_eq!(slot_components, 0);
    assert_eq!(frame_root_components, 0);
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
