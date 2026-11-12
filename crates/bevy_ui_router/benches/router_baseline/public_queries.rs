//! Resident application systems, with one active page and one retained sibling.
//! Scroll lookups intentionally measure misses: this headless fixture has no
//! normal UI layout or displayed history scroll record.
use super::*;
use bevy_app::Update;
use bevy_ui_router::ecs::RouterScheduleSet;
use std::hint::black_box;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Api {
    Empty,
    Activity,
    Inputs,
    Scroll,
}

impl Api {
    fn name(self) -> &'static str {
        match self {
            Self::Empty => "empty_loop",
            Self::Activity => "legacy_activity",
            Self::Inputs => "legacy_inputs",
            Self::Scroll => "legacy_scroll_miss",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Targets {
    SameRoot,
    PerTarget,
}

impl Targets {
    fn name(self) -> &'static str {
        match self {
            Self::SameRoot => "same_root",
            Self::PerTarget => "per_target",
        }
    }
}

#[derive(Resource)]
struct Workload {
    api: Api,
    targets: Targets,
    root: Entity,
}

impl Workload {
    fn target(&self, entity: Entity) -> Entity {
        match self.targets {
            Targets::SameRoot => self.root,
            Targets::PerTarget => entity,
        }
    }
}

#[derive(Component)]
struct ConsumerWork {
    active: bool,
    observed: bool,
    visits: usize,
}

fn empty_loop(config: Res<Workload>, mut work: Query<(Entity, &mut ConsumerWork)>) {
    for (entity, mut work) in &mut work {
        black_box(config.target(entity));
        work.observed = true;
        work.visits += 1;
    }
}

fn activity_loop(
    config: Res<Workload>,
    activity: RouteActivity,
    mut work: Query<(Entity, &mut ConsumerWork)>,
) {
    for (entity, mut work) in &mut work {
        work.observed = black_box(activity.is_active(config.target(entity)));
        work.visits += 1;
    }
}

fn inputs_loop(
    config: Res<Workload>,
    inputs: RouteInputs,
    mut work: Query<(Entity, &mut ConsumerWork)>,
) {
    for (entity, mut work) in &mut work {
        work.observed = inputs.current(config.target(entity)).is_some_and(|input| {
            black_box(input.location().path());
            true
        });
        work.visits += 1;
    }
}

fn scroll_loop(
    config: Res<Workload>,
    restorations: RouteScrollRestorations,
    mut work: Query<(Entity, &mut ConsumerWork)>,
) {
    for (entity, mut work) in &mut work {
        work.observed = black_box(restorations.pending(config.target(entity), "body")).is_some();
        work.visits += 1;
    }
}

fn populate(fixture: &mut Fixture, page: Entity, content: usize, active: bool) {
    spawn_control_content(
        fixture.app.world_mut(),
        page,
        content,
        fixture.shape.levels(),
    );
    let added = fixture
        .app
        .world_mut()
        .query_filtered::<Entity, (With<Content>, Without<ConsumerWork>)>()
        .iter(fixture.app.world())
        .collect::<Vec<_>>();
    assert_eq!(added.len(), content);
    for entity in added {
        fixture
            .app
            .world_mut()
            .entity_mut(entity)
            .insert(ConsumerWork {
                active,
                observed: false,
                visits: 0,
            });
    }
    fixture.app.update();
}

fn fixture(content: usize, shape: ContentShape) -> (Fixture, Entity, Entity) {
    let mut fixture = Fixture::configured(0, shape, Some(2));
    let active = only::<Page>(&mut fixture.app);
    populate(&mut fixture, active, content, true);
    fixture.navigate(Action::Push, "/items/b", false);
    let inactive = fixture
        .app
        .world_mut()
        .query::<(Entity, &Page)>()
        .iter(fixture.app.world())
        .find(|(_, page)| page.0 == "b")
        .unwrap()
        .0;
    populate(&mut fixture, inactive, content, false);
    fixture.navigate(Action::Back, "/items/a", false);
    fixture.app.insert_resource(Workload {
        api: Api::Empty,
        targets: Targets::PerTarget,
        root: active,
    });
    // All rows pay the same four cheap run conditions. Only the selected
    // consumer executes, with its own normal typed SystemParam access.
    fixture.app.add_systems(
        Update,
        (
            empty_loop.run_if(|config: Res<Workload>| config.api == Api::Empty),
            activity_loop.run_if(|config: Res<Workload>| config.api == Api::Activity),
            inputs_loop.run_if(|config: Res<Workload>| config.api == Api::Inputs),
            scroll_loop.run_if(|config: Res<Workload>| config.api == Api::Scroll),
        )
            .after(RouterScheduleSet::PublishSnapshots),
    );
    (fixture, active, inactive)
}

fn assert_behavior(fixture: &mut Fixture, active: Entity, inactive: Entity, content: usize) {
    fixture
        .app
        .world_mut()
        .run_system_once(
            move |activity: RouteActivity,
                  inputs: RouteInputs,
                  scroll: RouteScrollRestorations,
                  work: Query<(Entity, &ConsumerWork)>| {
                assert!(activity.is_active(active));
                assert_eq!(
                    inputs.current(active).unwrap().location().path(),
                    "/items/a"
                );
                assert!(scroll.pending(active, "body").is_none());
                assert!(!activity.is_active(inactive));
                assert!(inputs.current(inactive).is_none());
                assert!(scroll.pending(inactive, "body").is_none());
                let mut active_count = 0;
                let mut inactive_count = 0;
                for (entity, work) in &work {
                    assert_eq!(activity.is_active(entity), work.active);
                    assert_eq!(inputs.current(entity).is_some(), work.active);
                    assert!(scroll.pending(entity, "body").is_none());
                    if work.active {
                        active_count += 1;
                    } else {
                        inactive_count += 1;
                    }
                }
                assert_eq!((active_count, inactive_count), (content, content));
            },
        )
        .unwrap();
    let snapshot = fixture.snapshot();
    assert_eq!(snapshot.active().location().unwrap().path(), "/items/a");
    assert_eq!(snapshot.history().entries().len(), 2);
    assert_eq!(snapshot.history().cursor(), Some(0));
    assert!(snapshot.running_request().is_none());
    assert!(!snapshot.cleanup().foreground_pending());
    assert_eq!(snapshot.cleanup().quarantine_count(), 0);
}

fn assert_consumer(fixture: &mut Fixture, api: Api, targets: Targets, visits: usize) {
    let mut query = fixture.app.world_mut().query::<&ConsumerWork>();
    for work in query.iter(fixture.app.world()) {
        let expected = match api {
            Api::Empty => true,
            Api::Activity | Api::Inputs => targets == Targets::SameRoot || work.active,
            Api::Scroll => false,
        };
        assert_eq!(work.observed, expected);
        assert_eq!(work.visits, visits);
    }
}

fn measure(app: &mut App, samples: usize, batch: usize) -> Vec<Sample> {
    (0..samples)
        .map(|_| {
            let start = Instant::now();
            for _ in 0..batch {
                app.update();
            }
            Sample {
                elapsed: start.elapsed() / batch as u32,
                updates: batch,
            }
        })
        .collect()
}

pub(super) fn run(shape: ContentShape, samples: usize, smoke: bool) {
    let sizes: &[usize] = if smoke { &[32] } else { &[32, 1024] };
    let batch = std::env::var("ROUTER_BENCH_BATCH").map_or(if smoke { 2 } else { 4 }, |value| {
        value.parse::<usize>().expect("positive update batch")
    });
    assert!(batch > 0);
    let warmup = if smoke { 2 } else { 16 };
    let shape_name = match shape {
        ContentShape::Flat => "flat",
        ContentShape::Nested => "nested",
    };
    println!(
        "shape,api,targets,pending_result,active_children,inactive_children,targets_per_update,frames_per_sample,samples,mean_us,p50_us,p95_us,max_us,min_updates,max_updates,live_entities,retained_pages,factory_builds_during_samples"
    );
    for &content in sizes {
        let (mut fixture, active, inactive) = fixture(content, shape);
        assert_behavior(&mut fixture, active, inactive, content);
        let live = fixture.app.world().entities().count_spawned();
        let builds = fixture.factory_count();
        let mut visits = 0;
        for targets in [Targets::SameRoot, Targets::PerTarget] {
            for api in [Api::Empty, Api::Activity, Api::Inputs, Api::Scroll] {
                {
                    let mut config = fixture.app.world_mut().resource_mut::<Workload>();
                    config.api = api;
                    config.targets = targets;
                }
                measure(&mut fixture.app, 1, warmup);
                let values = measure(&mut fixture.app, samples, batch);
                visits += warmup + samples * batch;
                assert_consumer(&mut fixture, api, targets, visits);
                assert_behavior(&mut fixture, active, inactive, content);
                assert_eq!(fixture.app.world().entities().count_spawned(), live);
                assert_eq!(fixture.factory_count(), builds);
                println!(
                    "{shape_name},{},{},none,{content},{content},{},{batch},{},{live},1,0",
                    api.name(),
                    targets.name(),
                    content * 2,
                    timing_columns(&values),
                );
            }
        }
        fixture.release_parent_scope();
    }
}
