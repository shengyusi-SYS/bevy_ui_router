# Input and focus contract

[中文](input-focus.zh-CN.md) | [English](input-focus.md)

Router gives published pages input access and revokes gestures and focus when content leaves. Application input handlers, page background work, and window Back actions use the interfaces below. See [runtime behavior](../architecture/runtime.md#bevy-scheduling-and-input) for ordering and [lifecycle](lifecycle.md) for publication.

## Input eligibility, pointer, and focus

Standard UI pointer input uses native picking. Target-addressed keyboard/gamepad handlers read `RouterInputGate` after `RouterInputSet::Ready` in `PreUpdate`. Submit dynamic topology changes before `Refresh`; added and removed content updates eligibility at the next Ready boundary. The gate stores the permissions snapshot for that boundary.

Native UI eligibility uses `ComputedNode` and authored visibility, allowing controls whose geometry is calculated without a `Node`, such as `ScrollbarThumb`. Scope, publication, and inactive revocation rules remain the same.

Lifecycle switches revoke access in the same publication/isolation call, including active→retained. Hidden, pending, leaving, failed, retained, and residual content lose integrated input and active focus; other valid targets keep working.

Pointer revocation clears the target's current/previous hover, press/click/drag, and ancestor caches while preserving other targets. Inactive owned scopes are filtered again after real hover generation and before dispatch, cancelling gestures that began before departure.

Applications integrate target filtering for native `TabNavigationPlugin`/`InputDispatchPlugin`. Applications also arrange their custom dispatch, raw device reads, and direct World changes.

A pending Scene's `InputFocus::set`/AutoFocus is stored as a candidate. The last valid candidate is applied during publication when its original owner publishes successfully and the target still belongs to that source's focus-eligible active content.

Later valid set/clear operations take precedence, including setting the same value or clearing an already empty focus. A later pending candidate also replaces the earlier one. Failure, staleness, cancellation, hiding, or leaving the source discards the candidate. Without an explicit intent, the existing focus policy remains; reactivated retained content also waits for a new intent. An unused mutable borrow does not count as a focus operation.

Focus handling preserves the application's queue, cause, and deferred-event ordering. Pending/failed targets that were never active are excluded from deferred FocusGained/Lost dispatch.

## Page activity queries

`RouteActivity::is_active(Entity)` helps applications decide whether to run a page timer, query, or background task. It checks the published active assignment, the nearest registered boundary's exact claim, host registration incarnation, and current containment. Cached, pending, leaving, failed, quarantined, and unrelated application-host targets return false.

`RouteInputs::current(Entity)` uses the same active-frame resolution and returns a borrowed `RouteInputRef`, or None without valid active ownership. Both require Core to be Ready and the exact frame to remain in its logical active set. An invalid or inactive nearest registered boundary rejects the query immediately. Valid content in other Routers resolves through its own scope.

Ordinary page descendants and non-interactive entities can be queried without extra routing components. Both SystemParams read Router metadata and relevant hierarchy, so they work alongside application `Query<(Entity, &mut T)>` parameters. Run after `RouterScheduleSet::PublishSnapshots` to see this update's publication. Input references live only for the SystemParam borrow; copy the plain data you need before World mutation or await. See [navigation](navigation.md#matching-and-location-updates) for field meanings.

Activity, visibility, and input eligibility are separate checks. Applications use activity to control their systems and background work; input handlers still use the gate. Systems, observers, and other application logic in hidden retained content continue according to the application's scheduling.

Sources: [input](../../crates/bevy_ui_router_ecs/src/input.rs), [native focus](../../crates/bevy_ui_router_ecs/src/native_focus.rs), [activity](../../crates/bevy_ui_router_ecs/src/activity.rs), [current inputs](../../crates/bevy_ui_router_ecs/src/route_inputs.rs), and [scheduling](../../crates/bevy_ui_router_ecs/src/plugin.rs).

## Window-level Back input

The facade or Platform `desktop-back` / `android-back` features provide `DesktopBackInputPlugin` / `AndroidBackInputPlugin`. Both emit `PlatformBackRequested { window }` in `PlatformBackInputSet`, after Bevy `InputSystems` in `PreUpdate`.

| Source | Input converted to a Back request |
| --- | --- |
| Desktop mouse | `MouseButtonInput` with `Back + Pressed` |
| Traditional Android key | `KeyboardInput` with logical `BrowserBack` / `GoBack`, `Pressed`, and `repeat == false` |

Each source preserves its original window and message order. Multiple sources follow actual scheduling order. Released, other keys, and repeated Android keys are ignored; original input remains available to other readers.

Use one application handler to choose the window→RouterHandle mapping, local UI priority, and the action when history is empty. It then calls `RouterNavigation::back(router)` and handles the enqueue result. Back is a window-level intent and can fire at any pointer location. Types such as `BackPolicy` help express policy; the handler carries out the decision.

Place the handler in the same `PreUpdate` with `.after(PlatformBackInputSet)` so accepted requests enter the following `Update` navigation pass. Each `MessageReader` has its own cursor, so local handling and Router dispatch belong in one decision path. Save the installed handle and explicitly associate it with a window. `DesktopBackRequested` / `DesktopBackInputSet` are compatibility names for the shared types.

Plugins register raw and Back message channels idempotently. Device input comes from the application's Bevy backend; tests can inject standard messages. Features enable only the needed mouse or keyboard input. Applications select Router ECS, Scene, window, and platform runtime plugins separately.

The Android adapter uses traditional key messages; modern callbacks and predictive Back gestures are future capabilities. Bevy messages have no Android cancellation flag, so applications explicitly choose their exit behavior when history is empty. See the [system Back guide](../guides/platform-back.md#android-traditional-back) for the Android manifest compatibility setting.
