use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::AssetPlugin;
use bevy_scene::{ScenePlugin, WorldSceneExt, bsn};
use bevy_ui_router::prelude::DefaultOutlet;

use bevy_ecs::{
    component::Component,
    hierarchy::{ChildOf, Children},
};
use bevy_ui::{Node, Val};
use bevy_ui_router::prelude::*;

#[derive(Component, Clone, Default)]
struct NativePage;

#[derive(Component, Clone, Default)]
struct NativeLayout;

#[test]
fn facade_native_used_driver_requires_scene_runtime_and_keeps_binding_error_order() {
    let mut app = App::new();
    let driver = app
        .register_bsn_view(ViewDriverKey::new("native").unwrap(), |_| Ok(bsn! { Node }))
        .unwrap();
    let view = RouteViewKey::new("native").unwrap();
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
        .insert(view.clone(), RouteViewDescriptor::new(driver.clone()))
        .unwrap();
    let absent_fallback = RouteViewKey::new("absent").unwrap();
    let mut missing_views = RouteViewRegistry::new();
    missing_views
        .insert(view.clone(), RouteViewDescriptor::new(driver))
        .unwrap();
    assert_eq!(
        app.add_ui_router(RouterConfig::new(
            routes.clone(),
            missing_views,
            absent_fallback
        ))
        .unwrap_err()
        .kind(),
        RouterInstallErrorKind::MissingFallbackView
    );
    assert_eq!(
        app.add_ui_router(RouterConfig::new(routes, views, view))
            .unwrap_err()
            .kind(),
        RouterInstallErrorKind::MissingSceneRuntime
    );
}

#[test]
fn facade_native_missing_duplicate_and_non_node_outlets_fail_as_mounts() {
    for invalid in ["missing", "duplicate", "not-node"] {
        let mut app = App::new();
        app.add_plugins((
            TaskPoolPlugin::default(),
            AssetPlugin::default(),
            ScenePlugin,
        ));
        let layout_driver = app.register_bsn_view(ViewDriverKey::new("layout").unwrap(), move |_| {
            Ok(bevy_scene::SceneFunction(move |_, root: &mut bevy_scene::ResolvedScene| {
                root.push_template(Node { width: Val::Px(311.0), ..Node::default() });
                let count = match invalid { "missing" => 0, "duplicate" => 2, _ => 1 };
                for _ in 0..count {
                    let mut outlet = bevy_scene::ResolvedScene::default();
                    if invalid != "not-node" { outlet.push_template(Node::default()); }
                    outlet.push_template(<DefaultOutlet as bevy_ecs::template::FromTemplate>::Template::default());
                    root.get_or_insert_related_resolved_scenes::<ChildOf>().scenes.push(outlet);
                }
            }))
        }).unwrap();
        let leaf_driver = app
            .register_bsn_view(ViewDriverKey::new("leaf").unwrap(), |_| {
                Ok(bsn! { Node NativePage })
            })
            .unwrap();
        let layout = RouteViewKey::new("layout").unwrap();
        let leaf = RouteViewKey::new("leaf").unwrap();
        let mut routes = RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/").unwrap(),
                RouteNodeOptions::new().with_view(layout.clone()),
            )
            .unwrap();
        routes
            .route_with_options(
                routes.root_id().unwrap(),
                RoutePattern::path("child").unwrap(),
                RouteNodeOptions::new().with_view(leaf.clone()),
            )
            .unwrap();
        let mut views = RouteViewRegistry::new();
        views
            .insert(layout, RouteViewDescriptor::new(layout_driver))
            .unwrap();
        views
            .insert(leaf.clone(), RouteViewDescriptor::new(leaf_driver))
            .unwrap();
        let router = app
            .add_ui_router(RouterConfig::new(routes.build().unwrap(), views, leaf))
            .unwrap();
        let host = app
            .world_mut()
            .spawn((
                Node {
                    width: Val::Px(487.0),
                    ..Node::default()
                },
                RouterHostRegistration::new(router),
            ))
            .id();
        app.update();
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/child"))
            .unwrap();
        for _ in 0..20 {
            app.update();
        }
        let result = app
            .world()
            .resource::<RouterSnapshots>()
            .request(router, request);
        assert!(
            matches!(result, RequestLookup::Found(request) if matches!(request.status(), RequestStatus::Failed(failure) if failure.kind() == NavigationFailureKind::Mount)),
            "invalid={invalid}: {result:?}"
        );
        assert_eq!(app.world().get::<Node>(host).unwrap().width, Val::Px(487.0));
        assert_eq!(
            app.world()
                .resource::<RouterSnapshots>()
                .get(router)
                .unwrap()
                .active()
                .kind(),
            ActivePresentationKind::Error
        );
    }
}

