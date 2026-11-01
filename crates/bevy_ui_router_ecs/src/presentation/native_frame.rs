//! Native content retains its frame owner; ordinary content follows current containment.

use std::collections::{HashMap, HashSet};

use bevy_asset::UntypedHandle;
use bevy_ecs::{
    entity::{Entity, EntityHashSet},
    hierarchy::{ChildOf, Children},
    world::World,
};
use bevy_ui::{Display, Node};
use bevy_ui_router_bsn::DefaultOutlet;
use bevy_ui_router_core::{frame::RouteFrameVersion, slot::RouteSlotKey};

use super::native_scene::{NativeSceneCreationFailure, create_native_scene};
use super::*;
use crate::registry::ProvisionalOwner;

#[derive(Debug)]
pub(crate) struct NativeFrameContent {
    pub(crate) root: Entity,
    pub(crate) focus_owner: Option<crate::native_focus::NativeFocusOwner>,
    // Retain resolve dependencies through apply and unfinished physical settlement.
    #[allow(
        dead_code,
        reason = "Strong asset handles keep resolved dependencies alive while the frame exists."
    )]
    dependencies: Vec<UntypedHandle>,
    cleanup: Option<NativeCleanupPreparation>,
}

#[derive(Debug)]
struct NativeCleanupPreparation {
    /// Frozen containment is validation evidence; the enclosing RealizedFrame retains authority.
    closure: Vec<(Entity, Option<Entity>, Vec<Entity>)>,
    targets: Vec<(Entity, Entity, ComponentManifest)>,
}

#[derive(Clone, Debug)]
pub(crate) enum NativeCleanupPreparationError {
    Capacity {
        #[allow(
            dead_code,
            reason = "Retains native cleanup evidence for error diagnostics and settlement."
        )]
        requested: usize,
    },
    #[allow(
        dead_code,
        reason = "Retains native cleanup evidence for error diagnostics and settlement."
    )]
    Content(crate::frame_local::FrameLocalError),
    #[allow(
        dead_code,
        reason = "Retains native cleanup evidence for error diagnostics and settlement."
    )]
    MissingEntity(Entity),
    Projection {
        #[allow(
            dead_code,
            reason = "Retains native cleanup evidence for error diagnostics and settlement."
        )]
        root: Entity,
        #[allow(
            dead_code,
            reason = "Retains native cleanup evidence for error diagnostics and settlement."
        )]
        expected: Vec<(Entity, Option<Entity>, Vec<Entity>)>,
        #[allow(
            dead_code,
            reason = "Retains native cleanup evidence for error diagnostics and settlement."
        )]
        observed: Vec<(Entity, Option<Entity>, Vec<Entity>)>,
    },
}

impl NativeCleanupPreparationError {
    pub(crate) fn is_retryable(&self) -> bool {
        matches!(self, Self::Capacity { .. })
    }
}

pub(crate) fn is_native_content(world: &World, entity: Entity) -> bool {
    world
        .get::<native_scene::NativeContentRoot>(entity)
        .is_some()
}

pub(crate) fn is_native_shell(world: &World, entity: Entity) -> bool {
    world
        .get::<native_scene::NativeFrameShell>(entity)
        .is_some()
}

/// Observe visual ancestry, not ownership. Only an already authorized publication
/// target may substitute its prepared next values for current staging isolation.
pub(crate) fn authored_target_is_visible(
    world: &World,
    entity: Entity,
    next: &[FrameMutationTarget],
) -> bool {
    AuthoredVisibilityObservation::new(world, next).is_visible(entity)
}

#[derive(Clone, Copy)]
struct VisualAncestry {
    /// Every ancestor permits display and the parent chain is acyclic.
    displayed: bool,
    visible: bool,
}

/// Facts are shared only while World and the prepared next projection are borrowed.
/// They never survive a mutation or become lifecycle/ownership evidence.
pub(super) struct AuthoredVisibilityObservation<'w, 'n> {
    world: &'w World,
    next: &'n [FrameMutationTarget],
    facts: HashMap<Entity, Option<VisualAncestry>>,
    path: Vec<Entity>,
}

impl<'w, 'n> AuthoredVisibilityObservation<'w, 'n> {
    pub(super) fn new(world: &'w World, next: &'n [FrameMutationTarget]) -> Self {
        Self {
            world,
            next,
            facts: HashMap::new(),
            path: Vec::new(),
        }
    }

    pub(super) fn is_visible(&mut self, entity: Entity) -> bool {
        let mut current = entity;
        let mut ancestry = loop {
            if let Some(facts) = self.facts.get(&current) {
                // An unfinished ancestor closes a cycle. Display rejection also
                // propagates through descendants that explicitly set Visible.
                break facts.unwrap_or(VisualAncestry {
                    displayed: false,
                    visible: false,
                });
            }
            self.facts.insert(current, None);
            self.path.push(current);
            let Some(parent) = self.world.get::<ChildOf>(current).map(ChildOf::parent) else {
                break VisualAncestry {
                    displayed: true,
                    visible: true,
                };
            };
            current = parent;
        };
        while let Some(current) = self.path.pop() {
            ancestry = self.local_ancestry(current, ancestry);
            self.facts.insert(current, Some(ancestry));
        }
        let facts = self.facts[&entity].unwrap();
        facts.displayed && facts.visible
    }

    fn local_ancestry(&self, current: Entity, parent: VisualAncestry) -> VisualAncestry {
        let prepared = self.next.iter().find(|target| target.entity == current);
        let display = prepared
            .filter(|target| matches!(target.kind, PresentationTargetKind::NativeShell))
            .map(|target| {
                if target.visible_after {
                    Display::Flex
                } else {
                    Display::None
                }
            })
            .or_else(|| self.world.get::<Node>(current).map(|node| node.display));
        let visibility = prepared
            .filter(|target| !matches!(target.kind, PresentationTargetKind::NativeContent))
            .map(|target| {
                if target.visible_after {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                }
            })
            .or_else(|| self.world.get::<Visibility>(current).copied());
        VisualAncestry {
            displayed: display != Some(Display::None) && parent.displayed,
            visible: match visibility {
                Some(Visibility::Inherited) => parent.visible,
                Some(Visibility::Hidden) => false,
                Some(Visibility::Visible) | None => true,
            },
        }
    }
}

#[cfg(test)]
#[derive(bevy_ecs::resource::Resource)]
pub(crate) struct NativeCleanupCapacityRequest {
    pub(crate) additional_targets: usize,
}

fn cleanup_capacity_input(world: &World, target_count: usize) -> usize {
    #[cfg(test)]
    if let Some(request) = world.get_resource::<NativeCleanupCapacityRequest>() {
        return request.additional_targets;
    }
    let _ = world;
    target_count
}

fn content_structure_is_exact(world: &World, root: Entity, shell: Entity) -> bool {
    let Some(registry) = world.get_resource::<RouteEntityRegistry>() else {
        return false;
    };
    let table = world.get_resource::<QuarantineLeaseTable>();
    let record = |entity| {
        registry.record(entity).or_else(|| {
            table.and_then(|table| {
                table
                    .record(entity)
                    .or_else(|| table.unresolved_record(entity))
            })
        })
    };
    let Some(root_record) = record(root) else {
        return false;
    };
    let Some(shell_record) = record(shell) else {
        return false;
    };
    content_structure_matches(world, &root_record.claim(), &shell_record.claim())
}

