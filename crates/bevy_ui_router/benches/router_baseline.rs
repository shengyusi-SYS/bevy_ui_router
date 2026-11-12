//! Public-API, headless Router CPU-side elapsed-time baseline. See docs/performance.md.
//! No renderer, OS events, async I/O, or frame pacing is installed. The optional
//! transitions UI mode installs normal UI layout and native picking systems.
//! ROUTER_BENCH_SHAPE=flat (default) or nested selects equal-sized content trees.
//! ROUTER_BENCH_MODE=keep_alive selects the separate retention comparison CSV.
//! ROUTER_BENCH_MODE=instances measures parent/child inventory coordination.
//! ROUTER_BENCH_MODE=steady isolates initial dynamic content and ordinary scroll observation.
//! ROUTER_BENCH_MODE=retained_steady fixes active content while growing retained siblings.
#[path = "router_baseline/retained_instances.rs"]
mod retained_instances;
#[path = "router_baseline/retained_steady.rs"]
mod retained_steady;
#[path = "router_baseline/steady.rs"]
mod steady;
#[path = "router_baseline/transitions.rs"]
mod transitions;
use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::AssetPlugin;
use bevy_ecs::{prelude::*, system::RunSystemOnce};
use bevy_scene::{ResolvedScene, SceneFunction, ScenePlugin, bsn};
use bevy_ui::Node;
use bevy_ui_router::prelude::*;

const WARMUP_CYCLES: usize = 32;
const IDLE_BATCH: usize = 32;
const SOAK_BATCHES: usize = 8;

#[derive(Clone, Copy, Debug)]
enum ContentShape {
    Flat,
    Nested,
}

impl ContentShape {
    fn from_env() -> Self {
        match std::env::var("ROUTER_BENCH_SHAPE") {
            Err(std::env::VarError::NotPresent) => Self::Flat,
            Ok(value) if value == "flat" => Self::Flat,
            Ok(value) if value == "nested" => Self::Nested,
            value => panic!("ROUTER_BENCH_SHAPE must be flat or nested, got {value:?}"),
        }
    }

    fn levels(self) -> usize {
        match self {
            Self::Flat => 1,
            Self::Nested => 4,
        }
    }
}

// Distribute the same node budget across a shallow forest. Above the final
// level each parent has at most two children; no extra container nodes exist.
fn branches(content: usize, levels: usize) -> impl Iterator<Item = usize> {
    let roots = if levels == 1 { content } else { content.min(2) };
    (0..roots).map(move |index| content / roots + usize::from(index < content % roots))
}

fn append_content(root: &mut ResolvedScene, content: usize, levels: usize) {
    for size in branches(content, levels) {
        let mut child = ResolvedScene::default();
        child.push_template(Node::default());
        child.push_template(Content);
        if size > 1 {
            append_content(&mut child, size - 1, levels - 1);
        }
        root.get_or_insert_related_resolved_scenes::<ChildOf>()
            .scenes
            .push(child);
    }
}

fn spawn_control_content(world: &mut World, parent: Entity, content: usize, levels: usize) {
    for size in branches(content, levels) {
        let child = world
            .spawn((Node::default(), Content, ChildOf(parent)))
            .id();
        if size > 1 {
            spawn_control_content(world, child, size - 1, levels - 1);
        }
    }
}

fn assert_content_shape(app: &mut App, page: Entity, count: usize, shape: ContentShape) {
    let content = app
        .world_mut()
        .query_filtered::<Entity, With<Content>>()
        .iter(app.world())
        .collect::<Vec<_>>();
    assert_eq!(content.len(), count);
    let mut max_depth = 0;
    for child in content {
        let mut current = child;
        let mut depth = 0;
        while current != page {
            assert!(app.world().get::<Content>(current).is_some());
            assert!(app.world().get::<Node>(current).is_some());
            current = app.world().get::<ChildOf>(current).unwrap().parent();
            depth += 1;
            assert!(
                depth <= shape.levels(),
                "content must reach its page within the expected depth"
            );
        }
        max_depth = max_depth.max(depth);
    }
    // Existing sizes: one leaf, or enough nodes to populate all four levels.
    assert_eq!(
        max_depth,
        match count {
            0 => 0,
            1 => 1,
            _ => shape.levels(),
        }
    );
}

