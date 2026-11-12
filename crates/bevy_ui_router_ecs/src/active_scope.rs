mod read;

pub(crate) use read::ActiveScopeRead;
use read::{Boundary, ReadFacts};

use bevy_ecs::{
    entity::EntityHashSet,
    hierarchy::{ChildOf, Children},
    prelude::{Entity, Query, Res},
    query::QueryData,
    system::SystemParam,
};

use crate::{
    app_ext::RouterHostRegistration,
    components::{
        ChildSlotAuthority, ChildSlotLifecycle, ProvisionalSlotLifecycle, RouteOwnershipSeal,
    },
    navigation::{RouterEntry, RouterNavigation},
    presentation::{FrameContentTarget, FrameLifecycle, FrameOutput, FrameShell, RealizedFrame},
    registry::{
        EntityLifecycle, EntityOwner, ExactRegistryClaim, QuarantineLeaseTable, RegistryOwner,
        RouteEntityRecord, RouteEntityRegistry,
    },
};

#[derive(QueryData)]
struct ActiveScopeEntity {
    parent: Option<&'static ChildOf>,
    children: Option<&'static Children>,
    seal: Option<&'static RouteOwnershipSeal>,
    lifecycle: Option<&'static FrameLifecycle>,
    shell: Option<&'static FrameShell>,
    content: Option<&'static FrameContentTarget>,
    output: Option<&'static FrameOutput>,
    slot_authority: Option<&'static ChildSlotAuthority>,
    slot_lifecycle: Option<&'static ChildSlotLifecycle>,
    top_slot_lifecycle: Option<&'static ProvisionalSlotLifecycle>,
    host: Option<&'static RouterHostRegistration>,
}

/// Resolves current content to its exact logically and physically active frame.
///
/// This shared read-only proof observes dynamic descendants through live hierarchy
/// and stops at the nearest registered boundary. It reads only Router metadata and
/// hierarchy, not arbitrary application state, visibility or input eligibility.
#[derive(SystemParam)]
pub(crate) struct ActiveRouteScope<'w, 's> {
    navigation: Res<'w, RouterNavigation>,
    registry: Res<'w, RouteEntityRegistry>,
    quarantine: Res<'w, QuarantineLeaseTable>,
    entities: Query<'w, 's, ActiveScopeEntity>,
    #[cfg(feature = "bsn")]
    native_roots: Query<
        'w,
        's,
        (),
        bevy_ecs::query::With<crate::presentation::native_scene::NativeContentRoot>,
    >,
}

