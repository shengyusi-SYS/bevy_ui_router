use super::*;
use bevy_ecs::{
    error::{BevyError, ErrorContext},
    schedule::{
        FixedBitSet, MultiThreadedExecutor, ScheduleCleanupPolicy, Schedules,
        SingleThreadedExecutor, SystemExecutor, SystemSchedule,
    },
    system::{IntoSystem, SystemState},
};
use bevy_input_focus::process_recorded_focus_changes;
use bevy_internal::{
    app::{AppExit, Main},
    winit::WinitSettings,
};
use std::{
    any::TypeId,
    time::{Duration, Instant},
};

#[derive(Resource, Default)]
struct Samples {
    frame: usize,
    post: Option<Duration>,
    rows: Vec<(Duration, Duration)>,
    shape: Option<Shape>,
    original_post: Vec<TypeId>,
    prepare_type: Option<TypeId>,
}

#[derive(Clone, Debug, PartialEq)]
struct Shape {
    location: String,
    frames: usize,
    nodes: usize,
    laid_out: usize,
    entities: usize,
    markers: Vec<(u64, String)>,
    positions: Vec<(u64, Vec2)>,
}

fn shape(world: &mut World) -> Shape {
    let router = world.resource::<Demo>().router;
    let snapshot = world.resource::<RouterSnapshots>().get(router).unwrap();
    assert_eq!(snapshot.availability(), &RouterAvailability::Ready);
    assert!(snapshot.running_request().is_none());
    assert_eq!(snapshot.history().len(), 1);
    let location = snapshot.active().location().unwrap().path().to_owned();
    let frames = snapshot.active().frames().len();
    let nodes = world.query::<&Node>().iter(world).count();
    let laid_out = world
        .query::<&ComputedNode>()
        .iter(world)
        .filter(|node| node.size().x > 0.0 && node.size().y > 0.0)
        .count();
    assert!(laid_out > 0);
    let mut markers = world
        .query::<(Entity, &RouteScroll)>()
        .iter(world)
        .map(|(entity, marker)| (entity.to_bits(), marker.key().to_owned()))
        .collect::<Vec<_>>();
    markers.sort_by_key(|(entity, _)| *entity);
    assert_eq!(markers.len(), 1);
    let mut state = SystemState::<RouteScrollRestorations>::new(world);
    let restorations = state.get(world).unwrap();
    for (entity, key) in &markers {
        assert!(
            restorations
                .pending(Entity::from_bits(*entity), key)
                .is_none(),
            "diagnostic requires no pending restoration"
        );
    }
    let mut positions = world
        .query::<(Entity, &ScrollPosition)>()
        .iter(world)
        .map(|(entity, position)| (entity.to_bits(), position.0))
        .collect::<Vec<_>>();
    positions.sort_by_key(|(entity, _)| *entity);
    Shape {
        location,
        frames,
        nodes,
        laid_out,
        entities: world.entities().count_spawned() as usize,
        markers,
        positions,
    }
}

struct TimedPost(MultiThreadedExecutor);
impl SystemExecutor for TimedPost {
    fn init(&mut self, schedule: &SystemSchedule) {
        self.0.init(schedule);
    }
    fn run(
        &mut self,
        schedule: &mut SystemSchedule,
        world: &mut World,
        skip: Option<&FixedBitSet>,
        handler: fn(BevyError, ErrorContext),
    ) {
        let start = Instant::now();
        self.0.run(schedule, world, skip, handler);
        let elapsed = start.elapsed();
        world.resource_mut::<Samples>().post = Some(elapsed);
    }
    fn set_apply_final_deferred(&mut self, value: bool) {
        self.0.set_apply_final_deferred(value);
    }
}

