//! Native focus acceptance through the facade, actual Scene hooks and the
//! official InputFocusPlugin. No test injects Router focus intent or readiness.

use bevy_app::{App, PostUpdate, TaskPoolPlugin};
use bevy_asset::AssetPlugin;
use bevy_ecs::{
    prelude::*,
    schedule::IntoScheduleConfigs,
    template::{Template, TemplateContext},
};
use bevy_input_focus::{
    AutoFocus, FocusCause, FocusGained, FocusLost, InputFocus, InputFocusPlugin, InputFocusSystems,
};
use bevy_scene::{ResolvedScene, SceneFunction, ScenePlugin};
use bevy_ui::Node;
use bevy_ui_router::prelude::*;

#[derive(Clone, Copy, Debug)]
enum Operation {
    SetA,
    SetB,
    Clear,
    BorrowOnly,
}

#[derive(Resource, Clone, Copy)]
struct ApplicationTargets {
    a: Entity,
    b: Entity,
}

fn apply_operations(world: &mut World, operations: &[Operation]) {
    let targets = *world.resource::<ApplicationTargets>();
    let mut focus = world.resource_mut::<InputFocus>();
    for operation in operations {
        match operation {
            Operation::SetA => focus.set(targets.a, FocusCause::Pressed),
            Operation::SetB => focus.set(targets.b, FocusCause::Navigated),
            Operation::Clear => focus.clear(),
            Operation::BorrowOnly => focus.set_changed(),
        }
    }
}

#[derive(Resource, Default)]
struct Attempts {
    targets: Vec<Entity>,
    child_saw_focus: Vec<Option<Entity>>,
}

#[derive(Component)]
struct NativeFocusTarget;

#[derive(Resource, Default)]
struct PlainAutoFocus(bool);

#[derive(Clone)]
struct PlainTemplate;

impl Template for PlainTemplate {
    type Output = ();
    fn build_template(&self, context: &mut TemplateContext<'_, '_>) -> bevy_ecs::error::Result<()> {
        context.entity.insert((Node::default(), DefaultOutlet));
        if context.entity.world().resource::<PlainAutoFocus>().0 {
            let target = context.entity.id();
            context.entity.insert(AutoFocus);
            context
                .entity
                .world_scope(|world| world.resource_mut::<Attempts>().targets.push(target));
        }
        Ok(())
    }
    fn clone_template(&self) -> Self {
        self.clone()
    }
}

#[derive(Clone)]
struct FocusSceneTemplate {
    before: Vec<Operation>,
    after: Vec<Operation>,
    outlet: bool,
    autofocus: bool,
}

impl Template for FocusSceneTemplate {
    type Output = ();
    fn build_template(&self, context: &mut TemplateContext<'_, '_>) -> bevy_ecs::error::Result<()> {
        let entity = context.entity.id();
        context.entity.insert((Node::default(), NativeFocusTarget));
        if self.outlet {
            context.entity.insert(DefaultOutlet);
        }
        context
            .entity
            .world_scope(|world| apply_operations(world, &self.before));
        // This is the real official on_add hook, not a simulated focus request.
        if self.autofocus {
            context.entity.insert(AutoFocus);
        }
        context.entity.world_scope(|world| {
            world.resource_mut::<Attempts>().targets.push(entity);
            apply_operations(world, &self.after);
        });
        Ok(())
    }
    fn clone_template(&self) -> Self {
        self.clone()
    }
}

#[derive(Clone)]
struct ObserveThenFail;
impl Template for ObserveThenFail {
    type Output = ();
    fn build_template(&self, context: &mut TemplateContext<'_, '_>) -> bevy_ecs::error::Result<()> {
        context.entity.insert(Node::default());
        context.entity.world_scope(|world| {
            let actual = world.resource::<InputFocus>().get();
            world
                .resource_mut::<Attempts>()
                .child_saw_focus
                .push(actual);
        });
        Err("native focus child apply failure".into())
    }
    fn clone_template(&self) -> Self {
        self.clone()
    }
}

#[derive(Clone, Debug, PartialEq)]
enum FocusEvent {
    Gained(Entity, FocusCause),
    Lost(Entity),
}

#[derive(Resource, Default)]
struct Events(Vec<FocusEvent>);

// Runs after the first pending frame's official drain and before a later drain.
// A same-target set / already-clear clear is intentionally still an operation.
#[derive(Resource, Default)]
struct LaterOperations {
    update: usize,
    operations: Vec<Operation>,
    applied: bool,
}

fn later_application_operations(world: &mut World) {
    let operations = {
        let mut later = world.resource_mut::<LaterOperations>();
        later.update += 1;
        if later.update == 2 && !later.operations.is_empty() {
            later.applied = true;
            later.operations.clone()
        } else {
            Vec::new()
        }
    };
    apply_operations(world, &operations);
}

struct Fixture {
    app: App,
    router: RouterHandle,
    a: Entity,
    b: Entity,
    prefix: Entity,
}

fn fixture(before: Vec<Operation>, after: Vec<Operation>, fail: bool, nested: bool) -> Fixture {
    fixture_with_autofocus(before, after, fail, nested, true)
}

fn fixture_with_autofocus(
    before: Vec<Operation>,
    after: Vec<Operation>,
    fail: bool,
    nested: bool,
    autofocus: bool,
) -> Fixture {
    fixture_with_plugin(before, after, fail, nested, autofocus, Some(true))
}

