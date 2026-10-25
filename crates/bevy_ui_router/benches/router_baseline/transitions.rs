//! Public transition lifecycle, with equal frame-work budgets per comparison.
//! Optional UI=1 adds normal layout, UiStack construction and picking systems;
//! neither variant creates a renderer or measures presentation/GPU latency.
use super::*;
use bevy_app::Update;
use bevy_ui::{
    ComputedNode, ComputedUiRenderTargetInfo, ComputedUiTargetCamera, PositionType, UiStack,
    UiTransform, Val, Val2,
};
use bevy_ui_router::ecs::RouterScheduleSet;

const FRAME_BATCH: usize = 32;
const WARMUP: usize = 64;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Disabled,
    EnterOnly,
    HeldExit,
}

impl Mode {
    fn name(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::EnterOnly => "enter_only",
            Self::HeldExit => "held_exit",
        }
    }
}

#[derive(Resource, Default)]
struct Animation {
    id: Option<RouteTransitionId>,
    entering: Vec<Entity>,
    leaving: Vec<Entity>,
    presented: bool,
    frame: u32,
}

struct Driver(Mode);

impl RouteTransitionDriver for Driver {
    fn prepare(
        &mut self,
        world: &mut World,
        context: &RouteTransitionContext,
    ) -> TransitionPreparation {
        let mut entering = Vec::new();
        let mut leaving = Vec::new();
        for (targets, destination) in [
            (context.entering(), &mut entering),
            (context.leaving(), &mut leaving),
        ] {
            for target in targets.iter().filter(|target| target.is_outermost()) {
                if self.0 == Mode::EnterOnly && context.leaving().contains(target) {
                    continue;
                }
                let entity = target.surface();
                let mut node = world.get_mut::<Node>(entity).unwrap();
                node.position_type = PositionType::Absolute;
                node.left = Val::Px(0.0);
                node.top = Val::Px(0.0);
                world.get_mut::<UiTransform>(entity).unwrap().translation = Val2::px(20.0, 0.0);
                destination.push(entity);
            }
        }
        *world.resource_mut::<Animation>() = Animation {
            id: Some(context.id()),
            entering,
            leaving,
            presented: false,
            frame: 0,
        };
        TransitionPreparation::Run {
            hold_exit: self.0 == Mode::HeldExit,
        }
    }

    fn presented(&mut self, world: &mut World, id: &RouteTransitionId) {
        let mut state = world.resource_mut::<Animation>();
        assert_eq!(state.id.as_ref(), Some(id));
        state.presented = true;
    }

    fn cancelled(&mut self, world: &mut World, _: &RouteTransitionId, _: TransitionCancelReason) {
        *world.resource_mut::<Animation>() = Animation::default();
    }
}

/// A tiny application writer. The fixed update count normalizes work, and is
/// deliberately not interpreted as an animation duration or display frame rate.
fn animate(mut state: ResMut<Animation>, mut transforms: Query<&mut UiTransform>) {
    if !state.presented {
        return;
    }
    state.frame += 1;
    let position = (state.frame % FRAME_BATCH as u32) as f32;
    for entity in state.entering.iter().chain(&state.leaving) {
        transforms.get_mut(*entity).unwrap().translation = Val2::px(position, 0.0);
    }
}

struct Transitions {
    app: App,
    router: RouterHandle,
    host: Entity,
    content: usize,
    mode: Mode,
    keep_alive: bool,
    ui: bool,
    factory_calls: Arc<AtomicUsize>,
    current: Entity,
    pages: [Option<Entity>; 2],
    issued: Vec<NavigationRequestId>,
}

struct Phases {
    publication: Sample,
    holding: Sample,
    release: Sample,
    finish: Sample,
    idle: Sample,
}

