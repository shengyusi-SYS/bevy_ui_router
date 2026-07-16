#![allow(dead_code)]

mod navigation {
    pub struct RouterNavigation;
}

use crate::{navigation::RouterNavigation as NavigationAlias};

impl NavigationAlias {
    pub fn force_private_authority(&mut self) {}
}