#[test]
fn facade_native_root_and_internal_outlets_preserve_live_siblings() {
    for internal in [false, true] {
        let mut app = App::new();
        app.add_plugins((
            TaskPoolPlugin::default(),
            AssetPlugin::default(),
            ScenePlugin,
        ));
        let layout_driver = app.register_bsn_view(ViewDriverKey::new("layout").unwrap(), move |_| {
            Ok(bevy_scene::SceneFunction(move |_, root: &mut bevy_scene::ResolvedScene| {
                root.push_template(Node { width: Val::Px(311.0), ..Node::default() });
                root.push_template(NativeLayout);
                if internal {
                    let mut intermediate = bevy_scene::ResolvedScene::default();
                    intermediate.push_template(Node::default());
                    let mut anchor = bevy_scene::ResolvedScene::default();
                    anchor.push_template(Node { width: Val::Px(199.0), ..Node::default() });
                    anchor.push_template(<DefaultOutlet as bevy_ecs::template::FromTemplate>::Template::default());
                    intermediate.get_or_insert_related_resolved_scenes::<ChildOf>().scenes.push(anchor);
                    root.get_or_insert_related_resolved_scenes::<ChildOf>().scenes.push(intermediate);
                } else { root.push_template(<DefaultOutlet as bevy_ecs::template::FromTemplate>::Template::default()); }
            }))
        }).unwrap();
        let leaf_driver = app
            .register_bsn_view(ViewDriverKey::new("leaf").unwrap(), |_| {
                Ok(bsn! { Node { width: Val::Px(47.0) } NativePage })
            })
            .unwrap();
        let layout = RouteViewKey::new("layout").unwrap();
        let leaf = RouteViewKey::new("leaf").unwrap();
        let mut routes = RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/").unwrap(),
                RouteNodeOptions::new().with_view(layout.clone()),
            )
            .unwrap();
        routes
            .route_with_options(
                routes.root_id().unwrap(),
                RoutePattern::path("items/:id").unwrap(),
                RouteNodeOptions::new().with_view(leaf.clone()),
            )
            .unwrap();
        let mut views = RouteViewRegistry::new();
        views
            .insert(layout, RouteViewDescriptor::new(layout_driver))
            .unwrap();
        views
            .insert(leaf.clone(), RouteViewDescriptor::new(leaf_driver))
            .unwrap();
        let router = app
            .add_ui_router(RouterConfig::new(routes.build().unwrap(), views, leaf))
            .unwrap();
        let host = app
            .world_mut()
            .spawn((Node::default(), RouterHostRegistration::new(router)))
            .id();
        app.update();
        let first = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/items/a"))
            .unwrap();
        for _ in 0..20 {
            app.update();
        }
        let result = app
            .world()
            .resource::<RouterSnapshots>()
            .request(router, first);
        assert!(
            matches!(result, RequestLookup::Found(request) if request.status() == &RequestStatus::Succeeded),
            "internal={internal}, {result:?}"
        );
        let layout = app
            .world_mut()
            .query_filtered::<bevy_ecs::entity::Entity, bevy_ecs::query::With<NativeLayout>>()
            .single(app.world())
            .unwrap();
        let anchor = app
            .world_mut()
            .query_filtered::<bevy_ecs::entity::Entity, bevy_ecs::query::With<DefaultOutlet>>()
            .single(app.world())
            .unwrap();
        let old_leaf = app
            .world_mut()
            .query_filtered::<bevy_ecs::entity::Entity, bevy_ecs::query::With<NativePage>>()
            .single(app.world())
            .unwrap();
        let sibling = app
            .world_mut()
            .spawn((
                Node {
                    width: Val::Px(83.0),
                    ..Node::default()
                },
                ChildOf(anchor),
            ))
            .id();
        app.update();
        assert!(
            app.world()
                .resource::<RouterInputGate>()
                .allows_keyboard(sibling)
        );
        let second = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .replace(router, RouteLocation::new("/items/b"))
            .unwrap();
        for _ in 0..20 {
            app.update();
        }
        let result = app
            .world()
            .resource::<RouterSnapshots>()
            .request(router, second);
        assert!(
            matches!(result, RequestLookup::Found(request) if request.status() == &RequestStatus::Succeeded),
            "internal={internal}, {result:?}"
        );
        assert!(app.world().get_entity(old_leaf).is_err());
        assert_eq!(
            app.world().get::<ChildOf>(sibling).map(ChildOf::parent),
            Some(anchor)
        );
        assert_eq!(
            app.world().get::<Node>(layout).unwrap().width,
            Val::Px(311.0)
        );
        assert!(app.world().get_entity(host).is_ok());
        let parent_only = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .replace(router, RouteLocation::new("/"))
            .unwrap();
        for _ in 0..20 {
            app.update();
        }
        let result = app
            .world()
            .resource::<RouterSnapshots>()
            .request(router, parent_only);
        assert!(
            matches!(result, RequestLookup::Found(request) if request.status() == &RequestStatus::Succeeded),
            "remove slot internal={internal}, {result:?}"
        );
        assert_eq!(
            app.world().get::<ChildOf>(sibling).map(ChildOf::parent),
            Some(anchor)
        );
        assert_eq!(
            app.world()
                .get::<Children>(anchor)
                .unwrap()
                .iter()
                .filter(|entity| **entity == sibling)
                .count(),
            1
        );
    }
}

