use std::time::Duration;

use bevy_ecs::{
    entity::Entity,
    message::Messages,
    prelude::{Query, Res, ResMut, Resource, SystemSet, With},
    world::World,
};
use bevy_time::{Real, Time};
use bevy_tween::bevy_time_runner::{SkipTimeRunner, TimeContext, TimeRunner};
use bevy_window::RequestRedraw;

use super::super::{RouteTransitionId, request_release};

#[derive(Default)]
pub(super) struct RouteTweenClock;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, SystemSet)]
pub(super) enum TweenSet {
    Advance,
    Progress,
    Sample,
    Apply,
    Settle,
}

#[derive(Clone, Copy)]
pub(super) enum TweenPhase {
    Enter,
    Exit,
}

pub(super) struct TweenSession {
    id: RouteTransitionId,
    entering: Vec<Entity>,
    leaving: Vec<Entity>,
    last_sample: Option<Duration>,
    exit_pending: bool,
}

impl TweenSession {
    pub(super) fn new(id: RouteTransitionId, hold_exit: bool) -> Self {
        Self {
            id,
            entering: Vec::new(),
            leaving: Vec::new(),
            last_sample: None,
            exit_pending: hold_exit,
        }
    }

    pub(super) fn track(&mut self, phase: TweenPhase, entity: Entity) {
        match phase {
            TweenPhase::Enter => &mut self.entering,
            TweenPhase::Exit => &mut self.leaving,
        }
        .push(entity);
    }

    pub(super) fn roots(&self) -> impl Iterator<Item = Entity> + '_ {
        self.entering.iter().chain(&self.leaving).copied()
    }
}

#[derive(Default, Resource)]
pub(super) struct TweenSessions(pub(super) Vec<TweenSession>);

pub(super) fn start(world: &mut World, id: &RouteTransitionId) {
    let now = world.resource::<Time<Real>>().elapsed();
    let roots = {
        let mut sessions = world.resource_mut::<TweenSessions>();
        let Some(session) = sessions.0.iter_mut().find(|session| session.id == *id) else {
            return;
        };
        session.last_sample = Some(now);
        session.roots().collect::<Vec<_>>()
    };
    for entity in roots {
        if let Some(mut runner) = world.get_mut::<TimeRunner>(entity) {
            runner.set_paused(false);
        }
        if let Ok(mut root) = world.get_entity_mut(entity) {
            root.remove::<SkipTimeRunner>();
        }
    }
}

pub(super) fn stop(world: &mut World, id: &RouteTransitionId) {
    let session = {
        let mut sessions = world.resource_mut::<TweenSessions>();
        let Some(index) = sessions.0.iter().position(|session| session.id == *id) else {
            return;
        };
        sessions.0.swap_remove(index)
    };
    // Controller roots own their tween descendants through ordinary ChildOf.
    // Immediate removal stops writers before Router restores/reuses its surfaces.
    for root in session.roots() {
        let _ = world.despawn(root);
    }
}

pub(super) fn advance(
    time: Res<Time<Real>>,
    mut sessions: ResMut<TweenSessions>,
    mut runners: Query<&mut TimeRunner, With<TimeContext<RouteTweenClock>>>,
) {
    if sessions.0.is_empty() {
        return;
    }
    let now = time.elapsed();
    for session in &mut sessions.0 {
        let Some(previous) = session.last_sample else {
            continue;
        };
        // First sample uses presented's timestamp, so navigation time earlier
        // this update never consumes animation duration.
        let delta = now.saturating_sub(previous).as_secs_f32();
        session.last_sample = Some(now);
        for root in session.roots() {
            if let Ok(mut runner) = runners.get_mut(root) {
                runner.tick(delta);
            }
        }
    }
}

pub(super) fn settle(world: &mut World) {
    if world.resource::<TweenSessions>().0.is_empty() {
        return;
    }
    let mut sessions = std::mem::take(&mut world.resource_mut::<TweenSessions>().0);
    let mut running = false;
    let mut completion_requested = false;
    sessions.retain_mut(|session| {
        if session.last_sample.is_none() {
            return true;
        }
        // Progress and interpolation have run, including the final component
        // write. Despawn its writers before asking Router to release the exit.
        retire_finished(world, &mut session.entering);
        retire_finished(world, &mut session.leaving);
        if session.exit_pending && session.leaving.is_empty() {
            request_release(world, &session.id, false);
            session.exit_pending = false;
            completion_requested = true;
        }
        if session.entering.is_empty() && session.leaving.is_empty() {
            request_release(world, &session.id, true);
            completion_requested = true;
            false
        } else {
            running = true;
            true
        }
    });
    world.resource_mut::<TweenSessions>().0 = sessions;
    // Completion is requested after Update. Even the final animation frame
    // needs another update for Router to consume release/finish and restore
    // surfaces when the application uses a reactive event loop.
    if running || completion_requested {
        if let Some(mut redraw) = world.get_resource_mut::<Messages<RequestRedraw>>() {
            redraw.write(RequestRedraw);
        }
    }
}

fn retire_finished(world: &mut World, roots: &mut Vec<Entity>) {
    roots.retain(|root| match world.get::<TimeRunner>(*root) {
        Some(runner) if !runner.is_completed() => true,
        _ => {
            let _ = world.despawn(*root);
            false
        }
    });
}
