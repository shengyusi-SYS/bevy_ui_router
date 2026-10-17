use std::{
    collections::{HashMap, HashSet},
    fmt,
    hash::{Hash, Hasher},
    sync::Arc,
};

use bevy_camera::Camera;
use bevy_ecs::{
    prelude::*,
    system::{SystemParam, SystemState},
};
use bevy_ui::{
    ComputedNode, ComputedUiRenderTargetInfo, ComputedUiTargetCamera, ContentSize, Node,
    ScrollPosition, UiGlobalTransform, UiTransform,
    experimental::{UiChildren, UiRootNodes},
    ui_surface::UiSurface,
};
use bevy_ui_router_core::{
    fingerprint::RouteSegmentReuseKey,
    frame::RouteFrameVersion,
    integration::{HistoryEntryKey, HistoryFrameFact, HistoryPublicationFacts},
    presentation::{ActivePresentationKind, RouterAvailability},
};

use crate::{
    active_scope::ActiveRouteScope, navigation::RouterNavigation, presentation::RealizedFrame,
};

/// When a bound history scroll restoration may attempt normal UI layout.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RouteScrollRestorePolicy {
    /// Attempt once at the first normal layout after binding.
    #[default]
    Automatic,
    /// Wait for the application's `ready_route_scroll` command for this intent.
    Explicit,
}

/// Opts a UI scroll container into history-entry restoration.
///
/// Keys are local to one route frame. Use distinct, stable keys for multiple
/// containers in the same frame; duplicate keys are excluded from restoration.
#[derive(Component, Clone, Debug, Default, Eq, PartialEq)]
pub struct RouteScroll {
    key: String,
    policy: RouteScrollRestorePolicy,
}

impl RouteScroll {
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            policy: RouteScrollRestorePolicy::Automatic,
        }
    }

    /// The container's stable key within its owning route frame.
    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn with_restore_policy(mut self, policy: RouteScrollRestorePolicy) -> Self {
        self.policy = policy;
        self
    }
}

struct RestoreIdentity;

/// Opaque identity of one restoration, distinct on every Back publication.
///
/// Cloning this ID does not grant frame authority. Stale or consumed IDs have no
/// effect, including when another visit reuses the same entry and Entity.
#[derive(Clone)]
pub struct RouteScrollRestoreId(Arc<RestoreIdentity>);

impl PartialEq for RouteScrollRestoreId {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for RouteScrollRestoreId {}
impl Hash for RouteScrollRestoreId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.0).hash(state);
    }
}
impl fmt::Debug for RouteScrollRestoreId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RouteScrollRestoreId(..)")
    }
}

/// Read-only frozen target of an unfinished history scroll restoration.
#[derive(Clone, Debug)]
pub struct RouteScrollRestoration {
    id: RouteScrollRestoreId,
    position: ScrollPosition,
}
impl RouteScrollRestoration {
    pub fn id(&self) -> RouteScrollRestoreId {
        self.id.clone()
    }
    /// Requested position in two-dimensional logical pixels, before layout clamping.
    pub fn position(&self) -> &ScrollPosition {
        &self.position
    }
}

/// Looks up unfinished restoration using any content in its exact active frame.
///
/// The scroll container need not exist yet. Invalid or inactive boundaries return
/// `None`; the lookup does not borrow application scroll components.
#[derive(SystemParam)]
pub struct RouteScrollRestorations<'w, 's> {
    scope: ActiveRouteScope<'w, 's>,
}
impl RouteScrollRestorations<'_, '_> {
    pub fn pending(&self, scope_entity: Entity, key: &str) -> Option<RouteScrollRestoration> {
        let (entry, frame) = self.scope.resolve(scope_entity)?;
        let current = entry.engine.canonical_history().current_entry()?;
        let intent = entry.scroll.pending.iter().find(|intent| {
            intent.entry == current && intent.frame == *frame.frame() && intent.key == key
        })?;
        Some(RouteScrollRestoration {
            id: intent.id.clone(),
            position: intent.position.clone(),
        })
    }
}

