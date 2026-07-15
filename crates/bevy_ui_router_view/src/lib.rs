#![forbid(unsafe_code)]
#![cfg_attr(
    not(feature = "test-support"),
    doc = r#"
```compile_fail
use bevy_ui_router_view::FakeViewRenderer;
```

```compile_fail
use bevy_ui_router_view::prelude::FakeViewRenderer;
```

```compile_fail
use bevy_ui_router_view::prelude::FakeViewRendererMode;
```

```compile_fail
use bevy_ui_router_view::test_support::FakeViewRenderer;
```
"#
)]
#![cfg_attr(
    feature = "test-support",
    doc = r#"
```compile_fail
use bevy_ui_router_view::FakeViewRenderer;
```

```compile_fail
use bevy_ui_router_view::prelude::{FakeViewRenderer, FakeViewRendererMode};

let mut renderer = FakeViewRenderer::new(FakeViewRendererMode::Success);
renderer.commit();
```

```compile_fail
use bevy_ui_router_view::prelude::{FakeViewRenderer, FakeViewRendererMode};
use bevy_ui_router_core::prelude::FrameApplyToken;

let mut renderer = FakeViewRenderer::new(FakeViewRendererMode::Success);
let token: FrameApplyToken = unimplemented!();
renderer.accept_apply_token(token);
```

```compile_fail
use bevy_ui_router_view::prelude::{FakeViewRenderer, FakeViewRendererMode};
use bevy_ui_router_core::prelude::FrameApplyRequest;

let mut renderer = FakeViewRenderer::new(FakeViewRendererMode::Success);
let request: FrameApplyRequest = unimplemented!();
renderer.finalize(request);
```

```compile_fail
use bevy_ui_router_view::prelude::ViewCommitAuthority;
```

```compile_fail
use bevy_ui_router_view::test_support::ViewFinalizeAuthority;
```
"#
)]

//! ```compile_fail
//! use bevy_ui_router_view::prelude::ViewBuildRequest;
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_view::prelude::ViewApplyOutcome;
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_view::prelude::ViewReportSession;
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_view::prelude::ViewReportedOutput;
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_view::ViewRequestId;
//!
//! let _ = ViewRequestId::from_raw(1);
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_view::ViewBuildRequest;
//!
//! let _ = ViewBuildRequest::new;
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_view::ViewOutputId;
//!
//! let _ = ViewOutputId::from_raw(1);
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_view::{ViewBuildRequest, ViewRenderPlan};
//!
//! let request: &ViewBuildRequest = unimplemented!();
//! let _ = ViewRenderPlan::new(request);
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_view::ViewDisposalHint;
//!
//! let _ = ViewDisposalHint::new("fake-output");
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_view::{
//!     ViewBindingKey, ViewOutputId, ViewPartialOutput, ViewRequestId,
//! };
//!
//! let request_id: ViewRequestId = unimplemented!();
//! let output_id: ViewOutputId = unimplemented!();
//! let _ = ViewPartialOutput::new(request_id, output_id, ViewBindingKey::new("leaf"), None);
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_view::{ViewFailure, ViewRequestId};
//!
//! let request_id: ViewRequestId = unimplemented!();
//! let _ = ViewFailure::build_failed(request_id, "build failed");
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_view::{ViewFailure, ViewRequestId};
//!
//! let request_id: ViewRequestId = unimplemented!();
//! let _ = ViewFailure::apply_failed(request_id, "apply failed", None);
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_view::{ViewFailure, ViewRequestId};
//!
//! let request_id: ViewRequestId = unimplemented!();
//! let _ = ViewFailure::stale_result(request_id, "stale result");
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_view::{RouteViewDescriptor, ViewBindingKey, ViewReportSession};
//!
//! let descriptor = RouteViewDescriptor::new(ViewBindingKey::new("leaf"));
//! let _ = ViewReportSession::from_frame_request(&descriptor, descriptor.clone());
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_view::prelude::FrameApplyToken;
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_view::prelude::NavigationTransaction;
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_view::ViewDriverKey;
//!
//! let key = ViewDriverKey::new("renderer").unwrap();
//! let _ = key.as_str();
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_view::ViewBuildEffect;
//!
//! let _ = ViewBuildEffect::new;
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_view::ViewBuildEffect;
//!
//! fn inspect(effect: &ViewBuildEffect) {
//!     let _ = effect.request_id();
//! }
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_view::ViewBuildEffect;
//!
//! fn inspect(effect: &ViewBuildEffect) {
//!     let _ = effect.generation();
//! }
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_view::ViewBuildEffect;
//!
//! fn inspect(effect: &ViewBuildEffect) {
//!     let _ = effect.entity();
//! }
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_view::ViewBuildEffect;
//!
//! fn inspect(effect: &ViewBuildEffect) {
//!     let _ = effect.task();
//! }
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_view::ViewBuildEffect;
//!
//! fn inspect(effect: &ViewBuildEffect) {
//!     let _ = effect.mount();
//! }
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_view::ViewBuildEffect;
//!
//! fn inspect(effect: &ViewBuildEffect) {
//!     let _ = effect.commit();
//! }
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_view::ViewBuildEffect;
//!
//! fn inspect(effect: &ViewBuildEffect) {
//!     let _ = effect.report();
//! }
//! ```

mod contract;
mod effect;
mod registry;

#[cfg(feature = "test-support")]
mod testing;

pub use contract::{
    RouteViewDescriptor, ViewApplyOutcome, ViewBindingKey, ViewBuildOutcome, ViewBuildRequest,
    ViewDisposalHint, ViewFailure, ViewOutputId, ViewPartialOutput, ViewRenderPlan,
    ViewReportSession, ViewReportedOutput, ViewRequestId,
};
pub use effect::ViewBuildEffect;
pub use registry::{RouteViewRegistry, RouteViewRegistryError, ViewDriverKey, ViewDriverKeyError};

#[cfg(feature = "test-support")]
pub mod test_support {
    pub use crate::testing::{FakeViewRenderer, FakeViewRendererMode};
}

pub mod prelude {
    pub use crate::{RouteViewDescriptor, ViewBindingKey};

    #[cfg(feature = "test-support")]
    pub use crate::test_support::{FakeViewRenderer, FakeViewRendererMode};
}
