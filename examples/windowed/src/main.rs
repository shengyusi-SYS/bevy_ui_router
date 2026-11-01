//! A real window, renderer and device-input integration using only public APIs.
#[cfg(all(debug_assertions, feature = "ui-debug"))]
mod ui_debug;
use bevy_input_focus::{FocusCause, InputFocus, InputFocusPlugin};
use bevy_internal::window::PrimaryWindow;
use bevy_internal::{input::mouse::MouseScrollUnit, picking::Pickable, prelude::*};
use bevy_scene::{Scene, bsn};
use bevy_ui_router::prelude::*;

#[derive(Component, Clone, Copy, Debug)]
enum Action {
    Overview,
    Settings,
    Delayed,
    Outside,
    Replace,
    Back,
    Fail,
}

#[derive(Component, Clone, Default)]
struct Layout;
#[derive(Component, Clone, Default)]
struct Counter(u32);
#[derive(Component, Clone, Default)]
struct PageButton(u32);
#[derive(Component, Clone, Default)]
struct DelayedPage {
    loaded: bool,
}
#[derive(Clone, Copy, Default)]
enum RestoreAction {
    #[default]
    Load,
    Ready,
    Cancel,
}
#[derive(Component, Clone, Default)]
struct RestoreControl(RestoreAction);
#[derive(Component)]
struct Status;

#[derive(Component)]
struct WindowRouter(RouterHandle);

