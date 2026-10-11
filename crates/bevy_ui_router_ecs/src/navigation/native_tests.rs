use super::*;
use crate::prelude::*;
use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::AssetPlugin;
use bevy_ecs::prelude::{Component, Mut};
use bevy_ecs::template::{Template, TemplateContext};
use bevy_internal::reflect as bevy_reflect;
use bevy_scene::{SceneFunction, ScenePlugin};
use bevy_ui::{Node, Val};
use bevy_ui_router_core::prelude::*;
use bevy_ui_router_view::prelude::*;
use std::sync::atomic::AtomicBool;

#[derive(Resource, Default)]
struct NativeFallbackCalls(Arc<std::sync::atomic::AtomicUsize>);

#[derive(Default, bevy_internal::reflect::TypePath)]
struct NativeDependencyLoader;

impl bevy_asset::AssetLoader for NativeDependencyLoader {
    type Asset = bevy_scene::ScenePatch;
    type Settings = ();
    type Error = std::io::Error;
    async fn load(
        &self,
        _: &mut dyn bevy_asset::io::Reader,
        _: &(),
        _: &mut bevy_asset::LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        Ok(bevy_scene::ScenePatch {
            scene: None,
            dependencies: Vec::new(),
            resolved: None,
        })
    }
    fn extensions(&self) -> &[&str] {
        &["native_dep"]
    }
}

struct DependencyScene {
    inner: Box<dyn bevy_scene::Scene>,
    dependency: bool,
}

impl bevy_scene::Scene for DependencyScene {
    fn resolve(
        self,
        context: &mut bevy_scene::ResolveContext,
        scene: &mut bevy_scene::ResolvedScene,
    ) -> Result<(), bevy_scene::ResolveSceneError> {
        self.inner.resolve(context, scene)
    }
    fn register_dependencies(&self, dependencies: &mut bevy_scene::SceneDependencies) {
        if self.dependency {
            dependencies.register::<bevy_scene::ScenePatch>("native-test://held.native_dep".into());
        }
    }
}

#[derive(Clone)]
struct KeepProtectedThenFail(Entity);

#[derive(Resource)]
struct MakeFailureUnisolated;

#[derive(Resource)]
struct CleanFailure;

impl Template for KeepProtectedThenFail {
    type Output = ();
    fn build_template(&self, context: &mut TemplateContext<'_, '_>) -> bevy_ecs::error::Result<()> {
        let root = context.entity.id();
        context.entity.insert(Node {
            width: Val::Px(71.0),
            ..Node::default()
        });
        context.entity.world_scope(|world| {
            if !world.contains_resource::<CleanFailure>() {
                world.entity_mut(self.0).insert(ChildOf(root));
            }
            if world.contains_resource::<MakeFailureUnisolated>() {
                let shell = world
                    .resource::<RouteEntityRegistry>()
                    .records()
                    .iter()
                    .find(|record| {
                        matches!(
                            record.owner(),
                            RegistryOwner::Committed(EntityOwner::FrameShell(_))
                        ) && record.lifecycle() == EntityLifecycle::PreparedInert
                    })
                    .unwrap()
                    .entity();
                world.get_mut::<Node>(shell).unwrap().display = bevy_ui::Display::Flex;
            }
        });
        Err("native retained apply source".into())
    }
    fn clone_template(&self) -> Self {
        self.clone()
    }
}

fn fixture(failing: bool) -> (App, RouterHandle, Entity, Entity, Arc<AtomicBool>) {
    fixture_variant(failing, false)
}

fn fixture_variant(
    failing: bool,
    nested: bool,
) -> (App, RouterHandle, Entity, Entity, Arc<AtomicBool>) {
    fixture_with_dependency(failing, nested, false)
}

fn fixture_with_dependency(
    failing: bool,
    nested: bool,
    dependency: bool,
) -> (App, RouterHandle, Entity, Entity, Arc<AtomicBool>) {
    fixture_with_retention(failing, nested, dependency, 0, false)
}

fn fixture_with_retention(
    failing: bool,
    nested: bool,
    dependency: bool,
    capacity: usize,
    leaf_error: bool,
) -> (App, RouterHandle, Entity, Entity, Arc<AtomicBool>) {
    let mut app = App::new();
    if dependency {
        use bevy_asset::{
            AssetApp,
            io::{
                AssetSourceBuilder,
                memory::{Dir, MemoryAssetReader},
            },
        };
        let dir = Dir::default();
        dir.insert_asset(std::path::Path::new("held.native_dep"), Vec::new());
        app.register_asset_source(
            "native-test",
            AssetSourceBuilder::new(move || Box::new(MemoryAssetReader { root: dir.clone() })),
        );
    }
    app.add_plugins((
        TaskPoolPlugin::default(),
        AssetPlugin::default(),
        ScenePlugin,
    ));
    if dependency {
        use bevy_asset::AssetApp;
        app.register_asset_loader(NativeDependencyLoader);
    }
    let protected = app
        .world_mut()
        .spawn(Node {
            width: Val::Px(91.0),
            ..Node::default()
        })
        .id();
    let failure = Arc::new(AtomicBool::new(failing));
    let control = failure.clone();
    let driver = app
        .register_bsn_view(ViewDriverKey::new("retained").unwrap(), move |_| {
            let failing = control.load(Ordering::Relaxed);
            Ok(DependencyScene {
                dependency,
                inner: Box::new(SceneFunction(
                    move |_, root: &mut bevy_scene::ResolvedScene| {
                        root.push_template(Node {
                            width: Val::Px(71.0),
                            ..Node::default()
                        });
                        if failing {
                            root.push_bundle_template(KeepProtectedThenFail(protected));
                        }
                    },
                )),
            })
        })
        .unwrap();
    app.init_resource::<NativeFallbackCalls>();
    let fallback_calls = app.world().resource::<NativeFallbackCalls>().0.clone();
    let fallback_driver = app
        .register_bsn_view(ViewDriverKey::new("fallback").unwrap(), move |_| {
            fallback_calls.fetch_add(1, Ordering::Relaxed);
            Ok(bevy_scene::bsn! { Node })
        })
        .unwrap();
    let view = RouteViewKey::new("page").unwrap();
    let fallback = RouteViewKey::new("fallback").unwrap();
    let layout = RouteViewKey::new("layout").unwrap();
    let layout_driver = app
        .register_bsn_view(ViewDriverKey::new("layout").unwrap(), |_| {
            Ok(bevy_scene::bsn! { Node bevy_ui_router_bsn::DefaultOutlet })
        })
        .unwrap();
    let mut routes = RouteTree::builder()
        .root_with_options(
            RoutePattern::path("/").unwrap(),
            RouteNodeOptions::new().with_view(if nested { layout.clone() } else { view.clone() }),
        )
        .unwrap();
    if nested {
        let mut child_options = RouteNodeOptions::new()
            .with_view(view.clone())
            .with_keep_alive(capacity > 0);
        if leaf_error {
            child_options = child_options.with_error_view(
                ErrorBoundaryKey::named("cached-leaf-error").unwrap(),
                fallback.clone(),
            );
        }
        routes
            .route_with_options(
                routes.root_id().unwrap(),
                RoutePattern::path(if capacity == 0 { "child" } else { "child/:id" }).unwrap(),
                child_options,
            )
            .unwrap();
        if capacity > 0 {
            let branch = routes
                .route_with_options(
                    routes.root_id().unwrap(),
                    RoutePattern::path("broken").unwrap(),
                    RouteNodeOptions::new().with_view(layout.clone()),
                )
                .unwrap();
            routes
                .route_with_options(
                    branch,
                    RoutePattern::path(":id").unwrap(),
                    RouteNodeOptions::new()
                        .with_view(view.clone())
                        .with_keep_alive(true),
                )
                .unwrap();
        }
    }
    let mut views = RouteViewRegistry::new();
    views
        .insert(view, RouteViewDescriptor::new(driver))
        .unwrap();
    views
        .insert(layout, RouteViewDescriptor::new(layout_driver))
        .unwrap();
    views
        .insert(fallback.clone(), RouteViewDescriptor::new(fallback_driver))
        .unwrap();
    let router = app
        .add_ui_router(
            RouterConfig::new(routes.build().unwrap(), views, fallback)
                .with_keep_alive_capacity(capacity),
        )
        .unwrap();
    let host = app
        .world_mut()
        .spawn((Node::default(), RouterHostRegistration::new(router)))
        .id();
    app.update();
    (app, router, host, protected, failure)
}

fn settle(app: &mut App) {
    for _ in 0..20 {
        app.update();
    }
}

#[derive(Component, Clone, Default)]
struct AppCleanupNotice;

#[derive(Component)]
struct AppHostCleanupObserved;

#[derive(Resource)]
struct AppCleanupHost(Entity);

#[derive(Component)]
struct AppHostChildrenObserved;

#[derive(Component)]
struct AppUnrelatedSpawnObserved;

#[test]
fn native_host_admission_allows_app_children_observer_to_mark_host() {
    use bevy_ecs::prelude::{Commands, Insert, Local, On};
    let mut app = App::new();
    app.add_plugins((
        TaskPoolPlugin::default(),
        AssetPlugin::default(),
        ScenePlugin,
    ));
    let driver = app
        .register_bsn_view(ViewDriverKey::new("app-host-marker").unwrap(), |_| {
            Ok(bevy_scene::bsn! { Node })
        })
        .unwrap();
    let view = RouteViewKey::new("app-host-marker").unwrap();
    let routes = RouteTree::builder()
        .root_with_options(
            RoutePattern::path("/").unwrap(),
            RouteNodeOptions::new().with_view(view.clone()),
        )
        .unwrap()
        .build()
        .unwrap();
    let mut views = RouteViewRegistry::new();
    views
        .insert(view.clone(), RouteViewDescriptor::new(driver))
        .unwrap();
    let router = app
        .add_ui_router(RouterConfig::new(routes, views, view))
        .unwrap();
    let host = app
        .world_mut()
        .spawn((Node::default(), RouterHostRegistration::new(router)))
        .id();
    let unrelated = app.world_mut().spawn_empty().id();
    let removable = app.world_mut().spawn_empty().id();
    app.world_mut().add_observer(
        move |event: On<Insert, Children>, mut commands: Commands, mut observed: Local<bool>| {
            if event.entity == host && !*observed {
                *observed = true;
                commands.entity(host).insert(AppHostChildrenObserved);
                commands.entity(unrelated).insert(AppHostChildrenObserved);
                commands.spawn(AppUnrelatedSpawnObserved);
                commands.entity(removable).despawn();
            }
        },
    );
    settle(&mut app);
    assert!(
        app.world().get::<AppHostChildrenObserved>(host).is_some(),
        "the application's public Children observer must execute"
    );
    assert!(
        app.world()
            .get::<AppHostChildrenObserved>(unrelated)
            .is_some()
    );
    assert!(app.world().get_entity(removable).is_err());
    let mut spawned = app.world_mut().query::<&AppUnrelatedSpawnObserved>();
    assert_eq!(spawned.iter(app.world()).count(), 1);
    let entry = &app.world().resource::<RouterNavigation>().entries[0];
    assert_eq!(
        entry.engine.snapshot().availability(),
        &RouterAvailability::Ready,
        "an unrelated app host marker must not reject host admission"
    );
    let host_before = ProjectionlessHostProjection::from(entry.host.as_ref().unwrap());
    let request = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, RouteLocation::new("/"))
        .unwrap();
    settle(&mut app);
    let entry = &app.world().resource::<RouterNavigation>().entries[0];
    assert_eq!(
        entry.engine.request(request).unwrap().status(),
        &RequestStatus::Succeeded
    );
    assert_eq!(entry.engine.active().kind(), ActivePresentationKind::Route);
    assert_eq!(
        entry.engine.active().location(),
        Some(&RouteLocation::new("/"))
    );
    assert!(entry.engine.snapshot().running_request().is_none());
    assert!(!entry.cleanup_summary().foreground_pending());
    assert_eq!(
        ProjectionlessHostProjection::from(entry.host.as_ref().unwrap()),
        host_before
    );
    assert!(app.world().get::<AppHostChildrenObserved>(host).is_some());
}