fn fixture_with_plugin(
    before: Vec<Operation>,
    after: Vec<Operation>,
    fail: bool,
    nested: bool,
    autofocus: bool,
    plugin_first: Option<bool>,
) -> Fixture {
    let mut app = App::new();
    app.add_plugins((
        TaskPoolPlugin::default(),
        AssetPlugin::default(),
        ScenePlugin,
    ));
    if plugin_first == Some(true) {
        app.add_plugins(InputFocusPlugin);
    }
    app.init_resource::<Attempts>()
        .init_resource::<PlainAutoFocus>()
        .init_resource::<Events>()
        .init_resource::<LaterOperations>();
    app.add_systems(
        PostUpdate,
        later_application_operations.before(InputFocusSystems::FocusChangeEvents),
    );
    app.world_mut()
        .add_observer(|mut event: On<FocusGained>, mut events: ResMut<Events>| {
            events.0.push(FocusEvent::Gained(event.entity, event.cause));
            event.propagate(false);
        });
    app.world_mut()
        .add_observer(|mut event: On<FocusLost>, mut events: ResMut<Events>| {
            events.0.push(FocusEvent::Lost(event.entity));
            event.propagate(false);
        });
    let a = app.world_mut().spawn(Node::default()).id();
    let b = app.world_mut().spawn(Node::default()).id();
    let prefix = app.world_mut().spawn(Node::default()).id();
    app.insert_resource(ApplicationTargets { a, b });
    let driver = app
        .register_bsn_view(ViewDriverKey::new("focus").unwrap(), move |_| {
            let template = FocusSceneTemplate {
                before: before.clone(),
                after: after.clone(),
                outlet: nested,
                autofocus,
            };
            Ok(SceneFunction(move |_, root: &mut ResolvedScene| {
                root.push_bundle_template(template);
                if fail {
                    let mut child = ResolvedScene::default();
                    child.push_bundle_template(ObserveThenFail);
                    root.get_or_insert_related_resolved_scenes::<ChildOf>()
                        .scenes
                        .push(child);
                }
            }))
        })
        .unwrap();
    let plain = app
        .register_bsn_view(ViewDriverKey::new("plain").unwrap(), |_| {
            Ok(SceneFunction(|_, root: &mut ResolvedScene| {
                root.push_bundle_template(PlainTemplate)
            }))
        })
        .unwrap();
    let page = RouteViewKey::new("focus").unwrap();
    let other = RouteViewKey::new("plain").unwrap();
    let mut routes = RouteTree::builder()
        .root_with_options(
            RoutePattern::path("/").unwrap(),
            RouteNodeOptions::new().with_view(page.clone()),
        )
        .unwrap();
    if nested {
        let middle = routes
            .route_with_options(
                routes.root_id().unwrap(),
                RoutePattern::path("middle").unwrap(),
                RouteNodeOptions::new().with_view(other.clone()),
            )
            .unwrap();
        routes
            .route_with_options(
                middle,
                RoutePattern::path("leaf").unwrap(),
                RouteNodeOptions::new().with_view(other.clone()),
            )
            .unwrap();
    }
    let mut views = RouteViewRegistry::new();
    views
        .insert(page, RouteViewDescriptor::new(driver))
        .unwrap();
    views
        .insert(other.clone(), RouteViewDescriptor::new(plain))
        .unwrap();
    let router = app
        .add_ui_router(RouterConfig::new(routes.build().unwrap(), views, other))
        .unwrap();
    app.world_mut()
        .spawn((Node::default(), RouterHostRegistration::new(router)));
    if plugin_first == Some(false) {
        app.add_plugins(InputFocusPlugin);
    }
    app.update();
    Fixture {
        app,
        router,
        a,
        b,
        prefix,
    }
}

fn prepare_prefix(fixture: &mut Fixture, queued: bool, initially_focused: bool) {
    if initially_focused {
        fixture
            .app
            .world_mut()
            .resource_mut::<InputFocus>()
            .set(fixture.prefix, FocusCause::Navigated);
        fixture.app.update();
        fixture
            .app
            .world_mut()
            .resource_mut::<InputFocus>()
            .set(fixture.a, FocusCause::Pressed);
        if !queued {
            fixture.app.update();
        }
    }
    fixture.app.world_mut().resource_mut::<Events>().0.clear();
    *fixture.app.world_mut().resource_mut::<LaterOperations>() = LaterOperations::default();
}

fn push(fixture: &mut Fixture, nested: bool) -> NavigationRequestId {
    fixture
        .app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(
            fixture.router,
            RouteLocation::new(if nested { "/middle/leaf" } else { "/" }),
        )
        .unwrap()
}

fn settle(fixture: &mut Fixture, request: NavigationRequestId, failed: bool) {
    for _ in 0..32 {
        fixture.app.update();
        let result = fixture
            .app
            .world()
            .resource::<RouterSnapshots>()
            .request(fixture.router, request);
        match result {
            RequestLookup::Found(request) if request.status() == &RequestStatus::Succeeded => {
                assert!(!failed, "failure Scene unexpectedly succeeded");
                return;
            }
            RequestLookup::Found(request)
                if matches!(request.status(), RequestStatus::Failed(_)) =>
            {
                assert!(failed, "{result:?}");
                // Include ordinary fallback and event processing, not just the first failure snapshot.
                for _ in 0..8 {
                    fixture.app.update();
                }
                return;
            }
            _ => {}
        }
    }
    panic!("navigation did not settle");
}

fn assert_no_target_events(fixture: &Fixture, target: Entity) {
    assert!(
        fixture
            .app
            .world()
            .resource::<Events>()
            .0
            .iter()
            .all(|event| match event {
                FocusEvent::Gained(entity, _) | FocusEvent::Lost(entity) => *entity != target,
            }),
        "pending/failed target received an official focus event: {:?}",
        fixture.app.world().resource::<Events>().0
    );
}

#[test]
fn native_focus_fixture_without_autofocus_has_a_real_multi_update_pending_window() {
    let mut fixture = fixture_with_autofocus(Vec::new(), Vec::new(), false, true, false);
    prepare_prefix(&mut fixture, false, true);
    fixture
        .app
        .world_mut()
        .resource_mut::<LaterOperations>()
        .operations = vec![Operation::SetA];
    let request = push(&mut fixture, true);
    for _ in 0..2 {
        fixture.app.update();
        let target = fixture.app.world().resource::<Attempts>().targets[0];
        assert!(
            !fixture
                .app
                .world()
                .resource::<RouterInputGate>()
                .allows_keyboard(target)
        );
        assert!(
            matches!(fixture.app.world().resource::<RouterSnapshots>().request(fixture.router, request), RequestLookup::Found(request) if !matches!(request.status(), RequestStatus::Failed(_) | RequestStatus::Succeeded))
        );
        assert_eq!(
            fixture.app.world().resource::<InputFocus>().get(),
            Some(fixture.a)
        );
    }
    assert!(fixture.app.world().resource::<LaterOperations>().applied);
    settle(&mut fixture, request, false);
    let target = fixture.app.world().resource::<Attempts>().targets[0];
    assert!(
        fixture
            .app
            .world()
            .resource::<RouterInputGate>()
            .allows_keyboard(target)
    );
    assert_eq!(
        fixture.app.world().resource::<InputFocus>().get(),
        Some(fixture.a)
    );
    assert!(
        fixture.app.world().resource::<Events>().0.is_empty(),
        "the real later same-target set generated no event, but is still a superseding operation"
    );
}

