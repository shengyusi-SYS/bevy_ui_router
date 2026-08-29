#![cfg(feature = "ecs")]

use std::{
    collections::HashSet,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use bevy_app::App;
use bevy_camera::visibility::Visibility;
use bevy_ecs::{
    change_detection::Tick,
    component::ComponentId,
    entity::Entity,
    hierarchy::{ChildOf, Children},
    lifecycle::Insert,
    observer::On,
    prelude::Component,
};
use bevy_picking::Pickable;
use bevy_ui::{BackgroundColor, Node, Val};
#[cfg(feature = "test-support")]
use bevy_ui_router::ecs::{AuthorityFrameLifecycle, RouterAuthorityAudits};
use bevy_ui_router::prelude::*;
#[cfg(feature = "test-support")]
use bevy_ui_router_ecs::test_support::{
    testing_back_request_inventory, testing_router_request_inventory,
};

const NORMAL_RECT_WIDTH: u16 = 31;
const FALLBACK_RECT_WIDTH: u16 = 79;
const PARTIAL_RECT_WIDTH: u16 = 199;
const RECOVERY_PARTIAL_RECT_WIDTH: u16 = 197;

#[derive(Component, Debug, Eq, PartialEq)]
struct HostSentinel(u32);

#[derive(Component, Debug, Eq, PartialEq)]
struct HostMetadata(u64);

#[derive(Component, Debug, Eq, PartialEq)]
struct ExistingChild(u32);

#[derive(Component, Debug, Eq, PartialEq)]
struct ExistingSibling(u32);

#[derive(Clone, Debug, Eq, PartialEq)]
struct HostBoundaryFingerprint {
    host: Entity,
    host_non_children_components: Vec<ComponentId>,
    existing_children: Vec<Entity>,
    existing_child_components: Vec<(Entity, Vec<ComponentId>)>,
    top_slot: Option<Entity>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FixtureMode {
    Success { width: u16 },
    BuildFailure,
    ApplyFailure,
    DuplicateOutlet,
    ForeignSession,
    RelationshipCycle,
    MissingOutlet { width: u16 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct EffectRecord {
    view: RouteViewKey,
    location: RouteLocation,
    route: Option<RouteId>,
    id_param: Option<String>,
    fragment: Option<String>,
    origin: FrameOrigin,
    failure: Option<NavigationFailureKind>,
}

#[derive(Default)]
struct FixtureState {
    modes: Vec<(RouteViewKey, FixtureMode)>,
    one_shot_modes: Vec<(RouteViewKey, FixtureMode)>,
    effects: Vec<EffectRecord>,
}

#[derive(Clone, Default)]
struct FixtureControl {
    state: Arc<Mutex<FixtureState>>,
}

impl FixtureControl {
    fn set_mode(&self, view: &RouteViewKey, mode: FixtureMode) {
        let mut state = self
            .state
            .lock()
            .expect("fixture lock must not be poisoned");
        if let Some((_, stored)) = state.modes.iter_mut().find(|(key, _)| key == view) {
            *stored = mode;
        } else {
            state.modes.push((view.clone(), mode));
        }
    }

    fn set_one_shot_mode(&self, view: &RouteViewKey, mode: FixtureMode) {
        let mut state = self
            .state
            .lock()
            .expect("fixture lock must not be poisoned");
        if let Some((_, stored)) = state.one_shot_modes.iter_mut().find(|(key, _)| key == view) {
            *stored = mode;
        } else {
            state.one_shot_modes.push((view.clone(), mode));
        }
    }

    fn mode_and_record(&self, effect: &ViewBuildEffect) -> FixtureMode {
        let mut state = self
            .state
            .lock()
            .expect("fixture lock must not be poisoned");
        let one_shot = state
            .one_shot_modes
            .iter()
            .position(|(key, _)| key == effect.view());
        let mode = match one_shot {
            Some(index) => state.one_shot_modes.remove(index).1,
            None => state
                .modes
                .iter()
                .find_map(|(key, mode)| (key == effect.view()).then_some(*mode))
                .expect("every fixture view must have a mode"),
        };
        state.effects.push(EffectRecord {
            view: effect.view().clone(),
            location: effect.input().location().clone(),
            route: effect.input().route(),
            id_param: effect.input().params().get("id").map(str::to_owned),
            fragment: effect.input().fragment().map(str::to_owned),
            origin: effect.input().origin().clone(),
            failure: effect.input().failure().map(NavigationFailure::kind),
        });
        mode
    }

    fn effects(&self) -> Vec<EffectRecord> {
        self.state
            .lock()
            .expect("fixture lock must not be poisoned")
            .effects
            .clone()
    }
}

struct FixtureDriver {
    key: ViewDriverKey,
    control: FixtureControl,
    outlet_views: Vec<RouteViewKey>,
    prior_content: Option<ContentEntity>,
}

struct InstallMatrixDriver {
    key: ViewDriverKey,
}

impl EcsViewDriver for InstallMatrixDriver {
    fn key(&self) -> ViewDriverKey {
        self.key.clone()
    }

    fn apply(
        &mut self,
        _effect: &ViewBuildEffect,
        content: &mut FrameContentWriter<'_>,
        report: EcsViewReportSession<'_>,
    ) -> ViewBuildReport {
        content.spawn_empty().expect("install matrix driver stages");
        report.applied()
    }
}

impl EcsViewDriver for FixtureDriver {
    fn key(&self) -> ViewDriverKey {
        self.key.clone()
    }

    fn apply(
        &mut self,
        effect: &ViewBuildEffect,
        content: &mut FrameContentWriter<'_>,
        report: EcsViewReportSession<'_>,
    ) -> ViewBuildReport {
        match self.control.mode_and_record(effect) {
            FixtureMode::Success { width } => {
                let container = stage_rect(content, width);
                self.prior_content = Some(container);
                if self.outlet_views.iter().any(|view| view == effect.view()) {
                    content
                        .declare_outlet(container, RouteSlotKey::default())
                        .expect("layout outlet should stage");
                }
                report.applied()
            }
            FixtureMode::BuildFailure => report.build_failed("fixture build failure"),
            FixtureMode::ApplyFailure => {
                let width = match effect.input().origin() {
                    FrameOrigin::RouteError(_) | FrameOrigin::RouterFallback => {
                        RECOVERY_PARTIAL_RECT_WIDTH
                    }
                    FrameOrigin::MatchedRoute(_) => PARTIAL_RECT_WIDTH,
                };
                self.prior_content = Some(stage_rect(content, width));
                report.apply_failed("fixture apply failure after partial plan")
            }
            FixtureMode::DuplicateOutlet => {
                let anchor = content
                    .spawn_empty()
                    .expect("duplicate outlet anchor stages");
                content
                    .declare_outlet(anchor, RouteSlotKey::default())
                    .expect("first outlet declaration stages");
                let error = content
                    .declare_outlet(anchor, RouteSlotKey::default())
                    .expect_err("duplicate outlet must be rejected by the production writer");
                report.apply_failed(error.to_string())
            }
            FixtureMode::ForeignSession => {
                let foreign = self
                    .prior_content
                    .expect("the preceding committed frame supplies a foreign session handle");
                let error = content
                    .declare_outlet(foreign, RouteSlotKey::default())
                    .expect_err(
                        "a foreign-session outlet anchor must be rejected by the production writer",
                    );
                report.apply_failed(error.to_string())
            }
            FixtureMode::RelationshipCycle => {
                let parent = content.spawn_empty().expect("cycle parent stages");
                let child = content.spawn_empty().expect("cycle child stages");
                content
                    .add_child(parent, child)
                    .expect("the acyclic prefix stages");
                let error = content
                    .add_child(child, parent)
                    .expect_err("relationship cycle must be rejected by the production writer");
                report.apply_failed(error.to_string())
            }
            FixtureMode::MissingOutlet { width } => {
                self.prior_content = Some(stage_rect(content, width));
                report.applied()
            }
        }
    }
}

fn stage_rect(content: &mut FrameContentWriter<'_>, width: u16) -> ContentEntity {
    let container = content
        .spawn_empty()
        .expect("fixture container should stage");
    let rect = content
        .spawn_solid_rect(
            SolidRect::new(width, 23, ContentColor::srgba8(17, 43, 89, 255))
                .expect("fixture rectangle should be valid"),
        )
        .expect("fixture rectangle should stage");
    content
        .add_child(container, rect)
        .expect("fixture content edge should stage");
    container
}

struct RootFixture {
    app: App,
    router: RouterHandle,
    control: FixtureControl,
    normal: RouteViewKey,
    fallback: RouteViewKey,
    existing_children: Vec<Entity>,
    host_fingerprint: HostBoundaryFingerprint,
    admission_entities: HashSet<Entity>,
}

fn root_fixture(pattern: &str) -> RootFixture {
    let mut app = App::new();
    app.add_plugins(BevyUiRouterPlugin::default());

    let normal = RouteViewKey::new("fixture-normal").unwrap();
    let fallback = RouteViewKey::new("fixture-fallback").unwrap();
    let control = FixtureControl::default();
    control.set_mode(
        &normal,
        FixtureMode::Success {
            width: NORMAL_RECT_WIDTH,
        },
    );
    control.set_mode(
        &fallback,
        FixtureMode::Success {
            width: FALLBACK_RECT_WIDTH,
        },
    );

    let driver = app
        .register_ui_view_driver(FixtureDriver {
            key: ViewDriverKey::new("fixture-driver").unwrap(),
            control: control.clone(),
            outlet_views: Vec::new(),
            prior_content: None,
        })
        .unwrap();
    let routes = RouteTree::builder()
        .root_with_options(
            RoutePattern::path(pattern).unwrap(),
            RouteNodeOptions::new().with_view(normal.clone()),
        )
        .unwrap()
        .build()
        .unwrap();
    let mut views = RouteViewRegistry::new();
    for view in [normal.clone(), fallback.clone()] {
        views
            .insert(view, RouteViewDescriptor::new(driver.clone()))
            .unwrap();
    }
    let router = app
        .add_ui_router(RouterConfig::new(routes, views, fallback.clone()))
        .unwrap();

    let host = app
        .world_mut()
        .spawn((
            Node {
                width: Val::Px(640.0),
                height: Val::Px(360.0),
                ..Node::default()
            },
            HostSentinel(7),
            HostMetadata(73),
            RouterHostRegistration::new(router),
        ))
        .id();
    let existing_child = app
        .world_mut()
        .spawn((
            Node {
                width: Val::Px(13.0),
                height: Val::Px(17.0),
                ..Node::default()
            },
            ExistingChild(11),
            ChildOf(host),
        ))
        .id();
    let existing_sibling = app
        .world_mut()
        .spawn((
            Node {
                width: Val::Px(19.0),
                height: Val::Px(29.0),
                ..Node::default()
            },
            ExistingSibling(17),
            ChildOf(host),
        ))
        .id();
    let existing_children = vec![existing_child, existing_sibling];
    let mut host_fingerprint = capture_host_boundary(&app, host, &existing_children);
    app.update();
    let top_slot = app
        .world()
        .get::<Children>(host)
        .unwrap()
        .iter()
        .copied()
        .find(|child| !existing_children.contains(child))
        .expect("admission adds one exact router top slot");
    host_fingerprint.top_slot = Some(top_slot);
    let admission_entities = entity_set(&app);

    RootFixture {
        app,
        router,
        control,
        normal,
        fallback,
        existing_children,
        host_fingerprint,
        admission_entities,
    }
}

struct NestedFixture {
    app: App,
    router: RouterHandle,
    control: FixtureControl,
    layout: RouteViewKey,
    leaf: RouteViewKey,
    layout_route: RouteId,
    leaf_route: RouteId,
    host_fingerprint: HostBoundaryFingerprint,
}

struct RecoveryR1Fixture {
    app: App,
    router: RouterHandle,
    control: FixtureControl,
    section: RouteViewKey,
    leaf: RouteViewKey,
    error: RouteViewKey,
    fallback: RouteViewKey,
    root_route: RouteId,
    boundary_route: RouteId,
    host_fingerprint: HostBoundaryFingerprint,
}

fn recovery_r1_fixture(label: &str) -> RecoveryR1Fixture {
    let mut app = App::new();
    app.add_plugins(BevyUiRouterPlugin::default());

    let root = RouteViewKey::new(format!("{label}-normal-root")).unwrap();
    let section = RouteViewKey::new(format!("{label}-normal-section")).unwrap();
    let leaf = RouteViewKey::new(format!("{label}-normal-leaf")).unwrap();
    let error = RouteViewKey::new(format!("{label}-bound-error")).unwrap();
    let fallback = RouteViewKey::new(format!("{label}-router-fallback")).unwrap();
    let boundary = ErrorBoundaryKey::named(format!("{label}-boundary")).unwrap();
    let control = FixtureControl::default();
    for (view, width) in [
        (root.clone(), 51),
        (section.clone(), 53),
        (leaf.clone(), 55),
        (error.clone(), 57),
        (fallback.clone(), 59),
    ] {
        control.set_mode(&view, FixtureMode::Success { width });
    }
    let driver = app
        .register_ui_view_driver(FixtureDriver {
            key: ViewDriverKey::new(format!("{label}-driver")).unwrap(),
            control: control.clone(),
            outlet_views: vec![root.clone(), section.clone()],
            prior_content: None,
        })
        .unwrap();

    let mut routes = RouteTree::builder()
        .root_with_options(
            RoutePattern::pathless(),
            RouteNodeOptions::new().with_view(root.clone()),
        )
        .unwrap();
    let root_route = routes.root_id().unwrap();
    let boundary_route = routes
        .route_with_options(
            root_route,
            RoutePattern::path("section").unwrap(),
            RouteNodeOptions::new()
                .with_view(section.clone())
                .with_slot(RouteSlotKey::default())
                .with_error_view(boundary, error.clone()),
        )
        .unwrap();
    routes
        .route_with_options(
            boundary_route,
            RoutePattern::path("items/:id").unwrap(),
            RouteNodeOptions::new()
                .with_view(leaf.clone())
                .with_slot(RouteSlotKey::default()),
        )
        .unwrap();
    let mut views = RouteViewRegistry::new();
    for view in [
        root.clone(),
        section.clone(),
        leaf.clone(),
        error.clone(),
        fallback.clone(),
    ] {
        views
            .insert(view, RouteViewDescriptor::new(driver.clone()))
            .unwrap();
    }
    let router = app
        .add_ui_router(RouterConfig::new(
            routes.build().unwrap(),
            views,
            fallback.clone(),
        ))
        .unwrap();
    let host = app
        .world_mut()
        .spawn((
            Node {
                width: Val::Px(640.0),
                height: Val::Px(360.0),
                ..Node::default()
            },
            HostSentinel(7),
            HostMetadata(73),
            RouterHostRegistration::new(router),
        ))
        .id();
    let existing_child = app
        .world_mut()
        .spawn((
            Node {
                width: Val::Px(13.0),
                height: Val::Px(17.0),
                ..Node::default()
            },
            ExistingChild(11),
            ChildOf(host),
        ))
        .id();
    let existing_sibling = app
        .world_mut()
        .spawn((
            Node {
                width: Val::Px(19.0),
                height: Val::Px(29.0),
                ..Node::default()
            },
            ExistingSibling(17),
            ChildOf(host),
        ))
        .id();
    let existing_children = vec![existing_child, existing_sibling];
    let mut host_fingerprint = capture_host_boundary(&app, host, &existing_children);
    app.update();
    host_fingerprint.top_slot = app
        .world()
        .get::<Children>(host)
        .unwrap()
        .iter()
        .copied()
        .find(|child| !existing_children.contains(child));

    RecoveryR1Fixture {
        app,
        router,
        control,
        section,
        leaf,
        error,
        fallback,
        root_route,
        boundary_route,
        host_fingerprint,
    }
}

fn nested_fixture() -> NestedFixture {
    let mut app = App::new();
    app.add_plugins(BevyUiRouterPlugin::default());

    let layout = RouteViewKey::new("nested-layout").unwrap();
    let leaf = RouteViewKey::new("nested-leaf").unwrap();
    let fallback = RouteViewKey::new("nested-fallback").unwrap();
    let control = FixtureControl::default();
    for (view, width) in [
        (layout.clone(), 41),
        (leaf.clone(), 43),
        (fallback.clone(), FALLBACK_RECT_WIDTH),
    ] {
        control.set_mode(&view, FixtureMode::Success { width });
    }
    let driver = app
        .register_ui_view_driver(FixtureDriver {
            key: ViewDriverKey::new("nested-driver").unwrap(),
            control: control.clone(),
            outlet_views: vec![layout.clone()],
            prior_content: None,
        })
        .unwrap();
    let mut routes = RouteTree::builder()
        .root_with_options(
            RoutePattern::pathless(),
            RouteNodeOptions::new().with_view(layout.clone()),
        )
        .unwrap();
    let layout_route = routes.root_id().unwrap();
    let leaf_route = routes
        .route_with_options(
            layout_route,
            RoutePattern::path("items/:id").unwrap(),
            RouteNodeOptions::new()
                .with_view(leaf.clone())
                .with_slot(RouteSlotKey::default()),
        )
        .unwrap();
    let routes = routes.build().unwrap();
    let mut views = RouteViewRegistry::new();
    for view in [layout.clone(), leaf.clone(), fallback.clone()] {
        views
            .insert(view, RouteViewDescriptor::new(driver.clone()))
            .unwrap();
    }
    let router = app
        .add_ui_router(RouterConfig::new(routes, views, fallback))
        .unwrap();

    let host = app
        .world_mut()
        .spawn((
            Node {
                width: Val::Px(640.0),
                height: Val::Px(360.0),
                ..Node::default()
            },
            HostSentinel(7),
            HostMetadata(73),
            RouterHostRegistration::new(router),
        ))
        .id();
    let existing_child = app
        .world_mut()
        .spawn((
            Node {
                width: Val::Px(13.0),
                height: Val::Px(17.0),
                ..Node::default()
            },
            ExistingChild(11),
            ChildOf(host),
        ))
        .id();
    let existing_sibling = app
        .world_mut()
        .spawn((
            Node {
                width: Val::Px(19.0),
                height: Val::Px(29.0),
                ..Node::default()
            },
            ExistingSibling(17),
            ChildOf(host),
        ))
        .id();
    let existing_children = vec![existing_child, existing_sibling];
    let mut host_fingerprint = capture_host_boundary(&app, host, &existing_children);
    app.update();
    host_fingerprint.top_slot = app
        .world()
        .get::<Children>(host)
        .unwrap()
        .iter()
        .copied()
        .find(|child| !existing_children.contains(child));

    NestedFixture {
        app,
        router,
        control,
        layout,
        leaf,
        layout_route,
        leaf_route,
        host_fingerprint,
    }
}

fn component_manifest(app: &App, entity: Entity) -> Vec<ComponentId> {
    app.world()
        .get_entity(entity)
        .expect("fingerprinted app-owned Entity must exist")
        .archetype()
        .components()
        .to_vec()
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct WorldComponentFingerprint {
    entities: Vec<(Entity, Vec<ComponentId>, Option<Entity>, Vec<Entity>)>,
}

fn world_component_fingerprint(app: &App) -> WorldComponentFingerprint {
    let mut entities = app
        .world()
        .iter_entities()
        .map(|entity| {
            (
                entity.id(),
                entity.archetype().components().to_vec(),
                entity.get::<ChildOf>().map(ChildOf::parent),
                entity
                    .get::<Children>()
                    .map(|children| children.iter().copied().collect())
                    .unwrap_or_default(),
            )
        })
        .collect::<Vec<_>>();
    entities.sort_by_key(|(entity, _, _, _)| entity.to_bits());
    WorldComponentFingerprint { entities }
}

fn install_matrix_baseline(app: &mut App, label: &str) -> RouterHandle {
    let driver = app
        .register_ui_view_driver(InstallMatrixDriver {
            key: ViewDriverKey::new(format!("{label}-baseline-driver")).unwrap(),
        })
        .unwrap();
    let view = RouteViewKey::new(format!("{label}-baseline-view")).unwrap();
    let routes = RouteTree::builder()
        .root_with_options(
            RoutePattern::path("baseline").unwrap(),
            RouteNodeOptions::new().with_view(view.clone()),
        )
        .unwrap()
        .build()
        .unwrap();
    let mut views = RouteViewRegistry::new();
    views
        .insert(view.clone(), RouteViewDescriptor::new(driver))
        .unwrap();
    app.add_ui_router(RouterConfig::new(routes, views, view))
        .unwrap()
}

#[derive(Clone, Copy)]
enum InstallMatrixCase {
    Valid,
    MissingError,
    MissingNormalPrecedence,
    UnknownNormal,
    UnknownError,
    UnregisteredNormal,
    UnregisteredError,
    UnregisteredFallback,
    MissingFallback,
}

fn recovery_install_config(app: &mut App, label: &str, case: InstallMatrixCase) -> RouterConfig {
    let registered_driver = app
        .register_ui_view_driver(InstallMatrixDriver {
            key: ViewDriverKey::new(format!("{label}-registered-driver")).unwrap(),
        })
        .unwrap();
    let absent_driver = ViewDriverKey::new(format!("{label}-absent-driver")).unwrap();
    let normal = RouteViewKey::new(format!("{label}-normal")).unwrap();
    let error = RouteViewKey::new(format!("{label}-error")).unwrap();
    let fallback = RouteViewKey::new(format!("{label}-fallback")).unwrap();
    let boundary = ErrorBoundaryKey::named(format!("{label}-boundary")).unwrap();

    let options = match case {
        InstallMatrixCase::MissingNormalPrecedence => {
            RouteNodeOptions::new().with_error_boundary(boundary)
        }
        InstallMatrixCase::MissingError => RouteNodeOptions::new()
            .with_view(normal.clone())
            .with_error_boundary(boundary),
        _ => RouteNodeOptions::new()
            .with_view(normal.clone())
            .with_error_view(boundary, error.clone()),
    };
    let routes = RouteTree::builder()
        .root_with_options(RoutePattern::path("candidate").unwrap(), options)
        .unwrap()
        .build()
        .unwrap();
    let mut views = RouteViewRegistry::new();
    if !matches!(case, InstallMatrixCase::UnknownNormal) {
        let driver = if matches!(case, InstallMatrixCase::UnregisteredNormal) {
            absent_driver.clone()
        } else {
            registered_driver.clone()
        };
        views
            .insert(normal, RouteViewDescriptor::new(driver))
            .unwrap();
    }
    if !matches!(
        case,
        InstallMatrixCase::MissingError
            | InstallMatrixCase::MissingNormalPrecedence
            | InstallMatrixCase::UnknownError
    ) {
        let driver = if matches!(case, InstallMatrixCase::UnregisteredError) {
            absent_driver.clone()
        } else {
            registered_driver.clone()
        };
        views
            .insert(error, RouteViewDescriptor::new(driver))
            .unwrap();
    }
    if !matches!(case, InstallMatrixCase::MissingFallback) {
        let driver = if matches!(case, InstallMatrixCase::UnregisteredFallback) {
            absent_driver
        } else {
            registered_driver
        };
        views
            .insert(fallback.clone(), RouteViewDescriptor::new(driver))
            .unwrap();
    }
    RouterConfig::new(routes, views, fallback)
}

fn capture_host_boundary(
    app: &App,
    host: Entity,
    expected_children: &[Entity],
) -> HostBoundaryFingerprint {
    let children_component = app
        .world()
        .component_id::<Children>()
        .expect("the app-owned host has a Children relationship component");
    let host_non_children_components = component_manifest(app, host)
        .into_iter()
        .filter(|component| *component != children_component)
        .collect::<Vec<_>>();
    let existing_children = app
        .world()
        .get::<Children>(host)
        .expect("the app-owned host has its pre-existing children")
        .iter()
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(existing_children, expected_children);

    HostBoundaryFingerprint {
        host,
        host_non_children_components,
        existing_children: existing_children.clone(),
        existing_child_components: existing_children
            .into_iter()
            .map(|child| (child, component_manifest(app, child)))
            .collect(),
        top_slot: None,
    }
}

fn push_and_settle(
    app: &mut App,
    router: RouterHandle,
    location: RouteLocation,
    forbidden_rect_width: Option<u16>,
    leaving_entities: Option<&HashSet<Entity>>,
) -> NavigationRequestId {
    let request = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, location)
        .expect("fixture navigation should enqueue");

    for _ in 0..16 {
        app.update();
        if let Some(width) = forbidden_rect_width {
            assert!(
                node_entities_with_width(app, width).is_empty(),
                "a rejected partial ContentPlan must remain World-zero"
            );
        }
        let (committed, terminal) = {
            let snapshots = app.world().resource::<RouterSnapshots>();
            let RequestLookup::Found(snapshot) = snapshots.request(router, request) else {
                continue;
            };
            (
                snapshot
                    .trace()
                    .iter()
                    .any(|event| event.kind() == NavigationTraceEventKind::CommitPublished),
                matches!(
                    snapshot.status(),
                    RequestStatus::Succeeded | RequestStatus::Failed(_)
                ),
            )
        };
        if committed {
            if let Some(leaving_entities) = leaving_entities {
                let gate = app.world().resource::<RouterInputGate>();
                for entity in leaving_entities {
                    assert!(!gate.allows_pointer(*entity));
                    assert!(!gate.allows_keyboard(*entity));
                    assert!(!gate.allows_gamepad(*entity));
                }
            }
        }
        if terminal {
            return request;
        }
    }
    panic!("fixture request did not settle within 16 App::update calls")
}

fn replace_and_settle(
    app: &mut App,
    router: RouterHandle,
    location: RouteLocation,
    forbidden_rect_width: Option<u16>,
) -> NavigationRequestId {
    let request = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .replace(router, location)
        .expect("fixture replace should enqueue");
    for _ in 0..16 {
        app.update();
        if let Some(width) = forbidden_rect_width {
            assert!(
                node_entities_with_width(app, width).is_empty(),
                "a rejected replace ContentPlan must remain World-zero"
            );
        }
        let terminal = {
            let snapshots = app.world().resource::<RouterSnapshots>();
            matches!(
                snapshots.request(router, request),
                RequestLookup::Found(snapshot)
                    if matches!(
                        snapshot.status(),
                        RequestStatus::Succeeded | RequestStatus::Failed(_)
                    )
            )
        };
        if terminal {
            return request;
        }
    }
    panic!("fixture replace did not settle within 16 App::update calls")
}

fn drive_request_to_terminal(
    app: &mut App,
    router: RouterHandle,
    request: NavigationRequestId,
) -> NavigationRequestSnapshot {
    for _ in 0..16 {
        app.update();
        let snapshots = app.world().resource::<RouterSnapshots>();
        let RequestLookup::Found(snapshot) = snapshots.request(router, request) else {
            continue;
        };
        if matches!(
            snapshot.status(),
            RequestStatus::Succeeded | RequestStatus::Failed(_)
        ) {
            return snapshot.clone();
        }
    }
    panic!("fixture request did not settle within 16 App::update calls")
}

fn drive_r1_request_to_terminal_preserving_old_active(
    app: &mut App,
    router: RouterHandle,
    request: NavigationRequestId,
    old_active: Option<&ActivePresentation>,
) -> (NavigationRequestSnapshot, usize) {
    let mut precommit_observations = 0;
    for _ in 0..24 {
        app.update();
        let request_snapshot = match app
            .world()
            .resource::<RouterSnapshots>()
            .request(router, request)
        {
            RequestLookup::Found(snapshot) => snapshot.clone(),
            RequestLookup::NotRetained | RequestLookup::Unknown => continue,
        };
        let committed = request_snapshot
            .trace()
            .iter()
            .any(|event| event.kind() == NavigationTraceEventKind::CommitPublished);
        if !committed {
            if let Some(old_active) = old_active {
                assert_eq!(
                    router_snapshot(app, router).active(),
                    old_active,
                    "the previously published presentation remains visible before error commit"
                );
                precommit_observations += 1;
            }
        }
        if matches!(
            request_snapshot.status(),
            RequestStatus::Succeeded | RequestStatus::Failed(_)
        ) {
            return (request_snapshot, precommit_observations);
        }
    }
    let snapshot = router_snapshot(app, router);
    let request = match app
        .world()
        .resource::<RouterSnapshots>()
        .request(router, request)
    {
        RequestLookup::Found(request) => Some(request.clone()),
        _ => None,
    };
    panic!(
        "R1 request did not settle within 24 App::update calls: router={snapshot:?} request={request:?}"
    )
}

fn request_snapshot(
    app: &App,
    router: RouterHandle,
    request: NavigationRequestId,
) -> NavigationRequestSnapshot {
    let snapshots = app.world().resource::<RouterSnapshots>();
    let RequestLookup::Found(snapshot) = snapshots.request(router, request) else {
        panic!("completed fixture request must remain retained")
    };
    snapshot.clone()
}

fn router_snapshot(app: &App, router: RouterHandle) -> RouterSnapshot {
    app.world()
        .resource::<RouterSnapshots>()
        .get(router)
        .expect("installed router must have a snapshot")
        .clone()
}

fn assert_succeeded(snapshot: &NavigationRequestSnapshot) {
    assert_eq!(snapshot.status(), &RequestStatus::Succeeded);
    assert_eq!(
        snapshot.trace().last().map(NavigationTraceEvent::kind),
        Some(NavigationTraceEventKind::RequestTerminal)
    );
}

fn assert_failed(snapshot: &NavigationRequestSnapshot, expected: NavigationFailureKind) {
    assert!(
        matches!(
            snapshot.status(),
            RequestStatus::Failed(failure) if failure.kind() == expected
        ),
        "expected {expected:?}, got {:?}: {:?}",
        snapshot.status(),
        snapshot.trace()
    );
    assert_eq!(
        snapshot.trace().last().map(NavigationTraceEvent::kind),
        Some(NavigationTraceEventKind::RequestTerminal)
    );
}

fn assert_accepted_report_diagnostic(
    snapshot: &NavigationRequestSnapshot,
    request: NavigationRequestId,
) {
    assert!(snapshot.diagnostics().iter().any(|event| {
        event.request() == Some(request)
            && event.frame().is_some()
            && event.report_disposition() == Some(ReportDisposition::Accepted)
            && !event.redacted_message().is_empty()
    }));
}

fn assert_ready_host_snapshot(snapshot: &RouterSnapshot) {
    let host = snapshot.host();
    assert!(host.version().is_some());
    assert!(host.top_slot().is_some());
    assert_eq!(
        host.trace()
            .iter()
            .map(HostTraceEvent::kind)
            .collect::<Vec<_>>(),
        vec![
            HostTraceEventKind::AdmissionRequested,
            HostTraceEventKind::HostGranted,
            HostTraceEventKind::TopSlotRegistered,
            HostTraceEventKind::HostReady,
        ]
    );
}

fn assert_cleanup_trace_is_ordered(snapshot: &NavigationRequestSnapshot, expect_receipts: bool) {
    let trace = snapshot.trace();
    let commit = trace
        .iter()
        .position(|event| event.kind() == NavigationTraceEventKind::CommitPublished)
        .unwrap_or_else(|| panic!("commit must be traced: {trace:?}"));
    let cleanup_phase = trace
        .iter()
        .enumerate()
        .skip(commit + 1)
        .find_map(|(index, event)| {
            (event.kind() == NavigationTraceEventKind::RequestPhaseChanged
                && event.phase() == Some(&NavigationPhase::PostCommitCleanup))
            .then_some(index)
        })
        .expect("post-commit cleanup phase must be traced");
    let aggregate = trace
        .iter()
        .enumerate()
        .skip(cleanup_phase + 1)
        .find_map(|(index, event)| {
            (event.kind() == NavigationTraceEventKind::CleanupSettled
                && event.cleanup_disposition().is_none())
            .then_some(index)
        })
        .expect("aggregate cleanup settlement must be traced");
    let receipts = trace
        .iter()
        .enumerate()
        .filter_map(|(index, event)| {
            (event.kind() == NavigationTraceEventKind::CleanupSettled
                && event.cleanup_disposition().is_some())
            .then_some(index)
        })
        .collect::<Vec<_>>();
    let terminal = trace
        .iter()
        .position(|event| event.kind() == NavigationTraceEventKind::RequestTerminal)
        .expect("terminal transition must be traced");

    assert!(commit < cleanup_phase);
    assert!(
        receipts
            .iter()
            .all(|receipt| cleanup_phase < *receipt && *receipt < aggregate)
    );
    assert_eq!(!receipts.is_empty(), expect_receipts);
    assert!(cleanup_phase < aggregate && aggregate < terminal);
    assert_eq!(terminal + 1, trace.len());
}

fn assert_terminal_owner_closure(app: &App, router: RouterHandle) {
    let snapshot = router_snapshot(app, router);
    assert!(snapshot.running_request().is_none());
    assert!(snapshot.pending_phase().is_none());
    assert!(!snapshot.cleanup().foreground_pending());
    assert_eq!(snapshot.cleanup().quarantine_count(), 0);
    #[cfg(feature = "test-support")]
    {
        let audit = app
            .world()
            .resource::<RouterAuthorityAudits>()
            .get(router)
            .expect("terminal router has an authority audit");
        assert_eq!(audit.frames().len(), snapshot.active().frames().len());
        for (mapping, active) in audit.frames().iter().zip(snapshot.active().frames()) {
            assert_eq!(mapping.frame(), active);
            assert_eq!(mapping.lifecycle(), AuthorityFrameLifecycle::Active);
        }
        assert!(audit.orphan_slots().is_empty());
    }
}

fn assert_failed_route_trace_settles_before_fallback(
    snapshot: &NavigationRequestSnapshot,
    expected: NavigationFailureKind,
    expect_failed_cleanup: bool,
) {
    assert_failed(snapshot, expected);
    let trace = snapshot.trace();
    let mut expected_kinds = vec![
        NavigationTraceEventKind::FrameTaskIssued,
        NavigationTraceEventKind::RequestPhaseChanged,
    ];
    if expect_failed_cleanup {
        expected_kinds.extend([
            NavigationTraceEventKind::RequestPhaseChanged,
            NavigationTraceEventKind::CleanupSettled,
        ]);
    }
    expected_kinds.extend([
        NavigationTraceEventKind::HistoryIntentRejected,
        NavigationTraceEventKind::FrameTaskIssued,
        NavigationTraceEventKind::RequestPhaseChanged,
        NavigationTraceEventKind::ShellPrepared,
        NavigationTraceEventKind::ViewApplied,
        NavigationTraceEventKind::RequestPhaseChanged,
        NavigationTraceEventKind::FrameActivated,
        NavigationTraceEventKind::CommitPublished,
        NavigationTraceEventKind::RequestPhaseChanged,
        NavigationTraceEventKind::CleanupSettled,
        NavigationTraceEventKind::CleanupSettled,
        NavigationTraceEventKind::CleanupSettled,
        NavigationTraceEventKind::CleanupSettled,
        NavigationTraceEventKind::CleanupSettled,
        NavigationTraceEventKind::RequestTerminal,
    ]);
    assert_eq!(
        trace
            .iter()
            .map(NavigationTraceEvent::kind)
            .collect::<Vec<_>>(),
        expected_kinds,
        "failed route trace must contain only the canonical rejected attempt, fallback, and settlement events"
    );
    let fallback_index = if expect_failed_cleanup { 5 } else { 3 };
    assert!(
        trace[0]
            .frame()
            .is_some_and(|frame| matches!(frame.origin(), FrameOrigin::MatchedRoute(_)))
    );
    let fallback_frame = trace[fallback_index]
        .frame()
        .expect("failed route fallback must issue one frame");
    assert_eq!(fallback_frame.origin(), &FrameOrigin::RouterFallback);
    for (index, event) in trace.iter().enumerate().skip(1) {
        let expected_frame = [
            fallback_index,
            fallback_index + 2,
            fallback_index + 3,
            fallback_index + 5,
        ]
        .contains(&index)
        .then_some(fallback_frame);
        assert_eq!(
            event.frame(),
            expected_frame,
            "trace event {index} has the canonical frame projection"
        );
    }
    assert_eq!(trace[1].phase(), Some(&NavigationPhase::ApplyingFrame(0)));
    if expect_failed_cleanup {
        assert_eq!(trace[2].phase(), Some(&NavigationPhase::CleanupPending));
    }
    for (index, event) in trace.iter().enumerate() {
        let expected_phase = if index == 1 {
            Some(&NavigationPhase::ApplyingFrame(0))
        } else if expect_failed_cleanup && index == 2 {
            Some(&NavigationPhase::CleanupPending)
        } else if index == fallback_index + 1 {
            Some(&NavigationPhase::Recovering)
        } else if index == fallback_index + 4 {
            Some(&NavigationPhase::ReadyToSwitch)
        } else if index == fallback_index + 7 {
            Some(&NavigationPhase::PostCommitCleanup)
        } else {
            None
        };
        assert_eq!(
            event.phase(),
            expected_phase,
            "trace event {index} has the canonical phase projection"
        );
    }
    for (index, event) in trace.iter().enumerate() {
        let expected_disposition = (fallback_index + 8..fallback_index + 12)
            .contains(&index)
            .then_some(CleanupDisposition::Removed);
        assert_eq!(
            event.cleanup_disposition(),
            expected_disposition,
            "trace event {index} has the canonical cleanup disposition"
        );
    }
    for (index, event) in trace.iter().enumerate() {
        assert_eq!(event.outlet(), None, "trace event {index} has no outlet");
        assert_eq!(event.slot(), None, "trace event {index} has no slot");
        assert_eq!(
            event.mount_rejection(),
            None,
            "trace event {index} has no mount rejection"
        );
    }
}

fn assert_succeeded_back_trace(snapshot: &NavigationRequestSnapshot) {
    assert_succeeded(snapshot);
    let trace = snapshot.trace();
    assert_eq!(
        trace
            .iter()
            .map(NavigationTraceEvent::kind)
            .collect::<Vec<_>>(),
        vec![
            NavigationTraceEventKind::FrameTaskIssued,
            NavigationTraceEventKind::RequestPhaseChanged,
            NavigationTraceEventKind::ShellPrepared,
            NavigationTraceEventKind::ViewApplied,
            NavigationTraceEventKind::RequestPhaseChanged,
            NavigationTraceEventKind::FrameActivated,
            NavigationTraceEventKind::CommitPublished,
            NavigationTraceEventKind::RequestPhaseChanged,
            NavigationTraceEventKind::CleanupSettled,
            NavigationTraceEventKind::CleanupSettled,
            NavigationTraceEventKind::CleanupSettled,
            NavigationTraceEventKind::CleanupSettled,
            NavigationTraceEventKind::CleanupSettled,
            NavigationTraceEventKind::RequestTerminal,
        ],
        "successful Back trace must contain only the canonical target and post-commit cleanup events"
    );
    let frame = trace[0]
        .frame()
        .expect("successful Back must issue one matched-route frame");
    assert!(matches!(frame.origin(), FrameOrigin::MatchedRoute(_)));
    for index in [0, 2, 3, 5] {
        assert_eq!(
            trace[index].frame(),
            Some(frame),
            "successful Back trace event {index} must retain its exact matched frame"
        );
    }
    for index in [1, 4, 6, 7, 8, 9, 10, 11, 12, 13] {
        assert_eq!(
            trace[index].frame(),
            None,
            "trace event {index} has no frame"
        );
    }
    assert_eq!(trace[1].phase(), Some(&NavigationPhase::ApplyingFrame(0)));
    assert_eq!(trace[4].phase(), Some(&NavigationPhase::ReadyToSwitch));
    assert_eq!(trace[7].phase(), Some(&NavigationPhase::PostCommitCleanup));
    for index in [0, 2, 3, 5, 6, 8, 9, 10, 11, 12, 13] {
        assert_eq!(
            trace[index].phase(),
            None,
            "trace event {index} has no phase"
        );
    }
    for index in 8..12 {
        assert_eq!(
            trace[index].cleanup_disposition(),
            Some(CleanupDisposition::Removed),
            "successful Back cleanup receipt {index} must be Removed"
        );
    }
    for index in [0, 1, 2, 3, 4, 5, 6, 7, 12, 13] {
        assert_eq!(
            trace[index].cleanup_disposition(),
            None,
            "trace event {index} has no cleanup receipt disposition"
        );
    }
    for (index, event) in trace.iter().enumerate() {
        assert_eq!(event.outlet(), None, "trace event {index} has no outlet");
        assert_eq!(event.slot(), None, "trace event {index} has no slot");
        assert_eq!(
            event.mount_rejection(),
            None,
            "trace event {index} has no mount rejection"
        );
    }
}

fn assert_no_target_back_trace(snapshot: &NavigationRequestSnapshot) {
    assert_failed(snapshot, NavigationFailureKind::BackTargetUnavailable);
    assert_eq!(snapshot.requested(), None);
    assert_eq!(
        snapshot
            .trace()
            .iter()
            .map(NavigationTraceEvent::kind)
            .collect::<Vec<_>>(),
        vec![
            NavigationTraceEventKind::HistoryIntentRejected,
            NavigationTraceEventKind::RequestPhaseChanged,
            NavigationTraceEventKind::CleanupSettled,
            NavigationTraceEventKind::RequestTerminal,
        ],
        "the canonical no-target branch must publish only rejection, zero cleanup, and terminal"
    );
    assert_eq!(
        snapshot.trace()[1].phase(),
        Some(&NavigationPhase::CleanupPending)
    );
    for index in [0, 2, 3] {
        assert_eq!(snapshot.trace()[index].phase(), None);
    }
    assert_eq!(snapshot.trace()[2].cleanup_disposition(), None);
    for (index, event) in snapshot.trace().iter().enumerate() {
        assert_eq!(event.frame(), None, "trace event {index} has no frame");
        assert_eq!(event.outlet(), None, "trace event {index} has no outlet");
        assert_eq!(event.slot(), None, "trace event {index} has no slot");
        assert_eq!(
            event.mount_rejection(),
            None,
            "trace event {index} has no mount rejection"
        );
        assert_eq!(
            event.cleanup_disposition(),
            None,
            "trace event {index} has no cleanup receipt disposition"
        );
    }
}

fn assert_host_intact(app: &App, fingerprint: &HostBoundaryFingerprint) {
    let host = fingerprint.host;
    let existing_child = fingerprint.existing_children[0];
    let existing_sibling = fingerprint.existing_children[1];
    assert!(app.world().get_entity(host).is_ok());
    assert_eq!(
        app.world().get::<HostSentinel>(host),
        Some(&HostSentinel(7))
    );
    assert_eq!(
        app.world().get::<HostMetadata>(host),
        Some(&HostMetadata(73))
    );
    assert_eq!(
        app.world().get::<ExistingChild>(existing_child),
        Some(&ExistingChild(11))
    );
    assert_eq!(
        app.world().get::<ExistingSibling>(existing_sibling),
        Some(&ExistingSibling(17))
    );
    assert_eq!(
        app.world()
            .get::<ChildOf>(existing_child)
            .map(ChildOf::parent),
        Some(host)
    );
    assert_eq!(
        app.world()
            .get::<ChildOf>(existing_sibling)
            .map(ChildOf::parent),
        Some(host)
    );
    let children_component = app.world().component_id::<Children>().unwrap();
    let current_host_components = component_manifest(app, host)
        .into_iter()
        .filter(|component| *component != children_component)
        .collect::<Vec<_>>();
    assert_eq!(
        current_host_components, fingerprint.host_non_children_components,
        "production navigation must preserve every non-Children host component"
    );
    for (child, expected_components) in &fingerprint.existing_child_components {
        assert_eq!(
            component_manifest(app, *child),
            *expected_components,
            "production navigation must preserve each existing app child component set"
        );
    }
    let current_children = app
        .world()
        .get::<Children>(host)
        .unwrap()
        .iter()
        .copied()
        .collect::<Vec<_>>();
    let expected_children = fingerprint
        .existing_children
        .iter()
        .copied()
        .chain(fingerprint.top_slot)
        .collect::<Vec<_>>();
    assert_eq!(current_children, expected_children);
    let existing_set = fingerprint
        .existing_children
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    let current_existing_order = current_children
        .iter()
        .copied()
        .filter(|child| existing_set.contains(child))
        .collect::<Vec<_>>();
    assert_eq!(current_existing_order, fingerprint.existing_children);
    assert_eq!(
        current_children.len(),
        fingerprint.existing_children.len() + 1,
        "the exact router top-slot membership is the only allowed host child delta"
    );
    let host_node = app
        .world()
        .get::<Node>(host)
        .expect("host Node must remain");
    assert_eq!(host_node.width, Val::Px(640.0));
    assert_eq!(host_node.height, Val::Px(360.0));
    let child_node = app
        .world()
        .get::<Node>(existing_child)
        .expect("existing child Node must remain");
    assert_eq!(child_node.width, Val::Px(13.0));
    assert_eq!(child_node.height, Val::Px(17.0));
    let sibling_node = app
        .world()
        .get::<Node>(existing_sibling)
        .expect("existing sibling Node must remain");
    assert_eq!(sibling_node.width, Val::Px(19.0));
    assert_eq!(sibling_node.height, Val::Px(29.0));
}

fn observe_app_owned_inserts<T: Component>(
    app: &mut App,
    entities: &[Entity],
    writes: Arc<AtomicUsize>,
) {
    let entities = entities.iter().copied().collect::<HashSet<_>>();
    app.world_mut().add_observer(move |event: On<Insert, T>| {
        if entities.contains(&event.entity) {
            writes.fetch_add(1, Ordering::Relaxed);
        }
    });
}

fn assert_manifest_not_added_or_changed(app: &App, entity: Entity, last_run: Tick) {
    let world = app.world();
    let this_run = world.read_change_tick();
    let entity_ref = world.entity(entity);
    for component in entity_ref.archetype().components() {
        let ticks = entity_ref
            .get_change_ticks_by_id(*component)
            .expect("manifest component has change ticks");
        assert!(!ticks.is_added(last_run, this_run));
        assert!(!ticks.is_changed(last_run, this_run));
    }
}

fn entity_set(app: &App) -> HashSet<Entity> {
    app.world()
        .iter_entities()
        .map(|entity| entity.id())
        .collect()
}

fn node_entities_with_width(app: &mut App, width: u16) -> Vec<Entity> {
    let world = app.world_mut();
    let mut query = world.query::<(Entity, &Node)>();
    query
        .iter(world)
        .filter_map(|(entity, node)| (node.width == Val::Px(f32::from(width))).then_some(entity))
        .collect()
}

fn assert_real_solid_rect(app: &mut App, width: u16) {
    let entities = node_entities_with_width(app, width);
    assert_eq!(entities.len(), 1);
    assert_ne!(
        app.world()
            .get::<BackgroundColor>(entities[0])
            .expect("a realized SolidRect must carry BackgroundColor"),
        &BackgroundColor::default()
    );
    assert_ne!(
        app.world()
            .get::<Visibility>(entities[0])
            .expect("a realized SolidRect must carry Visibility"),
        &Visibility::Hidden
    );
    let pickable = app
        .world()
        .get::<Pickable>(entities[0])
        .expect("a realized SolidRect must carry Pickable");
    assert!(pickable.should_block_lower);
    assert!(pickable.is_hoverable);
    let gate = app.world().resource::<RouterInputGate>();
    assert!(gate.allows_pointer(entities[0]));
    assert!(gate.allows_keyboard(entities[0]));
    assert!(gate.allows_gamepad(entities[0]));
}

fn next_trace_event(
    trace: &[NavigationTraceEvent],
    after: Option<usize>,
    kind: NavigationTraceEventKind,
    predicate: impl Fn(&NavigationTraceEvent) -> bool,
) -> usize {
    trace
        .iter()
        .enumerate()
        .skip(after.map_or(0, |index| index + 1))
        .find_map(|(index, event)| (event.kind() == kind && predicate(event)).then_some(index))
        .unwrap_or_else(|| panic!("missing ordered {kind:?} event after {after:?}: {trace:?}"))
}

#[cfg(feature = "test-support")]
#[test]
fn recovery_r1_install_matrix_is_synchronous_and_world_zero() {
    {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let config =
            recovery_install_config(&mut app, "r1-install-valid", InstallMatrixCase::Valid);
        let world_before = world_component_fingerprint(&app);
        let router = app
            .add_ui_router(config)
            .expect("bound normal and error views with registered drivers install synchronously");
        assert!(
            app.world()
                .resource::<RouterSnapshots>()
                .get(router)
                .is_some()
        );
        assert_eq!(world_component_fingerprint(&app), world_before);
    }

    let rows = [
        (
            "missing-error",
            InstallMatrixCase::MissingError,
            RouterInstallErrorKind::MissingErrorView,
        ),
        (
            "missing-normal-precedence",
            InstallMatrixCase::MissingNormalPrecedence,
            RouterInstallErrorKind::MissingRouteView,
        ),
        (
            "unknown-normal",
            InstallMatrixCase::UnknownNormal,
            RouterInstallErrorKind::UnknownRouteView,
        ),
        (
            "unknown-error",
            InstallMatrixCase::UnknownError,
            RouterInstallErrorKind::UnknownRouteView,
        ),
        (
            "unregistered-normal",
            InstallMatrixCase::UnregisteredNormal,
            RouterInstallErrorKind::UnknownViewDriver,
        ),
        (
            "unregistered-error",
            InstallMatrixCase::UnregisteredError,
            RouterInstallErrorKind::UnknownViewDriver,
        ),
        (
            "unregistered-fallback",
            InstallMatrixCase::UnregisteredFallback,
            RouterInstallErrorKind::UnknownViewDriver,
        ),
        (
            "missing-fallback",
            InstallMatrixCase::MissingFallback,
            RouterInstallErrorKind::MissingFallbackView,
        ),
    ];

    for (label, case, expected) in rows {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let baseline = install_matrix_baseline(&mut app, label);
        let config = recovery_install_config(&mut app, label, case);
        let router_before = app
            .world()
            .resource::<RouterSnapshots>()
            .get(baseline)
            .expect("baseline router is synchronously visible")
            .clone();
        let requests_before =
            testing_back_request_inventory(&app.world().resource::<RouterNavigation>(), baseline)
                .expect("baseline router has a request inventory");
        let inventory_before = testing_router_request_inventory(
            app.world().resource::<RouterNavigation>(),
            app.world().resource::<RouterSnapshots>(),
        );
        let world_before = world_component_fingerprint(&app);
        let host_registrations_before = app
            .world()
            .iter_entities()
            .filter(|entity| entity.get::<RouterHostRegistration>().is_some())
            .count();

        let error = app
            .add_ui_router(config)
            .expect_err("invalid installation must fail synchronously without App::update");

        assert_eq!(error.kind(), expected, "install row {label}");
        assert_eq!(world_component_fingerprint(&app), world_before, "{label}");
        assert_eq!(
            app.world()
                .iter_entities()
                .filter(|entity| entity.get::<RouterHostRegistration>().is_some())
                .count(),
            host_registrations_before,
            "{label}"
        );
        assert_eq!(
            app.world().resource::<RouterSnapshots>().get(baseline),
            Some(&router_before),
            "{label}"
        );
        assert_eq!(
            testing_back_request_inventory(&app.world().resource::<RouterNavigation>(), baseline,),
            Some(requests_before),
            "{label}"
        );
        assert_eq!(
            testing_router_request_inventory(
                app.world().resource::<RouterNavigation>(),
                app.world().resource::<RouterSnapshots>(),
            ),
            inventory_before,
            "install row {label} must add neither a router nor a request to any inventory",
        );
    }
}

#[test]
fn history_replace_facade_replaces_empty_history() {
    let mut fixture = root_fixture("/items/:id");
    let request = fixture
        .app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .replace(fixture.router, RouteLocation::new("/items/2"))
        .expect("replace must enqueue");

    let terminal = drive_request_to_terminal(&mut fixture.app, fixture.router, request);
    assert_eq!(terminal.status(), &RequestStatus::Succeeded);
    assert_eq!(
        router_snapshot(&fixture.app, fixture.router)
            .history()
            .entries(),
        [RouteLocation::new("/items/2")]
    );
    assert_eq!(
        router_snapshot(&fixture.app, fixture.router)
            .history()
            .cursor(),
        Some(0)
    );
}

#[test]
fn history_c1_replace_then_back_restores_prior_route() {
    let mut fixture = nested_fixture();
    let route_a = RouteLocation::new("/items/1");
    let route_b = RouteLocation::new("/items/2");
    let route_b2 = RouteLocation::new("/items/3");

    let first = push_and_settle(
        &mut fixture.app,
        fixture.router,
        route_a.clone(),
        None,
        None,
    );
    assert_succeeded(&request_snapshot(&fixture.app, fixture.router, first));
    let second = push_and_settle(&mut fixture.app, fixture.router, route_b, None, None);
    assert_succeeded(&request_snapshot(&fixture.app, fixture.router, second));
    let replacement = replace_and_settle(&mut fixture.app, fixture.router, route_b2.clone(), None);
    let replacement_snapshot = request_snapshot(&fixture.app, fixture.router, replacement);
    assert_succeeded(&replacement_snapshot);
    assert_eq!(replacement_snapshot.requested(), Some(&route_b2));

    let before_back = router_snapshot(&fixture.app, fixture.router);
    let protected_layout = before_back.active().frames()[0].clone();
    let protected_child_slot = before_back.active().frames()[1].slot().clone();
    let back = fixture
        .app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .back(fixture.router)
        .expect("a published prior entry must enqueue Back");
    let terminal = drive_request_to_terminal(&mut fixture.app, fixture.router, back);

    assert_succeeded(&terminal);
    assert_eq!(terminal.requested(), Some(&route_a));
    assert_cleanup_trace_is_ordered(&terminal, true);
    let after = router_snapshot(&fixture.app, fixture.router);
    assert_eq!(after.active().location(), Some(&route_a));
    assert_eq!(after.history().entries(), [route_a.clone(), route_b2]);
    assert_eq!(after.history().cursor(), Some(0));
    assert_eq!(after.history().current(), Some(&route_a));
    assert_eq!(after.active().frames()[0], protected_layout);
    assert_eq!(after.active().frames()[1].slot(), protected_child_slot);
    assert_host_intact(&fixture.app, &fixture.host_fingerprint);
}

#[test]
fn history_c2_failed_back_preserves_cursor_and_resolved_target() {
    let mut fixture = root_fixture("/items/:id");
    let route_a = RouteLocation::new("/items/1");
    let route_b = RouteLocation::new("/items/2");

    let first = push_and_settle(
        &mut fixture.app,
        fixture.router,
        route_a.clone(),
        None,
        None,
    );
    let second = push_and_settle(
        &mut fixture.app,
        fixture.router,
        route_b.clone(),
        None,
        None,
    );
    assert_succeeded(&request_snapshot(&fixture.app, fixture.router, first));
    assert_succeeded(&request_snapshot(&fixture.app, fixture.router, second));
    let published_history = router_snapshot(&fixture.app, fixture.router)
        .history()
        .clone();

    fixture
        .control
        .set_mode(&fixture.normal, FixtureMode::BuildFailure);
    let back = fixture
        .app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .back(fixture.router)
        .expect("the published B entry has A as a Back target");
    let terminal = drive_request_to_terminal(&mut fixture.app, fixture.router, back);

    assert_failed_route_trace_settles_before_fallback(
        &terminal,
        NavigationFailureKind::Build,
        false,
    );
    assert_eq!(terminal.requested(), Some(&route_a));
    let after = router_snapshot(&fixture.app, fixture.router);
    assert_eq!(after.history(), &published_history);
    assert_eq!(after.history().entries(), [route_a.clone(), route_b]);
    assert_eq!(after.history().cursor(), Some(1));
    assert_eq!(after.active().kind(), ActivePresentationKind::Error);
    assert_eq!(after.active().requested(), Some(&route_a));
    assert_eq!(
        after.active().failure().map(NavigationFailure::kind),
        Some(NavigationFailureKind::Build)
    );
    assert_eq!(
        fixture.control.effects().last().unwrap().origin,
        FrameOrigin::RouterFallback
    );
    assert_terminal_owner_closure(&fixture.app, fixture.router);
    assert_host_intact(&fixture.app, &fixture.host_fingerprint);
}

#[test]
fn history_c3_first_success_makes_second_back_no_target() {
    let mut fixture = root_fixture("/items/:id");
    let route_a = RouteLocation::new("/items/1");
    let route_b = RouteLocation::new("/items/2");

    let first = push_and_settle(
        &mut fixture.app,
        fixture.router,
        route_a.clone(),
        None,
        None,
    );
    let second = push_and_settle(&mut fixture.app, fixture.router, route_b, None, None);
    assert_succeeded(&request_snapshot(&fixture.app, fixture.router, first));
    assert_succeeded(&request_snapshot(&fixture.app, fixture.router, second));

    let first_back = fixture
        .app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .back(fixture.router)
        .expect("the published B entry has a first Back target");
    let second_back = fixture
        .app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .back(fixture.router)
        .expect("the queued second Back is admitted before the first publishes");
    let first_terminal = drive_request_to_terminal(&mut fixture.app, fixture.router, first_back);
    let second_terminal = drive_request_to_terminal(&mut fixture.app, fixture.router, second_back);

    assert_succeeded_back_trace(&first_terminal);
    assert_eq!(first_terminal.requested(), Some(&route_a));
    assert_no_target_back_trace(&second_terminal);
    let after = router_snapshot(&fixture.app, fixture.router);
    assert_eq!(
        after.history().entries(),
        [route_a.clone(), RouteLocation::new("/items/2")]
    );
    assert_eq!(after.history().cursor(), Some(0));
    assert_eq!(after.active().location(), Some(&route_a));
    assert_eq!(
        fixture
            .control
            .effects()
            .iter()
            .filter(|effect| effect.origin == FrameOrigin::RouterFallback)
            .count(),
        0,
        "a no-target Back must not run the fallback driver"
    );
    assert_eq!(fixture.control.effects().len(), 3);
    assert_terminal_owner_closure(&fixture.app, fixture.router);
    assert_host_intact(&fixture.app, &fixture.host_fingerprint);
}

#[test]
fn history_c3_first_failure_retargets_second_back_to_same_entry() {
    let mut fixture = root_fixture("/items/:id");
    let route_a = RouteLocation::new("/items/1");
    let route_b = RouteLocation::new("/items/2");

    let first = push_and_settle(
        &mut fixture.app,
        fixture.router,
        route_a.clone(),
        None,
        None,
    );
    let second = push_and_settle(
        &mut fixture.app,
        fixture.router,
        route_b.clone(),
        None,
        None,
    );
    assert_succeeded(&request_snapshot(&fixture.app, fixture.router, first));
    assert_succeeded(&request_snapshot(&fixture.app, fixture.router, second));
    let published_history = router_snapshot(&fixture.app, fixture.router)
        .history()
        .clone();

    fixture
        .control
        .set_one_shot_mode(&fixture.normal, FixtureMode::BuildFailure);
    let first_back = fixture
        .app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .back(fixture.router)
        .expect("the published B entry has a first Back target");
    let second_back = fixture
        .app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .back(fixture.router)
        .expect("the queued second Back is admitted before the first fails");
    let first_terminal = drive_request_to_terminal(&mut fixture.app, fixture.router, first_back);

    assert_failed_route_trace_settles_before_fallback(
        &first_terminal,
        NavigationFailureKind::Build,
        false,
    );
    assert_eq!(first_terminal.requested(), Some(&route_a));
    let after_first_failure = router_snapshot(&fixture.app, fixture.router);
    assert_eq!(after_first_failure.history(), &published_history);
    assert_eq!(after_first_failure.history().cursor(), Some(1));

    let second_terminal = drive_request_to_terminal(&mut fixture.app, fixture.router, second_back);

    assert_succeeded_back_trace(&second_terminal);
    assert_eq!(second_terminal.requested(), Some(&route_a));
    let after = router_snapshot(&fixture.app, fixture.router);
    assert_eq!(after.history().entries(), [route_a.clone(), route_b]);
    assert_eq!(after.history().cursor(), Some(0));
    assert_eq!(after.active().location(), Some(&route_a));
    assert_eq!(
        fixture
            .control
            .effects()
            .iter()
            .filter(|effect| effect.origin == FrameOrigin::RouterFallback)
            .count(),
        1,
        "only the first failed Back may run fallback"
    );
    assert_terminal_owner_closure(&fixture.app, fixture.router);
    assert_host_intact(&fixture.app, &fixture.host_fingerprint);
}

#[test]
fn history_back_from_error_presentation_uses_published_history() {
    let mut fixture = root_fixture("/items/:id");
    let route_a = RouteLocation::new("/items/1");
    let route_b = RouteLocation::new("/items/2");

    let first = push_and_settle(
        &mut fixture.app,
        fixture.router,
        route_a.clone(),
        None,
        None,
    );
    let second = push_and_settle(
        &mut fixture.app,
        fixture.router,
        route_b.clone(),
        None,
        None,
    );
    assert_succeeded(&request_snapshot(&fixture.app, fixture.router, first));
    assert_succeeded(&request_snapshot(&fixture.app, fixture.router, second));
    let published_history = router_snapshot(&fixture.app, fixture.router)
        .history()
        .clone();

    fixture
        .control
        .set_mode(&fixture.normal, FixtureMode::BuildFailure);
    let failed_push = push_and_settle(
        &mut fixture.app,
        fixture.router,
        RouteLocation::new("/items/3"),
        None,
        None,
    );
    let failed_push = request_snapshot(&fixture.app, fixture.router, failed_push);
    assert_failed_route_trace_settles_before_fallback(
        &failed_push,
        NavigationFailureKind::Build,
        true,
    );
    let error = router_snapshot(&fixture.app, fixture.router);
    assert_eq!(error.active().kind(), ActivePresentationKind::Error);
    assert_eq!(error.history(), &published_history);

    fixture.control.set_mode(
        &fixture.normal,
        FixtureMode::Success {
            width: NORMAL_RECT_WIDTH,
        },
    );
    let back = fixture
        .app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .back(fixture.router)
        .expect("published history B must admit Back while Error is presented");
    let terminal = drive_request_to_terminal(&mut fixture.app, fixture.router, back);

    assert_succeeded(&terminal);
    assert_eq!(terminal.requested(), Some(&route_a));
    assert_cleanup_trace_is_ordered(&terminal, true);
    let after = router_snapshot(&fixture.app, fixture.router);
    assert_eq!(after.active().kind(), ActivePresentationKind::Route);
    assert_eq!(after.active().location(), Some(&route_a));
    assert_eq!(after.history().entries(), [route_a.clone(), route_b]);
    assert_eq!(after.history().cursor(), Some(0));
    assert_terminal_owner_closure(&fixture.app, fixture.router);
    assert_host_intact(&fixture.app, &fixture.host_fingerprint);
}

#[test]
fn history_replace_f1_equivalent_is_physical_zero_write() {
    let mut fixture = root_fixture("/items/:id");
    let target = RouteLocation::new("/items/1");
    let first = push_and_settle(&mut fixture.app, fixture.router, target.clone(), None, None);
    assert_succeeded(&request_snapshot(&fixture.app, fixture.router, first));
    let before = router_snapshot(&fixture.app, fixture.router);
    let frames_before = before.active().frames().to_vec();
    let entities_before = entity_set(&fixture.app);
    let effects_before = fixture.control.effects();
    #[cfg(feature = "test-support")]
    let authority_before = fixture
        .app
        .world()
        .resource::<RouterAuthorityAudits>()
        .get(fixture.router)
        .expect("published router has one authority audit")
        .clone();

    let replacement = replace_and_settle(&mut fixture.app, fixture.router, target.clone(), None);
    let request = request_snapshot(&fixture.app, fixture.router, replacement);
    assert_succeeded(&request);
    assert_cleanup_trace_is_ordered(&request, false);
    assert_eq!(
        request
            .trace()
            .iter()
            .filter(|event| event.kind() == NavigationTraceEventKind::CommitPublished)
            .count(),
        1
    );

    let after = router_snapshot(&fixture.app, fixture.router);
    assert_eq!(after.active().frames(), frames_before);
    assert_eq!(after.history().entries(), [target]);
    assert_eq!(after.history().cursor(), Some(0));
    assert_eq!(entity_set(&fixture.app), entities_before);
    assert_eq!(fixture.control.effects(), effects_before);
    #[cfg(feature = "test-support")]
    assert_eq!(
        fixture
            .app
            .world()
            .resource::<RouterAuthorityAudits>()
            .get(fixture.router)
            .expect("equivalent replace retains one authority audit"),
        &authority_before
    );
    assert_host_intact(&fixture.app, &fixture.host_fingerprint);
}

#[test]
fn history_replace_f2_fragment_remounts_only_deepest_leaf() {
    let mut fixture = nested_fixture();
    let first_location = RouteLocation::new("/items/1");
    let first = push_and_settle(&mut fixture.app, fixture.router, first_location, None, None);
    assert_succeeded(&request_snapshot(&fixture.app, fixture.router, first));
    let before = router_snapshot(&fixture.app, fixture.router);
    let layout = before.active().frames()[0].clone();
    let leaf = before.active().frames()[1].clone();
    let effects_before = fixture.control.effects().len();

    let target = RouteLocation::new("/items/1").with_fragment("details");
    let replacement = replace_and_settle(&mut fixture.app, fixture.router, target.clone(), None);
    let request = request_snapshot(&fixture.app, fixture.router, replacement);
    assert_succeeded(&request);
    assert_cleanup_trace_is_ordered(&request, true);

    let after = router_snapshot(&fixture.app, fixture.router);
    let frames = after.active().frames();
    assert_eq!(frames[0], layout);
    assert_eq!(frames[1].slot(), leaf.slot());
    assert_eq!(frames[1].frame(), leaf.frame());
    assert_eq!(frames[1].generation().raw(), leaf.generation().raw() + 1);
    assert_eq!(after.history().entries(), [target.clone()]);
    assert_eq!(after.history().cursor(), Some(0));
    let effects = fixture.control.effects();
    assert_eq!(effects.len(), effects_before + 1);
    assert_eq!(effects.last().unwrap().view, fixture.leaf);
    assert_eq!(effects.last().unwrap().location, target);
    assert_host_intact(&fixture.app, &fixture.host_fingerprint);
}

#[test]
fn history_replace_f3_canonical_path_preserves_search_and_fragment() {
    let mut fixture = root_fixture("/items/:id");
    let target = RouteLocation::new("items//2/")
        .with_search("?tag=a&tag=b")
        .with_fragment("#details");
    let replacement = replace_and_settle(&mut fixture.app, fixture.router, target.clone(), None);
    let request = request_snapshot(&fixture.app, fixture.router, replacement);
    assert_succeeded(&request);
    assert_eq!(request.requested(), Some(&target));
    let requested = request
        .requested()
        .expect("a replace request retains its canonical target");
    assert_eq!(requested.path(), "/items/2");
    assert_eq!(requested.search(), Some("?tag=a&tag=b"));
    assert_eq!(requested.fragment(), Some("#details"));
    let after = router_snapshot(&fixture.app, fixture.router);
    assert_eq!(after.active().location(), Some(&target));
    assert_eq!(after.history().current(), Some(&target));
}

#[test]
fn history_replace_failure_preserves_published_history() {
    let mut fixture = root_fixture("/known/:id");
    let first = push_and_settle(
        &mut fixture.app,
        fixture.router,
        RouteLocation::new("/known/one"),
        None,
        None,
    );
    assert_succeeded(&request_snapshot(&fixture.app, fixture.router, first));
    let before = router_snapshot(&fixture.app, fixture.router);
    let old_active = before.active().clone();
    let old_history = before.history().clone();
    let old_entities = entity_set(&fixture.app);

    fixture
        .control
        .set_mode(&fixture.normal, FixtureMode::ApplyFailure);
    fixture
        .control
        .set_mode(&fixture.fallback, FixtureMode::BuildFailure);
    let failed = replace_and_settle(
        &mut fixture.app,
        fixture.router,
        RouteLocation::new("/known/two"),
        Some(PARTIAL_RECT_WIDTH),
    );
    let request = request_snapshot(&fixture.app, fixture.router, failed);
    assert_failed(&request, NavigationFailureKind::RecoveryFailed);
    assert!(
        request
            .trace()
            .iter()
            .any(|event| event.kind() == NavigationTraceEventKind::HistoryIntentRejected)
    );
    assert_eq!(
        fixture.control.effects().last().unwrap().origin,
        FrameOrigin::RouterFallback
    );
    let after = router_snapshot(&fixture.app, fixture.router);
    assert_eq!(after.active(), &old_active);
    assert_eq!(after.history(), &old_history);
    assert!(
        !after
            .history()
            .entries()
            .contains(&RouteLocation::new("/known/two"))
    );
    assert_eq!(entity_set(&fixture.app), old_entities);
    assert_host_intact(&fixture.app, &fixture.host_fingerprint);
}

#[test]
fn nested_push_journey_a() {
    let mut fixture = nested_fixture();
    let request_id = push_and_settle(
        &mut fixture.app,
        fixture.router,
        RouteLocation::new("/items/1"),
        None,
        None,
    );
    let request = request_snapshot(&fixture.app, fixture.router, request_id);
    let router = router_snapshot(&fixture.app, fixture.router);
    let effects = fixture.control.effects();

    assert_eq!(
        effects
            .iter()
            .map(|effect| effect.view.clone())
            .collect::<Vec<_>>(),
        vec![fixture.layout.clone(), fixture.leaf.clone()]
    );
    assert_eq!(effects[0].location, RouteLocation::new("/items/1"));
    assert_eq!(effects[0].route, Some(fixture.layout_route));
    assert_eq!(effects[0].id_param, None);
    assert_eq!(effects[0].fragment, None);
    assert_eq!(
        effects[0].origin,
        FrameOrigin::MatchedRoute(fixture.layout_route)
    );
    assert_eq!(effects[1].location, RouteLocation::new("/items/1"));
    assert_eq!(effects[1].route, Some(fixture.leaf_route));
    assert_eq!(effects[1].id_param.as_deref(), Some("1"));
    assert_eq!(effects[1].fragment, None);
    assert_eq!(
        effects[1].origin,
        FrameOrigin::MatchedRoute(fixture.leaf_route)
    );

    assert_eq!(router.active().frames().len(), 2);
    assert_eq!(
        router.history().entries(),
        &[RouteLocation::new("/items/1")]
    );
    assert_eq!(request.status(), &RequestStatus::Succeeded);
    assert!(!router.cleanup().foreground_pending());
    assert_host_intact(&fixture.app, &fixture.host_fingerprint);
    assert_real_solid_rect(&mut fixture.app, 41);
    assert_real_solid_rect(&mut fixture.app, 43);

    let parent = &router.active().frames()[0];
    let child = &router.active().frames()[1];
    assert_eq!(
        parent.origin(),
        &FrameOrigin::MatchedRoute(fixture.layout_route)
    );
    assert_eq!(
        child.origin(),
        &FrameOrigin::MatchedRoute(fixture.leaf_route)
    );
    let default_outlet = RouteSlotKey::default();
    let trace = request.trace();
    let mut cursor = None;
    cursor = Some(next_trace_event(
        trace,
        cursor,
        NavigationTraceEventKind::FrameTaskIssued,
        |event| event.frame() == Some(parent),
    ));
    cursor = Some(next_trace_event(
        trace,
        cursor,
        NavigationTraceEventKind::ShellPrepared,
        |event| event.frame() == Some(parent),
    ));
    cursor = Some(next_trace_event(
        trace,
        cursor,
        NavigationTraceEventKind::ViewApplied,
        |event| event.frame() == Some(parent),
    ));
    cursor = Some(next_trace_event(
        trace,
        cursor,
        NavigationTraceEventKind::OutletRegistered,
        |event| event.frame() == Some(parent) && event.outlet() == Some(&default_outlet),
    ));
    cursor = Some(next_trace_event(
        trace,
        cursor,
        NavigationTraceEventKind::MountRequested,
        |event| event.frame() == Some(parent) && event.outlet() == Some(&default_outlet),
    ));
    cursor = Some(next_trace_event(
        trace,
        cursor,
        NavigationTraceEventKind::MountResolved,
        |event| event.frame() == Some(parent) && event.outlet() == Some(&default_outlet),
    ));
    cursor = Some(next_trace_event(
        trace,
        cursor,
        NavigationTraceEventKind::MountGranted,
        |event| {
            event.frame() == Some(parent)
                && event.outlet() == Some(&default_outlet)
                && event.slot() == Some(child.slot())
        },
    ));
    cursor = Some(next_trace_event(
        trace,
        cursor,
        NavigationTraceEventKind::FrameTaskIssued,
        |event| event.frame() == Some(child),
    ));
    cursor = Some(next_trace_event(
        trace,
        cursor,
        NavigationTraceEventKind::ShellPrepared,
        |event| event.frame() == Some(child),
    ));
    cursor = Some(next_trace_event(
        trace,
        cursor,
        NavigationTraceEventKind::ViewApplied,
        |event| event.frame() == Some(child),
    ));
    cursor = Some(next_trace_event(
        trace,
        cursor,
        NavigationTraceEventKind::FrameActivated,
        |event| event.frame() == Some(parent),
    ));
    cursor = Some(next_trace_event(
        trace,
        cursor,
        NavigationTraceEventKind::FrameActivated,
        |event| event.frame() == Some(child),
    ));
    cursor = Some(next_trace_event(
        trace,
        cursor,
        NavigationTraceEventKind::CommitPublished,
        |_| true,
    ));
    cursor = Some(next_trace_event(
        trace,
        cursor,
        NavigationTraceEventKind::RequestPhaseChanged,
        |event| event.phase() == Some(&NavigationPhase::PostCommitCleanup),
    ));
    cursor = Some(next_trace_event(
        trace,
        cursor,
        NavigationTraceEventKind::CleanupSettled,
        |event| event.cleanup_disposition().is_none(),
    ));
    let terminal = next_trace_event(
        trace,
        cursor,
        NavigationTraceEventKind::RequestTerminal,
        |_| true,
    );
    assert_eq!(terminal + 1, trace.len());
}

#[test]
fn reconciliation_journey_b_preserves_layout_and_replaces_leaf() {
    let mut fixture = nested_fixture();
    let first_location = RouteLocation::new("/items/1");
    let first_request = push_and_settle(
        &mut fixture.app,
        fixture.router,
        first_location.clone(),
        None,
        None,
    );
    assert_succeeded(&request_snapshot(
        &fixture.app,
        fixture.router,
        first_request,
    ));
    let first_router = router_snapshot(&fixture.app, fixture.router);
    let first_frames = first_router.active().frames();
    assert_eq!(first_frames.len(), 2);
    let preserved_layout = first_frames[0].clone();
    let old_leaf = first_frames[1].clone();

    let host = fixture.host_fingerprint.host;
    let existing_child = fixture.host_fingerprint.existing_children[0];
    let existing_sibling = fixture.host_fingerprint.existing_children[1];
    let writes = Arc::new(AtomicUsize::new(0));
    observe_app_owned_inserts::<HostSentinel>(&mut fixture.app, &[host], writes.clone());
    observe_app_owned_inserts::<HostMetadata>(&mut fixture.app, &[host], writes.clone());
    observe_app_owned_inserts::<RouterHostRegistration>(&mut fixture.app, &[host], writes.clone());
    observe_app_owned_inserts::<Node>(
        &mut fixture.app,
        &[host, existing_child, existing_sibling],
        writes.clone(),
    );
    observe_app_owned_inserts::<Children>(&mut fixture.app, &[host], writes.clone());
    observe_app_owned_inserts::<ExistingChild>(&mut fixture.app, &[existing_child], writes.clone());
    observe_app_owned_inserts::<ExistingSibling>(
        &mut fixture.app,
        &[existing_sibling],
        writes.clone(),
    );
    observe_app_owned_inserts::<ChildOf>(
        &mut fixture.app,
        &[existing_child, existing_sibling],
        writes.clone(),
    );
    fixture.app.world_mut().clear_trackers();
    let app_owned_change_baseline = fixture.app.world().last_change_tick();

    let second_location = RouteLocation::new("/items/2");
    let second_request = push_and_settle(
        &mut fixture.app,
        fixture.router,
        second_location.clone(),
        None,
        None,
    );
    let second_request = request_snapshot(&fixture.app, fixture.router, second_request);
    assert_succeeded(&second_request);
    assert_cleanup_trace_is_ordered(&second_request, true);
    let receipts = second_request
        .trace()
        .iter()
        .enumerate()
        .filter_map(|(index, event)| {
            (event.kind() == NavigationTraceEventKind::CleanupSettled)
                .then(|| {
                    event
                        .cleanup_disposition()
                        .map(|disposition| (index, disposition))
                })
                .flatten()
        })
        .collect::<Vec<_>>();
    assert!(!receipts.is_empty());
    assert!(
        receipts
            .iter()
            .all(|(_, disposition)| *disposition == CleanupDisposition::Removed)
    );
    let terminal = second_request
        .trace()
        .iter()
        .position(|event| event.kind() == NavigationTraceEventKind::RequestTerminal)
        .expect("successful journey has a terminal event");
    assert!(receipts.iter().all(|(index, _)| *index < terminal));

    let second_router = router_snapshot(&fixture.app, fixture.router);
    let second_frames = second_router.active().frames();
    assert_eq!(second_frames.len(), 2);
    assert_eq!(second_frames[0], preserved_layout);
    assert_eq!(second_frames[1].slot(), old_leaf.slot());
    assert_ne!(second_frames[1].frame(), old_leaf.frame());
    assert_eq!(
        second_router.history().entries(),
        &[first_location, second_location]
    );
    assert_eq!(second_router.history().cursor(), Some(1));
    assert_host_intact(&fixture.app, &fixture.host_fingerprint);
    assert_eq!(writes.load(Ordering::Relaxed), 0);
    assert_manifest_not_added_or_changed(&fixture.app, host, app_owned_change_baseline);
    assert_manifest_not_added_or_changed(&fixture.app, existing_child, app_owned_change_baseline);
    assert_manifest_not_added_or_changed(&fixture.app, existing_sibling, app_owned_change_baseline);
}

#[test]
fn reconciliation_nested_fragment_preserves_ancestors_and_advances_leaf_generation() {
    let mut fixture = nested_fixture();
    let first_location = RouteLocation::new("/items/1");
    let first_request = push_and_settle(
        &mut fixture.app,
        fixture.router,
        first_location.clone(),
        None,
        None,
    );
    assert_succeeded(&request_snapshot(
        &fixture.app,
        fixture.router,
        first_request,
    ));
    let first_router = router_snapshot(&fixture.app, fixture.router);
    let first_frames = first_router.active().frames();
    assert_eq!(first_frames.len(), 2);
    let preserved_layout = first_frames[0].clone();
    let first_leaf = first_frames[1].clone();
    let effects_before_fragment = fixture.control.effects().len();

    let fragment_location = RouteLocation::new("/items/1").with_fragment("details");
    let fragment_request = push_and_settle(
        &mut fixture.app,
        fixture.router,
        fragment_location.clone(),
        None,
        None,
    );
    let fragment_request = request_snapshot(&fixture.app, fixture.router, fragment_request);
    assert_succeeded(&fragment_request);
    assert_cleanup_trace_is_ordered(&fragment_request, true);

    let fragment_router = router_snapshot(&fixture.app, fixture.router);
    let fragment_frames = fragment_router.active().frames();
    assert_eq!(fragment_frames.len(), 2);
    assert_eq!(fragment_frames[0], preserved_layout);
    assert_eq!(fragment_frames[1].slot(), first_leaf.slot());
    assert_eq!(fragment_frames[1].frame(), first_leaf.frame());
    assert_eq!(
        fragment_frames[1].generation().raw(),
        first_leaf.generation().raw() + 1
    );

    let effects = fixture.control.effects();
    assert_eq!(effects.len(), effects_before_fragment + 1);
    let fragment_effect = effects.last().expect("fragment applies one leaf effect");
    assert_eq!(fragment_effect.view, fixture.leaf);
    assert_eq!(fragment_effect.fragment.as_deref(), Some("details"));
    assert_eq!(fragment_effect.location, fragment_location);
    assert_eq!(
        fragment_router.history().entries(),
        &[first_location, fragment_location]
    );
    assert_eq!(fragment_router.history().cursor(), Some(1));
    assert_host_intact(&fixture.app, &fixture.host_fingerprint);
}

#[test]
fn root_push_first_equivalent_fragment_and_changed_root_settle_with_physical_cleanup() {
    let mut fixture = root_fixture("*page");
    assert_eq!(fixture.existing_children.len(), 2);

    let first_request_id = push_and_settle(
        &mut fixture.app,
        fixture.router,
        RouteLocation::new("/alpha"),
        None,
        None,
    );
    let first_request = request_snapshot(&fixture.app, fixture.router, first_request_id);
    assert_succeeded(&first_request);
    assert_accepted_report_diagnostic(&first_request, first_request_id);
    assert_cleanup_trace_is_ordered(&first_request, false);
    let first_router = router_snapshot(&fixture.app, fixture.router);
    assert_ready_host_snapshot(&first_router);
    assert!(!first_router.cleanup().foreground_pending());
    assert_eq!(first_router.cleanup().quarantine_count(), 0);
    assert_eq!(first_router.cleanup().last_disposition(), None);
    assert_eq!(first_router.active().kind(), ActivePresentationKind::Route);
    assert_eq!(
        first_router.active().location(),
        Some(&RouteLocation::new("/alpha"))
    );
    assert_eq!(first_router.history().len(), 1);
    assert_eq!(
        first_router.history().current(),
        Some(&RouteLocation::new("/alpha"))
    );
    assert_eq!(first_router.active().frames().len(), 1);
    let first_frame = first_router.active().frames()[0].clone();
    let first_entities = entity_set(&fixture.app)
        .difference(&fixture.admission_entities)
        .copied()
        .collect::<HashSet<_>>();
    assert!(!first_entities.is_empty());
    assert_real_solid_rect(&mut fixture.app, NORMAL_RECT_WIDTH);
    assert_host_intact(&fixture.app, &fixture.host_fingerprint);

    let effects_before_equivalent = fixture.control.effects().len();
    let entities_before_equivalent = entity_set(&fixture.app);
    let equivalent_request_id = push_and_settle(
        &mut fixture.app,
        fixture.router,
        RouteLocation::new("alpha//"),
        None,
        None,
    );
    let equivalent_request = request_snapshot(&fixture.app, fixture.router, equivalent_request_id);
    assert_succeeded(&equivalent_request);
    assert_cleanup_trace_is_ordered(&equivalent_request, false);
    let equivalent_router = router_snapshot(&fixture.app, fixture.router);
    assert_eq!(fixture.control.effects().len(), effects_before_equivalent);
    assert_eq!(entity_set(&fixture.app), entities_before_equivalent);
    assert_eq!(equivalent_router.active().frames(), &[first_frame.clone()]);
    assert_eq!(equivalent_router.history().len(), 2);

    let fragment_request_id = push_and_settle(
        &mut fixture.app,
        fixture.router,
        RouteLocation::new("/alpha").with_fragment("details"),
        None,
        Some(&first_entities),
    );
    let fragment_request = request_snapshot(&fixture.app, fixture.router, fragment_request_id);
    assert_succeeded(&fragment_request);
    assert_accepted_report_diagnostic(&fragment_request, fragment_request_id);
    assert_cleanup_trace_is_ordered(&fragment_request, true);
    let fragment_router = router_snapshot(&fixture.app, fixture.router);
    let fragment_frame = fragment_router.active().frames()[0].clone();
    assert_eq!(fragment_frame.frame(), first_frame.frame());
    assert_eq!(
        fragment_frame.generation().raw(),
        first_frame.generation().raw() + 1
    );
    assert_eq!(fragment_router.history().len(), 3);
    assert_eq!(
        fragment_router.history().current(),
        Some(&RouteLocation::new("/alpha").with_fragment("details"))
    );
    assert!(
        first_entities
            .iter()
            .all(|entity| fixture.app.world().get_entity(*entity).is_err()),
        "fragment remount must physically close the previous frame closure"
    );
    let fragment_entities = entity_set(&fixture.app)
        .difference(&fixture.admission_entities)
        .copied()
        .collect::<HashSet<_>>();

    let changed_request_id = push_and_settle(
        &mut fixture.app,
        fixture.router,
        RouteLocation::new("/beta"),
        None,
        Some(&fragment_entities),
    );
    let changed_request = request_snapshot(&fixture.app, fixture.router, changed_request_id);
    assert_succeeded(&changed_request);
    assert_accepted_report_diagnostic(&changed_request, changed_request_id);
    assert_cleanup_trace_is_ordered(&changed_request, true);
    let changed_router = router_snapshot(&fixture.app, fixture.router);
    let changed_frame = &changed_router.active().frames()[0];
    assert_ne!(changed_frame.frame(), fragment_frame.frame());
    assert_eq!(changed_frame.generation().raw(), 0);
    assert_eq!(changed_router.history().len(), 4);
    assert!(!changed_router.cleanup().foreground_pending());
    assert_eq!(changed_router.cleanup().quarantine_count(), 0);
    assert_eq!(
        changed_router.cleanup().last_disposition(),
        Some(CleanupDisposition::Removed)
    );
    assert_eq!(
        changed_router.history().current(),
        Some(&RouteLocation::new("/beta"))
    );
    assert!(
        fragment_entities
            .iter()
            .all(|entity| fixture.app.world().get_entity(*entity).is_err()),
        "changed-root replacement must physically close the old frame closure"
    );
    assert_real_solid_rect(&mut fixture.app, NORMAL_RECT_WIDTH);

    let effects = fixture.control.effects();
    assert_eq!(effects.len(), 3, "equivalent push must be driver-zero");
    assert_eq!(effects[0].location, RouteLocation::new("/alpha"));
    assert_eq!(effects[1].fragment.as_deref(), Some("details"));
    assert_eq!(effects[2].location, RouteLocation::new("/beta"));
    assert_host_intact(&fixture.app, &fixture.host_fingerprint);
}

#[test]
fn root_push_match_failure_commits_router_fallback_without_history() {
    let mut fixture = root_fixture("/known");
    let requested = RouteLocation::new("/missing");
    let request_id = push_and_settle(
        &mut fixture.app,
        fixture.router,
        requested.clone(),
        None,
        None,
    );
    let request = request_snapshot(&fixture.app, fixture.router, request_id);
    assert_failed(&request, NavigationFailureKind::Match);
    assert_accepted_report_diagnostic(&request, request_id);
    assert!(request.diagnostics().iter().any(|event| {
        event.severity() == DiagnosticSeverity::Error
            && event.request().is_none()
            && event.frame().is_none()
            && event.report_disposition().is_none()
            && !event.redacted_message().is_empty()
    }));
    assert_cleanup_trace_is_ordered(&request, false);
    assert!(request.matched().is_none());

    let router = router_snapshot(&fixture.app, fixture.router);
    assert_eq!(router.active().kind(), ActivePresentationKind::Error);
    assert_eq!(router.active().requested(), Some(&requested));
    assert!(router.active().matched().is_none());
    assert_eq!(
        router.active().failure().map(NavigationFailure::kind),
        Some(NavigationFailureKind::Match)
    );
    assert_eq!(router.active().frames().len(), 1);
    assert_eq!(
        router.active().frames()[0].origin(),
        &FrameOrigin::RouterFallback
    );
    assert_eq!(router.history().len(), 0);
    assert!(router.history().current().is_none());
    assert!(router.requested().is_none());
    assert!(router.matched().is_none());
    assert!(router.pending_phase().is_none());
    assert!(router.running_request().is_none());
    assert_real_solid_rect(&mut fixture.app, FALLBACK_RECT_WIDTH);

    let effects = fixture.control.effects();
    assert_eq!(effects.len(), 1);
    assert_eq!(effects[0].view, fixture.fallback);
    assert_eq!(effects[0].origin, FrameOrigin::RouterFallback);
    assert_eq!(effects[0].failure, Some(NavigationFailureKind::Match));
    assert_host_intact(&fixture.app, &fixture.host_fingerprint);
}

#[test]
fn root_push_build_and_apply_failures_preserve_match_and_commit_error_presentation() {
    for (mode, expected) in [
        (FixtureMode::BuildFailure, NavigationFailureKind::Build),
        (FixtureMode::ApplyFailure, NavigationFailureKind::Apply),
    ] {
        let mut fixture = root_fixture("/known/:id");
        let first = push_and_settle(
            &mut fixture.app,
            fixture.router,
            RouteLocation::new("/known/one"),
            None,
            None,
        );
        let first = request_snapshot(&fixture.app, fixture.router, first);
        assert_succeeded(&first);
        let old_matched = first
            .matched()
            .expect("the prior request must retain its matched fingerprint")
            .clone();
        let old_entities = entity_set(&fixture.app);
        fixture.control.set_mode(&fixture.normal, mode);

        let requested = RouteLocation::new("/known/two");
        let failed_id = push_and_settle(
            &mut fixture.app,
            fixture.router,
            requested.clone(),
            (mode == FixtureMode::ApplyFailure).then_some(PARTIAL_RECT_WIDTH),
            Some(&old_entities),
        );
        let failed = request_snapshot(&fixture.app, fixture.router, failed_id);
        assert_failed(&failed, expected);
        assert_accepted_report_diagnostic(&failed, failed_id);
        let matched = failed
            .matched()
            .expect("post-match failure must retain its fingerprint")
            .clone();
        assert_ne!(matched, old_matched);
        assert_cleanup_trace_is_ordered(&failed, true);

        let router = router_snapshot(&fixture.app, fixture.router);
        assert_eq!(router.active().kind(), ActivePresentationKind::Error);
        assert_eq!(router.active().requested(), Some(&requested));
        assert_eq!(router.active().matched(), Some(&matched));
        assert_eq!(
            router.active().failure().map(NavigationFailure::kind),
            Some(expected)
        );
        assert_eq!(
            router.active().frames()[0].origin(),
            &FrameOrigin::RouterFallback
        );
        assert_eq!(router.history().len(), 1);
        assert_eq!(
            router.history().current(),
            Some(&RouteLocation::new("/known/one"))
        );
        assert!(node_entities_with_width(&mut fixture.app, PARTIAL_RECT_WIDTH).is_empty());
        assert_real_solid_rect(&mut fixture.app, FALLBACK_RECT_WIDTH);

        let effects = fixture.control.effects();
        let recovery = effects.last().expect("fallback effect must run");
        assert_eq!(recovery.view, fixture.fallback);
        assert_eq!(recovery.origin, FrameOrigin::RouterFallback);
        assert_eq!(recovery.failure, Some(expected));
        assert_host_intact(&fixture.app, &fixture.host_fingerprint);
    }
}

#[test]
fn root_push_fallback_failure_preserves_old_state_and_does_not_poison_the_queue() {
    let mut fixture = root_fixture("/known/:id");
    let first = push_and_settle(
        &mut fixture.app,
        fixture.router,
        RouteLocation::new("/known/one"),
        None,
        None,
    );
    assert_succeeded(&request_snapshot(&fixture.app, fixture.router, first));
    let before = router_snapshot(&fixture.app, fixture.router);
    let old_active = before.active().clone();
    let old_history = before.history().clone();
    let old_entities = entity_set(&fixture.app);

    fixture
        .control
        .set_mode(&fixture.normal, FixtureMode::ApplyFailure);
    fixture
        .control
        .set_mode(&fixture.fallback, FixtureMode::BuildFailure);
    let failed_id = push_and_settle(
        &mut fixture.app,
        fixture.router,
        RouteLocation::new("/known/two"),
        Some(PARTIAL_RECT_WIDTH),
        None,
    );
    let failed = request_snapshot(&fixture.app, fixture.router, failed_id);
    assert_failed(&failed, NavigationFailureKind::RecoveryFailed);
    assert_accepted_report_diagnostic(&failed, failed_id);

    let after_failure = router_snapshot(&fixture.app, fixture.router);
    assert_eq!(after_failure.active(), &old_active);
    assert_eq!(after_failure.history(), &old_history);
    assert_eq!(entity_set(&fixture.app), old_entities);
    assert!(node_entities_with_width(&mut fixture.app, PARTIAL_RECT_WIDTH).is_empty());
    assert_real_solid_rect(&mut fixture.app, NORMAL_RECT_WIDTH);
    assert_host_intact(&fixture.app, &fixture.host_fingerprint);

    fixture.control.set_mode(
        &fixture.normal,
        FixtureMode::Success {
            width: NORMAL_RECT_WIDTH,
        },
    );
    fixture.control.set_mode(
        &fixture.fallback,
        FixtureMode::Success {
            width: FALLBACK_RECT_WIDTH,
        },
    );
    let recovered_id = push_and_settle(
        &mut fixture.app,
        fixture.router,
        RouteLocation::new("/known/two"),
        None,
        Some(&old_entities),
    );
    let recovered_request = request_snapshot(&fixture.app, fixture.router, recovered_id);
    assert_succeeded(&recovered_request);
    assert_accepted_report_diagnostic(&recovered_request, recovered_id);
    let recovered_router = router_snapshot(&fixture.app, fixture.router);
    assert_eq!(
        recovered_router.active().kind(),
        ActivePresentationKind::Route
    );
    assert_eq!(
        recovered_router.active().location(),
        Some(&RouteLocation::new("/known/two"))
    );
    assert_eq!(recovered_router.history().len(), 2);
    assert_host_intact(&fixture.app, &fixture.host_fingerprint);
}

#[test]
fn recovery_r1_journey_d_uses_bound_error_views_for_failure_matrix() {
    {
        let mut fixture = recovery_r1_fixture("r1-match");
        let first = push_and_settle(
            &mut fixture.app,
            fixture.router,
            RouteLocation::new("/section/items/1"),
            None,
            None,
        );
        assert_succeeded(&request_snapshot(&fixture.app, fixture.router, first));
        let before = router_snapshot(&fixture.app, fixture.router);
        let old_active = before.active().clone();
        let old_history = before.history().clone();
        let requested = RouteLocation::new("/does-not-match");
        let request = fixture
            .app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(fixture.router, requested.clone())
            .unwrap();
        let (failed, _observed) = drive_r1_request_to_terminal_preserving_old_active(
            &mut fixture.app,
            fixture.router,
            request,
            Some(&old_active),
        );
        assert_failed(&failed, NavigationFailureKind::Match);
        assert_eq!(failed.requested(), Some(&requested));
        assert!(failed.matched().is_none());
        let after = router_snapshot(&fixture.app, fixture.router);
        assert_eq!(after.history(), &old_history);
        assert_eq!(after.active().requested(), Some(&requested));
        assert!(after.active().matched().is_none());
        assert_eq!(
            after.active().failure().map(NavigationFailure::kind),
            Some(NavigationFailureKind::Match)
        );
        assert_eq!(after.active().frames().len(), 1);
        assert_eq!(
            after.active().frames()[0].origin(),
            &FrameOrigin::RouterFallback
        );
        assert_eq!(
            fixture.control.effects().last().unwrap().view,
            fixture.fallback
        );
        assert_terminal_owner_closure(&fixture.app, fixture.router);
        assert_host_intact(&fixture.app, &fixture.host_fingerprint);
    }

    let rows = [
        (
            "build",
            FixtureMode::BuildFailure,
            NavigationFailureKind::Build,
            false,
        ),
        (
            "apply-partial",
            FixtureMode::ApplyFailure,
            NavigationFailureKind::Apply,
            false,
        ),
        (
            "duplicate-outlet",
            FixtureMode::DuplicateOutlet,
            NavigationFailureKind::Apply,
            true,
        ),
        (
            "foreign-session",
            FixtureMode::ForeignSession,
            NavigationFailureKind::Apply,
            true,
        ),
        (
            "relationship-cycle",
            FixtureMode::RelationshipCycle,
            NavigationFailureKind::Apply,
            true,
        ),
    ];
    for (label, mode, expected, plan_failure) in rows {
        let mut fixture = recovery_r1_fixture(&format!("r1-{label}"));
        let first_location = RouteLocation::new("/section/items/1");
        let first = push_and_settle(
            &mut fixture.app,
            fixture.router,
            first_location.clone(),
            None,
            None,
        );
        assert_succeeded(&request_snapshot(&fixture.app, fixture.router, first));
        let before = router_snapshot(&fixture.app, fixture.router);
        let old_active = before.active().clone();
        let old_frames = old_active.frames().to_vec();
        let old_history = before.history().clone();
        assert_eq!(old_frames.len(), 3);
        fixture.control.set_mode(&fixture.leaf, mode);
        let requested = RouteLocation::new("/section/items/2");
        let request = fixture
            .app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(fixture.router, requested.clone())
            .unwrap();
        let (failed, observed) = drive_r1_request_to_terminal_preserving_old_active(
            &mut fixture.app,
            fixture.router,
            request,
            Some(&old_active),
        );
        assert!(
            observed > 0,
            "row {label} must expose a pre-commit interval"
        );
        assert_failed(&failed, expected);
        assert_eq!(failed.requested(), Some(&requested));
        let matched = failed
            .matched()
            .expect("post-match failure retains the exact matched fingerprint")
            .clone();
        let terminal = failed
            .trace()
            .iter()
            .position(|event| event.kind() == NavigationTraceEventKind::RequestTerminal)
            .unwrap();
        assert_eq!(terminal + 1, failed.trace().len(), "row {label}");
        assert!(
            failed
                .trace()
                .iter()
                .enumerate()
                .all(
                    |(index, event)| event.kind() != NavigationTraceEventKind::CleanupSettled
                        || index < terminal
                ),
            "row {label} settles every cleanup before terminal"
        );
        if mode == FixtureMode::ApplyFailure {
            let cleanup_phase = failed
                .trace()
                .iter()
                .position(|event| event.kind() == NavigationTraceEventKind::CleanupSettled)
                .expect("partial output cleanup settles before recovery issuance");
            let continuation = failed
                .trace()
                .iter()
                .position(|event| event.kind() == NavigationTraceEventKind::HistoryIntentRejected)
                .expect("cleanup completion resumes the frozen recovery continuation");
            let recovery_issued = failed
                .trace()
                .iter()
                .position(|event| {
                    event.kind() == NavigationTraceEventKind::FrameTaskIssued
                        && event.frame().is_some_and(|frame| {
                            frame.origin() == &FrameOrigin::RouteError(fixture.boundary_route)
                        })
                })
                .expect("bound error frame task is issued");
            let cleanup_receipts = failed.trace()[cleanup_phase..continuation]
                .iter()
                .filter(|event| event.kind() == NavigationTraceEventKind::CleanupSettled)
                .collect::<Vec<_>>();
            assert!(!cleanup_receipts.is_empty());
            assert_eq!(
                cleanup_receipts.last().unwrap().cleanup_disposition(),
                None,
                "the aggregate receipt closes the entire failed-output cleanup ledger",
            );
            assert!(cleanup_phase < continuation && continuation < recovery_issued);
            assert!(
                failed.trace()[continuation + 1..recovery_issued]
                    .iter()
                    .all(|event| event.kind() != NavigationTraceEventKind::CleanupSettled)
            );
        }
        if plan_failure {
            assert!(
                failed.trace().iter().all(|event| {
                    !matches!(
                        event.kind(),
                        NavigationTraceEventKind::MountRequested
                            | NavigationTraceEventKind::MountResolved
                            | NavigationTraceEventKind::MountGranted
                            | NavigationTraceEventKind::MountRejected
                            | NavigationTraceEventKind::MountRejectedAfterMutation
                    )
                }),
                "plan failure row {label} must remain World-zero and emit no mount event"
            );
        }
        let after = router_snapshot(&fixture.app, fixture.router);
        assert_eq!(after.history(), &old_history, "row {label}");
        assert_eq!(after.active().requested(), Some(&requested), "row {label}");
        assert_eq!(after.active().matched(), Some(&matched), "row {label}");
        assert_eq!(
            after.active().failure().map(NavigationFailure::kind),
            Some(expected),
            "row {label}"
        );
        assert_eq!(after.active().frames().len(), 2, "row {label}");
        assert_eq!(after.active().frames()[0], old_frames[0], "row {label}");
        assert_eq!(
            after.active().frames()[0].origin(),
            &FrameOrigin::MatchedRoute(fixture.root_route),
            "row {label}"
        );
        let error_frame = &after.active().frames()[1];
        assert_eq!(
            error_frame.origin(),
            &FrameOrigin::RouteError(fixture.boundary_route),
            "row {label}"
        );
        assert_ne!(error_frame, &old_frames[1], "row {label}");
        assert_ne!(error_frame, &old_frames[2], "row {label}");
        assert_ne!(error_frame.frame(), old_frames[1].frame(), "row {label}");
        let effects = fixture.control.effects();
        let recovery = effects.last().unwrap();
        assert_eq!(recovery.view, fixture.error, "row {label}");
        assert_eq!(
            recovery.origin,
            FrameOrigin::RouteError(fixture.boundary_route),
            "row {label}"
        );
        assert_eq!(recovery.failure, Some(expected), "row {label}");
        assert!(
            node_entities_with_width(&mut fixture.app, PARTIAL_RECT_WIDTH).is_empty(),
            "row {label} removes failed output before error publication"
        );
        assert_terminal_owner_closure(&fixture.app, fixture.router);
        assert_host_intact(&fixture.app, &fixture.host_fingerprint);
    }

    {
        let mut fixture = recovery_r1_fixture("r1-missing-outlet");
        let before = router_snapshot(&fixture.app, fixture.router);
        assert!(before.requested().is_none());
        assert!(before.matched().is_none());
        assert!(before.active().frames().is_empty());
        assert_eq!(before.history().len(), 0);
        fixture
            .control
            .set_mode(&fixture.section, FixtureMode::MissingOutlet { width: 53 });
        let requested = RouteLocation::new("/section/items/1");
        let request = fixture
            .app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(fixture.router, requested.clone())
            .unwrap();
        let (failed, _) = drive_r1_request_to_terminal_preserving_old_active(
            &mut fixture.app,
            fixture.router,
            request,
            None,
        );
        assert_failed(&failed, NavigationFailureKind::Mount);
        assert_eq!(failed.requested(), Some(&requested));
        let matched = failed
            .matched()
            .expect("a mount failure retains the exact matched branch")
            .clone();
        let rejected = failed
            .trace()
            .iter()
            .position(|event| {
                event.kind() == NavigationTraceEventKind::MountRejected
                    && event.mount_rejection() == Some(MountRejectionKind::MissingOutlet)
            })
            .expect("missing entering outlet emits the real rejection");
        assert!(rejected > 0);
        assert_eq!(
            failed.trace()[rejected - 1].kind(),
            NavigationTraceEventKind::MountRequested
        );
        assert_eq!(
            failed.trace()[rejected - 1].frame(),
            failed.trace()[rejected].frame()
        );
        assert_eq!(
            failed.trace()[rejected - 1].outlet(),
            failed.trace()[rejected].outlet()
        );
        let rejected_frame = failed.trace()[rejected]
            .frame()
            .expect("mount rejection names its exact parent frame");
        assert_eq!(
            rejected_frame.origin(),
            &FrameOrigin::MatchedRoute(fixture.boundary_route),
        );
        assert_eq!(
            failed.trace()[rejected].outlet(),
            Some(&RouteSlotKey::default()),
        );
        assert!(failed.trace()[..=rejected].iter().all(|event| {
            event.kind() != NavigationTraceEventKind::MountGranted
                || event.frame() != Some(rejected_frame)
        }));
        let after = router_snapshot(&fixture.app, fixture.router);
        assert_eq!(after.history(), before.history());
        assert_eq!(after.active().requested(), Some(&requested));
        assert_eq!(after.active().matched(), Some(&matched));
        assert_eq!(
            after.active().failure().map(NavigationFailure::kind),
            Some(NavigationFailureKind::Mount),
        );
        assert_eq!(after.active().frames().len(), 2);
        assert_eq!(
            after.active().frames()[0].origin(),
            &FrameOrigin::MatchedRoute(fixture.root_route)
        );
        assert_eq!(
            after.active().frames()[1].origin(),
            &FrameOrigin::RouteError(fixture.boundary_route)
        );
        assert_eq!(
            fixture.control.effects().last().unwrap().view,
            fixture.error
        );
        assert_terminal_owner_closure(&fixture.app, fixture.router);
        assert_host_intact(&fixture.app, &fixture.host_fingerprint);
    }
}

#[test]
fn recovery_r1_journey_d_recovery_failure_cleans_all_attempts_and_queue_continues() {
    let mut fixture = recovery_r1_fixture("r1-recovery-failure");
    let first_location = RouteLocation::new("/section/items/1");
    let first = push_and_settle(
        &mut fixture.app,
        fixture.router,
        first_location.clone(),
        None,
        None,
    );
    assert_succeeded(&request_snapshot(&fixture.app, fixture.router, first));
    let before = router_snapshot(&fixture.app, fixture.router);
    let old_active = before.active().clone();
    let old_history = before.history().clone();
    let old_entities = entity_set(&fixture.app);
    fixture
        .control
        .set_one_shot_mode(&fixture.leaf, FixtureMode::ApplyFailure);
    fixture
        .control
        .set_one_shot_mode(&fixture.error, FixtureMode::ApplyFailure);

    let failed_location = RouteLocation::new("/section/items/2");
    let queued_location = RouteLocation::new("/section/items/3");
    let failed_id = fixture
        .app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(fixture.router, failed_location.clone())
        .unwrap();
    let queued_id = fixture
        .app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(fixture.router, queued_location.clone())
        .unwrap();
    let mut observed_queued_before_cleanup_closed = false;
    let mut failed_terminal = None;
    for _ in 0..32 {
        fixture.app.update();
        let snapshots = fixture.app.world().resource::<RouterSnapshots>();
        let RequestLookup::Found(failed) = snapshots.request(fixture.router, failed_id) else {
            continue;
        };
        let RequestLookup::Found(queued) = snapshots.request(fixture.router, queued_id) else {
            continue;
        };
        if !matches!(failed.status(), RequestStatus::Failed(_)) {
            assert_eq!(queued.status(), &RequestStatus::Queued);
            observed_queued_before_cleanup_closed = true;
            assert_eq!(snapshots.get(fixture.router).unwrap().active(), &old_active);
            assert_eq!(
                snapshots.get(fixture.router).unwrap().history(),
                &old_history
            );
        }
        if matches!(failed.status(), RequestStatus::Failed(_)) {
            failed_terminal = Some(failed.clone());
            break;
        }
    }
    let failed = failed_terminal.expect("recovery failure settles within 32 updates");
    assert!(observed_queued_before_cleanup_closed);
    assert_failed(&failed, NavigationFailureKind::RecoveryFailed);
    assert_eq!(failed.requested(), Some(&failed_location));
    assert!(failed.diagnostics().iter().any(|event| {
        event.request() == Some(failed_id)
            && event.frame().is_some_and(|frame| {
                frame.origin() == &FrameOrigin::RouteError(fixture.boundary_route)
            })
            && event.report_disposition() == Some(ReportDisposition::Accepted)
    }));
    let recovery_effect = fixture
        .control
        .effects()
        .into_iter()
        .find(|effect| {
            effect.view == fixture.error
                && effect.origin == FrameOrigin::RouteError(fixture.boundary_route)
        })
        .expect("the bound error driver is invoked before recovery failure settles");
    assert_eq!(recovery_effect.location, failed_location);
    assert_eq!(recovery_effect.failure, Some(NavigationFailureKind::Apply),);
    let terminal = failed
        .trace()
        .iter()
        .position(|event| event.kind() == NavigationTraceEventKind::RequestTerminal)
        .unwrap();
    let receipts = failed
        .trace()
        .iter()
        .enumerate()
        .filter_map(|(index, event)| {
            (event.kind() == NavigationTraceEventKind::CleanupSettled).then_some(index)
        })
        .collect::<Vec<_>>();
    assert!(receipts.len() >= 2);
    assert!(receipts.iter().all(|index| *index < terminal));
    assert_eq!(terminal + 1, failed.trace().len());
    assert!(node_entities_with_width(&mut fixture.app, PARTIAL_RECT_WIDTH).is_empty());
    assert!(node_entities_with_width(&mut fixture.app, RECOVERY_PARTIAL_RECT_WIDTH).is_empty());

    let after_failure = router_snapshot(&fixture.app, fixture.router);
    assert_eq!(after_failure.active(), &old_active);
    assert_eq!(after_failure.history(), &old_history);
    assert_eq!(entity_set(&fixture.app), old_entities);
    assert!(!after_failure.cleanup().foreground_pending());
    assert_eq!(after_failure.cleanup().quarantine_count(), 0);
    #[cfg(feature = "test-support")]
    {
        let audit = fixture
            .app
            .world()
            .resource::<RouterAuthorityAudits>()
            .get(fixture.router)
            .expect("the failed attempt leaves a public authority audit");
        assert_eq!(audit.frames().len(), old_active.frames().len());
        assert!(audit.orphan_slots().is_empty());
    }

    let queued = drive_request_to_terminal(&mut fixture.app, fixture.router, queued_id);
    assert_succeeded(&queued);
    assert_eq!(queued.requested(), Some(&queued_location));
    let after_queue = router_snapshot(&fixture.app, fixture.router);
    assert_eq!(after_queue.active().location(), Some(&queued_location));
    assert_eq!(
        after_queue.history().entries(),
        [first_location, queued_location]
    );
    assert_terminal_owner_closure(&fixture.app, fixture.router);
    assert_host_intact(&fixture.app, &fixture.host_fingerprint);
}
