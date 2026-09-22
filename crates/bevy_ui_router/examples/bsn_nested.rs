//! A finite headless demonstration of real UI layout, with no GPU or OS window.
//! Sidebar and toolbar survive page navigation through an internal default outlet.
use bevy_app::App;
use bevy_asset::AssetPlugin;
use bevy_ecs::prelude::*;
use bevy_input_focus::InputFocusPlugin;
use bevy_internal::{DefaultPlugins, camera::Camera2d, math::Vec2};
use bevy_scene::{ScenePlugin, bsn};
use bevy_ui::{ComputedNode, FlexDirection, Node, Val};
use bevy_ui_router::prelude::*;

#[derive(Component, Clone, Default)]
struct Layout;
#[derive(Component, Clone, Default)]
struct Sidebar;
#[derive(Component, Clone, Default)]
struct MainColumn;
#[derive(Component, Clone, Default)]
struct ToolbarState(u32);
#[derive(Component, Clone, Default)]
struct Page(&'static str);

fn main() {
    let mut app = App::new();
    // Cargo selects window/camera/UI/picking, without Winit or a GPU renderer.
    app.add_plugins(DefaultPlugins);
    if !app.is_plugin_added::<AssetPlugin>() {
        app.add_plugins(AssetPlugin::default());
    }
    if !app.is_plugin_added::<ScenePlugin>() {
        app.add_plugins(ScenePlugin);
    }
    app.add_plugins(InputFocusPlugin);
    app.world_mut().spawn(Camera2d);

    let layout_driver = app
        .register_bsn_view(ViewDriverKey::new("layout").unwrap(), |_| {
            Ok(bsn! {
                Node { width: Val::Px(480.0), height: Val::Px(240.0) }
                Layout
                Children [
                    (Node { width: Val::Px(80.0), height: Val::Percent(100.0) } Sidebar),
                    (Node {
                        width: Val::Px(400.0), height: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                    } MainColumn Children [
                        (Node { width: Val::Percent(100.0), height: Val::Px(32.0) } ToolbarState),
                        (Node { width: Val::Percent(100.0), flex_grow: 1.0 } DefaultOutlet)
                    ])
                ]
            })
        })
        .unwrap();
    let mut views = RouteViewRegistry::new();
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
    for name in ["overview", "settings", "help"] {
        let driver = app
            .register_bsn_view(ViewDriverKey::new(name).unwrap(), move |_| {
                Ok(bsn! {
                    Node { width: Val::Px(360.0), height: Val::Px(160.0) }
                    Page(name)
                })
            })
            .unwrap();
        let view = RouteViewKey::new(name).unwrap();
        views
            .insert(view.clone(), RouteViewDescriptor::new(driver))
            .unwrap();
        routes
            .route_with_options(
                root,
                RoutePattern::path(name).unwrap(),
                RouteNodeOptions::new().with_view(view),
            )
            .unwrap();
    }
    let fallback = RouteViewKey::new("error").unwrap();
    let driver = app
        .register_bsn_view(ViewDriverKey::new("error").unwrap(), |input| {
            eprintln!("Navigation failed: {:?}", input.failure());
            Ok(bsn! { Node })
        })
        .unwrap();
    views
        .insert(fallback.clone(), RouteViewDescriptor::new(driver))
        .unwrap();
    let router = app
        .add_ui_router(RouterConfig::new(routes.build().unwrap(), views, fallback))
        .unwrap();
    app.world_mut().spawn((
        Node {
            width: Val::Px(480.0),
            height: Val::Px(240.0),
            ..Default::default()
        },
        RouterHostRegistration::new(router),
    ));
    app.update();

    let request = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, RouteLocation::new("/overview"))
        .unwrap();
    settle(&mut app, router, request, "/overview", 1, 0);
    let layout = only::<Layout>(&mut app);
    let sidebar = only::<Sidebar>(&mut app);
    let column = only::<MainColumn>(&mut app);
    let toolbar = only::<ToolbarState>(&mut app);
    let outlet = only::<DefaultOutlet>(&mut app);
    // Observe the single child slot through Bevy's public hierarchy, without
    // importing Router's private slot/identity components or modifying the slot.
    let children = app.world().get::<Children>(outlet).unwrap();
    assert_eq!(children.len(), 1);
    let slot = children[0];
    let preserved = [layout, sidebar, column, toolbar, outlet, slot];
    let parents = preserved.map(|entity| app.world().get::<ChildOf>(entity).unwrap().parent());
    app.world_mut().get_mut::<ToolbarState>(toolbar).unwrap().0 = 7;
    let first_page = show_page(&mut app, "overview", slot, &preserved);

    let request = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, RouteLocation::new("/settings"))
        .unwrap();
    settle(&mut app, router, request, "/settings", 2, 1);
    let settings_page = show_page(&mut app, "settings", slot, &preserved);
    assert!(app.world().get_entity(first_page).is_err());
    // Replace the second history entry; Back still returns to the first one.
    let request = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .replace(router, RouteLocation::new("/help"))
        .unwrap();
    settle(&mut app, router, request, "/help", 2, 1);
    let help_page = show_page(&mut app, "help", slot, &preserved);
    assert!(app.world().get_entity(settings_page).is_err());
    let request = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .back(router)
        .unwrap();
    settle(&mut app, router, request, "/overview", 2, 0);
    show_page(&mut app, "overview", slot, &preserved);
    assert!(app.world().get_entity(help_page).is_err());

    assert_eq!(only::<Layout>(&mut app), layout);
    assert_eq!(only::<DefaultOutlet>(&mut app), outlet);
    assert_eq!(only::<ToolbarState>(&mut app), toolbar);
    for (entity, parent) in preserved.into_iter().zip(parents) {
        assert_eq!(app.world().get::<ChildOf>(entity).unwrap().parent(), parent);
    }
    assert_eq!(app.world().get::<ToolbarState>(toolbar).unwrap().0, 7);
    println!("Preserved layout, sidebar, toolbar state=7, internal outlet and child slot.");
}