#[test]
fn native_failed_child_cleanup_allows_app_hook_to_mark_host() {
    let mut app = App::new();
    app.add_plugins((
        TaskPoolPlugin::default(),
        AssetPlugin::default(),
        ScenePlugin,
    ));
    app.world_mut()
        .register_component_hooks::<AppCleanupNotice>()
        .on_despawn(|mut world, _| {
            let host = world.resource::<AppCleanupHost>().0;
            world.commands().entity(host).insert(AppHostCleanupObserved);
        });
    let layout_driver = app
        .register_bsn_view(ViewDriverKey::new("app-hook-layout").unwrap(), |_| {
            Ok(bevy_scene::bsn! { Node AppCleanupNotice bevy_ui_router_bsn::DefaultOutlet })
        })
        .unwrap();
    let child_driver = app
        .register_bsn_view(ViewDriverKey::new("app-hook-child").unwrap(), |input| {
            if input.location().path() == "/broken" {
                return Err(bevy_ui_router_bsn::BsnBuildError::new(
                    "application child build failed",
                ));
            }
            Ok(bevy_scene::bsn! { Node })
        })
        .unwrap();
    let reported_failure = Arc::new(std::sync::Mutex::new(None));
    let fallback_failure = reported_failure.clone();
    let fallback_driver = app
        .register_bsn_view(
            ViewDriverKey::new("app-hook-fallback").unwrap(),
            move |input| {
                *fallback_failure.lock().unwrap() = input.failure().cloned();
                Ok(bevy_scene::bsn! { Node })
            },
        )
        .unwrap();
    let layout = RouteViewKey::new("app-hook-layout").unwrap();
    let child = RouteViewKey::new("app-hook-child").unwrap();
    let fallback = RouteViewKey::new("app-hook-fallback").unwrap();
    let mut routes = RouteTree::builder()
        .root_with_options(
            RoutePattern::path("/").unwrap(),
            RouteNodeOptions::new().with_view(layout.clone()),
        )
        .unwrap();
    routes
        .route_with_options(
            routes.root_id().unwrap(),
            RoutePattern::path("broken").unwrap(),
            RouteNodeOptions::new().with_view(child.clone()),
        )
        .unwrap();
    let mut views = RouteViewRegistry::new();
    views
        .insert(layout, RouteViewDescriptor::new(layout_driver))
        .unwrap();
    views
        .insert(child, RouteViewDescriptor::new(child_driver))
        .unwrap();
    views
        .insert(fallback.clone(), RouteViewDescriptor::new(fallback_driver))
        .unwrap();
    let router = app
        .add_ui_router(RouterConfig::new(routes.build().unwrap(), views, fallback))
        .unwrap();
    let host = app
        .world_mut()
        .spawn((Node::default(), RouterHostRegistration::new(router)))
        .id();
    app.insert_resource(AppCleanupHost(host));
    app.update();
    let host_before = ProjectionlessHostProjection::from(
        app.world().resource::<RouterNavigation>().entries[0]
            .host
            .as_ref()
            .unwrap(),
    );
    let request = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, RouteLocation::new("/broken"))
        .unwrap();
    settle(&mut app);
    assert!(
        app.world().get::<AppHostCleanupObserved>(host).is_some(),
        "the application's own Scene component hook must execute"
    );
    let entry = &app.world().resource::<RouterNavigation>().entries[0];
    assert!(
        matches!(
            entry.engine.request(request).unwrap().status(),
            RequestStatus::Failed(_)
        ),
        "child failure must finish recovery after the app host marker changes: {:?}",
        entry.engine.request(request).unwrap().status()
    );
    assert!(entry.engine.snapshot().running_request().is_none());
    assert!(!entry.cleanup_summary().foreground_pending());
    let original = reported_failure
        .lock()
        .unwrap()
        .clone()
        .expect("fallback receives the original build failure");
    assert_eq!(original.kind(), NavigationFailureKind::Build);
    assert!(
        original
            .message()
            .contains("application child build failed")
    );
    assert_eq!(
        entry.engine.request(request).unwrap().status(),
        &RequestStatus::Failed(original.clone())
    );
    assert_eq!(entry.engine.active().kind(), ActivePresentationKind::Error);
    assert_eq!(
        entry.engine.active().requested(),
        Some(&RouteLocation::new("/broken"))
    );
    assert_eq!(entry.engine.active().failure(), Some(&original));
    assert_eq!(entry.engine.active().frames().len(), 1);
    assert_eq!(
        entry.engine.active().frames()[0].origin(),
        &FrameOrigin::RouterFallback
    );
    assert!(entry.engine.history().is_empty());
    assert_eq!(
        ProjectionlessHostProjection::from(entry.host.as_ref().unwrap()),
        host_before
    );
    assert!(app.world().get_entity(host).is_ok());

    let next = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, RouteLocation::new("/"))
        .unwrap();
    settle(&mut app);
    let entry = &app.world().resource::<RouterNavigation>().entries[0];
    assert_eq!(
        entry.engine.request(next).unwrap().status(),
        &RequestStatus::Succeeded
    );
    assert_eq!(
        entry.engine.request(request).unwrap().status(),
        &RequestStatus::Failed(original)
    );
    assert_eq!(entry.engine.active().kind(), ActivePresentationKind::Route);
    assert_eq!(
        entry.engine.active().location(),
        Some(&RouteLocation::new("/"))
    );
    assert_eq!(
        entry.engine.snapshot().availability(),
        &RouterAvailability::Ready
    );
    assert!(entry.engine.snapshot().running_request().is_none());
    assert!(!entry.cleanup_summary().foreground_pending());
    assert_eq!(
        ProjectionlessHostProjection::from(entry.host.as_ref().unwrap()),
        host_before
    );
    assert!(app.world().get::<AppHostCleanupObserved>(host).is_some());
}

#[derive(Clone, Copy)]
enum NativePublishedCase {
    Normal,
    Fallback,
    BoundError,
}

fn assert_native_published_host_loss_keeps_result(case: NativePublishedCase) {
    let mut app = App::new();
    app.add_plugins((
        TaskPoolPlugin::default(),
        AssetPlugin::default(),
        ScenePlugin,
    ));
    let normal_driver = app
        .register_bsn_view(ViewDriverKey::new("post-error-normal").unwrap(), |input| {
            if input.location().path() == "/broken" {
                return Err(bevy_ui_router_bsn::BsnBuildError::new(
                    "original native build failure",
                ));
            }
            Ok(bevy_scene::bsn! { Node bevy_ui_router_bsn::DefaultOutlet })
        })
        .unwrap();
    let original = Arc::new(std::sync::Mutex::new(None));
    let observed = original.clone();
    let error_driver = app
        .register_bsn_view(
            ViewDriverKey::new("post-error-view").unwrap(),
            move |input| {
                *observed.lock().unwrap() = input.failure().cloned();
                Ok(bevy_scene::bsn! { Node })
            },
        )
        .unwrap();
    let normal = RouteViewKey::new("post-error-normal").unwrap();
    let error = RouteViewKey::new("post-error-view").unwrap();
    let mut options = RouteNodeOptions::new().with_view(normal.clone());
    if matches!(case, NativePublishedCase::BoundError) {
        options = options.with_error_view(
            ErrorBoundaryKey::named("post-error-boundary").unwrap(),
            error.clone(),
        );
    }
    let mut routes = RouteTree::builder()
        .root_with_options(RoutePattern::path("/").unwrap(), options)
        .unwrap();
    routes
        .route_with_options(
            routes.root_id().unwrap(),
            RoutePattern::path("broken").unwrap(),
            RouteNodeOptions::new().with_view(normal.clone()),
        )
        .unwrap();
    let mut views = RouteViewRegistry::new();
    views
        .insert(normal, RouteViewDescriptor::new(normal_driver))
        .unwrap();
    views
        .insert(error.clone(), RouteViewDescriptor::new(error_driver))
        .unwrap();
    let router = app
        .add_ui_router(RouterConfig::new(routes.build().unwrap(), views, error))
        .unwrap();
    let host = app
        .world_mut()
        .spawn((Node::default(), RouterHostRegistration::new(router)))
        .id();
    app.update();
    let first = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, RouteLocation::new("/"))
        .unwrap();
    settle(&mut app);
    let history = {
        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert_eq!(
            entry.engine.request(first).unwrap().status(),
            &RequestStatus::Succeeded
        );
        entry.engine.history().clone()
    };
    let location = match case {
        NativePublishedCase::Normal => RouteLocation::new("/").with_fragment("next"),
        NativePublishedCase::Fallback => RouteLocation::new("/unmatched"),
        NativePublishedCase::BoundError => RouteLocation::new("/broken"),
    };
    let request = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, location)
        .unwrap();
    // Exercise the private production stages so the actual post-publication,
    // pre-cleanup window remains observable even when one update settles it.
    for _ in 0..8 {
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        commit_presentation_system(app.world_mut());
        if app.world().resource::<RouterNavigation>().entries[0]
            .engine
            .request(request)
            .unwrap()
            .status()
            == &RequestStatus::Running(NavigationPhase::PostCommitCleanup)
        {
            break;
        }
        settle_cleanup_system(app.world_mut());
    }
    let committed_history = {
        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert_eq!(
            entry.engine.active().kind(),
            if matches!(case, NativePublishedCase::Normal) {
                ActivePresentationKind::Route
            } else {
                ActivePresentationKind::Error
            }
        );
        assert_eq!(
            entry.engine.request(request).unwrap().status(),
            &RequestStatus::Running(NavigationPhase::PostCommitCleanup)
        );
        if matches!(case, NativePublishedCase::Normal) {
            assert_eq!(entry.engine.history().len(), history.len() + 1);
        } else {
            assert_eq!(entry.engine.history(), &history);
        }
        assert!(entry.cleanup_ledger.is_some());
        entry.engine.history().clone()
    };
    let expected = if matches!(case, NativePublishedCase::Normal) {
        assert!(original.lock().unwrap().is_none());
        RequestStatus::Succeeded
    } else {
        let original = original
            .lock()
            .unwrap()
            .clone()
            .expect("the native error factory receives the original failure");
        assert_eq!(
            original.kind(),
            if matches!(case, NativePublishedCase::BoundError) {
                NavigationFailureKind::Build
            } else {
                NavigationFailureKind::Match
            }
        );
        RequestStatus::Failed(original)
    };
    let queued = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, RouteLocation::new("/"))
        .unwrap();
    app.world_mut()
        .entity_mut(host)
        .remove::<RouterHostRegistration>();
    for _ in 0..8 {
        settle_cleanup_system(app.world_mut());
        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        if !matches!(
            entry.engine.request(request).unwrap().status(),
            RequestStatus::Running(_)
        ) {
            break;
        }
    }
    let assert_terminal = |app: &App| {
        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert_eq!(entry.engine.history(), &committed_history);
        assert_eq!(entry.engine.active().kind(), ActivePresentationKind::None);
        assert_eq!(
            entry.engine.snapshot().availability(),
            &RouterAvailability::Unavailable(RouterUnavailableReason::HostLost)
        );
        assert!(
            entry.active.is_none()
                && entry.host.is_none()
                && entry.cleanup_ledger.is_none()
                && entry.post_commit_compound.is_none()
                && entry.pending_frame_drive.is_none()
                && entry.pending_failed_branch.is_none()
        );
        assert!(entry.engine.snapshot().running_request().is_none());
        let queued_status = entry.engine.request(queued).unwrap().status().clone();
        assert!(
            matches!(&queued_status, RequestStatus::Failed(failure) if failure.kind() == NavigationFailureKind::HostLost)
        );
        let record = entry.engine.request(request).unwrap();
        assert_eq!(
            record
                .trace()
                .iter()
                .filter(|event| event.kind() == NavigationTraceEventKind::RequestTerminal)
                .count(),
            1
        );
        assert_eq!(
            record
                .diagnostics()
                .iter()
                .filter(|event| event.kind() == DiagnosticEventKind::PostCommitCleanupAnomaly)
                .count(),
            1
        );
        assert!(app.world().get::<Node>(host).is_some());
        assert_eq!(record.status(), &expected);
        (
            record.trace().to_vec(),
            record.diagnostics().to_vec(),
            queued_status,
            entry.engine.snapshot(),
        )
    };
    let settled = assert_terminal(&app);
    app.update();
    assert_eq!(assert_terminal(&app), settled);

    let snapshots = app.world().resource::<RouterSnapshots>();
    let RequestLookup::Found(published_request) = snapshots.request(router, request) else {
        panic!("the completed request must remain publicly observable");
    };
    assert_eq!(published_request.status(), &expected);
    assert_eq!(published_request.trace(), settled.0);
    assert_eq!(published_request.diagnostics(), settled.1);
    let RequestLookup::Found(published_queued) = snapshots.request(router, queued) else {
        panic!("the queued HostLost result must remain publicly observable");
    };
    assert_eq!(published_queued.status(), &settled.2);
    let published_router = snapshots.get(router).unwrap();
    assert_eq!(published_router.history(), settled.3.history());
    assert_eq!(published_router.active(), settled.3.active());
    assert_eq!(published_router.availability(), settled.3.availability());
    assert_eq!(published_router.diagnostics(), settled.3.diagnostics());
    assert!(published_router.running_request().is_none());
    assert!(published_router.pending_phase().is_none());
    assert!(!published_router.cleanup().foreground_pending());
}