/// Deferred application control of a specific restoration intent.
pub trait RouteScrollCommandsExt {
    /// Allows this intent to attempt layout once, even before its container exists.
    fn ready_route_scroll(&mut self, id: RouteScrollRestoreId) -> &mut Self;
    /// Cancels pending work without rolling back application or already-written values.
    fn cancel_route_scroll(&mut self, id: RouteScrollRestoreId) -> &mut Self;
}
impl RouteScrollCommandsExt for Commands<'_, '_> {
    fn ready_route_scroll(&mut self, id: RouteScrollRestoreId) -> &mut Self {
        self.queue(move |world: &mut World| {
            let Some(mut navigation) = world.get_resource_mut::<RouterNavigation>() else {
                return;
            };
            for entry in &mut navigation.entries {
                if let Some(intent) = entry
                    .scroll
                    .pending
                    .iter_mut()
                    .find(|intent| intent.id == id)
                {
                    intent.ready = true;
                    break;
                }
            }
        });
        self
    }
    fn cancel_route_scroll(&mut self, id: RouteScrollRestoreId) -> &mut Self {
        self.queue(move |world: &mut World| {
            let Some(mut navigation) = world.get_resource_mut::<RouterNavigation>() else {
                return;
            };
            for entry in &mut navigation.entries {
                entry.scroll.pending.retain(|intent| intent.id != id);
            }
        });
        self
    }
}

#[derive(Clone)]
struct Container {
    frame: RouteFrameVersion,
    entity: Entity,
    key: String,
    policy: RouteScrollRestorePolicy,
    position: ScrollPosition,
}

#[derive(Clone)]
struct Restoration {
    id: RouteScrollRestoreId,
    entry: HistoryEntryKey,
    frame: RouteFrameVersion,
    reuse: RouteSegmentReuseKey,
    key: String,
    position: ScrollPosition,
    binding: Option<Container>,
    ready: bool,
    attempted: bool,
}

#[derive(Clone)]
struct PublishedEntry {
    entry: HistoryEntryKey,
    frames: Vec<HistoryFrameFact>,
}

type DuplicateKeys = Vec<(RouteFrameVersion, String)>;

/// Values only. These observations cannot authorize a frame transition.
#[derive(Default)]
pub(crate) struct ScrollObservation {
    source: Vec<Container>,
    target: Vec<Container>,
    source_duplicates: DuplicateKeys,
    target_duplicates: DuplicateKeys,
}

#[derive(Clone, Default)]
pub(crate) struct ScrollState {
    records: HashMap<HistoryEntryKey, HashMap<(RouteSegmentReuseKey, String), ScrollPosition>>,
    // Latest valid displays survive layout gaps only while the exact marked
    // containers remain active; they never supply history or frame authority.
    displayed: Vec<Container>,
    published: Option<PublishedEntry>,
    pending: Vec<Restoration>,
    warned_duplicate: bool,
    warned_layout: bool,
}

pub(crate) struct PreparedScroll {
    pub(crate) state: ScrollState,
}

impl ScrollObservation {
    pub(crate) fn capture<'a>(
        world: &World,
        source: impl Iterator<Item = &'a RealizedFrame>,
        target: impl Iterator<Item = &'a RealizedFrame>,
        eligible: &HashSet<Entity>,
    ) -> Self {
        let (source, source_duplicates) = containers(world, source, eligible);
        let (target, target_duplicates) = containers(world, target, eligible);
        Self {
            source,
            target,
            source_duplicates,
            target_duplicates,
        }
    }
}

pub(crate) fn eligible_containers(world: &mut World) -> HashSet<Entity> {
    world
        .query_filtered::<Entity, (With<RouteScroll>, With<Node>, With<ScrollPosition>)>()
        .iter(world)
        .collect()
}

