//! Native BSN input through the stable facade and ordinary application updates.
//!
//! PointerInput is the upstream input-device boundary. Hover maps and gesture
//! caches are only inspected: the official UI backend and picking dispatcher
//! create them. No Router lifecycle systems or intermediate schedules are run.

use std::collections::BTreeMap;

use bevy_app::App;
use bevy_asset::AssetPlugin;
use bevy_ecs::prelude::*;
use bevy_input_focus::{FocusCause, InputFocus, InputFocusPlugin};
use bevy_internal::{
    DefaultPlugins,
    camera::{Camera2d, NormalizedRenderTarget},
    math::Vec2,
    window::{PrimaryWindow, Window, WindowRef},
};
use bevy_picking::{
    events::{Click, Drag, DragEnd, Pointer, PointerState, Press, Release},
    hover::{HoverMap, PickingInteraction, PreviousHoverMap},
    pointer::{
        Location, PointerAction, PointerButton, PointerId, PointerInput, PointerInteraction,
        PointerLocation, PointerPress,
    },
};
use bevy_scene::{ScenePlugin, bsn};
use bevy_ui::{ComputedNode, Node, PositionType, UiGlobalTransform, Val};
use bevy_ui_router::prelude::*;

#[derive(Component, Clone, Default)]
struct NativeInputPage;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Deliveries {
    presses: usize,
    drags: usize,
    releases: usize,
    clicks: usize,
    drag_ends: usize,
}

#[derive(Resource, Default)]
struct PointerDeliveries(BTreeMap<Entity, Deliveries>);

fn observe_pointer_delivery(app: &mut App) {
    app.init_resource::<PointerDeliveries>();
    app.world_mut().add_observer(
        |event: On<Pointer<Press>>, mut deliveries: ResMut<PointerDeliveries>| {
            deliveries
                .0
                .entry(event.event_target())
                .or_default()
                .presses += 1;
        },
    );
    app.world_mut().add_observer(
        |event: On<Pointer<Drag>>, mut deliveries: ResMut<PointerDeliveries>| {
            deliveries.0.entry(event.event_target()).or_default().drags += 1;
        },
    );
    app.world_mut().add_observer(
        |event: On<Pointer<Release>>, mut deliveries: ResMut<PointerDeliveries>| {
            deliveries
                .0
                .entry(event.event_target())
                .or_default()
                .releases += 1;
        },
    );
    app.world_mut().add_observer(
        |event: On<Pointer<Click>>, mut deliveries: ResMut<PointerDeliveries>| {
            deliveries.0.entry(event.event_target()).or_default().clicks += 1;
        },
    );
    app.world_mut().add_observer(
        |event: On<Pointer<DragEnd>>, mut deliveries: ResMut<PointerDeliveries>| {
            deliveries
                .0
                .entry(event.event_target())
                .or_default()
                .drag_ends += 1;
        },
    );
}

fn await_success(app: &mut App, router: RouterHandle, request: NavigationRequestId) {
    for _ in 0..32 {
        app.update();
        let result = app
            .world()
            .resource::<RouterSnapshots>()
            .request(router, request);
        if matches!(result, RequestLookup::Found(request) if request.status() == &RequestStatus::Succeeded)
        {
            return;
        }
        assert!(
            !matches!(result, RequestLookup::Found(request) if matches!(request.status(), RequestStatus::Failed(_))),
            "native navigation failed: {result:?}"
        );
    }
    panic!(
        "native navigation did not converge: {:?}",
        app.world()
            .resource::<RouterSnapshots>()
            .request(router, request)
    );
}

fn location_at(world: &World, window: Entity, target: Entity) -> Location {
    assert!(
        world
            .get::<ComputedNode>(target)
            .unwrap()
            .size()
            .cmpgt(Vec2::ZERO)
            .all(),
        "a hit-test target must have a real, nonempty computed UI layout"
    );
    Location {
        target: NormalizedRenderTarget::Window(WindowRef::Primary.normalize(Some(window)).unwrap()),
        position: *world.get::<UiGlobalTransform>(target).unwrap() * Vec2::ZERO,
    }
}

fn send_inputs(app: &mut App, inputs: impl IntoIterator<Item = PointerInput>) {
    for input in inputs {
        app.world_mut().write_message(input);
    }
    app.update();
}

fn assert_target_revoked(world: &World, target: Entity) {
    let gate = world.resource::<RouterInputGate>();
    assert!(!gate.allows_pointer(target));
    assert!(!gate.allows_keyboard(target));
    assert!(!gate.allows_gamepad(target));
    assert!(
        world
            .resource::<HoverMap>()
            .values()
            .all(|hits| !hits.contains_key(&target))
    );
    assert!(
        world
            .resource::<PreviousHoverMap>()
            .values()
            .all(|hits| !hits.contains_key(&target))
    );
    let state = world.resource::<PointerState>();
    for button in state.pointer_buttons.values() {
        assert!(!button.pressing.contains_key(&target));
        assert!(!button.clicking.contains_key(&target));
        assert!(!button.dragging.contains_key(&target));
        assert!(!button.dragging_over.contains_key(&target));
    }
    assert!(state.get_ancestors(&target).is_none());
    assert!(
        state
            .hovered_entity_ancestors
            .values()
            .all(|ancestors| !ancestors.contains(&target))
    );
    assert!(
        world
            .get::<PickingInteraction>(target)
            .is_none_or(|state| *state == PickingInteraction::None)
    );
}

#[derive(Clone, Copy, Debug)]
enum Departure {
    Replace,
    Back,
    Fallback,
    LoseHost,
    KeepAlive,
}