#[test]
fn native_published_normal_post_commit_host_loss_keeps_success() {
    assert_native_published_host_loss_keeps_result(NativePublishedCase::Normal);
}

#[test]
fn native_post_commit_host_loss_settles_live_and_already_retiring_cached_leaves() {
    let (mut app, router, host, protected, _) =
        fixture_with_retention(false, true, false, 3, false);
    app.world_mut().entity_mut(host).add_child(protected);
    let mut deep_cached_shell = None;
    for path in ["/child/1", "/child/2", "/broken/1", "/broken/2"] {
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new(path))
            .unwrap();
        settle(&mut app);
        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert_eq!(
            entry.engine.request(request).unwrap().status(),
            &RequestStatus::Succeeded
        );
        if path == "/broken/1" {
            deep_cached_shell = Some(
                entry
                    .active
                    .as_ref()
                    .unwrap()
                    .frames()
                    .last()
                    .unwrap()
                    .shell(),
            );
        }
    }
    let deep_cached_shell = deep_cached_shell.unwrap();
    let (version, mut owned) = {
        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        let inventory = entry.retained.inventory().unwrap();
        assert_eq!(inventory.len(), 3);
        let mut owned = entry
            .active
            .as_ref()
            .unwrap()
            .claims()
            .map(ExactRegistryClaim::entity)
            .collect::<Vec<_>>();
        for owner in &inventory {
            owned.extend(
                entry
                    .retained
                    .payload(owner.projection().frame())
                    .unwrap()
                    .claims()
                    .map(ExactRegistryClaim::entity),
            );
        }
        let binding = entry.host.as_ref().unwrap();
        owned.push(binding.top_slot);
        (binding.version.clone(), owned)
    };
    let request = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, RouteLocation::new("/child/3"))
        .unwrap();
    // Preserve the real published-but-not-yet-cleaned window. The broken-parent
    // cache is already in the normal batch; the root-parent caches are still live.
    for _ in 0..8 {
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        commit_presentation_system(app.world_mut());
        if app.world().resource::<RouterNavigation>().entries[0]
            .engine
            .request(request)
            .unwrap()
            .status()
            == &RequestStatus::Running(NavigationPhase::PostCommitCleanup)
        {
            break;
        }
        settle_cleanup_system(app.world_mut());
    }
    let committed_history = {
        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert_eq!(
            entry.engine.request(request).unwrap().status(),
            &RequestStatus::Running(NavigationPhase::PostCommitCleanup)
        );
        let inventory = entry.retained.inventory().unwrap();
        assert_eq!(inventory.len(), 2);
        assert!(entry.cleanup_ledger.is_some());
        let registry = app.world().resource::<RouteEntityRegistry>();
        assert_eq!(
            registry.record(deep_cached_shell).unwrap().lifecycle(),
            EntityLifecycle::Leaving
        );
        for owner in inventory {
            let branch = entry.retained.payload(owner.projection().frame()).unwrap();
            assert_eq!(
                registry
                    .record(branch.frames()[0].shell())
                    .unwrap()
                    .lifecycle(),
                EntityLifecycle::Retained
            );
        }
        owned.extend(
            entry
                .active
                .as_ref()
                .unwrap()
                .claims()
                .map(ExactRegistryClaim::entity),
        );
        entry.engine.history().clone()
    };
    app.world_mut()
        .entity_mut(host)
        .remove::<RouterHostRegistration>();
    settle(&mut app);
    {
        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        let record = entry.engine.request(request).unwrap();
        assert_eq!(record.status(), &RequestStatus::Succeeded);
        assert_eq!(entry.engine.history(), &committed_history);
        assert_eq!(
            entry.engine.snapshot().availability(),
            &RouterAvailability::Unavailable(RouterUnavailableReason::HostLost)
        );
        assert!(entry.retained.is_empty());
        assert!(entry.active.is_none() && entry.host.is_none());
        assert!(entry.cleanup_ledger.is_none() && entry.post_commit_compound.is_none());
        assert!(entry.pending_commit.is_none() && entry.pending_failed_branch.is_none());
        assert!(entry.authority_protocol_residual.is_none() && entry.native_residuals.is_empty());
        assert!(entry.engine.snapshot().running_request().is_none());
        assert_eq!(
            record
                .trace()
                .iter()
                .filter(|event| event.kind() == NavigationTraceEventKind::RequestTerminal)
                .count(),
            1
        );
        assert_eq!(
            record
                .diagnostics()
                .iter()
                .filter(|event| event.kind() == DiagnosticEventKind::PostCommitCleanupAnomaly)
                .count(),
            1
        );
    }
    assert!(
        owned
            .iter()
            .all(|entity| app.world().get_entity(*entity).is_err())
    );
    assert!(app.world().get::<Node>(host).is_some());
    assert_eq!(
        app.world().get::<Node>(protected).unwrap().width,
        Val::Px(91.0)
    );
    assert_eq!(
        app.world().get::<ChildOf>(protected).map(ChildOf::parent),
        Some(host)
    );
    app.world_mut()
        .entity_mut(host)
        .insert(RouterHostRegistration::new(router));
    settle(&mut app);
    {
        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert_ne!(entry.host.as_ref().unwrap().version, version);
        assert_eq!(entry.engine.history(), &committed_history);
        assert!(entry.active.is_none() && entry.engine.snapshot().running_request().is_none());
    }
    let next = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, RouteLocation::new("/child/1"))
        .unwrap();
    settle(&mut app);
    assert_eq!(
        app.world().resource::<RouterNavigation>().entries[0]
            .engine
            .request(next)
            .unwrap()
            .status(),
        &RequestStatus::Succeeded
    );
}

#[test]
fn native_delayed_retained_proof_removal_preserves_another_router_cache() {
    let (mut app, router, _, _, _) = fixture_with_retention(false, true, false, 2, false);
    for path in ["/child/1", "/child/2"] {
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new(path))
            .unwrap();
        settle(&mut app);
    }
    let removal = {
        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        let inventory = entry.retained.inventory().unwrap();
        assert_eq!(inventory.len(), 1);
        app.world()
            .resource::<RouteEntityRegistry>()
            .prepare_retained_boundaries(&[], &[inventory[0].projection().clone()])
            .unwrap()
    };
    let driver = app
        .register_bsn_view(ViewDriverKey::new("other-router-cache").unwrap(), |_| {
            Ok(bevy_scene::bsn! { Node bevy_ui_router_bsn::DefaultOutlet })
        })
        .unwrap();
    let view = RouteViewKey::new("other-router-cache").unwrap();
    let mut routes = RouteTree::builder()
        .root_with_options(
            RoutePattern::path("/").unwrap(),
            RouteNodeOptions::new().with_view(view.clone()),
        )
        .unwrap();
    routes
        .route_with_options(
            routes.root_id().unwrap(),
            RoutePattern::path(":id").unwrap(),
            RouteNodeOptions::new()
                .with_view(view.clone())
                .with_keep_alive(true),
        )
        .unwrap();
    let mut views = RouteViewRegistry::new();
    views
        .insert(view.clone(), RouteViewDescriptor::new(driver))
        .unwrap();
    let other = app
        .add_ui_router(
            RouterConfig::new(routes.build().unwrap(), views, view).with_keep_alive_capacity(2),
        )
        .unwrap();
    app.world_mut()
        .spawn((Node::default(), RouterHostRegistration::new(other)));
    settle(&mut app);
    for path in ["/a", "/b"] {
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(other, RouteLocation::new(path))
            .unwrap();
        settle(&mut app);
        assert_eq!(
            app.world().resource::<RouterNavigation>().entries[1]
                .engine
                .request(request)
                .unwrap()
                .status(),
            &RequestStatus::Succeeded
        );
    }
    let cache_is_current = |app: &App, index: usize| {
        let entry = &app.world().resource::<RouterNavigation>().entries[index];
        let inventory = entry.retained.inventory().unwrap();
        assert_eq!(inventory.len(), 1);
        let projection = inventory[0].projection();
        let branch = entry.retained.payload(projection.frame()).unwrap();
        app.world()
            .resource::<RouteEntityRegistry>()
            .retained_payload_is_current(
                app.world(),
                projection,
                &branch.frames()[0],
                branch.child_slots(),
            )
    };
    assert!(cache_is_current(&app, 0));
    assert!(cache_is_current(&app, 1));
    removal.apply(&mut app.world_mut().resource_mut::<RouteEntityRegistry>());
    assert!(!cache_is_current(&app, 0));
    assert!(
        cache_is_current(&app, 1),
        "a delayed exact removal must preserve another Router's newer cache"
    );
}

#[test]
fn native_published_fallback_post_commit_host_loss_keeps_original_failure() {
    assert_native_published_host_loss_keeps_result(NativePublishedCase::Fallback);
}

#[test]
fn native_published_bound_error_post_commit_host_loss_keeps_original_failure() {
    assert_native_published_host_loss_keeps_result(NativePublishedCase::BoundError);
}

#[test]
fn native_post_commit_isolation_keeps_authored_values_and_rejects_content_drift() {
    for drift in [false, true] {
        let (mut app, router, host, _, _) = fixture(false);
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        settle(&mut app);
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/").with_fragment("next"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        commit_presentation_system(app.world_mut());
        app.world_mut()
            .entity_mut(host)
            .remove::<RouterHostRegistration>();
        app.world_mut()
            .resource_scope(|world, mut navigation: Mut<RouterNavigation>| {
                let entry = &mut navigation.entries[0];
                let active = entry.active.as_ref().unwrap();
                let host = entry.host.as_ref().unwrap();
                let root = active.frames()[0].native_root().unwrap();
                let authored = *world.get::<Visibility>(root).unwrap();
                assert_eq!(authored, Visibility::Inherited);
                let original = entry.cleanup_ledger.as_mut().unwrap();
                let original_freeze = original
                    .freeze_for_post_commit_transfer(world)
                    .ok()
                    .unwrap();
                let mut authority = PreparedPostCommitAuthorityLedger::freeze_and_reserve(
                    world,
                    active,
                    host,
                    &original_freeze,
                    &[],
                )
                .ok()
                .unwrap();
                let edge = FrozenRetiredHostEdge::freeze(world, host).unwrap();
                let isolation = prepare_post_commit_input_isolation(
                    world,
                    active,
                    &original_freeze,
                    &authority,
                )
                .ok()
                .unwrap();
                let removed = original_freeze
                    .removed_entities_for_authority_refreeze(world, original, None)
                    .unwrap();
                assert!(
                    isolation
                        .apply_guarded(world, &|world| {
                            original_freeze.refreeze(world, original, None)
                                && authority
                                    .partition()
                                    .refreeze_allowing_input_isolation(world, &removed)
                                && edge.host_projection_is_current(world, false)
                        })
                        .is_ok()
                );
                assert_eq!(
                    world.get::<Visibility>(root),
                    Some(&authored),
                    "authorized isolation only writes the shell"
                );
                if drift {
                    *world.get_mut::<Visibility>(root).unwrap() = Visibility::Hidden;
                }
                assert_eq!(
                    authority.commit_input_isolation(world, &original_freeze, original),
                    !drift,
                    "the production refreeze must preserve exact Scene content values"
                );
            });
    }
}