fn containers<'a>(
    world: &World,
    frames: impl Iterator<Item = &'a RealizedFrame>,
    eligible: &HashSet<Entity>,
) -> (Vec<Container>, DuplicateKeys) {
    let mut result = Vec::new();
    let mut duplicates = Vec::new();
    for frame in frames {
        let managed = frame.managed_content_entities(world);
        let mut keys = HashMap::<&str, usize>::new();
        for entity in &managed {
            if let Some(marker) = world.get::<RouteScroll>(*entity) {
                *keys.entry(&marker.key).or_default() += 1;
            }
        }
        let mut duplicate_keys = keys
            .iter()
            .filter(|(_, count)| **count != 1)
            .map(|(key, _)| (*key).to_owned())
            .collect::<Vec<_>>();
        duplicate_keys.sort();
        duplicates.extend(
            duplicate_keys
                .into_iter()
                .map(|key| (frame.frame().clone(), key)),
        );
        let mut frame_containers = Vec::new();
        for entity in managed {
            let Some(marker) = world.get::<RouteScroll>(entity) else {
                continue;
            };
            if keys.get(marker.key.as_str()) != Some(&1) || !eligible.contains(&entity) {
                continue;
            }
            let Some(position) = world.get::<ScrollPosition>(entity) else {
                continue;
            };
            frame_containers.push(Container {
                frame: frame.frame().clone(),
                entity,
                key: marker.key.clone(),
                policy: marker.policy,
                position: position.clone(),
            });
        }
        frame_containers.sort_by(|a, b| a.key.cmp(&b.key));
        result.extend(frame_containers);
    }
    (result, duplicates)
}

fn reuse_key<'a>(
    facts: &'a [HistoryFrameFact],
    frame: &RouteFrameVersion,
) -> Option<&'a RouteSegmentReuseKey> {
    facts
        .iter()
        .find(|fact| fact.frame() == frame)
        .map(HistoryFrameFact::segment_reuse_key)
}
fn same_container(left: &Container, right: &Container) -> bool {
    left.frame == right.frame && left.entity == right.entity && left.key == right.key
}
fn same_binding(left: &Container, right: &Container) -> bool {
    same_container(left, right) && left.policy == right.policy
}
fn duplicate(duplicates: &DuplicateKeys, frame: &RouteFrameVersion, key: &str) -> bool {
    duplicates
        .iter()
        .any(|(other, other_key)| other == frame && other_key == key)
}

impl ScrollState {
    pub(crate) fn prepare(
        &self,
        facts: &HistoryPublicationFacts,
        observation: ScrollObservation,
    ) -> PreparedScroll {
        let mut state = self.clone();
        state.pending.clear();
        state.warn_duplicate(
            observation
                .source_duplicates
                .iter()
                .chain(&observation.target_duplicates)
                .next()
                .map(|(frame, key)| (frame, key.as_str())),
        );
        if let Some(entry) = facts.source_entry() {
            // Navigation can publish before this update's scroll systems. Do
            // not carry an intent invalidated by an application change into
            // the departed visit's record.
            let unfinished = self
                .pending
                .iter()
                .filter(|intent| {
                    intent.entry == entry
                        && reuse_key(facts.source_frames(), &intent.frame) == Some(&intent.reuse)
                        && !duplicate(&observation.source_duplicates, &intent.frame, &intent.key)
                        && intent.binding.as_ref().is_none_or(|binding| {
                            observation.source.iter().any(|current| {
                                same_binding(current, binding)
                                    && current.position.0 == binding.position.0
                            })
                        })
                })
                .collect::<Vec<_>>();
            let mut positions = observation
                .source
                .iter()
                .filter_map(|container| {
                    if unfinished.iter().any(|intent| {
                        intent.frame == container.frame && intent.key == container.key
                    }) {
                        return None;
                    }
                    let displayed = self
                        .displayed
                        .iter()
                        .find(|displayed| same_container(displayed, container))?;
                    let reuse = reuse_key(facts.source_frames(), &container.frame)?;
                    Some((
                        (reuse.clone(), container.key.clone()),
                        displayed.position.clone(),
                    ))
                })
                .collect::<HashMap<_, _>>();
            // Unfinished intentions preserve their original visit record even
            // when no container has appeared or its initial layout is smaller.
            for intent in unfinished {
                positions.insert(
                    (intent.reuse.clone(), intent.key.clone()),
                    intent.position.clone(),
                );
            }
            if positions.is_empty() {
                state.records.remove(&entry);
            } else {
                state.records.insert(entry, positions);
            }
        }
        state
            .records
            .retain(|entry, _| facts.surviving_entries().contains(entry));
        state.displayed.retain(|displayed| {
            observation
                .target
                .iter()
                .any(|container| same_container(displayed, container))
        });
        if facts.restores_back()
            && let Some(entry) = facts.target_entry()
            && let Some(records) = state.records.get(&entry)
        {
            for fact in facts.target_frames() {
                let mut keys = records
                    .iter()
                    .filter(|((reuse, _), _)| reuse == fact.segment_reuse_key())
                    .map(|((_, key), position)| (key, position))
                    .collect::<Vec<_>>();
                keys.sort_by(|(a, _), (b, _)| a.cmp(b));
                for (key, position) in keys {
                    if duplicate(&observation.target_duplicates, fact.frame(), key) {
                        continue;
                    }
                    state.pending.push(Restoration {
                        id: RouteScrollRestoreId(Arc::new(RestoreIdentity)),
                        entry,
                        frame: fact.frame().clone(),
                        reuse: fact.segment_reuse_key().clone(),
                        key: key.clone(),
                        position: position.clone(),
                        binding: observation
                            .target
                            .iter()
                            .find(|container| {
                                container.frame == *fact.frame() && container.key == *key
                            })
                            .cloned(),
                        ready: false,
                        attempted: false,
                    });
                }
            }
        }
        state.published = facts.target_entry().map(|entry| PublishedEntry {
            entry,
            frames: facts.target_frames().to_vec(),
        });
        PreparedScroll { state }
    }

