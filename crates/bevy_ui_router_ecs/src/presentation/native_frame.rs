//! Native content retains its frame owner; ordinary content follows current containment.

use std::collections::HashSet;

use bevy_asset::UntypedHandle;
use bevy_ecs::{
    entity::Entity,
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
    dependencies: Vec<UntypedHandle>,
    cleanup: Option<NativeCleanupPreparation>,
    cleanup_failure: Option<NativeCleanupPreparationError>,
}

#[derive(Debug)]
struct NativeCleanupPreparation {
    /// Frozen containment is validation evidence; the enclosing RealizedFrame retains authority.
    closure: Vec<(Entity, Option<Entity>, Vec<Entity>)>,
    next: usize,
}

#[derive(Clone, Debug)]
pub(crate) enum NativeCleanupPreparationError {
    Capacity {
        requested: usize,
    },
    Content(crate::frame_local::FrameLocalError),
    MissingEntity(Entity),
    Registry {
        entity: Entity,
        error: crate::registry::RegistryWriteError,
    },
    Protected(Box<NativeCleanupProtectedDrift>),
    Projection {
        root: Entity,
        expected: Vec<(Entity, Option<Entity>, Vec<Entity>)>,
        observed: Vec<(Entity, Option<Entity>, Vec<Entity>)>,
    },
}

#[derive(Clone, Debug)]
pub(crate) struct NativeCleanupProtectedDrift {
    entity: Entity,
    expected: crate::cleanup::FrozenFailureCleanupObservedEntity,
    observed: crate::cleanup::FrozenFailureCleanupObservedEntity,
    expected_node: Option<Node>,
    observed_node: Option<Node>,
}

impl NativeCleanupPreparationError {
    pub(crate) fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::Capacity { .. }
                | Self::Registry {
                    error: crate::registry::RegistryWriteError::CapacityExhausted,
                    ..
                }
        )
    }
}

#[derive(bevy_ecs::component::Component, Clone, Debug)]
pub(crate) struct NativeCleanupContent {
    pub(crate) root: Entity,
    pub(crate) shell: Entity,
    pub(crate) expected_parent: Entity,
    identity: ExactRegistryClaim,
}

pub(crate) fn is_native_content(world: &World, entity: Entity) -> bool {
    world
        .get::<native_scene::NativeContentRoot>(entity)
        .is_some()
        || world.get::<NativeCleanupContent>(entity).is_some()
}

pub(crate) fn is_native_shell(world: &World, entity: Entity) -> bool {
    world
        .get::<native_scene::NativeFrameShell>(entity)
        .is_some()
}

pub(crate) fn is_escaped_cleanup_content(world: &World, entity: Entity) -> bool {
    world
        .get::<NativeCleanupContent>(entity)
        .is_some_and(|content| {
            world
                .resource::<RouteEntityRegistry>()
                .record(entity)
                .or_else(|| {
                    world
                        .get_resource::<QuarantineLeaseTable>()
                        .and_then(|table| {
                            table
                                .record(entity)
                                .or_else(|| table.unresolved_record(entity))
                        })
                })
                .is_none_or(|record| {
                    record.owner() == content.identity.owner()
                        && record.seal() == content.identity.seal()
                })
                && !cleanup_content_is_contained(world, entity)
        })
}

/// Observe visual ancestry, not ownership. Only an already authorized publication
/// target may substitute its prepared next values for current staging isolation.
pub(crate) fn authored_target_is_visible(
    world: &World,
    entity: Entity,
    next: &[FrameMutationTarget],
) -> bool {
    let mut current = entity;
    let mut seen = HashSet::new();
    let mut visible = None;
    while seen.insert(current) {
        let prepared = next.iter().find(|target| target.entity == current);
        let display = prepared
            .filter(|target| matches!(target.kind, PresentationTargetKind::NativeShell))
            .map(|target| {
                if target.active_after {
                    Display::Flex
                } else {
                    Display::None
                }
            })
            .or_else(|| world.get::<Node>(current).map(|node| node.display));
        if display == Some(Display::None) {
            return false;
        }
        if visible.is_none() {
            let visibility = prepared
                .filter(|target| !matches!(target.kind, PresentationTargetKind::NativeContent))
                .map(|target| {
                    if target.active_after {
                        Visibility::Visible
                    } else {
                        Visibility::Hidden
                    }
                })
                .or_else(|| world.get::<Visibility>(current).copied());
            visible = match visibility {
                Some(Visibility::Inherited) => None,
                Some(Visibility::Hidden) => Some(false),
                Some(Visibility::Visible) | None => Some(true),
            };
        }
        let Some(parent) = world.get::<ChildOf>(current).map(ChildOf::parent) else {
            return visible.unwrap_or(true);
        };
        current = parent;
    }
    false
}

