//! Input boundary observations. Batch-derived facts never outlive immutable World access.

use std::collections::HashSet;

use bevy_ecs::{
    entity::{Entity, EntityHashMap, EntityHashSet},
    hierarchy::ChildOf,
    world::World,
};

use super::RouterInputGate;
use crate::{
    components::RouteOwnershipSeal,
    registry::{QuarantineLeaseTable, RouteEntityRegistry},
};

pub(super) fn target_is_denied_router_input(world: &World, target: Entity) -> bool {
    world
        .get_resource::<RouterInputGate>()
        .is_some_and(|gate| !gate.allows_pointer(target))
        && target_has_exact_router_boundary(world, target)
}

fn target_has_exact_router_boundary(world: &World, target: Entity) -> bool {
    let Some(registry) = world.get_resource::<RouteEntityRegistry>() else {
        return false;
    };
    let Some(quarantine) = world.get_resource::<QuarantineLeaseTable>() else {
        return false;
    };
    let mut current = target;
    let mut visited = HashSet::new();
    while visited.insert(current) {
        if entity_has_exact_router_claim(world, registry, quarantine, current) {
            return true;
        }
        let Some(parent) = world.get::<ChildOf>(current).map(ChildOf::parent) else {
            return false;
        };
        current = parent;
    }
    false
}

/// Observe a batch against one immutable World. Candidate membership and
/// shared ancestry are derived once for this call, never reused after mutation.
/// Small pointer-history batches keep the single-target predicate above.
pub(super) struct RouterInputObservation<'w> {
    world: &'w World,
    registry: &'w RouteEntityRegistry,
    quarantine: &'w QuarantineLeaseTable,
    allowed: EntityHashSet,
    // None marks the current path; finding it again closes an unclaimed cycle.
    boundaries: EntityHashMap<Option<bool>>,
    path: Vec<Entity>,
}

impl<'w> RouterInputObservation<'w> {
    pub(super) fn new(world: &'w World) -> Option<Self> {
        Some(Self {
            world,
            registry: world.get_resource::<RouteEntityRegistry>()?,
            quarantine: world.get_resource::<QuarantineLeaseTable>()?,
            allowed: world
                .get_resource::<RouterInputGate>()?
                .allowed_projection()
                .iter()
                .copied()
                .collect(),
            boundaries: EntityHashMap::default(),
            path: Vec::new(),
        })
    }

    pub(super) fn target_is_denied(&mut self, target: Entity) -> bool {
        if self.allowed.contains(&target) {
            return false;
        }
        self.path.clear();
        let mut current = target;
        let boundary = loop {
            if let Some(result) = self.boundaries.get(&current) {
                break result.unwrap_or(false);
            }
            self.boundaries.insert(current, None);
            self.path.push(current);
            if entity_has_exact_router_claim(self.world, self.registry, self.quarantine, current) {
                break true;
            }
            let Some(parent) = self.world.get::<ChildOf>(current).map(ChildOf::parent) else {
                break false;
            };
            current = parent;
        };
        for entity in self.path.drain(..) {
            self.boundaries.insert(entity, Some(boundary));
        }
        boundary
    }
}

fn entity_has_exact_router_claim(
    world: &World,
    registry: &RouteEntityRegistry,
    quarantine: &QuarantineLeaseTable,
    entity: Entity,
) -> bool {
    let claim = registry
        .record(entity)
        .map(|record| record.claim())
        .or_else(|| quarantine.record(entity).map(|record| record.claim()))
        .or_else(|| {
            quarantine
                .unresolved_record(entity)
                .map(|record| record.claim())
        });
    claim.is_some_and(|claim| {
        world.get::<RouteOwnershipSeal>(entity) == Some(&RouteOwnershipSeal::new(claim.seal()))
    })
}
