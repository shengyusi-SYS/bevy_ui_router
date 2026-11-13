//! Facts shared by public queries during one immutable parameter borrow.

use std::cell::{OnceCell, RefCell};

use bevy_ecs::{
    entity::{EntityHashMap, EntityHashSet},
    hierarchy::Children,
    prelude::Entity,
};

use super::{ActiveRouteScope, ExactRegistryClaim, RealizedFrame, RouteEntityRecord, RouterEntry};

type Resolved<'a> = Option<(&'a RouterEntry, &'a RealizedFrame)>;

#[derive(Clone, Copy)]
pub(super) struct Boundary<'a> {
    pub entry: &'a RouterEntry,
    pub frame: &'a RealizedFrame,
    pub claim: &'a ExactRegistryClaim,
}

#[derive(Default)]
pub(super) struct ReadFacts<'a> {
    records: OnceCell<EntityHashMap<&'a RouteEntityRecord>>,
    boundaries: OnceCell<EntityHashMap<Boundary<'a>>>,
    children: RefCell<EntityHashMap<EntityHashSet>>,
    frames: RefCell<Vec<FrameResult<'a>>>,
    targets: RefCell<EntityHashMap<Resolved<'a>>>,
}

struct FrameResult<'a> {
    entry: &'a RouterEntry,
    frame: &'a RealizedFrame,
    valid: bool,
}

impl<'a> ReadFacts<'a> {
    pub fn record(
        &self,
        scope: &'a ActiveRouteScope<'_, '_>,
        entity: Entity,
    ) -> Option<&'a RouteEntityRecord> {
        self.records
            .get_or_init(|| {
                let mut records = EntityHashMap::default();
                for record in scope.registry.records() {
                    // Match the original linear lookup's first-record semantics.
                    records.entry(record.entity()).or_insert(record);
                }
                records
            })
            .get(&entity)
            .copied()
    }

    pub fn boundary(
        &self,
        scope: &'a ActiveRouteScope<'_, '_>,
        entity: Entity,
    ) -> Option<Boundary<'a>> {
        self.boundaries
            .get_or_init(|| {
                let mut boundaries = EntityHashMap::default();
                for boundary in scope.boundaries() {
                    boundaries
                        .entry(boundary.claim.entity())
                        .or_insert(boundary);
                }
                boundaries
            })
            .get(&entity)
            .copied()
    }

    pub fn contains_child(&self, parent: Entity, children: &Children, child: Entity) -> bool {
        self.children
            .borrow_mut()
            .entry(parent)
            .or_insert_with(|| children.iter().copied().collect())
            .contains(&child)
    }

    pub fn frame_result(&self, entry: &RouterEntry, frame: &RealizedFrame) -> Option<bool> {
        self.frames
            .borrow()
            .iter()
            .find(|cached| std::ptr::eq(cached.entry, entry) && std::ptr::eq(cached.frame, frame))
            .map(|cached| cached.valid)
    }

    pub fn remember_frame(&self, entry: &'a RouterEntry, frame: &'a RealizedFrame, valid: bool) {
        // Only completed proofs from a fresh mount-chain walk are stored.
        // References name the exact borrowed objects, never a reusable shell ID.
        self.frames.borrow_mut().push(FrameResult {
            entry,
            frame,
            valid,
        });
    }

    pub fn target_result(&self, target: Entity) -> Option<Resolved<'a>> {
        self.targets.borrow().get(&target).copied()
    }

    pub fn remember_targets(
        &self,
        target: Entity,
        ordinary: EntityHashSet,
        resolved: Resolved<'a>,
    ) {
        let mut targets = self.targets.borrow_mut();
        if ordinary.is_empty() {
            // Registered, quarantined, and missing targets stop before an
            // ordinary path starts, but still retain their direct answer.
            targets.insert(target, resolved);
        } else {
            // The first ordinary node is already the original target.
            targets.extend(ordinary.into_iter().map(|entity| (entity, resolved)));
        }
    }
}

pub(crate) struct ActiveScopeRead<'a, 'w, 's> {
    scope: &'a ActiveRouteScope<'w, 's>,
    facts: ReadFacts<'a>,
}

impl<'a, 'w, 's> ActiveScopeRead<'a, 'w, 's> {
    pub(super) fn new(scope: &'a ActiveRouteScope<'w, 's>) -> Self {
        Self {
            scope,
            facts: ReadFacts::default(),
        }
    }

    pub(crate) fn resolve(&self, target: Entity) -> Resolved<'a> {
        if let Some(resolved) = self.facts.target_result(target) {
            return resolved;
        }
        self.scope.resolve_with_facts(target, Some(&self.facts))
    }
}

#[cfg(test)]
mod tests {
    use crate::{RouteActivity, RouteQueries};
    use bevy_ecs::{hierarchy::ChildOf, system::RunSystemOnce};

    #[test]
    fn direct_registered_output_does_not_authorize_its_ordinary_descendants() {
        let (mut app, output) = crate::input::tests::active_atomic_app("query-reader");
        let child = app.world_mut().spawn(ChildOf(output)).id();
        let grandchild = app.world_mut().spawn(ChildOf(child)).id();
        app.world_mut()
            .run_system_once(move |queries: RouteQueries, activity: RouteActivity| {
                for targets in [[output, child, grandchild], [grandchild, child, output]] {
                    let routes = queries.read();
                    for target in targets {
                        assert_eq!(routes.is_active(target), target == output);
                        assert_eq!(routes.is_active(target), activity.is_active(target));
                    }
                }
            })
            .unwrap();
    }
}
