//! Application animation only: this module owns effects, clocks, and overlap.

use std::time::Duration;

use bevy_internal::{
    math::curve::{
        Curve,
        easing::{EaseFunction, EasingCurve},
    },
    prelude::*,
};
use bevy_ui_router::{core::NavigationKind, prelude::*};

#[derive(Clone, Copy, Debug, Default)]
pub(super) enum Effect {
    #[default]
    Slide,
    ScaleRotate,
}

#[derive(Resource)]
pub(super) struct AnimationOptions {
    pub effect: Effect,
    pub hold_exit: bool,
}

impl Default for AnimationOptions {
    fn default() -> Self {
        Self {
            effect: Effect::Slide,
            hold_exit: true,
        }
    }
}

#[derive(Resource, Default)]
pub(super) struct AnimationState {
    current: Option<Animation>,
    pub last_event: String,
    pub cancellations: u32,
}

impl AnimationState {
    pub(super) fn is_running(&self) -> bool {
        self.current
            .as_ref()
            .is_some_and(|animation| animation.last_sample.is_some())
    }
}

struct Surface {
    entity: Entity,
    baseline: UiTransform,
}

struct Animation {
    id: RouteTransitionId,
    effect: Effect,
    direction: f32,
    entering: Vec<Surface>,
    leaving: Vec<Surface>,
    enter_timer: Timer,
    exit_timer: Timer,
    last_sample: Option<Duration>,
}

pub(super) struct DemoTransitionDriver;

impl RouteTransitionDriver for DemoTransitionDriver {
    fn prepare(
        &mut self,
        world: &mut World,
        context: &RouteTransitionContext,
    ) -> TransitionPreparation {
        let options = world.resource::<AnimationOptions>();
        let effect = options.effect;
        let hold_exit = options.hold_exit && !context.leaving().is_empty();
        let direction = if context.navigation() == NavigationKind::Back {
            -1.0
        } else {
            1.0
        };
        let mut animation = Animation {
            id: context.id(),
            effect,
            direction,
            entering: Vec::new(),
            leaving: Vec::new(),
            enter_timer: Timer::from_seconds(0.62, TimerMode::Once),
            exit_timer: Timer::from_seconds(0.38, TimerMode::Once),
            last_sample: None,
        };
        // Animate the outermost changed frames, so a nested route is not moved
        // twice. Preserved layout frames are never part of this write set.
        for (entering, targets) in [(true, context.entering()), (false, context.leaving())] {
            if !entering && !hold_exit {
                continue;
            }
            for target in targets.iter().filter(|target| target.is_outermost()) {
                let entity = target.surface();
                let baseline = *world
                    .get::<UiTransform>(entity)
                    .expect("authorized surface");
                // This example chooses overlapping full-size pages. The router
                // authorizes these existing component values and restores them;
                // it does not choose this layout or change the application host.
                let mut node = world.get_mut::<Node>(entity).expect("authorized surface");
                node.position_type = PositionType::Absolute;
                node.left = Val::Px(0.0);
                node.top = Val::Px(0.0);
                node.width = Val::Percent(100.0);
                node.height = Val::Percent(100.0);
                *world.get_mut::<ZIndex>(entity).expect("authorized surface") =
                    ZIndex(if entering { 1 } else { 0 });
                *world.get_mut::<UiTransform>(entity).unwrap() =
                    pose(effect, direction, baseline, entering, 0.0);
                let surface = Surface { entity, baseline };
                if entering {
                    animation.entering.push(surface);
                } else {
                    animation.leaving.push(surface);
                }
            }
        }
        let mut state = world.resource_mut::<AnimationState>();
        state.current = Some(animation);
        state.last_event = format!("Prepared {effect:?}; hold exit = {hold_exit}");
        TransitionPreparation::Run { hold_exit }
    }

    fn presented(&mut self, world: &mut World, id: &RouteTransitionId) {
        let now = world.resource::<Time<Real>>().elapsed();
        let mut state = world.resource_mut::<AnimationState>();
        if let Some(animation) = state
            .current
            .as_mut()
            .filter(|animation| animation.id == *id)
        {
            animation.last_sample = Some(now);
            state.last_event = "Presented: application timers started".into();
        }
    }

    fn cancelled(
        &mut self,
        world: &mut World,
        id: &RouteTransitionId,
        reason: TransitionCancelReason,
    ) {
        let mut state = world.resource_mut::<AnimationState>();
        if state
            .current
            .as_ref()
            .is_some_and(|animation| animation.id == *id)
        {
            // Removing the one writer synchronously is essential: the router
            // may restore and reuse these shells immediately after this returns.
            state.current = None;
            state.cancellations += 1;
            state.last_event = format!("Stopped old animation: {reason:?}");
        }
    }
}

fn pose(
    effect: Effect,
    direction: f32,
    baseline: UiTransform,
    entering: bool,
    progress: f32,
) -> UiTransform {
    let mut transform = baseline;
    let distance = if entering { 1.0 - progress } else { -progress };
    match effect {
        Effect::Slide => {
            // Router shells start with an identity transform; keep the captured
            // baseline for the remaining components and the final entering pose.
            transform.translation = Val2::percent(direction * distance * 100.0, 0.0);
        }
        Effect::ScaleRotate => {
            transform.scale *= if entering {
                0.72 + 0.28 * progress
            } else {
                1.0 - 0.14 * progress
            };
            transform.rotation *= Rot2::degrees(-8.0 * direction * distance);
        }
    }
    if entering && progress >= 1.0 {
        baseline
    } else {
        transform
    }
}

pub(super) fn animate(
    time: Res<Time<Real>>,
    mut state: ResMut<AnimationState>,
    mut transforms: Query<&mut UiTransform>,
    mut commands: Commands,
    mut redraw: MessageWriter<bevy_internal::window::RequestRedraw>,
) {
    let Some(mut animation) = state.current.take() else {
        return;
    };
    let Some(last_sample) = animation.last_sample else {
        state.current = Some(animation);
        return;
    };
    // The example owns redraw policy as well as its clock. Reactive winit
    // windows must keep drawing while this application animation is active.
    redraw.write(bevy_internal::window::RequestRedraw);
    let curve = EasingCurve::new(0.0, 1.0, EaseFunction::CubicOut);
    // The first sample shares presented's timestamp, so a slow preceding
    // update or an idle event loop cannot consume the new animation's duration.
    let now = time.elapsed();
    let elapsed = now.saturating_sub(last_sample);
    animation.last_sample = Some(now);
    animation.enter_timer.tick(elapsed);
    animation.exit_timer.tick(elapsed);
    for (entering, targets, progress) in [
        (true, &animation.entering, animation.enter_timer.fraction()),
        (false, &animation.leaving, animation.exit_timer.fraction()),
    ] {
        for surface in targets {
            if let Ok(mut transform) = transforms.get_mut(surface.entity) {
                *transform = pose(
                    animation.effect,
                    animation.direction,
                    surface.baseline,
                    entering,
                    curve.sample_clamped(progress),
                );
            }
        }
    }
    if animation.exit_timer.is_finished() && !animation.leaving.is_empty() {
        // Remove leaving writers before requesting release. The entering timer
        // deliberately continues after the navigation's exit obligation ends.
        animation.leaving.clear();
        commands.release_route_transition_exit(animation.id.clone());
        state.last_event = "Exit released; entering animation continues".into();
    }
    if animation.enter_timer.is_finished() {
        commands.finish_route_transition(animation.id);
        state.last_event = "Finished; all application writers stopped".into();
    } else {
        state.current = Some(animation);
    }
}
