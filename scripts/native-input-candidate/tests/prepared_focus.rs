use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use bevy_app::App;
use bevy_ecs::{prelude::*, world::DeferredWorld};
use bevy_input::InputPlugin;
use bevy_input_focus::{
    FocusCause, FocusGained, FocusLost, FocusObservation, FocusOperation, FocusOperationObserver,
    InputDispatchPlugin, InputFocus, InputFocusPlugin, InputTargetEligibility, PreparedFocusChange,
    set_focus,
};
use bevy_window::{PrimaryWindow, Window};

#[derive(Component)]
struct Unavailable;

#[derive(Component)]
struct SourceData(u32);

#[derive(Debug, PartialEq)]
struct SourceSnapshot {
    target: Entity,
    value: u32,
}

#[derive(Resource, Default)]
struct Calls {
    predicate: Arc<AtomicUsize>,
    observation: Arc<AtomicUsize>,
}

fn eligible(world: &World, target: Entity) -> bool {
    world
        .resource::<Calls>()
        .predicate
        .fetch_add(1, Ordering::SeqCst);
    world.entities().contains(target) && world.get::<Unavailable>(target).is_none()
}

fn observe(world: &World, operation: &FocusOperation) -> Option<FocusObservation> {
    world
        .resource::<Calls>()
        .observation
        .fetch_add(1, Ordering::SeqCst);
    let target = operation.target?;
    let value = world.get::<SourceData>(target)?.0;
    Some(Arc::new(SourceSnapshot { target, value }))
}

#[derive(Debug, PartialEq)]
enum Event {
    Gained(Entity, FocusCause),
    Lost(Entity),
}

#[derive(Resource, Default)]
struct Events(Vec<Event>);

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((InputPlugin, InputFocusPlugin, InputDispatchPlugin));
    app.init_resource::<Calls>();
    app.insert_resource(InputTargetEligibility::new(eligible));
    app.insert_resource(FocusOperationObserver::new(observe));
    app.init_resource::<Events>();
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    app.add_observer(|event: On<FocusGained>, mut events: ResMut<Events>| {
        events.0.push(Event::Gained(event.entity, event.cause));
    });
    app.add_observer(|event: On<FocusLost>, mut events: ResMut<Events>| {
        events.0.push(Event::Lost(event.entity));
    });
    app.update();
    app.world_mut().resource_mut::<InputFocus>().clear();
    app.update();
    take_events(&mut app);
    reset_calls(&app);
    app
}

fn set(app: &mut App, target: Entity, cause: FocusCause) {
    let mut world: DeferredWorld = app.world_mut().into();
    set_focus(&mut world, target, cause);
}

fn latest(app: &App) -> FocusOperation {
    app.world()
        .resource::<InputFocus>()
        .latest_operation()
        .unwrap()
}

fn take_events(app: &mut App) -> Vec<Event> {
    core::mem::take(&mut app.world_mut().resource_mut::<Events>().0)
}

fn reset_calls(app: &App) {
    let calls = app.world().resource::<Calls>();
    calls.predicate.store(0, Ordering::SeqCst);
    calls.observation.store(0, Ordering::SeqCst);
}

fn assert_no_calls(app: &App) {
    let calls = app.world().resource::<Calls>();
    assert_eq!(calls.predicate.load(Ordering::SeqCst), 0);
    assert_eq!(calls.observation.load(Ordering::SeqCst), 0);
}

#[test]
fn source_observation_is_frozen_at_the_original_operation() {
    let mut app = app();
    let target = app.world_mut().spawn((Unavailable, SourceData(17))).id();
    set(&mut app, target, FocusCause::Pressed);
    let request = latest(&app);
    assert!(!request.accepted);
    assert_eq!(
        request.observation::<SourceSnapshot>(),
        Some(&SourceSnapshot { target, value: 17 })
    );
    assert_eq!(app.world().resource::<InputFocus>().get(), None);

    app.world_mut().entity_mut(target).insert(SourceData(29));
    assert_eq!(
        request.observation::<SourceSnapshot>(),
        Some(&SourceSnapshot { target, value: 17 })
    );
    assert_eq!(
        latest(&app).observation::<SourceSnapshot>(),
        Some(&SourceSnapshot { target, value: 17 })
    );
    assert_eq!(
        app.world()
            .resource::<Calls>()
            .observation
            .load(Ordering::SeqCst),
        1
    );
}

