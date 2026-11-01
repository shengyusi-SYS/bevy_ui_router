# Roadmap

English | [简体中文](roadmap.zh-CN.md)

See the [overview](overview.md) for available features and the [behavior contracts](contracts/README.md) for detailed semantics. This page tracks upcoming work.

## In progress

| Work | Deliverable |
| --- | --- |
| Public documentation | English and Chinese user and developer guides, runnable examples, file cleanup, and privacy checks |
| Release preparation | MIT OR Apache-2.0 dual licensing is selected; choose the release version, then validate the final packages with an independent consumer |

Applications and UI code implement animation effects. The project uses MIT OR Apache-2.0 dual licensing. Maintainers will choose the release version; applications currently use a local checkout dependency.

## Future directions

### Bevy 0.20 migration (not started)

Support Bevy 0.19 and subsequent minor releases through the [version-mapping policy](guides/integration.md#bevy-version-requirements). The 0.20 compatibility assessment is complete; migration is deferred. Implementation work:

- Move Bevy to the `0.20` series and `bevy_tween` to `0.14`, update standalone consumers and windowed examples, and check toolchain requirements.
- Migrate Query iteration, pointer events, observer generics, BSN expressions, and affected Template, clipping, and AutoFocus tests.
- Include `PointerCaptureMap` in existing page input revocation, preserving other valid targets' captures and testing normal hit detection after navigation.
- Validate affected crates, public journeys, standalone consumers, and examples. Recheck focus, scroll, and animation scheduling, rerun the existing animation benchmark, and update the compatibility table and user documentation.

### Other directions

Automatic route activity integration with native Tab navigation and input dispatch needs suitable public extension points in Bevy/UI. This work also includes focus admission and replacement of the current focus bridge. Applications can use `RouterInputGate` in keyboard and gamepad handlers today.

Other directions include browser URL/history integration, asynchronous Scene loading, typed routes and search, multiple outlets, other renderers, and a public instance-eviction API. Each starts with an application use case, expected behavior, and a verification approach before an implementation plan.

## Release sequence

Choose the release version, update Cargo metadata, then run the checks and Cargo package verification from the [verification guide](development/verification.md). The six crates can be released in dependency order: Core → View → BSN / Platform → ECS → facade. Platform depends only on Core.
