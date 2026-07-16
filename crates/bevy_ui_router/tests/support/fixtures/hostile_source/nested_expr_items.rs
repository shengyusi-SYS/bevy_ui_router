#![allow(dead_code, non_local_definitions, unreachable_code)]

pub struct RouterNavigation;

fn sink<T>(_: T) {}

fn hidden() {
    sink({
        impl RouterNavigation {
            pub fn call_authority(&self) {}
        }
        0_u8
    });
    let _ = ({
        impl RouterNavigation {
            pub fn tuple_authority(&self) {}
        }
        0_u8
    },);
    return {
        impl RouterNavigation {
            pub fn return_authority(&self) {}
        }
    };
}