#[derive(Component, Clone, Default)]
struct Layout;
#[derive(Component, Clone, Default)]
struct Toolbar(u32);
#[derive(Component, Clone, Default)]
struct Page(String);
#[derive(Component, Clone, Default)]
struct Content;
#[derive(Component, Clone, Default)]
struct ErrorPage;

fn base_app() -> App {
    let mut app = App::new();
    app.add_plugins((
        TaskPoolPlugin::default(),
        AssetPlugin::default(),
        ScenePlugin,
    ));
    app
}

fn only<T: Component>(app: &mut App) -> Entity {
    app.world_mut()
        .query_filtered::<Entity, With<T>>()
        .single(app.world())
        .expect("exactly one authored entity")
}

struct Fixture {
    app: App,
    router: RouterHandle,
    host: Entity,
    content: usize,
    shape: ContentShape,
    issued: Vec<NavigationRequestId>,
    factory_calls: Option<Arc<AtomicUsize>>,
}

#[derive(Clone, Copy)]
enum Action {
    Push,
    Replace,
    Back,
}

#[derive(Clone, Copy)]
struct Sample {
    elapsed: Duration,
    updates: usize,
}

impl Fixture {
    fn new(content: usize, shape: ContentShape) -> Self {
        Self::configured(content, shape, None)
    }

    fn configured(content: usize, shape: ContentShape, capacity: Option<usize>) -> Self {
        Self::configured_layers(content, shape, capacity, false)
    }

