#![allow(dead_code, non_local_definitions)]

pub struct RouterNavigation;
pub struct RouterHandle;
pub struct Entity;

fn hidden() {
    impl RouterNavigation {
        pub fn force_private_authority(&mut self) {}
    }

    impl From<RouterHandle> for Entity {
        fn from(_: RouterHandle) -> Self {
            Self
        }
    }
}
