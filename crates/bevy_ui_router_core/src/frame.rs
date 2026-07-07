use crate::fingerprint::{RouteBranchFingerprint, RouteSegmentReuseKey};
use crate::ids::{
    RouteFrameId, RouteGeneration, RouteHostId, RouteId, RouteSlotId, RouterInstanceId,
};
use crate::ownership::CleanupScope;
use crate::slot::RouteSlotKey;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RouteFrameScope {
    router: RouterInstanceId,
    host: RouteHostId,
    slot: RouteSlotId,
}

impl RouteFrameScope {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) const fn new(
        router: RouterInstanceId,
        host: RouteHostId,
        slot: RouteSlotId,
    ) -> Self {
        Self { router, host, slot }
    }

    pub const fn router(&self) -> RouterInstanceId {
        self.router
    }

    pub const fn host(&self) -> RouteHostId {
        self.host
    }

    pub const fn slot(&self) -> RouteSlotId {
        self.slot
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RouteFrameStatus {
    Matching,
    Guarding,
    Redirecting,
    Building,
    ResolvingView,
    ApplyingView,
    Entering,
    Active,
    Pausing,
    Leaving,
    Cached,
    Error,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RouteFrameIdentity {
    router: RouterInstanceId,
    host: RouteHostId,
    slot: RouteSlotId,
    route: RouteId,
    frame: RouteFrameId,
    generation: RouteGeneration,
}

impl RouteFrameIdentity {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) const fn new(
        router: RouterInstanceId,
        host: RouteHostId,
        slot: RouteSlotId,
        route: RouteId,
        frame: RouteFrameId,
        generation: RouteGeneration,
    ) -> Self {
        Self {
            router,
            host,
            slot,
            route,
            frame,
            generation,
        }
    }

    pub const fn router(&self) -> RouterInstanceId {
        self.router
    }

    pub const fn host(&self) -> RouteHostId {
        self.host
    }

    pub const fn slot(&self) -> RouteSlotId {
        self.slot
    }

    pub const fn route(&self) -> RouteId {
        self.route
    }

    pub const fn frame(&self) -> RouteFrameId {
        self.frame
    }

    pub const fn generation(&self) -> RouteGeneration {
        self.generation
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouteFrameRecord {
    identity: RouteFrameIdentity,
    branch_index: usize,
    slot_key: RouteSlotKey,
    segment_reuse_key: RouteSegmentReuseKey,
    status: RouteFrameStatus,
}

impl RouteFrameRecord {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn new(
        identity: RouteFrameIdentity,
        branch_index: usize,
        slot_key: RouteSlotKey,
        segment_reuse_key: RouteSegmentReuseKey,
        status: RouteFrameStatus,
    ) -> Self {
        Self {
            identity,
            branch_index,
            slot_key,
            segment_reuse_key,
            status,
        }
    }

    pub const fn identity(&self) -> &RouteFrameIdentity {
        &self.identity
    }

    pub const fn branch_index(&self) -> usize {
        self.branch_index
    }

    pub const fn slot_key(&self) -> &RouteSlotKey {
        &self.slot_key
    }

    pub const fn segment_reuse_key(&self) -> &RouteSegmentReuseKey {
        &self.segment_reuse_key
    }

    pub const fn status(&self) -> RouteFrameStatus {
        self.status
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouteFrameBranch {
    frames: Vec<RouteFrameRecord>,
    branch_fingerprint: RouteBranchFingerprint,
}

impl RouteFrameBranch {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn new(
        frames: Vec<RouteFrameRecord>,
        branch_fingerprint: RouteBranchFingerprint,
    ) -> Self {
        Self {
            frames,
            branch_fingerprint,
        }
    }

    pub fn frames(&self) -> &[RouteFrameRecord] {
        &self.frames
    }

    pub const fn branch_fingerprint(&self) -> &RouteBranchFingerprint {
        &self.branch_fingerprint
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrameCleanupPlan {
    identity: RouteFrameIdentity,
    scope: CleanupScope,
}

impl FrameCleanupPlan {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn router_owned_partial_output(identity: RouteFrameIdentity) -> Self {
        Self {
            scope: CleanupScope::router_owned_subtree(
                identity.router(),
                identity.host(),
                identity.slot(),
                identity.frame(),
                identity.generation(),
            ),
            identity,
        }
    }

    pub const fn identity(&self) -> &RouteFrameIdentity {
        &self.identity
    }

    pub const fn scope(&self) -> CleanupScope {
        self.scope
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::branch::RouteParams;
    use crate::fingerprint::{RouteParamsFingerprint, RouteSearchFingerprint};

    fn segment(route: RouteId) -> RouteSegmentReuseKey {
        let params = RouteParams::new();
        let params_fingerprint = RouteParamsFingerprint::from_route_params(route, &params);
        RouteSegmentReuseKey::from_parts(route, &RouteSlotKey::default(), None, &params_fingerprint)
    }

    #[test]
    fn frame_scope_records_router_host_and_runtime_slot_without_active_branch() {
        let scope = RouteFrameScope::new(
            RouterInstanceId::new(1),
            RouteHostId::new(2),
            RouteSlotId::new(3),
        );

        assert_eq!(scope.router().raw(), 1);
        assert_eq!(scope.host().raw(), 2);
        assert_eq!(scope.slot().raw(), 3);
    }

    #[test]
    fn frame_identity_records_router_host_slot_route_frame_and_generation() {
        let identity = RouteFrameIdentity::new(
            RouterInstanceId::new(1),
            RouteHostId::new(2),
            RouteSlotId::new(3),
            RouteId::new(4),
            RouteFrameId::new(5),
            RouteGeneration::initial(),
        );

        assert_eq!(identity.router().raw(), 1);
        assert_eq!(identity.host().raw(), 2);
        assert_eq!(identity.slot().raw(), 3);
        assert_eq!(identity.route().raw(), 4);
        assert_eq!(identity.frame().raw(), 5);
        assert_eq!(identity.generation().raw(), 0);
    }

    #[test]
    fn frame_record_tracks_status_slot_key_and_segment_reuse_key() {
        let route = RouteId::new(7);
        let identity = RouteFrameIdentity::new(
            RouterInstanceId::new(1),
            RouteHostId::new(2),
            RouteSlotId::new(3),
            route,
            RouteFrameId::new(8),
            RouteGeneration::initial(),
        );
        let record = RouteFrameRecord::new(
            identity,
            2,
            RouteSlotKey::named("detail").unwrap(),
            segment(route),
            RouteFrameStatus::Building,
        );

        assert_eq!(record.identity(), &identity);
        assert_eq!(record.branch_index(), 2);
        assert_eq!(record.slot_key().as_str(), "detail");
        assert_eq!(record.status(), RouteFrameStatus::Building);
        assert_eq!(record.segment_reuse_key().as_str(), segment(route).as_str());
    }

    #[test]
    fn frame_branch_records_frames_and_branch_fingerprint() {
        let route = RouteId::new(11);
        let identity = RouteFrameIdentity::new(
            RouterInstanceId::new(1),
            RouteHostId::new(2),
            RouteSlotId::new(3),
            route,
            RouteFrameId::new(12),
            RouteGeneration::initial(),
        );
        let record = RouteFrameRecord::new(
            identity,
            0,
            RouteSlotKey::default(),
            segment(route),
            RouteFrameStatus::Matching,
        );
        let search =
            RouteSearchFingerprint::from_location(&crate::location::RouteLocation::new("/items"));
        let fingerprint = RouteBranchFingerprint::from_parts([record.segment_reuse_key()], &search);
        let branch = RouteFrameBranch::new(vec![record.clone()], fingerprint.clone());

        assert_eq!(branch.frames(), &[record]);
        assert_eq!(branch.branch_fingerprint().as_str(), fingerprint.as_str());
    }

    #[test]
    fn cleanup_plan_targets_only_router_owned_subtree() {
        let identity = RouteFrameIdentity::new(
            RouterInstanceId::new(1),
            RouteHostId::new(2),
            RouteSlotId::new(3),
            RouteId::new(4),
            RouteFrameId::new(5),
            RouteGeneration::initial(),
        );
        let cleanup = FrameCleanupPlan::router_owned_partial_output(identity);

        assert_eq!(cleanup.identity(), &identity);
        assert!(cleanup.scope().excludes_app_owned_host());
        assert_eq!(cleanup.scope().frame(), RouteFrameId::new(5));
    }
}
