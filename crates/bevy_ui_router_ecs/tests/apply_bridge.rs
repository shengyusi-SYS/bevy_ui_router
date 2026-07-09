use std::fs;
use std::path::Path;

use bevy_camera::visibility::Visibility;
use bevy_ecs::prelude::{Entity, World};
use bevy_picking::Pickable;
use bevy_ui::InteractionDisabled;
use bevy_ui_router_core::error::RouteErrorKind;
use bevy_ui_router_core::location::RouteLocation;
use bevy_ui_router_core::matcher::RouteMatcher;
use bevy_ui_router_core::pattern::RoutePattern;
use bevy_ui_router_core::route_tree::RouteTree;
use bevy_ui_router_core::runtime::RouterRuntime;
use bevy_ui_router_core::transaction::FrameApplyRequest;
use bevy_ui_router_ecs::{
    BeginFrameApplySessionError, CompleteFrameApplySessionError, EcsFrameApplyReport,
    EcsFrameApplySession, EcsFrameApplyStatus, FinishFrameApplySessionError, RouteFrameRoot,
    RouterEcsState,
};
use bevy_ui_router_view::{RouteViewDescriptor, ViewBindingKey, ViewReportSession};

#[test]
fn bridge_mints_session_from_core_frame_apply_request() {
    fn assert_public<T>() {}

    assert_public::<EcsFrameApplySession>();
    assert_public::<EcsFrameApplyReport>();
    assert_public::<EcsFrameApplyStatus>();

    let (mut state, request) = state_with_request("/items/42", 1);
    let descriptor = descriptor("leaf");
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
        matches!(
            state.begin_frame_apply_session(&request, descriptor),
            Err(BeginFrameApplySessionError::RequestAlreadyPending)
        ),
        "duplicate pending frame requests should be rejected"
    );
}

#[test]
fn bridge_releases_build_failed_session_for_same_request_after_finish() {
    let (mut state, request) = state_with_request("/items/42", 1);
    let descriptor = descriptor("leaf");
    let mut session = state
        .begin_frame_apply_session(&request, descriptor.clone())
        .expect("session should mint");

    let _report = session.record_build_failure("renderer build failed");
    state
        .finish_frame_apply_session(&session)
        .expect("terminal build failure should release the pending request");

    let reminted = state.begin_frame_apply_session(&request, descriptor);

    assert!(
        reminted.is_ok(),
        "the same frame request should be allowed again after terminal build failure is retired"
    );
}

#[test]
fn bridge_releases_apply_failed_session_for_same_request_after_finish() {
    let (mut state, request) = state_with_request("/items/42", 1);
    let descriptor = descriptor("leaf");
    let mut session = state
        .begin_frame_apply_session(&request, descriptor.clone())
        .expect("session should mint");

    let _report = session.record_apply_failure("renderer apply failed", Some("renderer-output"));
    state
        .finish_frame_apply_session(&session)
        .expect("terminal apply failure should release the pending request");

    let reminted = state.begin_frame_apply_session(&request, descriptor);

    assert!(
        reminted.is_ok(),
        "the same frame request should be allowed again after terminal apply failure is retired"
    );
}

#[test]
fn bridge_releases_stale_discarded_session_for_same_request_after_finish() {
    let (mut state, request) = state_with_request("/items/42", 1);
    let descriptor = descriptor("leaf");
    let mut session = state
        .begin_frame_apply_session(&request, descriptor.clone())
        .expect("session should mint");

    let _report = session.discard_stale("renderer stale");
    state
        .finish_frame_apply_session(&session)
        .expect("terminal stale discard should release the pending request");

    let reminted = state.begin_frame_apply_session(&request, descriptor);

    assert!(
        reminted.is_ok(),
        "the same frame request should be allowed again after terminal stale discard is retired"
    );
}

