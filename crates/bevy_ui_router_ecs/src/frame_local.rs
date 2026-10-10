use std::collections::HashSet;

use bevy_camera::visibility::Visibility;
use bevy_ecs::{
    hierarchy::{ChildOf, Children},
    prelude::{Entity, World},
};
use bevy_picking::Pickable;
use bevy_ui_router_core::frame::RouteFrameVersion;

use crate::{
    branch_transaction::{
        RealizedBranch, ReconciliationSlotRoleKind, SealedMountNewStagingAllowance,
        SealedReconciliationSlotRole,
    },
    components::{ChildSlotAuthority, ChildSlotLifecycle, RouteOwnershipSeal, RouterInputBarrier},
    mount::{validate_leaving_mounted_child_slot, validate_mounted_child_slot},
    presentation::{
        validate_active_frame_local_values,
        validate_active_frame_with_child_slots_and_entering_shells,
        validate_leaving_frame_with_child_slots, validate_realized_frame_with_child_slots,
        validate_reconciliation_entering_frame_with_child_slots,
    },
    registry::{
        ComponentManifest, EntityLifecycle, EntityOwner, RegistryOwner, RouteEntityRecord,
        RouteEntityRegistry,
    },
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FrameLocalError {
    MissingRoot(Entity),
    OwnershipDrift(Entity),
    RelationshipCycle(Entity),
    ForeignChildSlot(Entity),
}

pub(crate) fn collect_frame_local_entities(
    world: &World,
    frame: &RouteFrameVersion,
    root: Entity,
    registry: &RouteEntityRegistry,
) -> Result<Vec<Entity>, FrameLocalError> {
    collect_frame_local_entities_with_policy(world, frame, root, registry, ManifestPolicy::Strict)
}

#[derive(Clone, Copy)]
enum ManifestPolicy {
    Strict,
    Observed,
}

fn collect_frame_local_entities_with_policy(
    world: &World,
    frame: &RouteFrameVersion,
    root: Entity,
    registry: &RouteEntityRegistry,
    manifest_policy: ManifestPolicy,
) -> Result<Vec<Entity>, FrameLocalError> {
    if world.get_entity(root).is_err() || registry.record(root).is_none() {
        return Err(FrameLocalError::MissingRoot(root));
    }
    if !matches!(
        registry.record(root).map(RouteEntityRecord::owner),
        Some(RegistryOwner::Committed(EntityOwner::FrameShell(owner))) if owner == frame
    ) {
        return Err(FrameLocalError::OwnershipDrift(root));
    }

    #[cfg(feature = "bsn")]
    if let Some(content_root) = world.get::<Children>(root).and_then(|children| {
        children.iter().copied().find(|child| {
            world
                .get::<crate::presentation::native_scene::NativeContentRoot>(*child)
                .is_some()
        })
    }) {
        let content =
            crate::presentation::native_frame::collect_content(world, frame, content_root)?;
        let mut structure = vec![root, content_root];
        for entity in content {
            for child in world
                .get::<Children>(entity)
                .into_iter()
                .flat_map(|children| children.iter().copied())
            {
                if let Some(record) = registry.record(child) {
                    if !matches!(record.owner(), RegistryOwner::Committed(EntityOwner::ChildSlot { parent, .. }) if parent == frame)
                    {
                        return Err(FrameLocalError::OwnershipDrift(child));
                    }
                    structure.push(child);
                }
            }
        }
        return Ok(structure);
    }

    let mut visited = HashSet::new();
    let mut pending = vec![root];
    let mut ordered = Vec::new();
    while let Some(entity) = pending.pop() {
        if !visited.insert(entity) {
            return Err(FrameLocalError::RelationshipCycle(entity));
        }
        let record = registry
            .record(entity)
            .ok_or(FrameLocalError::OwnershipDrift(entity))?;
        let boundary = classify_owned_entity(frame, entity, record)?;
        if !manifest_is_acceptable(world, entity, record, manifest_policy)
            || world.get::<RouteOwnershipSeal>(entity)
                != Some(&RouteOwnershipSeal::new(record.seal()))
        {
            return Err(FrameLocalError::OwnershipDrift(entity));
        }
        ordered.push(entity);
        if boundary {
            continue;
        }

        let mut children = world
            .get::<Children>(entity)
            .map(|children| children.iter().copied().collect::<Vec<_>>())
            .unwrap_or_default();
        for child in &children {
            if world.get::<ChildOf>(*child).map(ChildOf::parent) != Some(entity) {
                return Err(FrameLocalError::OwnershipDrift(*child));
            }
        }
        children.retain(|child| registry.record(*child).is_some());
        children.sort_by_key(|child| {
            registry
                .record(*child)
                .expect("retained registry record remains present")
                .seal()
                .stable_identity()
        });
        for child in children.into_iter().rev() {
            let record = registry
                .record(child)
                .expect("sorted registry record remains present");
            classify_owned_entity(frame, child, record)?;
            pending.push(child);
        }
    }
    Ok(ordered)
}

fn manifest_is_acceptable(
    world: &World,
    entity: Entity,
    record: &RouteEntityRecord,
    policy: ManifestPolicy,
) -> bool {
    #[cfg(feature = "bsn")]
    if world
        .get::<crate::presentation::native_scene::NativeContentRoot>(entity)
        .is_some()
    {
        return matches!(
            record.owner(),
            RegistryOwner::Committed(EntityOwner::FrameContentTarget(_))
        );
    }
    match policy {
        ManifestPolicy::Strict => record.manifest().matches(world, entity),
        ManifestPolicy::Observed => {
            ComponentManifest::from_world(world, entity).is_some_and(|actual| {
                record
                    .manifest()
                    .component_ids()
                    .iter()
                    .all(|component| actual.component_ids().contains(component))
            })
        }
    }
}

fn classify_owned_entity(
    frame: &RouteFrameVersion,
    entity: Entity,
    record: &RouteEntityRecord,
) -> Result<bool, FrameLocalError> {
    match record.owner() {
        RegistryOwner::Committed(
            EntityOwner::FrameShell(owner)
            | EntityOwner::FrameContentTarget(owner)
            | EntityOwner::FrameOutput { frame: owner, .. },
        ) if owner == frame => Ok(false),
        RegistryOwner::Committed(EntityOwner::ChildSlot { parent, .. }) if parent == frame => {
            Ok(true)
        }
        RegistryOwner::Committed(EntityOwner::ChildSlot { .. }) => {
            Err(FrameLocalError::ForeignChildSlot(entity))
        }
        _ => Err(FrameLocalError::OwnershipDrift(entity)),
    }
}

pub(crate) fn validate_prepared_branch(
    world: &World,
    branch: &RealizedBranch,
) -> Result<(), FrameLocalError> {
    validate_branch_structure(branch)?;
    for slot in branch.child_slots() {
        if !validate_mounted_child_slot(world, slot, EntityLifecycle::PreparedInert) {
            return Err(FrameLocalError::OwnershipDrift(slot.slot()));
        }
    }
    for frame in branch.frames() {
        let child_slots = child_slot_expectations(branch, frame.frame());
        if !validate_realized_frame_with_child_slots(world, frame, &child_slots) {
            return Err(FrameLocalError::OwnershipDrift(frame.shell()));
        }
        validate_frame_claim_set(
            world,
            frame.frame(),
            frame.shell(),
            frame.claims(),
            branch,
            ManifestPolicy::Strict,
            &[],
        )?;
    }
    validate_branch_descendants(world, branch).map(|_| ())
}

/// Validate an entering parent before creating its next child slot. A replaced
/// frame can share a committed slot with the old frame until publication.
pub(crate) fn validate_entering_mount_parent(
    world: &World,
    frame: &crate::presentation::RealizedFrame,
    entering: &RealizedBranch,
    active: Option<&RealizedBranch>,
) -> bool {
    let existing = active
        .into_iter()
        .flat_map(RealizedBranch::frames)
        .filter(|old| {
            frame
                .sealed_reconciliation_entering_shell_for(old)
                .is_some()
        })
        .collect::<Vec<_>>();
    let leaving = active
        .into_iter()
        .flat_map(RealizedBranch::frames)
        .filter_map(|old| frame.sealed_reconciliation_leaving_sibling_shell_for(old))
        .cloned()
        .collect::<Vec<_>>();
    let existing_slots = active
        .into_iter()
        .flat_map(RealizedBranch::child_slots)
        .filter(|slot| frame.mount_binding().matches_mounted_child_slot(slot))
        .count();
    existing.len() <= 1
        && (existing.is_empty() || existing_slots == 1)
        && validate_reconciliation_entering_frame_with_child_slots(
            world,
            frame,
            &child_slot_expectations(entering, frame.frame()),
            existing.first().copied(),
            &leaving,
        )
}

pub(crate) fn validate_reconciliation_entering_branch<'payload>(
    world: &World,
    entering: impl Into<crate::instance::PayloadRef<'payload>>,
    preserved: &RealizedBranch,
    leaving: &RealizedBranch,
) -> Result<(), FrameLocalError> {
    let entering = entering.into();
    validate_reconciliation_entering_structure(entering, preserved)?;
    for slot in entering.child_slots() {
        if !validate_mounted_child_slot(world, slot, EntityLifecycle::PreparedInert) {
            return Err(FrameLocalError::OwnershipDrift(slot.slot()));
        }
    }
    for frame in entering.frames() {
        let child_slots = child_slot_expectations(entering, frame.frame());
        let existing_mounts = leaving
            .frames()
            .iter()
            .filter(|preserved| {
                frame
                    .sealed_reconciliation_entering_shell_for(preserved)
                    .is_some()
            })
            .collect::<Vec<_>>();
        let observed_leaving_shells = leaving
            .frames()
            .iter()
            .filter_map(|leaving| frame.sealed_reconciliation_leaving_sibling_shell_for(leaving))
            .cloned()
            .collect::<Vec<_>>();
        let preserved_slots = preserved
            .child_slots()
            .iter()
            .filter(|slot| frame.mount_binding().matches_mounted_child_slot(slot))
            .count();
        if existing_mounts.len() > 1
            || (existing_mounts.len() == 1 && preserved_slots != 1)
            || !validate_reconciliation_entering_frame_with_child_slots(
                world,
                frame,
                &child_slots,
                existing_mounts.first().copied(),
                &observed_leaving_shells,
            )
        {
            return Err(FrameLocalError::OwnershipDrift(frame.shell()));
        }
        validate_frame_claim_set(
            world,
            frame.frame(),
            frame.shell(),
            frame.claims(),
            entering,
            ManifestPolicy::Strict,
            &[],
        )?;
    }
    validate_branch_descendants(world, entering).map(|_| ())
}

