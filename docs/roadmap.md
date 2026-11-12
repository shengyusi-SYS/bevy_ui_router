# Roadmap

English | [简体中文](roadmap.zh-CN.md)

See the [overview](overview.md) for available features and the [behavior contracts](contracts/README.md) for detailed semantics. This page tracks upcoming work.

## In progress

| Work | Deliverable |
| --- | --- |
| Public documentation | English and Chinese user and developer guides, runnable examples, file cleanup, and privacy checks |
| Public query performance | A borrowed read session shared by activity, current-input, and pending-scroll queries; [delivery and verification plan](changes/query-performance/plan.md) |
| Release preparation | MIT OR Apache-2.0 dual licensing is selected; choose the release version, then validate the final packages with an independent consumer |

Applications and UI code implement animation effects. The project uses MIT OR Apache-2.0 dual licensing. Maintainers will choose the release version; applications currently use a local checkout dependency.

Current source targets Bevy `0.20`, with dependencies and standalone consumers declaring `0.20.1` and the optional Tween adapter declaring `bevy_tween 0.14.1`. See [version requirements](guides/integration.md#bevy-version-requirements) and [verification](development/verification.md) for compatibility and checks.

## Future directions

Automatic route activity integration with native Tab navigation and input dispatch needs suitable public extension points in Bevy/UI. This work also includes focus admission and replacement of the current focus bridge. Applications can use `RouterInputGate` in keyboard and gamepad handlers today.

Other directions include browser URL/history integration, asynchronous Scene loading, typed routes and search, multiple outlets, other renderers, and a public instance-eviction API. Each starts with an application use case, expected behavior, and a verification approach before an implementation plan.

## Release sequence

Choose the release version, update Cargo metadata, then run the checks and Cargo package verification from the [verification guide](development/verification.md). The six crates can be released in dependency order: Core → View → BSN / Platform → ECS → facade. Platform depends only on Core.
