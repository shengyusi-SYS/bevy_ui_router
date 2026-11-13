//! Ordered input permissions and prepared changes for one publication.

use bevy_ecs::{
    change_detection::Mut,
    entity::{Entity, EntityHashSet},
    resource::Resource,
};

#[derive(Resource)]
pub struct RouterInputGate {
    pub(super) allowed: Vec<Entity>,
    /// Targets that were previously authorized and must have all retained
    /// native pointer state removed before the next dispatch.
    pub(super) revoked: Vec<Entity>,
}

impl RouterInputGate {
    pub(crate) fn new() -> Self {
        Self {
            allowed: Vec::new(),
            revoked: Vec::new(),
        }
    }

    pub fn allows_pointer(&self, target: Entity) -> bool {
        self.allowed.contains(&target)
    }

    pub fn allows_keyboard(&self, target: Entity) -> bool {
        self.allowed.contains(&target)
    }

    pub fn allows_gamepad(&self, target: Entity) -> bool {
        self.allowed.contains(&target)
    }

    pub(crate) fn allowed_projection(&self) -> &[Entity] {
        &self.allowed
    }

    fn replace_allowed(&mut self, next: Vec<Entity>, revoked: Vec<Entity>) {
        self.allowed = next;
        self.revoked = revoked;
    }

    pub(super) fn refresh_allowed(
        &mut self,
        allowed: &EntityHashSet,
        grants: &EntityHashSet,
    ) -> bool {
        if self.allowed.iter().all(|entity| allowed.contains(entity))
            && (self.allowed.len() == allowed.len()
                || grants
                    .iter()
                    .all(|entity| !allowed.contains(entity) || self.allowed.contains(entity)))
            && self.revoked.iter().all(|entity| !allowed.contains(entity))
        {
            return false;
        }
        for entity in self.allowed.iter().copied() {
            if !allowed.contains(&entity) && !self.revoked.contains(&entity) {
                self.revoked.push(entity);
            }
        }
        self.allowed.retain(|entity| allowed.contains(entity));
        self.allowed.extend(
            grants
                .iter()
                .copied()
                .filter(|entity| allowed.contains(entity)),
        );
        self.allowed.sort_by_key(|entity| entity.to_bits());
        self.allowed.dedup();
        self.revoked.retain(|entity| !allowed.contains(entity));
        true
    }
}

#[derive(Clone)]
pub(crate) struct PreparedInputGateIsolation {
    next: Vec<Entity>,
    revoked: Vec<Entity>,
}

impl PreparedInputGateIsolation {
    pub(crate) fn revoked_targets(&self) -> &[Entity] {
        &self.revoked
    }

    pub(crate) fn apply(self, gate: &mut RouterInputGate) {
        gate.replace_allowed(self.next, self.revoked);
    }
}

pub(crate) fn try_prepare_input_gate_isolation(
    gate: &RouterInputGate,
    targets: &[Entity],
) -> Result<PreparedInputGateIsolation, ()> {
    let mut next = Vec::new();
    next.try_reserve_exact(gate.allowed.len()).map_err(|_| ())?;
    next.extend(gate.allowed.iter().copied());
    let targets = targets.iter().copied().collect::<EntityHashSet>();
    next.retain(|entity| !targets.contains(entity));
    let next_members = next.iter().copied().collect::<EntityHashSet>();
    let revoked = try_prepare_pointer_revocations(gate, &next_members)?;
    Ok(PreparedInputGateIsolation { next, revoked })
}

/// Prepared values only. Binding checks the complete source before taking the
/// mutable receiver; publication can prepare its other receivers in between.
pub(crate) struct InputGateChange {
    before_allowed: Vec<Entity>,
    before_revoked: Vec<Entity>,
    next: Vec<Entity>,
    revoked: Vec<Entity>,
    changed: bool,
}

