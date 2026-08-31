use std::sync::atomic::{AtomicU64, Ordering};

use bevy_camera::visibility::Visibility;
use bevy_ecs::{
    entity::Entity,
    hierarchy::{ChildOf, Children},
    prelude::World,
    resource::Resource,
};
use bevy_input_focus::InputFocus;
use bevy_picking::Pickable;
use bevy_ui_router_core::{
    HostVersion,
    frame::RouteFrameVersion,
    ids::{NavigationRequestId, RouteSlotId},
    integration::host_version_matches_frame,
    slot::RouteSlotKey,
};

use crate::{
    app_ext::RouterHandle,
    cleanup::cleanup_claim_has_exact_authority,
    components::{ProvisionalSlotLifecycle, RouteOwnershipSeal, RouterInputBarrier},
    content::{ContentColor, ContentPrimitiveKind},
    host::validated_host_candidate_is_current,
    input::{RouteFocusState, RouterInputGate},
    navigation::RouterEntry,
    presentation::{
        FrameLifecycle, RealizedFrame, validate_active_frame_with_child_slots,
        validate_realized_frame_with_child_slots,
    },
    registry::{
        EntityLifecycle, EntityOwner, ProvisionalOwner, QuarantineLease, QuarantineLeaseTable,
        QuarantineRootKind, RegistryOwner, RouteEntityRegistry, observed_owned_state,
    },
};

pub use crate::registry::AuditEntityState;
use crate::registry::QuarantineLeaseId;

const OWNED_ID_FLAG: u64 = 1 << 32;
const AUDIT_NAMESPACE_SHIFT: u32 = 33;
const ID_SEQUENCE_MASK: u64 = (1 << 32) - 1;
const REMOVED_RETENTION: usize = 256;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct AuditEntityId(u64);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorityHostMapping {
    version: HostVersion,
    entity: AuditEntityId,
    top_slot: AuditEntityId,
    app_entity: Entity,
}

impl AuthorityHostMapping {
    pub fn version(&self) -> &HostVersion {
        &self.version
    }

    pub fn entity(&self) -> AuditEntityId {
        self.entity
    }

    pub fn top_slot(&self) -> AuditEntityId {
        self.top_slot
    }