#[test]
fn successful_autofocus_publishes_once_after_preserving_original_queue() {
    let mut fixture = fixture(Vec::new(), Vec::new(), false, false);
    prepare_prefix(&mut fixture, true, true);
    let request = push(&mut fixture, false);
    settle(&mut fixture, request, false);
    let target = fixture.app.world().resource::<Attempts>().targets[0];
    assert!(
        fixture
            .app
            .world()
            .resource::<RouterInputGate>()
            .allows_keyboard(target)
    );
    assert_eq!(
        fixture.app.world().resource::<InputFocus>().get(),
        Some(target)
    );
    assert_eq!(
        fixture.app.world().resource::<Events>().0,
        vec![
            FocusEvent::Lost(fixture.prefix),
            FocusEvent::Gained(fixture.a, FocusCause::Pressed),
            FocusEvent::Lost(fixture.a),
            FocusEvent::Gained(target, FocusCause::Navigated),
        ]
    );
}

#[test]
fn failed_autofocus_preserves_app_operations_and_original_queue_without_target_events() {
    for (before, after, expected) in [
        (vec![], vec![], Some(false)),
        (vec![Operation::Clear], vec![], None),
        (vec![Operation::SetB], vec![], Some(true)),
        (vec![], vec![Operation::SetB], Some(true)),
        (vec![], vec![Operation::Clear], None),
    ] {
        let mut fixture = fixture(before.clone(), after.clone(), true, false);
        prepare_prefix(&mut fixture, true, true);
        let request = push(&mut fixture, false);
        settle(&mut fixture, request, true);
        let attempts = fixture.app.world().resource::<Attempts>();
        let target = attempts.targets[0];
        assert_eq!(
            attempts.child_saw_focus.len(),
            1,
            "real child failure was reached"
        );
        if after.is_empty() {
            assert_eq!(
                attempts.child_saw_focus[0],
                Some(target),
                "AutoFocus actually ran inside Scene before failure"
            );
        }
        assert_no_target_events(&fixture, target);
        let expected_target = expected.map(|b| if b { fixture.b } else { fixture.a });
        assert_eq!(
            fixture.app.world().resource::<InputFocus>().get(),
            expected_target,
            "before={before:?}, after={after:?}"
        );
        let mut expected_events = vec![
            FocusEvent::Lost(fixture.prefix),
            FocusEvent::Gained(fixture.a, FocusCause::Pressed),
        ];
        if expected_target != Some(fixture.a) {
            expected_events.push(FocusEvent::Lost(fixture.a));
            if let Some(entity) = expected_target {
                expected_events.push(FocusEvent::Gained(entity, FocusCause::Navigated));
            }
        }
        assert_eq!(fixture.app.world().resource::<Events>().0, expected_events);
    }
}

#[test]
fn successful_scene_preserves_last_set_or_clear_order_without_replaying_earlier_pending_focus() {
    for (before, after, expected) in [
        (vec![], vec![Operation::SetB], Some(true)),
        (vec![], vec![Operation::Clear], None),
        (vec![Operation::SetB], vec![], Some(false)),
    ] {
        let mut fixture = fixture(before, after, false, false);
        prepare_prefix(&mut fixture, false, true);
        let request = push(&mut fixture, false);
        settle(&mut fixture, request, false);
        let target = fixture.app.world().resource::<Attempts>().targets[0];
        let expected_target = expected.map(|app| if app { fixture.b } else { target });
        assert_eq!(
            fixture.app.world().resource::<InputFocus>().get(),
            expected_target
        );
        if expected_target != Some(target) {
            assert_no_target_events(&fixture, target);
        }
    }
}

#[test]
fn native_pending_autofocus_stays_silent_across_drain_then_publishes() {
    let mut fixture = fixture(Vec::new(), Vec::new(), false, true);
    prepare_prefix(&mut fixture, false, true);
    let request = push(&mut fixture, true);
    fixture.app.update();
    let target = fixture.app.world().resource::<Attempts>().targets[0];
    assert!(
        !fixture
            .app
            .world()
            .resource::<RouterInputGate>()
            .allows_keyboard(target)
    );
    assert_eq!(
        fixture.app.world().resource::<InputFocus>().get(),
        Some(fixture.a)
    );
    assert_no_target_events(&fixture, target);
    fixture.app.update();
    assert!(
        !fixture
            .app
            .world()
            .resource::<RouterInputGate>()
            .allows_keyboard(target),
        "the genuine nested branch is still pending across a second official drain"
    );
    assert_no_target_events(&fixture, target);
    settle(&mut fixture, request, false);
    assert_eq!(
        fixture.app.world().resource::<InputFocus>().get(),
        Some(target)
    );
    assert_eq!(
        fixture.app.world().resource::<Events>().0,
        vec![
            FocusEvent::Lost(fixture.a),
            FocusEvent::Gained(target, FocusCause::Navigated)
        ]
    );
}

#[test]
fn multiple_pending_frames_publish_only_the_last_real_focus_operation() {
    let mut fixture = fixture(vec![], vec![], false, true);
    prepare_prefix(&mut fixture, false, true);
    fixture.app.world_mut().resource_mut::<PlainAutoFocus>().0 = true;
    let request = push(&mut fixture, true);
    settle(&mut fixture, request, false);
    let targets = &fixture.app.world().resource::<Attempts>().targets;
    assert_eq!(targets.len(), 3);
    assert_no_target_events(&fixture, targets[0]);
    assert_no_target_events(&fixture, targets[1]);
    assert_eq!(
        fixture.app.world().resource::<InputFocus>().get(),
        Some(targets[2])
    );
    assert_eq!(
        fixture.app.world().resource::<Events>().0,
        vec![
            FocusEvent::Lost(fixture.a),
            FocusEvent::Gained(targets[2], FocusCause::Navigated)
        ]
    );
}

