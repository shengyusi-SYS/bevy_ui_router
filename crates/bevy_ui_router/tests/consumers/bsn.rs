// Runs in a separate workspace with only the facade and official Bevy crates.
// All Router progress uses ordinary App::update and stable public snapshots.
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::AssetPlugin;
use bevy_ecs::prelude::*;
use bevy_scene::{ResolvedScene, SceneFunction, ScenePlugin, bsn};
use bevy_ui::{Node, Val};
use bevy_ui_router::prelude::*;

#[derive(Component)]
struct HostSentinel(u32);

#[derive(Component, Clone, Default)]
struct NativeLayout;

#[derive(Component, Clone, Default)]
struct ToolbarState(u32);

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

fn settle(app: &mut App, router: RouterHandle, request: NavigationRequestId) -> RequestStatus {
    for _ in 0..32 {
        app.update();
        if let RequestLookup::Found(snapshot) =
            app.world().resource::<RouterSnapshots>().request(router, request)
        {
            if matches!(snapshot.status(), RequestStatus::Succeeded | RequestStatus::Failed(_)) {
                return snapshot.status().clone();
            }
        }
    }
    panic!(
        "native consumer request did not settle: {:?}",
        app.world().resource::<RouterSnapshots>().request(router, request)
    );
}

fn only<T: Component>(app: &mut App) -> Entity {
    app.world_mut().query_filtered::<Entity, With<T>>().single(app.world()).unwrap()
}

fn assert_preserved_layout(app: &mut App, layout: Entity, anchor: Entity, toolbar: Entity) {
    assert_eq!(only::<NativeLayout>(app), layout);
    assert_eq!(only::<DefaultOutlet>(app), anchor);
    assert_eq!(only::<ToolbarState>(app), toolbar);
    assert_eq!(app.world().get::<ToolbarState>(toolbar).unwrap().0, 73);
    assert_eq!(app.world().get::<ChildOf>(anchor).map(ChildOf::parent), Some(layout));
    assert_eq!(app.world().get::<ChildOf>(toolbar).map(ChildOf::parent), Some(layout));
    assert_eq!(app.world().get::<Node>(layout).unwrap().width, Val::Px(311.0));
}

fn assert_leaf(app: &mut App, item: &str, instance: usize) -> Entity {
    let entity = only::<NativeLeaf>(app);
    let leaf = app.world().get::<NativeLeaf>(entity).unwrap();
    assert_eq!(leaf.item, item);
    assert_eq!(leaf.instance, instance);
    assert!(app.world().resource::<RouterInputGate>().allows_keyboard(entity));
    entity
}

