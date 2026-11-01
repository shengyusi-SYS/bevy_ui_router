//! A finite headless demonstration of real UI layout, with no GPU or OS window.
//! Sidebar and toolbar survive child navigation and a visit outside the layout.
//! Query reuse is explicit. Application tasks populate content after publication,
//! then allow a fresh child's history-entry scroll to restore through normal UI layout.
use bevy_app::{App, Update};
use bevy_asset::AssetPlugin;
use bevy_ecs::prelude::*;
use bevy_ecs::system::RunSystemOnce;
use bevy_input_focus::InputFocusPlugin;
use bevy_internal::{
    DefaultPlugins,
    camera::{Camera, Camera2d, ComputedCameraValues, RenderTargetInfo},
    math::{UVec2, Vec2},
    tasks::{AsyncComputeTaskPool, Task, block_on, futures_lite::future, poll_once},
};
use bevy_scene::{PatchFromTemplate, ScenePlugin, bsn};
use bevy_ui::{ComputedNode, FlexDirection, Node, Overflow, ScrollPosition, Val};
use bevy_ui_router::prelude::*;

#[derive(Component, Clone, Default)]
struct Layout;
#[derive(Component, Clone, Default)]
struct Sidebar;
#[derive(Component, Clone, Default)]
struct MainColumn;
#[derive(Component, Clone, Default)]
struct WorkspaceOutlet;
#[derive(Component, Clone, Default)]
struct ToolbarState(u32);
#[derive(Component, Clone, Default)]
struct Page(&'static str);
#[derive(Component, Clone, Default)]
struct PageBody;
#[derive(Component)]
struct ContentTask(Task<LoadedRows>);
#[derive(Component)]
struct LoadedContent(RouteLocation);

struct LoadedRows {
    source: RouteLocation,
    rows: Vec<Vec2>,
}

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
    app.add_systems(
        Update,
        install_loaded_content.after(RouterScheduleSet::PublishSnapshots),
    );
    // The headless app supplies the target information normally provided by the
    // renderer; propagation and layout still run through Bevy's normal systems.
    app.world_mut().spawn((
        Camera2d,
        Camera {
            computed: ComputedCameraValues {
                target_info: Some(RenderTargetInfo {
                    physical_size: UVec2::new(480, 240),
                    scale_factor: 1.0,
                }),
                ..Default::default()
            },
            ..Default::default()
        },
    ));

    let workspace = page("workspace", |_| {
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
                    (Node { width: Val::Percent(100.0), flex_grow: 1.0 } DefaultOutlet WorkspaceOutlet)
                ])
            ]
        })
    })
    .keep_alive()
    .preserve_on_search()
    .children([content_page("overview"), content_page("settings"), content_page("help")]);
    let root = layout(|_| {
        Ok(bsn! {
            Node { width: Val::Percent(100.0), height: Val::Percent(100.0) }
            DefaultOutlet
        })
    })
    .preserve_on_search()
    .children([workspace, content_page("outside")]);
    let router = app
        .add_bsn_router(
            BsnRouter::new(root)
                .fallback(|input| {
                    eprintln!("Navigation failed: {:?}", input.failure());
                    Ok(bsn! { Node })
                })
                .keep_alive_capacity(1),
        )
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

    let initial_location = RouteLocation::new("/workspace/overview").with_search("sort=name");
    let request = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, initial_location.clone())
        .unwrap();
    settle(&mut app, router, request, &initial_location, 1, 0);
    let layout = only::<Layout>(&mut app);
    let sidebar = only::<Sidebar>(&mut app);
    let column = only::<MainColumn>(&mut app);
    let toolbar = only::<ToolbarState>(&mut app);
    let outlet = only::<WorkspaceOutlet>(&mut app);
    // Observe the single child slot through Bevy's public hierarchy, without
    // importing Router's private slot/identity components or modifying the slot.
    let children = app.world().get::<Children>(outlet).unwrap();
    assert_eq!(children.len(), 1);
    let slot = children[0];
    let preserved = [layout, sidebar, column, toolbar, outlet, slot];
    let parents = preserved.map(|entity| app.world().get::<ChildOf>(entity).unwrap().parent());
    app.world_mut().get_mut::<ToolbarState>(toolbar).unwrap().0 = 7;
    assert_preserved(&mut app, &preserved, &parents);
    let first_page = show_page(&mut app, "overview", slot, &preserved);
    load_content(&mut app, first_page);
    let saved_scroll = Vec2::new(24.0, 96.0);
    app.world_mut()
        .get_mut::<ScrollPosition>(first_page)
        .unwrap()
        .0 = saved_scroll;
    app.update();
    assert_scroll(&app, first_page, saved_scroll);

    // Only the stable root/layout opt in. The default leaf is rebuilt on query
    // changes, while the existing toolbar reads the new published input.
    let changed_location = RouteLocation::new("/workspace/overview").with_search("sort=date");
    let request = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, changed_location.clone())
        .unwrap();
    settle(&mut app, router, request, &changed_location, 2, 1);
    assert_preserved(&mut app, &preserved, &parents);
    let changed_overview = show_page(&mut app, "overview", slot, &preserved);
    assert_ne!(changed_overview, first_page);
    assert!(app.world().get_entity(first_page).is_err());
    assert_current_input(&mut app, toolbar, Some(&changed_location));
    // Replace the second history entry; Back still returns to the first one.
    let help_location = RouteLocation::new("/workspace/help").with_search("sort=date");
    let request = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .replace(router, help_location.clone())
        .unwrap();
    settle(&mut app, router, request, &help_location, 2, 1);
    assert_preserved(&mut app, &preserved, &parents);
    let help_page = show_page(&mut app, "help", slot, &preserved);
    assert!(app.world().get_entity(changed_overview).is_err());
    let request = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .back(router)
        .unwrap();
    settle(&mut app, router, request, &initial_location, 2, 0);
    assert_preserved(&mut app, &preserved, &parents);
    let returned_overview = show_page(&mut app, "overview", slot, &preserved);
    assert_ne!(returned_overview, first_page);
    assert!(app.world().get_entity(help_page).is_err());
    let restoration = app
        .world_mut()
        .run_system_once(move |restorations: RouteScrollRestorations| {
            restorations.pending(returned_overview, "body")
        })
        .unwrap()
        .expect("Back freezes the original entry's scroll for the fresh child");
    assert_eq!(restoration.position().0, saved_scroll);
    // Navigation is terminal even though application content has not arrived.
    // The result installation below readies this exact publication's intent.
    assert_scroll(&app, returned_overview, Vec2::ZERO);
    assert!(
        app.world()
            .get::<LoadedContent>(returned_overview)
            .is_none()
    );
    load_content(&mut app, returned_overview);
    assert_scroll(&app, returned_overview, saved_scroll);
    assert!(
        app.world_mut()
            .run_system_once(move |restorations: RouteScrollRestorations| {
                restorations.pending(returned_overview, "body")
            })
            .unwrap()
            .is_none()
    );
    println!("Restored the fresh overview's body to {saved_scroll:?} after content readiness.");

    // Leave the whole layout. Only it opts into retention; its child is disposed.
    let overview = only::<Page>(&mut app);
    let outside_location = RouteLocation::new("/outside").with_search("filter=other");
    let request = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, outside_location.clone())
        .unwrap();
    settle(&mut app, router, request, &outside_location, 2, 1);
    let outside = only::<Page>(&mut app);
    assert_eq!(app.world().get::<Page>(outside).unwrap().0, "outside");
    assert!(app.world().get_entity(overview).is_err());
    assert_preserved(&mut app, &preserved, &parents);
    assert_current_input(&mut app, toolbar, None);
    assert!(
        !app.world()
            .resource::<RouterInputGate>()
            .allows_pointer(layout)
    );

    // Reuse the original layout, but let the new route select a fresh child.
    let returned_location = RouteLocation::new("/workspace/settings").with_search("sort=duration");
    let request = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, returned_location.clone())
        .unwrap();
    settle(&mut app, router, request, &returned_location, 3, 2);
    assert_preserved(&mut app, &preserved, &parents);
    let returned_settings = show_page(&mut app, "settings", slot, &preserved);
    assert_ne!(returned_settings, changed_overview);
    assert!(app.world().get_entity(outside).is_err());
    assert!(
        app.world()
            .resource::<RouterInputGate>()
            .allows_pointer(layout)
    );
    assert_current_input(&mut app, toolbar, Some(&returned_location));
    load_content(&mut app, returned_settings);

    println!("Reused the original layout and toolbar state=7 with the newly selected child.");
}

