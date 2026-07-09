use bevy_ecs::prelude::{Entity, Resource, World};
use bevy_ui_router_core::branch::MatchedBranch;
use bevy_ui_router_core::frame::FrameCleanupPlan;
use bevy_ui_router_core::runtime::{BeginNavigationError, FrameApplyReportError, RouterRuntime};
use bevy_ui_router_core::transaction::{
    FrameApplyOutcome, FrameApplyRequest, NavigationTransactionSnapshot, NavigationTransactionState,
};
use bevy_ui_router_view::{RouteViewDescriptor, ViewRequestId};

use crate::bridge::{
    BeginFrameApplySessionError, EcsFrameApplyReport, EcsFrameApplySession,
    EcsFrameApplySessionStateHandle, EcsFrameApplyStatus, FinishFrameApplySessionError,
};
use crate::cleanup::{
    CleanupCapability, cleanup_frame_apply_request_subtree, cleanup_router_owned_subtree,
    cleanup_unclaimed_frame_root_subtree,
};
use crate::components::{
    RouteFocusSnapshot, RouteFrameRoot, RouteFrameRootLifecycleState, RouteHost, RouteSlot,
};
use crate::finalize::{
    InactiveFinalizationCapability, PostCommitActivation, PrepareInertCapability,
    activate_committed_root, finalize_inactive_root, prepare_inert_root,
};

#[derive(Clone, Debug, Eq, PartialEq)]
struct PendingFrameApplySession {
    request: FrameApplyRequest,
    view_request_id: ViewRequestId,
    session_state: EcsFrameApplySessionStateHandle,
    reported_root: Option<Entity>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompleteFrameApplySessionError {
    SessionNotRegistered,
    MissingReport,
    MissingRoot,
    InvalidRoot,
    CoreStaleRejected,
    CoreReportRejected,
    CoreCommitRejected,
    CleanupAckRejected,
}

#[derive(Resource, Debug)]
pub struct RouterEcsState {
    runtime: RouterRuntime,
    pending_apply_requests: Vec<PendingFrameApplySession>,
}

impl RouterEcsState {
    pub fn new() -> Self {
        Self {
            runtime: RouterRuntime::new(),
            pending_apply_requests: Vec::new(),
        }
    }

    pub(crate) const fn runtime(&self) -> &RouterRuntime {
        &self.runtime
    }

    pub const fn router(&self) -> bevy_ui_router_core::ids::RouterInstanceId {
        self.runtime.router()
    }

    pub const fn host(&self) -> bevy_ui_router_core::ids::RouteHostId {
        self.runtime.host()
    }

    pub const fn slot(&self) -> bevy_ui_router_core::ids::RouteSlotId {
        self.runtime.slot()
    }

    pub fn route_host(&self) -> RouteHost {
        RouteHost::from_state(self)
    }

    pub fn route_slot(&self) -> RouteSlot {
        RouteSlot::from_state(self)
    }

    pub fn begin_first_navigation(
        &mut self,
        target: &MatchedBranch,
    ) -> Result<NavigationTransactionSnapshot, BeginNavigationError> {
        self.runtime.begin_first_navigation(target)
    }

    pub fn begin_frame_apply_session(
        &mut self,
        request: &FrameApplyRequest,
        descriptor: RouteViewDescriptor,
    ) -> Result<EcsFrameApplySession, BeginFrameApplySessionError> {
        if self
            .pending_apply_requests
            .iter()
            .any(|pending| pending.request == *request)
        {
            return Err(BeginFrameApplySessionError::RequestAlreadyPending);
        }

        let Some(pending) = self.runtime.pending_snapshot() else {
            return Err(BeginFrameApplySessionError::RequestNotPending);
        };
        if !pending
            .requests()
            .iter()
            .any(|candidate| candidate == request)
        {
            return Err(BeginFrameApplySessionError::RequestNotPending);
        }

        let session = EcsFrameApplySession::new(request, descriptor);
        self.pending_apply_requests.push(PendingFrameApplySession {
            request: request.clone(),
            view_request_id: session.view_request_id(),
            session_state: session.state_handle(),
            reported_root: None,
        });
        Ok(session)
    }

