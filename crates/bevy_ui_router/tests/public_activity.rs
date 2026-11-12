#![cfg(feature = "bsn")]

use bevy_app::{App, TaskPoolPlugin, Update};
use bevy_asset::AssetPlugin;
use bevy_ecs::{prelude::*, system::RunSystemOnce};
use bevy_scene::{ResolveContext, ResolveSceneError, ResolvedScene, Scene, ScenePlugin};
use bevy_ui::Node;
use bevy_ui_router::{ecs::RouterScheduleSet, prelude::*};

#[derive(Component, Clone, Default)]
struct AppWork {
    router: &'static str,
    page: u32,
    ticks: usize,
    active: bool,
    factory_location: Option<RouteLocation>,
    current_location: Option<RouteLocation>,
    current_params: Option<RouteParams>,
}

impl AppWork {
    fn new(router: &'static str, page: u32) -> Self {
        Self {
            router,
            page,
            ticks: 0,
            active: false,
            factory_location: None,
            current_location: None,
            current_params: None,
        }
    }
}

// This is an ordinary application system with mutable application state. The
// Router query must not borrow all components or depend on input eligibility.
fn perform_active_work(
    queries: RouteQueries,
    activity: RouteActivity,
    inputs: RouteInputs,
    mut work: Query<(Entity, &mut AppWork)>,
) {
    let routes = queries.read();
    for (entity, mut work) in &mut work {
        work.active = routes.is_active(entity);
        assert_eq!(work.active, activity.is_active(entity));
        let input = routes.current(entity);
        let original = inputs.current(entity);
        assert_eq!(
            input.as_ref().map(|i| i.location()),
            original.as_ref().map(|i| i.location())
        );
        assert_eq!(
            input.as_ref().map(|i| i.params()),
            original.as_ref().map(|i| i.params())
        );
        assert_eq!(
            input.is_some(),
            work.active,
            "activity and inputs must share exact qualification for {entity}"
        );
        work.current_location = input.as_ref().map(|input| input.location().clone());
        work.current_params = input.as_ref().map(|input| input.params().clone());
        if work.active {
            work.ticks += 1;
        }
    }
}

fn observe_current_inputs(queries: RouteQueries, mut work: Query<(Entity, &mut AppWork)>) {
    let inputs = queries.read();
    for (entity, mut work) in &mut work {
        let current = inputs.current(entity);
        work.current_location = current.as_ref().map(|input| input.location().clone());
        work.current_params = current.as_ref().map(|input| input.params().clone());
    }
}

struct PageScene {
    router: &'static str,
    page: u32,
    outlet: bool,
    factory_location: RouteLocation,
}

impl Scene for PageScene {
    fn resolve(
        self,
        _: &mut ResolveContext,
        root: &mut ResolvedScene,
    ) -> Result<(), ResolveSceneError> {
        root.push_template(Node::default());
        let children = root.get_or_insert_related_resolved_scenes::<ChildOf>();
        let mut background = ResolvedScene::default();
        // A non-visual content entity still belongs to the active page.
        let mut work = AppWork::new(self.router, self.page);
        work.factory_location = Some(self.factory_location);
        background.push_template(work);
        children.scenes.push(background);
        if self.outlet {
            let mut outlet = ResolvedScene::default();
            outlet.push_template(Node::default());
            outlet.push_template(
                <DefaultOutlet as bevy_ecs::template::FromTemplate>::Template::default(),
            );
            children.scenes.push(outlet);
        }
        Ok(())
    }
}

fn install_router(app: &mut App, name: &'static str) -> (RouterHandle, Entity) {
    let mut views = RouteViewRegistry::new();
    for (suffix, outlet) in [("layout", true), ("leaf", false), ("fallback", false)] {
        let key = format!("activity-{name}-{suffix}");
        let driver = app
            .register_bsn_view(ViewDriverKey::new(&key).unwrap(), move |input| {
                Ok(PageScene {
                    router: name,
                    page: if outlet {
                        0
                    } else {
                        input.params().get("id").unwrap_or("99").parse().unwrap()
                    },
                    outlet,
                    factory_location: input.location().clone(),
                })
            })
            .unwrap();
        views
            .insert(
                RouteViewKey::new(&key).unwrap(),
                RouteViewDescriptor::new(driver),
            )
            .unwrap();
    }
    let mut routes = RouteTree::builder()
        .root_with_options(
            RoutePattern::path("/orgs/:org").unwrap(),
            RouteNodeOptions::new()
                .with_view(RouteViewKey::new(format!("activity-{name}-layout")).unwrap()),
        )
        .unwrap();
    routes
        .route_with_options(
            routes.root_id().unwrap(),
            RoutePattern::path("items/:id").unwrap(),
            RouteNodeOptions::new()
                .with_view(RouteViewKey::new(format!("activity-{name}-leaf")).unwrap())
                .with_slot(RouteSlotKey::default())
                .with_keep_alive(true),
        )
        .unwrap();
    let router = app
        .add_ui_router(
            RouterConfig::new(
                routes.build().unwrap(),
                views,
                RouteViewKey::new(format!("activity-{name}-fallback")).unwrap(),
            )
            .with_keep_alive_capacity(2),
        )
        .unwrap();
    let host = app
        .world_mut()
        .spawn((
            Node::default(),
            RouterHostRegistration::new(router),
            AppWork::new(name, 100),
        ))
        .id();
    (router, host)
}