impl Transitions {
    fn new(content: usize, shape: ContentShape, mode: Mode, keep_alive: bool, ui: bool) -> Self {
        let mut app = if ui {
            use bevy_internal::{
                DefaultPlugins,
                camera::{Camera, Camera2d, ComputedCameraValues, RenderTargetInfo},
                math::UVec2,
            };
            let mut app = App::new();
            app.add_plugins(DefaultPlugins);
            if !app.is_plugin_added::<ScenePlugin>() {
                app.add_plugins(ScenePlugin);
            }
            app.add_plugins(bevy_input_focus::InputFocusPlugin);
            app.world_mut().spawn((
                Camera2d,
                Camera {
                    computed: ComputedCameraValues {
                        target_info: Some(RenderTargetInfo {
                            physical_size: UVec2::new(640, 480),
                            scale_factor: 1.0,
                        }),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            ));
            app
        } else {
            base_app()
        };
        app.init_resource::<Animation>()
            .add_systems(Update, animate.after(RouterScheduleSet::PublishSnapshots));
        let layout = app
            .register_bsn_view(ViewDriverKey::new("transition-layout").unwrap(), |_| {
                Ok(bsn! {
                    Node { width: Val::Percent(100.0), height: Val::Percent(100.0) }
                    Layout DefaultOutlet
                })
            })
            .unwrap();
        let factory_calls = Arc::new(AtomicUsize::new(0));
        let calls = factory_calls.clone();
        let page = app
            .register_bsn_view(
                ViewDriverKey::new("transition-page").unwrap(),
                move |input| {
                    calls.fetch_add(1, Ordering::Relaxed);
                    let id = input.params().get("id").unwrap().to_owned();
                    Ok(SceneFunction(move |_, root: &mut ResolvedScene| {
                        root.push_template(Node {
                            width: Val::Percent(100.0),
                            height: Val::Percent(100.0),
                            ..Default::default()
                        });
                        root.push_template(Page(id));
                        append_content(root, content, shape.levels());
                    }))
                },
            )
            .unwrap();
        let mut views = RouteViewRegistry::new();
        for (key, driver) in [("layout", layout), ("page", page)] {
            views
                .insert(
                    RouteViewKey::new(key).unwrap(),
                    RouteViewDescriptor::new(driver),
                )
                .unwrap();
        }
        let mut tree = RouteTree::builder()
            .root_with_options(
                RoutePattern::pathless(),
                RouteNodeOptions::new().with_view(RouteViewKey::new("layout").unwrap()),
            )
            .unwrap();
        tree.route_with_options(
            tree.root_id().unwrap(),
            RoutePattern::path(":id").unwrap(),
            RouteNodeOptions::new()
                .with_view(RouteViewKey::new("page").unwrap())
                .with_keep_alive(keep_alive),
        )
        .unwrap();
        let mut config = RouterConfig::new(
            tree.build().unwrap(),
            views,
            RouteViewKey::new("page").unwrap(),
        )
        .with_keep_alive_capacity(if keep_alive { 2 } else { 0 });
        if mode != Mode::Disabled {
            config = config.with_transition_driver(Driver(mode));
        }
        let router = app.add_ui_router(config).unwrap();
        let host = app
            .world_mut()
            .spawn((
                Node {
                    width: Val::Px(640.0),
                    height: Val::Px(480.0),
                    ..Default::default()
                },
                RouterHostRegistration::new(router),
            ))
            .id();
        app.update();
        let mut fixture = Self {
            app,
            router,
            host,
            content,
            mode,
            keep_alive,
            ui,
            factory_calls,
            current: host,
            pages: [None, None],
            issued: Vec::new(),
        };
        fixture.operation(0, 1);
        fixture
    }

    fn snapshot(&self) -> &RouterSnapshot {
        self.app
            .world()
            .resource::<RouterSnapshots>()
            .get(self.router)
            .unwrap()
    }

    fn assert_ui_page(&self, page: Entity) {
        let world = self.app.world();
        assert!(world.resource::<UiStack>().uinodes.contains(&page));
        assert!(
            world
                .get::<ComputedUiTargetCamera>(page)
                .unwrap()
                .get()
                .is_some()
        );
        let target = world.get::<ComputedUiRenderTargetInfo>(page).unwrap();
        assert!(target.physical_size().min_element() > 0);
        assert!(target.scale_factor() > 0.0);
        assert!(
            world
                .get::<ComputedNode>(page)
                .unwrap()
                .size()
                .min_element()
                > 0.0
        );
    }

    fn settled(&self, request: NavigationRequestId) -> bool {
        let snapshots = self.app.world().resource::<RouterSnapshots>();
        let terminal = matches!(
            snapshots.request(self.router, request),
            RequestLookup::Found(result) if matches!(result.status(), RequestStatus::Succeeded)
        );
        terminal
            && self.snapshot().running_request().is_none()
            && !self.snapshot().cleanup().foreground_pending()
    }

    fn settle(&mut self, request: NavigationRequestId) -> Sample {
        let start = Instant::now();
        for updates in 1..=32 {
            self.app.update();
            if self.settled(request) {
                return Sample {
                    elapsed: start.elapsed(),
                    updates,
                };
            }
        }
        panic!("transition request did not settle: {:?}", self.snapshot());
    }

    fn frame_batch(&mut self, frames: usize) -> Sample {
        let start = Instant::now();
        for _ in 0..frames {
            self.app.update();
        }
        Sample {
            elapsed: start.elapsed() / frames as u32,
            updates: 1,
        }
    }

    fn stop(&mut self, finish: bool) {
        let mut state = self.app.world_mut().resource_mut::<Animation>();
        let id = state.id.clone();
        state.leaving.clear();
        if finish {
            state.entering.clear();
            state.presented = false;
            state.id = None;
        }
        if let Some(id) = id {
            let mut commands = self.app.world_mut().commands();
            if finish {
                commands.finish_route_transition(id);
            } else {
                commands.release_route_transition_exit(id);
            }
        }
    }

    fn operation(&mut self, target: usize, frames: usize) -> Phases {
        let id = ["a", "b"][target];
        let location = RouteLocation::new(format!("/{id}"));
        let previous = self.current;
        let start = Instant::now();
        let request = self
            .app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .replace(self.router, location.clone())
            .unwrap();
        let mut publication = None;
        for updates in 1..=32 {
            self.app.update();
            if self.snapshot().active().location() == Some(&location) {
                if self.mode != Mode::Disabled {
                    assert!(self.app.world().resource::<Animation>().presented);
                }
                publication = Some(Sample {
                    elapsed: start.elapsed(),
                    updates,
                });
                break;
            }
        }
        let publication = publication.expect("publication within 32 updates");
        let active = self
            .app
            .world_mut()
            .run_system_once(|activity: RouteActivity, pages: Query<(Entity, &Page)>| {
                pages
                    .iter()
                    .filter(|(entity, _)| activity.is_active(*entity))
                    .map(|(entity, page)| (entity, page.0.clone()))
                    .collect::<Vec<_>>()
            })
            .unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].1, id);
        self.current = active[0].0;
        if self.ui {
            // UI measurements must observe real production layout and stack
            // output, not just an installed plugin with an inactive camera.
            self.assert_ui_page(self.current);
            if self.mode == Mode::HeldExit && previous != self.host {
                self.assert_ui_page(previous);
            }
        }
        assert!(
            self.app
                .world()
                .resource::<RouterInputGate>()
                .allows_keyboard(self.current)
        );
        if previous != self.host {
            assert!(
                !self
                    .app
                    .world()
                    .resource::<RouterInputGate>()
                    .allows_keyboard(previous)
            );
        }
        if self.keep_alive {
            if let Some(original) = self.pages[target] {
                assert_eq!(
                    self.current, original,
                    "warm cache reuses the original Entity"
                );
            }
            self.pages[target] = Some(self.current);
        } else {
            assert_ne!(self.current, previous);
        }

        let holding = self.frame_batch(frames);
        let release_start = Instant::now();
        self.stop(false);
        let mut release = self.settle(request);
        release.elapsed = release_start.elapsed();
        let finish_start = Instant::now();
        self.stop(true);
        self.app.update();
        let finish = Sample {
            elapsed: finish_start.elapsed(),
            updates: 1,
        };
        let idle = self.frame_batch(frames);
        assert!(self.settled(request));
        assert_eq!(self.snapshot().cleanup().quarantine_count(), 0);
        assert!(self.app.world().get_entity(self.host).is_ok());
        if !self.keep_alive && previous != self.host {
            assert!(self.app.world().get_entity(previous).is_err());
        }
        let pages = self
            .app
            .world_mut()
            .query_filtered::<Entity, With<Page>>()
            .iter(self.app.world())
            .count();
        assert_eq!(
            pages,
            if self.keep_alive && self.pages.iter().all(Option::is_some) {
                2
            } else {
                1
            }
        );
        let contents = self
            .app
            .world_mut()
            .query_filtered::<Entity, With<Content>>()
            .iter(self.app.world())
            .count();
        assert_eq!(contents, pages * self.content);
        self.issued.push(request);
        Phases {
            publication,
            holding,
            release,
            finish,
            idle,
        }
    }
}

