# Application-defined route transitions

[中文](transitions.md) | [English](transitions.en.md)

Router installs no transition driver by default. Applications choose effects, easing, durations, and overlap layout. The optional Tween adapter coordinates controller startup, cancellation, and completion; custom animation systems implement the `RouteTransitionDriver` described below. Import these types from `bevy_ui_router::prelude::*`.

The current presentation-surface integration targets native Bevy UI / BSN pages.

## Use the Tween adapter

Enable Router features `bsn,tween` and add `bevy_tween = { version = "0.13", default-features = false }`. Install `RouteTweenPlugin::default()`; the application still provides `TimePlugin` (included in `DefaultPlugins`). Run the complete example:

```sh
cargo run --locked --manifest-path examples/windowed/Cargo.toml --features tween --bin tween_transitions
```

The [window entry point](../../examples/windowed/src/bin/tween_transitions.rs) uses unified declarations, Commands, and retained counters. Its [application effects](../../examples/windowed/src/bin/tween_transitions/effects.rs) provide slide, scale/rotation, and enter-only modes without application sessions, timers, or completion callbacks. Buttons display the current effect and exit-animation mode; changes apply to the next navigation.

Configure `.transition_driver(RouteTweenDriver::new(prepare_effect))` on `BsnRouter`. The application supplies one preparation callback, for example:

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

`enter` and `exit` accept finite timelines built with the dependency's `tween`, `sequence`, or `parallel` combinators. Adding an exit track holds the outgoing page. The adapter releases it after all exit writers complete and finishes the session after all tracks complete. No tracks means no transition. Every writer targeting a leaving surface belongs to an exit track.

The adapter starts at zero after route publication using `Time<Real>`. It applies interpolation before UI Prepare and removes controllers after the final write. Cancellation stops writers synchronously; successive navigation follows the original queue. Reactive windows receive redraw requests during animation and when the final completion request is submitted.

The adapter supports `EaseKind` / `EaseClosure` and boxed component interpolation, with `UiTransform`, `Node`, and `ZIndex` registered by default. Register other application-content components with `RouteTweenPlugin::default().with_component::<T>()`, using `with_closure` or a boxed interpolator. The adapter does not register typed interpolators, resource/asset tweens, or tween events; use a manual driver for those integrations. Controllers occupy independent entities and preserve existing surface-write boundaries; additional application resource side effects remain application-owned.

When also using `DefaultTweenPlugins`, install `RouteTweenPlugin` first so its private time context can be registered with the upstream debug plugin. If upstream debug was installed earlier, timing remains independent, but it may misreport the unknown context; the adapter preserves existing application debug configuration. Custom clocks or specialized completion policies also use the manual driver below.

## Run the manual driver example

From the repository root:

```sh
cargo run --locked --manifest-path examples/windowed/Cargo.toml --bin transitions
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

## Register a driver

Register one application driver per Router:

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
| `presented(world, id)` | Start animation after route publication. This does not mean the entering animation has finished. |
| `cancelled(world, id, reason)` | Stop every writer for this session and remove its application animation state before returning. |

Every `prepare` is followed by `presented` or `cancelled`; a presented session can also be cancelled later. `Skip` invokes `cancelled(..., Skipped)` so the application can discard preparation state. Other reasons are `PreparationAborted`, `Superseded`, and `AuthorityLost`. Cancelling a published animation does not undo committed routing or history.

`RouteTransitionContext` supplies an opaque `id()`, Router, request, `NavigationKind`, `from()` / `to()`, and `entering()`, `leaving()`, and `preserved()` frames. Each changed target exposes its exact `frame()`, authored `content()` root, and temporarily editable `surface()`. Whole-page effects usually select `is_outermost()` targets to avoid transforming both a parent frame and its nested child. Preserved frames provide context only.

## Surfaces and animation state

`surface()` is the Entity of an existing Router shell. During authorization, the application may change values in its existing `Node` layout fields except `display`, plus `UiTransform` and `ZIndex`. Router continues to own relationships, component membership, visibility, input, and identity. The application sets overlap layout in `prepare`; Router adds no animation wrapper and does not replace the application host.

Keep timers, animation components, or third-party animators in application resources or separate application Entities, explicitly targeting the surface. Do not attach animation components to Router shells. Applications can also animate their own `content()`; they are responsible for cleaning up those changes and other resource side effects.

`cancelled` must stop writers synchronously. A flag processed on the next frame is insufficient. Commands queued by the callback are flushed before Router continues. On finish or cancellation, Router restores shell baselines where exact ownership still holds, preserving its current `display` value. Keeping an Entity handle does not extend the writing permission.

The example stores its single animation writer in a resource and removes it immediately in `cancelled`. Its animation system runs in `Update` after `RouterScheduleSet::PublishSnapshots`, changing ordinary Bevy component values each frame.

The application also owns window redraw. Reactive winit does not keep updating merely because a timer exists; the example sends `RequestRedraw` while animation runs. Applications using on-demand updates must also arrange wakeups for other pending timer-driven work.

## Release exits and finish

`Run { hold_exit: true }` keeps outgoing pages visible while deferring their existing cleanup or retention disposition. They are already inactive and cannot receive routed input or retain focus. `Run { hold_exit: false }` leaves that disposition immediate, supporting initial entry and entry-only animation; do not keep writing leaving surfaces after returning.

Stop the relevant animation writers before issuing commands:

```rust
// Leaving writers have stopped; entering animation may continue.
commands.release_route_transition_exit(session_id.clone());

// All animation writers have stopped; also release any remaining exit.
commands.finish_route_transition(session_id);
```

`RouteTransitionId` identifies only this animation session. Duplicate or stale commands do not affect later sessions. The example releases when the exit animation ends, then finishes when the entering animation ends.

Active presentation, current inputs, and history change at route publication. A held exit can keep the request in `PostCommitCleanup`. Once the exit responsibility is settled, the request can become terminal while entering animation continues. Entry-only animation does not extend the request lifetime.

If a later request is waiting for an old exit responsibility, Router can cancel the old session and settle that exit before advancing the queue. This does not determine whether the next target reuses any surface. Equivalent navigation with no actual entering or leaving targets starts no new animation session; an existing entry-only session can continue.

The responsibility split follows [Vue Router transitions](https://router.vuejs.org/guide/advanced/transitions.html) and [Motion presence](https://motion.dev/docs/react-animate-presence): routing manages instance presence while applications compose effects, without importing a DOM model or animation runtime.