#[test]
fn later_same_target_set_or_clear_across_official_drain_supersedes_native_pending_intent() {
    for (initially_focused, operations, expected_focused) in [
        (true, vec![Operation::SetA], true),
        (false, vec![Operation::Clear], false),
        (true, vec![Operation::SetB, Operation::SetA], true),
    ] {
        let mut fixture = fixture(Vec::new(), Vec::new(), false, true);
        prepare_prefix(&mut fixture, false, initially_focused);
        fixture
            .app
            .world_mut()
            .resource_mut::<LaterOperations>()
            .operations = operations;
        let request = push(&mut fixture, true);
        fixture.app.update();
        let target = fixture.app.world().resource::<Attempts>().targets[0];
        fixture.app.update();
        assert!(fixture.app.world().resource::<LaterOperations>().applied);
        assert!(
            !fixture
                .app
                .world()
                .resource::<RouterInputGate>()
                .allows_keyboard(target),
            "later operation ran while actual native branch was pending"
        );
        settle(&mut fixture, request, false);
        assert_eq!(
            fixture.app.world().resource::<InputFocus>().get(),
            expected_focused.then_some(fixture.a)
        );
        assert_no_target_events(&fixture, target);
    }
}

#[test]
fn native_authority_loss_keeps_focus_queue_source_for_readmission() {
    for autofocus in [false, true] {
        let mut fixture = fixture_with_autofocus(vec![], vec![], false, false, autofocus);
        prepare_prefix(&mut fixture, false, true);
        let request = push(&mut fixture, false);
        settle(&mut fixture, request, false);
        let host = fixture
            .app
            .world_mut()
            .query_filtered::<Entity, With<RouterHostRegistration>>()
            .single(fixture.app.world())
            .unwrap();
        let history = fixture
            .app
            .world()
            .resource::<RouterSnapshots>()
            .get(fixture.router)
            .unwrap()
            .history()
            .clone();
        #[derive(Resource)]
        struct LoseHost(bool);
        fixture.app.insert_resource(LoseHost(true));
        fixture.app.world_mut().add_observer(
            move |_: On<Add, NativeFocusTarget>,
                  mut lose: ResMut<LoseHost>,
                  mut commands: Commands| {
                if lose.0 {
                    lose.0 = false;
                    commands.entity(host).remove::<RouterHostRegistration>();
                }
            },
        );
        let old = fixture.app.world().resource::<Attempts>().targets[0];
        fixture
            .app
            .world_mut()
            .resource_mut::<InputFocus>()
            .set(old, FocusCause::Navigated);
        fixture.app.update();
        fixture
            .app
            .world_mut()
            .resource_mut::<InputFocus>()
            .set(fixture.a, FocusCause::Pressed);
        fixture
            .app
            .world_mut()
            .resource_mut::<InputFocus>()
            .set(old, FocusCause::Navigated);
        fixture.app.world_mut().resource_mut::<Events>().0.clear();
        let lost = fixture
            .app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .replace(
                fixture.router,
                RouteLocation::new("/").with_fragment("lost-host"),
            )
            .unwrap();
        for _ in 0..24 {
            fixture.app.update();
        }
        assert!(
            !fixture.app.world().resource::<LoseHost>().0,
            "the real Scene hook removed the host during replacement"
        );
        assert!(
            fixture
                .app
                .world()
                .resource::<Events>()
                .0
                .contains(&FocusEvent::Gained(fixture.a, FocusCause::Pressed)),
            "authority isolation lost an earlier valid queued app operation"
        );
        let snapshots = fixture.app.world().resource::<RouterSnapshots>();
        assert!(
            matches!(snapshots.request(fixture.router, lost), RequestLookup::Found(request) if matches!(request.status(), RequestStatus::Failed(failure) if failure.kind() == NavigationFailureKind::HostLost))
        );
        let snapshot = snapshots.get(fixture.router).unwrap();
        assert_eq!(snapshot.history(), &history);
        assert_eq!(snapshot.active().kind(), ActivePresentationKind::None);
        assert_eq!(
            fixture.app.world().resource::<Attempts>().targets.len(),
            2,
            "no fallback or replacement publication may follow HostLost"
        );
        assert!(
            fixture.app.world().get::<Node>(host).is_some(),
            "the same application host survives settlement"
        );
        for target in &fixture.app.world().resource::<Attempts>().targets {
            assert!(
                !fixture
                    .app
                    .world()
                    .resource::<RouterInputGate>()
                    .allows_keyboard(*target)
            );
            assert_ne!(
                fixture.app.world().resource::<InputFocus>().get(),
                Some(*target)
            );
        }
        fixture
            .app
            .world_mut()
            .entity_mut(host)
            .insert(RouterHostRegistration::new(fixture.router));
        for _ in 0..8 {
            fixture.app.update();
        }
        let request = fixture
            .app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(
                fixture.router,
                RouteLocation::new("/").with_fragment("readmitted"),
            )
            .unwrap();
        settle(&mut fixture, request, false);
        let target = *fixture
            .app
            .world()
            .resource::<Attempts>()
            .targets
            .last()
            .unwrap();
        assert!(
            fixture
                .app
                .world()
                .resource::<RouterInputGate>()
                .allows_keyboard(target)
        );
        if autofocus {
            assert_eq!(
                fixture.app.world().resource::<InputFocus>().get(),
                Some(target)
            );
        }
    }
}

#[test]
fn no_operation_mutable_borrow_does_not_cancel_native_pending_focus() {
    let mut fixture = fixture(vec![], vec![], false, true);
    prepare_prefix(&mut fixture, false, true);
    fixture
        .app
        .world_mut()
        .resource_mut::<LaterOperations>()
        .operations = vec![Operation::BorrowOnly];
    let request = push(&mut fixture, true);
    settle(&mut fixture, request, false);
    assert!(fixture.app.world().resource::<LaterOperations>().applied);
    let target = fixture.app.world().resource::<Attempts>().targets[0];
    assert_eq!(
        fixture.app.world().resource::<InputFocus>().get(),
        Some(target)
    );
}

#[test]
fn authored_hidden_native_target_cannot_consume_pending_autofocus() {
    let mut fixture = fixture(vec![], vec![], false, false);
    prepare_prefix(&mut fixture, false, true);
    fixture.app.world_mut().add_observer(
        |event: On<Add, NativeFocusTarget>, mut commands: Commands| {
            commands.entity(event.entity).insert(Node {
                display: bevy_ui::Display::None,
                ..Node::default()
            });
        },
    );
    let request = push(&mut fixture, false);
    settle(&mut fixture, request, false);
    let target = fixture.app.world().resource::<Attempts>().targets[0];
    assert_no_target_events(&fixture, target);
    assert_eq!(
        fixture.app.world().resource::<InputFocus>().get(),
        Some(fixture.a)
    );
    assert!(
        !fixture
            .app
            .world()
            .resource::<RouterInputGate>()
            .allows_keyboard(target)
    );
    assert_eq!(
        fixture.app.world().get::<Node>(target).unwrap().display,
        bevy_ui::Display::None
    );
}

