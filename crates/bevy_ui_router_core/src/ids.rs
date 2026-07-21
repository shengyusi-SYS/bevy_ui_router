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

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RouterInstanceId(u64);

impl RouterInstanceId {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) const fn new(raw: u64) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RouteHostId(u64);

impl RouteHostId {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) const fn new(raw: u64) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RouteSlotId(u64);

impl RouteSlotId {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) const fn new(raw: u64) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RouteId(u64);

impl RouteId {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) const fn new(raw: u64) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RouteFrameId(u64);

impl RouteFrameId {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) const fn new(raw: u64) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct NavigationRequestId {
    router: RouterIdentity,
    sequence: u64,
}

impl NavigationRequestId {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) const fn router(self) -> RouterInstanceId {
        self.router.router()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) const fn router_identity(self) -> RouterIdentity {
        self.router
    }

    pub(crate) const fn sequence(self) -> u64 {
        self.sequence
    }
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct FrameTaskId(u64);

impl FrameTaskId {
    pub(crate) const fn raw(self) -> u64 {
        self.0
    }
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct TransactionId(u64);

impl TransactionId {
    pub(crate) const fn raw(self) -> u64 {
        self.0
    }
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct CleanupBatchId(u64);

impl CleanupBatchId {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) const fn new(raw: u64) -> Self {
        Self(raw)
    }

