#![forbid(unsafe_code)]

pub mod back;
pub mod branch;
pub mod diagnostics;
pub mod error;
pub mod history;
pub mod ids;
pub mod location;
pub mod matcher;
pub mod meta;
pub mod ownership;
pub mod pattern;
pub mod route_tree;
pub mod slot;

pub mod prelude {
    pub use crate::back::{BackCapabilitySnapshot, BackIntent, BackPolicy};
    pub use crate::branch::{MatchedBranch, MatchedRoute, RouteParams};
    pub use crate::diagnostics::{
        DiagnosticEvent, DiagnosticEventKind, DiagnosticIdentity, DiagnosticSnapshot,
    };
    pub use crate::error::{RouteError, RouteErrorKind, RouteErrorSourceCategory};
    pub use crate::history::{HistoryError, MemoryHistory};
    pub use crate::ids::{
        RouteFrameId, RouteGeneration, RouteHostId, RouteId, RouteSlotId, RouterInstanceId,
    };
    pub use crate::location::RouteLocation;
    pub use crate::matcher::RouteMatcher;
    pub use crate::meta::{ErrorBoundaryKey, RouteExplicitKey, RouteMeta};
    pub use crate::ownership::{CleanupScope, OwnedRegion};
    pub use crate::pattern::{RoutePathSegment, RoutePattern, RoutePatternKind};
    pub use crate::route_tree::{RouteNodeOptions, RouteTree, RouteTreeBuilder};
    pub use crate::slot::RouteSlotKey;
}