    fn configured_layers(
        content: usize,
        shape: ContentShape,
        capacity: Option<usize>,
        middle: bool,
    ) -> Self {
        let mut app = base_app();
        let factory_calls = capacity.map(|_| Arc::new(AtomicUsize::new(0)));
        let page_calls = factory_calls.clone();
        let layout = app
            .register_bsn_view(ViewDriverKey::new("layout").unwrap(), |_| {
                Ok(bsn! {
                    Node Layout Children [Node Toolbar(73)
                        --
                        Node DefaultOutlet
                    ]
                })
            })
            .unwrap();
        let page = app
            .register_bsn_view(ViewDriverKey::new("page").unwrap(), move |input| {
                if let Some(calls) = &page_calls {
                    calls.fetch_add(1, Ordering::Relaxed);
                }
                let id = input.params().get("id").unwrap().to_owned();
                Ok(SceneFunction(move |_, root: &mut ResolvedScene| {
                    root.push_template(Node::default());
                    root.push_template(Page(id));
                    if matches!(shape, ContentShape::Nested) {
                        append_content(root, content, shape.levels());
                        return;
                    }
                    for _ in 0..content {
                        let mut child = ResolvedScene::default();
                        child.push_template(Node::default());
                        child.push_template(Content);
                        root.get_or_insert_related_resolved_scenes::<ChildOf>()
                            .scenes
                            .push(child);
                    }
                }))
            })
            .unwrap();
        let broken = app
            .register_bsn_view(ViewDriverKey::new("broken").unwrap(), |_| {
                Err::<(), _>(BsnBuildError::new("baseline factory failure"))
            })
            .unwrap();
        let fallback = app
            .register_bsn_view(ViewDriverKey::new("fallback").unwrap(), |input| {
                assert!(input.failure().is_some());
                Ok(bsn! { Node ErrorPage })
            })
            .unwrap();
        let layout_key = RouteViewKey::new("layout").unwrap();
        let fallback_key = RouteViewKey::new("fallback").unwrap();
        let mut views = RouteViewRegistry::new();
        for (key, driver) in [
            ("layout", layout),
            ("page", page),
            ("broken", broken),
            ("fallback", fallback),
        ] {
            views
                .insert(
                    RouteViewKey::new(key).unwrap(),
                    RouteViewDescriptor::new(driver),
                )
                .unwrap();
        }
        let middle_key = middle.then(|| {
            let driver = app
                .register_bsn_view(ViewDriverKey::new("middle").unwrap(), |_| {
                    Ok(bsn! { Node DefaultOutlet })
                })
                .unwrap();
            let key = RouteViewKey::new("middle").unwrap();
            views
                .insert(key.clone(), RouteViewDescriptor::new(driver))
                .unwrap();
            key
        });
        let mut routes = RouteTree::builder()
            .root_with_options(
                RoutePattern::path(if capacity.is_some() { "/:section" } else { "/" }).unwrap(),
                RouteNodeOptions::new().with_view(layout_key),
            )
            .unwrap();
        let root = routes.root_id().unwrap();
        let page_parent = if let Some(key) = middle_key {
            routes
                .route_with_options(
                    root,
                    RoutePattern::pathless(),
                    RouteNodeOptions::new().with_view(key),
                )
                .unwrap()
        } else {
            root
        };
        let page_pattern = if capacity.is_some() {
            ":id"
        } else {
            "items/:id"
        };
        for (path, key) in [(page_pattern, "page"), ("broken", "broken")] {
            let mut options = RouteNodeOptions::new().with_view(RouteViewKey::new(key).unwrap());
            if key == "page" && capacity.is_some_and(|capacity| capacity > 0) {
                options = options.with_keep_alive(true);
            }
            routes
                .route_with_options(page_parent, RoutePattern::path(path).unwrap(), options)
                .unwrap();
        }
        let mut config = RouterConfig::new(routes.build().unwrap(), views, fallback_key);
        if let Some(capacity) = capacity.filter(|capacity| *capacity > 0) {
            config = config.with_keep_alive_capacity(capacity);
        }
        let router = app.add_ui_router(config).unwrap();
        let host = app
            .world_mut()
            .spawn((Node::default(), RouterHostRegistration::new(router)))
            .id();
        app.update();
        let mut fixture = Self {
            app,
            router,
            host,
            content,
            shape,
            issued: Vec::new(),
            factory_calls,
        };
        fixture.navigate(Action::Push, "/items/a", false);
        fixture.assert_page("a", 1, 0);
        fixture
    }

    fn snapshot(&self) -> &RouterSnapshot {
        self.app
            .world()
            .resource::<RouterSnapshots>()
            .get(self.router)
            .unwrap()
    }

    // Timing includes enqueue, ordinary App::update, and the public completion poll.
    // Fixture setup, reporting, and detailed assertions are outside the timer.
    fn navigate(&mut self, action: Action, path: &str, failure: bool) -> Sample {
        let location = RouteLocation::new(path);
        let start = Instant::now();
        let mut navigation = self.app.world_mut().resource_mut::<RouterNavigation>();
        let request = match action {
            Action::Push => navigation.push(self.router, location),
            Action::Replace => navigation.replace(self.router, location),
            Action::Back => navigation.back(self.router),
        }
        .unwrap();
        for updates in 1..=32 {
            self.app.update();
            let snapshots = self.app.world().resource::<RouterSnapshots>();
            let RequestLookup::Found(result) = snapshots.request(self.router, request) else {
                continue;
            };
            if !matches!(
                result.status(),
                RequestStatus::Succeeded | RequestStatus::Failed(_)
            ) {
                continue;
            }
            let router = snapshots.get(self.router).unwrap();
            if router.running_request().is_some() || router.cleanup().foreground_pending() {
                continue;
            }
            let elapsed = start.elapsed();
            assert_eq!(router.cleanup().quarantine_count(), 0);
            match (failure, result.status()) {
                (false, RequestStatus::Succeeded) => {}
                (true, RequestStatus::Failed(error)) => {
                    assert_eq!(error.kind(), NavigationFailureKind::Build);
                    assert!(error.message().contains("baseline factory failure"));
                }
                (_, status) => panic!("unexpected navigation result: {status:?}"),
            }
            self.issued.push(request);
            return Sample { elapsed, updates };
        }
        panic!(
            "navigation failed to settle within 32 updates: {:?}",
            self.snapshot()
        );
    }