#[test]
fn native_outlet_loss_after_scene_preparation_keeps_exact_current_task_source() {
    let (mut app, router, _, _, _) = fixture_variant(false, true);
    app.world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, RouteLocation::new("/child"))
        .unwrap();
    settle(&mut app);
    let request = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .replace(
            router,
            RouteLocation::new("/child").with_fragment("after-prepare"),
        )
        .unwrap();
    collect_navigation_system(app.world_mut());
    let (task, pending, anchor, history) = {
        let mut navigation = app.world_mut().resource_mut::<RouterNavigation>();
        let entry = &mut navigation.entries[0];
        let EngineAction::FrameTask(task) = entry.engine.action().unwrap() else {
            panic!("real leaf task");
        };
        let pending = entry
            .pending_commit
            .take()
            .unwrap_or_else(PendingCommit::new);
        let slot = &entry.active.as_ref().unwrap().child_slots()[0];
        assert!(slot.anchor_binding().is_native());
        (task, pending, slot.anchor(), entry.engine.history().clone())
    };
    // Resolve a real ordinary Scene first, then change the actual bound marker
    // before the production realization check. A pre-factory check alone cannot
    // authorize this later mount or settle its failed owner.
    let mut factory: bevy_ui_router_bsn::BsnSceneFactory =
        Box::new(|_| Ok(Box::new(bevy_scene::bsn! { Node })));
    let prepared = crate::bsn::prepare_bsn_view(app.world(), &mut factory, task.input()).unwrap();
    app.world_mut()
        .entity_mut(anchor)
        .remove::<bevy_ui_router_bsn::DefaultOutlet>();
    app.world_mut()
        .resource_scope(|world, mut navigation: Mut<RouterNavigation>| {
            let entry = &mut navigation.entries[0];
            let host = entry.host.as_ref().unwrap();
            let result = realize_native_task(
                world,
                &entry.engine,
                host.candidate.clone(),
                host.top_slot,
                host.top_slot_claim.clone(),
                Some(&pending.branch),
                entry.active.as_ref(),
                &entry.retained,
                &task,
                prepared,
            );
            let Err(crate::presentation::native_frame::NativeFrameFailure::Creation(failure)) =
                result
            else {
                panic!("missing preserved binding fails before allocation");
            };
            let failure =
                crate::presentation::native_failure::NativeFailureOwner::creation(&task, failure);
            reject_native_task(world, entry, pending, task, ViewFailureKind::Apply, failure);
            let mut failed = entry.pending_failed_branch.take().unwrap();
            let slot =
                exact_current_task_outlet(&entry.engine, &failed, entry.active.as_ref()).unwrap();
            assert_eq!(slot.anchor(), anchor);
            assert!(failed_branch_primary_claims(&failed.branch).is_empty());
            let mut failure = failed.branch.take_native_failure().unwrap();
            let source = failure.source.clone();
            failure.source.frame = entry.active.as_ref().unwrap().frames()[0].frame().clone();
            failed.branch = failed.branch.with_native_failure(failure);
            assert!(
                exact_current_task_outlet(&entry.engine, &failed, entry.active.as_ref()).is_none(),
                "a different native failure identity cannot authorize current retirement"
            );
            let mut failure = failed.branch.take_native_failure().unwrap();
            failure.source = source;
            failed.branch = failed.branch.with_native_failure(failure);
            let slot =
                exact_current_task_outlet(&entry.engine, &failed, entry.active.as_ref()).unwrap();
            let mut manifest = FrozenCleanupManifest::from_failed_prepared_branch(
                world,
                world.resource::<RouteEntityRegistry>(),
                &failed.branch,
            )
            .unwrap();
            manifest
                .observe_current_task_outlet(world, world.resource::<RouteEntityRegistry>(), slot)
                .unwrap();
            assert_eq!(
                manifest.len(),
                0,
                "current slot was not forged into a failed obligation"
            );
            assert_eq!(manifest.boundary_drifts().len(), 1);
            assert!(current_task_outlet_matches_drift(
                slot,
                &manifest.boundary_drifts()[0]
            ));
            let FailedManifestDisposition::CurrentAuthorityLoss { failed, .. } =
                classify_failed_manifest(
                    world,
                    &entry.engine,
                    manifest,
                    failed,
                    entry.active.as_ref(),
                    entry.host.as_ref(),
                )
            else {
                panic!("the exact current outlet is authority-loss evidence");
            };
            entry.pending_failed_branch = Some(failed);
        });
    settle(&mut app);
    let entry = &app.world().resource::<RouterNavigation>().entries[0];
    assert!(
        matches!(entry.engine.request(request).unwrap().status(), RequestStatus::Failed(failure) if failure.kind() == NavigationFailureKind::AuthorityLost)
    );
    assert_eq!(entry.engine.history(), &history);
    assert_eq!(entry.engine.active().kind(), ActivePresentationKind::None);
    assert!(entry.pending_failed_branch.is_none() && entry.cleanup_ledger.is_none());
    assert!(entry.authority_protocol_residual.is_none());
}

#[test]
fn native_allocated_child_retains_exact_external_outlet_evidence() {
    let (mut app, router, _, _, _) = fixture_variant(false, true);
    app.world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, RouteLocation::new("/child"))
        .unwrap();
    settle(&mut app);
    app.world_mut()
        .resource_mut::<RouterNavigation>()
        .replace(
            router,
            RouteLocation::new("/child").with_fragment("allocated"),
        )
        .unwrap();
    collect_navigation_system(app.world_mut());
    app.world_mut().resource_scope(|world, mut navigation: Mut<RouterNavigation>| {
        let entry = &mut navigation.entries[0];
        let EngineAction::FrameTask(task) = entry.engine.action().unwrap() else { panic!("real leaf task"); };
        let mut pending = entry.pending_commit.take().unwrap_or_else(PendingCommit::new);
        let host = entry.host.as_ref().unwrap();
        let mut factory: bevy_ui_router_bsn::BsnSceneFactory = Box::new(|_| Ok(Box::new(bevy_scene::bsn! { Node })));
        let prepared = crate::bsn::prepare_bsn_view(world, &mut factory, task.input()).unwrap();
        let frame = realize_native_task(world, &entry.engine, host.candidate.clone(), host.top_slot,
            host.top_slot_claim.clone(), Some(&pending.branch), entry.active.as_ref(),
                &entry.retained, &task, prepared)
            .unwrap_or_else(|_| panic!("actual native child allocation succeeds"));
        let shell = frame.shell();
        let slot = frame.mount_binding().slot();
        let anchor = frame.mount_binding().physical_parent().0;
        assert!(pending.branch.push_frame(frame).is_ok());
        // The allocated pending frame is a real cleanup obligation. Its mount
        // remains external to pending, even when the native marker then disappears.
        world.entity_mut(anchor).remove::<bevy_ui_router_bsn::DefaultOutlet>();
        let failed = FailedPreparedBranch::new(pending.branch);
        let manifest = FrozenCleanupManifest::from_failed_prepared_branch(world, world.resource::<RouteEntityRegistry>(), &failed).unwrap();
        let drift = manifest.boundary_drifts().iter().find(|drift| matches!(
            drift.source, FailedBoundarySource::FramePhysicalParent { mount_slot, removed_child, .. }
                if mount_slot == slot && removed_child == shell
        )).expect("allocated native child freezes its actual external outlet");
        assert!(exact_failed_source_obligation(drift, &failed).is_some());
        assert!(matches!(classify_failed_boundary(drift.clone(), &failed, entry.active.as_ref(), entry.host.as_ref()),
            FailedBoundaryDisposition::CurrentAuthorityLoss(_)));
        let mut wrong_source = drift.clone();
        let FailedBoundarySource::FramePhysicalParent { mount_slot, .. } = &mut wrong_source.source else { unreachable!() };
        *mount_slot = anchor;
        assert!(matches!(classify_failed_boundary(wrong_source, &failed, entry.active.as_ref(), entry.host.as_ref()),
            FailedBoundaryDisposition::AmbiguousInvalidTopology(_)),
            "the native role alone does not authorize an unrelated physical source");
    });
}

#[test]
fn issued_native_fallback_host_loss_covers_original_active_and_readmits_without_factory() {
    for (with_active, retry) in [(false, false), (true, false), (true, true)] {
        let (mut app, router, host, protected, _) = fixture(false);
        if with_active {
            app.world_mut()
                .resource_mut::<RouterNavigation>()
                .push(router, RouteLocation::new("/"))
                .unwrap();
            settle(&mut app);
            let root = app.world().resource::<RouterNavigation>().entries[0]
                .active
                .as_ref()
                .unwrap()
                .frames()[0]
                .native_root()
                .unwrap();
            app.world_mut().spawn((Node::default(), ChildOf(root)));
            app.update();
        }
        let history = app.world().resource::<RouterNavigation>().entries[0]
            .engine
            .history()
            .clone();
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/unmatched"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        // Move the genuinely issued, not-yet-executed fallback task into the
        // production retry holder; no lifecycle classification is injected.
        let identity = {
            let entry = &mut app.world_mut().resource_mut::<RouterNavigation>().entries[0];
            let EngineAction::FrameTask(task) = entry.engine.action().unwrap() else {
                panic!("real fallback task expected");
            };
            assert_eq!(
                task.frame().origin(),
                &bevy_ui_router_core::frame::FrameOrigin::RouterFallback
            );
            let identity = task.report_identity();
            let active = entry.active.take().unwrap_or_else(RealizedBranch::empty);
            retain_frame_task_after_authority_retry(
                entry,
                with_active,
                active,
                PendingCommit::new(),
                task,
                None,
            );
            identity
        };
        app.world_mut()
            .entity_mut(host)
            .remove::<RouterHostRegistration>();
        if retry {
            app.insert_resource(
                crate::presentation::native_frame::NativeCleanupCapacityRequest {
                    additional_targets: usize::MAX,
                },
            );
            app.update();
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            assert!(
                entry
                    .pending_frame_drive
                    .as_ref()
                    .unwrap()
                    .task
                    .report_identity()
                    == identity
            );
            assert!(entry.active.is_some() && entry.host.is_some());
            assert_eq!(
                app.world()
                    .resource::<NativeFallbackCalls>()
                    .0
                    .load(Ordering::Relaxed),
                0
            );
            app.world_mut()
                .remove_resource::<crate::presentation::native_frame::NativeCleanupCapacityRequest>(
                );
        }
        settle(&mut app);
        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert!(
            matches!(entry.engine.request(request).unwrap().status(), RequestStatus::Failed(failure) if failure.kind() == NavigationFailureKind::HostLost)
        );
        assert_eq!(entry.engine.history(), &history);
        assert_eq!(entry.engine.active().kind(), ActivePresentationKind::None);
        assert!(
            entry.active.is_none()
                && entry.pending_frame_drive.is_none()
                && entry.authority_protocol_residual.is_none()
        );
        assert_eq!(
            app.world()
                .resource::<NativeFallbackCalls>()
                .0
                .load(Ordering::Relaxed),
            0
        );
        assert!(
            app.world().get::<Node>(host).is_some() && app.world().get::<Node>(protected).is_some()
        );
        app.world_mut()
            .entity_mut(host)
            .insert(RouterHostRegistration::new(router));
        settle(&mut app);
        let next = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/").with_fragment("readmitted"))
            .unwrap();
        settle(&mut app);
        assert!(
            matches!(app.world().resource::<RouterSnapshots>().request(router, next), RequestLookup::Found(request) if request.status() == &RequestStatus::Succeeded)
        );
    }
}

