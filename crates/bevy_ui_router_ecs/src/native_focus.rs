//! A fixed-version InputFocus queue adapter. Observations never confer lifecycle
//! authority: only the original native frame owner may consume a pending intent.

use crate::{
    components::RouteOwnershipSeal,
    input::pointer_resource_entity,
    presentation::{
        FrameContentTarget, native_failure::NativeFailureSource, native_scene::NativeContentRoot,
    },
    registry::{
        EntityLifecycle, EntityOwner, ExactRegistryClaim, RegistryOwner, RouteEntityRegistry,
    },
};
use bevy_app::{App, MainScheduleOrder, PostUpdate};
use bevy_ecs::{
    prelude::*,
    schedule::{ScheduleCleanupPolicy, ScheduleLabel},
    system::{IntoSystem, System},
};
use bevy_input_focus::{FocusCause, InputFocus, InputFocusSystems, process_recorded_focus_changes};
use bevy_reflect::structs::Struct;

type Change = Option<(Entity, FocusCause)>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NativeFocusError {
    MissingResource,
    Schema,
    ResourceSource,
    OriginalSource,
    QueueSource,
    CurrentSource,
    SequenceExhausted,
    DrainSource,
    ScheduleSource,
}

struct Fields<'a> {
    changes: &'a Vec<Change>,
    original: Option<Entity>,
    current: Option<Entity>,
}

fn fields(focus: &InputFocus) -> Result<Fields<'_>, NativeFocusError> {
    let changes = focus
        .field("recorded_changes")
        .and_then(|field| field.try_downcast_ref::<Vec<Change>>())
        .ok_or(NativeFocusError::Schema)?;
    let original = *focus
        .field("original_focus")
        .and_then(|field| field.try_downcast_ref::<Option<Entity>>())
        .ok_or(NativeFocusError::Schema)?;
    let current = *focus
        .field("current_focus")
        .and_then(|field| field.try_downcast_ref::<Option<Entity>>())
        .ok_or(NativeFocusError::Schema)?;
    Ok(Fields {
        changes,
        original,
        current,
    })
}

#[derive(Component, Clone, Debug, Eq, PartialEq)]
pub(crate) struct NativeFocusSource(pub(crate) NativeFailureSource);

/// Created from the real FrameTask, moved with NativeFrameContent, consumed once
/// by successful publication or dropped with the original failed/terminal owner.
#[derive(Debug)]
pub(crate) struct NativeFocusOwner {
    pub(crate) source: NativeFailureSource,
    after: u64,
}

#[derive(Clone)]
struct PendingObservation {
    source: NativeFailureSource,
    root: ExactRegistryClaim,
    target: Entity,
    cause: FocusCause,
    order: u64,
}

#[derive(Resource, Default, Clone)]
struct NativeFocusCursor {
    active: bool,
    baseline: Option<(Entity, InputFocus)>,
    order: u64,
    latest: Option<PendingObservation>,
    error: Option<NativeFocusError>,
}

enum Target {
    Allowed,
    Denied,
    Pending(NativeFailureSource, ExactRegistryClaim),
}

fn classify(world: &World, target: Entity, denied: &[Entity]) -> Target {
    if denied.contains(&target) || world.get_entity(target).is_err() {
        return Target::Denied;
    }
    let Some(registry) = world.get_resource::<RouteEntityRegistry>() else {
        return Target::Allowed;
    };
    let mut current = target;
    let mut seen = std::collections::HashSet::new();
    loop {
        if !seen.insert(current) {
            return Target::Denied;
        }
        if let Some(record) = registry.record(current) {
            if world.get::<RouteOwnershipSeal>(current)
                != Some(&RouteOwnershipSeal::new(record.seal()))
            {
                return Target::Denied;
            }
            if record.lifecycle() == EntityLifecycle::Active {
                if crate::presentation::native_frame::is_native_content(world, current)
                    && !crate::presentation::native_frame::authored_target_is_visible(
                        world,
                        target,
                        &[],
                    )
                {
                    return Target::Denied;
                }
                return Target::Allowed;
            }
            if record.lifecycle() == EntityLifecycle::PreparedInert
                && let RegistryOwner::Committed(EntityOwner::FrameContentTarget(frame)) =
                    record.owner()
                && world.get::<NativeContentRoot>(current).is_some()
                && world
                    .get::<FrameContentTarget>(current)
                    .is_some_and(|target| target.matches_frame(frame))
                && let Some(source) = world.get::<NativeFocusSource>(current)
                && source.0.frame == *frame
            {
                return Target::Pending(source.0.clone(), record.claim());
            }
            return Target::Denied;
        }
        if crate::input::inert_table_root_is_exact(world, current) {
            return Target::Denied;
        }
        let Some(parent) = world.get::<ChildOf>(current).map(ChildOf::parent) else {
            return Target::Allowed;
        };
        if world
            .get::<Children>(parent)
            .is_none_or(|children| !children.contains(&current))
        {
            return Target::Denied;
        }
        current = parent;
    }
}