    fn assert_page(&mut self, id: &str, entries: usize, cursor: usize) -> Entity {
        let page = only::<Page>(&mut self.app);
        assert_eq!(self.app.world().get::<Page>(page).unwrap().0, id);
        let content = self
            .app
            .world_mut()
            .query_filtered::<Entity, With<Content>>()
            .iter(self.app.world())
            .collect::<Vec<_>>();
        assert_eq!(content.len(), self.content);
        assert_content_shape(&mut self.app, page, self.content, self.shape);
        let gate = self.app.world().resource::<RouterInputGate>();
        assert!(gate.allows_keyboard(page));
        for child in content {
            assert!(gate.allows_keyboard(child));
        }
        let snapshot = self.snapshot();
        let location = RouteLocation::new(format!("/items/{id}"));
        assert_eq!(snapshot.active().location(), Some(&location));
        assert_eq!(snapshot.history().current(), Some(&location));
        assert_eq!(snapshot.history().entries().len(), entries);
        assert_eq!(snapshot.history().cursor(), Some(cursor));
        assert!(self.app.world().get_entity(self.host).is_ok());
        page
    }

    fn retained(&self) -> usize {
        let snapshots = self.app.world().resource::<RouterSnapshots>();
        self.issued
            .iter()
            .filter(|&&id| matches!(snapshots.request(self.router, id), RequestLookup::Found(_)))
            .count()
    }

    fn cycle(&mut self) -> [Sample; 3] {
        let layout = only::<Layout>(&mut self.app);
        let toolbar = only::<Toolbar>(&mut self.app);
        let outlet = only::<DefaultOutlet>(&mut self.app);
        let live = self.app.world().entities().count_spawned();
        let mut previous = only::<Page>(&mut self.app);
        let mut samples = Vec::with_capacity(3);
        for (action, id, cursor) in [
            (Action::Push, "b", 1),
            (Action::Replace, "c", 1),
            (Action::Back, "a", 0),
        ] {
            samples.push(self.navigate(action, &format!("/items/{id}"), false));
            assert!(self.app.world().get_entity(previous).is_err());
            previous = self.assert_page(id, 2, cursor);
            assert_eq!(only::<Layout>(&mut self.app), layout);
            assert_eq!(only::<Toolbar>(&mut self.app), toolbar);
            assert_eq!(only::<DefaultOutlet>(&mut self.app), outlet);
            assert_eq!(self.app.world().get::<Toolbar>(toolbar).unwrap().0, 73);
            assert_eq!(self.app.world().entities().count_spawned(), live);
        }
        samples.try_into().unwrap_or_else(|_| unreachable!())
    }

    fn fail_and_recover(&mut self) -> [Sample; 2] {
        let history = self.snapshot().history().clone();
        let live = self.app.world().entities().count_spawned();
        let failed = self.navigate(Action::Push, "/broken", true);
        assert_eq!(self.snapshot().history(), &history);
        assert_eq!(
            self.snapshot().active().kind(),
            ActivePresentationKind::Error
        );
        let error = only::<ErrorPage>(&mut self.app);
        assert!(
            self.app
                .world()
                .resource::<RouterInputGate>()
                .allows_keyboard(error)
        );
        assert_eq!(
            self.app
                .world_mut()
                .query_filtered::<Entity, With<Page>>()
                .iter(self.app.world())
                .count(),
            0
        );
        let recovered = self.navigate(Action::Replace, "/items/a", false);
        self.assert_page("a", history.entries().len(), history.cursor().unwrap());
        assert!(self.app.world().get_entity(error).is_err());
        assert_eq!(self.app.world().entities().count_spawned(), live);
        [failed, recovered]
    }
}

