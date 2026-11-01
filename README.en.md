# Bevy UI Router

English | [简体中文](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/README.md)

Organize pages, shared layouts, and navigation history in Bevy UI. Define a route tree and page Scenes, then navigate with `push`, `replace`, and `back`. Router preserves reusable layouts and handles mounting, error views, input, and focus.

- **Nested pages**: path parameters, index routes, and pathless layouts, with one default outlet per layout.
- **Page continuity**: retain layouts and child pages independently, including their entities and local state. Restore scroll positions for each history entry.
- **Current context**: pages and reused layouts can read the current route, parameters, and activity state.
- **Optional integration**: use Core/View independently, enable `bsn` for Bevy Scenes, or add desktop mouse and Android back adapters.

## Run the examples

Clone the repository and run the smallest example from its root:

```sh
git clone https://github.com/shengyusi-SYS/bevy_ui_router.git
cd bevy_ui_router
cargo run --locked -p bevy_ui_router --features bsn --example bsn_quick_start
```

On success, it prints `Succeeded` and the current path, then exits. It demonstrates a complete navigation through a finite sequence of updates. Run the nested example next to explore shared layouts, current inputs, application data loading, and scroll restoration:

```sh
cargo run --locked -p bevy_ui_router --features bsn --example bsn_nested
```

Open the interactive window:

```sh
cargo run --locked --manifest-path examples/windowed/Cargo.toml
```

The window demonstrates navigation, back, layout and page counters, error recovery, keyboard interaction, and scrolling. On Linux it uses X11; see the [integration guide](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/docs/guides/integration.en.md) for build dependencies and controls.

The optional Tween adapter lets applications define effects while the library coordinates start, cancellation, and completion:

```sh
cargo run --locked --manifest-path examples/windowed/Cargo.toml --features tween --bin tween_transitions
```

For custom animation systems, the manual driver example provides slide, scale/rotation, and cancellation during queued navigation:

```sh
cargo run --locked --manifest-path examples/windowed/Cargo.toml --bin transitions
```

## Add it to an application

Use a local dependency pointing to your checkout:

```toml
[dependencies]
bevy_ui_router = { path = "../bevy_ui_router/crates/bevy_ui_router", features = ["bsn"] }
```

The project uses Rust edition 2024 and Bevy 0.19. The lockfile pins Bevy 0.19.1. Use a Rust toolchain capable of building that Bevy version.

Follow the [integration guide](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/docs/guides/integration.en.md): associate routes and factories directly with `BsnRouter`, configure `initial`, and mount an application-owned host. Ordinary page systems navigate with `commands.router(handle).push("/items/42?tab=info")`. The [quick-start source](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/crates/bevy_ui_router/examples/bsn_quick_start.rs) is a complete runnable example.

| Feature | Purpose |
| --- | --- |
| `view` (default) | Renderer-neutral view descriptions and registration |
| `ecs` | Bevy plugin, hosts, navigation, snapshots, activity, and input gate |
| `bsn` | Recommended Scene integration; includes `ecs` and `view` |
| `tween` | Optional `bevy_tween` transition coordination; includes `ecs`, with application-defined effects |
| `platform` | Platform navigation interfaces |
| `desktop-back` / `android-back` | Mouse back button / Android legacy back key |

Your application owns its windows, cameras, UI styling, and data tasks. Router manages routed content inside the host. Keyboard and gamepad handlers query `RouterInputGate` after `RouterInputSet::Ready`; the guide shows the wiring.

## Documentation and contributions

- [Documentation](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/docs/README.en.md): user and developer guides in English and Chinese.
- [Overview](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/docs/overview.en.md): capabilities and common concepts.
- [Contributing](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/CONTRIBUTING.en.md): development entry points and checks.
- [Roadmap](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/docs/roadmap.en.md): work in progress and release preparation.

## Production use

[LianYin](https://lianyin.eztools.fun/en/) is a cross-platform, multi-device music player that uses this library in production for navigation, history, page retention, and scroll restoration. See the [LianYin repository](https://github.com/shengyusi-SYS/lianyin) for product information and releases.

## License

Licensed under either [MIT](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/LICENSE-MIT) or [Apache-2.0](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/LICENSE-APACHE), at your option.
