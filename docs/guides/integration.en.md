# Application integration

[中文](integration.md) | [English](integration.en.md)

The `bsn` feature of `bevy_ui_router` connects Bevy Scenes to a route tree. Router handles page changes, shared layouts, and navigation history. This guide starts with runnable examples, then covers retained state, scrolling, input, and platform Back controls.

## Run the examples

From the repository root:

```sh
cargo run --locked -p bevy_ui_router --features bsn --example bsn_quick_start
cargo run --locked -p bevy_ui_router --features bsn --example bsn_nested
```

| Example | Contents | Runtime |
| --- | --- | --- |
| [bsn_quick_start.rs](../../crates/bevy_ui_router/examples/bsn_quick_start.rs) | Register Scenes, create a Router, navigate, and read results | Headless; exits when complete |
| [bsn_nested.rs](../../crates/bevy_ui_router/examples/bsn_nested.rs) | Nested layouts, query updates, application async content, and scroll restoration | Headless with real UI layout; exits when complete |
| [windowed](../../examples/windowed/) | Clickable pages, retained state, focus, and error recovery | Interactive window |

Start the windowed example:

```sh
cargo run --locked --manifest-path examples/windowed/Cargo.toml
```

The windowed application configures its window, camera, font, and styling. Its Linux build uses X11 and needs the X11/xkbcommon development libraries. Running it requires a display and graphics driver. See [window debugging](debugging.en.md) for automated input and screenshots.

## Use Router in a separate Cargo application

Create `router-demo` next to the `bevy_ui_router` checkout. Copy [the quick start](../../crates/bevy_ui_router/examples/bsn_quick_start.rs) into `router-demo/src/main.rs`, then add this `Cargo.toml`. Adjust `path` to match your directory layout.

```toml
[package]
name = "router-demo"
version = "0.1.0"
edition = "2024"

[dependencies]
bevy_ui_router = { path = "../bevy_ui_router/crates/bevy_ui_router", default-features = false, features = ["bsn"] }
bevy_app = { version = "0.19", default-features = false }
bevy_asset = { version = "0.19", default-features = false }
bevy_ecs = { version = "0.19", default-features = false }
bevy_scene = { version = "0.19", default-features = false }
bevy_ui = { version = "0.19", default-features = false }
```

Run `cargo run` in `router-demo`. The project uses Rust edition 2024 and Bevy `0.19`; the repository lockfile selects `0.19.1`. Toolchain details and check commands are in the [verification guide](../development/verification.en.md).

For the nested example, also add its layout, camera, and input dependencies:

```toml
bevy_input_focus = { version = "0.19", default-features = false }
bevy_internal = { version = "0.19", default-features = false, features = ["bevy_window", "bevy_camera", "bevy_ui", "bevy_picking"] }
```

## Set up the application

1. Install Bevy plugins. A minimal headless application uses `TaskPoolPlugin`, `AssetPlugin`, and `ScenePlugin`. Visible UI also needs window, camera, UI/render, and input plugins.
2. Register Scene factories with `AppBsnExt::register_bsn_view`, keeping each returned `ViewDriverKey`. A factory receives `&RouteViewInput` and returns a fresh Scene or `BsnBuildError`.
3. Create a `RouteViewRegistry`. Use `RouteViewDescriptor` to bind each view key to its driver key.
4. Declare routes, normal views, and error views with `RouteTree::builder`. Prepare a Router fallback view.
5. Call `add_ui_router(RouterConfig::new(routes, views, fallback))` and keep the `RouterHandle`.
6. Add `RouterHostRegistration::new(handle)` to an application-created `Node`. Advance one update and use `RouterSnapshots` to check that the host is available. The [lifecycle contract](../contracts/lifecycle.en.md) describes host topology.
7. Navigate through `RouterNavigation::{push,replace,back}`. Keep the request ID and read its result through `RouterSnapshots`.

`register_bsn_view` and `add_ui_router` install the shared Router systems. You can also install `BevyUiRouterPlugin` beforehand. Driver keys are unique: duplicate registration returns an error immediately, and Router installation checks all view/driver bindings.

