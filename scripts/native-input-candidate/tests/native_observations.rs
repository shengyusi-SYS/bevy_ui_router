use bevy_app::App;
use bevy_ecs::prelude::*;
use bevy_input::{
    ButtonState, InputPlugin,
    keyboard::{Key, KeyCode, KeyboardInput},
};
use bevy_input_focus::{
    AcquireFocus, FocusCause, FocusObservation, FocusOperation, FocusOperationObserver,
    InputDispatchPlugin, InputFocus, InputFocusPlugin,
    tab_navigation::{TabGroup, TabIndex, TabNavigationPlugin},
};
use bevy_window::{PrimaryWindow, Window};
use std::sync::Arc;

#[derive(Component)]
struct SourceData(u32);
fn observe(world: &World, operation: &FocusOperation) -> Option<FocusObservation> {
    Some(Arc::new(world.get::<SourceData>(operation.target?)?.0))
}
fn latest(app: &App) -> FocusOperation {
    app.world()
        .resource::<InputFocus>()
        .latest_operation()
        .unwrap()
}
#[test]
fn native_initial_focus_acquire_and_tab_use_synchronous_source_observation() {
    let mut app = App::new();
    app.add_plugins((
        InputPlugin,
        InputFocusPlugin,
        InputDispatchPlugin,
        TabNavigationPlugin,
    ));
    app.insert_resource(FocusOperationObserver::new(observe));
    let window = app
        .world_mut()
        .spawn((Window::default(), PrimaryWindow, SourceData(10)))
        .id();
    let group = app.world_mut().spawn(TabGroup::new(0)).id();
    let a = app
        .world_mut()
        .spawn((TabIndex(0), ChildOf(group), SourceData(20)))
        .id();
    let b = app
        .world_mut()
        .spawn((TabIndex(1), ChildOf(group), SourceData(30)))
        .id();
    app.update();
    assert_eq!(latest(&app).observation::<u32>(), Some(&10));
    app.world_mut().trigger(AcquireFocus {
        focused_entity: a,
        window,
    });
    let first = latest(&app);
    assert_eq!(first.observation::<u32>(), Some(&20));
    app.world_mut().entity_mut(a).insert(SourceData(21));
    app.world_mut().trigger(AcquireFocus {
        focused_entity: a,
        window,
    });
    assert_ne!(first.identity(), latest(&app).identity());
    assert_eq!(first.observation::<u32>(), Some(&20));
    assert_eq!(latest(&app).observation::<u32>(), Some(&21));
    app.world_mut().write_message(KeyboardInput {
        key_code: KeyCode::Tab,
        logical_key: Key::Tab,
        state: ButtonState::Pressed,
        text: None,
        repeat: false,
        window,
    });
    app.update();
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(b));
    assert_eq!(latest(&app).observation::<u32>(), Some(&30));
    assert_eq!(latest(&app).cause, Some(FocusCause::Navigated));
    app.world_mut().trigger(AcquireFocus {
        focused_entity: window,
        window,
    });
    let first_clear = latest(&app);
    assert_eq!(first_clear.target, None);
    app.world_mut().trigger(AcquireFocus {
        focused_entity: window,
        window,
    });
    assert_eq!(app.world().resource::<InputFocus>().get(), None);
    assert_ne!(first_clear.identity(), latest(&app).identity());
}