struct FailNativeTemplate(String);

impl bevy_ecs::template::Template for FailNativeTemplate {
    type Output = ();
    fn build_template(
        &self,
        _: &mut bevy_ecs::template::TemplateContext<'_, '_>,
    ) -> bevy_ecs::error::Result<()> {
        Err(bevy_ecs::error::BevyError::error(self.0.clone()))
    }
    fn clone_template(&self) -> Self {
        Self(self.0.clone())
    }
}

#[test]
fn facade_native_apply_failure_keeps_original_error_through_fallback() {
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    };
    let mut app = App::new();
    app.add_plugins((
        TaskPoolPlugin::default(),
        AssetPlugin::default(),
        ScenePlugin,
    ));
    let calls = Arc::new(AtomicUsize::new(0));
    let factory_calls = calls.clone();
    let normal_driver = app
        .register_bsn_view(ViewDriverKey::new("failure").unwrap(), move |_| {
            let count = factory_calls.fetch_add(1, Ordering::SeqCst);
            Ok(bevy_scene::SceneFunction(
                move |_, root: &mut bevy_scene::ResolvedScene| {
                    root.push_template(Node::default());
                    let mut child = bevy_scene::ResolvedScene::default();
                    child.push_template(Node {
                        width: Val::Px(913.0),
                        ..Node::default()
                    });
                    child.push_bundle_template(FailNativeTemplate(format!(
                        "authored child failure {count}"
                    )));
                    root.get_or_insert_related_resolved_scenes::<ChildOf>()
                        .scenes
                        .push(child);
                },
            ))
        })
        .unwrap();
    let failures = Arc::new(Mutex::new(Vec::new()));
    let received = failures.clone();
    let error_driver = app
        .register_bsn_view(ViewDriverKey::new("error").unwrap(), move |input| {
            received
                .lock()
                .unwrap()
                .push(input.failure().unwrap().message().to_owned());
            Ok(bsn! { Node { width: Val::Px(71.0) } })
        })
        .unwrap();
    let normal = RouteViewKey::new("page").unwrap();
    let fallback = RouteViewKey::new("error").unwrap();
    let routes = RouteTree::builder()
        .root_with_options(
            RoutePattern::path("/").unwrap(),
            RouteNodeOptions::new().with_view(normal.clone()),
        )
        .unwrap()
        .build()
        .unwrap();
    let mut views = RouteViewRegistry::new();
    views
        .insert(normal, RouteViewDescriptor::new(normal_driver))
        .unwrap();
    views
        .insert(fallback.clone(), RouteViewDescriptor::new(error_driver))
        .unwrap();
    let router = app
        .add_ui_router(RouterConfig::new(routes, views, fallback))
        .unwrap();
    let host = app
        .world_mut()
        .spawn((
            Node {
                width: Val::Px(347.0),
                ..Node::default()
            },
            RouterHostRegistration::new(router),
        ))
        .id();
    app.update();
    let request = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, RouteLocation::new("/"))
        .unwrap();
    for _ in 0..16 {
        app.update();
    }
    let snapshot = app
        .world()
        .resource::<RouterSnapshots>()
        .request(router, request);
    assert!(
        matches!(snapshot, RequestLookup::Found(request) if matches!(request.status(), RequestStatus::Failed(failure) if failure.kind() == NavigationFailureKind::Apply && failure.message().contains("authored child failure 0"))),
        "{snapshot:?}"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(failures.lock().unwrap().len(), 1);
    assert!(failures.lock().unwrap()[0].contains("authored child failure 0"));
    assert_eq!(
        app.world()
            .resource::<RouterSnapshots>()
            .get(router)
            .unwrap()
            .active()
            .kind(),
        ActivePresentationKind::Error
    );
    assert_eq!(app.world().get::<Node>(host).unwrap().width, Val::Px(347.0));
    assert!(
        !app.world_mut()
            .query::<&Node>()
            .iter(app.world())
            .any(|node| node.width == Val::Px(913.0))
    );
}

