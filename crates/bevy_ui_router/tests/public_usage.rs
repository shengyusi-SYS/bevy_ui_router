//! Recommended declarations and commands through ordinary application schedules.
use bevy_app::{App, TaskPoolPlugin, Update};
use bevy_asset::AssetPlugin;
use bevy_ecs::prelude::*;
use bevy_scene::{ScenePlugin, bsn};
use bevy_ui::Node;
use bevy_ui_router::prelude::*;

#[derive(Component, Clone, Default)]
struct LayoutState;

#[derive(Component, Clone, Default)]
struct PageState {
    creation: Option<RouteLocation>,
    current: Option<RouteLocation>,
    parameter: String,
    visits: usize,
}

#[derive(Component, Clone, Default)]
struct ErrorPage;

#[derive(Component, Clone, Default)]
struct OtherRouterPage;

#[derive(Component, Clone, Default)]
struct OtherRouterError;

#[derive(Resource)]
struct AppNavigation {
    router: RouterHandle,
    next: Option<Action>,
}

enum Action {
    Push(&'static str),
    Replace(&'static str),
    Back,
}

fn update_pages(
    inputs: RouteInputs,
    mut pages: Query<(Entity, &mut PageState)>,
    mut actions: ResMut<AppNavigation>,
    mut commands: Commands,
) {
    for (entity, mut page) in &mut pages {
        if let Some(input) = inputs.current(entity) {
            page.current = Some(input.location().clone());
            page.parameter = input.params().get("id").unwrap().to_owned();
            page.visits += 1;
        }
    }
    let mut router = commands.router(actions.router);
    match actions.next.take() {
        Some(Action::Push(address)) => {
            router.push(address);
        }
        Some(Action::Replace(address)) => {
            router.replace(address);
        }
        Some(Action::Back) => {
            router.back();
        }
        None => {}
    }
}

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((
        TaskPoolPlugin::default(),
        AssetPlugin::default(),
        ScenePlugin,
    ));
    app
}

fn declaration() -> BsnRouter {
    BsnRouter::new(
        layout(|_| Ok(bsn! { Node LayoutState DefaultOutlet }))
            .preserve_on_search()
            .children([
                index(|_| Ok(bsn! { Node })),
                page("items/:id", |input| {
                    let creation = Some(input.location().clone());
                    Ok(bsn! { Node PageState { creation: creation } })
                })
                .keep_alive()
                .preserve_on_search(),
                page("other", |_| Ok(bsn! { Node })),
            ]),
    )
    .fallback(|_| Ok(bsn! { Node ErrorPage }))
    .keep_alive_capacity(2)
}

fn settle(app: &mut App, router: RouterHandle, address: &str) {
    let expected = RouteLocation::from_address(address);
    for _ in 0..32 {
        app.update();
        let snapshot = app
            .world()
            .resource::<RouterSnapshots>()
            .get(router)
            .unwrap();
        if snapshot.active().location() == Some(&expected)
            && snapshot.running_request().is_none()
            && !snapshot.cleanup().foreground_pending()
        {
            assert_eq!(snapshot.cleanup().quarantine_count(), 0);
            return;
        }
    }
    panic!(
        "navigation did not settle at {address}: {:?}",
        app.world().resource::<RouterSnapshots>().get(router)
    );
}

fn only<T: Component>(app: &mut App) -> Entity {
    app.world_mut()
        .query_filtered::<Entity, With<T>>()
        .single(app.world())
        .unwrap()
}

#[test]
fn declarations_and_commands_preserve_current_inputs_history_and_retained_state() {
    let mut app = app();
    let router = app
        .add_bsn_router(declaration().initial("/items/7?tab=first"))
        .unwrap();
    app.world_mut()
        .spawn((Node::default(), RouterHostRegistration::new(router)));
    app.insert_resource(AppNavigation { router, next: None })
        .add_systems(
            Update,
            update_pages.after(RouterScheduleSet::PublishSnapshots),
        );
    settle(&mut app, router, "/items/7?tab=first");
    let layout = only::<LayoutState>(&mut app);
    let page = only::<PageState>(&mut app);

    app.world_mut().resource_mut::<AppNavigation>().next =
        Some(Action::Replace("/items/7?tab=next"));
    settle(&mut app, router, "/items/7?tab=next");
    assert_eq!(only::<LayoutState>(&mut app), layout);
    assert_eq!(only::<PageState>(&mut app), page);
    let state = app.world().get::<PageState>(page).unwrap();
    assert_eq!(state.creation.as_ref().unwrap().search(), Some("tab=first"));
    assert_eq!(state.current.as_ref().unwrap().search(), Some("tab=next"));
    assert_eq!(state.parameter, "7");

    app.world_mut().resource_mut::<AppNavigation>().next = Some(Action::Push("/other"));
    settle(&mut app, router, "/other");
    let visits = app.world().get::<PageState>(page).unwrap().visits;
    app.update();
    assert_eq!(app.world().get::<PageState>(page).unwrap().visits, visits);
    app.world_mut().resource_mut::<AppNavigation>().next = Some(Action::Back);
    settle(&mut app, router, "/items/7?tab=next");
    assert_eq!(only::<PageState>(&mut app), page);
    assert!(app.world().get::<PageState>(page).unwrap().visits > visits);
    let snapshot = app
        .world()
        .resource::<RouterSnapshots>()
        .get(router)
        .unwrap();
    assert_eq!(snapshot.history().len(), 2);
    assert_eq!(snapshot.history().cursor(), Some(0));
}

#[test]
fn initial_waits_for_its_host_and_does_not_replay_after_reregistration() {
    let mut app = app();
    let router = app
        .add_bsn_router(declaration().initial("/items/1"))
        .unwrap();
    for _ in 0..3 {
        app.update();
    }
    assert_eq!(
        app.world()
            .resource::<RouterSnapshots>()
            .get(router)
            .unwrap()
            .history()
            .len(),
        0
    );
    let host = app
        .world_mut()
        .spawn((Node::default(), RouterHostRegistration::new(router)))
        .id();
    settle(&mut app, router, "/items/1");
    app.world_mut()
        .entity_mut(host)
        .remove::<RouterHostRegistration>();
    for _ in 0..32 {
        app.update();
    }
    app.world_mut()
        .entity_mut(host)
        .insert(RouterHostRegistration::new(router));
    for _ in 0..32 {
        app.update();
    }
    let snapshot = app
        .world()
        .resource::<RouterSnapshots>()
        .get(router)
        .unwrap();
    assert_eq!(snapshot.availability(), &RouterAvailability::Ready);
    assert_eq!(snapshot.history().len(), 1);
    assert_eq!(snapshot.active().kind(), ActivePresentationKind::None);
    app.world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, "/other".into())
        .unwrap();
    settle(&mut app, router, "/other");
    assert_eq!(
        app.world()
            .resource::<RouterSnapshots>()
            .get(router)
            .unwrap()
            .history()
            .len(),
        2
    );
}

