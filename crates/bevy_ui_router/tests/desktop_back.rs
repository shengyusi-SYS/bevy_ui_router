//! Desktop input through the facade, an application return policy and normal navigation.
use bevy_app::{App, PreUpdate, TaskPoolPlugin};
use bevy_asset::AssetPlugin;
use bevy_ecs::{prelude::*, system::RunSystemOnce, template::FromTemplate};
use bevy_input::{
    ButtonState, InputPlugin,
    mouse::{MouseButton, MouseButtonInput},
};
use bevy_scene::{ResolvedScene, SceneFunction, ScenePlugin};
use bevy_ui::Node;
use bevy_ui_router::prelude::*;
use bevy_window::Window;

#[derive(Component, Clone, Default)]
struct Page(&'static str, &'static str);

#[derive(Component, Clone, Default)]
struct LocalValue(u32);

#[derive(Component)]
struct WindowNavigation {
    router: RouterHandle,
    modal_open: bool,
    closed_modals: usize,
}

#[derive(Resource, Default)]
struct DispatchLog(
    Vec<(
        Entity,
        Result<NavigationRequestId, NavigationEnqueueErrorKind>,
    )>,
);

fn return_policy(
    mut requests: MessageReader<DesktopBackRequested>,
    mut windows: Query<&mut WindowNavigation>,
    mut navigation: ResMut<RouterNavigation>,
    mut log: ResMut<DispatchLog>,
) {
    for request in requests.read() {
        let Ok(mut target) = windows.get_mut(request.window) else {
            continue;
        };
        if target.modal_open {
            target.modal_open = false;
            target.closed_modals += 1;
            continue;
        }
        let result = navigation.back(target.router).map_err(|error| error.kind());
        log.0.push((request.window, result));
    }
}

