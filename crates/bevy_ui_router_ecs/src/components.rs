use bevy_ecs::prelude::{Component, Entity};
use bevy_ui_router_core::frame::RouteFrameIdentity;
use bevy_ui_router_core::runtime::RouterRuntime;
use bevy_ui_router_core::transaction::FrameApplyRequest;
use bevy_ui_router_core::{ids::RouteHostId, ids::RouteSlotId, ids::RouterInstanceId};

use crate::registry::OwnershipSeal;
use crate::state::RouterEcsState;

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RouteOwnershipSeal(OwnershipSeal);

impl RouteOwnershipSeal {
    pub(crate) const fn new(seal: OwnershipSeal) -> Self {
        Self(seal)
    }
    pub(crate) const fn seal(self) -> OwnershipSeal {
        self.0
    }
}

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RouterInputBarrier {
    pointer: bool,
    keyboard: bool,
    gamepad: bool,
}

impl RouterInputBarrier {
    pub(crate) const fn deny_all() -> Self {
        Self {
            pointer: false,
            keyboard: false,
            gamepad: false,
        }
    }
    pub(crate) const fn denies_all(&self) -> bool {
        !self.pointer && !self.keyboard && !self.gamepad
    }
}

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProvisionalSlotLifecycle {
    PreparedInert,
    Quarantined,
}

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProvisionalStagingProbe;

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProvisionalPostWriteProbe;

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct RouteHost {
    router: RouterInstanceId,
    host: RouteHostId,
}

impl RouteHost {
    pub(crate) fn from_runtime(runtime: &RouterRuntime) -> Self {
        Self {
            router: runtime.router(),
            host: runtime.host(),
        }
    }

    pub(crate) fn from_state(state: &RouterEcsState) -> Self {
        Self::from_runtime(state.runtime())
    }

    pub const fn router(&self) -> RouterInstanceId {
        self.router
    }

    pub const fn host(&self) -> RouteHostId {
        self.host
    }
}

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct RouteSlot {
    router: RouterInstanceId,
    host: RouteHostId,
    slot: RouteSlotId,
}

impl RouteSlot {
    pub(crate) fn from_runtime(runtime: &RouterRuntime) -> Self {
        Self {
            router: runtime.router(),
            host: runtime.host(),
            slot: runtime.slot(),
        }
    }

    pub(crate) fn from_state(state: &RouterEcsState) -> Self {
        Self::from_runtime(state.runtime())
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

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct RouteFrameRoot {
    identity: RouteFrameIdentity,
    branch_index: usize,
}

impl RouteFrameRoot {
    pub fn from_frame_request(request: &FrameApplyRequest) -> Self {
        Self {
            identity: *request.frame().identity(),
            branch_index: request.frame().branch_index(),
        }
    }

    pub const fn identity(&self) -> &RouteFrameIdentity {
        &self.identity
    }

    pub const fn branch_index(&self) -> usize {
        self.branch_index
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RouteFrameRootLifecycleSnapshot {
    Preparing,
    PreparedInert,
    Active,
    Leaving,
    Hidden,
    Failed,
    CleanupPending,
    Cached,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RouteFrameInteractionSnapshot {
    accepts_pointer: bool,
    accepts_keyboard: bool,
    accepts_gamepad: bool,
}

impl RouteFrameInteractionSnapshot {
    pub const fn accepts_pointer(&self) -> bool {
        self.accepts_pointer
    }

    pub const fn accepts_keyboard(&self) -> bool {
        self.accepts_keyboard
    }

    pub const fn accepts_gamepad(&self) -> bool {
        self.accepts_gamepad
    }

    #[allow(dead_code)]
    pub(crate) const fn new(
        accepts_pointer: bool,
        accepts_keyboard: bool,
        accepts_gamepad: bool,
    ) -> Self {
        Self {
            accepts_pointer,
            accepts_keyboard,
            accepts_gamepad,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RouteFocusSnapshot {
    focused: Option<Entity>,
}

impl RouteFocusSnapshot {
    pub const fn focused(&self) -> Option<Entity> {
        self.focused
    }

    #[allow(dead_code)]
    pub(crate) const fn new(focused: Option<Entity>) -> Self {
        Self { focused }
    }
}

#[allow(dead_code)]
#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RouteFrameRootLifecycleState {
    snapshot: RouteFrameRootLifecycleSnapshot,
}

#[allow(dead_code)]
impl RouteFrameRootLifecycleState {
    pub(crate) const fn new(snapshot: RouteFrameRootLifecycleSnapshot) -> Self {
        Self { snapshot }
    }

    pub(crate) const fn snapshot(&self) -> RouteFrameRootLifecycleSnapshot {
        self.snapshot
    }
}

#[allow(dead_code)]
#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RouteFrameInteractionBarrier {
    accepts_pointer: bool,
    accepts_keyboard: bool,
    accepts_gamepad: bool,
}

#[allow(dead_code)]
impl RouteFrameInteractionBarrier {
    pub(crate) const fn new(
        accepts_pointer: bool,
        accepts_keyboard: bool,
        accepts_gamepad: bool,
    ) -> Self {
        Self {
            accepts_pointer,
            accepts_keyboard,
            accepts_gamepad,
        }
    }

    pub(crate) const fn snapshot(&self) -> RouteFrameInteractionSnapshot {
        RouteFrameInteractionSnapshot::new(
            self.accepts_pointer,
            self.accepts_keyboard,
            self.accepts_gamepad,
        )
    }
}
