# Add page navigation to a Bevy app

English | [简体中文](integration.zh-CN.md)

Build a window with Home and Settings pages: press `1` or `2` to navigate, and `B` to go back. All dependencies and code are included below. Existing Bevy applications can follow the same three steps.

First follow [Bevy’s environment setup](https://bevy.org/learn/quick-start/getting-started/setup/) for Rust and platform dependencies. Skip this if you already have a working Bevy window application.

## 1. Create a project

Run these commands in your projects directory so the two folders sit alongside each other:

```sh
git clone https://github.com/shengyusi-SYS/bevy_ui_router.git
cargo new router-demo
cd router-demo
```

If you already have Router, create `router-demo` next to it. Replace `Cargo.toml` with:

```toml
[package]
name = "router-demo"
version = "0.1.0"
edition = "2024"

[dependencies]
bevy = { version = "0.19", default-features = false, features = ["ui"] }
bevy_ui_router = { path = "../bevy_ui_router/crates/bevy_ui_router", features = ["bsn"] }
```

This enables Bevy's UI features and Router's `bsn` page integration. Router is not yet published, so the dependency uses a local path. In an existing application, add the Router dependency and keep your Bevy configuration.

## 2. Add pages and navigation

Put this complete program in `src/main.rs`:

```rust
use bevy::prelude::*;
use bevy_ui_router::prelude::*;

#[derive(Resource)]
struct Pages(RouterHandle);

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins);

    let router = app
        .add_bsn_router(
            BsnRouter::new(
                layout(|_| {
                    Ok(bsn! {
                        Node { width: percent(100), height: percent(100) }
                        DefaultOutlet
                    })
                })
                .children([
                    index(|_| Ok(page_content("Home"))),
                    page("settings", |_| Ok(page_content("Settings"))),
                ]),
            )
            .fallback(|_| Ok(page_content("Page unavailable")))
            .initial("/"),
        )
        .expect("valid routes");

    app.insert_resource(Pages(router))
        .add_systems(Startup, setup)
        .add_systems(Update, navigate)
        .run();
}

fn page_content(title: &'static str) -> impl Scene {
    bsn! {
        Node {
            width: percent(100), height: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
        }
        Children [(Text(title))]
    }
}

fn setup(mut commands: Commands, pages: Res<Pages>) {
    commands.spawn(Camera2d);
    commands
        .spawn(Node {
            width: percent(100),
            height: percent(100),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(px(24)),
            ..default()
        })
        .with_children(|root| {
            root.spawn(Text::new("1: Home    2: Settings    B: Back"));
            root.spawn((
                Node {
                    width: percent(100),
                    flex_grow: 1.0,
                    ..default()
                },
                RouterHostRegistration::new(pages.0),
            ));
        });
}

fn navigate(keys: Res<ButtonInput<KeyCode>>, pages: Res<Pages>, mut commands: Commands) {
    if keys.just_pressed(KeyCode::Digit1) {
        commands.router(pages.0).push("/");
    } else if keys.just_pressed(KeyCode::Digit2) {
        commands.router(pages.0).push("/settings");
    } else if keys.just_pressed(KeyCode::KeyB) {
        commands.router(pages.0).back();
    }
}
```

Three parts of the program handle routing:

1. **Declare pages.** `index` matches the parent path `/`; `page("settings", ...)` matches `/settings`. The outer `layout` provides a shared layout, and `DefaultOutlet` marks where the child page appears. Each closure returns a new Bevy Scene when a page needs creating; this example describes its UI with `bsn!`.
2. **Choose where pages appear.** `setup` adds `RouterHostRegistration` to an ordinary UI container. Routed pages appear inside it. Container sizes, cameras, and windows use normal Bevy UI setup.
3. **Navigate.** `navigate` calls `commands.router(handle)`. The example stores the handle in a `Pages` resource so systems can use it.

`.initial("/")` selects the first page. The required `.fallback(...)` supplies an error page for an unmatched address or failed page creation.

## 3. Run and switch pages

```sh
cargo run
```

The window starts on **Home**. Press `2` for **Settings**, then `B` to return to Home. The shortcut hint stays at the top while the page below changes. These are application-wide shortcuts.

There are three navigation operations:

```rust
commands.router(pages.0).push("/settings");     // Add a visit
commands.router(pages.0).replace("/settings");  // Replace the current visit
commands.router(pages.0).back();                // Return to the previous visit
```

These calls queue a page change rather than changing the screen immediately. Without an earlier visit, `back()` leaves the page in place and logs the reason. To handle results in code, see [navigation troubleshooting](debugging.md#diagnose-navigation-first).

## Add your own content

Replace `page_content` with your UI. Put a navigation bar in `layout` and `DefaultOutlet` in its body area; the navigation bar can stay in place as you switch between sibling pages.

Child paths are relative to their parent route. For an item detail page, declare `page("items/:id", ...)` and navigate to `/items/42`. Addresses also support query strings and fragments, such as `/items/42?sort=name#details`. See [page state and scrolling](pages.md) to read current parameters, retain pages, or load content later.

In an existing application, install Bevy plugins before `add_bsn_router`; it installs Router's systems. `DefaultPlugins` already includes the required Scene plugins.

<details>
<summary>When choosing Bevy plugins individually</summary>

Install `AssetPlugin` and `ScenePlugin` before adding Router. For manual route-tree and view-registry setup, see the [Scene reference](../contracts/scene.md#registration-and-runtime-setup).

</details>

Next, add [input and focus for page controls](pages.md#input-and-focus), [transitions](transitions.md), or [system Back](platform-back.md) as needed.

## Run the examples

After the tutorial, move from `router-demo` into the neighboring Router repository to try retained counters, scroll restoration, and error pages:

```sh
cd ../bevy_ui_router/examples/windowed
cargo run
```

To check routing without opening a window, return to the repository root and run:

```sh
cd ../..
cargo run -p bevy_ui_router --features bsn --example bsn_quick_start
```

It prints `Succeeded` and the current path, then exits. The [nested example](../../crates/bevy_ui_router/examples/bsn_nested.rs) combines layouts, query updates, delayed content, and scrolling:

```sh
cargo run -p bevy_ui_router --features bsn --example bsn_nested
```

Windowed examples need a display and graphics driver and use X11 on Linux. Follow the Bevy system setup linked at the start of this page. For layout inspection and screenshots, see [debugging](debugging.md#start-and-inspect-a-window).

## Bevy version requirements

| Bevy | Router |
| --- | --- |
| `0.19` | Current source, not yet published |

Starting with Bevy 0.19, each supported Bevy minor version has a corresponding Router release series. Choose the series matching your application. `version = "0.19"` accepts compatible patch updates; applications use their own lockfiles.

See the [roadmap](../roadmap.md#bevy-020-migration-not-started) for work on other Bevy versions.