struct Observation {
    cursor: NativeFocusCursor,
    next: InputFocus,
}

fn prepare_observation(world: &World, denied: &[Entity]) -> Result<Observation, NativeFocusError> {
    let mut cursor = world.resource::<NativeFocusCursor>().clone();
    if let Some(error) = cursor.error {
        return Err(error);
    }
    let entity =
        pointer_resource_entity::<InputFocus>(world).ok_or(NativeFocusError::MissingResource)?;
    let focus = world
        .get_resource::<InputFocus>()
        .ok_or(NativeFocusError::MissingResource)?;
    let new = fields(focus)?;
    let Some((source, before)) = &cursor.baseline else {
        cursor.baseline = Some((entity, focus.clone()));
        return Ok(Observation {
            cursor,
            next: focus.clone(),
        });
    };
    if *source != entity {
        return Err(NativeFocusError::ResourceSource);
    }
    let old = fields(before)?;
    if new.original != old.original {
        return Err(NativeFocusError::OriginalSource);
    }
    if !new.changes.starts_with(old.changes) {
        return Err(NativeFocusError::QueueSource);
    }
    let suffix = &new.changes[old.changes.len()..];
    let expected = suffix
        .last()
        .map(|change| change.map(|(entity, _)| entity))
        .unwrap_or(old.current);
    if new.current != expected || new.current != focus.get() {
        return Err(NativeFocusError::CurrentSource);
    }
    let mut next = before.clone();
    for change in suffix {
        cursor.order = cursor
            .order
            .checked_add(1)
            .ok_or(NativeFocusError::SequenceExhausted)?;
        cursor.latest = None;
        match *change {
            Some((target, cause)) => match classify(world, target, denied) {
                Target::Allowed => next.set(target, cause),
                Target::Denied => {}
                Target::Pending(source, root) => {
                    cursor.latest = Some(PendingObservation {
                        source,
                        root,
                        target,
                        cause,
                        order: cursor.order,
                    })
                }
            },
            None => next.clear(),
        }
    }
    cursor.baseline = Some((entity, next.clone()));
    Ok(Observation { cursor, next })
}

pub(crate) fn observe(world: &mut World, denied: &[Entity]) -> Result<(), NativeFocusError> {
    if !world
        .get_resource::<NativeFocusCursor>()
        .is_some_and(|cursor| cursor.active)
    {
        return Ok(());
    }
    match prepare_observation(world, denied) {
        Ok(observation) => {
            if world.resource::<InputFocus>() != &observation.next {
                *world.resource_mut::<InputFocus>() = observation.next;
            }
            *world.resource_mut::<NativeFocusCursor>() = observation.cursor;
            Ok(())
        }
        Err(error) => {
            world.resource_mut::<NativeFocusCursor>().error = Some(error);
            Err(error)
        }
    }
}

pub(crate) fn begin(
    world: &mut World,
    task: &bevy_ui_router_core::integration::RootFrameTask,
) -> Result<NativeFocusOwner, NativeFocusError> {
    world.resource_mut::<NativeFocusCursor>().active = true;
    observe(world, &[])?;
    Ok(NativeFocusOwner {
        source: NativeFailureSource::from_task(task),
        after: world.resource::<NativeFocusCursor>().order,
    })
}

pub(crate) fn source_is_valid(world: &World) -> bool {
    world
        .get_resource::<NativeFocusCursor>()
        .is_none_or(|cursor| cursor.error.is_none())
}

pub(crate) struct PublicationFocus {
    expected: InputFocus,
    pub(crate) candidate: Option<(Entity, FocusCause)>,
}

impl PublicationFocus {
    pub(crate) fn matches(&self, focus: &InputFocus) -> bool {
        self.expected == *focus
    }
}

pub(crate) fn prepare_publication(
    world: &mut World,
    frames: &[crate::presentation::RealizedFrame],
    eligible: &[Entity],
) -> Option<PublicationFocus> {
    prepare_publication_frames(world, frames.iter(), eligible)
}

pub(crate) fn prepare_publication_frames<'a>(
    world: &mut World,
    mut frames: impl Iterator<Item = &'a crate::presentation::RealizedFrame>,
    eligible: &[Entity],
) -> Option<PublicationFocus> {
    observe(world, &[]).ok()?;
    let cursor = world.get_resource::<NativeFocusCursor>();
    let candidate = cursor
        .and_then(|cursor| cursor.latest.as_ref())
        .and_then(|latest| {
            frames.find_map(|frame| {
                let owner = frame.native_focus_owner()?;
                if owner.source != latest.source
                    || latest.order <= owner.after
                    || latest.source.frame != *frame.frame()
                {
                    return None;
                }
                let root = frame.native_root()?;
                if latest.root.entity() != root
                    || world
                        .resource::<RouteEntityRegistry>()
                        .record(root)?
                        .claim()
                        != latest.root
                    || world.get::<NativeFocusSource>(root)?.0 != owner.source
                    || !eligible.contains(&latest.target)
                    || !crate::presentation::native_frame::collect_content(
                        world,
                        frame.frame(),
                        root,
                    )
                    .is_ok_and(|content| content.contains(&latest.target))
                {
                    return None;
                }
                Some((latest.target, latest.cause))
            })
        });
    Some(PublicationFocus {
        expected: world.get_resource::<InputFocus>()?.clone(),
        candidate,
    })
}

