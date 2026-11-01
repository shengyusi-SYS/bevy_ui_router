# System Back

English | [简体中文](platform-back.md)

Once page navigation works, connect mouse Back or Android Back to the same operation.

## Desktop mouse Back

Add `desktop-back` to Router's features in `Cargo.toml`, for example `features = ["bsn", "desktop-back"]`. The code below uses `bevy::prelude::*` and `bevy_ui_router::prelude::*`.

First install the plugin and register the return handler in your existing App:

```rust
app.add_plugins(DesktopBackInputPlugin)
    .add_systems(PreUpdate, platform_back.after(PlatformBackInputSet));
```

Next, use an application component to associate Router with the window receiving input:

```rust
#[derive(Component)]
struct WindowRouter(RouterHandle);
```

After creating the window, call `commands.entity(window).insert(WindowRouter(router))`. Here, `window` is the window Entity and `router` is the handle returned by `add_bsn_router`.

Finally, add the handler. Decide whether to close a local overlay first; if the overlay handles this return, skip navigation:

```rust
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

`NoBackEntry` means there is no previous page; the application chooses feedback or exit. A successful Back uses the existing page retention and scroll restoration settings.

Make one decision for overlays and page navigation in the same handler, so separate message readers do not each perform a return. See the [windowed example](../../examples/windowed/src/main.rs) for complete wiring.

## Android traditional Back

Add `android-back` in an Android Bevy application, for example `features = ["bsn", "android-back"]`. Use `AndroidBackInputPlugin` and reuse the window binding and `platform_back` handler above:

```rust
app.add_plugins(AndroidBackInputPlugin)
    .add_systems(PreUpdate, platform_back.after(PlatformBackInputSet));
```

This plugin receives traditional Back key input.

On Android 16 and later with `targetSdk >= 36`, traditional Back keys require a compatibility setting. Set it on the existing `<application>` or target `<activity>` in `AndroidManifest.xml`:

```xml
<application android:enableOnBackInvokedCallback="false">
    <!-- Existing application settings and activities. -->
</application>
```

This setting enables the traditional Back path and disables system predictive Back animations. See the [Android 16 migration guidance](https://developer.android.com/about/versions/16/behavior-changes-16#predictive-back). An activity's setting takes precedence over the application default; see [activity-level configuration](https://developer.android.com/guide/navigation/custom-back/predictive-back-gesture#opt-out-at-an-activity-level).

Modern Back callbacks and predictive gestures need separate integration. Ignoring this key message does not automatically delegate to Android's default return behavior.