fn native_input_journey(departure: Departure, focus_on_application: bool) {
    let mut app = App::new();
    // These Cargo-selected default plugins provide headless window, camera,
    // layout and picking. No Winit event loop or GPU renderer is required.
    app.add_plugins(DefaultPlugins);
    if !app.is_plugin_added::<AssetPlugin>() {
        app.add_plugins(AssetPlugin::default());
    }
    if !app.is_plugin_added::<ScenePlugin>() {
        app.add_plugins(ScenePlugin);
    }
    if !app.is_plugin_added::<InputFocusPlugin>() {
        app.add_plugins(InputFocusPlugin);
    }
    observe_pointer_delivery(&mut app);
    let window = app
        .world_mut()
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(app.world())
        .unwrap();
    app.world_mut()
        .get_mut::<Window>(window)
        .unwrap()
        .resolution
        .set(320.0, 180.0);
    app.world_mut().spawn(Camera2d);

    let driver = app
        .register_bsn_view(ViewDriverKey::new("native-input").unwrap(), |_| {
            Ok(bsn! {
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(20.0),
                    top: Val::Px(20.0),
                    width: Val::Px(48.0),
                    height: Val::Px(32.0),
                }
                NativeInputPage
            })
        })
        .unwrap();
    let view = RouteViewKey::new("native-input").unwrap();
    let mut views = RouteViewRegistry::new();
    views
        .insert(view.clone(), RouteViewDescriptor::new(driver))
        .unwrap();
    let root_view = if matches!(departure, Departure::KeepAlive) {
        let layout = app
            .register_bsn_view(ViewDriverKey::new("native-input-layout").unwrap(), |_| {
                Ok(bsn! {
                    Node { width: Val::Percent(100.0), height: Val::Percent(100.0) }
                    bevy_picking::Pickable { should_block_lower: false, is_hoverable: false }
                    DefaultOutlet
                })
            })
            .unwrap();
        let key = RouteViewKey::new("native-input-layout").unwrap();
        views
            .insert(key.clone(), RouteViewDescriptor::new(layout))
            .unwrap();
        key
    } else {
        view.clone()
    };
    let mut routes = RouteTree::builder()
        .root_with_options(
            RoutePattern::path("/").unwrap(),
            RouteNodeOptions::new().with_view(root_view),
        )
        .unwrap();
    if matches!(departure, Departure::KeepAlive) {
        routes
            .route_with_options(
                routes.root_id().unwrap(),
                RoutePattern::path("items/:id").unwrap(),
                RouteNodeOptions::new()
                    .with_view(view.clone())
                    .with_slot(RouteSlotKey::default())
                    .with_keep_alive(true),
            )
            .unwrap();
    }
    let capacity = if matches!(departure, Departure::KeepAlive) {
        2
    } else {
        0
    };
    let router = app
        .add_ui_router(
            RouterConfig::new(routes.build().unwrap(), views, view)
                .with_keep_alive_capacity(capacity),
        )
        .unwrap();
    let host = app
        .world_mut()
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..Node::default()
            },
            RouterHostRegistration::new(router),
        ))
        .id();
    let application_target = app
        .world_mut()
        .spawn(Node {
            position_type: PositionType::Absolute,
            left: Val::Px(140.0),
            top: Val::Px(60.0),
            width: Val::Px(48.0),
            height: Val::Px(32.0),
            ..Node::default()
        })
        .id();
    app.update();
    let initial_location = if matches!(departure, Departure::KeepAlive) {
        "/items/1"
    } else {
        "/"
    };
    let initial = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, RouteLocation::new(initial_location))
        .unwrap();
    await_success(&mut app, router, initial);
    if matches!(departure, Departure::Back) {
        let second = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(
                router,
                RouteLocation::new("/").with_fragment("second-entry"),
            )
            .unwrap();
        await_success(&mut app, router, second);
        assert_eq!(
            app.world()
                .resource::<RouterSnapshots>()
                .get(router)
                .unwrap()
                .history()
                .len(),
            2
        );
    }
    // The regular next frame consumes the published tree in layout/picking.
    app.update();
    let router_target = app
        .world_mut()
        .query_filtered::<Entity, With<NativeInputPage>>()
        .single(app.world())
        .unwrap();
    assert!(
        app.world()
            .resource::<RouterInputGate>()
            .allows_pointer(router_target)
    );
    let router_location = location_at(app.world(), window, router_target);
    let application_location = location_at(app.world(), window, application_target);
    let router_pointer = PointerId::Touch(0x4253_4e01);
    let application_pointer = PointerId::Touch(0x4253_4e02);
    for (pointer, location) in [
        (router_pointer, router_location.clone()),
        (application_pointer, application_location.clone()),
    ] {
        app.world_mut().spawn((
            pointer,
            PointerLocation::new(location),
            PointerPress::default(),
            PointerInteraction::default(),
        ));
    }
    app.update();
    let hover = app.world().resource::<HoverMap>();
    assert!(
        hover
            .get(&router_pointer)
            .is_some_and(|hits| hits.contains_key(&router_target)),
        "the native BSN page must be hit by the real UI picking backend"
    );
    assert!(
        hover
            .get(&application_pointer)
            .is_some_and(|hits| hits.contains_key(&application_target)),
        "the independent application target must also be hit"
    );

    let focused = if focus_on_application {
        application_target
    } else {
        router_target
    };
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(focused, FocusCause::Navigated);
    app.update();
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(focused));

    send_inputs(
        &mut app,
        [
            PointerInput::new(
                router_pointer,
                router_location.clone(),
                PointerAction::Press(PointerButton::Primary),
            ),
            PointerInput::new(
                application_pointer,
                application_location.clone(),
                PointerAction::Press(PointerButton::Primary),
            ),
        ],
    );
    let moved_router = Location {
        position: router_location.position + Vec2::X,
        ..router_location
    };
    let moved_application = Location {
        position: application_location.position + Vec2::X,
        ..application_location
    };
    send_inputs(
        &mut app,
        [
            PointerInput::new(
                router_pointer,
                moved_router.clone(),
                PointerAction::Move { delta: Vec2::X },
            ),
            PointerInput::new(
                application_pointer,
                moved_application.clone(),
                PointerAction::Move { delta: Vec2::X },
            ),
        ],
    );
    let before = app.world().resource::<PointerDeliveries>().0.clone();
    for target in [router_target, application_target] {
        assert_eq!(before[&target].presses, 1);
        assert_eq!(before[&target].drags, 1);
        assert_eq!(
            app.world().get::<PickingInteraction>(target),
            Some(&PickingInteraction::Pressed)
        );
    }

    match departure {
        Departure::Replace => {
            let replacement = app
                .world_mut()
                .resource_mut::<RouterNavigation>()
                .replace(router, RouteLocation::new("/").with_fragment("replacement"))
                .unwrap();
            await_success(&mut app, router, replacement);
            let next_target = app
                .world_mut()
                .query_filtered::<Entity, With<NativeInputPage>>()
                .single(app.world())
                .unwrap();
            assert_ne!(next_target, router_target);
            assert!(
                app.world()
                    .resource::<RouterInputGate>()
                    .allows_pointer(next_target)
            );
        }
        Departure::Back => {
            let back = app
                .world_mut()
                .resource_mut::<RouterNavigation>()
                .back(router)
                .unwrap();
            await_success(&mut app, router, back);
            let snapshot = app
                .world()
                .resource::<RouterSnapshots>()
                .get(router)
                .unwrap();
            assert_eq!(snapshot.active().kind(), ActivePresentationKind::Route);
            assert_eq!(snapshot.active().location(), Some(&RouteLocation::new("/")));
            assert_eq!(snapshot.history().cursor(), Some(0));
            assert_eq!(snapshot.history().current(), Some(&RouteLocation::new("/")));
            assert_eq!(
                snapshot.history().entries(),
                [
                    RouteLocation::new("/"),
                    RouteLocation::new("/").with_fragment("second-entry")
                ]
            );
            let next_target = app
                .world_mut()
                .query_filtered::<Entity, With<NativeInputPage>>()
                .single(app.world())
                .unwrap();
            assert_ne!(next_target, router_target);
            assert!(
                app.world()
                    .resource::<RouterInputGate>()
                    .allows_pointer(next_target)
            );
        }
        Departure::Fallback => {
            let failed = app
                .world_mut()
                .resource_mut::<RouterNavigation>()
                .push(router, RouteLocation::new("/missing"))
                .unwrap();
            for _ in 0..32 {
                app.update();
                if matches!(app.world().resource::<RouterSnapshots>().request(router, failed),
                    RequestLookup::Found(request) if matches!(request.status(), RequestStatus::Failed(_)))
                {
                    break;
                }
            }
            assert!(
                matches!(app.world().resource::<RouterSnapshots>().request(router, failed),
                RequestLookup::Found(request) if matches!(request.status(), RequestStatus::Failed(error)
                    if error.kind() == NavigationFailureKind::Match))
            );
            let snapshot = app
                .world()
                .resource::<RouterSnapshots>()
                .get(router)
                .unwrap();
            assert_eq!(snapshot.active().kind(), ActivePresentationKind::Error);
            assert_eq!(snapshot.history().len(), 1);
            let next_target = app
                .world_mut()
                .query_filtered::<Entity, With<NativeInputPage>>()
                .single(app.world())
                .unwrap();
            assert_ne!(next_target, router_target);
            assert!(
                app.world()
                    .resource::<RouterInputGate>()
                    .allows_pointer(next_target)
            );
        }
        Departure::KeepAlive => {
            let second = app
                .world_mut()
                .resource_mut::<RouterNavigation>()
                .push(router, RouteLocation::new("/items/2"))
                .unwrap();
            await_success(&mut app, router, second);
            assert!(
                app.world().get::<NativeInputPage>(router_target).is_some(),
                "retention must keep the original native Entity"
            );
            let pages = app
                .world_mut()
                .query_filtered::<Entity, With<NativeInputPage>>()
                .iter(app.world())
                .collect::<Vec<_>>();
            assert_eq!(pages.len(), 2);
            assert!(pages.iter().any(|target| {
                *target != router_target
                    && app
                        .world()
                        .resource::<RouterInputGate>()
                        .allows_pointer(*target)
            }));
        }
        Departure::LoseHost => {
            // Loss while idle must be observed without enqueuing another request.
            app.world_mut()
                .entity_mut(host)
                .remove::<RouterHostRegistration>();
            for _ in 0..32 {
                app.update();
                let snapshot = app
                    .world()
                    .resource::<RouterSnapshots>()
                    .get(router)
                    .unwrap();
                if snapshot.availability()
                    == &RouterAvailability::Unavailable(RouterUnavailableReason::HostLost)
                    && snapshot.active().kind() == ActivePresentationKind::None
                {
                    break;
                }
            }
            let snapshot = app
                .world()
                .resource::<RouterSnapshots>()
                .get(router)
                .unwrap();
            assert_eq!(
                snapshot.availability(),
                &RouterAvailability::Unavailable(RouterUnavailableReason::HostLost)
            );
            assert_eq!(snapshot.active().kind(), ActivePresentationKind::None);
        }
    }
    assert!(
        app.world().get_entity(host).is_ok(),
        "Router must preserve the application host"
    );
    assert!(
        matches!(app.world().resource::<RouterSnapshots>().request(router, initial), RequestLookup::Found(request) if request.status() == &RequestStatus::Succeeded)
    );
    assert_target_revoked(app.world(), router_target);
    assert_eq!(
        app.world().resource::<InputFocus>().get(),
        focus_on_application.then_some(application_target),
        "departure={departure:?}"
    );
    let state = app.world().resource::<PointerState>();
    let button = state
        .get(application_pointer, PointerButton::Primary)
        .unwrap();
    assert!(button.pressing.contains_key(&application_target));
    assert!(button.clicking.contains_key(&application_target));
    assert!(button.dragging.contains_key(&application_target));
    assert_eq!(
        app.world().get::<PickingInteraction>(application_target),
        Some(&PickingInteraction::Pressed)
    );

    let moved_router_again = Location {
        position: moved_router.position + Vec2::X,
        ..moved_router
    };
    let moved_application_again = Location {
        position: moved_application.position + Vec2::X,
        ..moved_application
    };
    send_inputs(
        &mut app,
        [
            PointerInput::new(
                router_pointer,
                moved_router_again.clone(),
                PointerAction::Move { delta: Vec2::X },
            ),
            PointerInput::new(
                application_pointer,
                moved_application_again.clone(),
                PointerAction::Move { delta: Vec2::X },
            ),
        ],
    );
    let after_move = &app.world().resource::<PointerDeliveries>().0;
    assert_eq!(
        after_move[&router_target], before[&router_target],
        "old native content must receive no continued gesture"
    );
    assert_eq!(
        after_move[&application_target].drags,
        before[&application_target].drags + 1
    );
    send_inputs(
        &mut app,
        [
            PointerInput::new(
                router_pointer,
                moved_router_again,
                PointerAction::Release(PointerButton::Primary),
            ),
            PointerInput::new(
                application_pointer,
                moved_application_again,
                PointerAction::Release(PointerButton::Primary),
            ),
        ],
    );
    let after_release = &app.world().resource::<PointerDeliveries>().0;
    assert_eq!(
        after_release[&router_target], before[&router_target],
        "old native content must receive no historical release, click or drag end"
    );
    assert_eq!(after_release[&application_target].releases, 1);
    assert_eq!(after_release[&application_target].clicks, 1);
    assert_eq!(after_release[&application_target].drag_ends, 1);
    assert_target_revoked(app.world(), router_target);
    assert_eq!(
        app.world().resource::<InputFocus>().get(),
        focus_on_application.then_some(application_target)
    );

    if matches!(departure, Departure::KeepAlive) {
        let cached_other = app
            .world_mut()
            .query_filtered::<Entity, With<NativeInputPage>>()
            .iter(app.world())
            .find(|target| *target != router_target)
            .unwrap();
        let hit = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/items/1"))
            .unwrap();
        await_success(&mut app, router, hit);
        app.update();
        assert!(app.world().get::<NativeInputPage>(router_target).is_some());
        let gate = app.world().resource::<RouterInputGate>();
        assert!(gate.allows_pointer(router_target));
        assert!(gate.allows_keyboard(router_target));
        assert!(gate.allows_gamepad(router_target));
        assert_target_revoked(app.world(), cached_other);
        assert_eq!(
            app.world().resource::<InputFocus>().get(),
            focus_on_application.then_some(application_target),
            "reactivation must not replay historical focus"
        );
        let restored_location = location_at(app.world(), window, router_target);
        send_inputs(
            &mut app,
            [PointerInput::new(
                router_pointer,
                restored_location.clone(),
                PointerAction::Move { delta: Vec2::ZERO },
            )],
        );
        assert!(
            app.world()
                .resource::<HoverMap>()
                .get(&router_pointer)
                .is_some_and(|hits| hits.contains_key(&router_target))
        );
        let before_new_gesture = app.world().resource::<PointerDeliveries>().0.clone();
        send_inputs(
            &mut app,
            [PointerInput::new(
                router_pointer,
                restored_location.clone(),
                PointerAction::Press(PointerButton::Primary),
            )],
        );
        let restored_moved = Location {
            position: restored_location.position + Vec2::X,
            ..restored_location
        };
        send_inputs(
            &mut app,
            [PointerInput::new(
                router_pointer,
                restored_moved.clone(),
                PointerAction::Move { delta: Vec2::X },
            )],
        );
        send_inputs(
            &mut app,
            [PointerInput::new(
                router_pointer,
                restored_moved,
                PointerAction::Release(PointerButton::Primary),
            )],
        );
        let deliveries = &app.world().resource::<PointerDeliveries>().0;
        let before = before_new_gesture[&router_target];
        assert_eq!(
            deliveries[&router_target],
            Deliveries {
                presses: before.presses + 1,
                drags: before.drags + 1,
                releases: before.releases + 1,
                clicks: before.clicks + 1,
                drag_ends: before.drag_ends + 1,
            }
        );
        assert_eq!(
            deliveries[&application_target],
            before_new_gesture[&application_target]
        );
    }
}