#[derive(Resource)]
struct Demo {
    router: RouterHandle,
    last_request: Option<NavigationRequestId>,
    result: String,
}

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "Bevy UI Router - retained pages and history scroll".into(),
            resolution: (960, 640).into(),
            focused: !cfg!(all(debug_assertions, feature = "ui-debug")),
            ..default()
        }),
        ..default()
    }))
    // Do not install InputDispatchPlugin or TabNavigationPlugin: this example
    // explicitly gates its target-addressed keyboard handler below.
    .add_plugins((InputFocusPlugin, DesktopBackInputPlugin));
    #[cfg(all(debug_assertions, feature = "ui-debug"))]
    app.add_plugins(ui_debug::UiDebugPlugin);

    let root_view = layout(|_| {
        Ok(bsn! {
            Node { width: Val::Percent(100.0), height: Val::Percent(100.0) } DefaultOutlet
        })
    });
    let workspace = page("workspace", |_| {
        Ok(bsn! {
            Node {
                width: Val::Percent(100.0), height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column, row_gap: Val::Px(10.0),
                padding: UiRect::all(Val::Px(16.0)),
            }
            BackgroundColor(Color::srgb(0.08, 0.12, 0.19)) Layout Name::new("layout-a")
            Children [
                (Text({"Shared layout / nested outlet".to_owned()}) Pickable { should_block_lower: false, is_hoverable: false }),
                (Button Node { width: Val::Percent(100.0), height: Val::Px(40.0), padding: UiRect::all(Val::Px(8.0)) }
                    BackgroundColor(Color::srgb(0.18, 0.27, 0.37)) Counter Name::new("layout-counter")
                    Children [(Text({"Layout counter: 0 (click +1)".to_owned()}) Pickable { should_block_lower: false, is_hoverable: false })]),
                (Node { width: Val::Percent(100.0), flex_grow: 1.0 } DefaultOutlet)
            ]
        })
    });
    let mut workspace_pages = Vec::new();
    for (name, color) in [
        ("overview", Color::srgb(0.12, 0.35, 0.32)),
        ("settings", Color::srgb(0.22, 0.22, 0.45)),
    ] {
        let view = page(name, move |_| {
            Ok(bsn! {
                Node {
                    width: Val::Percent(100.0), height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column, row_gap: Val::Px(10.0),
                    padding: UiRect::all(Val::Px(12.0)),
                }
                BackgroundColor(color) Name::new(name)
                Children [
                    (Text({name.to_owned()}) TextFont { font_size: FontSize::Px(24.0) } Pickable { should_block_lower: false, is_hoverable: false }),
                    (Text({"Click / Space adds one. Scroll, switch pages, then Back.".to_owned()}) Pickable { should_block_lower: false, is_hoverable: false }),
                    (Button Node { width: Val::Px(440.0), height: Val::Px(44.0), flex_shrink: 0.0, padding: UiRect::all(Val::Px(8.0)) }
                        BackgroundColor(Color::srgb(0.12, 0.18, 0.25)) PageButton Name::new(format!("{name}-count"))
                        Children [(Text({"Page count: 0 (click / Space)".to_owned()}) Pickable { should_block_lower: false, is_hoverable: false })]),
                    (Node {
                        width: Val::Percent(100.0), height: Val::Px(120.0), flex_shrink: 0.0,
                        overflow: Overflow::scroll_y(),
                    }
                    Name::new(format!("{name}-scroll")) RouteScroll::new("list")
                    BackgroundColor(Color::srgb(0.06, 0.10, 0.16))
                    Children [(Node {
                        width: Val::Percent(100.0), min_height: Val::Px(720.0), flex_shrink: 0.0,
                        padding: UiRect::all(Val::Px(12.0)),
                        flex_direction: FlexDirection::Column,
                    } Pickable::IGNORE Children [
                        (Text({list_text(name)}) Pickable::IGNORE)
                    ])])
                ]
            })
        });
        workspace_pages.push(view.keep_alive());
    }
    let delayed = page("delayed", |_| {
        Ok(bsn! {
            Node {
                width: Val::Percent(100.0), height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column, row_gap: Val::Px(8.0),
                padding: UiRect::all(Val::Px(12.0)),
            }
            BackgroundColor(Color::srgb(0.25, 0.27, 0.14)) Name::new("delayed") DelayedPage
            Children [
                (Text({"Delayed list / explicit restore".to_owned()}) TextFont { font_size: FontSize::Px(24.0) } Pickable::IGNORE),
                (Button Node { width: Val::Px(440.0), height: Val::Px(36.0), flex_shrink: 0.0, padding: UiRect::all(Val::Px(6.0)) }
                    BackgroundColor(Color::srgb(0.12, 0.18, 0.25)) PageButton Name::new("delayed-count")
                    Children [(Text({"Page count: 0 (click / Space)".to_owned()}) Pickable::IGNORE)]),
                (Node { column_gap: Val::Px(8.0) } Children [
                    (Button RestoreControl({RestoreAction::Load}) Node { padding: UiRect::all(Val::Px(6.0)) } BackgroundColor(Color::srgb(0.17, 0.23, 0.32))
                        Children [(Text({"Load [L]".to_owned()}) Pickable::IGNORE)]),
                    (Button RestoreControl({RestoreAction::Ready}) Node { padding: UiRect::all(Val::Px(6.0)) } BackgroundColor(Color::srgb(0.17, 0.23, 0.32))
                        Children [(Text({"Ready [G]".to_owned()}) Pickable::IGNORE)]),
                    (Button RestoreControl({RestoreAction::Cancel}) Node { padding: UiRect::all(Val::Px(6.0)) } BackgroundColor(Color::srgb(0.17, 0.23, 0.32))
                        Children [(Text({"Cancel [C]".to_owned()}) Pickable::IGNORE)])
                ])
            ]
        })
    });
    workspace_pages.push(delayed);
    let outside = page("outside", |_| {
        Ok(bsn! {
            Node { width: Val::Percent(100.0), height: Val::Percent(100.0), padding: UiRect::all(Val::Px(24.0)) }
            Name::new("outside") BackgroundColor(Color::srgb(0.18, 0.15, 0.27))
            Children [(Text({"Outside layout A. Return to a new child: A keeps its own state.".to_owned()}) Pickable::IGNORE)]
        })
    });
    let broken = page("broken", |_| {
        Err::<Box<dyn Scene>, _>(BsnBuildError::new("Intentional example build failure"))
    });
    workspace_pages.push(broken);
    let fallback = |input: &RouteViewInput| {
        let message = format!(
            "Build failed; fallback is visible.\n{:?}\n\nUse Overview or Settings to recover.",
            input.failure()
        );
        Ok(bsn! {
            Node { width: Val::Percent(100.0), height: Val::Percent(100.0), padding: UiRect::all(Val::Px(24.0)) }
            Name::new("fallback") BackgroundColor(Color::srgb(0.36, 0.12, 0.15))
            Children [(Text(message) Pickable { should_block_lower: false, is_hoverable: false })]
        })
    };
    let router = app
        .add_bsn_router(
            BsnRouter::new(
                root_view.children([workspace.keep_alive().children(workspace_pages), outside]),
            )
            .fallback(fallback)
            .keep_alive_capacity(3)
            .initial("/workspace/overview"),
        )
        .unwrap();
    app.insert_resource(Demo {
        router,
        last_request: None,
        result: "Initial page configured".into(),
    })
    .add_systems(Startup, setup)
    .add_systems(
        PreUpdate,
        (keyboard, restore_keyboard).after(RouterInputSet::Ready),
    )
    .add_systems(PreUpdate, platform_back.after(PlatformBackInputSet))
    .add_systems(
        Update,
        update_status.after(RouterScheduleSet::PublishSnapshots),
    )
    .add_observer(click)
    .add_observer(restore_click)
    .add_observer(press)
    .add_observer(release)
    .add_observer(scroll)
    .run();
}

