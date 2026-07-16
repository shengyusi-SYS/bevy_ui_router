#![allow(dead_code, non_local_definitions)]

pub struct RouterNavigation;

fn hidden() {
    macro_rules! add_api {
        () => {
            impl RouterNavigation {
                pub fn macro_authority(&self) {}
            }
        };
    }
    add_api!();
}