#[test]
fn facade_native_root_publishes_and_refreshes_dynamic_content() {
    let mut app = App::new();
    app.add_plugins((
        TaskPoolPlugin::default(),
        AssetPlugin::default(),
        ScenePlugin,
    ));
    let driver = app.register_bsn_view(ViewDriverKey::new("native").unwrap(), |_| {
        Ok(bsn! { Node { width: Val::Px(157.0) } NativePage Children [Node { height: Val::Px(29.0) }] })
    }).unwrap();
    let view = RouteViewKey::new("page").unwrap();
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
    app.update();
    let request = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, RouteLocation::new("/"))
        .unwrap();
    for _ in 0..12 {
        app.update();
    }
    let snapshot = app
        .world()
        .resource::<RouterSnapshots>()
        .request(router, request);
    assert!(
        matches!(snapshot, RequestLookup::Found(request) if request.status() == &RequestStatus::Succeeded),
        "{snapshot:?}"
    );
    let mut pages = app
        .world_mut()
        .query_filtered::<bevy_ecs::entity::Entity, bevy_ecs::query::With<NativePage>>();
    let root = pages.single(app.world()).unwrap();
    assert_eq!(app.world().get::<Node>(root).unwrap().width, Val::Px(157.0));
    assert!(
        app.world()
            .resource::<RouterInputGate>()
            .allows_keyboard(root)
    );
    let dynamic = app.world_mut().spawn((Node::default(), ChildOf(root))).id();
    app.update();
    assert!(
        app.world()
            .resource::<RouterInputGate>()
            .allows_keyboard(dynamic)
    );
    let retained_dynamic = app.world_mut().spawn((Node::default(), ChildOf(root))).id();
    let retained_grandchild = app
        .world_mut()
        .spawn((Node::default(), ChildOf(retained_dynamic)))
        .id();
    app.world_mut().entity_mut(dynamic).remove::<ChildOf>();
    app.update();
    assert!(
        !app.world()
            .resource::<RouterInputGate>()
            .allows_keyboard(dynamic)
    );
    let replacement = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .replace(router, RouteLocation::new("/").with_fragment("new"))
        .unwrap();
    for _ in 0..16 {
        app.update();
    }
    let snapshot = app
        .world()
        .resource::<RouterSnapshots>()
        .request(router, replacement);
    assert!(
        matches!(snapshot, RequestLookup::Found(request) if request.status() == &RequestStatus::Succeeded),
        "{snapshot:?}"
    );
    assert!(
        app.world().get_entity(root).is_err(),
        "the leaving native root must be cleaned"
    );
    assert!(
        app.world().get_entity(dynamic).is_ok(),
        "a pre-moved-out child belongs to the application"
    );
    assert!(
        app.world().get_entity(retained_dynamic).is_err()
            && app.world().get_entity(retained_grandchild).is_err(),
        "dynamic content still inside the leaving page must be cleaned"
    );
    assert!(app.world().get_entity(host).is_ok());
}