    fn sample_displayed(
        &mut self,
        current: Option<HistoryEntryKey>,
        mut observed: Vec<Container>,
        laid_out: &HashSet<Entity>,
    ) {
        self.displayed.retain(|displayed| {
            observed
                .iter()
                .any(|container| same_container(displayed, container))
        });
        observed.retain(|container| laid_out.contains(&container.entity));
        if let Some(published) = &self.published
            && current == Some(published.entry)
        {
            let positions = observed
                .iter()
                .filter_map(|container| {
                    if self.pending.iter().any(|intent| {
                        intent.frame == container.frame && intent.key == container.key
                    }) {
                        return None;
                    }
                    let reuse = reuse_key(&published.frames, &container.frame)?;
                    Some((
                        (reuse.clone(), container.key.clone()),
                        container.position.clone(),
                    ))
                })
                .collect::<HashMap<_, _>>();
            if !positions.is_empty() {
                self.records
                    .entry(published.entry)
                    .or_default()
                    .extend(positions);
            }
        }
        for container in observed {
            if let Some(displayed) = self
                .displayed
                .iter_mut()
                .find(|displayed| same_container(displayed, &container))
            {
                *displayed = container;
            } else {
                self.displayed.push(container);
            }
        }
    }

    fn warn_duplicate(&mut self, duplicate: Option<(&RouteFrameVersion, &str)>) {
        if !self.warned_duplicate
            && let Some((frame, key)) = duplicate
        {
            eprintln!(
                "bevy_ui_router: duplicate RouteScroll key within a frame \
                 (router={}, frame={}, generation={}, key={key:?}); that key is excluded",
                frame.router().raw(),
                frame.frame().raw(),
                frame.generation().raw()
            );
            self.warned_duplicate = true;
        }
    }
    fn warn_layout(&mut self, missing: Option<&Container>) {
        if !self.warned_layout
            && let Some(container) = missing
        {
            eprintln!(
                "bevy_ui_router: RouteScroll restoration requires the normal UI layout and camera \
                 (router={}, frame={}, generation={}, key={:?})",
                container.frame.router().raw(),
                container.frame.frame().raw(),
                container.frame.generation().raw(),
                container.key
            );
            self.warned_layout = true;
        }
    }
}

struct ActiveObservation {
    entry: Option<HistoryEntryKey>,
    frames: Vec<RouteFrameVersion>,
    containers: Vec<Container>,
    duplicates: DuplicateKeys,
}
impl ActiveObservation {
    fn accepts(&self, intent: &Restoration) -> bool {
        self.entry == Some(intent.entry)
            && self.frames.contains(&intent.frame)
            && !duplicate(&self.duplicates, &intent.frame, &intent.key)
    }
    fn binding<'a>(&'a self, intent: &Restoration) -> Option<&'a Container> {
        let binding = intent.binding.as_ref()?;
        self.containers
            .iter()
            .find(|current| same_binding(current, binding))
    }
}