pub(crate) fn validate_committed_branch(
    world: &World,
    branch: &RealizedBranch,
    leaving: bool,
) -> Result<Vec<Entity>, FrameLocalError> {
    validate_committed_branch_with_entering(world, branch, leaving, None, None, None)
}

pub(crate) fn validate_active_branch_with_entering(
    world: &World,
    branch: &RealizedBranch,
    entering: Option<&RealizedBranch>,
    staging_allowance: Option<&SealedMountNewStagingAllowance>,
) -> Result<Vec<Entity>, FrameLocalError> {
    validate_committed_branch_with_entering(
        world,
        branch,
        false,
        entering.map(Into::into),
        staging_allowance,
        None,
    )
}

pub(crate) fn validate_active_branch_for_splice<'payload>(
    world: &World,
    preserved: &RealizedBranch,
    entering: impl Into<crate::instance::PayloadRef<'payload>>,
    slot_roles: &[SealedReconciliationSlotRole],
) -> Result<Vec<Entity>, FrameLocalError> {
    let entering = entering.into();
    validate_committed_branch_with_entering(
        world,
        preserved,
        false,
        Some(entering),
        None,
        Some(slot_roles),
    )
}

pub(crate) fn validate_preserved_branch_local(
    world: &World,
    preserved: &RealizedBranch,
    slot_roles: &[SealedReconciliationSlotRole],
    entering_boundaries: &[Entity],
) -> Result<Vec<Entity>, FrameLocalError> {
    validate_branch_structure(preserved)?;
    for slot in preserved.child_slots() {
        if !validate_sealed_existing_slot_role(world, slot, slot_roles, false) {
            return Err(FrameLocalError::OwnershipDrift(slot.slot()));
        }
    }
    if let Some(role) = slot_roles.iter().find(|role| {
        matches!(
            role.kind(),
            ReconciliationSlotRoleKind::PreservedActive
                | ReconciliationSlotRoleKind::RetainEmpty
                | ReconciliationSlotRoleKind::ActivateExisting
        ) && role.current() != EntityLifecycle::Retained
            && preserved
                .child_slots()
                .iter()
                .filter(|slot| sealed_existing_role_matches_slot(role, slot))
                .count()
                != 1
    }) {
        return Err(FrameLocalError::OwnershipDrift(role.entity()));
    }
    for frame in preserved.frames() {
        if !validate_active_frame_local_values(world, frame) {
            return Err(FrameLocalError::OwnershipDrift(frame.shell()));
        }
    }
    validate_branch_descendants_with_boundaries(world, preserved, entering_boundaries)
}