#[test]
fn same_value_set_supersedes_a_prepared_deferred_request() {
    let mut app = app();
    let active = app.world_mut().spawn_empty().id();
    let pending = app.world_mut().spawn((Unavailable, SourceData(7))).id();
    set(&mut app, active, FocusCause::Navigated);
    set(&mut app, pending, FocusCause::Pressed);
    let request = latest(&app);
    let prepared = PreparedFocusChange::prepare_fulfil_deferred(
        app.world().resource::<InputFocus>(),
        &request,
    )
    .unwrap();
    set(&mut app, active, FocusCause::Pressed);
    assert_ne!(latest(&app).identity(), request.identity());
    let mut focus = app.world_mut().resource_mut::<InputFocus>();
    assert!(prepared.bind(&mut focus).is_err());
    assert_eq!(focus.get(), Some(active));
}

#[test]
fn empty_clear_supersedes_a_prepared_deferred_request() {
    let mut app = app();
    let pending = app.world_mut().spawn((Unavailable, SourceData(7))).id();
    set(&mut app, pending, FocusCause::Pressed);
    let request = latest(&app);
    let prepared = PreparedFocusChange::prepare_fulfil_deferred(
        app.world().resource::<InputFocus>(),
        &request,
    )
    .unwrap();
    app.world_mut().resource_mut::<InputFocus>().clear();
    assert_ne!(latest(&app).identity(), request.identity());
    let mut focus = app.world_mut().resource_mut::<InputFocus>();
    assert!(prepared.bind(&mut focus).is_err());
    assert_eq!(focus.get(), None);
}

#[test]
fn newly_inserted_focus_instance_rejects_the_old_prepared_request() {
    let mut app = app();
    let pending = app.world_mut().spawn((Unavailable, SourceData(7))).id();
    set(&mut app, pending, FocusCause::Pressed);
    let request = latest(&app);
    let old_instance = app.world().resource::<InputFocus>().instance_id().clone();
    let prepared = PreparedFocusChange::prepare_fulfil_deferred(
        app.world().resource::<InputFocus>(),
        &request,
    )
    .unwrap();
    app.insert_resource(InputFocus::default());
    assert_ne!(
        app.world().resource::<InputFocus>().instance_id(),
        &old_instance
    );
    let mut focus = app.world_mut().resource_mut::<InputFocus>();
    assert!(prepared.bind(&mut focus).is_err());
    assert_eq!(focus.get(), None);
}

#[test]
fn prepared_apply_keeps_original_identity_source_cause_and_fifo_without_callbacks() {
    let mut app = app();
    let a = app.world_mut().spawn_empty().id();
    let b = app.world_mut().spawn((Unavailable, SourceData(17))).id();
    let c = app.world_mut().spawn_empty().id();
    set(&mut app, a, FocusCause::Navigated);
    set(&mut app, b, FocusCause::Pressed);
    let request = latest(&app);
    // Ordinary UI readiness facts change before the caller prepares fulfilment.
    app.world_mut()
        .entity_mut(b)
        .remove::<Unavailable>()
        .insert(SourceData(29));
    reset_calls(&app);
    let prepared = PreparedFocusChange::prepare_fulfil_deferred(
        app.world().resource::<InputFocus>(),
        &request,
    )
    .unwrap();
    let retired = {
        let mut focus = app.world_mut().resource_mut::<InputFocus>();
        let bound = match prepared.bind(&mut focus) {
            Ok(bound) => bound,
            Err(_) => panic!("unchanged original request must bind"),
        };
        bound.apply()
    };
    assert_no_calls(&app);
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(b));
    let completed = latest(&app);
    assert_eq!(completed.identity(), request.identity());
    assert_eq!(completed.sequence, request.sequence);
    assert_eq!(completed.cause, Some(FocusCause::Pressed));
    assert_eq!(
        completed.observation::<SourceSnapshot>(),
        Some(&SourceSnapshot {
            target: b,
            value: 17
        })
    );
    assert!(completed.accepted);
    // Retired storage is released after the bounded mutation has returned.
    drop(retired);
    set(&mut app, c, FocusCause::Navigated);
    app.update();
    assert_eq!(
        take_events(&mut app),
        vec![
            Event::Gained(a, FocusCause::Navigated),
            Event::Lost(a),
            Event::Gained(b, FocusCause::Pressed),
            Event::Lost(b),
            Event::Gained(c, FocusCause::Navigated),
        ]
    );
}