#[test]
fn late_official_plugin_preserves_pending_intent_and_does_not_dispatch_early() {
    let mut fixture = fixture_with_plugin(vec![], vec![], false, true, true, None);
    prepare_prefix(&mut fixture, false, true);
    let request = push(&mut fixture, true);
    fixture.app.update();
    let target = fixture.app.world().resource::<Attempts>().targets[0];
    assert!(
        !fixture
            .app
            .world()
            .resource::<RouterInputGate>()
            .allows_keyboard(target)
    );
    assert!(
        fixture.app.world().resource::<Events>().0.is_empty(),
        "resource-only integration must not add an event processor"
    );
    fixture.app.add_plugins(InputFocusPlugin);
    fixture.app.update();
    assert_no_target_events(&fixture, target);
    settle(&mut fixture, request, false);
    assert_eq!(
        fixture.app.world().resource::<InputFocus>().get(),
        Some(target)
    );
    assert_eq!(
        fixture
            .app
            .world()
            .resource::<Events>()
            .0
            .iter()
            .filter(|event| matches!(event, FocusEvent::Gained(entity, _) if *entity == target))
            .count(),
        1
    );
}

#[test]
fn navigation_resource_scope_tail_app_set_supersedes_scene_autofocus() {
    let mut fixture = fixture(vec![], vec![], false, false);
    prepare_prefix(&mut fixture, false, true);
    #[derive(Resource, Default)]
    struct TailFired(bool);
    fixture.app.init_resource::<TailFired>();
    let b = fixture.b;
    fixture.app.world_mut().add_observer(
        move |_: On<Insert, RouterNavigation>,
              attempts: Res<Attempts>,
              mut fired: ResMut<TailFired>,
              mut focus: ResMut<InputFocus>| {
            if !attempts.targets.is_empty() && !fired.0 {
                fired.0 = true;
                focus.set(b, FocusCause::Pressed);
            }
        },
    );
    let request = push(&mut fixture, false);
    settle(&mut fixture, request, false);
    assert!(fixture.app.world().resource::<TailFired>().0);
    let target = fixture.app.world().resource::<Attempts>().targets[0];
    assert_no_target_events(&fixture, target);
    assert_eq!(fixture.app.world().resource::<InputFocus>().get(), Some(b));
    assert_eq!(
        fixture.app.world().resource::<Events>().0,
        vec![
            FocusEvent::Lost(fixture.a),
            FocusEvent::Gained(b, FocusCause::Pressed)
        ]
    );
}

#[derive(Resource, Default, Debug)]
struct PendingSourceMove {
    root: Option<Entity>,
    target: Option<Entity>,
    sibling: Option<Entity>,
    hook_focus: Option<Entity>,
    tail_focus: Option<Entity>,
    tail_parent: Option<Entity>,
    tail_children: Vec<Entity>,
    tail_gate: bool,
    observed: bool,
}

#[derive(Clone)]
enum SourceMoveTemplate {
    Root,
    Target,
    Sibling,
}

impl Template for SourceMoveTemplate {
    type Output = ();

    fn build_template(&self, context: &mut TemplateContext<'_, '_>) -> bevy_ecs::error::Result<()> {
        let entity = context.entity.id();
        context.entity.insert(Node::default());
        if matches!(self, Self::Target) {
            context.entity.insert(AutoFocus);
        }
        context.entity.world_scope(|world| {
            let focus = world.resource::<InputFocus>().get();
            let mut evidence = world.resource_mut::<PendingSourceMove>();
            match self {
                Self::Root => evidence.root = Some(entity),
                Self::Target => {
                    evidence.target = Some(entity);
                    evidence.hook_focus = focus;
                }
                Self::Sibling => evidence.sibling = Some(entity),
            }
        });
        Ok(())
    }

    fn clone_template(&self) -> Self {
        self.clone()
    }
}

fn pending_source_move_fixture() -> Fixture {
    let mut app = App::new();
    app.add_plugins((
        TaskPoolPlugin::default(),
        AssetPlugin::default(),
        ScenePlugin,
        InputFocusPlugin,
    ));
    app.init_resource::<Events>()
        .init_resource::<LaterOperations>()
        .init_resource::<PendingSourceMove>();
    app.world_mut()
        .add_observer(|mut event: On<FocusGained>, mut events: ResMut<Events>| {
            events.0.push(FocusEvent::Gained(event.entity, event.cause));
            event.propagate(false);
        });
    app.world_mut()
        .add_observer(|mut event: On<FocusLost>, mut events: ResMut<Events>| {
            events.0.push(FocusEvent::Lost(event.entity));
            event.propagate(false);
        });
    let a = app.world_mut().spawn(Node::default()).id();
    let b = app.world_mut().spawn(Node::default()).id();
    let prefix = app.world_mut().spawn(Node::default()).id();
    let parent_driver = app
        .register_bsn_view(ViewDriverKey::new("preserved-parent").unwrap(), |_| {
            Ok(bevy_scene::bsn! { Node DefaultOutlet })
        })
        .unwrap();
    let child_driver = app
        .register_bsn_view(ViewDriverKey::new("moving-focus-child").unwrap(), |_| {
            Ok(SceneFunction(|_, root: &mut ResolvedScene| {
                root.push_bundle_template(SourceMoveTemplate::Root);
                for template in [SourceMoveTemplate::Target, SourceMoveTemplate::Sibling] {
                    let mut child = ResolvedScene::default();
                    child.push_bundle_template(template);
                    root.get_or_insert_related_resolved_scenes::<ChildOf>()
                        .scenes
                        .push(child);
                }
            }))
        })
        .unwrap();
    let parent_view = RouteViewKey::new("parent").unwrap();
    let child_view = RouteViewKey::new("child").unwrap();
    let mut routes = RouteTree::builder()
        .root_with_options(
            RoutePattern::path("/").unwrap(),
            RouteNodeOptions::new().with_view(parent_view.clone()),
        )
        .unwrap();
    routes
        .route_with_options(
            routes.root_id().unwrap(),
            RoutePattern::path("child").unwrap(),
            RouteNodeOptions::new().with_view(child_view.clone()),
        )
        .unwrap();
    let mut views = RouteViewRegistry::new();
    views
        .insert(parent_view.clone(), RouteViewDescriptor::new(parent_driver))
        .unwrap();
    views
        .insert(child_view, RouteViewDescriptor::new(child_driver))
        .unwrap();
    let router = app
        .add_ui_router(RouterConfig::new(
            routes.build().unwrap(),
            views,
            parent_view,
        ))
        .unwrap();
    app.world_mut()
        .spawn((Node::default(), RouterHostRegistration::new(router)));
    app.update();
    Fixture {
        app,
        router,
        a,
        b,
        prefix,
    }
}

