# Bevy UI Router

English | [简体中文](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/README.md)

Organize pages, shared layouts, and navigation history in Bevy UI. Associate routes with page Scenes, then navigate with `push`, `replace`, and `back`. Router manages mounting, layout reuse, error presentation, and input revocation when content leaves.

- Retain layout and page state independently; restore scroll positions per history entry on Back.
- Read current paths, parameters, and activity from pages and reused layouts.
- Add application-defined transitions, optional Tween coordination, mouse Back, or Android legacy Back integration.

The current source targets **Bevy 0.19**, uses Rust edition 2024, and is not yet published. Later Bevy versions get corresponding Router release series; see the [compatibility table](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/docs/guides/integration.en.md#bevy-version-requirements).

## Usage at a glance

In a Bevy App with `AssetPlugin` and `ScenePlugin` installed, declare pages, select the initial route, and attach Router to an application-owned UI container:

```rust
use bevy_app::App;
use bevy_ecs::prelude::Commands;
use bevy_scene::bsn;
use bevy_ui::Node;
use bevy_ui_router::prelude::*;

fn install_pages(app: &mut App) -> Result<RouterHandle, BsnRouterError> {
    let router = app.add_bsn_router(
        BsnRouter::new(
            layout(|_| Ok(bsn! { Node DefaultOutlet })).children([
                index(|_| Ok(bsn! { Node })),
                page("settings", |_| Ok(bsn! { Node })),
            ]),
        )
        .fallback(|_| Ok(bsn! { Node }))
        .initial("/"),
    )?;
    app.world_mut()
        .spawn((Node::default(), RouterHostRegistration::new(router)));
    Ok(router)
}

fn open_settings(commands: &mut Commands, router: RouterHandle) {
    commands.router(router).push("/settings");
}
```

The `Node` values are placeholders for page content. Your application supplies real Scenes, container layout, windows, and cameras. `DefaultOutlet` marks the child-page location; `fallback` supplies your error page. Navigation also supports `.replace(...)` and `.back()`. Commands enqueue when Bevy applies deferred commands; use `RouterNavigation` when you need an immediate request ID.

## Run the examples

Start with a complete example that needs no window or graphics device:

```sh
git clone https://github.com/shengyusi-SYS/bevy_ui_router.git
cd bevy_ui_router
cargo run --locked -p bevy_ui_router --features bsn --example bsn_quick_start
```

It prints `Succeeded` and the current path, then exits. To try clicking, Back, and page retention directly, run:

```sh
cargo run --locked --manifest-path examples/windowed/Cargo.toml
```

The windowed example needs a display and graphics driver; Linux uses X11. See [integration](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/docs/guides/integration.en.md#run-the-examples) for dependencies and runtime setup.

| What to try next | Example |
| --- | --- |
| Nested layouts, query updates, asynchronous content, and scroll restoration | [Nested example](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/docs/guides/integration.en.md#run-the-examples), runs without a window |
| Compose transitions using existing tweens | [Tween example and integration](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/docs/guides/transitions.en.md#use-the-tween-adapter) |
| Connect your own animation system | [Manual driver example](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/docs/guides/transitions.en.md#run-the-manual-driver-example) |

## Add it to an application

Use a dependency pointing to your local checkout:

```toml
[dependencies]
bevy_ui_router = { path = "../bevy_ui_router/crates/bevy_ui_router", features = ["bsn"] }
```

The [integration guide](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/docs/guides/integration.en.md#use-router-in-a-separate-cargo-application) provides a complete `Cargo.toml` and plugin setup. Start with `bsn` for ordinary Bevy UI applications; Core/View can also be used independently.

| Feature | Purpose |
| --- | --- |
| `bsn` | Recommended Bevy Scene page integration; includes `ecs` and `view` |
| `tween` | Optional `bevy_tween` coordination, with application-defined effects |
| `desktop-back` / `android-back` | Mouse Back button / Android legacy Back key |
| `view` (default) | Renderer-neutral view descriptions and registration |
| `ecs` | Bevy execution layer for applications assembling their own view drivers |
| `platform` | Independent platform navigation contracts |

## Common tasks

| I want to… | Start here |
| --- | --- |
| Retain page state and control rebuilding on query changes | [Retain layouts and pages](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/docs/guides/integration.en.md#retain-layouts-and-pages) |
| Read current parameters and manage background work or reactivation | [Load content and reactivate pages](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/docs/guides/integration.en.md#load-content-and-reactivate-pages) |
| Restore scroll after Back, waiting for asynchronous content | [Restore scroll positions](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/docs/guides/integration.en.md#restore-scroll-positions) |
| Diagnose a missing initial page, failed navigation, or lost state | [Navigation and window debugging](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/docs/guides/debugging.en.md) |

## Documentation and contributions

- [Documentation index](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/docs/README.en.md): user guides, API reference, and developer entry points in both languages.
- [Overview](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/docs/overview.en.md): capabilities and responsibility boundaries.
- [Contributing](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/CONTRIBUTING.en.md): development entry points and checks.
- [Roadmap](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/docs/roadmap.en.md): upcoming work and release preparation.

## Production use

[LianYin](https://lianyin.eztools.fun/en/) is a cross-platform, multi-device music player that uses this library in production for navigation, history, page retention, and scroll restoration. See the [LianYin repository](https://github.com/shengyusi-SYS/lianyin) for product information and releases.

## License

Licensed under either [MIT](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/LICENSE-MIT) or [Apache-2.0](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/LICENSE-APACHE), at your option.
