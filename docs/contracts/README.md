# Behavior contracts

[中文](README.zh-CN.md) | [English](README.md)

These pages describe current public behavior and the cross-layer rules that support it. See the [integration guide](../guides/integration.md) for setup and [architecture](../architecture/README.md) for module relationships.

| Topic | Contents |
| --- | --- |
| [Navigation and observation](navigation.md) | Matching, params and locations, retention, history, scrolling, and snapshots |
| [Scenes and mounting](scene.md) | Factory registration, Scene creation, content boundaries, and the default outlet |
| [Lifecycle and cleanup](lifecycle.md) | Hosts, identity, ownership transfers, publication, failure recovery, and cleanup |
| [Input and focus](input-focus.md) | Input eligibility, gesture cancellation, focus, activity queries, and window Back input |

## Shared responsibilities

Core decides matching, request ordering, logical frame/generation, history, recovery, and presentation publication. View provides renderer-neutral keys, descriptors, a registry, and read-only effects/inputs.

ECS executes host/slot/frame bindings, Scene creation, staging, input/focus handling, publication preparation, and cleanup. BSN supplies Scene factories, outlet declarations, and build errors. The facade combines common APIs and optional features; internal controlled handoffs manage lifecycle owners and registry writes.

Source code and rustdoc define exact public signatures. When changing behavior, update its contract and tests together. Use the [development guide](../development/README.md) to locate cross-layer work.
