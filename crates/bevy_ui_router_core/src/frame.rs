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
use crate::location::RouteLocation;
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
pub(crate) struct LogicalChildSlotBinding {
    parent_branch_index: usize,
    parent: RouteFrameVersion,
    outlet: RouteSlotKey,
    slot: RouteSlotId,
    child: Option<RouteFrameVersion>,
}

impl LogicalChildSlotBinding {
    pub(crate) fn occupied(
        parent_branch_index: usize,
        parent: RouteFrameVersion,
        outlet: RouteSlotKey,
        child: RouteFrameVersion,
    ) -> Self {
        Self {
            parent_branch_index,
            parent,
            outlet,
            slot: child.slot(),
            child: Some(child),
        }
    }

    #[allow(dead_code)]
    pub(crate) fn retained_empty(
        parent_branch_index: usize,
        parent: RouteFrameVersion,
        outlet: RouteSlotKey,
        slot: RouteSlotId,
    ) -> Self {
        Self {
            parent_branch_index,
            parent,
            outlet,
            slot,
            child: None,
        }
    }

    pub(crate) const fn parent_branch_index(&self) -> usize {
        self.parent_branch_index
    }

    pub(crate) const fn parent(&self) -> &RouteFrameVersion {
        &self.parent
    }

    pub(crate) const fn outlet(&self) -> &RouteSlotKey {
        &self.outlet
    }

    pub(crate) const fn slot(&self) -> RouteSlotId {
        self.slot
    }

