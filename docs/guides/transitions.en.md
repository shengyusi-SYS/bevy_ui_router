# Add page transitions

English | [简体中文](transitions.md)

Animate page changes in an existing Bevy UI application. Router manages how long entering and leaving pages remain available; your application chooses effects, durations, and easing. Start with the Tween adapter below, or connect an existing animation system with the custom driver later on this page.

## 1. Add the dependency and plugin

Enable `tween` on your existing `bevy_ui_router` dependency: `features = ["bsn", "tween"]`. Then add:

```toml
[dependencies.bevy_tween]
version = "0.13"
default-features = false
```

Keep the tutorial's imports and install the plugin before `add_bsn_router`:

```rust
app.add_plugins(RouteTweenPlugin::default());
```

The tutorial's `DefaultPlugins` already includes the time plugin animation needs.

## 2. Connect the effect to your router

Add `.transition_driver(RouteTweenDriver::new(prepare_effect))` to your existing `BsnRouter` builder, then define `prepare_effect` below. It slides an entering page in from the right over 240 milliseconds.

`surface()` returns the outer animation node Router provides for the page. Here we change its existing `UiTransform`; `is_outermost()` avoids moving both a parent and its child.

```rust
use std::time::Duration;
use bevy_tween::{combinator::tween, prelude::*};

fn prepare_effect(
    world: &mut World,
    context: &RouteTransitionContext,
    tracks: &mut RouteTweenTracks,
) {
    for target in context.entering().iter().filter(|target| target.is_outermost()) {
        let surface = target.surface();
        world.get_mut::<UiTransform>(surface).unwrap().translation = Val2::px(24.0, 0.0);
        tracks.enter(tween(
            Duration::from_millis(240),
            EaseKind::CubicOut,
            surface.into_target().with_closure(|value: &mut UiTransform, progress, _| {
                value.translation = Val2::px(24.0 * (1.0 - progress), 0.0);
            }),
        ));
    }
}
```

Use `tracks.enter(...)` for entering animation and `tracks.exit(...)` for leaving animation. Both accept finite timelines built with `tween`, `sequence`, and `parallel`. Put all leaving-page animation in `exit`; Router waits for it to finish before hiding or cleaning up the page. Adding no tracks switches pages immediately.

The adapter handles timing, cancellation, completion, and redraw for on-demand windows. Animations may change the outer node's existing `UiTransform`, `ZIndex`, and `Node` layout fields except `display`. Router continues to manage its components, parent/child relationships, visibility, and input.

## 3. Run and adjust the example

From the repository root, enter the windowed example directory:

```sh
cd examples/windowed
cargo run --features tween --bin tween_transitions
```

Click page buttons to navigate. Click `Effect: …` to change effects and `Exit: …` to toggle exit animation; changes apply to the next navigation. Find effects in [effects.rs](../../examples/windowed/src/bin/tween_transitions/effects.rs) and complete setup in the [example entry point](../../examples/windowed/src/bin/tween_transitions.rs).

<details>
<summary>More interpolation types, custom plugins, and execution order</summary>

The adapter supports `EaseKind` / `EaseClosure` and boxed component interpolation, with `UiTransform`, `Node`, and `ZIndex` registered by default. Register other application-content components with `RouteTweenPlugin::default().with_component::<T>()`, using `with_closure` or a boxed interpolator. Typed interpolators, resource/asset tweens, and tween events need the custom driver below.

When also using `DefaultTweenPlugins`, install `RouteTweenPlugin` first so the upstream debug plugin recognizes the route animation's independent clock. Reversing that order preserves timing but may produce an unknown-time-context warning. With a custom Bevy plugin set, provide `TimePlugin`.

Animation starts at zero after the page switch using `Time<Real>`. Interpolation runs before UI Prepare, and controllers are removed after their final write. Cancellation stops writes synchronously. On-demand windows receive redraw requests during animation and when completion is submitted. Use the driver interface below for custom clocks or completion policies.

</details>

## Connect an existing animation system

Implement `RouteTransitionDriver` to connect your own animation system. To try the example without Tween, run from the `examples/windowed` directory:

```sh
cargo run --bin transitions
```

