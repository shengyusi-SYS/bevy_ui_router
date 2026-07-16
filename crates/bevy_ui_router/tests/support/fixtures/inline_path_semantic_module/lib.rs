#![allow(dead_code)]

pub struct RouterNavigation;

mod wrapper {
    #[path = "physical.rs"]
    mod outer;
}

mod outer;
