#![cfg(feature = "ecs")]

use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

use bevy_app::App;
use bevy_ecs::{entity::Entity, hierarchy::ChildOf, prelude::Component};
use bevy_ui::{BackgroundColor, Node, Val};
use bevy_ui_router::prelude::*;

const NORMAL_RECT_WIDTH: u16 = 31;
const FALLBACK_RECT_WIDTH: u16 = 79;
const PARTIAL_RECT_WIDTH: u16 = 199;

#[derive(Component, Debug, Eq, PartialEq)]
struct HostSentinel(u32);

#[derive(Component, Debug, Eq, PartialEq)]
struct ExistingChild(u32);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FixtureMode {
    Success { width: u16 },
    BuildFailure,
    ApplyFailure,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct EffectRecord {
    view: RouteViewKey,
    location: RouteLocation,
    fragment: Option<String>,
    origin: FrameOrigin,
    failure: Option<NavigationFailureKind>,
}

#[derive(Default)]
struct FixtureState {
    modes: Vec<(RouteViewKey, FixtureMode)>,
    effects: Vec<EffectRecord>,
}

#[derive(Clone, Default)]
struct FixtureControl {
    state: Arc<Mutex<FixtureState>>,
}

impl FixtureControl {
    fn set_mode(&self, view: &RouteViewKey, mode: FixtureMode) {
        let mut state = self
            .state
            .lock()
            .expect("fixture lock must not be poisoned");
        if let Some((_, stored)) = state.modes.iter_mut().find(|(key, _)| key == view) {
            *stored = mode;
        } else {
            state.modes.push((view.clone(), mode));
        }
    }

    fn mode_and_record(&self, effect: &ViewBuildEffect) -> FixtureMode {
        let mut state = self
            .state
            .lock()
            .expect("fixture lock must not be poisoned");
        let mode = state
            .modes
            .iter()
            .find_map(|(key, mode)| (key == effect.view()).then_some(*mode))
            .expect("every fixture view must have a mode");
        state.effects.push(EffectRecord {
            view: effect.view().clone(),
            location: effect.input().location().clone(),
            fragment: effect.input().fragment().map(str::to_owned),
            origin: effect.input().origin().clone(),
            failure: effect.input().failure().map(NavigationFailure::kind),
        });
        mode
    }

    fn effects(&self) -> Vec<EffectRecord> {
        self.state
            .lock()
            .expect("fixture lock must not be poisoned")
            .effects
            .clone()
    }
}

struct FixtureDriver {
    key: ViewDriverKey,
    control: FixtureControl,
}

impl EcsViewDriver for FixtureDriver {
    fn key(&self) -> ViewDriverKey {
        self.key.clone()
    }