fn main() {
    let mut app = App::new();
    app.add_plugins((TaskPoolPlugin::default(), AssetPlugin::default(), ScenePlugin));
    let calls = Arc::new(FactoryCalls::default());
    let factory_calls = Arc::clone(&calls);
    let layout_driver = app.register_bsn_view(ViewDriverKey::new("layout").unwrap(), move |_| {
        factory_calls.layout.fetch_add(1, Ordering::SeqCst);
        Ok(bsn! {
            Node { width: Val::Px(311.0) }
            NativeLayout
            Children [
                (Node { height: Val::Px(32.0) } ToolbarState),
                (Node { flex_grow: 1.0 } DefaultOutlet)
            ]
        })
    }).unwrap();
    let factory_calls = Arc::clone(&calls);
    let leaf_driver = app.register_bsn_view(ViewDriverKey::new("leaf").unwrap(), move |input| {
        let item = input.params().get("id").expect("leaf route-local id").to_owned();
        let instance = factory_calls.leaf.fetch_add(1, Ordering::SeqCst) + 1;
        Ok(SceneFunction(move |_, root: &mut ResolvedScene| {
            root.push_template(Node { width: Val::Px(47.0), ..Node::default() });
            root.push_template(NativeLeaf { item, instance });
        }))
    }).unwrap();
    let factory_calls = Arc::clone(&calls);
    let broken_driver = app.register_bsn_view(ViewDriverKey::new("broken").unwrap(), move |_| {
        let attempt = factory_calls.broken.fetch_add(1, Ordering::SeqCst) + 1;
        let template = FailTemplate {
            message: format!("consumer authored child failure {attempt}"),
            calls: Arc::clone(&factory_calls),
        };
        Ok(SceneFunction(move |_, root: &mut ResolvedScene| {
            root.push_template(Node::default());
            let mut child = ResolvedScene::default();
            child.push_template(Node { width: Val::Px(913.0), ..Node::default() });
            child.push_template(PartialFailedChild);
            child.push_bundle_template(template);
            root.get_or_insert_related_resolved_scenes::<ChildOf>().scenes.push(child);
        }))
    }).unwrap();
    let received_failures = Arc::new(Mutex::new(Vec::new()));
    let received = Arc::clone(&received_failures);
    let factory_calls = Arc::clone(&calls);
    let fallback_driver = app.register_bsn_view(ViewDriverKey::new("fallback").unwrap(), move |input| {
        factory_calls.fallback.fetch_add(1, Ordering::SeqCst);
        let failure = input.failure().expect("fallback receives original failure");
        received.lock().unwrap().push((failure.kind(), failure.message().to_owned()));
        Ok(bsn! { Node { width: Val::Px(71.0) } ErrorPage })
    }).unwrap();

    let layout = RouteViewKey::new("layout").unwrap();
    let leaf = RouteViewKey::new("leaf").unwrap();
    let broken = RouteViewKey::new("broken").unwrap();
    let fallback = RouteViewKey::new("fallback").unwrap();
    let mut routes = RouteTree::builder()
        .root_with_options(RoutePattern::path("/").unwrap(), RouteNodeOptions::new().with_view(layout.clone()))
        .unwrap();
    let root = routes.root_id().unwrap();
    routes.route_with_options(root, RoutePattern::path("items/:id").unwrap(), RouteNodeOptions::new().with_view(leaf.clone())).unwrap();
    routes.route_with_options(root, RoutePattern::path("broken").unwrap(), RouteNodeOptions::new().with_view(broken.clone())).unwrap();
    let mut views = RouteViewRegistry::new();
    for (view, driver) in [
        (layout, layout_driver), (leaf, leaf_driver),
        (broken, broken_driver), (fallback.clone(), fallback_driver),
    ] {
        views.insert(view, RouteViewDescriptor::new(driver)).unwrap();
    }
    let router = app.add_ui_router(RouterConfig::new(routes.build().unwrap(), views, fallback)).unwrap();
    let host = app.world_mut().spawn((
        Node { width: Val::Px(487.0), ..Node::default() },
        HostSentinel(7), RouterHostRegistration::new(router),
    )).id();
    let app_sibling = app.world_mut().spawn((
        Node { width: Val::Px(83.0), ..Node::default() }, ChildOf(host),
    )).id();
    app.update();

    let first = app.world_mut().resource_mut::<RouterNavigation>().push(router, RouteLocation::new("/items/a")).unwrap();
    assert_eq!(settle(&mut app, router, first), RequestStatus::Succeeded);
    let layout = only::<NativeLayout>(&mut app);
    let anchor = only::<DefaultOutlet>(&mut app);
    let toolbar = only::<ToolbarState>(&mut app);
    app.world_mut().get_mut::<ToolbarState>(toolbar).unwrap().0 = 73;
    let first_leaf = assert_leaf(&mut app, "a", 1);
    assert_preserved_layout(&mut app, layout, anchor, toolbar);

    let pushed = app.world_mut().resource_mut::<RouterNavigation>().push(router, RouteLocation::new("/items/b")).unwrap();
    assert_eq!(settle(&mut app, router, pushed), RequestStatus::Succeeded);
    let second_leaf = assert_leaf(&mut app, "b", 2);
    assert_ne!(second_leaf, first_leaf);
    assert!(app.world().get_entity(first_leaf).is_err());
    assert_preserved_layout(&mut app, layout, anchor, toolbar);
    assert_eq!(app.world().resource::<RouterSnapshots>().get(router).unwrap().history().entries(), &[RouteLocation::new("/items/a"), RouteLocation::new("/items/b")]);

    let replaced = app.world_mut().resource_mut::<RouterNavigation>().replace(router, RouteLocation::new("/items/c")).unwrap();
    assert_eq!(settle(&mut app, router, replaced), RequestStatus::Succeeded);
    let third_leaf = assert_leaf(&mut app, "c", 3);
    assert_ne!(third_leaf, second_leaf);
    assert!(app.world().get_entity(second_leaf).is_err());
    assert_preserved_layout(&mut app, layout, anchor, toolbar);
    assert_eq!(app.world().resource::<RouterSnapshots>().get(router).unwrap().history().entries(), &[RouteLocation::new("/items/a"), RouteLocation::new("/items/c")]);

    let back = app.world_mut().resource_mut::<RouterNavigation>().back(router).unwrap();
    assert_eq!(settle(&mut app, router, back), RequestStatus::Succeeded);
    let returned_leaf = assert_leaf(&mut app, "a", 4);
    assert_ne!(returned_leaf, third_leaf);
    assert_ne!(returned_leaf, first_leaf, "back creates a fresh Scene rather than retaining the old UI");
    assert!(app.world().get_entity(third_leaf).is_err());
    assert_preserved_layout(&mut app, layout, anchor, toolbar);
    assert_eq!(calls.layout.load(Ordering::SeqCst), 1);
    assert_eq!(calls.leaf.load(Ordering::SeqCst), 4);
    assert_eq!(calls.fallback.load(Ordering::SeqCst), 0);
    let history_before_failure = app.world().resource::<RouterSnapshots>().get(router).unwrap().history().clone();
    assert_eq!(history_before_failure.current(), Some(&RouteLocation::new("/items/a")));
    assert_eq!(history_before_failure.cursor(), Some(0));

    let broken = app.world_mut().resource_mut::<RouterNavigation>().push(router, RouteLocation::new("/broken")).unwrap();
    let RequestStatus::Failed(failure) = settle(&mut app, router, broken) else {
        panic!("a real failing Scene template must fail navigation")
    };
    assert_eq!(failure.kind(), NavigationFailureKind::Apply, "{:?}", app.world().resource::<RouterSnapshots>().request(router, broken));
    assert!(failure.message().contains("consumer authored child failure 1"));
    assert_eq!(*received_failures.lock().unwrap(), vec![(failure.kind(), failure.message().to_owned())]);
    assert_eq!(calls.broken.load(Ordering::SeqCst), 1);
    assert_eq!(calls.failing_template.load(Ordering::SeqCst), 1);
    assert_eq!(calls.fallback.load(Ordering::SeqCst), 1);
    assert_eq!(calls.layout.load(Ordering::SeqCst), 1);
    assert_eq!(calls.leaf.load(Ordering::SeqCst), 4);
    let snapshots = app.world().resource::<RouterSnapshots>();
    let snapshot = snapshots.get(router).unwrap();
    assert_eq!(snapshot.active().kind(), ActivePresentationKind::Error);
    assert_eq!(snapshot.history(), &history_before_failure);
    let error_page = only::<ErrorPage>(&mut app);
    assert!(app.world().resource::<RouterInputGate>().allows_keyboard(error_page));
    assert!(app.world_mut().query_filtered::<Entity, With<PartialFailedChild>>().iter(app.world()).next().is_none());
    assert!(!app.world_mut().query::<&Node>().iter(app.world()).any(|node| node.width == Val::Px(913.0)));
    assert_eq!(app.world().get::<HostSentinel>(host).unwrap().0, 7);
    assert_eq!(app.world().get::<Node>(host).unwrap().width, Val::Px(487.0));
    assert_eq!(app.world().get::<ChildOf>(app_sibling).map(ChildOf::parent), Some(host));
    assert_eq!(app.world().get::<Node>(app_sibling).unwrap().width, Val::Px(83.0));
}
