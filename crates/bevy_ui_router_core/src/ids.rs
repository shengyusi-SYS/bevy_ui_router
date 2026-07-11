//! Opaque logical identities are issued only by crate-private allocators.
//!
//! ```compile_fail
//! use bevy_ui_router_core::prelude::RouteFrameId;
//!
//! let _ = RouteFrameId(1);
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_core::prelude::RouteFrameId;
//!
//! let _ = RouteFrameId::new(1);
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_core::prelude::RouteGeneration;
//!
//! let _ = RouteGeneration::next;
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_core::prelude::RouteGeneration;
//!
//! let generation = RouteGeneration::initial();
//! let _ = generation.next();
//! ```

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};

macro_rules! id_type {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(u64);

        impl $name {
            #[allow(dead_code)]
            pub(crate) const fn new(raw: u64) -> Self {
                Self(raw)
            }

            pub const fn raw(self) -> u64 {
                self.0
            }
        }
    };
}

id_type!(RouterInstanceId);
id_type!(RouteHostId);
id_type!(RouteSlotId);
id_type!(RouteId);
id_type!(RouteFrameId);

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct NavigationRequestId {
    domain: WorldDomainId,
    router: RouterInstanceId,
    sequence: u64,
}

impl NavigationRequestId {
    pub(crate) const fn router(self) -> RouterInstanceId {
        self.router
    }
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct FrameTaskId(u64);

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct TransactionId(u64);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct WorldDomainId(u64);

#[allow(dead_code)]
#[derive(Debug)]
pub(crate) struct CoreIdAllocator {
    domain: WorldDomainId,
    next_router: u64,
    next_host: u64,
    next_slot: u64,
    next_frame: u64,
    next_task: u64,
    next_transaction: u64,
    next_request_by_router: BTreeMap<RouterInstanceId, u64>,
}

#[allow(dead_code)]
impl CoreIdAllocator {
    pub(crate) fn new() -> Self {
        static NEXT_WORLD_DOMAIN: AtomicU64 = AtomicU64::new(1);

        Self {
            domain: WorldDomainId(NEXT_WORLD_DOMAIN.fetch_add(1, Ordering::Relaxed)),
            next_router: 1,
            next_host: 1,
            next_slot: 1,
            next_frame: 1,
            next_task: 1,
            next_transaction: 1,
            next_request_by_router: BTreeMap::new(),
        }
    }

    pub(crate) fn allocate_router(&mut self) -> RouterInstanceId {
        let id = RouterInstanceId::new(self.next_router);
        self.next_router += 1;
        id
    }

    pub(crate) fn allocate_host(&mut self) -> RouteHostId {
        let id = RouteHostId::new(self.next_host);
        self.next_host += 1;
        id
    }

    pub(crate) fn allocate_slot(&mut self) -> RouteSlotId {
        let id = RouteSlotId::new(self.next_slot);
        self.next_slot += 1;
        id
    }

    pub(crate) fn allocate_frame(&mut self) -> RouteFrameId {
        let id = RouteFrameId::new(self.next_frame);
        self.next_frame += 1;
        id
    }

    pub(crate) fn allocate_frame_task(&mut self) -> FrameTaskId {
        let id = FrameTaskId(self.next_task);
        self.next_task += 1;
        id
    }

    pub(crate) fn allocate_transaction(&mut self) -> TransactionId {
        let id = TransactionId(self.next_transaction);
        self.next_transaction += 1;
        id
    }

    pub(crate) fn allocate_request(&mut self, router: RouterInstanceId) -> NavigationRequestId {
        let sequence = self.next_request_by_router.entry(router).or_insert(1);
        let id = NavigationRequestId {
            domain: self.domain,
            router,
            sequence: *sequence,
        };
        *sequence += 1;
        id
    }

    pub(crate) fn owns_request(
        &self,
        request: NavigationRequestId,
        router: RouterInstanceId,
    ) -> bool {
        request.domain == self.domain && request.router() == router
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RouteGeneration(u64);

impl RouteGeneration {
    #[allow(dead_code)]
    pub(crate) const fn initial() -> Self {
        Self(0)
    }

    #[allow(dead_code)]
    pub(crate) const fn next(self) -> Self {
        Self(self.0 + 1)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_expose_raw_values_without_public_construction() {
        let router = RouterInstanceId::new(7);
        let host = RouteHostId::new(8);
        let slot = RouteSlotId::new(9);
        let route = RouteId::new(10);
        let frame = RouteFrameId::new(11);

        assert_eq!(router.raw(), 7);
        assert_eq!(host.raw(), 8);
        assert_eq!(slot.raw(), 9);
        assert_eq!(route.raw(), 10);
        assert_eq!(frame.raw(), 11);
    }

    #[test]
    fn generation_advances_inside_core() {
        let first = RouteGeneration::initial();
        let second = first.next();

        assert_eq!(first.raw(), 0);
        assert_eq!(second.raw(), 1);
        assert_ne!(first, second);
    }

    #[test]
    fn core_allocator_burns_request_task_and_transaction_identities() {
        let mut allocator = CoreIdAllocator::new();
        let router = allocator.allocate_router();
        let first_request = allocator.allocate_request(router);
        let second_request = allocator.allocate_request(router);

        assert!(allocator.owns_request(first_request, router));
        assert_ne!(first_request, second_request);
        assert_ne!(
            allocator.allocate_frame_task(),
            allocator.allocate_frame_task()
        );
        assert_ne!(
            allocator.allocate_transaction(),
            allocator.allocate_transaction()
        );
    }
}