fn install(app: &mut App, group: &'static str) -> (RouterHandle, Entity) {
    let mut views = RouteViewRegistry::new();
    for name in ["layout", "a", "b"] {
        let driver = app
            .register_bsn_view(ViewDriverKey::new(format!("{group}-{name}")).unwrap(), move |_| {
                Ok(SceneFunction(move |_, scene: &mut ResolvedScene| {
                    scene.push_template(Node::default());
                    scene.push_template(Page(group, name));
                    scene.push_template(LocalValue::default());
                    if name == "layout" {
                        scene.push_template(<DefaultOutlet as FromTemplate>::Template::default());
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
    let mut routes = RouteTree::builder()
        .root_with_options(
            RoutePattern::pathless(),
            RouteNodeOptions::new().with_view(RouteViewKey::new("layout").unwrap()),
        )
        .unwrap();
    let root = routes.root_id().unwrap();
    for name in ["a", "b"] {
        routes
            .route_with_options(
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
                routes.build().unwrap(),
                views,
                RouteViewKey::new("b").unwrap(),
            )
            .with_keep_alive_capacity(2),
        )
        .unwrap();
    app.world_mut()
        .spawn((Node::default(), RouterHostRegistration::new(router)));
    let window = app
        .world_mut()
        .spawn((
            Window::default(),
            WindowNavigation {
                router,
                modal_open: false,
                closed_modals: 0,
            },
        ))
        .id();
    (router, window)
}

fn settle(app: &mut App, router: RouterHandle, request: NavigationRequestId) {
    for _ in 0..32 {
        app.update();
        if let RequestLookup::Found(record) = app
            .world()
            .resource::<RouterSnapshots>()
            .request(router, request)
        {
            match record.status() {
                RequestStatus::Succeeded => return,
                RequestStatus::Failed(failure) => panic!("navigation failed: {failure:?}"),
                _ => (),
            }
        }
    }
    panic!("navigation did not settle");
}

fn push(app: &mut App, router: RouterHandle, path: &str) {
    let request = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, RouteLocation::new(path))
        .unwrap();
    settle(app, router, request);
}

fn press(app: &mut App, window: Entity, button: MouseButton) {
    for state in [ButtonState::Pressed, ButtonState::Released] {
        app.world_mut().write_message(MouseButtonInput {
            window,
            button,
            state,
        });
    }
    app.update();
}

fn snapshot(app: &App, router: RouterHandle) -> RouterSnapshot {
    app.world()
        .resource::<RouterSnapshots>()
        .get(router)
        .unwrap()
        .clone()
}

#[test]
fn application_policy_targets_one_window_and_back_reuses_the_original_page() {
    let mut app = App::new();
    app.add_plugins((
        TaskPoolPlugin::default(),
        AssetPlugin::default(),
        ScenePlugin,
        InputPlugin,
        BevyUiRouterPlugin::default(),
        DesktopBackInputPlugin,
    ))
    .init_resource::<DispatchLog>()
    .add_systems(PreUpdate, return_policy.after(DesktopBackInputSet));
    let (first, first_window) = install(&mut app, "first");
    let (second, second_window) = install(&mut app, "second");
    let unbound_window = app.world_mut().spawn(Window::default()).id();
    app.update();
    push(&mut app, first, "/a");
    let original = app
        .world_mut()
        .query::<(Entity, &Page)>()
        .iter(app.world())
        .find(|(_, page)| page.0 == "first" && page.1 == "a")
        .unwrap()
        .0;
    app.world_mut().get_mut::<LocalValue>(original).unwrap().0 = 41;
    push(&mut app, first, "/b");
    push(&mut app, second, "/a");
    push(&mut app, second, "/b");
    let first_before = snapshot(&app, first);
    let second_before = snapshot(&app, second);

    press(&mut app, first_window, MouseButton::Forward);
    app.world_mut()
        .get_mut::<WindowNavigation>(first_window)
        .unwrap()
        .modal_open = true;
    press(&mut app, first_window, MouseButton::Back);
    assert_eq!(
        app.world()
            .get::<WindowNavigation>(first_window)
            .unwrap()
            .closed_modals,
        1
    );
    press(&mut app, unbound_window, MouseButton::Back);
    assert!(app.world().resource::<DispatchLog>().0.is_empty());
    assert_eq!(snapshot(&app, first), first_before);
    assert_eq!(snapshot(&app, second), second_before);

    press(&mut app, first_window, MouseButton::Back);
    let log = &app.world().resource::<DispatchLog>().0;
    assert_eq!(log.len(), 1, "release must not enqueue a second Back");
    assert_eq!(log[0].0, first_window);
    let request = log[0].1.unwrap();
    settle(&mut app, first, request);
    assert_eq!(
        snapshot(&app, first).active().location(),
        Some(&RouteLocation::new("/a"))
    );
    assert!(!snapshot(&app, first).history().can_back());
    assert_eq!(app.world().get::<LocalValue>(original).unwrap().0, 41);
    assert!(
        app.world_mut()
            .run_system_once(move |activity: RouteActivity| activity.is_active(original))
            .unwrap()
    );
    assert_eq!(snapshot(&app, second), second_before);

    let first_after = snapshot(&app, first);
    press(&mut app, first_window, MouseButton::Back);
    assert_eq!(
        app.world().resource::<DispatchLog>().0[1],
        (first_window, Err(NavigationEnqueueErrorKind::NoBackEntry))
    );
    assert_eq!(snapshot(&app, first), first_after);
    assert!(app.world().get::<Window>(first_window).is_some());
    press(&mut app, second_window, MouseButton::Back);
    let log = &app.world().resource::<DispatchLog>().0;
    assert_eq!(log.len(), 3);
    assert_eq!(log[2].0, second_window);
    let request = log[2].1.unwrap();
    settle(&mut app, second, request);
    assert_eq!(
        snapshot(&app, second).active().location(),
        Some(&RouteLocation::new("/a"))
    );
    assert_eq!(snapshot(&app, first), first_after);
}
