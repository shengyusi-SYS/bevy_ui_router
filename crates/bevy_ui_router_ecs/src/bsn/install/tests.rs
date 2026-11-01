use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use bevy_app::TaskPoolPlugin;
use bevy_asset::AssetPlugin;
use bevy_scene::{ScenePlugin, bsn};
use bevy_ui::Node;

use super::*;
use crate::{
    RouterInstallErrorKind,
    bsn::{AppBsnExt, layout, page},
    navigation::RouterNavigation,
};

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((
        TaskPoolPlugin::default(),
        AssetPlugin::default(),
        ScenePlugin,
    ));
    app
}

#[test]
fn invalid_nested_declaration_reports_its_location_without_installing_factories() {
    let mut app = app();
    let calls = Arc::new(AtomicUsize::new(0));
    let factory_calls = calls.clone();
    let layout_calls = calls.clone();
    let declaration = BsnRouter::new(
        layout(move |_| {
            layout_calls.fetch_add(1, Ordering::Relaxed);
            Ok(bsn! { Node })
        })
        .children([
            page("valid", |_| Ok(bsn! { Node })),
            page("broken//path", move |_| {
                factory_calls.fetch_add(1, Ordering::Relaxed);
                Ok(bsn! { Node })
            }),
        ]),
    )
    .fallback(|_| Ok(bsn! { Node }));

    let error = app.add_bsn_router(declaration).unwrap_err();
    let BsnRouterError::Route { declaration, .. } = error else {
        panic!("invalid path must produce a declaration error");
    };
    assert!(declaration.contains("children[1] (broken//path)"));
    assert_eq!(calls.load(Ordering::Relaxed), 0);
    assert!(!app.world().contains_resource::<RouterNavigation>());
    assert!(!app.world().contains_resource::<ViewDriverStore>());
}

#[test]
fn generated_keys_coexist_with_manual_factories_and_failed_install_leaves_no_batch() {
    let mut missing_runtime = App::new();
    let manual = driver_key(0, 0);
    missing_runtime
        .register_bsn_view(manual.clone(), |_| Ok(bsn! { Node }))
        .unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let definition = || {
        let calls = calls.clone();
        BsnRouter::new(page("/", move |_| {
            calls.fetch_add(1, Ordering::Relaxed);
            Ok(bsn! { Node })
        }))
        .fallback(|_| Ok(bsn! { Node }))
    };
    let error = missing_runtime.add_bsn_router(definition()).unwrap_err();
    assert!(matches!(
        error,
        BsnRouterError::Install(error)
            if error.kind() == RouterInstallErrorKind::MissingSceneRuntime
    ));
    assert!(
        missing_runtime
            .world()
            .resource::<RouterNavigation>()
            .entries
            .is_empty()
    );
    let drivers = missing_runtime.world().resource::<ViewDriverStore>();
    assert!(drivers.contains(&manual));
    assert!(!drivers.contains(&driver_key(1, 0)));

    let mut app = app();
    let manual_calls = calls.clone();
    app.register_bsn_view(manual.clone(), move |_| {
        manual_calls.fetch_add(1, Ordering::Relaxed);
        Ok(bsn! { Node })
    })
    .unwrap();
    let first = app.add_bsn_router(definition()).unwrap();
    let second = app.add_bsn_router(definition()).unwrap();
    assert_ne!(first, second);
    let drivers = app.world().resource::<ViewDriverStore>();
    assert!(drivers.contains(&manual));
    assert!(drivers.contains(&driver_key(1, 0)));
    assert!(drivers.contains(&driver_key(2, 0)));
    assert_eq!(calls.load(Ordering::Relaxed), 0);
}

#[test]
fn first_ready_uses_initial_unless_an_explicit_request_was_successfully_enqueued() {
    use crate::{RouterHostRegistration, RouterSnapshots};
    use bevy_ui_router_core::prelude::{NavigationKind, RouteLocation, RouterAvailability};
    use std::sync::Mutex;

    for explicit in [
        None,
        Some(NavigationKind::Push),
        Some(NavigationKind::Replace),
    ] {
        let mut app = app();
        let built = Arc::new(Mutex::new(Vec::<String>::new()));
        let factory_built = built.clone();
        let router = app
            .add_bsn_router(
                BsnRouter::new(page(":page", move |input| {
                    factory_built
                        .lock()
                        .unwrap()
                        .push(input.location().path().to_owned());
                    Ok(bsn! { Node })
                }))
                .fallback(|_| Ok(bsn! { Node }))
                .initial("/default"),
            )
            .unwrap();
        // Rejected application work before host admission does not consume the
        // configured default; only a successful explicit enqueue supersedes it.
        assert!(
            app.world_mut()
                .resource_mut::<RouterNavigation>()
                .push(router, RouteLocation::new("/too-early"))
                .is_err()
        );
        app.world_mut()
            .spawn((Node::default(), RouterHostRegistration::new(router)));

        // A real Update system submits between the existing host-admission and
        // collection sets. Every stage advances through ordinary App::update.
        use crate::plugin::RouterLifecycleSet;
        use bevy_app::Update;
        use bevy_ecs::{
            prelude::{Local, ResMut},
            schedule::IntoScheduleConfigs,
        };
        app.add_systems(
            Update,
            (move |mut navigation: ResMut<RouterNavigation>, mut submitted: Local<bool>| {
                if *submitted
                    || navigation.entries[0].engine.availability() != &RouterAvailability::Ready
                {
                    return;
                }
                if let Some(operation) = explicit {
                    match operation {
                        NavigationKind::Push => {
                            navigation.push(router, RouteLocation::new("/explicit"))
                        }
                        NavigationKind::Replace => {
                            navigation.replace(router, RouteLocation::new("/explicit"))
                        }
                        NavigationKind::Back => unreachable!(),
                    }
                    .unwrap();
                }
                *submitted = true;
            })
            .after(RouterLifecycleSet::ObserveAuthority)
            .before(RouterLifecycleSet::CollectNavigation),
        );
        for _ in 0..24 {
            app.update();
        }
        let expected = if explicit.is_some() {
            "/explicit"
        } else {
            "/default"
        };
        let snapshot = app
            .world()
            .resource::<RouterSnapshots>()
            .get(router)
            .unwrap();
        assert_eq!(
            snapshot.active().location(),
            Some(&RouteLocation::new(expected))
        );
        assert_eq!(snapshot.history().len(), 1);
        assert!(snapshot.running_request().is_none());
        assert_eq!(*built.lock().unwrap(), vec![expected.to_owned()]);
    }
}