// This test checks the outlet declaration in an official Scene. Native Router
// registration/publication is covered by the facade and public navigation tests.
#[test]
fn facade_outlet_declaration_is_usable_in_an_official_scene() {
    let mut app = App::new();
    app.add_plugins((
        TaskPoolPlugin::default(),
        AssetPlugin::default(),
        ScenePlugin,
    ));
    let root = app
        .world_mut()
        .spawn_scene(bsn! { DefaultOutlet })
        .unwrap()
        .id();
    assert!(app.world().get::<DefaultOutlet>(root).is_some());
}

#[test]
fn facade_native_cleanup_observer_can_return_remaining_content_and_focus_to_application() {
    assert_cleanup_observer_returns_application_content(2);
}

#[test]
fn facade_native_cleanup_retains_contained_residual_beside_escaped_content() {
    assert_cleanup_observer_returns_application_content(3);
}

fn assert_cleanup_observer_returns_application_content(content: usize) {
    use bevy_ecs::prelude::*;
    use bevy_input_focus::{FocusCause, InputFocus, InputFocusPlugin};

    #[derive(Component)]
    struct EscapeOnDespawn;

    #[derive(Resource)]
    struct EscapeObservation {
        children: Vec<Entity>,
        application_parent: Entity,
        callback_count: usize,
        escaped: Option<Entity>,
    }

    let mut app = App::new();
    app.add_plugins((
        TaskPoolPlugin::default(),
        AssetPlugin::default(),
        ScenePlugin,
        InputFocusPlugin,
    ));
    let driver = app
        .register_bsn_view(ViewDriverKey::new("native").unwrap(), move |_| {
            Ok(bevy_scene::SceneFunction(
                move |_, root: &mut bevy_scene::ResolvedScene| {
                    root.push_template(Node::default());
                    root.push_template(NativePage);
                    for _ in 0..content {
                        let mut child = bevy_scene::ResolvedScene::default();
                        child.push_template(Node::default());
                        root.get_or_insert_related_resolved_scenes::<ChildOf>()
                            .scenes
                            .push(child);
                    }
                },
            ))
        })
        .unwrap();
    let view = RouteViewKey::new("page").unwrap();
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
    let application_parent = app.world_mut().spawn(Node::default()).id();
    app.update();
    let initial = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, RouteLocation::new("/"))
        .unwrap();
    for _ in 0..16 {
        app.update();
    }
    assert!(matches!(
        app.world().resource::<RouterSnapshots>().request(router, initial),
        RequestLookup::Found(request) if request.status() == &RequestStatus::Succeeded
    ));
    let root = app
        .world_mut()
        .query_filtered::<Entity, With<NativePage>>()
        .single(app.world())
        .unwrap();
    let children = app
        .world()
        .get::<Children>(root)
        .unwrap()
        .iter()
        .collect::<Vec<_>>();
    assert_eq!(children.len(), content);
    for child in children.iter().copied() {
        assert!(
            app.world()
                .resource::<RouterInputGate>()
                .allows_keyboard(child)
        );
        app.world_mut().entity_mut(child).insert(EscapeOnDespawn);
    }
    app.insert_resource(EscapeObservation {
        children,
        application_parent,
        callback_count: 0,
        escaped: None,
    });
    app.add_observer(
        |event: On<Despawn<EscapeOnDespawn>>,
         mut commands: Commands,
         mut observation: ResMut<EscapeObservation>| {
            observation.callback_count += 1;
            if observation.callback_count != 1 {
                return;
            }
            let sibling = observation
                .children
                .iter()
                .copied()
                .find(|child| *child != event.entity)
                .unwrap();
            let parent = observation.application_parent;
            // This mutation is emitted by an actual Bevy despawn observer and applied
            // by the normal command flush, while cleanup still has remaining content.
            commands.queue(move |world: &mut World| {
                assert!(
                    world.get_entity(sibling).is_ok(),
                    "the sibling must still be alive when the observer runs"
                );
                world.entity_mut(sibling).insert(ChildOf(parent));
                world
                    .resource_mut::<InputFocus>()
                    .set(sibling, FocusCause::Pressed);
                world.resource_mut::<EscapeObservation>().escaped = Some(sibling);
            });
        },
    );
    let replacement = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .replace(router, RouteLocation::new("/").with_fragment("replacement"))
        .unwrap();
    let mut settled = false;
    for _ in 0..32 {
        app.update();
        let snapshots = app.world().resource::<RouterSnapshots>();
        let snapshot = snapshots.get(router).unwrap();
        if matches!(snapshots.request(router, replacement), RequestLookup::Found(request) if request.status() == &RequestStatus::Succeeded)
            && snapshot.running_request().is_none()
            && !snapshot.cleanup().foreground_pending()
        {
            settled = true;
            break;
        }
    }
    assert!(
        settled,
        "published replacement and foreground cleanup must finish within 32 updates"
    );
    // Additional normal updates ensure a later gate/focus refresh does not reclaim
    // the application-owned entity after the cleanup request has completed.
    for _ in 0..4 {
        app.update();
    }
    let observation = app.world().resource::<EscapeObservation>();
    assert_eq!(
        observation.callback_count, 1,
        "the escaped sibling must not also be despawned"
    );
    let escaped = observation
        .escaped
        .expect("the real despawn observer must run");
    assert!(app.world().get_entity(escaped).is_ok());
    assert_eq!(
        app.world().get::<ChildOf>(escaped).unwrap().parent(),
        application_parent
    );
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(escaped));
    let gate = app.world().resource::<RouterInputGate>();
    assert!(!gate.allows_keyboard(escaped));
    assert!(!gate.allows_pointer(escaped));
    assert!(!gate.allows_gamepad(escaped));
    let retained = observation
        .children
        .iter()
        .copied()
        .filter(|entity| *entity != escaped && app.world().get_entity(*entity).is_ok())
        .collect::<Vec<_>>();
    assert_eq!(retained.len(), content - 2);
    for entity in retained {
        assert_eq!(
            app.world().get::<ChildOf>(entity).map(ChildOf::parent),
            Some(root)
        );
        assert!(!gate.allows_keyboard(entity));
        assert!(!gate.allows_pointer(entity));
        assert!(!gate.allows_gamepad(entity));
        let mut ancestor = Some(entity);
        let mut hidden_by_display = false;
        while let Some(current) = ancestor {
            hidden_by_display |= app
                .world()
                .get::<Node>(current)
                .is_some_and(|node| node.display == bevy_ui::Display::None);
            ancestor = app.world().get::<ChildOf>(current).map(ChildOf::parent);
        }
        assert!(
            hidden_by_display,
            "contained residual must remain visually isolated"
        );
    }
    assert!(app.world().get_entity(host).is_ok());
    assert!(app.world().get_entity(application_parent).is_ok());
    let snapshots = app.world().resource::<RouterSnapshots>();
    assert!(
        matches!(snapshots.request(router, replacement), RequestLookup::Found(request) if request.status() == &RequestStatus::Succeeded)
    );
    assert!(snapshots.get(router).unwrap().running_request().is_none());
    assert!(
        !snapshots
            .get(router)
            .unwrap()
            .cleanup()
            .foreground_pending()
    );
}