#[test]
fn authority_native_cleanup_retains_current_projection_after_capacity_retry() {
    use crate::presentation::native_frame::NativeCleanupPreparationError;
    let (mut app, router, host, _, _) = fixture(false);
    app.world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, RouteLocation::new("/"))
        .unwrap();
    settle(&mut app);
    let root = app.world().resource::<RouterNavigation>().entries[0]
        .active
        .as_ref()
        .unwrap()
        .frames()[0]
        .native_root()
        .unwrap();
    app.world_mut().spawn((Node::default(), ChildOf(root)));
    app.world_mut().spawn((Node::default(), ChildOf(root)));
    app.update();
    app.world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, RouteLocation::new("/unmatched"))
        .unwrap();
    collect_navigation_system(app.world_mut());
    let task_identity = {
        let entry = &mut app.world_mut().resource_mut::<RouterNavigation>().entries[0];
        let EngineAction::FrameTask(task) = entry.engine.action().unwrap() else {
            panic!("actual fallback task");
        };
        let identity = task.report_identity();
        let active = entry.active.take().unwrap();
        retain_frame_task_after_authority_retry(
            entry,
            true,
            active,
            PendingCommit::new(),
            task,
            None,
        );
        identity
    };
    app.world_mut()
        .entity_mut(host)
        .remove::<RouterHostRegistration>();
    crate::cleanup::fail_next_reservation_for_test();
    app.update();
    let claims = {
        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert!(
            entry
                .pending_frame_drive
                .as_ref()
                .unwrap()
                .task
                .report_identity()
                == task_identity
        );
        let active = entry.active.as_ref().unwrap();
        let claims = active.claims().cloned().collect::<Vec<_>>();
        assert_eq!(
            claims.len(),
            2,
            "only original shell/root claims; captured ordinary targets remain with the owner"
        );
        claims
    };
    let added = app.world_mut().spawn((Node::default(), ChildOf(root))).id();
    app.update();
    let entry = &app.world().resource::<RouterNavigation>().entries[0];
    let residual = entry
        .authority_protocol_residual
        .as_ref()
        .expect("bounded original owner receiver");
    let AuthorityProtocolResidualKind::NativeCleanup(NativeCleanupPreparationError::Projection {
        root: observed_root,
        expected,
        observed,
    }) = &residual.kind
    else {
        panic!("current typed error must not collapse to InvalidManifest or old Capacity");
    };
    assert_eq!(*observed_root, root);
    assert!(
        !expected
            .iter()
            .find(|(entity, _, _)| *entity == root)
            .unwrap()
            .2
            .contains(&added)
    );
    assert!(
        observed
            .iter()
            .find(|(entity, _, _)| *entity == root)
            .unwrap()
            .2
            .contains(&added)
    );
    assert_eq!(
        residual
            .active
            .as_ref()
            .unwrap()
            .claims()
            .cloned()
            .collect::<Vec<_>>(),
        claims
    );
    assert!(entry.pending_frame_drive.is_none());
    assert_eq!(
        app.world()
            .resource::<NativeFallbackCalls>()
            .0
            .load(Ordering::Relaxed),
        0
    );
}

#[test]
fn native_current_content_hiding_clears_route_focus_but_escape_returns_to_app() {
    for escaped in [false, true] {
        let (mut app, router, host, _, _) = fixture(false);
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        settle(&mut app);
        let root = app.world().resource::<RouterNavigation>().entries[0]
            .active
            .as_ref()
            .unwrap()
            .frames()[0]
            .native_root()
            .unwrap();
        let target = app.world_mut().spawn((Node::default(), ChildOf(root))).id();
        app.update();
        app.world_mut()
            .resource_mut::<crate::input::RouteFocusState>()
            .set_focused(Some(target));
        app.world_mut()
            .resource_mut::<bevy_input_focus::InputFocus>()
            .set(target, bevy_input_focus::FocusCause::Pressed);
        if escaped {
            app.world_mut().entity_mut(target).insert(ChildOf(host));
        }
        app.world_mut().get_mut::<Node>(target).unwrap().display = bevy_ui::Display::None;
        app.update();
        assert_eq!(
            app.world()
                .resource::<crate::input::RouteFocusState>()
                .focused(),
            escaped.then_some(target)
        );
        assert_eq!(
            app.world().resource::<bevy_input_focus::InputFocus>().get(),
            escaped.then_some(target)
        );
        assert!(
            !app.world()
                .resource::<RouterInputGate>()
                .allows_keyboard(target)
        );
    }
}

fn preload_native_dependency(app: &mut App) -> bevy_asset::Handle<bevy_scene::ScenePatch> {
    let preload = app
        .world()
        .resource::<bevy_asset::AssetServer>()
        .load::<bevy_scene::ScenePatch>("native-test://held.native_dep");
    for _ in 0..1000 {
        if app
            .world()
            .resource::<bevy_asset::AssetServer>()
            .is_loaded(preload.id())
        {
            break;
        }
        app.update();
        std::thread::yield_now();
    }
    assert!(
        app.world()
            .resource::<bevy_asset::AssetServer>()
            .is_loaded(preload.id())
    );
    preload
}

fn assert_retained_failure(
    app: &App,
    router: RouterHandle,
    request: NavigationRequestId,
    host: Entity,
    protected: Entity,
) {
    let result = app
        .world()
        .resource::<RouterSnapshots>()
        .request(router, request);
    assert!(
        matches!(result, RequestLookup::Found(request) if matches!(request.status(), RequestStatus::Failed(failure) if failure.kind() == NavigationFailureKind::Apply && failure.message().contains("native retained apply source"))),
        "{result:?}"
    );
    assert_eq!(
        app.world()
            .resource::<RouterSnapshots>()
            .get(router)
            .unwrap()
            .active()
            .kind(),
        ActivePresentationKind::Error
    );
    assert!(app.world().get_entity(host).is_ok());
    assert_eq!(
        app.world().get::<Node>(protected).unwrap().width,
        Val::Px(91.0)
    );
    let entry = &app.world().resource::<RouterNavigation>().entries[0];
    assert_eq!(entry.native_residuals.len(), 1);
    assert!(
        entry.native_residuals[0]
            .residual
            .validate_inert(app.world())
            .is_ok()
    );
    assert!(entry.pending_failed_branch.is_none());
    assert!(entry.cleanup_ledger.is_none());
    assert!(entry.pending_back_target_execution.is_none());
    assert!(entry.authority_protocol_residual.is_none());
}

#[test]
fn native_retained_root_failure_transfers_owner_then_allows_next_navigation() {
    let (mut app, router, host, protected, failure) = fixture(true);
    let request = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, RouteLocation::new("/").with_fragment("first"))
        .unwrap();
    settle(&mut app);
    assert_retained_failure(&app, router, request, host, protected);
    let failed_root = app.world().get::<ChildOf>(protected).unwrap().parent();
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(failed_root, bevy_input_focus::FocusCause::Navigated);
    app.update();
    assert_eq!(app.world().resource::<InputFocus>().get(), None);
    let app_target = app.world_mut().spawn(Node::default()).id();
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(app_target, bevy_input_focus::FocusCause::Navigated);
    app.update();
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(app_target));
    failure.store(false, Ordering::Relaxed);
    let next = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, RouteLocation::new("/").with_fragment("next"))
        .unwrap();
    settle(&mut app);
    assert!(
        matches!(app.world().resource::<RouterSnapshots>().request(router, next), RequestLookup::Found(request) if request.status() == &RequestStatus::Succeeded)
    );
    assert!(app.world().get_entity(protected).is_ok());
}

#[test]
fn native_bound_error_preserves_cache_in_the_surviving_parent_scope() {
    let (mut app, router, host, protected, failure) =
        fixture_with_retention(false, true, false, 2, true);
    app.world_mut().entity_mut(host).add_child(protected);
    for path in ["/child/1", "/child/2", "/child/3"] {
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new(path))
            .unwrap();
        settle(&mut app);
        assert_eq!(
            app.world().resource::<RouterNavigation>().entries[0]
                .engine
                .request(request)
                .unwrap()
                .status(),
            &RequestStatus::Succeeded
        );
    }
    let (version, top_slot, parent, parent_shell, history, cached, cached_entities, old_leaf) = {
        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        let active = entry.active.as_ref().unwrap();
        let cached = entry
            .retained
            .inventory()
            .unwrap()
            .iter()
            .map(|owner| owner.projection().clone())
            .collect::<Vec<_>>();
        assert_eq!(cached.len(), 2);
        let cached_entities = cached
            .iter()
            .flat_map(|projection| {
                entry
                    .retained
                    .payload(projection.frame())
                    .unwrap()
                    .claims()
                    .map(ExactRegistryClaim::entity)
            })
            .collect::<Vec<_>>();
        let host = entry.host.as_ref().unwrap();
        (
            host.version.clone(),
            host.top_slot,
            active.frames()[0].frame().clone(),
            active.frames()[0].shell(),
            entry.engine.history().clone(),
            cached,
            cached_entities,
            active
                .frames()
                .last()
                .unwrap()
                .claims()
                .map(ExactRegistryClaim::entity)
                .collect::<Vec<_>>(),
        )
    };
    app.insert_resource(CleanFailure);
    failure.store(true, Ordering::Relaxed);
    let request = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, RouteLocation::new("/child/4"))
        .unwrap();
    settle(&mut app);
    {
        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert_eq!(
            entry.engine.snapshot().availability(),
            &RouterAvailability::Ready
        );
        let record = entry.engine.request(request).unwrap();
        let RequestStatus::Failed(original) = record.status() else {
            panic!("the leaf error view must finish the original failed request");
        };
        assert_eq!(original.kind(), NavigationFailureKind::Apply);
        assert!(original.message().contains("native retained apply source"));
        assert_eq!(entry.engine.active().kind(), ActivePresentationKind::Error);
        assert_eq!(entry.engine.active().failure(), Some(original));
        assert_eq!(entry.engine.active().frames().len(), 2);
        assert!(matches!(
            entry.engine.active().frames()[1].origin(),
            FrameOrigin::RouteError { .. }
        ));
        let active = entry.active.as_ref().unwrap();
        assert_eq!(active.frames()[0].frame(), &parent);
        assert_eq!(active.frames()[0].shell(), parent_shell);
        assert_eq!(entry.host.as_ref().unwrap().version, version);
        assert_eq!(entry.host.as_ref().unwrap().top_slot, top_slot);
        assert_eq!(entry.engine.history(), &history);
        assert_eq!(
            entry
                .retained
                .inventory()
                .unwrap()
                .iter()
                .map(|owner| owner.projection())
                .collect::<Vec<_>>(),
            cached.iter().collect::<Vec<_>>()
        );
        assert!(entry.pending_commit.is_none() && entry.pending_failed_branch.is_none());
        assert!(entry.cleanup_ledger.is_none() && entry.authority_protocol_residual.is_none());
        assert!(
            entry.native_residuals.is_empty()
                && entry.engine.snapshot().running_request().is_none()
        );
    }
    let gate = app.world().resource::<RouterInputGate>();
    assert!(
        cached_entities
            .iter()
            .all(|entity| app.world().get_entity(*entity).is_ok()
                && !gate.allows_pointer(*entity)
                && !gate.allows_keyboard(*entity)
                && !gate.allows_gamepad(*entity))
    );
    assert!(
        old_leaf
            .iter()
            .all(|entity| app.world().get_entity(*entity).is_err())
    );
    assert_eq!(
        app.world().get::<ChildOf>(protected).map(ChildOf::parent),
        Some(host)
    );
    failure.store(false, Ordering::Relaxed);
    let next = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, RouteLocation::new("/child/1"))
        .unwrap();
    settle(&mut app);
    assert_eq!(
        app.world().resource::<RouterNavigation>().entries[0]
            .engine
            .request(next)
            .unwrap()
            .status(),
        &RequestStatus::Succeeded
    );
}

