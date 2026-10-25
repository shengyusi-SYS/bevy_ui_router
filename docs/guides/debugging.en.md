# Window debugging

[中文](debugging.md) | [English](debugging.en.md)

The [windowed example](../../examples/windowed/src/main.rs) exposes a Bevy Remote Protocol (BRP) endpoint for sending input, inspecting layout and state, and saving rendered screenshots. Input is handled inside the application, keeping the system pointer and keyboard focus in place.

## Start and inspect

From the repository root:

```sh
cargo run --locked --manifest-path examples/windowed/Cargo.toml --features ui-debug
```

In another terminal, run the Python client from the same directory:

```sh
python3 examples/windowed/ui_debug.py snapshot
python3 examples/windowed/ui_debug.py find "Layout counter"
python3 examples/windowed/ui_debug.py capture router-ui.png
```

Running the example requires a display and graphics driver. The Python client uses the standard library. In a debug build with the feature enabled, `ui-debug` listens on `127.0.0.1:15703` and keeps the window updating to process requests. Release builds retain their normal runtime behavior. The debug window starts without requesting focus.

`snapshot` includes:

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

Inactive content has null route fields. `find` filters nodes by name or displayed text. Select `router_active=true` when locating the current page, because retained page Entities still exist.

Clicks use **logical coordinates** within the window. Divide the target's `center_physical` by the window's `scale_factor` before passing it to `click`. For example, if the resulting logical position is `(100, 120)`:

```sh
python3 examples/windowed/ui_debug.py click 100 120
python3 examples/windowed/ui_debug.py key Digit2 --logical '{"Character":"2"}'
python3 examples/windowed/ui_debug.py key Space
```

The client waits for application frames between press and release. Screenshots are returned as PNG through `router/capture` and `router/capture_status`. Use `--url` to select an endpoint; run `python3 examples/windowed/ui_debug.py --help` for all options.

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

The terminal and status bar show request state, target Entity, history cursor, and focus. The page's keyboard handler checks the gate after `RouterInputSet::Ready`.

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

Replace the illustrative `123` with the window Entity from the snapshot. Negative `y` scrolls down. Send it with `python3 examples/windowed/ui_debug.py rpc router/input '<JSON>'`. The example's scroll observer changes Bevy `ScrollPosition`; Router's marker stores positions per visit.

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

Duplicate keys or missing valid layout/camera produce a terminal warning with the first affected Router, frame, generation, and key. Each Router reports each warning category once. Restoration behavior is described in the [navigation contract](../contracts/navigation.en.md).

Debug input travels through Bevy `WindowEvent` and typed messages, and screenshots come from the renderer. To check system events, physical devices, or native platform behavior, run the same interactions in that environment and record its configuration. Polling, continuous updates, and screenshots add runtime work; use the dedicated commands in the [performance guide](../performance.en.md) for measurement. The [verification guide](../development/verification.en.md) lists the complete test entry points.
