use bevy_app::App;
use bevy_ecs::{prelude::*, system::RunSystemOnce, world::DeferredWorld};
use bevy_input::InputPlugin;
use bevy_input_focus::{
    AutoFocus, FocusCause, FocusGained, FocusLost, FocusWriter, InputDispatchPlugin, InputFocus,
    InputFocusPlugin, InputTargetEligibility, clear_focus, set_focus,
};
use bevy_window::{PrimaryWindow, Window};

#[derive(Component)]
struct Unavailable;

fn available(world: &World, entity: Entity) -> bool {
    world.entities().contains(entity) && world.get::<Unavailable>(entity).is_none()
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
    app.insert_resource(InputTargetEligibility::new(available));
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
    app
}

fn take_events(app: &mut App) -> Vec<Event> {
    core::mem::take(&mut app.world_mut().resource_mut::<Events>().0)
}

fn set(app: &mut App, entity: Entity, cause: FocusCause) {
    app.world_mut()
        .run_system_once(move |mut focus: FocusWriter| focus.set(entity, cause))
        .unwrap();
}

#[test]
fn synchronous_writer_checks_eligibility_and_keeps_previous_valid_focus() {
    let mut app = app();
    let a = app.world_mut().spawn_empty().id();
    let denied = app.world_mut().spawn(Unavailable).id();

    app.world_mut()
        .run_system_once(move |mut focus: FocusWriter| {
            focus.set(a, FocusCause::Pressed);
            assert_eq!(focus.get(), Some(a));
            focus.set(denied, FocusCause::Navigated);
            assert_eq!(focus.get(), Some(a));
            let latest = focus.latest_operation().unwrap();
            assert_eq!(latest.target, Some(denied));
            assert_eq!(latest.cause, Some(FocusCause::Navigated));
            assert!(!latest.accepted);
        })
        .unwrap();

    app.update();
    assert_eq!(
        take_events(&mut app),
        vec![Event::Gained(a, FocusCause::Pressed)]
    );
}

#[test]
fn deferred_world_helpers_use_the_same_synchronous_qualification() {
    let mut app = app();
    let a = app.world_mut().spawn_empty().id();
    let denied = app.world_mut().spawn(Unavailable).id();

    app.world_mut()
        .run_system_once(move |mut world: DeferredWorld| {
            set_focus(&mut world, a, FocusCause::Pressed);
            assert_eq!(world.resource::<InputFocus>().get(), Some(a));
            set_focus(&mut world, denied, FocusCause::Navigated);
            assert_eq!(world.resource::<InputFocus>().get(), Some(a));
            assert!(
                !world
                    .resource::<InputFocus>()
                    .latest_operation()
                    .unwrap()
                    .accepted
            );
            clear_focus(&mut world);
            assert_eq!(world.resource::<InputFocus>().get(), None);
        })
        .unwrap();

    app.update();
    assert_eq!(
        take_events(&mut app),
        vec![Event::Gained(a, FocusCause::Pressed), Event::Lost(a)]
    );
}

#[test]
fn denied_autofocus_never_becomes_effective_or_emits_focus_events() {
    let mut app = app();
    let denied = app.world_mut().spawn((Unavailable, AutoFocus)).id();

    let focus = app.world().resource::<InputFocus>();
    assert_eq!(focus.get(), None);
    let latest = focus.latest_operation().unwrap();
    assert_eq!(latest.target, Some(denied));
    assert_eq!(latest.cause, Some(FocusCause::Navigated));
    assert!(!latest.accepted);
    app.update();
    assert_eq!(app.world().resource::<InputFocus>().get(), None);
    assert!(take_events(&mut app).is_empty());
}

#[test]
fn valid_a_b_a_sequence_preserves_every_transition_and_cause() {
    let mut app = app();
    let a = app.world_mut().spawn_empty().id();
    let b = app.world_mut().spawn_empty().id();

    app.world_mut()
        .run_system_once(move |mut focus: FocusWriter| {
            focus.set(a, FocusCause::Navigated);
            assert_eq!(focus.get(), Some(a));
            focus.set(b, FocusCause::Pressed);
            assert_eq!(focus.get(), Some(b));
            focus.set(a, FocusCause::Pressed);
            assert_eq!(focus.get(), Some(a));
        })
        .unwrap();

    app.update();
    assert_eq!(
        take_events(&mut app),
        vec![
            Event::Gained(a, FocusCause::Navigated),
            Event::Lost(a),
            Event::Gained(b, FocusCause::Pressed),
            Event::Lost(b),
            Event::Gained(a, FocusCause::Pressed),
        ]
    );
}