#[cfg(test)]
#[derive(bevy_ecs::resource::Resource)]
pub(crate) struct NativeCleanupCapacityLimit {
    pub(crate) after_claims: usize,
}

fn cleanup_registration_capacity(world: &World, index: usize) -> usize {
    #[cfg(test)]
    if world
        .get_resource::<NativeCleanupCapacityLimit>()
        .is_some_and(|limit| index > limit.after_claims)
    {
        return usize::MAX;
    }
    let _ = (world, index);
    1
}

/// Surrender only the cleanup bookkeeping issued for this exact escaped child.
/// The retired seal is never reused and no authored component or relationship is changed.
pub(crate) fn release_escaped_cleanup_content(world: &mut World) {
    let escaped = world
        .query::<(Entity, &NativeCleanupContent)>()
        .iter(world)
        .filter(|(entity, _)| !cleanup_content_is_contained(world, *entity))
        .map(|(entity, content)| (entity, content.identity.clone()))
        .collect::<Vec<_>>();
    for (entity, issued) in escaped {
        let current = world
            .resource::<RouteEntityRegistry>()
            .record(entity)
            .filter(|record| record.owner() == issued.owner() && record.seal() == issued.seal())
            .map(|record| record.claim());
        let Some(current) = current else {
            continue;
        };
        let markers_are_exact = world.get::<RouteOwnershipSeal>(entity)
            == Some(&RouteOwnershipSeal::new(current.seal()))
            && world.get::<FrameLifecycle>(entity) == Some(&FrameLifecycle(current.lifecycle()));
        if world
            .resource_mut::<RouteEntityRegistry>()
            .remove_exact(&current)
        {
            if markers_are_exact && let Ok(mut entity) = world.get_entity_mut(entity) {
                entity.remove::<(RouteOwnershipSeal, FrameLifecycle)>();
                // Retain non-authoritative provenance while the original cleanup
                // owner still contains this surrendered obligation. All input and
                // cleanup consumers exclude it outside the original content root.
            }
            world.flush();
        }
    }
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
    let RegistryOwner::Committed(EntityOwner::FrameContentTarget(frame)) = root_record.owner()
    else {
        return false;
    };
    shell_record.owner() == &RegistryOwner::Committed(EntityOwner::FrameShell(frame.clone()))
        && world.get::<FrameContentTarget>(root) == Some(&FrameContentTarget(frame.clone()))
        && world.get::<FrameShell>(shell) == Some(&FrameShell(frame.clone()))
        && world.get::<RouteOwnershipSeal>(root)
            == Some(&RouteOwnershipSeal::new(root_record.seal()))
        && world.get::<RouteOwnershipSeal>(shell)
            == Some(&RouteOwnershipSeal::new(shell_record.seal()))
        && world.get::<FrameLifecycle>(root) == Some(&FrameLifecycle(root_record.lifecycle()))
        && world.get::<FrameLifecycle>(shell) == Some(&FrameLifecycle(shell_record.lifecycle()))
        && world.get::<native_scene::NativeContentRoot>(root).is_some()
        && is_native_shell(world, shell)
        && world.get::<ChildOf>(root).map(ChildOf::parent) == Some(shell)
        && world
            .get::<Children>(shell)
            .is_some_and(|children| children.len() == 1 && children[0] == root)
}

