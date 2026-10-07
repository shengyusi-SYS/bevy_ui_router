use std::collections::{HashMap, HashSet};

use bevy_camera::Camera;
use bevy_ecs::{prelude::*, system::SystemState};
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
    presentation::ActivePresentationKind,
};

use crate::{navigation::RouterNavigation, presentation::RealizedFrame};

/// Opts a UI scroll container into history-entry restoration.
///
/// Keys are local to one route frame. Use distinct, stable keys for multiple
/// containers in the same frame; duplicate keys are excluded from restoration.
#[derive(Component, Clone, Debug, Default, Eq, PartialEq)]
pub struct RouteScroll(String);

impl RouteScroll {
    pub fn new(key: impl Into<String>) -> Self {
        Self(key.into())
    }
}

#[derive(Clone)]
struct Container {
    frame: RouteFrameVersion,
    entity: Entity,
    key: String,
    position: ScrollPosition,
}

#[derive(Clone)]
struct Restoration {
    container: Container,
    original: ScrollPosition,
}

#[derive(Clone)]
struct PublishedEntry {
    entry: HistoryEntryKey,
    frames: Vec<HistoryFrameFact>,
}

/// Values only. These observations cannot authorize a frame transition.
#[derive(Default)]
pub(crate) struct ScrollObservation {
    source: Vec<Container>,
    target: Vec<Container>,
    duplicates: bool,
    layout_installed: bool,
}

#[derive(Clone, Default)]
pub(crate) struct ScrollState {
    records: HashMap<HistoryEntryKey, HashMap<(RouteSegmentReuseKey, String), ScrollPosition>>,
    // Exact bindings are short lived: replaced after each normal layout and
    // publication, never used as a second source of history or frame authority.
    displayed: Vec<Container>,
    published: Option<PublishedEntry>,
    pending: Vec<Restoration>,
    warned_duplicate: bool,
    warned_layout: bool,
}

pub(crate) struct ScrollWrite {
    pub(crate) entity: Entity,
    pub(crate) position: ScrollPosition,
}

pub(crate) struct PreparedScroll {
    pub(crate) state: ScrollState,
    pub(crate) writes: Vec<ScrollWrite>,
}

