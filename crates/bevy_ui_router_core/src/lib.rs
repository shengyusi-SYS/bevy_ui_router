#![forbid(unsafe_code)]

pub mod ids;
pub mod location;

pub mod prelude {
    pub use crate::ids::{
        RouteFrameId, RouteGeneration, RouteHostId, RouteId, RouteSlotId, RouterInstanceId,
    };
    pub use crate::location::RouteLocation;
}
