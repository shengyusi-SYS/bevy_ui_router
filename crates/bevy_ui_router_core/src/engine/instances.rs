//! Original instance identity and exact external parent dependency.
//!
//! These are projections, not a second identity issuer or lifecycle owner.
use std::cmp::Ordering;

use crate::{
    admission::HostVersion,
    frame::{FrameOrigin, RouteFrameVersion},
    ids::RouteSlotId,
    slot::RouteSlotKey,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct InstanceFacts {
    pub(super) frame: RouteFrameVersion,
    pub(super) host: HostVersion,
    pub(super) parent: RouteFrameVersion,
    pub(super) outlet: RouteSlotKey,
    pub(super) slot: RouteSlotId,
}

impl InstanceFacts {
    /// Only owner-set comparison uses this order; inventory keeps departure order.
    /// Different dependencies on one identity are contradictory projections,
    /// not distinct instances. Full projection equality validates the dependency.
    pub(super) fn compare_identity(&self, other: &Self) -> Ordering {
        fn origin_key(origin: &FrameOrigin) -> (u8, u64) {
            match origin {
                FrameOrigin::MatchedRoute(route) => (0, route.raw()),
                FrameOrigin::RouteError(route) => (1, route.raw()),
                FrameOrigin::RouterFallback => (2, 0),
            }
        }
        let key = |facts: &Self| {
            (
                (
                    facts.host.router_identity(),
                    facts.host.host(),
                    facts.host.epoch(),
                ),
                (
                    facts.frame.router_identity(),
                    facts.frame.host(),
                    facts.frame.frame(),
                    facts.frame.generation(),
                    facts.frame.slot(),
                    origin_key(facts.frame.origin()),
                ),
            )
        };
        key(self).cmp(&key(other))
    }
}

/// An instance-owned slot. Its inactive inventory fact never claims an active child.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetainedOwnSlotProjection {
    pub(super) parent: RouteFrameVersion,
    pub(super) outlet: RouteSlotKey,
    pub(super) slot: RouteSlotId,
}
impl RetainedOwnSlotProjection {
    pub const fn parent(&self) -> &RouteFrameVersion {
        &self.parent
    }
    pub const fn outlet(&self) -> &RouteSlotKey {
        &self.outlet
    }
    pub const fn slot(&self) -> RouteSlotId {
        self.slot
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TargetInstanceSourceKind {
    Current,
    Retained,
    Fresh,
}

pub(super) fn dependency_closure(
    root: &RouteFrameVersion,
    inventory: &[&super::retained::RetainedLeafProjection],
) -> Vec<RouteFrameVersion> {
    let mut closure = vec![root.clone()];
    loop {
        let before = closure.len();
        for instance in inventory {
            if closure.contains(instance.parent()) && !closure.contains(instance.frame()) {
                closure.push(instance.frame().clone());
            }
        }
        if closure.len() == before {
            return closure;
        }
    }
}
fn depth(
    instance: &super::retained::RetainedLeafProjection,
    inventory: &[&super::retained::RetainedLeafProjection],
) -> usize {
    let mut parent = instance.parent();
    let mut visited = vec![instance.frame()];
    while let Some(found) = inventory.iter().find(|instance| instance.frame() == parent) {
        if visited.contains(&found.frame()) {
            break;
        }
        visited.push(found.frame());
        parent = found.parent();
    }
    visited.len()
}
pub(super) fn sort_child_first(instances: &mut Vec<&super::retained::RetainedLeafProjection>) {
    let inventory = instances.clone();
    instances.sort_by(|left, right| {
        depth(right, &inventory)
            .cmp(&depth(left, &inventory))
            .then_with(|| left.compare_instance(right))
    });
}
pub(super) fn sort_roots_child_first(
    roots: &mut Vec<RouteFrameVersion>,
    dependencies: &[(RouteFrameVersion, RouteFrameVersion)],
) {
    let original = roots.clone();
    let depth = |frame: &RouteFrameVersion| {
        let mut visited = vec![frame];
        let mut current = frame;
        while let Some((_, parent)) = dependencies.iter().find(|(child, _)| child == current) {
            if visited.contains(&parent) {
                break;
            }
            visited.push(parent);
            current = parent;
        }
        visited.len()
    };
    roots.sort_by(|left, right| {
        depth(right).cmp(&depth(left)).then_with(|| {
            original
                .iter()
                .position(|frame| frame == left)
                .cmp(&original.iter().position(|frame| frame == right))
        })
    });
}