fn settle(
    app: &mut App,
    router: RouterHandle,
    request: NavigationRequestId,
    location: &str,
    entries: usize,
    cursor: usize,
) {
    for _ in 0..32 {
        app.update();
        let snapshots = app.world().resource::<RouterSnapshots>();
        if let RequestLookup::Found(result) = snapshots.request(router, request) {
            match result.status() {
                RequestStatus::Succeeded => {
                    let snapshot = snapshots.get(router).unwrap();
                    assert_eq!(
                        snapshot.active().location(),
                        Some(&RouteLocation::new(location))
                    );
                    assert_eq!(snapshot.history().entries().len(), entries);
                    assert_eq!(snapshot.history().cursor(), Some(cursor));
                    println!("Succeeded: {location}; history entries={entries}, cursor={cursor}");
                    return;
                }
                RequestStatus::Failed(failure) => panic!("navigation failed: {failure:?}"),
                _ => {}
            }
        }
    }
    panic!("navigation did not settle within 32 updates");
}

fn only<T: Component>(app: &mut App) -> Entity {
    app.world_mut()
        .query_filtered::<Entity, With<T>>()
        .single(app.world())
        .unwrap()
}

fn show_page(app: &mut App, name: &str, slot: Entity, layout: &[Entity]) -> Entity {
    let page = only::<Page>(app);
    let world = app.world();
    assert_eq!(world.get::<Page>(page).unwrap().0, name);
    // The native page may have Router wrappers: inspect ancestry, not a fixed depth.
    let mut ancestor = page;
    while ancestor != slot {
        ancestor = world
            .get::<ChildOf>(ancestor)
            .expect("page belongs to the preserved slot")
            .parent();
    }
    for &entity in layout.iter().chain(std::iter::once(&page)) {
        let size = world.get::<ComputedNode>(entity).unwrap().size();
        assert!(
            size.cmpgt(Vec2::ZERO).all(),
            "empty UI layout for {entity:?}"
        );
    }
    println!(
        "  {name} computed size: {:?}",
        world.get::<ComputedNode>(page).unwrap().size()
    );
    page
}