fn content_structure_matches(
    world: &World,
    root_claim: &ExactRegistryClaim,
    shell_claim: &ExactRegistryClaim,
) -> bool {
    let root = root_claim.entity();
    let shell = shell_claim.entity();
    let RegistryOwner::Committed(EntityOwner::FrameContentTarget(frame)) = root_claim.owner()
    else {
        return false;
    };
    shell_claim.owner() == &RegistryOwner::Committed(EntityOwner::FrameShell(frame.clone()))
        && world.get::<FrameContentTarget>(root) == Some(&FrameContentTarget(frame.clone()))
        && world.get::<FrameShell>(shell) == Some(&FrameShell(frame.clone()))
        && world.get::<RouteOwnershipSeal>(root)
            == Some(&RouteOwnershipSeal::new(root_claim.seal()))
        && world.get::<RouteOwnershipSeal>(shell)
            == Some(&RouteOwnershipSeal::new(shell_claim.seal()))
        && world.get::<FrameLifecycle>(root) == Some(&FrameLifecycle(root_claim.lifecycle()))
        && world.get::<FrameLifecycle>(shell) == Some(&FrameLifecycle(shell_claim.lifecycle()))
        && is_native_content(world, root)
        && is_native_shell(world, shell)
        && world.get::<ChildOf>(root).map(ChildOf::parent) == Some(shell)
        && world
            .get::<Children>(shell)
            .is_some_and(|children| children.len() == 1 && children[0] == root)
}

/// Current relationships are facts, not authority. The caller retains the original
/// frame owner and validates its exact source claims before authorizing mutation.
/// Any registered structure (including a child slot) terminates ordinary containment.
pub(crate) fn current_containment(
    world: &World,
    target: Entity,
    root: Entity,
    shell: Entity,
) -> bool {
    let Some(registry) = world.get_resource::<RouteEntityRegistry>() else {
        return false;
    };
    let table = world.get_resource::<QuarantineLeaseTable>();
    let mut current = target;
    let mut visited = HashSet::new();
    while visited.insert(current) {
        if current == root {
            return content_structure_is_exact(world, root, shell);
        }
        if world.get_entity(current).is_err()
            || registry.record(current).is_some()
            || table.is_some_and(|table| {
                table.record(current).is_some() || table.unresolved_record(current).is_some()
            })
        {
            return false;
        }
        let Some(parent) = world.get::<ChildOf>(current).map(ChildOf::parent) else {
            return false;
        };
        if world
            .get::<Children>(parent)
            .is_none_or(|children| !children.contains(&current))
        {
            return false;
        }
        current = parent;
    }
    false
}

pub(crate) fn content_scope_is_inert_for(
    world: &World,
    root_claim: &ExactRegistryClaim,
    shell_claim: &ExactRegistryClaim,
) -> bool {
    let shell = shell_claim.entity();
    content_structure_matches(world, root_claim, shell_claim)
        && world
            .get::<Node>(shell)
            .is_some_and(|node| node.display == Display::None)
        && world.get::<Visibility>(shell) == Some(&Visibility::Hidden)
        && world.get::<Pickable>(shell) == Some(&Pickable::IGNORE)
        && world
            .get::<RouterInputBarrier>(shell)
            .is_some_and(RouterInputBarrier::denies_all)
}

pub(crate) fn content_scope_is_inert(world: &World, root: Entity) -> bool {
    let Some(shell) = world.get::<ChildOf>(root).map(ChildOf::parent) else {
        return false;
    };
    content_structure_is_exact(world, root, shell)
        && world
            .get::<Node>(shell)
            .is_some_and(|node| node.display == Display::None)
        && world.get::<Visibility>(shell) == Some(&Visibility::Hidden)
        && world.get::<Pickable>(shell) == Some(&Pickable::IGNORE)
        && world
            .get::<RouterInputBarrier>(shell)
            .is_some_and(RouterInputBarrier::denies_all)
}

pub(super) fn isolate_native_frame(
    world: &mut World,
    frame: &RealizedFrame,
    required: &impl Fn(&ExactRegistryClaim) -> bool,
    protected: &impl Fn(&World) -> bool,
) -> bool {
    if !protected(world)
        || !frame
            .entities
            .iter()
            .filter(|entity| required(&entity.claim))
            .all(|entity| frame_claim_is_current(world, entity))
    {
        return false;
    }
    if !required(&frame.entities[0].claim) {
        return frame
            .entities
            .iter()
            .filter(|entity| required(&entity.claim))
            .all(|entity| content_scope_is_inert(world, entity.entity));
    }
    let shell = frame.shell();
    let Some(mut node) = world.get_mut::<Node>(shell) else {
        return false;
    };
    node.display = Display::None;
    if let Some(mut visibility) = world.get_mut::<Visibility>(shell) {
        *visibility = Visibility::Hidden;
    } else {
        return false;
    }
    if let Some(mut pickable) = world.get_mut::<Pickable>(shell) {
        *pickable = Pickable::IGNORE;
    } else {
        return false;
    }
    if let Some(mut barrier) = world.get_mut::<RouterInputBarrier>(shell) {
        *barrier = RouterInputBarrier::deny_all();
    } else {
        return false;
    }
    protected(world)
        && frame
            .entities
            .iter()
            .filter(|entity| required(&entity.claim))
            .all(|entity| entity.entity == shell || content_scope_is_inert(world, entity.entity))
}

pub(crate) enum NativeFrameFailure {
    Mount(DeferredRealizationFailure),
    Creation(NativeSceneCreationFailure),
}

impl NativeFrameFailure {
    pub(crate) fn before_mount(
        error: RealizationError,
        prepared: crate::bsn::PreparedBsnScene,
    ) -> Self {
        Self::Creation(native_scene::failure_without_allocations(
            native_scene::NativeSceneCreationError::Mount(error),
            prepared.dependencies,
        ))
    }
}

/// One immutable observation of managed content and its cut child-slot boundaries.
/// The root's exact structural claim remains the authority; these are facts only.
pub(crate) struct NativeContentObservation<'w> {
    pub(crate) root: Entity,
    pub(crate) managed: Vec<Entity>,
    pub(crate) child_slots: Vec<Entity>,
    pub(crate) membership: EntityHashSet,
    _world: &'w World,
}

pub(crate) fn collect_content(
    world: &World,
    frame: &RouteFrameVersion,
    root: Entity,
) -> Result<Vec<Entity>, crate::frame_local::FrameLocalError> {
    observe_content(world, frame, root).map(|observation| observation.managed)
}