    fn apply(
        &mut self,
        effect: &ViewBuildEffect,
        content: &mut FrameContentWriter<'_>,
        report: EcsViewReportSession<'_>,
    ) -> ViewBuildReport {
        match self.control.mode_and_record(effect) {
            FixtureMode::Success { width } => {
                stage_rect(content, width);
                report.applied()
            }
            FixtureMode::BuildFailure => report.build_failed("fixture build failure"),
            FixtureMode::ApplyFailure => {
                stage_rect(content, PARTIAL_RECT_WIDTH);
                report.apply_failed("fixture apply failure after partial plan")
            }
        }
    }
}

fn stage_rect(content: &mut FrameContentWriter<'_>, width: u16) {
    let container = content
        .spawn_empty()
        .expect("fixture container should stage");
    let rect = content
        .spawn_solid_rect(
            SolidRect::new(width, 23, ContentColor::srgba8(17, 43, 89, 255))
                .expect("fixture rectangle should be valid"),
        )
        .expect("fixture rectangle should stage");
    content
        .add_child(container, rect)
        .expect("fixture content edge should stage");
}

struct RootFixture {
    app: App,
    router: RouterHandle,
    control: FixtureControl,
    normal: RouteViewKey,
    fallback: RouteViewKey,
    host: Entity,
    existing_child: Entity,
    admission_entities: HashSet<Entity>,
}

fn root_fixture(pattern: &str) -> RootFixture {
    let mut app = App::new();
    app.add_plugins(BevyUiRouterPlugin::default());

    let normal = RouteViewKey::new("fixture-normal").unwrap();
    let fallback = RouteViewKey::new("fixture-fallback").unwrap();
    let control = FixtureControl::default();
    control.set_mode(
        &normal,
        FixtureMode::Success {
            width: NORMAL_RECT_WIDTH,
        },
    );
    control.set_mode(
        &fallback,
        FixtureMode::Success {
            width: FALLBACK_RECT_WIDTH,
        },
    );

    let driver = app
        .register_ui_view_driver(FixtureDriver {
            key: ViewDriverKey::new("fixture-driver").unwrap(),
            control: control.clone(),
        })
        .unwrap();
    let routes = RouteTree::builder()
        .root_with_options(
            RoutePattern::path(pattern).unwrap(),
            RouteNodeOptions::new().with_view(normal.clone()),
        )
        .unwrap()
        .build()
        .unwrap();
    let mut views = RouteViewRegistry::new();
    for view in [normal.clone(), fallback.clone()] {
        views
            .insert(view, RouteViewDescriptor::new(driver.clone()))
            .unwrap();
    }
    let router = app
        .add_ui_router(RouterConfig::new(routes, views, fallback.clone()))
        .unwrap();

    let host = app
        .world_mut()
        .spawn((
            Node {
                width: Val::Px(640.0),
                height: Val::Px(360.0),
                ..Node::default()
            },
            HostSentinel(7),
            RouterHostRegistration::new(router),
        ))
        .id();
    let existing_child = app
        .world_mut()
        .spawn((
            Node {
                width: Val::Px(13.0),
                height: Val::Px(17.0),
                ..Node::default()
            },
            ExistingChild(11),
            ChildOf(host),
        ))
        .id();
    app.update();
    let admission_entities = entity_set(&app);

    RootFixture {
        app,
        router,
        control,
        normal,
        fallback,
        host,
        existing_child,
        admission_entities,
    }
}

fn push_and_settle(
    app: &mut App,
    router: RouterHandle,
    location: RouteLocation,
    forbidden_rect_width: Option<u16>,
    leaving_entities: Option<&HashSet<Entity>>,
) -> NavigationRequestId {
    let request = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, location)
        .expect("fixture navigation should enqueue");

    for _ in 0..16 {
        app.update();
        if let Some(width) = forbidden_rect_width {
            assert!(
                node_entities_with_width(app, width).is_empty(),
                "a rejected partial ContentPlan must remain World-zero"
            );
        }
        let (committed, terminal) = {
            let snapshots = app.world().resource::<RouterSnapshots>();
            let RequestLookup::Found(snapshot) = snapshots.request(router, request) else {
                continue;
            };
            (
                snapshot
                    .trace()
                    .iter()
                    .any(|event| event.kind() == NavigationTraceEventKind::CommitPublished),
                matches!(
                    snapshot.status(),
                    RequestStatus::Succeeded | RequestStatus::Failed(_)
                ),
            )
        };
        if committed {
            if let Some(leaving_entities) = leaving_entities {
                let gate = app.world().resource::<RouterInputGate>();
                for entity in leaving_entities {
                    assert!(!gate.allows_pointer(*entity));
                    assert!(!gate.allows_keyboard(*entity));
                    assert!(!gate.allows_gamepad(*entity));
                }
            }
        }
        if terminal {
            return request;
        }
    }
    panic!("fixture request did not settle within 16 App::update calls")
}

