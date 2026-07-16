#![allow(dead_code)]

use bevy_ui_router_core::prelude::{RouteViewInput, RouteViewKey};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewRequestId(u64);

impl ViewRequestId {
    pub(crate) const fn from_raw(value: u64) -> Self {
        Self(value)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }
}

pub struct ViewBuildEffect {
    view: RouteViewKey,
    input: RouteViewInput,
    request_id: ViewRequestId,
}

impl ViewBuildEffect {
    pub(crate) fn new(
        view: RouteViewKey,
        input: RouteViewInput,
        request_id: ViewRequestId,
    ) -> Self {
        Self {
            view,
            input,
            request_id,
        }
    }

    pub fn view(&self) -> &RouteViewKey {
        &self.view
    }

    pub fn request_id(&self) -> ViewRequestId {
        self.request_id
    }

    pub fn input(&self) -> &RouteViewInput {
        &self.input
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ui_router_core::prelude::{RouteViewInput, RouteViewKey};

    #[test]
    fn effect_exposes_only_read_only_view_and_input_shapes() {
        let _: for<'a> fn(&'a ViewBuildEffect) -> &'a RouteViewKey = ViewBuildEffect::view;
        let _: for<'a> fn(&'a ViewBuildEffect) -> &'a RouteViewInput = ViewBuildEffect::input;
    }

    #[test]
    fn construction_and_request_identity_are_owner_private_shapes() {
        let _: fn(RouteViewKey, RouteViewInput, ViewRequestId) -> ViewBuildEffect =
            ViewBuildEffect::new;
        let _: fn(&ViewBuildEffect) -> ViewRequestId = ViewBuildEffect::request_id;
        let _: fn(u64) -> ViewRequestId = ViewRequestId::from_raw;
    }

    #[test]
    fn request_id_has_the_final_read_only_value_projection() {
        let request_id = ViewRequestId::from_raw(7);

        assert_eq!(request_id.raw(), 7);
    }

    #[test]
    fn request_id_observes_distinct_frame_task_sequences() {
        let parent = ViewRequestId::from_raw(11);
        let child = ViewRequestId::from_raw(12);

        assert_ne!(parent, child);
        assert_eq!((parent.raw(), child.raw()), (11, 12));
    }
}
