//! Public held-exit navigation through the official UI layout and picking
//! backend. No computed geometry, visibility, or lifecycle result is injected.

use std::collections::HashMap;

use bevy_app::{App, PreUpdate};
use bevy_asset::AssetPlugin;
use bevy_camera::visibility::{InheritedVisibility, Visibility};
use bevy_ecs::prelude::*;
use bevy_internal::{
    DefaultPlugins,
    camera::{Camera2d, NormalizedRenderTarget},
    math::Vec2,
    window::{PrimaryWindow, Window, WindowRef},
};
use bevy_picking::{
    Pickable, PickingSystems,
    events::{PointerClick, PointerPress},
    hover::HoverMap,
    pointer::{
        Location, PointerAction, PointerButton, PointerId, PointerInput, PointerInteraction,
        PointerLocation, PointerPressState,
    },
};
use bevy_scene::{ScenePlugin, bsn};
use bevy_ui::{
    ComputedNode, GlobalZIndex, Node, OverrideClip, PositionType, UiGlobalTransform, UiStack, Val,
};
use bevy_ui_router_bsn::DefaultOutlet;
use bevy_ui_router_core::prelude::*;
use bevy_ui_router_view::{RouteViewDescriptor, RouteViewRegistry, ViewDriverKey};

use super::{
    NativeUiInputProjection, project_native_ui_input_stack, restore_native_ui_input_stack,
};
use crate::{
    prelude::*,
    registry::{EntityLifecycle, RouteEntityRegistry},
};

#[derive(Clone, Component, Default)]
struct Page(u32);

#[derive(Default, Resource)]
struct HeldExit {
    presented: Option<RouteTransitionId>,
}

struct HoldVisibleExit;

impl RouteTransitionDriver for HoldVisibleExit {
    fn prepare(
        &mut self,
        world: &mut World,
        context: &RouteTransitionContext,
    ) -> TransitionPreparation {
        if context.leaving().is_empty() {
            return TransitionPreparation::Skip;
        }
        for target in context.entering().iter().chain(context.leaving()) {
            if target.is_outermost() {
                let mut node = world.get_mut::<Node>(target.surface()).unwrap();
                node.position_type = PositionType::Absolute;
                node.left = Val::Px(0.0);
                node.top = Val::Px(0.0);
            }
        }
        TransitionPreparation::Run { hold_exit: true }
    }

    fn presented(&mut self, world: &mut World, id: &RouteTransitionId) {
        world.resource_mut::<HeldExit>().presented = Some(id.clone());
    }

    fn cancelled(
        &mut self,
        world: &mut World,
        id: &RouteTransitionId,
        _reason: TransitionCancelReason,
    ) {
        let mut held = world.resource_mut::<HeldExit>();
        if held.presented.as_ref() == Some(id) {
            held.presented = None;
        }
    }
}

#[derive(Default, Resource)]
struct StackAudit {
    source: Option<UiStack>,
    completed: usize,
}

fn capture_backend_source(
    stack: Res<UiStack>,
    projection: Res<NativeUiInputProjection>,
    mut audit: ResMut<StackAudit>,
) {
    assert!(audit.source.is_none());
    if projection.presence {
        audit.source = Some(UiStack {
            uinodes: stack.uinodes.clone(),
            partition: stack.partition.clone(),
        });
    }
}

fn assert_source_restored_before_hover(
    stack: Res<UiStack>,
    projection: Res<NativeUiInputProjection>,
    mut audit: ResMut<StackAudit>,
) {
    assert!(projection.original.is_none());
    if let Some(source) = audit.source.take() {
        assert_eq!(stack.uinodes, source.uinodes);
        assert_eq!(stack.partition, source.partition);
        audit.completed += 1;
    }
}

#[derive(Default, Resource)]
struct Deliveries {
    presses: HashMap<Entity, usize>,
    clicks: HashMap<Entity, usize>,
}

fn node(left: f32, top: f32, width: f32, height: f32) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Px(left),
        top: Val::Px(top),
        width: Val::Px(width),
        height: Val::Px(height),
        ..Node::default()
    }
}