fn request_snapshot(
    app: &App,
    router: RouterHandle,
    request: NavigationRequestId,
) -> NavigationRequestSnapshot {
    let snapshots = app.world().resource::<RouterSnapshots>();
    let RequestLookup::Found(snapshot) = snapshots.request(router, request) else {
        panic!("completed fixture request must remain retained")
    };
    snapshot.clone()
}

fn router_snapshot(app: &App, router: RouterHandle) -> RouterSnapshot {
    app.world()
        .resource::<RouterSnapshots>()
        .get(router)
        .expect("installed router must have a snapshot")
        .clone()
}

fn assert_succeeded(snapshot: &NavigationRequestSnapshot) {
    assert_eq!(snapshot.status(), &RequestStatus::Succeeded);
    assert_eq!(
        snapshot.trace().last().map(NavigationTraceEvent::kind),
        Some(NavigationTraceEventKind::RequestTerminal)
    );
}

fn assert_failed(snapshot: &NavigationRequestSnapshot, expected: NavigationFailureKind) {
    assert!(matches!(
        snapshot.status(),
        RequestStatus::Failed(failure) if failure.kind() == expected
    ));
    assert_eq!(
        snapshot.trace().last().map(NavigationTraceEvent::kind),
        Some(NavigationTraceEventKind::RequestTerminal)
    );
}

fn assert_accepted_report_diagnostic(
    snapshot: &NavigationRequestSnapshot,
    request: NavigationRequestId,
) {
    assert!(snapshot.diagnostics().iter().any(|event| {
        event.request() == Some(request)
            && event.frame().is_some()
            && event.report_disposition() == Some(ReportDisposition::Accepted)
            && !event.redacted_message().is_empty()
    }));
}

fn assert_ready_host_snapshot(snapshot: &RouterSnapshot) {
    let host = snapshot.host();
    assert!(host.version().is_some());
    assert!(host.top_slot().is_some());
    assert_eq!(
        host.trace()
            .iter()
            .map(HostTraceEvent::kind)
            .collect::<Vec<_>>(),
        vec![
            HostTraceEventKind::AdmissionRequested,
            HostTraceEventKind::HostGranted,
            HostTraceEventKind::TopSlotRegistered,
            HostTraceEventKind::HostReady,
        ]
    );
}

fn assert_cleanup_trace_is_ordered(snapshot: &NavigationRequestSnapshot, expect_receipts: bool) {
    let trace = snapshot.trace();
    let commit = trace
        .iter()
        .position(|event| event.kind() == NavigationTraceEventKind::CommitPublished)
        .expect("commit must be traced");
    let cleanup_phase = trace
        .iter()
        .enumerate()
        .skip(commit + 1)
        .find_map(|(index, event)| {
            (event.kind() == NavigationTraceEventKind::RequestPhaseChanged
                && event.phase() == Some(&NavigationPhase::PostCommitCleanup))
            .then_some(index)
        })
        .expect("post-commit cleanup phase must be traced");
    let aggregate = trace
        .iter()
        .enumerate()
        .skip(cleanup_phase + 1)
        .find_map(|(index, event)| {
            (event.kind() == NavigationTraceEventKind::CleanupSettled
                && event.cleanup_disposition().is_none())
            .then_some(index)
        })
        .expect("aggregate cleanup settlement must be traced");
    let receipts = trace[cleanup_phase + 1..aggregate]
        .iter()
        .filter(|event| {
            event.kind() == NavigationTraceEventKind::CleanupSettled
                && event.cleanup_disposition().is_some()
        })
        .count();
    let terminal = trace
        .iter()
        .position(|event| event.kind() == NavigationTraceEventKind::RequestTerminal)
        .expect("terminal transition must be traced");

    assert!(commit < cleanup_phase);
    assert_eq!(receipts > 0, expect_receipts);
    assert!(cleanup_phase < aggregate && aggregate < terminal);
    assert_eq!(terminal + 1, trace.len());
}