    pub(crate) const fn raw(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct WorldDomainId(u64);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RouterIdentity {
    domain: WorldDomainId,
    router: RouterInstanceId,
}

impl RouterIdentity {
    pub(crate) const fn router(self) -> RouterInstanceId {
        self.router
    }

    pub(crate) const fn provenance(self) -> (u64, u64) {
        (self.domain.0, self.router.raw())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ReservedFrameAttempt {
    frame_task: FrameTaskId,
    frame: RouteFrameId,
}

impl ReservedFrameAttempt {
    pub(crate) const fn new(frame_task: FrameTaskId, frame: RouteFrameId) -> Self {
        Self { frame_task, frame }
    }

    pub(crate) const fn frame_task(self) -> FrameTaskId {
        self.frame_task
    }

    pub(crate) const fn frame(self) -> RouteFrameId {
        self.frame
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReservedBranchAttempt {
    request: NavigationRequestId,
    transaction: TransactionId,
    cleanup_batch: CleanupBatchId,
    frames: Vec<ReservedFrameAttempt>,
}

impl ReservedBranchAttempt {
    pub(crate) const fn request(&self) -> NavigationRequestId {
        self.request
    }

    pub(crate) const fn transaction(&self) -> TransactionId {
        self.transaction
    }

    pub(crate) const fn cleanup_batch(&self) -> CleanupBatchId {
        self.cleanup_batch
    }

    pub(crate) fn frames(&self) -> &[ReservedFrameAttempt] {
        &self.frames
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ReservedAttemptIds {
    transaction: TransactionId,
    frame_task: FrameTaskId,
    candidate_frame: RouteFrameId,
    cleanup_batch: CleanupBatchId,
}

impl ReservedAttemptIds {
    pub(crate) const fn transaction(self) -> TransactionId {
        self.transaction
    }

    pub(crate) const fn frame_task(self) -> FrameTaskId {
        self.frame_task
    }

    pub(crate) const fn candidate_frame(self) -> RouteFrameId {
        self.candidate_frame
    }

    pub(crate) const fn cleanup_batch(self) -> CleanupBatchId {
        self.cleanup_batch
    }
}

#[cfg(any(test, feature = "test-support"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReservationCounter {
    Request,
    Transaction,
    FrameTask,
    CandidateFrame,
    CleanupBatch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct IdExhaustion;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RequestAllocationError {
    ForeignRouter,
    Exhausted,
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug)]
pub(crate) struct CoreIdAllocator {
    domain: WorldDomainId,
    next_router: u64,
    next_host: u64,
    next_slot: u64,
    next_frame: u64,
    next_task: u64,
    next_transaction: u64,
    next_cleanup_batch: u64,
    next_mount_sequence: u64,
    next_request_by_router: BTreeMap<RouterIdentity, u64>,
}

impl CoreIdAllocator {
    #[cfg_attr(not(test), allow(dead_code))]
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
            next_cleanup_batch: 1,
            next_mount_sequence: 1,
            next_request_by_router: BTreeMap::new(),
        })
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn allocate_router_identity(&mut self) -> Result<RouterIdentity, IdExhaustion> {
        Ok(RouterIdentity {
            domain: self.domain,
            router: RouterInstanceId::new(checked_take(&mut self.next_router)?),
        })
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn owns_router_identity(&self, router: RouterIdentity) -> bool {
        router.domain == self.domain
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn allocate_host(&mut self) -> Result<RouteHostId, IdExhaustion> {
        checked_take(&mut self.next_host).map(RouteHostId::new)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn allocate_slot(&mut self) -> Result<RouteSlotId, IdExhaustion> {
        checked_take(&mut self.next_slot).map(RouteSlotId::new)
    }

    #[cfg_attr(not(test), allow(dead_code))]
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

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn allocate_frame(&mut self) -> Result<RouteFrameId, IdExhaustion> {
        checked_take(&mut self.next_frame).map(RouteFrameId::new)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn allocate_frame_task(&mut self) -> Result<FrameTaskId, IdExhaustion> {
        checked_take(&mut self.next_task).map(FrameTaskId)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn allocate_transaction(&mut self) -> Result<TransactionId, IdExhaustion> {
        checked_take(&mut self.next_transaction).map(TransactionId)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn allocate_mount_sequence(&mut self) -> Result<u64, IdExhaustion> {
        checked_take(&mut self.next_mount_sequence)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn allocate_request(
        &mut self,
        router: RouterIdentity,
    ) -> Result<NavigationRequestId, RequestAllocationError> {
        if router.domain != self.domain {
            return Err(RequestAllocationError::ForeignRouter);
        }
        let current = self
            .next_request_by_router
            .get(&router)
            .copied()
            .unwrap_or(1);
        let next = current
            .checked_add(1)
            .ok_or(RequestAllocationError::Exhausted)?;
        let id = NavigationRequestId {
            router,
            sequence: current,
        };
        self.next_request_by_router.insert(router, next);
        Ok(id)
    }

    pub(crate) fn reserve_navigation_branch(
        &mut self,
        router: RouterIdentity,
        branch_len: usize,
    ) -> Result<ReservedBranchAttempt, RequestAllocationError> {
        if router.domain != self.domain {
            return Err(RequestAllocationError::ForeignRouter);
        }
        let branch_len =
            u64::try_from(branch_len).map_err(|_| RequestAllocationError::Exhausted)?;
        if branch_len == 0 {
            return Err(RequestAllocationError::Exhausted);
        }
        let request = self
            .next_request_by_router
            .get(&router)
            .copied()
            .unwrap_or(1);
        let next_request = request
            .checked_add(1)
            .ok_or(RequestAllocationError::Exhausted)?;
        let next_transaction = self
            .next_transaction
            .checked_add(1)
            .ok_or(RequestAllocationError::Exhausted)?;
        let next_task = self
            .next_task
            .checked_add(branch_len)
            .ok_or(RequestAllocationError::Exhausted)?;
        let next_frame = self
            .next_frame
            .checked_add(branch_len)
            .ok_or(RequestAllocationError::Exhausted)?;
        let next_cleanup_batch = self
            .next_cleanup_batch
            .checked_add(1)
            .ok_or(RequestAllocationError::Exhausted)?;

        let frames = (0..branch_len)
            .map(|offset| ReservedFrameAttempt {
                frame_task: FrameTaskId(self.next_task + offset),
                frame: RouteFrameId::new(self.next_frame + offset),
            })
            .collect();
        let reserved = ReservedBranchAttempt {
            request: NavigationRequestId {
                router,
                sequence: request,
            },
            transaction: TransactionId(self.next_transaction),
            cleanup_batch: CleanupBatchId(self.next_cleanup_batch),
            frames,
        };
        self.next_request_by_router.insert(router, next_request);
        self.next_transaction = next_transaction;
        self.next_task = next_task;
        self.next_frame = next_frame;
        self.next_cleanup_batch = next_cleanup_batch;
        Ok(reserved)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn reserve_navigation(
        &mut self,
        router: RouterIdentity,
    ) -> Result<ReservedBranchAttempt, RequestAllocationError> {
        self.reserve_navigation_branch(router, 1)
    }

    pub(crate) fn reserve_attempt(&mut self) -> Result<ReservedAttemptIds, IdExhaustion> {
        let next_transaction = self.next_transaction.checked_add(1).ok_or(IdExhaustion)?;
        let next_task = self.next_task.checked_add(1).ok_or(IdExhaustion)?;
        let next_frame = self.next_frame.checked_add(1).ok_or(IdExhaustion)?;
        let next_cleanup_batch = self.next_cleanup_batch.checked_add(1).ok_or(IdExhaustion)?;

        let reserved = ReservedAttemptIds {
            transaction: TransactionId(self.next_transaction),
            frame_task: FrameTaskId(self.next_task),
            candidate_frame: RouteFrameId::new(self.next_frame),
            cleanup_batch: CleanupBatchId(self.next_cleanup_batch),
        };
        self.next_transaction = next_transaction;
        self.next_task = next_task;
        self.next_frame = next_frame;
        self.next_cleanup_batch = next_cleanup_batch;
        Ok(reserved)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn owns_request(
        &self,
        request: NavigationRequestId,
        router: RouterIdentity,
    ) -> bool {
        router.domain == self.domain && request.router_identity() == router
    }

    #[cfg(test)]
    pub(crate) fn snapshot(&self) -> CoreIdAllocatorSnapshot {
        CoreIdAllocatorSnapshot {
            next_router: self.next_router,
            next_host: self.next_host,
            next_slot: self.next_slot,
            next_frame: self.next_frame,
            next_task: self.next_task,
            next_transaction: self.next_transaction,
            next_cleanup_batch: self.next_cleanup_batch,
            next_mount_sequence: self.next_mount_sequence,
            next_request_by_router: self.next_request_by_router.clone(),
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn testing_exhaust_reservation_counter(
        &mut self,
        router: RouterIdentity,
        counter: ReservationCounter,
    ) {
        match counter {
            ReservationCounter::Request => {
                self.next_request_by_router.insert(router, u64::MAX);
            }
            ReservationCounter::Transaction => self.next_transaction = u64::MAX,
            ReservationCounter::FrameTask => self.next_task = u64::MAX,
            ReservationCounter::CandidateFrame => self.next_frame = u64::MAX,
            ReservationCounter::CleanupBatch => self.next_cleanup_batch = u64::MAX,
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn testing_exhaust_slot_counter(&mut self) {
        self.next_slot = u64::MAX;
    }

    #[cfg(test)]
    pub(crate) fn testing_exhaust_mount_sequence_counter(&mut self) {
        self.next_mount_sequence = u64::MAX;
    }
}

#[cfg_attr(not(test), allow(dead_code))]
fn allocate_world_domain(counter: &AtomicU64) -> Result<WorldDomainId, IdExhaustion> {
    counter
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1)
        })
        .map(WorldDomainId)
        .map_err(|_| IdExhaustion)
}

#[cfg_attr(not(test), allow(dead_code))]
fn checked_take(counter: &mut u64) -> Result<u64, IdExhaustion> {
    let current = *counter;
    *counter = current.checked_add(1).ok_or(IdExhaustion)?;
    Ok(current)
}

#[cfg(test)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CoreIdAllocatorSnapshot {
    next_router: u64,
    next_host: u64,
    next_slot: u64,
    next_frame: u64,
    next_task: u64,
    next_transaction: u64,
    next_cleanup_batch: u64,
    next_mount_sequence: u64,
    next_request_by_router: BTreeMap<RouterIdentity, u64>,
}

#[cfg(test)]
impl CoreIdAllocatorSnapshot {
    pub(crate) const fn next_slot(&self) -> u64 {
        self.next_slot
    }

    pub(crate) const fn next_mount_sequence(&self) -> u64 {
        self.next_mount_sequence
    }

    pub(crate) const fn next_frame(&self) -> u64 {
        self.next_frame
    }

    pub(crate) const fn next_frame_task(&self) -> u64 {
        self.next_task
    }

    pub(crate) const fn next_transaction(&self) -> u64 {
        self.next_transaction
    }

    pub(crate) const fn next_cleanup_batch(&self) -> u64 {
        self.next_cleanup_batch
    }

    pub(crate) fn next_request(&self, router: RouterIdentity) -> Option<u64> {
        self.next_request_by_router.get(&router).copied()
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RouteGeneration(u64);

impl RouteGeneration {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) const fn initial() -> Self {
        Self(0)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) const fn checked_next(self) -> Result<Self, IdExhaustion> {
        match self.0.checked_add(1) {
            Some(next) => Ok(Self(next)),
            None => Err(IdExhaustion),
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) const fn next(self) -> Self {
        match self.checked_next() {
            Ok(next) => next,
            Err(_) => panic!("route generation exhausted"),
        }
    }

    pub const fn raw(self) -> u64 {
        self.0
    }

    #[cfg(test)]
    pub(crate) const fn testing_max() -> Self {
        Self(u64::MAX)
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
    fn exact_router_identity_has_internal_value_traits() {
        fn assert_traits<T: Copy + Eq + std::hash::Hash>() {}

        assert_traits::<RouterIdentity>();
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
        let router = allocator.allocate_router_identity().unwrap();
        let first_request = allocator.allocate_request(router).unwrap();
        let second_request = allocator.allocate_request(router).unwrap();

        assert_eq!(first_request.router(), router.router());
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
    fn request_ownership_rejects_foreign_world_with_same_raw_router() {
        let mut first = CoreIdAllocator::new().unwrap();
        let first_router = first.allocate_router_identity().unwrap();
        let request = first.allocate_request(first_router).unwrap();

        let mut second = CoreIdAllocator::new().unwrap();
        let foreign_router = second.allocate_router_identity().unwrap();

        assert_eq!(first_router.router(), foreign_router.router());
        assert!(!first.owns_request(request, foreign_router));
        let before = first.next_request_by_router.clone();
        assert_eq!(
            first.allocate_request(foreign_router),
            Err(RequestAllocationError::ForeignRouter)
        );
        assert_eq!(first.next_request_by_router, before);
    }

    #[test]
    fn request_ownership_rejects_a_different_router_in_the_same_world() {
        let mut allocator = CoreIdAllocator::new().unwrap();
        let router = allocator.allocate_router_identity().unwrap();
        let other_router = allocator.allocate_router_identity().unwrap();
        let request = allocator.allocate_request(router).unwrap();

        assert!(!allocator.owns_request(request, other_router));
        assert_ne!(request, allocator.allocate_request(other_router).unwrap());
    }

    #[test]
    fn every_allocator_counter_reports_exhaustion_without_wrapping() {
        let mut allocator = CoreIdAllocator::new().unwrap();

        allocator.next_router = u64::MAX;
        assert_eq!(allocator.allocate_router_identity(), Err(IdExhaustion));
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

        let router = RouterIdentity {
            domain: allocator.domain,
            router: RouterInstanceId::new(7),
        };
        allocator.next_request_by_router.insert(router, u64::MAX);
        assert_eq!(
            allocator.allocate_request(router),
            Err(RequestAllocationError::Exhausted)
        );
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

    #[test]
    fn navigation_reservation_is_atomic_for_every_reserved_counter() {
        for counter in [
            ReservationCounter::Request,
            ReservationCounter::Transaction,
            ReservationCounter::FrameTask,
            ReservationCounter::CandidateFrame,
            ReservationCounter::CleanupBatch,
        ] {
            let mut allocator = CoreIdAllocator::new().unwrap();
            let router = allocator.allocate_router_identity().unwrap();
            allocator.testing_exhaust_reservation_counter(router, counter);
            let before = allocator.snapshot();

            assert!(allocator.reserve_navigation(router).is_err());
            assert_eq!(allocator.snapshot(), before, "counter: {counter:?}");
        }
    }

    #[test]
    fn branch_reservation_is_atomic_for_every_reserved_counter() {
        for counter in [
            ReservationCounter::Request,
            ReservationCounter::Transaction,
            ReservationCounter::FrameTask,
            ReservationCounter::CandidateFrame,
            ReservationCounter::CleanupBatch,
        ] {
            let mut allocator = CoreIdAllocator::new().unwrap();
            let router = allocator.allocate_router_identity().unwrap();
            allocator.testing_exhaust_reservation_counter(router, counter);
            let before = allocator.snapshot();

            assert!(allocator.reserve_navigation_branch(router, 2).is_err());
            assert_eq!(allocator.snapshot(), before, "counter: {counter:?}");
        }
    }

    #[test]
    fn branch_reservation_length_exhaustion_is_complete_zero_change() {
        for counter in [
            ReservationCounter::FrameTask,
            ReservationCounter::CandidateFrame,
        ] {
            let mut allocator = CoreIdAllocator::new().unwrap();
            let router = allocator.allocate_router_identity().unwrap();
            match counter {
                ReservationCounter::FrameTask => allocator.next_task = u64::MAX - 1,
                ReservationCounter::CandidateFrame => allocator.next_frame = u64::MAX - 1,
                _ => unreachable!(),
            }
            let before = allocator.snapshot();

            assert_eq!(
                allocator.reserve_navigation_branch(router, 2),
                Err(RequestAllocationError::Exhausted)
            );
            assert_eq!(allocator.snapshot(), before, "counter: {counter:?}");
        }
    }

    #[test]
    fn branch_reservation_returns_one_attempt_per_frame_and_burns_successes() {
        let mut allocator = CoreIdAllocator::new().unwrap();
        let router = allocator.allocate_router_identity().unwrap();

        let first = allocator.reserve_navigation_branch(router, 2).unwrap();
        let second = allocator.reserve_navigation_branch(router, 2).unwrap();

        assert_eq!(first.frames().len(), 2);
        assert_eq!(
            first.frames()[0].frame_task().raw() + 1,
            first.frames()[1].frame_task().raw()
        );
        assert_eq!(
            first.frames()[0].frame().raw() + 1,
            first.frames()[1].frame().raw()
        );
        assert!(second.request().sequence() > first.request().sequence());
        assert!(second.transaction().raw() > first.transaction().raw());
        assert!(second.cleanup_batch().raw() > first.cleanup_batch().raw());
        assert!(second.frames()[0].frame_task().raw() > first.frames()[1].frame_task().raw());
        assert!(second.frames()[0].frame().raw() > first.frames()[1].frame().raw());
    }
}