    pub fn matches_app_entity(&self, entity: Entity) -> bool {
        self.app_entity == entity
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthoritySlotKind {
    HostTop,
    ChildOutlet,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthoritySlotMapping {
    id: RouteSlotId,
    entity: AuditEntityId,
    host: HostVersion,
    kind: AuthoritySlotKind,
    parent_frame: Option<RouteFrameVersion>,
    outlet: Option<RouteSlotKey>,
}

impl AuthoritySlotMapping {
    pub fn id(&self) -> RouteSlotId {
        self.id
    }

    pub fn entity(&self) -> AuditEntityId {
        self.entity
    }

    pub fn host(&self) -> &HostVersion {
        &self.host
    }

    pub fn kind(&self) -> AuthoritySlotKind {
        self.kind
    }

    pub fn parent_frame(&self) -> Option<&RouteFrameVersion> {
        self.parent_frame.as_ref()
    }

    pub fn outlet(&self) -> Option<&RouteSlotKey> {
        self.outlet.as_ref()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthorityFrameLifecycle {
    PreparedInert,
    Active,
    LeavingInert,
    Quarantined,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorityFrameMapping {
    frame: RouteFrameVersion,
    root: AuditEntityId,
    slot_entity: AuditEntityId,
    lifecycle: AuthorityFrameLifecycle,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorityOutletMapping {
    parent: RouteFrameVersion,
    key: RouteSlotKey,
    anchor: AuditEntityId,
    child_slot: Option<AuditEntityId>,
}

impl AuthorityOutletMapping {
    pub fn parent(&self) -> &RouteFrameVersion {
        &self.parent
    }

    pub fn key(&self) -> &RouteSlotKey {
        &self.key
    }

    pub fn anchor(&self) -> AuditEntityId {
        self.anchor
    }

    pub fn child_slot(&self) -> Option<AuditEntityId> {
        self.child_slot
    }
}

impl AuthorityFrameMapping {
    pub fn frame(&self) -> &RouteFrameVersion {
        &self.frame
    }

    pub fn root(&self) -> AuditEntityId {
        self.root
    }

    pub fn slot_entity(&self) -> AuditEntityId {
        self.slot_entity
    }

    pub fn lifecycle(&self) -> AuthorityFrameLifecycle {
        self.lifecycle
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorityContentMapping {
    frame: RouteFrameVersion,
    entity: AuditEntityId,
    width_px: u16,
    height_px: u16,
    color: ContentColor,
    visibility_hidden: bool,
    pickable_active: bool,
}

impl AuthorityContentMapping {
    pub fn frame(&self) -> &RouteFrameVersion {
        &self.frame
    }

    pub fn entity(&self) -> AuditEntityId {
        self.entity
    }

    pub fn primitive(&self) -> ContentPrimitiveKind {
        ContentPrimitiveKind::SolidRect
    }

    pub fn width_px(&self) -> u16 {
        self.width_px
    }

    pub fn height_px(&self) -> u16 {
        self.height_px
    }

    pub fn color(&self) -> ContentColor {
        self.color
    }

    pub fn visibility_hidden(&self) -> bool {
        self.visibility_hidden
    }

    pub fn pickable_active(&self) -> bool {
        self.pickable_active
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RouterLifecycleAuditEventKind {
    ShellPrepared,
    FrameIsolated,
    FrameActivated,
    RegistryPublished,
    QuarantineOpened,
    QuarantineReaped,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouterLifecycleAuditEvent {
    kind: RouterLifecycleAuditEventKind,
    frame: Option<RouteFrameVersion>,
    entity: Option<AuditEntityId>,
    visibility_hidden: bool,
    accepts_pointer: bool,
    accepts_keyboard: bool,
    accepts_gamepad: bool,
    focused: bool,
}

impl RouterLifecycleAuditEvent {
    pub fn kind(&self) -> RouterLifecycleAuditEventKind {
        self.kind
    }

    pub fn frame(&self) -> Option<&RouteFrameVersion> {
        self.frame.as_ref()
    }

    pub fn entity(&self) -> Option<AuditEntityId> {
        self.entity
    }

    pub fn visibility_hidden(&self) -> bool {
        self.visibility_hidden
    }

    pub fn accepts_pointer(&self) -> bool {
        self.accepts_pointer
    }

    pub fn accepts_keyboard(&self) -> bool {
        self.accepts_keyboard
    }

    pub fn accepts_gamepad(&self) -> bool {
        self.accepts_gamepad
    }

    pub fn focused(&self) -> bool {
        self.focused
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QuarantineOwnerKind {
    HostTopSlot,
    ChildSlot,
    FrameShell,
    FrameOutput,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuarantineOwnerSnapshot {
    lease_id: QuarantineLeaseId,
    kind: QuarantineOwnerKind,
    former_host: HostVersion,
    entity_count: usize,
}

impl QuarantineOwnerSnapshot {
    pub fn lease_id(&self) -> QuarantineLeaseId {
        self.lease_id
    }

    pub fn kind(&self) -> QuarantineOwnerKind {
        self.kind
    }

    pub fn former_host(&self) -> &HostVersion {
        &self.former_host
    }

    pub fn entity_count(&self) -> usize {
        self.entity_count
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuarantineSnapshot {
    root: AuditEntityId,
    foreign_targets: Vec<AuditEntityId>,
    root_owner: QuarantineOwnerSnapshot,
    foreign_component_count: usize,
    protected_parent: bool,
    inert: bool,
}

impl QuarantineSnapshot {
    pub fn former_host(&self) -> &HostVersion {
        self.root_owner.former_host()
    }

    pub fn root(&self) -> AuditEntityId {
        self.root
    }

    pub fn foreign_targets(&self) -> &[AuditEntityId] {
        &self.foreign_targets
    }

    pub fn root_owner(&self) -> &QuarantineOwnerSnapshot {
        &self.root_owner
    }

    pub fn foreign_count(&self) -> usize {
        self.foreign_targets.len()
    }

    pub fn foreign_component_count(&self) -> usize {
        self.foreign_component_count
    }

    pub fn protected_parent(&self) -> bool {
        self.protected_parent
    }

    pub fn inert(&self) -> bool {
        self.inert
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouterAuthorityAuditSnapshot {
    host: Option<AuthorityHostMapping>,
    slots: Vec<AuthoritySlotMapping>,
    frames: Vec<AuthorityFrameMapping>,
    outlets: Vec<AuthorityOutletMapping>,
    content: Vec<AuthorityContentMapping>,
    orphan_slots: Vec<RouteSlotId>,
    orphan_slot_mappings: Vec<AuthoritySlotMapping>,
    quarantines: Vec<QuarantineSnapshot>,
    events: Vec<RouterLifecycleAuditEvent>,
    owned_states: Vec<(AuditEntityId, AuditEntityState)>,
    namespace: u32,
    evicted_owned_through: u32,
}

impl RouterAuthorityAuditSnapshot {
    fn empty() -> Self {
        Self {
            host: None,
            slots: Vec::new(),
            frames: Vec::new(),
            outlets: Vec::new(),
            content: Vec::new(),
            orphan_slots: Vec::new(),
            orphan_slot_mappings: Vec::new(),
            quarantines: Vec::new(),
            events: Vec::new(),
            owned_states: Vec::new(),
            namespace: 0,
            evicted_owned_through: 0,
        }
    }

    pub fn host(&self) -> Option<&AuthorityHostMapping> {
        self.host.as_ref()
    }

    pub fn slots(&self) -> &[AuthoritySlotMapping] {
        &self.slots
    }

    pub fn frames(&self) -> &[AuthorityFrameMapping] {
        &self.frames
    }

    pub fn outlets(&self) -> &[AuthorityOutletMapping] {
        &self.outlets
    }

    pub fn content(&self) -> &[AuthorityContentMapping] {
        &self.content
    }

    pub fn orphan_slots(&self) -> &[RouteSlotId] {
        &self.orphan_slots
    }

    pub fn quarantines(&self) -> &[QuarantineSnapshot] {
        &self.quarantines
    }

    pub fn events(&self) -> &[RouterLifecycleAuditEvent] {
        &self.events
    }

    pub fn owned_entity_state(&self, entity: AuditEntityId) -> AuditEntityState {
        self.owned_states
            .iter()
            .find_map(|(stored, state)| (*stored == entity).then_some(*state))
            .unwrap_or_else(|| {
                let namespace = (entity.0 >> AUDIT_NAMESPACE_SHIFT) as u32;
                let sequence = (entity.0 & ID_SEQUENCE_MASK) as u32;
                if namespace == self.namespace
                    && entity.0 & OWNED_ID_FLAG != 0
                    && sequence <= self.evicted_owned_through
                {
                    AuditEntityState::NotRetained
                } else {
                    AuditEntityState::Unknown
                }
            })
    }
}

#[derive(Clone)]
struct AuditIdentityRecord {
    router: RouterHandle,
    entity: Entity,
    id: AuditEntityId,
    frame: Option<RouteFrameVersion>,
    request: Option<NavigationRequestId>,
    interactive: bool,
    frame_root: bool,
    removed_at: Option<u64>,
}

struct RouterAuditNamespace {
    router: RouterHandle,
    namespace: u32,
    next_owned: u32,
    next_observed: u32,
    next_removed: u64,
    evicted_owned_through: u32,
}

struct FrameAuditSample {
    frame: RouteFrameVersion,
    root: AuditEntityId,
    entities: Vec<Entity>,
    interactive: Vec<Entity>,
}

#[derive(Clone, Copy)]
struct EventStateSample {
    visibility_hidden: bool,
    accepts_pointer: bool,
    accepts_keyboard: bool,
    accepts_gamepad: bool,
    focused: bool,
}

fn sample_event_state(
    world: &World,
    entities: &[Entity],
    interactive: &[Entity],
) -> EventStateSample {
    let visibility_hidden = !entities.is_empty()
        && entities
            .iter()
            .all(|entity| world.get::<Visibility>(*entity) == Some(&Visibility::Hidden));
    let gate = world.get_resource::<RouterInputGate>();
    let accepts_pointer = !interactive.is_empty()
        && gate.is_some_and(|gate| {
            interactive
                .iter()
                .any(|entity| gate.allows_pointer(*entity))
        });
    let accepts_keyboard = !interactive.is_empty()
        && gate.is_some_and(|gate| {
            interactive
                .iter()
                .any(|entity| gate.allows_keyboard(*entity))
        });
    let accepts_gamepad = !interactive.is_empty()
        && gate.is_some_and(|gate| {
            interactive
                .iter()
                .any(|entity| gate.allows_gamepad(*entity))
        });
    let input_focus = world.get_resource::<InputFocus>().and_then(InputFocus::get);
    let route_focus = world
        .get_resource::<RouteFocusState>()
        .and_then(RouteFocusState::focused);
    let focused = entities
        .iter()
        .any(|entity| Some(*entity) == input_focus || Some(*entity) == route_focus);
    EventStateSample {
        visibility_hidden,
        accepts_pointer,
        accepts_keyboard,
        accepts_gamepad,
        focused,
    }
}

fn quarantine_owner_frame(owner: &RegistryOwner) -> Option<&RouteFrameVersion> {
    match owner {
        RegistryOwner::Committed(EntityOwner::ChildSlot { parent, .. })
        | RegistryOwner::Provisional(ProvisionalOwner::ChildSlot { parent, .. }) => Some(parent),
        RegistryOwner::Committed(
            EntityOwner::FrameShell(frame)
            | EntityOwner::FrameContentTarget(frame)
            | EntityOwner::FrameOutput { frame, .. },
        ) => Some(frame),
        RegistryOwner::Committed(EntityOwner::HostTopSlot { .. })
        | RegistryOwner::Provisional(ProvisionalOwner::HostTopSlot(_)) => None,
    }
}

fn push_host_candidate(candidates: &mut Vec<HostVersion>, host: &HostVersion) {
    if !candidates.contains(host) {
        candidates.push(host.clone());
    }
}

fn quarantine_host_candidates(
    previous: Option<&RouterAuthorityAuditSnapshot>,
    current: &RouterAuthorityAuditSnapshot,
    entry_host: Option<&HostVersion>,
) -> Vec<HostVersion> {
    let mut candidates = Vec::new();
    if let Some(previous) = previous {
        if let Some(host) = previous.host.as_ref() {
            push_host_candidate(&mut candidates, &host.version);
        }
        for slot in previous
            .slots
            .iter()
            .chain(previous.orphan_slot_mappings.iter())
        {
            push_host_candidate(&mut candidates, &slot.host);
        }
        for quarantine in &previous.quarantines {
            push_host_candidate(&mut candidates, quarantine.former_host());
        }
    }
    if let Some(host) = current.host.as_ref() {
        push_host_candidate(&mut candidates, &host.version);
    }
    for slot in current
        .slots
        .iter()
        .chain(current.orphan_slot_mappings.iter())
    {
        push_host_candidate(&mut candidates, &slot.host);
    }
    if let Some(host) = entry_host {
        push_host_candidate(&mut candidates, host);
    }
    candidates
}

fn quarantine_former_host(
    lease: &QuarantineLease,
    router: RouterHandle,
    previous: Option<&RouterAuthorityAuditSnapshot>,
    current: &RouterAuthorityAuditSnapshot,
    entry_host: Option<&HostVersion>,
) -> Option<HostVersion> {
    if lease.root().provisional_top() {
        return lease
            .origin()
            .filter(|origin| origin.router() == router)
            .map(|origin| origin.former_host().clone());
    }
    if let Some(previous) = previous
        && let Some(snapshot) = previous
            .quarantines
            .iter()
            .find(|snapshot| snapshot.root_owner.lease_id == lease.id())
    {
        return Some(snapshot.former_host().clone());
    }

    let candidates = quarantine_host_candidates(previous, current, entry_host);
    if let Some(host) = lease.root().committed_host() {
        return candidates
            .iter()
            .find(|candidate| *candidate == host)
            .cloned();
    }
    if let Some(frame) = lease.root().frame() {
        return candidates
            .iter()
            .find(|candidate| host_version_matches_frame(candidate, frame))
            .cloned();
    }
    None
}

fn quarantine_lease_is_inert(world: &World, lease: &QuarantineLease) -> bool {
    let Some(gate) = world.get_resource::<RouterInputGate>() else {
        return false;
    };
    let mut focus_closure = Vec::new();
    for record in lease.records() {
        let entity = record.entity();
        if world.get_entity(entity).is_err() {
            continue;
        }
        let Ok(claim) = record.try_claim() else {
            return false;
        };
        if !cleanup_claim_has_exact_authority(world, &claim)
            || world.get::<Visibility>(entity) != Some(&Visibility::Hidden)
            || world.get::<Pickable>(entity) != Some(&Pickable::IGNORE)
            || !world
                .get::<RouterInputBarrier>(entity)
                .is_some_and(RouterInputBarrier::denies_all)
            || gate.allows_pointer(entity)
            || gate.allows_keyboard(entity)
            || gate.allows_gamepad(entity)
        {
            return false;
        }
        if !focus_closure.contains(&entity) {
            focus_closure.push(entity);
        }
    }

    let mut cursor = 0;
    while cursor < focus_closure.len() {
        let entity = focus_closure[cursor];
        cursor += 1;
        if let Some(children) = world.get::<Children>(entity) {
            for child in children.iter().copied() {
                if !focus_closure.contains(&child) {
                    focus_closure.push(child);
                }
            }
        }
    }
    let Some(input_focus) = world.get_resource::<InputFocus>() else {
        return false;
    };
    let Some(route_focus) = world.get_resource::<RouteFocusState>() else {
        return false;
    };
    input_focus
        .get()
        .is_none_or(|focused| !focus_closure.contains(&focused))
        && route_focus
            .focused()
            .is_none_or(|focused| !focus_closure.contains(&focused))
}

#[derive(Resource)]
pub struct RouterAuthorityAudits {
    identities: Vec<AuditIdentityRecord>,
    namespaces: Vec<RouterAuditNamespace>,
    snapshots: Vec<(RouterHandle, RouterAuthorityAuditSnapshot)>,
}

impl RouterAuthorityAudits {
    pub(crate) const fn new() -> Self {
        Self {
            identities: Vec::new(),
            namespaces: Vec::new(),
            snapshots: Vec::new(),
        }
    }

    pub fn get(&self, router: RouterHandle) -> Option<&RouterAuthorityAuditSnapshot> {
        self.snapshots
            .iter()
            .find_map(|(stored, snapshot)| (*stored == router).then_some(snapshot))
    }

    pub(crate) fn input_probe_targets(
        &self,
    ) -> impl Iterator<Item = (RouterHandle, AuditEntityId, Entity, bool)> + '_ {
        self.identities
            .iter()
            .filter(|identity| identity.removed_at.is_none())
            .map(|identity| {
                (
                    identity.router,
                    identity.id,
                    identity.entity,
                    identity.id.0 & OWNED_ID_FLAG != 0,
                )
            })
    }

    pub(crate) fn refresh(&mut self, world: &World, entries: &[RouterEntry]) {
        let mut snapshots = Vec::with_capacity(entries.len());
        for entry in entries {
            let namespace = self.ensure_namespace(entry.handle);
            let request = entry.engine.snapshot().running_request();
            let known_frames = entry
                .pending_frames()
                .chain(
                    entry
                        .active
                        .as_ref()
                        .into_iter()
                        .flat_map(|branch| branch.frames()),
                )
                .chain(entry.leaving_cleanup_frames())
                .chain(entry.failed_cleanup_frames())
                .chain(entry.authority_protocol_residual_frames())
                .map(|frame| frame.frame().clone())
                .collect::<Vec<_>>();
            let previous = self.snapshots.iter().find_map(|(handle, snapshot)| {
                (*handle == entry.handle).then_some(snapshot.clone())
            });
            let Some(host) = entry.host.as_ref() else {
                let mut current = RouterAuthorityAuditSnapshot::empty();
                current.namespace = namespace;
                current.orphan_slot_mappings = self.collect_orphan_slots(
                    world,
                    entry.handle,
                    request,
                    &known_frames,
                    previous.as_ref(),
                    &current.slots,
                    None,
                );
                current.orphan_slots = current
                    .orphan_slot_mappings
                    .iter()
                    .map(AuthoritySlotMapping::id)
                    .collect();
                current
                    .owned_states
                    .extend(current.orphan_slot_mappings.iter().map(|slot| {
                        (
                            slot.entity,
                            self.entity_for(slot.entity)
                                .map(|entity| observed_owned_state(world, entity))
                                .unwrap_or(AuditEntityState::NotRetained),
                        )
                    }));
                current.owned_states.extend(
                    entry
                        .authority_protocol_residual_entities()
                        .into_iter()
                        .map(|entity| {
                            (
                                self.id_for(
                                    entry.handle,
                                    entity,
                                    true,
                                    None,
                                    request,
                                    false,
                                    false,
                                ),
                                if world.get_entity(entity).is_ok() {
                                    AuditEntityState::Unknown
                                } else {
                                    AuditEntityState::Removed
                                },
                            )
                        }),
                );
                let snapshot =
                    self.finalize_snapshot(world, entry, previous.as_ref(), current, &[]);
                snapshots.push((entry.handle, snapshot));
                continue;
            };
            let host_is_current = validated_host_candidate_is_current(world, &host.candidate)
                && world
                    .get_resource::<RouteEntityRegistry>()
                    .and_then(|registry| registry.record(host.top_slot))
                    .is_some_and(|record| {
                        record.claim() == host.top_slot_claim
                            && matches!(
                                record.owner(),
                                    RegistryOwner::Committed(EntityOwner::HostTopSlot {
                                        host: version,
                                        slot,
                                    }) if version == &host.version && *slot == host.slot_id
                            )
                            && record.lifecycle() == EntityLifecycle::PreparedInert
                            && record.manifest().matches(world, host.top_slot)
                            && world.get::<RouteOwnershipSeal>(host.top_slot)
                                == Some(&RouteOwnershipSeal::new(record.seal()))
                    })
                && world.get::<ChildOf>(host.top_slot).map(ChildOf::parent) == Some(host.host)
                && world.get::<Visibility>(host.top_slot) == Some(&Visibility::Visible)
                && world.get::<Pickable>(host.top_slot) == Some(&Pickable::IGNORE)
                && world
                    .get::<RouterInputBarrier>(host.top_slot)
                    .is_some_and(RouterInputBarrier::denies_all)
                && world.get::<ProvisionalSlotLifecycle>(host.top_slot)
                    == Some(&ProvisionalSlotLifecycle::PreparedInert);
            if !host_is_current {
                let mut current = RouterAuthorityAuditSnapshot::empty();
                current.namespace = namespace;
                current.orphan_slot_mappings = self.collect_orphan_slots(
                    world,
                    entry.handle,
                    request,
                    &known_frames,
                    previous.as_ref(),
                    &current.slots,
                    Some(&host.version),
                );
                current.orphan_slots = current
                    .orphan_slot_mappings
                    .iter()
                    .map(AuthoritySlotMapping::id)
                    .collect();
                current
                    .owned_states
                    .extend(current.orphan_slot_mappings.iter().map(|slot| {
                        (
                            slot.entity,
                            self.entity_for(slot.entity)
                                .map(|entity| observed_owned_state(world, entity))
                                .unwrap_or(AuditEntityState::NotRetained),
                        )
                    }));
                let snapshot =
                    self.finalize_snapshot(world, entry, previous.as_ref(), current, &[]);
                snapshots.push((entry.handle, snapshot));
                continue;
            }
            let host_id = self.id_for(entry.handle, host.host, false, None, None, false, false);
            let top_slot_id = self.id_for(
                entry.handle,
                host.top_slot,
                true,
                None,
                request,
                false,
                false,
            );
            let mut owned_states = vec![(top_slot_id, AuditEntityState::PresentOwned)];
            let mut frames = Vec::new();
            let mut outlets = Vec::new();
            let mut content = Vec::new();
            let mut samples = Vec::new();
            let tracked_child_slots = entry
                .tracked_child_slots()
                .into_iter()
                .filter(|slot| slot.is_current(world))
                .collect::<Vec<_>>();
            let frame_roles = entry
                .pending_frames()
                .map(|frame| (frame, AuthorityFrameLifecycle::PreparedInert))
                .chain(
                    entry
                        .active
                        .as_ref()
                        .into_iter()
                        .flat_map(|branch| branch.frames())
                        .map(|frame| (frame, AuthorityFrameLifecycle::Active)),
                )
                .chain(
                    entry
                        .leaving_cleanup_frames()
                        .into_iter()
                        .map(|frame| (frame, AuthorityFrameLifecycle::LeavingInert)),
                )
                .chain(
                    entry
                        .failed_cleanup_frames()
                        .iter()
                        .map(|frame| (frame, AuthorityFrameLifecycle::PreparedInert)),
                );
            for (frame, lifecycle) in frame_roles {
                let frame_is_current = match lifecycle {
                    AuthorityFrameLifecycle::PreparedInert => {
                        let child_slots = tracked_child_slots
                            .iter()
                            .filter(|slot| slot.parent() == frame.frame())
                            .map(|slot| (slot.anchor(), slot.expected_anchor_children().to_vec()))
                            .collect::<Vec<_>>();
                        validate_realized_frame_with_child_slots(world, frame, &child_slots)
                    }
                    AuthorityFrameLifecycle::Active => {
                        let child_slots = tracked_child_slots
                            .iter()
                            .filter(|slot| slot.parent() == frame.frame())
                            .map(|slot| (slot.anchor(), slot.expected_anchor_children().to_vec()))
                            .collect::<Vec<_>>();
                        validate_active_frame_with_child_slots(world, frame, &child_slots)
                    }
                    AuthorityFrameLifecycle::LeavingInert => {
                        frame_registry_is_current(world, frame, EntityLifecycle::Leaving)
                    }
                    AuthorityFrameLifecycle::Quarantined => false,
                };
                if !frame_is_current {
                    continue;
                }
                let interactive = frame.interactive_entities();
                let entities = frame
                    .claims()
                    .map(|claim| claim.entity())
                    .collect::<Vec<_>>();
                let mut root = None;
                for entity in &entities {
                    let is_root = *entity == frame.shell();
                    let id = self.id_for(
                        entry.handle,
                        *entity,
                        true,
                        Some(frame.frame()),
                        request,
                        interactive.contains(entity),
                        is_root,
                    );
                    if !owned_states.iter().any(|(stored, _)| *stored == id) {
                        owned_states.push((id, AuditEntityState::PresentOwned));
                    }
                    if is_root {
                        root = Some(id);
                    }
                }
                let root = root.expect("validated frame must include its shell");
                let slot_entity = if frame.mount_binding().slot() == host.top_slot
                    && frame.frame().slot() == host.slot_id
                {
                    top_slot_id
                } else {
                    let Some(tracked) = tracked_child_slots.iter().find(|tracked| {
                        tracked.entity() == frame.mount_binding().slot()
                            && tracked.slot() == frame.frame().slot()
                    }) else {
                        continue;
                    };
                    self.id_for(
                        entry.handle,
                        tracked.entity(),
                        true,
                        Some(tracked.parent()),
                        request,
                        false,
                        false,
                    )
                };
                frames.push(AuthorityFrameMapping {
                    frame: frame.frame().clone(),
                    root,
                    slot_entity,
                    lifecycle,
                });
                let expected_lifecycle = match lifecycle {
                    AuthorityFrameLifecycle::PreparedInert => EntityLifecycle::PreparedInert,
                    AuthorityFrameLifecycle::Active => EntityLifecycle::Active,
                    AuthorityFrameLifecycle::LeavingInert => EntityLifecycle::Leaving,
                    AuthorityFrameLifecycle::Quarantined => continue,
                };
                for outlet in frame.outlets() {
                    let anchor_is_exact = outlet.parent() == frame.frame()
                        && world
                            .get_resource::<RouteEntityRegistry>()
                            .and_then(|registry| registry.record(outlet.anchor()))
                            .is_some_and(|record| {
                                record.claim() == *outlet.anchor_claim()
                                    && record.lifecycle() == expected_lifecycle
                                    && matches!(
                                        record.owner(),
                                        RegistryOwner::Committed(EntityOwner::FrameOutput {
                                            frame: owner_frame,
                                            ..
                                        }) if owner_frame == outlet.parent()
                                    )
                                    && record.manifest().matches(world, outlet.anchor())
                                    && world.get::<RouteOwnershipSeal>(outlet.anchor())
                                        == Some(&RouteOwnershipSeal::new(record.seal()))
                            });
                    if !anchor_is_exact {
                        continue;
                    }
                    let anchor = self.id_for(
                        entry.handle,
                        outlet.anchor(),
                        true,
                        Some(frame.frame()),
                        request,
                        interactive.contains(&outlet.anchor()),
                        false,
                    );
                    let child_slot = tracked_child_slots
                        .iter()
                        .find(|slot| {
                            slot.parent() == outlet.parent()
                                && slot.outlet() == outlet.key()
                                && slot.anchor() == outlet.anchor()
                        })
                        .map(|slot| {
                            self.id_for(
                                entry.handle,
                                slot.entity(),
                                true,
                                Some(slot.parent()),
                                request,
                                false,
                                false,
                            )
                        });
                    outlets.push(AuthorityOutletMapping {
                        parent: outlet.parent().clone(),
                        key: outlet.key().clone(),
                        anchor,
                        child_slot,
                    });
                }
                for (entity, rect) in frame.audit_content() {
                    let id = self.id_for(
                        entry.handle,
                        entity,
                        true,
                        Some(frame.frame()),
                        request,
                        true,
                        false,
                    );
                    content.push(AuthorityContentMapping {
                        frame: frame.frame().clone(),
                        entity: id,
                        width_px: rect.width_px(),
                        height_px: rect.height_px(),
                        color: rect.color(),
                        visibility_hidden: world.get::<Visibility>(entity)
                            == Some(&Visibility::Hidden),
                        pickable_active: world.get::<Pickable>(entity).is_some_and(|pickable| {
                            pickable.should_block_lower && pickable.is_hoverable
                        }),
                    });
                }
                samples.push(FrameAuditSample {
                    frame: frame.frame().clone(),
                    root,
                    entities,
                    interactive,
                });
            }
            let mut slots = vec![AuthoritySlotMapping {
                id: host.slot_id,
                entity: top_slot_id,
                host: host.version.clone(),
                kind: AuthoritySlotKind::HostTop,
                parent_frame: None,
                outlet: None,
            }];
            for tracked in &tracked_child_slots {
                if !frames.iter().any(|frame| frame.frame == *tracked.parent())
                    || slots.iter().any(|item| item.id == tracked.slot())
                {
                    continue;
                }
                let entity = self.id_for(
                    entry.handle,
                    tracked.entity(),
                    true,
                    Some(tracked.parent()),
                    request,
                    false,
                    false,
                );
                if !owned_states.iter().any(|(id, _)| *id == entity) {
                    owned_states.push((entity, AuditEntityState::PresentOwned));
                }
                slots.push(AuthoritySlotMapping {
                    id: tracked.slot(),
                    entity,
                    host: host.version.clone(),
                    kind: AuthoritySlotKind::ChildOutlet,
                    parent_frame: Some(tracked.parent().clone()),
                    outlet: Some(tracked.outlet().clone()),
                });
            }
            let orphan_slot_mappings = self.collect_orphan_slots(
                world,
                entry.handle,
                request,
                &known_frames,
                previous.as_ref(),
                &slots,
                Some(&host.version),
            );
            let orphan_slots = orphan_slot_mappings
                .iter()
                .map(AuthoritySlotMapping::id)
                .collect();
            for slot in &orphan_slot_mappings {
                if !owned_states.iter().any(|(id, _)| *id == slot.entity) {
                    owned_states.push((
                        slot.entity,
                        self.entity_for(slot.entity)
                            .map(|entity| observed_owned_state(world, entity))
                            .unwrap_or(AuditEntityState::NotRetained),
                    ));
                }
            }
            outlets.sort_by(|left, right| {
                (
                    left.parent.router().raw(),
                    left.parent.host().raw(),
                    left.parent.slot().raw(),
                    left.parent.frame().raw(),
                    left.parent.generation().raw(),
                    left.key.as_str(),
                )
                    .cmp(&(
                        right.parent.router().raw(),
                        right.parent.host().raw(),
                        right.parent.slot().raw(),
                        right.parent.frame().raw(),
                        right.parent.generation().raw(),
                        right.key.as_str(),
                    ))
            });
            let snapshot = self.finalize_snapshot(
                world,
                entry,
                previous.as_ref(),
                RouterAuthorityAuditSnapshot {
                    host: Some(AuthorityHostMapping {
                        version: host.version.clone(),
                        entity: host_id,
                        top_slot: top_slot_id,
                        app_entity: host.host,
                    }),
                    slots,
                    frames,
                    outlets,
                    content,
                    orphan_slots,
                    orphan_slot_mappings,
                    quarantines: Vec::new(),
                    events: Vec::new(),
                    owned_states,
                    namespace,
                    evicted_owned_through: 0,
                },
                &samples,
            );
            snapshots.push((entry.handle, snapshot));
        }
        self.snapshots = snapshots;
    }

    fn finalize_snapshot(
        &mut self,
        world: &World,
        entry: &RouterEntry,
        previous: Option<&RouterAuthorityAuditSnapshot>,
        mut current: RouterAuthorityAuditSnapshot,
        samples: &[FrameAuditSample],
    ) -> RouterAuthorityAuditSnapshot {
        self.collect_quarantine_snapshots(world, entry, previous, &mut current);
        if let Some(previous) = previous {
            for (id, _) in &previous.owned_states {
                if current
                    .owned_states
                    .iter()
                    .any(|(current, _)| current == id)
                {
                    continue;
                }
                let state = self
                    .entity_for(*id)
                    .map(|entity| observed_owned_state(world, entity))
                    .unwrap_or(AuditEntityState::NotRetained);
                current.owned_states.push((*id, state));
            }
        }

        let mut events = previous
            .map(|snapshot| snapshot.events.clone())
            .unwrap_or_default();
        self.append_lifecycle_events(world, previous, &current, samples, &mut events);
        const EVENT_RETENTION: usize = 128;
        if events.len() > EVENT_RETENTION {
            events.drain(..events.len() - EVENT_RETENTION);
        }
        current.events = events;

        for (id, state) in &current.owned_states {
            if *state != AuditEntityState::Removed {
                continue;
            }
            let Some(identity_index) = self.identities.iter().position(|item| item.id == *id)
            else {
                continue;
            };
            if self.identities[identity_index].removed_at.is_none() {
                let namespace = self
                    .namespaces
                    .iter_mut()
                    .find(|item| item.router == entry.handle)
                    .expect("router audit namespace exists");
                namespace.next_removed = namespace
                    .next_removed
                    .checked_add(1)
                    .expect("audit removal sequence exhausted");
                self.identities[identity_index].removed_at = Some(namespace.next_removed);
            }
        }

        let mut unpinned_removed = self
            .identities
            .iter()
            .filter(|item| item.router == entry.handle && item.removed_at.is_some())
            .filter(|item| {
                !item
                    .request
                    .is_some_and(|request| entry.engine.request(request).is_some())
            })
            .map(|item| (item.id, item.removed_at.unwrap()))
            .collect::<Vec<_>>();
        unpinned_removed.sort_by_key(|(_, removed_at)| std::cmp::Reverse(*removed_at));
        let mut evicted = unpinned_removed
            .into_iter()
            .skip(REMOVED_RETENTION)
            .map(|(id, _)| id)
            .collect::<Vec<_>>();
        evicted.extend(
            current
                .owned_states
                .iter()
                .filter(|(_, state)| *state == AuditEntityState::Unknown)
                .filter(|(id, _)| {
                    !current
                        .quarantines
                        .iter()
                        .any(|quarantine| quarantine.root == *id)
                })
                .filter_map(|(id, _)| {
                    self.identities
                        .iter()
                        .find(|item| item.id == *id)
                        .filter(|item| {
                            !item
                                .request
                                .is_some_and(|request| entry.engine.request(request).is_some())
                        })
                        .map(|_| *id)
                }),
        );
        evicted.sort_by_key(|id| id.0);
        evicted.dedup();
        if !evicted.is_empty() {
            let namespace = self
                .namespaces
                .iter_mut()
                .find(|item| item.router == entry.handle)
                .expect("router audit namespace exists");
            for id in &evicted {
                namespace.evicted_owned_through = namespace
                    .evicted_owned_through
                    .max((id.0 & ID_SEQUENCE_MASK) as u32);
            }
            current.owned_states.retain(|(id, _)| !evicted.contains(id));
            self.identities.retain(|item| !evicted.contains(&item.id));
        }
        let namespace = self
            .namespaces
            .iter()
            .find(|item| item.router == entry.handle)
            .expect("router audit namespace exists");
        current.namespace = namespace.namespace;
        current.evicted_owned_through = namespace.evicted_owned_through;
        let current_host = current.host.as_ref().map(AuthorityHostMapping::entity);
        let current_foreign_targets = current
            .quarantines
            .iter()
            .flat_map(|quarantine| quarantine.foreign_targets.iter().copied())
            .collect::<Vec<_>>();
        self.identities.retain(|item| {
            item.router != entry.handle
                || item.id.0 & OWNED_ID_FLAG != 0
                || Some(item.id) == current_host
                || current_foreign_targets.contains(&item.id)
        });
        current
    }

    fn collect_quarantine_snapshots(
        &mut self,
        world: &World,
        entry: &RouterEntry,
        previous: Option<&RouterAuthorityAuditSnapshot>,
        current: &mut RouterAuthorityAuditSnapshot,
    ) {
        let Some(table) = world.get_resource::<QuarantineLeaseTable>() else {
            return;
        };
        let request = entry.engine.snapshot().running_request();
        let entry_host = entry.host.as_ref().map(|host| &host.version);
        let mut quarantines = Vec::new();
        for lease in table.leases() {
            let Some(former_host) =
                quarantine_former_host(lease, entry.handle, previous, current, entry_host)
            else {
                continue;
            };
            let root_entity = lease.root().entity();
            for record in lease.records() {
                let id = self.id_for(
                    entry.handle,
                    record.entity(),
                    true,
                    quarantine_owner_frame(record.record().owner()),
                    request,
                    false,
                    record.entity() == root_entity,
                );
                let state = observed_owned_state(world, record.entity());
                if let Some((_, current_state)) = current
                    .owned_states
                    .iter_mut()
                    .find(|(current_id, _)| *current_id == id)
                {
                    *current_state = state;
                } else {
                    current.owned_states.push((id, state));
                }
            }
            let kind = match lease.root().kind() {
                QuarantineRootKind::HostTopSlot => QuarantineOwnerKind::HostTopSlot,
                QuarantineRootKind::ChildSlot => QuarantineOwnerKind::ChildSlot,
                QuarantineRootKind::FrameShell => QuarantineOwnerKind::FrameShell,
                QuarantineRootKind::FrameOutput => QuarantineOwnerKind::FrameOutput,
                QuarantineRootKind::ResidualContentTarget => continue,
            };
            let previous_quarantine = previous.and_then(|snapshot| {
                snapshot
                    .quarantines
                    .iter()
                    .find(|snapshot| snapshot.root_owner.lease_id == lease.id())
            });
            let root = if let Some(previous_quarantine) = previous_quarantine {
                if self.entity_for(previous_quarantine.root) != Some(root_entity) {
                    continue;
                }
                previous_quarantine.root
            } else {
                self.id_for(
                    entry.handle,
                    root_entity,
                    true,
                    lease.root().frame(),
                    request,
                    false,
                    true,
                )
            };
            if !lease
                .records()
                .iter()
                .any(|record| record.entity() == root_entity)
                && !current
                    .owned_states
                    .iter()
                    .any(|(current_id, _)| *current_id == root)
            {
                current.owned_states.push((
                    root,
                    if world.get_entity(root_entity).is_err() {
                        AuditEntityState::Removed
                    } else {
                        AuditEntityState::Unknown
                    },
                ));
            }

            let foreign_targets = lease
                .facts()
                .foreign_descendants()
                .iter()
                .copied()
                .map(|entity| self.id_for(entry.handle, entity, false, None, request, false, false))
                .collect();
            quarantines.push(QuarantineSnapshot {
                root,
                foreign_targets,
                root_owner: QuarantineOwnerSnapshot {
                    lease_id: lease.id(),
                    kind,
                    former_host,
                    entity_count: lease.entity_count(),
                },
                foreign_component_count: lease.facts().foreign_components().len(),
                protected_parent: lease.facts().protected_parent().is_some(),
                inert: quarantine_lease_is_inert(world, lease),
            });
        }
        current.quarantines = quarantines;
    }

    fn append_lifecycle_events(
        &self,
        world: &World,
        previous: Option<&RouterAuthorityAuditSnapshot>,
        current: &RouterAuthorityAuditSnapshot,
        samples: &[FrameAuditSample],
        events: &mut Vec<RouterLifecycleAuditEvent>,
    ) {
        let previous_slots = previous.map_or(&[][..], |snapshot| snapshot.slots.as_slice());
        for slot in &current.slots {
            if !previous_slots.iter().any(|old| old.entity == slot.entity) {
                events.push(self.event_for_entity(
                    world,
                    RouterLifecycleAuditEventKind::RegistryPublished,
                    None,
                    slot.entity,
                ));
            }
        }

        let previous_frames = previous.map_or(&[][..], |snapshot| snapshot.frames.as_slice());

        // Isolation must be observable before activation during replacement commit.
        for frame in &current.frames {
            if frame.lifecycle == AuthorityFrameLifecycle::LeavingInert
                && previous_frames.iter().any(|old| {
                    old.root == frame.root && old.lifecycle == AuthorityFrameLifecycle::Active
                })
            {
                events.push(self.event_for_frame(
                    world,
                    RouterLifecycleAuditEventKind::FrameIsolated,
                    samples.iter().find(|sample| sample.root == frame.root),
                ));
            }
        }

        for frame in &current.frames {
            let previous_frame = previous_frames.iter().find(|old| old.root == frame.root);
            if previous_frame.is_none() {
                events.push(self.event_for_frame(
                    world,
                    RouterLifecycleAuditEventKind::RegistryPublished,
                    samples.iter().find(|sample| sample.root == frame.root),
                ));
                if frame.lifecycle == AuthorityFrameLifecycle::PreparedInert {
                    events.push(self.event_for_frame(
                        world,
                        RouterLifecycleAuditEventKind::ShellPrepared,
                        samples.iter().find(|sample| sample.root == frame.root),
                    ));
                }
            }
        }

        for frame in &current.frames {
            let activated = previous_frames
                .iter()
                .find(|old| old.root == frame.root)
                .is_some_and(|old| {
                    old.lifecycle == AuthorityFrameLifecycle::PreparedInert
                        && frame.lifecycle == AuthorityFrameLifecycle::Active
                });
            if activated {
                events.push(self.event_for_frame(
                    world,
                    RouterLifecycleAuditEventKind::FrameActivated,
                    samples.iter().find(|sample| sample.root == frame.root),
                ));
            }
        }

        if let Some(previous) = previous {
            let mut emitted_frames = Vec::new();
            for (id, state) in &current.owned_states {
                let old = previous.owned_entity_state(*id);
                let kind = match (old, state) {
                    (AuditEntityState::Quarantined, AuditEntityState::Removed) => {
                        Some(RouterLifecycleAuditEventKind::QuarantineReaped)
                    }
                    (old, AuditEntityState::Quarantined)
                        if old != AuditEntityState::Quarantined =>
                    {
                        Some(RouterLifecycleAuditEventKind::QuarantineOpened)
                    }
                    _ => None,
                };
                if let Some(kind) = kind {
                    let identity = self.identities.iter().find(|item| item.id == *id);
                    if let Some(frame) = identity.and_then(|item| item.frame.as_ref()) {
                        if emitted_frames.contains(frame) {
                            continue;
                        }
                        emitted_frames.push(frame.clone());
                        let root = self
                            .identities
                            .iter()
                            .find(|item| item.frame.as_ref() == Some(frame) && item.frame_root);
                        let entities = self
                            .identities
                            .iter()
                            .filter(|item| item.frame.as_ref() == Some(frame))
                            .map(|item| item.entity)
                            .collect::<Vec<_>>();
                        let interactive = self
                            .identities
                            .iter()
                            .filter(|item| item.frame.as_ref() == Some(frame) && item.interactive)
                            .map(|item| item.entity)
                            .collect::<Vec<_>>();
                        let sample = sample_event_state(world, &entities, &interactive);
                        events.push(Self::build_event(
                            kind,
                            Some(frame.clone()),
                            root.map(|item| item.id).or(Some(*id)),
                            sample,
                        ));
                    } else {
                        events.push(self.event_for_entity(world, kind, None, *id));
                    }
                }
            }
        }
    }

    fn event_for_frame(
        &self,
        world: &World,
        kind: RouterLifecycleAuditEventKind,
        sample: Option<&FrameAuditSample>,
    ) -> RouterLifecycleAuditEvent {
        let sample = sample.expect("current frame transition must have an exact-owned sample");
        Self::build_event(
            kind,
            Some(sample.frame.clone()),
            Some(sample.root),
            sample_event_state(world, &sample.entities, &sample.interactive),
        )
    }

    fn event_for_entity(
        &self,
        world: &World,
        kind: RouterLifecycleAuditEventKind,
        frame: Option<RouteFrameVersion>,
        entity: AuditEntityId,
    ) -> RouterLifecycleAuditEvent {
        let entities = self.entity_for(entity).into_iter().collect::<Vec<_>>();
        Self::build_event(
            kind,
            frame,
            Some(entity),
            sample_event_state(world, &entities, &[]),
        )
    }

    fn build_event(
        kind: RouterLifecycleAuditEventKind,
        frame: Option<RouteFrameVersion>,
        entity: Option<AuditEntityId>,
        sample: EventStateSample,
    ) -> RouterLifecycleAuditEvent {
        RouterLifecycleAuditEvent {
            kind,
            frame,
            entity,
            visibility_hidden: sample.visibility_hidden,
            accepts_pointer: sample.accepts_pointer,
            accepts_keyboard: sample.accepts_keyboard,
            accepts_gamepad: sample.accepts_gamepad,
            focused: sample.focused,
        }
    }

    fn entity_for(&self, id: AuditEntityId) -> Option<Entity> {
        self.identities
            .iter()
            .find_map(|item| (item.id == id).then_some(item.entity))
    }

    #[allow(clippy::too_many_arguments)]
    fn id_for(
        &mut self,
        router: RouterHandle,
        entity: Entity,
        owned: bool,
        frame: Option<&RouteFrameVersion>,
        request: Option<NavigationRequestId>,
        interactive: bool,
        frame_root: bool,
    ) -> AuditEntityId {
        if let Some(item) = self.identities.iter_mut().find(|item| {
            item.router == router
                && item.entity == entity
                && (item.id.0 & OWNED_ID_FLAG != 0) == owned
        }) {
            if frame.is_some() {
                item.frame = frame.cloned();
            }
            if request.is_some() {
                item.request = request;
            }
            item.interactive |= interactive;
            item.frame_root |= frame_root;
            return item.id;
        }
        let namespace = self
            .namespaces
            .iter_mut()
            .find(|item| item.router == router)
            .expect("router audit namespace exists");
        let sequence = if owned {
            namespace.next_owned = namespace
                .next_owned
                .checked_add(1)
                .expect("owned audit id exhausted");
            namespace.next_owned
        } else {
            namespace.next_observed = namespace
                .next_observed
                .checked_add(1)
                .expect("observed audit id exhausted");
            namespace.next_observed
        };
        let raw = ((namespace.namespace as u64) << AUDIT_NAMESPACE_SHIFT)
            | u64::from(owned) * OWNED_ID_FLAG
            | u64::from(sequence);
        let id = AuditEntityId(raw);
        self.identities.push(AuditIdentityRecord {
            router,
            entity,
            id,
            frame: frame.cloned(),
            request,
            interactive,
            frame_root,
            removed_at: None,
        });
        id
    }

    fn ensure_namespace(&mut self, router: RouterHandle) -> u32 {
        if let Some(item) = self.namespaces.iter().find(|item| item.router == router) {
            return item.namespace;
        }
        static NEXT_AUDIT_NAMESPACE: AtomicU64 = AtomicU64::new(1);
        let raw = NEXT_AUDIT_NAMESPACE
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(1)
            })
            .expect("audit namespace exhausted");
        let namespace = u32::try_from(raw)
            .ok()
            .filter(|namespace| *namespace < (1 << 31))
            .expect("audit namespace exhausted");
        self.namespaces.push(RouterAuditNamespace {
            router,
            namespace,
            next_owned: 0,
            next_observed: 0,
            next_removed: 0,
            evicted_owned_through: 0,
        });
        namespace
    }

    fn collect_orphan_slots(
        &mut self,
        world: &World,
        router: RouterHandle,
        request: Option<NavigationRequestId>,
        known_frames: &[RouteFrameVersion],
        previous: Option<&RouterAuthorityAuditSnapshot>,
        current: &[AuthoritySlotMapping],
        known_host: Option<&HostVersion>,
    ) -> Vec<AuthoritySlotMapping> {
        let mut orphans: Vec<AuthoritySlotMapping> = Vec::new();
        if let Some(previous) = previous {
            for slot in previous
                .slots
                .iter()
                .chain(previous.orphan_slot_mappings.iter())
            {
                if current.iter().any(|item| item.id == slot.id) {
                    continue;
                }
                let still_exact_owned = self.entity_for(slot.entity).is_some_and(|entity| {
                    matches!(
                        observed_owned_state(world, entity),
                        AuditEntityState::PresentOwned | AuditEntityState::Quarantined
                    )
                });
                if still_exact_owned && !orphans.iter().any(|item| item.id == slot.id) {
                    orphans.push(slot.clone());
                }
            }
        }

        let mut host_versions = previous
            .into_iter()
            .flat_map(|snapshot| {
                snapshot
                    .slots
                    .iter()
                    .chain(snapshot.orphan_slot_mappings.iter())
                    .map(|slot| slot.host.clone())
            })
            .collect::<Vec<_>>();
        if let Some(host) = known_host
            && !host_versions.contains(host)
        {
            host_versions.push(host.clone());
        }
        let mut frame_versions = known_frames.to_vec();
        if let Some(previous) = previous {
            frame_versions.extend(previous.frames.iter().map(|frame| frame.frame.clone()));
            frame_versions.extend(
                previous
                    .slots
                    .iter()
                    .chain(previous.orphan_slot_mappings.iter())
                    .filter_map(|slot| slot.parent_frame.clone()),
            );
        }
        frame_versions.sort_by_key(|frame| (frame.frame().raw(), frame.slot().raw()));
        frame_versions.dedup();

        let mut facts = Vec::new();
        if let Some(registry) = world.get_resource::<RouteEntityRegistry>() {
            for record in registry.records() {
                facts.push((record.entity(), record.owner().clone()));
            }
        }
        if let Some(table) = world.get_resource::<QuarantineLeaseTable>() {
            for entity in table.inert_owned_entities() {
                let record = table
                    .record(entity)
                    .or_else(|| table.unresolved_record(entity));
                if let Some(record) = record {
                    facts.push((entity, record.owner().clone()));
                }
            }
        }
        facts.sort_by_key(|(entity, owner)| {
            let slot_priority = match owner {
                RegistryOwner::Committed(EntityOwner::ChildSlot { .. })
                | RegistryOwner::Provisional(crate::registry::ProvisionalOwner::ChildSlot {
                    ..
                }) => 0,
                RegistryOwner::Committed(EntityOwner::HostTopSlot { .. }) => 1,
                _ => 2,
            };
            (slot_priority, entity.to_bits())
        });
        facts.dedup_by_key(|(entity, _)| *entity);

        for (entity, owner) in facts {
            let mapping = match owner {
                RegistryOwner::Committed(EntityOwner::HostTopSlot { host, slot })
                    if host_versions.contains(&host)
                        && !current.iter().any(|item| item.id == slot) =>
                {
                    let entity = self.id_for(router, entity, true, None, request, false, false);
                    Some(AuthoritySlotMapping {
                        id: slot,
                        entity,
                        host,
                        kind: AuthoritySlotKind::HostTop,
                        parent_frame: None,
                        outlet: None,
                    })
                }
                RegistryOwner::Committed(EntityOwner::ChildSlot {
                    parent,
                    outlet,
                    slot,
                })
                | RegistryOwner::Provisional(crate::registry::ProvisionalOwner::ChildSlot {
                    parent,
                    outlet,
                    slot,
                }) if (frame_versions.contains(&parent)
                    || host_versions
                        .iter()
                        .any(|host| host_version_matches_frame(host, &parent)))
                    && !current.iter().any(|item| item.id == slot) =>
                {
                    let Some(host) = known_host
                        .filter(|host| host_version_matches_frame(host, &parent))
                        .cloned()
                        .or_else(|| {
                            host_versions
                                .iter()
                                .find(|host| host_version_matches_frame(host, &parent))
                                .cloned()
                        })
                    else {
                        continue;
                    };
                    let entity =
                        self.id_for(router, entity, true, Some(&parent), request, false, false);
                    Some(AuthoritySlotMapping {
                        id: slot,
                        entity,
                        host,
                        kind: AuthoritySlotKind::ChildOutlet,
                        parent_frame: Some(parent),
                        outlet: Some(outlet),
                    })
                }
                _ => None,
            };
            if let Some(mapping) = mapping
                && !orphans.iter().any(|item| item.id == mapping.id)
            {
                orphans.push(mapping);
            }
        }
        orphans.sort_by_key(|slot| slot.id.raw());
        orphans
    }
}

fn frame_registry_is_current(
    world: &World,
    frame: &RealizedFrame,
    lifecycle: EntityLifecycle,
) -> bool {
    frame.claims().all(|claim| {
        claim.lifecycle() == lifecycle
            && world
                .get_resource::<RouteEntityRegistry>()
                .and_then(|registry| registry.record(claim.entity()))
                .is_some_and(|record| {
                    record.claim() == *claim
                        && record.lifecycle() == lifecycle
                        && record.manifest().matches(world, claim.entity())
                })
            && world.get::<Visibility>(claim.entity()) == Some(&Visibility::Hidden)
            && world.get::<Pickable>(claim.entity()) == Some(&Pickable::IGNORE)
            && world.get::<RouteOwnershipSeal>(claim.entity())
                == Some(&RouteOwnershipSeal::new(claim.seal()))
            && world.get::<FrameLifecycle>(claim.entity()) == Some(&FrameLifecycle(lifecycle))
            && world
                .get::<RouterInputBarrier>(claim.entity())
                .is_some_and(RouterInputBarrier::denies_all)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        app_ext::{AppRouterExt, RouterConfig, RouterHostRegistration},
        components::ProvisionalPostWriteProbe,
        content::{ContentColor, FrameContentWriter, SolidRect},
        driver::{EcsViewDriver, EcsViewReportSession, ViewBuildReport},
        navigation::{
            RouterNavigation, collect_navigation_system, commit_presentation_system,
            drive_navigation_system, observe_authority_system, settle_cleanup_system,
        },
        plugin::BevyUiRouterPlugin,
    };
    use bevy_app::App;
    use bevy_ecs::{
        lifecycle::HookContext,
        prelude::{Component, Resource},
    };
    use bevy_ui::Node;
    use bevy_ui_router_core::prelude::{
        RouteLocation, RouteNodeOptions, RoutePattern, RouteTree, RouteViewKey,
    };
    use bevy_ui_router_view::{
        RouteViewDescriptor, RouteViewRegistry, ViewBuildEffect, ViewDriverKey,
    };

    struct AuditDriver(ViewDriverKey);

    #[derive(Component)]
    struct AuditCleanupDrift;

    #[derive(Component)]
    struct AuditProvisionalDrift;

    #[derive(Resource)]
    struct AuditProvisionalDriftHost(Entity);

    #[test]
    fn provisional_quarantine_is_attributed_only_to_its_exact_router_origin() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        app.world_mut()
            .register_component_hooks::<ProvisionalPostWriteProbe>()
            .on_add(|mut world, HookContext { entity, .. }| {
                let target = world
                    .get_resource::<AuditProvisionalDriftHost>()
                    .map(|target| target.0);
                if world.get::<ChildOf>(entity).map(ChildOf::parent) == target {
                    world
                        .commands()
                        .entity(entity)
                        .insert(AuditProvisionalDrift);
                }
            });
        let driver = ViewDriverKey::new("provisional-origin-audit-driver").unwrap();
        app.register_ui_view_driver(AuditDriver(driver.clone()))
            .unwrap();
        let make_config = || {
            let view = RouteViewKey::new("root").unwrap();
            let routes = RouteTree::builder()
                .root_with_options(
                    RoutePattern::path("/").unwrap(),
                    RouteNodeOptions::new().with_view(view.clone()),
                )
                .unwrap()
                .build()
                .unwrap();
            let mut views = RouteViewRegistry::new();
            views
                .insert(view.clone(), RouteViewDescriptor::new(driver.clone()))
                .unwrap();
            RouterConfig::new(routes, views, view)
        };
        let router_a = app.add_ui_router(make_config()).unwrap();
        let router_b = app.add_ui_router(make_config()).unwrap();
        app.world_mut()
            .spawn((Node::default(), RouterHostRegistration::new(router_b)));
        observe_authority_system(app.world_mut());
        let host_a = app
            .world_mut()
            .spawn((Node::default(), RouterHostRegistration::new(router_a)))
            .id();
        app.world_mut()
            .insert_resource(AuditProvisionalDriftHost(host_a));

        observe_authority_system(app.world_mut());
        assert_eq!(
            app.world()
                .iter_entities()
                .filter(|entity| entity.contains::<AuditProvisionalDrift>())
                .count(),
            1,
            "the production post-write hook changes the real staging predicate input"
        );
        observe_authority_system(app.world_mut());

        let table = app.world().resource::<QuarantineLeaseTable>();
        let lease = table
            .leases()
            .iter()
            .find(|lease| lease.root().provisional_top())
            .expect("the drifted first-admission top slot is quarantined");
        assert_eq!(lease.origin().map(|origin| origin.router()), Some(router_a));
        let audits = app.world().resource::<RouterAuthorityAudits>();
        let snapshot_a = audits
            .get(router_a)
            .expect("router A audit remains present");
        let snapshot_b = audits
            .get(router_b)
            .expect("router B audit remains present");
        assert_eq!(snapshot_a.quarantines().len(), 1);
        assert_eq!(
            snapshot_a.quarantines()[0].root_owner().kind(),
            QuarantineOwnerKind::HostTopSlot
        );
        assert_eq!(
            snapshot_a.quarantines()[0].former_host(),
            lease
                .origin()
                .expect("production quarantine freezes its exact origin")
                .former_host()
        );
        assert!(snapshot_b.quarantines().is_empty());
    }

    #[cfg(feature = "test-support")]
    #[test]
    fn active_nested_audit_uses_active_child_slot_expectations() {
        let (mut app, failed, _, _, _) = crate::navigation::tests::failed_nested_branch_fixture();
        let router = app.world().resource::<RouterNavigation>().entries[0].handle;
        let mut active = failed.into_realized_for_test();
        let targets = active
            .entering_mutation_targets(EntityLifecycle::PreparedInert, EntityLifecycle::Active);
        let transitions = active.lifecycle_transitions(EntityLifecycle::Active, false);
        let resulting = app.world_mut().resource_scope(
            |world, mut registry: bevy_ecs::prelude::Mut<RouteEntityRegistry>| {
                let registry_plan = registry
                    .prepare_lifecycle_mutation(&transitions)
                    .expect("prepared nested claims remain exact");
                let resulting = registry_plan.resulting_claims().to_vec();
                crate::presentation::prepare_presentation_mutation(world, &targets)
                    .expect("prepared nested presentation remains exact")
                    .apply();
                registry_plan.apply(&mut registry);
                resulting
            },
        );
        let rebind = active
            .prepare_claim_rebind(
                &resulting,
                EntityLifecycle::PreparedInert,
                EntityLifecycle::Active,
            )
            .expect("all nested claim aliases remain exact");
        active.apply_claim_rebind(rebind);
        app.world_mut().resource_mut::<RouterNavigation>().entries[0].active = Some(active);

        app.world_mut().resource_scope(
            |world, navigation: bevy_ecs::prelude::Mut<RouterNavigation>| {
                world.resource_scope(
                    |world, mut audits: bevy_ecs::prelude::Mut<RouterAuthorityAudits>| {
                        audits.refresh(world, &navigation.entries);
                    },
                );
            },
        );

        let snapshot = app
            .world()
            .resource::<RouterAuthorityAudits>()
            .get(router)
            .expect("router audit remains present");
        assert_eq!(snapshot.frames().len(), 2);
        assert!(
            snapshot
                .frames()
                .iter()
                .all(|frame| { frame.lifecycle() == AuthorityFrameLifecycle::Active })
        );
        assert!(
            snapshot
                .slots()
                .iter()
                .any(|slot| slot.kind() == AuthoritySlotKind::ChildOutlet)
        );
    }

    impl EcsViewDriver for AuditDriver {
        fn key(&self) -> ViewDriverKey {
            self.0.clone()
        }

        fn apply(
            &mut self,
            _effect: &ViewBuildEffect,
            content: &mut FrameContentWriter<'_>,
            report: EcsViewReportSession<'_>,
        ) -> ViewBuildReport {
            content
                .spawn_empty()
                .expect("structural output should be accepted");
            content
                .spawn_solid_rect(SolidRect::new(8, 6, ContentColor::srgba8(1, 2, 3, 255)).unwrap())
                .unwrap();
            report.applied()
        }
    }

    struct BurstAuditDriver {
        key: ViewDriverKey,
        calls: usize,
    }

    impl EcsViewDriver for BurstAuditDriver {
        fn key(&self) -> ViewDriverKey {
            self.key.clone()
        }

        fn apply(
            &mut self,
            _effect: &ViewBuildEffect,
            content: &mut FrameContentWriter<'_>,
            report: EcsViewReportSession<'_>,
        ) -> ViewBuildReport {
            let count = if self.calls == 0 { 300 } else { 1 };
            self.calls += 1;
            for _ in 0..count {
                content
                    .spawn_solid_rect(
                        SolidRect::new(1, 1, ContentColor::srgba8(1, 1, 1, 255)).unwrap(),
                    )
                    .unwrap();
            }
            report.applied()
        }
    }

    #[test]
    fn audit_refreshes_at_each_authority_mutation_and_omits_structural_outputs() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let driver = ViewDriverKey::new("audit-driver").unwrap();
        app.register_ui_view_driver(AuditDriver(driver.clone()))
            .unwrap();
        let view = RouteViewKey::new("root").unwrap();
        let routes = RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/").unwrap(),
                RouteNodeOptions::new().with_view(view.clone()),
            )
            .unwrap()
            .build()
            .unwrap();
        let mut views = RouteViewRegistry::new();
        views
            .insert(view.clone(), RouteViewDescriptor::new(driver))
            .unwrap();
        let router = app
            .add_ui_router(RouterConfig::new(routes, views, view))
            .unwrap();
        let host = app
            .world_mut()
            .spawn((Node::default(), RouterHostRegistration::new(router)))
            .id();

        observe_authority_system(app.world_mut());
        let host_snapshot = app
            .world()
            .resource::<RouterAuthorityAudits>()
            .get(router)
            .unwrap();
        assert!(host_snapshot.host().unwrap().matches_app_entity(host));
        assert_eq!(host_snapshot.slots().len(), 1);
        assert_eq!(host_snapshot.slots()[0].parent_frame(), None);
        assert_eq!(host_snapshot.slots()[0].outlet(), None);
        assert!(host_snapshot.orphan_slots().is_empty());
        assert!(host_snapshot.frames().is_empty());

        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        let pending = app
            .world()
            .resource::<RouterAuthorityAudits>()
            .get(router)
            .unwrap();
        assert_eq!(pending.frames().len(), 1);
        assert_eq!(
            pending.frames()[0].lifecycle(),
            AuthorityFrameLifecycle::PreparedInert
        );
        assert_eq!(pending.content().len(), 1);
        assert!(pending.content()[0].visibility_hidden());
        assert!(!pending.content()[0].pickable_active());
        assert!(pending.events().iter().any(|event| {
            event.kind() == RouterLifecycleAuditEventKind::ShellPrepared
                && event.frame() == Some(pending.frames()[0].frame())
                && event.entity() == Some(pending.frames()[0].root())
                && event.visibility_hidden()
                && !event.accepts_pointer()
                && !event.accepts_keyboard()
                && !event.accepts_gamepad()
                && !event.focused()
        }));

        commit_presentation_system(app.world_mut());
        settle_cleanup_system(app.world_mut());

        let (stale_output, stale_root, stale_host) = {
            let active = app
                .world()
                .resource::<RouterAuthorityAudits>()
                .get(router)
                .unwrap();
            assert_eq!(
                active.frames()[0].lifecycle(),
                AuthorityFrameLifecycle::Active
            );
            assert!(!active.content()[0].visibility_hidden());
            assert!(active.content()[0].pickable_active());
            assert_eq!(active.frames()[0].slot_entity(), active.slots()[0].entity());
            assert_eq!(
                active.owned_entity_state(active.frames()[0].root()),
                AuditEntityState::PresentOwned
            );
            assert!(active.events().iter().any(|event| {
                event.kind() == RouterLifecycleAuditEventKind::FrameActivated
                    && event.frame() == Some(active.frames()[0].frame())
                    && event.entity() == Some(active.frames()[0].root())
                    && !event.visibility_hidden()
                    && event.accepts_pointer()
                    && event.accepts_keyboard()
                    && event.accepts_gamepad()
            }));
            assert_ne!(active.frames()[0].root(), active.content()[0].entity());
            (
                active.content()[0].entity(),
                active.frames()[0].root(),
                active.host().unwrap().version().clone(),
            )
        };

        let (active_entities, active_interactive) = {
            let frame = app.world().resource::<RouterNavigation>().entries[0]
                .active
                .as_ref()
                .unwrap();
            (
                frame
                    .claims()
                    .map(|claim| claim.entity())
                    .collect::<Vec<_>>(),
                frame.interactive_entities(),
            )
        };
        let focused_output = active_interactive[0];
        *app.world_mut().resource_mut::<InputFocus>() = InputFocus::from_entity(focused_output);
        app.world_mut()
            .resource_mut::<RouteFocusState>()
            .set_focused(Some(focused_output));
        assert!(sample_event_state(app.world(), &active_entities, &active_interactive).focused);

        let output = app.world().resource::<RouterNavigation>().entries[0]
            .active
            .as_ref()
            .unwrap()
            .interactive_entities()[0];
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/").with_fragment("quarantine"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        commit_presentation_system(app.world_mut());
        app.world_mut().entity_mut(output).insert(AuditCleanupDrift);
        let foreign_descendant = app.world_mut().spawn(ChildOf(output)).id();
        settle_cleanup_system(app.world_mut());
        if app
            .world()
            .resource::<crate::registry::RouteEntityRegistry>()
            .record(output)
            .is_some()
        {
            settle_cleanup_system(app.world_mut());
        }

        let drifted = app
            .world()
            .resource::<RouterAuthorityAudits>()
            .get(router)
            .unwrap();
        assert!(app.world().get_entity(output).is_ok());
        assert!(
            app.world()
                .resource::<crate::registry::RouteEntityRegistry>()
                .record(output)
                .is_none()
        );
        assert!(
            app.world()
                .resource::<crate::registry::QuarantineLeaseTable>()
                .record(output)
                .is_some()
        );
        assert_eq!(
            drifted.owned_entity_state(stale_output),
            AuditEntityState::Quarantined
        );
        let quarantine = drifted
            .quarantines()
            .iter()
            .find(|quarantine| quarantine.root() == stale_root)
            .expect("foreign component cleanup exposes the frame-shell quarantine");
        assert_eq!(quarantine.former_host(), &stale_host);
        assert_eq!(
            quarantine.root_owner().kind(),
            QuarantineOwnerKind::FrameShell
        );
        assert_eq!(
            quarantine.root_owner().entity_count(),
            app.world()
                .resource::<QuarantineLeaseTable>()
                .lease(quarantine.root_owner().lease_id())
                .unwrap()
                .entity_count()
        );
        assert!(!quarantine.foreign_targets().is_empty());
        assert_eq!(
            quarantine.foreign_count(),
            quarantine.foreign_targets().len()
        );
        assert!(
            quarantine
                .foreign_targets()
                .iter()
                .all(|target| drifted.owned_entity_state(*target) == AuditEntityState::Unknown)
        );
        assert!(app.world().get_entity(foreign_descendant).is_ok());
        assert_eq!(quarantine.foreign_component_count(), 1);
        assert!(!quarantine.protected_parent());
        assert!(quarantine.inert());
        assert_eq!(
            drifted.owned_entity_state(quarantine.root()),
            AuditEntityState::Quarantined
        );
        assert!(drifted.events().iter().any(|event| {
            event.kind() == RouterLifecycleAuditEventKind::QuarantineOpened
                && event.frame().is_some()
        }));

        assert!(app.world_mut().despawn(output));
        app.world_mut().remove_resource::<QuarantineLeaseTable>();
        app.world_mut().insert_resource(QuarantineLeaseTable::new());
        let mut audits = app
            .world_mut()
            .remove_resource::<RouterAuthorityAudits>()
            .unwrap();
        let navigation = app
            .world_mut()
            .remove_resource::<RouterNavigation>()
            .unwrap();
        audits.refresh(app.world(), &navigation.entries);
        app.world_mut().insert_resource(navigation);
        app.world_mut().insert_resource(audits);
        let reaped = app
            .world()
            .resource::<RouterAuthorityAudits>()
            .get(router)
            .unwrap();
        assert_eq!(
            reaped.owned_entity_state(stale_output),
            AuditEntityState::Removed
        );
        assert!(reaped.events().iter().any(|event| {
            event.kind() == RouterLifecycleAuditEventKind::QuarantineReaped
                && event.frame().is_some()
        }));
    }

    #[test]
    fn event_hidden_fact_requires_explicit_hidden_on_every_owned_entity() {
        let mut world = World::new();
        world.insert_resource(InputFocus::default());
        world.insert_resource(RouteFocusState::default());
        world.insert_resource(crate::input::RouterInputGate::new());
        let shell = world.spawn(Visibility::Hidden).id();
        let missing_visibility = world.spawn_empty().id();
        let denied = world.spawn(Visibility::Hidden).id();
        world.resource_scope(|_world, gate: bevy_ecs::prelude::Mut<RouterInputGate>| {
            crate::input::prepare_input_gate_mutation(gate, &[], vec![missing_visibility])
                .unwrap()
                .apply();
        });
        *world.resource_mut::<InputFocus>() = InputFocus::from_entity(missing_visibility);

        let sample = sample_event_state(
            &world,
            &[shell, missing_visibility, denied],
            &[missing_visibility, denied],
        );

        assert!(!sample.visibility_hidden);
        assert!(sample.accepts_pointer);
        assert!(sample.accepts_keyboard);
        assert!(sample.accepts_gamepad);
        let event = RouterAuthorityAudits::build_event(
            RouterLifecycleAuditEventKind::FrameActivated,
            None,
            Some(AuditEntityId(1)),
            sample,
        );
        assert!(event.focused());
    }

    #[test]
    fn authority_loss_keeps_exact_owned_top_slot_observable_as_orphan() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let driver = ViewDriverKey::new("audit-orphan-driver").unwrap();
        app.register_ui_view_driver(AuditDriver(driver.clone()))
            .unwrap();
        let view = RouteViewKey::new("root").unwrap();
        let routes = RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/").unwrap(),
                RouteNodeOptions::new().with_view(view.clone()),
            )
            .unwrap()
            .build()
            .unwrap();
        let mut views = RouteViewRegistry::new();
        views
            .insert(view.clone(), RouteViewDescriptor::new(driver))
            .unwrap();
        let router = app
            .add_ui_router(RouterConfig::new(routes, views, view))
            .unwrap();
        let app_host = app
            .world_mut()
            .spawn((Node::default(), RouterHostRegistration::new(router)))
            .id();
        observe_authority_system(app.world_mut());
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        commit_presentation_system(app.world_mut());
        settle_cleanup_system(app.world_mut());

        let before = app
            .world()
            .resource::<RouterAuthorityAudits>()
            .get(router)
            .unwrap();
        let slot = before.slots()[0].id();
        let slot_entity = before.slots()[0].entity();
        let app_host_id = before.host().unwrap().entity();
        let raw_slot = app.world().resource::<RouterNavigation>().entries[0]
            .host
            .as_ref()
            .unwrap()
            .top_slot;
        let frame = app.world().resource::<RouterNavigation>().entries[0]
            .active
            .as_ref()
            .unwrap()
            .first_frame()
            .unwrap()
            .frame()
            .clone();
        let slot_claim = app
            .world()
            .resource::<RouteEntityRegistry>()
            .record(raw_slot)
            .unwrap()
            .claim();
        app.world_mut()
            .resource_mut::<RouteEntityRegistry>()
            .compare_exchange_owner(
                &slot_claim,
                RegistryOwner::Committed(EntityOwner::FrameOutput {
                    frame,
                    output: u64::MAX,
                    local_index: usize::MAX,
                }),
            )
            .unwrap();
        drive_navigation_system(app.world_mut());

        let after = app
            .world()
            .resource::<RouterAuthorityAudits>()
            .get(router)
            .unwrap();
        assert!(after.host().is_none());
        assert!(after.slots().is_empty());
        assert_eq!(after.orphan_slots(), &[slot]);
        assert_eq!(
            after.owned_entity_state(slot_entity),
            AuditEntityState::PresentOwned
        );
        assert_eq!(
            after.owned_entity_state(app_host_id),
            AuditEntityState::Unknown
        );
        assert!(app.world().get_entity(app_host).is_ok());

        drive_navigation_system(app.world_mut());
        let repeated = app
            .world()
            .resource::<RouterAuthorityAudits>()
            .get(router)
            .unwrap();
        assert_eq!(repeated.orphan_slots(), &[slot]);
        assert_eq!(
            repeated.owned_entity_state(slot_entity),
            AuditEntityState::PresentOwned
        );
    }

    #[test]
    fn child_slot_parent_drift_is_observable_as_a_persistent_orphan() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let driver = ViewDriverKey::new("audit-child-slot-driver").unwrap();
        app.register_ui_view_driver(AuditDriver(driver.clone()))
            .unwrap();
        let view = RouteViewKey::new("root").unwrap();
        let routes = RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/").unwrap(),
                RouteNodeOptions::new().with_view(view.clone()),
            )
            .unwrap()
            .build()
            .unwrap();
        let mut views = RouteViewRegistry::new();
        views
            .insert(view.clone(), RouteViewDescriptor::new(driver))
            .unwrap();
        let router = app
            .add_ui_router(RouterConfig::new(routes, views, view))
            .unwrap();
        let app_host = app
            .world_mut()
            .spawn((Node::default(), RouterHostRegistration::new(router)))
            .id();
        observe_authority_system(app.world_mut());
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        commit_presentation_system(app.world_mut());
        settle_cleanup_system(app.world_mut());

        let (host, slot, frame, child) = {
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            let host = entry.host.as_ref().unwrap();
            let frame = entry.active.as_ref().unwrap();
            (
                host.version.clone(),
                host.slot_id,
                frame.first_frame().unwrap().frame().clone(),
                frame.interactive_entities()[0],
            )
        };
        let child_claim = app
            .world()
            .resource::<RouteEntityRegistry>()
            .record(child)
            .unwrap()
            .claim();
        app.world_mut()
            .resource_mut::<RouteEntityRegistry>()
            .compare_exchange_owner(
                &child_claim,
                RegistryOwner::Committed(EntityOwner::ChildSlot {
                    parent: frame.clone(),
                    outlet: RouteSlotKey::named("drifted-child").unwrap(),
                    slot,
                }),
            )
            .unwrap();
        app.world_mut().entity_mut(child).insert(ChildOf(app_host));
        let top_slot = app.world().resource::<RouterNavigation>().entries[0]
            .host
            .as_ref()
            .unwrap()
            .top_slot;
        let top_claim = app
            .world()
            .resource::<RouteEntityRegistry>()
            .record(top_slot)
            .unwrap()
            .claim();
        assert!(
            app.world_mut()
                .resource_mut::<RouteEntityRegistry>()
                .remove_exact(&top_claim)
        );

        let mut fresh = RouterAuthorityAudits::new();
        fresh.ensure_namespace(router);
        let orphans =
            fresh.collect_orphan_slots(app.world(), router, None, &[], None, &[], Some(&host));
        assert_eq!(orphans.len(), 1);
        assert_eq!(orphans[0].id(), slot);
        assert_eq!(orphans[0].kind(), AuthoritySlotKind::ChildOutlet);
        assert_eq!(orphans[0].parent_frame(), Some(&frame));
        assert!(orphans[0].outlet().is_some());
        assert_eq!(
            observed_owned_state(app.world(), child),
            AuditEntityState::PresentOwned
        );
        assert_eq!(fresh.entity_for(orphans[0].entity()), Some(child));
        assert!(app.world().get_entity(app_host).is_ok());

        let committed_claim = app
            .world()
            .resource::<RouteEntityRegistry>()
            .record(child)
            .unwrap()
            .claim();
        assert!(
            app.world_mut()
                .resource_mut::<RouteEntityRegistry>()
                .remove_exact(&committed_claim)
        );
        let provisional = app.world_mut().spawn_empty().id();
        let provisional_manifest =
            crate::registry::ComponentManifest::from_world(app.world(), provisional).unwrap();
        let provisional_owner =
            RegistryOwner::Provisional(crate::registry::ProvisionalOwner::ChildSlot {
                parent: frame.clone(),
                outlet: RouteSlotKey::named("provisional-child").unwrap(),
                slot,
            });
        {
            let mut registry = app.world_mut().resource_mut::<RouteEntityRegistry>();
            let seal = registry.issue_seal();
            registry
                .register(
                    provisional,
                    provisional_owner,
                    seal,
                    provisional_manifest,
                    EntityLifecycle::PreparedInert,
                )
                .unwrap();
        }
        let mut fresh_provisional = RouterAuthorityAudits::new();
        fresh_provisional.ensure_namespace(router);
        let provisional_orphans = fresh_provisional.collect_orphan_slots(
            app.world(),
            router,
            None,
            &[],
            None,
            &[],
            Some(&host),
        );
        assert_eq!(provisional_orphans.len(), 1);
        assert_eq!(
            provisional_orphans[0].kind(),
            AuthoritySlotKind::ChildOutlet
        );
        assert_eq!(provisional_orphans[0].parent_frame(), Some(&frame));
        assert_eq!(
            fresh_provisional.entity_for(provisional_orphans[0].entity()),
            Some(provisional)
        );

        let (other_app, _branch, _child_entities, _child_slot, _parent_entities) =
            crate::navigation::tests::failed_nested_branch_fixture();
        let other_host = other_app.world().resource::<RouterNavigation>().entries[0]
            .host
            .as_ref()
            .unwrap()
            .version
            .clone();
        let mut cross_router = RouterAuthorityAudits::new();
        cross_router.ensure_namespace(router);
        assert!(
            cross_router
                .collect_orphan_slots(app.world(), router, None, &[], None, &[], Some(&other_host),)
                .iter()
                .all(|slot| slot.kind() != AuthoritySlotKind::ChildOutlet)
        );

        let mut no_host = RouterAuthorityAudits::new();
        no_host.ensure_namespace(router);
        assert!(
            no_host
                .collect_orphan_slots(app.world(), router, None, &[], None, &[], None)
                .is_empty()
        );
    }

    #[test]
    fn reconciliation_audit_derives_removed_from_exact_world_and_registry_absence() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let driver = ViewDriverKey::new("audit-cleanup-driver").unwrap();
        app.register_ui_view_driver(AuditDriver(driver.clone()))
            .unwrap();
        let view = RouteViewKey::new("root").unwrap();
        let routes = RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/").unwrap(),
                RouteNodeOptions::new().with_view(view.clone()),
            )
            .unwrap()
            .build()
            .unwrap();
        let mut views = RouteViewRegistry::new();
        views
            .insert(view.clone(), RouteViewDescriptor::new(driver))
            .unwrap();
        let router = app
            .add_ui_router(RouterConfig::new(routes, views, view))
            .unwrap();
        app.world_mut()
            .spawn((Node::default(), RouterHostRegistration::new(router)));
        observe_authority_system(app.world_mut());

        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        commit_presentation_system(app.world_mut());
        settle_cleanup_system(app.world_mut());
        let removed_root = app
            .world()
            .resource::<RouterAuthorityAudits>()
            .get(router)
            .unwrap()
            .frames()[0]
            .root();
        let removed_entity = app.world().resource::<RouterNavigation>().entries[0]
            .active
            .as_ref()
            .unwrap()
            .first_frame()
            .unwrap()
            .shell();

        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/").with_fragment("replacement"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        commit_presentation_system(app.world_mut());
        settle_cleanup_system(app.world_mut());

        assert!(app.world().get_entity(removed_entity).is_err());
        assert!(
            app.world()
                .resource::<crate::registry::RouteEntityRegistry>()
                .record(removed_entity)
                .is_none()
        );
        let settled = app
            .world()
            .resource::<RouterAuthorityAudits>()
            .get(router)
            .unwrap();
        assert_eq!(
            settled.owned_entity_state(removed_root),
            AuditEntityState::Removed
        );
        assert!(settled.orphan_slots().is_empty());
        let kinds = settled
            .events()
            .iter()
            .map(RouterLifecycleAuditEvent::kind)
            .collect::<Vec<_>>();
        let isolated = kinds
            .iter()
            .position(|kind| *kind == RouterLifecycleAuditEventKind::FrameIsolated)
            .unwrap();
        let activated = kinds
            .iter()
            .rposition(|kind| *kind == RouterLifecycleAuditEventKind::FrameActivated)
            .unwrap();
        assert!(isolated < activated);
    }

    #[test]
    fn audit_bounds_unpinned_removed_tombstones_and_reports_eviction() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let driver = ViewDriverKey::new("audit-retention-driver").unwrap();
        app.register_ui_view_driver(BurstAuditDriver {
            key: driver.clone(),
            calls: 0,
        })
        .unwrap();
        let view = RouteViewKey::new("root").unwrap();
        let routes = RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/").unwrap(),
                RouteNodeOptions::new().with_view(view.clone()),
            )
            .unwrap()
            .build()
            .unwrap();
        let mut views = RouteViewRegistry::new();
        views
            .insert(view.clone(), RouteViewDescriptor::new(driver))
            .unwrap();
        let router = app
            .add_ui_router(RouterConfig::new(routes, views, view))
            .unwrap();
        app.world_mut()
            .spawn((Node::default(), RouterHostRegistration::new(router)));
        observe_authority_system(app.world_mut());

        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        commit_presentation_system(app.world_mut());
        settle_cleanup_system(app.world_mut());
        let first_contents = app
            .world()
            .resource::<RouterAuthorityAudits>()
            .get(router)
            .unwrap()
            .content()
            .iter()
            .map(AuthorityContentMapping::entity)
            .collect::<Vec<_>>();
        let first_content = first_contents[0];

        for replacement in 0..70 {
            app.world_mut()
                .resource_mut::<RouterNavigation>()
                .push(
                    router,
                    RouteLocation::new("/").with_fragment(format!("replacement-{replacement}")),
                )
                .unwrap();
            collect_navigation_system(app.world_mut());
            drive_navigation_system(app.world_mut());
            commit_presentation_system(app.world_mut());
            settle_cleanup_system(app.world_mut());
            if replacement == 0 {
                let pinned = app
                    .world()
                    .resource::<RouterAuthorityAudits>()
                    .get(router)
                    .unwrap();
                assert_eq!(
                    pinned.owned_entity_state(first_content),
                    AuditEntityState::Removed
                );
                assert!(
                    pinned
                        .owned_states
                        .iter()
                        .filter(|(_, state)| *state == AuditEntityState::Removed)
                        .count()
                        > REMOVED_RETENTION
                );
            }
        }

        let audit = app.world().resource::<RouterAuthorityAudits>();
        let snapshot = audit.get(router).unwrap();
        assert_eq!(
            snapshot.owned_entity_state(first_content),
            AuditEntityState::NotRetained
        );
        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        let unpinned_removed = snapshot
            .owned_states
            .iter()
            .filter(|(_, state)| *state == AuditEntityState::Removed)
            .filter(|(id, _)| {
                audit
                    .identities
                    .iter()
                    .find(|item| item.id == *id)
                    .and_then(|item| item.request)
                    .is_none_or(|request| entry.engine.request(request).is_none())
            })
            .count();
        assert_eq!(unpinned_removed, REMOVED_RETENTION);
        let first_retained = first_contents
            .iter()
            .position(|id| snapshot.owned_entity_state(*id) == AuditEntityState::Removed)
            .expect("the newest burst tombstones must reach the 256-entry boundary");
        assert!(first_retained > 0);
        assert!(
            first_contents[..first_retained]
                .iter()
                .all(|id| snapshot.owned_entity_state(*id) == AuditEntityState::NotRetained)
        );
        assert!(
            first_contents[first_retained..]
                .iter()
                .all(|id| snapshot.owned_entity_state(*id) == AuditEntityState::Removed)
        );
        let request_pinned = audit
            .identities
            .iter()
            .filter(|item| {
                item.request
                    .is_some_and(|request| entry.engine.request(request).is_some())
            })
            .count();
        assert!(audit.identities.len() <= REMOVED_RETENTION + request_pinned + 1);
        assert_eq!(
            snapshot.owned_entity_state(snapshot.host().unwrap().entity()),
            AuditEntityState::Unknown
        );
    }

    #[test]
    fn audit_entity_ids_are_process_unique_even_for_equal_entity_bits_in_distinct_worlds() {
        let mut first_world = World::new();
        let mut second_world = World::new();
        let first_entity = first_world.spawn_empty().id();
        let second_entity = second_world.spawn_empty().id();
        assert_eq!(first_entity.to_bits(), second_entity.to_bits());
        let mut first = RouterAuthorityAudits::new();
        let mut second = RouterAuthorityAudits::new();
        let router = RouterHandle::fixture(1, 1);
        let first_namespace = first.ensure_namespace(router);
        second.ensure_namespace(router);

        let first_id = first.id_for(router, first_entity, true, None, None, false, false);
        let second_id = second.id_for(router, second_entity, true, None, None, false, false);
        assert_ne!(first_id, second_id);

        let mut snapshot = RouterAuthorityAuditSnapshot::empty();
        snapshot.namespace = first_namespace;
        snapshot.evicted_owned_through = (first_id.0 & ID_SEQUENCE_MASK) as u32;
        assert_eq!(
            snapshot.owned_entity_state(first_id),
            AuditEntityState::NotRetained
        );
        assert_eq!(
            snapshot.owned_entity_state(second_id),
            AuditEntityState::Unknown
        );
    }

    #[test]
    fn owned_and_observed_audit_ids_for_one_entity_remain_in_distinct_domains() {
        let mut world = World::new();
        let entity = world.spawn_empty().id();
        let router = RouterHandle::fixture(1, 1);
        let mut audits = RouterAuthorityAudits::new();
        let namespace = audits.ensure_namespace(router);

        let owned = audits.id_for(router, entity, true, None, None, false, false);
        let observed = audits.id_for(router, entity, false, None, None, false, false);
        assert_ne!(owned, observed);

        let mut snapshot = RouterAuthorityAuditSnapshot::empty();
        snapshot.namespace = namespace;
        snapshot
            .owned_states
            .push((owned, AuditEntityState::Quarantined));
        assert_eq!(
            snapshot.owned_entity_state(owned),
            AuditEntityState::Quarantined
        );
        assert_eq!(
            snapshot.owned_entity_state(observed),
            AuditEntityState::Unknown
        );
    }

    #[test]
    fn input_probes_resolve_router_scoped_audit_ids_through_the_production_gate() {
        use crate::input::prepare_input_gate_mutation;
        use crate::input_probe::{InputProbeResult, RouterInputModality, RouterInputProbes};

        let mut world = World::new();
        world.insert_resource(RouterInputGate::new());
        let active_entity = world.spawn_empty().id();
        let foreign_entity = world.spawn_empty().id();
        world.resource_scope(|_world, gate: bevy_ecs::prelude::Mut<RouterInputGate>| {
            prepare_input_gate_mutation(gate, &[], vec![active_entity])
                .expect("the production gate accepts one exact active target")
                .apply();
        });

        let router = RouterHandle::fixture(1, 1);
        let foreign_router = RouterHandle::fixture(2, 1);
        let mut audits = RouterAuthorityAudits::new();
        audits.ensure_namespace(router);
        audits.ensure_namespace(foreign_router);
        let active = audits.id_for(router, active_entity, true, None, None, true, false);
        let observed_active = audits.id_for(router, active_entity, false, None, None, false, false);
        let foreign = audits.id_for(router, foreign_entity, false, None, None, false, false);

        let mut probes = RouterInputProbes::new();
        probes.refresh(world.resource::<RouterInputGate>(), &audits);

        for modality in [
            RouterInputModality::Pointer,
            RouterInputModality::Keyboard,
            RouterInputModality::Gamepad,
        ] {
            assert_eq!(
                probes.dispatch(router, active, modality),
                InputProbeResult::Delivered
            );
            assert_eq!(probes.delivery_count(active, modality), 1);
            assert_eq!(
                probes.dispatch(router, observed_active, modality),
                InputProbeResult::Blocked
            );
            assert_eq!(probes.delivery_count(observed_active, modality), 0);
            assert_eq!(
                probes.dispatch(router, foreign, modality),
                InputProbeResult::Blocked
            );
            assert_eq!(probes.delivery_count(foreign, modality), 0);
            assert_eq!(
                probes.dispatch(foreign_router, active, modality),
                InputProbeResult::Unknown
            );
            assert_eq!(probes.delivery_count(active, modality), 1);
        }

        world.resource_scope(|_world, gate: bevy_ecs::prelude::Mut<RouterInputGate>| {
            prepare_input_gate_mutation(gate, &[active_entity], Vec::new())
                .expect("the production gate can revoke the active target")
                .apply();
        });
        assert_eq!(
            probes.dispatch(router, active, RouterInputModality::Pointer),
            InputProbeResult::Blocked,
            "dispatch must observe production-gate revocation without a probe refresh"
        );
        assert_eq!(
            probes.delivery_count(active, RouterInputModality::Pointer),
            1
        );
    }
}
