#![allow(dead_code)]

extern crate evil_bevy_ecs as bevy_ecs;

#[derive(bevy_ecs::component::Component)]
#[component]
pub struct RouterHandle;

pub fn assert_api(value: &RouterHandle) {
    value.extern_authority();
}
