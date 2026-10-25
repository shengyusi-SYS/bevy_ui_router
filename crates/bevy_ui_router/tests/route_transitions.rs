//! Public journeys: animation timing is application state; route ownership is not.
#![cfg(feature = "bsn")]

use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::AssetPlugin;
use bevy_camera::visibility::Visibility;
use bevy_ecs::{prelude::*, system::RunSystemOnce, template::FromTemplate};
use bevy_scene::{ResolvedScene, SceneFunction, ScenePlugin};
use bevy_ui::{Display, Node, PositionType, UiTransform, Val, Val2, ZIndex};
use bevy_ui_router::prelude::*;

#[derive(Component, Clone, Default)]
struct Page(&'static str);

#[derive(Resource, Default)]
struct Animation {
    current: Option<RouteTransitionContext>,
    prepared: usize,
    presented: usize,
    cancelled: Vec<TransitionCancelReason>,
    hold: bool,
    abort_host: Option<Entity>,
}

struct Driver;

impl RouteTransitionDriver for Driver {
    fn prepare(
        &mut self,
        world: &mut World,
        context: &RouteTransitionContext,
    ) -> TransitionPreparation {
        let leaving = context
            .leaving()
            .iter()
            .map(|target| target.content())
            .collect::<Vec<_>>();
        let from = context.from().cloned();
        world
            .run_system_once(move |activity: RouteActivity, inputs: RouteInputs| {
                for entity in &leaving {
                    assert!(
                        activity.is_active(*entity),
                        "original current remains readable in prepare"
                    );
                    assert_eq!(
                        inputs
                            .current(*entity)
                            .map(|input| input.location().clone()),
                        from
                    );
                }
            })
            .unwrap();
        if let Some(host) = world.resource_mut::<Animation>().abort_host.take() {
            world.entity_mut(host).remove::<RouterHostRegistration>();
        }
        for target in context.entering() {
            assert_eq!(
                world.get::<Node>(target.surface()).unwrap().display,
                Display::None
            );
            // This is application choreography, not a built-in router effect.
            let mut node = world.get_mut::<Node>(target.surface()).unwrap();
            node.position_type = PositionType::Absolute;
            node.left = Val::Px(80.0);
            world
                .get_mut::<UiTransform>(target.surface())
                .unwrap()
                .translation = Val2::px(12.0, 8.0);
            *world.get_mut::<ZIndex>(target.surface()).unwrap() = ZIndex(7);
        }
        let mut animation = world.resource_mut::<Animation>();
        animation.prepared += 1;
        animation.current = Some(context.clone());
        TransitionPreparation::Run {
            hold_exit: animation.hold,
        }
    }

    fn presented(&mut self, world: &mut World, id: &RouteTransitionId) {
        let context = world.resource::<Animation>().current.clone().unwrap();
        assert_eq!(&context.id(), id);
        for target in context.entering() {
            let node = world.get::<Node>(target.surface()).unwrap();
            assert_eq!(node.display, Display::Flex);
            assert_eq!(
                node.left,
                Val::Px(80.0),
                "initial application styles survive publication"
            );
            assert_eq!(
                world
                    .get::<UiTransform>(target.surface())
                    .unwrap()
                    .translation,
                Val2::px(12.0, 8.0)
            );
            assert_eq!(world.get::<ZIndex>(target.surface()), Some(&ZIndex(7)));
        }
        world.resource_mut::<Animation>().presented += 1;
    }

    fn cancelled(
        &mut self,
        world: &mut World,
        _: &RouteTransitionId,
        reason: TransitionCancelReason,
    ) {
        let mut animation = world.resource_mut::<Animation>();
        animation.cancelled.push(reason);
        animation.current = None;
    }
}

struct Fixture {
    app: App,
    router: RouterHandle,
    host: Entity,
}

impl Fixture {
    fn new(capacity: usize) -> Self {
        Self::create(capacity, false)
    }

    fn create(capacity: usize, nested: bool) -> Self {
        let mut app = App::new();
        app.add_plugins((
            TaskPoolPlugin::default(),
            AssetPlugin::default(),
            ScenePlugin,
            BevyUiRouterPlugin::default(),
        ));
        app.init_resource::<Animation>();
        let mut views = RouteViewRegistry::new();
        for name in ["layout", "a", "b", "fresh", "child"] {
            let driver = app.register_bsn_view(ViewDriverKey::new(name).unwrap(), move |_| {
                Ok(SceneFunction(move |_, root: &mut ResolvedScene| {
                    root.push_template(Node::default());
                    root.push_template(Page(name));
                    if name == "layout" || (nested && name == "a") {
                        root.push_template(<DefaultOutlet as FromTemplate>::Template::default());
                    }
                }))
            }).unwrap();
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
                RouteNodeOptions::new().with_view(RouteViewKey::new("layout").unwrap()),
            )
            .unwrap();
        let root = tree.root_id().unwrap();
        for name in ["a", "b", "fresh"] {
            let route = tree
                .route_with_options(
                    root,
                    RoutePattern::path(name).unwrap(),
                    RouteNodeOptions::new()
                        .with_view(RouteViewKey::new(name).unwrap())
                        .with_keep_alive(name != "fresh"),
                )
                .unwrap();
            if nested && name == "a" {
                tree.route_with_options(
                    route,
                    RoutePattern::path("child").unwrap(),
                    RouteNodeOptions::new().with_view(RouteViewKey::new("child").unwrap()),
                )
                .unwrap();
            }
        }
        let router = app
            .add_ui_router(
                RouterConfig::new(
                    tree.build().unwrap(),
                    views,
                    RouteViewKey::new("fresh").unwrap(),
                )
                .with_keep_alive_capacity(capacity)
                .with_transition_driver(Driver),
            )
            .unwrap();
        let host = app
            .world_mut()
            .spawn((Node::default(), RouterHostRegistration::new(router)))
            .id();
        app.update();
        Self { app, router, host }
    }

    fn push(&mut self, path: &str) -> NavigationRequestId {
        self.app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(self.router, RouteLocation::new(path))
            .unwrap()
    }

    fn wait_presented(&mut self, count: usize) {
        for _ in 0..24 {
            self.app.update();
            if self.app.world().resource::<Animation>().presented >= count {
                return;
            }
        }
        panic!(
            "presentation {count} missing: {:?}",
            self.app
                .world()
                .resource::<RouterSnapshots>()
                .get(self.router)
        );
    }

    fn settle(&mut self, id: NavigationRequestId) {
        for _ in 0..24 {
            self.app.update();
            if matches!(self.app.world().resource::<RouterSnapshots>().request(self.router,id), RequestLookup::Found(request) if request.status() == &RequestStatus::Succeeded)
            {
                return;
            }
        }
        panic!(
            "request did not succeed: {:?}",
            self.app
                .world()
                .resource::<RouterSnapshots>()
                .request(self.router, id)
        );
    }

    fn finish(&mut self, exit_only: bool) {
        let id = self
            .app
            .world()
            .resource::<Animation>()
            .current
            .as_ref()
            .unwrap()
            .id();
        self.app
            .world_mut()
            .run_system_once(move |mut commands: Commands| {
                if exit_only {
                    commands.release_route_transition_exit(id.clone());
                } else {
                    commands.finish_route_transition(id.clone());
                }
            })
            .unwrap();
        self.app.world_mut().flush();
    }

    fn page(&mut self, name: &str) -> Entity {
        self.app
            .world_mut()
            .query::<(Entity, &Page)>()
            .iter(self.app.world())
            .find_map(|(entity, page)| (page.0 == name).then_some(entity))
            .unwrap()
    }
}

#[test]
fn enter_only_finishes_navigation_independently_and_noop_keeps_its_session() {
    let mut f = Fixture::new(0);
    let first = f.push("/a");
    f.wait_presented(1);
    f.settle(first);
    let context = f
        .app
        .world()
        .resource::<Animation>()
        .current
        .clone()
        .unwrap();
    let a = f.page("a");
    assert!(
        f.app
            .world()
            .resource::<RouterInputGate>()
            .allows_pointer(a)
    );
    let again = f.push("/a");
    f.settle(again);
    assert_eq!(f.app.world().resource::<Animation>().prepared, 1);
    assert!(f.app.world().resource::<Animation>().cancelled.is_empty());
    f.finish(false);
    f.app.update();
    for target in context.entering() {
        let node = f.app.world().get::<Node>(target.surface()).unwrap();
        assert_eq!(node.display, Display::Flex);
        assert_eq!(node.left, Val::Auto);
        assert_eq!(
            f.app.world().get::<UiTransform>(target.surface()),
            Some(&UiTransform::default())
        );
        assert_eq!(
            f.app.world().get::<ZIndex>(target.surface()),
            Some(&ZIndex::default())
        );
    }
}

#[test]
fn held_retained_exit_is_visible_inert_and_back_reactivates_the_same_instance() {
    let mut f = Fixture::new(2);
    let first = f.push("/a");
    f.wait_presented(1);
    f.settle(first);
    f.finish(false);
    f.app.update();
    let a = f.page("a");
    f.app.world_mut().resource_mut::<Animation>().hold = true;
    let second = f.push("/b");
    f.wait_presented(2);
    let context = f
        .app
        .world()
        .resource::<Animation>()
        .current
        .clone()
        .unwrap();
    assert_eq!(context.navigation(), NavigationKind::Push);
    assert_eq!(context.leaving().len(), 1);
    let shell = context.leaving()[0].surface();
    assert_eq!(
        f.app.world().get::<Visibility>(shell),
        Some(&Visibility::Visible)
    );
    assert!(
        !f.app
            .world()
            .resource::<RouterInputGate>()
            .allows_pointer(a)
    );
    let b = f.page("b");
    assert!(
        f.app
            .world()
            .resource::<RouterInputGate>()
            .allows_pointer(b)
    );
    for _ in 0..3 {
        f.app.update();
    }
    assert_eq!(
        f.app.world().get::<Visibility>(shell),
        Some(&Visibility::Visible)
    );
    f.finish(true);
    f.settle(second);
    assert_eq!(
        f.app.world().get::<Visibility>(shell),
        Some(&Visibility::Hidden)
    );
    assert_eq!(f.page("a"), a);
    f.finish(false);
    f.app.update();
    let back = f
        .app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .back(f.router)
        .unwrap();
    f.wait_presented(3);
    assert_eq!(
        f.app
            .world()
            .resource::<Animation>()
            .current
            .as_ref()
            .unwrap()
            .navigation(),
        NavigationKind::Back
    );
    assert_eq!(f.page("a"), a);
    assert!(
        f.app
            .world()
            .resource::<RouterInputGate>()
            .allows_pointer(a)
    );
    f.finish(false);
    f.settle(back);
}

#[test]
fn fresh_exit_cleanup_uses_content_present_when_released_and_queued_navigation_progresses() {
    let mut f = Fixture::new(0);
    let first = f.push("/fresh");
    f.wait_presented(1);
    f.settle(first);
    f.finish(false);
    f.app.update();
    let outgoing = f.page("fresh");
    f.app.world_mut().resource_mut::<Animation>().hold = true;
    let second = f.push("/a");
    f.wait_presented(2);
    let added = f
        .app
        .world_mut()
        .spawn((Node::default(), ChildOf(outgoing)))
        .id();
    let moved = f
        .app
        .world_mut()
        .spawn((Node::default(), ChildOf(outgoing)))
        .id();
    f.app.world_mut().entity_mut(moved).remove::<ChildOf>();
    let third = f.push("/b");
    f.wait_presented(3);
    f.settle(second);
    assert!(f.app.world().get_entity(outgoing).is_err());
    assert!(f.app.world().get_entity(added).is_err());
    assert!(f.app.world().get_entity(moved).is_ok());
    assert!(
        f.app
            .world()
            .resource::<Animation>()
            .cancelled
            .contains(&TransitionCancelReason::Superseded)
    );
    f.finish(false);
    f.settle(third);
}

#[test]
fn retained_parent_and_disposable_child_stay_visible_until_the_same_exit_release() {
    let mut f = Fixture::create(2, true);
    let first = f.push("/a/child");
    f.wait_presented(1);
    f.settle(first);
    f.finish(false);
    f.app.update();
    let parent = f.page("a");
    let child = f.page("child");
    f.app.world_mut().resource_mut::<Animation>().hold = true;
    let second = f.push("/b");
    f.wait_presented(2);
    let context = f
        .app
        .world()
        .resource::<Animation>()
        .current
        .clone()
        .unwrap();
    assert_eq!(context.leaving().len(), 2);
    assert_eq!(
        context
            .leaving()
            .iter()
            .filter(|target| target.is_outermost())
            .count(),
        1
    );
    for target in context.leaving() {
        assert_eq!(
            f.app.world().get::<Visibility>(target.surface()),
            Some(&Visibility::Visible)
        );
        assert!(
            !f.app
                .world()
                .resource::<RouterInputGate>()
                .allows_pointer(target.content())
        );
    }
    f.finish(false);
    f.settle(second);
    assert!(f.app.world().get_entity(parent).is_ok());
    assert!(f.app.world().get_entity(child).is_err());
    let back = f
        .app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .back(f.router)
        .unwrap();
    f.wait_presented(3);
    assert_eq!(f.page("a"), parent);
    assert_ne!(f.page("child"), child);
    f.finish(false);
    f.settle(back);
}

#[test]
fn fresh_back_uses_the_same_transition_policy_without_a_cache() {
    let mut f = Fixture::new(0);
    let first = f.push("/a");
    f.wait_presented(1);
    f.settle(first);
    // Navigation must stop the previous entering writer even without an exit hold.
    let second = f.push("/b");
    f.wait_presented(2);
    f.settle(second);
    assert_eq!(
        f.app.world().resource::<Animation>().cancelled,
        vec![TransitionCancelReason::Superseded]
    );
    f.app.world_mut().resource_mut::<Animation>().hold = true;
    let back = f
        .app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .back(f.router)
        .unwrap();
    f.wait_presented(3);
    assert_eq!(
        f.app
            .world()
            .resource::<Animation>()
            .current
            .as_ref()
            .unwrap()
            .navigation(),
        NavigationKind::Back
    );
    f.finish(false);
    f.settle(back);
}

#[test]
fn authority_loss_cancels_entering_writers_before_existing_cleanup() {
    let mut f = Fixture::new(0);
    let first = f.push("/a");
    f.wait_presented(1);
    f.settle(first);
    f.app
        .world_mut()
        .entity_mut(f.host)
        .remove::<RouterHostRegistration>();
    for _ in 0..4 {
        f.app.update();
    }
    let animation = f.app.world().resource::<Animation>();
    assert!(animation.current.is_none());
    assert_eq!(
        animation.cancelled,
        vec![TransitionCancelReason::AuthorityLost]
    );
    assert!(
        f.app.world().get_entity(f.host).is_ok(),
        "application host is preserved"
    );
}

#[test]
fn rejected_publication_cancels_its_started_preparation() {
    let mut f = Fixture::new(0);
    f.app.world_mut().resource_mut::<Animation>().abort_host = Some(f.host);
    let request = f.push("/a");
    for _ in 0..24 {
        f.app.update();
    }
    let animation = f.app.world().resource::<Animation>();
    assert_eq!(animation.prepared, 1);
    assert_eq!(animation.presented, 0);
    assert!(animation.current.is_none());
    assert_eq!(
        animation.cancelled,
        vec![TransitionCancelReason::PreparationAborted]
    );
    assert!(
        matches!(f.app.world().resource::<RouterSnapshots>().request(f.router, request), RequestLookup::Found(request) if matches!(request.status(), RequestStatus::Failed(_)))
    );
    assert!(f.app.world().get_entity(f.host).is_ok());
}

#[test]
fn held_exit_host_loss_settles_published_result_and_allows_new_registration() {
    for remove_entities in [false, true] {
        let mut f = Fixture::new(2);
        let first = f.push("/a");
        f.wait_presented(1);
        f.settle(first);
        f.finish(false);
        f.app.update();
        f.app.world_mut().resource_mut::<Animation>().hold = true;
        let published = f.push("/b");
        f.wait_presented(2);
        let queued = f.push("/fresh");
        let history = f
            .app
            .world()
            .resource::<RouterSnapshots>()
            .get(f.router)
            .unwrap()
            .history()
            .clone();
        if remove_entities {
            f.app.world_mut().entity_mut(f.host).despawn();
        } else {
            f.app
                .world_mut()
                .entity_mut(f.host)
                .remove::<RouterHostRegistration>();
        }
        f.settle(published);
        let snapshots = f.app.world().resource::<RouterSnapshots>();
        assert_eq!(snapshots.get(f.router).unwrap().history(), &history);
        assert!(
            matches!(snapshots.request(f.router, queued), RequestLookup::Found(request) if matches!(request.status(), RequestStatus::Failed(_)))
        );
        assert!(f.app.world().resource::<Animation>().current.is_none());
        assert!(
            f.app
                .world()
                .resource::<Animation>()
                .cancelled
                .contains(&TransitionCancelReason::AuthorityLost)
        );
        if remove_entities {
            f.host = f
                .app
                .world_mut()
                .spawn((Node::default(), RouterHostRegistration::new(f.router)))
                .id();
        } else {
            assert!(f.app.world().get_entity(f.host).is_ok());
            f.app
                .world_mut()
                .entity_mut(f.host)
                .insert(RouterHostRegistration::new(f.router));
        }
        for _ in 0..24 {
            f.app.update();
            if f.app
                .world()
                .resource::<RouterSnapshots>()
                .get(f.router)
                .unwrap()
                .availability()
                == &RouterAvailability::Ready
            {
                break;
            }
        }
        assert_eq!(
            f.app
                .world()
                .resource::<RouterSnapshots>()
                .get(f.router)
                .unwrap()
                .availability(),
            &RouterAvailability::Ready
        );
        let restored = f.push("/fresh");
        f.wait_presented(3);
        f.settle(restored);
    }
}