#[test]
fn native_keep_alive_revokes_old_input_and_accepts_a_new_gesture_after_hit() {
    for focus_on_application in [false, true] {
        native_input_journey(Departure::KeepAlive, focus_on_application);
    }
}

#[test]
fn native_replacement_revokes_existing_gestures_and_only_departing_focus() {
    for focus_on_application in [false, true] {
        native_input_journey(Departure::Replace, focus_on_application);
    }
}

#[test]
fn native_back_revokes_existing_gestures_and_only_departing_focus() {
    for focus_on_application in [false, true] {
        native_input_journey(Departure::Back, focus_on_application);
    }
}

#[test]
fn native_fallback_revokes_existing_gestures_and_only_departing_focus() {
    for focus_on_application in [false, true] {
        native_input_journey(Departure::Fallback, focus_on_application);
    }
}

#[test]
fn native_idle_host_loss_revokes_existing_gestures_and_only_departing_focus() {
    for focus_on_application in [false, true] {
        native_input_journey(Departure::LoseHost, focus_on_application);
    }
}

// Review overlay on the r1 source: actual official visibility/layout and public
// navigation are the evidence. No private eligibility or lifecycle is seeded.
mod live_content_eligibility_review {
    use super::*;
    use bevy_camera::visibility::{InheritedVisibility, Visibility};
    use bevy_input_focus::AutoFocus;
    use bevy_scene::{ResolvedScene, SceneFunction};

