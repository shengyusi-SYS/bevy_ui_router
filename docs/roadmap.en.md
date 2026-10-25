# Roadmap

English | [简体中文](roadmap.md)

See the [overview](overview.en.md) for available features and the [behavior contracts](contracts/README.en.md) for detailed semantics. This page tracks upcoming work.

## In progress

| Work | Deliverable |
| --- | --- |
| View transition lifecycle | Controlled APIs for application animations to keep leaving views alive, then complete or cancel a transition, with an example and combined behavior tests. See the [design](changes/view-transitions/design.en.md) and [plan](changes/view-transitions/plan.en.md) |
| Public documentation | English and Chinese user and developer guides, runnable examples, file cleanup, and privacy checks |
| Release preparation | MIT OR Apache-2.0 dual licensing is selected; choose the release version, then validate the final packages with an independent consumer |

Applications and UI code implement animation effects. The project uses MIT OR Apache-2.0 dual licensing. Maintainers will choose the release version; applications currently use a local checkout dependency.

## Future directions

Automatic route activity integration with native Tab navigation and input dispatch needs suitable public extension points in Bevy/UI. This work also includes focus admission and replacement of the current focus bridge. Applications can use `RouterInputGate` in keyboard and gamepad handlers today.

Other directions include browser URL/history integration, asynchronous Scene loading, typed routes and search, multiple outlets, other renderers, and a public instance-eviction API. Each starts with an application use case, expected behavior, and a verification approach before an implementation plan.

## Release sequence

Choose the release version, update Cargo metadata, then run the final-tree checks and packaged consumer from the [verification guide](development/verification.en.md). The six crates can be released in dependency order: Core → View → BSN / Platform → ECS → facade. Platform depends only on Core.

Release content checks cover source, documentation, examples, and package contents. Scan Git history separately. To share only the current source, create a standalone release snapshot from the checked files.