fn push(app: &mut App, router: RouterHandle, path: &str) {
    let id = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, RouteLocation::new(path))
        .unwrap();
    for _ in 0..32 {
        app.update();
        let request = app
            .world()
            .resource::<RouterSnapshots>()
            .request(router, id);
        if matches!(request, RequestLookup::Found(request) if request.status() == &RequestStatus::Succeeded)
        {
            return;
        }
        assert!(
            !matches!(request, RequestLookup::Found(request) if matches!(request.status(), RequestStatus::Failed(_))),
            "navigation failed: {request:?}"
        );
    }
    panic!("navigation failed to settle");
}

fn push_to_held_exit(app: &mut App, router: RouterHandle, path: &str) -> NavigationRequestId {
    let id = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, RouteLocation::new(path))
        .unwrap();
    for _ in 0..32 {
        app.update();
        let request = app
            .world()
            .resource::<RouterSnapshots>()
            .request(router, id);
        assert!(
            !matches!(request, RequestLookup::Found(request) if matches!(request.status(), RequestStatus::Failed(_))),
            "navigation failed: {request:?}"
        );
        if app.world().resource::<HeldExit>().presented.is_some() {
            assert!(
                matches!(request, RequestLookup::Found(request) if request.status() == &RequestStatus::Running(NavigationPhase::PostCommitCleanup))
            );
            return id;
        }
    }
    panic!("navigation did not publish its held exit");
}

fn page(app: &mut App, id: u32) -> Entity {
    app.world_mut()
        .query::<(Entity, &Page)>()
        .iter(app.world())
        .find_map(|(entity, page)| (page.0 == id).then_some(entity))
        .unwrap()
}

fn location_at(world: &World, window: Entity, target: Entity) -> Location {
    assert!(
        world
            .get::<ComputedNode>(target)
            .unwrap()
            .size()
            .cmpgt(Vec2::ZERO)
            .all()
    );
    assert!(world.get::<InheritedVisibility>(target).unwrap().get());
    Location {
        target: NormalizedRenderTarget::Window(WindowRef::Primary.normalize(Some(window)).unwrap()),
        position: *world.get::<UiGlobalTransform>(target).unwrap() * Vec2::ZERO,
    }
}

fn click(app: &mut App, pointer: PointerId, location: &Location) {
    for action in [
        PointerAction::Press(PointerButton::Primary),
        PointerAction::Release(PointerButton::Primary),
    ] {
        app.world_mut()
            .write_message(PointerInput::new(pointer, location.clone(), action));
        app.update();
    }
}

fn hovered(app: &App, pointer: PointerId, target: Entity) -> bool {
    app.world()
        .resource::<HoverMap>()
        .get(&pointer)
        .is_some_and(|hits| hits.contains_key(&target))
}

