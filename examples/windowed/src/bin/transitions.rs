//! Run: `cargo run --manifest-path examples/windowed/Cargo.toml --bin transitions`.
//!
//! 1/2/3 navigate to two retained pages and a fresh page; B goes Back. E switches
//! the next animation between slide and scale/rotation; H toggles exit holding.
//! R starts a burst of navigation while animations are running. Page counters
//! reveal retained-instance reuse versus fresh creation. Buttons do the same.
//!
//! The driver is an ordinary consumer of the public API. `animation.rs` owns
//! every timing, effect, and overlapping-layout choice. Its single resource is
//! the animation writer; cancellation removes it synchronously. No animation
//! components are attached to router-owned shells and no third-party runtime is
//! required. The unchanged application host clips both example effects.

#[path = "transitions/animation.rs"]
mod animation;

#[cfg(all(debug_assertions, feature = "ui-debug"))]
#[path = "../ui_debug.rs"]
mod ui_debug;

use animation::{AnimationOptions, AnimationState, DemoTransitionDriver, Effect};
use bevy_input_focus::InputFocusPlugin;
use bevy_internal::{picking::Pickable, prelude::*};
use bevy_scene::{Scene, bsn};
use bevy_ui_router::{
    ecs::{RouterInputSet, RouterScheduleSet},
    prelude::*,
};

#[derive(Clone, Copy, Component)]
enum Action {
    Notes,
    Gallery,
    Fresh,
    Back,
    Effect,
    Hold,
    Burst,
}

#[derive(Component, Clone, Default)]
struct PageCounter(u32);

#[derive(Component)]
struct Status;