// Comparable authored content without Router; Router-owned wrappers are intentionally absent.
fn control(content: usize, shape: ContentShape) -> App {
    let mut app = base_app();
    let host = app.world_mut().spawn(Node::default()).id();
    let layout = app
        .world_mut()
        .spawn((Node::default(), Layout, ChildOf(host)))
        .id();
    app.world_mut()
        .spawn((Node::default(), Toolbar(73), ChildOf(layout)));
    let outlet = app
        .world_mut()
        .spawn((Node::default(), DefaultOutlet, ChildOf(layout)))
        .id();
    let page = app
        .world_mut()
        .spawn((Node::default(), Page("a".into()), ChildOf(outlet)))
        .id();
    match shape {
        ContentShape::Flat => {
            for _ in 0..content {
                app.world_mut()
                    .spawn((Node::default(), Content, ChildOf(page)));
            }
        }
        ContentShape::Nested => {
            spawn_control_content(app.world_mut(), page, content, shape.levels())
        }
    }
    assert_content_shape(&mut app, page, content, shape);
    app
}

fn idle(app: &mut App, samples: usize) -> Vec<Sample> {
    for _ in 0..64 {
        app.update();
    }
    (0..samples)
        .map(|_| {
            let start = Instant::now();
            for _ in 0..IDLE_BATCH {
                app.update();
            }
            Sample {
                elapsed: start.elapsed() / IDLE_BATCH as u32,
                updates: 1,
            }
        })
        .collect()
}

fn timing_columns(samples: &[Sample]) -> String {
    let mut times = samples
        .iter()
        .map(|s| s.elapsed.as_secs_f64() * 1e6)
        .collect::<Vec<_>>();
    times.sort_by(f64::total_cmp);
    let percentile = |p: usize| times[(times.len() * p).div_ceil(100) - 1];
    let min_updates = samples.iter().map(|s| s.updates).min().unwrap();
    let max_updates = samples.iter().map(|s| s.updates).max().unwrap();
    format!(
        "{},{:.3},{:.3},{:.3},{:.3},{min_updates},{max_updates}",
        samples.len(),
        times.iter().sum::<f64>() / times.len() as f64,
        percentile(50),
        percentile(95),
        times.last().unwrap()
    )
}

fn report(
    name: &str,
    content: usize,
    batch: usize,
    samples: &[Sample],
    live: u32,
    retained: usize,
) {
    println!(
        "{name},{content},{batch},{},{live},{retained}",
        timing_columns(samples)
    );
}

struct PageCensus {
    pages: Vec<(Entity, bool)>,
    active_content: usize,
    inactive_content: usize,
    live: u32,
}

impl PageCensus {
    fn active_page(&self) -> Entity {
        self.pages.iter().find(|(_, active)| *active).unwrap().0
    }
    fn inactive_pages(&self) -> usize {
        self.pages.iter().filter(|(_, active)| !active).count()
    }
}

impl Fixture {
    fn factory_count(&self) -> usize {
        self.factory_calls.as_ref().unwrap().load(Ordering::Relaxed)
    }