struct TimedMain {
    inner: SingleThreadedExecutor,
    variant: String,
    warmup: usize,
    count: usize,
}
impl SystemExecutor for TimedMain {
    fn init(&mut self, schedule: &SystemSchedule) {
        self.inner.init(schedule);
    }
    fn run(
        &mut self,
        schedule: &mut SystemSchedule,
        world: &mut World,
        skip: Option<&FixedBitSet>,
        handler: fn(BevyError, ErrorContext),
    ) {
        let start = Instant::now();
        self.inner.run(schedule, world, skip, handler);
        let main = start.elapsed();
        let frame = world.resource::<Samples>().frame;
        if frame == self.warmup {
            let observed = shape(world);
            world.resource_mut::<Samples>().shape = Some(observed);
        }
        let mut samples = world.resource_mut::<Samples>();
        samples.frame += 1;
        if frame >= self.warmup {
            let post = samples.post.unwrap();
            samples.rows.push((post, main));
        }
        if samples.rows.len() != self.count {
            return;
        }
        let expected = samples.shape.clone().unwrap();
        let rows = samples.rows.clone();
        drop(samples);
        assert_eq!(expected, shape(world), "UI shape changed during sampling");
        let stock = IntoSystem::into_system(process_recorded_focus_changes).system_type();
        let schedules = world.resource::<Schedules>();
        let current = schedules
            .get(PostUpdate)
            .unwrap()
            .systems()
            .unwrap()
            .map(|(_, system)| (system.system_type(), system.is_exclusive()))
            .collect::<Vec<_>>();
        let samples = world.resource::<Samples>();
        let original = &samples.original_post;
        assert_eq!(original.len(), current.len());
        let removed = original
            .iter()
            .filter(|kind| !current.iter().any(|(now, _)| now == *kind))
            .copied()
            .collect::<Vec<_>>();
        let added = current
            .iter()
            .filter(|(kind, _)| !original.contains(kind))
            .collect::<Vec<_>>();
        assert_eq!(removed, vec![stock]);
        assert_eq!(added.len(), 1);
        assert!(!added[0].1);
        let present = current
            .iter()
            .filter(|(kind, _)| Some(*kind) == samples.prepare_type)
            .count();
        assert_eq!(present, usize::from(self.variant == "full"));
        println!(
            "SCROLL_PROBE_META,{},warmup={},samples={},post_systems={},prepare_present={},stock_replaced=1,shape={:?}",
            self.variant,
            self.warmup,
            self.count,
            current.len(),
            present,
            expected
        );
        println!("SCROLL_PROBE_HEADER,variant,sample,post_executor_us,main_executor_us");
        for (index, (post, main)) in rows.iter().enumerate() {
            println!(
                "SCROLL_PROBE,{},{},{:.3},{:.3}",
                self.variant,
                index,
                post.as_secs_f64() * 1e6,
                main.as_secs_f64() * 1e6
            );
        }
        world.write_message(AppExit::Success);
    }
    fn set_apply_final_deferred(&mut self, value: bool) {
        self.inner.set_apply_final_deferred(value);
    }
}

pub(super) fn install(app: &mut App) {
    let variant =
        std::env::var("ROUTER_SCROLL_PROBE").expect("choose full or without_empty_prepare");
    assert!(matches!(variant.as_str(), "full" | "without_empty_prepare"));
    let (original_post, prepare_type) =
        app.world_mut()
            .schedule_scope(PostUpdate, |world, schedule| {
                schedule.initialize(world).unwrap();
                let keys = schedule
                    .graph()
                    .systems_in_set(RouterScrollSet::Prepare.intern())
                    .unwrap();
                assert_eq!(keys.len(), 1);
                let prepare_type = schedule
                    .systems()
                    .unwrap()
                    .find_map(|(key, system)| keys.contains(&key).then_some(system.system_type()))
                    .unwrap();
                if variant == "without_empty_prepare" {
                    assert_eq!(
                        schedule
                            .remove_systems_in_set(
                                RouterScrollSet::Prepare,
                                world,
                                ScheduleCleanupPolicy::RemoveSystemsOnly
                            )
                            .unwrap(),
                        1
                    );
                    schedule.initialize(world).unwrap();
                    assert!(
                        schedule
                            .graph()
                            .systems_in_set(RouterScrollSet::Prepare.intern())
                            .unwrap()
                            .is_empty()
                    );
                }
                let original = schedule
                    .systems()
                    .unwrap()
                    .map(|(_, system)| system.system_type())
                    .collect::<Vec<_>>();
                (original, prepare_type)
            });
    let stock = IntoSystem::into_system(process_recorded_focus_changes).system_type();
    assert_eq!(
        original_post.iter().filter(|kind| **kind == stock).count(),
        1
    );
    app.insert_resource(WinitSettings::continuous())
        .insert_resource(Samples {
            original_post,
            prepare_type: Some(prepare_type),
            ..Default::default()
        })
        .edit_schedule(PostUpdate, |schedule| {
            schedule.set_executor(TimedPost(MultiThreadedExecutor::new()));
        })
        .edit_schedule(Main, |schedule| {
            schedule.set_executor(TimedMain {
                inner: SingleThreadedExecutor::new(),
                variant: variant.clone(),
                warmup: 128,
                count: 512,
            });
        });
}
