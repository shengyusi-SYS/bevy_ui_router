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
    sync::{Arc, Mutex},
};

#[derive(Component, Clone, Default)]
struct Page(&'static str);
#[derive(Component, Clone, Default, Debug, PartialEq)]
struct LocalValue(u32);

struct Fixture {
    app: App,
    router: RouterHandle,
    factories: Arc<Mutex<HashMap<&'static str, usize>>>,
}

impl Fixture {
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
                    RequestStatus::Failed(failure) => panic!("navigation failed: {failure:?}"),
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
        let request = self
            .app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(self.router, RouteLocation::new(path))
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
