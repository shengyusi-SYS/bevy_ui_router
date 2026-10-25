# Documentation

English | [简体中文](README.md)

Start by [running an example](../README.en.md#run-the-examples), then follow the [integration guide](guides/integration.en.md). Each user and developer guide has an English and Chinese version.

| I want to… | Start here |
| --- | --- |
| Understand what Router does | [Overview](overview.en.md) |
| Add routing to a Bevy application | [Integration](guides/integration.en.md) |
| Add application-owned animation | [Transition guide](guides/transitions.en.md) |
| Debug a window, navigation, or input | [Debugging](guides/debugging.en.md) |
| Look up navigation, Scene, lifecycle, and input behavior | [Behavior contracts](contracts/README.en.md) |
| Understand modules and data structures | [Architecture](architecture/README.en.md) |
| Follow a navigation through the runtime | [Runtime](architecture/runtime.en.md) |
| Change ownership handoffs or failure cleanup | [Lifecycle execution](architecture/lifecycle.en.md) |
| Fix a bug or build a feature | [Development](development/README.en.md) |
| Run checks and validate packages | [Verification](development/verification.en.md) |
| Measure navigation and idle updates | [Performance](performance.en.md) |
| Understand key design choices | [Design rationale](development/design-lessons.en.md) |
| See upcoming work | [Roadmap](roadmap.en.md) |

## API reference and history

Generate the API reference from the repository root:

```sh
cargo doc --workspace --all-features --no-deps --locked
```

Open `target/doc/bevy_ui_router/index.html`. Examples link to runnable source; the development guide links features to implementation and tests. Completed plans and experiments remain available through `git log -- docs`. Retained [performance data](data/performance/README.en.md) includes source and environment information.
