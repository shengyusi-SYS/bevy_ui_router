//! A real window, renderer and device-input integration using only public APIs.
use bevy_input_focus::{FocusCause, InputFocus, InputFocusPlugin};
use bevy_internal::{picking::Pickable, prelude::*};
use bevy_scene::{Scene, bsn};
use bevy_ui_router::{
    ecs::{RouterInputSet, RouterScheduleSet},
    prelude::*,
};

#[derive(Component, Clone, Copy, Debug)]
enum Action {
    Overview,
    Settings,
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
#[derive(Component)]
struct Status;

#[derive(Resource)]
struct Demo {
    router: RouterHandle,
    initial_requested: bool,
    last_request: Option<NavigationRequestId>,
    result: String,
}

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "Bevy UI Router - MVP".into(),
            resolution: (960, 640).into(),
            ..default()
        }),
        ..default()
    }))
    // Do not install InputDispatchPlugin or TabNavigationPlugin: this example
    // explicitly gates its target-addressed keyboard handler below.
    .add_plugins(InputFocusPlugin);

    let mut views = RouteViewRegistry::new();
    let layout = register(&mut app, &mut views, "layout", |_| {
        Ok(bsn! {
            Node {
                width: Val::Percent(100.0), height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column, row_gap: Val::Px(20.0),
                padding: UiRect::all(Val::Px(24.0)),
            }
            BackgroundColor(Color::srgb(0.08, 0.12, 0.19)) Layout
            Children [
                (Text({"Shared layout / nested outlet".to_owned()}) Pickable { should_block_lower: false, is_hoverable: false }),
                (Button Node { width: Val::Px(440.0), height: Val::Px(48.0), padding: UiRect::all(Val::Px(12.0)) }
                    BackgroundColor(Color::srgb(0.18, 0.27, 0.37)) Counter
                    Children [(Text({"Layout counter: 0 (click +1)".to_owned()}) Pickable { should_block_lower: false, is_hoverable: false })]),
                (Node { width: Val::Percent(100.0), flex_grow: 1.0 } DefaultOutlet)
            ]
        })
    });
    let mut routes = RouteTree::builder()
        .root_with_options(
            RoutePattern::path("/").unwrap(),
            RouteNodeOptions::new().with_view(layout),
        )
        .unwrap();
    let root = routes.root_id().unwrap();
    for (name, color) in [
        ("overview", Color::srgb(0.12, 0.35, 0.32)),
        ("settings", Color::srgb(0.22, 0.22, 0.45)),
    ] {
        let view = register(&mut app, &mut views, name, move |_| {
            Ok(bsn! {
                Node {
                    width: Val::Percent(100.0), height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column, row_gap: Val::Px(20.0),
                    padding: UiRect::all(Val::Px(24.0)),
                }
                BackgroundColor(color)
                Children [
                    (Text({name.to_owned()}) TextFont { font_size: FontSize::Px(28.0) } Pickable { should_block_lower: false, is_hoverable: false }),
                    (Text({"Click the button to focus it; Space increments only the focused, active page.".to_owned()}) Pickable { should_block_lower: false, is_hoverable: false }),
                    (Button Node { width: Val::Px(440.0), height: Val::Px(52.0), padding: UiRect::all(Val::Px(12.0)) }
                        BackgroundColor(Color::srgb(0.12, 0.18, 0.25)) PageButton
                        Children [(Text({"Page count: 0 (click / Space)".to_owned()}) Pickable { should_block_lower: false, is_hoverable: false })])
                ]
            })
        });
        routes
            .route_with_options(
                root,
                RoutePattern::path(name).unwrap(),
                RouteNodeOptions::new().with_view(view),
            )
            .unwrap();
    }
    let broken = register(&mut app, &mut views, "broken", |_| {
        Err::<Box<dyn Scene>, _>(BsnBuildError::new("Intentional example build failure"))
    });
    routes
        .route_with_options(
            root,
            RoutePattern::path("broken").unwrap(),
            RouteNodeOptions::new().with_view(broken),
        )
        .unwrap();
    let fallback = register(&mut app, &mut views, "fallback", |input| {
        let message = format!(
            "Build failed; fallback is visible.\n{:?}\n\nUse Overview or Settings to recover.",
            input.failure()
        );
        Ok(bsn! {
            Node { width: Val::Percent(100.0), height: Val::Percent(100.0), padding: UiRect::all(Val::Px(24.0)) }
            BackgroundColor(Color::srgb(0.36, 0.12, 0.15))
            Children [(Text(message) Pickable { should_block_lower: false, is_hoverable: false })]
        })
    });
    let router = app
        .add_ui_router(RouterConfig::new(routes.build().unwrap(), views, fallback))
        .unwrap();
    app.insert_resource(Demo {
        router,
        initial_requested: false,
        last_request: None,
        result: "Waiting for host".into(),
    })
    .add_systems(Startup, setup)
    .add_systems(PreUpdate, keyboard.after(RouterInputSet::Ready))
    .add_systems(
        Update,
        update_status.after(RouterScheduleSet::PublishSnapshots),
    )
    .add_observer(click)
    .add_observer(press)
    .add_observer(release)
    .run();
}

