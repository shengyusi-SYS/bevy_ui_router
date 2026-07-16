#![allow(dead_code)]

pub struct RouterNavigation;
pub struct RouterHandle;
pub struct Entity;

mod outer {
    use crate::RouterHandle as H;
    use crate::RouterNavigation as NavigationAlias;

    mod inner {
        impl super::NavigationAlias {
            pub fn force_private_authority(&mut self) {}
        }

        impl From<crate::outer::H> for crate::Entity {
            fn from(_: crate::outer::H) -> Self {
                Self
            }
        }
    }
}
