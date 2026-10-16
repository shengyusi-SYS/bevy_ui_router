use bevy_app::App;
use bevy_ecs::{prelude::*, system::RunSystemOnce, world::DeferredWorld};
use bevy_input::{
    ButtonState, InputPlugin,
    keyboard::{Key, KeyCode, KeyboardInput},
};
use bevy_input_focus::{
    AcquireFocus, FocusCause, FocusedInput, InputDispatchPlugin, InputFocus, InputFocusPlugin,
    InputTargetEligibility, set_focus,
    tab_navigation::{NavAction, TabGroup, TabIndex, TabNavigation, TabNavigationPlugin},
};
use bevy_window::{PrimaryWindow, Window};

#[derive(Component)]
struct Denied;

#[derive(Resource, Default)]
struct KeyboardTargets(Vec<Entity>);

fn accepts_target(world: &World, entity: Entity) -> bool {
    world.get::<Denied>(entity).is_none()
}

fn record_keyboard(event: On<FocusedInput<KeyboardInput>>, mut targets: ResMut<KeyboardTargets>) {
    if event.input.key_code == KeyCode::KeyA && event.input.state == ButtonState::Pressed {
        targets.0.push(event.focused_entity);
    }
}

fn keyboard(window: Entity, key_code: KeyCode, state: ButtonState) -> KeyboardInput {
    let logical_key = match key_code {
        KeyCode::Tab => Key::Tab,
        KeyCode::ShiftLeft => Key::Shift,
        _ => Key::Character("a".into()),
    };
    KeyboardInput {
        key_code,
        logical_key,
        state,
        text: None,
        repeat: false,
        window,
    }
}

fn tab(app: &mut App, window: Entity, reverse: bool) {
    app.world_mut().write_message(keyboard(
        window,
        KeyCode::ShiftLeft,
        if reverse {
            ButtonState::Pressed
        } else {
            ButtonState::Released
        },
    ));
    app.world_mut()
        .write_message(keyboard(window, KeyCode::Tab, ButtonState::Released));
    app.world_mut()
        .write_message(keyboard(window, KeyCode::Tab, ButtonState::Pressed));
    app.update();
}

fn focus(app: &mut App, entity: Entity) {
    let mut world: DeferredWorld = app.world_mut().into();
    set_focus(&mut world, entity, FocusCause::Navigated);
}

fn current(app: &App) -> Option<Entity> {
    app.world().resource::<InputFocus>().get()
}