// Normal parameters are important: no exclusive system's implicit World::flush
// may split the pipe or deliver the official processor's Commands early.
fn before_drain(mut params: ParamSet<(&World, ResMut<NativeFocusCursor>, ResMut<InputFocus>)>) {
    if !params.p1().active {
        return;
    }
    let result = prepare_observation(params.p0(), &[]);
    match result {
        Ok(observation) => {
            if *params.p2() != observation.next {
                *params.p2() = observation.next;
            }
            *params.p1() = observation.cursor;
        }
        Err(error) => params.p1().error = Some(error),
    }
}

fn after_drain(mut params: ParamSet<(&World, ResMut<NativeFocusCursor>)>) {
    if !params.p1().active || params.p1().error.is_some() {
        return;
    }
    let result = (|| {
        let world = params.p0();
        let cursor = world.resource::<NativeFocusCursor>();
        let (source, before) = cursor
            .baseline
            .as_ref()
            .ok_or(NativeFocusError::DrainSource)?;
        if pointer_resource_entity::<InputFocus>(world) != Some(*source) {
            return Err(NativeFocusError::ResourceSource);
        }
        let focus = world
            .get_resource::<InputFocus>()
            .ok_or(NativeFocusError::MissingResource)?;
        let now = fields(focus)?;
        if !now.changes.is_empty() || now.current != before.get() || now.original != now.current {
            return Err(NativeFocusError::DrainSource);
        }
        Ok((*source, focus.clone()))
    })();
    match result {
        Ok(baseline) => params.p1().baseline = Some(baseline),
        Err(error) => params.p1().error = Some(error),
    }
}

#[derive(ScheduleLabel, Clone, Debug, Eq, PartialEq, Hash)]
struct NativeFocusDrainBoundary;

fn install_drain(world: &mut World) {
    if !world.resource::<NativeFocusCursor>().active {
        return;
    }
    if world
        .get_resource::<bevy_ecs::schedule::Schedules>()
        .and_then(|schedules| schedules.get(PostUpdate))
        .is_none()
    {
        let _ = observe(world, &[]);
        return;
    }
    let result = world.try_schedule_scope(PostUpdate, |world, schedule| {
        schedule
            .initialize(world)
            .map_err(|_| NativeFocusError::ScheduleSource)?;
        let stock = IntoSystem::into_system(process_recorded_focus_changes).system_type();
        let bridge = IntoSystem::into_system(
            before_drain
                .pipe(process_recorded_focus_changes)
                .pipe(after_drain),
        )
        .system_type();
        let types = schedule
            .systems()
            .map_err(|_| NativeFocusError::ScheduleSource)?
            .map(|(_, system)| system.system_type())
            .collect::<Vec<_>>();
        let stock_count = types.iter().filter(|kind| **kind == stock).count();
        let bridge_count = types.iter().filter(|kind| **kind == bridge).count();
        if stock_count + bridge_count > 1 {
            return Err(NativeFocusError::ScheduleSource);
        }
        if stock_count == 1 {
            if schedule
                .remove_systems_in_set(
                    process_recorded_focus_changes,
                    world,
                    ScheduleCleanupPolicy::RemoveSystemsOnly,
                )
                .map_err(|_| NativeFocusError::ScheduleSource)?
                != 1
            {
                return Err(NativeFocusError::ScheduleSource);
            }
            schedule.add_systems(
                before_drain
                    .pipe(process_recorded_focus_changes)
                    .pipe(after_drain)
                    .in_set(InputFocusSystems::FocusChangeEvents),
            );
        }
        Ok(())
    });
    if result.is_err() || result.is_ok_and(|result| result.is_err()) {
        world.resource_mut::<NativeFocusCursor>().error = Some(NativeFocusError::ScheduleSource);
    }
    // Initialization/ScheduleBuilt callbacks append to the same old baseline.
    // Never reset it from an after-install snapshot and lose superseding writes.
    let _ = observe(world, &[]);
}

pub(crate) fn install(app: &mut App) {
    if app.world().contains_resource::<NativeFocusCursor>() {
        return;
    }
    app.init_resource::<NativeFocusCursor>();
    app.world_mut()
        .resource_mut::<MainScheduleOrder>()
        .insert_before(PostUpdate, NativeFocusDrainBoundary);
    app.add_systems(NativeFocusDrainBoundary, install_drain);
}