fn register<S: Scene>(
    app: &mut App,
    views: &mut RouteViewRegistry,
    name: &str,
    factory: impl FnMut(&RouteViewInput) -> Result<S, BsnBuildError> + Send + Sync + 'static,
) -> RouteViewKey {
    let driver = app
        .register_bsn_view(ViewDriverKey::new(name).unwrap(), factory)
        .unwrap();
    let key = RouteViewKey::new(name).unwrap();
    views
        .insert(key.clone(), RouteViewDescriptor::new(driver))
        .unwrap();
    key
}

fn setup(mut commands: Commands, demo: Res<Demo>) {
    commands.spawn(Camera2d);
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(20.0)),
                row_gap: Val::Px(16.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.04, 0.06, 0.1)),
        ))
        .with_children(|shell| {
            shell.spawn((
                Text::new("Bevy UI Router / interactive MVP"),
                TextFont {
                    font_size: FontSize::Px(28.0),
                    ..default()
                },
            ));
            shell
                .spawn(Node {
                    column_gap: Val::Px(12.0),
                    ..default()
                })
                .with_children(|bar| {
                    for (label, action) in [
                        ("Overview [1]", Action::Overview),
                        ("Settings [2]", Action::Settings),
                        ("Replace [R]", Action::Replace),
                        ("Back [B]", Action::Back),
                        ("Fail [E]", Action::Fail),
                    ] {
                        bar.spawn((
                            Button,
                            action,
                            Node {
                                padding: UiRect::all(Val::Px(12.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.17, 0.23, 0.32)),
                        ))
                        .with_child((Text::new(label), Pickable::IGNORE));
                    }
                });
            shell.spawn((
                Text::new(""),
                TextFont {
                    font_size: FontSize::Px(16.0),
                    ..default()
                },
                Status,
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
        Action::Overview => navigation.push(demo.router, RouteLocation::new("/overview")),
        Action::Settings => navigation.push(demo.router, RouteLocation::new("/settings")),
        Action::Replace => navigation.replace(demo.router, RouteLocation::new("/overview")),
        Action::Back => navigation.back(demo.router),
        Action::Fail => navigation.push(demo.router, RouteLocation::new("/broken")),
    };
    match result {
        Ok(request) => {
            demo.last_request = Some(request);
            demo.result = format!("{action:?} pending");
        }
        Err(error) => demo.result = format!("{action:?}: {error:?}"),
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
    mut navigation: ResMut<RouterNavigation>,
    focus: Res<InputFocus>,
    layouts: Query<Entity, With<Layout>>,
    counters: Query<(&Counter, &Children)>,
    pages: Query<(&PageButton, &Children)>,
    mut texts: Query<&mut Text>,
    status: Query<Entity, With<Status>>,
    mut last_state: Local<String>,
) {
    let Some(snapshot) = snapshots.get(demo.router) else {
        return;
    };
    if !demo.initial_requested && snapshot.availability() == &RouterAvailability::Ready {
        demo.initial_requested = true;
        navigate(Action::Overview, &mut demo, &mut navigation);
    }
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
    for (counter, children) in &counters {
        for child in children {
            if let Ok(mut text) = texts.get_mut(*child) {
                text.0 = format!("Layout counter: {} (click +1)", counter.0);
            }
        }
    }
    for (page, children) in &pages {
        for child in children {
            if let Ok(mut text) = texts.get_mut(*child) {
                text.0 = format!("Page count: {} (click / Space)", page.0);
            }
        }
    }
    let state = format!(
        "{} | history {} / {:?} | focus {:?}\n{}",
        snapshot
            .active()
            .location()
            .map_or("No active route", RouteLocation::path),
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
