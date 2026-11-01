# Feature development

[中文](README.md) | [English](README.en.md)

Use the feature map below to find the relevant contracts, implementation, and tests. The [architecture overview](../architecture/README.en.md) explains module relationships, [runtime guide](../architecture/runtime.en.md) explains execution order, and [verification guide](verification.en.md) lists commands.

## Find the code for a change

| Topic | Behavior | Main implementation | Verification |
| --- | --- | --- | --- |
| Declaration installation, initial pages, and Commands navigation | [Integration](../guides/integration.en.md), [navigation contract](../contracts/navigation.en.md) | ECS [declarations](../../crates/bevy_ui_router_ecs/src/bsn/declaration.rs), [installation](../../crates/bevy_ui_router_ecs/src/bsn/install.rs), [Commands](../../crates/bevy_ui_router_ecs/src/navigation_commands.rs) | [Public usage journeys](../../crates/bevy_ui_router/tests/public_usage.rs), [installation tests](../../crates/bevy_ui_router_ecs/src/bsn/install/tests.rs) |
| Path matching, parameters, query/fragment, and common-prefix reuse | [Navigation](../contracts/navigation.en.md) | Core [matcher](../../crates/bevy_ui_router_core/src/matcher.rs), [branch diff](../../crates/bevy_ui_router_core/src/branch_diff.rs) | Core tests, [public navigation journeys](../../crates/bevy_ui_router/tests/public_navigation_journeys.rs) |
| push/replace/back, history, and snapshots | [Navigation and observation](../contracts/navigation.en.md) | Core [engine](../../crates/bevy_ui_router_core/src/engine.rs), [history](../../crates/bevy_ui_router_core/src/history.rs); ECS [navigation](../../crates/bevy_ui_router_ecs/src/navigation.rs), [snapshots](../../crates/bevy_ui_router_ecs/src/snapshots.rs) | [Public navigation journeys](../../crates/bevy_ui_router/tests/public_navigation_journeys.rs), including nested Back after rebuilding |
| Layout/page retention, capacity, and LRU | [Retention](../contracts/navigation.en.md), [retained lifecycle](../contracts/lifecycle.en.md) | Core [retained](../../crates/bevy_ui_router_core/src/engine/retained.rs); ECS [retained](../../crates/bevy_ui_router_ecs/src/retained.rs), [retained miss](../../crates/bevy_ui_router_ecs/src/navigation/retained_miss.rs) | [Core retention tests](../../crates/bevy_ui_router_core/src/integration_retained_tests.rs), [public retention journeys](../../crates/bevy_ui_router/tests/retained_navigation.rs) |
| Scroll restoration per visit | [Scroll restoration](../contracts/navigation.en.md) | Core [history authority](../../crates/bevy_ui_router_core/src/history_authority.rs); ECS [scroll](../../crates/bevy_ui_router_ecs/src/scroll.rs) | [Real layout and scroll journeys](../../crates/bevy_ui_router/tests/scroll_restoration.rs) |
| Page activity and current inputs | [Page activity](../contracts/input-focus.en.md), [navigation](../contracts/navigation.en.md) | ECS [activity](../../crates/bevy_ui_router_ecs/src/activity.rs), [route inputs](../../crates/bevy_ui_router_ecs/src/route_inputs.rs) | [Activity journeys](../../crates/bevy_ui_router/tests/public_activity.rs), [standalone BSN application](../../crates/bevy_ui_router/tests/consumers/bsn.rs) |
| Scene factories, plugin setup, and default outlets | [Scenes and mounting](../contracts/scene.en.md) | [BSN](../../crates/bevy_ui_router_bsn/src/lib.rs), [ECS bridge](../../crates/bevy_ui_router_ecs/src/bsn.rs), [native Scene](../../crates/bevy_ui_router_ecs/src/presentation/native_scene.rs), [mount](../../crates/bevy_ui_router_ecs/src/mount.rs) | [BSN facade tests](../../crates/bevy_ui_router/tests/bsn_facade_features.rs), [standalone BSN application](../../crates/bevy_ui_router/tests/consumers/bsn.rs) |
| Hosts, identity, publication, failure recovery, and cleanup | [Lifecycle contract](../contracts/lifecycle.en.md), [execution protocol](../architecture/lifecycle.en.md) | Core [integration](../../crates/bevy_ui_router_core/src/integration.rs); ECS [registry](../../crates/bevy_ui_router_ecs/src/registry.rs), [navigation](../../crates/bevy_ui_router_ecs/src/navigation.rs), [cleanup](../../crates/bevy_ui_router_ecs/src/cleanup.rs) | Full affected crates, [native regressions](../../crates/bevy_ui_router_ecs/src/navigation/native_tests.rs), public journeys, and standalone applications |
| Route transitions | [Transition guide](../guides/transitions.en.md), [lifecycle](../contracts/lifecycle.en.md) | Core [exit](../../crates/bevy_ui_router_core/src/engine/exit.rs), ECS [transition](../../crates/bevy_ui_router_ecs/src/transition.rs), [Tween adapter](../../crates/bevy_ui_router_ecs/src/transition/tween.rs), and [exit disposition](../../crates/bevy_ui_router_ecs/src/navigation/deferred_exit.rs) | [Public transition journeys](../../crates/bevy_ui_router/tests/route_transitions.rs), [Tween adapter tests](../../crates/bevy_ui_router_ecs/src/transition/tween/tests.rs), real picking, and window effects |
| Desktop mouse and Android traditional Back | [Window-level Back](../contracts/input-focus.en.md) | Platform [Back input](../../crates/bevy_ui_router_platform/src/back_input.rs), its two input sources, and the application's Back handler | [Back journeys](../../crates/bevy_ui_router/tests/platform_back.rs), minimal-dependency applications, and windowed example compilation |
| Pointer, keyboard/gamepad gates, and focus | [Input and focus](../contracts/input-focus.en.md), [scheduling](../architecture/runtime.en.md) | ECS [input](../../crates/bevy_ui_router_ecs/src/input.rs), [native focus](../../crates/bevy_ui_router_ecs/src/native_focus.rs), [plugin](../../crates/bevy_ui_router_ecs/src/plugin.rs) | [Native pointer](../../crates/bevy_ui_router/tests/bsn_native_input.rs), [native focus](../../crates/bevy_ui_router/tests/bsn_native_focus.rs), and window interaction |
| Facade, features, dependencies, examples, and package contents | [Shared boundaries](../contracts/README.en.md), [Scene registration](../contracts/scene.en.md) | [Facade](../../crates/bevy_ui_router/src/lib.rs), [manifest](../../crates/bevy_ui_router/Cargo.toml), [App extension](../../crates/bevy_ui_router_ecs/src/app_ext.rs) | [Standalone applications](../../crates/bevy_ui_router/tests/facade_consumers.rs), [dependency checks](../../crates/bevy_ui_router/tests/dependency_contract.rs), examples, and packaged consumer |
| Performance | [Performance guide](../performance.en.md) and the affected feature contract | Profile the actual workload to locate hotspots | Same-machine benchmark comparisons and corresponding behavior tests |