#[test]
fn native_invalid_cached_content_is_settled_before_the_same_request_builds_fresh() {
    let (mut app, router, host, protected, _) =
        fixture_with_retention(false, true, false, 2, false);
    app.world_mut().entity_mut(host).add_child(protected);
    for path in ["/child/1", "/child/2"] {
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new(path))
            .unwrap();
        settle(&mut app);
        assert_eq!(
            app.world().resource::<RouterNavigation>().entries[0]
                .engine
                .request(request)
                .unwrap()
                .status(),
            &RequestStatus::Succeeded
        );
    }
    let (version, parent, active_root, cached_shell, cached_root, cached_claims) = {
        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        let inventory = entry.retained.inventory().unwrap();
        assert_eq!(inventory.len(), 1);
        let cached = entry
            .retained
            .payload(inventory[0].projection().frame())
            .unwrap();
        let active = entry.active.as_ref().unwrap();
        (
            entry.host.as_ref().unwrap().version.clone(),
            active.frames()[0].frame().clone(),
            active.frames().last().unwrap().native_root().unwrap(),
            cached.frames()[0].shell(),
            cached.frames()[0].native_root().unwrap(),
            cached
                .claims()
                .map(ExactRegistryClaim::entity)
                .collect::<Vec<_>>(),
        )
    };
    assert!(app.world_mut().despawn(cached_root));
    let request = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, RouteLocation::new("/child/1"))
        .unwrap();
    for _ in 0..20 {
        app.update();
        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert_eq!(
            entry.host.as_ref().map(|host| &host.version),
            Some(&version)
        );
        assert_eq!(
            entry.engine.snapshot().availability(),
            &RouterAvailability::Ready
        );
        if entry.engine.active().location() == Some(&RouteLocation::new("/child/2")) {
            assert!(
                app.world()
                    .resource::<RouterInputGate>()
                    .allows_pointer(active_root)
            );
        }
        if !matches!(
            entry.engine.request(request).unwrap().status(),
            RequestStatus::Running(_)
        ) {
            break;
        }
    }
    let entry = &app.world().resource::<RouterNavigation>().entries[0];
    let record = entry.engine.request(request).unwrap();
    assert_eq!(record.status(), &RequestStatus::Succeeded);
    assert_eq!(
        record
            .trace()
            .iter()
            .filter(|event| event.kind() == NavigationTraceEventKind::FrameTaskIssued)
            .count(),
        1,
        "a cache miss resumes the original task"
    );
    assert_eq!(
        record
            .trace()
            .iter()
            .filter(|event| event.kind() == NavigationTraceEventKind::RequestTerminal)
            .count(),
        1
    );
    let active = entry.active.as_ref().unwrap();
    assert_eq!(active.frames()[0].frame(), &parent);
    assert_ne!(active.frames().last().unwrap().shell(), cached_shell);
    let fresh_root = active.frames().last().unwrap().native_root().unwrap();
    assert_ne!(fresh_root, cached_root);
    assert!(
        app.world()
            .resource::<RouterInputGate>()
            .allows_pointer(fresh_root)
    );
    assert!(entry.cleanup_ledger.is_none() && entry.pending_commit.is_none());
    assert!(entry.pending_failed_branch.is_none() && entry.native_residuals.is_empty());
    assert!(entry.authority_protocol_residual.is_none());
    assert!(cached_claims.iter().all(|entity| {
        app.world().get_entity(*entity).is_err()
            && app
                .world()
                .resource::<RouteEntityRegistry>()
                .record(*entity)
                .is_none()
    }));
    assert_eq!(
        app.world().get::<ChildOf>(protected).map(ChildOf::parent),
        Some(host)
    );
    assert_eq!(
        app.world().get::<Node>(protected).unwrap().width,
        Val::Px(91.0)
    );
}

#[test]
fn native_invalid_cached_content_returns_its_task_when_authority_changes_before_acceptance() {
    let (mut app, router, host, protected, _) =
        fixture_with_retention(false, true, false, 2, false);
    app.world_mut().entity_mut(host).add_child(protected);
    for path in ["/child/1", "/child/2"] {
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new(path))
            .unwrap();
        settle(&mut app);
    }
    let (version, history, cached_root, owned) = {
        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        let inventory = entry.retained.inventory().unwrap();
        let cached = entry
            .retained
            .payload(inventory[0].projection().frame())
            .unwrap();
        (
            entry.host.as_ref().unwrap().version.clone(),
            entry.engine.history().clone(),
            cached.frames()[0].native_root().unwrap(),
            entry
                .active
                .as_ref()
                .unwrap()
                .claims()
                .chain(cached.claims())
                .map(ExactRegistryClaim::entity)
                .collect::<Vec<_>>(),
        )
    };
    assert!(app.world_mut().despawn(cached_root));
    let request = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, RouteLocation::new("/child/1"))
        .unwrap();
    collect_navigation_system(app.world_mut());
    drive_navigation_system(app.world_mut());
    assert!(
        app.world().resource::<RouterNavigation>().entries[0]
            .retained_miss_cleanup
            .is_some()
    );
    app.world_mut()
        .entity_mut(host)
        .remove::<RouterHostRegistration>();
    settle(&mut app);
    let entry = &app.world().resource::<RouterNavigation>().entries[0];
    assert!(matches!(entry.engine.request(request).unwrap().status(),
        RequestStatus::Failed(failure) if failure.kind() == NavigationFailureKind::HostLost));
    assert_eq!(entry.engine.history(), &history);
    assert!(entry.host.is_none() && entry.active.is_none());
    assert!(entry.retained_miss_cleanup.is_none() && entry.retained.is_empty());
    assert!(entry.cleanup_ledger.is_none() && entry.pending_commit.is_none());
    assert!(entry.authority_protocol_residual.is_none());
    assert!(
        owned
            .iter()
            .all(|entity| app.world().get_entity(*entity).is_err())
    );
    assert_eq!(
        app.world().get::<ChildOf>(protected).map(ChildOf::parent),
        Some(host)
    );
    app.world_mut()
        .entity_mut(host)
        .insert(RouterHostRegistration::new(router));
    settle(&mut app);
    assert_ne!(
        app.world().resource::<RouterNavigation>().entries[0]
            .host
            .as_ref()
            .unwrap()
            .version,
        version
    );
    let next = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, RouteLocation::new("/child/1"))
        .unwrap();
    settle(&mut app);
    assert_eq!(
        app.world().resource::<RouterNavigation>().entries[0]
            .engine
            .request(next)
            .unwrap()
            .status(),
        &RequestStatus::Succeeded
    );
}

#[test]
fn native_invalid_cached_content_releases_its_completed_cleanup_before_host_recovery() {
    use bevy_ecs::prelude::{Commands, Despawn, On};
    let (mut app, router, host, protected, _) =
        fixture_with_retention(false, true, false, 2, false);
    app.world_mut().entity_mut(host).add_child(protected);
    for path in ["/child/1", "/child/2"] {
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new(path))
            .unwrap();
        settle(&mut app);
    }
    let (cached_shell, cached_root, history, owned) = {
        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        let inventory = entry.retained.inventory().unwrap();
        let cached = entry
            .retained
            .payload(inventory[0].projection().frame())
            .unwrap();
        (
            cached.frames()[0].shell(),
            cached.frames()[0].native_root().unwrap(),
            entry.engine.history().clone(),
            cached
                .claims()
                .chain(entry.active.as_ref().unwrap().claims())
                .map(ExactRegistryClaim::entity)
                .collect::<Vec<_>>(),
        )
    };
    // The real final shell removal causes host loss after its own receipt is
    // committed. Completing this cache must not claim the old protected proof
    // passed or prevent the normal driver from observing the changed host.
    app.world_mut()
        .add_observer(move |event: On<Despawn, Node>, mut commands: Commands| {
            if event.entity == cached_shell {
                commands.entity(host).remove::<RouterHostRegistration>();
            }
        });
    assert!(app.world_mut().despawn(cached_root));
    let request = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, RouteLocation::new("/child/1"))
        .unwrap();
    settle(&mut app);
    let entry = &app.world().resource::<RouterNavigation>().entries[0];
    assert!(matches!(entry.engine.request(request).unwrap().status(),
        RequestStatus::Failed(failure) if failure.kind() == NavigationFailureKind::HostLost));
    assert_eq!(entry.engine.history(), &history);
    assert!(entry.retained_miss_cleanup.is_none() && entry.retained.is_empty());
    assert!(entry.host.is_none() && entry.active.is_none());
    assert!(entry.cleanup_ledger.is_none() && entry.authority_protocol_residual.is_none());
    assert!(owned.iter().all(|entity| {
        app.world().get_entity(*entity).is_err()
            && app
                .world()
                .resource::<RouteEntityRegistry>()
                .record(*entity)
                .is_none()
    }));
    assert_eq!(
        app.world().get::<ChildOf>(protected).map(ChildOf::parent),
        Some(host)
    );
    app.world_mut()
        .entity_mut(host)
        .insert(RouterHostRegistration::new(router));
    settle(&mut app);
    let next = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, RouteLocation::new("/child/1"))
        .unwrap();
    settle(&mut app);
    assert_eq!(
        app.world().resource::<RouterNavigation>().entries[0]
            .engine
            .request(next)
            .unwrap()
            .status(),
        &RequestStatus::Succeeded
    );
}

