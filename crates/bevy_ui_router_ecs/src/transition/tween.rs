//! Optional `bevy_tween` coordination for application-defined route effects.
//!
//! The application supplies one preparation callback and ordinary tween
//! combinators. This adapter owns their controllers and route completion timing;
//! it does not choose poses, easing, durations, or overlapping layout.

use std::{any::TypeId, time::Duration};

use bevy_app::{App, Plugin, PostUpdate};
use bevy_ecs::{
    component::{Component, Mutable},
    schedule::IntoScheduleConfigs,
    system::{Commands, SystemState},
    world::World,
};
use bevy_tween::{
    bevy_time_runner::{SkipTimeRunner, TimeContext, TimeRunnerDebugPlugin, time_runner_system},
    combinator::{AnimationBuilderExt, AnimationCommands},
    interpolation::{EaseClosure, EaseKind, sample_interpolations_system},
    tween::component_dyn_tween_system_with_time_context,
};
use bevy_ui::{Node, UiSystems, UiTransform, ZIndex};

use super::{
    RouteTransitionContext, RouteTransitionDriver, RouteTransitionId, TransitionCancelReason,
    TransitionPreparation,
};

mod runtime;
use runtime::{RouteTweenClock, TweenPhase, TweenSession, TweenSessions, TweenSet};

#[cfg(all(test, feature = "bsn"))]
mod tests;

type ComponentInstaller = (TypeId, fn(&mut App));

/// Installs route tween coordination and boxed component interpolators.
///
/// Requires Bevy's `TimePlugin` (included in `DefaultPlugins`). The default
/// components are [`UiTransform`], [`Node`], and [`ZIndex`]. Register another
/// application component with [`Self::with_component`], then use
/// `TargetComponent::with_closure` or a boxed interpolator in its tween.
/// This adapter installs `EaseKind`/`EaseClosure` sampling and boxed component
/// tween systems. Typed interpolators, asset/resource tweens, and tween events
/// are not registered here; applications needing those integrations can use
/// the underlying [`RouteTransitionDriver`] directly.
///
/// Controllers use a separate time context, so an application's existing
/// `DefaultTweenPlugins` may coexist without advancing them twice. This plugin
/// sends `RequestRedraw` while presented tracks run when a window message
/// resource exists; it does not install a window or change event-loop settings.
///
/// Install this plugin before `DefaultTweenPlugins` so the dependency's debug
/// plugin recognizes the route time context. An already installed custom debug
/// configuration is preserved; version 0.7 has no incremental context registration
/// and may report a missing-context warning for these otherwise valid runners.
pub struct RouteTweenPlugin {
    components: Vec<ComponentInstaller>,
}

impl Default for RouteTweenPlugin {
    fn default() -> Self {
        Self {
            components: Vec::new(),
        }
        .with_component::<UiTransform>()
        .with_component::<Node>()
        .with_component::<ZIndex>()
    }
}

impl RouteTweenPlugin {
    /// Enables boxed tweens for an additional application component.
    ///
    /// Component registration is idempotent within this plugin configuration.
    pub fn with_component<C: Component<Mutability = Mutable>>(mut self) -> Self {
        let id = TypeId::of::<C>();
        if !self
            .components
            .iter()
            .any(|(registered, _)| *registered == id)
        {
            self.components.push((id, install_component::<C>));
        }
        self
    }
}

impl Plugin for RouteTweenPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<TimeRunnerDebugPlugin>() {
            app.world_mut()
                .register_component::<TimeContext<RouteTweenClock>>();
            let mut debug = TimeRunnerDebugPlugin::default();
            debug.add_time_step::<RouteTweenClock>();
            app.add_plugins(debug);
        }
        app.init_resource::<TweenSessions>()
            .configure_sets(
                PostUpdate,
                (
                    TweenSet::Advance,
                    TweenSet::Progress,
                    TweenSet::Sample,
                    TweenSet::Apply,
                    TweenSet::Settle,
                )
                    .chain()
                    .before(UiSystems::Prepare),
            )
            .add_systems(
                PostUpdate,
                (
                    runtime::advance.in_set(TweenSet::Advance),
                    time_runner_system::<RouteTweenClock>.in_set(TweenSet::Progress),
                    (
                        sample_interpolations_system::<EaseKind, RouteTweenClock>,
                        sample_interpolations_system::<EaseClosure, RouteTweenClock>,
                    )
                        .in_set(TweenSet::Sample),
                    runtime::settle.in_set(TweenSet::Settle),
                ),
            );
        for (_, install) in &self.components {
            install(app);
        }
    }
}