pub(crate) fn validate_active_branch_local(
    world: &World,
    active: &RealizedBranch,
    entering_boundaries: &[Entity],
) -> Result<Vec<Entity>, FrameLocalError> {
    validate_branch_structure(active)?;
    for slot in active.child_slots() {
        let occupied = active
            .frames()
            .iter()
            .any(|frame| frame.mount_binding().matches_mounted_child_slot(slot));
        let valid = match slot.slot_claim().lifecycle() {
            EntityLifecycle::Active if occupied => {
                validate_leaving_mounted_child_slot(world, slot, EntityLifecycle::Active)
            }
            EntityLifecycle::PreparedInert if !occupied => {
                validate_mounted_child_slot(world, slot, EntityLifecycle::PreparedInert)
            }
            EntityLifecycle::Active
            | EntityLifecycle::PreparedInert
            | EntityLifecycle::Retained
            | EntityLifecycle::Leaving
            | EntityLifecycle::Quarantined => false,
        };
        if !valid {
            return Err(FrameLocalError::OwnershipDrift(slot.slot()));
        }
    }
    for frame in active.frames() {
        if !validate_active_frame_local_values(world, frame) {
            return Err(FrameLocalError::OwnershipDrift(frame.shell()));
        }
    }
    validate_branch_descendants_with_boundaries(world, active, entering_boundaries)
}

pub(crate) fn validate_leaving_branch_for_splice(
    world: &World,
    leaving: &RealizedBranch,
    slot_roles: &[SealedReconciliationSlotRole],
) -> Result<Vec<Entity>, FrameLocalError> {
    validate_committed_branch_with_entering(world, leaving, true, None, None, Some(slot_roles))
}