/// A cleanup marker is bookkeeping outside its original live content boundary.
/// It does not turn an escaped ordinary child into a permanent Router target.
pub(crate) fn cleanup_content_is_contained(world: &World, entity: Entity) -> bool {
    let Some(content) = world.get::<NativeCleanupContent>(entity) else {
        return false;
    };
    let mut current = entity;
    let mut visited = HashSet::new();
    while visited.insert(current) {
        if current == content.root {
            return content_structure_is_exact(world, content.root, content.shell);
        }
        if current != entity
            && world
                .resource::<RouteEntityRegistry>()
                .record(current)
                .is_some_and(|record| {
                    !matches!(
                        record.owner(),
                        RegistryOwner::Committed(EntityOwner::FrameOutput { .. })
                    ) || world
                        .get::<NativeCleanupContent>(current)
                        .is_none_or(|parent| parent.root != content.root)
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

pub(crate) fn content_scope_is_inert(world: &World, entity: Entity) -> bool {
    let shell = if world
        .get::<native_scene::NativeContentRoot>(entity)
        .is_some()
    {
        world.get::<ChildOf>(entity).map(ChildOf::parent)
    } else {
        world
            .get::<NativeCleanupContent>(entity)
            .filter(|_| cleanup_content_is_contained(world, entity))
            .map(|content| content.shell)
    };
    shell.is_some_and(|shell| {
        let root = world
            .get::<NativeCleanupContent>(entity)
            .map_or(entity, |content| content.root);
        content_structure_is_exact(world, root, shell)
            && world
                .get::<Node>(shell)
                .is_some_and(|node| node.display == Display::None)
            && world.get::<Visibility>(shell) == Some(&Visibility::Hidden)
            && world.get::<Pickable>(shell) == Some(&Pickable::IGNORE)
            && world
                .get::<RouterInputBarrier>(shell)
                .is_some_and(RouterInputBarrier::denies_all)
    })
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

/// Current frame-local content, excluding all child slots and their frames.
/// The root's exact structural claim is the authority; returned entities are facts only.
pub(crate) fn collect_content(
    world: &World,
    frame: &RouteFrameVersion,
    root: Entity,
) -> Result<Vec<Entity>, crate::frame_local::FrameLocalError> {
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
    let mut seen = HashSet::new();
    let mut pending = vec![root];
    let mut content = Vec::new();
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
            let cleanup_content = world
                .get::<NativeCleanupContent>(entity)
                .is_some_and(|content| content.root == root)
                && matches!(record.owner(), RegistryOwner::Committed(EntityOwner::FrameOutput { frame: owner, .. }) if owner == frame)
                && world.get::<RouteOwnershipSeal>(entity)
                    == Some(&RouteOwnershipSeal::new(record.seal()));
            if !cleanup_content {
                return Err(FrameLocalError::OwnershipDrift(entity));
            }
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
                                && !(world.get::<NativeCleanupContent>(entity).is_some()
                                    && !cleanup_content_is_contained(world, entity))
                        })
                })
        {
            return Err(FrameLocalError::OwnershipDrift(entity));
        }
        content.push(entity);
        let children = world
            .get::<Children>(entity)
            .map(|children| children.iter().copied().collect::<Vec<_>>())
            .unwrap_or_default();
        for child in &children {
            if world.get::<ChildOf>(*child).map(ChildOf::parent) != Some(entity) {
                return Err(FrameLocalError::OwnershipDrift(*child));
            }
        }
        pending.extend(children.into_iter().rev());
    }
    Ok(content)
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

pub(crate) fn realize_native_child(
    world: &mut World,
    parent: &RealizedFrame,
    mounted: &MountedChildSlot,
    task: &FrameTask,
    prepared: crate::bsn::PreparedBsnScene,
) -> Result<RealizedFrame, NativeFrameFailure> {
    if !matches!(task.mount_target(), FrameMountTarget::ChildOutlet { parent: task_parent, outlet } if task_parent == *parent.frame() && outlet == *mounted.outlet())
        || parent.frame() != mounted.parent()
        || task.frame().slot() != mounted.slot_id()
        || parent.outlet(mounted.outlet()).is_none_or(|outlet| {
            outlet.anchor() != mounted.anchor() || outlet.anchor_claim() != mounted.anchor_claim()
        })
    {
        return Err(NativeFrameFailure::before_mount(
            RealizationError::InvalidMount,
            prepared,
        ));
    }
    realize_native_with_mount(
        world,
        parent.host_candidate.clone(),
        FrameMountBinding::from_mounted_child_slot(mounted),
        task.frame().clone(),
        prepared,
    )
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
    let runtime_components = runtime_ui_component_ids(world);
    let mount_ready = validate_mount_binding_allowing_additions(
        world,
        &host_candidate,
        &mount,
        &frame,
        &runtime_components,
    )
    .and_then(|()| {
        synchronize_manifest_allowing_additions(
            world,
            mount.slot(),
            mount.slot_claim(),
            &runtime_components,
        )
    })
    .and_then(|()| validate_mount_binding(world, &host_candidate, &mount, &frame));
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
            cleanup_failure: None,
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
        runtime_components[0],
        runtime_components[1],
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
/// Fresh claims narrow that existing owner's current content into the shared cleanup ledger.
/// The frame stores partial progress before the first hook-triggering write.
pub(crate) fn prepare_content_cleanup(
    world: &mut World,
    frame: &mut RealizedFrame,
) -> Result<(), NativeCleanupPreparationError> {
    let result = prepare_content_cleanup_inner(world, frame);
    if let Err(error) = &result {
        if let Some(native) = frame.native.as_mut() {
            // The owner retains the current observation even if a downstream
            // residual receiver must return it for capacity. This is not the
            // separately held original Scene failure or an error history.
            native.cleanup_failure = Some(error.clone());
        }
        release_escaped_cleanup_content(world);
    }
    result
}

fn prepare_content_cleanup_inner(
    world: &mut World,
    frame: &mut RealizedFrame,
) -> Result<(), NativeCleanupPreparationError> {
    let Some(root) = frame.native_root() else {
        return Ok(());
    };
    let content = collect_content(world, frame.frame(), root)
        .map_err(NativeCleanupPreparationError::Content)?;
    if frame.native.as_ref().unwrap().cleanup.is_none() {
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
        frame
            .entities
            .try_reserve(content.len().saturating_sub(1))
            .map_err(|_| NativeCleanupPreparationError::Capacity {
                requested: content.len(),
            })?;
        frame.native.as_mut().unwrap().cleanup =
            Some(NativeCleanupPreparation { closure, next: 1 });
    }
    if !cleanup_projection_is_current(world, frame) {
        return Err(cleanup_projection_error(world, frame));
    }
    // This is an operation-local preservation proof, not a second content owner.
    // Each write is checked before any next write. A retry retains the original
    // target projection and claims while observing the current protected closure.
    let protected = freeze_cleanup_protected(world, frame, &content)?;
    let lifecycle = frame.entities[0].claim.lifecycle();
    loop {
        let state = frame.native.as_ref().unwrap().cleanup.as_ref().unwrap();
        if state.next >= state.closure.len() {
            break;
        }
        let index = state.next;
        let (entity, parent, _) = &state.closure[index];
        let entity = *entity;
        let parent = parent.ok_or(NativeCleanupPreparationError::MissingEntity(entity))?;
        let requested = cleanup_registration_capacity(world, index);
        frame
            .entities
            .try_reserve(requested)
            .map_err(|_| NativeCleanupPreparationError::Capacity { requested })?;
        let owner = RegistryOwner::Committed(EntityOwner::FrameOutput {
            frame: frame.frame.clone(),
            output: u64::MAX,
            local_index: index,
        });
        let manifest = ComponentManifest::from_world(world, entity)
            .ok_or(NativeCleanupPreparationError::MissingEntity(entity))?;
        let claim = {
            let mut registry = world.resource_mut::<RouteEntityRegistry>();
            registry
                .try_reserve_registrations(&[owner.clone()])
                .map_err(|error| NativeCleanupPreparationError::Registry { entity, error })?;
            let seal = registry.issue_seal();
            registry
                .register(entity, owner, seal, manifest.clone(), lifecycle)
                .map_err(|error| NativeCleanupPreparationError::Registry { entity, error })?;
            registry.record(entity).unwrap().claim()
        };
        frame.entities.push(RealizedEntity {
            entity,
            claim: claim.clone(),
            role: RealizedRole::Empty,
            parent,
            solid_rect: None,
        });
        frame
            .native
            .as_mut()
            .unwrap()
            .cleanup
            .as_mut()
            .unwrap()
            .next += 1;
        world.entity_mut(entity).insert((
            RouteOwnershipSeal::new(claim.seal()),
            FrameLifecycle(lifecycle),
            NativeCleanupContent {
                root,
                shell: frame.shell(),
                expected_parent: parent,
                identity: claim.clone(),
            },
        ));
        world.flush();
        let actual = ComponentManifest::from_world(world, entity)
            .ok_or(NativeCleanupPreparationError::MissingEntity(entity))?;
        validate_cleanup_protected(world, &protected)?;
        if !world
            .resource_mut::<RouteEntityRegistry>()
            .compare_exchange_manifest_exact(&claim, &manifest, actual)
            || !cleanup_projection_is_current(world, frame)
        {
            return Err(cleanup_projection_error(world, frame));
        }
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

struct CleanupProtectedEntity {
    entity: Entity,
    observed: crate::cleanup::FrozenFailureCleanupObservedEntity,
    node: Option<Node>,
}

fn freeze_cleanup_protected(
    world: &World,
    frame: &RealizedFrame,
    targets: &[Entity],
) -> Result<Vec<CleanupProtectedEntity>, NativeCleanupPreparationError> {
    let mut ancestor = frame.shell();
    let mut ancestors = HashSet::new();
    while let Some(parent) = world.get::<ChildOf>(ancestor).map(ChildOf::parent) {
        if !ancestors.insert(ancestor)
            || world
                .get::<Children>(parent)
                .is_none_or(|children| !children.contains(&ancestor))
        {
            return Err(NativeCleanupPreparationError::Content(
                crate::frame_local::FrameLocalError::OwnershipDrift(ancestor),
            ));
        }
        ancestor = parent;
    }
    let mut visited = HashSet::new();
    let mut pending = vec![ancestor];
    let mut protected = Vec::new();
    while let Some(entity) = pending.pop() {
        if !visited.insert(entity) {
            return Err(NativeCleanupPreparationError::Content(
                crate::frame_local::FrameLocalError::RelationshipCycle(entity),
            ));
        }
        for child in world
            .get::<Children>(entity)
            .into_iter()
            .flat_map(|children| children.iter().copied())
        {
            if world.get::<ChildOf>(child).map(ChildOf::parent) != Some(entity) {
                return Err(NativeCleanupPreparationError::Content(
                    crate::frame_local::FrameLocalError::OwnershipDrift(child),
                ));
            }
            pending.push(child);
        }
        if targets.contains(&entity) {
            continue;
        }
        protected
            .try_reserve(1)
            .map_err(|_| NativeCleanupPreparationError::Capacity { requested: 1 })?;
        protected.push(CleanupProtectedEntity {
            entity,
            observed: crate::cleanup::FrozenFailureCleanupObservedEntity::observe(
                world,
                world.resource::<RouteEntityRegistry>(),
                entity,
            ),
            node: world.get::<Node>(entity).cloned(),
        });
    }
    Ok(protected)
}

fn validate_cleanup_protected(
    world: &World,
    protected: &[CleanupProtectedEntity],
) -> Result<(), NativeCleanupPreparationError> {
    for expected in protected {
        let node = world.get::<Node>(expected.entity).cloned();
        if !expected
            .observed
            .matches_exact(world, world.resource::<RouteEntityRegistry>())
            || node != expected.node
        {
            return Err(NativeCleanupPreparationError::Protected(Box::new(
                NativeCleanupProtectedDrift {
                    entity: expected.entity,
                    expected: expected.observed.clone(),
                    observed: crate::cleanup::FrozenFailureCleanupObservedEntity::observe(
                        world,
                        world.resource::<RouteEntityRegistry>(),
                        expected.entity,
                    ),
                    expected_node: expected.node.clone(),
                    observed_node: node,
                },
            )));
        }
    }
    Ok(())
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
    let Some(root) = frame.native_root() else {
        return false;
    };
    let active = lifecycle == EntityLifecycle::Active;
    let expected_node = Node {
        display: if active { Display::Flex } else { Display::None },
        width: Val::Percent(100.0),
        height: Val::Percent(100.0),
        ..Node::default()
    };
    let registry = world.resource::<RouteEntityRegistry>();
    frame.entities.iter().all(|entity| {
        registry
            .record(entity.entity)
            .is_some_and(|record| record.claim() == entity.claim && record.lifecycle() == lifecycle)
            && world.get::<RouteOwnershipSeal>(entity.entity)
                == Some(&RouteOwnershipSeal::new(entity.claim.seal()))
            && world.get::<FrameLifecycle>(entity.entity) == Some(&FrameLifecycle(lifecycle))
            && world.get::<ChildOf>(entity.entity).map(ChildOf::parent) == Some(entity.parent)
    }) && world.get::<Node>(frame.shell()) == Some(&expected_node)
        && world.get::<FrameShell>(frame.shell()) == Some(&FrameShell(frame.frame.clone()))
        && world.get::<Visibility>(frame.shell())
            == Some(if active {
                &Visibility::Visible
            } else {
                &Visibility::Hidden
            })
        && world.get::<Pickable>(frame.shell()) == Some(&Pickable::IGNORE)
        && world
            .get::<RouterInputBarrier>(frame.shell())
            .is_some_and(RouterInputBarrier::denies_all)
        && world
            .get::<Children>(frame.shell())
            .is_some_and(|children| children.len() == 1 && children[0] == root)
        && collect_content(world, frame.frame(), root).is_ok()
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

    fn active_native_frame() -> (App, Entity, RealizedFrame) {
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
        app.update();
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
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
        let frame = branch.into_single_frame().ok().flatten().unwrap();
        (app, host, frame)
    }

    #[test]
    fn cleanup_retry_retains_partial_claims_and_original_owner() {
        let (mut app, _, mut frame) = active_native_frame();
        let original_frame = frame.frame().clone();
        let original_shell = frame.entities[0].claim.clone();
        app.world_mut()
            .insert_resource(NativeCleanupCapacityLimit { after_claims: 1 });
        let error = prepare_content_cleanup(app.world_mut(), &mut frame).unwrap_err();
        assert!(error.is_retryable());
        assert_eq!(frame.entities.len(), 3);
        let partial = frame.entities[2].claim.clone();
        assert_eq!(
            frame
                .native
                .as_ref()
                .unwrap()
                .cleanup
                .as_ref()
                .unwrap()
                .next,
            2
        );
        app.world_mut()
            .remove_resource::<NativeCleanupCapacityLimit>();
        prepare_content_cleanup(app.world_mut(), &mut frame).unwrap();
        assert_eq!(frame.entities.len(), 4);
        assert_eq!(frame.entities[2].claim, partial);
        assert_eq!(frame.entities[0].claim, original_shell);
        assert_eq!(frame.frame(), &original_frame);
        assert_eq!(
            app.world()
                .resource::<RouteEntityRegistry>()
                .record(partial.entity())
                .unwrap()
                .claim(),
            partial
        );
        assert_eq!(
            app.world()
                .get::<Node>(frame.native_root().unwrap())
                .unwrap()
                .width,
            Val::Px(101.0)
        );
    }

    #[test]
    fn cleanup_excludes_pre_moved_content_and_releases_post_marker_escape() {
        let (mut app, _, mut frame) = active_native_frame();
        let root = frame.native_root().unwrap();
        let children = app.world().get::<Children>(root).unwrap().to_vec();
        let application = app.world_mut().spawn(Node::default()).id();
        app.world_mut()
            .entity_mut(children[0])
            .insert(ChildOf(application));
        let escaped = children[1];
        app.world_mut().add_observer(
            move |event: On<Insert, NativeCleanupContent>, mut commands: Commands| {
                if event.entity == escaped {
                    commands.entity(escaped).insert(ChildOf(application));
                }
            },
        );
        let error = prepare_content_cleanup(app.world_mut(), &mut frame).unwrap_err();
        assert!(!error.is_retryable());
        for child in children {
            assert!(app.world().get_entity(child).is_ok());
            assert_eq!(
                app.world().get::<ChildOf>(child).map(ChildOf::parent),
                Some(application)
            );
            assert!(
                app.world()
                    .resource::<RouteEntityRegistry>()
                    .record(child)
                    .is_none()
            );
            assert!(app.world().get::<RouteOwnershipSeal>(child).is_none());
            assert!(child != escaped || is_escaped_cleanup_content(app.world(), child));
        }
        assert_eq!(
            frame.entities.len(),
            3,
            "only the post-marker escaped claim was ever issued"
        );
        assert!(frame.native.as_ref().unwrap().cleanup_failure.is_some());
    }

    #[test]
    fn cleanup_stops_after_hook_changes_protected_sibling() {
        let (mut app, host, mut frame) = active_native_frame();
        let protected = app
            .world_mut()
            .spawn((
                Node {
                    width: Val::Px(303.0),
                    ..Node::default()
                },
                ChildOf(host),
            ))
            .id();
        app.world_mut().add_observer(
            move |_: On<Insert, NativeCleanupContent>, mut nodes: Query<&mut Node>| {
                nodes.get_mut(protected).unwrap().width = Val::Px(404.0);
            },
        );
        let error = prepare_content_cleanup(app.world_mut(), &mut frame).unwrap_err();
        assert!(
            matches!(error, NativeCleanupPreparationError::Protected(evidence) if evidence.entity == protected)
        );
        assert_eq!(
            frame.entities.len(),
            3,
            "no next marker is issued after protected drift"
        );
        assert!(app.world().get_entity(protected).is_ok());
        assert!(collect_content(app.world(), frame.frame(), frame.native_root().unwrap()).is_ok());
    }

    #[test]
    fn public_replace_settles_post_marker_escape_without_retaining_app_focus_authority() {
        use bevy_input_focus::{FocusCause, InputFocus};
        let (mut app, _, frame) = active_native_frame();
        let root = frame.native_root().unwrap();
        let escaped = app.world().get::<Children>(root).unwrap()[0];
        let application = app.world_mut().spawn(Node::default()).id();
        let router = app.world().resource::<RouterNavigation>().entries[0].handle;
        app.world_mut().resource_mut::<RouterNavigation>().entries[0].active =
            Some(crate::branch_transaction::RealizedBranch::from_single_frame(frame));
        app.init_resource::<InputFocus>();
        app.world_mut().add_observer(
            move |event: On<Insert, NativeCleanupContent>, mut commands: Commands| {
                if event.entity == escaped {
                    commands.queue(move |world: &mut World| {
                        world.entity_mut(escaped).insert(ChildOf(application));
                        world
                            .resource_mut::<InputFocus>()
                            .set(escaped, FocusCause::Navigated);
                    });
                }
            },
        );
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .replace(router, RouteLocation::new("/").with_fragment("replacement"))
            .unwrap();
        for _ in 0..12 {
            app.update();
        }
        assert!(
            matches!(app.world().resource::<RouterSnapshots>().request(router, request), RequestLookup::Found(request) if matches!(request.status(), RequestStatus::Failed(_)))
        );
        assert!(app.world().get_entity(escaped).is_ok());
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
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(escaped));
        assert!(
            !app.world()
                .resource::<RouterInputGate>()
                .allows_keyboard(escaped)
        );
        for _ in 0..3 {
            app.update();
        }
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(escaped));
    }
}