    #[derive(Component, Clone, Default)]
    struct ReviewParent;
    #[derive(Component, Clone, Default)]
    struct ReviewTarget;
    #[derive(Component, Clone, Default)]
    struct OtherActive;

    #[derive(Resource, Default)]
    struct FocusEvents {
        lost: Vec<Entity>,
        gained: Vec<(Entity, FocusCause)>,
    }

    fn official_app() -> App {
        let mut app = App::new();
        app.init_resource::<FocusEvents>();
        app.add_observer(
            |mut event: On<bevy_input_focus::FocusLost>, mut events: ResMut<FocusEvents>| {
                events.lost.push(event.entity);
                event.propagate(false);
            },
        );
        app.add_observer(
            |mut event: On<bevy_input_focus::FocusGained>, mut events: ResMut<FocusEvents>| {
                events.gained.push((event.entity, event.cause));
                event.propagate(false);
            },
        );
        app.add_plugins(DefaultPlugins);
        if !app.is_plugin_added::<AssetPlugin>() {
            app.add_plugins(AssetPlugin::default());
        }
        if !app.is_plugin_added::<ScenePlugin>() {
            app.add_plugins(ScenePlugin);
        }
        if !app.is_plugin_added::<InputFocusPlugin>() {
            app.add_plugins(InputFocusPlugin);
        }
        let window = app
            .world_mut()
            .query_filtered::<Entity, With<PrimaryWindow>>()
            .single(app.world())
            .unwrap();
        app.world_mut()
            .get_mut::<Window>(window)
            .unwrap()
            .resolution
            .set(320.0, 180.0);
        app.world_mut().spawn(Camera2d);
        app
    }

    fn root_router(app: &mut App, driver: ViewDriverKey, name: &str) -> RouterHandle {
        let view = RouteViewKey::new(name).unwrap();
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
        let router = app
            .add_ui_router(RouterConfig::new(routes, views, view))
            .unwrap();
        app.world_mut()
            .spawn((Node::default(), RouterHostRegistration::new(router)));
        app.update();
        router
    }