#[test]
fn native_leaf_cache_settles_after_failure_or_host_loss() {
    for host_loss in [false, true] {
        let (mut app, router, host, protected, failure) =
            fixture_with_retention(false, true, false, 2, false);
        app.world_mut().entity_mut(host).add_child(protected);
        for path in ["/child/1", "/child/2", "/child/3"] {
            let request = app
                .world_mut()
                .resource_mut::<RouterNavigation>()
                .push(router, RouteLocation::new(path))
                .unwrap();
            settle(&mut app);
            assert_eq!(
                app.world().resource::<RouterNavigation>().entries[0]
                    .engine
                    .request(request)
                    .unwrap()
                    .status(),
                &RequestStatus::Succeeded
            );
        }
        let (version, history, active, cached, mut owned) = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            let binding = entry.host.as_ref().unwrap();
            let cached = entry
                .retained
                .inventory()
                .unwrap()
                .iter()
                .map(|owner| owner.projection().clone())
                .collect::<Vec<_>>();
            assert_eq!(cached.len(), 2);
            let mut owned = entry
                .active
                .as_ref()
                .unwrap()
                .claims()
                .map(ExactRegistryClaim::entity)
                .collect::<Vec<_>>();
            for projection in &cached {
                owned.extend(
                    entry
                        .retained
                        .payload(projection.frame())
                        .unwrap()
                        .claims()
                        .map(ExactRegistryClaim::entity),
                );
            }
            owned.push(binding.top_slot);
            (
                binding.version.clone(),
                entry.engine.history().clone(),
                entry.engine.active().clone(),
                cached,
                owned,
            )
        };
        app.insert_resource(CleanFailure);
        failure.store(true, Ordering::Relaxed);
        let original_owned = owned.len();
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/broken/4"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        for _ in 0..8 {
            drive_navigation_system(app.world_mut());
            if app.world().resource::<RouterNavigation>().entries[0]
                .pending_failed_branch
                .is_some()
            {
                break;
            }
        }
        {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            let pending = entry.pending_failed_branch.as_ref().unwrap();
            assert!(
                matches!(pending.terminal, PendingFailedBranchTerminal::Request(_)),
                "the real successful prefix produces its original FailureCleanupRequest"
            );
            assert!(pending.branch.native_failure().is_some());
            assert!(!pending.branch.frames().is_empty());
            owned.extend(
                pending
                    .branch
                    .frames()
                    .iter()
                    .flat_map(|frame| frame.claims())
                    .map(ExactRegistryClaim::entity),
            );
            owned.extend(
                pending
                    .branch
                    .child_slots()
                    .iter()
                    .map(|slot| slot.slot_claim().entity()),
            );
        }
        if !host_loss {
            // Stop at original cleanup completion, before Drive builds the fallback.
            // Ordinary failure must preserve the current/cache throughout this batch.
            for _ in 0..8 {
                settle_cleanup_system(app.world_mut());
                let entry = &app.world().resource::<RouterNavigation>().entries[0];
                if entry.pending_failed_branch.is_none() && entry.cleanup_ledger.is_none() {
                    break;
                }
            }
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            assert_eq!(
                entry.engine.snapshot().availability(),
                &RouterAvailability::Ready
            );
            assert_eq!(entry.engine.active(), &active);
            assert_eq!(entry.engine.history(), &history);
            assert_eq!(
                entry
                    .retained
                    .inventory()
                    .unwrap()
                    .iter()
                    .map(|owner| owner.projection())
                    .collect::<Vec<_>>(),
                cached.iter().collect::<Vec<_>>()
            );
            assert!(entry.pending_failed_branch.is_none() && entry.cleanup_ledger.is_none());
            assert!(entry.authority_protocol_residual.is_none());
            assert!(matches!(
                entry.engine.request(request).unwrap().status(),
                RequestStatus::Running(_)
            ));
            assert_eq!(
                app.world()
                    .resource::<NativeFallbackCalls>()
                    .0
                    .load(Ordering::Relaxed),
                0
            );
            assert!(
                owned[..original_owned]
                    .iter()
                    .all(|entity| app.world().get_entity(*entity).is_ok())
            );
            assert!(
                owned[original_owned..]
                    .iter()
                    .all(|entity| app.world().get_entity(*entity).is_err())
            );
            let top_slot = entry.host.as_ref().unwrap().top_slot;
            settle(&mut app);
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            assert_eq!(
                entry.engine.snapshot().availability(),
                &RouterAvailability::Ready,
                "fallback must replace the old parent scope without losing host authority"
            );
            assert!(matches!(entry.engine.request(request).unwrap().status(),
                RequestStatus::Failed(error) if error.kind() == NavigationFailureKind::Apply
                    && error.message().contains("native retained apply source")));
            assert_eq!(entry.engine.active().kind(), ActivePresentationKind::Error);
            assert_eq!(
                entry.engine.active().frames()[0].origin(),
                &FrameOrigin::RouterFallback
            );
            assert_eq!(
                entry.engine.active().requested(),
                Some(&RouteLocation::new("/broken/4"))
            );
            assert_eq!(entry.engine.history(), &history);
            assert!(entry.retained.is_empty() && entry.cleanup_ledger.is_none());
            assert!(entry.pending_commit.is_none() && entry.pending_failed_branch.is_none());
            assert!(
                entry.authority_protocol_residual.is_none() && entry.native_residuals.is_empty()
            );
            assert!(entry.engine.snapshot().running_request().is_none());
            assert_eq!(entry.host.as_ref().unwrap().version, version);
            assert_eq!(
                app.world()
                    .resource::<NativeFallbackCalls>()
                    .0
                    .load(Ordering::Relaxed),
                1
            );
            assert!(
                owned
                    .iter()
                    .filter(|entity| **entity != top_slot)
                    .all(|entity| app.world().get_entity(*entity).is_err())
            );
            assert!(app.world().get_entity(host).is_ok());
            assert_eq!(
                app.world().get::<ChildOf>(protected).map(ChildOf::parent),
                Some(host)
            );
            failure.store(false, Ordering::Relaxed);
            let next = app
                .world_mut()
                .resource_mut::<RouterNavigation>()
                .push(router, RouteLocation::new("/child/1"))
                .unwrap();
            settle(&mut app);
            assert_eq!(
                app.world().resource::<RouterNavigation>().entries[0]
                    .engine
                    .request(next)
                    .unwrap()
                    .status(),
                &RequestStatus::Succeeded
            );
            continue;
        }
        app.world_mut()
            .entity_mut(host)
            .remove::<RouterHostRegistration>();
        settle_cleanup_system(app.world_mut());
        {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            assert_eq!(
                entry.engine.snapshot().availability(),
                &RouterAvailability::RecoveringAuthority
            );
            assert!(
                entry
                    .cleanup_ledger
                    .as_ref()
                    .is_some_and(CleanupLedger::is_authority_settlement)
            );
            assert!(entry.authority_protocol_residual.is_none());
        }
        settle(&mut app);
        {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            assert!(matches!(entry.engine.request(request).unwrap().status(),
            RequestStatus::Failed(error) if error.kind() == NavigationFailureKind::HostLost));
            assert_eq!(entry.engine.history(), &history);
            assert!(entry.retained.is_empty());
            assert!(entry.active.is_none() && entry.host.is_none());
            assert!(entry.pending_failed_branch.is_none() && entry.cleanup_ledger.is_none());
            assert!(
                entry.authority_protocol_residual.is_none() && entry.native_residuals.is_empty()
            );
            assert!(entry.engine.snapshot().running_request().is_none());
            assert_eq!(
                entry.engine.snapshot().availability(),
                &RouterAvailability::Unavailable(RouterUnavailableReason::HostLost)
            );
        }
        assert!(
            owned
                .iter()
                .all(|entity| app.world().get_entity(*entity).is_err())
        );
        assert!(app.world().get_entity(host).is_ok());
        assert_eq!(
            app.world().get::<Node>(protected).unwrap().width,
            Val::Px(91.0)
        );
        assert_eq!(
            app.world().get::<ChildOf>(protected).map(ChildOf::parent),
            Some(host)
        );
        app.world_mut()
            .entity_mut(host)
            .insert(RouterHostRegistration::new(router));
        settle(&mut app);
        {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            assert_ne!(entry.host.as_ref().unwrap().version, version);
            assert_eq!(entry.engine.history(), &history);
            assert!(entry.active.is_none() && entry.engine.snapshot().running_request().is_none());
        }
        failure.store(false, Ordering::Relaxed);
        let next = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/child/1"))
            .unwrap();
        settle(&mut app);
        assert_eq!(
            app.world().resource::<RouterNavigation>().entries[0]
                .engine
                .request(next)
                .unwrap()
                .status(),
            &RequestStatus::Succeeded
        );
        assert_eq!(
            app.world()
                .resource::<NativeFallbackCalls>()
                .0
                .load(Ordering::Relaxed),
            0
        );
    }
}

#[test]
fn native_retained_back_failure_consumes_history_pair_via_failure_cleanup() {
    let (mut app, router, host, protected, failure) = fixture(false);
    for fragment in ["a", "b"] {
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/").with_fragment(fragment))
            .unwrap();
        settle(&mut app);
    }
    failure.store(true, Ordering::Relaxed);
    let request = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .back(router)
        .unwrap();
    settle(&mut app);
    assert_retained_failure(&app, router, request, host, protected);
}

#[test]
fn native_retained_nested_failure_settles_real_prefix_and_terminal_residual() {
    let (mut app, router, host, protected, _) = fixture_variant(true, true);
    let request = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, RouteLocation::new("/child"))
        .unwrap();
    settle(&mut app);
    assert_retained_failure(&app, router, request, host, protected);
    let nodes = app
        .world_mut()
        .query::<&Node>()
        .iter(app.world())
        .filter(|node| node.width == Val::Px(71.0))
        .count();
    assert_eq!(
        nodes, 1,
        "only the inert native residual keeps its authored root"
    );
}

#[test]
fn native_retained_retry_keeps_original_source_then_current_drift_fail_closes() {
    let (mut app, router, host, protected, _) = fixture(true);
    app.world_mut()
        .insert_resource(NativeFailureTransferCapacity);
    let request = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, RouteLocation::new("/"))
        .unwrap();
    settle(&mut app);
    let (source, claims) = {
        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        let failure = entry
            .pending_failed_branch
            .as_ref()
            .unwrap()
            .branch
            .native_failure()
            .unwrap();
        (
            failure.source.clone(),
            failure.claims().cloned().collect::<Vec<_>>(),
        )
    };
    settle(&mut app);
    {
        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        let failure = entry
            .pending_failed_branch
            .as_ref()
            .unwrap()
            .branch
            .native_failure()
            .unwrap();
        assert!(failure.source == source);
        assert_eq!(failure.claims().cloned().collect::<Vec<_>>(), claims);
        assert!(entry.native_residuals.is_empty());
    }
    app.world_mut()
        .remove_resource::<NativeFailureTransferCapacity>();
    let root = claims
        .iter()
        .find(|claim| {
            matches!(
                claim.owner(),
                RegistryOwner::Committed(EntityOwner::FrameContentTarget(_))
            )
        })
        .unwrap()
        .entity();
    app.world_mut()
        .entity_mut(root)
        .remove::<RouteOwnershipSeal>();
    settle(&mut app);
    let entry = &app.world().resource::<RouterNavigation>().entries[0];
    let owner = entry.authority_protocol_residual.as_ref().unwrap();
    let failure = owner
        .failed
        .as_ref()
        .unwrap()
        .branch
        .native_failure()
        .unwrap();
    assert!(failure.source == source);
    assert!(matches!(
        owner.kind,
        AuthorityProtocolResidualKind::NativeCreation(_)
    ));
    assert!(entry.pending_failed_branch.is_none());
    assert!(entry.native_residuals.is_empty());
    assert!(app.world().get_entity(host).is_ok());
    assert!(app.world().get_entity(protected).is_ok());
    assert!(
        matches!(app.world().resource::<RouterSnapshots>().request(router, request), RequestLookup::Found(request) if matches!(request.status(), RequestStatus::Failed(_)))
    );
}

#[test]
fn native_retained_unisolated_back_fail_close_keeps_original_pair_and_error() {
    let (mut app, router, host, protected, failure) = fixture(false);
    for fragment in ["a", "b"] {
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/").with_fragment(fragment))
            .unwrap();
        settle(&mut app);
    }
    failure.store(true, Ordering::Relaxed);
    app.world_mut().insert_resource(MakeFailureUnisolated);
    app.world_mut()
        .resource_mut::<RouterNavigation>()
        .back(router)
        .unwrap();
    settle(&mut app);
    let entry = &app.world().resource::<RouterNavigation>().entries[0];
    let owner = entry.authority_protocol_residual.as_ref().unwrap();
    assert!(matches!(
        owner.kind,
        AuthorityProtocolResidualKind::NativeCreation(
            crate::presentation::native_scene::NativeSceneIsolationFailure::ShellNotInert
        )
    ));
    assert!(matches!(
        owner.back_target.as_deref(),
        Some(PendingBackTargetExecution::Driving(_))
    ));
    assert!(
        owner
            .failed
            .as_ref()
            .unwrap()
            .branch
            .native_failure()
            .unwrap()
            .error
            .to_string()
            .contains("native retained apply source")
    );
    assert!(entry.pending_back_target_execution.is_none());
    assert!(entry.pending_failed_branch.is_none());
    assert!(entry.native_residuals.is_empty());
    assert!(app.world().get_entity(host).is_ok());
    assert!(app.world().get_entity(protected).is_ok());
}

#[test]
fn native_retained_dependencies_live_until_real_failure_settlement_not_residual_destruction() {
    let (mut app, router, host, protected, _) = fixture_with_dependency(true, false, true);
    let preload = preload_native_dependency(&mut app);
    let bevy_asset::Handle::Strong(strong) = &preload else {
        panic!("loaded dependency is strong")
    };
    let lifetime = Arc::downgrade(strong);
    app.world_mut()
        .insert_resource(NativeFailureTransferCapacity);
    let request = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, RouteLocation::new("/"))
        .unwrap();
    settle(&mut app);
    drop(preload);
    settle(&mut app);
    assert!(
        lifetime.upgrade().is_some(),
        "the unfinished original failure owns its real Scene dependency"
    );
    app.world_mut()
        .remove_resource::<NativeFailureTransferCapacity>();
    settle(&mut app);
    assert_retained_failure(&app, router, request, host, protected);
    assert!(
        lifetime.upgrade().is_none(),
        "terminal physical retention does not extend the data lease"
    );
}

