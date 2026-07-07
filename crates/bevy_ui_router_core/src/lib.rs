#![forbid(unsafe_code)]

pub mod history;
pub mod ids;
pub mod location;

pub mod prelude {
    pub use crate::history::{HistoryError, MemoryHistory};
    pub use crate::ids::{
        RouteFrameId, RouteGeneration, RouteHostId, RouteId, RouteSlotId, RouterInstanceId,
    };
    pub use crate::location::RouteLocation;
}