#[test]
fn bridge_keeps_apply_success_session_pending_until_later_finalization() {
    let (mut state, request) = state_with_request("/items/42", 1);
    let descriptor = descriptor("leaf");
    let mut session = state
        .begin_frame_apply_session(&request, descriptor.clone())
        .expect("session should mint");

    let _report = session.record_apply_success(Some("renderer-output"));

    assert_eq!(
        state.finish_frame_apply_session(&session),
        Err(FinishFrameApplySessionError::SessionNotTerminal)
    );
    assert_eq!(
        state.begin_frame_apply_session(&request, descriptor),
        Err(BeginFrameApplySessionError::RequestAlreadyPending)
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
    let (mut state, request) = state_with_request("/items/42", 1);
    let foreign_request = issued_request("/items/42", 0);
    let descriptor = descriptor("leaf");
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
    let (mut state, request) = state_with_request("/items/42", 1);
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
    let (mut state, request) = state_with_request("/items/42", 1);
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
    let (mut state, request) = state_with_request("/items/42", 1);
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
    let (mut state, request) = state_with_request("/items/42", 1);
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

#[test]
fn renderer_success_is_not_active_until_ecs_finalization_and_core_commit() {
    let mut state = RouterEcsState::new();
    let snapshot = state_navigation(&mut state, "/");
    let request = snapshot.requests()[0].clone();
    let descriptor = descriptor("root");
    let mut session = state
        .begin_frame_apply_session(&request, descriptor)
        .expect("session should mint from state-issued request");
    let mut world = World::new();
    let (_host, _slot, root, _child) = mount_frame_root(&mut world, &state, &request);

    session.record_apply_success(Some("renderer-output"));

    assert_eq!(session.status(), Some(EcsFrameApplyStatus::Pending));
    assert!(world.get::<InteractionDisabled>(root).is_none());

    let report = state
        .complete_frame_apply_session(&mut world, &mut session, Some(root), &[])
        .expect("single successful request should finalize and commit");

    assert_eq!(report.status(), EcsFrameApplyStatus::Finalized);
    assert!(session.finalized());
    assert_active_root(&world, root);
}

#[test]
fn renderer_build_failure_reports_core_failure_without_partial_output_or_host_cleanup() {
    let mut state = RouterEcsState::new();
    let snapshot = state_navigation(&mut state, "/");
    let request = snapshot.requests()[0].clone();
    let mut session = state
        .begin_frame_apply_session(&request, descriptor("root"))
        .expect("session should mint");
    let mut world = World::new();
    let host = world.spawn((state.route_host(),)).id();

    session.record_build_failure("renderer build failed");
    let report = state
        .complete_frame_apply_session(&mut world, &mut session, None, &[])
        .expect("build failure should report and acknowledge core cleanup");

    assert_eq!(report.status(), EcsFrameApplyStatus::Failed);
    assert!(report.partial_output().is_none());
    assert!(session.finalized());
    assert!(world.get_entity(host).is_ok(), "app-owned host must remain");
    assert!(
        state.begin_first_navigation(&single_match("/")).is_ok(),
        "failed navigation cleanup acknowledgement should unblock later navigation"
    );
}

#[test]
fn renderer_apply_failure_cleans_partial_router_owned_output_and_preserves_host() {
    let mut state = RouterEcsState::new();
    let snapshot = state_navigation(&mut state, "/");
    let request = snapshot.requests()[0].clone();
    let mut session = state
        .begin_frame_apply_session(&request, descriptor("root"))
        .expect("session should mint");
    let mut world = World::new();
    let (host, _slot, root, child) = mount_frame_root(&mut world, &state, &request);

    session.record_apply_failure("renderer apply failed", Some("renderer-output"));
    let report = state
        .complete_frame_apply_session(&mut world, &mut session, Some(root), &[])
        .expect("apply failure should report, cleanup, and acknowledge");

    assert_eq!(report.status(), EcsFrameApplyStatus::Failed);
    assert!(session.finalized());
    assert!(world.get_entity(host).is_ok(), "app-owned host must remain");
    assert!(
        world.get_entity(root).is_err(),
        "partial root should be removed"
    );
    assert!(
        world.get_entity(child).is_err(),
        "router-owned partial subtree should be removed"
    );
}

#[test]
fn renderer_stale_result_is_discarded_without_core_commit_or_visible_output() {
    let mut state = RouterEcsState::new();
    let snapshot = state_navigation(&mut state, "/");
    let request = snapshot.requests()[0].clone();
    let mut session = state
        .begin_frame_apply_session(&request, descriptor("root"))
        .expect("session should mint");
    let mut world = World::new();
    let (host, _slot, root, _child) = mount_frame_root(&mut world, &state, &request);

    session.discard_stale("renderer stale");
    let report = state
        .complete_frame_apply_session(&mut world, &mut session, Some(root), &[])
        .expect("renderer-local stale should be discarded locally");

    assert_eq!(report.status(), EcsFrameApplyStatus::StaleDiscarded);
    assert!(session.finalized());
    assert!(world.get_entity(host).is_ok(), "app-owned host must remain");
    assert!(
        world.get_entity(root).is_err(),
        "stale partial root should be removed"
    );
    assert_eq!(
        state.begin_first_navigation(&single_match("/")),
        Err(bevy_ui_router_core::runtime::BeginNavigationError::PendingNavigation),
        "renderer-local stale must not report to core or clear the pending navigation"
    );
}

#[test]
fn foreign_view_report_session_is_rejected_before_core_report() {
    bridge_rejects_view_report_from_foreign_frame_request();
}

#[test]
fn duplicate_same_request_view_report_session_is_rejected_before_core_report() {
    let (mut state, request) = state_with_request("/items/42", 1);
    let descriptor = descriptor("leaf");
    let session = state
        .begin_frame_apply_session(&request, descriptor.clone())
        .expect("session should mint");
    let duplicate = ViewReportSession::from_frame_request(&request, descriptor);

    assert!(
        !session.accepts_report_session(&duplicate),
        "same frame request through a different report session must be rejected"
    );
}

#[test]
fn unowned_frame_request_is_rejected_before_core_report() {
    let request = issued_request("/items/42", 1);
    let mut state = RouterEcsState::new();

    assert_eq!(
        state.begin_frame_apply_session(&request, descriptor("leaf")),
        Err(BeginFrameApplySessionError::RequestNotPending)
    );
}

#[test]
fn mismatched_root_identity_is_rejected_before_core_report_or_inert_preparation() {
    let mut state = RouterEcsState::new();
    let snapshot = state_navigation(&mut state, "/items/42");
    assert!(
        snapshot.requests().len() >= 2,
        "nested route should issue multiple frame requests"
    );
    let request = snapshot.requests()[0].clone();
    let mismatched_request = snapshot.requests()[1].clone();
    let mut session = state
        .begin_frame_apply_session(&request, descriptor("root"))
        .expect("session should mint from the state-owned request");
    let mut world = World::new();
    let (host, _slot, root, child) =
        mount_frame_root_for_request(&mut world, &state, &mismatched_request);

    session.record_apply_success(Some("renderer-output"));
    let error = state
        .complete_frame_apply_session(&mut world, &mut session, Some(root), &[])
        .expect_err("root identity mismatch should be rejected before core report");

    assert_eq!(error, CompleteFrameApplySessionError::InvalidRoot);
    assert!(session.finalized());
    assert!(world.get_entity(host).is_ok(), "app-owned host must remain");
    assert!(
        world.get_entity(root).is_err(),
        "mismatched unclaimed root should be cleaned"
    );
    assert!(
        world.get_entity(child).is_err(),
        "mismatched unclaimed subtree should be cleaned"
    );
    assert_eq!(
        state.begin_first_navigation(&single_match("/")),
        Err(bevy_ui_router_core::runtime::BeginNavigationError::PendingNavigation),
        "invalid root must not be reported to core or clear the pending navigation"
    );
    state
        .finish_frame_apply_session(&session)
        .expect("locally rejected invalid root should release ecs pending state");
}

#[test]
fn multiple_frame_requests_report_and_cleanup_in_branch_index_order() {
    let mut state = RouterEcsState::new();
    let snapshot = state_navigation(&mut state, "/items/42");
    assert!(
        snapshot.requests().len() >= 2,
        "nested route should issue multiple frame requests"
    );
    let requests = snapshot.requests().to_vec();
    let mut sessions = requests
        .iter()
        .enumerate()
        .map(|(index, request)| {
            state
                .begin_frame_apply_session(request, descriptor(&format!("frame-{index}")))
                .expect("session should mint")
        })
        .collect::<Vec<_>>();
    let mut world = World::new();
    let roots = requests
        .iter()
        .rev()
        .map(|request| {
            let (_host, _slot, root, _child) = mount_frame_root(&mut world, &state, request);
            root
        })
        .collect::<Vec<_>>();

    for reverse_index in 0..sessions.len() {
        let session_index = sessions.len() - 1 - reverse_index;
        sessions[session_index].record_apply_success(Some(format!("frame-{session_index}")));
        let report = state
            .complete_frame_apply_session(
                &mut world,
                &mut sessions[session_index],
                Some(roots[reverse_index]),
                &[],
            )
            .expect("reported requests should be accepted by core");

        if reverse_index + 1 == sessions.len() {
            assert_eq!(report.status(), EcsFrameApplyStatus::Finalized);
        } else {
            assert_eq!(report.status(), EcsFrameApplyStatus::ReportedToCore);
            assert_inert_root(&world, roots[reverse_index]);
        }
    }

    for root in roots {
        assert_active_root(&world, root);
    }
    for session in &sessions {
        assert_eq!(session.status(), Some(EcsFrameApplyStatus::Finalized));
        assert!(session.finalized());
    }
}

#[test]
fn reported_session_cannot_finish_before_navigation_end_but_can_after_commit() {
    let mut state = RouterEcsState::new();
    let snapshot = state_navigation(&mut state, "/items/42");
    assert!(
        snapshot.requests().len() >= 2,
        "nested route should issue multiple frame requests"
    );
    let requests = snapshot.requests().to_vec();
    let mut sessions = requests
        .iter()
        .enumerate()
        .map(|(index, request)| {
            state
                .begin_frame_apply_session(request, descriptor(&format!("frame-{index}")))
                .expect("session should mint")
        })
        .collect::<Vec<_>>();
    let mut world = World::new();
    let roots = requests
        .iter()
        .map(|request| {
            let (_host, _slot, root, _child) = mount_frame_root(&mut world, &state, request);
            root
        })
        .collect::<Vec<_>>();

    sessions[0].record_apply_success(Some("frame-0"));
    let first_report = state
        .complete_frame_apply_session(&mut world, &mut sessions[0], Some(roots[0]), &[])
        .expect("first report should reach core");
    assert_eq!(first_report.status(), EcsFrameApplyStatus::ReportedToCore);
    assert_eq!(
        state.finish_frame_apply_session(&sessions[0]),
        Err(FinishFrameApplySessionError::SessionNotTerminal),
        "reported roots must stay tracked until navigation is committed or failed"
    );

    for index in 1..sessions.len() {
        sessions[index].record_apply_success(Some(format!("frame-{index}")));
        state
            .complete_frame_apply_session(&mut world, &mut sessions[index], Some(roots[index]), &[])
            .expect("remaining reports should reach core");
    }

    for root in &roots {
        assert_active_root(&world, *root);
    }
    assert!(
        state.begin_first_navigation(&single_match("/")).is_ok(),
        "a later navigation may begin before old reported sessions are explicitly finished"
    );
    for session in &sessions {
        state
            .finish_frame_apply_session(session)
            .expect("reported sessions should release after navigation ends");
    }
}

fn descriptor(binding: &str) -> RouteViewDescriptor {
    RouteViewDescriptor::new(ViewBindingKey::new(binding))
}

fn mount_frame_root(
    world: &mut World,
    state: &RouterEcsState,
    request: &FrameApplyRequest,
) -> (Entity, Entity, Entity, Entity) {
    mount_frame_root_for_request(world, state, request)
}

fn mount_frame_root_for_request(
    world: &mut World,
    state: &RouterEcsState,
    request: &FrameApplyRequest,
) -> (Entity, Entity, Entity, Entity) {
    let host = world.spawn((state.route_host(),)).id();
    let slot = world.spawn((state.route_slot(),)).id();
    let root = world
        .spawn((
            RouteFrameRoot::from_frame_request(request),
            Visibility::Visible,
            Pickable::default(),
        ))
        .id();
    let child = world.spawn((Visibility::Visible, Pickable::default())).id();

    world.entity_mut(host).add_child(slot);
    world.entity_mut(slot).add_child(root);
    world.entity_mut(root).add_child(child);

    (host, slot, root, child)
}

fn assert_inert_root(world: &World, root: Entity) {
    assert_eq!(world.get::<Visibility>(root), Some(&Visibility::Hidden));
    assert_eq!(world.get::<Pickable>(root), Some(&Pickable::IGNORE));
    assert!(world.get::<InteractionDisabled>(root).is_some());
}

fn assert_active_root(world: &World, root: Entity) {
    assert!(world.get_entity(root).is_ok(), "active root should exist");
    assert_eq!(world.get::<Visibility>(root), Some(&Visibility::Visible));
    assert_eq!(world.get::<Pickable>(root), Some(&Pickable::default()));
    assert!(world.get::<InteractionDisabled>(root).is_none());
}

fn state_navigation(
    state: &mut RouterEcsState,
    path: &str,
) -> bevy_ui_router_core::transaction::NavigationTransactionSnapshot {
    state
        .begin_first_navigation(&single_match(path))
        .expect("state runtime should begin navigation")
}

fn single_match(path: &str) -> bevy_ui_router_core::branch::MatchedBranch {
    RouteMatcher::new(route_tree())
        .match_location(&RouteLocation::new(path))
        .unwrap()
}

fn state_with_request(path: &str, request_index: usize) -> (RouterEcsState, FrameApplyRequest) {
    let mut state = RouterEcsState::new();
    let snapshot = state_navigation(&mut state, path);
    (state, snapshot.requests()[request_index].clone())
}

fn issued_request(path: &str, request_index: usize) -> FrameApplyRequest {
    let target = RouteMatcher::new(route_tree())
        .match_location(&RouteLocation::new(path))
        .unwrap();
    let mut runtime = RouterRuntime::new();

    runtime.begin_first_navigation(&target).unwrap().requests()[request_index].clone()
}

fn route_tree() -> RouteTree {
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