    pub fn complete_frame_apply_session(
        &mut self,
        world: &mut World,
        session: &mut EcsFrameApplySession,
        root: Option<Entity>,
        inactive_roots: &[Entity],
    ) -> Result<EcsFrameApplyReport, CompleteFrameApplySessionError> {
        let pending_index = self
            .pending_apply_index(session)
            .ok_or(CompleteFrameApplySessionError::SessionNotRegistered)?;
        let report = session
            .report()
            .ok_or(CompleteFrameApplySessionError::MissingReport)?;

        match report.status() {
            EcsFrameApplyStatus::Pending => {
                self.complete_successful_apply(world, session, pending_index, root, inactive_roots)
            }
            EcsFrameApplyStatus::Failed => self.complete_failed_apply(world, session, root, report),
            EcsFrameApplyStatus::StaleDiscarded => {
                if root.is_some() {
                    cleanup_frame_apply_request_subtree(
                        world,
                        session.request(),
                        CleanupCapability,
                    );
                }
                session.mark_terminal_cleanup_complete();
                Ok(session
                    .report()
                    .expect("stale report should remain recorded"))
            }
            EcsFrameApplyStatus::Finalized | EcsFrameApplyStatus::ReportedToCore => Ok(report),
        }
    }

    pub fn finish_frame_apply_session(
        &mut self,
        session: &EcsFrameApplySession,
    ) -> Result<(), FinishFrameApplySessionError> {
        let Some(index) = self.pending_apply_index(session) else {
            return Err(FinishFrameApplySessionError::SessionNotRegistered);
        };

        if !session.can_finish() {
            return Err(FinishFrameApplySessionError::SessionNotTerminal);
        }

        self.pending_apply_requests.remove(index);
        Ok(())
    }

    fn complete_successful_apply(
        &mut self,
        world: &mut World,
        session: &mut EcsFrameApplySession,
        pending_index: usize,
        root: Option<Entity>,
        inactive_roots: &[Entity],
    ) -> Result<EcsFrameApplyReport, CompleteFrameApplySessionError> {
        let root = root.ok_or(CompleteFrameApplySessionError::MissingRoot)?;
        if !root_matches_request(world, root, session.request()) {
            cleanup_invalid_apply_root(world, root, session.request());
            session.mark_terminal_cleanup_complete();
            return Err(CompleteFrameApplySessionError::InvalidRoot);
        }

        let prepared = prepare_inert_root(world, root, PrepareInertCapability);
        if prepared.lifecycle()
            != Some(crate::components::RouteFrameRootLifecycleSnapshot::PreparedInert)
        {
            cleanup_frame_apply_request_subtree(world, session.request(), CleanupCapability);
            session.mark_terminal_cleanup_complete();
            return Err(CompleteFrameApplySessionError::InvalidRoot);
        }

        for inactive_root in inactive_roots {
            finalize_inactive_root(world, *inactive_root, InactiveFinalizationCapability);
        }

        let snapshot = match self
            .runtime
            .report_frame_apply(session.request().token(), FrameApplyOutcome::Applied)
        {
            Ok(snapshot) => snapshot,
            Err(FrameApplyReportError::Stale(_stale)) => {
                cleanup_frame_apply_request_subtree(world, session.request(), CleanupCapability);
                session.mark_terminal_cleanup_complete();
                return Err(CompleteFrameApplySessionError::CoreStaleRejected);
            }
            Err(_error) => {
                cleanup_frame_apply_request_subtree(world, session.request(), CleanupCapability);
                session.mark_terminal_cleanup_complete();
                return Err(CompleteFrameApplySessionError::CoreReportRejected);
            }
        };
        self.pending_apply_requests[pending_index].reported_root = Some(root);

        if snapshot.state() == NavigationTransactionState::ReadyToCommit {
            self.commit_and_activate_reported_roots(world, &snapshot)?;
        } else {
            session.mark_reported_to_core();
        }

        Ok(session
            .report()
            .expect("success report should remain recorded"))
    }