/// The factory creates the page once. Its current published location is read
/// through RouteInputs when application work starts or installs its result.
fn content_page(name: &'static str) -> BsnRoute {
    page(name, move |_| {
        Ok(bsn! {
            Node {
                width: Val::Px(360.0), height: Val::Px(160.0),
                flex_shrink: 0.0, overflow: Overflow::scroll(),
            }
            { RouteScroll::patch(|marker, _| {
                *marker = RouteScroll::new("body")
                    .with_restore_policy(RouteScrollRestorePolicy::Explicit);
            }) }
            Page(name)
            Children [(Node {
                width: Val::Px(720.0), flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
            } PageBody)]
        })
    })
}

/// Application work owns only ordinary input/data, never a World borrow or a
/// Router lifecycle capability. A real application can replace this computation
/// with its own data source without changing the route or scroll contracts.
fn load_content(app: &mut App, page: Entity) {
    let mut inputs = bevy_ecs::system::SystemState::<RouteInputs>::new(app.world_mut());
    let source = inputs
        .get(app.world())
        .unwrap()
        .current(page)
        .expect("the application starts work for its active page")
        .location()
        .clone();
    let task_source = source.clone();
    let task = AsyncComputeTaskPool::get().spawn(async move {
        future::yield_now().await;
        LoadedRows {
            source: task_source,
            rows: vec![Vec2::new(720.0, 20.0); 30],
        }
    });
    app.world_mut().entity_mut(page).insert(ContentTask(task));
    for _ in 0..32 {
        app.update();
        if app
            .world()
            .get::<LoadedContent>(page)
            .is_some_and(|loaded| loaded.0 == source)
        {
            return;
        }
    }
    panic!("application content did not arrive within 32 updates");
}

