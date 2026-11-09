# Bevy UI Router

English | [简体中文](README.zh-CN.md)

Page navigation for Bevy UI. Organize pages by path, reuse shared layouts, and navigate with `push`, `replace`, and `back`. Opt into retained page state, scroll restoration on Back, and transitions designed by your application.

Currently supports **Bevy 0.20.1**. Not yet published to crates.io.

This project is developed entirely by GPT flagship models (to use up my remaining quota); my role is discussion, supervision, and feedback.

## Run the examples

To try the existing features first, run the interactive example:

```sh
git clone https://github.com/shengyusi-SYS/bevy_ui_router.git
cd bevy_ui_router/examples/windowed
cargo run
```

Click Overview / Settings, change a counter and return to it, then try scrolling and Back. Without a desktop environment, try the [headless examples](docs/guides/integration.md#run-the-examples).

## Add routing to your app

This declares a home page at `/` and settings at `/settings`. `DefaultOutlet` marks where the child page appears:

```rust
let pages = BsnRouter::new(
    layout(|_| Ok(bsn! { Node DefaultOutlet })).children([
        index(|_| Ok(bsn! { Text("Home") })),
        page("settings", |_| Ok(bsn! { Text("Settings") })),
    ]),
)
.fallback(|_| Ok(bsn! { Text("Page unavailable") }))
.initial("/");
```

Page content uses Bevy's `bsn!` syntax.

**[Start here: run a complete two-page window](docs/guides/integration.md)**. The tutorial includes dependencies, complete code, navigation controls, and an explanation of each step.

## Next steps

- [Page state and scrolling](docs/guides/pages.md): retain state, read current parameters, load content, and restore scroll.
- [Transitions](docs/guides/transitions.md): define effects with Tween or your existing animation system.
- [System Back](docs/guides/platform-back.md): connect mouse Back and Android traditional Back.
- [Troubleshooting](docs/guides/debugging.md): find missing pages, navigation failures, or lost state.

Find API references, behavior rules, and developer material in the [documentation index](docs/README.md). To contribute, see [contributing](CONTRIBUTING.md).

## Production use

[LianYin](https://lianyin.eztools.fun/en/) is a cross-platform, multi-device music player that uses this library in production for navigation, history, page retention, and scroll restoration. See the [LianYin repository](https://github.com/shengyusi-SYS/lianyin) for product information and releases.

## License

Licensed under either [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