#[test]
fn same_value_set_and_empty_clear_are_real_ordered_operations() {
    let mut app = app();
    let a = app.world_mut().spawn_empty().id();
    let denied = app.world_mut().spawn(Unavailable).id();
    set(&mut app, a, FocusCause::Navigated);
    app.update();
    take_events(&mut app);

    set(&mut app, denied, FocusCause::Navigated);
    let pending = app
        .world()
        .resource::<InputFocus>()
        .latest_operation()
        .unwrap();
    assert!(!pending.accepted);
    app.world_mut()
        .run_system_once(move |mut focus: FocusWriter| {
            focus.set(a, FocusCause::Pressed);
            let same_value = focus.latest_operation().unwrap();
            assert!(same_value.sequence > pending.sequence);
            assert!(same_value.accepted);
            assert_eq!(same_value.target, Some(a));
            assert_eq!(same_value.cause, Some(FocusCause::Pressed));
            focus.clear();
            let first_clear = focus.latest_operation().unwrap();
            assert!(first_clear.sequence > same_value.sequence);
            assert!(first_clear.accepted);
            assert_eq!(first_clear.target, None);
            assert_eq!(first_clear.cause, None);
            focus.clear();
            let empty_clear = focus.latest_operation().unwrap();
            assert!(empty_clear.sequence > first_clear.sequence);
            assert!(empty_clear.accepted);
            assert_eq!(empty_clear.target, None);
            assert_eq!(empty_clear.cause, None);
        })
        .unwrap();

    app.update();
    assert_eq!(take_events(&mut app), vec![Event::Lost(a)]);
}

#[test]
fn mutable_borrow_without_an_operation_does_not_supersede_the_latest_request() {
    let mut app = app();
    let denied = app.world_mut().spawn(Unavailable).id();
    set(&mut app, denied, FocusCause::Pressed);
    let before = app
        .world()
        .resource::<InputFocus>()
        .latest_operation()
        .unwrap();
    app.world_mut()
        .run_system_once(|_focus: ResMut<InputFocus>| {})
        .unwrap();
    let after = app
        .world()
        .resource::<InputFocus>()
        .latest_operation()
        .unwrap();
    assert_eq!(after.sequence, before.sequence);
    assert_eq!(after.target, before.target);
    assert_eq!(after.cause, before.cause);
    assert_eq!(after.accepted, before.accepted);
    assert_eq!(app.world().resource::<InputFocus>().get(), None);
}

#[test]
fn native_revocation_releases_effective_focus_without_superseding_pending_request() {
    let mut app = app();
    let a = app.world_mut().spawn_empty().id();
    let pending = app.world_mut().spawn(Unavailable).id();
    set(&mut app, a, FocusCause::Pressed);
    app.update();
    take_events(&mut app);

    set(&mut app, pending, FocusCause::Navigated);
    let before = app
        .world()
        .resource::<InputFocus>()
        .latest_operation()
        .unwrap();
    assert!(!before.accepted);
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(a));
    app.world_mut().entity_mut(a).insert(Unavailable);
    app.update();

    let focus = app.world().resource::<InputFocus>();
    assert_eq!(focus.get(), None);
    let after = focus.latest_operation().unwrap();
    assert_eq!(after.sequence, before.sequence);
    assert_eq!(after.target, Some(pending));
    assert_eq!(after.cause, before.cause);
    assert!(!after.accepted);
    assert_eq!(take_events(&mut app), vec![Event::Lost(a)]);
}

#[test]
fn target_invalidated_before_first_event_drain_never_emits_gained_or_lost() {
    let mut app = app();
    let a = app.world_mut().spawn_empty().id();
    set(&mut app, a, FocusCause::Pressed);
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(a));
    let before = app
        .world()
        .resource::<InputFocus>()
        .latest_operation()
        .unwrap();
    app.world_mut().entity_mut(a).insert(Unavailable);
    app.update();

    let focus = app.world().resource::<InputFocus>();
    assert_eq!(focus.get(), None);
    assert_eq!(focus.latest_operation().unwrap().sequence, before.sequence);
    assert!(take_events(&mut app).is_empty());
}

#[test]
fn existing_directional_navigation_uses_the_same_admission() {
    use bevy_input_focus::directional_navigation::{
        DirectionalNavigation, DirectionalNavigationError, DirectionalNavigationMap,
    };
    use bevy_math::CompassOctant;
    let mut app = app();
    let a = app.world_mut().spawn_empty().id();
    let b = app.world_mut().spawn(Unavailable).id();
    let mut map = DirectionalNavigationMap::default();
    map.add_looping_edges(&[a, b], CompassOctant::East);
    app.insert_resource(map);
    set(&mut app, a, FocusCause::Navigated);
    let result = app
        .world_mut()
        .run_system_once(|mut nav: DirectionalNavigation| nav.navigate(CompassOctant::East))
        .unwrap();
    assert_eq!(
        result,
        Err(DirectionalNavigationError::IneligibleTarget { target: b })
    );
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(a));
    assert!(
        !app.world()
            .resource::<InputFocus>()
            .latest_operation()
            .unwrap()
            .accepted
    );
    app.world_mut().entity_mut(b).remove::<Unavailable>();
    let result = app
        .world_mut()
        .run_system_once(|mut nav: DirectionalNavigation| nav.navigate(CompassOctant::East))
        .unwrap();
    assert_eq!(result, Ok(b));
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(b));
}