The [windowed example](../../examples/windowed/src/bin/transitions.rs) implements both a slide and a scale/rotation effect through the same driver interface. Buttons and keyboard shortcuts perform the same actions:

| Key | Action |
| --- | --- |
| `1` / `2` | Open the retained Notes / Gallery page |
| `3` | Open Fresh, which creates a new instance on return |
| `B` | Back |
| `E` | Change the effect for the next transition |
| `H` | Toggle holding the outgoing page; disabling it animates entry only |
| `R` | Send successive navigation requests and observe cancellation |

Click a page counter before navigating to compare retained state with fresh creation. The [animation implementation](../../examples/windowed/src/bin/transitions/animation.rs) uses Bevy's `Time<Real>`, `Timer`, `EasingCurve`, and `UiTransform`, with no additional animation runtime. See [application integration](integration.en.md#run-the-examples) for window requirements.

### Register the driver

Register the driver for your Router:

```rust
let declaration = BsnRouter::new(root)
    .fallback(error_page)
    .transition_driver(MyTransitionDriver);
let router = app.add_bsn_router(declaration)?;
```

`MyTransitionDriver` implements `RouteTransitionDriver`. Callbacks run synchronously and receive `&mut World` to access application state:

| Callback | Application responsibility |
| --- | --- |
| `prepare(world, context)` | Set initial styles and record animation state without starting elapsed-time animation. Return `Skip` or `Run { hold_exit }`. |
| `presented(world, id)` | Start animation after the page switch. |
| `cancelled(world, id, reason)` | Stop every writer for this session and remove its application animation state before returning. |

Every `prepare` is followed by `presented` or `cancelled`; a presented session can also be cancelled later. `Skip` invokes `cancelled(..., Skipped)` so the application can discard preparation state. Other reasons are `PreparationAborted`, `Superseded`, and `AuthorityLost`. Cancelling a published animation does not undo committed routing or history.

`RouteTransitionContext` supplies the animation’s `id()`, Router, navigation request, `NavigationKind`, and previous/next locations through `from()` / `to()`. Use `entering()` and `leaving()` for changed targets; shared pages in `preserved()` provide read-only context. A target’s `frame()` identifies the route instance, `content()` is the application content root, and `surface()` is the outer animation node. Whole-page effects usually select `is_outermost()` to avoid transforming both a parent and its child.

### Store animation state and handle cancellation

Set overlapping page layouts in `prepare`.

Keep timers, animation components, or third-party animators in application resources or separate entities, targeting the outer node. You can also animate your own `content()` directly.

`cancelled` must stop all writes for this animation before returning. Commands queued by the callback are flushed before Router continues. On finish or cancellation, Router restores the original values on outer nodes it still manages, preserving the current `display`. Stop modifying those nodes once the animation ends.

The example stores its single animation writer in a resource and removes it immediately in `cancelled`. Its animation system runs in `Update` after `RouterScheduleSet::PublishSnapshots`, changing ordinary Bevy component values each frame.

With on-demand winit updates, send `RequestRedraw` while animation runs to keep the window updating.

### Release pages after exit animation

`Run { hold_exit: true }` keeps outgoing pages visible while deferring cleanup or hiding for retention. They are already inactive and cannot receive routed input or retain focus. `Run { hold_exit: false }` does not wait for exit animation, supporting initial entry and entry-only animation; do not keep modifying the outgoing pages’ outer nodes after returning.

Stop the relevant animation writers before issuing commands:

```rust
// Leaving writers have stopped; entering animation may continue.
commands.release_route_transition_exit(session_id.clone());

// All animation writers have stopped; also release any remaining exit.
commands.finish_route_transition(session_id);
```

Use this animation’s `RouteTransitionId`: release the outgoing page when exit animation ends, then finish the transition when all animation ends.

<details>
<summary>Successive navigation and request completion</summary>

The active page, current parameters, and history change at the page switch, without waiting for animation. Holding an outgoing page can keep the request in `PostCommitCleanup`. Once cleanup or retention of the outgoing page completes, the request can finish while entering animation continues. Entry-only animation does not extend the request.

If a later request is waiting for an old exit animation, Router can cancel it and handle the outgoing page before advancing the queue. Navigation with no actual entering or leaving pages starts no new animation; an existing entry-only animation can continue.

</details>