fn validate_committed_branch_with_entering(
    world: &World,
    branch: &RealizedBranch,
    leaving: bool,
    entering: Option<crate::instance::PayloadRef<'_>>,
    staging_allowance: Option<&SealedMountNewStagingAllowance>,
    sealed_slot_roles: Option<&[SealedReconciliationSlotRole]>,
) -> Result<Vec<Entity>, FrameLocalError> {
    validate_branch_structure(branch)?;
    if let Some(allowance) = staging_allowance.filter(|allowance| {
        world
            .get::<ChildOf>(allowance.entity())
            .map(ChildOf::parent)
            == Some(allowance.anchor())
    }) {
        if branch
            .frames()
            .iter()
            .filter(|frame| frame.frame() == allowance.parent())
            .count()
            != 1
            || !validate_mount_new_staging_allowance(world, allowance)
        {
            return Err(FrameLocalError::ForeignChildSlot(allowance.entity()));
        }
    }
    let mut sealed_entering_shells = Vec::new();
    for slot in branch.child_slots() {
        let occupied = branch
            .frames()
            .iter()
            .any(|frame| frame.mount_binding().matches_mounted_child_slot(slot))
            || entering.into_iter().any(|entering| {
                entering
                    .frames()
                    .iter()
                    .any(|frame| frame.mount_binding().matches_mounted_child_slot(slot))
            });
        let valid = if let Some(roles) = sealed_slot_roles {
            validate_sealed_existing_slot_role(world, slot, roles, leaving)
        } else if leaving {
            validate_leaving_mounted_child_slot(world, slot, EntityLifecycle::Active)
        } else if entering.is_some() && occupied {
            match slot.slot_claim().lifecycle() {
                EntityLifecycle::Active => {
                    validate_leaving_mounted_child_slot(world, slot, EntityLifecycle::Active)
                }
                EntityLifecycle::PreparedInert => {
                    validate_mounted_child_slot(world, slot, EntityLifecycle::PreparedInert)
                }
                EntityLifecycle::Retained
                | EntityLifecycle::Leaving
                | EntityLifecycle::Quarantined => false,
            }
        } else {
            validate_mounted_child_slot(
                world,
                slot,
                if occupied {
                    EntityLifecycle::Active
                } else {
                    EntityLifecycle::PreparedInert
                },
            )
        };
        if !valid {
            return Err(FrameLocalError::OwnershipDrift(slot.slot()));
        }
    }
    if let Some(roles) = sealed_slot_roles {
        let mismatched_role = roles.iter().find(|role| {
            let belongs_to_branch = if leaving {
                role.kind() == ReconciliationSlotRoleKind::Leaving
            } else {
                matches!(
                    role.kind(),
                    ReconciliationSlotRoleKind::PreservedActive
                        | ReconciliationSlotRoleKind::RetainEmpty
                        | ReconciliationSlotRoleKind::ActivateExisting
                )
            };
            belongs_to_branch
                && role.current() != EntityLifecycle::Retained
                && branch
                    .child_slots()
                    .iter()
                    .filter(|slot| sealed_existing_role_matches_slot(role, slot))
                    .count()
                    != 1
        });
        if let Some(role) = mismatched_role {
            return Err(FrameLocalError::OwnershipDrift(role.entity()));
        }
    }
    for frame in branch.frames() {
        let mut child_slots = child_slot_expectations(branch, frame.frame());
        let mut external_boundaries = Vec::new();
        if let Some(entering) = entering {
            let entering_slots = entering
                .child_slots()
                .iter()
                .filter(|slot| slot.parent() == frame.frame())
                .collect::<Vec<_>>();
            if let Some(roles) = sealed_slot_roles {
                if entering_slots
                    .iter()
                    .any(|slot| !sealed_mount_new_expectation_is_unique(roles, slot))
                    || roles.iter().any(|role| {
                        role.kind() == ReconciliationSlotRoleKind::MountNew
                            && role.parent() == frame.frame()
                            && entering_slots
                                .iter()
                                .filter(|slot| sealed_mount_new_role_matches_slot(role, slot))
                                .count()
                                != 1
                    })
                {
                    return Err(FrameLocalError::ForeignChildSlot(frame.shell()));
                }
                external_boundaries.extend(entering_slots.iter().map(|slot| slot.slot()));
                child_slots.extend(
                    entering_slots
                        .into_iter()
                        .map(|slot| (slot.anchor(), slot.expected_anchor_children().to_vec())),
                );
            } else {
                external_boundaries.extend(entering_slots.iter().map(|slot| slot.slot()));
                child_slots.extend(
                    entering_slots
                        .into_iter()
                        .map(|slot| (slot.anchor(), slot.expected_anchor_children().to_vec())),
                );
            }
        }
        if let Some(allowance) = staging_allowance.filter(|allowance| {
            allowance.parent() == frame.frame()
                && world
                    .get::<ChildOf>(allowance.entity())
                    .map(ChildOf::parent)
                    == Some(allowance.anchor())
        }) {
            external_boundaries.push(allowance.entity());
            child_slots.push((
                allowance.anchor(),
                allowance.expected_anchor_children().to_vec(),
            ));
        }
        let valid = if leaving {
            validate_leaving_frame_with_child_slots(world, frame, &child_slots)
        } else {
            let entering_shells = entering
                .into_iter()
                .flat_map(|payload| payload.frames())
                .filter_map(|entering| entering.sealed_reconciliation_entering_shell_for(frame))
                .cloned()
                .collect::<Vec<_>>();
            if entering_shells.len() > 1 {
                return Err(FrameLocalError::OwnershipDrift(frame.shell()));
            }
            let valid = validate_active_frame_with_child_slots_and_entering_shells(
                world,
                frame,
                &child_slots,
                &entering_shells,
            );
            if valid {
                sealed_entering_shells.extend(entering_shells);
                sealed_entering_shells.extend(
                    frame
                        .mount_binding()
                        .staging_existing_shells()
                        .iter()
                        .cloned(),
                );
            }
            valid
        };
        if !valid {
            return Err(FrameLocalError::OwnershipDrift(frame.shell()));
        }
        validate_frame_claim_set(
            world,
            frame.frame(),
            frame.shell(),
            frame.claims(),
            branch,
            ManifestPolicy::Observed,
            &external_boundaries,
        )?;
    }
    validate_branch_descendants_allowing_entering(world, branch, &sealed_entering_shells, &[])
}

fn validate_sealed_existing_slot_role(
    world: &World,
    slot: &crate::mount::MountedChildSlot,
    roles: &[SealedReconciliationSlotRole],
    leaving: bool,
) -> bool {
    let mut matching = roles
        .iter()
        .filter(|role| sealed_existing_role_matches_slot(role, slot));
    let Some(role) = matching.next() else {
        return false;
    };
    if matching.next().is_some() {
        return false;
    }
    let transition_is_valid = if leaving {
        role.kind() == ReconciliationSlotRoleKind::Leaving
            && matches!(
                role.current(),
                EntityLifecycle::Active | EntityLifecycle::PreparedInert
            )
            && role.next() == Some(EntityLifecycle::Leaving)
    } else {
        matches!(
            (role.kind(), role.current(), role.next()),
            (
                ReconciliationSlotRoleKind::PreservedActive,
                EntityLifecycle::Active,
                None,
            ) | (
                ReconciliationSlotRoleKind::RetainEmpty,
                EntityLifecycle::Active,
                Some(EntityLifecycle::PreparedInert),
            ) | (
                ReconciliationSlotRoleKind::RetainEmpty,
                EntityLifecycle::PreparedInert,
                None,
            ) | (
                ReconciliationSlotRoleKind::ActivateExisting,
                EntityLifecycle::PreparedInert,
                Some(EntityLifecycle::Active),
            )
        )
    };
    if !transition_is_valid {
        return false;
    }
    if role.current() == EntityLifecycle::PreparedInert && !leaving {
        validate_mounted_child_slot(world, slot, role.current())
    } else {
        validate_leaving_mounted_child_slot(world, slot, role.current())
    }
}

fn sealed_existing_role_matches_slot(
    role: &SealedReconciliationSlotRole,
    slot: &crate::mount::MountedChildSlot,
) -> bool {
    role.entity() == slot.slot()
        && role.claim() == slot.slot_claim()
        && role.parent() == slot.parent()
        && role.outlet() == slot.outlet()
        && role.slot() == slot.slot_id()
        && role.anchor() == slot.anchor()
        && role.anchor_claim() == slot.anchor_claim()
}

pub(crate) fn sealed_mount_new_expectation_is_unique(
    roles: &[SealedReconciliationSlotRole],
    slot: &crate::mount::MountedChildSlot,
) -> bool {
    roles
        .iter()
        .filter(|role| sealed_mount_new_role_matches_slot(role, slot))
        .count()
        == 1
}

