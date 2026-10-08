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
