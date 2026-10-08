//! Opt-in restoration exercised through the public BSN/router and real UI layout.
use std::sync::{Arc, Mutex};

use bevy_app::{App, PostUpdate};
use bevy_ecs::prelude::*;
use bevy_input_focus::InputFocusPlugin;
use bevy_internal::{
    DefaultPlugins,
    camera::{Camera, Camera2d, ComputedCameraValues, RenderTargetInfo},
    math::{UVec2, Vec2},
};
use bevy_scene::{ScenePlugin, bsn};
use bevy_ui::{
    ComputedNode, ComputedUiRenderTargetInfo, ComputedUiTargetCamera, FlexDirection, Node,
    Overflow, ScrollPosition, UiSystems, UiTransform, Val,
};
use bevy_ui_router::prelude::*;

#[derive(Component, Clone, Default)]
struct Page(&'static str);

#[derive(Component, Clone, Default)]
struct ScrollContainer(&'static str);

#[derive(Component, Clone, Default)]
struct PageContent(&'static str);

#[derive(Clone, Copy)]
struct FixtureOptions {
    duplicate_keys: bool,
    initial: Vec2,
    fallback_fails: bool,
    camera: bool,
    keep_alive_capacity: usize,
}

impl Default for FixtureOptions {
    fn default() -> Self {
        Self {
            duplicate_keys: false,
            initial: Vec2::ZERO,
            fallback_fails: false,
            camera: true,
            keep_alive_capacity: 0,
        }
    }
}

struct Fixture {
    app: App,
    router: RouterHandle,
    host: Entity,
    camera: Option<Entity>,
    content_size: Arc<Mutex<Vec2>>,
}

fn fixture(options: FixtureOptions) -> Fixture {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins);
    if !app.is_plugin_added::<ScenePlugin>() {
        app.add_plugins(ScenePlugin);
    }
    app.add_plugins(InputFocusPlugin);
    // There is no GPU renderer in this test. Supply the real camera input that
    // UI propagation normally receives from the renderer's camera system.
    let camera = options.camera.then(|| {
        app.world_mut()
            .spawn((
                Camera2d,
                Camera {
                    computed: ComputedCameraValues {
                        target_info: Some(RenderTargetInfo {
                            physical_size: UVec2::new(640, 480),
                            scale_factor: 1.0,
                        }),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            ))
            .id()
    });
    let content_size = Arc::new(Mutex::new(Vec2::new(600.0, 800.0)));
    let mut views = RouteViewRegistry::new();
    let layout_driver = app
        .register_bsn_view(ViewDriverKey::new("layout").unwrap(), |_| {
            Ok(bsn! {
                Node {
                    width: Val::Px(160.0), height: Val::Px(360.0),
                    flex_direction: FlexDirection::Column,
                }
                Children [
                    (Node {
                        width: Val::Px(160.0), height: Val::Px(120.0),
                        flex_shrink: 0.0, overflow: Overflow::scroll(),
                    }
                    // The same key is valid in the parent and leaf frames.
                    RouteScroll::new("body") ScrollContainer("prefix")
                    Children [(Node {
                        width: Val::Px(600.0), height: Val::Px(800.0), flex_shrink: 0.0,
                    })]),
                    (Node {
                        width: Val::Px(160.0), height: Val::Px(120.0),
                        flex_shrink: 0.0, overflow: Overflow::scroll(),
                    }
                    ScrollContainer("unmarked-prefix")
                    Children [(Node {
                        width: Val::Px(600.0), height: Val::Px(800.0), flex_shrink: 0.0,
                    })]),
                    (Node {
                        width: Val::Px(160.0), height: Val::Px(120.0), flex_shrink: 0.0,
                    } DefaultOutlet)
                ]
            })
        })
        .unwrap();
    let layout_view = RouteViewKey::new("layout").unwrap();
    views
        .insert(layout_view.clone(), RouteViewDescriptor::new(layout_driver))
        .unwrap();
    let mut routes = RouteTree::builder()
        .root_with_options(
            RoutePattern::path("/").unwrap(),
            RouteNodeOptions::new().with_view(layout_view),
        )
        .unwrap();
    let root = routes.root_id().unwrap();
    for name in ["a", "b", "c", "broken", "fallback"] {
        let size = Arc::clone(&content_size);
        let driver = app
            .register_bsn_view(ViewDriverKey::new(name).unwrap(), move |_| {
                if name == "broken" || (name == "fallback" && options.fallback_fails) {
                    return Err(BsnBuildError::new("scroll fixture build failure"));
                }
                let size = *size.lock().unwrap();
                Ok(bsn! {
                    Node {
                        width: Val::Px(160.0), height: Val::Px(120.0),
                        flex_shrink: 0.0, overflow: Overflow::scroll(),
                    }
                    RouteScroll::new("body") ScrollPosition({ options.initial }) Page(name)
                    ScrollContainer("body")
                    Children [
                        (Node {
                            width: Val::Px({ size.x }), height: Val::Px({ size.y }), flex_shrink: 0.0,
                        }
                        PageContent(name)
                        Children [
                            (Node {
                                width: Val::Px(100.0), height: Val::Px(80.0),
                                flex_shrink: 0.0, overflow: Overflow::scroll(),
                            }
                            RouteScroll::new(if options.duplicate_keys { "body" } else { "secondary" })
                            ScrollContainer("secondary")
                            Children [(Node {
                                width: Val::Px(500.0), height: Val::Px(600.0), flex_shrink: 0.0,
                            })])
                        ])
                    ]
                })
            })
            .unwrap();
        let view = RouteViewKey::new(name).unwrap();
        views
            .insert(view.clone(), RouteViewDescriptor::new(driver))
            .unwrap();
        if name != "fallback" {
            routes
                .route_with_options(
                    root,
                    RoutePattern::path(name).unwrap(),
                    RouteNodeOptions::new()
                        .with_view(view)
                        .with_keep_alive(options.keep_alive_capacity > 0),
                )
                .unwrap();
        }
    }
    let router = app
        .add_ui_router(
            RouterConfig::new(
                routes.build().unwrap(),
                views,
                RouteViewKey::new("fallback").unwrap(),
            )
            .with_keep_alive_capacity(options.keep_alive_capacity),
        )
        .unwrap();
    let host = app
        .world_mut()
        .spawn((
            Node {
                width: Val::Px(160.0),
                height: Val::Px(360.0),
                ..Default::default()
            },
            RouterHostRegistration::new(router),
        ))
        .id();
    app.update();
    Fixture {
        app,
        router,
        host,
        camera,
        content_size,
    }
}

fn app() -> (App, RouterHandle) {
    let fixture = fixture(FixtureOptions::default());
    (fixture.app, fixture.router)
}

fn page(app: &mut App, name: &str) -> Entity {
    app.world_mut()
        .query::<(Entity, &Page)>()
        .iter(app.world())
        .find_map(|(entity, page)| {
            (page.0 == name
                && app
                    .world()
                    .resource::<RouterInputGate>()
                    .allows_keyboard(entity))
            .then_some(entity)
        })
        .expect("the active page exists")
}

fn container(app: &mut App, name: &str) -> Entity {
    app.world_mut()
        .query::<(Entity, &ScrollContainer)>()
        .iter(app.world())
        .find_map(|(entity, container)| {
            (container.0 == name
                && app
                    .world()
                    .resource::<RouterInputGate>()
                    .allows_keyboard(entity))
            .then_some(entity)
        })
        .expect("the named scroll container exists")
}

fn content(app: &mut App, name: &str) -> Entity {
    app.world_mut()
        .query::<(Entity, &PageContent)>()
        .iter(app.world())
        .find_map(|(entity, content)| {
            (content.0 == name
                && app
                    .world()
                    .resource::<RouterInputGate>()
                    .allows_keyboard(entity))
            .then_some(entity)
        })
        .expect("the page content exists")
}

fn settle_status(
    app: &mut App,
    router: RouterHandle,
    request: NavigationRequestId,
) -> RequestStatus {
    for _ in 0..32 {
        app.update();
        let snapshots = app.world().resource::<RouterSnapshots>();
        let RequestLookup::Found(snapshot) = snapshots.request(router, request) else {
            panic!("the public request remains observable")
        };
        match snapshot.status() {
            RequestStatus::Succeeded | RequestStatus::Failed(_) => {
                return snapshot.status().clone();
            }
            _ => {}
        }
    }
    panic!(
        "navigation must settle in a bounded number of normal updates: {:?}; {:?}",
        app.world()
            .resource::<RouterSnapshots>()
            .request(router, request),
        app.world().resource::<RouterSnapshots>().get(router),
    )
}

fn settle(app: &mut App, router: RouterHandle, request: NavigationRequestId) {
    let status = settle_status(app, router, request);
    assert_eq!(
        status,
        RequestStatus::Succeeded,
        "navigation failed: {status:?}"
    );
}

fn push(app: &mut App, router: RouterHandle, path: &str) {
    let request = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, RouteLocation::new(path))
        .unwrap();
    settle(app, router, request);
}

fn replace(app: &mut App, router: RouterHandle, path: &str) {
    let request = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .replace(router, RouteLocation::new(path))
        .unwrap();
    settle(app, router, request);
}

fn back(app: &mut App, router: RouterHandle) {
    let request = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .back(router)
        .unwrap();
    settle(app, router, request);
}

fn history(app: &App, router: RouterHandle) -> HistorySnapshot {
    app.world()
        .resource::<RouterSnapshots>()
        .get(router)
        .unwrap()
        .history()
        .clone()
}

fn assert_vec(actual: Vec2, expected: Vec2) {
    assert!(
        (actual - expected).abs().max_element() < 0.01,
        "expected {expected:?}, observed {actual:?}"
    );
}

fn logical_position(app: &App, entity: Entity) -> Vec2 {
    let target = app
        .world()
        .get::<ComputedUiRenderTargetInfo>(entity)
        .unwrap();
    assert!(target.physical_size().min_element() > 0);
    assert!(target.scale_factor() > 0.0);
    assert!(
        app.world()
            .get::<ComputedUiTargetCamera>(entity)
            .unwrap()
            .get()
            .is_some()
    );
    let node = app.world().get::<ComputedNode>(entity).unwrap();
    assert!(node.size().min_element() > 0.0);
    node.scroll_position * node.inverse_scale_factor
}

fn assert_position(app: &App, entity: Entity, expected: Vec2) {
    assert_vec(
        app.world().get::<ScrollPosition>(entity).unwrap().0,
        expected,
    );
    assert_vec(logical_position(app, entity), expected);
}

fn scroll(app: &mut App, entity: Entity, position: Vec2) {
    app.world_mut().get_mut::<ScrollPosition>(entity).unwrap().0 = position;
    app.update();
    assert_position(app, entity, position);
}

fn drive_availability(app: &mut App, router: RouterHandle, expected: RouterAvailability) {
    for _ in 0..32 {
        app.update();
        let snapshots = app.world().resource::<RouterSnapshots>();
        let snapshot = snapshots.get(router).unwrap();
        if snapshot.availability() == &expected
            && !snapshot.cleanup().foreground_pending()
            && snapshot.running_request().is_none()
            && snapshot.pending_phase().is_none()
        {
            return;
        }
    }
    panic!(
        "router did not reach {expected:?}; current: {:?}",
        app.world()
            .resource::<RouterSnapshots>()
            .get(router)
            .unwrap()
    );
}

#[test]
fn repeated_entries_restore_distinct_positions_on_cache_hits_misses_and_eviction() {
    for capacity in [0, 1, 2] {
        let Fixture {
            mut app, router, ..
        } = fixture(FixtureOptions {
            keep_alive_capacity: capacity,
            ..Default::default()
        });
        push(&mut app, router, "/a");
        let original_a = page(&mut app, "a");
        let prefix = container(&mut app, "prefix");
        let secondary = container(&mut app, "secondary");
        let first_body = Vec2::new(45.0, 70.0);
        let first_secondary = Vec2::new(12.0, 30.0);
        let first_prefix = Vec2::new(24.0, 32.0);
        scroll(&mut app, original_a, first_body);
        scroll(&mut app, secondary, first_secondary);
        scroll(&mut app, prefix, first_prefix);

        push(&mut app, router, "/b");
        let original_b = page(&mut app, "b");
        let b_position = Vec2::new(88.0, 150.0);
        scroll(&mut app, original_b, b_position);
        push(&mut app, router, "/a");
        let shared_a = page(&mut app, "a");
        if capacity > 0 {
            assert_eq!(shared_a, original_a);
            assert_position(&app, shared_a, first_body);
        } else {
            assert_ne!(shared_a, original_a);
        }
        let later_body = Vec2::new(210.0, 250.0);
        let later_secondary = Vec2::new(80.0, 110.0);
        let later_prefix = Vec2::new(44.0, 52.0);
        let secondary = container(&mut app, "secondary");
        scroll(&mut app, shared_a, later_body);
        scroll(&mut app, secondary, later_secondary);
        scroll(&mut app, prefix, later_prefix);
        push(&mut app, router, "/c");
        assert_eq!(history(&app, router).entries().len(), 4);
        if capacity == 1 {
            assert!(app.world().get_entity(original_b).is_err());
        }

        back(&mut app, router);
        let returned_a = page(&mut app, "a");
        let secondary = container(&mut app, "secondary");
        assert_position(&app, returned_a, later_body);
        assert_position(&app, secondary, later_secondary);
        assert_position(&app, prefix, later_prefix);
        if capacity > 0 {
            assert_eq!(returned_a, shared_a);
        } else {
            assert_ne!(returned_a, shared_a);
        }

        back(&mut app, router);
        let returned_b = page(&mut app, "b");
        assert_position(&app, returned_b, b_position);
        if capacity == 2 {
            assert_eq!(returned_b, original_b);
        } else {
            assert_ne!(returned_b, original_b);
        }

        back(&mut app, router);
        let first_entry_a = page(&mut app, "a");
        let secondary = container(&mut app, "secondary");
        assert_position(&app, first_entry_a, first_body);
        assert_position(&app, secondary, first_secondary);
        assert_position(&app, prefix, first_prefix);
        assert_eq!(history(&app, router).cursor(), Some(0));
        if capacity > 0 {
            assert_eq!(first_entry_a, shared_a);
        } else {
            assert_ne!(first_entry_a, returned_a);
        }
    }
}

#[test]
fn back_restores_both_axes_on_a_fresh_page() {
    // Removing publication restore or saving the wrong entry must fail this journey.
    let (mut app, router) = app();
    push(&mut app, router, "/a");
    let first = page(&mut app, "a");
    scroll(&mut app, first, Vec2::new(70.0, 240.0));
    push(&mut app, router, "/b");
    assert!(app.world().get_entity(first).is_err());
    back(&mut app, router);
    let restored = page(&mut app, "a");
    assert_ne!(restored, first);
    assert_position(&app, restored, Vec2::new(70.0, 240.0));
}

#[test]
fn repeated_locations_keep_separate_entry_positions() {
    let (mut app, router) = app();
    push(&mut app, router, "/a");
    let first = page(&mut app, "a");
    scroll(&mut app, first, Vec2::new(20.0, 100.0));
    push(&mut app, router, "/b");
    push(&mut app, router, "/a");
    let second = page(&mut app, "a");
    assert_position(&app, second, Vec2::ZERO);
    scroll(&mut app, second, Vec2::new(80.0, 500.0));
    back(&mut app, router);
    back(&mut app, router);
    let restored = page(&mut app, "a");
    assert_position(&app, restored, Vec2::new(20.0, 100.0));
}

#[test]
fn back_restores_marked_preserved_prefix_and_leaves_unmarked_prefix_alone() {
    let (mut app, router) = app();
    push(&mut app, router, "/a");
    let prefix = container(&mut app, "prefix");
    let unmarked = container(&mut app, "unmarked-prefix");
    scroll(&mut app, prefix, Vec2::new(30.0, 150.0));
    scroll(&mut app, unmarked, Vec2::new(20.0, 120.0));
    push(&mut app, router, "/b");
    assert_eq!(container(&mut app, "prefix"), prefix);
    assert_position(&app, prefix, Vec2::new(30.0, 150.0));
    scroll(&mut app, prefix, Vec2::new(90.0, 450.0));
    scroll(&mut app, unmarked, Vec2::new(50.0, 360.0));
    back(&mut app, router);
    assert_eq!(container(&mut app, "prefix"), prefix);
    assert_eq!(container(&mut app, "unmarked-prefix"), unmarked);
    assert_position(&app, prefix, Vec2::new(30.0, 150.0));
    assert_position(&app, unmarked, Vec2::new(50.0, 360.0));
}

#[test]
fn containers_restore_independently_with_frame_local_keys() {
    let (mut app, router) = app();
    push(&mut app, router, "/a");
    let body = page(&mut app, "a");
    let secondary = container(&mut app, "secondary");
    let prefix = container(&mut app, "prefix");
    scroll(&mut app, body, Vec2::new(70.0, 240.0));
    scroll(&mut app, secondary, Vec2::new(25.0, 130.0));
    scroll(&mut app, prefix, Vec2::new(40.0, 180.0));
    push(&mut app, router, "/b");
    back(&mut app, router);
    let body = page(&mut app, "a");
    let secondary = container(&mut app, "secondary");
    assert_position(&app, body, Vec2::new(70.0, 240.0));
    assert_position(&app, secondary, Vec2::new(25.0, 130.0));
    assert_position(&app, prefix, Vec2::new(40.0, 180.0));
}

#[test]
fn missing_container_layout_keeps_its_last_valid_position_on_fresh_back() {
    let (mut app, router) = app();
    push(&mut app, router, "/a");
    let body = page(&mut app, "a");
    let secondary = container(&mut app, "secondary");
    scroll(&mut app, body, Vec2::new(70.0, 240.0));
    scroll(&mut app, secondary, Vec2::new(25.0, 130.0));
    app.world_mut()
        .entity_mut(secondary)
        .remove::<bevy_ui::UiGlobalTransform>();
    app.update();

    push(&mut app, router, "/b");
    back(&mut app, router);
    let restored = page(&mut app, "a");
    let restored_secondary = container(&mut app, "secondary");
    assert_ne!(restored, body);
    assert_ne!(restored_secondary, secondary);
    assert_position(&app, restored, Vec2::new(70.0, 240.0));
    assert_position(&app, restored_secondary, Vec2::new(25.0, 130.0));
}

#[test]
fn removing_a_scroll_marker_discards_its_last_valid_live_position() {
    let (mut app, router) = app();
    push(&mut app, router, "/a");
    let secondary = container(&mut app, "secondary");
    scroll(&mut app, secondary, Vec2::new(25.0, 130.0));
    app.world_mut()
        .entity_mut(secondary)
        .remove::<RouteScroll>();
    app.update();
    // Rejoining with the same key does not revive a witness discarded while
    // the container was unmarked, and this instance has no new layout sample.
    app.world_mut()
        .entity_mut(secondary)
        .insert(RouteScroll::new("secondary"))
        .remove::<bevy_ui::UiGlobalTransform>();
    app.update();

    push(&mut app, router, "/b");
    back(&mut app, router);
    let restored_secondary = container(&mut app, "secondary");
    assert_ne!(restored_secondary, secondary);
    assert_position(&app, restored_secondary, Vec2::ZERO);
}

#[test]
fn a_replacement_entity_with_the_same_scroll_key_has_no_old_live_position() {
    let (mut app, router) = app();
    push(&mut app, router, "/a");
    let secondary = container(&mut app, "secondary");
    scroll(&mut app, secondary, Vec2::new(25.0, 130.0));
    let parent = content(&mut app, "a");
    app.world_mut().despawn(secondary);
    let replacement = app
        .world_mut()
        .spawn((
            Node {
                width: Val::Px(100.0),
                height: Val::Px(80.0),
                flex_shrink: 0.0,
                overflow: Overflow::scroll(),
                ..Default::default()
            },
            RouteScroll::new("secondary"),
            ScrollPosition(Vec2::new(9.0, 19.0)),
            ScrollContainer("secondary"),
            ChildOf(parent),
        ))
        .id();
    app.world_mut().spawn((
        Node {
            width: Val::Px(500.0),
            height: Val::Px(600.0),
            flex_shrink: 0.0,
            ..Default::default()
        },
        ChildOf(replacement),
    ));
    app.world_mut()
        .entity_mut(replacement)
        .remove::<bevy_ui::UiGlobalTransform>();
    app.update();

    push(&mut app, router, "/b");
    back(&mut app, router);
    let restored_secondary = container(&mut app, "secondary");
    assert_ne!(restored_secondary, replacement);
    assert_position(&app, restored_secondary, Vec2::ZERO);
}

#[test]
fn duplicate_keys_do_not_pick_one_container_to_restore() {
    let Fixture {
        mut app, router, ..
    } = fixture(FixtureOptions {
        duplicate_keys: true,
        initial: Vec2::new(7.0, 30.0),
        ..Default::default()
    });
    push(&mut app, router, "/a");
    let body = page(&mut app, "a");
    let secondary = container(&mut app, "secondary");
    scroll(&mut app, body, Vec2::new(70.0, 240.0));
    scroll(&mut app, secondary, Vec2::new(25.0, 130.0));
    push(&mut app, router, "/b");
    back(&mut app, router);
    let body = page(&mut app, "a");
    let secondary = container(&mut app, "secondary");
    assert_position(&app, body, Vec2::new(7.0, 30.0));
    assert_position(&app, secondary, Vec2::ZERO);
}

#[test]
fn equivalent_back_restores_an_older_entry_on_the_same_entity() {
    let (mut app, router) = app();
    push(&mut app, router, "/a");
    let first = page(&mut app, "a");
    scroll(&mut app, first, Vec2::new(20.0, 100.0));
    push(&mut app, router, "/a");
    assert_eq!(page(&mut app, "a"), first);
    assert_position(&app, first, Vec2::new(20.0, 100.0));
    scroll(&mut app, first, Vec2::new(80.0, 500.0));
    push(&mut app, router, "/b");
    back(&mut app, router);
    let current = page(&mut app, "a");
    assert_position(&app, current, Vec2::new(80.0, 500.0));
    back(&mut app, router);
    assert_eq!(page(&mut app, "a"), current);
    assert_position(&app, current, Vec2::new(20.0, 100.0));
    assert_eq!(history(&app, router).cursor(), Some(0));
}

#[test]
fn replace_uses_the_replacement_visit_position_without_resetting_live_content() {
    let (mut app, router) = app();
    push(&mut app, router, "/a");
    let first = page(&mut app, "a");
    scroll(&mut app, first, Vec2::new(20.0, 100.0));
    replace(&mut app, router, "/a");
    assert_eq!(page(&mut app, "a"), first);
    assert_position(&app, first, Vec2::new(20.0, 100.0));
    scroll(&mut app, first, Vec2::new(80.0, 500.0));
    push(&mut app, router, "/b");
    back(&mut app, router);
    let restored = page(&mut app, "a");
    assert_position(&app, restored, Vec2::new(80.0, 500.0));
    assert_eq!(
        history(&app, router).entries(),
        [RouteLocation::new("/a"), RouteLocation::new("/b")]
    );
}

#[test]
fn push_after_back_prunes_forward_visits_without_reusing_their_positions() {
    let (mut app, router) = app();
    push(&mut app, router, "/a");
    let first = page(&mut app, "a");
    scroll(&mut app, first, Vec2::new(20.0, 100.0));
    push(&mut app, router, "/b");
    push(&mut app, router, "/a");
    let forward = page(&mut app, "a");
    scroll(&mut app, forward, Vec2::new(80.0, 500.0));
    back(&mut app, router);
    back(&mut app, router);
    let oldest = page(&mut app, "a");
    assert_position(&app, oldest, Vec2::new(20.0, 100.0));
    push(&mut app, router, "/a");
    assert_eq!(page(&mut app, "a"), oldest);
    assert_position(&app, oldest, Vec2::new(20.0, 100.0));
    assert_eq!(
        history(&app, router).entries(),
        [RouteLocation::new("/a"), RouteLocation::new("/a")]
    );
    scroll(&mut app, oldest, Vec2::new(40.0, 200.0));
    push(&mut app, router, "/c");
    back(&mut app, router);
    let newest = page(&mut app, "a");
    assert_position(&app, newest, Vec2::new(40.0, 200.0));
    back(&mut app, router);
    assert_eq!(page(&mut app, "a"), newest);
    assert_position(&app, newest, Vec2::new(20.0, 100.0));
}

#[test]
fn a_shorter_restored_page_settles_the_clamp_before_content_grows_again() {
    let Fixture {
        mut app,
        router,
        content_size,
        ..
    } = fixture(FixtureOptions::default());
    push(&mut app, router, "/a");
    let first = page(&mut app, "a");
    scroll(&mut app, first, Vec2::new(70.0, 240.0));
    push(&mut app, router, "/b");
    *content_size.lock().unwrap() = Vec2::new(180.0, 160.0);
    back(&mut app, router);
    let restored = page(&mut app, "a");
    assert_position(&app, restored, Vec2::new(20.0, 40.0));
    let child = content(&mut app, "a");
    let mut node = app.world_mut().get_mut::<Node>(child).unwrap();
    node.width = Val::Px(600.0);
    node.height = Val::Px(800.0);
    drop(node);
    app.update();
    assert_position(&app, restored, Vec2::new(20.0, 40.0));
}

#[test]
fn back_without_a_record_keeps_the_live_container_position() {
    let (mut app, router) = app();
    push(&mut app, router, "/a");
    let body = page(&mut app, "a");
    app.world_mut().entity_mut(body).remove::<RouteScroll>();
    scroll(&mut app, body, Vec2::new(20.0, 100.0));
    push(&mut app, router, "/a");
    app.world_mut()
        .entity_mut(body)
        .insert(RouteScroll::new("body"));
    scroll(&mut app, body, Vec2::new(80.0, 500.0));
    back(&mut app, router);
    assert_eq!(page(&mut app, "a"), body);
    assert_position(&app, body, Vec2::new(80.0, 500.0));
}

#[test]
fn restoration_saves_displayed_logical_pixels_across_camera_scale_changes() {
    let Fixture {
        mut app,
        router,
        camera,
        content_size,
        ..
    } = fixture(FixtureOptions::default());
    let camera = camera.unwrap();
    app.world_mut()
        .get_mut::<Camera>(camera)
        .unwrap()
        .computed
        .target_info
        .as_mut()
        .unwrap()
        .scale_factor = 2.0;
    push(&mut app, router, "/a");
    let first = page(&mut app, "a");
    app.world_mut().get_mut::<ScrollPosition>(first).unwrap().0 = Vec2::new(900.0, 1200.0);
    app.update();
    assert_vec(logical_position(&app, first), Vec2::new(440.0, 680.0));
    assert_vec(
        app.world()
            .get::<ComputedNode>(first)
            .unwrap()
            .scroll_position,
        Vec2::new(880.0, 1360.0),
    );
    push(&mut app, router, "/b");
    *content_size.lock().unwrap() = Vec2::new(1200.0, 1800.0);
    app.world_mut()
        .get_mut::<Camera>(camera)
        .unwrap()
        .computed
        .target_info
        .as_mut()
        .unwrap()
        .scale_factor = 1.5;
    back(&mut app, router);
    let restored = page(&mut app, "a");
    assert_position(&app, restored, Vec2::new(440.0, 680.0));
    assert_vec(
        app.world()
            .get::<ComputedNode>(restored)
            .unwrap()
            .scroll_position,
        Vec2::new(660.0, 1020.0),
    );
}

#[test]
fn failure_presentation_cannot_overwrite_the_departed_normal_entry_position() {
    let (mut app, router) = app();
    push(&mut app, router, "/a");
    let normal = page(&mut app, "a");
    scroll(&mut app, normal, Vec2::new(70.0, 240.0));
    let before = history(&app, router);
    let request = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, RouteLocation::new("/broken"))
        .unwrap();
    let status = settle_status(&mut app, router, request);
    assert!(
        matches!(status, RequestStatus::Failed(ref failure) if failure.kind() == NavigationFailureKind::Build),
        "{status:?}"
    );
    assert_eq!(history(&app, router), before);
    assert_eq!(
        app.world()
            .resource::<RouterSnapshots>()
            .get(router)
            .unwrap()
            .active()
            .kind(),
        ActivePresentationKind::Error
    );
    assert!(app.world().get_entity(normal).is_err());
    let fallback = page(&mut app, "fallback");
    scroll(&mut app, fallback, Vec2::new(90.0, 450.0));
    push(&mut app, router, "/b");
    back(&mut app, router);
    let restored = page(&mut app, "a");
    assert_position(&app, restored, Vec2::new(70.0, 240.0));
}

#[test]
fn failed_navigation_that_preserves_normal_content_keeps_its_scroll_untouched() {
    let Fixture {
        mut app, router, ..
    } = fixture(FixtureOptions {
        fallback_fails: true,
        ..Default::default()
    });
    push(&mut app, router, "/a");
    let normal = page(&mut app, "a");
    scroll(&mut app, normal, Vec2::new(70.0, 240.0));
    let before = history(&app, router);
    let request = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, RouteLocation::new("/broken"))
        .unwrap();
    let status = settle_status(&mut app, router, request);
    assert!(
        matches!(status, RequestStatus::Failed(ref failure) if failure.kind() == NavigationFailureKind::RecoveryFailed),
        "{status:?}"
    );
    assert_eq!(history(&app, router), before);
    assert_eq!(page(&mut app, "a"), normal);
    assert_position(&app, normal, Vec2::new(70.0, 240.0));
    push(&mut app, router, "/b");
    back(&mut app, router);
    let restored = page(&mut app, "a");
    assert_position(&app, restored, Vec2::new(70.0, 240.0));
}

#[test]
fn canonical_entry_positions_survive_host_loss_and_readmission() {
    let Fixture {
        mut app,
        router,
        host,
        ..
    } = fixture(FixtureOptions::default());
    push(&mut app, router, "/a");
    let normal = page(&mut app, "a");
    scroll(&mut app, normal, Vec2::new(70.0, 240.0));
    push(&mut app, router, "/b");
    let old_page = page(&mut app, "b");
    scroll(&mut app, old_page, Vec2::new(33.0, 88.0));
    let secondary = container(&mut app, "secondary");
    scroll(&mut app, secondary, Vec2::new(25.0, 130.0));
    // A partial layout observation must not erase another container's last
    // valid display. The primary remains eligible while this one has no layout.
    app.world_mut()
        .entity_mut(secondary)
        .remove::<bevy_ui::UiGlobalTransform>();
    app.update();
    let before = history(&app, router);
    let old_host = app
        .world()
        .resource::<RouterSnapshots>()
        .get(router)
        .unwrap()
        .host()
        .version()
        .unwrap()
        .clone();
    app.world_mut()
        .entity_mut(host)
        .remove::<RouterHostRegistration>();
    drive_availability(
        &mut app,
        router,
        RouterAvailability::Unavailable(RouterUnavailableReason::HostLost),
    );
    assert_eq!(history(&app, router), before);
    assert!(
        !app.world()
            .resource::<RouterInputGate>()
            .allows_keyboard(old_page)
    );
    app.world_mut()
        .entity_mut(host)
        .insert(RouterHostRegistration::new(router));
    drive_availability(&mut app, router, RouterAvailability::Ready);
    assert_ne!(
        app.world()
            .resource::<RouterSnapshots>()
            .get(router)
            .unwrap()
            .host()
            .version(),
        Some(&old_host)
    );
    push(&mut app, router, "/c");
    back(&mut app, router);
    let restored_b = page(&mut app, "b");
    assert_position(&app, restored_b, Vec2::new(33.0, 88.0));
    let restored_secondary = container(&mut app, "secondary");
    assert_position(&app, restored_secondary, Vec2::new(25.0, 130.0));
    back(&mut app, router);
    let restored = page(&mut app, "a");
    assert_position(&app, restored, Vec2::new(70.0, 240.0));
}

#[test]
fn pixel_layout_without_a_camera_does_not_create_a_restoration_record() {
    let initial = Vec2::new(11.0, 27.0);
    let Fixture {
        mut app, router, ..
    } = fixture(FixtureOptions {
        camera: false,
        initial,
        ..Default::default()
    });
    push(&mut app, router, "/a");
    let first = page(&mut app, "a");
    app.world_mut().get_mut::<ScrollPosition>(first).unwrap().0 = Vec2::new(70.0, 240.0);
    app.update();
    let node = app.world().get::<ComputedNode>(first).unwrap();
    assert!(
        node.size().min_element() > 0.0,
        "Px layout still computes without a render target"
    );
    assert_vec(
        node.scroll_position * node.inverse_scale_factor,
        Vec2::new(70.0, 240.0),
    );
    assert_eq!(
        app.world()
            .get::<ComputedUiRenderTargetInfo>(first)
            .unwrap()
            .physical_size(),
        UVec2::ZERO
    );
    push(&mut app, router, "/b");
    back(&mut app, router);
    let restored = page(&mut app, "a");
    assert_vec(
        app.world().get::<ScrollPosition>(restored).unwrap().0,
        initial,
    );
}

#[test]
fn restoration_settlement_rejects_a_content_root_reparented_after_publication() {
    let Fixture {
        mut app,
        router,
        host,
        content_size,
        ..
    } = fixture(FixtureOptions::default());
    push(&mut app, router, "/a");
    let normal = page(&mut app, "a");
    scroll(&mut app, normal, Vec2::new(70.0, 240.0));
    push(&mut app, router, "/b");
    *content_size.lock().unwrap() = Vec2::new(180.0, 160.0);
    let host_version = app
        .world()
        .resource::<RouterSnapshots>()
        .get(router)
        .unwrap()
        .host()
        .version()
        .unwrap()
        .clone();
    let application_root = app
        .world_mut()
        .spawn(Node {
            width: Val::Px(160.0),
            height: Val::Px(120.0),
            ..Default::default()
        })
        .id();
    let moved = Arc::new(Mutex::new(None));
    let observed = Arc::clone(&moved);
    app.add_systems(
        PostUpdate,
        (move |world: &mut World| {
            if observed.lock().unwrap().is_some() {
                return;
            }
            let published = world
                .resource::<RouterSnapshots>()
                .get(router)
                .unwrap()
                .active()
                .location()
                == Some(&RouteLocation::new("/a"));
            if !published {
                return;
            }
            let entity = world
                .query::<(Entity, &Page)>()
                .iter(world)
                .find_map(|(entity, page)| (page.0 == "a").then_some(entity))
                .unwrap();
            let requested = world.get::<ScrollPosition>(entity).unwrap().0;
            world.entity_mut(entity).insert(ChildOf(application_root));
            *observed.lock().unwrap() = Some((entity, requested));
        })
        .before(UiSystems::Layout),
    );
    back(&mut app, router);
    let (restored, before_layout) = moved
        .lock()
        .unwrap()
        .expect("the application moved the published content root before layout");
    assert_vec(before_layout, Vec2::new(70.0, 240.0));
    assert_eq!(
        app.world().get::<ChildOf>(restored).unwrap().parent(),
        application_root
    );
    assert_vec(logical_position(&app, restored), Vec2::new(20.0, 40.0));
    // Full binding validation must abandon settlement, rather than writing the
    // new clamp to content that is no longer in the published active branch.
    assert_vec(
        app.world().get::<ScrollPosition>(restored).unwrap().0,
        before_layout,
    );
    assert!(app.world().get::<RouterHostRegistration>(host).is_some());
    assert_eq!(
        app.world()
            .resource::<RouterSnapshots>()
            .get(router)
            .unwrap()
            .host()
            .version(),
        Some(&host_version)
    );
}

#[test]
fn a_missing_geometry_component_abandons_restoration_for_the_container_or_ancestor() {
    let initial = Vec2::new(11.0, 27.0);
    for remove_from_ancestor in [false, true] {
        let Fixture {
            mut app, router, ..
        } = fixture(FixtureOptions {
            initial,
            ..Default::default()
        });
        push(&mut app, router, "/a");
        let first = page(&mut app, "a");
        scroll(&mut app, first, Vec2::new(70.0, 240.0));
        push(&mut app, router, "/b");
        let mut removed = false;
        app.add_systems(
            PostUpdate,
            (move |world: &mut World| {
                if removed {
                    return;
                }
                let target = world
                    .query::<(Entity, &Page)>()
                    .iter(world)
                    .find_map(|(entity, page)| (page.0 == "a").then_some(entity));
                let Some(mut target) = target else {
                    return;
                };
                if remove_from_ancestor {
                    target = world
                        .query_filtered::<Entity, With<DefaultOutlet>>()
                        .single(world)
                        .unwrap();
                }
                world.entity_mut(target).remove::<UiTransform>();
                removed = true;
            })
            .before(UiSystems::Layout),
        );
        back(&mut app, router);
        let restored = page(&mut app, "a");
        assert_vec(
            app.world().get::<ScrollPosition>(restored).unwrap().0,
            initial,
        );
    }
}

#[test]
fn losing_the_camera_at_restoration_keeps_the_scene_initial_position() {
    let Fixture {
        mut app,
        router,
        camera,
        ..
    } = fixture(FixtureOptions::default());
    push(&mut app, router, "/a");
    let first = page(&mut app, "a");
    scroll(&mut app, first, Vec2::new(70.0, 240.0));
    push(&mut app, router, "/b");
    app.world_mut().despawn(camera.unwrap());
    back(&mut app, router);
    let restored = page(&mut app, "a");
    assert_vec(
        app.world().get::<ScrollPosition>(restored).unwrap().0,
        Vec2::ZERO,
    );
    app.update();
    assert_vec(
        app.world().get::<ScrollPosition>(restored).unwrap().0,
        Vec2::ZERO,
    );
}
