//! Direct Bevy Scene creation/deletion cost reference, not equivalent Router semantics.
//! Same base plugins and authored page content as router_baseline; no Router is installed.
//! No renderer, UI layout, OS events, async I/O, or frame pacing is installed.
use std::time::{Duration, Instant};

use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::{AssetPlugin, AssetServer, Assets};
use bevy_ecs::{bundle::BundleScratch, prelude::*};
use bevy_scene::{
    ResolvedScene, ResolvedSceneRoot, Scene, SceneFunction, ScenePatch, ScenePlugin, bsn,
};
use bevy_ui::Node;

const WARMUP_CYCLES: usize = 32;
const SOAK_BATCHES: usize = 8;

#[derive(Component, Clone, Default)]
struct Layout;
#[derive(Component, Clone, Default)]
struct Toolbar(u32);
#[derive(Component, Clone, Default)]
struct Outlet;
#[derive(Component, Clone, Default)]
struct Page(String);
#[derive(Component, Clone, Default)]
struct Content;

fn resolve(app: &App, scene: impl Scene) -> ResolvedSceneRoot {
    ResolvedSceneRoot::resolve(
        Box::new(scene),
        app.world().resource::<AssetServer>(),
        app.world().resource::<Assets<ScenePatch>>(),
    )
    .expect("resolve authored scene")
}

fn only<T: Component>(app: &mut App) -> Entity {
    app.world_mut()
        .query_filtered::<Entity, With<T>>()
        .single(app.world())
        .expect("exactly one authored entity")
}

struct Fixture {
    app: App,
    host: Entity,
    layout: Entity,
    toolbar: Entity,
    outlet: Entity,
    page: Option<Entity>,
    content: usize,
    live: u32,
}

impl Fixture {
    fn new(content: usize) -> Self {
        let mut app = App::new();
        app.add_plugins((
            TaskPoolPlugin::default(),
            AssetPlugin::default(),
            ScenePlugin,
        ));
        let host = app.world_mut().spawn(Node::default()).id();
        let scene = resolve(
            &app,
            bsn! { Node Layout Children [(Node Toolbar(73)), (Node Outlet)] },
        );
        let layout = app.world_mut().spawn(ChildOf(host)).id();
        scene
            .apply(
                &mut app.world_mut().entity_mut(layout),
                &mut BundleScratch::default(),
            )
            .unwrap();
        let toolbar = only::<Toolbar>(&mut app);
        let outlet = only::<Outlet>(&mut app);
        app.update();
        let mut fixture = Self {
            app,
            host,
            layout,
            toolbar,
            outlet,
            page: None,
            content,
            live: 0,
        };
        fixture.replace("a");
        fixture.live = fixture.app.world().entities().count_spawned();
        fixture.assert_current("a", &[]);
        fixture
    }

    // Includes SceneFunction construction, resolve, root allocation, official apply,
    // old page recursive despawn + flush, and one ordinary App::update.
    // Capturing old entity IDs and all detailed assertions are outside the timer.
    // Unlike Router this has no history, ownership validation, input gate, or failure protocol.
    fn replace(&mut self, id: &str) -> Duration {
        let old = self
            .page
            .map(|page| {
                let mut entities = vec![page];
                if let Some(children) = self.app.world().get::<Children>(page) {
                    entities.extend(children.iter());
                }
                entities
            })
            .unwrap_or_default();
        let start = Instant::now();
        let id_owned = id.to_owned();
        let content = self.content;
        let scene = resolve(
            &self.app,
            SceneFunction(move |_, root: &mut ResolvedScene| {
                root.push_template(Node::default());
                root.push_template(Page(id_owned));
                for _ in 0..content {
                    let mut child = ResolvedScene::default();
                    child.push_template(Node::default());
                    child.push_template(Content);
                    root.get_or_insert_related_resolved_scenes::<ChildOf>()
                        .scenes
                        .push(child);
                }
            }),
        );
        let page = self.app.world_mut().spawn(ChildOf(self.outlet)).id();
        scene
            .apply(
                &mut self.app.world_mut().entity_mut(page),
                &mut BundleScratch::default(),
            )
            .unwrap();
        drop(scene);
        if let Some(previous) = self.page.replace(page) {
            self.app.world_mut().entity_mut(previous).despawn();
        }
        self.app.world_mut().flush();
        self.app.update();
        let elapsed = start.elapsed();
        if self.live != 0 {
            self.assert_current(id, &old);
        }
        elapsed
    }