Scenes created in code still use Bevy's Asset/Scene runtime. If installation returns `MissingSceneRuntime`, install `AssetPlugin` and `ScenePlugin` first so that `AssetServer` and `Assets<ScenePatch>` are available.

## Nest content and observe navigation

Add `DefaultOutlet` to the internal `Node` where a layout Scene should display its child page. Each layout currently uses one default outlet. Switching between sibling pages reuses their common parent layout, outlet, and component state. The application controls the host layout; Router manages its routed content. Place application controls that need an independent lifetime outside that content boundary. See the [Scene contract](../contracts/scene.en.md) for mounting rules.

Pass the three parts of a location separately:

```rust
let location = RouteLocation::new("/items/42")
    .with_search("sort=name")
    .with_fragment("details");
```

`new` receives the path. Search and fragment retain their supplied strings; the caller chooses whether those strings contain `?` or `#`. Matching and update behavior are covered by the [navigation contract](../contracts/navigation.en.md).

`Ok(request_id)` means the request was queued. A windowed application advances through `App::run`. A headless program can follow the quick start and wait for `RequestStatus::Succeeded` or `Failed` within a bounded update loop. `RouterSnapshots::request(handle, id)` returns `NotRetained` when a completed record has been evicted, or `Unknown` when no matching record exists or the ID belongs to another Router.

Schedule systems that need the current update's complete snapshots after `RouterScheduleSet::PublishSnapshots` in `Update`. After a page becomes active, its request can remain in `PostCommitCleanup` while departing content is removed. Read the displayed state and request completion state separately.

## Retain layouts and pages

Enable retention for a layout or page with a normal parent route, and configure the Router's inactive capacity:

```rust
let page = RouteNodeOptions::new()
    .with_view(page_view)
    .with_keep_alive(true);
routes.route_with_options(parent, RoutePattern::path(":id")?, page)?;
let config = RouterConfig::new(routes.build()?, views, fallback)
    .with_keep_alive_capacity(2);
let router = app.add_ui_router(config)?;
```

| Setting | Purpose |
| --- | --- |
| `with_keep_alive(true)` | Retain this route's Entity and local state after departure. Off by default. Common layouts still reuse normally within the active branch. |
| `with_keep_alive_capacity(n)` | Maximum number of inactive retained instances. Defaults to 0. Parent layouts and child pages count separately. |
| `with_preserve_on_search(true)` | Allow reuse across query changes. Off by default. Active and retained instances follow the same rule, including parameter and identity checks. |

The first two settings enable retention together. Each retained instance is bound to its original normal parent instance and outlet/slot. The parent may be active or retained. A root mounted directly under the host has no such parent, so it is cleaned up on departure. Removing or evicting a parent also removes retained instances that depend on it.

Configure parent and child routes separately. When only a layout is retained, returning to it selects or creates the child from the target route. To reuse several levels across a query change, enable `preserve_on_search` at each level. Router rebuilds from the first node with a different identity or a disabled setting. If search and fragment change together, the deepest frame's fragment rebuild rule also applies. See [navigation](../contracts/navigation.en.md) for capacity, LRU, and history-entry behavior.

## Load content and reactivate pages

Pages can load data with ordinary Bevy systems and tasks:

1. The factory creates the page structure. Its `RouteViewInput` describes the route at creation time.
2. Once the page is active, read the current location with `RouteInputs::current` and start or update tasks using the application's data identifiers.
3. Tasks return data or handles. Application systems check the matching page and data version before changing the World. The application decides whether to continue or cancel work after departure.
4. Reactivating a retained page uses the original Entity and component state, keeping the result of the existing factory call. Page systems can use `RouteActivity` to decide whether to run this update.
5. When content is ready for scroll restoration, mark the current restoration intent ready.

The [nested example](../../crates/bevy_ui_router/examples/bsn_nested.rs) combines these steps. Activity, current input, and input eligibility queries are explained below.

## Restore scroll positions

