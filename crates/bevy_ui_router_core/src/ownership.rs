//! ```compile_fail
//! use bevy_ui_router_core::prelude::{
//!     CleanupScope, RouteFrameId, RouteGeneration, RouteHostId, RouteSlotId,
//!     RouterInstanceId,
//! };
//!
//! fn forge(
//!     router: RouterInstanceId,
//!     host: RouteHostId,
//!     slot: RouteSlotId,
//!     frame: RouteFrameId,
//!     generation: RouteGeneration,
//! ) {
//!     let _ = CleanupScope::router_owned_subtree(router, host, slot, frame, generation);
//! }
//! ```

use crate::ids::{RouteFrameId, RouteGeneration, RouteHostId, RouteSlotId, RouterInstanceId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OwnedRegion {
    RouterOwnedSubtree,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CleanupScope {
    router: RouterInstanceId,
    host: RouteHostId,
    slot: RouteSlotId,
    frame: RouteFrameId,
    generation: RouteGeneration,
    region: OwnedRegion,
}

impl CleanupScope {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) const fn router_owned_subtree(
        router: RouterInstanceId,
        host: RouteHostId,
        slot: RouteSlotId,
        frame: RouteFrameId,
        generation: RouteGeneration,
    ) -> Self {
        Self {
            router,
            host,
            slot,
            frame,
            generation,
            region: OwnedRegion::RouterOwnedSubtree,
        }
    }

    pub const fn router(self) -> RouterInstanceId {
        self.router
    }

    pub const fn host(self) -> RouteHostId {
        self.host
    }

    pub const fn slot(self) -> RouteSlotId {
        self.slot
    }

    pub const fn frame(self) -> RouteFrameId {
        self.frame
    }

    pub const fn generation(self) -> RouteGeneration {
        self.generation
    }

    pub const fn region(self) -> OwnedRegion {
        self.region
    }

    pub const fn excludes_app_owned_host(self) -> bool {
        matches!(self.region, OwnedRegion::RouterOwnedSubtree)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleanup_scope_targets_only_router_owned_subtree() {
        let scope = CleanupScope::router_owned_subtree(
            RouterInstanceId::new(1),
            RouteHostId::new(2),
            RouteSlotId::new(3),
            RouteFrameId::new(4),
            RouteGeneration::initial(),
        );

        assert_eq!(scope.router().raw(), 1);
        assert_eq!(scope.host().raw(), 2);
        assert_eq!(scope.slot().raw(), 3);
        assert_eq!(scope.frame().raw(), 4);
        assert_eq!(scope.generation().raw(), 0);
        assert_eq!(scope.region(), OwnedRegion::RouterOwnedSubtree);
        assert!(scope.excludes_app_owned_host());
    }
}
