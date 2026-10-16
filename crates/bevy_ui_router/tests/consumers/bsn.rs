// Runs in a separate workspace with only the facade and official Bevy crates.
// All Router progress uses ordinary App::update and stable public snapshots.
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

use bevy_app::{App, TaskPoolPlugin, Update};
use bevy_asset::AssetPlugin;
use bevy_ecs::prelude::*;
use bevy_ecs::system::RunSystemOnce;
use bevy_scene::{ResolvedScene, SceneFunction, ScenePlugin, bsn};
use bevy_ui::{Node, Val};
use bevy_ui_router::ecs::RouterScheduleSet;
use bevy_ui_router::prelude::*;

#[derive(Component)]
struct HostSentinel(u32);

#[derive(Component, Clone, Default)]
struct NativeLayout;

#[derive(Component, Clone, Default)]
struct ToolbarState(u32, Option<RouteLocation>);

#[derive(Component, Clone, Default)]
struct NativeLeaf {
    item: String,
    instance: usize,
}

#[derive(Component, Clone, Default)]
struct ErrorPage;

#[derive(Component, Clone, Default)]
struct PartialFailedChild;

#[derive(Default)]
struct FactoryCalls {
    layout: AtomicUsize,
    leaf: AtomicUsize,
    broken: AtomicUsize,
    fallback: AtomicUsize,
    failing_template: AtomicUsize,
}

#[derive(Clone)]
struct FailTemplate {
    message: String,
    calls: Arc<FactoryCalls>,
}

impl bevy_ecs::template::Template for FailTemplate {
    type Output = ();

    fn build_template(
        &self,
        _: &mut bevy_ecs::template::TemplateContext<'_, '_>,
    ) -> bevy_ecs::error::Result<()> {
        self.calls.failing_template.fetch_add(1, Ordering::SeqCst);
        Err(bevy_ecs::error::BevyError::error(self.message.clone()))
    }

    fn clone_template(&self) -> Self {
        self.clone()
    }
}

#[derive(Clone, Copy)]
enum ExpectedNative {
    Leaf(usize),
    Error,
}

#[derive(Resource)]
struct TrackedRequest {
    router: RouterHandle,
    request: NavigationRequestId,
    native: ExpectedNative,
}

#[derive(Clone)]
struct UpdateObservation {
    request: Option<NavigationRequestSnapshot>,
    router: RouterSnapshot,
    native: Option<Entity>,
}

#[derive(Resource, Default)]
struct SnapshotObservation(Option<UpdateObservation>);

fn published_native<'a>(
    expected: ExpectedNative,
    gate: &RouterInputGate,
    entities: impl Iterator<Item = (Entity, Option<&'a NativeLeaf>, bool)>,
) -> Option<Entity> {
    let mut published = entities.filter_map(|(entity, leaf, error)| {
        let matches = match expected {
            ExpectedNative::Leaf(instance) => leaf.is_some_and(|leaf| leaf.instance == instance),
            ExpectedNative::Error => error,
        };
        (matches && gate.allows_keyboard(entity)).then_some(entity)
    });
    let entity = published.next();
    assert!(
        published.next().is_none(),
        "only one expected native target may be active"
    );
    entity
}

fn observe_snapshots(
    tracked: Option<Res<TrackedRequest>>,
    snapshots: Res<RouterSnapshots>,
    activity: RouteActivity,
    inputs: RouteInputs,
    mut toolbar: Query<(Entity, &mut ToolbarState)>,
    gate: Res<RouterInputGate>,
    native: Query<(Entity, Option<&NativeLeaf>, Has<ErrorPage>)>,
    mut observation: ResMut<SnapshotObservation>,
) {
    let Some(tracked) = tracked else { return };
    let request = match snapshots.request(tracked.router, tracked.request) {
        RequestLookup::Found(snapshot) => Some(snapshot.clone()),
        RequestLookup::Unknown => None,
        RequestLookup::NotRetained => panic!("tracked request was evicted"),
    };
    let published = published_native(tracked.native, &gate, native.iter());
    let router = snapshots.get(tracked.router).unwrap();
    for (entity, mut state) in &mut toolbar {
        state.1 = inputs.current(entity).map(|input| {
            assert!(input.params().is_empty(), "parent parameters stay route-local");
            assert!(input.failure().is_none());
            assert_eq!(Some(input.location()), router.active().location());
            input.location().clone()
        });
    }
    if let Some(entity) = published {
        assert!(activity.is_active(entity));
        let input = inputs.current(entity).expect("input is published with native content");
        assert_eq!(Some(input.location()), router.active().location().or_else(|| router.active().requested()));
        assert_eq!(input.origin(), router.active().frames().last().unwrap().origin());
        assert_eq!(input.failure(), router.active().failure());
        match tracked.native {
            ExpectedNative::Leaf(_) => {
                assert!(input.route().is_some());
                assert_eq!(input.params().get("id"), Some(native.get(entity).unwrap().1.unwrap().item.as_str()));
            },
            ExpectedNative::Error => {
                assert_eq!(input.route(), None);
                assert!(input.params().is_empty());
            },
        }
    }
    observation.0 = Some(UpdateObservation {
        request,
        router: snapshots.get(tracked.router).unwrap().clone(),
        native: published,
    });
}

