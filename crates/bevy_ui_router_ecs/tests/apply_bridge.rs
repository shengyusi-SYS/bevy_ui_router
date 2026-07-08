use std::fs;
use std::path::Path;

use bevy_ui_router_core::error::RouteErrorKind;
use bevy_ui_router_core::location::RouteLocation;
use bevy_ui_router_core::matcher::RouteMatcher;
use bevy_ui_router_core::pattern::RoutePattern;
use bevy_ui_router_core::route_tree::RouteTree;
use bevy_ui_router_core::runtime::RouterRuntime;
use bevy_ui_router_core::transaction::FrameApplyRequest;
use bevy_ui_router_ecs::{
    EcsFrameApplyReport, EcsFrameApplySession, EcsFrameApplyStatus, RouterEcsState,
};
use bevy_ui_router_view::{RouteViewDescriptor, ViewBindingKey, ViewReportSession};

#[test]
fn bridge_mints_session_from_core_frame_apply_request() {
    fn assert_public<T>() {}

    assert_public::<EcsFrameApplySession>();
    assert_public::<EcsFrameApplyReport>();
    assert_public::<EcsFrameApplyStatus>();

    let request = issued_request("/items/42", 1);
    let descriptor = descriptor("leaf");
    let mut state = RouterEcsState::new();
    let session = state
        .begin_frame_apply_session(&request, descriptor.clone())
        .expect("core-issued frame requests should mint ecs apply sessions");

    assert_eq!(session.request(), &request);
    assert_eq!(session.frame(), request.frame());
    assert_eq!(
        session.build_request().descriptor().binding(),
        descriptor.binding()
    );
    assert!(session.report().is_none());
    assert!(!session.finalized());
    assert!(
        state
            .begin_frame_apply_session(&request, descriptor)
            .is_err(),
        "duplicate pending frame requests should be rejected"
    );
}

#[test]
fn bridge_does_not_expose_frame_apply_token_mutation() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let bridge = fs::read_to_string(manifest_dir.join("src").join("bridge.rs"))
        .expect("bridge source should be readable");
    let state = fs::read_to_string(manifest_dir.join("src").join("state.rs"))
        .expect("state source should be readable");

    for forbidden in [
        "pub fn token_mut(",
        "pub fn stale_check(",
        "pub fn report_to_core(",
        "pub fn commit_ready_navigation(",
        "pub fn runtime(&self) -> &RouterRuntime",
        "pub fn runtime_mut(&mut self) -> &mut RouterRuntime",
        "pub fn from_runtime(runtime: RouterRuntime) -> Self",
    ] {
        assert!(
            !bridge.contains(forbidden) && !state.contains(forbidden),
            "ecs bridge foundation exposes forbidden authority fragment `{forbidden}`"
        );
    }
}

#[test]
fn bridge_rejects_view_report_from_foreign_frame_request() {
    let request = issued_request("/items/42", 1);
    let foreign_request = issued_request("/items/42", 0);
    let descriptor = descriptor("leaf");
    let mut state = RouterEcsState::new();
    let session = state
        .begin_frame_apply_session(&request, descriptor.clone())
        .expect("session should mint");
    let foreign_frame = ViewReportSession::from_frame_request(&foreign_request, descriptor.clone());
    let foreign_provenance = ViewReportSession::from_frame_request(&request, descriptor);

    assert!(
        !session.accepts_report_session(&foreign_frame),
        "sessions must reject reports from a different frame request"
    );
    assert!(
        !session.accepts_report_session(&foreign_provenance),
        "sessions must reject reports from a non-owned view report session even for the same frame request"
    );
}

#[test]
fn bridge_records_renderer_build_failure_without_partial_output() {
    let request = issued_request("/items/42", 1);
    let mut state = RouterEcsState::new();
    let mut session = state
        .begin_frame_apply_session(&request, descriptor("leaf"))
        .expect("session should mint");

    let report = session.record_build_failure("renderer build failed");

    assert_eq!(report.status(), EcsFrameApplyStatus::Failed);
    assert_eq!(
        report
            .failure()
            .expect("build failure should be recorded")
            .error()
            .kind(),
        RouteErrorKind::ViewBuildFailed
    );
    assert!(report.partial_output().is_none());
    assert!(!session.finalized());
}

#[test]
fn bridge_records_renderer_apply_success_as_unfinalized_observation() {
    let request = issued_request("/items/42", 1);
    let mut state = RouterEcsState::new();
    let mut session = state
        .begin_frame_apply_session(&request, descriptor("leaf"))
        .expect("session should mint");
    let request_id = session.build_request().request_id();

    let report = session.record_apply_success(Some("renderer-output"));

    assert_eq!(report.status(), EcsFrameApplyStatus::Pending);
    let output = report
        .partial_output()
        .expect("apply success should record local partial output");
    assert_eq!(output.request_id(), request_id);
    assert_eq!(output.binding().as_str(), "leaf");
    assert_eq!(
        output.disposal_hint().map(|hint| hint.as_str()),
        Some("renderer-output")
    );
    assert!(report.failure().is_none());
    assert!(!session.finalized());
}

#[test]
fn bridge_records_renderer_apply_failure_as_unfinalized_failure() {
    let request = issued_request("/items/42", 1);
    let mut state = RouterEcsState::new();
    let mut session = state
        .begin_frame_apply_session(&request, descriptor("leaf"))
        .expect("session should mint");
    let request_id = session.build_request().request_id();

    let report = session.record_apply_failure("renderer apply failed", Some("renderer-output"));

    assert_eq!(report.status(), EcsFrameApplyStatus::Failed);
    let failure = report
        .failure()
        .expect("apply failure should be recorded locally");
    assert_eq!(failure.error().kind(), RouteErrorKind::ViewApplyFailed);
    assert_eq!(
        report.partial_output().map(|output| output.request_id()),
        Some(request_id)
    );
    assert!(!session.finalized());
}

#[test]
fn bridge_discards_renderer_local_stale_without_core_report() {
    let request = issued_request("/items/42", 1);
    let mut state = RouterEcsState::new();
    let mut session = state
        .begin_frame_apply_session(&request, descriptor("leaf"))
        .expect("session should mint");

    let report = session.discard_stale("renderer stale");

    assert_eq!(report.status(), EcsFrameApplyStatus::StaleDiscarded);
    assert_eq!(
        report
            .failure()
            .expect("stale discard should keep the failure")
            .error()
            .kind(),
        RouteErrorKind::StaleResult
    );
    assert!(report.partial_output().is_none());
    assert!(!session.finalized());
}

fn descriptor(binding: &str) -> RouteViewDescriptor {
    RouteViewDescriptor::new(ViewBindingKey::new(binding))
}

fn issued_request(path: &str, request_index: usize) -> FrameApplyRequest {
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
    let target = RouteMatcher::new(builder.build().unwrap())
        .match_location(&RouteLocation::new(path))
        .unwrap();
    let mut runtime = RouterRuntime::new();

    runtime.begin_first_navigation(&target).unwrap().requests()[request_index].clone()
}