#[test]
fn held_exit_input_candidates_do_not_block_active_or_application_targets() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins);
    if !app.is_plugin_added::<AssetPlugin>() {
        app.add_plugins(AssetPlugin::default());
    }
    if !app.is_plugin_added::<ScenePlugin>() {
        app.add_plugins(ScenePlugin);
    }
    app.init_resource::<StackAudit>()
        .init_resource::<Deliveries>()
        .init_resource::<HeldExit>()
        .add_systems(
            PreUpdate,
            (
                capture_backend_source
                    .after(RouterInputSet::Refresh)
                    .before(project_native_ui_input_stack),
                assert_source_restored_before_hover
                    .after(restore_native_ui_input_stack)
                    .before(PickingSystems::Hover),
            ),
        );
    app.world_mut().add_observer(
        |event: On<PointerPress>, mut deliveries: ResMut<Deliveries>| {
            *deliveries.presses.entry(event.event_target()).or_default() += 1;
        },
    );
    app.world_mut().add_observer(
        |event: On<PointerClick>, mut deliveries: ResMut<Deliveries>| {
            *deliveries.clicks.entry(event.event_target()).or_default() += 1;
        },
    );
    let window = app
        .world_mut()
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(app.world())
        .unwrap();
    app.world_mut()
        .get_mut::<Window>(window)
        .unwrap()
        .resolution
        .set(320.0, 180.0);
    app.world_mut().spawn(Camera2d);
    let layout_driver = app
        .register_bsn_view(
            ViewDriverKey::new("input-projection-layout").unwrap(),
            |_| {
                Ok(bsn! {
                    Node { width: Val::Percent(100.0), height: Val::Percent(100.0) }
                    Pickable { should_block_lower: false, is_hoverable: false }
                    DefaultOutlet
                })
            },
        )
        .unwrap();
    let page_driver = app
        .register_bsn_view(
            ViewDriverKey::new("input-projection-page").unwrap(),
            |input| {
                let id = input.params().get("id").unwrap().parse::<u32>().unwrap();
                Ok(bsn! {
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(20.0), top: Val::Px(20.0),
                        width: Val::Px(100.0), height: Val::Px(80.0),
                    }
                    Page(id)
                })
            },
        )
        .unwrap();
    let layout_view = RouteViewKey::new("input-projection-layout").unwrap();
    let page_view = RouteViewKey::new("input-projection-page").unwrap();
    let mut views = RouteViewRegistry::new();
    views
        .insert(layout_view.clone(), RouteViewDescriptor::new(layout_driver))
        .unwrap();
    views
        .insert(page_view.clone(), RouteViewDescriptor::new(page_driver))
        .unwrap();
    let mut routes = RouteTree::builder()
        .root_with_options(
            RoutePattern::path("/").unwrap(),
            RouteNodeOptions::new().with_view(layout_view.clone()),
        )
        .unwrap();
    routes
        .route_with_options(
            routes.root_id().unwrap(),
            RoutePattern::path("items/:id").unwrap(),
            RouteNodeOptions::new()
                .with_view(page_view)
                .with_keep_alive(true),
        )
        .unwrap();
    let router = app
        .add_ui_router(
            RouterConfig::new(routes.build().unwrap(), views, layout_view)
                .with_keep_alive_capacity(2)
                .with_transition_driver(HoldVisibleExit),
        )
        .unwrap();
    app.world_mut().spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..Node::default()
        },
        Pickable::IGNORE,
        RouterHostRegistration::new(router),
    ));
    let application = app
        .world_mut()
        .spawn((node(180.0, 20.0, 80.0, 80.0), Pickable::default()))
        .id();
    app.update();
    push(&mut app, router, "/items/1");
    let old = page(&mut app, 1);
    let held_request = push_to_held_exit(&mut app, router, "/items/2");
    let new = page(&mut app, 2);
    assert_eq!(
        app.world()
            .resource::<RouteEntityRegistry>()
            .record(old)
            .unwrap()
            .lifecycle(),
        EntityLifecycle::Retained
    );
    assert!(
        !app.world()
            .resource::<RouterInputGate>()
            .allows_pointer(old)
    );
    assert!(
        app.world()
            .resource::<RouterInputGate>()
            .allows_pointer(new)
    );
    app.world_mut()
        .entity_mut(old)
        .insert((GlobalZIndex(100), OverrideClip));
    let old_node = app.world().get::<Node>(old).unwrap().clone();
    let old_visibility = *app.world().get::<Visibility>(old).unwrap();
    assert!(app.world().get::<Pickable>(old).is_none());
    app.update();
    app.update();
    let overlap = location_at(app.world(), window, old);
    let application_location = location_at(app.world(), window, application);
    let produced_presence = app.world().resource::<NativeUiInputProjection>().presence;
    assert!(
        produced_presence,
        "the public held-exit producer enables projection"
    );
    let pointer = PointerId::Touch(0x5452_1001);
    let app_pointer = PointerId::Touch(0x5452_1002);
    for (id, location) in [
        (pointer, overlap.clone()),
        (app_pointer, application_location.clone()),
    ] {
        app.world_mut().spawn((
            id,
            PointerLocation::new(location),
            PointerPressState::default(),
            PointerInteraction::default(),
        ));
    }
    // Negative control: the production post-hover gate already rejects old,
    // but without candidate projection it cannot recover the obscured new hit.
    // Temporarily disable only the optimization input produced above; restore
    // its actual value afterwards. Scope, visibility, and geometry stay real.
    app.world_mut()
        .resource_mut::<NativeUiInputProjection>()
        .set_presence(false);
    app.update();
    assert!(!hovered(&app, pointer, old));
    assert!(!hovered(&app, pointer, new));
    assert!(hovered(&app, app_pointer, application));
    assert!(
        app.world()
            .resource::<RouterInputGate>()
            .allows_pointer(new)
    );
    app.world_mut()
        .resource_mut::<NativeUiInputProjection>()
        .set_presence(produced_presence);
    app.update();
    assert!(hovered(&app, pointer, new));
    assert!(!hovered(&app, pointer, old));
    click(&mut app, pointer, &overlap);
    click(&mut app, app_pointer, &application_location);
    let deliveries = app.world().resource::<Deliveries>();
    assert_eq!(deliveries.presses.get(&new), Some(&1));
    assert_eq!(deliveries.clicks.get(&new), Some(&1));
    assert_eq!(deliveries.clicks.get(&application), Some(&1));
    assert_eq!(deliveries.presses.get(&old), None);
    assert_eq!(deliveries.clicks.get(&old), None);
    assert_eq!(app.world().get::<Node>(old), Some(&old_node));
    assert_eq!(app.world().get::<Visibility>(old), Some(&old_visibility));
    assert!(app.world().get::<Pickable>(old).is_none());
    assert!(app.world().get::<InheritedVisibility>(old).unwrap().get());

    let dynamic = app
        .world_mut()
        .spawn((
            node(0.0, 0.0, 100.0, 80.0),
            ChildOf(old),
            GlobalZIndex(200),
            OverrideClip,
        ))
        .id();
    app.update();
    app.update();
    assert!(hovered(&app, pointer, new));
    assert!(!hovered(&app, pointer, dynamic));
    assert!(
        !app.world()
            .resource::<RouterInputGate>()
            .allows_pointer(dynamic)
    );
    click(&mut app, pointer, &overlap);
    assert_eq!(
        app.world().resource::<Deliveries>().clicks.get(&new),
        Some(&2)
    );
    assert_eq!(
        app.world().resource::<Deliveries>().clicks.get(&dynamic),
        None
    );

    app.world_mut()
        .entity_mut(dynamic)
        .insert(ChildOf(application));
    app.update();
    app.update();
    let moved = location_at(app.world(), window, dynamic);
    app.world_mut().write_message(PointerInput::new(
        app_pointer,
        moved.clone(),
        PointerAction::Move { delta: Vec2::ZERO },
    ));
    app.update();
    assert!(hovered(&app, app_pointer, dynamic));
    click(&mut app, app_pointer, &moved);
    assert_eq!(
        app.world().resource::<Deliveries>().clicks.get(&dynamic),
        Some(&1)
    );
    assert!(app.world().resource::<StackAudit>().completed > 0);
    assert!(app.world().resource::<StackAudit>().source.is_none());
    assert!(
        app.world_mut()
            .resource_mut::<NativeUiInputProjection>()
            .take_failure()
            .is_none()
    );

    let session = app
        .world_mut()
        .resource_mut::<HeldExit>()
        .presented
        .take()
        .unwrap();
    app.world_mut().commands().finish_route_transition(session);
    app.world_mut().flush();
    for _ in 0..32 {
        app.update();
        let request = app
            .world()
            .resource::<RouterSnapshots>()
            .request(router, held_request);
        if matches!(request, RequestLookup::Found(request) if request.status() == &RequestStatus::Succeeded)
        {
            break;
        }
        assert!(
            !matches!(request, RequestLookup::Found(request) if matches!(request.status(), RequestStatus::Failed(_)))
        );
    }
    assert!(
        matches!(app.world().resource::<RouterSnapshots>().request(router, held_request), RequestLookup::Found(request) if request.status() == &RequestStatus::Succeeded)
    );
    app.update();
    assert!(!app.world().get::<InheritedVisibility>(old).unwrap().get());
    assert!(!app.world().resource::<NativeUiInputProjection>().presence);
    assert_eq!(
        app.world().get::<ChildOf>(dynamic).unwrap().parent(),
        application
    );
}