fn install_loaded_content(
    mut commands: Commands,
    mut tasks: Query<(Entity, &mut ContentTask)>,
    bodies: Query<(Entity, &ChildOf), With<PageBody>>,
    inputs: RouteInputs,
    restorations: RouteScrollRestorations,
) {
    for (page, mut task) in &mut tasks {
        // This application chooses to install only on active pages. Keeping a
        // task while hidden is a policy choice; retention does not cancel it.
        let Some(current) = inputs.current(page) else {
            continue;
        };
        let Some(result) = block_on(poll_once(&mut task.0)) else {
            continue;
        };
        commands.entity(page).remove::<ContentTask>();
        if &result.source != current.location() {
            continue;
        }
        let (body, _) = bodies
            .iter()
            .find(|(_, parent)| parent.parent() == page)
            .expect("the application owns a stable content region below the page");
        for size in result.rows {
            commands.spawn((
                Node {
                    width: Val::Px(size.x),
                    height: Val::Px(size.y),
                    flex_shrink: 0.0,
                    ..Default::default()
                },
                ChildOf(body),
            ));
        }
        commands.entity(page).insert(LoadedContent(result.source));
        // These Commands install content before the normal layout/scroll pass.
        // Look up the current intent now rather than carrying an old restore ID
        // through the task or guessing readiness from a number of frames.
        if let Some(intent) = restorations.pending(page, "body") {
            commands.ready_route_scroll(intent.id());
        }
    }
}

fn assert_current_input(app: &mut App, target: Entity, expected: Option<&RouteLocation>) {
    let mut inputs = bevy_ecs::system::SystemState::<RouteInputs>::new(app.world_mut());
    let input = inputs.get(app.world()).unwrap();
    assert_eq!(
        input.current(target).map(|input| input.location()),
        expected
    );
}

fn assert_preserved(app: &mut App, entities: &[Entity; 6], parents: &[Entity; 6]) {
    let [layout, sidebar, column, toolbar, outlet, _slot] = *entities;
    assert_eq!(only::<Layout>(app), layout);
    assert_eq!(only::<Sidebar>(app), sidebar);
    assert_eq!(only::<MainColumn>(app), column);
    assert_eq!(only::<ToolbarState>(app), toolbar);
    assert_eq!(only::<WorkspaceOutlet>(app), outlet);
    for (&entity, &parent) in entities.iter().zip(parents) {
        assert_eq!(app.world().get::<ChildOf>(entity).unwrap().parent(), parent);
    }
    assert_eq!(app.world().get::<ToolbarState>(toolbar).unwrap().0, 7);
}

fn settle(
    app: &mut App,
    router: RouterHandle,
    request: NavigationRequestId,
    location: &RouteLocation,
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
                    assert_eq!(snapshot.active().location(), Some(location));
                    assert_eq!(snapshot.history().entries().len(), entries);
                    assert_eq!(snapshot.history().cursor(), Some(cursor));
                    println!("Succeeded: {location:?}; history entries={entries}, cursor={cursor}");
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

fn assert_scroll(app: &App, entity: Entity, expected: Vec2) {
    let world = app.world();
    let requested = world.get::<ScrollPosition>(entity).unwrap().0;
    let computed = world.get::<ComputedNode>(entity).unwrap();
    let displayed = computed.scroll_position * computed.inverse_scale_factor;
    assert!(
        requested.abs_diff_eq(expected, 0.001),
        "requested: {requested:?}"
    );
    assert!(
        displayed.abs_diff_eq(expected, 0.001),
        "displayed: {displayed:?}"
    );
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
