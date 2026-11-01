use std::sync::{Arc, Mutex};

use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::AssetPlugin;
use bevy_ecs::{prelude::*, template::FromTemplate};
use bevy_scene::{ResolvedScene, SceneFunction, ScenePlugin};
use bevy_time::{Real, Time};
use bevy_tween::{
    DefaultTweenPlugins, bevy_time_runner::TimeRunner, combinator::tween, tween::IntoTarget,
};
use bevy_ui::{Display, Val2};
use bevy_ui_router_bsn::DefaultOutlet;
use bevy_ui_router_core::prelude::*;
use bevy_ui_router_view::{RouteViewDescriptor, RouteViewRegistry, ViewDriverKey};
use bevy_window::RequestRedraw;

use super::*;
use crate::{
    AppBsnExt, AppRouterExt, RequestLookup, RouterConfig, RouterHandle, RouterHostRegistration,
    RouterNavigation, RouterSnapshots,
};

#[derive(Clone, Component, Default)]
struct Page;

type Samples = Arc<Mutex<Vec<(usize, bool, f32)>>>;

#[derive(Default, Resource)]
struct Prepared {
    contexts: Vec<RouteTransitionContext>,
    samples: Samples,
}

struct Fixture {
    app: App,
    router: RouterHandle,
}

impl Fixture {
    fn new() -> Self {
        let mut app = App::new();
        app.add_plugins((
            TaskPoolPlugin::default(),
            AssetPlugin::default(),
            ScenePlugin,
            RouteTweenPlugin::default(),
            DefaultTweenPlugins::default(),
        ));
        // Controlled clock input, with the real production schedules unchanged.
        app.init_resource::<Time<Real>>()
            .init_resource::<Time>()
            .init_resource::<Prepared>()
            .add_message::<RequestRedraw>();
        let mut views = RouteViewRegistry::new();
        for name in ["root", "a", "b", "c"] {
            let driver = app
                .register_bsn_view(ViewDriverKey::new(name).unwrap(), move |_| {
                    Ok(SceneFunction(move |_, scene: &mut ResolvedScene| {
                        scene.push_template(Node::default());
                        scene.push_template(Page);
                        if name == "root" {
                            scene.push_template(
                                <DefaultOutlet as FromTemplate>::Template::default(),
                            );
                        }
                    }))
                })
                .unwrap();
            views
                .insert(
                    RouteViewKey::new(name).unwrap(),
                    RouteViewDescriptor::new(driver),
                )
                .unwrap();
        }
        let mut tree = RouteTree::builder()
            .root_with_options(
                RoutePattern::pathless(),
                RouteNodeOptions::new().with_view(RouteViewKey::new("root").unwrap()),
            )
            .unwrap();
        let root = tree.root_id().unwrap();
        for name in ["a", "b", "c"] {
            tree.route_with_options(
                root,
                RoutePattern::path(name).unwrap(),
                RouteNodeOptions::new()
                    .with_view(RouteViewKey::new(name).unwrap())
                    .with_keep_alive(true),
            )
            .unwrap();
        }
        let router = app
            .add_ui_router(
                RouterConfig::new(
                    tree.build().unwrap(),
                    views,
                    RouteViewKey::new("a").unwrap(),
                )
                .with_keep_alive_capacity(3)
                .with_transition_driver(RouteTweenDriver::new(effect)),
            )
            .unwrap();
        app.world_mut()
            .spawn((Node::default(), RouterHostRegistration::new(router)));
        app.update();
        Self { app, router }
    }

    fn push(&mut self, path: &str) -> NavigationRequestId {
        self.app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(self.router, RouteLocation::new(path))
            .unwrap()
    }

    fn update(&mut self, seconds: f32) {
        self.app
            .world_mut()
            .resource_mut::<Time<Real>>()
            .advance_by(Duration::from_secs_f32(seconds));
        // A different default clock proves ordinary TweenPlugins cannot tick
        // this adapter's controller timelines.
        self.app
            .world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs(10));
        self.app.update();
    }

    fn published(&mut self, path: &str) -> RouteTransitionContext {
        for _ in 0..32 {
            self.update(2.0);
            if self
                .app
                .world()
                .resource::<RouterSnapshots>()
                .get(self.router)
                .and_then(|snapshot| snapshot.active().location())
                .is_some_and(|location| location.path() == path)
            {
                return self
                    .app
                    .world()
                    .resource::<Prepared>()
                    .contexts
                    .last()
                    .unwrap()
                    .clone();
            }
        }
        panic!("{path} did not publish");
    }

    fn settled(&mut self, request: NavigationRequestId) {
        for _ in 0..32 {
            self.update(0.0);
            if matches!(
                self.app.world().resource::<RouterSnapshots>().request(self.router, request),
                RequestLookup::Found(snapshot) if snapshot.status() == &RequestStatus::Succeeded
            ) {
                return;
            }
        }
        panic!("request did not settle");
    }
}