fn pending_source_membership_at_publication(move_to_preserved: bool) {
    let mut fixture = pending_source_move_fixture();
    let parent_request = push(&mut fixture, false);
    settle(&mut fixture, parent_request, false);
    let parent = fixture
        .app
        .world_mut()
        .query_filtered::<Entity, With<DefaultOutlet>>()
        .single(fixture.app.world())
        .unwrap();
    assert!(
        fixture
            .app
            .world()
            .resource::<RouterInputGate>()
            .allows_keyboard(parent)
    );
    prepare_prefix(&mut fixture, false, true);
    fixture.app.world_mut().add_observer(
        move |_: On<Insert, RouterNavigation>,
              mut evidence: ResMut<PendingSourceMove>,
              focus: Res<InputFocus>,
              gate: Res<RouterInputGate>,
              parents: Query<&ChildOf>,
              children: Query<&Children>,
              mut commands: Commands| {
            let (Some(root), Some(target)) = (evidence.root, evidence.target) else {
                return;
            };
            if evidence.observed {
                return;
            }
            evidence.observed = true;
            evidence.tail_focus = focus.get();
            evidence.tail_parent = parents.get(target).ok().map(ChildOf::parent);
            evidence.tail_children = children.get(root).unwrap().iter().collect();
            evidence.tail_gate = gate.allows_keyboard(target);
            if move_to_preserved {
                // Only an ordinary authored child moves. No focus operation is
                // produced here, and another child keeps the source's Children.
                commands.entity(target).insert(ChildOf(parent));
            }
        },
    );
    let request = fixture
        .app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(fixture.router, RouteLocation::new("/child"))
        .unwrap();
    settle(&mut fixture, request, false);
    let world = fixture.app.world();
    let evidence = world.resource::<PendingSourceMove>();
    let root = evidence.root.unwrap();
    let target = evidence.target.unwrap();
    let sibling = evidence.sibling.unwrap();
    assert!(evidence.observed);
    assert_eq!(
        evidence.hook_focus,
        Some(target),
        "official AutoFocus hook ran"
    );
    assert_eq!(
        evidence.tail_focus,
        Some(fixture.a),
        "pending focus was isolated before the tail"
    );
    assert!(
        !evidence.tail_gate,
        "the source was still pending at the tail"
    );
    assert_eq!(evidence.tail_parent, Some(root));
    assert_eq!(evidence.tail_children.len(), 2);
    assert!(evidence.tail_children.contains(&target));
    assert!(evidence.tail_children.contains(&sibling));
    assert_eq!(world.get::<ChildOf>(sibling).unwrap().parent(), root);
    let final_parent = world.get::<ChildOf>(target).unwrap().parent();
    assert_eq!(final_parent, if move_to_preserved { parent } else { root });
    assert_eq!(
        world.get::<Children>(root).unwrap().contains(&target),
        !move_to_preserved
    );
    assert!(world.get::<Children>(root).unwrap().contains(&sibling));
    assert!(
        world.get::<DefaultOutlet>(parent).is_some(),
        "the same published parent survives"
    );
    assert!(world.resource::<RouterInputGate>().allows_keyboard(target));
    if move_to_preserved {
        assert_eq!(
            world.resource::<InputFocus>().get(),
            Some(fixture.a),
            "an intent from the former pending source cannot follow the target into a preserved frame"
        );
        assert!(world.resource::<Events>().0.is_empty());
    } else {
        assert_eq!(world.resource::<InputFocus>().get(), Some(target));
        assert_eq!(
            world.resource::<Events>().0,
            vec![
                FocusEvent::Lost(fixture.a),
                FocusEvent::Gained(target, FocusCause::Navigated)
            ]
        );
    }
}

#[test]
fn pending_source_membership_positive_autofocus_control() {
    pending_source_membership_at_publication(false);
}

#[test]
fn pending_source_membership_move_to_preserved_does_not_replay_autofocus() {
    pending_source_membership_at_publication(true);
}

#[test]
fn resource_scope_tail_set_on_another_active_router_is_not_classified_as_pending() {
    let mut fixture = fixture(vec![], vec![], false, true);
    #[derive(Component, Clone, Default)]
    struct OtherActive;
    let driver = fixture
        .app
        .register_bsn_view(ViewDriverKey::new("other-active").unwrap(), |_| {
            Ok(bevy_scene::bsn! { Node OtherActive })
        })
        .unwrap();
    let view = RouteViewKey::new("other-active").unwrap();
    let routes = RouteTree::builder()
        .root_with_options(
            RoutePattern::path("/").unwrap(),
            RouteNodeOptions::new().with_view(view.clone()),
        )
        .unwrap()
        .build()
        .unwrap();
    let mut views = RouteViewRegistry::new();
    views
        .insert(view.clone(), RouteViewDescriptor::new(driver))
        .unwrap();
    let router = fixture
        .app
        .add_ui_router(RouterConfig::new(routes, views, view))
        .unwrap();
    fixture
        .app
        .world_mut()
        .spawn((Node::default(), RouterHostRegistration::new(router)));
    fixture.app.update();
    let request = fixture
        .app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, RouteLocation::new("/"))
        .unwrap();
    for _ in 0..12 {
        fixture.app.update();
    }
    assert!(
        matches!(fixture.app.world().resource::<RouterSnapshots>().request(router, request), RequestLookup::Found(request) if request.status() == &RequestStatus::Succeeded)
    );
    let other = fixture
        .app
        .world_mut()
        .query_filtered::<Entity, With<OtherActive>>()
        .single(fixture.app.world())
        .unwrap();
    assert!(
        fixture
            .app
            .world()
            .resource::<RouterInputGate>()
            .allows_keyboard(other)
    );
    #[derive(Resource, Default)]
    struct TailFired(bool);
    fixture.app.init_resource::<TailFired>();
    fixture.app.world_mut().add_observer(
        move |_: On<Insert, RouterNavigation>,
              attempts: Res<Attempts>,
              mut fired: ResMut<TailFired>,
              mut focus: ResMut<InputFocus>| {
            if !attempts.targets.is_empty() && !fired.0 {
                fired.0 = true;
                focus.set(other, FocusCause::Pressed);
            }
        },
    );
    fixture.app.world_mut().resource_mut::<Events>().0.clear();
    let request = push(&mut fixture, true);
    settle(&mut fixture, request, false);
    assert!(fixture.app.world().resource::<TailFired>().0);
    assert_no_target_events(
        &fixture,
        fixture.app.world().resource::<Attempts>().targets[0],
    );
    assert_eq!(
        fixture.app.world().resource::<InputFocus>().get(),
        Some(other)
    );
    assert_eq!(
        fixture.app.world().resource::<Events>().0,
        vec![FocusEvent::Gained(other, FocusCause::Pressed)]
    );
    assert!(
        fixture
            .app
            .world()
            .resource::<RouterInputGate>()
            .allows_keyboard(other)
    );
}