pub(super) fn run(shape: ContentShape, samples: usize) {
    let ui = match std::env::var("ROUTER_BENCH_UI").as_deref() {
        Err(std::env::VarError::NotPresent) | Ok("0") => false,
        Ok("1") => true,
        value => panic!("ROUTER_BENCH_UI must be unset, 0 or 1, got {value:?}"),
    };
    let contents = match std::env::var("ROUTER_BENCH_CONTENT") {
        Err(std::env::VarError::NotPresent) => vec![32, 128, 1024],
        Ok(value) => {
            let content = value.parse().expect("content size must be 32, 128 or 1024");
            assert!(matches!(content, 32 | 128 | 1024));
            vec![content]
        }
        Err(error) => panic!("ROUTER_BENCH_CONTENT: {error}"),
    };
    println!(
        "ui,transition,cache,phase,content_children,frames_per_sample,samples,mean_us,p50_us,p95_us,max_us,min_updates,max_updates,live_entities,factory_builds"
    );
    for content in contents {
        for keep_alive in [false, true] {
            for mode in [Mode::Disabled, Mode::EnterOnly, Mode::HeldExit] {
                let mut fixture = Transitions::new(content, shape, mode, keep_alive, ui);
                for index in 0..WARMUP {
                    // Warm each lifecycle stage and bounded request eviction
                    // without repeating the measured 32-update frame batches.
                    fixture.operation((index + 1) % 2, 1);
                }
                for _ in 0..64 {
                    fixture.app.update();
                }
                let builds_before = fixture.factory_calls.load(Ordering::Relaxed);
                let live_before = fixture.app.world().entities().count_spawned();
                let mut phases: [Vec<Sample>; 5] =
                    std::array::from_fn(|_| Vec::with_capacity(samples));
                for index in 0..samples {
                    let result = fixture.operation((index + 1) % 2, FRAME_BATCH);
                    for (values, value) in phases.iter_mut().zip([
                        result.publication,
                        result.holding,
                        result.release,
                        result.finish,
                        result.idle,
                    ]) {
                        values.push(value);
                    }
                    assert_eq!(fixture.app.world().entities().count_spawned(), live_before);
                }
                let builds = fixture.factory_calls.load(Ordering::Relaxed) - builds_before;
                assert_eq!(builds, if keep_alive { 0 } else { samples });
                assert!(
                    matches!(
                        fixture
                            .app
                            .world()
                            .resource::<RouterSnapshots>()
                            .request(fixture.router, fixture.issued[0]),
                        RequestLookup::NotRetained
                    ),
                    "warmup must fill and evict the bounded request records"
                );
                for (phase, values) in [
                    "publication",
                    "holding_update",
                    "release",
                    "finish",
                    "idle_update",
                ]
                .into_iter()
                .zip(&phases)
                {
                    let frames = if phase.ends_with("_update") {
                        FRAME_BATCH
                    } else {
                        1
                    };
                    println!(
                        "{},{},{},{phase},{content},{frames},{},{live_before},{builds}",
                        usize::from(ui),
                        mode.name(),
                        if keep_alive { "hit" } else { "fresh" },
                        timing_columns(values)
                    );
                }
            }
        }
    }
}