fn effect(world: &mut World, context: &RouteTransitionContext, tracks: &mut RouteTweenTracks) {
    let (number, samples) = {
        let mut prepared = world.resource_mut::<Prepared>();
        prepared.contexts.push(context.clone());
        (prepared.contexts.len(), prepared.samples.clone())
    };
    for (entering, targets) in [(true, context.entering()), (false, context.leaving())] {
        for target in targets.iter().filter(|target| target.is_outermost()) {
            let surface = target.surface();
            world.get_mut::<UiTransform>(surface).unwrap().translation =
                Val2::px(if entering { 100.0 } else { 0.0 }, 0.0);
            let samples = samples.clone();
            let animation = tween(
                Duration::from_secs_f32(if entering { 1.0 } else { 0.25 }),
                EaseKind::Linear,
                surface
                    .into_target()
                    .with_closure(move |transform: &mut UiTransform, value, _| {
                        samples.lock().unwrap().push((number, entering, value));
                        let x = if entering { 1.0 - value } else { -value };
                        transform.translation = Val2::px(x * 100.0, 0.0);
                    }),
            );
            if entering {
                tracks.enter(animation);
            } else {
                tracks.exit(animation);
            }
        }
    }
}

#[test]
fn publication_starts_at_zero_and_exit_releases_after_its_final_tween_write() {
    let mut fixture = Fixture::new();
    let a = fixture.push("/a");
    let first = fixture.published("/a");
    let surface = first
        .entering()
        .iter()
        .find(|target| target.is_outermost())
        .unwrap()
        .surface();
    assert_eq!(
        fixture
            .app
            .world()
            .get::<UiTransform>(surface)
            .unwrap()
            .translation,
        Val2::px(100.0, 0.0)
    );
    fixture
        .app
        .world_mut()
        .resource_mut::<Messages<RequestRedraw>>()
        .clear();
    fixture.update(1.0);
    assert!(
        !fixture
            .app
            .world()
            .resource::<Messages<RequestRedraw>>()
            .is_empty(),
        "the final tween frame wakes Router to consume completion"
    );
    fixture.settled(a);
    assert!(fixture.app.world().resource::<TweenSessions>().0.is_empty());

    let b = fixture.push("/b");
    let second = fixture.published("/b");
    let entering = second.entering()[0].surface();
    let leaving = second.leaving()[0].surface();
    assert_eq!(
        fixture
            .app
            .world()
            .get::<UiTransform>(entering)
            .unwrap()
            .translation,
        Val2::px(100.0, 0.0)
    );
    fixture.update(0.25);
    assert_eq!(
        fixture
            .app
            .world()
            .get::<UiTransform>(leaving)
            .unwrap()
            .translation,
        Val2::px(-100.0, 0.0)
    );
    assert_eq!(
        fixture.app.world().get::<Node>(leaving).unwrap().display,
        Display::Flex
    );
    fixture.settled(b);
    assert_eq!(
        fixture.app.world().get::<Node>(leaving).unwrap().display,
        Display::None
    );
    assert_eq!(
        fixture
            .app
            .world()
            .get::<UiTransform>(entering)
            .unwrap()
            .translation,
        Val2::px(75.0, 0.0)
    );
    assert!(
        !fixture.app.world().resource::<TweenSessions>().0.is_empty(),
        "enter animation can outlive request terminal"
    );
    fixture.update(0.75);
    fixture.update(0.0);
    assert!(fixture.app.world().resource::<TweenSessions>().0.is_empty());
    let mut controllers = fixture.app.world_mut().query::<&TimeRunner>();
    assert_eq!(controllers.iter(fixture.app.world()).count(), 0);
}

#[test]
fn superseding_navigation_stops_old_tracks_before_the_next_tween_schedule() {
    let mut fixture = Fixture::new();
    let a = fixture.push("/a");
    fixture.published("/a");
    fixture.update(1.0);
    fixture.settled(a);
    let b = fixture.push("/b");
    fixture.published("/b");
    fixture.update(0.1);
    let samples = fixture.app.world().resource::<Prepared>().samples.clone();
    let before = samples
        .lock()
        .unwrap()
        .iter()
        .filter(|(number, _, _)| *number == 2)
        .count();

    let c = fixture.push("/c");
    fixture.published("/c");
    fixture.update(1.0);
    fixture.settled(b);
    fixture.settled(c);
    let after = samples
        .lock()
        .unwrap()
        .iter()
        .filter(|(number, _, _)| *number == 2)
        .count();
    assert_eq!(
        before, after,
        "cancelled session must have no remaining component writers"
    );
    assert!(fixture.app.world().resource::<TweenSessions>().0.is_empty());
}