#[test]
fn native_idle_loss_readmission_preserves_focus_source_and_queued_app_cause() {
    let mut fixture = fixture(vec![], vec![], false, false);
    prepare_prefix(&mut fixture, false, true);
    let request = push(&mut fixture, false);
    settle(&mut fixture, request, false);
    let old = fixture.app.world().resource::<Attempts>().targets[0];
    let host = fixture
        .app
        .world_mut()
        .query_filtered::<Entity, With<RouterHostRegistration>>()
        .single(fixture.app.world())
        .unwrap();
    fixture.app.world_mut().resource_mut::<Events>().0.clear();
    fixture
        .app
        .world_mut()
        .resource_mut::<InputFocus>()
        .set(fixture.a, FocusCause::Pressed);
    fixture
        .app
        .world_mut()
        .resource_mut::<InputFocus>()
        .set(old, FocusCause::Navigated);
    fixture
        .app
        .world_mut()
        .entity_mut(host)
        .remove::<RouterHostRegistration>();
    for _ in 0..24 {
        fixture.app.update();
    }
    assert!(
        fixture
            .app
            .world()
            .resource::<Events>()
            .0
            .contains(&FocusEvent::Gained(fixture.a, FocusCause::Pressed))
    );
    fixture
        .app
        .world_mut()
        .entity_mut(host)
        .insert(RouterHostRegistration::new(fixture.router));
    for _ in 0..8 {
        fixture.app.update();
    }
    let request = fixture
        .app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(
            fixture.router,
            RouteLocation::new("/").with_fragment("readmitted"),
        )
        .unwrap();
    settle(&mut fixture, request, false);
    let target = *fixture
        .app
        .world()
        .resource::<Attempts>()
        .targets
        .last()
        .unwrap();
    assert_eq!(
        fixture.app.world().resource::<InputFocus>().get(),
        Some(target)
    );
}

#[test]
fn native_back_publishes_only_its_fresh_autofocus_owner() {
    let mut fixture = fixture(vec![], vec![], false, false);
    let request = push(&mut fixture, false);
    settle(&mut fixture, request, false);
    let first = fixture.app.world().resource::<Attempts>().targets[0];
    let request = fixture
        .app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(
            fixture.router,
            RouteLocation::new("/").with_fragment("second"),
        )
        .unwrap();
    settle(&mut fixture, request, false);
    let second = fixture.app.world().resource::<Attempts>().targets[1];
    assert_ne!(first, second);
    fixture.app.world_mut().resource_mut::<Events>().0.clear();
    let request = fixture
        .app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .back(fixture.router)
        .unwrap();
    settle(&mut fixture, request, false);
    let targets = &fixture.app.world().resource::<Attempts>().targets;
    assert_eq!(targets.len(), 3);
    let restored = targets[2];
    assert_ne!(restored, first);
    assert_ne!(restored, second);
    assert_eq!(
        fixture.app.world().resource::<InputFocus>().get(),
        Some(restored)
    );
    assert!(
        fixture
            .app
            .world()
            .resource::<RouterInputGate>()
            .allows_keyboard(restored)
    );
    let events = &fixture.app.world().resource::<Events>().0;
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, FocusEvent::Gained(entity, _) if *entity == restored))
            .count(),
        1
    );
    assert!(!events.iter().any(|event| matches!(event, FocusEvent::Gained(entity, _) if *entity == first || *entity == second)));
}

#[test]
fn native_projectionless_fallback_consumes_its_own_autofocus_not_the_failed_owner() {
    let mut fixture = fixture(vec![], vec![], true, false);
    prepare_prefix(&mut fixture, true, true);
    fixture.app.world_mut().resource_mut::<PlainAutoFocus>().0 = true;
    let request = push(&mut fixture, false);
    settle(&mut fixture, request, true);
    let targets = &fixture.app.world().resource::<Attempts>().targets;
    assert_eq!(
        targets.len(),
        2,
        "real failed Scene followed by the native fallback Scene"
    );
    let failed = targets[0];
    let fallback = targets[1];
    assert_no_target_events(&fixture, failed);
    assert_eq!(
        fixture.app.world().resource::<InputFocus>().get(),
        Some(fallback)
    );
    assert!(
        fixture
            .app
            .world()
            .resource::<RouterInputGate>()
            .allows_keyboard(fallback)
    );
    assert_eq!(
        fixture.app.world().resource::<Events>().0,
        vec![
            FocusEvent::Lost(fixture.prefix),
            FocusEvent::Gained(fixture.a, FocusCause::Pressed),
            FocusEvent::Lost(fixture.a),
            FocusEvent::Gained(fallback, FocusCause::Navigated),
        ]
    );
}

// This now exercises the production adapter through registration/navigation;
// the initial recipe-only version established installation feasibility first.
mod drain_installation {
    use super::*;
    use bevy_app::Update;
    use bevy_ecs::schedule::ScheduleCleanupPolicy;
    use bevy_input_focus::process_recorded_focus_changes;

    #[derive(Resource, Default)]
    struct Evidence {
        update_complete: bool,
        initialized_after_update: bool,
        calls: Vec<&'static str>,
        public_member: usize,
        observer_appended: bool,
    }

    #[derive(Resource)]
    struct Enabled(bool);