fn sealed_mount_new_role_matches_slot(
    role: &SealedReconciliationSlotRole,
    slot: &crate::mount::MountedChildSlot,
) -> bool {
    role.kind() == ReconciliationSlotRoleKind::MountNew
        && role.current() == EntityLifecycle::PreparedInert
        && role.next() == Some(EntityLifecycle::Active)
        && role.entity() == slot.slot()
        && role.claim() == slot.slot_claim()
        && role.parent() == slot.parent()
        && role.outlet() == slot.outlet()
        && role.slot() == slot.slot_id()
        && role.anchor() == slot.anchor()
        && role.anchor_claim() == slot.anchor_claim()
}

fn validate_mount_new_staging_allowance(
    world: &World,
    allowance: &SealedMountNewStagingAllowance,
) -> bool {
    let registry = world.resource::<RouteEntityRegistry>();
    let record_is_exact = registry
        .record(allowance.entity())
        .is_some_and(|record| record.claim() == *allowance.claim());
    let anchor_is_exact = allowance
        .anchor_binding()
        .is_current(world, allowance.parent(), true);
    record_is_exact
        && anchor_is_exact
        && allowance.matches_binding(
            allowance.parent(),
            allowance.outlet(),
            allowance.slot(),
            allowance.entity(),
            allowance.claim(),
            allowance.anchor(),
            allowance.anchor_binding(),
        )
        && world.get::<RouteOwnershipSeal>(allowance.entity())
            == Some(&RouteOwnershipSeal::new(allowance.claim().seal()))
        && world
            .get::<ChildSlotAuthority>(allowance.entity())
            .is_some_and(|authority| {
                authority.parent() == allowance.parent()
                    && authority.outlet() == allowance.outlet()
                    && authority.slot() == allowance.slot()
            })
        && matches!(
            world.get::<ChildSlotLifecycle>(allowance.entity()),
            Some(ChildSlotLifecycle::ProvisionalInert | ChildSlotLifecycle::RegisteredInert)
        )
        && world.get::<Visibility>(allowance.entity()) == Some(&Visibility::Hidden)
        && world.get::<Pickable>(allowance.entity()) == Some(&Pickable::IGNORE)
        && world
            .get::<RouterInputBarrier>(allowance.entity())
            .is_some_and(RouterInputBarrier::denies_all)
        && world
            .get::<Children>(allowance.anchor())
            .map(|children| children.iter().copied().collect::<Vec<_>>())
            .unwrap_or_default()
            == allowance.expected_anchor_children()
}

fn validate_reconciliation_entering_structure<'payload>(
    entering: impl Into<crate::instance::PayloadRef<'payload>>,
    preserved: &RealizedBranch,
) -> Result<(), FrameLocalError> {
    let entering = entering.into();
    if entering.frames().is_empty() {
        return entering.child_slots().first().map_or(Ok(()), |slot| {
            Err(FrameLocalError::ForeignChildSlot(slot.slot()))
        });
    }
    let mut roots = 0usize;
    for frame in entering.frames() {
        let incoming = entering
            .child_slots()
            .iter()
            .filter(|slot| frame.mount_binding().matches_mounted_child_slot(slot))
            .count();
        if incoming == 0 {
            roots += 1;
        } else if incoming != 1 {
            return Err(FrameLocalError::OwnershipDrift(frame.shell()));
        }
    }
    let mut preserved_boundaries = 0usize;
    for slot in entering.child_slots() {
        let parent_is_entering = entering
            .frames()
            .iter()
            .any(|frame| frame.frame() == slot.parent());
        let parent_is_preserved = preserved
            .frames()
            .iter()
            .any(|frame| frame.frame() == slot.parent());
        let children = entering
            .frames()
            .iter()
            .filter(|frame| frame.mount_binding().matches_mounted_child_slot(slot))
            .count();
        if parent_is_entering == parent_is_preserved || children != 1 {
            return Err(FrameLocalError::ForeignChildSlot(slot.slot()));
        }
        preserved_boundaries += usize::from(parent_is_preserved);
    }
    if roots + preserved_boundaries != 1 {
        return Err(FrameLocalError::OwnershipDrift(
            entering.frames()[0].shell(),
        ));
    }
    Ok(())
}

fn validate_branch_structure(branch: &RealizedBranch) -> Result<(), FrameLocalError> {
    if branch.frames().is_empty() {
        return branch.child_slots().first().map_or(Ok(()), |slot| {
            Err(FrameLocalError::ForeignChildSlot(slot.slot()))
        });
    }
    let mut roots = 0usize;
    for frame in branch.frames() {
        let incoming = branch
            .child_slots()
            .iter()
            .filter(|slot| frame.mount_binding().matches_mounted_child_slot(slot))
            .count();
        if incoming == 0 {
            roots += 1;
        } else if incoming != 1 {
            return Err(FrameLocalError::OwnershipDrift(frame.shell()));
        }
    }
    if roots != 1 {
        return Err(FrameLocalError::OwnershipDrift(branch.frames()[0].shell()));
    }
    for slot in branch.child_slots() {
        if !branch
            .frames()
            .iter()
            .any(|frame| frame.frame() == slot.parent())
            || branch
                .frames()
                .iter()
                .filter(|frame| frame.mount_binding().matches_mounted_child_slot(slot))
                .count()
                > 1
        {
            return Err(FrameLocalError::ForeignChildSlot(slot.slot()));
        }
    }
    Ok(())
}

fn child_slot_expectations<'payload>(
    branch: impl Into<crate::instance::PayloadRef<'payload>>,
    frame: &RouteFrameVersion,
) -> Vec<(Entity, Vec<Entity>)> {
    let branch = branch.into();
    branch
        .child_slots()
        .iter()
        .filter(|slot| slot.parent() == frame)
        .map(|slot| (slot.anchor(), slot.expected_anchor_children().to_vec()))
        .collect()
}

