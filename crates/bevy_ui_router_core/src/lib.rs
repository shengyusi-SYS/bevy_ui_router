#![forbid(unsafe_code)]

pub mod ids;

pub mod prelude {
    pub use crate::ids::{
        RouteFrameId, RouteGeneration, RouteHostId, RouteId, RouteSlotId, RouterInstanceId,
    };
}
