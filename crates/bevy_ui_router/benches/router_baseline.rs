//! Public-API, headless Router CPU-side elapsed-time baseline. See docs/performance.md.
//! No renderer, UI layout, OS events, async I/O, or frame pacing is installed.
use std::time::{Duration, Instant};

use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::AssetPlugin;
use bevy_ecs::prelude::*;
use bevy_scene::{ResolvedScene, SceneFunction, ScenePlugin, bsn};
use bevy_ui::Node;
use bevy_ui_router::prelude::*;

const WARMUP_CYCLES: usize = 32;
const IDLE_BATCH: usize = 32;
const SOAK_BATCHES: usize = 8;

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
    issued: Vec<NavigationRequestId>,
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
    fn new(content: usize) -> Self {
        let mut app = base_app();
        let layout = app
            .register_bsn_view(ViewDriverKey::new("layout").unwrap(), |_| {
                Ok(bsn! { Node Layout Children [(Node Toolbar(73)), (Node DefaultOutlet)] })
            })
            .unwrap();
        let page = app
            .register_bsn_view(ViewDriverKey::new("page").unwrap(), move |input| {
                let id = input.params().get("id").unwrap().to_owned();
                Ok(SceneFunction(move |_, root: &mut ResolvedScene| {
                    root.push_template(Node::default());
                    root.push_template(Page(id));
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
        let mut routes = RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/").unwrap(),
                RouteNodeOptions::new().with_view(layout_key),
            )
            .unwrap();
        let root = routes.root_id().unwrap();
        for (path, key) in [("items/:id", "page"), ("broken", "broken")] {
            routes
                .route_with_options(
                    root,
                    RoutePattern::path(path).unwrap(),
                    RouteNodeOptions::new().with_view(RouteViewKey::new(key).unwrap()),
                )
                .unwrap();
        }
        let router = app
            .add_ui_router(RouterConfig::new(
                routes.build().unwrap(),
                views,
                fallback_key,
            ))
            .unwrap();
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
            issued: Vec::new(),
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
        let gate = self.app.world().resource::<RouterInputGate>();
        assert!(gate.allows_keyboard(page));
        for child in content {
            assert_eq!(
                self.app.world().get::<ChildOf>(child).unwrap().parent(),
                page
            );
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
fn control(content: usize) -> App {
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
    for _ in 0..content {
        app.world_mut()
            .spawn((Node::default(), Content, ChildOf(page)));
    }
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

fn report(
    name: &str,
    content: usize,
    batch: usize,
    samples: &[Sample],
    live: u32,
    retained: usize,
) {
    let mut times = samples
        .iter()
        .map(|s| s.elapsed.as_secs_f64() * 1e6)
        .collect::<Vec<_>>();
    times.sort_by(f64::total_cmp);
    let percentile = |p: usize| times[(times.len() * p).div_ceil(100) - 1];
    let min_updates = samples.iter().map(|s| s.updates).min().unwrap();
    let max_updates = samples.iter().map(|s| s.updates).max().unwrap();
    println!(
        "{name},{content},{batch},{},{:.3},{:.3},{:.3},{:.3},{min_updates},{max_updates},{live},{retained}",
        samples.len(),
        times.iter().sum::<f64>() / times.len() as f64,
        percentile(50),
        percentile(95),
        times.last().unwrap()
    );
}

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    for arg in &args {
        assert!(
            matches!(arg.as_str(), "--bench" | "--test"),
            "supported arguments: --bench, --test (smoke only)"
        );
    }
    let smoke = args.iter().any(|arg| arg == "--test");
    let samples = if smoke {
        2
    } else {
        assert!(
            !cfg!(debug_assertions),
            "use cargo bench for optimized measurements; --test runs a smoke check"
        );
        std::env::var("ROUTER_BENCH_SAMPLES")
            .map_or(128, |value| value.parse().expect("positive sample count"))
    };
    assert!(samples > 0);
    eprintln!(
        "Router baseline: {}-{}, optimized={}, samples={samples}, idle frames/sample={IDLE_BATCH}; {}",
        std::env::consts::OS,
        std::env::consts::ARCH,
        !cfg!(debug_assertions),
        if smoke {
            "SMOKE ONLY, timings are not a baseline"
        } else {
            "CPU-side wall time, no GPU/layout/I/O"
        }
    );
    println!(
        "scenario,content_children,batch,samples,mean_us,p50_us,p95_us,max_us,min_updates,max_updates,live_entities,retained_requests"
    );
    for content in [1, 32, 128] {
        let mut plain = control(content);
        let values = idle(&mut plain, samples);
        report(
            "idle_control",
            content,
            0,
            &values,
            plain.world().entities().count_spawned(),
            0,
        );
        let mut fixture = Fixture::new(content);
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
        let mut fixture = Fixture::new(content);
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