#[derive(Resource)]
struct Demo {
    router: RouterHandle,
    initial_requested: bool,
    last_request: Option<NavigationRequestId>,
    result: String,
    burst: Option<(Timer, usize)>,
}

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "Bevy UI Router - application transitions".into(),
            resolution: (1060, 720).into(),
            focused: !cfg!(all(debug_assertions, feature = "ui-debug")),
            ..default()
        }),
        ..default()
    }))
    .add_plugins(InputFocusPlugin)
    .init_resource::<AnimationOptions>()
    .init_resource::<AnimationState>();
    #[cfg(all(debug_assertions, feature = "ui-debug"))]
    app.add_plugins(ui_debug::UiDebugPlugin);

    let mut views = RouteViewRegistry::new();
    let root_view = register(&mut app, &mut views, "root", |_| {
        Ok(bsn! {
            Node { width: Val::Percent(100.0), height: Val::Percent(100.0) } DefaultOutlet
        })
    });
    let mut routes = RouteTree::builder()
        .root_with_options(
            RoutePattern::pathless(),
            RouteNodeOptions::new().with_view(root_view),
        )
        .unwrap();
    let root = routes.root_id().unwrap();
    for (name, retained, color) in [
        ("notes", true, Color::srgb(0.08, 0.31, 0.35)),
        ("gallery", true, Color::srgb(0.28, 0.19, 0.43)),
        ("fresh", false, Color::srgb(0.40, 0.22, 0.12)),
    ] {
        let view = register(&mut app, &mut views, name, move |_| {
            let title = format!(
                "{} / {}",
                name.to_uppercase(),
                if retained {
                    "retained instance"
                } else {
                    "new instance on return"
                }
            );
            Ok(bsn! {
                Node {
                    width: Val::Percent(100.0), height: Val::Percent(100.0),
                    padding: UiRect::all(Val::Px(42.0)), row_gap: Val::Px(22.0),
                    flex_direction: FlexDirection::Column,
                }
                BackgroundColor(color) Name::new(format!("{name}-page"))
                Children [
                    (Text(title) TextFont { font_size: FontSize::Px(34.0) } Pickable::IGNORE),
                    (Text({"The router publishes the route. This application animates its surface.".to_owned()}) Pickable::IGNORE),
                    (Button PageCounter Name::new(format!("{name}-counter"))
                        Node { width: Val::Px(330.0), padding: UiRect::all(Val::Px(18.0)) }
                        BackgroundColor(Color::srgb(0.07, 0.10, 0.16))
                        Children [(Text({"Page count: 0 (click +1)".to_owned()}) Pickable::IGNORE)]),
                    (Node { flex_grow: 1.0 } Pickable::IGNORE),
                    (Text({"Try E, then navigate again. Try H for enter-only presentation.\nR sends new requests before earlier animations finish.".to_owned()}) Pickable::IGNORE)
                ]
            })
        });
        routes
            .route_with_options(
                root,
                RoutePattern::path(name).unwrap(),
                RouteNodeOptions::new()
                    .with_view(view)
                    .with_keep_alive(retained),
            )
            .unwrap();
    }
    let fallback = register(&mut app, &mut views, "fallback", |input| {
        let message = format!("View error: {:?}", input.failure());
        Ok(bsn! {
            Node { width: Val::Percent(100.0), height: Val::Percent(100.0), padding: UiRect::all(Val::Px(30.0)) }
            BackgroundColor(Color::srgb(0.35, 0.08, 0.1))
            Children [(Text(message) Pickable::IGNORE)]
        })
    });
    let router = app
        .add_ui_router(
            RouterConfig::new(routes.build().unwrap(), views, fallback)
                .with_keep_alive_capacity(2)
                .with_transition_driver(DemoTransitionDriver),
        )
        .unwrap();
    app.insert_resource(Demo {
        router,
        initial_requested: false,
        last_request: None,
        result: "Waiting for host".into(),
        burst: None,
    })
    .add_systems(Startup, setup)
    .add_systems(PreUpdate, keyboard.after(RouterInputSet::Ready))
    .add_systems(
        Update,
        (
            initial_navigation,
            rapid_navigation,
            animation::animate,
            update_text,
        )
            .chain()
            .after(RouterScheduleSet::PublishSnapshots),
    )
    .add_observer(click)
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
                row_gap: Val::Px(14.0),
                padding: UiRect::all(Val::Px(20.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.035, 0.05, 0.08)),
        ))
        .with_children(|shell| {
            shell.spawn((
                Text::new("Route presence / application animation"),
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
                        ("Notes [1]", Action::Notes),
                        ("Gallery [2]", Action::Gallery),
                        ("Fresh [3]", Action::Fresh),
                        ("Back [B]", Action::Back),
                        ("Effect [E]", Action::Effect),
                        ("Hold exit [H]", Action::Hold),
                        ("Rapid [R]", Action::Burst),
                    ] {
                        bar.spawn((
                            Button,
                            action,
                            Node {
                                padding: UiRect::all(Val::Px(10.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.13, 0.19, 0.28)),
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
            ));
            shell.spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_grow: 1.0,
                    min_height: Val::Px(0.0),
                    overflow: Overflow::clip(),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.06, 0.09, 0.14)),
                RouterHostRegistration::new(demo.router),
            ));
        });
}

fn request(path: &str, demo: &mut Demo, navigation: &mut RouterNavigation) {
    match navigation.push(demo.router, RouteLocation::new(path)) {
        Ok(id) => {
            demo.last_request = Some(id);
            demo.result = format!("Requested {path}");
        }
        Err(error) => demo.result = format!("{error:?}"),
    }
}

fn act(
    action: Action,
    demo: &mut Demo,
    options: &mut AnimationOptions,
    navigation: &mut RouterNavigation,
) {
    match action {
        Action::Notes => request("/notes", demo, navigation),
        Action::Gallery => request("/gallery", demo, navigation),
        Action::Fresh => request("/fresh", demo, navigation),
        Action::Back => match navigation.back(demo.router) {
            Ok(id) => {
                demo.last_request = Some(id);
                demo.result = "Requested Back".into();
            }
            Err(error) => demo.result = format!("{error:?}"),
        },
        Action::Effect => {
            options.effect = match options.effect {
                Effect::Slide => Effect::ScaleRotate,
                Effect::ScaleRotate => Effect::Slide,
            }
        }
        Action::Hold => options.hold_exit = !options.hold_exit,
        Action::Burst => demo.burst = Some((Timer::from_seconds(0.14, TimerMode::Repeating), 0)),
    }
}