fn assert_host_intact(app: &App, host: Entity, existing_child: Entity) {
    assert_eq!(
        app.world().get::<HostSentinel>(host),
        Some(&HostSentinel(7))
    );
    assert_eq!(
        app.world().get::<ExistingChild>(existing_child),
        Some(&ExistingChild(11))
    );
    assert_eq!(
        app.world()
            .get::<ChildOf>(existing_child)
            .map(ChildOf::parent),
        Some(host)
    );
    let host_node = app
        .world()
        .get::<Node>(host)
        .expect("host Node must remain");
    assert_eq!(host_node.width, Val::Px(640.0));
    assert_eq!(host_node.height, Val::Px(360.0));
    let child_node = app
        .world()
        .get::<Node>(existing_child)
        .expect("existing child Node must remain");
    assert_eq!(child_node.width, Val::Px(13.0));
    assert_eq!(child_node.height, Val::Px(17.0));
}

fn entity_set(app: &App) -> HashSet<Entity> {
    app.world()
        .iter_entities()
        .map(|entity| entity.id())
        .collect()
}

fn node_entities_with_width(app: &mut App, width: u16) -> Vec<Entity> {
    let world = app.world_mut();
    let mut query = world.query::<(Entity, &Node)>();
    query
        .iter(world)
        .filter_map(|(entity, node)| (node.width == Val::Px(f32::from(width))).then_some(entity))
        .collect()
}

fn assert_real_solid_rect(app: &mut App, width: u16) {
    let entities = node_entities_with_width(app, width);
    assert_eq!(entities.len(), 1);
    assert_ne!(
        app.world()
            .get::<BackgroundColor>(entities[0])
            .expect("a realized SolidRect must carry BackgroundColor"),
        &BackgroundColor::default()
    );
    let gate = app.world().resource::<RouterInputGate>();
    assert!(gate.allows_pointer(entities[0]));
    assert!(gate.allows_keyboard(entities[0]));
    assert!(gate.allows_gamepad(entities[0]));
}

