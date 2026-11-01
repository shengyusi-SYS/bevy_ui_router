# Scene and mounting contract

[中文](scene.zh-CN.md) | [English](scene.md)

This page covers factory registration, synchronous Scene creation, and mounting nested pages. See the [integration guide](../guides/integration.md) for setup and [lifecycle](lifecycle.md) for hosts and publication.

## Registration and runtime setup

The recommended entry point is `BsnRouter::new(root).fallback(factory)` with `AppBsnExt::add_bsn_router`. `page(path, factory)` declares a page or layout with a path, `layout(factory)` declares a pathless layout, and `index(factory)` declares a default child. `.children(...)` keeps nesting explicit; `.error_view(factory)` associates an error page directly with its node. Each declaration has one explicit root; no hidden parent is inserted.

Declarations compile to the existing Core route tree, View registry, and BSN factories. Construction and installation never call a factory. Pattern/tree errors retain the original `RouteError` and declaration location. Generated driver keys check the current registration namespace, allowing multiple Routers and manual registrations to coexist. Validation or installation failure leaves no factory registrations from the attempted batch; success uses the same runtime pipeline.

The application adds `RouterHostRegistration` to its own host. `.initial(location)` supplies a default destination; later navigation uses Commands or immediate `RouterNavigation`, with results observed through snapshots. See [navigation](navigation.md#requests-history-and-presentation) for enqueue and startup rules.

| Feature | Capability |
| --- | --- |
| `default` / `view` | Renderer-neutral view model with Bevy-free dependencies |
| `ecs` | Bevy ECS integration |
| `bsn` | Official Scene bridge, including `ecs` |

Advanced explicit integration can still build mappings through `register_bsn_view`, `RouteViewRegistry`, and `RouterConfig`. Driver keys share one registration namespace. Duplicate registration fails synchronously, preserves the original factory, and does not call it. `RouteViewRegistry::insert` also rejects duplicate view keys and preserves the descriptor. Installation validates normal views, error views, fallback, and driver bindings in node declaration order, checking a node's normal view first.

The current registration API is for setup; dynamic unregister/rebind is a future extension. Applications configure windows, cameras, fonts, rendering, and asset paths. Public entry points are in [App extension](../../crates/bevy_ui_router_ecs/src/app_ext.rs).

## Scene profile and creation ownership

The declaration interface and `AppBsnExt::register_bsn_view` accept the same synchronous factory that takes read-only `RouteViewInput` and returns a fresh Scene or `BsnBuildError`. Each fresh view effect calls the factory once and consumes its Scene once. Error presentations use their own factory call and identity; keep-alive hits reuse already applied entities.

Factory input includes requested location, matched route, route-local params, raw search, fragment, origin, and the original failure for error attempts. The factory builds declarations from this data. The ECS execution layer holds World, Commands, Entity, slot, frame/generation, and lifecycle authority.

Supported content includes code-composed native Scenes, standard `ChildOf` declaration graphs, component/bundle/fallible templates, named references inside the graph, and resource handles. Cached Scene asset inclusion, including loaded patches, returns an error before UI allocation. Dependencies that are not ready fail in this synchronous profile. Applications manage non-`ChildOf` output and entities that templates create outside the declared closure.

Synchronous Scenes also require `AssetServer` and `Assets<ScenePatch>`. A minimal setup can install `TaskPoolPlugin`, `AssetPlugin`, and `ScenePlugin`. Installing a Router with BSN bindings without these resources returns `MissingSceneRuntime`, preserved as `BsnRouterError::Install` by the declaration interface; removing them later produces an explicit failure before calling Scene execution.

Execution follows: official resolve → declaration preflight → exact Entity preallocation and registration → apply the original Scene with a private shell wrapper and reference seed. Original root/child names still resolve to their original content entities, while anonymous nodes use attempt-local unique references. Scene component values and declared Children order are preserved, with official relationship callbacks performing relationship writes.

Declaration entities are allocated early. The official declaration getter may add empty `Children` to a leaf and trigger its hook. Error reporting preserves the original source. Early registration covers apply failures before `ChildOf` is installed, and dependency handles remain held through actual apply/settlement completion.

Apply, adoption, and immediate failure isolation run in one exclusive call. Creation obligations cover partially built UI that is not yet attached. Isolation completes before returning to ordinary systems or the next input/presentation boundary, followed by safe cleanup. See [BSN preparation](../../crates/bevy_ui_router_ecs/src/bsn.rs) and the [native Scene executor](../../crates/bevy_ui_router_ecs/src/presentation/native_scene.rs).

## Frame content and default outlet

The hierarchy is application host → top slot → FrameShell → FrameContentTarget → Scene content. A nested page mounts its next frame through an outlet and child slot in the parent content.

Router controls shell layout and isolation. Scenes define ordinary `Node`, `Visibility`, `Pickable`, and other content-root values; lifecycle changes preserve these authored values.

A layout with a child route declares exactly one `DefaultOutlet` on its content root or an inner `Node`. A leaf may omit it. Missing, duplicate, non-container, or invalidly related outlets produce mount failure. Reusing a parent layout preserves its outlet anchor and child slot together; the marker declaration and actual binding are managed separately. Moving the marker is checked against the existing binding.

Before consuming a mount, ECS revalidates parent/frame generation, anchor, slot, and relationships. An invalid outlet on an entering parent is pending rejection. Losing the binding of a committed, preserved parent is authority loss. Each follows its corresponding failure path.

A frame's content traversal stops at child slots. Nested frames are managed by their own identity, role, and binding. See [lifecycle](lifecycle.md#cleanup-and-content-boundaries) for creation/cleanup obligations and [input and focus](input-focus.md) for pending focus.
