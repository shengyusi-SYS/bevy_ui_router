// No-BSN feature consumer: exercises plugin/resource setup and synchronous
// installation rejection, not rendering. Independent rendering behavior is
// covered by private ECS unit tests; the BSN consumer proves public rendering.
use bevy_app::{App, Update};
use bevy_ecs::prelude::{Component, Entity, Query};
use bevy_ui::{Node, Val};
use bevy_ui_router::prelude::*;

#[derive(Component)]
struct HostSentinel(u32);

fn tick_active_content(activity: RouteActivity, inputs: RouteInputs, mut work: Query<(Entity, &mut HostSentinel)>) {
    for (entity, mut sentinel) in &mut work {
        assert!(inputs.current(entity).is_none(), "app-owned host has no route input");
        if activity.is_active(entity) {
            sentinel.0 += 1;
        }
    }
}

fn config(bound: bool) -> RouterConfig {
    let view = RouteViewKey::new("normal").unwrap();
    let mut views = RouteViewRegistry::new();
    if bound {
        views
            .insert(
                view.clone(),
                RouteViewDescriptor::new(ViewDriverKey::new("unregistered").unwrap()),
            )
            .unwrap();
    }
    let routes = RouteTree::builder()
        .root_with_options(
            RoutePattern::path("/").unwrap(),
            RouteNodeOptions::new().with_view(view.clone()),
        )
        .unwrap()
        .build()
        .unwrap();
    RouterConfig::new(routes, views, view)
}

fn main() {
    let mut app = App::new();
    app.add_plugins(BevyUiRouterPlugin::default());
    app.add_systems(Update, tick_active_content);
    assert!(app.world().contains_resource::<RouterNavigation>());
    assert!(app.world().contains_resource::<RouterSnapshots>());
    assert!(app.world().contains_resource::<RouterInputGate>());
    let node = Node {
        width: Val::Px(37.0),
        ..Node::default()
    };
    let host = app.world_mut().spawn((node.clone(), HostSentinel(7))).id();
    let entities_before = app.world().entities().count_spawned();

    // No production renderer is registered by the ECS-only feature. A view
    // binding and an executable factory remain distinct installation facts.
    assert_eq!(
        app.add_ui_router(config(false)).unwrap_err().kind(),
        RouterInstallErrorKind::UnknownRouteView,
    );
    assert_eq!(
        app.add_ui_router(config(true)).unwrap_err().kind(),
        RouterInstallErrorKind::UnknownViewDriver,
    );
    app.update();
    assert_eq!(app.world().entities().count_spawned(), entities_before);
    assert_eq!(app.world().get::<Node>(host), Some(&node));
    assert_eq!(app.world().get::<HostSentinel>(host).unwrap().0, 7);
    let gate = app.world().resource::<RouterInputGate>();
    assert!(!gate.allows_pointer(host));
    assert!(!gate.allows_keyboard(host));
    assert!(!gate.allows_gamepad(host));
}