    fn navigate(app: &mut App, router: RouterHandle, path: &str) {
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new(path))
            .unwrap();
        await_success(app, router, request);
        app.update();
        app.update();
    }

    fn fixture(hidden_parent: bool) -> (App, Entity, Entity, Entity, Entity) {
        let mut app = official_app();
        let driver = app
            .register_bsn_view(
                ViewDriverKey::new("review-visibility").unwrap(),
                move |_| {
                    Ok(SceneFunction(move |_, root: &mut ResolvedScene| {
                        root.push_template(Node {
                            width: Val::Px(120.0),
                            height: Val::Px(100.0),
                            ..Node::default()
                        });
                        root.push_template(if hidden_parent {
                            Visibility::Hidden
                        } else {
                            Visibility::Inherited
                        });
                        root.push_template(ReviewParent);
                        let mut child = ResolvedScene::default();
                        child.push_template(Node {
                            width: Val::Px(80.0),
                            height: Val::Px(60.0),
                            ..Node::default()
                        });
                        child.push_template(Visibility::Visible);
                        child.push_template(ReviewTarget);
                        child.push_template(AutoFocus);
                        root.get_or_insert_related_resolved_scenes::<ChildOf>()
                            .scenes
                            .push(child);
                    }))
                },
            )
            .unwrap();
        let router = root_router(&mut app, driver, "review-visibility");
        navigate(&mut app, router, "/");
        let parent = app
            .world_mut()
            .query_filtered::<Entity, With<ReviewParent>>()
            .single(app.world())
            .unwrap();
        let target = app
            .world_mut()
            .query_filtered::<Entity, With<ReviewTarget>>()
            .single(app.world())
            .unwrap();
        assert_eq!(app.world().get::<ChildOf>(target).unwrap().parent(), parent);
        let application = app.world_mut().spawn(Node::default()).id();
        let other_driver = app
            .register_bsn_view(ViewDriverKey::new("review-other").unwrap(), |_| {
                Ok(bsn! { Node OtherActive })
            })
            .unwrap();
        let other_router = root_router(&mut app, other_driver, "review-other");
        navigate(&mut app, other_router, "/");
        let other = app
            .world_mut()
            .query_filtered::<Entity, With<OtherActive>>()
            .single(app.world())
            .unwrap();
        assert!(
            app.world()
                .resource::<RouterInputGate>()
                .allows_keyboard(other)
        );
        (app, parent, target, application, other)
    }

    #[test]
    fn native_explicit_visible_child_overrides_hidden_parent_for_gate_and_autofocus() {
        let (app, parent, target, _, _) = fixture(true);
        assert!(
            !app.world()
                .get::<InheritedVisibility>(parent)
                .unwrap()
                .get()
        );
        assert!(
            app.world()
                .get::<InheritedVisibility>(target)
                .unwrap()
                .get(),
            "official Visibility propagation makes explicit Visible override a Hidden parent"
        );
        assert!(
            app.world()
                .get::<ComputedNode>(target)
                .unwrap()
                .size()
                .cmpgt(Vec2::ZERO)
                .all()
        );
        let actual = (
            app.world()
                .resource::<RouterInputGate>()
                .allows_keyboard(target),
            app.world().resource::<InputFocus>().get(),
        );
        eprintln!(
            "explicit-visible official=true nonempty-layout=true target={target:?} gate/focus={actual:?}"
        );
        assert_eq!(actual, (true, Some(target)));
    }

    #[test]
    fn native_dynamic_hidden_parent_preserves_explicit_visible_child_focus() {
        let (mut app, parent, target, _, _) = fixture(false);
        app.world_mut().resource_mut::<FocusEvents>().lost.clear();
        *app.world_mut().get_mut::<Visibility>(parent).unwrap() = Visibility::Hidden;
        app.update();
        assert!(
            !app.world()
                .get::<InheritedVisibility>(parent)
                .unwrap()
                .get()
        );
        assert!(
            app.world()
                .get::<InheritedVisibility>(target)
                .unwrap()
                .get()
        );
        assert!(
            app.world()
                .resource::<RouterInputGate>()
                .allows_keyboard(target)
        );
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(target));
        assert!(app.world().resource::<FocusEvents>().lost.is_empty());
    }

    fn hide_after_success(hide_parent: bool, preserve: Option<bool>, visibility: bool) {
        let (mut app, parent, target, application, other) = fixture(false);
        assert!(
            app.world()
                .resource::<RouterInputGate>()
                .allows_keyboard(target)
        );
        assert_eq!(
            app.world().resource::<InputFocus>().get(),
            Some(target),
            "actual AutoFocus was published before dynamic hiding"
        );
        let retained = preserve.map(|other_active| if other_active { other } else { application });
        if let Some(retained) = retained {
            app.world_mut()
                .resource_mut::<InputFocus>()
                .set(retained, FocusCause::Pressed);
            app.update();
            assert_eq!(app.world().resource::<InputFocus>().get(), Some(retained));
        }
        app.world_mut().resource_mut::<FocusEvents>().lost.clear();
        app.world_mut().resource_mut::<FocusEvents>().gained.clear();
        if visibility {
            *app.world_mut().get_mut::<Visibility>(target).unwrap() = Visibility::Hidden;
        } else {
            app.world_mut()
                .get_mut::<Node>(if hide_parent { parent } else { target })
                .unwrap()
                .display = bevy_ui::Display::None;
        }
        app.update();
        app.update();
        if visibility {
            assert!(
                !app.world()
                    .get::<InheritedVisibility>(target)
                    .unwrap()
                    .get()
            );
        } else {
            assert_eq!(
                app.world().get::<ComputedNode>(target).unwrap().size(),
                Vec2::ZERO,
                "official layout excludes the hidden node's subtree"
            );
        }
        assert!(
            !app.world()
                .resource::<RouterInputGate>()
                .allows_keyboard(target)
        );
        assert!(
            app.world()
                .resource::<RouterInputGate>()
                .allows_keyboard(other)
        );
        eprintln!(
            "dynamic-hide parent={hide_parent} retained={retained:?} target={target:?} gate=false focus={:?}",
            app.world().resource::<InputFocus>().get()
        );
        assert_eq!(app.world().resource::<InputFocus>().get(), retained);
        let events = app.world().resource::<FocusEvents>();
        assert_eq!(
            events.lost,
            if retained.is_none() {
                vec![target]
            } else {
                vec![]
            }
        );
        assert!(
            events.gained.is_empty(),
            "hiding must not synthesize another focus request"
        );
    }

    #[test]
    fn native_display_none_target_revokes_already_published_autofocus() {
        hide_after_success(false, None, false);
    }

    #[test]
    fn native_display_none_ancestor_revokes_already_published_autofocus() {
        hide_after_success(true, None, false);
    }

    #[test]
    fn native_visibility_hidden_revokes_focus_with_official_lost_event() {
        hide_after_success(false, None, true);
    }

    #[test]
    fn native_dynamic_hide_preserves_application_and_other_active_focus() {
        for parent in [false, true] {
            for other_active in [false, true] {
                hide_after_success(parent, Some(other_active), false);
            }
        }
    }

    #[test]
    fn native_dynamic_hide_preserves_queued_application_focus_cause() {
        let (mut app, _, target, application, _) = fixture(false);
        app.world_mut().resource_mut::<FocusEvents>().lost.clear();
        app.world_mut().resource_mut::<FocusEvents>().gained.clear();
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(application, FocusCause::Navigated);
        app.world_mut().get_mut::<Node>(target).unwrap().display = bevy_ui::Display::None;
        app.update();
        assert_eq!(
            app.world().resource::<InputFocus>().get(),
            Some(application)
        );
        let events = app.world().resource::<FocusEvents>();
        assert_eq!(events.lost, vec![target]);
        assert_eq!(events.gained, vec![(application, FocusCause::Navigated)]);
    }

    #[test]
    fn official_clipping_component_is_added_and_removed_for_display_none() {
        let mut app = official_app();
        let parent = app.world_mut().spawn(Node::default()).id();
        let child = app
            .world_mut()
            .spawn((Node::default(), ChildOf(parent)))
            .id();
        app.update();
        assert!(app.world().get::<bevy_ui::CalculatedClip>(child).is_none());
        app.world_mut().get_mut::<Node>(parent).unwrap().display = bevy_ui::Display::None;
        app.update();
        assert_eq!(
            app.world()
                .get::<bevy_ui::CalculatedClip>(child)
                .unwrap()
                .clip,
            bevy_internal::math::Rect::default()
        );
        app.world_mut().get_mut::<Node>(parent).unwrap().display = bevy_ui::Display::Flex;
        app.update();
        assert!(app.world().get::<bevy_ui::CalculatedClip>(child).is_none());
    }

    #[test]
    fn native_child_frame_below_display_none_outlet_has_no_gate_or_autofocus() {
        nested_outlet_journey(true, false);
    }

    #[test]
    fn native_visible_outlet_control_reaches_child_autofocus() {
        nested_outlet_journey(false, false);
    }

    #[test]
    fn native_initial_hidden_outlet_completes_direct_nested_navigation() {
        nested_outlet_journey(true, true);
    }

    fn nested_outlet_journey(hidden: bool, initially_hidden: bool) {
        let mut app = official_app();
        let layout = app
            .register_bsn_view(ViewDriverKey::new("hidden-outlet-layout").unwrap(), move |_| {
                let display = if initially_hidden { bevy_ui::Display::None } else { bevy_ui::Display::Flex };
                Ok(
                    bsn! { Node { width: Val::Px(120.0), height: Val::Px(100.0) } Children [
                        (Node { width: Val::Px(120.0), height: Val::Px(100.0), display: display } DefaultOutlet ReviewParent)
                    ] },
                )
            })
            .unwrap();
        let leaf = app.register_bsn_view(ViewDriverKey::new("hidden-outlet-leaf").unwrap(), |_| {
            Ok(bsn! { Node { width: Val::Px(80.0), height: Val::Px(60.0) } ReviewTarget AutoFocus })
        }).unwrap();
        let layout_key = RouteViewKey::new("layout").unwrap();
        let leaf_key = RouteViewKey::new("leaf").unwrap();
        let mut routes = RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/").unwrap(),
                RouteNodeOptions::new().with_view(layout_key.clone()),
            )
            .unwrap();
        routes
            .route_with_options(
                routes.root_id().unwrap(),
                RoutePattern::path("child").unwrap(),
                RouteNodeOptions::new().with_view(leaf_key.clone()),
            )
            .unwrap();
        let mut views = RouteViewRegistry::new();
        views
            .insert(layout_key.clone(), RouteViewDescriptor::new(layout))
            .unwrap();
        views
            .insert(leaf_key, RouteViewDescriptor::new(leaf))
            .unwrap();
        let router = app
            .add_ui_router(RouterConfig::new(
                routes.build().unwrap(),
                views,
                layout_key,
            ))
            .unwrap();
        app.world_mut()
            .spawn((Node::default(), RouterHostRegistration::new(router)));
        app.update();
        if !initially_hidden {
            navigate(&mut app, router, "/");
            let declared_outlet = app
                .world_mut()
                .query_filtered::<Entity, With<ReviewParent>>()
                .single(app.world())
                .unwrap();
            if hidden {
                app.world_mut()
                    .get_mut::<Node>(declared_outlet)
                    .unwrap()
                    .display = bevy_ui::Display::None;
            }
        }
        let focus_before_child = app.world().resource::<InputFocus>().get();
        navigate(&mut app, router, "/child");
        let outlet = app
            .world_mut()
            .query_filtered::<Entity, With<ReviewParent>>()
            .single(app.world())
            .unwrap();
        let target = app
            .world_mut()
            .query_filtered::<Entity, With<ReviewTarget>>()
            .single(app.world())
            .unwrap();
        let mut ancestor = target;
        let mut reaches_outlet = false;
        while let Some(parent) = app.world().get::<ChildOf>(ancestor) {
            ancestor = parent.parent();
            if ancestor == outlet {
                reaches_outlet = true;
                break;
            }
        }
        assert!(
            reaches_outlet,
            "real child-frame mount must descend from the declared hidden outlet"
        );
        let size = app.world().get::<ComputedNode>(target).unwrap().size();
        assert_eq!(size == Vec2::ZERO, hidden);
        let actual = (
            app.world()
                .resource::<RouterInputGate>()
                .allows_keyboard(target),
            app.world().resource::<InputFocus>().get(),
        );
        eprintln!(
            "outlet={outlet:?} hidden={hidden} descendant={target:?} official-layout={size:?} gate/focus={actual:?}"
        );
        assert_eq!(
            actual,
            if hidden {
                (false, focus_before_child)
            } else {
                (true, Some(target))
            }
        );
        if !hidden {
            app.world_mut().resource_mut::<FocusEvents>().lost.clear();
            app.world_mut().get_mut::<Node>(outlet).unwrap().display = bevy_ui::Display::None;
            app.update();
            app.update();
            assert_eq!(
                app.world().get::<ComputedNode>(target).unwrap().size(),
                Vec2::ZERO
            );
            assert!(
                !app.world()
                    .resource::<RouterInputGate>()
                    .allows_keyboard(target)
            );
            assert_eq!(app.world().resource::<InputFocus>().get(), None);
            assert_eq!(app.world().resource::<FocusEvents>().lost, vec![target]);
            assert!(app.world().get::<bevy_ui::CalculatedClip>(target).is_some());
            app.world_mut().get_mut::<Node>(outlet).unwrap().display = bevy_ui::Display::Flex;
            app.update();
            app.update();
            assert!(app.world().get::<bevy_ui::CalculatedClip>(target).is_none());
            assert!(
                app.world()
                    .resource::<RouterInputGate>()
                    .allows_keyboard(target)
            );
            assert_eq!(
                app.world().resource::<InputFocus>().get(),
                None,
                "restoring display must not replay the consumed AutoFocus intent"
            );
        }
    }
}