impl InputGateChange {
    pub(crate) fn prepare(
        gate: &RouterInputGate,
        remove: &[Entity],
        add: Vec<Entity>,
    ) -> Option<Self> {
        let mut members = gate.allowed.iter().copied().collect::<EntityHashSet>();
        let mut removals = EntityHashSet::default();
        for &entity in remove {
            if !members.contains(&entity) || !removals.insert(entity) {
                return None;
            }
        }
        let mut additions = EntityHashSet::default();
        for &entity in &add {
            if !additions.insert(entity)
                || (members.contains(&entity) && !removals.contains(&entity))
            {
                return None;
            }
        }

        let mut next = gate.allowed.clone();
        next.retain(|entity| !removals.contains(entity));
        next.extend(add);
        // The untouched prefix already belongs to the old membership set.
        let same_permissions = gate.allowed.len() == next.len()
            && additions.iter().all(|entity| members.contains(entity));
        members.retain(|entity| !removals.contains(entity));
        members.extend(additions);
        let revoked = try_prepare_pointer_revocations(gate, &members).ok()?;
        let changed = !same_permissions || gate.revoked != revoked;
        Some(Self {
            before_allowed: gate.allowed.clone(),
            before_revoked: gate.revoked.clone(),
            next,
            revoked,
            changed,
        })
    }

    pub(crate) fn revoked_targets(&self) -> &[Entity] {
        &self.revoked
    }

    pub(crate) fn bind<'w>(
        self,
        gate: Mut<'w, RouterInputGate>,
    ) -> Option<PreparedInputGateMutation<'w>> {
        if gate.allowed != self.before_allowed || gate.revoked != self.before_revoked {
            return None;
        }
        Some(PreparedInputGateMutation {
            gate,
            next: self.next,
            revoked: self.revoked,
            changed: self.changed,
        })
    }
}

pub(crate) struct PreparedInputGateMutation<'w> {
    gate: Mut<'w, RouterInputGate>,
    next: Vec<Entity>,
    revoked: Vec<Entity>,
    changed: bool,
}

impl PreparedInputGateMutation<'_> {
    pub(crate) fn apply(mut self) {
        if self.changed {
            self.gate.replace_allowed(self.next, self.revoked);
        }
    }
}

fn try_prepare_pointer_revocations(
    gate: &RouterInputGate,
    next: &EntityHashSet,
) -> Result<Vec<Entity>, ()> {
    let capacity = gate
        .revoked
        .len()
        .checked_add(gate.allowed.len())
        .ok_or(())?;
    let mut revoked = Vec::new();
    revoked.try_reserve_exact(capacity).map_err(|_| ())?;
    revoked.extend(gate.revoked.iter().copied());
    let mut members = gate.revoked.iter().copied().collect::<EntityHashSet>();
    for entity in gate.allowed.iter().copied() {
        if !next.contains(&entity) && members.insert(entity) {
            revoked.push(entity);
        }
    }
    revoked.retain(|entity| !next.contains(entity));
    Ok(revoked)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::world::World;

    #[test]
    fn change_rejects_missing_removals_duplicates_and_existing_additions() {
        let mut world = World::new();
        let existing = world.spawn_empty().id();
        let other = world.spawn_empty().id();
        let gate = RouterInputGate {
            allowed: vec![existing],
            revoked: Vec::new(),
        };
        for (remove, add) in [
            (vec![other], Vec::new()),
            (vec![existing, existing], Vec::new()),
            (Vec::new(), vec![other, other]),
            (Vec::new(), vec![existing]),
        ] {
            assert!(InputGateChange::prepare(&gate, &remove, add).is_none());
        }
    }

    #[test]
    fn binding_rechecks_both_source_projections_without_mutating_them() {
        for change_revoked in [false, true] {
            let mut world = World::new();
            let existing = world.spawn_empty().id();
            let entering = world.spawn_empty().id();
            let unrelated = world.spawn_empty().id();
            world.insert_resource(RouterInputGate {
                allowed: vec![existing],
                revoked: Vec::new(),
            });
            let change = InputGateChange::prepare(
                world.resource::<RouterInputGate>(),
                &[existing],
                vec![entering],
            )
            .unwrap();
            {
                let mut gate = world.resource_mut::<RouterInputGate>();
                if change_revoked {
                    gate.revoked.push(unrelated);
                } else {
                    gate.allowed.push(unrelated);
                }
            }
            let source = {
                let gate = world.resource::<RouterInputGate>();
                (gate.allowed.clone(), gate.revoked.clone())
            };
            assert!(
                change
                    .bind(world.resource_mut::<RouterInputGate>())
                    .is_none()
            );
            let gate = world.resource::<RouterInputGate>();
            assert_eq!((&gate.allowed, &gate.revoked), (&source.0, &source.1));
        }
    }
}
