use crate::RouterNavigation as NavigationAlias;

mod inner {
    impl crate::outer::NavigationAlias {
        pub fn force_private_authority(&mut self) {}
    }
}
