//! Opt-in window CPU sampling; ordinary example runs install no extra systems.
//!
//! `ROUTER_TRANSITION_PERF=/tmp/transitions.csv cargo run --release
//! --manifest-path examples/windowed/Cargo.toml --bin transitions`.
//! Main update intervals and render submission timings are not display FPS or
//! GPU timings. All file I/O happens after the finite measurement sequence.

use std::{
    fs::File,
    io::{BufWriter, Write},
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use bevy_internal::{
    prelude::*,
    render::{Extract, ExtractSchedule, Render, RenderApp, RenderSystems, renderer::render_system},
};
use bevy_ui_router::{ecs::RouterScheduleSet, prelude::*};

use super::{
    Demo,
    animation::{AnimationOptions, AnimationState, Effect},
    initial_navigation, rapid_navigation, request,
};

const MODES: [&str; 5] = [
    "idle",
    "slide_enter",
    "slide_hold",
    "scale_enter",
    "scale_hold",
];
const WARMUP_SAMPLES: usize = 60;
const DEFAULT_SAMPLES: usize = 120;

#[derive(Clone, Copy)]
struct Sample {
    mode: usize,
    render: bool,
    elapsed: Duration,
    interval: Duration,
}

#[derive(Resource, Clone, Default)]
struct Shared(Arc<Mutex<Vec<Sample>>>);

/// Extracted with this main frame, so a newer pipelined update cannot relabel
/// an older render job through a shared atomic stage flag.
#[derive(Resource, Default)]
struct SampleTag(u8);

#[derive(Resource)]
struct Measurement {
    path: PathBuf,
    started: Instant,
    mode: usize,
    warming: bool,
    serial: usize,
    qualified: usize,
    samples: usize,
    initialized: bool,
    initial_requested: bool,
    pending: Option<NavigationRequestId>,
    next_notes: bool,
    finished: bool,
    error: Option<String>,
}

impl Measurement {
    fn qualifies(&self, animations: &AnimationState) -> bool {
        self.initialized
            && !self.finished
            && self.qualified
                < if self.warming {
                    WARMUP_SAMPLES
                } else {
                    self.samples
                }
            && (self.mode == 0 || animations.is_running())
    }

    fn next_phase(&mut self) {
        self.serial += 1;
        self.qualified = 0;
        if self.warming {
            self.warming = false;
        } else {
            self.mode += 1;
            self.warming = true;
            self.finished = self.mode == MODES.len();
        }
    }
}

#[derive(Resource)]
struct MainClock {
    started: Instant,
    previous: Option<Instant>,
    interval: Duration,
    serial: usize,
    qualified: bool,
}

#[derive(Resource)]
struct RenderClock {
    started: Instant,
    phase: u8,
}

pub(super) fn configure(app: &mut App) {
    let Some(path) = std::env::var_os("ROUTER_TRANSITION_PERF") else {
        return;
    };
    assert!(
        !path.is_empty(),
        "ROUTER_TRANSITION_PERF must name an output CSV"
    );
    let samples = std::env::var("ROUTER_TRANSITION_PERF_SAMPLES")
        .map(|value| value.parse::<usize>().expect("positive sample count"))
        .unwrap_or(DEFAULT_SAMPLES);
    assert!(samples > 0, "positive sample count");
    let now = Instant::now();
    let shared = Shared(Arc::new(Mutex::new(Vec::with_capacity(
        samples * MODES.len() * 3,
    ))));
    app.world_mut().resource_mut::<Demo>().initial_requested = true;
    app.insert_resource(Measurement {
        path: path.into(),
        started: now,
        mode: 0,
        warming: true,
        serial: 0,
        qualified: 0,
        samples,
        initialized: false,
        initial_requested: false,
        pending: None,
        next_notes: false,
        finished: false,
        error: None,
    })
    .insert_resource(MainClock {
        started: now,
        previous: None,
        interval: Duration::ZERO,
        serial: 0,
        qualified: false,
    })
    .insert_resource(shared.clone())
    .init_resource::<SampleTag>()
    .add_systems(First, begin_main)
    .add_systems(
        Update,
        drive
            .after(RouterScheduleSet::PublishSnapshots)
            .before(initial_navigation)
            .before(rapid_navigation),
    )
    .add_systems(Last, finish_main);
    if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
        render_app
            .insert_resource(shared)
            .insert_resource(RenderClock {
                started: now,
                phase: 0,
            })
            .init_resource::<SampleTag>()
            .add_systems(ExtractSchedule, extract_sample_tag)
            .add_systems(
                Render,
                begin_render
                    .in_set(RenderSystems::Render)
                    .before(render_system),
            )
            .add_systems(
                Render,
                finish_render
                    .in_set(RenderSystems::Render)
                    .after(render_system),
            );
    }
}

fn begin_main(
    measurement: Res<Measurement>,
    animations: Res<AnimationState>,
    mut clock: ResMut<MainClock>,
) {
    let now = Instant::now();
    clock.interval = clock.previous.map_or(Duration::ZERO, |prior| now - prior);
    clock.previous = Some(now);
    clock.started = now;
    clock.serial = measurement.serial;
    clock.qualified = measurement.qualifies(&animations);
}