#[test]
fn the_original_request_can_only_be_fulfilled_once() {
    let mut app = app();
    let pending = app.world_mut().spawn((Unavailable, SourceData(7))).id();
    set(&mut app, pending, FocusCause::Pressed);
    let request = latest(&app);
    let first = PreparedFocusChange::prepare_fulfil_deferred(
        app.world().resource::<InputFocus>(),
        &request,
    )
    .unwrap();
    let second = PreparedFocusChange::prepare_fulfil_deferred(
        app.world().resource::<InputFocus>(),
        &request,
    )
    .unwrap();
    app.world_mut().entity_mut(pending).remove::<Unavailable>();
    let retired = {
        let mut focus = app.world_mut().resource_mut::<InputFocus>();
        let bound = match first.bind(&mut focus) {
            Ok(bound) => bound,
            Err(_) => panic!("unchanged original request must bind"),
        };
        bound.apply()
    };
    drop(retired);
    let mut focus = app.world_mut().resource_mut::<InputFocus>();
    assert!(second.bind(&mut focus).is_err());
    assert!(PreparedFocusChange::prepare_fulfil_deferred(&focus, &request).is_err());
}

#[test]
fn revoke_then_fulfil_keeps_the_original_request_and_native_release_order() {
    let mut app = app();
    let old = app.world_mut().spawn_empty().id();
    let pending = app.world_mut().spawn((Unavailable, SourceData(41))).id();
    set(&mut app, old, FocusCause::Navigated);
    set(&mut app, pending, FocusCause::Pressed);
    let request = latest(&app);
    let prepared =
        PreparedFocusChange::prepare_revoke_if(app.world().resource::<InputFocus>(), &[old])
            .unwrap();
    let prepared = prepared.with_deferred(&request).unwrap();
    app.world_mut().entity_mut(pending).remove::<Unavailable>();
    reset_calls(&app);
    let retired = {
        let mut focus = app.world_mut().resource_mut::<InputFocus>();
        prepared.bind(&mut focus).unwrap().apply()
    };
    assert_no_calls(&app);
    assert!(retired.is_some());
    assert_eq!(latest(&app).identity(), request.identity());
    assert!(latest(&app).accepted);
    drop(retired);
    app.update();
    assert_eq!(
        take_events(&mut app),
        vec![
            Event::Gained(old, FocusCause::Navigated),
            Event::Lost(old),
            Event::Gained(pending, FocusCause::Pressed)
        ]
    );
}

#[test]
fn revocation_with_no_target_is_a_validation_only_noop() {
    let mut app = app();
    let active = app.world_mut().spawn_empty().id();
    let other = app.world_mut().spawn_empty().id();
    set(&mut app, active, FocusCause::Pressed);
    let before = app.world().resource::<InputFocus>().clone();
    let prepared = PreparedFocusChange::prepare_revoke_if(&before, &[other]).unwrap();
    reset_calls(&app);
    let retired = {
        let mut focus = app.world_mut().resource_mut::<InputFocus>();
        prepared.bind(&mut focus).unwrap().apply()
    };
    assert!(retired.is_none());
    assert_eq!(app.world().resource::<InputFocus>(), &before);
    assert_no_calls(&app);
    app.update();
    assert_eq!(
        take_events(&mut app),
        vec![Event::Gained(active, FocusCause::Pressed)]
    );
}

#[test]
fn tracked_binding_keeps_noop_ticks_and_marks_only_real_revocation() {
    let mut app = app();
    let active = app.world_mut().spawn_empty().id();
    let other = app.world_mut().spawn_empty().id();
    set(&mut app, active, FocusCause::Pressed);
    let before = app.world().resource::<InputFocus>().clone();
    let before_tick = app
        .world()
        .get_resource_change_ticks::<InputFocus>()
        .unwrap()
        .changed;
    app.world_mut().increment_change_tick();
    reset_calls(&app);

    let prepared = PreparedFocusChange::prepare_revoke_if(&before, &[other]).unwrap();
    let retired = prepared
        .bind_mut(app.world_mut().resource_mut::<InputFocus>())
        .unwrap()
        .apply();
    assert!(retired.is_none());
    assert_eq!(app.world().resource::<InputFocus>(), &before);
    assert_eq!(
        app.world()
            .get_resource_change_ticks::<InputFocus>()
            .unwrap()
            .changed,
        before_tick
    );

    app.world_mut().increment_change_tick();
    let prepared =
        PreparedFocusChange::prepare_revoke_if(app.world().resource::<InputFocus>(), &[active])
            .unwrap();
    let retired = prepared
        .bind_mut(app.world_mut().resource_mut::<InputFocus>())
        .unwrap()
        .apply();
    assert!(retired.is_some());
    assert_eq!(app.world().resource::<InputFocus>().get(), None);
    assert_ne!(
        app.world()
            .get_resource_change_ticks::<InputFocus>()
            .unwrap()
            .changed,
        before_tick
    );
    assert_no_calls(&app);
    drop(retired);
}