fn setup(mut commands: Commands, demo: Res<Demo>, window: Single<Entity, With<PrimaryWindow>>) {
    commands.entity(*window).insert(WindowRouter(demo.router));
    commands.spawn(Camera2d);
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(16.0)),
                row_gap: Val::Px(10.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.04, 0.06, 0.1)),
        ))
        .with_children(|shell| {
            shell.spawn((
                Text::new("Bevy UI Router / independent retention and scroll"),
                TextFont {
                    font_size: FontSize::Px(28.0),
                    ..default()
                },
            ));
            shell
                .spawn(Node {
                    column_gap: Val::Px(8.0),
                    row_gap: Val::Px(8.0),
                    flex_wrap: FlexWrap::Wrap,
                    ..default()
                })
                .with_children(|bar| {
                    for (label, action) in [
                        ("Overview [1]", Action::Overview),
                        ("Settings [2]", Action::Settings),
                        ("Delayed [3]", Action::Delayed),
                        ("Outside [4]", Action::Outside),
                        ("Replace [R]", Action::Replace),
                        ("Back [B / mouse side]", Action::Back),
                        ("Fail [E]", Action::Fail),
                    ] {
                        bar.spawn((
                            Button,
                            action,
                            Node {
                                padding: UiRect::all(Val::Px(8.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.17, 0.23, 0.32)),
                        ))
                        .with_child((
                            Text::new(label),
                            TextFont {
                                font_size: FontSize::Px(16.0),
                                ..default()
                            },
                            Pickable::IGNORE,
                        ));
                    }
                });
            shell.spawn((
                Text::new(""),
                TextFont {
                    font_size: FontSize::Px(16.0),
                    ..default()
                },
                Status,
                Name::new("status"),
            ));
            shell.spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_grow: 1.0,
                    ..default()
                },
                RouterHostRegistration::new(demo.router),
            ));
        });
}

fn navigate(action: Action, demo: &mut Demo, navigation: &mut RouterNavigation) {
    let result = match action {
        Action::Overview => navigation.push(demo.router, RouteLocation::new("/workspace/overview")),
        Action::Settings => navigation.push(demo.router, RouteLocation::new("/workspace/settings")),
        Action::Delayed => navigation.push(demo.router, RouteLocation::new("/workspace/delayed")),
        Action::Outside => navigation.push(demo.router, RouteLocation::new("/outside")),
        Action::Replace => {
            navigation.replace(demo.router, RouteLocation::new("/workspace/overview"))
        }
        Action::Back => navigation.back(demo.router),
        Action::Fail => navigation.push(demo.router, RouteLocation::new("/workspace/broken")),
    };
    match result {
        Ok(request) => {
            demo.last_request = Some(request);
            demo.result = format!("{action:?} pending");
        }
        Err(error) => demo.result = format!("{action:?}: {error:?}"),
    }
}

fn platform_back(
    mut requests: MessageReader<PlatformBackRequested>,
    windows: Query<&WindowRouter>,
    mut demo: ResMut<Demo>,
    mut navigation: ResMut<RouterNavigation>,
) {
    // This is the single application return-policy handler. Local UI can handle
    // a request here before deciding to navigate.
    for request in requests.read() {
        if windows
            .get(request.window)
            .is_ok_and(|target| target.0 == demo.router)
        {
            navigate(Action::Back, &mut demo, &mut navigation);
        }
    }
}

fn click(
    event: On<Pointer<Click>>,
    actions: Query<&Action>,
    mut counters: Query<&mut Counter>,
    mut pages: Query<&mut PageButton>,
    gate: Res<RouterInputGate>,
    mut focus: ResMut<InputFocus>,
    mut demo: ResMut<Demo>,
    mut navigation: ResMut<RouterNavigation>,
) {
    let target = event.event_target();
    if let Ok(action) = actions.get(target) {
        navigate(*action, &mut demo, &mut navigation);
    } else if gate.allows_pointer(target) {
        if let Ok(mut counter) = counters.get_mut(target) {
            counter.0 += 1;
            println!("Layout counter {target:?}: {}", counter.0);
        }
        if let Ok(mut page) = pages.get_mut(target) {
            page.0 += 1;
            focus.set(target, FocusCause::Pressed);
            println!("Page click {target:?}: {}", page.0);
        }
    }
}

