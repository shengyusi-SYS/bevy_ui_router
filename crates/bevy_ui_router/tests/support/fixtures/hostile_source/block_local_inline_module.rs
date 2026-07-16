#![allow(dead_code, non_local_definitions)]

pub struct RouterNavigation;

fn container() {
    mod outer {
        use crate::RouterNavigation as H;

        mod inner {
            impl super::H {
                pub fn local_module_authority(&mut self) {}
            }
        }
    }
}

pub fn assert_api(value: &mut RouterNavigation) {
    value.local_module_authority();
}
