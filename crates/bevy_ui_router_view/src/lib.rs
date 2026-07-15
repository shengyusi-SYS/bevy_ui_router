#![forbid(unsafe_code)]

mod effect;
mod registry;

pub use effect::{ViewBuildEffect, ViewRequestId};
pub use registry::{
    RouteViewDescriptor, RouteViewRegistry, RouteViewRegistryError, ViewDriverKey,
    ViewDriverKeyError,
};

#[doc(hidden)]
pub mod integration {
    use bevy_ui_router_core::prelude::{RouteViewInput, RouteViewKey};

    use crate::{RouteViewRegistry, ViewBuildEffect, ViewDriverKey};

    pub fn driver_key<'a>(
        registry: &'a RouteViewRegistry,
        view: &RouteViewKey,
    ) -> Option<&'a ViewDriverKey> {
        registry
            .get(view)
            .map(super::registry::RouteViewDescriptor::driver)
    }

    pub fn build_effect(
        view: RouteViewKey,
        input: RouteViewInput,
        request_sequence: u64,
    ) -> ViewBuildEffect {
        ViewBuildEffect::new(
            view,
            input,
            super::effect::ViewRequestId::from_raw(request_sequence),
        )
    }
}

pub mod prelude {
    pub use crate::{
        RouteViewDescriptor, RouteViewRegistry, RouteViewRegistryError, ViewBuildEffect,
        ViewDriverKey, ViewDriverKeyError, ViewRequestId,
    };
}