    fn complete_failed_apply(
        &mut self,
        world: &mut World,
        session: &mut EcsFrameApplySession,
        root: Option<Entity>,
        report: EcsFrameApplyReport,
    ) -> Result<EcsFrameApplyReport, CompleteFrameApplySessionError> {
        let failure = report
            .failure()
            .ok_or(CompleteFrameApplySessionError::MissingReport)?;
        let snapshot = match self.runtime.report_frame_apply(
            session.request().token(),
            FrameApplyOutcome::Failed(failure.error().clone()),
        ) {
            Ok(snapshot) => snapshot,
            Err(FrameApplyReportError::Stale(_stale)) => {
                if root.is_some() {
                    cleanup_frame_apply_request_subtree(
                        world,
                        session.request(),
                        CleanupCapability,
                    );
                }
                session.mark_terminal_cleanup_complete();
                return Err(CompleteFrameApplySessionError::CoreStaleRejected);
            }
            Err(_error) => {
                if root.is_some() {
                    cleanup_frame_apply_request_subtree(
                        world,
                        session.request(),
                        CleanupCapability,
                    );
                }
                session.mark_terminal_cleanup_complete();
                return Err(CompleteFrameApplySessionError::CoreReportRejected);
            }
        };

        let cleanup = snapshot
            .failure()
            .map(|failure| failure.cleanup().to_vec())
            .unwrap_or_default();
        self.cleanup_core_plans(world, &cleanup);
        if root.is_some() {
            cleanup_frame_apply_request_subtree(world, session.request(), CleanupCapability);
        }
        self.runtime
            .acknowledge_failed_navigation_cleanup(&cleanup)
            .map_err(|_error| CompleteFrameApplySessionError::CleanupAckRejected)?;
        self.mark_reported_sessions_terminal_cleanup_complete();
        session.mark_terminal_cleanup_complete();

        Ok(session
            .report()
            .expect("failure report should remain recorded"))
    }

    fn commit_and_activate_reported_roots(
        &mut self,
        world: &mut World,
        snapshot: &NavigationTransactionSnapshot,
    ) -> Result<(), CompleteFrameApplySessionError> {
        self.runtime
            .commit_ready_navigation()
            .map_err(|_error| CompleteFrameApplySessionError::CoreCommitRejected)?;
        let current_requests = snapshot.requests();
        let mut reported_roots = self
            .pending_apply_requests
            .iter()
            .filter(|pending| {
                current_requests
                    .iter()
                    .any(|request| request == &pending.request)
            })
            .filter_map(|pending| {
                pending.reported_root.map(|root| {
                    (
                        pending.request.frame().branch_index(),
                        pending.request.token().sequence(),
                        root,
                        pending.session_state.clone(),
                    )
                })
            })
            .collect::<Vec<_>>();
        reported_roots
            .sort_by_key(|(branch_index, sequence, _root, _state)| (*branch_index, *sequence));

        for (_branch_index, _sequence, root, session_state) in reported_roots {
            activate_committed_root(world, root, PostCommitActivation);
            session_state.mark_finalized();
        }
        Ok(())
    }

    fn cleanup_core_plans(&self, world: &mut World, cleanup: &[FrameCleanupPlan]) {
        let mut ordered_cleanup = cleanup.to_vec();
        ordered_cleanup.sort_by_key(|plan| {
            (
                plan.identity().router().raw(),
                plan.identity().host().raw(),
                plan.identity().slot().raw(),
                plan.identity().route().raw(),
                plan.identity().frame().raw(),
                plan.identity().generation().raw(),
            )
        });

        for plan in ordered_cleanup {
            cleanup_router_owned_subtree(world, &plan, CleanupCapability);
        }
    }

    fn pending_apply_index(&self, session: &EcsFrameApplySession) -> Option<usize> {
        self.pending_apply_requests.iter().position(|pending| {
            pending.request == *session.request()
                && pending.view_request_id == session.view_request_id()
        })
    }