fn observed_router(app: &App) -> &RouterSnapshot {
    &app.world()
        .resource::<SnapshotObservation>()
        .0
        .as_ref()
        .unwrap()
        .router
}

fn settle(
    app: &mut App,
    router: RouterHandle,
    request: NavigationRequestId,
    native: ExpectedNative,
) -> RequestStatus {
    app.insert_resource(TrackedRequest {
        router,
        request,
        native,
    });
    for _ in 0..32 {
        // A fresh sample is required on every update, including initial/pending
        // frames; an earlier terminal observation cannot settle this request.
        app.world_mut().resource_mut::<SnapshotObservation>().0 = None;
        app.update();
        let observed = app
            .world()
            .resource::<SnapshotObservation>()
            .0
            .clone()
            .expect("application snapshot observer must execute in this Update");
        let snapshots = app.world().resource::<RouterSnapshots>();
        assert_eq!(Some(&observed.router), snapshots.get(router));
        match snapshots.request(router, request) {
            RequestLookup::Found(snapshot) => assert_eq!(observed.request.as_ref(), Some(snapshot)),
            RequestLookup::Unknown => assert!(observed.request.is_none()),
            RequestLookup::NotRetained => panic!("tracked request was evicted"),
        }
        let mut query = app
            .world_mut()
            .query::<(Entity, Option<&NativeLeaf>, Has<ErrorPage>)>();
        let post_native = published_native(
            native,
            app.world().resource::<RouterInputGate>(),
            query.iter(app.world()),
        );
        assert_eq!(
            observed.native, post_native,
            "native publication must already be visible to the Update observer"
        );
        let status = observed
            .request
            .as_ref()
            .map(NavigationRequestSnapshot::status);
        let terminal = matches!(
            status,
            Some(RequestStatus::Succeeded | RequestStatus::Failed(_))
        );
        assert_eq!(
            terminal,
            observed.native.is_some(),
            "snapshots and native publication must agree in the same update: {status:?}"
        );
        if terminal {
            return status.unwrap().clone();
        }
    }
    panic!(
        "native consumer request did not settle: {:?}",
        app.world()
            .resource::<RouterSnapshots>()
            .request(router, request)
    );
}

fn only<T: Component>(app: &mut App) -> Entity {
    app.world_mut()
        .query_filtered::<Entity, With<T>>()
        .single(app.world())
        .unwrap()
}

fn assert_preserved_layout(app: &mut App, layout: Entity, anchor: Entity, toolbar: Entity) {
    assert_eq!(only::<NativeLayout>(app), layout);
    assert_eq!(only::<DefaultOutlet>(app), anchor);
    assert_eq!(only::<ToolbarState>(app), toolbar);
    assert_eq!(app.world().get::<ToolbarState>(toolbar).unwrap().0, 73);
    assert_eq!(app.world().get::<ToolbarState>(toolbar).unwrap().1.as_ref(), observed_router(app).active().location());
    assert_eq!(
        app.world().get::<ChildOf>(anchor).map(ChildOf::parent),
        Some(layout)
    );
    assert_eq!(
        app.world().get::<ChildOf>(toolbar).map(ChildOf::parent),
        Some(layout)
    );
    assert_eq!(
        app.world().get::<Node>(layout).unwrap().width,
        Val::Px(311.0)
    );
}

fn assert_leaf(app: &mut App, item: &str, instance: usize) -> Entity {
    let entity = only::<NativeLeaf>(app);
    let leaf = app.world().get::<NativeLeaf>(entity).unwrap();
    assert_eq!(leaf.item, item);
    assert_eq!(leaf.instance, instance);
    assert_eq!(
        app.world().get::<RouteScroll>(entity),
        Some(&RouteScroll::new("body").with_restore_policy(RouteScrollRestorePolicy::Explicit))
    );
    assert!(
        app.world()
            .resource::<RouterInputGate>()
            .allows_keyboard(entity)
    );
    entity
}