fn keyboard(
    keys: Res<ButtonInput<KeyCode>>,
    mut demo: ResMut<Demo>,
    mut options: ResMut<AnimationOptions>,
    mut navigation: ResMut<RouterNavigation>,
) {
    for (key, action) in [
        (KeyCode::Digit1, Action::Notes),
        (KeyCode::Digit2, Action::Gallery),
        (KeyCode::Digit3, Action::Fresh),
        (KeyCode::KeyB, Action::Back),
        (KeyCode::KeyE, Action::Effect),
        (KeyCode::KeyH, Action::Hold),
        (KeyCode::KeyR, Action::Burst),
    ] {
        if keys.just_pressed(key) {
            act(action, &mut demo, &mut options, &mut navigation);
        }
    }
}

fn click(
    event: On<Pointer<Click>>,
    actions: Query<&Action>,
    mut counters: Query<&mut PageCounter>,
    gate: Res<RouterInputGate>,
    mut demo: ResMut<Demo>,
    mut options: ResMut<AnimationOptions>,
    mut navigation: ResMut<RouterNavigation>,
) {
    let target = event.event_target();
    if let Ok(action) = actions.get(target) {
        act(*action, &mut demo, &mut options, &mut navigation);
    } else if gate.allows_pointer(target)
        && let Ok(mut counter) = counters.get_mut(target)
    {
        counter.0 += 1;
    }
}

fn initial_navigation(
    mut demo: ResMut<Demo>,
    snapshots: Res<RouterSnapshots>,
    mut navigation: ResMut<RouterNavigation>,
) {
    if !demo.initial_requested
        && snapshots
            .get(demo.router)
            .is_some_and(|snapshot| snapshot.availability() == &RouterAvailability::Ready)
    {
        demo.initial_requested = true;
        request("/notes", &mut demo, &mut navigation);
    }
}

fn rapid_navigation(
    time: Res<Time<Real>>,
    mut demo: ResMut<Demo>,
    mut navigation: ResMut<RouterNavigation>,
) {
    let Some((mut timer, mut index)) = demo.burst.take() else {
        return;
    };
    timer.tick(time.delta());
    let paths = [
        "/gallery", "/fresh", "/notes", "/gallery", "/fresh", "/notes",
    ];
    if timer.just_finished() {
        request(paths[index], &mut demo, &mut navigation);
        index += 1;
    }
    if index < paths.len() {
        demo.burst = Some((timer, index));
    }
}

fn update_text(
    mut demo: ResMut<Demo>,
    options: Res<AnimationOptions>,
    animations: Res<AnimationState>,
    snapshots: Res<RouterSnapshots>,
    counters: Query<(&PageCounter, &Children)>,
    status: Query<Entity, With<Status>>,
    mut texts: Query<&mut Text>,
) {
    if let Some(id) = demo.last_request
        && let RequestLookup::Found(result) = snapshots.request(demo.router, id)
        && matches!(
            result.status(),
            RequestStatus::Succeeded | RequestStatus::Failed(_)
        )
    {
        demo.result = format!("Request: {:?}", result.status());
        demo.last_request = None;
    }
    for (counter, children) in &counters {
        for child in children {
            if let Ok(mut text) = texts.get_mut(*child) {
                text.set_if_neq(Text::new(format!("Page count: {} (click +1)", counter.0)));
            }
        }
    }
    for entity in &status {
        if let Ok(mut text) = texts.get_mut(entity) {
            text.set_if_neq(Text::new(format!(
                "Next effect: {:?} | Hold exit: {} | Cancellations: {} | {}\n{}",
                options.effect,
                options.hold_exit,
                animations.cancellations,
                demo.result,
                animations.last_event
            )));
        }
    }
}
