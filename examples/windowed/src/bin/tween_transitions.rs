//! Run with `--bin tween_transitions --features tween`.
//! The application defines poses and timelines; the optional adapter handles
//! starting, cancellation, exit release, completion, and reactive redraw.

#[path = "tween_transitions/effects.rs"]
mod effects;

use bevy_input_focus::InputFocusPlugin;
use bevy_internal::{picking::Pickable, prelude::*};
use bevy_scene::{Scene, bsn};
use bevy_ui_router::prelude::*;
use effects::{Effects, prepare_effect};

#[derive(Resource)]
struct Demo(RouterHandle);

#[derive(Component, Clone)]
enum Action {
    Navigate(&'static str),
    Back,
    Effect,
    Hold,
}

#[derive(Component, Clone, Default)]
struct PageCounter(u32);

fn main() {
    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Bevy UI Router - tween adapter".into(),
                resolution: (960, 680).into(),
                ..default()
            }),
            ..default()
        }),
        InputFocusPlugin,
        RouteTweenPlugin::default(),
    ))
    .init_resource::<Effects>();

    let router = app
        .add_bsn_router(
            BsnRouter::new(
                layout(|_| {
                    Ok(bsn! {
                        Node { width: Val::Percent(100.0), height: Val::Percent(100.0) }
                        DefaultOutlet
                    })
                })
                .children([
                    page("notes", |_| {
                        Ok(page_scene("Notes", Color::srgb(0.08, 0.31, 0.35)))
                    })
                    .keep_alive(),
                    page("gallery", |_| {
                        Ok(page_scene("Gallery", Color::srgb(0.28, 0.19, 0.43)))
                    })
                    .keep_alive(),
                ]),
            )
            .fallback(|_| Ok(bsn! { Node Children [(Text::new("Page unavailable"))] }))
            .keep_alive_capacity(2)
            .initial("/notes")
            .transition_driver(RouteTweenDriver::new(prepare_effect)),
        )
        .unwrap();
    app.insert_resource(Demo(router))
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            update_counters.after(RouterScheduleSet::PublishSnapshots),
        )
        .add_observer(click)
        .run();
}

fn page_scene(title: &'static str, color: Color) -> impl Scene {
    bsn! {
        Node {
            width: Val::Percent(100.0), height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center, justify_content: JustifyContent::Center,
            row_gap: Val::Px(24.0),
        }
        BackgroundColor(color)
        Children [
            (Text::new(title) TextFont { font_size: FontSize::Px(36.0) }),
            (
                Button PageCounter(0)
                Node { padding: UiRect::all(Val::Px(18.0)) }
                BackgroundColor(Color::srgb(0.08, 0.12, 0.18))
                Children [(Text::new("Count: 0") Pickable::IGNORE)]
            ),
            (Text::new("Click the count, switch pages, then return. The instance retains its state.")),
        ]
    }
}

fn setup(mut commands: Commands, demo: Res<Demo>) {
    commands.spawn(Camera2d);
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(16.0),
                padding: UiRect::all(Val::Px(24.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.04, 0.06, 0.10)),
        ))
        .with_children(|root| {
            root.spawn((
                Text::new("Application effects, automatic transition coordination"),
                TextFont {
                    font_size: FontSize::Px(24.0),
                    ..default()
                },
            ));
            root.spawn(Node {
                column_gap: Val::Px(12.0),
                ..default()
            })
            .with_children(|bar| {
                for (text, action) in [
                    ("Notes", Action::Navigate("/notes")),
                    ("Gallery", Action::Navigate("/gallery")),
                    ("Back", Action::Back),
                    ("Switch effect", Action::Effect),
                    ("Toggle exit", Action::Hold),
                ] {
                    bar.spawn((
                        Button,
                        action,
                        Node {
                            padding: UiRect::all(Val::Px(12.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.14, 0.19, 0.28)),
                    ))
                    .with_child((Text::new(text), Pickable::IGNORE));
                }
            });
            root.spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_grow: 1.0,
                    min_height: Val::Px(0.0),
                    overflow: Overflow::clip(),
                    ..default()
                },
                RouterHostRegistration::new(demo.0),
            ));
        });
}

fn click(
    click: On<Pointer<Click>>,
    demo: Res<Demo>,
    actions: Query<&Action>,
    inputs: RouteInputs,
    mut counters: Query<&mut PageCounter>,
    mut effects: ResMut<Effects>,
    mut commands: Commands,
) {
    if let Ok(action) = actions.get(click.entity) {
        match action {
            Action::Navigate(path) => {
                commands.router(demo.0).push(*path);
            }
            Action::Back => {
                commands.router(demo.0).back();
            }
            Action::Effect => effects.scale_rotate = !effects.scale_rotate,
            Action::Hold => effects.hold_exit = !effects.hold_exit,
        }
    } else if inputs.current(click.entity).is_some() {
        if let Ok(mut counter) = counters.get_mut(click.entity) {
            counter.0 += 1;
        }
    }
}

fn update_counters(
    inputs: RouteInputs,
    counters: Query<(Entity, &PageCounter, &Children), Changed<PageCounter>>,
    mut text: Query<&mut Text>,
) {
    for (entity, counter, children) in &counters {
        if inputs.current(entity).is_none() {
            continue;
        }
        for child in children {
            if let Ok(mut text) = text.get_mut(*child) {
                text.0 = format!("Count: {}", counter.0);
            }
        }
    }
}
