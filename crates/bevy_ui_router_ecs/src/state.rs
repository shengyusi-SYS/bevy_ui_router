use bevy_ecs::prelude::{Entity, Resource, World};
use bevy_ui_router_core::branch::MatchedBranch;
use bevy_ui_router_core::frame::FrameCleanupPlan;
use bevy_ui_router_core::runtime::{BeginNavigationError, RouterRuntime};
use bevy_ui_router_core::transaction::{
    FrameApplyOutcome, FrameApplyRequest, NavigationTransactionSnapshot, NavigationTransactionState,
};
use bevy_ui_router_view::{RouteViewDescriptor, ViewRequestId};

use crate::bridge::{
    BeginFrameApplySessionError, EcsFrameApplyReport, EcsFrameApplySession, EcsFrameApplyStatus,
    FinishFrameApplySessionError,
};
use crate::cleanup::{
    CleanupCapability, cleanup_frame_apply_request_subtree, cleanup_router_owned_subtree,
};
use crate::components::{RouteFocusSnapshot, RouteHost, RouteSlot};
use crate::finalize::{
    InactiveFinalizationCapability, PostCommitActivation, PrepareInertCapability,
    activate_committed_root, finalize_inactive_root, prepare_inert_root,
};

#[derive(Clone, Debug, Eq, PartialEq)]
struct PendingFrameApplySession {
    request: FrameApplyRequest,
    view_request_id: ViewRequestId,
    reported_root: Option<Entity>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompleteFrameApplySessionError {
    SessionNotRegistered,
    MissingReport,
    MissingRoot,
    InvalidRoot,
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

        let session = EcsFrameApplySession::new(request, descriptor);
        self.pending_apply_requests.push(PendingFrameApplySession {
            request: request.clone(),
            view_request_id: session.view_request_id(),
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
            .cloned()
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
                    .expect("stale report should remain recorded")
                    .clone())
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

        if !session.can_finish()
            && !self.reported_session_can_finish_after_navigation_end(index, session)
        {
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
            Err(_error) => {
                cleanup_frame_apply_request_subtree(world, session.request(), CleanupCapability);
                session.mark_terminal_cleanup_complete();
                return Err(CompleteFrameApplySessionError::CoreReportRejected);
            }
        };
        self.pending_apply_requests[pending_index].reported_root = Some(root);

        if snapshot.state() == NavigationTransactionState::ReadyToCommit {
            self.commit_and_activate_reported_roots(world, session)?;
        } else {
            session.mark_reported_to_core();
        }

        Ok(session
            .report()
            .expect("success report should remain recorded")
            .clone())
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
        session.mark_terminal_cleanup_complete();

        Ok(session
            .report()
            .expect("failure report should remain recorded")
            .clone())
    }

    fn commit_and_activate_reported_roots(
        &mut self,
        world: &mut World,
        session: &mut EcsFrameApplySession,
    ) -> Result<(), CompleteFrameApplySessionError> {
        self.runtime
            .commit_ready_navigation()
            .map_err(|_error| CompleteFrameApplySessionError::CoreCommitRejected)?;
        let mut reported_roots = self
            .pending_apply_requests
            .iter()
            .filter_map(|pending| {
                pending.reported_root.map(|root| {
                    (
                        pending.request.frame().branch_index(),
                        pending.request.token().sequence(),
                        root,
                    )
                })
            })
            .collect::<Vec<_>>();
        reported_roots.sort_by_key(|(branch_index, sequence, _root)| (*branch_index, *sequence));

        for (_branch_index, _sequence, root) in reported_roots {
            activate_committed_root(world, root, PostCommitActivation);
        }
        session.mark_finalized();
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

    fn reported_session_can_finish_after_navigation_end(
        &self,
        pending_index: usize,
        session: &EcsFrameApplySession,
    ) -> bool {
        session.status() == Some(EcsFrameApplyStatus::ReportedToCore)
            && self.pending_apply_requests[pending_index]
                .reported_root
                .is_some()
            && self.runtime.pending_snapshot().is_none()
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
