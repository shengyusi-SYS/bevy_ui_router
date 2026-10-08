#![cfg(feature = "bsn")]

use std::{
    collections::HashSet,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::AssetPlugin;
use bevy_camera::visibility::Visibility;
use bevy_ecs::{
    change_detection::Tick,
    component::ComponentId,
    entity::Entity,
    hierarchy::{ChildOf, Children},
    lifecycle::Insert,
    observer::On,
    prelude::Component,
    template::{FromTemplate, Template, TemplateContext},
};
use bevy_input_focus::InputFocus;
use bevy_picking::Pickable;
use bevy_scene::{ResolveContext, ResolveSceneError, ResolvedScene, Scene, ScenePlugin, bsn};
use bevy_ui::{Node, Val};
use bevy_ui_router::prelude::*;

const NORMAL_CONTENT_WIDTH: u16 = 31;
const FALLBACK_CONTENT_WIDTH: u16 = 79;
const PARTIAL_CONTENT_WIDTH: u16 = 199;
const RECOVERY_PARTIAL_CONTENT_WIDTH: u16 = 197;

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
    ResolveFailure,
    ApplyFailure,
    DuplicateOutlet,
    NonNodeOutlet,
    MissingOutlet { width: u16 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct EffectRecord {
    view: RouteViewKey,
    location: RouteLocation,
    route: Option<RouteId>,
    id_param: Option<String>,
    params: Vec<(String, String)>,
    search: Option<String>,
    fragment: Option<String>,
    origin: FrameOrigin,
    failure: Option<NavigationFailureKind>,
    failure_message: Option<String>,
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
    partial_writes: Arc<Mutex<Vec<Entity>>>,
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

    fn mode_and_record(&self, view: &RouteViewKey, input: &RouteViewInput) -> FixtureMode {
        let mut state = self
            .state
            .lock()
            .expect("fixture lock must not be poisoned");
        let one_shot = state.one_shot_modes.iter().position(|(key, _)| key == view);
        let mode = match one_shot {
            Some(index) => state.one_shot_modes.remove(index).1,
            None => state
                .modes
                .iter()
                .find_map(|(key, mode)| (key == view).then_some(*mode))
                .expect("every fixture view must have a mode"),
        };
        state.effects.push(EffectRecord {
            view: view.clone(),
            location: input.location().clone(),
            route: input.route(),
            id_param: input.params().get("id").map(str::to_owned),
            params: input
                .params()
                .iter()
                .map(|(key, value)| (key.to_owned(), value.to_owned()))
                .collect(),
            search: input.raw_search().map(str::to_owned),
            fragment: input.fragment().map(str::to_owned),
            origin: input.origin().clone(),
            failure: input.failure().map(NavigationFailure::kind),
            failure_message: input.failure().map(|failure| failure.message().to_owned()),
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

#[derive(Component, Clone, Default, Debug, Eq, PartialEq)]
struct LocalState(u32);

#[derive(Clone)]
struct FailAfterChildWrite {
    width: u16,
    message: &'static str,
    applied: Arc<Mutex<Vec<Entity>>>,
}

impl Template for FailAfterChildWrite {
    type Output = ();

    fn build_template(&self, context: &mut TemplateContext<'_, '_>) -> bevy_ecs::error::Result<()> {
        // A real official template writes its declared child before failing. This
        // child has not yet received the executor's ChildOf relationship.
        context.entity.insert(Node {
            width: Val::Px(f32::from(self.width)),
            ..Node::default()
        });
        assert!(context.entity.get::<ChildOf>().is_none());
        self.applied.lock().unwrap().push(context.entity.id());
        Err(bevy_ecs::error::BevyError::error(self.message))
    }

    fn clone_template(&self) -> Self {
        self.clone()
    }
}

struct FixtureScene {
    mode: FixtureMode,
    has_outlet: bool,
    partial: FailAfterChildWrite,
}

impl Scene for FixtureScene {
    fn resolve(
        self,
        _: &mut ResolveContext,
        root: &mut ResolvedScene,
    ) -> Result<(), ResolveSceneError> {
        if self.mode == FixtureMode::ResolveFailure {
            return Err(ResolveSceneError::MissingSceneDependency(
                "fixture-required.scene".into(),
            ));
        }
        let width = match self.mode {
            FixtureMode::Success { width } | FixtureMode::MissingOutlet { width } => width,
            _ => 0,
        };
        root.push_template(Node {
            width: Val::Px(f32::from(width)),
            height: Val::Px(23.0),
            ..Node::default()
        });
        root.push_template(LocalState(0));
        root.push_template(Pickable::default());
        if self.mode == FixtureMode::ApplyFailure {
            let mut child = ResolvedScene::default();
            child.push_bundle_template(self.partial);
            root.get_or_insert_related_resolved_scenes::<ChildOf>()
                .scenes
                .push(child);
        } else {
            let outlets = match self.mode {
                FixtureMode::MissingOutlet { .. } => 0,
                FixtureMode::DuplicateOutlet => 2,
                FixtureMode::NonNodeOutlet => 1,
                _ => usize::from(self.has_outlet),
            };
            if outlets > 0 {
                let mut container = ResolvedScene::default();
                container.push_template(Node::default());
                for _ in 0..outlets {
                    let mut anchor = ResolvedScene::default();
                    if self.mode != FixtureMode::NonNodeOutlet {
                        anchor.push_template(Node::default());
                    }
                    anchor.push_template(<DefaultOutlet as FromTemplate>::Template::default());
                    container
                        .get_or_insert_related_resolved_scenes::<ChildOf>()
                        .scenes
                        .push(anchor);
                }
                root.get_or_insert_related_resolved_scenes::<ChildOf>()
                    .scenes
                    .push(container);
            }
        }
        Ok(())
    }
}

fn native_app() -> App {
    let mut app = App::new();
    app.add_plugins((
        TaskPoolPlugin::default(),
        AssetPlugin::default(),
        ScenePlugin,
        BevyUiRouterPlugin::default(),
    ));
    app
}

fn register_fixture_view(
    app: &mut App,
    control: &FixtureControl,
    view: &RouteViewKey,
    has_outlet: bool,
) -> ViewDriverKey {
    let control = control.clone();
    let view = view.clone();
    app.register_bsn_view(
        ViewDriverKey::new(format!("{view:?}")).unwrap(),
        move |input| {
            let mode = control.mode_and_record(&view, input);
            if mode == FixtureMode::BuildFailure {
                return Err(BsnBuildError::new("fixture build failure"));
            }
            Ok(FixtureScene {
                mode,
                has_outlet,
                partial: FailAfterChildWrite {
                    width: match input.origin() {
                        FrameOrigin::RouteError(_) | FrameOrigin::RouterFallback => {
                            RECOVERY_PARTIAL_CONTENT_WIDTH
                        }
                        FrameOrigin::MatchedRoute(_) => PARTIAL_CONTENT_WIDTH,
                    },
                    message: match input.origin() {
                        FrameOrigin::MatchedRoute(_) => {
                            "fixture apply failure after partial child write"
                        }
                        _ => "fixture recovery apply failure after partial child write",
                    },
                    applied: control.partial_writes.clone(),
                },
            })
        },
    )
    .unwrap()
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
    let mut app = native_app();

    let normal = RouteViewKey::new("fixture-normal").unwrap();
    let fallback = RouteViewKey::new("fixture-fallback").unwrap();
    let control = FixtureControl::default();
    control.set_mode(
        &normal,
        FixtureMode::Success {
            width: NORMAL_CONTENT_WIDTH,
        },
    );
    control.set_mode(
        &fallback,
        FixtureMode::Success {
            width: FALLBACK_CONTENT_WIDTH,
        },
    );

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
            .insert(
                view.clone(),
                RouteViewDescriptor::new(register_fixture_view(&mut app, &control, &view, false)),
            )
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

struct RecoveryR3AdmissionFixture {
    app: App,
    router: RouterHandle,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RecoveryR2LossKind {
    RegistrationRemoval,
    HostDespawn,
}

#[derive(Component, Debug, Eq, PartialEq)]
struct RecoveryR3ForeignChildSentinel(u64);

fn remove_recovery_host(fixture: &mut RecoveryR1Fixture, kind: RecoveryR2LossKind) {
    let host = fixture.host_fingerprint.host;
    match kind {
        RecoveryR2LossKind::RegistrationRemoval => {
            assert!(
                fixture
                    .app
                    .world_mut()
                    .entity_mut(host)
                    .take::<RouterHostRegistration>()
                    .is_some()
            );
        }
        RecoveryR2LossKind::HostDespawn => {
            assert!(fixture.app.world_mut().despawn(host));
        }
    }
}

fn recovery_r1_fixture(label: &str) -> RecoveryR1Fixture {
    let mut app = native_app();

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
    let alternate_section = routes
        .route_with_options(
            root_route,
            RoutePattern::path("alternate").unwrap(),
            RouteNodeOptions::new()
                .with_view(section.clone())
                .with_slot(RouteSlotKey::default()),
        )
        .unwrap();
    routes
        .route_with_options(
            alternate_section,
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
            .insert(
                view.clone(),
                RouteViewDescriptor::new(register_fixture_view(
                    &mut app,
                    &control,
                    &view,
                    view == root || view == section,
                )),
            )
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

fn recovery_r3_admission_fixture(label: &str) -> RecoveryR3AdmissionFixture {
    let mut app = native_app();
    let view = RouteViewKey::new(format!("{label}-view")).unwrap();
    let driver = app
        .register_bsn_view(
            ViewDriverKey::new(format!("{label}-driver")).unwrap(),
            |_| Ok(bsn! { Node }),
        )
        .unwrap();
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
    RecoveryR3AdmissionFixture { app, router }
}

fn nested_fixture() -> NestedFixture {
    nested_fixture_with_depth(2, RoutePattern::pathless())
}

fn nested_fixture_with_depth(depth: usize, root_pattern: RoutePattern) -> NestedFixture {
    assert!(depth >= 2);
    nested_fixture_with_patterns(
        root_pattern,
        (2..depth).map(|_| RoutePattern::pathless()).collect(),
    )
}

fn nested_fixture_with_patterns(
    root_pattern: RoutePattern,
    ancestors: Vec<RoutePattern>,
) -> NestedFixture {
    nested_fixture_with_retention(root_pattern, ancestors, 0)
}

fn nested_fixture_with_retention(
    root_pattern: RoutePattern,
    ancestors: Vec<RoutePattern>,
    capacity: usize,
) -> NestedFixture {
    nested_fixture_with_retention_and_detail(root_pattern, ancestors, capacity, None)
}

fn nested_fixture_with_retention_and_detail(
    root_pattern: RoutePattern,
    ancestors: Vec<RoutePattern>,
    capacity: usize,
    detail: Option<RouteViewKey>,
) -> NestedFixture {
    let mut app = native_app();

    let layout = RouteViewKey::new("nested-layout").unwrap();
    let leaf = RouteViewKey::new("nested-leaf").unwrap();
    let fallback = RouteViewKey::new("nested-fallback").unwrap();
    let control = FixtureControl::default();
    for (view, width) in [
        (layout.clone(), 41),
        (leaf.clone(), 43),
        (fallback.clone(), FALLBACK_CONTENT_WIDTH),
    ] {
        control.set_mode(&view, FixtureMode::Success { width });
    }
    let mut routes = RouteTree::builder()
        .root_with_options(
            root_pattern,
            RouteNodeOptions::new().with_view(layout.clone()),
        )
        .unwrap();
    let layout_route = routes.root_id().unwrap();
    let mut leaf_parent = layout_route;
    for pattern in ancestors {
        leaf_parent = routes
            .route_with_options(
                leaf_parent,
                pattern,
                RouteNodeOptions::new()
                    .with_view(layout.clone())
                    .with_slot(RouteSlotKey::default()),
            )
            .unwrap();
    }
    let leaf_route = routes
        .route_with_options(
            leaf_parent,
            RoutePattern::path("items/:id").unwrap(),
            RouteNodeOptions::new()
                .with_view(leaf.clone())
                .with_slot(RouteSlotKey::default())
                .with_keep_alive(capacity > 0),
        )
        .unwrap();
    if let Some(detail) = &detail {
        control.set_mode(detail, FixtureMode::Success { width: 47 });
        routes
            .route_with_options(
                leaf_route,
                RoutePattern::path("detail").unwrap(),
                RouteNodeOptions::new().with_view(detail.clone()),
            )
            .unwrap();
    }
    let leaf_has_outlet = detail.is_some();
    let routes = routes.build().unwrap();
    let mut views = RouteViewRegistry::new();
    let mut fixture_views = vec![layout.clone(), leaf.clone(), fallback.clone()];
    fixture_views.extend(detail);
    for view in fixture_views {
        views
            .insert(
                view.clone(),
                RouteViewDescriptor::new(register_fixture_view(
                    &mut app,
                    &control,
                    &view,
                    view == layout || (leaf_has_outlet && view == leaf),
                )),
            )
            .unwrap();
    }
    let router = app
        .add_ui_router(
            RouterConfig::new(routes, views, fallback).with_keep_alive_capacity(capacity),
        )
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
        .register_bsn_view(
            ViewDriverKey::new(format!("{label}-baseline-driver")).unwrap(),
            |_| Ok(bsn! { Node }),
        )
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
        .register_bsn_view(
            ViewDriverKey::new(format!("{label}-registered-driver")).unwrap(),
            |_| Ok(bsn! { Node }),
        )
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
    forbidden_content_width: Option<u16>,
    leaving_entities: Option<&HashSet<Entity>>,
) -> NavigationRequestId {
    let request = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, location)
        .expect("fixture navigation should enqueue");

    for _ in 0..16 {
        app.update();
        if let Some(width) = forbidden_content_width {
            assert!(
                node_entities_with_width(app, width).is_empty(),
                "failed native child output must be removed before returning to the app"
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
    panic!(
        "fixture request did not settle within 16 App::update calls: router={:?} request={:?}",
        router_snapshot(app, router),
        request_snapshot(app, router, request)
    )
}

fn replace_and_settle(
    app: &mut App,
    router: RouterHandle,
    location: RouteLocation,
    forbidden_content_width: Option<u16>,
) -> NavigationRequestId {
    let request = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .replace(router, location)
        .expect("fixture replace should enqueue");
    for _ in 0..16 {
        app.update();
        if let Some(width) = forbidden_content_width {
            assert!(
                node_entities_with_width(app, width).is_empty(),
                "failed native child output must be removed before returning to the app"
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

fn assert_frame_diagnostic(snapshot: &NavigationRequestSnapshot, request: NavigationRequestId) {
    assert!(snapshot.diagnostics().iter().any(|event| {
        event.request() == Some(request)
            && event.frame().is_some()
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
                && event.cleanup_disposition().is_some()
                && index > cleanup_phase)
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
}

fn assert_failed_route_trace_settles_before_fallback(
    snapshot: &NavigationRequestSnapshot,
    expected: NavigationFailureKind,
    expect_failed_cleanup: bool,
) {
    assert_failed(snapshot, expected);
    let trace = snapshot.trace();
    let rejected = next_trace_event(
        trace,
        None,
        NavigationTraceEventKind::HistoryIntentRejected,
        |_| true,
    );
    let fallback = next_trace_event(
        trace,
        Some(rejected),
        NavigationTraceEventKind::FrameTaskIssued,
        |event| {
            event
                .frame()
                .is_some_and(|frame| frame.origin() == &FrameOrigin::RouterFallback)
        },
    );
    let frame = trace[fallback].frame().unwrap();
    assert!(trace[..rejected].iter().any(|event| {
        event.kind() == NavigationTraceEventKind::FrameTaskIssued
            && event
                .frame()
                .is_some_and(|frame| matches!(frame.origin(), FrameOrigin::MatchedRoute(_)))
    }));
    if expect_failed_cleanup {
        assert!(trace[..rejected].iter().any(|event| event.kind()
            == NavigationTraceEventKind::CleanupSettled
            && event.cleanup_disposition().is_none()));
    }
    let applied = next_trace_event(
        trace,
        Some(fallback),
        NavigationTraceEventKind::ViewApplied,
        |event| event.frame() == Some(frame),
    );
    let active = next_trace_event(
        trace,
        Some(applied),
        NavigationTraceEventKind::FrameActivated,
        |event| event.frame() == Some(frame),
    );
    next_trace_event(
        trace,
        Some(active),
        NavigationTraceEventKind::CommitPublished,
        |_| true,
    );
    assert_cleanup_trace_is_ordered(snapshot, true);
}

fn assert_succeeded_back_trace(snapshot: &NavigationRequestSnapshot) {
    assert_succeeded(snapshot);
    let trace = snapshot.trace();
    let issued = next_trace_event(
        trace,
        None,
        NavigationTraceEventKind::FrameTaskIssued,
        |event| {
            event
                .frame()
                .is_some_and(|frame| matches!(frame.origin(), FrameOrigin::MatchedRoute(_)))
        },
    );
    let frame = trace[issued].frame().unwrap();
    let applied = next_trace_event(
        trace,
        Some(issued),
        NavigationTraceEventKind::ViewApplied,
        |event| event.frame() == Some(frame),
    );
    let active = next_trace_event(
        trace,
        Some(applied),
        NavigationTraceEventKind::FrameActivated,
        |event| event.frame() == Some(frame),
    );
    next_trace_event(
        trace,
        Some(active),
        NavigationTraceEventKind::CommitPublished,
        |_| true,
    );
    assert_cleanup_trace_is_ordered(snapshot, true);
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
        fingerprint.existing_children.len() + usize::from(fingerprint.top_slot.is_some()),
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

fn assert_host_after_registration_loss(app: &App, fingerprint: &HostBoundaryFingerprint) {
    let host = fingerprint.host;
    let existing_child = fingerprint.existing_children[0];
    let existing_sibling = fingerprint.existing_children[1];
    assert_eq!(
        app.world().get::<HostSentinel>(host),
        Some(&HostSentinel(7))
    );
    assert_eq!(
        app.world().get::<HostMetadata>(host),
        Some(&HostMetadata(73))
    );
    assert!(app.world().get::<RouterHostRegistration>(host).is_none());
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
    assert_eq!(
        app.world()
            .get::<Children>(host)
            .unwrap()
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        fingerprint.existing_children
    );
    if let Some(top_slot) = fingerprint.top_slot {
        assert!(app.world().get_entity(top_slot).is_err());
    }
    let children_component = app.world().component_id::<Children>().unwrap();
    let registration_component = app
        .world()
        .component_id::<RouterHostRegistration>()
        .unwrap();
    let current_host_components = component_manifest(app, host)
        .into_iter()
        .filter(|component| *component != children_component)
        .collect::<Vec<_>>();
    let expected_host_components = fingerprint
        .host_non_children_components
        .iter()
        .copied()
        .filter(|component| *component != registration_component)
        .collect::<Vec<_>>();
    assert_eq!(current_host_components, expected_host_components);
    for (child, expected_components) in &fingerprint.existing_child_components {
        assert_eq!(component_manifest(app, *child), *expected_components);
    }
    let host_node = app.world().get::<Node>(host).unwrap();
    assert_eq!(host_node.width, Val::Px(640.0));
    assert_eq!(host_node.height, Val::Px(360.0));
    let child_node = app.world().get::<Node>(existing_child).unwrap();
    assert_eq!(child_node.width, Val::Px(13.0));
    assert_eq!(child_node.height, Val::Px(17.0));
    let sibling_node = app.world().get::<Node>(existing_sibling).unwrap();
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

fn spawn_recovery_r3_app_host(app: &mut App, router: RouterHandle) -> HostBoundaryFingerprint {
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
    let child = app
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
    let sibling = app
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
    capture_host_boundary(app, host, &[child, sibling])
}

fn drive_recovery_r3_availability(
    app: &mut App,
    router: RouterHandle,
    expected: &RouterAvailability,
) -> RouterSnapshot {
    for _ in 0..24 {
        app.update();
        let snapshot = router_snapshot(app, router);
        if snapshot.availability() == expected
            && snapshot.running_request().is_none()
            && snapshot.pending_phase().is_none()
            && !snapshot.cleanup().foreground_pending()
        {
            return snapshot;
        }
    }
    panic!(
        "R3 router did not reach stable {expected:?}: {:?}",
        router_snapshot(app, router)
    )
}

fn assert_recovery_r3_enqueue_is_request_zero(
    app: &mut App,
    router: RouterHandle,
    location: RouteLocation,
) {
    let snapshot_before = router_snapshot(app, router);
    let error = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, location)
        .expect_err("an unavailable host rejects before allocating a request");
    assert_eq!(error.kind(), NavigationEnqueueErrorKind::HostUnavailable);
    assert_eq!(router_snapshot(app, router), snapshot_before);
}

fn settle_recovery_r3_queued_registration_loss(fixture: &mut RecoveryR1Fixture) -> RouterSnapshot {
    // Exercise re-admission after a real foreground HostLost. Exact loss during
    // post-commit cleanup is covered privately; fully idle loss has separate
    // ordinary-App-update regressions and does not rely on this queued request.
    let request = fixture
        .app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(fixture.router, RouteLocation::new("/section/items/3"))
        .unwrap();
    remove_recovery_host(fixture, RecoveryR2LossKind::RegistrationRemoval);
    let unavailable = drive_recovery_r3_availability(
        &mut fixture.app,
        fixture.router,
        &RouterAvailability::Unavailable(RouterUnavailableReason::HostLost),
    );
    let failed = request_snapshot(&fixture.app, fixture.router, request);
    assert_failed(&failed, NavigationFailureKind::HostLost);
    assert!(
        failed
            .trace()
            .iter()
            .all(|event| event.kind() != NavigationTraceEventKind::CommitPublished)
    );
    unavailable
}

fn drive_until_publication(
    app: &mut App,
    router: RouterHandle,
    request: NavigationRequestId,
) -> RouterSnapshot {
    for _ in 0..16 {
        app.update();
        if request_snapshot(app, router, request)
            .trace()
            .iter()
            .any(|event| event.kind() == NavigationTraceEventKind::CommitPublished)
        {
            return router_snapshot(app, router);
        }
    }
    panic!(
        "navigation must publish before host loss: {:?}",
        request_snapshot(app, router, request)
    );
}

fn settle_recovery_r3_same_host_queued_registration_loss(
    fixture: &mut RecoveryR1Fixture,
    old_top_slot: Entity,
    foreign: Entity,
    location: RouteLocation,
) -> RouterSnapshot {
    let request = push_and_settle(&mut fixture.app, fixture.router, location, None, None);
    let committed = request_snapshot(&fixture.app, fixture.router, request);
    assert_succeeded(&committed);
    fixture
        .app
        .world_mut()
        .entity_mut(foreign)
        .insert(ChildOf(old_top_slot));
    let unavailable = settle_recovery_r3_queued_registration_loss(fixture);
    assert_eq!(
        request_snapshot(&fixture.app, fixture.router, request),
        committed
    );
    unavailable
}

fn node_entities_with_width(app: &mut App, width: u16) -> Vec<Entity> {
    let world = app.world_mut();
    let mut query = world.query::<(Entity, &Node)>();
    query
        .iter(world)
        .filter_map(|(entity, node)| (node.width == Val::Px(f32::from(width))).then_some(entity))
        .collect()
}

fn assert_native_content(app: &mut App, width: u16) {
    let entities = node_entities_with_width(app, width);
    assert_eq!(entities.len(), 1);
    assert_ne!(
        app.world()
            .get::<Visibility>(entities[0])
            .expect("native authored content must carry Visibility"),
        &Visibility::Hidden
    );
    let pickable = app
        .world()
        .get::<Pickable>(entities[0])
        .expect("native authored content must carry Pickable");
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

#[test]
fn recovery_r1_install_matrix_is_synchronous_and_world_zero() {
    {
        let mut app = native_app();
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
        let mut app = native_app();
        let baseline = install_matrix_baseline(&mut app, label);
        let config = recovery_install_config(&mut app, label, case);
        let router_before = app
            .world()
            .resource::<RouterSnapshots>()
            .get(baseline)
            .expect("baseline router is synchronously visible")
            .clone();
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
    }
}

#[test]
fn native_duplicate_registration_keeps_the_original_factory_without_invoking_either() {
    let mut fixture = root_fixture("/items/:id");
    let rejected_calls = Arc::new(AtomicUsize::new(0));
    let calls = rejected_calls.clone();
    let world_before = world_component_fingerprint(&fixture.app);
    let error = fixture
        .app
        .register_bsn_view(
            ViewDriverKey::new(format!("{:?}", fixture.normal)).unwrap(),
            move |_| {
                calls.fetch_add(1, Ordering::Relaxed);
                Ok(bsn! { Node { width: Val::Px(911.0) } })
            },
        )
        .unwrap_err();
    assert_eq!(error.kind(), ViewDriverRegistrationErrorKind::DuplicateKey);
    assert_eq!(world_component_fingerprint(&fixture.app), world_before);
    assert!(fixture.control.effects().is_empty());
    assert_eq!(rejected_calls.load(Ordering::Relaxed), 0);
    let request = push_and_settle(
        &mut fixture.app,
        fixture.router,
        RouteLocation::new("/items/1"),
        None,
        None,
    );
    assert_succeeded(&request_snapshot(&fixture.app, fixture.router, request));
    assert_eq!(fixture.control.effects().len(), 1);
    assert_eq!(fixture.control.effects()[0].view, fixture.normal);
    assert_eq!(rejected_calls.load(Ordering::Relaxed), 0);
    assert_native_content(&mut fixture.app, NORMAL_CONTENT_WIDTH);
    assert!(node_entities_with_width(&mut fixture.app, 911).is_empty());
}

#[test]
fn native_committed_outlet_loss_is_authority_loss_not_entering_rejection() {
    let mut fixture = nested_fixture();
    fixture.app.init_resource::<InputFocus>();
    let first = push_and_settle(
        &mut fixture.app,
        fixture.router,
        RouteLocation::new("/items/1"),
        None,
        None,
    );
    assert_succeeded(&request_snapshot(&fixture.app, fixture.router, first));
    let before = router_snapshot(&fixture.app, fixture.router);
    let layout = node_entities_with_width(&mut fixture.app, 41)[0];
    let leaf = node_entities_with_width(&mut fixture.app, 43)[0];
    assert!(
        fixture
            .app
            .world()
            .resource::<RouterInputGate>()
            .allows_keyboard(leaf)
    );
    *fixture.app.world_mut().resource_mut::<InputFocus>() = InputFocus::from_entity(leaf);
    let container = fixture.app.world().get::<Children>(layout).unwrap()[0];
    let anchor = fixture.app.world().get::<Children>(container).unwrap()[0];
    fixture
        .app
        .world_mut()
        .entity_mut(anchor)
        .remove::<DefaultOutlet>();
    let request = push_and_settle(
        &mut fixture.app,
        fixture.router,
        RouteLocation::new("/items/2"),
        None,
        None,
    );
    let failed = request_snapshot(&fixture.app, fixture.router, request);
    assert_failed(&failed, NavigationFailureKind::AuthorityLost);
    assert!(
        failed
            .trace()
            .iter()
            .all(|event| event.kind() != NavigationTraceEventKind::CommitPublished)
    );
    let after = router_snapshot(&fixture.app, fixture.router);
    assert_eq!(after.history(), before.history());
    assert_eq!(after.active().kind(), ActivePresentationKind::None);
    assert_terminal_owner_closure(&fixture.app, fixture.router);
    let gate = fixture.app.world().resource::<RouterInputGate>();
    for entity in [layout, leaf] {
        assert!(!gate.allows_pointer(entity));
        assert!(!gate.allows_keyboard(entity));
        assert!(!gate.allows_gamepad(entity));
    }
    assert_eq!(fixture.app.world().resource::<InputFocus>().get(), None);
    assert!(
        fixture
            .app
            .world()
            .get_entity(fixture.host_fingerprint.top_slot.unwrap())
            .is_err()
    );
    let mut retired_boundary = fixture.host_fingerprint.clone();
    retired_boundary.top_slot = None;
    assert_host_intact(&fixture.app, &retired_boundary);
}

#[test]
fn history_back_no_entry_is_synchronous_and_preserves_snapshots() {
    let mut fixture = root_fixture("/items/:id");
    let first = push_and_settle(
        &mut fixture.app,
        fixture.router,
        RouteLocation::new("/items/1"),
        None,
        None,
    );
    let before = router_snapshot(&fixture.app, fixture.router);
    let retained = request_snapshot(&fixture.app, fixture.router, first);
    assert_succeeded(&retained);
    let error = fixture
        .app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .back(fixture.router)
        .expect_err("cursor zero rejects Back synchronously");
    assert_eq!(error.kind(), NavigationEnqueueErrorKind::NoBackEntry);
    assert_eq!(router_snapshot(&fixture.app, fixture.router), before);
    assert_eq!(
        request_snapshot(&fixture.app, fixture.router, first),
        retained
    );
    fixture.app.update();
    assert_eq!(router_snapshot(&fixture.app, fixture.router), before);
    assert_eq!(
        request_snapshot(&fixture.app, fixture.router, first),
        retained
    );
}

#[test]
fn history_back_then_push_truncates_forward_tail() {
    let mut fixture = root_fixture("/items/:id");
    let [a, b, c, d] = ["/items/1", "/items/2", "/items/3", "/items/4"].map(RouteLocation::new);
    for location in [&a, &b, &c] {
        let request = push_and_settle(
            &mut fixture.app,
            fixture.router,
            location.clone(),
            None,
            None,
        );
        assert_succeeded(&request_snapshot(&fixture.app, fixture.router, request));
    }
    let back = fixture
        .app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .back(fixture.router)
        .unwrap();
    let completed = drive_request_to_terminal(&mut fixture.app, fixture.router, back);
    assert_succeeded(&completed);
    assert_eq!(completed.requested(), Some(&b));
    let pushed = push_and_settle(&mut fixture.app, fixture.router, d.clone(), None, None);
    assert_succeeded(&request_snapshot(&fixture.app, fixture.router, pushed));
    let after = router_snapshot(&fixture.app, fixture.router);
    assert_eq!(after.history().entries(), [a, b, d.clone()]);
    assert_eq!(after.history().cursor(), Some(2));
    assert_eq!(after.history().current(), Some(&d));
    assert_eq!(after.active().location(), Some(&d));
    assert!(!after.history().entries().contains(&c));
}

#[test]
fn nested_replace_revokes_old_input_and_focus_but_preserves_layout_input() {
    // Gate/focus facade behavior; native picking dispatch is exercised by ECS.
    let mut fixture = nested_fixture();
    let first = push_and_settle(
        &mut fixture.app,
        fixture.router,
        RouteLocation::new("/items/1"),
        None,
        None,
    );
    assert_succeeded(&request_snapshot(&fixture.app, fixture.router, first));
    let layout = node_entities_with_width(&mut fixture.app, 41);
    let old = node_entities_with_width(&mut fixture.app, 43);
    assert_eq!(layout.len(), 1);
    assert_eq!(old.len(), 1);
    let gate = fixture.app.world().resource::<RouterInputGate>();
    for target in [layout[0], old[0]] {
        assert!(gate.allows_pointer(target));
        assert!(gate.allows_keyboard(target));
        assert!(gate.allows_gamepad(target));
    }
    *fixture.app.world_mut().resource_mut::<InputFocus>() = InputFocus::from_entity(old[0]);
    let replacement = replace_and_settle(
        &mut fixture.app,
        fixture.router,
        RouteLocation::new("/items/2"),
        None,
    );
    assert_succeeded(&request_snapshot(&fixture.app, fixture.router, replacement));
    let new = node_entities_with_width(&mut fixture.app, 43);
    assert_eq!(new.len(), 1);
    assert_ne!(new[0], old[0]);
    assert_eq!(node_entities_with_width(&mut fixture.app, 41), layout);
    let gate = fixture.app.world().resource::<RouterInputGate>();
    assert!(!gate.allows_pointer(old[0]));
    assert!(!gate.allows_keyboard(old[0]));
    assert!(!gate.allows_gamepad(old[0]));
    for target in [layout[0], new[0]] {
        assert!(gate.allows_pointer(target));
        assert!(gate.allows_keyboard(target));
        assert!(gate.allows_gamepad(target));
    }
    assert_eq!(fixture.app.world().resource::<InputFocus>().get(), None);
    assert_host_intact(&fixture.app, &fixture.host_fingerprint);
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
fn history_back_restores_nested_location_after_search_rebuild() {
    let mut fixture = nested_fixture();
    let original = RouteLocation::new("/items/1").with_search("filter=archived");
    let next = RouteLocation::new("/items/2");
    for location in [original.clone(), next.clone()] {
        let request = push_and_settle(&mut fixture.app, fixture.router, location, None, None);
        assert_succeeded(&request_snapshot(&fixture.app, fixture.router, request));
    }
    let old_layout = node_entities_with_width(&mut fixture.app, 41)[0];
    let old_leaf = node_entities_with_width(&mut fixture.app, 43)[0];
    let back = fixture
        .app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .back(fixture.router)
        .expect("a published prior entry must enqueue Back");
    let terminal = drive_request_to_terminal(&mut fixture.app, fixture.router, back);
    assert_succeeded(&terminal);
    assert_eq!(terminal.requested(), Some(&original));
    let after = router_snapshot(&fixture.app, fixture.router);
    assert_eq!(after.active().location(), Some(&original));
    assert_eq!(after.history().entries(), [original.clone(), next]);
    assert_eq!(after.history().cursor(), Some(0));
    assert_eq!(after.history().current(), Some(&original));
    assert!(fixture.app.world().get_entity(old_layout).is_err());
    assert!(fixture.app.world().get_entity(old_leaf).is_err());
    assert_eq!(node_entities_with_width(&mut fixture.app, 41).len(), 1);
    assert_eq!(node_entities_with_width(&mut fixture.app, 43).len(), 1);
    let effects = fixture.control.effects();
    let restored = &effects[effects.len() - 2..];
    for effect in restored {
        assert_eq!(effect.location, original);
        assert_eq!(effect.search.as_deref(), Some("filter=archived"));
    }
    assert_eq!(restored[1].id_param.as_deref(), Some("1"));
    assert_host_intact(&fixture.app, &fixture.host_fingerprint);
}

#[test]
fn history_back_rebuilds_deep_ancestors_and_mounts_each_child() {
    let mut fixture = nested_fixture_with_depth(3, RoutePattern::pathless());
    let original = RouteLocation::new("/items/1").with_search("filter=original");
    let next = RouteLocation::new("/items/2");
    for location in [original.clone(), next.clone()] {
        let request = push_and_settle(&mut fixture.app, fixture.router, location, None, None);
        assert_succeeded(&request_snapshot(&fixture.app, fixture.router, request));
    }
    let old_layouts = node_entities_with_width(&mut fixture.app, 41);
    assert_eq!(old_layouts.len(), 2);
    let effects_before = fixture.control.effects().len();
    let back = fixture
        .app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .back(fixture.router)
        .unwrap();
    let terminal = drive_request_to_terminal(&mut fixture.app, fixture.router, back);
    assert_succeeded(&terminal);
    assert_eq!(
        terminal
            .trace()
            .iter()
            .filter(|event| event.kind() == NavigationTraceEventKind::MountGranted)
            .count(),
        2
    );
    let after = router_snapshot(&fixture.app, fixture.router);
    assert_eq!(after.active().frames().len(), 3);
    assert_eq!(after.active().location(), Some(&original));
    assert_eq!(after.history().entries(), [original.clone(), next]);
    assert_eq!(after.history().cursor(), Some(0));
    assert!(
        old_layouts
            .iter()
            .all(|entity| fixture.app.world().get_entity(*entity).is_err())
    );
    assert_eq!(node_entities_with_width(&mut fixture.app, 41).len(), 2);
    assert_eq!(node_entities_with_width(&mut fixture.app, 43).len(), 1);
    let effects = fixture.control.effects();
    assert_eq!(effects.len() - effects_before, 3);
    for effect in &effects[effects_before..] {
        assert_eq!(effect.location, original);
        assert_eq!(effect.search.as_deref(), Some("filter=original"));
    }
    assert_terminal_owner_closure(&fixture.app, fixture.router);
    assert_host_intact(&fixture.app, &fixture.host_fingerprint);
}

#[test]
fn history_back_rebuilds_parameterized_section_beneath_preserved_root() {
    let mut fixture = nested_fixture_with_patterns(
        RoutePattern::path("/").unwrap(),
        vec![RoutePattern::path("sections/:section").unwrap()],
    );
    let original = RouteLocation::new("/sections/a/items/1");
    let next = RouteLocation::new("/sections/b/items/2");
    let first = push_and_settle(
        &mut fixture.app,
        fixture.router,
        original.clone(),
        None,
        None,
    );
    assert_succeeded(&request_snapshot(&fixture.app, fixture.router, first));
    let original_layouts = node_entities_with_width(&mut fixture.app, 41);
    assert_eq!(original_layouts.len(), 2);
    let first_root = router_snapshot(&fixture.app, fixture.router)
        .active()
        .frames()[0]
        .clone();
    let second = push_and_settle(&mut fixture.app, fixture.router, next.clone(), None, None);
    assert_succeeded(&request_snapshot(&fixture.app, fixture.router, second));
    let before = router_snapshot(&fixture.app, fixture.router);
    assert_eq!(before.active().frames()[0], first_root);
    let current_layouts = node_entities_with_width(&mut fixture.app, 41);
    assert_eq!(current_layouts.len(), 2);
    let preserved = current_layouts
        .iter()
        .copied()
        .filter(|entity| original_layouts.contains(entity))
        .collect::<Vec<_>>();
    assert_eq!(
        preserved.len(),
        1,
        "only the root content survives the section parameter change"
    );
    let root_entity = preserved[0];
    let old_section = *current_layouts
        .iter()
        .find(|entity| **entity != root_entity)
        .unwrap();
    let old_leaf = node_entities_with_width(&mut fixture.app, 43)[0];
    fixture
        .app
        .world_mut()
        .entity_mut(root_entity)
        .insert(LocalState(61));
    let effects_before = fixture.control.effects().len();
    let back = fixture
        .app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .back(fixture.router)
        .unwrap();
    let terminal = drive_request_to_terminal(&mut fixture.app, fixture.router, back);
    assert_succeeded(&terminal);
    let after = router_snapshot(&fixture.app, fixture.router);
    assert_eq!(after.active().location(), Some(&original));
    assert_eq!(after.active().frames().len(), 3);
    assert_eq!(after.active().frames()[0], first_root);
    assert_eq!(
        fixture.app.world().get::<LocalState>(root_entity),
        Some(&LocalState(61))
    );
    assert!(fixture.app.world().get_entity(old_section).is_err());
    assert!(fixture.app.world().get_entity(old_leaf).is_err());
    assert_eq!(node_entities_with_width(&mut fixture.app, 41).len(), 2);
    assert_eq!(node_entities_with_width(&mut fixture.app, 43).len(), 1);
    assert!(terminal.trace().iter().any(|event| event.kind()
        == NavigationTraceEventKind::MountGranted
        && event.frame() == Some(&after.active().frames()[1])));
    assert_eq!(after.history().entries(), [original.clone(), next]);
    assert_eq!(after.history().cursor(), Some(0));
    let effects = fixture.control.effects();
    assert_eq!(effects.len() - effects_before, 2);
    for effect in &effects[effects_before..] {
        assert_eq!(effect.location, original);
    }
    assert_eq!(
        effects[effects_before].params,
        [("section".to_owned(), "a".to_owned())]
    );
    assert_eq!(
        effects[effects_before + 1].params,
        [("id".to_owned(), "1".to_owned())]
    );
    assert_eq!(effects.last().unwrap().id_param.as_deref(), Some("1"));
    assert_terminal_owner_closure(&fixture.app, fixture.router);
    assert_host_intact(&fixture.app, &fixture.host_fingerprint);
}

#[test]
fn history_back_restores_child_beneath_preserved_parent() {
    let mut fixture = nested_fixture_with_depth(2, RoutePattern::path("/").unwrap());
    let original = RouteLocation::new("/items/1");
    let parent_only = RouteLocation::new("/");
    for location in [original.clone(), parent_only.clone()] {
        let request = push_and_settle(&mut fixture.app, fixture.router, location, None, None);
        assert_succeeded(&request_snapshot(&fixture.app, fixture.router, request));
    }
    let before = router_snapshot(&fixture.app, fixture.router);
    assert_eq!(before.active().frames().len(), 1);
    let preserved_parent = before.active().frames()[0].clone();
    let parent_entity = node_entities_with_width(&mut fixture.app, 41)[0];
    fixture
        .app
        .world_mut()
        .entity_mut(parent_entity)
        .insert(LocalState(47));
    let effects_before = fixture.control.effects().len();
    let back = fixture
        .app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .back(fixture.router)
        .unwrap();
    let terminal = drive_request_to_terminal(&mut fixture.app, fixture.router, back);
    assert_succeeded(&terminal);
    let after = router_snapshot(&fixture.app, fixture.router);
    assert_eq!(after.active().location(), Some(&original));
    assert_eq!(after.active().frames().len(), 2);
    assert_eq!(after.active().frames()[0], preserved_parent);
    assert_eq!(
        fixture.app.world().get::<LocalState>(parent_entity),
        Some(&LocalState(47))
    );
    assert_eq!(after.history().entries(), [original, parent_only]);
    assert_eq!(after.history().cursor(), Some(0));
    assert_eq!(fixture.control.effects().len() - effects_before, 1);
    assert_terminal_owner_closure(&fixture.app, fixture.router);
    assert_host_intact(&fixture.app, &fixture.host_fingerprint);
}

#[test]
fn history_back_nested_mount_failure_settles_and_allows_retry() {
    assert_nested_back_failure_allows_retry(
        FixtureMode::MissingOutlet { width: 41 },
        NavigationFailureKind::Mount,
    );
}

#[test]
fn history_back_nested_apply_failure_settles_and_allows_retry() {
    assert_nested_back_failure_allows_retry(
        FixtureMode::ApplyFailure,
        NavigationFailureKind::Apply,
    );
}

#[test]
fn history_back_nested_layout_build_failure_settles_and_allows_retry() {
    assert_nested_back_failure_allows_retry(
        FixtureMode::BuildFailure,
        NavigationFailureKind::Build,
    );
}

fn assert_nested_back_failure_allows_retry(mode: FixtureMode, expected: NavigationFailureKind) {
    let mut fixture = nested_fixture();
    let original = RouteLocation::new("/items/1").with_search("filter=original");
    let next = RouteLocation::new("/items/2");
    for location in [original.clone(), next] {
        let request = push_and_settle(&mut fixture.app, fixture.router, location, None, None);
        assert_succeeded(&request_snapshot(&fixture.app, fixture.router, request));
    }
    let history = router_snapshot(&fixture.app, fixture.router)
        .history()
        .clone();
    let failed_view = if expected != NavigationFailureKind::Apply {
        &fixture.layout
    } else {
        &fixture.leaf
    };
    fixture.control.set_one_shot_mode(failed_view, mode);
    let back = fixture
        .app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .back(fixture.router)
        .unwrap();
    let terminal = drive_request_to_terminal(&mut fixture.app, fixture.router, back);
    assert_failed(&terminal, expected);
    assert_eq!(terminal.requested(), Some(&original));
    assert_eq!(
        router_snapshot(&fixture.app, fixture.router).history(),
        &history
    );
    assert_terminal_owner_closure(&fixture.app, fixture.router);
    assert_host_intact(&fixture.app, &fixture.host_fingerprint);
    if expected == NavigationFailureKind::Mount {
        assert!(
            terminal
                .trace()
                .iter()
                .any(|event| event.mount_rejection() == Some(MountRejectionKind::MissingOutlet))
        );
    } else if expected == NavigationFailureKind::Apply {
        let partial = fixture.control.partial_writes.lock().unwrap();
        assert!(
            !partial.is_empty(),
            "the official BSN apply must actually write before failing"
        );
        assert!(
            partial
                .iter()
                .all(|entity| fixture.app.world().get_entity(*entity).is_err())
        );
    }
    let retry = fixture
        .app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .back(fixture.router)
        .unwrap();
    let terminal = drive_request_to_terminal(&mut fixture.app, fixture.router, retry);
    assert_succeeded(&terminal);
    let after = router_snapshot(&fixture.app, fixture.router);
    assert_eq!(after.history().entries(), history.entries());
    assert_eq!(after.history().cursor(), Some(0));
    assert_eq!(after.active().location(), Some(&original));
    assert_terminal_owner_closure(&fixture.app, fixture.router);
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
            width: NORMAL_CONTENT_WIDTH,
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
    assert_host_intact(&fixture.app, &fixture.host_fingerprint);
}

#[test]
fn native_factory_inputs_drive_equivalent_param_fragment_and_search_updates() {
    let mut fixture = nested_fixture();
    let initial = RouteLocation::new("items//1/")
        .with_search("?tag=a&tag=b")
        .with_fragment("#first");
    let first = push_and_settle(
        &mut fixture.app,
        fixture.router,
        initial.clone(),
        None,
        None,
    );
    assert_succeeded(&request_snapshot(&fixture.app, fixture.router, first));
    let initial_frames = router_snapshot(&fixture.app, fixture.router)
        .active()
        .frames()
        .to_vec();
    let layout = node_entities_with_width(&mut fixture.app, 41)[0];
    let leaf = node_entities_with_width(&mut fixture.app, 43)[0];
    let intermediate = fixture.app.world().get::<Children>(layout).unwrap()[0];
    let anchor = fixture.app.world().get::<Children>(intermediate).unwrap()[0];
    assert!(fixture.app.world().get::<DefaultOutlet>(anchor).is_some());
    assert_ne!(
        anchor, layout,
        "the outlet is an ordinary internal container"
    );
    let slot = fixture.app.world().get::<Children>(anchor).unwrap()[0];
    fixture
        .app
        .world_mut()
        .get_mut::<LocalState>(layout)
        .unwrap()
        .0 = 73;
    let effects = fixture.control.effects();
    assert_eq!(effects.len(), 2);
    for (effect, route, param) in [
        (&effects[0], fixture.layout_route, None),
        (&effects[1], fixture.leaf_route, Some("1")),
    ] {
        assert_eq!(effect.location.path(), "/items/1");
        assert_eq!(effect.search.as_deref(), Some("?tag=a&tag=b"));
        assert_eq!(effect.fragment.as_deref(), Some("#first"));
        assert_eq!(effect.route, Some(route));
        assert_eq!(effect.origin, FrameOrigin::MatchedRoute(route));
        assert_eq!(effect.id_param.as_deref(), param);
        assert_eq!(effect.failure, None);
    }

    let entities = entity_set(&fixture.app);
    fixture.app.world_mut().clear_trackers();
    let tick = fixture.app.world().last_change_tick();
    let equivalent = RouteLocation::new("//items/1//")
        .with_search("?tag=a&tag=b")
        .with_fragment("#first");
    let request = replace_and_settle(&mut fixture.app, fixture.router, equivalent, None);
    assert_succeeded(&request_snapshot(&fixture.app, fixture.router, request));
    assert_eq!(fixture.control.effects(), effects);
    assert_eq!(entity_set(&fixture.app), entities);
    assert_eq!(
        router_snapshot(&fixture.app, fixture.router)
            .active()
            .frames(),
        initial_frames
    );
    for entity in [layout, intermediate, anchor, slot, leaf] {
        assert_manifest_not_added_or_changed(&fixture.app, entity, tick);
    }

    let changed_param = RouteLocation::new("/items/2")
        .with_search("?tag=a&tag=b")
        .with_fragment("#first");
    let request = replace_and_settle(&mut fixture.app, fixture.router, changed_param, None);
    assert_succeeded(&request_snapshot(&fixture.app, fixture.router, request));
    let param_frames = router_snapshot(&fixture.app, fixture.router)
        .active()
        .frames()
        .to_vec();
    assert_eq!(param_frames[0], initial_frames[0]);
    assert_eq!(param_frames[1].slot(), initial_frames[1].slot());
    assert_ne!(param_frames[1].frame(), initial_frames[1].frame());
    assert!(fixture.app.world().get_entity(leaf).is_err());
    assert_eq!(fixture.control.effects().len(), 3);
    assert_eq!(fixture.control.effects()[2].view, fixture.leaf);
    assert_eq!(fixture.control.effects()[2].id_param.as_deref(), Some("2"));

    let changed_fragment = RouteLocation::new("/items/2")
        .with_search("?tag=a&tag=b")
        .with_fragment("#next");
    let request = replace_and_settle(
        &mut fixture.app,
        fixture.router,
        changed_fragment.clone(),
        None,
    );
    assert_succeeded(&request_snapshot(&fixture.app, fixture.router, request));
    let fragment_frames = router_snapshot(&fixture.app, fixture.router)
        .active()
        .frames()
        .to_vec();
    assert_eq!(fragment_frames[0], initial_frames[0]);
    assert_eq!(fragment_frames[1].frame(), param_frames[1].frame());
    assert_eq!(fragment_frames[1].slot(), param_frames[1].slot());
    assert_eq!(
        fragment_frames[1].generation().raw(),
        param_frames[1].generation().raw() + 1
    );
    assert_eq!(fixture.control.effects().len(), 4);
    assert_eq!(
        fixture.control.effects()[3].fragment.as_deref(),
        Some("#next")
    );
    assert_eq!(
        fixture.app.world().get::<LocalState>(layout),
        Some(&LocalState(73))
    );
    assert_eq!(
        fixture.app.world().get::<ChildOf>(anchor).unwrap().parent(),
        intermediate
    );
    assert_eq!(
        fixture.app.world().get::<ChildOf>(slot).unwrap().parent(),
        anchor
    );
    assert_eq!(
        &fixture.app.world().get::<Children>(anchor).unwrap()[..],
        &[slot]
    );

    // Raw search is intentionally order-sensitive, even with the same keys.
    let changed_search = RouteLocation::new("/items/2")
        .with_search("?tag=b&tag=a")
        .with_fragment("#next");
    let request = replace_and_settle(
        &mut fixture.app,
        fixture.router,
        changed_search.clone(),
        None,
    );
    assert_succeeded(&request_snapshot(&fixture.app, fixture.router, request));
    let after = router_snapshot(&fixture.app, fixture.router);
    assert_eq!(after.history().entries(), [changed_search.clone()]);
    assert_eq!(after.history().cursor(), Some(0));
    for (old, new) in fragment_frames.iter().zip(after.active().frames()) {
        assert_ne!(old.frame(), new.frame());
    }
    for entity in [layout, intermediate, anchor, slot] {
        assert!(fixture.app.world().get_entity(entity).is_err());
    }
    let effects = fixture.control.effects();
    assert_eq!(effects.len(), 6);
    assert_eq!(effects[4].view, fixture.layout);
    assert_eq!(effects[4].id_param, None);
    assert_eq!(effects[5].view, fixture.leaf);
    assert_eq!(effects[5].id_param.as_deref(), Some("2"));
    for effect in &effects[4..] {
        assert_eq!(effect.location, changed_search);
        assert_eq!(effect.search.as_deref(), Some("?tag=b&tag=a"));
        assert_eq!(effect.fragment.as_deref(), Some("#next"));
    }
    assert_terminal_owner_closure(&fixture.app, fixture.router);
    assert_host_intact(&fixture.app, &fixture.host_fingerprint);
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
        Some(PARTIAL_CONTENT_WIDTH),
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
    assert_native_content(&mut fixture.app, 41);
    assert_native_content(&mut fixture.app, 43);

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
fn retained_native_hit_preserves_state_and_skips_factory_and_scene_apply() {
    let mut fixture =
        nested_fixture_with_retention(RoutePattern::path("/:section").unwrap(), Vec::new(), 2);
    let scene_writes = Arc::new(AtomicUsize::new(0));
    let observed = scene_writes.clone();
    fixture
        .app
        .world_mut()
        .add_observer(move |_: On<Insert, LocalState>| {
            observed.fetch_add(1, Ordering::Relaxed);
        });
    let first = push_and_settle(
        &mut fixture.app,
        fixture.router,
        RouteLocation::new("/catalog/items/1"),
        None,
        None,
    );
    assert_succeeded(&request_snapshot(&fixture.app, fixture.router, first));
    let original = node_entities_with_width(&mut fixture.app, 43)[0];
    let first_frames = router_snapshot(&fixture.app, fixture.router)
        .active()
        .frames()
        .to_vec();
    fixture
        .app
        .world_mut()
        .get_mut::<LocalState>(original)
        .unwrap()
        .0 = 91;

    let second = push_and_settle(
        &mut fixture.app,
        fixture.router,
        RouteLocation::new("/catalog/items/2"),
        None,
        None,
    );
    assert_succeeded(&request_snapshot(&fixture.app, fixture.router, second));
    let second_entity = node_entities_with_width(&mut fixture.app, 43)
        .into_iter()
        .find(|entity| *entity != original)
        .unwrap();
    let effects = fixture.control.effects();
    let writes = scene_writes.load(Ordering::Relaxed);
    assert!(
        writes > 0,
        "the real Scene executor must have applied the fixture"
    );
    assert!(
        !fixture
            .app
            .world()
            .resource::<RouterInputGate>()
            .allows_pointer(original)
    );

    let hit = push_and_settle(
        &mut fixture.app,
        fixture.router,
        RouteLocation::new("/catalog/items/1"),
        None,
        None,
    );
    assert_succeeded(&request_snapshot(&fixture.app, fixture.router, hit));
    let leaves = node_entities_with_width(&mut fixture.app, 43);
    let gate = fixture.app.world().resource::<RouterInputGate>();
    let active = leaves
        .iter()
        .copied()
        .filter(|entity| gate.allows_pointer(*entity))
        .collect::<Vec<_>>();
    assert_eq!(
        active,
        [original],
        "native cache hit must activate the original Entity"
    );
    assert!(gate.allows_keyboard(original) && gate.allows_gamepad(original));
    assert!(!gate.allows_pointer(second_entity));
    assert_eq!(
        fixture.app.world().get::<LocalState>(original),
        Some(&LocalState(91))
    );
    assert_eq!(
        fixture.control.effects(),
        effects,
        "hit must bypass the BSN factory"
    );
    assert_eq!(
        scene_writes.load(Ordering::Relaxed),
        writes,
        "hit must not reapply the Scene"
    );
    assert_eq!(
        router_snapshot(&fixture.app, fixture.router)
            .active()
            .frames(),
        first_frames
    );
    assert_host_intact(&fixture.app, &fixture.host_fingerprint);

    let outside = push_and_settle(
        &mut fixture.app,
        fixture.router,
        RouteLocation::new("/other/items/3"),
        None,
        None,
    );
    assert_succeeded(&request_snapshot(&fixture.app, fixture.router, outside));
    assert!(fixture.app.world().get_entity(original).is_err());
    assert!(fixture.app.world().get_entity(second_entity).is_err());
    assert_host_intact(&fixture.app, &fixture.host_fingerprint);
}

#[test]
fn retained_native_empty_slot_hit_preserves_fragment_and_raw_search_contracts() {
    let detail = RouteViewKey::new("nested-detail").unwrap();
    let mut fixture = nested_fixture_with_retention_and_detail(
        RoutePattern::path("/:section").unwrap(),
        Vec::new(),
        2,
        Some(detail.clone()),
    );
    let location = |path: &str, fragment: &str| {
        RouteLocation::new(path)
            .with_search("?tag=a&tag=b")
            .with_fragment(fragment)
    };
    let push = |fixture: &mut NestedFixture, target: RouteLocation| {
        let request = push_and_settle(&mut fixture.app, fixture.router, target, None, None);
        assert_succeeded(&request_snapshot(&fixture.app, fixture.router, request));
        assert_terminal_owner_closure(&fixture.app, fixture.router);
    };
    let active_leaf = |app: &mut App| {
        let leaves = node_entities_with_width(app, 43);
        let gate = app.world().resource::<RouterInputGate>();
        let active = leaves
            .into_iter()
            .filter(|entity| gate.allows_pointer(*entity))
            .collect::<Vec<_>>();
        assert_eq!(active.len(), 1);
        active[0]
    };

    push(&mut fixture, location("/catalog/items/1/detail", "#first"));
    let initial_frames = router_snapshot(&fixture.app, fixture.router)
        .active()
        .frames()
        .to_vec();
    assert_eq!(initial_frames.len(), 3);
    let layout = node_entities_with_width(&mut fixture.app, 41)[0];
    let original = active_leaf(&mut fixture.app);
    fixture
        .app
        .world_mut()
        .get_mut::<LocalState>(original)
        .unwrap()
        .0 = 91;
    let first_detail = node_entities_with_width(&mut fixture.app, 47)[0];
    let first_detail_shell = fixture
        .app
        .world()
        .get::<ChildOf>(first_detail)
        .unwrap()
        .parent();
    let empty_slot = fixture
        .app
        .world()
        .get::<ChildOf>(first_detail_shell)
        .unwrap()
        .parent();
    let own_anchor = fixture
        .app
        .world()
        .get::<ChildOf>(empty_slot)
        .unwrap()
        .parent();
    assert!(
        fixture
            .app
            .world()
            .get::<DefaultOutlet>(own_anchor)
            .is_some()
    );

    // A real child departure leaves its parent's original, now empty slot.
    push(&mut fixture, location("/catalog/items/1", "#first"));
    assert_eq!(
        router_snapshot(&fixture.app, fixture.router)
            .active()
            .frames(),
        &initial_frames[..2]
    );
    assert!(fixture.app.world().get_entity(first_detail).is_err());
    assert!(fixture.app.world().get_entity(first_detail_shell).is_err());
    assert!(fixture.app.world().get_entity(empty_slot).is_ok());
    assert!(
        fixture
            .app
            .world()
            .get::<Children>(empty_slot)
            .is_none_or(|children| children.is_empty())
    );
    assert_eq!(active_leaf(&mut fixture.app), original);

    push(&mut fixture, location("/catalog/items/2", "#first"));
    let other_leaf = active_leaf(&mut fixture.app);
    assert_ne!(other_leaf, original);
    assert!(fixture.app.world().get_entity(original).is_ok());
    assert!(fixture.app.world().get_entity(empty_slot).is_ok());
    assert!(
        !fixture
            .app
            .world()
            .resource::<RouterInputGate>()
            .allows_pointer(original)
    );
    let effects_before_hit = fixture.control.effects();

    push(&mut fixture, location("/catalog/items/1", "#first"));
    assert_eq!(active_leaf(&mut fixture.app), original);
    assert_eq!(
        fixture.app.world().get::<LocalState>(original),
        Some(&LocalState(91))
    );
    assert_eq!(fixture.control.effects(), effects_before_hit);
    assert_eq!(
        fixture
            .app
            .world()
            .get::<ChildOf>(empty_slot)
            .unwrap()
            .parent(),
        own_anchor
    );
    assert!(
        fixture
            .app
            .world()
            .get::<Children>(empty_slot)
            .is_none_or(|children| children.is_empty())
    );
    assert_eq!(
        router_snapshot(&fixture.app, fixture.router)
            .active()
            .frames(),
        &initial_frames[..2]
    );

    push(&mut fixture, location("/catalog/items/1/detail", "#first"));
    let appended_frames = router_snapshot(&fixture.app, fixture.router)
        .active()
        .frames()
        .to_vec();
    assert_eq!(&appended_frames[..2], &initial_frames[..2]);
    assert_eq!(appended_frames[2].slot(), initial_frames[2].slot());
    let second_detail = node_entities_with_width(&mut fixture.app, 47)[0];
    assert_ne!(second_detail, first_detail);
    let second_detail_shell = fixture
        .app
        .world()
        .get::<ChildOf>(second_detail)
        .unwrap()
        .parent();
    assert_eq!(
        fixture
            .app
            .world()
            .get::<ChildOf>(second_detail_shell)
            .unwrap()
            .parent(),
        empty_slot
    );
    assert_eq!(
        fixture
            .app
            .world()
            .get::<Children>(empty_slot)
            .unwrap()
            .as_ref(),
        &[second_detail_shell]
    );
    assert_eq!(active_leaf(&mut fixture.app), original);
    let effects_after_append = fixture.control.effects();
    assert_eq!(effects_after_append.len(), effects_before_hit.len() + 1);
    assert_eq!(effects_after_append.last().unwrap().view, detail);
    push(&mut fixture, location("/catalog/items/1", "#first"));
    assert!(fixture.app.world().get_entity(second_detail).is_err());
    assert!(fixture.app.world().get_entity(empty_slot).is_ok());
    assert_eq!(fixture.control.effects(), effects_after_append);

    // Even when the earlier fragment is cached, fragment-only replacement
    // rebuilds the deepest leaf and preserves the published parent instance.
    let mut prior_entity = original;
    let mut prior_frame = initial_frames[1].clone();
    for fragment in ["#next", "#first"] {
        let effects = fixture.control.effects();
        let request = replace_and_settle(
            &mut fixture.app,
            fixture.router,
            location("/catalog/items/1", fragment),
            None,
        );
        assert_succeeded(&request_snapshot(&fixture.app, fixture.router, request));
        assert_terminal_owner_closure(&fixture.app, fixture.router);
        let frames = router_snapshot(&fixture.app, fixture.router)
            .active()
            .frames()
            .to_vec();
        assert_eq!(frames[0], initial_frames[0]);
        assert_eq!(frames[1].frame(), prior_frame.frame());
        assert_eq!(frames[1].slot(), prior_frame.slot());
        assert_eq!(
            frames[1].generation().raw(),
            prior_frame.generation().raw() + 1
        );
        let current = active_leaf(&mut fixture.app);
        assert_ne!(current, prior_entity);
        assert_ne!(
            current, original,
            "fragment replacement must not reactivate cached A"
        );
        let after = fixture.control.effects();
        assert_eq!(after.len(), effects.len() + 1);
        assert_eq!(&after[..effects.len()], effects.as_slice());
        assert_eq!(after.last().unwrap().view, fixture.leaf);
        assert_eq!(after.last().unwrap().fragment.as_deref(), Some(fragment));
        prior_entity = current;
        prior_frame = frames[1].clone();
    }
    assert!(fixture.app.world().get_entity(original).is_ok());
    assert_eq!(
        fixture.app.world().get::<LocalState>(original),
        Some(&LocalState(91))
    );
    let old_leaves = node_entities_with_width(&mut fixture.app, 43);
    assert_eq!(
        old_leaves.len(),
        3,
        "two inactive leaves remain before scope replacement"
    );
    let before_search = router_snapshot(&fixture.app, fixture.router)
        .active()
        .frames()
        .to_vec();
    let effects = fixture.control.effects();
    let changed_search = RouteLocation::new("/catalog/items/1")
        .with_search("?tag=b&tag=a")
        .with_fragment("#first");
    let request = replace_and_settle(
        &mut fixture.app,
        fixture.router,
        changed_search.clone(),
        None,
    );
    assert_succeeded(&request_snapshot(&fixture.app, fixture.router, request));
    assert_terminal_owner_closure(&fixture.app, fixture.router);
    let after_search = router_snapshot(&fixture.app, fixture.router);
    assert_eq!(after_search.active().location(), Some(&changed_search));
    for (old, new) in before_search.iter().zip(after_search.active().frames()) {
        assert_ne!(
            old.frame(),
            new.frame(),
            "raw search changes rebuild the entire branch"
        );
    }
    for old in old_leaves
        .into_iter()
        .chain([layout, empty_slot, own_anchor])
    {
        assert!(
            fixture.app.world().get_entity(old).is_err(),
            "old scope Entity {old:?} must settle"
        );
    }
    assert_eq!(node_entities_with_width(&mut fixture.app, 41).len(), 1);
    assert_eq!(node_entities_with_width(&mut fixture.app, 43).len(), 1);
    let after = fixture.control.effects();
    assert_eq!(after.len(), effects.len() + 2);
    assert_eq!(&after[..effects.len()], effects.as_slice());
    assert_eq!(after[effects.len()].view, fixture.layout);
    assert_eq!(after[effects.len() + 1].view, fixture.leaf);
    for effect in &after[effects.len()..] {
        assert_eq!(effect.search.as_deref(), Some("?tag=b&tag=a"));
        assert_eq!(effect.fragment.as_deref(), Some("#first"));
    }
    assert_host_intact(&fixture.app, &fixture.host_fingerprint);
}

#[test]
fn retained_native_missing_own_outlet_settles_cache_and_resumes_the_original_request() {
    let detail = RouteViewKey::new("nested-detail").unwrap();
    let mut fixture = nested_fixture_with_retention_and_detail(
        RoutePattern::path("/:section").unwrap(),
        Vec::new(),
        2,
        Some(detail),
    );
    let a = RouteLocation::new("/catalog/items/1");
    let b = RouteLocation::new("/catalog/items/2");
    let detail_location = RouteLocation::new("/catalog/items/1/detail");
    let active_leaf = |app: &mut App| {
        let leaves = node_entities_with_width(app, 43);
        let gate = app.world().resource::<RouterInputGate>();
        let active = leaves
            .into_iter()
            .filter(|entity| gate.allows_pointer(*entity))
            .collect::<Vec<_>>();
        assert_eq!(active.len(), 1);
        active[0]
    };

    for target in [detail_location.clone(), a.clone()] {
        let request = push_and_settle(&mut fixture.app, fixture.router, target, None, None);
        assert_succeeded(&request_snapshot(&fixture.app, fixture.router, request));
        assert_terminal_owner_closure(&fixture.app, fixture.router);
    }
    let original_a = active_leaf(&mut fixture.app);
    let layout = node_entities_with_width(&mut fixture.app, 41)[0];
    let parent_frame = router_snapshot(&fixture.app, fixture.router)
        .active()
        .frames()[0]
        .clone();
    fixture
        .app
        .world_mut()
        .get_mut::<LocalState>(original_a)
        .unwrap()
        .0 = 91;
    fixture
        .app
        .world_mut()
        .get_mut::<LocalState>(layout)
        .unwrap()
        .0 = 73;
    let container = fixture.app.world().get::<Children>(original_a).unwrap()[0];
    let own_anchor = fixture.app.world().get::<Children>(container).unwrap()[0];
    let own_slot = fixture.app.world().get::<Children>(own_anchor).unwrap()[0];
    assert!(
        fixture
            .app
            .world()
            .get::<DefaultOutlet>(own_anchor)
            .is_some()
    );
    assert!(
        fixture
            .app
            .world()
            .get::<Children>(own_slot)
            .is_none_or(|children| children.is_empty())
    );
    assert!(node_entities_with_width(&mut fixture.app, 47).is_empty());

    let request = push_and_settle(&mut fixture.app, fixture.router, b.clone(), None, None);
    assert_succeeded(&request_snapshot(&fixture.app, fixture.router, request));
    assert_terminal_owner_closure(&fixture.app, fixture.router);
    let original_b = active_leaf(&mut fixture.app);
    fixture
        .app
        .world_mut()
        .get_mut::<LocalState>(original_b)
        .unwrap()
        .0 = 92;
    assert_ne!(original_a, original_b);
    assert!(fixture.app.world().get_entity(original_a).is_ok());
    assert!(
        !fixture
            .app
            .world()
            .resource::<RouterInputGate>()
            .allows_pointer(original_a)
    );
    let before = router_snapshot(&fixture.app, fixture.router);
    let effects = fixture.control.effects();

    // Only ordinary Native content changes. The cached shell, owned empty
    // slot, registry claims, shared parent outlet and active B stay untouched.
    fixture
        .app
        .world_mut()
        .entity_mut(own_anchor)
        .remove::<DefaultOutlet>();
    assert_eq!(active_leaf(&mut fixture.app), original_b);
    assert_eq!(
        fixture
            .app
            .world()
            .get::<ChildOf>(own_slot)
            .unwrap()
            .parent(),
        own_anchor
    );

    let resumed = push_and_settle(&mut fixture.app, fixture.router, a.clone(), None, None);
    let resumed_request = request_snapshot(&fixture.app, fixture.router, resumed);
    assert_succeeded(&resumed_request);
    assert_eq!(resumed_request.requested(), Some(&a));
    assert_terminal_owner_closure(&fixture.app, fixture.router);
    let fresh_a = active_leaf(&mut fixture.app);
    assert_ne!(fresh_a, original_a);
    assert_eq!(
        fixture.app.world().get::<LocalState>(fresh_a),
        Some(&LocalState(0))
    );
    for old in [original_a, container, own_anchor, own_slot] {
        assert!(
            fixture.app.world().get_entity(old).is_err(),
            "old cache Entity {old:?} must settle"
        );
    }
    assert_eq!(
        fixture.app.world().get::<LocalState>(original_b),
        Some(&LocalState(92))
    );
    assert_eq!(
        fixture.app.world().get::<LocalState>(layout),
        Some(&LocalState(73))
    );
    let after = router_snapshot(&fixture.app, fixture.router);
    assert_eq!(after.active().frames()[0], parent_frame);
    assert_eq!(before.active().frames()[0], parent_frame);
    assert_eq!(
        after.history().entries(),
        [detail_location, a.clone(), b.clone(), a.clone()]
    );
    let after_effects = fixture.control.effects();
    assert_eq!(after_effects.len(), effects.len() + 1);
    assert_eq!(&after_effects[..effects.len()], effects.as_slice());
    assert_eq!(after_effects.last().unwrap().view, fixture.leaf);
    assert_eq!(after_effects.last().unwrap().id_param.as_deref(), Some("1"));
    assert_host_intact(&fixture.app, &fixture.host_fingerprint);

    let hit_b = push_and_settle(&mut fixture.app, fixture.router, b, None, None);
    assert_succeeded(&request_snapshot(&fixture.app, fixture.router, hit_b));
    assert_terminal_owner_closure(&fixture.app, fixture.router);
    assert_eq!(active_leaf(&mut fixture.app), original_b);
    assert_eq!(fixture.control.effects(), after_effects);
    let back = fixture
        .app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .back(fixture.router)
        .unwrap();
    let back_request = drive_request_to_terminal(&mut fixture.app, fixture.router, back);
    assert_succeeded(&back_request);
    assert_eq!(back_request.requested(), Some(&a));
    assert_terminal_owner_closure(&fixture.app, fixture.router);
    assert_eq!(active_leaf(&mut fixture.app), fresh_a);
    assert_eq!(fixture.control.effects(), after_effects);
    assert_eq!(
        fixture.app.world().get::<LocalState>(original_b),
        Some(&LocalState(92))
    );
    assert_eq!(
        router_snapshot(&fixture.app, fixture.router)
            .active()
            .frames()[0],
        parent_frame
    );
    assert_host_intact(&fixture.app, &fixture.host_fingerprint);
}

fn active_native_retained_leaf(app: &mut App) -> Entity {
    let leaves = node_entities_with_width(app, 43);
    let gate = app.world().resource::<RouterInputGate>();
    let active = leaves
        .into_iter()
        .filter(|entity| gate.allows_pointer(*entity))
        .collect::<Vec<_>>();
    assert_eq!(active.len(), 1);
    active[0]
}

struct InvalidRetainedOutletFixture {
    fixture: NestedFixture,
    cached_root: Entity,
    own_anchor: Entity,
    own_slot: Entity,
    active_root: Entity,
    layout_root: Entity,
}

fn invalid_retained_outlet_fixture() -> InvalidRetainedOutletFixture {
    let mut fixture = nested_fixture_with_retention_and_detail(
        RoutePattern::path("/:section").unwrap(),
        Vec::new(),
        2,
        Some(RouteViewKey::new("nested-detail").unwrap()),
    );
    for path in ["/catalog/items/1/detail", "/catalog/items/1"] {
        let request = push_and_settle(
            &mut fixture.app,
            fixture.router,
            RouteLocation::new(path),
            None,
            None,
        );
        assert_succeeded(&request_snapshot(&fixture.app, fixture.router, request));
        assert_terminal_owner_closure(&fixture.app, fixture.router);
    }
    let cached_root = active_native_retained_leaf(&mut fixture.app);
    let layout_root = node_entities_with_width(&mut fixture.app, 41)[0];
    fixture
        .app
        .world_mut()
        .get_mut::<LocalState>(cached_root)
        .unwrap()
        .0 = 91;
    fixture
        .app
        .world_mut()
        .get_mut::<LocalState>(layout_root)
        .unwrap()
        .0 = 73;
    let container = fixture.app.world().get::<Children>(cached_root).unwrap()[0];
    let own_anchor = fixture.app.world().get::<Children>(container).unwrap()[0];
    let own_slot = fixture.app.world().get::<Children>(own_anchor).unwrap()[0];
    assert!(
        fixture
            .app
            .world()
            .get::<DefaultOutlet>(own_anchor)
            .is_some()
    );
    assert!(
        fixture
            .app
            .world()
            .get::<Children>(own_slot)
            .is_none_or(|children| children.is_empty())
    );
    assert!(node_entities_with_width(&mut fixture.app, 47).is_empty());

    let request = push_and_settle(
        &mut fixture.app,
        fixture.router,
        RouteLocation::new("/catalog/items/2"),
        None,
        None,
    );
    assert_succeeded(&request_snapshot(&fixture.app, fixture.router, request));
    assert_terminal_owner_closure(&fixture.app, fixture.router);
    let active_root = active_native_retained_leaf(&mut fixture.app);
    assert_ne!(active_root, cached_root);
    assert!(fixture.app.world().get_entity(cached_root).is_ok());
    assert!(
        !fixture
            .app
            .world()
            .resource::<RouterInputGate>()
            .allows_pointer(cached_root)
    );
    fixture
        .app
        .world_mut()
        .get_mut::<LocalState>(active_root)
        .unwrap()
        .0 = 92;

    // Invalidate only the inactive leaf's own empty outlet declaration. Its
    // shell, slot, shared parent and current sibling retain their identities.
    fixture
        .app
        .world_mut()
        .entity_mut(own_anchor)
        .remove::<DefaultOutlet>();
    assert_eq!(active_native_retained_leaf(&mut fixture.app), active_root);
    assert_eq!(
        fixture
            .app
            .world()
            .get::<ChildOf>(own_slot)
            .unwrap()
            .parent(),
        own_anchor
    );
    InvalidRetainedOutletFixture {
        fixture,
        cached_root,
        own_anchor,
        own_slot,
        active_root,
        layout_root,
    }
}

#[test]
fn retained_native_invalid_non_target_own_outlet_preserves_fresh_sibling_navigation() {
    let InvalidRetainedOutletFixture {
        mut fixture,
        cached_root,
        own_anchor,
        own_slot,
        active_root,
        layout_root,
    } = invalid_retained_outlet_fixture();
    let before = router_snapshot(&fixture.app, fixture.router);
    let effects = fixture.control.effects();
    let c = RouteLocation::new("/catalog/items/3");

    let request = push_and_settle(&mut fixture.app, fixture.router, c.clone(), None, None);
    assert_succeeded(&request_snapshot(&fixture.app, fixture.router, request));
    assert_terminal_owner_closure(&fixture.app, fixture.router);
    let fresh_c = active_native_retained_leaf(&mut fixture.app);
    assert_ne!(fresh_c, cached_root);
    assert_ne!(fresh_c, active_root);
    let after = router_snapshot(&fixture.app, fixture.router);
    assert_eq!(after.availability(), &RouterAvailability::Ready);
    assert_eq!(after.active().frames()[0], before.active().frames()[0]);
    assert_eq!(
        after.history().entries(),
        [
            RouteLocation::new("/catalog/items/1/detail"),
            RouteLocation::new("/catalog/items/1"),
            RouteLocation::new("/catalog/items/2"),
            c,
        ]
    );
    for (entity, state) in [(cached_root, 91), (active_root, 92), (layout_root, 73)] {
        assert_eq!(
            fixture.app.world().get::<LocalState>(entity),
            Some(&LocalState(state))
        );
    }
    assert!(
        fixture
            .app
            .world()
            .get::<DefaultOutlet>(own_anchor)
            .is_none()
    );
    assert_eq!(
        fixture
            .app
            .world()
            .get::<ChildOf>(own_slot)
            .unwrap()
            .parent(),
        own_anchor
    );
    let gate = fixture.app.world().resource::<RouterInputGate>();
    for inactive in [cached_root, active_root] {
        assert!(!gate.allows_pointer(inactive));
        assert!(!gate.allows_keyboard(inactive));
        assert!(!gate.allows_gamepad(inactive));
    }
    assert!(gate.allows_keyboard(fresh_c) && gate.allows_gamepad(fresh_c));
    let after_effects = fixture.control.effects();
    assert_eq!(after_effects.len(), effects.len() + 1);
    assert_eq!(&after_effects[..effects.len()], effects.as_slice());
    assert_eq!(after_effects.last().unwrap().view, fixture.leaf);
    assert_eq!(after_effects.last().unwrap().id_param.as_deref(), Some("3"));
    assert_host_intact(&fixture.app, &fixture.host_fingerprint);
}

#[test]
fn retained_native_lru_evicts_the_oldest_inactive_leaf_and_rebuilds_on_return() {
    let mut fixture =
        nested_fixture_with_retention(RoutePattern::path("/:section").unwrap(), Vec::new(), 2);
    let scene_writes = Arc::new(AtomicUsize::new(0));
    let observed = scene_writes.clone();
    fixture
        .app
        .world_mut()
        .add_observer(move |_: On<Insert, LocalState>| {
            observed.fetch_add(1, Ordering::Relaxed);
        });
    let mut originals = Vec::new();
    for (step, (id, fresh)) in [
        (1, true),
        (2, true),
        (3, true),
        (1, false),
        (4, true),
        (1, false),
        (2, true),
    ]
    .into_iter()
    .enumerate()
    {
        let effects_before = fixture.control.effects();
        let writes_before = scene_writes.load(Ordering::Relaxed);
        let request = push_and_settle(
            &mut fixture.app,
            fixture.router,
            RouteLocation::new(format!("/catalog/items/{id}")),
            None,
            None,
        );
        assert_succeeded(&request_snapshot(&fixture.app, fixture.router, request));
        let leaves = node_entities_with_width(&mut fixture.app, 43);
        assert!(
            leaves.len() <= 3,
            "only two inactive leaves plus the active leaf may remain"
        );
        let gate = fixture.app.world().resource::<RouterInputGate>();
        let active = leaves
            .iter()
            .copied()
            .filter(|entity| gate.allows_pointer(*entity))
            .collect::<Vec<_>>();
        assert_eq!(active.len(), 1);
        let current = active[0];
        assert!(
            leaves
                .iter()
                .filter(|entity| **entity != current)
                .all(|entity| {
                    !gate.allows_pointer(*entity)
                        && !gate.allows_keyboard(*entity)
                        && !gate.allows_gamepad(*entity)
                })
        );
        if let Some((_, original)) = originals.iter().find(|(key, _)| *key == id) {
            if fresh {
                assert_ne!(current, *original, "an evicted leaf must be rebuilt");
                assert!(fixture.app.world().get_entity(*original).is_err());
            } else {
                assert_eq!(current, *original, "an LRU hit keeps the original Entity");
                assert_eq!(
                    fixture.app.world().get::<LocalState>(current),
                    Some(&LocalState(90 + id))
                );
            }
        } else {
            originals.push((id, current));
            fixture
                .app
                .world_mut()
                .get_mut::<LocalState>(current)
                .unwrap()
                .0 = 90 + id;
        }
        let effects_after = fixture.control.effects();
        let fresh_leaf_calls = effects_after
            .iter()
            .filter(|effect| effect.view == fixture.leaf)
            .count()
            - effects_before
                .iter()
                .filter(|effect| effect.view == fixture.leaf)
                .count();
        assert_eq!(fresh_leaf_calls, usize::from(fresh));
        if fresh {
            assert!(scene_writes.load(Ordering::Relaxed) > writes_before);
        } else {
            assert_eq!(scene_writes.load(Ordering::Relaxed), writes_before);
        }
        if step == 4 {
            let oldest = originals.iter().find(|(id, _)| *id == 2).unwrap().1;
            let recently_used = originals.iter().find(|(id, _)| *id == 1).unwrap().1;
            let other_cached = originals.iter().find(|(id, _)| *id == 3).unwrap().1;
            assert!(
                fixture.app.world().get_entity(oldest).is_err(),
                "B is the oldest inactive leaf after A was reactivated"
            );
            assert!(fixture.app.world().get_entity(recently_used).is_ok());
            assert_eq!(
                fixture.app.world().get::<LocalState>(other_cached),
                Some(&LocalState(93)),
                "eviction must preserve the other inactive leaf"
            );
        }
        assert_host_intact(&fixture.app, &fixture.host_fingerprint);
    }
    let leaf_calls = fixture
        .control
        .effects()
        .into_iter()
        .filter(|effect| effect.view == fixture.leaf)
        .collect::<Vec<_>>();
    assert_eq!(
        leaf_calls
            .iter()
            .filter(|effect| effect.id_param.as_deref() == Some("1"))
            .count(),
        1
    );
    assert_eq!(
        leaf_calls
            .iter()
            .filter(|effect| effect.id_param.as_deref() == Some("2"))
            .count(),
        2
    );
}

#[test]
fn retained_native_host_loss_cleans_cache_and_readmits_without_replay() {
    let mut fixture =
        nested_fixture_with_retention(RoutePattern::path("/:section").unwrap(), Vec::new(), 2);
    let mut previous_requests = Vec::new();
    for path in ["/catalog/items/1", "/catalog/items/2", "/catalog/items/3"] {
        previous_requests.push(push_and_settle(
            &mut fixture.app,
            fixture.router,
            RouteLocation::new(path),
            None,
            None,
        ));
    }
    let old_leaves = node_entities_with_width(&mut fixture.app, 43);
    assert_eq!(old_leaves.len(), 3);
    let before = router_snapshot(&fixture.app, fixture.router);
    let effects = fixture.control.effects();
    let host = fixture.host_fingerprint.host;
    let request = fixture
        .app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(fixture.router, RouteLocation::new("/catalog/items/1"))
        .unwrap();
    fixture
        .app
        .world_mut()
        .entity_mut(host)
        .remove::<RouterHostRegistration>();
    let lost = drive_recovery_r3_availability(
        &mut fixture.app,
        fixture.router,
        &RouterAvailability::Unavailable(RouterUnavailableReason::HostLost),
    );
    assert_failed(
        &request_snapshot(&fixture.app, fixture.router, request),
        NavigationFailureKind::HostLost,
    );
    assert_eq!(lost.history(), before.history());
    assert_eq!(lost.active().kind(), ActivePresentationKind::None);
    assert!(
        old_leaves
            .iter()
            .all(|entity| fixture.app.world().get_entity(*entity).is_err())
    );
    assert_eq!(
        fixture.control.effects(),
        effects,
        "host loss must not invoke the queued request's factory"
    );
    assert_host_after_registration_loss(&fixture.app, &fixture.host_fingerprint);
    for previous in previous_requests {
        assert_succeeded(&request_snapshot(&fixture.app, fixture.router, previous));
    }
    fixture
        .app
        .world_mut()
        .entity_mut(host)
        .insert(RouterHostRegistration::new(fixture.router));
    let ready = drive_recovery_r3_availability(
        &mut fixture.app,
        fixture.router,
        &RouterAvailability::Ready,
    );
    assert_ne!(ready.host().version(), before.host().version());
    assert_eq!(ready.history(), before.history());
    assert_eq!(ready.active().kind(), ActivePresentationKind::None);
    assert_eq!(
        fixture.control.effects(),
        effects,
        "readmission does not replay old requests"
    );
    fixture.host_fingerprint.top_slot = fixture
        .app
        .world()
        .get::<Children>(host)
        .unwrap()
        .iter()
        .copied()
        .find(|child| !fixture.host_fingerprint.existing_children.contains(child));
    let next = push_and_settle(
        &mut fixture.app,
        fixture.router,
        RouteLocation::new("/catalog/items/1"),
        None,
        None,
    );
    assert_succeeded(&request_snapshot(&fixture.app, fixture.router, next));
    let fresh = node_entities_with_width(&mut fixture.app, 43);
    assert_eq!(fresh.len(), 1);
    assert!(!old_leaves.contains(&fresh[0]));
    assert!(
        fixture
            .app
            .world()
            .resource::<RouterInputGate>()
            .allows_pointer(fresh[0])
    );
    assert_host_intact(&fixture.app, &fixture.host_fingerprint);
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
    assert_frame_diagnostic(&first_request, first_request_id);
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
    assert_native_content(&mut fixture.app, NORMAL_CONTENT_WIDTH);
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
    assert_frame_diagnostic(&fragment_request, fragment_request_id);
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
    assert_frame_diagnostic(&changed_request, changed_request_id);
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
    assert_native_content(&mut fixture.app, NORMAL_CONTENT_WIDTH);

    let effects = fixture.control.effects();
    assert_eq!(effects.len(), 3, "equivalent push must be driver-zero");
    assert_eq!(effects[0].location, RouteLocation::new("/alpha"));
    assert_eq!(effects[0].params, [("page".to_owned(), "alpha".to_owned())]);
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
    assert_frame_diagnostic(&request, request_id);
    assert!(request.diagnostics().iter().any(|event| {
        event.severity() == DiagnosticSeverity::Error
            && event.request().is_none()
            && event.frame().is_none()
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
    assert_native_content(&mut fixture.app, FALLBACK_CONTENT_WIDTH);

    let effects = fixture.control.effects();
    assert_eq!(effects.len(), 1);
    assert_eq!(effects[0].view, fixture.fallback);
    assert_eq!(effects[0].origin, FrameOrigin::RouterFallback);
    assert_eq!(effects[0].failure, Some(NavigationFailureKind::Match));
    assert_host_intact(&fixture.app, &fixture.host_fingerprint);
}

#[test]
fn root_push_build_resolve_and_apply_failures_preserve_source_through_fallback() {
    for (mode, expected) in [
        (FixtureMode::BuildFailure, NavigationFailureKind::Build),
        (FixtureMode::ResolveFailure, NavigationFailureKind::Build),
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
            (mode == FixtureMode::ApplyFailure).then_some(PARTIAL_CONTENT_WIDTH),
            Some(&old_entities),
        );
        let failed = request_snapshot(&fixture.app, fixture.router, failed_id);
        assert_failed(&failed, expected);
        assert_frame_diagnostic(&failed, failed_id);
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
        assert!(node_entities_with_width(&mut fixture.app, PARTIAL_CONTENT_WIDTH).is_empty());
        assert_native_content(&mut fixture.app, FALLBACK_CONTENT_WIDTH);

        let effects = fixture.control.effects();
        let recovery = effects.last().expect("fallback effect must run");
        assert_eq!(recovery.view, fixture.fallback);
        assert_eq!(recovery.origin, FrameOrigin::RouterFallback);
        assert_eq!(recovery.failure, Some(expected));
        let RequestStatus::Failed(original) = failed.status() else {
            unreachable!()
        };
        assert_eq!(
            recovery.failure_message.as_deref(),
            Some(original.message())
        );
        let source = match mode {
            FixtureMode::BuildFailure => "fixture build failure",
            FixtureMode::ResolveFailure => "fixture-required.scene",
            FixtureMode::ApplyFailure => "fixture apply failure after partial child write",
            _ => unreachable!(),
        };
        assert!(original.message().contains(source), "{original:?}");
        let writes = fixture.control.partial_writes.lock().unwrap();
        assert_eq!(writes.len(), usize::from(mode == FixtureMode::ApplyFailure));
        assert!(
            writes
                .iter()
                .all(|entity| fixture.app.world().get_entity(*entity).is_err())
        );
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
        Some(PARTIAL_CONTENT_WIDTH),
        None,
    );
    let failed = request_snapshot(&fixture.app, fixture.router, failed_id);
    assert_failed(&failed, NavigationFailureKind::RecoveryFailed);
    assert_frame_diagnostic(&failed, failed_id);

    let after_failure = router_snapshot(&fixture.app, fixture.router);
    assert_eq!(after_failure.active(), &old_active);
    assert_eq!(after_failure.history(), &old_history);
    assert_eq!(entity_set(&fixture.app), old_entities);
    assert!(node_entities_with_width(&mut fixture.app, PARTIAL_CONTENT_WIDTH).is_empty());
    assert_native_content(&mut fixture.app, NORMAL_CONTENT_WIDTH);
    assert_host_intact(&fixture.app, &fixture.host_fingerprint);

    fixture.control.set_mode(
        &fixture.normal,
        FixtureMode::Success {
            width: NORMAL_CONTENT_WIDTH,
        },
    );
    fixture.control.set_mode(
        &fixture.fallback,
        FixtureMode::Success {
            width: FALLBACK_CONTENT_WIDTH,
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
    assert_frame_diagnostic(&recovered_request, recovered_id);
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
        ),
        (
            "apply-partial",
            FixtureMode::ApplyFailure,
            NavigationFailureKind::Apply,
        ),
        (
            "resolve",
            FixtureMode::ResolveFailure,
            NavigationFailureKind::Build,
        ),
    ];
    for (label, mode, expected) in rows {
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
        let RequestStatus::Failed(original) = failed.status() else {
            unreachable!()
        };
        assert_eq!(
            recovery.failure_message.as_deref(),
            Some(original.message())
        );
        assert!(
            node_entities_with_width(&mut fixture.app, PARTIAL_CONTENT_WIDTH).is_empty(),
            "row {label} removes failed output before error publication"
        );
        assert_terminal_owner_closure(&fixture.app, fixture.router);
        assert_host_intact(&fixture.app, &fixture.host_fingerprint);
    }

    for (mode, rejection) in [
        (
            FixtureMode::MissingOutlet { width: 53 },
            MountRejectionKind::MissingOutlet,
        ),
        (
            FixtureMode::DuplicateOutlet,
            MountRejectionKind::RejectedAfterMutation,
        ),
        (
            FixtureMode::NonNodeOutlet,
            MountRejectionKind::RejectedAfterMutation,
        ),
    ] {
        let mut fixture = recovery_r1_fixture("r1-invalid-entering-outlet");
        let before = router_snapshot(&fixture.app, fixture.router);
        assert!(before.requested().is_none());
        assert!(before.matched().is_none());
        assert!(before.active().frames().is_empty());
        assert_eq!(before.history().len(), 0);
        fixture.control.set_mode(&fixture.section, mode);
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
                matches!(
                    event.kind(),
                    NavigationTraceEventKind::MountRejected
                        | NavigationTraceEventKind::MountRejectedAfterMutation
                ) && event.mount_rejection() == Some(rejection)
            })
            .unwrap_or_else(|| {
                panic!("invalid entering outlet emits the typed rejection: {mode:?}, {failed:?}")
            });
        assert!(failed.trace()[..rejected].iter().any(|event| {
            event.kind() == NavigationTraceEventKind::MountRequested
                && event.frame() == failed.trace()[rejected].frame()
                && event.outlet() == failed.trace()[rejected].outlet()
        }));
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
        if rejection == MountRejectionKind::MissingOutlet {
            assert!(failed.trace()[..=rejected].iter().all(|event| {
                event.kind() != NavigationTraceEventKind::MountGranted
                    || event.frame() != Some(rejected_frame)
            }));
        }
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
fn recovery_r1_journey_d_nested_push_without_error_binding_publishes_fallback_after_cleanup() {
    let mut fixture = nested_fixture();
    let first_location = RouteLocation::new("/items/1");
    let first = push_and_settle(
        &mut fixture.app,
        fixture.router,
        first_location.clone(),
        None,
        None,
    );
    assert_succeeded(&request_snapshot(&fixture.app, fixture.router, first));
    let before = router_snapshot(&fixture.app, fixture.router);
    let history_before = before.history().clone();
    assert_eq!(before.active().frames().len(), 2);

    fixture
        .control
        .set_one_shot_mode(&fixture.leaf, FixtureMode::ApplyFailure);
    let requested = RouteLocation::new("/items/2");
    let request = fixture
        .app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(fixture.router, requested.clone())
        .unwrap();
    let failed = drive_request_to_terminal(&mut fixture.app, fixture.router, request);

    assert_failed(&failed, NavigationFailureKind::Apply);
    assert_eq!(failed.requested(), Some(&requested));
    assert!(failed.matched().is_some());
    let cleanup = failed
        .trace()
        .iter()
        .position(|event| event.kind() == NavigationTraceEventKind::CleanupSettled)
        .expect("the real failed leaf output must settle cleanup");
    let fallback = failed
        .trace()
        .iter()
        .position(|event| {
            event.kind() == NavigationTraceEventKind::FrameTaskIssued
                && event
                    .frame()
                    .is_some_and(|frame| frame.origin() == &FrameOrigin::RouterFallback)
        })
        .expect("the no-binding continuation must issue RouterFallback");
    let terminal = failed
        .trace()
        .iter()
        .position(|event| event.kind() == NavigationTraceEventKind::RequestTerminal)
        .expect("the fallback publication must terminalize the failed request");
    assert!(cleanup < fallback && fallback < terminal);
    assert_eq!(terminal + 1, failed.trace().len());

    let after = router_snapshot(&fixture.app, fixture.router);
    assert_eq!(after.history(), &history_before);
    assert_eq!(after.active().kind(), ActivePresentationKind::Error);
    assert_eq!(after.active().requested(), Some(&requested));
    assert_eq!(after.active().matched(), failed.matched());
    assert_eq!(
        after.active().failure().map(NavigationFailure::kind),
        Some(NavigationFailureKind::Apply)
    );
    assert_eq!(after.active().frames().len(), 1);
    assert_eq!(
        after.active().frames()[0].origin(),
        &FrameOrigin::RouterFallback
    );
    assert!(node_entities_with_width(&mut fixture.app, PARTIAL_CONTENT_WIDTH).is_empty());
    assert_native_content(&mut fixture.app, FALLBACK_CONTENT_WIDTH);
    let fallback_effect = fixture.control.effects().last().cloned().unwrap();
    assert_eq!(fallback_effect.origin, FrameOrigin::RouterFallback);
    assert_eq!(fallback_effect.failure, Some(NavigationFailureKind::Apply));
    assert_terminal_owner_closure(&fixture.app, fixture.router);
    assert_host_intact(&fixture.app, &fixture.host_fingerprint);
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
    // RecoveryFailed is a summary; both source errors remain readable in the
    // public request diagnostics, associated with their actual frame origins.
    for (origin, source) in [
        (
            old_active.frames().last().unwrap().origin().clone(),
            "fixture apply failure after partial child write",
        ),
        (
            FrameOrigin::RouteError(fixture.boundary_route),
            "fixture recovery apply failure after partial child write",
        ),
    ] {
        assert!(
            failed.diagnostics().iter().any(|event| {
                event.request() == Some(failed_id)
                    && event.frame().is_some_and(|frame| frame.origin() == &origin)
                    && event.redacted_message().contains(source)
            }),
            "{failed:?}"
        );
    }
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
    assert!(
        recovery_effect
            .failure_message
            .as_deref()
            .unwrap()
            .contains("fixture apply failure after partial child write")
    );
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
    assert!(node_entities_with_width(&mut fixture.app, PARTIAL_CONTENT_WIDTH).is_empty());
    assert!(node_entities_with_width(&mut fixture.app, RECOVERY_PARTIAL_CONTENT_WIDTH).is_empty());

    let after_failure = router_snapshot(&fixture.app, fixture.router);
    assert_eq!(after_failure.active(), &old_active);
    assert_eq!(after_failure.history(), &old_history);
    assert_eq!(entity_set(&fixture.app), old_entities);
    assert!(!after_failure.cleanup().foreground_pending());
    assert_eq!(after_failure.cleanup().quarantine_count(), 0);

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

#[test]
fn queued_navigation_fails_when_host_is_lost_before_update() {
    // Exact apply/ready/sealed windows are covered inside ECS; this consumer
    // changes the app-owned host only between ordinary updates.
    for kind in [
        RecoveryR2LossKind::RegistrationRemoval,
        RecoveryR2LossKind::HostDespawn,
    ] {
        for with_active in [false, true] {
            let mut fixture = recovery_r1_fixture("public-prepublication-host-loss");
            if with_active {
                let first = push_and_settle(
                    &mut fixture.app,
                    fixture.router,
                    RouteLocation::new("/section/items/1"),
                    None,
                    None,
                );
                assert_succeeded(&request_snapshot(&fixture.app, fixture.router, first));
            }
            let history_before = router_snapshot(&fixture.app, fixture.router)
                .history()
                .clone();
            let target = RouteLocation::new("/section/items/2");
            let request = fixture
                .app
                .world_mut()
                .resource_mut::<RouterNavigation>()
                .push(fixture.router, target.clone())
                .unwrap();
            remove_recovery_host(&mut fixture, kind);
            let failed = drive_request_to_terminal(&mut fixture.app, fixture.router, request);
            assert_failed(&failed, NavigationFailureKind::HostLost);
            assert_eq!(failed.requested(), Some(&target));
            assert!(
                failed
                    .trace()
                    .iter()
                    .all(|event| event.kind() != NavigationTraceEventKind::CommitPublished)
            );
            let after = router_snapshot(&fixture.app, fixture.router);
            assert_eq!(after.history(), &history_before);
            assert_eq!(after.active().kind(), ActivePresentationKind::None);
            assert_eq!(
                after.availability(),
                &RouterAvailability::Unavailable(RouterUnavailableReason::HostLost)
            );
            assert_terminal_owner_closure(&fixture.app, fixture.router);
            if kind == RecoveryR2LossKind::RegistrationRemoval {
                assert_host_after_registration_loss(&fixture.app, &fixture.host_fingerprint);
            } else {
                assert!(
                    fixture
                        .app
                        .world()
                        .get_entity(fixture.host_fingerprint.host)
                        .is_err()
                );
            }
        }
    }
}

fn run_recovery_r2_published_history_and_queued_loss_row(label: &str, kind: RecoveryR2LossKind) {
    let mut fixture = recovery_r1_fixture(label);
    let first_location = RouteLocation::new("/section/items/1");
    let first = push_and_settle(
        &mut fixture.app,
        fixture.router,
        first_location.clone(),
        None,
        None,
    );
    assert_succeeded(&request_snapshot(&fixture.app, fixture.router, first));
    let committed_location = RouteLocation::new("/section/items/2");
    let committed = fixture
        .app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(fixture.router, committed_location.clone())
        .unwrap();
    let queued = fixture
        .app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(fixture.router, RouteLocation::new("/section/items/3"))
        .unwrap();

    // Observe the real publication at an ordinary update boundary. The exact
    // post-commit cleanup window has separate ECS-private regression coverage.
    let before_loss = drive_until_publication(&mut fixture.app, fixture.router, committed);
    assert_eq!(before_loss.history().current(), Some(&committed_location));
    remove_recovery_host(&mut fixture, kind);
    let committed_snapshot = drive_request_to_terminal(&mut fixture.app, fixture.router, committed);
    assert_succeeded(&committed_snapshot);
    assert_eq!(committed_snapshot.requested(), Some(&committed_location));
    let queued_snapshot = drive_request_to_terminal(&mut fixture.app, fixture.router, queued);
    assert_failed(&queued_snapshot, NavigationFailureKind::HostLost);
    assert!(
        queued_snapshot
            .trace()
            .iter()
            .all(|event| event.kind() != NavigationTraceEventKind::CommitPublished)
    );

    let after = router_snapshot(&fixture.app, fixture.router);
    assert_eq!(after.history(), before_loss.history());
    assert_eq!(
        after.history().entries(),
        [first_location, committed_location.clone()]
    );
    assert_eq!(after.history().current(), Some(&committed_location));
    assert_eq!(after.active().kind(), ActivePresentationKind::None);
    assert_eq!(
        after.availability(),
        &RouterAvailability::Unavailable(RouterUnavailableReason::HostLost)
    );
    assert_terminal_owner_closure(&fixture.app, fixture.router);
    if kind == RecoveryR2LossKind::RegistrationRemoval {
        assert_host_after_registration_loss(&fixture.app, &fixture.host_fingerprint);
    } else {
        assert!(
            fixture
                .app
                .world()
                .get_entity(fixture.host_fingerprint.host)
                .is_err()
        );
    }
}

#[test]
fn recovery_r2_registration_loss_preserves_published_history_and_fails_queued_navigation() {
    run_recovery_r2_published_history_and_queued_loss_row(
        "r2-postcommit-registration-removal",
        RecoveryR2LossKind::RegistrationRemoval,
    );
}

#[test]
fn recovery_r2_host_despawn_preserves_published_history_and_fails_queued_navigation() {
    run_recovery_r2_published_history_and_queued_loss_row(
        "r2-postcommit-host-despawn",
        RecoveryR2LossKind::HostDespawn,
    );
}

#[test]
fn recovery_r3_unavailable_reason_and_enqueue_mapping_are_source_bound() {
    #[derive(Clone, Copy, Debug)]
    enum Row {
        InitialNoCandidate,
        Conflict,
        MissingAncestorNode,
        CyclicAncestry,
    }

    for (label, row) in [
        ("initial-none", Row::InitialNoCandidate),
        ("conflict", Row::Conflict),
        ("missing-ancestor-node", Row::MissingAncestorNode),
        ("cyclic-ancestry", Row::CyclicAncestry),
    ] {
        let mut fixture = recovery_r3_admission_fixture(&format!("r3-source-{label}"));
        let mut repair_parent = None;
        match row {
            Row::InitialNoCandidate => {}
            Row::Conflict => {
                fixture
                    .app
                    .world_mut()
                    .spawn((Node::default(), RouterHostRegistration::new(fixture.router)));
                fixture
                    .app
                    .world_mut()
                    .spawn((Node::default(), RouterHostRegistration::new(fixture.router)));
            }
            Row::MissingAncestorNode => {
                let parent = fixture.app.world_mut().spawn_empty().id();
                fixture.app.world_mut().spawn((
                    Node::default(),
                    RouterHostRegistration::new(fixture.router),
                    ChildOf(parent),
                ));
                repair_parent = Some(parent);
            }
            Row::CyclicAncestry => {
                let grandparent = fixture.app.world_mut().spawn((Node::default(),)).id();
                let intermediate = fixture
                    .app
                    .world_mut()
                    .spawn((Node::default(), ChildOf(grandparent)))
                    .id();
                let host = fixture
                    .app
                    .world_mut()
                    .spawn((
                        Node::default(),
                        RouterHostRegistration::new(fixture.router),
                        ChildOf(intermediate),
                    ))
                    .id();
                fixture
                    .app
                    .world_mut()
                    .entity_mut(grandparent)
                    .insert(ChildOf(host));
            }
        }
        let invalid_entity_baseline = entity_set(&fixture.app);
        let expected = match row {
            Row::InitialNoCandidate => RouterAvailability::AwaitingHost,
            Row::Conflict => RouterAvailability::Unavailable(RouterUnavailableReason::HostConflict),
            Row::MissingAncestorNode | Row::CyclicAncestry => {
                RouterAvailability::Unavailable(RouterUnavailableReason::InvalidHostUiTopology)
            }
        };
        let unavailable =
            drive_recovery_r3_availability(&mut fixture.app, fixture.router, &expected);
        assert_eq!(unavailable.active().kind(), ActivePresentationKind::None);
        assert!(unavailable.host().version().is_none());
        assert!(unavailable.host().top_slot().is_none());
        assert_eq!(
            entity_set(&fixture.app),
            invalid_entity_baseline,
            "row {label} must classify before staging any router Entity"
        );
        assert_recovery_r3_enqueue_is_request_zero(
            &mut fixture.app,
            fixture.router,
            RouteLocation::new("/"),
        );

        if let Some(parent) = repair_parent {
            let ready_events_before = unavailable
                .host()
                .trace()
                .iter()
                .filter(|event| event.kind() == HostTraceEventKind::HostReady)
                .count();
            fixture
                .app
                .world_mut()
                .entity_mut(parent)
                .insert(Node::default());
            let ready = drive_recovery_r3_availability(
                &mut fixture.app,
                fixture.router,
                &RouterAvailability::Ready,
            );
            assert!(ready.host().version().is_some());
            assert!(ready.host().top_slot().is_some());
            assert!(
                ready
                    .host()
                    .trace()
                    .iter()
                    .map(HostTraceEvent::kind)
                    .collect::<Vec<_>>()
                    .ends_with(&[
                        HostTraceEventKind::AdmissionRequested,
                        HostTraceEventKind::HostGranted,
                        HostTraceEventKind::TopSlotRegistered,
                        HostTraceEventKind::HostReady,
                    ]),
                "the repaired revision must take the complete production admission path"
            );
            assert_eq!(ready.active().kind(), ActivePresentationKind::None);
            assert_eq!(ready.history().len(), 0);
            assert_eq!(
                ready
                    .host()
                    .trace()
                    .iter()
                    .filter(|event| event.kind() == HostTraceEventKind::HostReady)
                    .count(),
                ready_events_before + 1,
                "repairing one real topology revision permits exactly one Ready transition"
            );
            assert_eq!(
                ready
                    .host()
                    .trace()
                    .iter()
                    .filter(|event| event.kind() == HostTraceEventKind::AdmissionRequested)
                    .count(),
                1,
                "the rejected revision must not be retried before the topology changes"
            );
            assert_eq!(
                entity_set(&fixture.app).len(),
                invalid_entity_baseline.len() + 1
            );
            let stable_entities = entity_set(&fixture.app);
            let stable_trace = ready.host().trace().to_vec();
            fixture.app.update();
            fixture.app.update();
            let stable = router_snapshot(&fixture.app, fixture.router);
            assert_eq!(stable.availability(), &RouterAvailability::Ready);
            assert_eq!(stable.host().trace(), stable_trace);
            assert_eq!(entity_set(&fixture.app), stable_entities);
        }
    }
}

#[test]
fn idle_host_loss_registration_removal_clears_active_without_a_new_request() {
    assert_idle_host_loss(RecoveryR2LossKind::RegistrationRemoval, true);
}

#[test]
fn idle_host_loss_despawn_clears_active_without_a_new_request() {
    assert_idle_host_loss(RecoveryR2LossKind::HostDespawn, true);
}

#[test]
fn idle_host_loss_registration_removal_invalidates_ready_without_active() {
    assert_idle_host_loss(RecoveryR2LossKind::RegistrationRemoval, false);
}

#[test]
fn idle_host_loss_despawn_invalidates_ready_without_active() {
    assert_idle_host_loss(RecoveryR2LossKind::HostDespawn, false);
}

#[test]
fn idle_host_loss_same_entity_new_incarnation_re_admits_without_replay() {
    let mut fixture = recovery_r1_fixture("idle-same-incarnation");
    let request = push_and_settle(
        &mut fixture.app,
        fixture.router,
        RouteLocation::new("/section/items/1"),
        None,
        None,
    );
    let committed = request_snapshot(&fixture.app, fixture.router, request);
    assert_succeeded(&committed);
    for _ in 0..3 {
        fixture.app.update();
    }
    assert_terminal_owner_closure(&fixture.app, fixture.router);
    let before = router_snapshot(&fixture.app, fixture.router);
    let effects = fixture.control.effects();
    let old_top = fixture.host_fingerprint.top_slot.unwrap();
    remove_recovery_host(&mut fixture, RecoveryR2LossKind::RegistrationRemoval);
    fixture
        .app
        .world_mut()
        .entity_mut(fixture.host_fingerprint.host)
        .insert(RouterHostRegistration::new(fixture.router));
    fixture.app.update();
    let unavailable = router_snapshot(&fixture.app, fixture.router);
    assert_eq!(
        unavailable.availability(),
        &RouterAvailability::Unavailable(RouterUnavailableReason::HostLost)
    );
    assert_eq!(unavailable.active().kind(), ActivePresentationKind::None);
    assert_terminal_owner_closure(&fixture.app, fixture.router);
    let ready = drive_recovery_r3_availability(
        &mut fixture.app,
        fixture.router,
        &RouterAvailability::Ready,
    );
    assert_ne!(ready.host().version(), before.host().version());
    assert_ne!(ready.host().top_slot(), before.host().top_slot());
    assert!(fixture.app.world().get_entity(old_top).is_err());
    assert_eq!(ready.active().kind(), ActivePresentationKind::None);
    assert_eq!(ready.history(), before.history());
    assert_eq!(fixture.control.effects(), effects);
    assert_eq!(
        request_snapshot(&fixture.app, fixture.router, request),
        committed
    );
    let restored = replace_and_settle(
        &mut fixture.app,
        fixture.router,
        RouteLocation::new("/section/items/1"),
        None,
    );
    assert_succeeded(&request_snapshot(&fixture.app, fixture.router, restored));
    assert_eq!(
        router_snapshot(&fixture.app, fixture.router).history(),
        before.history()
    );
}

fn assert_idle_host_loss(loss: RecoveryR2LossKind, with_active: bool) {
    let mut fixture = recovery_r1_fixture("idle-host-loss");
    let committed = if with_active {
        let first = push_and_settle(
            &mut fixture.app,
            fixture.router,
            RouteLocation::new("/section/items/1"),
            None,
            None,
        );
        let second = push_and_settle(
            &mut fixture.app,
            fixture.router,
            RouteLocation::new("/section/items/2"),
            None,
            None,
        );
        [first, second]
            .into_iter()
            .map(|request| {
                let snapshot = request_snapshot(&fixture.app, fixture.router, request);
                assert_succeeded(&snapshot);
                (request, snapshot)
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    assert_terminal_owner_closure(&fixture.app, fixture.router);
    let before = router_snapshot(&fixture.app, fixture.router);
    assert_eq!(before.availability(), &RouterAvailability::Ready);
    assert!(before.host().version().is_some());
    assert!(before.host().top_slot().is_some());
    assert_eq!(
        before.active().kind(),
        if with_active {
            ActivePresentationKind::Route
        } else {
            ActivePresentationKind::None
        }
    );
    if with_active {
        assert_eq!(
            before.history().entries(),
            [
                RouteLocation::new("/section/items/1"),
                RouteLocation::new("/section/items/2"),
            ]
        );
        assert_eq!(before.history().cursor(), Some(1));
    } else {
        assert!(before.history().entries().is_empty());
    }

    // Let ordinary scheduling run after terminal settlement, with no commands.
    // Stable public state proves this is not the post-commit cleanup window.
    for _ in 0..3 {
        fixture.app.update();
        assert_terminal_owner_closure(&fixture.app, fixture.router);
        assert_eq!(router_snapshot(&fixture.app, fixture.router), before);
        for (request, snapshot) in &committed {
            assert_eq!(
                request_snapshot(&fixture.app, fixture.router, *request),
                *snapshot
            );
        }
    }
    let effects_before = fixture.control.effects();
    let old_targets = [51, 53, 55]
        .into_iter()
        .flat_map(|width| node_entities_with_width(&mut fixture.app, width))
        .collect::<Vec<_>>();
    assert_eq!(old_targets.len(), if with_active { 3 } else { 0 });
    for target in &old_targets {
        let gate = fixture.app.world().resource::<RouterInputGate>();
        assert!(gate.allows_pointer(*target));
        assert!(gate.allows_keyboard(*target));
        assert!(gate.allows_gamepad(*target));
    }
    let app_target = fixture
        .app
        .world_mut()
        .spawn((Node::default(), HostSentinel(991)))
        .id();
    let app_target_components = component_manifest(&fixture.app, app_target);
    let focus = old_targets.first().copied().unwrap_or(app_target);
    *fixture.app.world_mut().resource_mut::<InputFocus>() = InputFocus::from_entity(focus);

    remove_recovery_host(&mut fixture, loss);
    for _ in 0..24 {
        fixture.app.update();
        // The existing input refresh must revoke invalid targets on the first
        // ordinary update, independently of eventual lifecycle settlement.
        for target in &old_targets {
            let gate = fixture.app.world().resource::<RouterInputGate>();
            assert!(!gate.allows_pointer(*target));
            assert!(!gate.allows_keyboard(*target));
            assert!(!gate.allows_gamepad(*target));
        }
        assert_eq!(
            fixture.app.world().resource::<InputFocus>().get(),
            if with_active { None } else { Some(app_target) }
        );
        let snapshot = router_snapshot(&fixture.app, fixture.router);
        assert_eq!(
            snapshot.history(),
            before.history(),
            "host loss never rolls history back"
        );
        for (request, committed) in &committed {
            assert_eq!(
                request_snapshot(&fixture.app, fixture.router, *request),
                *committed
            );
        }
        assert_eq!(
            fixture.control.effects(),
            effects_before,
            "idle observation must not build a view"
        );
        if snapshot.availability()
            == &RouterAvailability::Unavailable(RouterUnavailableReason::HostLost)
            && snapshot.running_request().is_none()
            && snapshot.pending_phase().is_none()
            && !snapshot.cleanup().foreground_pending()
        {
            break;
        }
    }
    let unavailable = router_snapshot(&fixture.app, fixture.router);
    assert_eq!(
        component_manifest(&fixture.app, app_target),
        app_target_components
    );
    assert_eq!(
        fixture.app.world().get::<HostSentinel>(app_target),
        Some(&HostSentinel(991))
    );
    assert_eq!(
        unavailable.availability(),
        &RouterAvailability::Unavailable(RouterUnavailableReason::HostLost),
        "ordinary App::update must observe {loss:?} after genuine idle (with_active={with_active}); observed active={:?}",
        unavailable.active().kind()
    );
    assert_eq!(unavailable.active().kind(), ActivePresentationKind::None);
    assert_terminal_owner_closure(&fixture.app, fixture.router);
    for target in old_targets {
        assert!(fixture.app.world().get_entity(target).is_err());
    }
    match loss {
        RecoveryR2LossKind::RegistrationRemoval => {
            assert_host_after_registration_loss(&fixture.app, &fixture.host_fingerprint);
        }
        RecoveryR2LossKind::HostDespawn => {
            assert!(
                fixture
                    .app
                    .world()
                    .get_entity(fixture.host_fingerprint.host)
                    .is_err()
            );
            assert!(
                fixture
                    .app
                    .world()
                    .get_entity(fixture.host_fingerprint.top_slot.unwrap())
                    .is_err()
            );
        }
    }
    let new_host = match loss {
        RecoveryR2LossKind::RegistrationRemoval => {
            fixture
                .app
                .world_mut()
                .entity_mut(fixture.host_fingerprint.host)
                .insert(RouterHostRegistration::new(fixture.router));
            fixture.host_fingerprint.host
        }
        RecoveryR2LossKind::HostDespawn => {
            spawn_recovery_r3_app_host(&mut fixture.app, fixture.router).host
        }
    };
    let ready = drive_recovery_r3_availability(
        &mut fixture.app,
        fixture.router,
        &RouterAvailability::Ready,
    );
    assert_ne!(ready.host().version(), before.host().version());
    assert_ne!(ready.host().top_slot(), before.host().top_slot());
    assert_eq!(ready.active().kind(), ActivePresentationKind::None);
    assert_eq!(ready.history(), before.history());
    assert_eq!(fixture.control.effects(), effects_before);
    assert!(fixture.app.world().get_entity(new_host).is_ok());
    for (request, committed) in &committed {
        assert_eq!(
            request_snapshot(&fixture.app, fixture.router, *request),
            *committed
        );
    }
    let restored = replace_and_settle(
        &mut fixture.app,
        fixture.router,
        RouteLocation::new("/section/items/2"),
        None,
    );
    assert_succeeded(&request_snapshot(&fixture.app, fixture.router, restored));
    assert_terminal_owner_closure(&fixture.app, fixture.router);
}

#[test]
fn recovery_r3_journey_g_re_admission_uses_fresh_epoch_and_explicit_restore() {
    let mut fixture = recovery_r1_fixture("r3-fresh-host-readmission");
    let location = RouteLocation::new("/section/items/1");
    let first = push_and_settle(
        &mut fixture.app,
        fixture.router,
        location.clone(),
        None,
        None,
    );
    assert_succeeded(&request_snapshot(&fixture.app, fixture.router, first));
    let before_loss = router_snapshot(&fixture.app, fixture.router);
    let old_host_version = before_loss.host().version().unwrap().clone();
    let old_top_slot = before_loss.host().top_slot().unwrap();
    let restored_location = RouteLocation::new("/section/items/2");
    let second = push_and_settle(
        &mut fixture.app,
        fixture.router,
        restored_location.clone(),
        None,
        None,
    );
    let committed = request_snapshot(&fixture.app, fixture.router, second);
    assert_succeeded(&committed);
    let published = router_snapshot(&fixture.app, fixture.router);
    assert_eq!(published.history().current(), Some(&restored_location));
    let old_frames = published.active().frames().to_vec();
    let unavailable = settle_recovery_r3_queued_registration_loss(&mut fixture);
    assert_eq!(
        request_snapshot(&fixture.app, fixture.router, second),
        committed
    );
    let history_before = unavailable.history().clone();
    assert_eq!(
        history_before.entries(),
        [location.clone(), restored_location.clone()]
    );
    assert_eq!(history_before.current(), Some(&restored_location));
    let effects_before = fixture.control.effects();
    assert_eq!(
        unavailable.availability(),
        &RouterAvailability::Unavailable(RouterUnavailableReason::HostLost)
    );
    assert_eq!(unavailable.active().kind(), ActivePresentationKind::None);
    assert_eq!(unavailable.history(), &history_before);
    assert_terminal_owner_closure(&fixture.app, fixture.router);
    assert_host_after_registration_loss(&fixture.app, &fixture.host_fingerprint);
    assert_recovery_r3_enqueue_is_request_zero(
        &mut fixture.app,
        fixture.router,
        RouteLocation::new("/section/items/2"),
    );

    let mut new_host = spawn_recovery_r3_app_host(&mut fixture.app, fixture.router);
    let readmission_baseline = entity_set(&fixture.app);
    let ready = drive_recovery_r3_availability(
        &mut fixture.app,
        fixture.router,
        &RouterAvailability::Ready,
    );
    assert_eq!(ready.active().kind(), ActivePresentationKind::None);
    assert_eq!(ready.history(), &history_before);
    assert_ne!(ready.host().version(), Some(&old_host_version));
    assert_ne!(ready.host().top_slot(), Some(old_top_slot));
    assert_eq!(fixture.control.effects(), effects_before);
    assert_eq!(
        entity_set(&fixture.app).len(),
        readmission_baseline.len() + 1
    );
    new_host.top_slot = fixture
        .app
        .world()
        .get::<Children>(new_host.host)
        .unwrap()
        .iter()
        .copied()
        .find(|child| !new_host.existing_children.contains(child));
    assert_host_intact(&fixture.app, &new_host);

    fixture.app.update();
    fixture.app.update();
    let before_restore = router_snapshot(&fixture.app, fixture.router);
    assert_eq!(before_restore.active().kind(), ActivePresentationKind::None);
    assert_eq!(before_restore.history(), &history_before);
    assert_eq!(fixture.control.effects(), effects_before);

    let restored = replace_and_settle(
        &mut fixture.app,
        fixture.router,
        history_before.current().unwrap().clone(),
        None,
    );
    assert_succeeded(&request_snapshot(&fixture.app, fixture.router, restored));
    let after_restore = router_snapshot(&fixture.app, fixture.router);
    assert_eq!(after_restore.active().kind(), ActivePresentationKind::Route);
    assert_eq!(after_restore.active().location(), Some(&restored_location));
    assert_eq!(after_restore.history(), &history_before);
    assert!(
        after_restore
            .active()
            .frames()
            .iter()
            .all(|frame| !old_frames.contains(frame)),
        "explicit restore must build only fresh frame identities"
    );
    assert_eq!(after_restore.host().version(), ready.host().version());
    assert_eq!(after_restore.host().top_slot(), ready.host().top_slot());
    assert_terminal_owner_closure(&fixture.app, fixture.router);
    assert_host_intact(&fixture.app, &new_host);
}

#[test]
fn recovery_r3_journey_g_same_host_readmission_preserves_detached_old_content() {
    let mut fixture = recovery_r1_fixture("r3-same-host-readmission");
    let location = RouteLocation::new("/section/items/1");
    let first = push_and_settle(
        &mut fixture.app,
        fixture.router,
        location.clone(),
        None,
        None,
    );
    assert_succeeded(&request_snapshot(&fixture.app, fixture.router, first));
    let before_loss = router_snapshot(&fixture.app, fixture.router);
    let old_host_version = before_loss.host().version().unwrap().clone();
    let old_top_slot_id = before_loss.host().top_slot().unwrap();
    let old_top_slot = fixture.host_fingerprint.top_slot.unwrap();
    let foreign = fixture
        .app
        .world_mut()
        .spawn(RecoveryR3ForeignChildSentinel(0x731))
        .id();

    let restored_location = RouteLocation::new("/section/items/2");
    let unavailable = settle_recovery_r3_same_host_queued_registration_loss(
        &mut fixture,
        old_top_slot,
        foreign,
        restored_location.clone(),
    );
    let history_after_loss = unavailable.history().clone();
    assert_eq!(
        history_after_loss.entries(),
        [location.clone(), restored_location.clone()]
    );
    assert_eq!(history_after_loss.current(), Some(&restored_location));
    assert_eq!(
        unavailable.availability(),
        &RouterAvailability::Unavailable(RouterUnavailableReason::HostLost)
    );
    assert_eq!(unavailable.active().kind(), ActivePresentationKind::None);
    assert!(fixture.app.world().get_entity(old_top_slot).is_ok());
    assert_eq!(
        fixture
            .app
            .world()
            .get::<ChildOf>(foreign)
            .map(ChildOf::parent),
        Some(old_top_slot)
    );
    assert_eq!(
        fixture
            .app
            .world()
            .get::<RecoveryR3ForeignChildSentinel>(foreign),
        Some(&RecoveryR3ForeignChildSentinel(0x731))
    );
    assert_eq!(
        fixture
            .app
            .world()
            .get::<Children>(fixture.host_fingerprint.host)
            .unwrap()
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        fixture.host_fingerprint.existing_children
    );

    fixture
        .app
        .world_mut()
        .entity_mut(fixture.host_fingerprint.host)
        .insert(RouterHostRegistration::new(fixture.router));
    let ready = drive_recovery_r3_availability(
        &mut fixture.app,
        fixture.router,
        &RouterAvailability::Ready,
    );
    let new_top_slots = fixture
        .app
        .world()
        .get::<Children>(fixture.host_fingerprint.host)
        .expect("readmission attaches a new top slot")
        .iter()
        .copied()
        .filter(|child| !fixture.host_fingerprint.existing_children.contains(child))
        .collect::<Vec<_>>();
    assert_eq!(new_top_slots.len(), 1);
    let new_top_slot = new_top_slots[0];
    assert_ne!(new_top_slot, old_top_slot);
    assert_ne!(ready.host().version(), Some(&old_host_version));
    assert_ne!(ready.host().top_slot(), Some(old_top_slot_id));
    assert_eq!(ready.active().kind(), ActivePresentationKind::None);
    assert_eq!(ready.history(), &history_after_loss);
    assert_eq!(
        fixture
            .app
            .world()
            .get::<ChildOf>(old_top_slot)
            .map(ChildOf::parent),
        None,
        "old HostLease must detach the retired top-slot edge before new admission"
    );
    assert_eq!(
        fixture
            .app
            .world()
            .get::<ChildOf>(foreign)
            .map(ChildOf::parent),
        Some(old_top_slot)
    );
    fixture.host_fingerprint.top_slot = Some(new_top_slot);
    assert_host_intact(&fixture.app, &fixture.host_fingerprint);

    let restored = replace_and_settle(
        &mut fixture.app,
        fixture.router,
        history_after_loss.current().unwrap().clone(),
        None,
    );
    assert_succeeded(&request_snapshot(&fixture.app, fixture.router, restored));
    let after = router_snapshot(&fixture.app, fixture.router);
    assert_eq!(after.active().kind(), ActivePresentationKind::Route);
    assert_eq!(after.active().location(), Some(&restored_location));
    assert_eq!(after.host().version(), ready.host().version());
    assert_eq!(after.host().top_slot(), ready.host().top_slot());
    assert_eq!(after.history(), &history_after_loss);
    assert!(after.running_request().is_none());
    assert!(after.pending_phase().is_none());
    assert!(!after.cleanup().foreground_pending());
    assert!(fixture.app.world().get_entity(old_top_slot).is_ok());
    assert!(fixture.app.world().get::<ChildOf>(old_top_slot).is_none());
    assert_eq!(
        fixture
            .app
            .world()
            .get::<ChildOf>(foreign)
            .map(ChildOf::parent),
        Some(old_top_slot)
    );
    assert_eq!(
        fixture
            .app
            .world()
            .get::<RecoveryR3ForeignChildSentinel>(foreign),
        Some(&RecoveryR3ForeignChildSentinel(0x731))
    );
    assert_host_intact(&fixture.app, &fixture.host_fingerprint);
}