fn install_component<C: Component<Mutability = Mutable>>(app: &mut App) {
    app.add_systems(
        PostUpdate,
        component_dyn_tween_system_with_time_context::<C, RouteTweenClock>()
            .in_set(TweenSet::Apply),
    );
}

type TweenCombinator =
    Box<dyn FnOnce(&mut AnimationCommands, &mut Duration) + Send + Sync + 'static>;

/// Enter and exit timelines created during one transition's preparation.
///
/// Pass `bevy_tween` combinators such as `tween`, `sequence`, and `parallel`.
/// Use `EaseKind`/`EaseClosure` with boxed component interpolators registered
/// through [`RouteTweenPlugin`]; this is not the dependency's full plugin group.
/// Each call creates a finite controller outside the routed hierarchy; target
/// components explicitly through the supplied transition context. All writers
/// for leaving surfaces must belong to exit tracks. Tracks may not reparent
/// their controller entities or install writers outside their controller tree.
#[derive(Default)]
pub struct RouteTweenTracks {
    entering: Vec<TweenCombinator>,
    leaving: Vec<TweenCombinator>,
}

impl RouteTweenTracks {
    /// Runs a timeline after publication. Its completion finishes the enter side.
    pub fn enter(
        &mut self,
        animation: impl FnOnce(&mut AnimationCommands, &mut Duration) + Send + Sync + 'static,
    ) -> &mut Self {
        self.entering.push(Box::new(animation));
        self
    }

    /// Holds leaving content until every exit timeline has applied its last pose.
    pub fn exit(
        &mut self,
        animation: impl FnOnce(&mut AnimationCommands, &mut Duration) + Send + Sync + 'static,
    ) -> &mut Self {
        self.leaving.push(Box::new(animation));
        self
    }
}

/// Turns one application preparation callback into a complete transition driver.
///
/// Install [`RouteTweenPlugin`] before using this driver. The callback sets
/// initial styles and adds enter/exit combinators; it does not manage session
/// resources, timers, cancellation, or release/finish commands. Adding no tracks
/// skips animation. Concrete effects and any application resource side effects
/// remain application-owned.
pub struct RouteTweenDriver<F> {
    prepare: F,
}

impl<F> RouteTweenDriver<F>
where
    F: FnMut(&mut World, &RouteTransitionContext, &mut RouteTweenTracks) + Send + Sync + 'static,
{
    pub fn new(prepare: F) -> Self {
        Self { prepare }
    }
}

impl<F> RouteTransitionDriver for RouteTweenDriver<F>
where
    F: FnMut(&mut World, &RouteTransitionContext, &mut RouteTweenTracks) + Send + Sync + 'static,
{
    fn prepare(
        &mut self,
        world: &mut World,
        context: &RouteTransitionContext,
    ) -> TransitionPreparation {
        assert!(
            world.contains_resource::<TweenSessions>(),
            "install RouteTweenPlugin before using RouteTweenDriver"
        );
        let mut tracks = RouteTweenTracks::default();
        (self.prepare)(world, context, &mut tracks);
        if tracks.entering.is_empty() && tracks.leaving.is_empty() {
            return TransitionPreparation::Skip;
        }
        let hold_exit = !tracks.leaving.is_empty();
        let mut session = TweenSession::new(context.id(), hold_exit);
        let mut commands = SystemState::<Commands>::new(world);
        {
            let mut queue = commands
                .get_mut(world)
                .expect("Commands is available in the preparation World");
            for (phase, tracks) in [
                (TweenPhase::Enter, tracks.entering),
                (TweenPhase::Exit, tracks.leaving),
            ] {
                for track in tracks {
                    let root = queue
                        .animation_in_time_context::<RouteTweenClock>()
                        .disabled(true)
                        .insert(track)
                        .id();
                    session.track(phase, root);
                }
            }
        }
        commands.apply(world);
        // The dependency tags only Added<TimeRunner> children. Tag immediately
        // while preparation owns the complete controller tree, so a timeline
        // that waits several updates for publication has no startup ambiguity.
        for entity in session.roots() {
            let children = world
                .get::<bevy_ecs::hierarchy::Children>(entity)
                .map(|children| children.to_vec())
                .unwrap_or_default();
            for child in children {
                world
                    .entity_mut(child)
                    .insert(TimeContext::<RouteTweenClock>::default());
            }
            world.entity_mut(entity).insert(SkipTimeRunner);
        }
        world.resource_mut::<TweenSessions>().0.push(session);
        TransitionPreparation::Run { hold_exit }
    }

    fn presented(&mut self, world: &mut World, id: &RouteTransitionId) {
        runtime::start(world, id);
    }

    fn cancelled(&mut self, world: &mut World, id: &RouteTransitionId, _: TransitionCancelReason) {
        runtime::stop(world, id);
    }
}
