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
}

impl AppWork {
    fn new(router: &'static str, page: u32) -> Self {
        Self {
            router,
            page,
            ticks: 0,
            active: false,
        }
    }
}

// This is an ordinary application system with mutable application state. The
// Router query must not borrow all components or depend on input eligibility.
fn perform_active_work(activity: RouteActivity, mut work: Query<(Entity, &mut AppWork)>) {
    for (entity, mut work) in &mut work {
        work.active = activity.is_active(entity);
        if work.active {
            work.ticks += 1;
        }
    }
}

struct PageScene {
    router: &'static str,
    page: u32,
    outlet: bool,
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
        background.push_template(AppWork::new(self.router, self.page));
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
            RoutePattern::path("/").unwrap(),
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
        .push(router, RouteLocation::new(format!("/items/{id}")))
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
    assert!(work(&app, other_router).active);

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
