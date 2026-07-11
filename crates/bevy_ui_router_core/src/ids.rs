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

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct NavigationRequestId {
    domain: WorldDomainId,
    router: RouterInstanceId,
    sequence: u64,
}

impl NavigationRequestId {
    #[allow(dead_code)]
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct IdExhaustion;

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
    pub(crate) fn new() -> Result<Self, IdExhaustion> {
        static NEXT_WORLD_DOMAIN: AtomicU64 = AtomicU64::new(1);

        Ok(Self {
            domain: allocate_world_domain(&NEXT_WORLD_DOMAIN)?,
            next_router: 1,
            next_host: 1,
            next_slot: 1,
            next_frame: 1,
            next_task: 1,
            next_transaction: 1,
            next_request_by_router: BTreeMap::new(),
        })
    }

    pub(crate) fn allocate_router(&mut self) -> Result<RouterInstanceId, IdExhaustion> {
        checked_take(&mut self.next_router).map(RouterInstanceId::new)
    }

    pub(crate) fn allocate_host(&mut self) -> Result<RouteHostId, IdExhaustion> {
        checked_take(&mut self.next_host).map(RouteHostId::new)
    }

    pub(crate) fn allocate_slot(&mut self) -> Result<RouteSlotId, IdExhaustion> {
        checked_take(&mut self.next_slot).map(RouteSlotId::new)
    }

    pub(crate) fn allocate_host_slot(
        &mut self,
    ) -> Result<(RouteHostId, RouteSlotId), IdExhaustion> {
        let next_host = self.next_host.checked_add(1).ok_or(IdExhaustion)?;
        let next_slot = self.next_slot.checked_add(1).ok_or(IdExhaustion)?;
        let result = (
            RouteHostId::new(self.next_host),
            RouteSlotId::new(self.next_slot),
        );
        self.next_host = next_host;
        self.next_slot = next_slot;
        Ok(result)
    }

    pub(crate) fn allocate_frame(&mut self) -> Result<RouteFrameId, IdExhaustion> {
        checked_take(&mut self.next_frame).map(RouteFrameId::new)
    }

    pub(crate) fn allocate_frame_task(&mut self) -> Result<FrameTaskId, IdExhaustion> {
        checked_take(&mut self.next_task).map(FrameTaskId)
    }

    pub(crate) fn allocate_transaction(&mut self) -> Result<TransactionId, IdExhaustion> {
        checked_take(&mut self.next_transaction).map(TransactionId)
    }

    pub(crate) fn allocate_request(
        &mut self,
        router: RouterInstanceId,
    ) -> Result<NavigationRequestId, IdExhaustion> {
        let current = self
            .next_request_by_router
            .get(&router)
            .copied()
            .unwrap_or(1);
        let next = current.checked_add(1).ok_or(IdExhaustion)?;
        let id = NavigationRequestId {
            domain: self.domain,
            router,
            sequence: current,
        };
        self.next_request_by_router.insert(router, next);
        Ok(id)
    }

    pub(crate) fn owns_request(
        &self,
        request: NavigationRequestId,
        router: RouterInstanceId,
    ) -> bool {
        request.domain == self.domain && request.router() == router
    }

    #[cfg(test)]
    pub(crate) fn snapshot(&self) -> CoreIdAllocatorSnapshot {
        CoreIdAllocatorSnapshot {
            next_host: self.next_host,
            next_slot: self.next_slot,
        }
    }
}

#[allow(dead_code)]
fn allocate_world_domain(counter: &AtomicU64) -> Result<WorldDomainId, IdExhaustion> {
    counter
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1)
        })
        .map(WorldDomainId)
        .map_err(|_| IdExhaustion)
}

