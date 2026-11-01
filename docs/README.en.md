# Documentation

English | [简体中文](README.md)

## Use Router

Start with the [two-page window tutorial](guides/integration.en.md). Dependencies, complete code, and run instructions are all on one page.

Once it runs, read what you need:

- [Page state and scrolling](guides/pages.en.md): retain pages, read parameters, load content, restore scroll, and handle input.
- [Transitions](guides/transitions.en.md): design effects with Tween or your own animation system.
- [System Back](guides/platform-back.en.md): mouse Back and Android traditional Back.
- [Troubleshooting](guides/debugging.en.md): start with routing state, then inspect the window and input.

The [overview](overview.en.md) describes available capabilities. The [compatibility table](guides/integration.en.md#bevy-version-requirements) helps you select a version.

## Look up behavior

The [behavior reference](contracts/README.en.md) covers navigation, Scenes, lifecycle, and input. Generate the API reference from the repository root:

```sh
cargo doc -p bevy_ui_router --all-features --no-deps --open
```

## Contribute

Use the [development guide](development/README.en.md) to find implementations and tests. See [architecture](architecture/README.en.md) for module relationships and [runtime](architecture/runtime.en.md) for execution flow.

[Verification](development/verification.en.md) · [Performance](performance.en.md) · [Design rationale](development/design-lessons.en.md) · [Roadmap](roadmap.en.md)

Completed plans and experiments are available through `git log -- docs`; [performance data](data/performance/README.en.md) includes source and environment information.
