use bevy_ecs::prelude::Component;
use bevy_ui_router_core::{frame::RouteFrameVersion, ids::RouteSlotId, slot::RouteSlotKey};

use crate::registry::OwnershipSeal;

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

    pub(crate) const fn allow_all() -> Self {
        Self {
            pointer: true,
            keyboard: true,
            gamepad: true,
        }
    }

    pub(crate) const fn allows_all(&self) -> bool {
        self.pointer && self.keyboard && self.gamepad
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

#[derive(Component, Clone, Debug, Eq, PartialEq)]
pub(crate) struct ChildSlotAuthority {
    parent: RouteFrameVersion,
    outlet: RouteSlotKey,
    slot: RouteSlotId,
}

impl ChildSlotAuthority {
    pub(crate) fn new(parent: RouteFrameVersion, outlet: RouteSlotKey, slot: RouteSlotId) -> Self {
        Self {
            parent,
            outlet,
            slot,
        }
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
}

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ChildSlotLifecycle {
    ProvisionalInert,
    RegisteredInert,
    Quarantined,
}

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ChildSlotBarrierProbe;

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ChildSlotNodeProbe;

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ChildSlotAuthorityProbe;

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ChildSlotRelationshipProbe;