fn keyboard(
    keys: Res<ButtonInput<KeyCode>>,
    gate: Res<RouterInputGate>,
    focus: Res<InputFocus>,
    mut pages: Query<&mut PageButton>,
    mut demo: ResMut<Demo>,
    mut navigation: ResMut<RouterNavigation>,
) {
    // These global shortcuts belong to the application shell, not a routed page.
    for (key, action) in [
        (KeyCode::Digit1, Action::Overview),
        (KeyCode::Digit2, Action::Settings),
        (KeyCode::Digit3, Action::Delayed),
        (KeyCode::Digit4, Action::Outside),
        (KeyCode::KeyR, Action::Replace),
        (KeyCode::KeyB, Action::Back),
        (KeyCode::KeyE, Action::Fail),
    ] {
        if keys.just_pressed(key) {
            navigate(action, &mut demo, &mut navigation);
        }
    }
    if keys.just_pressed(KeyCode::Space)
        && let Some(target) = focus.get()
        && gate.allows_keyboard(target)
        && let Ok(mut page) = pages.get_mut(target)
    {
        page.0 += 1;
        println!("Page keyboard {target:?}: {}", page.0);
    }
}

fn restore_click(
    event: On<Pointer<Click>>,
    actions: Query<&RestoreControl>,
    gate: Res<RouterInputGate>,
    activity: RouteActivity,
    restorations: RouteScrollRestorations,
    mut pages: Query<(Entity, &mut DelayedPage)>,
    mut commands: Commands,
) {
    let target = event.event_target();
    if !gate.allows_pointer(target) {
        return;
    }
    if let Ok(action) = actions.get(target)
        && let Some((scope, mut page)) = pages
            .iter_mut()
            .find(|(scope, _)| activity.is_active(*scope))
    {
        restore(action.0, scope, &mut page, &restorations, &mut commands);
    }
}

fn restore_keyboard(
    keys: Res<ButtonInput<KeyCode>>,
    gate: Res<RouterInputGate>,
    activity: RouteActivity,
    restorations: RouteScrollRestorations,
    mut pages: Query<(Entity, &mut DelayedPage)>,
    mut commands: Commands,
) {
    let Some((scope, mut page)) = pages
        .iter_mut()
        .find(|(scope, _)| activity.is_active(*scope))
    else {
        return;
    };
    if !gate.allows_keyboard(scope) {
        return;
    }
    for (key, action) in [
        (KeyCode::KeyL, RestoreAction::Load),
        (KeyCode::KeyG, RestoreAction::Ready),
        (KeyCode::KeyC, RestoreAction::Cancel),
    ] {
        if keys.just_pressed(key) {
            restore(action, scope, &mut page, &restorations, &mut commands);
        }
    }
}

fn restore(
    action: RestoreAction,
    scope: Entity,
    page: &mut DelayedPage,
    restorations: &RouteScrollRestorations,
    commands: &mut Commands,
) {
    match action {
        RestoreAction::Load if !page.loaded => {
            page.loaded = true;
            // Application-owned readiness: create ordinary UI before UiSystems::Prepare.
            // Router can expose the visit's restore intent before this container exists.
            commands.entity(scope).with_children(|parent| {
                parent
                    .spawn((
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Px(120.0),
                            flex_shrink: 0.0,
                            overflow: Overflow::scroll_y(),
                            ..default()
                        },
                        Name::new("delayed-scroll"),
                        RouteScroll::new("list")
                            .with_restore_policy(RouteScrollRestorePolicy::Explicit),
                        BackgroundColor(Color::srgb(0.06, 0.10, 0.16)),
                    ))
                    .with_children(|viewport| {
                        viewport
                            .spawn((
                                Node {
                                    width: Val::Percent(100.0),
                                    min_height: Val::Px(720.0),
                                    flex_shrink: 0.0,
                                    padding: UiRect::all(Val::Px(12.0)),
                                    flex_direction: FlexDirection::Column,
                                    ..default()
                                },
                                Pickable::IGNORE,
                            ))
                            .with_children(|content| {
                                content.spawn((Text::new(list_text("delayed")), Pickable::IGNORE));
                            });
                    });
            });
        }
        RestoreAction::Load => {}
        RestoreAction::Ready | RestoreAction::Cancel => {
            if let Some(pending) = restorations.pending(scope, "list") {
                match action {
                    RestoreAction::Ready => commands.ready_route_scroll(pending.id()),
                    RestoreAction::Cancel => commands.cancel_route_scroll(pending.id()),
                    RestoreAction::Load => unreachable!(),
                };
            }
        }
    }
}