pub(crate) fn observe_content<'w>(
    world: &'w World,
    frame: &RouteFrameVersion,
    root: Entity,
) -> Result<NativeContentObservation<'w>, crate::frame_local::FrameLocalError> {
    use crate::frame_local::FrameLocalError;
    let registry = world.resource::<RouteEntityRegistry>();
    let root_record = registry
        .record(root)
        .ok_or(FrameLocalError::MissingRoot(root))?;
    if !matches!(root_record.owner(), RegistryOwner::Committed(EntityOwner::FrameContentTarget(owner)) if owner == frame)
        || world.get::<FrameContentTarget>(root) != Some(&FrameContentTarget(frame.clone()))
        || world.get::<RouteOwnershipSeal>(root)
            != Some(&RouteOwnershipSeal::new(root_record.seal()))
        || world.get::<native_scene::NativeContentRoot>(root).is_none()
    {
        return Err(FrameLocalError::OwnershipDrift(root));
    }
    let mut seen = EntityHashSet::default();
    let mut pending = vec![root];
    let mut content = Vec::new();
    let mut child_slots = Vec::new();
    while let Some(entity) = pending.pop() {
        if !seen.insert(entity) {
            return Err(FrameLocalError::RelationshipCycle(entity));
        }
        if entity != root
            && let Some(record) = registry.record(entity)
        {
            if matches!(record.owner(), RegistryOwner::Committed(EntityOwner::ChildSlot { parent, .. }) | RegistryOwner::Provisional(ProvisionalOwner::ChildSlot { parent, .. }) if parent == frame)
                && world.get::<RouteOwnershipSeal>(entity)
                    == Some(&RouteOwnershipSeal::new(record.seal()))
                && crate::cleanup::cleanup_claim_has_exact_authority(world, &record.claim())
            {
                continue;
            }
            return Err(FrameLocalError::OwnershipDrift(entity));
        }
        if world.get_entity(entity).is_err() {
            return Err(FrameLocalError::OwnershipDrift(entity));
        }
        if entity != root
            && registry.record(entity).is_none()
            && world
                .get_resource::<QuarantineLeaseTable>()
                .is_some_and(|table| {
                    table
                        .record(entity)
                        .or_else(|| table.unresolved_record(entity))
                        .is_some_and(|record| {
                            world.get::<RouteOwnershipSeal>(entity)
                                == Some(&RouteOwnershipSeal::new(record.seal()))
                        })
                })
        {
            return Err(FrameLocalError::OwnershipDrift(entity));
        }
        content.push(entity);
        if let Some(children) = world.get::<Children>(entity) {
            for child in children.iter().copied() {
                if world.get::<ChildOf>(child).map(ChildOf::parent) != Some(entity) {
                    return Err(FrameLocalError::OwnershipDrift(child));
                }
                // Preserve the physical enumeration order: managed parent order,
                // then its authored children. The later visit proves each cut.
                if registry.record(child).is_some() {
                    child_slots.push(child);
                }
            }
            pending.extend(children.iter().rev().copied());
        }
    }
    for slot in &child_slots {
        seen.remove(slot);
    }
    Ok(NativeContentObservation {
        root,
        managed: content,
        child_slots,
        membership: seen,
        _world: world,
    })
}

pub(crate) fn realize_native_root(
    world: &mut World,
    host_candidate: ValidatedHostCandidate,
    top_slot: Entity,
    top_slot_claim: ExactRegistryClaim,
    frame: RouteFrameVersion,
    prepared: crate::bsn::PreparedBsnScene,
) -> Result<RealizedFrame, NativeFrameFailure> {
    let mount = FrameMountBinding::host_top(top_slot, top_slot_claim, host_candidate.host());
    realize_native_with_mount(world, host_candidate, mount, frame, prepared)
}

pub(crate) fn realize_native_reconciliation_child(
    world: &mut World,
    mount: &crate::branch_transaction::SealedReconciliationStagingMount<'_>,
    task: &FrameTask,
    prepared: crate::bsn::PreparedBsnScene,
) -> Result<RealizedFrame, NativeFrameFailure> {
    if !matches!(task.mount_target(), FrameMountTarget::ChildOutlet { parent, outlet } if parent == *mount.parent().frame() && outlet == *mount.slot().outlet())
        || task.frame().slot() != mount.slot().slot_id()
    {
        return Err(NativeFrameFailure::before_mount(
            RealizationError::InvalidMount,
            prepared,
        ));
    }
    realize_native_with_mount(
        world,
        mount.parent().host_candidate.clone(),
        FrameMountBinding::from_reconciliation_staging_mount(mount),
        task.frame().clone(),
        prepared,
    )
}

fn realize_native_with_mount(
    world: &mut World,
    host_candidate: ValidatedHostCandidate,
    mount: FrameMountBinding,
    frame: RouteFrameVersion,
    prepared: crate::bsn::PreparedBsnScene,
) -> Result<RealizedFrame, NativeFrameFailure> {
    let mount_ready = validate_mount_binding(world, &host_candidate, &mount, &frame);
    if let Err(error) = mount_ready {
        return Err(NativeFrameFailure::before_mount(error, prepared));
    }
    let prior_mount_children = world
        .get::<Children>(mount.slot)
        .map(|children| children.iter().copied().collect::<Vec<_>>())
        .unwrap_or_default();
    let mount_manifest = world
        .resource::<RouteEntityRegistry>()
        .record(mount.slot)
        .unwrap()
        .manifest()
        .clone();
    let adoption =
        create_native_scene(world, frame, prepared).map_err(NativeFrameFailure::Creation)?;
    let (frame, shell, root, dependencies) = adoption.into_parts();
    let shell_entity = shell.entity();
    let root_entity = root.entity();
    let mut result = RealizedFrame {
        frame,
        host_candidate,
        mount,
        prior_mount_children,
        shell: shell_entity,
        mount_stage: FrameMountStage::ShellAllocated,
        entities: vec![
            RealizedEntity {
                entity: shell_entity,
                claim: shell,
                role: RealizedRole::Shell,
                parent: Entity::PLACEHOLDER,
                solid_rect: None,
            },
            RealizedEntity {
                entity: root_entity,
                claim: root,
                role: RealizedRole::ContentTarget,
                parent: shell_entity,
                solid_rect: None,
            },
        ],
        outlets: Vec::new(),
        prewrite_complete: true,
        native: Some(NativeFrameContent {
            root: root_entity,
            focus_owner: None,
            dependencies,
            cleanup: None,
        }),
    };
    result.entities[0].parent = result.mount.slot;
    if validate_mount_binding(world, &result.host_candidate, &result.mount, result.frame()).is_err()
    {
        return Err(NativeFrameFailure::Mount(
            DeferredRealizationFailure::with_residual(RealizationError::PostWriteDrift, result),
        ));
    }
    let shell_manifest = world
        .resource::<RouteEntityRegistry>()
        .record(shell_entity)
        .unwrap()
        .manifest()
        .clone();
    world
        .entity_mut(shell_entity)
        .insert(ChildOf(result.mount.slot));
    world.flush();
    result.mount_stage = FrameMountStage::ShellAttached;
    let relationship_ids = [
        world.register_component::<ChildOf>(),
        world.register_component::<Children>(),
    ];
    let shell_sync = synchronize_manifest_allowing_from_expected(
        world,
        shell_entity,
        &result.entities[0].claim,
        &shell_manifest,
        &relationship_ids,
    );
    let mount_sync = synchronize_manifest_allowing_from_expected(
        world,
        result.mount.slot,
        result.mount.slot_claim(),
        &mount_manifest,
        &relationship_ids,
    );
    let valid = shell_sync.is_ok()
        && mount_sync.is_ok()
        && mount_slot_child_order_extended_exactly(
            world,
            result.mount.slot,
            &result.prior_mount_children,
            shell_entity,
        );
    if !valid {
        return Err(NativeFrameFailure::Mount(
            DeferredRealizationFailure::with_residual(RealizationError::PostWriteDrift, result),
        ));
    }
    let content = collect_content(world, result.frame(), root_entity).map_err(|_| ());
    let Ok(content) = content else {
        return Err(NativeFrameFailure::Mount(
            DeferredRealizationFailure::with_residual(RealizationError::PostWriteDrift, result),
        ));
    };
    for anchor in content
        .into_iter()
        .filter(|entity| world.get::<DefaultOutlet>(*entity).is_some())
    {
        result.outlets.push(RealizedOutlet {
            parent: result.frame.clone(),
            key: RouteSlotKey::default(),
            anchor,
            anchor_binding: OutletBinding::NativeContent {
                anchor,
                root: result.entities[1].claim.clone(),
            },
        });
    }
    Ok(result)
}