fn validate_frame_claim_set<'a, 'payload>(
    world: &World,
    frame: &RouteFrameVersion,
    root: Entity,
    claims: impl Iterator<Item = &'a crate::registry::ExactRegistryClaim>,
    branch: impl Into<crate::instance::PayloadRef<'payload>>,
    manifest_policy: ManifestPolicy,
    external_boundaries: &[Entity],
) -> Result<(), FrameLocalError> {
    let branch = branch.into();
    let registry = world
        .get_resource::<RouteEntityRegistry>()
        .ok_or(FrameLocalError::MissingRoot(root))?;
    let claims = claims.collect::<Vec<_>>();
    if let Some(stale) = claims.iter().find(|claim| {
        registry
            .record(claim.entity())
            .is_none_or(|record| record.claim() != (**claim).clone())
    }) {
        return Err(FrameLocalError::OwnershipDrift(stale.entity()));
    }
    let physical =
        collect_frame_local_entities_with_policy(world, frame, root, registry, manifest_policy)?;
    let expected = claims
        .iter()
        .map(|claim| claim.entity())
        .collect::<HashSet<_>>();
    let mut boundaries = branch
        .child_slots()
        .iter()
        .filter(|slot| slot.parent() == frame)
        .map(|slot| slot.slot())
        .collect::<HashSet<_>>();
    for boundary in external_boundaries {
        if expected.contains(boundary) || !boundaries.insert(*boundary) {
            return Err(FrameLocalError::ForeignChildSlot(*boundary));
        }
    }
    let actual = physical.into_iter().collect::<HashSet<_>>();
    if actual.len() != expected.len() + boundaries.len()
        || !expected.is_subset(&actual)
        || !boundaries.is_subset(&actual)
    {
        return Err(FrameLocalError::OwnershipDrift(root));
    }
    Ok(())
}

pub(crate) fn validate_branch_descendants<'payload>(
    world: &World,
    branch: impl Into<crate::instance::PayloadRef<'payload>>,
) -> Result<Vec<Entity>, FrameLocalError> {
    let branch = branch.into();
    validate_branch_descendants_allowing_entering(world, branch, &[], &[])
}

fn validate_branch_descendants_with_boundaries(
    world: &World,
    branch: &RealizedBranch,
    opaque_boundaries: &[Entity],
) -> Result<Vec<Entity>, FrameLocalError> {
    validate_branch_descendants_allowing_entering(world, branch, &[], opaque_boundaries)
}

fn validate_branch_descendants_allowing_entering<'payload>(
    world: &World,
    branch: impl Into<crate::instance::PayloadRef<'payload>>,
    sealed_entering_shells: &[crate::registry::ExactRegistryClaim],
    opaque_boundaries: &[Entity],
) -> Result<Vec<Entity>, FrameLocalError> {
    let branch = branch.into();
    if branch.frames().is_empty() {
        return Ok(Vec::new());
    }
    let registry = world
        .get_resource::<RouteEntityRegistry>()
        .ok_or_else(|| FrameLocalError::MissingRoot(branch.frames()[0].shell()))?;
    let mut ordered = Vec::new();
    let mut branch_seen = HashSet::new();
    for frame in branch.frames() {
        let local = collect_frame_focus_unit(
            world,
            frame.frame(),
            frame.shell(),
            registry,
            opaque_boundaries,
        )?;
        extend_focus_closure(&mut ordered, &mut branch_seen, local)?;
    }
    for slot in branch.child_slots() {
        let Some(child) = branch
            .frames()
            .iter()
            .find(|frame| frame.mount_binding().matches_mounted_child_slot(slot))
        else {
            continue;
        };
        let foreign = collect_child_slot_foreign_focus(
            world,
            slot.slot(),
            child.frame(),
            child.shell(),
            registry,
            sealed_entering_shells,
            opaque_boundaries,
        )?;
        extend_focus_closure(&mut ordered, &mut branch_seen, foreign)?;
    }
    Ok(ordered)
}

fn collect_frame_focus_unit(
    world: &World,
    frame: &RouteFrameVersion,
    root: Entity,
    registry: &RouteEntityRegistry,
    opaque_boundaries: &[Entity],
) -> Result<Vec<Entity>, FrameLocalError> {
    if world.get_entity(root).is_err() || registry.record(root).is_none() {
        return Err(FrameLocalError::MissingRoot(root));
    }
    let mut pending = vec![root];
    let mut visited = HashSet::new();
    let mut ordered = Vec::new();
    while let Some(entity) = pending.pop() {
        if !visited.insert(entity) {
            return Err(FrameLocalError::RelationshipCycle(entity));
        }
        let boundary = if let Some(record) = registry.record(entity) {
            let boundary = classify_owned_entity(frame, entity, record)?;
            if world.get::<RouteOwnershipSeal>(entity)
                != Some(&RouteOwnershipSeal::new(record.seal()))
            {
                return Err(FrameLocalError::OwnershipDrift(entity));
            }
            boundary
        } else {
            false
        };
        ordered.push(entity);
        if boundary {
            continue;
        }
        let mut children = world
            .get::<Children>(entity)
            .map(|children| children.iter().copied().collect::<Vec<_>>())
            .unwrap_or_default();
        children.retain(|child| !opaque_boundaries.contains(child));
        for child in &children {
            if world.get::<ChildOf>(*child).map(ChildOf::parent) != Some(entity) {
                return Err(FrameLocalError::OwnershipDrift(*child));
            }
        }
        children.sort_by_key(|child| {
            // Native cleanup adds transient claims without changing content.
            // Keep its focus projection stable across that preparation.
            #[cfg(feature = "bsn")]
            if crate::presentation::native_frame::is_native_shell(world, root) {
                return (1, (0, child.to_bits()));
            }
            registry
                .record(*child)
                .map(|record| (0, record.seal().stable_identity()))
                .unwrap_or((1, (0, child.to_bits())))
        });
        pending.extend(children.into_iter().rev());
    }
    Ok(ordered)
}

