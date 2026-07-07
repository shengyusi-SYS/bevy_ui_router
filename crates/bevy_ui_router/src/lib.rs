#![forbid(unsafe_code)]
#![cfg_attr(
    not(feature = "view-test-support"),
    doc = r#"
```compile_fail
use bevy_ui_router::prelude::FakeViewRenderer;
```

```compile_fail
use bevy_ui_router::prelude::FakeViewRendererMode;
```

```compile_fail
use bevy_ui_router::view::FakeViewRenderer;
```

```compile_fail
use bevy_ui_router::view::test_support::FakeViewRenderer;
```

```compile_fail
use bevy_ui_router::view::prelude::FakeViewRendererMode;
```

```compile_fail
use bevy_ui_router::prelude::ViewBuildRequest;
```
"#
)]
#![cfg_attr(
    feature = "view-test-support",
    doc = r#"
```compile_fail
use bevy_ui_router::prelude::{FakeViewRenderer, FakeViewRendererMode};

let mut renderer = FakeViewRenderer::new(FakeViewRendererMode::Success);
renderer.commit();
```

```compile_fail
use bevy_ui_router::prelude::{FakeViewRenderer, FakeViewRendererMode, FrameApplyToken};

let mut renderer = FakeViewRenderer::new(FakeViewRendererMode::Success);
let token: FrameApplyToken = unimplemented!();
renderer.accept_apply_token(token);
```

```compile_fail
use bevy_ui_router::prelude::{FakeViewRenderer, FakeViewRendererMode, FrameApplyRequest};

let mut renderer = FakeViewRenderer::new(FakeViewRendererMode::Success);
let request: FrameApplyRequest = unimplemented!();
renderer.finalize(request);
```

```compile_fail
use bevy_ui_router::prelude::ViewCommitAuthority;
```

```compile_fail
use bevy_ui_router::view::test_support::ViewFinalizeAuthority;
```
"#
)]

pub use bevy_ui_router_core as core;

#[cfg(feature = "view")]
pub use bevy_ui_router_view as view;

#[cfg(feature = "ecs")]
pub use bevy_ui_router_ecs as ecs;

#[cfg(feature = "bsn")]
pub use bevy_ui_router_bsn as bsn;

#[cfg(feature = "platform")]
pub use bevy_ui_router_platform as platform;

pub mod prelude {
    #[allow(unused_imports)]
    pub use bevy_ui_router_core::prelude::*;

    #[cfg(feature = "view")]
    #[allow(unused_imports)]
    pub use bevy_ui_router_view::prelude::*;
}
