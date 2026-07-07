//! ```compile_fail
//! use bevy_ui_router_core::prelude::RouteFrameId;
//!
//! let _ = RouteFrameId(1);
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_core::prelude::RouteFrameId;
//!
//! let _ = RouteFrameId::new(1);
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_core::prelude::RouteGeneration;
//!
//! let _ = RouteGeneration::next;
//! ```

macro_rules! id_type {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(u64);

        impl $name {
            #[allow(dead_code)]
            pub(crate) const fn new(raw: u64) -> Self {
                Self(raw)
            }

            pub const fn raw(self) -> u64 {
                self.0
            }
        }
    };
}

id_type!(RouterInstanceId);
id_type!(RouteHostId);
id_type!(RouteSlotId);
id_type!(RouteId);
id_type!(RouteFrameId);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RouteGeneration(u64);

impl RouteGeneration {
    #[allow(dead_code)]
    pub(crate) const fn initial() -> Self {
        Self(0)
    }

    #[allow(dead_code)]
    pub(crate) const fn next(self) -> Self {
        Self(self.0 + 1)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_expose_raw_values_without_public_construction() {
        let router = RouterInstanceId::new(7);
        let host = RouteHostId::new(8);
        let slot = RouteSlotId::new(9);
        let route = RouteId::new(10);
        let frame = RouteFrameId::new(11);

        assert_eq!(router.raw(), 7);
        assert_eq!(host.raw(), 8);
        assert_eq!(slot.raw(), 9);
        assert_eq!(route.raw(), 10);
        assert_eq!(frame.raw(), 11);
    }

    #[test]
    fn generation_advances_inside_core() {
        let first = RouteGeneration::initial();
        let second = first.next();

        assert_eq!(first.raw(), 0);
        assert_eq!(second.raw(), 1);
        assert_ne!(first, second);
    }
}