#[test]
fn root_push_first_equivalent_fragment_and_changed_root_settle_with_physical_cleanup() {
    let mut fixture = root_fixture("*page");

    let first_request_id = push_and_settle(
        &mut fixture.app,
        fixture.router,
        RouteLocation::new("/alpha"),
        None,
        None,
    );
    let first_request = request_snapshot(&fixture.app, fixture.router, first_request_id);
    assert_succeeded(&first_request);
    assert_accepted_report_diagnostic(&first_request, first_request_id);
    assert_cleanup_trace_is_ordered(&first_request, false);
    let first_router = router_snapshot(&fixture.app, fixture.router);
    assert_ready_host_snapshot(&first_router);
    assert!(!first_router.cleanup().foreground_pending());
    assert_eq!(first_router.cleanup().quarantine_count(), 0);
    assert_eq!(first_router.cleanup().last_disposition(), None);
    assert_eq!(first_router.active().kind(), ActivePresentationKind::Route);
    assert_eq!(
        first_router.active().location(),
        Some(&RouteLocation::new("/alpha"))
    );
    assert_eq!(first_router.history().len(), 1);
    assert_eq!(
        first_router.history().current(),
        Some(&RouteLocation::new("/alpha"))
    );
    assert_eq!(first_router.active().frames().len(), 1);
    let first_frame = first_router.active().frames()[0].clone();
    let first_entities = entity_set(&fixture.app)
        .difference(&fixture.admission_entities)
        .copied()
        .collect::<HashSet<_>>();
    assert!(!first_entities.is_empty());
    assert_real_solid_rect(&mut fixture.app, NORMAL_RECT_WIDTH);
    assert_host_intact(&fixture.app, fixture.host, fixture.existing_child);

    let effects_before_equivalent = fixture.control.effects().len();
    let entities_before_equivalent = entity_set(&fixture.app);
    let equivalent_request_id = push_and_settle(
        &mut fixture.app,
        fixture.router,
        RouteLocation::new("alpha//"),
        None,
        None,
    );
    let equivalent_request = request_snapshot(&fixture.app, fixture.router, equivalent_request_id);
    assert_succeeded(&equivalent_request);
    assert_cleanup_trace_is_ordered(&equivalent_request, false);
    let equivalent_router = router_snapshot(&fixture.app, fixture.router);
    assert_eq!(fixture.control.effects().len(), effects_before_equivalent);
    assert_eq!(entity_set(&fixture.app), entities_before_equivalent);
    assert_eq!(equivalent_router.active().frames(), &[first_frame.clone()]);
    assert_eq!(equivalent_router.history().len(), 2);

    let fragment_request_id = push_and_settle(
        &mut fixture.app,
        fixture.router,
        RouteLocation::new("/alpha").with_fragment("details"),
        None,
        Some(&first_entities),
    );
    let fragment_request = request_snapshot(&fixture.app, fixture.router, fragment_request_id);
    assert_succeeded(&fragment_request);
    assert_accepted_report_diagnostic(&fragment_request, fragment_request_id);
    assert_cleanup_trace_is_ordered(&fragment_request, true);
    let fragment_router = router_snapshot(&fixture.app, fixture.router);
    let fragment_frame = fragment_router.active().frames()[0].clone();
    assert_eq!(fragment_frame.frame(), first_frame.frame());
    assert_eq!(
        fragment_frame.generation().raw(),
        first_frame.generation().raw() + 1
    );
    assert_eq!(fragment_router.history().len(), 3);
    assert_eq!(
        fragment_router.history().current(),
        Some(&RouteLocation::new("/alpha").with_fragment("details"))
    );
    assert!(
        first_entities
            .iter()
            .all(|entity| fixture.app.world().get_entity(*entity).is_err()),
        "fragment remount must physically close the previous frame closure"
    );
    let fragment_entities = entity_set(&fixture.app)
        .difference(&fixture.admission_entities)
        .copied()
        .collect::<HashSet<_>>();

    let changed_request_id = push_and_settle(
        &mut fixture.app,
        fixture.router,
        RouteLocation::new("/beta"),
        None,
        Some(&fragment_entities),
    );
    let changed_request = request_snapshot(&fixture.app, fixture.router, changed_request_id);
    assert_succeeded(&changed_request);
    assert_accepted_report_diagnostic(&changed_request, changed_request_id);
    assert_cleanup_trace_is_ordered(&changed_request, true);
    let changed_router = router_snapshot(&fixture.app, fixture.router);
    let changed_frame = &changed_router.active().frames()[0];
    assert_ne!(changed_frame.frame(), fragment_frame.frame());
    assert_eq!(changed_frame.generation().raw(), 0);
    assert_eq!(changed_router.history().len(), 4);
    assert!(!changed_router.cleanup().foreground_pending());
    assert_eq!(changed_router.cleanup().quarantine_count(), 0);
    assert_eq!(
        changed_router.cleanup().last_disposition(),
        Some(CleanupDisposition::Removed)
    );
    assert_eq!(
        changed_router.history().current(),
        Some(&RouteLocation::new("/beta"))
    );
    assert!(
        fragment_entities
            .iter()
            .all(|entity| fixture.app.world().get_entity(*entity).is_err()),
        "changed-root replacement must physically close the old frame closure"
    );
    assert_real_solid_rect(&mut fixture.app, NORMAL_RECT_WIDTH);

    let effects = fixture.control.effects();
    assert_eq!(effects.len(), 3, "equivalent push must be driver-zero");
    assert_eq!(effects[0].location, RouteLocation::new("/alpha"));
    assert_eq!(effects[1].fragment.as_deref(), Some("details"));
    assert_eq!(effects[2].location, RouteLocation::new("/beta"));
    assert_host_intact(&fixture.app, fixture.host, fixture.existing_child);
}

