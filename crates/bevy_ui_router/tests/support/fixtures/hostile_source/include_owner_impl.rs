#![allow(dead_code)]

pub struct RouterNavigation;

include!("include_owner_impl_payload.rs");

pub fn assert_api(value: &mut RouterNavigation) {
    value.include_authority();
}