#[test]
fn native_plugins_share_eligibility_without_replacing_navigation_or_bubbling() {
    let mut app = App::new();
    app.add_plugins((
        InputPlugin,
        InputFocusPlugin,
        InputDispatchPlugin,
        TabNavigationPlugin,
    ))
    .init_resource::<KeyboardTargets>()
    .insert_resource(InputTargetEligibility::new(accepts_target))
    .add_observer(record_keyboard);

    let window = app
        .world_mut()
        .spawn((Window::default(), PrimaryWindow))
        .id();
    // Spawn the later group first: native group ordering must still take precedence.
    let late_group = app.world_mut().spawn(TabGroup::new(20)).id();
    let late = app
        .world_mut()
        .spawn((TabIndex(0), ChildOf(late_group)))
        .id();
    let host = app
        .world_mut()
        .spawn((TabGroup::new(10), TabIndex(-1)))
        .id();
    let denied_wrapper = app
        .world_mut()
        .spawn((TabIndex(0), Denied, ChildOf(host)))
        .id();
    let legal_descendant = app
        .world_mut()
        .spawn((TabIndex(2), ChildOf(denied_wrapper)))
        .id();
    let first = app.world_mut().spawn((TabIndex(1), ChildOf(host))).id();
    let modal = app.world_mut().spawn(TabGroup::modal()).id();
    let modal_first = app.world_mut().spawn((TabIndex(0), ChildOf(modal))).id();
    let modal_second = app.world_mut().spawn((TabIndex(1), ChildOf(modal))).id();
    app.update();

    // Native Tab and Shift+Tab preserve group/index ordering. Rejecting a wrapper
    // excludes only that candidate and leaves its legitimate child reachable.
    tab(&mut app, window, false);
    assert_eq!(current(&app), Some(first));
    tab(&mut app, window, false);
    assert_eq!(current(&app), Some(legal_descendant));
    tab(&mut app, window, false);
    assert_eq!(current(&app), Some(late));
    tab(&mut app, window, true);
    assert_eq!(current(&app), Some(legal_descendant));

    // An ordinary hierarchy edit is observed on the next native navigation.
    let dynamic_descendant = app
        .world_mut()
        .spawn((TabIndex(3), ChildOf(denied_wrapper)))
        .id();
    tab(&mut app, window, false);
    assert_eq!(current(&app), Some(dynamic_descendant));

    // Eligible modal focus keeps the stock modal cycle, including reverse wrap.
    focus(&mut app, modal_first);
    tab(&mut app, window, false);
    assert_eq!(current(&app), Some(modal_second));
    tab(&mut app, window, false);
    assert_eq!(current(&app), Some(modal_first));
    tab(&mut app, window, true);
    assert_eq!(current(&app), Some(modal_second));

    // An ineligible old modal focus must not trap navigation in its old modal.
    focus(&mut app, modal_first);
    app.world_mut().entity_mut(modal_first).insert(Denied);
    let next = app
        .world_mut()
        .run_system_once(|nav: TabNavigation, focus: Res<InputFocus>| {
            nav.navigate(&focus, NavAction::Next)
        })
        .unwrap();
    assert_eq!(next, Ok(first));
    tab(&mut app, window, false);
    assert_eq!(current(&app), Some(first));

    // AcquireFocus follows the stock ancestor traversal after rejecting its
    // original candidate. It does not stop at a denied wrapper.
    app.world_mut().trigger(AcquireFocus {
        focused_entity: denied_wrapper,
        window,
    });
    assert_eq!(current(&app), Some(host));
    app.world_mut().trigger(AcquireFocus {
        focused_entity: legal_descendant,
        window,
    });
    assert_eq!(current(&app), Some(legal_descendant));
    focus(&mut app, denied_wrapper);
    let rejected = app
        .world()
        .resource::<InputFocus>()
        .latest_operation()
        .unwrap();
    assert!(!rejected.accepted);
    app.world_mut().trigger(AcquireFocus {
        focused_entity: legal_descendant,
        window,
    });
    let reasserted = app
        .world()
        .resource::<InputFocus>()
        .latest_operation()
        .unwrap();
    assert_eq!(reasserted.sequence, rejected.sequence + 1);
    assert_eq!(reasserted.target, Some(legal_descendant));
    assert!(reasserted.accepted);

    // Qualification is checked only for the starting target. Normal bubbling
    // still visits the denied wrapper, app-owned host, and primary window.
    app.world_mut().resource_mut::<KeyboardTargets>().0.clear();
    app.world_mut()
        .write_message(keyboard(window, KeyCode::KeyA, ButtonState::Pressed));
    app.update();
    assert_eq!(
        app.world().resource::<KeyboardTargets>().0,
        vec![legal_descendant, denied_wrapper, host, window]
    );

    // Revoking a live focused entity prevents dispatch to it on the very next
    // native message. The unchanged fallback is the primary window.
    app.world_mut().entity_mut(legal_descendant).insert(Denied);
    app.world_mut().resource_mut::<KeyboardTargets>().0.clear();
    app.world_mut()
        .write_message(keyboard(window, KeyCode::KeyA, ButtonState::Pressed));
    app.update();
    assert_eq!(app.world().resource::<KeyboardTargets>().0, vec![window]);
    assert_eq!(current(&app), None);

    // Removing the provider restores stock behavior, including the previously
    // rejected wrapper's original index and ordinary group order.
    app.world_mut().remove_resource::<InputTargetEligibility>();
    tab(&mut app, window, false);
    assert_eq!(current(&app), Some(denied_wrapper));
    tab(&mut app, window, false);
    assert_eq!(current(&app), Some(first));
    tab(&mut app, window, false);
    assert_eq!(current(&app), Some(legal_descendant));
}