fn navigate(app: &mut App, router: RouterHandle, id: u32) {
    let request = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, RouteLocation::new(format!("/orgs/7/items/{id}")))
        .unwrap();
    for _ in 0..16 {
        app.update();
        let snapshots = app.world().resource::<RouterSnapshots>();
        let RequestLookup::Found(request) = snapshots.request(router, request) else {
            continue;
        };
        if matches!(request.status(), RequestStatus::Succeeded)
            && snapshots.get(router).unwrap().running_request().is_none()
        {
            return;
        }
    }
    panic!("activity fixture navigation must settle");
}

fn target(app: &mut App, router: &'static str, page: u32) -> Entity {
    let mut query = app.world_mut().query::<(Entity, &AppWork)>();
    let targets = query
        .iter(app.world())
        .filter_map(|(entity, work)| (work.router == router && work.page == page).then_some(entity))
        .collect::<Vec<_>>();
    assert_eq!(targets.len(), 1);
    targets[0]
}

fn work(app: &App, target: Entity) -> &AppWork {
    app.world().get::<AppWork>(target).unwrap()
}

#[test]
fn current_inputs_update_preserved_content_without_replacing_factory_snapshot() {
    let mut app = App::new();
    app.add_plugins((
        TaskPoolPlugin::default(),
        AssetPlugin::default(),
        ScenePlugin,
        BevyUiRouterPlugin,
    ));
    app.add_systems(
        Update,
        observe_current_inputs.after(RouterScheduleSet::PublishSnapshots),
    );
    let (router, host) = install_router(&mut app, "inputs");
    app.update();
    navigate(&mut app, router, 1);
    let parent = target(&mut app, "inputs", 0);
    let old_leaf = target(&mut app, "inputs", 1);
    let first = RouteLocation::new("/orgs/7/items/1");
    assert_eq!(work(&app, parent).current_location.as_ref(), Some(&first));
    assert_eq!(
        work(&app, parent)
            .current_params
            .as_ref()
            .unwrap()
            .get("org"),
        Some("7")
    );
    assert_eq!(
        work(&app, parent)
            .current_params
            .as_ref()
            .unwrap()
            .get("id"),
        None
    );
    assert_eq!(
        work(&app, old_leaf)
            .current_params
            .as_ref()
            .unwrap()
            .get("id"),
        Some("1")
    );
    assert_eq!(
        work(&app, old_leaf)
            .current_params
            .as_ref()
            .unwrap()
            .get("org"),
        None
    );
    assert!(work(&app, host).current_location.is_none());

    navigate(&mut app, router, 2);
    let leaf = target(&mut app, "inputs", 2);
    let second = RouteLocation::new("/orgs/7/items/2");
    assert_eq!(target(&mut app, "inputs", 0), parent);
    assert_eq!(work(&app, parent).current_location.as_ref(), Some(&second));
    assert_eq!(work(&app, leaf).current_location.as_ref(), Some(&second));
    assert_eq!(
        work(&app, leaf).current_params.as_ref().unwrap().get("id"),
        Some("2")
    );
    assert_eq!(work(&app, parent).factory_location.as_ref(), Some(&first));
    assert!(work(&app, old_leaf).current_location.is_none());
}

#[test]
fn nearest_inactive_registered_boundary_never_borrows_active_ancestor_inputs() {
    let mut app = App::new();
    app.add_plugins((
        TaskPoolPlugin::default(),
        AssetPlugin::default(),
        ScenePlugin,
        BevyUiRouterPlugin,
    ));
    let (router, _) = install_router(&mut app, "boundary");
    app.update();
    navigate(&mut app, router, 1);
    let old_leaf = target(&mut app, "boundary", 1);
    let old_root = app.world().get::<ChildOf>(old_leaf).unwrap().parent();
    navigate(&mut app, router, 2);
    let active_leaf = target(&mut app, "boundary", 2);
    let active_root = app.world().get::<ChildOf>(active_leaf).unwrap().parent();
    // A current ancestry path to active content is not ownership qualification.
    // Query immediately, before observation/cleanup can repair the changed edge.
    app.world_mut()
        .entity_mut(old_root)
        .insert(ChildOf(active_root));
    let dynamic = app
        .world_mut()
        .spawn((AppWork::new("boundary", 101), ChildOf(old_root)))
        .id();
    app.world_mut()
        .run_system_once(perform_active_work)
        .unwrap();
    assert!(!work(&app, old_leaf).active);
    assert!(!work(&app, dynamic).active);
    assert!(work(&app, old_leaf).current_location.is_none());
    assert!(work(&app, dynamic).current_location.is_none());
    assert!(work(&app, active_leaf).active);
    assert_eq!(
        work(&app, active_leaf).current_location.as_ref(),
        Some(&RouteLocation::new("/orgs/7/items/2"))
    );
}