Add `RouteScroll::new("body")`, Bevy's `ScrollPosition`, and a `Node` with scroll overflow to the scroll container. Import `RouteScroll` from the facade/prelude. Use a different stable key for each container within a page, such as `"body"` and `"sidebar"`. A child page may reuse its parent layout's key.

The default policy restores once during the first normal layout after a unique container binding is found. Choose an explicit policy when restoration should wait for data or dynamic content:

```rust
let marker = RouteScroll::new("body")
    .with_restore_policy(RouteScrollRestorePolicy::Explicit);
```

BSN updates components through the official `PatchFromTemplate` mechanism; see the [nested example](../../crates/bevy_ui_router/examples/bsn_nested.rs). When the content is ready, find the current restoration intent and mark it ready:

```rust
fn allow_restore(
    restores: RouteScrollRestorations,
    mut commands: Commands,
    pages: Query<(Entity, &PageState)>,
) {
    for (scope, page) in &pages {
        if page.content_ready {
            if let Some(restore) = restores.pending(scope, "body") {
                commands.ready_route_scroll(restore.id());
            }
        }
    }
}
```

`PageState` is an application component. `scope` can be any content Entity within the current active frame, so the query works before the container exists. `restore.position()` is the saved two-dimensional position in logical pixels for that visit.

Cancel with `commands.cancel_route_scroll(restore.id())`. When handling user scrolling, use the container's `RouteScroll::key()` to find and cancel a waiting restoration. This also records the user's intention when the container is already at an edge and its numeric position stays unchanged. Ready triggers one restoration; later content growth follows ordinary layout. Each Back operation has its own restoration ID.

Order systems as follows:

| Work | Schedule position |
| --- | --- |
| Create UI structure and apply deferred writes | In `Update` or before `UiSystems::Prepare`, allowing camera and related propagation to finish |
| Send ready/cancel or change existing component values | Before `RouterScrollSet::Prepare` |
| Override the restoration position from application logic | After `RouterScrollSet::Prepare` and before Layout |
| Read the final clamped position | After normal Layout |

Install normal UI layout and provide a valid camera/render target. Read navigation and layout results after their respective stages. The [navigation contract](../contracts/navigation.en.md) covers early and late containers, retained pages, and history entries.

## Connect input and focus

`RouteActivity` helps decide whether page systems should run. `RouteInputs` provides current route information. Both are available from the facade/prelude and can share a system with the application's mutable component queries:

```rust
fn update_pages(activity: RouteActivity, mut pages: Query<(Entity, &mut PageState)>) {
    for (entity, mut page) in &mut pages {
        if activity.is_active(entity) {
            page.update();
        }
    }
}

fn update_route_labels(inputs: RouteInputs, mut labels: Query<(Entity, &mut Text)>) {
    for (entity, mut text) in &mut labels {
        if let Some(input) = inputs.current(entity)
            && text.0 != input.location().path()
        {
            text.0 = input.location().path().to_owned();
        }
    }
}

app.add_systems(
    Update,
    (update_pages, update_route_labels).after(RouterScheduleSet::PublishSnapshots),
);
```

The application defines `PageState` and its `update` method. Query current inputs each update so reused layouts and reactivated pages see the new location. Write text only when it changes to reduce `Changed<Text>` notifications.

`RouteInputRef::params()` returns the owning frame's own parameters; `location()` includes the full path/search/fragment. Error frames provide `failure()` and `origin()`, while a normal prefix has `failure() == None`. The fallback has `route() == None` and empty parameters. Inactive content or content with an invalid owner returns `None`. Copy values needed across World changes or `await`. See the [activity contract](../contracts/input-focus.en.md) and [standalone consumer](../../crates/bevy_ui_router/tests/consumers/bsn.rs).

Use the dedicated gate for targeted input:

- **Pointer:** use Bevy UI picking. Router isolates inactive content and revokes a departing target's previous gesture state. Schedule content creation, hiding, and movement before input refresh.
- **Keyboard/gamepad:** run targeted systems after `RouterInputSet::Ready` in `PreUpdate`, and check `RouterInputGate::allows_keyboard` or `allows_gamepad` for each target. Global shortcuts follow the application's own rules.
- **Focus:** install `InputFocusPlugin` and let the application choose initial focus. Router maintains managed targets' focus eligibility during publication and departure. When integrating `TabNavigationPlugin` / `InputDispatchPlugin`, connect the Router gate to target selection and dispatch. The windowed example uses an explicitly gated keyboard handler.

Follow [window debugging](debugging.en.md) to check interaction. Platform and device verification is recorded in the [verification guide](../development/verification.en.md).

## Desktop mouse Back

Enable the facade's `desktop-back` feature, for example `features = ["bsn", "desktop-back"]`. Import the adapter from `bevy_ui_router::prelude::*` or `platform::prelude::*`:

```rust
app.add_plugins(DesktopBackInputPlugin)
    .add_systems(PreUpdate, platform_back.after(PlatformBackInputSet));
```

Store the Router handle on the window Entity. Use one application handler to decide whether to close a local overlay or navigate:

```rust
#[derive(Component)]
struct WindowRouter(RouterHandle);

fn platform_back(
    mut requests: MessageReader<PlatformBackRequested>,
    windows: Query<&WindowRouter>,
    mut navigation: ResMut<RouterNavigation>,
) {
    for request in requests.read() {
        let Ok(target) = windows.get(request.window) else { continue };
        // Handle local overlays here before deciding to navigate.
        match navigation.back(target.0) {
            Ok(request_id) => { /* Save the ID and observe RouterSnapshots. */ }
            Err(error) => eprintln!("Back was not enqueued: {error:?}"),
        }
    }
}
```

`WindowRouter` is an application component. Attach it with `commands.entity(window).insert(WindowRouter(router))` to associate the input window with the Router's host. On `NoBackEntry`, the application chooses feedback or a host action such as exit. A queued Back follows the usual history, retention, scroll restoration, and cleanup flow.

Every `MessageReader` can read the message, so keep local UI priority and the navigation decision in the same handler. The [windowed example](../../examples/windowed/src/main.rs) has runnable wiring; the [Back journeys](../../crates/bevy_ui_router/tests/platform_back.rs) cover multiple windows and overlay priority.

Enabling `desktop-back` on its own uses Bevy app/ECS/input. The application supplies its window and input backend. See the [architecture overview](../architecture/README.en.md) for `platform` and default-facade dependencies.

## Android traditional Back

Enable `android-back` in an Android Bevy application, for example `features = ["bsn", "android-back"]`. Install the key source and reuse the application handler above:

```rust
app.add_plugins(AndroidBackInputPlugin)
    .add_systems(PreUpdate, platform_back.after(PlatformBackInputSet));
```

The adapter accepts the first press of logical `BrowserBack` / `GoBack`, skipping repeats and releases. The application configures Android windowing and packaging. On its own, this feature uses the std and keyboard features of Bevy app/ECS/input.

On Android 16 and later with `targetSdk >= 36`, traditional Back keys require a compatibility setting. Set it on the existing `<application>` or target `<activity>` in `AndroidManifest.xml`:

```xml
<application android:enableOnBackInvokedCallback="false">
    <!-- Existing application settings and activities. -->
</application>
```

This setting enables the traditional Back path and disables system predictive Back animations. See the [Android 16 migration guidance](https://developer.android.com/about/versions/16/behavior-changes-16#predictive-back). An activity's setting takes precedence over the application default; see [activity-level configuration](https://developer.android.com/guide/navigation/custom-back/predictive-back-gesture#opt-out-at-an-activity-level).

This adapter represents a key-press intent. Bevy's `KeyboardInput` supplies press/release information; Android cancellation flags, predictive progress, and modern Back callbacks need dedicated application integration. After ignoring an intent or receiving `NoBackEntry`, the same handler explicitly chooses the host's return behavior. See the [Android consumer](../../crates/bevy_ui_router/tests/consumers/android_back.rs) for minimal wiring and the [verification guide](../development/verification.en.md) for platform status.