impl<'w, 's> ActiveRouteScope<'w, 's> {
    pub(crate) fn read(&self) -> ActiveScopeRead<'_, 'w, 's> {
        ActiveScopeRead::new(self)
    }

    /// Resolves the exact nearest published frame; invalid boundaries stop lookup.
    pub(crate) fn resolve(&self, target: Entity) -> Option<(&RouterEntry, &RealizedFrame)> {
        self.resolve_with_facts(target, None)
    }

    fn resolve_with_facts<'a>(
        &'a self,
        target: Entity,
        facts: Option<&ReadFacts<'a>>,
    ) -> Option<(&'a RouterEntry, &'a RealizedFrame)> {
        let mut current = target;
        let mut seen = EntityHashSet::default();
        while seen.insert(current) {
            if self.is_quarantined(current) || self.entities.get(current).is_err() {
                return None;
            }
            if let Some(record) = self.record(current, facts) {
                // The nearest registered boundary controls inheritance. A direct
                // registered target and an ordinary descendant have different roles.
                let Boundary {
                    entry,
                    frame,
                    claim,
                } = self.boundary(current, facts)?;
                if claim.lifecycle() != EntityLifecycle::Active
                    || !record.matches_claim(claim)
                    || !self.frame_scope_is_current(entry, frame, facts)
                {
                    return None;
                }
                let valid = if current == target {
                    self.registered_content_is_current(frame, target, facts)
                } else {
                    frame.native_root() == Some(current)
                        && self.is_native_root(current)
                        && self.claim_is_current(claim, facts)
                };
                return valid.then_some((entry, frame));
            }
            let entity = self.entities.get(current).ok()?;
            let parent = entity.parent.map(ChildOf::parent)?;
            if !self.edge_is_current(current, parent, facts) {
                return None;
            }
            current = parent;
        }
        None
    }

    fn record<'a>(
        &'a self,
        entity: Entity,
        facts: Option<&ReadFacts<'a>>,
    ) -> Option<&'a RouteEntityRecord> {
        match facts {
            Some(facts) => facts.record(self, entity),
            None => self.registry.record(entity),
        }
    }

    fn boundary<'a>(
        &'a self,
        entity: Entity,
        facts: Option<&ReadFacts<'a>>,
    ) -> Option<Boundary<'a>> {
        if let Some(facts) = facts {
            return facts.boundary(self, entity);
        }
        self.boundaries()
            .find(|boundary| boundary.claim.entity() == entity)
    }

    fn boundaries(&self) -> impl Iterator<Item = Boundary<'_>> {
        self.navigation.entries.iter().flat_map(|entry| {
            entry.active.iter().flat_map(move |active| {
                active.frames().iter().flat_map(move |frame| {
                    frame.claims().map(move |claim| Boundary {
                        entry,
                        frame,
                        claim,
                    })
                })
            })
        })
    }

    fn is_quarantined(&self, entity: Entity) -> bool {
        self.quarantine.record(entity).is_some()
            || self.quarantine.unresolved_record(entity).is_some()
    }

    fn is_native_root(&self, entity: Entity) -> bool {
        #[cfg(feature = "bsn")]
        {
            self.native_roots.contains(entity)
        }
        #[cfg(not(feature = "bsn"))]
        {
            let _ = entity;
            false
        }
    }

    fn edge_is_current(
        &self,
        child: Entity,
        parent: Entity,
        facts: Option<&ReadFacts<'_>>,
    ) -> bool {
        self.entities
            .get(child)
            .is_ok_and(|entity| entity.parent.map(ChildOf::parent) == Some(parent))
            && self.entities.get(parent).is_ok_and(|entity| {
                entity.children.is_some_and(|children| match facts {
                    Some(facts) => facts.contains_child(parent, children, child),
                    None => children.contains(&child),
                })
            })
    }

    fn claim_is_current<'a>(
        &'a self,
        claim: &ExactRegistryClaim,
        facts: Option<&ReadFacts<'a>>,
    ) -> bool {
        if self.is_quarantined(claim.entity())
            || !self
                .record(claim.entity(), facts)
                .is_some_and(|record| record.matches_claim(claim))
        {
            return false;
        }
        let Ok(facts) = self.entities.get(claim.entity()) else {
            return false;
        };
        if facts.seal.map(|seal| seal.seal()) != Some(claim.seal()) {
            return false;
        }
        match claim.owner() {
            RegistryOwner::Committed(EntityOwner::FrameShell(frame)) => {
                facts.lifecycle.map(|lifecycle| lifecycle.0) == Some(claim.lifecycle())
                    && facts.shell.is_some_and(|shell| shell.matches_frame(frame))
            }
            RegistryOwner::Committed(EntityOwner::FrameContentTarget(frame)) => {
                facts.lifecycle.map(|lifecycle| lifecycle.0) == Some(claim.lifecycle())
                    && facts
                        .content
                        .is_some_and(|content| content.matches_frame(frame))
            }
            RegistryOwner::Committed(EntityOwner::FrameOutput {
                frame, local_index, ..
            }) => {
                facts.lifecycle.map(|lifecycle| lifecycle.0) == Some(claim.lifecycle())
                    && facts
                        .output
                        .is_some_and(|output| output.matches_owner(frame, *local_index))
            }
            RegistryOwner::Committed(EntityOwner::ChildSlot {
                parent,
                outlet,
                slot,
            }) => {
                let lifecycle = match claim.lifecycle() {
                    EntityLifecycle::PreparedInert => ChildSlotLifecycle::RegisteredInert,
                    EntityLifecycle::Active => ChildSlotLifecycle::Active,
                    EntityLifecycle::Retained => ChildSlotLifecycle::Retained,
                    EntityLifecycle::Leaving => ChildSlotLifecycle::Leaving,
                    EntityLifecycle::Quarantined => ChildSlotLifecycle::Quarantined,
                };
                facts.slot_lifecycle == Some(&lifecycle)
                    && facts.slot_authority.is_some_and(|authority| {
                        authority.parent() == parent
                            && authority.outlet() == outlet
                            && authority.slot() == *slot
                    })
            }
            RegistryOwner::Committed(EntityOwner::HostTopSlot { .. }) => {
                claim.lifecycle() == EntityLifecycle::PreparedInert
                    && facts.top_slot_lifecycle == Some(&ProvisionalSlotLifecycle::PreparedInert)
            }
            RegistryOwner::Provisional(_) => false,
        }
    }

    fn frame_scope_is_current<'a>(
        &'a self,
        entry: &'a RouterEntry,
        frame: &'a RealizedFrame,
        facts: Option<&ReadFacts<'a>>,
    ) -> bool {
        if let Some(valid) = facts.and_then(|facts| facts.frame_result(entry, frame)) {
            return valid;
        }
        let valid = self.frame_scope_is_current_with_parents(
            entry,
            frame,
            &mut EntityHashSet::default(),
            facts,
        );
        if let Some(facts) = facts {
            facts.remember_frame(entry, frame, valid);
        }
        valid
    }

    fn frame_scope_is_current_with_parents<'a>(
        &'a self,
        entry: &RouterEntry,
        frame: &RealizedFrame,
        seen: &mut EntityHashSet,
        facts: Option<&ReadFacts<'a>>,
    ) -> bool {
        if !seen.insert(frame.shell())
            || entry.engine.availability()
                != &bevy_ui_router_core::presentation::RouterAvailability::Ready
            || !entry.engine.active().frames().contains(frame.frame())
        {
            return false;
        }
        let Some(host) = entry.host.as_ref() else {
            return false;
        };
        if !self.entities.get(host.host).is_ok_and(|facts| {
            facts.host.is_some_and(|registration| {
                registration.router() == entry.handle
                    && registration.incarnation() == host.registration_incarnation
            })
        }) {
            return false;
        }
        let Some(shell) = frame.claims().find(|claim| claim.entity() == frame.shell()) else {
            return false;
        };
        if shell.lifecycle() != EntityLifecycle::Active
            || !self.claim_is_current(shell, facts)
            || !self.edge_is_current(frame.shell(), frame.top_slot(), facts)
            || !self.claim_is_current(frame.mount_binding().slot_claim(), facts)
        {
            return false;
        }
        if let Some(root) = frame.native_root() {
            let Some(root_claim) = frame.claims().find(|claim| claim.entity() == root) else {
                return false;
            };
            if root_claim.lifecycle() != EntityLifecycle::Active
                || !self.claim_is_current(root_claim, facts)
                || !self.is_native_root(root)
                || !self.edge_is_current(root, frame.shell(), facts)
            {
                return false;
            }
        }
        let mount = frame.mount_binding();
        match mount.slot_claim().owner() {
            RegistryOwner::Committed(EntityOwner::HostTopSlot { .. }) => {
                mount.slot_claim() == &host.top_slot_claim
                    && frame.top_slot() == host.top_slot
                    && self.edge_is_current(host.top_slot, host.host, facts)
            }
            RegistryOwner::Committed(EntityOwner::ChildSlot { .. }) => {
                let Some(active) = entry.active.as_ref() else {
                    return false;
                };
                let Some(slot) = active.child_slots().iter().find(|slot| {
                    slot.slot_claim() == mount.slot_claim()
                        && mount.matches_mounted_child_slot(slot)
                }) else {
                    return false;
                };
                let Some(parent) = active
                    .frames()
                    .iter()
                    .find(|parent| parent.frame() == slot.parent())
                else {
                    return false;
                };
                let source = slot.anchor_binding().source_claim();
                // The anchor's source must belong to the exact published
                // parent, whose own mount chain must still reach this host.
                parent.claims().any(|claim| claim == source)
                    && self.registered_content_is_current(parent, source.entity(), facts)
                    && self.edge_is_current(slot.slot(), slot.anchor(), facts)
                    && (slot.anchor() == source.entity()
                        || self.native_containment_is_current(
                            slot.anchor(),
                            source.entity(),
                            facts,
                        ))
                    && self.frame_scope_is_current_with_parents(entry, parent, seen, facts)
            }
            _ => false,
        }
    }

    fn registered_content_is_current<'a>(
        &'a self,
        frame: &RealizedFrame,
        target: Entity,
        facts: Option<&ReadFacts<'a>>,
    ) -> bool {
        let mut current = target;
        let mut seen = EntityHashSet::default();
        while seen.insert(current) {
            let Some((claim, parent)) = frame
                .cleanup_members()
                .find(|(claim, _)| claim.entity() == current)
            else {
                return false;
            };
            if claim.lifecycle() != EntityLifecycle::Active
                || !self.claim_is_current(claim, facts)
                || !self.edge_is_current(current, parent, facts)
            {
                return false;
            }
            if current == frame.shell() {
                return true;
            }
            current = parent;
        }
        false
    }

    fn native_containment_is_current<'a>(
        &'a self,
        target: Entity,
        root: Entity,
        facts: Option<&ReadFacts<'a>>,
    ) -> bool {
        if !self.is_native_root(root) {
            return false;
        }
        let mut current = target;
        let mut seen = EntityHashSet::default();
        while seen.insert(current) {
            if current == root {
                return true;
            }
            if self.record(current, facts).is_some() || self.is_quarantined(current) {
                return false;
            }
            let Ok(entity) = self.entities.get(current) else {
                return false;
            };
            let Some(parent) = entity.parent.map(ChildOf::parent) else {
                return false;
            };
            if !self.edge_is_current(current, parent, facts) {
                return false;
            }
            current = parent;
        }
        false
    }
}