For cross-layer changes, follow the affected behavior through related topics. Navigation reuse affects Scene rebuilding, frame identity, and focus; outlet changes affect mounting and failure cleanup. Choose tests using the [verification guide](verification.en.md).

## Develop features and maintain documentation

Start with the user problem, expected behavior, and gap in the current implementation. Identify the modules and contracts that need to change. Complex features can use `docs/changes/<feature>/design.md` and `plan.md`: the design explains behavior, boundaries, and reasons; the plan orders delivery, checks, and review. Small fixes update the relevant code, tests, and documentation directly.

Mark unfinished designs with their status and scope at the top, and link their current contracts. The [roadmap](../roadmap.en.md) lists active topics. Read [design lessons](design-lessons.en.md) for decisions about input extensions and other tradeoffs.

When a feature is complete, update its content in the appropriate place:

| Content | Maintained in |
| --- | --- |
| Product capabilities and use cases | [Overview](../overview.en.md) |
| User-visible behavior and cross-layer agreements | [Contracts](../contracts/README.en.md) |
| Module relationships and execution | [Architecture](../architecture/README.en.md) |
| Setup, interaction, and troubleshooting | [Integration](../guides/integration.en.md) and [debugging](../guides/debugging.en.md) |
| Check commands and platform records | [Verification](verification.en.md) |
| Reusable reasoning and tradeoffs | [Design lessons](design-lessons.en.md) |

Maintain each rule in its owning topic and update both languages together. Use Git history for completed plans and progress logs, keeping the main documentation focused on current behavior and active work.

## Check before and after a change

Before editing, trace the real call path and find tests that cover the affected user behavior. Afterwards, check the complete operation: a navigation journey includes queueing a request, displaying a page, cleaning up departing content, and navigating again. For failure paths, inspect displayed content, input/focus, and the request's final state together.

Run formatting, compilation, and tests suited to the change. For documentation, run link and release-content checks using the [verification guide](verification.en.md). Report commands, features, source state, and environment, along with checks still to run. Shared-workspace and commit rules are in [AGENTS.md](../../AGENTS.md).
