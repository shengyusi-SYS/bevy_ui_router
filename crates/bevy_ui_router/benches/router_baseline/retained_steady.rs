//! Keep one active leaf and parent unchanged while accumulating retained siblings.
use super::*;
use bevy_internal::math::Vec2;
use bevy_ui::{
    ComputedNode, ComputedUiRenderTargetInfo, ComputedUiTargetCamera, FlexDirection, Overflow,
    ScrollPosition, UiStack, Val,
};

const ACTIVE_CONTENT: usize = 32;
const PARENT_CONTENT: usize = 128;
const MAX_RETAINED: usize = 4;

#[derive(Component)]
struct ParentContent;

struct RetainedSteady {
    fixture: Fixture,
    active: Entity,
    parent: Entity,
    retained: Vec<Entity>,
    ui: bool,
}

fn app(ui: bool) -> App {
    if !ui {
        return base_app();
    }
    // Match the transitions UI fixture: normal UI/picking plugins and genuine
    // camera input, without a renderer or a fabricated layout witness.
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
}

impl RetainedSteady {
    fn new(shape: ContentShape, ui: bool) -> Self {
        let mut app = app(ui);
        let layout = app
            .register_bsn_view(
                ViewDriverKey::new("retained-steady-layout").unwrap(),
                |_| {
                    Ok(bsn! {
                        Node {
                            width: Val::Px(640.0), height: Val::Px(480.0),
                            flex_direction: FlexDirection::Column,
                        }
                        Layout
                        Children [Node {
                            width: Val::Px(160.0), height: Val::Px(120.0), flex_shrink: 0.0,
                        } DefaultOutlet]
                    })
                },
            )
            .unwrap();
        let factory_calls = Arc::new(AtomicUsize::new(0));
        let calls = factory_calls.clone();
        let page = app
            .register_bsn_view(
                ViewDriverKey::new("retained-steady-page").unwrap(),
                move |input| {
                    calls.fetch_add(1, Ordering::Relaxed);
                    let id = input.params().get("id").unwrap().to_owned();
                    Ok(SceneFunction(move |_, root: &mut ResolvedScene| {
                        root.push_template(Node {
                            width: Val::Px(160.0),
                            height: Val::Px(120.0),
                            flex_shrink: 0.0,
                            overflow: Overflow::scroll(),
                            ..Default::default()
                        });
                        root.push_template(Page(id));
                        root.push_template(RouteScroll::new("body"));
                        root.push_template(ScrollPosition::default());
                    }))
                },
            )
            .unwrap();
        let layout_key = RouteViewKey::new("layout").unwrap();
        let page_key = RouteViewKey::new("page").unwrap();
        let mut views = RouteViewRegistry::new();
        views
            .insert(layout_key.clone(), RouteViewDescriptor::new(layout))
            .unwrap();
        views
            .insert(page_key.clone(), RouteViewDescriptor::new(page))
            .unwrap();
        let mut routes = RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/:section").unwrap(),
                RouteNodeOptions::new().with_view(layout_key),
            )
            .unwrap();
        routes
            .route_with_options(
                routes.root_id().unwrap(),
                RoutePattern::path(":id").unwrap(),
                RouteNodeOptions::new()
                    .with_view(page_key.clone())
                    .with_keep_alive(true),
            )
            .unwrap();
        let router = app
            .add_ui_router(
                RouterConfig::new(routes.build().unwrap(), views, page_key)
                    .with_keep_alive_capacity(MAX_RETAINED),
            )
            .unwrap();
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
        let mut fixture = Fixture {
            app,
            router,
            host,
            content: ACTIVE_CONTENT,
            shape,
            issued: Vec::new(),
            factory_calls: Some(factory_calls),
        };
        fixture.navigate(Action::Push, "/items/a", false);
        let active = only::<Page>(&mut fixture.app);
        let parent = only::<Layout>(&mut fixture.app);

