use super::*;
use bevy_ecs::{
    error::{BevyError, ErrorContext},
    schedule::{
        FixedBitSet, MultiThreadedExecutor, ScheduleLabel, Schedules, SingleThreadedExecutor,
        SystemExecutor, SystemSchedule,
    },
    system::IntoSystem,
};
use bevy_input_focus::process_recorded_focus_changes;
use bevy_internal::{
    app::{AppExit, Main, MainScheduleOrder},
    winit::WinitSettings,
};
use std::{
    any::TypeId,
    time::{Duration, Instant},
};

#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
struct BeforeFocus;
#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
struct AfterFocus;

#[derive(Resource, Default)]
struct Samples {
    frame: usize,
    start: Option<Instant>,
    boundary: Option<Duration>,
    rows: Vec<(Duration, Duration)>,
    shape: Option<Shape>,
    original_post: Vec<TypeId>,
}

fn before(mut samples: ResMut<Samples>) {
    samples.start = Some(Instant::now());
}

fn after(mut samples: ResMut<Samples>) {
    samples.boundary = Some(samples.start.take().unwrap().elapsed());
}

struct TimedMain {
    inner: SingleThreadedExecutor,
    variant: String,
    warmup: usize,
    samples: usize,
}

#[derive(Clone, Debug, PartialEq)]
struct Shape {
    location: String,
    frames: usize,
    nodes: usize,
    laid_out: usize,
    entities: usize,
    positions: Vec<(u64, Vec2)>,
}

fn shape(world: &mut World) -> Shape {
    let router = world.resource::<Demo>().router;
    let snapshot = world.resource::<RouterSnapshots>().get(router).unwrap();
    assert_eq!(snapshot.availability(), &RouterAvailability::Ready);
    assert!(snapshot.running_request().is_none());
    let location = snapshot.active().location().unwrap().path().to_owned();
    let frames = snapshot.active().frames().len();
    let nodes = world.query::<&Node>().iter(world).count();
    let laid_out = world
        .query::<&ComputedNode>()
        .iter(world)
        .filter(|node| node.size().x > 0.0 && node.size().y > 0.0)
        .count();
    assert!(laid_out > 0, "real UI layout must have positive dimensions");
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
        positions,
    }
}

impl SystemExecutor for TimedMain {
    fn init(&mut self, schedule: &SystemSchedule) {
        self.inner.init(schedule);
    }

    fn run(
        &mut self,
        schedule: &mut SystemSchedule,
        world: &mut World,
        skip_systems: Option<&FixedBitSet>,
        error_handler: fn(BevyError, ErrorContext),
    ) {
        let start = Instant::now();
        self.inner.run(schedule, world, skip_systems, error_handler);
        let main = start.elapsed();
        let frame = world.resource::<Samples>().frame;
        if frame == self.warmup {
            let observed = shape(world);
            world.resource_mut::<Samples>().shape = Some(observed);
        }
        let mut samples = world.resource_mut::<Samples>();
        samples.frame += 1;
        if frame >= self.warmup {
            let boundary = samples.boundary.unwrap();
            samples.rows.push((boundary, main));
        }
        if samples.rows.len() != self.samples {
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
        let original = &world.resource::<Samples>().original_post;
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
        assert_eq!(
            removed,
            vec![stock],
            "only the official drain should be replaced"
        );
        assert_eq!(
            added.len(),
            1,
            "one real focus pipe must replace the official drain"
        );
        assert!(
            !added[0].1,
            "the focus pipe must keep its normal system wrapper"
        );
        println!(
            "FOCUS_PROBE_META,{},warmup={},samples={},stock_replaced=1,shape={:?}",
            self.variant, self.warmup, self.samples, expected
        );
        println!("FOCUS_PROBE_HEADER,variant,sample,boundary_us,main_executor_us");
        for (index, (boundary, main)) in rows.iter().enumerate() {
            println!(
                "FOCUS_PROBE,{},{},{:.3},{:.3}",
                self.variant,
                index,
                boundary.as_secs_f64() * 1e6,
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
    let variant = std::env::var("ROUTER_FOCUS_PROBE_EXECUTOR").expect("choose multi or single");
    assert!(matches!(variant.as_str(), "multi" | "single"));
    let labels = app
        .world()
        .resource::<Schedules>()
        .iter()
        .filter(|(label, _)| format!("{label:?}") == "NativeFocusDrainBoundary")
        .map(|(_, schedule)| schedule.label())
        .collect::<Vec<_>>();
    assert_eq!(labels.len(), 1);
    let label = labels[0];
    app.world_mut().schedule_scope(label, |world, schedule| {
        schedule.initialize(world).unwrap();
        let systems = schedule
            .systems()
            .unwrap()
            .map(|(_, system)| system.is_exclusive())
            .collect::<Vec<_>>();
        assert_eq!(systems.len(), 1);
        assert!(systems[0]);
    });
    let original_post = app
        .world_mut()
        .schedule_scope(PostUpdate, |world, schedule| {
            schedule.initialize(world).unwrap();
            schedule
                .systems()
                .unwrap()
                .map(|(_, system)| system.system_type())
                .collect::<Vec<_>>()
        });
    let stock = IntoSystem::into_system(process_recorded_focus_changes).system_type();
    assert_eq!(
        original_post.iter().filter(|kind| **kind == stock).count(),
        1
    );
    {
        let mut order = app.world_mut().resource_mut::<MainScheduleOrder>();
        let index = order
            .labels
            .iter()
            .position(|current| *current == label)
            .unwrap();
        // The private label is already interned; helpers compare concrete labels.
        order.labels.insert(index, BeforeFocus.intern());
        order.labels.insert(index + 2, AfterFocus.intern());
        assert_eq!(order.labels[index + 1], label);
    }
    app.insert_resource(WinitSettings::continuous())
        .insert_resource(Samples {
            original_post,
            ..Default::default()
        })
        .add_systems(BeforeFocus, before)
        .add_systems(AfterFocus, after)
        .edit_schedule(BeforeFocus, |schedule| {
            schedule.set_executor(SingleThreadedExecutor::new());
        })
        .edit_schedule(AfterFocus, |schedule| {
            schedule.set_executor(SingleThreadedExecutor::new());
        })
        .edit_schedule(label, |schedule| {
            if variant == "multi" {
                schedule.set_executor(MultiThreadedExecutor::new());
            } else {
                schedule.set_executor(SingleThreadedExecutor::new());
            }
        })
        .edit_schedule(Main, |schedule| {
            schedule.set_executor(TimedMain {
                inner: SingleThreadedExecutor::new(),
                variant: variant.clone(),
                warmup: 128,
                samples: 512,
            });
        });
}
