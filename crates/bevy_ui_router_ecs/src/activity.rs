use std::collections::HashSet;

use bevy_ecs::{
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
        RouteEntityRegistry,
    },
};

#[derive(QueryData)]
struct ActivityEntity {
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

/// Queries whether a target belongs to currently published active route content.
///
/// Native content added after publication is observed through its current parent
/// relationships. Retained, pending, leaving, quarantined and unrelated targets
/// return `false`. Activity is independent of authored visibility and input.
///
/// This parameter only reads Router metadata and hierarchy components, so it can
/// be used beside an application's `Query<(Entity, &mut T)>` for its own state.
#[derive(SystemParam)]
pub struct RouteActivity<'w, 's> {
    navigation: Res<'w, RouterNavigation>,
    registry: Res<'w, RouteEntityRegistry>,
    quarantine: Res<'w, QuarantineLeaseTable>,
    entities: Query<'w, 's, ActivityEntity>,
    #[cfg(feature = "bsn")]
    native_roots: Query<
        'w,
        's,
        (),
        bevy_ecs::query::With<crate::presentation::native_scene::NativeContentRoot>,
    >,
}

impl RouteActivity<'_, '_> {
    /// Returns the target's activity at the time this method is called.
    ///
    /// Systems that need this update's completed publication can run after
    /// [`crate::RouterScheduleSet::PublishSnapshots`]. This query grants no
    /// lifecycle or cleanup authority.
    pub fn is_active(&self, target: Entity) -> bool {
        let mut current = target;
        let mut seen = HashSet::new();
        while seen.insert(current) {
            if self.is_quarantined(current) || self.entities.get(current).is_err() {
                return false;
            }
            if let Some(record) = self.registry.record(current) {
                // The first registered boundary owns this content. A retained
                // leaf must never inherit activity from its active parent.
                for entry in &self.navigation.entries {
                    let Some(active) = entry.active.as_ref() else {
                        continue;
                    };
                    for frame in active.frames() {
                        let Some(claim) = frame.claims().find(|claim| claim.entity() == current)
                        else {
                            continue;
                        };
                        if claim.lifecycle() != EntityLifecycle::Active
                            || !record.matches_claim(claim)
                            || !self.frame_scope_is_current(entry, frame)
                        {
                            return false;
                        }
                        if current == target {
                            return self.registered_content_is_current(frame, target);
                        }
                        return frame.native_root() == Some(current)
                            && self.is_native_root(current)
                            && self.claim_is_current(claim);
                    }
                }
                return false;
            }
            let Ok(facts) = self.entities.get(current) else {
                return false;
            };
            let Some(parent) = facts.parent.map(ChildOf::parent) else {
                return false;
            };
            if !self.edge_is_current(current, parent) {
                return false;
            }
            current = parent;
        }
        false
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

    fn edge_is_current(&self, child: Entity, parent: Entity) -> bool {
        self.entities
            .get(child)
            .is_ok_and(|facts| facts.parent.map(ChildOf::parent) == Some(parent))
            && self.entities.get(parent).is_ok_and(|facts| {
                facts
                    .children
                    .is_some_and(|children| children.contains(&child))
            })
    }

    fn claim_is_current(&self, claim: &ExactRegistryClaim) -> bool {
        if self.is_quarantined(claim.entity())
            || !self
                .registry
                .record(claim.entity())
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

    fn frame_scope_is_current(&self, entry: &RouterEntry, frame: &RealizedFrame) -> bool {
        self.frame_scope_is_current_with_parents(entry, frame, &mut HashSet::new())
    }

    fn frame_scope_is_current_with_parents(
        &self,
        entry: &RouterEntry,
        frame: &RealizedFrame,
        seen: &mut HashSet<Entity>,
    ) -> bool {
        if !seen.insert(frame.shell()) {
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
            || !self.claim_is_current(shell)
            || !self.edge_is_current(frame.shell(), frame.top_slot())
            || !self.claim_is_current(frame.mount_binding().slot_claim())
        {
            return false;
        }
        if let Some(root) = frame.native_root() {
            let Some(root_claim) = frame.claims().find(|claim| claim.entity() == root) else {
                return false;
            };
            if root_claim.lifecycle() != EntityLifecycle::Active
                || !self.claim_is_current(root_claim)
                || !self.is_native_root(root)
                || !self.edge_is_current(root, frame.shell())
            {
                return false;
            }
        }
        let mount = frame.mount_binding();
        match mount.slot_claim().owner() {
            RegistryOwner::Committed(EntityOwner::HostTopSlot { .. }) => {
                mount.slot_claim() == &host.top_slot_claim
                    && frame.top_slot() == host.top_slot
                    && self.edge_is_current(host.top_slot, host.host)
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
                    && self.registered_content_is_current(parent, source.entity())
                    && self.edge_is_current(slot.slot(), slot.anchor())
                    && (slot.anchor() == source.entity()
                        || self.native_containment_is_current(slot.anchor(), source.entity()))
                    && self.frame_scope_is_current_with_parents(entry, parent, seen)
            }
            _ => false,
        }
    }

    fn registered_content_is_current(&self, frame: &RealizedFrame, target: Entity) -> bool {
        let mut current = target;
        let mut seen = HashSet::new();
        while seen.insert(current) {
            let Some((claim, parent)) = frame
                .cleanup_members()
                .find(|(claim, _)| claim.entity() == current)
            else {
                return false;
            };
            if claim.lifecycle() != EntityLifecycle::Active
                || !self.claim_is_current(claim)
                || !self.edge_is_current(current, parent)
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

    fn native_containment_is_current(&self, target: Entity, root: Entity) -> bool {
        if !self.is_native_root(root) {
            return false;
        }
        let mut current = target;
        let mut seen = HashSet::new();
        while seen.insert(current) {
            if current == root {
                return true;
            }
            if self.registry.record(current).is_some() || self.is_quarantined(current) {
                return false;
            }
            let Ok(facts) = self.entities.get(current) else {
                return false;
            };
            let Some(parent) = facts.parent.map(ChildOf::parent) else {
                return false;
            };
            if !self.edge_is_current(current, parent) {
                return false;
            }
            current = parent;
        }
        false
    }
}