mod inactive_native_journeys {
    use super::*;
    use bevy_app::PreUpdate;
    use bevy_ecs::{
        schedule::IntoScheduleConfigs,
        template::{Template, TemplateContext},
    };
    use bevy_scene::{ResolvedScene, SceneFunction};
    use bevy_ui_router::ecs::RouterInputSet;

    #[derive(Component, Clone, Default)]
    struct InactiveTarget;

    #[derive(Clone)]
    struct PendingRoot;
    impl Template for PendingRoot {
        type Output = ();
        fn build_template(
            &self,
            context: &mut TemplateContext<'_, '_>,
        ) -> bevy_ecs::error::Result<()> {
            context
                .entity
                .insert((target_node(), InactiveTarget, DefaultOutlet));
            Ok(())
        }
        fn clone_template(&self) -> Self {
            self.clone()
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum CommandKind {
        Keyboard,
        Gamepad,
    }

    #[derive(Message, Clone, Copy)]
    struct TargetCommand {
        target: Entity,
        kind: CommandKind,
    }

    #[derive(Resource, Default)]
    struct CommandDeliveries(Vec<(Entity, CommandKind)>);

    // This is an application consumer of the documented target-addressed
    // keyboard/gamepad contract, scheduled at the public input boundary.
    fn consume_commands(
        mut commands: MessageReader<TargetCommand>,
        gate: Res<RouterInputGate>,
        mut delivered: ResMut<CommandDeliveries>,
    ) {
        for command in commands.read() {
            let allowed = match command.kind {
                CommandKind::Keyboard => gate.allows_keyboard(command.target),
                CommandKind::Gamepad => gate.allows_gamepad(command.target),
            };
            if allowed {
                delivered.0.push((command.target, command.kind));
            }
        }
    }

    fn app() -> (App, Entity, Entity) {
        let mut app = App::new();
        app.add_plugins(DefaultPlugins);
        if !app.is_plugin_added::<AssetPlugin>() {
            app.add_plugins(AssetPlugin::default());
        }
        if !app.is_plugin_added::<ScenePlugin>() {
            app.add_plugins(ScenePlugin);
        }
        if !app.is_plugin_added::<InputFocusPlugin>() {
            app.add_plugins(InputFocusPlugin);
        }
        observe_pointer_delivery(&mut app);
        app.add_message::<TargetCommand>()
            .init_resource::<CommandDeliveries>();
        app.add_systems(PreUpdate, consume_commands.after(RouterInputSet::Ready));
        let window = app
            .world_mut()
            .query_filtered::<Entity, With<PrimaryWindow>>()
            .single(app.world())
            .unwrap();
        app.world_mut()
            .get_mut::<Window>(window)
            .unwrap()
            .resolution
            .set(320.0, 180.0);
        app.world_mut().spawn(Camera2d);
        let control = app
            .world_mut()
            .spawn(Node {
                position_type: PositionType::Absolute,
                left: Val::Px(140.0),
                top: Val::Px(60.0),
                width: Val::Px(48.0),
                height: Val::Px(32.0),
                ..Node::default()
            })
            .id();
        (app, window, control)
    }

    fn target_node() -> Node {
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(20.0),
            top: Val::Px(20.0),
            width: Val::Px(48.0),
            height: Val::Px(32.0),
            ..Node::default()
        }
    }