#[test]
fn application_query_tracks_active_content_retention_and_exact_router_host() {
    let mut app = App::new();
    app.add_plugins((
        TaskPoolPlugin::default(),
        AssetPlugin::default(),
        ScenePlugin,
        BevyUiRouterPlugin,
    ));
    app.add_systems(
        Update,
        perform_active_work.after(RouterScheduleSet::PublishSnapshots),
    );
    let (first, first_host) = install_router(&mut app, "first");
    let (second, second_host) = install_router(&mut app, "second");
    app.update();
    navigate(&mut app, first, 1);
    let a = target(&mut app, "first", 1);
    let a_root = app.world().get::<ChildOf>(a).unwrap().parent();
    let layout = target(&mut app, "first", 0);
    assert!(work(&app, a).active && work(&app, layout).active);
    assert!(!work(&app, first_host).active);
    assert!(!app.world().resource::<RouterInputGate>().allows_pointer(a));

    navigate(&mut app, second, 7);
    let other_router = target(&mut app, "second", 7);
    navigate(&mut app, first, 2);
    let b = target(&mut app, "first", 2);
    let b_root = app.world().get::<ChildOf>(b).unwrap().parent();
    assert!(!work(&app, a).active && work(&app, b).active);
    assert!(work(&app, layout).active && work(&app, other_router).active);

    // These are real content changes after publication, with no Router marker.
    let inactive_dynamic = app
        .world_mut()
        .spawn((AppWork::new("first", 101), ChildOf(a_root)))
        .id();
    let active_dynamic = app
        .world_mut()
        .spawn((AppWork::new("first", 102), ChildOf(b_root)))
        .id();
    let inactive_ticks = work(&app, a).ticks;
    app.update();
    assert_eq!(work(&app, a).ticks, inactive_ticks);
    assert!(!work(&app, inactive_dynamic).active);
    assert!(work(&app, active_dynamic).active);
    assert_eq!(
        work(&app, active_dynamic)
            .current_params
            .as_ref()
            .unwrap()
            .get("id"),
        Some("2")
    );
    assert_eq!(
        work(&app, active_dynamic)
            .current_params
            .as_ref()
            .unwrap()
            .get("org"),
        None
    );
    assert!(work(&app, other_router).active);
    assert_eq!(
        work(&app, other_router).current_location.as_ref(),
        Some(&RouteLocation::new("/orgs/7/items/7"))
    );

    navigate(&mut app, first, 1);
    assert_eq!(target(&mut app, "first", 1), a);
    assert!(work(&app, a).active && work(&app, inactive_dynamic).active);
    assert!(!work(&app, b).active && !work(&app, active_dynamic).active);
    assert!(work(&app, other_router).active);

    // A same-Entity registration replacement is a new host incarnation, even
    // before the Router's next observation system processes it.
    app.world_mut()
        .entity_mut(first_host)
        .remove::<RouterHostRegistration>();
    app.world_mut()
        .entity_mut(first_host)
        .insert(RouterHostRegistration::new(first));
    app.world_mut()
        .run_system_once(perform_active_work)
        .unwrap();
    assert!(!work(&app, a).active && !work(&app, layout).active);
    assert!(work(&app, other_router).active);
    assert!(!work(&app, second_host).active);

    // Moving an authored parent root also moves its outlet and active child.
    // The child must still belong to the exact published host through that
    // parent; its own unchanged root/shell/slot are not sufficient.
    let other_layout = target(&mut app, "second", 0);
    let other_layout_root = app.world().get::<ChildOf>(other_layout).unwrap().parent();
    let unrelated = app.world_mut().spawn(Node::default()).id();
    app.world_mut()
        .entity_mut(other_layout_root)
        .insert(ChildOf(unrelated));
    app.world_mut()
        .run_system_once(perform_active_work)
        .unwrap();
    assert!(!work(&app, other_router).active);
    assert!(!work(&app, other_layout).active);
}

#[test]
fn a_new_reader_observes_host_replacement_within_one_system_run() {
    let mut app = App::new();
    app.add_plugins((
        TaskPoolPlugin::default(),
        AssetPlugin::default(),
        ScenePlugin,
        BevyUiRouterPlugin,
    ));
    let (router, host) = install_router(&mut app, "reader");
    app.update();
    navigate(&mut app, router, 1);
    let target = target(&mut app, "reader", 1);
    app.world_mut()
        .run_system_once(
            move |mut params: ParamSet<(RouteQueries, Query<&mut RouterHostRegistration>)>| {
                {
                    let queries = params.p0();
                    let routes = queries.read();
                    assert!(routes.is_active(target));
                    assert!(routes.current(target).is_some());
                }
                *params.p1().get_mut(host).unwrap() = RouterHostRegistration::new(router);
                let queries = params.p0();
                let routes = queries.read();
                assert!(!routes.is_active(target));
                assert!(routes.current(target).is_none());
            },
        )
        .unwrap();
}
