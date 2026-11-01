# Navigation and window debugging

[中文](debugging.zh-CN.md) | [English](debugging.md)

Start with the symptoms below to check routing state, then inspect window layout and input as needed.

## Diagnose navigation first

| Symptom | Check first |
| --- | --- |
| Installation returns `MissingSceneRuntime` | Install `AssetPlugin` and `ScenePlugin` before adding Router; the minimal example also installs `TaskPoolPlugin` |
| The initial page never appears | Check `.initial(...)`, a host with the correct `RouterHostRegistration`, and ongoing App updates; `AwaitingHost` means Router is still waiting for a host |
| The snapshot has the expected location but the screen is empty | Check application host/page sizes, visibility, camera/render target, and UI renderer |
| Navigation returns no request ID | Commands return no synchronous ID; enqueue failures log a warning by default. Use `RouterNavigation` for the immediate enqueue result, then snapshots for execution results |
| Navigation fails | Inspect the request's `RequestStatus::Failed`, `trace()`, and `diagnostics()` for the failure source; a displayed error page also exposes `active().failure()` |
| The page changed but the request remains in `PostCommitCleanup` | The page and history have changed, but exit animation or cleanup is still pending; inspect `cleanup()` and whether exit animation has finished |
| Back cannot run | `NoBackEntry` means there is no earlier successful entry; the application chooses feedback or a host exit action |
| Page state is recreated | Check retention opt-in, capacity, whether the original parent layout still exists, and changes to path parameters, query strings, or fragments; see [retention](pages.md#retain-page-state) |
| Scroll is not restored | First confirm a successful Back and a saved position for that visit, then inspect `RouteScrollRestorations::pending`, container keys, layout/camera, and ready/cancel state; see [scroll restoration](pages.md#restore-scroll-positions) |

Read the current update's result after `RouterScheduleSet::PublishSnapshots`. Call this helper on demand from a diagnostic action, error handler, or state change rather than printing the same records every frame:

```rust
use bevy_ui_router::prelude::*;

fn inspect_router(snapshots: &RouterSnapshots, router: RouterHandle) {
    let Some(snapshot) = snapshots.get(router) else { return };
    eprintln!("availability={:?}, location={:?}, phase={:?}, cleanup={:?}",
        snapshot.availability(), snapshot.active().location(),
        snapshot.pending_phase(), snapshot.cleanup());
    if let Some(failure) = snapshot.active().failure() {
        eprintln!("active error: {failure:?}");
    }
    for event in snapshot.diagnostics().events() {
        eprintln!("{:?}: {:?}", event.kind(), event.redacted_message());
    }
}
```

With a request ID returned by `RouterNavigation`, use `snapshots.request(router, id)`: `Found` exposes `status()`, `trace()`, and `diagnostics()`; `NotRetained` means the completed record was evicted; `Unknown` means this Router has no matching request. `running_request()` only identifies a running request and becomes None after it terminates.

## Start and inspect a window

The [windowed example](../../examples/windowed/src/main.rs) exposes a Bevy Remote Protocol (BRP) endpoint for sending input, inspecting layout and state, and saving rendered screenshots. Input is handled inside the application, keeping the system pointer and keyboard focus in place.

From the repository root, enter the windowed example directory and start it:

```sh
cd examples/windowed
cargo run --features ui-debug
```

In another terminal, also enter `examples/windowed`, then run the Python client:

```sh
python3 ui_debug.py snapshot
python3 ui_debug.py find "Layout counter"
python3 ui_debug.py capture router-ui.png
```

The Python client uses only the standard library. The debug service runs only in debug builds, at `127.0.0.1:15703` by default.

<details>
<summary>snapshot field reference</summary>

| Field | Meaning |
| --- | --- |
| `windows`, `scale_factor` | Window Entity and display scale |
| `center_physical`, `size_physical` | Actual node position and size in physical pixels |
| `router_active` | Whether the content belongs to the active page |
| `pointer_allowed`, `keyboard_allowed` | Current input eligibility |
| `route_location`, `route_search`, `route_fragment` | Separate raw path, search, and fragment strings |
| `route_id` | The owning frame's route; null for the fallback |
| `scroll_requested`, `scroll_actual` | Requested position and the position clamped by layout |
| `pending_restore` | Waiting position for `list` in the current scope, available even before its container exists |
| `focus` | The focused Entity |

</details>

Inactive content has null route fields. `find` filters nodes by name or displayed text. Select `router_active=true` when locating the current page, because retained page Entities still exist.

Clicks use **logical coordinates** within the window. Divide the target's `center_physical` by the window's `scale_factor` before passing it to `click`. For example, if the resulting logical position is `(100, 120)`:

```sh
python3 ui_debug.py click 100 120
python3 ui_debug.py key Digit2
python3 ui_debug.py key Space
```

The client waits for application frames between press and release. Screenshots are returned as PNG through `router/capture` and `router/capture_status`. Use `--url` to select an endpoint; run `python3 ui_debug.py --help` for all options.

## Check user behavior

| Action | Expected result |
| --- | --- |
| Overview / Settings, or `1` / `2` | Push the page and update location and history in the status bar |
| Replace (`R`), then Back (`B` or the mouse Back button) | Replace updates the current entry; Back returns to the previous successful entry |
| Increase Layout counter and page count, then switch between Overview and Settings | Keep the original layout/page Entities and counts; inactive pages leave layout and input |
| Increase Layout counter, visit Outside (`4`), then open a different child page | Keep layout A and its count, select the child from the new route, and display the new path in the toolbar |
| Scroll a list, leave the page, then go Back | Restore that history entry's position; repeated visits to a page keep separate positions |
| Delayed (`3`) → Load (`L`) → scroll → Outside → Back | Create a new Delayed page, with a restoration intent available before the list is created |
| After returning, use Ready (`G`), then Load | Restore on the first normal layout; loading first and marking ready later also works |
| After returning, use Cancel (`C`), then Load, or scroll while waiting for ready | Cancel the waiting restoration and keep the new list's initial position or the user's new position |
| Click page counter to focus it, then press Space | Increase the active page's count; departure clears its focus, and clicking the new page gives it focus |
| Hold a page button, switch with `1`/`2`, then release | Revoke the departing page's gesture while current content and other valid targets remain interactive |
| Fail (`E`), then Overview | Show the Build error fallback, retain history on failure, then recover through navigation |

The example has a retained capacity of 3. Layout A, Overview, and Settings enable retention; Delayed and Outside are cleaned up on departure. The fallback replaces the route layout and clears old retained content, so recovery creates new layout and page instances. Load demonstrates delayed application content; Ready/Cancel use public scroll commands.

The terminal and status bar show request state, target Entity, history cursor, and focus. The page's keyboard handler checks input eligibility after `RouterInputSet::Ready`.

## Scroll and hold gestures

Lists use real `MouseWheel` messages. Move the cursor into `overview-scroll`, `settings-scroll`, or the loaded `delayed-scroll` region, then send an event through `router/input`:

```json
{
  "MouseWheel": {
    "window": 123,
    "unit": "Pixel",
    "x": 0.0,
    "y": -120.0,
    "phase": "Moved"
  }
}
```

Replace the illustrative `123` with the window Entity from the snapshot. Negative `y` scrolls down. Send it with `python3 ui_debug.py rpc router/input '<JSON>'`. The example's scroll observer changes Bevy `ScrollPosition`; Router's marker stores positions per visit.

For a held gesture, send `MouseButtonInput` twice with the same window and `button: "Left"`, first with `state: "Pressed"`, then with `state: "Released"`. Navigate and wait for frame advancement between those calls to check departure cancellation. Use `click` for ordinary clicks. See `Bridge.event` and `Bridge.click` in the [client](../../examples/windowed/ui_debug.py) for message encoding.

## Troubleshoot

| Symptom | What to check |
| --- | --- |
| Client cannot connect | Start a debug build with `--features ui-debug`, keep it running, and use `127.0.0.1:15703` |
| Click lands in the wrong place | Locate an active node in a fresh snapshot and convert physical coordinates using the window scale |
| Back does not restore a position | Confirm Back succeeded and the visit has a saved position, then inspect `router_active`, `pending_restore`, and both scroll fields |
| Explicit restoration keeps waiting | Send Ready after content is available; check that the key is unique and the container has `ScrollPosition` and scroll overflow |
| `pending_restore` disappeared | Check whether restoration finished, the user scrolled, the application canceled, or the binding became invalid |
| Layout exists but the screenshot is blank | Check the window, camera/render target, and renderer; advance frames and capture again |

Duplicate keys or missing valid layout/camera produce a terminal warning with the first affected Router, frame, generation, and key. Each Router reports each warning category once. Restoration behavior is described in the [navigation contract](../contracts/navigation.md).

See the [performance guide](../performance.md) for measurement and the [verification guide](../development/verification.md) for platform checks and the complete test entry points.
