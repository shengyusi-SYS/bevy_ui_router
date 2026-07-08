use bevy_ecs::prelude::{Entity, Resource};
use bevy_ui_router_core::runtime::RouterRuntime;
use bevy_ui_router_core::transaction::FrameApplyRequest;
use bevy_ui_router_view::{RouteViewDescriptor, ViewRequestId};

use crate::bridge::{
    BeginFrameApplySessionError, EcsFrameApplySession, FinishFrameApplySessionError,
};
use crate::components::{RouteFocusSnapshot, RouteHost, RouteSlot};

#[derive(Clone, Debug, Eq, PartialEq)]
struct PendingFrameApplySession {
    request: FrameApplyRequest,
    view_request_id: ViewRequestId,
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
        });
        Ok(session)
    }

    pub fn finish_frame_apply_session(
        &mut self,
        session: &EcsFrameApplySession,
    ) -> Result<(), FinishFrameApplySessionError> {
        if !session.can_finish() {
            return Err(FinishFrameApplySessionError::SessionNotTerminal);
        }

        let Some(index) = self.pending_apply_requests.iter().position(|pending| {
            pending.request == *session.request()
                && pending.view_request_id == session.view_request_id()
        }) else {
            return Err(FinishFrameApplySessionError::SessionNotRegistered);
        };

        self.pending_apply_requests.remove(index);
        Ok(())
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