    fn assert_current(&mut self, id: &str, old: &[Entity]) {
        let page = self.page.unwrap();
        assert_eq!(only::<Page>(&mut self.app), page);
        let world = self.app.world();
        assert_eq!(world.get::<Page>(page).unwrap().0, id);
        assert_eq!(world.get::<ChildOf>(page).unwrap().parent(), self.outlet);
        assert_eq!(
            world.get::<ChildOf>(self.layout).unwrap().parent(),
            self.host
        );
        assert_eq!(
            world.get::<ChildOf>(self.toolbar).unwrap().parent(),
            self.layout
        );
        assert_eq!(
            world.get::<ChildOf>(self.outlet).unwrap().parent(),
            self.layout
        );
        assert_eq!(world.get::<Toolbar>(self.toolbar).unwrap().0, 73);
        assert!(world.get::<Node>(self.host).is_some());
        assert!(world.get::<Layout>(self.layout).is_some());
        assert!(world.get::<Outlet>(self.outlet).is_some());
        assert_eq!(world.get::<Children>(self.layout).unwrap().len(), 2);
        assert_eq!(
            world.get::<Children>(self.outlet).unwrap().as_ref(),
            &[page]
        );
        assert_eq!(world.get::<Children>(page).unwrap().len(), self.content);
        for child in world.get::<Children>(page).unwrap().iter() {
            assert!(world.get::<Content>(child).is_some());
            assert!(world.get::<Node>(child).is_some());
            assert_eq!(world.get::<ChildOf>(child).unwrap().parent(), page);
        }
        for entity in old {
            assert!(
                world.get_entity(*entity).is_err(),
                "old page content must be gone"
            );
        }
        assert_eq!(world.entities().count_spawned(), self.live);
        let count = self
            .app
            .world_mut()
            .query_filtered::<Entity, With<Content>>()
            .iter(self.app.world())
            .count();
        assert_eq!(count, self.content);
    }
}

fn report(content: usize, batch: usize, samples: &[Duration], live: u32) {
    let mut times = samples
        .iter()
        .map(|s| s.as_secs_f64() * 1_000_000.0)
        .collect::<Vec<_>>();
    times.sort_by(f64::total_cmp);
    let percentile = |p: usize| times[(times.len() * p).div_ceil(100) - 1];
    println!(
        "direct_bsn_replace,{content},{batch},{},{:.3},{:.3},{:.3},{:.3},1,1,{live}",
        times.len(),
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
        "Direct BSN cost reference: {}-{}, optimized={}, samples={samples}; {}. Includes resolve/apply/despawn/flush/one App::update; no Router semantics, GPU/layout/I/O.",
        std::env::consts::OS,
        std::env::consts::ARCH,
        !cfg!(debug_assertions),
        if smoke {
            "SMOKE ONLY, timings are not a baseline"
        } else {
            "CPU-side wall time"
        }
    );
    println!(
        "scenario,content_children,batch,samples,mean_us,p50_us,p95_us,max_us,min_updates,max_updates,live_entities"
    );
    for content in [1, 32, 128] {
        let mut fixture = Fixture::new(content);
        for _ in 0..WARMUP_CYCLES {
            fixture.replace("b");
            fixture.replace("a");
        }
        for batch in 0..=SOAK_BATCHES {
            let values = (0..samples)
                .map(|i| fixture.replace(if i % 2 == 0 { "b" } else { "a" }))
                .collect::<Vec<_>>();
            report(content, batch, &values, fixture.live);
        }
    }
}