    fn census(&mut self, id: &str, inactive: usize) -> PageCensus {
        let (pages, active_content, inactive_content) = self
            .app
            .world_mut()
            .run_system_once(
                |activity: RouteActivity,
                 pages: Query<Entity, With<Page>>,
                 content: Query<Entity, With<Content>>| {
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
                    )
                },
            )
            .unwrap();
        let census = PageCensus {
            pages,
            active_content,
            inactive_content,
            live: self.app.world().entities().count_spawned(),
        };
        assert_eq!(census.pages.len(), inactive + 1);
        assert_eq!(census.inactive_pages(), inactive);
        assert_eq!(census.active_content, self.content);
        assert_eq!(census.inactive_content, self.content * inactive);
        let active = census.active_page();
        assert_eq!(self.app.world().get::<Page>(active).unwrap().0, id);
        let gate = self.app.world().resource::<RouterInputGate>();
        for (entity, active) in &census.pages {
            assert_eq!(gate.allows_keyboard(*entity), *active);
        }
        let snapshot = self.snapshot();
        let location = RouteLocation::new(format!("/items/{id}"));
        assert_eq!(snapshot.active().location(), Some(&location));
        assert_eq!(snapshot.history().current(), Some(&location));
        assert_eq!(snapshot.history().entries().len(), 1);
        assert_eq!(snapshot.history().cursor(), Some(0));
        census
    }

    fn release_parent_scope(&mut self) {
        let old = self
            .app
            .world_mut()
            .query_filtered::<Entity, Or<(With<Page>, With<Content>)>>()
            .iter(self.app.world())
            .collect::<Vec<_>>();
        let layout = only::<Layout>(&mut self.app);
        self.navigate(Action::Replace, "/outside", false);
        assert!(
            old.into_iter()
                .all(|entity| self.app.world().get_entity(entity).is_err())
        );
        assert!(self.app.world().get_entity(layout).is_err());
        assert_eq!(
            self.app
                .world_mut()
                .query_filtered::<Entity, Or<(With<Page>, With<Content>)>>()
                .iter(self.app.world())
                .count(),
            0
        );
        assert!(self.app.world().get_entity(self.host).is_ok());
    }
}

fn report_keep_alive(
    name: &str,
    content: usize,
    capacity: usize,
    samples: &[Sample],
    census: &PageCensus,
    builds: usize,
) {
    println!(
        "{name},{content},{capacity},{},{},1,{},{builds}",
        timing_columns(samples),
        census.live,
        census.inactive_pages()
    );
}