#[test]
fn root_push_match_failure_commits_router_fallback_without_history() {
    let mut fixture = root_fixture("/known");
    let requested = RouteLocation::new("/missing");
    let request_id = push_and_settle(
        &mut fixture.app,
        fixture.router,
        requested.clone(),
        None,
        None,
    );
    let request = request_snapshot(&fixture.app, fixture.router, request_id);
    assert_failed(&request, NavigationFailureKind::Match);
    assert_accepted_report_diagnostic(&request, request_id);
    assert!(request.diagnostics().iter().any(|event| {
        event.severity() == DiagnosticSeverity::Error
            && event.request().is_none()
            && event.frame().is_none()
            && event.report_disposition().is_none()
            && !event.redacted_message().is_empty()
    }));
    assert_cleanup_trace_is_ordered(&request, false);
    assert!(request.matched().is_none());

    let router = router_snapshot(&fixture.app, fixture.router);
    assert_eq!(router.active().kind(), ActivePresentationKind::Error);
    assert_eq!(router.active().requested(), Some(&requested));
    assert!(router.active().matched().is_none());
    assert_eq!(
        router.active().failure().map(NavigationFailure::kind),
        Some(NavigationFailureKind::Match)
    );
    assert_eq!(router.active().frames().len(), 1);
    assert_eq!(
        router.active().frames()[0].origin(),
        &FrameOrigin::RouterFallback
    );
    assert_eq!(router.history().len(), 0);
    assert!(router.history().current().is_none());
    assert!(router.requested().is_none());
    assert!(router.matched().is_none());
    assert!(router.pending_phase().is_none());
    assert!(router.running_request().is_none());
    assert_real_solid_rect(&mut fixture.app, FALLBACK_RECT_WIDTH);

    let effects = fixture.control.effects();
    assert_eq!(effects.len(), 1);
    assert_eq!(effects[0].view, fixture.fallback);
    assert_eq!(effects[0].origin, FrameOrigin::RouterFallback);
    assert_eq!(effects[0].failure, Some(NavigationFailureKind::Match));
    assert_host_intact(&fixture.app, fixture.host, fixture.existing_child);
}

#[test]
fn root_push_build_and_apply_failures_preserve_match_and_commit_error_presentation() {
    for (mode, expected) in [
        (FixtureMode::BuildFailure, NavigationFailureKind::Build),
        (FixtureMode::ApplyFailure, NavigationFailureKind::Apply),
    ] {
        let mut fixture = root_fixture("/known/:id");
        let first = push_and_settle(
            &mut fixture.app,
            fixture.router,
            RouteLocation::new("/known/one"),
            None,
            None,
        );
        let first = request_snapshot(&fixture.app, fixture.router, first);
        assert_succeeded(&first);
        let old_matched = first
            .matched()
            .expect("the prior request must retain its matched fingerprint")
            .clone();
        let old_entities = entity_set(&fixture.app);
        fixture.control.set_mode(&fixture.normal, mode);

        let requested = RouteLocation::new("/known/two");
        let failed_id = push_and_settle(
            &mut fixture.app,
            fixture.router,
            requested.clone(),
            (mode == FixtureMode::ApplyFailure).then_some(PARTIAL_RECT_WIDTH),
            Some(&old_entities),
        );
        let failed = request_snapshot(&fixture.app, fixture.router, failed_id);
        assert_failed(&failed, expected);
        assert_accepted_report_diagnostic(&failed, failed_id);
        let matched = failed
            .matched()
            .expect("post-match failure must retain its fingerprint")
            .clone();
        assert_ne!(matched, old_matched);
        assert_cleanup_trace_is_ordered(&failed, true);

        let router = router_snapshot(&fixture.app, fixture.router);
        assert_eq!(router.active().kind(), ActivePresentationKind::Error);
        assert_eq!(router.active().requested(), Some(&requested));
        assert_eq!(router.active().matched(), Some(&matched));
        assert_eq!(
            router.active().failure().map(NavigationFailure::kind),
            Some(expected)
        );
        assert_eq!(
            router.active().frames()[0].origin(),
            &FrameOrigin::RouterFallback
        );
        assert_eq!(router.history().len(), 1);
        assert_eq!(
            router.history().current(),
            Some(&RouteLocation::new("/known/one"))
        );
        assert!(node_entities_with_width(&mut fixture.app, PARTIAL_RECT_WIDTH).is_empty());
        assert_real_solid_rect(&mut fixture.app, FALLBACK_RECT_WIDTH);

        let effects = fixture.control.effects();
        let recovery = effects.last().expect("fallback effect must run");
        assert_eq!(recovery.view, fixture.fallback);
        assert_eq!(recovery.origin, FrameOrigin::RouterFallback);
        assert_eq!(recovery.failure, Some(expected));
        assert_host_intact(&fixture.app, fixture.host, fixture.existing_child);
    }
}