    fn mark_reported_sessions_terminal_cleanup_complete(&self) {
        for pending in &self.pending_apply_requests {
            if pending.reported_root.is_some() {
                pending.session_state.mark_terminal_cleanup_complete();
            }
        }
    }
}

fn root_matches_request(world: &World, root: Entity, request: &FrameApplyRequest) -> bool {
    let Some(frame_root) = world.get::<RouteFrameRoot>(root) else {
        return false;
    };
    let request_identity = request.frame().identity();

    frame_root.identity() == request_identity
        && frame_root.branch_index() == request.frame().branch_index()
}

fn cleanup_invalid_apply_root(world: &mut World, root: Entity, request: &FrameApplyRequest) {
    if world.get::<RouteFrameRoot>(root).is_some()
        && world.get::<RouteFrameRootLifecycleState>(root).is_none()
    {
        cleanup_unclaimed_frame_root_subtree(world, root, CleanupCapability);
    } else {
        cleanup_frame_apply_request_subtree(world, request, CleanupCapability);
    }
}

impl Default for RouterEcsState {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(dead_code)]
#[derive(Default, Resource, Debug)]
pub(crate) struct RouteFocusState {
    focused: Option<Entity>,
}

#[allow(dead_code)]
impl RouteFocusState {
    pub(crate) const fn focused(&self) -> Option<Entity> {
        self.focused
    }

    pub(crate) fn set_focused(&mut self, focused: Option<Entity>) {
        self.focused = focused;
    }

    pub(crate) const fn snapshot(&self) -> RouteFocusSnapshot {
        RouteFocusSnapshot::new(self.focused)
    }
}

#[cfg(test)]
mod tests {
    use bevy_camera::visibility::Visibility;
    use bevy_ecs::prelude::{Entity, World};
    use bevy_picking::Pickable;
    use bevy_ui::InteractionDisabled;
    use bevy_ui_router_core::branch::MatchedBranch;
    use bevy_ui_router_core::branch_diff::BranchDiff;
    use bevy_ui_router_core::location::RouteLocation;
    use bevy_ui_router_core::matcher::RouteMatcher;
    use bevy_ui_router_core::pattern::RoutePattern;
    use bevy_ui_router_core::route_tree::RouteTree;
    use bevy_ui_router_core::runtime::BeginNavigationError;
    use bevy_ui_router_view::{RouteViewDescriptor, ViewBindingKey};

    use crate::components::RouteFrameRoot;

    use super::{CompleteFrameApplySessionError, RouterEcsState};

    #[test]
    fn core_stale_rejection_discards_local_partial_output_without_forged_ecs_token() {
        let target = single_match("/");
        let mut state = RouterEcsState::new();
        let snapshot = state
            .begin_first_navigation(&target)
            .expect("navigation should begin");
        let request = snapshot.requests()[0].clone();
        let mut session = state
            .begin_frame_apply_session(&request, descriptor("root"))
            .expect("session should mint from the original pending request");
        let mut world = World::new();
        let (host, root, child) = mount_frame_root(&mut world, &state, &request);

        session.record_apply_success(Some("renderer-output"));

        let mut replacement_runtime = bevy_ui_router_core::runtime::RouterRuntime::new();
        replacement_runtime
            .begin_first_navigation(&target)
            .expect("replacement runtime should hold a different pending transaction");
        state.runtime = replacement_runtime;

        let error = state
            .complete_frame_apply_session(&mut world, &mut session, Some(root), &[])
            .expect_err("replacement runtime should reject the original token as stale");

        assert_eq!(error, CompleteFrameApplySessionError::CoreStaleRejected);
        assert!(session.finalized());
        assert!(world.get_entity(host).is_ok(), "app-owned host must remain");
        assert!(
            world.get_entity(root).is_err(),
            "stale partial root is removed"
        );
        assert!(
            world.get_entity(child).is_err(),
            "stale partial subtree is removed"
        );
        assert_eq!(
            state.begin_first_navigation(&target),
            Err(BeginNavigationError::PendingNavigation),
            "core stale rejection must leave the replacement pending navigation unchanged"
        );
        state
            .finish_frame_apply_session(&session)
            .expect("stale-cleaned session should release ECS pending state");
    }