#[allow(dead_code)]
fn checked_take(counter: &mut u64) -> Result<u64, IdExhaustion> {
    let current = *counter;
    *counter = current.checked_add(1).ok_or(IdExhaustion)?;
    Ok(current)
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CoreIdAllocatorSnapshot {
    next_host: u64,
    next_slot: u64,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RouteGeneration(u64);

impl RouteGeneration {
    #[allow(dead_code)]
    pub(crate) const fn initial() -> Self {
        Self(0)
    }

    #[allow(dead_code)]
    pub(crate) const fn checked_next(self) -> Result<Self, IdExhaustion> {
        match self.0.checked_add(1) {
            Some(next) => Ok(Self(next)),
            None => Err(IdExhaustion),
        }
    }

    #[allow(dead_code)]
    pub(crate) const fn next(self) -> Self {
        match self.checked_next() {
            Ok(next) => next,
            Err(_) => panic!("route generation exhausted"),
        }
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
        let mut allocator = CoreIdAllocator::new().unwrap();
        let router = allocator.allocate_router().unwrap();
        let first_request = allocator.allocate_request(router).unwrap();
        let second_request = allocator.allocate_request(router).unwrap();

        assert!(allocator.owns_request(first_request, router));
        assert_ne!(first_request, second_request);
        assert_ne!(
            allocator.allocate_frame_task().unwrap(),
            allocator.allocate_frame_task().unwrap()
        );
        assert_ne!(
            allocator.allocate_transaction().unwrap(),
            allocator.allocate_transaction().unwrap()
        );
    }

    #[test]
    fn every_allocator_counter_reports_exhaustion_without_wrapping() {
        let mut allocator = CoreIdAllocator::new().unwrap();

        allocator.next_router = u64::MAX;
        assert_eq!(allocator.allocate_router(), Err(IdExhaustion));
        assert_eq!(allocator.next_router, u64::MAX);
        allocator.next_router = 1;

        allocator.next_host = u64::MAX;
        assert_eq!(allocator.allocate_host(), Err(IdExhaustion));
        assert_eq!(allocator.next_host, u64::MAX);
        allocator.next_host = 1;

        allocator.next_slot = u64::MAX;
        assert_eq!(allocator.allocate_slot(), Err(IdExhaustion));
        assert_eq!(allocator.next_slot, u64::MAX);
        allocator.next_slot = 1;

        allocator.next_frame = u64::MAX;
        assert_eq!(allocator.allocate_frame(), Err(IdExhaustion));
        assert_eq!(allocator.next_frame, u64::MAX);
        allocator.next_frame = 1;

        allocator.next_task = u64::MAX;
        assert_eq!(allocator.allocate_frame_task(), Err(IdExhaustion));
        assert_eq!(allocator.next_task, u64::MAX);
        allocator.next_task = 1;

        allocator.next_transaction = u64::MAX;
        assert_eq!(allocator.allocate_transaction(), Err(IdExhaustion));
        assert_eq!(allocator.next_transaction, u64::MAX);

        let router = RouterInstanceId::new(7);
        allocator.next_request_by_router.insert(router, u64::MAX);
        assert_eq!(allocator.allocate_request(router), Err(IdExhaustion));
        assert_eq!(allocator.next_request_by_router[&router], u64::MAX);
    }

    #[test]
    fn world_domain_and_generation_exhaustion_are_checked() {
        let domain_counter = AtomicU64::new(u64::MAX);
        assert_eq!(allocate_world_domain(&domain_counter), Err(IdExhaustion));
        assert_eq!(domain_counter.load(Ordering::Relaxed), u64::MAX);

        let exhausted = RouteGeneration(u64::MAX);
        assert_eq!(exhausted.checked_next(), Err(IdExhaustion));
        assert_eq!(exhausted.raw(), u64::MAX);
    }

    #[test]
    fn host_slot_pair_exhaustion_is_atomic() {
        let mut allocator = CoreIdAllocator::new().unwrap();
        allocator.next_slot = u64::MAX;
        let before = allocator.snapshot();

        assert_eq!(allocator.allocate_host_slot(), Err(IdExhaustion));
        assert_eq!(allocator.snapshot(), before);
    }
}
