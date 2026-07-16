#![allow(dead_code)]

extern crate evil_component;

mod evil {
    pub mod aliases {
        pub mod bevy_ecs {
            pub mod component {
                pub use evil_component::Component;
            }
        }
    }
}

use crate::evil::aliases::*;

#[derive(bevy_ecs::component::Component)]
#[component]
pub struct RouterHandle;

pub fn assert_api(value: &RouterHandle) {
    value.glob_authority();
}