    #[test]
    fn later_commit_does_not_reactivate_unfinished_sessions_from_previous_navigation() {
        let tree = items_route_tree();
        let matcher = RouteMatcher::new(tree);
        let first_target = matched_branch(&matcher, "/items/1");
        let second_target = matched_branch(&matcher, "/items/2");
        let diff = BranchDiff::compare(&first_target, &second_target);
        let mut state = RouterEcsState::new();
        let mut world = World::new();

        let first_snapshot = state
            .begin_first_navigation(&first_target)
            .expect("first navigation should begin");
        let first_requests = first_snapshot.requests().to_vec();
        let mut first_sessions = begin_sessions(&mut state, &first_requests, "first");
        let first_roots = first_requests
            .iter()
            .map(|request| {
                let (_host, root, _child) = mount_frame_root(&mut world, &state, request);
                root
            })
            .collect::<Vec<_>>();

        complete_successful_sessions(&mut state, &mut world, &mut first_sessions, &first_roots);
        for root in &first_roots {
            assert_active_root(&world, *root);
        }

        let second_snapshot = state
            .runtime
            .begin_reconciled_navigation(&second_target, &diff)
            .expect("second reconciled navigation should begin");
        let second_requests = second_snapshot.requests().to_vec();
        assert!(
            !second_requests.is_empty(),
            "second navigation should have entering frame requests"
        );
        let mut second_sessions = begin_sessions(&mut state, &second_requests, "second");
        let second_roots = second_requests
            .iter()
            .map(|request| {
                let (_host, root, _child) = mount_frame_root(&mut world, &state, request);
                root
            })
            .collect::<Vec<_>>();

        complete_successful_sessions(&mut state, &mut world, &mut second_sessions, &second_roots);

        for root in &first_roots {
            assert_active_root(&world, *root);
        }
        for root in &second_roots {
            assert_active_root(&world, *root);
        }
        for session in &first_sessions {
            state
                .finish_frame_apply_session(session)
                .expect("old finalized sessions should remain finishable after later commit");
        }
    }

    fn complete_successful_sessions(
        state: &mut RouterEcsState,
        world: &mut World,
        sessions: &mut [crate::bridge::EcsFrameApplySession],
        roots: &[Entity],
    ) {
        for (index, session) in sessions.iter_mut().enumerate() {
            session.record_apply_success(Some(format!("frame-{index}")));
            state
                .complete_frame_apply_session(world, session, Some(roots[index]), &[])
                .expect("session should complete successfully");
        }
    }

    fn begin_sessions(
        state: &mut RouterEcsState,
        requests: &[bevy_ui_router_core::transaction::FrameApplyRequest],
        prefix: &str,
    ) -> Vec<crate::bridge::EcsFrameApplySession> {
        requests
            .iter()
            .enumerate()
            .map(|(index, request)| {
                state
                    .begin_frame_apply_session(request, descriptor(format!("{prefix}-{index}")))
                    .expect("session should mint from pending request")
            })
            .collect()
    }

    fn mount_frame_root(
        world: &mut World,
        state: &RouterEcsState,
        request: &bevy_ui_router_core::transaction::FrameApplyRequest,
    ) -> (Entity, Entity, Entity) {
        let host = world.spawn((state.route_host(),)).id();
        let slot = world.spawn((state.route_slot(),)).id();
        let root = world
            .spawn((
                RouteFrameRoot::from_frame_request(request),
                Visibility::Visible,
                Pickable::default(),
            ))
            .id();
        let child = world.spawn((Visibility::Visible, Pickable::default())).id();

        world.entity_mut(host).add_child(slot);
        world.entity_mut(slot).add_child(root);
        world.entity_mut(root).add_child(child);

        (host, root, child)
    }

    fn descriptor(binding: impl Into<String>) -> RouteViewDescriptor {
        RouteViewDescriptor::new(ViewBindingKey::new(binding))
    }

    fn single_match(path: &str) -> bevy_ui_router_core::branch::MatchedBranch {
        RouteMatcher::new(route_tree())
            .match_location(&RouteLocation::new(path))
            .unwrap()
    }

    fn matched_branch(matcher: &RouteMatcher, path: &str) -> MatchedBranch {
        matcher.match_location(&RouteLocation::new(path)).unwrap()
    }

    fn route_tree() -> RouteTree {
        RouteTree::builder()
            .root(RoutePattern::path("/").unwrap())
            .unwrap()
            .build()
            .unwrap()
    }

    fn items_route_tree() -> RouteTree {
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
        builder.build().unwrap()
    }

    fn assert_active_root(world: &World, root: Entity) {
        assert!(world.get_entity(root).is_ok(), "active root should exist");
        assert_eq!(world.get::<Visibility>(root), Some(&Visibility::Visible));
        assert_eq!(world.get::<Pickable>(root), Some(&Pickable::default()));
        assert!(world.get::<InteractionDisabled>(root).is_none());
    }
}
