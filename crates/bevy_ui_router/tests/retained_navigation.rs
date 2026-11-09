//! Public navigation across independently configured retained and disposable leaves.
#![cfg(feature = "bsn")]

use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::AssetPlugin;
use bevy_ecs::{prelude::*, template::FromTemplate};
use bevy_input_focus::{FocusCause, InputFocus};
use bevy_scene::{ResolvedScene, SceneFunction, ScenePlugin};
use bevy_ui::Node;
use bevy_ui_router::prelude::*;
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Component, Clone, Default)]
struct Page(&'static str);
#[derive(Component, Clone, Default, Debug, PartialEq)]
struct LocalValue(u32);

struct Fixture {
    app: App,
    router: RouterHandle,
    factories: Arc<Mutex<HashMap<&'static str, usize>>>,
    reject_child: Arc<AtomicBool>,
}

impl Fixture {
    fn nested(capacity: usize, keep_child: bool) -> Self {
        Self::nested_with_search_policy(capacity, keep_child, false)
    }

    fn nested_with_search_policy(capacity: usize, keep_child: bool, preserve_search: bool) -> Self {
        let mut app = App::new();
        app.add_plugins((
            TaskPoolPlugin::default(),
            AssetPlugin::default(),
            ScenePlugin,
            BevyUiRouterPlugin::default(),
        ));
        let factories = Arc::new(Mutex::new(HashMap::new()));
        let reject_child = Arc::new(AtomicBool::new(false));
        let mut views = RouteViewRegistry::new();
        for name in ["layout", "p", "c", "d", "q"] {
            let calls = factories.clone();
            let reject_child = reject_child.clone();
            let driver = app
                .register_bsn_view(ViewDriverKey::new(name).unwrap(), move |_| {
                    *calls.lock().unwrap().entry(name).or_default() += 1;
                    if name == "d" && reject_child.swap(false, Ordering::SeqCst) {
                        return Err(BsnBuildError::new("detail build rejected"));
                    }
                    Ok(SceneFunction(move |_, root: &mut ResolvedScene| {
                        root.push_template(Node::default());
                        root.push_template(Page(name));
                        root.push_template(LocalValue::default());
                        if name == "layout" {
                            root.push_template(
                                <DefaultOutlet as FromTemplate>::Template::default(),
                            );
                        } else if name == "p" {
                            let mut container = ResolvedScene::default();
                            container.push_template(Node::default());
                            container.push_template(<DefaultOutlet as FromTemplate>::Template::default());
                            root.get_or_insert_related_resolved_scenes::<ChildOf>().scenes.push(container);
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
                RouteNodeOptions::new()
                    .with_view(RouteViewKey::new("layout").unwrap())
                    .with_preserve_on_search(preserve_search),
            )
            .unwrap();
        let root = tree.root_id().unwrap();
        let parent = tree
            .route_with_options(
                root,
                RoutePattern::path("p").unwrap(),
                RouteNodeOptions::new()
                    .with_view(RouteViewKey::new("p").unwrap())
                    .with_preserve_on_search(preserve_search)
                    .with_keep_alive(true),
            )
            .unwrap();
        for (name, keep) in [("c", keep_child), ("d", false)] {
            let mut options = RouteNodeOptions::new()
                .with_view(RouteViewKey::new(name).unwrap())
                .with_keep_alive(keep);
            if name == "d" {
                options = options.with_error_view(
                    ErrorBoundaryKey::named("detail").unwrap(),
                    RouteViewKey::new("q").unwrap(),
                );
            }
            tree.route_with_options(parent, RoutePattern::path(name).unwrap(), options)
                .unwrap();
        }
        tree.route_with_options(
            root,
            RoutePattern::path("q").unwrap(),
            RouteNodeOptions::new().with_view(RouteViewKey::new("q").unwrap()),
        )
        .unwrap();
        let router = app
            .add_ui_router(
                RouterConfig::new(
                    tree.build().unwrap(),
                    views,
                    RouteViewKey::new("q").unwrap(),
                )
                .with_keep_alive_capacity(capacity),
            )
            .unwrap();
        app.world_mut()
            .spawn((Node::default(), RouterHostRegistration::new(router)));
        app.update();
        Self {
            app,
            router,
            factories,
            reject_child,
        }
    }

    fn new(capacity: usize, keep: [bool; 5]) -> Self {
        let mut app = App::new();
        app.add_plugins((
            TaskPoolPlugin::default(),
            AssetPlugin::default(),
            ScenePlugin,
            BevyUiRouterPlugin::default(),
        ));
        let factories = Arc::new(Mutex::new(HashMap::new()));
        let mut views = RouteViewRegistry::new();
        for name in ["layout", "a", "b", "c", "d", "e"] {
            let calls = factories.clone();
            let driver = app
                .register_bsn_view(ViewDriverKey::new(name).unwrap(), move |_| {
                    *calls.lock().unwrap().entry(name).or_default() += 1;
                    Ok(SceneFunction(move |_, root: &mut ResolvedScene| {
                        root.push_template(Node::default());
                        root.push_template(Page(name));
                        root.push_template(LocalValue::default());
                        if name == "layout" {
                            root.push_template(
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
                RouteNodeOptions::new().with_view(RouteViewKey::new("layout").unwrap()),
            )
            .unwrap();
        let parent = tree.root_id().unwrap();
        for (name, keep) in ["a", "b", "c", "d", "e"].into_iter().zip(keep) {
            tree.route_with_options(
                parent,
                RoutePattern::path(name).unwrap(),
                RouteNodeOptions::new()
                    .with_view(RouteViewKey::new(name).unwrap())
                    .with_keep_alive(keep),
            )
            .unwrap();
        }
        let router = app
            .add_ui_router(
                RouterConfig::new(
                    tree.build().unwrap(),
                    views,
                    RouteViewKey::new("e").unwrap(),
                )
                .with_keep_alive_capacity(capacity),
            )
            .unwrap();
        app.world_mut()
            .spawn((Node::default(), RouterHostRegistration::new(router)));
        app.update();
        Self {
            app,
            router,
            factories,
            reject_child: Arc::new(AtomicBool::new(false)),
        }
    }
    fn settle(&mut self, request: NavigationRequestId) {
        for _ in 0..32 {
            self.app.update();
            if let RequestLookup::Found(request) = self
                .app
                .world()
                .resource::<RouterSnapshots>()
                .request(self.router, request)
            {
                match request.status() {
                    RequestStatus::Succeeded => return,
                    RequestStatus::Failed(failure) => panic!(
                        "navigation failed: {failure:?}, trace: {:?}",
                        request.trace()
                    ),
                    _ => (),
                }
            }
        }
        panic!(
            "navigation did not settle: {:?}",
            self.app
                .world()
                .resource::<RouterSnapshots>()
                .request(self.router, request)
        );
    }
    fn push(&mut self, path: &str) {
        self.push_location(RouteLocation::new(path));
    }
    fn push_location(&mut self, location: RouteLocation) {
        let request = self
            .app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(self.router, location)
            .unwrap();
        self.settle(request);
    }
    fn back(&mut self) {
        let request = self
            .app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .back(self.router)
            .unwrap();
        self.settle(request);
    }
    fn active(&mut self, name: &str) -> Entity {
        let mut pages = self.app.world_mut().query::<(Entity, &Page)>();
        let gate = self.app.world().resource::<RouterInputGate>();
        let active = pages
            .iter(self.app.world())
            .filter(|(entity, page)| page.0 == name && gate.allows_pointer(*entity))
            .map(|(entity, _)| entity)
            .collect::<Vec<_>>();
        assert_eq!(active.len(), 1);
        active[0]
    }
    fn calls(&self, name: &str) -> usize {
        self.factories
            .lock()
            .unwrap()
            .get(name)
            .copied()
            .unwrap_or(0)
    }

    fn fail(&mut self, request: NavigationRequestId, kind: NavigationFailureKind) {
        for _ in 0..32 {
            self.app.update();
            if let RequestLookup::Found(record) = self
                .app
                .world()
                .resource::<RouterSnapshots>()
                .request(self.router, request)
            {
                if let RequestStatus::Failed(failure) = record.status() {
                    assert_eq!(failure.kind(), kind, "{:?}", record.trace());
                    return;
                }
                assert_ne!(record.status(), &RequestStatus::Succeeded);
            }
        }
        panic!("the original request must terminate");
    }
}

#[test]
fn search_opt_in_still_replaces_deepest_fragment_for_push_replace_and_back() {
    for replace in [false, true] {
        let mut fixture = Fixture::nested_with_search_policy(3, false, true);
        fixture.push_location(
            RouteLocation::new("/p")
                .with_search("sort=name")
                .with_fragment("a"),
        );
        let root = fixture.active("layout");
        let initial = fixture.active("p");
        let frame = |fixture: &Fixture| {
            fixture
                .app
                .world()
                .resource::<RouterSnapshots>()
                .get(fixture.router)
                .unwrap()
                .active()
                .frames()
                .last()
                .unwrap()
                .clone()
        };
        let initial_frame = frame(&fixture);
        let target = RouteLocation::new("/p")
            .with_search("sort=date")
            .with_fragment("b");
        if replace {
            let request = fixture
                .app
                .world_mut()
                .resource_mut::<RouterNavigation>()
                .replace(fixture.router, target.clone())
                .unwrap();
            fixture.settle(request);
        } else {
            fixture.push_location(target.clone());
        }
        assert_eq!(fixture.active("layout"), root);
        assert_ne!(
            fixture.active("p"),
            initial,
            "search opt-in must not bypass fragment replacement"
        );
        let changed = frame(&fixture);
        assert_eq!(changed.frame(), initial_frame.frame());
        assert_eq!(changed.slot(), initial_frame.slot());
        assert_eq!(
            changed.generation().raw(),
            initial_frame.generation().raw() + 1
        );

        fixture.push_location(
            RouteLocation::new("/p")
                .with_search("sort=duration")
                .with_fragment("c"),
        );
        let before_back = fixture.active("p");
        let before_back_frame = frame(&fixture);
        fixture.back();
        assert_eq!(fixture.active("layout"), root);
        assert_ne!(fixture.active("p"), before_back);
        let returned = frame(&fixture);
        assert_eq!(returned.frame(), before_back_frame.frame());
        assert_eq!(returned.slot(), before_back_frame.slot());
        assert_eq!(
            returned.generation().raw(),
            before_back_frame.generation().raw() + 1
        );
        assert_eq!(
            fixture
                .app
                .world()
                .resource::<RouterSnapshots>()
                .get(fixture.router)
                .unwrap()
                .active()
                .location(),
            Some(&target)
        );
        assert_eq!(fixture.calls("p"), 4);
    }
}

#[test]
fn search_opt_in_reuses_current_and_retained_layouts_with_fresh_children() {
    let mut fixture = Fixture::nested_with_search_policy(3, false, true);
    fixture.push_location(RouteLocation::new("/p/c").with_search("sort=name"));
    let root = fixture.active("layout");
    let parent = fixture.active("p");
    let first_child = fixture.active("c");
    fixture
        .app
        .world_mut()
        .get_mut::<LocalValue>(parent)
        .unwrap()
        .0 = 41;

    fixture.push_location(RouteLocation::new("/p/c").with_search("sort=date"));
    assert_eq!(fixture.active("layout"), root);
    assert_eq!(fixture.active("p"), parent);
    assert_ne!(fixture.active("c"), first_child);
    assert_eq!(fixture.calls("p"), 1);
    assert_eq!(fixture.calls("c"), 2);

    fixture.push_location(RouteLocation::new("/q").with_search("filter=other"));
    let mut inputs = bevy_ecs::system::SystemState::<RouteInputs>::new(fixture.app.world_mut());
    assert!(
        inputs
            .get(fixture.app.world())
            .unwrap()
            .current(parent)
            .is_none()
    );
    let returned = RouteLocation::new("/p/d").with_search("sort=duration");
    fixture.push_location(returned.clone());
    assert_eq!(fixture.active("layout"), root);
    assert_eq!(fixture.active("p"), parent);
    assert_eq!(
        fixture.app.world().get::<LocalValue>(parent),
        Some(&LocalValue(41))
    );
    assert_eq!(fixture.calls("p"), 1);
    assert_eq!(fixture.calls("d"), 1);
    assert_eq!(
        inputs
            .get(fixture.app.world())
            .unwrap()
            .current(parent)
            .unwrap()
            .location(),
        &returned,
    );
    fixture.back();
    fixture.back();
    assert_eq!(fixture.active("p"), parent);
    assert_eq!(
        fixture.app.world().get::<LocalValue>(parent),
        Some(&LocalValue(41))
    );
    assert_eq!(
        inputs
            .get(fixture.app.world())
            .unwrap()
            .current(parent)
            .unwrap()
            .location(),
        &RouteLocation::new("/p/c").with_search("sort=date"),
    );
}

fn assert_parent_and_child_retention_are_independent(keep_child: bool) {
    let mut fixture = Fixture::nested(3, keep_child);
    fixture.push("/p/c");
    let parent = fixture.active("p");
    let child = fixture.active("c");
    fixture
        .app
        .world_mut()
        .get_mut::<LocalValue>(parent)
        .unwrap()
        .0 = 47;
    fixture
        .app
        .world_mut()
        .get_mut::<LocalValue>(child)
        .unwrap()
        .0 = 63;

    fixture.push("/q");
    assert_eq!(
        fixture.app.world().get::<LocalValue>(parent),
        Some(&LocalValue(47))
    );
    assert_eq!(fixture.app.world().get_entity(child).is_ok(), keep_child);
    assert!(
        !fixture
            .app
            .world()
            .resource::<RouterInputGate>()
            .allows_pointer(parent)
    );
    let mut input_state =
        bevy_ecs::system::SystemState::<RouteInputs>::new(fixture.app.world_mut());
    assert!(
        input_state
            .get(fixture.app.world())
            .unwrap()
            .current(parent)
            .is_none()
    );

    fixture.push("/p/d");
    assert_eq!(fixture.active("p"), parent);
    assert_eq!(
        fixture.app.world().get::<LocalValue>(parent),
        Some(&LocalValue(47))
    );
    fixture.active("d");
    assert_eq!(fixture.calls("p"), 1);
    assert_eq!(fixture.calls("d"), 1);
    let inputs = input_state.get(fixture.app.world()).unwrap();
    assert_eq!(
        inputs.current(parent).unwrap().location(),
        &RouteLocation::new("/p/d")
    );
    assert!(inputs.current(child).is_none());

    fixture.push("/p/c");
    let returned_child = fixture.active("c");
    assert_eq!(fixture.active("p"), parent);
    assert_eq!(fixture.calls("p"), 1);
    if keep_child {
        assert_eq!(returned_child, child);
        assert_eq!(
            fixture.app.world().get::<LocalValue>(child),
            Some(&LocalValue(63))
        );
        assert_eq!(fixture.calls("c"), 1);
    } else {
        assert_ne!(returned_child, child);
        assert_eq!(fixture.calls("c"), 2);
    }
}

#[test]
fn retained_parent_selects_new_child_after_disposable_child_leaves() {
    assert_parent_and_child_retention_are_independent(false);
}

#[test]
fn retained_parent_keeps_independent_child_inventory_across_another_layout() {
    assert_parent_and_child_retention_are_independent(true);
}

#[test]
fn retained_parent_creates_its_first_child_slot_on_return() {
    let mut fixture = Fixture::nested(3, true);
    fixture.push("/p");
    let parent = fixture.active("p");
    fixture
        .app
        .world_mut()
        .get_mut::<LocalValue>(parent)
        .unwrap()
        .0 = 47;
    fixture.push("/q");
    fixture.push("/p/d");
    assert_eq!(fixture.active("p"), parent);
    assert_eq!(
        fixture.app.world().get::<LocalValue>(parent),
        Some(&LocalValue(47))
    );
    assert_eq!(fixture.calls("p"), 1);
    assert_eq!(fixture.calls("d"), 1);
}

#[test]
fn retained_parent_back_recreates_only_its_disposable_child() {
    let mut fixture = Fixture::nested(3, false);
    fixture.push("/p/c");
    let parent = fixture.active("p");
    let child = fixture.active("c");
    fixture
        .app
        .world_mut()
        .get_mut::<LocalValue>(parent)
        .unwrap()
        .0 = 47;
    fixture.push("/q");
    fixture.back();
    assert_eq!(fixture.active("p"), parent);
    assert_ne!(fixture.active("c"), child);
    assert_eq!(
        fixture.app.world().get::<LocalValue>(parent),
        Some(&LocalValue(47))
    );
    assert_eq!(fixture.calls("p"), 1);
    assert_eq!(fixture.calls("c"), 2);
    assert_eq!(
        fixture
            .app
            .world()
            .resource::<RouterSnapshots>()
            .get(fixture.router)
            .unwrap()
            .history()
            .current(),
        Some(&RouteLocation::new("/p/c"))
    );
}

#[test]
fn retained_parent_survives_fresh_child_build_failure_and_bound_error() {
    let mut fixture = Fixture::nested(3, true);
    fixture.push("/p/c");
    let parent = fixture.active("p");
    let child = fixture.active("c");
    fixture
        .app
        .world_mut()
        .get_mut::<LocalValue>(parent)
        .unwrap()
        .0 = 47;
    fixture.push("/q");
    let history = fixture
        .app
        .world()
        .resource::<RouterSnapshots>()
        .get(fixture.router)
        .unwrap()
        .history()
        .clone();
    fixture.reject_child.store(true, Ordering::SeqCst);
    let request = fixture
        .app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(fixture.router, RouteLocation::new("/p/d"))
        .unwrap();
    for _ in 0..32 {
        fixture.app.update();
        let RequestLookup::Found(record) = fixture
            .app
            .world()
            .resource::<RouterSnapshots>()
            .request(fixture.router, request)
        else {
            continue;
        };
        if matches!(record.status(), RequestStatus::Failed(_)) {
            break;
        }
    }
    let snapshots = fixture.app.world().resource::<RouterSnapshots>();
    let RequestLookup::Found(record) = snapshots.request(fixture.router, request) else {
        panic!("the original request remains queryable");
    };
    assert!(matches!(record.status(), RequestStatus::Failed(failure)
        if failure.kind() == NavigationFailureKind::Build));
    assert_eq!(snapshots.get(fixture.router).unwrap().history(), &history);
    assert_eq!(fixture.active("p"), parent);
    assert_eq!(
        fixture.app.world().get::<LocalValue>(parent),
        Some(&LocalValue(47))
    );
    assert_eq!(fixture.calls("p"), 1);
    assert!(fixture.app.world().get_entity(child).is_ok());
    assert!(
        !fixture
            .app
            .world()
            .resource::<RouterInputGate>()
            .allows_pointer(child)
    );
    let mut input_state =
        bevy_ecs::system::SystemState::<RouteInputs>::new(fixture.app.world_mut());
    let inputs = input_state.get(fixture.app.world()).unwrap();
    assert!(inputs.current(parent).unwrap().failure().is_none());
    assert_eq!(
        inputs.current(parent).unwrap().location(),
        &RouteLocation::new("/p/d")
    );
    fixture.push("/p/c");
    // Error presentations keep the existing full-rematch policy on the next navigation.
    assert_ne!(fixture.active("p"), parent);
    assert_ne!(fixture.active("c"), child);
    assert_eq!(fixture.calls("p"), 2);
    assert_eq!(fixture.calls("c"), 2);
}

#[test]
fn retained_parent_invalid_content_retires_its_child_inventory_before_fresh_return() {
    let mut fixture = Fixture::nested(3, true);
    fixture.push("/p/c");
    let parent = fixture.active("p");
    let child = fixture.active("c");
    fixture.push("/q");
    assert!(fixture.app.world_mut().despawn(parent));
    fixture.push("/p/d");
    assert_ne!(fixture.active("p"), parent);
    fixture.active("d");
    assert_eq!(fixture.calls("p"), 2);
    assert_eq!(fixture.calls("d"), 1);
    assert!(fixture.app.world().get_entity(child).is_err());
    fixture.push("/p/c");
    assert_ne!(fixture.active("c"), child);
    assert_eq!(fixture.calls("c"), 2);
}

#[test]
fn retained_parent_invalidated_during_child_apply_completes_both_original_cleanups() {
    let mut fixture = Fixture::nested(3, true);
    fixture.push("/p/c");
    let parent = fixture.active("p");
    let child = fixture.active("c");
    fixture.push("/q");
    let mut outlets = fixture
        .app
        .world_mut()
        .query::<(Entity, &DefaultOutlet, &ChildOf)>();
    let anchor = outlets
        .iter(fixture.app.world())
        .find(|(_, _, binding)| binding.parent() == parent)
        .unwrap()
        .0;
    let invalidate = Arc::new(AtomicBool::new(true));
    fixture.app.world_mut().add_observer(
        move |event: On<Insert<Page>>, pages: Query<&Page>, mut commands: Commands| {
            if pages.get(event.entity).is_ok_and(|page| page.0 == "d")
                && invalidate.swap(false, Ordering::SeqCst)
            {
                commands.entity(anchor).remove::<Node>();
            }
        },
    );
    fixture.push("/p/d");
    assert_ne!(fixture.active("p"), parent);
    fixture.active("d");
    assert_eq!(fixture.calls("p"), 2);
    assert_eq!(fixture.calls("d"), 2);
    assert!(fixture.app.world().get_entity(child).is_err());
    fixture.push("/p/c");
    assert_ne!(fixture.active("c"), child);
}

#[test]
fn retained_parent_disposed_forest_completes_before_host_loss_and_readmission() {
    let mut fixture = Fixture::nested(3, true);
    fixture.push("/p/c");
    let parent = fixture.active("p");
    let child = fixture.active("c");
    let shell = fixture.app.world().get::<ChildOf>(parent).unwrap().parent();
    let mut hosts = fixture
        .app
        .world_mut()
        .query_filtered::<Entity, With<RouterHostRegistration>>();
    let host = hosts.single(fixture.app.world()).unwrap();
    fixture.push("/q");
    let history = fixture
        .app
        .world()
        .resource::<RouterSnapshots>()
        .get(fixture.router)
        .unwrap()
        .history()
        .clone();
    fixture.app.world_mut().add_observer(
        move |event: On<Despawn<Node>>, mut commands: Commands| {
            if event.entity == shell {
                commands.entity(host).remove::<RouterHostRegistration>();
            }
        },
    );
    assert!(fixture.app.world_mut().despawn(parent));
    let request = fixture
        .app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(fixture.router, RouteLocation::new("/p/d"))
        .unwrap();
    fixture.fail(request, NavigationFailureKind::HostLost);
    assert_eq!(
        fixture
            .app
            .world()
            .resource::<RouterSnapshots>()
            .get(fixture.router)
            .unwrap()
            .history(),
        &history
    );
    assert!(fixture.app.world().get_entity(host).is_ok());
    assert!(fixture.app.world().get_entity(shell).is_err());
    assert!(fixture.app.world().get_entity(child).is_err());
    fixture
        .app
        .world_mut()
        .entity_mut(host)
        .insert(RouterHostRegistration::new(fixture.router));
    for _ in 0..32 {
        fixture.app.update();
        if fixture
            .app
            .world()
            .resource::<RouterSnapshots>()
            .get(fixture.router)
            .unwrap()
            .availability()
            == &RouterAvailability::Ready
        {
            break;
        }
    }
    assert_eq!(
        fixture
            .app
            .world()
            .resource::<RouterSnapshots>()
            .get(fixture.router)
            .unwrap()
            .availability(),
        &RouterAvailability::Ready
    );
    fixture.push("/p/d");
    assert_ne!(fixture.active("p"), parent);
    fixture.active("d");
}

#[test]
fn retained_parent_live_forest_host_loss_transfers_original_cleanup_before_terminal() {
    let mut fixture = Fixture::nested(3, true);
    fixture.push("/p/c");
    let parent = fixture.active("p");
    let child = fixture.active("c");
    let shell = fixture.app.world().get::<ChildOf>(parent).unwrap().parent();
    let mut outlets = fixture
        .app
        .world_mut()
        .query::<(Entity, &DefaultOutlet, &ChildOf)>();
    let anchor = outlets
        .iter(fixture.app.world())
        .find(|(_, _, edge)| edge.parent() == parent)
        .unwrap()
        .0;
    let mut hosts = fixture
        .app
        .world_mut()
        .query_filtered::<Entity, With<RouterHostRegistration>>();
    let host = hosts.single(fixture.app.world()).unwrap();
    fixture.push("/q");
    let history = fixture
        .app
        .world()
        .resource::<RouterSnapshots>()
        .get(fixture.router)
        .unwrap()
        .history()
        .clone();
    let fired = Arc::new(AtomicBool::new(false));
    let observed = Arc::clone(&fired);
    fixture.app.world_mut().add_observer(
        move |event: On<Despawn<Page>>, mut commands: Commands| {
            if event.entity == child {
                observed.store(true, Ordering::SeqCst);
                commands.entity(host).remove::<RouterHostRegistration>();
            }
        },
    );
    fixture.app.world_mut().entity_mut(anchor).remove::<Node>();
    let request = fixture
        .app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(fixture.router, RouteLocation::new("/p/d"))
        .unwrap();
    fixture.fail(request, NavigationFailureKind::AuthorityLost);
    assert!(
        fired.load(Ordering::SeqCst),
        "the real child cleanup hook must run"
    );
    let snapshot = fixture
        .app
        .world()
        .resource::<RouterSnapshots>()
        .get(fixture.router)
        .unwrap();
    assert_eq!(snapshot.history(), &history);
    assert!(snapshot.running_request().is_none());
    assert!(fixture.app.world().get_entity(host).is_ok());
    assert!(
        fixture.app.world().get_entity(shell).is_ok(),
        "the original live residual remains owned"
    );
    assert!(
        !fixture
            .app
            .world()
            .resource::<RouterInputGate>()
            .allows_pointer(parent)
    );
    assert_eq!(fixture.calls("d"), 0);
}

fn assert_mixed_return(back: bool, keep_b: bool) {
    let mut fixture = Fixture::new(2, [true, keep_b, false, false, false]);
    fixture.push("/a");
    let original = fixture.active("a");
    let layout = fixture.active("layout");
    fixture
        .app
        .world_mut()
        .get_mut::<LocalValue>(original)
        .unwrap()
        .0 = 91;
    fixture
        .app
        .world_mut()
        .resource_mut::<InputFocus>()
        .set(original, FocusCause::Navigated);
    fixture.push("/b");
    let b = fixture.active("b");
    assert!(
        !fixture
            .app
            .world()
            .resource::<RouterInputGate>()
            .allows_pointer(original)
    );
    assert_ne!(
        fixture.app.world().resource::<InputFocus>().get(),
        Some(original)
    );
    if back {
        fixture.back();
    } else {
        fixture.push("/a");
    }
    assert_eq!(fixture.active("a"), original);
    assert_eq!(fixture.active("layout"), layout);
    assert_eq!(
        fixture.app.world().get::<LocalValue>(original),
        Some(&LocalValue(91))
    );
    assert_eq!(fixture.calls("a"), 1);
    let snapshot = fixture
        .app
        .world()
        .resource::<RouterSnapshots>()
        .get(fixture.router)
        .unwrap();
    assert_eq!(
        snapshot.active().location(),
        Some(&RouteLocation::new("/a"))
    );
    assert_eq!(snapshot.history().entries().len(), if back { 2 } else { 3 });
    assert!(
        !fixture
            .app
            .world()
            .resource::<RouterInputGate>()
            .allows_pointer(b)
    );
    if !keep_b {
        assert!(fixture.app.world().get_entity(b).is_err());
    }
    // A successful hit must leave usable mount facts for a later fresh replacement.
    fixture.push("/c");
    fixture.push("/d");
    fixture.active("d");
}

#[test]
fn mixed_retention_back_reuses_the_original_leaf_and_continues() {
    assert_mixed_return(true, false);
}
#[test]
fn mixed_retention_push_reuses_the_original_leaf_and_continues() {
    assert_mixed_return(false, false);
}
#[test]
fn both_retained_back_control() {
    assert_mixed_return(true, true);
}

fn eviction_then_fresh(capacity: usize) {
    let mut fixture = Fixture::new(capacity, [true, true, true, false, false]);
    fixture.push("/a");
    let a = fixture.active("a");
    let layout = fixture.active("layout");
    for path in ["/b", "/c", "/d", "/e"] {
        fixture.push(path);
    }
    fixture.active("e");
    assert_eq!(fixture.active("layout"), layout);
    assert_eq!(fixture.calls("layout"), 1);
    if capacity < 3 {
        assert!(fixture.app.world().get_entity(a).is_err());
    }
}
#[test]
fn lru_retirement_preserves_later_fresh_mounts() {
    for capacity in [1, 2] {
        eviction_then_fresh(capacity);
    }
}
#[test]
fn fresh_mounts_without_eviction_control() {
    eviction_then_fresh(3);
}