fn observe_active(
    world: &World,
    handle: crate::RouterHandle,
    eligible: &HashSet<Entity>,
) -> ActiveObservation {
    let navigation = world.resource::<RouterNavigation>();
    let entry = navigation
        .entries
        .iter()
        .find(|entry| entry.handle == handle)
        .unwrap();
    let active = entry.active.as_ref().filter(|active| {
        entry.engine.availability() == &RouterAvailability::Ready
            && entry.engine.active().kind() == ActivePresentationKind::Route
            && entry
                .host
                .as_ref()
                .is_some_and(|host| crate::navigation::current_host_binding_is_exact(world, host))
            && crate::frame_local::validate_active_branch_with_entering(
                world,
                active,
                entry.current_reconciliation_entering_branch(),
                entry.current_mount_new_staging_allowance(),
            )
            .is_ok()
    });
    let (containers, duplicates) = containers(
        world,
        active.iter().flat_map(|branch| branch.frames()),
        eligible,
    );
    ActiveObservation {
        entry: active.and_then(|_| entry.engine.canonical_history().current_entry()),
        frames: active
            .iter()
            .flat_map(|branch| branch.frames())
            .map(|frame| frame.frame().clone())
            .collect(),
        containers,
        duplicates,
    }
}

/// The only requested-position writer, before the normal layout. Binding and
/// application priority are rechecked against the current exact active branch.
pub(crate) fn prepare_scroll_system(world: &mut World) {
    let handles = world
        .resource::<RouterNavigation>()
        .entries
        .iter()
        .filter(|entry| !entry.scroll.pending.is_empty())
        .map(|entry| entry.handle)
        .collect::<Vec<_>>();
    if handles.is_empty() {
        return;
    }
    let eligible = eligible_containers(world);
    for handle in handles {
        let observation = observe_active(world, handle, &eligible);
        let pending = {
            let mut navigation = world.resource_mut::<RouterNavigation>();
            std::mem::take(
                &mut navigation
                    .entries
                    .iter_mut()
                    .find(|entry| entry.handle == handle)
                    .unwrap()
                    .scroll
                    .pending,
            )
        };
        let mut waiting = Vec::with_capacity(pending.len());
        for mut intent in pending {
            if !observation.accepts(&intent) {
                continue;
            }
            if intent.binding.is_none() {
                intent.binding = observation
                    .containers
                    .iter()
                    .find(|current| current.frame == intent.frame && current.key == intent.key)
                    .cloned();
            }
            let Some(binding) = &intent.binding else {
                waiting.push(intent);
                continue;
            };
            let Some(current) = observation.binding(&intent) else {
                continue;
            };
            if current.position.0 != binding.position.0 {
                continue;
            }
            if current.policy == RouteScrollRestorePolicy::Automatic || intent.ready {
                let mut position = world.get_mut::<ScrollPosition>(current.entity).unwrap();
                if position.0 != intent.position.0 {
                    position.0 = intent.position.0;
                }
                intent.attempted = true;
            }
            waiting.push(intent);
        }
        let mut navigation = world.resource_mut::<RouterNavigation>();
        let state = &mut navigation
            .entries
            .iter_mut()
            .find(|entry| entry.handle == handle)
            .unwrap()
            .scroll;
        state.pending = waiting;
        state.warn_duplicate(
            observation
                .duplicates
                .first()
                .map(|(frame, key)| (frame, key.as_str())),
        );
    }
}