    pub(crate) const fn child(&self) -> Option<&RouteFrameVersion> {
        self.child.as_ref()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProjectedChildSlotRoleKind {
    PreservedActive,
    RetainEmpty,
    ActivateExisting,
    MountNew,
    Leaving,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProjectedChildSlotRole {
    parent_branch_index: usize,
    outlet: RouteSlotKey,
    kind: ProjectedChildSlotRoleKind,
    binding: Option<LogicalChildSlotBinding>,
}

impl ProjectedChildSlotRole {
    fn existing(kind: ProjectedChildSlotRoleKind, binding: LogicalChildSlotBinding) -> Self {
        Self {
            parent_branch_index: binding.parent_branch_index(),
            outlet: binding.outlet().clone(),
            kind,
            binding: Some(binding),
        }
    }

    fn mount_new(parent_branch_index: usize, outlet: RouteSlotKey) -> Self {
        Self {
            parent_branch_index,
            outlet,
            kind: ProjectedChildSlotRoleKind::MountNew,
            binding: None,
        }
    }

    pub(crate) const fn parent_branch_index(&self) -> usize {
        self.parent_branch_index
    }

    pub(crate) const fn outlet(&self) -> &RouteSlotKey {
        &self.outlet
    }

    pub(crate) const fn kind(&self) -> ProjectedChildSlotRoleKind {
        self.kind
    }

    pub(crate) const fn binding(&self) -> Option<&LogicalChildSlotBinding> {
        self.binding.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProjectedExistingFrame {
    branch_index: usize,
    version: RouteFrameVersion,
}

impl ProjectedExistingFrame {
    fn new(branch_index: usize, version: RouteFrameVersion) -> Self {
        Self {
            branch_index,
            version,
        }
    }

    pub(crate) const fn branch_index(&self) -> usize {
        self.branch_index
    }

    pub(crate) const fn version(&self) -> &RouteFrameVersion {
        &self.version
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProjectedEnteringSlot {
    Existing(RouteSlotId),
    MountNew,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProjectedEnteringFrame {
    branch_index: usize,
    route: RouteId,
    origin: FrameOrigin,
    frame: RouteFrameId,
    generation: RouteGeneration,
    slot: ProjectedEnteringSlot,
}

impl ProjectedEnteringFrame {
    pub(crate) const fn branch_index(&self) -> usize {
        self.branch_index
    }

    pub(crate) const fn route(&self) -> RouteId {
        self.route
    }

    pub(crate) const fn origin(&self) -> &FrameOrigin {
        &self.origin
    }

    pub(crate) const fn frame(&self) -> RouteFrameId {
        self.frame
    }

    pub(crate) const fn generation(&self) -> RouteGeneration {
        self.generation
    }

    pub(crate) const fn slot(&self) -> ProjectedEnteringSlot {
        self.slot
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RouteReconciliationBaseline {
    router: RouterIdentity,
    host: RouteHostId,
    host_epoch: u64,
    top_slot: RouteSlotId,
    location: Option<RouteLocation>,
    branch_fingerprint: Option<RouteBranchFingerprint>,
    frames: Vec<RouteFrameVersion>,
    child_slots: Vec<LogicalChildSlotBinding>,
}

impl RouteReconciliationBaseline {
    fn matches(
        &self,
        router: RouterIdentity,
        host: RouteHostId,
        host_epoch: u64,
        top_slot: RouteSlotId,
        location: Option<&RouteLocation>,
        branch: Option<&MatchedBranch>,
        frames: &[RouteFrameVersion],
        child_slots: &[LogicalChildSlotBinding],
    ) -> bool {
        self.router == router
            && self.host == host
            && self.host_epoch == host_epoch
            && self.top_slot == top_slot
            && self.location.as_ref() == location
            && self.branch_fingerprint.as_ref() == branch.map(MatchedBranch::branch_fingerprint)
            && self.frames == frames
            && self.child_slots == child_slots
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReconciliationProjectionError {
    InvalidActiveBaseline,
    GenerationExhausted,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RouteReconciliationProjection {
    baseline: RouteReconciliationBaseline,
    preserved: Vec<ProjectedExistingFrame>,
    leaving: Vec<ProjectedExistingFrame>,
    entering: Vec<ProjectedEnteringFrame>,
    retained: Vec<crate::engine::retained::RetainedLeafProjection>,
    instances_selected: bool,
    child_slots: Vec<ProjectedChildSlotRole>,
    target_fingerprint: RouteBranchFingerprint,
    target: ReconciliationProjectionTarget,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ReconciliationProjectionTarget {
    MatchedBranch,
    BoundError {
        boundary_index: usize,
        boundary_route: RouteId,
        frame: RouteFrameId,
        generation: RouteGeneration,
    },
}

impl RouteReconciliationProjection {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_running(
        router: RouterIdentity,
        host: RouteHostId,
        host_epoch: u64,
        top_slot: RouteSlotId,
        active_location: Option<&RouteLocation>,
        active_branch: Option<&MatchedBranch>,
        active_frames: &[RouteFrameVersion],
        active_child_slots: &[LogicalChildSlotBinding],
        target: &MatchedBranch,
        reserved_frames: &[RouteFrameId],
        fragment_replacement: bool,
    ) -> Result<Self, ReconciliationProjectionError> {
        if reserved_frames.len() != target.routes().len()
            || !active_baseline_is_consistent(
                router,
                host,
                top_slot,
                active_location,
                active_branch,
                active_frames,
                active_child_slots,
            )
        {
            return Err(ReconciliationProjectionError::InvalidActiveBaseline);
        }

        let common_prefix = match active_branch {
            Some(active) if fragment_replacement => active.routes().len().saturating_sub(1),
            Some(active) => BranchDiff::compare(active, target).common_prefix_len(),
            None => 0,
        };
        if common_prefix > active_frames.len() || common_prefix > target.routes().len() {
            return Err(ReconciliationProjectionError::InvalidActiveBaseline);
        }

        let preserved = active_frames[..common_prefix]
            .iter()
            .cloned()
            .enumerate()
            .map(|(branch_index, version)| ProjectedExistingFrame::new(branch_index, version))
            .collect();
        let leaving = active_frames[common_prefix..]
            .iter()
            .cloned()
            .enumerate()
            .rev()
            .map(|(offset, version)| ProjectedExistingFrame::new(common_prefix + offset, version))
            .collect();

        let child_slots = project_child_slots(common_prefix, target, active_child_slots);
        let mut entering = Vec::with_capacity(target.routes().len() - common_prefix);
        for branch_index in common_prefix..target.routes().len() {
            let route = &target.routes()[branch_index];
            let (frame, generation) = if fragment_replacement
                && branch_index + 1 == target.routes().len()
                && branch_index < active_frames.len()
            {
                let active = &active_frames[branch_index];
                (
                    active.frame(),
                    active
                        .generation()
                        .checked_next()
                        .map_err(|_| ReconciliationProjectionError::GenerationExhausted)?,
                )
            } else {
                (reserved_frames[branch_index], RouteGeneration::initial())
            };
            let slot = if branch_index == 0 {
                ProjectedEnteringSlot::Existing(top_slot)
            } else {
                child_slots
                    .iter()
                    .find(|slot| {
                        slot.parent_branch_index() + 1 == branch_index
                            && slot.outlet() == route.slot()
                    })
                    .map_or(ProjectedEnteringSlot::MountNew, |slot| {
                        slot.binding()
                            .map_or(ProjectedEnteringSlot::MountNew, |binding| {
                                ProjectedEnteringSlot::Existing(binding.slot())
                            })
                    })
            };
            entering.push(ProjectedEnteringFrame {
                branch_index,
                route: route.route(),
                origin: FrameOrigin::MatchedRoute(route.route()),
                frame,
                generation,
                slot,
            });
        }

        Ok(Self {
            baseline: RouteReconciliationBaseline {
                router,
                host,
                host_epoch,
                top_slot,
                location: active_location.cloned(),
                branch_fingerprint: active_branch.map(|branch| branch.branch_fingerprint().clone()),
                frames: active_frames.to_vec(),
                child_slots: active_child_slots.to_vec(),
            },
            preserved,
            leaving,
            entering,
            retained: Vec::new(),
            instances_selected: false,
            child_slots,
            target_fingerprint: target.branch_fingerprint().clone(),
            target: ReconciliationProjectionTarget::MatchedBranch,
        })
    }

    pub(crate) fn retarget_bound_error(
        &self,
        target: &MatchedBranch,
        boundary_index: usize,
        prepared_prefix: &[RouteFrameVersion],
        frame: RouteFrameId,
        generation: RouteGeneration,
        slot: ProjectedEnteringSlot,
    ) -> Option<Self> {
        let boundary = target.routes().get(boundary_index)?;
        if target.branch_fingerprint() != &self.target_fingerprint
            || boundary_index > self.baseline.frames.len().max(target.routes().len())
        {
            return None;
        }
        let mut preserved = self
            .preserved
            .iter()
            .filter(|candidate| candidate.branch_index < boundary_index)
            .cloned()
            .collect::<Vec<_>>();
        preserved.sort_by_key(|candidate| candidate.branch_index);
        if preserved
            .iter()
            .enumerate()
            .any(|(index, candidate)| index != candidate.branch_index)
        {
            return None;
        }
        let prepared_start = preserved.len();
        if prepared_start + prepared_prefix.len() != boundary_index {
            return None;
        }
        let mut entering = Vec::with_capacity(prepared_prefix.len() + 1);
        for (offset, version) in prepared_prefix.iter().enumerate() {
            let branch_index = prepared_start + offset;
            let route = target.routes().get(branch_index)?;
            let projected = self
                .entering
                .iter()
                .find(|candidate| candidate.branch_index == branch_index)?;
            let slot_matches = match projected.slot {
                ProjectedEnteringSlot::Existing(expected) => expected == version.slot(),
                ProjectedEnteringSlot::MountNew => true,
            };
            if version.origin() != &FrameOrigin::MatchedRoute(route.route())
                || projected.route != route.route()
                || &projected.origin != version.origin()
                || projected.frame != version.frame()
                || projected.generation != version.generation()
                || !slot_matches
            {
                return None;
            }
            entering.push(ProjectedEnteringFrame {
                branch_index,
                route: route.route(),
                origin: version.origin().clone(),
                frame: version.frame(),
                generation: version.generation(),
                slot: projected.slot,
            });
        }
        entering.push(ProjectedEnteringFrame {
            branch_index: boundary_index,
            route: boundary.route(),
            origin: FrameOrigin::RouteError(boundary.route()),
            frame,
            generation,
            slot,
        });

        let leaving = self
            .baseline
            .frames
            .iter()
            .cloned()
            .enumerate()
            .filter(|(index, _)| *index >= prepared_start)
            .rev()
            .map(|(index, version)| ProjectedExistingFrame::new(index, version))
            .collect();
        let mut child_slots = Vec::new();
        let mut consumed = Vec::new();
        for child_index in 1..boundary_index {
            let route = target.routes().get(child_index)?;
            let parent_index = child_index - 1;
            let parent = preserved
                .iter()
                .find(|candidate| candidate.branch_index == parent_index)
                .map(|candidate| &candidate.version)
                .or_else(|| prepared_prefix.get(parent_index.checked_sub(prepared_start)?))?;
            let role = self.child_slots.iter().find(|role| {
                role.parent_branch_index == parent_index
                    && role.outlet == *route.slot()
                    && role.kind != ProjectedChildSlotRoleKind::Leaving
                    && role
                        .binding()
                        .is_none_or(|binding| binding.parent() == parent)
            })?;
            child_slots.push(role.clone());
            if let Some(binding) = role.binding.as_ref() {
                consumed.push(binding.slot());
            }
        }
        if boundary_index > 0 {
            let parent_index = boundary_index - 1;
            match slot {
                ProjectedEnteringSlot::Existing(expected) => {
                    let parent = preserved
                        .iter()
                        .find(|candidate| candidate.branch_index == parent_index)
                        .map(|candidate| candidate.version.clone())
                        .or_else(|| {
                            prepared_prefix
                                .get(parent_index.checked_sub(prepared_start)?)
                                .cloned()
                        })?;
                    let role = self.child_slots.iter().find(|role| {
                        role.parent_branch_index == parent_index
                            && role.outlet == *boundary.slot()
                            && role
                                .binding()
                                .is_none_or(|binding| binding.parent() == &parent)
                    })?;
                    if let Some(binding) = role.binding.as_ref() {
                        if binding.parent() != &parent
                            || binding.outlet() != boundary.slot()
                            || binding.slot() != expected
                        {
                            return None;
                        }
                        let kind = if binding.child().is_some() {
                            ProjectedChildSlotRoleKind::PreservedActive
                        } else {
                            ProjectedChildSlotRoleKind::ActivateExisting
                        };
                        child_slots.push(ProjectedChildSlotRole::existing(kind, binding.clone()));
                    } else if role.kind == ProjectedChildSlotRoleKind::MountNew {
                        child_slots.push(role.clone());
                    } else {
                        return None;
                    }
                    consumed.push(expected);
                }
                ProjectedEnteringSlot::MountNew => child_slots.push(
                    ProjectedChildSlotRole::mount_new(parent_index, boundary.slot().clone()),
                ),
            }
        }
        let mut residual = self
            .baseline
            .child_slots
            .iter()
            .filter(|binding| !consumed.contains(&binding.slot()))
            .cloned()
            .map(|binding| {
                let kind = if binding.parent_branch_index() < prepared_start {
                    ProjectedChildSlotRoleKind::RetainEmpty
                } else {
                    ProjectedChildSlotRoleKind::Leaving
                };
                ProjectedChildSlotRole::existing(kind, binding)
            })
            .collect::<Vec<_>>();
        residual.sort_by(|left, right| {
            right
                .parent_branch_index()
                .cmp(&left.parent_branch_index())
                .then_with(|| left.outlet().cmp(right.outlet()))
        });
        child_slots.extend(residual);
        for role in self
            .child_slots
            .iter()
            .filter(|role| role.kind() == ProjectedChildSlotRoleKind::RetainEmpty)
        {
            if let Some(binding) = role.binding() {
                if binding.parent_branch_index() < boundary_index
                    && !child_slots
                        .iter()
                        .filter_map(|existing| existing.binding())
                        .any(|existing| existing.slot() == binding.slot())
                {
                    child_slots.push(role.clone());
                }
            }
        }

        Some(Self {
            baseline: self.baseline.clone(),
            preserved,
            leaving,
            entering,
            retained: self
                .retained
                .iter()
                .take(boundary_index.saturating_sub(self.preserved.len()))
                .cloned()
                .collect(),
            instances_selected: true,
            child_slots,
            target_fingerprint: self.target_fingerprint.clone(),
            target: ReconciliationProjectionTarget::BoundError {
                boundary_index,
                boundary_route: boundary.route(),
                frame,
                generation,
            },
        })
    }

    /// Select the entire retained prefix once; current remains the actual baseline.
    pub(crate) fn with_instance_selection(
        &self,
        selected: &[crate::engine::retained::RetainedLeafProjection],
        target: &MatchedBranch,
    ) -> Option<Self> {
        if self.instances_selected
            || self.target != ReconciliationProjectionTarget::MatchedBranch
            || target.branch_fingerprint() != &self.target_fingerprint
            || selected.len() > self.entering.len()
        {
            return None;
        }
        let start = self.preserved.len();
        let mut next = self.clone();
        for (offset, instance) in selected.iter().enumerate() {
            let index = start + offset;
            let entering = self.entering.get(offset)?;
            let route = target.routes().get(index)?;
            let parent = if offset == 0 {
                self.preserved.last()?.version()
            } else {
                selected.get(offset - 1)?.frame()
            };
            if index == 0
                || entering.branch_index() != index
                || instance.parent() != parent
                || instance.outlet() != route.slot()
                || instance.frame().origin() != &FrameOrigin::MatchedRoute(route.route())
                || instance.frame().router_identity() != self.baseline.router
                || instance.frame().host() != self.baseline.host
            {
                return None;
            }
            if offset == 0 && entering.slot() != ProjectedEnteringSlot::Existing(instance.slot()) {
                return None;
            }
        }
        next.retained = selected.to_vec();
        for (offset, instance) in selected.iter().enumerate() {
            next.entering[offset].frame = instance.frame().frame();
            next.entering[offset].generation = instance.frame().generation();
            next.entering[offset].slot = ProjectedEnteringSlot::Existing(instance.slot());
        }
        next.instances_selected = true;
        // Replace old fresh-parent slot assumptions with the selected parent's own slots.
        for (offset, instance) in selected.iter().enumerate() {
            let index = start + offset;
            let requested = target.routes().get(index + 1).map(|route| route.slot());
            next.child_slots
                .retain(|role| role.parent_branch_index() != index || role.binding().is_some());
            for (slot_index, own) in instance.own_slots().iter().enumerate() {
                if own.parent() != instance.frame()
                    || instance.own_slots()[..slot_index]
                        .iter()
                        .any(|old| old.slot() == own.slot() || old.outlet() == own.outlet())
                    || next
                        .child_slots
                        .iter()
                        .filter_map(|role| role.binding())
                        .any(|old| old.slot() == own.slot())
                {
                    return None;
                }
                let used = requested == Some(own.outlet());
                next.child_slots.push(ProjectedChildSlotRole::existing(
                    if used {
                        ProjectedChildSlotRoleKind::ActivateExisting
                    } else {
                        ProjectedChildSlotRoleKind::RetainEmpty
                    },
                    LogicalChildSlotBinding::retained_empty(
                        index,
                        instance.frame().clone(),
                        own.outlet().clone(),
                        own.slot(),
                    ),
                ));
            }
            if let Some(outlet) = requested {
                if let Some(own) = instance
                    .own_slots()
                    .iter()
                    .find(|own| own.outlet() == outlet)
                {
                    if let Some(entering) = next
                        .entering
                        .iter_mut()
                        .find(|entering| entering.branch_index == index + 1)
                    {
                        entering.slot = ProjectedEnteringSlot::Existing(own.slot());
                    }
                    if let Some(child) = selected.get(offset + 1) {
                        if child.slot() != own.slot() {
                            return None;
                        }
                    }
                } else {
                    if selected.get(offset + 1).is_some() {
                        return None;
                    }
                    next.child_slots
                        .push(ProjectedChildSlotRole::mount_new(index, outlet.clone()));
                }
            }
        }
        Some(next)
    }

    pub(crate) fn invalidate_retained_suffix(
        &self,
        index: usize,
        target: &MatchedBranch,
        reserved: &[RouteFrameId],
    ) -> Option<Self> {
        let start = self.preserved.len();
        if !self.instances_selected
            || index < start
            || index >= start + self.retained.len()
            || reserved.len() != target.routes().len()
        {
            return None;
        }
        let mut next = self.clone();
        next.retained.truncate(index - start);
        next.entering.retain(|frame| frame.branch_index() < index);
        next.child_slots.retain(|role| {
            role.parent_branch_index() < index || role.kind() == ProjectedChildSlotRoleKind::Leaving
        });
        for child_index in index..target.routes().len() {
            let route = target.routes().get(child_index)?;
            let slot = if child_index == index {
                let parent = next
                    .preserved
                    .iter()
                    .find(|frame| frame.branch_index() == index - 1)
                    .map(|frame| frame.version())
                    .or_else(|| {
                        next.retained
                            .iter()
                            .find(|instance| {
                                instance.frame().origin()
                                    == &FrameOrigin::MatchedRoute(
                                        target.routes()[index - 1].route(),
                                    )
                            })
                            .map(|instance| instance.frame())
                    })?;
                let role = next.child_slots.iter().find(|role| {
                    role.parent_branch_index() == index - 1
                        && role.outlet() == route.slot()
                        && role.kind() != ProjectedChildSlotRoleKind::Leaving
                        && role
                            .binding()
                            .is_none_or(|binding| binding.parent() == parent)
                })?;
                match role.binding() {
                    Some(binding) => ProjectedEnteringSlot::Existing(binding.slot()),
                    None => ProjectedEnteringSlot::MountNew,
                }
            } else {
                ProjectedEnteringSlot::MountNew
            };
            next.entering.push(ProjectedEnteringFrame {
                branch_index: child_index,
                route: route.route(),
                origin: FrameOrigin::MatchedRoute(route.route()),
                frame: reserved[child_index],
                generation: RouteGeneration::initial(),
                slot,
            });
            if child_index + 1 < target.routes().len() {
                next.child_slots.push(ProjectedChildSlotRole::mount_new(
                    child_index,
                    target.routes()[child_index + 1].slot().clone(),
                ));
            }
        }
        Some(next)
    }

    pub(crate) fn reissue_fresh_suffix(
        &self,
        target: &MatchedBranch,
        reserved: &[RouteFrameId],
    ) -> Option<Self> {
        if !self.instances_selected || reserved.len() != target.routes().len() {
            return None;
        }
        let mut next = self.clone();
        let Some(index) = self
            .fresh_entering()
            .next()
            .map(|frame| frame.branch_index())
        else {
            return Some(next);
        };
        next.child_slots.retain(|role| {
            role.parent_branch_index() < index || role.kind() == ProjectedChildSlotRoleKind::Leaving
        });
        for frame in &mut next.entering {
            if frame.branch_index() < index {
                continue;
            }
            frame.frame = reserved[frame.branch_index];
            frame.generation = RouteGeneration::initial();
            if frame.branch_index > index {
                frame.slot = ProjectedEnteringSlot::MountNew;
            }
            if frame.branch_index + 1 < target.routes().len() {
                next.child_slots.push(ProjectedChildSlotRole::mount_new(
                    frame.branch_index,
                    target.routes()[frame.branch_index + 1].slot().clone(),
                ));
            }
        }
        Some(next)
    }

    pub(crate) fn fresh_entering(
        &self,
    ) -> impl DoubleEndedIterator<Item = &ProjectedEnteringFrame> {
        self.entering.iter().filter(|frame| {
            self.source_at(frame.branch_index())
                == Some(crate::engine::instances::TargetInstanceSourceKind::Fresh)
        })
    }

    pub(crate) fn retained(&self) -> &[crate::engine::retained::RetainedLeafProjection] {
        &self.retained
    }
    pub(crate) const fn instances_selected(&self) -> bool {
        self.instances_selected
    }
    pub(crate) fn source_at(
        &self,
        index: usize,
    ) -> Option<crate::engine::instances::TargetInstanceSourceKind> {
        use crate::engine::instances::TargetInstanceSourceKind::*;
        if self
            .preserved
            .iter()
            .any(|frame| frame.branch_index() == index)
        {
            Some(Current)
        } else if index >= self.preserved.len()
            && index < self.preserved.len() + self.retained.len()
        {
            Some(Retained)
        } else if self
            .entering
            .iter()
            .any(|frame| frame.branch_index() == index)
        {
            Some(Fresh)
        } else {
            None
        }
    }

    pub(crate) fn matches_matched_target(&self, target: &MatchedBranch) -> bool {
        self.target == ReconciliationProjectionTarget::MatchedBranch
            && target.branch_fingerprint() == &self.target_fingerprint
    }

    pub(crate) fn matches_bound_error_target(
        &self,
        target_fingerprint: &RouteBranchFingerprint,
        boundary_index: usize,
        boundary_route: RouteId,
        frame: RouteFrameId,
        generation: RouteGeneration,
    ) -> bool {
        target_fingerprint == &self.target_fingerprint
            && self.target
                == ReconciliationProjectionTarget::BoundError {
                    boundary_index,
                    boundary_route,
                    frame,
                    generation,
                }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn matches_active(
        &self,
        router: RouterIdentity,
        host: RouteHostId,
        host_epoch: u64,
        top_slot: RouteSlotId,
        location: Option<&RouteLocation>,
        branch: Option<&MatchedBranch>,
        frames: &[RouteFrameVersion],
        child_slots: &[LogicalChildSlotBinding],
    ) -> bool {
        self.baseline.matches(
            router,
            host,
            host_epoch,
            top_slot,
            location,
            branch,
            frames,
            child_slots,
        )
    }

    pub(crate) fn preserved(&self) -> &[ProjectedExistingFrame] {
        &self.preserved
    }

    pub(crate) fn leaving(&self) -> &[ProjectedExistingFrame] {
        &self.leaving
    }

    pub(crate) fn entering(&self) -> &[ProjectedEnteringFrame] {
        &self.entering
    }

    pub(crate) fn child_slots(&self) -> &[ProjectedChildSlotRole] {
        &self.child_slots
    }

    pub(crate) const fn target_fingerprint(&self) -> &RouteBranchFingerprint {
        &self.target_fingerprint
    }
}

fn active_baseline_is_consistent(
    router: RouterIdentity,
    host: RouteHostId,
    top_slot: RouteSlotId,
    active_location: Option<&RouteLocation>,
    active_branch: Option<&MatchedBranch>,
    active_frames: &[RouteFrameVersion],
    active_child_slots: &[LogicalChildSlotBinding],
) -> bool {
    if active_branch.is_some() != active_location.is_some() {
        return false;
    }
    if let Some(branch) = active_branch {
        if branch.routes().len() != active_frames.len() {
            return false;
        }
        for (branch_index, (route, frame)) in branch.routes().iter().zip(active_frames).enumerate()
        {
            let expected_slot = if branch_index == 0 {
                Some(top_slot)
            } else {
                active_child_slots
                    .iter()
                    .find(|binding| {
                        binding.parent_branch_index() + 1 == branch_index
                            && binding.outlet() == route.slot()
                            && binding.child() == Some(frame)
                    })
                    .map(LogicalChildSlotBinding::slot)
            };
            if frame.router_identity() != router
                || frame.host() != host
                || frame.origin() != &FrameOrigin::MatchedRoute(route.route())
                || expected_slot != Some(frame.slot())
            {
                return false;
            }
        }
    } else {
        if active_location.is_some()
            || active_frames
                .iter()
                .enumerate()
                .any(|(branch_index, frame)| {
                    frame.router_identity() != router
                        || frame.host() != host
                        || (branch_index == 0 && frame.slot() != top_slot)
                })
        {
            return false;
        }
    }

    let mut seen_slots = Vec::with_capacity(active_child_slots.len());
    let mut seen_mappings = Vec::with_capacity(active_child_slots.len());
    active_child_slots.iter().all(|binding| {
        let parent_matches =
            active_frames.get(binding.parent_branch_index()) == Some(binding.parent());
        let child_matches = binding.child().is_none_or(|child| {
            active_frames.get(binding.parent_branch_index() + 1) == Some(child)
                && child.slot() == binding.slot()
        });
        let mapping = (binding.parent_branch_index(), binding.outlet().clone());
        parent_matches
            && child_matches
            && !seen_slots.contains(&binding.slot())
            && !seen_mappings.contains(&mapping)
            && {
                seen_slots.push(binding.slot());
                seen_mappings.push(mapping);
                true
            }
    })
}

fn project_child_slots(
    common_prefix: usize,
    target: &MatchedBranch,
    active_child_slots: &[LogicalChildSlotBinding],
) -> Vec<ProjectedChildSlotRole> {
    let mut projected = Vec::new();
    let mut consumed = Vec::new();

    for child_index in 1..target.routes().len() {
        let parent_index = child_index - 1;
        let outlet = target.routes()[child_index].slot();
        let existing = if parent_index < common_prefix {
            active_child_slots.iter().enumerate().find(|(_, binding)| {
                binding.parent_branch_index() == parent_index && binding.outlet() == outlet
            })
        } else {
            None
        };
        if let Some((index, binding)) = existing {
            consumed.push(index);
            let kind = if binding.child().is_some() {
                ProjectedChildSlotRoleKind::PreservedActive
            } else {
                ProjectedChildSlotRoleKind::ActivateExisting
            };
            projected.push(ProjectedChildSlotRole::existing(kind, binding.clone()));
        } else {
            projected.push(ProjectedChildSlotRole::mount_new(
                parent_index,
                outlet.clone(),
            ));
        }
    }

    let mut residual = active_child_slots
        .iter()
        .enumerate()
        .filter(|(index, _)| !consumed.contains(index))
        .map(|(_, binding)| {
            let kind = if binding.parent_branch_index() < common_prefix {
                ProjectedChildSlotRoleKind::RetainEmpty
            } else {
                ProjectedChildSlotRoleKind::Leaving
            };
            ProjectedChildSlotRole::existing(kind, binding.clone())
        })
        .collect::<Vec<_>>();
    residual.sort_by(|left, right| {
        right
            .parent_branch_index()
            .cmp(&left.parent_branch_index())
            .then_with(|| left.outlet().cmp(right.outlet()))
    });
    projected.extend(residual);
    projected
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