#[test]
fn native_clean_back_failure_keeps_zero_physical_history_path() {
    let (mut app, router, host, protected, failure) = fixture(false);
    for fragment in ["a", "b"] {
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/").with_fragment(fragment))
            .unwrap();
        settle(&mut app);
    }
    failure.store(true, Ordering::Relaxed);
    app.world_mut().insert_resource(CleanFailure);
    let request = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .back(router)
        .unwrap();
    settle(&mut app);
    let result = app
        .world()
        .resource::<RouterSnapshots>()
        .request(router, request);
    assert!(
        matches!(result, RequestLookup::Found(request) if matches!(request.status(), RequestStatus::Failed(failure) if failure.kind() == NavigationFailureKind::Apply)),
        "{result:?}"
    );
    let entry = &app.world().resource::<RouterNavigation>().entries[0];
    assert!(entry.native_residuals.is_empty());
    assert!(entry.pending_back_target_execution.is_none());
    assert!(entry.pending_failed_branch.is_none());
    assert!(entry.authority_protocol_residual.is_none());
    assert!(app.world().get_entity(host).is_ok());
    assert!(app.world().get::<ChildOf>(protected).is_none());
}

#[test]
fn native_success_dependencies_move_into_frame_and_release_after_cleanup() {
    let (mut app, router, _, _, _) = fixture_with_dependency(false, false, true);
    let preload = preload_native_dependency(&mut app);
    let bevy_asset::Handle::Strong(strong) = &preload else {
        panic!("loaded dependency is strong")
    };
    let lifetime = Arc::downgrade(strong);
    let request = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, RouteLocation::new("/"))
        .unwrap();
    settle(&mut app);
    assert!(
        matches!(app.world().resource::<RouterSnapshots>().request(router, request), RequestLookup::Found(request) if request.status() == &RequestStatus::Succeeded)
    );
    drop(preload);
    settle(&mut app);
    assert!(
        lifetime.upgrade().is_some(),
        "the adopted frame received the original dependency handles"
    );
    app.world_mut()
        .resource_mut::<RouterNavigation>()
        .replace(router, RouteLocation::new("/missing"))
        .unwrap();
    settle(&mut app);
    assert!(
        lifetime.upgrade().is_none(),
        "the finished old frame cleanup releases its data holder"
    );
}

#[test]
fn native_idle_host_loss_despawn_escape_is_bounded_and_returns_app_focus() {
    use bevy_ecs::prelude::{Commands, Despawn, On};
    #[derive(Component)]
    struct EscapeTrigger;
    #[derive(Resource, Default)]
    struct EscapeCalls(usize);
    let (mut app, router, host, _, _) = fixture(false);
    app.world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, RouteLocation::new("/"))
        .unwrap();
    settle(&mut app);
    let root = app.world().resource::<RouterNavigation>().entries[0]
        .active
        .as_ref()
        .unwrap()
        .frames()[0]
        .native_root()
        .unwrap();
    let child = app.world_mut().spawn((Node::default(), ChildOf(root))).id();
    // Cleanup removes the later child first, so its real despawn callback can
    // move the still-live first child out of the original content boundary.
    let trigger = app
        .world_mut()
        .spawn((Node::default(), ChildOf(root), EscapeTrigger))
        .id();
    let application = app.world_mut().spawn(Node::default()).id();
    app.init_resource::<EscapeCalls>();
    app.world_mut().add_observer(
        move |event: On<Despawn, EscapeTrigger>, mut commands: Commands| {
            if event.entity == trigger {
                commands.queue(move |world: &mut World| {
                    assert!(world.get_entity(child).is_ok());
                    world.resource_mut::<EscapeCalls>().0 += 1;
                    world.entity_mut(child).insert(ChildOf(application));
                    world
                        .resource_mut::<InputFocus>()
                        .set(child, bevy_input_focus::FocusCause::Navigated);
                });
            }
        },
    );
    app.world_mut()
        .entity_mut(host)
        .remove::<RouterHostRegistration>();
    settle(&mut app);
    let entry = &app.world().resource::<RouterNavigation>().entries[0];
    assert!(entry.idle_host_loss.is_none());
    assert!(
        entry.host.is_none(),
        "permanent native preflight cannot leave the lost host owner retrying forever"
    );
    assert!(entry.active.is_none());
    assert!(entry.idle_host_terminal.is_some());
    assert_eq!(app.world().resource::<EscapeCalls>().0, 1);
    assert!(app.world().get_entity(trigger).is_err());
    assert!(
        !app.world()
            .resource::<RouterInputGate>()
            .allows_keyboard(child)
    );
    assert_eq!(
        app.world().get::<ChildOf>(child).map(ChildOf::parent),
        Some(application)
    );
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(child));
}

#[test]
fn native_cleanup_allows_unrelated_app_sibling_presentation_update() {
    use bevy_ecs::prelude::{Commands, Despawn, On};

    let (mut app, router, host, sibling, _) = fixture(false);
    app.world_mut().entity_mut(host).add_child(sibling);
    app.world_mut()
        .entity_mut(sibling)
        .insert(Visibility::Visible);
    let first = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, RouteLocation::new("/"))
        .unwrap();
    settle(&mut app);
    assert_eq!(
        app.world().resource::<RouterNavigation>().entries[0]
            .engine
            .request(first)
            .unwrap()
            .status(),
        &RequestStatus::Succeeded
    );
    let root = app.world().resource::<RouterNavigation>().entries[0]
        .active
        .as_ref()
        .unwrap()
        .frames()[0]
        .native_root()
        .unwrap();
    let child = app.world_mut().spawn((Node::default(), ChildOf(root))).id();
    app.update();
    let updated = Arc::new(AtomicBool::new(false));
    let observed_update = updated.clone();
    // A normal application callback runs after the cleanup plan has frozen its
    // protected facts. This sibling is outside the routed content and target set.
    app.world_mut()
        .add_observer(move |event: On<Despawn, Node>, mut commands: Commands| {
            if event.entity == child {
                let updated = updated.clone();
                commands.queue(move |world: &mut World| {
                    world.get_mut::<Node>(sibling).unwrap().display = bevy_ui::Display::None;
                    world.entity_mut(sibling).insert(Visibility::Hidden);
                    updated.store(true, Ordering::Relaxed);
                });
            }
        });
    let replacement = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .replace(router, RouteLocation::new("/").with_fragment("app-update"))
        .unwrap();
    settle(&mut app);

    assert!(observed_update.load(Ordering::Relaxed));
    assert_eq!(
        app.world().get::<ChildOf>(sibling).map(ChildOf::parent),
        Some(host)
    );
    assert_eq!(
        app.world().get::<Node>(sibling).unwrap().display,
        bevy_ui::Display::None
    );
    assert_eq!(
        app.world().get::<Visibility>(sibling),
        Some(&Visibility::Hidden)
    );
    assert!(
        app.world()
            .resource::<RouteEntityRegistry>()
            .record(sibling)
            .is_none()
    );
    let entry = &app.world().resource::<RouterNavigation>().entries[0];
    let record = entry.engine.request(replacement).unwrap();
    assert_eq!(
        record.status(),
        &RequestStatus::Succeeded,
        "unrelated app presentation must not stop cleanup: {:?}",
        entry
            .cleanup_ledger
            .as_ref()
            .and_then(CleanupLedger::cleanup_settlement_permanent_evidence)
    );
    assert!(entry.cleanup_ledger.is_none());
    assert!(
        entry.authority_protocol_residual.is_none(),
        "authority residual: CleanupStopped={}, retained cleanup source={:?}",
        entry
            .authority_protocol_residual
            .as_ref()
            .is_some_and(|residual| {
                matches!(
                    &residual.kind,
                    AuthorityProtocolResidualKind::CleanupStopped
                )
            }),
        entry
            .authority_protocol_residual
            .as_ref()
            .and_then(AuthorityProtocolResidualOwner::retained_cleanup_settlement_ledger)
            .and_then(CleanupLedger::cleanup_settlement_permanent_evidence)
    );
    assert!(app.world().get_entity(child).is_err());
    assert!(app.world().get_entity(root).is_err());
}

#[test]
fn native_cleanup_despawn_registered_ancestor_stops_remaining_content_removal() {
    use crate::presentation::FrameLifecycle;
    use crate::registry::ComponentManifest;
    use bevy_ecs::prelude::{Commands, Despawn, On};

    #[derive(Component)]
    struct RegisterAncestorOnDespawn;
    #[derive(Resource, Default)]
    struct RegistrationCalls(usize);

    let (mut app, router, host, _, _) = fixture(false);
    app.world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, RouteLocation::new("/"))
        .unwrap();
    settle(&mut app);
    let frame = &app.world().resource::<RouterNavigation>().entries[0]
        .active
        .as_ref()
        .unwrap()
        .frames()[0];
    let root = frame.native_root().unwrap();
    let frame_identity = frame.frame().clone();
    let parent = app.world_mut().spawn((Node::default(), ChildOf(root))).id();
    let child = app
        .world_mut()
        .spawn((Node::default(), ChildOf(parent)))
        .id();
    // The deepest target is genuinely removed first. Its callback establishes
    // a registry boundary while the remaining child's relationships stay intact.
    let trigger = app
        .world_mut()
        .spawn((Node::default(), ChildOf(child), RegisterAncestorOnDespawn))
        .id();
    app.init_resource::<RegistrationCalls>();
    app.world_mut().add_observer(
        move |event: On<Despawn, RegisterAncestorOnDespawn>, mut commands: Commands| {
            if event.entity != trigger {
                return;
            }
            let frame_identity = frame_identity.clone();
            commands.queue(move |world: &mut World| {
                assert!(world.get_entity(child).is_ok());
                assert_eq!(
                    world.get::<ChildOf>(child).map(ChildOf::parent),
                    Some(parent)
                );
                let manifest = ComponentManifest::from_world(world, parent).unwrap();
                let claim = world
                    .resource_mut::<RouteEntityRegistry>()
                    .register(
                        parent,
                        RegistryOwner::Committed(EntityOwner::FrameOutput {
                            frame: frame_identity,
                            output: u64::MAX,
                            local_index: 0,
                        }),
                        manifest,
                        EntityLifecycle::Active,
                    )
                    .unwrap();
                world.entity_mut(parent).insert((
                    RouteOwnershipSeal::new(claim.seal()),
                    FrameLifecycle(claim.lifecycle()),
                ));
                world.resource_mut::<RegistrationCalls>().0 += 1;
            });
        },
    );
    let replacement = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .replace(
            router,
            RouteLocation::new("/").with_fragment("registered-ancestor"),
        )
        .unwrap();
    for _ in 0..32 {
        app.update();
    }
    assert_eq!(app.world().resource::<RegistrationCalls>().0, 1);
    assert!(app.world().get_entity(trigger).is_err());
    assert!(
        app.world().get_entity(child).is_ok(),
        "cleanup must not traverse the newly registered ancestor boundary"
    );
    assert_eq!(
        app.world().get::<ChildOf>(child).map(ChildOf::parent),
        Some(parent)
    );
    assert_eq!(
        app.world().get::<ChildOf>(parent).map(ChildOf::parent),
        Some(root)
    );
    assert!(
        app.world()
            .resource::<RouteEntityRegistry>()
            .record(parent)
            .is_some()
    );
    assert!(app.world().get_entity(host).is_ok());
    let snapshots = app.world().resource::<RouterSnapshots>();
    assert!(
        matches!(snapshots.request(router, replacement), RequestLookup::Found(request) if request.status() == &RequestStatus::Succeeded)
    );
    let snapshot = snapshots.get(router).unwrap();
    assert!(snapshot.running_request().is_none());
    assert!(!snapshot.cleanup().foreground_pending());
}