    fn install(app: &mut App, driver: ViewDriverKey, depth: usize) -> RouterHandle {
        let page = RouteViewKey::new("inactive-page").unwrap();
        let plain = RouteViewKey::new("inactive-plain").unwrap();
        let plain_driver = app
            .register_bsn_view(ViewDriverKey::new("inactive-plain").unwrap(), |_| {
                Ok(bsn! { Node DefaultOutlet })
            })
            .unwrap();
        let mut routes = RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/").unwrap(),
                RouteNodeOptions::new().with_view(page.clone()),
            )
            .unwrap();
        let mut parent = routes.root_id().unwrap();
        for _ in 0..depth {
            parent = routes
                .route_with_options(
                    parent,
                    RoutePattern::path("child").unwrap(),
                    RouteNodeOptions::new().with_view(plain.clone()),
                )
                .unwrap();
        }
        let mut views = RouteViewRegistry::new();
        views
            .insert(page, RouteViewDescriptor::new(driver))
            .unwrap();
        views
            .insert(plain.clone(), RouteViewDescriptor::new(plain_driver))
            .unwrap();
        let router = app
            .add_ui_router(RouterConfig::new(routes.build().unwrap(), views, plain))
            .unwrap();
        app.world_mut().spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..Node::default()
            },
            RouterHostRegistration::new(router),
        ));
        app.update();
        router
    }

    fn pointers(app: &mut App, window: Entity, control: Entity) -> [(PointerId, Location); 2] {
        let target = Location {
            target: NormalizedRenderTarget::Window(
                WindowRef::Primary.normalize(Some(window)).unwrap(),
            ),
            // The exact center authored by target_node. Publication's positive
            // control below proves this same coordinate really hits the target.
            position: Vec2::new(44.0, 36.0),
        };
        let pointers = [
            (PointerId::Touch(0x991), target),
            (
                PointerId::Touch(0x992),
                location_at(app.world(), window, control),
            ),
        ];
        for (pointer, location) in &pointers {
            app.world_mut().spawn((
                *pointer,
                PointerLocation::new(location.clone()),
                PointerPress::default(),
                PointerInteraction::default(),
            ));
        }
        pointers
    }

    fn commands(app: &mut App, target: Entity) {
        for kind in [CommandKind::Keyboard, CommandKind::Gamepad] {
            app.world_mut()
                .write_message(TargetCommand { target, kind });
        }
    }

    fn action(app: &mut App, pointers: &[(PointerId, Location); 2], action: PointerAction) {
        send_inputs(
            app,
            pointers.iter().map(|(pointer, location)| {
                PointerInput::new(*pointer, location.clone(), action.clone())
            }),
        );
    }

    fn assert_running(app: &App, router: RouterHandle, request: NavigationRequestId) {
        assert!(
            matches!(app.world().resource::<RouterSnapshots>().request(router, request),
            RequestLookup::Found(request) if matches!(request.status(), RequestStatus::Running(_))),
            "the real nested Scene must still be pending at this input boundary"
        );
    }

    #[test]
    fn native_pending_pointer_and_ready_commands_wait_for_real_publication() {
        let (mut app, window, control) = app();
        let driver = app
            .register_bsn_view(ViewDriverKey::new("inactive-pending").unwrap(), |_| {
                Ok(SceneFunction(|_, root: &mut ResolvedScene| {
                    root.push_bundle_template(PendingRoot);
                }))
            })
            .unwrap();
        // Each genuine child frame requires its own ordinary update. No test
        // pause hook or lifecycle state injection keeps the root pending.
        let depth = 8;
        let router = install(&mut app, driver, depth);
        let pointers = pointers(&mut app, window, control);
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(
                router,
                RouteLocation::new(format!("/{}", vec!["child"; depth].join("/"))),
            )
            .unwrap();
        app.update();
        let target = app
            .world_mut()
            .query_filtered::<Entity, With<InactiveTarget>>()
            .single(app.world())
            .unwrap();
        assert_running(&app, router, request);
        assert_target_revoked(app.world(), target);
        for input in [
            PointerAction::Press(PointerButton::Primary),
            PointerAction::Move { delta: Vec2::X },
            PointerAction::Release(PointerButton::Primary),
        ] {
            commands(&mut app, target);
            action(&mut app, &pointers, input);
            assert_running(&app, router, request);
            assert_target_revoked(app.world(), target);
            assert!(
                !app.world()
                    .resource::<PointerDeliveries>()
                    .0
                    .contains_key(&target)
            );
            assert!(app.world().resource::<CommandDeliveries>().0.is_empty());
        }
        let control_delivery = app.world().resource::<PointerDeliveries>().0[&control];
        assert_eq!(control_delivery.presses, 1);
        assert_eq!(control_delivery.releases, 1);
        assert_eq!(control_delivery.clicks, 1);
        await_success(&mut app, router, request);
        app.update();
        assert_eq!(
            location_at(app.world(), window, target).position,
            pointers[0].1.position
        );
        assert!(
            app.world()
                .resource::<HoverMap>()
                .get(&pointers[0].0)
                .is_some_and(|hits| hits.contains_key(&target))
        );
        commands(&mut app, target);
        action(
            &mut app,
            &pointers,
            PointerAction::Press(PointerButton::Primary),
        );
        action(
            &mut app,
            &pointers,
            PointerAction::Release(PointerButton::Primary),
        );
        let delivery = app.world().resource::<PointerDeliveries>().0[&target];
        assert_eq!(delivery.presses, 1);
        assert_eq!(delivery.releases, 1);
        assert_eq!(delivery.clicks, 1);
        assert_eq!(
            app.world().resource::<CommandDeliveries>().0,
            vec![
                (target, CommandKind::Keyboard),
                (target, CommandKind::Gamepad)
            ]
        );
    }

    #[derive(Clone)]
    struct PreserveApplicationChildThenFail(Entity);
    impl Template for PreserveApplicationChildThenFail {
        type Output = ();
        fn build_template(
            &self,
            context: &mut TemplateContext<'_, '_>,
        ) -> bevy_ecs::error::Result<()> {
            let root = context.entity.id();
            context.entity.insert((target_node(), InactiveTarget));
            context.entity.world_scope(|world| {
                world.entity_mut(self.0).insert(ChildOf(root));
            });
            Err("native retained pointer apply source".into())
        }
        fn clone_template(&self) -> Self {
            self.clone()
        }
    }

    #[test]
    fn native_failed_retained_residual_rejects_pointer_and_ready_commands() {
        let (mut app, window, control) = app();
        // A pre-existing application entity attached by the Scene forces real
        // failure retention: cleanup must preserve this foreign child.
        let protected = app
            .world_mut()
            .spawn(Node {
                width: Val::Px(7.0),
                height: Val::Px(7.0),
                ..Node::default()
            })
            .id();
        let driver = app
            .register_bsn_view(ViewDriverKey::new("inactive-failure").unwrap(), move |_| {
                Ok(SceneFunction(move |_, root: &mut ResolvedScene| {
                    root.push_bundle_template(PreserveApplicationChildThenFail(protected));
                }))
            })
            .unwrap();
        let router = install(&mut app, driver, 0);
        let pointers = pointers(&mut app, window, control);
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        for _ in 0..32 {
            app.update();
            if matches!(app.world().resource::<RouterSnapshots>().request(router, request),
                RequestLookup::Found(request) if matches!(request.status(), RequestStatus::Failed(_)))
            {
                break;
            }
        }
        assert!(
            matches!(app.world().resource::<RouterSnapshots>().request(router, request),
            RequestLookup::Found(request) if matches!(request.status(), RequestStatus::Failed(error)
                if error.kind() == NavigationFailureKind::Apply && error.message().contains("native retained pointer apply source")))
        );
        assert_eq!(
            app.world()
                .resource::<RouterSnapshots>()
                .get(router)
                .unwrap()
                .active()
                .kind(),
            ActivePresentationKind::Error
        );
        let target = app.world().get::<ChildOf>(protected).unwrap().parent();
        assert!(
            app.world().get::<InactiveTarget>(target).is_some(),
            "failed native root must remain physically present around protected app content"
        );
        assert_eq!(
            app.world().get::<Node>(target).unwrap().width,
            Val::Px(48.0)
        );
        assert_target_revoked(app.world(), target);
        for input in [
            PointerAction::Press(PointerButton::Primary),
            PointerAction::Move { delta: Vec2::X },
            PointerAction::Release(PointerButton::Primary),
        ] {
            commands(&mut app, target);
            commands(&mut app, protected);
            action(&mut app, &pointers, input);
            assert!(app.world().get_entity(target).is_ok());
            assert_eq!(
                app.world().get::<ChildOf>(protected).unwrap().parent(),
                target
            );
            assert_target_revoked(app.world(), target);
            assert_target_revoked(app.world(), protected);
            assert!(
                !app.world()
                    .resource::<PointerDeliveries>()
                    .0
                    .contains_key(&target)
            );
            assert!(
                !app.world()
                    .resource::<PointerDeliveries>()
                    .0
                    .contains_key(&protected)
            );
            assert!(app.world().resource::<CommandDeliveries>().0.is_empty());
        }
        let delivery = app.world().resource::<PointerDeliveries>().0[&control];
        assert_eq!(delivery.presses, 1);
        assert_eq!(delivery.releases, 1);
        assert_eq!(delivery.clicks, 1);
    }

    #[test]
    fn ready_application_consumer_tracks_dynamic_content_membership_and_visibility() {
        let (mut app, _, _) = app();
        let driver = app
            .register_bsn_view(ViewDriverKey::new("ready-dynamic").unwrap(), |_| {
                Ok(SceneFunction(|_, root: &mut ResolvedScene| {
                    root.push_template(target_node());
                    root.push_template(InactiveTarget);
                }))
            })
            .unwrap();
        let router = install(&mut app, driver, 0);
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        await_success(&mut app, router, request);
        let root = app
            .world_mut()
            .query_filtered::<Entity, With<InactiveTarget>>()
            .single(app.world())
            .unwrap();
        // All topology changes happen before the next public Refresh/Ready.
        let child = app
            .world_mut()
            .spawn((
                Node {
                    width: Val::Px(10.0),
                    height: Val::Px(10.0),
                    ..Node::default()
                },
                ChildOf(root),
            ))
            .id();
        let consume = |app: &mut App, expected_child: bool| {
            app.world_mut()
                .resource_mut::<CommandDeliveries>()
                .0
                .clear();
            commands(app, root);
            commands(app, child);
            app.update();
            let mut expected = vec![(root, CommandKind::Keyboard), (root, CommandKind::Gamepad)];
            if expected_child {
                expected.extend([
                    (child, CommandKind::Keyboard),
                    (child, CommandKind::Gamepad),
                ]);
            }
            assert_eq!(app.world().resource::<CommandDeliveries>().0, expected);
        };
        consume(&mut app, true);
        app.world_mut().entity_mut(child).remove::<ChildOf>();
        consume(&mut app, false);
        app.world_mut().entity_mut(child).insert(ChildOf(root));
        consume(&mut app, true);
        app.world_mut().get_mut::<Node>(child).unwrap().display = bevy_ui::Display::None;
        consume(&mut app, false);
        app.world_mut().get_mut::<Node>(child).unwrap().display = bevy_ui::Display::Flex;
        consume(&mut app, true);
        app.world_mut().entity_mut(child).despawn();
        consume(&mut app, false);
    }
}