pub(crate) fn finish_focus(
    world: &mut World,
    owner: crate::native_focus::NativeFocusOwner,
    result: Result<RealizedFrame, NativeFrameFailure>,
) -> Result<RealizedFrame, NativeFrameFailure> {
    match result {
        Ok(mut frame) => {
            let root = frame
                .native_root()
                .expect("native realization keeps its root");
            let source = crate::native_focus::NativeFocusSource(owner.source.clone());
            frame.native.as_mut().unwrap().focus_owner = Some(owner);
            world.entity_mut(root).insert(source);
            world.flush();
            let source_id = world.register_component::<crate::native_focus::NativeFocusSource>();
            let sync = synchronize_manifest_allowing_additions(
                world,
                root,
                &frame.entities[1].claim,
                &[source_id],
            );
            let focus = crate::native_focus::observe(world, &[]);
            if let Err(error) = focus {
                return Err(NativeFrameFailure::Mount(
                    DeferredRealizationFailure::with_residual(
                        RealizationError::NativeFocus(error),
                        frame,
                    ),
                ));
            }
            if sync.is_err() {
                return Err(NativeFrameFailure::Mount(
                    DeferredRealizationFailure::with_residual(
                        RealizationError::PostWriteDrift,
                        frame,
                    ),
                ));
            }
            Ok(frame)
        }
        Err(NativeFrameFailure::Creation(mut failure)) => {
            let targets = failure.focus_entities();
            if let Err(error) = crate::native_focus::observe(world, &targets) {
                failure.set_focus_error(error);
            }
            Err(NativeFrameFailure::Creation(failure))
        }
        Err(NativeFrameFailure::Mount(failure)) => {
            let (error, residual) = failure.into_parts();
            let targets = residual
                .as_ref()
                .map(|frame| frame.live_interactive_entities(world))
                .unwrap_or_default();
            let error = crate::native_focus::observe(world, &targets)
                .err()
                .map(RealizationError::NativeFocus)
                .unwrap_or(error);
            Err(NativeFrameFailure::Mount(DeferredRealizationFailure {
                error,
                residual,
            }))
        }
    }
}

/// Called only for a frame retained by an accepted leaving/failed role owner.
/// Only relationship evidence and capacity are prepared for ordinary content.
/// The frame retains the frozen targets and remains the sole lifecycle owner.
pub(crate) fn prepare_content_cleanup(
    world: &mut World,
    frame: &mut RealizedFrame,
) -> Result<(), NativeCleanupPreparationError> {
    let Some(root) = frame.native_root() else {
        return Ok(());
    };
    if world.get_entity(root).is_err() {
        // An absent content boundary has no current descendants to acquire.
        // Keep every original/partially prepared obligation and its evidence:
        // the authority cleanup ledger must still classify absent structures
        // and any surviving ordinary targets captured before the source disappeared.
        return Ok(());
    }
    let content = collect_content(world, frame.frame(), root)
        .map_err(NativeCleanupPreparationError::Content)?;
    if frame.native.as_ref().unwrap().cleanup.is_none() {
        let requested = cleanup_capacity_input(world, content.len().saturating_sub(1));
        let mut targets = Vec::new();
        targets
            .try_reserve_exact(requested)
            .map_err(|_| NativeCleanupPreparationError::Capacity { requested })?;
        for entity in content.iter().copied().skip(1) {
            let parent = world
                .get::<ChildOf>(entity)
                .map(ChildOf::parent)
                .ok_or(NativeCleanupPreparationError::MissingEntity(entity))?;
            let manifest = ComponentManifest::from_world(world, entity)
                .ok_or(NativeCleanupPreparationError::MissingEntity(entity))?;
            targets.push((entity, parent, manifest));
        }
        let closure = content
            .iter()
            .copied()
            .map(|entity| {
                (
                    entity,
                    world.get::<ChildOf>(entity).map(ChildOf::parent),
                    world
                        .get::<Children>(entity)
                        .map(|children| children.iter().copied().collect())
                        .unwrap_or_default(),
                )
            })
            .collect::<Vec<_>>();
        frame.native.as_mut().unwrap().cleanup =
            Some(NativeCleanupPreparation { targets, closure });
    }
    if !cleanup_projection_is_current(world, frame) {
        return Err(cleanup_projection_error(world, frame));
    }
    // Root authored component changes are legitimate until this cleanup freeze.
    let record = world
        .resource::<RouteEntityRegistry>()
        .record(root)
        .ok_or(NativeCleanupPreparationError::MissingEntity(root))?;
    let old = record.manifest().clone();
    let claim = record.claim();
    let actual = ComponentManifest::from_world(world, root)
        .ok_or(NativeCleanupPreparationError::MissingEntity(root))?;
    if !world
        .resource_mut::<RouteEntityRegistry>()
        .compare_exchange_manifest_exact(&claim, &old, actual)
    {
        return Err(cleanup_projection_error(world, frame));
    }
    Ok(())
}

pub(crate) fn contained_cleanup_targets(
    frame: &RealizedFrame,
) -> Vec<(Entity, Entity, ComponentManifest)> {
    frame
        .native
        .as_ref()
        .and_then(|native| native.cleanup.as_ref())
        .map(|cleanup| cleanup.targets.clone())
        .unwrap_or_default()
}

fn cleanup_projection_error(world: &World, frame: &RealizedFrame) -> NativeCleanupPreparationError {
    let native = frame
        .native
        .as_ref()
        .expect("cleanup projection belongs to native frame");
    let expected = native
        .cleanup
        .as_ref()
        .expect("preparation retains source projection")
        .closure
        .clone();
    let observed = expected
        .iter()
        .map(|(entity, _, _)| {
            (
                *entity,
                world.get::<ChildOf>(*entity).map(ChildOf::parent),
                world
                    .get::<Children>(*entity)
                    .map(|children| children.iter().copied().collect())
                    .unwrap_or_default(),
            )
        })
        .collect();
    NativeCleanupPreparationError::Projection {
        root: native.root,
        expected,
        observed,
    }
}

