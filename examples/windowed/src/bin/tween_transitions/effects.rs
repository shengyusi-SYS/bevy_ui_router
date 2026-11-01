//! These visual choices belong to the example application, not Router.

use std::time::Duration;

use bevy_internal::prelude::*;
use bevy_tween::{combinator::tween, prelude::*};
use bevy_ui_router::prelude::*;

#[derive(Resource)]
pub(super) struct Effects {
    pub scale_rotate: bool,
    pub hold_exit: bool,
}

impl Default for Effects {
    fn default() -> Self {
        Self {
            scale_rotate: false,
            hold_exit: true,
        }
    }
}

pub(super) fn prepare_effect(
    world: &mut World,
    context: &RouteTransitionContext,
    tracks: &mut RouteTweenTracks,
) {
    let options = world.resource::<Effects>();
    let scale_rotate = options.scale_rotate;
    let hold_exit = options.hold_exit;
    let direction = if context.navigation() == NavigationKind::Back {
        -1.0
    } else {
        1.0
    };
    for (entering, targets) in [(true, context.entering()), (false, context.leaving())] {
        if !entering && !hold_exit {
            continue;
        }
        for target in targets.iter().filter(|target| target.is_outermost()) {
            let entity = target.surface();
            let baseline = *world.get::<UiTransform>(entity).unwrap();
            // This application chooses full-size overlapping pages. Effects
            // operate on existing surface values, never add shell components.
            let mut node = world.get_mut::<Node>(entity).unwrap();
            node.position_type = PositionType::Absolute;
            node.left = Val::Px(0.0);
            node.top = Val::Px(0.0);
            node.width = Val::Percent(100.0);
            node.height = Val::Percent(100.0);
            *world.get_mut::<ZIndex>(entity).unwrap() = ZIndex(if entering { 1 } else { 0 });

            let pose = move |progress: f32| {
                let mut pose = baseline;
                let distance = if entering { 1.0 - progress } else { -progress };
                if scale_rotate {
                    pose.scale *= if entering {
                        0.78 + 0.22 * progress
                    } else {
                        1.0 - 0.12 * progress
                    };
                    pose.rotation *= Rot2::degrees(-7.0 * direction * distance);
                } else {
                    pose.translation = Val2::percent(direction * distance * 100.0, 0.0);
                }
                pose
            };
            *world.get_mut::<UiTransform>(entity).unwrap() = pose(0.0);
            let timeline = tween(
                Duration::from_secs_f32(if entering { 0.55 } else { 0.32 }),
                EaseKind::CubicOut,
                entity
                    .into_target()
                    .with_closure(move |target: &mut UiTransform, progress, _| {
                        *target = pose(progress);
                    }),
            );
            if entering {
                tracks.enter(timeline);
            } else {
                tracks.exit(timeline);
            }
        }
    }
}