    struct ReadsCompletedUpdate;
    impl FromWorld for ReadsCompletedUpdate {
        fn from_world(world: &mut World) -> Self {
            let complete = world.resource::<Evidence>().update_complete;
            assert!(
                complete,
                "PostUpdate Local initialization ran before Update completed"
            );
            world.resource_mut::<Evidence>().initialized_after_update = true;
            Self
        }
    }

    fn finish_update(mut evidence: ResMut<Evidence>) {
        evidence.update_complete = true;
    }
    fn local_reader(_: Local<ReadsCompletedUpdate>) {}
    fn before_function(mut evidence: ResMut<Evidence>) {
        evidence.calls.push("before");
    }
    fn after_function(mut evidence: ResMut<Evidence>) {
        evidence.calls.push("after");
    }
    fn public_member(mut evidence: ResMut<Evidence>) {
        evidence.public_member += 1;
    }
    fn enabled(enabled: Res<Enabled>) -> bool {
        enabled.0
    }

    #[derive(Resource)]
    struct RefocusPendingOnce(Option<Entity>);

    fn refocus_pending_once(
        mut pending: ResMut<RefocusPendingOnce>,
        mut focus: ResMut<InputFocus>,
    ) {
        if let Some(target) = pending.0.take() {
            focus.set(target, FocusCause::Pressed);
        }
    }

    #[test]
    fn externally_initialized_post_update_rechecks_the_drain_before_pending_focus() {
        let mut fixture = fixture(Vec::new(), Vec::new(), false, true);
        prepare_prefix(&mut fixture, false, true);
        let request = push(&mut fixture, true);
        fixture.app.update();
        let target = fixture.app.world().resource::<Attempts>().targets[0];
        assert!(
            !fixture
                .app
                .world()
                .resource::<RouterInputGate>()
                .allows_keyboard(target)
        );
        assert_no_target_events(&fixture, target);

        // An application reconfigures and builds PostUpdate before the next
        // Router boundary. The graph flag is already clear at that boundary.
        fixture
            .app
            .insert_resource(RefocusPendingOnce(Some(target)))
            .add_systems(
                PostUpdate,
                refocus_pending_once.before(InputFocusSystems::FocusChangeEvents),
            );
        fixture
            .app
            .world_mut()
            .schedule_scope(PostUpdate, |world, schedule| {
                assert_eq!(
                    schedule
                        .remove_systems_in_set(
                            process_recorded_focus_changes,
                            world,
                            ScheduleCleanupPolicy::RemoveSystemsOnly,
                        )
                        .unwrap(),
                    1
                );
                schedule.add_systems(
                    process_recorded_focus_changes.in_set(InputFocusSystems::FocusChangeEvents),
                );
                schedule.initialize(world).unwrap();
                assert!(!schedule.is_changed());
            });

        fixture.app.update();
        assert!(
            fixture
                .app
                .world()
                .resource::<RefocusPendingOnce>()
                .0
                .is_none()
        );
        assert!(
            !fixture
                .app
                .world()
                .resource::<RouterInputGate>()
                .allows_keyboard(target),
            "the real nested branch is still pending during the rebuilt drain"
        );
        assert_no_target_events(&fixture, target);
        settle(&mut fixture, request, false);
        assert_eq!(
            fixture.app.world().resource::<InputFocus>().get(),
            Some(target)
        );
        assert_eq!(
            fixture.app.world().resource::<Events>().0,
            vec![
                FocusEvent::Lost(fixture.a),
                FocusEvent::Gained(target, FocusCause::Pressed),
            ]
        );
    }

    #[test]
    fn official_drain_composition_preserves_order_conditions_deferred_and_initialization() {
        for plugin_first in [true, false] {
            let mut fixture =
                fixture_with_plugin(vec![], vec![], false, false, false, Some(plugin_first));
            let a = fixture.a;
            let b = fixture.b;
            let router = fixture.router;
            let app = &mut fixture.app;
            app.init_resource::<Evidence>()
                .insert_resource(Enabled(true));
            app.register_bsn_view(ViewDriverKey::new("repeat-install").unwrap(), |_| {
                Ok(bevy_scene::bsn! { Node })
            })
            .unwrap();
            app.add_systems(Update, finish_update);
            app.add_systems(
                PostUpdate,
                (
                    local_reader,
                    before_function.before(process_recorded_focus_changes),
                    after_function.after(process_recorded_focus_changes),
                    public_member.in_set(InputFocusSystems::FocusChangeEvents),
                ),
            );
            app.configure_sets(
                PostUpdate,
                InputFocusSystems::FocusChangeEvents.run_if(enabled),
            );
            app.world_mut().add_observer(
                move |event: On<FocusGained>,
                      mut focus: ResMut<InputFocus>,
                      mut evidence: ResMut<Evidence>| {
                    assert_eq!(
                        evidence.calls.last(),
                        Some(&"before"),
                        "official function ordering/deferred delivery changed"
                    );
                    evidence.calls.push("event");
                    if event.entity == a && !evidence.observer_appended {
                        evidence.observer_appended = true;
                        focus.set(b, FocusCause::Pressed);
                    }
                },
            );
            app.world_mut()
                .resource_mut::<InputFocus>()
                .set(a, FocusCause::Navigated);
            app.world_mut()
                .resource_mut::<RouterNavigation>()
                .push(router, RouteLocation::new("/"))
                .unwrap();
            app.update();
            let evidence = app.world().resource::<Evidence>();
            assert!(evidence.initialized_after_update && evidence.observer_appended);
            assert_eq!(evidence.calls, vec!["before", "event", "after"]);
            assert_eq!(evidence.public_member, 1);
            assert_eq!(app.world().resource::<InputFocus>().get(), Some(b));
            app.world_mut().resource_mut::<Evidence>().calls.clear();
            app.world_mut().resource_mut::<Enabled>().0 = false;
            app.update();
            assert_eq!(
                app.world().resource::<Evidence>().calls,
                vec!["before", "after"]
            );
            assert_eq!(app.world().resource::<Evidence>().public_member, 1);
            app.world_mut().resource_mut::<Evidence>().calls.clear();
            app.world_mut().resource_mut::<Enabled>().0 = true;
            app.update();
            assert_eq!(
                app.world().resource::<Evidence>().calls,
                vec!["before", "event", "after"]
            );
            assert_eq!(app.world().resource::<Evidence>().public_member, 2);
        }
    }
}