        // Ordinary World operations after publication exercise dynamic native
        // containment. The parent budget is independent of every leaf budget.
        spawn_control_content(
            fixture.app.world_mut(),
            parent,
            PARENT_CONTENT,
            shape.levels(),
        );
        let parent_content = fixture
            .app
            .world_mut()
            .query_filtered::<Entity, With<Content>>()
            .iter(fixture.app.world())
            .collect::<Vec<_>>();
        assert_eq!(parent_content.len(), PARENT_CONTENT);
        for entity in parent_content {
            fixture
                .app
                .world_mut()
                .entity_mut(entity)
                .remove::<Content>()
                .insert(ParentContent);
        }
        let mut result = Self {
            fixture,
            active,
            parent,
            retained: Vec::new(),
            ui,
        };
        result.populate(active, ACTIVE_CONTENT);
        result
    }

    fn populate(&mut self, page: Entity, count: usize) {
        spawn_control_content(
            self.fixture.app.world_mut(),
            page,
            count,
            self.fixture.shape.levels(),
        );
        // One existing content node supplies real overflow without adding an
        // extra node to the equal-budget flat/nested trees.
        let first = self.fixture.app.world().get::<Children>(page).unwrap()[0];
        self.fixture.app.world_mut().entity_mut(first).insert(Node {
            width: Val::Px(600.0),
            height: Val::Px(800.0),
            flex_shrink: 0.0,
            ..Default::default()
        });
        self.fixture.app.update();
    }

    fn accumulate(&mut self, count: usize, content: usize) {
        for index in self.retained.len()..count {
            let id = format!("retained-{index}");
            self.fixture
                .navigate(Action::Replace, &format!("/items/{id}"), false);
            let page = self
                .fixture
                .app
                .world_mut()
                .query::<(Entity, &Page)>()
                .iter(self.fixture.app.world())
                .find(|(_, page)| page.0 == id)
                .unwrap()
                .0;
            self.populate(page, content);
            self.retained.push(page);
            self.fixture.navigate(Action::Replace, "/items/a", false);
        }
    }

    fn assert_stable(&mut self, retained_content: usize, builds: usize, live: u32) {
        assert_eq!(self.fixture.factory_count(), builds);
        assert_eq!(only::<Layout>(&mut self.fixture.app), self.parent);
        assert_eq!(self.fixture.app.world().entities().count_spawned(), live);
        let (pages, active_content, inactive_content, parent_content) = self
            .fixture
            .app
            .world_mut()
            .run_system_once(
                |activity: RouteActivity,
                 pages: Query<Entity, With<Page>>,
                 content: Query<Entity, With<Content>>,
                 parent: Query<Entity, With<ParentContent>>| {
                    let pages = pages
                        .iter()
                        .map(|entity| (entity, activity.is_active(entity)))
                        .collect::<Vec<_>>();
                    let active_content = content
                        .iter()
                        .filter(|entity| activity.is_active(*entity))
                        .count();
                    (
                        pages,
                        active_content,
                        content.iter().count() - active_content,
                        parent
                            .iter()
                            .filter(|entity| activity.is_active(*entity))
                            .count(),
                    )
                },
            )
            .unwrap();
        assert_eq!(pages.len(), self.retained.len() + 1);
        assert_eq!(pages.iter().filter(|(_, active)| *active).count(), 1);
        assert!(pages.contains(&(self.active, true)));
        for entity in &self.retained {
            assert!(pages.contains(&(*entity, false)));
        }
        assert_eq!(active_content, ACTIVE_CONTENT);
        assert_eq!(inactive_content, self.retained.len() * retained_content);
        assert_eq!(parent_content, PARENT_CONTENT);
        let world = self.fixture.app.world();
        let gate = world.resource::<RouterInputGate>();
        for (entity, active) in pages {
            assert_eq!(gate.allows_keyboard(entity), active);
        }
        let snapshot = self.fixture.snapshot();
        let location = RouteLocation::new("/items/a");
        assert_eq!(snapshot.active().location(), Some(&location));
        assert_eq!(snapshot.active().frames().len(), 2);
        assert_eq!(snapshot.history().entries().len(), 1);
        assert_eq!(snapshot.history().current(), Some(&location));
        assert!(snapshot.running_request().is_none());
        assert!(!snapshot.cleanup().foreground_pending());
        assert_eq!(snapshot.cleanup().quarantine_count(), 0);
        let requested = world.get::<ScrollPosition>(self.active).unwrap().0;
        assert!((requested - position(false)).abs().max_element() < 0.01);
        if self.ui {
            assert!(world.resource::<UiStack>().uinodes.contains(&self.active));
            assert!(
                world
                    .get::<ComputedUiTargetCamera>(self.active)
                    .unwrap()
                    .get()
                    .is_some()
            );
            let target = world
                .get::<ComputedUiRenderTargetInfo>(self.active)
                .unwrap();
            assert!(target.physical_size().min_element() > 0);
            assert!(target.scale_factor() > 0.0);
            let node = world.get::<ComputedNode>(self.active).unwrap();
            assert!(node.size().min_element() > 0.0);
            assert!(node.content_size.y > node.size().y);
            assert!(
                (node.scroll_position * node.inverse_scale_factor - requested)
                    .abs()
                    .max_element()
                    < 0.01
            );
        }
    }

    fn measure(&mut self, samples: usize, batch: usize, changing: bool) -> Vec<Sample> {
        (0..samples)
            .map(|_| {
                let start = Instant::now();
                for index in 0..batch {
                    if changing {
                        self.fixture
                            .app
                            .world_mut()
                            .get_mut::<ScrollPosition>(self.active)
                            .unwrap()
                            .0 = position(index % 2 == 0);
                    }
                    self.fixture.app.update();
                }
                Sample {
                    elapsed: start.elapsed() / batch as u32,
                    updates: batch,
                }
            })
            .collect()
    }
}

