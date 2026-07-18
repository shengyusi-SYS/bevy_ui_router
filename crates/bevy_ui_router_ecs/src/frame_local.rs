use std::collections::HashSet;

use bevy_ecs::{
    hierarchy::{ChildOf, Children},
    prelude::{Entity, World},
};
use bevy_ui_router_core::frame::RouteFrameVersion;

use crate::{
    branch_transaction::RealizedBranch,
    components::RouteOwnershipSeal,
    mount::{validate_leaving_mounted_child_slot, validate_mounted_child_slot},
    presentation::{
        validate_active_frame_with_child_slots, validate_leaving_frame_with_child_slots,
        validate_realized_frame_with_child_slots,
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
        )?;
    }
    validate_branch_descendants(world, branch).map(|_| ())
}

pub(crate) fn validate_committed_branch(
    world: &World,
    branch: &RealizedBranch,
    leaving: bool,
) -> Result<Vec<Entity>, FrameLocalError> {
    validate_branch_structure(branch)?;
    for slot in branch.child_slots() {
        let valid = if leaving {
            validate_leaving_mounted_child_slot(world, slot)
        } else {
            validate_mounted_child_slot(world, slot, EntityLifecycle::Active)
        };
        if !valid {
            return Err(FrameLocalError::OwnershipDrift(slot.slot()));
        }
    }
    for frame in branch.frames() {
        let child_slots = child_slot_expectations(branch, frame.frame());
        let valid = if leaving {
            validate_leaving_frame_with_child_slots(world, frame, &child_slots)
        } else {
            validate_active_frame_with_child_slots(world, frame, &child_slots)
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
        )?;
    }
    validate_branch_descendants(world, branch)
}

fn validate_branch_structure(branch: &RealizedBranch) -> Result<(), FrameLocalError> {
    if branch.frames().is_empty() {
        return branch.child_slots().first().map_or(Ok(()), |slot| {
            Err(FrameLocalError::ForeignChildSlot(slot.slot()))
        });
    }
    if branch.frames().len() != branch.child_slots().len() + 1 {
        return Err(FrameLocalError::OwnershipDrift(branch.frames()[0].shell()));
    }
    Ok(())
}

fn child_slot_expectations(
    branch: &RealizedBranch,
    frame: &RouteFrameVersion,
) -> Vec<(Entity, Vec<Entity>)> {
    branch
        .child_slots()
        .iter()
        .filter(|slot| slot.parent() == frame)
        .map(|slot| (slot.anchor(), slot.expected_anchor_children().to_vec()))
        .collect()
}

fn validate_frame_claim_set<'a>(
    world: &World,
    frame: &RouteFrameVersion,
    root: Entity,
    claims: impl Iterator<Item = &'a crate::registry::ExactRegistryClaim>,
    branch: &RealizedBranch,
    manifest_policy: ManifestPolicy,
) -> Result<(), FrameLocalError> {
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
    let boundaries = branch
        .child_slots()
        .iter()
        .filter(|slot| slot.parent() == frame)
        .map(|slot| slot.slot())
        .collect::<HashSet<_>>();
    let actual = physical.into_iter().collect::<HashSet<_>>();
    if actual.len() != expected.len() + boundaries.len()
        || !expected.is_subset(&actual)
        || !boundaries.is_subset(&actual)
    {
        return Err(FrameLocalError::OwnershipDrift(root));
    }
    Ok(())
}

fn validate_branch_descendants(
    world: &World,
    branch: &RealizedBranch,
) -> Result<Vec<Entity>, FrameLocalError> {
    if branch.frames().is_empty() {
        return Ok(Vec::new());
    }
    let registry = world
        .get_resource::<RouteEntityRegistry>()
        .ok_or_else(|| FrameLocalError::MissingRoot(branch.frames()[0].shell()))?;
    let mut ordered = Vec::new();
    let mut branch_seen = HashSet::new();
    for frame in branch.frames() {
        let local = collect_frame_focus_unit(world, frame.frame(), frame.shell(), registry)?;
        extend_focus_closure(&mut ordered, &mut branch_seen, local)?;
    }
    for (index, slot) in branch.child_slots().iter().enumerate() {
        let child = &branch.frames()[index + 1];
        let foreign = collect_child_slot_foreign_focus(
            world,
            slot.slot(),
            child.frame(),
            child.shell(),
            registry,
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
        for child in &children {
            if world.get::<ChildOf>(*child).map(ChildOf::parent) != Some(entity) {
                return Err(FrameLocalError::OwnershipDrift(*child));
            }
        }
        children.sort_by_key(|child| {
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
        if entity == child_root {
            continue;
        }
        if !visited.insert(entity) {
            return Err(FrameLocalError::RelationshipCycle(entity));
        }
        if registry.record(entity).is_some() {
            return Err(FrameLocalError::OwnershipDrift(entity));
        }
        ordered.push(entity);
        let mut children = world
            .get::<Children>(entity)
            .map(|children| children.iter().copied().collect::<Vec<_>>())
            .unwrap_or_default();
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
        let seal = world.resource_mut::<RouteEntityRegistry>().issue_seal();
        world
            .entity_mut(entity)
            .insert(RouteOwnershipSeal::new(seal));
        let manifest = ComponentManifest::from_world(world, entity).unwrap();
        world
            .resource_mut::<RouteEntityRegistry>()
            .register(
                entity,
                RegistryOwner::Committed(owner),
                seal,
                manifest,
                EntityLifecycle::PreparedInert,
            )
            .unwrap();
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
}
