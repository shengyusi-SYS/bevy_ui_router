use bevy_ecs::prelude::{Entity, Resource};
use bevy_ui_router_core::runtime::RouterRuntime;

use crate::components::{RouteFocusSnapshot, RouteHost, RouteSlot};

#[derive(Resource, Debug)]
pub struct RouterEcsState {
    runtime: RouterRuntime,
}

impl RouterEcsState {
    pub fn new() -> Self {
        Self {
            runtime: RouterRuntime::new(),
        }
    }

    pub fn from_runtime(runtime: RouterRuntime) -> Self {
        Self { runtime }
    }

    pub const fn runtime(&self) -> &RouterRuntime {
        &self.runtime
    }

    pub fn runtime_mut(&mut self) -> &mut RouterRuntime {
        &mut self.runtime
    }

    pub fn route_host(&self) -> RouteHost {
        RouteHost::from_runtime(self.runtime())
    }

    pub fn route_slot(&self) -> RouteSlot {
        RouteSlot::from_runtime(self.runtime())
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