fn cleanup_projection_is_current(world: &World, frame: &RealizedFrame) -> bool {
    let Some(native) = &frame.native else {
        return false;
    };
    let Some(cleanup) = &native.cleanup else {
        return false;
    };
    let Ok(content) = collect_content(world, frame.frame(), native.root) else {
        return false;
    };
    content
        .iter()
        .copied()
        .eq(cleanup.closure.iter().map(|(entity, _, _)| *entity))
        && cleanup.closure.iter().all(|(entity, parent, children)| {
            world.get::<ChildOf>(*entity).map(ChildOf::parent) == *parent
                && world
                    .get::<Children>(*entity)
                    .map(|actual| actual.iter().copied().collect::<Vec<_>>())
                    .unwrap_or_default()
                    == *children
        })
        && frame.entities.iter().all(|entity| {
            world
                .resource::<RouteEntityRegistry>()
                .record(entity.entity)
                .is_some_and(|record| record.claim() == entity.claim)
        })
}

pub(super) fn validate_native_values(
    world: &World,
    frame: &RealizedFrame,
    lifecycle: EntityLifecycle,
) -> bool {
    observe_native_values(world, frame, lifecycle).is_some()
}

pub(super) fn observe_native_values<'w>(
    world: &'w World,
    frame: &RealizedFrame,
    lifecycle: EntityLifecycle,
) -> Option<NativeContentObservation<'w>> {
    let root = frame.native_root()?;
    let registry = world.resource::<RouteEntityRegistry>();
    let valid =
        frame.entities.iter().all(|entity| {
            registry.record(entity.entity).is_some_and(|record| {
                record.claim() == entity.claim && record.lifecycle() == lifecycle
            }) && world.get::<RouteOwnershipSeal>(entity.entity)
                == Some(&RouteOwnershipSeal::new(entity.claim.seal()))
                && world.get::<FrameLifecycle>(entity.entity) == Some(&FrameLifecycle(lifecycle))
                && world.get::<ChildOf>(entity.entity).map(ChildOf::parent) == Some(entity.parent)
        }) && shell_visual_values_are_current(world, frame.shell(), frame.frame(), lifecycle)
            && world.get::<FrameShell>(frame.shell()) == Some(&FrameShell(frame.frame.clone()))
            && world.get::<Pickable>(frame.shell()) == Some(&Pickable::IGNORE)
            && world
                .get::<RouterInputBarrier>(frame.shell())
                .is_some_and(RouterInputBarrier::denies_all)
            && world
                .get::<Children>(frame.shell())
                .is_some_and(|children| children.len() == 1 && children[0] == root);
    if !valid {
        return None;
    }
    observe_content(world, frame.frame(), root).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::*;
    use bevy_app::{App, TaskPoolPlugin};
    use bevy_asset::AssetPlugin;
    use bevy_ecs::prelude::*;
    use bevy_scene::{ScenePlugin, bsn};
    use bevy_ui_router_core::prelude::*;
    use bevy_ui_router_view::prelude::*;

    fn active_native_branch(
        nested: bool,
    ) -> (App, Entity, crate::branch_transaction::RealizedBranch) {
        let mut app = App::new();
        app.add_plugins((
            TaskPoolPlugin::default(),
            AssetPlugin::default(),
            ScenePlugin,
        ));
        let driver = app
            .register_bsn_view(ViewDriverKey::new("cleanup-native").unwrap(), |_| {
                Ok(bsn! { Node { width: Val::Px(101.0) } Children [
                    (Node { width: Val::Px(21.0) }),
                    (Node { width: Val::Px(22.0) })
                ] })
            })
            .unwrap();
        let view = RouteViewKey::new("native").unwrap();
        let mut views = RouteViewRegistry::new();
        views
            .insert(view.clone(), RouteViewDescriptor::new(driver))
            .unwrap();
        let root_view = if nested {
            let driver = app
                .register_bsn_view(ViewDriverKey::new("observation-layout").unwrap(), |_| {
                    Ok(bsn! { Node DefaultOutlet })
                })
                .unwrap();
            let key = RouteViewKey::new("layout").unwrap();
            views
                .insert(key.clone(), RouteViewDescriptor::new(driver))
                .unwrap();
            key
        } else {
            view.clone()
        };
        let mut routes = RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/").unwrap(),
                RouteNodeOptions::new().with_view(root_view),
            )
            .unwrap();
        if nested {
            routes
                .route_with_options(
                    routes.root_id().unwrap(),
                    RoutePattern::path("child").unwrap(),
                    RouteNodeOptions::new()
                        .with_view(view.clone())
                        .with_slot(RouteSlotKey::default()),
                )
                .unwrap();
        }
        let router = app
            .add_ui_router(RouterConfig::new(routes.build().unwrap(), views, view))
            .unwrap();
        let host = app
            .world_mut()
            .spawn((Node::default(), RouterHostRegistration::new(router)))
            .id();
        app.update();
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(
                router,
                RouteLocation::new(if nested { "/child" } else { "/" }),
            )
            .unwrap();
        for _ in 0..12 {
            app.update();
        }
        assert!(
            matches!(app.world().resource::<RouterSnapshots>().request(router, request), RequestLookup::Found(request) if request.status() == &RequestStatus::Succeeded)
        );
        let branch = app.world_mut().resource_mut::<RouterNavigation>().entries[0]
            .active
            .take()
            .unwrap();
        (app, host, branch)
    }

    fn active_native_frame() -> (App, Entity, RealizedFrame) {
        let (app, host, branch) = active_native_branch(false);
        let frame = branch.into_single_frame().ok().flatten().unwrap();
        (app, host, frame)
    }

    #[test]
    fn active_native_observation_preserves_boundaries_and_rechecks_dynamic_membership() {
        let (mut app, _, branch) = active_native_branch(true);
        assert_eq!(branch.frames().len(), 2);
        let parent = &branch.frames()[0];
        let child = &branch.frames()[1];
        let slot = branch.child_slots()[0].slot();
        let dynamic = app
            .world_mut()
            .spawn((Node::default(), ChildOf(child.native_root().unwrap())))
            .id();
        let application = app.world_mut().spawn_empty().id();

        let focus = crate::frame_local::validate_active_branch_with_entering(
            app.world(),
            &branch,
            None,
            None,
        )
        .unwrap();
        assert!(focus.contains(&parent.shell()));
        assert!(focus.contains(&slot));
        assert!(focus.contains(&child.shell()));
        assert!(focus.contains(&dynamic));
        let observed = crate::frame_local::observe_active_branch_with_entering(
            app.world(),
            &branch,
            None,
            None,
        )
        .unwrap();
        assert!(!observed.frames()[0].contains_managed_content(slot));
        assert!(!observed.frames()[0].contains_managed_content(child.shell()));
        assert!(!observed.frames()[0].contains_managed_content(dynamic));
        assert!(observed.frames()[1].contains_managed_content(dynamic));
        for (frame, observed) in branch.frames().iter().zip(observed.frames()) {
            let expected =
                collect_content(app.world(), frame.frame(), frame.native_root().unwrap()).unwrap();
            assert_eq!(observed.managed_content_entities(), expected.as_slice());
        }
        drop(observed);

        app.world_mut()
            .entity_mut(dynamic)
            .insert(ChildOf(application));
        let observed = crate::frame_local::observe_active_branch_with_entering(
            app.world(),
            &branch,
            None,
            None,
        )
        .unwrap();
        assert!(!observed.frames()[1].contains_managed_content(dynamic));
        drop(observed);

        let slot_sibling = app.world_mut().spawn(ChildOf(slot)).id();
        assert!(
            crate::frame_local::validate_active_branch_with_entering(
                app.world(),
                &branch,
                None,
                None,
            )
            .is_err()
        );
        assert!(
            crate::frame_local::observe_active_branch_with_entering(
                app.world(),
                &branch,
                None,
                None,
            )
            .is_err()
        );
        assert!(app.world_mut().despawn(slot_sibling));
        assert!(
            crate::frame_local::validate_active_branch_with_entering(
                app.world(),
                &branch,
                None,
                None,
            )
            .is_ok()
        );
        assert!(
            crate::frame_local::observe_active_branch_with_entering(
                app.world(),
                &branch,
                None,
                None,
            )
            .is_ok()
        );

        app.world_mut().spawn(ChildOf(parent.shell()));
        assert!(
            crate::frame_local::validate_active_branch_with_entering(
                app.world(),
                &branch,
                None,
                None,
            )
            .is_err()
        );
        assert!(
            crate::frame_local::observe_active_branch_with_entering(
                app.world(),
                &branch,
                None,
                None,
            )
            .is_err()
        );
    }

    #[test]
    fn absent_native_root_cleanup_keeps_claims_without_physical_writes() {
        for despawn_host in [false, true] {
            let (mut app, host, mut frame) = active_native_frame();
            let root = frame.native_root().unwrap();
            let claims = frame.claims().cloned().collect::<Vec<_>>();
            assert!(
                app.world_mut()
                    .despawn(if despawn_host { host } else { root })
            );
            let entities = app
                .world()
                .iter_entities()
                .map(|entity| (entity.id(), entity.archetype().components().to_vec()))
                .collect::<Vec<_>>();
            app.world_mut().clear_trackers();
            let last_run = app.world().last_change_tick();

            prepare_content_cleanup(app.world_mut(), &mut frame).unwrap();

            assert_eq!(frame.claims().cloned().collect::<Vec<_>>(), claims);
            assert!(frame.native.as_ref().unwrap().cleanup.is_none());
            assert_eq!(
                app.world()
                    .iter_entities()
                    .map(|entity| { (entity.id(), entity.archetype().components().to_vec()) })
                    .collect::<Vec<_>>(),
                entities
            );
            for (entity, components) in entities {
                let entity = app.world().entity(entity);
                for component in components {
                    let ticks = entity.get_change_ticks_by_id(component).unwrap();
                    assert!(!ticks.is_added(last_run, app.world().read_change_tick()));
                    assert!(!ticks.is_changed(last_run, app.world().read_change_tick()));
                }
            }
            for claim in claims {
                assert_eq!(
                    app.world()
                        .resource::<RouteEntityRegistry>()
                        .record(claim.entity())
                        .unwrap()
                        .claim(),
                    claim
                );
            }
        }
    }

    #[test]
    fn absent_native_root_retains_captured_targets_and_original_owner() {
        for despawn_host in [false, true] {
            let (mut app, host, mut frame) = active_native_frame();
            let root = frame.native_root().unwrap();
            prepare_content_cleanup(app.world_mut(), &mut frame).unwrap();
            let claims = frame.claims().cloned().collect::<Vec<_>>();
            let native = frame.native.as_ref().unwrap();
            let closure = native.cleanup.as_ref().unwrap().closure.clone();
            let focus = native
                .focus_owner
                .as_ref()
                .map(|owner| owner.source.clone());
            let targets = contained_cleanup_targets(&frame);
            assert_eq!(targets.len(), 2);
            let escaped = targets[0].0;
            let authored = app.world().get::<Node>(escaped).unwrap().clone();
            let application = app.world_mut().spawn(Node::default()).id();
            app.world_mut()
                .entity_mut(escaped)
                .insert(ChildOf(application));
            app.world_mut()
                .despawn(if despawn_host { host } else { root });

            prepare_content_cleanup(app.world_mut(), &mut frame).unwrap();

            assert_eq!(frame.claims().cloned().collect::<Vec<_>>(), claims);
            let native = frame.native.as_ref().unwrap();
            assert_eq!(native.cleanup.as_ref().unwrap().closure, closure);
            assert_eq!(
                native
                    .focus_owner
                    .as_ref()
                    .map(|owner| owner.source.clone()),
                focus
            );
            assert_eq!(app.world().get::<Node>(escaped), Some(&authored));
            assert_eq!(
                app.world().get::<ChildOf>(escaped).map(ChildOf::parent),
                Some(application)
            );
            assert!(
                app.world()
                    .resource::<RouteEntityRegistry>()
                    .record(escaped)
                    .is_none()
            );
            assert!(app.world().get::<RouteOwnershipSeal>(escaped).is_none());
            assert!(!current_containment(
                app.world(),
                escaped,
                root,
                frame.shell()
            ));
            assert_eq!(contained_cleanup_targets(&frame), targets);
            assert_eq!(
                frame.claims().count(),
                2,
                "ordinary content never acquired a claim"
            );
        }
    }

    #[test]
    fn live_native_root_seal_drift_remains_a_permanent_cleanup_failure() {
        let (mut app, _, mut frame) = active_native_frame();
        let root = frame.native_root().unwrap();
        let claims = frame.claims().cloned().collect::<Vec<_>>();
        app.world_mut()
            .entity_mut(root)
            .remove::<RouteOwnershipSeal>();
        let error = prepare_content_cleanup(app.world_mut(), &mut frame).unwrap_err();
        assert!(
            matches!(error, NativeCleanupPreparationError::Content(crate::frame_local::FrameLocalError::OwnershipDrift(entity)) if entity == root)
        );
        assert!(!error.is_retryable());
        assert_eq!(frame.claims().cloned().collect::<Vec<_>>(), claims);
    }

    #[test]
    fn cleanup_capacity_retry_retains_original_owner_until_complete_capture() {
        let (mut app, _, mut frame) = active_native_frame();
        let original_frame = frame.frame().clone();
        let claims = frame.claims().cloned().collect::<Vec<_>>();
        app.insert_resource(NativeCleanupCapacityRequest {
            additional_targets: usize::MAX,
        });
        let error = prepare_content_cleanup(app.world_mut(), &mut frame).unwrap_err();
        assert!(matches!(
            error,
            NativeCleanupPreparationError::Capacity {
                requested: usize::MAX
            }
        ));
        assert!(error.is_retryable());
        assert!(frame.native.as_ref().unwrap().cleanup.is_none());
        assert!(contained_cleanup_targets(&frame).is_empty());
        assert_eq!(frame.claims().cloned().collect::<Vec<_>>(), claims);
        app.world_mut()
            .remove_resource::<NativeCleanupCapacityRequest>();
        prepare_content_cleanup(app.world_mut(), &mut frame).unwrap();
        let targets = contained_cleanup_targets(&frame);
        assert_eq!(targets.len(), 2);
        assert_eq!(frame.frame(), &original_frame);
        assert_eq!(frame.claims().cloned().collect::<Vec<_>>(), claims);
        for (entity, _, _) in targets {
            assert!(
                app.world()
                    .resource::<RouteEntityRegistry>()
                    .record(entity)
                    .is_none()
            );
            assert!(app.world().get::<RouteOwnershipSeal>(entity).is_none());
            assert!(app.world().get::<FrameLifecycle>(entity).is_none());
        }
        assert_eq!(
            app.world()
                .get::<Node>(frame.native_root().unwrap())
                .unwrap()
                .width,
            Val::Px(101.0)
        );
    }

    #[test]
    fn cleanup_excludes_content_moved_out_before_capture() {
        let (mut app, _, mut frame) = active_native_frame();
        let root = frame.native_root().unwrap();
        let children = app.world().get::<Children>(root).unwrap().to_vec();
        let application = app.world_mut().spawn(Node::default()).id();
        app.world_mut()
            .entity_mut(children[0])
            .insert(ChildOf(application));
        prepare_content_cleanup(app.world_mut(), &mut frame).unwrap();
        let targets = contained_cleanup_targets(&frame);
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].0, children[1]);
        assert_eq!(
            app.world().get::<ChildOf>(children[0]).map(ChildOf::parent),
            Some(application)
        );
        assert!(
            app.world()
                .resource::<RouteEntityRegistry>()
                .record(children[0])
                .is_none()
        );
        assert_eq!(frame.claims().count(), 2);
    }

    #[test]
    fn public_replace_cleans_unregistered_contained_targets() {
        let (mut app, _, frame) = active_native_frame();
        let root = frame.native_root().unwrap();
        let mut targets = app.world().get::<Children>(root).unwrap().to_vec();
        let dynamic = app.world_mut().spawn((Node::default(), ChildOf(root))).id();
        let grandchild = app
            .world_mut()
            .spawn((Node::default(), ChildOf(dynamic)))
            .id();
        targets.extend([dynamic, grandchild]);
        let router = app.world().resource::<RouterNavigation>().entries[0].handle;
        app.world_mut().resource_mut::<RouterNavigation>().entries[0].active =
            Some(crate::branch_transaction::RealizedBranch::from_single_frame(frame));
        for entity in &targets {
            assert!(
                app.world()
                    .resource::<RouteEntityRegistry>()
                    .record(*entity)
                    .is_none()
            );
            assert!(app.world().get::<FrameLifecycle>(*entity).is_none());
        }
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .replace(router, RouteLocation::new("/").with_fragment("cleanup"))
            .unwrap();
        for _ in 0..12 {
            app.update();
        }
        assert!(
            matches!(app.world().resource::<RouterSnapshots>().request(router, request),
            RequestLookup::Found(request) if request.status() == &RequestStatus::Succeeded)
        );
        assert!(app.world().get_entity(root).is_err());
        for entity in targets {
            assert!(app.world().get_entity(entity).is_err());
        }
    }

    #[derive(Component, Clone, Copy)]
    struct EscapeOnDespawn {
        escaped: Entity,
        application: Entity,
    }

    #[test]
    fn public_replace_despawn_escape_gets_terminal_receipt_and_keeps_app_focus() {
        use bevy_input_focus::{FocusCause, InputFocus};
        let (mut app, _, frame) = active_native_frame();
        let root = frame.native_root().unwrap();
        let mut children = app.world().get::<Children>(root).unwrap().to_vec();
        children.sort_by_key(|entity| entity.to_bits());
        let trigger = children[0];
        let escaped = children[1];
        let application = app.world_mut().spawn(Node::default()).id();
        app.init_resource::<InputFocus>();
        app.world_mut()
            .register_component_hooks::<EscapeOnDespawn>()
            .on_despawn(|mut world, context| {
                let EscapeOnDespawn {
                    escaped,
                    application,
                } = *world.get::<EscapeOnDespawn>(context.entity).unwrap();
                world.commands().queue(move |world: &mut World| {
                    world.entity_mut(escaped).insert(ChildOf(application));
                    world
                        .resource_mut::<InputFocus>()
                        .set(escaped, FocusCause::Navigated);
                });
            });
        app.world_mut().entity_mut(trigger).insert(EscapeOnDespawn {
            escaped,
            application,
        });
        let router = app.world().resource::<RouterNavigation>().entries[0].handle;
        app.world_mut().resource_mut::<RouterNavigation>().entries[0].active =
            Some(crate::branch_transaction::RealizedBranch::from_single_frame(frame));
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .replace(
                router,
                RouteLocation::new("/").with_fragment("cleanup-escape"),
            )
            .unwrap();
        for _ in 0..12 {
            app.update();
        }
        assert!(
            matches!(app.world().resource::<RouterSnapshots>().request(router, request),
            RequestLookup::Found(request) if request.status() == &RequestStatus::Succeeded)
        );
        let snapshots = app.world().resource::<RouterSnapshots>();
        assert!(snapshots.get(router).unwrap().running_request().is_none());
        assert!(
            !snapshots
                .get(router)
                .unwrap()
                .cleanup()
                .foreground_pending()
        );
        assert!(
            app.world().resource::<RouterNavigation>().entries[0]
                .cleanup_ledger
                .is_none()
        );
        assert!(app.world().get_entity(trigger).is_err());
        assert_eq!(
            app.world().get::<ChildOf>(escaped).map(ChildOf::parent),
            Some(application)
        );
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(escaped));
        assert!(
            app.world()
                .resource::<RouteEntityRegistry>()
                .record(escaped)
                .is_none()
        );
        assert!(app.world().get::<FrameLifecycle>(escaped).is_none());
        assert!(
            app.world()
                .resource::<QuarantineLeaseTable>()
                .authority_lost_tombstone(escaped)
                .is_some()
        );
        for _ in 0..3 {
            app.update();
        }
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(escaped));
    }

    #[derive(Component, Clone, Copy)]
    struct ProtectedOnDespawn {
        target: Entity,
        parent: Entity,
        action: u8,
    }
    #[derive(Component)]
    struct UnrelatedApplicationData;

    fn cleanup_protected_hook_case(action: u8) {
        let (mut app, host, frame) = active_native_frame();
        let root = frame.native_root().unwrap();
        let mut children = app.world().get::<Children>(root).unwrap().to_vec();
        children.sort_by_key(|entity| entity.to_bits());
        let trigger = children[0];
        let remaining = children[1];
        let application = app.world_mut().spawn(Node::default()).id();
        let protected = app.world_mut().spawn((Node::default(), ChildOf(host))).id();
        let target = if action == 1 { host } else { protected };
        app.world_mut()
            .register_component_hooks::<ProtectedOnDespawn>()
            .on_despawn(|mut world, context| {
                let fact = *world.get::<ProtectedOnDespawn>(context.entity).unwrap();
                world
                    .commands()
                    .queue(move |world: &mut World| match fact.action {
                        0 => {
                            world.entity_mut(fact.target).insert(ChildOf(fact.parent));
                        }
                        1 => {
                            world
                                .entity_mut(fact.target)
                                .remove::<RouterHostRegistration>();
                        }
                        2 => {
                            world
                                .entity_mut(fact.target)
                                .insert(UnrelatedApplicationData);
                            world.get_mut::<Node>(fact.target).unwrap().width = Val::Px(404.0);
                        }
                        _ => unreachable!(),
                    });
            });
        app.world_mut()
            .entity_mut(trigger)
            .insert(ProtectedOnDespawn {
                target,
                parent: application,
                action,
            });
        let router = app.world().resource::<RouterNavigation>().entries[0].handle;
        app.world_mut().resource_mut::<RouterNavigation>().entries[0].active =
            Some(crate::branch_transaction::RealizedBranch::from_single_frame(frame));
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .replace(
                router,
                RouteLocation::new("/").with_fragment("cleanup-protected"),
            )
            .unwrap();
        for _ in 0..12 {
            app.update();
        }
        assert!(
            app.world().get_entity(trigger).is_err(),
            "real despawn hook ran"
        );
        let snapshots = app.world().resource::<RouterSnapshots>();
        let RequestLookup::Found(request) = snapshots.request(router, request) else {
            panic!("request missing");
        };
        if action != 1 {
            assert_eq!(request.status(), &RequestStatus::Succeeded);
            assert!(app.world().get_entity(remaining).is_err());
            assert!(
                app.world().resource::<RouterNavigation>().entries[0]
                    .authority_protocol_residual
                    .is_none()
            );
            if action == 0 {
                assert_eq!(
                    app.world().get::<ChildOf>(protected).map(ChildOf::parent),
                    Some(application)
                );
            } else {
                assert_eq!(
                    app.world().get::<Node>(protected).unwrap().width,
                    Val::Px(404.0)
                );
                assert!(
                    app.world()
                        .get::<UnrelatedApplicationData>(protected)
                        .is_some()
                );
            }
        } else {
            // Publication already succeeded; protected drift retires authority without
            // rewriting the original navigation result.
            assert_eq!(request.status(), &RequestStatus::Succeeded);
            assert!(
                app.world().get_entity(remaining).is_ok(),
                "no next destructive write after protected/source drift"
            );
            let navigation = app.world().resource::<RouterNavigation>();
            let residual = navigation.entries[0]
                .authority_protocol_residual
                .as_ref()
                .unwrap();
            let ledger = residual.retained_cleanup_settlement_ledger().unwrap();
            assert!(ledger.is_stopped());
            assert!(!ledger.issued_attempt_present_for_test());
            assert_eq!(
                ledger.receipt_count(),
                1,
                "the real first removal receipt is retained"
            );
            assert_eq!(ledger.obligation_entities_for_test().len(), 4);
            for target in [root, trigger, remaining] {
                assert!(ledger.obligation_entities_for_test().contains(&target));
            }
            assert_eq!(ledger.leaving_frames().len(), 1);
            assert_eq!(ledger.leaving_frames()[0].native_root(), Some(root));
        }
        assert!(snapshots.get(router).unwrap().running_request().is_none());
        assert!(
            !snapshots
                .get(router)
                .unwrap()
                .cleanup()
                .foreground_pending()
        );
        assert!(
            app.world().resource::<RouterNavigation>().entries[0]
                .cleanup_ledger
                .is_none()
        );
        assert!(app.world().get_entity(host).is_ok());
        assert!(app.world().get_entity(protected).is_ok());
    }

    #[test]
    fn cleanup_unrelated_app_relationship_change_remains_legal() {
        cleanup_protected_hook_case(0);
    }
    #[test]
    fn cleanup_host_registration_drift_stops_before_next_removal_and_terminates() {
        cleanup_protected_hook_case(1);
    }
    #[test]
    fn cleanup_unrelated_protected_component_change_remains_legal() {
        cleanup_protected_hook_case(2);
    }

    #[derive(Component, Clone, Copy)]
    struct ChangeContainedContentOnDespawn {
        root: Entity,
        target: Entity,
        reorder: bool,
    }

    #[derive(Resource, Default)]
    struct ContainedContentCallbackObservation {
        calls: usize,
        changed: bool,
    }

    fn cleanup_legal_contained_content_callback(reorder: bool) {
        let (mut app, host, frame) = active_native_frame();
        let root = frame.native_root().unwrap();
        app.world_mut().spawn((Node::default(), ChildOf(root)));
        let mut children = app.world().get::<Children>(root).unwrap().to_vec();
        children.sort_by_key(|entity| entity.to_bits());
        let trigger = children[0];
        let target = children[1];
        app.init_resource::<ContainedContentCallbackObservation>();
        app.world_mut()
            .register_component_hooks::<ChangeContainedContentOnDespawn>()
            .on_despawn(|mut world, context| {
                let fact = *world
                    .get::<ChangeContainedContentOnDespawn>(context.entity)
                    .unwrap();
                world.commands().queue(move |world: &mut World| {
                    assert!(world.get_entity(fact.target).is_ok());
                    let changed = if fact.reorder {
                        let before = world.get::<Children>(fact.root).unwrap().to_vec();
                        assert!(
                            before.len() >= 2,
                            "reorder must affect two remaining live siblings"
                        );
                        world
                            .get_mut::<Children>(fact.root)
                            .unwrap()
                            .sort_by_key(|entity| {
                                std::cmp::Reverse(
                                    before.iter().position(|old| old == entity).unwrap(),
                                )
                            });
                        let after = world.get::<Children>(fact.root).unwrap().to_vec();
                        assert_ne!(before, after);
                        for child in &after {
                            assert_eq!(
                                world.get::<ChildOf>(*child).map(ChildOf::parent),
                                Some(fact.root)
                            );
                        }
                        true
                    } else {
                        world
                            .entity_mut(fact.target)
                            .insert(UnrelatedApplicationData);
                        world.get_mut::<Node>(fact.target).unwrap().width = Val::Px(527.0);
                        world.get::<UnrelatedApplicationData>(fact.target).is_some()
                            && world.get::<Node>(fact.target).unwrap().width == Val::Px(527.0)
                    };
                    let mut observation =
                        world.resource_mut::<ContainedContentCallbackObservation>();
                    observation.calls += 1;
                    observation.changed = changed;
                });
            });
        app.world_mut()
            .entity_mut(trigger)
            .insert(ChangeContainedContentOnDespawn {
                root,
                target,
                reorder,
            });
        let router = app.world().resource::<RouterNavigation>().entries[0].handle;
        app.world_mut().resource_mut::<RouterNavigation>().entries[0].active =
            Some(crate::branch_transaction::RealizedBranch::from_single_frame(frame));
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .replace(
                router,
                RouteLocation::new("/").with_fragment("legal-content-callback"),
            )
            .unwrap();
        for _ in 0..16 {
            app.update();
        }
        let observation = app
            .world()
            .resource::<ContainedContentCallbackObservation>();
        assert_eq!(observation.calls, 1);
        assert!(
            observation.changed,
            "the real callback must change a remaining target's observed facts"
        );
        for child in children {
            assert!(app.world().get_entity(child).is_err());
        }
        assert!(app.world().get_entity(root).is_err());
        assert!(app.world().get_entity(host).is_ok());
        let snapshots = app.world().resource::<RouterSnapshots>();
        assert!(
            matches!(snapshots.request(router, request), RequestLookup::Found(request) if request.status() == &RequestStatus::Succeeded)
        );
        let snapshot = snapshots.get(router).unwrap();
        assert!(snapshot.running_request().is_none());
        assert!(!snapshot.cleanup().foreground_pending());
        assert_eq!(snapshot.cleanup().quarantine_count(), 0);
    }

    #[test]
    fn cleanup_contained_target_component_and_style_change_remains_legal() {
        cleanup_legal_contained_content_callback(false);
    }

    #[test]
    fn cleanup_contained_sibling_order_change_remains_legal() {
        cleanup_legal_contained_content_callback(true);
    }
}