#[test]
fn root_push_fallback_failure_preserves_old_state_and_does_not_poison_the_queue() {
    let mut fixture = root_fixture("/known/:id");
    let first = push_and_settle(
        &mut fixture.app,
        fixture.router,
        RouteLocation::new("/known/one"),
        None,
        None,
    );
    assert_succeeded(&request_snapshot(&fixture.app, fixture.router, first));
    let before = router_snapshot(&fixture.app, fixture.router);
    let old_active = before.active().clone();
    let old_history = before.history().clone();
    let old_entities = entity_set(&fixture.app);

    fixture
        .control
        .set_mode(&fixture.normal, FixtureMode::ApplyFailure);
    fixture
        .control
        .set_mode(&fixture.fallback, FixtureMode::BuildFailure);
    let failed_id = push_and_settle(
        &mut fixture.app,
        fixture.router,
        RouteLocation::new("/known/two"),
        Some(PARTIAL_RECT_WIDTH),
        None,
    );
    let failed = request_snapshot(&fixture.app, fixture.router, failed_id);
    assert_failed(&failed, NavigationFailureKind::RecoveryFailed);
    assert_accepted_report_diagnostic(&failed, failed_id);

    let after_failure = router_snapshot(&fixture.app, fixture.router);
    assert_eq!(after_failure.active(), &old_active);
    assert_eq!(after_failure.history(), &old_history);
    assert_eq!(entity_set(&fixture.app), old_entities);
    assert!(node_entities_with_width(&mut fixture.app, PARTIAL_RECT_WIDTH).is_empty());
    assert_real_solid_rect(&mut fixture.app, NORMAL_RECT_WIDTH);
    assert_host_intact(&fixture.app, fixture.host, fixture.existing_child);

    fixture.control.set_mode(
        &fixture.normal,
        FixtureMode::Success {
            width: NORMAL_RECT_WIDTH,
        },
    );
    fixture.control.set_mode(
        &fixture.fallback,
        FixtureMode::Success {
            width: FALLBACK_RECT_WIDTH,
        },
    );
    let recovered_id = push_and_settle(
        &mut fixture.app,
        fixture.router,
        RouteLocation::new("/known/two"),
        None,
        Some(&old_entities),
    );
    let recovered_request = request_snapshot(&fixture.app, fixture.router, recovered_id);
    assert_succeeded(&recovered_request);
    assert_accepted_report_diagnostic(&recovered_request, recovered_id);
    let recovered_router = router_snapshot(&fixture.app, fixture.router);
    assert_eq!(
        recovered_router.active().kind(),
        ActivePresentationKind::Route
    );
    assert_eq!(
        recovered_router.active().location(),
        Some(&RouteLocation::new("/known/two"))
    );
    assert_eq!(recovered_router.history().len(), 2);
    assert_host_intact(&fixture.app, fixture.host, fixture.existing_child);
}