fn collect_child_slot_foreign_focus(
    world: &World,
    slot: Entity,
    child_frame: &RouteFrameVersion,
    child_root: Entity,
    registry: &RouteEntityRegistry,
    sealed_entering_shells: &[crate::registry::ExactRegistryClaim],
    opaque_boundaries: &[Entity],
) -> Result<Vec<Entity>, FrameLocalError> {
    if world.get::<ChildOf>(child_root).map(ChildOf::parent) != Some(slot)
        || !matches!(
            registry.record(child_root).map(RouteEntityRecord::owner),
            Some(RegistryOwner::Committed(EntityOwner::FrameShell(owner))) if owner == child_frame
        )
    {
        return Err(FrameLocalError::OwnershipDrift(child_root));
    }
    let mut pending = world
        .get::<Children>(slot)
        .map(|children| children.iter().copied().collect::<Vec<_>>())
        .unwrap_or_default();
    pending.retain(|child| !opaque_boundaries.contains(child));
    for child in &pending {
        if world.get::<ChildOf>(*child).map(ChildOf::parent) != Some(slot) {
            return Err(FrameLocalError::OwnershipDrift(*child));
        }
    }
    pending.retain(|child| *child != child_root);
    pending.sort_by_key(|entity| entity.to_bits());
    pending.reverse();
    let mut visited = HashSet::new();
    let mut ordered = Vec::new();
    while let Some(entity) = pending.pop() {
        if entity == child_root || opaque_boundaries.contains(&entity) {
            continue;
        }
        if !visited.insert(entity) {
            return Err(FrameLocalError::RelationshipCycle(entity));
        }
        if let Some(record) = registry.record(entity) {
            if record.lifecycle() == EntityLifecycle::Retained {
                if registry.retained_shell_is_current(world, entity, slot) {
                    continue;
                }
                return Err(FrameLocalError::OwnershipDrift(entity));
            }
            if sealed_entering_shells
                .iter()
                .any(|claim| claim.entity() == entity && *claim == record.claim())
            {
                continue;
            }
            return Err(FrameLocalError::OwnershipDrift(entity));
        }
        ordered.push(entity);
        let mut children = world
            .get::<Children>(entity)
            .map(|children| children.iter().copied().collect::<Vec<_>>())
            .unwrap_or_default();
        children.retain(|child| !opaque_boundaries.contains(child));
        for child in &children {
            if world.get::<ChildOf>(*child).map(ChildOf::parent) != Some(entity) {
                return Err(FrameLocalError::OwnershipDrift(*child));
            }
        }
        children.sort_by_key(|child| child.to_bits());
        pending.extend(children.into_iter().rev());
    }
    Ok(ordered)
}