fn keep_alive(content: usize, shape: ContentShape, samples: usize) {
    for (name, capacity, ids, inactive) in [
        ("disabled", 0, &["b", "a"][..], 0),
        ("miss", 2, &["b"][..], 1),
        ("hit", 2, &["b", "a"][..], 1),
        ("lru", 2, &["b", "c", "d", "a"][..], 2),
    ] {
        let mut fixture = Fixture::configured(content, shape, Some(capacity));
        let mut values = Vec::with_capacity(samples);
        let mut builds = 0;
        let mut stable_live = None;
        for index in 0..WARMUP_CYCLES + samples {
            if name == "miss" && index != 0 {
                // A first miss is measured separately from full-capacity eviction.
                fixture.release_parent_scope();
                fixture.navigate(Action::Replace, "/items/a", false);
            }
            let id = ids[index % ids.len()];
            let previous = fixture
                .app
                .world_mut()
                .query::<(Entity, &Page)>()
                .iter(fixture.app.world())
                .map(|(entity, page)| (entity, page.0.clone()))
                .collect::<Vec<_>>();
            let layout = only::<Layout>(&mut fixture.app);
            let toolbar = only::<Toolbar>(&mut fixture.app);
            let before_builds = fixture.factory_count();
            let sample = fixture.navigate(Action::Replace, &format!("/items/{id}"), false);
            let built = fixture.factory_count() - before_builds;
            let census = fixture.census(id, inactive.min(index + 1));
            assert_eq!(only::<Layout>(&mut fixture.app), layout);
            assert_eq!(only::<Toolbar>(&mut fixture.app), toolbar);
            assert_eq!(fixture.app.world().get::<Toolbar>(toolbar).unwrap().0, 73);
            if name == "hit" && index > 0 {
                assert_eq!(built, 0);
                let original = previous.iter().find(|(_, page)| page == id).unwrap().0;
                assert_eq!(census.active_page(), original);
            } else {
                assert_eq!(built, 1);
                assert!(
                    previous
                        .iter()
                        .all(|(entity, _)| *entity != census.active_page())
                );
            }
            if name == "disabled" {
                assert!(
                    previous.iter().all(|(entity, _)| fixture
                        .app
                        .world()
                        .get_entity(*entity)
                        .is_err())
                );
            }
            if name == "lru" && index >= 2 {
                let oldest_id = ids[(index + 1) % ids.len()];
                let oldest = previous.iter().find(|(_, id)| id == oldest_id).unwrap().0;
                assert!(fixture.app.world().get_entity(oldest).is_err());
                assert_eq!(previous.len(), 3);
                assert_eq!(
                    previous
                        .iter()
                        .filter(|(entity, _)| fixture.app.world().get_entity(*entity).is_err())
                        .count(),
                    1
                );
                // The other two instances survive the selected oldest eviction.
                assert_eq!(
                    census
                        .pages
                        .iter()
                        .filter(|(entity, _)| previous.iter().any(|(old, _)| old == entity))
                        .count(),
                    2
                );
            }
            if index >= WARMUP_CYCLES {
                assert_eq!(census.live, *stable_live.get_or_insert(census.live));
                values.push(sample);
                builds += built;
            }
        }
        let current = ids[(WARMUP_CYCLES + samples - 1) % ids.len()];
        let census = fixture.census(current, inactive);
        report_keep_alive(
            &format!("{name}_replace"),
            content,
            capacity,
            &values,
            &census,
            builds,
        );
        let before_builds = fixture.factory_count();
        let values = idle(&mut fixture.app, samples);
        let after = fixture.census(current, inactive);
        assert_eq!(after.live, census.live);
        assert_eq!(fixture.factory_count(), before_builds);
        report_keep_alive(
            &format!("{name}_idle"),
            content,
            capacity,
            &values,
            &after,
            0,
        );
        fixture.release_parent_scope();
    }
}

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    for arg in &args {
        assert!(
            matches!(arg.as_str(), "--bench" | "--test"),
            "supported arguments: --bench, --test (smoke only)"
        );
    }
    let shape = ContentShape::from_env();
    let smoke = args.iter().any(|arg| arg == "--test");
    let samples = if smoke {
        2
    } else {
        assert!(
            !cfg!(debug_assertions),
            "use cargo bench for optimized measurements; --test runs a smoke check"
        );
        let default_samples =
            if std::env::var("ROUTER_BENCH_MODE").as_deref() == Ok("retained_steady") {
                32
            } else {
                128
            };
        std::env::var("ROUTER_BENCH_SAMPLES").map_or(default_samples, |value| {
            value.parse().expect("positive sample count")
        })
    };
    assert!(samples > 0);
    eprintln!(
        "Router baseline: {}-{}, optimized={}, shape={shape:?}, samples={samples}, idle frames/sample={IDLE_BATCH}; {}",
        std::env::consts::OS,
        std::env::consts::ARCH,
        !cfg!(debug_assertions),
        if smoke {
            "SMOKE ONLY, timings are not a baseline"
        } else {
            "CPU-side wall time, no renderer/GPU/I/O; selected mode controls UI layout"
        }
    );
    match std::env::var("ROUTER_BENCH_MODE") {
        Ok(mode) if mode == "transitions" => {
            transitions::run(shape, samples);
            return;
        }
        Ok(mode) if mode == "steady" => {
            steady::run(shape, samples);
            return;
        }
        Ok(mode) if mode == "retained_steady" => {
            retained_steady::run(shape, samples, smoke);
            return;
        }
        Ok(mode) if mode == "instances" => {
            retained_instances::run(shape, samples);
            return;
        }
        Ok(mode) if mode == "keep_alive" => {
            println!(
                "scenario,content_children,capacity,samples,mean_us,p50_us,p95_us,max_us,min_updates,max_updates,live_entities,active_pages,inactive_pages,factory_builds"
            );
            for content in [1, 32, 128] {
                keep_alive(content, shape, samples);
            }
            return;
        }
        Err(std::env::VarError::NotPresent) => {}
        mode => {
            panic!(
                "ROUTER_BENCH_MODE must be unset, steady, retained_steady, keep_alive, instances or transitions, got {mode:?}"
            )
        }
    }
    println!(
        "scenario,content_children,batch,samples,mean_us,p50_us,p95_us,max_us,min_updates,max_updates,live_entities,retained_requests"
    );
    for content in [1, 32, 128] {
        let mut plain = control(content, shape);
        let values = idle(&mut plain, samples);
        report(
            "idle_control",
            content,
            0,
            &values,
            plain.world().entities().count_spawned(),
            0,
        );
        let mut fixture = Fixture::new(content, shape);
        let values = idle(&mut fixture.app, samples);
        fixture.assert_page("a", 1, 0);
        report(
            "idle_one_request",
            content,
            0,
            &values,
            fixture.app.world().entities().count_spawned(),
            fixture.retained(),
        );
        // Keep identical current location and one-entry history in both idle cases.
        for _ in 0..WARMUP_CYCLES {
            for id in ["b", "a"] {
                let previous = only::<Page>(&mut fixture.app);
                fixture.navigate(Action::Replace, &format!("/items/{id}"), false);
                fixture.assert_page(id, 1, 0);
                assert!(fixture.app.world().get_entity(previous).is_err());
            }
        }
        let retained = fixture.retained();
        assert!(
            retained < fixture.issued.len(),
            "warmup must reach request eviction"
        );
        let values = idle(&mut fixture.app, samples);
        fixture.assert_page("a", 1, 0);
        report(
            "idle_retained",
            content,
            0,
            &values,
            fixture.app.world().entities().count_spawned(),
            retained,
        );
        for _ in 0..WARMUP_CYCLES {
            fixture.cycle();
        }
        let mut navigation = [Vec::new(), Vec::new(), Vec::new()];
        for _ in 0..samples {
            for (values, sample) in navigation.iter_mut().zip(fixture.cycle()) {
                values.push(sample);
            }
        }
        for (name, values) in ["push", "replace", "back"].into_iter().zip(&navigation) {
            report(
                name,
                content,
                0,
                values,
                fixture.app.world().entities().count_spawned(),
                fixture.retained(),
            );
        }
        // Warm the failure/recovery path separately from normal navigation.
        for _ in 0..WARMUP_CYCLES {
            fixture.fail_and_recover();
        }
        let mut recovery = [Vec::new(), Vec::new()];
        for _ in 0..samples {
            for (values, sample) in recovery.iter_mut().zip(fixture.fail_and_recover()) {
                values.push(sample);
            }
        }
        for (name, values) in ["failure", "recover"].into_iter().zip(&recovery) {
            report(
                name,
                content,
                0,
                values,
                fixture.app.world().entities().count_spawned(),
                fixture.retained(),
            );
        }
        // A fresh, normally warmed fixture keeps failure diagnostics out of soak comparisons.
        let mut fixture = Fixture::new(content, shape);
        for _ in 0..WARMUP_CYCLES {
            fixture.cycle();
        }
        let retained = fixture.retained();
        let live = fixture.app.world().entities().count_spawned();
        for batch in 1..=SOAK_BATCHES {
            let mut values = Vec::with_capacity(samples * 3);
            for _ in 0..samples {
                values.extend(fixture.cycle());
            }
            assert_eq!(fixture.retained(), retained);
            assert_eq!(fixture.app.world().entities().count_spawned(), live);
            report(
                "soak_cycle_operations",
                content,
                batch,
                &values,
                live,
                retained,
            );
        }
    }
}
