#![allow(dead_code)]

mod navigation {
    pub struct RouterNavigation;
}

use crate::navigation::{self as navigation_alias};

impl navigation_alias::RouterNavigation {
    pub fn force_private_authority(&mut self) {}
}