// Numbered rows keep scroll movement visible throughout the list.
fn list_text(name: &str) -> String {
    (1..=30)
        .map(|row| format!("{name} / row {row:02}"))
        .collect::<Vec<_>>()
        .join("\n")
}

// This is application input behavior; RouteScroll only opts into history storage.
fn scroll(
    event: On<Pointer<Scroll>>,
    gate: Res<RouterInputGate>,
    restorations: RouteScrollRestorations,
    mut positions: Query<(&RouteScroll, &mut ScrollPosition)>,
    mut commands: Commands,
) {
    let target = event.event_target();
    if event.y != 0.0
        && gate.allows_pointer(target)
        && let Ok((marker, mut position)) = positions.get_mut(target)
    {
        // Even at a scroll boundary, user input supersedes a pending restore.
        if let Some(pending) = restorations.pending(target, marker.key()) {
            commands.cancel_route_scroll(pending.id());
        }
        let unit = match event.unit {
            MouseScrollUnit::Line => 32.0,
            MouseScrollUnit::Pixel => 1.0,
        };
        position.0.y = (position.0.y - event.y * unit).max(0.0);
    }
}

fn press(event: On<Pointer<Press>>, pages: Query<(), With<PageButton>>) {
    if pages.contains(event.event_target()) {
        println!("Page press {:?}", event.event_target());
    }
}

fn release(event: On<Pointer<Release>>, pages: Query<(), With<PageButton>>) {
    if pages.contains(event.event_target()) {
        println!("Page release {:?}", event.event_target());
    }
}

fn update_status(
    mut demo: ResMut<Demo>,
    snapshots: Res<RouterSnapshots>,
    focus: Res<InputFocus>,
    layouts: Query<Entity, With<Layout>>,
    counters: Query<(Entity, &Counter, &Children)>,
    pages: Query<(Entity, &PageButton, &Children)>,
    activity: RouteActivity,
    inputs: RouteInputs,
    mut texts: Query<&mut Text>,
    status: Query<Entity, With<Status>>,
    mut last_state: Local<String>,
) {
    let Some(snapshot) = snapshots.get(demo.router) else {
        return;
    };
    if let Some(request) = demo.last_request
        && let RequestLookup::Found(result) = snapshots.request(demo.router, request)
        && matches!(
            result.status(),
            RequestStatus::Succeeded | RequestStatus::Failed(_)
        )
    {
        demo.result = format!("{:?}", result.status());
        demo.last_request = None;
    }
    for (entity, counter, children) in &counters {
        if !activity.is_active(entity) {
            continue;
        }
        for child in children {
            if let Ok(mut text) = texts.get_mut(*child) {
                let path = inputs.current(*child).map(|input| input.location().path());
                text.set_if_neq(Text(format!(
                    "Layout counter: {} (click +1) / {}",
                    counter.0,
                    path.unwrap_or("inactive")
                )));
            }
        }
    }
    for (entity, page, children) in &pages {
        // Retained entities remain in ordinary application queries.
        if !activity.is_active(entity) {
            continue;
        }
        for child in children {
            if let Ok(mut text) = texts.get_mut(*child) {
                text.set_if_neq(Text(format!("Page count: {} (click / Space)", page.0)));
            }
        }
    }
    let location = snapshot.active().location().map_or_else(
        || format!("No active route ({:?})", snapshot.availability()),
        |location| {
            format!(
                "{} | search {:?} | fragment {:?}",
                location.path(),
                location.search(),
                location.fragment()
            )
        },
    );
    let state = format!(
        "{} | history {} / {:?} | focus {:?}\n{}",
        location,
        snapshot.history().entries().len(),
        snapshot.history().cursor(),
        focus.get(),
        demo.result
    );
    if *last_state != state {
        println!(
            "State: {state}; layout {:?}",
            layouts.iter().collect::<Vec<_>>()
        );
        last_state.clone_from(&state);
        if let Ok(entity) = status.single()
            && let Ok(mut text) = texts.get_mut(entity)
        {
            text.0 = state;
        }
    }
}
