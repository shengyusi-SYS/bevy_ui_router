#![forbid(unsafe_code)]

mod app_ext;
#[cfg(feature = "test-support")]
mod audit;
#[cfg(test)]
mod authority_regression_tests;
mod branch_transaction;
mod cleanup;
mod components;
mod content;
mod driver;
mod frame_local;
mod host;
mod input;
mod lifecycle_preparation;
mod mount;
mod navigation;
mod plugin;
mod presentation;
mod registry;
mod snapshots;
#[cfg(test)]
mod task10_regression_tests;
#[cfg(feature = "test-support")]
#[doc(hidden)]
pub mod test_support;

pub use app_ext::{
    AppRouterExt, RouterConfig, RouterHandle, RouterHostRegistration, RouterInstallError,
    RouterInstallErrorKind, ViewDriverRegistrationError, ViewDriverRegistrationErrorKind,
};
#[cfg(feature = "test-support")]
pub use audit::{
    AuditEntityId, AuditEntityState, AuthorityContentMapping, AuthorityFrameLifecycle,
    AuthorityFrameMapping, AuthorityHostMapping, AuthorityOutletMapping, AuthoritySlotKind,
    AuthoritySlotMapping, RouterAuthorityAuditSnapshot, RouterAuthorityAudits,
    RouterLifecycleAuditEvent, RouterLifecycleAuditEventKind,
};
pub use content::{
    ContentColor, ContentEntity, ContentPrimitiveError, ContentPrimitiveErrorKind,
    ContentPrimitiveKind, ContentWriteError, ContentWriteErrorKind, FrameContentWriter, SolidRect,
};
pub use driver::{EcsViewDriver, EcsViewReportSession, ViewBuildReport, ViewBuildReportKind};
pub use input::RouterInputGate;
pub use navigation::{NavigationEnqueueError, NavigationEnqueueErrorKind, RouterNavigation};
pub use plugin::{BevyUiRouterPlugin, RouterInputSet, RouterScheduleSet};
pub use snapshots::{NavigationRequestSnapshot, RequestLookup, RouterSnapshot, RouterSnapshots};

pub mod prelude {
    pub use crate::{
        AppRouterExt, BevyUiRouterPlugin, ContentColor, ContentEntity, ContentPrimitiveError,
        ContentPrimitiveErrorKind, ContentPrimitiveKind, ContentWriteError, ContentWriteErrorKind,
        EcsViewDriver, EcsViewReportSession, FrameContentWriter, NavigationEnqueueError,
        NavigationEnqueueErrorKind, NavigationRequestSnapshot, RequestLookup, RouterConfig,
        RouterHandle, RouterHostRegistration, RouterInputGate, RouterInputSet, RouterInstallError,
        RouterInstallErrorKind, RouterNavigation, RouterScheduleSet, RouterSnapshot,
        RouterSnapshots, SolidRect, ViewBuildReport, ViewBuildReportKind,
        ViewDriverRegistrationError, ViewDriverRegistrationErrorKind,
    };

    #[cfg(feature = "test-support")]
    pub use crate::{
        AuditEntityId, AuditEntityState, AuthorityContentMapping, AuthorityFrameLifecycle,
        AuthorityFrameMapping, AuthorityHostMapping, AuthorityOutletMapping, AuthoritySlotKind,
        AuthoritySlotMapping, RouterAuthorityAuditSnapshot, RouterAuthorityAudits,
        RouterLifecycleAuditEvent, RouterLifecycleAuditEventKind,
    };
}