impl ScrollObservation {
    pub(crate) fn capture<'a>(
        world: &World,
        source: impl Iterator<Item = &'a RealizedFrame>,
        target: impl Iterator<Item = &'a RealizedFrame>,
        eligible: &HashSet<Entity>,
    ) -> Self {
        if eligible.is_empty() {
            return Self::default();
        }
        let (source, source_duplicates) = containers(world, source, eligible);
        let (target, target_duplicates) = containers(world, target, eligible);
        Self {
            source,
            target,
            duplicates: source_duplicates || target_duplicates,
            layout_installed: world.contains_resource::<UiSurface>(),
        }
    }

    pub(crate) fn target_entities(&self) -> Vec<Entity> {
        self.target
            .iter()
            .map(|container| container.entity)
            .collect()
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
) -> (Vec<Container>, bool) {
    let mut result = Vec::new();
    let mut duplicates = false;
    for frame in frames {
        let managed = if frame.native_root().is_some() {
            frame.content_input_facts(world, &[]).0
        } else {
            frame.claims().map(|claim| claim.entity()).collect()
        };
        let mut keys = HashMap::<&str, usize>::new();
        for entity in &managed {
            if let Some(marker) = world.get::<RouteScroll>(*entity) {
                *keys.entry(&marker.0).or_default() += 1;
            }
        }
        duplicates |= keys.values().any(|count| *count != 1);
        for entity in managed {
            let Some(marker) = world.get::<RouteScroll>(entity) else {
                continue;
            };
            if keys.get(marker.0.as_str()) != Some(&1) || !eligible.contains(&entity) {
                continue;
            }
            let Some(position) = world.get::<ScrollPosition>(entity) else {
                continue;
            };
            result.push(Container {
                frame: frame.frame().clone(),
                entity,
                key: marker.0.clone(),
                position: position.clone(),
            });
        }
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

impl ScrollState {
    pub(crate) fn prepare(
        &self,
        facts: &HistoryPublicationFacts,
        observation: ScrollObservation,
    ) -> PreparedScroll {
        let mut state = self.clone();
        state.pending.clear();
        if observation.duplicates && !state.warned_duplicate {
            eprintln!(
                "bevy_ui_router: duplicate RouteScroll key within a frame; that key is excluded"
            );
            state.warned_duplicate = true;
        }
        if let Some(entry) = facts.source_entry() {
            let positions = observation
                .source
                .iter()
                .filter_map(|container| {
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
            if positions.is_empty() {
                state.records.remove(&entry);
            } else {
                state.records.insert(entry, positions);
            }
        }
        state
            .records
            .retain(|entry, _| facts.surviving_entries().contains(entry));
        let mut writes = Vec::new();
        if facts.restores_back()
            && let Some(records) = facts
                .target_entry()
                .and_then(|entry| state.records.get(&entry))
        {
            for mut container in observation.target {
                let Some(reuse) = reuse_key(facts.target_frames(), &container.frame) else {
                    continue;
                };
                let Some(position) = records.get(&(reuse.clone(), container.key.clone())) else {
                    continue;
                };
                if !observation.layout_installed {
                    if !state.warned_layout {
                        eprintln!(
                            "bevy_ui_router: RouteScroll restoration requires the normal UI layout and camera"
                        );
                        state.warned_layout = true;
                    }
                    continue;
                }
                let original = container.position.clone();
                container.position = position.clone();
                writes.push(ScrollWrite {
                    entity: container.entity,
                    position: position.clone(),
                });
                state.pending.push(Restoration {
                    container,
                    original,
                });
            }
        }
        // Preserved source observations remain usable until the next layout;
        // entering frames acquire a witness only after an actual layout.
        state.displayed.retain(|container| {
            facts
                .target_frames()
                .iter()
                .any(|fact| fact.frame() == &container.frame)
        });
        state.published = facts.target_entry().map(|entry| PublishedEntry {
            entry,
            frames: facts.target_frames().to_vec(),
        });
        PreparedScroll { state, writes }
    }

    fn sample_displayed(&mut self, current: Option<HistoryEntryKey>, observed: Vec<Container>) {
        // Keep the current visit's last valid display even if authority loss
        // removes its physical branch before another navigation publication.
        // The entry/frame binding comes from the original Core publication.
        if let Some(published) = &self.published
            && current == Some(published.entry)
        {
            let positions = observed
                .iter()
                .filter_map(|container| {
                    let reuse = reuse_key(&published.frames, &container.frame)?;
                    Some((
                        (reuse.clone(), container.key.clone()),
                        container.position.clone(),
                    ))
                })
                .collect::<HashMap<_, _>>();
            if !positions.is_empty() {
                // A container without a valid layout witness keeps its last
                // valid value; publication sanitizes departed container keys.
                self.records
                    .entry(published.entry)
                    .or_default()
                    .extend(positions);
            }
        }
        self.displayed = observed;
    }
}

/// Runs once after the normal UI layout. A missing layout/camera abandons this
/// restoration; it never waits a fixed number of frames or runs layout itself.
pub(crate) fn settle_scroll_system(world: &mut World) {
    let eligible = world
        .query_filtered::<Entity, (With<RouteScroll>, With<Node>, With<ScrollPosition>)>()
        .iter(world)
        .collect::<HashSet<_>>();
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
        let (mut observed, duplicates) = {
            let navigation = world.resource::<RouterNavigation>();
            let entry = navigation
                .entries
                .iter()
                .find(|entry| entry.handle == handle)
                .unwrap();
            let active = entry.active.as_ref().filter(|active| {
                crate::frame_local::validate_active_branch_with_entering(
                    world,
                    active,
                    entry.current_reconciliation_entering_branch(),
                    entry.current_mount_new_staging_allowance(),
                )
                .is_ok()
            });
            if eligible.is_empty()
                || entry.engine.active().kind() != ActivePresentationKind::Route
                || entry.host.as_ref().is_none_or(|host| {
                    !crate::navigation::current_host_binding_is_exact(world, host)
                })
            {
                (Vec::new(), false)
            } else {
                containers(
                    world,
                    active.iter().flat_map(|branch| branch.frames()),
                    &eligible,
                )
            }
        };
        let laid_out = laid_out_containers(world, &observed);
        let lacks_layout = !observed.is_empty() && laid_out.len() != observed.len();
        for container in &mut observed {
            if laid_out.contains(&container.entity) {
                let node = world.get::<ComputedNode>(container.entity).unwrap();
                container.position =
                    ScrollPosition(node.scroll_position * node.inverse_scale_factor);
            }
        }
        let pending = {
            let mut navigation = world.resource_mut::<RouterNavigation>();
            let entry = navigation
                .entries
                .iter_mut()
                .find(|entry| entry.handle == handle)
                .unwrap();
            std::mem::take(&mut entry.scroll.pending)
        };
        let mut missing_layout = false;
        for request in pending {
            let Some(current) = observed
                .iter()
                .find(|container| same_container(container, &request.container))
            else {
                continue;
            };
            let Some(position) = world.get::<ScrollPosition>(current.entity) else {
                continue;
            };
            if position.0 != request.container.position.0 {
                continue;
            }
            let settled = if laid_out.contains(&current.entity) {
                &current.position
            } else {
                missing_layout = true;
                &request.original
            };
            if position.0 != settled.0 {
                world.get_mut::<ScrollPosition>(current.entity).unwrap().0 = settled.0;
            }
        }
        observed.retain(|container| laid_out.contains(&container.entity));
        let mut navigation = world.resource_mut::<RouterNavigation>();
        let entry = navigation
            .entries
            .iter_mut()
            .find(|entry| entry.handle == handle)
            .unwrap();
        entry
            .scroll
            .sample_displayed(entry.engine.canonical_history().current_entry(), observed);
        if duplicates && !entry.scroll.warned_duplicate {
            eprintln!(
                "bevy_ui_router: duplicate RouteScroll key within a frame; that key is excluded"
            );
            entry.scroll.warned_duplicate = true;
        }
        if (missing_layout || lacks_layout) && !entry.scroll.warned_layout {
            eprintln!(
                "bevy_ui_router: RouteScroll restoration requires the normal UI layout and camera"
            );
            entry.scroll.warned_layout = true;
        }
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