fn main() {
    let mut app = App::new();
    app.add_plugins((
        TaskPoolPlugin::default(),
        AssetPlugin::default(),
        ScenePlugin,
    ));
    let calls = Arc::new(FactoryCalls::default());
    let factory_calls = Arc::clone(&calls);
    let layout_driver = app
        .register_bsn_view(ViewDriverKey::new("layout").unwrap(), move |_| {
            factory_calls.layout.fetch_add(1, Ordering::SeqCst);
            Ok(bsn! {
                Node { width: Val::Px(311.0) }
                NativeLayout
                Children [
                    (Node { height: Val::Px(32.0) } ToolbarState),
                    (Node { flex_grow: 1.0 } DefaultOutlet)
                ]
            })
        })
        .unwrap();
    let factory_calls = Arc::clone(&calls);
    let leaf_driver = app
        .register_bsn_view(ViewDriverKey::new("leaf").unwrap(), move |input| {
            let item = input
                .params()
                .get("id")
                .expect("leaf route-local id")
                .to_owned();
            let instance = factory_calls.leaf.fetch_add(1, Ordering::SeqCst) + 1;
            Ok(SceneFunction(move |_, root: &mut ResolvedScene| {
                root.push_template(Node {
                    width: Val::Px(47.0),
                    ..Node::default()
                });
                root.push_template(NativeLeaf { item, instance });
                root.push_template(
                    RouteScroll::new("body").with_restore_policy(RouteScrollRestorePolicy::Explicit),
                );
            }))
        })
        .unwrap();
    let factory_calls = Arc::clone(&calls);
    let broken_driver = app
        .register_bsn_view(ViewDriverKey::new("broken").unwrap(), move |_| {
            let attempt = factory_calls.broken.fetch_add(1, Ordering::SeqCst) + 1;
            let template = FailTemplate {
                message: format!("consumer authored child failure {attempt}"),
                calls: Arc::clone(&factory_calls),
            };
            Ok(SceneFunction(move |_, root: &mut ResolvedScene| {
                root.push_template(Node::default());
                let mut child = ResolvedScene::default();
                child.push_template(Node {
                    width: Val::Px(913.0),
                    ..Node::default()
                });
                child.push_template(PartialFailedChild);
                child.push_bundle_template(template);
                root.get_or_insert_related_resolved_scenes::<ChildOf>()
                    .scenes
                    .push(child);
            }))
        })
        .unwrap();
    let received_failures = Arc::new(Mutex::new(Vec::new()));
    let received = Arc::clone(&received_failures);
    let factory_calls = Arc::clone(&calls);
    let fallback_driver = app
        .register_bsn_view(ViewDriverKey::new("fallback").unwrap(), move |input| {
            factory_calls.fallback.fetch_add(1, Ordering::SeqCst);
            let failure = input.failure().expect("fallback receives original failure");
            received
                .lock()
                .unwrap()
                .push((failure.kind(), failure.message().to_owned()));
            Ok(bsn! { Node { width: Val::Px(71.0) } ErrorPage })
        })
        .unwrap();

    let layout = RouteViewKey::new("layout").unwrap();
    let leaf = RouteViewKey::new("leaf").unwrap();
    let broken = RouteViewKey::new("broken").unwrap();
    let fallback = RouteViewKey::new("fallback").unwrap();
    let mut routes = RouteTree::builder()
        .root_with_options(
            RoutePattern::path("/").unwrap(),
            RouteNodeOptions::new()
                .with_view(layout.clone())
                .with_preserve_on_search(true),
        )
        .unwrap();
    let root = routes.root_id().unwrap();
    routes
        .route_with_options(
            root,
            RoutePattern::path("items/:id").unwrap(),
            RouteNodeOptions::new().with_view(leaf.clone()),
        )
        .unwrap();
    routes
        .route_with_options(
            root,
            RoutePattern::path("broken").unwrap(),
            RouteNodeOptions::new().with_view(broken.clone()),
        )
        .unwrap();
    let mut views = RouteViewRegistry::new();
    for (view, driver) in [
        (layout, layout_driver),
        (leaf, leaf_driver),
        (broken, broken_driver),
        (fallback.clone(), fallback_driver),
    ] {
        views
            .insert(view, RouteViewDescriptor::new(driver))
            .unwrap();
    }
    let router = app
        .add_ui_router(RouterConfig::new(routes.build().unwrap(), views, fallback))
        .unwrap();
    app.init_resource::<SnapshotObservation>().add_systems(
        Update,
        observe_snapshots.after(RouterScheduleSet::PublishSnapshots),
    );
    let host = app
        .world_mut()
        .spawn((
            Node {
                width: Val::Px(487.0),
                ..Node::default()
            },
            HostSentinel(7),
            RouterHostRegistration::new(router),
        ))
        .id();
    let app_sibling = app
        .world_mut()
        .spawn((
            Node {
                width: Val::Px(83.0),
                ..Node::default()
            },
            ChildOf(host),
        ))
        .id();
    app.update();

    let first = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, RouteLocation::new("/items/a"))
        .unwrap();
    assert_eq!(
        settle(&mut app, router, first, ExpectedNative::Leaf(1)),
        RequestStatus::Succeeded
    );
    assert_eq!(
        observed_router(&app).active().location(),
        Some(&RouteLocation::new("/items/a"))
    );
    assert_eq!(
        observed_router(&app).history().entries(),
        &[RouteLocation::new("/items/a")]
    );
    let layout = only::<NativeLayout>(&mut app);
    let anchor = only::<DefaultOutlet>(&mut app);
    let toolbar = only::<ToolbarState>(&mut app);
    app.world_mut().get_mut::<ToolbarState>(toolbar).unwrap().0 = 73;
    let first_leaf = assert_leaf(&mut app, "a", 1);
    assert_preserved_layout(&mut app, layout, anchor, toolbar);

    let pushed = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, RouteLocation::new("/items/b"))
        .unwrap();
    assert_eq!(
        settle(&mut app, router, pushed, ExpectedNative::Leaf(2)),
        RequestStatus::Succeeded
    );
    assert_eq!(
        observed_router(&app).active().location(),
        Some(&RouteLocation::new("/items/b"))
    );
    let second_leaf = assert_leaf(&mut app, "b", 2);
    assert_ne!(second_leaf, first_leaf);
    assert!(app.world().get_entity(first_leaf).is_err());
    assert_preserved_layout(&mut app, layout, anchor, toolbar);
    assert_eq!(
        observed_router(&app).history().entries(),
        &[
            RouteLocation::new("/items/a"),
            RouteLocation::new("/items/b")
        ]
    );

    let replaced = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .replace(router, RouteLocation::new("/items/c"))
        .unwrap();
    assert_eq!(
        settle(&mut app, router, replaced, ExpectedNative::Leaf(3)),
        RequestStatus::Succeeded
    );
    assert_eq!(
        observed_router(&app).active().location(),
        Some(&RouteLocation::new("/items/c"))
    );
    let third_leaf = assert_leaf(&mut app, "c", 3);
    assert_ne!(third_leaf, second_leaf);
    assert!(app.world().get_entity(second_leaf).is_err());
    assert_preserved_layout(&mut app, layout, anchor, toolbar);
    assert_eq!(
        observed_router(&app).history().entries(),
        &[
            RouteLocation::new("/items/a"),
            RouteLocation::new("/items/c")
        ]
    );

    let back = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .back(router)
        .unwrap();
    assert_eq!(
        settle(&mut app, router, back, ExpectedNative::Leaf(4)),
        RequestStatus::Succeeded
    );
    assert_eq!(
        observed_router(&app).active().location(),
        Some(&RouteLocation::new("/items/a"))
    );
    let returned_leaf = assert_leaf(&mut app, "a", 4);
    // This consumer has no UI layout plugin and therefore no displayed scroll
    // record. The same public lookup works without manufacturing an intent.
    assert!(
        app.world_mut()
            .run_system_once(move |restorations: RouteScrollRestorations| {
                restorations.pending(returned_leaf, "body")
            })
            .unwrap()
            .is_none()
    );
    assert_ne!(returned_leaf, third_leaf);
    assert_ne!(
        returned_leaf, first_leaf,
        "back creates a fresh Scene rather than retaining the old UI"
    );
    assert!(app.world().get_entity(third_leaf).is_err());
    assert_preserved_layout(&mut app, layout, anchor, toolbar);
    assert_eq!(calls.layout.load(Ordering::SeqCst), 1);
    assert_eq!(calls.leaf.load(Ordering::SeqCst), 4);
    assert_eq!(calls.fallback.load(Ordering::SeqCst), 0);
    let history_before_failure = observed_router(&app).history().clone();
    assert_eq!(
        history_before_failure.current(),
        Some(&RouteLocation::new("/items/a"))
    );
    assert_eq!(history_before_failure.cursor(), Some(0));

    let broken = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, RouteLocation::new("/broken"))
        .unwrap();
    let RequestStatus::Failed(failure) = settle(&mut app, router, broken, ExpectedNative::Error)
    else {
        panic!("a real failing Scene template must fail navigation")
    };
    assert_eq!(
        failure.kind(),
        NavigationFailureKind::Apply,
        "{:?}",
        app.world()
            .resource::<RouterSnapshots>()
            .request(router, broken)
    );
    assert!(
        failure
            .message()
            .contains("consumer authored child failure 1")
    );
    assert_eq!(
        *received_failures.lock().unwrap(),
        vec![(failure.kind(), failure.message().to_owned())]
    );
    assert_eq!(calls.broken.load(Ordering::SeqCst), 1);
    assert_eq!(calls.failing_template.load(Ordering::SeqCst), 1);
    assert_eq!(calls.fallback.load(Ordering::SeqCst), 1);
    assert_eq!(calls.layout.load(Ordering::SeqCst), 1);
    assert_eq!(calls.leaf.load(Ordering::SeqCst), 4);
    let snapshot = observed_router(&app);
    assert_eq!(snapshot.active().kind(), ActivePresentationKind::Error);
    assert_eq!(
        snapshot.active().requested(),
        Some(&RouteLocation::new("/broken"))
    );
    assert_eq!(snapshot.active().failure(), Some(&failure));
    assert_eq!(snapshot.history(), &history_before_failure);
    let error_page = only::<ErrorPage>(&mut app);
    assert!(
        app.world()
            .resource::<RouterInputGate>()
            .allows_keyboard(error_page)
    );
    assert!(
        app.world_mut()
            .query_filtered::<Entity, With<PartialFailedChild>>()
            .iter(app.world())
            .next()
            .is_none()
    );
    assert!(
        !app.world_mut()
            .query::<&Node>()
            .iter(app.world())
            .any(|node| node.width == Val::Px(913.0))
    );
    assert_eq!(app.world().get::<HostSentinel>(host).unwrap().0, 7);
    assert_eq!(app.world().get::<Node>(host).unwrap().width, Val::Px(487.0));
    assert_eq!(
        app.world().get::<ChildOf>(app_sibling).map(ChildOf::parent),
        Some(host)
    );
    assert_eq!(
        app.world().get::<Node>(app_sibling).unwrap().width,
        Val::Px(83.0)
    );

    // A long-lived application recovers, then replaces pages without accumulating
    // old UI. Count live entities, not allocator slots, which may legitimately grow.
    let restored = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .replace(router, RouteLocation::new("/items/restored"))
        .unwrap();
    assert_eq!(
        settle(&mut app, router, restored, ExpectedNative::Leaf(5)),
        RequestStatus::Succeeded
    );
    let layout = only::<NativeLayout>(&mut app);
    let anchor = only::<DefaultOutlet>(&mut app);
    let toolbar = only::<ToolbarState>(&mut app);
    app.world_mut().get_mut::<ToolbarState>(toolbar).unwrap().0 = 73;
    let mut previous = assert_leaf(&mut app, "restored", 5);
    let live_count = app.world().entities().count_spawned();
    for index in 0..96 {
        let item = format!("repeat-{index}");
        let location = RouteLocation::new(format!("/items/{item}"))
            .with_search(format!("sort={index}"));
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .replace(router, location.clone())
            .unwrap();
        assert_eq!(
            settle(&mut app, router, request, ExpectedNative::Leaf(index + 6)),
            RequestStatus::Succeeded
        );
        assert!(app.world().get_entity(previous).is_err());
        previous = assert_leaf(&mut app, &item, index + 6);
        assert_preserved_layout(&mut app, layout, anchor, toolbar);
        assert_eq!(app.world().entities().count_spawned(), live_count);
        let snapshot = observed_router(&app);
        assert_eq!(snapshot.active().location(), Some(&location));
        assert_eq!(snapshot.history().current(), Some(&location));
        assert_eq!(snapshot.history().entries().len(), 2);
        assert_eq!(snapshot.running_request(), None);
        assert!(!snapshot.cleanup().foreground_pending());
    }
    assert!(matches!(
        app.world()
            .resource::<RouterSnapshots>()
            .request(router, first),
        RequestLookup::NotRetained
    ));
    assert_eq!(calls.layout.load(Ordering::SeqCst), 2);
    assert_eq!(app.world().get::<HostSentinel>(host).unwrap().0, 7);
    assert_eq!(
        app.world().get::<ChildOf>(app_sibling).unwrap().parent(),
        host
    );
}
