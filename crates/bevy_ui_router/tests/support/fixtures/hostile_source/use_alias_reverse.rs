#![allow(dead_code)]

mod handles {
    pub struct RouterHandle;
}

use crate::handles::RouterHandle as H;

pub struct Entity;

impl From<H> for Entity {
    fn from(_: H) -> Self {
        Self
    }
}