#[test]
fn startup_host_accepts_ui_inheritance_from_the_first_post_update() {
    use bevy_app::{HierarchyPropagatePlugin, PostUpdate, Propagate, Startup};
    use bevy_ui::{ComputedUiRenderTargetInfo, ComputedUiTargetCamera};

    let mut app = app();
    app.add_plugins((
        HierarchyPropagatePlugin::<ComputedUiTargetCamera>::new(PostUpdate),
        HierarchyPropagatePlugin::<ComputedUiRenderTargetInfo>::new(PostUpdate),
    ));
    let router = app
        .add_bsn_router(declaration().initial("/items/1"))
        .unwrap();
    app.add_systems(Startup, move |mut commands: Commands| {
        commands
            .spawn((
                Node::default(),
                Propagate(ComputedUiTargetCamera::default()),
                Propagate(ComputedUiRenderTargetInfo::default()),
            ))
            .with_children(|parent| {
                parent.spawn((Node::default(), RouterHostRegistration::new(router)));
            });
    });
    // Admission happens in Update, before Bevy first propagates the UI facts.
    app.update();
    let host = only::<RouterHostRegistration>(&mut app);
    assert!(
        app.world()
            .get::<bevy_app::Inherited<ComputedUiTargetCamera>>(host)
            .is_some()
    );
    settle(&mut app, router, "/items/1");
    app.world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, "/other".into())
        .unwrap();
    settle(&mut app, router, "/other");
    app.world_mut()
        .resource_mut::<RouterNavigation>()
        .back(router)
        .unwrap();
    settle(&mut app, router, "/items/1");
    let snapshot = app
        .world()
        .resource::<RouterSnapshots>()
        .get(router)
        .unwrap();
    assert_eq!(snapshot.availability(), &RouterAvailability::Ready);
    assert_eq!(snapshot.history().len(), 2);
    assert_eq!(snapshot.history().cursor(), Some(0));
}

#[test]
fn independent_declarations_do_not_share_factories_or_fallbacks() {
    let mut app = app();
    let first = app
        .add_bsn_router(declaration().initial("/items/1"))
        .unwrap();
    let second = app
        .add_bsn_router(
            BsnRouter::new(page("/second", |_| Ok(bsn! { Node OtherRouterPage })))
                .fallback(|_| Ok(bsn! { Node OtherRouterError }))
                .initial("/second"),
        )
        .unwrap();
    app.world_mut()
        .spawn((Node::default(), RouterHostRegistration::new(first)));
    app.world_mut()
        .spawn((Node::default(), RouterHostRegistration::new(second)));
    settle(&mut app, first, "/items/1");
    settle(&mut app, second, "/second");
    let other_page = only::<OtherRouterPage>(&mut app);
    assert!(app.world().get::<OtherRouterPage>(other_page).is_some());
    let first_page = only::<PageState>(&mut app);
    assert_eq!(
        app.world()
            .get::<PageState>(first_page)
            .unwrap()
            .creation
            .as_ref()
            .unwrap()
            .path(),
        "/items/1"
    );
    app.world_mut()
        .resource_mut::<RouterNavigation>()
        .push(second, "/missing".into())
        .unwrap();
    for _ in 0..32 {
        app.update();
    }
    let snapshots = app.world().resource::<RouterSnapshots>();
    assert_eq!(
        snapshots.get(first).unwrap().active().kind(),
        ActivePresentationKind::Route
    );
    assert_eq!(
        snapshots.get(second).unwrap().active().kind(),
        ActivePresentationKind::Error
    );
    assert_eq!(snapshots.get(first).unwrap().history().len(), 1);
    assert_eq!(snapshots.get(second).unwrap().history().len(), 1);
    let error_page = only::<OtherRouterError>(&mut app);
    assert!(app.world().get::<OtherRouterError>(error_page).is_some());
    assert!(
        app.world_mut()
            .query_filtered::<Entity, With<ErrorPage>>()
            .iter(app.world())
            .next()
            .is_none()
    );
}