fn extend_focus_closure(
    ordered: &mut Vec<Entity>,
    seen: &mut HashSet<Entity>,
    unit: Vec<Entity>,
) -> Result<(), FrameLocalError> {
    for entity in unit {
        if !seen.insert(entity) {
            return Err(FrameLocalError::RelationshipCycle(entity));
        }
        ordered.push(entity);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use bevy_ecs::{hierarchy::ChildOf, prelude::World};
    use bevy_ui_router_core::{
        integration::{EngineAction, RouterEngine},
        prelude::{RouteLocation, RoutePattern, RouteTree, RouteViewKey},
        slot::RouteSlotKey,
    };

    use super::*;
    use crate::{
        components::RouteOwnershipSeal,
        registry::{
            ComponentManifest, EntityLifecycle, EntityOwner, RegistryOwner, RouteEntityRegistry,
        },
    };

    fn frame_fixture(label: &str) -> RouteFrameVersion {
        let view = RouteViewKey::new(label).unwrap();
        let tree = RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/").unwrap(),
                bevy_ui_router_core::prelude::RouteNodeOptions::new().with_view(view.clone()),
            )
            .unwrap()
            .build()
            .unwrap();
        let mut engine = RouterEngine::install(tree, view).unwrap();
        let token = engine.request_host_admission().unwrap();
        let grant = engine.grant_host_admission(&token).unwrap();
        engine.confirm_host(grant);
        engine.push(RouteLocation::new("/")).unwrap();
        engine.collect();
        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("frame task expected")
        };
        task.frame().clone()
    }

    fn register_owned(world: &mut World, entity: Entity, owner: EntityOwner) {
        let initial_manifest = ComponentManifest::from_world(world, entity).unwrap();
        let claim = world
            .resource_mut::<RouteEntityRegistry>()
            .register(
                entity,
                RegistryOwner::Committed(owner),
                initial_manifest.clone(),
                EntityLifecycle::PreparedInert,
            )
            .unwrap();
        world
            .entity_mut(entity)
            .insert(RouteOwnershipSeal::new(claim.seal()));
        let manifest = ComponentManifest::from_world(world, entity).unwrap();
        assert!(
            world
                .resource_mut::<RouteEntityRegistry>()
                .compare_exchange_manifest_exact(&claim, &initial_manifest, manifest)
        );
    }

    fn two_level_fixture() -> (
        World,
        RouteFrameVersion,
        RouteFrameVersion,
        Entity,
        Entity,
        Entity,
        Entity,
        Entity,
    ) {
        let parent = frame_fixture("frame-local-parent");
        let child = frame_fixture("frame-local-child");
        let mut world = World::new();
        world.insert_resource(RouteEntityRegistry::new());
        let parent_root = world.spawn_empty().id();
        let parent_decoration = world.spawn_empty().id();
        let child_slot = world.spawn_empty().id();
        let child_root = world.spawn_empty().id();
        let child_button = world.spawn_empty().id();

        world.entity_mut(parent_root).add_child(child_slot);
        world.entity_mut(parent_root).add_child(parent_decoration);
        world.entity_mut(child_slot).add_child(child_root);
        world.entity_mut(child_root).add_child(child_button);
        world.flush();

        register_owned(
            &mut world,
            parent_root,
            EntityOwner::FrameShell(parent.clone()),
        );
        register_owned(
            &mut world,
            parent_decoration,
            EntityOwner::FrameOutput {
                frame: parent.clone(),
                output: 1,
                local_index: 0,
            },
        );
        register_owned(
            &mut world,
            child_slot,
            EntityOwner::ChildSlot {
                parent: parent.clone(),
                outlet: RouteSlotKey::default(),
                slot: child.slot(),
            },
        );
        register_owned(
            &mut world,
            child_root,
            EntityOwner::FrameShell(child.clone()),
        );
        register_owned(
            &mut world,
            child_button,
            EntityOwner::FrameOutput {
                frame: child.clone(),
                output: 2,
                local_index: 0,
            },
        );

        (
            world,
            parent,
            child,
            parent_root,
            parent_decoration,
            child_slot,
            child_root,
            child_button,
        )
    }

    #[test]
    fn parent_traversal_stops_at_exact_child_slot_boundary() {
        let (
            world,
            parent,
            child,
            parent_root,
            parent_decoration,
            child_slot,
            child_root,
            child_button,
        ) = two_level_fixture();
        let registry = world.resource::<RouteEntityRegistry>();

        assert_eq!(
            collect_frame_local_entities(&world, &parent, parent_root, registry).unwrap(),
            vec![parent_root, parent_decoration, child_slot]
        );
        assert_eq!(
            collect_frame_local_entities(&world, &child, child_root, registry).unwrap(),
            vec![child_root, child_button]
        );
    }

    #[test]
    fn frame_local_order_uses_stable_registry_identity() {
        let (world, parent, _, parent_root, parent_decoration, child_slot, _, _) =
            two_level_fixture();
        let physical = world
            .get::<bevy_ecs::hierarchy::Children>(parent_root)
            .unwrap()
            .iter()
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(physical, vec![child_slot, parent_decoration]);
        assert_eq!(
            collect_frame_local_entities(
                &world,
                &parent,
                parent_root,
                world.resource::<RouteEntityRegistry>()
            )
            .unwrap(),
            vec![parent_root, parent_decoration, child_slot]
        );
    }

    #[test]
    fn frame_local_cycle_is_world_zero() {
        let parent = frame_fixture("frame-local-cycle");
        let mut world = World::new();
        world.insert_resource(RouteEntityRegistry::new());
        let root = world.spawn_empty().id();
        let child = world.spawn_empty().id();
        world.entity_mut(root).add_child(child);
        world.entity_mut(root).insert(ChildOf(child));
        world.flush();
        register_owned(&mut world, root, EntityOwner::FrameShell(parent.clone()));
        register_owned(
            &mut world,
            child,
            EntityOwner::FrameOutput {
                frame: parent.clone(),
                output: 3,
                local_index: 0,
            },
        );
        let root_manifest = ComponentManifest::from_world(&world, root).unwrap();
        let child_manifest = ComponentManifest::from_world(&world, child).unwrap();

        assert_eq!(
            collect_frame_local_entities(
                &world,
                &parent,
                root,
                world.resource::<RouteEntityRegistry>()
            ),
            Err(FrameLocalError::RelationshipCycle(root))
        );
        assert_eq!(
            ComponentManifest::from_world(&world, root),
            Some(root_manifest)
        );
        assert_eq!(
            ComponentManifest::from_world(&world, child),
            Some(child_manifest)
        );
    }

    #[test]
    fn foreign_child_slot_is_rejected_before_mutation() {
        let parent = frame_fixture("frame-local-owned-root");
        let foreign = frame_fixture("frame-local-foreign-slot");
        let mut world = World::new();
        world.insert_resource(RouteEntityRegistry::new());
        let parent_root = world.spawn_empty().id();
        let child_slot = world.spawn_empty().id();
        world.entity_mut(parent_root).add_child(child_slot);
        world.flush();
        register_owned(
            &mut world,
            parent_root,
            EntityOwner::FrameShell(parent.clone()),
        );
        register_owned(
            &mut world,
            child_slot,
            EntityOwner::ChildSlot {
                parent: foreign,
                outlet: RouteSlotKey::default(),
                slot: parent.slot(),
            },
        );
        assert_eq!(
            collect_frame_local_entities(
                &world,
                &parent,
                parent_root,
                world.resource::<RouteEntityRegistry>()
            ),
            Err(FrameLocalError::ForeignChildSlot(child_slot))
        );
        assert!(world.get_entity(child_slot).is_ok());
    }

    #[test]
    fn observed_focus_closure_rejects_registered_foreign_authority() {
        let (mut app, branch, _, _, parent_entities) =
            crate::navigation::tests::failed_nested_branch_fixture();
        let foreign = frame_fixture("frame-local-foreign-authority");
        let parent = *parent_entities
            .last()
            .expect("nested parent retains a realized entity");
        let foreign_root = app.world_mut().spawn(ChildOf(parent)).id();
        register_owned(
            app.world_mut(),
            foreign_root,
            EntityOwner::FrameShell(foreign),
        );

        assert_eq!(
            validate_branch_descendants(app.world(), branch.realized_for_test()),
            Err(FrameLocalError::OwnershipDrift(foreign_root))
        );
    }

    #[test]
    fn observed_focus_closure_includes_unregistered_foreign_descendants() {
        let (mut app, branch, _, child_slot, parent_entities) =
            crate::navigation::tests::failed_nested_branch_fixture();
        let parent = *parent_entities
            .last()
            .expect("nested parent retains a realized entity");
        let foreign = app.world_mut().spawn(ChildOf(parent)).id();
        let foreign_leaf = app.world_mut().spawn(ChildOf(foreign)).id();

        let closure = validate_branch_descendants(app.world(), branch.realized_for_test()).unwrap();
        assert!(closure.contains(&foreign));
        assert!(closure.contains(&foreign_leaf));
        assert!(closure.contains(&child_slot));
    }

    #[test]
    fn retained_empty_structure_uses_exact_binding_instead_of_positional_count() {
        let (_app, branch, _, _, _) = crate::navigation::tests::failed_nested_branch_fixture();
        let mut retained = branch.into_realized_for_test();
        let removed = retained
            .pop_frame_for_test()
            .expect("nested fixture has one occupied child frame");
        let slot = &retained.child_slots()[0];
        assert!(removed.mount_binding().matches_mounted_child_slot(slot));

        assert_eq!(validate_branch_structure(&retained), Ok(()));
    }
}