/// Runs once after normal UI layout. Explicit intents remain waiting; attempted
/// intents settle once or abandon missing geometry without chasing range growth.
pub(crate) fn settle_scroll_system(world: &mut World) {
    let eligible = eligible_containers(world);
    if eligible.is_empty()
        && world
            .resource::<RouterNavigation>()
            .entries
            .iter()
            .all(|entry| entry.scroll.pending.is_empty() && entry.scroll.displayed.is_empty())
    {
        return;
    }
    let handles = world
        .resource::<RouterNavigation>()
        .entries
        .iter()
        .map(|entry| entry.handle)
        .collect::<Vec<_>>();
    for handle in handles {
        let mut observation = observe_active(world, handle, &eligible);
        let laid_out = laid_out_containers(world, &observation.containers);
        let missing_layout = observation
            .containers
            .iter()
            .position(|container| !laid_out.contains(&container.entity));
        let pending = {
            let mut navigation = world.resource_mut::<RouterNavigation>();
            std::mem::take(
                &mut navigation
                    .entries
                    .iter_mut()
                    .find(|entry| entry.handle == handle)
                    .unwrap()
                    .scroll
                    .pending,
            )
        };
        let mut waiting = Vec::with_capacity(pending.len());
        for intent in pending {
            if !observation.accepts(&intent) {
                continue;
            }
            if intent.binding.is_none() {
                waiting.push(intent);
                continue;
            }
            let Some(current) = observation.binding(&intent) else {
                continue;
            };
            let binding = intent.binding.as_ref().unwrap();
            if !intent.attempted {
                if current.position.0 == binding.position.0 {
                    waiting.push(intent);
                }
                continue;
            }
            if current.position.0 != intent.position.0 {
                continue;
            }
            let settled = if laid_out.contains(&current.entity) {
                let node = world.get::<ComputedNode>(current.entity).unwrap();
                ScrollPosition(node.scroll_position * node.inverse_scale_factor)
            } else {
                binding.position.clone()
            };
            if current.position.0 != settled.0 {
                world.get_mut::<ScrollPosition>(current.entity).unwrap().0 = settled.0;
            }
        }
        for container in &mut observation.containers {
            if laid_out.contains(&container.entity) {
                let node = world.get::<ComputedNode>(container.entity).unwrap();
                container.position =
                    ScrollPosition(node.scroll_position * node.inverse_scale_factor);
            }
        }
        let mut navigation = world.resource_mut::<RouterNavigation>();
        let entry = navigation
            .entries
            .iter_mut()
            .find(|entry| entry.handle == handle)
            .unwrap();
        entry.scroll.pending = waiting;
        entry.scroll.warn_duplicate(
            observation
                .duplicates
                .first()
                .map(|(frame, key)| (frame, key.as_str())),
        );
        entry
            .scroll
            .warn_layout(missing_layout.map(|index| &observation.containers[index]));
        entry.scroll.sample_displayed(
            entry.engine.canonical_history().current_entry(),
            observation.containers,
            &laid_out,
        );
    }
}

fn laid_out_containers(world: &mut World, containers: &[Container]) -> HashSet<Entity> {
    if containers.is_empty() || !world.contains_resource::<UiSurface>() {
        return HashSet::new();
    }
    let mut ui = SystemState::<(
        UiRootNodes,
        UiChildren,
        Query<
            (
                &ComputedNode,
                &ComputedUiRenderTargetInfo,
                &ComputedUiTargetCamera,
            ),
            (
                With<Node>,
                With<ContentSize>,
                With<UiTransform>,
                With<UiGlobalTransform>,
            ),
        >,
        Query<&Camera>,
    )>::new(world);
    let Ok((roots, children, layout_nodes, cameras)) = ui.get(world) else {
        return HashSet::new();
    };
    let roots = roots.iter().collect::<HashSet<_>>();
    let surface = world.resource::<UiSurface>();
    containers
        .iter()
        .filter_map(|container| {
            let (node, info, target_camera) = layout_nodes.get(container.entity).ok()?;
            let camera = target_camera.get()?;
            if cameras.get(camera).is_err()
                || info.physical_size().min_element() == 0
                || !info.scale_factor().is_finite()
                || info.scale_factor() <= 0.0
                || !node.inverse_scale_factor.is_finite()
                || node.inverse_scale_factor <= 0.0
                || !node.scroll_position.is_finite()
            {
                return None;
            }
            let mut entity = container.entity;
            let mut seen = HashSet::new();
            loop {
                if layout_nodes.get(entity).is_err() {
                    return None;
                }
                if roots.contains(&entity) {
                    break;
                }
                if !seen.insert(entity) {
                    return None;
                }
                let parent = children.get_parent(entity)?;
                if !children
                    .iter_ui_children(parent)
                    .any(|child| child == entity)
                {
                    return None;
                }
                entity = parent;
            }
            surface
                .root_entity_to_viewport_node
                .contains_key(&entity)
                .then_some(container.entity)
        })
        .collect()
}