fn drive(
    mut measurement: ResMut<Measurement>,
    mut demo: ResMut<Demo>,
    mut options: ResMut<AnimationOptions>,
    animations: Res<AnimationState>,
    snapshots: Res<RouterSnapshots>,
    mut navigation: ResMut<RouterNavigation>,
) {
    if measurement.finished {
        return;
    }
    if measurement.started.elapsed() > Duration::from_secs(90) {
        measurement.error = Some("window measurement exceeded 90 seconds".into());
        measurement.finished = true;
        return;
    }
    if let Some(id) = measurement.pending {
        match snapshots.request(demo.router, id) {
            RequestLookup::Found(result) => match result.status() {
                RequestStatus::Succeeded if !animations.is_running() => {
                    measurement.pending = None;
                    measurement.initialized = true;
                }
                RequestStatus::Failed(error) => {
                    measurement.error = Some(format!("navigation failed: {error:?}"));
                    measurement.finished = true;
                    return;
                }
                _ => {}
            },
            _ => {}
        }
    }
    if !measurement.initial_requested {
        if snapshots
            .get(demo.router)
            .is_some_and(|snapshot| snapshot.availability() == &RouterAvailability::Ready)
        {
            options.hold_exit = false;
            request("/notes", &mut demo, &mut navigation);
            measurement.pending = demo.last_request;
            measurement.initial_requested = true;
        }
        return;
    }
    if !measurement.initialized {
        return;
    }
    let needed = if measurement.warming {
        WARMUP_SAMPLES
    } else {
        measurement.samples
    };
    if measurement.qualified >= needed
        && (measurement.warming || measurement.pending.is_none() && !animations.is_running())
    {
        measurement.next_phase();
        if measurement.finished {
            return;
        }
    }
    if measurement.mode > 0 && measurement.pending.is_none() && !animations.is_running() {
        options.effect = if measurement.mode <= 2 {
            Effect::Slide
        } else {
            Effect::ScaleRotate
        };
        options.hold_exit = measurement.mode == 2 || measurement.mode == 4;
        let path = if measurement.next_notes {
            "/notes"
        } else {
            "/gallery"
        };
        request(path, &mut demo, &mut navigation);
        measurement.pending = demo.last_request;
        measurement.next_notes = !measurement.next_notes;
    }
}

fn finish_main(
    mut measurement: ResMut<Measurement>,
    animations: Res<AnimationState>,
    clock: Res<MainClock>,
    shared: Res<Shared>,
    mut tag: ResMut<SampleTag>,
    mut exit: MessageWriter<AppExit>,
) {
    tag.0 = 0;
    if measurement.finished {
        let values = shared.0.lock().unwrap();
        match save(&measurement, &values) {
            Ok(()) if measurement.error.is_none() => {
                println!(
                    "transition performance samples: {}",
                    measurement.path.display()
                );
                exit.write(AppExit::Success);
            }
            result => {
                eprintln!(
                    "transition performance failed: {:?}; {result:?}",
                    measurement.error
                );
                exit.write(AppExit::error());
            }
        }
        return;
    }
    if clock.serial == measurement.serial && clock.qualified && measurement.qualifies(&animations) {
        measurement.qualified += 1;
        if !measurement.warming {
            tag.0 = measurement.mode as u8 + 1;
            shared.0.lock().unwrap().push(Sample {
                mode: measurement.mode,
                render: false,
                elapsed: clock.started.elapsed(),
                interval: clock.interval,
            });
        }
    }
}

fn extract_sample_tag(source: Extract<Res<SampleTag>>, mut tag: ResMut<SampleTag>) {
    tag.0 = (**source).0;
}

fn begin_render(tag: Res<SampleTag>, mut clock: ResMut<RenderClock>) {
    clock.started = Instant::now();
    clock.phase = tag.0;
}

fn finish_render(shared: Res<Shared>, clock: Res<RenderClock>) {
    let phase = clock.phase;
    if phase > 0 {
        shared.0.lock().unwrap().push(Sample {
            mode: phase as usize - 1,
            render: true,
            elapsed: clock.started.elapsed(),
            interval: Duration::ZERO,
        });
    }
}

fn save(measurement: &Measurement, values: &[Sample]) -> std::io::Result<()> {
    let mut file = BufWriter::new(File::create(&measurement.path)?);
    writeln!(
        file,
        "# CPU-side wall-clock; update intervals are not presentation intervals"
    )?;
    writeln!(
        file,
        "# render_submit includes render_system, submission, and present scheduling/waits; excludes extraction/queue/prepare"
    )?;
    writeln!(
        file,
        "# warmup_qualified_updates={WARMUP_SAMPLES}; target_samples={}; completed={}",
        measurement.samples,
        measurement.error.is_none()
    )?;
    writeln!(file, "mode,metric,sample,elapsed_ns,update_interval_ns")?;
    for (index, sample) in values.iter().enumerate() {
        writeln!(
            file,
            "{},{},{index},{},{}",
            MODES[sample.mode],
            if sample.render {
                "render_submit"
            } else {
                "main_world"
            },
            sample.elapsed.as_nanos(),
            sample.interval.as_nanos(),
        )?;
    }
    file.flush()
}