fn position(alternate: bool) -> Vec2 {
    if alternate {
        Vec2::new(24.0, 48.0)
    } else {
        Vec2::new(8.0, 16.0)
    }
}

pub(super) fn run(shape: ContentShape, samples: usize, smoke: bool) {
    let ui = match std::env::var("ROUTER_BENCH_UI").as_deref() {
        Err(std::env::VarError::NotPresent) | Ok("0") => false,
        Ok("1") => true,
        value => panic!("ROUTER_BENCH_UI must be unset, 0 or 1, got {value:?}"),
    };
    let sizes: &[usize] = if smoke { &[32] } else { &[32, 1024] };
    let counts: &[usize] = if smoke { &[0, 4] } else { &[0, 1, 4] };
    let batch = if smoke { 4 } else { IDLE_BATCH };
    let warmup = if smoke { 4 } else { 64 };
    println!(
        "ui,scenario,parent_children,active_children,retained_children_per_page,retained_pages,frames_per_sample,samples,mean_us,p50_us,p95_us,max_us,min_updates,max_updates,live_entities,active_pages,retained_content,retained_requests,factory_builds,factory_builds_during_samples,scroll_writes"
    );
    for &content in sizes {
        let mut state = RetainedSteady::new(shape, ui);
        for &count in counts {
            state.accumulate(count, content);
            state
                .fixture
                .app
                .world_mut()
                .get_mut::<ScrollPosition>(state.active)
                .unwrap()
                .0 = position(false);
            for _ in 0..warmup {
                state.fixture.app.update();
            }
            let live = state.fixture.app.world().entities().count_spawned();
            let builds = state.fixture.factory_count();
            assert_eq!(builds, count + 1);
            state.assert_stable(content, builds, live);
            for changing in [false, true] {
                // Warm the exact update workload before measuring it.
                state.measure(1, warmup, changing);
                let values = state.measure(samples, batch, changing);
                state.assert_stable(content, builds, live);
                println!(
                    "{},{},{PARENT_CONTENT},{ACTIVE_CONTENT},{content},{count},{batch},{},{live},1,{},{},{builds},0,{}",
                    usize::from(ui),
                    if changing {
                        "scroll_update"
                    } else {
                        "idle_update"
                    },
                    timing_columns(&values),
                    count * content,
                    state.fixture.retained(),
                    if changing { samples * batch } else { 0 },
                );
            }
        }
        state.fixture.release_parent_scope();
        assert_eq!(
            state
                .fixture
                .app
                .world_mut()
                .query_filtered::<Entity, With<ParentContent>>()
                .iter(state.fixture.app.world())
                .count(),
            0,
        );
    }
}
