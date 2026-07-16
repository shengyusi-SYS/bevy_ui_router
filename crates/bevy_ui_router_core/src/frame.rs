//! ```compile_fail
//! use bevy_ui_router_core::prelude::{
//!     RouteFrameId, RouteFrameIdentity, RouteGeneration, RouteHostId, RouteId,
//!     RouteSlotId, RouterInstanceId,
//! };
//!
//! fn forge(
//!     router: RouterInstanceId,
//!     host: RouteHostId,
//!     slot: RouteSlotId,
//!     route: RouteId,
//!     frame: RouteFrameId,
//!     generation: RouteGeneration,
//! ) {
//!     let _ = RouteFrameIdentity::new(router, host, slot, route, frame, generation);
//! }
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_core::frame::FrameAllocator;
//!
//! let _ = FrameAllocator::new();
//! ```

use crate::branch::MatchedBranch;
use crate::branch_diff::BranchDiff;
use crate::fingerprint::{RouteBranchFingerprint, RouteSegmentReuseKey};
use crate::ids::{
    RouteFrameId, RouteGeneration, RouteHostId, RouteId, RouteSlotId, RouterIdentity,
    RouterInstanceId,
};
use crate::meta::ErrorBoundaryKey;
use crate::ownership::CleanupScope;
use crate::slot::RouteSlotKey;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FrameOrigin {
    MatchedRoute(RouteId),
    RouteError(RouteId),
    RouterFallback,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouteFrameVersion {
    router: RouterIdentity,
    host: RouteHostId,
    slot: RouteSlotId,
    origin: FrameOrigin,
    frame: RouteFrameId,
    generation: RouteGeneration,
}

impl RouteFrameVersion {
    #[allow(dead_code)]
    pub(crate) const fn new(
        router: RouterIdentity,
        host: RouteHostId,
        slot: RouteSlotId,
        origin: FrameOrigin,
        frame: RouteFrameId,
        generation: RouteGeneration,
    ) -> Self {
        Self {
            router,
            host,
            slot,
            origin,
            frame,
            generation,
        }
    }

    pub fn router(&self) -> RouterInstanceId {
        self.router.router()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) const fn router_identity(&self) -> RouterIdentity {
        self.router
    }

    pub fn host(&self) -> RouteHostId {
        self.host
    }

    pub fn slot(&self) -> RouteSlotId {
        self.slot
    }

    pub fn origin(&self) -> &FrameOrigin {
        &self.origin
    }

    pub fn frame(&self) -> RouteFrameId {
        self.frame
    }

    pub fn generation(&self) -> RouteGeneration {
        self.generation
    }
}

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

    #[cfg(feature = "test-support")]
    pub(crate) fn with_next_generation(&self) -> Self {
        Self::new(
            self.router,
            self.host,
            self.slot,
            self.route,
            self.frame,
            self.generation.next(),
        )
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

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn with_status(&self, status: RouteFrameStatus) -> Self {
        let mut next = self.clone();
        next.status = status;
        next
    }

    #[cfg(feature = "test-support")]
    pub(crate) fn with_next_generation(&self) -> Self {
        let mut next = self.clone();
        next.identity = self.identity.with_next_generation();
        next
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

    fn belongs_to_scope(&self, scope: RouteFrameScope) -> bool {
        !self.frames.is_empty()
            && self.frames.iter().all(|record| {
                let identity = record.identity();
                identity.router() == scope.router()
                    && identity.host() == scope.host()
                    && identity.slot() == scope.slot()
            })
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn from_matched_branch(
        scope: RouteFrameScope,
        branch: &MatchedBranch,
        reserved_frames: &[RouteFrameId],
    ) -> Self {
        assert_eq!(reserved_frames.len(), branch.routes().len());
        let frames = branch
            .routes()
            .iter()
            .zip(reserved_frames.iter().copied())
            .enumerate()
            .map(|(index, (matched, frame))| {
                let identity = RouteFrameIdentity::new(
                    scope.router(),
                    scope.host(),
                    scope.slot(),
                    matched.route(),
                    frame,
                    RouteGeneration::initial(),
                );
                RouteFrameRecord::new(
                    identity,
                    index,
                    matched.slot().clone(),
                    matched.segment_reuse_key().clone(),
                    RouteFrameStatus::Active,
                )
            })
            .collect();
        Self::new(frames, branch.branch_fingerprint().clone())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ActiveFrameBaseline {
    identity: RouteFrameIdentity,
    branch_index: usize,
}

impl ActiveFrameBaseline {
    fn from_record(record: &RouteFrameRecord) -> Self {
        Self {
            identity: *record.identity(),
            branch_index: record.branch_index(),
        }
    }

    fn matches_record(&self, record: &RouteFrameRecord) -> bool {
        let identity = record.identity();
        self.identity.router() == identity.router()
            && self.identity.host() == identity.host()
            && self.identity.slot() == identity.slot()
            && self.identity.route() == identity.route()
            && self.identity.frame() == identity.frame()
            && self.identity.generation() == identity.generation()
            && self.branch_index == record.branch_index()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ActiveBranchBaseline {
    branch_fingerprint: RouteBranchFingerprint,
    frames: Vec<ActiveFrameBaseline>,
}

impl ActiveBranchBaseline {
    fn from_branch(branch: &RouteFrameBranch) -> Self {
        Self {
            branch_fingerprint: branch.branch_fingerprint().clone(),
            frames: branch
                .frames()
                .iter()
                .map(ActiveFrameBaseline::from_record)
                .collect(),
        }
    }

    fn matches_branch(&self, branch: &RouteFrameBranch) -> bool {
        self.branch_fingerprint == *branch.branch_fingerprint()
            && self.frames.len() == branch.frames().len()
            && self
                .frames
                .iter()
                .zip(branch.frames())
                .all(|(baseline, current)| baseline.matches_record(current))
    }

    pub(crate) const fn branch_fingerprint(&self) -> &RouteBranchFingerprint {
        &self.branch_fingerprint
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrameReconciliationPlan {
    preserved: Vec<RouteFrameRecord>,
    leaving: Vec<RouteFrameRecord>,
    entering: Vec<RouteFrameRecord>,
    baseline_branch: Option<ActiveBranchBaseline>,
    target_fingerprint: RouteBranchFingerprint,
    error_boundaries: BTreeMap<RouteId, Option<ErrorBoundaryKey>>,
}

impl FrameReconciliationPlan {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn from_diff(
        scope: RouteFrameScope,
        active: Option<&RouteFrameBranch>,
        target: &MatchedBranch,
        diff: &BranchDiff,
        reserved_frames: &[RouteFrameId],
    ) -> Self {
        let in_scope_active = active.filter(|branch| branch.belongs_to_scope(scope));
        let baseline_branch = in_scope_active.map(ActiveBranchBaseline::from_branch);
        let target_fingerprint = target.branch_fingerprint().clone();
        let common = diff.common_prefix_len();
        let can_reuse = in_scope_active.is_some_and(|branch| {
            branch.branch_fingerprint() == diff.baseline_branch_fingerprint()
                && &target_fingerprint == diff.target_branch_fingerprint()
                && common <= branch.frames().len()
                && common <= target.routes().len()
        });

        let (preserved, leaving, entering) = if let Some(branch) = in_scope_active {
            if can_reuse {
                let active_frames = branch.frames();
                let preserved = active_frames[..common].to_vec();
                let leaving = active_frames[common..]
                    .iter()
                    .rev()
                    .map(|record| record.with_status(RouteFrameStatus::Leaving))
                    .collect();
                let entering = build_entering_records(
                    scope,
                    target,
                    common,
                    RouteFrameStatus::Building,
                    reserved_frames,
                );
                (preserved, leaving, entering)
            } else {
                (
                    Vec::new(),
                    branch
                        .frames()
                        .iter()
                        .rev()
                        .map(|record| record.with_status(RouteFrameStatus::Leaving))
                        .collect(),
                    build_entering_records(
                        scope,
                        target,
                        0,
                        RouteFrameStatus::Building,
                        reserved_frames,
                    ),
                )
            }
        } else {
            (
                Vec::new(),
                Vec::new(),
                build_entering_records(
                    scope,
                    target,
                    0,
                    RouteFrameStatus::Building,
                    reserved_frames,
                ),
            )
        };

        Self {
            preserved,
            leaving,
            entering,
            baseline_branch,
            target_fingerprint,
            error_boundaries: error_boundaries_from(target),
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn from_first_navigation(
        scope: RouteFrameScope,
        target: &MatchedBranch,
        reserved_frames: &[RouteFrameId],
    ) -> Self {
        let entering = build_entering_records(
            scope,
            target,
            0,
            RouteFrameStatus::Building,
            reserved_frames,
        );
        Self {
            preserved: Vec::new(),
            leaving: Vec::new(),
            entering,
            baseline_branch: None,
            target_fingerprint: target.branch_fingerprint().clone(),
            error_boundaries: error_boundaries_from(target),
        }
    }

    pub fn preserved(&self) -> &[RouteFrameRecord] {
        &self.preserved
    }

    pub fn leaving(&self) -> &[RouteFrameRecord] {
        &self.leaving
    }

    pub fn entering(&self) -> &[RouteFrameRecord] {
        &self.entering
    }

    pub fn baseline_fingerprint(&self) -> Option<&RouteBranchFingerprint> {
        self.baseline_branch
            .as_ref()
            .map(ActiveBranchBaseline::branch_fingerprint)
    }

    pub const fn target_fingerprint(&self) -> &RouteBranchFingerprint {
        &self.target_fingerprint
    }

    pub(crate) fn matches_current_active(&self, current_active: Option<&RouteFrameBranch>) -> bool {
        match (&self.baseline_branch, current_active) {
            (None, None) => true,
            (Some(_), None) | (None, Some(_)) => false,
            (Some(baseline), Some(current)) => baseline.matches_branch(current),
        }
    }

    pub(crate) fn nearest_error_boundary_for(&self, route: RouteId) -> Option<&ErrorBoundaryKey> {
        self.error_boundaries.get(&route).and_then(Option::as_ref)
    }
}

fn error_boundaries_from(branch: &MatchedBranch) -> BTreeMap<RouteId, Option<ErrorBoundaryKey>> {
    branch
        .routes()
        .iter()
        .map(|route| {
            (
                route.route(),
                branch.nearest_error_boundary_for(route.route()).cloned(),
            )
        })
        .collect()
}

fn build_entering_records(
    scope: RouteFrameScope,
    target: &MatchedBranch,
    start_index: usize,
    status: RouteFrameStatus,
    reserved_frames: &[RouteFrameId],
) -> Vec<RouteFrameRecord> {
    assert_eq!(reserved_frames.len(), target.routes().len());
    target.routes()[start_index..]
        .iter()
        .enumerate()
        .map(|(offset, matched)| {
            let branch_index = start_index + offset;
            let identity = RouteFrameIdentity::new(
                scope.router(),
                scope.host(),
                scope.slot(),
                matched.route(),
                reserved_frames[branch_index],
                RouteGeneration::initial(),
            );
            RouteFrameRecord::new(
                identity,
                branch_index,
                matched.slot().clone(),
                matched.segment_reuse_key().clone(),
                status,
            )
        })
        .collect()
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
    use crate::branch_diff::BranchDiff;
    use crate::fingerprint::{RouteParamsFingerprint, RouteSearchFingerprint};
    use crate::ids::CoreIdAllocator;
    use crate::location::RouteLocation;
    use crate::matcher::RouteMatcher;
    use crate::meta::ErrorBoundaryKey;
    use crate::pattern::RoutePattern;
    use crate::route_tree::{RouteNodeOptions, RouteTree};

    #[test]
    fn frame_versions_preserve_world_provenance_behind_raw_router_projection() {
        let mut first = CoreIdAllocator::new().unwrap();
        let first_frame = RouteFrameVersion::new(
            first.allocate_router_identity().unwrap(),
            first.allocate_host().unwrap(),
            first.allocate_slot().unwrap(),
            FrameOrigin::RouterFallback,
            first.allocate_frame().unwrap(),
            RouteGeneration::initial(),
        );

        let mut second = CoreIdAllocator::new().unwrap();
        let second_frame = RouteFrameVersion::new(
            second.allocate_router_identity().unwrap(),
            second.allocate_host().unwrap(),
            second.allocate_slot().unwrap(),
            FrameOrigin::RouterFallback,
            second.allocate_frame().unwrap(),
            RouteGeneration::initial(),
        );

        assert_eq!(first_frame.router(), second_frame.router());
        assert_eq!(first_frame.host(), second_frame.host());
        assert_eq!(first_frame.slot(), second_frame.slot());
        assert_ne!(first_frame, second_frame);
    }

    fn sample_items_matcher() -> RouteMatcher {
        let mut builder = RouteTree::builder()
            .root(RoutePattern::path("/").unwrap())
            .unwrap();
        let root = builder.root_id().unwrap();
        let items = builder
            .route(root, RoutePattern::path("items").unwrap())
            .unwrap();
        builder
            .route(items, RoutePattern::path(":id").unwrap())
            .unwrap();
        RouteMatcher::new(builder.build().unwrap())
    }

    fn sample_nested_boundary_matcher() -> RouteMatcher {
        let mut builder = RouteTree::builder()
            .root(RoutePattern::path("/").unwrap())
            .unwrap();
        let root = builder.root_id().unwrap();
        let section = builder
            .route_with_options(
                root,
                RoutePattern::path("section").unwrap(),
                RouteNodeOptions::new()
                    .with_error_boundary(ErrorBoundaryKey::named("section").unwrap()),
            )
            .unwrap();
        let items = builder
            .route_with_options(
                section,
                RoutePattern::path("items").unwrap(),
                RouteNodeOptions::new()
                    .with_error_boundary(ErrorBoundaryKey::named("items").unwrap()),
            )
            .unwrap();
        builder
            .route(items, RoutePattern::path(":id").unwrap())
            .unwrap();
        RouteMatcher::new(builder.build().unwrap())
    }

    fn reserve_frame_ids(allocator: &mut CoreIdAllocator, count: usize) -> Vec<RouteFrameId> {
        (0..count)
            .map(|_| allocator.allocate_frame().unwrap())
            .collect()
    }

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
    fn frame_record_with_status_preserves_identity_and_metadata() {
        let route = RouteId::new(13);
        let identity = RouteFrameIdentity::new(
            RouterInstanceId::new(1),
            RouteHostId::new(2),
            RouteSlotId::new(3),
            route,
            RouteFrameId::new(14),
            RouteGeneration::initial(),
        );
        let record = RouteFrameRecord::new(
            identity,
            4,
            RouteSlotKey::named("detail").unwrap(),
            segment(route),
            RouteFrameStatus::Building,
        );
        let next = record.with_status(RouteFrameStatus::Active);

        assert_eq!(next.identity(), record.identity());
        assert_eq!(next.branch_index(), record.branch_index());
        assert_eq!(next.slot_key(), record.slot_key());
        assert_eq!(next.segment_reuse_key(), record.segment_reuse_key());
        assert_eq!(next.status(), RouteFrameStatus::Active);
        assert_eq!(record.status(), RouteFrameStatus::Building);
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

    #[test]
    fn first_navigation_enters_full_branch_without_active_snapshot() {
        let matcher = sample_items_matcher();
        let target = matcher
            .match_location(&RouteLocation::new("/items/1"))
            .unwrap();
        let mut allocator = CoreIdAllocator::new().unwrap();
        let scope = RouteFrameScope::new(
            RouterInstanceId::new(1),
            RouteHostId::new(2),
            RouteSlotId::new(3),
        );

        let reserved = reserve_frame_ids(&mut allocator, target.routes().len());
        let plan = FrameReconciliationPlan::from_first_navigation(scope, &target, &reserved);

        assert!(plan.baseline_fingerprint().is_none());
        assert!(plan.preserved().is_empty());
        assert!(plan.leaving().is_empty());
        assert_eq!(plan.entering().len(), target.routes().len());
        assert_eq!(
            plan.entering()[0].identity().router(),
            RouterInstanceId::new(1)
        );
        assert_eq!(plan.entering()[0].status(), RouteFrameStatus::Building);
    }

    #[test]
    fn reconciliation_preserves_common_prefix_and_allocates_entering_suffix() {
        let matcher = sample_items_matcher();
        let old_branch = matcher
            .match_location(&RouteLocation::new("/items/1"))
            .unwrap();
        let new_branch = matcher
            .match_location(&RouteLocation::new("/items/2"))
            .unwrap();
        let diff = BranchDiff::compare(&old_branch, &new_branch);
        let mut allocator = CoreIdAllocator::new().unwrap();
        let scope = RouteFrameScope::new(
            RouterInstanceId::new(1),
            RouteHostId::new(2),
            RouteSlotId::new(3),
        );
        let old_reserved = reserve_frame_ids(&mut allocator, old_branch.routes().len());
        let active = RouteFrameBranch::from_matched_branch(scope, &old_branch, &old_reserved);
        let new_reserved = reserve_frame_ids(&mut allocator, new_branch.routes().len());

        let plan = FrameReconciliationPlan::from_diff(
            scope,
            Some(&active),
            &new_branch,
            &diff,
            &new_reserved,
        );

        assert_eq!(plan.preserved().len(), 2);
        assert_eq!(plan.leaving().len(), 1);
        assert_eq!(plan.entering().len(), 1);
        assert_eq!(
            plan.preserved()[0].identity().frame(),
            active.frames()[0].identity().frame()
        );
        assert_ne!(
            plan.entering()[0].identity().frame(),
            active.frames()[2].identity().frame()
        );
        assert_eq!(plan.entering()[0].status(), RouteFrameStatus::Building);
    }

    #[test]
    fn reconciliation_fails_closed_when_active_branch_does_not_match_diff_baseline() {
        let matcher = sample_items_matcher();
        let stale_branch = matcher
            .match_location(&RouteLocation::new("/items/1"))
            .unwrap();
        let old_branch = matcher
            .match_location(&RouteLocation::new("/items/2"))
            .unwrap();
        let new_branch = matcher
            .match_location(&RouteLocation::new("/items/3"))
            .unwrap();
        let diff = BranchDiff::compare(&old_branch, &new_branch);
        let mut allocator = CoreIdAllocator::new().unwrap();
        let scope = RouteFrameScope::new(
            RouterInstanceId::new(1),
            RouteHostId::new(2),
            RouteSlotId::new(3),
        );
        let stale_reserved = reserve_frame_ids(&mut allocator, stale_branch.routes().len());
        let active = RouteFrameBranch::from_matched_branch(scope, &stale_branch, &stale_reserved);
        let new_reserved = reserve_frame_ids(&mut allocator, new_branch.routes().len());

        let plan = FrameReconciliationPlan::from_diff(
            scope,
            Some(&active),
            &new_branch,
            &diff,
            &new_reserved,
        );

        assert_eq!(
            plan.baseline_fingerprint(),
            Some(active.branch_fingerprint())
        );
        assert!(plan.preserved().is_empty());
        assert_eq!(plan.leaving().len(), active.frames().len());
        assert_eq!(plan.entering().len(), new_branch.routes().len());
        assert_eq!(plan.entering()[0].branch_index(), 0);
    }

    #[test]
    fn reconciliation_does_not_leave_or_preserve_foreign_scope_active_branch() {
        let matcher = sample_items_matcher();
        let old_branch = matcher
            .match_location(&RouteLocation::new("/items/1"))
            .unwrap();
        let new_branch = matcher
            .match_location(&RouteLocation::new("/items/2"))
            .unwrap();
        let diff = BranchDiff::compare(&old_branch, &new_branch);
        let mut allocator = CoreIdAllocator::new().unwrap();
        let active_scope = RouteFrameScope::new(
            RouterInstanceId::new(9),
            RouteHostId::new(9),
            RouteSlotId::new(9),
        );
        let requested_scope = RouteFrameScope::new(
            RouterInstanceId::new(1),
            RouteHostId::new(2),
            RouteSlotId::new(3),
        );
        let old_reserved = reserve_frame_ids(&mut allocator, old_branch.routes().len());
        let active =
            RouteFrameBranch::from_matched_branch(active_scope, &old_branch, &old_reserved);
        let new_reserved = reserve_frame_ids(&mut allocator, new_branch.routes().len());

        let plan = FrameReconciliationPlan::from_diff(
            requested_scope,
            Some(&active),
            &new_branch,
            &diff,
            &new_reserved,
        );

        assert!(plan.baseline_fingerprint().is_none());
        assert!(plan.preserved().is_empty());
        assert!(plan.leaving().is_empty());
        assert_eq!(plan.entering().len(), new_branch.routes().len());
        assert_eq!(
            plan.entering()[0].identity().router(),
            requested_scope.router()
        );
    }

    #[test]
    fn fail_closed_diff_replaces_entire_frame_branch() {
        let mut old_builder = RouteTree::builder()
            .root(RoutePattern::path("/").unwrap())
            .unwrap();
        let old_root = old_builder.root_id().unwrap();
        let old_items = old_builder
            .route(old_root, RoutePattern::path("items").unwrap())
            .unwrap();
        old_builder
            .route(old_items, RoutePattern::path(":id").unwrap())
            .unwrap();
        let old_matcher = RouteMatcher::new(old_builder.build().unwrap());

        let mut new_builder = RouteTree::builder()
            .root(RoutePattern::path("/").unwrap())
            .unwrap();
        let new_root = new_builder.root_id().unwrap();
        let layout = new_builder
            .route(new_root, RoutePattern::pathless())
            .unwrap();
        let items = new_builder
            .route(layout, RoutePattern::path("items").unwrap())
            .unwrap();
        new_builder
            .route(items, RoutePattern::path(":id").unwrap())
            .unwrap();
        let new_matcher = RouteMatcher::new(new_builder.build().unwrap());

        let old_branch = old_matcher
            .match_location(&RouteLocation::new("/items/1"))
            .unwrap();
        let new_branch = new_matcher
            .match_location(&RouteLocation::new("/items/1"))
            .unwrap();
        let diff = BranchDiff::compare(&old_branch, &new_branch);
        let mut allocator = CoreIdAllocator::new().unwrap();
        let scope = RouteFrameScope::new(
            RouterInstanceId::new(1),
            RouteHostId::new(2),
            RouteSlotId::new(3),
        );
        let old_reserved = reserve_frame_ids(&mut allocator, old_branch.routes().len());
        let active = RouteFrameBranch::from_matched_branch(scope, &old_branch, &old_reserved);
        let new_reserved = reserve_frame_ids(&mut allocator, new_branch.routes().len());

        let plan = FrameReconciliationPlan::from_diff(
            scope,
            Some(&active),
            &new_branch,
            &diff,
            &new_reserved,
        );

        assert!(diff.provenance_changed());
        assert!(plan.preserved().is_empty());
        assert_eq!(plan.leaving().len(), active.frames().len());
        assert_eq!(plan.entering().len(), new_branch.routes().len());
    }

    #[test]
    fn reconciliation_plan_tracks_nearest_error_boundary_by_route() {
        let matcher = sample_nested_boundary_matcher();
        let old_branch = matcher
            .match_location(&RouteLocation::new("/section/items/1"))
            .unwrap();
        let new_branch = matcher
            .match_location(&RouteLocation::new("/section/items/2"))
            .unwrap();
        let diff = BranchDiff::compare(&old_branch, &new_branch);
        let mut allocator = CoreIdAllocator::new().unwrap();
        let scope = RouteFrameScope::new(
            RouterInstanceId::new(1),
            RouteHostId::new(2),
            RouteSlotId::new(3),
        );
        let old_reserved = reserve_frame_ids(&mut allocator, old_branch.routes().len());
        let active = RouteFrameBranch::from_matched_branch(scope, &old_branch, &old_reserved);
        let new_reserved = reserve_frame_ids(&mut allocator, new_branch.routes().len());

        let plan = FrameReconciliationPlan::from_diff(
            scope,
            Some(&active),
            &new_branch,
            &diff,
            &new_reserved,
        );

        let leaf = new_branch.leaf().unwrap().route();
        let root = new_branch.routes()[0].route();

        assert_eq!(
            plan.nearest_error_boundary_for(leaf)
                .map(ErrorBoundaryKey::as_str),
            Some("items")
        );
        assert_eq!(plan.nearest_error_boundary_for(root), None);
        assert_eq!(plan.nearest_error_boundary_for(RouteId::new(999)), None);
    }

    #[test]
    fn first_navigation_constructor_records_router_level_fallback_without_attachment_step() {
        let matcher = sample_items_matcher();
        let target = matcher
            .match_location(&RouteLocation::new("/items/1"))
            .unwrap();
        let leaf = target.leaf().unwrap().route();
        let mut allocator = CoreIdAllocator::new().unwrap();
        let scope = RouteFrameScope::new(
            RouterInstanceId::new(1),
            RouteHostId::new(2),
            RouteSlotId::new(3),
        );

        let reserved = reserve_frame_ids(&mut allocator, target.routes().len());
        let plan = FrameReconciliationPlan::from_first_navigation(scope, &target, &reserved);

        assert_eq!(plan.nearest_error_boundary_for(leaf), None);
    }
}
